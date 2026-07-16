# 任务: 移除 test_resolve_paths 集成测试路径解析功能

> **变更**: remove-integration-test-path-resolution
> **日期**: 2026-07-15

---

## Phase 1: Schema 层清理

- [x] 从 `testResolvePathsInputSchema` 移除 `integration_scenarios` 字段
- [x] 从 `testResolvePathsInputSchema` 移除 `integration_root` 字段
- [x] 从 `testResolvePathsInputSchema` 移除 `extension` 字段
- [x] 从 `testResolvePathsOutputSchema` 移除 `integration_tests` 字段
- [x] 更新 `testResolvePathsInputSchema`/`testResolvePathsOutputSchema` 的英文注释（不再提及集成测试功能）

## Phase 2: 核心逻辑清理

- [x] 从 `test-resolve-paths.ts` 移除 `IntegrationTestEntry` 接口定义
- [x] 从 `test-resolve-paths.ts` 移除纯函数：`normalizeIntegrationRoot`、`isValidIntegrationRoot`、`deriveIntegrationTestPath`、`normalizeExtension`、`inferExtension`
- [x] 从 `test-resolve-paths.ts` 移除 `resolveIntegrationTests` 函数
- [x] 从 `ResolveTestPathsParams` 接口移除 `integrationScenarios`、`extension`、`integrationRoot` 字段
- [x] 从 `ResolveTestPathsResult` 接口移除 `integration_tests` 字段
- [x] 从 `TestResolvePathsInput` 接口移除 `integration_scenarios`、`extension`、`integration_root` 字段
- [x] 更新 `resolveTestPaths` 函数：移除 `resolveIntegrationTests` 调用，移除 `integration_tests` 返回值组装
- [x] 更新 `runTestResolvePaths` 函数：移除 `integrationScenarios`、`extension`、`integrationRoot` 参数传递

## Phase 3: MCP 注册层更新

- [x] 更新 `registerTestResolvePathsTool` 的 description 文本，移除集成测试路径相关说明（"integration"、"__tests__/<scenario>/" 等）

## Phase 4: 测试文件清理

- [x] 移除 `test-resolve-paths.test.ts` 中 `describe('runTestResolvePaths -- 集成测试路径', ...)` 整个 describe 块（约 lines 181-284）
- [x] 移除 `test-resolve-paths.test.ts` 中 `describe('runTestResolvePaths -- integration_root 向后兼容', ...)` 整个 describe 块（约 lines 526-546）
- [x] 移除 `test-resolve-paths.test.ts` 中 `describe('runTestResolvePaths -- integration_root 为点号', ...)` 整个 describe 块（约 lines 549-570）
- [x] 移除 `test-resolve-paths.test.ts` 中 `describe('runTestResolvePaths -- integration_root 子目录前缀', ...)` 整个 describe 块（约 lines 573-617）
- [x] 移除 `test-resolve-paths.test.ts` 中 `describe('runTestResolvePaths -- integration_root 与 unit_tests 隔离', ...)` 整个 describe 块（约 lines 619-671）
- [x] 移除 `test-resolve-paths.test.ts` 中 `describe('runTestResolvePaths -- integration_root 路径穿越', ...)` 整个 describe 块（约 lines 673-695）
- [x] 移除 `test-resolve-paths.test.ts` 中 `describe('runTestResolvePaths -- integration_root snake_case 映射', ...)` 整个 describe 块（约 lines 697-740）
- [x] 更新 `test-resolve-paths.test.ts` 中 `describe('runTestResolvePaths -- 端到端编排', ...)` 内引用 `integration_scenarios`/`integration_tests` 的测试用例（约 lines 472-519），移除包含集成测试参数的集成测试
- [x] 确认 `mcp.test.ts` 工具注册个数验证保持 10

## Phase 5: Subagent 提示更新

- [x] 更新 `test-design-planner.md` Step 7：不再识别"场景名列表"
- [x] 更新 `test-design-planner.md` Step 8：移除 `integration_root`/`integration_scenarios` 说明
- [x] 更新 `test-design-planner.md` Step 9：删除整个"集成测试路径"段落（原 lines 20-24）
- [x] 更新 `test-design-planner.md` Step 11：删除"将合并后的 integration_tests 映射到集成测试表格"（原 line 26）
- [x] 更新 `test-design-planner.md` Step 12：仅提及单元测试 errors
- [x] 更新 `test-design-planner.md` Constraints 第 4 条（原 line 40）：放宽为仅单元测试路径 MUST 来自 `test_resolve_paths`
- [x] 更新 `test-design-planner.md` Constraints 第 5 条（原 line 41）：`test_resolve_paths` 仅用于单元测试路径推导

## Phase 6: 编译验证与残留检查

- [x] 运行 TypeScript 编译确保无类型错误（`npx tsc --noEmit`）
- [x] 全局 grep 检查 `IntegrationTestEntry`、`integration_tests`、`integration_scenarios`、`integration_root`、`IntegrationScenarios`、`IntegrationRoot` 等标识符无残留引用
