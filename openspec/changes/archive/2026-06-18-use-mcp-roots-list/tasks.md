# 任务: use-mcp-roots-list

> **变更**: use-mcp-roots-list
> **日期**: 2026-06-18

---

## 阶段 1: 新增 `project-root` 模块

- [x] 新增 `plugins/dev-team/bin/src/lib/project-root.ts`
- [x] 定义模块级变量 `mcpProjectRootCache: string | null = null`
- [x] 实现并 **export** `fileUriToPath(uri: string): string`：
  - 使用 `fileURLToPath`（`node:url`）将 `file://` URI 转为本地绝对路径
  - 对输出调用 `path.resolve()` 规范化
  - 支持 Unix URI（`file:///home/user/project`）与 Windows URI（`file:///D:/Projects/...`）
- [x] 实现 **export** `getMcpCachedProjectRoot(): string | null` — 返回当前 MCP 缓存
- [x] 实现 **export** `resetMcpProjectRootCacheForTests(): void` — 将缓存重置为 `null`（仅测试用）
- [x] 实现内部函数 `applyRootsList(roots: { uri: string }[]): void`：
  - `roots` 非空时取 `roots[0].uri`，经 `fileUriToPath` 写入缓存
  - `roots` 为空时不修改缓存（初始化场景保持 `null`；刷新场景见 D6）
- [x] 实现 **export** `refreshProjectRootFromMcp(server: Server): Promise<void>`：
  - 调用 `server.listRoots()`
  - 成功且非空 → 更新缓存
  - 失败或空列表 → 若已有缓存则保留；若无缓存则保持 `null`；写 stderr 日志，不抛错
- [x] 实现 **export** `initProjectRootFromMcp(server: Server): Promise<void>`：
  - 检查 `server.getClientCapabilities()?.roots`；未声明则直接返回
  - 调用 `refreshProjectRootFromMcp(server)` 完成首次解析
  - 若 `getClientCapabilities()?.roots?.listChanged === true`，注册 `notifications/roots/list_changed` handler，收到通知时再次调用 `refreshProjectRootFromMcp(server)`

## 阶段 2: 修改 `getProjectDir` 回退链

- [x] 修改 `plugins/dev-team/bin/src/utils/constant.ts`：
  - 从 `../lib/project-root` 导入 `getMcpCachedProjectRoot`
  - `getProjectDir()` 返回 `getMcpCachedProjectRoot() ?? process.env.CLAUDE_PROJECT_DIR || process.env.CURSOR_PROJECT_DIR || process.cwd()`
- [x] 确认 `plugins/dev-team/bin/src/utils/index.ts` 仍 re-export `getProjectDir`（无需改动，除非 import 路径变化）
- [x] 确认 `plugins/dev-team/bin/src/lib/change.ts` 的 `getChangeDir()` 无需修改（已通过 `getProjectDir()` 自动受益）

## 阶段 3: MCP 服务器生命周期集成

- [x] 修改 `plugins/dev-team/bin/src/mcp.ts`：
  - 从 `./lib/project-root` 导入 `initProjectRootFromMcp`
  - 在 `await server.connect(transport)` 之后添加 `await initProjectRootFromMcp(server.server)`
- [x] 确认 `resolveProjectRoot(cwd?)` 逻辑不变：`cwd || getProjectDir()`
- [x] 确认所有 tool handler 签名与 schema 注册未改动

## 阶段 4: 单元测试

- [x] 新增 `plugins/dev-team/bin/src/lib/project-root.test.ts`
- [x] 每个 `describe` 块开头调用 `resetMcpProjectRootCacheForTests()`，必要时 `vi.stubEnv` / `vi.spyOn(process, 'cwd')` 隔离 env 与 cwd
- [x] 添加 **AC-1** 用例：mock `listRoots()` 返回 `{ roots: [{ uri: 'file:///D:/Projects/wps-claude-plugin' }] }`，`initProjectRootFromMcp` 后断言 `getProjectDir()` 等于平台规范化路径（Windows: `D:\Projects\wps-claude-plugin`）
- [x] 添加 **fileUriToPath** 用例：
  - Windows URI `file:///D:/Projects/wps-claude-plugin` → 正确本地路径
  - Unix URI `file:///home/user/projects/my-app` → `/home/user/projects/my-app`
- [x] 添加 **AC-4** 用例：mock 无 `roots` capability（`getClientCapabilities()` 返回 `{}` 或 `roots: undefined`），断言 `getProjectDir()` 回退到 env/cwd 链
- [x] 添加 **AC-4** 补充：`listRoots()` 抛错时 init 不抛错，`getProjectDir()` 回退
- [x] 添加 **AC-5** 用例：mock `listRoots()` 返回 `{ roots: [] }`，断言回退到 env/cwd 链
- [x] 添加 **AC-6** 用例：初始化缓存为旧路径后，模拟 `list_changed` 通知 + 新 `listRoots()` 响应，断言 `getProjectDir()` 返回新路径
- [x] 添加 **AC-6** 补充：刷新失败时保留已有缓存
- [x] 添加 env 优先级用例：无 MCP 缓存 + `CLAUDE_PROJECT_DIR` 已设置 → 返回 env 值（spec legacy fallback scenario）
- [x] 在 `plugins/dev-team/bin` 目录运行 `pnpm test`，确认 MCP roots 相关用例全部通过（**AC-8**；另有 5 个与 glob 相关的既有失败，与本变更无关）

## 阶段 5: 版本升级与构建

- [x] 将 `plugins/dev-team/.claude-plugin/plugin.json` 的 `version` 从 `2.6.22` 递增为 `2.6.23`（**AC-9**）
- [x] 在 `plugins/dev-team/bin` 运行 `pnpm build`，重新生成 `dev-team-mcp.cjs`

## 阶段 6: 验收确认

- [x] 确认未修改任何 MCP tool 的 Zod schema 文件
- [x] 确认未修改 `phase_log`、`phase_next` 等无 `project_root` 参数工具的 handler
- [x] 确认 CLI 入口（`cli.ts` / `dev-team-cli.cjs`）未改动
- [x] **AC-2**（手动，验收阶段验证）：在 Cursor MCP 环境调用 `change_list`（不传 `project_root`），验证返回的 `project_root` 为实际工作区路径
- [x] **AC-3**（手动，验收阶段验证）：在 Cursor MCP 环境调用 `phase_log`，验证 `eval.json` 写入工作区 `openspec/changes/<change>/eval.json`
- [x] **AC-7**（手动，验收阶段验证）：调用 `change_list` 并传入显式 `project_root`，验证使用传入值而非 MCP 自动解析
- [x] 对照 proposal 验收标准 AC-1~AC-9 逐项勾选（AC-2/AC-3/AC-7 待验收阶段手动确认）
