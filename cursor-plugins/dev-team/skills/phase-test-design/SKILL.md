---
name: phase-test-design
description: Test-design-planner writes test-design.md, then evaluator checks. Loops on fail.
disable-model-invocation: true
---

## Usage

```
dev-team:phase-test-design [change-name]
```

## Process

### Detect active change

Call `mcp__plugin_dev-team_dev-team__change_list` to get active changes. If <change-name> is provided, use it. Otherwise, select the only one change or prompt user to select.

### Phase Check

At the start of this turn, generate a new non-empty opaque `run_id` (e.g. UUID). Pass the same `run_id` to every `phase_next` call in this turn (Phase Check, backtrack recall, Verdict Phase Result).

Call `mcp__plugin_dev-team_dev-team__phase_next(change=<change-name>, run_id=<run_id>)` to get workflow state.

If `next_phase` is "test-design" continue to `### Run Executor`.

Otherwise, follow the table bellow:

| 条件 | 含义 | 处理 |
|---|---|---|
| `done == true` | 流程已完成 | 停止：报告异常，如需修改可开启新流程 |
| `allowed_backtrack_phases[].id have "test-design"` | 回溯至当前步骤 | 继续步骤 `**Backtrack**` |
| `last_result.verdict == "fail"` and `allowed_backtrack_phases[].id not have "test-design"` | 不支持回溯至当前步骤 | 停止：告知异常及支持回溯的步骤 |
| `last_result.verdict == "pass"` and `allowed_backtrack_phases[].id not have "test-design"` | 下一步不匹配 | 停止：告知异常及应该执行的步骤 `next_phase` |

**Backtrack**

```
mcp__plugin_dev-team_dev-team__backtrack({
  change: "<change-name>",
  phase: "<last_result.phase>",
  backtrack_to: "test-design",
  backtrack_reason: "用户手动执行回溯，推测原因：<Infer from `last_result.report`>"
})
```

If response `modified` is true, recall `mcp__plugin_dev-team_dev-team__phase_next(change=<name>, run_id=<run_id>)`, continue to `### Run Executor`.

### Run Executor

Call `Agent` with response of `phase_next`:

```
Agent({
  description: "Execute phase <next_phase>",
  subagent_type: executor.agent_type,
  prompt: executor.prompt
})
```

### Run Evaluator

Call `Agent` with response of `phase_next`:

```
Agent({
  description: "Evaluate phase <next_phase>",
  subagent_type: evaluator.agent_type,
  prompt: evaluator.prompt
})
```

### Verdict Phase Result

```
result = mcp__plugin_dev-team_dev-team__phase_next(change=<change-name>, run_id=<run_id>)

if result.last_result is null:
  → 错误：Evaluator 未正确写入 eval.json，停止

if result.last_result.verdict == "pass" → continue to `### Report`

if result.last_result.verdict == "fail" → retry from `### Run Executor` or stop with backtrack suggestion to one of `allowed_backtrack_phases[].id`
```

### Report

Show verdict, pass/total, and notes.
