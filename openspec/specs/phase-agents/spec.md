## Requirements

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

### Requirement: Subagent agent.md carries domain knowledge (MCP tool update)

The sub-agent table SHALL be updated to reflect the new MCP tools available to the unit-test-executor subagent:

| Agent | Role | MCP Tool |
|-------|------|----------|
| unit-test-executor | Executor subagent (sonnet) | test_detect_frameworks, test_get_framework_config |
| test-gen-generator | Generator subagent (sonnet) | test_detect_frameworks, test_get_framework_config |

#### Scenario: unit-test-executor agent.md references new MCP tools

- **WHEN** reading `agents/unit-test-executor.md`
- **THEN** the file SHALL contain references to `mcp__plugin_dev-team_dev-team__test_detect_frameworks` and `mcp__plugin_dev-team_dev-team__test_get_framework_config`
- **AND** the Process section SHALL include steps for framework detection, command resolution, and coverage execution

### Requirement: test-gen-generator agent calls MCP tools for framework-aware test generation

`plugins/dev-team/agents/test-gen-generator.md` SHALL be updated to call `test_detect_frameworks` MCP tool at the start of its Process section to detect the project's test framework(s), and `test_get_framework_config` MCP tool for each detected framework to obtain framework conventions for generating framework-specific test syntax.

The Process section SHALL include the following new steps:

1. **Framework detection** (after determining change name, before reading source code): Call `test_detect_frameworks` to detect the test framework(s). If config.json has `test.frameworks` configured, use it; otherwise fall back to file-extension heuristics.
2. **Framework config resolution** (new step): Call `test_get_framework_config` for each detected framework to get the framework name, enabling framework-specific syntax selection.

Test skeleton generation SHALL use the detected framework's native test syntax:

| Framework | Test Syntax | Import / Module Declaration |
|-----------|-------------|----------------------------|
| jest | `describe` / `it` / `expect` | `import { describe, it, expect } from '@jest/globals'` |
| vitest | `describe` / `it` / `expect` / `vi` | `import { describe, it, expect, vi } from 'vitest'` |
| vite-plus | `describe` / `it` / `expect` | (same as vitest convention) |
| bun | `describe` / `test` / `expect` | `import { describe, test, expect } from 'bun:test'` |
| rust | `#[cfg(test)]` module, `#[test]` functions | `mod tests { use super::*; #[test] fn ... }` |

All generated test skeletons SHALL include framework-appropriate skip markers:
- Jest / Vitest / Vite-plus / Bun: `it.skip(...)` or `test.skip(...)`
- Rust: `#[ignore]`

#### Scenario: test-gen-generator detects vitest and generates vitest-syntax skeletons

- **WHEN** test-gen-generator begins its Process section
- **THEN** it SHALL call `mcp__plugin_dev-team_dev-team__test_detect_frameworks` to detect the project's test framework
- **AND** if vitest is detected, call `mcp__plugin_dev-team_dev-team__test_get_framework_config` with `{"framework": "vitest"}`
- **AND** generate test skeletons using `import { describe, it, expect, vi } from 'vitest'` syntax
- **AND** use `it.skip(...)` as the skip marker

#### Scenario: test-gen-generator detects jest and generates jest-syntax skeletons

- **WHEN** test-gen-generator detects `jest` as the project test framework
- **THEN** it SHALL generate test skeletons using `describe` / `it` / `expect` patterns
- **AND** use `import { describe, it, expect } from '@jest/globals'` for imports
- **AND** name test files `<name>.test.ts` colocated with source

#### Scenario: test-gen-generator detects bun and generates bun test syntax

- **WHEN** test-gen-generator detects `bun` as the test framework
- **THEN** it SHALL generate test skeletons using `describe` / `test` / `expect` patterns (not `it`)
- **AND** use `import { describe, test, expect } from 'bun:test'` for imports

#### Scenario: test-gen-generator detects rust and generates inline test module

- **WHEN** test-gen-generator detects `rust` as the test framework for `.rs` source files
- **THEN** it SHALL generate inline `#[cfg(test)]` module within the source file containing `#[test]` annotated test functions
- **AND** use `#[ignore]` as the skip marker on each test function
- **AND** include `use super::*;` in the test module

#### Scenario: test-gen-generator falls back to heuristics when no config

- **WHEN** `test_detect_frameworks` returns empty results or config.json has no `test.frameworks`
- **THEN** test-gen-generator SHALL fall back to file-extension heuristics:
  - `.ts` / `.tsx` / `.js` / `.jsx` => vitest-style (describe / it / expect)
  - `.py` => pytest (def test_*)
  - `.rs` => rust (#[cfg(test)] mod tests)
- **AND** use the corresponding framework-appropriate syntax and skip markers

#### Scenario: test-gen-generator uses framework-matching skip markers

- **WHEN** test-gen-generator generates a test skeleton for a vitest project
- **THEN** it SHALL use `it.skip(...)` or `test.skip(...)` as the skip marker
- **AND** SHALL NOT use `@pytest.mark.skip` or `#[ignore]` which belong to different frameworks

### Requirement: test-design-planner 调用 test_resolve_paths 获取测试文件路径

`plugins/dev-team/agents/test-design-planner.md` SHALL 在编写 test-design.md 之前调用 MCP 工具 `test_resolve_paths`，根据 design.md 与源码 Grep 结果确定的模块列表获取规定的测试文件路径。

Process 章节 SHALL 包含以下步骤（在读取 proposal.md、design.md 并 Grep 源码之后，写入 test-design.md 之前）：

1. 从 design.md 的变更范围与 Grep 结果汇总**精确模块列表**（文件路径或目录路径，相对于项目根目录）
2. 从 proposal.md / design.md 识别需要集成测试覆盖的**场景名**列表（若有）
3. 调用 `mcp__plugin_dev-team_dev-team__test_resolve_paths`，传入 `modules` 与可选的 `integration_scenarios`、`extension`
4. 将返回的 `unit_tests` 映射到 test-design.md `验收范围` 与 `单元测试 > 用例` 表格的 `测试文件` 列（`source` → 被测模块，`test_file` → 测试文件路径）
5. 将返回的 `integration_tests` 映射到 `集成测试 > 用例` 表格的 `测试文件` 列
6. 若 `errors` 非空，在 test-design.md `不可测试项` 或相应章节记录无法解析的模块及原因

Agent SHALL NOT 手工拼接或猜测测试文件路径；所有 `测试文件` 列的值 MUST 来自 `test_resolve_paths` 返回值（或明确标注为不可测试并说明原因）。

Agent 工具权限 SHALL 包含 `test_resolve_paths` MCP 工具。

#### Scenario: planner 使用 MCP 返回的单元测试路径填充 coverage map

- **WHEN** `test_resolve_paths` 对 `modules: ["src/config.ts"]` 返回 `unit_tests: [{source: "src/config.ts", test_file: "src/config.test.ts"}]`
- **THEN** test-design.md `验收范围` 表格中对应 AC 行的 `测试文件` 列为 `src/config.test.ts`
- **AND** `单元测试 > 用例` 表格中相关行的 `测试文件` 列同为 `src/config.test.ts`

#### Scenario: planner 使用 MCP 返回的集成测试路径

- **WHEN** `test_resolve_paths` 对 `integration_scenarios: ["api-flow"]` 返回 `integration_tests: [{scenario: "api-flow", test_file: "__tests__/api-flow/api-flow.test.ts"}]`
- **THEN** test-design.md `集成测试 > 用例` 表格的 `测试文件` 列为 `__tests__/api-flow/api-flow.test.ts`

#### Scenario: planner 传入目录级模块列表

- **WHEN** design.md 变更范围为目录 `plugins/dev-team/bin/src/commands/`
- **THEN** planner 将该目录路径传入 `test_resolve_paths` 的 `modules` 参数
- **AND** 使用返回的多条 `unit_tests` 条目分别填充各源文件对应的测试文件路径

#### Scenario: planner 记录 errors 中的不可解析模块

- **WHEN** `test_resolve_paths` 返回 `errors: [{path: "src/legacy.md", message: "..."}]`
- **THEN** test-design.md 在 `不可测试项` 章节列出该路径及原因
- **AND** 不在 coverage map 中为该路径填写虚构的测试文件路径

#### Scenario: test-design-planner.md 包含 test_resolve_paths 调用说明

- **WHEN** 读取 `plugins/dev-team/agents/test-design-planner.md`
- **THEN** Process 章节包含对 `mcp__plugin_dev-team_dev-team__test_resolve_paths` 的调用步骤
- **AND** 明确禁止手工拼接测试文件路径

### Requirement: implementation-generator 不再负责静态检查

`plugins/dev-team/agents/implementation-generator.md` SHALL 移除所有静态检查相关指令。具体包括：

- Process 步骤 7-8（调用 `config_get("static_analysis")` 及执行检查）
- Output 章节中 `reports/static_analysis.json` 报告生成要求及 JSON 模板
- frontmatter `description` 中对 AUTO static-check 的引用

Process 步骤 6（标记 tasks.md 完成）SHALL 成为最后一步。Generator 的核心职责 SHALL 仅为：读取 design/tasks/specs、编写实现代码、标记任务完成。

Generator SHALL NOT 调用 `config_get` 获取 `static_analysis` 配置。
Generator SHALL NOT 写入 `openspec/changes/<change-name>/reports/static_analysis.json`。

静态检查 SHALL 由 `subagentStop` hook（`static-check-hook`）在 agent 结束时自动执行。

#### Scenario: implementation-generator Process 不含静态检查步骤

- **WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md` 的 `## Process` 章节
- **THEN** 不包含 `config_get` 调用或 `static_analysis` 关键字
- **AND** 最后一步为标记 tasks.md 任务完成（原步骤 6）
- **AND** 不存在原步骤 7、8

#### Scenario: implementation-generator Output 不含 static_analysis 报告

- **WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md` 的 `## Output` 章节
- **THEN** 不包含 `reports/static_analysis.json` 路径
- **AND** 不包含静态检查 JSON 报告模板

#### Scenario: implementation-generator frontmatter 不引用 AUTO static-check

- **WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md` 的 frontmatter `description`
- **THEN** 不包含 `static-check` 或 `static_analysis` 相关描述

### Requirement: implementation-evaluator 移除 I7 静态检查项

`plugins/dev-team/agents/implementation-evaluator.md` SHALL 从 Static Checklist 表格中移除 I7 行：

> I7 | 静态检查通过（lint、类型检查） | 若config_get {"key":"static_analysis"} 可以获取检查脚本，检查有 `openspec/changes/<change-name>/reports/static_analysis.json`

移除 I7 后，evaluator 的判定规则 SHALL 仍使用 `"pass" only if ALL items pass"`，检查项范围为 I1-I6 及 I8（原 I8 保持不变）。

Evaluator SHALL NOT 读取 `reports/static_analysis.json` 或调用 `config_get` 验证静态检查结果。

#### Scenario: implementation-evaluator 检核表不含 I7

- **WHEN** 读取 `plugins/dev-team/agents/implementation-evaluator.md` 的 `## Static Checklist` 表格
- **THEN** 表格中不存在 ID 为 `I7` 的行
- **AND** 不包含 `static_analysis` 或 `reports/static_analysis.json` 关键字

#### Scenario: implementation-evaluator 判定范围更新

- **WHEN** 读取 `plugins/dev-team/agents/implementation-evaluator.md` 的判定规则
- **THEN** 规则表述为 `"pass" only if ALL items pass"`
- **AND** 检查项 ID 范围为 I1-I6 和 I8，不含 I7

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
