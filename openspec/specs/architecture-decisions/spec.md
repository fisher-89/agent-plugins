# architecture-decisions Specification

## Purpose
TBD - created by archiving change add-architect-role. Update Purpose after archive.
## Requirements
### Requirement: Create ADR record

The system SHALL create a new Architecture Decision Record in `openspec/architecture/decisions/` with the filename format `YYYY-MM-DD-<kebab-title>.md`.
Creation SHALL be performed through MCP `archi_decide` with `action` `create` (not via `archi-decide.py`).

#### Scenario: Create new ADR

- **WHEN** an ADR is created with title "Use PostgreSQL for Primary Store" on 2026-05-12
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

The system SHALL support listing all ADRs and filtering by status through MCP `archi_decide` with `action` `list`.

#### Scenario: List all ADRs

- **WHEN** ADRs are queried without a status filter
- **THEN** the system SHALL return ADRs sorted by date descending

#### Scenario: Filter by status

- **WHEN** ADRs are queried with status `accepted`
- **THEN** the system SHALL return only ADRs with status `accepted`

### Requirement: Update ADR status

The system SHALL support updating an ADR's status and content through MCP `archi_decide` with `action` `update`.

#### Scenario: Accept a proposed ADR

- **WHEN** an ADR's status is changed from `proposed` to `accepted`
- **THEN** the file SHALL be updated with the new status

### Requirement: ADR operations are delivered via MCP archi_decide

The system SHALL expose ADR create, list, and update through a single MCP tool named `archi_decide`.
The tool MUST accept a required `action` field with value `create`, `list`, or `update`.
The tool MUST accept a required `project_root` string and resolve it via the shared MCP project-root candidates/force path used by other `archi_*` tools.
The tool MUST NOT be registered as a CLI subcommand on `cli.ts`.
After this change, `plugins/dev-team/utils/archi-decide.py` MUST NOT exist in plugin source, and built products MUST NOT ship or list that script.

#### Scenario: tools/list includes archi_decide

- **WHEN** a client calls MCP `tools/list`
- **THEN** the response SHALL include a tool whose `name` is `archi_decide`
- **AND** the name MUST use underscore form (MUST NOT be `archi/decide`)

#### Scenario: create via MCP writes ADR file

- **WHEN** the client calls `archi_decide` with `action` `create`, a valid `project_root`, and required create fields (`title`, `background`, `decision`)
- **THEN** the system SHALL create `openspec/architecture/decisions/YYYY-MM-DD-<kebab-title>.md` under that project root
- **AND** the response SHALL indicate success and include the created filename or path

#### Scenario: create defaults status to proposed

- **WHEN** the client calls `archi_decide` with `action` `create` and omits `status`
- **THEN** the created ADR status SHALL be `proposed`

#### Scenario: list via MCP returns sorted ADRs

- **WHEN** the client calls `archi_decide` with `action` `list` and a valid `project_root`
- **THEN** the system SHALL return ADR entries sorted by date descending
- **AND** when `status` filter is provided, only matching ADRs SHALL be returned

#### Scenario: update via MCP changes status

- **WHEN** the client calls `archi_decide` with `action` `update`, `file` naming an existing ADR, and `status` `accepted`
- **THEN** the ADR file SHALL be updated so its status field is `accepted`

#### Scenario: superseded requires superseded_by

- **WHEN** the client calls `archi_decide` with `action` `update` and `status` `superseded` without `superseded_by`
- **THEN** the tool SHALL return a structured failure
- **AND** the ADR file MUST NOT be updated to `superseded`

#### Scenario: Python script is removed

- **WHEN** the plugin source tree and built products are inspected after the change
- **THEN** `utils/archi-decide.py` MUST NOT be present
- **AND** home-image / plugin manifests MUST NOT list `utils/archi-decide.py`

#### Scenario: No CLI archi-decide command

- **WHEN** the `dev-team` CLI command surface is inspected
- **THEN** it MUST NOT expose an `archi-decide` or `archi decide` subcommand for ADR management

### Requirement: Create renders from adr template

When `action` is `create`, the system SHALL render the ADR markdown from `templates/adr.md` (plugin template), filling title, date, status, background, decision, consequences, alternatives, and scope placeholders.
The rendered document MUST still satisfy the existing field requirements (title, date, status, background, decision, consequences, alternatives, affected scope section `## 影响范围`).

#### Scenario: Created ADR matches template sections

- **WHEN** an ADR is created via `archi_decide` `create`
- **THEN** the file SHALL include the section headings defined by `templates/adr.md` (`## 背景`, `## 决策`, `## 后果`, `## 备选方案`, `## 影响范围`)
- **AND** the status line SHALL use one of `proposed`, `accepted`, `deprecated`, `superseded`

#### Scenario: Missing template fails clearly

- **WHEN** `action` is `create` and the ADR template file cannot be resolved
- **THEN** the tool SHALL return a structured failure with an error message
- **AND** it MUST NOT write a partial ADR file

