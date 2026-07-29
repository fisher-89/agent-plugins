# 任务: mcp-workspace-root

> **变更**: mcp-workspace-root
> **日期**: 2026-07-29

---

## 阶段 1: 重写 `lib/project-root`（候选 + pending/force）

- [x] 删除 `ProjectRootLockError` / `ProjectRootLockErrorCode` / `isProjectRootLockError` / `requireLockedProjectRoot` / `getMcpCachedProjectRoot` 及不可变 `mcpProjectRootCache` / `lockFailureReason`
- [x] 新增模块状态：`candidates`（去重集合）、`pendingKey`（至多一条）、`callScopedProjectRoot`
- [x] 新增 `ProjectRootResolveError` / `ProjectRootResolveErrorCode`（`invalid_path` | `not_in_candidates`）与 `isProjectRootResolveError`
- [x] 实现路径辅助：可用性校验（绝对 / `existsSync` / 拒绝 `${...}`）；`normalizeRootPath` + 平台相关比较键（不去 `realpath`）；`fileUriToPath` 与裸绝对 roots 条目转换保留
- [x] 实现 `WORKSPACE_FOLDER_PATHS`：**全部**可用段入候选（不再要求恰好 1 个）
- [x] 实现 `collectProjectRootCandidates(server)`：合并 `CLAUDE_PROJECT_DIR` ∪ `WORKSPACE` ∪ `roots/list`；`listRoots` 超时 2000ms；失败不阻断；禁用 cwd；不注册 `list_changed`；不因 `len==1` 锁定默认根
- [x] 保留 `initProjectRootFromMcp` 作为对 `collectProjectRootCandidates` 的薄别名（或同步改名调用点）
- [x] 实现 `stableStringify` + pending 键 `f(toolName, args)`；实现 `resolveProjectRootForTool(toolName, args)`（合法 → ∈ → force → 写 pending 报错；空候选逃生；非法路径不改 pending）
- [x] 实现 `runWithCallScopedRoot` / `getProjectRootCandidates`
- [x] 调整 `getProjectDir()`：call-scoped → 可用 `CLAUDE_PROJECT_DIR` → 恰好 1 个可用 `WORKSPACE_FOLDER_PATHS` → `process.cwd()`（仅 CLI；MCP 不得依赖末位）

## 阶段 2: MCP schemas 恢复必填 `project_root`

- [x] `schemas/change-list.schema.ts`：input 增加必填 `project_root`；**保留** output 的 `project_root`
- [x] `schemas/config-get.schema.ts`：增加必填 `project_root`
- [x] `schemas/archi-query.schema.ts` / `archi-validate.schema.ts` / `archi-write.schema.ts` / `archi-check.schema.ts`：增加必填 `project_root`
- [x] `schemas/test-detect-frameworks.schema.ts` / `test-resolve-paths.schema.ts`：增加必填 `project_root`
- [x] `schemas/phase-log.schema.ts` / `phase-next.schema.ts` / `backtrack.schema.ts`：增加必填 `project_root`
- [x] 确认 CLI/command 层显式 `projectRoot` / `project_root` fixture 能力未被误删或绑死到 MCP-only 契约

## 阶段 3: `mcp.ts` 改为 collect + resolve

- [x] 将 post-connect `init`/`collect` 调用改为候选采集语义；启动日志打印 candidates（或「空候选」），避免「已锁定 / 未锁定致全失败」文案
- [x] 用 `withResolvedProjectRoot(toolName, args, run)` 替换 `withLockedProjectRoot`：内部 `resolveProjectRootForTool` + `runWithCallScopedRoot`；捕获 `ProjectRootResolveError` 返回 JSON 错误载荷（含 `candidates` / `force_hint`）
- [x] 全部 tool handler（含 `phase_log` / `phase_next` / `backtrack`）传入完整 `args`；有参工具将 resolve 根传入 command / archi API
- [x] 确认不注册 `notifications/roots/list_changed`；MCP resolve 路径无 `process.cwd()` 回退

## 阶段 4: 配置核对、升版与重建

- [x] 核对 `plugins/dev-team/.mcp.json`：`dev-team` 无 `${workspaceFolder}`、无未展开 `CLAUDE_PROJECT_DIR` 回灌；有偏差则按方案纠正（不改 LikeC4 范围外行为除非必要）
- [x] 将 `plugins/dev-team/package.json` 的 `version` 从 `2.10.4` 升级为 `2.10.5`
- [x] 运行 `node scripts/build-plugins.mjs`，刷新双端产物

## 阶段 5: 实现自检（不含写测试）

- [x] 对照 design 变更清单：proposal「实现文件」均已覆盖；无残留 `requireLockedProjectRoot` / `ProjectRootLockError` 生产代码引用
- [x] 确认未实现 `list_changed` 热刷新；未加独立 `force` 字段 / 专用 resolve·set tool；未做 `len==1` 省略参默认
- [x] 确认未向上 walk git / `openspec/`；未改 CLI `options.projectRoot` 与 `{project_root}` 模板占位符
