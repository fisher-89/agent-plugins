## ADDED Requirements

### Requirement: Backtrack phase targets use prefix-free phase IDs
Backtrack targets in the `backtrack_to` field of eval.json entries SHALL use prefix-free phase IDs. The `phase-next.ts` module SHALL perform phase table lookups and comparisons using these new IDs.

The following backtrack flows are affected (all references updated):

| Scenario | Old Backtrack Target | New Backtrack Target |
|----------|---------------------|----------------------|
| acceptance finds unmet requirements | `01-proposal` | `proposal` |
| code-review finds design deviation | `02-dev-design` | `dev-design` |
| unit-test finds syntax errors | `04-test-gen` | `test-gen` |
| unit-test finds logic errors | `05-implement` | `implement` |
| unit-test finds design issues | `03-test-design` | `test-design` |
| integration-test finds contract mismatch | `02-dev-design` | `dev-design` |
| integration-test finds both issues | `["04-test-gen", "05-implement"]` | `["test-gen", "implement"]` |

#### Scenario: Backtrack to proposal uses new ID
- **WHEN** acceptance-evaluator sets `backtrack_to: "proposal"` in eval.json
- **THEN** `phase_next` detects the backtrack and returns `next_phase: "proposal"`

#### Scenario: Backtrack to dev-design uses new ID
- **WHEN** code-review-evaluator sets `backtrack_to: "dev-design"` in eval.json
- **THEN** `phase_next` detects the backtrack and returns `next_phase: "dev-design"`

#### Scenario: Array backtrack uses prefix-free IDs
- **WHEN** evaluator sets `backtrack_to: ["test-gen", "implement"]` in eval.json
- **THEN** `phase_next` returns `next_phase: "test-gen"` (earliest in phase table)

### Requirement: backtrack hint builder uses prefix-free phase IDs in evaluator prompts
The `buildBacktrackHint()` function SHALL use the phase table values as-is to construct the backtrack hint appended to evaluator prompts. Since the phase IDs in the table are now prefix-free, the backtrack hint SHALL display prefix-free IDs.

#### Scenario: Backtrack hint shows prefix-free IDs
- **WHEN** evaluator is returned for phase `implement` in requirement workflow
- **THEN** the backtrack hint lists available backtrack phases as `proposal`, `dev-design`, `test-design`, `test-gen` (no numeric prefixes)

#### Scenario: Backtrack hint shows test-only phases prefix-free
- **WHEN** evaluator is returned for phase `unit-test` in test-only workflow
- **THEN** the backtrack hint lists available backtrack phases as `proposal`, `code-analyze`, `test-design`, `test-gen` (no numeric prefixes)

## MODIFIED Requirements

### Requirement: Evaluator can mark backtrack target (ID update)
The system SHALL allow Evaluators to set a `backtrack_to` field in their eval JSON, pointing to one or more prior phase identifiers.

The system SHALL permit backtrack from the following evaluators to the following targets (prefix-free IDs):
- acceptance (acceptance, acceptance-evaluator) -> proposal
- code-review (code-review, code-review-evaluator) -> dev-design
- unit-test (unit-test, unit-test-evaluator) -> test-gen, implement, test-design, dev-design
- integration-test (integration-test, integration-test-evaluator) -> test-gen, implement, test-design, dev-design

Test-execution evaluators SHALL NOT backtrack to `proposal` or `acceptance`.

#### Scenario: Acceptance evaluator finds unmet requirement
- **WHEN** acceptance-evaluator finds acceptance_criteria from proposal.md without corresponding test or implementation evidence
- **THEN** eval report has verdict "fail" with backtrack_to set to `"proposal"` (not `"01-proposal"`)

#### Scenario: Unit-test evaluator finds syntax error in test files
- **WHEN** unit-test-evaluator detects a syntax/import error in test files
- **THEN** eval report has verdict "fail" with backtrack_to set to `"test-gen"` (not `"04-test-gen"`)

#### Scenario: Integration-test evaluator finds contract mismatch
- **WHEN** integration-test-evaluator finds interface signature mismatch between test and implementation
- **THEN** eval report has verdict "fail" with backtrack_to set to `"dev-design"` (not `"02-dev-design"`)

## Module Contract

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `buildBacktrackHint()` | UNCHANGED | Uses phase table values as-is (no hardcoded prefixes) |
| `computeAllowedBacktrackPhases()` | UNCHANGED | Uses phase table IDs dynamically |
| `getLatestBacktrackTarget()` | UNCHANGED | Reads backtrack_to as-is from eval.json |
| `handleBacktrack()` | UNCHANGED | Phase table lookup uses new IDs from workflow.ts |
| `resolvePhaseNext()` | UNCHANGED | All internal logic uses phase table values dynamically |

The phase-next backtrack logic contains no hardcoded phase ID strings — all phase identification is delegated to the phase table in `workflow.ts`.
