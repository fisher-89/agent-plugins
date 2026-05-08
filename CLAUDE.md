# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is **wps-claude-plugin**, a custom Claude Code plugin designed to enhance development workflow with OpenSpec integration.

## Repository Structure

- `.claude-plugin/plugin.json` — Plugin manifest
- `plugin/` — Plugin source directory (skills, commands)
  - `plugin/skills/` — OpenSpec skills (explore, propose, apply-change, archive-change)
  - `plugin/commands/opsx/` — Slash commands for OpenSpec workflow
  - `plugin/hooks/` — Plugin-local copy of hooks
    - `plugin/hooks/hooks.json` — Hook configuration (UserPromptSubmit, PreToolUse, SessionStart)
    - `plugin/hooks/on-user-prompt.py` — Hook script: checks openspec changes on user prompt
    - `plugin/hooks/pre-tool-openspec-test.py` — Hook script: TDD GATE injection, checks test file existence before production code
    - `plugin/hooks/pre-tool-commit-review.py` — Hook script: lint/type/test/review gates before git commit
    - `plugin/hooks/pre-tool-skill.py` — Hook script: compliance gate before archive, review loop state check before apply
  - `plugin/utils/` — Shared utility modules
    - `plugin/utils/active-change.py` — Shared `find_active_change()` and `count_tasks()` with priority ordering
- `demo-project/` — Demo project for testing plugin behavior

## Hook: UserPromptSubmit

When a user submits a prompt, the hook:
1. Scans `openspec/changes/` in the working project for active changes
2. Reports change names, artifacts, and task progress
3. Instructs Claude to check if documents need updates before responding

## Hook: PreToolUse — Write|Edit (TDD GATE)

When Claude is about to write/edit code files during an active OpenSpec change:
1. Detects active change using shared `find_active_change()` with priority ordering
2. Maps source file to expected test file (colocated convention)
3. If test file exists: brief reminder to ensure tests cover changes
4. If test file missing: TDD GATE — strong instruction to run `test-generator.py` before implementing

## Hook: PreToolUse — Bash (Commit Gate)

When Claude runs a git commit during an active OpenSpec change:
1. Runs type/lint checks → deny if errors
2. Runs full test suite → deny if failures
3. Checks code review reports in `test-reports/`
4. Security errors → deny commit
5. Non-security errors → allow with warning
6. No review → recommend running code review first

## Hook: PreToolUse — Skill (Archive/Apply Gate)

When Claude invokes a Skill tool:
1. **archive-change**: Runs `compliance-check.py` → deny if compliance failure
2. **apply-change**: Checks `review-loop-state.json` → warn if paused

## Plugin Identity

- **Name:** wps-claude-plugin
- **Version:** 1.0.0
- **Author:** zhangbohan