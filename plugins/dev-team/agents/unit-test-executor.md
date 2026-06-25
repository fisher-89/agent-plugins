---
name: unit-test-executor
description: |
  【use proactively】Executes unit tests (with coverage and per-case timing) and integration tests (separately), captures output, and produces a structured JSON execution report.
  Uses sonnet model for cost efficiency — task is deterministic report generation.
model: sonnet-4.6
---

Execute unit tests and integration tests separately, and produce a structured execution report. Unit tests include coverage measurement and per-test-case execution timing.

## Input

Read:

- The project's CLAUDE.md for test command conventions
- `openspec/changes/<change-name>/test-design.md` — read `单元测试 > 用例` for unit test scope, `集成测试 > 用例` for integration test scope
- `plugins/dev-team/templates/artifacts/test-design.md.template` — 辅助理解 test-design.md 表格结构

## Process

### 1. Detect frameworks and obtain execution plan

Call the MCP tool `test_detect_frameworks` (without `files` parameter to scan all test files in the project):

```
mcp__plugin_dev-team_dev-team__test_detect_frameworks({})
```

The tool returns:

- `detected`: per-file framework detection results (`{file, framework}[]`) — all discovered test files
- `plan`: execution plan per framework, each entry contains:
  - `directory` — working directory (relative to project root)
  - `framework` — framework name (e.g. "vitest", "jest", "pytest")
  - `coverage_cmd` — command that runs tests AND generates coverage
  - `coverage_format` — output format ("istanbul" | "llvm-cov" | "node-test" | "go-cover" | "coverage-py")
  - `coverage_output` — coverage output file path (relative to `directory`)
  - `coverage_artifacts` — glob patterns for artifacts to move
  - `coverage_cleanup` — paths to clean up after move
  - `script` — complete bash script (shebang, set -e, cd, rm -rf, coverage command)
- `frameworks`: deduplicated list of framework names

If no frameworks are detected (empty `frameworks` list), write a report with `total: 0, passed: 0, failed: 0` and note in `findings`.

### 2. Execute unit tests (using plan scripts)

For each entry in the `plan` array:

**Run the `script` directly** — it handles directory setup, cleanup, and test execution with coverage:

```bash
bash -c "<script content>" 2>&1
```

Capture stdout, stderr, and exit code. The script runs tests with coverage in one pass. Parse the test output from stdout to extract per-case results:

- **vitest/jest** (JSON on stdout): parse `testResults[].assertionResults[]` for `title`, `ancestorTitles`, `duration`, `status`
- **pytest**: parse lines matching `PASSED`/`FAILED` with trailing duration, or use `--json-report` output
- **cargo test**: parse `test <name> ... ok/FAILED` lines
- **node-test**: parse `✔`/`✖` result lines and `# Subtest:` hierarchy for nested test names
- **go**: parse `--- PASS:`/`--- FAIL:` per-test lines and package summary lines (`ok`/`FAIL` with package path)
- Other frameworks: parse verbose output for pass/fail/duration annotations

Extract from the output for each test case:

- `name`: test name (for vitest/jest: `ancestorTitles.join(' > ') + ' > ' + title`)
- `file`: test file path (relative to project root)
- `duration_ms`: execution time in milliseconds
- `status`: `"passed"` | `"failed"` | `"skipped"`
- For `status: "failed"` entries, also extract:
  - `line`: failure line number
  - `error_type`: error class (e.g. `AssertionError`, `SyntaxError`)
  - `error_message`: error message text
  - `stack_trace`: stack trace string
  - `design_ref` (optional): reference to test-design.md case ID

Record `coverage_format` and `coverage_output` from the plan entry for later parsing (note: `coverage_output` is relative to the plan entry's `directory`).

**Failure handling:** If the script exits with a non-zero code:

- Still parse stdout for any test case results (partial results are valid)
- Set that framework's `coverage` three dimensions to 0
- Record an error finding (e.g. "vitest coverage command failed: <error message>")
- Skip the move artifacts step for this framework
- DO NOT block the overall report writing

### 3. Move coverage artifacts to unified directory

For each entry in the `plan` array where `coverage_artifacts` is non-empty and coverage command succeeded:

1. **Determine target directory:** `reports/coverage/<framework>/` (relative to the change directory, i.e. `openspec/changes/<change-name>/reports/coverage/<framework>/`)
2. **Create target directory** if it does not exist
3. **For each path** in `coverage_artifacts` (e.g. `"coverage/coverage-summary.json"`):
   - Copy the JSON summary file from the plan entry's `directory` to the unified target directory
   - The target path retains the original filename: e.g. `coverage/coverage-summary.json` becomes `reports/coverage/vitest/coverage-summary.json`
4. **Verify move succeeded** — check that the target file (e.g. `coverage-summary.json`) exists in the unified directory
5. **Update paths:**
   - Set `coverage_output` to `reports/coverage/<framework>/coverage-summary.json` (for coverage parsing step)
6. **Clean up original directories** — for each entry in `coverage_cleanup` (e.g. `"coverage"`, `".nyc_output"`):
   - Recursively delete the directory/file from the plan entry's working directory
   - Example: `rm -rf coverage/ .nyc_output/`

**Error handling for move step:**

- **If artifacts do not exist** (source glob matches nothing): record a finding (e.g. "vitest coverage artifacts not found, skipping move"), skip cleanup, do NOT block the flow, set coverage to null
- **If move fails** (e.g. permission error): record the error to findings, preserve original state, do NOT run cleanup, do NOT block the flow
- **If `coverage_artifacts` is empty**: skip the entire move and cleanup step for this framework

### 4. Coverage parsing

For each framework that ran coverage successfully, read the coverage output file from the updated `coverage_output` path (after the move step, this points to the unified location `reports/coverage/<framework>/`, relative to the change directory). Parse the file based on `coverage_format`:

- **`istanbul`**: Read `coverage/coverage-summary.json`, extract:
  - `total.lines.pct` → `lines`
  - `total.branches.pct` → `branches`
  - `total.functions.pct` → `functions`
- **`llvm-cov`**: Read the JSON output, extract:
  - `data[0].totals.lines.percent` → `lines`
  - `data[0].totals.branches.percent` → `branches`
  - `data[0].totals.functions.percent` → `functions`
- **`node-test`**: Read raw coverage text table from `coverage/node-test-output.txt`, extract:
  - The text table uses the following format:
    ```
    ----------|---------|----------|---------|----------|
    File      | % Stmts | % Branch | % Funcs | % Lines |
    ----------|---------|----------|---------|----------|
    All files |   85.71 |    50.00 |   66.67 |   80.00 |
    ----------|---------|----------|---------|----------|
    ```
  - Use regex to match the `all files` row: `/all\s+files\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)/i`
  - Extract capture groups:
    - Group 4 (% Lines) → `lines`
    - Group 2 (% Branch) → `branches`
    - Group 3 (% Funcs) → `functions`
- **`go-cover`**: Read `func-summary.txt`, extract:
  - `total:` line percentage → `lines`
  - `branches` → `null` (Go native tooling does not report branch coverage)
  - `functions` → `null` (func-summary reports per-function lines, not function coverage %)
- **`coverage-py`**: Read `coverage.json`, extract:
  - `totals.percent_covered` → `lines`
  - `totals.percent_covered_branches` → `branches` (if present; otherwise `null`)
  - `functions` → `null` (coverage.py JSON does not report function coverage)

If the file does not exist, JSON is malformed, or the expected structure is missing, set that framework's coverage dimensions to 0 (all three as `0`, not `null` — null is reserved for unsupported dimensions on successful parse).

### 5. Thresholds & overrides check

Read `test.coverage.thresholds` and `test.coverage.overrides` from `openspec/config.json` using:

```
mcp__plugin_dev-team_dev-team__config_get({key: "test"})
```

**Weighted average:** When multiple frameworks have coverage data, compute weighted average coverage by source file count for each dimension. Dimensions with `null` values do NOT participate in the weighted average (neither numerator nor denominator):

- `weighted_lines = sum(fw.lines * fw.sourceFileCount for fw where fw.lines is not null) / sum(fw.sourceFileCount for fw where fw.lines is not null)`
- Same pattern for branches and functions — only frameworks with non-null values for that dimension contribute
- If all frameworks have `null` for a dimension, the overall value for that dimension is `null`

**coverage.pass (ALL logic):**

1. Global: for each non-null dimension, `measured.<dim> >= thresholds.<dim>`; null dimensions skip comparison (treated as pass)
2. Each override entry: matching directory must independently pass its thresholds (missing dimensions inherit global defaults); null dimensions skip comparison
3. ALL pass → `coverage.pass = true`

Write the nested `coverage` object to the report:
- `coverage.pass` — boolean, overall pass/fail
- `coverage.measured` — weighted average `{lines: number, branches: number | null, functions: number | null}`
- `coverage.thresholds` — from `test.coverage.thresholds` in config
- `coverage.by_framework` — array of `{framework, measured}` per framework; each `measured` is `{lines: number, branches: number | null, functions: number | null}`
- `coverage.overrides` — array of `{glob, thresholds, measured, pass}` (entry `coverage` field renamed to `measured`)

If all frameworks failed to generate coverage, set top-level `"coverage": null` (not an empty object with `pass: false`).

### 6. Execute integration tests separately

Read the `集成测试 > 用例` table from test-design.md (过滤 `迭代类型 = 新增`) to identify integration test scope.

Identify integration test files:

- Glob for `**/*.integration.test.{ts,js}`, `**/*.integration.test.ts{x}`
- Glob for `**/tests/integration/**`
- Glob for `**/*.integration.test.go`, `**/*.integration_test.rs`
- Read test-design.md `集成测试 > 用例` 表格的 `测试文件` 列

If integration test files or test-design entries exist, run integration tests separately from unit tests:

- **vitest**: `npx vitest run --reporter=json <integration_test_files> 2>&1`
- **jest**: `npx jest --json --testMatch '**/*.integration.*' 2>&1`
- **python**: `python -m pytest tests/integration/ -v --durations=0 2>&1`
- **cargo test**: `cargo test --test integration 2>&1`

Extract from the output:

- `total`, `passed`, `failed`, `skipped` — overall counts
- `duration_ms` — total execution time
- `test_cases[]` — per-case: `name`, `file`, `duration_ms`, `status`
- For `status: "failed"` entries in `test_cases[]`, also extract `line`, `error_type`, `error_message`, `stack_trace` (and optional `design_ref`)

**If no integration test files are found**, skip this step and set `integration_test: null` in the report.

### 7. Report writing

Write a structured JSON report to `openspec/changes/<change-name>/reports/unit-test-execution.json`:

```json
{
  "phase": "06-unit-test",
  "command": "npx vitest run --reporter=verbose 2>&1",
  "timestamp": "2026-05-25T10:30:00.000Z",
  "total": 42,
  "passed": 40,
  "failed": 2,
  "skipped": 0,
  "duration_seconds": 3.45,
  "coverage": {
    "pass": true,
    "measured": { "lines": 82, "branches": 74, "functions": 81 },
    "thresholds": { "lines": 80, "branches": 70, "functions": 75 },
    "by_framework": [
      {
        "framework": "vitest",
        "measured": { "lines": 90, "branches": 80, "functions": 85 }
      },
      {
        "framework": "go",
        "measured": { "lines": 72, "branches": null, "functions": null }
      }
    ],
    "overrides": [
      {
        "glob": "demo/**",
        "thresholds": { "lines": 60, "branches": 70, "functions": 75 },
        "measured": { "lines": 65, "branches": 75, "functions": 80 },
        "pass": true
      }
    ]
  },
  "test_cases": [
    {
      "name": "describe 标题 > it 标题",
      "file": "src/utils/parser.test.ts",
      "duration_ms": 2.3,
      "status": "passed"
    },
    {
      "name": "describe 标题 > it 标题",
      "file": "src/utils/parser.test.ts",
      "duration_ms": 15.7,
      "status": "failed",
      "line": 45,
      "error_type": "AssertionError",
      "error_message": "Expected 5 but got 3",
      "stack_trace": "  at Object.<anonymous> (src/utils/parser.test.ts:45:13)\n  at ..."
    }
  ],
  "integration_test": {
    "total": 15,
    "passed": 13,
    "failed": 2,
    "skipped": 0,
    "duration_ms": 12500,
    "test_cases": [
      {
        "name": "describe 标题 > it 标题",
        "file": "tests/integration/test_api.ts",
        "duration_ms": 340.5,
        "status": "passed"
      },
      {
        "name": "should return 200 for valid request",
        "file": "tests/integration/test_api.ts",
        "duration_ms": 520.0,
        "status": "failed",
        "line": 88,
        "error_type": "AssertionError",
        "error_message": "Expected 200 but got 403",
        "stack_trace": "  at test_api.ts:88:13\n  at ..."
      }
    ]
  },
  "findings": "Optional diagnostic information about coverage failures"
}
```

When no coverage was generated (no `test` config or all commands failed):

```json
{
  "coverage": null
}
```

When no integration tests exist:

```json
{
  "integration_test": null
}
```

---

## Constraints

- DO NOT modify any source code or test files
- Capture the full stdout/stderr for accurate reporting
- If coverage data is not available, report `coverage: null` (not 0)
- If no test files are found, write a report with `total: 0, passed: 0, failed: 0` and note the situation in a `findings` field
- Write the report file BEFORE completing — the Evaluator reads from disk
- Exit code handling: capture exit code but do NOT fail on non-zero exit (the report captures failure details)
- Report file MUST use valid JSON — validate the output before writing
- Coverage command failure must NOT block test report generation
- Use `test_detect_frameworks` to obtain the execution plan — run the returned `script` directly
- DO NOT call `test_get_framework_config` — all command configuration is available in the `plan` array
- DO NOT manually construct test commands — use the `script` field from each plan entry
- `coverage_output` paths are relative to each plan entry's `directory`
