---
name: phase-acceptance
description: |
  EVALUATOR-ONLY phase (E only): acceptance-evaluator traces requirements from proposal.md
  through the codebase. No Planner, no Generator. Runs once. Can set backtrack_to to "proposal".
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Acceptance phase — Evaluator traces requirements through codebase for final verification.

## Usage

```
/dev-team:phase-acceptance [change-name]
```

## Steps

### 1. Parse change name

If a change name is provided, use it. Otherwise call `mcp__plugin_dev-team_dev-team__change_list` to get active changes and prompt user to select.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__phase_next(change=<name>, workflow_type="requirement")`. If `result.done` is not `true`, stop — prior phase gates have not passed.

### 3. Evaluate (once)

```
Agent({
  description: "Acceptance evaluation",
  subagent_type: "dev-team:acceptance-evaluator",
  prompt: "Perform acceptance evaluation for change '<name>'. Append result to eval.json."
})
```

### 4. Check backtrack

Read latest phase "acceptance" entry from eval.json. If `backtrack_to` is "proposal", inform user: "Acceptance found unmet requirements. Run `/dev-team:phase-proposal` to re-evaluate."

### 5. Report

Show verdict, pass/total, notes, and backtrack suggestion if applicable.
