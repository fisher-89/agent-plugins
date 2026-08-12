# phase-next session-scoped round / retry

> Explore notes — 2026-08-11  
> 相关代码: `plugins/dev-team/bin/src/commands/phase-next.ts`（`computeRound`, `checkRoundLimit`, `checkRetryLimit`）

## 问题

`phase_next` 当前：

- `round = entries.length + 1`（change 生涯累计）
- `round > 20` → `round_limit_exceeded`
- 单 phase `verdict===fail` 条数 `>= 5` → `max_retries_exceeded`（同样生涯累计）

设计初衷：无人值守 LOOP（尤其 backtrack）防死循环。  
副作用：真实 workflow 常拆成多轮用户介入（中断后续跑、分段执行），合法流程会吃光同一份 20 / 5 预算。

## 设计目的（不变）

限额保护的是 **单次无人值守 LOOP**，不是 change 历史总长度。

## 已拍板决策

1. **Session 边界** = 任意用户消息触发的新 agent turn  
   - 含：新指令、中断后继续、闲聊后再开  
   - **含：`ask-user` 的回复**（重试 / 回溯 / 停止）→ 也清零
2. **`round` 语义与展示** = 仅当前 session  
   - `[Round x/20]`、完成摘要中的 rounds 均不再表示生涯累计
3. **`fail >= 5`（max_retries）** = 与 round **同一 session 窗口**内计数  
   - 历史 session 的 fail **不**再挡住新窗继续跑同一 phase
4. **风险接受**：人在回路（含 ask-user「重试」）= 续命 round 与 fail 两道预算；跨 phase backtrack 晃荡不再靠生涯总预算硬收口，靠人在回路 + session 内两道闸
5. **未传 `run_id`**：`phase_next` **报错**（不回退生涯累计、不默认放行）。调用方必须显式提供 session 边界。
6. **`run_id` baseline 存储**：进程内控制（MCP/CLI 进程内存 map）。不写入 eval.json / 磁盘。  
   - 首次见到某 `run_id`：记录 anchor（如当时 `entries.length`）  
   - 同进程后续带同一 `run_id` 的调用：相对该 anchor 算 round / fails  
   - 进程重启后 map 清空：同一 `run_id` 再出现视为新 anchor（等同新窗；可接受）
7. **`fails` 口径**：本窗内该 phase 的 `verdict===fail` **条数**（与现状一致，仅加 session 窗；不做「连续 fail」）
8. **进程内 map key**：`(change, run_id)` —— 同一 `run_id` 字符串跨 change 不共享 anchor

## 机制倾向

`phase_next` 只读 eval.json，看不见用户消息；仓库已无 UserPromptSubmit 意图 hook。

→ 由 **调用方（workflow / phase skill）在每个 turn 入口生成 `run_id`（或等价 anchor）**，传入 `phase_next`：

```
phase_next(change, run_id):   # run_id 必填，缺则报错
  key = (change, run_id)
  若 map 无 key: map[key] = entries.length   # anchor
  window = entries[anchor..]
  round  = |window| + 1                      # 展示与限额同源
  fails  = count(window, phase == next && verdict == fail)  # 本窗条数
  if round > 20  → round_limit_exceeded
  if fails >= 5  → max_retries_exceeded
```

同一 `(change, run_id)` 同时裁剪 round 与 fail，不必两套边界。

备选曾讨论：纯 skill 本地计数（限额变 prompt 约定，较弱）；eval.json session marker（任意提问写盘太重）。倾向 **run_id 参数 + 进程内 map**。

## 语义示意

```
User message (任意，含 ask-user)
        │
        ▼
  run_id = new()
  LOOP: phase_next(..., run_id)
        round / fails 仅计本窗
        >20 或 fails>=5 → STOP
        │
下一句用户话 → 新 run_id → 两窗预算重置
```

## 开放实现细节（proposal 时可定）

- 缺 `run_id` 时的 `error` 码 / 文案（已定行为：报错；具体字符串待定）
- skill 文案与 schema（`phase-next` 入参 `run_id`、响应 `round`、错误文案）同步；所有调用 `phase_next` 的 skill 必须在 turn 入口生成并传入 `run_id`
- anchor 取 `entries.length` 时与 mid-phase 中断、skipped 条目的边界是否足够（实现时核对测试）

## 下一步

- 足够收敛时可开 change：`dev-team_phase-proposal`（主题建议与本文件名对齐：`phase-next-session-round`）
