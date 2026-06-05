# 测试设计: unit-test-coverage-report

> **变更**: unit-test-coverage-report
> **日期**: 2026-06-05
> **基于**: proposal.md, design.md, config-schema/spec.md, test-execution-diagnostics/spec.md, phase-agents/spec.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | Config Schema (test 节点)、MCP 工具逻辑 (test-detect-frameworks / test-get-framework-config)、覆盖率解析器 (coverage-parser)、覆盖率计算器 (coverage-calculator)、MCP 注册集成 | vite-plus (vp test) | 覆盖率 >= 90% lines, 85% branches |
| 集成测试 | MCP 工具注册 + handler 到 command 的完整调用链路、覆盖率解析器的真实文件读取、覆盖率计算器与 config 读取的联动 | vite-plus (vp test) | 关键路径 100% 覆盖 |

### 测试框架与配置

- 使用 `vite-plus/test`（`import { describe, it, expect } from 'vite-plus/test'`）
- 测试文件命名规范：与被测试源文件同目录，命名为 `<filename>.test.ts`
- 运行命令：`pnpm run -C ./plugins/dev-team/bin test`
- 与 config.json 文件系统交互的测试使用 `fs.mkdtempSync` 创建临时项目目录（遵循现有 `archi-commands.test.ts` 模式）

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `src/schemas/config.schema.test.ts` | 单元 | configSchema.test 节点 frameworks array 结构 |
| AC-2 | `src/schemas/config.schema.test.ts` | 单元 | test.coverage.thresholds 默认值 lines=80/branches=70/functions=75 |
| AC-3 | `src/schemas/config.schema.test.ts` | 单元 | coverage.overrides glob 定制阈值及缺失项继承 |
| AC-4 | `src/commands/test-detect-frameworks.test.ts` | 单元 | 按 glob 首匹配规则检测文件归属框架 |
| AC-5 | `src/commands/test-get-framework-config.test.ts` | 单元 | 返回框架测试命令、覆盖率命令、格式、输出路径 |
| AC-6 | `src/commands/test-get-framework-config.test.ts`, `src/lib/coverage-parser.test.ts`, `src/lib/coverage-calculator.test.ts` | 单元 | 覆盖率命令执行后解析并写入扩展报告（连续多文件覆盖） |
| AC-7 | `src/lib/coverage-calculator.test.ts` | 单元 | coverage_by_framework 数组构建 |
| AC-8 | `src/lib/coverage-calculator.test.ts` | 单元 | ALL 逻辑：lines/branches/functions 三者达标 |
| AC-9 | `src/lib/coverage-calculator.test.ts` | 单元 | overrides 分组单独校验 |
| AC-10 | — | — | 由 agent.md 内容审核覆盖（非可测试代码） |
| AC-11 | `src/commands/test-get-framework-config.test.ts`, `src/lib/coverage-parser.test.ts` | 单元 | 未知框架降级、覆盖率命令失败降级 |
| AC-12 | `src/lib/coverage-parser.test.ts` | 集成 | HTML 报告路径收集 |
| AC-13 | `src/schemas/config.schema.test.ts` | 单元 | frameworks 字符串简写 schema 验证 |
| AC-14 | `src/schemas/config.schema.test.ts` | 单元 | 无效框架名 Zod 拒绝 |
| AC-15 | `src/commands/test-detect-frameworks.test.ts` | 单元 | 字符串简写归一化为单框架检测 |
| AC-16 | — | — | 由 agent.md 评审覆盖（非可测试代码） |
| AC-17 | — | — | 由 agent.md 评审覆盖（非可测试代码） |
| AC-18 | — | — | 由 agent.md 评审覆盖（非可测试代码） |

---

## 3. 正向 AC（Forward ACs）

| AC ID | 需求描述 | 测试层级 |
|-------|---------|----------|
| AC-1 | `config.schema.ts` 中 `test.frameworks` 定义为 `{glob, framework}[]` 数组结构，通过 `parseConfig` 验证后返回正确数据 | 单元 |
| AC-2 | `test.coverage.thresholds` 三个维度有默认值 lines=80 / branches=70 / functions=75，不配置时使用默认值，部分配置时缺失项使用各自默认值 | 单元 |
| AC-3 | `test.coverage.overrides` 支持按 glob 定制阈值，缺失的维度继承全局阈值 | 单元 |
| AC-4 | `test_detect_frameworks` 接收文件路径列表，按 `test.frameworks` 的 glob 顺序首匹配，返回每文件框架归属和唯一框架列表 | 单元 |
| AC-5 | `test_get_framework_config` 接收框架名，返回该框架的 `test_cmd`、`coverage_cmd`、`coverage_format`、`coverage_output` | 单元 |
| AC-6 | 覆盖率命令执行后，`coverage` 为 `{lines, branches, functions}` 对象，`coverage_pass` 为 boolean | 单元 |
| AC-7 | 多框架项目 `coverage_by_framework` 数组包含每个框架的框架名、三维度覆盖率、HTML 报告路径 | 单元 |
| AC-8 | `coverage_pass` ALL 判定：lines / branches / functions 三个维度全部达标才为 true | 单元 |
| AC-9 | overrides 分组单独校验，各组全部通过 `coverage_pass` 才为 true | 单元 |
| AC-11 | 框架无对应覆盖率工具时，`coverage` 三维度均为 0 且 `html_reports` 不含该框架 | 单元 |
| AC-12 | HTML 覆盖率报告路径写入 `html_reports` 数组 | 集成 |
| AC-13 | `test.frameworks` 接受单一字符串 `"vitest"`，通过 schema 验证 | 单元 |
| AC-15 | `test.frameworks` 为字符串时，`test_detect_frameworks` 将其作为唯一框架进行匹配 | 单元 |

### 正向场景 1: 完整配置的 schema 验证通过

- **AC**: AC-1, AC-2, AC-3, AC-13
- **级别**: 单元测试
- **测试文件**: `src/schemas/config.schema.test.ts`
- **场景描述**: 构造包含 `test.frameworks`（数组+字符串两种格式）、`test.coverage.thresholds`（含部分自定义）、`test.coverage.overrides` 的完整 config 对象，验证 `parseConfig` 返回正确解析的结构，缺失阈值项使用默认值

### 正向场景 2: test_detect_frameworks 按 glob 首匹配

- **AC**: AC-4, AC-15
- **级别**: 单元测试
- **测试文件**: `src/commands/test-detect-frameworks.test.ts`
- **场景描述**: 配置 `test.frameworks` 数组 `[{"glob": "**/*.test.ts", "framework": "vitest"}, {"glob": "**/tests/**/*.rs", "framework": "rust"}]`，传入文件 `["src/utils/helper.test.ts", "tests/test_auth.rs"]`，验证返回 `detected` 数组包含正确的 framework 映射，`frameworks` 为 `["vitest", "rust"]`

### 正向场景 3: test_detect_frameworks 处理字符串简写

- **AC**: AC-15
- **级别**: 单元测试
- **测试文件**: `src/commands/test-detect-frameworks.test.ts`
- **场景描述**: 配置 `test.frameworks` 为字符串 `"vitest"`，传入文件 `["src/utils/helper.test.ts"]`，验证工具内部将其归一化为 `[{"glob": "**/*.{test,spec}.{js,ts,jsx,tsx}", "framework": "vitest"}]` 并正确匹配

### 正向场景 4: test_get_framework_config 返回已知框架配置

- **AC**: AC-5
- **级别**: 单元测试
- **测试文件**: `src/commands/test-get-framework-config.test.ts`
- **场景描述**: 传入 `{"framework": "vitest"}`，验证返回 `test_cmd: "npx vitest run --reporter=verbose"`、`coverage_cmd: "npx vitest run --coverage"`、`coverage_format: "istanbul"`、`coverage_output: "coverage/coverage-summary.json"`；对 jest、vite-plus、bun、rust 分别验证

### 正向场景 5: 覆盖率解析器解析 istanbul 格式

- **AC**: AC-6
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-parser.test.ts`
- **场景描述**: 构造 istanbul 格式的 `coverage-summary.json` 文件（含 `total.lines.pct: 85`、`total.branches.pct: 75`、`total.functions.pct: 80`），调用 `parseCoverageOutput`，验证返回 `{lines: 85, branches: 75, functions: 80}`

### 正向场景 6: 覆盖率解析器解析 llvm-cov 格式

- **AC**: AC-6
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-parser.test.ts`
- **场景描述**: 构造 llvm-cov 格式的 JSON 文件（含 `data[0].totals.lines.percent: 90`、`data[0].totals.branches.percent: 80`、`data[0].totals.functions.percent: 85`），调用 `parseCoverageOutput`，验证返回 `{lines: 90, branches: 80, functions: 85}`

### 正向场景 7: 加权平均覆盖率计算

- **AC**: AC-7
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-calculator.test.ts`
- **场景描述**: vitest 框架 lines=90/branches=80/functions=85（3 个源文件），rust 框架 lines=70/branches=65/functions=75（2 个源文件），调用 `computeWeightedAverage`，验证加权平均结果为 lines=82、branches=74、functions=81

### 正向场景 8: ALL 逻辑全部达标

- **AC**: AC-8
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-calculator.test.ts`
- **场景描述**: coverage=`{lines: 82, branches: 74, functions: 81}`, thresholds=`{lines: 80, branches: 70, functions: 75}`, overrides=`[]`，调用 `checkCoveragePass`，验证返回 true

### 正向场景 9: overrides 单维度定制通过

- **AC**: AC-3, AC-9
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-calculator.test.ts`
- **场景描述**: 全局阈值 lines=80/branches=70/functions=75，override `{"glob": "demo/**", "thresholds": {"lines": 60}}`，demo 目录覆盖率 lines=65/branches=75/functions=80，验证 override 分组 pass=true，继承了 branches 和 functions 的全局阈值

### 正向场景 10: 字符串简写 frameworks schema 通过

- **AC**: AC-13
- **级别**: 单元测试
- **测试文件**: `src/schemas/config.schema.test.ts`
- **场景描述**: 配置 `{"test": {"frameworks": "vitest"}}`，验证 `parseConfig` 通过，返回的 `test.frameworks` 为字符串 `"vitest"`

---

## 4. 反向 AC（Reverse ACs）

| AC ID | 场景描述 | 测试层级 |
|-------|---------|----------|
| AC-8 | lines/branches/functions 中某一维度不达标时 `coverage_pass=false` | 单元 |
| AC-9 | overrides 分组不通过导致整体 `coverage_pass=false` | 单元 |
| AC-11 | 覆盖率命令失败时 coverage 三维度均为 0 且 html_reports 不含该框架 | 单元 |
| AC-14 | `test.frameworks` 字符串值为无效框架名时 Zod schema 拒绝 | 单元 |
| AC-4 | `test_detect_frameworks` 处理不匹配任何 glob 的文件返回 `"unknown"` | 单元 |
| AC-4 | `test_detect_frameworks` 处理空文件列表返回空 detected | 单元 |
| AC-5 | `test_get_framework_config` 收到未知框架名返回错误 | 单元 |
| AC-2 | thresholds 配置非数值（字符串）时 schema 拒绝 | 单元 |
| AC-6 | `coverage-parser` 文件不存在或 JSON 解析失败返回 null | 单元 |
| AC-1 | frameworks 数组元素缺少 `framework` 字段时 schema 拒绝 | 单元 |
| AC-7 | 所有覆盖率命令均失败时 `coverage` 为 null | 单元 |
| AC-9 | overrides glob 无匹配文件时不影响全局通过 | 单元 |
| AC-15 | `test.frameworks` 为字符串但 config 中未配置 `test` 节点时无框架检测 | 单元 |

### 反向场景 1: 某一维度覆盖率低于阈值

- **AC**: AC-8
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-calculator.test.ts`
- **场景描述**: coverage=`{lines: 85, branches: 60, functions: 80}`, thresholds=`{lines: 80, branches: 70, functions: 75}`，调用 `checkCoveragePass`，验证返回 false（branches 60 < 70）

### 反向场景 2: overrides 分组不达标

- **AC**: AC-9
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-calculator.test.ts`
- **场景描述**: 全局 lines/branches/functions 均达标，但 `core/**` override 设 lines=90，该目录实际 lines=85，调用 `checkCoveragePass`，验证返回 false，且 `coverage_overrides` 包含 `{glob: "core/**", ... pass: false}`

### 反向场景 3: 覆盖命令失败降级

- **AC**: AC-11
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-parser.test.ts`
- **场景描述**: 覆盖率输出文件不存在时，调用 `parseCoverageOutput`，验证返回 null；覆盖率输出文件 JSON 格式错误时，验证返回 null

### 反向场景 4: 无效框架名 schema 拒绝

- **AC**: AC-14
- **级别**: 单元测试
- **测试文件**: `src/schemas/config.schema.test.ts`
- **场景描述**: 配置 `{"test": {"frameworks": "mocha"}}`，验证 `parseConfig` 抛出 ZodError；使用 `safeParseConfig` 时验证 `success=false`

### 反向场景 5: 文件不匹配任何框架

- **AC**: AC-4
- **级别**: 单元测试
- **测试文件**: `src/commands/test-detect-frameworks.test.ts`
- **场景描述**: 配置 `test.frameworks` 为 `[{"glob": "**/*.test.ts", "framework": "vitest"}]`，传入不匹配的文件 `["unknown.js"]`，验证 `framework` 为 `"unknown"`

### 反向场景 6: 未知框架名称查询

- **AC**: AC-5
- **级别**: 单元测试
- **测试文件**: `src/commands/test-get-framework-config.test.ts`
- **场景描述**: 传入 `{"framework": "unknown-framework"}`，验证返回错误信息指示未知框架名称

### 反向场景 7: thresholds 类型错误

- **AC**: AC-2
- **级别**: 单元测试
- **测试文件**: `src/schemas/config.schema.test.ts`
- **场景描述**: 配置 `{"test": {"coverage": {"thresholds": {"lines": "high"}}}}`，验证 Zod schema 拒绝，指出 `lines` 必须为 number

### 反向场景 8: frameworks 数组元素缺少必填字段

- **AC**: AC-1
- **级别**: 单元测试
- **测试文件**: `src/schemas/config.schema.test.ts`
- **场景描述**: 配置 `{"test": {"frameworks": [{"glob": "**/*.test.ts"}]}}`（缺少 `framework`），验证 Zod schema 拒绝

### 反向场景 9: frameworks 非字符串非数组类型

- **AC**: AC-1, AC-13
- **级别**: 单元测试
- **测试文件**: `src/schemas/config.schema.test.ts`
- **场景描述**: 配置 `{"test": {"frameworks": 123}}`，验证 Zod schema 拒绝

### 反向场景 10: 所有覆盖率命令失败 coverage 为 null

- **AC**: AC-7, AC-11
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-calculator.test.ts`
- **场景描述**: 所有框架的覆盖率均为 null（命令全部失败），调用加权平均计算，验证 `coverage` 为 null，`coverage_pass` 为 false

### 反向场景 11: overrides glob 无匹配

- **AC**: AC-9
- **级别**: 单元测试
- **测试文件**: `src/lib/coverage-calculator.test.ts`
- **场景描述**: override 配置 `{"glob": "nonexistent/**", "thresholds": {"lines": 90}}`，但无文件匹配该 glob，验证 `coverage_overrides` 该项 pass=true（无文件则无条件通过），全局通过时 `coverage_pass=true`

---

## 5. 测试策略

### 5.1 方法

采用从内到外（inside-out）的测试策略：
1. 先对独立无副作用的纯函数和纯计算逻辑编写单元测试（schema 验证、覆盖率解析、加权平均计算）
2. 再对有文件系统依赖的模块编写带 mock 或 temp 目录的单元测试（框架检测、框架配置查询）
3. 最后编写验证 MCP 注册和 handler 链路的集成测试

所有测试使用 mock，不依赖真实覆盖率命令的执行环境。测试覆盖率输出文件通过手动构造 JSON 内容写入临时目录来模拟。

### 5.2 测试分类

- **单元测试**:
  - Config Schema (`config.schema.ts`) — `test` 节点的 Zod 验证规则，包括 framworks 联合类型、thresholds 默认值、overrides 结构、字符串枚举约束
  - 覆盖率解析器 (`coverage-parser.ts`) — istanbul 和 llvm-cov 两种格式的 JSON 解析，文件不存在/格式错误等异常处理
  - 覆盖率计算器 (`coverage-calculator.ts`) — 加权平均计算、ALL 逻辑判定、overrides 分组校验
  - 框架检测命令 (`test-detect-frameworks.ts`) — glob 首匹配、字符串归一化、自动扫描、无匹配文件处理
  - 框架配置查询命令 (`test-get-framework-config.ts`) — 已知框架返回正确配置、未知框架返回错误
  - MCP Input/Output Schema — Zod schema 验证（两个工具的 input/output schema）

- **集成测试**:
  - MCP 注册 + handler 集成 — 验证 `mcp.ts` 中工具注册时 inputSchema/outputSchema 连接到正确的 command handler
  - 覆盖率解析器与文件系统 — 在临时目录中创建覆盖率输出文件并验证完整解析链路
  - 覆盖率计算器与 config 读取联动 — 使用临时 config.json 验证 `buildCoverageOverrides` 完整流程

#### 测试分类说明

- **单元测试**: 不依赖外部服务/数据库的测试，测试独立函数、方法或组件的单一行为。使用 mock 模拟所有外部依赖。对于 `test-detect-frameworks.ts` 的文件系统读取使用 mock 或 temp 目录。
- **集成测试**: 依赖 mock 外部服务的测试，验证多个模块/组件的协同工作。可以启动轻量级测试容器或使用内存数据库。

> **注意**: 不再使用端到端测试 (E2E)。所有测试均使用 mock，不依赖真实外部环境。

### 5.3 模拟策略

| 依赖 | 模拟策略 | 适用测试 |
|------|----------|----------|
| `openspec/config.json` 文件 | 使用 `fs.mkdtempSync` 创建临时项目目录，在临时目录中构造 `openspec/config.json`；或使用 `vi.mock` 模拟 `readConfig` 函数 | test-detect-frameworks.test.ts, coverage-calculator.test.ts |
| `node:fs` 文件读取 | 对于 `coverage-parser`：在临时目录中创建覆盖率输出 JSON 文件；对 `test-detect-frameworks`：mock `readConfig` 返回值 | coverage-parser.test.ts, test-detect-frameworks.test.ts |
| `node:glob` 文件扫描 | 在临时目录中创建匹配 glob 的测试文件，使用 `process.cwd` 导向临时目录 | test-detect-frameworks.test.ts |
| `zod/v4` | 直接使用，不 mock | 所有 schema 测试 |
| Framework 命令注册表 | 测试内部直接验证硬编码数据，不 mock | test-get-framework-config.test.ts |

---

## 6. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 空 `test` 节点配置 | `{"schema": "spec-driven"}`（无 `test` 字段） | `parseConfig` 通过，无 `test` 字段，不报错 | `src/schemas/config.schema.test.ts` |
| thresholds 部分配置 | `{"test": {"coverage": {"thresholds": {"lines": 90}}}}` | 解析后 lines=90, branches=70, functions=75 | `src/schemas/config.schema.test.ts` |
| overrides 条目缺失所有阈值字段 | `{"glob": "src/**", "thresholds": {}}` | 验证通过，该 override 继承全局阈值全量 | `src/schemas/config.schema.test.ts` |
| frameworks 数组中的 `framework` 字段为空字符串 | `[{"glob": "**/*.ts", "framework": ""}]` | Zod 不拒绝（string 类型通过），但运行时 `test_get_framework_config` 返回错误 | `src/schemas/config.schema.test.ts` |
| frameworks 数组中的 glob 为空字符串 | `[{"glob": "", "framework": "vitest"}]` | Zod 验证通过（`z.string()` 接受空串），但运行时 glob 扫描不匹配任何文件 | `src/schemas/config.schema.test.ts` |
| `test_detect_frameworks` 传入空文件列表 | `{"files": []}` | 返回 `{detected: [], frameworks: []}` | `src/commands/test-detect-frameworks.test.ts` |
| `test_detect_frameworks` 不传 files（自动扫描但无匹配文件） | `{}` — config 中 glob 不匹配任何文件 | 返回 `{detected: [], frameworks: []}` | `src/commands/test-detect-frameworks.test.ts` |
| 多框架中单个框架覆盖率命令失败 | vitest 成功 (lines=90)，rust 失败 (null) | `coverage_by_framework` 仅含 vitest 记录；rust 不计入加权平均；`coverage` 取 vitest 值 | `src/lib/coverage-calculator.test.ts` |
| `coverage_by_framework` 中所有框架 `coverage` 均为 null | 所有覆盖率命令失败 | `computeWeightedAverage` 返回 null | `src/lib/coverage-calculator.test.ts` |
| `coverage_by_framework` 中单个框架 `coverage` 为 null（部分失败） | vitest `{lines:90,branches:80,functions:85}`, rust `null` | 加权平均只计算 vitest，源文件数为仅 vitest 源文件数 | `src/lib/coverage-calculator.test.ts` |
| istanbul JSON 中 `total` 下缺少 `branches` 字段 | `{"total": {"lines": {"pct": 85}, "functions": {"pct": 80}}}` | `parseCoverageOutput` 返回 null 或缺失维度设为 0（应由设计确认） | `src/lib/coverage-parser.test.ts` |
| llvm-cov JSON 中 `data` 数组为空 | `{"data": [], "totals": {...}}` | `parseCoverageOutput` 返回 null | `src/lib/coverage-parser.test.ts` |
| istanbul JSON 中 `pct` 值为字符串 | `{"total": {"lines": {"pct": "85.3"}}}` | JSON.parse 后变为 number（如果原始文件是 JSON），或格式非标准时返回 null | `src/lib/coverage-parser.test.ts` |
| `test.frameworks` 配置为 `"vite-plus"` | 字符串简写，非数组 | `test_detect_frameworks` 归一化为 `[{glob: "**/*.{test,spec}.{js,ts,jsx,tsx}", framework: "vite-plus"}]` | `src/commands/test-detect-frameworks.test.ts` |

---

## 7. 测试数据

### 7.1 Config Schema 测试数据

```typescript
// 完整配置
const fullConfig = {
  schema: 'spec-driven' as const,
  test: {
    frameworks: [
      { glob: '**/*.test.ts', framework: 'vitest' },
      { glob: '**/tests/**/*.rs', framework: 'rust' },
    ],
    coverage: {
      thresholds: { lines: 90, branches: 80, functions: 85 },
      overrides: [
        { glob: 'demo/**', thresholds: { lines: 60 } },
      ],
    },
  },
};

// 字符串简写配置
const stringFrameworksConfig = {
  schema: 'spec-driven' as const,
  test: {
    frameworks: 'vitest' as const,
    coverage: {
      thresholds: { lines: 80, branches: 70, functions: 75 },
    },
  },
};

// 仅 frameworks 无 coverage
const minimalConfig = {
  schema: 'spec-driven' as const,
  test: {
    frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
  },
};
```

### 7.2 Coverage Parser 测试数据

```typescript
// istanbul 格式 (coverage/coverage-summary.json)
const istanbulCoverage = {
  total: {
    lines: { total: 100, covered: 85, pct: 85 },
    branches: { total: 50, covered: 37, pct: 74 },
    functions: { total: 30, covered: 24, pct: 80 },
  },
};

// llvm-cov 格式
const llvmCovOutput = {
  data: [
    {
      totals: {
        lines: { percent: 90, count: 100 },
        branches: { percent: 80, count: 50 },
        functions: { percent: 85, count: 30 },
      },
    },
  ],
};
```

### 7.3 Coverage Calculator 测试数据

```typescript
// 多框架覆盖率数据
const multiFramework = [
  { framework: 'vitest', coverage: { lines: 90, branches: 80, functions: 85 }, sourceFileCount: 3 },
  { framework: 'rust', coverage: { lines: 70, branches: 65, functions: 75 }, sourceFileCount: 2 },
];

// thresholds 配置
const thresholds = { lines: 80, branches: 70, functions: 75 };

// overrides 配置
const overrides = [
  { glob: 'core/**', thresholds: { lines: 90 }, coverage: { lines: 85, branches: 75, functions: 80 } },
];
```

### 7.4 Framework Command Registry 测试数据

```typescript
const vitestConfig = {
  framework: 'vitest',
  test_cmd: 'npx vitest run --reporter=verbose',
  coverage_cmd: 'npx vitest run --coverage',
  coverage_format: 'istanbul',
  coverage_output: 'coverage/coverage-summary.json',
};

const rustConfig = {
  framework: 'rust',
  test_cmd: 'cargo test',
  coverage_cmd: 'cargo llvm-cov --all --coverage',
  coverage_format: 'llvm-cov',
  coverage_output: 'coverage/coverage-summary.json',
};
```

---

## 8. 不可测试项

- **agent.md 文件内容** — `unit-test-executor.md`、`unit-test-evaluator.md`、`test-gen-generator.md` 是 LLM 指令文件，非可执行代码。其正确性由 code review 和 agent 测试阶段验证（AC-10, AC-16, AC-17, AC-18）。
- **MCP 工具注册的 schema 连接** — `mcp.ts` 中工具注册的 inputSchema/outputSchema 连接到对应 schema 变量，这部分是 TypeScript 编译时检查的类型安全连接，运行时无需测试。
- **实际覆盖率命令的执行环境依赖** — 真实覆盖率命令（`npx vitest run --coverage` 等）需要完整的 Node.js 项目环境和依赖安装，不在单元测试范围内。覆盖率输出数据通过手动构造 JSON 文件来模拟。
- **HTML 报告文件的真实生成** — HTML 覆盖率报告由第三方工具（c8/istanbul/llvm-cov）生成，本变更不实现 HTML 报告生成逻辑，仅收集路径。验证 AC-12 需要在集成测试环境中运行真实覆盖率命令，不在单元测试覆盖范围。
