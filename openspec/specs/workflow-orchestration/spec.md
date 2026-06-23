## ADDED Requirements

### Requirement: Prerequisite dependency table
The workflow layer SHALL define explicit prerequisites for each phase via a new `getPrerequisites(phaseId: string, workflowType?: string): string[]` function in `workflow.ts`. This SHALL replace the implicit "all prior phases" logic in gate-check, timestamp-check, and backtrack-check, while the existing `getPriorPhases()` SHALL be retained for backward compatibility.

The prerequisite table for the `requirement` workflow_type SHALL be:

| Phase | Prerequisites | Notes |
|-------|---------------|-------|
| 01-proposal | [] | Root phase, no dependencies |
| 02-dev-design | [01-proposal] | Only depends on proposal |
| 03-test-design | [01-proposal, 02-dev-design] | Depends on proposal and dev-design |
| 05-implement | [02-dev-design] | Only depends on dev-design; may run in parallel with 03-test-design after 02 passes |
| 04-test-gen | [03-test-design, 05-implement] | Requires both test design AND completed implementation |
| 06-unit-test | [04-test-gen, 05-implement] | Needs both tracks |
| 07-code-review | [04-test-gen, 05-implement] | Same prerequisites as unit-test — can run in parallel |
| 08-integration-test | [04-test-gen, 05-implement] | Same prerequisites — parallel leaf node |
| 09-acceptance | [01-proposal, 02-dev-design, 05-implement] | Pure dev track, no test track dependency |

For the `bug-fix` workflow_type:

| Phase | Prerequisites |
|-------|--------------|
| 01-proposal | [] |
| 02-dev-design | [01-proposal] |
| 05-implement | [02-dev-design] |
| 06-unit-test | [05-implement] |
| 07-code-review | [05-implement] |
| 09-acceptance | [07-code-review] |

For the `refactor` workflow_type, the prerequisite table SHALL match `requirement`.

#### Scenario: getPrerequisites returns correct dependencies for requirement workflow
- **WHEN** `getPrerequisites("01-proposal", "requirement")` is called
- **THEN** it returns `[]`

- **WHEN** `getPrerequisites("02-dev-design", "requirement")` is called
- **THEN** it returns `["01-proposal"]`

- **WHEN** `getPrerequisites("03-test-design", "requirement")` is called
- **THEN** it returns `["01-proposal", "02-dev-design"]`

- **WHEN** `getPrerequisites("05-implement", "requirement")` is called
- **THEN** it returns `["02-dev-design"]`

- **WHEN** `getPrerequisites("04-test-gen", "requirement")` is called
- **THEN** it returns `["03-test-design", "05-implement"]`

- **WHEN** `getPrerequisites("06-unit-test", "requirement")` is called
- **THEN** it returns `["04-test-gen", "05-implement"]`

- **WHEN** `getPrerequisites("07-code-review", "requirement")` is called
- **THEN** it returns `["04-test-gen", "05-implement"]`

- **WHEN** `getPrerequisites("08-integration-test", "requirement")` is called
- **THEN** it returns `["04-test-gen", "05-implement"]`

- **WHEN** `getPrerequisites("09-acceptance", "requirement")` is called
- **THEN** it returns `["01-proposal", "02-dev-design", "05-implement"]`

#### Scenario: getPrerequisites returns correct dependencies for bug-fix workflow
- **WHEN** `getPrerequisites("05-implement", "bug-fix")` is called
- **THEN** it returns `["02-dev-design"]`

- **WHEN** `getPrerequisites("09-acceptance", "bug-fix")` is called
- **THEN** it returns `["07-code-review"]`

#### Scenario: getPrerequisites is fault-tolerant
- **WHEN** `getPrerequisites("99-unknown")` is called
- **THEN** it returns `[]`

### Requirement: Dependents graph (reverse dependency lookup)
The workflow layer SHALL define a `getDependents(phaseId: string, workflowType?: string): string[]` function that returns all phases that list the given phase as a prerequisite. This is the reverse of `getPrerequisites()` and drives stale propagation.

For the `requirement` workflow_type, the dependents graph SHALL be:

| Phase | Dependents |
|-------|-----------|
| 01-proposal | [02-dev-design, 03-test-design, 09-acceptance] |
| 02-dev-design | [03-test-design, 05-implement, 09-acceptance] |
| 03-test-design | [04-test-gen] |
| 05-implement | [04-test-gen, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance] |
| 04-test-gen | [06-unit-test, 07-code-review, 08-integration-test] |
| 06-unit-test | [] |
| 07-code-review | [] |
| 08-integration-test | [] |
| 09-acceptance | [] |

The function SHALL:
1. Accept a `workflowType` parameter (defaults to `"requirement"`)
2. Return the dependents array for the given phase in that workflow
3. Return an empty array for unknown or invalid phase IDs
4. Be derived from `getPrerequisites()` (single source of truth)

#### Scenario: getDependents returns correct dependents
- **WHEN** `getDependents("01-proposal")` is called
- **THEN** it returns `["02-dev-design", "03-test-design", "09-acceptance"]`

- **WHEN** `getDependents("02-dev-design")` is called
- **THEN** it returns `["03-test-design", "05-implement", "09-acceptance"]`

- **WHEN** `getDependents("05-implement")` is called
- **THEN** it returns `["04-test-gen", "06-unit-test", "07-code-review", "08-integration-test", "09-acceptance"]`

- **WHEN** `getDependents("04-test-gen")` is called
- **THEN** it returns `["06-unit-test", "07-code-review", "08-integration-test"]`

- **WHEN** `getDependents("06-unit-test")` is called
- **THEN** it returns `[]`

- **WHEN** `getDependents("09-acceptance")` is called
- **THEN** it returns `[]`

#### Scenario: getDependents is fault-tolerant
- **WHEN** `getDependents("99-unknown")` is called
- **THEN** it returns `[]`

### Requirement: markPhaseStale propagates immediately on call
The `markPhaseStale(entries, phaseId)` function in `eval-json.ts` SHALL:

1. Find the latest pass entry for `phaseId` and mark it `stale: true`
2. Immediately call `propagateStale(entries, phaseId)` to mark all downstream dependents stale
3. If no pass entry exists, do nothing (no-op)

`propagateStale` SHALL recursively follow dependents via `getDependents()` and mark ALL entries for each affected phase as `stale: true`.

This ensures downstream invalidation happens atomically at the moment of marking, not deferred to when the phase is redone.

#### Scenario: markPhaseStale propagates along dev track
- **GIVEN** eval.json has pass entries for all phases 01-09
- **WHEN** `markPhaseStale(entries, "02-dev-design")` is called
- **THEN** the latest `02-dev-design` pass entry is marked `stale: true`
- **AND** entries for `03-test-design`, `05-implement`, `09-acceptance` are marked stale (direct dependents)
- **AND** entries for `04-test-gen`, `06-unit-test`, `07-code-review`, `08-integration-test` are marked stale (transitive)

#### Scenario: markPhaseStale propagates along test track only
- **GIVEN** eval.json has pass entries for all phases 01-09
- **WHEN** `markPhaseStale(entries, "03-test-design")` is called
- **THEN** entries for `04-test-gen`, `06-unit-test`, `07-code-review`, `08-integration-test` are marked stale
- **AND** entries for `01-proposal`, `02-dev-design`, `05-implement`, `09-acceptance` remain non-stale

#### Scenario: markPhaseStale full transitive closure
- **GIVEN** eval.json has pass entries for all phases 01-09
- **WHEN** `markPhaseStale(entries, "01-proposal")` is called
- **THEN** all entries for phases 01-09 are marked stale (root phase change affects everything)

#### Scenario: markPhaseStale when no downstream entries exist
- **GIVEN** eval.json only has entries for phases 01-02
- **WHEN** `markPhaseStale(entries, "02-dev-design")` is called
- **THEN** `propagateStale` completes without error (no downstream entries to mark)
- **AND** eval.json is not corrupted

#### Scenario: markPhaseStale does not affect same-phase new entry
- **GIVEN** eval.json has a previous pass entry for `02-dev-design` (attempt 1)
- **WHEN** a NEW pass entry for `02-dev-design` (attempt 2) is written AFTER markPhaseStale was called
- **THEN** the new entry is NOT stale (only old entries were marked)
- **AND** `hasPhasePassed` finds the new non-stale entry and returns true

### Requirement: Backtrack triggers markPhaseStale (with immediate propagation)
When `phase_log` writes an entry that contains `backtrack_to`, it SHALL call `markPhaseStale()` for each target phase BEFORE writing the new entry. `markPhaseStale` handles both marking the target and propagating downstream.

This replaces the previous behavior of deleting entries via `clearEntriesFromPhase()`.

#### Scenario: single backtrack_to marks target + propagates
- **GIVEN** eval.json has pass entries for 01-proposal through 05-implement
- **WHEN** `phase_log` writes a fail entry for `05-implement` with `backtrack_to: "02-dev-design"`
- **THEN** `markPhaseStale("02-dev-design")` is called
- **AND** 02 latest pass is marked stale, and downstream 03,04,05,06,07,08,09 are all stale (propagated)
- **AND** only 01 remains non-stale

#### Scenario: array backtrack_to calls markPhaseStale for each target
- **GIVEN** eval.json has pass entries for 01-09
- **WHEN** `phase_log` writes a fail entry with `backtrack_to: ["02-dev-design", "03-test-design"]`
- **THEN** `markPhaseStale` is called for "02-dev-design" (propagates to 03,05,09 and beyond)
- **AND** `markPhaseStale` is called for "03-test-design" (propagates to 04,06,07,08)

#### Scenario: backtrack_to target has no pass entry
- **GIVEN** eval.json has no entry for 02-dev-design
- **WHEN** `phase_log` writes a fail entry with `backtrack_to: "02-dev-design"`
- **THEN** `markPhaseStale` completes without error (nothing to mark)
- **AND** the fail entry is written normally

### Requirement: hasPhasePassed filters stale entries
The `hasPhasePassed()` function in `phase-next.ts` SHALL ignore entries where `stale === true`. A phase is considered "passed" only if it has at least one entry with `verdict === 'pass'` (or `skipped === true`) AND `stale` is NOT `true`.

Entries without an `stale` field SHALL be treated as `stale: false` (backward compatibility).

#### Scenario: hasPhasePassed returns false for stale entries
- **GIVEN** eval.json has `[{phase: "02-dev-design", verdict: "pass", stale: true}]`
- **WHEN** `hasPhasePassed(entries, "02-dev-design")` is called
- **THEN** it returns `false`

#### Scenario: hasPhasePassed returns true for non-stale pass
- **GIVEN** eval.json has `[{phase: "02-dev-design", verdict: "pass", stale: false}]` and `[{phase: "02-dev-design", verdict: "pass", stale: true}]`
- **WHEN** `hasPhasePassed(entries, "02-dev-design")` is called
- **THEN** it returns `true` (finds the non-stale entry)

#### Scenario: hasPhasePassed backward compatible with no stale field
- **GIVEN** eval.json has `[{phase: "02-dev-design", verdict: "pass"}]` (no stale field)
- **WHEN** `hasPhasePassed(entries, "02-dev-design")` is called
- **THEN** it returns `true` (missing stale treated as false)

### Requirement: phase_next MCP tool
The system SHALL provide `mcp__plugin_dev-team_dev-team__phase_next` MCP tool that returns the next phase to execute, its agent assignments, and prompt strings.

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
2. If `backtrack_to` is set in the latest entry (string or array): return the earliest target phase as `next_phase`. Entry stale marking and propagation were already handled by `markPhaseStale()` when the backtrack entry was written; no additional entry modification is needed here.
3. If last phase verdict is `fail` AND attempts < 5: return the same phase for retry
4. If last phase verdict is `fail` AND attempts >= 5: return `error: "max_retries_exceeded"`
5. If last phase verdict is `pass`: return the next sequential phase per `workflow_type` phase table (skipping phases that already have valid pass entries)
6. If all phases have valid (non-stale) pass entries: return `done: true`
7. If `round` > 20: return `error: "round_limit_exceeded"`
8. EVAL-ONLY phases return `planner: null`

**`workflow_type` phase tables (server-side):**

| workflow_type | Phases |
|---------------|--------|
| `requirement` | 01-proposal, 02-dev-design, 03-test-design, 05-implement, 04-test-gen, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance |
| `bug-fix` | 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance |
| `refactor` | 01-proposal, 02-dev-design, 03-test-design, 05-implement, 04-test-gen, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance |

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

#### Scenario: phase_next respects workflow_type
- **WHEN** `phase_next` is called with `workflow_type: "bug-fix"`
- **THEN** the phase sequence is: 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance
- **AND** phases 03-test-design, 04-test-gen, 08-integration-test are omitted

#### Scenario: phase_next handles string backtrack_to for backward compatibility
- **WHEN** `phase_next` detects `backtrack_to: "02-dev-design"` (string, not array) in the latest eval entry
- **THEN** it processes it identically to `["02-dev-design"]` (single-element array)
- **AND** returns the target phase as `next_phase`

### Requirement: Workflow skill thin loop
The workflow skill SHALL be a thin orchestration loop with no hardcoded phase knowledge. All phase sequencing, agent assignment, and prompt generation SHALL be owned by `phase_next` on the MCP server.

**Workflow skill logic:**
```
1. Assemble context (change name, explore context if available)
2. If change does not exist -> scaffold (openspec_new_change)
3. Loop:
   a. result = MCP phase_next(change, workflow_type)
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
- Handle backtrack logic (phase_next handles it server-side)
- Handle stale marking (phase_log handles it server-side)

#### Scenario: Workflow skill is a thin loop
- **WHEN** reading `skills/workflow-requirement/SKILL.md`
- **THEN** it contains no hardcoded phase table
- **AND** it contains no agent name references other than in the phase_next response handling
- **AND** it delegates all sequencing decisions to `phase_next`

#### Scenario: Workflow skill handles phase_next error
- **WHEN** `phase_next` returns `error: "round_limit_exceeded"` or `error: "max_retries_exceeded"`
- **THEN** the workflow skill SHALL stop and display the error message
- **AND** PushNotification SHALL be sent

#### Scenario: Workflow skill resumes incomplete change
- **WHEN** user invokes `/dev-team:workflow-requirement <existing-change>` for a change with partial eval.json entries
- **THEN** the workflow detects the change already exists and does NOT re-scaffold
- **AND** `phase_next` returns the first phase without valid pass entry
- **AND** the workflow resumes from that phase

#### Scenario: Workflow skill resumes after interruption
- **WHEN** user re-invokes `/dev-team:workflow-requirement <change>` after a previous run was stopped (e.g., max retries, user interrupt)
- **THEN** the workflow picks up from where it left off
- **AND** already-passed phases are skipped via `phase_next` server-side logic
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
- **AND** passes the extracted context to proposal-planner via the prompt from phase_next

#### Scenario: No explore context — ask user
- **WHEN** user invokes workflow-requirement without argument and no explore context is detected
- **THEN** the workflow prompts the user: "想构建什么变更？"
- **AND** derives kebab-case from the user's response

### Requirement: Workflow completion notification
After `phase_next` returns `done: true`, the workflow-orchestration system SHALL stop and notify the user that all phases have passed. Archive is a manual step performed by the user.

The completion step SHALL:
1. Display a completion summary listing all phases and their verdicts
2. Send a PushNotification: "所有 phase 已完成，请检查后手动执行 /dev-team:openspec-archive-change"
3. NOT automatically run `openspec archive` or move the change directory

The user SHALL manually inspect the results and run `/dev-team:openspec-archive-change` when satisfied.

#### Scenario: Workflow stops on completion, user manually archives
- **WHEN** phase_next returns `done: true`
- **THEN** the workflow displays a completion summary and stops
- **AND** sends PushNotification reminding user to manually archive
- **AND** does NOT move the change to openspec/changes/archive/

### Requirement: Extensibility for workflow variants
The workflow-orchestration system SHALL support multiple `workflow_type` values via the `phase_next` interface. New workflow variants SHALL be created by:
1. Defining a new phase table in the MCP server for the `workflow_type`
2. Creating a thin skill file that calls `phase_next` with the appropriate `workflow_type`

No skill-level changes to pipeline logic are required for new variants.

Reserved workflow variants:
- `workflow-bug-fix`: Simplified pipeline (skip test-design, test-gen, integration-test)
- `workflow-refactor`: Full pipeline with emphasis on design review

#### Scenario: New workflow variant requires only skill file + server phase table
- **WHEN** a future `workflow-bug-fix` skill is created
- **THEN** the skill file is a copy of workflow-requirement with `workflow_type: "bug-fix"`
- **AND** the MCP server has the bug-fix phase table defined
- **AND** no other changes are needed

## Module Contract

### workflow.ts (lib/)

| Export | Type | Purpose |
|--------|------|---------|
| `getPrerequisites(phaseId, workflowType?)` | `string[]` | Return explicit prerequisite phase list for a given phase |
| `getDependents(phaseId, workflowType?)` | `string[]` | Return phases that list the given phase as a prerequisite (derived from prerequisites) |
| `PHASE_PREREQUISITES` | `Record<string, string[]>` | Prerequisite table keyed by phase ID; `04-test-gen` adds `05-implement` |
| `PHASE_REQUIREMENT` | `string[]` | MODIFIED: `05-implement` entry before `04-test-gen` |
| `getPriorPhases(phaseId)` | `string[]` | Retained for backward compatibility — returns all prior phases |

### eval-json.ts (lib/)

| Export | Type | Purpose |
|--------|------|---------|
| `propagateStale(entries, phaseId, workflowType?)` | `void` | Recursively mark all downstream dependent phase entries as stale (called by `markPhaseStale`) |
| `markPhaseStale(entries, phaseId)` | `void` | Mark the latest pass entry stale AND immediately call `propagateStale` for downstream propagation |
| `checkGate(entries, prerequisites)` | `GateResult` | Check each prerequisite has a non-stale pass entry |

### MCP Tools

| Tool | Change | Purpose |
|------|--------|---------|
| phase_next | MODIFIED (behavior) | Phase table order and prerequisite checks for implement-before-test-gen |
| phase_log | UNCHANGED | Stale propagation uses updated dependents graph |
| phase_check | DEPRECATED | Removed from workflow loop; logic merged into `phase_next`. Tool retained for debugging. |

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

