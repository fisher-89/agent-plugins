# 设计: reorder-test-gen-after-implement

> **变更**: reorder-test-gen-after-implement
> **日期**: 2026-06-22
> **基于**: proposal.md, specs/pge-workflow-engine/spec.md, specs/pipeline-backtrack/spec.md, specs/workflow-orchestration/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Workflow 配置 | 定义 phase 顺序（`PHASE_REQUIREMENT`）、前置依赖（`PHASE_PREREQUISITES`）、`getPrerequisites()` / `getDependents()` | `plugins/dev-team/bin/src/lib/workflow.ts` | 无 | TypeScript |
| Eval JSON 管理 | `markPhaseStale()` / `propagateStale()` 沿依赖图传播失效标记 | `plugins/dev-team/bin/src/lib/eval-json.ts` | workflow.ts (`getDependents`) | TypeScript |
| phase/next 指令 | 按 phase 表线性扫描，返回首个未 pass 的 phase（只读） | `plugins/dev-team/bin/src/commands/phase-next.ts` | workflow.ts (`getPhaseTable`) | TypeScript |
| phase/log 指令 | 写入 eval 条目；遇 `backtrack_to` 时调用 `markPhaseStale()` | `plugins/dev-team/bin/src/commands/phase-log.ts` | eval-json.ts, workflow.ts | TypeScript |
| 单元测试 | 验证阶段顺序、依赖图、调度语义、失效传播 | `workflow.test.ts`, `phase-next.test.ts`, `eval-json.test.ts` | 上述模块 | Vitest |

### 组件图

```
workflow.ts（唯一变更源）
┌─────────────────────────────────────────────────────────────┐
│ PHASE_REQUIREMENT  ──▶ 顺序: …03 → 05 → 04 → 06…           │
│ PHASE_PREREQUISITES ──▶ 04-test-gen: [03, 05]              │
│ getPrerequisites() / getDependents()  （派生，自动更新）      │
│ PHASES / getPhaseIndex()               （派生，自动更新）      │
└──────────────────────────┬──────────────────────────────────┘
                           │
         ┌─────────────────┼─────────────────┐
         ▼                 ▼                 ▼
  eval-json.ts       phase-next.ts      phase-log.ts
  propagateStale()   resolvePhaseNext()  markPhaseStale()
  （行为派生变更）    （无需代码变更）      （行为派生变更）
```

**变更范围极小**：仅修改 `workflow.ts` 中的数组顺序与一行 prerequisite；`phase-next.ts` 与 `eval-json.ts` 无需改代码，行为通过配置派生。

---

## 数据流

### 流程描述

**调整前（当前）**：

```
01-proposal → 02-dev-design → 03-test-design → 04-test-gen → 05-implement → 06-unit-test → …
```

**调整后**：

```
01-proposal → 02-dev-design → 03-test-design → 05-implement → 04-test-gen → 06-unit-test → …
```

**正常推进（无回溯）**：

1. 工作流循环调用 `phase/next`（MCP 工具）
2. `resolvePhaseNext()` 对 `getPhaseTable()` 返回的 phase 表做**线性扫描**，找第一个 `hasPhasePassed()` 为 false 的 phase
3. 因 `05-implement` 在数组中排在 `04-test-gen` 之前，03 完成后调度器自然返回 `05-implement`，而非 `04-test-gen`
4. 05 pass 后，线性扫描继续，返回 `04-test-gen`
5. test-gen-generator 此时可读取真实实现代码，生成对准实际 API 的测试

**并行轨道语义（不变）**：

- `05-implement` 前置仍为 `[02-dev-design]`，不依赖 `03-test-design`
- `04-test-gen` 前置变为 `[03-test-design, 05-implement]`，必须等测试设计与实现均完成
- 数组顺序仍使 `03-test-design` 在 `05-implement` 之前被扫描到（与现有 parallel-dev-test-tracks 行为一致）

**回溯 / 失效传播**：

1. `phase/log` 写入带 `backtrack_to` 的 fail 条目时调用 `markPhaseStale(target)`
2. `markPhaseStale()` 标记目标 pass 条目为 `stale: true`，再调用 `propagateStale()`
3. `propagateStale()` 通过 `getDependents()` 递归标记下游
4. **关键变化**：`getDependents("05-implement")` 现包含 `04-test-gen` 作为**直接** downstream（因 04 新增对 05 的前置依赖）
5. 回溯到 05 时，`04-test-gen` 被直接标记 stale，无需经 06 等间接链路

**进行中变更的 eval.json 兼容（AC-11）**：

- 旧 eval.json 可能已有 `04-test-gen` pass 但无 `05-implement` pass
- 新顺序下线性扫描先遇到 `05-implement`（未 pass）→ 返回 `05-implement`
- 04 的旧 pass 条目保留但暂不参与 06 调度（06 仍需 04+05 均有效 pass；05 未完成时不会推进到 06）

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| PhaseDefinition | `id`, `pattern`, `planner`, `evaluator`, `auto_steps` | `id` 唯一标识 phase；顺序由 `PHASE_REQUIREMENT` 数组位置决定 | 内存（workflow.ts） |
| PhasePrerequisites | Phase ID → `string[]` | 与 `getDependents()` 互为逆关系；04 新增依赖 05 | 内存（`PHASE_PREREQUISITES`） |
| EvalEntry | `phase`, `verdict`, `stale?`, `backtrack_to?`, … | `phase` 关联 PhaseDefinition；`stale: true` 被 `hasPhasePassed()` 忽略 | `eval.json` |

### 依赖图（requirement / refactor）

```
01-proposal
    ├──▶ 02-dev-design ──▶ 05-implement ──┐
    │         │                │           │
    │         └──▶ 03-test-design           │
    │                    │                  │
    │                    └──▶ 04-test-gen ◀─┘
    │                              │
    │                    06 / 07 / 08
    └──▶ 09-acceptance ◀── (via 05)
```

| Phase | Prerequisites（变更后） | Dependents（变更后） |
|-------|------------------------|---------------------|
| 04-test-gen | `[03-test-design, 05-implement]` | `[06, 07, 08]` |
| 05-implement | `[02-dev-design]`（不变） | `[04-test-gen, 06, 07, 08, 09]` |

---

## 路由/API 设计

本变更不新增 MCP 工具或修改 schema；`phase/next`、`phase/log` 的输入/输出结构不变。

| 方法 | 路径 | 描述 | 输入 | 输出 | 变更 |
|------|------|------|------|------|------|
| MCP | `phase/next` | 返回下一待执行 phase | `change`, `workflow_type?` | `{done, next_phase, planner, evaluator, …}` | **行为**：03 后返回 05；05 后返回 04 |
| MCP | `phase/log` | 写入 eval 条目 | 同现有 | `{written, phase, attempt}` | 无变更 |
| MCP | `phase/check` | 已弃用调试工具 | 同现有 | 同现有 | 无变更 |

### phase/next 调度语义（关键场景）

| eval.json 状态 | 期望 `next_phase` |
|----------------|-------------------|
| 01–03 pass，05/04 均未 pass | `05-implement` |
| 01–03 及 05 pass，04 未 pass | `04-test-gen` |
| 04 pass 但 05 仅 stale / 未 pass | `05-implement` |
| 05 pass 但 04 stale | `04-test-gen` |
| 01–05 及 04 pass | `06-unit-test` |
| 全部 9 phase 有效 pass | `done: true` |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **保持 phase 标识符不变**（仍为 `04-test-gen`、`05-implement`） | 避免破坏已有 eval.json 条目、skill gate check、agent 定义与 MCP 引用 | 重新编号为 04-implement / 05-test-gen — 迁移成本高，需改 skill/agent/eval 全链路 |
| D2 | **仅调整 `PHASE_REQUIREMENT` 数组顺序 + `PHASE_PREREQUISITES`** | 与现有架构一致：`workflow.ts` 是唯一真相源；`phase-next` 线性扫描 + `getDependents` 派生传播，无需改 orchestrator 代码 | 在 `resolvePhaseNext()` 中新增显式 prerequisite 检查 — 过度设计，数组顺序已能正确调度 |
| D3 | **不修改 bug-fix 工作流** | bug-fix 无 03/04 phase，顺序与依赖均不受影响 | 同步调整 bug-fix — 无必要 |
| D4 | **不修改 phase skill / agent 定义** | gate check 仍按各自 phase ID 校验；test-gen-generator 已具备读源码能力，顺序调整即可受益 | 修改 test-gen prompt 强制读实现 — 已有能力，无需重复 |
| D5 | **进行中变更需手动补跑 05 后重跑 04** | 旧 eval 中 04 pass 在 05 之前写入时，新逻辑会先要求 05；属预期行为 | 自动迁移 eval.json 删除无效 04 pass — 增加复杂度，且可能丢失有用历史 |

---

## 依赖

### 运行时依赖

- `parallel-dev-test-tracks` 变更已落地 — 提供 `PHASE_PREREQUISITES`、`getDependents()`、`propagateStale()` 基础设施
- OpenSpec CLI / MCP dev-team 插件 — 工作流循环通过 `phase/next` 驱动

### 构建/测试依赖

- Vitest — 运行 `workflow.test.ts`、`phase-next.test.ts`、`eval-json.test.ts`
- 现有 TypeScript 编译链 — 无新包

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 进行中变更 eval.json 已有 04 pass 但无 05 pass | `phase/next` 返回 05 而非 06，需补跑 | 低 | AC-11 文档说明；先完成 05 再重跑 04 |
| 测试断言依赖旧阶段索引（05 index=4, 04 index=3） | CI 失败 | 中 | 同步更新 `workflow.test.ts`、`phase-next.test.ts` 全部 04/05 相关断言 |
| 开发者误以为 04 仍在 05 之前执行 | 误解工作流 | 低 | spec 依赖图与本 design 明确新顺序 |
| test-gen 在实现后生成覆盖手写测试 | 意外 diff | 低 | test-gen-generator 已有 skip/TODO 约束；evaluator 对照 test-design.md |

---

## 迁移步骤

1. 合并代码后，`PHASE_REQUIREMENT` 与 `PHASE_PREREQUISITES` 自动生效
2. 对于**进行中**的 requirement/refactor 变更：
   - 若 eval.json 中 `04-test-gen` 已 pass 但 `05-implement` 未 pass → 下次 `phase/next` 将返回 `05-implement`
   - 完成 05 后，`phase/next` 返回 `04-test-gen` 要求重跑（基于真实实现重新生成测试）
3. bug-fix 变更无需任何操作
4. 升级 `plugin.json` 版本号以标记插件行为变更

---

## 待决问题

- 无。本变更为配置层重排，scope 明确且无外部 API 变更。
