## MODIFIED Requirements

### Requirement: Skill routing maps phase skills to agent chains

The system SHALL route phase skill invocations to their corresponding agents in the correct sequence.
The system SHALL support the following skill-to-agent mappings:

- `phase-requirements` → `requirements-planner` → `requirements-evaluator` (DESIGN: P→E)
- `phase-test-design` → `test-design-planner` → `test-design-evaluator` (DESIGN: P→E)
- `phase-dev-proposal` → `dev-proposal-planner` → `dev-proposal-evaluator` (DESIGN: P→E)
- `phase-test-gen` → `test-gen-generator` → `test-gen-evaluator` (EXECUTION: G→E)
- `phase-implement` → `implementation-generator` → `implementation-evaluator` (EXECUTION: G→E)
- `phase-code-review` → `code-review-evaluator` (EVALUATOR-ONLY: E)
- `phase-acceptance` → `acceptance-evaluator` (EVALUATOR-ONLY: E)
- `phase-archive` → archive flow (sequential: eval check → openspec archive → git commit)

#### Scenario: DESIGN skill routes to P→E

- **WHEN** user invokes `/dev-team:phase-requirements`
- **THEN** the skill sequentially invokes requirements-planner then requirements-evaluator (no Generator)

#### Scenario: EXECUTION skill routes to G→E

- **WHEN** user invokes `/dev-team:phase-implement`
- **THEN** the skill sequentially invokes implementation-generator then implementation-evaluator

#### Scenario: EVALUATOR-ONLY skill routes to E

- **WHEN** user invokes `/dev-team:phase-code-review`
- **THEN** the skill invokes code-review-evaluator only (no Planner, no Generator)

#### Scenario: Eval loop on failure (DESIGN)

- **WHEN** a DESIGN Evaluator returns verdict "fail"
- **THEN** the skill re-invokes the Planner with the failed checklist items and eval notes

#### Scenario: Eval loop on failure (EXECUTION)

- **WHEN** an EXECUTION Evaluator returns verdict "fail"
- **THEN** the skill re-invokes the Generator with the failed checklist items and eval notes

#### Scenario: No loop for EVALUATOR-ONLY failure

- **WHEN** an EVALUATOR-ONLY Evaluator returns verdict "fail" with backtrack_to set
- **THEN** the skill outputs the result and stops — the user must manually invoke the target phase
