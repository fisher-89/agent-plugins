---
name: phase-test-design
description: |
  DESIGN phase (P→E): test-design-planner reads proposal.md and writes test-design.md, then test-design-evaluator checks with static checklist.
  Loops on fail until all required checklist items pass.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Test design phase — Planner writes test-design.md, Evaluator checks it against proposal.md.

## Usage

```
/dev-team:phase-test-design [change-name]
```

## Process

### Step 1: Detect active change

If a change name is provided, use it. Otherwise find the active change.

### Step 2: Verify prerequisite

Check that `openspec/changes/<name>/phases/proposal.md` exists. If not, direct user to run `/dev-team:phase-requirements` first.

### Step 3: P→E Loop

**3a. Invoke Planner:**
```
Agent({
  description: "Write test-design.md",
  subagent_type: "test-design-planner",
  prompt: "Write test-design.md for change '<name>'. Read proposal.md, follow the template at plugins/dev-team/templates/artifacts/test-design.md.template. Write to openspec/changes/<name>/phases/test-design.md."
})
```

**3b. Invoke Evaluator:**
```
Agent({
  description: "Evaluate test-design.md",
  subagent_type: "test-design-evaluator",
  prompt: "Evaluate test-design.md for change '<name>' against proposal.md. Use your static checklist and append result to eval.json."
})
```

**3c. Check verdict:**
- Read the latest entry for phase "02-test-design" from eval.json
- If verdict is "pass": phase complete
- If verdict is "fail": re-invoke Planner with failed items, re-run Evaluator
- Loop until pass or user interrupts

### Step 4: Report result

Display verdict, pass/total items, and notes.

## DESIGN Phase Pattern (P→E)

- **Planner** (`test-design-planner`, opus, Read/Write): reads proposal.md, writes test-design.md
- **Evaluator** (`test-design-evaluator`, opus, Read/Write): checks test-design.md against proposal.md with static checklist, appends to eval.json
- **Loop**: if fail → Planner re-invoked with failed items → Evaluator re-runs
