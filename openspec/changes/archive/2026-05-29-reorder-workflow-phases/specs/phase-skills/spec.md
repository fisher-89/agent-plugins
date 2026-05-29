## MODIFIED Requirements

### Requirement: Nine user-triggered phase skills
The system SHALL provide 9 skills: `dev-team:phase-requirements`, `dev-team:phase-dev-design`, `dev-team:phase-test-design`, `dev-team:phase-test-gen`, `dev-team:phase-implement`, `dev-team:phase-unit-test`, `dev-team:phase-code-review`, `dev-team:phase-integration-test`, `dev-team:phase-acceptance`.

The skill `dev-team:phase-dev-proposal` is RENAMED to `dev-team:phase-dev-design`. Its directory SHALL be `skills/phase-dev-design/` (formerly `skills/phase-dev-proposal/`).

所有技能 SHALL 遵循精简编排器模式，通过 `dev-team eval-check --change <name> --phase <phase-code>` 进行前置验证，使用一行 prompt 调用 agent。

#### Scenario: DESIGN skill executes Planner->Evaluator with eval-check gate
- **WHEN** user invokes `dev-team:phase-requirements`, `dev-team:phase-dev-design`, or `dev-team:phase-test-design`
- **THEN** the skill first calls `dev-team eval-check --change "<name>" --phase "<phase-code>"` to validate prior phases, phase state, and backtrack markers
- **AND** if eval-check passes (exit code 0):
  - `phase-requirements`: the main agent (skill itself) directly writes proposal.md + specs/ to disk (no Planner subagent)
  - `phase-dev-design` / `phase-test-design`: invokes Planner subagent with one-line prompt (subagent reads full instructions from its own agent.md)
- **AND** then invokes the corresponding Evaluator subagent with a one-line prompt
- **AND** the skill reads the latest eval.json entry to determine verdict
- **AND** loops back to Planner if verdict is "fail" (up to max attempts)

#### Scenario: Skill 遵循精简编排器模式调用 eval-check
- **WHEN** 用户调用任意一个精简后的阶段技能
- **THEN** 在执行业务逻辑之前，技能先执行 `dev-team eval-check --change "<name>" --phase "<phase-code>"`
- **AND** 如果 eval-check 退出码非 0，技能输出错误信息并停止
- **AND** Evaluator subagent 使用一行 prompt（subagent 从自己的 agent.md 获取完整的 checklist 和 eval-log CLI 指令）

#### Scenario: Skill 在 evaluator 完成后读取 eval.json 判断 verdict
- **WHEN** Evaluator subagent 执行完成
- **THEN** 技能读取 `openspec/changes/<name>/eval.json` 获取当前阶段的最新条目
- **AND** 如果 verdict 为 "fail"，重新执行业务逻辑（主 agent 或 subagent，最多 5 次尝试）
- **AND** 如果 verdict 为 "pass" 或 达到最大尝试次数，技能输出结果报告

## REMOVED Requirements

### Requirement: phase-dev-proposal skill
**Reason**: 重命名为 phase-dev-design，职责不变（产出 design.md + tasks.md）。
**Migration**: 用户改用 `/dev-team:phase-dev-design` 替代 `/dev-team:phase-dev-proposal`。目录 `skills/phase-dev-proposal/` 重命名为 `skills/phase-dev-design/`。

## ADDED Requirements

### Requirement: phase-dev-design skill
The system SHALL provide `dev-team:phase-dev-design` skill at `skills/phase-dev-design/SKILL.md` with name `phase-dev-design` and gate check phase `02-dev-design`.

The skill SHALL invoke dev-design-planner (writes design.md + tasks.md) and dev-design-evaluator (evaluates against proposal.md) in a P→E loop.

Gate check: `dev-team eval-check --change "<name>" --phase 02-dev-design`

#### Scenario: phase-dev-design gate check
- **WHEN** user invokes `/dev-team:phase-dev-design <name>`
- **THEN** the skill runs `dev-team eval-check --change "<name>" --phase 02-dev-design`
- **AND** requires prior phase [01-requirements] to have pass record

### Requirement: phase-test-design uses updated phase code
The system SHALL use `03-test-design` (formerly `02-test-design`) as the phase identifier for the test-design phase.

Gate check: `dev-team eval-check --change "<name>" --phase 03-test-design`

The test-design-planner SHALL read `design.md` (produced by 02-dev-design) in addition to `proposal.md` as input for determining test scope and strategy.

#### Scenario: phase-test-design gate check
- **WHEN** user invokes `/dev-team:phase-test-design <name>`
- **THEN** the skill runs `dev-team eval-check --change "<name>" --phase 03-test-design`
- **AND** requires prior phases [01-requirements, 02-dev-design] to have pass records

#### Scenario: test-design-planner reads design.md
- **WHEN** test-design-planner is invoked
- **THEN** it reads `openspec/changes/<name>/design.md` as input for architecture context
- **AND** generates test scope and strategy informed by the design's architecture, data flow, and route design
