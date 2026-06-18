# 测试设计: use-mcp-roots-list

> **日期**: 2026-06-18

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | MCP 客户端支持 roots 且返回 `file://` 工作区 URI 时，`getProjectDir()` 返回该 URI 对应的本地绝对路径 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — roots 可用时缓存首个 root |
| AC-1 | MCP 客户端支持 roots 且返回 `file://` 工作区 URI 时，`getProjectDir()` 返回该 URI 对应的本地绝对路径 | 单元测试 | `plugins/dev-team/bin/src/utils/constant.test.ts` | `getProjectDir` — MCP 缓存优先于 env/cwd |
| AC-2 | 在 Cursor MCP 环境中调用 `change_list`（不传 `project_root`）时，返回的 `project_root` 为实际工作区路径，而非用户主目录 | 集成测试 | — | Cursor MCP 手工验证 |
| AC-3 | 在 Cursor MCP 环境中调用 `phase_log`（不传 `project_root`）时，`eval.json` 写入工作区 `openspec/changes/<change>/eval.json` | 集成测试 | — | Cursor MCP 手工验证 |
| AC-4 | 客户端不支持 roots 或 `listRoots()` 失败时，`getProjectDir()` 回退到 `CLAUDE_PROJECT_DIR \|\| CURSOR_PROJECT_DIR \|\| process.cwd()` | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — 无 roots capability / listRoots 抛错 |
| AC-4 | 客户端不支持 roots 或 `listRoots()` 失败时，`getProjectDir()` 回退到 `CLAUDE_PROJECT_DIR \|\| CURSOR_PROJECT_DIR \|\| process.cwd()` | 单元测试 | `plugins/dev-team/bin/src/utils/constant.test.ts` | `getProjectDir` — 无 MCP 缓存时 env/cwd 回退链 |
| AC-5 | 客户端返回空 roots 列表时，回退到环境变量 / `process.cwd()` 链 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — 空 roots 数组 |
| AC-5 | 客户端返回空 roots 列表时，回退到环境变量 / `process.cwd()` 链 | 单元测试 | `plugins/dev-team/bin/src/utils/constant.test.ts` | `getProjectDir` — 缓存为 null 时回退 |
| AC-6 | 收到 `roots/list_changed` 通知后，缓存的项目根被刷新 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `refreshProjectRootFromMcp` / `list_changed` 通知 — 缓存更新 |
| AC-6 | 收到 `roots/list_changed` 通知后，缓存的项目根被刷新 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `refreshProjectRootFromMcp` — 刷新失败保留旧缓存 |
| AC-6 | 若 `roots.listChanged === true`，注册 `notifications/roots/list_changed` handler（数据流步骤 2） | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — list_changed 注册（`roots.listChanged === true` 时注册 handler） |
| AC-6 | 若 `roots.listChanged` 为 `false` 或未声明，不注册 notification handler（数据流步骤 2） | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — list_changed 注册（`listChanged` 为 false 或未声明时不注册） |
| D6 | `roots/list_changed` 刷新返回空 roots 时保留已有有效缓存 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `refreshProjectRootFromMcp` — 刷新返回空 roots |
| D8 | 导出 `resetMcpProjectRootCacheForTests()` 供 vitest 用例间隔离模块级缓存 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `resetMcpProjectRootCacheForTests` — 测试隔离 |
| AC-7 | 显式传入 `project_root` 的工具参数仍优先于 MCP 自动解析 | 集成测试 | — | Cursor MCP 手工验证（`change_list` 显式 `project_root`） |
| AC-1~AC-6 | `file://` URI 正确转换为平台本地绝对路径 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | `fileUriToPath` — Windows / Unix URI 转换 |
| AC-1~AC-3 | `getChangeDir()` 间接受益于 MCP 根解析 | 单元测试 | `plugins/dev-team/bin/src/lib/change.test.ts` | `getChangeDir` — MCP 缓存已设置时路径拼接 |
| AC-8 | 单元测试全部通过 | 集成测试 | — | 全量测试回归（`plugins/dev-team/bin` 执行 `pnpm test`） |
| AC-8 | 运行 `pnpm build` 重新打包 `dev-team-mcp.cjs`（迁移步骤 7） | 集成测试 | — | 构建回归（`plugins/dev-team/bin` 执行 `pnpm build`） |
| AC-9 | `plugin.json` 版本号已升级 | — | — | 人工检查 `plugins/dev-team/.claude-plugin/plugin.json`（见不可测试项） |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `fileUriToPath` — Windows URI 转换 | 正向 | `file:///D:/Projects/wps-claude-plugin` 转为 `D:\Projects\wps-claude-plugin`（或平台等价路径）(AC-1) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `fileUriToPath` — Windows URI 转换 | 边界 | `file:///D:/Projects/wps-claude-plugin/` 尾随斜杠时路径规范化正确 | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `fileUriToPath` — Unix URI 转换 | 正向 | `file:///home/user/projects/my-app` 转为 `/home/user/projects/my-app` | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `fileUriToPath` — 无效输入 | 异常 | 空字符串 `""` 抛错或返回可预期错误 | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `fileUriToPath` — 无效输入 | 异常 | 非 `file://` scheme（如 `http://example.com`）抛错 | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `fileUriToPath` — 无效输入 | 边界 | URI 含 URL 编码字符（如 `%20`）解码为正确本地路径 | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — roots 可用时缓存首个 root | 正向 | mock `getClientCapabilities().roots` 存在且 `listRoots()` 返回单条 Windows `file://` root，断言 `getMcpCachedProjectRoot()` 与 `getProjectDir()` 为本地绝对路径 (AC-1) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — roots 可用时缓存首个 root | 正向 | mock 返回多条 root 时仅使用 `roots[0].uri`（D3 多 root 策略） | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — 无 roots capability | 正向 | `getClientCapabilities()?.roots` 为 `undefined` 时不调用 `listRoots()`，缓存保持 `null` (AC-4) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — listRoots 抛错 | 异常 | `listRoots()` reject/throw 时缓存保持 `null`，不阻断初始化 (AC-4) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — 空 roots 数组 | 边界 | `listRoots()` 返回 `{ roots: [] }` 时缓存保持 `null` (AC-5) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — list_changed 注册 | 正向 | `roots.listChanged === true` 时调用 `setNotificationHandler('notifications/roots/list_changed', ...)` (AC-6) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `initProjectRootFromMcp` — list_changed 注册 | 边界 | `roots.listChanged` 为 `false` 或未声明时不注册 notification handler (AC-6) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `refreshProjectRootFromMcp` / `list_changed` 通知 — 缓存更新 | 正向 | 初始缓存为旧路径，模拟 `list_changed` 后 `listRoots()` 返回新 URI，断言 `getProjectDir()` 返回新路径 (AC-6) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `refreshProjectRootFromMcp` — 刷新失败保留旧缓存 | 异常 | 已有有效缓存时 `listRoots()` 失败，`getProjectDir()` 仍返回旧路径 (AC-6 / D6) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `refreshProjectRootFromMcp` — 刷新返回空 roots | 边界 | 已有有效缓存时 refresh 返回 `{ roots: [] }`，保留旧缓存 (D6) | 新增 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `resetMcpProjectRootCacheForTests` — 测试隔离 | 边界 | 每用例 `beforeEach` 调用 reset 后缓存为 `null`，避免用例间污染 (D8) | 新增 |
| `plugins/dev-team/bin/src/utils/constant.test.ts` | `getProjectDir` — MCP 缓存优先于 env/cwd | 正向 | 通过 `initProjectRootFromMcp` 或测试 helper 设置 MCP 缓存后，即使 `CLAUDE_PROJECT_DIR` / `CURSOR_PROJECT_DIR` 已设置，仍返回缓存值 (AC-1 / D5) | 新增 |
| `plugins/dev-team/bin/src/utils/constant.test.ts` | `getProjectDir` — 无 MCP 缓存时 env/cwd 回退链 | 正向 | 缓存为 `null` 且 `CLAUDE_PROJECT_DIR` 已设置时返回该 env 值 (AC-4) | 新增 |
| `plugins/dev-team/bin/src/utils/constant.test.ts` | `getProjectDir` — 无 MCP 缓存时 env/cwd 回退链 | 正向 | 无 MCP 缓存、无 `CLAUDE_PROJECT_DIR` 时返回 `CURSOR_PROJECT_DIR` | 新增 |
| `plugins/dev-team/bin/src/utils/constant.test.ts` | `getProjectDir` — 无 MCP 缓存时 env/cwd 回退链 | 边界 | 无 MCP 缓存且两 env 均未设置时返回 `process.cwd()` (AC-4 / AC-5) | 新增 |
| `plugins/dev-team/bin/src/utils/constant.test.ts` | `getProjectDir` — env 空字符串 | 边界 | `CLAUDE_PROJECT_DIR=""` 视为未设置，继续检查 `CURSOR_PROJECT_DIR` | 新增 |
| `plugins/dev-team/bin/src/lib/change.test.ts` | `getChangeDir` — MCP 缓存已设置时路径拼接 | 正向 | MCP 缓存为 `/workspace/project` 时 `getChangeDir('my-change')` 等于 `/workspace/project/openspec/changes/my-change`（平台规范化） | 新增 |
| `plugins/dev-team/bin/src/lib/change.test.ts` | `getChangeDir` — 无 MCP 缓存时回退 | 边界 | 无 MCP 缓存、设置 `CLAUDE_PROJECT_DIR` 时 `getChangeDir` 基于 env 根目录拼接 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | MCP `Server` | 构造最小 mock 对象：`getClientCapabilities()` 返回 `{ roots?: { listChanged?: boolean } }`；`listRoots()` 返回 `{ roots: [{ uri, name? }] }` 或 reject；`setNotificationHandler(method, handler)` 捕获 `notifications/roots/list_changed` 回调供测试触发 | `initProjectRootFromMcp`、`refreshProjectRootFromMcp`、`list_changed` 全部用例 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | 模块级缓存 | 每用例 `beforeEach` 调用 `resetMcpProjectRootCacheForTests()`；必要时直接通过 init/refresh 写入后再断言 `getMcpCachedProjectRoot()` | 所有 project-root 用例 |
| `plugins/dev-team/bin/src/lib/project-root.test.ts` | `process.stderr` | 可选 spy `console.error` 或 stderr write，验证 `listRoots` 失败时记录日志且不抛错 | listRoots 失败、空 roots 用例 |
| `plugins/dev-team/bin/src/utils/constant.test.ts` | 环境变量 | `beforeEach`/`afterEach` 保存并恢复 `process.env.CLAUDE_PROJECT_DIR`、`process.env.CURSOR_PROJECT_DIR`；配合 `resetMcpProjectRootCacheForTests()` 隔离 | env 回退链全部用例 |
| `plugins/dev-team/bin/src/utils/constant.test.ts` | MCP 缓存 | 通过 mock `Server` 调用 `initProjectRootFromMcp` 设置缓存，或测试内 export 的 cache helper（若 init 过重可仅 set cache via init mock） | MCP 优先于 env 用例 |
| `plugins/dev-team/bin/src/utils/constant.test.ts` | `process.cwd()` | 一般无需 mock；回退至 cwd 的用例可断言返回值与 `process.cwd()` 相等 | 最低优先级回退用例 |
| `plugins/dev-team/bin/src/lib/change.test.ts` | MCP 缓存 / env | 复用 project-root 的 init mock 或 env 设置 + `resetMcpProjectRootCacheForTests()`；不 mock `path.resolve` | `getChangeDir` 路径拼接用例 |

---

## 集成测试

本变更的核心路径解析逻辑在 `project-root.test.ts`、`constant.test.ts`、`change.test.ts` 内通过 mock MCP `Server` 覆盖。下列为全量回归与 Cursor 环境手工验证项：

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-8 | — | 全量测试回归 | 在 `plugins/dev-team/bin` 执行 `pnpm test`，含新增 `project-root.test.ts`、`constant.test.ts`、`change.test.ts` 及既有用例全部通过 | 新增 |
| AC-8 | — | 构建回归 | 在 `plugins/dev-team/bin` 执行 `pnpm build`（`vp pack`）无错误，重新生成 `dev-team-mcp.cjs`（迁移步骤 7） | 新增 |
| AC-2 | — | Cursor MCP `change_list` | 不传 `project_root` 调用 `change_list`，断言返回 `project_root` 为实际工作区路径（非用户主目录） | 新增 |
| AC-3 | — | Cursor MCP `phase_log` | 不传 `project_root` 调用 `phase_log`，检查工作区 `openspec/changes/<change>/eval.json` 已更新（非主目录下同名路径） | 新增 |
| AC-7 | — | Cursor MCP 显式 `project_root` | 调用 `change_list` 并传入自定义 `project_root`，断言返回字段与扫描目录均使用该值，不受 MCP 缓存影响 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| — | 无 | AC-8 / 构建回归依赖本地或 CI 执行 `pnpm test` / `pnpm build` | 全量回归 |
| — | Cursor MCP 客户端 | 无 mock；在 Cursor IDE 中启用 dev-team MCP 服务器，利用真实 `roots/list` 握手 | AC-2、AC-3、AC-7 手工验证 |

---

## 不可测试项

- **AC-2（Cursor 中 `change_list` 的 `project_root`）** — **原因**: 依赖 Cursor MCP 客户端通过 stdio 注入真实 `roots/list` 响应；无法在 vitest 进程内复现完整 MCP 子进程 + 客户端 capability 握手。实施阶段在 Cursor 中手工调用验证。
- **AC-3（Cursor 中 `phase_log` 写入路径）** — **原因**: 同上，需真实 MCP 会话与文件系统副作用；单测已通过 `getChangeDir` + mock 缓存间接保证路径拼接逻辑。
- **AC-7（显式 `project_root` 覆盖 MCP 缓存）** — **原因**: `resolveProjectRoot` 为 `mcp.ts` 内部未导出函数；其语义为 `cwd \|\| getProjectDir()`，逻辑 trivial 且 proposal 指定 Cursor 手工验证。可选补充：在既有 `change-list.test.ts` 中已有显式 `project_root` fixture 模式，但不属于本变更新增测试范围。
- **AC-9（`plugin.json` 版本号升级）** — **原因**: 版本 bump 为发布流程人工检查项；实施时确认 `plugins/dev-team/.claude-plugin/plugin.json` 的 `version` 已从 `2.6.22` 递增至 `2.6.23`（或当前基线 +1）。
- **`mcp.ts` 生命周期 wiring（connect 后 await init）** — **原因**: `main()` 为 async 入口，启动 stdio transport 与注册全部 tool handler；单测 mock 成本高且 `initProjectRootFromMcp` 行为已在 `project-root.test.ts` 覆盖。竞态缓解（D2：connect 后立即 await init）通过代码审查 + AC-2/AC-3 手工验证确认。
- **`plugins/dev-team/bin/src/mcp.test.ts`** — **原因**: `test_resolve_paths` 为 `mcp.ts` 推导出该路径，但 design 迁移步骤仅要求新增 `project-root.test.ts`；`mcp.ts` 无新增可导出纯函数，不强制新增 `mcp.test.ts` 文件。
