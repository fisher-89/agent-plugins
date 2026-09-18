# change-files Specification (delta)

## MODIFIED Requirements

### Requirement: change_files MCP 工具注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `change_files` 的 MCP 工具,作为 change 文件清单(`workflow.json` 的 `file_log` 字段,净状态经派生视图读取)的手动补录与显式修正通道。工具 SHALL 接收至少以下输入:

- `change`:`string`,必填,目标 change 名
- `op`:`"append" | "set"`,必填
- `written` / `deleted`:`string[]`,可选路径列表(至少提供其一)

工具 SHALL 校验 change 存在且 `workflow.json` 通过 `workflowFileSchema`;对缺失 `file_log` 字段的旧 change SHALL 硬报错并指引重建,MUST NOT 静默补建字段以外的元数据。写保护对 `workflow.json` 的既有规则不受影响——本工具经 MCP `fs` 写入,与 `phase_log` / `backtrack` 同一通道。

#### Scenario: 工具出现在 MCP 列表

- **WHEN** MCP server 启动
- **THEN** `change_files` 出现在可用工具列表,inputSchema 含必填 `change` 与 `op`

#### Scenario: 旧 change 硬报错

- **WHEN** `change_files` 目标 change 的 `workflow.json` 无 `file_log` 字段
- **THEN** 返回错误并指引重建
- **AND** MUST NOT 修改该文件

### Requirement: append 语义为 workflow scope 按 path upsert

`op: "append"` SHALL 将传入路径以 **workflow scope 记录** upsert 进 `file_log`:append `written` 路径产生/覆盖为 `{ op: 'write', scope: 'workflow' }` 记录,append `deleted` 路径产生/覆盖为 `{ op: 'delete', scope: 'workflow' }` 记录——workflow scope 内按 path 键控,同 path 原位覆盖(后条胜),不产生重复记录。该 path 若已有 phase scope 审计记录 SHALL 保留(派生净状态由后条胜规则自然被 workflow 记录覆盖),即 append 补录 MUST NOT 抹掉 phase 级审计历史。append SHALL NOT 应用 gitignore 过滤。upsert 的确切实现形态(签名与排序位置)由 design 定。

#### Scenario: append write 路径覆盖派生净状态

- **WHEN** 派生净状态 deleted 含 `src/a.ts`,调用 `append` 且 `written: ["src/a.ts"]`
- **THEN** `file_log` 新增/覆盖该 path 的 `{ op: "write", scope: "workflow" }` 记录,派生净状态 written 含 `src/a.ts` 且 deleted 不含

#### Scenario: append 保留 phase 审计记录

- **WHEN** `file_log` 已含 `src/b.ts` 的 `{ op: "write", scope: "implement", attempt: 1 }` 记录,调用 `append` 且 `written: ["src/b.ts"]`
- **THEN** 该 phase 记录仍在 log 中,新增 workflow scope 记录;派生净状态 written 含 `src/b.ts`(仅一条)

#### Scenario: append 去重

- **WHEN** `file_log` 已含 `src/b.ts` 的 workflow scope write 记录,再次 `append` 且 `written: ["src/b.ts"]`
- **THEN** workflow scope 内 `src/b.ts` 仅一条 write 记录(原位覆盖)

### Requirement: set 语义为净状态覆写

`op: "set"` SHALL 使涉及 path 的派生净状态与传入数组精确一致,用于 hook 认不出的批量还原、`git stash`/`git clean` 后的状态修正,以及非 Claude 环境的手动灌入。log 级映射 SHALL 为:**删除涉及 path 的全部既有记录(含 phase 审计记录)+ 以传入桶追加对应 workflow scope 记录**——即 set 会抹掉这些 path 的 phase 级审计(与现行 set 覆盖净状态的精神一致);未涉及 path 的记录(含 phase 审计)MUST NOT 改动。未提供的桶保持不变。`set` MUST NOT 触发追加折叠, MUST NOT 修改 `workflow.json` 的其他字段(`workflow_type`、`created`、`eval`、`active_phase`、`interrupted`、未知键保留),MUST NOT 应用 gitignore 过滤。确切的记录删除与追加实现形态由 design 定。

#### Scenario: set 覆写 written 派生净状态

- **WHEN** 派生净状态 written 为 `["src/a.ts", "src/b.ts"]`,调用 `set` 且 `written: ["src/a.ts"]`
- **THEN** 派生净状态 written 精确等于 `["src/a.ts"]`(`src/b.ts` 的全部 log 记录被删除)

#### Scenario: set 抹除涉及 path 的 phase 审计但不动其余

- **WHEN** `file_log` 含 `src/a.ts` 的 phase scope 记录与 `src/c.ts` 的 phase scope 记录,调用 `set` 且 `written: ["src/a.ts"]`
- **THEN** `src/a.ts` 的记录被删除并代以 workflow scope 记录;`src/c.ts` 的 phase 审计记录逐字保留

#### Scenario: set 保留其他字段

- **WHEN** 对含 `workflow_type` / `created` / `eval` / `active_phase` 的 `workflow.json` 调用 `set`
- **THEN** 这些字段逐字节保留(含格式:2 空格缩进 + 末尾换行)

### Requirement: 查询输出聚合净状态且不含审计明细

`workflow_files` SHALL 读取 `<projectRoot>/openspec/changes/<change>/workflow.json`,聚合 `file_log` 的派生净状态并输出 `{ written: string[], deleted: string[] }`(相对项目根 POSIX 风格路径,目录删除以目录路径本身返回)——输出形状与存储重构前一致。输出 MUST NOT 包含 `file_log` 明细(scope / attempt / at 审计记录)——该明细仅审计用途、消费方 MUST NOT 依赖,agent 正是该禁令的约束对象。

#### Scenario: 返回派生净状态

- **WHEN** 目标 change 派生净状态 written 为 `["src/a.ts"]`、deleted 为 `["src/old.ts"]`,调用 `workflow_files`
- **THEN** 输出 `written` 精确等于 `["src/a.ts"]`、`deleted` 精确等于 `["src/old.ts"]`

#### Scenario: 输出不含审计明细

- **WHEN** 目标 change 的 `file_log` 含 phase scope 记录,调用 `workflow_files`
- **THEN** 输出仅含 `written` 与 `deleted` 两个字段,不含 `file_log` 明细

### Requirement: 查询硬报错语义与既有消费方一致

`workflow_files` SHALL 复用与其它清单消费方一致的硬报错语义,MUST NOT 回退 git diff、MUST NOT 静默补建字段:目标 change 的 `workflow.json` 不存在、JSON 非法、未通过 `workflowFileSchema`、或缺失 `file_log` 字段(机制前旧 change)时,SHALL 返回错误且文案含重建指引。

#### Scenario: 旧 change 缺 file_log 字段硬报错

- **WHEN** 目标 change 的 `workflow.json` 无 `file_log` 字段,调用 `workflow_files`
- **THEN** 返回错误且文案含重建指引
- **AND** MUST NOT 回退为 git 工作区 diff

#### Scenario: 文件缺失硬报错

- **WHEN** 目标 change 目录不存在或其中无 `workflow.json`,调用 `workflow_files`
- **THEN** 返回错误并指引经 `change_create` 创建

### Requirement: workflow.json 文件清单逻辑统一由 modules/workflow 导出

系统 SHALL 以 `plugins/dev-team/bin/src/modules/workflow/` 目录(含其 `files/` 子目录)作为 `workflow.json` 操作逻辑的模块边界:文件清单的读取、log 覆盖/追加、派生与查询逻辑 SHALL 位于该目录并从 `workflow/index.ts` 统一导出(`readFileLog`、`appendLogEntries`、`deriveNetState`、`recordFileOps`、`getChangedFiles` 及 `FileLogEntry` / `FileOp` 类型,确切签名以 workflow-file-inventory 规格的 Module Contract 为准)。**写入 `file_log` 字段的完整业务实现亦 SHALL 位于该目录**:PostToolUse 记录器的归账管线(路径规范化、自污染排除、gitignore 过滤、scope 写 log、落盘)与 `change_files` 的 append/set 语义 SHALL 全部位于该目录;`commands/record-files.ts` SHALL 仅保留 hook 事件解析(stdin 读取、`tool_input` 提取、session 绑定、`phase_next` / `phase_start` 识别、门控判定与 scope 解析)与错误吞并策略;`commands/change-files.ts` SHALL 仅保留输入校验与委托——两者 MUST NOT 内联 log 覆盖/落盘实现。`lib/file-inventory.ts` 保持删除状态,既有消费方(`change_files`、PostToolUse 记录器、`test-execution`、`test-resolve-paths`、`c4-cross-ref`、`shell-file-ops`)SHALL 从 `modules/workflow` 导入,MUST NOT 保留 re-export shim。后续新增的操作 `workflow.json` 的逻辑 SHALL 同样位于 `modules/workflow/` 并从 `workflow/index.ts` 导出(既有 eval 写入、创建、回溯逻辑的迁移由后续 change 增量处理,本变更 MUST NOT 一次性搬空)。

#### Scenario: 单一出口无旧路径残留

- **WHEN** 在仓库内 grep `lib/file-inventory` 导入,及 `commands/record-files.ts` / `commands/change-files.ts` 中的 `foldFileOps` / `writeFileInventory` / `readFileInventory` 残留引用
- **THEN** 无任何源码或测试残留该导入路径与旧桶形态 API,两命令文件内无 log 覆盖/落盘实现
- **AND** `plugins/dev-team/bin/src/lib/file-inventory.ts` 不存在,迁移不留 shim

#### Scenario: 记录器委托模块行为不变

- **WHEN** 门控通过,记录器收到 `tool_name: "Write"`、`tool_input.file_path: "<root>/src/foo.ts"` 的 PostToolUse 事件
- **THEN** 归账经模块归账管线完成,`file_log` 以门控解析的 scope 新增该路径的 write 记录
- **AND** 自污染排除、gitignore 过滤与覆盖/追加行为与规格一致

#### Scenario: change_files 委托模块语义一致

- **WHEN** 调用 `change_files`(append 或 set)
- **THEN** workflow scope upsert / 覆盖语义、phase 审计保留策略与 `workflow_type` / `created` / `eval` / `active_phase` 保留行为与规格一致
- **AND** 实现位于 `modules/workflow/`,命令层仅做输入校验与委托

#### Scenario: 后续逻辑入口约定

- **WHEN** 后续 change 为 `workflow.json` 新增操作逻辑
- **THEN** 该逻辑位于 `plugins/dev-team/bin/src/modules/workflow/` 并经 `workflow/index.ts` 导出

## Module Contract

### MCP 工具注册: `plugins/dev-team/bin/src/mcp.ts`

| 字段 | 值 |
|------|-----|
| 工具名 | `workflow_files` |
| 输入 | `{ change: string, project_root?: string }` |
| 输出 | `{ written: string[], deleted: string[] }`(派生净状态,无 `file_log` 明细) |
| handler | 解析 project_root → `getChangedFiles`(读 `file_log` → `deriveNetState`)→ 只读返回;无任何写路径 |

### Module: `plugins/dev-team/bin/src/modules/workflow/`(log 形态,详表见 workflow-file-inventory 规格)

| 函数 | 参数 | 返回 | 描述 |
|------|------|------|------|
| `getChangedFiles` | `options: { change: string; project_root?: string }` | `{ written: string[]; deleted: string[] }` | 只读查询派生净状态(签名不变,实现改读 `file_log`) |
| `readFileLog` | `changeDir: string` | `FileLogEntry[]` | 读取并校验 `file_log`(取代 `readFileInventory`) |
| `appendLogEntries` | `changeDir: string, entries: FileLogEntry[]` | `FileLogEntry[]` | 键控覆盖/追加后落盘(取代 `foldFileOps` + `writeFileInventory` 桶形态) |
| `recordFileOps` | `changeDir, ops, context(含门控解析的 scope)` | `void` | 归账管线:规范化 → 排除 → 过滤 → scope 写 log |
| `appendWorkflowFiles` | `changeDir, paths` | 派生净状态 | append 语义:workflow scope 按 path upsert(取代 `appendFileOps`) |
| `setWorkflowFiles` | `changeDir, paths` | 派生净状态 | set 语义:删涉及 path 全部记录 + 追加 workflow 记录(取代 `setFileBuckets`) |

### 消费方(迁移后形态)

| 文件 | 保留职责 | 移除内容 |
|------|---------|---------|
| `commands/record-files.ts` | stdin 解析、`phase_next` / `phase_start` 识别、门控判定与 scope 解析、`tool_input` 提取、错误吞并 exit-0 | 清单读写内联实现(一律经模块) |
| `commands/change-files.ts` | options 校验、`resolveChangeDir`、委托 | 旧桶形态 append/set 委托(改委托 log API) |

### API 接口

| method | path | request | response |
|--------|------|---------|----------|
| MCP tool | `change_files` | `{ change: string, op: "append"\|"set", written?: string[], deleted?: string[] }` | `{ written: string[], deleted: string[] }`(派生净状态,实现委托模块) |
| MCP tool | `workflow_files` | `{ change: string, project_root?: string }` | `{ written: string[], deleted: string[] }`(派生净状态) |

---
