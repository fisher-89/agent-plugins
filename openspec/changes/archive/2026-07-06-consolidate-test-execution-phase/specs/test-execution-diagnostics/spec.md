## MODIFIED Requirements

### Requirement: Executor (sonnet) executes all tests and produces structured report

test-execution-executor SHALL be invoked in the consolidated `test-execution` phase, responsible for executing all automated tests and producing a structured report file.

Executor SHALL use sonnet model.
Executor SHALL execute test commands (as determined by the framework detection plan), capturing stdout/stderr and exit code.
Executor SHALL extract from test output: total test cases, passed, failed, skipped, coverage percentages (for all test types combined).
Executor SHALL write the structured report file to `reports/test-execution.json`.

Report file SHALL contain the following fields:
- `phase`: `"test-execution"` (was `"06-unit-test"` or `"08-integration-test"`)
- `command`: actual test command executed
- `timestamp`: ISO 8601 timestamp
- `total`: total test cases
- `passed`: passed count
- `failed`: failed count
- `skipped`: skipped count
- `duration_seconds`: execution duration
- `test_cases`: test case detail array (same schema as before)
- `coverage`: coverage nested object or null (same schema as before)
- `findings`: optional diagnostic information string

**Changes from previous version**:
- Phase identifier: `test-execution` (was `06-unit-test` or `08-integration-test`)
- Report path: `reports/test-execution.json` (was `reports/unit-test-execution.json` or `reports/integration-test-execution.json`)
- Scope: all automated tests combined (was split into unit/integration)
- No separate integration-test sub-report (`integration_test` field REMOVED from report schema)

#### Scenario: test-execution Executor runs all tests and produces report

- **WHEN** test-execution-executor is called in the `test-execution` phase
- **THEN** it executes the project's test commands (for all detected frameworks)
- **AND** it extracts test case statistics and coverage metrics from all test output
- **AND** it writes the data to `reports/test-execution.json`
- **AND** the report includes all required fields with `phase: "test-execution"`

#### Scenario: No separate integration_test field in report

- **WHEN** the executor writes the report
- **THEN** the report SHALL NOT contain an `integration_test` field
- **AND** all test results (unit + integration) are combined in the top-level `test_cases`, `total`, `passed`, `failed`, `skipped` fields

## REMOVED Requirements

### Requirement: Executor (sonnet) executes tests and produces structured report (for integration-test)

**Reason**: The integration-test phase has been removed. All test execution is now handled by the consolidated `test-execution` phase.

**Migration**: Integration test results are now included in the `reports/test-execution.json` report alongside unit test results. No separate report file is generated.

### Requirement: phase field 06-unit-test or 08-integration-test in report

**Reason**: Both separate phase identifiers are replaced by `test-execution`.

**Migration**: The report `phase` field is now `"test-execution"` for all automated test execution.

### Requirement: integration_test sub-report object in report schema

**Reason**: With the consolidation, there is no separate integration test sub-report. All test results are combined.

**Migration**: The `integration_test` field is removed from the report schema. All test cases (unit + integration) appear in the top-level `test_cases` array.

### Requirement: Integration-test Executor runs and produces report

**Reason**: Integration-test executor and its report are no longer needed.

**Migration**: All integration test files are now run by the test-execution executor as part of the framework's regular test execution.

## Module Contract

### Report Schema Changes

| Field | Before | After |
|-------|--------|-------|
| `phase` | `"06-unit-test"` or `"08-integration-test"` or `"unit-test"` or `"integration-test"` | `"test-execution"` |
| `integration_test` | Optional sub-report object | REMOVED |
| Report path | `reports/unit-test-execution.json` or `reports/integration-test-execution.json` | `reports/test-execution.json` |
