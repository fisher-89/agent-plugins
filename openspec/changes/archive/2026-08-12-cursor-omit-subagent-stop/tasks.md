# 任务: cursor-omit-subagent-stop

> **变更**: cursor-omit-subagent-stop
> **日期**: 2026-08-12

---

## 阶段 1: Canonical hooks — Cursor 省略 subagentStop

- [x] 修改 `plugins/dev-team/hooks/hooks.canonical.json`：两条 `subagentStop` 的 `matchers.cursor` 设为 `null`；保留 `matchers.claude`、`loop_limit: 5`、`commandTemplate`（含 `static-check`）
- [x] 按需更新顶层 `description`，注明静态检查 hook 主要作用于 Claude `SubagentStop`
- [x] 修改 `plugins/dev-team/build/hooks-profile.ts` 中 `buildCursorNative`：对 `preToolUse` / `subagentStop` 过滤 `matchers.cursor == null` 的条目；过滤后若某事件数组为空则**省略该键**（禁止写空数组）
- [x] 同步让 `buildClaudeNested` 的 `SubagentStop` 与 `PreToolUse` 一样过滤 null `matchers.claude`（本变更 matcher 仍非 null，产物应继续含两项 generator）

## 阶段 2: `__INCLUDE` 展开模块

- [x] 新增 `plugins/dev-team/build/expand-includes.ts`，实现模块内 `resolveFragment(id, env)`（不导出）：唯一根 `_fragments/`，优先级 `<id>.<agent>.md` → `<id>.md`，皆无抛错；空文件合法；行为经 `expandIncludes` 间接测试
- [x] 实现并导出 `expandIncludes(text, env, stack?, depth?)`：匹配 `__INCLUDE:([a-z0-9-]+)__`；环检测；软深度上限默认 `16`（模块内常量，不导出）；递归展开后对嵌入体 `trim` 再 splice
- [x] 非法 id / 缺文件 / 环 / 超深均使构建失败（错误信息可读）

## 阶段 3: Assemble 管道与残留断言

- [x] 修改 `plugins/dev-team/build/assemble.ts`：在对 outDir 文本应用 `applyEnvTokens` 之前调用 `expandIncludes`（顺序：`expandIncludes` → `applyEnvTokens` → `assertNoNameTokens`）
- [x] 确认 `copyAgents` / `copySkills` / `copyStaticAssets` 等不会把 `_fragments/` 拷进任何 `outDir`
- [x] 修改 `plugins/dev-team/build/assert-no-tokens.ts`：产物中残留 `__INCLUDE:` 视为失败

## 阶段 4: Fragment 源与 generator 引用

- [x] 新增空文件 `plugins/dev-team/_fragments/static-analysis-gate.md`（Claude default → 空串）
- [x] 新增 `plugins/dev-team/_fragments/static-analysis-gate.cursor.md`：结束前必须调用 `node "__DEV_TEAM_ROOT__/bin/__BIN:cli__" run_static_analysis`；失败修复重跑；未通过不得结束
- [x] 在 `plugins/dev-team/agents/implementation-generator.md` 的 `## Process` 既有步骤之后追加 `__INCLUDE:static-analysis-gate__`
- [x] 在 `plugins/dev-team/agents/test-gen-generator.md` 的 `## Process` 既有步骤之后追加 `__INCLUDE:static-analysis-gate__`
- [x] 两 generator 源正文不得再手写与 Claude `SubagentStop` 重复的静态检查长步骤清单

## 阶段 5: 版本 bump 与产物重建

- [x] 将 `plugins/dev-team/package.json` 的 `version` patch bump（`2.10.27` → `2.10.28`）
- [x] 在 `plugins/dev-team` 执行构建（`vp pack` / 仓库既定根构建入口），刷新 `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`
- [x] 实现完成后将 `openspec/todo.md` 第 60 行勾选为 `[x]`

## 阶段 6: 实现自检（不含写测试）

- [x] AC-1：`cursor-plugins/dev-team/hooks/hooks.json` 与 `cursor-home-image/dev-team/hooks.json` 的 `hooks` 下无 `subagentStop` 键（亦非空数组）
- [x] AC-2：`claude-plugins/dev-team/hooks/hooks.json` 仍含 `SubagentStop`，matcher 覆盖两 generator，command 含 `static-check`，`loop_limit` 语义不变
- [x] AC-3：canonical 两条 `subagentStop` 的 `matchers.cursor` 均为 `null`
- [x] AC-4 / AC-5：存在平台档时 Cursor 用 `.cursor.md`；Claude 空 default 嵌入空串；assemble 顺序为 include → env tokens → 断言；环/缺文件/残留 `__INCLUDE:` 会失败
- [x] AC-6：Cursor / cursorHome 两 generator 产物含 `run_static_analysis` 门禁；Claude 对应位置无该步骤
- [x] AC-7：任意 outDir 中不存在 `_fragments/` 目录
