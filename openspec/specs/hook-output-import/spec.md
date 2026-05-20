## ADDED Requirements

### Requirement: Commit gate is removed
The system SHALL NOT perform any commit-time evaluation validation. The existing commit gate hooks (`hooks/commit-gates/before-commit.py`, `hooks/commit-gates/quality.py`, `hooks/commit-gates/architecture.py`) SHALL be removed. Commit is performed as the final step of the archive flow without any pre-commit evaluation hooks.

#### Scenario: Commit proceeds without eval gate
- **WHEN** the archive flow triggers git commit
- **THEN** no eval JSON or code review checks are performed by hooks

### Requirement: Eval check script validates chain before archive
The system SHALL provide an eval check script (`utils/eval-check.py`) that validates the eval chain before archive.
The script SHALL read `openspec/changes/<name>/phases/eval.json`, extract the latest entry per phase by timestamp, and verify all phases in the expected sequence have verdict "pass" with no gaps or active backtrack markers.
If any check fails, the script SHALL output the failure reason and exit with a non-zero code.

#### Scenario: All phases pass, eval check succeeds
- **WHEN** eval check script runs and all phases (01-requirements through 07-acceptance) have latest verdict "pass"
- **THEN** the script outputs "PASS" and exits with code 0 — archive proceeds

#### Scenario: Missing phase causes failure
- **WHEN** eval check script runs and a required phase has no entries in eval.json
- **THEN** the script outputs the missing phase name and exits with code 1

#### Scenario: Failed phase causes failure
- **WHEN** eval check script runs and the latest entry for a phase has verdict "fail"
- **THEN** the script outputs the failed phase name, the failing checklist items, and exits with code 1

### Requirement: Archive flow executes sequentially
The system SHALL provide an archive flow that executes in strict sequence:
1. **eval check**: run `utils/eval-check.py` to validate all phases passed
2. **openspec archive**: run `openspec archive <change-name>` to finalize the change
3. **git commit**: run `git commit` to snapshot the completed work

If step 1 fails, steps 2 and 3 SHALL NOT execute.

#### Scenario: Full archive flow succeeds
- **WHEN** eval check passes, openspec archive completes, and git commit succeeds
- **THEN** the change is archived and all work is committed

#### Scenario: Archive flow aborts on eval check failure
- **WHEN** eval check script exits with non-zero code
- **THEN** the archive flow stops and outputs the failure details — archive and commit do not run

### Requirement: Hook output formats eval status
When a phase Evaluator produces a verdict, the hook output SHALL include a brief summary with: phase name, verdict, pass/total items, and any backtrack marker.
The output SHALL be surfaced to the user via the PreToolUse hook's normal output mechanism.

#### Scenario: Eval pass output
- **WHEN** Evaluator returns verdict "pass" with 8/8 items passing
- **THEN** hook outputs "Phase 01 (requirements): PASS (8/8)"

#### Scenario: Eval fail output with backtrack
- **WHEN** Evaluator returns verdict "fail" with backtrack_to "01-requirements"
- **THEN** hook outputs "Phase 07 (acceptance): FAIL — backtrack to 01-requirements"

### Requirement: Eval results stored in eval.json
All Evaluator output SHALL be appended to `openspec/changes/<name>/phases/eval.json` as an array. Each entry includes `phase`, `timestamp`, `attempt`, `verdict`, `items`, and `backtrack_to` fields.
No per-phase eval files SHALL be created — eval.json is the single source of truth for all evaluation results.

#### Scenario: Code review findings in eval.json
- **WHEN** code-review-evaluator finds a security issue
- **THEN** the finding is recorded as a checklist item with pass=false, evidence pointing to the specific file and line, and notes describing the issue — appended to eval.json

#### Scenario: Acceptance gaps in eval.json
- **WHEN** acceptance-evaluator finds an acceptance criterion without implementation
- **THEN** the gap is recorded as a checklist item with pass=false, evidence quoting the criterion, and backtrack_to set — appended to eval.json

## REMOVED Requirements

### Requirement: Hook files import hook-output module directly
**Reason**: The UserPromptSubmit hook (`on-user-prompt.py`) is being removed. The `output_user_prompt_submit` function import requirement no longer applies since the hook file no longer exists. The PreToolUse and SessionStart import requirements remain valid.
**Migration**: The import requirement is superseded by the fact that `on-user-prompt.py` is deleted. The `output_pre_tool_use` and `output_session_start` import requirements remain in effect for their respective hooks.
