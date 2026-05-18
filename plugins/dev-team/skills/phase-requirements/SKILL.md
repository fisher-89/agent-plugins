---
name: phase-requirements
description: |
  DESIGN phase (P→E): requirements-planner writes proposal.md, then requirements-evaluator checks with static checklist.
  Loops on fail until all required checklist items pass. Use this as the first phase of the PGE workflow.
  Replaces: /dev-team:openspec-propose
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Requirements phase — Planner writes proposal.md, Evaluator checks it.

## Usage

```
/dev-team:phase-requirements [change-name]
```

## Process

### Step 1: Detect active change

If a change name is provided, use it. Otherwise infer from conversation context or find the active change.

### Step 2: Check for backtrack marker

Read `openspec/changes/<name>/phases/eval.json` if it exists. Search for entries where `backtrack_to` is `"01-requirements"` and the latest entry for that phase. If found, re-run the Evaluator first:

```
Agent({
  description: "Evaluate proposal.md (backtrack)",
  subagent_type: "requirements-evaluator",
  prompt: "Re-evaluate proposal.md for change '<name>'. A backtrack marker was set. Check against your static checklist and append result to eval.json."
})
```

### Step 3: P→E Loop

Run the Planner → Evaluator loop:

**3a. Invoke Planner:**
```
Agent({
  description: "Write proposal.md",
  subagent_type: "requirements-planner",
  prompt: "Write proposal.md for change '<name>'. Follow the template at plugins/dev-team/templates/artifacts/proposal.md.template. Write to openspec/changes/<name>/phases/proposal.md."
})
```

**3b. Invoke Evaluator:**
```
Agent({
  description: "Evaluate proposal.md",
  subagent_type: "requirements-evaluator",
  prompt: "Evaluate proposal.md for change '<name>'. Use your static checklist and append result to eval.json."
})
```

**3c. Check verdict:**
- Read the latest entry for phase "01-requirements" from eval.json
- If verdict is "pass": phase complete, proceed
- If verdict is "fail": re-invoke Planner with failed items from the eval entry, then re-run Evaluator
- Loop until pass or until user interrupts

### Step 4: Report result

Display the Evaluator's verdict, pass/total items, and any notes.

## DESIGN Phase Pattern (P→E)

- **Planner** (`requirements-planner`, opus, Read/Write): writes proposal.md artifact
- **Evaluator** (`requirements-evaluator`, opus, Read/Write): checks with static checklist, appends to eval.json
- **Loop**: if fail → Planner re-invoked with failed items → Evaluator re-runs
- **No Generator**: the Planner IS the producer for DESIGN phases
