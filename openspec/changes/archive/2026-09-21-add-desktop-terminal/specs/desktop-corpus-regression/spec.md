# desktop-corpus-regression Specification

## ADDED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `workflow::tests/fixtures` | 三代代表语料 | 入仓管理；覆盖 v2 / v1 / v0 + 损坏样本 |
| `workflow::tests` 快照回归 | 漂移防线 | 全量解析 → golden 对比；golden 更新需显式确认 |
