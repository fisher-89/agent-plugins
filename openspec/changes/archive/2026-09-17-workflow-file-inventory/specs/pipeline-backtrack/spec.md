# pipeline-backtrack Specification（workflow-file-inventory 增量）

## ADDED Requirements

### Requirement: backtrack 到 implement 或更早时清空文件清单

`runBacktrack` SHALL 在 `backtrack_to` 目标为 `implement` phase 或其之前的 phase（`proposal` / `dev-design` / `test-design`）时，把该 change 的 `workflow.json` 的 `files` 重置为 `{ written: [], deleted: [] }`，使废弃方案的文件操作残留不污染后续突变范围与对账；回溯到 `implement` 之后的 phase（`test-gen` / `test-execution` 等）SHALL 保留 `files` 不变。

清空 SHALL 与既有写回同通道完成（保留 `workflow_type` / `created` / `eval` / 未知键）；目标 change 的 `workflow.json` 缺失 `files` 字段时 SHALL 硬报错（与其它消费方一致），MUST NOT 静默补建。

#### Scenario: 回溯到 implement 清空清单

- **WHEN** 某 change 的 `files.written` 含 3 个路径
- **AND** 调用 `backtrack(phase: "test-execution", backtrack_to: "implement", backtrack_reason: <非空>)`
- **THEN** 写回后 `files` 等于 `{ "written": [], "deleted": [] }`
- **AND** `eval` 的 backtrack 标记与 stale 传播行为不变

#### Scenario: 回溯到 test-gen 保留清单

- **WHEN** 调用 `backtrack(backtrack_to: "test-gen", ...)`
- **THEN** `files` 内容不变

#### Scenario: 旧 change 报错

- **WHEN** 目标 change 的 `workflow.json` 无 `files` 字段且触发清空规则
- **THEN** 返回硬错误并指引重建
- **AND** 评估条目的 backtrack 改写 MUST NOT 部分生效

## Module Contract

### Module: `plugins/dev-team/bin/src/commands/backtrack.ts`（增量）

| 符号 | 变更 |
|------|------|
| 清空规则 | **ADDED** `backtrack_to` ∈ {`implement` 及更早} 时重置 `files`；经既有写回通道持久化 |
| 既有校验链 | 不变（change 存在 → `workflow.json` 合法 → phase 表 → 目标索引 → reason → 最新条目） |
