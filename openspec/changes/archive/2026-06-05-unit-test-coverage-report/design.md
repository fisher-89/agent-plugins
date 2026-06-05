# 设计: unit-test-coverage-report

> **变更**: unit-test-coverage-report
> **日期**: 2026-06-05
> **基于**: proposal.md, config-schema/spec.md, test-execution-diagnostics/spec.md, phase-agents/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Config Schema (`test` 节点) | 声明 `test.frameworks` (string | `{glob, framework}[]`)、`test.coverage.thresholds` (lines/branches/functions 默认值) 和 `test.coverage.overrides` (按 glob 定制阈值) 的 Zod 验证规则 | `plugins/dev-team/bin/src/schemas/config.schema.ts` | zod/v4 | TypeScript + Zod |
| MCP 工具: `test_detect_frameworks` | 接收文件路径列表（或自动扫描匹配 `test.frameworks` 中 glob 的文件），按首匹配规则返回每文件所属框架及唯一框架名称列表 | `plugins/dev-team/bin/src/mcp.ts`（注册）+ `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts`（实现） | config.json `test.frameworks` 配置 | TypeScript + MCP SDK |
| MCP 工具: `test_get_framework_config` | 接收框架名称，返回该框架的测试命令、覆盖率命令、覆盖率输出格式和输出文件路径 | `plugins/dev-team/bin/src/mcp.ts`（注册）+ `plugins/dev-team/bin/src/commands/test-get-framework-config.ts`（实现） | 硬编码框架命令注册表 | TypeScript + MCP SDK |
| Framework Command Registry | 硬编码的框架配置映射表，存储五个已知框架（jest/vitest/vite-plus/bun/rust）的 test_cmd、coverage_cmd、coverage_format、coverage_output | `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` | 无 | TypeScript 常量 |
| Coverage Parser | 根据 `coverage_format` 标识（istanbul / llvm-cov）解析覆盖率输出文件，提取 lines/branches/functions 三维度百分比 | `plugins/dev-team/bin/src/lib/coverage-parser.ts` | 文件系统读取 | TypeScript |
| Coverage Calculator | 计算多框架加权平均覆盖率（按源文件数加权）；从 config.json 读取 thresholds + overrides；执行 ALL 逻辑判定 coverage_pass | `plugins/dev-team/bin/src/lib/coverage-calculator.ts` | config.json（thresholds/overrides）、各框架覆盖率数据 | TypeScript |
| unit-test-executor agent | 在 06-unit-test 阶段执行测试和覆盖率命令，调用 MCP 工具检测框架、解析覆盖率输出、计算 coverage_pass、写入扩展报告 | `plugins/dev-team/agents/unit-test-executor.md` | test_detect_frameworks、test_get_framework_config、coverage-parser、coverage-calculator | Markdown agent (sonnet) |
| unit-test-evaluator agent | 在 06-unit-test 阶段读取扩展报告中的 coverage_pass/coverage/coverage_by_framework 字段，进行门控判断并产出 findings | `plugins/dev-team/agents/unit-test-evaluator.md` | unit-test-executor 产出的报告 | Markdown agent (opus) |
| test-gen-generator agent | 在 04-test-gen 阶段调用 `test_detect_frameworks` 和 `test_get_framework_config` MCP 工具获取项目测试框架，据此生成正确框架语法（jest/vitest/bun/rust）的测试骨架文件 | `plugins/dev-team/agents/test-gen-generator.md` | test_detect_frameworks、test_get_framework_config | Markdown agent (sonnet) |
| MCP Tool Input/Output Schemas | 定义 `test_detect_frameworks` 和 `test_get_framework_config` 的输入/输出 Zod schema | `plugins/dev-team/bin/src/schemas/`（新建文件） | zod/v4 | TypeScript + Zod |

### 组件图

```
+-------------------+        +----------------------------+
|   config.json     |        |  Framework Command Registry|
|  - test.frameworks|------->|  (hardcoded map)           |
|  - test.coverage  |        |  jest / vitest / bun / ... |
+--------+----------+        +-------------+--------------+
         |                                 |
         v                                 v
+--------+----------+        +-------------+--------------+
| Config Schema     |        | MCP: test_get_framework_   |
| (Zod validation)  |        |      config                |
| config.schema.ts  |        +-------------+--------------+
+-------------------+                      |
           ^                               v
           |          +--------------------+------------------+
           |          |  MCP: test_detect_frameworks         |
           |          |  (glob matching, file→framework)     |
           |          +----------------+---------------------+
           |                           |
           |    +----------------------v----------------------+
           |    |  unit-test-executor (agent sonnet)          |
           +--->|  1. Call test_detect_frameworks             |
                |  2. Call test_get_framework_config per fw   |
                |  3. Run test command                        |
                |  4. Run coverage command per framework      |
                |  5. Parse coverage output (istanbul/llvm)   |
                |  6. Calculate weighted avg + coverage_pass  |
                |  7. Write extended report                   |
                +------------------+--------------------------+
                                   |
                                   v
                +------------------+--------------------------+
                |  unit-test-evaluator (agent opus)            |
                |  1. Read report (coverage_pass, coverage,   |
                |     coverage_by_framework)                   |
                |  2. Coverage gate: pass/fail + findings     |
                |  3. phase_log to eval.json                  |
                +---------------------------------------------+

+-------------------+        +----------------------------+
|   config.json     |        |  Framework Command Registry|
|  - test.frameworks|------->|  (hardcoded map)           |
|  - test.coverage  |        |  jest / vitest / bun / ... |
+--------+----------+        +-------------+--------------+
         |                                 |
         v                                 v
+--------+----------+        +-------------+--------------+
| MCP: test_detect_ |        | MCP: test_get_framework_   |
| frameworks        |------->|      config                |
+-------------------+        +----------------------------+
         |
         v
+--------+-----------------------------------------------+
|  test-gen-generator (agent sonnet)                     |
|  1. Call test_detect_frameworks to find test framework |
|  2. Call test_get_framework_config for test_cmd        |
|  3. Generate tests with correct framework syntax        |
+--------------------------------------------------------+
```

---

## 数据流

### 流程描述

1. **配置加载**: Executor 读取 `openspec/config.json` 的 `test` 节点，获取 frameworks 映射、coverage thresholds 和 overrides。
2. **框架检测**: Executor 调用 `test_detect_frameworks` MCP 工具（参数可选：指定文件列表或自动扫描）。工具从 config.json 读取 `test.frameworks`，将字符串格式归一化为数组，然后按首匹配规则为每个文件确定框架归属。返回 `{detected: [...], frameworks: [...]}`。
3. **命令解析**: Executor 遍历 framework 列表，对每个框架调用 `test_get_framework_config` MCP 工具，获取 test_cmd、coverage_cmd、coverage_format、coverage_output。
4. **测试执行**: Executor 按框架执行测试命令，捕获 stdout/stderr 和退出码，提取用例统计（total/passed/failed/skipped）。
5. **覆盖率执行**: 测试完成后，Executor 为每个框架运行覆盖率命令。捕获 stdout/stderr 和非零退出码。命令失败时不阻塞流程。
6. **覆盖率解析**: Executor 根据 `coverage_format`（istanbul/llvm-cov）读取覆盖率输出文件，解析 lines/branches/functions 三维度百分比。
7. **加权平均计算**: 多框架场景下，按各框架源文件数量加权平均计算三维度综合覆盖率。若所有框架覆盖率均失败，coverage 设为 null。
8. **门禁判定**: Executor 从 config.json 读取 thresholds 和 overrides，执行 ALL 逻辑判定：
   - 全局：lines >= thresholds.lines AND branches >= thresholds.branches AND functions >= thresholds.functions
   - 每个 overrides 条目：匹配目录单独校验，缺失维度继承全局阈值
   - 全部通过 → coverage_pass = true
9. **报告写入**: Executor 写入扩展的 `unit-test-execution.json`，包含扩展的 coverage 字段。
10. **评估**: Evaluator 读取报告，使用 `coverage_pass` 进行门控判断，生成包含具体维度信息的 findings。

### 数据模型

#### config.json test 节点

| 字段 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `test.frameworks` | `string \| {glob: string, framework: string}[]` | 无 | 测试框架映射。字符串简写时有效值为 `jest`/`vitest`/`vite-plus`/`bun`/`rust` |
| `test.coverage.thresholds.lines` | number | `80` | 行覆盖率门禁百分比 |
| `test.coverage.thresholds.branches` | number | `70` | 分支覆盖率门禁百分比 |
| `test.coverage.thresholds.functions` | number | `75` | 函数覆盖率门禁百分比 |
| `test.coverage.overrides` | `{glob: string, thresholds: {lines?, branches?, functions?}}[]` | `[]` | 按目录定制的阈值覆盖，缺失项继承全局默认值 |

#### Framework Command Registry

| 字段 | 类型 | 描述 |
|------|------|------|
| `framework` | string | 框架名称（作为键） |
| `test_cmd` | string | 测试命令模板 |
| `coverage_cmd` | string | 覆盖率命令模板 |
| `coverage_format` | `"istanbul" \| "llvm-cov"` | 覆盖率输出格式标识 |
| `coverage_output` | string | 覆盖率输出文件路径（相对项目根） |

#### test_detect_frameworks 输出

| 字段 | 类型 | 描述 |
|------|------|------|
| `detected` | `{file: string, framework: string}[]` | 每个文件及其所属框架，无匹配时 framework 为 `"unknown"` |
| `frameworks` | `string[]` | 去重后的唯一框架名称列表 |

#### test_get_framework_config 输出

| 字段 | 类型 | 描述 |
|------|------|------|
| `framework` | string | 框架名称 |
| `test_cmd` | string | 测试命令 |
| `coverage_cmd` | string | 覆盖率命令 |
| `coverage_format` | `"istanbul" \| "llvm-cov"` | 覆盖率输出格式 |
| `coverage_output` | string | 覆盖率输出文件路径 |

#### unit-test-execution.json 报告（扩展字段）

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `coverage` | `{lines: number, branches: number, functions: number} \| null` | 是 | 三维度覆盖率（加权平均），null 表示未生成 |
| `coverage_thresholds` | `{lines: number, branches: number, functions: number}` | 是 | 门禁阈值，从 config.json 读取 |
| `coverage_overrides` | `{glob: string, thresholds: object, coverage: object, pass: boolean}[]` | 否 | 各 overrides 分组的校验结果 |
| `coverage_pass` | boolean | 是 | ALL 判定结果 |
| `coverage_by_framework` | `{framework: string, coverage: {lines, branches, functions}, html_report: string\|null}[]` | 否 | 各框架覆盖率详情 |
| `html_reports` | `string[]` | 是 | HTML 覆盖率报告路径列表（可能为空） |

---

## 路由/API 设计

### MCP 工具: test_detect_frameworks

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| `registerTool` | `dev-team MCP server` | 检测文件所属的测试框架，按 config.json 中 `test.frameworks` 的 glob 首匹配 | `{files?: string[]}` — 文件路径列表（可选，不传则自动扫描） | `{detected: {file, framework}[], frameworks: string[]}` | MCP 内部 |

### MCP 工具: test_get_framework_config

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| `registerTool` | `dev-team MCP server` | 获取指定框架的测试和覆盖率命令配置 | `{framework: string}` — 框架名称 | `{framework, test_cmd, coverage_cmd, coverage_format, coverage_output}` | MCP 内部 |

### 框架命令注册表

| 框架名 | test_cmd | coverage_cmd | coverage_format | coverage_output |
|--------|----------|--------------|-----------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage` | istanbul | `coverage/coverage-summary.json` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage` | istanbul | `coverage/coverage-summary.json` |
| vite-plus | `vp test` | `vp test --coverage` | istanbul | `coverage/coverage-summary.json` |
| bun | `bun test` | `bun test --coverage` | istanbul | `coverage/coverage-summary.json` |
| rust | `cargo test` | `cargo llvm-cov --all --coverage` | llvm-cov | `coverage/coverage-summary.json` |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **框架命令配置硬编码在代码中，而非 config.json** | 框架命令是工具链版本的实现细节，由插件维护者而非项目开发者管理。用户只需声明框架名称，无需了解具体命令。避免 config.json 因工具链升级而过时。 | 备选: 将命令模板放入 config.json 的 `test.commands` 字段。否决理由：增加用户配置负担，框架升级时需要手动更新配置，且命令模板与项目逻辑无关。 |
| D2 | **覆盖率命令在测试执行后独立运行（post-run）** | 覆盖率收集通常需要编译插桩（instrumentation），与测试命令分开执行更可靠。分开执行避免修改测试命令的参数，降低测试执行出错的风险。 | 备选: 将覆盖率参数嵌入测试命令（如 `--coverage` 标志）。否决理由：某些框架的覆盖率收集需要不同的构建配置（如 c8/istanbul），与测试命令耦合导致失败时无法区分是测试失败还是覆盖率失败。 |
| D3 | **Zod schema 使用 `.passthrough()` 允许额外字段** | 与现有 schema 设计保持一致，避免因验证拒绝而被其他工具管理的配置键。`test` 节点本身被精确类型化，不影响 passthrough 行为。 | 备选: 使用 `.strict()` 拒绝所有未知字段。否决理由：会破坏与现有工具（如 config_set）的向下兼容性，这些工具可能会在 config.json 中写入插件未知的键。 |
| D4 | **多框架覆盖率按源文件数量加权平均** | 源文件数量近似反映各框架代码库的测试覆盖范围。相比简单平均，更准确地反映整体覆盖率水平。 | 备选: 简单算术平均。否决理由：如果某框架只有少量文件（如 1 个 Python 文件）而另一框架有大量文件（如 100 个 TypeScript），等权平均会严重扭曲整体覆盖率估计。备选: 按代码行数加权。否决理由：行数统计成本高，需要额外的工具支持，且行数与覆盖率的相关性不显著优于源文件数。 |
| D5 | **Executor 负责计算 `coverage_pass`，Evaluator 仅读取** | Executor 拥有完整的覆盖率数据和配置上下文（thresholds/overrides），最适合在写入报告前完成门禁判定。Evaluator 读取现成的 `coverage_pass` 避免了重复计算逻辑，保持评估逻辑的简洁性。 | 备选: Evaluator 根据报告中的原始 coverage + thresholds 重新计算 coverage_pass。否决理由：引入重复逻辑和维护负担；overrides 的目录匹配和源文件计数在 Executor 环境中更容易实现（有文件系统访问）。 |
| D6 | **`test.frameworks` 字符串简写由 Zod `z.enum()` 约束有效值** | 在 schema 层捕获无效框架名称，提供即时反馈。类型安全，IDE 自动补全友好。字符串值与数组值在同一字段中共存（联合类型），减少配置意外。 | 备选: 允许任意字符串，在运行时由 `test_get_framework_config` 返回错误。否决理由：延迟错误发现，用户在配置时无法获得有效值的提示，增加运行时失败的概率。 |

---

## 依赖

### 运行时依赖

- `@modelcontextprotocol/sdk` — MCP 工具注册和通信
- `zod/v4` — 配置 schema 验证和输入/输出 schema 定义
- `node:fs` / `node:path` — 文件系统操作（读取覆盖率输出文件、扫描测试文件）

### 构建/测试依赖

- `typescript` — 类型检查和编译
- `eslint` — 代码质量（项目已有配置）

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 覆盖率命令因缺少依赖而失败 | 覆盖率报告降级为 0，但不阻塞测试执行 | 中 | Executor 捕获覆盖率命令的非零退出码，记录到报告但不中断流程；`coverage_pass` 设为 false |
| 覆盖率输出格式解析失败 | 无法提取百分比，覆盖率报告不完整 | 低 | 使用已知输出格式（istanbul JSON、llvm-cov JSON）并附加 Schema 验证；无法解析时记录原始输出到 report.findings |
| glob 模式匹配意外覆盖 | 错误的框架被选中导致命令执行异常 | 低 | `test_detect_frameworks` 使用首匹配规则，日志记录匹配结果供人工审查 |
| 字符串简写的默认 glob 与项目实际测试文件不匹配 | 框架检测不到文件，覆盖率不生成 | 低 | 用户可通过切换到数组格式明确指定 glob 来解决；默认 glob 使用通用的测试文件模式 |
| 新增 config 键与已有 `.passthrough()` 冲突 | Zod schema 验证拒绝合法配置 | 低 | 使用 `.passthrough()` + 精确字段定义，额外的 `test` 字段被精确类型化 |

---

## 迁移步骤

1. 在 `config.schema.ts` 中扩展 Zod schema，在 `.passthrough()` 之前添加 `test` 节点定义
2. 新建 `schemas/test-detect-frameworks.schema.ts` 和 `schemas/test-get-framework-config.schema.ts` 定义 MCP 工具的 input/output schema
3. 新建 `commands/test-detect-frameworks.ts` 实现框架检测逻辑（glob 匹配 + config 读取）
4. 新建 `commands/test-get-framework-config.ts` 实现框架命令注册表和查询逻辑
5. 新建 `lib/coverage-parser.ts` 实现 istanbul 和 llvm-cov 格式解析
6. 新建 `lib/coverage-calculator.ts` 实现加权平均和 coverage_pass 判定
7. 在 `mcp.ts` 中注册两个新工具
8. 更新 `schemas/index.ts` 导出新 schema
9. 更新 `plugins/dev-team/agents/unit-test-executor.md` 增加框架检测、覆盖率执行、扩展报告步骤
10. 更新 `plugins/dev-team/agents/unit-test-evaluator.md` 增加扩展覆盖率字段的读取和门控逻辑
11. 更新 `plugins/dev-team/agents/test-gen-generator.md` 增加框架检测 MCP 工具调用，根据检测到的框架生成对应语法的测试骨架
12. 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号

---

## 待决问题

- 覆盖率输出文件路径 `coverage/coverage-summary.json` 对于 vitest/jest 是默认路径，但某些项目可能自定义了输出目录。是否需要支持自定义输出路径？
- 源文件数量的获取方式：使用 Glob 匹配框架对应的源文件 glob 模式。对于 `vite-plus` 框架，默认 glob 是否应该与 vitest/jest 共用 `**/*.{test,spec}.{js,ts,jsx,tsx}` 还是使用独立的模式？
- `coverage_overrides` 的目录匹配逻辑：是否支持嵌套的 glob 匹配（如 `src/**` 匹配 `src/core/util.ts`），还是仅支持精确目录匹配？
