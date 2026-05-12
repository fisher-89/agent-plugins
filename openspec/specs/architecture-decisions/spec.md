# architecture-decisions Specification

## Purpose
TBD - created by archiving change add-architect-role. Update Purpose after archive.
## Requirements
### Requirement: Create ADR record
The system SHALL create a new Architecture Decision Record in `openspec/architecture/decisions/` with the filename format `YYYY-MM-DD-<kebab-title>.md`.

#### Scenario: Create new ADR
- **WHEN** the sub-agent creates an ADR with title "Use PostgreSQL for Primary Store" on 2026-05-12
- **THEN** the file SHALL be created at `decisions/2026-05-12-use-postgresql-for-primary-store.md`

#### Scenario: ADR contains all required fields
- **WHEN** an ADR is created
- **THEN** the file SHALL contain: title, date, status, background, decision, consequences, alternatives, and affected scope

### Requirement: ADR status field
The system SHALL support four status values: `proposed`, `accepted`, `deprecated`, `superseded`.

#### Scenario: New ADR starts as proposed
- **WHEN** a new ADR is created without explicit status
- **THEN** the status SHALL default to `proposed`

#### Scenario: Superseded ADR references replacement
- **WHEN** an ADR is marked `superseded`
- **THEN** the ADR SHALL include a reference to the superseding ADR

### Requirement: Affected scope references model elements
The system SHALL include an `## 影响范围` section that lists model element IDs affected by the decision.

#### Scenario: ADR references model elements
- **WHEN** a decision affects `paymentService` and `apiGateway` elements
- **THEN** the scope section SHALL list `paymentService` and `apiGateway`

### Requirement: Query ADRs
The system SHALL support listing all ADRs and filtering by status.

#### Scenario: List all ADRs
- **WHEN** the sub-agent queries all ADRs
- **THEN** the system SHALL return ADRs sorted by date descending

#### Scenario: Filter by status
- **WHEN** the sub-agent queries ADRs with status `accepted`
- **THEN** the system SHALL return only ADRs with status `accepted`

### Requirement: Update ADR status
The system SHALL support updating an ADR's status and content.

#### Scenario: Accept a proposed ADR
- **WHEN** the sub-agent changes an ADR's status from `proposed` to `accepted`
- **THEN** the file SHALL be updated with the new status

