## ADDED Requirements

### Requirement: phase-proposal skill
The system SHALL provide `dev-team:phase-proposal` skill at `skills/phase-proposal/SKILL.md` with name `phase-proposal`.

The skill SHALL follow the DESIGN P→E pattern:
- **Planner**: `proposal-planner` sub-agent writes proposal.md + specs/ to disk
- **Evaluator**: `proposal-evaluator` sub-agent evaluates against checklist, appends to eval.json via MCP eval/log

Gate check: `mcp__plugin_dev-team_dev-team__eval/check` with phase `01-proposal`.

The skill SHALL support the following input modes:
- kebab-case arg → validate, create change if not exists
- Description arg (Chinese/spaces/natural language) → derive kebab-case, confirm, scaffold
- No arg + explore context → extract decisions, derive kebab-case, scaffold
- No arg + no context → ask user what to build

#### Scenario: phase-proposal executes P→E loop
- **WHEN** user invokes `/dev-team:phase-proposal <change-name>`
- **THEN** the skill runs gate check `mcp__plugin_dev-team_dev-team__eval/check` with phase `01-proposal`
- **AND** invokes `proposal-planner` sub-agent with one-line prompt
- **AND** after planner completes, invokes `proposal-evaluator` sub-agent
- **AND** reads eval.json for verdict
- **AND** loops back to planner if verdict is "fail" (max 5 attempts)

#### Scenario: phase-proposal handles explore context
- **WHEN** user invokes `/dev-team:phase-proposal` without argument after an explore session
- **THEN** the skill detects explore context (decision tables, diagrams, "What We Figured Out" summaries)
- **AND** extracts key decisions from the explore conversation
- **AND** derives a kebab-case change name from the explore topic
- **AND** passes the explore context as EXPLORE_CONTEXT_SUMMARY to the proposal-planner prompt

### Requirement: workflow-requirement skill
The system SHALL provide `dev-team:workflow-requirement` skill at `skills/workflow-requirement/SKILL.md` with name `workflow-requirement`.

The skill SHALL be a thin orchestration loop with NO hardcoded phase knowledge. All phase sequencing, agent assignment, and prompt generation SHALL be delegated to `mcp__plugin_dev-team_dev-team__eval/next`.

**Skill logic (pseudocode):**
```
1. Assemble context: parse change name, detect explore context
2. Scaffold change if needed (openspec_new_change)
3. Loop:
   result = MCP eval/next(change, workflow_type="requirement")
   if result.error → report, STOP
   if result.done → break
   if result.planner → Agent(result.planner.agent_type, result.planner.prompt)
   for step in result.auto_steps → Bash(step.command)
   Agent(result.evaluator.agent_type, result.evaluator.prompt)
   output: "[Round R/20] [Phase N/9] {phase}: executed"
4. 全部 pass 后停止，通知用户手动 archive
```

The skill SHALL NOT:
- Contain a hardcoded phase table
- Know which sub-agent to invoke for which phase
- Generate any prompt strings for sub-agents
- Handle backtrack logic (eval/next handles it server-side)

#### Scenario: workflow-requirement calls eval/next in a loop
- **WHEN** user invokes `/dev-team:workflow-requirement <change-name>`
- **THEN** the skill assembles context (Step 0) and scaffolds the change if needed
- **AND** enters a loop calling `eval/next` before each phase
- **AND** invokes the planner and evaluator agents returned by `eval/next`
- **AND** breaks the loop when `eval/next` returns `done: true`
- **AND** displays completion summary and stops（不自动 archive）

#### Scenario: workflow-requirement handles eval/next errors
- **WHEN** `eval/next` returns `error: "round_limit_exceeded"` or `error: "max_retries_exceeded"`
- **THEN** the workflow SHALL stop and display the error message
- **AND** PushNotification SHALL be sent

#### Scenario: workflow-requirement delegates backtrack to eval/next
- **WHEN** a phase evaluator sets `backtrack_to` in eval.json
- **THEN** on the next loop iteration, `eval/next` detects the backtrack and returns the target phase
- **AND** the workflow skill does NOT handle backtrack itself — it simply calls the agents returned by `eval/next`

#### Scenario: workflow-requirement inherits explore context
- **WHEN** user invokes `/dev-team:workflow-requirement` without argument after an explore session
- **THEN** Step 0 detects explore context in the conversation
- **AND** extracts key decisions, diagrams, and "What We Figured Out" summaries
- **AND** derives a kebab-case change name
- **AND** passes the explore context to proposal-planner by appending it to the prompt returned by `eval/next`

#### Scenario: workflow-requirement handles no-op test phases via eval/next
- **WHEN** Phase 06 (unit-test) or Phase 08 (integration-test) detects no test files
- **THEN** the executor agent writes a skipped entry to eval.json via MCP eval/log with `skipped: true`
- **AND** on the next loop iteration, `eval/next` sees the pass (skipped) entry and returns the next phase
- **AND** the workflow skill does NOT implement special no-op handling

#### Scenario: workflow-requirement resumes incomplete change
- **WHEN** user invokes `/dev-team:workflow-requirement <existing-change>` for a change that already has partial eval.json entries
- **THEN** the workflow detects the change already exists and does NOT re-scaffold
- **AND** `eval/next` returns the first unpassed phase based on eval.json state
- **AND** the workflow resumes from that phase

#### Scenario: workflow-requirement resumes after interruption
- **WHEN** user re-invokes `/dev-team:workflow-requirement <change>` after a previous run was interrupted
- **THEN** already-passed phases are automatically skipped by `eval/next`
- **AND** execution resumes from the first incomplete phase

#### Scenario: workflow-requirement reports progress
- **WHEN** the workflow loop executes
- **THEN** after each iteration, the skill SHALL output: `[Round {result.round}/20] [Phase {result.phase_index}/{result.total_phases}] {result.next_phase}: executed`
- **AND** upon `eval/next` returning `done: true` or `error`, PushNotification SHALL be sent

## REMOVED Requirements

### Requirement: phase-requirements skill
**Reason**: 重构为 phase-proposal（P→E 模式，使用 proposal-planner + proposal-evaluator 子代理替代主模型直接写入）。
**Migration**: 用户改用 `/dev-team:phase-proposal` 替代 `/dev-team:phase-requirements`。目录 `skills/phase-requirements/` 删除。

## MODIFIED Requirements

### Requirement: Nine user-triggered phase skills
The system SHALL provide 9 phase skills. The `dev-team:phase-requirements` skill is REMOVED and replaced by `dev-team:phase-proposal`.

Updated skill list:

| # | Skill | Status |
|---|-------|--------|
| 1 | dev-team:phase-proposal | NEW (replaces phase-requirements) |
| 2 | dev-team:phase-dev-design | unchanged |
| 3 | dev-team:phase-test-design | unchanged |
| 4 | dev-team:phase-test-gen | unchanged |
| 5 | dev-team:phase-implement | unchanged |
| 6 | dev-team:phase-unit-test | unchanged |
| 7 | dev-team:phase-code-review | unchanged |
| 8 | dev-team:phase-integration-test | unchanged |
| 9 | dev-team:phase-acceptance | unchanged |

#### Scenario: DESIGN skill executes Planner->Evaluator with eval/check gate
- **WHEN** user invokes `dev-team:phase-proposal`, `dev-team:phase-dev-design`, or `dev-team:phase-test-design`
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__eval/check` with `change` and `phase` arguments to validate prior phases
- **AND** invokes Planner subagent with one-line prompt
- **AND** then invokes the corresponding Evaluator subagent
- **AND** the skill reads the latest eval.json entry to determine verdict
- **AND** loops back to Planner if verdict is "fail" (up to max attempts)

#### Scenario: phase-proposal gate check uses eval/check
- **WHEN** reading `skills/phase-proposal/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__eval/check` (not `eval_check`)

## Module Contract

### Skill Files (`plugins/dev-team/skills/`)

| Skill | MCP Tools Used | Agent (Planner) | Agent (Evaluator) | Contract |
|-------|---------------|-----------------|-------------------|----------|
| phase-proposal | eval/check | proposal-planner | proposal-evaluator | P→E loop, phase `01-proposal` |
| workflow-requirement | eval/next, eval/log | (from eval/next response) | (from eval/next response) | Thin loop: call eval/next → invoke returned agents → repeat; no hardcoded phase table |
