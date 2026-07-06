## MODIFIED Requirements

### Requirement: Evaluator agents call phase_log with updated phase ID

All evaluator agent `.md` files SHALL reference their phase ID using the consolidated naming. The following evaluator agent files are affected:

| Agent File | Old Phase ID | New Phase ID |
|-----------|-------------|--------------|
| `proposal-evaluator.md` | `proposal` | `proposal` (unchanged) |
| `dev-design-evaluator.md` | `dev-design` | `dev-design` (unchanged) |
| `test-design-evaluator.md` | `test-design` | `test-design` (unchanged) |
| `test-gen-evaluator.md` | `test-gen` | `test-gen` (unchanged) |
| `implementation-evaluator.md` | `implement` | `implement` (unchanged) |
| `code-analyze-evaluator.md` | `code-analyze` | `code-analyze` (unchanged) |
| `code-review-evaluator.md` | `code-review` | `code-review` (unchanged) |
| `acceptance-evaluator.md` | `acceptance` | `acceptance` (unchanged) |
| `unit-test-evaluator.md` | `unit-test` | DELETED |
| `test-execution-evaluator.md` | NEW | `test-execution` |
| `integration-test-evaluator.md` | `integration-test` | DELETED |

#### Scenario: test-execution-evaluator calls phase_log with updated ID
- **WHEN** reading `test-execution-evaluator.md`
- **THEN** the `phase_log` call uses `phase: "test-execution"`

#### Scenario: integration-test-evaluator does not exist
- **WHEN** checking for `integration-test-evaluator.md`
- **THEN** the file SHALL NOT exist in the agents/ directory

### Requirement: Executor agents call phase_log with updated phase ID

Executor agent `.md` files SHALL use the updated phase IDs when calling `mcp__plugin_dev-team_dev-team__phase_log` for no-op skip entries:

| Agent File | Old Phase ID | New Phase ID |
|-----------|-------------|--------------|
| `unit-test-executor.md` | `unit-test` | DELETED |
| `test-execution-executor.md` | NEW | `test-execution` |
| `integration-test-executor.md` | `integration-test` | DELETED |

#### Scenario: test-execution-executor uses updated ID for skip
- **WHEN** reading `test-execution-executor.md`
- **THEN** the `phase_log` call for no-op skip uses `phase: "test-execution"`

### Requirement: Test-execution evaluator backtrack targets use consolidated IDs

`test-execution-evaluator.md` SHALL update backtrack target references:

| Agent File | Old Backtrack Targets | New Backtrack Targets |
|-----------|----------------------|----------------------|
| `unit-test-evaluator.md` | `test-gen`, `implement`, `test-design`, `dev-design` | DELETED |
| `test-execution-evaluator.md` | NEW | `test-gen`, `implement`, `test-design`, `dev-design` |
| `integration-test-evaluator.md` | `test-gen`, `implement`, `test-design`, `dev-design`, `integration-test` | DELETED |

#### Scenario: test-execution-evaluator backtrack targets use consolidated IDs
- **WHEN** reading `test-execution-evaluator.md`
- **THEN** backtrack targets reference `test-gen`, `implement`, `test-design`, `dev-design` only
- **AND** do NOT include `integration-test`

## REMOVED Requirements

### Requirement: unit-test-evaluator calls phase_log with prefix-free phase ID
**Reason**: Agent file renamed. The new `test-execution-evaluator.md` replaces it.
**Migration**: See test-execution-evaluator entries above.

### Requirement: integration-test-evaluator calls phase_log with prefix-free phase ID
**Reason**: Agent file deleted. All testing is consolidated into test-execution.
**Migration**: Replace all references to integration-test-evaluator with test-execution-evaluator.

### Requirement: unit-test-executor calls phase_log with prefix-free phase ID
**Reason**: Agent file renamed to `test-execution-executor.md`.
**Migration**: New executor agent uses `phase: "test-execution"`.

### Requirement: integration-test-executor calls phase_log with prefix-free phase ID
**Reason**: Agent file deleted. All testing is consolidated into test-execution.
**Migration**: No replacement needed.

### Requirement: unit-test-executor agent calls test_detect_frameworks and test_get_framework_config MCP tools
**Reason**: Agent file renamed. The test-execution-executor follows same pattern but with updated file paths and phase references.
**Migration**: The new agent at `test-execution-executor.md` contains equivalent instructions with updated references.

### Requirement: unit-test-evaluator agent reads expanded coverage fields from report
**Reason**: Agent file renamed. The test-execution-evaluator follows same pattern with updated report paths.
**Migration**: The new agent at `test-execution-evaluator.md` reads `reports/test-execution.json` instead of `reports/unit-test-execution.json`.

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent File | Change | Details |
|-----------|--------|---------|
| `unit-test-executor.md` | RENAMED → `test-execution-executor.md` | Prompt updated: "Run and fix tests" (was "unit tests"). Phase_log uses `test-execution`. Runs all test types. |
| `unit-test-evaluator.md` | RENAMED → `test-execution-evaluator.md` | Phase_log uses `test-execution`. Backtrack targets updated. Reads `reports/test-execution.json`. |
| `integration-test-executor.md` | DELETED | Integration testing merged into test-execution-executor |
| `integration-test-evaluator.md` | DELETED | Integration testing merged into test-execution-evaluator |

### Agent: test-execution-executor.md

| Aspect | Value |
|--------|-------|
| **Model** | `sonnet-4.6` |
| **Phase** | `test-execution` |
| **Input** | `reports/test-execution.json` (CLI-generated) |
| **Scope** | All automated tests (unit + integration combined) |

### Agent: test-execution-evaluator.md

| Aspect | Value |
|--------|-------|
| **Model** | sonnet (as defined in system definition) |
| **Phase** | `test-execution` |
| **Input** | `reports/test-execution.json` |
| **Backtrack targets** | `test-gen`, `implement`, `test-design`, `dev-design` |
| **Backtrack NOT allowed** | `proposal`, `acceptance` |
