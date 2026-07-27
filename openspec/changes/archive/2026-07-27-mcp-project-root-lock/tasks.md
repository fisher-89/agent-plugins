# 任务: mcp-project-root-lock

> **变更**: mcp-project-root-lock
> **日期**: 2026-07-24

---

## 阶段 1: 重写 `lib/project-root` 并归并 `getProjectDir`

- [x] 在 `plugins/dev-team/bin/src/lib/project-root.ts` 新增 `ProjectRootLockError` / `ProjectRootLockErrorCode`，以及模块级 `lockFailureReason`（或等价）用于记录失败原因
- [x] 实现路径可用性校验辅助（模块内部即可）：绝对路径、`existsSync`、拒绝含 `${...}` 的字面量
- [x] 实现 `WORKSPACE_FOLDER_PATHS` 解析：按 `,` 与 `;` 分割、trim、去空段；仅当恰好 1 个可用绝对路径时返回该路径，否则返回「不可用」（多路径不取 `[0]`）
- [x] 扩展 URI/路径转换：保留 `fileUriToPath`；roots 条目若为裸绝对路径且通过可用性校验亦可采纳
- [x] 重写 `initProjectRootFromMcp(server)`：
  - 若缓存已锁定 → 立即 return（进程内不可变）
  - 优先级：可用 `CLAUDE_PROJECT_DIR` →（有 roots capability 时）`listRoots()` 恰好 1 个可用根 → 恰好 1 个 `WORKSPACE_FOLDER_PATHS`
  - `listRoots` 失败 / `-32601` / 空 / 多根：写可区分的 stderr，继续下一通道，不抛到 connect
  - 全失败：缓存保持 `null`，记录失败原因；**不**回退 `cwd`；**不** `process.exit`
- [x] 删除 `refreshProjectRootFromMcp` / `applyRootsList` 中「取 `roots[0]`」与 `list_changed` 刷新语义（可内联为「恰好 1 个才写入」的私有逻辑）
- [x] 新增并导出 `requireLockedProjectRoot(): string`：有缓存则返回；否则抛带原因的 `ProjectRootLockError`
- [x] 将 `getProjectDir()` 从 `utils/constant.ts` 迁入本模块：优先锁定缓存 → 可用 `CLAUDE_PROJECT_DIR` → 恰好 1 个 WORKSPACE → 最后 `process.cwd()`（仅 CLI/command 辅助；MCP 不得依赖末位）
- [x] 保留并导出 `getMcpCachedProjectRoot` / `McpServerLike`

## 阶段 2: 删除 `utils/constant` 并调整 barrel

- [x] 删除 `plugins/dev-team/bin/src/utils/constant.ts`
- [x] 修改 `plugins/dev-team/bin/src/utils/index.ts`：`export { getProjectDir } from '../lib/project-root'`
- [x] 确认仓库内无残留对 `utils/constant` 的实现 import（搜索即可）；command / hooks / `lib/change` 可继续 `from '../utils'` 或改为直接 `from '../lib/project-root'`（以实现时风格为准）

## 阶段 3: MCP schemas 去掉 `project_root` input

- [x] `schemas/change-list.schema.ts`：input 删除 `project_root`；**保留** output 的 `project_root`
- [x] `schemas/config-get.schema.ts`：删除 `project_root`
- [x] `schemas/archi-query.schema.ts` / `archi-validate.schema.ts` / `archi-write.schema.ts` / `archi-check.schema.ts`：删除 `project_root`
- [x] `schemas/test-detect-frameworks.schema.ts` / `test-resolve-paths.schema.ts`：删除 `project_root`
- [x] 确认 CLI/command 层类型（如 `runChangeList` 的 `project_root`、`runConfigGet` 的 `projectRoot`）**未**被误删

## 阶段 4: `mcp.ts` 入口强制锁定

- [x] 删除 `resolveProjectRoot`；改为从 `./lib/project-root` 导入 `requireLockedProjectRoot`、`ProjectRootLockError`（及仍需要的 `initProjectRootFromMcp`）
- [x] 为全部 tool handler（含 `phase_log` / `phase_next` / `backtrack`）在入口调用 `requireLockedProjectRoot()`
- [x] 有参工具：用锁定根调用 command / archi API，不再读取 `args.project_root`
- [x] 统一捕获 `ProjectRootLockError`，返回 `{ isError: true, content: [{ type: 'text', text: message }] }`（或与现有 MCP SDK 一致的可观测错误）
- [x] 确认不注册 `notifications/roots/list_changed`
- [x] 启动 logging：已锁定则打印锁定路径；未锁定则打印明确「未锁定」信息，避免暗示已用 cwd（勿再无条件调用 `getProjectDir()` 作为启动文案）

## 阶段 5: 版本升级与重建 bundle

- [x] 将 `plugins/dev-team/.claude-plugin/plugin.json` 的 `version` 从 `2.10.2` 升级为 `2.10.3`
- [x] 在 `plugins/dev-team/bin` 运行 `pnpm build`，重建 `dev-team-mcp.cjs`

## 阶段 6: 实现自检（不含写测试）

- [x] 对照 design 变更清单：proposal「实现文件」均已覆盖；`utils/constant.ts` 已删除
- [x] 确认未向共享 `.mcp.json` 写入 `${workspaceFolder}`
- [x] 确认未实现 `list_changed` 热刷新；未改 CLI `options.projectRoot` 契约与 `{project_root}` 模板占位符
