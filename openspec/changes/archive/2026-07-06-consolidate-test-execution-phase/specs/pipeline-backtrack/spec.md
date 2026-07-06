## MODIFIED Requirements

### Requirement: Backtrack phase targets use updated phase IDs

Backtrack targets in the `backtrack_to` field of eval.json entries SHALL use the updated phase IDs. No `integration-test` references remain.

The following backtrack flows are updated:

| Scenario | New Backtrack Target | Change |
|----------|---------------------|--------|
| acceptance finds unmet requirements | `proposal` | Unchanged |
| code-review finds design deviation | `dev-design` | Unchanged |
| test-execution finds syntax errors | `test-gen` | Was `unit-test` backtrack in old spec, now `test-execution` phase |
| test-execution finds logic errors | `implement` | Same target, different phase origin |
| test-execution finds design issues | `test-design` | Same target, different phase origin |

#### Scenario: Backtrack to proposal uses updated ID (unchanged)
- **WHEN** acceptance-evaluator sets `backtrack_to: "proposal"` in eval.json
- **THEN** `phase_next` detects the backtrack and returns `next_phase: "proposal"`

#### Scenario: Backtrack from test-execution uses updated phase name
- **WHEN** test-execution-evaluator sets `backtrack_to: "test-gen"` in eval.json
- **THEN** `phase_next` detects the backtrack and returns `next_phase: "test-gen"`

### Requirement: Stale propagation no longer propagates to integration-test

The `propagateStale` function SHALL NOT propagate stale marking to `integration-test` because that phase no longer exists in any workflow's phase table. Propagation follows `getDependents()` which is dynamically derived from updated prerequisite tables.

#### Scenario: propagateStale from implement no longer mentions integration-test
- **GIVEN** eval.json has entries for all 8 phases of the requirement workflow
- **WHEN** `propagateStale(entries, "implement")` is called
- **THEN** entries for `test-gen`, `test-execution`, `code-review`, `acceptance` are marked stale (direct or transitive)
- **AND** entries for `integration-test` are NOT marked (phase no longer exists)

#### Scenario: propagateStale from test-gen no longer mentions integration-test
- **GIVEN** eval.json has entries for all phases
- **WHEN** `propagateStale(entries, "test-gen")` is called
- **THEN** `test-execution` and `code-review` entries are marked stale
- **AND** `integration-test` entries are NOT marked (phase no longer exists)

### Requirement: Test-execution evaluator backtrack targets (consolidated)

When the test-execution evaluator (`test-execution-evaluator`) sets a `backtrack_to` field, the system SHALL validate that each target phase is within the permitted set.

Permitted targets for test-execution-evaluator: `test-gen`, `implement`, `test-design`, `dev-design`.

If the test-execution evaluator attempts to backtrack to an invalid target (e.g., `proposal` or `acceptance`), the skill SHALL override the `backtrack_to` to `dev-design` (safe default) and log a warning in eval.json.

If ANY element in the array is outside the permitted set, the entire backtrack SHALL be overridden to `dev-design` and a warning logged.

**Changes from previous version**:
- Merged the two separate permission sets (unit-test-evaluator + integration-test-evaluator) into one
- Integration-test as a backtrack target no longer exists
- The phase origin changed from `unit-test`/`integration-test` to `test-execution`

#### Scenario: Test-execution evaluator tries invalid backtrack to proposal
- **WHEN** test-execution-evaluator sets `backtrack_to` to `"proposal"`
- **THEN** the skill detects this is outside the permitted set
- **AND** overrides `backtrack_to` to `"dev-design"`
- **AND** logs "测试执行 Evaluator 试图回溯到 proposal 阶段，已自动修正为 dev-design（安全默认）"

#### Scenario: Test-execution evaluator valid array backtrack
- **WHEN** test-execution-evaluator sets `backtrack_to` to `["test-gen", "implement"]`
- **THEN** the skill accepts the array as-is
- **AND** does NOT override

## REMOVED Requirements

### Requirement: Unit-test evaluator backtrack to integration-test
**Reason**: The `integration-test` phase no longer exists. The test-execution evaluation handles all scenarios previously covered by separate unit-test and integration-test evaluators.

**Migration**: Both unit-test-evaluator and integration-test-evaluator backtrack logic merged into test-execution-evaluator. Permitted targets unchanged (`test-gen`, `implement`, `test-design`, `dev-design`).

### Requirement: Integration-test evaluator backtrack to unit-test
**Reason**: Integration-test phase removed. No separate evaluator exists for it.

**Migration**: All test execution backtrack flows now originate from `test-execution-evaluator`.

### Requirement: Integration-test evaluator backtrack to code-review
**Reason**: No longer needed as a distinct backtrack edge. The test-execution evaluator may still identify structural issues but the code-review phase has a separate evaluator path.

**Migration**: The consolidated test-execution-evaluator backtrack targets remain `test-gen`, `implement`, `test-design`, `dev-design`.

### Requirement: propagateStale from implement (integration-test dependents removed)
**Reason**: `getDependents("implement")` no longer includes `integration-test`. Dependents are now: `test-gen`, `test-execution`, `code-review`, `acceptance`.

**Migration**: Propagation logic updated via `getDependents()` which dynamically reads from prerequisites table.

## Module Contract

### eval-json.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `markPhaseStale(entries, phaseId)` | UNCHANGED | Behavior unchanged — propagation now uses updated dependents graph |
| `propagateStale(entries, phaseId, workflowType?)` | UNCHANGED | Uses `getDependents()` which no longer returns `integration-test` |

### workflow.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `getDependents("implement")` | MODIFIED | Returns `["test-gen", "test-execution", "code-review", "acceptance"]` (no `integration-test`) |
| `getDependents("test-gen")` | MODIFIED | Returns `["test-execution", "code-review"]` (no `integration-test`) |

### Eval JSON Schema

| Field | Change | Description |
|-------|--------|-------------|
| `backtrack_to` | UNCHANGED | `string | string[] | null` — no schema change needed, only phase ID values change |
