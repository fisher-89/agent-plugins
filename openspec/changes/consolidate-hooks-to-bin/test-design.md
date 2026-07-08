# 测试设计: consolidate-hooks-to-bin

> **日期**: 2026-07-07

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `dev-team-hooks.cjs` 可通过 `node .../dev-team-hooks.cjs protect-files` 和 `node .../dev-team-hooks.cjs static-check` 调用，分别输出预期 JSON | 单元测试 | `plugins/dev-team/bin/src/hooks.test.ts` | hooks 子命令调度 |
| AC-2 | 对同样的 stdin JSON 输入，`dev-team-hooks.cjs protect-files` 输出与原有 `protect-files.mjs` 输出一致 | 单元测试 | `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 |
| AC-3 | `hooks.ts` 中不包含手写 `globToRegex` 实现，而是 `import { matchGlob } from './lib/glob'` | 单元测试 | `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 复用 picomatch |
| AC-4 | `static-check` 子命令直接调用 `runStaticAnalysis()` 而非 `spawnSync`，不产生子进程 | 单元测试 | `plugins/dev-team/bin/src/hooks.test.ts` | static-check 进程内调用 |
| AC-5 | `dev-team-hooks.cjs static-check` 对 CLI exit code 0/非0 的输出行为与原有 `static-check.mjs` 一致 | 单元测试 | `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 |
| AC-6 | `npm run build` 后 `bin/dev-team-hooks.cjs` 文件存在且为有效 CJS bundle | 集成测试 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 |
| AC-7 | `hooks/hooks.json` 中所有 `command` 字段指向 `bin/dev-team-hooks.cjs` 而非 `hooks/scripts/*.mjs` | 集成测试 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 |
| AC-8 | `hooks/scripts/` 下无 `protect-files.mjs`、`static-check.mjs` 及其 `.test.mjs` | 集成测试 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 |
| AC-9 | `vitest run` 执行 hooks 相关测试全部通过 | 不可测试 | — | — |
| AC-10 | `plugin.json` version 从 `2.8.3` 递增（patch bump） | 集成测试 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 |

---

## 单元测试

### 用例

#### 测试文件: `plugins/dev-team/bin/src/hooks.test.ts`

##### hooks 子命令调度 (AC-1)

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/hooks.test.ts` | hooks 子命令调度 | 正向 | `process.argv[2] = 'protect-files'` 时 main 调用 `runProtectFiles` 并输出 allow/deny JSON | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | hooks 子命令调度 | 正向 | `process.argv[2] = 'static-check'` 时 main 调用 `runStaticCheck` 并输出 block/允许 JSON | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | hooks 子命令调度 | 正向 | `runProtectFiles` 输出格式为 `{ hookSpecificOutput: { hookEventName, permissionDecision } }` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | hooks 子命令调度 | 异常 | 未知子命令 `process.argv[2] = 'unknown'` 时 stderr 输出错误信息并 `process.exit(1)` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | hooks 子命令调度 | 异常 | `process.argv[2]` 未定义时 stderr 输出 help/error 并 `process.exit(1)` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | hooks 子命令调度 | 边界 | 顶层 try-catch 包裹整个处理流程，任何未捕获异常输出 `{ decision: "block", reason }` 而非抛到框架 | 新增 |

##### protect-files 功能等价迁移 (AC-2)

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `parseInput` 解析 Write 受保护文件路径返回 `{ decision: "deny", reason }`，与原有 protect-files.mjs 输出一致 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `parseInput` 解析 Edit 受保护文件路径返回 `{ decision: "deny", reason }` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectBashWrite` — `>` 重定向到 eval.json 返回 deny（等价移植原 `protect-files.test.mjs`） | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectBashWrite` — `>>` 追加到 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectBashWrite` — `tee` 写入 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectBashWrite` — heredoc 写入 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectBashWrite` — `>&` 重定向到 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectBashWrite` — `>|` noclobber 重定向到 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `Set-Content` 写入 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `Out-File` 写入 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `Add-Content` 追加 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `Export-Csv` 写入 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `Export-CliXml` 写入 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `Tee-Object` 写入 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `>` 重定向到 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `>>` 追加到 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `*>` 合并流到 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `[System.IO.File]::WriteAllText` 写入 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `detectPowerShellWrite` — `[System.IO.File]::AppendAllText` 追加 eval.json 返回 deny | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `outputAllow` 返回含 `permissionDecision: "allow"` 的有效 JSON（等价移植） | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `outputDeny(reason)` 返回含 `permissionDecision: "deny"` 和 reason 的有效 JSON | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `buildDenyReason` — 自定义 reason 含 `%s` 和 `%t` 占位符替换 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 正向 | `extractChangeName` 从 `openspec/changes/my-feature/eval.json` 提取 `my-feature` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 异常 | `parseInput` 空字符串输入返回 `{ decision: "allow" }` (fail-open) | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 异常 | `parseInput` 无效 JSON 输入返回 `{ decision: "allow" }` (fail-open) | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 异常 | `parseInput` 有效 JSON 缺失 `tool_name` 返回 `{ decision: "allow" }` (fail-open) | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 异常 | `parseInput` Write 工具缺失 `tool_input.file_path` 返回 `{ decision: "allow" }` (fail-open) | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 异常 | `parseInput` 未知工具名称返回 `{ decision: "allow" }` (fail-open) | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 异常 | `detectBashWrite` 空命令字符串返回 `{ decision: "allow" }` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 异常 | `detectPowerShellWrite` 空命令字符串返回 `{ decision: "allow" }` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `isProtected` filePath 为空字符串/undefined/null 时返回 `{ matched: false }` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `isProtected` patterns 为空数组时返回 `{ matched: false }` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `buildDenyReason` filePath/toolName 为空字符串时占位符替换仍安全 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `buildDenyReason` pattern 为 null 时使用默认回退文案 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `buildDenyReason` reason 含特殊字符（换行、引号、反斜杠、emoji）时序列化可 JSON.parse | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `detectBashWrite` 含 `->` 但无写入操作时返回 allow | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `detectBashWrite` python/node 命令豁免返回 allow | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `detectPowerShellWrite` python/node 命令豁免返回 allow | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `detectPowerShellWrite` 命令字符串 eval.json 出现在非路径上下文中不应误报 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `extractChangeName` Windows 反斜杠路径提取 change name | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 功能等价迁移 | 边界 | `extractChangeName` 路径不含 openspec/changes/ 时返回空字符串 | 新增 |

##### protect-files 复用 picomatch (AC-3)

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 复用 picomatch | 正向 | `isProtected` 使用 `pattern.match`（由 `matchGlob` 编译）进行匹配，对 eval.json 模式返回 `matched: true` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 复用 picomatch | 正向 | 用户配置自定义 glob 模式后 `isProtected` 正确匹配对应路径 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 复用 picomatch | 边界 | 超长路径前缀 + eval.json 仍应匹配 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 复用 picomatch | 边界 | Windows 反斜杠路径归一化后匹配 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 复用 picomatch | 边界 | 路径含特殊字符（空格、括号、Unicode）时 glob 匹配正确 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | protect-files 复用 picomatch | 代码审查 | hooks.ts 源文件不包含 `globToRegex` 函数定义，而是 `import { matchGlob } from './lib/glob'` | 新增 |

##### static-check 进程内调用 (AC-4)

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 进程内调用 | 正向 | `runStaticCheck` 直接调用 `runStaticAnalysis()` 而非通过 `spawnSync` 创建子进程 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 进程内调用 | 正向 | `runStaticCheck` 内部不包含 `import { spawnSync }` 或 `execFileSync` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 进程内调用 | 正向 | `runStaticCheck` 调用 `runStaticAnalysis` 时传递从 stdin 解析的 workspaceRoot | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 进程内调用 | 异常 | `runStaticAnalysis` 抛异常时 `runStaticCheck` 捕获并输出 `{ decision: "block", reason }` | 新增 |

##### static-check 功能等价迁移 (AC-5)

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 正向 | `formatOutput` CLI exit 0 时应返回 `{}`，与原有 `static-check.mjs` 一致 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 正向 | `formatOutput` CLI exit 非 0 时应返回 `{ decision: "block", reason }` 且 reason 含错误信息 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 正向 | `formatOutput` 应将 stdout 与 stderr 合并进 reason | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 正向 | `parseWorkspaceRoot` 从合法 event JSON 提取 `workspace_roots[0]` | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 异常 | `parseWorkspaceRoot` `workspace_roots` 为空数组时返回 null | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 异常 | `parseWorkspaceRoot` 无 `workspace_roots` 字段时返回 null | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 异常 | `parseWorkspaceRoot` 非法 JSON 时返回 null 且不抛异常 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 边界 | `formatOutput` CLI 输出含特殊字符时 `JSON.stringify` 可解析 | 新增 |
| `plugins/dev-team/bin/src/hooks.test.ts` | static-check 功能等价迁移 | 边界 | `handleMissingCli` CLI 文件不存在时返回 `{ decision: "block" }` 且 reason 含路径 | 新增 |

#### 测试文件: `plugins/dev-team/bin/src/vite.config.test.ts`

##### hooks 构建配置

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/vite.config.test.ts` | hooks 构建配置 | 正向 | `pack` 数组包含 `{ name: 'hooks', ... }` 打包项 | 新增 |
| `plugins/dev-team/bin/src/vite.config.test.ts` | hooks 构建配置 | 正向 | hooks 打包项 `entry` 为 `'src/hooks.ts'` | 新增 |
| `plugins/dev-team/bin/src/vite.config.test.ts` | hooks 构建配置 | 正向 | hooks 打包项 `outputOptions.file` 为 `'dev-team-hooks.cjs'` | 新增 |
| `plugins/dev-team/bin/src/vite.config.test.ts` | hooks 构建配置 | 正向 | hooks 打包项 `format` 为 `'cjs'`，`platform` 为 `'node'` | 新增 |
| `plugins/dev-team/bin/src/vite.config.test.ts` | hooks 构建配置 | 正向 | `OUTPUT_FILE_NAMES` 包含 `'dev-team-hooks.cjs'` 和 `'dev-team-config.schema.json'` | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/hooks.test.ts` | `process.argv` | 在测试执行前替换 `process.argv` 数组，测试后恢复 | hooks 子命令调度（AC-1） |
| `plugins/dev-team/bin/src/hooks.test.ts` | `process.stdout.write` / `process.stderr.write` | 使用 `vi.spyOn(process.stdout, 'write')` 捕获输出；`vi.spyOn(process.stderr, 'write')` 捕获错误信息 | hooks 子命令调度（AC-1）、protect-files 输出格式（AC-2） |
| `plugins/dev-team/bin/src/hooks.test.ts` | `process.exit` | 使用 `vi.spyOn(process, 'exit').mockImplementation(() => { throw ... })` 捕获 exit code | hooks 子命令调度错误处理（AC-1） |
| `plugins/dev-team/bin/src/hooks.test.ts` | `process.stdin` (fd 0) | 使用 `vi.spyOn(fs, 'readFileSync').mockReturnValue(stdinJson)` 模拟 stdin 输入 | protect-files 输入解析（AC-2）、static-check 输入解析（AC-5） |
| `plugins/dev-team/bin/src/hooks.test.ts` | `readConfig` / `runStaticAnalysis` 等外部模块 | 使用 `vi.mock('../lib/config')` 和 `vi.mock('../commands/run-static-analysis')` 隔离文件系统调用 | protect-files 模式加载（AC-2）、static-check 进程内调用（AC-4） |
| `plugins/dev-team/bin/src/hooks.test.ts` | `import.meta.resolve` | 使用 `vi.mock` 或直接注入 mock 返回值 | static-check 路径解析（AC-5） |
| `plugins/dev-team/bin/src/vite.config.test.ts` | vite-plus `defineConfig` | 导入配置模块，检查返回对象属性，不做静态类型验证 | hooks 构建配置验证 |

---

## 集成测试

### 用例

#### 测试文件: `plugins/dev-team/bin/__tests__/build/build.test.ts`

##### hooks 构建物生成 (AC-6, AC-7, AC-8, AC-10)

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-6 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `npm run build`（`vp pack`）后 `bin/dev-team-hooks.cjs` 文件存在 | 新增 |
| AC-6 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `dev-team-hooks.cjs` 文件非空（大于 0 字节） | 新增 |
| AC-6 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `node dev-team-hooks.cjs` 不传参时 stderr 输出错误信息，不产生未捕获异常 | 新增 |
| AC-6 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `node dev-team-hooks.cjs protect-files` 可执行且输出格式为 JSON | 新增 |
| AC-6 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `node dev-team-hooks.cjs static-check` 可执行且输出格式为 JSON | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `hooks/hooks.json` 中 PreToolUse 所有 command 指向 `dev-team-hooks.cjs protect-files` | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `hooks/hooks.json` 中 SubagentStop 所有 command 指向 `dev-team-hooks.cjs static-check` | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `hooks/hooks.json` 中无 command 引用 `hooks/scripts/protect-files.mjs` | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `hooks/hooks.json` 中无 command 引用 `hooks/scripts/static-check.mjs` | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `hooks/scripts/protect-files.mjs` 不存在 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `hooks/scripts/static-check.mjs` 不存在 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `hooks/scripts/protect-files.test.mjs` 不存在 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `hooks/scripts/static-check.test.mjs` 不存在 | 新增 |
| AC-10 | `plugins/dev-team/bin/__tests__/build/build.test.ts` | hooks 构建物生成 | `plugin.json` version 为 `2.8.4`（patch bump from `2.8.3`） | 新增 |

#### 测试文件: `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts`

##### hooks 子命令端到端执行

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1 | `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | hooks 子命令端到端执行 | `node dev-team-hooks.cjs protect-files` 传入合法 Write 工具 stdin，输出 `permissionDecision: "allow"` | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | hooks 子命令端到端执行 | `node dev-team-hooks.cjs protect-files` 传入对受保护文件 Write 的 stdin，输出 `permissionDecision: "deny"` | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | hooks 子命令端到端执行 | `node dev-team-hooks.cjs protect-files` 传入 Bash eval.json 写入命令，输出 `permissionDecision: "deny"` | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | hooks 子命令端到端执行 | `node dev-team-hooks.cjs protect-files` 空 stdin 返回 `permissionDecision: "allow"` (fail-open) | 新增 |
| AC-5 | `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | hooks 子命令端到端执行 | `node dev-team-hooks.cjs static-check` 传入合法 SubagentStop stdin，CLI exit 0 时输出 `{}` | 新增 |
| AC-5 | `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | hooks 子命令端到端执行 | `node dev-team-hooks.cjs static-check` 传入合法 stdin，CLI exit 非0 时输出 `{ decision: "block", reason }` | 新增 |
| AC-6 | `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | hooks 子命令端到端执行 | `dev-team-hooks.cjs` 为有效 CJS 文件，`require()` 可加载且不抛语法错误 | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | hooks 子命令端到端执行 | `node bin/dev-team-hooks.cjs protect-files` 通过 `CLAUDE_PROJECT_ROOT` 或 fallback 正确读取配置文件 | 新增 |

> **注意**: 已有集成测试 `protect-files-regression` 和 `static-check-hook-e2e` 直接引用旧 `.mjs` 脚本。本变更实施后，这两个旧测试文件将被废弃并删除，其测试场景等价移植到新的 `hooks.test.ts`（单元测试）和 `__tests__/hook-execution/hook-execution.test.ts`（集成测试）中。删除的文件包括：
> - `plugins/dev-team/hooks/scripts/protect-files.test.mjs` — 废弃
> - `plugins/dev-team/hooks/scripts/static-check.test.mjs` — 废弃
> - `plugins/dev-team/bin/__tests__/protect-files-regression/protect-files-regression.test.ts` — 废弃
> - `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` — 废弃

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/build/build.test.ts` | 文件系统 | 使用 `fs.existsSync` / `fs.statSync` 检查构建产物是否存在；使用 `JSON.parse(fs.readFileSync(...))` 检查 hooks.json 和 plugin.json 内容 | hooks 构建物生成（AC-6, AC-7, AC-8, AC-10） |
| `plugins/dev-team/bin/__tests__/build/build.test.ts` | `execFileSync` | 使用 `execFileSync`（或 `execSync`）执行 `node dev-team-hooks.cjs <subcommand>` 并捕获 stdout/stderr | hooks bundle 调用验证（AC-6） |
| `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | `execFileSync` | 通过 `execFileSync(process.execPath, ['bin/dev-team-hooks.cjs', subcommand], { input: stdinJson })` 模拟 hook 运行时调用 | hooks 子命令端到端执行 |
| `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | 临时项目目录 | 使用 `fs.mkdtempSync` 创建临时项目，写入配置，设置 `CLAUDE_PROJECT_ROOT` 环境变量控制配置读取 | protect-files 配置驱动执行 |
| `plugins/dev-team/bin/__tests__/hook-execution/hook-execution.test.ts` | 临时 CLI stub | 在临时目录中创建 mock `dev-team-cli.cjs` stub，控制 `runStaticAnalysis` 的 exit code 和 stderr 输出 | static-check 端到端执行 |

---

## 不可测试项

- **AC-9 (所有测试通过)** — 该条件为整体测试执行的结果验证，非单个测试用例。通过持续集成/CI 的 `vitest run` 命令整体验证，不在 test-design 中映射为独立测试用例。

- **代码审查项 (AC-3 import 验证)** — `hooks.ts` 中是否包含手写 `globToRegex` 实现的检查属于静态代码审查，可通过 ESLint 规则或 Code Review 阶段人工验证。单元测试通过功能等价性（AC-2）间接验证迁移正确性。

---

## 参数类型 — 边界值系统映射

| 参数 | 类型 | 边界值 | 最小值 | 覆盖场景 |
|------|------|--------|--------|----------|
| `process.argv[2]` (子命令) | string | `""`, `"protect-files"`, `"static-check"`, `"unknown"`, undefined | 5 | 子命令调度 AC-1 |
| stdin (protect-files) | string | `""`, `"  "`, `"{not json"`, 合法 Write/Edit/Bash/PowerShell JSON, 缺失字段 JSON | 7+ | 输入解析 AC-2 |
| stdin (static-check) | string | `""`, `"not json"`, `{ workspace_roots: [] }`, `{ workspace_roots: ['/path'] }`, 无 workspace_roots 字段 | 5 | 输入解析 AC-5 |
| file_path | string | `""`, `"openspec/config.json"`, Windows 反斜杠, 超长路径(>400), 含空格/Unicode/特殊字符 | 6+ | 路径匹配 AC-2, AC-3 |
| CLI exit code | number | 0, 1, 2, 127 | 4 | static-check 输出 AC-5 |
| patterns 数组 | array | `[]`, `[单模式]`, `[内置+自定义]` | 3 | 模式加载 AC-2 |
| `buildDenyReason` pattern | object/null | `{ reason: "..." }`, `{ no reason }`, null | 3 | 拒绝文案 AC-2 |
