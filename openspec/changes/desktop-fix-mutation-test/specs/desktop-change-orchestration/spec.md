# desktop-change-orchestration Specification (Delta)

## ADDED Requirements

### Requirement: MutationTest 步接线与独立反馈预算

walker SHALL 在 test-execution 相位按 `TEST_EXECUTION_STEPS` 步序列（`PhaseStart → TestExecution → MutationTest → PhaseLog`，定义见 desktop-workflow-write-face）顺序执行：TestExecution 结论 pass 后才执行 MutationTest 门禁步（`ToolCommand::MutationTest` 经 `MutationRunner` port 由 checks 边界 infra 实现），TestExecution fail / error 走既有反馈边修复重入——「测试 / 覆盖率修绿后再执行突变」由步序列顺序执行天然保证，walker MUST NOT 在 TestExecution 未 pass 时发起突变 spawn。MutationTest 失败（中断 / 未达标）SHALL 走**独立定向反馈边**（反馈边②，报告指引式口径，见 desktop-mutation-testing）：注入本相位 executor 会话修复（补测试杀突变）——反馈边注入对象分角色清单随本条扩为 static-check / test-execution / mutation-test 失败注入 executor、verdict 解析失败注入 evaluator。反馈边② SHALL 经 `feedback_limit(step, "mutation-test")` 独立计数、上限 5（与 static-check / test-execution 反馈计数器分立，各自计满各自升格），MUST NOT 计入相位 retry 预算；超限 walker SHALL 代写 fail checklist 经写面 `phase_log` 升格相位 fail（不跑 evaluator，进入重试 / 决策路径）。`step_phase_log` 的 test 分支 SHALL 消费 `test_outcome` + `mutation_outcome` 双槽（机械 checklist 双槽，见 desktop-test-execution）；archive 分支双槽恒 None（三分流按「产物在否」分离语义不变）。

#### Scenario: 拆步时序绿后才突变

- **WHEN** TestExecution 首次 fail、经反馈边修复重入 pass 后 walker 推进
- **THEN** MutationTest 仅在 TestExecution pass 后执行；修复期间零突变进程 spawn，步序列零乱序

#### Scenario: 反馈边②独立计数与超限升格

- **WHEN** MutationTest 未达标连续反馈修复 5 次仍不达标
- **THEN** 反馈边②计数独立（与 static-check / test-execution 分立）且不消耗相位 retry 预算；第 5 次后 walker 代写 fail checklist 升格相位 fail，不再注入反馈边

#### Scenario: 双槽分支分离不误判

- **WHEN** 分别驱动 test-execution 相位（test_outcome + mutation_outcome 双槽产物）与 archive 相位（双槽恒 None）
- **THEN** `step_phase_log` test 分支消费双槽机械 checklist，archive 分支仅命中空 checklist 分支，互不误判

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs`（Component） | MutationTest 步接线 | 步序列顺序执行（TestExecution 绿后才突变）；`feedback_limit(step, "mutation-test")` 独立计数上限 5；超限代写 fail checklist 升格相位 fail |
| `crates/core/orchestration/src/port.rs`（API） | 三件套封闭集扩展 | `ToolCommand::MutationTest { change_id }` / `ToolStepOutput::MutationTest(载荷)` / `MutationRunner` port（实现落 infra/checks，core/orchestration 零依赖 checks） |
| `crates/core/orchestration/src/steps.rs`（Function） | id → name 解析接线 | 与 test-execution 分支同式（`store.get_change(id).map(name)`）；清单经 vcs 缝取数注入 runner |
| `crates/core/workflow/src/write/phase_table.rs`（Component） | 步序列单源 | `TEST_EXECUTION_STEPS` 增 MutationTest 步（`on_fail=FeedbackLoop`）；requirement / test-only 两表同式 |
