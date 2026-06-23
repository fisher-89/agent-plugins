## ADDED Requirements

### Requirement: code-analyze-planner reverse-engineers existing code into design.md
`plugins/dev-team/agents/code-analyze-planner.md` SHALL be a DESIGN planner agent that reads `proposal.md` and the existing codebase, then writes `design.md` in a format compatible with what `test-design-planner` expects.

Unlike `dev-design-planner`, which designs future implementation, `code-analyze-planner` SHALL reverse-engineer the architecture of already-implemented code.

The agent SHALL:
1. Read `openspec/changes/<change-name>/proposal.md` for test coverage scope and acceptance criteria
2. Read `plugins/dev-team/templates/artifacts/design.md.template` for structure guidance
3. Explore the existing codebase (Read, Grep, Glob) to identify components, data flows, and module boundaries
4. Write `openspec/changes/<change-name>/design.md` covering:
   - **Architecture Components**: Each component with responsibility, dependencies, technology, and file paths
   - **Data Flow**: How data moves through the existing system
   - **Route / API Design**: If applicable — existing endpoints with method, path, input, output
   - **Decisions**: Documented architectural patterns observed in the code (not future design decisions)
   - **Change Scope / Module Boundaries**: Modules in scope for test coverage per proposal.md
5. NOT write `tasks.md` (test-only workflow has no implementation phase)

The agent SHALL NOT include testing strategy, test architecture, unit test, or integration test sections in design.md — tests are handled by downstream workflow phases.

#### Scenario: code-analyze phase uses planner and evaluator without tasks.md (AC-3)
- **WHEN** `code-analyze-planner` completes for a test-only change
- **THEN** it writes `openspec/changes/<change-name>/design.md`
- **AND** does NOT write `openspec/changes/<change-name>/tasks.md`
- **AND** phase `02-code-analyze` is evaluated by `code-analyze-evaluator` per `test-only-workflow` spec

#### Scenario: design.md is compatible with test-design-planner input
- **WHEN** `test-design-planner` reads the design.md produced by `code-analyze-planner`
- **THEN** it finds Architecture Components, Data Flow, and module/file paths sufficient to determine test scope
- **AND** can call `test_resolve_paths` using module paths from the design

#### Scenario: planner reads existing codebase not proposal-only
- **WHEN** `code-analyze-planner` executes its Process section
- **THEN** it uses Grep/Glob/Read to inspect actual source files referenced in proposal.md scope
- **AND** documents observed architecture rather than proposing new implementation

#### Scenario: planner agent definition exists with correct metadata
- **WHEN** reading `plugins/dev-team/agents/code-analyze-planner.md`
- **THEN** the frontmatter `name` is `code-analyze-planner`
- **AND** the description indicates reverse-engineering existing code for test coverage workflows
- **AND** Tools include Read, Write, Grep, Glob

### Requirement: code-analyze-evaluator validates design.md without tasks.md
`plugins/dev-team/agents/code-analyze-evaluator.md` SHALL be a DESIGN evaluator agent that validates `design.md` against a static checklist adapted from `dev-design-evaluator`, excluding tasks.md requirements.

The evaluator SHALL:
1. Read `design.md` and `proposal.md` (NOT `tasks.md`)
2. Evaluate checklist items covering: architecture components, data flow, decision documentation, proposal AC coverage, project architecture consistency, and template section completeness
3. NOT require `tasks.md` existence or task ordering checks (D5, D6 from dev-design-evaluator are omitted or replaced)
4. Verify design.md does NOT contain test strategy sections
5. Call `mcp__plugin_dev-team_dev-team__phase_log` with `phase: "02-code-analyze"` (no `workflow_type` parameter — server reads from `workflow.json`)

Static checklist SHALL include at minimum:
- Architecture components table complete (responsibility, dependencies, technology, file paths)
- Data flow describes concrete steps for existing code paths
- Design covers every acceptance criterion from proposal.md
- No test strategy sections in design.md
- No tasks.md required

#### Scenario: evaluator passes without tasks.md
- **WHEN** `code-analyze-evaluator` evaluates a complete design.md and proposal.md with no tasks.md present
- **THEN** it MAY return verdict `"pass"` if all checklist items pass
- **AND** does NOT fail for missing tasks.md

#### Scenario: evaluator fails incomplete architecture analysis
- **WHEN** design.md lacks file paths for components in proposal scope
- **THEN** the evaluator returns verdict `"fail"` with evidence citing the incomplete component table
- **AND** the skill loops back to `code-analyze-planner`

#### Scenario: evaluator logs phase 02-code-analyze
- **WHEN** `code-analyze-evaluator` completes evaluation
- **THEN** it calls `phase_log` with `phase: "02-code-analyze"`
- **AND** includes checklist items in the log entry

#### Scenario: evaluator agent definition exists with Write disallowed
- **WHEN** reading `plugins/dev-team/agents/code-analyze-evaluator.md`
- **THEN** the frontmatter `name` is `code-analyze-evaluator`
- **AND** Write/Edit tools are disallowed (evaluator read-only pattern)
- **AND** Process section references `phase_log` MCP tool without `workflow_type` parameter (server reads from `workflow.json`)

### Requirement: proposal-planner confirms workflow_type when not set
`plugins/dev-team/agents/proposal-planner.md` SHALL read `openspec/changes/<change-name>/workflow.json` early in its Process section.

When `workflow.json` does not exist or `workflow_type` is NOT set (typical when the user entered via `phase-proposal` or direct agent invocation rather than a `workflow-*` skill), the agent SHALL:
1. Use AskQuestion to present available workflow types: `requirement`, `bug-fix`, `refactor`, `test-only`
2. Include brief descriptions to help the user choose (e.g., test-only = existing code, supplement tests only)
3. Write the confirmed `workflow_type` to `workflow.json` before proceeding to write proposal.md and specs/

When `workflow_type` IS already set in `workflow.json` (user entered via `workflow-*` skill), the agent SHALL NOT re-prompt and SHALL proceed directly.

#### Scenario: proposal-planner asks when workflow_type absent (AC-18)
- **WHEN** `proposal-planner` executes for a change that has no `workflow.json` or `workflow.json` lacks `workflow_type`
- **THEN** it uses AskQuestion to confirm the workflow type with the user
- **AND** writes the selected value to `workflow.json`
- **AND** only then writes proposal.md and specs/

#### Scenario: proposal-planner skips confirmation when workflow_type set
- **WHEN** `proposal-planner` executes for a change whose `workflow.json` already has `{"workflow_type": "test-only"}`
- **THEN** it does NOT ask the user to confirm workflow type
- **AND** proceeds directly to write proposal artifacts

#### Scenario: proposal-planner test-only choice aligns with test coverage intent
- **WHEN** user selects `test-only` during proposal-planner confirmation
- **THEN** `workflow.json` is written with `{"workflow_type": "test-only"}`
- **AND** subsequent `phase_next(change)` calls use the test-only phase table

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | Role | Tools | Output Artifacts | Contract |
|-------|------|-------|------------------|----------|
| code-analyze-planner | DESIGN Planner | Read, Write, Grep, Glob, Bash | `design.md` only | Reverse-engineer existing code; no tasks.md; compatible with test-design-planner |
| code-analyze-evaluator | DESIGN Evaluator | Read, phase_log | eval.json entry | Validate design.md against proposal; no tasks.md requirement; phase `02-code-analyze` |
| proposal-planner | DESIGN Planner | Read, Write, AskQuestion | proposal.md, specs/, `workflow.json` | Confirm workflow_type when absent; write to `workflow.json`; then write proposal artifacts |

### Phase Assignment (workflow.ts)

| Phase | Planner | Evaluator |
|-------|---------|-----------|
| 02-code-analyze (test-only only) | `dev-team:code-analyze-planner` | `dev-team:code-analyze-evaluator` |
