---
name: phase-proposal
description: Proposal-planner writes proposal.md + specs/, then evaluator checks. Loops on fail.
disable-model-invocation: true
---

## Process

### Resolve the target change

Call `mcp__plugin_dev-team_dev-team__change_list()` to get active changes.

**Decision tree based on user input and change list:**

1. **User provided a parameter that exactly matches an existing change name** → use that change, skip to `### Confirm workflow type`.
2. **User provided a description (not an exact change name match)**:
   - If **no active changes exist** → treat as a new change. Derive a kebab-case name and proceed to `### Create the change directory`.
   - If **active changes exist**, judge whether the description semantically relates to an existing change (e.g., the description refines, extends, or refers to the same topic as an existing change name).
     - **Confident it matches an existing change** → use that change, skip to `### Confirm workflow type`.
     - **Confident it is unrelated to any existing change** → treat as a new change. Derive a kebab-case name and proceed to `### Create the change directory`.
     - **Uncertain** → use `AskUserQuestion` to present the potentially matching change(s) plus a "Create a new change" option. Let the user decide.
3. **No parameter provided AND exactly one active change exists** → auto-select that change, skip to `### Confirm workflow type`.
4. **No parameter provided AND multiple active changes exist** → use `AskUserQuestion` to present the list of active changes (plus an "Other — describe a new change" option). If the user picks an existing change, skip to `### Confirm workflow type`. If the user describes a new change, derive a kebab-case name and proceed to `### Create the change directory`.
5. **No parameter provided AND zero active changes exist** → use `AskUserQuestion` (open-ended, no preset options) to ask: "What change do you want to work on? Describe what you want to build or fix." Derive a kebab-case name from the response and proceed to `### Create the change directory`.

**IMPORTANT**: Do NOT proceed without a resolved change name.

### Create the change directory

Only reached when starting a **new** change (not resuming an existing one).

```bash
openspec new change "<name>"
```

### Confirm workflow type

Check if `openspec/changes/<name>/workflow.json` exists:

- **Already exists** → skip to `### Phase Check`.
- **Does not exist** → use `AskUserQuestion` to confirm:

  **Options:**
  - `requirement` — full development + test pipeline
  - `bug-fix` — simplified fix pipeline
  - `refactor` — full pipeline for refactoring
  - `test-only` — supplement tests only

  After confirmation, write `openspec/changes/<name>/workflow.json`:
  ```json
  {"workflow_type": "<choice>"}
  ```

### Phase Check

Call `mcp__plugin_dev-team_dev-team__phase_next(change=<change-name>)` to get workflow state. 

If `next_phase` is "proposal" continue to `### Run Executor`.

Otherwise, follow the table bellow:

| 条件 | 含义 | 处理 |
|---|---|---|
| `done == true` | 流程已完成 | 停止：报告异常，如需修改可开启新流程 |
| `allowed_backtrack_phases[].id have "proposal"` | 回溯至当前步骤 | 继续步骤 `**Backtrack**` |
| `last_result.verdict == "fail"` and `allowed_backtrack_phases[].id not have "proposal"` | 不支持回溯至当前步骤 | 停止：告知异常及支持回溯的步骤 |
| `last_result.verdict == "pass"` and `allowed_backtrack_phases[].id not have "proposal"` | 下一步不匹配 | 停止：告知异常及应该执行的步骤 `next_phase` |

**Backtrack**
```
mcp__plugin_dev-team_dev-team__backtrack({
  change: "<change-name>",
  phase: "<last_result.phase>",
  backtrack_to: "proposal",
  backtrack_reason: "用户手动执行回溯，推测原因：<Infer from `last_result.report`>"
})
```
If response `modified` is true, recall `mcp__plugin_dev-team_dev-team__phase_next(change=<name>)`, continue to `### Run Executor`.

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
result = mcp__plugin_dev-team_dev-team__phase_next(change=<change-name>)

if result.last_result is null:
  → 错误：Evaluator 未正确写入 eval.json，停止

if result.last_result.verdict == "pass" → continue to `### Report`

if result.last_result.verdict == "fail" → retry from `### Run Executor` or stop with error
```

### Report

Show verdict, pass/total, notes, and specs/ files list.
