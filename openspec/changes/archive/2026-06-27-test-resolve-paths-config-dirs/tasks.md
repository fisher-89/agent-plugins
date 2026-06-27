# 任务: test-resolve-paths-config-dirs

> **变更**: test-resolve-paths-config-dirs
> **日期**: 2026-06-26

---

## 阶段 1: Schema 修改

- [x] 修改 `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts`：
  - 将 `modules` 从 `z.array(z.string()).min(1)` 改为 `z.union([z.array(z.string()), z.literal("git-change")])`
  - 更新 `modules` 的 `describe` 文本，反映三种模式：空数组自动推导、`"git-change"` 读取 git diff、非空数组 config 过滤
- [x] 更新 `TestResolvePathsInput` TypeScript 类型签名（`plugins/dev-team/bin/src/commands/test-resolve-paths.ts`）

## 阶段 2: 核心逻辑实现 — modules 分发

- [x] 在 `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` 的 `resolveTestPaths` 中增加 `modules` 类型分发逻辑：
  - 若 `modules === "git-change"` → 执行 `git diff HEAD --name-only`（使用 `execSync`），解析输出得到文件列表作为 `effectiveModules`
  - 若 `Array.isArray(modules) && modules.length > 0` → `effectiveModules = modules`
  - 若 `modules` 为空数组 → 进入 config-driven 扫描分支

## 阶段 3: 核心逻辑实现 — test config 过滤（非空 modules）

- [x] 当 `effectiveModules` 非空时，调用 `runTestDetectFrameworks({ files: effectiveModules, projectRoot })` 获取检测结果
- [x] 仅对 `result.detected` 中的文件调用 `isSourceFile()` + `deriveUnitTestPath()` + `addUnitTest()`
- [x] 将被过滤掉的 `effectiveModules` 文件（不在 `detected` 中）写入 `errors`，message 为 `"Not in test config scope"`

## 阶段 4: 核心逻辑实现 — config-driven 目录扫描（空 modules）

- [x] modules 为空数组时，调用 `runTestDetectFrameworks({ projectRoot })` 获取执行计划
- [x] 从返回的 `plan` 中提取唯一 `directory` 值列表
- [x] 实现本地 `collectFiles` 递归扫描函数（排除 node_modules、.git、.claude、dist、build、target、.vp、coverage、.nyc_output）
- [x] 使用 `isSourceFile()` 过滤出可测试源文件
- [x] 跨目录去重源文件列表（基于源文件路径的 POSIX 形式）
- [x] 对扫描到的源文件调用 `deriveUnitTestPath()` 生成单元测试路径
- [x] 处理 `plan` 为空的情况（无 test.framework 且无 test.overrides）：
  - 在 `errors` 中添加指导性错误消息
  - 返回空的 `unit_tests` 和 `integration_tests`

## 阶段 5: "git-change" 错误处理

- [x] `git diff HEAD --name-only` 执行使用 try-catch 包裹
- [x] 失败时在 `errors` 中添加 `{ path: "git", message: <错误信息> }`，返回空 `unit_tests`

## 阶段 6: MCP 描述更新

- [x] 修改 `plugins/dev-team/bin/src/mcp.ts` 中 `test_resolve_paths` 工具的 `description`，反映三种模式

## 阶段 7: 验证

- [x] 确认 `modules: "git-change"` 在 git 仓库中返回变更文件的测试路径（仅 test config 覆盖范围内的）
- [x] 确认 `modules: "git-change"` 在非 git 仓库中返回 git 错误
- [x] 确认 `modules: ["src/config.ts", "scripts/not-in-scope.ts"]` 仅返回 test config 覆盖范围内的文件
- [x] 确认 `modules: []` 且 config.json 配置了 `test.overrides` 时，`unit_tests` 包含该目录下的源文件推导结果
- [x] 确认 `modules: []` 且 config.json 既无 `test.framework` 也无 `test.overrides` 时，`errors` 包含指导性错误消息
- [x] 确认 schema 接受 `{modules: []}`、`{modules: "git-change"}`、`{modules: ["src/foo.ts"]}`，拒绝 `{modules: 123}` 等非法值
