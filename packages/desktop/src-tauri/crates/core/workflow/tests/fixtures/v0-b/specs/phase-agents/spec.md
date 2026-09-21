## ADDED Requirements

### Requirement: Planner agents produce .md artifacts

The system SHALL provide 3 Planner agents: `requirements-planner`, `test-design-planner`, `dev-proposal-planner`.
Each Planner agent SHALL produce a single .md artifact following the suggested template in `templates/artifacts/`.
Each Planner agent SHALL NOT produce a separate checklist JSON — checklists are static and embedded in Evaluator prompts.
Each Planner agent SHALL use the `opus` model.
There is NO separate Generator for DESIGN phases — the Planner is the sole producer.

#### Scenario: Requirements planner produces proposal.md

- **WHEN** requirements-planner agent is invoked
- **THEN** it writes proposal.md to `openspec/changes/<name>/phases/` following the proposal.md template

#### Scenario: Test-design planner produces test-design.md

- **WHEN** test-design-planner agent is invoked
- **THEN** it reads proposal.md, then writes test-design.md to `openspec/changes/<name>/phases/`

#### Scenario: Dev-proposal planner produces design.md

- **WHEN** dev-proposal-planner agent is invoked
- **THEN** it reads proposal.md and test-design.md, then writes design.md and tasks.md to `openspec/changes/<name>/phases/`

### Requirement: Generator agents produce code changes on disk

The system SHALL provide 2 Generator agents: `test-gen-generator`, `implementation-generator`.
Each Generator agent SHALL write code files directly to disk — the git diff of uncommitted changes IS the artifact.
Each Generator agent SHALL use the `sonnet` model.
Generator agents SHALL NOT produce JSON reports — the code they write is self-documenting, and describing it in JSON adds no value.

#### Scenario: Test-gen generator writes test files

- **WHEN** test-gen-generator agent is invoked
- **THEN** it reads test-design.md, then writes test skeleton files to disk colocated with source files

#### Scenario: Implementation generator writes implementation code

- **WHEN** implementation-generator agent is invoked
- **THEN** it reads design.md and tasks.md, then writes implementation code to disk for pending tasks

### Requirement: Evaluator agents have static checklists

The system SHALL provide 7 Evaluator agents, one per PGE phase: `requirements-evaluator`, `test-design-evaluator`, `dev-proposal-evaluator`, `test-gen-evaluator`, `implementation-evaluator`, `code-review-evaluator`, `acceptance-evaluator`.
Each Evaluator agent SHALL have a static binary checklist embedded in its agent prompt — checklist items are phase-specific and designed for the artifact type being evaluated.
Each Evaluator agent SHALL output an eval JSON with `verdict` (pass/fail), `items` (per-item pass/fail with evidence), and `backtrack_to` (nullable phase identifier).
Each Evaluator agent SHALL use the `opus` model.

#### Scenario: DESIGN evaluator checks .md artifact

- **WHEN** a DESIGN Evaluator (E1-E3) runs
- **THEN** it reads the Planner's .md artifact and evaluates it against its static checklist (completeness, clarity, coverage of required sections)

#### Scenario: EXECUTION evaluator checks generated code against .md design

- **WHEN** an EXECUTION Evaluator (E4-E5) runs
- **THEN** it runs git diff to inspect the Generator's code changes AND reads the prior DESIGN .md artifact, then evaluates the code against its static checklist

#### Scenario: EVALUATOR-ONLY evaluator inspects codebase

- **WHEN** an EVALUATOR-ONLY Evaluator (E6-E7) runs
- **THEN** it reads the relevant DESIGN .md artifact AND inspects the codebase (diffs, files, reports), then evaluates against its static checklist

#### Scenario: Evaluator marks item with evidence

- **WHEN** Evaluator checks a checklist item
- **THEN** the output includes the item_id, pass/fail, and a specific evidence quote or observation from the artifact or code

### Requirement: Context isolation prevents reasoning leakage

No agent SHALL have access to another agent's reasoning chain, conversation history, or tool-call details.
Agents MAY read each other's output artifacts (.md files, .json files) — these are the only inter-agent communication channel.
DESIGN Evaluators (E1-E3) SHALL receive only the .md artifact to evaluate.
EXECUTION Evaluators (E4-E5) SHALL receive the prior DESIGN .md artifact and access to git diff and code files to inspect the Generator's output.
EVALUATOR-ONLY Evaluators (E6-E7) SHALL receive the prior DESIGN .md artifact and access to the codebase for inspection.

#### Scenario: Evaluator cannot see Planner reasoning

- **WHEN** requirements-evaluator evaluates proposal.md
- **THEN** its context contains only proposal.md and its static checklist — no access to the requirements-planner's conversation or reasoning

#### Scenario: Code-review evaluator can inspect code

- **WHEN** code-review-evaluator runs
- **THEN** it can Read, Grep, and Glob the codebase to inspect staged changes, but cannot read the implementation-generator's conversation history

### Requirement: Agent tools scoped by role

Each Planner agent SHALL have access to Read and Write tools only.
Each Generator agent SHALL have access to Read, Write, Grep, Glob, and Bash tools.
DESIGN Evaluators (E1-E3) SHALL have access to Read and Write tools only. EXECUTION Evaluators (E4-E5) SHALL have access to Read, Write, and Bash tools (Bash for running git diff to inspect Generator output).
EVALUATOR-ONLY Evaluators (E6-E7) SHALL have access to Read, Write, Grep, Glob, and Bash tools (to inspect codebase, diffs, and reports).

#### Scenario: DESIGN evaluator cannot modify code

- **WHEN** a DESIGN Evaluator runs
- **THEN** it has no access to Bash, Grep, or Glob — only Read (to read its input artifact) and Write (to write its eval report)

#### Scenario: Acceptance evaluator can verify implementation

- **WHEN** acceptance-evaluator runs
- **THEN** it can Grep for requirement IDs in code, Glob for test files, and Read implementation files to verify traceability
