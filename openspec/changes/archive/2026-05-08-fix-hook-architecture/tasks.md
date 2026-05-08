# Tasks: Fix Hook Architecture Gaps

> **Change**: fix-hook-architecture
> **Created**: 2026-05-08

## Phase 1: Extract Shared Module & Fix find_active_change()

- [x] Extract `find_active_change()` and `count_tasks()` to `plugin/utils/active-change.py`
  - Create shared module with `find_active_change(changes_dir, cwd="")`
  - Implement priority ordering: pending tasks → staged files inference → latest mtime
  - Add `match_staged_to_change()`, `get_latest_mtime()`, `get_staged_files()`
  - Add unit tests for all functions

- [x] Update `pre-tool-commit-review.py` to use shared `active-change.py`
  - Replace local `find_active_change()` with import from `active-change.py`
  - Replace local `count_tasks()` with import from `active-change.py`
  - Pass `cwd` to `find_active_change()`
  - Verify commit gate still works for pending-task changes

- [x] Update `pre-tool-openspec-test.py` to use shared `active-change.py`
  - Replace local `find_active_change()` with import
  - Replace local `count_tasks()` with import
  - Pass `cwd` to `find_active_change()`

- [x] Verify: all-tasks-done commit still triggers gate
  - Create a test change with all tasks marked complete
  - Run `git commit` → should still execute lint/test/review checks

## Phase 2: Add pre-tool-skill.py Hook

- [x] Create `plugin/hooks/pre-tool-skill.py`
  - Parse `tool_name="Skill"` and `skill` name from input
  - Implement `handle_archive_skill()`: run compliance-check.py, deny on failure
  - Implement `handle_apply_skill()`: check review-loop-state, warn on paused
  - Import `find_active_change()` from `active-change.py`
  - Handle `compliance-check.py` not available gracefully

- [x] Update `plugin/hooks/hooks.json`
  - Add `Skill` matcher with `pre-tool-skill.py`

- [x] Verify: archive with compliance failure is denied
  - Create a change missing proposal.md or tasks.md
  - Try to archive → should get deny with blocking issues

- [x] Verify: archive with compliance pass is allowed
  - Create a complete change (all artifacts, review passed)
  - Archive → should allow

## Phase 3: Enhance pre-tool-openspec-test.py

- [x] Add test file existence check in `pre-tool-openspec-test.py`
  - Map source file path to expected test file path (colocated convention)
  - If test file doesn't exist, inject stronger TDD GATE context
  - Include explicit command to run `test-generator.py`

- [x] Update context message format
  - Old: "TDD: Write tests BEFORE production code."
  - New: "TDD GATE: No test file found for 'X'. BEFORE implementing, you MUST run test-generator.py..."

- [x] Verify: writing production code without test triggers TDD GATE
  - Active change → write to `src/auth.js` → get TDD GATE instruction

## Phase 4: Update Improvement Doc & Demo

- [x] Update `improvement.md` with accurate status
  - Remove "缺少 SKILL 源码" claim
  - Add distinction: tool exists vs. flow integrated
  - Mark completed items

- [x] Update demo-project to reflect new Hook behavior
  - Ensure demo change works with new `find_active_change()` logic

- [x] Update CLAUDE.md if needed
  - Document new Hook: `pre-tool-skill.py`
  - Document `active-change.py` shared module

## Phase 5: Connect ERROR → Task → Re-enter Apply Loop

- [ ] Enhance `pre-tool-commit-review.py` to inject ERROR→Task instruction
  - When review verdict is BLOCK with non-security errors
  - Inject context with explicit commands to:
    1. Run `review-parser.py --generate-tasks --append-to tasks.md`
    2. Run `review-loop-state.py --record <errors> <tasks>`
    3. Re-invoke apply-change skill
  - Change from "allow + warn" to "allow + actionable instruction"

- [ ] Enhance `pre-tool-skill.py` handle_apply_skill() for loop state
  - Check if review-loop-state.json exists and has errors
  - If loop count < max_loops and current_errors > 0:
    - Inject context: "N errors pending. Run review-parser to generate fix tasks."
  - If loop count >= max_loops (paused):
    - Inject context for manual intervention (already implemented)

- [ ] Create integration test for ERROR→Task flow
  - Create a change with a code review report containing ERRORs
  - Attempt git commit → should get actionable instructions
  - Follow instructions → fix tasks should appear in tasks.md

- [ ] Update `improvement.md` to mark Phase 5 complete
  - Update "接通 ERROR → Task → 重入 Apply" status to ✅
