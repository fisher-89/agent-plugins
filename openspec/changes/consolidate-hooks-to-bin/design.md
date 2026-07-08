# 设计: 将 hooks 脚本迁入 bin TS 构建体系

> **变更**: consolidate-hooks-to-bin
> **日期**: 2026-07-07

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| hooks 入口 | 接收 subcommand 参数，分派到 protect-files / static-check 子命令处理函数 | `bin/src/hooks.ts` | 子命令模块 | TypeScript |
| protect-files 子命令 | 从 stdin 读取 PreToolUse 事件 JSON，检查文件路径是否匹配保护规则，输出 allow/deny 决策 | `bin/src/hooks.ts`（runProtectFiles 函数） | `lib/glob.ts`（`matchGlob`）、`lib/config.ts`（`readConfig`） | TypeScript |
| static-check 子命令 | 从 stdin 读取 SubagentStop 事件 JSON，进程内调用 `runStaticAnalysis()`，输出 block/允许结果 | `bin/src/hooks.ts`（runStaticCheck 函数） | `commands/run-static-analysis.ts`（`runStaticAnalysis`） | TypeScript |
| vite 构建 | 为 hooks 入口生成独立 CJS bundle | `bin/vite.config.ts` | vite-plus | TypeScript / vite-plus |
| hooks.json 配置 | 定义 hook 事件与 command 的绑定关系，command 路径指向新的 bundle | `hooks/hooks.json` | 无（JSON 配置） | JSON |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/hooks.ts` | hooks 单入口文件，含 `protect-files` 和 `static-check` 两个子命令的完整实现 |

<!-- 新增文件含所有原本在 hooks/scripts/*.mjs 中的业务逻辑迁移，以及子命令调度逻辑 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/vite.config.ts` | `pack` 数组新增 `{ name: 'hooks', entry: 'src/hooks.ts', outputOptions: { file: 'dev-team-hooks.cjs' } }` 打包项；`OUTPUT_FILE_NAMES` 新增 `'dev-team-hooks.cjs'` | 新增 hooks bundle 构建配置 |
| `plugins/dev-team/hooks/hooks.json` | 所有 `command` 字段的值从 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-files.mjs"` 改为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" protect-files`，从 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"` 改为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" static-check` | 指向新的 hooks bundle 子命令 |
| `plugins/dev-team/.claude-plugin/plugin.json` | `version` 从 `2.8.3` 递增到 `2.8.4` | 插件版本 patch bump |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/hooks/scripts/protect-files.mjs` | 功能已迁入 `bin/src/hooks.ts`，不再需要 |
| `plugins/dev-team/hooks/scripts/static-check.mjs` | 功能已迁入 `bin/src/hooks.ts`，不再需要 |

<!-- hooks/scripts/ 下的 .test.mjs 文件在 proposal.md 列为测试文件，按约束不列入变更清单 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `runProtectFiles` | `bin/src/hooks.ts` | 新增 | `function runProtectFiles(): void` | protect-files 子命令入口。读取 stdin 的 PreToolUse JSON，使用 `matchGlob` 进行路径保护判定，输出 `{ hookSpecificOutput: { hookEventName: "PreToolUse", permissionDecision: "allow"|"deny"[, permissionDecisionReason: string] } }` |
| `runStaticCheck` | `bin/src/hooks.ts` | 新增 | `function runStaticCheck(): void` | static-check 子命令入口。读取 stdin 的 SubagentStop JSON 提取 workspaceRoot，直接调用 `runStaticAnalysis({ projectRoot })` 获取 exit code，输出 `{}`（exit 0）或 `{ decision: "block", reason: string }`（exit 非0） |
| `main` | `bin/src/hooks.ts` | 新增 | `function main(): void` | 顶层入口函数。解析 `process.argv[2]` 为子命令名，分派到 `runProtectFiles` 或 `runStaticCheck`，未知命令输出错误到 stderr 并 exit(1)。被 CJS bundle 的顶层调用 |

<!--
  protect-files 中的内部辅助函数（isProtected、buildDenyReason、detectBashWrite、detectPowerShellWrite、parseInput等）
  为模块私有函数（无 export），不列入公共 API。
-->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ProtectedPattern` | `bin/src/hooks.ts` | interface（新增） | `{ glob: string; reason?: string }` — 保护模式，其中 `glob` 传递给 `matchGlob` 进行路径匹配 |
| `ToolDecision` | `bin/src/hooks.ts` | interface（新增） | `{ decision: "allow" \| "deny"; reason?: string }` — 写入保护检查的判定结果 |
| `ProtectionResult` | `bin/src/hooks.ts` | interface（新增） | `{ matched: boolean; matchedGlob?: string; reason?: string }` — `isProtected` 返回的路径匹配结果 |

<!-- 无新增或修改的 enum 或 class -->

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `hooks.PreToolUse[0..2].hooks[0].command` | `hooks/hooks.json` | 修改 | — | 从 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-files.mjs"` 改为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" protect-files` |
| `hooks.SubagentStop[0..1].hooks[0].command` | `hooks/hooks.json` | 修改 | — | 从 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"` 改为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" static-check` |
| `version` | `.claude-plugin/plugin.json` | 修改 | `2.8.4` | patch bump from `2.8.3` |

<!-- hooks.json 的 JSON 结构（嵌套层级、matcher、loop_limit 等）不修改 -->

---

## 数据模型

本变更不引入新的持久化数据模型。protect-files 使用的写入保护配置（`openspec/config.json.write_protection`）由 `lib/config.ts` 的 `readConfig` 读取，数据模型由 `lib/schemas/config/config.schema.ts` 中的 Zod schema 定义，不在本变更范围内修改。

---

## 路由/API 设计

<!-- 本变更为 hooks 内部重构，不涉及 HTTP API，此节省略 -->

---

## 依赖

### 运行时依赖

- `picomatch` — 已存在于 `bin/package.json`，`lib/glob.ts` 使用。本变更通过复用 `matchGlob` 间接引入，不新增依赖
- 无新增运行时依赖

### 构建/测试依赖

- `vite-plus` — 已存在于 `bin/`，hooks 打包项使用其 `defineConfig`
- 无新增构建/测试依赖

---

## 待决问题

- `runStaticAnalysis` 函数将 static_analysis 命令的输出写入 `process.stderr`，static-check 子命令需要在进程内捕获该输出以构建 `reason` 字段。方案：在 `runProtectFiles` 调用前临时替换 `process.stderr.write` 为收集函数，调用后恢复。此方案在单次执行的 hook 场景下足够可靠，但若有更优雅的改造（如让 `runStaticAnalysis` 返回输出字符串而非写入 stderr），则可选但需确认不修改 `run-static-analysis.ts` 的要求是否严格。当前设计按约束不修改该文件。
