## ADDED Requirements

### Requirement: Change directory workflow_type in workflow.json
Each change directory SHALL store its workflow type in `openspec/changes/<name>/workflow.json` as the single source of truth:

```json
{
  "workflow_type": "requirement | bug-fix | refactor | test-only"
}
```

Rules:
1. `workflow-*` skills SHALL write `workflow.json` when scaffolding a new change (Step 1)
2. `phase_next` and `phase_log` SHALL read `workflow_type` from this file — they SHALL NOT accept `workflow_type` as an MCP parameter
3. When `workflow.json` does not exist or `workflow_type` is absent, server-side logic SHALL default to `"requirement"` for backward compatibility
4. A change directory SHOULD NOT modify `workflow.json` after workflow execution has begun

#### Scenario: workflow-test-only writes workflow.json on scaffold (AC-19)
- **WHEN** `workflow-test-only` skill creates a new change via `openspec new change`
- **THEN** it writes `{"workflow_type": "test-only"}` to `workflow.json` before entering the orchestration loop

#### Scenario: workflow-requirement writes workflow.json on scaffold
- **WHEN** `workflow-requirement` skill creates a new change
- **THEN** it writes `{"workflow_type": "requirement"}` to `workflow.json`

#### Scenario: phase_next resolves workflow from file not parameter (AC-17)
- **WHEN** inspecting `phaseNextInputSchema`
- **THEN** it contains only `change` as input (no `workflow_type`)
- **AND** `runPhaseNext()` calls `getWorkflowType(change)` to select the phase table (reading from `workflow.json`)

## MODIFIED Requirements

### Requirement: phase_next MCP tool
The system SHALL provide `mcp__plugin_dev-team_dev-team__phase_next` MCP tool that returns the next phase to execute, its agent assignments, and prompt strings.

**Input:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change` | string | true | Change name |

The MCP tool SHALL NOT accept `workflow_type` as an input parameter. Server-side logic SHALL read `workflow_type` from `openspec/changes/<change>/workflow.json` via `getWorkflowType(change)`, defaulting to `"requirement"` when the file is absent or the field is missing.

**`workflow_type` phase tables (server-side):**

| workflow_type | Phases |
|---------------|--------|
| `requirement` | 01-proposal, 02-dev-design, 03-test-design, 05-implement, 04-test-gen, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance |
| `bug-fix` | 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance |
| `refactor` | 01-proposal, 02-dev-design, 03-test-design, 05-implement, 04-test-gen, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance |
| `test-only` | 01-proposal, 02-code-analyze, 03-test-design, 04-test-gen, 06-unit-test, 08-integration-test |

Phase progression, prerequisite enforcement, completion semantics, and prompt customization for `test-only` SHALL follow the `test-only-workflow` capability spec.

**Key change from previous version:** Entry deletion via `clearEntriesFromPhase()` is REMOVED. Stale marking and propagation are handled by `markPhaseStale()`, called by `phase_log` when writing entries with `backtrack_to`. `phase_next` only reads and filters — it never modifies eval.json. `phase_check` is no longer called in the workflow loop.

#### Scenario: phase_next returns first phase on initial call
- **WHEN** `phase_next` is called with a change that has no eval.json entries
- **THEN** it returns `next_phase: "01-proposal"` with `planner.agent_type: "dev-team:proposal-planner"` and `evaluator.agent_type: "dev-team:proposal-evaluator"`
- **AND** `done: false`

#### Scenario: phase_next returns next phase after pass
- **WHEN** `phase_next` is called after phase 01-proposal has a non-stale pass entry in eval.json
- **THEN** it returns `next_phase: "02-dev-design"` with `planner.agent_type: "dev-team:dev-design-planner"`

#### Scenario: phase_next returns same phase for retry after fail
- **WHEN** `phase_next` is called after phase 01-proposal has a fail entry with attempt < 5
- **THEN** it returns `next_phase: "01-proposal"` (same phase for retry)
- **AND** increments `round`

#### Scenario: phase_next returns error on max retries
- **WHEN** `phase_next` is called after phase 01-proposal has 5 consecutive fail entries
- **THEN** it returns `error: "max_retries_exceeded"` with descriptive message

#### Scenario: phase_next handles backtrack (target marked stale by phase_log)
- **WHEN** `phase_next` is called after phase_log wrote a fail entry with `backtrack_to: "05-implement"`, and the latest pass entry for 05-implement was marked stale
- **THEN** it returns `next_phase: "05-implement"` (the earliest backtrack target)
- **AND** does NOT delete any eval.json entries
- **AND** increments `round`

#### Scenario: phase_next handles array backtrack_to (targets marked stale by phase_log)
- **WHEN** `phase_next` is called after phase_log wrote a fail entry with `backtrack_to: ["02-dev-design", "03-test-design"]`, and both 02 and 03 latest pass entries were marked stale
- **THEN** it returns `next_phase: "02-dev-design"` (the earliest target in phase table order)
- **AND** does NOT delete any eval.json entries

#### Scenario: phase_next skips stale entries to find next valid phase
- **WHEN** `phase_next` is called, and eval.json has: 01✓(not stale), 02✓(stale:true, after backtrack and redo), 03✓(not stale), 04✓(not stale), 05✓(stale:true)
- **THEN** it returns `next_phase: "02-dev-design"` (first phase without valid pass entry)
- **AND** 03 and 04 are NOT returned (they still have valid pass entries)

#### Scenario: phase_next returns done when all phases pass
- **WHEN** `phase_next` is called after all 9 phases have valid (non-stale) pass entries in eval.json
- **THEN** it returns `done: true`

#### Scenario: phase_next resumes from partial completion after test-design
- **WHEN** `phase_next` is called for a change that has pass entries for phases 01-03 but no entries for phase 05 onward
- **THEN** it returns `next_phase: "05-implement"` (implement before test-gen)
- **AND** phases 01-03 are NOT re-executed

#### Scenario: phase_next returns test-gen after implement pass
- **WHEN** `phase_next` is called for a change that has pass entries for phases 01-03 and 05-implement but no valid pass for 04-test-gen
- **THEN** it returns `next_phase: "04-test-gen"`
- **AND** phases 01-03 and 05 are NOT re-executed

#### Scenario: phase_next prefers implement when test-gen prerequisite missing
- **WHEN** `phase_next` is called, 04-test-gen has valid pass but 05-implement has only stale pass
- **THEN** it returns `next_phase: "05-implement"` (04 requires 05; stale 05 blocks downstream 04 validity for 06-unit-test)

#### Scenario: phase_next returns implement when dev-design passes (parallel with test-design)
- **WHEN** `phase_next` is called, 01-proposal and 02-dev-design have valid pass, and neither 03-test-design nor 05-implement has passed
- **THEN** it returns `next_phase: "03-test-design"` (earlier in phase table)
- **AND** after 03 passes, if 05 has not passed, it returns `05-implement` before `04-test-gen`

#### Scenario: phase_next handles mid-phase interruption
- **WHEN** `phase_next` is called for a change where phase 03 has a planner-run entry but no evaluator verdict
- **THEN** it returns `next_phase: "03-test-design"` (re-execute from planner)
- **AND** the incomplete entry is treated as if the phase hasn't been evaluated yet

#### Scenario: phase_next returns error on round limit
- **WHEN** `phase_next` is called with `round` > 20
- **THEN** it returns `error: "round_limit_exceeded"`

#### Scenario: phase_next returns evaluator-only phase
- **WHEN** `phase_next` returns phase 07-code-review or 09-acceptance
- **THEN** `planner` is `null`
- **AND** `evaluator` contains the agent_type and prompt

#### Scenario: phase_next respects workflow_type from workflow.json for bug-fix
- **WHEN** change `workflow.json` has `{"workflow_type": "bug-fix"}`
- **AND** `phase_next(change)` is called
- **THEN** the phase sequence is: 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance
- **AND** phases 03-test-design, 04-test-gen, 08-integration-test are omitted

#### Scenario: phase_next handles string backtrack_to for backward compatibility
- **WHEN** `phase_next` detects `backtrack_to: "02-dev-design"` (string, not array) in the latest eval entry
- **THEN** it processes it identically to `["02-dev-design"]` (single-element array)
- **AND** returns the target phase as `next_phase`

#### Scenario: phase_next delegates test-only behavior to test-only-workflow spec
- **WHEN** change `workflow.json` has `{"workflow_type": "test-only"}`
- **AND** `phase_next(change)` is called
- **THEN** phase progression, prerequisites, completion, and prompts SHALL conform to the `test-only-workflow` capability spec
- **AND** requirement, bug-fix, and refactor behavior remain unchanged

#### Scenario: phase_next never receives invalid backtrack entries from test-only workflow
- **WHEN** test-only evaluators attempt backtrack to phases outside the test-only table
- **THEN** `phase_log` rejects the invalid backtrack before writing to eval.json
- **AND** `phase_next` does NOT encounter `invalid_backtrack_target` errors from polluted eval.json

#### Scenario: phase_next defaults to requirement when workflow.json absent (AC-13)
- **WHEN** change has no `workflow.json` file (or the file lacks `workflow_type`)
- **AND** `phase_next(change)` is called
- **THEN** the `requirement` phase table is used

### Requirement: Workflow skill thin loop
The workflow skill SHALL be a thin orchestration loop with no hardcoded phase knowledge. All phase sequencing, agent assignment, and prompt generation SHALL be owned by `phase_next` on the MCP server.

**Workflow skill logic:**
```
1. Assemble context (change name, explore context if available)
2. If change does not exist -> scaffold (openspec_new_change) AND write workflow.json
3. Loop:
   a. result = MCP phase_next(change)
   b. if result.error -> report error, STOP
   c. if result.done -> stop, report completion, PushNotification（用户手动 archive）
   d. if result.planner -> Agent(result.planner.agent_type, result.planner.prompt)
   e. for step in result.auto_steps -> Bash(step.command)
   f. if result.evaluator -> Agent(result.evaluator.agent_type, result.evaluator.prompt)
   g. output: "[Round {result.round}/20] [Phase {result.phase_index}/{result.total_phases}] {result.next_phase}: executed"
4. 全部 phase pass 后停止，显示完成摘要，提醒用户检查后手动执行 `/dev-team:openspec-archive-change`
```

The workflow skill SHALL NOT:
- Contain a hardcoded phase table
- Know which agent to invoke for which phase
- Generate prompts for sub-agents
- Know the phase ordering or sequence
- Pass `workflow_type` to `phase_next` or `phase_log`
- Handle backtrack logic (phase_next handles it server-side)
- Handle stale marking (phase_log handles it server-side)

#### Scenario: Workflow skill is a thin loop
- **WHEN** reading `skills/workflow-requirement/SKILL.md`
- **THEN** it contains no hardcoded phase table
- **AND** it contains no agent name references other than in the phase_next response handling
- **AND** it delegates all sequencing decisions to `phase_next`
- **AND** its loop calls `phase_next(change=<name>)` without `workflow_type`
- **AND** Step 2 writes `workflow.json` (not passing `workflow_type` to MCP)

#### Scenario: Workflow skill handles phase_next error
- **WHEN** `phase_next` returns `error: "round_limit_exceeded"` or `error: "max_retries_exceeded"`
- **THEN** the workflow skill SHALL stop and display the error message
- **AND** PushNotification SHALL be sent

### Requirement: Extensibility for workflow variants
The workflow-orchestration system SHALL support multiple `workflow_type` values stored in `workflow.json`. New workflow variants SHALL be created by:
1. Defining a new phase table in the MCP server for the `workflow_type`
2. Creating a thin skill file that writes the `workflow_type` to `workflow.json` on scaffold and calls `phase_next(change)` without a workflow_type parameter

No skill-level changes to pipeline logic are required for new variants.

Reserved workflow variants:
- `workflow-bug-fix`: Simplified pipeline (skip test-design, test-gen, integration-test)
- `workflow-refactor`: Full pipeline with emphasis on design review
- `workflow-test-only`: Test-only pipeline for existing code (see `test-only-workflow` capability spec)

#### Scenario: New workflow variant requires only skill file plus server phase table
- **WHEN** the `workflow-test-only` skill is created
- **THEN** the skill file is a copy of workflow-requirement with `{"workflow_type": "test-only"}` written to `workflow.json` on scaffold
- **AND** the MCP server has the test-only phase table and prerequisite table defined per `test-only-workflow` spec
- **AND** the orchestration loop calls `phase_next(change)` without passing workflow_type
- **AND** no other skill-level pipeline logic changes are needed

## Module Contract

### MCP Tools

| Tool | Change | Purpose |
|------|--------|---------|
| phase_next | MODIFIED | Remove `workflow_type` from input schema; read from `workflow.json` via `getWorkflowType()` |
| phase_log | MODIFIED | No `workflow_type` input; backtrack validation reads from `workflow.json` (see `pge-workflow-engine` spec) |

### Schemas

| File | Change | Purpose |
|------|--------|---------|
| `phase-next.schema.ts` | MODIFIED | Remove `workflow_type` field from `phaseNextInputSchema` |

### Change Directory

| File | Field | Purpose |
|------|-------|---------|
| `workflow.json` | `workflow_type` | Single source of truth for workflow variant selection |
