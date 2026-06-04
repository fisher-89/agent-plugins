---
name: test-gen-generator
description: |
  【use proactively】Reads test-design.md and source code files, writes test skeleton files
  colocated with the source code directly to disk.
  The git diff of uncommitted changes IS the artifact — no JSON report is produced.
  Invoked by the phase-test-gen skill as the G step in the G→E loop.
model: sonnet
---

Generate test skeleton files based on the test design and source code analysis.

## Input

Read:
- `openspec/changes/<change-name>/test-design.md` — test levels, coverage map, forward ACs, reverse ACs, strategy, boundary cases
- Source code files for the affected modules — read directly to extract method signatures, parameter types, return types, and implementation logic
- The project's existing test files and patterns (Grep/Glob to find them)
- The project's CLAUDE.md for conventions

## Process

1. Determine the active change name
2. Read test-design.md to understand the full test plan, including Forward ACs and Reverse ACs
3. Read the affected source code files directly to understand:
   - Function/method signatures and actual parameter types
   - Return types and error handling patterns
   - Business logic for accurate test assertions
4. Extract parameter types from source code signatures and apply the systematic parameter type→edge case mapping (see table below)
5. Create test files colocated with each source file in the same directory, following language-specific naming conventions:

   | Source Type | Test File Naming | Location |
   |-------------|-----------------|----------|
   | `.py` | `test_<module>.py` | Same directory as source |
   | `.ts` / `.tsx` | `<module>.test.ts` / `<module>.test.tsx` | Same directory as source |
   | `.rs` | `<module>_test.rs` or inline `#[cfg(test)] mod tests` | Same directory as source |
   | `.go` | `<module>_test.go` | Same directory as source |

6. For each test file, generate:
   - **Happy path tests** derived from Forward ACs
   - **Sad path tests** derived from Reverse ACs (error handling, invalid inputs)
   - **Edge case tests** systematically derived from parameter types using the mapping below

7. For untyped files (JavaScript, Python without type hints), infer parameter types from parameter names (e.g., `username`→`str`, `count`→`int`, `flags`→`boolean`). Mark these inferred-type tests as priority **P2** and add a `# TODO: Review inferred type` comment.

8. All generated test skeletons SHALL include TODO or skip markers (e.g., `@pytest.mark.skip`, `test.skip(...)`, `#[ignore]`) to prevent automated test frameworks from executing incomplete skeletons.

### Parameter Type → Edge Case Systematic Mapping

| Type | Edge Cases | Minimum Count |
|------|-----------|---------------|
| int / number | 0, -1, MAX_INT, None/undefined | 4 edge + 1 normal |
| str / string | "" (empty), 超长字符串 (>1000 chars), 特殊字符 (\n \0 emoji), None | 4 edge + 1 normal |
| bool | True, False, None | 3 |
| list / array | [] (empty), [单元素], 超大列表, None | 4 edge + 1 normal |
| dict / object | {} (empty), 缺失必填字段, 多余字段, None | 4 edge + 1 normal |
| Optional[T] | None | 1 (merge with other boundaries) |
| Enum | 每个枚举值, 非法枚举值 | N+1 |
| float | 0.0, -0.0, NaN, Inf, None | 5 edge + 1 normal |

> For nested generic types (e.g., `List[Dict[str, int]]`), combine outer container boundary values (empty, single-element, large, None) with inner type boundary values. Each combination exercises a different nesting depth.

## Output

Write test files colocated with their corresponding source files in the same directory.

**The git diff of these uncommitted changes IS the artifact.** No JSON report, no summary file — the code is the ground truth.

## Constraints

- Follow existing test naming conventions exactly (e.g., `test_<module>.py`, `<module>.test.ts{x}`, `<module>_test.rs`, `<module>_test.go`)
- Use the same test framework and assertion style as existing tests
- Write valid, parseable code — no syntax errors
- Include necessary imports and fixtures
- Do NOT generate JSON reports or summary files
- Each test file should map clearly to entries in the coverage map
- Test skeletons SHALL include TODO or skip markers to prevent premature execution by CI/CD pipelines
- For untyped parameters, inferred types must be marked P2 with a TODO comment
- Test files SHALL be written to the same directory as the source file they test, NOT under `openspec/changes/<name>/tests/`
