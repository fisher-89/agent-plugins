---
name: phase-acceptance
description: |
  EVALUATOR-ONLY phase (E only): acceptance-evaluator traces requirements from proposal.md through the codebase.
  No Planner, no Generator — the evaluation IS the work. Can set backtrack_to to "01-requirements".
  This is the final PGE phase before archive.
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

## Process

### Step 1: Detect active change

If a change name is provided, use it. Otherwise find the active change.

### Step 2: Verify prerequisites

Check that `openspec/changes/<name>/phases/proposal.md` and `tasks.md` exist.

### Step 3: Run Evaluator (once, no loop)

EVALUATOR-ONLY phases run the Evaluator once. If the verdict is "fail" with backtrack_to set, the user must manually invoke the target phase.

```
Agent({
  description: "Acceptance evaluation",
  subagent_type: "acceptance-evaluator",
  prompt: "Perform acceptance evaluation for change '<name>'. Read proposal.md from openspec/changes/<name>/phases/. Trace every acceptance criterion through the codebase. Check for scope creep and requirement gaps. Use your static checklist and append result to eval.json."
})
```

### Step 4: Report result

Display verdict, pass/total items, and notes.

If backtrack_to is set to "01-requirements":
- Inform user: "Acceptance found unmet requirements. Run `/dev-team:phase-requirements` to re-evaluate."
- Do NOT automatically loop — the user decides when to backtrack.

## EVALUATOR-ONLY Pattern (E)

- **Evaluator** (`acceptance-evaluator`, opus, Read/Write/Grep/Glob/Bash): inspects full codebase against proposal.md, appends to eval.json
- **No Planner, no Generator** — the evaluation IS the work
- **Runs once** — no automatic loop
- **Backtrack**: can set backtrack_to to "01-requirements" (unmet acceptance criteria)
- **Final PGE phase** — after this passes, the change is ready for archive
