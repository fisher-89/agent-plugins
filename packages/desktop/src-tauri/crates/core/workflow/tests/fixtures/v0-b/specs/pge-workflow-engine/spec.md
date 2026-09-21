## ADDED Requirements

### Requirement: DESIGN phase executes P→E loop

The system SHALL execute DESIGN phases (requirements, test-design, dev-proposal) in Planner → Evaluator sequence.
The Planner SHALL produce a .md artifact following the suggested template — no checklist generation.
The Evaluator SHALL validate the .md artifact against its static checklist (embedded in agent prompt).
If any required checklist item fails, the skill SHALL loop back to the Planner with the failed items.
If all required items pass, the skill SHALL unlock the next phase.

#### Scenario: Planner writes .md artifact directly

- **WHEN** a DESIGN phase Planner agent is invoked
- **THEN** it outputs a single .md artifact (proposal.md / test-design.md / design.md) to `openspec/changes/<name>/phases/`

#### Scenario: DESIGN eval passes

- **WHEN** Evaluator checks the .md artifact against its static checklist
- **THEN** all required items are marked pass and the eval report has verdict "pass"

#### Scenario: DESIGN eval fails, loop back to Planner

- **WHEN** Evaluator finds a required checklist item with pass=false
- **THEN** eval report has verdict "fail" and the skill re-invokes the Planner with failed items

### Requirement: EXECUTION phase executes G→E loop

The system SHALL execute EXECUTION phases (test-gen, implementation) in Generator → Evaluator sequence.
The Generator SHALL write code files directly to disk — the git diff of uncommitted changes IS the artifact.
The Evaluator SHALL validate the Generator's code output (via git diff) against the prior DESIGN .md artifact using its static checklist.
If any required checklist item fails, the skill SHALL loop back to Generator.
If all required items pass, the skill SHALL unlock the next phase.

#### Scenario: Test-gen evaluation uses test-design.md as reference

- **WHEN** test-gen Evaluator runs
- **THEN** it reads test-design.md as its design reference and validates the generated test files (via git diff) against its static checklist

#### Scenario: Implementation evaluation uses design.md as reference

- **WHEN** implementation Evaluator runs
- **THEN** it reads design.md as its design reference and validates the implementation code (via git diff) against its static checklist

#### Scenario: EXECUTION eval fails, loop back to Generator

- **WHEN** Evaluator finds a required checklist item with pass=false
- **THEN** eval report has verdict "fail" and the skill re-invokes the Generator with failed items and eval notes

### Requirement: EVALUATOR-ONLY phase executes E directly

The system SHALL execute EVALUATOR-ONLY phases (code-review, acceptance) as Evaluator-only — no Planner, no Generator.
The Evaluator SHALL inspect the relevant DESIGN .md artifact and the codebase (diffs, files, reports) directly.
The Evaluator SHALL evaluate against its static checklist and output eval JSON with findings.
If the Evaluator finds issues, it SHALL set verdict "fail" with appropriate backtrack_to marker.

#### Scenario: Code-review evaluator inspects code diff

- **WHEN** code-review Evaluator runs
- **THEN** it reads design.md, inspects the staged code diff, and evaluates against its static checklist (security, test coverage, error handling, code quality)

#### Scenario: Acceptance evaluator traces requirements

- **WHEN** acceptance Evaluator runs
- **THEN** it reads proposal.md, inspects the codebase for acceptance criteria coverage, and evaluates against its static checklist (req traceability, no scope creep)

#### Scenario: Eval check script validates chain

- **WHEN** archive flow is triggered
- **THEN** the eval check script reads eval.json, extracts the latest entry per phase (by timestamp), and validates all phases passed with no gaps

### Requirement: AUTO phase has no PGE overhead

The system SHALL execute AUTO phases (static-check, test-execution) automatically after implementation Generator completes, without human invocation or PGE evaluation.

#### Scenario: Static-check auto-triggers after implementation

- **WHEN** implementation Generator completes writing code
- **THEN** static-check runs automatically via lint-runner and writes lint report to reports/

#### Scenario: Test-execution auto-triggers after implementation

- **WHEN** implementation Generator completes writing code
- **THEN** full test suite runs automatically via test-runner and writes test report to reports/

### Requirement: Archive flow executes sequentially

The system SHALL execute the archive flow as a non-PGE sequential process: (1) eval check script validates all phases passed, (2) OpenSpec archive finalizes the change, (3) git commit snapshots the work.
The eval check script SHALL read `eval.json`, extract the latest entry per phase by timestamp, verify all phases have verdict "pass" with no gaps, and verify all tasks in `tasks.md` are marked complete (`[x]`).
The archive flow SHALL replace the existing commit hook gates — no commit-time eval validation is performed.

#### Scenario: Eval check passes, archive proceeds

- **WHEN** eval check script confirms all phases passed and all tasks complete
- **THEN** openspec archive runs, then git commit executes

#### Scenario: Eval check fails on missing phase

- **WHEN** eval check script finds a required phase missing from eval.json
- **THEN** it outputs the missing phase name and exits with error — archive does not proceed

#### Scenario: Eval check fails on incomplete tasks

- **WHEN** eval check script finds unchecked tasks (`[ ]`) in tasks.md
- **THEN** it outputs the count of incomplete tasks and exits with error — archive does not proceed

### Requirement: Shared eval output format

Every Evaluator SHALL append its result to `openspec/changes/<name>/phases/eval.json` as an array entry: `{phase: string, timestamp: string, attempt: int, verdict: "pass"|"fail", report: string, items: [{item_id: string, pass: boolean, evidence: string, notes: string}], backtrack_to: string|null}`. The `report` field SHALL contain a human-readable summary of the evaluation (what was checked, key findings, why the verdict was reached), not exceeding 500 characters.
eval.json SHALL be an append-only array — each evaluation (pass or fail) adds a new entry. The latest entry per phase is determined by the maximum `timestamp` value.
Checklist items in evaluator prompts SHALL use the reference format: `[{id: string, criterion: string, required: boolean, evidence_hint: string}]`.

#### Scenario: Evaluator appends to eval.json

- **WHEN** any Evaluator completes its evaluation
- **THEN** it appends a new entry to `openspec/changes/<name>/phases/eval.json` with phase, timestamp, attempt, verdict, report (≤500 chars), items, and backtrack_to fields

#### Scenario: eval.json preserves full history

- **WHEN** a phase loops P→E→P→E with multiple attempts
- **THEN** eval.json contains all attempts (both pass and fail), each with incrementing attempt numbers and unique timestamps

#### Scenario: Latest result determined by timestamp

- **WHEN** the eval check script reads eval.json
- **THEN** it identifies the latest entry per phase by the maximum timestamp value

#### Scenario: Static checklist item has evidence hint

- **WHEN** a checklist item in an Evaluator prompt includes evidence_hint "Must list at least 3 stakeholders with roles"
- **THEN** the Evaluator checks that the proposal.md artifact satisfies this specific constraint and cites evidence
