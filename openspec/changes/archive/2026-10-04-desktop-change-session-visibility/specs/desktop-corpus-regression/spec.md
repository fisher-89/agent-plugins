# desktop-corpus-regression Delta

## ADDED Requirements

### Requirement: 含会话槽位的新代际语料与 golden 对账

workflow.json eval 条目引入会话槽位字段（desktop-workflow-write-face 的 schema 演进）后，语料 SHALL 补充覆盖该字段面的样本：至少一个 eval 条目携带 executor / evaluator / decision 会话槽位的 workflow.json 样本入仓（样本来源——手工构造最小样本 vs 随首个含槽位归档 change 收录——由 design 定稿）。既有 fixtures 样本文件内容 MUST NOT 改写；DTO 增槽位键导致的 golden 快照 diff SHALL 沿既有显式再生成流程处置（`DESKTOP_GOLDEN_REWRITE=1` 覆写 → 复核一致），diff SHALL 人工确认预期范围（仅新增槽位键、其余投影不变）并留痕，MUST NOT 静默自动重写。「golden 更新是显式行为」的既有约束不变——本需求是对该流程的一次显式行使，非约束豁免。

#### Scenario: 新代际样本覆盖槽位字段

- **WHEN** 检查 `tests/fixtures/` 语料清单
- **THEN** 含至少一个 eval 条目携带会话槽位字段的样本，快照回归测试将其纳入全量解析

#### Scenario: golden 显式对账

- **WHEN** DTO 增槽位键后运行快照回归测试
- **THEN** 既有 fixture 的 diff 呈现预期差异（新增槽位键），经显式再生成后复核全绿；diff 确认记录在案

#### Scenario: 既有样本零改写

- **WHEN** 对比本次变更前后的 `tests/fixtures/` 样本文件内容
- **THEN** 既有样本（v2 / v1 / v0 及损坏样本）逐字节不变，仅 golden 快照与新增样本文件为增量

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/` | 语料增量 | 增含会话槽位字段的样本（来源 design 定稿）；既有样本文件零改写 |
| `crates/core/workflow/tests/golden/` | 快照对账 | `DESKTOP_GOLDEN_REWRITE=1` 显式再生成 → 复核一致；diff 人工确认仅新增槽位键并留痕 |
| `corpus_golden_test.rs` | 回归防线 | 全量解析 + golden 比对含新样本；显式行为约束不豁免 |
