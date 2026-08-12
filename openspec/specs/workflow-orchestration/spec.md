## 权威边界

编排层规格。引擎语义（`run_id` 校验、session window、`round`/`retry`、phase 表与前置依赖）见 `pge-workflow-engine`；回溯决策见 `pipeline-backtrack`；独立 phase skill 清单见 `phase-skills`。本文件不复述 phase 表全文。

## ADDED Requirements

### Requirement: workflow skill 在 turn 入口生成 run_id 并传入 phase_next

`workflow-requirement` 与 `workflow-test-only` SHALL 在每个用户消息触发的 agent turn 入口生成非空 `run_id`，并在该 turn 的编排 LOOP 内每次调用 `phase_next` 时传入同一值。

**用户反馈的任意消息**（含 AskUser 回复、中断后继续、闲聊后再开）MUST 生成新的 `run_id`，MUST NOT 复用上一值（决议 C9）。同 turn 内 agent 自主循环（含 backtrack 后 recall）仍复用该 turn 的 `run_id`。

```
gate = __MCP:phase_next__(change=<change-name>, run_id=<run_id>)
```

#### Scenario: LOOP 内同一 run_id

- **WHEN** 用户调用 `/dev-team:workflow-requirement` 或 `/dev-team:workflow-test-only` 开启一 turn
- **THEN** skill 在进入 LOOP 前生成 `run_id`，且 LOOP 内每一次 `phase_next` 携带该同一值

#### Scenario: 用户任意新消息换 run_id

- **WHEN** 用户发送任意新消息（含对 ask-user / AskUserQuestion 的回复）
- **THEN** 新 turn SHALL 生成新的 `run_id`，后续 `phase_next` MUST NOT 复用旧值

### Requirement: workflow 进度与完成摘要使用 session round

进度行 `[Round {gate.round}/20]` 与完成摘要中的 rounds SHALL 与 `phase_next` 返回的 `round` 同源（session 窗口语义），MUST NOT 使用 change 生涯累计。phase 序号分母使用 `gate.total_phases`（requirement=8 / test-only=5），与 round 分母 20 含义不同。

#### Scenario: 进度行展示 session round

- **WHEN** `phase_next` 返回 `round: 3`
- **THEN** 进度输出 SHALL 包含 `[Round 3/20]`（或等价）

#### Scenario: 完成摘要与 gate.round 同源

- **WHEN** `gate.done === true`
- **THEN** 完成摘要 rounds SHALL 使用本次返回的 `gate.round`

## MODIFIED Requirements

### Requirement: Workflow orchestration layer

Workflow skills（`workflow-requirement`、`workflow-test-only`）SHALL 在 phase 之上编排：直接调用 Agent + MCP `phase_next` / `phase_log` + Bash，MUST NOT 调用对应的 `phase-*` Skill。独立 `phase-*` skill 仍可单独调用。

phase 集合与数量以 `pge-workflow-engine` 的 `PHASE_*` 表为准（requirement 8 / test-only 5）。门禁统一 `phase_next`（决议 C4=A）。

#### Scenario: 编排层不经 phase skill

- **WHEN** workflow 执行 Phase `proposal`
- **THEN** 直接调用 Agent(`proposal-planner`) 与 Agent(`proposal-evaluator`)，使用 MCP `phase_next` 与 `phase_log`
- **AND** MUST NOT 调用 Skill(`phase-proposal`)

#### Scenario: 进度分母来自 total_phases

- **WHEN** `phase_next` 返回 `total_phases: 8`（或 test-only 的 `5`）
- **THEN** 进度输出为 `[Phase {phase_index}/{total_phases}]`

## Module Contract

| Workflow | workflow_type | Phase Count | Contract |
|----------|---------------|-------------|----------|
| workflow-requirement | `requirement` | 8（见 pge） | 用户消息入口新 `run_id`；LOOP 内 `phase_next(change, run_id)`；round 为 session 语义 |
| workflow-test-only | `test-only` | 5（见 pge / test-only-workflow） | 同上 |
