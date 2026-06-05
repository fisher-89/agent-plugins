## ADDED Requirements

### Requirement: unit-test-executor agent calls test_detect_frameworks and test_get_framework_config MCP tools

`plugins/dev-team/agents/unit-test-executor.md` SHALL be updated to call the `test_detect_frameworks` MCP tool at the start of its Process section to determine which test frameworks are present in the project.
After determining the framework(s), the Executor SHALL call `test_get_framework_config` MCP tool for each detected framework to obtain the appropriate test command and coverage command.

The Process section SHALL be updated as follows:

1. **Framework detection** (new step, before test execution): Call `test_detect_frameworks` to get the list of detected frameworks. If no frameworks are detected, fall back to manual file globbing (existing behavior).
2. **Command resolution** (new step): For each detected framework, call `test_get_framework_config` to resolve `test_cmd` and `coverage_cmd`.
3. **Test execution**: Run test commands (same as existing, but using resolved commands from step 2).
4. **Coverage execution** (new step): After test execution, run `coverage_cmd` for each framework.
5. **Coverage parsing** (new step): Parse coverage output using the `coverage_format` returned from `test_get_framework_config`, extracting lines / branches / functions three dimensions.
6. **Thresholds & overrides check** (new step): Read `test.coverage.thresholds` and `test.coverage.overrides` from config.json; compute `coverage_pass` via ALL logic (global + overrides groups all pass).
7. **Report writing**: Write extended report with `coverage: {lines, branches, functions}`, `coverage_thresholds`, `coverage_overrides`, `coverage_pass`, `coverage_by_framework`, `html_reports` fields.

The agent tool list SHALL include `test_detect_frameworks` and `test_get_framework_config` MCP tool permissions.

#### Scenario: Executor calls test_detect_frameworks before test execution

- **WHEN** unit-test-executor begins execution
- **THEN** it SHALL call `mcp__plugin_dev-team_dev-team__test_detect_frameworks` before running any test commands
- **AND** use the returned framework list to resolve test commands via `mcp__plugin_dev-team_dev-team__test_get_framework_config`

#### Scenario: Executor runs coverage commands after test execution

- **WHEN** test execution completes successfully or with failures
- **THEN** the Executor SHALL run `coverage_cmd` for each detected framework
- **AND** parse the output to extract lines / branches / functions three dimensions of coverage data
- **AND** include coverage data in the structured report

#### Scenario: Executor falls back to manual globbing when no frameworks detected

- **WHEN** `test_detect_frameworks` returns an empty list (no `test.frameworks` configured in config.json)
- **THEN** the Executor SHALL fall back to manual file globbing for `**/*.test.ts`, `**/*.test.js`, `**/test_*.py`, etc.
- **AND** report `coverage: null` and `coverage_pass: false` in the output report

### Requirement: unit-test-evaluator agent reads expanded coverage fields from report

`plugins/dev-team/agents/unit-test-evaluator.md` SHALL be updated to read the expanded coverage fields from the execution report.
The evaluator SHALL:
- Read `coverage_pass` from the report for direct gate pass/fail decision
- Read `coverage_thresholds` and `coverage_overrides` from the report (already populated by executor from config.json)
- Read `coverage: {lines, branches, functions}` for per-dimension evidence in the findings
- Read `coverage_by_framework` for detailed per-framework evidence in the findings
- Read `html_reports` to reference HTML coverage reports in the evaluation evidence

The evaluator SHALL NOT re-calculate coverage or re-run coverage commands. All coverage calculations are performed by the executor.

The Step 1 "Validate report completeness" SHALL be updated to include `coverage_pass` and `coverage_by_framework` as optional but validated fields (present when coverage data was generated, absent when not configured).

#### Scenario: Evaluator validates coverage_pass field

- **WHEN** unit-test-evaluator reads the execution report that contains `coverage_pass`, `coverage: {lines, branches, functions}`, `coverage_thresholds`, and `coverage_by_framework`
- **THEN** it SHALL use `coverage_pass` to determine the coverage checklist item result
- **AND** include per-dimension coverage and `coverage_by_framework` details in the findings text

#### Scenario: Evaluator handles report without coverage fields

- **WHEN** the execution report does not contain `coverage_pass` or `coverage_by_framework`
- **THEN** the evaluator SHALL treat coverage as not configured
- **AND** pass the coverage check with evidence "覆盖率检查未配置或生成失败，跳过"

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

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | Tools | Contract |
|-------|-------|----------|
| unit-test-executor | Read, Write, Grep, Glob, Bash, test_detect_frameworks, test_get_framework_config | Calls `test_detect_frameworks` to detect frameworks, `test_get_framework_config` to resolve commands; runs coverage after tests; reads thresholds + overrides from config.json; computes coverage_pass (ALL: lines AND branches AND functions for global + overrides); writes extended report with `coverage: {lines, branches, functions}`, `coverage_thresholds`, `coverage_overrides`, `coverage_pass`, `coverage_by_framework`, `html_reports` |
| unit-test-evaluator | Read, phase_log | Reads expanded coverage fields (`coverage: {lines, branches, functions}`, `coverage_pass`, `coverage_thresholds`, `coverage_overrides`, `coverage_by_framework`) from report; uses `coverage_pass` for gate decision; findings include specific failing dimensions and override groups |
| test-gen-generator | Read, Write, Grep, Glob, Bash, test_detect_frameworks, test_get_framework_config | Calls `test_detect_frameworks` to detect test frameworks, `test_get_framework_config` to get framework conventions; generates framework-specific test syntax (jest: describe/it, vitest: describe/it/vi, bun: describe/test, rust: #[cfg(test)]) with matching skip markers; writes colocated test files with systematic edge case tests from parameter types |
