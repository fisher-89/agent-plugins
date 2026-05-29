## MODIFIED Requirements

### Requirement: Nine user-triggered phase skills
The system SHALL provide 9 skills: `dev-team:phase-requirements`, `dev-team:phase-dev-design`, `dev-team:phase-test-design`, `dev-team:phase-test-gen`, `dev-team:phase-implement`, `dev-team:phase-unit-test`, `dev-team:phase-code-review`, `dev-team:phase-integration-test`, `dev-team:phase-acceptance`.

The skills SHALL call MCP tools using the hierarchical `xx/yy` format:
- `mcp__plugin_dev-team_dev-team__eval/check` (formerly `eval_check`)
- `mcp__plugin_dev-team_dev-team__eval/log` (formerly `eval_log`)

所有技能 SHALL 遵循精简编排器模式，通过 MCP tool `eval/check` 进行前置验证，使用一行 prompt 调用 agent。

#### Scenario: DESIGN skill executes Planner->Evaluator with eval/check gate
- **WHEN** user invokes `dev-team:phase-requirements`, `dev-team:phase-dev-design`, or `dev-team:phase-test-design`
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__eval/check` with `change` and `phase` arguments to validate prior phases, phase state, and backtrack markers
- **AND** if eval/check returns `passed: true`:
  - `phase-requirements`: the main agent (skill itself) directly writes proposal.md + specs/ to disk (no Planner subagent)
  - `phase-dev-design` / `phase-test-design`: invokes Planner subagent with one-line prompt (subagent reads full instructions from its own agent.md)
- **AND** then invokes the corresponding Evaluator subagent with a one-line prompt
- **AND** the skill reads the latest eval.json entry to determine verdict
- **AND** loops back to Planner if verdict is "fail" (up to max attempts)

#### Scenario: Skill 遵循精简编排器模式调用 eval/check
- **WHEN** 用户调用任意一个精简后的阶段技能
- **THEN** 在执行业务逻辑之前，技能先调用 `mcp__plugin_dev-team_dev-team__eval/check` with change="<name>" and phase="<phase-code>"
- **AND** 如果 eval/check 返回 `passed: false`，技能输出错误信息并停止
- **AND** Evaluator subagent 从自己的 agent.md 获取完整的 checklist 和 eval/log MCP tool 调用指令

#### Scenario: Skill 在 evaluator 完成后读取 eval.json 判断 verdict
- **WHEN** Evaluator subagent 执行完成
- **THEN** 技能读取 `openspec/changes/<name>/eval.json` 获取当前阶段的最新条目
- **AND** 如果 verdict 为 "fail"，重新执行业务逻辑（主 agent 或 subagent，最多 5 次尝试）
- **AND** 如果 verdict 为 "pass" 或 达到最大尝试次数，技能输出结果报告

## ADDED Requirements

### Requirement: phase skills use hierarchical MCP tool names
All phase skill SKILL.md files SHALL reference MCP tools using the hierarchical `xx/yy` format. No skill file SHALL contain references to the deprecated flat-format names (`eval_check`, `eval_log`).

The following files SHALL be updated:

| Skill File | Old Reference | New Reference |
|------------|---------------|---------------|
| `skills/phase-requirements/SKILL.md` | `eval_check` | `eval/check` |
| `skills/phase-dev-design/SKILL.md` | `eval_check` | `eval/check` |
| `skills/phase-test-design/SKILL.md` | `eval_check` | `eval/check` |
| `skills/phase-test-gen/SKILL.md` | `eval_check` | `eval/check` |
| `skills/phase-implement/SKILL.md` | `eval_check` | `eval/check` |
| `skills/phase-unit-test/SKILL.md` | `eval_log` | `eval/log` |
| `skills/phase-code-review/SKILL.md` | `eval_check` | `eval/check` |
| `skills/phase-integration-test/SKILL.md` | `eval_log` | `eval/log` |
| `skills/phase-acceptance/SKILL.md` | `eval_check` | `eval/check` |
| `skills/openspec-archive-change/SKILL.md` | `eval_check` | `eval/check` |

#### Scenario: phase-requirements gate check uses eval/check
- **WHEN** reading `skills/phase-requirements/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__eval/check` (not `eval_check`)

#### Scenario: phase-unit-test evaluator uses eval/log
- **WHEN** reading `skills/phase-unit-test/SKILL.md`
- **THEN** the file references `mcp__plugin_dev-team_dev-team__eval/log` (not `eval_log`)
