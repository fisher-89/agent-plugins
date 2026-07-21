# 设计: refactor-backtrack-to-skill

> **变更**: refactor-backtrack-to-skill
> **日期**: 2026-07-17

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| backtrack 命令 | 实现新的 backtrack MCP 工具，唯一修改 eval.json 回溯状态的入口 | `plugins/dev-team/bin/src/commands/backtrack.ts` | eval-json.ts, workflow.ts, change.ts | TypeScript |
| backtrack schema | 定义 backtrack 工具的输入/输出 Zod schema | `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` | zod/v4 | TypeScript |
| phase-log 命令 | 精简后的评估记录命令，移除回溯参数处理 | `plugins/dev-team/bin/src/commands/phase-log.ts` | eval-json.ts, change.ts, schemas | TypeScript |
| phase-next 命令 | 工作流下一阶段调度，新增 `last_result` 字段，移除 evalutor prompt 中的回溯提示 | `plugins/dev-team/bin/src/commands/phase-next.ts` | eval-json.ts, workflow.ts, change.ts, schemas | TypeScript |
| eval-json 库 | eval.json 读写和 entry 构建，移除 BuildEntryParams 中的回溯字段 | `plugins/dev-team/bin/src/lib/eval-json.ts` | schemas, workflow.ts, fs, path | TypeScript |
| MCP 注册 | 注册 backtrack 工具，更新 phase_log 和 phase_next 处理器 | `plugins/dev-team/bin/src/mcp.ts` | commands, schemas, MCP SDK | TypeScript |
| test-execution-evaluator | 纯诊断，移除 6 分支决策树和所有 backtrack_to 设置 | `plugins/dev-team/agents/test-execution-evaluator.md` | — | Markdown (agent spec) |
| code-review-evaluator | 纯诊断，移除回溯设置，改为在 report 中描述设计矛盾 | `plugins/dev-team/agents/code-review-evaluator.md` | — | Markdown (agent spec) |
| acceptance-evaluator | 纯诊断，移除回溯设置，改为在 report 中列出未满足的 AC-ID | `plugins/dev-team/agents/acceptance-evaluator.md` | — | Markdown (agent spec) |
| workflow/phase skills | 统一回溯决策流程，基于 last_result.verdict 做 retry/backtrack/ask-user 三叉决策。不继承 evaluator 的决策树逻辑，所有 skill 使用一致的决策模式 | 10 个 SKILL.md 文件 (workflow + phase) | phase_next MCP, backtrack MCP | Markdown (skill spec) |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、MCP 工具 Handler，私有函数不列入。
  签名格式：TS → function parseImports(file: string): Import[]
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/commands/backtrack.ts` | 新的 backtrack 命令实现。唯一修改 eval.json 回溯状态的入口。读取 eval.json，验证合法性，原地修改指定 phase 最新 entry 的 `backtrack_to` 和 `backtrack_reason`，标记目标 phase 的最新 pass entry 为 stale 并向下游传播，写入 eval.json |
| `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` | backtrack MCP 工具的输入/输出 Zod schema 定义 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | 从 `phaseLogInputSchema` 中移除 `backtrack_to` 和 `backtrack_reason` 字段；保留 `phaseLogSchema` 中这两个字段（用于解析已有 eval.json 条目，向后兼容） | Evaluator 不再通过 phase_log 设置回溯参数。旧 eval.json 仍可被解析 |
| `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | `phaseNextOutputSchema` 中增加 `last_result` 字段：`z.object({ phase: phaseIdSchema, verdict: z.enum(['pass','fail']), report: z.string(), timestamp: z.iso.datetime() }).nullable().describe(...)` | 使 skill 可以直接获取最新评估结果，无需额外读取 eval.json |
| `plugins/dev-team/bin/src/schemas/index.ts` | 导出 backtrack schema (`backtrackInputSchema`, `backtrackOutputSchema`) | 使 MCP 注册模块可导入 |
| `plugins/dev-team/bin/src/commands/phase-log.ts` | 移除 `handleBacktrackMarking()` 函数（第 35-63 行）；移除 `validateBacktrackReason()` 函数（第 69-74 行）；从 `runPhaseLog()`（第 86-134 行）中移除对这两个函数的调用；修改 `buildEntry()` 调用：移除 `backtrack_to` 和 `backtrack_reason` 参数；简化写入逻辑为始终使用 `appendEntry()`（不再有条件分支） | phase_log 不再处理回溯逻辑。所有回溯状态修改由独立 backtrack 工具负责 |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 移除 `buildBacktrackHint()` 函数（第 56-60 行）；移除 `computeAllowedBacktrackPhases()` 函数（第 44-51 行，保留 `allowed_backtrack_phases` 响应字段但由其他方式提供）；从 `buildPhaseDef()` （第 68-96 行）中移除 evaluator prompt 的 `+ backtrackHint` 附加；在所有 response builder 函数中增加 `last_result` 字段；在 `resolvePhaseNext()`（第 356-398 行）中计算 `last_result`——从 entries 中提取最新 entry 的 phase/verdict/report/timestamp 快照 | Evaluator prompt 不再包含回溯提示；响应增加 last_result；`handleBacktrack()`（第 241-288 行）保留但仅读取 eval.json 中旧条目的 `backtrack_to`（新条目不再由 phase_log 写入回溯字段，兼容过渡期） |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 从 `BuildEntryParams` 类型（第 13-23 行）中移除 `backtrack_to` 和 `backtrack_reason`；从 `buildEntry()`（第 79-95 行）中移除这两个字段的处理；保留 `markPhaseStale()` 和 `propagateStale()` 供 backtrack 工具使用 | buildEntry 不再处理回溯字段（由独立 backtrack 工具处理）。`markPhaseStale()` 和 `propagateStale()` 保持公共导出供 backtrack 命令调用 |
| `plugins/dev-team/bin/src/mcp.ts` | 新增 `registerBacktrackTool()` 函数注册名称为 `"backtrack"` 的 MCP 工具；在 `connectToServer()` 中调用 `registerBacktrackTool(server)`；导入 `runBacktrack`、`backtrackInputSchema`、`backtrackOutputSchema` | 新增 backtrack MCP 工具；现有 phase_log 工具注册不变（schema 已在 schema 层更新） |
| `plugins/dev-team/agents/test-execution-evaluator.md` | 移除 Step 4（"Apply diagnostic decision tree"）中所有设置 `backtrack_to` 的分支逻辑（第 70-116 行）；重构 Step 4 为纯诊断步骤：分析失败根因，确定诊断类型，但不输出 `backtrack_to`；Step 5（构建 findings，第 118-137 行）中移除 `backtrack_to` 相关字段描述；Step 6（第 139-143 行）中移除向 phase_log 传递 `backtrack_to` 的指令；新增约束："禁止设置 backtrack_to"；保留"无法判断"分支的诊断逻辑，移除返回 `backtrack_to` 的建议 | Evaluator 不再做回溯路由决策。诊断为纯分析，路由决策由 main agent / skill 负责 |
| `plugins/dev-team/agents/code-review-evaluator.md` | 移除 Step 8（"Backtrack if design contradictions found"）；替换为"If design contradictions found, describe them in report with file:line evidence"；新增约束："禁止设置 backtrack_to" | Evaluator 不再做回溯路由决策 |
| `plugins/dev-team/agents/acceptance-evaluator.md` | 移除 Step 7（"Backtrack if requirements gaps found"）；替换为"If requirements gaps found, list unmet AC-IDs in report"；新增约束："禁止设置 backtrack_to" | Evaluator 不再做回溯路由决策 |
| `plugins/dev-team/skills/phase-test-execution/SKILL.md` | Step 3c 中增加统一回溯决策逻辑：Evaluator 执行后检查 `last_result.verdict`，pass 则完成，fail 则分析 report 做 retry/backtrack/ask-user 三叉决策。**不继承 test-execution-evaluator 的 6 分支决策树**——skill 通过分析 Evaluator 的诊断 report（而非硬编码分支）做路由决策。移除旧有的"Check verdict"和"if backtrack_to is set"逻辑 | 统一回溯决策模式。决策树不迁移到 skill，skill 通过分析诊断 report 做路由 |
| `plugins/dev-team/skills/phase-code-review/SKILL.md` | Step 4 从"Check backtrack"改为统一回溯决策逻辑：检查 `last_result.verdict`，若 fail 且 report 提到设计矛盾，调用 `backtrack(change, "code-review", "dev-design", reason)` | 统一回溯决策模式，接管设计矛盾回溯决策 |
| `plugins/dev-team/skills/phase-acceptance/SKILL.md` | Step 4 从"Check backtrack"改为统一回溯决策逻辑：检查 `last_result.verdict`，若 fail 且 report 提到未满足的 AC-ID，调用 `backtrack(change, "acceptance", "proposal", reason)` | 统一回溯决策模式，接管需求缺口回溯决策 |
| `plugins/dev-team/skills/workflow-requirement/SKILL.md` | Step 2 循环中增加基于 `last_result` 的回溯决策流程：Evaluator 执行后调用 phase_next 获取 `last_result`，若 verdict 为 fail 则进入 retry/backtrack/ask-user 三叉决策 | skill 接管回溯路由决策（workflow 层面） |
| `plugins/dev-team/skills/workflow-test-only/SKILL.md` | 同上，增加统一回溯决策流程。保留现有的 code-bugs-found 报告逻辑 | skill 接管回溯路由决策（workflow 层面） |
| `plugins/dev-team/skills/phase-proposal/SKILL.md` | Step 4（P→E Loop）增加统一回溯决策逻辑：Evaluator 执行后通过 phase_next 获取 `last_result`，基于 verdict 做 retry/backtrack/ask-user 决策 | 统一回溯决策模式 |
| `plugins/dev-team/skills/phase-dev-design/SKILL.md` | 同上。移除 Step 3 "Check backtrack"逻辑（由统一流程替代） | 统一回溯决策模式 |
| `plugins/dev-team/skills/phase-test-design/SKILL.md` | Step 3（P→E Loop）增加统一回溯决策逻辑 | 统一回溯决策模式 |
| `plugins/dev-team/skills/phase-implement/SKILL.md` | Step 3（G→E Loop）增加统一回溯决策逻辑 | 统一回溯决策模式 |
| `plugins/dev-team/skills/phase-test-gen/SKILL.md` | Step 3（G→E Loop）增加统一回溯决策逻辑 | 统一回溯决策模式 |
| `plugins/dev-team/.claude-plugin/plugin.json` | 版本号递增（当前 2.8.13 → 2.9.0） | 插件版本升级（minor，新增功能） |

### 公共函数 / API

<!--
  仅列模块级导出函数、MCP 工具 Handler。私有函数（模块内部、下划线前缀）不列入。
-->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `backtrack` | `plugins/dev-team/bin/src/mcp.ts` | 新增 (MCP 工具) | `backtrack(change: string, phase: string, backtrack_to: string, backtrack_reason: string) -> { modified: boolean, phase: string, target: string }` | 新的 MCP 工具，唯一修改 eval.json 回溯状态的入口。由 skill 在判定需要回溯时调用 |
| `runBacktrack` | `plugins/dev-team/bin/src/commands/backtrack.ts` | 新增 | `runBacktrack(options: BacktrackOptions) -> BacktrackResult` | backtrack 命令核心实现。验证参数合法性，原地修改 entry，标记 stale，写入 eval.json |
| `runPhaseLog` | `plugins/dev-team/bin/src/commands/phase-log.ts` | 修改 | `runPhaseLog(options: PhaseLogOptions) -> PhaseLogResult` | 移除 `backtrack_to`/`backtrack_reason` 参数处理。不再处理回溯逻辑，始终使用 `appendEntry()` |
| `runPhaseNext` | `plugins/dev-team/bin/src/commands/phase-next.ts` | 修改 | `runPhaseNext(options: PhaseNextOptions) -> PhaseNextResult` | 响应增加 `last_result` 字段（最新 entry 快照）；移除 evaluator prompt 中的回溯提示 |
| `registerBacktrackTool` | `plugins/dev-team/bin/src/mcp.ts` | 新增 | `registerBacktrackTool(server: McpServer): void` | 向 MCP 服务器注册 backtrack 工具 |
| `resolvePhaseNext` | `plugins/dev-team/bin/src/commands/phase-next.ts` | 修改 | `resolvePhaseNext(opts: ResolvePhaseNextOptions) -> ResolvePhaseNextResult` | 结果中增加 `last_result` 计算；从 entries 中提取最新 entry 的 phase/verdict/report/timestamp 快照 |

### 类型定义

<!--
  含 interface、type alias、enum、公共 API class
-->

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `BacktrackInput` | `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` | 新增 (z.infer) | backtrack 工具输入：`{ change: string, phase: string, backtrack_to: string, backtrack_reason: string }` |
| `BacktrackOutput` | `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` | 新增 (z.infer) | backtrack 工具输出：`{ modified: boolean, phase: string, target: string }` |
| `PhaseLogOptions` | `plugins/dev-team/bin/src/commands/phase-log.ts` | 修改 | 移除 `backtrack_to` 和 `backtrack_reason` 字段 |
| `PhaseNextResult` | `plugins/dev-team/bin/src/commands/phase-next.ts` | 修改 | 增加 `last_result: { phase, verdict, report, timestamp } \| null` 字段 |
| `PhaseNextOptions` | `plugins/dev-team/bin/src/commands/phase-next.ts` | 不变 | input 类型不变（只接受 `change`） |
| `BuildEntryParams` | `plugins/dev-team/bin/src/lib/eval-json.ts` | 修改 | 移除 `backtrack_to` 和 `backtrack_reason` 字段 |
| `EvalEntry` | `plugins/dev-team/bin/src/lib/eval-json.ts` (从 `phaseLogSchema` 派生) | 不变 | 类型仍包含 `backtrack_to`/`backtrack_reason`（用于读取和解析已有 eval.json 条目，向后兼容） |
| `phaseNextInputSchema` | `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | 不变 | input schema 不变 |
| `phaseNextOutputSchema` | `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | 修改 | 增加 `last_result` 字段；移除不存在的字段（无，仅新增） |
| `phaseLogInputSchema` | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | 修改 | 移除 `backtrack_to` 和 `backtrack_reason` 字段 |
| `phaseLogSchema` | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | 不变 | 保留 `backtrack_to` 和 `backtrack_reason`（向后兼容，用于解析已有 eval.json 条目） |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `plugins/dev-team/.claude-plugin/plugin.json` | 修改 | `"2.9.0"`（当前 2.8.13） | 插件版本递增。minor 版本号 +1（新增 backtrack MCP 工具，属于功能增强） |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| eval.json entry (`EvalEntry`) | `phase`, `attempt`, `verdict`, `report`, `checklist`, `backtrack_to`, `backtrack_reason`, `skipped`, `timestamp`, `stale` | `backtrack_to` 指向同一 eval.json 中其他 entry 的 `phase` 值；`stale` 机制通过 `getDependents()` 传播 | `openspec/changes/<name>/eval.json` (JSON Array) |
| `last_result`（运行时） | `phase`, `verdict`, `report`, `timestamp` | 从 `phase_next` 响应返回，对应 eval.json 最新 entry 的快照 | 运行时计算，不持久化 |

**写入路径变更：**

```
旧写入路径：
  Evaluator → phase_log(backtrack_to, backtrack_reason)
    → runPhaseLog() → handleBacktrackMarking() → markPhaseStale() → propagateStale()
    → buildEntry(含回溯字段) → writeEvalJson() / appendEntry()
  
新写入路径：
  Evaluator → phase_log(无回溯参数)
    → runPhaseLog() → buildEntry(无回溯字段) → appendEntry()
  
  Skill（独立调用）:
    phase_next → 返回 last_result (最新 entry 快照)
    if 判定需要回溯:
      backtrack(change, phase, target, reason)
        → runBacktrack() → 验证合法性
        → 原地修改最新 entry 的 backtrack_to / backtrack_reason
        → markPhaseStale() → propagateStale()
        → writeEvalJson()
```

**读取路径变更：**

```
旧读取路径：
  phase_next → getLatestBacktrackInfo(entries) → handleBacktrack()
    → 若有 backtrack_to → 返回回溯目标 phase 并追加 reasonSuffix
  
新读取路径：
  phase_next → resolvePhaseNext()
    → 仍保留 handleBacktrack() 用于兼容过渡期已存在的旧回溯条目
    → 新增 last_result 计算（始终可用）
    → allowed_backtrack_phases 仍用于给 skill 提供可选回溯目标参考
```

---

## 路由/API 设计

<!--
  本变更涉及 MCP 工具，非 HTTP API。以下列出 MCP 工具变更。
-->

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| `backtrack` | MCP 工具 | 设置指定 phase 最新 entry 的回溯目标和原因（唯一修改 backtrack 状态的入口） | `{ change: string, phase: string, backtrack_to: string, backtrack_reason: string }` | `{ modified: boolean, phase: string, target: string }` | MCP 原生 |
| `phase_log` (更新) | MCP 工具 | 追加评估结果到 eval.json。不再接受 `backtrack_to`/`backtrack_reason` 参数 | `{ change: string, phase: string, report: string, checklist: array, skip?: boolean }` | `{ written: boolean, phase: string, attempt: number }` | MCP 原生 |
| `phase_next` (更新) | MCP 工具 | 返回下一可执行 phase，含最新评估结果快照（`last_result`）。Evaluator prompt 不再包含回溯提示 | `{ change: string }` | `{ done, error, message, next_phase, planner, evaluator, allowed_backtrack_phases, total_phases, phase_index, round, last_result }` | MCP 原生 |

### backtrack 工具验证逻辑

```
runBacktrack({change, phase, backtrack_to, backtrack_reason}):
  1. getChangeDir(change) → 验证 change 存在
  2. getWorkflowType(change) → 获取 workflow type
  3. getPhaseTable(workflowType) → 获取 phase 表
  4. 验证 phase 在 phase 表中存在
  5. 验证 backtrack_to 在 phase 表中的索引 < phase 的索引
  6. readEvalJson(changeDir) → 读取 entries
  7. 查找 phase 的最新 entry（按 timestamp 降序）→ 未找到则报错
  8. 原地修改该 entry：设置 backtrack_to 和 backtrack_reason
  9. markPhaseStale(entries, backtrack_to, workflowType) → 标记目标 phase stale 并传播
  10. writeEvalJson(changeDir, entries)
  11. 返回 { modified: true, phase, target: backtrack_to }
```

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。现有依赖：`zod/v4`、`@modelcontextprotocol/sdk`

### 构建/测试依赖

- 无新增构建/测试依赖

---

## 待决问题

1. **`allowed_backtrack_phases` 响应字段的去留。** 由于 evaluator 不再需要回溯提示（`buildBacktrackHint()` 已移除），`allowed_backtrack_phases` 对 skill 层的参考价值有限——skill 可以通过 `backtrack` 工具的输入验证来确定目标合法性。建议保留该字段（向后兼容，对 skill 有辅助参考作用），本次变更不涉及移除。

2. **`handleBacktrack()` 在 phase_next 中的保留。** 新 eval.json 条目不再由 phase_log 写入 `backtrack_to`，但现有 eval.json 中可能存在旧条目设置了回溯字段。`handleBacktrack()` 保留以兼容这些旧条目，确保过渡期内工作流可正常处理回溯。预计在下一个主要版本中移除。

3. **`backtrack` 工具的 `backtrack_to` 支持数组的决策。** 当前 `phaseLogSchema` 中 `backtrack_to` 类型为 `string | string[] | null`。新的 `backtrack` 工具仅接受 `string` 类型（单目标回溯）。从过往使用来看，`backtrack_to` 数组从未实际使用过。接受此设计，简化实现。

4. **`markPhaseStale` 中的 `workflowType` 参数传递。** `backtrack.ts` 需要读取 `workflow.json` 以获取 workflow type，然后调用 `markPhaseStale()`。这与当前 `phase-log.ts` 中的做法一致（通过 `getWorkflowType()` 获取）。无需额外修改。

5. **覆盖机制的设计确认。** 根据用户指示，SKILL 不需要继承 test-execution-evaluator 中的回溯判断逻辑。Evaluator 的决策树完全移除，不迁移到 skill。Skill 层使用统一的 retry/backtrack/ask-user 三叉决策模式，通过分析 Evaluator 的诊断 report 做路由决策，而非硬编码分支逻辑。所有 Evaluator 保持一致——纯诊断，无特殊回溯逻辑。
