# 测试设计: add-test-path-resolver-api

> **日期**: 2026-06-10

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `test_resolve_paths` 对 `src/config.ts` 返回单元测试路径 `src/config.test.ts` | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- TypeScript 单元测试路径推导 |
| AC-2 | `test_resolve_paths` 对 `src/component/Button.tsx` 返回 `src/component/Button.test.tsx` | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- TSX 单元测试路径推导 |
| AC-3 | `test_resolve_paths` 对 `src/auth.py` 返回 `src/test_auth.py` | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- Python 单元测试路径推导 |
| AC-4 | `test_resolve_paths` 对 `src/handler.go` 返回 `src/handler_test.go` | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- Go 单元测试路径推导 |
| AC-5 | `test_resolve_paths` 对 `src/lib.rs` 返回 `src/lib_test.rs` | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- Rust 单元测试路径推导 |
| AC-6 | 传入目录 `src/commands/` 时展开目录内所有源文件并返回对应单元测试路径列表 | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 目录展开 |
| AC-7 | `integration_scenarios: ["api-flow"]` 且 `extension: "ts"` 时返回 `__tests__/api-flow/api-flow.test.ts` | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 集成测试路径（显式 extension） |
| AC-8 | 未传 `extension` 时从 `modules` 中源文件扩展名推断集成测试扩展名 | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 集成测试扩展名推断 |
| AC-9 | 不存在的路径、非源文件路径写入 `errors` 且其余有效条目仍正常返回 | 单元测试 | `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 错误收集 |
| AC-10 | `modules` 为空数组时返回校验错误（不返回空成功结果） | 单元测试 | `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- modules 必填校验 |
| AC-11 | `mcp.ts` 注册 `test_resolve_paths`，input/output schema 与 spec 一致 | 单元测试 | `schemas/test-resolve-paths.schema.test.ts` | input/output schema 契约验证 |
| AC-11 | `mcp.ts` 注册 `test_resolve_paths`，input/output schema 与 spec 一致 | 集成测试 | `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | MCP 工具注册静态检查 |
| AC-12 | `test-design-planner.md` Process 包含调用 `test_resolve_paths` 的步骤，并用返回路径填充 `测试文件` 列 | 单元测试 | `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | test-design-planner Agent 静态检查 |
| AC-13 | 构建通过：`plugins/dev-team/bin` 下 `npm run build` 无错误 | 集成测试 | — | CI / 本地构建验证（见不可测试项） |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `commands/test-resolve-paths.test.ts` | `deriveUnitTestPath` -- 纯函数确定性 | 正向 | `src/config.ts` → `src/config.test.ts` (AC-1) | 新增 |
| `commands/test-resolve-paths.test.ts` | `deriveUnitTestPath` -- 纯函数确定性 | 正向 | `src/component/Button.tsx` → `src/component/Button.test.tsx` (AC-2) | 新增 |
| `commands/test-resolve-paths.test.ts` | `deriveUnitTestPath` -- 纯函数确定性 | 正向 | `src/auth.py` → `src/test_auth.py` (AC-3) | 新增 |
| `commands/test-resolve-paths.test.ts` | `deriveUnitTestPath` -- 纯函数确定性 | 正向 | `src/handler.go` → `src/handler_test.go` (AC-4) | 新增 |
| `commands/test-resolve-paths.test.ts` | `deriveUnitTestPath` -- 纯函数确定性 | 正向 | `src/lib.rs` → `src/lib_test.rs` (AC-5) | 新增 |
| `commands/test-resolve-paths.test.ts` | `deriveUnitTestPath` -- JS/TS 扩展名全覆盖 | 正向 | `.js`、`.jsx`、`.mjs`、`.cjs` 均生成 `<name>.test<ext>` 同级路径 | 新增 |
| `commands/test-resolve-paths.test.ts` | `deriveUnitTestPath` -- 纯函数确定性 | 边界 | 相同输入多次调用产生相同输出（不依赖 fs） | 新增 |
| `commands/test-resolve-paths.test.ts` | `deriveUnitTestPath` -- 纯函数确定性 | 边界 | 源路径含空格或 Unicode 字符时 basename 正确保留 | 新增 |
| `commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` -- 纯函数 | 正向 | `scenario="api-flow"`, `ext="ts"` → `__tests__/api-flow/api-flow.test.ts` (AC-7) | 新增 |
| `commands/test-resolve-paths.test.ts` | `normalizeExtension` -- 纯函数 | 正向 | `".ts"` → `"ts"`；`"TS"` → `"ts"` | 新增 |
| `commands/test-resolve-paths.test.ts` | `normalizeExtension` -- 纯函数 | 边界 | 空字符串 `""` 的处理（依实现：回退默认或原样传递） | 新增 |
| `commands/test-resolve-paths.test.ts` | `inferExtension` -- 纯函数 | 正向 | 源文件均为 `.py` 时推断 `py` (AC-8) | 新增 |
| `commands/test-resolve-paths.test.ts` | `inferExtension` -- 纯函数 | 正向 | 显式 `extension` 优先于众数推断 | 新增 |
| `commands/test-resolve-paths.test.ts` | `inferExtension` -- 纯函数 | 边界 | 无有效源文件时默认 `ts` | 新增 |
| `commands/test-resolve-paths.test.ts` | `inferExtension` -- 纯函数 | 边界 | 混合扩展名时取众数（如 2×`.ts` + 1×`.py` → `ts`） | 新增 |
| `commands/test-resolve-paths.test.ts` | `isTestFile` / `isSourceFile` -- 纯函数 | 正向 | `foo.test.ts`、`test_auth.py`、`handler_test.go`、`lib_test.rs` 识别为测试文件 | 新增 |
| `commands/test-resolve-paths.test.ts` | `isTestFile` / `isSourceFile` -- 纯函数 | 异常 | `README.md`、`config.json` 识别为非源文件 | 新增 |
| `commands/test-resolve-paths.test.ts` | `isWithinProjectRoot` -- 纯函数 | 异常 | `../../../outside.ts` 解析后越出 `project_root` 返回 false | 新增 |
| `commands/test-resolve-paths.test.ts` | `toPosixRelativePath` -- 纯函数 | 边界 | Windows 反斜杠路径规范为 POSIX `/` 分隔符 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 目录展开 | 正向 | 目录 `src/commands/` 含 `foo.ts` 与 `bar.ts`，返回两条 `unit_tests` 且路径正确 (AC-6) | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 目录展开 | 正向 | `unit_tests` 按 `source` 字典序排序且去重 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 目录展开 | 边界 | `src/node_modules/pkg/index.ts` 被跳过 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 目录展开 | 边界 | 跳过 `.git`、`dist`、`build`、`coverage`、`.nyc_output`、`target` 目录 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 目录展开 | 边界 | 已有测试文件 `foo.test.ts` 不被当作源文件展开 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 目录展开 | 边界 | 跳过 `.md`、`.json`、`.yaml`、`.yml`、`.txt`、`.lock` 文件 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 集成测试路径 | 正向 | `integration_scenarios: ["api-flow"]` + `extension: "ts"` → `__tests__/api-flow/api-flow.test.ts` (AC-7) | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 集成测试路径 | 正向 | `modules: ["src/auth.py"]` + `integration_scenarios: ["db-roundtrip"]`（无 extension）→ `__tests__/db-roundtrip/db-roundtrip.test.py` (AC-8) | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 集成测试路径 | 边界 | 无有效源文件（仅 `README.md` 写入 errors）时集成测试默认 `.test.ts` 扩展名 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 集成测试路径 | 边界 | 多个 `integration_scenarios` 按 `scenario` 字典序排序 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 集成测试路径 | 边界 | `integration_scenarios` 未传或为空数组时 `integration_tests` 为空 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 错误收集 | 异常 | `src/missing.ts` 不存在 + `src/config.ts` 存在：errors 含缺失项，unit_tests 仍含有效条目 (AC-9) | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 错误收集 | 异常 | `README.md` 写入 errors，`unit_tests` 为空 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 错误收集 | 异常 | 传入已是测试文件的路径（如 `src/config.test.ts`）写入 errors | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 错误收集 | 异常 | 路径穿越 `../../../outside.ts` 写入 errors | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 错误收集 | 边界 | `errors` 按 `path` 字典序排序 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- 错误收集 | 边界 | 单条失败不中断其余 modules 条目的处理 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- modules 边界 | 边界 | `modules` 含单元素文件路径时返回单条 `unit_tests` | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- modules 边界 | 边界 | `modules` 含大量文件路径（如 100+ 条）时不抛错且全部解析 | 新增 |
| `commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- modules 边界 | 边界 | `modules` 混合文件与目录路径时合并展开结果 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 正向 | 正向 | 完整有效输入（含 `modules`、`integration_scenarios`、`extension`、`project_root`）通过验证 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 正向 | 正向 | 仅 `modules` 最小必填输入通过验证 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 正向 | 正向 | `extension` 带前导 `.`（如 `".ts"`）的输入通过 schema（规范化在 command 层） | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 正向 | 正向 | `project_root` 为 `null` 时通过验证（nullable） | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 异常 | 异常 | `modules: []` 被 input schema 拒绝 (AC-10) | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 异常 | 异常 | 缺少 `modules` 字段时拒绝 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 异常 | 异常 | `modules` 为非数组类型时拒绝 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsOutputSchema` -- 正向 | 正向 | 完整输出对象（含 `unit_tests`、`integration_tests`、`errors`）通过验证 (AC-11) | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsOutputSchema` -- 正向 | 正向 | 三个数组均为空 `[]` 时通过验证 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsOutputSchema` -- 异常 | 异常 | 缺少 `unit_tests` / `integration_tests` / `errors` 任一字段时拒绝 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsOutputSchema` -- 异常 | 异常 | `unit_tests` 条目缺少 `source` 或 `test_file` 时拒绝 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsOutputSchema` -- 异常 | 异常 | `errors` 条目 `message` 为 `null` 时拒绝 | 新增 |
| `schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsOutputSchema` -- 边界 | 边界 | `integration_tests` 含多条 scenario 条目时通过验证 | 新增 |
| `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | test-design-planner Agent 静态检查 | 正向 | Process 包含 `test_resolve_paths` / `mcp__plugin_dev-team_dev-team__test_resolve_paths` 调用步骤 (AC-12) | 新增 |
| `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | test-design-planner Agent 静态检查 | 正向 | 包含禁止手工拼接测试路径的约束 (AC-12) | 新增 |
| `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | test-design-planner Agent 静态检查 | 正向 | 包含将 `unit_tests` / `integration_tests` 映射到 `测试文件` 列的说明 (AC-12) | 新增 |
| `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | test-design-planner Agent 静态检查 | 正向 | 包含 `errors` → `不可测试项` 的处理说明 (AC-12) | 新增 |
| `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | MCP 工具注册静态检查 | 正向 | `mcp.ts` 包含 `registerTool('test_resolve_paths'` 或等效注册 (AC-11) | 新增 |
| `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | MCP 工具注册静态检查 | 正向 | `mcp.ts` handler 调用 `runTestResolvePaths` | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `commands/test-resolve-paths.test.ts` | 无（纯函数） | `deriveUnitTestPath`、`deriveIntegrationTestPath`、`normalizeExtension`、`inferExtension`、`isTestFile`、`isSourceFile`、`isWithinProjectRoot`、`toPosixRelativePath` 为无外部依赖纯函数，直接断言 | 所有纯函数测试用例 |
| `commands/test-resolve-paths.test.ts` | 文件系统 | 通过 `fs.mkdtempSync` 创建临时项目 fixture，写入源文件/目录结构，测试结束后 `fs.rmSync` 清理；不 mock `fs`，使用真实临时目录（沿用 `test-detect-frameworks.test.ts` 模式） | `resolveTestPaths` 目录展开、错误收集、集成路径推断用例 |
| `schemas/test-resolve-paths.schema.test.ts` | 无 | Zod schema 为纯数据验证，直接 `safeParse` 断言 | 所有 schema 测试用例 |
| `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | 无 | 使用 `re` 读取 `test-design-planner.md` 与 `mcp.ts` 源文件进行静态文本匹配 | Agent 与 MCP 注册静态检查 |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1~AC-9 | `commands/test-resolve-paths.test.ts` | `runTestResolvePaths` -- 端到端编排 | 传入 `modules`、`integration_scenarios`、`extension`，`project_root` 指向含 fixture 的临时目录，返回结构与 `resolveTestPaths` 一致 | 新增 |
| AC-1~AC-9 | `commands/test-resolve-paths.test.ts` | `runTestResolvePaths` -- project_root 覆盖 | `project_root` 指向绝对路径且该路径下源文件存在时，相对路径解析正确 | 新增 |
| AC-11 | `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py` | MCP 注册与 schema 导入 | `mcp.ts` 导入 `testResolvePathsInputSchema` / `testResolvePathsOutputSchema` 且工具名与 spec 一致 | 新增 |
| AC-13 | — | 构建验证 | `plugins/dev-team/bin` 下 `npm run test` 全部通过 | 新增 |
| AC-13 | — | 构建验证 | `plugins/dev-team/bin` 下 `npm run build`（`vp pack`）无错误 | 新增 |
| — | `commands/test-resolve-paths.test.ts` | 回归：`test_detect_frameworks` 不受影响 | 本变更不修改 `test-detect-frameworks.ts`；现有 `test-detect-frameworks.test.ts` 用例保持通过（CI 全量 `npm run test` 验证） | 废弃 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `commands/test-resolve-paths.test.ts` | `process.env.CLAUDE_PROJECT_DIR` | `runTestResolvePaths` 测试显式传入 `project_root`，不依赖环境变量默认值 | `runTestResolvePaths` 集成用例 |
| `commands/test-resolve-paths.test.ts` | `resolveProjectRoot` | 不 mock；`runTestResolvePaths` 单元内自行解析 `project_root` 参数后委托 `resolveTestPaths` | `runTestResolvePaths` 用例 |
| — | MCP SDK / stdio transport | 不启动真实 MCP 服务器；通过 `runTestResolvePaths` 命令层函数间接验证 handler 逻辑 | AC-11 MCP handler 行为 |

---

## 不可测试项

- **MCP stdio 传输层端到端调用（AC-11 运行时）** — 测试在 Node.js 进程内通过 `runTestResolvePaths` 与 schema 单元测试验证 handler 逻辑，不启动真实 MCP stdio 会话。工具注册名称与 import 通过 Python 静态检查与代码审查确认。
- **`npm run build` 打包产物内容逐字段断言（AC-13）** — 构建成功由 CI / 本地 `npm run build` 验证；不对 `dev-team-mcp.cjs` 产物做逐行快照测试。回归依赖全量 `npm run test` 与构建无错误。
- **test-design-planner 运行时实际调用 MCP 并生成 test-design.md（AC-12 端到端）** — Agent 行为通过 `test-design-planner.md` 静态检查验证 Process 与 Constraints 文案；不在本变更中启动真实 Agent 会话做 E2E。
- **Python 工具 `test-scope.py` / `test-generator.py` 路径规则对齐** — 明确 out-of-scope；本变更以 `test-resolve-paths.ts` 为权威，不对 Python 工具做回归测试。
- **`.java` 等 spec 未列出的扩展名** — 待决问题；本次严格限定七种可测试扩展名，超出集合的路径写入 `errors`，不单独为未支持语言编写用例。
