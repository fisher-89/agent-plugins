## MODIFIED Requirements

### Requirement: UserPromptSubmit hook provides intent-based routing
The hook SHALL analyze user prompts, detect intent and scope, check active change state, and produce a routing output that either auto-executes the appropriate skill or presents options for user selection.

#### Scenario: Auto-execute explore for high-confidence implement with no active change
- **WHEN** user submits an implement intent prompt with high confidence AND no active changes exist
- **THEN** hook outputs `Auto-Route: /openspec-explore` with reason, and Claude SHALL invoke the explore skill

#### Scenario: Auto-execute archive for completed change
- **WHEN** user submits an archive intent prompt with high confidence AND active changes have all tasks complete
- **THEN** hook outputs `Auto-Route: /openspec-archive` with reason, and Claude SHALL invoke the archive skill

#### Scenario: Suggest options for medium-confidence implement
- **WHEN** user submits an implement intent prompt with medium or low confidence AND no active changes exist
- **THEN** hook outputs suggestion with options: 1) `/openspec-explore` (recommended) 2) Direct answer

#### Scenario: Suggest apply-change when active change has tasks in progress
- **WHEN** user submits a development intent prompt AND active changes exist with tasks in progress
- **THEN** hook outputs suggestion with options: 1) `/openspec-apply-change` (continue) 2) Direct answer

#### Scenario: Direct answer for questions and explore intents
- **WHEN** user submits a question or explore intent prompt
- **THEN** hook outputs no routing suggestion, allowing Claude to answer directly

#### Scenario: Direct answer for trivial fixes
- **WHEN** user submits a fix intent prompt AND scope is trivial (single file, ≤2 functions, no public API change)
- **THEN** hook outputs no routing suggestion, allowing Claude to fix directly

#### Scenario: Active change context included in direct answers
- **WHEN** routing decision is `direct` AND active changes exist
- **THEN** hook includes active change information and instructs Claude to check if documents need updates

#### Scenario: Active change fully complete triggers auto-commit instructions
- **WHEN** active changes exist AND all tasks are complete AND no archive intent detected
- **THEN** hook includes auto-commit instructions (existing behavior preserved)

## ADDED Requirements

### Requirement: Hook output distinguishes auto-route and suggestion formats
The hook SHALL produce two distinct output formats depending on execution mode.

#### Scenario: Auto-route output format
- **WHEN** routing mode is `auto`
- **THEN** output contains `Auto-Route: /openspec-<skill>` line followed by reason explanation

#### Scenario: Suggestion output format
- **WHEN** routing mode is `suggest`
- **THEN** output contains detected intent, numbered options with skill commands, and instruction to reply with number or keyword

### Requirement: Existing active change reporting preserved
The hook SHALL preserve existing behavior for reporting active change names, artifacts, and task progress when changes are detected.

#### Scenario: Active changes with artifacts reported
- **WHEN** active changes exist with proposal.md, design.md, tasks.md, or specs
- **THEN** hook lists each change with artifact status and task completion counts

#### Scenario: Active change schema reported
- **WHEN** active change has .openspec.yaml with schema field
- **THEN** hook includes schema information in change report
