## MODIFIED Requirements

### Requirement: Nine user-triggered phase skills
The system SHALL provide 9 skills: `dev-team:phase-requirements`, `dev-team:phase-test-design`, `dev-team:phase-dev-proposal`, `dev-team:phase-test-gen`, `dev-team:phase-implement`, `dev-team:phase-unit-test`, `dev-team:phase-code-review`, `dev-team:phase-integration-test`, `dev-team:phase-acceptance`. 所有技能 SHALL 遵循精简编排器模式，通过 `dev-team eval-check --change <name> --phase <phase-code>` 进行前置验证，使用一行 prompt 调用 agent。

#### Scenario: DESIGN skill executes Planner->Evaluator with eval-check gate
- **WHEN** user invokes `dev-team:phase-requirements`, `dev-team:phase-test-design`, or `dev-team:phase-dev-proposal`
- **THEN** the skill first calls `dev-team eval-check --change "<name>" --phase "<phase-code>"` to validate prior phases, phase state, and backtrack markers
- **AND** if eval-check passes (exit code 0):
  - `phase-requirements`: the main agent (skill itself) directly writes proposal.md + specs/ to disk (no Planner subagent)
  - `phase-test-design` / `phase-dev-proposal`: invokes Planner subagent with one-line prompt (subagent reads full instructions from its own agent.md)
- **AND** then invokes the corresponding Evaluator subagent with a one-line prompt
- **AND** the skill reads the latest eval.json entry to determine verdict
- **AND** loops back to Planner if verdict is "fail" (up to max attempts)

#### Scenario: EXECUTION skill invokes Generator->Evaluator with eval-check gate
- **WHEN** user invokes `dev-team:phase-test-gen`
- **THEN** the skill first calls `dev-team eval-check --change "<name>" --phase "<phase-code>"` for prior phase validation
- **AND** if eval-check passes, invokes test-gen-generator with one-line prompt
- **AND** then invokes test-gen-evaluator with one-line prompt
- **AND** verifies verdict from eval.json and loops on failure (up to max attempts)

#### Scenario: Implementation skill invokes Generator->AUTO->Evaluator with eval-check gate
- **WHEN** user invokes `dev-team:phase-implement`
- **THEN** the skill first calls `dev-team eval-check --change "<name>" --phase "05-implement"` for prior phase validation
- **AND** if eval-check passes, invokes implementation-generator with one-line prompt "Implement pending tasks for change '<name>'."
- **AND** after Generator completes, runs AUTO static-check via `python plugins/dev-team/utils/lint-runner.py --change "<name>" --project-root . --save-report`
- **AND** if static-check fails, loops back to Generator with static-check failure details
- **AND** if static-check passes, invokes implementation-evaluator with one-line prompt "Evaluate implementation for change '<name>'."
- **AND** verifies verdict from eval.json and loops on failure (up to max attempts)
- **AND** no unit-test sub-step runs inside the implement phase

#### Scenario: Test execution skill triggers Executor->Evaluator (report generation)
- **WHEN** user invokes `dev-team:phase-unit-test` or `dev-team:phase-integration-test`
- **THEN** the skill invokes the Executor agent (sonnet, runs tests and writes a structured report), then the Evaluator agent (reads the report and applies diagnostic checks) in sequence

#### Scenario: EVALUATOR-ONLY skill invokes Evaluator directly with eval-check gate
- **WHEN** user invokes `dev-team:phase-code-review` or `dev-team:phase-acceptance`
- **THEN** the skill first calls `dev-team eval-check --change "<name>" --phase "<phase-code>"` for prior phase validation
- **AND** if eval-check passes, invokes ONLY the corresponding Evaluator agent with a one-line prompt (no Planner, no Generator, no Executor)
- **AND** the Evaluator inspects the codebase/diffs/reports directly and appends result to eval.json
- **AND** the skill reads the verdict from eval.json and reports the result (runs once, no automatic loop)

### Requirement: code-review invokes single evaluator
`dev-team:phase-code-review` SHALL invoke a single evaluator: code-review-evaluator only.
The integration-test-execution-evaluator is NO LONGER part of code-review phase — it has been moved to its own standalone phase skill (`dev-team:phase-integration-test`).
The code-review evaluator output SHALL be appended to eval.json as a single entry.

#### Scenario: code-review runs code-review evaluator only
- **WHEN** user invokes `dev-team:phase-code-review`
- **THEN** the skill invokes code-review-evaluator only
- **AND** eval.json contains exactly one entry with phase "07-code-review"

#### Scenario: code-review does not trigger integration tests
- **WHEN** user invokes `dev-team:phase-code-review`
- **THEN** no integration-test execution is triggered by this skill
- **AND** integration tests are handled separately by `dev-team:phase-integration-test`

## ADDED Requirements

### Requirement: Slim orchestrator pattern for phase skills
所有 7 个阶段技能（phase-requirements, phase-test-design, phase-dev-proposal, phase-implement, phase-test-gen, phase-code-review, phase-acceptance）SHALL 采用统一的精简编排器模式，遵循以下结构：

```
1. Usage 说明（不变）
2. Change name 解析（保留在 Skill，依赖对话历史）
3. eval-check CLI gate:
   dev-team eval-check --change <name> --phase <phase-code>
   （替代手动 eval.json 解析、backtrack 检测、prior phase 验证）
4. 执行阶段:
   - phase-requirements: 主 agent 直接写 proposal.md + specs/（不启动 Planner subagent）
   - 其他 DESIGN: Planner subagent 写文件（一行 prompt）
   - EXECUTION: Generator subagent 写代码（一行 prompt）
   - 所有阶段: Evaluator subagent 评估（一行 prompt）
5. Verdict loop：读取 eval.json 最新条目，判断 pass/fail
6. 结果报告
```

每个技能 SHALL NOT 超过 60 行（包括 YAML frontmatter），SHALL NOT 包含 Phase Pattern 文档段落。

#### Scenario: phase-requirements 使用主 agent 直接执行 Planner 工作
- **WHEN** 用户调用 `phase-requirements`
- **THEN** 技能先执行 `dev-team eval-check --change "<name>" --phase "01-requirements"` 进行门控
- **AND** 如果 eval-check 通过，主 agent（skill 自身）直接读取 proposal.md.template、收集 CLI context、写入 proposal.md 和 specs/ 到磁盘
- **AND** 不启动 `requirements-planner` subagent

#### Scenario: 其他 DESIGN skill 使用 Planner subagent
- **WHEN** 用户调用 `phase-test-design` 或 `phase-dev-proposal`
- **THEN** Planner 保持为 subagent（一行 prompt），从自己的 agent.md 获取完整指令
- **AND** 行为与精简前一致，仅 prompt 极简化

#### Scenario: EXECUTION skill 使用 Generator subagent 写代码
- **WHEN** 用户调用 EXECUTION 类阶段技能（phase-implement, phase-test-gen）
- **THEN** Generator 保持为 subagent（需要隔离的文件系统操作和独立的 sonnet 模型配置）
- **AND** skill 以一行 prompt 调用 Generator subagent

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

#### Scenario: Skill 不包含 Phase Pattern 文档段落
- **WHEN** 检查精简后的技能文件
- **THEN** 文件末尾不存在 `## DESIGN Phase Pattern`、`## EXECUTION Phase Pattern` 或 `## EVALUATOR-ONLY Pattern` 章节
- **AND** 所有非执行逻辑的文档内容已被移除

### Requirement: Agent prompt 简化为一行（仅 Evaluator 和 Generator subagent）
每个精简后的技能传递给 subagent 的 prompt SHALL 为一行字符串，不包含模板路径、输出路径、详细 process 描述。Subagent 从自己的 agent.md 文件获取完整的 Input/Process/Output/Constraints。

**注意**: `phase-requirements` 的 Planner 已由主 agent 直接执行，不再通过 subagent。其他 DESIGN 阶段的 Planner subagent（test-design-planner, dev-proposal-planner）保持不变。

各技能的 prompt 模板：

| 技能 | Agent | Prompt |
|------|-------|--------|
| phase-requirements | requirements-evaluator | `"Evaluate proposal.md for change '<name>'."` |
| phase-test-design | test-design-planner | `"Write test-design.md for change '<name>'."` |
| phase-test-design | test-design-evaluator | `"Evaluate test-design.md for change '<name>'."` |
| phase-dev-proposal | dev-proposal-planner | `"Write design.md and tasks.md for change '<name>'."` |
| phase-dev-proposal | dev-proposal-evaluator | `"Evaluate design.md and tasks.md for change '<name>'."` |
| phase-implement | implementation-generator | `"Implement pending tasks for change '<name>'."` |
| phase-implement | implementation-evaluator | `"Evaluate implementation for change '<name>'."` |
| phase-test-gen | test-gen-generator | `"Generate test skeleton files for change '<name>'."` |
| phase-test-gen | test-gen-evaluator | `"Evaluate generated test code for change '<name>'."` |
| phase-code-review | code-review-evaluator | `"Review code changes for change '<name>'."` |
| phase-acceptance | acceptance-evaluator | `"Perform acceptance evaluation for change '<name>'."` |

#### Scenario: 技能传递一行 prompt 给 agent
- **WHEN** 技能调用 Agent 工具
- **WHEN** prompt 字段不是一行字符串（包含换行符或多行内容）
- **THEN** 评估器 SHALL 记录违反项

#### Scenario: 技能不传递模板路径给 agent
- **WHEN** 技能调用 test-design-planner agent
- **THEN** prompt 中不包含 `plugins/dev-team/templates/artifacts/test-design.md.template`
- **AND** agent 从自己的 agent.md 获取模板路径

#### Scenario: 技能不传递输出路径给 agent
- **WHEN** 技能调用 dev-proposal-planner agent
- **THEN** prompt 中不包含 `openspec/changes/<name>/design.md`
- **AND** agent 从自己的 agent.md 获取输出路径

### Requirement: 移除 eval-check.py，统一使用 TypeScript CLI
`plugins/dev-team/utils/eval-check.py` SHALL 被删除。所有 eval-check 调用 SHALL 统一使用 TypeScript CLI `dev-team eval-check --change <name> --phase <phase-code>`。

`plugins/dev-team/skills/openspec-archive-change/SKILL.md` 中的 eval-check 引用 SHALL 更新为使用 CLI：
```bash
# 旧: python plugins/dev-team/utils/eval-check.py --change "<name>" --project-root . --json
# 新: dev-team eval-check --change <name> --phase 09-acceptance --json
```

#### Scenario: eval-check.py 已被删除
- **WHEN** 检查 `plugins/dev-team/utils/eval-check.py` 文件
- **THEN** 该文件不存在

#### Scenario: openspec-archive-change 使用 CLI 而非 Python 脚本
- **WHEN** grep 搜索 `plugins/dev-team/skills/openspec-archive-change/SKILL.md`
- **THEN** eval-check 调用使用 `dev-team eval-check` 而非 `python plugins/dev-team/utils/eval-check.py`

### Requirement: Phase code 修正 implementation-evaluator.md 中的阶段代码
`plugins/dev-team/agents/implementation-evaluator.md` 中的 eval-log 示例 SHALL 使用 `05-implement` 而非 `05-implementation`。

#### Scenario: implementation-evaluator.md 使用 05-implement
- **WHEN** grep 搜索 `plugins/dev-team/agents/implementation-evaluator.md`
- **THEN** eval-log 命令行示例使用 `--phase 05-implement` 而非 `--phase 05-implementation`

### Requirement: 废弃 code-review skill 和 agent 删除
`plugins/dev-team/skills/code-review/` 目录（含 SKILL.md）SHALL 被删除。`plugins/dev-team/agents/code-review.md` SHALL 被删除。不再提供 `/code-review` 命令入口。

#### Scenario: code-review skill 目录不存在
- **WHEN** 检查 `plugins/dev-team/skills/code-review/` 目录
- **THEN** 该目录不存在（或已完全删除）

#### Scenario: code-review.md agent 不存在
- **WHEN** 检查 `plugins/dev-team/agents/code-review.md` 文件
- **THEN** 该文件不存在

### Requirement: phase-implement 保留 AUTO 阶段 shell 命令
`phase-implement` 技能的 AUTO 阶段（lint/type check 静态检查）SHALL 保留在技能编排层，不移动到 subagent。技能在 implementation-generator 完成后 SHALL 直接执行 `python plugins/dev-team/utils/lint-runner.py --change "<name>" --project-root . --save-report`。

#### Scenario: phase-implement 技能包含 lint-runner 调用
- **WHEN** 检查 `plugins/dev-team/skills/phase-implement/SKILL.md`
- **THEN** 文件包含 `lint-runner.py` 命令的显式调用
- **AND** 该调用在 Generator agent 调用之后、Evaluator agent 调用之前

### Requirement: phase-requirements 保留 explore context 检测
`phase-requirements` 技能 SHALL 保留交互式 change name 解析逻辑，包括 explore context 检测的 Branch A/B 决策树。

#### Scenario: phase-requirements 包含 Branch A 逻辑
- **WHEN** 用户调用 `/dev-team:phase-requirements <name>` 并带参数
- **THEN** 技能验证 change name 格式、检查 change_exists、继续到 Step 2

#### Scenario: phase-requirements 包含 Branch B 逻辑
- **WHEN** 用户调用 `/dev-team:phase-requirements` 不带参数
- **THEN** 技能扫描对话历史检测 explore session 标记
- **AND** 根据是否检测到 explore context 分别走 Branch B1 或 Branch B2

### Requirement: Unit-test phase skill triggers executor then evaluator
`dev-team:phase-unit-test` SHALL invoke two agents sequentially: first unit-test-executor (sonnet), then unit-test-evaluator.
The skill SHALL check if unit test files exist before invoking the executor. If no test files exist, the skill SHALL append a skipped entry to eval.json with `skipped: true` and proceed.
The executor SHALL use the sonnet model.
The evaluator SHALL use the opus model for diagnostic decision making.

#### Scenario: Unit-test executor runs and evaluator validates
- **WHEN** user invokes `dev-team:phase-unit-test` and unit test files exist
- **THEN** the skill invokes unit-test-executor (sonnet), waits for the structured report to be written to `reports/unit-test-execution.json`
- **AND** then invokes unit-test-evaluator to validate the report and apply diagnostic decision tree
- **AND** eval.json contains two entries with phase "06-unit-test" and different phase_suffix values ("executor", "evaluator")

#### Scenario: Unit-test phase skipped (no test files)
- **WHEN** user invokes `dev-team:phase-unit-test` and no test files exist (Glob for `**/*.test.*` returns empty)
- **THEN** the skill does NOT invoke executor or evaluator
- **AND** appends a skipped entry with `skipped: true` and verdict "pass"

### Requirement: Integration-test phase skill triggers executor then evaluator
`dev-team:phase-integration-test` SHALL invoke two agents sequentially: first integration-test-executor (sonnet), then integration-test-evaluator.
The skill SHALL check if integration test files exist before invoking the executor. If no integration test files exist, the skill SHALL append a skipped entry to eval.json with `skipped: true` and proceed.
The executor SHALL use the sonnet model.

#### Scenario: Integration-test executor runs and evaluator validates
- **WHEN** user invokes `dev-team:phase-integration-test` and integration test files exist
- **THEN** the skill invokes integration-test-executor (sonnet), waits for the structured report to be written to `reports/integration-test-execution.json`
- **AND** then invokes integration-test-evaluator to validate the report and apply diagnostic decision tree
- **AND** eval.json contains two entries with phase "08-integration-test" and different phase_suffix values ("executor", "evaluator")

#### Scenario: Integration-test phase skipped (no integration test files)
- **WHEN** user invokes `dev-team:phase-integration-test` and no integration test files exist (Glob for `**/*.integration.test.*` returns empty)
- **THEN** the skill does NOT invoke executor or evaluator
- **AND** appends a skipped entry with `skipped: true` and verdict "pass"
