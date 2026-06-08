# 任务: per-directory-test-execution

> **变更**: per-directory-test-execution
> **日期**: 2026-06-08

---

## 阶段一：实现 `deriveWorkingDirectory` 纯函数

- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` 中新增导出的 `deriveWorkingDirectory(glob: string): string` 函数
  - 实现规则：查找第一个通配符（`*`、`?`、`{`）之前的路径前缀作为工作目录
  - 无通配符时返回整个 glob 字符串
  - 通配符在首个位置时返回 `"."`
  - POSIX 风格路径分隔符归一化（Windows 反斜杠替换为正斜杠）
- [x] 为 `deriveWorkingDirectory` 编写单元测试（覆盖 AC-2 至 AC-5）：
  - AC-2: `"plugins/dev-team/bin"` 返回 `"plugins/dev-team/bin"`（无通配符）
  - AC-3: `"src/**/*.test.ts"` 返回 `"src"`（第一个通配符前路径）
  - AC-4: `"**/*.test.ts"` 返回 `"."`（通配符在起始位置）
  - AC-5: `"{src,lib}/*.test.ts"` 返回 `"."`（`{` 被视为通配符）
  - `"tests/?nit/*.test.ts"` 返回 `"tests"`（`?` 通配符）
  - `"packages/*/src/__tests__/*.test.ts"` 返回 `"packages"`（`*` 通配符）
  - `"plugins\\dev-team\\bin"` 返回 `"plugins/dev-team/bin"`（Windows 路径归一化）

## 阶段二：扩展 `test_detect_frameworks` 输出类型和 schema

- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` 中新增 `PlanEntry` 接口：
  - `directory: string` — 执行工作目录
  - `framework: string` — 框架名称
  - `coverage_cmd: string` — 覆盖率命令
  - `coverage_format: "istanbul" | "llvm-cov"` — 覆盖率格式
  - `coverage_output: string` — 覆盖率输出路径
- [x] 扩展 `TestDetectFrameworksResult` 接口，新增 `plan: PlanEntry[]` 字段
- [x] 扩展 `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` 的 `testDetectFrameworksOutputSchema`，新增 `plan` 数组：
  - 每个条目包含 `directory`、`framework`、`coverage_cmd`、`coverage_format`、`coverage_output`
  - 使用 Zod 的 `z.object` 定义嵌套结构
- [x] 验证扩展后的 schema 导出正确：更新 `plugins/dev-team/bin/src/schemas/index.ts` 中的导出（若需）

## 阶段三：修改 `runTestDetectFrameworks` 生成 `plan`

- [x] 在 `runTestDetectFrameworks` 函数中，对归一化后的 `test.frameworks` 配置逐条目生成 `plan`：
  - 调用 `deriveWorkingDirectory(glob)` 获取 `directory`
  - 调用 `runTestGetFrameworkConfig({framework})` 获取 `coverage_cmd`、`coverage_format`、`coverage_output`
  - 组装 `PlanEntry` 并加入 `plan` 数组
- [x] 处理边界情况：
  - 当 `test.frameworks` 未配置或为空数组时，`plan` 为空数组（AC-10）
  - 当 `test.frameworks` 为字符串简写时（如 `"vitest"`），默认 glob 的 `directory` 应为 `"."`（AC-9）
- [x] 确保 `plan` 数组顺序与 `test.frameworks` 配置顺序一致

## 阶段四：更新 `unit-test-executor.md` Agent 指南

- [x] 删除原步骤 2（命令解析——调用 `test_get_framework_config` 的说明）
- [x] 删除原步骤 3（单独运行 `test_cmd` 的说明）
- [x] 删除原步骤 4（单独运行 `coverage_cmd` 的说明）
- [x] 新增合并步骤：Executor 从 `test_detect_frameworks` 返回的 `plan` 中直接获取执行计划
- [x] 新增步骤说明：Executor 遍历 `plan` 数组，在每个条目的 `directory` 下执行 `coverage_cmd`
- [x] 更新约束和场景描述以反映新流程
- [x] 添加步骤更改对照表（旧流程 vs 新流程）
- [x] 删除或更新末尾约束中关于 `test_get_framework_config` 的引用

## 阶段五：编写集成测试覆盖 `plan` 输出

- [x] 扩展 `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`，新增 `describe('plan generation')` 测试套件
- [x] AC-1: 测试 `test_detect_frameworks({})` 返回的 JSON 中 `plan` 字段通过 Zod schema 验证
- [x] AC-6: 配置 `[{glob: "plugins/dev-team/bin", framework: "vite-plus"}]` 时，验证 `plan[0]` 包含预期的 `directory`、`coverage_cmd`、`coverage_format`、`coverage_output`
- [x] AC-9: 配置为字符串简写 `"vitest"` 时，`plan` 包含 `{directory: ".", framework: "vitest"}` 且命令与注册表一致
- [x] AC-10: 配置未设置或为空数组时，`plan` 为空数组
- [x] AC-11: 确认现有所有单元测试全部通过（`detected` 和 `frameworks` 字段不变）

## 阶段六：验证与清理

- [x] 运行全部现有测试，确认无回归（AC-11）
- [x] 手动审核 `mcp.ts` 中 `test_detect_frameworks` 工具的注册代码，确认输出 schema 变更不会破坏 MCP 协议
- [x] 确认 `test_get_framework_config` 工具及其 MCP 入口未修改
- [x] 确认 `coverage-parser.ts` 和 `coverage-calculator.ts` 未修改
- [x] 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（如 `2.5.10` -> `2.6.0`）
