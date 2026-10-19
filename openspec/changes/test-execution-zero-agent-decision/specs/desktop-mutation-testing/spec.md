# desktop-mutation-testing Specification (Delta)

## REMOVED Requirements

### Requirement: 双时点反馈边（报告指引式口径）

**Reason**: test-execution 相位内零 WorkerAgent 会话裁定下，突变失败「注入本相位 executor 会话补测试杀突变、`FeedbackLoop` 独立计数上限 5」的反馈边②整体退役。两时点（中断 / 未达标）仍各自落账，但落账后一律升格相位 fail、把裁决权交回决策面。「反馈边②为报告指引式口径、与 test-execution 反馈边①有界内联自足反向并存、MUST NOT 被后来者统一掉」的裁定随两反馈边一并失效。

**Migration**: 由「失败升格交决策（报告指引式信息面）」承接：`feedback_text`（中断错误摘要 / score 与 threshold 结论摘要 + 报告路径清单）改落升格 fail 条目的 `report` 供决策 prompt 消费，「明细由修复会话自行读报告定位」的取向，由决策 agent 依 prompt 指引用 `read` 读取报告文件承继。若未归档的 `desktop-mutation-filter-observability` 先落地，其 `ProcessOutput` 双段错误摘要（错误信息段 + 输出末尾 50 行）属生产者侧装配，随 `feedback_text` 原样转入升格 `report`，语义不变。

## ADDED Requirements

### Requirement: 失败升格交决策（报告指引式信息面）

MutationTest 失败 SHALL 一律升格相位 fail（零 WorkerAgent 会话，不注入修复、不续注会话），两时点同式：突变执行**中断**（段执行 error / harness 崩 / 超时）与全部段正常跑完但**未达标**（score < threshold）均以固定三段收场——代写一条 fail checklist → `phase_log` 落账 → 截断本相位余下步（`StepFlow::Upgraded`）回 `phase_next` 进决策分叉；达标 SHALL pass 直接收口（机械 checklist 落账、零会话）。升格落账的 `report` SHALL 携完整反馈文本（`feedback_text`，上限 `MAX_REPORT_CHARS = 2000` 字符）：中断时为中断错误摘要 + 已产报告路径清单，未达标时为 score / threshold 结论摘要 + 原始报告路径清单；evidence SHALL 携诊断摘要（`diagnose_brief` 口径）+ 报告目录。反馈内容 MUST NOT 内联存活突变体清单——决策 agent SHALL 依决策 prompt 的指引自行 `read` 段级 / 归并报告定位（报告指引式取向由决策信息面承继）。失败步 `on_fail` SHALL 为 `OnFail::Escalate`（与 test-execution 两步同），MUST NOT 声明 `FeedbackLoop`；失败条目三会话槽位恒 `None`。`OnFail::Escalate` 的归属守卫 SHALL 保证其只出现在 TestExecution / MutationTest 步上。

#### Scenario: 中断立即升格

- **WHEN** 第 2 段执行中 harness 崩溃（第 1 段已产报告）
- **THEN** 立即升格相位 fail（不等第 3 段跑完）：零 WorkerAgent 会话、一条 fail PhaseRecord（report 携中断错误摘要 + 第 1 段报告路径清单），回 `phase_next` 进决策分叉

#### Scenario: 未达标升格

- **WHEN** 全部段跑完且汇总 score 低于 threshold
- **THEN** 升格相位 fail：report 携 score / threshold 结论摘要与原始报告路径清单，MUST NOT 内联存活突变体明细；零会话请求

#### Scenario: 达标零会话收口

- **WHEN** 全部段跑完且逐 suite 全部达标（或突变面为空跳过）
- **THEN** MutationTest pass，机械 checklist 落账进入 PhaseLog，推进下一相位，零 WorkerAgent 会话发起

#### Scenario: 两口径并存红线随反馈边退役

- **WHEN** 审阅 test-execution 与 MutationTest 的失败处置表述
- **THEN** 两处均为「升格交决策」单一口径，无「有界内联自足 vs 报告指引式反向并存」红线（两反馈边均已不存在）；如未来重引相位内修复会话，须经用户重新裁决并显式提出 spec 变更

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/workflow/src/write/phase_table.rs` | 步声明 | `TEST_EXECUTION_STEPS` 的 MutationTest 步 `on_fail = Some(OnFail::Escalate)` |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test`） | 门禁步执行体 | 删反馈循环与 `fix_session`；`Escalate` 前置分支（fail / error 均升格）；保留 runner `Err → Terminal` |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test_upgraded_phase_log`） | 升格落账 | report = `feedback_text`（≤2000）、evidence = `diagnose_brief` + 报告目录、三会话槽位 None |
| `crates/core/orchestration/src/port.rs` | 载荷语义（字段面不变） | `MutationTestOutcome.feedback_text` 消费者由「反馈边修复 prompt」改为「升格 report」 |
| `crates/core/workflow/src/write/phase_table_test.rs` | 声明守卫 | `Escalate` 仅允许出现在 TestExecution / MutationTest 步的断言 |
