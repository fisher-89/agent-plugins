---
name: phase-implement
description: |
  EXECUTION phase (G→E): implementation-generator writes implementation code to disk (git diff IS the artifact).
  AUTO phases (static-check, test-execution) run automatically after code generation, before evaluation.
  Then implementation-evaluator checks via git diff against design.md. Loops on fail.
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

**3b. AUTO: Static Check (automatic after Generator completes):**
Run lint and type checking on changed files:
```bash
python plugins/dev-team/utils/lint-runner.py --change "<name>" --project-root . --save-report
```

**3c. AUTO: Test Execution (automatic after static check):**
Run the full test suite:
```bash
python plugins/dev-team/utils/test-runner.py --change "<name>" --project-root . --save-report
```

**3d. Invoke Evaluator:**
```
Agent({
  description: "Evaluate implementation",
  subagent_type: "implementation-evaluator",
  prompt: "Evaluate implementation code for change '<name>' against design.md. Run git diff to inspect the Generator's output. Check static and test reports if available. Use your static checklist and append result to eval.json."
})
```

**3e. Check verdict:**
- Read the latest entry for phase "05-implementation" from eval.json
- If verdict is "pass": phase complete
- If verdict is "fail": re-invoke Generator with failed items, re-run AUTO phases, re-run Evaluator
- Loop until pass or user interrupts

### Step 4: Report result

Display verdict, pass/total items, and notes.

## EXECUTION Phase Pattern (G→E with AUTO)

- **Generator** (`implementation-generator`, sonnet, Read/Write/Grep/Glob/Bash): writes code to disk
- **AUTO**: static-check (lint/type) then test-execution (full suite) — no human invocation needed
- **Evaluator** (`implementation-evaluator`, opus, Read/Write/Bash): inspects git diff + design.md, appends to eval.json
- **Loop**: if fail → Generator re-invoked → AUTO re-run → Evaluator re-runs
