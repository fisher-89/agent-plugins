## ADDED Requirements

### Requirement: Seven user-triggered phase skills

The system SHALL provide 7 skills: `dev-team:phase-requirements`, `dev-team:phase-test-design`, `dev-team:phase-dev-proposal`, `dev-team:phase-test-gen`, `dev-team:phase-implement`, `dev-team:phase-code-review`, `dev-team:phase-acceptance`.

#### Scenario: DESIGN skill triggers P→E

- **WHEN** user invokes `dev-team:phase-requirements`, `dev-team:phase-test-design`, or `dev-team:phase-dev-proposal`
- **THEN** the skill invokes the corresponding Planner agent (which writes a .md artifact), then the corresponding Evaluator agent (which evaluates with its static checklist) in sequence

#### Scenario: EXECUTION skill triggers G→E

- **WHEN** user invokes `dev-team:phase-test-gen`
- **THEN** the skill invokes test-gen-generator agent (writes test files to disk), then test-gen-evaluator agent (inspects git diff + test-design.md) in sequence

#### Scenario: Implementation skill triggers AUTO phases

- **WHEN** implementation-generator completes writing implementation code
- **THEN** static-check (lint + type) and test-execution (full suite) run automatically before returning control to the Evaluator, which inspects the git diff

#### Scenario: EVALUATOR-ONLY skill triggers E directly

- **WHEN** user invokes `dev-team:phase-code-review` or `dev-team:phase-acceptance`
- **THEN** the skill invokes ONLY the corresponding Evaluator agent (no Planner, no Generator). The Evaluator inspects the codebase/diffs/reports directly and appends result to eval.json.

### Requirement: Skill detects active change

Each phase skill SHALL detect the active change using `find_active_change()` with the same priority ordering as existing hooks (pending tasks > staged files > most recent).

#### Scenario: Skill runs against active change

- **WHEN** user invokes a phase skill without specifying a change name
- **THEN** the skill automatically operates on the change with pending tasks, or the most recently modified change

### Requirement: Loop behavior varies by phase type

DESIGN skills SHALL loop P→E→P→E... until Evaluator returns verdict "pass".
EXECUTION skills SHALL loop G→E→G→E... until Evaluator returns verdict "pass".
EVALUATOR-ONLY skills SHALL run the Evaluator once (no loop — Evaluator output is the final action). If backtrack is set, the user manually invokes the target phase.

#### Scenario: DESIGN loop re-invokes Planner

- **WHEN** requirements-evaluator returns verdict "fail" with 3 failing items
- **THEN** the skill re-invokes requirements-planner with the 3 failing items and their eval notes, then re-runs the Evaluator

#### Scenario: EVALUATOR-ONLY skill runs once

- **WHEN** code-review-evaluator returns verdict "fail" with backtrack_to "03-dev-proposal"
- **THEN** the skill outputs the result and stops — the user must manually invoke the target phase

### Requirement: Deprecated skill migration messages

The system SHALL update `openspec-propose` and `openspec-apply-change` skills to display migration messages directing users to the new phase skills. The deprecated skills SHALL remain functional during migration.

#### Scenario: User invokes deprecated propose skill

- **WHEN** user invokes `/dev-team:openspec-propose`
- **THEN** the skill displays a message directing them to `/dev-team:phase-requirements` and `/dev-team:phase-dev-proposal` before proceeding with existing behavior
