# desktop-change-orchestration Specification（delta: archi-check-and-gate）

## ADDED Requirements

### Requirement: ArchiCheck 步接线与反馈边

Walker SHALL 在 implement 相位执行架构漂移门禁步（`StepNode::ArchiCheck` / `ToolCommand::ArchiCheck`，相位序列见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）。该步 SHALL 经 `ArchiCheckRunner` port（定义落 `crates/core/orchestration::port`，实现落 `crates/infra/archi`，见 desktop-archi-gate「ArchiCheck runner port 与报告落盘」）取结论，产出走既有 `ToolStepOutput` 封闭集新增变体 `ArchiCheck(ArchiCheckOutcome)`；`workflow` 侧 MUST NOT 出现任何 `archi` 依赖或 `archi_check` 字面判据（`workflow` ⇄ `archi` 互不依赖，本步的语义只由相位表与 `on_fail` 声明承载）。

分派语义 SHALL 与既有门禁步同型：

- **fault 先于结论**：`ArchiCheckOutcome.fault` 在位时早退——`Environment` → 步节点 `failed` + `Terminal::failed`，`Stopped` → 步节点 `stopped` + `Terminal::stopped`；两臂均 MUST NOT 发起 WorkerAgent 会话、MUST NOT 消耗反馈边计数。架构检查零进程 spawn（见 desktop-archi-gate），故 `fault` 在生产路径上恒 `None`；字段与分派臂保留是为了让 walker 的第四个 runner 与既有三步同形，MUST NOT 为它写特例分支。
- **通过**：`passed = true` → 步节点 `passed`（detail 携检查模式与状态词）→ 继续 evaluator 步。
- **不通过**：`passed = false` → 步节点 `failed`（detail 为诊断摘要，经既有 `diagnose_brief` 口径截断）→ 独立计数 `feedback`（上限取步声明的 `FeedbackLoop { limit: 3 }`，与 static-check 的 5 次、verdict 的 5 次互不占用）→ 未超限时把违例诊断注入**同一 executor 会话**（`continue_session`，`WorkerRole::Executor`，`EdgeKind::Feedback`，修复节点为 `instance_no` 递增的新节点）后重跑本步；已超限时 SHALL 升格相位 fail：经写面 `phase_log` 代写 fail checklist（不跑 evaluator、仅携 executor 槽位），置 `cursor.failed` 并回 `phase_next` 进决策分叉。
- **反馈文案有界**：注入 executor 的文本 SHALL 由 runner 侧装配（违例按 `(from, to)` 聚合、每条附档位与出现次数与至多两处 `file:line`，结构性错误单列），整体 MUST 不超过 `MAX_REPORT_CHARS` 量级——MUST NOT 逐行铺开全部出现点（同一元素对在 40 个文件各命中一次时注入 40 行流水账会挤掉执行指令）。

步节点 SHALL 以 `ChangeStepKind::ArchiCheck` 上图与落库（`RunStepKind::ArchiCheck` 尾部追加，见 desktop-change-state-store「run 执行图落库」），报告路径随步 detail 呈现，MUST NOT 走节点 `output_ref`（该字段是 exec-outputs 回查通道，架构报告是 JSON 文件、非 exec 输出）。

#### Scenario: 违例注入同一 executor 会话

- **WHEN** implement 相位 executor 收口后 ArchiCheck 步报一条 `undeclaredDependency`（`App.Billing` → `App.Auth`，引入方 `strict`）
- **THEN** 步节点 `failed`（detail 含该违例摘要），反馈边计数 +1，`WorkerRole::Executor` 以 `continue_session` = 首次 executor 会话 id 发起一轮修复会话，新修复节点 `instance_no` 递增并以 `feedback` 边连回；修复后重跑本步

#### Scenario: 反馈边超限升格相位 fail

- **WHEN** 同一 attempt 内 ArchiCheck 连续 3 次不通过（修复后仍违例）
- **THEN** walker 不再发起第 4 次修复会话，经写面代写 fail checklist（report 携汇总诊断、`skipped=false`、仅 executor 槽位），相位按 fail 收场回 `phase_next` 进决策分叉；evaluator 步不执行

#### Scenario: 通过后进入 evaluator

- **WHEN** ArchiCheck 步以 `passed = true` 收场（`errors` 与 `structuralErrors` 皆空）
- **THEN** 步节点 `passed`、零反馈会话，walker 顺序进入 evaluator 步；静态检查步与 verdict 步的反馈计数不受影响

#### Scenario: 零进程零会话的失败面

- **WHEN** 审查 `step_archi_check` 与 `ArchiCheckRunner` 实现的进程触点与会话触点
- **THEN** 除反馈边注入的 executor 续注外零 WorkerAgent 会话、零进程 spawn；`fault` 恒 `None`，`settle_fault` 分支仅作为同形保留（不产生任何新增环境文案常量）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/port.rs`（`ToolCommand`） | 命令封闭集 | 尾部追加 `ArchiCheck { change_id }`（载荷最简：报告槽位恒 `Archi`，由 runner 单点取用——不设 slot 字段，避免出现恒一值的形参；磁盘面 port 收 name，id → 记录 → name 解析在 `steps.rs`） |
| `crates/core/orchestration/src/port.rs`（`ToolStepOutput`） | 产出封闭集 | 尾部追加 `ArchiCheck(ArchiCheckOutcome)`；`ArchiCheckOutcome { passed, diagnostics, fault, report_dir, report_ref }`（`report_ref` 为报告文件路径，`None` = 落盘失败不阻断结论） |
| `crates/core/orchestration/src/port.rs`（`ArchiCheckRunner`） | 门禁 runner port | `fn run(&self, root: &str, name: &str, changed: Vec<String>, cancel: CancelFlag) -> BoxToolFuture`——`changed` 由 `steps.rs` 经 `ArchiveVcsPort::change_edit_set(root, "HEAD")` 注入（与 `MutationRunner` 同式，infra 侧零 git 依赖；取数失败显式传播，MUST NOT 降级为空清单）；报告槽位不设形参（恒 `ReportSlot::Archi`，由 runner 内部取用） |
| `crates/core/orchestration/src/steps.rs` | 分派与审计 | `ToolCommand::ArchiCheck` 分支：name 解析（未建档显式 `Err`）→ `change_edit_set` → runner → `StepKind::ArchiCheck` 审计行（摘要 `archi_check passed=<bool> status=<status>`，reference 为报告目录） |
| `crates/core/orchestration/src/walker.rs`（`step_archi_check`） | 反馈边执行体 | fault 先行、`FeedbackLoop` 计数 ≤3、超限代写 fail `phase_log` + `StepFlow::Upgraded`、未超限 `continue_session` 注入 executor |
| `crates/core/orchestration/src/run_history.rs` | 词汇映射 | `ChangeStepKind::ArchiCheck` → `RunStepKind::ArchiCheck` 1:1（`mapped_step` 穷尽臂） |
| `crates/core/orchestration/src/control.rs` | 步分派 | `is_session_step` 对 `ArchiCheck` 判假（工具步，非会话步） |
