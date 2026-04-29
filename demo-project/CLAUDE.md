# Demo Project

This is a demo project for testing the wps-claude-plugin hooks.

## OpenSpec Integration

The project uses OpenSpec for change management:
- `openspec/changes/` - Active change proposals
- `openspec/changes/archive/` - Archived changes

## Testing the Hook

When you ask a question in this project, the UserPromptSubmit hook should:
1. Detect the active changes in `openspec/changes/`
2. Provide context about the change documents to Claude
3. Claude will check if documents need updates before responding