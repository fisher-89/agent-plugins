---
name: phase-dev-proposal
description: |
  DESIGN phase (P→E): dev-proposal-planner reads proposal.md + test-design.md, writes design.md + tasks.md, then dev-proposal-evaluator checks with static checklist.
  Loops on fail until all required checklist items pass.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Dev proposal phase — Planner writes design.md + tasks.md, Evaluator checks them.

## Usage

```
/dev-team:phase-dev-proposal [change-name]
```

## Process

### Step 1: Detect active change

If a change name is provided, use it. Otherwise find the active change.

### Step 2: Verify prerequisites

Check that `openspec/changes/<name>/phases/proposal.md` and `openspec/changes/<name>/phases/test-design.md` exist. If not, direct user to run prior phases first.

### Step 3: Check for backtrack marker

Read `openspec/changes/<name>/phases/eval.json` if it exists. Search for entries where `backtrack_to` is `"03-dev-proposal"`. If found, re-run the Evaluator first.

### Step 4: P→E Loop

**4a. Invoke Planner:**
```
Agent({
  description: "Write design.md and tasks.md",
  subagent_type: "dev-proposal-planner",
  prompt: "Write design.md and tasks.md for change '<name>'. Read proposal.md and test-design.md. Follow the template at plugins/dev-team/templates/artifacts/design.md.template. Write to openspec/changes/<name>/phases/design.md and .../tasks.md."
})
```

**4b. Invoke Evaluator:**
```
Agent({
  description: "Evaluate design.md",
  subagent_type: "dev-proposal-evaluator",
  prompt: "Evaluate design.md and tasks.md for change '<name>' against proposal.md. Use your static checklist and append result to eval.json."
})
```

**4c. Check verdict:**
- Read the latest entry for phase "03-dev-proposal" from eval.json
- If verdict is "pass": phase complete
- If verdict is "fail": re-invoke Planner with failed items, re-run Evaluator
- Loop until pass or user interrupts

### Step 5: Report result

Display verdict, pass/total items, and notes.

## DESIGN Phase Pattern (P→E)

- **Planner** (`dev-proposal-planner`, opus, Read/Write): reads proposal.md + test-design.md, writes design.md + tasks.md
- **Evaluator** (`dev-proposal-evaluator`, opus, Read/Write): checks design.md against proposal.md with static checklist, appends to eval.json
- **Loop**: if fail → Planner re-invoked → Evaluator re-runs
- **Backtrack support**: E6 (code-review) can set backtrack_to to "03-dev-proposal"; this skill handles it in Step 3
