## ADDED Requirements

### Requirement: test-only six-phase workflow structure
The system SHALL support a `test-only` workflow_type with 6 phases optimized for supplementing test coverage on already-implemented code:

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| 01-proposal | 01-proposal | DESIGN Planner→Evaluator | Test-focused proposal (coverage gaps, testing strategies, acceptance criteria for existing code) |
| 02-code-analyze | 02-code-analyze | DESIGN Planner→Evaluator | Reverse-engineer existing code architecture; output design.md only (no tasks.md) |
| 03-test-design | 03-test-design | DESIGN Planner→Evaluator | Test scenario design (reuses existing agents) |
| 04-test-gen | 04-test-gen | EXEC Generator→Evaluator | Test code generation (reuses existing agents) |
| 06-unit-test | 06-unit-test | EXEC Executor→Evaluator | Unit test execution (reuses existing agents) |
| 08-integration-test | 08-integration-test | EXEC Executor→Evaluator | Integration test execution (reuses existing agents) |

Phases `02-dev-design`, `05-implement`, `07-code-review`, and `09-acceptance` SHALL NOT appear in the `test-only` phase table.

The `test-only` workflow SHALL NOT include an acceptance phase. Workflow completion SHALL occur when all 6 phases in the table have valid (non-stale) pass entries.

A change executes in test-only mode when its `workflow.json` contains `{"workflow_type": "test-only"}`. MCP tools resolve this automatically — callers SHALL NOT pass `workflow_type` as a parameter.

#### Scenario: test-only phase table excludes implementation phases
- **WHEN** `getPhaseTable("test-only")` is called
- **THEN** it returns exactly 6 phase definitions with IDs: `01-proposal`, `02-code-analyze`, `03-test-design`, `04-test-gen`, `06-unit-test`, `08-integration-test`
- **AND** does NOT include `02-dev-design`, `05-implement`, `07-code-review`, or `09-acceptance`

#### Scenario: 02-code-analyze uses code-analyze agents
- **WHEN** Phase `02-code-analyze` executes in the `test-only` workflow
- **THEN** the planner agent_type is `dev-team:code-analyze-planner`
- **AND** the evaluator agent_type is `dev-team:code-analyze-evaluator`
- **AND** the phase follows the DESIGN Planner→Evaluator pattern

### Requirement: test-only prerequisite dependency table
The `test-only` workflow_type SHALL define explicit prerequisites:

| Phase | Prerequisites |
|-------|--------------|
| 01-proposal | [] |
| 02-code-analyze | [01-proposal] |
| 03-test-design | [01-proposal, 02-code-analyze] |
| 04-test-gen | [03-test-design] |
| 06-unit-test | [04-test-gen] |
| 08-integration-test | [04-test-gen] |

`getPrerequisites(phaseId, "test-only")` SHALL return the prerequisites from this table.
`getDependents(phaseId, "test-only")` SHALL derive the reverse mapping from this table.

#### Scenario: getPrerequisites returns test-only dependencies
- **WHEN** `getPrerequisites("02-code-analyze", "test-only")` is called
- **THEN** it returns `["01-proposal"]`

- **WHEN** `getPrerequisites("03-test-design", "test-only")` is called
- **THEN** it returns `["01-proposal", "02-code-analyze"]`

- **WHEN** `getPrerequisites("04-test-gen", "test-only")` is called
- **THEN** it returns `["03-test-design"]`

- **WHEN** `getPrerequisites("06-unit-test", "test-only")` is called
- **THEN** it returns `["04-test-gen"]`

- **WHEN** `getPrerequisites("08-integration-test", "test-only")` is called
- **THEN** it returns `["04-test-gen"]`

#### Scenario: getDependents returns test-only reverse dependencies (AC-5, AC-6)
- **WHEN** `getDependents("01-proposal", "test-only")` is called
- **THEN** it returns `["02-code-analyze", "03-test-design"]`

- **WHEN** `getDependents("02-code-analyze", "test-only")` is called
- **THEN** it returns `["03-test-design"]`

- **WHEN** `getDependents("04-test-gen", "test-only")` is called
- **THEN** it returns `["06-unit-test", "08-integration-test"]`

### Requirement: test-only phase progression via phase_next
When change `workflow.json` has `{"workflow_type": "test-only"}`, `phase_next(change)` SHALL advance phases according to the prerequisite DAG: `01-proposal` → `02-code-analyze` → `03-test-design` → `04-test-gen` → `06-unit-test` / `08-integration-test`.

#### Scenario: phase_next returns 01-proposal on empty eval (AC-1)
- **WHEN** change `workflow.json` has `{"workflow_type": "test-only"}`
- **AND** `phase_next(change)` is called for a change with no eval.json entries
- **THEN** it returns `next_phase: "01-proposal"`
- **AND** `done: false`

#### Scenario: test-only phases advance in DAG order (AC-2)
- **WHEN** change `workflow.json` has `{"workflow_type": "test-only"}`
- **AND** phases 01 through 03 have valid non-stale pass entries, and 04-test-gen has not passed
- **THEN** `phase_next(change)` returns `next_phase: "04-test-gen"`

- **WHEN** phases 01 through 04 have valid pass entries and neither 06 nor 08 has passed
- **THEN** `phase_next(change)` returns the earlier leaf phase in phase table order (`06-unit-test` before `08-integration-test`)

#### Scenario: test-design blocked until code-analyze passes (AC-4)
- **WHEN** change `workflow.json` has `{"workflow_type": "test-only"}`
- **AND** 01-proposal has valid pass but 02-code-analyze does not
- **THEN** `phase_next(change)` returns `next_phase: "02-code-analyze"`
- **AND** does NOT return `03-test-design`

- **WHEN** 01-proposal and 02-code-analyze have valid pass entries and 03-test-design has not passed
- **THEN** `phase_next(change)` returns `next_phase: "03-test-design"`
- **AND** the test-design-planner can read `design.md` produced by code-analyze

#### Scenario: unit and integration tests run in parallel after test-gen
- **WHEN** change `workflow.json` has `{"workflow_type": "test-only"}`
- **AND** phases 01–04 have valid pass entries, 06-unit-test has passed, and 08-integration-test has not
- **THEN** `phase_next(change)` returns `next_phase: "08-integration-test"`

### Requirement: test-only workflow completion semantics
When all 6 test-only phases have valid (non-stale) pass entries, `phase_next(change)` SHALL return `done: true` with `total_phases: 6`. The workflow SHALL NOT require or execute `09-acceptance`.

When tests discover production code bugs, the workflow SHALL handle them via the engine-layer backtrack rejection and evaluator self-adaptation flow defined in this spec and `pge-workflow-engine` spec — NOT by writing invalid `backtrack_to` targets to eval.json.

#### Scenario: phase_next returns done after all six phases pass (AC-7)
- **WHEN** change `workflow.json` has `{"workflow_type": "test-only"}`
- **AND** phases 01, 02, 03, 04, 06, and 08 all have valid non-stale pass entries
- **THEN** `phase_next(change)` returns `done: true`
- **AND** `total_phases` is 6
- **AND** no `09-acceptance` phase is executed or required

#### Scenario: code bug discovery does not write invalid backtrack (AC-11, AC-16)
- **WHEN** change `workflow.json` has `{"workflow_type": "test-only"}`
- **AND** `unit-test-evaluator` calls `phase_log` with `backtrack_to: "05-implement"`
- **THEN** `phase_log` rejects the call and does NOT write to eval.json
- **AND** the evaluator re-calls `phase_log` with `verdict: "fail"`, `backtrack_to: null`, and a report containing code bug details
- **AND** `phase_next` does NOT later error with `invalid_backtrack_target`

### Requirement: test-only evaluator expected path for code bug discovery
For changes whose `workflow.json` has `workflow_type: test-only`, the evaluator prompts for phases `06-unit-test` and `08-integration-test` in `PHASE_TEST_ONLY` SHALL include a WORKFLOW_CONTEXT instruction:

> WORKFLOW_CONTEXT: test-only — 无 implement/dev-design 阶段。若 phase_log 因 backtrack 目标不存在而拒绝调用，以 verdict:'fail', backtrack_to:null 重新记录，report 中包含发现的代码 bug 详情，然后返回主 agent 附带 bug 信息摘要。

The unit-test-evaluator and integration-test-evaluator agent `.md` decision trees SHALL remain unchanged. Evaluators call `phase_log` without a `workflow_type` parameter — the server reads it from `workflow.json`.

#### Scenario: evaluator prompts include WORKFLOW_CONTEXT instruction (AC-14)
- **WHEN** `phase_next(change)` returns phase `06-unit-test` or `08-integration-test` for a change with `workflow_type: test-only`
- **THEN** the evaluator prompt includes the WORKFLOW_CONTEXT test-only instruction
- **AND** the prompt instructs re-logging with `verdict: "fail"`, `backtrack_to: null` when backtrack is rejected

#### Scenario: evaluator self-adapts after invalid backtrack rejection (AC-16)
- **WHEN** `phase_log` rejects a call because `backtrack_to: "05-implement"` is not in the test-only phase table
- **THEN** the evaluator re-calls `phase_log` with `verdict: "fail"`, `backtrack_to: null`
- **AND** the report field includes bug details in the form: `测试发现 N 个代码 bug (工作流无 implement 阶段): {bug_summary}`
- **AND** the evaluator returns structured bug findings to the main agent

#### Scenario: reused test phase planner prompts unchanged
- **WHEN** `phase_next(change)` returns phases `03-test-design` or `04-test-gen` for a change with `workflow_type: test-only`
- **THEN** the planner and evaluator prompt strings match those used for `workflow_type: "requirement"`

### Requirement: test-only proposal prompt customization
For changes whose `workflow.json` has `workflow_type: test-only`, the `01-proposal` planner prompt in `workflow.ts` SHALL guide the proposal toward test coverage gaps, testing strategies, and acceptance criteria for existing code. It SHALL differ from the `requirement` workflow `01-proposal` prompt.

Phases `03-test-design` and `04-test-gen` SHALL use the same prompts as the `requirement` workflow. Phases `06-unit-test` and `08-integration-test` SHALL use requirement-equivalent base prompts plus the WORKFLOW_CONTEXT instruction defined above.

#### Scenario: 01-proposal prompt differs from requirement workflow (AC-9)
- **WHEN** `phase_next(change)` returns phase `01-proposal` for a change with `workflow_type: test-only`
- **THEN** the planner prompt references test coverage gaps and testing strategies for existing code
- **AND** the prompt string differs from the `requirement` workflow `01-proposal` prompt

## Module Contract

### workflow.ts (`plugins/dev-team/bin/src/lib/`)

| Export / Constant | Purpose |
|-------------------|---------|
| `PHASE_TEST_ONLY` | 6-phase table; `06-unit-test`/`08-integration-test` evaluator prompts include WORKFLOW_CONTEXT |
| `PHASE_TEST_ONLY_PREREQUISITES` | Prerequisite DAG defined above |
| `PHASE_TABLES['test-only']` | Registers PHASE_TEST_ONLY |
| `PHASE_PREREQUISITES_TABLES['test-only']` | Registers PHASE_TEST_ONLY_PREREQUISITES |

### Change Directory

| File | Field | Value for test-only |
|------|-------|---------------------|
| `workflow.json` | `workflow_type` | `test-only` |

### MCP Tools

| Tool | Contract |
|------|----------|
| phase_next | Reads `workflow_type` from `workflow.json`; returns phases, agents, and prompts per this spec |
| phase_log | Reads `workflow_type` from `workflow.json`; validates backtrack against test-only phase table |

### Tests

| File | AC Coverage |
|------|-------------|
| `phase-next.test.ts` | AC-1, AC-2, AC-4, AC-7 (fixture `workflow.json` with `workflow_type: test-only`) |
| `workflow.test.ts` | AC-5, AC-6 |
| `phase-log.test.ts` | AC-11, AC-12, AC-13 (fixture `workflow.json`) |
