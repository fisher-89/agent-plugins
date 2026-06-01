## ADDED Requirements

### Requirement: phase-proposal skill
The system SHALL provide `dev-team:phase-proposal` skill at `skills/phase-proposal/SKILL.md` with name `phase-proposal`.

The skill SHALL follow the DESIGN P→E pattern:
- **Planner**: `proposal-planner` sub-agent writes proposal.md + specs/ to disk
- **Evaluator**: `proposal-evaluator` sub-agent evaluates against checklist, appends to eval.json via MCP phase/log

Gate check: `mcp__plugin_dev-team_dev-team__phase/check` with phase `01-proposal`.

The skill SHALL support the following input modes:
- kebab-case arg → validate, create change if not exists
- Description arg (Chinese/spaces/natural language) → derive kebab-case, confirm, scaffold
- No arg + explore context → extract decisions, derive kebab-case, scaffold
- No arg + no context → ask user what to build

#### Scenario: phase-proposal executes P→E loop
- **WHEN** user invokes `/dev-team:phase-proposal <change-name>`
- **THEN** the skill runs gate check `mcp__plugin_dev-team_dev-team__phase/check` with phase `01-proposal`
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

The skill SHALL be a thin orchestration loop with NO hardcoded phase knowledge. All phase sequencing, agent assignment, and prompt generation SHALL be delegated to `mcp__plugin_dev-team_dev-team__phase/next`.

**Skill logic (pseudocode):**
```
1. Assemble context: parse change name, detect explore context
2. Scaffold change if needed (openspec_new_change)
3. Loop:
   result = MCP phase/next(change, workflow_type="requirement")
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
- Handle backtrack logic (phase/next handles it server-side)

#### Scenario: workflow-requirement calls phase/next in a loop
- **WHEN** user invokes `/dev-team:workflow-requirement <change-name>`
- **THEN** the skill assembles context (Step 0) and scaffolds the change if needed
- **AND** enters a loop calling `phase/next` before each phase
- **AND** invokes the planner and evaluator agents returned by `phase/next`
- **AND** breaks the loop when `phase/next` returns `done: true`
- **AND** displays completion summary and stops（不自动 archive）

#### Scenario: workflow-requirement handles phase/next errors
- **WHEN** `phase/next` returns `error: "round_limit_exceeded"` or `error: "max_retries_exceeded"`
- **THEN** the workflow SHALL stop and display the error message
- **AND** PushNotification SHALL be sent

#### Scenario: workflow-requirement delegates backtrack to phase/next
- **WHEN** a phase evaluator sets `backtrack_to` in eval.json
- **THEN** on the next loop iteration, `phase/next` detects the backtrack and returns the target phase
- **AND** the workflow skill does NOT handle backtrack itself — it simply calls the agents returned by `phase/next`

#### Scenario: workflow-requirement inherits explore context
- **WHEN** user invokes `/dev-team:workflow-requirement` without argument after an explore session
- **THEN** Step 0 detects explore context in the conversation
- **AND** extracts key decisions, diagrams, and "What We Figured Out" summaries
- **AND** derives a kebab-case change name
- **AND** passes the explore context to proposal-planner by appending it to the prompt returned by `phase/next`

#### Scenario: workflow-requirement handles no-op test phases via phase/next
- **WHEN** Phase 06 (unit-test) or Phase 08 (integration-test) detects no test files
- **THEN** the executor agent writes a skipped entry to eval.json via MCP phase/log with `skipped: true`
- **AND** on the next loop iteration, `phase/next` sees the pass (skipped) entry and returns the next phase
- **AND** the workflow skill does NOT implement special no-op handling

#### Scenario: workflow-requirement resumes incomplete change
- **WHEN** user invokes `/dev-team:workflow-requirement <existing-change>` for a change that already has partial eval.json entries
- **THEN** the workflow detects the change already exists and does NOT re-scaffold
- **AND** `phase/next` returns the first unpassed phase based on eval.json state
- **AND** the workflow resumes from that phase

#### Scenario: workflow-requirement resumes after interruption
- **WHEN** user re-invokes `/dev-team:workflow-requirement <change>` after a previous run was interrupted
- **THEN** already-passed phases are automatically skipped by `phase/next`
- **AND** execution resumes from the first incomplete phase

#### Scenario: workflow-requirement reports progress
- **WHEN** the workflow loop executes
- **THEN** after each iteration, the skill SHALL output: `[Round {result.round}/20] [Phase {result.phase_index}/{result.total_phases}] {result.next_phase}: executed`
- **AND** upon `phase/next` returning `done: true` or `error`, PushNotification SHALL be sent

### Requirement: phase-dev-design skill
The system SHALL provide `dev-team:phase-dev-design` skill at `skills/phase-dev-design/SKILL.md` with name `phase-dev-design` and gate check phase `02-dev-design`.

The skill SHALL invoke dev-design-planner (writes design.md + tasks.md) and dev-design-evaluator (evaluates against proposal.md) in a P→E loop.

Gate check: `dev-team eval-check --change "<name>" --phase 02-dev-design`

#### Scenario: phase-dev-design gate check
- **WHEN** user invokes `/dev-team:phase-dev-design <name>`
- **THEN** the skill runs `dev-team eval-check --change "<name>" --phase 02-dev-design`
- **AND** requires prior phase [01-proposal] to have pass record

### Requirement: phase-test-design uses updated phase code
The system SHALL use `03-test-design` (formerly `02-test-design`) as the phase identifier for the test-design phase.

Gate check: `dev-team eval-check --change "<name>" --phase 03-test-design`

The test-design-planner SHALL read `design.md` (produced by 02-dev-design) in addition to `proposal.md` as input for determining test scope and strategy.

#### Scenario: phase-test-design gate check
- **WHEN** user invokes `/dev-team:phase-test-design <name>`
- **THEN** the skill runs `dev-team eval-check --change "<name>" --phase 03-test-design`
- **AND** requires prior phases [01-proposal, 02-dev-design] to have pass records

#### Scenario: test-design-planner reads design.md
- **WHEN** test-design-planner is invoked
- **THEN** it reads `openspec/changes/<name>/design.md` as input for architecture context
- **AND** generates test scope and strategy informed by the design's architecture, data flow, and route design

### Requirement: phase skills use hierarchical MCP tool names
All phase skill SKILL.md files SHALL reference MCP tools using the hierarchical `xx/yy` format. No skill file SHALL contain references to the deprecated flat-format names (`eval_check`, `eval_log`).

The following files SHALL be updated:

| Skill File | Old Reference | New Reference |
|------------|---------------|---------------|
| `skills/phase-proposal/SKILL.md` | `eval_check` | `phase/check` |
| `skills/phase-dev-design/SKILL.md` | `eval_check` | `phase/check` |
| `skills/phase-test-design/SKILL.md` | `eval_check` | `phase/check` |
| `skills/phase-test-gen/SKILL.md` | `eval_check` | `phase/check` |
| `skills/phase-implement/SKILL.md` | `eval_check` | `phase/check` |
| `skills/phase-unit-test/SKILL.md` | `eval_log` | `phase/log` |
| `skills/phase-code-review/SKILL.md` | `eval_check` | `phase/check` |
| `skills/phase-integration-test/SKILL.md` | `eval_log` | `phase/log` |
| `skills/phase-acceptance/SKILL.md` | `eval_check` | `phase/check` |
| `skills/openspec-archive-change/SKILL.md` | `eval_check` | `phase/check` |

#### Scenario: phase-proposal gate check uses phase/check
- **WHEN** reading `skills/phase-proposal/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__phase/check` (not `eval_check`)

#### Scenario: phase-unit-test evaluator uses phase/log
- **WHEN** reading `skills/phase-unit-test/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__phase/log` (not `eval_log`)

## REMOVED Requirements

### Requirement: phase-requirements skill
**Reason**: 重构为 phase-proposal（P→E 模式，使用 proposal-planner + proposal-evaluator 子代理替代主模型直接写入）。
**Migration**: 用户改用 `/dev-team:phase-proposal` 替代 `/dev-team:phase-requirements`。目录 `skills/phase-requirements/` 删除。

### Requirement: phase-dev-proposal skill
**Reason**: 重命名为 phase-dev-design，职责不变（产出 design.md + tasks.md）。
**Migration**: 用户改用 `/dev-team:phase-dev-design` 替代 `/dev-team:phase-dev-proposal`。目录 `skills/phase-dev-proposal/` 重命名为 `skills/phase-dev-design/`。

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

#### Scenario: DESIGN skill executes Planner->Evaluator with phase/check gate
- **WHEN** user invokes `dev-team:phase-proposal`, `dev-team:phase-dev-design`, or `dev-team:phase-test-design`
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__phase/check` with `change` and `phase` arguments to validate prior phases
- **AND** invokes Planner subagent with one-line prompt
- **AND** then invokes the corresponding Evaluator subagent
- **AND** the skill reads the latest eval.json entry to determine verdict
- **AND** loops back to Planner if verdict is "fail" (up to max attempts)

#### Scenario: Skill 遵循精简编排器模式调用 phase/check
- **WHEN** 用户调用任意一个精简后的阶段技能
- **THEN** 在执行业务逻辑之前，技能先调用 `mcp__plugin_dev-team_dev-team__phase/check` with change="<name>" and phase="<phase-code>"
- **AND** 如果 phase/check 返回 `passed: false`，技能输出错误信息并停止
- **AND** Evaluator subagent 从自己的 agent.md 获取完整的 checklist 和 phase/log MCP tool 调用指令

#### Scenario: Skill 在 evaluator 完成后读取 eval.json 判断 verdict
- **WHEN** Evaluator subagent 执行完成
- **THEN** 技能读取 `openspec/changes/<name>/eval.json` 获取当前阶段的最新条目
- **AND** 如果 verdict 为 "fail"，重新执行业务逻辑（主 agent 或 subagent，最多 5 次尝试）
- **AND** 如果 verdict 为 "pass" 或 达到最大尝试次数，技能输出结果报告

#### Scenario: phase-proposal gate check uses phase/check
- **WHEN** reading `skills/phase-proposal/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__phase/check` (not `eval_check`)

## Module Contract

### Skill Files (`plugins/dev-team/skills/`)

| Skill | MCP Tools Used | Agent (Planner) | Agent (Evaluator) | Contract |
|-------|---------------|-----------------|-------------------|----------|
| phase-proposal | phase/check | proposal-planner | proposal-evaluator | P→E loop, phase `01-proposal` |
| phase-dev-design | phase/check | dev-design-planner | dev-design-evaluator | P→E loop, phase `02-dev-design` |
| phase-test-design | phase/check | test-design-planner | test-design-evaluator | P→E loop, phase `03-test-design` |
| phase-test-gen | phase/check | test-gen-generator | test-gen-evaluator | G→E loop, phase `04-test-gen` |
| phase-implement | phase/check | implementation-generator | implementation-evaluator | G→E + AUTO, phase `05-implement` |
| phase-unit-test | phase/log | unit-test-executor | unit-test-evaluator | EXEC loop, phase `06-unit-test` |
| phase-code-review | phase/check | (none) | code-review-evaluator | EVAL-ONLY, phase `07-code-review` |
| phase-integration-test | phase/log | integration-test-executor | integration-test-evaluator | EXEC loop, phase `08-integration-test` |
| phase-acceptance | phase/check | (none) | acceptance-evaluator | EVAL-ONLY, phase `09-acceptance` |
| workflow-requirement | phase/next, phase/log | (from phase/next response) | (from phase/next response) | Thin loop: call phase/next → invoke returned agents → repeat; no hardcoded phase table |
| openspec-archive-change | phase/check | (none) | (none) | Archive completed change |
