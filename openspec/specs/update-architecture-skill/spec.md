## ADDED Requirements

### Requirement: Skill routes to architecture subagent

The system SHALL provide an `update-architecture` skill at `plugins/dev-team/skills/update-architecture/SKILL.md` that routes user requests to the `architecture` subagent via the `Agent` tool.

#### Scenario: Skill invokes agent

- **WHEN** the user types `/dev-team:update-architecture`
- **THEN** the skill SHALL invoke `Agent({subagent_type: "architecture", ...})` with a prompt tailored from the user's request

#### Scenario: Skill passes user intent

- **WHEN** the user invokes the skill with specific instructions
- **THEN** the agent prompt SHALL include the user's full request and relevant context

### Requirement: Skill is user-invocable only

The skill SHALL set `disable-model-invocation: true` in its frontmatter so that the model never suggests or invokes it proactively.

#### Scenario: Model does not suggest the skill

- **WHEN** architecture-related tasks arise during normal conversation
- **THEN** the model SHALL NOT suggest using `/dev-team:update-architecture` unless the user explicitly asks for architecture updates

#### Scenario: User can still invoke

- **WHEN** the user types `/dev-team:update-architecture` or asks to update architecture
- **THEN** the skill SHALL be invoked normally
