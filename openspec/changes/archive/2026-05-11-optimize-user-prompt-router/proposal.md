# Proposal: Optimize UserPromptSubmit Hook Intent Router

## Why

The current UserPromptSubmit hook has a single-path suggestion (always recommends `/openspec-explore`) regardless of user intent or active change state. This creates friction when users want to quickly fix a small issue, ask a question, or continue an existing workflow. A smarter routing system can reduce unnecessary workflow overhead while still guiding users toward structured development when appropriate.

## What Changes

- **Intent classification**: Detect user intent type (explore/implement/fix/archive/question) with confidence levels
- **Scope estimation**: Estimate change scope (trivial/small/medium/large) from prompt content
- **State-aware routing**: Route based on both intent and active change state (tasks progress, change stage)
- **Auto-execution**: High-confidence scenarios automatically trigger the appropriate skill
- **Smart suggestions**: Low-confidence scenarios offer clear options for user to choose
- **Trivial fix detection**: Recognize small fixes (single file, ≤2 functions, no public API change) and route directly

## Capabilities

### New Capabilities

- `intent-router`: Intent classification and routing decision system for UserPromptSubmit hook

### Modified Capabilities

- `user-prompt-hook`: Enhanced hook behavior with state-aware routing

## Impact

**Affected Files:**
- `plugins/dev-team/hooks/on-user-prompt.py` — Main refactor with new routing logic

**New Utility (optional):**
- `plugins/dev-team/utils/intent-router.py` — Intent classification and routing logic (if separation desired)

**Not Affected:**
- `hooks.json` — Hook configuration unchanged
- Other hooks — No changes required

**Backward Compatibility:**
- Existing behavior preserved for edge cases
- No breaking changes to hook interface
