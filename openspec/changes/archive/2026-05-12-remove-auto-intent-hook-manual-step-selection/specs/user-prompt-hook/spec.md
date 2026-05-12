## REMOVED Requirements

### Requirement: UserPromptSubmit hook provides intent-based routing
**Reason**: The UserPromptSubmit hook (`on-user-prompt.py`) and its entry in `hooks.json` are being removed entirely. Intent-based routing is the core functionality being eliminated.
**Migration**: Users manually invoke skills via slash commands (`/dev-team:openspec-explore`, `/dev-team:openspec-propose`, `/dev-team:openspec-apply-change`, `/dev-team:openspec-archive-change`). PreToolUse hooks continue to provide automated workflow gates (TDD, commit, skill).

### Requirement: Hook output distinguishes auto-route and suggestion formats
**Reason**: The auto-route and suggestion output formats are only meaningful within the intent routing system being removed.
**Migration**: Not needed — slash commands provide direct action without routing output formats.

### Requirement: Existing active change reporting preserved
**Reason**: With the UserPromptSubmit hook removed, there is no hook firing on every prompt to report active change status. Active change reporting is still available through: (1) SessionStart hook initial status, (2) CLAUDE.md instructions for Claude to check `openspec/changes/`, and (3) PreToolUse hooks that reference active changes when gates trigger.
**Migration**: Claude continues to be aware of active changes via CLAUDE.md instructions and SessionStart context. Users can check status with the OpenSpec CLI directly.
