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

## Usage

```
/dev-team:phase-proposal [change-name-or-description]
```

## Steps

### 1. Parse change name

Source `plugins/dev-team/utils/openspec-cli.sh`.

**With argument — classify by format:**

- **Arg is pure kebab-case** (`[a-z][a-z0-9-]*`): treat as existing change name → validate via `validate_change_name`. If not exists, `openspec_new_change`. Proceed to Step 2.

- **Arg is NOT kebab-case** (contains Chinese, spaces, or natural language): treat as change description → derive kebab-case via `derive_kebab_case`, confirm with user, scaffold via `openspec_new_change`. Handle conflicts with numeric suffix. Proceed to Step 2.

**Without argument:** Detect explore context (decision tables, diagrams, "What We Figured Out"). If found: extract decisions, ask user for kebab-case name, `derive_kebab_case`, confirm, scaffold. If not: ask "想构建什么变更？" derive kebab-case, confirm, scaffold. Handle conflicts with numeric suffix. Save explore context as EXPLORE_CONTEXT_SUMMARY.

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
