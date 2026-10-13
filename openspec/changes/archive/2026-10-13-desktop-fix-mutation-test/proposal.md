# 提案: desktop-fix-mutation-test

> **变更**: desktop-fix-mutation-test
> **日期**: 2026-10-13
> **状态**: draft
> **探索**: `openspec/changes/desktop-fix-mutation-test/explore.md`（五轮探索已收敛：根因定性 + 拆步 + diff 清单封装 + Rust 映射表 + 拆段与 sandbox 复用 + 双时点反馈边）

---

## 问题

近几次 `packages/desktop` 变更的 `reports/test/summary.json` 中 `mutation: null`（突变测试未执行）。根因**不是回归 bug，而是设计性缺失**，三层叠加：

1. **桌面 checks 域 V1 明文不移植 mutation**：`desktop-test-execution` V1 边界第 1 条「mutation 不移植：报告 `mutation: null` 合法」；`infra/checks/src/testexec/mod.rs:394` 构造 SubReport 硬编码 `mutation: None`；`core/checks/src/aggregate.rs` `aggregate_mutation` 注释明说「生产聚合恒 None」；walker 机械 checklist 第 3 条写死「mutation null 自动通过（V1 恒真）」。
2. **配置开着但没人消费**：`openspec/config.json` 里 `packages/desktop` suite `mutation.score: 80`、`packages/desktop/src-tauri` `70`；桌面 `core/config` 的 `MutationConfig{cwd, score}` 解析在位，但 `testexec/detect.rs` 的 `TestPlan` 没有 mutation 字段——检测产物根本不带突变面。
3. **Rust 从来没有突变能力**：CLI 框架注册表 `test-framework.ts` 只有 jest / vitest / vite-plus 有 `mutation_execution`（Stryker）；rust 档无突变执行器。历史上哪怕走 CLI，`packages/desktop/src-tauri`（rust 档 70）的突变也从未跑过。

双实现出线点切换（约 2026-10-08 起 `summary.command` 由 `dev-team test-execution` 切到 `desktop change run: test-execution gate`）后 mutation 恒 null；CLI 时代同 change mutation 有值（80~95 分，Stryker）。叠加「零执行绿过」旁支（`plans: []` 也算 pass，本变更不治），近期变更实际测试面收窄。

拆步动机：现状突变紧跟测试执行一把梭，单场 Stryker 51~69 文件 335s~1209s（5~20 分钟），失败重试成本高；应先修测试 / 覆盖率、绿了再跑突变，并在待突变文件过多时自动拆段顺序执行。

## 提案

只在桌面端（`packages/desktop/src-tauri`）落地「拆步 + Rust 突变 + 分段」，CLI（`plugins/dev-team`）零改动：

1. **拆步**：test-execution 相位步序列 `TEST_EXECUTION_STEPS` 改为 `PhaseStart → TestExecution → MutationTest(新) → PhaseLog`——walker 顺序执行天然保证「测试 / 覆盖率修绿后才跑突变」。步词汇封闭集 18→19：`StepNode::MutationTest` → `ChangeStepKind::MutationTest` → `RunStepKind::MutationTest`（线词 `mutation_test`）→ `bindings.ts` → 前端流程视图（白名单式扩展，涟漪面固定）。port 三件套：`ToolCommand::MutationTest { change_id }` / `ToolStepOutput::MutationTest(载荷)` / `MutationRunner` port（照 `TestExecutionRunner` 同式；spawn 不进 core，实现落 `infra/checks/src/mutation/`）。
2. **突变范围 = diff 确认**：worktree 内 `git diff HEAD` 恰为本 change 编辑集（`desktop-change-worktree` 既有保证）。vcs 边界单点封装 `change_edit_set(root, base) -> Vec<String>`（`git diff <base> --name-only -z` + `git ls-files --others --exclude-standard -z` 并入 untracked，排序去重 POSIX 归一）——**突变范围与文件清单的唯一入口**；`MutationRunner::run(root, name, files)` 收清单入参（清单获取经 vcs 缝注入，缝形态 design 定稿；checks-runtime 依赖面不扩 vcs）。
3. **Rust 突变 = cargo-mutants**：`--in-diff`（行级精度，untracked 面经 `git add -N` 或 `--file` 补充，design 定稿）、`--json`、per-mutant `--timeout`、`--jobs = available_parallelism()/2`（突变并发 = 一半 CPU，延续 CLI 现状口径）、`--baseline skip`（TestExecution 绿跑保证基线）。映射表定案：`caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`；score 沿既有公式 `(killed+timeout)/(total-ignored-compileError-runtimeError)`（分母排除 ignored + compileError + runtimeError）。js 族（jest / vitest / vite-plus）沿 Stryker（无 in-diff，按 diff 文件清单喂 `mutate`）；无突变执行器的框架（node-test）该 suite 突变跳过并记 evidence（MUST NOT Err，与 CLI 注册表口径一致）。
4. **自动拆段**：估算突变体量超段预算（目标 5 分钟/段 ≈ 250~300 mutants @ 半 CPU 1~2 s/mutant；软护栏 10 分钟，MUST NOT 硬失败语义；单文件超预算接受）→ 字典序均分 k 段顺序执行；首段静态估算 + 后续段按上一段实测速率自适应修正；段级原始报告 `mutation-seg-<i>.json` 全程留存，计数归并沿 `aggregate_mutation` 既有公式（段当子报告，MUST NOT 新增归并公式）。**分段多轮 MUST 复用 sandbox**：Stryker 各段共用同一 `tempDirPath`、cargo-mutants 共享 `CARGO_TARGET_DIR` 增量编译；清理时点移到全段完成后一次（只清 `_stryker-tmp` 与临时 config，报告工件不触碰）。
5. **双时点反馈边②**（与反馈边①口径差异显式并存，见下）：突变执行**中断**（段执行 error / harness 崩 / 超时）→ 立即反馈 executor 修复会话（不等汇总）；全部段正常跑完 → 汇总得分，仅**未达标**（score < threshold）才反馈；达标 → pass 直接收口（零反馈）。反馈内容只含结论摘要（score / threshold / 中断错误摘要）+ **原始报告路径清单**，MUST NOT 内联存活突变体清单——明细交 executor 自行读报告。独立计数（label `"mutation-test"`，与 static-check / test-execution 反馈计数器分立、上限 5），超限升格相位 fail。
6. **null 语义观察期不收紧**：`mutation: null` 保持合法，机械 checklist 第 3 条与 evaluator T3 不改；第 3 条 evidence 改为区分「跑过且达标 / diff 空跳过 / null」三态。PhaseLog 产物面挂 `test_outcome` + `mutation_outcome` 双槽。
7. **段级复用门断点续跑**：段报告在位 + 输入 mtime 未变 → 复用该段；中断 / 重试只重跑未完段。
8. **config schema 不扩展**：jobs / per-mutant timeout / 拆段预算 / 分段目标均写常量观察，不进 `suite.mutation` 字段。

> **⚠ 反馈边口径并存（防误统一）**：反馈边①（test-execution，`desktop-test-execution`）原则是「修复会话输入面自足、MUST NOT 依赖修复会话自读报告文件」（findings_detail 有界内联）；反馈边②（MutationTest）**反其道**：结论 + 报告路径指引，executor 自读报告。**两种口径并存系用户明确决策**，MUST NOT 被后来者「统一」掉。

## 关键决策

explore 五轮逐轮定稿，每项列明理由与被拒绝的备选方案：

| # | 决策 | 理由 | 被拒绝的备选方案 |
|---|------|------|------------------|
| D1 | 只处理桌面端，CLI 零改动 | 桌面 gate 已是出线点；CLI 侧拆步 / 清单反推逻辑保留原样 | 双侧同改：涟漪面翻倍，CLI 已冻结 |
| D2 | 突变范围 = diff 确认（`change_edit_set` 单点封装） | worktree 语义下 `git diff HEAD` 恰为编辑集（既有 spec 保证）；整体封装成一个功能，不散落多处取 diff | change 清单反推源文件 + net-zero 去噪（CLI 先例）：清单维护成本高、噪音大 |
| D3 | untracked 并入 `change_edit_set`（`git ls-files --others --exclude-standard`） | 新建源文件没 add 不进 diff；CLI `expandMutationDiffWithInferredSources` 有「brand-new 文件保留」先例 | 不并入：新建源文件漏突变 |
| D4 | unviable→compileError、not-mutated→ignored | 分母排除口径与 Stryker compileError 同语义；`MutationMeasured` 字段现成 | unviable→ignored：分母口径漂移、分数不可比 |
| D5 | 拆段目标 5 分钟/段、软护栏 10 分钟、单文件超预算接受 | 预算 10 分钟的一半切段留浮动（历史实测每文件耗时 4 倍差，静态切段不可靠）；段是软目标非硬失败 | 按文件数静态切段：速率波动 4 倍差，不可靠；硬失败语义：误伤长文件 |
| D6 | 分段多轮 MUST 复用 sandbox（共享 tempDirPath / CARGO_TARGET_DIR；全段完成后一次清理） | sandbox 建置 / 全量编译只发生一次，段间只换 mutate 面 | 每段独立 sandbox：初始化成本随段数线性放大 |
| D7 | 反馈边②双时点 + 报告指引式（MUST NOT 内联存活清单） | 中断即反馈不等汇总；明细交 executor 读原始报告（报告全程留存），反馈 prompt 有界 | 照反馈边①内联存活突变体清单：大 change 下清单爆炸，且与用户决策相悖 |
| D8 | null 语义观察期不收紧（checklist 第 3 条 / evaluator T3 不改） | 先观察桌面突变落地后的实际通过率再决定是否升格 error | 立即收紧 null 语义：存量 / 过渡期 change 面临误伤 |
| D9 | config schema 不扩展（jobs / timeout / 预算写常量观察） | 参数未到需要用户调的程度；避免 schema 面发散 | 进 `suite.mutation` 字段：schema 扩展 + 双侧 schema 对拍面扩大 |
| D10 | 「零计划绿过」旁支不治 | 与本变更主题正交，独立 change 再议 | 顺手修：范围蔓延 |

## 能力

### 新增能力

- **desktop-mutation-testing**（新）— 桌面突变测试执行域：突变范围清单封装（`change_edit_set` 唯一入口）、框架矩阵（Stryker / cargo-mutants）与 outcome 映射表、score 公式与逐 suite 阈值判定、分段顺序执行与自适应、sandbox 跨段复用、双时点反馈边②（报告指引式口径）、段级复用门断点续跑、突变常量参数（不进 config schema）。

### 修改的能力

- **desktop-test-execution** — 「执行链与报告落盘」拆步语义改写（mutation 报告位不再恒 null，突变面由独立 MutationTest 步承载）；「步骤可观测与最小载荷」增 `ChangeStepKind::MutationTest` 步词汇与 `ToolStepOutput::MutationTest` 载荷（零 Serialize 不进 IPC）；「V1 范围与边界留痕」第 1 条「mutation 不移植」撤销改写、第 3 条 net-zero 口径改写；新增「MutationTest 确定性门禁步与时序」「机械 checklist 双槽与 evidence 三态」两 requirement。
- **desktop-change-orchestration** — 新增「MutationTest 步接线与独立反馈预算」：walker 拆步时序（TestExecution 绿后才进 MutationTest）、反馈边②独立计数（`feedback_limit(step, "mutation-test")`）、port 三件套接入。
- **desktop-workflow-write-face** — 「相位定义有序 step 序列与 artifact 清单」`StepNode` 词汇 18→19（增 `mutation_test`）；「test-only 相位表与前置依赖」`TEST_EXECUTION_STEPS` 序列改写（`[PhaseStart, TestExecution, MutationTest, PhaseLog]`）。
- **desktop-checks-domain** — 新增「mutation 执行面与 MutationRunner port 归属」：`infra/checks/src/mutation/` 执行模块、纯层判定落 `core/checks`、port 归 `orchestration::port`、checks-runtime 依赖面不扩 vcs。
- **desktop-change-state-store** — `RunNodeRecord.step_kind` / `StepRecord.step_kind` 封闭集扩 `mutation_test`（十八值 → 十九值）。
- **desktop-change-queries** — run 执行图读面 `stepKind` 十九值全词汇（含 `mutation_test`）。
- **desktop-change-flow-view** — 常驻渲染 `runs[].nodes` 十九值全词汇；前端步词汇归一表（`normalizeKind` / `stepWord`）扩 `mutation_test` ↔ `mutationTest`，`mutationTest` 归 exec 组。
- **desktop-corpus-regression** — 节点全词汇样本十八类 → 十九类（含 `mutation_test`）。
- **desktop-crate-layout** — checks 边界成员注记扩 mutation-test；vcs 边界职责扩变更文件清单（`change_edit_set`）。

### 引用沿用（零 delta）

- **desktop-change-worktree** — 「diff 面恰为本 change 编辑集」是突变范围的权威来源，本变更零语义改动（清单封装落 vcs，不碰 worktree 语义）。
- **desktop-ipc-type-bindings** — `ChangeStepKind` 新变体经 tauri-specta 再生与一致性守卫既有纪律承载，机制零改动。
- **mutation-testing** / **unit-test-executor** / **test-detect-frameworks** 等 CLI 系 — 零改动（D1）；其 `mutation-testing` 能力的 Stryker / 计数公式语义为桌面映射表的对齐参照。
- **config-schema** / **desktop-workspace-config** — `suite.mutation.score` 既有字段（desktop 80 / src-tauri 70）原样消费，schema 不扩展（D9）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` — `StepNode::MutationTest` + `RunStepKind::MutationTest`（18→19 封闭集，线词 `mutation_test`；`group()` 归 Exec）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` — `TEST_EXECUTION_STEPS` 改 `[PhaseStart, TestExecution, MutationTest, PhaseLog]`（MutationTest `on_fail=FeedbackLoop { limit }`）
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` — `ChangeStepKind::MutationTest`
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` — `ToolCommand::MutationTest { change_id }` / `ToolStepOutput::MutationTest(载荷)` / `MutationRunner` port（与 `TestExecutionRunner` 并列）；载荷 = conclusion + score / threshold + 计数 + 报告路径清单 + 反馈文本（零 Serialize）
- `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` — id → 记录 → name 解析接线（与 test-execution 同式）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `mutation_test_loop`（双时点反馈、`feedback_limit(step, "mutation-test")` 独立计数、超限代写 fail checklist 升格相位 fail）；机械 checklist 双槽（`test_outcome` + `mutation_outcome`）
- `packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs` — `aggregate_mutation` 由「生产恒 None」转为消费段子报告（计数求和公式不变）
- `packages/desktop/src-tauri/crates/core/checks/src/model.rs` — 段级报告 / MutationBlock 归并的纯层模型（`MutationMeasured` 既有字段零扩展）
- `packages/desktop/src-tauri/crates/infra/checks/src/mutation/`（新模块）— 分段调度（静态初值 + 自适应）、Stryker / cargo-mutants 模板展开与 spawn、映射、段级与归并报告写盘、sandbox 复用与全段后一次清理
- `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` — `change_edit_set(root, base)`（diff `--name-only -z` + `ls-files --others --exclude-standard -z`，排序去重 POSIX 归一）
- `packages/desktop/src-tauri/src/types/generated/bindings.ts` — 随 `ChangeStepKind` 新变体再生
- `packages/desktop/src/views/changes/flow/run-state.ts` — `normalizeKind` / `stepWord` 扩 `mutation_test` ↔ `mutationTest`；`mutationTest` 归 exec 组（`GROUP_LABEL` 随动）
- `packages/desktop/package.json` — `version` 0.5.1 → 0.5.2（归档时执行）

### 测试文件

- `packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs`（或既有词汇测试）— 19 值全映射（`StepNode` ↔ `RunStepKind` 逐词同源）、`mutation_test` 线词
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs` — `TEST_EXECUTION_STEPS` 四步序列、MutationTest `on_fail` 形态
- `packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs` — 三件套载荷构造与零 Serialize 边界
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` — 拆步时序（TestExecution 绿后才进 MutationTest）、双时点反馈（中断立即反馈 / 未达标报告指引 / 达标零反馈）、独立预算 `feedback_limit_of("test-execution", StepNode::MutationTest)` == 5 且与 static-check / test-execution 分立、超限升格、机械 checklist 双槽与 evidence 三态
- `packages/desktop/src-tauri/crates/core/checks/src/aggregate_test.rs` — 段子报告归并（计数求和）、逐 suite 阈值判定、score 公式
- `packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs` — mutation-block / 段级报告 fixtures 对拍（含映射表样本）
- `packages/desktop/src-tauri/crates/infra/checks/src/mutation/*_test.rs` — 映射表（含 unviable / not-mutated 分支）、分段切分与自适应、sandbox 复用与清理时点（报告工件不触碰）、跳过面 evidence
- `packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs` — `change_edit_set`（tracked + untracked 并入、排序去重、POSIX 归一）
- `packages/desktop/src/views/changes/flow/run-state.test.ts` — 归一表扩 `mutation_test`、exec 组归类
- golden / 语料 — 十九值 nodes 投影走 `DESKTOP_GOLDEN_REWRITE=1` 显式重写

### 不要修改

- `plugins/dev-team` 全部（TS CLI、三类交付产物、`test-framework.ts` 注册表、`mutation-parser`、`stryker-config.ts`；版本 2.10.x 保持）
- `openspec/config.json` 的 `suite.mutation` 字段与 config schema（D9：jobs / timeout / 预算 / 段目标写常量）
- 机械 checklist 第 3 条「mutation null 自动通过」判定语义与 evaluator T3（D8：观察期保留；仅 evidence 文案三态化）
- `desktop-change-worktree` 语义（基线恒 HEAD、编辑囚于 worktree、diff 面 = 编辑集）
- 反馈边①（test-execution `findings_detail` 有界内联）口径——与反馈边②并存，MUST NOT 统一（见提案警示）
- `MutationBlock` / `MutationMeasured` schema（双侧对齐既有，corpus `mutation-block` fixture 口径）
- 「零执行绿过」旁支（D10：不治）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 拆步相位表 | `TEST_EXECUTION_STEPS` = `[PhaseStart, TestExecution, MutationTest, PhaseLog]`；TestExecution fail 走既有反馈边修复重入，绿后才执行 MutationTest（walker 顺序保证）；requirement 与 test-only 两表同式 |
| AC-2 | 步词汇 19 值全映射 | `StepNode` / `ChangeStepKind` / `RunStepKind` 含 `MutationTest` / `mutation_test`（1:1 全映射）；bindings 再生后前端流程视图现有步骤渲染直接吃（零渲染改动），归一表 `mutation_test` ↔ `mutationTest` 在案且归 exec 组 |
| AC-3 | change_edit_set 唯一入口 | `change_edit_set(root, HEAD)` 返回跟踪改动 + untracked 并入、排序去重、POSIX 归一的清单；全仓无第二处取 diff / 列清单触点；`MutationRunner::run(root, name, files)` 收清单入参，checks-runtime 依赖面不扩 vcs |
| AC-4 | Rust 突变与映射表 | cargo-mutants 跑 rust suite（`--in-diff` / `--jobs=half CPU` / per-mutant `--timeout` / `--baseline skip`）；映射 `caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`；score = `(killed+timeout)/(total-ignored-compileError-runtimeError)`；js 族走 Stryker（`mutate` = diff 文件清单）；node-test 等无执行器框架突变跳过记 evidence 不 Err |
| AC-5 | 分段与 sandbox 复用 | 估算超段预算（5 分钟目标）自动分段顺序执行、段报告 `mutation-seg-<i>.json` 留存、计数归并沿 `aggregate_mutation`；Stryker 各段共用同一 `tempDirPath`、cargo-mutants 共享 `CARGO_TARGET_DIR`；清理仅 `_stryker-tmp` 与临时 config 且在全段完成后一次；单文件超预算不失败 |
| AC-6 | 双时点反馈边② | 段中断立即反馈（错误摘要 + 报告路径清单，不等汇总）；全部段完成汇总 score，未达标反馈（score / threshold + 报告路径清单）；达标 pass 零反馈；反馈 prompt 零存活突变体内联；独立计数上限 5 且与 static-check / test-execution 分立，超限升格相位 fail |
| AC-7 | null 观察期与双槽 | 未跑 / diff 空跳过时 `mutation: null` 合法（checklist 第 3 条 / evaluator T3 不改）；PhaseLog 产物面挂 `test_outcome` + `mutation_outcome` 双槽；第 3 条 evidence 区分「跑过且达标 / diff 空跳过 / null」三态 |
| AC-8 | 段级复用门 | 段报告在位 + 输入 mtime 未变复用该段（零 spawn）；中断重试只重跑未完段；输入变化段失效重跑 |
| AC-9 | 语料与守卫 | corpus 含映射表样本与段级报告 fixtures、对拍解析与聚合结论；十九值 nodes 投影 golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写；bindings 一致性守卫绿 |
| AC-10 | 全管线通过 | `pnpm -C packages/desktop run client:check` 与 `pnpm -C packages/desktop run test` 全绿；`cargo test --workspace`（src-tauri 根）全绿；`plugins/dev-team` 零 diff |
| AC-11 | desktop 版本升级（归档时执行） | `packages/desktop/package.json` `version` 0.5.1 → 0.5.2；`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 版本不随动 |

## 风险

1. **cargo-mutants 生态依赖与行为漂移**：外部工具版本演进可能改 outcome 语义（unviable 判定 / JSON 输出形态）；映射表以 corpus fixtures 钉住口径，工具升级需重验。工具不可达时处置（显式 Err 停给用户 vs 跳过留痕）design 定稿，倾向沿「程序解析前置检查」既有经验显式 Err。
2. **速率估算波动**：历史实测每文件耗时 4 倍差（5~19 s），自适应修正仍可能段溢出到护栏（10 分钟）——软目标语义已定案（单文件超预算接受），极端长尾段耗时不可完全消除。
3. **untracked 面进 `--in-diff` 补丁的口径**：`git diff HEAD` 不含 untracked 内容；`git add -N`（intent-to-add）与 `--file` 补充两法各有副作用（index 触碰 vs 双轨执行），design 定稿；清单层面 untracked 并入已定案（D3）不受影响。
4. **两反馈边口径并存被误统一**：反馈边①「输入面自足」与反馈边②「报告指引式」相反，spec delta 已显式留痕警示；后续变更若「顺手统一」会翻案用户决策，需在 review 拦截。
5. **步词汇涟漪面 golden 重写**：19 值全映射触 bindings / 前端归一表 / corpus golden，重写面较大——均走 `DESKTOP_GOLDEN_REWRITE=1` 显式流程，禁止静默重写。
6. **null 观察期结论不一致仍存在**：桌面 mutation 落地后与 CLI 同 change 的分数口径差异（override 组不产 / 逐 suite 阈值 vs CLI 合并口径）在过渡期仍可能不一致；多 suite 多阈值（desktop 80 / src-tauri 70）时归并块阈值取首块（沿 `aggregate_mutation` 既有语义），**门禁结论以逐 suite 全过为准**——两者在单 suite 场景一致，多阈值并存时报告位与结论面的口径差留痕观察。
7. **长任务活性**：突变步 10+ 分钟量级，walker 停止 / 桌面重启场景下段级断点续跑依赖段报告在位；run 中途崩溃丢节点 / 边史的既定权衡（desktop-change-state-store）不变。
8. **「零计划绿过」旁支不治**（D10）：`plans: []` 空执行仍 pass 的面保留，后续独立 change 处理。
