## ADDED Requirements

### Requirement: Backtrack phase targets use prefix-free phase IDs
Backtrack targets in the `backtrack_to` field of eval.json entries SHALL use prefix-free phase IDs. The `phase-next.ts` module SHALL perform phase table lookups and comparisons using these new IDs.

The following backtrack flows are affected (all references updated):

| Scenario | Old Backtrack Target | New Backtrack Target |
|----------|---------------------|----------------------|
| acceptance finds unmet requirements | `01-proposal` | `proposal` |
| code-review finds design deviation | `02-dev-design` | `dev-design` |
| unit-test finds syntax errors | `04-test-gen` | `test-gen` |
| unit-test finds logic errors | `05-implement` | `implement` |
| unit-test finds design issues | `03-test-design` | `test-design` |
| integration-test finds contract mismatch | `02-dev-design` | `dev-design` |
| integration-test finds both issues | `["04-test-gen", "05-implement"]` | `["test-gen", "implement"]` |

#### Scenario: Backtrack to proposal uses new ID
- **WHEN** acceptance-evaluator sets `backtrack_to: "proposal"` in eval.json
- **THEN** `phase_next` detects the backtrack and returns `next_phase: "proposal"`

#### Scenario: Backtrack to dev-design uses new ID
- **WHEN** code-review-evaluator sets `backtrack_to: "dev-design"` in eval.json
- **THEN** `phase_next` detects the backtrack and returns `next_phase: "dev-design"`

#### Scenario: Array backtrack uses prefix-free IDs
- **WHEN** evaluator sets `backtrack_to: ["test-gen", "implement"]` in eval.json
- **THEN** `phase_next` returns `next_phase: "test-gen"` (earliest in phase table)

### Requirement: backtrack hint builder uses prefix-free phase IDs in evaluator prompts
The `buildBacktrackHint()` function SHALL use the phase table values as-is to construct the backtrack hint appended to evaluator prompts. Since the phase IDs in the table are now prefix-free, the backtrack hint SHALL display prefix-free IDs.

#### Scenario: Backtrack hint shows prefix-free IDs
- **WHEN** evaluator is returned for phase `implement` in requirement workflow
- **THEN** the backtrack hint lists available backtrack phases as `proposal`, `dev-design`, `test-design`, `test-gen` (no numeric prefixes)

#### Scenario: Backtrack hint shows test-only phases prefix-free
- **WHEN** evaluator is returned for phase `unit-test` in test-only workflow
- **THEN** the backtrack hint lists available backtrack phases as `proposal`, `code-analyze`, `test-design`, `test-gen` (no numeric prefixes)

### Requirement: Stale field on eval entries
Each entry in eval.json SHALL support an optional `stale` field of type `boolean`. When `stale` is `true`, the entry SHALL be ignored by `hasPhasePassed()`, `checkGate()`, and any other logic that checks for "completed" phases.

Entries without an `stale` field SHALL be treated as `stale: false` (backward compatibility with existing eval.json files).

The `stale` field SHALL NEVER be set to `true` on the entry being written — it only marks OTHER entries as stale (the target of backtrack, or downstream dependents after a phase is redone).

#### Scenario: new entries default to not stale
- **WHEN** `phase/log` writes a new pass entry
- **THEN** the entry has `stale: false` (or the field is absent)
- **AND** the entry is counted as a valid pass

#### Scenario: existing entries without stale field are treated as valid
- **GIVEN** eval.json has `[{phase:"dev-design", verdict:"pass"}]` (no stale field)
- **WHEN** `hasPhasePassed(entries, "dev-design")` is called
- **THEN** it returns `true`

### Requirement: markPhaseStale — mark latest pass + immediately propagate downstream
The `eval-json.ts` module SHALL export a `markPhaseStale(entries: any[], phaseId: string, workflowType?: string): void` function that:

1. Finds all entries for the given `phaseId`
2. Sorts by timestamp descending
3. Marks the first entry where `verdict === 'pass'` as `stale: true`
4. Immediately calls `propagateStale(entries, phaseId, workflowType)` to mark all downstream dependents stale
5. If no pass entry exists, does nothing (no-op, no propagation)

This function SHALL NOT delete entries.

#### Scenario: markPhaseStale marks latest pass entry + propagates
- **GIVEN** eval.json has `[{phase:"dev-design", verdict:"pass", attempt:1}, {phase:"dev-design", verdict:"pass", attempt:2}]` and downstream entries for test-design, implement
- **WHEN** `markPhaseStale(entries, "dev-design")` is called
- **THEN** the attempt 2 entry is marked `stale: true`
- **AND** `propagateStale` is called for "dev-design"
- **AND** downstream entries (test-design, implement and beyond) are marked stale

#### Scenario: markPhaseStale with no pass entry is no-op
- **GIVEN** eval.json has `[{phase:"dev-design", verdict:"fail"}]` (no pass entries)
- **WHEN** `markPhaseStale(entries, "dev-design")` is called
- **THEN** no entries are modified
- **AND** `propagateStale` is NOT called
- **AND** the function does not throw

### Requirement: propagateStale — transitive downstream stale marking
The `eval-json.ts` module SHALL export a `propagateStale(entries: any[], phaseId: string, workflowType?: string): void` function that:

1. Finds all direct dependents of `phaseId` via `getDependents(phaseId, workflowType)`
2. For each dependent, marks ALL entries (not just pass entries) for that phase as `stale: true`
3. Recursively propagates to the dependents' dependents (transitive closure)

The propagation SHALL use a visited-set to prevent infinite loops (defensive, as the dependency graph is a DAG).

The propagation SHALL NOT throw if some dependents have no entries in eval.json (just skip them).

#### Scenario: propagateStale from dev-design
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "dev-design")` is called
- **THEN** `test-design`, `implement`, `acceptance` entries are marked stale (direct dependents)
- **AND** `test-gen`, `unit-test`, `code-review`, `integration-test` entries are marked stale (transitive)

#### Scenario: propagateStale from test-design
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "test-design")` is called
- **THEN** `test-gen`, `unit-test`, `code-review`, `integration-test` entries are marked stale
- **AND** `proposal`, `dev-design`, `implement`, `acceptance` entries are NOT stale

#### Scenario: propagateStale from implement
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "implement")` is called
- **THEN** `test-gen`, `unit-test`, `code-review`, `integration-test`, `acceptance` entries are marked stale (direct dependents of implement)
- **AND** `proposal`, `dev-design`, `test-design` entries are NOT stale

#### Scenario: propagateStale from leaf phase is no-op
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "unit-test")` is called
- **THEN** no entries are marked stale (unit-test has no dependents)

#### Scenario: propagateStale with no downstream entries
- **GIVEN** eval.json only has entries for phases proposal through test-design
- **WHEN** `propagateStale(entries, "dev-design")` is called
- **THEN** the function completes without error
- **AND** existing entries are NOT modified (no matching dependents)

#### Scenario: propagateStale from acceptance is no-op
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "acceptance")` is called
- **THEN** no entries are marked stale (acceptance has no dependents)

#### Scenario: propagateStale preserves stale:false entries of skipped dependents
- **GIVEN** eval.json has `test-gen` entry with NO entries (phase not yet executed)
- **WHEN** `propagateStale(entries, "test-design")` is called
- **THEN** the function skips test-gen (no entries to mark) and continues to unit-test
- **AND** unit-test entries are marked stale

### Requirement: Implement backtrack invalidates test-gen directly
Because `test-gen` lists `implement` as a prerequisite, any stale propagation from `implement` SHALL mark `test-gen` as a **direct** dependent (not only via transitive chain through other phases).

#### Scenario: markPhaseStale on implement propagates to test-gen
- **GIVEN** eval.json has valid pass entries for phases proposal through implement and test-gen
- **WHEN** `markPhaseStale(entries, "implement")` is called
- **THEN** the latest pass entry for `implement` is marked stale
- **AND** all entries for `test-gen` are marked stale (direct dependent)
- **AND** entries for `unit-test`, `code-review`, `integration-test`, `acceptance` are marked stale (transitive)

#### Scenario: test-gen pass without implement pass is invalid after reorder
- **GIVEN** eval.json has a non-stale pass for `test-gen` but no valid pass for `implement`
- **WHEN** `phase/next` evaluates whether `test-gen` has passed
- **THEN** `test-gen` SHALL NOT be considered complete for scheduling `unit-test`
- **AND** `phase/next` SHALL return `implement` if its prerequisites are met, or block until prerequisites are satisfied

### Requirement: phase/log triggers markPhaseStale on backtrack_to (with immediate propagation)
When `runPhaseLog()` writes an entry that has a non-null `backtrack_to` field, it SHALL call `markPhaseStale()` for each target phase BEFORE writing the new entry. `markPhaseStale` handles both marking the target stale and propagating downstream.

- If `backtrack_to` is a string: `markPhaseStale(entries, backtrack_to)`
- If `backtrack_to` is an array: `markPhaseStale(entries, target)` for each target in the array

#### Scenario: phase/log marks backtrack target + propagates
- **GIVEN** eval.json has pass entries for proposal through implement
- **WHEN** `runPhaseLog` is called with `phase: "implement", verdict: "fail", backtrack_to: "dev-design"`
- **THEN** `markPhaseStale(entries, "dev-design")` is called
- **AND** dev-design is marked stale, and downstream test-design, test-gen, implement, unit-test, code-review, integration-test, acceptance are all stale (immediate propagation)
- **AND** the new fail entry for implement is written

#### Scenario: phase/log handles array backtrack targets with propagation
- **GIVEN** eval.json has pass entries for all phases
- **WHEN** `runPhaseLog` is called with `backtrack_to: ["dev-design", "test-design"]`
- **THEN** `markPhaseStale` is called for each target
- **AND** each call independently propagates downstream

### Requirement: phase/log pass entries do NOT trigger propagation
When `runPhaseLog()` writes a pass entry (verdict === 'pass' or skipped === true), it SHALL NOT call `propagateStale`. Propagation was already done when `markPhaseStale` was called at backtrack time.

#### Scenario: phase/log on pass writes entry only
- **GIVEN** eval.json has entries for all phases, some stale
- **WHEN** `runPhaseLog` writes a new pass entry for dev-design
- **THEN** the entry is written with `stale: false`
- **AND** `propagateStale` is NOT called
- **AND** no other entries are modified

#### Scenario: phase/log on first-time pass
- **GIVEN** eval.json only has entries for proposal
- **WHEN** `runPhaseLog` writes a pass entry for dev-design (first time)
- **THEN** the entry is written successfully
- **AND** no propagation occurs

### Requirement: phase/next no longer modifies eval.json
The `phase/next` MCP tool SHALL NOT delete or modify entries in eval.json. The `clearEntriesFromPhase()` function and associated entry-persistence logic SHALL be removed.

Instead, `phase/next` SHALL:
1. Read eval.json entries
2. Use `hasPhasePassed()` (which filters stale entries) to determine phase completion
3. Use `getLatestBacktrackTarget()` to detect backtrack direction
4. Return the next phase without modifying eval.json

Entry stale marking is handled entirely by `phase/log`:
- On fail with backtrack_to: `markPhaseStale()` (marks target + immediately propagates downstream)
- On pass: no extra action (propagation already happened at backtrack time)

This separation of concerns means `phase/next` is a pure read operation, while `phase/log` is the sole write+mutate operation.

#### Scenario: phase/next is read-only
- **WHEN** `phase/next` is called with any eval.json state
- **THEN** eval.json is NOT modified (no entries added, removed, or changed)
- **AND** the response only contains the next phase to execute

#### Scenario: phase/next handles backtrack without clearing entries
- **WHEN** `phase/next` is called and the latest entry has `backtrack_to: "dev-design"`
- **THEN** it returns `next_phase: "dev-design"`
- **AND** does NOT delete or mark stale any entries in eval.json (phase/log already handled that)

#### Scenario: clearEntriesFromPhase is removed
- **WHEN** reviewing `phase-next.ts` source code
- **THEN** `clearEntriesFromPhase()` function does NOT exist
- **AND** `writeEvalJson()` is NOT called from `resolvePhaseNext()` or `runPhaseNext()`
- **AND** `updatedEntries` is NOT present in `ResolvePhaseNextResult`

### Requirement: Array backtrack_to support
The eval.json schema SHALL support `backtrack_to` as either a string or an array of strings. When `backtrack_to` is an array, `phase/log` SHALL call `markPhaseStale()` for each element.

In `phase/next`, when `backtrack_to` is an array, the SHALL return the earliest target phase (by phase table index) as `next_phase`.

String format SHALL be fully backward compatible.

#### Scenario: Array backtrack_to marks all targets stale
- **GIVEN** eval.json has pass entries for dev-design and test-design
- **WHEN** evaluator writes entry with `backtrack_to: ["dev-design", "test-design"]`
- **THEN** both dev-design and test-design are marked stale by phase/log
- **AND** phase/next returns `next_phase: "dev-design"` (earliest in phase table)

#### Scenario: String backtrack_to is backward compatible
- **WHEN** `backtrack_to` is `"dev-design"` (string)
- **THEN** it behaves identically to `["dev-design"]`

#### Scenario: Invalid target in array is rejected
- **WHEN** phase/log receives `backtrack_to: ["dev-design", "99-invalid"]`
- **THEN** it SHALL reject with `error: "invalid_backtrack_target"` before modifying any entries
- **AND** no entries are marked stale

## MODIFIED Requirements

### Requirement: Evaluator can mark backtrack target
The system SHALL allow Evaluators to set a `backtrack_to` field in their eval JSON, pointing to one or more prior phase identifiers.

The `backtrack_to` field SHALL accept both a single string and an array of strings. When set as an array, `phase/log` SHALL mark ALL target phases as stale and `phase/next` SHALL return the earliest target.

The system SHALL permit backtrack from the following evaluators to the following targets:
- acceptance (acceptance-evaluator) -> proposal
- code-review (code-review-evaluator) -> dev-design
- unit-test (unit-test-evaluator) -> test-gen, implement, test-design, dev-design
- integration-test (integration-test-evaluator) -> test-gen, implement, test-design, dev-design
The test-execution evaluators SHALL NOT backtrack to proposal or acceptance.

All references to `03-dev-proposal` are UPDATED to `dev-design`. All references to `02-test-design` are UPDATED to `test-design`.

#### Scenario: Acceptance evaluator finds unmet requirement
- **WHEN** acceptance-evaluator finds acceptance_criteria from proposal.md without corresponding test or implementation evidence
- **THEN** eval report has verdict "fail" with backtrack_to set to "proposal"

#### Scenario: Code review evaluator finds design deviation
- **WHEN** code-review-evaluator finds implementation that contradicts design.md
- **THEN** eval report has verdict "fail" with backtrack_to set to "dev-design"

#### Scenario: Unit-test evaluator finds syntax error in test files
- **WHEN** unit-test-evaluator (in unit-test phase) detects a syntax/import error in test files from the executor's report
- **THEN** eval report has verdict "fail" with backtrack_to set to "test-gen"

#### Scenario: Unit-test evaluator finds logic error in implementation
- **WHEN** unit-test-evaluator (in unit-test phase) finds assertion failures pointing to implementation code
- **THEN** eval report has verdict "fail" with backtrack_to set to "implement"

#### Scenario: Integration-test evaluator finds contract mismatch
- **WHEN** integration-test-evaluator (in integration-test phase) finds interface signature mismatch between test and implementation
- **THEN** eval report has verdict "fail" with backtrack_to set to "dev-design"

#### Scenario: Integration-test evaluator backtracks to unit-test
- **WHEN** integration-test-evaluator finds that unit-test phase report was incomplete (missing coverage data or key test scenarios)
- **THEN** eval report has verdict "fail" with backtrack_to set to "unit-test"

#### Scenario: Integration-test evaluator backtracks to code-review
- **WHEN** integration-test-evaluator identifies a structural issue that should have been caught by code-review
- **THEN** eval report has verdict "fail" with backtrack_to set to "code-review"

#### Scenario: Evaluator can backtrack to both tracks (array format)
- **WHEN** integration-test-evaluator finds both test syntax errors AND implementation logic errors
- **THEN** eval report has verdict "fail" with backtrack_to set to `["test-gen", "implement"]`
- **AND** phase/log marks both test-gen and implement latest pass entries as stale

### Requirement: Backtrack target validation for test-execution evaluators
When a test-execution evaluator (unit-test-evaluator or integration-test-evaluator) sets a `backtrack_to` field, the system SHALL validate that each target phase is within the permitted set.

Permitted targets for unit-test-evaluator: test-design, dev-design, test-gen, implement.
Permitted targets for integration-test-evaluator: test-design, dev-design, test-gen, implement.

If the test-execution evaluator attempts to backtrack to an invalid target (e.g., requirements or acceptance), the skill SHALL override the backtrack_to to dev-design (safe default) and log a warning in eval.json.

If ANY element in the array is outside the permitted set, the entire backtrack SHALL be overridden to dev-design (dev-design) and a warning logged.

#### Scenario: Unit-test evaluator tries invalid backtrack
- **WHEN** unit-test-evaluator sets backtrack_to to "proposal"
- **THEN** the skill detects this is outside the permitted set for unit-test-evaluator
- **AND** overrides backtrack_to to "dev-design"
- **AND** logs "单元测试 Evaluator 试图回溯到 requirements 阶段，已自动修正为 dev-design（安全默认）" in the eval entry

#### Scenario: Integration-test evaluator tries invalid backtrack
- **WHEN** integration-test-evaluator sets backtrack_to to "acceptance"
- **THEN** the skill overrides backtrack_to to "dev-design"
- **AND** logs diagnostic warning in the eval entry

#### Scenario: Unit-test evaluator tries invalid backtrack (array with one invalid)
- **WHEN** unit-test-evaluator sets backtrack_to to `["test-gen", "acceptance"]` (09 is outside permitted set)
- **THEN** the skill overrides the ENTIRE backtrack_to to "dev-design"
- **AND** logs diagnostic warning in the eval entry

#### Scenario: Unit-test evaluator tries valid array backtrack
- **WHEN** unit-test-evaluator sets backtrack_to to `["test-gen", "implement"]` (both within permitted set)
- **THEN** the skill accepts the array as-is
- **AND** does NOT override

### Requirement: Backtrack chain integrity
When the diagnostic decision tree produces a backtrack_to target, the system SHALL verify that the target phase precedes the current phase in the workflow sequence: dev-design < test-design < test-gen < implement < unit-test < code-review < integration-test < acceptance.

If the backtrack target is a phase that has no entries in eval.json, the skill SHALL override backtrack_to to dev-design (dev-design) as a safe default.

If backtrack would create a cycle (e.g., unit-test -> implement -> unit-test), the system SHALL detect the cycle and require manual resolution by outputting the cycle path. For diagnostic test-execution evaluators, a cycle is defined as: backtrack_to targeting a phase that already has a non-null backtrack_to pointing back to the current evaluator's phase.

For array backtrack_to, cycle detection SHALL check each target independently. A cycle with ANY target SHALL trigger the cycle resolution workflow.

#### Scenario: Backtrack target phase has no entries
- **WHEN** integration-test-evaluator sets backtrack_to to "test-gen" but test-gen phase has no entries in eval.json
- **THEN** the skill overrides backtrack_to to "dev-design"
- **AND** logs "回溯目标 test-gen 阶段未执行，自动修正为 dev-design（安全默认）"

#### Scenario: Cycle detected between unit-test and implement
- **WHEN** unit-test-evaluator sets backtrack_to to "implement", and implement phase's latest entry has backtrack_to set to "unit-test"
- **THEN** the eval report includes a warning and the skill pauses for user intervention with the cycle path

## Module Contract

### eval-json.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `markPhaseStale(entries, phaseId)` | MODIFIED (behavior) | Backtrack to implement immediately stale-marks test-gen |
| `propagateStale(entries, phaseId, workflowType?)` | MODIFIED (behavior) | Uses updated `getDependents()` where 05→04 is direct edge |

### workflow.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `getDependents("implement")` | MODIFIED | Now includes `test-gen` as direct dependent |

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `clearEntriesFromPhase()` | REMOVED | No longer needed — entries are never deleted |
| `writeEvalJson()` | REMOVED from phase-next | phase/next no longer writes to eval.json |
| `resolvePhaseNext()` | MODIFIED | Removes entry deletion logic; `updatedEntries` no longer present in result |
| `runPhaseNext()` | MODIFIED | Removes `writeEvalJson` persistence step |
| `hasPhasePassed()` | MODIFIED | Filters entries where `stale === true` |

### phase-log.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `runPhaseLog()` | MODIFIED | After writing entry with `backtrack_to`: calls `markPhaseStale()` for each target (which internally propagates). Pass entries require no extra action. |

### Eval JSON Schema

| Field | Change | Type | Description |
|-------|--------|------|-------------|
| `stale` | NEW | `boolean` (optional, default false) | When true, the entry is ignored by gate-check and hasPhasePassed |
| `backtrack_to` | MODIFIED | `string \| string[] \| null` | Previously only string. Now accepts array for multi-track backtrack. |
