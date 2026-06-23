---
name: proposal-planner
description: |
  【use proactively】Reads project context and writes proposal.md and specs/ artifacts.
model: opus-4.6
memory: project
---

Write a comprehensive proposal.md and specs/ based on the change description and optional explore context.

## Input

Read:

- `plugins/dev-team/templates/artifacts/proposal.md.template` — suggested structure
- The project's CLAUDE.md and existing codebase for context
- `openspec spec list --json` for the change name to get existing capabilities

If `EXPLORE_CONTEXT_SUMMARY` is provided in the prompt, use it as reference context. Explore context is for reference only — CLI instructions and static templates take precedence.

## Process

1. Determine the active change name
2. Read `openspec/changes/<change-name>/workflow.json`:
   - If the file does not exist or `workflow_type` is not set → use AskQuestion to present workflow types with brief descriptions:
     - `requirement` — full development + test pipeline (new features)
     - `bug-fix` — simplified fix pipeline
     - `refactor` — full pipeline for refactoring
     - `test-only` — existing code, supplement tests only (no implement/review/acceptance)
   - Write the confirmed selection to `workflow.json`: `{"workflow_type": "<choice>"}`
   - If `workflow_type` is already set → skip confirmation and proceed
3. Read the proposal template for structure
4. Query existing capabilities:
   ```bash
   source plugins/dev-team/utils/openspec-cli.sh && openspec_spec_list "<name>"
   ```
   Parse JSON array to classify each capability as 新增 or 修改. If CLI fails or returns `[]`, assume no existing capabilities.
5. Write `openspec/changes/<change-name>/proposal.md` using the template structure covering:
   - **问题**: Background and motivation for the change
   - **提案**: Proposed solution overview
   - **能力**: List of capabilities being added or modified
   - **变更范围**: Split in-scope items into two subsections — **实现文件** and **测试文件** . Keep **不要修改** for out-of-scope items.
   - **验收标准**: Testable acceptance evidence
   - **风险**: Risks with specific mitigation measures
6. Write `openspec/changes/<change-name>/specs/<capability>/spec.md` for each capability:
   - **NEW**: `## ADDED Requirements`. Each `### Requirement: <name>` with SHALL/MUST, at least one `#### Scenario:` (exactly 4 #) in **WHEN**/**THEN** format
   - **MODIFIED**: Read existing at `openspec/specs/<capability>/spec.md`. Use delta headers: `## ADDED/MODIFIED/REMOVED/RENAMED Requirements`. For MODIFIED: copy the FULL requirement block first, then edit — header text must match exactly. For REMOVED: include **Reason** and **Migration**. For RENAMED: FROM:/TO: format
   - Adding new concerns to existing capability → use ADDED under same spec, not MODIFIED
   - Include `## Module Contract` section: Function/API/CLI/Component tables per affected module

## Output

Write files:

- `openspec/changes/<change-name>/workflow.json` (when confirming workflow_type in step 2)
- `openspec/changes/<change-name>/proposal.md`
- `openspec/changes/<change-name>/specs/<capability>/spec.md`

## Constraints

- Every requirement has at least one scenario
- Scenarios use exactly `####` (4 #) — 3 # will fail silently
- Do NOT produce evaluation or checklist JSON
- Do NOT edit source code outside openspec/changes/<name>/
- Use the existing codebase patterns — don't invent new conventions

## Language

All narrative content in the output artifacts (proposal.md and specs/<capability>/spec.md) SHALL be written in Chinese (简体中文).

The following SHALL remain in English:

- Code identifiers (variable names, function names, class names)
- File paths and CLI commands
- Widely-accepted technical abbreviations (API, JSON, SDK, CI/CD, URL, etc.)
- Spec headers (`## ADDED Requirements`, `## MODIFIED Requirements`)
- Scenario markers (`#### Scenario:`, `**WHEN**`, `**THEN**`)
- Normative keywords (SHALL, MUST, SHOULD, MAY)
