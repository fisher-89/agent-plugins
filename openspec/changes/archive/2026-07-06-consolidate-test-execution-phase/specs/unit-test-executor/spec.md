## MODIFIED Requirements

### Requirement: Agent (test-execution-executor) reads CLI summary report and adds diagnostics

**ID**: REQ-TEE-DIAG-1
**Priority**: MUST
**Description**: The test-execution-executor agent SHALL read the CLI-generated summary report `reports/test-execution.json` and add diagnostic information to the `findings` field. The agent SHALL:
1. Read `conclusion` — if `"fail"`, analyze `problems[]` for patterns
2. Read `coverage` — if non-null and `pass: false`, identify which dimension(s) fall short
3. Read `coverage.by_framework` — if a framework has unexpectedly low coverage, suggest possible causes (missing tests, config issue)
4. Write diagnostic text to `findings` field (append or override)
5. Leave all other fields unchanged

**Changes from previous spec**:
- Agent file renamed: `unit-test-executor.md` → `test-execution-executor.md`
- Report file path: `reports/test-execution.json` (was `reports/unit-test-execution.json`)
- Sub-report path: `reports/test-execution/<framework>.json` (was `reports/unit-test/<framework>.json`)
- Phase: `test-execution` (was `unit-test`)
- Agent no longer distinguishes "unit test" vs "integration test" — runs ALL tests found by framework plan

#### Scenario: Agent analyzes coverage failure reasons

**WHEN** `conclusion` 为 `"fail"` 且 `problems` 包含覆盖率不达标的条目
**THEN** 代理 SHALL 在 `findings` 中添加诊断消息，指出具体哪个维度不达标
**AND** SHALL 建议可能的根因（如：该框架缺少测试文件、覆盖率配置错误）

#### Scenario: Agent analyzes test failure reasons

**WHEN** `failed > 0` 且 `problems` 包含失败用例
**THEN** 代理 SHALL 读取子报告 `reports/test-execution/<framework>.json` 中的失败用例详情
**AND** SHALL 在 `findings` 中总结失败模式（如：大量超时、特定模块失败）
**AND** SHALL NOT 修改 `test_cases` 或 `conclusion`

### Requirement: Agent runs all tests via CLI (unit + integration combined)

**ID**: REQ-TEE-ALL-1
**Priority**: MUST
**Description**: The `dev-team test-execution` CLI command SHALL run ALL test files discovered by the framework plan, without distinguishing between unit and integration tests. For each framework in the plan (`test_detect_frameworks` result), the CLI SHALL:
1. Run the framework's `script` which executes all matching test files
2. Parse test output and coverage for all test files combined
3. Generate a single summary report at `reports/test-execution.json`
4. Generate per-framework sub-reports at `reports/test-execution/<framework>.json`

Frameworks that naturally distinguish test types (e.g., Go's `_test.go` for unit, custom integration test files) SHALL NOT be split into separate executions. All test files matching the framework's glob patterns SHALL be executed together.

#### Scenario: vitest runs all test files including __tests__/

**WHEN** the project uses vitest
**THEN** `dev-team test-execution` SHALL run vitest on all files matching `**/*.{test,spec}.{js,ts,jsx,tsx}`
**AND** files in `__tests__/` directories are included (they match vitest's default glob)
**AND** a single sub-report is generated at `reports/test-execution/vitest.json`

#### Scenario: go test runs all ./... packages

**WHEN** the project uses Go
**THEN** `dev-team test-execution` SHALL run `go test -json -cover ...` across all packages
**AND** unit tests (`*_test.go`) and integration tests (with `//go:build integration` tags) are both executed
**AND** a single sub-report is generated at `reports/test-execution/go.json`

#### Scenario: pytest runs all tests/ files

**WHEN** the project uses pytest
**THEN** `dev-team test-execution` SHALL run `pytest` across all test files matching `**/test_*.py`
**AND** unit and integration test files are both executed together
**AND** a single sub-report is generated at `reports/test-execution/pytest.json`

### Requirement: Report file paths updated from unit-test to test-execution

**ID**: REQ-TEE-PATH-1
**Priority**: MUST
**Description**: All CLI output paths SHALL be updated from the `reports/unit-test/` namespace to `reports/test-execution/`:

| Old Path | New Path |
|----------|----------|
| `reports/unit-test-execution.json` | `reports/test-execution.json` |
| `reports/unit-test/<framework>.json` | `reports/test-execution/<framework>.json` |
| `reports/coverage/<framework>/` (via unit-test) | `reports/coverage/<framework>/` (unchanged — coverage output paths are framework-relative) |

The summary report `phase` field SHALL use `"test-execution"` instead of `"06-unit-test"` or `"unit-test"`.

#### Scenario: CLI writes report to new paths

**WHEN** `dev-team test-execution` completes successfully
**THEN** summary report exists at `reports/test-execution.json`
**AND** sub-reports exist at `reports/test-execution/<framework>.json`

#### Scenario: Report phase field updated

**WHEN** reading `reports/test-execution.json` phase field
**THEN** it equals `"test-execution"` (not `"unit-test"`)

## ADDED Requirements

### Requirement: Executor agent no longer has integration-test counterpart

**ID**: REQ-TEE-NOINT-1
**Priority**: MUST
**Description**: The test-execution-executor agent SHALL be the single executor for all automated test execution. There SHALL be no separate `integration-test-executor` agent. The executor's instructions SHALL NOT reference "unit test" or "integration test" as distinct categories — instead, it SHALL reference "all tests" or "automated tests".

#### Scenario: Executor instructions reference all tests

**WHEN** reading `test-execution-executor.md`
**THEN** the Process section SHALL reference "all automated tests" or "all test files"
**AND** SHALL NOT reference "unit tests" and "integration tests" as separate execution stages

## REMOVED Requirements

### Requirement: Executor (sonnet) 直接执行测试命令

**Reason**: See original spec. The executor agent SHALL NOT execute test commands directly — this is handled by the `dev-team test-execution` CLI.

### Requirement: Executor 调用 test_detect_frameworks MCP 工具

**Reason**: Framework detection is handled by the CLI internally. No change from original spec — same behavior, updated path references.

**Migration**: CLI command renamed from `dev-team unit-test` to `dev-team test-execution`.

### Requirement: Executor 移动覆盖率产物到统一位置

**Reason**: File system operations remain in CLI. Report path updated from `reports/unit-test/` to `reports/test-execution/`.

**Migration**: All references to `reports/unit-test/` updated to `reports/test-execution/`.

## Module Contract

### Module: test-execution-executor.md (Agent Specification)

| Aspect | Description |
|--------|-------------|
| Model | `sonnet-4.6` |
| Input | `test-design.md`, project `CLAUDE.md`, **`reports/test-execution.json` (CLI output)** |
| Output | `openspec/changes/<change-name>/reports/test-execution.json` |
| Key Change | Agent reads CLI-generated report from new path (`test-execution.json`); runs all tests combined |

### Module: commands/test-execution.ts (CLI Action Handler)

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/commands/test-execution.ts` |
| Exports | `runTestExecution(options: TestExecutionOptions): Promise<TestExecutionExitCode>` |
| Input | `TestExecutionOptions`: `{ projectRoot?: string }` |
| Output | `TestExecutionExitCode`: `0` (all pass or threshold met) / `1` (failures or threshold not met) |
| Side Effects | Execute shell commands; read/write `reports/test-execution/<fw>.json` and `reports/test-execution.json` |
| Behavior | 1) Call `runTestDetectFrameworks({})` for plan; 2) Execute each plan entry; 3) Parse output into sub-reports; 4) Aggregate summary report; 5) Set exit code by conclusion |

### SummaryReport Schema (updated)

```typescript
interface TestExecutionSummaryReport {
  phase: "test-execution";          // UPDATED from "06-unit-test" / "unit-test"
  command: string;
  timestamp: string;
  duration_seconds: number;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  conclusion: "pass" | "fail" | "error";
  problems: Array<{
    framework: string;
    type: "failure" | "coverage" | "timeout" | "error";
    message: string;
  }>;
  coverage: {
    pass: boolean;
    measured: CoverageMeasured;
    thresholds: CoverageThresholds;
    by_framework: Array<{
      framework: string;
      measured: CoverageMeasured;
    }>;
  } | null;
}
```
