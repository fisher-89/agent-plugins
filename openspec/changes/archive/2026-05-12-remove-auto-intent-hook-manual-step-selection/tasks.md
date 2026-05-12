## 1. Remove UserPromptSubmit hook

- [x] 1.1 Remove the `UserPromptSubmit` entry from `plugins/dev-team/hooks/hooks.json`
- [x] 1.2 Delete `plugins/dev-team/hooks/on-user-prompt.py`

## 2. Update plugin metadata

- [x] 2.1 Bump version to `2.0.0` in `plugins/dev-team/.claude-plugin/plugin.json` (breaking change)

## 3. Update documentation

- [x] 3.1 Update `CLAUDE.md` to remove references to UserPromptSubmit hook and intent-based routing, and document the new manual step workflow with slash commands
