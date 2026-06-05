# 任务: unit-test-coverage-report

> **变更**: unit-test-coverage-report
> **日期**: 2026-06-05

---

## 阶段 1: Config Schema 扩展

此阶段扩展 Zod schema，为 openspec/config.json 增加 `test` 配置节点的验证规则。后续阶段依赖此 schema 确保配置合法性。

- [x] **1.1** 在 `plugins/dev-team/bin/src/schemas/config.schema.ts` 中，于 `.passthrough()` 之前扩展 `z.object({})`，增加 `test` 可选字段，其类型为 `z.object({...})`，包含 `frameworks`（`string | {glob, framework}[]` 联合类型）、`coverage`（可选，含 `thresholds` 和 `overrides`）
- [x] **1.2** 在 `test` 节点的 `frameworks` 字段中实现字符串简写：使用 `z.union([z.enum(["jest", "vitest", "vite-plus", "bun", "rust"]), z.array(z.object({glob: z.string(), framework: z.string()}))])`，确保字符串值受 `z.enum()` 约束
- [x] **1.3** 在 `test` 节点的 `coverage.thresholds` 字段中实现三个维度：`lines: z.number().default(80)`, `branches: z.number().default(70)`, `functions: z.number().default(75)`
- [x] **1.4** 在 `test` 节点的 `coverage.overrides` 字段中实现：`z.array(z.object({glob: z.string(), thresholds: z.object({lines: z.number().optional(), branches: z.number().optional(), functions: z.number().optional()})})).optional()`，缺失维度继承全局默认值的逻辑由运行时覆盖
- [x] **1.5** 确认 `OpenSpecConfig` 类型自动从扩展后的 schema `z.infer<>` 推导，无需手动更新类型定义
- [x] **1.6** 运行 `pnpm run -C ./plugins/dev-team/bin check` 确保编译通过，检查现有 config.json（当前无 `test` 节点）仍可通过 `.passthrough()` 验证

## 阶段 2: MCP 工具 Input/Output Schema

此阶段为两个新 MCP 工具定义输入/输出 Zod schema，遵循现有 schema 文件命名和导出模式。

- [x] **2.1** 新建 `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts`，导出 `testDetectFrameworksInputSchema`（`{files?: z.array(z.string())}`）和 `testDetectFrameworksOutputSchema`（`{detected: z.array(z.object({file: z.string(), framework: z.string()})), frameworks: z.array(z.string())}`）
- [x] **2.2** 新建 `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.ts`，导出 `testGetFrameworkConfigInputSchema`（`{framework: z.string()}`）和 `testGetFrameworkConfigOutputSchema`（`{framework: z.string(), test_cmd: z.string(), coverage_cmd: z.string(), coverage_format: z.string(), coverage_output: z.string()}`）
- [x] **2.3** 更新 `plugins/dev-team/bin/src/schemas/index.ts`，导出两个新 schema 文件中的四个 schema（两个 input、两个 output）

## 阶段 3: MCP 工具实现

此阶段实现两个 MCP 工具的核心逻辑，包括框架检测、命令注册表查询和覆盖率解析。这是整个变更的核心工程代码。

- [x] **3.1** 新建 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts`，实现 `runTestDetectFrameworks(options: {files?: string[], projectRoot?: string})` 函数：
  - 读取 openspec/config.json 的 `test.frameworks` 字段
  - 若 `test.frameworks` 为字符串，归一化为 `[{glob: "<默认glob>", framework: "<字符串值>"}]`，默认 glob 根据框架名选择（jest/vitest/vite-plus/bun → `**/*.{test,spec}.{js,ts,jsx,tsx}`, rust → `**/tests/**/*.rs`）
  - 若未提供 files 参数，使用 Glob 自动扫描匹配所有 glob 模式的测试文件
  - 按 `test.frameworks` 数组顺序对每个文件执行首匹配
  - 返回 `{detected: {file, framework}[], frameworks: string[]}`
- [x] **3.2** 新建 `plugins/dev-team/bin/src/commands/test-get-framework-config.ts`，实现框架命令注册表（硬编码）和 `runTestGetFrameworkConfig(options: {framework: string})` 函数：
  - 定义 Map 或 Record，包含 jest/vitest/vite-plus/bun/rust 五个框架的 test_cmd、coverage_cmd、coverage_format、coverage_output
  - 未知框架名称返回错误
  - 返回匹配的框架配置对象
- [x] **3.3** 新建 `plugins/dev-team/bin/src/lib/coverage-parser.ts`，实现覆盖率输出解析函数 `parseCoverageOutput(filePath: string, format: "istanbul" | "llvm-cov"): {lines: number, branches: number, functions: number} | null`：
  - `istanbul` 格式：从 `coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 读取
  - `llvm-cov` 格式：从 JSON 输出的 `data[0].totals.lines.percent`、`data[0].totals.branches.percent`、`data[0].totals.functions.percent` 读取
  - 文件不存在或 JSON 解析失败时返回 null
- [x] **3.4** 新建 `plugins/dev-team/bin/src/lib/coverage-calculator.ts`，实现：
  - `computeWeightedAverage(coverageByFramework: {framework, coverage, sourceFileCount}[]): {lines, branches, functions}` — 按源文件数加权平均
  - `checkCoveragePass(coverage: {lines, branches, functions}, thresholds: {lines, branches, functions}, overrides: {glob, thresholds, coverage}[]): boolean` — ALL 逻辑判定
    - 全局：lines >= thresholds.lines AND branches >= thresholds.branches AND functions >= thresholds.functions
    - 每个 override 条目：匹配目录独立判定，缺失维度继承全局阈值
  - `buildCoverageOverrides(coverageByFramework, overrides, config): {glob, thresholds, coverage, pass}[]` — 构建 override 校验结果数组
- [x] **3.5** 更新 `plugins/dev-team/bin/src/mcp.ts`：
  - 在两个 `registerTool` 调用中注册 `test_detect_frameworks` 和 `test_get_framework_config`
  - `test_detect_frameworks` 的 handler 调用 `runTestDetectFrameworks`，传入文件列表和 projectRoot（通过 `resolveProjectRoot` 获取）
  - `test_get_framework_config` 的 handler 调用 `runTestGetFrameworkConfig`，传入 framework 名称
  - 两个工具的 inputSchema 和 outputSchema 使用阶段 2 中定义的 schema
- [x] **3.6** 运行 `pnpm run -C ./plugins/dev-team/bin check` 确保编译通过
- [x] **3.7** 运行 `pnpm run -C ./plugins/dev-team/bin test`（如有）确保现有测试不中断

## 阶段 4: Agent 文件更新

此阶段更新 executor、evaluator 和 test-gen-generator agent 的 Markdown 指令文件，注入框架检测、覆盖率执行、扩展评估逻辑和测试框架感知能力。Agent 文件是 LLM 执行的指令，需要清晰、结构化的步骤描述。

- [x] **4.0** 更新 `plugins/dev-team/agents/test-gen-generator.md`：
  - 在 Input 章节增加：读取 `openspec/config.json` 的 `test.frameworks` 配置
  - 在 Process 中增加 Step "Framework detection": 调用 `test_detect_frameworks` MCP 工具检测项目的测试框架列表
  - 增加 Step "Framework config": 调用 `test_get_framework_config` 获取框架的 test_cmd，了解测试运行方式
  - 在生成测试文件时，根据检测到的框架选择正确的测试语法和断言风格（jest 用 `describe/it`+`expect`，vitest 用 `describe/it`+`expect`/`vi`，bun 用 `describe/test`+`expect`，rust 用 `#[cfg(test)]`+`#[test]`）
  - 根据 `test_get_framework_config` 返回的 `test_cmd` 确定测试文件命名和放置约定
  - 在工具列表中新增 `test_detect_frameworks` 和 `test_get_framework_config` MCP 工具引用
- [x] **4.1** 更新 `plugins/dev-team/agents/unit-test-executor.md`：
  - 在 Process 中增加 Step "Framework detection": 调用 `test_detect_frameworks` MCP 工具检测框架列表（优先于现有手动 Glob），若无检测结果则回退到手动 Glob
  - 增加 Step "Command resolution": 对每个检测到的框架调用 `test_get_framework_config` 获取 test_cmd 和 coverage_cmd（替代现有硬编码命令判断逻辑）
  - 增加 Step "Coverage execution": 测试执行后为每个框架运行 coverage_cmd，捕获退出码
  - 增加 Step "Coverage parsing": 根据 coverage_format 解析覆盖率输出文件，提取 lines/branches/functions
  - 增加 Step "Thresholds & overrides check": 从 config.json 读取阈值，计算加权平均和 coverage_pass
  - 更新 Output 章节的报告 JSON 模板，将 `"coverage": 85.3` 替换为扩展格式：
    ```json
    "coverage": { "lines": 82, "branches": 74, "functions": 81 },
    "coverage_thresholds": { "lines": 80, "branches": 70, "functions": 75 },
    "coverage_pass": true,
    "coverage_by_framework": [
      { "framework": "vitest", "coverage": { "lines": 90, "branches": 80, "functions": 85 }, "html_report": "coverage/index.html" }
    ],
    "html_reports": ["coverage/index.html"]
    ```
  - 删除 `"coverage": 0` 的退场描述（注：无覆盖率配置时设为 `coverage: null` 而非 0）
  - 在工具列表中新增 `test_detect_frameworks` 和 `test_get_framework_config` MCP 工具引用
- [x] **4.2** 更新 `plugins/dev-team/agents/unit-test-evaluator.md`：
  - 更新 Step 1 "Validate report completeness": 将 `coverage` 从 `number (0-100)` 改为 `{lines, branches, functions} | null`；增加 `coverage_pass`(boolean) 和 `coverage_by_framework`(array) 作为可选验证字段
  - 更新 Step 3 "All-pass check": 在 findings 中增加覆盖率信息（从 `coverage_pass` 和 `coverage` 读取）
  - 更新 Step 5 "Build findings": 覆盖率 findings 格式更新为包含三维度和 `coverage_by_framework` 详情
  - 增加覆盖率门控子步骤：若 `coverage_pass` 为 false 且 `coverage` 不为 null，标记 checklist 项为 fail，evidence 指出不达标维度和目录
  - 增加降级处理：若 `coverage` 为 null，标记 checklist 项为 pass 并注明 "覆盖率检查未配置或生成失败，跳过"

## 阶段 5: 集成验证

此阶段验证所有组件协同工作，确保每个验收条件满足。

- [x] **5.1** 验证 AC-1/AC-2/AC-3：创建包含 `test` 配置的 config.json 样板，通过 schema 验证并读取正确结构
- [x] **5.2** 验证 AC-4/AC-5：启动 dev-server，调用 `test_detect_frameworks` 和 `test_get_framework_config` MCP 工具，验证返回正确结果
- [x] **5.3** 验证 AC-13/AC-14/AC-15：配置 `test.frameworks` 为字符串 `"vitest"`，验证 schema 通过；配置 `"mocha"` 验证 schema 拒绝；验证 `test_detect_frameworks` 将其归一化为单个框架
- [x] **5.4** 端到端验证：在一个测试项目中运行完整的 06-unit-test 阶段，验证 executor 产出包含扩展覆盖率字段的报告，evaluator 正确读取并评估
- [x] **5.5** 验证 AC-6/AC-8/AC-9/AC-11：构造多框架、overrides 分组、覆盖率命令失败等场景，验证 coverage_pass 判定符合 ALL 逻辑
- [x] **5.6** 验证 AC-10：覆盖率不达标时 evaluator 的 findings 包含具体的维度名和百分比
- [x] **5.7** 验证 AC-12：覆盖率命令运行后，检查 HTML 报告文件是否生成且路径写入 `html_reports`

## 阶段 6: 收尾

- [x] **6.1** 更新 `plugins/dev-team/.claude-plugin/plugin.json` 中的 `version` 字段（当前 2.5.7 → 递增）
- [x] **6.2** 运行完整构建和测试流水线，确保无回归
- [x] **6.3** 在 `openspec/config.json` 中添加 `test` 配置注释或文档示例（可选，不修改 config.json 内容本身）
