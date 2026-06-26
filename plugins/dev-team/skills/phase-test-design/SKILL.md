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

If a change name is provided, use it. Otherwise call `mcp__plugin_dev-team_dev-team__change_list` to get active changes and prompt user to select.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__phase_next(change=<name>, workflow_type="requirement")`. If `result.next_phase` is not `test-design`, stop — prior phase gates have not passed.

### 3. P→E Loop

**3a. Planner:**
```
Agent({
  description: "Write test-design.md",
  subagent_type: result.planner.agent_type,
  prompt: result.planner.prompt
})
```

**3b. Evaluator:**
```
Agent({
  description: "Evaluate test-design.md",
  subagent_type: result.evaluator.agent_type,
  prompt: result.evaluator.prompt
})
```

**3c. Verdict:** Read latest phase "test-design" entry from eval.json. If "fail", redo Planner with failed items, then Evaluator. Loop max 5x.

### 4. Report

Show verdict, pass/total, and notes.
