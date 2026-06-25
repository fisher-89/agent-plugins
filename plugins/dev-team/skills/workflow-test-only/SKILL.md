---
name: workflow-test-only
description: |
  Test-only PGE workflow orchestrator — executes 6 phases for supplementing test coverage on existing code.
  No hardcoded phase knowledge. Uses phase_next for all orchestration decisions.
  On code bug discovery, writes a report and asks user to continue or terminate.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

**Input**: Optionally specify a change name (kebab-case), OR a description of what the user wants to build. If omitted, check if it can be inferred from conversation context.

## Steps

### Step 0: Resolve the target change

Call `mcp__plugin_dev-team_dev-team__change_list()` to get active changes.

**Decision tree based on user input and change list:**

1. **User provided a parameter that exactly matches an existing change name** → use that change, skip to Step 2.
2. **User or context provided a description (not an exact change name match)**:
   - If **no active changes exist** → treat as a new change. Derive a kebab-case name and proceed to Step 1.
   - If **active changes exist**, judge whether the description semantically relates to an existing change (e.g., the description refines, extends, or refers to the same topic as an existing change name).
     - **Confident it matches an existing change** → use that change, skip to Step 2.
     - **Confident it is unrelated to any existing change** → treat as a new change. Derive a kebab-case name and proceed to Step 1.
     - **Uncertain** → use AskQuestion to present the potentially matching change(s) plus a "Create a new change" option. Let the user decide.
3. **No parameter provided AND exactly one active change exists** → auto-select that change, skip to Step 2.
4. **No parameter provided AND multiple active changes exist** → use AskQuestion to present the list of active changes (plus an "Other — describe a new change" option). If the user picks an existing change, skip to Step 2. If the user describes a new change, derive a kebab-case name and proceed to Step 1.
5. **No parameter provided AND zero active changes exist** → use AskQuestion (open-ended, no preset options) to ask: "What change do you want to work on? Describe the test coverage you want to add or fix." Derive a kebab-case name from the response and proceed to Step 1.

**IMPORTANT**: Do NOT proceed without a resolved change name.

### Step 1: Create the change directory

Only reached when starting a **new** change (not resuming an existing one).

```bash
openspec new change "<name>"
```

This creates a scaffolded change in the planning home resolved by the CLI with `.openspec.yaml`.

Write `openspec/changes/<name>/workflow.json`:

```json
{"workflow_type": "test-only"}
```

### Step 2: Orchestration loop

Enter the main execution loop. Each iteration calls phase_next, executes the returned
planner and evaluator agents, and reports progress.

```
LOOP:
  result = mcp__plugin_dev-team_dev-team__phase_next(change=<name>)

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

  if result.evaluator:
    eval_result = Agent({
      description: "Evaluate artifacts for phase {result.next_phase}",
      subagent_type: result.evaluator.agent_type,
      prompt: result.evaluator.prompt
    })

    if eval_result indicates code bugs found (verdict fail, backtrack_to null, report mentions code bugs):
      Write openspec/changes/<name>/reports/code-bugs-found.md summarizing bugs from eval report
      Notify user: tests discovered production code bugs
      AskQuestion: continue workflow (e.g. proceed to integration-test) or terminate
      if user chooses terminate:
        STOP
      else:
        continue LOOP

  输出: "[Round {result.round}/20] [Phase {result.phase_index}/{result.total_phases}] {result.next_phase}: executed"
  PushNotification("Workflow {name}: Phase {result.next_phase} completed ({result.phase_index}/{result.total_phases})")

  继续 LOOP
```

### Step 3: Completion

All six test-only phases have passed evaluation.

1. 显示完成摘要:
   - Done: all test-only phases passed
   - Total phases: 6
   - Total rounds: {round}
   - Note: discovering implementation bugs via tests also fulfills the test-only workflow purpose

2. PushNotification("Test-only workflow for change '{name}' completed. Please verify and run /dev-team:openspec-archive-change")

3. **Do NOT auto-archive.** The user must manually run `/dev-team:openspec-archive-change` after inspection.

4. 完成提示: "All test-only phases completed. Please review the results and run `/dev-team:openspec-archive-change` to finalize."
