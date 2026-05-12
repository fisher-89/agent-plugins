# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is **wps-claude-plugin**, a custom Claude Code plugin designed to enhance development workflow with OpenSpec integration.

## Architecture: Slim Plugin + Report-Driven Gates

The plugin follows a "slim" architecture:
- **OpenSpec CLI** is the source of truth for skills (explore, propose, apply-change, archive-change)
- **Plugin provides hooks** for report-driven workflow gates
- **Plugin provides agents** for specialized, context-intensive work (code review)
- **Plugin provides Python utilities** for deterministic, scriptable tasks (architecture validation, model management, ADRs)
- **No embedded skills** - skills are invoked via OpenSpec CLI directly; plugin skills are thin routing wrappers

## Coding Guidelines

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

1. **Think before coding**: State assumptions. If unclear, stop and ask.
2. **Simplicity first**: No speculative features. No abstractions for single-use code.
3. **Surgical changes**: Touch only what's needed. Match existing style.
4. **Goal-Driven Execution**: Define success criteria. Loop until verified.

## Repository Structure

- `marketplace.json` — Claude plugin marketplace configuration
- `plugins/dev-team/` — Dev-team plugin source directory
  - `plugins/dev-team/.claude-plugin/plugin.json` — Plugin manifest
  - `plugins/dev-team/skills/` — Plugin-specific skills
    - `plugins/dev-team/skills/code-review/` — Thin routing skill → Agent tool
    - `plugins/dev-team/skills/openspec-*/` — OpenSpec workflow skills
  - `plugins/dev-team/agents/` — Subagent definitions
    - `plugins/dev-team/agents/code-review.md` — Code review agent (pure LLM)
  - `plugins/dev-team/hooks/` — Hook implementations
    - `plugins/dev-team/hooks/hooks.json` — Hook configuration (PreToolUse, SessionStart)
    - `plugins/dev-team/hooks/pre-tool-openspec-test.py` — Hook script: TDD GATE injection
    - `plugins/dev-team/hooks/pre-tool-skill.py` — Hook script: compliance gate + report chain gate
    - `plugins/dev-team/hooks/session-start-ensure-openspec.py` — Hook script: ensure openspec CLI installed
    - `plugins/dev-team/hooks/commit-gates/` — Commit gate modules
      - `plugins/dev-team/hooks/commit-gates/before-commit.py` — Unified commit gate router
      - `plugins/dev-team/hooks/commit-gates/quality.py` — SDD quality gates (lint/test/review)
      - `plugins/dev-team/hooks/commit-gates/architecture.py` — Architecture validation gate
  - `plugins/dev-team/utils/` — Shared utility modules
    - `plugins/dev-team/utils/active-change.py` — Shared `find_active_change()` and `count_tasks()`
    - `plugins/dev-team/utils/step-report.py` — Step report generation for report-driven gates
    - `plugins/dev-team/utils/report-chain.py` — Report chain validation
    - `plugins/dev-team/utils/test-scope.py` — Test scope identification (with --save-report)
    - `plugins/dev-team/utils/test-generator.py` — Test skeleton generator (with --save-report)
    - `plugins/dev-team/utils/lint-runner.py` — Lint/type runner (with --save-report)
    - `plugins/dev-team/utils/test-runner.py` — Test runner (with --save-report)
    - `plugins/dev-team/utils/archi-model.py` — Architecture model query/validate/write
    - `plugins/dev-team/utils/archi-validate.py` — Architecture validation engine
    - `plugins/dev-team/utils/archi-decide.py` — ADR management (create/list/update)
  - `plugins/dev-team/templates/` — Template files
    - `plugins/dev-team/templates/step-report.json` — Step report schema template
    - `plugins/dev-team/templates/model.c4` — C4 architecture model template
    - `plugins/dev-team/templates/adr.md` — Architecture Decision Record template
- `demo-project/` — Demo project for testing plugin behavior

Users manually invoke skills via slash commands to control their workflow:
- `/dev-team:code-review` — Route to code-review subagent for staged change review
- `/dev-team:openspec-explore` — Explore ideas and investigate problems
- `/dev-team:openspec-propose` — Propose a new change with full artifacts
- `/dev-team:openspec-apply-change` — Implement tasks from an OpenSpec change
- `/dev-team:openspec-archive-change` — Archive a completed change

## Hook: PreToolUse — Write|Edit (TDD GATE)

When Claude is about to write/edit code files during an active OpenSpec change:
1. Detects active change using shared `find_active_change()` with priority ordering
2. Maps source file to expected test file (colocated convention)
3. If test file exists: brief reminder to ensure tests cover changes
4. If test file missing: TDD GATE — strong instruction to run `test-generator.py` before implementing

## Hook: PreToolUse — Bash (Commit Gate)

When Claude runs a git commit during an active OpenSpec change:
1. **SDD Quality Gate** (`quality.py`):
   1. **Report Chain Check**: Verify current task has scope → lint → scoped-test reports
   2. Runs type/lint checks → deny if errors
   3. Runs full test suite → deny if failures
   4. Checks code review reports in `test-reports/`
   5. Security errors → deny commit
   6. Non-security errors → allow with ERROR→Task instruction to generate fix tasks
   7. No review → recommend running code review first
2. **Architecture Gate** (`architecture.py`):
   1. If no model at `openspec/architecture/model.c4` → skip
   2. If no code files staged → skip
   3. Check for validate-*.json report in `openspec/architecture/reports/`
   4. Missing report → deny with instruction to run `archi-validate.py --staged`
   5. Report staged → allow

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

- **Name:** dev-team
- **Version:** 2.1.0
- **Author:** zhangbohan