# desktop-corpus-regression Specification

## Purpose

定义 workflow crate 的语料回归防线：从仓库归档 change 中挑选三代（v2 / v1 / v0）代表样本与损坏样本入仓管理，以全量解析 + golden 快照对比的回归测试捕获 TS schema 演进导致的 Rust 解析静默劣化。

## Requirements

### Requirement: 三代代表性 fixture 语料

workflow crate SHALL 在 `tests/fixtures/` 维护从仓库 74 个归档 change 中挑选的三代（v2 / v1 / v0）代表样本语料，入仓随包版本管理。语料 SHALL 覆盖：完整 v2（含 file_log、active_phase、interrupted、backtrack 轨迹）、v1（eval + 旧 files 桶）、v0（仅 markdown 产物），以及至少一个字段损坏样本（用于降级路径）。

#### Scenario: 语料覆盖三代与损坏样本

- **WHEN** 检查 `tests/fixtures/` 语料清单
- **THEN** 含 v2、v1、v0 代表样本各至少一个，及至少一个损坏字段样本

### Requirement: 全量解析快照回归

workflow crate SHALL 提供全量解析语料并与 golden 快照对比的回归测试：对 fixtures 全样本执行代际探测 + 解析 + queries 聚合，输出序列化结果与 golden 文件比对。该测试 SHALL 防止 TS 侧 schema 演进导致 Rust 解析静默劣化——golden 更新 SHALL 是显式行为（需人工确认差异符合预期），MUST NOT 静默自动重写。

#### Scenario: 快照回归通过

- **WHEN** 运行快照回归测试
- **THEN** 全部 fixture 的解析输出与 golden 一致

#### Scenario: schema 漂移被捕获

- **WHEN** 某字段形状变化导致解析输出改变
- **THEN** 回归测试失败并呈现差异，需显式确认后才能更新 golden

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
| `workflow::tests/fixtures` | 三代代表语料 | 入仓管理；覆盖 v2 / v1 / v0 + 损坏样本 |
| `workflow::tests` 快照回归 | 漂移防线 | 全量解析 → golden 对比；golden 更新需显式确认 |
| `crates/core/workflow/tests/fixtures/` | 语料增量 | 增含会话槽位字段的样本（来源 design 定稿）；既有样本文件零改写 |
| `crates/core/workflow/tests/golden/` | 快照对账 | `DESKTOP_GOLDEN_REWRITE=1` 显式再生成 → 复核一致；diff 人工确认仅新增槽位键并留痕 |
| `corpus_golden_test.rs` | 回归防线 | 全量解析 + golden 比对含新样本；显式行为约束不豁免 |
