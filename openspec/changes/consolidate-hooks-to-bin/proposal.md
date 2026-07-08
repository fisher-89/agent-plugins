# 提案: 将 hooks 脚本迁入 bin TS 构建体系

> **变更**: consolidate-hooks-to-bin
> **日期**: 2026-07-07
> **状态**: 草稿

---

## 问题

`plugins/dev-team/hooks/scripts/` 下的独立 `.mjs` hook 脚本与 `plugins/dev-team/bin/src/` TypeScript 构建体系完全隔离，带来以下问题：

1. **代码孤岛**：hooks 与 bin 隔离，无 TypeScript 类型安全，无法共享 bin 中已有的 lib/ 工具函数（如 glob 匹配、config 读取）
2. **重复实现**：`protect-files.mjs` 手写 `globToRegex`，而 `bin/src/lib/glob.ts` 已使用 `picomatch` 第三方库实现相同功能
3. **测试体系分裂**：hooks 使用 `node:test`，bin 使用 `vitest`，两套测试框架增加维护成本
4. **static-check 跨进程调用**：`static-check.mjs` 通过 `spawnSync` 调用 `dev-team-cli.cjs run_static_analysis`，增加进程启动延迟和错误处理复杂度
5. **无统一入口**：新增 hook 需在 `hooks.json` + `hooks/scripts/` 各自添加文件，流程分散

---

## 提案

将 hooks 脚本迁入 bin 的 TypeScript 构建体系，与 MCP/CLI 平级管理。

### 核心方案

1. **单入口 + 子命令调度**：创建 `bin/src/hooks.ts` -> `dev-team-hooks.cjs`，通过子命令 `protect-files` / `static-check` 调度
2. **static-check 进程内调用**：直接 `import { runStaticAnalysis }` 替代 `spawnSync`，消除跨进程开销
3. **protect-files 复用 picomatch**：使用 `bin/src/lib/glob.ts` 的 `matchGlob` 替代手写 `globToRegex`，同时复用 `bin/src/lib/config.ts` 的 `readConfig` 的 Zod schema 验证
4. **测试统一为 vitest**：测试文件迁移到 `bin/src/`，使用 vitest 框架

### 构建配置

- `vite.config.ts` 新增 `{ name: 'hooks', entry: 'src/hooks.ts' }` 打包项
- hooks 单文件 bundle 预期大小与 CLI 类似（约 144K），启动延迟在可接受范围
- `hooks/hooks.json` 的 command 路径统一指向 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" <subcommand>`

---

## 能力

### 修改的能力

- `protect-files-hook` — 将 protect-files hook 从 `hooks/scripts/protect-files.mjs` 迁移到 `bin/src/hooks.ts`，复用 picomatch 和 Zod config 读取
- `static-check-hook` — 将 static-check hook 从 `hooks/scripts/static-check.mjs` 迁移到 `bin/src/hooks.ts`，改为进程内调用 `runStaticAnalysis`

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/hooks.ts` — 新增：hooks 单入口，含 protect-files 和 static-check 子命令
- `plugins/dev-team/bin/vite.config.ts` — 修改：pack 数组新增 hooks 打包项
- `plugins/dev-team/hooks/hooks.json` — 修改：所有 command 路径指向 `dev-team-hooks.cjs` 子命令
- `plugins/dev-team/.claude-plugin/plugin.json` — 修改：version patch bump

### 测试文件

- `plugins/dev-team/bin/src/hooks.test.ts` — 新增：hooks 子命令调度测试
- `plugins/dev-team/bin/src/hooks.protect-files.test.ts` — 新增：protect-files 迁入后的 vitest 测试（等价移植原 `protect-files.test.mjs`）
- `plugins/dev-team/bin/src/hooks.static-check.test.ts` — 新增：static-check 迁入后的 vitest 测试（等价移植原 `static-check.test.mjs`）

### 删除文件

- `plugins/dev-team/hooks/scripts/protect-files.mjs` — 删除：功能已迁入 hooks.ts
- `plugins/dev-team/hooks/scripts/protect-files.test.mjs` — 删除：测试已迁入 vitest
- `plugins/dev-team/hooks/scripts/static-check.mjs` — 删除：功能已迁入 hooks.ts
- `plugins/dev-team/hooks/scripts/static-check.test.mjs` — 删除：测试已迁入 vitest

### 不要修改

- `plugins/dev-team/bin/src/cli.ts` — CLI 入口不涉及 hooks 变更
- `plugins/dev-team/bin/src/mcp.ts` — MCP 入口不涉及 hooks 变更
- `plugins/dev-team/bin/src/commands/run-static-analysis.ts` — static-analysis 命令本身不修改，仅调用方式从 spawnSync 变为进程内 import
- `plugins/dev-team/bin/src/lib/glob.ts`、`config.ts` — 库文件已就绪，不修改
- `plugins/dev-team/hooks/hooks.json` 的 JSON 结构（PreToolUse/SubagentStop 嵌套结构）— 仅改 command 字符串，不改 schema
- `openspec/specs/write-protection-config/` — 写入保护配置 schema 不涉及

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `bin/src/hooks.ts` 新增入口文件 | `dev-team-hooks.cjs` 可通过 `node .../dev-team-hooks.cjs protect-files` 和 `node .../dev-team-hooks.cjs static-check` 调用，分别输出预期 JSON |
| AC-2 | protect-files 子命令功能等价 | 对同样的 stdin JSON 输入，`dev-team-hooks.cjs protect-files` 输出与原有 `protect-files.mjs` 输出一致（permissionDecision 和 reason 值相同） |
| AC-3 | protect-files 复用 picomatch 库 | `hooks.ts` 中不包含手写 `globToRegex` 实现，而是 `import { matchGlob } from './lib/glob'` |
| AC-4 | static-check 子命令进程内调用 | `static-check` 子命令直接调用 `runStaticAnalysis()` 而非 `spawnSync`，不产生子进程 |
| AC-5 | static-check 功能等价 | `dev-team-hooks.cjs static-check` 对 CLI exit code 0/非0 的输出行为与原有 `static-check.mjs` 一致（`{}` 或 `{ decision, reason }`） |
| AC-6 | hooks 构建物生成 | `npm run build` 后 `bin/dev-team-hooks.cjs` 文件存在且为有效 CJS bundle |
| AC-7 | hooks.json command 路径更新 | `hooks/hooks.json` 中所有 `command` 字段指向 `bin/dev-team-hooks.cjs` 而非 `hooks/scripts/*.mjs` |
| AC-8 | 旧 .mjs 文件已删除 | `hooks/scripts/` 下无 `protect-files.mjs`、`static-check.mjs` 及其 `.test.mjs` |
| AC-9 | 所有测试通过 | `vitest run` 执行 hooks 相关测试全部通过 |
| AC-10 | 插件版本已升级 | `plugin.json` version 从 `2.8.3` 递增（patch bump） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| protect-files 高频调用下 bundle 启动延迟 | 每次 Write/Edit/Bash/PowerShell 增加几百毫秒延迟 | 低 | CLI bundle 仅 144K，hooks bundle 预期类似规模；在开发环境实测延迟，若 >500ms 考虑常驻进程方案 |
| picomatch 行为与手写 globToRegex 有细微差异 | 保护规则误判（漏拦截或误拦截） | 低 | 在迁移测试中针对所有现有用例做等价性断言，包括边界情况 |
| static-check 进程内调用错误影响 hook 框架 | Import 错误或未捕获异常导致 SubagentStop hook 崩溃 | 中 | hooks.ts 顶层 try-catch 包裹整个处理流程，任何异常都输出 `{ decision: "block", reason }` 而非抛到框架 |
| hooks.json 路径更新遗漏某个 matcher | 某个 hook 失效 | 低 | 代码审查时逐项核对 hooks.json 所有 command 字段 |
| 测试迁移遗漏测试用例 | 等价回归覆盖率不足 | 低 | 逐条对照原 .test.mjs 的所有 describe/it 用例，确保每条都有对应 vitest 用例 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 入口形态 | 单文件 + 子命令（`hooks.ts` -> `dev-team-hooks.cjs`） | 与 CLI/MCP 构建模式一致，新增 hook 只需加子命令 case | 多入口（每个 hook 一个 bundle）— 增加构建配置复杂度 |
| static-check 调用方式 | 进程内 `import { runStaticAnalysis }` | 消除 spawnSync 延迟，利用已有 TypeScript 类型 | 保持 spawnSync — 不解决代码孤岛问题 |
| glob 匹配库 | 复用 `lib/glob.ts` 的 picomatch | 消除手写实现，类型安全，与 CLI 一致 | 继续手写 globToRegex — 维护两套实现 |
| 测试框架 | 迁移到 vitest | 与 bin 统一测试框架 | 保持 node:test — 测试体系继续分裂 |

### 待决问题

- 无
