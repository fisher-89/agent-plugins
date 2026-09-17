# change-create Specification（workflow-file-inventory 增量）

## MODIFIED Requirements

### Requirement: change_create creates directory and workflow.json

The tool SHALL create `openspec/changes/<name>/` directory and write `workflow.json` with `workflow_type`（调用方传入，**必填**，取值来自 `workflowTypeSchema` 的 4 值枚举）and `created` set to the current date in `YYYY-MM-DD` format. The tool SHALL NOT write `.openspec.yaml`.

写入的对象 SHALL 满足 `workflowFileSchema`。该工具 SHALL 是 `workflow.json` 的唯一创建者：读取方（`getWorkflowType`）在文件缺失或格式非法时 SHALL 报错并指引本工具，MUST NOT 以缺省 `"requirement"` 代替。

The tool SHALL NOT create `eval.json`. The tool SHALL NOT write an `eval` key (including `"eval": []`) on the new `workflow.json`.

本变更新增：`change_create` SHALL 在新建的 `workflow.json` 中写入初始文件清单 `files: { written: [], deleted: [] }`（两个空数组），使 `workflow-file-inventory` 的消费方可依赖该字段存在；缺失 `files` 的 change 一律视为机制前旧 change 并被消费方硬报错。初始 `files` MUST NOT 预填任何路径。

#### Scenario: Default workflow.json is created

- **WHEN** `change_create` is called with `name: "my-change"`
- **THEN** a directory `openspec/changes/my-change/` SHALL exist
- **AND** `openspec/changes/my-change/workflow.json` SHALL contain `workflow_type` 与 `created`
- **AND** `workflow.json` 的 `files` SHALL 等于 `{ "written": [], "deleted": [] }`
- **AND** `workflow.json` SHALL NOT contain key `eval`
- **AND** `openspec/changes/my-change/eval.json` SHALL NOT exist
- **AND** `openspec/changes/my-change/.openspec.yaml` SHALL NOT exist

#### Scenario: Existing directory is still rejected without writing eval.json

- **WHEN** `change_create` is called with `name: "my-change"`
- **AND** `openspec/changes/my-change/` already exists
- **THEN** the tool SHALL return an error
- **AND** SHALL NOT create `eval.json` or add `eval` to an existing `workflow.json`
- **AND** SHALL NOT 覆写既有 change 的 `files`

## Module Contract

### 命令实现: `plugins/dev-team/bin/src/commands/change-create.ts`（增量）

| 符号 | 变更 |
|------|------|
| `runChangeCreate` 写入对象 | **ADDED** `files: { written: [], deleted: [] }` 初始净状态；`workflow_type` / `created` / 不写 `eval` 等既有契约不变 |

### 元数据文件: `openspec/changes/<name>/workflow.json`（增量）

| 字段 | 类型 | 值 |
|------|------|-----|
| `files` | `{ written: string[], deleted: string[] }` | 创建时初始化为两个空数组 |
| `workflow_type` / `created` / `eval` | — | 不变 |
