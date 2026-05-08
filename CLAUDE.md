# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is **wps-claude-plugin**, a custom Claude Code plugin designed to enhance development workflow with OpenSpec integration.

## Architecture: Slim Plugin + Report-Driven Gates

The plugin follows a "slim" architecture:
- **OpenSpec CLI** is the source of truth for skills (explore, propose, apply-change, archive-change)
- **Plugin provides hooks** for report-driven workflow gates
- **No embedded skills** - skills are invoked via OpenSpec CLI directly

## Repository Structure

- `.claude-plugin/plugin.json` — Plugin manifest
- `plugin/` — Plugin source directory
  - `plugin/skills/` — Plugin-specific skills (code-review only)
  - `plugin/hooks/` — Hook implementations
    - `plugin/hooks/hooks.json` — Hook configuration (UserPromptSubmit, PreToolUse, SessionStart)
    - `plugin/hooks/on-user-prompt.py` — Hook script: checks openspec changes on user prompt
    - `plugin/hooks/pre-tool-openspec-test.py` — Hook script: TDD GATE injection
    - `plugin/hooks/pre-tool-commit-review.py` — Hook script: lint/type/test/review gates + report chain check
    - `plugin/hooks/pre-tool-skill.py` — Hook script: compliance gate + report chain gate
    - `plugin/hooks/session-start-ensure-openspec.py` — Hook script: ensure openspec CLI installed
  - `plugin/utils/` — Shared utility modules
    - `plugin/utils/active-change.py` — Shared `find_active_change()` and `count_tasks()`
    - `plugin/utils/step-report.py` — Step report generation for report-driven gates
    - `plugin/utils/report-chain.py` — Report chain validation
    - `plugin/utils/test-scope.py` — Test scope identification (with --save-report)
    - `plugin/utils/test-generator.py` — Test skeleton generator (with --save-report)
    - `plugin/utils/lint-runner.py` — Lint/type runner (with --save-report)
    - `plugin/utils/test-runner.py` — Test runner (with --save-report)
  - `plugin/templates/` — Template files
    - `plugin/templates/step-report.json` — Step report schema template
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
1. **Report Chain Check**: Verify current task has scope → lint → scoped-test reports
2. Runs type/lint checks → deny if errors
3. Runs full test suite → deny if failures
4. Checks code review reports in `test-reports/`
5. Security errors → deny commit
6. Non-security errors → allow with ERROR→Task instruction to generate fix tasks
7. No review → recommend running code review first

## Hook: PreToolUse — Skill (Archive/Apply Gate)

When Claude invokes a Skill tool:
1. **archive-change**: Runs compliance check + all report chains check → deny if any failure
2. **apply-change**: Checks `review-loop-state.json` → provide context for active loop

## Hook: SessionStart — Ensure OpenSpec

When a session starts:
1. Checks if `openspec` CLI is installed
2. If not installed: outputs context asking user to install with `npm install -g openspec-cli`
3. No skill sync logic - skills are invoked via CLI directly

## Report-Driven Gates

Each SDD workflow step produces a JSON report:

```
openspec/changes/<name>/reports/
├── task-1_scope.json       (test-scope.py --save-report)
├── task-1_skeleton.json    (test-generator.py --save-report)
├── task-1_lint.json        (lint-runner.py --save-report)
├── task-1_scoped-test.json (test-runner.py --save-report)
├── full-test.json          (all tasks complete)
└── code-review.json        (after full-test passes)
```

Hooks validate report chains to ensure SDD workflow is followed.

## Plugin Identity

- **Name:** wps-claude-plugin
- **Version:** 1.0.0
- **Author:** zhangbohan