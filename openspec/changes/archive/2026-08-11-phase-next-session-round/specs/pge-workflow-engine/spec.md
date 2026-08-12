## ADDED Requirements

### Requirement: phase_next 要求必填 run_id

`phaseNextInputSchema` 与 `runPhaseNext` / MCP `phase_next` SHALL 要求调用方提供非空字符串参数 `run_id`，用作 session 窗口标识。

当 `run_id` 缺失、为 `null`、或为空字符串（含仅空白）时，`phase_next` SHALL 返回错误响应：

- `error` MUST 为 `"missing_run_id"`
- `done` MUST 为 `false`
- `next_phase` MUST 为 `null`
- MUST NOT 回退为 change 生涯累计 round / fail 计数
- MUST NOT 在缺少 `run_id` 时放行编排

#### Scenario: 缺少 run_id 返回 missing_run_id

- **WHEN** 调用 `phase_next` / `runPhaseNext` 且未提供 `run_id`（或 `run_id` 为空串）
- **THEN** 响应 `error` SHALL 为 `"missing_run_id"`
- **AND** `done` SHALL 为 `false`
- **AND** `next_phase` SHALL 为 `null`

#### Scenario: 提供非空 run_id 可继续编排

- **WHEN** 调用 `phase_next` 且 `change` 与非空 `run_id` 均有效
- **THEN** 工具 SHALL NOT 因 `run_id` 缺失而失败
- **AND** SHALL 按该 `run_id` 对应的 session 窗口计算 `round` 与 retry 限额

### Requirement: session 窗口由进程内 (change, run_id) anchor 界定

`phase_next` SHALL 在 MCP/CLI 进程内存中维护 map，key 为 `(change, run_id)`，value 为首次见到该 key 时 `eval.json` 的 `entries.length`（anchor）。

同一进程内后续带相同 `(change, run_id)` 的调用 SHALL 复用该 anchor。Session 窗口 MUST 为 `entries[anchor..]`（从 anchor 起的后缀切片）。

规则：

- 首次见到某 `(change, run_id)`：记录 `anchor = entries.length`，此时 window 为空，`round` 为 `1`
- 同一 `run_id` 字符串用于不同 `change` MUST NOT 共享 anchor
- map MUST NOT 写入 `eval.json` 或其它磁盘状态
- 进程重启后 map 清空；同一 `run_id` 再出现 SHALL 视为新 anchor（新 session 窗口）

#### Scenario: 首次 run_id 建立 anchor

- **WHEN** 某 change 的 `eval.json` 已有 12 条 entries
- **AND** 首次以 `run_id: "s1"` 调用 `phase_next`
- **THEN** 进程内 SHALL 记录 `(change, "s1") → 12`
- **AND** 响应 `round` SHALL 为 `1`

#### Scenario: 同 run_id 复用 anchor

- **WHEN** 已为 `(change, "s1")` 建立 anchor=12
- **AND** 此后又追加了 3 条 entries（总长 15）
- **AND** 再次以 `run_id: "s1"` 调用 `phase_next`
- **THEN** session 窗口 SHALL 为最后 3 条 entries
- **AND** 响应 `round` SHALL 为 `4`

#### Scenario: 不同 change 不共享同一 run_id 的 anchor

- **WHEN** change `A` 已对 `run_id: "s1"` 建立 anchor
- **AND** 对 change `B` 首次以 `run_id: "s1"` 调用 `phase_next`
- **THEN** change `B` SHALL 使用独立的 anchor（基于 `B` 当时的 `entries.length`）
- **AND** MUST NOT 复用 change `A` 的 anchor

#### Scenario: 进程重启后同一 run_id 视为新窗

- **WHEN** 进程内存 map 已清空（进程重启）
- **AND** 再次以曾用过的 `run_id` 调用 `phase_next`
- **THEN** SHALL 以当前 `entries.length` 建立新 anchor
- **AND** `round` SHALL 从该新窗重新起算

### Requirement: round 与 round_limit 仅计当前 session 窗口

`phase_next` 响应中的 `round` SHALL 表示当前 session 窗口内的下一轮序号：

```
round = |entries[anchor..]| + 1
```

当 `round > 20` 时，SHALL 返回 `error: "round_limit_exceeded"`（阈值 20 不变）。

`round` MUST NOT 再使用 change 生涯 `entries.length + 1`（除非 anchor 恰为 0 且从未换过 session，此时数值偶然相等）。

Skill 展示（如 `[Round {round}/20]`）与完成摘要中的 rounds SHALL 与该字段同源，表示 session round。

#### Scenario: session 内超过 20 round

- **WHEN** 某 `(change, run_id)` 的 window 已有 20 条 entries（`round` 将为 21）
- **AND** 以同一 `run_id` 调用 `phase_next`
- **THEN** 响应 `error` SHALL 为 `"round_limit_exceeded"`
- **AND** `done` SHALL 为 `false`

#### Scenario: 生涯条目很多但新 session 不受阻

- **WHEN** change 生涯已有 25 条 entries（旧语义下会 `round_limit_exceeded`）
- **AND** 以新的非空 `run_id` 首次调用 `phase_next`
- **THEN** 响应 SHALL NOT 为 `round_limit_exceeded`
- **AND** `round` SHALL 为 `1`

#### Scenario: 同 session 累加 round

- **WHEN** 以固定 `run_id` 在空窗上首次调用得到 `round: 1`
- **AND** 随后在同 `run_id` 下 eval 追加 1 条 entry 再调用
- **THEN** 响应 `round` SHALL 为 `2`

### Requirement: max_retries 仅计当前 session 窗口内的 fail 条数

对即将执行的下一 phase，`phase_next` SHALL 统计 session 窗口内满足 `phase === next_phase && verdict === "fail"` 的条目数（条数口径，非「连续 fail」）。

当该计数 `>= 5` 时，SHALL 返回 `error: "max_retries_exceeded"`（阈值 5 不变）。

窗口外（anchor 之前）的历史 fail MUST NOT 计入，也 MUST NOT 阻止新 session 继续同一 phase。

#### Scenario: 窗内 5 次 fail 触发 max_retries_exceeded

- **WHEN** 当前 session 窗口内目标 phase 已有 5 条 `verdict === "fail"` entries
- **AND** `phase_next` 将返回该 phase 作为 `next_phase`
- **AND** 以同一 `run_id` 调用
- **THEN** 响应 `error` SHALL 为 `"max_retries_exceeded"`

#### Scenario: 历史 session 的 fail 不挡住新窗

- **WHEN** change 生涯中某 phase 已有 5 条以上 fail，但均位于当前 `run_id` 的 anchor 之前
- **AND** 以新 `run_id` 调用 `phase_next` 且下一 phase 仍为该 phase
- **THEN** 响应 SHALL NOT 为 `max_retries_exceeded`
- **AND** SHALL 允许继续返回该 `next_phase`（若未触及其它错误）

#### Scenario: 窗内不足 5 次 fail 可重试

- **WHEN** 当前 session 窗口内目标 phase 有 4 条 fail
- **AND** 以同一 `run_id` 调用 `phase_next`
- **THEN** 响应 SHALL NOT 为 `max_retries_exceeded`

## MODIFIED Requirements

### Requirement: phase_next 输出 schema 包含 last_result 字段

`phaseNextOutputSchema` SHALL 包含 `last_result` 字段，类型为 `{ phase: string, verdict: 'pass' | 'fail', report: string, timestamp: string } | null`。

此字段代表最新的 eval.json 条目（按时间戳降序排列）。当 eval.json 为空时，`last_result` SHALL 为 `null`。

此字段供 skill 用于在 evaluator 运行后确定下一步操作：
- `verdict === "pass"` → 继续到下一 phase
- `verdict === "fail"` → 进入回溯决策流程（重试、回溯或询问用户）

调用 `runPhaseNext` / `phase_next` 时 MUST 同时提供有效 `change` 与非空 `run_id`（见 ADDED：`phase_next` 要求必填 `run_id`）。

#### Scenario: 正常 phase 时 phase_next 响应包含 last_result
- **WHEN** 调用 `runPhaseNext({change: "my-change", run_id: "s1"})` 且 `phase_next` 返回正常 phase 响应（非 done、非 error）
- **THEN** 响应包含 `last_result`，带有 `phase`、`verdict`、`report` 和 `timestamp` 字段
- **AND** 所有其他已有字段（`next_phase`、`planner`、`evaluator`、`round` 等）不变

#### Scenario: eval.json 为空时 phase_next 响应包含 null 的 last_result
- **WHEN** eval.json 为空且以有效 `run_id` 调用 `phase_next`
- **THEN** `last_result` 为 `null`

#### Scenario: 出错时 phase_next 响应包含 null 的 last_result
- **WHEN** `phase_next` 返回错误响应（例如超过轮次上限）
- **THEN** `last_result` 为 `null`

#### Scenario: 完成时 phase_next 响应包含 null 的 last_result
- **WHEN** 所有 phase 已通过且 `phase_next` 返回 `done: true`
- **THEN** `last_result` 为 `null`

#### Scenario: last_result 来自按时间戳排序的最新条目
- **WHEN** eval.json 有条目 `[{phase:"proposal", verdict:"pass", report:"ok", timestamp:"2026-01-01T00:00:00Z"}, {phase:"dev-design", verdict:"fail", report:"设计不完整", timestamp:"2026-01-01T01:00:00Z"}]` 且调用 `runPhaseNext({change, run_id})`
- **THEN** `last_result` 为 `{phase: "dev-design", verdict: "fail", report: "设计不完整", timestamp: "2026-01-01T01:00:00Z"}`

#### Scenario: 乱序条目按时间戳取最新
- **WHEN** eval.json 有条目乱序：`[{phase:"acceptance", verdict:"pass", report:"ok", timestamp:"2026-01-02T00:00:00Z"}, {phase:"proposal", verdict:"pass", report:"ok", timestamp:"2026-01-01T00:00:00Z"}]`
- **AND** 以有效 `run_id` 调用 `phase_next`
- **THEN** `last_result` 反映 `acceptance` 条目（按时间戳最新）

## Module Contract

### phase-next.schema.ts (`plugins/dev-team/bin/src/schemas/`)

| Export | Change | Purpose |
|--------|--------|---------|
| `phaseNextInputSchema` | MODIFIED | 增加必填 `run_id: z.string().min(1)`（session 窗口 id） |
| `phaseNextOutputSchema.round` | MODIFIED（描述） | `round` 表示当前 session 窗口轮次（1-based），非 change 生涯累计 |

### phase-next.ts (`plugins/dev-team/bin/src/commands/`)

| Export / Symbol | Change | Purpose |
|-----------------|--------|---------|
| 进程内 session map | ADDED | key=`(change, run_id)` → `anchor: number`（首次见到时的 `entries.length`） |
| `computeRound` | MODIFIED | 基于 session window：`\|window\| + 1` |
| `checkRetryLimit` | MODIFIED | 仅统计 window 内目标 phase 的 fail 条数 |
| `checkRoundLimit` | UNCHANGED（阈值） | 仍为 `round > 20`；输入 `round` 改为 session 语义 |
| `runPhaseNext` | MODIFIED | 校验 `run_id`；解析/复用 anchor；缺参返回 `missing_run_id` |
| `resolvePhaseNext` | MODIFIED | 接收 window 或 anchor，使 round/retry 同源 |

### mcp.ts

| Tool | Change | Purpose |
|------|--------|---------|
| `phase_next` | MODIFIED | 将调用方 `run_id` 传入 `runPhaseNext`；input schema 与上对齐 |
