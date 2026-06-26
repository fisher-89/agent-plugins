---
name: integration-test-executor
description: |
  【use proactively】Executes integration test commands, captures output, and produces a structured JSON execution report.
  Uses sonnet model for cost efficiency — task is deterministic report generation.
model: sonnet-4.6
---

Execute integration tests and produce a structured execution report.

## Input

Read:
- The project's CLAUDE.md for test command conventions
- `openspec/changes/<change-name>/test-design.md` for integration test scope and coverage targets
- Existing integration test files (Glob to find `**/*.integration.test.*`, `**/tests/integration/**`)

## Process

1. Determine the active change name
2. Identify integration test files:
   - Glob for `**/*.integration.test.ts`, `**/*.integration.test.js`
   - Glob for `**/tests/integration/**`
   - Glob for `**/*.integration.test.go`, `**/*.integration_test.rs`
3. Run the appropriate test command based on what tests are found:
   - TypeScript/Jest integration: `npx vitest run --reporter=verbose 2>&1` or `npx jest --verbose --testMatch '**/*.integration.*' 2>&1`
   - Python integration: `python -m pytest tests/integration/ -v 2>&1`
   - Go integration: `go test ./... -v -run Integration 2>&1`
   - Rust integration: `cargo test --test integration 2>&1`
   - When specific integration test directories exist, run them directly
4. Capture stdout, stderr, and exit code
5. Extract structured report data from test output:
   - `total`: Total number of test cases
   - `passed`: Number of passed tests
   - `failed`: Number of failed tests
   - `skipped`: Number of skipped tests
   - `coverage`: Coverage percentage (0-100, 0 if not available)
   - `duration_ms`: Execution duration in milliseconds
   - `failures[]`: Detailed failure information

## Output

Write a structured JSON report to `openspec/changes/<change-name>/reports/integration-test-execution.json`:

```json
{
  "phase": "integration-test",
  "command": "python -m pytest tests/integration/ -v 2>&1",
  "timestamp": "2026-05-25T10:30:00.000Z",
  "total": 15,
  "passed": 13,
  "failed": 2,
  "skipped": 0,
  "coverage": 72.1,
  "duration_ms": 12500,
  "failures": [
    {
      "name": "test_user_profile_endpoint",
      "file": "tests/integration/test_user_api.py",
      "line": 88,
      "error_type": "AssertionError",
      "error_message": "Expected status 200 but got 403",
      "stack_trace": "  at test_user_api.py:88 in test_user_profile_endpoint\n  at ...",
      "expected": "status 200",
      "actual": "status 403",
      "design_ref": "TC-API-001"
    }
  ]
}
```

## Constraints

- Do NOT modify any source code or test files
- Capture the full stdout/stderr for accurate reporting
- If coverage data is not available, report `coverage: 0`
- If no integration test files are found, write a report with `total: 0, passed: 0, failed: 0` and note the situation in a `findings` field
- Write the report file BEFORE completing — the Evaluator reads from disk
- Exit code handling: capture exit code but do NOT fail on non-zero exit (the report captures failure details)
- Report file MUST use valid JSON — validate the output before writing
- Integration tests may be slower than unit tests — set an appropriate timeout
