## ADDED Requirements

### Requirement: phase-requirements Planner 由主 agent 直接执行 + requirements-planner.md 删除
`phase-requirements` 的 Planner 工作（proposal.md + specs/）SHALL 由主 agent（skill 自身）直接执行，不通过 subagent。

- **Rationale**: proposal.md + specs/ 的创作受益于完整对话上下文（explore context、CLI instructions、用户偏好），主 agent 直接读写文件，避免通过 subagent prompt 转述上下文导致信息丢失。
- **删除**: `plugins/dev-team/agents/requirements-planner.md` 随本次变更删除，不再需要。
- **其他 DESIGN 阶段不变**: `test-design-planner` 和 `dev-proposal-planner` 保持为 subagent。
- **Evaluator 仍为 subagent**: checklist 评估是确定性工作，subagent 隔离确保评估独立于 Planner 的推理过程。

#### Scenario: phase-requirements 不启动 requirements-planner subagent
- **WHEN** 用户调用 `/dev-team:phase-requirements <name>`
- **THEN** 主 agent 直接读取 proposal.md.template、收集 CLI context、写入 proposal.md 和 specs/
- **AND** 不调用 `Agent({ subagent_type: "requirements-planner", ... })`

#### Scenario: requirements-planner.md 已被删除
- **WHEN** 检查 `plugins/dev-team/agents/requirements-planner.md` 文件
- **THEN** 该文件不存在

### Requirement: Subagent agent.md 承载领域知识
每个被精简技能调用的 subagent，其 agent.md SHALL 已承载原技能中硬编码的领域知识——包括模板路径、输入输出路径、完整 Process 描述。技能层不再传递这些细节，agent 从自己的 agent.md 获取。

**作为 subagent 启动的 agent**:

| Agent | 角色 | 已承载内容 |
|-------|------|-----------|
| test-design-planner | Planner subagent | 模板路径、输入路径、覆盖率映射格式、边界情况模板 |
| dev-proposal-planner | Planner subagent | 模板路径、输入路径、architecture/data-flow/decisions 结构、tasks.md 格式 |
| requirements-evaluator | Evaluator subagent | 静态 checklist（R1-R9）、eval-log CLI 调用格式、输入路径 |
| test-design-evaluator | Evaluator subagent | 静态 checklist（T1-T9）、eval-log CLI 调用格式、交叉验证逻辑 |
| dev-proposal-evaluator | Evaluator subagent | 静态 checklist（D1-D9）、eval-log CLI 调用格式、tasks.md 验证 |
| implementation-generator | Generator subagent | 输入路径、task completion 指令（标记 `[x]`）、输出说明（git diff IS the artifact） |
| implementation-evaluator | Evaluator subagent | 静态 checklist（I1-I8）、eval-log CLI 调用格式、git diff 检查流程 |
| test-gen-generator | Generator subagent | 输入路径、输出路径 `openspec/changes/<name>/tests/`、文件类型约束 |
| test-gen-evaluator | Evaluator subagent | 静态 checklist（G1-G8）、eval-log CLI 调用格式、git diff 检查流程 |
| code-review-evaluator | Evaluator subagent | 静态 checklist（C1-C7）、backtrack_to 支持、安全模式搜索 |
| acceptance-evaluator | Evaluator subagent | 静态 checklist（A1-A5）、backtrack_to 支持、需求追踪逻辑 |

#### Scenario: 每个 agent.md 包含输入输出路径
- **WHEN** 检查以上 subagent 的 agent.md 文件
- **THEN** 每个文件在 `## Input` 和 `## Output` 章节中包含了具体的文件路径（模板路径、输入输出路径）
- **AND** 路径使用 `<change-name>` 占位符而非硬编码的变更名称

#### Scenario: 技能不再传递路径信息给 agent
- **WHEN** 技能调用 subagent
- **THEN** prompt 中不包含模板路径、输出路径或 process 步骤描述
- **AND** agent 从自己的 agent.md 获取所有领域知识

### Requirement: Agent 使用 dev-team CLI 命令写入 eval.json
所有 evaluator agent（requirements-evaluator, test-design-evaluator, dev-proposal-evaluator, implementation-evaluator, test-gen-evaluator, code-review-evaluator, acceptance-evaluator, unit-test-evaluator, integration-test-evaluator）SHALL 通过 `dev-team eval-log --change <name> --phase <phase-code> ...` CLI 命令向 eval.json 追加条目，而非直接写入文件。

#### Scenario: Evaluator agent 使用 eval-log CLI
- **WHEN** evaluator agent 需要将评估结果写入 eval.json
- **THEN** 它执行 `dev-team eval-log --change <name> --phase <phase-code> --verdict pass|fail --report "<report>" --items '<items>'`
- **AND** 不使用直接的文件写入操作

## REMOVED Requirements

### Requirement: code-review 独立 agent
`plugins/dev-team/agents/code-review.md` 作为独立 subagent 已被删除。

- **Reason**: 该 agent 是废弃的 `/code-review` skill 的一部分，已被 PGE 工作流的 `phase-code-review` 阶段和 `code-review-evaluator` agent 完全取代
- **Migration**: 用户应使用 `/dev-team:phase-code-review` 替代 `/code-review`，该 skill 调用 `code-review-evaluator` agent 进行代码审查
