---
name: phase-code-review
description: |
  EVALUATOR-ONLY phase (E only): code-review-evaluator inspects code diff against design.md.
  No Planner, no Generator. Runs once. Can set backtrack_to to "03-dev-proposal".
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Code review phase — Evaluator inspects code diff for security, coverage, and design consistency.

## Usage

```
/dev-team:phase-code-review [change-name]
```

## Steps

### 1. Parse change name

If a change name is provided, use it. Otherwise run `openspec list --json` and prompt user to select.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__eval_check` with change="<name>" and phase="07-code-review". If `passed` is false, stop — prior phase gates have not passed.

### 3. Evaluate (once)

```
Agent({
  description: "Code review evaluation",
  subagent_type: "code-review-evaluator",
  prompt: "Review code changes for change '<name>'. Append result to eval.json."
})
```

### 4. Check backtrack

Read latest phase "07-code-review" entry from eval.json. If `backtrack_to` is "03-dev-proposal", inform user: "Code review found design deviations. Run `/dev-team:phase-dev-proposal` to re-evaluate."

### 5. Report

Show verdict, pass/total, notes, and backtrack suggestion if applicable.
