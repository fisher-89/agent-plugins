## ADDED Requirements

### Requirement: Skill gate intercepts Write/Edit when pending skill route exists
The system SHALL intercept `Write` and `Edit` tool calls via PreToolUse hook when a pending skill route state file exists (`pending-skill.json`). The hook SHALL deny the tool call and instruct Claude to invoke the required skill first.

#### Scenario: Write tool blocked by pending skill route
- **WHEN** a pending skill route state file exists and Claude attempts to use the Write tool
- **THEN** the PreToolUse hook SHALL return `deny` with an additionalContext message instructing Claude to invoke the required skill

#### Scenario: Edit tool blocked by pending skill route
- **WHEN** a pending skill route state file exists and Claude attempts to use the Edit tool
- **THEN** the PreToolUse hook SHALL return `deny` with an additionalContext message instructing Claude to invoke the required skill

### Requirement: Skill gate allows Write/Edit when no pending route exists
The system SHALL allow `Write` and `Edit` tool calls when no pending skill route state file exists.

#### Scenario: No pending state file
- **WHEN** no pending skill route state file exists and Claude attempts to use the Write or Edit tool
- **THEN** the PreToolUse hook SHALL return `allow`

### Requirement: Skill gate auto-mode deny message format
When the pending route has `mode: auto`, the deny message SHALL include the exact skill name to invoke and the reason, using imperative language.

#### Scenario: Auto-mode deny message
- **WHEN** the pending skill route has `mode: auto` and Claude attempts a Write/Edit
- **THEN** the deny message SHALL contain the skill invocation instruction in the format: `Invoke the Skill tool with skill="<skill-name>" before writing any code.`
- **AND** the message SHALL include the reason from the routing decision

### Requirement: Skill gate suggest-mode deny message format
When the pending route has `mode: suggest`, the deny message SHALL present options including invoking the recommended skill and an option to skip the workflow.

#### Scenario: Suggest-mode deny message
- **WHEN** the pending skill route has `mode: suggest` and Claude attempts a Write/Edit
- **THEN** the deny message SHALL list the recommended skill as option 1
- **AND** the message SHALL include an option to skip the workflow (e.g., "implement directly")
- **AND** the message SHALL include the intent type, confidence, and scope

### Requirement: Skill gate expires stale pending states
The system SHALL treat a pending skill route state as expired if its timestamp is older than 5 minutes. Expired states SHALL be cleared and the tool call SHALL be allowed.

#### Scenario: Expired pending state
- **WHEN** a pending skill route state file exists but its timestamp is older than 5 minutes
- **THEN** the PreToolUse hook SHALL delete the state file
- **AND** the hook SHALL return `allow`

#### Scenario: Fresh pending state
- **WHEN** a pending skill route state file exists and its timestamp is within 5 minutes
- **THEN** the PreToolUse hook SHALL enforce the gate (deny or allow based on mode)

### Requirement: Skill gate does not intercept read-only tools
The system SHALL NOT intercept Read, Grep, Glob, or other read-only tool calls, even when a pending skill route exists.

#### Scenario: Read tool not intercepted
- **WHEN** a pending skill route state file exists and Claude attempts to use the Read tool
- **THEN** the hook SHALL NOT be triggered (matcher is `Write|Edit` only)

### Requirement: Skill gate clears state when user chooses to skip workflow
When the user explicitly indicates they want to skip the workflow (e.g., typing "implement directly" or "skip workflow"), the UserPromptSubmit hook SHALL clear the pending state file before outputting context.

#### Scenario: User skips workflow
- **WHEN** a pending skill route state file exists and the user's new prompt contains skip keywords (e.g., "implement directly", "skip", "直接实现")
- **THEN** the UserPromptSubmit hook SHALL clear the pending state file
- **AND** the hook SHALL output context without routing suggestions
