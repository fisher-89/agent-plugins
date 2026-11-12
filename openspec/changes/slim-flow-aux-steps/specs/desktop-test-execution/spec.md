# desktop-test-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: 失败即升格交决策（零 agent 修复）

test-execution 相位两步（TestExecution / MutationTest）结论 fail / error 时 walker SHALL 立即以固定三段升格相位 fail：桌面代写一条 fail checklist（`phase_log` 落账）→ 截断本相位余下步（`StepFlow::Upgraded`）→ 回 `phase_next` 进决策分叉。升格全程 MUST NOT 发起任何 WorkerAgent 会话（不注入修复、不续注会话、不自走重跑循环），裁决权 SHALL 交回决策 agent（backtrack / stop / ask）。升格落账的 `report` SHALL 携完整失败明细（test-execution = `findings_detail`，上限 `MAX_REPORT_CHARS = 2000` 字符；原反馈边修复 prompt 的明细面转喂决策信息面），evidence SHALL 携诊断摘要（`diagnose_brief` 口径）与报告目录（`report_dir`）——决策 agent 据此判根因并可自行 `read` 报告文件取全量。fail 条目 MUST NOT 携 executor / evaluator 会话槽位（本相位零会话；决策会话槽位由 `decision-log` 单独挂账）。runner 以 `Err` 收场（环境 / 配置 / 写盘问题）SHALL 仍按「test-execution 确定性门禁步与绿跑零 agent」的 Terminal 语义停给用户，MUST NOT 升格进决策（环境问题不该由决策 agent 裁决）。

升格路径的落账形态 SHALL 与相位收口共用同一落账入口（`settle_phase(PhaseSettle::Fail(PhaseFail { item, report, evidence, executor_session }))`，见 desktop-change-orchestration「相位入口与收口内化（计划图仅含真实步）」）：升格时由门禁执行体以 `Fail` 形态落账、不走 `Pass` 的机械 checklist 三分流；相位正常 pass 收口时走 `Pass` 形态。落账产物（`PhaseRecord` + `ChecklistItemRecord`）形状与既有逐字一致。

#### Scenario: 首败即升格零会话

- **WHEN** test-execution 相位首次执行且 runner 结论 `conclusion=Fail`（或 `Error`）
- **THEN** 零 `WorkerRole::Executor` / `WorkerRole::Evaluator` 请求、恰一条 `verdict=Fail` PhaseRecord（`executor` / `evaluator` / `decision` 三槽位均 None）、`TestExecution` 节点呈失败态、余下步（MutationTest）不执行、`drive` 的 `PhaseSettle::Pass` 收口零调用（fail 已由门禁执行体经 `PhaseSettle::Fail` 落账）；run 回 `phase_next` 进决策分叉

#### Scenario: 升格落账的 report 与 evidence 分工

- **WHEN** runner 产出超 200 字的 findings 明细后升格
- **THEN** fail 条目 `report` 携完整明细文本（≤2000 字符，超长按上限截断并留省略号），`checklist` 恰一条 `pass=false`、evidence 为 `diagnose_brief` 摘要 + 报告目录路径；决策 prompt 的「失败相位报告」段取该 `report`

#### Scenario: 同相位每次失败各自升格

- **WHEN** 决策 backtrack 回前置相位、修复后再次进入 test-execution 且再次 fail
- **THEN** 新落一条 fail 条目（无决策槽位），照常升格并触发一次新的决策分叉（幂等闸只作用于单条目：已挂决策会话的条目不再重复分叉）

### Requirement: MutationTest 确定性门禁步与时序

Walker SHALL 在 test-execution 相位以独立 MutationTest 门禁步（`ToolCommand::MutationTest`，经 `MutationRunner` port 由 checks 边界 infra 实现）承接突变测试，步序列为 `TestExecution → MutationTest`（开相与相位收口由相位机固定动作承载，不计入 `steps`，见 desktop-change-orchestration「相位入口与收口内化（计划图仅含真实步）」）：TestExecution 结论 pass（测试 / 覆盖率修绿）后才执行 MutationTest——「修复后再执行突变」由 walker 顺序执行天然保证；TestExecution fail / error 首败即升格（见「失败即升格交决策（零 agent 修复）」），相位内 MUST NOT 修复重入，只有决策 backtrack 重入并真重跑（复用门因输入变化失效）后才可能走到 MutationTest。MutationTest 绿跑零 WorkerAgent 会话；结论 pass 时相位以机械 checklist 由相位收口落账（verdict=pass）推进；中断（段执行 error / 超时）与未达标两时点均以升格收场（失败步 `on_fail=Escalate`，零会话，见 desktop-mutation-testing「失败升格交决策（报告指引式信息面）」）。runner 以 `Err` 收场（突变程序不可达 / 配置不支持 / 写盘失败）SHALL 停给用户（显式失败，与 WorkerAgent 失败同等呈现）。MutationTest 载荷 `ToolStepOutput::MutationTest` SHALL 为 conclusion + score / threshold + 计数 + 报告路径清单 + 反馈文本，载荷零 Serialize 不进 IPC（与 TestExecution 同式）；`ChangeStepKind::MutationTest` SHALL 入步词汇封闭集（18→19），经 `RunUpdate::Step` 上图（前端流程视图现有步骤渲染直接吃，零渲染改动）。

#### Scenario: 绿后才跑突变

- **WHEN** TestExecution 首次 fail（升格交决策）后经决策 backtrack 修复重入并 pass，walker 走到 MutationTest
- **THEN** MutationTest 在 TestExecution pass 后才执行（决策等待与修复期零突变 spawn）；MutationTest 节点随 `RunUpdate::Step` 上图；重入的 TestExecution 节点为独立 attempt / `instance_no`

#### Scenario: runner 显式失败停给用户

- **WHEN** cargo-mutants / npx stryker 程序不可达或配置不支持
- **THEN** runner 显式 `Err`，图上 MutationTest 节点呈失败态，run 不静默越过、不产出部分结论

#### Scenario: 相位内两节点无辅助步

- **WHEN** test-execution 相位一次绿跑收口后查询执行图
- **THEN** 该相位恰两个 step 节点（`TestExecution` / `MutationTest`）+ 串行 forward 边；无 `phaseStart` / `phaseLog` 节点，相位落账体现为 `PhaseRecord`（机械 checklist）与 `active_phase` 清位

### Requirement: 机械 checklist 双槽与 evidence 三态

相位收口的机械 checklist 产物面 SHALL 挂 `test_outcome` + `mutation_outcome` 双槽（两步结论独立成槽）。机械 checklist 第 3 条「mutation null 自动通过」判定语义 SHALL 保留（null 观察期不收紧），但 evidence SHALL 区分三态：「跑过且达标（score / threshold）/ diff 空跳过（突变面为空）/ null（未跑）」。`mutation: null` SHALL 保持合法（diff 空跳过 / 未跑场景不判 fail）。机械 checklist 的构造与落账由相位收口动作承载（`settle_phase` 的三分流之一），MUST NOT 依赖任何计划步节点。

#### Scenario: 三态 evidence 落账

- **WHEN** MutationTest 跑过且达标后相位收口代写机械 checklist
- **THEN** 第 3 条 pass、evidence 为「跑过且达标（score X ≥ threshold Y）」形态；diff 空跳过场景 evidence 为「diff 空跳过」、null 场景为「null（未跑）」，三态可辨

#### Scenario: null 合法不判 fail

- **WHEN** 突变面为空（change 编辑集不含可突变文件）或 MutationTest 未执行
- **THEN** summary mutation 位为 null，机械 checklist 第 3 条 pass（观察期语义不变），相位照常推进

#### Scenario: 落账不依赖辅助步节点

- **WHEN** test-execution 相位绿跑后审查图节点与落账产物
- **THEN** 机械 checklist 由相位收口一次落账（`PhaseRecord` + 三条 `ChecklistItemRecord`）；图上零 `phase_log` 节点、零 `StepKind::PhaseLog` 审计行；结论与 evidence 内容与本变更前逐字一致

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs`（`TEST_EXECUTION_STEPS`） | 步序列收窄 | `TestExecution → MutationTest` 两步（删两端 `PhaseStart` / `PhaseLog`）；`on_fail=Escalate` 逐字不变；两表共用同一 static |
| `crates/core/orchestration/src/walker.rs`（`step_test_execution_escalate` / `step_mutation_test_escalate`） | 升格落账入口 | 改调 `settle_phase(PhaseSettle::Fail(PhaseFail { item, report, evidence, executor_session: None }))`（原 `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` 退役）；report / evidence 装配文案逐字不变；随后 `StepFlow::Upgraded` |
| `crates/core/orchestration/src/walker.rs`（`settle_phase`） | 机械 checklist 落账 | `test_outcome` + `mutation_outcome` 双槽分支（`test_execution_phase_log_input` 逐字搬入）；三条目 pass、evidence 三态语义零变 |
| `crates/core/checks` / `crates/infra/checks`（**零改动**） | runner 面 | 三 runner 签名、报告槽位、`fault` 归因逐字不变 |
