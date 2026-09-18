# 设计: workflow-files-query-api

> **变更**: workflow-files-query-api
> **日期**: 2026-09-17

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `workflow_files` MCP 工具注册 | 在 `MCP_TOOLS` 数组注册只读查询工具：inputSchema/outputSchema 绑定、只读 description、经 `withResolvedProjectRoot` 解析注入 `project_root` 后调用 `runWorkflowFiles` | `plugins/dev-team/bin/src/mcp.ts` | `schemas`（新 schema）、`modules/workflow`（`runWorkflowFiles`）、`lib/project-root` | TypeScript + `@modelcontextprotocol/sdk` `registerTool` |
| 清单查询命令 `files-query` | 只读聚合指定 change 的 `files` 净状态并投影为 `{ written, deleted }`（结构性排除 `source`）；无任何写路径 | `plugins/dev-team/bin/src/modules/workflow/files-query.ts` | `modules/workflow/file-inventory`（`readFileInventory`）、`lib/change`（`getChangeDir`）、`lib/project-root`（`getProjectDir`）、`schemas` | TypeScript |
| 文件清单库（迁移） | `workflow.json.files` 的读取（四态硬报错）、折叠、写回；逻辑逐字迁移，仅改文件位置与相对导入 | `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | `schemas`（`workflowFileSchema`）、`utils`（`isPlainObject`）、`node:fs` / `node:path` | TypeScript + zod v4 |
| `modules/workflow` 模块出口 | `workflow.json` 操作逻辑的统一 barrel：`readFileInventory` / `foldFileOps` / `writeFileInventory` / `runWorkflowFiles` 及 `FileInventory` / `FileOp` 类型 | `plugins/dev-team/bin/src/modules/workflow/index.ts` | `file-inventory.ts`、`files-query.ts` | TypeScript |
| `workflow-files` schema | 查询工具输入/输出 schema 与派生类型；输入复用公共 `projectRootSchema` | `plugins/dev-team/bin/src/schemas/workflow-files.schema.ts` | `zod/v4`、`schemas/public/project-root.schema` | zod v4 |
| schemas 出口扩展 | 导出 `workflowFilesInputSchema` / `workflowFilesOutputSchema` 及 `WorkflowFilesInput` / `WorkflowFilesOutput` 类型 | `plugins/dev-team/bin/src/schemas/index.ts` | `workflow-files.schema.ts` | TypeScript |
| evaluator 提示切换 | 三个 evaluator 的范围圈定步骤由「直接 Read `workflow.json`」改为调用 `__MCP:workflow_files__`；三态对账判定语义不变 | `plugins/dev-team/agents/implementation-evaluator.md`、`code-review-evaluator.md`、`acceptance-evaluator.md` | MCP 工具 `workflow_files` | Markdown 提示词 |

### 模块关系与数据流

```
agent (evaluator) --__MCP:workflow_files__--> mcp.ts
  mcp.ts --withResolvedProjectRoot--> runWorkflowFiles(files-query.ts)
    files-query.ts --getChangeDir--> readFileInventory(file-inventory.ts)
      readFileInventory --workflowFileSchema 校验+四态硬报错--> files 净状态
    files-query.ts --投影(仅 written/deleted, 丢弃 source)--> { written, deleted }
```

既有写通道不变：PostToolUse 记录器（`record-files.ts`）与 `change_files` 仍经 `writeFileInventory` / `foldFileOps` 落盘；本变更仅把它们与查询的共同底层迁入 `modules/workflow`，并将全部消费方的导入收敛到 `workflow/index.ts` 单一出口。

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  proposal.md 与 specs/（change-files、phase-agents）已由提案阶段写入并通过，属既有产物，
  不列入本变更清单。测试文件由 test-design / test-gen 阶段承接（见变更清单末尾的信息性说明）。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/modules/workflow/index.ts` | 模块公共出口（barrel）：re-export `file-inventory.ts` 的 `readFileInventory` / `foldFileOps` / `writeFileInventory` 与 `FileInventory` / `FileOp` 类型，及 `files-query.ts` 的 `runWorkflowFiles`。后续 `workflow.json` 操作逻辑统一从此导出 |
| `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 自 `lib/file-inventory.ts` **整体迁移**（内容逐字保留，不做语义改动）：`readFileInventory` / `foldFileOps` / `writeFileInventory` 三函数、`FileInventory` / `FileOp` 类型、四态硬报错文案与写回保留键纪律；仅调整文件头注释与相对导入（`../schemas` → `../../schemas`、`../utils` → `../../utils`） |
| `plugins/dev-team/bin/src/modules/workflow/files-query.ts` | `runWorkflowFiles` 只读查询：`getChangeDir` 定位 change 目录 → `readFileInventory` 读取校验 → 投影返回 `{ written, deleted }`。不引入任何 fs 写调用，返回值结构性排除 `source` |
| `plugins/dev-team/bin/src/schemas/workflow-files.schema.ts` | `workflowFilesInputSchema`（`change` 必填 + `project_root`）与 `workflowFilesOutputSchema`（`written` / `deleted` 字符串数组），及派生类型 `WorkflowFilesInput` / `WorkflowFilesOutput`；形状仿照 `change-files.schema.ts`（含中文 `.describe()`） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/index.ts` | 追加 `export { workflowFilesInputSchema, workflowFilesOutputSchema, type WorkflowFilesInput, type WorkflowFilesOutput } from './workflow-files.schema'` | 与既有 schema 导出风格一致 |
| `plugins/dev-team/bin/src/mcp.ts` | ① 顶部静态导入 `runWorkflowFiles` 与新 schema；② `MCP_TOOLS` 数组新增 `workflow_files` 条目（紧邻 `change_files`，读写成对），handler 仿照 `change_files`：`withResolvedProjectRoot('workflow_files', …)` → `runWorkflowFiles({ change, project_root })` → `jsonContent`；③ description 表述为只读查询并写明与写通道分工 | description 要点：只读、从不修改 `workflow.json`、不含 `source` 审计映射、缺失/非法/schema 不通过/缺 `files` 时硬报错且不回退 git diff、补录或修正请用 `change_files`。新条目位于既有 `// Stryker disable StringLiteral,ArrowFunction` 区块内，无需额外豁免 |
| `plugins/dev-team/bin/src/commands/change-files.ts` | 导入改为 `import { type FileInventory, readFileInventory, writeFileInventory } from '../modules/workflow'` | 逻辑零改动，仅改导入来源至 barrel |
| `plugins/dev-team/bin/src/commands/record-files.ts` | `FileOp` / `foldFileOps` / `readFileInventory` / `writeFileInventory` 的导入改为 `'../modules/workflow'` | 逻辑零改动 |
| `plugins/dev-team/bin/src/commands/test-execution.ts` | `readFileInventory` 导入改为 `'../modules/workflow'` | 逻辑零改动 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `readFileInventory` 导入改为 `'../modules/workflow'` | 逻辑零改动 |
| `plugins/dev-team/bin/src/lib/c4-cross-ref.ts` | `import { readFileInventory } from './file-inventory'` 改为 `'../modules/workflow'` | 逻辑零改动 |
| `plugins/dev-team/bin/src/lib/shell-file-ops.ts` | `import { type FileOp } from './file-inventory'` 改为 `'../modules/workflow'` | 逻辑零改动 |
| `plugins/dev-team/agents/implementation-evaluator.md` | 范围圈定改走查询 API：① Process 第 3 步「Read `workflow.json` 的 `files` 清单」→「Call `__MCP:workflow_files__`（输入目标 change 名）获取文件清单」；② Input 节的 `workflow.json` 读取条目改为经 `__MCP:workflow_files__` 获取；③「范围三态对账」节中 `files` 缺失情形的处置表述改为「查询工具硬报错时按其文案重建」 | 三态对账判定表与「design 变更清单 × `files` × 文件系统」权威表述保留（概念引用，非读取指令）；提示词不得引用查询输出中不存在的 `source` 字段 |
| `plugins/dev-team/agents/code-review-evaluator.md` | ① Process 第 3 步「Read `workflow.json` 的 `files` 清单圈定审查范围」→ 调用 `__MCP:workflow_files__`；② Inspect 节范围权威句同步为「经 `__MCP:workflow_files__` 获取的清单」 | `git diff` 仅观察辅助的约束原样保留；对账语义不变 |
| `plugins/dev-team/agents/acceptance-evaluator.md` | Inspect 节范围权威句「the change file inventory (`workflow.json.files`)」→「the change file inventory fetched via `__MCP:workflow_files__`」 | 本文件无 Process 层直接读取步骤，仅此一处表述 |
| `plugins/dev-team/package.json` | `version` 提升（`2.10.37` → `2.10.38`，patch 位递增，以实现时为准），随后执行 `pnpm -C plugins/dev-team run build` 刷新 `claude-plugins/`、`cursor-plugins/`、`cursor-home-image/` 产物 | CLAUDE.md 升级规则；产物目录由 build 生成，不手改 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/lib/file-inventory.ts` | 迁入 `modules/workflow/file-inventory.ts` 后删除；**不留 re-export shim**（避免 knip 判 dead export），全部消费方在本次一次性改道 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `runWorkflowFiles` | `plugins/dev-team/bin/src/modules/workflow/files-query.ts`（经 `modules/workflow/index.ts` 导出） | 新增 | `export function runWorkflowFiles(options: WorkflowFilesOptions): WorkflowFilesOutput` | 只读查询核心：`project_root` 缺省回退 `getProjectDir()`；`readFileInventory(changeDir)` 取净状态后返回 `{ written, deleted }`。MUST NOT 出现 git diff 回退或字段补建 |
| `workflow_files`（MCP 工具） | `plugins/dev-team/bin/src/mcp.ts` | 新增 | 输入 `{ change: string, project_root?: string }`；输出 `{ written: string[], deleted: string[] }` | 只读查询通道；`change` 为 inputSchema 必填项；命名遵循 `mcp-tool-namespace` 的 `xx_yy` 两段规则（domain=workflow） |
| `readFileInventory` | `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 修改（迁移） | `export function readFileInventory(changeDir: string): FileInventory` | 实现与硬报错文案逐字迁移，仅文件位置变化 |
| `foldFileOps` | `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 修改（迁移） | `export function foldFileOps(inventory: FileInventory, ops: FileOp[]): FileInventory` | 同上（纯函数折叠） |
| `writeFileInventory` | `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 修改（迁移） | `export function writeFileInventory(changeDir: string, files: FileInventory): void` | 同上（保留键写回纪律） |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `FileInventory` | `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 修改（迁移） | `z.infer<typeof workflowFilesSchema>`：`{ written: string[], deleted: string[], source?: Record<string, string> }`；类型本体不变，仅迁移位置 |
| `FileOp` | `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 修改（迁移） | `interface FileOp { op: 'write' \| 'delete' \| 'revert'; path: string; agentType?: string }`；迁移 |
| `WorkflowFilesInput` | `plugins/dev-team/bin/src/schemas/workflow-files.schema.ts` | 新增 | `z.input<typeof workflowFilesInputSchema>` |
| `WorkflowFilesOutput` | `plugins/dev-team/bin/src/schemas/workflow-files.schema.ts` | 新增 | `z.output<typeof workflowFilesOutputSchema>`：`{ written: string[], deleted: string[] }` |
| `WorkflowFilesOptions` | `plugins/dev-team/bin/src/modules/workflow/files-query.ts` | 新增 | `export type WorkflowFilesOptions = Omit<WorkflowFilesInput, 'project_root'> & { project_root?: string }`，仿照 `ChangeFilesOptions` 模式 |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `plugins/dev-team/package.json` | 修改 | `string`（`"2.10.37"` → `"2.10.38"`） | CLAUDE.md 升级规则：改源码后提升版本并 rebuild 三产物目录 |

#### 测试文件影响（信息性说明，不列入上述变更清单，由 test-design / test-gen 阶段承接）

proposal「变更范围 - 测试文件」已列出：迁移 `lib/file-inventory.test.ts` → `modules/workflow/file-inventory.test.ts`、新建 `files-query.test.ts`、修改 `mcp.test.ts` 及 4 个旧路径测试导入。代码检索另发现 **两处 proposal 未列出的 `vi.mock` 旧路径**，删除 `lib/file-inventory.ts` 后 mock 将落空，test-design 时必须一并纳入：

- `plugins/dev-team/bin/src/hooks.test.ts:63` — `vi.mock('./lib/file-inventory', …)` → 需指向 `./modules/workflow`
- `plugins/dev-team/bin/src/commands/test-execution.test.ts:73-74` — `vi.mock('../lib/file-inventory', …)` → 需指向 `../modules/workflow`

另 `plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts` 仅在注释中引用旧路径（非导入，不阻断编译），可顺带刷新。

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `FileInventory`（`workflow.json.files`，既有不变） | `written: string[]`、`deleted: string[]`、`source?: Record<string, string>`（路径 → subagent `agent_type`，仅审计） | 由 `workflowFileSchema`（`schemas/workflow.schema.ts`，本变更不改）承载；`foldFileOps` / `change_files` 写入，`runWorkflowFiles` 只读投影 | `openspec/changes/<change>/workflow.json` 的 `files` 字段（2 空格缩进 + 尾换行；`workflow_type` / `created` / `eval` / 未知键写回时保留） |
| `WorkflowFilesOutput`（新增） | `written: string[]`、`deleted: string[]`（相对项目根 POSIX 风格，目录删除以目录路径返回） | `FileInventory` 的只读投影——查询时新建字面量对象，结构性排除 `source` | 不持久化（MCP 工具输出） |
| 硬报错四态（语义约定，非持久化模型） | ① `workflow.json` 不存在 → 指引经 `change_create` 创建；② JSON 非法/根非对象 → 解析失败报错；③ 未过 `workflowFileSchema` → 格式非法 + issue 明细；④ 缺 `files` → 重建指引 | 四态由迁移后的 `readFileInventory` 统一承载，`workflow_files` 与写通道消费方共用同一文案 | 无 |

---

## 依赖

### 运行时依赖

- 无新增。既有：`zod ^4.4.3`（新 schema 校验）、`@modelcontextprotocol/sdk ^1.29.0`（工具注册，既有）、Node.js 内置 `fs` / `path`（迁移代码沿用）。

### 构建/测试依赖

- 无新增。既有：`typescript ^6.0.3`、`vite-plus`（`vp test` / `vp check`）、`knip ^6.17.1`（knip entry 已含 `bin/src/mcp.ts`，新增 barrel 经 `mcp.ts` / `record-files.ts` 导入链可达，无 dead-export 风险——前提是全部消费方改从 barrel 导入）、`@stryker-mutator`（质量门禁，既有）。

---

## 验收标准对齐

| AC-ID | 设计落点 |
|-------|----------|
| AC-1 | `mcp.ts` `MCP_TOOLS` 新增 `workflow_files` 条目：inputSchema 必填 `change`、description 只读表述（修改文件表 + 公共函数/API 表）；`mcp.test.ts` listTools 断言由测试阶段承接 |
| AC-2 | `modules/workflow/files-query.ts` `runWorkflowFiles`：`readFileInventory` 取净状态后仅投影 `written` / `deleted` 返回（结构性排除 `source`），函数体内无任何 fs 写调用，只读不落盘由实现保证、`files-query.test.ts` 验证 |
| AC-3 | 硬报错四态复用迁移后的 `readFileInventory`（文案含重建指引），`runWorkflowFiles` 不实现 git diff 回退路径（数据模型「硬报错四态」行 + AC-3 对应测试四态覆盖） |
| AC-4 | `modules/workflow/index.ts` barrel 导出四函数两类型；`file-inventory.ts` 整体迁移；6 个导入方改道 barrel；`lib/file-inventory.ts` 删除不留 shim；任务含 grep 验证「无 `lib/file-inventory` 导入残留」 |
| AC-5 | 三个 evaluator md 的范围圈定步骤改指 `__MCP:workflow_files__`，Input/Inspect 节的 `workflow.json` 清单读取表述同步移除（修改文件表）；占位符名与工具名一致性由 `mcp.test.ts` 名称断言 + md grep 双重保障 |
| AC-6 | `package.json` `version` 提升任务 + `pnpm -C plugins/dev-team run build` 刷新 `claude-plugins/`、`cursor-plugins/`、`cursor-home-image/`，git status 确认 |

---

## 待决问题

- `lib/workflow.ts`（PGE phase 配置）与 `modules/workflow/`（`workflow.json` 操作）命名并存，后续 change 是否重命名澄清（如 `lib/phase-config.ts`）——本变更 MUST NOT 触碰 `lib/workflow.ts`。
- 未来是否出现需向 agent 暴露 `source` 审计映射的运维场景（当前契约禁止；若放开需修订 `workflow-file-inventory` 规格）。
- 其余 `workflow.json` 操作逻辑（eval 写入、`change_create`、`backtrack`）迁入 `modules/workflow` 的切分节奏——本变更明确不一次性搬空。
