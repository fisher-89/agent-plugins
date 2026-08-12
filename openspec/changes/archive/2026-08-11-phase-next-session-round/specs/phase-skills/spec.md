## ADDED Requirements

### Requirement: phase skill 调用 phase_next 时必须传入 run_id

所有直接调用 `phase_next` 的 `phase-*` skill（`phase-proposal`、`phase-dev-design`、`phase-test-design`、`phase-implement`、`phase-test-gen`、`phase-test-execution`、`phase-code-review`、`phase-acceptance`）SHALL：

1. 在用户消息触发的本 turn 入口生成一个非空 `run_id`
2. 本 turn 内每一次 `__MCP:phase_next__`（含 backtrack 后的 recall）传入同一 `run_id`
3. 不得省略 `run_id`（省略将导致 `missing_run_id`）

调用形态：

```
__MCP:phase_next__(change=<change-name>, run_id=<run_id>)
```

#### Scenario: phase-proposal Phase Check 传入 run_id

- **WHEN** 用户调用 `phase-proposal` skill
- **THEN** skill 文案中的 Phase Check `phase_next` 调用 SHALL 包含 `run_id`
- **AND** backtrack 后 recall 的 `phase_next` SHALL 使用同一 turn 的 `run_id`

#### Scenario: phase-implement 等 EXEC skills 传入 run_id

- **WHEN** 用户调用 `phase-implement`（或同类 EXEC/EVAL phase skill）
- **THEN** 初始 gate check 与 backtrack 后 recall 的 `phase_next` SHALL 均携带本 turn 的 `run_id`

#### Scenario: 独立 phase skill 与 workflow 约定一致

- **WHEN** 用户不经过 workflow、直接调用某一 `phase-*` skill
- **THEN** 该 skill 仍 SHALL 自行生成 `run_id` 并传入 `phase_next`
- **AND** MUST NOT 依赖 workflow skill 注入 `run_id`

## Module Contract

### Skill Files (`plugins/dev-team/skills/`)

| Skill | Change | Contract |
|-------|--------|----------|
| `phase-proposal` | MODIFIED | turn 入口生成 `run_id`；所有 `phase_next` 传入 |
| `phase-dev-design` | MODIFIED | 同上 |
| `phase-test-design` | MODIFIED | 同上 |
| `phase-implement` | MODIFIED | 同上 |
| `phase-test-gen` | MODIFIED | 同上 |
| `phase-test-execution` | MODIFIED | 同上 |
| `phase-code-review` | MODIFIED | 同上 |
| `phase-acceptance` | MODIFIED | 同上 |
