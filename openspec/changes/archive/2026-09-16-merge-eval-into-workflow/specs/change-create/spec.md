## Purpose

MCP tool that creates a new change directory with `workflow.json` metadata. 本变更禁止再创建并行的 `eval.json`，新建文件 MUST NOT 带空 `eval` 数组（以免挡住遗留回退），并把该工具确立为 `workflow.json` 的**唯一创建者**——读取方不再以缺省类型兜底。

## MODIFIED Requirements

### Requirement: change_create creates directory and workflow.json

The tool SHALL create `openspec/changes/<name>/` directory and write `workflow.json` with `workflow_type`（调用方传入，**必填**，取值来自 `workflowTypeSchema` 的 4 值枚举）and `created` set to the current date in `YYYY-MM-DD` format. The tool SHALL NOT write `.openspec.yaml`.

写入的对象 SHALL 满足 `workflowFileSchema`。该工具 SHALL 是 `workflow.json` 的唯一创建者：读取方（`getWorkflowType`）在文件缺失或格式非法时 SHALL 报错并指引本工具，MUST NOT 以缺省 `"requirement"` 代替。

The tool SHALL NOT create `eval.json`. The tool SHALL NOT write an `eval` key (including `"eval": []`) on the new `workflow.json`.

#### Scenario: Default workflow.json is created

- **WHEN** `change_create` is called with `name: "my-change"`
- **THEN** a directory `openspec/changes/my-change/` SHALL exist
- **AND** `openspec/changes/my-change/workflow.json` SHALL contain `workflow_type` 与 `created`
- **AND** `workflow.json` SHALL NOT contain key `eval`
- **AND** `openspec/changes/my-change/eval.json` SHALL NOT exist
- **AND** `openspec/changes/my-change/.openspec.yaml` SHALL NOT exist

#### Scenario: Existing directory is still rejected without writing eval.json

- **WHEN** `change_create` is called with `name: "my-change"`
- **AND** `openspec/changes/my-change/` already exists
- **THEN** the tool SHALL return an error
- **AND** SHALL NOT create `eval.json` or add `eval` to an existing `workflow.json`

## ADDED Requirements

### Requirement: change_create does not initialize eval history

评估历史仅由后续 `phase_log` / `backtrack` 经 `writeEvalJson` 写入 `workflow.json.eval`。`change_create` MUST 把评估字段留给首次评估写入，以便 `readEvalJson` 在首次写入前仍能回退读取遗留 `eval.json`（若存在）。

#### Scenario: 新建 change 读取评估为空

- **WHEN** `change_create` 刚成功
- **AND** 调用 `readEvalJson`（无遗留 `eval.json`）
- **THEN** 返回 `[]`

#### Scenario: 缺 workflow.json 时下游报错并指引本工具

- **WHEN** 某 change 目录没有 `workflow.json`（未经 `change_create` 建立）
- **THEN** `phase_next` / `backtrack` / `phase_log` SHALL 报错，文案含该文件的绝对路径与 `change_create` 指引
- **AND** 该错误 MUST NOT 被缺省 `"requirement"` 掩盖

## Module Contract

### 元数据文件: `openspec/changes/<name>/workflow.json`

| 字段 | 类型 | 值 |
|------|------|-----|
| `workflow_type` | `string`（4 值枚举，必填） | 调用方传入（`requirement` / `bug-fix` / `refactor` / `test-only`） |
| `created` | `string` | `YYYY-MM-DD` |
| `eval` | — | **创建时不写此键** |
| 未知键 | — | 允许（`workflowFileSchema` 为 loose object） |

### 命令实现: `plugins/dev-team/bin/src/commands/change-create.ts`

| 函数 | 变更 |
|------|------|
| `runChangeCreate` | 仍 `writeFileSync` `workflow.json`；唯一创建者；MUST NOT `eval.json`；MUST NOT `eval` 键 |

### Schema: `plugins/dev-team/bin/src/schemas/change-create.schema.ts`

| 字段 | 变更 |
|------|------|
| `workflow_type` | 枚举改为引用 `workflowTypeSchema`（值域与 describe 不变） |
