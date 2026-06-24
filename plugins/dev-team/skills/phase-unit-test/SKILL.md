---
name: phase-unit-test
description: |
  EXECUTION phase (Executor->Evaluator): unit-test-executor (sonnet) runs tests and generates structured report.
  Then unit-test-evaluator (opus) validates the report, applies diagnostic decision tree, and appends to eval.json.
  Includes no-op detection: if no test files exist, the phase is skipped with skipped:true.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Unit test execution phase — Executor runs tests, Evaluator diagnoses failures.

## Usage

```
/dev-team:phase-unit-test [change-name]
```

## Process

### Step 1: Detect active change

If a change name is provided, use it. Otherwise find the active change.

### Step 2: No-op detection

Check if any unit test files exist:

```bash
count=$(find . -type f \( -name "*.test.ts{x}" -o -name "*.test.js{x}" -o -name "*_test.rs" \) 2>/dev/null | wc -l)
count_unit=$(find . -path "*/tests/unit/*" -type f 2>/dev/null | wc -l)
count_dunder=$(find . -path "*/__tests__/*" -type f 2>/dev/null | wc -l)
total=$((count + count_unit + count_dunder))
echo "Unit test files found: $total"
```

If total is 0 (no unit test files found):
- Skip the phase: append a skipped entry to eval.json via MCP:
  ```
  mcp__plugin_dev-team_dev-team__phase_log({change: "<name>", phase: "06-unit-test", report: "No unit tests found, phase skipped (no-op)", items: '[]', backtrack_to: null, skipped: true, findings: "未发现单元测试文件，阶段跳过"})
  ```
- Phase complete.

If total > 0, proceed to the Executor->Evaluator loop.

### Step 3: Executor->Evaluator Loop

**3a. Invoke Executor:**
```
Agent({
  description: "Execute unit tests",
  subagent_type: "dev-team:unit-test-executor",
  model: "sonnet",
  prompt: "Execute unit tests for change '<name>' and produce a structured JSON execution report at openspec/changes/<name>/reports/unit-test-execution.json. Read test-design.md for context then run the appropriate test commands."
})
```

After Executor completes, validate the Read tool calls:

```bash
# Check that no source code files were read by the Executor
# (Executor should only read test files and test-design.md)
```

If Read violations are found:
- Record to eval.json findings and re-invoke the Executor with a warning.
- If violations persist after 3 attempts, set verdict "fail" with findings listing the violations.

**3b. Invoke Evaluator:**
```
Agent({
  description: "Evaluate unit test results",
  subagent_type: "dev-team:unit-test-evaluator",
  prompt: "Evaluate unit test results for change '<name>'. Read the execution report from openspec/changes/<name>/reports/unit-test-execution.json. Validate report completeness, apply diagnostic decision tree, and append result to eval.json."
})
```

**3c. Check verdict:**
- Read the latest entry for phase "06-unit-test" from eval.json
- If verdict is "pass": phase complete
- If verdict is "fail": re-invoke Executor with failed items and evaluator notes, re-run Evaluator
- If backtrack_to is set: inform the user to run the target phase (`/dev-team:phase-<backtrack_target>`)
- Loop until pass or user interrupts

### Step 4: Report result

Display verdict, pass/total items, and notes. If skipped, display "(skipped: no applicable tests)".

## EXECUTION Phase Pattern (Executor->Evaluator)

- **No-op detection**: skill-level file existence check before Executor invocation
- **Executor** (`unit-test-executor`, sonnet, Read/Write/Grep/Glob/Bash): runs tests, writes structured JSON report
- **Read validation**: skill layer checks Executor's Read tool calls against blacklist
- **Evaluator** (`unit-test-evaluator`, opus, Read/Write/Bash): validates report, applies diagnostic decision tree, appends to eval.json
- **Loop**: if fail -> Executor re-invoked -> Evaluator re-runs
- **Backtrack**: Evaluator can set backtrack_to for root cause recovery (test-gen, implement, test-design, dev-design)
- **AskUserQuestion**: Used when the diagnostic tree cannot determine root cause (timeout 5 min, fallback to dev-design)
