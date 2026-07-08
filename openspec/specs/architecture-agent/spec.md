## ADDED Requirements

### Requirement: Agent definition exists

The system SHALL provide an `architecture` subagent definition at `plugins/dev-team/agents/architecture.md` with model `opus`, tools `[Read, Bash, Write, Grep, Glob]`, and instructions covering four operational modes.

#### Scenario: Agent file is valid

- **WHEN** the plugin is loaded
- **THEN** `architecture` SHALL be registered as an available subagent type

#### Scenario: Agent has required tools

- **WHEN** the architecture agent is invoked
- **THEN** it SHALL have access to Read, Bash, Write, Grep, and Glob tools

### Requirement: Propose mode

The architecture agent SHALL support a propose mode that reads current model files and relevant code, drafts DSL changes using correct LikeC4 DSL syntax (`element <name>` in specification, `metadata { }` blocks for metadata), validates them via `dev-team archi validate` (MCP `archi_validate`), and presents the diff to the user for confirmation.

#### Scenario: Propose a new component

- **WHEN** the user asks to add a component for `src/services/payment/`
- **THEN** the agent SHALL read existing `models/*.c4`, grep for payment-related code, draft a DSL snippet with `metadata { path ['./src/services/payment/'] }` syntax, validate it, and present the proposal with a diff

#### Scenario: Proposal validation fails on syntax

- **WHEN** the drafted DSL snippet uses incorrect specification or metadata syntax
- **THEN** the agent SHALL report the validation error and fix the proposal before presenting

#### Scenario: Agent does not auto-write

- **WHEN** the agent generates a valid proposal
- **THEN** the agent SHALL NOT write the model file without explicit user confirmation

### Requirement: Validate mode

The architecture agent SHALL support a validate mode that runs `dev-team archi check` on staged or specified files, and explains violations in plain language with suggested fixes.

#### Scenario: Validate staged files

- **WHEN** the user asks to validate architecture
- **THEN** the agent SHALL run `dev-team archi check --staged` and interpret the results

#### Scenario: Explain violations

- **WHEN** `dev-team archi check` reports an `unmodeled_dependency` violation
- **THEN** the agent SHALL explain which elements are involved, which import triggered the violation, and suggest either adding a relationship or updating `metadata.path`

### Requirement: Decide mode

The architecture agent SHALL support a decide mode that helps draft ADRs by gathering context and writing via `archi-decide.py create`.

#### Scenario: Create an ADR

- **WHEN** the user asks to record an architectural decision
- **THEN** the agent SHALL gather background from conversation, confirm scope, and invoke `archi-decide.py create` with the appropriate arguments

### Requirement: Review mode

The architecture agent SHALL support a review mode that reads all `models/*.c4`, critiques the model for completeness, consistency, and coupling, and presents findings.

#### Scenario: Review model quality

- **WHEN** the user asks to review the architecture
- **THEN** the agent SHALL read all `models/*.c4` files, identify elements without relationships, elements without `metadata.path`, and orphaned relationships, and present a structured critique
