---
name: workflow-requirement
description: |
  Full PGE workflow orchestrator — executes all 9 phases sequentially.
  No hardcoded phase knowledge. Uses phase/next for all orchestration decisions.
  Calls Agent(planner) → Bash(auto_steps) → Agent(evaluator) in a loop until done.
  On completion, notifies user to archive manually.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Full workflow orchestrator — executes all PGE phases via phase/next loop.

This skill does NOT contain any hardcoded phase table, agent name, or prompt.
Every phase, agent type, and prompt is returned by the phase/next MCP tool.

## Usage

```
/dev-team:workflow-requirement [change-name-or-description]
```

## Steps

### Step 0: Parse change name

Source `plugins/dev-team/utils/openspec-cli.sh`.

**With argument — classify by format:**

- **Arg is pure kebab-case** (`[a-z][a-z0-9-]*`): treat as existing change name → validate via `validate_change_name`. If not exists, `openspec_new_change`. Proceed to Step 2.

- **Arg is NOT kebab-case** (contains Chinese, spaces, or natural language): treat as change description → derive kebab-case via `derive_kebab_case`, scaffold via `openspec_new_change`. Handle conflicts with numeric suffix. Proceed to Step 2.

**Without argument:** Detect explore context (decision tables, diagrams, "What We Figured Out"). If found: extract decisions, ask user for kebab-case name, `derive_kebab_case`, confirm, scaffold. If not: ask "想构建什么变更？" derive kebab-case, confirm, scaffold. Handle conflicts with numeric suffix. Save explore context as EXPLORE_CONTEXT_SUMMARY.

### Step 1: Scaffold if needed

If the change directory does not exist, run `openspec_new_change "<name>"` to scaffold.

### Step 2: Orchestration loop

Enter the main execution loop. Each iteration calls phase/next, executes the returned
planner and evaluator agents, and reports progress.

```
LOOP:
  result = mcp__plugin_dev-team_dev-team__phase/next(change=<name>, workflow_type="requirement")

  if result.error:
    报告: "Workflow error [{result.error}]: {result.message}"
    PushNotification("Workflow {name} failed: {result.error}")
    STOP

  if result.done:
    跳转到 Step 3

  if result.planner:
    Agent({
      description: "Write artifacts for phase {result.next_phase}",
      subagent_type: result.planner.agent_type,
      prompt: result.planner.prompt
    })

  for step in result.auto_steps:
    Bash(step.command)

  if result.evaluator:
    Agent({
      description: "Evaluate artifacts for phase {result.next_phase}",
      subagent_type: result.evaluator.agent_type,
      prompt: result.evaluator.prompt
    })

  输出: "[Round {result.round}/20] [Phase {result.phase_index}/{result.total_phases}] {result.next_phase}: executed"
  PushNotification("Workflow {name}: Phase {result.next_phase} completed ({result.phase_index}/{result.total_phases})")

  继续 LOOP
```

### Step 3: Completion

All phases have passed evaluation.

1. 显示完成摘要:
   - Done: all phases passed
   - Total phases: {total_phases}
   - Total rounds: {round}

2. PushNotification("Workflow for change '{name}' completed. Please verify and run /dev-team:openspec-archive-change")

3. **Do NOT auto-archive.** The user must manually run `/dev-team:openspec-archive-change` after inspection.

4. 完成提示: "All phases completed. Please review the results and run `/dev-team:openspec-archive-change` to finalize."
