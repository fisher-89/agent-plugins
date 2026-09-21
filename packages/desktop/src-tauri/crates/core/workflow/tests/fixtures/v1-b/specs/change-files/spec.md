# change-files Specification

## ADDED Requirements

### Requirement: change_files MCP 工具注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `change_files` 的 MCP 工具，作为 change 文件清单（`workflow.json` 的 `files` 字段）的手动补录与显式修正通道。工具 SHALL 接收至少以下输入：

- `change`：`string`，必填，目标 change 名
- `op`：`"append" | "set"`，必填
- `written` / `deleted`：`string[]`，可选路径列表（至少提供其一）

工具 SHALL 校验 change 存在且 `workflow.json` 通过 `workflowFileSchema`；对缺失 `files` 字段的旧 change SHALL 硬报错并指引重建，MUST NOT 静默补建字段以外的元数据。写保护对 `workflow.json` 的既有规则不受影响——本工具经 MCP `fs` 写入，与 `phase_log` / `backtrack` 同一通道。

#### Scenario: 工具出现在 MCP 列表

- **WHEN** MCP server 启动
- **THEN** `change_files` 出现在可用工具列表，inputSchema 含必填 `change` 与 `op`

#### Scenario: 旧 change 硬报错

- **WHEN** `change_files` 目标 change 的 `workflow.json` 无 `files` 字段
- **THEN** 返回错误并指引重建
- **AND** MUST NOT 修改该文件

### Requirement: append 语义为净状态内追加去重

`op: "append"` SHALL 将传入路径合并入对应桶（`written` / `deleted`），按路径前缀与精确路径去重；append `written` 路径时 SHALL 按 `written += P ; deleted -= P` 折叠，append `deleted` 路径时按 `deleted += P ; written -= P` 折叠——即 append 单条路径等价于 hook 折叠规则的单次 write/delete 事件。append SHALL NOT 携带或改写条目来源审计字段：已存在路径去重时保留既有来源，新并入路径无 subagent 来源（见 workflow-file-inventory 变更的「清单条目来源审计」要求）。

#### Scenario: append write 路径折叠

- **WHEN** `files.written` 为 `[]`、`files.deleted` 为 `["src/a.ts"]`，调用 `append` 且 `written: ["src/a.ts"]`
- **THEN** `files.written` 含 `src/a.ts` 且 `files.deleted` 不含 `src/a.ts`

#### Scenario: append 去重

- **WHEN** `files.written` 已含 `src/b.ts`，再次 `append` 且 `written: ["src/b.ts"]`
- **THEN** `files.written` 中 `src/b.ts` 仅出现一次

### Requirement: set 语义为净状态覆写

`op: "set"` SHALL 以传入的 `written` / `deleted` 数组**整体覆写**对应桶（未提供的桶保持不变），用于 hook 认不出的批量还原、`git stash`/`git clean` 后的状态修正，以及非 Claude 环境的手动灌入。`set` MUST NOT 触发追加折叠，MUST NOT 修改 `workflow.json` 的其他字段（`workflow_type`、`created`、`eval`、未知键保留）。覆写后的条目无 subagent 来源（人工净状态）。

#### Scenario: set 覆写 written

- **WHEN** `files.written` 为 `["src/a.ts", "src/b.ts"]`，调用 `set` 且 `written: ["src/a.ts"]`
- **THEN** `files.written` 精确等于 `["src/a.ts"]`

#### Scenario: set 保留其他字段

- **WHEN** 对含 `workflow_type` / `created` / `eval` 的 `workflow.json` 调用 `set`
- **THEN** 这些字段逐字节保留（含格式：2 空格缩进 + 末尾换行）

### Requirement: 兜底定位与文案

工具 SHALL 在非 Claude 环境（Cursor 等 hooks 支持不全的平台）作为文件清单的补录入口；工具 description SHALL 说明两种用途：补录 hook 漏记的文件操作、显式修正净状态（还原）。文案 MUST NOT 引导用户绕过 PreToolUse 写保护直接改 `workflow.json`。

#### Scenario: description 说明双用途

- **WHEN** 读取 `change_files` 工具定义
- **THEN** description 含补录与修正净状态两类用途说明
- **AND** 不含绕过写保护的指引

## Module Contract

### MCP 工具注册: `plugins/dev-team/bin/src/mcp.ts`

| 字段    | 值                                                                                  |
| ------- | ----------------------------------------------------------------------------------- |
| 工具名  | `change_files`                                                                      |
| 输入    | `{ change: string, op: "append" \| "set", written?: string[], deleted?: string[] }` |
| 输出    | `{ written: string[], deleted: string[] }`（操作后的净状态）                        |
| handler | 校验 change 与 schema → 折叠/覆写 → 经既有 `workflow.json` 写通道持久化             |

### Schema: `plugins/dev-team/bin/src/schemas/change-files.schema.ts`（新增）

| Property    | Description                                                                                    |
| ----------- | ---------------------------------------------------------------------------------------------- |
| **Exports** | `changeFilesInputSchema` / `changeFilesOutputSchema`（Zod v4，与 `schemas/index.ts` 一致导出） |
| **校验**    | `written`/`deleted` 至少其一非可省；路径为相对项目根 POSIX 风格                                |
