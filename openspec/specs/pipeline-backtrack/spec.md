## ADDED Requirements

### Requirement: Evaluator can mark backtrack target
The system SHALL allow Evaluators to set a `backtrack_to` field in their eval JSON, pointing to a prior phase identifier (e.g., "01-requirements", "03-dev-proposal").
The system SHALL only permit backtrack from: P9 (acceptance) to P1 (requirements), and P7 (code-review) to P3 (dev-proposal).

#### Scenario: Acceptance evaluator finds unmet requirement
- **WHEN** acceptance-evaluator finds acceptance_criteria from proposal.md without corresponding test or implementation evidence
- **THEN** eval report has verdict "fail" with backtrack_to set to "01-requirements"

#### Scenario: Code review evaluator finds design deviation
- **WHEN** code-review-evaluator finds implementation that contradicts design.md
- **THEN** eval report has verdict "fail" with backtrack_to set to "03-dev-proposal"

### Requirement: Backtrack triggers prior phase re-evaluation
When a phase skill detects a `backtrack_to` marker targeting its phase in the change's eval reports, it SHALL re-run the Evaluator for that phase before allowing progression, and SHALL clear the backtrack marker after evaluation passes.

#### Scenario: Requirements skill detects backtrack marker
- **WHEN** user invokes `dev-team:phase-requirements` and a backtrack marker pointing to "01-requirements" exists
- **THEN** the skill re-runs requirements-evaluator against the current proposal.md before showing the Planner prompt

#### Scenario: Backtrack marker cleared after pass
- **WHEN** re-evaluation passes with verdict "pass"
- **THEN** the backtrack_to field is set to null in the eval report

### Requirement: Backtrack chain integrity
If backtrack would create a cycle (e.g., P3 → P9 → P3), the system SHALL detect the cycle and require manual resolution by outputting the cycle path.

#### Scenario: Cycle detected
- **WHEN** an Evaluator attempts to set backtrack_to that would create a phase loop
- **THEN** the eval report includes a warning and the skill pauses for user intervention
