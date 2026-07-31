## MODIFIED Requirements

### Requirement: Executor (sonnet) executes all tests and produces structured report

test-execution-executor SHALL be invoked in the consolidated `test-execution` phase, responsible for executing all automated tests and producing a structured report file.

Executor SHALL use sonnet model.
Executor SHALL invoke the `dev-team test-execution` CLI（而非直接抓取测试命令 stdout 作为权威结果）。
CLI SHALL 通过文件通道采集各 plan 结果并写入结构化报告。
Executor SHALL read the structured summary at `reports/test/summary.json`（有 change 时带 `openspec/changes/<change>/` 前缀）and may enrich `findings`.

Report file SHALL contain the following fields:
- `phase`: `"test-execution"`
- `command`: actual test command / CLI invocation metadata
- `timestamp`: ISO 8601 timestamp
- `total`: total test cases
- `passed`: passed count
- `failed`: failed count
- `skipped`: skipped count
- `duration_seconds`: execution duration（或等价 duration 字段，与现 schema 一致）
- `coverage`: coverage nested object or null
- `mutation`: mutation nested object or null（若启用）
- `problems`: problem list
- `plans`: path index array（`id` / `framework` / `directory` / `path`）
- `findings`: optional diagnostic information string

**Changes from previous version**:
- Report path: `reports/test-execution.json` → `reports/test/summary.json`
- Atomic reports: `reports/test/<planId>/report.json`（经 `plans[]`）
- Collection: stdout JSON parse → plan 目录文件通道
- Scope: all automated tests combined（不变）

#### Scenario: test-execution Executor runs all tests and produces report

**WHEN** test-execution-executor is called in the `test-execution` phase
**THEN** it invokes `dev-team test-execution` for all detected frameworks
**AND** the CLI writes summary data to `reports/test/summary.json`
**AND** the report includes required fields with `phase: "test-execution"` and `plans[]`

#### Scenario: No separate integration_test field in report

**WHEN** the executor / CLI writes the report
**THEN** the report SHALL NOT contain an `integration_test` field
**AND** all test results (unit + integration) are combined in the top-level aggregates

#### Scenario: diagnostics use plans index for per-plan details

**WHEN** summary `failed > 0` or `problems` 非空
**THEN** executor diagnostics SHALL 通过 `plans[].path` 打开对应 `report.json`
**AND** SHALL NOT 依赖捕获的测试命令 stdout 作为权威明细

## REMOVED Requirements

### Requirement: Executor (sonnet) executes tests and produces structured report (for integration-test)

**Reason**: The integration-test phase has been removed. All test execution is now handled by the consolidated `test-execution` phase.

**Migration**: Integration test results are now included in the `reports/test/summary.json` report alongside unit test results. No separate report file is generated.

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
| Report path | `reports/test-execution.json` | `reports/test/summary.json` |
| Atomic report | `reports/test-execution/<planId>.json` | `reports/test/<planId>/report.json` |
| `plans` | 不存在 | `Array<{ id, framework, directory, path }>` |
| `phase` | `"test-execution"` | `"test-execution"`（不变） |
| Collection | stdout / 旧 coverage 路径 | plan 目录文件通道 |
| `integration_test` | Optional sub-report object | REMOVED |
