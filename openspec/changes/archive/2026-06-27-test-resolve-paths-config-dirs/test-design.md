# 测试设计: test-resolve-paths-config-dirs

> **日期**: 2026-06-26

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | 当 `modules: []` 且 config.json 中配置了 `test.overrides`（如 `plugins/dev-team/bin` → `vite-plus`）时，`unit_tests` 包含该目录下的源文件推导结果 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 自动扫描 |
| AC-1 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/config-driven-auto-scan/config-driven-auto-scan.test.ts` | 空 modules 时自动调用 test_detect_frameworks 获取 plan[].directory 并扫描 |
| AC-2 | 当 `modules: []` 且 config.json 中既无 `test.framework` 也无 `test.overrides` 时，`errors` 包含提示用户配置 framework 的消息，`unit_tests` 为空 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 自动扫描 |
| AC-2 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/no-test-config/no-test-config.test.ts` | 无 test 配置时 errors 包含指导性消息 |
| AC-3 | 当 `modules: ["src/config.ts", "scripts/not-in-test-scope.ts"]` 且 test config 仅覆盖 `src/` 时，`unit_tests` 仅包含 `src/config.ts` 的推导结果 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 过滤 |
| AC-3 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/config-driven-filter/config-driven-filter.test.ts` | 非空 modules 时经 test_detect_frameworks 过滤后再推导 |
| AC-4 | `testResolvePathsInputSchema` 接受 `{modules: []}` 和 `{modules: "git-change"}`，拒绝 `{modules: 123}` 等非法类型 | 单元测试 | `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 |
| AC-5 | 当 `modules: "git-change"` 且 `git diff HEAD --name-only` 返回 `["src/foo.ts"]` 时，`unit_tests` 包含 `src/foo.ts` 的推导结果 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 |
| AC-5 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/git-change/git-change.test.ts` | git diff 变更文件经 config 过滤后推导测试路径 |
| AC-6 | 当多个 override 指向同一目录，或 `modules` 和 git diff 产生重复文件时，`unit_tests` 中的源文件不重复 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 去重 |
| AC-6 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/dedup/dedup.test.ts` | 跨目录/跨 source 的扫描结果去重 |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 正向 | `modules: []` 空数组应通过 schema 验证（原 .min(1) 拒绝 → 现允许） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 正向 | `modules: "git-change"` 字符串字面量应通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 正向 | `modules: ["src/a.ts"]` 常规数组仍应通过验证（向后兼容） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 异常 | `modules: 123` 非法数字类型应被拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 异常 | `modules: true` 布尔类型应被拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 异常 | `modules: { key: "val" }` 对象类型应被拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 异常 | `modules` 字段缺失时应被拒绝（回归） | 废弃 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 边界 | `modules: [""]` 空字符串数组应通过 schema（路径校验在 command 层） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` | testResolvePathsInputSchema -- union 类型 | 边界 | `modules: "GIT-CHANGE"` 大小写不匹配的字面量应被拒绝 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 自动扫描 | 正向 | `modules: []` 且 `test.overrides` 配置有效时，`unit_tests` 包含扫描到的源文件推导结果 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 自动扫描 | 正向 | `modules: []` 时 `runTestDetectFrameworks` 被调用（通过 spy 验证） | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 自动扫描 | 异常 | `modules: []` 且 plan 为空时 `errors` 包含 "No test configuration" 提示，`unit_tests` 为空 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 过滤 | 正向 | `modules: ["src/config.ts"]` 在 test config 覆盖范围内时正常返回 `unit_tests` | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 过滤 | 异常 | `modules: ["scripts/not-covered.ts"]` 不在 test config 范围内时该文件不出现在 `unit_tests` 中，`errors` 包含跳过的提示 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- config-driven 过滤 | 边界 | `modules` 混合范围内外文件，仅范围内的文件进入 `unit_tests` | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 正向 | `modules: "git-change"` 且 git diff 返回变更文件时，返回对应 `unit_tests` | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 异常 | `modules: "git-change"` 且 git 命令失败时（非 git 仓库），`errors` 包含错误消息 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 异常 | `modules: "git-change"` 返回的变更文件均不在 test config 范围内时 `unit_tests` 为空 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 异常 | `modules: "git-change"` 且 git diff 返回空（无变更）时 `unit_tests` 为空 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- git-change 模式 | 边界 | `modules: "git-change"` 且 git diff 返回大量文件（100+）时不应抛错 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 去重 | 正向 | 多个 override 指向同一目录时扫描结果去重 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 去重 | 正向 | `modules` 包含重复文件路径时 `unit_tests` 去重 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths -- 去重 | 边界 | config-driven 扫描与 `modules` 显式传入产生重叠时去重 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `runTestDetectFrameworks` | 使用 `vi.mock` 或 `vi.spyOn` 拦截 `test-detect-frameworks` 模块的 `runTestDetectFrameworks` 函数。空 modules 场景返回 mock plan 条目；过滤场景返回对应 detected 结果；无配置场景返回空 plan | config-driven 自动扫描、config-driven 过滤、无 test 配置 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `child_process.execSync` | 使用 `vi.spyOn` 拦截 `execSync`。`"git-change"` 场景返回预设的 git diff 输出；非 git 仓库场景抛出异常 | git-change 模式 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `readConfig` / `openspec/config.json` | 通过 mock `runTestDetectFrameworks` 间接控制，使其返回不同的 plan/detected 结果来模拟不同配置状态 | 全部 config-driven 场景 |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1 | `plugins/dev-team/bin/__tests__/config-driven-auto-scan/config-driven-auto-scan.test.ts` | 空 modules 时自动扫描 | `modules: []` 且 config.json 含 `test.overrides` 时，`unit_tests` 包含 `plugins/dev-team/bin/src/` 下源文件的推导结果 | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/config-driven-auto-scan/config-driven-auto-scan.test.ts` | 空 modules 时自动扫描 | `modules: []` 时 `runTestDetectFrameworks` 被真实调用且 `plan[].directory` 被用作扫描根目录 | 新增 |
| AC-2 | `plugins/dev-team/bin/__tests__/no-test-config/no-test-config.test.ts` | 无 test 配置 | 临时移除 config.json 的 `test` 字段后调用 `modules: []`，`errors` 包含指导配置消息 | 新增 |
| AC-3 | `plugins/dev-team/bin/__tests__/config-driven-filter/config-driven-filter.test.ts` | 非空 modules 经 config 过滤 | `modules: ["src/commands/test-resolve-paths.ts", "node_modules/some-dep/index.ts"]` 时仅前者进入 `unit_tests` | 新增 |
| AC-5 | `plugins/dev-team/bin/__tests__/git-change/git-change.test.ts` | git diff 变更文件推导 | 在工作树中有未暂存变更时调用 `modules: "git-change"`，`unit_tests` 包含变更文件的推导结果 | 新增 |
| AC-5 | `plugins/dev-team/bin/__tests__/git-change/git-change.test.ts` | git diff 为空 | 干净的工作树中调用 `modules: "git-change"`，`unit_tests` 为空 | 新增 |
| AC-5 | `plugins/dev-team/bin/__tests__/git-change/git-change.test.ts` | git diff 失败 | 非 git 目录中调用 `modules: "git-change"`，`errors` 包含 git 错误 | 新增 |
| AC-6 | `plugins/dev-team/bin/__tests__/dedup/dedup.test.ts` | 跨目录去重 | 当 `test.overrides` 中多个条目指向同一目录时，`unit_tests` 不包含重复条目 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/no-test-config/no-test-config.test.ts` | `openspec/config.json` | 测试前将 config.json 的 `test` 字段临时清空（备份后置空，测试后恢复），验证无配置时的 errors 行为 | 无 test 配置 |
| `plugins/dev-team/bin/__tests__/git-change/git-change.test.ts` | 工作树状态 | 依赖真实 git diff：创建临时文件做 unstaged 变更后调用 `"git-change"`，或通过 `git init` 在空仓库中测试 | git-change 模式 |

---

## 不可测试项

- `AC-3` 中被过滤出 `unit_tests` 的文件是否精确写入 `errors` 的消息格式 — 消息文案属开发阶段可调整内容，验收条件未要求精确匹配错误消息字符串，由单元测试覆盖行为语义而非文案。
- MCP 工具 `test_resolve_paths` 的 description 文本更新 — 描述文本变更仅影响 MCP 注册的元数据，无运行时可断言的行为变化，无需为此单独编写测试。
