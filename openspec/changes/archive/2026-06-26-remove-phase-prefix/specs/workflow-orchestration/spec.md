## MODIFIED Requirements

### Requirement: workflow-requirement skill (phase ID output update)
The system SHALL provide `dev-team:workflow-requirement` skill at `skills/workflow-requirement/SKILL.md` with name `workflow-requirement`.

The skill SHALL be a thin orchestration loop with NO hardcoded phase knowledge. All phase sequencing, agent assignment, and prompt generation SHALL be delegated to `mcp__plugin_dev-team_dev-team__phase_next`.

The skill SHALL use `result.next_phase` for all phase identity references in output messages (progress, notifications). Phase IDs returned by `phase_next` no longer contain numeric prefixes — output messages SHALL display them as-is (e.g., "proposal", "implement", "acceptance").

#### Scenario: workflow-requirement outputs prefix-free phase IDs
- **WHEN** the workflow executes phase `proposal`
- **THEN** the progress output shows `Phase proposal` (not `Phase 01-proposal`)

#### Scenario: workflow-requirement PushNotification uses prefix-free phase ID
- **WHEN** the workflow completes a phase and sends PushNotification
- **THEN** the notification includes the prefix-free phase ID from `result.next_phase`

### Requirement: workflow-test-only skill (phase ID output update)
The system SHALL provide `dev-team:workflow-test-only` skill at `skills/workflow-test-only/SKILL.md` with name `workflow-test-only`.

The skill SHALL use `result.next_phase` for all phase identity references. Phase IDs returned by `phase_next` no longer contain numeric prefixes.

The completion step SHALL report using prefix-free phase IDs.

#### Scenario: workflow-test-only outputs prefix-free phase IDs
- **WHEN** the workflow executes phase `test-gen`
- **THEN** the progress output shows `Phase test-gen` (not `Phase 04-test-gen`)

## Module Contract

### Workflow Skills (`plugins/dev-team/skills/`)

| Skill | Change | Details |
|-------|--------|---------|
| workflow-requirement | MODIFIED | Output messages and notifications use `result.next_phase` as-is (prefix-free) |
| workflow-test-only | MODIFIED | Output messages and notifications use `result.next_phase` as-is (prefix-free) |
