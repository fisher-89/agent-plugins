---
name: __SKILL:workflow-test-only__
description: |
  Test-only PGE workflow orchestrator.
  No hardcoded phase knowledge. Uses phase_next for all orchestration decisions.
  On code bug discovery, writes a report and asks user to continue or terminate.
disable-model-invocation: true
---

**Input**: Optionally specify a change name (kebab-case), OR a description of what the user wants to build. If omitted, check if it can be inferred from conversation context.

## Evaluator 约定

Evaluator agent 内部调用 `__MCP:phase_log__` 将 verdict/report/checklist 写入 `workflow.json`（`eval` 字段）。Skill 通过 `phase_next` 的 `last_result` 字段读取 verdict。

## Constraint

**除下文Steps指定步骤外，禁止直接执行以下操作：**

- **禁止**改代码（__TOOL_WRITE__ / __TOOL_EDIT__）
- **禁止**执行测试或运行 shell 命令（__TOOL_BASH__）
- **禁止**调用 phase_log（应由 evaluator subagent 负责）

## Steps

### Step 0: Resolve the target change

Call `__MCP:change_list__()` to get active changes.

**Decision tree based on user input and change list:**

1. **User provided a parameter that exactly matches an existing change name** → use that change, skip to Step 2.
2. **User or context provided a description (not an exact change name match)**:
   - If **no active changes exist** → treat as a new change. Derive a kebab-case name and proceed to Step 1.
   - If **active changes exist**, judge whether the description semantically relates to an existing change.
     - **Confident it matches an existing change** → use that change, skip to Step 2.
     - **Confident it is unrelated to any existing change** → treat as a new change. Derive a kebab-case name and proceed to Step 1.
     - **Uncertain** → use `__TOOL_ASK_USER__` to present the potentially matching change(s) plus a "Create a new change" option.
3. **No parameter provided AND exactly one active change exists** → auto-select that change, skip to Step 2.
4. **No parameter provided AND multiple active changes exist** → use `__TOOL_ASK_USER__` to present the list plus "Other — describe a new change".
5. **No parameter provided AND zero active changes exist**:
   - List `openspec/explores/*.md` (topic-named drafts).
     - **Exactly one draft** → use its stem as the change name (confirm if unclear), proceed to Step 1.
     - **Multiple drafts** → `__TOOL_ASK_USER__` to pick a draft or describe a new change.
     - **No drafts** but conversation has **explore signals** → derive kebab-case name from topic, proceed to Step 1.
     - **Otherwise** → `__TOOL_ASK_USER__`: "What change do you want to work on?"

**IMPORTANT**: Do NOT proceed without a resolved change name.

### Step 1: Create the change directory

Only reached when starting a **new** change.

```
__MCP:change_create__({ name: "<change-name>", workflow_type: "test-only" })
```

`change_create` is the ONLY creator of `workflow.json`; the file it writes is the
precondition of `phase_next` / `backtrack` (a missing or malformed file is an
error, not a default), so do NOT hand-write or edit it. Evaluator results land in
`workflow.json.eval` through `__MCP:phase_log__` — never by writing the file directly.

**Promote explore draft** (mechanical move):

1. Under `openspec/explores/`, find a file whose stem matches `<change-name>` or clearly matches the topic.
2. One match and change `explore.md` missing → move to `openspec/changes/<change-name>/explore.md`.
3. Multiple candidates → `__TOOL_ASK_USER__` which to promote (or skip).
4. No draft → continue without change `explore.md`.

### Step 2: Orchestration loop

At the start of this turn (before entering the LOOP), generate a new non-empty opaque `run_id` (e.g. UUID). Use the same `run_id` for every `phase_next` call within this turn. Each new user message (including replies after `ask-user`) MUST generate a fresh `run_id`.

```
LOOP:
  -- Phase Check --
  gate = __MCP:phase_next__({change: "<change-name>", run_id: "<run_id>"})

  if gate.error:
    报告: "Workflow error [{gate.error}]: {gate.message}"
    PushNotification("Workflow {change-name} failed: {gate.error}")
    STOP

  if gate.done:
    proceed to Step 3

  if gate.last_result.verdict == "fail":
    分析 gate.last_result.report 确定失败原因

    -- 保留现有的 code-bugs-found 报告逻辑 --
    if report mentions code bugs:
      Write openspec/changes/<change-name>/reports/code-bugs-found.md summarizing bugs from eval report
      Notify user: tests discovered production code bugs
      `__TOOL_ASK_USER__`: continue workflow or terminate
      if user chooses terminate:
        STOP
      else:
        continue LOOP

    从 gate.allowed_backtrack_phases 获取可以回溯的步骤
    三叉决策分支：
      - retry：continue with gate.last_result.phase
      - backtrack：
        __MCP:backtrack__({
          change: "<change-name>",
          phase: "<gate.last_result.phase>",
          backtrack_to: "<从 report 分析出的目标 phase，必须在gate.allowed_backtrack_phases中>",
          backtrack_reason: "<从 report 提取的原因>"
        })
      - ask-user：__TOOL_ASK_USER__ 请求用户选择：
        - 重试（继续 LOOP）
        - 回溯到指定 phase
        - 停止

  -- Explore handoff (proposal only) --
  if gate.next_phase == "proposal":
    target = openspec/changes/<change-name>/explore.md  (free-form; no template)
    inbox = openspec/explores/<topic-kebab>.md  (explore-owned drafts)
    if target missing:
      promote matching draft from inbox (stem == change name or single obvious match) via move
      if multiple candidates → __TOOL_ASK_USER__
    else if target exists and conversation has new insights not in file → Append (default)
    Do NOT invent a full first draft from conversation when it should live in openspec/explores/ first
    CRITICAL: Do NOT append explore body / EXPLORE_CONTEXT_SUMMARY to gate.executor.prompt
    Reminder: explore.md alone does not update proposal.md; this proposal executor is the convergence path

  -- Run Executor if Exist --
  if gate.executor:
    Agent({
      description: "Execute phase {gate.next_phase}",
      subagent_type: gate.executor.agent_type,
      prompt: gate.executor.prompt
    })

  -- Run Evaluator --
  if gate.evaluator:
    Agent({
      description: "Evaluate phase {gate.next_phase}",
      subagent_type: gate.evaluator.agent_type,
      prompt: gate.evaluator.prompt
    })

  输出: "[Round {gate.round}/20] [Phase {gate.phase_index}/{gate.total_phases}] {gate.next_phase}: executed"  # gate.round is session-scoped for this turn
  PushNotification("Workflow {change-name}: Phase {gate.next_phase} completed ({gate.phase_index}/{gate.total_phases})")

  继续 LOOP
```

### Step 3: Completion

All phases have passed evaluation.

1. 显示完成摘要：Done / Total phases: {total_phases} / Total rounds: {round} (session round for this turn, from gate.round) / Note: discovering implementation bugs via tests also fulfills the test-only workflow purpose
2. PushNotification("Test-only workflow for change '{change-name}' completed. Please verify and run __CALL_SKILL:openspec-archive-change__")
3. **Do NOT auto-archive.**
4. 完成提示: "All test-only phases completed. Please review the results and run `__CALL_SKILL:openspec-archive-change__` to finalize."
