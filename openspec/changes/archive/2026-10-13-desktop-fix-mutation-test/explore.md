# 桌面突变测试缺失根因与「拆步 + Rust 突变 + 分段」升级调研

> 状态：探索中（2026-10-12）；主题与背景骨架 + 首轮调研结论已落盘。

## 主题与背景

近几次 packages/desktop 变更记录的 `reports/test/summary.json` 中 `mutation: null`（突变测试未执行）。本探索：

1. 分析 mutation:null 的根因（是否 bug、缺在哪些层）
2. 升级一：突变测试与执行测试拆成两个 step（test-execution 阶段先修测试/覆盖率，绿了再跑突变）
3. 升级二：补充 Rust 突变测试能力
4. 升级三：待突变文件过多时自动拆段顺序执行（降单任务时长、降重试成本）

## 首轮调研证据

### mutation:null 根因（结论：设计性缺失，不是回归 bug）

- 双实现出线点切换：`summary.command` 由 `dev-team test-execution`（TS CLI，带 Stryker）切到
  `desktop change run: test-execution gate`（桌面 Rust 原生 checks 域，约 2026-10-08 起）。
  - CLI 产出的 change（至 2026-10-10-desktop-change-db-identity）：mutation 有值（80~95 分，Stryker）。
  - 桌面 gate 产出的 change（2026-10-08 起）：mutation 恒 null。
- 三层原因叠加：
  1. **桌面 checks 域 V1 明文不移植 mutation**：`openspec/specs/desktop-test-execution/spec.md`
     V1 边界第 1 条「mutation 不移植：报告 mutation: null 合法」；`infra/checks/src/testexec/mod.rs:394`
     构造 SubReport 硬编码 `mutation: None`；`core/checks/src/aggregate.rs` `aggregate_mutation`
     注释明说「生产聚合恒 None」；walker 机械 checklist 第三条写死「mutation null 自动通过（V1 恒真）」。
  2. **配置开着但没人消费**：`openspec/config.json` 里 `packages/desktop` suite `mutation.score: 80`、
     `src-tauri` `70`；桌面 `core/config` 的 `MutationConfig{cwd,score}` 解析在位，但 `testexec/detect.rs`
     的 `TestPlan` 没有 mutation 字段——检测产物根本不带突变面。
  3. **Rust 从来没有突变能力**：CLI 框架注册表 `test-framework.ts` 只有 jest/vitest/vite-plus 有
     `mutation_execution`（`npx --prefix "{prefix}" stryker run "{config}"`）；rust/go/bun/pytest/node-test
     均无。历史上哪怕走 CLI，`packages/desktop/src-tauri`（rust 档 70）的突变也从未跑过
     （历史 summary 的 mutation overrides 只有 `packages/desktop` 一个 glob）。
- 旁支发现：2026-10-08/09/10 有四个 change summary `plans: []`、total 0、duration 1s、conclusion pass、
  findings「全部框架未产出覆盖数据」——零执行绿过（gate 文件门径过滤掉全部 plan 也算 pass）。与 mutation
  缺失叠加，近期变更实际测试面收窄。
- 耗时数据（拆段动机）：CLI Stryker 51~69 文件单场 335s~1209s（5~20 分钟），timeout 上限 3h；
  历史 commit 反复调 Stryker（3h timeout、并发封顶、tmp 目录）都是单场长任务的止痛药。

### 现状架构落点（两套实现）

```
CLI（plugins/dev-team）                          桌面（packages/desktop/src-tauri）
─────────────────────────                        ─────────────────────────────────
dev-team test-execution（单命令一把梭）            walker: TEST_EXECUTION_STEPS
  detect → 逐 plan:                              PhaseStart
    测试执行（模板 spawn）                          → TestExecution (ToolCommand::TestExecution)
    → runMutationPhase（紧跟，Stryker）               check 链: detect→完整性→reuse→
    → mutation.json → MutationBlock                   spawn→report→aggregate→diagnose
  汇总 computeMutationResult                       → PhaseLog（机械 checklist，3 条）
    （计数求和公式，非百分比平均）                     mutation 恒 None，第 3 条「null 自动通过」
  复用门: mutationRequired && mutation===null
    → 不复用（--skip-mutation 报告不满足门）
突变范围: --change → change 清单 written 桶
  + 反推源文件 + net-zero 去噪（与 HEAD 同内容剔除）
```

关键既有件（升级可复用）：
- `MutationBlock`/`MutationMeasured` schema 双侧已对齐（Rust `core/checks/src/model.rs` 保真 CLI zod）；
  corpus 有 `mutation-block` fixture。
- `aggregate_mutation`（Rust）/`computeMutationScoreFromCounts`（TS）都是**计数求和再算分**，
  天然支持「分段结果归并」——段当子报告即可，不需要新聚合公式。
- `MutationMeasured` 已有 `compileError/runtimeError/ignored` 字段，cargo-mutants 的
  unviable/timeout/not-mutated 有映射落点。
- 步词汇是 1:1 全映射封闭集：`StepNode`（core/workflow）↔ `ChangeStepKind`（core/orchestration）
  ↔ `RunStepKind`（infra/store）↔ `bindings.ts` ↔ 前端流程视图；加一步是白名单式扩展但涟漪面固定。

### 三项升级的落点草图

1. **拆步**：`TEST_EXECUTION_STEPS` 变
   `PhaseStart → TestExecution → MutationTest(新) → PhaseLog`；walker 顺序执行天然保证
   「测试/覆盖率修绿后才跑突变」。需新增 `StepNode::MutationTest`（18→19 封闭集）、
   `ToolCommand::MutationTest`/`ToolStepOutput::MutationTest`/`MutationRunner` port、
   独立反馈边预算（补测试杀突变）。CLI 侧对应拆 `--tests-only`/`--mutation-only`（或子命令），
   summary 单/双文件是决策点（影响 evaluator T3 与复用门）。
2. **Rust 突变**：cargo-mutants 是唯一现实候选（JSON 输出、`--file` 过滤、`--in-diff`、
   per-mutant timeout、`--jobs`、baseline 缓存）。CLI 注册表 rust 框架补 `mutation_execution`；
   `mutation-parser` 加第二解析分支；score 口径需对齐 Stryker 公式
   `(killed+timeout)/(total-ignored-compileError-runtimeError)`（决策点：unviable→compileError？）。
3. **自动拆段**：文件数超阈值 → 字典序均分 k 段顺序执行，段级报告 `mutation-<i>.json`，
   计数归并出总块；段级复用门断点续跑（重试只重跑未完段）；每段独立 timeout。

## 用户决策（2026-10-12 第二轮）

1. **只处理桌面端**：CLI（plugins/dev-team）不动；拆步只在桌面 walker 侧做。
2. **突变范围 = diff 确认**：不用 change 清单反推那套；框架支持"仅突变 diff"时（cargo-mutants
   `--in-diff`）优先 diff 方案（行级精度），否则按 diff 文件列表喂 mutate（Stryker）。
3. **null 语义先不收紧**：mutation:null 保持合法（观察期），机械 checklist 第 3 条与 evaluator
   T3 不改；持续观察结果后再决定是否升格 error。
4. **"零计划绿过"旁支不治**。

## 第二轮调研：落地细节

### diff 范围来源（决策 2 的锚点）

- `desktop-change-worktree` spec 已保证：**worktree 内 `git diff HEAD` 恰为本 change 编辑集**
  （基线恒 HEAD、编辑囚于 worktree）——突变范围的天然权威，无需清单。
- **坑：untracked 新文件不在 `git diff HEAD` 里**（新建源文件没 add 就不进 diff）。候选补法：
  `git status --porcelain` 未跟踪面并入（CLI 的 `expandMutationDiffWithInferredSources` 也有
  "brand-new 文件保留"先例）；或 create/executor 后 `git add -N`。需要定口径。
- `DiffContextPort`（orchestration::port）只有声明无实现无接线（产出还是拼接文本）；突变范围
  需要**结构化 name-only 清单**，落点应在 vcs 边界（infra/vcs git.rs 已有 `diff --name-only`
  先例，加 `changed_files(root, base)` 之类的口子，实现落 infra、port 归消费者）。

### cargo-mutants（Rust 突变）衔接（决策 2 的执行面）

```
MutationTest 步（rust suite）
  diff.patch ← git diff HEAD（worktree）
  cargo mutants --in-diff diff.patch \
      --json reports/test/<plan>/mutation.json \
      --timeout <T> --jobs <N> [--baseline skip] \
      （分段时：patch 按文件拆 k 片，逐段 --in-diff seg-i.patch 顺序跑）
  outcomes → MutationMeasured 映射（口径决策点）：
    caught→killed  missed→survived  timeout→timeout
    unviable→compileError?  not-mutated→ignored?
  score 沿用既有公式 (killed+timeout)/(total-ignored-compileError-runtimeError)
  （决策点：若 unviable 不映射 compileError 而映射 ignored，分母口径不同 → 分数漂移）
```

- `--baseline skip`：拆步后 TestExecution 已保证测试绿跑，基线可省——每段少跑一遍全量测试。
- `--in-diff` 的分段：一个 patch 喂一次；分段即按文件拆 patch 片。`--file` 过滤是备选。
- Stryker 侧无 in-diff：沿 CLI 先例 `mutate: [diff 文件列表]`（`restrictMutationScope` 同款），
  分段即列表切片。

### 拆步落点（决策 1 的改动面清单）

- `phase_table.rs` `TEST_EXECUTION_STEPS`：
  `PhaseStart → TestExecution → MutationTest(新) → PhaseLog`。
  TestExecution fail 走既有反馈边修测试/覆盖率（绿了才走到 MutationTest）——满足"修复后
  再执行突变"；MutationTest fail 走**独立反馈边**（补测试杀突变，`feedback_limit(step, label)`
  既有机制，label 独立计数）。
- 步词汇封闭集扩展（1:1 全映射涟漪面）：`StepNode::MutationTest`（core/workflow）→
  `ChangeStepKind::MutationTest`（core/orchestration state）→ `RunStepKind`（infra/store）→
  `bindings.ts` → 前端流程视图（现有步骤渲染直接吃）。
- port 三件套：`ToolCommand::MutationTest { change_id }` / `ToolStepOutput::MutationTest(载荷)`
  / `MutationRunner` port（照 TestExecutionRunner 同式；spawn 不进 core，实现落 infra/checks
  testexec/ 旁新 `mutation/` 模块或 testexec 内子模块）。
- 机械 checklist：PhaseLog 产物面挂 test_outcome + mutation_outcome 双槽；第 3 条"mutation null
  自动通过"保留（决策 3 观察期），但 evidence 改为区分"跑过且达标 / diff 空跳过 / null"。
- 复用门：MutationTest 段独立复用（段报告在位 + 输入 mtime）；分段后段级断点续跑
  （重试只重跑未完段）。
- Rust 侧改动面均在 `packages/desktop/src-tauri`（user：只处理桌面端），TS CLI 零改动。

### 拆段触发口径（决策 2 语境下的新认识）

- diff 面通常小（change 编辑集），"文件过多"是大 change / 全量场景的兜底；触发阈值候选：
  diff 文件数 N（如 >10）或估算突变体数。段粒度：文件字典序均分 k 段。
- 顺序执行 + 段级报告 `mutation-seg-<i>.json` + 计数归并（`aggregate_mutation` 现公式直接吃）。

## 用户决策（2026-10-12 第三轮）

1. **change 文件清单获取：实现由我定，但整体封装成一个功能**（不要散落多处取 diff）。
2. **unviable → compileError** 定案（沿 `MutationMeasured.compileError` 字段，分母排除口径
   与 Stryker compileError 同语义）。
3. **并发与拆段目标**：突变并发 = 一半 CPU（现状延续）；拆分后**单轮尽量不超过 10 分钟**。

## 第三轮调研：清单封装、并发现状、10 分钟预算换算

### change 文件清单封装（决策 1 的落位定案）

- **现状钉点**：`"一半 CPU"` 在 CLI `stryker-config.ts:128`（`concurrency: os.cpus().length / 2`，
  注释「本地突变仅占用一半内核，保障用户其他操作」）；桌面 Rust 侧等价物
  `available_parallelism()/2`，`--jobs` 与段执行并发同锚。
- **封装定案**：vcs 边界（`infra/vcs/src/git.rs`）单点封装
  `change_edit_set(root, base) -> Vec<String>`：
  - 跟踪改动：`git diff <base> --name-only -z`（base=HEAD，worktree 语义下即本 change 编辑集）
  - untracked 并入：`git ls-files --others --exclude-standard -z`（等价 porcelain 未跟踪面、
    已展开目录、尊重 gitignore）
  - 排序去重、POSIX 归一后返回；**这是突变范围与文件清单的唯一入口**（清单被 MutationRunner
    与未来消费者共用，整体封装满足）。
- port 面沿「port 归消费者」先例：`MutationRunner::run(root, change, files)` 收清单入参，
  walker 经 vcs 缝（`ArchiveVcsPort` 扩结构化方法或并列新 port——design 定稿）取清单注入；
  checks-runtime 依赖面不扩 vcs。CLI 不动（其清单/反推逻辑保留原样）。

### 10 分钟预算换算（决策 3 的拆段依据）

历史实测（半 CPU Stryker，CLI 时代 summary）：

| change | 文件 | mutants | 耗时 | 速率 |
|---|---|---|---|---|
| desktop-change-db-identity | 69 | 1075 | 1209s | 1.12 s/mutant |
| desktop-change-flow | 51+66 | 1075 | 973s | 0.90 s/mutant |
| sdk-engine-gap-closure | 67 | 191 | 335s | 1.75 s/mutant |

- **每文件耗时跨 5~19 秒波动（4 倍差），按文件数静态切段不可靠**；按 mutant 数估算是主线
  （半 CPU 下约 1~2 s/mutant，10 分钟预算 ≈ 350~550 mutants/段）。
- 但 mutant 数事先不可知（跑完才知道）→ **静态初值 + 自适应修正**：
  1. 首段按保守常数切（文件行数估 mutant 密度，或文件数兜底常数）；
  2. 顺序执行天然知道上一段实测速率（秒/mutant），动态调整后续段大小；
  3. 段级独立 timeout 保护（10 分钟量级的段软目标，不是硬失败语义）。
- cargo-mutants 侧：`--jobs = available_parallelism()/2`、per-mutant `--timeout` 防单突变体挂死
  （与「段不超过 10 分钟」正交：段是分组粒度，mutant timeout 是单体护栏）。

## 用户决策（2026-10-12 第四轮）

1. **`not-mutated → ignored` 同意**。→ 映射表完整定案：
   `caught→killed / missed→survived / timeout→timeout / unviable→compileError /
   not-mutated→ignored`，score 沿用既有公式（分母排除 ignored + compileError + runtimeError）。
2. **单文件超预算接受**（段是软目标）；**拆分预留浮动空间：按 5 分钟分段**（预算 10 分钟的
   一半切段，实测溢出到 10 分钟可接受）；**分段多轮执行 MUST 复用 sandbox，避免多轮初始化**。
3. （反馈边明细面用户未表态——暂按我方建议：存活突变体清单照 findings_detail 同款截断
   口径，提案时再确认。）
4. **config schema 不扩展**：jobs / timeout / 拆段预算 / 分段目标均写常量观察，不进
   suite.mutation 字段。

## 第四轮调研：sandbox 复用落点

### 分段多轮的初始化成本与复用点

```
分段执行（顺序，共享 sandbox / 缓存）
─────────────────────────────────────────────────────
js (Stryker):                       rust (cargo-mutants):
  segment-1: stryker run seg-1       segment-1: cargo mutants seg-1 ─┐
  segment-2: stryker run seg-2       segment-2: cargo mutants seg-2  │ 共享 CARGO_TARGET_DIR
  ...                                ...                             │ （增量编译缓存跨段复用，
  segment-k: stryker run seg-k       segment-k: cargo mutants seg-k ─┘  workspace 只全量编译一次）
  ├─ 共享同一 tempDirPath（sandbox 只初始化一次）
  └─ cleanupMutationArtifacts 移到【全部段完成后】一次清理
```

- **Stryker 侧**：现状 `tempDirPath = reportDir/_stryker-tmp`、`cleanupMutationArtifacts` 每场
  run 后清理 config + tmp。分段改造点：① 各段**共用同一 `tempDirPath`**（sandbox 建置 +
  node_modules 链接/安装只发生一次，后续段只换 `mutate` 文件列表与 config）；② 清理时点
  从"每段后"移到"全段完成后"（含临时 config 文件攒批清理）。
- **cargo-mutants 侧**：天然共享 workspace `target/`（每段增量编译，不重置 target dir）；
  `--baseline skip` 已定案（TestExecution 绿跑保证基线）。段间只换 `--in-diff` 的 patch 片。
- **拆段参数定案**：分段目标 **5 分钟/段**（半 CPU 约 250~300 mutants/段按 1~2 s/mutant 折算），
  段级软护栏 10 分钟；单文件超预算接受；自适应修正以上一段实测速率调整后续段大小。

## 用户决策（2026-10-12 第五轮）：反馈边②形态定案

1. **双时点反馈**：
   - 突变执行**中断**（段执行 error / harness 崩 / 超时）→ **立即反馈 executor**，不等汇总；
   - 全部段正常跑完 → **最后汇总突变得分**，仅未达标（score < threshold）才反馈。
   - 达标 → pass 直接收口（机械 checklist 落账，零反馈）。
2. **原始突变报告全程留存 reports 目录**（各段原始报告，不清理）；反馈内容只需
   **告知报告文件路径**，MUST NOT 内联存活突变体清单。
3. **遗留开放点 #1（明细截断口径）关闭**：原"存活突变体有界装配"方案作废，被
   「结论 + 报告路径指引」取代——反馈 prompt 里的明细面只含结论摘要
   （score / threshold / 中断错误摘要），明细交给 executor 自行读报告。

### 反馈边②最终形态

```
MutationTest 步（分段顺序执行，共享 sandbox）
  seg-1 ──► seg-2 ──► … ──► seg-k
    │                      │
    │ 中断（error/超时）      │ 全部完成
    ▼                      ▼
  立即反馈 executor      汇总: Σ计数 → score = (killed+timeout)
  「突变中断于 seg-i:       /(total-ignored-compileError-runtimeError)
   错误摘要 + 报告路径清单」   ├─ score ≥ threshold → PASS 收口（零反馈）
    │                      └─ score <  threshold → 反馈 executor
    │                        「score X% < threshold Y%
    │                         + 原始报告路径清单」
    ▼                      ▼
  executor 读 reports 下原始报告自行定位存活突变体 / 中断原因
  （修复 → 输入 mtime 变 → 复用门失效 → 重跑）
```

- **报告留存**：`reports/test/<plan>/mutation/`（或段级 `mutation-seg-<i>.json`）下
  原始 Stryker / cargo-mutants 报告全程保留；sandbox/临时 config 的清理不影响
  报告工件（cleanup 只动 `_stryker-tmp` 与临时 config）。
- **预算计数**：中断反馈与未达标反馈都走 `FeedbackLoop` 独立计数（照既有
  `feedback_limit` 机制，label 独立），超限升格相位 fail 不变。

### ⚠ spec 写作警示（防误统一）

`desktop-test-execution` 现行反馈边①原则是「修复会话输入面自足、**MUST NOT 依赖
修复会话自读报告文件**」（findings_detail 有界内联）。反馈边②**反其道**：结论 +
报告路径指引，executor 自读报告。**这是两种口径并存、系用户明确决策**，spec delta
必须显式写成两反馈边的口径差异（MutationTest 反馈边 requirement 自带报告指引式语义），
MUST NOT 被后来者"统一"掉；机械 checklist 第 3 条「mutation null 自动通过」保留不变
（决策 3 观察期）。

## 收口状态与 spec delta 估计（提案输入）

方案主线已收口（根因 / 拆步 / diff 清单封装 / Rust 映射表 / 拆段与 sandbox 复用 / 常量配置）。
预期 spec 改动面（单 change 或拆多 change 提案时的规格量级）：

| spec | 改动 |
|---|---|
| `desktop-test-execution` | **主战场**：V1 边界第 1 条「mutation 不移植」撤销改写（null 语义保持合法——决策 3 观察期）；新增 MutationTest 步 requirement（diff 范围 / 分段 / sandbox 复用 / 反馈边②独立预算）；「步骤可观测」加 MutationTest 步词汇；「执行链」拆步语义改写 |
| `desktop-checks-domain` | infra/checks 模块表加 mutation 执行面（testexec 旁 mutation/ 子模块）；port 表加 MutationRunner |
| `desktop-change-orchestration` | 相位表 TEST_EXECUTION_STEPS 步序列改写（PhaseStart→TestExecution→MutationTest→PhaseLog） |
| `desktop-crate-layout` | checks 边界行注记扩 mutation 面 |
| 新 spec `desktop-mutation-testing`（或并入 test-execution） | 映射表 / 分段 / sandbox 复用 / 清单封装契约（`change_edit_set`）——提案时定归属 |
| `desktop-change-worktree` | 估计零改动（清单封装落 vcs，不碰 worktree 语义） |
| CLI 系 spec（mutation-testing / unit-test-executor 等） | **零改动**（决策 1：只处理桌面端） |

## 遗留开放点

无。原 #1（明细截断口径）已由第五轮「双时点反馈 + 报告指引式」解消。全部主线决策收口，可转 propose。
