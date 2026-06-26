## MODIFIED Requirements

### Requirement: phase_log backtrack validation uses prefix-free phase IDs
When `runPhaseLog()` validates backtrack targets against the workflow phase table, both the `phase` parameter and `backtrack_to` targets SHALL use prefix-free phase IDs.

The `handleBacktrackMarking()` function SHALL use the new phase table (with prefix-free IDs) to validate:
1. `options.phase` exists in the phase table
2. Each backtrack target exists in the phase table
3. Each backtrack target precedes `options.phase` in array order

When a backtrack target is not found in the prefix-free phase table, the error message SHALL list available phases using the new IDs.

#### Scenario: phase_log validates backtrack with prefix-free IDs
- **WHEN** `phase_log` is called with `phase: "unit-test"`, `backtrack_to: "test-gen"`
- **AND** workflow phase table uses prefix-free IDs
- **THEN** `handleBacktrackMarking()` validates `"test-gen"` exists in the table
- **AND** the call succeeds (phase exists and precedes current)

#### Scenario: phase_log rejects invalid backtrack with prefix-free ID in error
- **WHEN** `phase_log` is called with `phase: "unit-test"`, `backtrack_to: "old-05-implement"`
- **AND** the old ID `"old-05-implement"` is not in the prefix-free phase table
- **THEN** the call is rejected with an error
- **AND** the error message lists available phases using prefix-free IDs (e.g., `proposal`, `dev-design`, `implement`, etc.)

## Module Contract

### phase-log.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `runPhaseLog()` | UNCHANGED | No API change; references phase table from `workflow.ts` which now has prefix-free IDs |
| `handleBacktrackMarking()` | UNCHANGED | No logic change; validates against whatever phase table `getPhaseTable()` returns |
| `resolveVerdict()` | UNCHANGED | No phase ID dependency |
