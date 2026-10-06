# desktop-corpus-regression Specification (Delta)

## RENAMED Requirements

- FROM: `### Requirement: 全量解析快照回归`
- TO: `### Requirement: 全量聚合快照回归`

## MODIFIED Requirements

### Requirement: 三代代表性 fixture 语料

workflow crate SHALL 在 `tests/fixtures/` 维护语料夹具，载体 SHALL 从 workflow.json 目录树改为 **db 种子夹具**（经 store change 域操作面写入 `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` 的种子构造器）+ 磁盘产物树（markdown 产物与 db 缺记录的文档形态目录）。语料 SHALL 覆盖：完整执行史（多 attempt、fail→retry、backtrack 与 stale 标记、skipped 条目）、会话槽位全/缺两态、db 缺记录文档形态（磁盘目录 + 无 db 记录）、以及坏行样本（解不出为合法记录的种子，用于显式错误路径）。原 workflow.json 语料（v1-b / v1-c / v2-a 等三代 fixtures 与损坏样本）随 parse 退役失去解析主体，处置形态（删除 vs 转标 legacy 子目录保留）由 design 定稿，MUST NOT 静默遗留在默认语料路径参与回归。

#### Scenario: 语料覆盖状态面形态

- **WHEN** 检查 `tests/fixtures/` 语料清单与种子构造器
- **THEN** 含完整执行史（含 backtrack / stale / skipped）、槽位全缺两态、文档形态与坏行样本各至少一个，全部经 db 种子构造

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

## REMOVED Requirements

### Requirement: 含会话槽位的新代际语料与 golden 对账

**Reason**: 该需求是 workflow.json eval 条目会话槽位字段（解析面）的语料与 golden 对账条款；槽位随本变更成为 PhaseRecord 原生列（库内一等查询），「新代际样本覆盖槽位字段」的对账对象消失——槽位全/缺两态覆盖并入改写后的「三代代表性 fixture 语料」db 种子语料，detail 线面演进对账由改写后的「全量聚合快照回归」显式重写流程继续承担。

**Migration**: 语料夹具按「三代代表性 fixture 语料」改写为 db 种子（槽位全/缺样本在列）；detail DTO 的 golden 快照按冻结契约显式重写流程处置（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕）；「golden 更新是显式行为」约束不豁免、不失效。

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/` | db 种子语料 | 种子构造器经 store change 域操作面建档；覆盖执行史形态 + 文档形态 + 坏行；历史 workflow.json 夹具删除或转标 legacy（design 定稿） |
| `crates/core/workflow/tests/golden/` | 快照对账 | `DESKTOP_GOLDEN_REWRITE=1` 显式再生成 → diff 人工确认留痕 → 复核一致 |
| `crates/core/workflow/tests/corpus_golden_test.rs` | 回归防线 | db 种子 → 聚合 → golden 比对；显式行为约束不豁免 |
