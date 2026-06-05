---
name: workflow-requirement
description: |
  Full PGE workflow orchestrator — executes all 9 phases sequentially.
  No hardcoded phase knowledge. Uses phase_next for all orchestration decisions.
  Calls Agent(planner) → Bash(auto_steps) → Agent(evaluator) in a loop until done.
  On completion, notifies user to archive manually.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Full workflow orchestrator — executes all PGE phases via phase_next loop.

This skill does NOT contain any hardcoded phase table, agent name, or prompt.
Every phase, agent type, and prompt is returned by the phase_next MCP tool.

## Input:

The user's request should include a change name (kebab-case) OR a description of what they want to build.

## Steps

### Step 0: **If no clear input provided, ask what they want to build**

Use the **AskUserQuestion tool** (open-ended, no preset options) to ask:

> "What change do you want to work on? Describe what you want to build or fix."

From their description, derive a kebab-case name (e.g., "add user authentication" → `add-user-auth`).

**IMPORTANT**: Do NOT proceed without understanding what the user wants to build.

### Step 1: Create the change directory\*\*

```bash
openspec new change "<name>"
```

This creates a scaffolded change in the planning home resolved by the CLI with `.openspec.yaml`.

### Step 2: Orchestration loop

Enter the main execution loop. Each iteration calls phase_next, executes the returned
planner and evaluator agents, and reports progress.

```
LOOP:
  result = mcp__plugin_dev-team_dev-team__phase_next(change=<name>, workflow_type="requirement")

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
