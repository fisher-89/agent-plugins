# Tasks: Slim Plugin with Report-Driven Gates

> **Change**: slim-plugin-report-gates
> **Created**: 2026-05-08

## Phase 1: Delete Embedded Skills

- [x] Delete `plugin/skills/openspec-explore/` directory
  - Remove entire directory and its contents

- [x] Delete `plugin/skills/openspec-propose/` directory
  - Remove entire directory and its contents

- [x] Delete `plugin/skills/openspec-apply-change/` directory
  - Remove entire directory and its contents

- [x] Delete `plugin/skills/openspec-archive-change/` directory
  - Remove entire directory and its contents

- [x] Create `plugin/hooks/session-start-ensure-openspec.py`
  - Check if openspec CLI is installed
  - If not installed: output additionalContext asking user to install
  - If installed: output empty context
  - No skill sync logic
  - No auto-install logic

- [x] Update `plugin/hooks/hooks.json`
  - Replace `session-start-sync-skills.py` with `session-start-ensure-openspec.py`

- [x] Delete `plugin/hooks/session-start-sync-skills.py`
  - Remove file after new hook is in place

## Phase 2: Report Generation

- [x] Create report template `plugin/templates/step-report.json`
  - Define standard report schema with schema, change, task_id, step, status, timestamp, details

- [x] Create shared report utility in `plugin/utils/step-report.py`
  - `save_step_report(change, task_id, step, status, details, project_root)` function
  - Creates `openspec/changes/<name>/reports/` directory if needed
  - Writes `task-<N>_<step>.json` or `<step>.json` for global steps
  - ISO-8601 UTC timestamps

- [x] Enhance `plugin/utils/test-scope.py` with `--save-report`
  - Add `--save-report --change <name>` CLI arguments
  - When flag set: call `save_step_report()` with step="scope"
  - Preserve existing stdout output

- [x] Enhance `plugin/utils/test-generator.py` with `--save-report`
  - Add `--save-report --change <name>` CLI arguments
  - When flag set: call `save_step_report()` with step="skeleton"
  - Preserve existing stdout output

- [x] Enhance `plugin/utils/lint-runner.py` with `--save-report`
  - Add `--save-report --change <name>` CLI arguments
  - When flag set: call `save_step_report()` with step="lint"
  - Include error/warning counts in details
  - Preserve existing stdout output

- [x] Enhance `plugin/utils/test-runner.py` with `--save-report`
  - Add `--save-report --change <name>` CLI arguments
  - When flag set: call `save_step_report()` with step="scoped-test" or "full-test"
  - Include pass/fail counts in details
  - Preserve existing stdout output

## Phase 3: Hook Report Chain Check

- [x] Create report chain checker in `plugin/utils/report-chain.py`
  - `check_report_chain(change_dir, task_id)` → (ok, error_message)
  - `check_all_report_chains(change_dir, tasks_md)` → (ok, issues_list)
  - `check_final_reports(change_dir)` → (ok, error_message)
  - `get_current_task(change_dir)` → task_id or None
  - `parse_tasks(tasks_md_path)` → list of (id, description, completed)

- [x] Enhance `plugin/hooks/pre-tool-commit-review.py` with report chain check
  - Before running lint/test/review: check current task report chain
  - If chain incomplete → deny with step-specific guidance
  - If chain complete → proceed with existing gate logic

- [x] Enhance `plugin/hooks/pre-tool-skill.py` archive check with report chains
  - After compliance check: check all task report chains
  - Check final reports (full-test.json, code-review.json)
  - If any chain incomplete → deny with details

## Phase 4: ERROR → Task Close Loop (from fix-hook-architecture Phase 5)

- [x] Enhance `plugin/hooks/pre-tool-commit-review.py` to inject ERROR→Task instruction
  - When review verdict is BLOCK with non-security errors
  - Inject context with explicit commands to:
    1. Run `review-parser.py --generate-tasks --append-to tasks.md`
    2. Run `review-loop-state.py --record <errors> <tasks>`
    3. Re-invoke apply-change skill
  - Change from "allow + warn" to "allow + actionable instruction"

- [x] Enhance `plugin/hooks/pre-tool-skill.py` handle_apply_skill() for loop state
  - Check if review-loop-state.json exists and has errors
  - If loop count < max_loops and current_errors > 0:
    - Inject context: "N errors pending. Run review-parser to generate fix tasks."
  - If loop count >= max_loops (paused):
    - Inject context for manual intervention (already implemented)

- [x] Create integration test for ERROR→Task flow
  - Create a change with a code review report containing ERRORs
  - Attempt git commit → should get actionable instructions
  - Follow instructions → fix tasks should appear in tasks.md

## Phase 5: Documentation

- [x] Update `improvement.md`
  - Mark P0-2 as resolved (skill no longer embedded)
  - Update status for all affected items
  - Add report-driven gates as new capability
  - (File does not exist - skipped)

- [x] Update `CLAUDE.md`
  - Remove references to embedded openspec skills
  - Document session-start-ensure-openspec.py behavior
  - Document report-driven gates concept
  - Document report chain check in hooks

- [x] Update `README.md`
  - Reflect new architecture (no embedded skills)
  - Update plugin structure description
  - Add report-driven workflow explanation
