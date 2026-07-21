# 提案: refactor-backtrack-to-skill

> **变更**: refactor-backtrack-to-skill
> **日期**: 2026-07-17
> **状态**: 草案

---

## 问题

当前 PGE 工作流中存在三个架构问题：

### 1. 信息不对称导致的路由决策

Evaluator subagent 在评估时只能看到当前 phase 的产物（checklist + report），对全局状态——哪些 phase 已完成、哪些已通过、回溯链历史——一无所知。然而，Evaluator 通过 `phase_log` 的 `backtrack_to` 参数直接决定了工作流路由。这意味着信息最少的角色做出了最重要的导航决策。

### 2. 决策质量不一致

在 9 个 Evaluator agent 中，只有 `test-execution-evaluator` 拥有结构化的回溯决策树（6 分支）。`code-review-evaluator` 和 `acceptance-evaluator` 有模糊的一句话指引。其余 6 个 Evaluator 完全没有回溯逻辑。回溯决策的质量严重依赖于具体 Evaluator 的实现质量，且 `test-execution-evaluator` 因拥有特殊的回溯逻辑使其与其他阶段不一致。

### 3. 无覆盖机制

Main agent（skill）拥有完整的全局上下文和工作流状态，但无法覆盖 Evaluator 的路由决策。

---

## 提案

### 核心原则

**Evaluator = 诊断。Skill = 路由决策。所有阶段保持一致的决策模式。**

Evaluator 只负责判断"什么失败了、为什么失败"，不负责决定"去哪里修复"。路由决策权上移给拥有全局上下文的 skill（main agent）。`test-execution-evaluator` 不拥有特殊权限——其回溯决策树与其他 Evaluator 的决策逻辑一样，完全迁移至 skill 层，由 `phase-test-execution/SKILL.md` 接管。

### 三大 API 变更

**1. `phase_log` — 移除 `backtrack_to` 和 `backtrack_reason` 参数**

Evaluator 调用 `phase_log` 时仅传入：`phase`、`report`、`checklist`。verdict 由 checklist 自动计算。Evaluator 不再设置回溯目标。

具体影响：
- 从 `phase-log.schema.ts` 的 `phaseLogInputSchema` 中移除 `backtrack_to` 和 `backtrack_reason` 字段
- `phaseLogSchema` 中保留这两个字段，用于解析已有 eval.json 条目（向后兼容）
- 从 `phase-log.ts` 中移除 `handleBacktrackMarking()` 和 `validateBacktrackReason()` 函数
- 从 `eval-json.ts` 的 `BuildEntryParams` 中移除 `backtrack_to` 和 `backtrack_reason`

**2. 新增 `backtrack` MCP 工具**

签名：`backtrack(change, phase, backtrack_to, backtrack_reason)`

该工具是唯一能够修改 eval.json 回溯状态的入口。行为：
a. 验证 change 存在
b. 验证 phase 在工作流中存在
c. 验证 backtrack_to 在 phase 表中位于 phase 之前
d. 读取 eval.json
e. 查找 phase 的最新 entry——未找到则报错
f. 原地修改该 entry：设置 `backtrack_to` 和 `backtrack_reason`
g. 标记目标 phase 的最新 pass entry 为 stale 并向下游传播
h. 写入 eval.json
i. 返回 `{ modified: true, phase, target: backtrack_to }`

**与旧行为的关键区别**：不创建新 entry，原地修改已有 entry，因此 `computeRound` 不会被膨胀。

**3. `phase_next` — 响应中增加 `last_result` 字段**

新增字段：`last_result: { phase, verdict, report, timestamp } | null`

返回最近一条 eval entry 的快照，使 skill 无需在 Evaluator 执行后再次读取 eval.json。Skill 可直接根据 `last_result` 的 `verdict` 判断是否需要进入回溯决策流程。

### 统一 Skill 回溯决策流程

所有 phase-* SKILL.md 和 workflow-* SKILL.md 统一采用以下模式。此模式适用于所有 phase，包括 `test-execution`：

```
Evaluator 执行完成后调用 phase_next
phase_next 返回 last_result

if verdict == "pass" → 继续下一 phase
if verdict == "fail":
  1. 分析 last_result.report 确定失败根因
  2. 决策分支：
     - retry：继续循环（phase_next 自然重试，因无 pass entry）
     - backtrack：调用 backtrack(change, phase, target, reason)
     - 请求用户确认：如果非常确信失败可以忽略
       使用 AskUserQuestion "Phase X failed: <reason>. Continue anyway?"
       如果用户同意 → 用户手动修改 eval.json 中 verdict 为 "pass"
       (skill 禁止在失败时自行写入 verdict="pass")
  3. 继续循环
```

### 从 Evaluator 迁移到 Skill 的决策树

- `test-execution-evaluator` 的 6 分支决策树 → `phase-test-execution/SKILL.md`（与其他 phase 的回溯决策模式保持一致，不赋予任何特殊权限）
- `code-review-evaluator` 的"设计矛盾 → dev-design"决策逻辑 → `phase-code-review/SKILL.md`
- `acceptance-evaluator` 的"需求缺口 → proposal"决策逻辑 → `phase-acceptance/SKILL.md`

### Evaluator Agent 变更

三个涉及回溯决策的 Evaluator 统一变更为纯诊断模式：

1. **test-execution-evaluator.md**：移除 Step 4（6 分支决策树）。移除所有 `backtrack_to` 设置。描述从"sets verdict and backtrack_to"改为"sets verdict and diagnoses root cause"。保留诊断逻辑，仅去掉路由决策。新增约束："禁止设置 backtrack_to"[DESIGN DECISION]

2. **code-review-evaluator.md**：移除 Step 8（"Backtrack if design contradictions found"）。替换为"If design contradictions found, describe them in report with file:line evidence"。新增约束："禁止设置 backtrack_to"。

3. **acceptance-evaluator.md**：移除 Step 7（"Backtrack if requirements gaps found"）。替换为"If requirements gaps found, list unmet AC-IDs in report"。新增约束："禁止设置 backtrack_to"。

其余 6 个 Evaluator 无需变更——它们从未设置过 `backtrack_to`。

---

## 能力

### 修改的能力

- `pipeline-backtrack` — 重构回溯机制：`phase_log` 移除回溯参数，新增独立 `backtrack` MCP 工具，`phase_next` 响应增加 `last_result`。Evaluator 角色从"诊断+路由决策"收缩为"纯诊断"，所有 Evaluator 保持一致的行为模式（无特例）。Skill 角色从"被动接收"扩展为"主动决策"。决策树从 Evaluator Agent 迁移到 Skill。

- `pge-workflow-engine` — `phase_next` 输出 schema 增加 `last_result` 字段，使技能层可以直接获取最新评估结果而不必额外读取 eval.json。

---

## 变更范围

### 实现文件

**新增文件：**
- `plugins/dev-team/bin/src/commands/backtrack.ts` — 新的 backtrack 命令实现
- `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` — backtrack 工具输入/输出 schema

**修改文件——基础设施（6 个）：**
- `plugins/dev-team/bin/src/commands/phase-log.ts` — 移除 `handleBacktrackMarking()`、`validateBacktrackReason()`，移除 `backtrack_to`/`backtrack_reason` 参数处理
- `plugins/dev-team/bin/src/commands/phase-next.ts` — 移除 `buildBacktrackHint()`、`computeAllowedBacktrackPhases()` 及相关 prompt 构建；增加 `last_result` 字段支持
- `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` — 从 `phaseLogInputSchema` 移除 `backtrack_to`、`backtrack_reason`；保留 `phaseLogSchema` 中这两个字段（向后兼容）
- `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` — 增加 `last_result` 字段定义
- `plugins/dev-team/bin/src/schemas/index.ts` — 导出 backtrack schema
- `plugins/dev-team/bin/src/lib/eval-json.ts` — `BuildEntryParams` 移除 `backtrack_to`、`backtrack_reason`
- `plugins/dev-team/bin/src/mcp.ts` — 注册 `backtrack` 工具，更新 `phase_log` 处理器

**修改文件——Evaluator Agent（3 个）：**
- `plugins/dev-team/agents/test-execution-evaluator.md` — 移除 Step 4 决策树，改为纯诊断
- `plugins/dev-team/agents/code-review-evaluator.md` — 移除 Step 8 回溯，改为在 report 中描述矛盾
- `plugins/dev-team/agents/acceptance-evaluator.md` — 移除 Step 7 回溯，改为列出未满足的 AC-ID

**修改文件——Skill（10 个）：**
- `plugins/dev-team/skills/workflow-requirement/SKILL.md` — 增加基于 `last_result` 的回溯决策流程
- `plugins/dev-team/skills/workflow-test-only/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-proposal/SKILL.md` — 统一回溯决策逻辑
- `plugins/dev-team/skills/phase-dev-design/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-test-design/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-implement/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-test-gen/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-test-execution/SKILL.md` — 同上，接管 6 分支决策树（与其他 skill 使用相同的决策模式，不赋予特殊权限）
- `plugins/dev-team/skills/phase-code-review/SKILL.md` — 同上，接管设计矛盾回溯
- `plugins/dev-team/skills/phase-acceptance/SKILL.md` — 同上，接管需求缺口回溯

### 测试文件

- `plugins/dev-team/bin/src/commands/backtrack.test.ts` — 新增，覆盖 backtrack 工具的全部场景
- `plugins/dev-team/bin/src/commands/phase-log.test.ts` — 更新，移除 `backtrack_to`/`backtrack_reason` 相关测试用例
- `plugins/dev-team/bin/src/commands/phase-next.test.ts` — 更新，移除 `backtrack_hint` 测试，增加 `last_result` 测试

### 不要修改

- `plugins/dev-team/bin/src/lib/workflow.ts` — phase 表、依赖关系不变
- `plugins/dev-team/bin/src/lib/change-config.ts`
- `plugins/dev-team/bin/src/lib/change.ts`
- 6 个不涉及 `backtrack_to` 的 Evaluator Agent：proposal-evaluator、dev-design-evaluator、test-design-evaluator、implementation-evaluator、test-gen-evaluator、code-analyze-evaluator
- 所有 `.test.ts` 文件以外的测试数据文件（如 fixture、mock 文件）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `phase_log` 移除 backtrack_to/reason | `phaseLogInputSchema` 不再包含 `backtrack_to` 和 `backtrack_reason` 字段；调用 `runPhaseLog({change, phase, report, checklist})` 成功且返回 `{written: true}`；调用 `phaseLogInputSchema.parse()` 传入带回溯字段的对象抛出 Zod 错误 |
| AC-2 | `backtrack` MCP 工具正常运作 | 调用 `backtrack({change, phase:"test-execution", backtrack_to:"test-gen", backtrack_reason:"语法错误"})` 后，eval.json 中 `test-execution` 的最新 entry 的 `backtrack_to` 被设置为 `"test-gen"` 且 `test-gen` 的 pass entry 被标记为 stale |
| AC-3 | `backtrack` 验证回调目标合法性 | `backtrack({change, phase:"test-gen", backtrack_to:"acceptance"})` 抛出错误（acceptance 在 test-gen 之后）|
| AC-4 | `phase_next` 返回 `last_result` | `runPhaseNext({change})` 的响应包含 `last_result` 字段，其值为最新 entry 的快照（或 null）|
| AC-5 | 三个 Evaluator 不再设置 backtrack_to | test-execution-evaluator 的 Step 4（6 分支决策树）已移除，新增"禁止设置 backtrack_to"约束；code-review-evaluator 的 Step 8 已移除，新增相同约束；acceptance-evaluator 的 Step 7 已移除，新增相同约束 |
| AC-6 | Skill 决策流程统一 | phase-test-execution/SKILL.md 包含 6 分支决策树（从 evaluator 迁移，使用与其他 skill 相同的决策模式）；workflow-requirement/SKILL.md 的回调流程包含基于 last_result.verdict 的 retry/backtrack/ask-user 三叉决策 |
| AC-7 | 插件版本升级 | `plugins/dev-team/.claude-plugin/plugin.json` 版本号已递增 |
| AC-8 | 旧 eval.json 兼容性 | 含 `backtrack_to`/`backtrack_reason` 字段的旧 eval.json 条目可被 `readEvalJson()` 无错误解析（Zod 保留未知字段）|

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Skill 回溯决策逻辑错误导致无限循环 | 工作流无法完成，最多 20 轮后报错 | 中 | `phase_next` 保留 `round_limit_exceeded` 保护（20 轮上限）；SKILL.md 中明确三叉决策流程 |
| Evaluator Agent 未完全移除 backtrack_to（模型注意力不足） | 某些 run 中 Evaluator 仍尝试设置 `backtrack_to` | 低 | Schema 层硬性从 `phaseLogInputSchema` 移除字段，Evaluator 即使写入也会被 Zod 拒绝；测试覆盖 |
| 现有 eval.json 数据兼容性问题 | 旧 entry 带有 `backtrack_to`/`backtrack_reason` 字段导致读取失败 | 低 | `phaseLogSchema` 保留这两个字段（标记为 optional）；Zod 默认忽略未知字段 |
| Skill 决策流程编写不一致（10 个 skill 文件） | 部分 skill 遗漏回溯决策逻辑 | 中 | 统一模式文档化；Code Review 阶段检查所有 10 个 skill 的一致性 |
| 从 Evaluator 迁移决策树时遗漏分支 | test-execution 的覆盖率分支丢失 | 低 | 逐分支对照迁移；验收测试覆盖全部 6 种情况 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| backtrack_to/reason 从 phase_log 移除后放在哪 | 新增独立 `backtrack` MCP 工具 | 单一入口，集中验证，避免 phase_log 膨胀 | 1) 继续留在 phase_log（当前方案 - 信息不对称问题）2) 加到 phase_next 作为 override 参数 |
| Skill 如何获取评估结果 | `phase_next` 增加 `last_result` | 零额外开销，skill 无需再次读盘 | 1) Skill 自行读取 eval.json（额外 IO）2) Evaluator 直接返回结果给 skill（破坏 agent 模式） |
| Evaluator Agent 中的决策树迁移到哪 | 迁移到对应 phase 的 SKILL.md | SKILL.md 是 skill 的规范文档，Evaluator 不再具备路由上下文 | 留在 Evaluator 中但通过某种方式传递全局上下文（复杂且脆弱） |
| test-execution-evaluator 的 6 分支决策树是否特殊处理 | **否**——与 code-review 和 acceptance 一样，全部迁移至 skill。Evaluator 仅做纯诊断 | 保持一致的设计模式，消除特例，降低维护成本。Skill 层统一处理路由决策 | 保留 test-execution-evaluator 的部分回溯逻辑（不一致，增加维护负担）[DESIGN DECISION] |
| `phaseLogSchema` 中的 `backtrack_to`/`backtrack_reason` 是否保留 | 保留——但仅用于解析已有条目 | 现有 eval.json 文件包含这些字段，移除会导致解析失败。`phaseLogSchema` 保留字段，`phaseLogInputSchema` 移除 | 从 `phaseLogSchema` 完全移除（需要额外兼容处理）|

### 待决问题

- `markPhaseStale` 中的 `workflowType` 参数传递是否足够健壮？`backtrack` 命令需要自行读取 `workflow.json` 以获取 workflow type，这与当前分散的 `getWorkflowType()` 调用一致。

---
