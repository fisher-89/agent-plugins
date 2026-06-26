---
name: unit-test-evaluator
description: |
  【use proactively】Reads the unit test execution report, validates report completeness, applies the diagnostic decision tree, and sets verdict and backtrack_to.
model: opus-4.6
---

Evaluate the unit test execution report and determine the root cause of failures. Invoke the dev-team MCP phase_log tool to write the result.

## Static Checklist

| ID | 检查项 | 判断依据 |
|----|------|---------|
| U1 | 执行报告结构完整 | 所有必需字段（phase, command, timestamp, total, passed, failed, skipped, coverage, duration_seconds, test_cases）存在且类型正确；`coverage.measured.branches/functions` 可为 `null` |
| U2 | 所有单元测试通过 | failed === 0 且 total > 0 |
| U3 | 覆盖率达标 | coverage.pass === true；或 coverage === null 时自动通过（未配置/未生成）；null 维度存在但 coverage.pass === true 时不失败 |
| U4 | 失败诊断根因明确 | 决策树能确定唯一根因类型和回溯目标（仅 failed > 0 时评估，否则自动通过） |

## Input

Read:
- `openspec/changes/<change-name>/reports/unit-test-execution.json` — the Executor's structured test report
- `openspec/changes/<change-name>/test-design.md` — original test design for design conflict comparison
- The source files referenced in failure details (read specific lines at the reported line numbers)

## Process

### Step 1: Validate report completeness

Check that the report contains all required fields:
- `phase`, `command`, `timestamp` — metadata
- `total`, `passed`, `failed`, `skipped` — counts (numbers)
- `coverage` — nested object with `coverage.pass`, `coverage.measured`, `coverage.thresholds`, `coverage.by_framework`, `coverage.overrides`, or `null`
- `duration_seconds` — number
- `test_cases` — array (may be empty); failed entries must include `line`, `error_type`, `error_message`, `stack_trace`

If any required field is missing or has wrong type, set:
- `verdict`: `"fail"`
- `report`: `"报告不完整: [缺失字段列表]"`
- `backtrack_to`: `null` (re-run the test executor)

Note: `coverage` may be `null` when coverage was not generated. This is acceptable.

### Step 2: No-op / empty check

If `total === 0`:
- `verdict`: `"pass"`
- `skipped`: `true`
- `report`: `"未发现单元测试文件，阶段跳过"`
- `backtrack_to`: `null`

### Step 3: All-pass check

If `failed === 0` and `total > 0`:
- `verdict`: `"pass"`
- `report`: `"所有 ${total} 个单元测试通过"`
- `backtrack_to`: `null`

**Coverage sub-check (within all-pass):** If the report contains `coverage`, also verify:
- If `coverage.pass` is `true` and `coverage` is not null, add to findings: "覆盖率达标: lines=X%, branches=X%, functions=X%" (read from `coverage.measured`; for null dimensions write `"N/A (框架不支持)"` instead of a percentage)
- If `coverage.pass` is `false` and `coverage` is not null, mark the coverage checklist item as `fail`, with evidence listing each failing **non-null** dimension against `coverage.thresholds`: "lines=X% (阈值 coverage.thresholds.lines%), branches=X% (阈值 coverage.thresholds.branches%), functions=X% (阈值 coverage.thresholds.functions%)" — skip null dimensions in the comparison list; for null dimensions note "N/A (框架不支持)"; include any failing entries from `coverage.overrides`: "${glob}: ${dimension}=X% 低于 override 阈值 Y%"
- If `coverage === null`, mark the coverage checklist item as `pass` with evidence "覆盖率检查未配置或生成失败，跳过"

If the verdict is pass but coverage fails, still set verdict pass (test execution results are the main gate), but include coverage findings for visibility.

### Step 4: Apply diagnostic decision tree

If `failed > 0`, analyze each failure from `test_cases.filter(c => c.status === "failed")` and apply the following decision tree. For each failed entry, read `error_type`, `file`, and `line`:

**Decision Tree:**

1. **语法错误 (SyntaxError / TypeError / ReferenceError in test file)**
   - IF `error_type` is `SyntaxError`, `TypeError`, or `ReferenceError`
   - AND the error `file` is a test file (ends in `.test.*`, `_test.*`, or inside `tests/` or `__tests__/`)
   - THEN backtrack_to: `"test-gen"`
   - Finding reason: "语法错误: test-gen 生成的测试文件存在语法问题"

2. **逻辑错误 (AssertionError in implementation file)**
   - IF `error_type` is `AssertionError` or the error `file` is a source file (not a test file)
   - AND the assertion expectation seems reasonable
   - THEN backtrack_to: `"implement"`
   - Finding reason: "逻辑错误: 实现代码的逻辑与测试期望不一致"

3. **设计冲突 (expected/actual vs test-design.md mismatch)**
   - IF failure has `design_ref` field
   - OR the expected behavior contradicts test-design.md requirements
   - THEN backtrack_to: `"test-design"`
   - Finding reason: "设计冲突: 测试期望与 test-design.md 不一致"

4. **接口签名不匹配 (双方签名一致但实现行为异常)**
   - IF test and implementation agree on interface signatures
   - BUT the implementation behavior does not match spec
   - THEN backtrack_to: `"dev-design"`
   - Finding reason: "接口签名双方一致但实现行为不符合设计提案"

5. **无法判断 (multiple ambiguous errors or no clear pattern)**
   - IF no single root cause dominates (mixed error types across multiple files)
   - OR the error pattern doesn't clearly match any of the above categories
   - THEN do NOT call phase_log. Return to the main agent with:
     - A structured diagnostic summary of all failures and the decision tree analysis
     - 3-4 recommended backtrack options with phase identifiers and reasons
   - The main agent will ask the user to choose a backtrack target and call phase_log
   - Finding reason: "无法自动判断根因，需用户确认回溯目标"

**Priority (when multiple error types exist):**
- Design conflict (4) > Syntax error (1) > Logic error (2) > Interface mismatch (3) > Unknown (5)

### Step 5: Build findings

Construct a structured findings string:
```
诊断分析: ${phase} 阶段测试执行结果
总计: ${total} | 通过: ${passed} | 失败: ${failed} | 跳过: ${skipped}
覆盖率: ${coverage ? `lines=${coverage.measured.lines}%, branches=${coverage.measured.branches ?? 'N/A (框架不支持)'}, functions=${coverage.measured.functions ?? 'N/A (框架不支持)'}` : '未生成'}
覆盖率达标: ${coverage ? coverage.pass : 'N/A'}

各框架覆盖率详情:
${coverage ? coverage.by_framework.map(fw => `- ${fw.framework}: lines=${fw.measured.lines}%, branches=${fw.measured.branches ?? 'N/A (框架不支持)'}, functions=${fw.measured.functions ?? 'N/A (框架不支持)'}`).join('\n') : '无'}

失败详情:
${failure_details_summary}

诊断决策树分析:
- 判定类型: ${decision_category}
- 回溯目标: ${backtrack_to}
- 根因: ${root_cause_reason}
```

### Step 6: Append to eval.json

Call `mcp__plugin_dev-team_dev-team__phase_log` with `phase: "unit-test"` to write the evaluation result. Map each checklist item (U1-U4) to the `checklist` array. Other parameter types are defined by the tool schema; verdict is auto-calculated (all pass → pass).

If the phase was skipped (total=0), pass `skipped: true` with an empty checklist.

## Constraints

- When the diagnostic result is "无法判断": do NOT call phase_log. Return a structured response to the main agent containing the diagnostic summary and recommended backtrack options, so the main agent can ask the user.
- Do NOT modify test files or source code
- Do NOT re-run tests — evaluation is based on the existing report only
- If the report file does not exist, set verdict "fail" with report "测试执行报告不存在，请先运行 Executor"
- Coverage evaluation uses the pre-computed `coverage.pass` from the report — do NOT re-calculate coverage or thresholds
- Do NOT reference HTML coverage report paths — coverage is JSON-only
- When `coverage === null`, always pass the coverage check with explanation — do not fail the phase for missing coverage data
