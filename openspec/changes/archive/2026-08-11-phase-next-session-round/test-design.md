# 测试设计: phase-next-session-round

> **日期**: 2026-08-11

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 调用 `phase_next` 缺省或空 `run_id` 时返回 `error: "missing_run_id"`，且不回退生涯累计计数 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts`；`plugins/dev-team/bin/src/mcp.test.ts` |
| AC-2 | 同进程内首次 `(change, run_id)` 以当时 `entries.length` 为 anchor；`round = \|entries[anchor..]\| + 1`；`round > 20` → `round_limit_exceeded` | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` |
| AC-3 | 同 window 内目标 phase 的 `verdict===fail` 条数 `>= 5` → `max_retries_exceeded`；窗口外历史 fail 不计入 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` |
| AC-4 | 更换 `run_id` 后 round 与 fails 预算重置；长历史 change 在新 `run_id` 下可继续推进（不受生涯 20/5 阻挡） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` |
| AC-5 | 同一 `run_id` 跨不同 `change` 不共享 anchor；进程重启后 map 清空，同一 `run_id` 视为新 anchor | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts`（`vi.resetModules` + 动态 re-import） |
| AC-6 | workflow 与 phase skill 文案要求 turn 入口生成 `run_id` 并传入每次 `phase_next`；进度 `[Round x/20]` 表示 session round | 集成测试 | `plugins/dev-team/skills/workflow-*/SKILL.md`、`plugins/dev-team/skills/phase-*/SKILL.md` → `plugins/dev-team/bin/__tests__/phase-next-run-id-skills/phase-next-run-id-skills.test.ts` |
| AC-7 | 带同一 `run_id` 的无人值守 LOOP 仍在 20 round / 5 fail 处停止；phase 推进、backtrack、done 行为不变 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts`；`plugins/dev-team/bin/src/mcp.test.ts` |

---

## 单元测试

框架：`vite-plus`（`vp test`，断言库为 vitest API：`describe` / `it` / `expect` / `vi`）。既有用例通过 helper `next(entries, change?, workflowType?)` 调用 `runPhaseNext`；本变更后 helper 与全部既有调用 MUST 增加非空 `run_id`（建议默认 `'test-run'`），并按 session 语义调整 round / retry 断言。

### `plugins/dev-team/bin/src/commands/phase-next.ts` -> `plugins/dev-team/bin/src/commands/phase-next.test.ts`

#### 待测功能

- `runPhaseNext(options: PhaseNextOptions): PhaseNextResult`: 校验必填非空 `run_id`；维护进程内 `(change, run_id) → anchor` map；按 session window 计算 `round` / fail 限额；推进 / backtrack / done 仍基于完整 `entries`
- `PhaseNextOptions`（类型）: `z.input<typeof phaseNextInputSchema>`，含 `project_root`、`change`、`run_id`
- `PhaseNextResult`（类型）: `z.output<typeof phaseNextOutputSchema>`；`error` 可取 `"missing_run_id"`；`round` 为 session 窗口下一轮序号

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `runPhaseNext — missing_run_id (AC-1)` | 异常 | 省略 `run_id`（cast 绕过类型）时返回 `error: "missing_run_id"`、`done: false`、`next_phase: null`、`round: 0`，且不读取推进结果 | 新增 |
| `runPhaseNext — missing_run_id (AC-1)` | 异常 | `run_id: ""` 时返回 `missing_run_id`，不回退生涯累计 | 新增 |
| `runPhaseNext — missing_run_id (AC-1)` | 异常 | `run_id: "   "`（仅空白，trim 后空）时返回 `missing_run_id` | 新增 |
| `runPhaseNext — missing_run_id (AC-1)` | 异常 | `run_id: null` / `undefined`（cast）时返回 `missing_run_id` | 新增 |
| `runPhaseNext — missing_run_id (AC-1)` | 边界 | 生涯已有 25 条 entries 且缺 `run_id` 时仍为 `missing_run_id`，不得出现 `round_limit_exceeded` | 新增 |
| `runPhaseNext — session round (AC-2)` | 正向 | entries 已有 12 条时首次 `run_id: "s1"` → `round === 1`（anchor=12，window 空） | 新增 |
| `runPhaseNext — session round (AC-2)` | 正向 | 同 `run_id: "s1"` 下 entries 增至 15 后再调用 → `round === 4` | 新增 |
| `runPhaseNext — session round (AC-2)` | 正向 | 同 `run_id` 空窗首次调用 `round === 1`；再追加 1 条 entry 后 `round === 2` | 新增 |
| `runPhaseNext — session round (AC-2)` | 异常 | 同 `run_id` 窗内已有 20 条 entries 再调用 → `error: "round_limit_exceeded"`、`done: false` | 新增 |
| `runPhaseNext — session round (AC-2)` | 边界 | 同 `run_id` 窗内 19 条（`round === 20`）→ `error === null`（阈值仍为 `>` 20） | 新增 |
| `runPhaseNext — session round (AC-2)` | 边界 | 窗内含 `skipped` 条目时每条仍计 1 round（与旧「每条 entry 计 1」对齐） | 新增 |
| `runPhaseNext — session max_retries (AC-3)` | 异常 | 同 window 内目标 phase 已有 5 条 `verdict==="fail"` → `max_retries_exceeded` | 新增 |
| `runPhaseNext — session max_retries (AC-3)` | 正向 | 同 window 内目标 phase 仅 4 条 fail → 仍返回该 `next_phase`，`error === null` | 新增 |
| `runPhaseNext — session max_retries (AC-3)` | 正向 | 生涯中目标 phase ≥5 次 fail 但均在 anchor 之前；新 `run_id` 首次调用 → 非 `max_retries_exceeded`，可继续返回该 phase | 新增 |
| `runPhaseNext — session max_retries (AC-3)` | 边界 | 窗内 5 条 fail 分属不同 phase，当前 next 仅对应 4 条 → 不触发 `max_retries_exceeded`（按 next_phase 过滤） | 新增 |
| `runPhaseNext — 新 session 重置 (AC-4)` | 正向 | 生涯 25 条 entries（旧语义会触顶）+ 新 `run_id` 首次调用 → 非 `round_limit_exceeded`，`round === 1`，可返回 `next_phase` | 新增 |
| `runPhaseNext — 新 session 重置 (AC-4)` | 正向 | 同 change 先用 `run_id: "s1"` 耗尽 fail 预算后，换 `run_id: "s2"` → fails 预算重置，可继续该 phase | 新增 |
| `runPhaseNext — 新 session 重置 (AC-4)` | 异常 | 旧 `run_id: "s1"` 已建立 session 后，下一调用换为空串 `run_id: ""` → `missing_run_id`，不得沿用旧 anchor 继续推进 | 新增 |
| `runPhaseNext — 新 session 重置 (AC-4)` | 异常 | 换 `run_id: "s2"` 重置后，在同窗内再堆满 21 条 entries → 仍返回 `round_limit_exceeded`（重置非永久豁免） | 新增 |
| `runPhaseNext — 新 session 重置 (AC-4)` | 边界 | 更换 `run_id` 后 anchor 取当前 `entries.length`，与旧 map 条目无关 | 新增 |
| `runPhaseNext — map 隔离与易失 (AC-5)` | 正向 | change `A` 对 `run_id: "s1"` 建立 anchor 后，change `B` 同 `run_id` 首次调用使用 B 自身 `entries.length` 为新 anchor | 新增 |
| `runPhaseNext — map 隔离与易失 (AC-5)` | 正向 | `vi.resetModules()` 后动态 `import('./phase-next')`，同一 `run_id` 在长历史 entries 上 `round === 1`（新 anchor） | 新增 |
| `runPhaseNext — map 隔离与易失 (AC-5)` | 异常 | change `A`/`B` 同 `run_id` 各自建 anchor 后，对 `A` 调用时不得读到 `B` 的 window（错误 change key 碰撞隔离） | 新增 |
| `runPhaseNext — map 隔离与易失 (AC-5)` | 异常 | `vi.resetModules()` 清空 map 后若 `run_id` 为空/仅空白 → 仍 `missing_run_id`，不得因空 map 放行 | 新增 |
| `runPhaseNext — map 隔离与易失 (AC-5)` | 边界 | 同一 import 实例内连续两次同 `(change, run_id)` 复用 anchor；resetModules 后第三次才重锚定 | 新增 |
| `runPhaseNext — 回归 (AC-7)` | 正向 | 同一 `run_id` 下空 eval → `next_phase === "proposal"`、`round === 1`（既有 First Run） | 新增 |
| `runPhaseNext — 回归 (AC-7)` | 正向 | 同一 `run_id` 下 proposal pass → `next_phase === "dev-design"`（Normal Progression） | 新增 |
| `runPhaseNext — 回归 (AC-7)` | 正向 | 同一 `run_id` 下全部 phase pass → `done === true` | 新增 |
| `runPhaseNext — 回归 (AC-7)` | 正向 | 同一 `run_id` 下最新 entry 含 `backtrack_to` → 返回目标 phase，且无 `updatedEntries` | 新增 |
| `runPhaseNext — 回归 (AC-7)` | 异常 | 同一 `run_id` 窗内 round>20 → 仍停止于 `round_limit_exceeded` | 新增 |
| `runPhaseNext — 回归 (AC-7)` | 异常 | 同一 `run_id` 窗内 5 fail → 仍停止于 `max_retries_exceeded` | 新增 |
| `runPhaseNext — 回归 (AC-7)` | 边界 | mid-phase 中断（仅 prior pass、当前无 evaluator 条目）仍返回未完成 phase | 新增 |
| `runPhaseNext — Round Limit (既有)` | 异常 | 无 `run_id` 的生涯 21 条触顶用例 → 改为带固定 `run_id` 且先建立空窗再堆满 21 条，或废弃后由 AC-2/AC-7 场景替代 | 废弃 |
| `runPhaseNext — Retry Logic (既有)` | 异常 | 生涯累计 5 fail 触顶用例 → 改为同 `run_id` 窗内 5 fail；历史 fail 在窗外的场景改由 AC-3/AC-4 覆盖 | 废弃 |
| `runPhaseNext — Input Validation` | 异常 | `change: ""` 仍抛 `Missing required parameter: change`（与 `missing_run_id` 结构化错误区分） | 新增 |
| `runPhaseNext — Input Validation` | 异常 | `run_id` 为非 string（number / object，cast）→ `missing_run_id` 或等效拒绝，不建立 anchor | 新增 |
| `runPhaseNext — Input Validation` | 边界 | `run_id` 为超长字符串（>1000 chars）且非空 → 接受并按正常 session 编排 | 新增 |
| `runPhaseNext — Input Validation` | 边界 | `run_id` 含 `\n` / emoji 等特殊字符 → 接受；不同特殊串视为不同 key | 新增 |
| `Boundary Scenarios` | 正向 | `run_id: "0"` / 单字符 → 合法非空，可建立 anchor | 新增 |
| `Boundary Scenarios` | 异常 | 重置窗口后传入空 `run_id`（`""` / 仅空白）→ `missing_run_id`，不建立/复用任何 anchor | 新增 |
| `Boundary Scenarios` | 异常 | `run_id` 类型非法（number / null，cast）→ 拒绝且不污染进程内 map | 新增 |
| `Boundary Scenarios` | 边界 | `run_id` 含内嵌 `\0` 与另一 `change` 拼接时仍按独立 key 隔离，不与合法 `(change, run_id)` 碰撞串台 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `fs.existsSync` / `fs.readFileSync` | 沿用现有 `vi.mock('fs')`；`next()` helper 按 change 目录、`workflow.json`、`eval.json` 返回 fixture | 全部 `runPhaseNext` 用例 |
| 进程内 session map | 同文件内默认共享模块实例以测 anchor 复用；AC-5 易失用例使用 `vi.resetModules()` + 动态 `import('../commands/phase-next')` 取得空 map；**不得**依赖任何 map clear 公共导出 | AC-2～AC-5、AC-7 |
| `eval.json` entries | 通过 `passEntry` / `failEntry` / `skippedEntry` / `backtrackEntry` 构造；长历史用循环生成 N 条 | round / retry / 重置 / 回归 |
| `workflow.json` | helper 可选 `workflowType` 写回 JSON；缺省 requirement | test-only 与 requirement 回归 |

---

### `plugins/dev-team/bin/src/mcp.ts` -> `plugins/dev-team/bin/src/mcp.test.ts`

#### 待测功能

- MCP 工具 `phase_next`（`TOOLS` 注册项）: `inputSchema` 为 `phaseNextInputSchema`（含必填 `run_id`）；handler 将 `args.run_id` 传入 `runPhaseNext({ project_root, change, run_id })`
- `connectToServer(transport): Promise<McpServer>`: 注册工具集（既有）；本变更仅扩展 `phase_next` 参数契约

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `MCP schema — phase_next run_id (AC-1)` | 正向 | `phaseNextInputSchema.shape` 含 `run_id`；`listTools` 中 `phase_next.inputSchema.required` 含 `run_id` | 新增 |
| `MCP schema — phase_next run_id (AC-1)` | 异常 | `safeParse({ project_root, change })` 缺 `run_id` → `success === false` | 新增 |
| `MCP schema — phase_next run_id (AC-1)` | 异常 | `safeParse({ ..., run_id: "" })` → `success === false` | 新增 |
| `MCP schema — phase_next run_id (AC-1)` | 异常 | `safeParse({ ..., run_id: "   " })` → `success === false`（`trim().min(1)`） | 新增 |
| `MCP schema — phase_next run_id (AC-1)` | 异常 | `safeParse({ ..., run_id: 123 })` / `null` / `{}` → `success === false`（非法类型） | 新增 |
| `MCP schema — phase_next run_id (AC-1)` | 边界 | `run_id` 超长 / 含 `\n` / emoji 且非空 → schema 通过 | 新增 |
| `MCP 工具调用 — phase_next 传 run_id (AC-7)` | 正向 | `callTool(phase_next, { change, run_id, project_root })` 成功且 spy `runPhaseNext` 收到同一 `run_id` | 新增 |
| `MCP 工具调用 — phase_next 传 run_id (AC-7)` | 异常 | `callTool` 传入非法类型 `run_id`（number / null）→ schema 失败 / `isError`，`runPhaseNext` spy 次数为 0 | 新增 |
| `MCP 工具调用 — phase_next 传 run_id (AC-1)` | 异常 | `callTool` 省略 `run_id` 时 schema 校验失败 / `isError`，`runPhaseNext` spy 次数为 0 | 新增 |
| `MCP 工具调用 — 全 11 handler` | 正向 | 既有「11 个 tool 各 callTool 一次」用例中 `phase_next` args 增加 `run_id`，否则 schema 失败 | 新增 |
| `MCP 工具调用 — 全 11 handler` | 异常 | 同一套 11 handler 调用中 `phase_next` 故意省略 `run_id` → 该条 `isError`，且 `runPhaseNext` spy 次数为 0 | 新增 |
| `MCP 工具调用 — 业务成功路径` | 正向 | 既有「phase_next 返回下一阶段信息」arguments 增加非空 `run_id` | 新增 |
| `MCP 工具调用 — 业务成功路径` | 异常 | 「返回下一阶段信息」路径传入空 `run_id` → 工具失败，不得返回业务 `next_phase` | 新增 |
| `MCP schema — 必填 project_root` | 异常 | 既有 `phase_next` 仅 `{ change: 'c' }` 的 safeParse fixture 改为同时缺 `run_id` 或补最小合法字段集，避免误测 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `runPhaseNext` | `vi.spyOn(phaseNextCmd, 'runPhaseNext')` 断言调用参数含 `run_id`；成功路径可走真实实现 + 临时 change 目录 | handler 接线 / 全 11 handler |
| `InMemoryTransport` + temp project | 沿用 `setupTempProject` / `setResolvedRoot` / `withProjectRoot` | callTool 成功与缺参 |
| `phaseNextInputSchema` | 直接 `safeParse`，不 mock | schema 必填与边界 |

---

## 集成测试

框架：`vite-plus`；skill 静态检查落在 `plugins/dev-team/bin/__tests__/`。

### Skill 文案 → phase_next(run_id) 调用约定 → `plugins/dev-team/bin/__tests__/phase-next-run-id-skills/phase-next-run-id-skills.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/skills/workflow-requirement/SKILL.md` | 编排方：turn 入口生成 `run_id`，LOOP 内传入 `phase_next` |
| `plugins/dev-team/skills/workflow-test-only/SKILL.md` | 编排方：与 requirement 相同的 `run_id` 约定 |
| `plugins/dev-team/skills/phase-proposal/SKILL.md` | 独立 phase 调用方：Phase Check / recall 均带 `run_id` |
| `plugins/dev-team/skills/phase-dev-design/SKILL.md` | 独立 phase 调用方 |
| `plugins/dev-team/skills/phase-test-design/SKILL.md` | 独立 phase 调用方 |
| `plugins/dev-team/skills/phase-implement/SKILL.md` | 独立 phase 调用方 |
| `plugins/dev-team/skills/phase-test-gen/SKILL.md` | 独立 phase 调用方 |
| `plugins/dev-team/skills/phase-test-execution/SKILL.md` | 独立 phase 调用方 |
| `plugins/dev-team/skills/phase-code-review/SKILL.md` | 独立 phase 调用方 |
| `plugins/dev-team/skills/phase-acceptance/SKILL.md` | 独立 phase 调用方 |

**关联AC**: AC-6

**关系描述**:

workflow 与各 phase skill 是 `phase_next` 的唯一生产调用方。引擎强制 `run_id` 后，若文案仍写 `phase_next(change=...)` 而不生成/传入 `run_id`，运行时会全部失败或误复用旧 session。本关系通过读取 SKILL.md 静态文本验证：turn 入口生成约定、`__MCP:phase_next__` 调用形态含 `run_id=`、以及 `[Round {gate.round}/20]` / 完成摘要的 session 语义说明。无法在 Vitest 中驱动真实 agent turn，故以文档契约静态检查代替 E2E。

#### 场景: workflow skill 要求 turn 入口 run_id

验证 `workflow-requirement` 与 `workflow-test-only` 在进入 LOOP 前要求生成非空 `run_id`，且文中所有 `__MCP:phase_next__` 调用包含 `run_id=`；并说明下一用户消息（含 ask-user 回复）须重新生成。前置条件为 skill 文件已按 design 修改。输入为文件正文；预期匹配生成/`run_id=`/session round 文案。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `workflow-requirement/SKILL.md` 含 turn 入口生成 `run_id` 的步骤，且每个 `__MCP:phase_next__` 含 `run_id=` | 新增 |
| 正向 | `workflow-test-only/SKILL.md` 同样要求生成并传入 `run_id` | 新增 |
| 正向 | 两份 workflow skill 均说明 ask-user / 下一用户消息须新生成 `run_id` | 新增 |
| 正向 | 进度行仍为 `[Round {gate.round}/20]`（或等价），并注明 session 语义而非生涯累计 | 新增 |
| 异常 | 不得残留仅 `phase_next__(change=<change-name>)` 且无 `run_id` 的调用形态 | 新增 |

##### Mock策略

<!-- 无跨进程 Mock：直接 fs.readFileSync 读取仓库内 SKILL.md -->

#### 场景: phase skill 独立传入 run_id

验证 8 个 `phase-*` skill 在 Phase Check 与 backtrack 后 recall 的 `phase_next` 均要求本 turn 的 `run_id`，且不依赖 workflow 注入。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 每个 `phase-*/SKILL.md` 正文要求 turn 入口生成 `run_id` | 新增 |
| 正向 | 每个 skill 中全部 `__MCP:phase_next__` 调用含 `run_id=` | 新增 |
| 正向 | backtrack recall 路径（若存在）与初始 Phase Check 使用同一 turn `run_id` 的文案约束存在 | 新增 |
| 异常 | 任一目标 `phase-*/SKILL.md` 残留无 `run_id=` 的 `__MCP:phase_next__` 调用形态 → 测试失败 | 新增 |
| 边界 | 枚举恰好 8 个目标 skill 目录，缺文件则失败 | 新增 |

##### Mock策略

<!-- 无跨进程 Mock：批量读取 skills 目录下 SKILL.md -->

---

### MCP handler → runPhaseNext(run_id) 接线 → `plugins/dev-team/bin/src/mcp.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | 契约：必填非空 `run_id` |
| `plugins/dev-team/bin/src/mcp.ts` | 中间件：校验后将 `run_id` 传入命令层 |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 执行方：session 窗口编排 |

**关联AC**: AC-1, AC-7

**关系描述**:

MCP 层 Zod schema 会在 handler 前拒绝缺/空 `run_id`；handler 成功路径必须把调用方 `run_id` 原样交给 `runPhaseNext`。若只改命令层而不改 schema/handler，客户端仍可能漏参或参数丢失。该关系在既有 `mcp.test.ts`（InMemoryTransport）中扩展断言，确认 schema、callTool 与 spy 参数一致；结构化 `missing_run_id` 以命令层直调单测为主（MCP 缺参通常在 schema 层失败）。

#### 场景: callTool 传递 run_id

验证合法 `project_root` + `change` + `run_id` 时工具成功，且 `runPhaseNext` 收到相同 `run_id`；缺 `run_id` 时不进入命令层。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `callTool(phase_next)` 带非空 `run_id` → 非 error，spy 参数含该 `run_id` | 新增 |
| 异常 | 省略 `run_id` → isError，spy 次数 0 | 新增 |
| 边界 | `run_id` 仅空白 → schema 失败，spy 次数 0 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `runPhaseNext` | spy 记录 args；或临时 change 目录走真实实现 | 正向接线 |
| project root resolve | `setResolvedRoot` / mock resolve 成功 | 全部 callTool |

---

## 不可测试项

- `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` 的 colocated 单元测试路径 — **原因**: `test_resolve_paths` 返回 `Not in test config scope`。契约行为由 `mcp.test.ts` 对 `phaseNextInputSchema.safeParse` / `listTools.inputSchema.required` 覆盖，不单独新建 `phase-next.schema.test.ts`。
- AC-6 真实 agent turn 生成 UUID/`run_id` 并跨多轮 LOOP 调用 — **原因**: 依赖 Cursor/Claude Code 运行时与 LLM 遵守 skill 文案；自动化仅能做 SKILL.md 静态契约检查，完整人在回路续命行为需手动或 staging E2E。
- 生产环境 Node 进程真正退出后的 map 易失 — **原因**: 测试进程内用 `vi.resetModules()` + 动态 re-import 模拟模块级状态清空；不启动独立 OS 进程做跨进程断言（设计已接受且禁止 test-only reset API）。
- `getOrCreateAnchor` / `computeRound` / `checkRetryLimit` / `resolvePhaseNext` 的直接单测 — **原因**: 均为模块私有；通过 `runPhaseNext` 黑盒覆盖，符合「No test-only exports」。
- ask-user「重试」清零预算的产品风险接受项 — **原因**: 属人在回路策略，非确定性代码路径；由 skill 文案（新 `run_id`）与 AC-4 引擎重置用例间接保障。
