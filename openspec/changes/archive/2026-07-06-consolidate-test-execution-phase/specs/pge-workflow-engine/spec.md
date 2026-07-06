## MODIFIED Requirements

### Requirement: Eight-phase workflow structure (was Nine-phase)

The requirement workflow SHALL support 8 sequential phases with the following identifiers:

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| proposal | proposal | DESIGN Planner->Evaluator | Proposal and requirements |
| dev-design | dev-design | DESIGN Planner->Evaluator | Implementation design |
| test-design | test-design | DESIGN Planner->Evaluator | Test scenario design |
| implement | implement | EXEC Generator->Evaluator + AUTO static-check | Implementation code generation |
| test-gen | test-gen | EXEC Generator->Evaluator | Test code generation |
| test-execution | test-execution | EXEC Executor->Evaluator (sonnet) | All automated test execution (unit + integration) |
| code-review | code-review | EVAL-ONLY Evaluator | Code review evaluation |
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
- **THEN** eval.json contains entries for all 8 phases with verdict "pass"

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

### Requirement: Prerequisite dependency table (ID update for test-execution)

The prerequisite table for the `requirement` workflow_type SHALL use the updated phase identifiers:

| Phase | Prerequisites | Notes |
|-------|---------------|-------|
| proposal | [] | Root phase, no dependencies |
| dev-design | [proposal] | Only depends on proposal |
| test-design | [proposal, dev-design] | Depends on proposal and dev-design |
| implement | [dev-design] | Only depends on dev-design |
| test-gen | [test-design, implement] | Requires both test design AND completed implementation |
| test-execution | [test-gen, implement] | Needs both tracks |
| code-review | [test-gen, implement] | Same prerequisites as test-execution |
| acceptance | [proposal, dev-design, implement] | Pure dev track, no test track dependency |

For the `bug-fix` workflow_type:

| Phase | Prerequisites |
|-------|--------------|
| proposal | [] |
| dev-design | [proposal] |
| implement | [dev-design] |
| test-execution | [implement] |
| code-review | [implement] |
| acceptance | [code-review] |

For the `test-only` workflow_type:

| Phase | Prerequisites |
|-------|--------------|
| proposal | [] |
| code-analyze | [proposal] |
| test-design | [proposal, code-analyze] |
| test-gen | [test-design] |
| test-execution | [test-gen] |

#### Scenario: getPrerequisites with updated IDs for requirement workflow
- **WHEN** `getPrerequisites("proposal", "requirement")` is called
- **THEN** it returns `[]`

- **WHEN** `getPrerequisites("implement", "requirement")` is called
- **THEN** it returns `["dev-design"]`

- **WHEN** `getPrerequisites("test-gen", "requirement")` is called
- **THEN** it returns `["test-design", "implement"]`

- **WHEN** `getPrerequisites("test-execution", "requirement")` is called
- **THEN** it returns `["test-gen", "implement"]`

- **WHEN** `getPrerequisites("acceptance", "requirement")` is called
- **THEN** it returns `["proposal", "dev-design", "implement"]`

- **WHEN** `getPrerequisites("code-review", "requirement")` is called
- **THEN** it returns `["test-gen", "implement"]` (no longer includes `integration-test`)

#### Scenario: getPrerequisites with updated IDs for bug-fix workflow
- **WHEN** `getPrerequisites("implement", "bug-fix")` is called
- **THEN** it returns `["dev-design"]`

- **WHEN** `getPrerequisites("acceptance", "bug-fix")` is called
- **THEN** it returns `["code-review"]`

- **WHEN** `getPrerequisites("test-execution", "bug-fix")` is called
- **THEN** it returns `["implement"]`

#### Scenario: getPrerequisites for test-only workflow
- **WHEN** `getPrerequisites("test-execution", "test-only")` is called
- **THEN** it returns `["test-gen"]`

### Requirement: Dependents graph (reverse dependency lookup, ID update)

For the `requirement` workflow_type, the dependents graph SHALL use the updated phase identifiers:

| Phase | Dependents |
|-------|-----------|
| proposal | [dev-design, test-design, acceptance] |
| dev-design | [test-design, implement, acceptance] |
| test-design | [test-gen] |
| implement | [test-gen, test-execution, code-review, acceptance] |
| test-gen | [test-execution, code-review] |
| test-execution | [] |
| code-review | [] |
| acceptance | [] |

#### Scenario: getDependents returns correct dependents with updated IDs
- **WHEN** `getDependents("proposal")` is called
- **THEN** it returns `["dev-design", "test-design", "acceptance"]`

- **WHEN** `getDependents("implement")` is called
- **THEN** it returns `["test-gen", "test-execution", "code-review", "acceptance"]` (no longer includes `integration-test`)

- **WHEN** `getDependents("test-gen")` is called
- **THEN** it returns `["test-execution", "code-review"]` (no longer includes `integration-test`)

- **WHEN** `getDependents("test-execution")` is called
- **THEN** it returns `[]`

## REMOVED Requirements

### Requirement: Nine-phase workflow structure (with integration-test)

**Reason**: The `integration-test` phase has been removed as part of consolidating all test execution into the single `test-execution` phase. The requirement workflow now has 8 phases instead of 9.

**Migration**: Remove `integration-test` from all phase tables (PHASE_REQUIREMENT, PHASE_BUG_FIX, PHASE_TEST_ONLY). Update all phase references accordingly.

### Requirement: integration-test phase in prerequisite table

**Reason**: All `integration-test` entries in the `PHASE_PREREQUISITES` table have been removed. The `test-execution` phase replaces both `unit-test` and `integration-test`.

**Migration**: Remove `integration-test: ["test-gen", "implement"]` from all prerequisite tables.

### Requirement: integration-test in dependents graph

**Reason**: The `integration-test` phase no longer exists in the workflow, so it is no longer a dependent of any phase.

**Migration**: Remove `integration-test` from the dependents arrays of `implement` and `test-gen`.

## Module Contract

### workflow.ts (`plugins/dev-team/bin/src/lib/`)

| Export / Constant | Change | Purpose |
|-------------------|--------|---------|
| `PHASE_REQUIREMENT` | MODIFIED | Remove `integration-test` entry; rename `unit-test` → `test-execution` (id, description, planner, evaluator) |
| `PHASE_BUG_FIX` | MODIFIED | Remove `integration-test` (was not present); rename `unit-test` → `test-execution` |
| `PHASE_TEST_ONLY` | MODIFIED | Remove `integration-test` entry; rename `unit-test` → `test-execution` |
| `PHASE_REQUIREMENT` length | MODIFIED | From 9 to 8 phases |
| `PHASE_BUG_FIX` length | MODIFIED | From 6 to 6 (no change) |
| `PHASE_TEST_ONLY` length | MODIFIED | From 6 to 5 phases |
| `PHASE_PREREQUISITES` | MODIFIED | Remove `integration-test` key; rename `unit-test` key → `test-execution` |
| `PHASE_BUG_FIX_PREREQUISITES` | MODIFIED | Rename `unit-test` → `test-execution` |
| `PHASE_TEST_ONLY_PREREQUISITES` | MODIFIED | Remove `integration-test` key; rename `unit-test` → `test-execution` |
| `getPhaseTable()` | UNCHANGED | No API change — phase table content updated |
| `getPrerequisites()` | UNCHANGED | No API change — prerequisite table content updated |
| `getDependents()` | UNCHANGED | No API change — dependents derived from updated prerequisites |

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `hasPhasePassed()` | UNCHANGED | Filters `stale: true` entries — no behavioral change |
| `runPhaseNext()` | UNCHANGED | No API change — phase table content updated |
| `getLatestBacktrackTarget()` | UNCHANGED | Detects backtrack_to in latest entry |

### eval-json.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `checkGate()` | UNCHANGED | Accepts prerequisites array; filters `stale: true` entries |

### Phase Tables Comparison

| Workflow | Before | After |
|----------|--------|-------|
| requirement | 9 phases incl. `unit-test`, `integration-test` | 8 phases: `test-execution` replaces both |
| bug-fix | 6 phases: `unit-test` (no `integration-test`) | 6 phases: `test-execution` replaces `unit-test` |
| test-only | 6 phases incl. `unit-test`, `integration-test` | 5 phases: `test-execution` replaces both |
