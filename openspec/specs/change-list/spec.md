# change-list Specification

## Purpose
Existing MCP tool that lists all active (non-archived) changes under `openspec/changes/`. This change extends it with a `workflow_done` boolean field to replace `openspec status --json` workflow status. 本变更把评估来源从独立 `eval.json` 改为 `readEvalJson`（权威 `workflow.json.eval`，含遗留回退），不再把 `eval.json` 列为 artifact，并对 `workflow.json` 缺失 / 格式非法保持容错（返回 `null` / `false`，不抛错）。

## Requirements

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

### Requirement: workflow_done reuses hasPhasePassed from phase-next

The `workflow_done` computation SHALL reuse the existing `hasPhasePassed()` function from `phase-next.ts` (or a shared utility extracted from it) to determine each phase's pass status. The implementation MUST NOT duplicate the pass-detection logic.

#### Scenario: Same logic as phase gate

- **WHEN** `change_list` computes `workflow_done`
- **AND** the same change is evaluated by `phase_next` for gate check
- **THEN** both tools SHALL agree on whether a phase has passed
- **AND** both SHALL use the same code path for pass detection

### Requirement: change_list output schema includes workflow_done

The `changeEntrySchema` in `schemas/change-list.schema.ts` SHALL include `workflow_done: z.boolean()`.

#### Scenario: Schema validation passes

- **WHEN** the output of `change_list` is validated against `changeEntrySchema`
- **THEN** `workflow_done` SHALL be present as a required boolean field

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

### MCP 工具注册: `plugins/dev-team/bin/src/mcp.ts`

| 字段 | 值（变更前） | 值（变更后） |
|------|-------------|-------------|
| 工具名 | `change_list` | `change_list`（不变） |
| 输入 | `{ project_root }` | `{ project_root }`（不变） |
| 输出 | `{ name, artifacts, tasks, latest_phase }[]` | `{ name, artifacts, tasks, latest_phase, workflow_done }[]` |

### 命令实现: `plugins/dev-team/bin/src/commands/change-list.ts`

| 函数 | 签名 | 变更 |
|------|------|------|
| `listChanges` | 现有 | handler 新增 `workflow_done` 计算调用 |
| `computeWorkflowDone` | `(changeDir: string) => boolean` | NEW；继续 `readEvalJson` + `hasPhasePassed`；注释改为评估存储而非 `eval.json` 文件 |
| `KNOWN_ARTIFACTS` | — | 移除 `'eval.json'` |
| `readWorkflowType` | — | 改用 `workflowFileSchema.safeParse`（删除手工 `isPlainObject` / `typeof` 判断）；任一失败返回 `null`，不抛错 |
| `processChangeEntry` | — | `latest_phase` 仍来自 `readEvalJson` |

### Schema: `plugins/dev-team/bin/src/schemas/change-list.schema.ts`

| 字段 | 类型 | 变更 |
|------|------|------|
| `name` | `string` | 不变 |
| `artifacts` | `Artifact[]` | 描述示例去掉 `eval.json` |
| `tasks` | `Task[]` | 不变 |
| `latest_phase` | `string \| null` | 描述改为「最新评估条目；无条目则为 null」，不再引用 `eval.json` 文件 |
| `workflow_done` | `boolean` | **ADDED** |

### 依赖: `plugins/dev-team/bin/src/commands/phase-next.ts`

| 函数 | 用途 |
|------|------|
| `hasPhasePassed(entries, phaseId)` | 判断某 phase 是否有非 stale 的 pass/skipped 条目；复用为 `workflow_done` 的计算基础 |
