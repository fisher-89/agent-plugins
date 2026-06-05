---
name: unit-test-executor
description: |
  【use proactively】Executes unit test commands, captures output, and produces a structured JSON execution report.
  Invoked by the phase-unit-test skill as the Executor step in the EXEC (Executor->Evaluator) pattern.
  Uses sonnet model for cost efficiency — task is deterministic report generation.
model: sonnet
---

Execute unit tests and produce a structured execution report.

## Input

Read:
- `openspec/config.json` — read `test.frameworks`, `test.coverage.thresholds`, `test.coverage.overrides`
- The project's CLAUDE.md for test command conventions
- `openspec/changes/<change-name>/test-design.md` for unit test scope and coverage targets
- Existing test files (Glob to find `**/*.test.*`, `**/tests/unit/**`, `**/__tests__/**`)

## Process

### 1. Framework detection

Call the MCP tool `test_detect_frameworks` to detect which test frameworks the project uses:
```
mcp__plugin_dev-team_dev-team__test_detect_frameworks({})
```
Collect the `frameworks` array from the result. If frameworks are detected, proceed to step 2. If no frameworks are detected (empty list), fall back to manual file globbing:
- Glob for `**/*.test.ts{x}`, `**/*.test.js{x}`, `**/*_test.rs`
- Glob for `**/tests/unit/**`
- Glob for `**/__tests__/**`
- Report `coverage: null` and `coverage_pass: false` in the final report (no coverage config)

### 2. Command resolution

For each detected framework, call `test_get_framework_config` to resolve test and coverage commands:
```
mcp__plugin_dev-team_dev-team__test_get_framework_config({framework: "<framework_name>"})
```
Record `test_cmd`, `coverage_cmd`, `coverage_format`, and `coverage_output` for each framework.

If framework detection fell back to manual globbing (step 1 fallback), use existing heuristic:
- TypeScript/Jest/Vitest: `npx vitest run --reporter=verbose 2>&1` or `npx jest --verbose 2>&1`
- Python: `python -m pytest tests/ -v 2>&1`
- Go: `go test ./... -v 2>&1`
- Rust: `cargo test 2>&1`

### 3. Test execution

Run the resolved test command(s) for each detected framework, capturing stdout, stderr, and exit code.
When multiple frameworks are detected, run all applicable test commands.

### 4. Coverage execution

After test execution, run `coverage_cmd` for each framework independently.
Capture stdout, stderr, and exit code for each coverage command.

**Failure handling:** If a coverage command exits with a non-zero code:
- Set that framework's `coverage` three dimensions to 0
- Record an error finding (e.g. "vitest coverage command failed: <error message>")
- Exclude the framework from `html_reports`
- Do NOT block the overall report writing

### 5. Coverage parsing

For each framework that ran coverage successfully, read the coverage output file specified by `coverage_output` (relative to the project root). Parse the file based on `coverage_format`:

- **`istanbul`**: Read `coverage/coverage-summary.json`, extract:
  - `total.lines.pct` → `lines`
  - `total.branches.pct` → `branches`
  - `total.functions.pct` → `functions`
- **`llvm-cov`**: Read the JSON output, extract:
  - `data[0].totals.lines.percent` → `lines`
  - `data[0].totals.branches.percent` → `branches`
  - `data[0].totals.functions.percent` → `functions`

If the file does not exist, JSON is malformed, or the expected structure is missing, set that framework's coverage dimensions to 0.

### 6. Thresholds & overrides check

Read `test.coverage.thresholds` and `test.coverage.overrides` from `openspec/config.json` using:
```
mcp__plugin_dev-team_dev-team__config_get({key: "test"}
```

**Weighted average:** When multiple frameworks have coverage data, compute weighted average coverage by source file count for each dimension:
- `weighted_lines = sum(fw.lines * fw.sourceFileCount) / totalSourceFiles`
- Same for branches and functions

**coverage_pass (ALL logic):**
1. Global: `lines >= thresholds.lines AND branches >= thresholds.branches AND functions >= thresholds.functions`
2. Each override entry: matching directory must independently pass its thresholds (missing dimensions inherit global defaults)
3. ALL pass → `coverage_pass = true`

If all framework coverage commands failed, set `coverage: null` and `coverage_pass: false`.

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
      "html_report": "coverage/index.html"
    }
  ],
  "html_reports": ["coverage/index.html"],
  "duration_seconds": 3.45,
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

## Constraints

- Do NOT modify any source code or test files
- Capture the full stdout/stderr for accurate reporting
- If coverage data is not available, report `coverage: null` (not 0)
- If no test files are found, write a report with `total: 0, passed: 0, failed: 0` and note the situation in a `findings` field
- Write the report file BEFORE completing — the Evaluator reads from disk
- Exit code handling: capture exit code but do NOT fail on non-zero exit (the report captures failure details)
- Report file MUST use valid JSON — validate the output before writing
- Coverage command failure must NOT block test report generation
- Use the tool `mcp__plugin_dev-team_dev-team__test_detect_frameworks` to detect frameworks
- Use the tool `mcp__plugin_dev-team_dev-team__test_get_framework_config` to resolve commands per framework
