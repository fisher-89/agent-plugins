## MODIFIED Requirements

### Requirement: Evaluator agents call phase_log with prefix-free phase ID
All evaluator agent `.md` files SHALL reference their phase ID without the numeric prefix when calling the `mcp__plugin_dev-team_dev-team__phase_log` MCP tool. The following evaluator agent files are affected:

| Agent File | Old Phase ID | New Phase ID |
|-----------|-------------|--------------|
| `proposal-evaluator.md` | `01-proposal` | `proposal` |
| `dev-design-evaluator.md` | `02-dev-design` | `dev-design` |
| `test-design-evaluator.md` | `03-test-design` | `test-design` |
| `test-gen-evaluator.md` | `04-test-gen` | `test-gen` |
| `implementation-evaluator.md` | `05-implement` | `implement` |
| `code-analyze-evaluator.md` | `02-code-analyze` | `code-analyze` |
| `code-review-evaluator.md` | `07-code-review` | `code-review` |
| `acceptance-evaluator.md` | `09-acceptance` | `acceptance` |
| `unit-test-evaluator.md` | `06-unit-test` | `unit-test` |
| `integration-test-evaluator.md` | `08-integration-test` | `integration-test` |

The `phase_log` call within each evaluator agent SHALL use the prefix-free phase ID as the `phase` parameter value.

#### Scenario: proposal-evaluator calls phase_log with prefix-free ID
- **WHEN** reading `proposal-evaluator.md`
- **THEN** the `phase_log` call uses `phase: "proposal"` (not `"01-proposal"`)

#### Scenario: acceptance-evaluator calls phase_log with prefix-free ID
- **WHEN** reading `acceptance-evaluator.md`
- **THEN** the `phase_log` call uses `phase: "acceptance"` (not `"09-acceptance"`)

### Requirement: Executor agents call phase_log with prefix-free phase ID
Executor agent `.md` files SHALL use prefix-free phase IDs when calling `mcp__plugin_dev-team_dev-team__phase_log` for no-op skip entries:

| Agent File | Old Phase ID | New Phase ID |
|-----------|-------------|--------------|
| `unit-test-executor.md` | `06-unit-test` | `unit-test` |
| `integration-test-executor.md` | `08-integration-test` | `integration-test` |

#### Scenario: unit-test-executor uses prefix-free ID for skip
- **WHEN** reading `unit-test-executor.md`
- **THEN** the `phase_log` call for no-op skip uses `phase: "unit-test"` (not `"06-unit-test"`)

#### Scenario: integration-test-executor uses prefix-free ID for skip
- **WHEN** reading `integration-test-executor.md`
- **THEN** the `phase_log` call for no-op skip uses `phase: "integration-test"` (not `"08-integration-test"`)

### Requirement: Test-execution evaluator backtrack targets use prefix-free IDs
`unit-test-evaluator.md` and `integration-test-evaluator.md` SHALL update all backtrack target references to use prefix-free phase IDs:

| Agent File | Old Backtrack Targets | New Backtrack Targets |
|-----------|----------------------|----------------------|
| `unit-test-evaluator.md` | `04-test-gen`, `05-implement`, `03-test-design`, `02-dev-design` | `test-gen`, `implement`, `test-design`, `dev-design` |
| `integration-test-evaluator.md` | `04-test-gen`, `05-implement`, `03-test-design`, `02-dev-design`, `08-integration-test` | `test-gen`, `implement`, `test-design`, `dev-design`, `integration-test` |

#### Scenario: unit-test-evaluator backtrack targets use prefix-free IDs (AC-7)
- **WHEN** reading `unit-test-evaluator.md`
- **THEN** backtrack targets reference `test-gen`, `implement`, `test-design`, `dev-design` (without numeric prefix)

#### Scenario: integration-test-evaluator backtrack targets use prefix-free IDs
- **WHEN** reading `integration-test-evaluator.md`
- **THEN** backtrack targets reference `test-gen`, `implement`, `test-design`, `dev-design` (without numeric prefix)

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent File | Change | Details |
|-----------|--------|---------|
| `proposal-evaluator.md` | MODIFIED | `phase_log` phase: `"01-proposal"` → `"proposal"` |
| `dev-design-evaluator.md` | MODIFIED | `phase_log` phase: `"02-dev-design"` → `"dev-design"` |
| `test-design-evaluator.md` | MODIFIED | `phase_log` phase: `"03-test-design"` → `"test-design"` |
| `test-gen-evaluator.md` | MODIFIED | `phase_log` phase: `"04-test-gen"` → `"test-gen"` |
| `implementation-evaluator.md` | MODIFIED | `phase_log` phase: `"05-implement"` → `"implement"` |
| `code-analyze-evaluator.md` | MODIFIED | `phase_log` phase: `"02-code-analyze"` → `"code-analyze"` |
| `code-review-evaluator.md` | MODIFIED | `phase_log` phase: `"07-code-review"` → `"code-review"` |
| `acceptance-evaluator.md` | MODIFIED | `phase_log` phase: `"09-acceptance"` → `"acceptance"` |
| `unit-test-evaluator.md` | MODIFIED | `phase_log` phase: `"06-unit-test"` → `"unit-test"`; backtrack targets prefix-free |
| `integration-test-evaluator.md` | MODIFIED | `phase_log` phase: `"08-integration-test"` → `"integration-test"`; backtrack targets prefix-free |
| `unit-test-executor.md` | MODIFIED | `phase_log` phase: `"06-unit-test"` → `"unit-test"` |
| `integration-test-executor.md` | MODIFIED | `phase_log` phase: `"08-integration-test"` → `"integration-test"` |

### Requirement: unit-test-executor agent calls test_detect_frameworks and test_get_framework_config MCP tools

`plugins/dev-team/agents/unit-test-executor.md` SHALL be updated to call the `test_detect_frameworks` MCP tool at the start of its Process section to determine which test frameworks are present in the project.
After determining the framework(s), the Executor SHALL use the `plan` array returned by `test_detect_frameworks` (including `script`, `coverage_format`, `coverage_artifacts`, `coverage_cleanup`) to run coverage commands — it SHALL NOT call `test_get_framework_config` separately.

The Process section SHALL be updated as follows:

1. **Framework detection** (before test execution): Call `test_detect_frameworks` to get the list of detected frameworks and execution plan. If no frameworks are detected, fall back to manual file globbing (existing behavior).
2. **Coverage execution**: For each plan entry, run the returned `script` in the corresponding `directory`. Parse test output for all supported frameworks including:
   - **node-test**: parse `✔`/`✖` test lines and `# Subtest:` annotations from `node --test` verbose output
   - **go**: parse `--- PASS:`/`--- FAIL:` lines and `ok`/`FAIL` package summary lines
   - **pytest**: parse `PASSED`/`FAILED` lines with trailing duration (existing rule)
3. **Artifact move**: Move JSON/text coverage summary files to `reports/coverage/<framework>/` per plan configuration (SHALL NOT move or record HTML reports).
4. **Coverage parsing**: Parse coverage output using `coverage_format`, extracting lines / branches / functions into per-framework `measured` values. Supported formats:
   - `istanbul` / `node-test`: read `total.lines.pct`, `total.branches.pct`, `total.functions.pct` from `coverage-summary.json`
   - `llvm-cov`: read `data[0].totals.*.percent` from JSON
   - `go-cover`: read `total:` line from `func-summary.txt` for lines; set `branches: null`, `functions: null`
   - `coverage-py`: read `totals.percent_covered` for lines, `totals.percent_covered_branches` for branches; set `functions: null`
5. **Thresholds & overrides check**: Read `test.coverage.thresholds` and `test.coverage.overrides` from config.json; compute weighted average into `coverage.measured` (null dimensions excluded from weighting); populate `coverage.overrides`; set `coverage.pass` via ALL logic where null dimensions skip comparison.
6. **Test case extraction**: Extract per-case results into `test_cases[]`; for failed cases, include `line`, `error_type`, `error_message`, `stack_trace` directly on the test case entry.
7. **Report writing**: Write structured report with nested `coverage` object (or `coverage: null`), `test_cases[]`, and optional `integration_test` sub-report. `coverage.measured` and `by_framework[].measured` SHALL use `{lines: number, branches: number | null, functions: number | null}`.

The agent documentation SHALL document `coverage_format` as `"istanbul" | "llvm-cov" | "node-test" | "go-cover" | "coverage-py"`.

The agent tool list SHALL include `test_detect_frameworks` MCP tool permission.

#### Scenario: Executor parses go-cover format with null dimensions

- **WHEN** unit-test-executor completes go framework coverage parsing
- **THEN** it writes `by_framework` entry with `measured: {lines: <number>, branches: null, functions: null}`
- **AND** `coverage.pass` ignores null dimensions in threshold comparison

#### Scenario: Executor parses coverage-py format with null functions

- **WHEN** unit-test-executor completes pytest framework coverage parsing
- **THEN** it writes `by_framework` entry with `measured: {lines: <number>, branches: <number>, functions: null}`

#### Scenario: Executor parses node-test format via istanbul-compatible JSON

- **WHEN** unit-test-executor completes node-test framework coverage (after parser script)
- **THEN** it reads `coverage-summary.json` using istanbul field paths
- **AND** writes `measured: {lines: <number>, branches: <number>, functions: <number>}`

#### Scenario: Executor writes nested coverage object in report

- **WHEN** unit-test-executor completes coverage execution and threshold checks
- **THEN** it writes `coverage: {pass, measured, thresholds, by_framework, overrides}` to `unit-test-execution.json`
- **AND** does NOT write separate top-level `coverage_pass`, `coverage_thresholds`, `coverage_by_framework`, `coverage_overrides`, or `html_reports` fields

#### Scenario: Executor writes coverage null when no coverage configured

- **WHEN** `test_detect_frameworks` returns an empty plan (no `test.frameworks` configured in config.json)
- **THEN** the Executor SHALL fall back to manual file globbing for test files
- **AND** report `coverage: null` in the output report

### Requirement: unit-test-evaluator agent reads expanded coverage fields from report

`plugins/dev-team/agents/unit-test-evaluator.md` SHALL be updated to read the simplified nested coverage object and merged test_cases from the execution report.
The evaluator SHALL:
- Read `coverage.pass` from the report for direct gate pass/fail decision (when `coverage` is not null)
- Read `coverage.thresholds` and `coverage.overrides` from the report (already populated by executor from config.json)
- Read `coverage.measured` for per-dimension evidence in the findings; when a dimension is `null`, findings SHALL annotate `"N/A (框架不支持)"` for that dimension
- Read `coverage.by_framework` for detailed per-framework evidence; null dimensions in per-framework `measured` SHALL be annotated `"N/A (框架不支持)"` in findings
- Read failed test details from `test_cases[]` entries where `status === "failed"` for the diagnostic decision tree

The evaluator SHALL NOT re-calculate coverage or re-run coverage commands. All coverage calculations are performed by the executor.
The evaluator SHALL NOT reference HTML coverage report paths in findings or checklist evidence.

The Static Checklist U1 SHALL require: `phase`, `command`, `timestamp`, `total`, `passed`, `failed`, `skipped`, `coverage` (object or null), `duration_seconds`, `test_cases` (array).

The Static Checklist U3 SHALL pass when `coverage === null` OR `coverage.pass === true`.

When validating `coverage` object completeness, `measured` fields `branches` and `functions` SHALL accept `null` as valid values (indicating framework does not support that dimension).

Step 5 findings template SHALL:
- Reference `coverage.measured` per-dimension values, using `"N/A (框架不支持)"` for null dimensions
- Reference `coverage.by_framework[].measured` with null dimension annotations
- NOT fail U3 when null dimensions exist but `coverage.pass` is true

#### Scenario: Evaluator annotates null dimensions in findings

- **WHEN** unit-test-evaluator reads a report with go framework `measured: {lines: 85, branches: null, functions: null}` and `coverage.pass: true`
- **THEN** U3 checklist item passes
- **AND** findings include lines=85% and branches/functions annotated as "N/A (框架不支持)"

#### Scenario: Evaluator validates nested coverage object with nullable dimensions

- **WHEN** unit-test-evaluator reads a report containing `coverage: {pass: true, measured: {lines: 85, branches: null, functions: null}, ...}`
- **THEN** it SHALL use `coverage.pass` to determine the coverage checklist item result
- **AND** SHALL NOT fail U1 for null `branches` or `functions` values

#### Scenario: Evaluator handles report with coverage null

- **WHEN** the execution report contains `"coverage": null`
- **THEN** the evaluator SHALL pass the coverage check with evidence "覆盖率检查未配置或生成失败，跳过"

#### Scenario: Evaluator decision tree reads from failed test_cases

- **WHEN** the report has `failed > 0` and failed entries in `test_cases[]`
- **THEN** the evaluator applies the diagnostic decision tree using fields from those test_cases entries

## Module Contract

### Agent: unit-test-executor.md

| Step | Change |
|------|--------|
| Step 2 (test output parsing) | 新增 node-test、go 框架输出解析规则 |
| Step 4 (coverage parsing) | 新增 `node-test`、`go-cover`、`coverage-py` 三种格式解析路径 |
| Step 5 (thresholds) | null 维度跳过加权平均与阈值比较 |
| `coverage_format` 文档 | 扩展为 5 值枚举 |
| `measured` 类型 | `{lines: number, branches: number \| null, functions: number \| null}` |

### Agent: unit-test-evaluator.md

| Check | Change |
|-------|--------|
| U1 validation | `measured.branches` / `measured.functions` 接受 null |
| U3 coverage | null 维度不影响 pass 判定 |
| Step 5 findings | null 维度标注 "N/A (框架不支持)" |
