## MODIFIED Requirements

### Requirement: phase-proposal skill
The system SHALL provide `dev-team:phase-proposal` skill at `skills/phase-proposal/SKILL.md` with name `phase-proposal`.

The skill SHALL follow the DESIGN P→E pattern:
- **Planner**: `proposal-planner` sub-agent writes proposal.md + specs/ to disk
- **Evaluator**: `proposal-evaluator` sub-agent evaluates against checklist, appends to eval.json via MCP phase_log

Gate check: `mcp__plugin_dev-team_dev-team__phase_check` with phase `01-proposal`.

The skill SHALL support the following input modes:
- kebab-case arg => validate, create change if not exists
- Description arg (Chinese/spaces/natural language) => derive kebab-case, confirm, scaffold
- No arg + explore context => extract decisions, derive kebab-case, scaffold
- No arg + no context => ask user what to build

#### Scenario: phase-proposal executes P→E loop
- **WHEN** user invokes `/dev-team:phase-proposal <change-name>`
- **THEN** the skill runs gate check `mcp__plugin_dev-team_dev-team__phase_check` with phase `01-proposal`
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

The skill SHALL be a thin orchestration loop with NO hardcoded phase knowledge. All phase sequencing, agent assignment, and prompt generation SHALL be delegated to `mcp__plugin_dev-team_dev-team__phase_next`.

**Skill logic (pseudocode):**
```
1. Assemble context: parse change name, detect explore context
2. Scaffold change if needed (openspec_new_change)
3. Loop:
   result = MCP phase_next(change, workflow_type="requirement")
   if result.error -> report, STOP
   if result.done -> break
   if result.planner -> Agent(result.planner.agent_type, result.planner.prompt)
   for step in result.auto_steps -> Bash(step.command)
   Agent(result.evaluator.agent_type, result.evaluator.prompt)
   output: "[Round R/20] [Phase N/9] {phase}: executed"
4. All pass, stop, notify user to archive manually
```

The skill SHALL NOT:
- Contain a hardcoded phase table
- Know which sub-agent to invoke for which phase
- Generate any prompt strings for sub-agents
- Handle backtrack logic (phase_next handles it server-side)

#### Scenario: workflow-requirement calls phase_next in a loop
- **WHEN** user invokes `/dev-team:workflow-requirement <change-name>`
- **THEN** the skill assembles context (Step 0) and scaffolds the change if needed
- **AND** enters a loop calling `phase_next` before each phase
- **AND** invokes the planner and evaluator agents returned by `phase_next`
- **AND** breaks the loop when `phase_next` returns `done: true`
- **AND** displays completion summary and stops

#### Scenario: workflow-requirement handles phase_next errors
- **WHEN** `phase_next` returns `error: "round_limit_exceeded"` or `error: "max_retries_exceeded"`
- **THEN** the workflow SHALL stop and display the error message
- **AND** PushNotification SHALL be sent

#### Scenario: workflow-requirement delegates backtrack to phase_next
- **WHEN** a phase evaluator sets `backtrack_to` in eval.json
- **THEN** on the next loop iteration, `phase_next` detects the backtrack and returns the target phase
- **AND** the workflow skill does NOT handle backtrack itself -- it simply calls the agents returned by `phase_next`

#### Scenario: workflow-requirement inherits explore context
- **WHEN** user invokes `/dev-team:workflow-requirement` without argument after an explore session
- **THEN** Step 0 detects explore context in the conversation
- **AND** extracts key decisions, diagrams, and "What We Figured Out" summaries
- **AND** derives a kebab-case change name
- **AND** passes the explore context to proposal-planner by appending it to the prompt returned by `phase_next`

#### Scenario: workflow-requirement handles no-op test phases via phase_next
- **WHEN** Phase 06 (unit-test) or Phase 08 (integration-test) detects no test files
- **THEN** the executor agent writes a skipped entry to eval.json via MCP phase_log with `skipped: true`
- **AND** on the next loop iteration, `phase_next` sees the pass (skipped) entry and returns the next phase
- **AND** the workflow skill does NOT implement special no-op handling

#### Scenario: workflow-requirement resumes incomplete change
- **WHEN** user invokes `/dev-team:workflow-requirement <existing-change>` for a change that already has partial eval.json entries
- **THEN** the workflow detects the change already exists and does NOT re-scaffold
- **AND** `phase_next` returns the first unpassed phase based on eval.json state
- **AND** the workflow resumes from that phase

#### Scenario: workflow-requirement resumes after interruption
- **WHEN** user re-invokes `/dev-team:workflow-requirement <change>` after a previous run was interrupted
- **THEN** already-passed phases are automatically skipped by `phase_next`
- **AND** execution resumes from the first incomplete phase

#### Scenario: workflow-requirement reports progress
- **WHEN** the workflow loop executes
- **THEN** after each iteration, the skill SHALL output: `[Round {result.round}/20] [Phase {result.phase_index}/{result.total_phases}] {result.next_phase}: executed`
- **AND** upon `phase_next` returning `done: true` or `error`, PushNotification SHALL be sent

### Requirement: phase skills use underscore MCP tool names
All phase skill SKILL.md files SHALL reference MCP tools using the underscore `xx_yy` format. No skill file SHALL contain references to the deprecated slash-format names (`phase/check`, `phase/log`, `phase/next`, `archi/query`, `archi/validate`, `archi/write`, `archi/check`).

The following tool references SHALL be updated from slash to underscore format:

| Domain | Old Reference (xx/yy) | New Reference (xx_yy) |
|--------|-----------------------|----------------------|
| Phase gate | `mcp__plugin_dev-team_dev-team__phase/check` | `mcp__plugin_dev-team_dev-team__phase_check` |
| Phase log | `mcp__plugin_dev-team_dev-team__phase/log` | `mcp__plugin_dev-team_dev-team__phase_log` |
| Phase next | `mcp__plugin_dev-team_dev-team__phase/next` | `mcp__plugin_dev-team_dev-team__phase_next` |
| Archi query | `mcp__plugin_dev-team_dev-team__archi/query` | `mcp__plugin_dev-team_dev-team__archi_query` |
| Archi validate | `mcp__plugin_dev-team_dev-team__archi/validate` | `mcp__plugin_dev-team_dev-team__archi_validate` |
| Archi write | `mcp__plugin_dev-team_dev-team__archi/write` | `mcp__plugin_dev-team_dev-team__archi_write` |
| Archi check | `mcp__plugin_dev-team_dev-team__archi/check` | `mcp__plugin_dev-team_dev-team__archi_check` |

#### Scenario: phase-proposal gate check uses phase_check
- **WHEN** reading `skills/phase-proposal/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__phase_check` (not `phase/check`)

#### Scenario: phase-unit-test evaluator uses phase_log
- **WHEN** reading `skills/phase-unit-test/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__phase_log` (not `phase/log`)

#### Scenario: workflow-requirement uses phase_next
- **WHEN** reading `skills/workflow-requirement/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__phase_next` (not `phase/next`)

### Requirement: Nine user-triggered phase skills (tool name update)
The system SHALL provide 9 phase skills. All references to MCP tools within these skills SHALL use `xx_yy` underscore format.

Updated skill list with MCP tool references:

| # | Skill | MCP Tools Used (new) |
|---|-------|---------------------|
| 1 | dev-team:phase-proposal | phase_check |
| 2 | dev-team:phase-dev-design | phase_check |
| 3 | dev-team:phase-test-design | phase_check |
| 4 | dev-team:phase-test-gen | phase_check |
| 5 | dev-team:phase-implement | phase_check |
| 6 | dev-team:phase-unit-test | phase_log |
| 7 | dev-team:phase-code-review | phase_check |
| 8 | dev-team:phase-integration-test | phase_log |
| 9 | dev-team:phase-acceptance | phase_check |

#### Scenario: DESIGN skill executes Planner->Evaluator with phase_check gate
- **WHEN** user invokes `dev-team:phase-proposal`, `dev-team:phase-dev-design`, or `dev-team:phase-test-design`
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__phase_check` with `change` and `phase` arguments to validate prior phases
- **AND** invokes Planner subagent with one-line prompt
- **AND** then invokes the corresponding Evaluator subagent
- **AND** the skill reads the latest eval.json entry to determine verdict
- **AND** loops back to Planner if verdict is "fail" (up to max attempts)

#### Scenario: Skill follows thin orchestrator pattern calling phase_check
- **WHEN** a thin phase skill is invoked
- **THEN** before executing business logic, the skill calls `mcp__plugin_dev-team_dev-team__phase_check` with `change=<name>` and `phase=<phase-code>`
- **AND** if phase_check returns `passed: false`, the skill outputs an error and stops
- **AND** Evaluator subagent gets full checklist and phase_log MCP tool invocation instructions from its own agent.md

#### Scenario: Skill reads eval.json after evaluator completes
- **WHEN** Evaluator subagent finishes execution
- **THEN** the skill reads `openspec/changes/<name>/eval.json` for the latest entry of the current phase
- **AND** if verdict is "fail", re-executes business logic (main agent or subagent, max 5 attempts)
- **AND** if verdict is "pass" or max attempts reached, the skill outputs a result report

## ADDED Requirements

### Requirement: phase-test-design skill delegates scenario-oriented behavior to agent
The phase-test-design skill SHALL remain a thin P→E orchestrator. All Forward/Reverse AC categorization logic SHALL reside in the test-design-planner agent definition, not in the skill itself. The planner SHALL grep source code to extract real API signatures (function names, parameter types, return types) as supplementary input alongside proposal.md and design.md, but SHALL NOT output parameter types or risk markers in test-design.md.

#### Scenario: phase-test-design prompt unchanged, agent respects no-type constraint
- **WHEN** phase-test-design invokes test-design-planner
- **THEN** the skill uses the existing one-line prompt: `"Write test-design.md for change '<name>'."`
- **AND** the agent greps source code to extract real API signatures as supplementary input
- **AND** the agent does NOT output parameter type tables or risk markers in test-design.md
- **AND** the skill does NOT pass any additional source-file-related context in the prompt

#### Scenario: phase-test-design gate check remains unchanged
- **WHEN** phase-test-design executes
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__phase_check` with `change` and `phase="03-test-design"`
- **AND** the gate check logic is unaffected by the agent-level changes

### Requirement: phase-test-gen skill delegates colocated output and edge case generation to agent
The phase-test-gen skill SHALL remain a thin G→E orchestrator. All source code reading, test file colocation, and edge case generation logic SHALL reside in the test-gen-generator agent definition, not in the skill itself.

#### Scenario: phase-test-gen prompt unchanged with code-aware generator
- **WHEN** phase-test-gen invokes test-gen-generator
- **THEN** the skill uses the existing one-line prompt: `"Generate test files for change '<name>'."`
- **AND** the agent handles all source code reading, colocated file placement, and edge case generation internally
- **AND** the skill does NOT specify an output directory in the prompt

#### Scenario: phase-test-gen evaluator checklist driven by agent definition
- **WHEN** phase-test-gen invokes test-gen-evaluator
- **THEN** the skill uses the existing one-line prompt: `"Evaluate generated test code for change '<name>' against test-design.md. Append result to eval.json."`
- **AND** the evaluator reads its updated checklist (G1, G2) from the agent definition
- **AND** the skill does NOT hardcode checklist items

#### Scenario: phase-test-gen gate check remains unchanged
- **WHEN** phase-test-gen executes
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__phase_check` with `change` and `phase="04-test-gen"`
- **AND** the gate check logic is unaffected by the agent-level changes

## Module Contract

### Skill Files (`plugins/dev-team/skills/`)

| Skill | MCP Tools Used (new) | Agent (Planner) | Agent (Evaluator) | Contract |
|-------|----------------------|-----------------|-------------------|----------|
| phase-proposal | phase_check | proposal-planner | proposal-evaluator | P→E loop, phase `01-proposal` |
| phase-dev-design | phase_check | dev-design-planner | dev-design-evaluator | P→E loop, phase `02-dev-design` |
| phase-test-design | phase_check | test-design-planner | test-design-evaluator | P→E loop, phase `03-test-design`. Agent handles Forward/Reverse AC categorization from proposal.md + design.md + Grep source code. |
| phase-test-gen | phase_check | test-gen-generator | test-gen-evaluator | G→E loop, phase `04-test-gen`. Agent handles colocated output and edge case generation. |
| phase-implement | phase_check | implementation-generator | implementation-evaluator | G→E + AUTO, phase `05-implement` |
| phase-unit-test | phase_log | unit-test-executor | unit-test-evaluator | EXEC loop, phase `06-unit-test` |
| phase-code-review | phase_check | (none) | code-review-evaluator | EVAL-ONLY, phase `07-code-review` |
| phase-integration-test | phase_log | integration-test-executor | integration-test-evaluator | EXEC loop, phase `08-integration-test` |
| phase-acceptance | phase_check | (none) | acceptance-evaluator | EVAL-ONLY, phase `09-acceptance` |
| workflow-requirement | phase_next, phase_log | (from phase_next response) | (from phase_next response) | Thin loop: call phase_next -> invoke returned agents -> repeat; no hardcoded phase table |
| openspec-archive-change | phase_check | (none) | (none) | Archive completed change |

### Design Rationale

The thin orchestrator pattern ensures that behavioral changes to test phases are contained within agent definitions rather than propagated to skill files. This keeps the skill layer stable and allows agent-level iteration without modifying the orchestration. Both skills pass generic one-line prompts; all domain logic about Forward/Reverse AC categorization (test-design), source code reading and parameter type extraction (test-gen), test file placement (test-gen), and edge case generation (test-gen) lives in the respective agent definitions.

## ADDED Requirements

### Requirement: workflow-test-only skill
The system SHALL provide `dev-team:workflow-test-only` skill at `skills/workflow-test-only/SKILL.md` with name `workflow-test-only`.

The skill description SHALL indicate it orchestrates the test-only PGE workflow for supplementing test coverage on already-implemented code.

The skill SHALL follow the same change resolution logic (Step 0) and scaffold logic (Step 1) as `workflow-requirement`.

After scaffolding a new change (Step 1), the skill SHALL write `{"workflow_type": "test-only"}` to `openspec/changes/<name>/workflow.json`. This is the single source of truth — the skill SHALL NOT pass `workflow_type` to MCP tools.

The orchestration loop (Step 2) SHALL call:
```
mcp__plugin_dev-team_dev-team__phase_next(change=<name>)
```

When an evaluator returns after discovering production code bugs (eval entry with `verdict: "fail"`, `backtrack_to: null`, and bug details in report), the skill SHALL:
1. Write a bug report to `openspec/changes/<name>/reports/` (e.g. `code-bugs-found.md` or JSON)
2. Notify the user that tests discovered code bugs in the production code
3. Allow the user to decide whether to continue the workflow (e.g. proceed to integration-test) or terminate

The completion step (Step 3) SHALL report `total_phases: 6` and note that discovering implementation bugs via tests fulfills the test-only workflow purpose before manual archive.

The skill SHALL NOT auto-archive.

#### Scenario: workflow-test-only skill executes six-phase loop (AC-8)
- **WHEN** user invokes `/dev-team:workflow-test-only <change-name>`
- **THEN** the skill enters a loop calling `phase_next(change=<name>)` without `workflow_type`
- **AND** executes all phases returned by `phase_next` until `done: true`
- **AND** the skill file contains no hardcoded phase table or agent names

#### Scenario: workflow-test-only writes workflow.json on scaffold (AC-19)
- **WHEN** `workflow-test-only` creates a new change in Step 1
- **THEN** it writes `{"workflow_type": "test-only"}` to `workflow.json`
- **AND** subsequent `phase_next(change)` calls resolve the test-only phase table from that file

#### Scenario: workflow-test-only skill file does not pass workflow_type to MCP
- **WHEN** reading `skills/workflow-test-only/SKILL.md`
- **THEN** the frontmatter `name` is `workflow-test-only`
- **AND** the loop calls `phase_next(change=<name>)` without a `workflow_type` argument
- **AND** no `phase_log` call includes `workflow_type`

#### Scenario: workflow-test-only uses underscore MCP tool names
- **WHEN** reading `skills/workflow-test-only/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__phase_next` (not `phase/next`)

#### Scenario: workflow-test-only completion does not reference acceptance
- **WHEN** the skill reaches Step 3 (Completion)
- **THEN** the completion message does NOT mention phase `09-acceptance`
- **AND** instructs user to run `/dev-team:openspec-archive-change` manually

#### Scenario: workflow-test-only inherits explore context like workflow-requirement
- **WHEN** user invokes `/dev-team:workflow-test-only` without argument after an explore session
- **THEN** Step 0 detects explore context and derives a kebab-case change name
- **AND** explore context is available for the proposal phase via phase_next prompt handling

#### Scenario: workflow generates bug report when tests find code bugs (AC-15)
- **WHEN** the evaluator returns after logging `verdict: "fail"`, `backtrack_to: null` with code bug details in report
- **THEN** the skill writes a bug report under `openspec/changes/<name>/reports/`
- **AND** notifies the user that tests discovered production code bugs
- **AND** prompts the user to continue (e.g. to integration-test) or terminate the workflow

#### Scenario: user may continue workflow after unit-test bug discovery
- **WHEN** unit-test phase discovers code bugs and the user chooses to continue
- **THEN** the skill resumes the phase_next loop
- **AND** may proceed to `08-integration-test` per prerequisite DAG

### Requirement: workflow-requirement skill writes workflow.json on scaffold
`plugins/dev-team/skills/workflow-requirement/SKILL.md` SHALL be updated to write `{"workflow_type": "requirement"}` to `workflow.json` when scaffolding a new change, and its orchestration loop SHALL call `phase_next(change=<name>)` without a `workflow_type` parameter.

#### Scenario: workflow-requirement no longer passes workflow_type to phase_next
- **WHEN** reading `skills/workflow-requirement/SKILL.md`
- **THEN** the loop calls `phase_next(change=<name>)` without `workflow_type`
- **AND** Step 1 writes `{"workflow_type": "requirement"}` to `workflow.json` for new changes

