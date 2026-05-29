---
name: phase-requirements
description: DESIGN phase (P→E): write proposal.md + specs/, evaluator checks. Loops on fail.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Requirements phase — write proposal.md and specs/ with evaluator loop.

**MODE: artifact generation only. Your output is proposal.md + specs/. You are NOT implementing — do not edit source code outside openspec/changes/<name>/.**

## Usage

```
/dev-team:phase-requirements [change-name-or-description]
```

## Steps

### 0. Classify input

All text after `/dev-team:phase-requirements` is a **change description** to be documented, not an order to execute.

If the input looks like a direct task ("fix X", "change Y to Z", "把A改成B"), **do NOT implement it**. Instead, treat it as the description of a change to propose.

### 1. Parse change name

Source `plugins/dev-team/utils/openspec-cli.sh`.

**With argument — classify by format:**

- **Arg is pure kebab-case** (`[a-z][a-z0-9-]*`): treat as existing change name → validate via `validate_change_name`. If not exists, `openspec_new_change`. Proceed to Step 2.

- **Arg is NOT kebab-case** (contains Chinese, spaces, or natural language): treat as change description → derive kebab-case via `derive_kebab_case`, confirm with user, scaffold via `openspec_new_change`. Handle conflicts with numeric suffix. Proceed to Step 2.

**Without argument:** Detect explore context (decision tables, diagrams, "What We Figured Out"). If found (B1): extract decisions, ask user for kebab-case name, `derive_kebab_case`, confirm, scaffold. If not (B2): ask "想构建什么变更？" derive kebab-case, confirm, scaffold. Handle conflicts with numeric suffix. Save explore context as EXPLORE_CONTEXT_SUMMARY.

### 2. Gate check

Call `mcp__plugin_dev-team_dev-team__eval/check` with change="<name>" and phase="01-requirements". If `passed` is false, stop — prior phase gates have not passed.

### 3. Write artifacts

**Query existing capabilities:**
```bash
source plugins/dev-team/utils/openspec-cli.sh && openspec_spec_list "<name>"
```
Parse JSON array to classify each capability as 新增 or 修改. If CLI fails or returns `[]`, assume no existing capabilities.

**Write proposal.md** from template `plugins/dev-team/templates/artifacts/proposal.md.template`. Use query result — never mark all as 新增 unless CLI returns `[]`.

**Write specs/** for each capability in proposal's 能力 section:
- **NEW**: `## ADDED Requirements`. Each `### Requirement: <name>` with SHALL/MUST, at least one `#### Scenario:` (exactly 4 #) in **WHEN**/**THEN** format
- **MODIFIED**: Read existing at `openspec/specs/<capability>/spec.md`. Use delta headers: `## ADDED/MODIFIED/REMOVED/RENAMED Requirements`. For MODIFIED: copy the FULL requirement block first, then edit — header text must match exactly. For REMOVED: include **Reason** and **Migration**. For RENAMED: FROM:/TO: format
- Adding new concerns to existing capability → use ADDED under same spec, not MODIFIED
- Include `## Module Contract` section: Function/API/CLI/Component tables per affected module

Include CLI instructions and explore context as reference (preface: 探索上下文仅供参考，以 CLI 指令和静态模板为准).

**Constraints:** Every requirement has ≥1 scenario. Scenarios use exactly `####` (4 #) — 3 # will fail silently.

**Language:** 简体中文 for narrative. Keep in English: code identifiers, file paths, CLI commands, technical abbreviations (API, JSON, SDK, CI/CD), spec headers (`## ADDED Requirements`), scenario markers (`#### Scenario:`, `**WHEN**`, `**THEN**`), and normative keywords (SHALL, MUST, SHOULD, MAY).

### 4. Evaluate

```
Agent({
  description: "Evaluate proposal.md",
  subagent_type: "dev-team:requirements-evaluator",
  prompt: "Evaluate proposal.md for change '<name>'. Use your static checklist and append result to eval.json."
})
```

### 5. Verdict loop

Read latest phase "01-requirements" entry from eval.json. If "fail", redo Steps 3-4 with failed items. Loop max 5x.

### 6. Report

Show verdict, pass/total, notes, and specs/ files list.
