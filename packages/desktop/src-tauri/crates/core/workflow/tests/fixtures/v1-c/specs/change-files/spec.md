# change-files Delta

## MODIFIED Requirements

### Requirement: workflow.json 文件清单逻辑统一由 modules/workflow 导出

系统 SHALL 以 `plugins/dev-team/bin/src/modules/workflow/` 目录（含其 `files/` 子目录）作为 `workflow.json` 操作逻辑的模块边界：文件清单的读取、折叠、写入与查询逻辑 SHALL 位于该目录并从 `workflow/index.ts` 统一导出（`readFileInventory`、`foldFileOps`、`writeFileInventory`、`getChangedFiles` 及 `FileInventory` / `FileOp` 类型）。**写入 `files` 字段的完整业务实现亦 SHALL 位于该目录**：PostToolUse 记录器的归账管线（路径规范化、自污染排除、gitignore 过滤、折叠、落盘，拟 `recordFileOps`）与 `change_files` 的 append/set 净状态折叠语义（拟 `appendFileOps` / `setFileBuckets`，确切签名以 Module Contract 为准）SHALL 迁入该目录并经 `workflow/index.ts` 导出。`commands/record-files.ts` SHALL 仅保留 hook 事件解析（stdin 读取、`tool_input` 提取、session 绑定与 `phase_next` 识别）与错误吞并策略；`commands/change-files.ts` SHALL 仅保留输入校验与委托——两者 MUST NOT 再内联清单折叠/落盘实现。`lib/file-inventory.ts` 保持删除状态，既有消费方（`change_files`、PostToolUse 记录器、`test-execution`、`test-resolve-paths`、`c4-cross-ref`、`shell-file-ops`）SHALL 从 `modules/workflow` 导入，MUST NOT 保留 re-export shim。后续新增的操作 `workflow.json` 的逻辑 SHALL 同样位于 `modules/workflow/` 并从 `workflow/index.ts` 导出（既有 eval 写入、创建、回溯逻辑的迁移由后续 change 增量处理，本变更 MUST NOT 一次性搬空）。

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

### Module: `plugins/dev-team/bin/src/modules/workflow/`

| 函数                                           | 参数                                                                                     | 返回                                       | 描述                                                              |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------ | ----------------------------------------------------------------- |
| `getChangedFiles`                              | `options: { change: string; project_root?: string }`                                     | `{ written: string[]; deleted: string[] }` | 只读查询指定 change 的文件清单净状态                              |
| `readFileInventory`                            | `changeDir: string`                                                                      | `FileInventory`                            | 读取并校验 `workflow.json` 的 `files`                             |
| `foldFileOps`                                  | `inventory: FileInventory, ops: FileOp[]`                                                | `FileInventory`                            | 净状态折叠（纯函数）                                              |
| `writeFileInventory`                           | `changeDir: string, files: FileInventory`                                                | `void`                                     | 写回 `files` 净状态                                               |
| `recordFileOps`（新增，名可调）                | `changeDir: string, ops: FileOp[], context: { projectRoot: string; agentType?: string }` | `void`                                     | 归账管线：规范化 → 自污染排除 → gitignore 过滤 → 读 → 折叠 → 落盘 |
| `appendFileOps`（新增，名可调）                | `changeDir: string, paths: { written?: string[]; deleted?: string[] }`                   | `FileInventory`                            | append 语义（读改写）：折叠合并去重，保留既有 source              |
| `setFileBuckets`（新增，名可调）               | `changeDir: string, paths: { written?: string[]; deleted?: string[] }`                   | `FileInventory`                            | set 语义（读改写）：整桶覆写，覆写条目清除 source                 |
| `loadGitignoreFilter` / `isGitIgnored`（新增） | 见 workflow-file-inventory delta                                                         | —                                          | gitignore 过滤器（归账管线内部消费）                              |
| 类型                                           | `FileInventory` / `FileOp`                                                               | —                                          | 清单净状态与文件操作类型                                          |

### 消费方（迁移后形态）

| 文件                       | 保留职责                                                              | 移除内容                                                                                              |
| -------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `commands/record-files.ts` | stdin 解析、`phase_next` 绑定识别、`tool_input` 提取、错误吞并 exit-0 | `normalizeRecordedPath` / `isExcludedFromInventory` / `collectRecordedOps` / 读折叠写步骤（迁入模块） |
| `commands/change-files.ts` | options 校验、`resolveChangeDir`、委托                                | `applyAppend` / `applySet` / `dedupe`（迁入模块）                                                     |

### API 接口

| method   | path             | request                                                                           | response                                                         |
| -------- | ---------------- | --------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| MCP tool | `change_files`   | `{ change: string, op: "append"\|"set", written?: string[], deleted?: string[] }` | `{ written: string[], deleted: string[] }`（不变，实现委托模块） |
| MCP tool | `workflow_files` | `{ change: string, project_root?: string }`                                       | `{ written: string[], deleted: string[] }`（不变）               |
