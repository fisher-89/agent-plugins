## MODIFIED Requirements

### Requirement: dev-team test-execution CLI command (renamed from unit-test)

`plugins/dev-team/bin/src/cli.ts` SHALL register a `test-execution` subcommand instead of `unit-test`, associated with `commands/test-execution.ts` action handler. The command SHALL support `--project-root <path>` option. When executed, SHALL call `runTestDetectFrameworks({})` to get the framework plan.

**Changes from previous version**:
- CLI command name: `unit-test` → `test-execution`
- Handler file: `commands/unit-test.ts` → `commands/test-execution.ts`
- Report output paths: `reports/unit-test-execution.json` → `reports/test-execution.json`
- Sub-report paths: `reports/unit-test/<fw>.json` → `reports/test-execution/<fw>.json`
- Schema file: `schemas/unit-test-output.schema.ts` → `schemas/test-execution-output.schema.ts`

#### Scenario: test-execution subcommand registered

**WHEN** `dev-team test-execution` is called
**THEN** CLI SHALL parse the subcommand and route to `runTestExecution` function
**AND** `runTestExecution` SHALL call `runTestDetectFrameworks({})` to get the framework plan
**AND** SHALL execute each framework in the plan

#### Scenario: --project-root option support

**WHEN** `dev-team test-execution --project-root /custom/path` is called
**THEN** SHALL use `/custom/path` as project root for all internal functions
**AND** test report files SHALL be written to `/custom/path/reports/`

#### Scenario: deprecated unit-test command

**WHEN** `dev-team unit-test` is called
**THEN** CLI SHALL output a deprecation message: "`unit-test` has been renamed to `test-execution`. Please use `dev-team test-execution`."
**AND** SHALL still route to `runTestExecution` (backward compatibility)

### Requirement: Schema file renamed and updated

`schemas/unit-test-output.schema.ts` SHALL be renamed to `schemas/test-execution-output.schema.ts`. The `phase` field in the summary report schema SHALL be updated to accept `"test-execution"` instead of `"06-unit-test"` or `"unit-test"`.

#### Scenario: schema phase field updated

**WHEN** reading `schemas/test-execution-output.schema.ts`
**THEN** the SummaryReport schema `phase` field references `"test-execution"`
**AND** the SubReport schema `framework` field is unchanged

## ADDED Requirements

### Requirement: CLI source file renamed

**ID**: REQ-TEC-1
**Priority**: MUST
**Description**: `commands/unit-test.ts` SHALL be renamed to `commands/test-execution.ts`. The exported interface `UnitTestOptions` SHALL be renamed to `TestExecutionOptions`. The exported function `runUnitTest` SHALL be renamed to `runTestExecution`. All internal type names SHALL follow the same naming convention.

#### Scenario: renamed exports exist

**WHEN** importing from `commands/test-execution.ts`
**THEN** `runTestExecution` is available (not `runUnitTest`)
**AND** `TestExecutionOptions` type is available (not `UnitTestOptions`)

## REMOVED Requirements

### Requirement: dev-team unit-test CLI command exists
**Reason**: CLI command renamed from `unit-test` to `test-execution`.

**Migration**: Backward-compatible alias provided. All internal references updated to new command name.

### Requirement: Schema file at unit-test-output.schema.ts
**Reason**: Schema file renamed to `test-execution-output.schema.ts`.

**Migration**: All imports updated to new file path.

## Module Contract

### Module: commands/test-execution.ts (CLI Action Handler)

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/commands/test-execution.ts` |
| Exports | `runTestExecution(options: TestExecutionOptions): Promise<TestExecutionExitCode>` |
| Input | `TestExecutionOptions`: `{ projectRoot?: string }` |
| Output | `TestExecutionExitCode`: `0` / `1` |
| Side Effects | Execute shell commands; read/write `reports/test-execution/<fw>.json` and `reports/test-execution.json` |

### Module: schemas/test-execution-output.schema.ts (renamed)

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` |
| Exports | `testExecutionSubReportSchema`, `testExecutionSummaryReportSchema`, `TestExecutionSubReport`, `TestExecutionSummaryReport` |
| Phase field | `"test-execution"` (was `"06-unit-test"` or `"unit-test"`) |
