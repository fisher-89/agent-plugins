---
name: phase-implement
description: |
  EXECUTION phase (G→E): generator writes code, then evaluator inspects git diff. Loops on fail. Replaces: /dev-team:openspec-apply-change
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Implementation phase — Generator writes code, Evaluator inspects.

## Usage

```
/dev-team:phase-implement [change-name]
```

## Steps

### 1. Parse change name
If a name is provided, use it. Otherwise run `openspec list --json` and prompt user to select.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__phase_check` with change="<name>" and phase="05-implement". If `passed` is false, stop — prior phase gates have not passed.

### 3. G→E Loop

**3a. Generator:**
```
Agent({
  description: "Implement pending tasks",
  subagent_type: "dev-team:implementation-generator",
  prompt: "Implement pending tasks for change '<name>'."
})
```

**3b. Evaluator:**
```
Agent({
  description: "Evaluate implementation",
  subagent_type: "dev-team:implementation-evaluator",
  prompt: "Evaluate implementation code for change '<name>' against design.md. Append result to eval.json."
})
```

**3c. Verdict:** Read latest phase "05-implement" entry from eval.json. If "fail", redo Generator with failed items, then Evaluator. Loop max 5x.

### 4. Report
Show verdict, pass/total, and notes.
