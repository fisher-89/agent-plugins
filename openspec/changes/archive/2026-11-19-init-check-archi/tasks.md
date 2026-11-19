# 任务: init-check-archi

> 覆盖 proposal「变更范围 - 实现文件」9 行 = **0 新增 + 9 修改**（`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` 在提案中明示「零改动，列出以明示不在范围」，故本文件只作**核对面**任务登记，不含任何编辑动作），另加 design 补入的 **5 处机械闭合面**（port 签名变更的编译扩散：`walker_test.rs` / `steps_test.rs` / `port_test.rs` / `infra/archi/runner_test.rs` / `infra/checks/src/mutation/mod_test.rs`——只补 `mode` 形参 / 字段 / 模式臂 / 实参位，零用例语义改动）。
>
> **本相位不写测试、不跑测试门禁**：`phase_table_test.rs` 的步序 / `expected_on_fail` / 归属守卫断言、`walker_test.rs` 的四节点 emit 序与 archi 红 / `skipped` / 取消 / 漂移用例、`steps_test.rs` 的全量零 vcs 用例、`port_test.rs` 的 `as_str` 对拍、`infra/archi/runner_test.rs` 的「恒增量模式」断言改述与全量落盘用例，**全部归测试相位**（test-design / test-gen / test-execution）。本相位在测试文件上只承担「让既有用例编译通过」的最低机械改动，自查止于 `cargo check --all-targets` 与 `grep` 判据；`pnpm -C packages/desktop run check`、`cargo nextest run --workspace`、`pnpm api:check` 等门禁命令由桌面端执行，本相位不发起、不执行。
>
> 依赖序要点：**阶段一**（port 词汇与载荷）解锁阶段二 / 三 / 五 / 七——它是唯一签名源，改完之前全仓 `--all-targets` 必然红；**阶段四**（相位表四步）与阶段五（walker 两执行体）互为语义配对：`Terminal` 声明在表里、分派在 walker，二者缺一则该步落到「计划图漂移」台账；**阶段二**（`steps.rs` 分派）与**阶段三**（runner 映射）各自只依赖阶段一，互不依赖；**阶段六**（口径改述）须在阶段四 / 五之后（同文件的 doc 与代码一次落定，避免二次写同一区块）；**阶段七**（机械闭合面）解锁全仓 `--all-targets` 编译；**阶段八**是收尾（版本 + 零 diff 核对）。
>
> 自查 cwd：Rust 侧为 `packages/desktop/src-tauri`（下文路径均相对该目录的 `crates/…` / `src/…`）；仓库级命令另行标注。
>
> 不预铺：无 db 迁移 / 新表 / 新字段、无新 IPC 命令、无新出线类型、无 `bindings.ts` 生成物变更、无 `.dev-team/**` 资产变更、无 `config.schema.json` / `KNOWN_KEYS` 变更、无 `tauri.conf.json` / `capabilities/default.json` 变更、无 `core/archi/src/{drift,scan,dto}/**` 变更、无 `infra/archi/src/report.rs` 变更、无 `core/workflow/src/prompts/**` 变更、无前端文件变更、无新 crate / 新外部包 / 新 npm 依赖、无路由。

## 阶段一：port 词汇与命令载荷（`core/orchestration/src/port.rs`）

- [x] 新增 `pub enum ArchiCheckMode { Full, Incremental }`：落 `ArchiCheckOutcome` 之前的类型区（`ArchiCheckRunner` 之前），derive `Debug, Clone, Copy, PartialEq, Eq`（**零 `Serialize` / `Deserialize` / `specta::Type`**），两变体各带一行 doc（`Full` = 扫整个观察根、`Incremental` = 只扫 change 编辑集）
- [x] 同类型 `impl ArchiCheckMode` 加 `pub fn as_str(self) -> &'static str`：`Full => "full"` / `Incremental => "incremental"`（与 `ArchiDriftMode::as_str` 逐字同词面）；**不设 `ALL` 常量**（port 面零枚举遍历消费者，与 `ReportSlot` 同形）
- [x] 同文件类型 doc 补一句总纲：`ArchiCheckMode` 是纯进程内 port 词汇（零 serde / 零 specta、不进 IPC），模式由消费面声明、runner MUST NOT 推断，port 词 → 判定域 `ArchiDriftMode` 的映射只在 `crates/infra/archi` 一处
- [x] 修改 `ToolCommand::ArchiCheck`：`{ change_id: String }` → `{ change_id: String, mode: ArchiCheckMode }`（`mode` 放**第 2 位**，与 `StaticCheck` / `TestExecution` 的 `slot` 同位同型）；variant doc 改述为「架构漂移门禁（两挂载点：initialize 全量 / implement 增量；报告槽位恒 `ReportSlot::Archi` 由 runner 单点取用——不设 slot 字段；两挂载点同树落 `reports/archi`）」
- [x] 修改 `ArchiCheckRunner::run`：签名改为 `fn run(&self, root: &str, name: &str, mode: ArchiCheckMode, changed: Vec<String>, cancel: CancelFlag) -> BoxToolFuture`（`mode` 放**第 3 位**）；trait doc 补「模式由消费面声明、runner MUST NOT 推断」「两挂载点同树落 `reports/archi`」两句，既有「零进程 / 写盘失败降级 / 取消以 `fault` 正常返回」三条逐字保留；同 doc 的「**四参显式化**」改述为「**五参显式化**」并把 `mode` 插入形参清单（`root` / `name` / `mode` / `changed` / 末位取消信号）——「报告槽位不设形参（恒 `ReportSlot::Archi`，runner 内部取用）」一句逐字保留
- [x] 确认 `ArchiCheckOutcome` 五字段（`passed` / `diagnostics` / `fault` / `report_dir` / `report_ref`）逐字不动；`core/orchestration/src/lib.rs` 根再导出名单**零改动**（`ArchiCheckMode` 的消费者全在 crate 内或经 `orchestration::port::` 直引）
- [x] 自查 阶段一：`grep -n "pub enum ArchiCheckMode" -A 8 crates/core/orchestration/src/port.rs` 恰一处且块内**无** `serde` / `specta` 字样；`grep -n "pub fn as_str(self) -> &'static str" crates/core/orchestration/src/port.rs` 命中数 +1（`ArchiCheckMode` 一处）；`grep -n "mode: ArchiCheckMode" crates/core/orchestration/src/port.rs` 恰两处（`ToolCommand` 变体 + `run` 形参）；`grep -n "四参显式化" crates/core/orchestration/src/port.rs` **零命中**、`grep -n "五参显式化" crates/core/orchestration/src/port.rs` 恰一处；`grep -n "ArchiCheckMode" crates/core/orchestration/src/lib.rs` **零命中**；`git diff --stat packages/desktop/src/types/generated/` **零命中**

## 阶段二：命令面两模式分派（`core/orchestration/src/steps.rs`）

- [x] `use crate::port::{…}` 补 `ArchiCheckMode`（按既有字母序插入）
- [x] 修改 `ToolCommand::ArchiCheck` 臂（现 `:275-299`）：解构补 `mode`，把既有 `let changed = vcs.change_edit_set(Path::new(&step.root), "HEAD")?;` 包进 `match mode { ArchiCheckMode::Full => Vec::new(), ArchiCheckMode::Incremental => vcs.change_edit_set(Path::new(&step.root), "HEAD")? }`
- [x] 同臂注释改述为两行语义：全量臂「零 vcs 触碰——取数只会引入与基线架构无关的失败面（git 不可达时基线架构检查照样执行）」、增量臂「变更集唯一入口；取数失败显式传播为步失败，MUST NOT 降级空清单（空清单会静默变成 `noChanges` = 假通过）」
- [x] 同臂 runner 调用改 `archi.run(&step.root, &name, mode, changed, cancel).await`；`resolve_change_name(store.as_ref(), &change_id, "架构漂移门禁")` 与 `audit_outcome(... StepKind::ArchiCheck ...)` 的摘要文本逐字不动
- [x] 自查 阶段二：`grep -n "ArchiCheckMode::Full" crates/core/orchestration/src/steps.rs` 恰一处；`grep -n "change_edit_set" crates/core/orchestration/src/steps.rs` 的 `ArchiCheck` 邻域恰一处调用（位于 `Incremental` 臂）；`grep -n "StepKind::ArchiCheck" crates/core/orchestration/src/steps.rs` 恰一处且周边摘要串零改动（`git diff` 逐行核对）；`grep -n "Full => Vec::new()" crates/core/orchestration/src/steps.rs` 恰一处

## 阶段三：门禁执行体模式映射（`crates/infra/archi/src/runner.rs`）

- [x] `use orchestration::port::{…}` 补 `ArchiCheckMode`
- [x] 新增私有 `fn drift_mode(mode: ArchiCheckMode) -> ArchiDriftMode`（文件尾部 helper 区，`file_stem` 之前）：`ArchiCheckMode::Full => ArchiDriftMode::Full` / `ArchiCheckMode::Incremental => ArchiDriftMode::Incremental`；doc 写明「port 词 → 判定域词的唯一映射点」
- [x] `impl ArchiCheckRunner for LocalArchiCheck` 的 `run` 加 `mode` 形参（第 3 位），`check(archi::drift::CheckInput { …, mode: drift_mode(mode), … })`；`changed: &changed` 原样透传（**MUST NOT** 在 runner 侧判空 / 清空——全量忽略变更集是判定域既有语义，runner 侧分支即第二套模式判定）
- [x] 确认其余逐字不动：报告目录 `change_reports(root_path, &name, ReportSlot::Archi)`、`Err(CheckError::Stopped)` → `fault: Some(RunFault::Stopped)`、`write_report(...).ok()` 降级、`passed` 派生、`compose_text` 全部零 diff；`crates/infra/archi/src/report.rs` **零 diff**
- [x] 自查 阶段三：`grep -n "ArchiDriftMode::" crates/infra/archi/src/runner.rs` 只命中 `drift_mode` 函数体（映射臂两处 + 返回类型一处），`check` 调用点仅一处 `mode: drift_mode(mode)`；`grep -n "changed.is_empty\|!changed" crates/infra/archi/src/runner.rs` **零命中**；`grep -rn "std::process\|tokio::process" crates/infra/archi/src/` **零命中**；`git diff --stat -- crates/infra/archi/src/report.rs` 无输出

## 阶段四：相位表四步单源（`crates/core/workflow/src/write/phase_table.rs`）

- [x] `INITIALIZE_STEPS` 在 `StaticCheck` 表项与 `TestExecution` 表项之间插入 `StepDefinition { node: StepNode::ArchiCheck, prompt_ref: None, on_fail: Some(OnFail::Terminal) }`（`Clean` 的 `on_fail: None` 与 `StaticCheck` / `TestExecution` 两表项逐字不动；末步仍 `TestExecution`）
- [x] 改述 `:107-112` 的 `INITIALIZE_STEPS` doc：步序 `Clean → StaticCheck → ArchiCheck → TestExecution`；三检查步 `on_fail=Terminal`（失败即停——无反馈边、零 agent 会话注入，基线的债不属于本次变更）；插入理由一句（代价升序：秒级 lint / 毫秒级进程内架构判定 / 分钟级测试；失败即停让基线架构红在一次长测试跑之前露头）；保留既有「开相与相位收口不在 `steps` 内」两句
- [x] 改述 `:376-378` 的 initialize `PhaseDefinition` 注释：`INITIALIZE_STEPS`「三步两表共享」→「四步两表共享」；其余描述（executor / evaluator 双缺席、零 `artifact_manifest`、`backtrackable=false`）逐字保留
- [x] 确认 implement 表项的 `ArchiCheck`（`on_fail: Some(OnFail::FeedbackLoop { limit: 3 })`）与 `TEST_ONLY_TABLE` 的 initialize 条目（仍指同一 `INITIALIZE_STEPS`）逐字不动
- [x] 自查 阶段四：`grep -n "INITIALIZE_STEPS" crates/core/workflow/src/write/phase_table.rs` 命中「static 定义一处 + 两表引用两处」；`grep -n "StepNode::ArchiCheck" -B2 -A3 crates/core/workflow/src/write/phase_table.rs` 恰两处（initialize 表项 `Terminal` + implement 表项 `FeedbackLoop`）；`grep -n "三步两表共享\|Clean → StaticCheck → TestExecution" crates/core/workflow/src/write/phase_table.rs` **零命中**；`grep -n "FeedbackLoop { limit: 3 }" crates/core/workflow/src/write/phase_table.rs` 仍在位

## 阶段五：walker 分派与终态执行体（`crates/core/orchestration/src/walker.rs`）

- [x] `use crate::port::{…}` 补 `ArchiCheckMode`
- [x] initialize 词汇区（`:52-60`）新增常量 `TXT_BASELINE_ARCHI_FAILED`，**逐字**：`"基线架构检查未通过，请在基线上修复模型或代码后重新发起（基线的债不属于本次变更）；报告目录 {dir}"`（置于 `TXT_BASELINE_STATIC_FAILED` 之后）；同区注释改述为「baseline 四条失败文案（static / archi / test Error / test Fail——四条互不共用文案，均 Terminal 停给用户；`{dir}` 为各步报告目录占位）」
- [x] 修改 `step_archi_check`（现 `:1494`）为分派入口：`match step.on_fail { Some(OnFail::Terminal) => step_archi_check_terminal(...).await, Some(OnFail::FeedbackLoop { .. }) => step_archi_check_feedback(..., step).await, Some(OnFail::Escalate) | None => Err(Terminal::failed(format!("计划图漂移：phase \"{}\" 的 archi-check 步缺 OnFail::Terminal / FeedbackLoop 声明", ctx.phase))) }`；入口 doc 改述为两态表（`FeedbackLoop` = implement 反馈边 / `Terminal` = initialize 单发终态 / `Escalate`·`None` = 计划图漂移）
- [x] 把既有反馈边实现体原样搬入 `step_archi_check_feedback(ctx, trace, cursor, step)`：`feedback_limit(step, "archi-check")` 取上限、`begin_node` + `emit_node(running)`、`run_tool(ToolCommand::ArchiCheck { change_id, mode: ArchiCheckMode::Incremental }, "archi-check", None)`、fault 早退 `settle_fault(..., template: None, &outcome.report_dir, None)`、passed → `StepFlow::Proceed`、failed → 反馈边 / 超限升格 `StepFlow::Upgraded`；**除 `mode` 字段外逐字不改**（含 `FIX_HINT` 相关文本、`instance_no` 递增、`EdgeKind::Feedback`）
- [x] 新增 `step_archi_check_terminal(ctx, trace, cursor) -> Result<StepFlow, Terminal>`（置于 `step_archi_check_feedback` 之前，与 `step_static_check_terminal` 同型）：`begin_node(..., ChangeStepKind::ArchiCheck, EdgeKind::Forward)` → `emit_node(running)` → `let outcome: ArchiCheckOutcome = run_tool(ctx.tools, ctx.guard, &ctx.request.root, ToolCommand::ArchiCheck { change_id: ctx.request.change_id.clone(), mode: ArchiCheckMode::Full }, "archi-check", None).await?`
- [x] 同函数三段收口：① `if let Some(fault) = outcome.fault.as_ref() { return Err(settle_fault(ctx.guard, ctx.phase, cursor.attempt, ChangeStepKind::ArchiCheck, instance_no, fault, None, &outcome.report_dir, None)); }`（`template = None`——零新增环境文案，`output_ref = None`）；② `if outcome.passed { emit_node(..., StepSettle::passed(Some(diagnose_brief(&outcome.diagnostics)))); return Ok(StepFlow::Proceed); }`（detail 携 `mode=full` / 状态词 / 扫描文件数——文本由 runner 装配；`skipped` / `noChanges` 两态逐字文案原样透出）；③ `emit_node(..., StepSettle::failed(Some(diagnose_brief(&outcome.diagnostics)))); Err(Terminal::failed(TXT_BASELINE_ARCHI_FAILED.replace("{dir}", &outcome.report_dir)))`。该函数**禁止**出现 `run_worker` / `feedback` 计数 / `step_gate_fail_phase_log` / `.with_output_ref(`
- [x] 函数 doc 写明：单发一次、零会话、零反馈边、零 fail checklist 代写（`Terminal` 语义天然保证 PhaseRecord 零 initialize 条目）、fault 先于结论（本 runner 零进程 ⇒ 生产路径恒 `None`，本臂只作同形保留）
- [x] 改述 `:477-480` 的相位内计划步执行体区段注释：步序四步（`Clean → StaticCheck → ArchiCheck → TestExecution`）、三检查步 `on_fail=Terminal`；`run_step` 的分派臂（`StepNode::ArchiCheck => step_archi_check(...)`）逐字不动
- [x] 自查 阶段五：`grep -n "TXT_BASELINE_ARCHI_FAILED" crates/core/orchestration/src/walker.rs` 恰两处（常量定义 + terminal 使用）；`grep -n "fn step_archi_check" crates/core/orchestration/src/walker.rs` 恰三个定义（分派 / terminal / feedback）；`grep -n "ArchiCheckMode::" crates/core/orchestration/src/walker.rs` 恰两处（`Full` 在 terminal、`Incremental` 在 feedback）；`grep -n "两检查步\|Clean → StaticCheck → TestExecution" crates/core/orchestration/src/walker.rs` **零命中**；`awk '/fn step_archi_check_terminal/,/^}/' crates/core/orchestration/src/walker.rs | grep -n "run_worker\|feedback\|phase_log\|output_ref"` **零命中**

## 阶段六：词汇与口径改述（`state.rs` / `layout/mod.rs`）

- [x] `crates/core/workflow/src/state.rs:417-421`：`OnFail::Terminal` doc 的「消费点仅 initialize 相位两检查步 StaticCheck / TestExecution」改为「消费点仅 initialize 相位三检查步 StaticCheck / ArchiCheck / TestExecution（基线绿门——失败即停，无反馈边、零 agent 会话可注入，重试基线无意义）」；「语义与缺省 `None` 等价」句逐字保留
- [x] `crates/core/workflow/src/state.rs:313`：`StepNode::ArchiCheck` doc 由「archi-check 门禁工具步（架构漂移判定，定向反馈边、零进程）」改为「archi-check 门禁工具步（架构漂移判定；按 `on_fail` 两态：`FeedbackLoop` = implement 反馈边、`Terminal` = initialize 单发终态，零进程）」；**`RunStepKind::ArchiCheck`（`:562`）与其 doc、`StepKind::ArchiCheck`（`:44`）与其 doc 逐字不动**（前者直出 `bindings.ts`；后者不在本变更面）
- [x] `crates/core/foundation/src/layout/mod.rs:58`：`ReportSlot::Baseline` 变体 doc「initialize 相位两检查步」改为「initialize 相位 static-check / test-execution 两步（archi 基线检查走 `Archi` 槽位）」
- [x] 同文件 `:48-53` 的 `ReportSlot` 类型 doc 补一句：`Archi` 由 initialize 基线检查与 implement 门禁**两挂载点共用**；`report_dir_name` / `change_reports` / `change_reports_in` 逻辑零改动
- [x] 确认零改动面：`crates/core/orchestration/src/state.rs` 的 `ChangeStepKind::ArchiCheck` doc（`:96`，直出 `bindings.ts`）、`crates/infra/archi/src/report.rs`、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs`、`packages/desktop/src/views/changes/flow/**`、`crates/core/workflow/src/prompts/**` 全部零 diff
- [x] 自查 阶段六：`grep -rn "两检查步" crates/ --include=*.rs | grep -v "_test.rs"` **零命中**（非测试源码面已清；测试文件的注释与断言消息串归测试相位，见「移交说明」）；`grep -rn "三步两表共享" crates/` **零命中**；`grep -n "定向反馈边" crates/core/workflow/src/state.rs` 恰三处且均为保留面（`StepKind::ArchiCheck` `:44`、`StepNode::StaticCheck` `:277`、`OnFail::FeedbackLoop` `:412`——`StepNode::ArchiCheck` 那处已改述、`RunStepKind` 本无该词）；`grep -n "定向反馈边" crates/core/orchestration/src/state.rs` 仍一处（`:96`）且**未被改动**；`git diff --stat packages/desktop/src/types/generated/` **零命中**；`grep -n "const TXT_BASELINE_" crates/core/orchestration/src/walker.rs` 恰四条定义且四条文案两两不同（`grep -n "const TXT_BASELINE_" -A2` 逐条比对）

## 阶段七：机械闭合面（测试文件签名跟随）

- [x] `crates/core/orchestration/src/walker_test.rs`：`NullArchiRunner::run`（`:5138`）与 `CountingArchiRunner::run`（`:14743`）签名补 `mode: ArchiCheckMode`（形参可命 `_mode`，函数体零改动）；`use` 区补 `ArchiCheckMode`
- [x] `crates/core/orchestration/src/steps_test.rs`：`UnusedArchiRunner::run`（`:623`）与 `RecordingArchiRunner::run`（`:4162`）签名补 `mode`（`RecordingArchiRunner` 如需记录模式可暂存形参但**不新增断言**，机械下限为忽略）；13 处 `ToolCommand::ArchiCheck { change_id: … }` 构造补 `mode: ArchiCheckMode::Incremental`（`:4251` / `:4289` / `:4320` / `:4349` / `:4376` / `:4411` / `:4459` / `:4498` / `:4530` / `:4567` / `:4603` / `:4833` / `:5055`）；`:5066` 的模式臂改 `ToolCommand::ArchiCheck { change_id, .. }`；`use` 区补 `ArchiCheckMode`
- [x] `crates/core/orchestration/src/port_test.rs`：① `RecordingArchiCheck`（`:3291`）的 `impl crate::port::ArchiCheckRunner for RecordingArchiCheck` 的 `run` 签名补 `mode: ArchiCheckMode`（第 3 位；机械下限以 `_mode` 忽略——`type ArchiCall` 保持四元组、`push((root, name, changed, cancel.is_set()))` 与捕获断言零改动）；② 两处 UFCS 调用点在第 3 实参位补 `ArchiCheckMode::Incremental`：`:3325`（`ArchiCheckRunner::run(runner.as_ref(), "/ws", "demo", changed.clone(), CancelFlag::never())` → 在 `"demo"` 与 `changed.clone()` 之间插入）与 `:3355`（`run(runner.as_ref(), "/ws", "demo", Vec::new(), cancel)` → 同位置插入）；③ `use crate::port::{…}` 补 `ArchiCheckMode`；④ `:249` 的 `ToolCommand::ArchiCheck { change_id }` 模式臂改 `{ change_id, .. }`（`:231` / `:1042` 的 `{ .. }` 零改动）
- [x] `crates/infra/archi/src/runner_test.rs`：3 处 `.run(...)` 调用补第 3 实参 `ArchiCheckMode::Incremental`（既有语义 = 增量，`:336` 的「恒增量模式」断言因此仍成立）；`use` 区补 `ArchiCheckMode`
- [x] `crates/infra/checks/src/mutation/mod_test.rs`：`UnusedArchiSeam::run`（`:1527`）签名补 `mode` 形参（该臂恒 `Err`，用例断言零改动）；`use` 区补 `ArchiCheckMode`（经 `orchestration::port`）
- [x] 全仓编译闭合：`cargo check --all-targets`（cwd `packages/desktop/src-tauri`）通过
- [x] 自查 阶段七：`grep -rn "impl .*ArchiCheckRunner for" crates/` 命中八处 = **七个真 impl**（`port_test.rs:3291` / `steps_test.rs:623` / `:4162` / `walker_test.rs:5138` / `:14743` / `infra/archi/src/runner.rs:55` / `infra/checks/src/mutation/mod_test.rs:1527`）+ 一处字符串字面量（`infra/archi/src/runner_test.rs:1198` 的 `include_str!("runner.rs")` 断言针，**MUST NOT 改动**、不计入签名核对），七个真 impl 的 `run` 签名均含 `mode: ArchiCheckMode`（**勿用**不带 `.*` 的 `grep "impl ArchiCheckRunner for"`——它七命中 = 六处真 impl + 同一处字面量，漏掉带 `crate::port::` 前缀的 `port_test.rs:3291`）；`grep -rn "ToolCommand::ArchiCheck {$" -A2 crates/ | grep -c "mode:"` 与构造点数一致；`cargo check --all-targets` 零错误；`git diff --stat` 的测试文件改动**只含**上述五份（逐行核对 `mode` / `..` / `_mode` 字样，`port_test.rs` 另含两处 UFCS 实参插入，均属编译闭合；**零断言文本改写**）

## 阶段八：版本与零 diff 核对

- [x] `packages/desktop/package.json`：`"version": "0.6.47"` → `"0.6.48"`（单行，其余逐字不动）
- [x] 确认 `packages/desktop/src-tauri/Cargo.toml` 的 crate 版本行（`version = "0.1.0"`）**零改动**；`packages/desktop/src-tauri/tauri.conf.json` 零改动（`"version": "../package.json"` 自动跟随）
- [x] 零 diff 白名单核对（供门禁与回溯复核，本相位只做 `git diff --stat` 人工比对）：`packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts`、`.dev-team/architecture/**`（模型 / 契约 / `INDEX.toml` / `config.json`）、`openspec/specs/**`（仅在 change 目录出 delta）、`crates/core/archi/src/**`、`crates/infra/archi/src/report.rs`、`crates/core/config/**`、`.dev-team/config.json`、`packages/desktop/src/**`（前端全部） —— 以上均**零 diff**
- [x] 自查 阶段八：`grep -n '"version"' packages/desktop/package.json` = `0.6.48`；`git diff --stat` 的文件集与 design「修改文件」+「机械闭合面」两表**逐项相符**（多一份或少一份即偏离设计）

## 移交说明（不含任务）

- 断言级改写的承接方：test-design / test-gen / test-execution——`phase_table_test.rs`（`INITIALIZE_STEP_ORDER` 3→4、步数与 `expected_on_fail` 表、`Terminal` / `Escalate` 归属守卫、步序串断言）、`walker_test.rs`（initialize 全绿链四节点 emit 序、archi 红 `TXT_BASELINE_ARCHI_FAILED` + `{dir}`、`skipped` 逐字「未检查（无模型）」、取消 → stopped、`Escalate` → 计划图漂移）、`steps_test.rs`（全量臂 vcs 零调用 + 空清单）、`port_test.rs`（`as_str` 逐值对拍与封闭性；`RecordingArchiCheck` 的 `mode` 透传捕获——本相位以 `_mode` 忽略、捕获元组仍四元，故 `ArchiCall` doc（`:3270`「捕获的四参」）、假件 doc（`:3268`）、测试 doc（`:3310`）、测试名（`:3313` `archi_check_runner四参透传_changed保序且零slot形参`）与断言消息（`:3343`「四参逐值到达」）的改述一并归此相位）、`infra/archi/runner_test.rs`（「恒增量模式」改述 + 全量落盘用例）。
- **测试文件侧的「两检查步 / 三步序」口径串**（实测：`steps_test.rs:3751`、`walker_test.rs:10417` / `10461` / `10576` / `10594`、`phase_table_test.rs:40` / `1060` / `1088` / `2452` / `2473` / `2495` / `2962` / `2972`）随断言改写一并在测试相位清零——AC-10 的 `grep` 终态判据在测试相位闭环后成立；本相位只保证非测试源码面零残留（阶段四 / 五 / 六），并在阶段七对测试文件做纯编译闭合（零注释改写、零断言改写）。
- 门禁命令（`pnpm -C packages/desktop run check`、`cargo nextest run --workspace`、`pnpm api:check`）由桌面端执行；本相位不重复执行。
- AC-8（本仓基线实证：全量判定 `status = clean` / `scannedFiles = 396` / `by_gate` 全 `off`）作为登记留痕，无实现任务。
