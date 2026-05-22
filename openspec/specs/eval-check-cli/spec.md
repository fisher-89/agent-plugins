# eval-check-cli Specification

## Purpose
Provides an `eval-check` CLI subcommand that validates prerequisite PGE phases have passed before allowing a workflow step to proceed. Performs gate checking, sequential ordering validation, backtrack marker detection, and phase state reporting.

## Requirements

### Requirement: eval-check validates prior phase gate
The system SHALL provide an `eval-check` subcommand that validates all prior phases have at least one entry with verdict "pass" in eval.json before allowing the current phase to proceed.
If any prior phase lacks a pass record, the system SHALL exit with code 1 and list the missing phases.
If all prior phases have pass records, the system SHALL continue to subsequent checks.
For the first phase (01-requirements), the system SHALL skip prior phase gate validation entirely.

#### Scenario: All prior phases have pass records
- **WHEN** eval.json contains entries with verdict "pass" for all phases prior to the specified `--phase`
- **THEN** the prior phase gate check passes and execution continues to sequential ordering check

#### Scenario: Prior phase missing pass record
- **WHEN** eval.json does not contain any entry with verdict "pass" for a prior phase
- **THEN** the command exits with code 1 and output lists the missing phase identifiers

#### Scenario: First phase has no prior phases
- **WHEN** `--phase 01-requirements` is specified
- **THEN** prior phase gate validation is skipped and prior_phases list is empty

### Requirement: eval-check validates sequential ordering of prior phase passes
The system SHALL validate that the pass records of prior phases appear in monotonically increasing timestamp order, matching the workflow phase sequence (01-requirements before 02-test-design before 03-dev-proposal, etc.).
If the pass timestamp of a later prior phase is earlier than or equal to an earlier prior phase, the system SHALL exit with code 1.
For identical timestamps (same millisecond), the system SHALL treat the order as correct to accommodate concurrent writes.

#### Scenario: Prior phase passes in correct sequence
- **WHEN** pass timestamps are strictly monotonically increasing (ts(01) < ts(02) < ts(03))
- **THEN** sequential ordering check passes

#### Scenario: Prior phase passes out of sequence
- **WHEN** a later prior phase has a pass timestamp earlier than or equal to an earlier prior phase
- **THEN** the command exits with code 1 and output indicates the ordering violation

### Requirement: eval-check reports current phase state
The system SHALL report the current phase state based on eval.json entries for the specified `--phase`:
- `"first_run"` when the phase has no entries in eval.json
- `"retry"` when the phase has entries but the latest entry has verdict "fail"
- `"passed"` when the phase has entries and the latest entry has verdict "pass"
The phase state SHALL be included in both human-readable output and JSON output.

#### Scenario: Phase has no entries
- **WHEN** the specified `--phase` has zero entries in eval.json
- **THEN** phase_state is reported as "first_run"

#### Scenario: Phase has failing entries
- **WHEN** the specified `--phase` has entries and the latest entry has verdict "fail"
- **THEN** phase_state is reported as "retry"

#### Scenario: Phase has passing entries
- **WHEN** the specified `--phase` has entries and the latest entry has verdict "pass"
- **THEN** phase_state is reported as "passed"

### Requirement: eval-check detects active backtrack markers on prior phases
The system SHALL inspect the latest eval.json entry for each prior phase. If any prior phase's latest entry has a non-null `backtrack_to` field, the system SHALL exit with code 1 and report the backtrack target.
This prevents progression when a prior phase has an active backtrack marker that requires re-evaluation.

#### Scenario: Prior phase has active backtrack marker
- **WHEN** a prior phase's latest eval entry contains `"backtrack_to": "01-requirements"`
- **THEN** the command exits with code 1 and output includes "backtrack" and the target phase identifier

#### Scenario: No backtrack markers
- **WHEN** no prior phase's latest entry contains a non-null backtrack_to field
- **THEN** backtrack check passes and execution continues

### Requirement: eval-check supports --json output flag
The system SHALL support a `--json` flag that outputs structured JSON to stdout.
The JSON output SHALL contain at minimum these fields:
- `passed`: boolean indicating whether all checks passed
- `phase`: the phase identifier specified via `--phase`
- `prior_phases`: array of prior phase identifiers
- `block_reasons`: array of human-readable reasons for blocking (empty array when passed is true)
- `phase_state`: "first_run" | "retry" | "passed"
- `exit_code`: numeric exit code (0 or 1)

#### Scenario: JSON output on pass
- **WHEN** `--json` flag is provided and all checks pass
- **THEN** stdout contains valid JSON with `"passed": true`, `"block_reasons": []`, `"phase_state": "<state>"`, and `"exit_code": 0`

#### Scenario: JSON output on block
- **WHEN** `--json` flag is provided and a check fails
- **THEN** stdout contains valid JSON with `"passed": false`, non-empty `"block_reasons"` array, and `"exit_code": 1`

### Requirement: eval-check validates phase name against workflow phases
The system SHALL validate that the `--phase` argument matches a known workflow phase identifier from the PHASES array (01-requirements, 02-test-design, 03-dev-proposal, 04-test-gen, 05-implementation, 06-code-review, 07-acceptance).
If the phase name is not recognized, the system SHALL exit with code 1 and output a meaningful error message.

#### Scenario: Valid phase name
- **WHEN** `--phase 05-implementation` is specified
- **THEN** phase name is accepted and processing continues

#### Scenario: Invalid phase name
- **WHEN** `--phase invalid-phase` is specified
- **THEN** the command exits with code 1 with an error message indicating the invalid phase name

### Requirement: eval-check --change resolves to valid change directory
The system SHALL resolve the `--change <name>` argument to `openspec/changes/<name>/phases/eval.json`.
If the change directory does not exist, the system SHALL exit with code 1 with a meaningful error message.

#### Scenario: Change exists
- **WHEN** `--change existing-change` is specified and the directory `openspec/changes/existing-change/` exists
- **THEN** eval.json is read from that change's phases directory

#### Scenario: Change does not exist
- **WHEN** `--change non-existent` is specified and no such directory exists
- **THEN** the command exits with code 1 with an error message indicating the change was not found
