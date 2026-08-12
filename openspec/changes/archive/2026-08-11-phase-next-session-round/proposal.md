# 提案: phase-next-session-round

> **变更**: phase-next-session-round
> **日期**: 2026-08-11
> **状态**: 草稿

---

## 问题

`phase_next` 当前以 change 生涯累计条目计算限额：

- `round = entries.length + 1`，`round > 20` → `round_limit_exceeded`
- 目标 phase 上 `verdict === fail` 条数 `>= 5` → `max_retries_exceeded`

设计初衷是保护无人值守 LOOP（尤其 backtrack）防死循环。副作用是真实 workflow 常被用户消息拆成多段（中断后续跑、分段执行、`ask-user` 后再继续），合法流程会吃光同一份 20 / 5 预算，无法在长历史 change 上继续推进。

限额本应保护的是**单次无人值守 LOOP**，不是 change 历史总长度。

---

## 提案

将 `round` 与同 phase `fail` 计数改为 **session 窗口**，由调用方显式传入 `run_id` 界定窗口；`phase_next` 进程内维护 `(change, run_id) → anchor` map，相对 `eval.json` 条目切片计预算。

### Session 边界

任意用户消息触发的新 agent turn 即为新 session，包括：新指令、中断后继续、闲聊后再开、以及 **`ask-user` 的回复**（重试 / 回溯 / 停止）。每个 turn 入口由 workflow / phase skill **生成新的 `run_id`** 并传入本 turn 内所有 `phase_next` 调用。

### 核心机制

```
phase_next(change, run_id):   # run_id 必填
  key = (change, run_id)
  若 map 无 key: map[key] = entries.length   # anchor（首次见到）
  window = entries[anchor..]
  round  = |window| + 1
  fails  = count(window, phase == next && verdict == fail)
  if round > 20  → round_limit_exceeded
  if fails >= 5  → max_retries_exceeded
```

要点：

1. **`run_id` 必填**：缺失或空串时返回错误（`missing_run_id`），不回退生涯累计、不默认放行。
2. **进程内 map**：不写 `eval.json` / 磁盘；进程重启后 map 清空，同一 `run_id` 再出现视为新 anchor（可接受）。
3. **map key** 为 `(change, run_id)`：同一 `run_id` 字符串跨 change 不共享 anchor。
4. **`round` 语义**：响应字段与 skill 展示（`[Round x/20]`、完成摘要 rounds）均为当前 session，不再表示生涯累计。
5. **`fails` 口径**：本窗内该 phase 的 `verdict===fail` **条数**（与现状一致，仅加 session 窗；不做「连续 fail」）。
6. **风险接受**：人在回路（含 ask-user「重试」）= 续命两道预算；跨 phase backtrack 晃荡不再靠生涯总预算硬收口。

### 调用方同步

所有调用 `phase_next` 的 skill（`workflow-requirement`、`workflow-test-only`、各 `phase-*`）MUST 在 turn 入口生成 `run_id`，并在该 turn 内每次 `phase_next` 传入同一值；下一用户消息后重新生成。

---

## 能力

### 新增能力

（无）

### 修改的能力

- **pge-workflow-engine** — `phase_next` 增加必填 `run_id`；round / max_retries 按进程内 session anchor 窗口计算；缺 `run_id` 报错
- **workflow-orchestration** — workflow skill 在 turn 入口生成 `run_id`，LOOP 内传入 `phase_next`；进度文案中的 round 为 session 语义
- **phase-skills** — 各 `phase-*` skill 在 turn 入口生成并传入 `run_id`，与 workflow 调用约定一致

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` — `phaseNextInputSchema` 增加必填 `run_id`（非空 string）；必要时更新 `round` 字段描述为 session-scoped
- `plugins/dev-team/bin/src/commands/phase-next.ts` — 进程内 `(change, run_id)` anchor map；`computeRound` / `checkRetryLimit` 基于 window；缺 `run_id` → `missing_run_id`；错误/完成文案中 round 与展示同源
- `plugins/dev-team/bin/src/mcp.ts` — 将 `run_id` 传入 `runPhaseNext`
- `plugins/dev-team/skills/workflow-requirement/SKILL.md` — turn 入口生成 `run_id`；所有 `__MCP:phase_next__` 传入；`[Round x/20]` / 完成摘要语义说明
- `plugins/dev-team/skills/workflow-test-only/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-*/SKILL.md` — 各 phase skill 的 `phase_next` 调用增加 `run_id`（turn 入口生成一次，本 turn 内复用）

### 测试文件

- `plugins/dev-team/bin/src/commands/phase-next.test.ts` — 覆盖：缺 `run_id` 报错；同 `run_id` 相对 anchor 计 round/fails；新 `run_id` 重置预算；跨 change 不共享；生涯累计长历史 + 新 session 可继续；既有 round/retry 用例改为带 `run_id` 并断言 session 语义
- `plugins/dev-team/bin/src/mcp.test.ts` — schema / 调用参数包含 `run_id`；缺参校验与现有工具测试模式对齐

### 不要修改

- 不把 session marker / `run_id` 写入 `eval.json` 或其它磁盘状态
- 不恢复 UserPromptSubmit 类 hook 来推断 session
- 不把限额改成纯 skill 本地计数（prompt 约定）
- 不改变 20 / 5 数值阈值本身（仅改变计数窗口）
- 不改变 `fails` 为「连续 fail」语义
- 不改变 backtrack / stale / phase 推进逻辑（除计数窗口外）
- 不在本变更引入跨进程持久化的 session store

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `run_id` 必填 | 调用 `phase_next` 缺省或空 `run_id` 时返回 `error: "missing_run_id"`，且不回退生涯累计计数 |
| AC-2 | session round | 同进程内首次 `(change, run_id)` 以当时 `entries.length` 为 anchor；`round = \|entries[anchor..]\| + 1`；`round > 20` → `round_limit_exceeded` |
| AC-3 | session max_retries | 同 window 内目标 phase 的 `verdict===fail` 条数 `>= 5` → `max_retries_exceeded`；窗口外历史 fail 不计入 |
| AC-4 | 新 session 重置 | 更换 `run_id` 后 round 与 fails 预算重置；长历史 change 在新 `run_id` 下可继续推进（不受生涯 20/5 阻挡） |
| AC-5 | map 隔离与易失 | 同一 `run_id` 跨不同 `change` 不共享 anchor；进程重启后 map 清空，同一 `run_id` 视为新 anchor |
| AC-6 | skill 同步 | workflow 与 phase skill 文案要求 turn 入口生成 `run_id` 并传入每次 `phase_next`；进度 `[Round x/20]` 表示 session round |
| AC-7 | 回归 | 带同一 `run_id` 的无人值守 LOOP 仍在 20 round / 5 fail 处停止；phase 推进、backtrack、done 行为不变 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 调用方忘记传/轮换 `run_id` | 缺参直接失败，或同 id 跨 turn 误续预算 | 中 | schema 必填 + skill 文案明确 turn 入口生成；测试覆盖缺参 |
| ask-user「重试」清零预算 | 恶意/失控重试可绕过原生涯闸 | 低（已接受） | 人在回路即续命；session 内仍有 20/5 |
| 进程重启丢失 map | 同一 `run_id` 预算意外重置 | 低（已接受） | 文档说明易失；不写盘 |
| mid-phase 中断 / skipped 与 anchor=`entries.length` 边界 | round 多计或少计 1 | 中 | 单测覆盖中断后续跑、skipped 条目；与现生涯语义对齐为「窗内每条 entry 计 1 round」 |
| MCP 客户端/旧 skill 未更新 | 调用全部失败 | 中 | 同步改全部 skill；插件升版重建产物 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| Session 如何界定 | 用户消息触发的 agent turn；由调用方生成 `run_id` | `phase_next` 只读 eval，看不见用户消息；仓库已无 UserPromptSubmit 意图 hook | eval.json 写 session marker；纯 skill 本地计数 |
| 缺 `run_id` | 报错 `missing_run_id`，不回退 | 强制显式 session 边界，避免静默用错窗口 | 缺省回退生涯累计；缺省放行 |
| 存储 | 进程内 map，key=`(change, run_id)` | 轻量、不污染 eval；跨 change 隔离 | 磁盘持久化；仅按 `run_id` 全局 key |
| fail 口径 | 窗内 fail **条数**（非连续） | 与现状一致，仅加窗口 | 连续 fail；生涯累计保留一道闸 |
| ask-user 是否清零 | 清零（新 `run_id`） | 人在回路 = 新无人值守窗 | ask-user 复用旧 `run_id` |

### 待决问题

- （无阻塞项）实现时核对 mid-phase 中断与 skipped 条目的 anchor 边界单测即可
