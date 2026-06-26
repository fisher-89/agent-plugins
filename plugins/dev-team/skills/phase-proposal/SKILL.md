---
name: phase-proposal
description: proposal-planner writes proposal.md + specs/, evaluator checks. Loops on fail.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Proposal phase — Planner writes proposal.md + specs/ with P→E loop.

**Input**: Optionally specify a change name (kebab-case), OR a description of what the user wants to build.

## Steps

### Step 0: Resolve the target change

Call `mcp__plugin_dev-team_dev-team__change_list()` to get active changes.

**Decision tree based on user input and change list:**

1. **User provided a parameter that exactly matches an existing change name** → use that change, skip to Step 2.
2. **User provided a description (not an exact change name match)**:
   - If **no active changes exist** → treat as a new change. Derive a kebab-case name and proceed to Step 1.
   - If **active changes exist**, judge whether the description semantically relates to an existing change (e.g., the description refines, extends, or refers to the same topic as an existing change name).
     - **Confident it matches an existing change** → use that change, skip to Step 2.
     - **Confident it is unrelated to any existing change** → treat as a new change. Derive a kebab-case name and proceed to Step 1.
     - **Uncertain** → use `AskUserQuestion` to present the potentially matching change(s) plus a "Create a new change" option. Let the user decide.
3. **No parameter provided AND exactly one active change exists** → auto-select that change, skip to Step 2.
4. **No parameter provided AND multiple active changes exist** → use `AskUserQuestion` to present the list of active changes (plus an "Other — describe a new change" option). If the user picks an existing change, skip to Step 2. If the user describes a new change, derive a kebab-case name and proceed to Step 1.
5. **No parameter provided AND zero active changes exist** → use `AskUserQuestion` (open-ended, no preset options) to ask: "What change do you want to work on? Describe what you want to build or fix." Derive a kebab-case name from the response and proceed to Step 1.

**IMPORTANT**: Do NOT proceed without a resolved change name.

### Step 1: Create the change directory

Only reached when starting a **new** change (not resuming an existing one).

```bash
openspec new change "<name>"
```

This creates a scaffolded change in the planning home resolved by the CLI with `.openspec.yaml`.

### Step 2: Confirm workflow type

Check if `openspec/changes/<name>/workflow.json` exists:

- **Already exists** (e.g., created by a workflow skill) → skip to Step 2.
- **Does not exist** (direct phase invocation) → use `AskUserQuestion` to confirm the workflow type:

  **Options:**
  - `requirement` — full development + test pipeline (new features)
  - `bug-fix` — simplified fix pipeline
  - `refactor` — full pipeline for refactoring
  - `test-only` — existing code, supplement tests only (no implement/review/acceptance)

  After confirmation, write `openspec/changes/<name>/workflow.json`:

  ```json
  {"workflow_type": "<choice>"}
  ```

### Step 3. Gate check

Read `workflow_type` from `openspec/changes/<name>/workflow.json`. Call `mcp__plugin_dev-team_dev-team__phase_next(change=<name>)`. If `result.next_phase` is not `proposal`, stop — prior phase gates have not passed.

### Step 4. P→E Loop

**4a. Planner:**

```
Agent({
  description: "Write proposal.md and specs/",
  subagent_type: "dev-team:proposal-planner",
  prompt: "Write proposal.md and specs/ for change '<name>'."
})
```

If explore context was found in Step 1, append to prompt: `EXPLORE_CONTEXT_SUMMARY: <context>`.

**4b. Evaluator:**

```
Agent({
  description: "Evaluate proposal.md",
  subagent_type: "dev-team:proposal-evaluator",
  prompt: "Evaluate proposal.md for change '<name>' against checklist."
})
```

**4c. Verdict:** Read latest phase "proposal" entry from eval.json. If "fail", redo Steps 3a-3b with failed items. Loop max 5x.

### Step 5. Report

Show verdict, pass/total, notes, and specs/ files list.
