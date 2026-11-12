# 任务: slim-flow-aux-steps

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 6 项（`walker.rs` / `steps.rs` / `phase_table.rs` / `workflow/state.rs` / `orchestration/state.rs` / `package.json`）。**新增文件为 0**（设计已定：全部落既有文件既有函数面）。
>
> **本相位不写用例、不改断言、不执行测试回归**：`proposal.md`「测试文件」表逐行（形状面断言改写 ~40 处、6 处 `walk_run` 整链期望值重定、新增 4 类守卫、golden 重写）归 test-design / test-gen / test-execution 相位。本相位对测试文件的唯一动作是**让工作区随语义变化编译通过**（`run_worker` 新形参 / `PhaseCursor` 字段置换 / 删函数后遗留调用点），MUST NOT 新增用例、MUST NOT 改断言口径、MUST NOT 把用例绿灯或 golden 通过写成完成条件。
>
> **依赖序（五阶段）**：
>
> - **阶段一**（`core/workflow` 计划图收窄 + 三张词汇表注释）——只改数据与注释，`orchestration` **编译**通过但**运行不可用**（辅助步声明已删而开相 / 落账动作尚未落地）。它与阶段二 MUST **同一提交**，见阶段一的红线说明。
> - **阶段二**（`walker.rs` 的相位机改造）——**一个编译闭合批**（`run_worker` 签名 / `PhaseCursor` 字段 / 删函数三者互相强制，拆开即工作区不可编译）。内部分 10 条（2.1–2.10），按依赖先后排。与阶段三构成「本变更可运行」的最小完整集。
> - **阶段三**（`steps.rs` 的 id→name 合流 + 开相 / 落账审计面归零）——与阶段二无编译耦合，但**审计面归零是 AC-2 的必要条件**，故与阶段一 / 二同批交付。
> - **阶段四**（注释与口径同步复核）——在代码形状定稿后一次收口（相位机形态、attempt 推导、瞬步表述）。
> - **阶段五**（版本 + 边界自查 + 交接备忘）——依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ① **词汇三表冻结**：`StepNode` / `RunStepKind` / `ChangeStepKind` 的 22 值与变体序 MUST NOT 增删重排（`RunNodeRecord.step` 经 bincode 按下标编码）；`StepGroup::Gate` 枚举值与 `group()` 的三词分支 MUST NOT 删除。
> ② **前端零 diff**：`packages/desktop/src/**` 全树零改动——`run-state.ts` 的五键（`phaseStart` / `phaseLog` / `verdictGate` / `retryGate` / `whitelistGate`）解码与 gate 分组是旧库行解码必需。
> ③ **store 零 diff**：`crates/infra/store/**` 全树零改动；attempt 推导式子与 `active_phase` 写面逐字不动；`phase_log` 的 `skipped` 约束 / report 上限 / 单事务落库逐字不动。
> ④ **写面通道保留、审计面归零**：`ToolCommand::{PhaseStart, PhaseLog}` 命令变体、`steps.rs` 的写面直调与 `run_phase_log` **保留**（`enter_phase` / `settle_phase` 仍经它们走写通道）；但这两分支的 `audit_outcome` 调用 MUST 删除——否则 `StepKind::PhaseStart` / `PhaseLog` 审计行照常产出，违反 AC-2「零 `StepRecord`」（见 design D11）。
> ⑤ **双失败面文案可辨**（AC-3 / AC-4 / R2）：`Pass` 形态落账失败一律 `相位 <phase> 全部步通过但落账失败: <err>` 前缀（唯一携「全部步通过」的形态）；`Fail` / `Skipped` 形态落账失败按 design D12 分文案（不得照抄「全部步通过」——那会与红节点 / 零步执行矛盾）；步内系统异常走既有「会话执行失败 / 会话失败收敛 / 会话被终止」文案，MUST NOT 借用落账前缀。
> ⑥ **门禁分工**：`pnpm -C packages/desktop run check` 整链、`cargo test` / `cargo nextest`、`vp test run`、golden 重写与复核**全由桌面端 static-check / test-execution 相位承接**。本文件的自查项只收**源码级静态判据**（`cargo check` 编译闭合、`grep` 零命中、读码结构复核、`git diff` 边界比对、版本行相对 `HEAD` +1 patch）；凡测试回归与整链门运行一律写成**「交接 …（由桌面端 <相位> 承接）」**条目，MUST NOT 作为任何阶段的完成判据。

## 阶段一：计划图收窄 + 三张词汇表注释（`core/workflow`；与阶段二同一提交）

> 本阶段结束时 `phase_table` 的辅助步声明归零、步数落到 29 / 23，而 `orchestration` 仍编译（`run_step` 的辅助步臂成为不可达臂，`step_phase_start` 等函数仍有调用点故无 `dead_code`）。这也是阶段二「首节点即真实步」的前提。
>
> **阶段一与阶段二 MUST 同一提交（运行期不可用窗口）**：辅助步声明一删，`step_phase_start` / `step_phase_log` 即失去全部调用点，于是开相与落账整条链路不存在——`active_phase` 永不写入、`phase_log` 的「开相才可落账」前置会直接 `Err`，run 在第一个相位即失败。阶段一只是**编辑顺序**上的准备步（先让计划图变成目标形态，再按新形态改 walker），不是可独立发布的单元。因此：
>
> - **MUST NOT** 为让中间态可运行而保留部分辅助步声明或临时补一个「开相 / 落账」的过渡实现（会形成两份落账 / 两套开相）；
> - 两阶段之间的校验手段只有 `cargo check`（编译闭合）；行为校验由阶段五移交的测试相位在**完整变更**上执行。

- [x] `crates/core/workflow/src/write/phase_table.rs`：`INITIALIZE_STEPS` 删首尾两项（`PhaseStart` / `PhaseLog`）→ 3 步 `Clean → StaticCheck → TestExecution`；doc 注释的步序串同步改「开相与相位收口不在 `steps` 内（`enter_phase` / `settle_phase` 承载）」
- [x] 同文件 `PROPOSAL_STEPS` / `DEV_DESIGN_STEPS` / `TEST_DESIGN_STEPS` 各删首尾两项 → 2 步 `Executor → Evaluator`（删 `VerdictGate` 项及其 `OnFail::FeedbackLoop { limit: FEEDBACK_LOOP_LIMIT }` 声明）
- [x] 同文件 `CODE_ANALYZE_STEPS` 同款 → 2 步 `Executor → Evaluator`（原 `VerdictGate` 的 `OnFail::Terminal` 声明随之消失）
- [x] 同文件 `IMPLEMENT_STEPS` 删首尾两项 → 4 步 `Executor → StaticCheck(FeedbackLoop{5}) → ArchiCheck(FeedbackLoop{ARCHI_FEEDBACK_LIMIT}) → Evaluator`（`ArchiCheck` 的座位、`on_fail` 值逐字不动）
- [x] 同文件 `TEST_GEN_STEPS` 删首尾两项 → 3 步 `Executor → StaticCheck(FeedbackLoop{5}) → Evaluator`
- [x] 同文件 `TEST_EXECUTION_STEPS` 删首尾两项 → 2 步 `TestExecution(Escalate) → MutationTest(Escalate)`；doc 注释步序串改「开相与相位收口由相位机固定动作承载，不计入 `steps`」
- [x] 同文件 `ARCHIVE_STEPS` 删首尾两项 → 9 步 `Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize`；doc 注释「11 步」改「9 步」并注明开相 / 落账移出
- [x] 同文件 `CODE_REVIEW_STEPS` / `ACCEPTANCE_STEPS` 各删首尾两项 → 1 步 `Evaluator`
- [x] 同文件 `IMPLEMENT_STEPS` 中「独立于 static-check 的 5 次与 verdict 的 5 次」与 `ARCHI_FEEDBACK_LIMIT` doc 的「与 static-check / verdict 的 5 次互不占用」：去掉 `verdict` 计数项（该反馈环已随 `finish_step` 退役）→「与 static-check 的 5 次互不占用」
- [x] `crates/core/workflow/src/state.rs`（**仅注释**）：`StepNode::PhaseStart` / `PhaseLog` / `VerdictGate` 三变体标 `decode-only`（计划图新表零引用、存量执行图节点行解码必需）；`RunStepKind` 的 `PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` 五变体标 `decode-only` + 「bincode 下标钉死，MUST NOT 删除或重排」；`StepGroup::Gate` 的枚举 doc 与 `group()` 的三词分支加「辅助步退场后无 `StepNode` 归属，词汇面仍冻结、分支不删」；`StepGroup::Exec` 的 doc 去掉 `phase-start` / `phase-log` 举例
- [x] `crates/core/orchestration/src/state.rs`（**仅注释**）：`ChangeStepKind` 五变体（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate`）标 `decode-only`（出线词与节点身份键的存量行依赖，新写面零产出）；枚举 doc 的步词汇分族表述改「新写面只产 WorkerAgent / ToolStep / Git 三类节点，Gate 类为零产出的存量解码词汇」
- [x] 自查 阶段一（源码静态判据）：`grep -c 'StepNode::PhaseStart\|StepNode::VerdictGate\|StepNode::PhaseLog' crates/core/workflow/src/write/phase_table.rs` 为 **0**；逐表步数读码复核 29（requirement：3/2/2/2/4/3/2/1/1/9）与 23（test-only：3/2/2/2/3/2/9）；`git diff -- crates/core/workflow/src/state.rs crates/core/orchestration/src/state.rs` 只含注释行（零 `match` 臂 / 零变体增删）；`cargo check --manifest-path packages/desktop/src-tauri/Cargo.toml` 通过（**仅编译**——本阶段运行期不可用，行为校验归阶段五移交的测试相位）
- [x] 交接 阶段一（测试回归，本相位不执行）：`phase_table_test` 的六处序表常量（`INITIALIZE_STEP_ORDER` 5→3、`CODE_ANALYZE_STEP_ORDER` 5→2、`ARCHIVE_STEP_ORDER` 11→9）与各相位步数断言、`state_test`（两 crate）的词汇冻结守卫用例，**由桌面端 test-design / test-gen / test-execution 相位承接**。**交付物 = 期望登记与移交**

## 阶段二：相位机改造（`core/orchestration/src/walker.rs`，一个编译闭合批）

> 本阶段 10 条互相强制（`run_worker` 签名、`PhaseCursor` 字段、删函数后调用点），按依赖序落地；期间工作区可能短暂不可编译，**不得**为保证每步可编译而保留双份实现（两份落账 / 两套裁决是本次改造最忌讳的中间态）。

- [x] **2.1 会话收口面**（先做，其后所有调用点才有 `SettleMode`）：
  - 新增 `pub(crate) enum SettleMode { Immediate, Deferred }`（D4；`Deferred` = 只发 running 拍）
  - 新增私有 `struct WorkerTurn { outcome: WorkerTurnOutcome, instance_no: u32 }`
  - 新增 `fn worker_settle(outcome: &WorkerTurnOutcome) -> (StepSettle, Option<Terminal>)`：从现 `run_worker` 的 `match outcome.status` 四臂**逐字搬入**（`Completed` → `passed(None)`+无终态；`Failed` → `failed(会话失败收敛[: <failure_reason 摘要>])` + `Err(failed)`；`Stopped` → `stopped(会话被终止)` + `Err(stopped)`；`Running` → `failed(会话收口缺终态)` + `Err(failed)`）
  - 新增 `fn worker_tokens(outcome: &WorkerTurnOutcome) -> (Option<u64>, Option<u64>)`：`sum_usage_tokens(&outcome.transcript, "input_tokens"/"output_tokens")` 两值单点
  - `run_worker` 加末位形参 `mode: SettleMode`，返回 `Result<WorkerTurn, Terminal>`：`Immediate` 走 `worker_settle` + `emit_node`（终态拍与 Deferred 调用方共用同一组装单点 `with_turn(settle, &outcome)`，即 `.with_session(..).with_tokens(..)` 的唯一书写处）后按终态返 `Err`／`Ok`（**与现状逐字等价**）；`Deferred` 只在 `worker.run` 的 `Err` 臂发 failed 节点并 `Err`，成功路径**零终态 emit**、返回 `WorkerTurn`（调用方负责终态）
  - 7 个调用点补实参：`step_executor` / `step_static_check`（反馈边修复轮）/ `step_archi_check`（反馈边修复轮）/ `step_resolve_conflicts` / `step_spec_sync` 传 `SettleMode::Immediate` 并取 `.outcome`；`step_evaluator` / `decision_session` 传 `SettleMode::Deferred`（此两处随 2.8 / 2.10 接线）

- [x] **2.2 `PhaseCursor` 与 `EvaluatorTurn`**：新增私有 `struct EvaluatorTurn { outcome: WorkerTurnOutcome, checklist: EvaluatorChecklist }`；`PhaseCursor` 删 `entry_kind` / `eval_outcome` / `verdict` 三字段、增 `evaluator_turn: Option<EvaluatorTurn>`（13 → 11 字段）；`PhaseCursor::new(next: &PhaseNextOutcome, sync_specs: bool)` 去掉 `entry_kind` 形参并同步 `drive` 的构造点

- [x] **2.3 相位入口与收口固定动作**：
  - 新增 `async fn enter_phase(tools: &Arc<dyn ToolStepPort>, guard: &RunGuard, request: &RunRequest, phase: &str) -> Result<u32, Terminal>`：体 = 现 `step_phase_start` 的 `run_tool(.. ToolCommand::PhaseStart ..)` 部分**逐字搬入**（label `"phase-start"`），**保留 `Ok` / `Err` 两臂的 `Err` 直通**、删掉两处 `begin_node` + `emit_node`
  - 新增私有 `enum PhaseSettle<'a> { Pass, Fail(PhaseFail<'a>), Skipped(&'a str) }` 与 `struct PhaseFail<'a> { item: &'a str, report: String, evidence: String, executor_session: Option<String> }`
  - 新增 `async fn settle_phase(tools: &Arc<dyn ToolStepPort>, guard: &RunGuard, request: &RunRequest, cursor: &mut PhaseCursor, phase: &str, settle: PhaseSettle<'_>) -> Result<u32, Terminal>`：先按形态组装 `PhaseLogInput` **并记下形态**（`Pass` / `Fail` / `Skipped`），再 `run_phase_log`；`Err(terminal)` → **保留 `terminal.status`、按形态重写 `terminal.reason`**（D12）：`Pass` → 「相位 {phase} 全部步通过但落账失败: {原 reason}」、`Fail(..)` → 「相位 {phase} 门禁失败落账失败: {原 reason}」、`Skipped(..)` → 「相位 {phase} 跳过落账失败: {原 reason}」（`Terminal` 为同模块私有结构，原地重组即得——`run_phase_log` 与 `run_tool` 的签名 / label **逐字不动**）；成功后回写 `cursor.failed`（D7：`Pass` → `checklist.verdict == Verdict::Fail`；`Fail(..)` → `true`；`Skipped(..)` → `false`），返回 `outcome.attempt`
  - `Pass` 三分流**逐字搬入**现 `step_phase_log`：① `cursor.test_outcome.take()` 在场 → `test_execution_phase_log_input(phase, &outcome, cursor.mutation_outcome.take().as_ref())`；② 否则 `cursor.evaluator_turn` 在场 → 以该 turn 的 `checklist`（`report` / `checklist` 字段移入 `PhaseLogInput`，`evaluator_session_id = Some(turn.outcome.session_id)`、`executor_session_id = cursor.executor_session.clone()`）；③ 两者皆无 → `PhaseLogInput { report: String::new(), checklist: vec![], skipped: false, 三槽位 None }`（archive 空 checklist 分支逐字保留，零 `phase == archive` 特判）
  - `Skipped(reason)` → `PhaseLogInput { report: reason.to_owned(), checklist: vec![], skipped: true, 三槽位 None }`（原 `step_initialize_skipped_phase_log` 的载荷逐字搬入）
  - `Fail(PhaseFail { item, report, evidence, executor_session })` → `PhaseLogInput { report, checklist: vec![ChecklistItem { item: item.to_owned(), pass: false, evidence }], skipped: false, executor_session_id: executor_session, evaluator_session_id: None, decision_session_id: None }`（原 `step_gate_fail_phase_log` 的载荷形状逐字搬入）
  - 实现注意：`PhaseFail` 的载荷**先绑到局部变量**再传入 `settle_phase(.., cursor, ..)`（同一表达式内既 `&mut cursor` 又在构造 `PhaseFail` 时读 `cursor.executor_session` 会撞借用检查；局部变量一次解决，MUST NOT 为此把签名改成 `&PhaseCursor`——那会逼出 `PhaseCursor` 的整包 clone，见 D2）

- [x] **2.4 相位入口边承载**（D3 / D9）：`EdgeTrace` 增 `entry_override: Option<EdgeKind>`（`Default` 派生即 `None`）；`begin_node` 的落边分支先把 `trace.entry_override.take()` 与传入 `incoming` 合成（`Some(override_kind) => override_kind, None => incoming`）再 `emit_edge`；`drive` 在确认走步序列后（非 legacy 短路分支）置 `trace.entry_override = Some(entry_kind)`，且在任何 `continue` / `return` 之前兜底 `trace.entry_override = None`

- [x] **2.5 `drive` 改造**：单相位序列改为 `enter_phase(ctx.tools, ctx.guard, ctx.request, phase) → for step in &next.steps { run_step(..) } → settle_phase(ctx.tools, ctx.guard, ctx.request, &mut cursor, phase, PhaseSettle::Pass)`：
  - `next.steps.is_empty()` → `Terminal::failed` 判据**保留**（文案去掉「相位定义恒含 PhaseStart 起步」改为「相位定义恒含至少一个真实步」）
  - `if phase == INITIALIZE_PHASE_ID && request.root == request.main_root` 短路分支改：`cursor.attempt = enter_phase(..)await?` → `settle_phase(PhaseSettle::Skipped(TXT_SKIP_LEGACY_INITIALIZE))`（零步执行、零 vcs / 零检查 spawn、主仓零 reset 零 clean 语义逐字不变）
  - 正常分支：`cursor.attempt = enter_phase(..)await?` → 步循环（`Proceed` 继续 / `Upgraded` 置 `upgraded = true` 并 `break` / `Err` 直返）→ 非 `upgraded` 时 `settle_phase(PhaseSettle::Pass)`
  - 落账失败文案按形态分（D12）：`Pass` 用「相位 <phase> 全部步通过但落账失败: <err>」（spec 的前缀，唯一带「全部步通过」的形态）；`Fail` / `Skipped` 用各自前缀——`Fail` / `Skipped` 形态在失败时已有红节点或本就零步执行，照抄「全部步通过」会给出事实错误的归因
  - `trace.failed_phase` 两次置位（D10）：步循环后先置一次（覆盖 `Upgraded` 分支），`Pass` 收口后再置一次
  - 其余停止门 / 相位间停等 / `resolve_deadlock` 分叉骨架逐字不动

- [x] **2.6 分派臂收窄与辅助步执行体删除**：
  - `run_step` 的 `PhaseStart` / `VerdictGate` / `PhaseLog` 三臂与原 `Decision | RetryGate | WhitelistGate => Ok(StepFlow::Proceed)` 空臂合并为**单一不可达臂**，返回 `Terminal::failed(format!("计划图漂移：相位 \"{}\" 的 steps 含不可达步词 \"{}\"（该步词不出现在任何相位的 steps 内）", ctx.phase, step.node.as_str()))`（D8）；其余 16 臂逐字不动（含 `ArchiCheck`）
  - 删 `step_phase_start`（职责已入 `enter_phase`）、`step_phase_log` 与 `step_initialize_skipped_phase_log`（职责已入 `settle_phase`）、`step_verdict_gate`（职责入 2.8）
  - `run_step` 的 `Decision` 变体不再与 `RetryGate` / `WhitelistGate` 分家（六词同臂）

- [x] **2.7 `open_step` / `close_step` 重命名**：`begin_archive_node` → `open_step`、`settle_archive_node` → `close_step`（**函数体逐字不动**），12 处 open 调用与 25 处 close 调用（`step_clean` / 归档九步族 / `fail_archive_step` / `fail_archive_node`）随动，语义零变。注意：`step_gate_fail_phase_log` 与两个 `*_upgraded_phase_log` 走的是裸 `begin_node`（不经这层辅助），故 2.9 的删改与之无关

- [x] **2.8 evaluator 内化裁决**：`step_evaluator` 改：`run_worker(.., SettleMode::Deferred)` → 取 `WorkerTurn { outcome, instance_no }` → **先**走会话状态面（D5 ①）：`let (settle, terminal) = worker_settle(&outcome); if let Some(terminal) = terminal { emit_node(.., instance_no, settle.with_session(Some(session)).with_tokens(worker_tokens(&outcome))); return Err(terminal); }`（`Failed` / `Stopped` / `Running` 三态仍落红节点 / 停止节点 + `Terminal`，MUST NOT 被业务分支吞掉）→ **再**走业务面：`match outcome.finalized`：
  - `Some(FinishPayload::Evaluator(checklist))` → `emit_node(.., instance_no, with_turn(StepSettle::passed(Some(format!("verdict={}", checklist.verdict.as_str()))), &outcome))`（同实例单拍终态，detail 携 verdict；会话 id / token 随行由 `with_turn` 单点承载）→ `cursor.evaluator_turn = Some(EvaluatorTurn { outcome, checklist })` → `Ok`
  - `Some(FinishPayload::Decision(_))`（理论不可达）与 `None` → 走同一失败分支：`reason = truncated(&outcome.transcript) ? "评估输出被截断（output_truncated），未产生 finish_step 收口产物" : "未调用 finish_step 收口（相位 evaluator 会话 Completed 但无收口载荷）"` → `emit_node(.., instance_no, with_turn(StepSettle::failed(Some(diagnose_brief(reason))), &outcome))` → `Err(Terminal::failed(reason))`（`Completed` 但无载荷 = 业务失败，MUST NOT 复用 `worker_settle` 的 passed）
  - 删 `cursor.eval_outcome` / `cursor.verdict` 的所有读写点

- [x] **2.9 落账面四处收敛**（D6 / D7）：
  - `step_gate_fail_phase_log` + `GateFailLog` **保留**：形参去 `trace` / `attempt`、加 `cursor: &mut PhaseCursor`；体内只装配 `PhaseFail { item: fail.item, report: fail.reason.to_owned(), evidence: fail.diagnostics.to_owned(), executor_session: cursor.executor_session.clone() }` 并 `settle_phase(PhaseSettle::Fail(..))`；原 `begin_node` / 两次 `emit_node` 删除；两个调用点（`step_static_check` 超限、`step_archi_check` 超限）随动，并删各自的 `cursor.failed = true`（已由 `settle_phase` 回写）
  - 删 `step_test_execution_upgraded_phase_log`，新增 `fn test_execution_fail(outcome: &TestExecutionOutcome) -> PhaseFail<'static>`：`item = "测试执行"`、`report` = `truncate_report(&detail)`（空 `findings_detail` 时回落 `conclusion=..；报告目录 ..`，逐字搬入）、`evidence = format!("{}；报告目录 {}", diagnose_brief(&outcome.findings_brief), outcome.report_dir)`、`executor_session: None`
  - 删 `step_mutation_test_upgraded_phase_log`，新增 `fn mutation_test_fail(outcome: &MutationTestOutcome) -> PhaseFail<'static>`：`item = "突变测试"`、`report` = `truncate_report(&detail)`（空 `feedback_text` 时回落 `conclusion=..；报告路径 ..`）、`evidence = format!("{}；{}", diagnose_brief(&outcome.feedback_text), mutation_report_refs(outcome))`、`executor_session: None`
  - `step_test_execution_escalate` / `step_mutation_test_escalate` 的升格两段改 `settle_phase(PhaseSettle::Fail(test_execution_fail(&outcome)))` / `settle_phase(PhaseSettle::Fail(mutation_test_fail(&outcome)))`，并删各自的 `cursor.failed = true`；节点 side（`failed` 节点的 `detail` / `output_ref`）逐字不动
  - `feedback_limit` / `truncate_report` / `mutation_evidence` / `mutation_report_refs` / `test_execution_phase_log_input` / `diagnose_brief` **逐字不动**

- [x] **2.10 决策与死锁节点面退场**（D5，`decision_session` / `resolve_deadlock`）：
  - `decision_session`：`run_worker(.., SettleMode::Deferred)` → `decision-log` 幂等挂账（顺序不变：**先**挂账，**后**裁决）→ 会话状态面（同 2.8：`worker_settle` 的 `terminal` 非 `None` 即 `emit` + `Err`）→ `match finalized`：`Ask` 臂 → `guard.emit(RunUpdate::Ask)` → `wait_answer`（`None` → `Err(stopped("ask 停等被终止"))`）→ `emit_node(.., instance_no, with_turn(StepSettle::passed(Some(format!("ask：{}", diagnose_brief(&question)))), &outcome))`（用户驱动的停等不计失败）→ `prompt = decision_answer_prompt(&answer)`、`continue_session = Some(session_id)` → `continue`；`None` 臂 → `Err(Terminal::failed(reason))`（截断 / 未收口两文案，同 2.8 口径）；`Some(Evaluator(_))` → `Err(Terminal::failed("决策会话返回 evaluator 收口载荷（相位表角色漂移）"))`
  - 非 `Ask` 动作：删 `WhitelistGate` 的 `begin_node` 与三次 `emit_node`；改 `ensure_backtrack_allowed` 判定后**在同一 decision 节点实例**收口：`Ok` → `emit_node(passed(Some(动作摘要)))`（`Backtrack { to }` → `format!("backtrack → {to}")`、`Stop` → `format!("stop：{reason}")`；`Ask` 臂在此不可达——应答路 `continue` 新一轮、终止路已 `Err`，故不重复「ask：…」文案，改按编排漂移同实例 failed + `Err(Terminal::failed("决策漂移：ask 动作未被应答循环消化"))`（与 `resolve_deadlock` 的同名漂移臂文案一致，且不留 running 节点））；`Err(message)` → `emit_node(failed(Some(diagnose_brief(&message))))` + `Err(Terminal::failed(format!("决策越权：{message}")))`；两种情形均 `with_turn(..)`（会话 id / token 单点组装）
  - `resolve_deadlock`：删三处 `RetryGate` 的 `begin_node` / `emit_node`（含 backtrack 落账前后的 passed 拍）；`trace.incoming = Some(EdgeKind::Backtrack)` 与 `run_tool(Backtrack)` 落账逐字不动；`DecisionAction::Stop` / `Ask` 两臂文案逐字不动
  - `ensure_backtrack_allowed`（`decision.rs`）与 `ToolCommand::DecisionLog` **逐字不动**

- [x] 自查 阶段二（源码静态判据）：`grep -rn 'step_phase_start\|step_phase_log(\|step_initialize_skipped_phase_log\|step_verdict_gate\|step_test_execution_upgraded_phase_log\|step_mutation_test_upgraded_phase_log\|begin_archive_node\|settle_archive_node' --include=*.rs crates/core/orchestration/src` 在**非测试**文件中零命中（`run_phase_log` 属保留面，注意区分 `step_phase_log` 与 `run_phase_log`）；`grep -rn 'ChangeStepKind::WhitelistGate\|ChangeStepKind::RetryGate\|ChangeStepKind::PhaseStart\|ChangeStepKind::PhaseLog\|ChangeStepKind::VerdictGate' crates/core/orchestration/src/walker.rs` 零命中；`grep -c 'emit_node(' crates/core/orchestration/src/walker.rs` 相对改造前减少相位机 / 落账面相关拍数（读码核对：无「落账 emitted 成节点」的残留）；`grep -c '全部步通过但落账失败'` 为 1（唯一一处，落在 `PhaseSettle::Pass` 的 `Err` 臂）且 `grep -c '门禁失败落账失败\|跳过落账失败'` 各为 1；读码复核 `worker_settle` 四臂与既有文案逐字等价、`SettleMode::Immediate` 路径与改造前等价、`Pass` 三分流与 `step_phase_log` 逐分支等价、`PhaseFail` 两装配函数与两个 `*_upgraded_phase_log` 载荷逐字等价；`cargo check --manifest-path packages/desktop/src-tauri/Cargo.toml` 通过
- [x] 交接 阶段二（测试回归，本相位不执行）：`walker_test.rs` 的形状面断言改写（`node_of(RunStepKind::PhaseStart, 0)` 找相位入口锚点 → 该相位首个真实步节点；`step_instances(RunStepKind::WhitelistGate)` → `step_instances(RunStepKind::Decision)`；`node.step == PhaseLog` 的「已落账」断言 → `phase_log_input` 载荷 / pipeline attempts）、6 处 `walk_run` 整链期望值重定（节点数与边数随 27 个辅助步节点退场而减少）、新增用例（`enter_phase` 无节点无审计 / `settle_phase` 三分流与落账失败 / evaluator 三态 / 决策三态 / 双失败面文案对拍 / `SettleMode::Deferred` 单拍终态 / 计划图漂移六词 / 词汇冻结守卫），**由桌面端 test-design / test-gen / test-execution 相位承接**。**交付物 = 期望登记与移交**

## 阶段三：`steps.rs` 的 id→name 合流 + 开相 / 落账审计面归零

- [x] `crates/core/orchestration/src/steps.rs`：新增私有 `fn resolve_change_name(store: &dyn ChangeStateStore, change_id: &str, what: &str) -> Result<String, String>`（`store.get_change(change_id).map_err(|e| e.to_string())?.map(|r| r.name).ok_or_else(|| format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从执行{what}"))`）
- [x] 四个门禁臂（`ToolCommand::StaticCheck` / `TestExecution` / `MutationTest` / `ArchiCheck`）各自的三段式 id→name 解析改调该函数，`what` 分别取 `"静态检查"` / `"测试门禁"` / `"突变门禁"` / `"架构漂移门禁"`（合成文案与原四处逐字等价）；各臂其余逻辑（runner 调用、vcs 取数、`audit_outcome`）**逐字不动**
- [x] **审计面归零（D11，proposal 措辞的修正点）**：`ToolCommand::PhaseStart` 与 `ToolCommand::PhaseLog` 两分支**删 `audit_outcome` 调用**（写面直调 `workflow::write::phase_start` / `phase_log` 与产出返回**逐字保留**）。理由：`enter_phase` 仍发 `PhaseStart`、`settle_phase` 仍经 `run_phase_log` 发 `PhaseLog`（orchestration 不持 store 句柄），故「调用点消失即审计行消失」的推断不成立；审计归零点只能是产出审计的这里。`audit_outcome` / `clip_summary` 函数本体与其余七个命令分支（`PhaseNext` / `Backtrack` / `DecisionLog` / 三个 runner / `ArchiCheck`）照常留审计
- [x] `StepKind::PhaseStart` / `PhaseLog` 词面与 `as_str` 逐字保留（存量行解码必需，MUST NOT 删除变体）
- [x] 自查 阶段三：`grep -c 'get_change(&change_id)' crates/core/orchestration/src/steps.rs` 为 **1**（只剩 `resolve_change_name` 体内一处；`run_archive_preflight` 的 `get_change(id)` 形态不同不计）；`grep -c 'StepKind::PhaseStart\|StepKind::PhaseLog' crates/core/orchestration/src/steps.rs` 为 **0**（两条审计调用已删，词面只在 `workflow/state.rs` 定义）；读码复核四处调用点文案与改造前逐字一致；`cargo check` 通过
- [x] 交接 阶段三（测试回归）：`steps_test` 的 `StepKind::PhaseStart` / `PhaseLog` 审计行断言（**必改为「零行」**，因为命令仍会发出但不再产审计）与四门禁「未建档」文案断言，**由桌面端 test-design / test-gen / test-execution 相位承接**。**交付物 = 期望登记与移交**

## 阶段四：注释与口径同步复核（代码形状定稿后一次收口）

- [x] `walker.rs` 顶部与本相位机相关 doc 注释同步：`run_step` 的分派说明由「决策分叉三节点（decision / retry-gate / whitelist-gate）为共享子序列、声明漂移即无操作跳过」改为「六变体不可达臂，计划图漂移即显式失败」；`PhaseCursor` doc 去掉已删字段表述；相位内步序注释去掉 `PhaseStart` / `PhaseLog`
- [x] `walker.rs` 的 `enter_phase` / `settle_phase` doc 写清口径：开相与收口是相位机固定动作（零 emit / 零审计 / 零计划步）；`attempt` 口径写「store 内该 `(change_id, phase)` 的 PhaseRecord 全量条数（含 stale）+ 1，由写面单点推导，walker MUST NOT 自推导、MUST NOT 取 `detail.pipeline` 条数」；`round` 与 attempt 两口径 MUST NOT 合并
- [x] `walker.rs` 的单 emit 瞬步表述同步：`emit_node` 的 doc 与 `accumulate_node` 合并语义注释保留，但相位机侧注释里「`phaseStart` 等单 emit 瞬步由此自然同刻」改为「**存量**单 emit 节点的同刻语义仍由合并侧承载（新写面无生产例，MUST NOT 因无生产例删除该合并分支）」。**注意**：`crates/core/orchestration/src/control.rs` 内同句注释**保留原样**（proposal「不要修改」将其列为零改动面；该句描述的合并语义仍然成立，不构成事实错误）
- [x] `core/workflow/src/write/phase_table.rs` 表头 doc（`INITIALIZE_STEPS` 五步 / `ARCHIVE_STEPS` 十一步 / `TEST_EXECUTION_STEPS` 四步 等口径串）与 `phase_table` 函数的相位表概述注释同步为 3 / 9 / 2
- [x] 自查 阶段四：`grep -n 'PhaseStart →\|PhaseLog\|VerdictGate' crates/core/orchestration/src/walker.rs crates/core/workflow/src/write/phase_table.rs` 只剩词面定义 / `decode-only` 注释（零步序串残留）；`grep -c 'verdict 的 5 次' crates/core/workflow/src/write/phase_table.rs` 为 0；`git diff --stat -- packages/desktop/src` **空**

## 阶段五：版本、边界自查与交接备忘

- [x] 修改 `packages/desktop/package.json` 的 `version`：按**落地时 `HEAD` 现值 +1 patch** 相对递增（当前盘面 `0.6.35`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行恰 `+1`，MUST NOT 写死字面号）；`dependencies` / `devDependencies` 零变化
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（零手改）；`src-tauri/Cargo.toml` 的 crate 版本行**未被改动**
- [x] 自查 边界零 diff（逐项 `git diff --stat`）：`crates/infra/store/**`、`packages/desktop/src/**`、`crates/core/workflow/src/prompts/*.md`（14 份）、`crates/core/orchestration/src/{port.rs, control.rs, snapshot.rs, run_history.rs, finish.rs, verdict.rs, decision.rs, prompt.rs, transcript.rs}`、`crates/core/workflow/src/{queries/**, model/**, write/{phase_log.rs, phase_start.rs, phase_next.rs, backtrack.rs, archive.rs}}`、`crates/infra/agent/**`、`.dev-team/architecture/**`、全部 `Cargo.toml` —— 以上**整文件零 diff**
- [x] 自查 关键判据集中复核（AC 逐条，均为读码 / `grep` / `git diff` 面，不含任何用例运行）：AC-1 辅助步零声明与 29 / 23 步数；AC-2 `enter_phase` 唯一开相且零 emit / 零审计（`steps.rs` 的 `PhaseStart` / `PhaseLog` 两分支 `audit_outcome` 调用已删）；AC-3 三分流 + 落账失败前缀 + 节点全绿；AC-4 evaluator 三态与两文案、零 feedback 边；AC-5 decision 节点承载裁决与两 gate 零 `begin_node`；AC-6 三表 22 值与序不变 + `StepGroup::Gate` 分支在案 + 五词 `decode-only` 注释；AC-7 `ARCHIVE_STEPS` 九步；AC-8 六词不可达臂文案；AC-9 七条机械 checklist 文案零删除（`grep` 逐条命中）；AC-10 前端整树零 diff
- [x] 交接交付门（本相位不执行，只登记期望）：`pnpm -C packages/desktop run check` 整链与 `packages/desktop/src/types/generated/**` 零 diff 的再生成核实归桌面端 **static-check**（期望：`ChangeStepKind` 值集不变 ⇒ `bindings:export` 零 diff）；`cargo test` / `pnpm -C packages/desktop test` 回归归 **test-execution**；`DESKTOP_GOLDEN_REWRITE=1` 重写 → 不带环境变量复跑零 diff 归 **test-execution**（期望差异仅步数 / 边数收缩，词汇面不减少）。**交付物 = 期望登记与移交**
- [x] 交接备忘（发给 test-design / test-gen / test-execution，本相位不写用例）：
  - `phase_table_test.rs`：六处序表常量与各相位步数断言（29 / 23）；新增「辅助步零声明」守卫（遍历两表统计 `PhaseStart` / `PhaseLog` / `VerdictGate` 合计 0）
  - `workflow/src/state_test.rs` + `orchestration/src/state_test.rs`：22 值与变体序守卫（删变体 / 重排即红）、`StepGroup::Gate` 三词归属守卫、五词 `decode-only` 注释在场
  - `walker_test.rs`：`enter_phase`（写 active_phase + attempt 全量含 stale + 1、零节点零审计、写失败 → `Terminal`）；`settle_phase` 三分流（`test_outcome` 双槽 / `evaluator_turn` / archive 空 checklist）+ `Pass` 落账失败「节点全绿 + 前缀文案」+ `Fail` / `Skipped` 落账失败的两条独立文案（D12）；evaluator 三态（合规 / 未收口 / 截断）+ 会话 `Failed` / `Stopped` 仍落红节点（D5 第二半）；决策三态（通过 / 越权一次即停 / ask 应答后 `instance_no` 递增）+ 两 gate 节点零产出；`SettleMode::Deferred` 单拍终态；六词不可达臂直构 `Terminal`；双失败面文案对拍；6 处 `walk_run` 整链期望值重定
  - `steps_test.rs`：`PhaseStart` / `PhaseLog` 命令的审计行断言改为「零行」（命令仍发出，审计面归零）；四门禁「未建档」文案不变
  - `corpus_golden_test.rs` + `tests/golden/*.json`：initialize 样本 5 → 3 节点、archive 样本 11 → 9 节点；五词存量解码样本保留（不与真实相位形态样本混同注释）；`tests/fixtures/README.md` 补「解码专用词汇样本 vs 真实相位形态样本」两条目
  - **测试改造与 golden 重写由承接相位落地**；本条在移交发出时即完成
