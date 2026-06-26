# 设计: remove-phase-prefix

> **变更**: remove-phase-prefix
> **日期**: 2026-06-25
> **基于**: proposal.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Workflow Config | 定义 phase 表、prerequisite 依赖表，是 phase ID 的唯一真实来源 | `plugins/dev-team/bin/src/lib/workflow.ts` | 无 | TypeScript |
| Phase Next Engine | 根据 eval.json 解析下一个待执行 phase；为 agent prompt 注入 `<change>` 和 `<phase>` 参数 | `plugins/dev-team/bin/src/commands/phase-next.ts` | Workflow Config, eval-json lib | TypeScript |
| Phase Log Engine | 将 evaluator 的评估结果写入 eval.json；处理 backtrack_to stale 标记 | `plugins/dev-team/bin/src/commands/phase-log.ts` | Workflow Config, eval-json lib | TypeScript |
| Phase SKILLs (x9) | 每个 phase SKILL 执行 gate check、调用 planner/executor/evaluator、读取 verdict | `plugins/dev-team/skills/phase-*/SKILL.md` | Phase Next Engine (phase_next MCP) | Markdown (agent skill) |
| Workflow SKILLs (x2) | 编排完整工作流：循环调用 phase_next，分派 planner/evaluator，处理 done/error | `plugins/dev-team/skills/workflow-*/SKILL.md` | Phase Next Engine | Markdown (agent skill) |
| Archive SKILL | 归档已完成 change：检查 eval chain 是否通过 | `plugins/dev-team/skills/openspec-archive-change/SKILL.md` | Phase Check (MCP), eval.json | Markdown (agent skill) |
| Evaluator Agents (x9) | 每个 evaluator 对 phase 产出进行逐项检查，调用 phase_log 写入结果 | `plugins/dev-team/agents/*-evaluator.md` | Phase Log Engine (phase_log MCP) | Markdown (agent prompt) |
| Executor Agents (x2) | 执行测试并生成结构化 JSON 报告 | `plugins/dev-team/agents/*-executor.md` | 测试框架 CLI | Markdown (agent prompt) |
| plugin.json | 插件版本号声明 | `plugins/dev-team/.claude-plugin/plugin.json` | 无 | JSON |

### 组件关系图

```
Workflow Config (workflow.ts)
  ├── phase 表 (id, planner, evaluator)
  └── prerequisite 表 (phase_id → phase_id[])
       │
       ▼
Phase Next Engine (phase-next.ts) ──→ Workflow SKILLs ──→ Phase SKILLs
  ├── interpolatePrompt(<change>, <phase>)           │              │
  ├── resolvePhaseNext()                             ▼              ▼
  └── buildPhaseDef() ─────────────────────→ Evaluator Agents ──→ Phase Log Engine
                                              (phase_log)           │
                                                                    ▼
                                                               eval.json
```

---

## 数据流

### 流程描述

**核心变更：** phase ID 从带序号前缀（如 `01-proposal`）变为纯名称（如 `proposal`）。所有硬编码的 phase ID 引用全部替换为无前缀形式。

**Phase ID 数据流：**

1. **定义阶段** — `workflow.ts` 中的 `PHASE_REQUIREMENT` 等数组定义 phase ID；`PHASE_PREREQUISITES` 等对象定义依赖关系。这是 phase ID 的唯一真实来源。
2. **解析阶段** — `phase-next.ts` 的 `resolvePhaseNext()` 读取 eval.json 中的 entries，与 phase 表比对，确定下一个待执行 phase。返回的 `next_phase`、`allowed_backtrack_phases` 均使用无前缀 ID。
3. **注入阶段** — `phase-next.ts` 的 `interpolatePrompt()` 除 `<change>` 外增加 `<phase>` 占位符替换，将当前 phase ID 注入到 agent 的 prompt 字符串中。
4. **门控阶段** — 每个 Phase SKILL 调用 `phase_next`，断言 `result.next_phase` 等于自身 phase ID（如 `"proposal"`），实现 gate check。
5. **评估阶段** — Evaluator agent 从 prompt 上下文中获得 phase ID（通过 `<phase>` 注入），调用 `phase_log` 时以该 ID 作为 phase 参数。backtrack 目标也使用无前缀 ID。
6. **记录阶段** — `phase-log.ts` 的 `handleBacktrackMarking()` 使用无前缀 ID 进行 phase 表查找和 stale 标记。
7. **回溯阶段** — `phase-next.ts` 的 `handleBacktrack()` 使用无前缀 ID 查找 phase 表中的回溯目标。

**`<phase>` 注入流程：**

```
phase-next.ts: buildPhaseDef()
  → evaluator.prompt = interpolatePrompt(template, change)
  → 新逻辑: template 中的 <phase> 被替换为 phase.id
  → agent .md 收到 prompt，其中包含实际 phase ID
  → agent 在 phase_log 调用中直接使用该 ID
```

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| PhaseDefinition | `{ id: string, description: string, planner: PhaseAgentDef|null, evaluator: PhaseAgentDef|null }` | `id` 是 `PHASE_PREREQUISITES` 的键；`id` 是 eval.json entries 中 `phase` 字段的来源 | 内存（workflow.ts 中静态定义） |
| Prerequisite Table | `Record<string, string[]>` — 每个 phase ID 映射到其直接依赖的 phase ID 数组 | 键和数组元素均使用 phase ID | 内存（workflow.ts 中静态定义） |
| EvalEntry (eval.json) | `{ phase: string, verdict: "pass"|"fail", report: string, checklist: CheckItem[], attempt: number, backtrack_to: string|string[]|null, ... }` | `phase` 字段关联到 PhaseDefinition.id | 文件 (eval.json) |
| phase_next 输出 | `{ next_phase: string|null, planner, evaluator, allowed_backtrack_phases, total_phases, phase_index, round, ... }` | `next_phase` 和 `allowed_backtrack_phases` 中的 ID 均为无前缀 ID | 瞬时（MCP 响应） |

**phase ID 变更映射：**

| 旧 ID (前缀) | 新 ID (无前缀) | 影响范围 |
|-------------|---------------|---------|
| `01-proposal` | `proposal` | workflow.ts, phase-next.test.ts, 所有 SKILL, proposal-evaluator |
| `02-dev-design` | `dev-design` | workflow.ts, 测试文件, phase-dev-design SKILL, dev-design-evaluator |
| `02-code-analyze` | `code-analyze` | workflow.ts (test-only), 测试文件, code-analyze-evaluator |
| `03-test-design` | `test-design` | workflow.ts, 测试文件, phase-test-design SKILL, test-design-evaluator |
| `04-test-gen` | `test-gen` | workflow.ts, 测试文件, phase-test-gen SKILL, test-gen-evaluator |
| `05-implement` | `implement` | workflow.ts, 测试文件, phase-implement SKILL, implementation-evaluator |
| `06-unit-test` | `unit-test` | workflow.ts, 测试文件, phase-unit-test SKILL, unit-test-evaluator/executor |
| `07-code-review` | `code-review` | workflow.ts, 测试文件, phase-code-review SKILL, code-review-evaluator |
| `08-integration-test` | `integration-test` | workflow.ts, 测试文件, phase-integration-test SKILL, integration-test-evaluator/executor |
| `09-acceptance` | `acceptance` | workflow.ts, 测试文件, openspec-archive-change SKILL, acceptance-evaluator |

---

## 路由/API 设计

本变更不涉及新增或修改 MCP 工具 API。涉及的现有 MCP 工具签名不变：

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP `phase_next` | `mcp__plugin_dev-team_dev-team__phase_next` | 确定下一个 phase | `{ change: string, workflow_type?: string }` | `{ next_phase, planner, evaluator, allowed_backtrack_phases, ... }` | 无 |
| MCP `phase_log` | `mcp__plugin_dev-team_dev-team__phase_log` | 写入 evaluation 结果 | `{ change, phase, report, checklist, backtrack_to?, skipped? }` | `{ written, phase, attempt }` | 无 |

**变更要点：**
- `phase_next` 的返回值中 `next_phase` 字段使用新 phase ID（如 `"proposal"` 而非 `"01-proposal"`）
- `phase_log` 的 `phase` 参数和 `backtrack_to` 参数均使用新 phase ID
- `allowed_backtrack_phases` 数组中的 `id` 字段使用新 phase ID
- 新增：`interpolatePrompt()` 支持 `<phase>` 占位符，替换为当前 phase 的无前缀 ID

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | phase ID 使用纯名称，不保留序号前缀 | 数组顺序是执行顺序的唯一真实来源，前缀冗余且具误导性；增删 phase 只需修改 workflow.ts 一个文件 | 保留前缀仅作为注释 —— 增加改动量和误解风险，且不能消除多文件同步问题 |
| D2 | SKILL.md 中保留硬编码的 phase ID（无前缀形式） | 每个 phase SKILL 天然知道自己的身份（如 `phase-proposal` 知道自己是 `"proposal"`），这种身份稳定不变；gate check 和 verdict 读取需要明确的 phase ID 断言 | 完全从 `phase_next` 返回值动态获取 —— 增加 SKILL 逻辑复杂度，且 gate check 的"我是谁"语义必须硬编码 |
| D3 | evaluator/executor agent .md 中的 phase ID 通过 `interpolatePrompt` 的 `<phase>` 占位符注入 | agent 从 prompt 上下文动态获取自己的 phase ID，无需硬编码；`phase_log` 调用直接使用注入的值；使 agent 与特定 phase 解耦 | agent 硬编码 phase ID —— 每次增删 phase 需修改所有 agent 文件 |
| D4 | workflow SKILL 中的 `total_phases` 使用 `result.total_phases` 动态值 | `phase_next` 返回的 `total_phases` 是权威来源；消除 `"Total phases: 6"` 等硬编码数字 | 硬编码数字 —— 增删 phase 时易遗漏，如已存在的 `workflow-test-only:105` |
| D5 | 不提供 eval.json 迁移脚本 | 存量 eval.json 中的旧 phase ID 不会与新代码匹配；用户需在升级前结束所有进行中的流程 | 提供迁移脚本自动标记存量旧 phase ID 为 stale —— 增加复杂度，且用户必须手动确认流程已结束，否则会导致数据不一致 |
| D6 | `phase_index`（1-based 序号）保持数值计算，不受 phase ID 去前缀影响 | `phase_index` 由 `phaseTable.findIndex()` 计算得出，与 phase ID 字符串格式无关；去前缀后 `phase_index` 值不变 | 移除 `phase_index` 字段 —— 对 UI 显示和通知不友好 |

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。所有变更仅在现有 TypeScript 代码和 Markdown agent 提示中进行标识符替换。

### 构建/测试依赖

- 无新增构建/测试依赖。

---

## 实现步骤

1. 修改 `workflow.ts`：phase 表中所有 `id` 字段去前缀；prerequisite 表中所有键名和值去前缀
2. 修改 `phase-next.ts`：`interpolatePrompt()` 增加 `<phase>` 替换逻辑
3. 修改 `phase-log.ts`：无硬编码 phase ID 变更（已动态使用 `options.phase`）
4. 修改 9 个 phase SKILL.md：gate check 断言和 verdict 读取路径中的 phase ID 去前缀
5. 修改 2 个 workflow SKILL.md：去除硬编码数字，使用 `result.total_phases` 等动态值
6. 修改 `openspec-archive-change/SKILL.md`：`phase_check` 调用中的 phase 参数去前缀
7. 修改 9 个 evaluator agent .md：`phase_log` 调用的 phase 参数去前缀（使用 `<phase>` 注入或去前缀的 ID）
8. 修改 2 个 executor agent .md：报告中的 phase 字段去前缀（使用 `<phase>` 注入）
9. 更新 `plugin.json` 版本号 patch bump
10. 编译验证 TypeScript 代码通过
11. 全局 grepl 验证无残留 `0[0-9]-` 模式

---

## 待决问题

- 升级策略：不提供迁移脚本，用户需在升级前手动结束所有进行中的 PGE 工作流。此决策已在 D5 中确认。
