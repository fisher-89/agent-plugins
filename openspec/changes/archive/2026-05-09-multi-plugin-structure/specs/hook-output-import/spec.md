## MODIFIED Requirements

### Requirement: Hook files import hook-output module directly
All hook files in `plugins/dev-team/hooks/` SHALL import output functions from `hook_output` using direct Python import statements after adding `UTILS_DIR` to `sys.path`.

#### Scenario: PreToolUse hook imports output_pre_tool_use
- **WHEN** a PreToolUse hook script runs
- **THEN** it imports `output_pre_tool_use` from `hook_output` via `from hook_output import output_pre_tool_use`

#### Scenario: UserPromptSubmit hook imports output_user_prompt_submit
- **WHEN** a UserPromptSubmit hook script runs
- **THEN** it imports `output_user_prompt_submit` from `hook_output` via `from hook_output import output_user_prompt_submit`

#### Scenario: SessionStart hook imports output_session_start
- **WHEN** a SessionStart hook script runs
- **THEN** it imports `output_session_start` from `hook_output` via `from hook_output import output_session_start`
