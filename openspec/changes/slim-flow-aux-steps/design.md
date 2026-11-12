# 设计: slim-flow-aux-steps

> **变更**: slim-flow-aux-steps
> **状态**: 设计定稿（基于已定稿 proposal + 8 份 delta spec）
> **基于**: `proposal.md`（问题 / 提案 / D1–D6 / A1–A6 + B1–B3 / 变更范围 / AC-1…AC-12 / R1–R11）、`explore.md`（只读结论稿）、`specs/{desktop-change-orchestration, desktop-workflow-write-face, desktop-change-archive, desktop-change-flow-view, desktop-change-state-store, desktop-test-execution, desktop-mutation-testing, desktop-corpus-regression}/spec.md`
> **性质**: 相位机形态变更（计划步 → 固定工程动作）+ 分派面收窄；**零 db 迁移、零 IPC 出线变化、零公共 API 面变化、零新 crate / 零新依赖边**
>
> **本篇边界**: 三层 crate 的 5 个 `.rs` 文件（`core/orchestration/{walker,steps}.rs`、`core/workflow/src/{state.rs,write/phase_table.rs}`、`core/orchestration/src/state.rs`）+ 版本一行。前端 `packages/desktop/src/**` 零 diff（`run-state.ts` 的五词解码为旧库行必需）；`crates/infra/store/**` 零 diff；14 份 role prompt md 零 diff。

---

## 起点核实（工作树实测；路径前缀 `packages/desktop/src-tauri/crates/`，锚点为改动前行号）

- **计划步表**（`core/workflow/src/write/phase_table.rs`）：11 张 `*_STEPS`，辅助步声明实为 **30 处**（`StepNode::PhaseStart`×11 / `PhaseLog`×11 / `VerdictGate`×8）。逐表步数实测：initialize 5 / proposal 5 / code-analyze 5 / dev-design 5 / test-design 5 / implement 7 / test-gen 6 / test-execution 4 / archive 11 / code-review 4 / acceptance 4 —— requirement 表合计 **56 步**（与 proposal 口径校正一致）。
- **相位机**（`core/orchestration/src/walker.rs`，3786 行）：`drive`(:214) → `run_step`(:519) 的 20 个分支（19 实臂 + 1 个三变体合并臂 `Decision | RetryGate | WhitelistGate => Proceed`）；辅助步执行体 `step_phase_start`(:616) / `step_initialize_skipped_phase_log`(:734) / `step_verdict_gate`(:1559) / `step_phase_log`(:1643)；落账面 `step_gate_fail_phase_log`(:1810) / `step_test_execution_upgraded_phase_log`(:1955) / `step_mutation_test_upgraded_phase_log`(:2027)；归档辅助 `begin_archive_node`(:2106) / `settle_archive_node`(:2133)（调用 12 / 25 处）；`resolve_deadlock`(:3125) / `decision_session`(:3246) / `run_worker`(:3441)。改造后 `run_step` 为 17 个分支（16 实臂 + 1 个六变体不可达臂），`match` 仍覆盖 22 变体。
- **相位携出状态**（`PhaseCursor`，:459-489）：实测 **13 字段**（`entry_kind` / `attempt` / `executor_session` / `executor_level` / `eval_outcome` / `verdict` / `test_outcome` / `mutation_outcome` / `preflight` / `rebase_conflicts` / `sync_specs` / `seal_outcome` / `failed`）。
- **词汇表**：`workflow/state.rs` 的 `StepNode` / `RunStepKind` 与 `orchestration/state.rs` 的 `ChangeStepKind` 各 **22 值**（顺序一致）；`RunNodeRecord.step` 经 bincode 按下标编码。`StepNode::group()` 的 `VerdictGate | RetryGate | WhitelistGate => StepGroup::Gate` 分支在位。
- **四门禁臂的 id→name 解析**（`core/orchestration/src/steps.rs:212/240/276/321`）：四处各写一遍 `store.get_change(&change_id)?.map(|r| r.name).ok_or_else(...)`，文案分别为「无从执行静态检查 / 测试门禁 / 突变门禁 / 架构漂移门禁」。
- **`ToolCommand::PhaseStart` / `PhaseLog`**（`port.rs:99/101`）：命令变体与写面直调（`workflow::write::{phase_start, phase_log}`）**零改动**——`enter_phase` / `settle_phase` 仍经这两条命令走写通道（orchestration 不持 store 句柄，写面只能经 port）；本变更删的是 `steps.rs` 这两分支的 `audit_outcome` 调用（审计面归零，见 D11）。
- **版本**：`packages/desktop/package.json:3` `version = "0.6.35"`。

### 与 proposal / explore 措辞的口径校正（不改需求意图，只定实现形态）

1. **`PhaseCursor` 字段数**：explore 记「14 字段」，实测 13。本设计删除 `entry_kind` / `eval_outcome` / `verdict` 三字段、新增 `evaluator_turn` 一字段 ⇒ 收口后 11 字段。
2. **`begin_archive_node` / `settle_archive_node` 不是「同一动作的两形态」**（proposal B2 的「只差是否先发 running」为简写）：二者是**开节点**（`begin_node` + running emit，12 处调用）与**收口节点**（单终态 emit，25 处调用）的成对辅助。B2 落地为**按对命名**（`open_step` / `close_step`），不合并为单函数——合体会把「收口须复用开出的 `instance_no`」这条不变量藏进参数默认值。
3. **`resolve_change_name` 是 `steps.rs` 的私有函数**：本变更**零公共 API 面变化**（`walk_run` / `RunRequest` / `ToolCommand` 签名全不动），故设计不把它登记为公共函数，只在「修改文件」与内部签名表登记。
4. **`step_gate_fail_phase_log` 与两个 `*_upgraded_phase_log` 处置不同**：spec Module Contract 明写前者「保留为载荷构造辅助、函数体只装配 `PhaseFail` 并调 `settle_phase`」，而两个 upgraded 落账是「**合入** `PhaseSettle::Fail`」（函数删除）。本设计按 spec 逐条落地（见 D6）。
5. **`run_worker` 与 `SettleMode` 的新形参会让全部 7 个调用点改动**（定义 1 处 + 调用 7 处），其中仅 evaluator / decision 两处传 `Deferred`；其余 5 处（executor / static-check 修复轮 / archi-check 修复轮 / resolve-conflicts / spec-sync）传 `Immediate`，语义逐字不变。
6. **`core/workflow` 的计划图收窄与 `walker.rs` 的相位机改造 MUST 同一提交**：辅助步声明删除后，`step_phase_start` / `step_phase_log` 失去全部调用点，开相与落账链路整段不存在（`active_phase` 永不写入、`phase_log` 的前置校验必 `Err`），run 在首相位即失败。tasks 的阶段一只是**编辑顺序**上的准备步，不是可独立发布的单元；本设计不在两阶段之间定义任何过渡形态。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | `enter_phase` / `settle_phase` 是否收 `guard` | 两者均多收一个 `guard: &RunGuard`（proposal 契约签名未列该形参） | 工具步写通道的唯一入口是 `run_tool(tools, guard, root, command, label, identity)`：它同时是**取消标志**（`ToolStepRequest.cancel`）与**输出窄化**（`TryFrom<ToolStepOutput>`）的单点。省掉 `guard` 只能另开一条直调 `tools.run(..)` 的旁路，等于给相位机再添一个不受取消观测的写通道 |
| D2 | `settle_phase` 收 `&PhaseCursor` 还是 `&mut PhaseCursor` | `&mut PhaseCursor` | `Pass` 三分流须**消费**产物（`test_outcome` / `mutation_outcome` / `evaluator_turn`），且收口结论要回写 `cursor.failed`（驱动 `trace.failed_phase`）。取 `&` 会把 `TestExecutionOutcome` / `MutationTestOutcome` / `WorkerTurnOutcome`（含转录）整体 clone 一次，只为绕开借用 |
| D3 | 相位入口的 `backtrack` 边由谁承载 | **迁移到相位首个真实步节点**：`EdgeTrace` 增 `entry_override: Option<EdgeKind>`，`begin_node` 在落边时**一次性**取用（有覆盖值用覆盖值，否则用调用方传的 `incoming`）；`PhaseCursor.entry_kind` 字段删除 | `PhaseStart` 曾是该边的唯一载体（`cursor.entry_kind` 只被 `step_phase_start` 的 `begin_node` 读取）。直接删字段会让「决策回溯后目标相位首节点 incoming 边为 `backtrack`」这条 spec 语义无落点；逐个步执行体改传参要动 18 处 `begin_node` 调用点（其中含 feedback 边修复轮的三处，语义各不同），覆盖值方案改动面 = 1 个字段 + 1 行 |
| D4 | `SettleMode` 的可见性 | `pub(crate) enum SettleMode { Immediate, Deferred }` | spec Module Contract 写 `pub`，但本枚举只出现在 `walker.rs` 内部（定义 + 两处生产调用），`walker::walk_run` / `RunRequest` 的公开面零变化。收进 `pub(crate)` 不新增对外词汇，`Immediate` / `Deferred` 语义仍逐字落在代码上 |
| D5 | `Deferred` 下的终态 emit 由谁写、怎么分类 | 会话收口拆两个纯函数：`worker_settle(&WorkerTurnOutcome) -> (StepSettle, Option<Terminal>)` 与 `worker_tokens(&WorkerTurnOutcome) -> (Option<u64>, Option<u64>)`；`run_worker` 的 `Immediate` 与 evaluator / decision 的 `Deferred` 共用同一份分类。`Deferred` 调用方固定写两段：**① 会话状态面（系统异常）**——`let (settle, terminal) = worker_settle(&outcome); if let Some(terminal) = terminal { emit(settle + session + tokens); return Err(terminal); }`（`Failed` / `Stopped` / `Running` 三态由此仍落**红节点 / 停止节点**，D5 第二半不破）；**② 业务面**（仅 `Completed` 可到此）——`emit(StepSettle::passed/failed(Some(自有 detail)).with_session(..).with_tokens(..))`，detail 由调用方给（`verdict=…` / `backtrack → x` / 两文案） | ① 会话状态面的四态判定与 token 求和**判据只有一份**（第二份会与既有「会话失败收敛 / 会话被终止 / 会话收口缺终态」文案分叉）；② 业务面必须由调用方裁定——「会话 `Completed` 但 `finalized` 缺席」是**业务失败**（`failed` 节点），而 `worker_settle` 对该状态给的是 `passed`，故该分支不允许复用 `worker_settle` 的 settle |
| D6 | 落账面四处收敛的边界 | ① `step_gate_fail_phase_log` + `GateFailLog` **保留**，形参去 `trace` / `attempt`（节点面消失），体内只装配 `PhaseFail` 并调 `settle_phase(PhaseSettle::Fail(..))`；② `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` **删除**，载荷改由新增纯函数 `test_execution_fail(&TestExecutionOutcome) -> PhaseFail<'static>` / `mutation_test_fail(&MutationTestOutcome) -> PhaseFail<'static>` 装配（`report` / `evidence` 组装逐字搬入），调用点两行改一行 | 两类处置的差异取自 spec 措辞（gate 那条是「保留为载荷构造辅助」，两个 upgraded 是「合入」）。`PhaseFail` 与 `GateFailLog` 字段并非一一对应（前者无 `reason` / `diagnostics` 之分），故 gate 侧保留其词汇面、两个 upgraded 侧改由具名装配函数承载文案（`truncate_report` / `mutation_report_refs` / `diagnose_brief` 调用面零变） |
| D7 | `cursor.failed` 的置位点 | 收 `settle_phase` 单点：`Pass` → `checklist.verdict == Fail`；`Fail(..)` → `true`；`Skipped(..)` → `false`；调用点（`step_gate_fail_phase_log` 的两处、两个 escalate 执行体）删各自的 `cursor.failed = true` | 相位 fail 的唯一事实源是「落账进了什么 verdict」，由落账动作回写才不会与落账产物分叉 |
| D8 | `run_step` 不可达臂的文案 | 六变体同一臂返回 `Terminal::failed(format!("计划图漂移：相位 \"{}\" 的 steps 含不可达步词 \"{}\"（该步词不出现在任何相位的 steps 内）", ctx.phase, step.node.as_str()))` | 原空臂 `Decision \| RetryGate \| WhitelistGate => Proceed` 把计划图漂移吞成一次空操作；把辅助步三词并入同一臂后，六词共用一句可归因文案（`StepNode::as_str` 已在位，零新词汇表） |
| D9 | 入口边覆盖值的生命周期 | `drive` 在**确认要走步序列**（非 legacy 短路）后置 `trace.entry_override = Some(entry_kind)`；`begin_node` 首次落边即消费；`drive` 在 `run_step` 返回 `Err` 与每轮 `continue` 前兜底清 `None` | legacy initialize 短路（零节点相位）与「首步即失败」都会让覆盖值无落点；兜底清零使「泄漏到下一相位」不可表达 |
| D10 | 升格分支的 `trace.failed_phase` 置位时机 | 步循环结束后先置 `trace.failed_phase = cursor.failed.then(|| phase.clone())`；`Upgraded` 分支据此置位后 `continue`；`Pass` 收口（`settle_phase`）之后再置一次 | `verdict=fail` 的结论由收口动作给出（`Pass` 形态），故 `failed_phase` 的第二次置位必须在收口之后——否则 verdict fail 相位的同相位重跑边判定读到旧值 |
| D11 | `phase_start` / `phase_log` 审计行如何归零（**proposal 措辞的修正点**） | **在 `steps.rs` 这两个命令分支内删掉 `audit_outcome` 调用**（写面直调与产出返回保留）。MUST NOT 依赖「walker 调用点消失」来达成 | proposal 的实现面注记写「`step_phase_start` / `step_phase_log` 调用点消失即不再经 `audit_outcome`」——该推断不成立：`enter_phase` 仍发 `ToolCommand::PhaseStart`、`settle_phase` 仍经 `run_phase_log` 发 `ToolCommand::PhaseLog`（orchestration 不持 store 句柄，写通道只能走 `ToolStepPort`），故 `audit_outcome` 会照常落行，直接违反 AC-2「零 `StepRecord`」与 state-store spec「两词新写面零产出」。审计是「相位机步骤」面而非「写面动作」面，故归零点应在产出审计的那一处（`steps.rs` 的两个分支） |
| D12 | 三形态落账失败是否共用同一前缀 | **仅 `Pass` 形态用「相位 <phase> 全部步通过但落账失败: <err>」**；`Fail(..)` → `PhaseFail` 无法落库时前缀为「相位 <phase> 门禁失败落账失败: <err>」，`Skipped(..)` → 「相位 <phase> 跳过落账失败: <err>」 | spec 的 ADDED requirement「相位收口失败可辨与相位内系统异常仍标 failed」把该前缀的**前提**钉成「相位内全部步皆 passed」——那恰是 `Pass` 形态的定义。`Fail` / `Skipped` 两形态在失败时已有红节点（门禁步）或本就零步执行（legacy 短路），若照抄「全部步通过」会给出**事实错误**的归因（R2 的变体：用户看到红节点却说「全部步通过」）。三形态仍共用同一 `settle_phase` 入口、同一 `run_phase_log`，只分文案 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 相位机固定动作（**修改**） | `enter_phase`（开相：active_phase + start_at + attempt，零 emit / 零审计）与 `settle_phase` + `PhaseSettle` / `PhaseFail`（收口：单入口三形态 → 落账 → 清位，落账失败可辨） | `crates/core/orchestration/src/walker.rs` | `workflow::write::{phase_start, phase_log}`（既有）、`ToolStepPort`（既有） | async Rust，既有 `run_tool` 单点 |
| 会话收口面（**修改**） | `SettleMode { Immediate, Deferred }`、`WorkerTurn { outcome, instance_no }`、`worker_settle` / `worker_tokens` 两个纯分类函数；`EvaluatorTurn { outcome, checklist }` | 同上 | `agent::{sum_usage_tokens, AgentRunStatus}`（既有） | 纯函数 + 既有 emit 单点 |
| 计划步分派收窄（**修改**） | `run_step` 的六词不可达臂（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` / `Decision`）统一 `Terminal::failed`；其余 16 臂逐字不动 | 同上 | `workflow::state::{StepNode, StepDefinition}`（既有） | `match` 穷尽（22 变体） |
| 相位入口边承载（**修改**） | `EdgeTrace.entry_override` 一次性覆盖 + `begin_node` 消费；`PhaseCursor.entry_kind` 退场 | 同上 | 零新增 | 单字段 + 单行分支 |
| 节点开 / 收口辅助（**修改**） | `open_step` / `close_step`（原 `begin_archive_node` / `settle_archive_node`，语义零变；调用面 = initialize 的 Clean 步 + 归档九步族 + `fail_archive_step`，辅助步退场后**不再**被落账面调用） | 同上 | 既有 `begin_node` / `emit_node` | 纯转发 |
| 决策与死锁收口（**修改**） | `decision_session`：删 `WhitelistGate` 节点、动作裁决写 decision 节点 detail、`Deferred` 终态单次 emit；`resolve_deadlock`：删 `RetryGate` 三次 emit | 同上 | `decision::ensure_backtrack_allowed`（既有纯函数）、`ToolCommand::{DecisionLog, Backtrack}` | 既有 loop（仅 ask 会进第二轮） |
| 计划图静态表（**修改**） | 11 张 `*_STEPS` 删 30 处辅助步声明；步数 requirement 29 / test-only 23 | `crates/core/workflow/src/write/phase_table.rs` | `state::{StepNode, StepDefinition, OnFail}`（既有） | 编译期 `const` |
| 词汇冻结（**修改**） | `StepNode` / `RunStepKind` / `ChangeStepKind` 三表 22 值保持 + `decode-only` 注释；`StepGroup::Gate` 分支保留 + 注释 | `crates/core/workflow/src/state.rs`、`crates/core/orchestration/src/state.rs` | 零新增 | 注释 only（零语义） |
| id→name 解析合流与开相 / 落账审计面归零（**修改**） | 四门禁臂 `resolve_change_name(store, change_id, what)`（私有）；`PhaseStart` / `PhaseLog` 两命令分支删 `audit_outcome` 调用（写面直调保留） | `crates/core/orchestration/src/steps.rs` | `workflow::state::ChangeStateStore`（既有） | 纯函数 / 单行删改 |

<!-- 零新增 crate、零新增模块目录、零新增依赖边：全部改动落在既有元素 App.Backend.Core.Orchestration / App.Backend.Core.Workflow 的文件面内部（.dev-team/architecture/models/** 零改动）。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/crates/` 前缀。测试文件与用例归测试相位，本清单只列**实现文件**。

### 新增文件

**无。** 全部改动落在既有文件的既有函数面上：`enter_phase` / `settle_phase` / `PhaseSettle` / `PhaseFail` / `EvaluatorTurn` / `SettleMode` / `WorkerTurn` / `worker_settle` / `worker_tokens` / `open_step` / `close_step` / `test_execution_fail` / `mutation_test_fail` / `resolve_change_name` 均落既有文件（`walker.rs` / `steps.rs`），不新增模块与文件。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `core/orchestration/src/walker.rs` | ① **新增** `enter_phase`（吸收 `step_phase_start`，零 emit / 零审计）；② **新增** `PhaseSettle` / `PhaseFail` / `settle_phase`（吸收 `step_phase_log` + `step_initialize_skipped_phase_log`，`Pass` 三分流逐字搬入，`Skipped` 收 legacy 短路，落账 `Err` 按形态分文案：`Pass` → 「相位 <phase> 全部步通过但落账失败: <err>」、`Fail` → 「门禁失败落账失败」、`Skipped` → 「跳过落账失败」，见 D12）；③ **新增** `SettleMode` / `WorkerTurn` / `worker_settle` / `worker_tokens`，`run_worker` 加 `mode: SettleMode` 形参并返回 `WorkerTurn`；④ **新增** `EvaluatorTurn`，`PhaseCursor` 的 `eval_outcome` + `verdict` 合一为 `evaluator_turn`（并删 `entry_kind`）；⑤ `step_evaluator` 内化裁决（读 `finalized`，缺席按截断 / 未收口两文案 `Terminal::failed`，合规时同实例 passed + `detail=verdict=…`，写 `evaluator_turn`）；⑥ `drive` 改 `enter_phase` → 迭代 `next.steps` → `settle_phase(Pass)`（legacy initialize 短路改 `enter_phase` + `settle_phase(Skipped)`；`steps.is_empty()` 判据保留；`EdgeTrace.entry_override` 置位与兜底清零；`failed_phase` 两次置位）；⑦ `run_step` 六词不可达臂；⑧ **删** `step_phase_start` / `step_phase_log` / `step_initialize_skipped_phase_log` / `step_verdict_gate` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`；⑨ `step_gate_fail_phase_log` 收窄为 `PhaseFail` 装配 + `settle_phase(Fail)`（`GateFailLog` 逐字保留）；⑩ **新增** `test_execution_fail` / `mutation_test_fail` 两个 `PhaseFail` 装配函数；⑪ `begin_archive_node` / `settle_archive_node` **重命名**为 `open_step` / `close_step`（全部 37 处调用点随动：12 处 open + 25 处 close，语义零变）；⑫ `step_test_execution_escalate` / `step_mutation_test_escalate` 改 `settle_phase(Fail)` 落账；⑬ `decision_session` 删 `WhitelistGate` 节点（动作裁决 / 越权文案写 decision 节点 detail，`ask` 轮 detail 记问答摘要后 passed）；⑭ `resolve_deadlock` 删 `RetryGate` 三次 emit；⑮ 注释同步（相位机形态、`attempt` 推导口径；`control.rs` 的瞬步 / 合并语义注释**不改**，属 proposal「不要修改」面） | 面内改动、零公共签变化（`walk_run` / `RunRequest` / `new_run_id` 全不动）；`crates/infra/store` 零改动；`ToolCommand::{PhaseStart, PhaseLog}` 与 `run_phase_log` 保留为写面通道 |
| `core/orchestration/src/steps.rs` | ① 四个门禁臂的 id→name 解析合流为私有 `resolve_change_name(store, change_id, what)`（`what` ∈ `"静态检查"` / `"测试门禁"` / `"突变门禁"` / `"架构漂移门禁"`，合成文案「change \\"{id}\\" 未建档（无 ChangeRecord），无从执行{what}」与原四处逐字等价）；② `ToolCommand::PhaseStart` 与 `ToolCommand::PhaseLog` 两分支**删其 `audit_outcome` 调用**（两分支仍保留写面直调与产出返回——`enter_phase` / `settle_phase` 仍经这两条命令走写通道，故审计行 MUST 在此归零，而非靠「调用点消失」） | `ToolCommand::{PhaseStart, PhaseLog}` 命令变体、写面直调、`audit_outcome` 函数本体与 `clip_summary` 逐字保留（其余七个命令分支照常留审计）；`StepKind::PhaseStart` / `PhaseLog` 词面保留解码 |
| `core/workflow/src/write/phase_table.rs` | 11 张 `*_STEPS` 删全部 `PhaseStart` / `VerdictGate` / `PhaseLog` 项（30 处）；步数 requirement = initialize 3 / proposal 2 / dev-design 2 / test-design 2 / implement 4 / test-gen 3 / test-execution 2 / code-review 1 / acceptance 1 / archive 9（29）；test-only = initialize 3 / proposal 2 / code-analyze 2 / test-design 2 / test-gen 3 / test-execution 2 / archive 9（23）；各 static 的 doc 注释步序串同步（`INITIALIZE_STEPS` 三步、`TEST_EXECUTION_STEPS` `TestExecution → MutationTest`、`ARCHIVE_STEPS` 九步、`CODE_ANALYZE_STEPS` P→E）；`ARCHI_FEEDBACK_LIMIT` doc 与 `IMPLEMENT_STEPS` 注释去掉「verdict 的 5 次」计数项（该反馈环已随 `finish_step` 退役） | 表内 const 序列与 `PhaseDefinition` 结构零变化；`on_fail` 语义零变化（static-check / archi-check 仍 `FeedbackLoop`，TestExecution / MutationTest 仍 `Escalate`，archive 各步仍 `None`） |
| `core/workflow/src/state.rs` | **仅注释**：`StepNode::PhaseStart` / `PhaseLog` / `VerdictGate` 三变体标 `decode-only`（计划图新表零引用、存量执行图节点行解码必需）；`RunStepKind` 的五词（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate`）同款注释（bincode 下标钉死、MUST NOT 删 / 重排）；`StepGroup::Gate` 分支与 `group()` 的三词归属加「辅助步退场后无 StepNode 归属，词汇面仍冻结」说明；`StepGroup::Exec` doc 去 `phase-start` / `phase-log` 举例 | 零语义改动、零变体增删、零顺序变化 |
| `core/orchestration/src/state.rs` | **仅注释**：`ChangeStepKind` 的五词（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate`）标 `decode-only`（出线词与节点身份键的存量行依赖，新写面零产出）；枚举 doc 的步词汇分族表述随「新写面只产三类节点」同步 | 零语义改动；`run_history.rs` 的 22 值 1:1 映射表**逐字不动**（五词仍有映射，只是新写面不产） |
| `packages/desktop/package.json` | `version` 按**落地时 `HEAD` 现值 +1 patch** 相对递增（当前盘面 `0.6.35`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行恰 `+1`，MUST NOT 写死字面号） | `tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

### 公共函数 / API

**本变更零公共 API 面变化**：`walk_run` / `RunRequest` / `new_run_id` / `ToolCommand` / `ToolStepOutput` / `WorkerTurnOutcome` / `WorkerRole` / 四个 runner port 的签名与变体集逐字不动，`lib.rs` 的再导出名单零变化。故无「公共函数 / API」条目。

**内部函数签名（非导出，仅本变更内约定；供实现与评审对拍）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `enter_phase` | `walker.rs` | 新增（吸收 `step_phase_start`） | `async fn enter_phase(tools: &Arc<dyn ToolStepPort>, guard: &RunGuard, request: &RunRequest, phase: &str) -> Result<u32, Terminal>` | 经 `ToolCommand::PhaseStart` 单事务写 active_phase（phase / attempt / start_at）；零 `begin_node` / 零 `emit_node` / 零 `StepRecord`；`Err` → `Terminal::failed("phase-start 失败: <err>")`（沿既有 label 文案） |
| `settle_phase` | `walker.rs` | 新增（吸收 `step_phase_log` + `step_initialize_skipped_phase_log`） | `async fn settle_phase(tools: &Arc<dyn ToolStepPort>, guard: &RunGuard, request: &RunRequest, cursor: &mut PhaseCursor, phase: &str, settle: PhaseSettle<'_>) -> Result<u32, Terminal>` | 单入口三形态 → `run_phase_log`（单事务含清位）→ 回写 `cursor.failed`；零 emit；落账 `Err(terminal)` → 保留其 `status`（`Stopped` 不降级为 `Failed`）而重写 `reason` 为「形态前缀 + 原 `reason`」（`Pass` → 「相位 <phase> 全部步通过但落账失败: <原因>」、`Fail` / `Skipped` 见 D12；`Terminal` 为同模块私有结构，原地重组即得，零签名变动）。前缀后的「原因」内含 `run_tool` 的 label（形如 `phase-log 失败: <store msg>`）——该嵌套是可接受的（保留工具步归因），spec 只约束前缀；返回落账 attempt |
| `run_worker` | `walker.rs` | 修改（+`SettleMode` 形参、返回 `WorkerTurn`） | `async fn run_worker(worker: &Arc<dyn WorkerAgentPort>, guard: &RunGuard, request: &RunRequest, root: &str, phase: &str, attempt: u32, role: WorkerRole, prompt: String, model_level: ModelLevel, continue_session: Option<String>, trace: &mut EdgeTrace, incoming: EdgeKind, mode: SettleMode) -> Result<WorkerTurn, Terminal>` | `Immediate`：既有语义逐字不变（running → 终态 → 非 `Completed` 即 `Err`）；`Deferred`：只发 running 拍，终态由调用方经 D5 的两个纯函数 emit；`worker.run` 的 `Err` 臂两模式下都发 failed 节点并 `Err` |
| `worker_settle` | `walker.rs` | 新增 | `fn worker_settle(outcome: &WorkerTurnOutcome) -> (StepSettle, Option<Terminal>)` | 会话状态面分类单点（`Completed` → `passed(None)` 无终态 / `Failed` → `failed(会话失败收敛[: 因])` + `Err(failed)` / `Stopped` → `stopped` + `Err(stopped)` / `Running` → `failed(会话收口缺终态)` + `Err(failed)`） |
| `worker_tokens` | `walker.rs` | 新增 | `fn worker_tokens(outcome: &WorkerTurnOutcome) -> (Option<u64>, Option<u64>)` | `sum_usage_tokens` 两键求和单点（`input_tokens` / `output_tokens`；缺席 → `None`） |
| `open_step` | `walker.rs` | 重命名（原 `begin_archive_node`） | `fn open_step(ctx: &StepContext<'_>, trace: &mut EdgeTrace, cursor: &PhaseCursor, kind: ChangeStepKind) -> u32` | `begin_node(Forward)` + running emit；语义零变 |
| `close_step` | `walker.rs` | 重命名（原 `settle_archive_node`） | `fn close_step(ctx: &StepContext<'_>, cursor: &PhaseCursor, kind: ChangeStepKind, instance_no: u32, settle: StepSettle)` | 单终态 emit（同实例身份键就地合并）；语义零变 |
| `test_execution_fail` | `walker.rs` | 新增 | `fn test_execution_fail(outcome: &TestExecutionOutcome) -> PhaseFail<'static>` | 原 `step_test_execution_upgraded_phase_log` 的 `item` / `report`（含 `truncate_report` 与空明细兜底）/ `evidence` 装配逐字搬入，`executor_session = None` |
| `mutation_test_fail` | `walker.rs` | 新增 | `fn mutation_test_fail(outcome: &MutationTestOutcome) -> PhaseFail<'static>` | 原 `step_mutation_test_upgraded_phase_log` 的同款装配（`feedback_text` / `mutation_report_refs` / `diagnose_brief`）逐字搬入，`executor_session = None` |
| `step_gate_fail_phase_log` | `walker.rs` | 修改（去 `trace` / `attempt` 形参） | `async fn step_gate_fail_phase_log(tools: &Arc<dyn ToolStepPort>, guard: &RunGuard, request: &RunRequest, cursor: &mut PhaseCursor, phase: &str, fail: GateFailLog<'_>) -> Result<(), Terminal>` | 体内只装配 `PhaseFail { item: fail.item, report: fail.reason.to_owned(), evidence: fail.diagnostics.to_owned(), executor_session: cursor.executor_session.clone() }` 并调 `settle_phase(PhaseSettle::Fail(..))`；原两处 emit 删除 |
| `step_evaluator` | `walker.rs` | 修改（签名零变） | `async fn step_evaluator(ctx: &StepContext<'_>, trace: &mut EdgeTrace, cursor: &mut PhaseCursor) -> Result<(), Terminal>` | `Deferred` 会话 → `worker_settle` / `worker_tokens` → 按 `finalized` 分派；合规写 `cursor.evaluator_turn` |
| `decision_session` | `walker.rs` | 修改（签名零变） | `async fn decision_session(worker: &Arc<dyn WorkerAgentPort>, tools: &Arc<dyn ToolStepPort>, guard: &RunGuard, request: &RunRequest, input: &DecisionInput, trace: &mut EdgeTrace) -> Result<DecisionAction, Terminal>` | 删 `WhitelistGate`；每轮 decision 节点同一实例收口（`passed` + 动作摘要 / `failed` + 越权文案）；`ask` 轮 passed + 问答摘要后开下一轮（`instance_no` 递增） |
| `begin_node` | `walker.rs` | 修改（签名零变） | `fn begin_node(guard: &RunGuard, trace: &mut EdgeTrace, phase: &str, attempt: u32, step: ChangeStepKind, incoming: EdgeKind) -> u32` | 落边时一次性消费 `trace.entry_override`（有覆盖值用覆盖值） |
| `resolve_change_name` | `steps.rs` | 新增（私有） | `fn resolve_change_name(store: &dyn ChangeStateStore, change_id: &str, what: &str) -> Result<String, String>` | 四门禁臂的 id→name 解析与「未建档」文案合流单点 |

**退役的内部面（含理由）**：`step_phase_start`（开相改 `enter_phase`，节点面消失）／`step_phase_log`（落账改 `settle_phase(Pass)`）／`step_initialize_skipped_phase_log`（legacy 短路改 `settle_phase(Skipped)`）／`step_verdict_gate`（裁决内化进 `step_evaluator`）／`step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`（合入 `PhaseSettle::Fail` + 两个具名装配函数）／`PhaseCursor::{entry_kind, eval_outcome, verdict}` 三字段（分别由 `EdgeTrace.entry_override`、`evaluator_turn` 承载）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `PhaseSettle<'a>`（enum） | `walker.rs` | 新增 | `{ Pass, Fail(PhaseFail<'a>), Skipped(&'a str) }`；相位收口的单入口三形态（私有） |
| `PhaseFail<'a>`（struct） | `walker.rs` | 新增 | `{ item: &'a str, report: String, evidence: String, executor_session: Option<String> }`；`item` 为 checklist 条目名（`静态检查` / `架构检查` / `测试执行` / `突变测试`），`report` ≤ 写面 `MAX_REPORT_CHARS`（装配侧截断），`evidence` 为诊断摘要 |
| `EvaluatorTurn`（struct） | `walker.rs` | 新增 | `{ outcome: WorkerTurnOutcome, checklist: EvaluatorChecklist }`——会话结果与收口载荷同源，消除「有 `eval_outcome` 无 `verdict`」的非法可达态（私有） |
| `SettleMode`（enum） | `walker.rs` | 新增 | `{ Immediate, Deferred }`（`pub(crate)`，见 D4）；`Deferred` = 只发 running 拍 |
| `WorkerTurn`（struct） | `walker.rs` | 新增 | `{ outcome: WorkerTurnOutcome, instance_no: u32 }`——`Deferred` 调用方据 `instance_no` 在同实例上补终态拍（私有） |
| `PhaseCursor`（struct） | `walker.rs` | 修改 | 删 `entry_kind` / `eval_outcome` / `verdict`；增 `evaluator_turn: Option<EvaluatorTurn>`；`PhaseCursor::new(next, sync_specs)` 两个形参（`entry_kind` 形参退场）；13 → 11 字段 |
| `EdgeTrace`（struct） | `walker.rs` | 修改 | 增 `entry_override: Option<EdgeKind>`（相位入口边一次性覆盖值，见 D3 / D9） |
| `GateFailLog<'a>`（struct） | `walker.rs` | 不改 | 三字段（`item` / `reason` / `diagnostics`）逐字保留（spec Module Contract 明写保留为载荷构造辅助） |
| `StepNode` / `RunStepKind` / `ChangeStepKind`（enum） | `core/workflow/src/state.rs` / `core/orchestration/src/state.rs` | 修改（仅注释） | 22 值保持、序不变；五词标 `decode-only` |
| `PhaseCursor` 之外的持久化类型 | `crates/infra/store/**` | **不改** | `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` / `RunNodeRecord` / `RunEdgeRecord` 形状与词面零变化 |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `string`（semver） | `0.6.35` → 落地时 `HEAD` 现值 +1 patch | 版本纪律项；`crates/core/config/config.schema.json` 零改动（无 `KNOWN_KEYS` / `TestFramework` 变更） |

---

## 数据模型

### 持久化面：零变更（词汇冻结 + 零迁移）

| 面 | 现状 | 本变更 |
|----|------|--------|
| `RunNodeRecord.step` | 22 值，bincode 按下标编码 | **零改动**（变体 MUST NOT 删除 / 重排；五词转 `decode-only`） |
| `StepRecord.step_kind` | 9 值（`phase_next` / `phase_start` / `phase_log` / `backtrack` / `decision_log` / 三个 runner / `archi_check`） | **零改动**；`phase_start` / `phase_log` 新写面零产出（walker 不再发这两条 `ToolCommand`），词面保留解码 |
| `PhaseRecord` / `ChecklistItemRecord` | `item` / `pass` / `evidence` 三字段 | **零改动**；`settle_phase` 产出的形状与既有落账逐字一致 |
| `active_phase` | 开相写入（phase / attempt / start_at），落账清位 | **零改动**；`enter_phase` 沿用 `start_phase` 写面 |
| attempt 推导 | `store` 内该 `(change_id, phase)` 的 PhaseRecord 全量条数（含 stale）+ 1 | **零改动**；`enter_phase` 不引入第二份推导（walker 无 store 句柄），`round`（窗口口径）与 attempt 两口径 MUST NOT 合并 |

### 相位机内存模型（本变更的「数据模型」主体）

```
drive()
  ├─ step_phase_next ─► done / error(决策分叉) / next_phase + steps
  ├─ stop_before 停等门（archive 前）
  ├─ enter_phase(phase) -> attempt                     ← 零 emit；写 active_phase
  ├─ [legacy initialize 短路] settle_phase(Skipped(跳过因))   ← 零步执行
  ├─ trace.entry_override = Some(entry_kind)           ← 首个真实步节点消费
  ├─ for step in next.steps { run_step(..) }           ← 只剩真实步（含 ArchiCheck）
  │     Proceed  → 继续
  │     Upgraded → break（fail 已由 settle_phase(Fail) 落账）
  │     Err      → Terminal（红节点路径）
  ├─ settle_phase(Pass)                                ← 三分流 → 落账 → 清位 → 回写 failed
  └─ 相位间停等（auto_next_phase=false 且非 stop_gate）
```

| 槽位 | 类型 | 生产者 | 消费者 |
|------|------|--------|--------|
| `cursor.evaluator_turn` | `Option<EvaluatorTurn>` | `step_evaluator`（同源携 outcome + checklist） | `settle_phase(Pass)`（`checklist` 落账 + `outcome.session_id` 填槽） |
| `cursor.test_outcome` / `mutation_outcome` | `Option<TestExecutionOutcome>` / `Option<MutationTestOutcome>` | `step_test_execution*` / `step_mutation_test*` | `settle_phase(Pass)` 的机械 checklist 分支（双槽） |
| `cursor.executor_session` | `Option<String>` | `step_executor` / 反馈边修复轮 | `settle_phase(Pass)`（executor 槽位）与 `PhaseFail` 装配 |
| `cursor.failed` | `bool` | `settle_phase` 单点回写（D7） | `drive` 的 `trace.failed_phase` |
| `trace.entry_override` | `Option<EdgeKind>` | `drive`（相位进入时） | `begin_node`（首个节点落边时一次性消费） |
| `trace.tail` / `trace.incoming` / `trace.failed_phase` | 既有 | 既有 | 既有（`Retry` 边判定为存量解码面，新写面不可达） |

### 关系与可见性（D4 的落点）

| 事件 | 图面承载 | 本变更 |
|------|----------|--------|
| 相位开相 | 原 `phase_start` 节点 | **零节点**（`active_phase` 在案，列表 / 详情读该字段） |
| verdict 裁决 | 原 `verdict_gate` 节点 | **evaluator 节点末实例**的终态 + `detail=verdict=…`；缺席即 failed + `Terminal` |
| 相位落账 | 原 `phase_log` 节点 | **零节点**（`PhaseRecord` + `pipeline.attempts` 已可见）；落账失败 → 节点全绿 + run failed + reason 前缀 |
| 决策动作裁决 | 原 `whitelist_gate` / `retry_gate` 节点 | **decision 节点实例**（`instance_no` 按轮递增）+ `detail`（`backtrack → x` / `stop：…` / `ask：…` / 越权文案携 `allowed`） |
| 相位入口 | 原 `phase_start` 节点的 incoming 边 | 该相位**首个真实步节点**的 incoming 边（`backtrack` / `forward`） |

<!-- 无路由 / API 设计：本变更零 method / path / IPC 命令演进。`ChangeStepKind` 值集不变 ⇒ `bindings:export` 再生成应零 diff；前端 `src/types/generated/**` 零改动。 -->

---

## 依赖

- **运行时依赖**：**零新增**。`walker.rs` 只用既有的 `agent`（`sum_usage_tokens` / `AgentRunStatus` / `ModelLevel`）、`workflow::{model, queries, write, state}`、`foundation::{layout, prune}`；`steps.rs` 只用既有 `workflow::{queries, state, write}`。`crates/core/workflow/Cargo.toml` 与 `crates/core/orchestration/Cargo.toml` **零 diff**。
- **依赖方向零变化**：`orchestration → workflow`（既有）／`orchestration → agent`（既有）／`orchestration → foundation`（既有）；`core` 侧不新增任何 `infra` 引用。
- **构建 / 测试依赖**：**零新增**（测试侧沿用既有 `tokio` / `serde_json` 夹具；本相位不编写用例）。
- **零 feature 开关变化**；零新 crate / 零新架构元素 / 零新依赖边。

---

## 验收标准落点

> 只给**实现侧落点**；用例编写与门禁执行由桌面端测试相位承接（见 tasks「交接」条目）。

| AC | 落点 |
|----|------|
| AC-1 辅助步全退场 | `phase_table.rs` 的 11 张 `*_STEPS` 内 `PhaseStart` / `VerdictGate` / `PhaseLog` 零声明（现盘 30 处）；逐表步数见「修改文件」行 |
| AC-2 相位入口内化 | `enter_phase`（唯一开相路径，零 `emit_node` / 零 `begin_node` / 零 `StepRecord`）+ `drive` 的 legacy 短路改 `enter_phase` + `settle_phase(Skipped)`；`steps.rs` 的 `PhaseStart` / `PhaseLog` 分支删 `audit_outcome` 调用（命令面仍在，审计面归零） |
| AC-3 相位收口内化 | `settle_phase` 三形态 + `Pass` 三分流（`test_outcome` 双槽 / `evaluator_turn` checklist / 空 checklist）；`Pass` 落账 `Err` → `Terminal::failed("相位 <phase> 全部步通过但落账失败: <err>")`，节点侧零 emit（全绿）；`Fail` / `Skipped` 形态落账失败按 D12 分文案 |
| AC-4 verdict 裁决内化 | `step_evaluator` 读 `outcome.finalized`：`Some(Evaluator(..))` → 同实例 passed（`detail=verdict=…`）+ `cursor.evaluator_turn`；缺席 → 同实例 failed + `Terminal::failed`（`truncated()` 判据分「输出被截断」/「未调用 finish_step 收口」两文案）；零 `feedback` 边、零 `verdict_gate` 节点。**相位内系统异常仍标 failed（D5 第二半）**：`Deferred` 路径先过 `worker_settle` 的 `terminal` 判定，`Failed` / `Stopped` / `Running` 三态照旧落红节点（或停止节点）+ `Terminal`，MUST NOT 被业务分支吞成 passed |
| AC-5 决策节点承载裁决 | `decision_session` 每轮同实例收口：`passed` + `backtrack → <to>` / `stop：<reason>` / `ask：<question>`，越权 → `failed` + 拒因（携 `allowed`）+ `Terminal::failed`；`WhitelistGate` / `RetryGate` 的 `begin_node` 调用点归零（`resolve_deadlock` 三次 emit 删除）；`ensure_backtrack_allowed` 逐字不动（纯函数） |
| AC-6 词汇冻结 | `workflow/state.rs` 与 `orchestration/state.rs` 仅注释：三表 22 值与序不变、五词 `decode-only`、`StepGroup::Gate` 分支保留；`bindings:export` 零 diff（`ChangeStepKind` 值集不变） |
| AC-7 归档链 9 步 | `ARCHIVE_STEPS` 九步序 + `drive` 的开相 / 收口在 `steps` 之外；archive 相位节点恰 9 个（`open_step` 族不变） |
| AC-8 计划图漂移显式失败 | `run_step` 的六词不可达臂（D8 文案）；`match` 仍穷尽 22 变体（`cargo check` 保证） |
| AC-9 行为面零回归 | `ToolCommand` 载荷断言面零变（`PhaseLogInput` / `DecisionLog` / 三个 runner 命令逐字不动）；七条机械 checklist 条目文案分散在**两处**：`PhaseFail` 的四个 `item`（静态检查 / 架构检查 / 测试执行 / 突变测试）与 `settle_phase` 机械分支的三条（suite 结论一致 / 聚合计数一致 / mutation null 自动通过），全部经相位收口落账、不经任何门步 |
| AC-10 前端零改动可解存量 | 后端零出线变化 ⇒ `packages/desktop/src/**` 零 diff（`run-state.ts` 的五键解码与 gate 分组保留为旧库行必需，本变更不触碰） |
| AC-11 golden 零 diff | 实现侧前置：节点词汇、边四值、身份键与序零变化 ⇒ 重写差异应只来自步数 / 边数收缩（重写与复核由测试相位承接） |
| AC-12 语料双口径 | 实现侧前置：五词仍可落 `RunNodeRecord`（变体冻结、映射表零改）⇒ 存量解码样本与真实相位形态样本可并存构造（样本集落地由测试相位承接） |

### 明确不做（知情边界，非遗漏）

- `StepSpec` 静态表 / 14 处「passed + 跳过文案」数据化 / `PhaseCursor` 拆窄 struct / `run_step` 表驱动或 trait 化 → `step-abstraction-model` 课题。
- `StepGroup::Gate` 与三张词汇表的死变体清理（需 db 迁移或一次性数据重写）→ 另立变更。
- `feedback_limit(step, label)` 的 label 分支改造（基线已收缩为 static-check / archi-check 两个调用点）。
- 前端任何渲染改动（`phaseStart` / `phaseLog` / 三门 gate 的解码与静默节点臂保留）。
- 架构模型改动：零新 crate、零新元素、零新依赖边（`.dev-team/architecture/models/**` 零 diff）。

---

## 待决问题

1. **`EvaluatorTurn` 是否该收窄**：spec 定「会话结果与收口载荷同源」，故 `outcome: WorkerTurnOutcome` 整包入槽（含 `transcript`）。生产上单相位最多驻留一条 evaluator 转录（决策会话在错误分支，不同时在位），内存代价可接受。若后续评估相位出现「多 evaluator 实例同相位」形态（如 evaluator 反馈边复活），应把该槽收窄为 `{ session_id, checklist }`——届时须同步 spec。
2. **decision 节点 detail 的文案格式是否为稳定契约**：本设计定 `backtrack → <phase>` / `stop：<reason>` / `ask：<question 摘要>`（全角冒号），越权写 `diagnose_brief` 截断后的拒因。前端零改动即显示（detail 面既有渲染），故文案并非渲染契约；但若后续决策信息面升级为结构化面（`RunUpdate::Decision` 之类），需重新评估该 detail 的双重身份。
3. **`settle_phase` 的返回值 `attempt` 目前零消费者**：保留是为对齐 spec 的 `Result<u32, Terminal>` 形态（落账 attempt 是唯一可观测的落账结果）。若后续观测面需要「本次落账 attempt」进入 run 收口摘要，可就地接线；本期不引入。
4. **D12（`Fail` / `Skipped` 落账失败的独立前缀）需在 spec 合并时补一句或多句**：本 delta 的 ADDED requirement 只写了 `Pass` 形态（「相位内全部步皆 passed … 固定前缀」），未覆盖另两形态的落账失败文案。实现按 D12 分文案后，spec 的该段应补「另两形态落账失败以各自前缀点明成因，不得复用『全部步通过』表述」——否则合入后 spec 与本实现的措辞不对齐（不构成行为冲突，但会在下次评审时需要口头解释）。
5. **D11（审计归零点在 `steps.rs` 而非 walker）需在 spec 合并时校正一句**：本 delta 的 `desktop-change-orchestration` Module Contract 表里 `steps.rs` 一行写「`ToolCommand::PhaseStart` / `PhaseLog` 不再有 walker 调用点，故不再产 `StepKind::PhaseStart` / `PhaseLog` 审计行」——事实是调用点仍在（`enter_phase` / `run_phase_log`），归零靠删 `audit_outcome` 调用。合入时应把该句改为「两分支的 `audit_outcome` 调用删除，故不再产这两类审计行」。
