# 设计: use-mcp-roots-list

> **变更**: use-mcp-roots-list
> **日期**: 2026-06-18
> **基于**: proposal.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `project-root` 模块（新增） | MCP `roots/list` 解析、`file://` URI 转换、模块级缓存读写、`roots/list_changed` 刷新 | `plugins/dev-team/bin/src/lib/project-root.ts` | `@modelcontextprotocol/sdk` `Server`、`node:url` `fileURLToPath` | TypeScript |
| `getProjectDir` | 对外统一项目根解析入口：MCP 缓存优先，否则 env/cwd 回退链 | `plugins/dev-team/bin/src/utils/constant.ts` | `lib/project-root.ts` | TypeScript |
| `getChangeDir` | 拼接 `openspec/changes/<name>/` 路径，间接受益于 MCP 根解析 | `plugins/dev-team/bin/src/lib/change.ts` | `getProjectDir`（via `utils`） | TypeScript / `path` |
| `resolveProjectRoot` | MCP tool handler 层：显式 `project_root` 优先，否则 `getProjectDir()` | `plugins/dev-team/bin/src/mcp.ts` | `getProjectDir` | TypeScript |
| MCP 服务器生命周期 | `connect` 后初始化项目根；可选注册 `roots/list_changed` 通知 | `plugins/dev-team/bin/src/mcp.ts` | `initProjectRootFromMcp`、`McpServer.server` | MCP SDK / stdio transport |
| 受影响 command 层 | 无 `project_root` 参数的工具通过 `getChangeDir()` 解析路径；有参数的工具通过 `resolveProjectRoot` | `phase-log.ts`、`phase-next.ts`、`change-list.ts` 等 | `getProjectDir` / `getChangeDir` | TypeScript（只读依赖，不修改） |

### 组件图

```
Cursor MCP 客户端
  |  初始化握手：声明 roots capability
  |  响应 roots/list 请求（file:// 工作区 URI）
  v
mcp.ts — main()
  |
  |-- registerTool(...) × N          （注册不变）
  |-- await server.connect(transport) （stdio 连接）
  |-- await initProjectRootFromMcp(server.server)
  |       |
  |       |-- getClientCapabilities()?.roots 存在?
  |       |       |-- 否 → 跳过 listRoots，缓存保持 null
  |       |       └── 是 → listRoots() → 取 roots[0].uri
  |       |               → fileUriToPath(uri) → 写入模块级缓存
  |       |
  |       └── roots.listChanged === true?
  |               └── 注册 notifications/roots/list_changed → refreshProjectRootFromMcp()
  |
  v
tool handler 调用
  |
  |-- 有 project_root 参数 → resolveProjectRoot(args.project_root) → 显式值
  |-- 无 project_root 参数 → getProjectDir() / getChangeDir()
  |                               |
  |                               |-- MCP 缓存已设置 → 返回缓存路径
  |                               └── 否则 → CLAUDE_PROJECT_DIR || CURSOR_PROJECT_DIR || cwd
  v
openspec/changes/...  读写（eval.json、change_list 扫描等）
```

---

## 数据流

### 流程描述

1. **MCP 启动与初始化**：`main()` 创建 `McpServer`、注册全部 tool handler（与现有一致），随后 `await server.connect(transport)` 完成 stdio 传输与客户端初始化握手。

2. **项目根解析（post-connect）**：`connect` 返回后立即 `await initProjectRootFromMcp(server.server)`：
   - 读取 `server.getClientCapabilities()?.roots`；
   - 若客户端未声明 `roots` capability，跳过 `listRoots()`，不设置缓存；
   - 若声明了 `roots`，调用 `server.listRoots()`；
   - 响应 `{ roots: [...] }` 非空时，取 **第一个** root 的 `uri`，经 `fileUriToPath()` 转为平台本地绝对路径，写入模块级变量 `mcpProjectRootCache`；
   - 若 `roots.listChanged === true`，注册 `notifications/roots/list_changed` handler，收到通知时调用 `refreshProjectRootFromMcp()` 重新 `listRoots()` 并更新缓存。

3. **同步路径查询**：任意 command 或 tool handler 调用 `getProjectDir()` 时：
   - 若 `mcpProjectRootCache !== null`，返回缓存值（同步、无 I/O）；
   - 否则按优先级回退：`process.env.CLAUDE_PROJECT_DIR` → `process.env.CURSOR_PROJECT_DIR` → `process.cwd()`。

4. **下游工具路径解析**：
   - `phase_log`、`phase_next`：无 `project_root` 参数 → `getChangeDir(change)` → `path.resolve(getProjectDir(), 'openspec', 'changes', change)`；
   - `change_list` 等：可选 `project_root` → `resolveProjectRoot(args.project_root)` → 显式传入时直接使用，不读 MCP 缓存。

5. **缓存刷新与失败处理**：
   - 初始化时 `listRoots()` 失败或返回空数组：记录 stderr 日志（可选），缓存保持 `null`，后续走回退链；MCP 服务器继续正常运行；
   - `roots/list_changed` 后刷新失败或返回空：若已有有效缓存则 **保留旧值**；若无缓存则继续走回退链。

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `mcpProjectRootCache` | `string \| null`（模块级） | 由 `initProjectRootFromMcp` / `refreshProjectRootFromMcp` 写入；`getProjectDir` 读取 | 进程内存，MCP 子进程生命周期内有效 |
| MCP Root 条目 | `uri: string`（`file://...`）、`name?: string` | 客户端通过 `roots/list` 返回；服务器仅使用 `roots[0].uri` | 无（协议 transient） |
| `getProjectDir()` 返回值 | 绝对路径字符串 | 所有 `openspec/` 相对路径的基准 | 无 |
| `getChangeDir(name)` 返回值 | `<projectRoot>/openspec/changes/<name>/` | 依赖 `getProjectDir()` | 无 |

#### 项目根解析优先级

| 优先级 | 来源 | 条件 |
|--------|------|------|
| 1（最高） | 工具显式 `project_root` 参数 | handler 调用 `resolveProjectRoot(args.project_root)` 且值非空 |
| 2 | MCP roots 缓存 | `initProjectRootFromMcp` 成功解析首个 root URI |
| 3 | `CLAUDE_PROJECT_DIR` | 环境变量已设置且非空 |
| 4 | `CURSOR_PROJECT_DIR` | 环境变量已设置且非空 |
| 5（最低） | `process.cwd()` | 以上均不可用 |

---

## 路由/API 设计

本变更不涉及 HTTP API。以下为 MCP 协议交互与内部模块契约。

### MCP 协议交互

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 请求 | `roots/list` | 服务器向客户端请求工作区根目录列表 | `{}` | `{ roots: [{ uri, name? }] }` | stdio MCP 会话 |
| MCP 通知 | `notifications/roots/list_changed` | 客户端工作区根变更时推送（仅当 `roots.listChanged: true`） | 无 payload | 无 | stdio MCP 会话 |

### MCP 工具契约（不变）

所有现有 MCP 工具的 input/output Zod schema **不修改**。以下仅说明路径解析行为：

| Tool | `project_root` 参数 | 解析路径 |
|------|---------------------|----------|
| `phase_log` | 无 | `getChangeDir()` → `getProjectDir()` |
| `phase_next` | 无 | `getChangeDir()` → `getProjectDir()` |
| `change_list` | 可选 | `resolveProjectRoot(args.project_root)` |
| `config_get/set/unset/context` | 可选 | `resolveProjectRoot(args.project_root)` |
| `archi_query/validate/write/check` | 可选 | `resolveProjectRoot(args.project_root)` |
| `test_detect_frameworks/resolve_paths` | 可选 | `resolveProjectRoot(args.project_root)` |

### 新增/修改内部模块契约

| 函数 | 模块 | 签名 | 行为 |
|------|------|------|------|
| `initProjectRootFromMcp` | `lib/project-root.ts` | `(server: Server) => Promise<void>` | 检查 roots capability → `listRoots()` → 缓存首个 root；注册 `list_changed` handler |
| `refreshProjectRootFromMcp` | `lib/project-root.ts` | `(server: Server) => Promise<void>` | 重新 `listRoots()` 并更新缓存；失败时保留旧缓存 |
| `fileUriToPath` | `lib/project-root.ts` | `(uri: string) => string` | `file://` URI → 平台本地绝对路径（export，供单测） |
| `getMcpCachedProjectRoot` | `lib/project-root.ts` | `() => string \| null` | 读取模块级缓存（供 `constant.ts` 与单测） |
| `resetMcpProjectRootCacheForTests` | `lib/project-root.ts` | `() => void` | 清缓存（仅测试用，export） |
| `getProjectDir` | `utils/constant.ts` | `() => string` | MCP 缓存 ?? env/cwd 回退链 |
| `getChangeDir` | `lib/change.ts` | `(changeName: string) => string` | 不变；自动受益于新 `getProjectDir()` |
| `resolveProjectRoot` | `mcp.ts` | `(cwd?: string \| null) => string` | 不变：`cwd \|\| getProjectDir()` |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 新增独立模块 `lib/project-root.ts` 承载 MCP roots 逻辑，`constant.ts` 仅做薄封装 | 职责分离：MCP 协议细节与通用 `getProjectDir` 入口解耦；便于 mock 单测 | **全部写入 `constant.ts`**：混合 MCP SDK 依赖与 utils 层，测试与维护成本高，已拒绝 |
| D2 | 在 `await server.connect(transport)` 之后立即 `await initProjectRootFromMcp()` | 确保首个 tool 请求到达时缓存已就绪；connect 完成即表示客户端 capability 可用 | **tool handler 内 lazy-init**：每次调用需 async/await，改动所有 handler 签名，已拒绝 |
| D3 | 多 root 时取列表第一个 root | 与 Cursor 单工作区场景一致；proposal 明确首版限制 | **按 `name` 或 URI 匹配**：需额外启发式，超出当前 scope，已拒绝 |
| D4 | 使用 Node.js 内置 `fileURLToPath`（`node:url`）做 URI 转换 | 标准库处理 Windows/Unix `file://` 差异，减少自研解析 bug | **手写正则解析**：Windows 盘符与编码 edge case 多，已拒绝 |
| D5 | MCP 缓存优先级高于 env 变量，但低于显式 `project_root` 工具参数 | MCP roots 是 Cursor 场景的正确工作区来源；env 仍服务 CLI/测试；显式参数保持 override 语义 | **env 优先于 MCP 缓存**：Cursor 未注入 env 时仍无法修复 bug，已拒绝 |
| D6 | `roots/list_changed` 刷新失败时保留已有缓存 | 避免工作区短暂不可达导致全部 tool 路径突变 | **刷新失败清空缓存**：可能回退到错误 cwd，已拒绝 |
| D7 | 不修改任何 tool schema 与 handler 签名 | 对外契约稳定；路径修复对调用方透明 | **为 `phase_log`/`phase_next` 新增 `project_root` 参数**：破坏现有 skill 调用约定，已拒绝 |
| D8 | 导出 `resetMcpProjectRootCacheForTests()` 供 vitest 隔离用例 | 模块级缓存在测试间需重置，避免顺序依赖 | **不 export、依赖进程隔离**：vitest 同进程运行会污染，已拒绝 |
| D9 | tool 注册顺序保持「先 registerTool、后 connect、再 init」 | 与现有 `mcp.ts` 结构一致，最小 diff | **connect 后再 registerTool**：SDK 可能要求不同顺序，无必要改动，已拒绝 |

---

## 依赖

### 运行时依赖

- `@modelcontextprotocol/sdk`（`^1.29.0`）— `Server.listRoots()`、`getClientCapabilities()`、notification handler 注册
- `node:url` — `fileURLToPath` 用于 `file://` → 本地路径
- `node:path` — `path.resolve` 规范化缓存路径（可选，在 `fileUriToPath` 输出后调用）

### 构建/测试依赖

- `plugins/dev-team/bin` — TypeScript 编译与 vitest（`vp test`）
- `vite-plus` — 打包 `dev-team-mcp.cjs`（`vp pack` / `pnpm build`）

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Cursor 客户端未声明 roots capability | 高 — 仍回退到错误 cwd | 低 | 保留 env/cwd 回退链；后续可通过 `.mcp.json` 注入 `CURSOR_PROJECT_DIR` 补充 |
| Windows `file://` URI 解析错误 | 中 — 路径指向错误目录 | 中 | 使用 `fileURLToPath`；单测覆盖 `file:///D:/Projects/...` |
| 多 root 工作区选错根目录 | 中 — 操作非预期工作区 | 低 | 首版取第一个 root；文档说明限制，后续变更可扩展匹配策略 |
| `init` 与首个 tool 请求竞态 | 中 — 短暂使用回退路径 | 低 | `connect` 后同步 `await init` 再进入事件循环处理请求 |
| MCP 规范 roots 未来 deprecated | 低 — 需迁移机制 | 低 | 保留 env/cwd 回退；roots 不可用时可改用配置注入 |
| 单测 mock 与 SDK 内部 API 变更 | 低 — 测试失败 | 低 | mock 最小 `Server` 接口（`listRoots`、`getClientCapabilities`、`setNotificationHandler`） |

---

## 迁移步骤

1. 新增 `plugins/dev-team/bin/src/lib/project-root.ts`，实现缓存、`fileUriToPath`、`initProjectRootFromMcp`、`refreshProjectRootFromMcp`。
2. 修改 `plugins/dev-team/bin/src/utils/constant.ts`：`getProjectDir()` 优先返回 MCP 缓存。
3. 修改 `plugins/dev-team/bin/src/mcp.ts`：在 `connect` 后调用 `await initProjectRootFromMcp(server.server)`。
4. 新增 `plugins/dev-team/bin/src/lib/project-root.test.ts`，覆盖 proposal AC-1、AC-4~AC-6 及 URI 转换场景。
5. 在 `plugins/dev-team/bin` 运行 `pnpm test`，确认全部通过（AC-8）。
6. 将 `plugins/dev-team/.claude-plugin/plugin.json` 版本从 `2.6.22` bump 至 `2.6.23`（AC-9）。
7. 运行 `pnpm build` 重新打包 `dev-team-mcp.cjs`。
8. 在 Cursor MCP 环境中手动验证 AC-2、AC-3、AC-7。

实施顺序与 `tasks.md` 阶段 1→6 一致。

---

## 待决问题

- 无。proposal 与 delta spec 已明确解析优先级、回退行为、多 root 策略及不修改范围。
