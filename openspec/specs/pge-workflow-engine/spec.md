## ADDED Requirements

### Requirement: hasPhasePassed filters stale entries
The `hasPhasePassed()` function in `phase-next.ts` SHALL be the single gate-check replacement. It SHALL ignore entries where `stale === true`. A phase is considered "passed" only if it has at least one entry with `verdict === 'pass'` (or `skipped === true`) AND `stale` is NOT `true`.

The stale filtering implicitly enforces prerequisite dependencies: if a prerequisite phase's pass entry is stale, `hasPhasePassed` returns false, so `phase/next` returns that prerequisite phase instead of the dependent.

#### Scenario: hasPhasePassed returns false for stale pass
- **WHEN** `hasPhasePassed(entries, "02-dev-design")` is called and only pass entry has `stale: true`
- **THEN** it returns `false`

#### Scenario: hasPhasePassed returns true with mix of stale and fresh entries
- **WHEN** `hasPhasePassed(entries, "02-dev-design")` is called with both stale and non-stale pass entries
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
- **WHEN** `phase/next` is called and 02-dev-design's only pass entry is stale
- **THEN** it returns `next_phase: "02-dev-design"` (not 03 or 05 which depend on 02)

#### Scenario: phase/next blocks test-gen when implement is stale
- **WHEN** `phase/next` is called, 04-test-gen has valid pass but 05-implement has only stale pass
- **THEN** it returns `next_phase: "05-implement"` (04-test-gen requires 05-implement; 04 must be regenerated after 05 is redone)

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
| 01-proposal | 01-proposal | DESIGN Planner->Evaluator | Proposal and requirements (replaces 01-requirements) |
| 02-dev-design | 02-dev-design | DESIGN Planner->Evaluator | Implementation design |
| 03-test-design | 03-test-design | DESIGN Planner->Evaluator | Test scenario design |
| 05-implement | 05-implement | EXEC Generator->Evaluator + AUTO static-check | Implementation code generation |
| 04-test-gen | 04-test-gen | EXEC Generator->Evaluator | Test code generation |
| 06-unit-test | 06-unit-test | EXEC Executor->Evaluator (sonnet) | Unit test execution and report validation |
| 07-code-review | 07-code-review | EVAL-ONLY Evaluator | Code review evaluation |
| 08-integration-test | 08-integration-test | EXEC Executor->Evaluator (sonnet) | Integration test execution and report validation |
| 09-acceptance | 09-acceptance | EVAL-ONLY Evaluator | Acceptance evaluation |

The phase formerly named `01-requirements` is RENAMED to `01-proposal`. The planner for phase 01 changes from the main agent (skill directly writes artifacts) to the `proposal-planner` sub-agent, consistent with phases 02-03.

**Execution order change:** `05-implement` SHALL appear before `04-test-gen` in the canonical `PHASES` array (derived from `PHASE_REQUIREMENT`). Phase identifiers are NOT renumbered — only scheduling order changes so implementation completes before test generation.

Each phase SHALL append its result to eval.json upon completion.

#### Scenario: Phase 01 uses proposal-planner sub-agent
- **WHEN** Phase 01-proposal executes
- **THEN** the `proposal-planner` sub-agent writes proposal.md + specs/
- **AND** the `proposal-evaluator` sub-agent evaluates against the checklist
- **AND** the phase follows the same P→E loop pattern as 02-dev-design and 03-test-design

#### Scenario: Full workflow progression with prerequisites
- **WHEN** all phases complete successfully from 01-proposal through 09-acceptance
- **THEN** eval.json contains entries for all 9 phases with verdict "pass"

#### Scenario: Phase ordering enforcement uses prerequisites
- **WHEN** phase/next scans and 02-dev-design has no valid pass
- **THEN** phase/next SHALL return 02-dev-design (03-test-design requires 02 as prerequisite)
- **AND** 03-test-design SHALL NOT be returned until 02 has a valid pass

- **WHEN** phase/next scans and 05-implement has no valid pass but 04-test-gen does
- **THEN** phase/next SHALL return 05-implement (04-test-gen requires 05 as prerequisite; 04 pass alone is insufficient)

#### Scenario: Implement executes before test-gen in canonical order
- **WHEN** phases 01-proposal, 02-dev-design, and 03-test-design have valid pass entries and neither 05-implement nor 04-test-gen has passed
- **THEN** `PHASES` index of `05-implement` is less than `04-test-gen`
- **AND** phase/next SHALL return `05-implement` as the next phase

#### Scenario: Test-gen runs after implement pass
- **WHEN** phases 01 through 03 and 05-implement have valid pass entries and 04-test-gen has not passed
- **THEN** phase/next SHALL return `04-test-gen` as the next phase

### Requirement: Workflow orchestration layer
The system SHALL support a workflow orchestration layer above the phase level. Workflow skills (`workflow-requirement`, and future `workflow-bug-fix`, `workflow-refactor`) SHALL invoke phases directly via Agent + MCP + Bash calls rather than through the Skill tool.

Each workflow skill SHALL:
1. Assemble context (change name, explore context if available)
2. Loop calling `phase/next` to determine the next phase to execute
3. For each phase: execute planner -> evaluator -> phase/log (no separate gate-check)
4. Stop on `done: true`, error, or max retries

#### Scenario: Workflow layer vs phase layer separation
- **WHEN** a workflow skill executes Phase 01
- **THEN** it directly calls Agent(`proposal-planner`) and Agent(`proposal-evaluator`) with MCP phase/check and phase/log
- **AND** it does NOT invoke Skill(`phase-proposal`)
- **AND** the phase-level skill (`phase-proposal`) remains independently invocable for single-phase execution

#### Scenario: Future workflow variants follow same pattern
- **WHEN** a future `workflow-bug-fix` or `workflow-refactor` skill is created
- **THEN** it SHALL use the same orchestration pattern (context assembly -> phase/next loop -> stop, user manually archives)
- **AND** it MAY customize the phase table (skipping some phases) and context assembly (different explore detection logic)

### Requirement: eval.schema.json supports workflow phases
The `eval.schema.json` SHALL include `01-proposal` in the phase identifier enumeration, and SHALL support the `stale` field as an optional boolean (default `false`).

Updated phase identifiers: `01-proposal`, `02-dev-design`, `03-test-design`, `04-test-gen`, `05-implement`, `06-unit-test`, `07-code-review`, `08-integration-test`, `09-acceptance`.

Existing eval.json files using `01-requirements` SHALL NOT be migrated — backward compatibility is maintained by treating the old identifier as a valid but deprecated phase string.

#### Scenario: phase/log accepts 01-proposal phase
- **WHEN** MCP phase/log is called with phase `01-proposal`
- **THEN** the call succeeds (no validation error)
- **AND** the entry is appended to eval.json with phase `01-proposal`

#### Scenario: eval entry with stale field
- **WHEN** an eval entry is created
- **THEN** it MAY include `"stale": true` or `"stale": false`
- **AND** if absent, `stale` is treated as `false`

## Module Contract

### workflow.ts (`plugins/dev-team/bin/src/lib/`)

| Export / Constant | Change | Purpose |
|-------------------|--------|---------|
| `PHASE_REQUIREMENT` | MODIFIED | Swap array positions: `05-implement` before `04-test-gen` |
| `PHASES` | MODIFIED (derived) | Canonical ordered ID list reflects implement-before-test-gen |
| `PHASE_PREREQUISITES['04-test-gen']` | MODIFIED | Add `'05-implement'` prerequisite |
| `getPrerequisites()` | MODIFIED (derived) | Returns updated deps for 04-test-gen |
| `getDependents()` | MODIFIED (derived) | `05-implement` now lists `04-test-gen` as dependent |
| `getPhaseIndex('05-implement')` | MODIFIED (derived) | Index 3 (was 4) |
| `getPhaseIndex('04-test-gen')` | MODIFIED (derived) | Index 4 (was 3) |

### Workflow Skills (`plugins/dev-team/skills/`)

| Workflow | workflow_type | Phase Table (server-side) |
|----------|--------------|--------------------------|
| workflow-requirement | `requirement` | 01-proposal through 09-acceptance |
| workflow-bug-fix (future) | `bug-fix` | 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance |
| workflow-refactor (future) | `refactor` | 01-proposal through 09-acceptance |

All workflow skills follow the thin loop pattern: context assembly → `phase/next` loop → stop on completion. Archive is performed manually by the user via `/dev-team:openspec-archive-change`. The phase table is defined server-side in the MCP server, not in the skill file.

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `hasPhasePassed()` | UNCHANGED | Filters `stale: true` entries |
| `runPhaseNext()` | MODIFIED (behavior) | Scans reordered phase table; respects new 04→05 prerequisite |
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

