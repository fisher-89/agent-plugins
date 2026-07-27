# 设计: mcp-project-root-lock

> **变更**: mcp-project-root-lock
> **日期**: 2026-07-24

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `project-root` 模块 | 启动锁定 resolve/cache；绝对路径与 `${...}` 校验；`WORKSPACE_FOLDER_PATHS` 分隔；roots 恰好 1 个才采纳；导出 `initProjectRootFromMcp` / `requireLockedProjectRoot` / `getProjectDir` / `getMcpCachedProjectRoot` | `plugins/dev-team/bin/src/lib/project-root.ts` | `@modelcontextprotocol/sdk`（`McpServerLike`）、`node:fs`、`node:path`、`node:url` | TypeScript |
| MCP 服务器生命周期 | `connect` 后一次性 init；全部 tool handler 入口强制锁定校验；删除 `args.project_root` 与 `list_changed` | `plugins/dev-team/bin/src/mcp.ts` | `lib/project-root`、commands、schemas | MCP SDK / stdio |
| MCP input schemas | 删除可选 `project_root` 字段，禁止模型 override | `plugins/dev-team/bin/src/schemas/*.schema.ts`（见变更清单） | zod/v4 | TypeScript |
| utils barrel | 删除 `constant.ts` 后继续 re-export `getProjectDir`，避免 command 层大面积改 import | `plugins/dev-team/bin/src/utils/index.ts` | `lib/project-root` | TypeScript |
| command / change 路径辅助 | CLI/单测仍可通过 `options.projectRoot` / `project_root` 注入；默认走 `getProjectDir()`；本变更不改 command 公共签名 | `commands/*`、`lib/change.ts` | `getProjectDir` | TypeScript |
| 插件清单 | 改代码后升级版本 | `plugins/dev-team/.claude-plugin/plugin.json` | 无 | JSON |
| MCP bundle | 重建可执行产物 | `plugins/dev-team/bin/dev-team-mcp.cjs` | `pnpm build`（`vp pack`） | CJS bundle |

### 组件图

```
宿主 (Claude Code / Cursor)
  |  CLAUDE_PROJECT_DIR（绝对路径）和/或 WORKSPACE_FOLDER_PATHS
  |  可选 roots capability + listRoots()
  v
mcp.ts — connectToServer()
  |
  |-- registerTool(...) × N
  |-- await server.connect(transport)
  |-- await initProjectRootFromMcp(server.server)
  |       |
  |       |-- 已锁定? → 立即返回（不可变）
  |       |-- 1. CLAUDE_PROJECT_DIR 可用绝对路径 → 锁定
  |       |-- 2. roots/list 恰好 1 个可用 file 根 → 锁定
  |       |-- 3. WORKSPACE_FOLDER_PATHS 恰好 1 个绝对路径 → 锁定
  |       └── 否则缓存保持 null，记录失败原因（不 exit）
  v
tool handler
  |
  |-- requireLockedProjectRoot()  // 未锁定 → 结构化错误，不写 cwd
  |-- 有参工具：将锁定根传入 command / archi_*（不再读 args.project_root）
  |-- 无参工具：校验通过后走既有 getChangeDir()→getProjectDir()（此时缓存必已锁定）
  v
openspec/ / models/ 读写
```

### 启动锁定优先级（进程内一次）

| 优先级 | 通道 | 成功条件 | 失败时 |
|--------|------|----------|--------|
| 1 | `CLAUDE_PROJECT_DIR` | 非空、绝对路径、磁盘存在、不含未展开 `${...}` | 记原因，继续下一通道 |
| 2 | MCP `roots/list` | 客户端声明 `roots`；`listRoots()` 成功；**恰好 1** 个可用 `file://` 或绝对路径根 | 0 / >1 / 抛错 / `-32601` → 不锁定该通道，继续 |
| 3 | `WORKSPACE_FOLDER_PATHS` | 按 `,` 或 `;` 分割并 trim 后 **恰好 1** 个绝对且存在的路径 | 0 / >1 → 不取 `[0]` |
| — | （无） | — | 缓存保持 `null`；**禁止**回退 `process.cwd()` |

---

## 变更清单

<!-- 无新增业务源文件；测试文件调整属测试阶段，本设计不列 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/project-root.ts` | 重写启动锁定：env → roots 恰 1 → WORKSPACE 恰 1；绝对路径/`exists`/`${...}` 校验；`WORKSPACE` 支持 `,`/`;`；删除 `list_changed` 刷新路径与「取 `roots[0]`」；新增 `requireLockedProjectRoot` / `getProjectDir` / `ProjectRootLockError`；二次 init 不改写已锁定缓存；roots 转换接受裸绝对路径 | AC-1~AC-3、AC-10 |
| `plugins/dev-team/bin/src/utils/constant.ts` | **删除**整文件 | 职责迁入 `lib/project-root`（AC-10） |
| `plugins/dev-team/bin/src/utils/index.ts` | 改为 `export { getProjectDir } from '../lib/project-root'`（或等价） | 保留 barrel，避免 command 层大改 import |
| `plugins/dev-team/bin/src/mcp.ts` | 删除 `resolveProjectRoot`；各 tool handler 入口调用 `requireLockedProjectRoot()`；有参工具用锁定根调用 command；无参工具（`phase_log` / `phase_next` / `backtrack`）亦先校验；捕获 `ProjectRootLockError` 返回可观测 MCP 错误；启动日志用锁定根或明确「未锁定」文案；不注册 `list_changed` | AC-4、AC-5、AC-6 |
| `plugins/dev-team/bin/src/schemas/change-list.schema.ts` | input 删除 `project_root`；output 保留只读 `project_root` | AC-4、AC-6 |
| `plugins/dev-team/bin/src/schemas/config-get.schema.ts` | input 删除 `project_root` | AC-4；config-get 能力 |
| `plugins/dev-team/bin/src/schemas/archi-query.schema.ts` | input 删除 `project_root` | AC-4 |
| `plugins/dev-team/bin/src/schemas/archi-validate.schema.ts` | input 删除 `project_root` | AC-4 |
| `plugins/dev-team/bin/src/schemas/archi-write.schema.ts` | input 删除 `project_root` | AC-4 |
| `plugins/dev-team/bin/src/schemas/archi-check.schema.ts` | input 删除 `project_root` | AC-4 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | input 删除 `project_root` | AC-4 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` | input 删除 `project_root` | AC-4；test-path-resolver 能力 |
| `plugins/dev-team/.claude-plugin/plugin.json` | `version` 递增（当前 `2.10.2` → `2.10.3`） | AC-11 / 项目规则 |
| `plugins/dev-team/bin/dev-team-mcp.cjs` | `pnpm build` 重建 | 发布产物与源码一致 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `initProjectRootFromMcp` | `plugins/dev-team/bin/src/lib/project-root.ts` | 修改 | `function initProjectRootFromMcp(server: McpServerLike): Promise<void>` | connect 后尽力锁定；失败不 exit；已锁定则 no-op；不注册 `list_changed` |
| `requireLockedProjectRoot` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `function requireLockedProjectRoot(): string` | 返回已锁定绝对路径；未锁定抛 `ProjectRootLockError`（含失败原因） |
| `getProjectDir` | `plugins/dev-team/bin/src/lib/project-root.ts` | 修改（迁入） | `function getProjectDir(): string` | CLI/command 辅助：优先锁定缓存；否则「可用」`CLAUDE_PROJECT_DIR`；否则恰好 1 个「可用」`WORKSPACE_FOLDER_PATHS`；最后 `process.cwd()`。**MCP handler 不得依赖末位 cwd**，必须先 `requireLockedProjectRoot()` |
| `getMcpCachedProjectRoot` | `plugins/dev-team/bin/src/lib/project-root.ts` | 保留 | `function getMcpCachedProjectRoot(): string \| null` | 只读缓存；供诊断/CLI 辅助 |
| `connectToServer` | `plugins/dev-team/bin/src/mcp.ts` | 修改 | `function connectToServer(transport: Transport): Promise<McpServer>` | post-connect 调用新版 init；tool 注册逻辑见上 |

「可用」路径规则（锁定通道与 `getProjectDir` 的 env 通道共用）：绝对、`fs.existsSync`、拒绝匹配 `/\$\{[^}]+\}/` 的字面量。

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `McpServerLike` | `plugins/dev-team/bin/src/lib/project-root.ts` | 保留 | `Pick<McpServer['server'], 'getClientCapabilities' \| 'listRoots'>` |
| `ProjectRootLockError` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `class ProjectRootLockError extends Error`；含 `code: ProjectRootLockErrorCode` 与可读 `message` |
| `ProjectRootLockErrorCode` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `'not_locked' \| 'multi_root' \| 'literal_env' \| 'invalid_path'`（实现可微调字面量集合，须覆盖未锁定 / 多根 / `${...}`） |

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `version` | `plugins/dev-team/.claude-plugin/plugin.json` | 修改 | `string` | `2.10.2` → `2.10.3` | 改插件代码后升版 |
| `CLAUDE_PROJECT_DIR` | 进程环境（宿主注入） | 既有（语义收紧） | `string` | 无 | 须为已存在绝对路径；拒绝 `${...}` 字面量 |
| `WORKSPACE_FOLDER_PATHS` | 进程环境（宿主注入） | 既有（解析收紧） | `string` | 无 | 按 `,`/`;` 分割；仅恰好 1 个可用路径时锁定 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| MCP 锁定缓存 | `mcpProjectRootCache: string \| null` | 仅由 `initProjectRootFromMcp` 在未锁定时写入；`requireLockedProjectRoot` / `getMcpCachedProjectRoot` / `getProjectDir` 读取 | 进程内存；生命周期内不可变 |
| 锁定失败原因 | `lockFailureReason: string \| null`（或等价） | init 各通道失败时更新；供 `requireLockedProjectRoot` 拼错误信息 | 进程内存 |
| MCP Root 条目 | `uri: string`、`name?: string` | `listRoots()` 响应；仅当可用根数量 === 1 时采纳 | 无（协议 transient） |
| `change_list` 输出 | `project_root: string` | 等于锁定根的只读回显 | 无 |
| CLI fixture | `options.projectRoot` / `project_root` | command 层可选覆盖；**不属于** MCP input schema | 调用方传入 |

路径可用性规则（锁定通道共用）：

- 绝对路径：`path.isAbsolute(p)`
- 存在：`fs.existsSync(p)`（目录或路径存在即可；与现有「工作区根」语义一致）
- 拒绝字面量：字符串匹配 `/\$\{[^}]+\}/`（或等价）则该通道不可用
- roots URI：优先 `file://` → 本地路径；若已是绝对本地路径亦可采纳（经同一可用性校验）

---

## 路由/API 设计

本变更不涉及 HTTP API。以下为 MCP 工具契约变更（非 HTTP）。

| Tool | input `project_root` | 项目根来源 | 未锁定行为 |
|------|----------------------|------------|------------|
| `phase_log` | 无 | 入口 `requireLockedProjectRoot()`；其后 `runPhaseLog` → `getChangeDir` → `getProjectDir`（缓存） | 结构化错误；不写 `eval.json` 到 cwd |
| `phase_next` | 无 | 同上 | 同上 |
| `backtrack` | 无 | 同上 | 同上 |
| `change_list` | **删除** | 锁定根传入 `runChangeList({ project_root })`；output 可回显 | 结构化错误 |
| `config_get` | **删除** | 锁定根传入 `runConfigGet({ projectRoot })` | 结构化错误 |
| `archi_query` / `archi_validate` / `archi_write` / `archi_check` | **删除** | 锁定根传入对应 lib API | 结构化错误 |
| `test_detect_frameworks` / `test_resolve_paths` | **删除** | 锁定根传入 command | 结构化错误 |

MCP 错误返回形态（与现有 SDK 习惯对齐）：

```ts
// handler 内
try {
  const projectRoot = requireLockedProjectRoot();
  // ...
} catch (err) {
  if (err instanceof ProjectRootLockError) {
    return {
      isError: true,
      content: [{ type: 'text', text: err.message }],
    };
  }
  throw err;
}
```

协议侧：`roots/list` 仍可作为锁定通道之一；**不**注册 `notifications/roots/list_changed`。

CLI / command 层契约不变：`runChangeList({ project_root })`、`runConfigGet({ projectRoot })` 等仍接受显式根（AC-7）；仅 MCP Zod input 删除该字段。

---

## 依赖

### 运行时依赖

- `@modelcontextprotocol/sdk` — `McpServer` / `listRoots` / `getClientCapabilities`（已有）
- Node.js 内置 `fs` / `path` / `url` — 存在性检查、路径规范化、`fileURLToPath`
- `zod` /v4 — MCP input schema（已有）

### 构建/测试依赖

- `vite-plus`（`vp pack` / `vp test`）— 重建 `dev-team-mcp.cjs` 与单测（测试实现属后续阶段）
- 无新增 npm 包

---

## 待决问题

- 无（proposal 开放项已全部拍板；多根严格失败为接受行为）
