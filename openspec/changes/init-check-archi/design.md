# 设计: init-check-archi

> **变更**: init-check-archi
> **日期**: 2026-11-18
> **基于**: `proposal.md`（问题三病灶 / 提案一~六 / 变更范围 / AC-1…AC-11 / R-1…R-9）、`explore.md`（只读，一句目标原文）与六份 delta spec（`desktop-archi-gate` 1 RENAMED + 3 MODIFIED；`desktop-change-orchestration` 2 MODIFIED；`desktop-workflow-write-face` 1 MODIFIED；`desktop-change-flow-view` 1 MODIFIED；`desktop-archi-drift` 3 MODIFIED；`desktop-checks-domain` 1 MODIFIED）
> **性质**: 在 initialize 表头相位追加第三个基线检查步 `archi_check`（全量判定、`on_fail=Terminal`）。改面只有四条：相位表多一步、port 多一个模式位、`steps.rs` 多一条分派、walker 多一个单发终态执行体；判定规则（扫什么 / 怎么归属 / 定级 / 豁免 / 报告形状）、报告落盘单点、前端呈现面、IPC 面、配置面**零改动**。

---

## 事实校准（提案 vs 代码实测）

不改 spec 意图，只把提案行文与代码实况的偏差点摆明并裁定：

1. **`steps_test.rs` 的 `ToolCommand::ArchiCheck` 不是「四处」而是 14 处**（13 处构造 + 1 处模式臂：`:4251` / `:4289` / `:4320` / `:4349` / `:4376` / `:4411` / `:4459` / `:4498` / `:4530` / `:4567` / `:4603` / `:4833` / `:5055` 构造，`:5066` 模式臂）。裁定：机械面一次改齐，13 处构造一律补 `mode: ArchiCheckMode::Incremental`（它们覆盖的正是增量取数臂，断言语义零改动）；全量臂的新用例归测试相位。
2. **`ArchiCheckRunner` 的假件实测六个 impl（连生产实现共七个真 impl）**：`NullArchiRunner`（`walker_test.rs:5138`）、`CountingArchiRunner`（`walker_test.rs:14743`）、`UnusedArchiRunner`（`steps_test.rs:623`）、`RecordingArchiRunner`（`steps_test.rs:4162`）、`UnusedArchiSeam`（`infra/checks/src/mutation/mod_test.rs:1527`）、`RecordingArchiCheck`（`port_test.rs:3291`，写全路径 `impl crate::port::ArchiCheckRunner for`）＋ 生产实现 `LocalArchiCheck`（`infra/archi/src/runner.rs:55`）。提案 R-4 只点了五个假件名字（漏 `port_test.rs` 的 `RecordingArchiCheck`）；按实测**逐处补 `mode`**，漏 `port_test.rs:3291` 及其两处 UFCS 调用（`:3325` / `:3355`）即 `cargo check --all-targets` 必红。**自查口径**：`grep "impl ArchiCheckRunner for"` 七命中 = 六处真 impl + `infra/archi/src/runner_test.rs:1198` 的 `include_str!("runner.rs")` 断言针字面量（`"impl ArchiCheckRunner for LocalArchiCheck {"`），漏掉带 `crate::port::` 前缀的 `port_test.rs:3291`；须用 `impl .*ArchiCheckRunner for`——八命中 = 七个真 impl + 同一处字面量（不参与签名核对、MUST NOT 改动；本变更加形参不改 impl 头，该断言仍真）。
3. **`infra/archi/src/runner_test.rs` 的 `.run(` 调用实测 3 处**，另有「恒增量模式」断言一处（`:337`：`assert_eq!(written.mode, ArchiDriftMode::Incremental, "恒增量模式")`）。前者是签名闭合面，后者是断言面（改述归测试相位）。
4. **`core/orchestration/src/lib.rs` 零改动**（提案未列，实测亦无需动）：`ArchiCheckMode` 的消费者全在 crate 内（`steps.rs` / `walker.rs` / `port_test.rs` 走 `crate::port`）或经 `orchestration::port::` 直引（`infra/archi/src/runner.rs`），而根再导出名单当前不收 `ReportSlot` 这类槽位词汇（`ReportSlot` 来自 `foundation::layout`）。根面不扩，避免无消费者的出口面扩张。
5. **`ChangeStepKind::ArchiCheck` 的 variant doc 直出 `bindings.ts`**（实测 `packages/desktop/src/types/generated/bindings.ts:1495`：「archi-check 门禁工具步（架构漂移判定，定向反馈边）」）；`StepKind::ArchiCheck`（`workflow/src/state.rs:44`，同措辞）虽 derive `specta::Type` 但未入 `bindings.ts`（实测 `grep` 只命中 `RunStepKind` / `ChangeStepKind`），而 `OnFail` / `StepNode` 两者**均不在** `bindings.ts` 内。裁定：`OnFail` / `StepNode` 两处 doc 随本变更改述（提案 §五 已列）；`ChangeStepKind` 的 doc MUST NOT 动（AC-9 绑定零 diff 优先）；`StepKind` 的 doc 本变更不动（保留在「待决问题」登记，避免为一句注释扩大实现面）。
6. **`ReportSlot` 的类型级 doc 已含三值说明**（`foundation/src/layout/mod.rs:48-53`），提案所说「两处措辞」实为「类型 doc 补一句 `Archi` 由本次两挂载点共用 + `Baseline` 变体 doc 一句收敛」，均零逻辑改动。
7. **契约零 diff 有机械依据**：`.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md` 只声明 `file_stem` 一 op 与零字段类型 `LocalArchiCheck`；`ArchiCheckRunner::run` 不在契约面，故签名加参**不产生契约判据**（`apiUndeclared` 对未声明导出当前 `severity() == Off`）⇒ AC-9 的 `api:check` 零差异成立。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 挂载点与步序 | `INITIALIZE_STEPS` = `Clean(None) → StaticCheck(Terminal) → ArchiCheck(Terminal) → TestExecution(Terminal)`；`ArchiCheck` 插在 `StaticCheck` 与 `TestExecution` 之间，表项 `on_fail: Some(OnFail::Terminal)`、`prompt_ref: None`；`Clean` 仍 `None`；`REQUIREMENT_TABLE` / `TEST_ONLY_TABLE` 的 initialize 条目仍指同一 `INITIALIZE_STEPS`（指针等值） | 代价升序（秒级 lint → 毫秒级进程内判定 → 分钟级测试）＋失败即停：基线架构红必须在一次长测试跑之前露头；两表共享单源是既有纪律（既有测试以指针等值断言，MUST NOT 为 test-only 另写一份表头） |
| D2 | port 词汇 `ArchiCheckMode` | 落 `crates/core/orchestration/src/port.rs`（紧随 `ArchiCheckOutcome` 之前的类型区）：`#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ArchiCheckMode { Full, Incremental }` + `pub fn as_str(self) -> &'static str`（`full` / `incremental`）。**零 serde / 零 specta / 不设 `ALL`** | 纯进程内 port 词汇，不进 IPC（零 `bindings.ts` 影响）；不设 `ALL` 与 `ReportSlot` 同形（port 面零枚举遍历消费者），封闭性由穷尽 `match` 的编译期锚 + `port_test` 逐值对拍承载 |
| D3 | `ToolCommand::ArchiCheck` 载荷 | `ArchiCheck { change_id: String, mode: ArchiCheckMode }`——`mode` 放**第 2 位**（与 `StaticCheck` / `TestExecution` 的 `slot` 同位同型）；variant doc 改述为「模式由消费面声明（initialize 全量 / implement 增量）、两挂载点同树落 `reports/archi`」 | 机械扩散面最小：与既有槽位载荷同形的字段序让「同一位置的第二个门禁参数」读起来一致（R-4） |
| D4 | `ArchiCheckRunner::run` 签名 | `fn run(&self, root: &str, name: &str, mode: ArchiCheckMode, changed: Vec<String>, cancel: CancelFlag) -> BoxToolFuture`——`mode` 放**第 3 位**（`root` / `name` 之后、`changed` 之前） | 提案 R-4 明示位次；`changed` 保持末二元（变更集 + 取消）的既有读法，增量臂调用点逐字可读 |
| D5 | `steps.rs` 两模式分派 | `match mode { ArchiCheckMode::Full => Vec::new(), ArchiCheckMode::Incremental => vcs.change_edit_set(Path::new(&step.root), "HEAD")? }`，随后 `archi.run(&step.root, &name, mode, changed, cancel)`；审计行 `StepKind::ArchiCheck` 与摘要文本逐字不动 | 全量臂**零 vcs 调用**（不取数、不探测）——git 不可达也不该让基线架构检查失败（spec「基线全量口径零 vcs 触碰」）；增量臂逐字保留「取数失败显式传播、MUST NOT 降级空清单」 |
| D6 | `walker.rs` 分派与两执行体 | `match step.on_fail`：`Some(OnFail::Terminal)` → 新 `step_archi_check_terminal`；`Some(OnFail::FeedbackLoop { .. })` → 既有实现体原样搬入 `step_archi_check_feedback`（唯一改动：`run_tool` 的命令携 `mode: ArchiCheckMode::Incremental`）；`Some(OnFail::Escalate) \| None` → `Err(Terminal::failed(format!("计划图漂移：phase \"{}\" 的 archi-check 步缺 OnFail::Terminal / FeedbackLoop 声明", ctx.phase)))` | 形态对齐 `step_test_execution` 的三臂 `match` 先例（Escalate / None 显式漂移，不静默 `Proceed`、不退化任一臂）；**不沿用** `step_static_check` 的 `if step.on_fail == Some(Terminal)` 前置判断——那会把 Escalate / None 的兜底交给 `feedback_limit` 的「缺 FeedbackLoop 声明」文案，与本步 spec 的三态表不符 |
| D7 | 终态执行体 `step_archi_check_terminal` | 与 `step_static_check_terminal` 逐段同型，差异仅三处：① 命令携 `mode: ArchiCheckMode::Full`，`run_tool` 的 `identity: None`（架构报告非 exec 输出 ⇒ 节点 `output_ref` 恒 `null`）；② `settle_fault(..., template: None, &outcome.report_dir, output_ref: None)`（`ArchiCheckOutcome` 无 `output_ref` 字段；零新增环境文案）；③ failed 分支 `Terminal::failed(TXT_BASELINE_ARCHI_FAILED.replace("{dir}", &outcome.report_dir))`。passed 分支 detail = `diagnose_brief(&outcome.diagnostics)` | 与同相位另两步同形是「同一相位三种 Err / 三态文案形态一致」的唯一机械保证（spec 明令 MUST NOT 为 archi 单开包装文案）；runner 侧文本已携 `mode=full` + 状态词 + 扫描文件数，`skipped` / `noChanges` 两态逐字文案原样透出（`diagnose_brief` 只压空白、不截断这两个短串） |
| D8 | 基线文案常量 | `TXT_BASELINE_ARCHI_FAILED = "基线架构检查未通过，请在基线上修复模型或代码后重新发起（基线的债不属于本次变更）；报告目录 {dir}"`，落 walker.rs initialize 词汇区（`TXT_BASELINE_STATIC_FAILED` 之后）；该区注释由「baseline 三态失败文案」改述为「baseline 四条失败文案（static / archi / test Error / test Fail，四条互不共用；`{dir}` 为各步报告目录占位）」 | 三检查步不共用文案（既有纪律）；`{dir}` 值 = `outcome.report_dir` = `reports/archi`，与 implement 门禁同一报告树（R-3 的回查锚） |
| D9 | 报告落盘面 | `crates/infra/archi/src/report.rs` **逐字不动**：目录仍经 `change_reports(root, name, ReportSlot::Archi)` 单点派生、文件名仍是 `<timestamp 紧凑形>.json`；两挂载点**同树共居**，靠报告体自述 `mode` / `change` / `timestamp` 区分 | 判定域零写盘纪律不变，落盘者恒为 runner；MUST NOT 新增第四槽位、MUST NOT 落 `reports/baseline`（checks 域 schema 分树是既有边界） |
| D10 | runner 模式映射单点 | `impl ArchiCheckRunner for LocalArchiCheck` 内恰一处映射（私有 `fn drift_mode(mode: ArchiCheckMode) -> ArchiDriftMode`）：`Full → Full`、`Incremental → Incremental`；`changed` **不在 runner 侧清空**——全量忽略变更集是 `drift::check` 的既有语义（`CheckInput.changed` 在 `Full` 分支不读），runner 再加 `if !changed.is_empty()` 一类分支就构成第二套模式判定 | 模式是入参不是派生量（spec「模式由消费面声明、runner MUST NOT 推断」）；port 词 → 判定域词的映射 `grep` 可验恰一处 |
| D11 | 口径改述清单（AC-10 机械面） | ① `phase_table.rs:107-112` doc：四步序 + 三检查步 `Terminal` + 插入理由一句；② `phase_table.rs:376-378` initialize `PhaseDefinition` 注释：`INITIALIZE_STEPS` 三步 → 四步；③ `workflow/src/state.rs:417-421` `OnFail::Terminal` doc：消费点「两检查步」→「initialize 表头三检查步（StaticCheck / ArchiCheck / TestExecution）」；④ `workflow/src/state.rs:313` `StepNode::ArchiCheck` doc：「定向反馈边、零进程」→「按 `on_fail` 两态：`FeedbackLoop`（implement）/ `Terminal`（initialize），零进程」；⑤ `layout/mod.rs:58` `ReportSlot::Baseline` doc：「两检查步」→「static-check / test-execution 两步（archi 基线检查走 `Archi` 槽位）」，`:48-53` 类型 doc 补一句「`Archi` 由 initialize 基线检查与 implement 门禁两挂载点共用」；⑥ `walker.rs:477-480` 区段注释：步序四步 + 三检查步；⑦ `port.rs` 的 `ArchiCheckRunner` trait doc：「**四参显式化**」→「**五参显式化**」并把 `mode` 插入形参说明（`root` / `name` / `mode` / `changed` / 末位取消）——「报告槽位不设形参」一句逐字保留 | 漏一处即同一事实两种自称（本仓历史高频病灶）；③④⑤⑦ 四处均**非出线类型**（实测均不在 `bindings.ts`），零生成物 diff |
| D12 | 零改动确认面 | `infra/archi/src/report.rs`、`core/archi/src/{drift,scan,dto}/**`、`workflow/src/state.rs` 的 `RunStepKind` doc、`orchestration/src/state.rs:96` `ChangeStepKind` doc、`workflow/src/state.rs:44` `StepKind` doc、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs`、`packages/desktop/src/views/changes/flow/**`、`crates/core/workflow/src/prompts/**`、`.dev-team/**`、`crates/core/config/**` | 逐条对应提案「不要修改」与 AC-9 / AC-11；`RunStepKind` / `ChangeStepKind` 的 doc 直出 `bindings.ts`，动它就制造无意义生成物 diff；装配面 `LocalToolSteps::new` 的四 runner 形参与实参都不变（第四 runner 仍是 `LocalArchiCheck`） |
| D13 | 版本 | `packages/desktop/package.json` `version` `0.6.47` → `0.6.48`（单行）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随（零手改）；`src-tauri/Cargo.toml` crate 版本行 `0.1.0` **不动** | 仓库版本纪律 |
| D14 | 明确不做（出 range） | ① 判定规则（扫描 / 归属 / 定级 / 豁免 / 报告形状）零改动；② 零新 IPC / 零新出线类型 / `ArchiCheckMode` 不进 specta；③ 不把 `api:check` 挂进 initialize（已在基线 static-check 的 `pnpm check` 链内）；④ 报告不改落 `reports/baseline`、不新增第四槽位；⑤ 不为 initialize 引入可配置开关 / 按工作区豁免；⑥ 前端零改动（`STEP_LABEL.archiCheck` / 归一 / 分组徽章已在案）；⑦ 基线红时不自动修复模型或代码；⑧ gate 档位 ratchet 不同批 | 逐条对应提案 §六；每条都是「另一件事」的边界，混入即扩大 AC 面 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 挂载点声明（**改**） | 两表表头四步序（`ArchiCheck(Terminal)` 插在 static-check 与 test-execution 之间）；单源 static | `crates/core/workflow/src/write/phase_table.rs` | `workflow::state::{StepNode, OnFail}` | 编译期 `const` 表 |
| port 词汇与载荷（**改**） | `ArchiCheckMode`（封闭两值 + `as_str`）；`ToolCommand::ArchiCheck { change_id, mode }`；`ArchiCheckRunner::run(root, name, mode, changed, cancel)` | `crates/core/orchestration/src/port.rs` | `foundation::layout::ReportSlot`（既有）、`workflow::write` | 纯进程内封闭集（零 serde / 零 specta） |
| 命令消费点（**改**） | 按 `mode` 分派变更集取数：全量 = 空清单 + 零 vcs；增量 = `change_edit_set` 且失败显式传播 | `crates/core/orchestration/src/steps.rs` | `crate::port::{ArchiCheckMode, ToolCommand}`、`ArchiveVcsPort` | 异步 `BoxToolFuture` |
| 两挂载点执行体（**改**） | `step_archi_check` 按 `on_fail` 三分派；`step_archi_check_terminal` 单发终态（fault 先行 / passed 收口 / failed `Terminal`）；`step_archi_check_feedback` 承接既有反馈边体；`TXT_BASELINE_ARCHI_FAILED` | `crates/core/orchestration/src/walker.rs` | 同上 + `RunGuard` / `StepSettle` / `diagnose_brief` / `settle_fault` | 既有 walker 执行体范式 |
| 门禁执行体（**改**） | `mode` → `ArchiDriftMode` 唯一映射 → `drift::check` → 落 `reports/archi` → 派生 `passed` / `diagnostics` / `report_dir` / `report_ref` | `crates/infra/archi/src/runner.rs` | `archi::drift`、`foundation::layout`、`orchestration::port` | 零进程、零 vcs |
| 报告落盘单点（**零改**） | 目录创建 + 文件名 + 序列化写盘 | `crates/infra/archi/src/report.rs` | `foundation::layout::{change_reports, ReportSlot}` | `std::fs` |
| 槽位派生（**零逻辑改**） | `ReportSlot::Archi` 供两挂载点共用；`Baseline` doc 收敛 | `crates/core/foundation/src/layout/mod.rs` | — | 既有 |
| 词汇面（**doc-only 改**） | `OnFail::Terminal` / `StepNode::ArchiCheck` 的 doc 补第三消费点与两态分派 | `crates/core/workflow/src/state.rs` | — | 非出线类型 |
| 装配面（**零改**） | `LocalToolSteps::new` 四 runner + vcs + store + run_id 形参与实参不变 | `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | — | 既有组合根 |
| 前端呈现面（**零改**） | `archi_check` 节点由后端 emit 序驱动上图；`STEP_LABEL.archiCheck` / 归一 / 分组徽章已在案 | `packages/desktop/src/views/changes/flow/**` | — | React（零改动） |

依赖方向零新增边：`core/workflow` ⇄ `core/archi` 的互不依赖不动（`workflow` 侧零 `archi` 词汇）；`infra/archi` 只多一个既有依赖 crate 内的 port 名字（`orchestration::port::ArchiCheckMode`）；`core/orchestration` 零新增依赖（`ArchiCheckMode` 是本 crate 自有类型）；前端零 npm 依赖。

---

## 变更清单 (Change Inventory)

<!-- 新增文件：本变更新增 0 文件——全部改动落在既有文件内（无新 crate、无新模块、无新 IPC、无新表）-->

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/workflow/src/write/phase_table.rs` | `INITIALIZE_STEPS` 在 `StaticCheck` 与 `TestExecution` 两表项之间插 `StepDefinition { node: StepNode::ArchiCheck, prompt_ref: None, on_fail: Some(OnFail::Terminal) }`（`StaticCheck` / `TestExecution` / `Clean` 三步项逐字不动）；`:107-112` doc 改述为四步序 + 三检查步 `Terminal` + 插入理由一句；`:376-378` initialize `PhaseDefinition` 注释「三步」→「四步」 | D1 / D11 / AC-1 |
| `crates/core/orchestration/src/port.rs` | 新增 `ArchiCheckMode`（两值 + `as_str`，紧贴 `ArchiCheckOutcome` 之前的类型区）；`ToolCommand::ArchiCheck` 加 `mode: ArchiCheckMode` 字段（第 2 位）并改述 variant doc；`ArchiCheckRunner::run` 加 `mode` 形参（第 3 位）并改述 trait doc（补「模式由消费面声明，runner MUST NOT 推断」与「两挂载点同树落 `reports/archi`」两句；把「**四参显式化**」改述为「**五参显式化**」并把 `mode` 插入形参清单 = `root` / `name` / `mode` / `changed` / 末位取消，「报告槽位不设形参」一句逐字保留）；`ArchiCheckOutcome` 五字段逐字不动 | D2 / D3 / D4 / D11 / AC-2 |
| `crates/core/orchestration/src/steps.rs` | `use crate::port::{…}` 补 `ArchiCheckMode`；`ToolCommand::ArchiCheck { change_id, mode }` 臂内 `match mode` 分派变更集（`Full` → `Vec::new()` 且零 vcs 调用；`Incremental` → 既有 `change_edit_set(..., "HEAD")?`），`archi.run(&step.root, &name, mode, changed, cancel)`；`resolve_change_name` 与审计行 `StepKind::ArchiCheck` 摘要逐字不动 | D5 / AC-3 |
| `crates/core/orchestration/src/walker.rs` | ① `use crate::port::{…}` 补 `ArchiCheckMode`；② 新增常量 `TXT_BASELINE_ARCHI_FAILED` 并改述 initialize 词汇区注释（三态 → 四条）；③ `step_archi_check` 改为 `match step.on_fail` 三分派（`Terminal` → 新执行体 / `FeedbackLoop` → 既有体 / `Escalate` / `None` → 计划图漂移 `Err`）；④ 既有反馈边实现体原样搬入 `step_archi_check_feedback`（`run_tool` 携 `mode: Incremental`）；⑤ 新增 `step_archi_check_terminal`（同 `step_static_check_terminal` 型，携 `mode: Full`、`identity: None`、`template: None`、`TXT_BASELINE_ARCHI_FAILED`）；⑥ `:477-480` 区段注释改述为四步序；⑦ `feedback_limit` / `step_clean` / `step_static_check*` / `step_test_execution*` 逐字不动 | D6 / D7 / D8 / D11 / AC-5 / AC-6 |
| `crates/core/workflow/src/state.rs` | 仅两处 doc：`OnFail::Terminal`（`Terminal` 消费点补 `ArchiCheck` → 三检查步）、`StepNode::ArchiCheck`（「定向反馈边、零进程」→「按 `on_fail` 两态：`FeedbackLoop` / `Terminal`，零进程」）；`RunStepKind` 与其 doc、`StepKind` 与其 doc 逐字不动 | D11 / D12 / AC-10（两处均非出线类型，`bindings.ts` 零 diff） |
| `crates/core/foundation/src/layout/mod.rs` | 仅 doc：`ReportSlot::Baseline` 变体注释收敛为「initialize 相位 static-check / test-execution 两步（archi 基线检查走 `Archi` 槽位）」；`ReportSlot` 类型 doc 补一句「`Archi` 由 initialize 基线检查与 implement 门禁两挂载点共用」；`report_dir_name` / `change_reports` 等逻辑逐字不动 | D11 / AC-5（槽位语义不变） |
| `crates/infra/archi/src/runner.rs` | `use orchestration::port::{…}` 补 `ArchiCheckMode`；`impl ArchiCheckRunner for LocalArchiCheck` 的 `run` 加 `mode` 形参并新增私有 `fn drift_mode(mode: ArchiCheckMode) -> ArchiDriftMode`（唯一映射），`check(CheckInput { mode: drift_mode(mode), changed: &changed, … })`；落盘 / 降级 / 取消 / `compose_text` / `passed` 派生逐字不动 | D9 / D10 / AC-4 |
| `packages/desktop/package.json` | `"version": "0.6.47"` → `"0.6.48"`（单行） | D13 / AC-11 |

### 机械闭合面（测试文件签名跟随，零用例语义改动）

> 以下五处是 `cargo check --all-targets` 的**编译闭合面**（port 签名与载荷变更的机械扩散，R-4），不是用例编写：只补 `mode` 形参 / 字段 / 模式臂 / 实参位，**零用例新增、零断言删除、零断言语义改写**。断言级改写（步序 3→4、四节点 emit 序、archi 红 / `skipped` / 取消 / 漂移用例、`as_str` 对拍、全量落盘用例、「恒增量模式」断言改述、`port_test.rs` 的 `mode` 透传捕获与「四参」措辞改述）归测试相位，见文末「测试面归属边界」。

| 文件 | 机械修改 |
|---|---|
| `crates/core/orchestration/src/walker_test.rs` | `NullArchiRunner`（`:5138`）与 `CountingArchiRunner`（`:14743`）的 `run` 签名补 `mode` 形参（忽略即可，`_mode`）；其余 `ToolCommand::ArchiCheck { .. }` 模式臂（`:801` / `:966` / 各 `matches!` 点 / `:16128`）零改动 |
| `crates/core/orchestration/src/steps_test.rs` | `UnusedArchiRunner`（`:623`）与 `RecordingArchiRunner`（`:4162`）的 `run` 签名补 `mode`；13 处构造点补 `mode: ArchiCheckMode::Incremental`（`:4251` / `:4289` / `:4320` / `:4349` / `:4376` / `:4411` / `:4459` / `:4498` / `:4530` / `:4567` / `:4603` / `:4833` / `:5055`）；`:5066` 模式臂改 `{ change_id, .. }` |
| `crates/core/orchestration/src/port_test.rs` | ① `RecordingArchiCheck`（`:3291`）的 `impl crate::port::ArchiCheckRunner` 的 `run` 签名补 `mode: ArchiCheckMode`（第 3 位；机械下限用 `_mode` 忽略——`type ArchiCall` 四元组、`:3299` 起的 `push((root, name, changed, cancel.is_set()))` 与 `:3340` 起的捕获断言因此零改动）；② 两处 UFCS 调用点在第 3 实参位插 `ArchiCheckMode::Incremental`：`:3325`（`ArchiCheckRunner::run(runner.as_ref(), "/ws", "demo", changed.clone(), CancelFlag::never())` → `"demo"` 与 `changed.clone()` 之间插 `mode`）与 `:3355`（`run(runner.as_ref(), "/ws", "demo", Vec::new(), cancel)` → 同位置插入）；③ `use crate::port::{…}` 补 `ArchiCheckMode`；④ `:249` 的 `ToolCommand::ArchiCheck { change_id }` 模式臂改 `{ change_id, .. }`（`describe` 臂 `:231` / `:1042` 已是 `{ .. }`，零改动） |
| `crates/infra/archi/src/runner_test.rs` | 3 处 `.run(...)` 调用补 `mode` 实参（既有语义 = 增量 ⇒ `ArchiCheckMode::Incremental`），使 `:336` 起的「恒增量模式」断言仍成立 |
| `crates/infra/checks/src/mutation/mod_test.rs` | `UnusedArchiSeam::run`（`:1527`）签名补 `mode` 形参（该臂恒 `Err`，用例断言不变；checks 域**实现**零改动） |

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `ArchiCheckMode::as_str` | `crates/core/orchestration/src/port.rs` | 新增 | `pub fn as_str(self) -> &'static str` | 出线词 `full` / `incremental`，与 `ArchiDriftMode::as_str` 同词面；零 serde / 零 specta |
| `ArchiCheckRunner::run` | `crates/core/orchestration/src/port.rs` | 修改 | `fn run(&self, root: &str, name: &str, mode: ArchiCheckMode, changed: Vec<String>, cancel: CancelFlag) -> BoxToolFuture` | `mode` 第 3 位（D4）；产出仍 `ToolStepOutput::ArchiCheck(ArchiCheckOutcome)`；取消以 `fault = Stopped` 正常返回、写盘失败降级 `report_ref = None` 两条不变量逐字保留 |
| `impl ArchiCheckRunner for LocalArchiCheck::run` | `crates/infra/archi/src/runner.rs` | 修改 | 同上（trait 实现的跟随形态） | 体只多一处 `mode` → `ArchiDriftMode` 映射（D10） |

> 私有执行体 `step_archi_check` / `step_archi_check_terminal` / `step_archi_check_feedback`、私有常量 `TXT_BASELINE_ARCHI_FAILED` 与 `static INITIALIZE_STEPS` 均为 crate 内私有面，按本表口径不列——其行为契约由两份 delta spec 与 D1 / D6 / D7 / D8 承载。

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `ArchiCheckMode` | `crates/core/orchestration/src/port.rs` | 新增 | `pub enum ArchiCheckMode { Full, Incremental }`，derive `Debug, Clone, Copy, PartialEq, Eq`（零 serde / 零 specta / 无 `ALL`）；`as_str` 两值；port 词汇单点，映射只在 `infra/archi` 一处 |
| `ToolCommand` | 同上 | 修改 | `ArchiCheck { change_id: String }` → `ArchiCheck { change_id: String, mode: ArchiCheckMode }`（`mode` 第 2 位）；其余十变体与 `derive` 集零改动 |
| `ArchiCheckOutcome` | 同上 | 零改动 | 五字段面不变（`passed` / `diagnostics` / `fault` / `report_dir` / `report_ref`）——`skipped` / `noChanges` 的逐字文案由 `diagnostics` 承载，无需新字段 |
| `ArchiCheckRunner` | 同上 | 修改 | trait 方法签名加 `mode`（见公共函数表）；trait doc 补「模式由消费面声明、runner MUST NOT 推断」与「两挂载点同树落盘」两句 |
| `ReportSlot` | `crates/core/foundation/src/layout/mod.rs` | doc-only | 三值封闭集与 `report_dir_name` 映射零改动；仅 `Baseline` / 类型 doc 措辞收敛（D11） |

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.48`（现值 `0.6.47`，+1 patch） | `src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随（零手改）；`src-tauri/Cargo.toml` crate 版本行 `0.1.0` 不动 |

> 配置项零新增：`.dev-team/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像与 `KNOWN_KEYS`）、`tauri.conf.json`、`capabilities/default.json` 全部零 diff；门禁挂载点不设任何运行时开关（无配置键、无环境变量、无相位名判断）。

<!-- 路由 / API 设计：不涉及——本变更零新 IPC 命令、零新出线类型、零 HTTP 端点；`ArchiCheckMode` 是纯进程内 port 词汇（零 specta），`bindings.ts` / `bindings.zod.ts` 生成物零 diff（AC-9） -->

---

## 验收标准回应

| AC | 落点 |
|---|---|
| AC-1 | `phase_table.rs` 的 `INITIALIZE_STEPS` 四步序（D1）+ 两表 initialize 条目共享同一 static（指针等值不变） |
| AC-2 | `port.rs` 的 `ArchiCheckMode`（两值 + `as_str`）、`ToolCommand::ArchiCheck { mode }`、`ArchiCheckRunner::run` 的 `mode` 形参（D2 / D3 / D4）；封闭性由穷尽 `match` 编译期锚 + `port_test` 逐值对拍 |
| AC-3 | `steps.rs` 的 `match mode` 分派：全量臂零 vcs 调用 + 空清单，增量臂逐字保留取数失败传播（D5） |
| AC-4 | `infra/archi/runner.rs` 的模式映射 + 既有 `report::write_report` 落 `reports/archi`（D9 / D10）；`reports/baseline` / `reports/test` 零触碰 |
| AC-5 | `step_archi_check_terminal` 三态收口 + `TXT_BASELINE_ARCHI_FAILED`（D7 / D8）；`skipped` / `noChanges` 文案由 runner 的 `compose_text` 原样透出（零改动） |
| AC-6 | 终态臂零 `run_worker` / 零 `feedback` 边 / 零 fail checklist（D7：只 `emit_node` + `Terminal`）；`Terminal` 语义天然保证 PhaseRecord 零 initialize 条目 |
| AC-7 | 新步由 emit 序驱动（`begin_node` + `emit_node`，`EdgeKind::Forward`）；相位机步迭代与收口逻辑零改动（`settle_phase` 判据仍是「steps 是否含 TestExecution 步」，本步不引入分支） |
| AC-8 | 实证留痕（不改代码）：本仓全量判定现状 `status = clean`（提案「本仓基线实测」表）；`gate` 档位 ratchet 出 range（D14 ⑧） |
| AC-9 | D12 零改动确认面 + 事实校准 5 / 7（`bindings.ts` / `.dev-team/**` / `api:check` 的零 diff 依据） |
| AC-10 | D11 的七处改述 + D12 的「零改动」注释（`RunStepKind` / `ChangeStepKind` / `StepKind`）与 D8 的四条互不共用文案；非测试源码面（`phase_table.rs` / `port.rs` / `state.rs` / `layout/mod.rs` / `walker.rs`）由本相位清零，测试文件内的同口径串随断言改写归测试相位（见文末「测试面归属边界」） |
| AC-11 | D13 版本 + 既有门禁链（`pnpm -C packages/desktop run check`、`cargo nextest run --workspace`）由桌面端执行；本设计不重复其内容 |

---

## 数据模型 (Data Model)

### port 词汇（进程内，非持久化）

| 类型 | 值 | 语义 | 持久化 |
|---|---|---|---|
| `ArchiCheckMode` | `Full` | 扫整个观察根（零变更集），initialize 基线检查 | 否（`ToolCommand` 载荷仅进程内流转，不落库、不落报告字段以外的任何面） |
| | `Incremental` | 只扫 change 编辑集（`change_edit_set(root, "HEAD")`），implement 门禁 | 否 |
| `ToolCommand::ArchiCheck` | `{ change_id, mode }` | 命令载荷 | 否（audit 行只记 `passed` 与 `report_dir`，形状零改动） |
| `ArchiCheckOutcome` | 五字段（不变） | 门禁产出 | 否（只在进程内消费） |

### 报告（`ArchiDriftReport`，落 `reports/archi/<timestamp>.json`）

- **字段面零改动**（`version = 3` 不变，14 字段 + `apiFindings` 并列槽位不变）。
- `mode` 字段：`full`（initialize 基线）/ `incremental`（implement 门禁）——同一棵树、同一命名规则，靠报告体自述与文件名时间戳区分。
- `scan.changedFiles`：全量为 `0`（全量忽略变更集）、增量为变更数；`scan.scannedFiles` 为通过排除约定的源文件数。
- `status` 五值推导与模式无关（`skipped` / `noChanges` 在全量下同样可能出现）；`passed = errors.is_empty() && model.structuralErrors.is_empty()` 两挂载点完全相同。

### 持久化与路径

| 面 | 落点 | 本次变化 |
|---|---|---|
| 漂移报告 | `<change>/reports/archi/<YYYYMMDDTHHMMSSZ>.json`（`ReportSlot::Archi` 单点派生） | 新增写者（initialize 全量），目录与命名规则不变 |
| checks 报告 | `<change>/reports/baseline/**`、`<change>/reports/test/**` | 零变化（三槽位分树边界不动） |
| db 表 / 字段 | `run_node` / `run_step` / `change_record` 等 | 零变化（`archi_check` 已在 `RunStepKind` 二十二值内，无新列） |
| 工作区配置 | `.dev-team/config.json` | 零变化（挂载点不可配置） |

---

## 依赖 (Dependencies)

### 运行时依赖

零新增：`core/orchestration` 只多一个自有类型（`ArchiCheckMode`）；`infra/archi` 已有 `orchestration` 依赖（本次只多用 `port::ArchiCheckMode` 一个名字）；`core/workflow` / `core/foundation` 零依赖变化（两文件只有 doc 改动）；`core/archi`（判定域）零改动。前端零 npm 依赖变化。

### 构建 / 测试依赖

零新增：无 dev-dependency 变化（`infra/archi` 既有 `tempfile` 够用）；无新工具链、无 `package.json` 脚本变化。

---

## 待决问题 (Open Questions)

1. **「定向反馈边」措辞残留**：`orchestration/src/state.rs:96`（`ChangeStepKind::ArchiCheck`，直出 `bindings.ts:1495`）与 `workflow/src/state.rs:44`（`StepKind::ArchiCheck`，未出线）的 variant doc 仍写着「定向反馈边」。本变更按 AC-9（生成物零 diff）**不动**前者、按最小面 **不动**后者。是否在后续「会重生成 bindings」的变更里一次性统一改述，留待独立裁决（AC-10 的 `grep` 判据不覆盖措辞）。
2. **全量扫描在超大仓库的时延**（提案 R-2）：本变更不放宽任何判据或预算；若实测超 `desktop-archi-drift`「运行期预算与语料」的 p95 预算，「按元素根分段」需另立变更——本变更内 MUST NOT 引入降级分支。

---

## 测试面归属边界

本设计只交付源码面与编译闭合面，**不含测试策略**。归属口径：

- **本相位（实现）承接**：上文「机械闭合面」表五处——只有 `mode` 形参 / 字段 / 模式臂 / 实参位的编译跟随，`cargo check --all-targets` 跑绿即止。
- **测试相位承接**（test-design / test-gen / test-execution）：`phase_table_test.rs` 的 `INITIALIZE_STEP_ORDER` 3→4、步数与 `expected_on_fail` 表、`Terminal` / `Escalate` 归属守卫、步序串断言；`walker_test.rs` 的 initialize 全绿链四节点 emit 序与 archi 红 / `skipped` / 取消 / `Escalate` 漂移用例；`steps_test.rs` 的全量零 vcs 用例；`port_test.rs` 的 `as_str` 逐值对拍，以及 `RecordingArchiCheck` 的 `mode` 透传捕获（本相位以 `_mode` 忽略，捕获元组仍四元）与随之而来的「四参」措辞改述（假件 doc `:3268`「捕获 `run(root, name, changed, cancel)` 四参」、`ArchiCall` doc `:3270`「捕获的四参」、测试 doc `:3310`、测试名 `:3313` `archi_check_runner四参透传_changed保序且零slot形参`、断言消息 `:3343`「四参逐值到达」）；`infra/archi/runner_test.rs` 的「恒增量模式」断言改述与全量落盘用例。前端零用例新增（零代码改动）。
- **AC-10 的 grep 终态**：`grep -rn '两检查步'` 在 `crates/` 下的残留只在测试文件（`steps_test.rs` / `walker_test.rs` / `phase_table_test.rs` 的注释与断言消息串），本相位**不动**测试文件注释（纯编译闭合是硬线），该口径串随测试相位的断言改写一并清零；非测试源码面（`phase_table.rs` / `port.rs` / `state.rs` / `layout/mod.rs` / `walker.rs`）由本相位清零（D11）。

