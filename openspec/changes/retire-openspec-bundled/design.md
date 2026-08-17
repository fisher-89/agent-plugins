# 设计: retire-openspec-bundled

> **变更**: retire-openspec-bundled
> **日期**: 2026-08-14

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| change_create MCP handler | 替代 `openspec new change`，校验 kebab-case 名称、创建目录、写 `workflow.json` 元数据 | `plugins/dev-team/bin/src/commands/change-create.ts` | `fs`, `path`, `schemas/change-create.schema.ts` | TypeScript, Zod |
| change_create input schema | 定义 `change_create` 工具的输入和输出 Zod schema | `plugins/dev-team/bin/src/schemas/change-create.schema.ts` | `zod/v4` | TypeScript, Zod |
| spec_list MCP handler | 替代 `openspec spec list --json`，扫描 `openspec/specs/*/spec.md` 返回 capability 列表 | `plugins/dev-team/bin/src/commands/spec-list.ts` | `fs`, `path`, `schemas/spec-list.schema.ts` | TypeScript, Zod |
| spec_list input schema | 定义 `spec_list` 工具的输入和输出 Zod schema | `plugins/dev-team/bin/src/schemas/spec-list.schema.ts` | `zod/v4` | TypeScript, Zod |
| change_list workflow_done 扩展 | 在 `runChangeList` 中为每个 change 计算 `workflow_done` 布尔字段 | `plugins/dev-team/bin/src/commands/change-list.ts` | `lib/workflow.ts`, `lib/eval-json.ts`, `phase-next.ts` | TypeScript |
| change_list output schema 扩展 | 在 `changeEntrySchema` 中新增 `workflow_done: z.boolean()` 字段 | `plugins/dev-team/bin/src/schemas/change-list.schema.ts` | `zod/v4` | TypeScript, Zod |
| schemas/index.ts 导出 | 新增 `change-create.schema` 和 `spec-list.schema` 的导出条目 | `plugins/dev-team/bin/src/schemas/index.ts` | — | TypeScript |
| MCP 工具注册 | 在 `mcp.ts` 中注册 `change_create`、`spec_list` 工具；扩展 `change_list` handler 传递 `workflow_done` | `plugins/dev-team/bin/src/mcp.ts` | 所有 MCP 工具 handler | TypeScript |
| assemble.ts 静态资产清理 | 从 `STATIC_BIN_FILES` 和 `writeHomeExtras` 的 `extraManaged` 移除 openspec 相关文件 | `plugins/dev-team/build/assemble.ts` | — | TypeScript |
| scan-files.ts 排除项清理 | 从 `EXCLUDE_BASENAMES` 移除 `'openspec-bundled.js'` | `plugins/dev-team/build/scan-files.ts` | — | TypeScript |
| vite.config.ts 排除项清理 | 从 `NO_OXC_FILES` 移除 `'bin/openspec-bundled.js'` | `plugins/dev-team/vite.config.ts` | — | TypeScript |
| home-install.ts 特判清理 | 移除 `writeExpanded` 中对 `openspec-bundled.js` 的 token 展开特判 | `plugins/dev-team/home-install.ts` | — | TypeScript |
| proposal-planner.md 调用点更新 | 将 `source .../openspec-cli.sh && openspec_spec_list` 改为 `__MCP:spec_list__` | `plugins/dev-team/agents/proposal-planner.md` | — | Markdown |
| archive SKILL.md 调用点更新 | 移除 `__MCP:phase_check__` 幽灵引用；将 `openspec status --json` 改为 `__MCP:change_list__` 的 `workflow_done` + `artifacts` | `openspec/specs/openspec-archive-change/SKILL.md`（注意：实际路径为 `plugins/dev-team/skills/openspec-archive-change/SKILL.md`） | — | Markdown |
| phase-next.ts hasPhasePassed 导出 | 将 `hasPhasePassed` 从 local function 改为 `export`，供 `change-list.ts` 复用 | `plugins/dev-team/bin/src/commands/phase-next.ts` | — | TypeScript |
| bin/openspec | 删除 bash wrapper | `plugins/dev-team/bin/openspec` | — | Shell |
| bin/openspec.cmd | 删除 Windows wrapper | `plugins/dev-team/bin/openspec.cmd` | — | Batch |
| utils/openspec-cli.sh | 删除 openspec CLI 的 shell 封装 | `plugins/dev-team/utils/openspec-cli.sh` | — | Shell |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
  签名格式：Python → create_adr(title: str, status: str = "proposed") -> dict；TS → function parseImports(file: string): Import[]
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/commands/change-create.ts` | change_create MCP 工具实现：校验名称、创建目录、写 workflow.json |
| `plugins/dev-team/bin/src/commands/spec-list.ts` | spec_list MCP 工具实现：扫描 `openspec/specs/*/spec.md` 返回 capability 列表 |
| `plugins/dev-team/bin/src/schemas/change-create.schema.ts` | change_create 工具的输入/输出 Zod schema 定义 |
| `plugins/dev-team/bin/src/schemas/spec-list.schema.ts` | spec_list 工具的输入/输出 Zod schema 定义 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/mcp.ts` | 导入 `change-create` 和 `spec-list` 的 schema 和 handler；在 `MCP_TOOLS` 数组中新增两个注册条目；`change_list` 的 handler 无需修改（`runChangeList` 内部已计算 `workflow_done`） | 注册新工具，扩展已有工具 |
| `plugins/dev-team/bin/src/commands/change-list.ts` | 新增 `computeWorkflowDone` 内部函数；在 `processChangeEntry` 中调用并写入返回对象的 `workflow_done` 字段；导入 `getPhaseTable`、`readEvalJson`、`hasPhasePassed` | 为每个 change 计算 workflow 完成状态 |
| `plugins/dev-team/bin/src/schemas/change-list.schema.ts` | 在 `changeEntrySchema` 中新增 `workflow_done: z.boolean()` 字段 | 扩展输出 schema |
| `plugins/dev-team/bin/src/schemas/index.ts` | 新增 `change-create.schema` 和 `spec-list.schema` 的导出 | 注册新 schema 的 barrel export |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 将 `hasPhasePassed` 函数从 `function` 改为 `export function` | 供 `change-list.ts` 复用 |
| `plugins/dev-team/build/assemble.ts` | 从 `STATIC_BIN_FILES` 移除 `'bin/openspec'`、`'bin/openspec-bundled.js'`、`'bin/openspec.cmd'`；从 `writeHomeExtras` 的 `extraManaged` 数组移除 `'bin/openspec'`、`'bin/openspec-bundled.js'`、`'bin/openspec.cmd'` | 构建管线不再复制 openspec bundle 及其 wrapper |
| `plugins/dev-team/build/scan-files.ts` | 从 `EXCLUDE_BASENAMES` 移除 `'openspec-bundled.js'` | 不再需要排除 bundle |
| `plugins/dev-team/vite.config.ts` | 从 `NO_OXC_FILES` 数组移除 `'bin/openspec-bundled.js'` | 不再需要排除 bundle |
| `plugins/dev-team/home-install.ts` | 在 `writeExpanded` 函数中移除 `base === 'openspec-bundled.js'` 的特判，改为对所有文件统一执行 `readFileSync` + `expandPathTokens` 或直接按现有逻辑 fallback | 不再需要特殊处理 bundle |
| `plugins/dev-team/agents/proposal-planner.md` | 将第 5 步的 `source __DEV_TEAM_ROOT__/utils/openspec-cli.sh && openspec_spec_list "<name>"` 改为 `__MCP:spec_list__` | 调用点迁移 |
| `plugins/dev-team/skills/openspec-archive-change/SKILL.md` | 移除第 2 步的 `openspec status --json` 调用，改为使用 `__MCP:change_list__` 返回的 `artifacts` 和 `workflow_done` 字段；移除第 3 步的 `__MCP:phase_check__` 幽灵引用及其整个步骤块；更新 guardrail 中不再引用 `.openspec.yaml` | 调用点迁移，清理幽灵引用 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `runChangeCreate` | `commands/change-create.ts` | 新增 | `function runChangeCreate(name: string, projectRoot: string): { name: string; path: string }` | 创建 change 目录并写入 workflow.json 元数据。校验 kebab-case 格式，拒绝已存在的目录 |
| `runSpecList` | `commands/spec-list.ts` | 新增 | `function runSpecList(projectRoot: string): { specs: { name: string; path: string; description: string }[] }` | 扫描 `openspec/specs/*/spec.md`，返回 capability 列表 |
| `hasPhasePassed` | `commands/phase-next.ts` | 修改（local → export） | `function hasPhasePassed(entries: EvalEntry[], phaseId: string): boolean` | 判断某 phase 是否有非 stale 的 pass/skipped 条目 |
| `changeCreateInputSchema` | `schemas/change-create.schema.ts` | 新增 | `ZodObject` | change_create 工具输入 schema：`name: z.string()`, `project_root: z.string()` |
| `changeCreateOutputSchema` | `schemas/change-create.schema.ts` | 新增 | `ZodObject` | change_create 工具输出 schema：`name: z.string()`, `path: z.string()` |
| `specListInputSchema` | `schemas/spec-list.schema.ts` | 新增 | `ZodObject` | spec_list 工具输入 schema：`project_root: projectRootSchema` |
| `specListOutputSchema` | `schemas/spec-list.schema.ts` | 新增 | `ZodObject` | spec_list 工具输出 schema：`specs: z.array(...)` |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| 不涉及 | — | — | 本变更不新增公共类型定义（interface / type alias / enum）。`change-list.schema.ts` 的 `changeEntrySchema` 新增 `workflow_done` 字段在 schema 层面完成，不新增独立类型。`change-create.schema.ts` 和 `spec-list.schema.ts` 的 schema 导出本身即为类型（`z.infer`），无需额外类型定义。 |

### 配置

<!-- 本变更不涉及 plugin.json、config.json、hooks.json、settings.json 等配置文件的键变更。构建管线中移除的特判（STATIC_BIN_FILES、EXCLUDE_BASENAMES、NO_OXC_FILES、home-install.ts 分支）是代码常量而非配置键，不影响运行时行为。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `workflow.json` | `workflow_type: string`（默认 `"requirement"`），`created: string`（ISO 日期 `YYYY-MM-DD`） | 每个 change 目录下唯一 | `openspec/changes/<name>/workflow.json`，由 `change_create` 工具写入 |
| `change_list` 返回条目 | `name`, `artifacts`, `tasks`, `latest_phase`, `workflow_done: boolean` | `workflow_done` 由 `workflow.json` → `getPhaseTable` → `eval.json` 三件推导得出 | 无持久化，实时计算。`workflow_done` 不写入磁盘 |
| `spec_list` 返回条目 | `name`, `path`, `description` | 每个 spec 文件对应一个 capability | 无持久化，实时扫描 `openspec/specs/<capability>/spec.md` |

---

## 路由/API 设计

<!-- 本变更为 MCP 工具变更，不涉及 HTTP API。此节省略。 -->

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。`change_create` 和 `spec_list` 均使用 `fs`、`path` 标准库及已有的 `zod/v4` 依赖。

### 构建/测试依赖

- 无新增构建/测试依赖。

---

## 待决问题

- 无。所有决策已在 `explore.md` 中落定。