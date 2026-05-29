## ADDED Requirements

### Requirement: Skill routing maps phase skills to agent chains
The system SHALL route phase skill invocations to their corresponding agents in the correct sequence.
The system SHALL support the following skill-to-agent mappings:
- `phase-requirements` → `requirements-planner` → `requirements-evaluator` (DESIGN: P→E)
- `phase-test-design` → `test-design-planner` → `test-design-evaluator` (DESIGN: P→E)
- `phase-dev-design` → `dev-design-planner` → `dev-design-evaluator` (DESIGN: P→E)
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

## REMOVED Requirements

### Requirement: Intent classification from prompt text
**Reason**: Automatic intent classification via regex keyword matching is unreliable and removes user control. Users will manually invoke skills via slash commands instead.
**Migration**: Use explicit slash commands: `/dev-team:openspec-explore` to explore, `/dev-team:openspec-propose` to propose, `/dev-team:openspec-apply-change` to implement, `/dev-team:openspec-archive-change` to archive. No replacement for automatic classification.

### Requirement: Confidence level assignment
**Reason**: Part of the removed intent classification system.
**Migration**: Not needed — users explicitly choose their action.

### Requirement: Scope estimation from prompt text
**Reason**: Part of the removed intent classification system. Scope estimation from prompt text was inherently unreliable.
**Migration**: Scope decisions are made during the explore/propose phase rather than inferred from prompt keywords.

### Requirement: Trivial fix detection
**Reason**: The trivial/non-trivial distinction was used to decide whether to apply workflow gates. With manual step selection, the user decides whether a change needs the full SDD workflow.
**Migration**: Users can still make small fixes directly without invoking any skill — the TDD gate will only trigger if an active OpenSpec change exists.

### Requirement: Routing decision based on intent, scope, and state
**Reason**: The entire routing engine is removed. Routing was the core of the auto-intent system, producing auto/suggest/direct decisions that the user never explicitly requested.
**Migration**: Users manually select which step to execute. PreToolUse hooks continue to enforce workflow quality gates regardless of how the user arrived at the action.
