# 测试设计: add-integration-root-param

> **日期**: 2026-06-16

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | 未传 `integration_root` 时，`integration_scenarios: ["api-flow"]` 仍返回 `__tests__/api-flow/api-flow.test.ts` | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 向后兼容（未传） |
| AC-2 | `integration_root: "."` 时行为与 AC-1 相同 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 为点号时无前缀 |
| AC-3 | `integration_root: "plugins/dev-team/bin"` 时返回 `plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts` | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 子目录前缀 |
| AC-4 | `deriveIntegrationTestPath("api-flow", "ts", "plugins/dev-team/bin")` 返回带前缀的 POSIX 路径 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` -- 纯函数带 integrationRoot |
| AC-5 | `integration_root` 含尾部斜杠时规范化后路径正确（无重复斜杠） | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` / `resolveTestPaths` -- 尾部斜杠规范化 |
| AC-6 | `integration_root` 不影响 `unit_tests` 推导结果 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 与 unit_tests 隔离 |
| AC-7 | `testResolvePathsInputSchema` 接受可选 `integration_root` 字段 | 单元测试 | `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- integration_root 可选字段 |
| AC-8 | `mcp.ts` 注册的 `test_resolve_paths` input schema 包含 `integration_root` | 单元测试 | `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | MCP 注册与 schema 静态检查 |
| AC-9 | `test-design-planner.md` Process 描述多框架下按 plan 条目 `directory` 传入 `integration_root` | 单元测试 | `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | test-design-planner Agent 静态检查 |
| AC-10 | `plugins/dev-team/bin` 下 `npm run build` 无错误 | 集成测试 | — | CI / 本地构建验证（见不可测试项） |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` -- 纯函数 | 正向 | 未传 `integrationRoot` 第三参数时，`("api-flow", "ts")` → `__tests__/api-flow/api-flow.test.ts` (AC-1) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` -- 纯函数 | 正向 | `integrationRoot: "."` 时 → `__tests__/api-flow/api-flow.test.ts` (AC-2) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` -- 纯函数 | 正向 | `("api-flow", "ts", "plugins/dev-team/bin")` → `plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts` (AC-4) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` -- 纯函数 | 边界 | `integrationRoot: "plugins/dev-team/bin/"` 尾部斜杠规范化后无 `//` 重复 (AC-5) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` -- 纯函数 | 边界 | `integrationRoot: ""` 空字符串时行为与未传或 `"."` 一致（无前缀） | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `deriveIntegrationTestPath` -- 纯函数 | 边界 | Windows 反斜杠 `integrationRoot: "plugins\\dev-team\\bin"` 规范为 POSIX `/` 分隔符 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 向后兼容 | 正向 | 未传 `integrationRoot` + `integration_scenarios: ["api-flow"]` + `extension: "ts"` → `__tests__/api-flow/api-flow.test.ts` (AC-1) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 为点号 | 正向 | `integrationRoot: "."` + 同上场景 → 与 AC-1 相同 (AC-2) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 子目录前缀 | 正向 | `integrationRoot: "plugins/dev-team/bin"` + `integration_scenarios: ["api-flow"]` → `plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts` (AC-3) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 子目录前缀 | 边界 | `integration_root: "plugins/dev-team/bin/"` 经 `runTestResolvePaths` 映射后路径与 AC-3 一致 (AC-5) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 与 unit_tests 隔离 | 正向 | `modules: ["src/config.ts"]` + `integrationRoot: "plugins/dev-team/bin"` 时 `unit_tests[0].test_file` 仍为 `src/config.test.ts` (AC-6) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 与 unit_tests 隔离 | 边界 | 多个 `modules` 含不同语言源文件时，`integrationRoot` 仅改变 `integration_tests`，`unit_tests` 列表与未传时完全一致 (AC-6) | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `resolveTestPaths` -- integration_root 路径穿越 | 异常 | `integrationRoot: "../outside"` 含 `..` 段时写入 `errors` 且 `integration_tests` 为空（若实现 D7 轻量校验） | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `runTestResolvePaths` -- snake_case 映射 | 正向 | `runTestResolvePaths({ modules, integration_scenarios, integration_root: "plugins/dev-team/bin" })` 与 `resolveTestPaths({ integrationRoot })` 结果一致 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `runTestResolvePaths` -- snake_case 映射 | 边界 | 未传 `integration_root` 时 `runTestResolvePaths` 与 `resolveTestPaths` 行为与变更前一致（回归） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 正向 | 正向 | 含 `integration_root: "plugins/dev-team/bin"` 的完整有效输入通过验证 (AC-7) | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 正向 | 正向 | 不含 `integration_root` 的最小必填输入仍通过（向后兼容） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 边界 | 边界 | `integration_root: ""` 空字符串通过 schema（规范化在 command 层） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | `testResolvePathsInputSchema` -- 边界 | 边界 | `integration_root: "."` 通过 schema | 新增 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | MCP 注册与 schema 静态检查 | 正向 | `test-resolve-paths.schema.ts` 含 `integration_root` 字段定义 (AC-8) | 新增 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | MCP 注册与 schema 静态检查 | 正向 | `mcp.ts` 注册 `test_resolve_paths` 且导入 `testResolvePathsInputSchema` (AC-8) | 新增 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | test-design-planner Agent 静态检查 | 正向 | Process 描述存在集成场景时先调用 `test_detect_frameworks` (AC-9) | 新增 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | test-design-planner Agent 静态检查 | 正向 | Process 将 plan 条目 `directory` 作为 `integration_root` 传入 `test_resolve_paths` (AC-9) | 新增 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | test-design-planner Agent 静态检查 | 正向 | Process 描述合并各次 `integration_tests` 映射到 `集成测试 > 用例` 表格 (AC-9) | 新增 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | test-design-planner Agent 静态检查 | 正向 | 禁止手工拼接测试路径的约束仍存在 (AC-9) | 新增 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | test-design-planner Agent 静态检查 | 正向 | 单元测试路径单次调用且不传 `integration_root` 的说明存在 (AC-9) | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | 无（纯函数） | `deriveIntegrationTestPath` 为无外部依赖纯函数，直接断言 | `deriveIntegrationTestPath` 全部用例 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | 文件系统 | 通过 `fs.mkdtempSync` 创建临时项目 fixture，测试结束后 `fs.rmSync` 清理；不 mock `fs` | `resolveTestPaths` / `runTestResolvePaths` 集成路径用例 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | 无 | Zod schema 纯数据验证，直接 `safeParse` 断言 | 所有 schema 测试用例 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | 无 | 使用 `re` 读取 `test-design-planner.md`、`mcp.ts`、`test-resolve-paths.schema.ts` 进行静态文本匹配 | Agent 与 MCP schema 静态检查 |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1~AC-6 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `runTestResolvePaths` -- integration_root 端到端 | 传入 `modules`、`integration_scenarios`、`integration_root`，`project_root` 指向临时目录，返回结构与 `resolveTestPaths` 一致 | 新增 |
| AC-8 | `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | MCP handler 透传 integration_root | `mcp.ts` handler 通过 `{ ...args }` spread 调用 `runTestResolvePaths`，源文件含 `integration_root` 透传路径 | 新增 |
| AC-10 | — | 构建验证 | `plugins/dev-team/bin` 下 `npm run test` 全部通过 | 新增 |
| AC-10 | — | 构建验证 | `plugins/dev-team/bin` 下 `npm run build`（`vp pack`）无错误 | 新增 |
| — | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | 回归：既有 test-resolve-paths 用例 | 未传 `integration_root` 时 AC-1~AC-9（add-test-path-resolver-api）行为不变 | 废弃 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | 文件系统 | 真实临时目录 fixture | `runTestResolvePaths` 端到端编排 |
| `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py` | 无 | 静态源文件读取 | MCP 注册与 Agent Process 静态检查 |

---

## 不可测试项

- **`plugins/dev-team/bin/src/mcp.ts` handler 运行时透传** — **原因**: AC-8 验收方法为代码审查；handler 已通过 `{ ...args }` spread 透传，行为由 schema 单元测试与 Python 静态检查间接覆盖，无需新建 `mcp.test.ts`（MCP 推导的 colocated 路径对应文件尚不存在）。
- **`plugins/dev-team/agents/test-design-planner.md`** — **原因**: `test_resolve_paths` 返回 `Not a testable source file`；Agent prompt 行为由 AC-9 Python 静态检查验证，非 colocated 单元测试。
- **`plugins/dev-team/.claude-plugin/plugin.json` 版本号升级** — **原因**: 版本号变更无业务逻辑，由人工发布流程确认。
- **AC-10 本地 `npm run build` 无错误** — **原因**: 构建结果为环境依赖的集成验证，由 CI 或实施阶段手动执行；test-design 仅记录验证项。
- **`test_detect_frameworks` 与 `test_get_framework_config`** — **原因**: 本变更明确不修改二者行为；回归由全量 `npm run test` 间接保证。
