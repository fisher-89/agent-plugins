## 权威边界

Planner 模板与共享 eval/checklist schema 形状。Agent 命名以 `phase-skills` 为准；回溯写入以 `pipeline-backtrack` 为准（决议 C1 命名对齐、C3=A）。本变更改评估历史的**磁盘落点**（`eval.json` 数组文件 → `workflow.json` 对象的 `eval` 字段），并新增 `workflow.json` 自身的形状 schema 与严格读取（文件缺失 / 格式非法即报错，不再缺省）。条目字段以 `phaseLogSchema` 为准。

## ADDED Requirements

### Requirement: Markdown templates for Planner artifacts

系统 SHALL 在 `templates/artifacts/` 提供建议模板（非强制结构）：

- `proposal.md.template` — Problem, Scope, Risks, Acceptance Criteria；「变更范围」SHALL 支持删除声明（实现文件 / 测试文件 / 删除文件 / 不要修改）
- `test-design.md.template` — Test Levels, Coverage Map, Test Strategy, Boundary Cases
- `design.md.template` — Architecture Components, Change Inventory, Data Model, Route/API, Dependencies, Open Questions；变更清单 SHALL 含「删除文件」子节（与「新增文件」「修改文件」并列，可省略注记保留），允许声明目录级删除

Evaluators 检查内容质量，不强制章节顺序。

计划侧删除声明与 actual 侧 `files.deleted` 同构：均以路径（含目录前缀）表达，消费方按前缀匹配对账（见 `workflow-file-inventory` 规格的对账三态）。

#### Scenario: proposal-planner 覆盖建议章节

- **WHEN** `proposal-planner` 写 proposal.md
- **THEN** 输出覆盖 Problem、Scope、Risks、Acceptance Criteria（可增删章节）

#### Scenario: design 变更清单可声明删除

- **WHEN** `dev-design-planner` 按 `design.md.template` 写变更清单且本 change 需删除 `src/old.ts`
- **THEN** 「删除文件」子节含 `src/old.ts`
- **AND** 无删除时该子节以省略注记表达，MUST NOT 强制为空表

#### Scenario: 目录级删除声明

- **WHEN** design 声明删除目录 `src/old-module/`
- **THEN** 声明以目录路径表达
- **AND** 与 actual 侧 `rm -rf` 记录的目录前缀同构可对账

### Requirement: Shared JSON schemas for evaluation

系统 SHALL 定义共享 schema：

- 评估条目：`phase`（无前缀 ID，如 `"proposal"`）、`timestamp`、`attempt`、`verdict`、`report`、`checklist`、可选 `backtrack_to` / `backtrack_reason` / `stale` 等（实现为 `phaseLogSchema`）。权威落点为 change 目录下 `workflow.json` 对象的 `eval` 数组字段，无独立 `eval.json`，无 per-phase 文件。
- `checklist.schema.json`：`{id, criterion, required, evidence_hint}` 参考格式（本变更不改）

**写入方**：新条目由 `phase_log` 追加时 MUST NOT 带 `backtrack_to`/`backtrack_reason`；这两字段仅由 `backtrack` 工具写入（schema 保留 optional 以解析旧条目与回溯后状态）。

#### Scenario: 条目符合 schema

- **WHEN** 任意 evaluator 经 `phase_log` 追加条目
- **THEN** 条目符合 `phaseLogSchema`，且创建时无回溯字段
- **AND** 条目出现在 `openspec/changes/<name>/workflow.json` 的 `eval` 数组中

#### Scenario: eval 只追加

- **WHEN** 同 phase 在 fail 后再次评估
- **THEN** 新结果追加到 `workflow.json.eval`；旧条目保留

#### Scenario: 按 timestamp 取最新

- **WHEN** 校验或 `phase_next` 需要某 phase 最新结果
- **THEN** 按 `phase` 分组取最大 `timestamp`（并尊重 stale 规则，见引擎/回溯规格）

### Requirement: Canonical eval store is workflow.json.eval

`readEvalJson(changeDir)` / `writeEvalJson(changeDir, entries)` SHALL 以 `workflow.json` 为权威文件：

- 根元素 MUST 为对象
- 评估历史 MUST 存在于可选字段 `eval`：值为 `phaseLogSchema` 条目的数组
- `change_create` 可不写 `eval` 键；缺失的 `eval` 在读取时视为「尚未迁入权威存储」，再走遗留回退

`writeEvalJson` SHALL：

1. 要求 `workflow.json` **已存在**；文件缺失 SHALL 抛错（文案含绝对路径与 `change_create` 指引），MUST NOT `mkdirSync`、MUST NOT 以缺省元数据创建该文件
2. 读取已有文件并经 `workflowFileSchema` 校验（根 MUST 为对象）；JSON 非法、根非对象或校验失败 SHALL 抛错，且 **MUST NOT 写入**
3. 设置 `eval` 为完整条目数组（覆盖该字段，不与磁盘上的 `eval.json` 做数组合并）
4. 保留 `workflow_type`、`created` 及未知键；MUST NOT 补写任何缺省元数据
5. 使用 2 空格缩进与末尾换行（与现行 `writeEvalJson` 格式一致）
6. 成功后若 `eval.json` 仍存在，SHALL 删除该文件
7. MUST NOT 再写入 `eval.json`

导出函数名 `readEvalJson` / `writeEvalJson` / `appendEntry` SHALL 保持不变。

#### Scenario: phase_log 写入 workflow.json 且不产生 eval.json

- **WHEN** `phase_log` 对仅有 `{ "workflow_type": "requirement", "created": "2026-09-11" }` 的 change 追加一条 pass
- **THEN** `workflow.json.eval` 长度为 1
- **AND** `workflow_type` 与 `created` 不变
- **AND** change 目录下不存在 `eval.json`

#### Scenario: 写入保留未知键

- **WHEN** `workflow.json` 含额外键（如 `"note": "x"`）且 `writeEvalJson` 写入两条 eval
- **THEN** 写回后 `"note"` 仍为 `"x"`

#### Scenario: 写入时文件缺失即失败

- **WHEN** change 目录内没有 `workflow.json`，`phase_log` 尝试写入评估
- **THEN** 抛错，文案含绝对路径与 `change_create` 指引
- **AND** MUST NOT 创建 `workflow.json`（或目录）
- **AND** MUST NOT 回落为「以缺省 `requirement` + 当日 `created` 补建」

#### Scenario: 写入时格式非法即失败

- **WHEN** `workflow.json` 存在但根为数组（或 `workflow_type` 缺失 / 非枚举 / JSON 语法错误）
- **THEN** `writeEvalJson` 抛错
- **AND** 磁盘上的文件内容不被改写

### Requirement: Legacy eval.json read fallback

`readEvalJson` SHALL 按以下优先级读取，MUST NOT 合并两个来源的数组：

1. `workflow.json` 存在且 `eval` 的值是数组（含空数组）→ 解析该数组并返回
2. 否则若 `eval.json` 存在 → 按现行规则解析（根必须为数组，元素走 `phaseLogSchema`）
3. 否则返回 `[]`

当 `workflow.json` 存在且 `eval` 键存在但值不是数组时，SHALL 抛错。当走步骤 2 且 `eval.json` 根不是数组时，SHALL 抛错（与现行「根元素必须是数组」一致）。

#### Scenario: 仅有遗留 eval.json

- **WHEN** change 目录有合法 `eval.json` 数组、`workflow.json` 无 `eval` 键（或文件不存在）
- **THEN** `readEvalJson` 返回该数组内容

#### Scenario: 权威空数组忽略遗留文件

- **WHEN** `workflow.json.eval` 为 `[]` 且磁盘上仍有非空 `eval.json`
- **THEN** `readEvalJson` 返回 `[]`
- **AND** MUST NOT 把 `eval.json` 中的条目并入结果

#### Scenario: eval 字段类型非法

- **WHEN** `workflow.json` 为 `{ "workflow_type": "requirement", "eval": {} }`
- **THEN** `readEvalJson` 抛错

### Requirement: workflow.json 形状与严格类型读取

系统 SHALL 在 `plugins/dev-team/bin/src/schemas/workflow.schema.ts` 定义 `workflow.json` 形状的唯一来源：

- `workflowTypeSchema = z.enum(['requirement', 'bug-fix', 'refactor', 'test-only'])`；`change-create.schema.ts` 的 `workflow_type` SHALL 引用同一 schema（值域与 describe 不变）
- `workflowEvalSchema = z.array(phaseLogSchema)`
- `workflowFileSchema = z.looseObject({ workflow_type: workflowTypeSchema, created?: string(/^\d{4}-\d{2}-\d{2}$/), eval?: workflowEvalSchema })`；未知键 SHALL 被允许（写入时原样保留）

`getWorkflowType(change)` SHALL 按「文件必须存在 → `JSON.parse` → 根 MUST 为对象 → `workflowFileSchema.parse` → 返回 `workflow_type`」读取；任一环节失败 SHALL 抛错（文案含绝对路径；文件缺失时另含 `change_create` 指引）。MUST NOT 在文件缺失或 `workflow_type` 缺失 / 非枚举 / 非字符串时返回缺省值（删除 `DEFAULT_WORKFLOW_TYPE` 兜底）。

`readEvalJson` 的宽严度 SHALL 与 `getWorkflowType` / `writeEvalJson` 不同：文件缺失时 MUST NOT 抛错，仍走遗留 `eval.json` 回退、否则返回 `[]`。

`change_create` SHALL 是该文件的唯一创建者。

#### Scenario: 文件缺失时严格报错

- **WHEN** change 目录存在但没有 `workflow.json`，调用 `getWorkflowType`（`phase_next` / `backtrack` 的入口）
- **THEN** 抛错，文案含该文件的绝对路径与 `change_create` 指引
- **AND** MUST NOT 返回 `"requirement"`

#### Scenario: 格式非法时严格报错

- **WHEN** `workflow.json` 分别为 `{}`（缺 `workflow_type`）、`{"workflow_type":"unknown"}`、根为数组、JSON 语法错误、`created` 为 `2026/09/11`
- **THEN** `getWorkflowType` 抛错，文案指出出错的字段路径
- **AND** `writeEvalJson` 对同一文件同样拒绝写入

#### Scenario: 合法文件读取且未知键不影响

- **WHEN** `workflow.json` 为 `{ "workflow_type": "test-only", "created": "2026-09-11", "note": "x" }`
- **THEN** `getWorkflowType` 返回 `"test-only"`

#### Scenario: 条目读取对缺文件保持宽容

- **WHEN** change 目录内既无 `workflow.json` 也无遗留 `eval.json`
- **THEN** `readEvalJson` 返回 `[]`，MUST NOT 抛错

## Module Contract

### `plugins/dev-team/bin/src/lib/eval-json.ts`

| Export | Change | Description |
|--------|--------|-------------|
| `readEvalJson` | MODIFIED | 读 `workflow.json.eval`；无该数组时回退 `eval.json`；两者皆无返回 `[]`；文件缺失不抛错 |
| `writeEvalJson` | MODIFIED | 要求 `workflow.json` 既存且通过 `workflowFileSchema`，否则抛错；把 `entries` 写入 `eval` 并保留其它键；成功后删除 `eval.json` |
| `appendEntry` | MODIFIED | 内部改走新的 read/write；行为仍为追加一条；不再创建目录 / 文件 |
| `buildEntry` / `computeAttempt` / `markPhaseStale` / `validateVerdict` / `validateReportLength` | UNCHANGED | 纯内存，与磁盘文件名无关 |

### `plugins/dev-team/bin/src/lib/change-config.ts`

| Export | Change | Description |
|--------|--------|-------------|
| `getWorkflowType` | MODIFIED | 文件必须存在且通过 `workflowFileSchema`；否则抛错。删除 `DEFAULT_WORKFLOW_TYPE`，**不再**缺省 `"requirement"` |

### `plugins/dev-team/bin/src/schemas/workflow.schema.ts`（新增文件）

| Export | Change | Description |
|--------|--------|-------------|
| `workflowTypeSchema` | ADDED | `z.enum(['requirement', 'bug-fix', 'refactor', 'test-only'])`；值域唯一来源 |
| `workflowEvalSchema` | ADDED | `z.array(phaseLogSchema)`；`workflow.json.eval` 形状唯一来源 |
| `workflowFileSchema` | ADDED | `z.looseObject({ workflow_type, created?, eval? })`；未知键允许 |
| `WorkflowFile` | ADDED | `z.infer<typeof workflowFileSchema>` |

### `openspec/changes/<name>/workflow.json`

| 字段 | 类型 | 变更 |
|------|------|------|
| `workflow_type` | `string`（4 值枚举，必填） | **MODIFIED** — 由 `workflowFileSchema` 校验；缺失 / 非枚举即报错 |
| `created` | `string`（`YYYY-MM-DD`，可选） | **MODIFIED** — 存在时必须匹配 `^\d{4}-\d{2}-\d{2}$` |
| `eval` | `EvalEntry[]`（可选） | **ADDED** — 原 `eval.json` 根数组 |
| 未知键 | 任意 | 允许；写入时原样保留 |
