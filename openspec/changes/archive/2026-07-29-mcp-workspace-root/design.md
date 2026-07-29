# 设计: mcp-workspace-root

> **变更**: mcp-workspace-root
> **日期**: 2026-07-29

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `project-root` 模块 | connect 合并采集候选；路径合法性 / ∈ 候选 / pending-force；导出 `collectProjectRootCandidates` / `resolveProjectRootForTool` / `getProjectDir`；删除 lock API | `plugins/dev-team/bin/src/lib/project-root.ts` | `@modelcontextprotocol/sdk`（`McpServerLike`）、`node:fs`、`node:path`、`node:url` | TypeScript |
| MCP 服务器生命周期 | connect 后只 collect；handler 用必填 `args.project_root` 走 resolve；调用期内注入 call-scoped 根供 `getChangeDir`；捕获 resolve 错误 | `plugins/dev-team/bin/src/mcp.ts` | `lib/project-root`、commands、schemas | MCP SDK / stdio |
| MCP input schemas | 全部触达项目树的 tool 增加必填 `project_root: z.string()` | `plugins/dev-team/bin/src/schemas/*.schema.ts`（见变更清单） | zod/v4 | TypeScript |
| command / change 路径辅助 | CLI/单测继续 `options.projectRoot`；phase 类工具经 `getChangeDir`→`getProjectDir` 读取 call-scoped 根；**不改** command 公共签名 | `commands/*`、`lib/change.ts` | `getProjectDir` | TypeScript |
| 共享 MCP 配置 | 保持无 `${workspaceFolder}`、无未展开 `CLAUDE_PROJECT_DIR` 回灌到 `dev-team` 服务 | `plugins/dev-team/.mcp.json` | 无 | JSON |
| 插件版本与双端产物 | 升版并重建 `claude-plugins` / `cursor-plugins` | `plugins/dev-team/package.json`、`node scripts/build-plugins.mjs` | 构建脚本 | JSON / Node |

### 组件图

```
宿主 (Claude Code / Cursor)
  |  CLAUDE_PROJECT_DIR ∪ WORKSPACE_FOLDER_PATHS ∪ roots/list
  v
mcp.ts — connectToServer()
  |
  |-- registerTool(...) × N（input schema 均必填 project_root）
  |-- await server.connect(transport)
  |-- await collectProjectRootCandidates(server.server)
  |       └── candidates = 合并去重；不锁定默认根；禁用 cwd
  v
tool handler(args)
  |
  |-- resolveProjectRootForTool(toolName, args)
  |       1. 合法？否 → invalid_path（不改 pending）
  |       2. ∈ candidates？是 → 清 pending，返回 P
  |       3. pendingKey == f(tool, args)？是 → candidates.add(P)，清 pending，返回 P
  |       4. 否则写 pending，抛 not_in_candidates（含 candidates + force_hint）
  |-- runWithCallScopedRoot(P, () => 业务)
  |       └── getProjectDir() 优先返回 P（供 getChangeDir / phase_*）
  v
openspec/ / models/ 读写
```

### Connect 候选采集（无默认根）

| 通道 | 采纳规则 | 失败时 |
|------|----------|--------|
| `CLAUDE_PROJECT_DIR` | 可用（绝对 + exists + 非 `${...}`）则加入集合 | 跳过该值，继续 |
| `WORKSPACE_FOLDER_PATHS` | 按 `,`/`;` 分割后**每一个**可用路径加入 | 无可用段则跳过；**不**因多路径失败 |
| MCP `roots/list` | 客户端有 roots capability 时调用；**每一个**可用 file/裸绝对根加入；超时 2000ms | 失败 / `-32601` / 超时 / 空：stderr 记录，**不**阻断其它通道已采结果 |
| `process.cwd()` | **禁止**作为候选 | — |

`len == 1` 与 `len > 1` 行为相同：只填充 `candidates`，不自动锁定、不省略 `project_root`。

### Resolve 校验顺序（每次 tool 调用）

| 步骤 | 条件 | 结果 |
|------|------|------|
| 1 | `project_root` 非法（相对 / 不存在 / 含 `${...}`） | `invalid_path`；**不**更新 pending |
| 2 | normalize 后 ∈ candidates | 清 pending；放行 |
| 3 | pending 键 == `f(toolName, 完整 arguments)` | `candidates.add(P)`；清 pending；放行（force，含空候选逃生） |
| 4 | 否则 | pending = 该键（仅保留最近一次）；`not_in_candidates` |

无独立 `force` 字段。成功执行或 force 成功后清除 pending。

### 路径相等（normalize）规则

用于 ∈ 候选比较与 pending 键中路径以外字段的稳定性（pending 键对 arguments 做稳定序列化，路径比较另走 normalize）：

1. `path.resolve(p)` 得到绝对路径（折叠 `.` / `..`）
2. 去掉尾部路径分隔符（保留 Windows 盘符根如 `C:\`、Unix `/`）
3. **不**调用 `realpath` / 不跟随符号链接（保证宿主工作区原样，AC-9）
4. 比较键：Unix 区分大小写；Windows（`process.platform === 'win32'`）对整串大小写不敏感（`localeCompare` 的 `sensitivity: 'accent'` 或等价 `toLowerCase`）
5. 候选集合内存中保留**首次纳入时的展示路径**（resolve + 去尾斜杠后的形式）；比较一律用比较键

---

## 变更清单

<!-- 无新增业务源文件；测试文件调整属测试阶段，本设计不列 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/project-root.ts` | 删除不可变 lock / `requireLockedProjectRoot` / `ProjectRootLockError*`；改为 `candidates` + `pending`；实现 `collectProjectRootCandidates`、`resolveProjectRootForTool`、`runWithCallScopedRoot`（或等价）、`getProjectRootCandidates`；`WORKSPACE`/`roots` 全部可用路径入集；roots 保留 2000ms 超时；`getProjectDir` 顺序见公共 API；MCP 路径禁止 cwd | AC-1、AC-3~AC-9、AC-11 |
| `plugins/dev-team/bin/src/mcp.ts` | post-connect 改为 collect；`withLockedProjectRoot` 替换为 `withResolvedProjectRoot(toolName, args, run)`；各 handler 传入完整 `args`；resolve 错误映射为结构化 `isError`；启动日志打印 candidates 而非「已锁定」；不注册 `list_changed` | AC-1~AC-7、AC-10 |
| `plugins/dev-team/bin/src/schemas/change-list.schema.ts` | input 增加必填 `project_root`；output 保留只读 `project_root` | AC-2 |
| `plugins/dev-team/bin/src/schemas/config-get.schema.ts` | input 增加必填 `project_root` | AC-2；config-get |
| `plugins/dev-team/bin/src/schemas/archi-query.schema.ts` | input 增加必填 `project_root` | AC-2 |
| `plugins/dev-team/bin/src/schemas/archi-validate.schema.ts` | input 增加必填 `project_root` | AC-2 |
| `plugins/dev-team/bin/src/schemas/archi-write.schema.ts` | input 增加必填 `project_root` | AC-2 |
| `plugins/dev-team/bin/src/schemas/archi-check.schema.ts` | input 增加必填 `project_root` | AC-2 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | input 增加必填 `project_root` | AC-2 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` | input 增加必填 `project_root` | AC-2；test-path-resolver |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | input 增加必填 `project_root` | AC-2 |
| `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` | input 增加必填 `project_root` | AC-2 |
| `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` | input 增加必填 `project_root` | AC-2 |
| `plugins/dev-team/.mcp.json` | 核对并保持：`dev-team` 服务无 `${workspaceFolder}`、无向其 `env` 回灌未展开 `CLAUDE_PROJECT_DIR` | AC-10 |
| `plugins/dev-team/package.json` | `version` 递增（当前 `2.10.4` → `2.10.5`） | 项目规则 |
| 双端产物（`claude-plugins/dev-team/`、`cursor-plugins/dev-team/` 等） | `node scripts/build-plugins.mjs` 重建 | 发布产物与源码一致 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `collectProjectRootCandidates` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增（替代原 init 锁定语义） | `function collectProjectRootCandidates(server: McpServerLike): Promise<void>` | connect 后合并去重填充 candidates；不锁定默认根；不 exit；不注册 `list_changed`；禁用 cwd |
| `initProjectRootFromMcp` | `plugins/dev-team/bin/src/lib/project-root.ts` | 修改 / 薄别名 | `function initProjectRootFromMcp(server: McpServerLike): Promise<void>` | 实现上直接委托 `collectProjectRootCandidates`，降低调用点改名成本；语义为采集而非锁定 |
| `resolveProjectRootForTool` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `function resolveProjectRootForTool(toolName: string, args: Record<string, unknown>): string` | 从 `args.project_root` 按合法 → ∈ → force 顺序解析；放行返回绝对路径；否则抛 `ProjectRootResolveError`；**禁止** cwd |
| `runWithCallScopedRoot` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `function runWithCallScopedRoot<T>(projectRoot: string, fn: () => T): T` | 在 `fn` 执行期间让 `getProjectDir()` 优先返回该根；`finally` 清除；供 phase_* → `getChangeDir` |
| `getProjectRootCandidates` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `function getProjectRootCandidates(): readonly string[]` | 只读候选快照（诊断 / 启动日志 / 错误载荷） |
| `getProjectDir` | `plugins/dev-team/bin/src/lib/project-root.ts` | 修改 | `function getProjectDir(): string` | CLI/command：call-scoped 根 → 可用 `CLAUDE_PROJECT_DIR` → 恰好 1 个可用 `WORKSPACE_FOLDER_PATHS` → `process.cwd()`。MCP handler **不得**依赖末位 cwd，必须先 `resolveProjectRootForTool` |
| `isProjectRootResolveError` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `function isProjectRootResolveError(err: unknown): err is ProjectRootResolveError` | 鸭类型守卫（跨模块副本） |
| `requireLockedProjectRoot` | `plugins/dev-team/bin/src/lib/project-root.ts` | **删除** | — | 由 `resolveProjectRootForTool` 替代 |
| `getMcpCachedProjectRoot` | `plugins/dev-team/bin/src/lib/project-root.ts` | **删除** | — | 由 `getProjectRootCandidates` / resolve 结果替代 |
| `connectToServer` | `plugins/dev-team/bin/src/mcp.ts` | 修改 | `function connectToServer(transport: Transport): Promise<McpServer>` | post-connect collect；handlers 走 resolve |

「可用」路径规则（采集与合法性校验共用）：绝对、`fs.existsSync`、拒绝匹配 `/\$\{[^}]+\}/`。

pending 键：`toolName + "\n" + stableStringify(args)`，其中 `stableStringify` 对对象键递归排序后 `JSON.stringify` 等价序列化；仅保留最近一次 pending。

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `McpServerLike` | `plugins/dev-team/bin/src/lib/project-root.ts` | 保留 | `Pick<McpServer['server'], 'getClientCapabilities' \| 'listRoots'>` |
| `ProjectRootResolveError` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `class ProjectRootResolveError extends Error`；字段：`code`、`candidates?: string[]`、`force_hint?: string`、`project_root?: string` |
| `ProjectRootResolveErrorCode` | `plugins/dev-team/bin/src/lib/project-root.ts` | 新增 | `'invalid_path' \| 'not_in_candidates'` |
| `ProjectRootLockError` / `ProjectRootLockErrorCode` / `isProjectRootLockError` | `plugins/dev-team/bin/src/lib/project-root.ts` | **删除** | 锁模型废弃 |

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `version` | `plugins/dev-team/package.json` | 修改 | `string` | `2.10.4` → `2.10.5` | 改插件源码后升版 |
| `mcpServers.dev-team` | `plugins/dev-team/.mcp.json` | 保持 | object | 见现文件 | 仅 `command`/`args`；**不**写 `${workspaceFolder}`；**不**回灌未展开 `CLAUDE_PROJECT_DIR` |
| `CLAUDE_PROJECT_DIR` | 进程环境（宿主注入） | 既有（采集语义） | `string` | 无 | 可用则入候选；拒绝 `${...}` / 相对 / 不存在 |
| `WORKSPACE_FOLDER_PATHS` | 进程环境（宿主注入） | 既有（采集语义放宽） | `string` | 无 | `,`/`;` 分割后全部可用路径入候选 |
| `ROOTS_LIST_TIMEOUT_MS` | `plugins/dev-team/bin/src/lib/project-root.ts`（模块常量） | 保留/明确 | `number` | `2000` | `listRoots` 超时；超时不阻断其它通道 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| 候选集合 | `candidates: Set<string>`（比较键去重；值存 normalize 后展示路径） | connect 时由三通道填充；force 时可追加 | 进程内存；重启清空 |
| pending | `pendingKey: string \| null`（`f(toolName, args)`） | 最近一次 ∉ 候选的合法调用；成功/force 清除；非法路径不改写 | 进程内存 |
| call-scoped 根 | `callScopedProjectRoot: string \| null` | `runWithCallScopedRoot` 设置；`getProjectDir` 优先读取 | 进程内存（单次调用栈） |
| MCP Root 条目 | `uri: string` | `listRoots()` → 转本地路径后入候选 | 无（协议 transient） |
| resolve 错误载荷 | `code`、`message`、`candidates?`、`force_hint?`、`project_root?` | handler 序列化为 MCP `isError` text（JSON） | 无 |
| `change_list` 输出 | `project_root: string` | 等于本次 resolve 生效根的只读回显 | 无 |
| CLI fixture | `options.projectRoot` / `project_root` | command 层可选覆盖；**不属于**「可省略」的 MCP 契约 | 调用方传入 |

### 错误载荷形状（MCP `isError`）

`content[0].text` 为 JSON 字符串（agent 可读；与旧纯 message 相比更结构化）：

```ts
// invalid_path
{
  code: 'invalid_path',
  message: string,          // 人类可读原因
  project_root: string      // 原始入参
}

// not_in_candidates（含空候选第一次调用）
{
  code: 'not_in_candidates',
  message: string,
  project_root: string,
  candidates: string[],     // 当前候选快照（可空）
  force_hint: string        // 固定说明：再次提交相同完整 arguments 将 force-add 并放行
}
```

`force_hint` 推荐文案（实现可微调措辞，语义不变）：

> Resubmit the same tool call with identical complete arguments to force-add this `project_root` into candidates and proceed.

---

## 路由/API 设计

本变更不涉及 HTTP API。以下为 MCP 工具契约变更（非 HTTP）。

| Tool | input `project_root` | 项目根来源 | 失败行为 |
|------|----------------------|------------|----------|
| `phase_log` | **必填** | resolve → call-scoped → `runPhaseLog` → `getChangeDir` | 结构化错误；不写 `eval.json` 到 cwd |
| `phase_next` | **必填** | 同上 | 同上 |
| `backtrack` | **必填** | 同上 | 同上 |
| `change_list` | **必填** | resolve 根传入 `runChangeList`；output 回显 | 结构化错误 |
| `config_get` | **必填** | resolve 根传入 `runConfigGet({ projectRoot })` | 结构化错误 |
| `archi_query` / `archi_validate` / `archi_write` / `archi_check` | **必填** | resolve 根传入对应 lib API | 结构化错误 |
| `test_detect_frameworks` / `test_resolve_paths` | **必填** | resolve 根传入 command | 结构化错误 |

Zod 字段统一形态：

```ts
project_root: z
  .string()
  .describe('Absolute host workspace folder for this call (must match a collected candidate, or be force-confirmed by resubmitting identical arguments)')
```

handler 包装（替换现有 `withLockedProjectRoot`）：

```ts
async function withResolvedProjectRoot<T>(
  toolName: string,
  args: Record<string, unknown>,
  run: (projectRoot: string) => T | Promise<T>,
): Promise<T | { isError: true; content: { type: 'text'; text: string }[] }> {
  try {
    const projectRoot = resolveProjectRootForTool(toolName, args);
    return await runWithCallScopedRoot(projectRoot, () => run(projectRoot));
  } catch (err) {
    if (!isProjectRootResolveError(err)) throw err;
    return {
      isError: true,
      content: [{ type: 'text', text: JSON.stringify({
        code: err.code,
        message: err.message,
        project_root: err.project_root,
        candidates: err.candidates,
        force_hint: err.force_hint,
      }) }],
    };
  }
}
```

协议侧：`roots/list` 仅作候选来源；**不**注册 `notifications/roots/list_changed`；**不**实现真正的热刷新。

CLI / command 层契约不变：显式 `projectRoot` / `project_root` fixture 保留（AC-11）。

---

## 依赖

### 运行时依赖

- `@modelcontextprotocol/sdk` — `McpServer` / `listRoots` / `getClientCapabilities`（已有）
- Node.js 内置 `fs` / `path` / `url` — 存在性检查、路径规范化、`fileURLToPath`
- `zod` /v4 — MCP input schema（已有）

### 构建/测试依赖

- `vite-plus`（`vp pack`）与 `node scripts/build-plugins.mjs` — 重建 MCP bundle 与双端产物
- 无新增 npm 包

---

## 待决问题

- 无（proposal 开放项已在本设计拍板：normalize 规则、错误 JSON 载荷、roots 2000ms 超时、`getProjectDir` CLI 回退顺序）
