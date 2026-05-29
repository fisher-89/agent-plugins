## MODIFIED Requirements

### Requirement: Nine-phase workflow structure
The system SHALL support 9 sequential phases with the following identifiers, order, and execution patterns:

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| 01-requirements | 01-requirements | DESIGN Planner->Evaluator | Requirements analysis |
| 02-dev-design | 02-dev-design | DESIGN Planner->Evaluator | Implementation design (formerly dev-proposal) |
| 03-test-design | 03-test-design | DESIGN Planner->Evaluator | Test scenario design |
| 04-test-gen | 04-test-gen | EXEC Generator->Evaluator | Test code generation |
| 05-implement | 05-implement | EXEC Generator->Evaluator + AUTO static-check | Implementation code generation |
| 06-unit-test | 06-unit-test | EXEC Executor->Evaluator (sonnet) | Unit test execution and report validation |
| 07-code-review | 07-code-review | EVAL-ONLY Evaluator | Code review evaluation |
| 08-integration-test | 08-integration-test | EXEC Executor->Evaluator (sonnet) | Integration test execution and report validation |
| 09-acceptance | 09-acceptance | EVAL-ONLY Evaluator | Acceptance evaluation |

The phase formerly named `03-dev-proposal` is RENAMED to `02-dev-design` and moved before `03-test-design`.
The phase formerly named `02-test-design` is RENUMBERED to `03-test-design` and moved after `02-dev-design`.

Each phase SHALL append its result to eval.json upon completion (pass, fail, or skipped).

#### Scenario: Full workflow progression
- **WHEN** all phases complete successfully from 01-requirements through 09-acceptance
- **THEN** eval.json contains entries for all 9 phases with verdict "pass"

#### Scenario: Phase ordering enforcement
- **WHEN** a phase attempts to execute before all prior phases have passed (or been skipped)
- **THEN** eval-check SHALL block the phase and list the missing prior phases

#### Scenario: dev-design runs before test-design
- **WHEN** user invokes `dev-team:phase-test-design`
- **THEN** eval-check SHALL require prior phases [01-requirements, 02-dev-design] to have pass records
- **AND** 02-dev-design (formerly 03-dev-proposal) SHALL precede 03-test-design in the workflow
