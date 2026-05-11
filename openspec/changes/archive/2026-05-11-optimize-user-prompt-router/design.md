# Design: Intent Router for UserPromptSubmit Hook

## Context

The current `on-user-prompt.py` hook has a simple intent detection that only suggests `/openspec-explore` for any development task. This creates friction when:
- Users want quick answers (questions, code exploration)
- Users want to fix small issues without workflow overhead
- Users are in the middle of implementing a change and want to continue
- Users want to archive completed work

The hook runs before Claude processes the user's prompt, so it can only inject context/suggestions, not directly invoke skills. However, it can output instructions that Claude will interpret and execute.

## Goals / Non-Goals

**Goals:**
- Classify user intent into meaningful categories with confidence levels
- Estimate scope of requested changes
- Make routing decisions based on intent + active change state
- Auto-execute appropriate skills for high-confidence scenarios
- Present clear options for low-confidence scenarios
- Preserve all existing behavior (active change reporting, auto-commit hints)

**Non-Goals:**
- AST-based public API detection (future improvement)
- Machine learning for intent classification (keyword-based is sufficient for now)
- User preference learning (future improvement)
- Spec conflict detection (future improvement)
- Direct skill invocation from hook (not supported by Claude Code hook API)

## Decisions

### Decision 1: Single-file implementation
**Rationale:** The hook logic is self-contained (~150 lines currently). Adding routing logic will increase to ~300 lines, which is manageable in a single file. Splitting into utils would add complexity without benefit.

**Alternatives considered:**
- Separate `intent-router.py` utility: Rejected because routing logic is specific to this hook, not reusable elsewhere.

### Decision 2: Keyword-based intent classification
**Rationale:** Simple, deterministic, and covers common patterns. No need for ML complexity when keyword matching provides clear signals.

**Patterns:**
```
explore:    分析, 理解, 研究, 查看, 了解, analyze, understand, explore, investigate
implement:  实现, 添加, 新增, 开发, 重构, 优化, add, implement, create, develop, refactor, optimize
fix:        修复, 解决, fix, bug, resolve, patch
archive:    归档, 完成, 结束, archive, done, complete, finish
question:   是什么, 为什么, 怎么做, what, why, how, explain
```

**Confidence:** Count keyword matches. ≥2 matches = high, 1 match = medium, 0 matches = low.

### Decision 3: Auto-execute via context injection
**Rationale:** Claude Code hooks cannot directly invoke skills. However, Claude interprets `Auto-Route: /openspec-<skill>` in the context and will execute the skill.

**Format:**
```
Auto-Route: /openspec-explore

Reason: Detected implement intent (high confidence), no active change.
```

### Decision 4: Trivial fix heuristics (Phase 1)
**Rationale:** Full AST analysis is overkill for initial implementation. Use keyword hints + scope estimate.

**Heuristics:**
- Keywords like "内部", "private", "helper", "工具函数" suggest non-public
- Scope `trivial` (typo, variable name, comment edits) is always direct
- Default: assume non-trivial to avoid missing important routing

### Decision 5: State-aware routing priority
**Rationale:** When active changes exist, routing should consider change state.

**Logic:**
```
if tasks_completed == 0:
    # Change exists but not started → likely need to review/explore first
    suggest explore
elif tasks_completed > 0 and tasks_completed < tasks_total:
    # In progress → likely want to continue
    suggest apply-change
elif tasks_completed == tasks_total:
    # All done → likely want to archive or commit
    if intent == archive: auto archive
    else: show auto-commit hint (existing behavior)
```

## Risks / Trade-offs

### Risk: False positive intent detection
**Mitigation:** Use medium/low confidence as gating for auto-execute. Only high-confidence triggers auto. Suggestions always give user an escape hatch ("Direct answer" option).

### Risk: Keyword patterns miss edge cases
**Mitigation:** Default to `question` intent with low confidence when no patterns match. This routes to direct answer, which is safe fallback.

### Risk: Trivial fix heuristic inaccurate
**Mitigation:** Conservative default: assume non-trivial. Better to suggest workflow for a trivial fix than to skip workflow for a complex fix.

### Trade-off: Single language patterns (Chinese + English only)
**Acceptance:** Target user base primarily uses Chinese and English. Adding more languages is straightforward if needed.

## Implementation Structure

```python
# on-user-prompt.py (refactored)

class IntentClassifier:
    PATTERNS: dict[str, list[str]]  # intent_type -> regex patterns
    SCOPE_HINTS: dict[str, list[str]]  # scope -> hint keywords

    def detect(self, prompt: str) -> IntentResult:
        # Returns (intent_type, confidence, scope)

class StateAnalyzer:
    def find_active_changes(self, cwd: str) -> list[ChangeInfo]:
        # Returns list of (name, path, stage, tasks_progress)

class Router:
    def decide(self, intent: IntentResult, changes: list[ChangeInfo]) -> RouteDecision:
        # Returns (action, mode, reason)

class OutputBuilder:
    def auto_route(self, action: str, reason: str) -> str:
        # Returns formatted auto-route context

    def suggestion(self, intent: IntentResult, changes: list[ChangeInfo], options: list[str]) -> str:
        # Returns formatted suggestion context

    def direct(self, changes: list[ChangeInfo]) -> str:
        # Returns existing change info + optional auto-commit hint

def main():
    input_data = json.load(sys.stdin)
    prompt = input_data.get("prompt", "")
    cwd = input_data.get("cwd", "")

    intent = IntentClassifier().detect(prompt)
    changes = StateAnalyzer().find_active_changes(cwd)
    decision = Router().decide(intent, changes)

    if decision.mode == "auto":
        output = OutputBuilder().auto_route(decision.action, decision.reason)
    elif decision.mode == "suggest":
        output = OutputBuilder().suggestion(intent, changes, decision.options)
    else:  # direct
        output = OutputBuilder().direct(changes)

    output_user_prompt_submit(output)
```
