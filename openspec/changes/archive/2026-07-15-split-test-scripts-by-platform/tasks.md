# 任务: split-test-scripts-by-platform

> **变更**: split-test-scripts-by-platform
> **日期**: 2026-07-15

---

## 阶段 1: FrameworkConfig 二级嵌套重构

变更 `plugins/dev-team/bin/src/lib/test-framework.ts`，将 `FrameworkConfig` 接口从扁平字段重构为 `shell`/`cmd` 二级嵌套结构。

- [x] 1.1 重构 `FrameworkConfig` 接口：移除顶层 `test_cmd: string` 和 `coverage_cleanup: string[]`，新增 `shell: { test_execution: string; coverage_cleanup: string[]; mutation_execution?: string }` 和 `cmd: { test_execution: string; coverage_cleanup: string[]; mutation_execution?: string }`，保持 `framework`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `default_glob`, `mutation_framework` 在顶层。`mutation_execution` 使用 `{config}` 占位符指代 Stryker 临时配置路径

- [x] 1.2 重写 `FRAMEWORK_REGISTRY['jest']`：`shell.test_execution` = 原 `test_cmd`，`shell.coverage_cleanup` = 原 `coverage_cleanup`；`cmd.test_execution` = 与 shell 相同；`cmd.coverage_cleanup` = `['coverage', '.nyc_output']`；`shell.mutation_execution` = `cmd.mutation_execution` = `npx stryker run "{config}"`

- [x] 1.3 重写 `FRAMEWORK_REGISTRY['vitest']`：`shell.test_execution` = 原 `test_cmd`，`shell.coverage_cleanup` = 原 `coverage_cleanup`；`cmd.test_execution` = 与 shell 相同；`cmd.coverage_cleanup` = `['coverage', '.nyc_output', 'test-stderr.txt']`；`shell.mutation_execution` = `cmd.mutation_execution` = `npx stryker run "{config}"`

- [x] 1.4 重写 `FRAMEWORK_REGISTRY['vite-plus']`：同 vitest 模式；`shell.mutation_execution` = `cmd.mutation_execution` = `npx stryker run "{config}"`

- [x] 1.5 重写 `FRAMEWORK_REGISTRY['bun']`：`shell.test_execution` = 原 `test_cmd`，`shell.coverage_cleanup` = 原 `coverage_cleanup`；`cmd.test_execution` = 与 shell 相同；`cmd.coverage_cleanup` = `['coverage']`

- [x] 1.6 重写 `FRAMEWORK_REGISTRY['rust']`：`shell.test_execution` = 原链式命令（保持不变）；`cmd.test_execution` = cmd.exe 版链式命令（使用 `&` + `%errorlevel%` + `exit /b`）；`cmd.coverage_cleanup` = `['coverage', 'target/llvm-cov']`

- [x] 1.7 重写 `FRAMEWORK_REGISTRY['node-test']`：`cmd.test_execution` = 与 shell 相同（`node --test --experimental-test-coverage {files}`）；`cmd.coverage_cleanup` = `['coverage']`

- [x] 1.8 重写 `FRAMEWORK_REGISTRY['go']`：`cmd.test_execution` = 与 shell 相同（`go test -json -coverprofile=...`）；`cmd.coverage_cleanup` = `['coverage', 'coverage.out']`

- [x] 1.9 重写 `FRAMEWORK_REGISTRY['pytest']`：`shell.test_execution` = 原链式命令；`cmd.test_execution` = cmd.exe 版链式命令；`cmd.coverage_cleanup` = `['.coverage', 'htmlcov']`

- [x] 1.10 确认 `getFrameworkConfig()` 返回类型自动适配新接口（函数签名不变，无额外修改）

---

## 阶段 2: TestPlan Schema 重构

变更 `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts`，使 `script` 字段支持双脚本对象。

- [x] 2.1 将 `testPlanSchema` 中 `script` 字段从 `z.string()` 改为 `z.object({ shell: z.string().describe('...'), cmd: z.string().describe('...') })`，更新其 describe 文案指向"平台特定执行脚本"

- [x] 2.2 ~~确认 `test_cmd` 字段保持 `z.string().optional()`（向后兼容，指向 `shell.test_execution`）~~ → 决定完全移除 `test_cmd` 和 `coverage_cleanup` 字段（D2 完整实现）；`script` 字段已更新为 `z.object({ shell, cmd })`

---

## 阶段 3: 脚本生成逻辑拆分

变更 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts`，将 `generateScript()` 拆分为两个平台专用函数，并更新 `buildPlanFromMappings()`。

- [x] 3.1 将 `generateScript()` 重命名为 `generateShellScript()`，修改其内部逻辑：从 `frameworkConfig.shell` 读取 `test_execution` 和 `coverage_cleanup`，`cd` 命令不变，`rm -rf` 命令不变，输出仍使用 `\n` 拼接

- [x] 3.2 新增 `generateCmdScript()` 函数：从 `frameworkConfig.cmd` 读取 `test_execution` 和 `coverage_cleanup`，使用 `cd /d <dir>` 代替 `cd <dir>`，对每个 cleanup item 输出 `if exist <item> rmdir /s/q <item> >nul 2>nul` 代替 `rm -rf`，输出使用 `\r\n` 拼接

- [x] 3.3 更新 `generateShellScript()` 的类型守卫（`typeof test_execution !== 'string'`, `Array.isArray(coverage_cleanup)`）以匹配新的子对象属性路径

- [x] 3.4 在 `buildPlanFromMappings()` 中，分别调用 `generateShellScript(directory, config)` 和 `generateCmdScript(directory, config)`，将结果构造为 `script: { shell: generateShellScript(...), cmd: generateCmdScript(...) }` 对象

- [x] 3.5 ~~在 `buildPlanFromMappings()` 中更新 `test_cmd` 字段来源：从 `config.shell.test_execution` 读取（保持向后兼容）~~ → 改为完全移除 `test_cmd` 和 `coverage_cleanup` 字段，与 D2 设计决策一致

---

## 阶段 4: Test Runner 平台感知选择

变更 `plugins/dev-team/bin/src/lib/test-runner.ts`，使 `buildTestCommand()` 和 `resolveShell()` 根据平台选择正确的脚本和 shell。

- [x] 4.1 更新 `buildTestCommand()`：在 Windows 上（`process.platform === 'win32'` 且 `!process.env.SHELL`）选择 `entry.script.cmd`，否则选择 `entry.script.shell`。对选中的脚本执行 `substitutePlaceholders()`

- [x] 4.2 更新 `resolveShell()`：Windows 上返回 `process.env.SHELL || process.env.COMSPEC || 'cmd.exe'`，Unix 上返回 `undefined`

- [x] 4.3 改造 `executeStrykerMutation()`：不再硬编码 `npx stryker run "${normalizedConfigPath}"`，改为从 `getFrameworkConfig(entry.framework)` 获取平台对应的 `mutation_execution` 模板（Windows 优先 `cmd.mutation_execution`，fallback 到 `shell.mutation_execution`），用 `normalizedConfigPath` 替换模板中的 `{config}` 占位符

---

## 阶段 5: 测试文件更新

根据新的接口和类型，更新 3 个测试文件使其通过编译和运行时校验。

- [x] 5.1 更新 `plugins/dev-team/bin/src/lib/test-framework.test.ts`：
  - 更新 `FRAMEWORK_SPEC_EXPECTED` 中 go/node-test/pytest 的预期值以匹配新结构（`shell.test_execution`, `cmd.test_execution`, `shell.coverage_cleanup`, `cmd.coverage_cleanup`）
  - 更新 `ALL_EIGHT` 相关测试中 `Object.keys(result).length` 的预期值（从 8 变为 8，但内容变为 `framework, shell, cmd, coverage_format, coverage_output, coverage_artifacts, default_glob, mutation_framework` —— 不含 `test_cmd`, `coverage_cleanup` 在顶层）
  - 更新所有 `result.test_cmd` 引用为 `result.shell.test_execution`
  - 更新所有 `result.coverage_cleanup` 引用为 `result.shell.coverage_cleanup`

- [x] 5.2 更新 `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`：
  - 更新所有 `typeof entry.script` 和 `typeof result.plan[0].script` 等校验从 `'string'` 改为 `'object'`
  - 更新 `entry.script.length` 校验为 `entry.script.shell.length` + `entry.script.cmd.length`
  - 更新 `entry.script` 内容校验（`toContain('rm -rf ...')` 等）使用 `entry.script.shell`
  - ~~更新 `result.plan[0].test_cmd` 校验来源为 `getFrameworkConfig(...).shell.test_execution`~~ → `test_cmd` 字段已移除，相关测试用例已删除

- [x] 5.3 更新 `plugins/dev-team/bin/src/lib/test-runner.test.ts`：
  - 更新所有手动构造的 mock `entry` 对象中 `script` 字段从 `string` 改为 `{ shell: string; cmd: string }` 对象
  - 确保 `.shell` 和 `.cmd` 内容正确反映对应平台的脚本
  - 更新断言中 `calledCmd` 相关校验引用新的脚本路径

---

## 顺序依赖关系

```
阶段 1 (FrameworkConfig) ──→ 阶段 2 (Schema) ──→ 阶段 3 (Script Gen) ──→ 阶段 4 (Runner)
                                    │                      │                       │
                                    └──────────────────────┴───────────────────────┘
                                          ↓                              ↓
                                    阶段 5.1 (framework test)    阶段 5.3 (runner test)
                                          ↓
                                    阶段 5.2 (detect-fw test)
```

阶段 1-4 需按顺序完成（每个阶段依赖前一阶段的接口/类型定义）。阶段 5 的三部分与阶段 3/4 并行（在 2 完成后即可开始 5.1/5.2；4 完成后即可开始 5.3）。
