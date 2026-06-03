# 设计: parallel-dev-test-tracks

> **变更**: parallel-dev-test-tracks
> **日期**: 2026-06-03
> **基于**: proposal.md, specs/pge-workflow-engine/spec.md, specs/pipeline-backtrack/spec.md, specs/workflow-orchestration/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Workflow 配置 | 定义 phase 前置依赖图 (`PHASE_PREREQUISITES`)、`getPrerequisites()`/`getDependents()` | `plugins/dev-team/bin/src/lib/workflow.ts` | 无 | TypeScript |
| Eval JSON 管理 | eval.json 读/写/校验；`markPhaseStale()`/`propagateStale()`——标记失效并沿依赖图传播 | `plugins/dev-team/bin/src/lib/eval-json.ts` | workflow.ts (getDependents) | TypeScript |
| phase/next 指令 | 确定下一执行 phase：过滤 stale 条目、检测 backtrack、重试/轮次限制。只读——不修改 eval.json | `plugins/dev-team/bin/src/commands/phase-next.ts` | workflow.ts, eval-json.ts | TypeScript |
| phase/log 指令 | 写入 eval 条目；遇 `backtrack_to` 时调用 `markPhaseStale()` 传播失效 | `plugins/dev-team/bin/src/commands/phase-log.ts` | eval-json.ts, workflow.ts | TypeScript |
| phase/check 指令 | 已弃用的多门控检查——仅保留用于调试 | `plugins/dev-team/bin/src/commands/phase-check.ts` | workflow.ts, eval-json.ts | TypeScript |
| Schema 定义 | Zod 输入/输出 schema，含 `backtrack_to` 数组支持 | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts`, `phase-next.schema.ts` | 无 | TypeScript + Zod v4 |
| MCP 注册 | 注册并暴露 MCP 工具 | `plugins/dev-team/bin/src/mcp.ts` | 所有 commands | TypeScript |

### 组件图

```
workflow.ts                              eval-json.ts
┌────────────────────┐                   ┌──────────────────────────────┐
│ PHASE_PREREQUISITES │                   │ readEvalJson / appendEntry   │
│ getPrerequisites()  │←──── 依赖 ───────│ markPhaseStale()             │
│ getDependents()     │                   │ propagateStale()             │
│ getPriorPhases()    │                   │ checkGate() (modified)       │
│ (保留向后兼容)       │                   │ buildEntry()                 │
└────────────────────┘                   └──────────┬───────────────────┘
        │                                            │
        │ 调用                                       │ 调用
        ▼                                            ▼
┌───────────────────────────────────────────────────────────────┐
│                       命令层 (commands/)                        │
│  ┌─────────────────┐    ┌─────────────────┐   ┌────────────┐   │
│  │  phase-next.ts   │    │  phase-log.ts    │   │phase-check │   │
│  │  (只读决策)       │    │  (写+失效传播)    │   │(已弃用调试) │   │
│  └─────────────────┘    └─────────────────┘   └────────────┘   │
└───────────────────────────────────────────────────────────────┘
        │                            │
        ▼                            ▼
┌───────────────────────────────────────────────────────────────┐
│                     MCP 注册 (mcp.ts)                         │
└───────────────────────────────────────────────────────────────┘
```

---

## 数据流

### 流程描述

**核心原则**：`phase/next` 是只读决策入口，`phase/log` 是唯一的写+失效传播入口。`phase/check` 从工作流循环中移除。

**正常流程（无回溯）**：
1. 工作流循环调用 `phase/next`（MCP 工具）
2. `phase/next` 检查 eval.json：`hasPhasePassed()` 过滤 `stale: true` 条目，按 `getPrerequisites()` 确定依赖是否满足
3. 返回下一未完成 phase 的配置（planner/evaluator agent）
4. 工作流执行 planner → 可选 auto_steps → evaluator
5. evaluator 调用 `phase/log` 写入结果
6. 循环回步骤 1

**回溯流程（有 backtrack_to）**：
1. evaluator 生成失败报告，设置 `backtrack_to`（字符串或数组）
2. `phase/log` 在写入新条目之前调用 `markPhaseStale(target)`：
   - 标记目标的最近 pass 条目 `stale: true`
   - **立即**沿 `getDependents()` 传播到所有下游（传递闭包）
3. 写入新 fail 条目（不含 stale 标记——stale 只标记旧条目）
4. 下次 `phase/next` 调用时，`hasPhasePassed()` 忽略所有 stale 条目
5. 自然的依赖图驱动：只有失效的 phase 及依赖失效的 phase 才会被重做

**依赖图驱动的轨道隔离**：
- Dev 轨道（02-dev-design → 05-implement）和 Test 轨道（03-test-design → 04-test-gen）互不依赖
- 回溯 02 不会影响 test 轨道（除非 test 轨道间接依赖 dev — 如 03 依赖 02）
- 06/07/08 依赖两个轨道，若任一轨道失效则需重做

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| EvalEntry | `phase: string`, `verdict: "pass"\|"fail"`, `stale?: boolean` (默认 false), `backtrack_to?: string \| string[]`, `attempt: number`, `timestamp: string`, `report: string`, `items: Item[]`, `skipped?: boolean`, `findings?: string`, `schema_version: string`, `phase_suffix?: string` | `phase` 关联到 workflow 的 PhaseDefinition；`backtrack_to` 引用目标 phase ID | `eval.json`（JSON 数组，每行一个条目） |
| PhaseDefinition | `id: string`, `pattern: DESIGN\|EXEC\|EVAL-ONLY`, `planner: PhaseAgentDef\|null`, `evaluator: PhaseAgentDef\|null`, `auto_steps: string[]` | `id` 是 phase 的唯一标识符 | 内存（`workflow.ts` 中定义） |
| PhasePrerequisites | Phase ID → `string[]`（前置 phase 列表） | 与 `getDependents()` 互为逆关系 | 内存（`workflow.ts` 中 `PHASE_PREREQUISITES` 表） |

### eval.json 条目示例

```json
// 正常 pass 条目（首次执行）
{"phase":"02-dev-design","timestamp":"2026-06-03T10:00:00.000Z","attempt":1,"verdict":"pass","report":"设计符合要求","items":[],"backtrack_to":null,"schema_version":"1.0"}

// 回溯后的 fail 条目（不含 stale 标记——stale 只标记旧条目）
{"phase":"05-implement","timestamp":"2026-06-03T10:05:00.000Z","attempt":2,"verdict":"fail","report":"实现与设计不符","items":[],"backtrack_to":"02-dev-design","schema_version":"1.0"}

// 被标记为失效的旧 pass 条目（markPhaseStale 修改）
{"phase":"02-dev-design","timestamp":"2026-06-03T10:00:00.000Z","attempt":1,"verdict":"pass","report":"设计符合要求","items":[],"backtrack_to":null,"schema_version":"1.0","stale":true}

// 重做后 pass 的新条目（不含 stale 字段——视为 false）
{"phase":"02-dev-design","timestamp":"2026-06-03T10:10:00.000Z","attempt":2,"verdict":"pass","report":"重新设计符合要求","items":[],"backtrack_to":null,"schema_version":"1.0"}
```

---

## 路由/API 设计

### MCP 工具

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP | `phase/next` | 返回下一待执行 phase（只读） | `change`(string), `workflow_type`(string?) | `{done, error, next_phase, phase_pattern, planner, evaluator, auto_steps, total_phases, phase_index, round}` | MCP |
| MCP | `phase/log` | 写入 eval 条目，触发失效传播 | `change`(string), `phase`(string), `verdict`(pass\|fail), `report`(string), `items`(string-JSON), `backtrack_to`(string\|string[]?), `attempt`(int?), `skipped`(bool?), `findings`(string?) | `{written, phase, attempt}` | MCP |
| MCP | `phase/check` | (已弃用) 多门控检查 | `change`(string), `phase`(string) | `{passed, phase, prior_phases, block_reasons, phase_state, details}` | MCP |

### 关键接口变更

**phase-next.ts `resolvePhaseNext()`**：
- 移除 `clearEntriesFromPhase()` 调用
- 移除 `updatedEntries` 返回值
- 回溯检测：直接返回最早的目标 phase（不修改 eval.json）
- `hasPhasePassed()` 新增 stale 过滤：`!e.stale` 对 `undefined` 返回 `true`（向后兼容）

**phase-log.ts `runPhaseLog()`**：
- 写入条目前：若 `backtrack_to` 非空，调用 `markPhaseStale()` 对每个目标标记+传播
- 移除写入前的 `checkGate()` 调用（gate 逻辑由 phase/next 全权负责）
- `backtrack_to` 参数在 schema 中从 `string` 改为 `string | string[]`

**eval-json.ts 新增函数**：
- `markPhaseStale(entries, phaseId)` — 标记目标 phase 最新 pass 条目 stale + 立即传播
- `propagateStale(entries, phaseId, workflowType?)` — 沿依赖图递归传播 stale
- `checkGate()` 修改 — 按 prerequisites 检查 + 过滤 stale 条目

**workflow.ts 新增导出**：
- `PHASE_PREREQUISITES` — 前置依赖表（`Record<string, string[]>`）
- `getPrerequisites(phaseId, workflowType?)` — 返回前置 phase 列表
- `getDependents(phaseId, workflowType?)` — 返回后置 phase 列表（从 `getPrerequisites()` 推导）

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | stale 标记替代 entry 删除 | 保留完整审计轨迹；下游通过 `propagateStale()` 立即失效，不依赖线性顺序；回溯到 dev 时 test 轨道若间接依赖 dev 也会失效 | **备选：保留 entry 删除**。删除更简单但丢失审计信息；无法区分"从未执行"和"已执行后被回溯清除"；追溯历史困难 |
| D2 | phase/next 改为只读，失效传播集中在 phase/log | 清晰分离职责：phase/next 只读决策，phase/log 写+变异；避免 race condition；测试更容易（phase/next 是纯函数） | **备选：phase/next 处理失效传播**。需要在决策入口混入副作用；多个入口修改同一数据源风险；与"决策+执行"分离原则相悖 |
| D3 | 前置依赖图替代线性顺序（`getPrerequisites`/`getDependents`） | 天然支持并行轨道：dev 和 test 轨道互不依赖；回溯范围精确（只传播 dependents）；依赖图 DAG 保证无环 | **备选：保留线性顺序 + 手动指定可跳过阶段**。需额外逻辑来跳过非依赖阶段；复杂度不降低；对并行执行无帮助 |
| D4 | `backtrack_to` 支持 `string \| string[]` | 支持多轨道回溯场景（如集成测试发现 test 和 impl 都有问题）；向后兼容单字符串格式 | **备选：仅支持字符串，多目标分多次回溯**。需要 evaluator 多次调用 phase/log；工作流增加轮次；用户体验差 |
| D5 | `getDependents()` 从 `getPrerequisites()` 推导（单一真相源） | 保证依赖图一致；添加新 workflow_type 时只需维护一张表 | **备选：维护独立的 prerequisites 和 dependents 两张表**。两表不一致的风险；维护成本翻倍 |
| D6 | `markPhaseStale()` 先标记再传播（两阶段） | 先确保目标 phase 的最新 pass 失效，再传播 downstream；如果传播过程中出错，目标已失效保证至少需要重做 | **备选：一次性递归遍历标记**。实现复杂；出现错误时状态不一致；回滚困难 |
| D7 | `propagateStale()` 标记 dependent 的**所有**条目为 stale（不仅是 pass） | 下游的 fail 条目也必须失效（它们基于失效的上游）；避免"上游已失效但下游 fail 条目看起来有效"的假象 | **备选：只标记 pass 条目**。部分条目仍显示 fail（未完成），但 phase/next 也能跳过；edge case 较多；最简单的就是全部标记 |
| D8 | phase/check 保留但弃用 | 为调试保留独立门控检查能力；现有测试和脚本可能仍引用；移除 MCP 工具是破坏性变更 | **备选：完全移除 phase/check**。破坏性变更风险；调试依赖此工具；保留成本低 |

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。所有变更在现有 TypeScript 代码库内完成（`workflow.ts`, `eval-json.ts`, `phase-next.ts`, `phase-log.ts`, `phase-check.ts`）。

### 构建/测试依赖

- 无新增构建/测试依赖。测试沿用现有 `vite-plus/test` 框架。

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 失效传播遗漏导致状态不一致 | 本应失效重跑的 phase 被跳过 | 中 | `propagateStale()` 使用 visited-set 防无限循环；DAG 保证无环；完整单元测试覆盖所有传播路径 |
| 旧 eval.json 无 stale 字段与新逻辑冲突 | 现有变更行为异常 | 低 | `!e.stale` 对 `undefined` 返回 `true`（视为未失效）；AC-12 覆盖向后兼容性 |
| `backtrack_to` 数组包含无效 phase ID | 传播跳过失效一步 | 低 | phase/log 在调用 `markPhaseStale` 前校验所有目标；无效目标拒绝整个操作 |
| phase/next 相位扫描改为依赖图后性能 | 仅 9 个 phase，无性能问题 | 低 | DAG 最多 9 节点；传递闭包 O(n)；条目数通常 < 100 |
| 并行轨道概念导致 workflow skill 误解 | skill 编写者以为需要处理并行回退 | 低 | 依赖图是**隐式**的——phase/next 自动处理；skill 循环不变：只调 phase/next，不关心内部逻辑 |

---

## 迁移步骤

无需迁移步骤。变更前后 eval.json 格式向后兼容：
- 旧条目无 `stale` 字段 → 视为 `false`
- `backtrack_to` 仍为字符串格式（旧条目）——新代码兼容处理
- `getPriorPhases()` 保留不变——仅新增 `getPrerequisites()`/`getDependents()`
- 所有现有 eval.json 文件在升级后行为不变

唯一的用户可见变化是：workflow 循环不再调用 `phase/check`，但这对用户透明。

---

## 待决问题

- 无。
