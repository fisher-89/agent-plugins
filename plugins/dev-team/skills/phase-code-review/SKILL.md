---
name: phase-code-review
description: |
  EVALUATOR-ONLY phase (E only): code-review-evaluator inspects code diff against design.md using a static checklist.
  No Planner, no Generator — the evaluation IS the work. Can set backtrack_to to "03-dev-proposal".
  Integration test review is NOT part of this phase — it runs independently as 08-integration-test.
  Single Evaluator mode: eval.json contains exactly one code-review entry per run.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Code review phase — Evaluator inspects code diff for security, test coverage, and design consistency.

## Usage

```
/dev-team:phase-code-review [change-name]
```

## Process

### Step 1: Detect active change

If a change name is provided, use it. Otherwise find the active change.

### Step 2: Verify prerequisites

Check that `openspec/changes/<name>/phases/design.md` exists. If not, code review proceeds against best-effort context.

### Step 3: Run Evaluator (once, no loop)

EVALUATOR-ONLY phases run the Evaluator once. If the verdict is "fail" with backtrack_to set, the user must manually invoke the target phase.

```
Agent({
  description: "Code review evaluation",
  subagent_type: "code-review-evaluator",
  prompt: "Review code changes for change '<name>'. Read design.md from openspec/changes/<name>/phases/. Inspect git diff and codebase for security, test coverage, and error handling. Use your static checklist and append result to eval.json."
})
```

### Step 4: Report result

Display verdict, pass/total items, and notes.

If backtrack_to is set to "03-dev-proposal":
- Inform user: "Code review found design deviations. Run `/dev-team:phase-dev-proposal` to re-evaluate."
- Do NOT automatically loop — the user decides when to backtrack.

## EVALUATOR-ONLY Pattern (E)

- **Evaluator** (`code-review-evaluator`, opus, Read/Write/Grep/Glob/Bash): inspects code diff + design.md + codebase, appends to eval.json
- **No Planner, no Generator** — the evaluation IS the work
- **Runs once** — no automatic loop
- **Backtrack**: can set backtrack_to to "03-dev-proposal" (design deviations)
