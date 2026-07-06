## MODIFIED Requirements

### Requirement: phaseIdSchema updated to remove unit-test and integration-test, add test-execution

The `phaseIdSchema` enum in `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` SHALL be updated:

**Before**:
```typescript
export const phaseIdSchema = z
  .enum([
    'proposal',
    'dev-design',
    'test-design',
    'implement',
    'test-gen',
    'unit-test',
    'code-review',
    'integration-test',
    'acceptance',
    'code-analyze',
  ])
```

**After**:
```typescript
export const phaseIdSchema = z
  .enum([
    'proposal',
    'dev-design',
    'test-design',
    'implement',
    'test-gen',
    'test-execution',
    'code-review',
    'acceptance',
    'code-analyze',
  ])
```

#### Scenario: phase/log accepts test-execution phase

**WHEN** MCP phase_log is called with phase `test-execution`
**THEN** the call succeeds (no validation error)
**AND** the entry is appended to eval.json with phase `test-execution`

#### Scenario: phase/log rejects unit-test phase

**WHEN** MCP phase_log is called with phase `unit-test`
**THEN** the call fails with validation error (not in enum)
**AND** no entry is written to eval.json

#### Scenario: phase/log rejects integration-test phase

**WHEN** MCP phase_log is called with phase `integration-test`
**THEN** the call fails with validation error (not in enum)
**AND** no entry is written to eval.json

#### Scenario: phase/log still accepts code-analyze (unchanged)

**WHEN** MCP phase_log is called with phase `code-analyze`
**THEN** the call succeeds (code-analyze remains in enum)

### Requirement: phaseIdSchema length validation

The phaseIdSchema SHALL have exactly 9 enum values (was 10 before the change):

| Enum Value | Status |
|-----------|--------|
| `proposal` | Unchanged |
| `dev-design` | Unchanged |
| `test-design` | Unchanged |
| `implement` | Unchanged |
| `test-gen` | Unchanged |
| `test-execution` | ADDED |
| `unit-test` | REMOVED |
| `code-review` | Unchanged |
| `integration-test` | REMOVED |
| `acceptance` | Unchanged |
| `code-analyze` | Unchanged |

#### Scenario: phaseIdSchema has exactly 9 values

**WHEN** inspecting `phaseIdSchema.options`
**THEN** the array length is 9 (was 10)

## Module Contract

### phase-log.schema.ts

| Export | Change | Description |
|--------|--------|-------------|
| `phaseIdSchema` | MODIFIED | Enum: removed `unit-test` and `integration-test`; added `test-execution`. 10 values → 9 values. |
| `phaseLogSchema` | UNCHANGED | Schema structure unchanged — only validates against updated phaseIdSchema |
| `phaseLogInputSchema` | UNCHANGED | Input schema structure unchanged |
