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
5. **No parameter provided AND zero active changes exist**:
   - List `openspec/explores/*.md` (topic-named drafts).
     - **Exactly one draft** → use its stem as the kebab-case change name (confirm with user if unclear), proceed to `### Create the change directory`.
     - **Multiple drafts** → `AskUserQuestion` to pick a draft (or "Other — describe a new change").
     - **No drafts** but conversation has **explore signals** → derive a kebab-case name from the explore topic and proceed to `### Create the change directory`.
     - **Otherwise** → use `AskUserQuestion` (open-ended) to ask: "What change do you want to work on?" Derive a kebab-case name and proceed to `### Create the change directory`.

**IMPORTANT**: Do NOT proceed without a resolved change name.

### Create the change directory

Only reached when starting a **new** change (not resuming an existing one).

Use `AskUserQuestion` to confirm the workflow type:

**Options:**
- `requirement` — full development + test pipeline
- `bug-fix` — simplified fix pipeline
- `refactor` — full pipeline for refactoring
- `test-only` — supplement tests only

```
mcp__plugin_dev-team_dev-team__change_create({ name: "<name>", workflow_type: "<choice>" })
```

Then **promote explore draft** (mechanical move, not rewrite):

1. Look under `openspec/explores/` for a file whose stem matches `<name>` or clearly matches the topic.
2. If **one** match and `openspec/changes/<name>/explore.md` does not exist → move that file to `openspec/changes/<name>/explore.md`.
3. If **multiple** candidates → `AskUserQuestion` which draft to promote (or skip).
4. If **no** draft → leave change without `explore.md` for now (handoff may still append later).

### Confirm workflow type

Check if `openspec/changes/<name>/workflow.json` exists:

- **Already exists** → skip type confirmation and continue to `### Phase Check`.
- **Does not exist** → **STOP**. Do NOT call `phase_next` (`workflow.json` is a precondition of `phase_next` / `backtrack` and a missing file is an error, not a default), and **MUST NOT** use Write/Edit to create the file (the write-protection hook denies it, and no default type is assumed anymore).

  Present the two options below to the user via `AskUserQuestion` and stop:

  (a) **Keep this directory** — the user writes `openspec/changes/<name>/workflow.json` themselves, matching `workflowFileSchema`:

  ```json
  { "workflow_type": "<requirement|bug-fix|refactor|test-only>", "created": "<YYYY-MM-DD>" }
  ```

  Then re-run this skill.

  (b) **Re-create the change** — if the directory content can be discarded, run the matching `workflow-requirement` / `workflow-test-only` skill (or call `mcp__plugin_dev-team_dev-team__change_create({ name: "<change-name>", workflow_type: "<choice>" })` directly). `change_create` rejects an already existing directory, so the directory must be removed or a new name chosen first.

### Phase Check

At the start of this turn, generate a new non-empty opaque `run_id` (e.g. UUID). Pass the same `run_id` to every `phase_next` call in this turn (Phase Check, backtrack recall, Verdict Phase Result).

Call `mcp__plugin_dev-team_dev-team__phase_next({ change: "<change-name>", run_id: "<run_id>" })` to get workflow state.

If `next_phase` is "proposal" continue to `### Explore handoff`.

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

If response `modified` is true, recall `mcp__plugin_dev-team_dev-team__phase_next({ change: "<change-name>", run_id: "<run_id>" })`, continue to `### Explore handoff`.

### Explore handoff

Run **before every** proposal executor (first run, retry, and after backtrack).

Authoritative note for the planner: `openspec/changes/<change-name>/explore.md` (free-form; no template).

Draft inbox: `openspec/explores/<topic-kebab>.md` (owned by explore; this skill only **promotes**).

1. **Promote** (if change `explore.md` missing):
   - Find matching draft under `openspec/explores/` (stem equals change name, or single obvious topic match).
   - One match → **move** to `openspec/changes/<change-name>/explore.md`.
   - Multiple → `AskUserQuestion` which to promote.
   - None → continue (planner can run without explore notes).
2. **Append** only when change `explore.md` already exists and the conversation has **new** insights not in the file (default append; rewrite only if user asks). Prefer that explore itself wrote those notes; do not invent a full first draft from conversation when a draft file should have been captured in `openspec/explores/` instead.
3. **Do NOT** paste explore body, conversation dumps, or `EXPLORE_CONTEXT_SUMMARY` into the subagent `prompt`.
4. Reminder: `explore.md` alone does not update `proposal.md`. This executor is the convergence path when formal requirements must change.

Then continue to `### Run Executor`.

### Run Executor

Call `mcp__plugin_dev-team_dev-team__phase_start({ change: "<change-name>", phase: "<next_phase>" })` once to open the phase running state (per-attempt timing archive and file-attribution gate). Repeat this call before every executor rerun (retry after fail, backtrack recall); the `phase_next` call in `### Verdict Phase Result` never triggers one.

Then call `Agent` with response of `phase_next`:

```
Agent({
  description: "Execute phase <next_phase>",
  subagent_type: executor.agent_type,
  prompt: executor.prompt
})
```

**CRITICAL**: Use `executor.prompt` as returned by `phase_next` only. Do not append explore text.

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
result = mcp__plugin_dev-team_dev-team__phase_next({ change: "<change-name>", run_id: "<run_id>" })

if result.last_result is null:
  → 错误：Evaluator 未通过 phase_log 写入 workflow.json（或 phase_next.last_result 未更新），停止

if result.last_result.verdict == "pass" → continue to `### Report`

if result.last_result.verdict == "fail" → retry from `### Explore handoff` or stop with error
```

### Report

Show verdict, pass/total, notes, and specs/ files list.
