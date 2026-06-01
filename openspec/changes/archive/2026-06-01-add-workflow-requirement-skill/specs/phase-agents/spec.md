## ADDED Requirements

### Requirement: proposal-planner agent
The system SHALL provide `dev-team:proposal-planner` agent at `agents/proposal-planner.md`.

The agent SHALL:
- Model: opus
- Tools: Read, Write, Grep, Glob, Bash
- Read `openspec/changes/<change-name>/proposal.md.template` for structure
- Read existing capability list via `openspec spec list --json`
- Read CLAUDE.md for project conventions
- Accept optional EXPLORE_CONTEXT_SUMMARY from the calling skill/workflow prompt

The agent SHALL write:
- `openspec/changes/<change-name>/proposal.md` covering: Problem, Proposal, Capabilities, Scope, Acceptance Criteria, Risks
- `openspec/changes/<change-name>/specs/<capability>/spec.md` for each capability:
  - NEW capabilities: `## ADDED Requirements` with `### Requirement: <name>`, SHALL/MUST, at least one `#### Scenario:` in **WHEN**/**THEN** format
  - MODIFIED capabilities: Read existing spec at `openspec/specs/<capability>/spec.md`, use delta headers (`## ADDED/MODIFIED/REMOVED/RENAMED Requirements`)

Language: 简体中文 for narrative, English for code identifiers, file paths, CLI commands, technical abbreviations, spec headers, scenario markers, and normative keywords.

#### Scenario: proposal-planner writes proposal.md from template
- **WHEN** invoked with change name `<name>`
- **THEN** the agent reads `plugins/dev-team/templates/artifacts/proposal.md.template`
- **AND** writes `openspec/changes/<name>/proposal.md` filling all template sections
- **AND** no placeholder content (TODO, TBD, {{...}}) remains

#### Scenario: proposal-planner writes specs for each capability
- **WHEN** proposal.md lists capabilities in the 能力 section
- **THEN** the agent writes a spec.md under `openspec/changes/<name>/specs/<capability>/` for each listed capability
- **AND** each spec.md uses delta headers appropriate to the capability type (new or modified)
- **AND** each requirement has at least one scenario with `#### Scenario:` (exactly 4 #)

#### Scenario: proposal-planner incorporates explore context
- **WHEN** the calling skill/workflow provides EXPLORE_CONTEXT_SUMMARY in the agent prompt
- **THEN** the agent uses the explore context as reference material for the problem statement and proposed solution
- **AND** the agent still validates all content against the static template and CLI instructions

## RENAMED Requirements

### RENAMED: requirements-evaluator → proposal-evaluator
- **FROM**: `requirements-evaluator` — Evaluates proposal.md against static binary checklist, appends to eval.json with phase `01-requirements`
- **TO**: `proposal-evaluator` — Evaluates proposal.md against the same static binary checklist, appends to eval.json with phase `01-proposal`

The agent file SHALL be renamed: `agents/requirements-evaluator.md` → `agents/proposal-evaluator.md`.

## MODIFIED Requirements

### Requirement: Subagent agent.md 承载领域知识
The sub-agent table SHALL include the new `proposal-planner` agent and reflect the renamed `proposal-evaluator`:

| Agent | 角色 | MCP Tool |
|-------|------|----------|
| proposal-planner | Planner subagent | (none — writes files directly) |
| proposal-evaluator | Evaluator subagent | eval/log |
| architecture | Architecture agent | archi/query, archi/validate, archi/write, archi/check, eval/log |
| dev-design-planner | Planner subagent | (none) |
| dev-design-evaluator | Evaluator subagent | eval/log |
| test-design-planner | Planner subagent | (none) |
| test-design-evaluator | Evaluator subagent | eval/log |
| test-gen-generator | Generator subagent | (none) |
| test-gen-evaluator | Evaluator subagent | eval/log |
| implementation-generator | Generator subagent | (none) |
| implementation-evaluator | Evaluator subagent | eval/log |
| unit-test-executor | Executor subagent (sonnet) | (none) |
| unit-test-evaluator | Evaluator subagent | eval/log |
| code-review-evaluator | Evaluator subagent | eval/log |
| integration-test-executor | Executor subagent (sonnet) | (none) |
| integration-test-evaluator | Evaluator subagent | eval/log |
| acceptance-evaluator | Evaluator subagent | eval/log |

The deprecated `requirements-evaluator` entry SHALL be removed from the table.

#### Scenario: proposal-evaluator uses eval/log with phase 01-proposal
- **WHEN** reading `agents/proposal-evaluator.md`
- **THEN** eval/log examples use phase `01-proposal`
- **AND** the checklist (R1-R10) is identical to the requirements-evaluator checklist

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | Tools | Contract |
|-------|-------|----------|
| proposal-planner | Read, Write, Grep, Glob, Bash | Writes proposal.md + specs/; accepts EXPLORE_CONTEXT_SUMMARY |
| proposal-evaluator | Read, eval/log | Same checklist as requirements-evaluator; phase `01-proposal` |
