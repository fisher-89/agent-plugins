## ADDED Requirements

### Requirement: workflow-test-only skill
The system SHALL provide `dev-team:workflow-test-only` skill at `skills/workflow-test-only/SKILL.md` with name `workflow-test-only`.

The skill description SHALL indicate it orchestrates the test-only PGE workflow for supplementing test coverage on already-implemented code.

The skill SHALL follow the same change resolution logic (Step 0) and scaffold logic (Step 1) as `workflow-requirement`.

After scaffolding a new change (Step 1), the skill SHALL write `{"workflow_type": "test-only"}` to `openspec/changes/<name>/workflow.json`. This is the single source of truth — the skill SHALL NOT pass `workflow_type` to MCP tools.

The orchestration loop (Step 2) SHALL call:
```
mcp__plugin_dev-team_dev-team__phase_next(change=<name>)
```

When an evaluator returns after discovering production code bugs (eval entry with `verdict: "fail"`, `backtrack_to: null`, and bug details in report), the skill SHALL:
1. Write a bug report to `openspec/changes/<name>/reports/` (e.g. `code-bugs-found.md` or JSON)
2. Notify the user that tests discovered code bugs in the production code
3. Allow the user to decide whether to continue the workflow (e.g. proceed to integration-test) or terminate

The completion step (Step 3) SHALL report `total_phases: 6` and note that discovering implementation bugs via tests fulfills the test-only workflow purpose before manual archive.

The skill SHALL NOT auto-archive.

#### Scenario: workflow-test-only skill executes six-phase loop (AC-8)
- **WHEN** user invokes `/dev-team:workflow-test-only <change-name>`
- **THEN** the skill enters a loop calling `phase_next(change=<name>)` without `workflow_type`
- **AND** executes all phases returned by `phase_next` until `done: true`
- **AND** the skill file contains no hardcoded phase table or agent names

#### Scenario: workflow-test-only writes workflow.json on scaffold (AC-19)
- **WHEN** `workflow-test-only` creates a new change in Step 1
- **THEN** it writes `{"workflow_type": "test-only"}` to `workflow.json`
- **AND** subsequent `phase_next(change)` calls resolve the test-only phase table from that file

#### Scenario: workflow-test-only skill file does not pass workflow_type to MCP
- **WHEN** reading `skills/workflow-test-only/SKILL.md`
- **THEN** the frontmatter `name` is `workflow-test-only`
- **AND** the loop calls `phase_next(change=<name>)` without a `workflow_type` argument
- **AND** no `phase_log` call includes `workflow_type`

#### Scenario: workflow-test-only uses underscore MCP tool names
- **WHEN** reading `skills/workflow-test-only/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__phase_next` (not `phase/next`)

#### Scenario: workflow-test-only completion does not reference acceptance
- **WHEN** the skill reaches Step 3 (Completion)
- **THEN** the completion message does NOT mention phase `09-acceptance`
- **AND** instructs user to run `/dev-team:openspec-archive-change` manually

#### Scenario: workflow-test-only inherits explore context like workflow-requirement
- **WHEN** user invokes `/dev-team:workflow-test-only` without argument after an explore session
- **THEN** Step 0 detects explore context and derives a kebab-case change name
- **AND** explore context is available for the proposal phase via phase_next prompt handling

#### Scenario: workflow generates bug report when tests find code bugs (AC-15)
- **WHEN** the evaluator returns after logging `verdict: "fail"`, `backtrack_to: null` with code bug details in report
- **THEN** the skill writes a bug report under `openspec/changes/<name>/reports/`
- **AND** notifies the user that tests discovered production code bugs
- **AND** prompts the user to continue (e.g. to integration-test) or terminate the workflow

#### Scenario: user may continue workflow after unit-test bug discovery
- **WHEN** unit-test phase discovers code bugs and the user chooses to continue
- **THEN** the skill resumes the phase_next loop
- **AND** may proceed to `08-integration-test` per prerequisite DAG

### Requirement: workflow-requirement skill writes workflow.json on scaffold
`plugins/dev-team/skills/workflow-requirement/SKILL.md` SHALL be updated to write `{"workflow_type": "requirement"}` to `workflow.json` when scaffolding a new change, and its orchestration loop SHALL call `phase_next(change=<name>)` without a `workflow_type` parameter.

#### Scenario: workflow-requirement no longer passes workflow_type to phase_next
- **WHEN** reading `skills/workflow-requirement/SKILL.md`
- **THEN** the loop calls `phase_next(change=<name>)` without `workflow_type`
- **AND** Step 1 writes `{"workflow_type": "requirement"}` to `workflow.json` for new changes

## Module Contract

### Skill Files (`plugins/dev-team/skills/`)

| Skill | workflow_type (in workflow.json) | MCP phase_next call | Phase Count | Contract |
|-------|----------------------------------|---------------------|-------------|----------|
| workflow-requirement | `requirement` (written on scaffold) | `phase_next(change=<name>)` | 9 | Full dev+test pipeline |
| workflow-test-only | `test-only` (written on scaffold) | `phase_next(change=<name>)` | 6 | Test-only pipeline; bug report on code bug discovery; no implement/review/acceptance |

### Change Directory

| File | Written by | Field |
|------|-----------|-------|
| `workflow.json` | workflow-* skills (Step 1) or proposal-planner (confirmation) | `workflow_type` |

### plugin.json

| Field | Change | Purpose |
|-------|--------|---------|
| `version` | MODIFIED | Bump after adding workflow-test-only skill |
