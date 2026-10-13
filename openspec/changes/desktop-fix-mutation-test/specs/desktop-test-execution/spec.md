# desktop-test-execution Specification (Delta)

## ADDED Requirements

### Requirement: MutationTest 确定性门禁步与时序

Walker SHALL 在 test-execution 相位以独立 MutationTest 门禁步（`ToolCommand::MutationTest`，经 `MutationRunner` port 由 checks 边界 infra 实现）承接突变测试，步序列拆为 `PhaseStart → TestExecution → MutationTest → PhaseLog`：TestExecution 结论 pass（测试 / 覆盖率修绿）后才执行 MutationTest——「修复后再执行突变」由 walker 顺序执行天然保证，TestExecution fail / error 走既有反馈边修复重入（绿了才走到 MutationTest）。MutationTest 绿跑零 WorkerAgent 会话；结论 pass 时相位以机械 checklist 代写 `phase_log`（verdict=pass）推进；中断（段执行 error / 超时）与未达标走「双时点反馈边」（见 desktop-mutation-testing），失败步 `on_fail=FeedbackLoop` 独立预算。runner 以 `Err` 收场（突变程序不可达 / 配置不支持 / 写盘失败）SHALL 停给用户（显式失败，与 WorkerAgent 失败同等呈现）。MutationTest 载荷 `ToolStepOutput::MutationTest` SHALL 为 conclusion + score / threshold + 计数 + 报告路径清单 + 反馈文本，载荷零 Serialize 不进 IPC（与 TestExecution 同式）；`ChangeStepKind::MutationTest` SHALL 入步词汇封闭集（18→19），经 `RunUpdate::Step` 上图（前端流程视图现有步骤渲染直接吃，零渲染改动）。

#### Scenario: 绿后才跑突变

- **WHEN** TestExecution 首次 fail（反馈边修复重入）后 pass，walker 走到 MutationTest
- **THEN** MutationTest 在 TestExecution pass 后才执行（修复期零突变 spawn）；MutationTest 节点随 `RunUpdate::Step` 上图

#### Scenario: runner 显式失败停给用户

- **WHEN** cargo-mutants / npx stryker 程序不可达或配置不支持
- **THEN** runner 显式 `Err`，图上 MutationTest 节点呈失败态，run 不静默越过、不产出部分结论

### Requirement: 机械 checklist 双槽与 evidence 三态

PhaseLog 机械 checklist 产物面 SHALL 挂 `test_outcome` + `mutation_outcome` 双槽（两步结论独立成槽）。机械 checklist 第 3 条「mutation null 自动通过」判定语义 SHALL 保留（null 观察期不收紧，与 evaluator T3 一致），但 evidence SHALL 区分三态：「跑过且达标（score / threshold）/ diff 空跳过（突变面为空）/ null（未跑）」。`mutation: null` SHALL 保持合法（diff 空跳过 / 未跑场景不判 fail）。

#### Scenario: 三态 evidence 落账

- **WHEN** MutationTest 跑过且达标后 PhaseLog 代写机械 checklist
- **THEN** 第 3 条 pass、evidence 为「跑过且达标（score X ≥ threshold Y）」形态；diff 空跳过场景 evidence 为「diff 空跳过」、null 场景为「null（未跑）」，三态可辨

#### Scenario: null 合法不判 fail

- **WHEN** 突变面为空（change 编辑集不含可突变文件）或 MutationTest 未执行
- **THEN** summary mutation 位为 null，机械 checklist 第 3 条 pass（观察期语义不变），相位照常推进

## MODIFIED Requirements

### Requirement: 执行链与报告落盘

执行链 SHALL 依序：detect（config tests[] + glob → plan entries）→ 完整性校验（字段 / 计数 / conclusion 一致性 checklist）→ 复用门 → 逐 suite 命令模板展开（`{results_file}` / `{report_dir}` / `{config_args}` / `{files}`）spawn（超时 + 输出捕获）→ 报告写盘 → 聚合（阈值 / override → conclusion）→ 诊断 findings。报告 SHALL 恒落 change 报告目录（`summary.json` + 逐 suite 子报告；路径经 foundation layout 推导，desktop 产品源码零磁盘路径字面量）；报告 MUST NOT 登记 file inventory / artifacts registry（MVP 口径）。突变面 SHALL 由独立 MutationTest 门禁步承载（见「MutationTest 确定性门禁步与时序」与 desktop-mutation-testing）：TestExecution 执行链不产突变结果；summary 的 `mutation` 位由 MutationTest 归并块回填（在位时），未跑 / diff 空跳过时保持 null（null 合法，见「机械 checklist 双槽与 evidence 三态」）。

#### Scenario: 全链绿跑落盘

- **WHEN** 全部 suite 执行成功且覆盖率达阈值
- **THEN** summary.json 与逐 suite 子报告落 change 报告目录，聚合 conclusion=pass，机械 checklist 可静态判定

#### Scenario: 报告路径经 layout 推导

- **WHEN** runner 写盘且对 desktop 产品源码做命名隔离扫描
- **THEN** 报告目录取自 foundation layout 推导（`reports/test` 挂 change 目录），零硬编码磁盘路径字面量，双禁令扫描通过

#### Scenario: 突变面独立步承载

- **WHEN** TestExecution 全链绿跑完成进入 MutationTest
- **THEN** TestExecution 产出不含突变结果（mutation 位由 MutationTest 归并回填或保持 null），两步报告各落其位

### Requirement: 步骤可观测与最小载荷

`ChangeStepKind::TestExecution` 与 `ChangeStepKind::MutationTest` SHALL 入步词汇封闭集（18→19，`mutation_test` 线词 1:1 全映射），门禁步状态经 `RunUpdate::Step` 上图（前端流程视图现有步骤渲染直接吃，零渲染改动）；IPC 类型随 bindings 再生成出线。`ToolStepOutput::TestExecution` 载荷面 SHALL 为 conclusion + 计数 + 诊断摘要 + 修复边明细文本 + 报告路径：摘要面服务步状态 detail（`diagnose_brief` 口径截断），明细面服务反馈边修复 prompt 直嵌（有界装配、pass 态恒空串）。`ToolStepOutput::MutationTest` 载荷面 SHALL 为 conclusion + score / threshold + 计数 + 报告路径清单 + 反馈文本（结论摘要式）。载荷 SHALL 零 Serialize 不进 IPC——明细文本 MUST NOT 经 bindings / `RunUpdate` 出线，只在进程内 walker 反馈边消费；全量 findings / 突变明细留报告文件，MUST NOT 经 IPC 面全量透传。

#### Scenario: 步骤上图零渲染改动

- **WHEN** 门禁步状态变化且 bindings 再生成
- **THEN** `RunUpdate::Step` 携 testExecution / mutationTest 步词汇流出，前端流程视图以既有步骤渲染直接消费

#### Scenario: 载荷双面各司其职

- **WHEN** 审查 `ToolStepOutput::TestExecution` 载荷
- **THEN** 无结构化 findings 全量数组字段：摘要面（`findings_brief`）走步 detail，明细面（`findings_detail`，有界文本）走反馈边修复 prompt；两者均不进 bindings / IPC，全量以报告文件为权威（`report_dir` 定位）

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续切片偿还，不由实现隐式吸收）：

1. **mutation 已移植（本变更偿还）**：突变由独立 MutationTest 步承载（js=Stryker / rust=cargo-mutants，见 desktop-mutation-testing）；`mutation: null` 保持合法（diff 空跳过 / 未跑，观察期），机械 checklist 第 3 条与 evaluator T3 null 语义不收紧——观察结果后再决定是否升格 error。
2. **agent 修复能力不进本期**：修阻塞错误（Step1b）与根因唯一性裁决（T4）不实现——红跑以 findings 呈现走反馈边或留给用户决策。
3. **net-zero 去噪不适用**：突变范围 = change 编辑集（`git diff HEAD` + untracked，`change_edit_set` 单点）；清单反推与 net-zero 去噪（CLI 先例）不移植——diff 面天然无「与 HEAD 同内容」噪音。
4. **无-change 场景不实现**：CLI `<root>/reports/test/` 分支桌面不实现（恒有 change）。
5. **报告不进 file inventory**：不做 artifacts registry 登记。

#### Scenario: 留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含 null 观察期与两反馈边口径并存的明知接受），后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs`（Component） | 相位步序列 | `TEST_EXECUTION_STEPS` = `[PhaseStart, TestExecution, MutationTest, PhaseLog]`（MutationTest `on_fail=FeedbackLoop`） |
| `crates/core/orchestration/src/port.rs`（API） | 突变步三件套 | `ToolCommand::MutationTest { change_id }` / `ToolStepOutput::MutationTest(结论摘要式载荷)` / `MutationRunner` port（与 `TestExecutionRunner` 并列）；载荷零 Serialize |
| `crates/core/orchestration/src/walker.rs`（Component） | 拆步接线 | TestExecution 绿后才进 MutationTest；机械 checklist 双槽（test_outcome + mutation_outcome）；反馈边②独立计数 label `"mutation-test"` |
| `crates/core/orchestration/src/state.rs`（API） | 步词汇扩展 | `ChangeStepKind::MutationTest` 入封闭集（18→19）；`RunUpdate::Step` 形态不变 |
| `crates/infra/checks/src/testexec/`（Component） | 测试执行链 | 突变面移出（不产突变结果、mutation 位不硬编码回填语义）；其余 detect / runner / report 链不变 |
