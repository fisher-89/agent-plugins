## MODIFIED Requirements

### Requirement: eval-check validates prior phase gate
The system SHALL provide an `phase_check` MCP tool (formerly `phase/check`, originally `eval_check`) that validates all prior phases have at least one entry with verdict "pass" OR `skipped: true` in eval.json before allowing the current phase to proceed.

When invoked as `mcp__plugin_dev-team_dev-team__phase_check` with arguments `change` and `phase`, the tool SHALL perform gate check, timestamp order check, and backtrack check. The behavior and output format SHALL remain identical to the former `phase/check` tool.

If any prior phase lacks both a pass record and a skipped record, the result SHALL indicate `passed: false` and list the missing phases.
If all prior phases have pass or skipped records, the result SHALL indicate `passed: true`.
For the first phase (01-requirements), the system SHALL skip prior phase gate validation entirely.
The prior phase sequence SHALL follow the 9-phase structure: 01-requirements, 02-dev-design, 03-test-design, 04-test-gen, 05-implement, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance.

#### Scenario: All prior phases have pass records
- **WHEN** eval.json contains entries with verdict "pass" for all phases prior to the specified `phase`
- **THEN** the prior phase gate check passes and execution continues to sequential ordering check

#### Scenario: Prior phase has skipped record instead of pass
- **WHEN** a prior phase has no "pass" entry but has an entry with `skipped: true` and verdict "pass"
- **THEN** the prior phase gate check passes — the skipped phase is treated as valid

#### Scenario: Prior phase (unit-test) missing both pass and skipped
- **WHEN** eval.json does not contain any entry with verdict "pass" or `skipped: true` for phase "06-unit-test" when checking prior phases for "08-integration-test"
- **THEN** the command exits with code 1 and output lists "06-unit-test" as a missing phase

#### Scenario: Integration-test phase gated on code-review pass
- **WHEN** phase_check validates prior phases for "08-integration-test"
- **THEN** it checks that "07-code-review" has a pass or skipped record before allowing integration-test to proceed

### Requirement: eval-check reports current phase state
The system SHALL report the current phase state based on eval.json entries for the specified phase via the `phase_check` MCP tool (formerly `phase/check`, originally `eval_check`):
- `"first_run"` when the phase has no entries in eval.json
- `"retry"` when the phase has entries but the latest entry has verdict "fail" and `skipped` is not true
- `"passed"` when the phase has entries and the latest entry has verdict "pass" (or `skipped: true`)
- `"skipped"` when the phase has entries and the latest entry has `skipped: true`

The phase state SHALL be included in both human-readable output and JSON output.
This behavior SHALL apply to all 9 phases including the new unit-test (06) and integration-test (08) phases.

#### Scenario: Phase has no entries
- **WHEN** the specified phase has zero entries in eval.json
- **THEN** phase_state is reported as "first_run"

#### Scenario: Phase was skipped
- **WHEN** the specified phase has entries and the latest entry has `skipped: true`
- **THEN** phase_state is reported as "skipped"

#### Scenario: Unit-test phase has failing entries
- **WHEN** the specified phase is "06-unit-test" and its entries have verdict "fail" (without skipped)
- **THEN** phase_state is reported as "retry"

#### Scenario: Integration-test phase has passing entries
- **WHEN** the specified phase is "08-integration-test" and its latest entry has verdict "pass"
- **THEN** phase_state is reported as "passed"

### Requirement: eval-check --json output includes skipped state
The JSON output via `phase_check` MCP tool SHALL contain at minimum these fields:
- `passed`: boolean indicating whether all checks passed
- `phase`: the phase identifier specified
- `prior_phases`: array of prior phase identifiers
- `block_reasons`: array of human-readable reasons for blocking (empty array when passed is true)
- `phase_state`: "first_run" | "retry" | "passed" | "skipped"
- `skipped_phases`: array of phase identifiers that were skipped (empty array when none)
- `exit_code`: numeric exit code (0 or 1)

The `skipped_phases` field SHALL be populated by scanning eval.json for entries with `skipped: true`.

#### Scenario: JSON output with skipped phases
- **WHEN** JSON is returned and prior phase "08-integration-test" was skipped
- **THEN** JSON contains `"skipped_phases": ["08-integration-test"]` and `"phase_state": "<state>"`

### Requirement: phase_log 记录评估结果
The system SHALL provide an `phase_log` MCP tool (formerly `phase/log`, originally `eval_log`) that appends an evaluation result entry to eval.json for a given workflow phase. The tool SHALL be invoked as `mcp__plugin_dev-team_dev-team__phase_log`.

The tool SHALL accept the same parameters as the former `phase/log`: change, phase, verdict, report, items, attempt, backtrack_to, skipped, findings. The behavior, validation, and output format SHALL remain identical.

#### Scenario: phase_log 追加 pass 记录
- **WHEN** `phase_log` is called with `verdict: "pass"`, valid `change` and `phase`
- **THEN** a pass entry is appended to `openspec/changes/<name>/eval.json`
- **AND** the entry includes all provided fields

#### Scenario: phase_log 追加 fail 记录含 backtrack
- **WHEN** `phase_log` is called with `verdict: "fail"` and `backtrack_to: "02-dev-design"`
- **THEN** a fail entry is appended with backtrack information

## Module Contract

### MCP Tools

| Tool Name | Description | Contract |
|-----------|-------------|----------|
| `phase_check` | Prior phase gate validation | Uses `xx_yy` naming; behavior identical to former `phase/check` |
| `phase_log` | Append evaluation result | Uses `xx_yy` naming; behavior identical to former `phase/log` |

### CLI Commands (unchanged by this rename)

| Command | Description | Contract |
|---------|-------------|----------|
| `dev-team eval-check` | CLI gate check (unchanged) | CLI name unaffected by MCP tool rename |
| `dev-team eval-log` | CLI eval log (unchanged) | CLI name unaffected by MCP tool rename |
