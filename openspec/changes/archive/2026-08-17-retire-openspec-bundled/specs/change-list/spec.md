# change-list Specification

## Purpose
Existing MCP tool that lists all active (non-archived) changes under `openspec/changes/`. This change extends it with a `workflow_done` boolean field to replace `openspec status --json` workflow status.

## ADDED Requirements

### Requirement: change_list returns workflow_done field

The `change_list` MCP tool SHALL return a `workflow_done` boolean field for each change entry. The field SHALL be computed by reading `workflow.json` to determine `workflow_type`, looking up the phase table via `getPhaseTable()`, reading `eval.json`, and checking that all phases have a non-stale `pass` or `skipped` verdict.

#### Scenario: All phases passed returns workflow_done true

- **WHEN** a change has `workflow.json` with `workflow_type: "requirement"`
- **AND** `eval.json` contains entries for all phases of the `requirement` workflow type
- **AND** each phase has at least one entry with `verdict: "pass"` or `skipped: true` and `stale` is not `true`
- **THEN** `change_list` returns `workflow_done: true` for that change

#### Scenario: Missing phase returns workflow_done false

- **WHEN** a change has `workflow.json` with `workflow_type: "requirement"`
- **AND** `eval.json` is missing entries for one or more phases
- **THEN** `change_list` returns `workflow_done: false` for that change

#### Scenario: No eval.json returns workflow_done false

- **WHEN** a change directory exists but has no `eval.json`
- **THEN** `change_list` returns `workflow_done: false` for that change

#### Scenario: Stale verdict does not count as done

- **WHEN** a phase entry exists with `verdict: "pass"` but `stale: true`
- **AND** the phase has no other non-stale pass/skipped entry
- **THEN** that phase SHALL NOT be considered completed
- **AND** `workflow_done` SHALL be `false`

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
| `computeWorkflowDone` | `(changeDir: string) => boolean` | NEW |

### Schema: `plugins/dev-team/bin/src/schemas/change-list.schema.ts`

| 字段 | 类型 | 变更 |
|------|------|------|
| `name` | `string` | 不变 |
| `artifacts` | `Artifact[]` | 不变 |
| `tasks` | `Task[]` | 不变 |
| `latest_phase` | `string \| null` | 不变 |
| `workflow_done` | `boolean` | **ADDED** |

### 依赖: `plugins/dev-team/bin/src/commands/phase-next.ts`

| 函数 | 用途 |
|------|------|
| `hasPhasePassed(entries, phaseId)` | 判断某 phase 是否有非 stale 的 pass/skipped 条目；复用为 `workflow_done` 的计算基础 |