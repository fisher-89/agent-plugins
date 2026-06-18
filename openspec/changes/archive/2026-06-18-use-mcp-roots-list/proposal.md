# 提案: use-mcp-roots-list

> **变更**: use-mcp-roots-list
> **日期**: 2026-06-18
> **状态**: draft

---

## 问题

`dev-team` MCP 服务器（`plugins/dev-team/bin/src/`）通过 `getProjectDir()`（`plugins/dev-team/bin/src/utils/constant.ts`）解析项目根目录：

```typescript
export function getProjectDir(): string {
  return process.env.CLAUDE_PROJECT_DIR || process.env.CURSOR_PROJECT_DIR || process.cwd();
}
```

在 Cursor IDE 中以 MCP 子进程方式运行时，存在以下问题：

1. **环境变量未注入**：`CLAUDE_PROJECT_DIR` 与 `CURSOR_PROJECT_DIR` 均未设置（`.mcp.json` 仅配置 `node ${CLAUDE_PLUGIN_ROOT}/bin/dev-team-mcp.cjs`，未传递工作区路径）。
2. **`process.cwd()` 指向错误目录**：MCP 子进程的当前工作目录为用户主目录（如 `C:\Users\wps`），而非实际工作区（如 `D:\Projects\wps-claude-plugin`）。
3. **下游工具读写路径错误**：
   - `change_list` 返回 `"project_root": "C:\\Users\\wps"` 而非工作区路径；
   - `phase_log` 将 `eval.json` 写入 `C:\Users\wps\openspec\changes\...\eval.json` 而非工作区下的 `openspec/changes/.../eval.json`；
   - `phase_next` 通过 `getChangeDir()` 间接依赖 `getProjectDir()`，同样读取错误位置的 `eval.json`。

受影响调用链包括：`mcp.ts` 的 `resolveProjectRoot()`、各 command 中的 `getProjectDir()` 回退、以及 `lib/change.ts` 的 `getChangeDir()`（被 `phase_log`、`phase_next` 等无 `project_root` 参数的工具使用）。

MCP 协议已定义标准的 **`roots/list`** 机制：客户端向服务器暴露工作区根目录（`file://` URI），服务器通过 `roots/list` 请求获取。Cursor 作为 MCP 客户端支持该能力，是当前场景下获取工作区路径的正确方式。

---

## 提案

在 MCP 服务器启动并完成客户端初始化后，通过 MCP SDK 的 `server.server.listRoots()` 向客户端请求工作区根目录，将解析结果缓存为项目根目录；`getProjectDir()` 优先返回该缓存值，并在客户端不支持 roots 或请求失败时回退到现有环境变量 / `process.cwd()` 链。

### 实现要点

1. **新增项目根解析模块**（如 `plugins/dev-team/bin/src/lib/project-root.ts`）：
   - `initProjectRootFromMcp(server: Server): Promise<void>` — 在 `server.connect()` 之后、处理首个 tool 请求之前调用；检查 `server.getClientCapabilities()?.roots`，若支持则调用 `listRoots()`，将首个 root 的 `file://` URI 转为本地绝对路径并写入模块级缓存。
   - `getProjectDir(): string` — 若 MCP 缓存已设置则返回缓存；否则保持现有 `CLAUDE_PROJECT_DIR || CURSOR_PROJECT_DIR || process.cwd()` 回退（兼容 CLI 与非 MCP 运行模式）。
   - `fileUriToPath(uri: string): string` — 将 MCP root URI（`file:///D:/Projects/...` 或 `file:///home/user/...`）规范化为平台本地路径。

2. **MCP 服务器生命周期集成**（`mcp.ts`）：
   - 在 `await server.connect(transport)` 之后调用 `initProjectRootFromMcp(server.server)`。
   - 若客户端声明 `roots.listChanged`，注册 `notifications/roots/list_changed` 处理器，收到通知时重新调用 `listRoots()` 并更新缓存。

3. **多 root 策略**：当客户端返回多个 root 时，使用列表中**第一个** root 作为项目根（与 Cursor 单工作区场景一致；多 root 场景可在后续变更中扩展）。

4. **无需修改 tool schema**：所有现有 MCP 工具的 input/output 签名保持不变；`project_root` 可选参数的行为不变（显式传入时仍优先于自动解析）。

5. **单元测试**：mock `Server.listRoots()` 与 `getClientCapabilities()`，覆盖 roots 可用、roots 不可用、空列表、Windows `file://` URI 解析、以及 `roots/list_changed` 缓存刷新等场景。

6. 按项目规则升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号。

---

## 能力

### 新增能力

- **mcp-project-root** — 定义 dev-team MCP 服务器通过 MCP `roots/list` 解析工作区项目根目录的机制，含缓存、回退与 URI 转换规则

### 修改的能力

（无 — 全局 `openspec/specs/` 中尚无 `mcp-project-root` 能力定义，本次以 delta spec 新增）

---

## 变更范围

### 实现以下特性

- 新增 `lib/project-root.ts`（或等价模块）实现 MCP roots 解析与缓存
- 修改 `utils/constant.ts` 中的 `getProjectDir()`，优先返回 MCP 缓存的项目根
- 在 `mcp.ts` 的 `main()` 中于 `connect` 后初始化项目根，并可选注册 `roots/list_changed` 通知
- 新增 `project-root.test.ts` 单元测试
- 升级 `plugin.json` 版本号
- 重新构建 `dev-team-mcp.cjs`

### 不要修改

- 各 MCP 工具的 input/output Zod schema（不新增 `project_root` 参数到 `phase_log`、`phase_next` 等）
- MCP tool 注册名称与 handler 签名
- 非 `dev-team` 插件
- CLI 入口（`cli.ts` / `dev-team-cli.cjs`）的独立行为 — CLI 仍可通过 `CLAUDE_PROJECT_DIR` 或 `process.cwd()` 工作；MCP 模式通过 roots 增强
- OpenSpec 工作流技能与 agent 定义

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | MCP 客户端支持 roots 且返回 `file://` 工作区 URI 时，`getProjectDir()` 返回该 URI 对应的本地绝对路径 | 单元测试 mock `listRoots()` 返回 `file:///D:/Projects/wps-claude-plugin`，断言 `getProjectDir()` 等于 `D:\Projects\wps-claude-plugin`（或平台等价路径） |
| AC-2 | 在 Cursor MCP 环境中调用 `change_list`（不传 `project_root`）时，返回的 `project_root` 为实际工作区路径，而非用户主目录 | 在 Cursor 中调用 `change_list`，检查 `project_root` 字段 |
| AC-3 | 在 Cursor MCP 环境中调用 `phase_log`（不传 `project_root`）时，`eval.json` 写入工作区 `openspec/changes/<change>/eval.json` | 调用 `phase_log` 后检查工作区（非主目录）下 `eval.json` 是否更新 |
| AC-4 | 客户端不支持 roots 或 `listRoots()` 失败时，`getProjectDir()` 回退到 `CLAUDE_PROJECT_DIR \|\| CURSOR_PROJECT_DIR \|\| process.cwd()` | 单元测试 mock 无 roots capability 或 `listRoots` 抛错，断言回退行为 |
| AC-5 | 客户端返回空 roots 列表时，回退到环境变量 / `process.cwd()` 链 | 单元测试 mock 空 `roots` 数组，断言回退 |
| AC-6 | 收到 `roots/list_changed` 通知后，缓存的项目根被刷新 | 单元测试模拟通知后再次 `listRoots`，断言 `getProjectDir()` 返回新值 |
| AC-7 | 显式传入 `project_root` 的工具参数仍优先于 MCP 自动解析 | 调用带 `project_root` 的 `change_list`，断言使用传入值 |
| AC-8 | 单元测试全部通过 | 在 `plugins/dev-team/bin` 运行 `pnpm test` |
| AC-9 | `plugin.json` 版本号已升级 | 检查 `plugins/dev-team/.claude-plugin/plugin.json` |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Cursor 客户端未声明 roots capability，仍回退到错误 cwd | 高 | 低 | 保留 env 回退链；若 Cursor 不支持 roots，可后续通过 MCP 配置注入 `CURSOR_PROJECT_DIR` 作为补充 |
| `file://` URI 在 Windows 上解析错误（盘符、编码） | 中 | 中 | 使用 Node.js `fileURLToPath` 或等价实现；单元测试覆盖 Windows 风格 URI |
| 多 root 工作区时选错根目录 | 中 | 低 | 首版取第一个 root；文档说明限制，后续可按 `name` 或 URI 匹配扩展 |
| `initProjectRootFromMcp` 在首个 tool 调用前未完成，竞态导致短暂使用回退路径 | 中 | 低 | 在 `connect` 后 `await init` 再注册 tool handler，或 tool handler 内 lazy-init 并 await |
| MCP 规范中 roots 已标记 deprecated（2026 RC） | 低 | 低 | 当前 Cursor 仍支持；保留 env/cwd 回退以便未来迁移到 tool 参数或配置注入 |
