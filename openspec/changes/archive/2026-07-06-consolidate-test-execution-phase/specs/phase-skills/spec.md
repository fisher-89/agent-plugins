## MODIFIED Requirements

### Requirement: phase-unit-test skill renamed to phase-test-execution

The system SHALL provide `dev-team:phase-test-execution` skill at `skills/phase-test-execution/SKILL.md` with name `phase-test-execution`. The `phase-unit-test` skill at `skills/phase-unit-test/` SHALL be removed (deleted or renamed).

The skill SHALL follow the EXEC P→E pattern:
- **Executor**: `test-execution-executor` sub-agent runs tests via CLI and produces execution report
- **Evaluator**: `test-execution-evaluator` sub-agent evaluates against checklist, appends to eval.json via MCP phase_log

Gate check phase ID: `test-execution`.

#### Scenario: phase-test-execution executes EXEC loop
- **WHEN** user invokes `/dev-team:phase-test-execution <change-name>`
- **THEN** the skill runs gate check `mcp__plugin_dev-team_dev-team__phase_check` with phase `test-execution`
- **AND** invokes `test-execution-executor` sub-agent
- **AND** after executor completes, invokes `test-execution-evaluator` sub-agent
- **AND** reads eval.json to determine verdict

#### Scenario: phase-test-execution skill file exists at new path
- **WHEN** reading `skills/phase-test-execution/SKILL.md`
- **THEN** the `name` frontmatter is `phase-test-execution`
- **AND** the gate check references `test-execution` (not `unit-test` or `06-unit-test`)

### Requirement: phase-integration-test skill removed

The `dev-team:phase-integration-test` skill at `skills/phase-integration-test/` SHALL be deleted entirely. Integration test execution is now handled by the consolidated `test-execution` phase.

#### Scenario: phase-integration-test directory removed
- **WHEN** checking `skills/phase-integration-test/SKILL.md`
- **THEN** the file SHALL NOT exist (the directory is deleted)

### Requirement: phase skills use result.next_phase for verdict reading (no change)

All individual phase SKILL.md files SHALL read eval.json verdict entries using the phase ID returned by `mcp__plugin_dev-team_dev-team__phase_next` (i.e., `result.next_phase`). This behavior is unchanged for unaffected skills. The test-execution skill follows the same pattern.

#### Scenario: phase-test-execution reads verdict from result.next_phase
- **WHEN** reading `skills/phase-test-execution/SKILL.md`
- **THEN** the verdict reading step references `result.next_phase`

### Requirement: Seven user-triggered phase skills (was nine)

The system SHALL provide 7 phase skills (down from 9). Updated skill list:

| # | Skill | MCP Tools Used | Agent (Planner) | Agent (Evaluator) |
|---|-------|----------------|-----------------|-------------------|
| 1 | dev-team:phase-proposal | phase_check | proposal-planner | proposal-evaluator |
| 2 | dev-team:phase-dev-design | phase_check | dev-design-planner | dev-design-evaluator |
| 3 | dev-team:phase-test-design | phase_check | test-design-planner | test-design-evaluator |
| 4 | dev-team:phase-test-gen | phase_check | test-gen-generator | test-gen-evaluator |
| 5 | dev-team:phase-implement | phase_check | implementation-generator | implementation-evaluator |
| 6 | dev-team:phase-test-execution | phase_log | test-execution-executor | test-execution-evaluator |
| 7 | dev-team:phase-code-review | phase_check | (none) | code-review-evaluator |
| 8 | dev-team:phase-acceptance | phase_check | (none) | acceptance-evaluator |

Removed skills:
- `dev-team:phase-unit-test` — replaced by `phase-test-execution`
- `dev-team:phase-integration-test` — removed (merged into `phase-test-execution`)

#### Scenario: phase-test-execution is an EXEC loop
- **WHEN** user invokes `dev-team:phase-test-execution`
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__phase_check` with `change` and `phase="test-execution"`
- **AND** invokes test-execution-executor sub-agent with one-line prompt
- **AND** then invokes test-execution-evaluator sub-agent
- **AND** the skill reads the latest eval.json entry to determine verdict
- **AND** loops back to Executor if verdict is "fail" (up to max attempts)

## REMOVED Requirements

### Requirement: Nine user-triggered phase skills (tool name update)
**Reason**: Reduced from 9 to 7 skills. `phase-unit-test` renamed to `phase-test-execution`; `phase-integration-test` deleted.

**Migration**: Skills list updated to 7 entries. Workflow skills that referenced these phases will dynamically resolve them via phase_next.

### Requirement: phase-unit-test (ID update references)
**Reason**: Skill file moved from `skills/phase-unit-test/` to `skills/phase-test-execution/`.

**Migration**: MCP tools, gate check phase, and evaluator references updated from `unit-test` to `test-execution`.

### Requirement: phase-integration-test (ID update references)
**Reason**: Skill file deleted entirely.

**Migration**: Integration test execution is now handled by `phase-test-execution`.

### Requirement: openspec-archive-change skill (no change needed)
**Reason**: The archive skill checks for `acceptance` phase pass, which is unaffected by this change.

## Module Contract

### Skill Files (`plugins/dev-team/skills/`)

| Skill | MCP Tools Used | Agent (Planner) | Agent (Evaluator) | Contract |
|-------|----------------|-----------------|-------------------|----------|
| phase-proposal | phase_check | proposal-planner | proposal-evaluator | P→E loop, phase `proposal` |
| phase-dev-design | phase_check | dev-design-planner | dev-design-evaluator | P→E loop, phase `dev-design` |
| phase-test-design | phase_check | test-design-planner | test-design-evaluator | P→E loop, phase `test-design` |
| phase-test-gen | phase_check | test-gen-generator | test-gen-evaluator | G→E loop, phase `test-gen` |
| phase-implement | phase_check | implementation-generator | implementation-evaluator | G→E + AUTO, phase `implement` |
| phase-test-execution | phase_log | test-execution-executor | test-execution-evaluator | EXEC loop, phase `test-execution` |
| phase-code-review | phase_check | (none) | code-review-evaluator | EVAL-ONLY, phase `code-review` |
| phase-acceptance | phase_check | (none) | acceptance-evaluator | EVAL-ONLY, phase `acceptance` |

### Skill Changes

| Skill | Change | Details |
|-------|--------|---------|
| phase-unit-test | RENAMED → `phase-test-execution` | Skill directory relocated: `skills/phase-unit-test/` → `skills/phase-test-execution/`. Phase ID updated to `test-execution`. Agents updated. |
| phase-integration-test | DELETED | Entire directory at `skills/phase-integration-test/` removed. Functionality merged into `phase-test-execution`. |
| workflow-requirement | MODIFIED | References to `integration-test` in progress output removed. |
| workflow-test-only | MODIFIED | Phase table references updated — test-only has 5 phases (removed integration-test). |
