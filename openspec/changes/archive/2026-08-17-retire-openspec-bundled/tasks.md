# 任务: retire-openspec-bundled

> **变更**: retire-openspec-bundled
> **日期**: 2026-08-14

---

## 阶段 1: Schema 定义

- [x] 创建 `plugins/dev-team/bin/src/schemas/change-create.schema.ts`，定义 `changeCreateInputSchema`（`name: z.string().min(1).max(128).regex(kebabCasePattern)` + `project_root: projectRootSchema`）和 `changeCreateOutputSchema`（`name: z.string()`, `path: z.string()`）
- [x] 创建 `plugins/dev-team/bin/src/schemas/spec-list.schema.ts`，定义 `specListInputSchema`（`project_root: projectRootSchema`）和 `specListOutputSchema`（`specs: z.array(z.object({ name, path, description }))`）
- [x] 在 `plugins/dev-team/bin/src/schemas/index.ts` 中导出 `change-create.schema` 和 `spec-list.schema` 的全部符号

## 阶段 2: 工具实现

- [x] 在 `plugins/dev-team/bin/src/commands/phase-next.ts` 中，将 `hasPhasePassed` 函数从 `function` 改为 `export function`（供 `change-list.ts` 复用）
- [x] 实现 `plugins/dev-team/bin/src/commands/change-create.ts`：
  - 导出 `runChangeCreate(name: string, projectRoot: string): { name: string; path: string }`
  - 校验 name 为 kebab-case（`^[a-z0-9][a-z0-9-]*$`），最长 128 字符
  - 检查 `openspec/changes/<name>/` 是否已存在，存在则抛错
  - 创建目录并写入 `workflow.json`（`{ "workflow_type": "requirement", "created": "YYYY-MM-DD" }`）
- [x] 实现 `plugins/dev-team/bin/src/commands/spec-list.ts`：
  - 导出 `runSpecList(projectRoot: string): { specs: { name: string; path: string; description: string }[] }`
  - 扫描 `openspec/specs/*/spec.md`，读取每个文件的 `## <name>` 标题作为 description
  - 对读取失败或格式异常的文件静默跳过
- [x] 扩展 `plugins/dev-team/bin/src/commands/change-list.ts`：
  - 导入 `getPhaseTable`（来自 `lib/workflow.ts`）、`readEvalJson`（来自 `lib/eval-json.ts`）、`hasPhasePassed`（来自 `phase-next.ts`）
  - 新增内部函数 `computeWorkflowDone(changeDir: string, projectRoot: string): boolean`
  - 逻辑：读 `workflow.json` 得 `workflow_type` → `getPhaseTable(workflowType)` → `readEvalJson(changeDir)` → 检查所有 phase 是否 `hasPhasePassed`
  - 在 `processChangeEntry` 的返回对象中新增 `workflow_done` 字段
- [x] 扩展 `plugins/dev-team/bin/src/schemas/change-list.schema.ts`：
  - 在 `changeEntrySchema` 中新增 `workflow_done: z.boolean().describe('所有 phase 是否已完成（非 stale 的 pass/skipped）')`

## 阶段 3: MCP 工具注册

- [x] 在 `plugins/dev-team/bin/src/mcp.ts` 中：
  - 导入 `changeCreateInputSchema`, `changeCreateOutputSchema`, `specListInputSchema`, `specListOutputSchema` 及 `runChangeCreate`, `runSpecList`
  - 在 `MCP_TOOLS` 数组中新增 `change_create` 条目（name/description/inputSchema/outputSchema/handler）
  - 在 `MCP_TOOLS` 数组中新增 `spec_list` 条目
  - `change_list` 条目无需修改 handler（`runChangeList` 内部已返回 `workflow_done`）

## 阶段 4: 构建管线清理

- [x] 在 `plugins/dev-team/build/assemble.ts` 中：从 `STATIC_BIN_FILES` 移除 `'bin/openspec'`、`'bin/openspec-bundled.js'`、`'bin/openspec.cmd'`
- [x] 在 `plugins/dev-team/build/assemble.ts` 中：从 `writeHomeExtras` 的 `extraManaged` 数组移除 `'bin/openspec'`、`'bin/openspec-bundled.js'`、`'bin/openspec.cmd'`
- [x] 在 `plugins/dev-team/build/scan-files.ts` 中：从 `EXCLUDE_BASENAMES` 移除 `'openspec-bundled.js'`
- [x] 在 `plugins/dev-team/vite.config.ts` 中：从 `NO_OXC_FILES` 数组移除 `'bin/openspec-bundled.js'`
- [x] 在 `plugins/dev-team/home-install.ts` 中：移除 `writeExpanded` 函数内 `base === 'openspec-bundled.js'` 的特判分支，使所有文件统一走 `readFileSync` + `expandPathTokens` 或直接复制

## 阶段 5: Skill/Agent 调用点更新

- [x] 修改 `plugins/dev-team/agents/proposal-planner.md`：将第 5 步的 `source __DEV_TEAM_ROOT__/utils/openspec-cli.sh && openspec_spec_list "<name>"` 替换为 `__MCP:spec_list__`
- [x] 修改 `plugins/dev-team/skills/openspec-archive-change/SKILL.md`：
  - 将第 2 步的 `openspec status --change "<name>" --json` 替换为使用 `__MCP:change_list__` 返回的 `artifacts`（文件存在性检查）和 `workflow_done`（workflow 完成状态）
  - 删除第 3 步整个 `__MCP:phase_check__` 幽灵引用块（该工具不存在）
  - 更新 guardrail 注释中关于 `.openspec.yaml` 的引用（第 114 行「Preserve .openspec.yaml」）

## 阶段 6: 删除文件

- [x] 删除 `plugins/dev-team/bin/openspec`（bash wrapper）
- [x] 删除 `plugins/dev-team/bin/openspec.cmd`（Windows wrapper）
- [x] 删除 `plugins/dev-team/utils/openspec-cli.sh`（openspec CLI 的 shell 封装，所有函数已无调用方）