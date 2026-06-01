## ADDED Requirements

### Requirement: phase/next MCP tool
The system SHALL provide `mcp__plugin_dev-team_dev-team__phase/next` MCP tool that returns the next phase to execute, its agent assignments, and prompt strings.

**Input:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change` | string | true | Change name |
| `workflow_type` | string | false | `"requirement"` (default), `"bug-fix"`, `"refactor"` |

**Output (phase ready to execute):**
```json
{
  "done": false,
  "error": null,
  "next_phase": "01-proposal",
  "phase_pattern": "DESIGN",
  "planner": {
    "agent_type": "dev-team:proposal-planner",
    "prompt": "Write proposal.md and specs/ for change '<name>'."
  },
  "evaluator": {
    "agent_type": "dev-team:proposal-evaluator",
    "prompt": "Evaluate proposal.md for change '<name>' against checklist."
  },
  "auto_steps": [],
  "total_phases": 9,
  "phase_index": 1,
  "round": 1
}
```

**Output (all phases complete):**
```json
{
  "done": true,
  "error": null,
  "next_phase": null,
  "planner": null,
  "evaluator": null,
  "auto_steps": [],
  "total_phases": 9,
  "phase_index": 9,
  "round": 12
}
```

**Output (error):**
```json
{
  "done": false,
  "error": "round_limit_exceeded",
  "message": "超过 20 轮限制，可能存在循环回溯",
  "next_phase": null,
  "planner": null,
  "evaluator": null
}
```

**Server-side logic:**
1. Read `eval.json` for the change
2. If `backtrack_to` is set in the latest entry: clear eval.json entries from the target phase onward, return the target phase as `next_phase`
3. If last phase verdict is `fail` AND attempts < 5: return the same phase for retry
4. If last phase verdict is `fail` AND attempts >= 5: return `error: "max_retries_exceeded"`
5. If last phase verdict is `pass`: return the next sequential phase per `workflow_type` phase table
6. If all phases pass: return `done: true`
7. If `round` > 20: return `error: "round_limit_exceeded"`
8. EVAL-ONLY phases return `planner: null`

**`workflow_type` phase tables (server-side):**

| workflow_type | Phases |
|---------------|--------|
| `requirement` | 01-proposal, 02-dev-design, 03-test-design, 04-test-gen, 05-implement, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance |
| `bug-fix` | 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance |
| `refactor` | 01-proposal, 02-dev-design, 03-test-design, 04-test-gen, 05-implement, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance |

#### Scenario: phase/next returns first phase on initial call
- **WHEN** `phase/next` is called with a change that has no eval.json entries
- **THEN** it returns `next_phase: "01-proposal"` with `planner.agent_type: "dev-team:proposal-planner"` and `evaluator.agent_type: "dev-team:proposal-evaluator"`
- **AND** `done: false`

#### Scenario: phase/next returns next phase after pass
- **WHEN** `phase/next` is called after phase 01-proposal has a pass entry in eval.json
- **THEN** it returns `next_phase: "02-dev-design"` with `planner.agent_type: "dev-team:dev-design-planner"`

#### Scenario: phase/next returns same phase for retry after fail
- **WHEN** `phase/next` is called after phase 01-proposal has a fail entry with attempt < 5
- **THEN** it returns `next_phase: "01-proposal"` (same phase for retry)
- **AND** increments `round`

#### Scenario: phase/next returns error on max retries
- **WHEN** `phase/next` is called after phase 01-proposal has 5 consecutive fail entries
- **THEN** it returns `error: "max_retries_exceeded"` with descriptive message

#### Scenario: phase/next handles backtrack
- **WHEN** `phase/next` detects `backtrack_to: "02-dev-design"` in the latest eval.json entry
- **THEN** it clears eval.json entries from phase 02-dev-design onward
- **AND** returns `next_phase: "02-dev-design"` with the appropriate planner and evaluator
- **AND** increments `round`

#### Scenario: phase/next returns done when all phases pass
- **WHEN** `phase/next` is called after all 9 phases have pass entries in eval.json
- **THEN** it returns `done: true`

#### Scenario: phase/next resumes from partial completion
- **WHEN** `phase/next` is called for a change that has pass entries for phases 01-03 but no entries for phase 04 onward
- **THEN** it returns `next_phase: "04-test-gen"` (the first unpassed phase)
- **AND** phases 01-03 are NOT re-executed

#### Scenario: phase/next handles mid-phase interruption
- **WHEN** `phase/next` is called for a change where phase 03 has a planner-run entry but no evaluator verdict
- **THEN** it returns `next_phase: "03-test-design"` (re-execute from planner)
- **AND** the incomplete entry is treated as if the phase hasn't been evaluated yet

#### Scenario: phase/next returns error on round limit
- **WHEN** `phase/next` is called with `round` > 20
- **THEN** it returns `error: "round_limit_exceeded"`

#### Scenario: phase/next returns evaluator-only phase
- **WHEN** `phase/next` returns phase 07-code-review or 09-acceptance
- **THEN** `planner` is `null`
- **AND** `evaluator` contains the agent_type and prompt

#### Scenario: phase/next respects workflow_type
- **WHEN** `phase/next` is called with `workflow_type: "bug-fix"`
- **THEN** the phase sequence is: 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance
- **AND** phases 03-test-design, 04-test-gen, 08-integration-test are omitted

### Requirement: Workflow skill thin loop
The workflow skill SHALL be a thin orchestration loop with no hardcoded phase knowledge. All phase sequencing, agent assignment, and prompt generation SHALL be owned by `phase/next` on the MCP server.

**Workflow skill logic:**
```
1. Assemble context (change name, explore context if available)
2. If change does not exist → scaffold (openspec_new_change)
3. Loop:
   a. result = MCP phase/next(change, workflow_type)
   b. if result.error → report error, STOP
   c. if result.done → stop, report completion, PushNotification（用户手动 archive）
   d. if result.planner → Agent(result.planner.agent_type, result.planner.prompt)
   e. for step in result.auto_steps → Bash(step.command)
   f. if result.evaluator → Agent(result.evaluator.agent_type, result.evaluator.prompt)
   g. output: "[Round {result.round}/20] [Phase {result.phase_index}/{result.total_phases}] {result.next_phase}: executed"
4. 全部 phase pass 后停止，显示完成摘要，提醒用户检查后手动执行 `/dev-team:openspec-archive-change`
```

The workflow skill SHALL NOT:
- Contain a hardcoded phase table
- Know which agent to invoke for which phase
- Generate prompts for sub-agents
- Know the phase ordering or sequence
- Handle backtrack logic (phase/next handles it server-side)

#### Scenario: Workflow skill is a thin loop
- **WHEN** reading `skills/workflow-requirement/SKILL.md`
- **THEN** it contains no hardcoded phase table
- **AND** it contains no agent name references other than in the phase/next response handling
- **AND** it delegates all sequencing decisions to `phase/next`

#### Scenario: Workflow skill handles phase/next error
- **WHEN** `phase/next` returns `error: "round_limit_exceeded"` or `error: "max_retries_exceeded"`
- **THEN** the workflow skill SHALL stop and display the error message
- **AND** PushNotification SHALL be sent

#### Scenario: Workflow skill resumes incomplete change
- **WHEN** user invokes `/dev-team:workflow-requirement <existing-change>` for a change with partial eval.json entries
- **THEN** the workflow detects the change already exists and does NOT re-scaffold
- **AND** `phase/next` returns the first unpassed phase based on eval.json
- **AND** the workflow resumes from that phase

#### Scenario: Workflow skill resumes after interruption
- **WHEN** user re-invokes `/dev-team:workflow-requirement <change>` after a previous run was stopped (e.g., max retries, user interrupt)
- **THEN** the workflow picks up from where it left off
- **AND** already-passed phases are skipped via `phase/next` server-side logic
- **AND** the round counter resets to the count from eval.json history

### Requirement: Explore context inheritance
The workflow-orchestration system SHALL detect and extract explore context from the conversation when no explicit change name is provided.

Explore context detection signals:
- "What We Figured Out" summary section
- ASCII decision tables or comparison diagrams
- Architectural analysis (component diagrams, data flow diagrams)
- Explicit exploration conclusions

When explore context is detected, the system SHALL:
1. Extract key decisions and conclusions as EXPLORE_CONTEXT_SUMMARY
2. Derive a kebab-case change name from the explore topic
3. Pass EXPLORE_CONTEXT_SUMMARY to the proposal-planner agent prompt
4. Allow the user to confirm or override the derived change name

#### Scenario: Explore context detected and used
- **WHEN** user invokes workflow-requirement after an explore session
- **THEN** the workflow extracts the problem statement, approach, and decisions
- **AND** derives a change name
- **AND** passes the extracted context to proposal-planner via the prompt from phase/next

#### Scenario: No explore context — ask user
- **WHEN** user invokes workflow-requirement without argument and no explore context is detected
- **THEN** the workflow prompts the user: "想构建什么变更？"
- **AND** derives kebab-case from the user's response

### Requirement: Workflow completion notification
After `phase/next` returns `done: true`, the workflow-orchestration system SHALL stop and notify the user that all phases have passed. Archive is a manual step performed by the user.

The completion step SHALL:
1. Display a completion summary listing all phases and their verdicts
2. Send a PushNotification: "所有 phase 已完成，请检查后手动执行 /dev-team:openspec-archive-change"
3. NOT automatically run `openspec archive` or move the change directory

The user SHALL manually inspect the results and run `/dev-team:openspec-archive-change` when satisfied.

#### Scenario: Workflow stops on completion, user manually archives
- **WHEN** phase/next returns `done: true`
- **THEN** the workflow displays a completion summary and stops
- **AND** sends PushNotification reminding user to manually archive
- **AND** does NOT move the change to openspec/changes/archive/

### Requirement: Extensibility for workflow variants
The workflow-orchestration system SHALL support multiple `workflow_type` values via the `phase/next` interface. New workflow variants SHALL be created by:
1. Defining a new phase table in the MCP server for the `workflow_type`
2. Creating a thin skill file that calls `phase/next` with the appropriate `workflow_type`

No skill-level changes to pipeline logic are required for new variants.

Reserved workflow variants:
- `workflow-bug-fix`: Simplified pipeline (skip test-design, test-gen, integration-test)
- `workflow-refactor`: Full pipeline with emphasis on design review

#### Scenario: New workflow variant requires only skill file + server phase table
- **WHEN** a future `workflow-bug-fix` skill is created
- **THEN** the skill file is a copy of workflow-requirement with `workflow_type: "bug-fix"`
- **AND** the MCP server has the bug-fix phase table defined
- **AND** no other changes are needed
