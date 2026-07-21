---
name: phase-acceptance
description: |
  Acceptance-evaluator traces requirements from proposal.md through the codebase.
  No Planner, no Generator. Runs once. Backtrack routing decisions made by skill.
disable-model-invocation: true
---

## Usage

```
/dev-team:phase-acceptance [change-name]
```

## Process

### Detect active change

Call `mcp__plugin_dev-team_dev-team__change_list` to get active changes. If <change-name> is provided, use it. Otherwise, select the only one change or prompt user to select.

### Phase Check

Call `mcp__plugin_dev-team_dev-team__phase_next(change=<change-name>)` to get workflow state. 

If `next_phase` is "acceptance" continue to `### Run Evaluator`.

Otherwise, follow the table bellow:

| 条件 | 含义 | 处理 |
|---|---|---|
| `done == true` | 流程已完成 | 停止：报告异常，如需修改可开启新流程 |
| `allowed_backtrack_phases[].id have "acceptance"` | 回溯至当前步骤 | 继续步骤 `**Backtrack**` |
| `last_result.verdict == "fail"` and `allowed_backtrack_phases[].id not have "acceptance"` | 不支持回溯至当前步骤 | 停止：告知异常及支持回溯的步骤 |
| `last_result.verdict == "pass"` and `allowed_backtrack_phases[].id not have "acceptance"` | 下一步不匹配 | 停止：告知异常及应该执行的步骤 `next_phase` |

**Backtrack**
```
mcp__plugin_dev-team_dev-team__backtrack({
  change: "<change-name>",
  phase: "<last_result.phase>",
  backtrack_to: "acceptance",
  backtrack_reason: "用户手动执行回溯，推测原因：<Infer from `last_result.report`>"
})
```
If response `modified` is true, recall `mcp__plugin_dev-team_dev-team__phase_next(change=<name>)`, continue to `### Run Evaluator`.

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
result = mcp__plugin_dev-team_dev-team__phase_next(change=<change-name>)

if result.done == true → continue to `### Report`

if result.done == false and result.last_result is null:
  → 错误：Evaluator 未正确写入 eval.json，停止

if result.last_result.verdict == "fail" → stop with backtrack suggestion to one of `allowed_backtrack_phases[].id`
```

### Report

Show verdict, pass/total, and notes.
