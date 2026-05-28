---
name: phase-dev-proposal
description: |
  DESIGN phase (P→E): dev-proposal-planner writes design.md + tasks.md, then evaluator checks.
  Loops on fail until pass.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Dev proposal phase — Planner writes design.md + tasks.md, Evaluator checks.

## Usage

```
/dev-team:phase-dev-proposal [change-name]
```

## Steps

### 1. Parse change name
If a name is provided, use it. Otherwise run `openspec list --json` and prompt user to select.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__eval_check` with change="<name>" and phase="03-dev-proposal". If `passed` is false, stop — prior phase gates have not passed.

### 3. Check backtrack
Read eval.json for `backtrack_to` = "03-dev-proposal". If found, run Evaluator first.

### 4. P→E Loop

**4a. Planner:**
```
Agent({
  description: "Write design.md and tasks.md",
  subagent_type: "dev-proposal-planner",
  prompt: "Write design.md and tasks.md for change '<name>'."
})
```

**4b. Evaluator:**
```
Agent({
  description: "Evaluate design.md",
  subagent_type: "dev-proposal-evaluator",
  prompt: "Evaluate design.md and tasks.md for change '<name>' against proposal.md. Append result to eval.json."
})
```

**4c. Verdict:** Read latest phase "03-dev-proposal" entry from eval.json. If "fail", redo Planner with failed items, then Evaluator. Loop max 5x.

### 5. Report
Show verdict, pass/total, and notes.
