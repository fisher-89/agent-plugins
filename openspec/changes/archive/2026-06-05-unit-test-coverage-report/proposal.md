# 提案: unit-test-coverage-report

> **变更**: unit-test-coverage-report
> **日期**: 2026-06-04
> **状态**: draft

---

## 问题

当前单元测试工作流（06-unit-test）中，executor 产出的结构化报告已定义 `coverage` 和 `coverage_threshold` 字段，evaluator 也具备覆盖率阈值检查逻辑（"测试执行覆盖率检查"），但缺乏实际生成覆盖率数据的机制。Executor 报告中的 `coverage` 始终为 `null`，evaluator 因"覆盖率工具未配置"而跳过检查，覆盖率门控形同虚设。

具体问题包括：

1. **覆盖率数据缺失**：Executor 不运行覆盖率命令，无法从测试输出中提取覆盖率百分比；report 格式中的 `coverage` 字段永远为空
2. **无框架感知能力**：Executor 硬编码了测试命令，无法根据项目实际使用的测试框架选择合适的覆盖率命令和输出格式
3. **无配置入口**：项目没有声明测试框架列表和覆盖率阈值的统一位置 —— `coverageThreshold` 定义在 CLAUDE.md 中，无法被工具化使用
4. **报告缺乏细粒度**：单一 `coverage` 数值无法反映多框架项目的各框架覆盖率详情，也没有关联 HTML 报告路径供人工查阅
5. **单框架配置冗余**：仅使用单一测试框架的项目仍需编写完整的数组对象格式 `[{"glob": "**/*.test.ts", "framework": "vitest"}]`，缺少轻量的字符串简写方式

---

## 提案

引入一套配置驱动的覆盖率生成机制，覆盖从框架检测到覆盖率报告输出的完整链路：

1. **Config 扩展**：在 `openspec/config.json` 中新增 `test` 配置节点，声明测试框架映射列表（glob → framework）、覆盖率阈值（lines / branches / functions 三维度）和按目录的 overrides（glob → 定制阈值）
2. **MCP 工具**：新增 `test_detect_frameworks`（文件路径 → 框架检测）和 `test_get_framework_config`（框架名 → 命令配置）两个 MCP 工具，供 executor 在运行时动态查询框架信息和对应的覆盖率命令
3. **Executor 增强**：单元测试执行完成后，Executor 自动为每个框架运行覆盖率命令，解析标准化输出格式（istanbul JSON、pytest coverage.json、llvm-cov JSON），提取 lines / branches / functions 三维度覆盖率，按 overrides 分组校验
4. **报告扩展**：`unit-test-execution.json` 的 `coverage` 字段从单一数字扩展为 `{lines, branches, functions}` 对象，增加 `coverage_pass`（ALL 判定）、`coverage_thresholds`、`coverage_by_framework`、`html_reports`
5. **Evaluator 适配**：Evaluator 读取扩展后的报告字段进行门控判断 —— lines / branches / functions 三者全部达标才通过
6. **Frameworks 字符串简写**：`test.frameworks` 字段扩展为同时接受单一框架名称字符串（如 `"vitest"`），当项目仅使用一个测试框架时无需编写数组。字符串值通过 `z.enum()` 约束为已知框架名

---

## 能力

### 修改的能力

- **config-schema** — config.json 的 Zod schema 增加 `test` 配置节点，支持 `test.frameworks`（string 简写或 glob → framework 映射数组）、`test.coverage.thresholds`（lines/branches/functions 三维度门禁，默认 80/70/75）和 `test.coverage.overrides`（按 glob 定制阈值）
- **test-execution-diagnostics** — 扩展覆盖率生成执行流程（覆盖率命令运行、lines/branches/functions 多格式解析、overrides 分组校验）、覆盖率报告格式（`coverage: {lines, branches, functions}`、`coverage_pass`、`coverage_by_framework`、`html_reports`）、新增 MCP 工具
- **phase-agents** — 更新 `unit-test-executor.md`、`unit-test-evaluator.md` 和 `test-gen-generator.md` agent 文件，增加框架检测 MCP 工具调用、覆盖率命令执行逻辑和框架感知测试生成能力

---

## 变更范围

### 实现以下特性

- `openspec/config.json` 增加 `test` 配置节点（`frameworks` + `coverage.thresholds` + `coverage.overrides`）
- `plugins/dev-team/bin/src/` 新增 `test_detect_frameworks` 和 `test_get_framework_config` MCP 工具注册
- `plugins/dev-team/bin/src/schemas/config.schema.ts` 更新 Zod schema 定义以包含 `test` 键，其中 `test.frameworks` 字段类型从 `{glob, framework}[]` 扩展为 `string | {glob, framework}[]`，字符串值通过 `z.enum(["jest", "vitest", "vite-plus", "bun", "rust"])` 约束
- `plugins/dev-team/agents/unit-test-executor.md` 更新：调用 MCP 工具检测框架、运行覆盖率命令、解析覆盖率输出、写入扩展报告
- `plugins/dev-team/agents/unit-test-evaluator.md` 更新：读取扩展报告中的三维度覆盖率（lines/branches/functions）和 `coverage_pass`、按 overrides 分组输出门禁结果
- `plugins/dev-team/agents/test-gen-generator.md` 更新：在 Process 开头调用 `test_detect_frameworks` 检测项目测试框架，调用 `test_get_framework_config` 获取框架配置，根据检测到的框架生成对应框架语法（Jest 的 describe/it、Vitest 的 describe/it+vi、Bun 的 describe/test、Rust 的 #[cfg(test)]）的测试骨架文件，并匹配框架对应的 skip marker 约定
- `unit-test-execution.json` 报告格式扩展：`coverage: {lines, branches, functions}` 对象、`coverage_thresholds`（含 overrides）、`coverage_pass`（ALL 判定）、`coverage_by_framework`（含三维度）、`html_reports`
- 框架命令注册表（jest / vitest / vite-plus / pytest）的覆盖率命令和输出解析逻辑

### 不要修改

- 集成测试覆盖率（08-integration-test）—— 后续可通过相同机制复用，本次不涉及
- 覆盖率报告的存档/持久化机制 —— 仅限于运行后生成的 HTML 报告文件，不做版本管理
- 覆盖率历史趋势追踪 —— 不引入历史数据对比或趋势图表
- 现有的 evaluator 诊断决策树逻辑 —— 仅适配覆盖率字段的读取，不改变失败分类和回溯判定规则
- 非测试相关的 phase agent 文件（proposal、dev-design、test-design、implement、code-review 等）

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | config.json 中可配置 `test.frameworks` 数组，每个元素包含 `glob` 和 `framework` 字段 | 使用 `config_get` MCP 工具读取 `test.frameworks` 返回正确结构 | P0 |
| AC-2 | config.json 中可配置 `test.coverage.thresholds`，含 `lines`（默认 80）、`branches`（默认 70）、`functions`（默认 75） | 不配置时使用默认值；部分配置时未配置项使用各自默认值 | P0 |
| AC-3 | `test.coverage.overrides` 支持按 glob 定制阈值，继承全局未配置项 | `{"glob": "demo/**", "thresholds": {"lines": 60}}` → demo 目录 lines=60，branches/functions 继承全局 | P1 |
| AC-4 | `test_detect_frameworks` MCP 工具接收文件路径列表，返回每文件的框架归属 | 传入 `["src/utils/helper.test.ts"]`，返回 `{"framework": "vitest"}` 或 `"unknown"` | P0 |
| AC-5 | `test_get_framework_config` MCP 工具返回框架的测试命令、覆盖率命令、输出格式和路径 | 传入 `"vitest"`，返回含 `test_cmd`、`coverage_cmd`、`coverage_format`、`coverage_output` 的对象 | P0 |
| AC-6 | Executor 运行测试后自动运行覆盖率命令，解析输出并写入报告 | 检查 `unit-test-execution.json` 中 `coverage` 为 `{lines, branches, functions}` 对象，`coverage_pass` 为 boolean | P0 |
| AC-7 | 多框架项目生成 `coverage_by_framework` 数组，每项含框架名、三维度覆盖率（lines/branches/functions）和 HTML 报告路径 | 在同时使用 vitest 和 rust 的项目中验证报告包含两个框架条目，每个框架有完整三维度数据 | P1 |
| AC-8 | `coverage_pass` 使用 ALL 判定：lines / branches / functions 三者全部达标才为 true | 构造 lines=90, branches=60, functions=80 的报告，thresholds 为 80/70/75 → branches 不达标 → coverage_pass=false | P0 |
| AC-9 | overrides 覆盖的目录单独校验，与全局阈值分别判定，全部通过才 coverage_pass=true | core/** 设 lines=90，该目录 lines=85 → 不通过；其他目录通过 → coverage_pass=false | P1 |
| AC-10 | Evaluator 门控不通过时 findings 指出具体不达标维度和目录 | branches=60（阈值 70），findings 含 "branches coverage 60% below threshold 70%" | P0 |
| AC-11 | 框架无对应覆盖率工具时，`coverage` 三维度均为 0 且 `html_reports` 不含该框架 | 配置一个未知框架，验证覆盖率报告降级 | P1 |
| AC-12 | HTML 覆盖率报告文件在运行后生成，路径写入 `html_reports` 数组 | 运行后检查文件系统上 HTML 报告文件存在，且路径与报告中的 `html_reports` 匹配 | P2 |
| AC-13 | `test.frameworks` 可配置为单一字符串 `"vitest"`，通过 schema 验证 | 使用 `config_get` 读取 `test.frameworks` 返回字符串 `"vitest"` | P0 |
| AC-14 | `test.frameworks` 为字符串时，无效框架名导致验证失败 | 配置 `"unknown-framework"`，Zod schema 返回验证错误 | P1 |
| AC-15 | `test.frameworks` 为字符串时，`test_detect_frameworks` 将其作为唯一的框架进行匹配 | 配置 `"jest"`，所有测试文件检测为 jest 框架 | P1 |
| AC-16 | test-gen-generator 调用 `test_detect_frameworks` 检测项目测试框架 | 检查 test-gen-generator agent.md 文件中包含 `test_detect_frameworks` 工具引用及 Process 步骤 | P0 |
| AC-17 | test-gen-generator 根据检测到的框架使用正确的测试语法生成骨架文件（Jest => describe/it，Vitest => describe/it+vi，Bun => describe/test，Rust => #[cfg(test)]） | 在 test-gen-generator 的 Process 步骤中验证框架特定语法的输出规则 | P1 |
| AC-18 | test-gen-generator 调用 `test_get_framework_config` 获取框架配置信息 | 检查 agent.md 文件中包含 `test_get_framework_config` 工具引用 | P1 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 覆盖率命令因缺少依赖而失败 | 覆盖率报告降级为 0，但不阻塞测试执行 | 中 | Executor 捕获覆盖率命令的非零退出码，记录到报告但不中断流程；`coverage_pass` 设为 false |
| 覆盖率输出格式解析失败 | 无法提取百分比，覆盖率报告不完整 | 低 | 使用已知输出格式（istanbul JSON、pytest coverage.json、llvm-cov JSON）并附加 Schema 验证；无法解析时记录原始输出到报告 |
| glob 模式匹配意外覆盖 | 错误的框架被选中导致命令执行异常 | 低 | `test_detect_frameworks` 使用首匹配规则，日志记录匹配结果供人工审查 |
| 新增 config 键与已有 `.passthrough()` 冲突 | Zod schema 验证拒绝合法配置 | 低 | 使用 `.passthrough()` + 精确字段定义，额外的 `test` 字段被精确类型化 |
