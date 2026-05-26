## MODIFIED Requirements

### Requirement: eval-check validates prior phase gate
The system SHALL provide an `eval-check` subcommand that validates all prior phases have at least one entry with verdict "pass" OR `skipped: true` in eval.json before allowing the current phase to proceed.
If any prior phase lacks both a pass record and a skipped record, the system SHALL exit with code 1 and list the missing phases.
If all prior phases have pass or skipped records, the system SHALL continue to subsequent checks.
For the first phase (01-requirements), the system SHALL skip prior phase gate validation entirely.
The prior phase sequence SHALL follow the 9-phase structure: 01-requirements, 02-test-design, 03-dev-proposal, 04-test-gen, 05-implement, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance.

#### Scenario: All prior phases have pass records
- **WHEN** eval.json contains entries with verdict "pass" for all phases prior to the specified `--phase`
- **THEN** the prior phase gate check passes and execution continues to sequential ordering check

#### Scenario: Prior phase has skipped record instead of pass
- **WHEN** a prior phase has no "pass" entry but has an entry with `skipped: true` and verdict "pass"
- **THEN** the prior phase gate check passes — the skipped phase is treated as valid

#### Scenario: Prior phase (unit-test) missing both pass and skipped
- **WHEN** eval.json does not contain any entry with verdict "pass" or `skipped: true` for phase "06-unit-test" when checking prior phases for "08-integration-test"
- **THEN** the command exits with code 1 and output lists "06-unit-test" as a missing phase

#### Scenario: Integration-test phase gated on code-review pass
- **WHEN** eval-check validates prior phases for "08-integration-test"
- **THEN** it checks that "07-code-review" has a pass or skipped record before allowing integration-test to proceed

### Requirement: eval-check reports current phase state
The system SHALL report the current phase state based on eval.json entries for the specified `--phase`:
- `"first_run"` when the phase has no entries in eval.json
- `"retry"` when the phase has entries but the latest entry has verdict "fail" and `skipped` is not true
- `"passed"` when the phase has entries and the latest entry has verdict "pass" (or `skipped: true`)
- `"skipped"` when the phase has entries and the latest entry has `skipped: true`

The phase state SHALL be included in both human-readable output and JSON output.
This behavior SHALL apply to all 9 phases including the new unit-test (06) and integration-test (08) phases.

#### Scenario: Phase has no entries
- **WHEN** the specified `--phase` has zero entries in eval.json
- **THEN** phase_state is reported as "first_run"

#### Scenario: Phase was skipped
- **WHEN** the specified `--phase` has entries and the latest entry has `skipped: true`
- **THEN** phase_state is reported as "skipped"

#### Scenario: Unit-test phase has failing entries
- **WHEN** the specified `--phase` is "06-unit-test" and its entries have verdict "fail" (without skipped)
- **THEN** phase_state is reported as "retry"

#### Scenario: Integration-test phase has passing entries
- **WHEN** the specified `--phase` is "08-integration-test" and its latest entry has verdict "pass"
- **THEN** phase_state is reported as "passed"

### Requirement: eval-check --json output includes skipped state
The JSON output SHALL contain at minimum these fields:
- `passed`: boolean indicating whether all checks passed
- `phase`: the phase identifier specified via `--phase`
- `prior_phases`: array of prior phase identifiers
- `block_reasons`: array of human-readable reasons for blocking (empty array when passed is true)
- `phase_state`: "first_run" | "retry" | "passed" | "skipped"
- `skipped_phases`: array of phase identifiers that were skipped (empty array when none)
- `exit_code`: numeric exit code (0 or 1)

The `skipped_phases` field SHALL be populated by scanning eval.json for entries with `skipped: true`.

#### Scenario: JSON output with skipped phases
- **WHEN** `--json` flag is provided and prior phase "08-integration-test" was skipped
- **THEN** stdout contains valid JSON with `"skipped_phases": ["08-integration-test"]` and `"phase_state": "<state>"`
