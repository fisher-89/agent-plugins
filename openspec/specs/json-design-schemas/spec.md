## ADDED Requirements

### Requirement: Markdown templates for Planner artifacts
The system SHALL provide suggested .md templates in `templates/artifacts/` for Planner outputs:
- `proposal.md.template` — sections: Problem, Stakeholders, Scope (in_scope / out_of_scope), Risks (with mitigations), Acceptance Criteria (with validation method)
- `test-design.md.template` — sections: Test Levels (level, scope, framework), Coverage Map (requirement_id → test_file), Test Strategy, Boundary Cases
- `design.md.template` — sections: Architecture Components, Data Flow, Route Design, Decisions (with rationale and alternatives)

Templates are suggestions, not enforced schemas — Planners may add or restructure sections as needed. Evaluators check content quality, not section ordering.

#### Scenario: Planner follows template structure
- **WHEN** requirements-planner writes proposal.md
- **THEN** the output covers all suggested sections: Problem, Stakeholders, Scope, Risks, Acceptance Criteria

#### Scenario: Template is advisory, not enforced
- **WHEN** a Planner adds an additional section beyond the template
- **THEN** the Evaluator evaluates the content against its checklist criteria, not against template section conformance

### Requirement: Shared JSON schemas for evaluation
The system SHALL define JSON schemas for shared evaluation artifacts:

- `eval.schema.json`: `phase` (string, e.g. "01-requirements"), `timestamp` (ISO 8601 string), `attempt` (int, 1-based), `verdict` ("pass"|"fail"), `report` (string, max 500 chars — evaluator's summary of what was checked and why the verdict was reached), `items` (array of {item_id, pass, evidence, notes}), `backtrack_to` (string|null), `schema_version` (string). All evaluators append entries to a single `eval.json` array — no per-phase eval files.
- `checklist.schema.json`: reference format `{id: string, criterion: string, required: boolean, evidence_hint: string}` — used to document checklist item structure, not as a generated artifact

#### Scenario: Eval entry conforms to shared schema
- **WHEN** any Evaluator appends an entry to eval.json
- **THEN** the entry conforms to eval.schema.json regardless of which phase produced it

#### Scenario: eval.json is append-only
- **WHEN** a phase re-evaluates after a fail verdict
- **THEN** the new result is appended to eval.json — previous entries for the same phase are preserved, not overwritten

#### Scenario: Latest entry per phase identified by timestamp
- **WHEN** eval check script reads eval.json
- **THEN** it groups entries by phase and selects the entry with the maximum timestamp as the latest result

#### Scenario: Checklist items follow reference format
- **WHEN** an Evaluator's static checklist is documented
- **THEN** each item follows the checklist.schema.json format with id, criterion, required, and evidence_hint fields
