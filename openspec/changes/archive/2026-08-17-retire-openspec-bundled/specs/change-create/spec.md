# change-create Specification

## Purpose
MCP tool that creates a new change directory with `workflow.json` metadata, replacing the `openspec new change` CLI command.

## ADDED Requirements

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

The tool SHALL create `openspec/changes/<name>/` directory and write `workflow.json` with `workflow_type: "requirement"` and `created` set to the current date in `YYYY-MM-DD` format. The tool SHALL NOT write `.openspec.yaml`.

#### Scenario: Default workflow.json is created

- **WHEN** `change_create` is called with `name: "my-change"`
- **THEN** a directory `openspec/changes/my-change/` SHALL exist
- **AND** `openspec/changes/my-change/workflow.json` SHALL contain `{"workflow_type": "requirement", "created": "<current-date>"}`
- **AND** `openspec/changes/my-change/.openspec.yaml` SHALL NOT exist

### Requirement: change_create rejects existing change

The tool SHALL check if the target directory `openspec/changes/<name>/` already exists. If it does, the tool SHALL return an error and SHALL NOT modify the existing directory.

#### Scenario: Existing directory is rejected

- **WHEN** `change_create` is called with `name: "my-change"`
- **AND** `openspec/changes/my-change/` already exists
- **THEN** the tool SHALL return an error indicating the change already exists
- **AND** SHALL NOT modify the existing directory or files

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

### 元数据文件: `openspec/changes/<name>/workflow.json`

| 字段 | 类型 | 值 |
|------|------|-----|
| `workflow_type` | `string` | `"requirement"`（默认，skill 可后续覆写） |
| `created` | `string` | `YYYY-MM-DD` 格式的创建日期 |