## REMOVED Requirements

### Requirement: Hook files import hook-output module directly
**Reason**: The UserPromptSubmit hook (`on-user-prompt.py`) is being removed. The `output_user_prompt_submit` function import requirement no longer applies since the hook file no longer exists. The PreToolUse and SessionStart import requirements remain valid.
**Migration**: The import requirement is superseded by the fact that `on-user-prompt.py` is deleted. The `output_pre_tool_use` and `output_session_start` import requirements remain in effect for their respective hooks.
