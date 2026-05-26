---
name: phase-implement
description: |
  EXECUTION phase (G→E+AUTO): implementation-generator writes implementation code to disk (git diff IS the artifact).
  AUTO static-check runs automatically after code generation, before evaluation.
  Test execution is NOT part of this phase — it runs independently as 06-unit-test.
  Then implementation-evaluator checks via git diff against design.md. Loops on fail.
  Static-check failure loops back to Generator (does NOT trigger unit-test phase).
  Replaces: /dev-team:openspec-apply-change
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Implementation phase — Generator writes code, AUTO phases run, Evaluator inspects.

## Usage

```
/dev-team:phase-implement [change-name]
```

## Process

### Step 1: Detect active change

If a change name is provided, use it. Otherwise find the active change.

### Step 2: Verify prerequisites

Check that `openspec/changes/<name>/phases/design.md` and `openspec/changes/<name>/phases/tasks.md` exist. If not, direct user to run prior phases.

### Step 3: G→E Loop

**3a. Invoke Generator:**
```
Agent({
  description: "Implement pending tasks",
  subagent_type: "implementation-generator",
  prompt: "Implement pending tasks for change '<name>'. Read design.md and tasks.md from openspec/changes/<name>/phases/. Write implementation code directly to disk. The git diff IS the artifact — no JSON reports. Mark completed tasks as [x] in tasks.md."
})
```

**3b. Read Path Validation (automatic after Generator completes):**
Before proceeding to AUTO, validate the Generator's Read tool calls against the tests/ directory blacklist:

```bash
# Check that no test files were read by the implementation-generator
# The implementation-generator should NOT have read files in tests/, __tests__/, or test/ directories
# If violations found, record in eval.json findings and re-invoke Generator with warning
```

If Read violations are found:
- Record a finding in eval.json with phase_suffix "read-validation"
- Re-invoke Generator with specific instructions to avoid reading test files
- If violations persist after 3 attempts, set verdict "fail" with findings listing the violations

**3c. AUTO: Static Check (automatic after Generator completes):**
Run lint and type checking on changed files:
```bash
python plugins/dev-team/utils/lint-runner.py --change "<name>" --project-root . --save-report
```

**IMPORTANT: Static check failure does NOT trigger unit-test phase.** If static check fails, the implementation has syntax or type errors that must be fixed first. The unit-test phase (06-unit-test) is a separate phase that runs independently. This implementation phase loops back to the Generator on static-check failure.

**3d. Invoke Evaluator:**
```
Agent({
  description: "Evaluate implementation",
  subagent_type: "implementation-evaluator",
  prompt: "Evaluate implementation code for change '<name>' against design.md. Run git diff to inspect the Generator's output. Check the static check report if available. Use your static checklist and append result to eval.json."
})
```

**3e. Check verdict:**
- Read the latest entry for phase "05-implement" from eval.json
- If verdict is "pass": phase complete
- If verdict is "fail": re-invoke Generator with failed items, re-run AUTO phases, re-run Evaluator
- Loop until pass or user interrupts

### Step 4: Report result

Display verdict, pass/total items, and notes.

## EXECUTION Phase Pattern (G→E with AUTO static-check)

- **Generator** (`implementation-generator`, sonnet, Read/Write/Grep/Glob/Bash): writes code to disk
- **AUTO**: static-check (lint/type) only — test execution is NOT part of this phase
- **Important**: static-check failure does NOT trigger unit-test (06-unit-test) phase. The implement phase loops back to its own Generator.
- **Evaluator** (`implementation-evaluator`, opus, Read/Write/Bash): inspects git diff + design.md, appends to eval.json
- **Loop**: if fail → Generator re-invoked → AUTO static-check re-run → Evaluator re-runs
