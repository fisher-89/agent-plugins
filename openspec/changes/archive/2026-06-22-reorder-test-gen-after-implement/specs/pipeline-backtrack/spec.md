## MODIFIED Requirements

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

## ADDED Requirements

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

## Module Contract

### eval-json.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `propagateStale()` | MODIFIED (behavior) | Uses updated `getDependents()` where 05→04 is direct edge |
| `markPhaseStale()` | MODIFIED (behavior) | Backtrack to 05-implement immediately stale-marks 04-test-gen |

### workflow.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `getDependents("05-implement")` | MODIFIED | Now includes `04-test-gen` as direct dependent |
