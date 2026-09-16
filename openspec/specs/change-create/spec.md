# change-create Specification

## Purpose
MCP tool that creates a new change directory with `workflow.json` metadata, replacing the `openspec new change` CLI command. 本变更禁止再创建并行的 `eval.json`，新建文件 MUST NOT 带空 `eval` 数组（以免挡住遗留回退），并把该工具确立为 `workflow.json` 的**唯一创建者**——读取方不再以缺省类型兜底。

## Requirements

### Requirement: change_create MCP tool is registered

The plugin SHALL register a `change_create` MCP tool in `mcp.ts` with input schema requiring a `name` field (string).

#### Scenario: Tool appears in MCP tool list

- **WHEN** the MCP server is started
- **THEN** `change_create` SHALL appear in the list of available MCP tools
- **AND** its `inputSchema` SHALL define `name` as a required string property

#### Scenario: Output schema is defined

- **WHEN** examining the `change_create` tool definition
- **THEN** the `outputSchema` SHALL define the return structure for the created change metadata

### Requirement: change_create validates kebab-case name

The tool SHALL validate the `name` parameter against the regex `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` (kebab-case). Invalid names SHALL be rejected with an error message.

#### Scenario: Valid kebab-case name is accepted

- **WHEN** `change_create` is called with `name: "my-new-feature"`
- **THEN** the tool SHALL proceed to create the change directory

#### Scenario: Invalid name is rejected

- **WHEN** `change_create` is called with `name: "My Feature"` (contains uppercase and space)
- **THEN** the tool SHALL return an error indicating the name is not valid kebab-case
- **AND** SHALL NOT create any directory or file

#### Scenario: Underscore name is rejected

- **WHEN** `change_create` is called with `name: "my_new_feature"` (contains underscores)
- **THEN** the tool SHALL return an error indicating the name is not valid kebab-case

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

### Requirement: change_create rejects existing change

The tool SHALL check if the target directory `openspec/changes/<name>/` already exists. If it does, the tool SHALL return an error and SHALL NOT modify the existing directory.

#### Scenario: Existing directory is rejected

- **WHEN** `change_create` is called with `name: "my-change"`
- **AND** `openspec/changes/my-change/` already exists
- **THEN** the tool SHALL return an error indicating the change already exists
- **AND** SHALL NOT modify the existing directory or files

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

### MCP 工具注册: `plugins/dev-team/bin/src/mcp.ts`

| 字段 | 值 |
|------|-----|
| 工具名 | `change_create` |
| 输入 | `{ name: string }` |
| 输出 | `{ name: string, path: string, workflow_type: string, created: string }` |
| handler | 调用 `createChange()` 函数 |

### 命令实现: `plugins/dev-team/bin/src/commands/change-create.ts`

| 函数 | 签名 | 描述 |
|------|------|------|
| `createChange` | `(name: string, changeDir: string) => { name, path, workflow_type, created }` | 校验名称、创建目录、写 `workflow.json`；已存在时 throw |
| `validateChangeName` | `(name: string) => boolean` | kebab-case 校验 |
| `runChangeCreate` | — | 仍 `writeFileSync` `workflow.json`；唯一创建者；MUST NOT `eval.json`；MUST NOT `eval` 键 |

### Schema: `plugins/dev-team/bin/src/schemas/change-create.schema.ts`

| 字段 | 变更 |
|------|------|
| `workflow_type` | 枚举改为引用 `workflowTypeSchema`（值域与 describe 不变） |

### 元数据文件: `openspec/changes/<name>/workflow.json`

| 字段 | 类型 | 值 |
|------|------|-----|
| `workflow_type` | `string`（4 值枚举，必填） | 调用方传入（`requirement` / `bug-fix` / `refactor` / `test-only`） |
| `created` | `string` | `YYYY-MM-DD` |
| `eval` | — | **创建时不写此键** |
| 未知键 | — | 允许（`workflowFileSchema` 为 loose object） |
