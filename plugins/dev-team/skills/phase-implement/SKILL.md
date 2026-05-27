---
name: phase-implement
description: |
  EXECUTION phase (G→E+AUTO): generator writes code, AUTO static-check, then evaluator inspects git diff. Loops on fail. Replaces: /dev-team:openspec-apply-change
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

## Steps

### 1. Parse change name
If a name is provided, use it. Otherwise run `openspec list --json` and prompt user to select.

### 2. Gate check
```bash
dev-team eval-check --change "<name>" --phase 05-implement
```
Stop if exit != 0.

### 3. G→E Loop

**3a. Generator:**
```
Agent({
  description: "Implement pending tasks",
  subagent_type: "implementation-generator",
  prompt: "Implement pending tasks for change '<name>'."
})
```

**3b. AUTO: Static check:**
```bash
python plugins/dev-team/utils/lint-runner.py --change "<name>" --project-root . --save-report
```

**3c. Evaluator:**
```
Agent({
  description: "Evaluate implementation",
  subagent_type: "implementation-evaluator",
  prompt: "Evaluate implementation code for change '<name>' against design.md. Append result to eval.json."
})
```

**3d. Verdict:** Read latest phase "05-implement" entry from eval.json. If "fail", redo Generator with failed items, then AUTO + Evaluator. Loop max 5x.

### 4. Report
Show verdict, pass/total, and notes.
