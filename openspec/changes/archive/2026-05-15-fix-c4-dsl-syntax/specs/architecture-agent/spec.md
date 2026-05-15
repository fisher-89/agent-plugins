## MODIFIED Requirements

### Requirement: Propose mode

The architecture agent SHALL support a propose mode that reads current model files and relevant code, drafts DSL changes using correct LikeC4 DSL syntax (`element <name>` in specification, `metadata { }` blocks for metadata), validates them via `archi-model.py validate --source`, and presents the diff to the user for confirmation.

#### Scenario: Propose a new component

- **WHEN** the user asks to add a component for `src/services/payment/`
- **THEN** the agent SHALL read existing `models/*.c4`, grep for payment-related code, draft a DSL snippet with `metadata { path ['./src/services/payment/'] }` syntax, validate it, and present the proposal with a diff

#### Scenario: Proposal validation fails on syntax

- **WHEN** the drafted DSL snippet uses incorrect specification or metadata syntax
- **THEN** the agent SHALL report the validation error and fix the proposal before presenting

#### Scenario: Agent does not auto-write

- **WHEN** the agent generates a valid proposal
- **THEN** the agent SHALL NOT write the model file without explicit user confirmation
