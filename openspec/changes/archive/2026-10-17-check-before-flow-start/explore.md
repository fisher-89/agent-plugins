# 桌面端变更流程启动前的基线检查门（static-check + test-execution，不含突变）

workflow_type: requirement

## 主题与背景

用户需求：桌面端变更流程（change flow run）启动前，先在基线上执行 static-check 与
test-execution（明确不含 mutation），任一不通过则停止流程，并提示用户在基线上修复后
重新发起。动机推断：若主仓基线本身是红的（lint 挂 / 测试挂），worktree 建域自 HEAD，
首个 run 的 implement 相位 static-check 步会把基线的锅甩给 executor 会话修——烧光
FeedbackLoop 预算（5 次）还可能修出与变更无关的改动。基线绿门把"基线的债"与"本次
变更的债"在发起前分家。

## 关键事实（代码落点）

- 入口：`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` 的
  `change_flow_start_with`。已有 4 道前置校验（① change 已建档 ② workflow_type 命中
  相位表 ③ exec root（worktree）目录存在 ④ 无并行 run）——全部毫秒级元数据校验，
  Err 直接经 invoke reject 返回前端 `error` 态。
- 执行缝：`ToolStepPort`（`orchestration::steps::LocalToolSteps`）内组合
  `StaticCheckRunner` / `TestExecutionRunner` / `MutationRunner` 三缝，infra 实现
  `ProcessStaticCheck::run(root, name)` / `ProcessTestExecution::run(root, change)`。
- static-check（`infra/checks/src/static_check.rs`）：读目标仓
  `<root>/openspec/config.json` 的 `static_analysis` 命令；无配置/空命令 = passed
  直接过；程序不可达显式 Err；每次 spawn（无复用门）；输出 record 落 change 目录。
- test-execution（`infra/checks/src/testexec/mod.rs`）：detect plans → **复用门**
  （既有 summary 在场 + 完整性 + 输入 mtime 未变 → 零 spawn 复用）→ 逐 suite spawn →
  聚合诊断；报告落 `<root>/openspec/changes/<name>/reports/test`。
- 流程内既有门禁：implement / test-gen 相位有 StaticCheck 步（FeedbackLoop ≤5，
  失败诊断注入 executor 会话修复）；test-execution 相位 = TestExecution 步 +
  MutationTest 步（mutation 用 `--baseline skip`，贵，故基线检查排除它合理）。
- walker（`orchestration/src/walker.rs`）：计划图驱动，相位表编译期静态单源
  （`workflow::write::phase_table`，requirement 9 相位 / test-only 6 相位）；
  run 级机制已有先例（`stop_before`、`auto_next_phase`）；`Terminal{status, reason}`
  是"停止并提示"的统一载体；`emit_node` 图节点前端已完整渲染
  （run-step-node 有 staticCheck / testExecution 节点 kind + outputRef stdout 面板）。
- worktree 基线：`create_change` 时 `git worktree add -b change/<name>` 自主仓 HEAD
  建域（基线恒 HEAD，脏仓不进 worktree）；run 的 exec_root = worktree。
- 结论三态：test-execution 有 Pass / Fail / **Error**（框架探测失败 / spawn 失败等
  环境问题）——基线检查对两者的处理与文案应区分（现有步内 error 与 fail 同走反馈边）。

## 现状时序 vs 目标时序

```
现状：
 create_change ──> worktree(HEAD 基线) ──> change_flow_start ──> drive 相位循环
                                            │ (仅元数据校验)
                                            └─ implement 相位 StaticCheck 步
                                               基线红 ⇒ 反馈边甩锅 executor
                                               （烧 5 次预算 / 修出无关改动）

目标：
 change_flow_start ──> [新] 基线检查门 ──绿──> drive 相位循环
                          │ static-check
                          │ test-execution（不含 mutation）
                          └─红──> 停止 + "请在基线上修复后重启" + 报告路径
```

## 设计张力与开放问题

### A. 挂载点三案

- **案 1｜start 第 5 道前置校验（同步）**：实现最薄（change_flow_start_with 里
  await 两检查，Err 返回）。代价：全量测试分钟级，invoke 悬挂无进度；失败零 run
  记录、零图节点，仅 error 字符串；`begin_run` 前并发语义要重排。
- **案 2｜run 内 drive 前的基线段（walker run 级 gate）**：begin_run 后、drive 前
  跑 baseline（emit staticCheck / testExecution 两个图节点），失败
  `Terminal::failed("基线未通过，请在基线修复后重新发起")`。相位表零改动（借用
  stop_before 的 run 级先例）；前端零改动（节点 / outputRef / 终态 reason 全复用）；
  run 历史留痕可回查。代价：walker 加一段 run 级序曲，节点身份键（phase, attempt,
  step）需归位（如 phase="baseline" 或挂首相位 attempt 前）。
- **案 3｜独立相位进相位表**：动相位机单源（phase_table_test 全表对照、phase_next
  路由、stale 传播、backtrack 白名单连坐），且"基线检查"无 attempt/verdict 相位
  语义——代价与语义都不匹配，倾向排除。

### B. "基线"语义（再发起场景）

worktree 首建 = HEAD 纯基线；但 failed/stopped run 后**再** start，worktree 已有
残留改动——此时检查失败未必是基线的债。选项：a) 每次 start 都跑（冒烟语义，简单）；
b) 仅 worktree 干净（`vcs` status 空）时跑（真"基线"语义，需 vcs 探测）。需用户定夺。

### C. 报告落点与复用红利

复用现有 runner ⇒ 报告落 `<change>/reports/...`（name 派生）。红利：基线绿 summary
可被 run 内 test-execution 相位步的 mtime 复用门直接复用（输入未变零 spawn，省一次
全量测试；implement 改码后 mtime 推进自动失效，语义正确）。代价：基线报告与 run 内
报告同目录混居，靠 exec_id / 时间戳区分。static-check 无复用门，run 内步会再 spawn
一次（秒级，可接受）。

### D. Error vs Fail

基线检查中 test-execution conclusion=Error（环境/配置问题）与 Fail（测试红）都应
停，但文案应区分：Error 引导查配置/环境，Fail 引导修基线测试。

### E. 前端面

案 2 前端零改动（RunNotice → 统一视图重查 → 图节点 + 终态 reason）。案 1 需确认
error 态呈现位置足够醒目（use-change-flow-run 的 error 已有渲染先例）。

## 测试先例

`src/commands/change_flow/mod_test.rs` 已有真件 spawn 先例：`write_static_shim`
（平台 shim 脚本）+ `openspec/config.json` 登记 static_analysis + `prefix_path`
PATH 注入——基线检查的测试可直接沿用；walker 侧有 walker_test 的 Terminal /
emit 断言先例。

## 待用户决策

1. 挂载点选案 1（同步 Err）还是案 2（run 内基线段，图可视化）？
2. 再发起（worktree 有残留）时每次都查，还是仅 worktree 干净时查？
3. 基线检查失败的具体提示文案措辞（含报告路径指引）。

---

## 第二轮：用户决策与落地勘测（追加）

### 已定决策

1. **案 2 + 表头 baseline 相位**：检查以相位形态呈现（可见、有图节点）；在
   worktree（exec_root）内执行——主仓未提交改动不进 worktree，基线语义纯净。
2. **再发起先 clean**：两步检查前加 clean 步，恢复 worktree 至基线态。
3. **文案区分 Error / Fail** 两种失败引导。
4. **报告分离**：基线检查报告与 run 内报告分目录，不做复用红利。

### 落地勘测（新事实）

- **phase_next 表驱动天然支持表头插入**：路由 = 表内首个未 pass 相位
  （`phase_next.rs:176`）；`all_passed` / 归档完成度 `completion_of` 自动含
  baseline；`allowed_backtrack_phases` 中 baseline 处 `idx==0` 自身无回溯白名单，
  但后续相位的白名单**会包含 baseline**（回溯到 baseline = 重跑基线检查，语义待定）。
- **`feedback_limit` 拒绝工具步 Terminal 声明**（walker.rs:1331，视为"计划图漂移"）
  ——baseline 相位 StaticCheck / TestExecution 步要"失败即停"，需给 walker 工具步
  执行体加 `OnFail::Terminal` 分支（现仅 FeedbackLoop 分支；重试基线无意义且烧
  round 预算，Terminal 语义正确）。
- **依赖引导在 create 时跑**（`create.rs:319` bootstrap，lockfile 首匹配 →
  `pnpm install --frozen-lockfile` 等，900s）——worktree 的 node_modules/target 是
  **未跟踪目录**：clean 裸 `git clean -fd` 会删掉依赖 → 基线测试直接 Error。
- **clean 排除面现成单源候选**：`detect.rs:296 PRUNED_DIR_NAMES`
  （node_modules/dist/build/target/coverage/.nyc_output）+ dot 目录剪枝——语义恰好
  是"依赖与工具产物树"，clean 排除面可复用此名单 + `openspec/`（工件 + 报告）。
- **ArchiveVcsPort 无 clean/reset 方法**：需新增 vcs 缝方法（如
  `restore_baseline(worktree)`），实现驻 infra/vcs（core 零 spawn 红线）。
- **ChangeStepKind 加枚举连坐面**：枚举 + `as_str` + store `RunStepKind` +
  bindings.ts 重新导出 + 前端 `run-step-node.tsx STEP_LABEL`（穷尽 Record——漏补
  TS 编译即报错，保障性好）+ `run-state.ts` snake/camel 映射。
- **报告分离改动面**：`record_exec_output` 与 test-execution 报告同锚
  （`<change>/reports/test/exec-outputs/`，layout 单点派生）；分离 = layout 加
  baseline 报告目录变体（如 `reports/baseline`）+ `ProcessTestExecution::run`
  报告槽位参数化 + exec_id 回查面（前端 stdout-panel 按
  `{root, changeName, execId}` 取数）对齐新落点。
- **前端结构零改动**：图节点渲染 / outputRef stdout 面板 / 终态 reason 全现成；
  baseline 相位名与 clean 步标签是文案补充。

### 目标形态（决策后的时序）

```
change_flow_start ──> walk_run ──> drive 相位循环
                                     │
                                     ├─ [表头] baseline 相位
                                     │    PhaseStart → Clean → StaticCheck
                                     │      → TestExecution → PhaseLog
                                     │    （执行目录 = worktree/exec_root；
                                     │      失败 Terminal 停止 + 文案区分
                                     │      Error（环境）/ Fail（测试红））
                                     ├─ proposal → ... → acceptance
                                     └─ [stop-before] archive
```

### 新冒出的关键决策点

#### F. baseline 相位 pass 的持久性（最关键）

PhaseRecord 的 pass **跨 run 持久**：baseline 进相位表 → 第一次 run pass 后，
后续再发起**不再重跑基线检查**。与原始诉求"变更流程启动前检查"的"每次启动"语义
有出入。两形态：

| 形态 | baseline pass 后 | 存量 change 兼容 | 实现面 |
|---|---|---|---|
| 进相位表 | 永久跳过（一次性门） | 无 baseline 条目 → 再发起先走 baseline + clean（见 G 破坏面） | 相位表 + walker 工具步 Terminal 分支 |
| run 级序曲（相位形态呈现、不落 PhaseRecord） | 每 run 必查 | 序曲对存量同样跑，无兼容问题 | walker drive 序曲，相位表零改动 |

#### G. clean 与"续走"语义的冲突（必须设计排除面）

再发起默认是**续走**（phase_next 按 PhaseRecord 跳过已 pass 相位），但已 pass 相位
的产物在 worktree 里是未提交面：

- `openspec/changes/<name>/`（proposal.md 等 + 报告树）= 未跟踪 → clean 删了则已
  pass 相位产物缺失（相位不重跑，上下文断粮）；
- implement 已 pass 的代码 = 未提交 tracked 修改 → reset 掉则后续相位以为已实现。

⇒ clean 排除面至少需：`openspec/` + `PRUNED_DIR_NAMES` 树；tracked 修改的 reset 面
与续走前提的冲突仍需取舍（接受重置？还是 clean 仅限"上一 run 失败残留"场景？）。

#### H. 回溯白名单是否含 baseline

后续相位决策 backtrack 到 baseline = 重跑基线检查——允许（语义自洽：基线变了重来）
还是从白名单排除（baseline 不可回溯）？

---

## 第三轮：initialize 决策勘测（追加）

### 已定决策（累计）

1. 案 2：run 内执行，worktree（exec_root）内检查。
2. 表头一次性相位，**命名 `initialize`**；再发起先 clean 恢复基线。
3. initialize 相位自身不产生代码改动（clean + 只读检查）。
4. **不接受被当作回溯目标**（backtrack 白名单排除 initialize）。
5. 文案区分 Error（环境）/ Fail（测试红）；报告分离（`reports/baseline`），不做复用。

### 勘测结论（落地路径）

- **一次性语义天然成立**：Terminal 失败不落 PhaseRecord（仅 phase_log 落账）→
  initialize 失败 = run failed + 零条目 → 再发起 phase_next 仍路由 initialize +
  clean 清残留 → 干净重试。pass 后条目持久 → 后续 run 永久跳过。无需额外机制。
- **回溯排除需加表属性**：`allowed_backtrack_phases`（phase_table.rs:719）是纯
  位置推导 `table[..=idx]`，无 per-phase 位——proposal 的白名单必含表头 initialize。
  排除机制选项：给 `PhaseDefinition` 加显式属性（如 `backtrackable: bool`，沿
  `prerequisites` 表内自描述先例，**推荐**）或改推导规则跳过 idx==0（隐式脆，表
  结构变化即漂移）。stale 传播走 `prerequisites`（`dependents`），initialize 无
  依赖入边不受波及，prerequisites 零改动。
- **clean 步走 vcs 直调先例**：归档 git 步（step_commit 等）walker 直调
  `ctx.vcs.*` 不经 ToolCommand 枚举——clean 同式（`ArchiveVcsPort` 新增
  restore/clean 方法，实现驻 infra/vcs）。`StepNode::Clean` 新变体归 Git 组
  （`group()`/`role()` 穷尽 match 编译期强制补）。
- **clean 安全红线（legacy 零 worktree）**：start 前置校验 3 中
  `record.worktree = None` → exec_root = **主仓 root**——clean 在主仓执行会
  reset 用户未提交改动（灾难）。clean 步 MUST 仅在 worktree 在场时执行；legacy
  记录以 `passed` + 跳过因短路（step_commit 的 legacy 短路先例，walker.rs:1931）。
  baseline 检查本身对 legacy（主仓执行）语义存疑——建议 initialize 整相位对
  legacy 记录短路跳过（passed + 跳过因，归档步同式）或至少 clean 跳过。
- **clean 排除面依赖**：PRUNED_DIR_NAMES（node_modules/dist/build/target/
  coverage/.nyc_output + dot）驻 core/checks/src/testexec/detect.rs 且为私有；
  orchestration 不依赖 checks crate。选项：a) orchestration 增依赖 checks 并
  公开名单；b) 名单上移 foundation（更基础层，**推荐**——两消费点共享单源）；
  c) vcs 缝自带默认。排除面还需加 `openspec/`（工件 + 报告树，未跟踪）。

### 遗留决策点

#### I. 存量 change 兼容（新冒出，需用户定夺）

功能上线前创建的 active change（无 initialize 条目、已跑过若干相位）再发起 →
phase_next 首个未 pass = initialize → **clean 会 reset 已 pass 相位的未提交代码**
（implement 代码丢失但 PhaseRecord 显示 pass → 语义断裂）。工件可经排除面保留，
tracked 代码改动无法保留。选项：

- **a) legacy 判定跳过**（推荐）：存在其他 PhaseRecord 条目而无 initialize 条目
  → 判 legacy，路由跳过 initialize（新 change 首走必经表头，该判据可靠）。
- b) 接受重置：存量 change 续走前被重置，需 backtrack 重做受影响相位。
- c) 迁移补假 pass 条目：语义谎言，不推荐。

#### J. initialize 后的相位间停等（小点）

`auto_next_phase=false`（手动档）时每相位 pass 后停等确认（walker.rs:315）——
initialize 也一样停等。发起 run 本身即是意图确认，initialize 后再停一次偏冗余；
但保持全相位一致实现最薄。倾向：豁免（发起即确认）还是一致（停等）？

#### K. initialize PhaseLog 形态（小点）

空 checklist（archive 同式，`derive_verdict([]) = Pass`）或记一条基线结论
（static-check 通过 + 测试计数留痕，test-execution 机械 checklist 同式倾向）。
倾向后者（留痕有回查价值）。

---

## 第四轮：J/K 决议与设计定稿（追加）

### 决议

- **J（initialize 后停等）：保持一致**——手动档下 initialize 收口后照常停等
  确认（与全相位一致，实现最薄零特判）。
- **K（PhaseLog 形态）：留痕**——机械 checklist **零 agent** 证据：
  `test_execution_phase_log_input`（walker.rs:1494）纯桌面代写，从
  `TestExecutionOutcome` 载荷直接组装 `PhaseLogInput`，三会话槽位恒 `None`。
  且 `step_phase_log` 三分流按「产物在否」天然分离（`cursor.test_outcome`
  在前）——initialize 相位 steps 含 TestExecution 步则 pass 后 `test_outcome`
  自动挂载，PhaseLog 自动走机械 checklist 分支，落账面几乎零新代码（checklist
  文案可微调：static-check 通过 + 测试计数 + 报告目录，或复用现三条目）。

### 决策台账（全部落定，除 I）

| # | 决策点 | 结论 |
|---|---|---|
| A | 挂载点 | run 内（walk_run 相位循环），worktree 内执行 |
| B | 形态 | 表头一次性相位 `initialize`，pass 持久跳过 |
| C | 报告 | 分离（`reports/baseline`），不复用 |
| D | 失败文案 | Terminal 停止；Error（环境）/ Fail（测试红）分文案 |
| E | 前端 | 零结构改动（图节点 / stdout 面板 / 终态 reason 全现成） |
| F | pass 持久性 | 一次性相位（失败 Terminal 不落账 → 重试干净；pass 持久跳过） |
| G | clean 语义 | 两检查前先 clean 恢复基线；排除面 = PRUNED_DIR_NAMES + openspec/ |
| H | 回溯 | `PhaseDefinition` 加 `backtrackable` 属性排除（prerequisites 零波及） |
| I | **存量 change 兼容** | **未决**（建议：有相位条目而无 initialize 条目 → 判 legacy 跳过） |
| J | initialize 后停等 | 保持一致（照常停等） |
| K | PhaseLog | 机械 checklist 留痕（零 agent） |

### 改动面地图（proposal 原料）

```
core/workflow
  state.rs         StepNode::Clean / CleanOutcome? + ChangeStepKind::Clean
                   （group→Git；role→None；穷尽 match 编译期强制补）
  phase_table.rs   PhaseDefinition { backtrackable: bool }（表内自描述先例）
                   + INITIALIZE_STEPS / INITIALIZE 表项（两表表头各一）
                   + allowed_backtrack_phases 按 backtrackable 过滤
  phase_next.rs    （零改动——首个未 pass 相位天然含 initialize）
core/orchestration
  port.rs          ArchiveVcsPort::restore_baseline(worktree, 排除面) 新缝
  walker.rs        step_clean（vcs 直调 + legacy/无 worktree 短路先例）
                   + step_static_check / step_test_execution 的 Terminal 分支
                   （feedback_limit 现拒绝工具步 Terminal——改走专用分支）
  state.rs         RunStepKind::clean 线词连坐
infra/vcs          restore_baseline 实现（clean -fdx 排除面 + reset --hard）
                   （驻 infra，core 零 spawn 红线）
foundation         PRUNED 名单上移共享单源（checks / vcs 两消费点）
infra/checks       报告槽位参数化（baseline 变体）；detect 引 foundation 名单
前端               STEP_LABEL 补 clean（穷尽 Record）+ initialize 相位名文案
bindings          export-bindings 重导出（ChangeStepKind / PhaseId 面）
```

### 终态时序（定稿）

```
change_flow_start
  └─ walk_run
       └─ drive 相位循环
            ├─ initialize（一次性；worktree 内）
            │    PhaseStart → Clean → StaticCheck → TestExecution
            │      → PhaseLog（机械 checklist 零 agent）
            │    失败 → Terminal（Error/Fail 分文案）→ run failed
            │    pass 后停等确认（与全相位一致）
            ├─ proposal → dev-design → ... → acceptance
            └─ [stop-before-archive] archive
```

---

## 第五轮：I 决议，探索收口（追加）

### 决议

- **I（存量 change 兼容）：无需兼容**——当前仅作者本人使用，无存量数据包袱。
  phase_next 路由零特判：存量 change 再发起照走 initialize + clean（接受其
  残留被重置，语义断裂面不存在——用户知情且接受）。

### 决策台账（终版，全部落定）

| # | 决策点 | 结论 |
|---|---|---|
| A | 挂载点 | run 内（walk_run 相位循环），worktree（exec_root）内执行 |
| B | 形态 | 表头一次性相位 `initialize`；失败 Terminal 不落账可干净重试，pass 持久跳过 |
| C | 报告 | 分离（`reports/baseline`），不做复用 |
| D | 失败文案 | Terminal 停止；Error（环境）/ Fail（测试红）分文案引导 |
| E | 前端 | 零结构改动；补 clean 步标签 + initialize 相位名文案 |
| F | clean 语义 | 两检查前先 clean；排除面 = PRUNED_DIR_NAMES + dot 目录 + `openspec/`；legacy 零 worktree 记录短路跳过（主仓不重置） |
| G | 回溯 | `PhaseDefinition.backtrackable` 表属性排除 initialize（prerequisites / stale 零波及） |
| H | 停等 | initialize 收口后照常停等（与全相位一致，零特判） |
| I | 存量兼容 | 无需兼容（单用户，无存量包袱；路由零特判） |
| J | PhaseLog | 机械 checklist 留痕（零 agent；test_outcome 自动走三分支，近零新代码） |

### 探索结论

设计完整收敛：一次性 initialize 相位（Clean → StaticCheck → TestExecution →
机械 checklist 落账）挂 requirement / test-only 两表表头，全部机制有既有先例
（表头路由零改动 / Terminal 终态 / vcs 直调步 / legacy 短路 / 机械 checklist /
停等一致），无未决问题。**可进 proposal。**（见第四轮「改动面地图」为 proposal
的变更范围原料。）
