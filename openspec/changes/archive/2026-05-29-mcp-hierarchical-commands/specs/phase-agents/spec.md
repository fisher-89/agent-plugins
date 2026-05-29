## MODIFIED Requirements

### Requirement: Subagent agent.md 承载领域知识
每个被精简技能调用的 subagent，其 agent.md SHALL 已承载原技能中硬编码的领域知识——包括模板路径、输入输出路径、完整 Process 描述。技能层不再传递这些细节，agent 从自己的 agent.md 获取。

Agent.md 中的 MCP tool 调用 SHALL 使用层级命名格式：
- `mcp__plugin_dev-team_dev-team__eval/log`（formerly `eval_log`）
- `mcp__plugin_dev-team_dev-team__eval/check`（formerly `eval_check`）
- `mcp__plugin_dev-team_dev-team__archi/query`（formerly `archi_query`）
- `mcp__plugin_dev-team_dev-team__archi/validate`（formerly `archi_validate`）
- `mcp__plugin_dev-team_dev-team__archi/write`（formerly `archi_write`）
- `mcp__plugin_dev-team_dev-team__archi/check`（formerly `archi_check`）

**作为 subagent 启动的 agent**:

| Agent | 角色 | MCP Tool 更新 |
|-------|------|--------------|
| architecture | Architecture agent | archi/query, archi/validate, archi/write, archi/check, eval/log |
| requirements-evaluator | Evaluator subagent | eval/log |
| dev-design-evaluator | Evaluator subagent | eval/log |
| test-design-evaluator | Evaluator subagent | eval/log |
| test-gen-evaluator | Evaluator subagent | eval/log |
| implementation-evaluator | Evaluator subagent | eval/log |
| unit-test-evaluator | Evaluator subagent | eval/log |
| code-review-evaluator | Evaluator subagent | eval/log |
| integration-test-evaluator | Evaluator subagent | eval/log |
| acceptance-evaluator | Evaluator subagent | eval/log |

#### Scenario: 每个 agent.md 使用层级 MCP tool 名称
- **WHEN** 检查以上 subagent 的 agent.md 文件
- **THEN** 所有 MCP tool 引用使用 `xx/yy` 层级格式（如 `mcp__plugin_dev-team_dev-team__eval/log`）
- **AND** 不存在旧格式引用（如 `eval_log`、`eval_check`、`archi_query`、`archi_validate`、`archi_write`、`archi_check`）

#### Scenario: 技能向 agent 传递的 prompt 中 tool 名称不变
- **WHEN** 技能调用 subagent
- **THEN** prompt 不直接包含 MCP tool 名称——tool 名称由 agent 的 agent.md 承载
- **AND** agent 从自己的 agent.md 获取正确的层级 MCP tool 名称

## ADDED Requirements

### Requirement: Agent files reference hierarchical MCP tool names
All agent agent.md files SHALL reference MCP tools using the hierarchical `xx/yy` format. No agent.md file SHALL contain references to deprecated flat-format names.

The following agent files SHALL be updated:

| Agent File | Old References | New References |
|------------|---------------|----------------|
| `agents/architecture.md` | `archi_query`, `archi_validate`, `archi_write`, `archi_check`, `eval_log` | `archi/query`, `archi/validate`, `archi/write`, `archi/check`, `eval/log` |
| `agents/requirements-evaluator.md` | `eval_log` | `eval/log` |
| `agents/dev-design-evaluator.md` | `eval_log` | `eval/log` |
| `agents/test-design-evaluator.md` | `eval_log` | `eval/log` |
| `agents/test-gen-evaluator.md` | `eval_log` | `eval/log` |
| `agents/implementation-evaluator.md` | `eval_log` | `eval/log` |
| `agents/unit-test-evaluator.md` | `eval_log` | `eval/log` |
| `agents/code-review-evaluator.md` | `eval_log` | `eval/log` |
| `agents/integration-test-evaluator.md` | `eval_log` | `eval/log` |
| `agents/acceptance-evaluator.md` | `eval_log` | `eval/log` |

#### Scenario: architecture agent uses hierarchical archi tool names
- **WHEN** reading `agents/architecture.md`
- **THEN** all archi tool references use `archi/query`, `archi/validate`, `archi/write`, `archi/check` format
- **AND** eval tool reference uses `eval/log`

#### Scenario: evaluator agents use eval/log
- **WHEN** reading any evaluator agent.md (e.g., `agents/requirements-evaluator.md`)
- **THEN** all eval tool references use `eval/log` format
- **AND** no references to `eval_log` remain

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | MCP Tools Used | Contract |
|-------|---------------|----------|
| architecture | archi/query, archi/validate, archi/write, archi/check, eval/log | All references use `xx/yy` format |
| *-evaluator (9 agents) | eval/log | References use `eval/log` format |

### Skill Files (`plugins/dev-team/skills/`)

| Skill | MCP Tools Used | Contract |
|-------|---------------|----------|
| phase-requirements | eval/check | Reference uses `eval/check` |
| phase-dev-design | eval/check | Reference uses `eval/check` |
| phase-test-design | eval/check | Reference uses `eval/check` |
| phase-test-gen | eval/check | Reference uses `eval/check` |
| phase-implement | eval/check | Reference uses `eval/check` |
| phase-unit-test | eval/log | Reference uses `eval/log` |
| phase-code-review | eval/check | Reference uses `eval/check` |
| phase-integration-test | eval/log | Reference uses `eval/log` |
| phase-acceptance | eval/check | Reference uses `eval/check` |
| openspec-archive-change | eval/check | Reference uses `eval/check` |
