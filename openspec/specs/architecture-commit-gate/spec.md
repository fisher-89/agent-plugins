## MODIFIED Requirements

### Requirement: Intercept git commit

The system SHALL intercept all `git commit` commands via a PreToolUse(Bash) hook and verify architecture validation status.

#### Scenario: Commit with validation report

- **WHEN** `git diff --cached` contains code changes AND `openspec/changes/<name>/reports/` contains a staged `architecture-architecture-validate-*.json`
- **THEN** the hook SHALL allow the commit

#### Scenario: Commit without validation report

- **WHEN** `git diff --cached` contains code changes AND no `architecture-validate-*.json` is staged
- **THEN** the hook SHALL deny the commit with a message instructing the agent to run archi-validate

#### Scenario: Commit with only architecture files

- **WHEN** `git diff --cached` contains only files under `openspec/specs/architecture/`
- **THEN** the hook SHALL allow the commit without requiring a validation report

#### Scenario: Commit with no code changes

- **WHEN** `git diff --cached` is empty or contains only non-code files (docs, config not mapped to any element)
- **THEN** the hook SHALL allow the commit

### Requirement: Model existence check uses models directory

The hook SHALL check for the existence of architecture model by looking for `openspec/specs/architecture/models/` directory containing at least one `*.c4` file. For backward compatibility, it SHALL also check the legacy `openspec/specs/architecture/model.c4` file.

#### Scenario: Models directory exists

- **WHEN** `openspec/specs/architecture/models/` contains at least one `*.c4` file
- **THEN** the hook SHALL treat the model as existing

#### Scenario: Only legacy model.c4 exists

- **WHEN** `openspec/specs/architecture/models/` does not exist but `openspec/specs/architecture/model.c4` exists
- **THEN** the hook SHALL treat the model as existing (backward compatible)

#### Scenario: No model in either location

- **WHEN** neither `models/` nor `model.c4` exists
- **THEN** the hook SHALL skip the architecture gate

### Requirement: Validation report must be staged

The system SHALL check that the validation report is staged in the same commit as the code changes it covers.

#### Scenario: Report exists but is not staged

- **WHEN** `architecture-validate-*.json` exists in the working tree but is not in `git diff --cached`
- **THEN** the hook SHALL deny the commit and instruct the agent to stage the report

### Requirement: Detection scope covers all changed files

The system SHALL check all files in the staged changes, not only those matching model `metadata.path`.

#### Scenario: Unmatched staged file

- **WHEN** `git diff --cached` includes a file not covered by any element's `metadata.path`
- **THEN** the file SHALL still require validation; the hook SHALL NOT skip it

### Requirement: Hook output guides agent to run validation

When denying a commit, the hook SHALL output a clear instruction telling the agent to invoke the archi-validate sub-agent.

#### Scenario: Deny message contains action guidance

- **WHEN** the hook denies a commit
- **THEN** the output SHALL instruct the agent to run archi-validate, stage the resulting report, and retry the commit
