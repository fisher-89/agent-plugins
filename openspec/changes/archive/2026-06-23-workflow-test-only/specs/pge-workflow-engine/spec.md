## ADDED Requirements

### Requirement: Existing workflow types remain unchanged after test-only addition
Adding the `test-only` workflow_type SHALL NOT alter phase tables, prerequisite tables, phase ordering, agent assignments, or prompts for `requirement`, `bug-fix`, or `refactor` workflow types.

Existing unit and integration tests for these workflow types SHALL continue to pass without modification to their assertions.

#### Scenario: requirement workflow phase table unchanged (AC-10)
- **WHEN** `getPhaseTable("requirement")` is called after `test-only` is registered
- **THEN** it returns the same 9 phases in the same order as before this change
- **AND** `getPrerequisites("04-test-gen", "requirement")` still returns `["03-test-design", "05-implement"]`

#### Scenario: bug-fix workflow unchanged (AC-10)
- **WHEN** `getPhaseTable("bug-fix")` is called after `test-only` is registered
- **THEN** it returns phases: `01-proposal`, `02-dev-design`, `05-implement`, `06-unit-test`, `07-code-review`, `09-acceptance`
- **AND** `getPrerequisites("09-acceptance", "bug-fix")` still returns `["07-code-review"]`

#### Scenario: refactor workflow unchanged (AC-10)
- **WHEN** `getPhaseTable("refactor")` is called after `test-only` is registered
- **THEN** it returns the same phase list as `requirement`
- **AND** `getPrerequisites("03-test-design", "refactor")` still returns `["01-proposal", "02-dev-design"]`

#### Scenario: existing workflow tests pass without modification (AC-10)
- **WHEN** `workflow.test.ts` and `phase-next.test.ts` are executed after implementing `test-only`
- **THEN** all pre-existing test cases for `requirement`, `bug-fix`, and `refactor` pass
- **AND** no existing test assertions require updating to accommodate the new workflow_type

### Requirement: Change config reader resolves workflow_type from workflow.json
The PGE workflow engine SHALL provide a `getWorkflowType(change: string): string` function (via `change-config.ts` or equivalent) that reads `openspec/changes/<change>/workflow.json` and returns the `workflow_type` field.

When `workflow.json` does not exist or `workflow_type` is absent, the function SHALL return `"requirement"` for backward compatibility with existing changes.

Supported values SHALL include: `"requirement"`, `"bug-fix"`, `"refactor"`, `"test-only"`.

#### Scenario: getWorkflowType reads test-only from workflow.json (AC-1)
- **WHEN** `workflow.json` contains `{"workflow_type": "test-only"}` for change `"my-change"`
- **THEN** `getWorkflowType("my-change")` returns `"test-only"`

#### Scenario: getWorkflowType defaults to requirement when file absent (AC-13)
- **WHEN** `workflow.json` does not exist for the change
- **THEN** `getWorkflowType(change)` returns `"requirement"`

#### Scenario: getWorkflowType defaults to requirement when field absent (AC-13)
- **WHEN** `workflow.json` exists but does not contain `workflow_type`
- **THEN** `getWorkflowType(change)` returns `"requirement"`

### Requirement: phase_log reads workflow_type from workflow.json for backtrack validation
`phase_log` SHALL NOT accept a `workflow_type` MCP parameter. Instead, `runPhaseLog()` SHALL call `getWorkflowType(change)` to determine the workflow-specific phase table by reading `workflow.json`.

`handleBacktrackMarking()` SHALL validate `backtrack_to` targets against `getPhaseTable(workflow_type)`, REPLACING the previous global `getPhaseIndex()` validation.

When a `backtrack_to` target is not present in the workflow's phase table, `phase_log` SHALL reject the call with an error message and SHALL NOT write any entry to eval.json:

> 当前工作流 {workflow_type} 不包含 phase '{target}'。可用的 phases: {available_phases_list}

#### Scenario: phase_log rejects invalid backtrack for test-only workflow (AC-11)
- **WHEN** change `workflow.json` has `{"workflow_type": "test-only"}`
- **AND** `phase_log` is called with `phase: "06-unit-test"`, `verdict: "fail"`, and `backtrack_to: "05-implement"`
- **THEN** the call is rejected with an error
- **AND** eval.json is NOT modified

#### Scenario: rejection error lists available phases (AC-12)
- **WHEN** `phase_log` rejects a backtrack target not in the test-only phase table
- **THEN** the error message includes `workflow_type: "test-only"`
- **AND** the error message lists available phases: `01-proposal`, `02-code-analyze`, `03-test-design`, `04-test-gen`, `06-unit-test`, `08-integration-test`

#### Scenario: phase_log uses requirement table when workflow.json absent (AC-13)
- **WHEN** change has no `workflow.json` file (or the file lacks `workflow_type`)
- **AND** `phase_log` is called with `backtrack_to: "02-dev-design"` for a requirement workflow phase
- **THEN** backtrack validation uses the `requirement` phase table
- **AND** valid requirement backtrack targets are accepted as before

#### Scenario: global getPhaseIndex validation is replaced not supplemented
- **WHEN** `handleBacktrackMarking()` validates backtrack targets
- **THEN** it uses `getPhaseTable(getWorkflowType(change))` membership check (where `getWorkflowType` reads from `workflow.json`)
- **AND** does NOT use global `getPhaseIndex()` for backtrack target validation

#### Scenario: phase_log MCP schema has no workflow_type field (AC-17)
- **WHEN** inspecting `phaseLogInputSchema`
- **THEN** it does NOT include a `workflow_type` property

## MODIFIED Requirements

### Requirement: Workflow orchestration layer
The system SHALL support a workflow orchestration layer above the phase level. Workflow skills (`workflow-requirement`, `workflow-test-only`, and future `workflow-bug-fix`, `workflow-refactor`) SHALL invoke phases directly via Agent + MCP + Bash calls rather than through the Skill tool.

Each workflow skill SHALL:
1. Assemble context (change name, explore context if available)
2. Write `workflow.json` when scaffolding a new change
3. Loop calling `phase/next` with only the `change` parameter (workflow type resolved server-side from `workflow.json`)
4. For each phase: execute planner -> evaluator -> phase/log (no separate gate-check)
5. Stop on `done: true`, error, or max retries

#### Scenario: Workflow layer vs phase layer separation
- **WHEN** a workflow skill executes Phase 01
- **THEN** it directly calls Agent(`proposal-planner`) and Agent(`proposal-evaluator`) with MCP phase/check and phase/log
- **AND** it does NOT invoke Skill(`phase-proposal`)
- **AND** the phase-level skill (`phase-proposal`) remains independently invocable for single-phase execution

#### Scenario: Future workflow variants follow same pattern
- **WHEN** a `workflow-test-only`, `workflow-bug-fix`, or `workflow-refactor` skill is created
- **THEN** it SHALL use the same orchestration pattern (context assembly -> write workflow.json -> phase/next loop -> stop, user manually archives)
- **AND** it MAY customize the phase table (skipping some phases) and context assembly (different explore detection logic)

### Requirement: Register test-only workflow_type in workflow engine
The PGE workflow engine SHALL register `test-only` as a fourth `workflow_type` by adding entries to `PHASE_TABLES` and `PHASE_PREREQUISITES_TABLES` in `workflow.ts`. Behavioral requirements for the six-phase structure, prerequisite DAG, phase progression, completion semantics, and prompt customization SHALL be defined in the `test-only-workflow` capability spec.

#### Scenario: workflow engine exposes test-only workflow_type
- **WHEN** `getPhaseTable("test-only")` is called
- **THEN** it returns the 6-phase table defined in `test-only-workflow` spec
- **AND** `getPrerequisites(phaseId, "test-only")` returns values from the `test-only-workflow` prerequisite table

## Module Contract

### change-config.ts (`plugins/dev-team/bin/src/lib/`)

| Export | Type | Purpose |
|--------|------|---------|
| `readWorkflowConfig(change)` | `Record<string, unknown>` | Parse `workflow.json` for a change |
| `getWorkflowType(change)` | `string` | Return `workflow_type` from `workflow.json`; default `"requirement"` |

### workflow.ts (`plugins/dev-team/bin/src/lib/`)

| Export / Constant | Change | Purpose |
|-------------------|--------|---------|
| `PHASE_TABLES['test-only']` | ADDED | Points to `PHASE_TEST_ONLY` (defined per `test-only-workflow` spec) |
| `PHASE_PREREQUISITES_TABLES['test-only']` | ADDED | Points to `PHASE_TEST_ONLY_PREREQUISITES` |
| `PHASE_REQUIREMENT` | UNCHANGED | No modification to requirement workflow |
| `PHASE_BUG_FIX` | UNCHANGED | No modification to bug-fix workflow |
| `PHASE_REFACTOR` | UNCHANGED | No modification to refactor workflow |

### Workflow Skills (`plugins/dev-team/skills/`)

| Workflow | workflow_type (in workflow.json) | Phase Table (server-side) |
|----------|----------------------------------|--------------------------|
| workflow-requirement | `requirement` | 01-proposal through 09-acceptance (unchanged) |
| workflow-test-only | `test-only` | Per `test-only-workflow` spec |
| workflow-bug-fix (future) | `bug-fix` | Unchanged |
| workflow-refactor (future) | `refactor` | Unchanged |

### MCP Tools

| Tool | Change | Purpose |
|------|--------|---------|
| phase_next | MODIFIED | Remove `workflow_type` input; read from `workflow.json` via `getWorkflowType()` |
| phase_log | MODIFIED | Remove `workflow_type` input; backtrack validation reads `workflow.json` via `getWorkflowType()` + `getPhaseTable()` |

### Schemas (`plugins/dev-team/bin/src/schemas/`)

| File | Change | Purpose |
|------|--------|---------|
| `phase-next.schema.ts` | MODIFIED | Remove `workflow_type` from `phaseNextInputSchema` |
| `phase-log.schema.ts` | UNCHANGED | No `workflow_type` field (never added to MCP schema) |

### Commands (`plugins/dev-team/bin/src/commands/`)

| File | Change | Purpose |
|------|--------|---------|
| `phase-next.ts` | MODIFIED | Use `getWorkflowType(change)` instead of `options.workflow_type` |
| `phase-log.ts` | MODIFIED | `handleBacktrackMarking()` uses `getWorkflowType(change)` |
| `phase-log.test.ts` | ADDED | AC-11, AC-12, AC-13 coverage with fixture `workflow.json` |
