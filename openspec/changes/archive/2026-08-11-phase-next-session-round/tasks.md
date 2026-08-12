# 任务: phase-next-session-round



> **变更**: phase-next-session-round

> **日期**: 2026-08-11



---



## 阶段 1: Schema 契约



- [x] 在 `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` 的 `phaseNextInputSchema` 增加必填字段 `run_id: z.string().trim().min(1)`，并补充 describe（session 窗口标识）

- [x] 更新同文件 `phaseNextOutputSchema.round` 的 describe，标明为当前 session 窗口轮次（1-based），非 change 生涯累计



## 阶段 2: phase_next 引擎（session 窗口）



- [x] 在 `plugins/dev-team/bin/src/commands/phase-next.ts` 增加模块级进程内 `Map`（key = `` `${change}\0${run_id}` `` → `anchor: number`），仅随模块加载初始化，不写磁盘

- [x] 实现模块私有 `getOrCreateAnchor(change, run_id, entriesLength)`：首次写入 `entriesLength`，之后复用；**不**将其或任何 map clear/reset 导出为公共 API（map 清空仅依赖进程退出或模块重载；AC-5 易失由后续测试用 `vi.resetModules()` + 动态 re-import 覆盖）

- [x] 修改 `runPhaseNext`：在读盘前校验 `run_id`；缺失 / 空串 / 仅空白时返回 `buildErrorResponse("missing_run_id", ...)`（`done: false`，`next_phase: null`，`last_result: null`，`round: 0`），不抛错、不回退生涯计数

- [x] 扩展 `resolvePhaseNext`（或等价内部路径）接收 `anchor`：`window = entries.slice(anchor)`；`round = computeRound(window)`（即 `|window| + 1`）

- [x] 修改 `checkRetryLimit` 调用点：仅对 `window` 统计目标 phase 的 `verdict === "fail"` 条数；`>= 5` 仍返回 `max_retries_exceeded`

- [x] 确认 `checkRoundLimit` 仍为 `round > 20`，但输入 `round` 已为 session 语义

- [x] 确认 `hasPhasePassed`、backtrack 解析、done 判定仍基于**完整** `entries`，仅限额改用 window

- [x] 确保所有成功 / 错误 / done 响应中的 `round` 与 session 计算同源（含 `round_limit_exceeded` / `max_retries_exceeded` 文案路径）



## 阶段 3: MCP 传参



- [x] 更新 `plugins/dev-team/bin/src/mcp.ts` 中 `phase_next` handler：将 `args.run_id` 传入 `runPhaseNext({ project_root, change, run_id })`



## 阶段 4: Workflow skills



- [x] 更新 `plugins/dev-team/skills/workflow-requirement/SKILL.md`：在 Orchestration LOOP 入口要求生成非空 `run_id`（opaque，如 UUID）；LOOP 内每次 `__MCP:phase_next__(change=..., run_id=...)` 使用同一值；注明下一用户消息（含 ask-user 回复）必须重新生成

- [x] 同文件：将进度行 `[Round {gate.round}/20]` 与 Step 3 完成摘要中的 rounds 说明为 session 语义（与 `gate.round` 同源），删除或改写任何「生涯累计」表述

- [x] 对 `plugins/dev-team/skills/workflow-test-only/SKILL.md` 做与上两项相同的修改



## 阶段 5: Phase skills



- [x] 更新 `phase-proposal`：turn 入口生成 `run_id`；所有 `phase_next`（含 Phase Check、backtrack 后 recall、结果复查）传入同一值

- [x] 更新 `phase-dev-design`：同上

- [x] 更新 `phase-test-design`：同上

- [x] 更新 `phase-implement`：同上

- [x] 更新 `phase-test-gen`：同上

- [x] 更新 `phase-test-execution`：同上

- [x] 更新 `phase-code-review`：同上

- [x] 更新 `phase-acceptance`：同上



## 阶段 6: 实现自检（不含写测试）



- [x] 对照 proposal AC-1～AC-7（除测试文件落地外）：缺 `run_id`、session round/fails、新 `run_id` 重置、跨 change 隔离、map 易失、skill 文案与展示语义、推进/backtrack/done 未改窗口外逻辑

- [x] 确认 `phase-next.ts` 公共导出仅为既有类型与 `runPhaseNext`（无 map reset/clear）；AC-5 易失语义靠模块状态自然消失，验证方式为 `vi.resetModules` + re-import

- [x] 全文检索 skills 与相关文档中残留的 `__MCP:phase_next__(change=` 无 `run_id` 调用形态并消除

