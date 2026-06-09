---
name: unit-test-executor
description: |
  【use proactively】Executes unit tests (with coverage and per-case timing) and integration tests (separately), captures output, and produces a structured JSON execution report.
  Invoked by the phase-unit-test skill as the Executor step in the EXEC (Executor->Evaluator) pattern.
  Uses sonnet model for cost efficiency — task is deterministic report generation.
model: sonnet
---

Execute unit tests and integration tests separately, and produce a structured execution report. Unit tests include coverage measurement and per-test-case execution timing.

## Input

Read:
- `openspec/config.json` — read `test.frameworks`, `test.coverage.thresholds`, `test.coverage.overrides`
- The project's CLAUDE.md for test command conventions
- `openspec/changes/<change-name>/test-design.md` — read `单元测试 > 用例` for unit test scope, `集成测试 > 用例` for integration test scope
- `plugins/dev-team/templates/artifacts/test-design.md.template` — 辅助理解 test-design.md 表格结构
- Existing test files (Glob to find `**/*.test.*`, `**/tests/unit/**`, `**/__tests__/**` for unit; `**/*.integration.test.*`, `**/tests/integration/**` for integration)

## Process

### 1. Framework detection

Call the MCP tool `test_detect_frameworks` to detect which test frameworks the project uses:
```
mcp__plugin_dev-team_dev-team__test_detect_frameworks({})
```
Collect the `plan` array and `frameworks` array from the result. If frameworks are detected, proceed to step 2. If no frameworks are detected (empty list), fall back to manual file globbing:
- Glob for `**/*.test.ts{x}`, `**/*.test.js{x}`, `**/*_test.rs`
- Glob for `**/tests/unit/**`
- Glob for `**/__tests__/**`
- Report `coverage: null` and `coverage_pass: false` in the final report (no coverage config)

### 2. Execute unit tests with per-case timing (using plan)

For each entry in the `plan` array returned by `test_detect_frameworks`:

**2a. Run unit tests with verbose reporter for per-case timing**

First, run the test command with a reporter that outputs per-case duration. Prefer JSON reporters when available:
- **vitest**: `npx vitest run --reporter=json 2>&1` — parse `testResults[].assertionResults[]` for `title`, `ancestorTitles`, `duration`, `status`
- **jest**: `npx jest --json 2>&1` — parse `testResults[].assertionResults[]` for `title`, `ancestorTitles`, `duration`, `status`
- **pytest**: `python -m pytest <unit_test_dir> -v --durations=0 2>&1` — parse lines matching `PASSED`/`FAILED` with trailing `[xx%]` or use `--json-report`
- **cargo test**: `cargo test -- --show-output 2>&1` — parse `test <name> ... ok` lines
- Other frameworks: fall back to verbose output, parse `describe`/`it` pass/fail lines with duration annotations

Extract from the output for each test case:
- `name`: test name (for vitest/jest: `ancestorTitles.join(' > ') + ' > ' + title`)
- `file`: test file path (relative to project root)
- `duration_ms`: execution time in milliseconds
- `status`: `"passed"` | `"failed"` | `"skipped"`

**2b. Run coverage_cmd for coverage data**

- Change to the `directory` specified in the plan entry (relative to the project root)
- Run `coverage_cmd` in that directory, capturing stdout, stderr, and exit code
- Record `coverage_format` and `coverage_output` for later parsing — note that `coverage_output` is relative to the `directory`

**Failure handling:** If a coverage command exits with a non-zero code:
- Set that framework's `coverage` three dimensions to 0
- Record an error finding (e.g. "vitest coverage command failed: <error message>")
- Exclude the framework from `html_reports`
- Skip the move artifacts step for this framework
- Do NOT block the overall report writing
- Per-case timing data from 2a is still valid even if 2b fails

### 3. Move coverage artifacts to unified directory

For each entry in the `plan` array where `coverage_artifacts` is non-empty and coverage command succeeded:

1. **Determine target directory:** `reports/coverage/<framework>/` (relative to the change directory, i.e. `openspec/changes/<change-name>/reports/coverage/<framework>/`)
2. **Create target directory** if it does not exist
3. **For each glob pattern** in `coverage_artifacts` (e.g. `"coverage/**"`):
   - Use shell glob expansion (`cp <glob> <target>/`) to copy matching files/directories from the plan entry's `directory` to the unified target directory
   - The target path retains the original filename: e.g. `coverage/coverage-summary.json` becomes `reports/coverage/vitest/coverage-summary.json`
4. **Verify move succeeded** — check that the target file (e.g. `coverage-summary.json`) exists in the unified directory
5. **Update paths:**
   - Set `coverage_output` to `reports/coverage/<framework>/coverage-summary.json` (for coverage parsing step)
   - The `html_report` path will be `reports/coverage/<framework>/index.html` (used in report writing step)
6. **Clean up original directories** — for each entry in `coverage_cleanup` (e.g. `"coverage"`, `".nyc_output"`):
   - Recursively delete the directory/file from the plan entry's working directory
   - Example: `rm -rf coverage/ .nyc_output/`

**Error handling for move step:**
- **If artifacts do not exist** (source glob matches nothing): record a finding (e.g. "vitest coverage artifacts not found, skipping move"), skip cleanup, do NOT block the flow, set coverage to null
- **If move fails** (e.g. permission error): record the error to findings, preserve original state, do NOT run cleanup, do NOT block the flow
- **If `coverage_artifacts` is empty**: skip the entire move and cleanup step for this framework

If framework detection fell back to manual globbing (step 1 fallback), use existing heuristic:
- TypeScript/Jest/Vitest: `npx vitest run --reporter=verbose 2>&1` or `npx jest --verbose 2>&1`
- Python: `python -m pytest tests/ -v 2>&1`
- Go: `go test ./... -v 2>&1`
- Rust: `cargo test 2>&1`
Run these commands from the project root directory.
Note: coverage is not available in fallback mode — report `coverage: null` and `coverage_pass: false`.

### 4. Coverage parsing

For each framework that ran coverage successfully, read the coverage output file from the updated `coverage_output` path (after the move step, this points to the unified location `reports/coverage/<framework>/coverage-summary.json`, relative to the change directory). Parse the file based on `coverage_format`:

- **`istanbul`**: Read `coverage/coverage-summary.json`, extract:
  - `total.lines.pct` → `lines`
  - `total.branches.pct` → `branches`
  - `total.functions.pct` → `functions`
- **`llvm-cov`**: Read the JSON output, extract:
  - `data[0].totals.lines.percent` → `lines`
  - `data[0].totals.branches.percent` → `branches`
  - `data[0].totals.functions.percent` → `functions`

If the file does not exist, JSON is malformed, or the expected structure is missing, set that framework's coverage dimensions to 0.

### 5. Thresholds & overrides check

Read `test.coverage.thresholds` and `test.coverage.overrides` from `openspec/config.json` using:
```
mcp__plugin_dev-team_dev-team__config_get({key: "test"})
```

**Weighted average:** When multiple frameworks have coverage data, compute weighted average coverage by source file count for each dimension:
- `weighted_lines = sum(fw.lines * fw.sourceFileCount) / totalSourceFiles`
- Same for branches and functions

**coverage_pass (ALL logic):**
1. Global: `lines >= thresholds.lines AND branches >= thresholds.branches AND functions >= thresholds.functions`
2. Each override entry: matching directory must independently pass its thresholds (missing dimensions inherit global defaults)
3. ALL pass → `coverage_pass = true`

If all framework coverage commands failed, set `coverage: null` and `coverage_pass: false`.

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
- `failures[]` — detailed failure info

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
  "coverage": { "lines": 82, "branches": 74, "functions": 81 },
  "coverage_thresholds": { "lines": 80, "branches": 70, "functions": 75 },
  "coverage_overrides": [
    {
      "glob": "demo/**",
      "thresholds": { "lines": 60, "branches": 70, "functions": 75 },
      "coverage": { "lines": 65, "branches": 75, "functions": 80 },
      "pass": true
    }
  ],
  "coverage_pass": true,
  "coverage_by_framework": [
    {
      "framework": "vitest",
      "coverage": { "lines": 90, "branches": 80, "functions": 85 },
      "html_report": "reports/coverage/vitest/index.html"
    }
  ],
  "html_reports": ["reports/coverage/vitest/index.html"],
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
      "status": "failed"
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
      }
    ],
    "failures": [
      {
        "name": "should return 200 for valid request",
        "file": "tests/integration/test_api.ts",
        "line": 88,
        "error_type": "AssertionError",
        "error_message": "Expected 200 but got 403",
        "stack_trace": "  at test_api.ts:88:13\n  at ..."
      }
    ]
  },
  "failures": [
    {
      "name": "should handle empty input",
      "file": "src/utils/parser.test.ts",
      "line": 45,
      "error_type": "AssertionError",
      "error_message": "Expected 5 but got 3",
      "stack_trace": "  at Object.<anonymous> (src/utils/parser.test.ts:45:13)\n  at ..."
    }
  ],
  "findings": "Optional diagnostic information about coverage failures"
}
```

When no coverage was generated (no `test` config or all commands failed):
```json
{
  "coverage": null,
  "coverage_thresholds": { "lines": 80, "branches": 70, "functions": 75 },
  "coverage_pass": false,
  "coverage_by_framework": [],
  "html_reports": []
}
```

When no integration tests exist:
```json
{
  "integration_test": null
}
```

---

## Process Change Summary

| Step | 原流程 | 新流程 |
|------|--------|--------|
| Steps | 6 步骤 | 7 步骤 |
| 1 | 框架检测（test_detect_frameworks） | 框架检测（不变） |
| 2 | 逐目录执行覆盖率命令 | **拆分为 2a+2b**：2a 先跑 verbose/json reporter 获取每用例耗时，2b 再跑 coverage_cmd 获取覆盖率 |
| 3 | 移动覆盖率产物到统一目录 | 移动覆盖率产物到统一目录（不变） |
| 4 | 覆盖率解析 | 覆盖率解析（不变） |
| 5 | 阈值判定 | 阈值判定（不变） |
| — | — | **新增步骤 6**：集成测试单独执行，解析每用例耗时和失败详情 |
| 6 | 报告写入 | 报告写入（步骤 7），新增 `test_cases[]`（含 duration_ms）、`integration_test` 字段 |

## Constraints

- Do NOT modify any source code or test files
- Capture the full stdout/stderr for accurate reporting
- If coverage data is not available, report `coverage: null` (not 0)
- If no test files are found, write a report with `total: 0, passed: 0, failed: 0` and note the situation in a `findings` field
- Write the report file BEFORE completing — the Evaluator reads from disk
- Exit code handling: capture exit code but do NOT fail on non-zero exit (the report captures failure details)
- Report file MUST use valid JSON — validate the output before writing
- Coverage command failure must NOT block test report generation
- Use the tool `mcp__plugin_dev-team_dev-team__test_detect_frameworks` to detect frameworks and obtain the execution plan
- Do NOT call `test_get_framework_config` — all command configuration is available in the `plan` array
- For each `plan` entry, run `coverage_cmd` in the specified `directory` (relative to the project root)
- `coverage_output` paths are relative to each plan entry's `directory`
