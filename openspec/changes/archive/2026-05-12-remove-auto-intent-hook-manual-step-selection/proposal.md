## Why

The current UserPromptSubmit hook attempts to infer user intent (explore/implement/fix/archive/question) from natural language via regex keyword matching. This approach is inherently unreliable — regex cannot understand semantic nuance, leading to misclassification and frustrating auto-behavior. Users should explicitly control their workflow by choosing which step to execute, rather than having the system guess.

## What Changes

- **BREAKING**: Remove `on-user-prompt.py` hook entirely — eliminates the UserPromptSubmit hook that performs intent classification, state analysis, routing decisions, and output building
- Remove `hooks.json` UserPromptSubmit entry, leaving only SessionStart and PreToolUse hooks
- **BREAKING**: Remove `intent-router` spec capability — intent classification, confidence scoring, scope estimation, trivial fix detection, and routing decision logic are all deleted
- Modify `user-prompt-hook` spec to remove intent-based routing requirements, keeping only active change reporting
- Remove `hook-output-import` spec requirement for UserPromptSubmit imports
- Users manually invoke skills via slash commands (`/dev-team:openspec-explore`, `/dev-team:openspec-propose`, `/dev-team:openspec-apply-change`, `/dev-team:openspec-archive-change`) to control their workflow

## Capabilities

### New Capabilities
<!-- None — this change is purely removal/simplification -->

### Modified Capabilities
- `user-prompt-hook`: Remove all intent-based routing requirements (intent classification, auto-route, suggestion output formats). Retain only the active change reporting behavior that informs users of ongoing changes.
- `hook-output-import`: Remove the UserPromptSubmit import requirement since the hook file is being deleted.
- `intent-router`: Remove entirely — this capability (intent classification, confidence scoring, scope estimation, routing decisions) is deleted.

## Impact

- `plugins/dev-team/hooks/on-user-prompt.py` — deleted
- `plugins/dev-team/hooks/hooks.json` — UserPromptSubmit entry removed
- `plugins/dev-team/.claude-plugin/plugin.json` — version bump to 2.0.0 (breaking change)
- `openspec/specs/intent-router/spec.md` — deleted
- `openspec/specs/user-prompt-hook/spec.md` — modified (remove intent routing, keep active change reporting)
- `openspec/specs/hook-output-import/spec.md` — modified (remove UserPromptSubmit import requirement)
- `CLAUDE.md` — updated to reflect removal of UserPromptSubmit hook
