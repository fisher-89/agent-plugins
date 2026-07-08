# 实施任务: 将 hooks 脚本迁入 bin TS 构建体系

> **变更**: consolidate-hooks-to-bin

---

## 阶段 1: 创建 hooks.ts 入口骨架

- [x] 在 `plugins/dev-team/bin/src/hooks.ts` 创建入口文件，添加 `main()` 函数解析 `process.argv[2]`，实现子命令分派（`case 'protect-files'` / `case 'static-check'` / default 报错 exit(1)）
- [x] 在 `main()` 顶层包裹 try-catch，确保任何未捕获异常输出 `{ decision: "block", reason: <错误信息> }` 而非崩溃（符合风险缓解措施）

## 阶段 2: 实现 protect-files 子命令（使用 picomatch 替代 globToRegex）

- [x] 在 `hooks.ts` 中定义内部类型 `ProtectedPattern`、`ToolDecision`、`ProtectionResult`
- [x] 从 `lib/glob.ts` 导入 `matchGlob`，从 `lib/config.ts` 导入 `readConfig`；构建内置默认保护模式列表（`openspec/changes/**/eval.json`、`openspec/config.json`）
- [x] 实现 `loadPatterns(projectRoot: string): ProtectedPattern[]` — 读取 config 的 `write_protection` 配置（使用 `readConfig`），与内置默认模式合并。注意：原 `loadConfig` 直接读取原始 JSON，新实现通过 `readConfig`（含 Zod 校验），但 `write_protection` 字段可能不在 Zod schema 中（确认后选择通过 `getValue` 或直接读取原始 JSON 获取 `write_protection.files`）
- [x] 实现 `isProtected(filePath: string, patterns: ProtectedPattern[]): ProtectionResult` — 使用 `matchGlob` 替代 `globToRegex` 进行路径匹配
- [x] 实现 `buildDenyReason(pattern: ProtectedPattern | null, filePath: string, toolName: string): string` — 占位符替换逻辑与原有实现一致（`%s` → filePath, `%t` → toolName）
- [x] 实现 `hasBashWriteOperator(cmd: string): boolean` 和 `extractBashWriteTargets(cmd: string): string[]` — 移植自原 `protect-files.mjs`
- [x] 实现 `detectBashWrite(cmd: string, patterns: ProtectedPattern[]): ToolDecision` — 移植自原 `detectBashWrite`，使用新的 `isProtected` 替代
- [x] 实现 `extractPowerShellWriteTargets(cmd: string): string[]` — 移植自原 `protect-files.mjs`
- [x] 实现 `detectPowerShellWrite(cmd: string, patterns: ProtectedPattern[]): ToolDecision` — 移植自原 `detectPowerShellWrite`，使用新的 `isProtected` 替代
- [x] 实现 `outputAllow(): string` 和 `outputDeny(reason: string): string` — 输出与原有格式完全一致
- [x] 实现 `parseInput(raw: string, patterns: ProtectedPattern[]): ToolDecision` — 移植自原 `parseInput`，对应 Write/Edit/Bash/PowerShell 四种工具的处理
- [x] 实现 `extractChangeName(filePath: string): string` — 移植自原 `extractChangeName`
- [x] 实现导出函数 `runProtectFiles(): void` — 读取 stdin，调用 `loadPatterns` 和 `parseInput`，输出结果 JSON
- [x] 确认 `hooks.ts` 中不包含手写 `globToRegex` 实现，仅通过 `import { matchGlob } from './lib/glob'` 使用 picomatch（满足 AC-3）

## 阶段 3: 实现 static-check 子命令（进程内调用替代 spawnSync）

- [x] 从 `commands/run-static-analysis` 导入 `runStaticAnalysis`
- [x] 实现 `captureStderr(): [() => string, () => void]` 辅助函数 — 临时替换 `process.stderr.write` 收集输出，返回 `[getCaptured, restore]` 函数对
- [x] 实现导出函数 `runStaticCheck(): void` — 读取 stdin，解析 `workspace_roots[0]` 为 projectRoot（兼容原有 `parseWorkspaceRoot` 逻辑），调用 `runStaticAnalysis({ projectRoot })` 并捕获 exit code 和 stderr 输出
- [x] 根据 `runStaticAnalysis` 返回的 exit code 格式化输出：0 → `{}`，非0 → `{ decision: "block", reason: "静态检查未通过，请修复以下错误后重新提交：\n\n" + <捕获的输出> }`
- [x] 确认 `static-check` 子命令中不存在 `spawnSync` 调用（满足 AC-4）

## 阶段 4: 更新构建配置

- [x] 在 `plugins/dev-team/bin/vite.config.ts` 中 `OUTPUT_FILE_NAMES` 数组新增 `'dev-team-hooks.cjs'`
- [x] 在 `plugins/dev-team/bin/vite.config.ts` 的 `pack` 数组新增 hooks 打包项：`{ name: 'hooks', platform: 'node', entry: 'src/hooks.ts', outputOptions: { file: 'dev-team-hooks.cjs', format: 'cjs', minify: true, sourcemap: true, cleanDir: false }, deps: { alwaysBundle: [/.*/] }, dts: false }`
- [x] 执行 `cd plugins/dev-team/bin && npm run build` 验证 `dev-team-hooks.cjs` 文件已生成且为有效 CJS bundle（满足 AC-6）

## 阶段 5: 更新 hooks.json 配置

- [x] 修改 `plugins/dev-team/hooks/hooks.json` 中 `PreToolUse[0].hooks[0].command` 为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" protect-files`
- [x] 修改 `plugins/dev-team/hooks/hooks.json` 中 `PreToolUse[1].hooks[0].command` 为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" protect-files`
- [x] 修改 `plugins/dev-team/hooks/hooks.json` 中 `PreToolUse[2].hooks[0].command` 为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" protect-files`
- [x] 修改 `plugins/dev-team/hooks/hooks.json` 中 `SubagentStop[0].hooks[0].command` 为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" static-check`
- [x] 修改 `plugins/dev-team/hooks/hooks.json` 中 `SubagentStop[1].hooks[0].command` 为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" static-check`
- [x] 逐项核对 `hooks/hooks.json` 中所有 `command` 字段均指向 `bin/dev-team-hooks.cjs`，无遗漏（满足 AC-7）

## 阶段 6: 删除旧的 .mjs 文件

- [x] 删除 `plugins/dev-team/hooks/scripts/protect-files.mjs`
- [x] 删除 `plugins/dev-team/hooks/scripts/static-check.mjs`
- [x] 验证 `hooks/scripts/` 下无 `protect-files.mjs`、`static-check.mjs` 文件残留（满足 AC-8）

## 阶段 7: 更新插件版本

- [x] 将 `plugins/dev-team/.claude-plugin/plugin.json` 中 `version` 从 `2.8.3` 递增为 `2.8.4`（满足 AC-10）
