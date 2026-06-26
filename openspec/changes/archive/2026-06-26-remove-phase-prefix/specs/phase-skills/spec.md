## ADDED Requirements

### Requirement: Phase skills use result.next_phase for verdict reading
All individual phase SKILL.md files SHALL read eval.json verdict entries using the phase ID returned by `mcp__plugin_dev-team_dev-team__phase_next` (i.e., `result.next_phase`) rather than hardcoded phase ID strings. This ensures the skill remains correct regardless of phase ID format.

The verdict reading step in each phase skill SHALL follow this pattern:
```
Read latest phase "{result.next_phase}" entry from eval.json
```

#### Scenario: phase-proposal reads verdict from result.next_phase
- **WHEN** reading `skills/phase-proposal/SKILL.md`
- **THEN** the verdict reading step references `result.next_phase` instead of `"01-proposal"`

## MODIFIED Requirements

### Requirement: phase-proposal skill (ID update)
The system SHALL provide `dev-team:phase-proposal` skill at `skills/phase-proposal/SKILL.md` with name `phase-proposal`.

The skill SHALL follow the DESIGN P-E pattern:
- **Planner**: `proposal-planner` sub-agent writes proposal.md + specs/ to disk
- **Evaluator**: `proposal-evaluator` sub-agent evaluates against checklist, appends to eval.json via MCP phase_log

Gate check: `mcp__plugin_dev-team_dev-team__phase_check` with phase `proposal`.

The skill SHALL support the same input modes as previously defined.

#### Scenario: phase-proposal gate check uses prefix-free phase ID
- **WHEN** user invokes `/dev-team:phase-proposal <change-name>`
- **THEN** the skill runs gate check `mcp__plugin_dev-team_dev-team__phase_check` with phase `proposal`
- **AND** invokes `proposal-planner` sub-agent with one-line prompt
- **AND** reads eval.json for verdict using the phase ID from `result.next_phase`

### Requirement: Nine phase skills (ID update)
All nine phase skill SKILL.md files SHALL update their hardcoded phase ID references as follows:

| Skill | Old Gate/Phase ID | New Gate/Phase ID |
|-------|-------------------|-------------------|
| phase-proposal | `01-proposal` | `proposal` |
| phase-dev-design | `02-dev-design` | `dev-design` |
| phase-test-design | `03-test-design` | `test-design` |
| phase-test-gen | `04-test-gen` | `test-gen` |
| phase-implement | `05-implement` | `implement` |
| phase-unit-test | `06-unit-test` | `unit-test` |
| phase-code-review | `07-code-review` | `code-review` |
| phase-integration-test | `08-integration-test` | `integration-test` |
| phase-acceptance | `09-acceptance` | `acceptance` |

This includes:
1. Gate check assertion (`result.next_phase` comparison)
2. Verdict reading from eval.json (phase name in "Read latest phase ... entry")
3. No-op skip `phase_log` call phase parameter (unit-test, integration-test)
4. Backtrack suggestion phase references (acceptance mentions `proposal`, code-review mentions `dev-design`)

#### Scenario: phase-implement gate check uses prefix-free phase ID
- **WHEN** reading `skills/phase-implement/SKILL.md`
- **THEN** gate check asserts `result.next_phase` is `"implement"` (not `"05-implement"`)

#### Scenario: phase-unit-test no-op skip uses prefix-free phase ID
- **WHEN** reading `skills/phase-unit-test/SKILL.md`
- **THEN** the no-op skip `phase_log` call uses `phase: "unit-test"` (not `"06-unit-test"`)

#### Scenario: phase-acceptance backtrack suggestion references prefix-free ID
- **WHEN** reading `skills/phase-acceptance/SKILL.md`
- **THEN** backtrack suggestion references phase `proposal` (not `"01-proposal"`)

#### Scenario: phase-code-review backtrack suggestion references prefix-free ID
- **WHEN** reading `skills/phase-code-review/SKILL.md`
- **THEN** backtrack suggestion references phase `dev-design` (not `"02-dev-design"`)

#### Scenario: phase-dev-design backtrack check uses prefix-free ID
- **WHEN** reading `skills/phase-dev-design/SKILL.md`
- **THEN** backtrack detection checks for `backtrack_to` = `"dev-design"` (not `"02-dev-design"`)

### Requirement: openspec-archive-change skill (ID update)
`plugins/dev-team/skills/openspec-archive-change/SKILL.md` SHALL update its phase reference from `"09-acceptance"` to `"acceptance"` when calling `mcp__plugin_dev-team_dev-team__phase_check` to validate the PGE eval chain.

#### Scenario: openspec-archive-change uses prefix-free phase ID
- **WHEN** reading `skills/openspec-archive-change/SKILL.md`
- **THEN** the `phase_check` call uses phase `"acceptance"` (not `"09-acceptance"`)

## Module Contract

### Skill Files (`plugins/dev-team/skills/`)

| Skill | Change | Details |
|-------|--------|---------|
| phase-proposal | MODIFIED | `"01-proposal"` → `"proposal"` in gate check and verdict read |
| phase-dev-design | MODIFIED | `"02-dev-design"` → `"dev-design"` in gate check, backtrack, verdict |
| phase-test-design | MODIFIED | `"03-test-design"` → `"test-design"` in gate check and verdict |
| phase-test-gen | MODIFIED | `"04-test-gen"` → `"test-gen"` in gate check and verdict |
| phase-implement | MODIFIED | `"05-implement"` → `"implement"` in gate check and verdict |
| phase-unit-test | MODIFIED | `"06-unit-test"` → `"unit-test"` in gate check, verdict, no-op skip |
| phase-code-review | MODIFIED | `"07-code-review"` → `"code-review"` in gate check, verdict, backtrack |
| phase-integration-test | MODIFIED | `"08-integration-test"` → `"integration-test"` in gate check, verdict, no-op skip |
| phase-acceptance | MODIFIED | `"09-acceptance"` → `"acceptance"` in gate and verdict; backtrack target `"01-proposal"` → `"proposal"` |
| openspec-archive-change | MODIFIED | `"09-acceptance"` → `"acceptance"` in phase_check call |
