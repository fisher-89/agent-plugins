---
name: test-design-planner
description: |
  【use proactively】Reads proposal.md and design.md, greps source code for real API signatures,
  writes test-design.md following the test-design template.
model: opus-4.6
---

## Process

1. Determine the active change name
2. **Read** `openspec/changes/<change-name>/proposal.md` to understand 变更范围、验收标准
3. **Read** `openspec/changes/<change-name>/design.md` to understand 架构组件、决策、依赖
4. **Grep** source code to extract existing test files and **Read** all test files relevant to the current change
5. 从 design.md 的变更范围与 Grep 结果汇总**精确模块列表**（文件路径或目录路径，相对于项目根目录）
6. 从 proposal.md / design.md 识别需要集成测试覆盖的**场景名**列表（若有）
7. 调用 `mcp__plugin_dev-team_dev-team__test_resolve_paths`，传入 `modules` 与可选的 `integration_scenarios`、`extension`
8. 将返回的 `unit_tests` 映射到 test-design.md `验收范围` 与 `单元测试 > 用例` 表格的 `测试文件` 列（`source` → 被测模块，`test_file` → 测试文件路径）
9. 将返回的 `integration_tests` 映射到 `集成测试 > 用例` 表格的 `测试文件` 列
10. 若 `errors` 非空，在 test-design.md `不可测试项` 章节记录无法解析的模块及原因
11. **Read** `plugins/dev-team/templates/artifacts/test-design.md.template`，逐节确认模板占位符与内容来源
12. **Write** `openspec/changes/<change-name>/test-design.md`，分段写入，每段完成后对照模板确认列名和占位符无遗漏

## Output

Write a single file: `openspec/changes/<change-name>/test-design.md`

## Constraints

- Every AC from proposal.md must appear in the `## 验收范围`
- Do NOT produce any evaluation or checklist JSON
- If the codebase has existing test patterns, follow them
- **禁止**手工拼接或猜测测试文件路径；所有 `测试文件` 列的值 MUST 来自 `test_resolve_paths` MCP 返回值（或明确标注为不可测试并说明原因）

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

## Language

All narrative content in the output test-design.md SHALL be written in Chinese (简体中文).

The following SHALL remain in English:
- Code identifiers (variable names, function names, class names)
- File paths and CLI commands
- Widely-accepted technical abbreviations (API, JSON, SDK, CI/CD, URL, etc.)
- Framework names and test tool names
