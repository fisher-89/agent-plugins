## MODIFIED Requirements

### Requirement: Executor (sonnet) 执行测试并产出结构化报告

unit-test-executor 和 integration-test-executor SHALL 在其所属的独立阶段（06-unit-test 和 08-integration-test）中被调用，负责执行对应类型的测试并产出结构化报告文件。
Executor SHALL 使用 sonnet 模型。
Executor SHALL 执行测试命令（如 `npm test`、`vp test` 等），捕获测试输出（stdout/stderr、退出码）。
Executor SHALL 从测试输出中提取：总用例数、通过数、失败数、跳过数、测试覆盖率百分比。
Executor SHALL 将提取的数据写入结构化报告文件（`reports/unit-test-execution.json` 或 `reports/integration-test-execution.json`）。
报告文件 SHALL 包含以下字段：
- `phase`: 阶段标识（06-unit-test 或 08-integration-test）
- `timestamp`: ISO 8601 时间戳
- `total`: 总用例数
- `passed`: 通过数
- `failed`: 失败数
- `skipped`: 跳过数
- `coverage`: 覆盖率对象（或 null，表示未生成），包含三个维度：
  - `lines`: 行覆盖率百分比（number）
  - `branches`: 分支覆盖率百分比（number）
  - `functions`: 函数覆盖率百分比（number）
- `coverage_thresholds`: 门禁阈值对象，包含 `lines`、`branches`、`functions`，从 config.json 的 `test.coverage.thresholds` 读取
- `coverage_overrides`: overrides 校验结果数组（当 config 含 overrides 时），每项包含 `glob`、`thresholds`、`coverage`、`pass`
- `coverage_pass`: 布尔值，表示所有维度在所有目录（全局 + overrides）均达标（ALL 判定）
- `coverage_by_framework`: 各框架覆盖率详情数组（当 coverage 生成时包含），每项包含：
  - `framework`: 框架名称
  - `coverage`: `{lines, branches, functions}` 对象
  - `html_report`: 该框架的 HTML 覆盖率报告路径（或 null）
- `html_reports`: HTML 覆盖率报告文件路径数组（可能为空）
- `failures`: 失败用例详情数组（测试名、错误信息、堆栈第一行、涉及文件路径）
- `duration_seconds`: 执行时长（秒）
- `test_command`: 实际执行的测试命令

#### Scenario: 单元测试 Executor 运行并产出报告

- **WHEN** unit-test-executor（sonnet）被 06-unit-test 阶段调用
- **THEN** 它执行项目相应的单元测试命令（由框架检测工具确定的命令）
- **AND** 它从测试输出中提取用例统计和覆盖率指标
- **AND** 它将这些数据写入 `reports/unit-test-execution.json`
- **AND** 报告包含所有必需字段（phase、timestamp、total、passed、failed、skipped、coverage: {lines, branches, functions}、coverage_thresholds、coverage_pass、coverage_by_framework、failures、duration_seconds）

#### Scenario: 集成测试 Executor 运行并产出报告

- **WHEN** integration-test-executor（sonnet）被 08-integration-test 阶段调用
- **THEN** 它检查 `tests/` 目录下是否有标记为 integration 的测试文件
- **AND** 如果有，则执行对应的集成测试命令
- **AND** 它将结果写入 `reports/integration-test-execution.json`
- **AND** 报告格式与 unit-test 报告完全一致（复用同一 JSON schema，覆盖率字段保留但 coverage_pass 始终为 false 直至集成测试覆盖率支持就绪）

#### Scenario: 测试命令执行失败（非零退出码）

- **WHEN** 测试命令以非零退出码退出且无测试输出
- **THEN** executor 将 `failed` 设为总用例数，`failures` 包含退出码和 stderr 内容
- **AND** 写入报告后退出（不重试）

### Requirement: 测试执行覆盖率检查

unit-test-evaluator 和 integration-test-evaluator SHALL 从 executor 产出的报告中读取覆盖率数据。
Evaluator SHALL 优先使用报告中的 `coverage_pass` 布尔值进行门控判断。`coverage_pass` 为 true 当且仅当 lines / branches / functions 三个维度全部达到各自阈值，且所有 overrides 分组也全部通过（ALL 判定）。
如果 `coverage_pass` 为 false 且 `coverage` 不为 null，Evaluator SHALL 将该 checklist 项标记为 fail，evidence 包含各维度覆盖率百分比、对应阈值，以及 `coverage_overrides` 中的不达标详情。
如果 `coverage_pass` 为 true，Evaluator SHALL 将该 checklist 项标记为 pass，evidence 包含各维度覆盖率百分比。
如果报告中的 `coverage` 为 null（覆盖率工具未配置或覆盖率生成失败），Evaluator SHALL 将该 checklist 项标记为 pass 并在 evidence 中注明 "覆盖率检查未配置或生成失败，跳过"。
覆盖率阈值由 openspec/config.json 中的 `test.coverage.thresholds` 定义，默认 {lines: 80, branches: 70, functions: 75}。Executor 在计算 `coverage_pass` 时 SHALL 使用该阈值。

#### Scenario: 所有维度覆盖率满足阈值

- **WHEN** 报告显示 `coverage_pass` 为 true，lines=85（阈值 80）、branches=75（阈值 70）、functions=80（阈值 75）
- **THEN** checklist 中覆盖率条目标记为 pass，evidence 包含三个维度的覆盖率百分比

#### Scenario: 某一维度覆盖率低于阈值

- **WHEN** 报告显示 `coverage_pass` 为 false，lines=85（阈值 80 ✓）、branches=60（阈值 70 ✗）、functions=80（阈值 75 ✓）
- **THEN** checklist 中覆盖率条目标记为 fail，evidence 指出 "branches coverage 60% below threshold 70%"

#### Scenario: overrides 分组不通过导致 coverage_pass 为 false

- **WHEN** 全局 lines/branches/functions 均达标，但 `core/**` override 设 lines=90，该目录实际 lines=85
- **THEN** `coverage_pass` 为 false，evidence 包含 "core/** lines=85 below override threshold 90"

#### Scenario: 覆盖率工具未配置或生成失败

- **WHEN** 报告中的 `coverage` 为 null（覆盖率生成未执行或失败）
- **THEN** checklist 中覆盖率条目标记为 pass，evidence 注明 "覆盖率检查未配置或生成失败，跳过"

## ADDED Requirements

### Requirement: Executor 使用框架检测工具确定覆盖率命令

unit-test-executor SHALL 调用 `test_detect_frameworks` MCP 工具检测项目中使用的测试框架。
检测到框架后，Executor SHALL 调用 `test_get_framework_config` MCP 工具获取每个框架的覆盖率命令和输出格式。
Executor SHALL 为每个检测到的框架运行覆盖率命令（与测试命令分开执行），捕获 stdout/stderr 和退出码。
Executor SHALL 根据 `test_get_framework_config` 返回的 `coverage_format` 字段选择对应的解析策略，从覆盖率输出中提取 lines / branches / functions 三个维度的百分比数值：
- `istanbul` 格式：从 `coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 读取
- `llvm-cov` 格式：从 `llvm-cov` JSON 输出的 `data[0].totals.lines.percent`、`data[0].totals.branches.percent`、`data[0].totals.functions.percent` 读取

覆盖率命令执行失败时（非零退出码），Executor SHALL：
- 将该框架的 `coverage` 三个维度均设为 0
- 记录错误信息到报告的 `findings` 字段
- 将该框架从 `html_reports` 中排除
- 不阻塞整体测试报告写入

#### Scenario: Executor 检测 vitest 框架并运行覆盖率命令

- **WHEN** unit-test-executor 发现项目中存在 `*.test.ts` 文件
- **THEN** 它调用 `test_detect_frameworks` 工具检测到 `vitest` 框架
- **AND** 它调用 `test_get_framework_config("vitest")` 获取覆盖率命令 `npx vitest run --coverage`
- **AND** 它在测试执行完毕后运行覆盖率命令
- **AND** 从 `coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 提取三个维度的覆盖率百分比

#### Scenario: Executor 处理多框架项目

- **WHEN** 项目同时包含 TypeScript (`*.test.ts`) 和 Python (`test_*.rs`) 测试文件
- **THEN** Executor 检测到 `vitest` 和 `rust` 两个框架
- **AND** 依次为每个框架运行覆盖率命令
- **AND** 报告中的 `coverage` 为各框架覆盖率按源文件数量加权的加权平均（lines / branches / functions 分别计算）
- **AND** `coverage_by_framework` 数组包含两条记录，每条包含 framework、coverage: {lines, branches, functions}、html_report

#### Scenario: 覆盖率命令执行失败不阻塞测试报告

- **WHEN** vitest 覆盖率命令因缺少 c8/istanbul 依赖而以非零退出码退出
- **THEN** Executor 将该框架的 `coverage` 三个维度均设为 0
- **AND** 在报告的 `findings` 中记录 "vitest coverage command failed: [错误信息]"
- **AND** 继续完成报告写入，`coverage_pass` 根据各维度加权平均结果和 thresholds 做 ALL 判定

### Requirement: Executor 计算加权平均覆盖率和 coverage_pass

当项目使用多个测试框架时，unit-test-executor SHALL 按 lines / branches / functions 三个维度分别计算加权平均覆盖率。权重 SHALL 为各框架对应的源文件数量。

加权平均覆盖率计算公式（每个维度独立计算）：`sum(每个框架该维度覆盖率 × 该框架源文件数) / 总源文件数`

加权平均覆盖率 SHALL 写入报告中的 `coverage: {lines, branches, functions}` 字段。

Executor SHALL 从 openspec/config.json 读取 `test.coverage.thresholds` 和 `test.coverage.overrides`。

`coverage_pass` 判定（ALL 逻辑）：
1. 全局：lines >= thresholds.lines AND branches >= thresholds.branches AND functions >= thresholds.functions
2. 对于每个 overrides 条目，匹配的目录单独计算覆盖率，与该 override 的 thresholds（缺失项继承全局）比对
3. ALL 通过（全局 + 所有 overrides 分组）→ `coverage_pass` = true；任一组任一维度不达标 → false

如果所有框架的覆盖率命令均失败，`coverage_pass` SHALL 为 false 且 `coverage` 保留为 null。

#### Scenario: 加权平均覆盖率按维度计算

- **WHEN** vitest 框架 lines=90/branches=80/functions=85（对应 3 个源文件），pytest 框架 lines=70/branches=65/functions=75（对应 2 个源文件）
- **THEN** 加权平均 lines=`(90*3 + 70*2)/5 = 82`，branches=`(80*3 + 65*2)/5 = 74`，functions=`(85*3 + 75*2)/5 = 81`
- **AND** `coverage` 字段写入 `{lines: 82, branches: 74, functions: 81}`

#### Scenario: 全局三维度全部达标时 coverage_pass 为 true

- **WHEN** `coverage` 为 `{lines: 82, branches: 74, functions: 81}`，`thresholds` 为 `{lines: 80, branches: 70, functions: 75}`
- **THEN** lines 82≥80 ✓，branches 74≥70 ✓，functions 81≥75 ✓ → `coverage_pass` 为 `true`

#### Scenario: 某一维度不达标时 coverage_pass 为 false

- **WHEN** `coverage` 为 `{lines: 82, branches: 60, functions: 81}`，`thresholds` 为 `{lines: 80, branches: 70, functions: 75}`
- **THEN** branches 60<70 ✗ → `coverage_pass` 为 `false`

#### Scenario: overrides 分组校验 coverage_pass

- **WHEN** 全局 lines=85/branches=75/functions=80（阈值 80/70/75），但 `core/**` override 设 lines=90，该目录 lines=85
- **THEN** overrides 分组 lines 85<90 ✗ → `coverage_pass` 为 `false`
- **AND** `coverage_overrides` 包含一条 `{glob: "core/**", thresholds: {lines: 90, branches: 70, functions: 75}, coverage: {lines: 85, ...}, pass: false}`

### Requirement: MCP 工具 test_detect_frameworks 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册一个名为 `test_detect_frameworks` 的 MCP 工具。
该工具的输入 SHALL 包含：
- `files`: 字符串数组，文件路径列表（可选，不传时自动扫描项目中匹配 `test.frameworks[*].glob` 的测试文件）
该工具的输出 SHALL 包含：
- `detected`: 对象数组，每项包含 `file`（文件路径）、`framework`（框架名称或 "unknown"）
- `frameworks`: 检测到的唯一框架名称列表
检测逻辑 SHALL 为：
1. 从 openspec/config.json 读取 `test.frameworks` 字段
2. 若 `test.frameworks` 为字符串，将其归一化为单条配置 `[{"glob": "<框架默认测试文件模式>", "framework": "<字符串值>"}]`，其中默认 glob 模式由框架名决定
3. 对于每个输入文件（或自动扫描到的文件），按数组中 glob 的顺序首匹配
4. 匹配到的第一个 glob 对应的 framework 即为该文件的框架归属
5. 无匹配时返回 `"unknown"`
该工具 SHALL 遵循 `test_` 域名前缀的下划线命名约定。

框架默认测试文件模式：
- `jest`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `vitest`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `vite-plus`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `bun`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `rust`: `**/tests/**/*.rs`

#### Scenario: test_detect_frameworks 按 glob 匹配文件到框架

- **WHEN** `test_detect_frameworks` 收到参数 `{"files": ["src/utils/helper.test.ts", "tests/test_auth.rs", "unknown.js"]}`
- **AND** config.json 中 `test.frameworks` 定义为 `[{"glob": "**/*.test.ts", "framework": "vitest"}, {"glob": "**/test_*.rs", "framework": "rust"}]`
- **THEN** 返回 `{"detected": [{"file": "src/utils/helper.test.ts", "framework": "vitest"}, {"file": "tests/test_auth.rs", "framework": "rust"}, {"file": "unknown.js", "framework": "unknown"}], "frameworks": ["vitest", "rust"]}`

#### Scenario: test_detect_frameworks 自动扫描文件

- **WHEN** `test_detect_frameworks` 收到参数 `{}`（无 files 参数）
- **THEN** 它自动扫描项目中匹配 `test.frameworks[*].glob` 模式的文件
- **AND** 返回所有匹配文件的框架归属

#### Scenario: test_detect_frameworks 首匹配规则

- **WHEN** 一个文件同时匹配两个 glob 模式（如 `tests/e2e/test_app.ts` 匹配 `**/*.ts` 和 `**/e2e/**`）
- **THEN** 返回 config.json 中 `frameworks` 数组里第一个匹配的框架

#### Scenario: test_detect_frameworks 处理字符串格式的 frameworks 配置

- **WHEN** config.json 中 `test.frameworks` 配置为字符串 `"vitest"`（而非数组）
- **THEN** `test_detect_frameworks` 内部将其归一化为 `[{"glob": "**/*.{test,spec}.{js,ts,jsx,tsx}", "framework": "vitest"}]`
- **AND** 对所有匹配该 glob 模式的文件返回 framework 为 `"vitest"`

### Requirement: MCP 工具 test_get_framework_config 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册一个名为 `test_get_framework_config` 的 MCP 工具。
该工具的输入 SHALL 包含：
- `framework`: 字符串，框架名称（如 `"vitest"`、`"jest"`）
该工具的输出 SHALL 包含：
- `framework`: 框架名称
- `test_cmd`: 测试命令字符串
- `coverage_cmd`: 覆盖率命令字符串
- `coverage_format`: 覆盖率输出格式标识（`"istanbul"`、`"llvm-cov"`）
- `coverage_output`: 覆盖率输出文件路径（相对于项目根目录，或 JSON 格式的路径模板）

框架命令注册表 SHALL 为硬编码实现（不在 config.json 中配置）。支持的框架名及其命令配置：

| 框架名 | test_cmd | coverage_cmd | coverage_format | coverage_output |
|--------|----------|--------------|-----------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage` | istanbul | `coverage/coverage-summary.json` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage` | istanbul | `coverage/coverage-summary.json` |
| vite-plus | `vp test` | `vp test --coverage` | istanbul | `coverage/coverage-summary.json` |
| bun | `bun test` | `bun test --coverage` | istanbul | `coverage/coverage-summary.json` |
| rust | `cargo test` | `cargo llvm-cov --all --coverage` | llvm-cov | `coverage/coverage-summary.json` |

未知框架名称 SHALL 返回错误。

#### Scenario: test_get_framework_config 返回 vitest 配置

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "vitest"}`
- **THEN** 返回包含 `test_cmd: "npx vitest run --reporter=verbose"`、`coverage_cmd: "npx vitest run --coverage"`、`coverage_format: "istanbul"`、`coverage_output: "coverage/coverage-summary.json"` 的对象

#### Scenario: test_get_framework_config 返回 bun 配置

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "bun"}`
- **THEN** 返回包含 `test_cmd: "bun test"`、`coverage_cmd: "bun test --coverage"`、`coverage_format: "istanbul"`、`coverage_output: "coverage/coverage-summary.json"` 的对象

#### Scenario: test_get_framework_config 返回 rust 配置

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "rust"}`
- **THEN** 返回包含 `test_cmd: "cargo test"`、`coverage_cmd: "cargo llvm-cov --all --coverage"`、`coverage_format: "llvm-cov"`、`coverage_output: "coverage/coverage-summary.json"` 的对象

#### Scenario: test_get_framework_config 返回未知框架错误

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "unknown-framework"}`
- **THEN** 返回错误信息指示未知框架名称

## Module Contract

### MCP Tools (`plugins/dev-team/bin/src/mcp.ts`)

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `test_detect_frameworks` | 检测文件所属的测试框架 | `files?: string[]` — 文件路径列表（可选，不传则自动扫描） | `{detected: {file, framework}[], frameworks: string[]}` |
| `test_get_framework_config` | 获取指定框架的测试和覆盖率命令配置 | `framework: string` — 框架名称 | `{framework, test_cmd, coverage_cmd, coverage_format, coverage_output}` |

### Framework Command Registry

| Framework | test_cmd | coverage_cmd | coverage_format | coverage_output |
|-----------|----------|--------------|-----------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage` | istanbul | `coverage/coverage-summary.json` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage` | istanbul | `coverage/coverage-summary.json` |
| vite-plus | `vp test` | `vp test --coverage` | istanbul | `coverage/coverage-summary.json` |
| bun | `bun test` | `bun test --coverage` | istanbul | `coverage/coverage-summary.json` |
| rust | `cargo test` | `cargo llvm-cov --all --coverage` | llvm-cov | `coverage/coverage-summary.json` |

### Coverage Report Schema (`reports/unit-test-execution.json`)

| Field | Type | Description |
|-------|------|-------------|
| `phase` | string | 阶段标识（06-unit-test 或 08-integration-test） |
| `timestamp` | string (ISO 8601) | 报告生成时间 |
| `total` | number | 总测试用例数 |
| `passed` | number | 通过数 |
| `failed` | number | 失败数 |
| `skipped` | number | 跳过数 |
| `coverage` | {lines: number, branches: number, functions: number} \| null | 三维度覆盖率（加权平均），null 表示未生成 |
| `coverage_thresholds` | {lines: number, branches: number, functions: number} | 门禁阈值（从 config.json 读取） |
| `coverage_overrides` | {glob: string, thresholds: object, coverage: object, pass: boolean}[] | 各 overrides 分组的校验结果（可选） |
| `coverage_pass` | boolean | 全局 + 所有 overrides 分组 ALL 达标（lines AND branches AND functions） |
| `coverage_by_framework` | {framework: string, coverage: {lines, branches, functions}, html_report: string\|null}[] | 各框架覆盖率详情（可选） |
| `html_reports` | string[] | HTML 覆盖率报告路径列表 |
| `failures` | {name, file, line, error_type, error_message, stack_trace}[] | 失败用例详情 |
| `duration_seconds` | number | 执行时长 |
| `test_command` | string | 实际执行的测试命令 |
