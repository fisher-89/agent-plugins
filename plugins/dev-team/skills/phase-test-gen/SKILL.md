---
name: phase-test-gen
description: |
  EXECUTION phase (G→E): test-gen-generator writes test files, then evaluator checks
  via git diff against test-design.md. Loops on fail.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Test generation phase — Generator writes test files, Evaluator inspects.

## Usage

```
/dev-team:phase-test-gen [change-name]
```

## Steps

### 1. Parse change name

If a change name is provided, use it. Otherwise run `openspec list --json` and prompt user to select.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__eval_check` with change="<name>" and phase="04-test-gen". If `passed` is false, stop — prior phase gates have not passed.

### 3. G→E Loop

**3a. Generator:**
```
Agent({
  description: "Generate test files",
  subagent_type: "test-gen-generator",
  prompt: "Generate test files for change '<name>'."
})
```

**3b. Evaluator:**
```
Agent({
  description: "Evaluate generated tests",
  subagent_type: "test-gen-evaluator",
  prompt: "Evaluate generated test code for change '<name>' against test-design.md. Append result to eval.json."
})
```

**3c. Verdict:** Read latest phase "04-test-gen" entry from eval.json. If "fail", redo Generator with failed items, then Evaluator. Loop max 5x.

### 4. Report

Show verdict, pass/total, and notes.
