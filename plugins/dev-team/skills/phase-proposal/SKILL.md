---
name: phase-proposal
description: DESIGN phase (P→E): proposal-planner writes proposal.md + specs/, evaluator checks. Loops on fail.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Proposal phase — Planner writes proposal.md + specs/ with P→E loop.

**MODE: artifact generation only. Your output is proposal.md + specs/. You are NOT implementing — do not edit source code outside openspec/changes/<name>/.**

## Input:

The user's request should include a change name (kebab-case) OR a description of what they want to build.

## Steps

### Step 0: **If no clear input provided, ask what they want to build**

Use the **AskUserQuestion tool** (open-ended, no preset options) to ask:

> "What change do you want to work on? Describe what you want to build or fix."

From their description, derive a kebab-case name (e.g., "add user authentication" → `add-user-auth`).

**IMPORTANT**: Do NOT proceed without understanding what the user wants to build.

### Step 1: Create the change directory\*\*

```bash
openspec new change "<name>"
```

This creates a scaffolded change in the planning home resolved by the CLI with `.openspec.yaml`.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__phase_check` with change="<name>" and phase="01-proposal". If `passed` is false, stop — prior phase gates have not passed.

### 3. P→E Loop

**3a. Planner:**

```
Agent({
  description: "Write proposal.md and specs/",
  subagent_type: "dev-team:proposal-planner",
  prompt: "Write proposal.md and specs/ for change '<name>'."
})
```

If explore context was found in Step 1, append to prompt: `EXPLORE_CONTEXT_SUMMARY: <context>`.

**3b. Evaluator:**

```
Agent({
  description: "Evaluate proposal.md",
  subagent_type: "dev-team:proposal-evaluator",
  prompt: "Evaluate proposal.md for change '<name>' against checklist."
})
```

**3c. Verdict:** Read latest phase "01-proposal" entry from eval.json. If "fail", redo Steps 3a-3b with failed items. Loop max 5x.

### 4. Report

Show verdict, pass/total, notes, and specs/ files list.
