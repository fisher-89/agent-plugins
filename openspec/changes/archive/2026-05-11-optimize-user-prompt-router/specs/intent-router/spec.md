## ADDED Requirements

### Requirement: Intent classification from prompt text
The system SHALL classify user prompts into one of the following intent types: `explore`, `implement`, `fix`, `archive`, `question`.

#### Scenario: Classify implementation intent
- **WHEN** prompt contains keywords like "实现", "添加", "新增", "implement", "add", "create"
- **THEN** system returns intent type `implement` with confidence level

#### Scenario: Classify fix intent
- **WHEN** prompt contains keywords like "修复", "解决", "fix", "bug", "resolve"
- **THEN** system returns intent type `fix` with confidence level

#### Scenario: Classify explore intent
- **WHEN** prompt contains keywords like "分析", "理解", "研究", "analyze", "understand", "explore"
- **THEN** system returns intent type `explore` with confidence level

#### Scenario: Classify archive intent
- **WHEN** prompt contains keywords like "归档", "完成", "archive", "done", "complete"
- **THEN** system returns intent type `archive` with confidence level

#### Scenario: Classify question intent
- **WHEN** prompt contains keywords like "是什么", "为什么", "怎么做", "what", "why", "how", "explain"
- **THEN** system returns intent type `question` with confidence level

#### Scenario: No intent matched
- **WHEN** prompt does not match any intent pattern
- **THEN** system returns intent type `question` with confidence `low`

### Requirement: Confidence level assignment
The system SHALL assign a confidence level of `high`, `medium`, or `low` based on the number of keyword matches.

#### Scenario: High confidence
- **WHEN** 2 or more keyword patterns match for the same intent type
- **THEN** confidence is `high`

#### Scenario: Medium confidence
- **WHEN** exactly 1 keyword pattern matches for the best intent type
- **THEN** confidence is `medium`

#### Scenario: Low confidence
- **WHEN** no keyword patterns match any intent type
- **THEN** confidence is `low`

### Requirement: Scope estimation from prompt text
The system SHALL estimate the scope of the requested change as `trivial`, `small`, `medium`, or `large`.

#### Scenario: Trivial scope
- **WHEN** prompt contains keywords like "typo", "变量名", "注释", "comment"
- **THEN** scope is estimated as `trivial`

#### Scenario: Small scope
- **WHEN** prompt contains keywords like "函数", "单文件", "function", "single file"
- **THEN** scope is estimated as `small`

#### Scenario: Medium scope
- **WHEN** prompt contains keywords like "模块", "多文件", "module", "multiple files"
- **THEN** scope is estimated as `medium`

#### Scenario: Large scope
- **WHEN** prompt contains keywords like "架构", "重构", "系统", "architecture", "refactor", "system"
- **THEN** scope is estimated as `large`

#### Scenario: Default scope
- **WHEN** no scope keywords are matched
- **THEN** scope defaults to `small`

### Requirement: Trivial fix detection
The system SHALL recognize a fix as trivial when ALL of the following conditions are met: single file, ≤2 functions affected, no public API impact.

#### Scenario: Trivial fix by keywords
- **WHEN** intent is `fix` AND prompt contains keywords like "内部", "private", "helper", "工具函数" OR scope hints indicate single file and small scope
- **THEN** fix is classified as `trivial`

#### Scenario: Non-trivial fix
- **WHEN** intent is `fix` AND prompt indicates multiple files, or >2 functions, or mentions public/interface/exported symbols
- **THEN** fix is classified as non-trivial (scope ≥ `small`)

### Requirement: Routing decision based on intent, scope, and state
The system SHALL produce a routing decision with an action (`explore`, `apply-change`, `archive`, `direct`) and an execution mode (`auto` or `suggest`).

#### Scenario: Question intent always routes direct
- **WHEN** intent is `question`
- **THEN** action is `direct`, mode is `auto`

#### Scenario: Explore intent always routes direct
- **WHEN** intent is `explore`
- **THEN** action is `direct`, mode is `auto`

#### Scenario: Trivial fix routes direct
- **WHEN** intent is `fix` AND scope is `trivial`
- **THEN** action is `direct`, mode is `auto`

#### Scenario: Non-trivial fix without active change routes explore
- **WHEN** intent is `fix` AND scope is ≥ `small` AND no active changes exist
- **THEN** action is `explore`, mode is `suggest`

#### Scenario: Non-trivial fix with active change routes apply-change
- **WHEN** intent is `fix` AND scope is ≥ `small` AND active changes exist
- **THEN** action is `apply-change`, mode is `suggest`

#### Scenario: Implement with high confidence and no active change auto-routes explore
- **WHEN** intent is `implement` AND confidence is `high` AND no active changes exist
- **THEN** action is `explore`, mode is `auto`

#### Scenario: Implement with lower confidence and no active change suggests explore
- **WHEN** intent is `implement` AND confidence is NOT `high` AND no active changes exist
- **THEN** action is `explore`, mode is `suggest`

#### Scenario: Implement with active change and no tasks started routes explore
- **WHEN** intent is `implement` AND active changes exist AND all active changes have 0 completed tasks
- **THEN** action is `explore`, mode is `suggest`

#### Scenario: Implement with active change and tasks in progress routes apply-change
- **WHEN** intent is `implement` AND active changes exist AND at least one active change has >0 completed tasks
- **THEN** action is `apply-change`, mode is `suggest`

#### Scenario: Archive with all tasks complete auto-routes archive
- **WHEN** intent is `archive` AND confidence is `high` AND active changes exist with all tasks complete
- **THEN** action is `archive`, mode is `auto`

#### Scenario: Archive with partial tasks suggests confirmation
- **WHEN** intent is `archive` AND active changes exist with incomplete tasks
- **THEN** action is `archive`, mode is `suggest`

#### Scenario: Archive with no active changes routes direct
- **WHEN** intent is `archive` AND no active changes exist
- **THEN** action is `direct`, mode is `auto`
