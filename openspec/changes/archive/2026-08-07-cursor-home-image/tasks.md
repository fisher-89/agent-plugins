# 任务: cursor-home-image

> **变更**: cursor-home-image
> **日期**: 2026-08-06

---

## 阶段 1：构建核心（env / token / pack）

- [x] 新增 `plugins/dev-team/build/env.ts`：定义 `ProductEnvKey` / `ProductEnv` / `HooksProfile` 等类型与三行 env 常量（含显式 `outDir`、cursorHome 用户级前缀）
- [x] 新增 `plugins/dev-team/build/apply-env-tokens.ts`：实现 `applyEnvTokens(text, env, options?)`（名称类五种 token + 可控路径 token）
- [x] 新增 `plugins/dev-team/build/scan-files.ts`：宽 include / 窄 exclude 枚举待处理文本文件
- [x] 新增 `plugins/dev-team/build/assert-no-tokens.ts`：实现 `assertNoNameTokens`（cursorHome 允许路径 token 残留）
- [x] 修改 `plugins/dev-team/vite.config.ts`：改为单轮 multi-entry pack，输出到 `.pack-staging/bin/{mcp,cli,hooks}.cjs`；移除 `SUPPORT_AGENTS` 倍增 pack；`build:done` 预留调用 `assembleAll` + `generateConfigJsonSchema`
- [x] 更新 `.gitignore`：忽略 `.pack-staging/`；确认不忽略 `cursor-home-image/`

## 阶段 2：hooks 权威源与 hooksProfile

- [x] 新增 `plugins/dev-team/hooks/hooks.canonical.json`：从现有 `hooks/hooks.json` 提取逻辑事件、command 模板（使用 `__DEV_TEAM_RUNTIME_ROOT__` + `__BIN:hooks__`）、loop_limit；为 claudeNested / cursorNative 分列 PreToolUse matcher（含 Cursor 的 `Shell` / `StrReplace`）
- [x] 新增 `plugins/dev-team/build/hooks-profile.ts`：实现 `buildHooksFile(canonical, env)`，分别组装 nested 与 native JSON；SubagentStop matcher 使用 `namePrefix + agentLogicalId`
- [x] 删除或以非权威方式处理源码 `plugins/dev-team/hooks/hooks.json`（成品仅由 assemble 写入各产物）

## 阶段 3：assemble×3 与安装器源

- [x] 新增 `plugins/dev-team/build/assemble.ts`：实现 `assemble(key)` / `assembleAll()`——清空/重建 `outDir`、复制静态资产、staging CJS → `namePrefix` 派生 bin 名、FS 权威 rename skills/agents、宽扫 `applyEnvTokens`、写 hooks/mcp/plugin manifest、写 home `manifest.json`、调用断言
- [x] 将原 `build-agent-artifacts.ts` 中的复制 / mcp / plugin.json 逻辑并入 assemble（或薄封装后删除旧 API：`AgentType`、`getOutputPathByAgent`、`buildAgentArtifacts`）
- [x] 更新 `plugins/dev-team/build/index.ts` 导出新公共 API
- [x] 新增 `plugins/dev-team/build/home-install.mjs`：`--root`、路径 token 绝对化、非 JSON 路径 `/` 规范化、按 manifest sync、hooks/mcp managed merge（`dev-team_` 识别）、写安装状态、Reload 提示；assemble 复制到 `cursor-home-image/dev-team/install.mjs`
- [x] 接通 vite `build:done` → `assembleAll()`，确保 schema 生成仍执行且 schema 被复制进三产物

## 阶段 4：源码占位符化

- [x] 将 `plugins/dev-team/bin/src/lib/workflow.ts`（及同类字符串）中的 `dev-team:…` agent_type 改为 `__AGENT:…__`
- [x] 将 `plugins/dev-team/skills/**` 中的 MCP / slash / skill 硬编码改为 `__MCP:` / `__SKILL_SLASH:` / `__SKILL:` 等
- [x] 将 `plugins/dev-team/agents/**` 同上；`proposal-evaluator.md` / `acceptance-evaluator.md` 的 `tools:` 去掉服务器级白名单，改为具体 `__MCP:<tool>__`（至少 `phase_log`）
- [x] 将 `plugins/dev-team/templates/**`、`plugins/dev-team/utils/**` 中的路径与限定名改为路径 token / 名称 token
- [x] 全量检索源码：确认无残留仅面向单一产品的硬编码限定名（`mcp__plugin_dev-team_dev-team__` 字面量、裸服务器 allowlist、函数形占位符）

## 阶段 5：protect-files Cursor 工具名

- [x] 修改 `plugins/dev-team/bin/src/hooks.ts`：`Shell` 与 `Bash` 共用命令写入检测；`StrReplace` 与 `Edit` 共用 `file_path` 保护；保留 Write/Edit/Bash/PowerShell 行为

## 阶段 6：文档、版本与产物落盘

- [x] 更新 `CLAUDE.md` 升版规则：bump `plugins/<name>/package.json` 后重建 claude / cursor / cursorHome 三产物
- [x] bump `plugins/dev-team/package.json` 的 `version`
- [x] 执行 `pnpm -C plugins/dev-team run build`，手动确认：三产物目录存在且 version 一致；staging CJS 仍含未展开 token；三产物无名称类 token 残留；home 保留路径 token；home 含前缀化 skills/agents、`install.mjs`、`manifest.json`、`mcp.dev-team.json`、根 `hooks.json`；plugin bin 为 `mcp.cjs`/`cli.cjs`/`hooks.cjs`；home bin 为 `dev-team_*.cjs`
- [x] 对干净或已有内容的临时 `--root` 试跑 `node cursor-home-image/dev-team/install.mjs`，确认路径绝对化、managed merge 保留用户条目、Reload 提示可见
- [x] 确认 `cursor-home-image/` 未被 gitignore，三产物可随变更一并提交
