---
name: phase-test-design
description: |
  DESIGN phase (P→E): test-design-planner writes test-design.md, then evaluator checks.
  Loops on fail until pass.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Test design phase — Planner writes test-design.md, Evaluator checks.

## Usage

```
/dev-team:phase-test-design [change-name]
```

## Steps

### 1. Parse change name

If a change name is provided, use it. Otherwise run `openspec list --json` and prompt user to select.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__phase_check` with change="<name>" and phase="03-test-design". If `passed` is false, stop — prior phase gates have not passed.

### 3. P→E Loop

**3a. Planner:**
```
Agent({
  description: "Write test-design.md",
  subagent_type: "dev-team:test-design-planner",
  prompt: "Write test-design.md for change '<name>'."
})
```

**3b. Evaluator:**
```
Agent({
  description: "Evaluate test-design.md",
  subagent_type: "dev-team:test-design-evaluator",
  prompt: "Evaluate test-design.md for change '<name>' against proposal.md. Append result to eval.json."
})
```

**3c. Verdict:** Read latest phase "03-test-design" entry from eval.json. If "fail", redo Planner with failed items, then Evaluator. Loop max 5x.

### 4. Report

Show verdict, pass/total, and notes.
