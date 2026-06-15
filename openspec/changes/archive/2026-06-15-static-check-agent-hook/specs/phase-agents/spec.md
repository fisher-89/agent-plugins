## ADDED Requirements

### Requirement: implementation-generator 不再负责静态检查

`plugins/dev-team/agents/implementation-generator.md` SHALL 移除所有静态检查相关指令。具体包括：

- Process 步骤 7-8（调用 `config_get("static_analysis")` 及执行检查）
- Output 章节中 `reports/static_analysis.json` 报告生成要求及 JSON 模板
- frontmatter `description` 中对 AUTO static-check 的引用

Process 步骤 6（标记 tasks.md 完成）SHALL 成为最后一步。Generator 的核心职责 SHALL 仅为：读取 design/tasks/specs、编写实现代码、标记任务完成。

Generator SHALL NOT 调用 `config_get` 获取 `static_analysis` 配置。
Generator SHALL NOT 写入 `openspec/changes/<change-name>/reports/static_analysis.json`。

静态检查 SHALL 由 `subagentStop` hook（`static-check-hook`）在 agent 结束时自动执行。

#### Scenario: implementation-generator Process 不含静态检查步骤

- **WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md` 的 `## Process` 章节
- **THEN** 不包含 `config_get` 调用或 `static_analysis` 关键字
- **AND** 最后一步为标记 tasks.md 任务完成（原步骤 6）
- **AND** 不存在原步骤 7、8

#### Scenario: implementation-generator Output 不含 static_analysis 报告

- **WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md` 的 `## Output` 章节
- **THEN** 不包含 `reports/static_analysis.json` 路径
- **AND** 不包含静态检查 JSON 报告模板

#### Scenario: implementation-generator frontmatter 不引用 AUTO static-check

- **WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md` 的 frontmatter `description`
- **THEN** 不包含 `static-check` 或 `static_analysis` 相关描述

### Requirement: implementation-evaluator 移除 I7 静态检查项

`plugins/dev-team/agents/implementation-evaluator.md` SHALL 从 Static Checklist 表格中移除 I7 行：

> I7 | 静态检查通过（lint、类型检查） | 若config_get {"key":"static_analysis"} 可以获取检查脚本，检查有 `openspec/changes/<change-name>/reports/static_analysis.json`

移除 I7 后，evaluator 的判定规则 SHALL 仍使用 `"pass" only if ALL items pass"`，检查项范围为 I1-I6 及 I8（原 I8 保持不变）。

Evaluator SHALL NOT 读取 `reports/static_analysis.json` 或调用 `config_get` 验证静态检查结果。

#### Scenario: implementation-evaluator 检核表不含 I7

- **WHEN** 读取 `plugins/dev-team/agents/implementation-evaluator.md` 的 `## Static Checklist` 表格
- **THEN** 表格中不存在 ID 为 `I7` 的行
- **AND** 不包含 `static_analysis` 或 `reports/static_analysis.json` 关键字

#### Scenario: implementation-evaluator 判定范围更新

- **WHEN** 读取 `plugins/dev-team/agents/implementation-evaluator.md` 的判定规则
- **THEN** 规则表述为 `"pass" only if ALL items pass"`
- **AND** 检查项 ID 范围为 I1-I6 和 I8，不含 I7

## Module Contract

### Agent 文件变更

| Agent | 文件 | 变更 |
|-------|------|------|
| implementation-generator | `plugins/dev-team/agents/implementation-generator.md` | 移除步骤 7-8、报告生成、frontmatter static-check 引用 |
| implementation-evaluator | `plugins/dev-team/agents/implementation-evaluator.md` | 移除 I7 检查项 |

### 职责边界

| 组件 | 静态检查职责 |
|------|-------------|
| implementation-generator | 无 — 仅编写代码 |
| static-check-hook | 有 — subagentStop 时自动执行 |
| implementation-evaluator | 无 — 不再验证静态检查结果 |
