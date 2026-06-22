## ADDED Requirements

### Requirement: Stale field on eval entries
Each entry in eval.json SHALL support an optional `stale` field of type `boolean`. When `stale` is `true`, the entry SHALL be ignored by `hasPhasePassed()`, `checkGate()`, and any other logic that checks for "completed" phases.

Entries without an `stale` field SHALL be treated as `stale: false` (backward compatibility with existing eval.json files).

The `stale` field SHALL NEVER be set to `true` on the entry being written — it only marks OTHER entries as stale (the target of backtrack, or downstream dependents after a phase is redone).

#### Scenario: new entries default to not stale
- **WHEN** `phase/log` writes a new pass entry
- **THEN** the entry has `stale: false` (or the field is absent)
- **AND** the entry is counted as a valid pass

#### Scenario: existing entries without stale field are treated as valid
- **GIVEN** eval.json has `[{phase:"02-dev-design", verdict:"pass"}]` (no stale field)
- **WHEN** `hasPhasePassed(entries, "02-dev-design")` is called
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
- **GIVEN** eval.json has `[{phase:"02-dev-design", verdict:"pass", attempt:1}, {phase:"02-dev-design", verdict:"pass", attempt:2}]` and downstream entries for 03,05
- **WHEN** `markPhaseStale(entries, "02-dev-design")` is called
- **THEN** the attempt 2 entry is marked `stale: true`
- **AND** `propagateStale` is called for "02-dev-design"
- **AND** downstream entries (03,05 and beyond) are marked stale

#### Scenario: markPhaseStale with no pass entry is no-op
- **GIVEN** eval.json has `[{phase:"02-dev-design", verdict:"fail"}]` (no pass entries)
- **WHEN** `markPhaseStale(entries, "02-dev-design")` is called
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
- **WHEN** `propagateStale(entries, "02-dev-design")` is called
- **THEN** `03-test-design`, `05-implement`, `09-acceptance` entries are marked stale (direct dependents)
- **AND** `04-test-gen`, `06-unit-test`, `07-code-review`, `08-integration-test` entries are marked stale (transitive)

#### Scenario: propagateStale from test-design
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "03-test-design")` is called
- **THEN** `04-test-gen`, `06-unit-test`, `07-code-review`, `08-integration-test` entries are marked stale
- **AND** `01-proposal`, `02-dev-design`, `05-implement`, `09-acceptance` entries are NOT stale

#### Scenario: propagateStale from implement
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "05-implement")` is called
- **THEN** `04-test-gen`, `06-unit-test`, `07-code-review`, `08-integration-test`, `09-acceptance` entries are marked stale (direct dependents of 05)
- **AND** `01-proposal`, `02-dev-design`, `03-test-design` entries are NOT stale

#### Scenario: propagateStale from leaf phase is no-op
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "06-unit-test")` is called
- **THEN** no entries are marked stale (06 has no dependents)

#### Scenario: propagateStale with no downstream entries
- **GIVEN** eval.json only has entries for phases 01-03
- **WHEN** `propagateStale(entries, "02-dev-design")` is called
- **THEN** the function completes without error
- **AND** existing entries are NOT modified (no matching dependents)

#### Scenario: propagateStale from 09-acceptance is no-op
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "09-acceptance")` is called
- **THEN** no entries are marked stale (09 has no dependents)

#### Scenario: propagateStale preserves stale:false entries of skipped dependents
- **GIVEN** eval.json has `04-test-gen` entry with NO entries (phase not yet executed)
- **WHEN** `propagateStale(entries, "03-test-design")` is called
- **THEN** the function skips 04-test-gen (no entries to mark) and continues to 06-unit-test
- **AND** 06-unit-test entries are marked stale

### Requirement: Implement backtrack invalidates test-gen directly
Because `04-test-gen` lists `05-implement` as a prerequisite, any stale propagation from `05-implement` SHALL mark `04-test-gen` as a **direct** dependent (not only via transitive chain through other phases).

#### Scenario: markPhaseStale on implement propagates to test-gen
- **GIVEN** eval.json has valid pass entries for phases 01-05 and 04-test-gen
- **WHEN** `markPhaseStale(entries, "05-implement")` is called
- **THEN** the latest pass entry for `05-implement` is marked stale
- **AND** all entries for `04-test-gen` are marked stale (direct dependent)
- **AND** entries for `06-unit-test`, `07-code-review`, `08-integration-test`, `09-acceptance` are marked stale (transitive)

#### Scenario: test-gen pass without implement pass is invalid after reorder
- **GIVEN** eval.json has a non-stale pass for `04-test-gen` but no valid pass for `05-implement`
- **WHEN** `phase/next` evaluates whether `04-test-gen` has passed
- **THEN** `04-test-gen` SHALL NOT be considered complete for scheduling `06-unit-test`
- **AND** `phase/next` SHALL return `05-implement` if its prerequisites are met, or block until prerequisites are satisfied

### Requirement: phase/log triggers markPhaseStale on backtrack_to (with immediate propagation)
When `runPhaseLog()` writes an entry that has a non-null `backtrack_to` field, it SHALL call `markPhaseStale()` for each target phase BEFORE writing the new entry. `markPhaseStale` handles both marking the target stale and propagating downstream.

- If `backtrack_to` is a string: `markPhaseStale(entries, backtrack_to)`
- If `backtrack_to` is an array: `markPhaseStale(entries, target)` for each target in the array

#### Scenario: phase/log marks backtrack target + propagates
- **GIVEN** eval.json has pass entries for 01-05
- **WHEN** `runPhaseLog` is called with `phase: "05-implement", verdict: "fail", backtrack_to: "02-dev-design"`
- **THEN** `markPhaseStale(entries, "02-dev-design")` is called
- **AND** 02 is marked stale, and downstream 03,04,05,06,07,08,09 are all stale (immediate propagation)
- **AND** the new fail entry for 05-implement is written

#### Scenario: phase/log handles array backtrack targets with propagation
- **GIVEN** eval.json has pass entries for 01-09
- **WHEN** `runPhaseLog` is called with `backtrack_to: ["02-dev-design", "03-test-design"]`
- **THEN** `markPhaseStale` is called for each target
- **AND** each call independently propagates downstream

### Requirement: phase/log pass entries do NOT trigger propagation
When `runPhaseLog()` writes a pass entry (verdict === 'pass' or skipped === true), it SHALL NOT call `propagateStale`. Propagation was already done when `markPhaseStale` was called at backtrack time.

#### Scenario: phase/log on pass writes entry only
- **GIVEN** eval.json has entries for all phases, some stale
- **WHEN** `runPhaseLog` writes a new pass entry for 02-dev-design
- **THEN** the entry is written with `stale: false`
- **AND** `propagateStale` is NOT called
- **AND** no other entries are modified

#### Scenario: phase/log on first-time pass
- **GIVEN** eval.json only has entries for 01-proposal
- **WHEN** `runPhaseLog` writes a pass entry for 02-dev-design (first time)
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
- **WHEN** `phase/next` is called and the latest entry has `backtrack_to: "02-dev-design"`
- **THEN** it returns `next_phase: "02-dev-design"`
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
- **GIVEN** eval.json has pass entries for 02-dev-design and 03-test-design
- **WHEN** evaluator writes entry with `backtrack_to: ["02-dev-design", "03-test-design"]`
- **THEN** both 02 and 03 are marked stale by phase/log
- **AND** phase/next returns `next_phase: "02-dev-design"` (earliest in phase table)

#### Scenario: String backtrack_to is backward compatible
- **WHEN** `backtrack_to` is `"02-dev-design"` (string)
- **THEN** it behaves identically to `["02-dev-design"]`

#### Scenario: Invalid target in array is rejected
- **WHEN** phase/log receives `backtrack_to: ["02-dev-design", "99-invalid"]`
- **THEN** it SHALL reject with `error: "invalid_backtrack_target"` before modifying any entries
- **AND** no entries are marked stale

## MODIFIED Requirements

### Requirement: Evaluator can mark backtrack target
The system SHALL allow Evaluators to set a `backtrack_to` field in their eval JSON, pointing to one or more prior phase identifiers.

The `backtrack_to` field SHALL accept both a single string and an array of strings. When set as an array, `phase/log` SHALL mark ALL target phases as stale and `phase/next` SHALL return the earliest target.

The system SHALL permit backtrack from the following evaluators to the following targets:
- acceptance (09-acceptance, acceptance-evaluator) -> requirements (01-requirements)
- code-review (07-code-review, code-review-evaluator) -> dev-design (02-dev-design)
- unit-test (06-unit-test, unit-test-evaluator) -> test-gen (04-test-gen), implement (05-implement), test-design (03-test-design), dev-design (02-dev-design)
- integration-test (08-integration-test, integration-test-evaluator) -> test-gen (04-test-gen), implement (05-implement), test-design (03-test-design), dev-design (02-dev-design)
The test-execution evaluators SHALL NOT backtrack to requirements (01-requirements) or acceptance (09-acceptance).

All references to `03-dev-proposal` are UPDATED to `02-dev-design`. All references to `02-test-design` are UPDATED to `03-test-design`.

#### Scenario: Acceptance evaluator finds unmet requirement
- **WHEN** acceptance-evaluator finds acceptance_criteria from proposal.md without corresponding test or implementation evidence
- **THEN** eval report has verdict "fail" with backtrack_to set to "01-requirements"

#### Scenario: Code review evaluator finds design deviation
- **WHEN** code-review-evaluator finds implementation that contradicts design.md
- **THEN** eval report has verdict "fail" with backtrack_to set to "02-dev-design"

#### Scenario: Unit-test evaluator finds syntax error in test files
- **WHEN** unit-test-evaluator (in 06-unit-test phase) detects a syntax/import error in test files from the executor's report
- **THEN** eval report has verdict "fail" with backtrack_to set to "04-test-gen"

#### Scenario: Unit-test evaluator finds logic error in implementation
- **WHEN** unit-test-evaluator (in 06-unit-test phase) finds assertion failures pointing to implementation code
- **THEN** eval report has verdict "fail" with backtrack_to set to "05-implement"

#### Scenario: Integration-test evaluator finds contract mismatch
- **WHEN** integration-test-evaluator (in 08-integration-test phase) finds interface signature mismatch between test and implementation
- **THEN** eval report has verdict "fail" with backtrack_to set to "02-dev-design"

#### Scenario: Integration-test evaluator backtracks to unit-test
- **WHEN** integration-test-evaluator finds that unit-test phase report was incomplete (missing coverage data or key test scenarios)
- **THEN** eval report has verdict "fail" with backtrack_to set to "06-unit-test"

#### Scenario: Integration-test evaluator backtracks to code-review
- **WHEN** integration-test-evaluator identifies a structural issue that should have been caught by code-review
- **THEN** eval report has verdict "fail" with backtrack_to set to "07-code-review"

#### Scenario: Evaluator can backtrack to both tracks (array format)
- **WHEN** integration-test-evaluator finds both test syntax errors AND implementation logic errors
- **THEN** eval report has verdict "fail" with backtrack_to set to `["04-test-gen", "05-implement"]`
- **AND** phase/log marks both 04-test-gen and 05-implement latest pass entries as stale

### Requirement: Backtrack target validation for test-execution evaluators
When a test-execution evaluator (unit-test-evaluator or integration-test-evaluator) sets a `backtrack_to` field, the system SHALL validate that each target phase is within the permitted set.

Permitted targets for unit-test-evaluator: test-design (03-test-design), dev-design (02-dev-design), test-gen (04-test-gen), implement (05-implement).
Permitted targets for integration-test-evaluator: test-design (03-test-design), dev-design (02-dev-design), test-gen (04-test-gen), implement (05-implement).

If the test-execution evaluator attempts to backtrack to an invalid target (e.g., requirements or acceptance), the skill SHALL override the backtrack_to to dev-design (02-dev-design) (safe default) and log a warning in eval.json.

If ANY element in the array is outside the permitted set, the entire backtrack SHALL be overridden to dev-design (02-dev-design) and a warning logged.

#### Scenario: Unit-test evaluator tries invalid backtrack
- **WHEN** unit-test-evaluator sets backtrack_to to "01-requirements"
- **THEN** the skill detects this is outside the permitted set for unit-test-evaluator
- **AND** overrides backtrack_to to "02-dev-design"
- **AND** logs "单元测试 Evaluator 试图回溯到 requirements 阶段，已自动修正为 dev-design（安全默认）" in the eval entry

#### Scenario: Integration-test evaluator tries invalid backtrack
- **WHEN** integration-test-evaluator sets backtrack_to to "09-acceptance"
- **THEN** the skill overrides backtrack_to to "02-dev-design"
- **AND** logs diagnostic warning in the eval entry

#### Scenario: Unit-test evaluator tries invalid backtrack (array with one invalid)
- **WHEN** unit-test-evaluator sets backtrack_to to `["04-test-gen", "09-acceptance"]` (09 is outside permitted set)
- **THEN** the skill overrides the ENTIRE backtrack_to to "02-dev-design"
- **AND** logs diagnostic warning in the eval entry

#### Scenario: Unit-test evaluator tries valid array backtrack
- **WHEN** unit-test-evaluator sets backtrack_to to `["04-test-gen", "05-implement"]` (both within permitted set)
- **THEN** the skill accepts the array as-is
- **AND** does NOT override

### Requirement: Backtrack chain integrity
When the diagnostic decision tree produces a backtrack_to target, the system SHALL verify that the target phase precedes the current phase in the workflow sequence: dev-design < test-design < test-gen < implement < unit-test < code-review < integration-test < acceptance.

If the backtrack target is a phase that has no entries in eval.json, the skill SHALL override backtrack_to to dev-design (02-dev-design) as a safe default.

If backtrack would create a cycle (e.g., unit-test -> implement -> unit-test), the system SHALL detect the cycle and require manual resolution by outputting the cycle path. For diagnostic test-execution evaluators, a cycle is defined as: backtrack_to targeting a phase that already has a non-null backtrack_to pointing back to the current evaluator's phase.

For array backtrack_to, cycle detection SHALL check each target independently. A cycle with ANY target SHALL trigger the cycle resolution workflow.

#### Scenario: Backtrack target phase has no entries
- **WHEN** integration-test-evaluator sets backtrack_to to "04-test-gen" but test-gen phase has no entries in eval.json
- **THEN** the skill overrides backtrack_to to "02-dev-design"
- **AND** logs "回溯目标 test-gen 阶段未执行，自动修正为 dev-design（安全默认）"

#### Scenario: Cycle detected between unit-test and implement
- **WHEN** unit-test-evaluator sets backtrack_to to "05-implement", and implement phase's latest entry has backtrack_to set to "06-unit-test"
- **THEN** the eval report includes a warning and the skill pauses for user intervention with the cycle path

## Module Contract

### eval-json.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `markPhaseStale(entries, phaseId)` | MODIFIED (behavior) | Backtrack to 05-implement immediately stale-marks 04-test-gen |
| `propagateStale(entries, phaseId, workflowType?)` | MODIFIED (behavior) | Uses updated `getDependents()` where 05→04 is direct edge |

### workflow.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `getDependents("05-implement")` | MODIFIED | Now includes `04-test-gen` as direct dependent |

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
