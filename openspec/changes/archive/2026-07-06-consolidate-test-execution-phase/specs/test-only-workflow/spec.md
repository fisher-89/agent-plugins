## MODIFIED Requirements

### Requirement: test-only five-phase workflow structure (was six)

The system SHALL support a `test-only` workflow_type with 5 phases (reduced from 6) optimized for supplementing test coverage on already-implemented code:

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| 01-proposal | proposal | DESIGN Planner→Evaluator | Test-focused proposal |
| 02-code-analyze | code-analyze | DESIGN Planner→Evaluator | Reverse-engineer existing code architecture |
| 03-test-design | test-design | DESIGN Planner→Evaluator | Test scenario design |
| 04-test-gen | test-gen | EXEC Generator→Evaluator | Test code generation |
| 05-test-execution | test-execution | EXEC Executor→Evaluator | Test execution (unit + integration combined) |

Phase `integration-test` SHALL NOT appear in the `test-only` phase table.

The `test-only` workflow SHALL NOT include an acceptance phase. Workflow completion SHALL occur when all 5 phases in the table have valid (non-stale) pass entries.

#### Scenario: test-only phase table excludes integration-test

- **WHEN** `getPhaseTable("test-only")` is called
- **THEN** it returns exactly 5 phase definitions with IDs: `proposal`, `code-analyze`, `test-design`, `test-gen`, `test-execution`
- **AND** does NOT include `integration-test`, `unit-test`, `dev-design`, `implement`, `code-review`, or `acceptance`

#### Scenario: test-execution uses test-execution-executor agent

- **WHEN** Phase `test-execution` executes in the `test-only` workflow
- **THEN** the executor agent_type is `dev-team:test-execution-executor`
- **AND** the evaluator agent_type is `dev-team:test-execution-evaluator`

### Requirement: test-only prerequisite dependency table (updated)

The `test-only` workflow_type SHALL define explicit prerequisites:

| Phase | Prerequisites |
|-------|--------------|
| proposal | [] |
| code-analyze | [proposal] |
| test-design | [proposal, code-analyze] |
| test-gen | [test-design] |
| test-execution | [test-gen] |

`getPrerequisites(phaseId, "test-only")` SHALL return the prerequisites from this table.
`getDependents(phaseId, "test-only")` SHALL derive the reverse mapping from this table.

#### Scenario: getPrerequisites returns test-only dependencies

- **WHEN** `getPrerequisites("code-analyze", "test-only")` is called
- **THEN** it returns `["proposal"]`

- **WHEN** `getPrerequisites("test-execution", "test-only")` is called
- **THEN** it returns `["test-gen"]`

#### Scenario: getPrerequisites no longer returns integration-test

- **WHEN** `getPrerequisites("integration-test", "test-only")` is called
- **THEN** it returns `[]` (unknown phase, fault-tolerant)

## REMOVED Requirements

### Requirement: test-only six-phase workflow structure (with integration-test)
**Reason**: The test-only workflow had 6 phases including `integration-test`. With consolidation, all test execution is handled by `test-execution`, reducing to 5 phases.

**Migration**: Phase table reduced from 6 to 5 entries. The `integration-test` entry is removed.

### Requirement: 08-integration-test in test-only phase table
**Reason**: Integration-test phase no longer exists in any workflow.

**Migration**: Replace with `test-execution` phase. The `test-execution` phase runs all test files.

### Requirement: integration-test phase in test-only prerequisite table
**Reason**: The `integration-test` key and its prerequisites are removed from the test-only prerequisite table.

**Migration**: Remove the `integration-test: ["test-gen"]` entry.

## Module Contract

### test-only Phase Table (before vs after)

| Before | After |
|--------|-------|
| proposal | proposal |
| code-analyze | code-analyze |
| test-design | test-design |
| test-gen | test-gen |
| **unit-test** | **test-execution** (renamed) |
| **integration-test** | **REMOVED** |

### test-only Phase Count

| Metric | Before | After |
|--------|--------|-------|
| Phase count | 6 | 5 |
| Unique phases | proposal, code-analyze, test-design, test-gen, unit-test, integration-test | proposal, code-analyze, test-design, test-gen, test-execution |
