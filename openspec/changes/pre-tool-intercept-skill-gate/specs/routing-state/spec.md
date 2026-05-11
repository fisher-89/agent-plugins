## ADDED Requirements

### Requirement: UserPromptSubmit writes routing state on non-direct decisions
When the Router produces a non-direct routing decision (action is `explore`, `apply-change`, or `archive`), the UserPromptSubmit hook SHALL write a pending skill route state file at `.claude/pending-skill.json` in the current working directory.

#### Scenario: Auto-mode routing decision
- **WHEN** the Router produces a decision with `mode: auto` and `action: explore`
- **THEN** the hook SHALL write a state file containing the decision, intent, timestamp, and version
- **AND** the hook SHALL also output the `additionalContext` as before

#### Scenario: Suggest-mode routing decision
- **WHEN** the Router produces a decision with `mode: suggest` and `action: explore`
- **THEN** the hook SHALL write a state file containing the decision, intent, timestamp, and version
- **AND** the hook SHALL also output the `additionalContext` as before

#### Scenario: Direct-mode routing decision
- **WHEN** the Router produces a decision with `mode: direct`
- **THEN** the hook SHALL NOT write a state file
- **AND** the hook SHALL output only the existing active change context

### Requirement: State file format
The pending skill route state file SHALL be a JSON file with the following structure:

```json
{
  "version": 1,
  "timestamp": "<ISO 8601 UTC timestamp>",
  "decision": {
    "action": "<explore|apply-change|archive>",
    "mode": "<auto|suggest>",
    "reason": "<human-readable reason>"
  },
  "intent": {
    "type": "<explore|implement|fix|archive|question>",
    "confidence": "<high|medium|low>",
    "scope": "<trivial|small|medium|large>"
  },
  "options": ["<option 1>", "<option 2>"]
}
```

#### Scenario: State file contains all required fields
- **WHEN** a state file is written
- **THEN** it SHALL contain `version` (integer), `timestamp` (ISO 8601), `decision` (object with action, mode, reason), `intent` (object with type, confidence, scope), and `options` (array of strings)

### Requirement: PreToolUse Skill hook clears state on matching skill invocation
When Claude invokes a Skill tool that matches the pending route's action (e.g., `dev-team:openspec-explore` for action `explore`), the PreToolUse Skill hook SHALL clear the pending state file.

#### Scenario: Matching skill invoked
- **WHEN** a pending state file exists with `action: explore` and Claude invokes the Skill tool with `skill: "dev-team:openspec-explore"`
- **THEN** the PreToolUse Skill hook SHALL delete the pending state file
- **AND** the hook SHALL allow the Skill invocation

#### Scenario: Non-matching skill invoked
- **WHEN** a pending state file exists with `action: explore` and Claude invokes a different skill (e.g., `dev-team:code-review`)
- **THEN** the PreToolUse Skill hook SHALL NOT delete the pending state file
- **AND** the hook SHALL proceed with its existing logic

### Requirement: State file location
The pending skill route state file SHALL be located at `.claude/pending-skill.json` relative to the current working directory (the project root).

#### Scenario: State file in project root
- **WHEN** the hook writes a state file
- **THEN** it SHALL be written to `<cwd>/.claude/pending-skill.json`
- **AND** the `.claude/` directory SHALL be created if it does not exist

### Requirement: State file is per-project
Each project directory SHALL have its own independent pending state file. The state file SHALL NOT be shared across projects.

#### Scenario: Multiple projects
- **WHEN** the user works in project A and a state file is written
- **THEN** the state file at `<projectA>/.claude/pending-skill.json` SHALL NOT affect project B
