# Tasks: Optimize UserPromptSubmit Hook Intent Router

## 1. Intent Classification

- [x] 1.1 Define `IntentClassifier` class with intent patterns (explore, implement, fix, archive, question)
- [x] 1.2 Implement keyword matching with regex patterns for Chinese and English
- [x] 1.3 Implement confidence level calculation (high/medium/low based on match count)
- [x] 1.4 Define scope estimation hints (trivial, small, medium, large)
- [x] 1.5 Implement scope estimation from prompt keywords
- [x] 1.6 Implement trivial fix detection heuristics (keywords + scope)

## 2. State Analysis

- [x] 2.1 Define `StateAnalyzer` class
- [x] 2.2 Implement `find_active_changes()` to scan openspec/changes directory
- [x] 2.3 Implement `get_change_stage()` to determine change phase (proposal/design/tasks)
- [x] 2.4 Implement `get_tasks_progress()` to count completed vs total tasks

## 3. Routing Decision

- [x] 3.1 Define `RouteDecision` dataclass (action, mode, reason, options)
- [x] 3.2 Define `Router` class
- [x] 3.3 Implement decision matrix for question/explore intents (always direct)
- [x] 3.4 Implement decision matrix for fix intent (trivial vs non-trivial)
- [x] 3.5 Implement decision matrix for implement intent (auto vs suggest, explore vs apply-change)
- [x] 3.6 Implement decision matrix for archive intent (auto vs suggest)
- [x] 3.7 Implement auto-execute condition check (high confidence + appropriate state)

## 4. Output Formatting

- [x] 4.1 Define `OutputBuilder` class
- [x] 4.2 Implement `auto_route()` format (Auto-Route: /openspec-<skill> + reason)
- [x] 4.3 Implement `suggestion()` format (intent info + numbered options)
- [x] 4.4 Implement `direct()` format (existing change info + auto-commit hint)
- [x] 4.5 Preserve existing active change reporting in direct mode

## 5. Main Hook Integration

- [x] 5.1 Refactor `main()` to use new classes (IntentClassifier, StateAnalyzer, Router, OutputBuilder)
- [x] 5.2 Integrate routing decision into output flow
- [x] 5.3 Ensure backward compatibility with existing behavior
- [x] 5.4 Test with various prompt types (implement, fix, explore, question, archive)
