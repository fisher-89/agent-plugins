# 任务: workflow-files-query-api

> 依赖顺序：schema → 模块迁移（file-inventory / files-query / barrel）→ 导入方切换与旧文件删除 → MCP 注册 → agent 提示切换 → 质量门禁与产物。测试文件由 test-design / test-gen 阶段承接，不在本列表（含 proposal 未列出的 `hooks.test.ts` 与 `commands/test-execution.test.ts` 两处 `vi.mock` 旧路径，见 design.md 信息性说明）。

## 阶段一：schema 与模块迁移

- [x] 新建 `plugins/dev-team/bin/src/schemas/workflow-files.schema.ts`：定义 `workflowFilesInputSchema`（`change: z.string().min(1)` 必填 + `projectRootSchema`，中文 `.describe()` 仿照 `change-files.schema.ts`）与 `workflowFilesOutputSchema`（`written` / `deleted` 字符串数组），导出派生类型 `WorkflowFilesInput` / `WorkflowFilesOutput`
- [x] 修改 `plugins/dev-team/bin/src/schemas/index.ts`：追加导出 `workflowFilesInputSchema` / `workflowFilesOutputSchema` / `type WorkflowFilesInput` / `type WorkflowFilesOutput`
- [x] 新建 `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts`：将 `plugins/dev-team/bin/src/lib/file-inventory.ts` 内容逐字迁移（`readFileInventory` / `foldFileOps` / `writeFileInventory`、`FileInventory` / `FileOp` 类型、四态硬报错文案、写回保留键纪律），仅调整文件头注释与相对导入（`../schemas` → `../../schemas`、`../utils` → `../../utils`）
- [x] 新建 `plugins/dev-team/bin/src/modules/workflow/files-query.ts`：实现 `export function runWorkflowFiles(options: WorkflowFilesOptions): WorkflowFilesOutput`（`project_root` 缺省回退 `getProjectDir()`，`getChangeDir` 定位后 `readFileInventory` 读取，仅投影 `{ written, deleted }` 返回；无任何 fs 写调用、无 git diff 回退）；导出 `type WorkflowFilesOptions = Omit<WorkflowFilesInput, 'project_root'> & { project_root?: string }`
- [x] 新建 `plugins/dev-team/bin/src/modules/workflow/index.ts`：barrel 统一导出 `readFileInventory` / `foldFileOps` / `writeFileInventory` / `runWorkflowFiles` 及 `FileInventory` / `FileOp` 类型

## 阶段二：导入方切换与旧文件删除

- [x] 修改 `plugins/dev-team/bin/src/commands/change-files.ts`：`FileInventory` / `readFileInventory` / `writeFileInventory` 导入改自 `'../modules/workflow'`（逻辑零改动）
- [x] 修改 `plugins/dev-team/bin/src/commands/record-files.ts`：`FileOp` / `foldFileOps` / `readFileInventory` / `writeFileInventory` 导入改自 `'../modules/workflow'`
- [x] 修改 `plugins/dev-team/bin/src/commands/test-execution.ts`：`readFileInventory` 导入改自 `'../modules/workflow'`
- [x] 修改 `plugins/dev-team/bin/src/commands/test-resolve-paths.ts`：`readFileInventory` 导入改自 `'../modules/workflow'`
- [x] 修改 `plugins/dev-team/bin/src/lib/c4-cross-ref.ts`：`readFileInventory` 导入改自 `'../modules/workflow'`
- [x] 修改 `plugins/dev-team/bin/src/lib/shell-file-ops.ts`：`type FileOp` 导入改自 `'../modules/workflow'`
- [x] 删除 `plugins/dev-team/bin/src/lib/file-inventory.ts`（不留 re-export shim）
- [x] 全仓 grep 验证：源码与测试中无 `lib/file-inventory` 导入残留（AC-4）

## 阶段三：MCP 工具注册

- [x] 修改 `plugins/dev-team/bin/src/mcp.ts`：顶部静态导入 `runWorkflowFiles` 与 `workflowFilesInputSchema` / `workflowFilesOutputSchema`；`MCP_TOOLS` 数组紧邻 `change_files` 新增 `workflow_files` 条目——inputSchema 必填 `change`、handler 经 `withResolvedProjectRoot('workflow_files', …)` 注入 `project_root` 后调用 `runWorkflowFiles` 并以 `jsonContent` 返回（仿照 `change_files` 接线）
- [x] 撰写 `workflow_files` description：表述为只读查询（从不修改 `workflow.json`、不含 `source` 审计映射、四态硬报错且不回退 git diff），并写明与写通道 `change_files` 的分工，MUST NOT 引导绕过写保护（AC-1）

## 阶段四：agent 提示切换

- [x] 修改 `plugins/dev-team/agents/implementation-evaluator.md`：Process 第 3 步与 Input 节的清单获取改为调用 `__MCP:workflow_files__`（输入目标 change 名）；「范围三态对账」节中 `files` 缺失情形表述改为「查询工具硬报错时按其文案重建」；判定表与三态权威表述保留，移除「Read `workflow.json`」清单读取指令
- [x] 修改 `plugins/dev-team/agents/code-review-evaluator.md`：Process 第 3 步改调用 `__MCP:workflow_files__`，Inspect 节范围权威句同步；`git diff` 仅观察辅助的约束原样保留
- [x] 修改 `plugins/dev-team/agents/acceptance-evaluator.md`：Inspect 节范围权威句由 `workflow.json.files` 改为经 `__MCP:workflow_files__` 获取的清单
- [x] grep 验证：三个 evaluator md 中无「Read `workflow.json`」清单读取指令残留，`__MCP:workflow_files__` 占位符与 mcp.ts 注册的工具名逐字一致（AC-5）

## 阶段五：质量门禁与产物

- [x] 运行 `pnpm -C plugins/dev-team run check`（`vp check --fix && knip`）：确认无 dead export / dead file（barrel 各导出均有消费方）、无旧路径残留
- [x] 提升 `plugins/dev-team/package.json` `version`（`2.10.37` → `2.10.38`，patch 位递增）
- [x] 执行 `pnpm -C plugins/dev-team run build` 刷新 `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/` 产物，git status 确认三产物目录已更新（AC-6）
