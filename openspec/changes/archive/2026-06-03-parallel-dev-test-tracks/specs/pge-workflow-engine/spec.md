## MODIFIED Requirements

### Requirement: Nine-phase workflow structure
The system SHALL support 9 sequential phases with the following identifiers:

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| 01-proposal | 01-proposal | DESIGN Planner->Evaluator | Proposal and requirements |
| 02-dev-design | 02-dev-design | DESIGN Planner->Evaluator | Implementation design |
| 03-test-design | 03-test-design | DESIGN Planner->Evaluator | Test scenario design |
| 04-test-gen | 04-test-gen | EXEC Generator->Evaluator | Test code generation |
| 05-implement | 05-implement | EXEC Generator->Evaluator + AUTO static-check | Implementation code generation |
| 06-unit-test | 06-unit-test | EXEC Executor->Evaluator (sonnet) | Unit test execution and report validation |
| 07-code-review | 07-code-review | EVAL-ONLY Evaluator | Code review evaluation |
| 08-integration-test | 08-integration-test | EXEC Executor->Evaluator (sonnet) | Integration test execution and report validation |
| 09-acceptance | 09-acceptance | EVAL-ONLY Evaluator | Acceptance evaluation |

Each phase SHALL append its result to eval.json upon completion.

#### Scenario: Phase 01 uses proposal-planner sub-agent
- **WHEN** Phase 01-proposal executes
- **THEN** the `proposal-planner` sub-agent writes proposal.md + specs/
- **AND** the `proposal-evaluator` sub-agent evaluates against the checklist
- **AND** the phase follows the same P->E loop pattern as 02-dev-design and 03-test-design

#### Scenario: Full workflow progression with prerequisites
- **WHEN** all phases complete successfully from 01-proposal through 09-acceptance
- **THEN** eval.json contains entries for all 9 phases with verdict "pass"

#### Scenario: Phase ordering enforcement uses prerequisites
- **WHEN** phase/next scans and 02-dev-design has no valid pass
- **THEN** phase/next SHALL return 02-dev-design (03-test-design requires 02 as prerequisite)
- **AND** 03-test-design SHALL NOT be returned until 02 has a valid pass

- **WHEN** phase/next scans and 04-test-gen has no valid pass but 05-implement does
- **THEN** phase/next SHALL return 04-test-gen (06-unit-test requires 04 as prerequisite)

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
- **THEN** it SHALL use the same orchestration pattern (context assembly -> phase table -> sequential execution -> stop, user manually archives)
- **AND** it MAY customize the phase table (skipping some phases) and context assembly (different explore detection logic)

### Requirement: eval.schema.json supports workflow phases
The `eval.schema.json` SHALL include `01-proposal` in the phase identifier enumeration, and SHALL support the `stale` field as an optional boolean (default `false`).

Updated phase identifiers: `01-proposal`, `02-dev-design`, `03-test-design`, `04-test-gen`, `05-implement`, `06-unit-test`, `07-code-review`, `08-integration-test`, `09-acceptance`.

#### Scenario: phase/log accepts 01-proposal phase
- **WHEN** MCP phase/log is called with phase `01-proposal`
- **THEN** the call succeeds (no validation error)
- **AND** the entry is appended to eval.json with phase `01-proposal`

#### Scenario: eval entry with stale field
- **WHEN** an eval entry is created
- **THEN** it MAY include `"stale": true` or `"stale": false`
- **AND** if absent, `stale` is treated as `false`

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

#### Scenario: phase/next naturally blocks convergence when one prerequisite missing
- **WHEN** `phase/next` is called, 04-test-gen has valid pass but 05-implement has only stale pass
- **THEN** it returns `next_phase: "05-implement"` (06-unit-test needs both, so 05 must be redone first)

## MODIFIED Requirements

### Requirement: phase-check.ts functions retained for internal use only
The `phase-check.ts` module SHALL be simplified to internal utility functions only. The MCP tool `phase/check` is DEPRECATED from the workflow loop.

Internal utilities retained:
- `checkGate()` — moved to `eval-json.ts`; filters stale entries; used by tests
- `determinePhaseState()` — determines first_run/retry/passed for reporting

Functions removed or no-oped:
- `checkTimestampOrder()` — no longer needed in stale model
- `checkBacktrack()` — replaced by `getLatestBacktrackTarget()` in phase/next

## Module Contract

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `hasPhasePassed()` | MODIFIED | Filters `stale: true` entries; replaces standalone gate-check |
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
| phase/check | DEPRECATED | Removed from workflow loop; logic merged into `phase/next`. Tool retained for debugging. |
