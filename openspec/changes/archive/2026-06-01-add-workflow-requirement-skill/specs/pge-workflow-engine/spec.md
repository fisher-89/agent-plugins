## ADDED Requirements

### Requirement: Workflow orchestration layer
The system SHALL support a workflow orchestration layer above the phase level. Workflow skills (`workflow-requirement`, and future `workflow-bug-fix`, `workflow-refactor`) SHALL invoke phases directly via Agent + MCP + Bash calls rather than through the Skill tool.

Each workflow skill SHALL:
1. Assemble context (change name, explore context if available)
2. Execute phases sequentially per a defined phase table
3. For each phase: gate check → skip if passed → execute → evaluate → verdict → next/stop
4. Stop on first phase failure (verdict=fail after max retries) or backtrack

#### Scenario: Workflow layer vs phase layer separation
- **WHEN** a workflow skill executes Phase 01
- **THEN** it directly calls Agent(`proposal-planner`) and Agent(`proposal-evaluator`) with MCP eval/check and eval/log
- **AND** it does NOT invoke Skill(`phase-proposal`)
- **AND** the phase-level skill (`phase-proposal`) remains independently invocable for single-phase execution

#### Scenario: Future workflow variants follow same pattern
- **WHEN** a future `workflow-bug-fix` or `workflow-refactor` skill is created
- **THEN** it SHALL use the same orchestration pattern (context assembly → phase table → sequential execution → stop, user manually archives)
- **AND** it MAY customize the phase table (skipping some phases) and context assembly (different explore detection logic)

### Requirement: eval.schema.json supports workflow phases
The `eval.schema.json` SHALL be updated to include `01-proposal` in the phase identifier enumeration.

Updated phase identifiers: `01-proposal`, `02-dev-design`, `03-test-design`, `04-test-gen`, `05-implement`, `06-unit-test`, `07-code-review`, `08-integration-test`, `09-acceptance`.

Existing eval.json files using `01-requirements` SHALL NOT be migrated — backward compatibility is maintained by treating the old identifier as a valid but deprecated phase string.

#### Scenario: eval/log accepts 01-proposal phase
- **WHEN** MCP eval/log is called with phase `01-proposal`
- **THEN** the call succeeds (no validation error)
- **AND** the entry is appended to eval.json with phase `01-proposal`

## MODIFIED Requirements

### Requirement: Nine-phase workflow structure
The system SHALL support 9 sequential phases with the following identifiers:

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| 01-proposal | 01-proposal | DESIGN Planner->Evaluator | Proposal and requirements (replaces 01-requirements) |
| 02-dev-design | 02-dev-design | DESIGN Planner->Evaluator | Implementation design |
| 03-test-design | 03-test-design | DESIGN Planner->Evaluator | Test scenario design |
| 04-test-gen | 04-test-gen | EXEC Generator->Evaluator | Test code generation |
| 05-implement | 05-implement | EXEC Generator->Evaluator + AUTO static-check | Implementation code generation |
| 06-unit-test | 06-unit-test | EXEC Executor->Evaluator (sonnet) | Unit test execution and report validation |
| 07-code-review | 07-code-review | EVAL-ONLY Evaluator | Code review evaluation |
| 08-integration-test | 08-integration-test | EXEC Executor->Evaluator (sonnet) | Integration test execution and report validation |
| 09-acceptance | 09-acceptance | EVAL-ONLY Evaluator | Acceptance evaluation |

The phase formerly named `01-requirements` is RENAMED to `01-proposal`. The planner for phase 01 changes from the main agent (skill directly writes artifacts) to the `proposal-planner` sub-agent, consistent with phases 02-03.

Each phase SHALL append its result to eval.json upon completion.

#### Scenario: Phase 01 uses proposal-planner sub-agent
- **WHEN** Phase 01-proposal executes
- **THEN** the `proposal-planner` sub-agent writes proposal.md + specs/
- **AND** the `proposal-evaluator` sub-agent evaluates against the checklist
- **AND** the phase follows the same P→E loop pattern as 02-dev-design and 03-test-design

#### Scenario: Full workflow progression with new phase ID
- **WHEN** all phases complete successfully from 01-proposal through 09-acceptance
- **THEN** eval.json contains entries for all 9 phases (including 01-proposal) with verdict "pass"

#### Scenario: Phase ordering enforcement
- **WHEN** a phase attempts to execute before all prior phases have passed (or been skipped)
- **THEN** eval-check SHALL block the phase and list the missing prior phases

## Module Contract

### Workflow Skills (`plugins/dev-team/skills/`)

| Workflow | workflow_type | Phase Table (server-side) |
|----------|--------------|--------------------------|
| workflow-requirement | `requirement` | 01-proposal through 09-acceptance |
| workflow-bug-fix (future) | `bug-fix` | 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance |
| workflow-refactor (future) | `refactor` | 01-proposal through 09-acceptance |

All workflow skills follow the thin loop pattern: context assembly → `eval/next` loop → stop on completion. Archive is performed manually by the user via `/dev-team:openspec-archive-change`. The phase table is defined server-side in the MCP server, not in the skill file.

### MCP Tools

| Tool | Purpose |
|------|---------|
| eval/next | Returns next phase to execute (agent_type, prompt); server-side gate/skip/retry/backtrack/round_limit |
| eval/check | Gate validation for single-phase execution |
| eval/log | Append evaluation result to eval.json |
