# 测试设计: split-test-scripts-by-platform

> **日期**: 2026-07-15

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `FrameworkConfig` 包含 `shell: { test_execution, coverage_cleanup }` 和 `cmd: { test_execution, coverage_cleanup }` 子对象；`FRAMEWORK_REGISTRY` 所有 8 个框架按新结构提供 | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` |
| AC-2 | `testPlanSchema` 中 `script` 为 `z.object({ shell: z.string(), cmd: z.string() })` | 单元测试 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构` |
| AC-3 | `generateShellScript()` 从 `frameworkConfig.shell` 读取，输出与原 `generateScript()` 一致 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- 各框架正向测试` |
| AC-4 | `generateCmdScript()` 从 `frameworkConfig.cmd` 读取，使用 `cd /d`、`rmdir /s/q` 等 cmd.exe 兼容语法 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- cmd.exe 兼容语法` |
| AC-5 | `buildPlanFromMappings()` 返回 `script: { shell: "...", cmd: "..." }` | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `buildPlanFromMappings -- 双脚本输出` |
| AC-6 | `buildTestCommand` Windows 选 `entry.script.cmd`，Unix 选 `entry.script.shell` | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `buildTestCommand -- 平台感知选择` |
| AC-7 | `resolveShell` Windows: `process.env.SHELL \|\| process.env.COMSPEC \|\| 'cmd.exe'`；Unix: `undefined` | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- 平台感知` |
| AC-8 | `npx stryker run` 继承 `resolveShell()` 的平台感知 | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `executeStrykerMutation -- 平台感知` |
| AC-9 | Unix 上所有测试行为与变更前完全一致 | 集成测试 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `CLI 端到端执行 — AC-9 Unix 向后兼容` |
| AC-10 | `process.env.SHELL` 存在时行为不变 | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- Windows SHELL 回退` |

---

## 单元测试

### 用例

#### test-framework.test.ts（AC-1 结构验证）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | `FrameworkConfig.shell.test_execution` 对所有 8 框架为非空字符串 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | `FrameworkConfig.shell.coverage_cleanup` 对所有 8 框架为非空数组 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | `FrameworkConfig.cmd.test_execution` 对所有 8 框架为非空字符串 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | `FrameworkConfig.cmd.coverage_cleanup` 对所有 8 框架为非空数组 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | 简单框架（jest/vitest/vite-plus/bun/node-test/go）的 `cmd.test_execution` 与 `shell.test_execution` 相同 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | rust 框架 `cmd.test_execution` 使用 `&` 链式和 `%errorlevel%` 替代 `; _X=$?` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | pytest 框架 `cmd.test_execution` 使用 `&` 链式和 `%errorlevel%` 替代 `; _X=$?` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | 共享字段（`coverage_format`/`coverage_output`/`coverage_artifacts`/`default_glob`/`mutation_framework`）保持在 `FrameworkConfig` 顶层 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | `shell.coverage_cleanup` 使用 `rm -rf` 兼容路径；`cmd.coverage_cleanup` 使用 `rmdir /s/q` / `del /q/f` 兼容路径 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | `shell.mutation_execution` 对于 jest/vitest/vite-plus 为 `npx stryker run "{config}"` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | `cmd.mutation_execution` 对于 jest/vitest/vite-plus 为 `npx stryker run "{config}"`（与 shell 相同） | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | 无 mutation 框架（bun/node-test/go/rust/pytest）的 `shell.mutation_execution` 和 `cmd.mutation_execution` 均为 undefined | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- shell/cmd 二级嵌套结构` | 正向 | 所有 8 框架不包含旧字段 `test_cmd` 在顶层 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- 未知框架` | 异常 | 未知框架名时抛出 Error | 废弃（已有，验证逻辑不变） |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- 边界测试` | 边界 | 框架名为空字符串 `""` 时抛出 Error | 废弃（已有，验证逻辑不变） |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- 边界测试` | 边界 | 框架名为 null/undefined 时抛出 Error | 废弃（已有，验证逻辑不变） |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- 边界测试` | 边界 | 框架名首尾空白 `"  vitest  "` 时抛出 Error | 废弃（已有，验证逻辑不变） |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- 边界测试` | 边界 | 框架名为超长字符串（>1000 字符）时抛出 Error | 废弃（已有，验证逻辑不变） |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | `getFrameworkConfig -- 边界测试` | 边界 | 框架名含特殊正则字符 `"jest?"` 时抛出 Error（不会误匹配） | 废弃（已有，验证逻辑不变） |

#### test-detect-frameworks.test.ts（AC-3、AC-4、AC-5 双脚本生成）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- shell 脚本正向` | 正向 | vitest 框架生成包含 `npx vitest run` 和 `rm -rf coverage` 的 bash 脚本 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- shell 脚本正向` | 正向 | directory 为 `"plugins/dev-team/bin"` 时生成 `cd plugins/dev-team/bin` 行 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- shell 脚本正向` | 正向 | 链式命令框架（rust/pytest）生成字符串包含 `; _X=$?;` 和 `exit $_X` | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- shell 脚本正向` | 正向 | 所有 8 框架生成的 shell 脚本为非空字符串 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- cmd 脚本正向` | 正向 | vitest 框架生成包含 `npx vitest run` 和 `if exist coverage rmdir /s/q coverage` 的 cmd 脚本 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- cmd 脚本正向` | 正向 | directory 为 `"plugins/dev-team/bin"` 时生成 `cd /d plugins/dev-team/bin` 行（带 `/d` 标志） | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- cmd 脚本正向` | 正向 | 链式命令框架（rust/pytest）生成字符串使用 `& if errorlevel` 模式替代 `; _X=$?;` | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- cmd 脚本正向` | 正向 | 所有 8 框架生成的 cmd 脚本为非空字符串 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- cmd 脚本正向` | 正向 | cmd 脚本使用 `\r\n` 行分隔符 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `buildPlanFromMappings -- 双脚本输出` | 正向 | `buildPlanFromMappings` 返回的 plan 条目的 `script` 为 `{ shell: string, cmd: string }` 对象 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `buildPlanFromMappings -- 双脚本输出` | 正向 | `script.shell` 内容与 `generateShellScript` 对同输入输出一致 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `buildPlanFromMappings -- 双脚本输出` | 正向 | `script.cmd` 内容与 `generateCmdScript` 对同输入输出一致 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- 边界测试` | 边界 | directory 为 `"."` 时不生成 `cd` 行 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- 边界测试` | 边界 | directory 为 `"."` 时不生成 `cd` 行 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- 边界测试` | 边界 | `coverage_cleanup` 为空数组时不生成清理命令行 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- 清理命令边界` | 边界 | directory 含空格路径时生成 `cd /d "path/with spaces"`（带引号） | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- 清理命令边界` | 边界 | `coverage_cleanup` 含路径分隔符时（如 `"target/llvm-cov"`）正确生成 `rmdir /s/q` 命令 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- 异常输入` | 异常 | 输入对象为 null 时抛出 TypeError | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- 异常输入` | 异常 | 输入对象为 undefined 时抛出 TypeError | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- 异常输入` | 异常 | directory 为非字符串类型时抛出 TypeError | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateShellScript -- 异常输入` | 异常 | `test_execution` 为 undefined 时抛出 TypeError | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- 异常输入` | 异常 | directory 为非字符串类型时抛出 TypeError | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `generateCmdScript -- 异常输入` | 异常 | `test_execution` 为 undefined 时抛出 TypeError | 新增 |

#### test-runner.test.ts（AC-6、AC-7、AC-8、AC-9、AC-10 平台感知选择）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `buildTestCommand -- 平台选择（AC-6）` | 正向 | 模拟 `process.platform === 'win32'` 时使用 `entry.script.cmd` 构建命令 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `buildTestCommand -- 平台选择（AC-6）` | 正向 | 模拟 `process.platform === 'linux'` 时使用 `entry.script.shell` 构建命令 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- 平台感知（AC-7）` | 正向 | 模拟 `process.platform === 'win32'` 且无 `SHELL`/`COMSPEC` 时返回 `'cmd.exe'` | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- 平台感知（AC-7）` | 正向 | 模拟 `process.platform === 'win32'` 且 `COMSPEC='C:\\Windows\\System32\\cmd.exe'` 时返回 `COMSPEC` 值 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- 平台感知（AC-7）` | 正向 | 模拟 `process.platform !== 'win32'` 时返回 `undefined` | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- Windows SHELL 回退（AC-10）` | 正向 | 模拟 `process.platform === 'win32'` 且 `SHELL='/usr/bin/bash'` 时返回 bash 路径 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- Windows SHELL 回退（AC-10）` | 正向 | `SHELL` 优先级高于 `COMSPEC`（两者同时存在时返回 `SHELL`） | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `executeStrykerMutation -- 平台感知（AC-8）` | 正向 | `executeStrykerMutation` 从 `FrameworkConfig.cmd.mutation_execution` 读取并替换 `{config}` 占位符 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `executePlanEntry -- 单一命令执行` | 正向 | 入口 `entry.script` 使用新对象结构 `{ shell, cmd }` 时执行命令正常（AC-9） | 废弃 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `executePlanEntry -- 单一命令执行` | 正向 | 入口缺失 `cmd` 字段时 fallback 到 `shell`（向后兼容） | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `buildTestCommand -- 边界` | 边界 | `entry.script` 为 `{ shell: "", cmd: "" }` 空字符串时返回空命令 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `buildTestCommand -- 边界` | 边界 | `entry.script` 为 `null` 或 `undefined` 时函数不崩溃 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- 边界` | 边界 | `process.platform` 为 `'win32'` 且 `SHELL` 与 `COMSPEC` 均未定义时返回 `'cmd.exe'` 默认值 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- 边界` | 边界 | `process.platform` 为 `'darwin'`（macOS）时返回 `undefined`（同 Unix 行为） | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `resolveShell -- 边界` | 边界 | `process.env.SHELL` 为空字符串时按 falsy 处理，回退到 `COMSPEC` 或 `'cmd.exe'` | 新增 |

#### test-detect-frameworks.schema.test.ts（AC-2 Schema 验证 — 新增文件）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 正向 | 包含 `script: { shell: "...", cmd: "..." }` 的 plan 通过 schema 验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 正向 | `script.shell` 和 `script.cmd` 均为多行带换行符的字符串时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 异常 | `script` 为旧格式字符串时 schema 拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 异常 | `script` 缺失 `shell` 字段时 schema 拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 异常 | `script` 缺失 `cmd` 字段时 schema 拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 异常 | `script` 为 `null` 时 schema 拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 边界 | `script` 为 `{ shell: "", cmd: "" }` 空字符串时通过 schema 验证（`z.string()` 允许空值） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 边界 | `script.shell` 为超长字符串（10000 字符）时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testPlanSchema -- script 对象结构（AC-2）` | 边界 | `script` 含多余字段（如 `script.extra`）时通过 passthrough 验证 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | 无 | `getFrameworkConfig` 为纯函数，无外部依赖，无需 mock | 所有测试用例 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | 无（generateShellScript/generateCmdScript 为纯函数） | 纯函数，无外部依赖，直接测试 | 所有 `generateShellScript`/`generateCmdScript` 测试用例 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `child_process.execSync` | 通过 `vi.mock('child_process')` 全局 mock `execSync`，各测试用例通过 `mockExecSync.mockReturnValue()` / `mockImplementation()` 控制返回值/异常 | AC-6/AC-7/AC-8 平台选择测试 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `process.platform` / `process.env` | 通过 `Object.defineProperty(process, 'platform', ...)` 或 `vi.stubEnv` 模拟不同平台和 SHELL/COMSPEC 环境变量 | `resolveShell` 平台感知测试 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | 无 | Zod schema 为纯数据验证逻辑，无需 mock | 所有 schema 测试用例 |

---

## 集成测试

### 用例

#### test_detect_frameworks_plan_output（AC-2、AC-5 Plan 输出结构）

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-2, AC-5 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_output/test_detect_frameworks_plan_output.test.ts` | `test_detect_frameworks — plan 双脚本输出` | 字符串简写 "vitest" 配置时 plan 条目包含 `script: { shell, cmd }` 且通过 schema 校验 | 废弃 |
| AC-2, AC-5 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_output/test_detect_frameworks_plan_output.test.ts` | `test_detect_frameworks — plan 双脚本输出` | `script.shell` 包含 `rm -rf` 和 bash 语法命令 | 新增 |
| AC-2, AC-5 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_output/test_detect_frameworks_plan_output.test.ts` | `test_detect_frameworks — plan 双脚本输出` | `script.cmd` 包含 `rmdir /s/q`、`cd /d` 和 cmd.exe 兼容语法 | 新增 |
| AC-2, AC-5 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_output/test_detect_frameworks_plan_output.test.ts` | `test_detect_frameworks — plan 双脚本输出` | 无 `test.framework` 配置时 plan 为空数组且 detected 均为 unknown（回归） | 废弃 |

#### cli-test-execution-execute（AC-6、AC-9 CLI 端到端）

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-9 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `CLI 端到端执行 — 新 script 结构` | `runTestExecution` 触发 `runTestDetectFrameworks` 获取 plan，plan 包含新 `script: { shell, cmd }` 结构 | 废弃 |
| AC-9 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `CLI 端到端执行 — 新 script 结构` | 对每个 framework plan entry 执行测试命令，`executePlanEntry` 接收到的 `entry.script` 为 `{ shell, cmd }` 对象 | 废弃 |
| AC-6 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `CLI 端到端执行 — 新 script 结构` | `buildTestCommand` 根据平台选择正确 script 且不影响框架测试结果 | 新增 |
| AC-9 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `CLI 端到端执行 — 幂等性` | 相同 plan 重复执行两次产生相同子报告内容（幂等性，使用新 script 结构） | 废弃 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_output/test_detect_frameworks_plan_output.test.ts` | 文件系统 | 通过 `fs.mkdtempSync` + `fs.writeFileSync` 创建临时项目，测试结束后 cleanup。不 mock 文件系统，使用真实临时目录 | 所有测试用例 |
| `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `runTestDetectFrameworks` / `executePlanEntry` | 通过 `vi.mock` mock CLI 内部模块，各测试用例控制返回值 | 所有测试用例 |

---

## 不可测试项

- **AC-9 Unix 向后兼容的运行时行为** -- 测试运行在开发机（Windows）上，无法直接模拟 Unix 平台的文件系统和 shell 行为。通过 `Object.defineProperty(process, 'platform')` mock 平台标识，验证 `resolveShell()` 和 `buildTestCommand()` 的逻辑分支正确性，间接保证 Unix 行为不变。

- **AC-10 `process.env.SHELL` 存在时行为不变** -- 真实 `SHELL` 环境变量配置仅在实际 Git Bash 安装环境下可见。通过 `vi.stubEnv` mock 环境变量验证 `resolveShell()` 返回正确的 shell 路径。

- **cmd.exe 生成脚本在真实 Windows 环境中的端到端执行** -- 脚本字符串的内容正确性通过单元测试验证（检查 `cd /d`、`rmdir /s/q`、`%errorlevel%` 等语法模式），但 cmd.exe 执行属于 Executor Agent 的环境，不在当前变更的测试范围内。

- **TestPlan.script 字段 TypeScript 类型约束** -- 编译时类型检查由 TypeScript 编译器保证，无法通过运行时测试直接验证。通过 Schema 测试（`test-detect-frameworks.schema.test.ts`）和集成测试（验证 `runTestDetectFrameworks` 输出始终包含 `script: { shell, cmd }`）间接确保接口行为正确。

- **`test-detect-frameworks.schema.test.ts` 为新增文件** -- `test_resolve_paths` 工具返回该路径作为 `test-detect-frameworks.schema.ts` 的 colocated 单元测试文件，该文件当前不存在，将在本变更中新增。若选择将 schema 验证集成到现有 `test-detect-frameworks.test.ts` 中，则无需创建此文件。
