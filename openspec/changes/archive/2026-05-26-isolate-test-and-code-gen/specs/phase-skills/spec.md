## MODIFIED Requirements

### Requirement: Nine user-triggered phase skills
The system SHALL provide 9 skills: `dev-team:phase-requirements`, `dev-team:phase-test-design`, `dev-team:phase-dev-proposal`, `dev-team:phase-test-gen`, `dev-team:phase-implement`, `dev-team:phase-unit-test`, `dev-team:phase-code-review`, `dev-team:phase-integration-test`, `dev-team:phase-acceptance`.

#### Scenario: DESIGN skill triggers Planner->Evaluator
- **WHEN** user invokes `dev-team:phase-requirements`, `dev-team:phase-test-design`, or `dev-team:phase-dev-proposal`
- **THEN** the skill invokes the corresponding Planner agent (which writes a .md artifact), then the corresponding Evaluator agent (which evaluates with its static checklist) in sequence

#### Scenario: EXECUTION skill triggers Generator->Evaluator (code generation)
- **WHEN** user invokes `dev-team:phase-test-gen`
- **THEN** the skill invokes test-gen-generator agent (writes test files to disk), then test-gen-evaluator agent (inspects git diff + test-design.md) in sequence

#### Scenario: Implementation skill triggers AUTO static-check then evaluator
- **WHEN** implementation-generator completes writing implementation code
- **THEN** static-check (lint + type) runs first automatically
- **AND** if static-check passes, the skill proceeds to implementation-evaluator (inspects git diff)
- **AND** if static-check fails, the skill loops back to implementation-generator with the static-check failure details
- **AND** no unit-test sub-step runs inside the implement phase

#### Scenario: Test execution skill triggers Executor->Evaluator (report generation)
- **WHEN** user invokes `dev-team:phase-unit-test` or `dev-team:phase-integration-test`
- **THEN** the skill invokes the Executor agent (sonnet, runs tests and writes a structured report), then the Evaluator agent (reads the report and applies diagnostic checks) in sequence

#### Scenario: EVALUATOR-ONLY skill triggers Evaluator directly
- **WHEN** user invokes `dev-team:phase-code-review` or `dev-team:phase-acceptance`
- **THEN** the skill invokes ONLY the corresponding Evaluator agent (no Planner, no Generator, no Executor). The Evaluator inspects the codebase/diffs/reports directly and appends result to eval.json.

### Requirement: code-review invokes single evaluator
`dev-team:phase-code-review` SHALL invoke a single evaluator: code-review-evaluator only.
The integration-test-execution-evaluator is NO LONGER part of code-review phase — it has been moved to its own standalone phase skill (`dev-team:phase-integration-test`).
The code-review evaluator output SHALL be appended to eval.json as a single entry.

#### Scenario: code-review runs code-review evaluator only
- **WHEN** user invokes `dev-team:phase-code-review`
- **THEN** the skill invokes code-review-evaluator only
- **AND** eval.json contains exactly one entry with phase "07-code-review"

#### Scenario: code-review does not trigger integration tests
- **WHEN** user invokes `dev-team:phase-code-review`
- **THEN** no integration-test execution is triggered by this skill
- **AND** integration tests are handled separately by `dev-team:phase-integration-test`

## ADDED Requirements

### Requirement: Unit-test phase skill triggers executor then evaluator
`dev-team:phase-unit-test` SHALL invoke two agents sequentially: first unit-test-executor (sonnet), then unit-test-evaluator.
The skill SHALL check if unit test files exist before invoking the executor. If no test files exist, the skill SHALL append a skipped entry to eval.json with `skipped: true` and proceed.
The executor SHALL use the sonnet model.
The evaluator SHALL use the opus model for diagnostic decision making.

#### Scenario: Unit-test executor runs and evaluator validates
- **WHEN** user invokes `dev-team:phase-unit-test` and unit test files exist
- **THEN** the skill invokes unit-test-executor (sonnet), waits for the structured report to be written to `reports/unit-test-execution.json`
- **AND** then invokes unit-test-evaluator to validate the report and apply diagnostic decision tree
- **AND** eval.json contains two entries with phase "06-unit-test" and different phase_suffix values ("executor", "evaluator")

#### Scenario: Unit-test phase skipped (no test files)
- **WHEN** user invokes `dev-team:phase-unit-test` and no test files exist (Glob for `**/*.test.*` returns empty)
- **THEN** the skill does NOT invoke executor or evaluator
- **AND** appends a skipped entry with `skipped: true` and verdict "pass"

### Requirement: Integration-test phase skill triggers executor then evaluator
`dev-team:phase-integration-test` SHALL invoke two agents sequentially: first integration-test-executor (sonnet), then integration-test-evaluator.
The skill SHALL check if integration test files exist before invoking the executor. If no integration test files exist, the skill SHALL append a skipped entry to eval.json with `skipped: true` and proceed.
The executor SHALL use the sonnet model.

#### Scenario: Integration-test executor runs and evaluator validates
- **WHEN** user invokes `dev-team:phase-integration-test` and integration test files exist
- **THEN** the skill invokes integration-test-executor (sonnet), waits for the structured report to be written to `reports/integration-test-execution.json`
- **AND** then invokes integration-test-evaluator to validate the report and apply diagnostic decision tree
- **AND** eval.json contains two entries with phase "08-integration-test" and different phase_suffix values ("executor", "evaluator")

#### Scenario: Integration-test phase skipped (no integration test files)
- **WHEN** user invokes `dev-team:phase-integration-test` and no integration test files exist (Glob for `**/*.integration.test.*` returns empty)
- **THEN** the skill does NOT invoke executor or evaluator
- **AND** appends a skipped entry with `skipped: true` and verdict "pass"
