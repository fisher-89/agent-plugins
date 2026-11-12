# desktop-mutation-testing Specification (Delta)

## MODIFIED Requirements

### Requirement: 失败升格交决策（报告指引式信息面）

MutationTest 失败 SHALL 一律升格相位 fail（零 WorkerAgent 会话，不注入修复、不续注会话），两时点同式：突变执行**中断**（执行 error / harness 崩 / 超时）与执行正常跑完但**未达标**（score < threshold）均以固定三步收场——代写一条 fail checklist（经相位收口落账入口）→ 截断本相位余下步（`StepFlow::Upgraded`）→ 回 `phase_next` 进决策分叉；达标 SHALL pass 直接收口（机械 checklist 由相位收口落账、零会话）。升格落账的 `report` SHALL 携完整反馈文本（`feedback_text`，上限 `MAX_REPORT_CHARS = 2000` 字符）：中断时为中断错误摘要 + 已产报告路径清单 + 保留的失败调试产物路径（保留面非空时），未达标时为 score / threshold 结论摘要 + 原始报告路径清单；错误摘要沿用 `ProcessOutput` 双段落口径（错误行组 + 末 50 行；见 desktop-exec-runtime）；evidence SHALL 携诊断摘要（`diagnose_brief` 口径）+ 报告目录。反馈内容 MUST NOT 内联存活突变体清单——决策 agent SHALL 依决策 prompt 的指引自行 `read` 原始 / 归并报告定位（报告指引式取向由决策信息面承继）。失败步 `on_fail` SHALL 为 `OnFail::Escalate`（与 test-execution 两步同），MUST NOT 声明 `FeedbackLoop`；失败条目三会话槽位恒 `None`。`OnFail::Escalate` 的归属守卫 SHALL 保证其只出现在 TestExecution / MutationTest 步上。

落账入口 SHALL 与相位收口共用（`settle_phase(PhaseSettle::Fail(PhaseFail { item, report, evidence, executor_session }))`，见 desktop-change-orchestration「相位入口与收口内化（计划图仅含真实步）」）：`item` = 「突变测试」、`executor_session` = `None`、report / evidence 装配逐字不变；落账产物形状（`PhaseRecord` + 一条 `ChecklistItemRecord`）与既有逐字一致，MUST NOT 依赖任何计划步节点。

#### Scenario: 中断立即升格

- **WHEN** 执行中 harness 崩溃（此前已有桶产出报告）
- **THEN** 立即升格相位 fail（不等其余桶跑完）：零 WorkerAgent 会话、一条 fail PhaseRecord（report 携中断错误摘要 + 已产报告路径清单 + 保留的失败调试产物路径清单（保留面非空时）），回 `phase_next` 进决策分叉

#### Scenario: 未达标升格

- **WHEN** 全部桶跑完且汇总 score 低于 threshold
- **THEN** 升格相位 fail：report 携 score / threshold 结论摘要与原始报告路径清单，MUST NOT 内联存活突变体明细；零会话请求

#### Scenario: 达标零会话收口

- **WHEN** 全部桶跑完且逐 suite 全部达标（或突变面为空跳过）
- **THEN** MutationTest pass，机械 checklist 由相位收口落账（`PhaseRecord` + 三条 `ChecklistItemRecord`），推进下一相位，零 WorkerAgent 会话发起；图上零 `phase_log` 节点

#### Scenario: 两口径并存红线随反馈边退役

- **WHEN** 审阅 test-execution 与 MutationTest 的失败处置表述
- **THEN** 两处均为「升格交决策」单一口径，无「有界内联自足 vs 报告指引式反向并存」红线（两反馈边均已不存在）；如未来重引相位内修复会话，须经用户重新裁决并显式提出 spec 变更

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test_escalate`） | 升格落账入口改调用 | 改调 `settle_phase(PhaseSettle::Fail(PhaseFail { item: "突变测试", report, evidence, executor_session: None }))`（原 `step_mutation_test_upgraded_phase_log` 退役）；`feedback_text` / 报告路径清单装配逐字不变；三会话槽位 None；随后 `StepFlow::Upgraded` |
| `crates/core/orchestration/src/walker.rs`（`settle_phase`） | 机械 checklist 落账 | mutation 双槽（`test_outcome` + `mutation_outcome`）与 evidence 三态语义逐字保留 |
| `crates/core/checks` / `crates/infra/checks`（**零改动**） | 突变面 | 四桶执行、报告落盘、sandbox 清理、`MutationRunner` 签名逐字不变 |
