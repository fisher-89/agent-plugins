## 权威边界

用户可调用的 `phase-*` skill 清单与 agent 链。门禁/`run_id` 引擎语义见 `pge-workflow-engine`；fail 后三叉决策见 `pipeline-backtrack`。本文件不复述 phase 前置依赖表。

门禁工具统一为 `phase_next`（决议 C4=A）。不使用 `phase_check`。

## ADDED Requirements

### Requirement: phase skill 调用 phase_next 时必须传入 run_id

所有 `phase-*` skill（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance）SHALL：

1. 在本 turn 入口生成非空 `run_id`
2. 本 turn 内每一次 `phase_next`（含 backtrack 后 recall）传入同一 `run_id`
3. 不得省略（省略 → 引擎 `missing_run_id`）

**用户反馈的任意消息**（含 AskUser 回复、中断后继续、闲聊后再开）MUST 生成新的 `run_id`，MUST NOT 复用上一值（决议 C9）。

独立调用时 MUST 自行生成 `run_id`，MUST NOT 依赖 workflow 注入。

```
__MCP:phase_next__(change=<change-name>, run_id=<run_id>)
```

#### Scenario: 独立 phase skill 传入 run_id

- **WHEN** 用户直接调用任一上述 `phase-*` skill
- **THEN** 初始 gate 与 backtrack 后 recall 的 `phase_next` SHALL 均携带本 turn 的 `run_id`

#### Scenario: 用户任意新消息换 run_id

- **WHEN** 用户发送任意新消息（含对 AskUserQuestion 的回复）
- **THEN** skill SHALL 生成新的 `run_id` 再调用 `phase_next`

## MODIFIED Requirements

### Requirement: Eight user-triggered phase skills

系统 SHALL 提供 8 个 phase skill（原 9：`phase-unit-test`→`phase-test-execution`，`phase-integration-test` 删除并入前者）：

| # | Skill | MCP | Planner / Executor | Evaluator |
|---|-------|-----|--------------------|-----------|
| 1 | phase-proposal | phase_next (+ phase_log via evaluator) | proposal-planner | proposal-evaluator |
| 2 | phase-dev-design | phase_next | dev-design-planner | dev-design-evaluator |
| 3 | phase-test-design | phase_next | test-design-planner | test-design-evaluator |
| 4 | phase-test-gen | phase_next | test-gen-generator | test-gen-evaluator |
| 5 | phase-implement | phase_next | implementation-generator | implementation-evaluator |
| 6 | phase-test-execution | phase_next + phase_log | test-execution-executor | test-execution-evaluator |
| 7 | phase-code-review | phase_next | (none) | code-review-evaluator |
| 8 | phase-acceptance | phase_next | (none) | acceptance-evaluator |

门禁：调用 `phase_next(change, run_id)`，以返回的 `next_phase` / 错误决定是否可执行本 phase。`phase-test-execution` 另经 executor/evaluator 使用 `phase_log`；读 verdict 使用 `phase_next` 的 `last_result` / `next_phase`（与其它 skill 相同）。

#### Scenario: phase-test-execution EXEC 循环

- **WHEN** 用户调用 `/dev-team:phase-test-execution <change-name>`
- **THEN** skill 以 `phase_next(change, run_id)` 做门禁（期望 `next_phase` 为 `test-execution` 或可继续）
- **AND** 依次调用 `test-execution-executor`、`test-execution-evaluator`
- **AND** fail 时按 `pipeline-backtrack` 决策（非 evaluator 设 `backtrack_to`）

#### Scenario: 旧 skill 路径不存在

- **WHEN** 检查 `skills/phase-unit-test/` 或 `skills/phase-integration-test/`
- **THEN** 目录 SHALL NOT 存在；功能由 `phase-test-execution` 承担

## Module Contract

路径：`plugins/dev-team/skills/`。Agent 映射同上表。归档 skill（`openspec-archive-change`）检查 `acceptance` pass，本变更不改其契约。
