# 测试设计: tests-array-cwd-config

> **日期**: 2026-07-22

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-2 | `root: "a/b"`, `cwd: ".."` → `directory` 为 `"a"`（相对项目根）；script 的 `cd` 指向该目录 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — plan.directory = absCwd (AC-2) |
| AC-2 | 变异阶段 cwd=absCwd；mutate 源路径相对 absCwd | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — absCwd 变异 (AC-2) |
| AC-2 | Stryker 临时配置以 absCwd 为 rootPath；mutate 相对该根 | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig — absCwd rootPath (AC-2) |
| AC-3 | suite 含 `config` 且框架有 `config_flag` 时 `{config_args}` 展开为 flag + 相对 absCwd 的路径；无 `config` 时为空；无 `config_flag` 却声明 `config` 时 plan/校验失败 | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig — config_flag 与 {config_args} (AC-3) |
| AC-3 | 同上（script 展开与拒绝路径） | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — {config_args} 注入 (AC-3) |
| AC-4 | 缺省 `includes` 时使用框架 `default_glob`（相对 root）；`excludes` 从范围中剔除；检测/路径推导/突变均遵守同一 scope | 单元测试 | `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded — suite excludes (AC-4) |
| AC-4 | 同上（exclude 模式并集） | 单元测试 | `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | getExcludeGlobs — suite excludes (AC-4) |
| AC-4 | 同上（detect scope） | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — suite scope (AC-4) |
| AC-4 | 同上（resolve 扫描根） | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — suite root 扫描 (AC-4) |
| AC-4 | 同上（mutation exclude） | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — suite exclude 变异 (AC-4) |
| AC-4 | 同上（detect / resolve / mutation 串联） | 集成测试 | `plugins/dev-team/bin/__tests__/tests-array-suite-scope/tests-array-suite-scope.test.ts` | suite scope 贯穿 detect/exclude/resolve |
| AC-5 | 各 suite 自带 `coverage`/`mutation`（或 schema 默认）；报告不再依赖顶层全局块 + override 级联 | 单元测试 | `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport — suite 阈值 (AC-5) |
| AC-5 | 同上（plan.mutation_score） | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — suite 阈值 (AC-5) |
| AC-6 | 无配置提示改为引导 `tests` | 单元测试 | `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 无配置提示引导 tests (AC-6) |
| AC-6 | 空 modules 无 plan 时错误文案引导配置 `tests` | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — 错误文案引导 tests (AC-6) |
| AC-6 | MCP `test_detect_frameworks` 工具描述改为 `tests` suite 映射 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — test_detect_frameworks description (AC-6) |
| AC-7 | 更新夹具后，bin 包内既有单测与 `__tests__` 集成场景通过 | 集成测试 | `plugins/dev-team/bin/__tests__/config-driven-auto-scan/config-driven-auto-scan.test.ts` 等既有夹具 | 既有集成夹具迁入 tests[] 后回归 |

---

## 单元测试

### `plugins/dev-team/bin/src/lib/test-framework.ts` -> `plugins/dev-team/bin/src/lib/test-framework.test.ts`

#### 待测功能

- `getFrameworkConfig(framework: string): FrameworkConfig`: 从 `FRAMEWORK_REGISTRY` 返回框架配置；本变更新增 `config_flag`，且 jest/vitest/vite-plus 的 `test_execution` 含 `{config_args}` 占位

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 正向 | jest 的 `config_flag` 为 `"--config"` 且 shell/cmd `test_execution` 均含 `{config_args}` | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 正向 | vitest 的 `config_flag` 为 `"--config"` 且 shell/cmd `test_execution` 均含 `{config_args}` | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 正向 | vite-plus 的 `config_flag` 为 `"--config"` 且 shell/cmd `test_execution` 均含 `{config_args}` | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 正向 | pytest / rust / go / bun / node-test 的 `config_flag` 为 `null` | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 异常 | 未知框架名（非法枚举）抛出 Error | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 异常 | `framework` 为 `undefined`/`null` 强转调用时抛错（非法入参） | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 边界 | `framework` 为空字符串 `""` 时抛错 | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 边界 | `framework` 为超长字符串（>1000 chars）时抛错 | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 边界 | `framework` 含特殊字符（`\n` / emoji）时抛错 | 新增 |
| getFrameworkConfig — config_flag 与 {config_args} (AC-3) | 边界 | 八框架均具备 `config_flag` 字段（`string \| null`），且返回对象为浅拷贝 | 新增 |
| getFrameworkConfig — known frameworks | 正向 | 既有已知框架查找行为保持不变 | 废弃 |
| getFrameworkConfig — unknown framework | 异常 | 未知框架名抛出 Error | 废弃 |
| getFrameworkConfig — edge cases | 边界 | 空字符串 / 非法枚举框架名抛错 | 废弃 |
| getFrameworkConfig — test_execution 模板化 (AC-3) | 正向 | 既有 `{files}` / `{directory}` 占位断言保留；支持 config 的框架额外含 `{config_args}` | 新增 |
| getFrameworkConfig — test_execution 模板化 (AC-3) | 异常 | 未知框架名查询时抛 Error，不返回残缺 `test_execution` 模板 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | `getFrameworkConfig` 为纯注册表查找，不 mock 外部依赖 | 全部 |

---

### `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` -> `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`

#### 待测功能

- `runTestDetectFrameworks(options: TestDetectFrameworksOptions): TestDetectFrameworksResult`: 从 `config.tests[]` 构建 plan；`directory`=absCwd；展开 `{config_args}`；suite scope 匹配；`mutation_score` 取自 suite

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestDetectFrameworks — plan.directory = absCwd (AC-2) | 正向 | `root: "a/b"`, `cwd: ".."` → `plan[0].directory === "a"`，shell script 含 `cd a` | 新增 |
| runTestDetectFrameworks — plan.directory = absCwd (AC-2) | 正向 | 省略 `cwd`（或缺省 `"."`）时 `directory` 等于 `root` | 新增 |
| runTestDetectFrameworks — plan.directory = absCwd (AC-2) | 正向 | `cwd: "nested"` 时 `directory` 为 `root/nested`（POSIX） | 新增 |
| runTestDetectFrameworks — plan.directory = absCwd (AC-2) | 异常 | `root` 指向不存在目录时 plan/错误行为明确（抛错或空 plan，按实现约定断言） | 新增 |
| runTestDetectFrameworks — plan.directory = absCwd (AC-2) | 边界 | `cwd` 为空字符串 `""` 时按实现约定处理（视为 `"."` 或校验失败） | 新增 |
| runTestDetectFrameworks — plan.directory = absCwd (AC-2) | 边界 | `cwd` 为超长相对路径（>1000 chars）不崩溃且 directory 可推导或明确失败 | 新增 |
| runTestDetectFrameworks — plan.directory = absCwd (AC-2) | 边界 | `cwd` 含特殊字符（空格 / Unicode）时 directory 与 script `cd` 正确转义或规范化 | 新增 |
| runTestDetectFrameworks — {config_args} 注入 (AC-3) | 正向 | suite 含 `config: "vite.config.ts"` 且框架为 vite-plus 时，script 在 `{config_args}` 位置展开为 `--config` + 相对 absCwd 的路径 | 新增 |
| runTestDetectFrameworks — {config_args} 注入 (AC-3) | 正向 | suite 无 `config` 时 script 中 `{config_args}` 展开为空（命令与今日无 config 行为一致） | 新增 |
| runTestDetectFrameworks — {config_args} 注入 (AC-3) | 异常 | suite 含 `config` 但框架为 pytest（`config_flag: null`）时 plan 构建失败并给出明确错误 | 新增 |
| runTestDetectFrameworks — {config_args} 注入 (AC-3) | 异常 | suite 含 `config` 但框架为 rust（`config_flag: null`）时 plan 构建失败 | 新增 |
| runTestDetectFrameworks — {config_args} 注入 (AC-3) | 边界 | `config` 为空字符串 `""` 时校验失败或按实现拒绝 | 新增 |
| runTestDetectFrameworks — {config_args} 注入 (AC-3) | 边界 | `config` 为超长路径（>1000）时展开不崩溃或明确失败 | 新增 |
| runTestDetectFrameworks — {config_args} 注入 (AC-3) | 边界 | `cwd: ".."` 且 `config` 相对 root 时，展开路径为相对 absCwd 的 POSIX 相对路径（非整段末尾 append） | 新增 |
| runTestDetectFrameworks — suite scope (AC-4) | 正向 | 省略 `includes` 时使用框架 `default_glob`（相对 root）匹配文件 | 新增 |
| runTestDetectFrameworks — suite scope (AC-4) | 正向 | 显式 `includes` + `excludes` 时仅 in-scope 文件进入 `detected[]` | 新增 |
| runTestDetectFrameworks — suite scope (AC-4) | 正向 | 多 suite 时数组顺序优先（第一个匹配决定 framework） | 新增 |
| runTestDetectFrameworks — suite scope (AC-4) | 异常 | `includes`/`excludes` 含非法 glob 语法时行为明确（不静默匹配全部） | 新增 |
| runTestDetectFrameworks — suite scope (AC-4) | 边界 | `includes: []`（空数组）时无文件匹配进 detected | 新增 |
| runTestDetectFrameworks — suite scope (AC-4) | 边界 | `includes` 为单元素数组时仅匹配该 glob | 新增 |
| runTestDetectFrameworks — suite scope (AC-4) | 边界 | `excludes` 超大列表（大量模式）时匹配仍正确且不超时失控 | 新增 |
| runTestDetectFrameworks — suite 阈值 (AC-5) | 正向 | `plan[].mutation_score` 取自对应 suite 的 `mutation.score`（非全局块） | 新增 |
| runTestDetectFrameworks — suite 阈值 (AC-5) | 正向 | suite 省略 `mutation` 时 `mutation_score` 为 schema 默认 70 | 新增 |
| runTestDetectFrameworks — suite 阈值 (AC-5) | 异常 | suite `mutation.score` 为非法值（`-1` / `>100` / `NaN`）时 schema 解析失败，detect 不产出该 suite plan | 新增 |
| runTestDetectFrameworks — suite 阈值 (AC-5) | 边界 | `mutation.score` 为 `0` 时 plan.mutation_score 为 0 | 新增 |
| runTestDetectFrameworks — suite 阈值 (AC-5) | 边界 | `mutation.score` 为 `100`（上界）时 plan.mutation_score 为 100 | 新增 |
| runTestDetectFrameworks — suite 阈值 (AC-5) | 边界 | `mutation.score` 省略 / `undefined` 时回落 schema 默认 70 | 新增 |
| runTestDetectFrameworks — 空 tests | 边界 | `tests: []` 时 `plan` 为空数组 | 新增 |
| runTestDetectFrameworks — 空 tests | 边界 | 配置仅含旧 `test` 键（无 `tests`）时不产生 suite plan（硬 breaking） | 新增 |
| runTestDetectFrameworks — 空 tests | 异常 | `tests` 元素缺少必填 `root`/`framework` 时配置解析失败，命令侧错误明确 | 新增 |
| detectFrameworks -- glob first-match (AC-4) | 正向 | 旧 `test.framework` + `overrides` 匹配逻辑 | 废弃 |
| runTestDetectFrameworks -- 向后兼容 (AC-11) | 正向 | 旧顶层 framework 短写兼容 | 废弃 |
| detectFrameworks -- exclude 过滤 (AC-3) | 正向 | 旧全局/override exclude 语义 | 废弃 |
| runTestDetectFrameworks -- plan mutation 字段 | 正向 | 全局 mutation + override 级联填充 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `openspec/config.json` 文件系统 | `createTempProject` 写入含 `tests[]` 的临时配置；测试结束 `cleanup()` | plan.directory / config_args / suite scope / 阈值 |
| `getFrameworkConfig` | 不 mock；真实读取 registry（含 `config_flag` / `{config_args}`） | {config_args} 注入 |
| `fs` 源文件树 | `writeFile` 在 temp project 下创建匹配/不匹配 scope 的文件 | suite scope |

---

### `plugins/dev-team/bin/src/lib/test-exclude.ts` -> `plugins/dev-team/bin/src/lib/test-exclude.test.ts`

#### 待测功能

- `isFileExcluded(filePath: string, config: OpenSpecConfig): boolean`: 仅当文件落在某 suite 的 `root` 下且匹配该 suite 的 scoped `excludes` 时返回 `true`
- `getExcludeGlobs(config: OpenSpecConfig): string[]`: 返回所有 suite 拼好的 project-relative exclude 模式并集（去重）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| isFileExcluded — suite excludes (AC-4) | 正向 | 文件在 suite.root 下且匹配 `excludes` 时返回 true | 新增 |
| isFileExcluded — suite excludes (AC-4) | 正向 | 文件匹配 excludes 但不在该 suite.root 下时返回 false（范围隔离） | 新增 |
| isFileExcluded — suite excludes (AC-4) | 正向 | 文件在 root 下但不匹配 excludes 时返回 false | 新增 |
| isFileExcluded — suite excludes (AC-4) | 正向 | 多 suite 各自 excludes 仅作用于对应 root 树 | 新增 |
| isFileExcluded — suite excludes (AC-4) | 异常 | `filePath` 为 `undefined`/`null` 时不抛未捕获异常（返回 false 或明确抛 TypeError，按实现断言） | 新增 |
| isFileExcluded — suite excludes (AC-4) | 异常 | `config.tests` 缺失或非数组时安全回落（false / 不崩溃） | 新增 |
| isFileExcluded — suite excludes (AC-4) | 边界 | `filePath` 为空字符串 `""` 时返回 false | 新增 |
| isFileExcluded — suite excludes (AC-4) | 边界 | `filePath` 为超长路径（>1000）不抛异常且匹配结果确定 | 新增 |
| isFileExcluded — suite excludes (AC-4) | 边界 | `filePath` 含特殊字符（`\n` / 空格 / emoji / Unicode）时匹配正确 | 新增 |
| isFileExcluded — suite excludes (AC-4) | 边界 | `excludes: []` 或省略时返回 false | 新增 |
| isFileExcluded — suite excludes (AC-4) | 边界 | `tests: []` 时返回 false | 新增 |
| isFileExcluded — suite excludes (AC-4) | 边界 | Windows 反斜杠路径归一化为 POSIX 后匹配 | 新增 |
| getExcludeGlobs — suite excludes (AC-4) | 正向 | 单 suite 返回拼成 project-relative 的 exclude 模式 | 新增 |
| getExcludeGlobs — suite excludes (AC-4) | 正向 | 多 suite 返回并集 | 新增 |
| getExcludeGlobs — suite excludes (AC-4) | 异常 | `config` 为 `null`/`undefined` 时不崩溃（返回 `[]` 或明确抛错） | 新增 |
| getExcludeGlobs — suite excludes (AC-4) | 边界 | `excludes` 为单元素数组时返回对应一条 project-relative 模式 | 新增 |
| getExcludeGlobs — suite excludes (AC-4) | 边界 | `excludes` 超大列表（大量模式）时返回完整并集 | 新增 |
| getExcludeGlobs — suite excludes (AC-4) | 边界 | 重复模式去重 | 新增 |
| getExcludeGlobs — suite excludes (AC-4) | 边界 | 无 excludes / 空 tests 时返回 `[]` | 新增 |
| isFileExcluded — 正向 | 正向 | 全局 `test.exclude` / override-level exclude | 废弃 |
| isFileExcluded — 向后兼容 (AC-6) | 正向 | 旧 `test` 节无 exclude 行为 | 废弃 |
| getExcludeGlobs — 正向 | 正向 | 合并全局与 override exclude | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `lib/glob.ts` / picomatch | 不 mock；直接调用 `matchGlob` | 全部 |
| `OpenSpecConfig` | 内存构造 `tests: [{ root, excludes, ... }]` 夹具（不再使用 `test.exclude`） | 全部 |

---

### `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` -> `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts`

#### 待测功能

- `runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult`: 空 modules 按 suite `root` 扫描（不因 `cwd` 上移漏扫/误扫）；错误文案引导配置 `tests`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestResolvePaths — suite root 扫描 (AC-4) | 正向 | `modules: []` 且 suite `root: "pkg/src"`, `cwd: ".."` 时仍扫描 `pkg/src` 下源文件，不扫描 `pkg` 根下无关文件 | 新增 |
| runTestResolvePaths — suite root 扫描 (AC-4) | 正向 | 显式 `includes`/`excludes` 时 unit_tests 仅含 in-scope 源文件 | 新增 |
| runTestResolvePaths — suite root 扫描 (AC-4) | 正向 | 非空 modules 时按 suite scope 过滤后再推导测试路径 | 新增 |
| runTestResolvePaths — suite root 扫描 (AC-4) | 异常 | modules 指向不存在路径时 errors 含对应 path/message，不崩溃 | 新增 |
| runTestResolvePaths — suite root 扫描 (AC-4) | 边界 | modules 为单元素合法路径时返回对应 unit_tests | 新增 |
| runTestResolvePaths — suite root 扫描 (AC-4) | 边界 | modules 为空数组且 suite includes 省略时按 default_glob/显式 includes 语义扫描 | 新增 |
| runTestResolvePaths — suite root 扫描 (AC-4) | 边界 | modules 含超长路径字符串（>1000）时进入 errors 或不崩溃 | 新增 |
| runTestResolvePaths — 错误文案引导 tests (AC-6) | 异常 | `modules: []` 且无 `tests` / `tests: []` 时 errors 提示配置 `tests`（不再提 `test.framework`/`test.overrides`） | 新增 |
| runTestResolvePaths — 错误文案引导 tests (AC-6) | 异常 | 配置仅含旧 `test.overrides`、无 `tests` 时 errors 引导配置 `tests`（硬 breaking） | 新增 |
| runTestResolvePaths — 错误文案引导 tests (AC-6) | 边界 | 错误 message 非空字符串且不含已废弃键名 | 新增 |
| runTestResolvePaths — exclude 过滤 (AC-4) | 正向 | 旧全局/override exclude | 废弃 |
| runTestResolvePaths — 向后兼容 (AC-6) | 正向 | 旧 `test` 节兼容路径 | 废弃 |
| runTestResolvePaths — config-driven 扫描 (AC-1) | 正向 | `test.overrides` 驱动空 modules 扫描 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `openspec/config.json` | `createTempProject` 写入 `tests[]`；`writeFile` 布置源文件树 | suite root 扫描 |
| `runTestDetectFrameworks` | 默认真实调用；仅在需隔离 plan 形状时 `vi.mock` 覆盖返回值 | 错误文案 / 边界 |

---

### `plugins/dev-team/bin/src/lib/test-report.ts` -> `plugins/dev-team/bin/src/lib/test-report.test.ts`

#### 待测功能

- `generateSubReport(framework, result, projectRoot, reportsDir, planDirectory): TestExecutionSubReport`: 覆盖率/变异阈值按匹配 suite 读取
- `generateSummaryReport(subReports, projectRoot, reportsDir): TestExecutionSummaryReport`: coverage/mutation 分组改 suite 维度（报告字段名可仍为 `overrides`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 正向 | suite 自定义 `coverage.lines/branches/functions` 时子报告/汇总使用该阈值 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 正向 | suite 省略 coverage/mutation 时使用 schema 默认 80/70/75 与 mutation 70 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 正向 | 两 suite 不同阈值时汇总按 suite 分组（`overrides` 字段语义为 suite 分组） | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 正向 | 配置无顶层全局 `test.coverage`/`test.mutation` 块时报告仍能给出阈值结论 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 异常 | coverage 产物缺失或 JSON 非法时子报告错误字段明确、不崩溃 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 异常 | suite `coverage.lines` 为 `-1` / 超过 100 时配置侧已拒绝，报告层不读到非法阈值 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 边界 | `coverage.lines/branches/functions` 均为 `0` 时阈值结论按 0 判定 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 边界 | `coverage.lines` 为 `100`（上界）时使用 100 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 边界 | `mutation.score` 为 `0` / `100` 时汇总分组使用该值 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 边界 | `tests: []` 时阈值回退行为明确（不读旧全局块） | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 边界 | `subReports: []`（空数组）时汇总报告结构合法 | 新增 |
| generateSubReport / generateSummaryReport — suite 阈值 (AC-5) | 边界 | `subReports` 为单元素时分组仅一条 | 新增 |
| generateSummaryReport -- coverage threshold (AC-9) | 正向 | 旧全局 + override 级联阈值 | 废弃 |
| generateSubReport / generateSummaryReport -- mutation 块 | 正向 | 旧 mutation overrides 级联 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `openspec/config.json` | temp project 写入多 suite / 缺省阈值配置；或 mock `readConfig` 返回内存 config | suite 阈值 |
| 覆盖率/变异报告文件 | 写入最小合法 coverage-summary / mutation.json 夹具 | 阈值判定与分组 |
| `fs` reportsDir | temp 目录作为 `reportsDir` | 全部写入类断言 |

---

### `plugins/dev-team/bin/src/lib/test-runner.ts` -> `plugins/dev-team/bin/src/lib/test-runner.test.ts`

#### 待测功能

- `executePlanEntry(entry, projectRoot, options?): ExecutionResult`: 变异阶段以 `entry.directory`（absCwd）为 cwd；mutate 源路径相对 absCwd；exclude 走 suite 语义

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| executePlanEntry — absCwd 变异 (AC-2) | 正向 | `entry.directory` 为上级目录时，mutation 子进程 cwd 指向该 absCwd | 新增 |
| executePlanEntry — absCwd 变异 (AC-2) | 正向 | 传入 `resolveStrykerConfig` 的源路径相对 absCwd（非 projectRoot） | 新增 |
| executePlanEntry — absCwd 变异 (AC-2) | 异常 | `entry.directory` 指向不存在目录时执行失败且错误信息明确 | 新增 |
| executePlanEntry — absCwd 变异 (AC-2) | 异常 | 测试命令为空时返回 `Empty test command` 结果，不进入 mutation | 新增 |
| executePlanEntry — absCwd 变异 (AC-2) | 边界 | `entry.directory` 为空字符串时行为明确（失败或回落 projectRoot，按实现断言） | 新增 |
| executePlanEntry — absCwd 变异 (AC-2) | 边界 | `options.timeout` 为 `0` 时超时行为明确 | 新增 |
| executePlanEntry — absCwd 变异 (AC-2) | 边界 | `options.timeout` 为 `-1` / `undefined` 时不崩溃 | 新增 |
| executePlanEntry — absCwd 变异 (AC-2) | 边界 | `options.files` 为 `[]` / 单元素 / 超大列表时命令构建正确 | 新增 |
| executePlanEntry — suite exclude 变异 (AC-4) | 正向 | suite `excludes` 命中的源文件不进入 Stryker mutate 列表 | 新增 |
| executePlanEntry — suite exclude 变异 (AC-4) | 正向 | 未排除源文件正常进入 mutate 列表 | 新增 |
| executePlanEntry — suite exclude 变异 (AC-4) | 异常 | exclude 后源文件列表为空时静默跳过 mutation | 新增 |
| executePlanEntry — suite exclude 变异 (AC-4) | 边界 | `options.mutationDiffFiles: []` 时 mutation 范围为空并跳过 | 新增 |
| executePlanEntry — suite exclude 变异 (AC-4) | 边界 | `options.mutationDiffFiles` 为单元素且被 excludes 命中时跳过 mutation | 新增 |
| executePlanEntry — suite exclude 变异 (AC-4) | 边界 | `options.noMutation: true` 时不调用 resolveStrykerConfig | 新增 |
| executePlanEntry — mutation exclude 过滤 (AC-5) | 正向 | 旧全局/override exclude | 废弃 |
| executePlanEntry — 向后兼容 (AC-6) | 正向 | 旧 `test` 节兼容 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `child_process` / `execCommand` | `vi.mock` 捕获 cwd、命令行与退出码；不真实跑测试 | absCwd 变异 |
| `resolveStrykerConfig` | spy/mock 断言 `rootPath` 与 `sourceFiles` 相对路径形态 | absCwd 变异 |
| `openspec/config.json` / `readConfig` | 内存或 temp 配置提供 suite excludes | suite exclude 变异 |
| 文件系统 | temp project + 最小源文件树 | mutation 路径断言 |

---

### `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` -> `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts`

#### 待测功能

- `resolveStrykerConfig(rootPath, sourceFiles, framework): { configPath, tempDirPath }`: 保证 `mutate` 相对 `rootPath`（absCwd）；临时 `stryker.config.*` 与 `.stryker-tmp` 落在该目录

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| resolveStrykerConfig — absCwd rootPath (AC-2) | 正向 | `rootPath` 为 absCwd 时，生成的 config 位于该目录，`tempDirPath` 为该目录下 `.stryker-tmp` | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 正向 | `sourceFiles` 含相对 projectRoot 的路径时，写入 mutate 的条目被规范为相对 `rootPath` | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 异常 | 不支持的 framework（如 `pytest` / 未知名）抛出 Error | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 异常 | `rootPath` 不存在或不可写时抛文件系统错误 | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 异常 | `framework` 为 `undefined`/`null` 时抛错 | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 边界 | `sourceFiles: []` 时仍生成合法临时配置（mutate 为空数组） | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 边界 | `sourceFiles` 为单元素列表时 mutate 仅一条且相对 rootPath | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 边界 | 超大 sourceFiles 列表均可写入且路径均为相对 rootPath | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 边界 | `rootPath` 为空字符串时行为明确（抛错） | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 边界 | `framework` 为空字符串 / 超长字符串 / 含特殊字符时抛 Unsupported 错误 | 新增 |
| resolveStrykerConfig — absCwd rootPath (AC-2) | 边界 | sourceFiles 条目含空字符串或特殊字符路径时 mutate 规范化或过滤行为明确 | 新增 |
| resolveStrykerConfig -- 临时配置生成 | 正向 | 既有临时配置生成主路径 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | `fs.mkdtempSync` 作为 `rootPath`；读回生成的 JSON 断言 `mutate` | 全部 |
| `crypto.randomBytes` | 不 mock（或固定 seed 若测试不稳定） | 临时文件名 |

---

### `plugins/dev-team/bin/src/commands/test-execution.ts` -> `plugins/dev-team/bin/src/commands/test-execution.test.ts`

#### 待测功能

- `runTestExecution(options: TestExecutionOptions): Promise<number>`: 无 plan 时控制台提示改为引导配置 `tests`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestExecution — 无配置提示引导 tests (AC-6) | 正向 | `plan.length === 0` 时日志包含配置 `tests` 的引导文案，返回 0 | 新增 |
| runTestExecution — 无配置提示引导 tests (AC-6) | 异常 | `runTestDetectFrameworks` 抛错时向上传播或转为非 0 退出码（按既有错误处理约定） | 新增 |
| runTestExecution — 无配置提示引导 tests (AC-6) | 异常 | 配置仅含旧 `test` 键导致空 plan 时仍引导 `tests`，不提及 `test.framework` | 新增 |
| runTestExecution — 无配置提示引导 tests (AC-6) | 边界 | 旧文案 `test.framework` / `test.overrides` 不再出现在日志中 | 新增 |
| runTestExecution — 无配置提示引导 tests (AC-6) | 边界 | `options` 缺省字段（如无 files）时仍能完成空 plan 提示路径 | 新增 |
| runTestExecution -- 正向 (AC-1) | 正向 | 既有执行主路径（fixture 改为 `tests[]`） | 新增 |
| runTestExecution -- 异常 | 异常 | 既有异常路径保持 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `runTestDetectFrameworks` | `vi.mock` 返回空 plan 或含 `tests[]` 语义的 plan | 无配置提示 / 正向执行 |
| `executePlanEntry` / `generateSubReport` / `generateSummaryReport` | mock 返回固定 ExecutionResult / 报告 | 正向执行 |
| `console.log` | spy 断言提示文案 | 无配置提示 |

---

### `plugins/dev-team/bin/src/mcp.ts` -> `plugins/dev-team/bin/src/mcp.test.ts`

#### 待测功能

- `connectToServer(transport: Transport): Promise<McpServer>`: 注册 MCP 工具；本变更将 `test_detect_frameworks` 工具 description 从旧 `test.framework` 表述改为 `tests` suite 映射

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| MCP 注册 — test_detect_frameworks description (AC-6) | 正向 | `listTools` 得到的 `test_detect_frameworks.description` 含 `tests`（suite 映射语义） | 新增 |
| MCP 注册 — test_detect_frameworks description (AC-6) | 正向 | description 非空且仍描述 detect / plan 行为 | 新增 |
| MCP 注册 — test_detect_frameworks description (AC-6) | 异常 | 工具未注册或缺 description 时测试失败（断言存在性） | 新增 |
| MCP 注册 — test_detect_frameworks description (AC-6) | 边界 | description 不再包含已废弃键名 `test.framework` / `test.overrides` | 新增 |
| MCP 注册 — test_detect_frameworks description (AC-6) | 边界 | description 字符串长度合理（非空、非超长垃圾文案） | 新增 |
| MCP 工具调用 — test_detect_frameworks | 正向 | 既有 callTool 返回 `detected` 字段路径保持（夹具配置迁 `tests[]`） | 新增 |
| MCP 工具调用 — test_detect_frameworks | 异常 | 无有效 `tests`（或 `tests: []`）时 callTool 返回空 `detected`/`plan` 或明确错误结构，不抛未处理异常 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `InMemoryTransport` + MCP Client | 真实协议级联通，不 mock McpServer | 全部 |
| `lib/project-root` | `vi.mock` 固定 `getMcpCachedProjectRoot` | 全部 |
| 临时 `openspec/config.json` | `setupTempProject` 写入 `tests[]`（异常路径写空/缺失 `tests`） | 工具调用正向/异常路径 |

---

### suite scope → detect / exclude / resolve → `plugins/dev-team/bin/__tests__/tests-array-suite-scope/tests-array-suite-scope.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/test-exclude.ts` | 按 suite `excludes`（相对 root）判定排除 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 按 suite scope 匹配 detected 文件 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 空 modules 以 suite.root 为扫描根并应用同一 scope |

**关联AC**: AC-4, AC-6, AC-7

**关系描述**:

同一份 `tests[]` 必须让检测、排除与路径推导看到一致的 scope：`under(root) ∩ includesEffective ∩ ¬excludes`。集成层用真实临时文件树串联三个模块，避免单测各自 mock 后 scope 口径漂移。典型出错模式是 `cwd: ".."` 时误用 absCwd 当扫描根、缺省 includes 未落到 `default_glob`，以及 excludes 未限制在对应 root 树。

#### 场景: cwd 上移不改变扫描根

suite `root` 在子目录、`cwd: ".."` 时，`modules: []` 的 resolve 仍只扫描 root 树内源文件。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | root 内源文件进入 unit_tests；root 外同级目录文件不进入 | 新增 |
| 正向 | detect 对显式 files 入参同样遵守 suite scope / excludes | 新增 |
| 异常 | 无有效 `tests` 时 resolve errors 引导配置 `tests` | 新增 |
| 边界 | 省略 includes 时仅匹配框架 default_glob 对应测试文件（或按实现：扫描源文件时显式 includes 才覆盖源码树） | 新增 |

#### 场景: includes/excludes 三方一致

同一配置下，被 excludes 命中的路径在 detect 的 `detected[]`、resolve 的 `unit_tests` 中均不出现。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | excludes 命中文件同时从 detected 与 unit_tests 消失 | 新增 |
| 正向 | includes 收窄后范围外文件不被检测/推导 | 新增 |
| 异常 | includes/excludes 同时为空数组时 detected/unit_tests 为空且不崩溃 | 新增 |
| 边界 | 多 suite 不同 excludes 时互不污染 | 新增 |
| 边界 | excludes 为单元素 / 超大列表时三方结果一致 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 临时文件系统 | 写入 `openspec/config.json`（`tests[]`）与多目录源/测试文件 | 全部场景 |
| 跨进程依赖 | 无；三模块同进程真实调用 | 全部 |

---

### suite 阈值 → 报告分组 → `plugins/dev-team/bin/__tests__/mutation-report-format/mutation-report-format.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 提供 suite 级 coverage/mutation 最终值 |
| `plugins/dev-team/bin/src/lib/test-report.ts` | 按 suite 读取阈值并生成子报告/汇总分组 |

**关联AC**: AC-5, AC-7

**关系描述**:

报告层不再合成「全局 coverage/mutation + overrides」级联，而是直接消费 parse 后的 suite 阈值。集成夹具从旧 `test.overrides` 迁到 `tests[]` 后，验证汇总报告中的 threshold / 分组字段反映 suite 维度。出错模式是仍读取已删除的全局块，或两 suite 阈值串用。

#### 场景: 多 suite 阈值无全局级联

临时项目配置两个不同 coverage/mutation 阈值的 suite，生成子报告后汇总，断言各组阈值来自对应 suite。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | suite A/B 自定义阈值分别出现在汇总分组中 | 新增 |
| 正向 | 省略 suite 阈值时使用 schema 默认，且不依赖任何顶层全局块 | 新增 |
| 异常 | suite 阈值为 `-1` 时配置解析失败，报告流程不采用非法值 | 新增 |
| 边界 | suite 阈值为 `0` / `100` 时汇总分组使用对应边界值 | 新增 |
| 边界 | 仅残留旧 `test.coverage` 键时报告不采用该值作为正式阈值来源 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 临时文件系统 | 写入 `tests[]` 配置与最小 coverage/mutation 产物 | 全部 |
| 测试执行子进程 | mock `executePlanEntry` 或直接构造 `ExecutionResult` 喂给报告 API | 不真实跑框架 |

---

### absCwd → mutation / stryker 临时配置 → `plugins/dev-team/bin/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 以 plan.directory（absCwd）执行变异阶段并过滤 mutate 源 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | 在 absCwd 下生成临时 Stryker 配置与 mutate 相对路径 |

**关联AC**: AC-2, AC-4, AC-7

**关系描述**:

变异阶段的工作目录、临时文件落点与 mutate 路径重写必须统一到 absCwd。集成测试更新既有 mutation 夹具为 `tests[]`，串联 runner 与 stryker-config，验证 `cwd: ".."` 场景下临时配置仍落在执行目录且 mutate 路径相对该根。出错模式是仍相对 projectRoot 写 mutate，或临时文件落到 root 而非 absCwd。

#### 场景: 变异 cwd 与 mutate 相对路径

配置 suite 使 absCwd 与 root 不同，触发 mutation 流程（子进程 mock），断言 Stryker 配置落点与 mutate 条目相对 absCwd。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | mutation 执行 cwd = absCwd；临时 `stryker.config.*` 位于 absCwd | 新增 |
| 正向 | mutate 列表路径相对 absCwd，且遵守 suite excludes | 新增 |
| 异常 | 不支持 mutation 的框架或非法 framework 时变异阶段失败信息明确 | 新增 |
| 边界 | `sourceFiles: []` 或 exclude 后为空时跳过 mutation 且不写非法 mutate | 新增 |
| 边界 | 夹具从 `test.overrides` 迁到 `tests[]` 后既有 mutation-diff-only / mutation-execution-flow 场景仍通过 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 测试/变异子进程 | mock `execCommand`/`execSync`，捕获 cwd 与命令 | 全部 |
| 临时文件系统 | 布置源文件与 `openspec/config.json`（`tests[]`） | 全部 |

---

### 既有 __tests__ 夹具迁入 tests[] → `plugins/dev-team/bin/__tests__/config-driven-auto-scan/config-driven-auto-scan.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 空 modules 自动扫描入口 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 由 `tests[]` 提供 plan/directory |

**关联AC**: AC-6, AC-7

**关系描述**:

proposal 明确要求更新 `__tests__` 中构造旧 `test.overrides` 的集成夹具（config-driven-auto-scan、dedup、no-test-config、mutation-diff-only 等）。本关系以「夹具配置形状迁移 + 行为断言更新」验证硬 breaking 后回归面。出错模式是夹具仍写旧键导致误判「无配置」或扫描根错误。

#### 场景: config-driven / dedup / no-test-config 夹具迁移

将上述夹具中的 `test.overrides` 改为等价 `tests[]`（方案 G），并更新无配置场景的错误文案期望为引导 `tests`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | config-driven-auto-scan：`modules: []` + `tests[]` 扫出期望 unit_tests | 新增 |
| 正向 | dedup：多 suite 指向同一扫描范围时 unit_tests 无重复 | 新增 |
| 异常 | no-test-config：无 `tests` 或 `tests: []` 时 errors 提示配置 `tests` | 新增 |
| 边界 | 仅含旧 `test.overrides` 的夹具期望从「成功扫描」改为「无有效 tests 配置」 | 新增 |

<!-- 无跨进程边界 Mock；沿用各夹具现有 temp project 模式，省略 Mock策略小节 -->

---

## 不可测试项

- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — **原因**: `test_resolve_paths` 返回 `Not in test config scope`（当前/迁移后 suite `excludes` 含 `src/schemas/**/*`）。
- `plugins/dev-team/bin/src/schemas/config/defaults.ts` — **原因**: 同上，不在测试配置 scope；常量正确性通过 suite 缺省阈值集成断言（80/70/75/70）间接验证。
- `plugins/dev-team/bin/dev-team-config.schema.json` — **原因**: 由 Zod `toJSONSchema` 构建产物镜像，无独立运行时单元路径；与 Zod 一致性依赖构建步骤 / code review。
- `openspec/config.json` — **原因**: 项目配置迁移文件，非可执行模块；AC-6 通过集成场景「本仓库 tests[] 迁移可出 plan」验证。
- `plugins/dev-team/agents/test-execution-executor.md` 等 agent 文案 — **原因**: Markdown 文档同步，非可执行代码；靠 code review 确认旧 `test.*` 键表述已改为 `tests`。
- `plugins/dev-team/.claude-plugin/plugin.json` 版本升级 — **原因**: 清单字段变更，无自动化行为断言；靠变更清单与发布检查确认。
