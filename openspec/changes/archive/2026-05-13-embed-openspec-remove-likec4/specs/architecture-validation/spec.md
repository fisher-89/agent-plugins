## MODIFIED Requirements

### Requirement: Map changed files to model elements via metadata.path
The system SHALL parse the model using a pure Python DSL parser and build a mapping from `metadata.path` to element IDs. Each changed file SHALL be checked against this mapping. No external CLI (likec4) SHALL be required for this operation.

#### Scenario: File matches an element
- **WHEN** changed file `src/services/payment/handler.ts` falls under element `paymentService` with `metadata.path "./src/services/payment/"`
- **THEN** the file SHALL be grouped under `paymentService` in the report

#### Scenario: File matches no element
- **WHEN** changed file `src/scripts/cleanup.ts` does not fall under any element's `metadata.path`
- **THEN** the file SHALL be listed under `unmatched_files` in the report
