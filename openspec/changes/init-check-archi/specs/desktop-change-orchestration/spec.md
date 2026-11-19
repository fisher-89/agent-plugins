# desktop-change-orchestration Specification（delta: init-check-archi）

## MODIFIED Requirements

### Requirement: initialize 基线检查相位执行体

Walker SHALL 以两表表头一次性 `initialize` 相位（`INITIALIZE_STEPS`，见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）承接变更流程启动前的基线绿门：步序 `Clean → StaticCheck → ArchiCheck → TestExecution`（开相与相位收口不在 `steps` 内，由 `enter_phase` / `settle_phase` 承载），全部检查与恢复 SHALL 在 exec root（worktree）内执行（主仓未提交改动不进 worktree，基线语义纯净）。initialize 相位 SHALL 为确定性相位（零 executor / evaluator / decision 会话，全程零 WorkerAgent）：

- **Clean 步**（vcs 直调先例同归档 git 步，不经 ToolCommand 枚举）：walker 经 `ArchiveVcsPort::restore_baseline(worktree, 排除面)` 恢复 worktree 至基线态——tracked 修改 reset、untracked 清除，排除面 = foundation 共享的依赖与工具产物树名单（node_modules / dist / build / target / coverage / .nyc_output + dot 目录，见 desktop-checks-domain「依赖与工具产物树名单共享单源」）+ `openspec/` 工件树（proposal 等过程文档与报告树为未跟踪面，MUST NOT 被 clean 删除）。Clean 步 MUST 仅在 worktree 在场时执行；失败即 `Terminal`（`on_fail=None`）。
- **StaticCheck / TestExecution 步**：复用既有检查域 runner（`StaticCheckRunner` / `TestExecutionRunner`，报告槽位为 baseline 变体，见 desktop-test-execution「baseline 报告槽位与树隔离」），MUST NOT 执行 mutation（基线检查明确不含突变）。两步声明 `on_fail=Terminal`：失败 SHALL 以 `Terminal` 收敛 run failed，MUST NOT 走反馈边（无 agent 会话可注入，重试基线无意义）。失败文案 SHALL 区分：test-execution `conclusion=Error`（环境 / 配置问题——引导查配置与环境后重新发起）与 `conclusion=Fail`（测试红——引导在基线上修复测试后重新发起）；static-check 失败引导在基线上修复后重新发起；文案 SHALL 携 baseline 报告目录路径供回查。
- **ArchiCheck 步（本挂载点为全量基线检查）**：复用既有架构漂移门禁 runner（`ArchiCheckRunner`，报告槽位恒 `ReportSlot::Archi`——**MUST NOT** 落 `reports/baseline`），命令 SHALL 为 `ToolCommand::ArchiCheck { change_id, mode: ArchiCheckMode::Full }`，变更集为空且 `steps.rs` MUST NOT 触碰 vcs 缝（全量模式忽略变更集；增量口径在基线上恒 `noChanges` = 空转，见 desktop-archi-gate「门禁挂载点」）。该步声明 `on_fail=Terminal`：`passed` → 节点 passed 后继续 test-execution；`failed`（`errors` 或模型结构错）→ 节点 failed + `Terminal::failed`（文案为基线架构专用模板，携 `reports/archi` 目录——三检查步不共用文案）；`skipped`（无模型）/ `noChanges`（零可判文件）→ 步 passed，但 detail MUST 逐字写「未检查（无模型）」/「未检查（无可判文件）」，MUST NOT 冒充「检查通过」（未接入架构模型的工作区 MUST NOT 因本步被阻断）；取消 → 节点 stopped + run stopped（fault 先于结论）。该臂 MUST NOT 发起 WorkerAgent 会话、MUST NOT 消耗反馈边计数、MUST NOT 自动改写模型或源码。

相位收口 SHALL 由 `settle_phase` 以机械 checklist 纯桌面代写（零 agent，沿 `test_execution_phase_log_input` 先例——steps 含 TestExecution 步则 `test_outcome` 自动挂载、三分支天然分派），report 指向 baseline 报告目录，三会话槽位恒 `None`；ArchiCheck 步的既存面（`mode=full`）MUST NOT 改变该分派（判据是「steps 是否含 TestExecution 步」，MUST NOT 引入按步新增分支）。

一次性语义 SHALL 由既有落账机制天然成立：任一步 `Terminal` 失败时 PhaseRecord 零 initialize 条目（图节点照常落 `RunNodeRecord` 留痕可回查）→ 再发起时 `phase_next` 仍路由 initialize、Clean 步清残留后干净重试；全绿 pass 落 PhaseRecord（机械 checklist，verdict=pass）→ 持久跳过（后续 run 不再重跑基线检查）。**三检查步在一次性语义上完全同权**：archi 基线红与 static 红、测试红一样，零 PhaseRecord 条目、零「部分检查已通过」残留。initialize 收口后的 phase 间停等 SHALL 与全相位一致（`auto_next_phase=false` 时照常 `ConfirmWait` 停等，零特判豁免）。legacy 记录（`ChangeRecord.worktree=None`）SHALL 整相位短路：步序列零执行、零 spawn、零判定（含 archi 全量扫描零执行）、主仓 MUST NOT 被 reset 或 clean，以 `enter_phase` → `settle_phase(skipped=true)` 收口（跳过因注明 legacy 主仓执行无 worktree 基线，一次性持久同 pass 语义）。

#### Scenario: 全绿序曲与一次性跳过

- **WHEN** 新建 requirement change 发起 run（基线 static-check、archi-check 与 test-execution 全绿，auto_next_phase=true）
- **THEN** initialize 相位先于 proposal 执行：Clean / StaticCheck / ArchiCheck / TestExecution 四节点按序上图（零 WorkerAgent 会话），checks 域两报告落 `reports/baseline`、漂移报告落 `reports/archi`；相位收口机械 checklist 落账 pass 后后续相位照常推进；同 change 再次发起 run 时 initialize 被持久跳过（零重跑）

#### Scenario: 基线 static-check 红 Terminal 停止

- **WHEN** 基线 static-check 失败（真件 shim 配置为红）
- **THEN** run 以 `Terminal` 收敛 failed，文案引导「在基线上修复后重新发起」并携 baseline 报告目录；PhaseRecord 零 initialize 条目、零反馈边注入；后续步（含 archi-check 与 test-execution）零执行；再发起时 Clean 步恢复基线后重试

#### Scenario: 基线 archi-check 红 Terminal 停止

- **WHEN** 基线全量架构判定返回 `errors` 非空或 `model.structuralErrors` 非空（且 static-check 已绿）
- **THEN** archi-check 节点 failed（detail 为诊断摘要）、test-execution 步零执行；run 以 `Terminal::failed` 收敛，终态文案为基线架构专用模板并携 `reports/archi` 目录；零 WorkerAgent 会话、零反馈边计数、模型与源码零改写；PhaseRecord 零 initialize 条目；再发起时 Clean 恢复基线后重跑四步

#### Scenario: 基线无模型不阻断

- **WHEN** 观察根下不存在架构模型（`<archi_dir>/models` 缺席或零 `*.toml`）
- **THEN** archi-check 节点 passed 且 detail 逐字「未检查（无模型）」，流程继续 test-execution；run 不因本步失败，报告 `status = skipped` 原样保留

#### Scenario: test-execution Error 与 Fail 分文案

- **WHEN** 基线 test-execution 分别以 `conclusion=Error`（框架探测失败 / spawn 失败等环境问题）与 `conclusion=Fail`（测试红）收场
- **THEN** 两者均以 `Terminal` 停止 run，但终态 reason 文案可辨：Error 引导查配置 / 环境，Fail 引导修基线测试；两态不共用同一段文案，且两者都与基线架构文案相异

#### Scenario: clean 恢复基线且排除面保留

- **WHEN** 上一次 run 失败后 worktree 残留未提交代码改动与 untracked 文件，再发起进入 initialize 的 Clean 步
- **THEN** tracked 修改被 reset、untracked 残留被清除，worktree 恢复基线态；node_modules / dist / build / target / coverage / .nyc_output 与 dot 目录、`openspec/` 工件树原样保留（依赖与过程文档不丢，后续基线测试可跑、已落产物与既存 `reports/archi` 报告可读）

#### Scenario: legacy 整相位短路

- **WHEN** 以 `worktree=None` 的存量记录发起 run
- **THEN** initialize 步序列零执行（零 vcs / 零检查 spawn / 零架构全量扫描），主仓零 reset 零 clean，相位经 `enter_phase` + `settle_phase(skipped=true)` 落账（跳过因可见），后续相位照常路由

#### Scenario: 收口停等与全相位一致

- **WHEN** 手动档（auto_next_phase=false）run 的 initialize 相位收口落账 pass
- **THEN** run 照常进入 phase 间停等（`ConfirmWait`），用户确认后续走 proposal；无「发起即确认」特判豁免

### Requirement: ArchiCheck 步接线与反馈边

Walker SHALL 执行架构漂移门禁步（`StepNode::ArchiCheck` / `ToolCommand::ArchiCheck`，相位序列见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），并 SHALL 以**步声明的 `on_fail`** 分派两个执行体（MUST NOT 按相位名判断）：

| 表项声明 | 执行体 | 行为 |
|---|---|---|
| `FeedbackLoop { limit: 3 }`（implement 相位） | `step_archi_check_feedback` | 违例注入同一 executor 会话修复，超限升格相位 fail |
| `Terminal`（initialize 表头） | `step_archi_check_terminal` | 单发一次：`passed` 继续 / `failed` 终态停止 run，零会话、零反馈边 |
| `Escalate` / `None` | —— | 计划图漂移：显式 `Err`（不静默 `Proceed`、不退化为任一执行体） |

该步 SHALL 经 `ArchiCheckRunner` port（定义落 `crates/core/orchestration::port`，实现落 `crates/infra/archi`，见 desktop-archi-gate「ArchiCheck runner port 与报告落盘」）取结论，产出走既有 `ToolStepOutput` 封闭集变体 `ArchiCheck(ArchiCheckOutcome)`；`workflow` 侧 MUST NOT 出现任何 `archi` 依赖或 `archi_check` 字面判据（`workflow` ⇄ `archi` 互不依赖，本步的语义只由相位表与 `on_fail` 声明承载）。

分派语义 SHALL 与既有门禁步同型：

- **fault 先于结论**：`ArchiCheckOutcome.fault` 在位时早退——`Environment` → 步节点 `failed` + `Terminal::failed`，`Stopped` → 步节点 `stopped` + `Terminal::stopped`；两臂均 MUST NOT 发起 WorkerAgent 会话、MUST NOT 消耗反馈边计数。架构检查零进程 spawn（见 desktop-archi-gate），故 `fault` 在生产路径上恒 `None`；字段与分派臂保留是为了让 walker 的第四个 runner 与既有三步同形，MUST NOT 为它写特例分支（两执行体各持一份同形早退臂，`settle_fault` 的 `template` 两处均为 `None`——不新增环境文案常量）。
- **通过（两执行体同式）**：`passed = true` → 步节点 `passed`（detail 携检查模式与状态词，经既有 `diagnose_brief` 口径截断）→ 继续下一步（implement → evaluator；initialize → test-execution）。
- **不通过 · 反馈边臂（implement）**：`passed = false` → 步节点 `failed`（detail 为诊断摘要）→ 独立计数 `feedback`（上限取步声明的 `FeedbackLoop { limit: 3 }`，与 static-check 的 5 次互不占用）→ 未超限时把违例诊断注入**同一 executor 会话**（`continue_session`，`WorkerRole::Executor`，`EdgeKind::Feedback`，修复节点为 `instance_no` 递增的新节点）后重跑本步；已超限时 SHALL 升格相位 fail：经相位收口 `settle_phase(PhaseSettle::Fail(..))` 代写 fail checklist（不跑 evaluator、仅携 executor 槽位），置 `cursor.failed` 并回 `phase_next` 进决策分叉。
- **不通过 · 终态臂（initialize）**：`passed = false` → 步节点 `failed` → `Terminal::failed`，文案取 initialize 族专用常量（携 `outcome.report_dir`）；MUST NOT 注入任何会话、MUST NOT 递增反馈计数、MUST NOT 代写 fail checklist（PhaseRecord 零 initialize 条目由 `Terminal` 语义天然保证）。
- **反馈文案有界**：注入 executor 的文本 SHALL 由 runner 侧装配（违例按 `(from, to)` 聚合、每条附档位与出现次数与至多两处 `file:line`，结构性错误单列），整体 MUST 不超过 `MAX_REPORT_CHARS` 量级——MUST NOT 逐行铺开全部出现点（同一元素对在 40 个文件各命中一次时注入 40 行流水账会挤掉执行指令）。

步节点 SHALL 以 `ChangeStepKind::ArchiCheck` 上图与落库（`RunStepKind::ArchiCheck` 尾部追加，见 desktop-change-state-store「run 执行图落库」），报告路径随步 detail 呈现（有界截断下可能不全，权威指针在 initialize 的终态文案与 `reports/archi` 目录），MUST NOT 走节点 `output_ref`（该字段是 exec-outputs 回查通道，架构报告是 JSON 文件、非 exec 输出）。

#### Scenario: 违例注入同一 executor 会话

- **WHEN** implement 相位 executor 收口后 ArchiCheck 步报一条 `undeclaredDependency`（`App.Billing` → `App.Auth`，引入方 `strict`）
- **THEN** 步节点 `failed`（detail 含该违例摘要），反馈边计数 +1，`WorkerRole::Executor` 以 `continue_session` = 首次 executor 会话 id 发起一轮修复会话，新修复节点 `instance_no` 递增并以 `feedback` 边连回；修复后重跑本步

#### Scenario: 反馈边超限升格相位 fail

- **WHEN** 同一 attempt 内 ArchiCheck 连续 3 次不通过（修复后仍违例）
- **THEN** walker 不再发起第 4 次修复会话，经相位收口代写 fail checklist（item「架构检查」、report 携汇总诊断、`skipped=false`、仅 executor 槽位），相位按 fail 收场回 `phase_next` 进决策分叉；evaluator 步不执行

#### Scenario: 通过后进入 evaluator

- **WHEN** ArchiCheck 步以 `passed = true` 收场（`errors` 与 `structuralErrors` 皆空）
- **THEN** 步节点 `passed`、零反馈会话，walker 顺序进入 evaluator 步；静态检查步的反馈计数不受影响

#### Scenario: 初始化表项单发终态零会话

- **WHEN** 同一 `step_archi_check` 分派入口被 initialize 表项（`on_fail=Terminal`）调用且结论为 `errors`
- **THEN** 步节点 `failed`、run `Terminal::failed`（基线架构文案）；零 WorkerAgent 会话、零 `feedback` 边、零 fail checklist 落账；同一 attempt 内不重跑该步

#### Scenario: 计划图漂移不静默

- **WHEN** `on_fail` 声明为 `Escalate`（或缺失）的 `ArchiCheck` 步被分派
- **THEN** walker 显式 `Err`（计划图漂移文案），MUST NOT 退化为反馈边臂或终态臂、MUST NOT 静默 `Proceed`

#### Scenario: 零进程零会话的失败面

- **WHEN** 审查 `step_archi_check_terminal` / `step_archi_check_feedback` 与 `ArchiCheckRunner` 实现的进程触点与会话触点
- **THEN** 除反馈边注入的 executor 续注外零 WorkerAgent 会话、零进程 spawn；`fault` 恒 `None`，`settle_fault` 分支仅作为同形保留（不产生任何新增环境文案常量）

## Module Contract

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/orchestration/src/walker.rs` | 两执行体与分派 | `step_archi_check(ctx, trace, cursor, step)`（分派入口）→ `step_archi_check_terminal` / `step_archi_check_feedback`；`TXT_BASELINE_ARCHI_FAILED`（`{dir}` 占位，与 `TXT_BASELINE_STATIC_FAILED` / `TXT_BASELINE_TEST_ERROR` / `TXT_BASELINE_TEST_FAILED` 同族不共用）；`step_clean` / `step_static_check_terminal` / `step_test_execution_terminal` 零改动 |
| `crates/core/orchestration/src/steps.rs` | 命令消费点 | `ToolCommand::ArchiCheck { change_id, mode }` 两模式分派（增量取数 / 全量零 vcs）；审计行 `StepKind::ArchiCheck` 与摘要文本零改动 |
| `crates/core/orchestration/src/port.rs` | port 语义 | `ArchiCheckMode`、`ArchiCheckRunner::run(root, name, mode, changed, cancel)`、`ArchiCheckOutcome`（字段零变化） |
| `crates/core/workflow/src/write/phase_table.rs` | 挂载点 + 分派依据 | `INITIALIZE_STEPS` 四步（`ArchiCheck` 在 `StaticCheck` 与 `TestExecution` 之间，`Terminal`）；implement 的 `ArchiCheck(FeedbackLoop { limit: 3 })` 逐字不变 |
| `crates/core/workflow/src/state.rs` | 词汇面（零扩展） | `StepNode::ArchiCheck` / `RunStepKind::ArchiCheck` 已在二十二值内，变体与序 MUST NOT 变；`OnFail::Terminal` 的 doc 补第三处消费点（**非出线类型，`bindings.ts` 零变化**） |
| `crates/infra/archi/src/runner.rs` | 执行体 | 见 desktop-archi-gate「ArchiCheck runner port 与报告落盘」 |
