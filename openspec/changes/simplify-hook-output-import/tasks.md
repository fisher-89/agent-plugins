## 1. Refactor Hook Imports

- [x] 1.1 Simplify `on-user-prompt.py` — replace importlib pattern with `from hook_output import output_user_prompt_submit`
- [x] 1.2 Simplify `pre-tool-openspec-test.py` — replace importlib pattern with `from hook_output import output_pre_tool_use`
- [x] 1.3 Simplify `pre-tool-commit-review.py` — replace importlib pattern with `from hook_output import output_pre_tool_use`
- [x] 1.4 Simplify `pre-tool-skill.py` — replace importlib pattern with `from hook_output import output_pre_tool_use`
- [x] 1.5 Simplify `session-start-ensure-openspec.py` — replace importlib pattern with `from hook_output import output_session_start`
- [x] 1.6 Simplify `session-start-worktree.py` — replace importlib pattern with `from hook_output import output_session_start`

## 2. Verification

- [x] 2.1 Test all hooks work by running plugin reload
