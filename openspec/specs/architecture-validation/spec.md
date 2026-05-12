# architecture-validation Specification

## Purpose
TBD - created by archiving change add-architect-role. Update Purpose after archive.
## Requirements
### Requirement: Validate all changed files
The system SHALL check all files in `git diff --cached` against the architecture model, not only files matched by `metadata.path`.

#### Scenario: All staged files are validated
- **WHEN** `git diff --cached` contains 5 files, 3 matched to elements and 2 unmatched
- **THEN** the validation report SHALL include entries for all 5 files

#### Scenario: No model exists
- **WHEN** `openspec/architecture/model.c4` does not exist
- **THEN** the system SHALL skip validation and report "no model to validate against"

### Requirement: Map changed files to model elements via metadata.path
The system SHALL parse the model and build a mapping from `metadata.path` to element IDs. Each changed file SHALL be checked against this mapping.

#### Scenario: File matches an element
- **WHEN** changed file `src/services/payment/handler.ts` falls under element `paymentService` with `metadata.path "./src/services/payment/"`
- **THEN** the file SHALL be grouped under `paymentService` in the report

#### Scenario: File matches no element
- **WHEN** changed file `src/scripts/cleanup.ts` does not fall under any element's `metadata.path`
- **THEN** the file SHALL be listed under `unmatched_files` in the report

### Requirement: Dependency-to-relationship cross-reference
The system SHALL parse import statements in changed files, map import targets to model elements via `metadata.path`, and verify that corresponding relationships exist in the model.

#### Scenario: Import matches a declared relationship
- **WHEN** element A's file imports from a path mapped to element B, and model declares `A -> B`
- **THEN** no violation is reported

#### Scenario: Import without a declared relationship
- **WHEN** element A's file imports from a path mapped to element B, but model has no `A -> B` relationship
- **THEN** a violation of type `unmodeled_dependency` SHALL be reported

#### Scenario: Import target not mapped to any element
- **WHEN** element A's file imports from `./utils/logger` which matches no element's `metadata.path`
- **THEN** a warning of type `unmapped_import_target` SHALL be reported

#### Scenario: Self-import within same element
- **WHEN** element A's file imports another file also mapped to element A
- **THEN** the import SHALL be ignored (intra-element dependencies are not architecture violations)

### Requirement: Detect unused relationships
The system SHALL check whether each declared relationship in the model has corresponding import evidence in the code.

#### Scenario: Relationship has no code evidence
- **WHEN** model declares `A -> B` but no import in A's code targets B's path
- **THEN** a warning of type `unused_relationship` SHALL be reported

### Requirement: Output JSON validation report
The system SHALL write a structured JSON report to `openspec/architecture/reports/validate-<timestamp>.json`.

#### Scenario: Report contains matched elements
- **WHEN** validation runs with changed files that match model elements
- **THEN** the report SHALL include a `matched` array with `element_id` and `files` for each matched element

#### Scenario: Report contains violations
- **WHEN** validation detects unmodeled dependencies
- **THEN** the report SHALL include a `violations` array with `type`, `source_file`, `import_target`, and `detail`

#### Scenario: Report contains model change summary
- **WHEN** `architecture/` directory files are also in the staged changes
- **THEN** the report SHALL include a `model_changes` field summarizing added, removed, and modified elements and relationships

