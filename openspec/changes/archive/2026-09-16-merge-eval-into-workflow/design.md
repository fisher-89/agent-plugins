# 设计: merge-eval-into-workflow

> **变更**: merge-eval-into-workflow
> **日期**: 2026-09-11

---

## 本轮回溯的增量（相对上一版 design.md）

本轮为用户手动的第二次回溯。需求变更共两项，proposal 阶段已把它们写入 `proposal.md` 与 10 个 spec：

| # | 需求变更 | 提案落点 | 本版 design 的处理 |
|---|---------|---------|-------------------|
| 1 | `workflow.json` 严格化：文件必须存在；缺失 / JSON 非法 / 根非对象 / schema 校验失败一律抛错并指引 `change_create`；删除 `DEFAULT_WORKFLOW_TYPE` 兜底与缺省补建，`change_create` 是唯一创建者 | 读写契约第 3、4 条；AC-13、AC-14 | 沿用上一轮已写入的严格化设计（新增 `workflow.schema.ts`、写路径收紧、`change_list` 容错、`phase-proposal` 停止并指引），**不回退** |
| 2 | 移除 `hooks.ts` 对遗留 `eval.json` 的内置写保护 | 读写契约第 7 条；AC-9 | **本轮回溯修正**：上一版 design 仍写「保留 `eval.json` glob」，与提案相反；本版把组件表、时序图、变更清单、glob 清单、AC-9 对齐行 5 处旧表述统一改为「内置集合 = `workflow.json` + `config.json`」 |

### 相对上一版的差异

| 维度 | 上一版 design.md | 本版 |
|------|-----------------|------|
| hook 内置 glob | `workflow.json` + 遗留 `eval.json` + `config.json` | `workflow.json` + `config.json`；**无** `eval.json` |
| AC-9 对齐 | 「Hook 保留 `eval.json` glob」 | 「`eval.json` 不再受内置写保护；用户仍可经 `write_protection.files` 自行保护」 |
| `hooks.ts` 变更 | 「本回溯不改」 | 复核并保持单 glob（现存代码已符合）；文件内 MUST NOT 出现 `eval.json` |
| `getWorkflowType` 严格化 | 本轮回溯新增 | 保留（与提案一致） |
| 提案 / 规格 | 待同步清单 | proposal 阶段已完成同步（10 个 spec + 提案），本阶段不再改动 |
| 验收标准 | AC-1 ~ AC-14（AC-9 为旧语义） | AC-1 ~ AC-14（AC-9 已改写） |

不变的部分：`eval.json` → `workflow.json.eval` 的合并本体、读优先级、遗留 `eval.json` 只读回退、写后删除遗留文件、`change_create` 不写 `eval` 键、`KNOWN_ARTIFACTS` 不含 `eval.json`、MCP 工具名与入参、`phaseLogSchema` 条目字段、`run_id` 窗口算法、`write_protection` 的用户配置形状。

### 现状核对（实现起点）

- `plugins/dev-team/bin/src/hooks.ts` 现有代码已是单 glob：`**/openspec/changes/**/workflow.json`（reason 提示 `phase_log` / `backtrack` / `change_create`）与 `**/openspec/config.json`，文件内不含 `eval.json` 字面量 —— 与 AC-8 / AC-9 一致，实现阶段只需复核并保持（不得回退为双 glob）。
- `plugins/dev-team/bin/src/lib/change-config.ts` 仍保留 `DEFAULT_WORKFLOW_TYPE = 'requirement'` 与缺文件返回 `{}` 的兜底；`plugins/dev-team/bin/src/lib/eval-json.ts` 的 `writeEvalJson` 仍 `mkdirSync` 并以缺省元数据补建文件 —— **属实现阶段工作**（AC-13、AC-14）。
- `plugins/dev-team/bin/src/mcp.ts` 的 `phase_log` / `backtrack` description、`schemas/config/config.schema.ts` 的 glob 示例、`hooks.ts` 的内置集合均已为新语义；`change_create` 的 description 仍需补「唯一创建者」契约（AC-10）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| workflow.json schema | 定义 `workflow.json` 对象形状与 `workflow_type` 值域；读 / 写校验的唯一来源 | `plugins/dev-team/bin/src/schemas/workflow.schema.ts` | `zod/v4`, `schemas/phase-log.schema.ts` | TypeScript, Zod |
| workflow.json 严格读取 | 文件必须存在 → JSON → 根对象 → schema；返回 `workflow_type`；缺文件 / 非法抛错 | `plugins/dev-team/bin/src/lib/change-config.ts` | `fs`, `path`, `workflow.schema`, `lib/change`, `lib/project-root` | TypeScript |
| Eval 存储读写 | 权威读写 `workflow.json.eval`；遗留 `eval.json` 只读回退；写前要求文件既存且合法；写成功后删除遗留文件 | `plugins/dev-team/bin/src/lib/eval-json.ts` | `fs`, `path`, `workflow.schema`, `phaseLogSchema`, `isPlainObject`, `lib/workflow` | TypeScript, Zod |
| change_create（唯一创建者） | 只写 `{ workflow_type, created }`；不写 `eval` 键、不建 `eval.json`；已存在目录仍拒绝 | `plugins/dev-team/bin/src/commands/change-create.ts` | `fs`, `path`, `kebabCasePattern` | TypeScript |
| phase_log | 校验 checklist / report，经 `appendEntry` 追加到 `workflow.json.eval`；包装读写错误 | `plugins/dev-team/bin/src/commands/phase-log.ts` | `lib/eval-json` | TypeScript |
| phase_next | 先严格解析 `workflow_type`，再只读条目；`anchor = entries.length`；缺文件 / 非法即抛错 | `plugins/dev-team/bin/src/commands/phase-next.ts` | `lib/eval-json`, `lib/change-config`, `lib/workflow` | TypeScript |
| backtrack | 原地改写最新条目 + stale 传播；persist 仅经 `writeEvalJson` | `plugins/dev-team/bin/src/commands/backtrack.ts` | `lib/eval-json`, `lib/change-config` | TypeScript |
| change_list | `artifacts` 不含 `eval.json`；容错读 `workflow_type` 与条目（失败即 `false` / `null`） | `plugins/dev-team/bin/src/commands/change-list.ts` | `lib/eval-json`, `workflow.schema`, `lib/workflow`, `commands/phase-next` | TypeScript |
| PreToolUse 写保护 | 内置 glob = `**/openspec/changes/**/workflow.json` + `**/openspec/config.json`；**不含** `eval.json`（用户可经 `write_protection.files` 自行保护）；MCP 进程内 `fs` 写入不经 hook | `plugins/dev-team/bin/src/hooks.ts` | `glob`（`matchGlob`）, `lib/config` | TypeScript |
| MCP 注册 | `phase_log` / `backtrack` description 指向 `workflow.json`；`change_create` 标明唯一创建者 | `plugins/dev-team/bin/src/mcp.ts` | 各 command handler | TypeScript, MCP |
| Skill / agent 文案 | 评估落盘 = `phase_log` → `workflow.json`；缺文件时禁止手写并停止 | `plugins/dev-team/skills/**`, `plugins/dev-team/agents/*.md` | MCP 工具名 | Markdown |

### 组件协作

```
读（严格）                                    写（要求文件既存且合法）
phase_next / backtrack                        phase_log / backtrack
        |                                             |
        v                                             v
  getWorkflowType(change)                    writeEvalJson(changeDir, entries)
  1. workflow.json 必须存在（否则抛错）        1. workflow.json 必须存在且通过
  2. JSON.parse → 根必须是对象                     workflowFileSchema（否则抛错）
  3. workflowFileSchema.parse                 2. doc.eval = entries（其它键原样保留）
  4. 返回 workflow_type                       3. JSON.stringify(doc, null, 2) + 末尾换行
        |                                     4. 删除遗留 eval.json
        +---------------------+-----------------------+
                              v
                       workflow.json
        { workflow_type, created, eval: EvalEntry[] }

读（容错，不抛错）
change_list → readWorkflowType(changeDir)
  existsSync / JSON.parse / workflowFileSchema.safeParse 任一失败 → null
  → workflow_done: false、latest_phase: null，change 仍列出

Agent Write/Edit/Bash/Shell/StrReplace
        |
        v
  hooks.ts PreToolUse：glob 命中 workflow.json / config.json → deny
  （eval.json 不在内置集合；用户可经 write_protection.files 自行加入）
  （MCP 进程内 Node fs 写入不经过 hook，仍成功）
```

### 校验边界

完整文件校验（`workflowFileSchema`）只用在两处：**类型读取**（`getWorkflowType`）与**写入**（`writeEvalJson`）。条目读取（`readEvalJson`）只校验 `eval` 字段（`workflowEvalSchema`），以便「遗留 `eval.json` 回退」与「与 `workflow_type` 无关的条目读取」保持独立，并使 `change_list` 能容错吞掉异常（AC-12）。`eval.json` 不再是写入目标，因此不再出现在任何校验分支里。

### 读优先级（`readEvalJson`）

1. `workflow.json` 存在且 `eval` 的值是数组（含 `[]`）→ 按 `workflowEvalSchema` 解析并返回；**不再**读取 `eval.json`，**禁止**两源数组合并。
2. 否则若遗留 `eval.json` 存在 → 按现行规则解析（根必须是数组，元素走 `phaseLogSchema`）。
3. 否则 → `[]`。
4. `workflow.json` 根不是对象 / JSON 非法 / `eval` 键存在但不是数组 → 抛错（不回退）；遗留 `eval.json` 根不是数组 → 抛错。

`phase_next` **只读**：即使磁盘上同时存在两文件，也不得写入、删除或改写。

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
  说明列以「（回溯修正）」标注本轮回溯（移除 eval.json 保护）相对上一版的增量；「（回溯新增）」为上一轮回溯（workflow.json 严格化）引入且本版保留；未标注行为最终态不变（实现可能已完成，需按本版复核）。
  下列产物由构建刷新，是生成物而非手改条目，故不列入本表：`plugins/dev-team/bin/dev-team-config.schema.json`（由 config.schema.ts 生成）、`claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/schemas/workflow.schema.ts` | （回溯新增）`workflow.json` 形状的单一来源：`workflowTypeSchema`（`requirement` / `bug-fix` / `refactor` / `test-only`）、`workflowEvalSchema`（`phaseLogSchema` 数组）、`workflowFileSchema`（`z.looseObject`：`workflow_type` 必填枚举、`created` 可选 `YYYY-MM-DD`、`eval` 可选数组、未知键允许）与 `WorkflowFile` 类型 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/change-config.ts` | （回溯新增）删除 `DEFAULT_WORKFLOW_TYPE`；`getWorkflowType(change)` 改为「文件必须存在 → `JSON.parse` → 根必须是对象 → `workflowFileSchema.parse` → 返回 `workflow_type`」，任一失败抛错 | AC-13、AC-14 的读取入口；文件内 MUST NOT 再出现 `'requirement'` 兜底字面量 |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | （回溯新增）`readWorkflowEval` 的数组改经 `workflowEvalSchema.parse`；`writeEvalJson` 删除 `mkdirSync`、`DEFAULT_WORKFLOW_TYPE` 与缺省补齐分支，改为「文件必须存在 + `workflowFileSchema` 校验」；`appendEntry` 不再创建目录 / 文件 | AC-1 ~ AC-5、AC-12、AC-13；导出名 `readEvalJson` / `writeEvalJson` / `appendEntry` 不变 |
| `plugins/dev-team/bin/src/commands/phase-log.ts` | 保留 `读取 workflow.json 失败` / `写入 workflow.json 失败` 包装；注释标明缺文件时内层错误自带 `change_create` 指引 | AC-1、AC-10、AC-13；调用面仍为 `readEvalJson` + `appendEntry`；MUST NOT 直接写 `eval.json` |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | `getWorkflowType(change)` 的抛错**不得**被吞（缺文件 / 非法直接终止）；`Failed to read workflow.json` 与 `round_limit` 文案保留；保持只读 | AC-3、AC-13、AC-14 |
| `plugins/dev-team/bin/src/commands/backtrack.ts` | 同 `phase-next`：类型解析抛错透出；persist 仅 `writeEvalJson`；保留「没有评估条目」文案 | AC-4、AC-13、AC-14 |
| `plugins/dev-team/bin/src/commands/change-list.ts` | （回溯新增）`readWorkflowType` 改用 `workflowFileSchema.safeParse`（删除手工 `isPlainObject` / `typeof` 判断），失败返回 `null`；`KNOWN_ARTIFACTS` 不含 `eval.json`；解析失败 → `latest_phase: null`、`workflow_done: false`，不崩溃 | AC-7、AC-12、AC-14 |
| `plugins/dev-team/bin/src/commands/change-create.ts` | 实现不变（只写 `{ workflow_type, created }`）；补契约注释：唯一创建者、MUST NOT 写 `eval` 键、MUST NOT 创建 `eval.json` | AC-6、AC-13 |
| `plugins/dev-team/bin/src/hooks.ts` | （回溯修正）内置 glob **只**保留 `**/openspec/changes/**/workflow.json`（reason 提示 `phase_log` / `backtrack` / `change_create`，支持 `%s` / `%t`）与 `**/openspec/config.json`；MUST NOT 含 `**/openspec/changes/**/eval.json`；`write_protection.files` 合并逻辑不变 | AC-8、AC-9；现存代码已符合，本轮只需复核并保持（不得回退为「过渡期保护 `eval.json`」） |
| `plugins/dev-team/bin/src/mcp.ts` | `phase_log` / `backtrack` description 已指向 `workflow.json`；`change_create` description 补「`workflow.json` 的唯一创建者；缺文件时读取方报错」 | AC-10；工具名与 input schema 不变 |
| `plugins/dev-team/bin/src/schemas/index.ts` | （回溯新增）导出 `workflowTypeSchema` / `workflowEvalSchema` / `workflowFileSchema` 与 `WorkflowFile` 类型 | 与既有 barrel 约定一致 |
| `plugins/dev-team/bin/src/schemas/change-create.schema.ts` | （回溯新增）`workflow_type` 的 `z.enum([...])` 改为引用 `workflowTypeSchema`（值域与 describe 不变） | 写方与读方同源 |
| `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` | 注释已指向 `workflow.json`；补一句：`getWorkflowType` 严格化后缺文件 / 非法会先抛错 | AC-10；输入字段不变 |
| `plugins/dev-team/bin/src/schemas/change-list.schema.ts` | `artifacts` / `latest_phase` 的 describe 去掉 `eval.json` 文件语义；`workflow_done` describe 补「`workflow.json` 缺失或格式非法时为 false」 | AC-7、AC-14 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | `write_protection.files[].glob` 的 describe 示例已是 `openspec/changes/**/workflow.json`；schema 形状不变 | 已一致，本轮仅复核；构建时经 `generateConfigJsonSchema` 刷新 `plugins/dev-team/bin/dev-team-config.schema.json` |
| `plugins/dev-team/skills/phase-proposal/SKILL.md` | （回溯新增）`### Confirm workflow type`：已存在 → 跳过并进入 Phase Check；不存在 → **停止**并指引（由用户手写 `{ "workflow_type": …, "created": … }`，或改走 `change_create` / `workflow-*` 新建），MUST NOT Write/Edit；删除「`__TOOL_ASK_USER__` 确认后调 `change_create`」旧分支 | AC-11、AC-13 |
| `plugins/dev-team/skills/phase-dev-design/SKILL.md`、`phase-test-design`、`phase-implement`、`phase-test-gen`、`phase-test-execution`、`phase-code-review`、`phase-acceptance` | 保留「Evaluator 未通过 `phase_log` 写入 `workflow.json`（或 `phase_next.last_result` 未更新）」错误文案 | AC-11；已一致，本轮仅复核 |
| `plugins/dev-team/skills/workflow-requirement/SKILL.md` | 保留 `change_create({ workflow_type: "requirement" })`；补「`workflow.json` 由 `change_create` 建立，是 `phase_next` / `backtrack` 的前置文件（不得手写）」 | AC-11、AC-13 |
| `plugins/dev-team/skills/workflow-test-only/SKILL.md` | 同 `workflow-requirement`（类型为 `test-only`） | AC-11、AC-13 |
| `plugins/dev-team/skills/openspec-archive-change/SKILL.md` | `artifacts` 示例不含 `eval.json`，完成性以 `workflow_done` 为准；补「`workflow_done: false` 亦可能因 `workflow.json` 缺失 / 格式非法，此时指引 `change_create`」 | AC-7、AC-11 |
| `plugins/dev-team/agents/proposal-evaluator.md`、`dev-design-evaluator.md`、`test-design-evaluator.md`、`implementation-evaluator.md`、`test-gen-evaluator.md`、`test-execution-evaluator.md`、`code-review-evaluator.md`、`acceptance-evaluator.md`、`code-analyze-evaluator.md` | 保留「经 `phase_log` 写入 `workflow.json`（`eval` 字段）；禁止 Write/Edit/Bash 改 `eval.json` 或 `workflow.json`」；标题 / 步骤名若仍写「Append to eval.json」改为指向 `phase_log` / `workflow.json` | AC-11；已一致，本轮仅复核 |
| `plugins/dev-team/agents/test-execution-executor.md` | 保留「评估落盘（含 skip）由 evaluator 经 `phase_log` 完成，不得直接写 `eval.json` / `workflow.json`」 | AC-11；已一致，本轮仅复核 |
| `plugins/dev-team/agents/test-design-planner.md` | 关系标题示例中的 `CLI参数 → eval.json持久化` 改为 `CLI参数 → workflow.json持久化` | 文案一致性（AC-11 的延伸）；不改 planner 行为 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `getWorkflowType` | `lib/change-config.ts` | 修改 | `function getWorkflowType(change: string): string` | 文件必须存在且通过 `workflowFileSchema`；否则抛错。**不再**缺省 `"requirement"` |
| `readEvalJson` | `lib/eval-json.ts` | 修改 | `function readEvalJson(changeDir: string): EvalEntry[]` | `workflow.json.eval` 经 `workflowEvalSchema` 解析；无该数组时回退遗留 `eval.json`；两者皆无返回 `[]`；文件缺失不抛错 |
| `writeEvalJson` | `lib/eval-json.ts` | 修改 | `function writeEvalJson(changeDir: string, entries: EvalEntry[]): void` | 要求文件既存且合法；把完整 `entries` 写入 `eval` 并保留其它键；成功后删除遗留 `eval.json`；MUST NOT 新建文件 |
| `appendEntry` | `lib/eval-json.ts` | 修改 | `function appendEntry(changeDir: string, entry: EvalEntry): void` | 仍为 `readEvalJson` + push + `writeEvalJson`；不再探测 / 创建目录或文件 |
| `buildEntry` | `lib/eval-json.ts` | 不变 | `function buildEntry(params: BuildEntryParams): EvalEntry` | 纯内存 |
| `computeAttempt` | `lib/eval-json.ts` | 不变 | `function computeAttempt(entries: EvalEntry[], phase: string, explicitAttempt?: number): number` | 纯内存 |
| `markPhaseStale` | `lib/eval-json.ts` | 不变 | `function markPhaseStale(entries: EvalEntry[], phaseId: string, workflowType: string): void` | 纯内存 |
| `validateVerdict` | `lib/eval-json.ts` | 不变 | `function validateVerdict(verdict: string, skipped?: boolean): void` | 纯内存 |
| `validateReportLength` | `lib/eval-json.ts` | 不变 | `function validateReportLength(report: string): void` | 纯内存 |
| `runPhaseLog` | `commands/phase-log.ts` | 修改 | `function runPhaseLog(options: PhaseLogOptions): PhaseLogResult` | 签名不变；错误包装文案指向 `workflow.json`，缺文件指引随内层错误透出 |
| `runPhaseNext` | `commands/phase-next.ts` | 修改 | `function runPhaseNext(options: PhaseNextOptions): PhaseNextResult` | 签名不变；类型解析失败即抛出；只读 `readEvalJson` |
| `runBacktrack` | `commands/backtrack.ts` | 修改 | `function runBacktrack(options: BacktrackOptions): BacktrackResult` | 签名不变；persist 仅 `writeEvalJson` |
| `runChangeList` | `commands/change-list.ts` | 修改 | `function runChangeList(projectRoot: string): ChangeListResult` | 签名不变；`artifacts` 不含 `eval.json`；解析失败不抛错 |
| `runChangeCreate` | `commands/change-create.ts` | 不变 | `function runChangeCreate(name: string, projectRoot: string, workflowType: string): { name: string; path: string }` | 行为不变（唯一创建者）；仅补契约注释 |
| `runProtectFiles` | `hooks.ts` | 不变 | `function runProtectFiles(): void` | `protect-files` CLI 子命令入口；`loadPatterns` 内置集合 = `workflow.json` + `config.json`（无 `eval.json`），用户 `write_protection.files` 追加逻辑不变 |

MCP 工具 `phase_log` / `phase_next` / `backtrack` / `change_list` / `change_create` 的**工具名与输入字段不变**；`cli.ts` 不注册上述命令。

### 类型定义

<!-- 含 interface、type alias、enum、公共 API class、Zod schema 常量 -->

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `workflowTypeSchema` | `schemas/workflow.schema.ts` | 新增 | `z.enum(['requirement', 'bug-fix', 'refactor', 'test-only'])`；`workflow_type` 值域唯一来源（`change-create.schema.ts` 与 `workflowFileSchema` 共用） |
| `workflowEvalSchema` | `schemas/workflow.schema.ts` | 新增 | `z.array(phaseLogSchema)`；`workflow.json.eval` 形状唯一来源 |
| `workflowFileSchema` | `schemas/workflow.schema.ts` | 新增 | `z.looseObject({ workflow_type, created?, eval? })`；`created` 存在时 MUST 匹配 `^\d{4}-\d{2}-\d{2}$`；未知键允许（写入时原样保留） |
| `WorkflowFile` | `schemas/workflow.schema.ts` | 新增 | `type WorkflowFile = z.infer<typeof workflowFileSchema>`；`change-config.ts` 私有读取函数的返回类型 |
| `EvalEntry` | `lib/eval-json.ts` | 不变 | `z.infer<typeof phaseLogSchema>`；条目字段与可选回溯 / stale 字段不变 |
| `changeCreateInputSchema.workflow_type` | `schemas/change-create.schema.ts` | 修改 | 改为引用 `workflowTypeSchema`；值域与 describe 不变 |

### 配置

<!--
  本变更不新增 / 删除任何 config.json、hooks.json、settings.json 的键。
  内置写保护 glob 是 hooks.ts 的代码常量（非用户配置键）；config.schema.ts 仅改 describe 示例字符串，用户 write_protection.files 形状不变。
-->

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `plugins/dev-team/package.json` | 修改 | `string` | `2.10.36` | 按 CLAUDE.md 规则对插件源码改动做 patch bump：HEAD 为 `2.10.35`，暂存区已是 `2.10.36`，本变更保持**单一** bump，不在同一提交内叠加两次 |
| `write_protection.files[].glob` | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 不变（仅 describe 示例） | `string` | — | 用户配置形状与既有用户自定义 glob 均不变；内置集合收窄只影响 `hooks.ts` 的默认集合 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `workflow.json` | `workflow_type: string`（必填，4 值枚举）；`created?: string`（可选，`YYYY-MM-DD`）；`eval?: EvalEntry[]`（可选）；未知键保留 | 每个活跃 change 目录一份；`eval` 即原 `eval.json` 根数组；`change_create` 是唯一创建者 | `openspec/changes/<name>/workflow.json` |
| `EvalEntry` | `phase`、`attempt`、`verdict`、`report`、`checklist`、`timestamp`；可选 `skipped`、`stale`、`backtrack_to`、`backtrack_reason` | 数组元素；新条目由 `phase_log` 追加（无回溯字段）；回溯由 `backtrack` 原地改写 | 仅存在于 `workflow.json.eval`（写入后）；读取时可能来自遗留 `eval.json` |
| 遗留 `eval.json` | 根为 `EvalEntry[]` | 只读回退源；权威 `eval` 数组一旦存在（含 `[]`）即忽略此文件；**不再受内置写保护**（用户可经 `write_protection.files` 自行保护） | 过渡期磁盘文件；`writeEvalJson` 成功后删除。已归档 `openspec/changes/archive/**` 不迁移 |
| Session anchor | 进程内 map：key = `(change, run_id)`，value = `entries.length` | 窗口 `entries[anchor..]`；`entries` 来自 `readEvalJson` | 不落盘；算法不变 |

### `getWorkflowType` 读契约

1. 目标路径 `path.join(getChangeDir(change, getProjectDir()), 'workflow.json')`。
2. 文件不存在 → 抛错（`workflow.json 不存在: <绝对路径>。该文件由 change_create 建立…`）。
3. `JSON.parse` 失败 → 抛错 `workflow.json 解析失败: <SyntaxError.message>`。
4. 根不是对象（含数组、`null`、标量）→ 抛错 `workflow.json 根元素必须是对象，但实际类型为 <typeof>`。
5. `workflowFileSchema.parse(doc)` 失败 → 抛错 `workflow.json 格式非法 (<绝对路径>): <path: message; …>`（涵盖 `workflow_type` 缺失 / 非枚举 / 非字符串、`eval` 非数组、`created` 非 `YYYY-MM-DD`）。
6. 成功 → 返回 `workflow_type`。**任何分支都不再返回缺省值。**

### `writeEvalJson` 写契约

1. 解析目标路径 `path.join(changeDir, 'workflow.json')`；文件不存在 → 抛错 `workflow.json 不存在: <绝对路径>，无法写入评估记录…`（**删除** `mkdirSync` 与文件自动创建）。
2. 读取现有文件，经 `parseWorkflowJson`（根必须是对象）+ `workflowFileSchema.parse` 校验；失败 → 抛错（格式非法即拒绝写入，交用户 / `change_create` 处置）。
3. 设置 `doc.eval` 为传入的**完整**条目数组（覆盖该字段，不与磁盘 `eval.json` 合并）。
4. 保留 `workflow_type`、`created` 及未知键；**不补写**任何缺省元数据。
5. 序列化：`JSON.stringify(doc, null, 2)` + 末尾换行（与现行格式一致）。
6. 写入成功后，若同目录存在 `eval.json`，`unlinkSync` 删除之。
7. MUST NOT 再 `writeFileSync` `eval.json`。

### `change_create` 写契约

写入对象仅为 `{ workflow_type, created }`（`workflow_type` 取自 `workflowTypeSchema`，`created` 为当日 `YYYY-MM-DD`）。MUST NOT 出现 `eval` 键（包括 `"eval": []`），MUST NOT 创建 `eval.json`。已存在目录仍拒绝，且不得补写。该工具是 `workflow.json` 的唯一创建者。

### 严格度与失败行为

| 情形 | `getWorkflowType`（phase_next / backtrack） | `readEvalJson`（条目读取） | `change_list` |
|------|--------------------------------------------|----------------------------|---------------|
| 文件不存在 | 抛错（指引 `change_create`） | 回退遗留 `eval.json`，否则 `[]` | `workflow_done: false`、`latest_phase: null` |
| JSON 非法 / 根非对象 | 抛错 | 抛错（回退路径不适用） | 同上（吞异常） |
| `workflow_type` 缺失 / 非法 | 抛错 | 不影响（只校验 `eval`） | 同上（吞异常） |
| `eval` 缺失 | 正常（类型可用） | 回退遗留 `eval.json`，否则 `[]` | 按条目计算结果 |
| `eval` 非数组 | 正常（类型可用） | 抛错 | 吞异常 → `false` / `null` |
| `created` 缺失 | 正常（可选） | 正常 | 正常 |
| `created` 非 `YYYY-MM-DD` | 抛错 | 正常 | 吞异常 → `false` / `null` |

写入路径（`phase_log` / `backtrack`）叠加 `writeEvalJson` 的前置条件：文件缺失或格式非法即抛错且不写盘（不建目录、不补缺省元数据）。

### 错误文案

| 位置 | 现行 | 改为 |
|------|------|------|
| `change-config.ts` 文件缺失 | （缺省 `requirement`） | `workflow.json 不存在: <绝对路径>。该文件由 change_create 建立，是工作流的前置条件；请通过 change_create 或 workflow-* skill 创建 change，不要手写该文件。` |
| `change-config.ts` 格式非法 | （无） | `workflow.json 格式非法 (<绝对路径>): workflow_type: <zod message>` |
| `change-config.ts` / `eval-json.ts` JSON 非法 | `workflow.json 解析失败: …` | 保持 |
| `change-config.ts` / `eval-json.ts` 根非对象 | `workflow.json 根元素必须是对象…` | 保持 |
| `eval-json.ts` 写时文件缺失 | （以缺省元数据创建文件） | `workflow.json 不存在: <绝对路径>，无法写入评估记录。请先通过 change_create 创建 change。` |
| `eval-json.ts` 权威 `eval` 非数组 | `workflow.json.eval 必须是数组，但实际类型为 …` | 保持 |
| `eval-json.ts` 遗留根非数组 | `eval.json 根元素必须是数组…` | 保持（仅供回退路径） |
| `phase-log.ts` 读 / 写失败 | `读取 / 写入 workflow.json 失败: …` | 保持（缺文件指引随内层消息透出） |
| `phase-next.ts` 读失败 | `Failed to read workflow.json: …` | 保持 |
| `phase-next.ts` round_limit | `请检查 workflow.json 中的 backtrack 记录` | 保持 |
| `backtrack.ts` 读 / 写失败 | `读取 / 写入 workflow.json 失败: …` | 保持 |
| `backtrack.ts` 无条目 | `Phase "…" 没有评估条目，无法设置回溯。请先执行该 phase 并记录评估结果。` | 保持 |
| `change-list.ts` 任何解析失败 | 吞异常 | 保持（`workflow_done: false`、`latest_phase: null`） |

### Hook 内置 glob（本轮回溯修正）

`loadPatterns()` 的内置集合（代码常量，先于用户配置匹配）：

- `**/openspec/changes/**/workflow.json` — reason 提示 `phase_log` / `backtrack` / `change_create`（继续支持 `%s` / `%t`）
- `**/openspec/config.json` — 提示用户自行操作

**不含** `**/openspec/changes/**/eval.json`：未配置 `write_protection` 时，对 `openspec/changes/<name>/eval.json` 的 Write / Edit / Bash / PowerShell / Shell / StrReplace 不再产生内置 deny；需要拦截该遗留文件的项目可经 `write_protection.files` 自行声明（与既有用户 glob 合并逻辑不变）。

MCP 进程内 `fs`（`change_create` / `writeEvalJson`）不经过 PreToolUse，写入仍成功。

### Skill：`phase-proposal` 的 `Confirm workflow type`

- `workflow.json` **已存在** → 跳过类型确认，进入 `### Phase Check`（不变）。
- **不存在** → **停止**，不进入 Phase Check（`phase_next` 会因缺文件报错）：
  - MUST NOT 用 Write/Edit 创建该文件（hook 会 deny，且缺省已移除）。
  - 指引用户二选一：(a) 由用户手动写入 `{ "workflow_type": "<type>", "created": "<YYYY-MM-DD>" }`（与 `workflowFileSchema` 一致）；(b) 保留目录内容，改走 `workflow-requirement` / `workflow-test-only` 等 skill 或直接调用 `change_create` 新建（`change_create` 拒绝已存在目录）。
  - 删除「用 `__TOOL_ASK_USER__` 确认类型后调用 `change_create`」的旧分支（该分支对已存在目录必然失败）。
- 新建 change 仍只通过 `__MCP:change_create__`。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `appendEntry` → `writeEvalJson` 写 `workflow.json.eval`（写契约第 3 条）；`phase-log.ts` 不再触碰 `eval.json`（写契约第 7 条） |
| AC-2 | 写契约第 4 条：设置 `eval` 后原样保留 `workflow_type` / `created` / 未知键，不补写缺省元数据 |
| AC-3 | 读优先级第 1、2 条：`workflow.json` 存在但无 `eval` 键时回退遗留 `eval.json`；`phase_next` 只读 |
| AC-4 | 写契约第 6 条：写成功后 `unlinkSync` 遗留 `eval.json` |
| AC-5 | 读优先级第 1、4 条：`eval` 为 `[]` 即命中权威空数组，忽略遗留文件、禁止合并 |
| AC-6 | `change_create` 写契约：仅 `{ workflow_type, created }`，无 `eval` 键、无 `eval.json`；唯一创建者 |
| AC-7 | `change-list.ts`：`KNOWN_ARTIFACTS` 不含 `eval.json`；`latest_phase` / `workflow_done` 来自 `readEvalJson` |
| AC-8 | Hook 内置 glob 含 `**/openspec/changes/**/workflow.json` → deny；MCP 进程内 `fs` 不受限 |
| AC-9 | Hook 内置集合 = `**/openspec/changes/**/workflow.json` + `**/openspec/config.json`，**不含** `eval.json`；未配置 `write_protection` 时 `openspec/changes/<name>/eval.json` 的 agent 写入不再产生内置 deny；用户仍可经 `write_protection.files` 自行保护（spec：`protect-files-hook` / `eval-json-protection`） |
| AC-10 | `mcp.ts` 的 `phase_log` / `backtrack` description 指向 `workflow.json`；`change_create` description 补唯一创建者契约 |
| AC-11 | `phase-proposal` 无 Write `workflow.json` 步骤 + 缺文件停止并指引；其余 skill / agent 文案指向 `phase_log` → `workflow.json` |
| AC-12 | 严格度表：`eval` 非数组 → `readEvalJson` 抛错；`change_list` 吞异常 → `false` / `null` |
| AC-13 | `workflow.json` 不存在时 `getWorkflowType` 抛错（不返回 `"requirement"`），文案含绝对路径与 `change_create` 指引；`phase_next` / `backtrack` 以可读错误终止；`phase_log` 经 `writeEvalJson` 的前置条件同样失败且**不创建**任何文件 |
| AC-14 | 格式非法（JSON 非法、根非对象、`workflow_type` 缺失 / 非枚举、`eval` 非数组、`created` 非 `YYYY-MM-DD`）时 `getWorkflowType` / `writeEvalJson` 抛错且不写入；`change_list` 对同一文件返回 `workflow_done: false`、`latest_phase: null` 且不崩溃 |

---

## 提案与规格同步状态

本轮回溯的 proposal 阶段已完成下列同步（proposal 已通过评审，本阶段不再改动，仅作为实现与评审的一致性基准）：

| 提案读写契约条款 | 目标语义 | 对应 AC |
|------------------|----------|---------|
| 1 权威存储 | `workflow.json.eval` 为唯一权威数组；`phase_log` / `backtrack` MUST NOT 再创建或更新 `eval.json` | AC-1 |
| 2 读取优先级 | `eval` 数组优先（含 `[]`）；否则遗留 `eval.json`；否则 `[]`；禁止合并 | AC-3、AC-5、AC-12 |
| 3 写入 | 以既存且合法的 `workflow.json` 为前提；缺文件 / 非法即抛错；保留元数据；成功后删除遗留文件 | AC-1、AC-2、AC-4、AC-13 |
| 4 形状与严格类型读取 | `workflow.schema.ts` 为唯一来源；`getWorkflowType` 四步严格校验，删除 `DEFAULT_WORKFLOW_TYPE` | AC-13、AC-14 |
| 5 `change_create` | 唯一创建者；只写 `{ workflow_type, created }`；不写 `eval` 键、不建 `eval.json` | AC-6 |
| 6 `change_list` | `KNOWN_ARTIFACTS` 移除 `eval.json`；缺文件 / 非法 → `null` / `false` 且不抛错 | AC-7、AC-12、AC-14 |
| 7 保护 | 内置 glob 仅 `workflow.json` + `config.json`；移除 `eval.json` | AC-8、AC-9 |
| 8 Skill / agent 文案 | 落盘 = `phase_log` → `workflow.json`；`phase-proposal` 禁止 Write 且缺文件时停止并指引 | AC-11 |

10 个 spec 均已同步：`json-design-schemas`（新增 `workflow.schema` Requirement + 严格读写）、`change-create`、`change-list`、`eval-check-cli`、`pipeline-backtrack`、`pge-workflow-engine`、`protect-files-hook`（内置集合移除 `eval.json`）、`eval-json-protection`（同上，并说明用户可自行保护）、`phase-agents`、`phase-skills`。

design 与 spec 的差异如需覆盖，由归档阶段以 spec delta 为准；本设计不新增提案层未覆盖的要求。

---

## 路由/API 设计

<!-- 本变更为 MCP 工具持久化落点、读取严格度与 hooks 内置保护集合的变更，不涉及 HTTP API。MCP 工具名、必填入参与 `phaseLogSchema` 条目字段均不变。 -->

---

## 依赖

### 运行时依赖

- 无新增。继续使用 Node `fs` / `path`、已有 `zod/v4`（新增使用 `z.object` / `z.looseObject` / `z.enum` / `z.array`）、`isPlainObject`（`utils/type-check.ts`）、`matchGlob`（`lib/glob`）。

### 构建/测试依赖

- 无新增。`config.schema.ts` 的 describe 变更经现有 `build/build-config-schema.ts` 的 `generateConfigJsonSchema` 在构建时刷新 `plugins/dev-team/bin/dev-team-config.schema.json`。
- 新增 `schemas/workflow.schema.ts` 不参与 JSON Schema 生成（`bin/dev-team-config.schema.json` 仅镜像 `configSchema`）。
- 按项目规则（CLAUDE.md）修改插件源码后需 patch bump `plugins/dev-team/package.json` 的 `version`，并执行 `pnpm -C plugins/dev-team run build` 刷新 `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`（组装产物不得手改）。

---

## 待决问题

- **遗留 `eval.json` 的伪造窗口**：其内置写保护已按要求移除，而 `readEvalJson` 仍在 `workflow.json` 无 `eval` 键时回退读取该文件。若后续认为该窗口不可接受，可选：(a) 把回退收紧为「仅当 `workflow.json` 不存在」；(b) 由 `change_create` 写入 `"eval": []` 并同时完成迁移；(c) 过渡期结束后删除回退分支。本变更不做，仅记录（与提案待决问题一致）。
- **`change_create` 是否支持「已存在目录 + 缺 `workflow.json` → 补写元数据」**：本设计不改（保持「已存在即拒绝」，`phase-proposal` 缺文件时指引用户手写或重建）。若实际使用中频繁出现「目录已存在但缺文件」，再开变更放开该语义。
- **是否为 `workflow.json` 生成编辑器用 JSON Schema**（`bin/dev-team-workflow.schema.json` + 文件内 `$schema` 键）：本版不做——运行期校验已由 `workflowFileSchema` 覆盖，生成物无消费方，且写入 `$schema` 会改变 `change_create` 的产物形状与 `workflow.json` 的键集。需要编辑器提示时另开变更。
- **`created` 是否必填**：本版取「可选，存在时必须是 `YYYY-MM-DD`」（无读取方消费该字段，必填会让手工 / 历史文件变成不可恢复的死路）；若后续有消费方，再收紧为必填。
