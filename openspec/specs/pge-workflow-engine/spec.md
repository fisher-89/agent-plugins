## ADDED Requirements

### Requirement: hasPhasePassed filters stale entries
The `hasPhasePassed()` function in `phase-next.ts` SHALL be the single gate-check replacement. It SHALL ignore entries where `stale === true`. A phase is considered "passed" only if it has at least one entry with `verdict === 'pass'` (or `skipped === true`) AND `stale` is NOT `true`.

The stale filtering implicitly enforces prerequisite dependencies: if a prerequisite phase's pass entry is stale, `hasPhasePassed` returns false, so `phase/next` returns that prerequisite phase instead of the dependent.

#### Scenario: hasPhasePassed returns false for stale pass
- **WHEN** `hasPhasePassed(entries, "dev-design")` is called and only pass entry has `stale: true`
- **THEN** it returns `false`

#### Scenario: hasPhasePassed returns true with mix of stale and fresh entries
- **WHEN** `hasPhasePassed(entries, "dev-design")` is called with both stale and non-stale pass entries
- **THEN** it returns `true`

#### Scenario: hasPhasePassed backward compatible with no stale field
- **WHEN** entries have no `stale` field
- **THEN** they are treated as `stale: false`

### Requirement: phase/next is the single decision point
The workflow SHALL NOT call `phase/check` in the execution loop. All gate-check, stale-filtering, and backtrack detection SHALL be handled internally by `phase/next` via:

1. `hasPhasePassed()` with stale filtering for gate-equivalent checks
2. `getLatestBacktrackTarget()` for backtrack detection
3. Phase table scanning for determining the next phase to execute

The deprecated `phase/check` MCP tool is retained for debugging only.

#### Scenario: phase/next naturally blocks dependent when prerequisite is stale
- **WHEN** `phase/next` is called and `dev-design`'s only pass entry is stale
- **THEN** it returns `next_phase: "dev-design"` (not `test-design` or `implement` which depend on `dev-design`)

#### Scenario: phase/next blocks test-gen when implement is stale
- **WHEN** `phase/next` is called, `test-gen` has valid pass but `implement` has only stale pass
- **THEN** it returns `next_phase: "implement"` (`test-gen` requires `implement`; `test-gen` must be regenerated after `implement` is redone)

### Requirement: <phase> parameter in prompt interpolation
The `interpolatePrompt()` function in `phase-next.ts` SHALL support a `<phase>` placeholder in addition to the existing `<change>` placeholder. When present in a prompt template, `<phase>` SHALL be replaced with the current phase ID at runtime.

This enables dynamic phase identity injection without hardcoding phase IDs in agent prompts.

#### Scenario: <phase> placeholder is replaced with current phase ID
- **WHEN** a prompt template contains `"Phase ID: <phase>"`
- **AND** `interpolatePrompt()` is called with the change name and current phase ID `"proposal"`
- **THEN** the result is `"Phase ID: proposal"`

#### Scenario: <change> placeholder remains unaffected
- **WHEN** a prompt template contains `"For change <change> in phase <phase>"`
- **AND** `interpolatePrompt()` is called with change `"my-change"` and phase `"dev-design"`
- **THEN** the result is `"For change my-change in phase dev-design"`

### Requirement: phase-check.ts functions retained for internal use only
The `phase-check.ts` module SHALL be simplified to internal utility functions only. The MCP tool `phase/check` is DEPRECATED from the workflow loop.

Internal utilities retained:
- `checkGate()` — moved to `eval-json.ts`; filters stale entries; used by tests
- `determinePhaseState()` — determines first_run/retry/passed for reporting

Functions removed or no-oped:
- `checkTimestampOrder()` — no longer needed in stale model
- `checkBacktrack()` — replaced by `getLatestBacktrackTarget()` in phase/next

## MODIFIED Requirements

### Requirement: Nine-phase workflow structure
The system SHALL support 9 sequential phases with the following identifiers:

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| proposal | proposal | DESIGN Planner->Evaluator | Proposal and requirements |
| dev-design | dev-design | DESIGN Planner->Evaluator | Implementation design |
| test-design | test-design | DESIGN Planner->Evaluator | Test scenario design |
| implement | implement | EXEC Generator->Evaluator + AUTO static-check | Implementation code generation |
| test-gen | test-gen | EXEC Generator->Evaluator | Test code generation |
| unit-test | unit-test | EXEC Executor->Evaluator (sonnet) | Unit test execution and report validation |
| code-review | code-review | EVAL-ONLY Evaluator | Code review evaluation |
| integration-test | integration-test | EXEC Executor->Evaluator (sonnet) | Integration test execution and report validation |
| acceptance | acceptance | EVAL-ONLY Evaluator | Acceptance evaluation |

Execution order is determined solely by array position in the `PHASE_REQUIREMENT` table. Phase identifiers no longer encode sequencing information.

**Execution order:** `implement` SHALL appear before `test-gen` in the canonical `PHASES` array (derived from `PHASE_REQUIREMENT`).

Each phase SHALL append its result to eval.json upon completion.

#### Scenario: Phase proposal uses proposal-planner sub-agent
- **WHEN** Phase `proposal` executes
- **THEN** the `proposal-planner` sub-agent writes proposal.md + specs/
- **AND** the `proposal-evaluator` sub-agent evaluates against the checklist
- **AND** the phase follows the same P→E loop pattern as `dev-design` and `test-design`

#### Scenario: Full workflow progression with prerequisites
- **WHEN** all phases complete successfully from `proposal` through `acceptance`
- **THEN** eval.json contains entries for all 9 phases with verdict "pass"

#### Scenario: Phase ordering enforcement uses prerequisites
- **WHEN** phase/next scans and `dev-design` has no valid pass
- **THEN** phase/next SHALL return `dev-design` (`test-design` requires `dev-design` as prerequisite)
- **AND** `test-design` SHALL NOT be returned until `dev-design` has a valid pass

- **WHEN** phase/next scans and `implement` has no valid pass but `test-gen` does
- **THEN** phase/next SHALL return `implement` (`test-gen` requires `implement` as prerequisite; `test-gen` pass alone is insufficient)

#### Scenario: Implement executes before test-gen in canonical order
- **WHEN** phases `proposal`, `dev-design`, and `test-design` have valid pass entries and neither `implement` nor `test-gen` has passed
- **THEN** `PHASES` index of `implement` is less than `test-gen`
- **AND** phase/next SHALL return `implement` as the next phase

#### Scenario: Test-gen runs after implement pass
- **WHEN** phases `proposal` through `test-design` and `implement` have valid pass entries and `test-gen` has not passed
- **THEN** phase/next SHALL return `test-gen` as the next phase

### Requirement: Prerequisite dependency table (ID update)
The prerequisite table for the `requirement` workflow_type SHALL use the new phase identifiers:

| Phase | Prerequisites | Notes |
|-------|---------------|-------|
| proposal | [] | Root phase, no dependencies |
| dev-design | [proposal] | Only depends on proposal |
| test-design | [proposal, dev-design] | Depends on proposal and dev-design |
| implement | [dev-design] | Only depends on dev-design |
| test-gen | [test-design, implement] | Requires both test design AND completed implementation |
| unit-test | [test-gen, implement] | Needs both tracks |
| code-review | [test-gen, implement] | Same prerequisites as unit-test |
| integration-test | [test-gen, implement] | Same prerequisites |
| acceptance | [proposal, dev-design, implement] | Pure dev track, no test track dependency |

For the `bug-fix` workflow_type:

| Phase | Prerequisites |
|-------|--------------|
| proposal | [] |
| dev-design | [proposal] |
| implement | [dev-design] |
| unit-test | [implement] |
| code-review | [implement] |
| acceptance | [code-review] |

#### Scenario: getPrerequisites with new IDs for requirement workflow
- **WHEN** `getPrerequisites("proposal", "requirement")` is called
- **THEN** it returns `[]`

- **WHEN** `getPrerequisites("implement", "requirement")` is called
- **THEN** it returns `["dev-design"]`

- **WHEN** `getPrerequisites("test-gen", "requirement")` is called
- **THEN** it returns `["test-design", "implement"]`

- **WHEN** `getPrerequisites("acceptance", "requirement")` is called
- **THEN** it returns `["proposal", "dev-design", "implement"]`

#### Scenario: getPrerequisites with new IDs for bug-fix workflow
- **WHEN** `getPrerequisites("implement", "bug-fix")` is called
- **THEN** it returns `["dev-design"]`

- **WHEN** `getPrerequisites("acceptance", "bug-fix")` is called
- **THEN** it returns `["code-review"]`

### Requirement: Dependents graph (reverse dependency lookup, ID update)
For the `requirement` workflow_type, the dependents graph SHALL use the new phase identifiers:

| Phase | Dependents |
|-------|-----------|
| proposal | [dev-design, test-design, acceptance] |
| dev-design | [test-design, implement, acceptance] |
| test-design | [test-gen] |
| implement | [test-gen, unit-test, code-review, integration-test, acceptance] |
| test-gen | [unit-test, code-review, integration-test] |
| unit-test | [] |
| code-review | [] |
| integration-test | [] |
| acceptance | [] |

#### Scenario: getDependents returns correct dependents with new IDs
- **WHEN** `getDependents("proposal")` is called
- **THEN** it returns `["dev-design", "test-design", "acceptance"]`

- **WHEN** `getDependents("implement")` is called
- **THEN** it returns `["test-gen", "unit-test", "code-review", "integration-test", "acceptance"]`

- **WHEN** `getDependents("unit-test")` is called
- **THEN** it returns `[]`

### Requirement: Workflow orchestration layer
The system SHALL support a workflow orchestration layer above the phase level. Workflow skills (`workflow-requirement`, and future `workflow-bug-fix`, `workflow-refactor`) SHALL invoke phases directly via Agent + MCP + Bash calls rather than through the Skill tool.

Each workflow skill SHALL:
1. Assemble context (change name, explore context if available)
2. Loop calling `phase/next` to determine the next phase to execute
3. For each phase: execute planner -> evaluator -> phase/log (no separate gate-check)
4. Stop on `done: true`, error, or max retries

#### Scenario: Workflow layer vs phase layer separation
- **WHEN** a workflow skill executes Phase `proposal`
- **THEN** it directly calls Agent(`proposal-planner`) and Agent(`proposal-evaluator`) with MCP phase/next and phase/log
- **AND** it does NOT invoke Skill(`phase-proposal`)
- **AND** the phase-level skill (`phase-proposal`) remains independently invocable for single-phase execution

#### Scenario: Future workflow variants follow same pattern
- **WHEN** a future `workflow-bug-fix` or `workflow-refactor` skill is created
- **THEN** it SHALL use the same orchestration pattern (context assembly -> phase/next loop -> stop, user manually archives)
- **AND** it MAY customize the phase table (skipping some phases) and context assembly (different explore detection logic)

### Requirement: eval.schema.json supports workflow phases
The `eval.schema.json` SHALL include `proposal` in the phase identifier enumeration, and SHALL support the `stale` field as an optional boolean (default `false`).

Updated phase identifiers: `proposal`, `dev-design`, `test-design`, `test-gen`, `implement`, `unit-test`, `code-review`, `integration-test`, `acceptance`.

Existing eval.json files using `01-requirements` or other old prefixed IDs SHALL NOT be migrated — backward compatibility is maintained by treating old identifiers as valid but deprecated phase strings.

#### Scenario: phase/log accepts proposal phase
- **WHEN** MCP phase/log is called with phase `proposal`
- **THEN** the call succeeds (no validation error)
- **AND** the entry is appended to eval.json with phase `proposal`

#### Scenario: eval entry with stale field
- **WHEN** an eval entry is created
- **THEN** it MAY include `"stale": true` or `"stale": false`
- **AND** if absent, `stale` is treated as `false`

## Module Contract

### workflow.ts (`plugins/dev-team/bin/src/lib/`)

| Export / Constant | Change | Purpose |
|-------------------|--------|---------|
| `PHASE_REQUIREMENT` | MODIFIED | All phase `id` fields: `01-proposal` → `proposal`, `02-dev-design` → `dev-design`, etc. Swap array positions: `implement` before `test-gen` |
| `PHASE_BUG_FIX` | MODIFIED | Same prefix removal on bug-fix phase table |
| `PHASE_TEST_ONLY` | MODIFIED | Same prefix removal on test-only phase table |
| `PHASE_PREREQUISITES` | MODIFIED | All keys and array values: `01-proposal` → `proposal`, etc. |
| `PHASE_BUG_FIX_PREREQUISITES` | MODIFIED | Same prefix removal |
| `PHASE_TEST_ONLY_PREREQUISITES` | MODIFIED | Same prefix removal |
| `getPhaseTable()` | UNCHANGED | No API change — phase table content updated |
| `getPrerequisites()` | UNCHANGED | No API change — prerequisite table content updated |
| `getDependents()` | UNCHANGED | No API change — dependents derived from updated prerequisites |

### Workflow Skills (`plugins/dev-team/skills/`)

| Workflow | workflow_type | Phase Table (server-side) |
|----------|--------------|--------------------------|
| workflow-requirement | `requirement` | proposal through acceptance |
| workflow-bug-fix (future) | `bug-fix` | proposal, dev-design, implement, unit-test, code-review, acceptance |
| workflow-refactor (future) | `refactor` | proposal through acceptance |

All workflow skills follow the thin loop pattern: context assembly → `phase/next` loop → stop on completion. Archive is performed manually by the user via `/dev-team:openspec-archive-change`. The phase table is defined server-side in the MCP server, not in the skill file.

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `hasPhasePassed()` | UNCHANGED | Filters `stale: true` entries |
| `runPhaseNext()` | UNCHANGED | No API change — phase table content updated |
| `getLatestBacktrackTarget()` | UNCHANGED | Detects backtrack_to in latest entry |

### phase-check.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `runPhaseCheck()` | DEPRECATED | No longer called in workflow loop; retained for debugging |
| `checkTimestampOrder()` | REMOVED | No longer needed |
| `checkBacktrack()` | REMOVED | Replaced by phase/next internal logic |

### eval-json.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `checkGate()` | MODIFIED | Accepts prerequisites array; filters `stale: true` entries (used internally by phase/next via hasPhasePassed) |

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `interpolatePrompt()` | MODIFIED | Add `<phase>` placeholder support; accept phase parameter |
| `buildPhaseDef()` | MODIFIED | Pass current phase ID to `interpolatePrompt()` when resolving prompts |
| `buildBacktrackHint()` | UNCHANGED | Uses phase table values directly (already dynamic) |

### MCP Tools

| Tool | Change | Purpose |
|------|--------|---------|
| phase/next | MODIFIED | Returns next phase to execute (agent_type, prompt); server-side gate/skip/retry/backtrack/round_limit; no longer modifies eval.json |
| phase/check | DEPRECATED | Removed from workflow loop; logic merged into `phase/next`. Tool retained for debugging. |
| phase/log | UNCHANGED | Append evaluation result to eval.json |

## ADDED Requirements

### Requirement: Existing workflow types remain unchanged after test-only addition
Adding the `test-only` workflow_type SHALL NOT alter phase tables, prerequisite tables, phase ordering, agent assignments, or prompts for `requirement`, `bug-fix`, or `refactor` workflow types.

Existing unit and integration tests for these workflow types SHALL continue to pass without modification to their assertions.

#### Scenario: requirement workflow phase table unchanged (AC-10)
- **WHEN** `getPhaseTable("requirement")` is called after `test-only` is registered
- **THEN** it returns the same 9 phases in the same order as before this change
- **AND** `getPrerequisites("test-gen", "requirement")` still returns `["test-design", "implement"]`

#### Scenario: bug-fix workflow unchanged (AC-10)
- **WHEN** `getPhaseTable("bug-fix")` is called after `test-only` is registered
- **THEN** it returns phases: `proposal`, `dev-design`, `implement`, `unit-test`, `code-review`, `acceptance`
- **AND** `getPrerequisites("acceptance", "bug-fix")` still returns `["code-review"]`

#### Scenario: refactor workflow unchanged (AC-10)
- **WHEN** `getPhaseTable("refactor")` is called after `test-only` is registered
- **THEN** it returns the same phase list as `requirement`
- **AND** `getPrerequisites("test-design", "refactor")` still returns `["proposal", "dev-design"]`

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
- **AND** `phase_log` is called with `phase: "unit-test"`, `verdict: "fail"`, and `backtrack_to: "implement"`
- **THEN** the call is rejected with an error
- **AND** eval.json is NOT modified

#### Scenario: rejection error lists available phases (AC-12)
- **WHEN** `phase_log` rejects a backtrack target not in the test-only phase table
- **THEN** the error message includes `workflow_type: "test-only"`
- **AND** the error message lists available phases: `proposal`, `code-analyze`, `test-design`, `test-gen`, `unit-test`, `integration-test`

#### Scenario: phase_log uses requirement table when workflow.json absent (AC-13)
- **WHEN** change has no `workflow.json` file (or the file lacks `workflow_type`)
- **AND** `phase_log` is called with `backtrack_to: "dev-design"` for a requirement workflow phase
- **THEN** backtrack validation uses the `requirement` phase table
- **AND** valid requirement backtrack targets are accepted as before

#### Scenario: global getPhaseIndex validation is replaced not supplemented
- **WHEN** `handleBacktrackMarking()` validates backtrack targets
- **THEN** it uses `getPhaseTable(getWorkflowType(change))` membership check (where `getWorkflowType` reads from `workflow.json`)
- **AND** does NOT use global `getPhaseIndex()` for backtrack target validation

#### Scenario: phase_log MCP schema has no workflow_type field (AC-17)
- **WHEN** inspecting `phaseLogInputSchema`
- **THEN** it does NOT include a `workflow_type` property

