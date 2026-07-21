## ADDED Requirements

### Requirement: phase_next 输出 schema 包含 last_result 字段

`phaseNextOutputSchema` SHALL 包含 `last_result` 字段，类型为 `{ phase: string, verdict: 'pass' | 'fail', report: string, timestamp: string } | null`。

此字段代表最新的 eval.json 条目（按时间戳降序排列）。当 eval.json 为空时，`last_result` SHALL 为 `null`。

此字段供 skill 用于在 evaluator 运行后确定下一步操作：
- `verdict === "pass"` → 继续到下一 phase
- `verdict === "fail"` → 进入回溯决策流程（重试、回溯或询问用户）

#### Scenario: 正常 phase 时 phase_next 响应包含 last_result
- **WHEN** 调用 `runPhaseNext({change: "my-change"})` 且 `phase_next` 返回正常 phase 响应（非 done、非 error）
- **THEN** 响应包含 `last_result`，带有 `phase`、`verdict`、`report` 和 `timestamp` 字段
- **AND** 所有其他已有字段（`next_phase`、`planner`、`evaluator`、`round` 等）不变

#### Scenario: eval.json 为空时 phase_next 响应包含 null 的 last_result
- **WHEN** eval.json 为空
- **THEN** `last_result` 为 `null`

#### Scenario: 出错时 phase_next 响应包含 null 的 last_result
- **WHEN** `phase_next` 返回错误响应（例如超过轮次上限）
- **THEN** `last_result` 为 `null`

#### Scenario: 完成时 phase_next 响应包含 null 的 last_result
- **WHEN** 所有 phase 已通过且 `phase_next` 返回 `done: true`
- **THEN** `last_result` 为 `null`

#### Scenario: last_result 来自按时间戳排序的最新条目
- **WHEN** eval.json 有条目 `[{phase:"proposal", verdict:"pass", report:"ok", timestamp:"2026-01-01T00:00:00Z"}, {phase:"dev-design", verdict:"fail", report:"设计不完整", timestamp:"2026-01-01T01:00:00Z"}]` 且调用 `runPhaseNext()`
- **THEN** `last_result` 为 `{phase: "dev-design", verdict: "fail", report: "设计不完整", timestamp: "2026-01-01T01:00:00Z"}`

#### Scenario: 乱序条目按时间戳取最新
- **WHEN** eval.json 有条目乱序：`[{phase:"acceptance", verdict:"pass", report:"ok", timestamp:"2026-01-02T00:00:00Z"}, {phase:"proposal", verdict:"pass", report:"ok", timestamp:"2026-01-01T00:00:00Z"}]`
- **THEN** `last_result` 反映 `acceptance` 条目（按时间戳最新）

---

## 模块契约

### phase-next.schema.ts

| Export | Change | Purpose |
|--------|--------|---------|
| `phaseNextOutputSchema` | ADDED `last_result` | `z.object({ phase: z.string(), verdict: z.enum(['pass', 'fail']), report: z.string(), timestamp: z.string() }).nullable()` — 最新 eval 条目快照，供 skill 使用 |

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `runPhaseNext()` | MODIFIED | 从 eval.json 读取最新条目并附加为 `last_result` |
| `resolvePhaseNext()` | MODIFIED | 在所有响应构建函数中传递 `last_result` |
| `PhaseNextResult` 类型 | MODIFIED | 包含 `last_result` 字段 |
| `buildPhaseResponse()` | MODIFIED | 增加 `lastResult?` 参数，包含在返回值中 |
| `buildDoneResponse()` | MODIFIED | 增加 `lastResult?` 参数，返回值中包含 `last_result: null` |
| `buildErrorResponse()` | MODIFIED | 增加 `lastResult?` 参数，返回值中包含 `last_result: null` |
