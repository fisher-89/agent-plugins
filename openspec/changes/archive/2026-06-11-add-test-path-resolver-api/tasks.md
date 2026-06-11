# 实施任务: add-test-path-resolver-api

---

## 阶段 1: Schema 定义

- [x] 新建 `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts`，导出：
  - `testResolvePathsInputSchema`：`modules: z.array(z.string()).min(1)`（必填）、`integration_scenarios`、`extension`、`project_root`（均可选）
  - `testResolvePathsOutputSchema`：`unit_tests`、`integration_tests`、`errors` 三个数组字段（均可为空数组，但字段必须存在）
- [x] 修改 `plugins/dev-team/bin/src/schemas/index.ts`，重新导出上述 schema

## 阶段 2: 核心实现

- [x] 新建 `plugins/dev-team/bin/src/commands/test-resolve-paths.ts`，定义类型 `ResolveTestPathsParams`、`ResolveTestPathsResult`、`UnitTestEntry`、`IntegrationTestEntry`、`ResolveError`
- [x] 实现辅助纯函数（可导出以便测试）：
  - `toPosixRelativePath(absolutePath, projectRoot)` — POSIX 相对路径
  - `isWithinProjectRoot(resolvedPath, projectRoot)` — 路径穿越检测
  - `isTestFile(filePath)` — 识别 `*.test.*`、`test_*.py`、`*_test.go`、`*_test.rs`
  - `isSourceFile(filePath)` — 扩展名在 `.ts/.tsx/.js/.jsx/.mjs/.cjs/.py/.go/.rs` 集合内
  - `deriveUnitTestPath(sourcePath)` — 按语言规则推导同级单元测试路径（AC-1~AC-5）
  - `deriveIntegrationTestPath(scenario, ext)` — 生成 `__tests__/<scenario>/<scenario>.test.<ext>`
  - `normalizeExtension(ext)` — 去除前导 `.`，小写化
  - `inferExtension(sourceFiles, explicitExtension?)` — 显式优先，否则众数，否则 `ts`
- [x] 实现 `expandDirectory(dirPath, projectRoot)` — 深度优先递归遍历，跳过排除目录/文件（spec 规定列表），返回源文件相对路径数组
- [x] 实现 `resolveTestPaths(params)` — 编排 modules 处理、目录展开、单元/集成路径推导、errors 收集、排序去重
- [x] 实现 `runTestResolvePaths(args)` — 解析 `project_root`（默认 `process.env.CLAUDE_PROJECT_DIR || process.cwd()`），委托 `resolveTestPaths`

## 阶段 3: MCP 注册

- [x] 修改 `plugins/dev-team/bin/src/mcp.ts`：
  - import `runTestResolvePaths` 与 schema
  - 注册 `test_resolve_paths` 工具，description 说明模块→测试路径推导用途
  - handler 调用 `resolveProjectRoot(args.project_root)` 后执行 `runTestResolvePaths`
- [x] 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（代码变更后必做）

## 阶段 4: Agent 集成

- [x] 修改 `plugins/dev-team/agents/test-design-planner.md` Process 章节，在 Grep 源码之后、写入 test-design.md 之前插入步骤：
  1. 从 design.md 变更范围与 Grep 结果汇总精确 `modules` 列表
  2. 从 proposal/design 识别 `integration_scenarios`（若有）
  3. 调用 `mcp__plugin_dev-team_dev-team__test_resolve_paths`，传入 `modules` 与可选参数
  4. 将 `unit_tests` 映射到 `验收范围` 与 `单元测试 > 用例` 的 `测试文件` 列
  5. 将 `integration_tests` 映射到 `集成测试 > 用例` 的 `测试文件` 列
  6. 将 `errors` 记入 `不可测试项` 章节
- [x] 在 Constraints 中明确：**禁止**手工拼接或猜测测试文件路径；`测试文件` 列 MUST 来自 MCP 返回值
- [x] 确认 `phase-test-design/SKILL.md` 无需修改（薄编排层不变）

## 阶段 5: 单元测试 — `resolveTestPaths` 核心逻辑

在 `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` 中，使用临时目录 fixture 编写测试：

### 单元测试路径推导（AC-1~AC-5）

- [x] AC-1：`src/config.ts` → `src/config.test.ts`
- [x] AC-2：`src/component/Button.tsx` → `src/component/Button.test.tsx`
- [x] AC-3：`src/auth.py` → `src/test_auth.py`
- [x] AC-4：`src/handler.go` → `src/handler_test.go`
- [x] AC-5：`src/lib.rs` → `src/lib_test.rs`

### 目录展开（AC-6）

- [x] 传入目录 `src/commands/`，内含 `foo.ts` 与 `bar.ts`，返回两条 `unit_tests` 且路径正确
- [x] 验证 `src/node_modules/pkg/index.ts` 被跳过（spec 场景）
- [x] 验证已有测试文件（如 `foo.test.ts`）不被当作源文件展开
- [x] 验证 `unit_tests` 按 `source` 字典序排序且去重

### 集成测试路径（AC-7、AC-8）

- [x] AC-7：`integration_scenarios: ["api-flow"]` + `extension: "ts"` → `__tests__/api-flow/api-flow.test.ts`
- [x] AC-8：仅 `modules: ["src/auth.py"]` + `integration_scenarios: ["db-roundtrip"]`（无 extension）→ `__tests__/db-roundtrip/db-roundtrip.test.py`
- [x] 无有效源文件时集成测试默认 `ts` 扩展名（spec 场景：`README.md` 写入 errors，integration 仍为 `.test.ts`）

### 错误收集（AC-9）

- [x] 不存在路径与有效路径混合：errors 含缺失项，unit_tests 仍含有效条目
- [x] 非源文件（`README.md`）写入 errors，unit_tests 为空
- [x] 路径穿越（`../../../outside.ts`）写入 errors
- [x] `errors` 按 `path` 字典序排序

### 纯函数确定性

- [x] `deriveUnitTestPath` 相同输入产生相同输出（不依赖 fs）

## 阶段 6: Schema 单元测试（AC-10、AC-11）

在 `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` 中：

- [x] AC-10：`modules: []` 输入被 input schema 拒绝
- [x] 有效完整输入/输出对象通过 schema 验证
- [x] 输出缺少 `unit_tests` / `integration_tests` / `errors` 任一字段时被拒绝
- [x] `extension` 带前导 `.`（如 `".ts"`）的输入通过 schema（规范化在 command 层处理）

## 阶段 7: Agent 静态检查（AC-12）

- [x] 新建 `openspec/changes/add-test-path-resolver-api/tests/test_agent_planner_resolve_paths.py`（或等效静态测试），验证 `test-design-planner.md`：
  - Process 包含 `test_resolve_paths` / `mcp__plugin_dev-team_dev-team__test_resolve_paths` 调用说明
  - 包含禁止手工拼接测试路径的约束
  - 包含将 `unit_tests` / `integration_tests` 映射到 `测试文件` 列的说明
  - 包含 `errors` → `不可测试项` 的处理说明

## 阶段 8: 构建与验证（AC-13）

- [x] 在 `plugins/dev-team/bin` 下运行 `npm run test`（`vp test`），全部通过
- [x] 在 `plugins/dev-team/bin` 下运行 `npm run build`（`vp pack`），无错误
- [x] 确认 `test_detect_frameworks` 与 `test_get_framework_config` 现有测试无回归
