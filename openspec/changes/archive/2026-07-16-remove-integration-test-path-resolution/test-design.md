# 测试设计: 移除 test_resolve_paths 集成测试路径解析功能

> **日期**: 2026-07-16

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | 输入 schema 移除 `integration_scenarios`、`integration_root`、`extension` | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 集成测试路径移除 |
| AC-1 | 输入 schema 移除 `integration_scenarios`、`integration_root`、`extension` | 集成测试 | `plugins/dev-team/bin/__tests__/legacy-param-ignore/legacy-param-ignore.test.ts` | legacy-param-ignore |
| AC-2 | 输出 schema 移除 `integration_tests` | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 集成测试路径移除 |
| AC-2 | 输出 schema 移除 `integration_tests` | 集成测试 | `plugins/dev-team/bin/__tests__/legacy-param-ignore/legacy-param-ignore.test.ts` | legacy-param-ignore |
| AC-3 | 移除 `IntegrationTestEntry` 接口（编译通过、无引用残留） | 编译验证 | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | （编译时类型检查） |
| AC-4 | 移除 `deriveIntegrationTestPath`、`normalizeIntegrationRoot`、`isValidIntegrationRoot`、`inferExtension`、`normalizeExtension`、`resolveIntegrationTests` 六个函数 | 编译验证 | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | （编译时类型检查） |
| AC-5 | MCP 工具 description 不再提及集成测试 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 -- test_resolve_paths description (AC-5) |
| AC-6 | 单元测试路径推导不受影响 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 |
| AC-6 | 单元测试路径推导不受影响 | 集成测试 | `plugins/dev-team/bin/__tests__/legacy-param-ignore/legacy-param-ignore.test.ts` | legacy-param-ignore |
| AC-7 | 传入 `integration_scenarios` 时不会报错也不会生成集成测试路径（参数被 Zod 静默忽略） | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 集成测试路径移除 |
| AC-7 | 传入 `integration_scenarios` 时不会报错也不会生成集成测试路径（参数被 Zod 静默忽略） | 集成测试 | `plugins/dev-team/bin/__tests__/legacy-param-ignore/legacy-param-ignore.test.ts` | legacy-param-ignore |
| AC-8 | 测试文件中集成测试相关 describe 块被移除 | 静态验证 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | grep 文件内容确认无旧 describe 标题 |
| AC-9 | `test-design-planner.md` 不再引用 `integration_scenarios`/`integration_root`/`integration_tests` | 静态验证 | `plugins/dev-team/agents/test-design-planner.md` | grep 文件内容确认无参数引用 |

---

## 单元测试

### 用例

#### 测试文件: `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts`

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 集成测试路径移除 | 正向 | 传入 integration_scenarios、extension、integration_root 时结果不包含 integration_tests 字段且无相关错误 (AC-1, AC-2, AC-7) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 集成测试路径移除 | 边界 | 仅传入 integration_scenarios 时结果不包含 integration_tests (AC-1, AC-2, AC-7) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 集成测试路径移除 | 边界 | integration_scenarios 为空数组时语义等价于不传（结果不含 integration_tests）(AC-7) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 集成测试路径移除 | 正向 | 同时传入三个旧参数时输出仅含 unit_tests 和 errors 两个顶层字段 (AC-1, AC-2, AC-7) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 集成测试路径移除 | 正向 | 不传任何旧参数时 unit_tests 与 errors 行为正常 (AC-6) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 | 正向 | 多个源文件路径各自返回同级测试文件 (AC-6) | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 | 边界 | unit_tests 按 source 字典序排序且去重 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 | 异常 | 已是测试文件的路径写入 errors 而非 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 | 异常 | 非源文件路径（.md、.json、.yaml 等）写入 errors | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 | 正向 | 派生 .py 文件的测试路径为 test_\<name\>.py | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 | 正向 | 派生 .go 文件的测试路径为 \<name\>_test.go | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 | 正向 | 派生 .rs 文件的测试路径为 \<name\>_test.rs | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 文件路径解析 | 正向 | 派生 .tsx/.jsx/.mjs 文件的测试路径 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 错误收集 | 正向 | 不存在的源文件路径仍推导出测试路径（无需判断文件是否存在） | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 错误收集 | 异常 | README.md 写入 errors，unit_tests 为空 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 错误收集 | 异常 | 已是测试文件的路径写入 errors | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 错误收集 | 异常 | 路径穿越 ../../../outside.ts 写入 errors | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 错误收集 | 边界 | errors 按 path 字典序排序 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 错误收集 | 正向 | 绝对路径在项目范围内正常解析 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 错误收集 | 正向 | 单条失败不中断其余 modules 处理 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- modules 边界 | 边界 | 单元素文件路径返回单条 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- modules 边界 | 边界 | 大量文件路径（100+）不抛错且全部解析 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- modules 边界 | 正向 | 多个不同目录文件路径各自返回同级测试文件 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 端到端编排 | 正向 | 传入 modules 返回完整解析结构 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 端到端编排 | 正向 | project_root 指向绝对路径时相对路径解析正确 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 自动扫描 | 正向 | modules: [] 且 test.overrides 有效时扫描并推导 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 自动扫描 | 正向 | modules: [] 时 runTestDetectFrameworks 被调用（spy 验证） | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 自动扫描 | 异常 | plan 为空时 errors 包含 "No test configuration" 提示，unit_tests 为空 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 过滤 | 正向 | modules 在 test config 范围内时正常返回 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 过滤 | 异常 | modules 不在 test config 范围内时写入 errors | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 过滤 | 边界 | 混合范围内外文件，仅范围内文件进入 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 正向 | git diff 返回变更文件时返回对应 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 异常 | git 命令失败时 errors 包含错误消息 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 边界 | 变更文件均不在 test config 范围内时 unit_tests 为空 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 边界 | git diff 返回空时 unit_tests 为空 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 异常 | extractErrorMessage 的 catch 分支覆盖（stderr 不可序列化） | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 异常 | execSync 抛出带 stderr 的非 Error 对象时 errors 包含消息 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 边界 | git diff 返回大量文件（100+）不抛错 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 去重 | 边界 | 多个 override 指向同一目录时扫描结果去重 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 去重 | 边界 | modules 包含重复文件路径时 unit_tests 去重 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 去重 | 正向 | 扫描与显式传入重叠时各自去重正常 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- exclude 过滤 | 正向 | 配置 test.exclude 后排除文件不出现在 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- exclude 过滤 | 正向 | 未被排除的文件正常出现在 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- exclude 过滤 | 正向 | 空 modules 自动扫描模式下 exclude 过滤生效 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- exclude 过滤 | 正向 | 混合排除/未排除文件时仅未排除文件进入 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- exclude 过滤 | 边界 | 不配置 exclude 时全部源文件正常进入 unit_tests | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- exclude 过滤 | 边界 | override-level exclude 不扩展到其他 override 区域 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 向后兼容 | 正向 | 不配置 exclude 时全部现有功能行为不变 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 向后兼容 | 边界 | exclude 为空数组时结果与无 exclude 一致 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 向后兼容 | 边界 | exclude 值为 undefined 时路径解析不受影响 | 保留 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 向后兼容 | 边界 | 配置中不存在 test 节时路径解析正常 | 保留 |

#### 测试文件: `plugins/dev-team/bin/src/mcp.test.ts`

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 -- test_resolve_paths description (AC-5) | 正向 | description 不含 "integration" 或 "\_\_tests\_\_" 字符串 | 新增 |
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — 工具已移除 | 异常 | registerTool 未注册 config_set/config_unset/config_context | 保留 |
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — config_get 保留 | 正向 | registerTool 已注册 config_get | 保留 |
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — 其他工具保留 | 正向 | 10 个必要工具均已注册（含 test_resolve_paths） | 保留 |
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — import 完整性 | 正向 | schemas 模块可成功导入 | 保留 |
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — import 完整性 | 正向 | 各 handler 可调用并返回正确响应结构 | 保留 |
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — import 完整性 | 正向 | toolkit 模块可成功导入 | 保留 |

### Mock策略

#### 测试文件: `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts`

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `runTestDetectFrameworks`（test-detect-frameworks 模块） | `vi.mock('./test-detect-frameworks')` 整体 mock；默认 `vi.fn(actual.runTestDetectFrameworks)` 保持透传；各 describe 通过 `vi.mocked(runTestDetectFrameworks).mockImplementation(...)` 覆写返回值 | 全部 describe 块 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `execSync`（child_process） | `vi.mock('child_process')` 整体 mock；默认透传；各 describe 通过 `vi.mocked(execSync).mockReturnValue(...)` 或 `mockImplementation(...)` 模拟 git diff 输出/异常 | git-change 模式 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | 临时文件系统（os.tmpdir） | `createTempProject()` 创建临时项目目录；`cleanup()` 删除 | 全部测试用例 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | openspec/config.json | 在临时项目根目录创建 `openspec/config.json` 控制 exclude/override 配置 | exclude 过滤、向后兼容 |

#### 测试文件: `plugins/dev-team/bin/src/mcp.test.ts`

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/mcp.test.ts` | `McpServer`（@modelcontextprotocol/sdk） | `vi.mock('@modelcontextprotocol/sdk/server/mcp')` mock 构造函数；`registerTool` 替换为 `vi.fn()` 记录调用 | 全部测试用例 |
| `plugins/dev-team/bin/src/mcp.test.ts` | `StdioServerTransport` | `vi.mock('@modelcontextprotocol/sdk/server/stdio')` mock 传输层 | 全部测试用例 |
| `plugins/dev-team/bin/src/mcp.test.ts` | `initProjectRootFromMcp` | `vi.mock('./lib/project-root')` mock 初始化 | 全部测试用例 |

---

## 集成测试

### 用例

本次变更为纯移除操作：移除 `test_resolve_paths` 的集成测试路径解析功能。核心验证路径为：
1. 旧参数（`integration_scenarios`、`integration_root`、`extension`）在传入后不被任何逻辑消费，输出不因这些参数变化
2. 输出中不含 `integration_tests` 字段
3. 单元测试路径推导行为不受影响

集成测试在 `plugins/dev-team/bin/__tests__/` 下增加 **legacy-param-ignore** 场景，在无 mock 的真实文件系统环境中端到端验证上述三条路径。

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1, AC-2, AC-7 | `plugins/dev-team/bin/__tests__/legacy-param-ignore/legacy-param-ignore.test.ts` | legacy-param-ignore | 传入 integration_scenarios、integration_root、extension 时输出不含 integration_tests 字段且无相关错误 | 新增 |
| AC-2, AC-7 | `plugins/dev-team/bin/__tests__/legacy-param-ignore/legacy-param-ignore.test.ts` | legacy-param-ignore | 输出对象的顶层键仅为 ["unit_tests", "errors"]，不含 integration_tests | 新增 |
| AC-6, AC-7 | `plugins/dev-team/bin/__tests__/legacy-param-ignore/legacy-param-ignore.test.ts` | legacy-param-ignore | 传入旧参数时的 unit_tests 返回值与不传旧参数时一致（单元测试路径推导不受影响） | 新增 |
| AC-6 | `plugins/dev-team/bin/__tests__/legacy-param-ignore/legacy-param-ignore.test.ts` | legacy-param-ignore | 传 modules 不含旧参数时行为与移除前完全一致 | 新增 |

### Mock策略

集成测试不 mock 任何模块：使用临时目录 + openspec/config.json 真实文件系统，直接调用 `runTestResolvePaths` API。`runTestDetectFrameworks` 和 `execSync` 均为真实调用，确保端到端覆盖。

---

## 不可测试项

| 条目 | 原因 |
|------|------|
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts`（AC-1、AC-2 的 schema 文件验证） | 配置文件 `test.overrides` 中 `exclude: ["src/schemas/**/*"]` 将该文件排除在测试配置范围外，`test_resolve_paths` 返回 `"Not in test config scope"` 错误。Schema 变更的验证方式：(1) TypeScript 编译时类型检查；(2) 间接通过 test-resolve-paths.test.ts（运行时行为）和 mcp.test.ts（类型导入）覆盖 |
| AC-3：`IntegrationTestEntry` 接口移除 | `IntegrationTestEntry` 是 TypeScript 接口，编译时被擦除，在运行时不可检测。覆盖方式：(1) TypeScript 编译验证（任何残留引用导致编译错误）；(2) AC-2/AC-7 测试通过 `expect(result).not.toHaveProperty('integration_tests')` 从行为上验证该接口不再被使用 |
| AC-4：六个集成测试路径函数移除 | `deriveIntegrationTestPath`、`normalizeIntegrationRoot`、`isValidIntegrationRoot`、`inferExtension`、`normalizeExtension`、`resolveIntegrationTests` 均为未导出的模块内部函数，运行时不可检测。覆盖方式：(1) TypeScript 编译验证；(2) AC-2/AC-7 测试从行为层面验证集成测试路径不再被生成 |
| AC-8：测试文件中集成测试相关 describe 块被移除 | `test-resolve-paths.test.ts` 中已被删除的旧 describe 块无法通过自动化测试验证其不存在。验证方式：代码审查或 CI grep 确认文件中不含 `describe('runTestResolvePaths -- 集成测试路径'`、`describe('runTestResolvePaths -- integration_root` 等标题 |
| AC-9：`test-design-planner.md` 内容更新 | Agent prompt 为 Markdown 文件，非可执行代码，无法通过自动化测试覆盖。验证方式：代码审查或 CI grep 确认文件中不含 `integration_scenarios`、`integration_root`、`integration_tests` 等参数引用 |
