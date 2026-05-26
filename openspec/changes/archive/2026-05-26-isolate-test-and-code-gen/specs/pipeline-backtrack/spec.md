## MODIFIED Requirements

### Requirement: Evaluator can mark backtrack target
The system SHALL allow Evaluators to set a `backtrack_to` field in their eval JSON, pointing to a prior phase identifier (e.g., "01-requirements", "03-dev-proposal").
The system SHALL permit backtrack from the following evaluators to the following targets:
- acceptance (09-acceptance, acceptance-evaluator) -> requirements (01-requirements)
- code-review (07-code-review, code-review-evaluator) -> dev-proposal (03-dev-proposal)
- unit-test (06-unit-test, unit-test-evaluator) -> test-gen (04-test-gen), implement (05-implement), test-design (02-test-design), dev-proposal (03-dev-proposal)
- integration-test (08-integration-test, integration-test-evaluator) -> test-gen (04-test-gen), implement (05-implement), test-design (02-test-design), dev-proposal (03-dev-proposal)
The test-execution evaluators SHALL NOT backtrack to requirements (01-requirements) or acceptance (09-acceptance).

#### Scenario: Acceptance evaluator finds unmet requirement
- **WHEN** acceptance-evaluator finds acceptance_criteria from proposal.md without corresponding test or implementation evidence
- **THEN** eval report has verdict "fail" with backtrack_to set to "01-requirements"

#### Scenario: Code review evaluator finds design deviation
- **WHEN** code-review-evaluator finds implementation that contradicts design.md
- **THEN** eval report has verdict "fail" with backtrack_to set to "03-dev-proposal"

#### Scenario: Unit-test evaluator finds syntax error in test files
- **WHEN** unit-test-evaluator (in 06-unit-test phase) detects a syntax/import error in test files from the executor's report
- **THEN** eval report has verdict "fail" with backtrack_to set to "04-test-gen"

#### Scenario: Unit-test evaluator finds logic error in implementation
- **WHEN** unit-test-evaluator (in 06-unit-test phase) finds assertion failures pointing to implementation code
- **THEN** eval report has verdict "fail" with backtrack_to set to "05-implementation"

#### Scenario: Integration-test evaluator finds contract mismatch
- **WHEN** integration-test-evaluator (in 08-integration-test phase) finds interface signature mismatch between test and implementation
- **THEN** eval report has verdict "fail" with backtrack_to set to "03-dev-proposal"

#### Scenario: Integration-test evaluator backtracks to unit-test
- **WHEN** integration-test-evaluator finds that unit-test phase report was incomplete (missing coverage data or key test scenarios)
- **THEN** eval report has verdict "fail" with backtrack_to set to "06-unit-test"

#### Scenario: Integration-test evaluator backtracks to code-review
- **WHEN** integration-test-evaluator identifies a structural issue that should have been caught by code-review
- **THEN** eval report has verdict "fail" with backtrack_to set to "07-code-review"

### Requirement: Backtrack triggers prior phase re-evaluation
When a phase skill detects a `backtrack_to` marker targeting its phase in the change's eval reports, it SHALL re-run the Evaluator for that phase before allowing progression, and SHALL clear the backtrack marker after evaluation passes.

#### Scenario: Requirements skill detects backtrack marker
- **WHEN** user invokes `dev-team:phase-requirements` and a backtrack marker pointing to "01-requirements" exists
- **THEN** the skill re-runs requirements-evaluator against the current proposal.md before showing the Planner prompt

#### Scenario: Test-gen skill detects backtrack marker
- **WHEN** user invokes `dev-team:phase-test-gen` and a backtrack marker pointing to "04-test-gen" exists
- **THEN** the skill re-runs test-gen-evaluator against the current test files before showing the Generator prompt

#### Scenario: Unit-test phase backtrack triggers executor re-run
- **WHEN** user invokes `dev-team:phase-unit-test` and a backtrack marker pointing to "06-unit-test" exists
- **THEN** the skill re-runs unit-test-executor (sonnet) and unit-test-evaluator before proceeding

#### Scenario: Backtrack marker cleared after pass
- **WHEN** re-evaluation passes with verdict "pass"
- **THEN** the backtrack_to field is set to null in the eval report

## ADDED Requirements

### Requirement: Backtrack target validation for test-execution evaluators
When a test-execution evaluator (unit-test-evaluator or integration-test-evaluator) sets a `backtrack_to` field, the system SHALL validate that the target phase is within the permitted set.
Permitted targets for unit-test-evaluator: test-design (02-test-design), dev-proposal (03-dev-proposal), test-gen (04-test-gen), implement (05-implement).
Permitted targets for integration-test-evaluator: test-design (02-test-design), dev-proposal (03-dev-proposal), test-gen (04-test-gen), implement (05-implement).
If the test-execution evaluator attempts to backtrack to an invalid target (e.g., requirements or acceptance), the skill SHALL override the backtrack_to to dev-proposal (03-dev-proposal) (safe default) and log a warning in eval.json.

#### Scenario: Unit-test evaluator tries invalid backtrack
- **WHEN** unit-test-evaluator sets backtrack_to to "01-requirements"
- **THEN** the skill detects this is outside the permitted set for unit-test-evaluator
- **AND** overrides backtrack_to to "03-dev-proposal"
- **AND** logs "单元测试 Evaluator 试图回溯到 requirements 阶段，已自动修正为 dev-proposal（安全默认）" in the eval entry

#### Scenario: Integration-test evaluator tries invalid backtrack
- **WHEN** integration-test-evaluator sets backtrack_to to "09-acceptance"
- **THEN** the skill overrides backtrack_to to "03-dev-proposal"
- **AND** logs diagnostic warning in the eval entry

### Requirement: Backtrack chain integrity for new paths
When the diagnostic decision tree produces a backtrack_to target, the system SHALL verify that the target phase precedes the current phase in the workflow sequence: test-design < dev-proposal < test-gen < implement < unit-test < code-review < integration-test < acceptance.
If the backtrack target is a phase that has no entries in eval.json, the skill SHALL override backtrack_to to dev-proposal (03-dev-proposal) as a safe default.
If backtrack would create a cycle (e.g., unit-test -> implement -> unit-test), the system SHALL detect the cycle and require manual resolution by outputting the cycle path. For diagnostic test-execution evaluators, a cycle is defined as: backtrack_to targeting a phase that already has a non-null backtrack_to pointing back to the current evaluator's phase.

#### Scenario: Backtrack target phase has no entries
- **WHEN** integration-test-evaluator sets backtrack_to to "04-test-gen" but test-gen phase has no entries in eval.json
- **THEN** the skill overrides backtrack_to to "03-dev-proposal"
- **AND** logs "回溯目标 test-gen 阶段未执行，自动修正为 dev-proposal（安全默认）"

#### Scenario: Cycle detected between unit-test and implement
- **WHEN** unit-test-evaluator sets backtrack_to to "05-implementation", and implement phase's latest entry has backtrack_to set to "06-unit-test"
- **THEN** the eval report includes a warning and the skill pauses for user intervention with the cycle path
