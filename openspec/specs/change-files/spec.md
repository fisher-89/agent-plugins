# change-files Specification

## Purpose

提供 `change_files` MCP 工具，作为 change 文件清单（`workflow.json` 的 `files` 字段）的手动补录与显式修正通道：`append` 按净状态折叠去重，`set` 整体覆写。

## Requirements

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

`op: "append"` SHALL 将传入路径合并入对应桶（`written` / `deleted`），按路径前缀与精确路径去重；append `written` 路径时 SHALL 按 `written += P ; deleted -= P` 折叠，append `deleted` 路径时按 `deleted += P ; written -= P` 折叠——即 append 单条路径等价于 hook 折叠规则的单次 write/delete 事件。append SHALL NOT 携带或改写条目来源审计字段：已存在路径去重时保留既有来源，新并入路径无 subagent 来源（见 workflow-file-inventory 规格的「清单条目来源审计」要求）。

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

Cursor 产物同样注册 PostToolUse 记录器（见 workflow-file-inventory 规格）后，本工具定位为漏记补录与净状态显式修正的兜底通道：hook 认不出的文件操作、hooks 未覆盖平台的事件、批量还原后的状态修正均经本工具进入清单。工具 description SHALL 说明两种用途：补录 hook 漏记的文件操作、显式修正净状态（还原）。文案 MUST NOT 引导用户绕过 PreToolUse 写保护直接改 `workflow.json`。

#### Scenario: description 说明双用途

- **WHEN** 读取 `change_files` 工具定义
- **THEN** description 含补录与修正净状态两类用途说明
- **AND** 不含绕过写保护的指引

### Requirement: workflow_files MCP 只读查询工具

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `workflow_files` 的 MCP 工具，作为 change 文件清单（`workflow.json` 的 `files` 字段）的只读查询通道，供 agent 代替直接 Read `workflow.json`。工具 SHALL 接收以下输入：

- `change`：`string`，必填，目标 change 名
- `project_root`：`string`，可选（MCP handler 经既有 `withResolvedProjectRoot` 解析注入）

工具 SHALL 严格只读：MUST NOT 提供任何写路径，MUST NOT 修改 `workflow.json` 的任何字段。工具命名遵循 `mcp-tool-namespace` 的 `xx_yy` 规则。description SHALL 表述为只读查询并写明与写通道（`change_files` 补录/修正）的分工，MUST NOT 引导绕过写保护。

#### Scenario: 工具出现在 MCP 列表

- **WHEN** MCP server 启动
- **THEN** `workflow_files` 出现在可用工具列表，inputSchema 含必填 `change`
- **AND** name 为下划线格式 `workflow_files`

#### Scenario: 查询只读不落盘

- **WHEN** 对任一 change 调用 `workflow_files`
- **THEN** 调用前后该 change 的 `workflow.json` 逐字节一致

### Requirement: 查询输出聚合净状态且不含来源审计

`workflow_files` SHALL 读取 `<projectRoot>/openspec/changes/<change>/workflow.json`，聚合 `files` 净状态并输出 `{ written: string[], deleted: string[] }`（相对项目根 POSIX 风格路径，目录删除以目录路径本身返回）。输出 MUST NOT 包含 `source` 来源审计映射——该字段仅审计用途、消费方 MUST NOT 读取（见 `workflow-file-inventory` 规格的「清单条目来源审计」要求），agent 正是该禁令的约束对象。

#### Scenario: 返回净状态

- **WHEN** 目标 change 的 `files.written` 为 `["src/a.ts"]`、`files.deleted` 为 `["src/old.ts"]`，调用 `workflow_files`
- **THEN** 输出 `written` 精确等于 `["src/a.ts"]`、`deleted` 精确等于 `["src/old.ts"]`

#### Scenario: 输出不含来源映射

- **WHEN** 目标 change 的 `files` 含非空 `source` 审计映射，调用 `workflow_files`
- **THEN** 输出仅含 `written` 与 `deleted` 两个字段，不含 `source`

### Requirement: 查询硬报错语义与既有消费方一致

`workflow_files` SHALL 复用与其它清单消费方一致的硬报错语义，MUST NOT 回退 git diff、MUST NOT 静默补建字段：目标 change 的 `workflow.json` 不存在、JSON 非法、未通过 `workflowFileSchema`、或缺失 `files` 字段（机制前旧 change）时，SHALL 返回错误且文案含重建指引。

#### Scenario: 旧 change 缺 files 字段硬报错

- **WHEN** 目标 change 的 `workflow.json` 无 `files` 字段，调用 `workflow_files`
- **THEN** 返回错误且文案含重建指引
- **AND** MUST NOT 回退为 git 工作区 diff

#### Scenario: 文件缺失硬报错

- **WHEN** 目标 change 目录不存在或其中无 `workflow.json`，调用 `workflow_files`
- **THEN** 返回错误并指引经 `change_create` 创建

### Requirement: workflow.json 文件清单逻辑统一由 modules/workflow 导出

系统 SHALL 以 `plugins/dev-team/bin/src/modules/workflow/` 目录（含其 `files/` 子目录）作为 `workflow.json` 操作逻辑的模块边界：文件清单的读取、折叠、写入与查询逻辑 SHALL 位于该目录并从 `workflow/index.ts` 统一导出（`readFileInventory`、`foldFileOps`、`writeFileInventory`、`getChangedFiles` 及 `FileInventory` / `FileOp` 类型）。**写入 `files` 字段的完整业务实现亦 SHALL 位于该目录**：PostToolUse 记录器的归账管线（路径规范化、自污染排除、gitignore 过滤、折叠、落盘 `recordFileOps`）与 `change_files` 的 append/set 净状态折叠语义（`appendFileOps` / `setFileBuckets`，确切签名以 Module Contract 为准）SHALL 迁入该目录并经 `workflow/index.ts` 导出。`commands/record-files.ts` SHALL 仅保留 hook 事件解析（stdin 读取、`tool_input` 提取、session 绑定与 `phase_next` 识别）与错误吞并策略；`commands/change-files.ts` SHALL 仅保留输入校验与委托——两者 MUST NOT 再内联清单折叠/落盘实现。`lib/file-inventory.ts` 保持删除状态，既有消费方（`change_files`、PostToolUse 记录器、`test-execution`、`test-resolve-paths`、`c4-cross-ref`、`shell-file-ops`）SHALL 从 `modules/workflow` 导入，MUST NOT 保留 re-export shim。后续新增的操作 `workflow.json` 的逻辑 SHALL 同样位于 `modules/workflow/` 并从 `workflow/index.ts` 导出（既有 eval 写入、创建、回溯逻辑的迁移由后续 change 增量处理，本变更 MUST NOT 一次性搬空）。

#### Scenario: 单一出口无旧路径残留

- **WHEN** 在仓库内 grep `lib/file-inventory` 导入，及 `commands/record-files.ts` / `commands/change-files.ts` 中的 `foldFileOps` / `writeFileInventory` / `applyAppend` / `applySet` 内联实现
- **THEN** 无任何源码或测试残留该导入路径，两命令文件内无折叠/落盘实现
- **AND** `plugins/dev-team/bin/src/lib/file-inventory.ts` 不存在，迁移不留 shim

#### Scenario: 记录器委托模块行为不变

- **WHEN** session 已绑定 change，记录器收到 `tool_name: "Write"`、`tool_input.file_path: "<root>/src/foo.ts"` 的 PostToolUse 事件
- **THEN** 归账经模块归账管线完成，`files.written` 含 `src/foo.ts`、`files.deleted` 不含
- **AND** 既有折叠规则、自污染排除与 source 审计行为与迁移前一致

#### Scenario: change_files 委托模块语义不变

- **WHEN** 调用 `change_files`（append 或 set）
- **THEN** 净状态折叠语义、来源审计保留策略与 `workflow_type` / `created` / `eval` 保留行为与迁移前一致
- **AND** 实现位于 `modules/workflow/`，命令层仅做输入校验与委托

#### Scenario: 后续逻辑入口约定

- **WHEN** 后续 change 为 `workflow.json` 新增操作逻辑
- **THEN** 该逻辑位于 `plugins/dev-team/bin/src/modules/workflow/` 并经 `workflow/index.ts` 导出

## Module Contract

### MCP 工具注册: `plugins/dev-team/bin/src/mcp.ts`

| 字段 | 值 |
|------|-----|
| 工具名 | `workflow_files` |
| 输入 | `{ change: string, project_root?: string }` |
| 输出 | `{ written: string[], deleted: string[] }`（聚合净状态，无 `source`） |
| handler | 解析 project_root → `getChangedFiles` → 只读返回；无任何写路径 |

### Module: `plugins/dev-team/bin/src/modules/workflow/`

| 函数 | 参数 | 返回 | 描述 |
|------|------|------|------|
| `getChangedFiles` | `options: { change: string; project_root?: string }` | `{ written: string[]; deleted: string[] }` | 只读查询指定 change 的文件清单净状态 |
| `readFileInventory` | `changeDir: string` | `FileInventory` | 读取并校验 `workflow.json` 的 `files` |
| `foldFileOps` | `inventory: FileInventory, ops: FileOp[]` | `FileInventory` | 净状态折叠（纯函数） |
| `writeFileInventory` | `changeDir: string, files: FileInventory` | `void` | 写回 `files` 净状态 |
| `recordFileOps` | `changeDir: string, ops: FileOp[], context: { projectRoot: string; agentType?: string }` | `void` | 归账管线：规范化 → 自污染排除 → gitignore 过滤 → 读 → 折叠 → 落盘 |
| `appendFileOps` | `changeDir: string, paths: { written?: string[]; deleted?: string[] }` | `FileInventory` | append 语义（读改写）：折叠合并去重，保留既有 source |
| `setFileBuckets` | `changeDir: string, paths: { written?: string[]; deleted?: string[] }` | `FileInventory` | set 语义（读改写）：整桶覆写，覆写条目清除 source |
| `loadGitignoreFilter` / `isGitIgnored` | 见 workflow-file-inventory 规格 | — | gitignore 过滤器（归账管线内部消费） |
| 类型 | `FileInventory` / `FileOp` | — | 清单净状态与文件操作类型 |

### 消费方（迁移后形态）

| 文件 | 保留职责 | 移除内容 |
|------|---------|---------|
| `commands/record-files.ts` | stdin 解析、`phase_next` 绑定识别、`tool_input` 提取、错误吞并 exit-0 | `normalizeRecordedPath` / `isExcludedFromInventory` / `collectRecordedOps` / 读折叠写步骤（迁入模块） |
| `commands/change-files.ts` | options 校验、`resolveChangeDir`、委托 | `applyAppend` / `applySet` / `dedupe`（迁入模块） |

### API 接口

| method | path | request | response |
|--------|------|---------|----------|
| MCP tool | `change_files` | `{ change: string, op: "append"\|"set", written?: string[], deleted?: string[] }` | `{ written: string[], deleted: string[] }`（实现委托模块） |
| MCP tool | `workflow_files` | `{ change: string, project_root?: string }` | `{ written: string[], deleted: string[] }` |
