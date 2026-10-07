# desktop-corpus-regression Specification (Delta)

## MODIFIED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/`（种子构造器扩展） | db 种子语料 worktree 维度 | worktree 两态建档样本（带 `worktree` / `base_commit` 与 legacy `None`）；聚合 golden 覆盖 detail `worktree` null / 非 null 两投影（字段语义见 desktop-change-state-store） |
