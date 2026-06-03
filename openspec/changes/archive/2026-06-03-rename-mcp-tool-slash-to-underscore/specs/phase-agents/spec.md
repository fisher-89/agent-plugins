## MODIFIED Requirements

### Requirement: Agent files reference underscore MCP tool names
All agent agent.md files SHALL reference MCP tools using the underscore `xx_yy` format. No agent.md file SHALL contain references to the deprecated slash-format names (`phase/check`, `phase/log`, `phase/next`, `archi/query`, `archi/validate`, `archi/write`, `archi/check`, `config/get`, `config/set`, `config/unset`, `config/context`).

The following agent files SHALL be updated:

| Agent File | Old References (xx/yy) | New References (xx_yy) |
|------------|------------------------|------------------------|
| `agents/architecture.md` | `archi/query`, `archi/validate`, `archi/write`, `archi/check`, `phase/log` | `archi_query`, `archi_validate`, `archi_write`, `archi_check`, `phase_log` |
| `agents/proposal-evaluator.md` | `phase/log` | `phase_log` |
| `agents/dev-design-evaluator.md` | `phase/log` | `phase_log` |
| `agents/test-design-evaluator.md` | `phase/log` | `phase_log` |
| `agents/test-gen-evaluator.md` | `phase/log` | `phase_log` |
| `agents/implementation-evaluator.md` | `phase/log` | `phase_log` |
| `agents/unit-test-evaluator.md` | `phase/log` | `phase_log` |
| `agents/code-review-evaluator.md` | `phase/log` | `phase_log` |
| `agents/integration-test-evaluator.md` | `phase/log` | `phase_log` |
| `agents/acceptance-evaluator.md` | `phase/log` | `phase_log` |

#### Scenario: architecture agent uses underscore archi tool names
- **WHEN** reading `agents/architecture.md`
- **THEN** all archi tool references use `archi_query`, `archi_validate`, `archi_write`, `archi_check` format
- **AND** eval tool reference uses `phase_log`

#### Scenario: evaluator agents use phase_log
- **WHEN** reading any evaluator agent.md
- **THEN** all eval tool references use `phase_log` format
- **AND** no references to `phase/log` remain

#### Scenario: no agent file contains slash-format MCP tool names
- **WHEN** grepping agent files for `mcp__plugin_dev-team_dev-team__.*/`
- **THEN** no matches are found

### Requirement: Subagent agent.md carries domain knowledge (tool name update)
The sub-agent table SHALL use the updated underscore MCP tool names:

| Agent | Role | MCP Tool |
|-------|------|----------|
| proposal-planner | Planner subagent | (none -- writes files directly) |
| proposal-evaluator | Evaluator subagent | phase_log |
| architecture | Architecture agent | archi_query, archi_validate, archi_write, archi_check, phase_log |
| dev-design-planner | Planner subagent | (none) |
| dev-design-evaluator | Evaluator subagent | phase_log |
| test-design-planner | Planner subagent | (none) |
| test-design-evaluator | Evaluator subagent | phase_log |
| test-gen-generator | Generator subagent | (none) |
| test-gen-evaluator | Evaluator subagent | phase_log |
| implementation-generator | Generator subagent | (none) |
| implementation-evaluator | Evaluator subagent | phase_log |
| unit-test-executor | Executor subagent (sonnet) | (none) |
| unit-test-evaluator | Evaluator subagent | phase_log |
| code-review-evaluator | Evaluator subagent | phase_log |
| integration-test-executor | Executor subagent (sonnet) | (none) |
| integration-test-evaluator | Evaluator subagent | phase_log |
| acceptance-evaluator | Evaluator subagent | phase_log |

#### Scenario: each agent.md uses underscore MCP tool names
- **WHEN** checking agent.md files for the subagents listed above
- **THEN** all MCP tool references use `xx_yy` underscore format (e.g., `mcp__plugin_dev-team_dev-team__phase_log`)
- **AND** no slash-format references remain (e.g., `phase/log`, `archi/query`)

#### Scenario: skill passes tool names to agents correctly
- **WHEN** a skill invokes a subagent
- **THEN** the prompt does NOT directly contain MCP tool names -- tool names are carried by the agent's agent.md
- **AND** the agent reads the correct underscore-format MCP tool names from its own agent.md

#### Scenario: proposal-evaluator uses phase_log with phase 01-proposal
- **WHEN** reading `agents/proposal-evaluator.md`
- **THEN** phase_log examples use phase `01-proposal`
- **AND** all references use `phase_log` (not `phase/log`)

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | Tools | Contract |
|-------|-------|----------|
| proposal-planner | Read, Write, Grep, Glob, Bash | Writes proposal.md + specs/; accepts EXPLORE_CONTEXT_SUMMARY |
| proposal-evaluator | Read, phase_log | Evaluates against checklist; phase `01-proposal`; 3-column checklist; verdict: ALL items pass |
| architecture | archi_query, archi_validate, archi_write, archi_check, phase_log | All references use `xx_yy` underscore format |
| dev-design-evaluator | Read, phase_log | Phase `02-dev-design`; 3-column checklist; D7/D9 mandatory; verdict: ALL items pass |
| test-design-evaluator | Read, phase_log | Phase `03-test-design`; 3-column checklist; T5/T8 mandatory; verdict: ALL items pass |
| test-gen-evaluator | Read, phase_log | 3-column checklist; verdict: ALL items pass |
| implementation-evaluator | Read, phase_log | 3-column checklist; verdict: ALL items pass |
| code-review-evaluator | Read, phase_log | 3-column checklist; verdict: ALL items pass (C1-C8) |
| acceptance-evaluator | Read, phase_log | 3-column checklist; A5 mandatory; verdict: ALL items pass (A1-A5) |
| unit-test-evaluator | Read, phase_log | Decision tree format (no static checklist) |
| integration-test-evaluator | Read, phase_log | Decision tree format (no static checklist) |
