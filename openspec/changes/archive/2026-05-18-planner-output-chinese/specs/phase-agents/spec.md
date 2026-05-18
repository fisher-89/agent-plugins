## ADDED Requirements

### Requirement: Planner agents output in Chinese
Each Planner agent (`requirements-planner`, `test-design-planner`, `dev-proposal-planner`) SHALL produce .md artifacts whose body content is written in Chinese (简体中文).
The agent prompt SHALL include an explicit language constraint instructing the model to write all narrative content in Chinese.
Code identifiers, file paths, CLI commands, template variables, and widely-accepted technical abbreviations (API, JSON, SDK, CI/CD, URL, etc.) SHALL remain in English.

#### Scenario: Requirements planner outputs Chinese proposal
- **WHEN** requirements-planner agent writes proposal.md
- **THEN** all section body content (Problem, Background, Motivation, Stakeholders, Scope, Risks, Acceptance Criteria descriptions) is written in Chinese
- **AND** code identifiers, file paths, and technical abbreviations remain in English

#### Scenario: Test-design planner outputs Chinese test design
- **WHEN** test-design-planner agent writes test-design.md
- **THEN** all section body content (Test Levels, Coverage Map, Test Strategy, Boundary Cases, Test Data descriptions) is written in Chinese
- **AND** code identifiers, file paths, and framework names remain in English

#### Scenario: Dev-proposal planner outputs Chinese design and tasks
- **WHEN** dev-proposal-planner agent writes design.md and tasks.md
- **THEN** all section body content (Architecture Components, Data Flow, Route Design, Decisions, Task descriptions) is written in Chinese
- **AND** code identifiers, file paths, endpoint paths, and technical abbreviations remain in English

### Requirement: Planner templates use Chinese section headers
The artifact templates in `plugins/dev-team/templates/artifacts/` SHALL use Chinese for all section headers, table column labels, and static label text.
Template variables (`{{change_name}}`, `{{date}}`, etc.) SHALL remain as English identifiers.

#### Scenario: Templates guide Chinese output
- **WHEN** a Planner agent reads a template file
- **THEN** all section headers (e.g., `## 1. 问题`, `## 2. 干系人`) are in Chinese
- **AND** all template variables retain their English identifiers
