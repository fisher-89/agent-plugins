## Context

The current `on-user-prompt.py` hook intercepts every user prompt and attempts to classify intent (explore/implement/fix/archive/question) using regex keyword matching on the raw prompt text. It also estimates scope, analyzes OpenSpec change state, and produces routing decisions that either auto-execute skills or display numbered suggestions.

This approach has fundamental problems:
- Regex patterns produce false positives on ambiguous prompts (e.g., "完成这个功能" could be "archive" or "implement")
- The system overrides user control — users can't bypass the routing even when they know exactly what they want
- No amount of pattern tuning can fix semantic misunderstanding
- Mixed Chinese/English keyword matching adds complexity without improving accuracy

The design removes this implicit inference layer entirely, shifting to explicit user-controlled workflow via slash commands.

## Goals / Non-Goals

**Goals:**
- Delete `on-user-prompt.py` and the UserPromptSubmit hook entry
- Remove all intent classification, confidence scoring, scope estimation, and routing logic
- Users manually invoke skills (`/dev-team:openspec-explore`, `/dev-team:openspec-propose`, `/dev-team:openspec-apply-change`, `/dev-team:openspec-archive-change`)
- PreToolUse hooks (TDD gate, commit gate, skill gate) continue to provide automated workflow enforcement
- SessionStart hook continues to ensure OpenSpec CLI is installed

**Non-Goals:**
- No changes to how PreToolUse hooks work or what they check
- No changes to how skills (explore, propose, apply-change, archive-change) work
- No replacement mechanism for intent detection — this is deliberately removed
- No changes to the report-chain / step-report system

## Decisions

### Decision 1: Remove UserPromptSubmit entirely rather than simplify it

**Rationale**: The UserPromptSubmit hook's sole purpose after removing intent routing would be to report active change status. This is already handled by:
- SessionStart hook showing initial state
- CLAUDE.md instructing Claude to check for changes
- PreToolUse hooks checking active change state before writes/commits/skills

A stripped-down hook that only reports active changes adds noise on every prompt without adding value. Removing the hook entry from `hooks.json` is cleaner than maintaining a simplified version.

**Alternatives considered**:
- Keep hook but remove classification: Would still fire on every prompt and add latency with no meaningful benefit.
- Keep only active change reporting: Redundant with SessionStart and CLAUDE.md instructions.

### Decision 2: Keep all PreToolUse hooks unchanged

**Rationale**: PreToolUse hooks (TDD gate, commit gate, skill gate) enforce workflow quality — they block unsafe actions like committing without tests or archiving without completed report chains. These are mechanical checks, not intent-driven, and remain valuable regardless of how the user chooses to work.

### Decision 3: No replacement UI — users use existing slash commands

**Rationale**: The OpenSpec skills are already available as slash commands:
- `/dev-team:openspec-explore`
- `/dev-team:openspec-propose`
- `/dev-team:openspec-apply-change`
- `/dev-team:openspec-archive-change`

No new UI is needed. Removing the intent hook means users simply use these commands directly instead of having the system guess which one they want.

## Risks / Trade-offs

- **Risk**: Users unfamiliar with slash commands may not know how to start working
  → Mitigation: SessionStart hook and CLAUDE.md provide clear instructions on available commands
- **Risk**: User types ambiguous natural language without using a slash command, and no routing happens
  → Mitigation: Claude can still respond naturally and suggest appropriate slash commands when relevant
