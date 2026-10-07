# desktop-corpus-regression Specification

## Purpose

定义 workflow crate 的语料回归防线：db 种子夹具（经 store change 域操作面构造 change 状态四模型）+ 磁盘产物树入仓管理，以全量聚合 + golden 快照对比的回归测试捕获 db 聚合与 detail 线面投影的静默劣化。

## Requirements

### Requirement: 三代代表性 fixture 语料

workflow crate SHALL 在 `tests/fixtures/` 维护语料夹具，载体 SHALL 从 workflow.json 目录树改为 **db 种子夹具**（经 store change 域操作面写入 `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` 的种子构造器）+ 磁盘产物树（markdown 产物与 db 缺记录的文档形态目录）。语料 SHALL 覆盖：完整执行史（多 attempt、fail→retry、backtrack 与 stale 标记、skipped 条目）、会话槽位全/缺两态、db 缺记录文档形态（磁盘目录 + 无 db 记录）、坏行样本（解不出为合法记录的种子，用于显式错误路径）、以及 worktree 维度两态——带 `worktree` / `base_commit` 的建档样本与 `worktree=None` legacy 样本各至少一个（覆盖 detail 线面 `worktree` 字段 null / 非 null 两投影，见 desktop-change-queries / desktop-change-worktree）。原 workflow.json 语料（v1-b / v1-c / v2-a 等三代 fixtures 与损坏样本）随 parse 退役失去解析主体，处置形态（删除 vs 转标 legacy 子目录保留）由 design 定稿，MUST NOT 静默遗留在默认语料路径参与回归。

#### Scenario: 语料覆盖状态面形态

- **WHEN** 检查 `tests/fixtures/` 语料清单与种子构造器
- **THEN** 含完整执行史（含 backtrack / stale / skipped）、槽位全缺两态、文档形态与坏行样本各至少一个，含 worktree 两态建档样本（带 `worktree` / `base_commit` 与 `worktree=None`）各至少一个，全部经 db 种子构造

#### Scenario: worktree 两态投影入 golden

- **WHEN** 对含 worktree 两态样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `worktree` 出线值与库内记录逐字一致（非 null 样本出绝对路径、None 样本出 null），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕，非静默接受

#### Scenario: 历史夹具处置显式

- **WHEN** 检查 parse 退役后原 workflow.json fixtures 的去留
- **THEN** 按 design 定稿处置（删除或转标 legacy 子目录），默认语料路径无 workflow.json 解析残留，处置结论留痕

### Requirement: 全量聚合快照回归

workflow crate SHALL 提供**全量聚合 + golden 快照对比**的回归测试：对 db 种子语料逐样本执行建档 → queries 聚合（列表 + 详情），输出序列化结果与 golden 文件比对。该测试 SHALL 防止 db 聚合与 detail 线面投影的静默劣化（原「TS schema 演进导致 Rust 解析静默劣化」防线随 parse 退役由本防线接管聚合面）；golden 更新 SHALL 是显式行为（`DESKTOP_GOLDEN_REWRITE=1` 覆写 → 人工确认 diff 预期范围并留痕 → 复核一致），MUST NOT 静默自动重写。

#### Scenario: 快照回归通过

- **WHEN** 运行快照回归测试
- **THEN** 全部 db 种子样本的聚合输出与 golden 一致

#### Scenario: 聚合漂移被捕获

- **WHEN** 某聚合或线面投影形状变化导致输出改变
- **THEN** 回归测试失败并呈现差异，需显式确认后才能更新 golden

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/` | db 种子语料 | 种子构造器经 store change 域操作面建档；覆盖执行史形态 + 文档形态 + 坏行；历史 workflow.json 夹具删除或转标 legacy（design 定稿） |
| `crates/core/workflow/tests/fixtures/`（种子构造器扩展） | db 种子语料 worktree 维度 | worktree 两态建档样本（带 `worktree` / `base_commit` 与 legacy `None`）；聚合 golden 覆盖 detail `worktree` null / 非 null 两投影（字段语义见 desktop-change-state-store） |
| `crates/core/workflow/tests/golden/` | 快照对账 | `DESKTOP_GOLDEN_REWRITE=1` 显式再生成 → diff 人工确认留痕 → 复核一致 |
| `crates/core/workflow/tests/corpus_golden_test.rs` | 回归防线 | db 种子 → 聚合 → golden 比对；显式行为约束不豁免 |
