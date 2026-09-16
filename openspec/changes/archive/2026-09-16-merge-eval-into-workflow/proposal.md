# 提案: merge-eval-into-workflow

> **变更**: merge-eval-into-workflow
> **日期**: 2026-09-11
> **状态**: 草稿

---

## 问题

每个 OpenSpec change 目前把工作流元数据与评估历史拆成两个中间产物：

- `openspec/changes/<name>/workflow.json` — 对象，含 `workflow_type` 与 `created`（由 `change_create` 写入；`getWorkflowType()` / `phase_next` / `backtrack` 读取）
- `openspec/changes/<name>/eval.json` — **数组**，元素为 `phaseLogSchema` 条目（由 `phase_log` 追加、`backtrack` 原地改写；`phase_next` / `change_list` 只读）

两套文件导致：change 目录多一个必须保护的产物；`change_list` 把 `eval.json` 当成 artifact；归档 skill、hooks、MCP 描述与大量测试都要同时理解两套路径。评估历史本就是该 change 工作流的一部分，不应独立成文件。

与之相邻的两处约束也需要在同一次变更里收紧：

- `getWorkflowType()` 在 `workflow.json` 缺失时缺省返回 `"requirement"`，于是一个 `test-only` / `bug-fix` 的 change 只要文件缺失，就会被按 `requirement` 阶段表推进，门禁形同失效；写入路径还会以缺省元数据自动补建文件与目录，使 `change_create` 之外出现第二个「创建者」。文件存在性与格式应作为工作流的前置条件被显式校验，而不是被静默兜底。
- 内置写保护在过渡期同时保护 `workflow.json` 与遗留 `eval.json`。评估历史既已收敛到 `workflow.json`，继续保护旧文件会让保护面与真实数据源长期不一致，应移除 `eval.json` 的 glob，只保留 `workflow.json` 与 `config.json`。

---

## 提案

将 `eval.json` 的条目数组并入同一份 `workflow.json`，作为对象字段 `eval`。条目形状不变（仍为 `phaseLogSchema`）。MCP 工具名与入参不变；`readEvalJson` / `writeEvalJson` 导出名称保留，内部改为读写 `workflow.json.eval`。

同时把 `workflow.json` 从「可选、缺失即兜底」提升为工作流的**前置文件**：新增 `bin/src/schemas/workflow.schema.ts` 作为其形状的唯一来源，`getWorkflowType()` 与 `writeEvalJson()` 在文件缺失或格式非法时抛错（删除 `DEFAULT_WORKFLOW_TYPE` 兜底与自动补建），`change_create` 成为该文件的唯一创建者。内置写保护随之收缩为 `workflow.json` + `config.json` 两个 glob（移除遗留 `eval.json`）。

合并后的文件形态：

```json
{
  "workflow_type": "requirement",
  "created": "2026-09-11",
  "eval": [
    {
      "phase": "proposal",
      "attempt": 1,
      "verdict": "pass",
      "report": "…",
      "checklist": [],
      "timestamp": "2026-09-11T00:00:00.000Z"
    }
  ]
}
```

其中 `workflow_type` 为必填枚举（`requirement` / `bug-fix` / `refactor` / `test-only`），`created` 可选，`eval` 为可选数组。`workflow.json` 由 `change_create` 建立，是 `phase_next` / `backtrack` / `phase_log` 的前置文件——**缺失或格式非法即报错，不再缺省**。

### 读写契约

1. **权威存储**：`workflow.json` 的 `eval` 字段（数组）。`phase_log` / `backtrack` MUST NOT 再创建或更新 `eval.json`。
2. **读取优先级**（`readEvalJson(changeDir)`）：
   - `workflow.json` 存在且 `eval` 为数组（含 `[]`）→ 按 `phaseLogSchema` 解析该数组，**不再**读取 `eval.json`
   - 否则若遗留 `eval.json` 存在 → 按现行逻辑解析为数组（兼容进行中的 change）
   - 否则 → `[]`
   - `workflow.json` 根非对象 / JSON 非法 / `eval` 存在但不是数组，或遗留 `eval.json` 根元素不是数组 → 抛错（与现行 `eval.json` 校验同级）
3. **写入**（`writeEvalJson` / `appendEntry`）：**MUST 以既存且合法的 `workflow.json` 为前提**。
   - 文件缺失 → 抛错并指引 `change_create`；MUST NOT `mkdirSync`、MUST NOT 以缺省元数据补建文件（删除 `DEFAULT_WORKFLOW_TYPE` 兜底与目录自动创建）
   - 文件存在 → 经 `workflowFileSchema` 校验（见下条）；失败即抛错、**不写入**
   - 把 `eval` 置为传入的**完整**条目数组；**保留** `workflow_type`、`created` 及未知键，MUST NOT 补写任何缺省元数据
   - 成功写入后，若同目录仍有 `eval.json`，SHALL 删除之，避免双源
4. **`workflow.json` 形状与严格类型读取**：新增 `bin/src/schemas/workflow.schema.ts` 作为唯一来源——`workflow_type` 为必填的 4 值枚举（`requirement` / `bug-fix` / `refactor` / `test-only`），`created` 可选且存在时必须匹配 `YYYY-MM-DD`，`eval` 为可选数组，未知键允许。`getWorkflowType()` 改为：文件 MUST 存在 → `JSON.parse` → 根 MUST 为对象 → `workflowFileSchema.parse`；任一失败即抛错（文案含绝对路径与 `change_create` 指引）。**不再缺省 `"requirement"`**。
5. **`change_create`**：是 `workflow.json` 的**唯一创建者**；仍只写 `{ workflow_type, created }`，**省略** `eval` 键（避免显式 `[]` 挡住遗留 `eval.json` 的回退读取）；MUST NOT 创建 `eval.json`。
6. **`change_list`**：`KNOWN_ARTIFACTS` 移除 `eval.json`；`latest_phase` / `workflow_done` 改为经 `readEvalJson` 读取（即 `workflow.json.eval`，含遗留回退）。无评估条目时 `latest_phase` 为 `null`、`workflow_done` 为 `false`（语义与「无 eval.json」相同）；`workflow.json` 缺失或格式非法时同样为 `null` / `false`，**不抛错**（change 仍出现在列表中）。
7. **保护**：PreToolUse 内置 glob **只**保留 `**/openspec/changes/**/workflow.json` 与 `**/openspec/config.json`；**删除** `**/openspec/changes/**/eval.json`（评估历史已收敛到 `workflow.json`，遗留文件仅作只读回退）。`workflow.json` 仅允许 MCP/`change_create` 经 Node `fs` 写入。
8. **Skill / agent 文案**：`phase_log` 写入目标改为 `workflow.json`；`phase-proposal` MUST NOT 再用 Write 工具手写 `workflow.json`——文件已存在则跳过类型确认，**缺失则停止**并指引（由用户手写，或改走 `change_create` / `workflow-*` skill 新建），不再依赖缺省 `requirement` 继续门禁。

MCP `phase_log` / `backtrack` / `phase_next` 的输入 schema、phase 表、`phaseLogSchema` 条目字段、session `run_id` 窗口算法均不变；窗口仍按 **条目数组长度** 计 `anchor`，只是数组来源改为 `workflow.json.eval`。

---

## 能力

### 新增能力

（无）

### 修改的能力

- **json-design-schemas** — 评估历史从独立 `eval.json` 数组文件迁入 `workflow.json.eval`；条目 schema 不变；新增 `workflow.schema`（`workflowTypeSchema` / `workflowEvalSchema` / `workflowFileSchema`）作为 `workflow.json` 形状的唯一来源；规定读优先级、写入要求文件既存且合法、写成功后删除遗留 `eval.json`
- **change-create** — 创建 change 时仍只写 `workflow_type`/`created`；不创建 `eval.json`；不写空 `eval` 数组；**是 `workflow.json` 的唯一创建者**，缺文件时读取方报错而非缺省
- **change-list** — artifacts 不再列出 `eval.json`；`latest_phase` / `workflow_done` 从 `workflow.json.eval`（含遗留回退）计算；`workflow.json` 缺失或格式非法时返回 `null` / `false` 且不抛错
- **eval-check-cli** — `phase_log` 追加到 `workflow.json.eval`；归档前校验读同一存储（现行为 `change_list.workflow_done`）
- **pipeline-backtrack** — `backtrack` 原地改写 `workflow.json.eval` 中的最新条目；前置条件为 `workflow.json` 既存且通过 schema
- **pge-workflow-engine** — `phase_next` 只读 `workflow.json.eval`；session anchor 仍为条目数组长度；`workflow.json` 缺失或格式非法时抛错（不再缺省 `requirement`）
- **protect-files-hook** — 内置保护集合改为 `workflow.json` + `config.json`；**移除** `eval.json` 内置保护
- **eval-json-protection** — 与 hook 对齐：内置保护集合不含 `eval.json`
- **phase-agents** — evaluator 经 `phase_log` 写入 `workflow.json`，不再要求直接写 `eval.json`
- **phase-skills** — skill 错误/归档文案与 `phase-proposal` 禁止 Write `workflow.json`；缺文件时停止并指引，不再依赖缺省

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/schemas/workflow.schema.ts`（**新增**）— `workflow.json` 形状的唯一来源：`workflowTypeSchema`、`workflowEvalSchema`、`workflowFileSchema`（`z.looseObject`）与 `WorkflowFile` 类型
- `plugins/dev-team/bin/src/schemas/index.ts` — 导出上述 schema 与类型
- `plugins/dev-team/bin/src/schemas/change-create.schema.ts` — `workflow_type` 的枚举改为引用 `workflowTypeSchema`（值域与 describe 不变）
- `plugins/dev-team/bin/src/lib/change-config.ts` — 删除 `DEFAULT_WORKFLOW_TYPE`；`getWorkflowType()` 改为「文件必须存在 → JSON → 根必须是对象 → `workflowFileSchema.parse` → 返回 `workflow_type`」，任一失败抛错（含绝对路径与 `change_create` 指引）
- `plugins/dev-team/bin/src/lib/eval-json.ts` — `readEvalJson` / `writeEvalJson` / `appendEntry` 改为 `workflow.json.eval`；遗留 `eval.json` 只读回退；写成功后删除 `eval.json`；**写入要求文件既存且合法**（删除 `mkdirSync`、`DEFAULT_WORKFLOW_TYPE` 与缺省补齐分支）
- `plugins/dev-team/bin/src/commands/phase-log.ts` — 错误文案改为 workflow.json；调用面仍用 `readEvalJson`/`appendEntry`；缺文件指引随内层错误透出
- `plugins/dev-team/bin/src/commands/phase-next.ts` — 类型解析抛错不被吞（缺文件 / 非法即终止）；读错误文案；`round_limit` 提示中的文件名；保持只读
- `plugins/dev-team/bin/src/commands/backtrack.ts` — 读/写/缺条目错误文案指向评估存储，不再写 `eval.json`；persist 仅 `writeEvalJson`
- `plugins/dev-team/bin/src/commands/change-list.ts` — `KNOWN_ARTIFACTS` 去掉 `eval.json`；`readWorkflowType` 改用 `workflowFileSchema.safeParse`，失败返回 `null` 且不抛错
- `plugins/dev-team/bin/src/commands/change-create.ts` — 确认不写 `eval` 键、不创建 `eval.json`；补「唯一创建者」契约注释
- `plugins/dev-team/bin/src/hooks.ts` — 内置 glob **只**保留 `**/openspec/changes/**/workflow.json`（reason 提示 `phase_log` / `backtrack` / `change_create`）与 `**/openspec/config.json`；**删除** `**/openspec/changes/**/eval.json` glob
- `plugins/dev-team/bin/src/mcp.ts` — `phase_log` / `backtrack` 工具 description 改为写入 `workflow.json`；`change_create` description 补「唯一创建者」
- `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` / `change-list.schema.ts` — 注释与 `artifacts` / `latest_phase` 描述去掉 `eval.json` 文件语义；补「`workflow.json` 缺失 / 非法时 `workflow_done: false`」
- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — `write_protection.files[].glob` 的示例字符串可改为 `workflow.json`（schema 形状不变）
- `plugins/dev-team/skills/phase-proposal/SKILL.md` — 删除「Write `workflow.json`」步骤；缺文件时**停止并指引**（不手写、不再依赖缺省）
- `plugins/dev-team/skills/phase-*/SKILL.md`、`workflow-*/SKILL.md`、`openspec-archive-change/SKILL.md` — 「写入 eval.json」类文案改为 `phase_log` → `workflow.json`；补充「`workflow.json` 由 `change_create` 建立，是 `phase_next` / `backtrack` 的前置文件」
- `plugins/dev-team/agents/*-evaluator.md` — 如 `test-execution-evaluator.md` 的「Append to eval.json」改为经 `phase_log` 写入 `workflow.json`

### 测试文件

- `plugins/dev-team/bin/src/lib/eval-json.test.ts` — 覆盖读 `workflow.json.eval`、遗留 `eval.json` 回退、`eval` 优先于遗留文件、写时保留 `workflow_type`/`created`、写后删除 `eval.json`、`eval` 非数组抛错；**写入时文件缺失 / 格式非法抛错且不创建文件**
- `plugins/dev-team/bin/src/lib/change-config.test.ts` — 缺文件抛错（含 `change_create` 指引）；JSON 非法 / 根非对象 / `workflow_type` 缺失或非枚举 / `created` 非 `YYYY-MM-DD` 抛错；含 `eval` 数组的 `workflow.json` 仍能解析 `workflow_type`
- `plugins/dev-team/bin/src/commands/phase-log.test.ts` — 持久化断言改为 `workflow.json`；缺 `workflow.json` 时失败且不新建文件
- `plugins/dev-team/bin/src/commands/phase-next.test.ts` — fs mock 从 `eval.json` 改为（或同时覆盖）`workflow.json.eval`；保留遗留文件回退用例；缺文件 / 非法时抛错
- `plugins/dev-team/bin/src/commands/backtrack.test.ts` — 错误文案；持久化目标；缺文件 / 非法时抛错
- `plugins/dev-team/bin/src/commands/change-list.test.ts` — artifacts 不含 `eval.json`；无评估条目 / 非法 eval 字段 / `workflow.json` 缺失或格式非法时 `workflow_done`/`latest_phase` 行为（不抛错）
- `plugins/dev-team/bin/src/commands/change-create.test.ts` — 不产生 `eval.json`；`workflow.json` 无 `eval` 键；写入的 `workflow_type` 通过 `workflowFileSchema`
- `plugins/dev-team/bin/src/hooks.test.ts` — Write/Bash/PowerShell 拦截 `workflow.json`；**`eval.json` 不再被内置规则 deny**；`config.json` 仍 deny
- `plugins/dev-team/bin/src/mcp.test.ts` — 工具 description；`workflow_done` 在无评估条目时为 false
- `plugins/dev-team/bin/__tests__/backtrack-reason-flow/`、`backtrack-reason-compat/`、`phase_log workflow-aware backtrack rejection/` — mock 路径与错误文案

### 不要修改

- MCP 工具名与输入字段（`phase_log` / `phase_next` / `backtrack` / `change_list` / `change_create` 的必填参数）
- `phaseLogSchema` / `phaseIdSchema` 条目字段与 phase 枚举
- `getPhaseTable` / 各 `workflow_type` 的 phase 表与前置依赖
- `run_id` session 窗口算法（只改条目数组的磁盘来源）
- `write_protection` 的用户配置形状（`files[].glob` / `files[].reason`）与既有用户自定义 glob —— 只改内置默认集合
- 遗留 `eval.json` 的回退读取分支本身（本变更保留该分支，只删除其内置写保护）
- `claude-plugins/`、`cursor-plugins/`、`cursor-home-image/` 组装产物（由 `plugins/dev-team` 构建刷新）
- `openspec/changes/archive/**` 中已归档的历史 `eval.json` / `workflow.json`（只读文物，不批量迁移、不补写元数据）
- 架构 MCP（`archi_*`）、测试执行/覆盖率报告路径

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `eval-json.ts` 读写 `workflow.json.eval` | `phase_log` 成功后 change 目录存在 `workflow.json.eval` 数组新条目，且 **不** 创建/更新 `eval.json` |
| AC-2 | 写入保留元数据 | 已有 `{ workflow_type, created }` 的文件在追加 eval 后两字段值不变，未知键仍在 |
| AC-3 | 遗留 `eval.json` 只读回退 | 仅有旧 `eval.json`、`workflow.json` 无 `eval` 键时，`phase_next` / `change_list` 行为与合并前一致 |
| AC-4 | 写后删除遗留文件 | 对同时存在两文件的 change 执行 `phase_log` 或 `backtrack` 后，`eval.json` 不存在，条目在 `workflow.json.eval` |
| AC-5 | `eval` 优先 | `workflow.json.eval` 为 `[]` 且磁盘上仍有旧 `eval.json` 时，读取结果为空数组（不合并两源） |
| AC-6 | `change_create` | 新建 change 只有 `workflow.json`（`workflow_type`+`created`），无 `eval` 键，无 `eval.json`；该工具是 `workflow.json` 的唯一创建者 |
| AC-7 | `change_list` artifacts | 即使已有评估历史，`artifacts` 也不包含 `eval.json`；`latest_phase`/`workflow_done` 仍正确 |
| AC-8 | 内置写保护 | Write/Edit/Bash/PowerShell/Shell/StrReplace 写 `openspec/changes/<name>/workflow.json` 被 deny；MCP 进程内 `fs` 写入仍成功 |
| AC-9 | `eval.json` 不再受内置写保护 | 内置 glob 仅为 `**/openspec/changes/**/workflow.json` 与 `**/openspec/config.json`；未配置 `write_protection` 时对 `openspec/changes/<name>/eval.json` 的 agent 写入**不再**产生内置 deny |
| AC-10 | MCP 描述 | `phase_log` / `backtrack` 的 description 提及 `workflow.json`，不再把 `eval.json` 当作写入目标 |
| AC-11 | Skill/agent | `phase-proposal` 无 Write `workflow.json` 步骤，缺文件时停止并指引（不手写）；evaluator/skill 文案指向 `phase_log` → `workflow.json` |
| AC-12 | 非法形状 | `workflow.json.eval` 为对象（非数组）时读取抛错；`change_list.workflow_done` 为 false 且不崩溃 |
| AC-13 | `workflow.json` 缺失严格报错 | `getWorkflowType()` 在文件缺失时抛错而**不再**返回 `"requirement"`；`phase_next` / `backtrack` 以可读错误终止（文案含绝对路径与 `change_create` 指引）；`phase_log` 同样失败且**不创建**任何文件 |
| AC-14 | `workflow.json` 格式非法严格报错 | JSON 非法、根非对象、`workflow_type` 缺失或非枚举、`eval` 非数组、`created` 非 `YYYY-MM-DD` 时 `getWorkflowType()` / `writeEvalJson` 抛错且不写入；`change_list` 对同一文件返回 `workflow_done: false`、`latest_phase: null` 且不崩溃 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 未走 `change_create` 的 change 目录缺 `workflow.json`（历史目录、手工建目录、脚本建目录） | `getWorkflowType()` 抛错 → `phase_next` / `backtrack` / `phase_log` 全部终止，工作流无法推进 | 中 | 报错文案含绝对路径与 `change_create` 指引；`phase-proposal` 缺文件时停止并给出二选一处置（用户手写，或改走 `change_create` / `workflow-*` 新建）；`workflow-*` skill 一律经 `change_create` 建目录 |
| 移除 `eval.json` 内置保护后，agent 可手写遗留文件 | 在 `workflow.json` 尚无 `eval` 键（创建后、首次评估前）时 `readEvalJson` 会回退读取伪造条目，可能伪造 pass | 低 | 本变更为用户明确要求的收敛方向；`change_create` 是唯一创建者、首次 `phase_log`/`backtrack` 即产生 `eval` 键并删除遗留文件，暴露窗口仅限「创建后、首次评估前」；agent/skill 文案仍禁止直接写 `eval.json`（见待决问题） |
| 双源短暂并存被误合并 | 评估历史错乱 | 低 | 明确「`eval` 数组存在则忽略 `eval.json`」，禁止数组合并 |
| 进行中 change 同时有 `eval.json` 与无 `eval` 键的 `workflow.json` | 首次 `phase_log`/`backtrack` 前读旧文件，写入后旧文件被删 | 中 | 读回退 + 写时迁移删除；`change_create` 不写空 `eval` |
| Agent 仍 Write `workflow.json`（如旧 `phase-proposal` 习惯） | 被 hook deny，独立 phase-proposal 无法把类型写成 test-only | 中 | skill 改为走 `change_create` 或由用户手写；文档说明非 `requirement` 类型须经对应 `workflow-*` skill 创建 |
| `workflow_type` 收为 4 值枚举后，磁盘上的历史/自定义类型被判非法 | 该 change 的 `phase_next` / `backtrack` 终止 | 低 | 值域与现有 `PHASE_TABLES` 的 4 张表一一对应，未收窄既有能力；错误文案含绝对路径与出错的字段路径，便于人工修正 |
| 保护 `workflow.json` 后，用户无法手改 `workflow_type` | 改类型须自行改文件或绕过 hook | 低 | 元数据由 MCP 维护；reason 提示用 `change_create` / `phase_log`；`eval.json` 不再受内置保护，手工干预遗留文件不必绕过 hook |
| 已归档 change 仍只有 `eval.json` | 只读文物，本变更不迁移 | 低 | 范围排除 `openspec/changes/archive/**` |
| fs mock 测试大量按 `endsWith('eval.json')` 分支 | 漏改导致单测假绿或假红 | 中 | 以 `eval-json.ts` 为唯一 IO，命令测尽量继续 mock `readEvalJson`；补文件系统契约测 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 字段名用 `eval` 还是 `entries`？ | `eval` | 与被合并产物 `eval.json` 同名，语义直接 | `entries`（贴近内存变量，但丢失产物名） |
| 是否保留函数名 `readEvalJson`？ | 保留 | 调用面（phase-log/next/backtrack/change-list）与大量 mock 不变 | 重命名为 `readWorkflowEval`（ churn 大） |
| 遗留 `eval.json`？ | 只读回退；权威写入后删除 | 进行中 change 不丢历史；避免长期双源 | 立即拒绝旧文件 / 启动时批量迁移 |
| `change_create` 是否写 `"eval": []`？ | 不写 | 显式空数组会按优先级挡住遗留 `eval.json` | 创建时写空数组并同时迁移 |
| `workflow.json` 缺失时是否缺省 `requirement`？ | 否，抛错并指引 `change_create` | 缺省会把 `test-only` / `bug-fix` 的 change 按 `requirement` 阶段表推进（门禁形同失效），且与「`change_create` 是唯一创建者」自相矛盾 | 保留缺省 + 仅告警（被否决：症状延迟到门禁失效才暴露） |
| `workflow.json` 形状校验放在哪里？ | 新增 `workflowFileSchema`（Zod），读写共用 | 原先的手工 `isPlainObject` + `typeof` 判断分散在 `change-config.ts` / `change-list.ts`，易漂移；项目已依赖 `zod/v4`；`workflowTypeSchema` 可与 `change-create.schema.ts` 同源 | 继续逐处手工判断（值域会各写一份） |
| 是否保护整个 `workflow.json`？ | 是；内置保护集合为 `workflow.json` + `config.json`，**不再保护 `eval.json`** | 评估历史已收敛到 `workflow.json`；遗留 `eval.json` 仅作只读回退，用户明确要求移除该 glob | 过渡期继续保护 `eval.json`（被用户否决）/ 只保护字段（hook 做不到） |
| `phase-proposal` 手写 `workflow.json`？ | 禁止 Write；文件缺失时**停止并指引** | 与 hook 保护、严格读取一致；缺省移除后不存在可继续的路径 | 给 hook 开「文件不存在则允许」特例；保留缺省 `requirement` |

### 待决问题

- **遗留 `eval.json` 的伪造窗口**：其内置写保护已按要求移除，而 `readEvalJson` 仍在 `workflow.json` 无 `eval` 键时回退读取该文件。若后续认为该窗口不可接受，可选：(a) 把回退收紧为「仅当 `workflow.json` 不存在」；(b) 由 `change_create` 写入 `"eval": []` 并同时完成迁移；(c) 过渡期结束后删除回退分支。本变更不做，仅记录。
- **`change_create` 是否支持「已存在目录 + 缺 `workflow.json` → 补写元数据」**：本变更不改（保持「已存在即拒绝」，`phase-proposal` 缺文件时指引用户手写或重建）。
- **非 `requirement` 类型的独立 `phase-proposal` 调用**：目录已存在且用户想要 `test-only` / `bug-fix` 时，按本变更 MUST 停止并指引改走对应 `workflow-*` skill，不再接受缺省 `requirement`。
