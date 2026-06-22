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

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `hasPhasePassed()` | UNCHANGED | Filters `stale: true` entries |
| `runPhaseNext()` | MODIFIED (behavior) | Scans reordered phase table; respects new 04→05 prerequisite |
