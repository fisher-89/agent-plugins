---
name: phase-acceptance
description: |
  EVALUATOR-ONLY phase (E only): acceptance-evaluator traces requirements from proposal.md
  through the codebase. No Planner, no Generator. Runs once. Can set backtrack_to to "01-requirements".
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

If a change name is provided, use it. Otherwise run `openspec list --json` and prompt user to select.

### 2. Gate check

```bash
dev-team eval-check --change "<name>" --phase 09-acceptance
```
Stop if exit != 0.

### 3. Evaluate (once)

```
Agent({
  description: "Acceptance evaluation",
  subagent_type: "acceptance-evaluator",
  prompt: "Perform acceptance evaluation for change '<name>'. Append result to eval.json."
})
```

### 4. Check backtrack

Read latest phase "09-acceptance" entry from eval.json. If `backtrack_to` is "01-requirements", inform user: "Acceptance found unmet requirements. Run `/dev-team:phase-requirements` to re-evaluate."

### 5. Report

Show verdict, pass/total, notes, and backtrack suggestion if applicable.
