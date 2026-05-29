## MODIFIED Requirements

### Requirement: Subagent agent.md 承载领域知识
每个被精简技能调用的 subagent，其 agent.md SHALL 已承载原技能中硬编码的领域知识——包括模板路径、输入输出路径、完整 Process 描述。技能层不再传递这些细节，agent 从自己的 agent.md 获取。

**作为 subagent 启动的 agent**:

| Agent | 角色 | 已承载内容 |
|-------|------|-----------|
| test-design-planner | Planner subagent | 模板路径、输入路径（proposal.md + design.md）、覆盖率映射格式、边界情况模板 |
| dev-design-planner | Planner subagent | 模板路径、输入路径（proposal.md + codebase）、architecture/data-flow/decisions 结构、tasks.md 格式 |
| requirements-evaluator | Evaluator subagent | 静态 checklist（R1-R9）、eval-log CLI 调用格式、输入路径 |
| test-design-evaluator | Evaluator subagent | 静态 checklist（T1-T9）、eval-log CLI 调用格式、交叉验证逻辑 |
| dev-design-evaluator | Evaluator subagent | 静态 checklist（D1-D9）、eval-log CLI 调用格式、tasks.md 验证 |
| implementation-generator | Generator subagent | 输入路径、task completion 指令（标记 `[x]`）、输出说明（git diff IS the artifact） |
| implementation-evaluator | Evaluator subagent | 静态 checklist（I1-I8）、eval-log CLI 调用格式、git diff 检查流程 |
| test-gen-generator | Generator subagent | 输入路径、输出路径 `openspec/changes/<name>/tests/`、文件类型约束 |
| test-gen-evaluator | Evaluator subagent | 静态 checklist（G1-G8）、eval-log CLI 调用格式、git diff 检查流程 |
| code-review-evaluator | Evaluator subagent | 静态 checklist（C1-C7）、backtrack_to 支持、安全模式搜索 |
| acceptance-evaluator | Evaluator subagent | 静态 checklist（A1-A5）、backtrack_to 支持、需求追踪逻辑 |

The agents `dev-proposal-planner` and `dev-proposal-evaluator` are RENAMED to `dev-design-planner` and `dev-design-evaluator` respectively. Their agent.md files SHALL be renamed accordingly:
- `agents/dev-proposal-planner.md` → `agents/dev-design-planner.md`
- `agents/dev-proposal-evaluator.md` → `agents/dev-design-evaluator.md`

#### Scenario: 每个 agent.md 包含输入输出路径
- **WHEN** 检查以上 subagent 的 agent.md 文件
- **THEN** 每个文件在 `## Input` 和 `## Output` 章节中包含了具体的文件路径（模板路径、输入输出路径）
- **AND** 路径使用 `<change-name>` 占位符而非硬编码的变更名称

#### Scenario: 技能不再传递路径信息给 agent
- **WHEN** 技能调用 subagent
- **THEN** prompt 中不包含模板路径、输出路径或 process 步骤描述
- **AND** agent 从自己的 agent.md 获取所有领域知识

## RENAMED Requirements

### RENAMED: dev-proposal-planner → dev-design-planner
- **FROM**: `dev-proposal-planner` — Reads proposal.md and test-design.md, writes design.md and tasks.md
- **TO**: `dev-design-planner` — Reads proposal.md and codebase (NOT test-design.md since it hasn't been generated yet), writes design.md and tasks.md

### RENAMED: dev-proposal-evaluator → dev-design-evaluator
- **FROM**: `dev-proposal-evaluator` — Evaluates design.md against checklist, appends to eval.json with phase "03-dev-proposal"
- **TO**: `dev-design-evaluator` — Evaluates design.md against checklist, appends to eval.json with phase "02-dev-design"

## ADDED Requirements

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
| dev-design-evaluator | 03-dev-proposal | 02-dev-design |
| test-design-evaluator | 02-test-design | 03-test-design |

Other evaluator agents (requirements-evaluator, implementation-evaluator, test-gen-evaluator, code-review-evaluator, acceptance-evaluator, unit-test-evaluator, integration-test-evaluator) SHALL keep their existing phase identifiers (01, 04, 05, 06, 07, 08, 09 unchanged).

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
| phase-dev-design | dev-design-planner | `"Write design.md and tasks.md for change '<name>'."` |
| phase-dev-design | dev-design-evaluator | `"Evaluate design.md and tasks.md for change '<name>'."` |

The `phase-test-design` skill SHALL continue to use `test-design-planner` and `test-design-evaluator` (unchanged agent names).

#### Scenario: phase-dev-design uses renamed agents
- **WHEN** reading `skills/phase-dev-design/SKILL.md`
- **THEN** Planner agent reference is `dev-design-planner` (not `dev-proposal-planner`)
- **AND** Evaluator agent reference is `dev-design-evaluator` (not `dev-proposal-evaluator`)
