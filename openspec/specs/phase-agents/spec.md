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

### Requirement: dev-design-planner 输入不依赖 test-design
The `dev-design-planner` agent SHALL read `proposal.md` and the project codebase (CLAUDE.md, existing code patterns) as input. It SHALL NOT read `test-design.md` because the dev-design phase (02) runs before the test-design phase (03).

#### Scenario: dev-design-planner does not reference test-design.md
- **WHEN** reading `agents/dev-design-planner.md`
- **THEN** the `## Input` section does NOT list `test-design.md`
- **AND** the input section lists `proposal.md` and codebase context

### Requirement: test-design-planner 输入包含 design.md
The `test-design-planner` agent SHALL read `proposal.md` AND `design.md` as input. The `design.md` (produced by 02-dev-design) provides architecture context — components, data flow, route design — that informs test scope, coverage mapping, and boundary case identification.

#### Scenario: test-design-planner reads design.md
- **WHEN** reading `agents/test-design-planner.md`
- **THEN** the `## Input` section lists both `proposal.md` and `design.md`

### Requirement: Phase identifier updates in evaluator agents
All evaluator agents that reference phase identifiers in eval-log commands SHALL use the updated identifiers:

| Agent | Old Phase | New Phase |
|-------|-----------|-----------|
| proposal-evaluator | 01-requirements | 01-proposal |
| dev-design-evaluator | 03-dev-proposal | 02-dev-design |
| test-design-evaluator | 02-test-design | 03-test-design |

Other evaluator agents (implementation-evaluator, test-gen-evaluator, code-review-evaluator, acceptance-evaluator, unit-test-evaluator, integration-test-evaluator) SHALL keep their existing phase identifiers (04, 05, 06, 07, 08, 09 unchanged).

#### Scenario: proposal-evaluator uses 01-proposal
- **WHEN** reading `agents/proposal-evaluator.md`
- **THEN** eval-log examples use phase `01-proposal`
- **AND** the checklist (R1-R10) is identical to the requirements-evaluator checklist

#### Scenario: dev-design-evaluator uses 02-dev-design
- **WHEN** reading `agents/dev-design-evaluator.md`
- **THEN** eval-log examples use `--phase 02-dev-design`

#### Scenario: test-design-evaluator uses 03-test-design
- **WHEN** reading `agents/test-design-evaluator.md`
- **THEN** eval-log examples use `--phase 03-test-design`

### Requirement: Agent prompt table updated for renamed agents
The one-line prompt templates in phase skills SHALL reference the renamed agents:

| 技能 | Agent | Prompt |
|------|-------|--------|
| phase-proposal | proposal-planner | `"Write proposal.md and specs/ for change '<name>'."` |
| phase-proposal | proposal-evaluator | `"Evaluate proposal.md for change '<name>' against checklist."` |
| phase-dev-design | dev-design-planner | `"Write design.md and tasks.md for change '<name>'."` |
| phase-dev-design | dev-design-evaluator | `"Evaluate design.md and tasks.md for change '<name>'."` |

The `phase-test-design` skill SHALL continue to use `test-design-planner` and `test-design-evaluator` (unchanged agent names).

#### Scenario: phase-proposal uses proposal-planner and proposal-evaluator
- **WHEN** reading `skills/phase-proposal/SKILL.md`
- **THEN** Planner agent reference is `dev-team:proposal-planner`
- **AND** Evaluator agent reference is `dev-team:proposal-evaluator`

#### Scenario: phase-dev-design uses renamed agents
- **WHEN** reading `skills/phase-dev-design/SKILL.md`
- **THEN** Planner agent reference is `dev-team:dev-design-planner` (not `dev-proposal-planner`)
- **AND** Evaluator agent reference is `dev-team:dev-design-evaluator` (not `dev-proposal-evaluator`)

### Requirement: Agent files reference hierarchical MCP tool names
All agent agent.md files SHALL reference MCP tools using the hierarchical `xx/yy` format. No agent.md file SHALL contain references to deprecated flat-format names.

The following agent files SHALL be updated:

| Agent File | Old References | New References |
|------------|---------------|----------------|
| `agents/architecture.md` | `archi_query`, `archi_validate`, `archi_write`, `archi_check`, `eval_log` | `archi/query`, `archi/validate`, `archi/write`, `archi/check`, `phase/log` |
| `agents/proposal-evaluator.md` | `eval_log` | `phase/log` |
| `agents/dev-design-evaluator.md` | `eval_log` | `phase/log` |
| `agents/test-design-evaluator.md` | `eval_log` | `phase/log` |
| `agents/test-gen-evaluator.md` | `eval_log` | `phase/log` |
| `agents/implementation-evaluator.md` | `eval_log` | `phase/log` |
| `agents/unit-test-evaluator.md` | `eval_log` | `phase/log` |
| `agents/code-review-evaluator.md` | `eval_log` | `phase/log` |
| `agents/integration-test-evaluator.md` | `eval_log` | `phase/log` |
| `agents/acceptance-evaluator.md` | `eval_log` | `phase/log` |

#### Scenario: architecture agent uses hierarchical archi tool names
- **WHEN** reading `agents/architecture.md`
- **THEN** all archi tool references use `archi/query`, `archi/validate`, `archi/write`, `archi/check` format
- **AND** eval tool reference uses `phase/log`

#### Scenario: evaluator agents use phase/log
- **WHEN** reading any evaluator agent.md
- **THEN** all eval tool references use `phase/log` format
- **AND** no references to `eval_log` remain

## RENAMED Requirements

### RENAMED: requirements-evaluator → proposal-evaluator
- **FROM**: `requirements-evaluator` — Evaluates proposal.md against static binary checklist, appends to eval.json with phase `01-requirements`
- **TO**: `proposal-evaluator` — Evaluates proposal.md against the same static binary checklist, appends to eval.json with phase `01-proposal`

The agent file SHALL be renamed: `agents/requirements-evaluator.md` → `agents/proposal-evaluator.md`.

### RENAMED: dev-proposal-planner → dev-design-planner
- **FROM**: `dev-proposal-planner` — Reads proposal.md and test-design.md, writes design.md and tasks.md
- **TO**: `dev-design-planner` — Reads proposal.md and codebase (NOT test-design.md since it hasn't been generated yet), writes design.md and tasks.md

### RENAMED: dev-proposal-evaluator → dev-design-evaluator
- **FROM**: `dev-proposal-evaluator` — Evaluates design.md against checklist, appends to eval.json with phase "03-dev-proposal"
- **TO**: `dev-design-evaluator` — Evaluates design.md against checklist, appends to eval.json with phase "02-dev-design"

## MODIFIED Requirements

### Requirement: Subagent agent.md 承载领域知识
The sub-agent table SHALL include the new `proposal-planner` agent and reflect the renamed `proposal-evaluator`:

| Agent | 角色 | MCP Tool |
|-------|------|----------|
| proposal-planner | Planner subagent | (none — writes files directly) |
| proposal-evaluator | Evaluator subagent | phase/log |
| architecture | Architecture agent | archi/query, archi/validate, archi/write, archi/check, phase/log |
| dev-design-planner | Planner subagent | (none) |
| dev-design-evaluator | Evaluator subagent | phase/log |
| test-design-planner | Planner subagent | (none) |
| test-design-evaluator | Evaluator subagent | phase/log |
| test-gen-generator | Generator subagent | (none) |
| test-gen-evaluator | Evaluator subagent | phase/log |
| implementation-generator | Generator subagent | (none) |
| implementation-evaluator | Evaluator subagent | phase/log |
| unit-test-executor | Executor subagent (sonnet) | (none) |
| unit-test-evaluator | Evaluator subagent | phase/log |
| code-review-evaluator | Evaluator subagent | phase/log |
| integration-test-executor | Executor subagent (sonnet) | (none) |
| integration-test-evaluator | Evaluator subagent | phase/log |
| acceptance-evaluator | Evaluator subagent | phase/log |

The deprecated `requirements-evaluator` entry SHALL be removed from the table.

#### Scenario: 每个 agent.md 使用层级 MCP tool 名称
- **WHEN** 检查以上 subagent 的 agent.md 文件
- **THEN** 所有 MCP tool 引用使用 `xx/yy` 层级格式（如 `mcp__plugin_dev-team_dev-team__phase/log`）
- **AND** 不存在旧格式引用（如 `eval_log`、`eval_check`、`archi_query`、`archi_validate`、`archi_write`、`archi_check`）

#### Scenario: 技能向 agent 传递的 prompt 中 tool 名称不变
- **WHEN** 技能调用 subagent
- **THEN** prompt 不直接包含 MCP tool 名称——tool 名称由 agent 的 agent.md 承载
- **AND** agent 从自己的 agent.md 获取正确的层级 MCP tool 名称

#### Scenario: proposal-evaluator uses phase/log with phase 01-proposal
- **WHEN** reading `agents/proposal-evaluator.md`
- **THEN** phase/log examples use phase `01-proposal`
- **AND** the checklist (R1-R10) is identical to the requirements-evaluator checklist

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | Tools | Contract |
|-------|-------|----------|
| proposal-planner | Read, Write, Grep, Glob, Bash | Writes proposal.md + specs/; accepts EXPLORE_CONTEXT_SUMMARY |
| proposal-evaluator | Read, phase/log | Same checklist as requirements-evaluator; phase `01-proposal` |
| architecture | archi/query, archi/validate, archi/write, archi/check, phase/log | All references use `xx/yy` format |
| *-evaluator (other agents) | phase/log | References use `phase/log` format |

### Skill Files (`plugins/dev-team/skills/`)

| Skill | MCP Tools Used | Contract |
|-------|---------------|----------|
| phase-proposal | phase/check | P→E loop, phase `01-proposal` |
| phase-dev-design | phase/check | Reference uses `phase/check` |
| phase-test-design | phase/check | Reference uses `phase/check` |
| phase-test-gen | phase/check | Reference uses `phase/check` |
| phase-implement | phase/check | Reference uses `phase/check` |
| phase-unit-test | phase/log | Reference uses `phase/log` |
| phase-code-review | phase/check | Reference uses `phase/check` |
| phase-integration-test | phase/log | Reference uses `phase/log` |
| phase-acceptance | phase/check | Reference uses `phase/check` |
| workflow-requirement | phase/next, phase/log | Thin loop: call phase/next → invoke returned agents → repeat |
| openspec-archive-change | phase/check | Reference uses `phase/check` |
