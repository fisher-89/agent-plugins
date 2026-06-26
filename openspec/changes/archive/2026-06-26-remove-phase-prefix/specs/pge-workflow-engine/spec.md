## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Nine-phase workflow structure
The system SHALL support 9 sequential phases with the following identifiers (prefix removed):

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

Each phase SHALL append its result to eval.json upon completion.

#### Scenario: Phase proposal uses proposal-planner sub-agent
- **WHEN** Phase `proposal` executes
- **THEN** the `proposal-planner` sub-agent writes proposal.md + specs/
- **AND** the `proposal-evaluator` sub-agent evaluates against the checklist
- **AND** the phase follows the same P-E loop pattern as dev-design and test-design

#### Scenario: Full workflow progression with prerequisites
- **WHEN** all phases complete successfully from `proposal` through `acceptance`
- **THEN** eval.json contains entries for all 9 phases with verdict "pass"

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

## Module Contract

### workflow.ts (`plugins/dev-team/bin/src/lib/`)

| Export / Constant | Change | Purpose |
|-------------------|--------|---------|
| `PHASE_REQUIREMENT` | MODIFIED | All phase `id` fields: `01-proposal` → `proposal`, `02-dev-design` → `dev-design`, etc. |
| `PHASE_BUG_FIX` | MODIFIED | Same prefix removal on bug-fix phase table |
| `PHASE_TEST_ONLY` | MODIFIED | Same prefix removal on test-only phase table |
| `PHASE_PREREQUISITES` | MODIFIED | All keys and array values: `01-proposal` → `proposal`, etc. |
| `PHASE_BUG_FIX_PREREQUISITES` | MODIFIED | Same prefix removal |
| `PHASE_TEST_ONLY_PREREQUISITES` | MODIFIED | Same prefix removal |
| `getPhaseTable()` | UNCHANGED | No API change — phase table content updated |
| `getPrerequisites()` | UNCHANGED | No API change — prerequisite table content updated |
| `getDependents()` | UNCHANGED | No API change — dependents derived from updated prerequisites |

### phase-next.ts (`commands/`)

| Export | Change | Purpose |
|--------|--------|---------|
| `interpolatePrompt()` | MODIFIED | Add `<phase>` placeholder support; accept phase parameter |
| `buildPhaseDef()` | MODIFIED | Pass current phase ID to `interpolatePrompt()` when resolving prompts |
| `buildBacktrackHint()` | UNCHANGED | Uses phase table values directly (already dynamic) |
