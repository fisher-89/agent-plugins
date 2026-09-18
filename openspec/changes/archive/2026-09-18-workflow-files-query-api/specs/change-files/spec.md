# change-files Specification

## ADDED Requirements

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

系统 SHALL 新建 `plugins/dev-team/bin/src/modules/workflow/` 目录作为 `workflow.json` 操作逻辑的模块边界：文件清单的读取、折叠、写入与查询逻辑 SHALL 位于该目录并从 `workflow/index.ts` 统一导出（`readFileInventory`、`foldFileOps`、`writeFileInventory`、`runWorkflowFiles` 及 `FileInventory` / `FileOp` 类型）。`lib/file-inventory.ts` SHALL 删除，既有消费方（`change_files`、PostToolUse 记录器、`test-execution`、`test-resolve-paths`、`c4-cross-ref`、`shell-file-ops`）SHALL 改从 `modules/workflow` 导入，MUST NOT 保留 re-export shim。后续新增的操作 `workflow.json` 的逻辑 SHALL 同样位于 `modules/workflow/` 并从 `workflow/index.ts` 导出（既有 eval 写入、创建、回溯逻辑的迁移由后续 change 增量处理，本变更 MUST NOT 一次性搬空）。

#### Scenario: 单一出口无旧路径残留

- **WHEN** 在仓库内 grep `lib/file-inventory` 导入
- **THEN** 无任何源码或测试残留该导入路径
- **AND** `plugins/dev-team/bin/src/lib/file-inventory.ts` 不存在

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
| handler | 解析 project_root → `runWorkflowFiles` → 只读返回；无任何写路径 |

### Module: `plugins/dev-team/bin/src/modules/workflow/`（新增）

| 函数 | 参数 | 返回 | 描述 |
|------|------|------|------|
| `runWorkflowFiles` | `options: { change: string; project_root?: string }` | `{ written: string[]; deleted: string[] }` | 只读查询指定 change 的文件清单净状态 |
| `readFileInventory` | `changeDir: string` | `FileInventory` | 读取并校验 `workflow.json` 的 `files`（自 `lib/file-inventory.ts` 迁移） |
| `foldFileOps` | `inventory: FileInventory, ops: FileOp[]` | `FileInventory` | 净状态折叠（纯函数，迁移） |
| `writeFileInventory` | `changeDir: string, files: FileInventory` | `void` | 写回 `files` 净状态（迁移） |
| 类型 | `FileInventory` / `FileOp` | — | 清单净状态与文件操作类型（迁移） |

### API 接口

| method | path | request | response |
|--------|------|---------|----------|
| MCP tool | `workflow_files` | `{ change: string, project_root?: string }` | `{ written: string[], deleted: string[] }` |

### CLI 命令

无 CLI 命令。

### 前端组件

无前端组件定义。
