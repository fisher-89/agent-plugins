## MODIFIED Requirements

### Requirement: New standalone phase agents for unit-test and integration-test
The unit-test-execution-evaluator and integration-test-execution-evaluator are REMOVED as sub-steps of implement/code-review phases.
Instead, the system SHALL provide 4 new agents organized into two standalone EXEC phases:
- **unit-test-executor** (sonnet, Generator role in 06-unit-test phase): runs unit tests, produces structured test report as artifact
- **unit-test-evaluator** (Evaluator role in 06-unit-test phase): reads the executor's report, validates completeness, applies diagnostic decision tree
- **integration-test-executor** (sonnet, Generator role in 08-integration-test phase): runs integration tests, produces structured test report as artifact
- **integration-test-evaluator** (Evaluator role in 08-integration-test phase): reads the executor's report, validates completeness, applies diagnostic decision tree

Each executor-evaluator pair SHALL operate as a full EXEC phase (Generator->Evaluator cycle), where the output artifact is a test execution report, not code.

#### Scenario: Unit-test executor (sonnet) produces test report
- **WHEN** unit-test-executor (sonnet) is invoked during 06-unit-test phase
- **THEN** it runs the unit test command via Bash tool
- **AND** captures test output and coverage data
- **AND** writes a structured report to `reports/unit-test-execution.json`

#### Scenario: Unit-test evaluator validates test report
- **WHEN** unit-test-evaluator is invoked after unit-test-executor completes
- **THEN** it reads the test report file from `reports/unit-test-execution.json`
- **AND** evaluates against its diagnostic checklist (report has all required fields? all tests pass? coverage threshold met?)
- **AND** applies diagnostic decision tree if tests failed

#### Scenario: Integration-test executor (sonnet) produces test report
- **WHEN** integration-test-executor (sonnet) is invoked during 08-integration-test phase
- **THEN** it runs the integration test command via Bash tool
- **AND** captures test output and coverage data
- **AND** writes a structured report to `reports/integration-test-execution.json`

#### Scenario: Integration-test evaluator validates test report
- **WHEN** integration-test-evaluator is invoked after integration-test-executor completes
- **THEN** it reads the test report file from `reports/integration-test-execution.json`
- **AND** evaluates against its diagnostic checklist (report quality, integration tests pass? mock configuration correct? inter-module contracts satisfied?)

### Requirement: Removed sub-step evaluators from implement and code-review
The unit-test-execution-evaluator SHALL NO LONGER exist as a sub-step within the implement phase (05-implement). Its functionality is replaced by the standalone unit-test-executor + unit-test-evaluator pair in the 06-unit-test phase.
The integration-test-execution-evaluator SHALL NO LONGER exist as a sub-step within the code-review phase (07-code-review). Its functionality is replaced by the standalone integration-test-executor + integration-test-evaluator pair in the 08-integration-test phase.

#### Scenario: Implement phase has no test execution agent
- **WHEN** implement phase AUTO step runs
- **THEN** only static-check runs; no test execution agent is invoked
- **AND** eval.json has no unit-test or integration-test entries

#### Scenario: Code-review phase has no test execution agent
- **WHEN** code-review phase runs
- **THEN** only code-review-evaluator is invoked; no integration-test agent is invoked
- **AND** eval.json has exactly one code-review entry

## ADDED Requirements

### Requirement: Executor (sonnet) tool scoping
The `unit-test-executor` SHALL have access to Read, Write, and Bash tools (Bash for running test commands, Write for producing structured report files, Read for reading test output).
The `integration-test-executor` SHALL have access to Read, Write, Bash, Grep, and Glob tools (Grep/Glob for discovering integration test files and inspecting mock configurations).
Both executors SHALL use the sonnet model.

#### Scenario: Unit-test executor runs tests and writes report
- **WHEN** unit-test-executor runs
- **THEN** it has Bash access to run test commands (e.g., `npm test`, `pytest`)
- **AND** it has Write access to write structured report to `reports/unit-test-execution.json`
- **AND** it does NOT have Grep or Glob access (report generation is driven by test output, not code search)

#### Scenario: Integration-test executor discovers and runs integration tests
- **WHEN** integration-test-executor runs
- **THEN** it has Glob access to discover integration test files
- **AND** it has Grep access to search for mock configurations and contract violations
- **AND** it uses sonnet model

### Requirement: Evaluator tool scoping for test phases
The `unit-test-evaluator` SHALL have access to Read and Write tools (Read for reading the test report, Write for appending to eval.json).
The `integration-test-evaluator` SHALL have access to Read, Write, Grep, and Glob tools (Grep/Glob for additional investigation when diagnosis requires it).
Both evaluators SHALL use the opus model.

#### Scenario: Unit-test evaluator reads report and validates
- **WHEN** unit-test-evaluator runs
- **THEN** it has Read access to read the executor's report file and source files for diagnosis
- **AND** it has Write access to append eval.json entries
- **AND** it does NOT need Bash access (tests were already run by executor)

#### Scenario: Integration-test evaluator investigates failures
- **WHEN** integration-test-evaluator detects failures in the report
- **THEN** it has Glob access to discover additional context
- **AND** it has Grep access to investigate root causes

### Requirement: Generator agents have file access constraints
The `test-gen-generator` agent prompt SHALL include a hard-coded file type blacklist: it MUST NOT read `.ts`, `.tsx`, `.js`, `.jsx`, `.py`, `.rs`, `.go`, `.java`, `.c`, `.cpp`, `.h`, `.hpp`, `.cs`, `.rb`, `.php` files.
The `implementation-generator` agent prompt SHALL include a hard-coded directory blacklist: it MUST NOT read files inside `tests/`, `__tests__/`, `test/`, `spec/` directories.
Both constraints SHALL be embedded in the agent system prompt as explicit instructions, not as separate configuration files.

#### Scenario: Test-gen generator prompt includes source file blacklist
- **WHEN** test-gen-generator agent prompt is loaded
- **THEN** the system prompt contains a block listing prohibited file extensions and instructions to avoid reading them

#### Scenario: Implementation generator prompt includes test directory blacklist
- **WHEN** implementation-generator agent prompt is loaded
- **THEN** the system prompt contains a block listing prohibited directories and instructions to avoid reading them

### Requirement: No-op detection in agent flow
The skill layer for EXEC phase invocation SHALL check whether the phase is applicable before invoking the executor agent.
For 06-unit-test: check if any test files exist (Glob `**/*.test.*` or `**/__tests__/`). If none match, skip invocation.
For 08-integration-test: check if any files match integration test patterns (`*.integration.test.*`, `tests/integration/*`). If none match, skip invocation.
The skip decision SHALL produce an eval.json entry with `skipped: true` instead of invoking the executor.

#### Scenario: No unit test files found, unit-test phase skipped
- **WHEN** unit-test phase runs and Glob for `**/*.test.*` returns empty
- **THEN** the unit-test-executor and unit-test-evaluator are NOT invoked
- **AND** eval.json gets a skipped entry with `skipped: true`, verdict "pass"

#### Scenario: No integration test files found, integration-test phase skipped
- **WHEN** integration-test phase runs and Glob for `**/*.integration.test.*` returns empty
- **THEN** integration-test-executor and integration-test-evaluator are NOT invoked
- **AND** eval.json gets a skipped entry with `skipped: true`, verdict "pass"

### Requirement: Architecture agent uses dev-team CLI commands
The `architecture` agent prompt SHALL reference `dev-team archi` CLI commands instead of `python plugins/dev-team/utils/archi-*.py` Python scripts.
All command examples in the agent prompt SHALL use the new CLI syntax:

| Old (Python) | New (CLI) |
|---|---|
| `python plugins/dev-team/utils/archi-model.py --command query` | `dev-team archi query` |
| `python plugins/dev-team/utils/archi-model.py --command validate --source "..."` | `dev-team archi validate --source "..."` |
| `python plugins/dev-team/utils/archi-model.py --command write --path ... --source "..."` | `dev-team archi write --path ... --source "..."` |
| `python plugins/dev-team/utils/archi-validate.py --project-root . --staged` | `dev-team archi check --staged` |
| `python plugins/dev-team/utils/archi-validate.py --project-root . --files "..."` | `dev-team archi check --files "..."` |

#### Scenario: Architecture agent PROPOSE mode validates via CLI
- **WHEN** architecture agent enters PROPOSE mode and needs to validate DSL
- **THEN** it runs `dev-team archi validate --source "<dsl>"` instead of `python archi-model.py --command validate`

#### Scenario: Architecture agent VALIDATE mode checks via CLI
- **WHEN** architecture agent enters VALIDATE mode
- **THEN** it runs `dev-team archi check --staged` instead of `python archi-validate.py --staged`

#### Scenario: Architecture agent writes model via CLI
- **WHEN** architecture agent enters PROPOSE mode and user confirms writing
- **THEN** it runs `dev-team archi write --path "<file>" --source "<dsl>"` instead of `python archi-model.py --command write`
