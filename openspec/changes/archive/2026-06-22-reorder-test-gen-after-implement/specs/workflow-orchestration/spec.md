## MODIFIED Requirements

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
| 06-unit-test | [04-test-gen, 05-implement] | Needs both test-gen and implement |
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

## Module Contract

### workflow.ts (`plugins/dev-team/bin/src/lib/`)

| Function / Constant | Change | Purpose |
|---------------------|--------|---------|
| `PHASE_REQUIREMENT` | MODIFIED | `05-implement` entry before `04-test-gen` |
| `PHASE_PREREQUISITES` | MODIFIED | `04-test-gen` adds `05-implement` |
| `getPrerequisites()` | MODIFIED (derived) | Reflects new 04 prerequisites |
| `getDependents()` | MODIFIED (derived) | `05-implement` includes `04-test-gen` |

### MCP Tools

| Tool | Change | Purpose |
|------|--------|---------|
| phase_next | MODIFIED (behavior) | Phase table order and prerequisite checks for implement-before-test-gen |
| phase_log | UNCHANGED | Stale propagation uses updated dependents graph |
| phase_check | UNCHANGED | Deprecated; debugging only |
