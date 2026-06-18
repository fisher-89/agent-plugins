## MODIFIED Requirements

### Requirement: unit-test-executor agent calls test_detect_frameworks and test_get_framework_config MCP tools

`plugins/dev-team/agents/unit-test-executor.md` SHALL be updated to call the `test_detect_frameworks` MCP tool at the start of its Process section to determine which test frameworks are present in the project.
After determining the framework(s), the Executor SHALL use the `plan` array returned by `test_detect_frameworks` (including `script`, `coverage_cmd`, `coverage_artifacts`, `coverage_cleanup`) to run coverage commands — it SHALL NOT call `test_get_framework_config` separately.

The Process section SHALL be updated as follows:

1. **Framework detection** (before test execution): Call `test_detect_frameworks` to get the list of detected frameworks and execution plan. If no frameworks are detected, fall back to manual file globbing (existing behavior).
2. **Coverage execution**: For each plan entry, run the returned `script` in the corresponding `directory`.
3. **Artifact move**: Move JSON coverage summary files to `reports/coverage/<framework>/` per plan configuration (SHALL NOT move or record HTML reports).
4. **Coverage parsing**: Parse coverage output using `coverage_format`, extracting lines / branches / functions three dimensions into per-framework `measured` values.
5. **Thresholds & overrides check**: Read `test.coverage.thresholds` and `test.coverage.overrides` from config.json; compute weighted average into `coverage.measured`; populate `coverage.overrides`; set `coverage.pass` via ALL logic (global + overrides groups all pass).
6. **Test case extraction**: Extract per-case results into `test_cases[]`; for failed cases, include `line`, `error_type`, `error_message`, `stack_trace` directly on the test case entry.
7. **Report writing**: Write structured report with nested `coverage` object (or `coverage: null`), `test_cases[]`, and optional `integration_test` sub-report. When `integration_test` is present, its `test_cases[]` SHALL use the same schema (error fields on failed entries; no `failures[]`). SHALL NOT write top-level `coverage_thresholds`, `coverage_pass`, `coverage_by_framework`, `coverage_overrides`, `html_reports`, or `failures`.

The agent tool list SHALL include `test_detect_frameworks` MCP tool permission.

#### Scenario: Executor writes nested coverage object in report

- **WHEN** unit-test-executor completes coverage execution and threshold checks
- **THEN** it writes `coverage: {pass, measured, thresholds, by_framework, overrides}` to `unit-test-execution.json`
- **AND** does NOT write separate top-level `coverage_pass`, `coverage_thresholds`, `coverage_by_framework`, `coverage_overrides`, or `html_reports` fields
- **AND** `by_framework` entries do NOT contain `html_report` (coverage uses JSON-only output)

#### Scenario: Executor merges failure details into test_cases

- **WHEN** unit tests fail with assertion errors
- **THEN** the Executor writes failed entries in `test_cases[]` with `status: "failed"` and error detail fields (`line`, `error_type`, `error_message`, `stack_trace`)
- **AND** does NOT write a top-level `failures[]` array

#### Scenario: Executor writes coverage null when no coverage configured

- **WHEN** `test_detect_frameworks` returns an empty plan (no `test.frameworks` configured in config.json)
- **THEN** the Executor SHALL fall back to manual file globbing for test files
- **AND** report `coverage: null` in the output report (no other coverage-related top-level fields)

#### Scenario: Executor integration_test sub-report uses same test_cases schema

- **WHEN** integration tests are executed and included in the report as `integration_test`
- **THEN** failed integration test details appear in `integration_test.test_cases[]` with error fields on failed entries
- **AND** `integration_test` does NOT contain a separate `failures[]` array

### Requirement: unit-test-evaluator agent reads expanded coverage fields from report

`plugins/dev-team/agents/unit-test-evaluator.md` SHALL be updated to read the simplified nested coverage object and merged test_cases from the execution report.
The evaluator SHALL:
- Read `coverage.pass` from the report for direct gate pass/fail decision (when `coverage` is not null)
- Read `coverage.thresholds` and `coverage.overrides` from the report (already populated by executor from config.json)
- Read `coverage.measured` for per-dimension evidence in the findings
- Read `coverage.by_framework` for detailed per-framework evidence (framework name and measured values)
- Read failed test details from `test_cases[]` entries where `status === "failed"` (fields: `error_type`, `file`, `line`, `error_message`, `stack_trace`, optional `design_ref`) for the diagnostic decision tree

The evaluator SHALL NOT re-calculate coverage or re-run coverage commands. All coverage calculations are performed by the executor.
The evaluator SHALL NOT reference HTML coverage report paths in findings or checklist evidence.

The Static Checklist U1 SHALL require: `phase`, `command`, `timestamp`, `total`, `passed`, `failed`, `skipped`, `coverage` (object or null), `duration_seconds`, `test_cases` (array).

The Static Checklist U3 SHALL pass when `coverage === null` OR `coverage.pass === true`.

Step 1 "Validate report completeness" SHALL validate `coverage` as either a nested object with required sub-fields (`pass`, `measured`, `thresholds`, `by_framework`, `overrides`) or `null`. It SHALL NOT require `failures`, `html_reports`, or flat coverage fields.

Step 4 diagnostic decision tree SHALL iterate `test_cases.filter(c => c.status === "failed")` instead of a separate `failures` array. Decision categories, priorities, and backtrack targets SHALL remain unchanged.

Step 5 findings template SHALL reference `coverage.measured`, `coverage.pass`, and `coverage.by_framework[].measured` instead of flat coverage fields. It SHALL NOT reference any HTML report paths.

#### Scenario: Evaluator validates nested coverage object

- **WHEN** unit-test-evaluator reads the execution report that contains `coverage: {pass: true, measured: {lines, branches, functions}, thresholds: {...}, by_framework: [...], overrides: [...]}`
- **THEN** it SHALL use `coverage.pass` to determine the coverage checklist item result
- **AND** include `coverage.measured` per-dimension values and `coverage.by_framework[].measured` details in the findings text (no HTML report references)

#### Scenario: Evaluator handles report with coverage null

- **WHEN** the execution report contains `"coverage": null`
- **THEN** the evaluator SHALL pass the coverage check with evidence "覆盖率检查未配置或生成失败，跳过"
- **AND** SHALL NOT fail U1 for missing coverage sub-fields

#### Scenario: Evaluator decision tree reads from failed test_cases

- **WHEN** the report has `failed > 0` and failed entries in `test_cases[]` with `error_type: "AssertionError"`
- **THEN** the evaluator applies the diagnostic decision tree using `error_type`, `file`, and `line` from those test_cases entries
- **AND** produces the same backtrack_to result as the prior schema that used `failures[]`

#### Scenario: Evaluator rejects report with deprecated flat fields only

- **WHEN** the execution report uses the old flat schema (`coverage_pass`, `failures[]`) without the nested `coverage` object or merged test_cases error fields
- **THEN** U1 SHALL fail with evidence listing missing or incorrect field structure
- **AND** verdict is `"fail"` with `backtrack_to: null` (re-run Executor)

## ADDED Requirements

### Requirement: FRAMEWORK_REGISTRY coverage commands use JSON-only reporter

`plugins/dev-team/bin/src/commands/test-get-framework-config.ts` 中的 `FRAMEWORK_REGISTRY` SHALL 为各框架配置 JSON-only coverage reporter，确保脚本输出仅包含 JSON 摘要文件。

各框架 `coverage_cmd` SHALL 指定 JSON reporter 参数：
- **vitest**: `npx vitest run --coverage --coverage.reporter=json-summary`
- **jest**: `npx jest --coverage --coverageReporters=json-summary`
- **vite-plus**: `vp test --coverage --coverage.reporter=json-summary`
- **bun**: `bun test --coverage --coverageReporters=json-summary`
- **rust**: `cargo llvm-cov --json`（llvm-cov 原生输出即为 JSON）

各框架 `coverage_artifacts` SHALL 收窄为仅 JSON 文件路径（如 `['coverage/coverage-summary.json']`），不包含 HTML 文件或 `coverage/**` glob。

`coverage_cleanup` 保持不变（仍清理整个 `coverage/` 临时目录）。

#### Scenario: vitest plan 返回 JSON-only coverage command

- **WHEN** `test_detect_frameworks` 为 vitest 框架生成 plan entry
- **THEN** `coverage_cmd` 为 `npx vitest run --coverage --coverage.reporter=json-summary`
- **AND** `coverage_artifacts` 为 `['coverage/coverage-summary.json']`
- **AND** 生成的 `script` 中包含该 JSON-only coverage command

#### Scenario: jest plan 返回 JSON-only coverage command

- **WHEN** `test_detect_frameworks` 为 jest 框架生成 plan entry
- **THEN** `coverage_cmd` 为 `npx jest --coverage --coverageReporters=json-summary`
- **AND** `coverage_artifacts` 为 `['coverage/coverage-summary.json']`

#### Scenario: rust plan 返回 JSON-only coverage command

- **WHEN** `test_detect_frameworks` 为 rust 框架生成 plan entry
- **THEN** `coverage_cmd` 为 `cargo llvm-cov --json`
- **AND** `coverage_artifacts` 为 `['coverage/coverage-summary.json']`

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | Tools | Contract |
|-------|-------|----------|
| unit-test-executor | Read, Write, Grep, Glob, Bash, test_detect_frameworks | Runs plan scripts; computes nested `coverage` object or `coverage: null`; writes `test_cases[]` with error fields on failed entries; no `failures[]` or `html_reports` |
| unit-test-evaluator | Read, phase_log | Reads `coverage.pass`, `coverage.measured`, `coverage.by_framework`, `coverage.overrides`; decision tree from failed `test_cases`; `coverage === null` auto-passes coverage check |

### MCP 工具变更

| 文件 | 变更 |
|------|------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` | `FRAMEWORK_REGISTRY` 各框架 `coverage_cmd` 加 JSON reporter 参数；`coverage_artifacts` 收窄为 JSON 文件 |

### Agent 文件变更

| Agent | 文件 | 变更 |
|-------|------|------|
| unit-test-executor | `plugins/dev-team/agents/unit-test-executor.md` | 报告 JSON 模板改为嵌套 `coverage`（无 html_report）、合并 failures 到 test_cases、覆盖率仅 JSON 输出 |
| unit-test-evaluator | `plugins/dev-team/agents/unit-test-evaluator.md` | U1/U3、Step 1/3/4/5 字段引用更新为新 schema |

### 职责边界

| 组件 | 报告 schema 职责 |
|------|-----------------|
| FRAMEWORK_REGISTRY | 配置 — 各框架 JSON-only coverage command 及 artifacts 路径 |
| unit-test-executor | 写入 — 嵌套 coverage、test_cases 含错误详情 |
| unit-test-evaluator | 读取 — 校验新 schema、决策树从 test_cases 失败条目取输入 |
| integration-test-executor/evaluator | 不在本次变更范围 — 仍使用现有独立报告格式 |
