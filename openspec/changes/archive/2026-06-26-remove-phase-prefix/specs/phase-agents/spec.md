## MODIFIED Requirements

### Requirement: Evaluator agents call phase_log with prefix-free phase ID
All evaluator agent `.md` files SHALL reference their phase ID without the numeric prefix when calling the `mcp__plugin_dev-team_dev-team__phase_log` MCP tool. The following evaluator agent files are affected:

| Agent File | Old Phase ID | New Phase ID |
|-----------|-------------|--------------|
| `proposal-evaluator.md` | `01-proposal` | `proposal` |
| `dev-design-evaluator.md` | `02-dev-design` | `dev-design` |
| `test-design-evaluator.md` | `03-test-design` | `test-design` |
| `test-gen-evaluator.md` | `04-test-gen` | `test-gen` |
| `implementation-evaluator.md` | `05-implement` | `implement` |
| `code-analyze-evaluator.md` | `02-code-analyze` | `code-analyze` |
| `code-review-evaluator.md` | `07-code-review` | `code-review` |
| `acceptance-evaluator.md` | `09-acceptance` | `acceptance` |
| `unit-test-evaluator.md` | `06-unit-test` | `unit-test` |
| `integration-test-evaluator.md` | `08-integration-test` | `integration-test` |

The `phase_log` call within each evaluator agent SHALL use the prefix-free phase ID as the `phase` parameter value.

#### Scenario: proposal-evaluator calls phase_log with prefix-free ID
- **WHEN** reading `proposal-evaluator.md`
- **THEN** the `phase_log` call uses `phase: "proposal"` (not `"01-proposal"`)

#### Scenario: acceptance-evaluator calls phase_log with prefix-free ID
- **WHEN** reading `acceptance-evaluator.md`
- **THEN** the `phase_log` call uses `phase: "acceptance"` (not `"09-acceptance"`)

### Requirement: Executor agents call phase_log with prefix-free phase ID
Executor agent `.md` files SHALL use prefix-free phase IDs when calling `mcp__plugin_dev-team_dev-team__phase_log` for no-op skip entries:

| Agent File | Old Phase ID | New Phase ID |
|-----------|-------------|--------------|
| `unit-test-executor.md` | `06-unit-test` | `unit-test` |
| `integration-test-executor.md` | `08-integration-test` | `integration-test` |

#### Scenario: unit-test-executor uses prefix-free ID for skip
- **WHEN** reading `unit-test-executor.md`
- **THEN** the `phase_log` call for no-op skip uses `phase: "unit-test"` (not `"06-unit-test"`)

#### Scenario: integration-test-executor uses prefix-free ID for skip
- **WHEN** reading `integration-test-executor.md`
- **THEN** the `phase_log` call for no-op skip uses `phase: "integration-test"` (not `"08-integration-test"`)

### Requirement: Test-execution evaluator backtrack targets use prefix-free IDs
`unit-test-evaluator.md` and `integration-test-evaluator.md` SHALL update all backtrack target references to use prefix-free phase IDs:

| Agent File | Old Backtrack Targets | New Backtrack Targets |
|-----------|----------------------|----------------------|
| `unit-test-evaluator.md` | `04-test-gen`, `05-implement`, `03-test-design`, `02-dev-design` | `test-gen`, `implement`, `test-design`, `dev-design` |
| `integration-test-evaluator.md` | `04-test-gen`, `05-implement`, `03-test-design`, `02-dev-design`, `08-integration-test` | `test-gen`, `implement`, `test-design`, `dev-design`, `integration-test` |

#### Scenario: unit-test-evaluator backtrack targets use prefix-free IDs (AC-7)
- **WHEN** reading `unit-test-evaluator.md`
- **THEN** backtrack targets reference `test-gen`, `implement`, `test-design`, `dev-design` (without numeric prefix)

#### Scenario: integration-test-evaluator backtrack targets use prefix-free IDs
- **WHEN** reading `integration-test-evaluator.md`
- **THEN** backtrack targets reference `test-gen`, `implement`, `test-design`, `dev-design` (without numeric prefix)

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent File | Change | Details |
|-----------|--------|---------|
| `proposal-evaluator.md` | MODIFIED | `phase_log` phase: `"01-proposal"` → `"proposal"` |
| `dev-design-evaluator.md` | MODIFIED | `phase_log` phase: `"02-dev-design"` → `"dev-design"` |
| `test-design-evaluator.md` | MODIFIED | `phase_log` phase: `"03-test-design"` → `"test-design"` |
| `test-gen-evaluator.md` | MODIFIED | `phase_log` phase: `"04-test-gen"` → `"test-gen"` |
| `implementation-evaluator.md` | MODIFIED | `phase_log` phase: `"05-implement"` → `"implement"` |
| `code-analyze-evaluator.md` | MODIFIED | `phase_log` phase: `"02-code-analyze"` → `"code-analyze"` |
| `code-review-evaluator.md` | MODIFIED | `phase_log` phase: `"07-code-review"` → `"code-review"` |
| `acceptance-evaluator.md` | MODIFIED | `phase_log` phase: `"09-acceptance"` → `"acceptance"` |
| `unit-test-evaluator.md` | MODIFIED | `phase_log` phase: `"06-unit-test"` → `"unit-test"`; backtrack targets prefix-free |
| `integration-test-evaluator.md` | MODIFIED | `phase_log` phase: `"08-integration-test"` → `"integration-test"`; backtrack targets prefix-free |
| `unit-test-executor.md` | MODIFIED | `phase_log` phase: `"06-unit-test"` → `"unit-test"` |
| `integration-test-executor.md` | MODIFIED | `phase_log` phase: `"08-integration-test"` → `"integration-test"` |
