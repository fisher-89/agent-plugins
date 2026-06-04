# phase-agents Specification

## ADDED Requirements

### Requirement: test-design-planner does not output parameter types or risk markers
test-design-planner SHALL base its test design analysis on proposal.md and design.md. The agent SHALL grep source code to extract real API signatures (function names, parameter types, return types) as supplementary input for informed test scenario design, but SHALL NOT output specific parameter types (such as `string`, `number`, `User`) in test-design.md. Risk markers for untyped files SHALL NOT be included, as type information is intentionally excluded from the test-design phase to avoid premature assumptions that may conflict with the final implementation.

#### Scenario: test-design.md contains no parameter type information
- **WHEN** reading test-design.md output
- **THEN** the artifact SHALL NOT contain a "Parameter Type Table" or equivalent section
- **AND** SHALL NOT list specific parameter types or type annotations for any function or method

#### Scenario: test-design.md contains no risk markers for untyped files
- **WHEN** the change involves JavaScript files or Python files without type hints
- **THEN** the agent SHALL NOT add risk markers or risk-related disclaimers in test-design.md
- **AND** SHALL NOT include notes about type inference or type uncertainty

### Requirement: test-design-planner produces forward and reverse ACs
test-design-planner SHALL categorize acceptance criteria into two distinct sections in test-design.md:
- **Forward ACs** (正向 AC): Happy path business scenarios that verify correct behavior under valid inputs
- **Reverse ACs** (反向 AC): Sad path business scenarios covering error handling, invalid inputs, boundary conditions, and expected failure modes

#### Scenario: test-design.md contains separate forward and reverse AC sections
- **WHEN** reading test-design.md output
- **THEN** the artifact SHALL contain a Forward ACs section listing all happy path scenarios
- **AND** a separate Reverse ACs section listing all sad path / error handling scenarios
- **AND** each section references specific acceptance criteria from proposal.md

#### Scenario: reverse ACs cover error handling for each forward path
- **WHEN** a Forward AC describes a successful login scenario
- **THEN** the corresponding Reverse AC SHALL cover invalid credentials, missing fields, and rate limiting scenarios

### Requirement: test-design-evaluator T2 checks AC coverage in coverage map
The T2 checklist item SHALL verify that every Forward AC (正向 AC) and Reverse AC (反向 AC) listed in test-design.md has a corresponding entry in the coverage map. Since test-design does not output parameter types, coverage verification is based on the test design's own acceptance criteria rather than source-level method signatures.

#### Scenario: test-design-evaluator T2 passes when all ACs have coverage map entries
- **WHEN** the coverage map in test-design.md contains entries for every Forward AC and Reverse AC
- **THEN** T2 SHALL pass with evidence listing each AC identifier and its corresponding coverage map entry

#### Scenario: test-design-evaluator T2 fails when an AC lacks a coverage map entry
- **WHEN** a Forward AC or Reverse AC is defined in test-design.md but has no corresponding coverage map entry
- **THEN** T2 SHALL fail with evidence listing the uncovered AC identifier

### Requirement: test-gen-generator reads source code (removed file type blacklist)
The file type blacklist constraint SHALL be removed from test-gen-generator's agent definition. The following types SHALL no longer be restricted:
`.ts`, `.tsx`, `.js`, `.jsx`, `.py`, `.rs`, `.go`, `.java`, `.c`, `.cpp`, `.h`, `.hpp`, `.hxx`, `.cxx`

test-gen-generator SHALL read source code files to understand method semantics, actual parameter types, and implementation logic for comprehensive test generation.

#### Scenario: test-gen-generator reads Python source files
- **WHEN** test-gen-generator runs for a change affecting `src/auth.py`
- **THEN** the agent SHALL read `src/auth.py` to understand function signatures and business logic
- **AND** generate test file `src/test_auth.py` with appropriate imports and test structure matching the source

#### Scenario: test-gen-generator reads TypeScript source files
- **WHEN** test-gen-generator runs for a change affecting `src/api/user.ts`
- **THEN** the agent SHALL read `src/api/user.ts` to understand TypeScript interfaces, function signatures, and async patterns
- **AND** generate test file `src/api/user.test.ts` with appropriate imports and test structure

#### Scenario: test-gen-generator reads Rust source files
- **WHEN** test-gen-generator runs for a change affecting `src/processor.rs`
- **THEN** the agent SHALL read `src/processor.rs` to understand function signatures and error handling patterns
- **AND** generate test file `src/processor_test.rs` following cargo test conventions

#### Scenario: test-gen-generator reads Go source files
- **WHEN** test-gen-generator runs for a change affecting `src/handler.go`
- **THEN** the agent SHALL read `src/handler.go` to understand exported function signatures
- **AND** generate test file `src/handler_test.go` following go test conventions

### Requirement: test-gen-generator writes colocated test files
test-gen-generator SHALL write test files colocated with the source files they test. The output location SHALL follow language-specific naming conventions:

| Source Type | Test File Naming | Location |
|-------------|-----------------|----------|
| `.py` | `test_<module>.py` | Same directory as source |
| `.ts` / `.tsx` | `<module>.test.ts` / `<module>.test.tsx` | Same directory as source |
| `.rs` | `<module>_test.rs` or inline `#[cfg(test)] mod tests` | Same directory as source |
| `.go` | `<module>_test.go` | Same directory as source |

The agent SHALL NOT write test files under `openspec/changes/<name>/tests/`.

#### Scenario: test file is colocated with Python module
- **WHEN** generating tests for `src/processor.py`
- **THEN** the test file SHALL be written as `src/test_processor.py`
- **AND** NOT under `openspec/changes/<name>/tests/`

#### Scenario: test file is colocated with TypeScript module
- **WHEN** generating tests for `src/api/handler.ts`
- **THEN** the test file SHALL be written as `src/api/handler.test.ts`
- **AND** NOT under `openspec/changes/<name>/tests/`

#### Scenario: test file is colocated with Rust module
- **WHEN** generating tests for `src/parser.rs`
- **THEN** the test file SHALL be written as `src/parser_test.rs` OR as an inline `#[cfg(test)] mod tests` block within the source file
- **AND** NOT under `openspec/changes/<name>/tests/`

### Requirement: test-gen-generator generates edge cases from parameter types
test-gen-generator SHALL use its source code analysis to extract real parameter types and systematically derive edge case test scenarios. The agent SHALL apply the parameter type to edge case mapping for each function parameter, producing at minimum the following categories per type. Since test-design.md does not output parameter types, the generator SHALL obtain type information directly from reading the source files:

| Type | Edge Cases |
|------|-----------|
| int / number | 0, -1, MAX_INT, None/undefined |
| str / string | "", 超长字符串, 特殊字符(\n \0 emoji), None |
| bool | True, False, None |
| list / array | [], [单元素], 超大列表, None |
| dict / object | {}, 缺失必填字段, 多余字段, None |
| Optional[T] | None |
| Enum | 每个枚举值, 非法枚举值 |
| float | 0.0, -0.0, NaN, Inf, None |

#### Scenario: integer parameter generates systematic boundary tests
- **WHEN** a function parameter is typed `int` or `number` (e.g., `def set_limit(limit: int)`)
- **THEN** the generator SHALL create test cases for: `limit=0`, `limit=-1`, `limit=MAX_INT`, `limit=None`
- **AND** include at least one valid in-range value test

#### Scenario: string parameter generates systematic boundary tests
- **WHEN** a function parameter is typed `str` or `string` (e.g., `function greet(name: string)`)
- **THEN** the generator SHALL create test cases for: empty string `""`, very long string (>1000 chars), special characters including `\n`, `\0`, emoji, and `None`/`undefined`
- **AND** include at least one valid string test case

#### Scenario: list parameter generates systematic boundary tests
- **WHEN** a function parameter is typed `list`, `array`, or `List[T]` (e.g., `def process_items(items: List[str])`)
- **THEN** the generator SHALL create test cases for: empty list `[]`, single-element list, very large list, and `None`

#### Scenario: bool parameter generates systematic boundary tests
- **WHEN** a function parameter is typed `bool` or `boolean` (e.g., `def toggle(enable: bool)`)
- **THEN** the generator SHALL create test cases for: `True`, `False`, and `None`

### Requirement: test-gen-evaluator G1 checks source-colocated test file existence
The G1 checklist item SHALL be updated from:
> "test-design.md 中每个 coverage map 条目在 `openspec/changes/<name>/tests/` 下都有对应的测试文件"

To:
> "源码中每个公开方法在源码目录中有对应的测试文件"

Rationale: Since test files are now colocated with source, G1 verifies that each public method in the affected source files has a matching test file in the same directory.

#### Scenario: test-gen-evaluator G1 passes with colocated test files
- **WHEN** `src/auth.py` has a matching `src/test_auth.py` that covers all public methods
- **THEN** G1 SHALL pass with evidence listing the colocated file pairs and the methods they cover

#### Scenario: test-gen-evaluator G1 fails when test file is missing
- **WHEN** `src/auth.py` exists but no `src/test_auth.py` file is present in the same directory
- **THEN** G1 SHALL fail listing `src/test_auth.py` as the missing test file path

### Requirement: test-gen-evaluator G2 checks naming convention and colocation
The G2 checklist item SHALL be updated from:
> "测试文件遵循项目命名规范且位于 `openspec/changes/<name>/tests/`"

To:
> "测试文件命名遵循语言规范且与源码共存于同一目录"

Rationale: The location constraint changes from `openspec/changes/<name>/tests/` to the source file's directory. Naming must follow language conventions for the test framework to discover them.

#### Scenario: test-gen-evaluator G2 passes with correct naming and colocation
- **WHEN** test files are named `test_*.py`, `*.test.ts`, `*_test.rs`, or `*_test.go` in the same directory as their source files
- **THEN** G2 SHALL pass with evidence listing each test file's name and directory location

#### Scenario: test-gen-evaluator G2 fails with wrong naming convention
- **WHEN** a generated test file uses incorrect naming (e.g., `auth_test.py` for a Python source instead of `test_auth.py`)
- **THEN** G2 SHALL fail citing the incorrect naming and the expected convention

#### Scenario: test-gen-evaluator G2 fails when test file is mislocated
- **WHEN** a test file is correctly named but placed in a different directory from its source (e.g., `openspec/changes/<name>/tests/test_auth.py` instead of `src/test_auth.py`)
- **THEN** G2 SHALL fail citing the mislocated path and the expected colocated location

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | Status | Input Changes | Output Changes |
|-------|--------|--------------|----------------|
| test-design-planner | MODIFIED | ADDED: Grep source code for real API signatures (function names, parameter types, return types) as supplementary input alongside proposal.md + design.md | ADDED: Forward ACs section, Reverse ACs section. SHALL NOT output parameter types or risk markers. |
| test-design-evaluator | MODIFIED | (unchanged) | MODIFIED: T2 checklist updated to verify each Forward/Reverse AC has a coverage map entry (not source-method-based) |
| test-gen-generator | MODIFIED | ADDED: Read source code files (removed file type blacklist); Extract parameter types directly from source (not from test-design.md) | MODIFIED: Test files written colocated with source instead of tests/ directory; ADDED: Systematic edge case tests from source-extracted parameter types |
| test-gen-evaluator | MODIFIED | (unchanged) | MODIFIED: G1 checklist updated from tests/ file check to colocated file check; G2 checklist updated from tests/ location to source-directory colocation |

### Template Files (`plugins/dev-team/templates/artifacts/`)

| Template | Changes |
|----------|---------|
| test-design.md.template | ADDED: 反向 AC (Reverse ACs) section alongside existing coverage map. Parameter type table and risk markers are intentionally excluded since test-design does not output type information. |
