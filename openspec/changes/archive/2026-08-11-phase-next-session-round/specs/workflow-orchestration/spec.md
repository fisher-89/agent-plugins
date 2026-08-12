## ADDED Requirements

### Requirement: workflow skill 在 turn 入口生成 run_id 并传入 phase_next

`workflow-requirement` 与 `workflow-test-only` skill SHALL 在每个用户消息触发的 agent turn 入口生成一个新的非空 `run_id`（opaque 字符串即可，例如 UUID），并在该 turn 的编排 LOOP 内每次调用 `phase_next` 时传入同一 `run_id`。

下一用户消息（含中断后继续、闲聊后再开、以及对 `ask-user` 的回复）MUST 生成新的 `run_id`，不得复用上一 turn 的值。

Skill 文案中的 `phase_next` 调用形态 SHALL 为（token 名随平台展开）：

```
gate = __MCP:phase_next__(change=<change-name>, run_id=<run_id>)
```

#### Scenario: workflow-requirement LOOP 传入同一 turn 的 run_id

- **WHEN** 用户调用 `/dev-team:workflow-requirement <change-name>`（或等价 skill）开启一 turn
- **THEN** skill SHALL 在进入 orchestration LOOP 前生成 `run_id`
- **AND** LOOP 内每一次 `phase_next` SHALL 携带该同一 `run_id`

#### Scenario: ask-user 回复后使用新 run_id

- **WHEN** workflow 因 fail 走 `ask-user` 分支并等待用户选择
- **AND** 用户回复「重试 / 回溯 / 停止」触发新的 agent turn
- **THEN** 新 turn SHALL 生成新的 `run_id`
- **AND** 后续 `phase_next` MUST NOT 复用 ask-user 之前 turn 的 `run_id`

#### Scenario: workflow-test-only 同样要求 run_id

- **WHEN** 用户调用 `/dev-team:workflow-test-only <change-name>`
- **THEN** skill SHALL 在 turn 入口生成 `run_id` 并传入该 turn 内所有 `phase_next` 调用

### Requirement: workflow 进度与完成摘要使用 session round

workflow skill 输出的 `[Round {gate.round}/20]` 以及完成摘要中的 total rounds SHALL 解释为当前 session 窗口轮次（与 `phase_next` 返回的 `round` 一致），MUST NOT 再描述为 change 生涯累计轮次。

#### Scenario: 进度行展示 session round

- **WHEN** `phase_next` 在某 session 返回 `round: 3`
- **AND** workflow 完成该 phase 的 executor/evaluator
- **THEN** 进度输出 SHALL 包含 `[Round 3/20]`（或等价，分母仍为 20）

#### Scenario: 完成摘要 rounds 与最后一次 gate.round 同源

- **WHEN** workflow 因 `gate.done === true` 进入完成步骤
- **THEN** 完成摘要中的 rounds SHALL 使用本次调用返回的 `gate.round`（session 语义）

## Module Contract

### Workflow Skills (`plugins/dev-team/skills/`)

| Skill | Change | Contract |
|-------|--------|----------|
| `workflow-requirement` | MODIFIED | turn 入口生成 `run_id`；LOOP 内 `phase_next(change, run_id)`；进度/摘要 round 为 session 语义 |
| `workflow-test-only` | MODIFIED | 同上 |
