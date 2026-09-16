# phase-agents Specification

## Purpose

Phase agent 文件内容。本变更要求 evaluator 经 `phase_log` 把结果写入 `workflow.json.eval`，禁止把独立 `eval.json` 当作写入目标。

## ADDED Requirements

### Requirement: Evaluator persists via phase_log into workflow.json

所有 evaluator / 会调用 `phase_log` 的 executor（含 `test-execution-evaluator`、`test-execution-executor` 的 skip 路径，以及 proposal…acceptance、code-analyze evaluator）SHALL：

- 使用 MCP `phase_log` 追加评估结果
- 将持久化目标描述为 change 的 `workflow.json`（字段 `eval`）
- MUST NOT 指示使用 Write/Edit/Bash 创建或修改 `eval.json` 或 `workflow.json`

标题或步骤名若仍写「Append to eval.json」，SHALL 改为指向 `phase_log` / `workflow.json`。

#### Scenario: test-execution-evaluator 写入说明

- **WHEN** 读取 `plugins/dev-team/agents/test-execution-evaluator.md` 的 phase_log 步骤
- **THEN** 该步骤要求调用 `phase_log` 且提及 `workflow.json`（或等价「工作流元数据中的评估记录」）
- **AND** SHALL NOT 要求直接写文件 `eval.json`

#### Scenario: evaluator 仍不设 backtrack_to

- **WHEN** evaluator 判定 fail 并调用 `phase_log`
- **THEN** 条目无 `backtrack_to`（不变，见 `pipeline-backtrack`）

## Module Contract

| 文件 | 变更 |
|------|------|
| `plugins/dev-team/agents/*-evaluator.md` | 持久化文案：`phase_log` → `workflow.json` |
| `plugins/dev-team/agents/test-execution-executor.md` | skip 时 `phase_log` 说明同步 |
