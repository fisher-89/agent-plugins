## MODIFIED Requirements

### Requirement: AUTO phase has no PGE overhead (static-check only)
The system SHALL execute the AUTO phase (static-check) automatically after implementation Generator completes, without human invocation.
The AUTO phase SHALL execute as a single step: static-check (lint + type-check).
Unit-test execution is NO LONGER part of the implement phase AUTO step — it has been moved to its own standalone phase (06-unit-test).
If static-check fails, the system SHALL NOT proceed to the unit-test phase — it SHALL immediately return verdict "fail" with backtrack_to set to implement phase.

#### Scenario: Static-check fails, no further phases triggered
- **WHEN** implementation Generator completes writing code, static-check finds lint or type errors
- **THEN** the system reports the static-check failures
- **AND** the unit-test phase (06-unit-test) is NOT triggered
- **AND** the system loops back to the implementation Generator with the static-check failure details

#### Scenario: Static-check passes, proceed to unit-test phase
- **WHEN** static-check passes successfully
- **THEN** the system proceeds to the unit-test phase (06-unit-test) for test execution and report generation

#### Scenario: Static-check output recorded in eval.json
- **WHEN** static-check completes (pass or fail)
- **THEN** an entry is appended to eval.json with phase "05-implementation", phase_suffix "static-check", and the appropriate verdict
- **AND** the report includes lint errors or type errors (if any)

### Requirement: code-review executes single evaluator
The system SHALL execute code-review phase as a single evaluator (code-review-evaluator only).
The integration-test-execution evaluator is NO LONGER part of the code-review phase — it has been moved to its own standalone phase (08-integration-test).
The code-review evaluator SHALL execute with its static checklist (security, test coverage, error handling, code quality).
The code-review phase SHALL produce a single entry in eval.json.

#### Scenario: Single evaluator in code-review
- **WHEN** code-review phase is invoked
- **THEN** the skill invokes code-review-evaluator only
- **AND** eval.json contains exactly one code-review phase entry

### Requirement: No-op phase skipping
A phase SHALL be skippable when the skill layer determines no work is required for that phase in the current change scope.
The skill layer SHALL check phase applicability before invoking the executor/evaluator. If the check indicates the phase is not applicable, the skill SHALL append a "skipped" entry to eval.json with verdict "pass" and evidence "skipped: no applicable tests" (or similar reason).
A skipped phase SHALL be treated as "passed" by the eval-check validator — it does not block archive flow.
The eval.json entry for a skipped phase SHALL include the field `"skipped": true`.
The following phases support no-op skipping:
- 06-unit-test: skipped when no unit test files exist (no `*.test.*`, `tests/unit/` or `__tests__/` files)
- 08-integration-test: skipped when no integration test files exist (no `*.integration.test.*` or `tests/integration/` files)

#### Scenario: Unit-test phase is skipped
- **WHEN** unit-test phase skill determines the current change has no test files (Glob for `**/*.test.*` returns empty)
- **THEN** the skill appends `{phase: "06-unit-test", skipped: true, verdict: "pass", evidence: "skipped: no applicable tests"}` to eval.json
- **AND** does not invoke the unit-test executor or evaluator

#### Scenario: Integration-test phase is skipped
- **WHEN** integration-test phase skill determines the current change has no integration test files (no files matching `*.integration.test.*` or `tests/integration/`)
- **THEN** the skill appends `{phase: "08-integration-test", skipped: true, verdict: "pass", evidence: "skipped: no applicable tests"}` to eval.json
- **AND** does not invoke the integration-test executor or evaluator

#### Scenario: Skipped phase passes eval-check
- **WHEN** eval-check runs and encounters a phase entry with `skipped: true`
- **THEN** it treats the phase as valid (equivalent to "pass") and does not block archive flow

## ADDED Requirements

### Requirement: Nine-phase workflow structure
The system SHALL support 9 sequential phases with the following identifiers, order, and execution patterns:

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| 01-requirements | 01-requirements | DESIGN Planner->Evaluator | Requirements analysis |
| 02-test-design | 02-test-design | DESIGN Planner->Evaluator | Test scenario design |
| 03-dev-proposal | 03-dev-proposal | DESIGN Planner->Evaluator | Implementation design |
| 04-test-gen | 04-test-gen | EXEC Generator->Evaluator | Test code generation |
| 05-implement | 05-implement | EXEC Generator->Evaluator + AUTO static-check | Implementation code generation |
| 06-unit-test | 06-unit-test | EXEC Executor->Evaluator (sonnet) | Unit test execution and report validation |
| 07-code-review | 07-code-review | EVAL-ONLY Evaluator | Code review evaluation |
| 08-integration-test | 08-integration-test | EXEC Executor->Evaluator (sonnet) | Integration test execution and report validation |
| 09-acceptance | 09-acceptance | EVAL-ONLY Evaluator | Acceptance evaluation |

Each phase SHALL append its result to eval.json upon completion (pass, fail, or skipped).

#### Scenario: Full workflow progression
- **WHEN** all phases complete successfully from 01-requirements through 09-acceptance
- **THEN** eval.json contains entries for all 9 phases with verdict "pass"

#### Scenario: Phase ordering enforcement
- **WHEN** a phase attempts to execute before all prior phases have passed (or been skipped)
- **THEN** eval-check SHALL block the phase and list the missing prior phases

### Requirement: EXEC phase for standalone test execution
The unit-test phase (06-unit-test) and integration-test phase (08-integration-test) SHALL use the EXEC (Executor->Evaluator) pattern.
Executor SHALL use the sonnet model, not the opus model used by DESIGN phase Planners.
Executor SHALL produce a structured test execution report file as its primary artifact (not code).
Evaluator SHALL read the Executor's report file, validate its completeness against a checklist, and apply the diagnostic decision tree.
If Executor fails to produce a valid report (missing critical fields), Evaluator SHALL set verdict to "fail" with "report_incomplete" in findings.

#### Scenario: Unit-test phase executor (sonnet) runs and produces report
- **WHEN** unit-test phase executor (sonnet) is invoked
- **THEN** it runs the configured unit test command (`npm test`, `pytest`, `go test`, etc.)
- **AND** captures stdout/stderr, exit code, and coverage data
- **AND** writes a structured report to `reports/unit-test-execution.json` containing total tests, passed, failed, skipped, coverage percentage, failure details, and execution duration

#### Scenario: Unit-test phase evaluator validates report
- **WHEN** unit-test phase evaluator is invoked
- **THEN** it reads `reports/unit-test-execution.json`
- **AND** validates that the report contains all required fields (total, passed, failed, skipped, coverage, failures, duration)
- **AND** if all tests pass and coverage meets threshold, sets verdict to "pass"
- **AND** if tests fail, applies the diagnostic decision tree and sets backtrack_to

#### Scenario: Integration-test phase executor (sonnet) runs and produces report
- **WHEN** integration-test phase executor (sonnet) is invoked
- **THEN** it runs the configured integration test command
- **AND** writes a structured report to `reports/integration-test-execution.json` with the same schema as unit-test reports

#### Scenario: Integration-test phase evaluator validates report
- **WHEN** integration-test phase evaluator is invoked
- **THEN** it reads `reports/integration-test-execution.json`
- **AND** validates report completeness and applies diagnostic decision tree
- **AND** may backtrack to unit-test (06-unit-test) or code-review (07-code-review) in addition to earlier phases

### Requirement: Diagnostic evaluator output format
The evaluator in unit-test and integration-test phases SHALL append entries to eval.json that include all standard fields (phase, timestamp, attempt, verdict, report, items, backtrack_to) plus an additional `findings` field.
The `findings` field SHALL contain a human-readable string describing the root cause analysis process and the decision reached by the diagnostic decision tree.
The `report` field SHALL include a summary of test execution results: total tests, passed, failed, skipped, and coverage percentage.

#### Scenario: Diagnostic evaluator output includes findings
- **WHEN** unit-test phase evaluator runs and detects a syntax error in test files from the executor's report
- **THEN** eval.json entry includes `findings: "语法错误在测试文件行，回溯到 test-gen 阶段"` and `backtrack_to: "04-test-gen"`
- **AND** `report` includes "total: 10, passed: 5, failed: 3, skipped: 2, coverage: 72%"
