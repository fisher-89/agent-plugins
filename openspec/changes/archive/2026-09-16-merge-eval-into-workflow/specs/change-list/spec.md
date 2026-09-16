# change-list Specification

## Purpose

`change_list` 列出活跃 change 的 artifacts、任务进度、`latest_phase` 与 `workflow_done`。本变更把评估来源从独立 `eval.json` 改为 `readEvalJson`（权威 `workflow.json.eval`，含遗留回退），不再把 `eval.json` 列为 artifact，并对 `workflow.json` 缺失 / 格式非法保持容错（返回 `null` / `false`，不抛错）。

## MODIFIED Requirements

### Requirement: change_list returns workflow_done field

The `change_list` MCP tool SHALL return a `workflow_done` boolean field for each change entry. The field SHALL be computed by reading `workflow.json` to determine `workflow_type`, looking up the phase table via `getPhaseTable()`, reading evaluation entries via `readEvalJson`（`workflow.json.eval`，无该数组时回退遗留 `eval.json`），and checking that all phases have a non-stale `pass` or `skipped` verdict.

`workflow.json` 的读取 SHALL 经 `workflowFileSchema.safeParse`；文件缺失或校验失败时 SHALL 视为未知类型，`workflow_done` 为 `false`、`latest_phase` 为 `null`，且 MUST NOT 抛出错误（该 change 仍正常列出）。

#### Scenario: All phases passed returns workflow_done true

- **WHEN** a change has `workflow.json` with `workflow_type: "requirement"`
- **AND** `workflow.json.eval`（或遗留 `eval.json` 回退）contains entries for all phases of the `requirement` workflow type
- **AND** each phase has at least one entry with `verdict: "pass"` or `skipped: true` and `stale` is not `true`
- **THEN** `change_list` returns `workflow_done: true` for that change

#### Scenario: Missing phase returns workflow_done false

- **WHEN** a change has `workflow.json` with `workflow_type: "requirement"`
- **AND** evaluation entries are missing for one or more phases
- **THEN** `change_list` returns `workflow_done: false` for that change

#### Scenario: No eval entries returns workflow_done false

- **WHEN** a change directory exists but `readEvalJson` 返回 `[]`（无 `eval` 键、无遗留 `eval.json`）
- **THEN** `change_list` returns `workflow_done: false` for that change

#### Scenario: Stale verdict does not count as done

- **WHEN** a phase entry exists with `verdict: "pass"` but `stale: true`
- **AND** the phase has no other non-stale pass/skipped entry
- **THEN** that phase SHALL NOT be considered completed
- **AND** `workflow_done` SHALL be `false`

#### Scenario: Malformed eval store does not crash

- **WHEN** `workflow.json.eval` 或遗留 `eval.json` 无法按 schema 解析
- **THEN** `workflow_done` SHALL be `false`
- **AND** `latest_phase` SHALL be `null`
- **AND** 该 change 仍出现在列表中

#### Scenario: Missing or malformed workflow.json does not crash

- **WHEN** change 目录没有 `workflow.json`，或文件存在但 JSON 非法 / 根非对象 / `workflow_type` 缺失或非枚举
- **THEN** `change_list` SHALL NOT 抛错
- **AND** `workflow_done` SHALL be `false`
- **AND** `latest_phase` SHALL be `null`
- **AND** 该 change 仍出现在列表中

## ADDED Requirements

### Requirement: artifacts list excludes eval.json

`KNOWN_ARTIFACTS` SHALL be `proposal.md`、`design.md`、`tasks.md`、`test-design.md`。SHALL NOT 包含 `eval.json`。`workflow.json` SHALL NOT 作为 artifact 列出（它是元数据，几乎每个 change 都有）。

`latest_phase` SHALL 取 `readEvalJson` 结果中 timestamp 最新的一条；无条目或解析失败时为 `null`。

#### Scenario: 有评估历史时 artifacts 仍不含 eval.json

- **WHEN** change 的 `workflow.json.eval` 非空
- **AND** 目录中存在 `proposal.md`
- **THEN** `artifacts` 包含 `proposal.md`
- **AND** `artifacts` SHALL NOT 包含 `eval.json`

#### Scenario: 遗留 eval.json 文件也不列入 artifacts

- **WHEN** 目录中仍存在遗留 `eval.json` 文件
- **THEN** `artifacts` SHALL NOT 包含 `eval.json`
- **AND** `latest_phase` 仍可经 `readEvalJson` 回退填出

## Module Contract

### 命令实现: `plugins/dev-team/bin/src/commands/change-list.ts`

| 符号 | 变更 |
|------|------|
| `KNOWN_ARTIFACTS` | 移除 `'eval.json'` |
| `computeWorkflowDone` | 继续 `readEvalJson` + `hasPhasePassed`；注释改为评估存储而非 `eval.json` 文件 |
| `readWorkflowType` | 改用 `workflowFileSchema.safeParse`（删除手工 `isPlainObject` / `typeof` 判断）；任一失败返回 `null`，不抛错 |
| `processChangeEntry` | `latest_phase` 仍来自 `readEvalJson` |

### Schema: `plugins/dev-team/bin/src/schemas/change-list.schema.ts`

| 字段 | 变更 |
|------|------|
| `artifacts` | 描述示例去掉 `eval.json` |
| `latest_phase` | 描述改为「最新评估条目；无条目则为 null」，不再引用 `eval.json` 文件 |
