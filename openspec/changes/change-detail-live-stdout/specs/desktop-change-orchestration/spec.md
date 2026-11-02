# desktop-change-orchestration Specification（delta: change-detail-live-stdout）

## MODIFIED Requirements

### Requirement: static-check spawn 步门禁

implement / test-gen 相位的 executor 收口后，walker SHALL 必经 static-check 步（checks 边界 `crates/infra/checks` spawn 执行，承接原 SubagentStop hook 的门禁职责；该步由相位 step 定义声明，非 `STATIC_CHECK_PHASES` 常量注入）；initialize 相位 SHALL 声明同款 static-check 步承载基线绿门（语义分叉见下）。static-check 步 SHALL 经 id → 记录 → name 解析（`store.get_change(id).map(name)`，与 test-execution 分支同式）把 change name 传入 `StaticCheckRunner::run(root, name)`——name 供 infra 侧 output 落盘（recorded，`output_ref` 回填，`<report_dir>/exec-outputs/<exec_id>.log`）；未建档（无 `ChangeRecord`）显式 `Err`。**`output_ref` 不再只在终态出现**：本步（与 test-execution 步）SHALL 经 `run_tool` 携带 `on_output_ref` 回调（见「脚本步运行中输出引用前置上抛」），使 running 与终态两拍节点携**同一** `output_ref`——运行中节点因此可直接进入 stdout 面板。implement / test-gen 相位的 static-check 失败 SHALL 走**定向反馈边**：将捕获的诊断反馈注入同一 executor 会话（Continue）修复，独立计数上限 5 次（沿原 hook `loop_limit` 语义）；每次修复重跑为 `instance_no` 递增的新节点、图上以 `feedback` 边表达。超限 SHALL 升格为相位 fail（walker 以桌面代写的 fail checklist 经写面 `phase_log` 落账，不跑 evaluator，进入决策分叉——重做本相位经决策回溯到自身表达（白名单自含），窗口 fail 计数即预算面）。MUST NOT 将反馈边重试直接计入相位 retry 预算。initialize 相位的 static-check 步失败 SHALL 以 `Terminal` 收敛（`on_fail=Terminal`，无反馈边、零 agent 会话注入——基线的债不属于本次变更，见「initialize 基线检查相位执行体」）。非 implement / test-gen / initialize 相位 MUST NOT 触发 static-check（其 step 序列不含 static-check 步）。

#### Scenario: 修复反馈边

- **WHEN** executor 收口后 static-check 报 lint / fmt 错误
- **THEN** walker 将诊断文本注入同一 executor 会话（Continue）重试修复，反馈边计数 +1，新修复节点 `instance_no` 递增；修复后 static-check 通过则相位继续 evaluator 步

#### Scenario: 反馈边超限升格

- **WHEN** 同一相位内定向反馈边已达 5 次仍不通过
- **THEN** walker 以代写 fail checklist 经写面 `phase_log` 落账（不跑 evaluator），相位按 fail 收场进入决策分叉（重做本相位经决策回溯到自身表达），walker 不再注入反馈边

#### Scenario: 门禁相位限定

- **WHEN** proposal / dev-design 等非门禁（implement / test-gen / initialize）相位 executor 收口
- **THEN** walker 不发起 static-check spawn（该相位 step 序列不含 static-check 步），直接进入 evaluator 步

#### Scenario: static-check 步 name 解析与 recorded

- **WHEN** walker 走到某已建档 change 的 static-check 步（假 store 供给 name）
- **THEN** `StaticCheckRunner::run(root, name)` 收到该 change 的裸名，`StaticCheckOutcome.output_ref` 回填（passed 与 failed 两分支均携 `output_ref`），节点可经 stdout 面板回查；change 未建档则显式 `Err`

#### Scenario: 运行中节点已携输出引用

- **WHEN** static-check 步的 `running` emit 已发出、进程尚未结束（假 runner 回调 `exec_id` 后挂起）
- **THEN** 该节点 `status == running` 且 `output_ref == Some(exec_id)`，与进程结束后终态 emit 的 `output_ref` 同一值；节点在运行中即可被点击进入 stdout 面板

#### Scenario: initialize 基线步无反馈边

- **WHEN** initialize 相位的 static-check 步失败
- **THEN** 步以 `Terminal` 收敛 run failed（零反馈边、零 executor 会话注入），文案引导在基线上修复后重新发起；不消耗 FeedbackLoop 预算

## ADDED Requirements

### Requirement: 脚本步运行中输出引用前置上抛

工具步 port SHALL 承载「输出引用上抛」通道，使脚本步（static-check / test-execution）在**运行中**即可暴露 `output_ref`：

- `ToolStepRequest` SHALL 增字段 `on_output_ref: Option<OutputRefSink>`；`OutputRefSink` SHALL 为 `Arc<dyn Fn(&str) + Send + Sync>` 的 newtype（`Clone`；手写 `Debug` 与 `PartialEq` / `Eq`——`Arc::ptr_eq` 身份语义，沿 `CancelFlag` 的「`Eq` = 同一底层对象」先例）。其构造面 SHALL 为 `pub fn new(run: impl Fn(&str) + Send + Sync + 'static) -> Self`（**跨 crate 可见**）：生产唯一路径是 `walker::run_tool`，但 `crates/infra/checks` 的顺序不变量用例（AC-11）必须能在回调内断言「上抛时刻 live `.log` 已在」，`pub(crate)` 会使该臂结构性不可写。投递面 SHALL 为 `pub fn emit(&self, exec_id: &str)`。
- `StaticCheckRunner::run` 与 `TestExecutionRunner::run` SHALL 各增末位形参 `on_output_ref: Option<OutputRefSink>`；实现（`crates/infra/checks`）在 live 记录文件**已创建之后**调用它一次（顺序与语义见 desktop-checks-domain「输出记录收敛到 infra/exec」）。**未提供或未调用 = 老行为**——既有测试 mock runner 只需在实现签名上补一个形参（忽略即可），其行为断言 MUST NOT 被迫改写。
- `walker::run_tool` SHALL 增 `identity: Option<StepIdentity>` 形参（`phase` / `attempt` / `step` / `instance_no` 四段），在位时为该步组装 `OutputRefSink`，回调内补发一拍带 `output_ref` 的 `running` 步状态；四位脚本步（static-check 两处 + test-execution 两处）传 `Some(identity)`，其余工具步传 `None`。
- 补发拍 SHALL 经 `ChangeFlowControl` 的现有累积器合并：`accumulate_node` 既有的「`output_ref` some 才覆写」语义 MUST NOT 改变——补发拍只补字段、MUST NOT 新增节点、MUST NOT 覆盖 `started_at`（保首见）、MUST NOT 改动 `status` 之外的在位值。
- 发射器 MUST 为 `'static`：回调跨 port 边界时不能借用 `&RunGuard`，故 SHALL 经 `RunGuard::emitter() -> StepEmitter`（持 `(root, change_id)` 复合键 + `Arc<ChangeFlowControl>`）重建，`StepEmitter::attach_output_ref(phase, attempt, step, instance_no, exec_id)` 复用同一铸造单点。
- **emit 面收口**：`StepSettle`、`emit_node`、`now_millis` SHALL 归位 `crates/core/orchestration/src/control.rs`（与 `accumulate_node` / `attach_session` 同文件、与 `RunGuard` 同模块），成为**铸造点单点**——`ChangeStepState` 加字段时只有一个地方要填、编译器可兜。walker 侧 SHALL 只扩一行 `use`，全部既有 `emit_node(guard, …)` 调用点（源码形与参数）MUST NOT 改动。
- 本上抛链 MUST NOT 改变 `RunUpdate` / `RunNotice` / `ChangeStepState` 的线面形状与 bincode 版本（`output_ref` 字段本就在案），MUST NOT 引入新通知类型。

#### Scenario: running 拍补发与终态同 id

- **WHEN** 假 runner 在 `run` 内回调 `on_output_ref("exec-x")` 后返回 `output_ref = Some("exec-x")` 的产出
- **THEN** 图上该节点先出一拍 `running + output_ref = Some("exec-x")`、终态拍同值；节点总数为 1（补发拍被就地合并，无重复节点）

#### Scenario: 未提供回调即老行为

- **WHEN** 以 `on_output_ref: None`（或 mock runner 从不调用回调）执行 static-check / test-execution 步
- **THEN** running emit 的 `output_ref` 仍为 `None`、终态拍按产出回填；零补发拍、零额外通知；既有 mock runner 只补一个未使用的形参即可通过编译，其行为断言零改写

#### Scenario: 补发拍不扰动既有合并语义

- **WHEN** 节点已有 running 拍（`started_at` 已铸），随后收到补发拍（携 `output_ref`）与终态拍
- **THEN** `started_at` 保持首见值、`ended_at` 与 token 仅 some 才覆写、`status` 按拍次推进；节点位置保持首次 emit 序

#### Scenario: emit 铸造单点归位后调用面零改

- **WHEN** 审查 `walker.rs` 与 `control.rs` 的 `emit_node` / `StepSettle` / `now_millis` 归属，与全部 `emit_node(...)` 调用点
- **THEN** 定义只在 `control.rs` 一处；`walker.rs` 侧仅 `use` 一行增加，全部调用点的源码文本与参数个数保持不变；既有时间戳 / token 铸造用例全绿

#### Scenario: 上抛时刻文件已在

- **WHEN** 回调内立刻（用其 `exec_id` + change 目录）读取 `<report_dir>/exec-outputs/<exec_id>.log`
- **THEN** 文件与其父目录均已在案（`Ok`），无需等待、无需重试——「先落盘后上抛」的顺序不变量在 walker 与 checks 两侧均由用例钉住。checks 侧用例 SHALL 能经 `OutputRefSink::new(..)`（`pub`）直构回调以完成该断言；真实双侧（walker × `ProcessStaticCheck`）的端到端组合用例 SHALL 落在同时依赖 `orchestration` 与 `checks-runtime` 的壳层 crate（`crates/core/orchestration` 的 `Cargo.toml` MUST NOT 为它引入 `checks-runtime` dev-dependency——那会形成 core → infra 反向依赖边）。

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/port.rs`（扩） | 工具步请求与 runner 契约 | `ToolStepRequest` 增 `on_output_ref: Option<OutputRefSink>`（缺省 `None` = 老行为）；`OutputRefSink` newtype（`Clone` + 手写 `Debug` + `PartialEq`/`Eq` 身份语义 + **`pub fn new(..)`**——跨 crate 构造面，checks 侧顺序不变量用例的前提）；`StaticCheckRunner::run` / `TestExecutionRunner::run` 各增末位 `on_output_ref: Option<OutputRefSink>`；`ToolStepOutput` / `RunFault` / 三 outcome 载荷零变更 |
| `crates/core/orchestration/src/steps.rs`（扩） | 进程内工具步透传 | static-check / test-execution 两分支把 `step.on_output_ref` 原样透传给对应 runner；其余分支不受影响（`Option` 未消费即丢弃） |
| `crates/core/orchestration/src/control.rs`（扩） | 步状态铸造与上抛发射器 | 上收 `StepSettle` / `emit_node` / `now_millis`（与 `accumulate_node` / `attach_session` 同文件，铸造点单点）；新增 `RunGuard::emitter() -> StepEmitter` 与 `StepEmitter { key, control: Arc<ChangeFlowControl> }`；`StepEmitter::attach_output_ref(phase, attempt, step, instance_no, exec_id)` 补发一拍 `running + output_ref`；`accumulate_node` 合并语义零改动 |
| `crates/core/orchestration/src/walker.rs`（收窄） | `run_tool` 组装回调 | 增 `identity: Option<StepIdentity>` 形参：在位时以 `guard.emitter()` 组装 `OutputRefSink`（回调内调 `attach_output_ref`），四位脚本步传 `Some`、其余传 `None`；`emit_node` / `StepSettle` / `now_millis` 定义移出本文件（顶层 `use` 扩一行，调用点零改动） |
| `crates/core/orchestration/src/state.rs`（零改动） | 活面契约 | `ChangeStepState` 字段面不变（`output_ref` 本就在案）；无新步词、无新状态值 |
| `src/commands/change_flow/mod.rs`（零改动） | 运行控制命令面 | `change_flow_start` / `stop` / `answer` / `confirm` / `watch` 签名与事件面不变；`RunUpdate::Step` 载荷不变 |
| `crates/infra/checks/src/static_check.rs` / `testexec/mod.rs`（消费面） | 上抛调用点 | 文件已建后调用 `on_output_ref(exec_id)` 一次；`on_output_ref == None` 时零行为差异（见 desktop-checks-domain） |
| `crates/infra/checks/src/mutation/**`（零改动） | 突变段 | 不接 `on_output_ref`（`MutationRunner::run` 签名不变），突变段零输出记录面 |
