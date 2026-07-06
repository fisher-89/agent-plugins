## MODIFIED Requirements

### Requirement: Workflow orchestration layer (updated phase count)

The system SHALL support a workflow orchestration layer above the phase level. Workflow skills (`workflow-requirement`, `workflow-test-only`) SHALL invoke phases directly via Agent + MCP + Bash calls.

The requirement workflow now has 8 phases (down from 9, `integration-test` removed). The test-only workflow now has 5 phases (down from 6, `integration-test` removed).

#### Scenario: Workflow layer vs phase layer separation (no change)

- **WHEN** a workflow skill executes Phase `proposal`
- **THEN** it directly calls Agent(`proposal-planner`) and Agent(`proposal-evaluator`) with MCP phase/next and phase/log
- **AND** it does NOT invoke Skill(`phase-proposal`)
- **AND** the phase-level skill remains independently invocable

#### Scenario: Workflow-requirement runs 8 phases max

- **WHEN** user invokes `/dev-team:workflow-requirement <change-name>`
- **AND** all phases complete successfully
- **THEN** the workflow iterates through at most 8 phases: proposal, dev-design, test-design, implement, test-gen, test-execution, code-review, acceptance
- **AND** `phase_next` returns `total_phases: 8` (was 9)

### Requirement: workflow-requirement skill progress output updated

The progress output `[Phase N/9]` SHALL be updated to `[Phase N/8]` for requirement workflow, reflecting the removal of `integration-test`. This is automatically handled by `phase_next` returning `total_phases` from the phase table.

#### Scenario: Progress output shows correct phase count

**WHEN** `phase_next` returns `total_phases: 8` for the requirement workflow
**THEN** the workflow skill outputs `[Phase {phase_index}/8]` (not `[Phase {phase_index}/9]`)

### Requirement: workflow-test-only skill progress output updated

The test-only workflow now has 5 phases (proposal, code-analyze, test-design, test-gen, test-execution). The `total_phases` returned by `phase_next` SHALL be 5.

#### Scenario: Workflow-test-only shows total_phases: 5

**WHEN** user invokes `/dev-team:workflow-test-only <change-name>`
**AND** all phases complete
**THEN** the completion message references 5 phases (was 6)

## REMOVED Requirements

### Requirement: workflow-test-only completion mentions integration-test
**Reason**: Integration-test phase removed from test-only workflow.

**Migration**: Completion message updated to reflect 5 phases.

## Module Contract

### Workflow Skills (`plugins/dev-team/skills/`)

| Workflow | workflow_type | Phase Table | Phase Count |
|----------|--------------|-------------|-------------|
| workflow-requirement | `requirement` | 8 phases: proposal through acceptance | 8 (was 9) |
| workflow-test-only | `test-only` | 5 phases: proposal through test-execution | 5 (was 6) |

### Phase Tables (updated)

| workflow_type | Phases (Before) | Phases (After) |
|---------------|----------------|----------------|
| `requirement` | 9 phases (proposal...acceptance, incl. unit-test + integration-test) | 8 phases (proposal...acceptance, test-execution only) |
| `test-only` | 6 phases (proposal...integration-test) | 5 phases (proposal...test-execution, no integration-test) |
