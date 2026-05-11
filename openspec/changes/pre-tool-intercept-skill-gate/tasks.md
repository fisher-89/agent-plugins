## 1. Core Infrastructure - Routing State Module

- [ ] 1.1 Create `plugins/dev-team/utils/routing-state.py` with dataclass `PendingState` containing version, timestamp, decision, intent, and options fields
- [ ] 1.2 Implement `write_pending_state(cwd, decision, intent)` function that writes JSON state to `.claude/pending-skill.json`
- [ ] 1.3 Implement `read_pending_state(cwd)` function that reads and parses the pending state file, returning None if not exists
- [ ] 1.4 Implement `clear_pending_state(cwd)` function that removes the pending state file
- [ ] 1.5 Implement `is_state_expired(state, timeout_minutes=5)` function that checks if state timestamp is older than timeout
- [ ] 1.6 Add unit tests for routing-state module functions

## 2. UserPromptSubmit Hook Enhancement

- [ ] 2.1 Add import for `write_pending_state` in `plugins/dev-team/hooks/on-user-prompt.py`
- [ ] 2.2 Add logic to write pending state when `decision.mode in ("auto", "suggest") and decision.action != "direct"`
- [ ] 2.3 Add skip workflow keyword detection ("implement directly", "skip", "直接实现") in IntentClassifier
- [ ] 2.4 Add logic to clear pending state when user's prompt contains skip keywords
- [ ] 2.5 Ensure `.claude/` directory is created if it doesn't exist when writing state

## 3. PreToolUse Skill Gate Hook

- [ ] 3.1 Create `plugins/dev-team/hooks/pre-tool-skill-gate.py` with main entry point
- [ ] 3.2 Implement state reading and expiration check at hook start
- [ ] 3.3 Implement `build_deny_message(state)` function for auto-mode format
- [ ] 3.4 Implement `build_deny_message(state)` function for suggest-mode format with options
- [ ] 3.5 Map decision action to skill name (explore → dev-team:openspec-explore, apply-change → dev-team:openspec-apply-change, archive → dev-team:openspec-archive-change)
- [ ] 3.6 Return `deny` with additionalContext when pending state exists and is not expired
- [ ] 3.7 Return `allow` when no pending state or state is expired (after clearing)

## 4. PreToolUse Skill Hook Enhancement

- [ ] 4.1 Add import for `clear_pending_state` in `plugins/dev-team/hooks/pre-tool-skill.py`
- [ ] 4.2 Add logic to detect openspec skill invocation (skill name starts with "dev-team:openspec")
- [ ] 4.3 Clear pending state when matching openspec skill is invoked
- [ ] 4.4 Ensure clear happens before allowing the skill invocation

## 5. Hook Configuration

- [ ] 5.1 Add new PreToolUse entry in `hooks.json` with matcher `Write|Edit` pointing to `pre-tool-skill-gate.py`
- [ ] 5.2 Position the new hook entry after existing Write|Edit hooks (TDD gate) in the hooks array
- [ ] 5.3 Validate hooks.json syntax after modification

## 6. Integration Testing

- [ ] 6.1 Test: UserPromptSubmit writes state file for implement intent without active change
- [ ] 6.2 Test: PreToolUse skill-gate denies Write when pending state exists
- [ ] 6.3 Test: PreToolUse skill-gate allows Write when no pending state exists
- [ ] 6.4 Test: PreToolUse skill-gate allows Write when state is expired
- [ ] 6.5 Test: PreToolUse Skill hook clears state when openspec skill is invoked
- [ ] 6.6 Test: UserPromptSubmit clears state when user types skip keywords
- [ ] 6.7 Test: Full flow - user types implement intent → state written → Write denied → Skill invoked → state cleared
