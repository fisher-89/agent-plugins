# 设计: phase-next-session-round

> **变更**: phase-next-session-round
> **日期**: 2026-08-11

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| phase_next schema | 入参增加必填非空 `run_id`；`round` 字段描述改为 session 窗口轮次 | `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | zod/v4 | TypeScript |
| phase_next 命令 | 进程内 `(change, run_id) → anchor` map；按 window 计算 `round` / fail；缺 `run_id` 返回 `missing_run_id` | `plugins/dev-team/bin/src/commands/phase-next.ts` | eval-json、workflow、change、schemas | TypeScript |
| MCP `phase_next` | 将调用方 `run_id` 传入 `runPhaseNext`；input schema 与上对齐 | `plugins/dev-team/bin/src/mcp.ts` | phase-next 命令、schemas | TypeScript / MCP |
| workflow skills | turn 入口生成 `run_id`，LOOP 内每次 `phase_next` 传入；进度/摘要 round 为 session 语义 | `plugins/dev-team/skills/workflow-requirement/SKILL.md`、`workflow-test-only/SKILL.md` | `phase_next` MCP | Markdown (skill) |
| phase skills | 独立调用时同样在 turn 入口生成并传入 `run_id`（含 backtrack 后 recall） | `plugins/dev-team/skills/phase-*/SKILL.md`（8 个） | `phase_next` MCP | Markdown (skill) |

### 核心数据流

```
用户消息 → skill turn 入口生成 run_id
                │
                ▼
LOOP / Phase Check:
  phase_next(change, run_id)
                │
                ▼
  校验 run_id → 读 eval.json → getOrCreateAnchor(change, run_id, entries.length)
                │
                ▼
  window = entries[anchor..]
  round  = |window| + 1
  fails  = count(window, phase==next && verdict==fail)
  （推进 / backtrack / done 仍基于完整 entries）
                │
                ▼
  响应 round（session）→ skill 展示 [Round x/20]
```

---

## 变更清单

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | `phaseNextInputSchema` 增加必填 `run_id: z.string().trim().min(1)`；`phaseNextOutputSchema.round` 的 `.describe(...)` 改为标明 session 窗口轮次（1-based），非 change 生涯累计 | 契约层强制 session 边界；trim 后空串视为无效 |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | ① 模块级进程内 `Map`（key=`change + "\\0" + run_id` → `anchor: number`），仅随模块加载初始化，无磁盘写入；② `runPhaseNext` 校验 `run_id`（缺失/空/仅空白）→ `buildErrorResponse("missing_run_id", ...)`，不抛错、不回退生涯计数；③ 模块私有 `getOrCreateAnchor`：首次记 `entries.length`，之后复用；④ `resolvePhaseNext` 接收 `anchor`，`round = computeRound(entries.slice(anchor))`，`checkRetryLimit` 仅扫 window；⑤ `hasPhasePassed` / backtrack / done 仍用完整 `entries`；⑥ 错误/完成响应中的 `round` 与成功路径同源（session）；⑦ **不**导出任何清空 / 重置 map 的公共 API（符合 CLAUDE.md「No test-only exports」） | 实现 AC-1～AC-5、AC-7 中与引擎相关的行为；阈值 20/5 不变 |
| `plugins/dev-team/bin/src/mcp.ts` | `phase_next` handler 将 `args.run_id` 传入 `runPhaseNext({ project_root, change, run_id })` | MCP 与命令层参数对齐 |
| `plugins/dev-team/skills/workflow-requirement/SKILL.md` | Step 2 LOOP 前增加「生成 `run_id`」；所有 `__MCP:phase_next__` 改为 `change=..., run_id=...`；注明下一用户消息（含 ask-user 回复）须新生成；`[Round {gate.round}/20]` 与完成摘要 rounds 说明为 session 语义 | AC-6；人在回路续命 |
| `plugins/dev-team/skills/workflow-test-only/SKILL.md` | 同上 | 与 requirement workflow 约定一致 |
| `plugins/dev-team/skills/phase-proposal/SKILL.md` | turn 入口生成 `run_id`；Phase Check / backtrack 后 recall / 结果复查等所有 `phase_next` 传入同一值 | 独立 phase skill 不依赖 workflow 注入 |
| `plugins/dev-team/skills/phase-dev-design/SKILL.md` | 同上 | 同上 |
| `plugins/dev-team/skills/phase-test-design/SKILL.md` | 同上 | 同上 |
| `plugins/dev-team/skills/phase-implement/SKILL.md` | 同上 | 同上 |
| `plugins/dev-team/skills/phase-test-gen/SKILL.md` | 同上 | 同上 |
| `plugins/dev-team/skills/phase-test-execution/SKILL.md` | 同上 | 同上 |
| `plugins/dev-team/skills/phase-code-review/SKILL.md` | 同上 | 同上 |
| `plugins/dev-team/skills/phase-acceptance/SKILL.md` | 同上 | 同上 |

<!-- 无新增实现文件：session map 落在现有 phase-next.ts 模块内，不单独拆文件 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `phase_next` | `plugins/dev-team/bin/src/mcp.ts` | 修改 (MCP 工具) | `phase_next(project_root: string, change: string, run_id: string) -> PhaseNextResult` | 入参增加必填 `run_id`；行为见 `runPhaseNext` |
| `runPhaseNext` | `plugins/dev-team/bin/src/commands/phase-next.ts` | 修改 | `function runPhaseNext(options: PhaseNextOptions): PhaseNextResult` | `PhaseNextOptions` 含必填 `run_id`；缺/空 → `{ error: "missing_run_id", done: false, next_phase: null, last_result: null, round: 0, ... }`；有值则按 session window 计 round/fails |
| `phaseNextInputSchema` | `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | 修改 | Zod object：`{ project_root, change, run_id }` | `run_id` 必填，`trim().min(1)` |

<!-- `computeRound` / `checkRetryLimit` / `resolvePhaseNext` / `getOrCreateAnchor` 均为模块私有，不列入公共 API -->
<!-- 进程内 map 清空仅依赖模块重新加载（进程重启或测试侧 vi.resetModules + 动态 re-import）；模块公共导出仅 `runPhaseNext` 与既有类型 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `PhaseNextOptions` | `plugins/dev-team/bin/src/commands/phase-next.ts`（`z.input<typeof phaseNextInputSchema>`） | 修改 | 增加 `run_id: string` |
| `PhaseNextResult` | 同上（`z.output<typeof phaseNextOutputSchema>`） | 修改（语义） | 字段形状不变；`round` 语义改为当前 session 窗口下一轮序号；`error` 可取 `"missing_run_id"` |
| `phaseNextInputSchema` | `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | 修改 | 见上 |
| `phaseNextOutputSchema` | 同上 | 修改（描述） | `round` 的 describe 文本更新为 session-scoped |

<!-- 不涉及 plugin.json / config.json 等配置键变更 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| Session anchor map（进程内） | key: `(change, run_id)`（实现为 `Map<string, number>`，key 串 `` `${change}\0${run_id}` ``）；value: `anchor`（首次见到时的 `entries.length`） | 同一 `run_id` 跨不同 `change` 不共享；与 `eval.json` 无写入关系 | **不持久化**；进程重启或模块重载后清空 |
| Session window（运行时切片） | `entries[anchor..]` | 由完整 `EvalEntry[]` 与 anchor 派生 | 不落盘 |
| `round`（响应字段） | `number`，`= \|window\| + 1` | 与 skill `[Round x/20]` / 完成摘要同源 | 仅响应内 |
| fail 计数（运行时） | window 内 `phase === next_phase && verdict === "fail"` 的条数 | `>= 5` → `max_retries_exceeded`；窗口外历史 fail 不计 | 仅计算 |
| eval.json `EvalEntry` | 既有字段不变 | 推进 / backtrack / stale / skipped 逻辑仍读完整条目 | `openspec/changes/<name>/eval.json`（本变更不改写入） |

### 算法（与 proposal / spec 对齐）

```
runPhaseNext({ change, run_id, project_root }):
  if run_id 缺失或 trim 后为空:
    return error "missing_run_id"  # done=false, next_phase=null, round=0
  entries = readEvalJson(...)
  key = (change, run_id)
  if map 无 key: map[key] = entries.length
  anchor = map[key]
  window = entries[anchor..]
  round  = window.length + 1
  # 限额用 window；编排用完整 entries
  if round > 20 → round_limit_exceeded
  fails(window, next) >= 5 → max_retries_exceeded
  否则按既有逻辑 resolve next / backtrack / done
```

### 边界约定

| 场景 | 行为 |
|------|------|
| 首次 `(change, run_id)`，已有 N 条 entries | `anchor=N`，window 空，`round=1` |
| 同 `run_id` 后续调用，entries 增长 | 复用 anchor；`round = (length - anchor) + 1` |
| 新 `run_id` | 新 anchor；预算重置（长历史可继续） |
| 进程重启后同一 `run_id` | map 随模块状态消失 → 新 anchor（可接受）；**无**公共 reset API |
| mid-phase 中断 / skipped | 窗内每条 entry（含 skipped）计 1 round，与旧「每条 entry 计 1」对齐，仅窗口从生涯改为 session |
| ask-user 后新 turn | skill 新 `run_id` → 两道预算重置 |

### 进程重启 / map 易失（AC-5 实现约束）

AC-5「进程重启后 map 清空」是模块级状态的自然行为，**不得**为此新增任何仅供测试或生产可调用的 map clear/reset 导出（CLAUDE.md「No test-only exports」）。

- **生产语义**：Node 进程退出后内存 map 消失；新进程加载 `phase-next.ts` 时得到空 map；同一 `run_id` 再次出现视为新 anchor。
- **易失验证方式（后续 test phase，非本阶段实现）**：用 vitest `vi.resetModules()` 后对 `phase-next` **动态 re-import**，取得带空 map 的新模块实例。同文件内需连续复用同一 map 的用例则保持同一 import 实例即可。
- **与 AC-4 的区分**：仅更换 `run_id` 验证「新 session 重置预算」（AC-4）；「同 `run_id` 在 map 清空后重锚定」（AC-5）靠模块重载模拟，不靠公开 API。

---

## 依赖

### 运行时依赖

- 无新增 npm 依赖；沿用现有 `zod/v4`、Node `fs`、MCP SDK
- skill 侧 `run_id` 为任意非空 opaque 字符串（建议 UUID / 随机串），由 agent 在 turn 入口生成，不引入代码库 UUID 库

### 构建/测试依赖

- 无新增；既有 vitest 覆盖扩展（测试实现由独立 phase 负责；本设计仅约束：验证 map 易失时用 `vi.resetModules` + re-import）

---

## 待决问题

- （无阻塞项）`missing_run_id` 在 MCP 协议层：Zod `inputSchema` 会在 handler 前拒绝缺参；结构化 `error: "missing_run_id"` 以保证由 `runPhaseNext` 直调路径返回。MCP 集成测试对齐「schema 必填」即可；单测覆盖结构化错误码。
- mid-phase 中断与 skipped 条目的 anchor 边界：实现时用单测核对「窗内每条 entry 计 1 round」，与现生涯语义对齐（proposal 已接受）。
