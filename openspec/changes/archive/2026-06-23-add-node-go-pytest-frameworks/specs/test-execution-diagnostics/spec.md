## ADDED Requirements

### Requirement: node-test 覆盖率 stdout 解析脚本

`plugins/dev-team/scripts/parse-node-test-coverage.mjs` SHALL 作为独立 Node.js 脚本存在，用于将 `node --test --experimental-test-coverage` 输出的 stdout 文本覆盖率表格转换为 istanbul 兼容的 `coverage-summary.json`。

脚本 CLI 接口 SHALL 为：
```
node parse-node-test-coverage.mjs <stdout-file> <output-json-path>
```

脚本 SHALL：
1. 读取 `<stdout-file>` 中的 node:test 覆盖率文本表格
2. 提取 `lines`、`branches`、`functions` 三个维度的总覆盖率百分比
3. 写入 `<output-json-path>`，格式与 istanbul `coverage-summary.json` 兼容（含 `total.lines.pct`、`total.branches.pct`、`total.functions.pct`）
4. 解析失败时以非零退出码退出并在 stderr 输出错误信息

#### Scenario: 解析有效 node:test stdout 并写入 JSON

- **WHEN** 脚本收到包含标准覆盖率表格的 stdout 文件路径和输出路径 `coverage/coverage-summary.json`
- **THEN** 脚本写入 JSON 文件，其中 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 均为有效数值
- **AND** 脚本以退出码 0 退出

#### Scenario: stdout 文件不存在或格式无效

- **WHEN** 脚本收到不存在的 stdout 文件路径，或文件内容不含可识别的覆盖率表格
- **THEN** 脚本以非零退出码退出
- **AND** stderr 包含可读的错误描述

### Requirement: 覆盖率 nullable 维度门控规则

当框架的 `coverage_format` 不支持某一覆盖率维度时，Executor SHALL 将该维度的 `measured` 值设为 `null`（而非 `0`）。

null 维度规则 SHALL 为：
1. **阈值比较**：`coverage.pass` 计算时，null 维度 SHALL 跳过与 `coverage.thresholds` 的比较（视为自动通过）
2. **加权平均**：计算 `coverage.measured` 时，null 维度 SHALL 不参与该维度的加权平均；仅非 null 框架贡献权重
3. **全 null 维度**：若所有框架在某维度均为 null，则 `coverage.measured` 中该维度 SHALL 为 `null`
4. **overrides 分组**：overrides 校验中 null 维度同样跳过比较

各框架 null 维度映射：
- `go-cover`：`branches: null`，`functions: null`
- `coverage-py`：`functions: null`
- `node-test`、`istanbul`、`llvm-cov`：三维度均为 number（无 null）

#### Scenario: Go 框架 branches 和 functions 为 null 且不影响 pass

- **WHEN** 项目仅使用 Go 框架，`measured` 为 `{lines: 85, branches: null, functions: null}`，阈值为 `{lines: 80, branches: 70, functions: 75}`
- **THEN** `coverage.pass` 为 `true`（lines 85≥80 ✓，branches/functions 跳过 ✓）

#### Scenario: pytest 框架 functions 为 null

- **WHEN** pytest 框架解析完成
- **THEN** `by_framework[].measured` 为 `{lines: <number>, branches: <number>, functions: null}`

#### Scenario: 多框架加权平均忽略 null 维度

- **WHEN** vitest 框架 `{lines: 90, branches: 80, functions: 85}`（3 源文件），go 框架 `{lines: 70, branches: null, functions: null}`（2 源文件）
- **THEN** 加权平均 lines=`(90*3 + 70*2)/5 = 82`
- **AND** 加权平均 branches=`80`（仅 vitest 贡献，go 的 null 不参与）
- **AND** 加权平均 functions=`85`（仅 vitest 贡献）

#### Scenario: 所有框架某维度均为 null 时 overall 也为 null

- **WHEN** 项目仅使用 Go 框架（branches 和 functions 均为 null）
- **THEN** `coverage.measured.branches` 为 `null`
- **AND** `coverage.measured.functions` 为 `null`

## MODIFIED Requirements

### Requirement: Executor (sonnet) 执行测试并产出结构化报告

unit-test-executor 和 integration-test-executor SHALL 在其所属的独立阶段（06-unit-test 和 08-integration-test）中被调用，负责执行对应类型的测试并产出结构化报告文件。
Executor SHALL 使用 sonnet 模型。
Executor SHALL 执行测试命令（如 `npm test`、`vp test` 等），捕获测试输出（stdout/stderr、退出码）。
Executor SHALL 从测试输出中提取：总用例数、通过数、失败数、跳过数、测试覆盖率百分比。
Executor SHALL 将提取的数据写入结构化报告文件（`reports/unit-test-execution.json` 或 `reports/integration-test-execution.json`）。
报告文件 SHALL 包含以下字段：
- `phase`: 阶段标识（06-unit-test 或 08-integration-test）
- `command`: 实际执行的测试命令（字符串）
- `timestamp`: ISO 8601 时间戳
- `total`: 总用例数
- `passed`: 通过数
- `failed`: 失败数
- `skipped`: 跳过数
- `duration_seconds`: 执行时长（秒）
- `test_cases`: 用例详情数组，每项包含 `name`、`file`、`duration_ms`、`status`（`"passed"` | `"failed"` | `"skipped"`）；当 `status` 为 `"failed"` 时，SHALL additionally 包含 `line`（number）、`error_type`（string）、`error_message`（string）、`stack_trace`（string），以及可选的 `design_ref`（string）
- `coverage`: 覆盖率嵌套对象，或 `null`（表示未配置/未生成覆盖率）。当非 null 时 SHALL 包含：
  - `pass`: 布尔值，全局 + 所有 overrides 分组 ALL 达标（lines AND branches AND functions，null 维度跳过比较）
  - `measured`: `{lines: number, branches: number | null, functions: number | null}` — 加权平均测量值
  - `thresholds`: `{lines: number, branches: number, functions: number}` — 门禁阈值（从 config.json 读取）
  - `by_framework`: 各框架覆盖率详情数组（可能为空），每项包含 `framework`（string）、`measured`（`{lines: number, branches: number | null, functions: number | null}`）
  - `overrides`: overrides 校验结果数组（当 config 含 overrides 时），每项包含 `glob`、`thresholds`、`measured`、`pass`
- `integration_test`: 集成测试子报告对象，或 `null`（无集成测试时）。当非 null 时结构 SHALL 包含 `total`、`passed`、`failed`、`skipped`、`duration_ms`、`test_cases`（与顶层 `test_cases` 相同 schema，失败条目含错误详情字段），SHALL NOT 包含独立的 `failures` 数组
- `findings`: 可选诊断信息字符串

报告文件 SHALL NOT 包含以下已废弃字段：`coverage_thresholds`、`coverage_pass`、`coverage_by_framework`、`coverage_overrides`、`html_reports`、`failures`、以及 `by_framework` 条目中的 `html_report`。

#### Scenario: 单元测试 Executor 运行并产出报告

- **WHEN** unit-test-executor（sonnet）被 06-unit-test 阶段调用
- **THEN** 它执行项目相应的单元测试命令（由框架检测工具确定的命令）
- **AND** 它从测试输出中提取用例统计和覆盖率指标
- **AND** 它将这些数据写入 `reports/unit-test-execution.json`
- **AND** 报告包含所有必需字段（phase、command、timestamp、total、passed、failed、skipped、duration_seconds、test_cases、coverage）

#### Scenario: Go 框架报告含 null 维度

- **WHEN** 项目使用 Go 框架且覆盖率解析成功
- **THEN** `coverage.by_framework` 中 go 条目的 `measured` 为 `{lines: <number>, branches: null, functions: null}`

#### Scenario: 集成测试 Executor 运行并产出报告

- **WHEN** integration-test-executor（sonnet）被 08-integration-test 阶段调用
- **THEN** 它检查 `tests/` 目录下是否有标记为 integration 的测试文件
- **AND** 如果有，则执行对应的集成测试命令
- **AND** 它将结果写入 `reports/integration-test-execution.json`
- **AND** 报告格式与 unit-test 报告复用同一 JSON schema（覆盖率字段保留但 `coverage` 可为 null 直至集成测试覆盖率支持就绪）

#### Scenario: 测试命令执行失败（非零退出码）

- **WHEN** 测试命令以非零退出码退出且无结构化测试输出
- **THEN** executor 将 `failed` 设为总用例数
- **AND** 在 `test_cases` 中写入一条或多条 `status: "failed"` 条目，`error_message` 包含退出码和 stderr 内容
- **AND** 写入报告后退出（不重试）

#### Scenario: 失败用例在 test_cases 中携带完整错误详情

- **WHEN** 单元测试执行产生失败用例
- **THEN** 对应 `test_cases[]` 条目的 `status` 为 `"failed"`
- **AND** 该条目包含 `line`、`error_type`、`error_message`、`stack_trace`
- **AND** 报告不存在顶层 `failures[]` 数组

#### Scenario: 无覆盖率时 coverage 为 null

- **WHEN** 未配置 `test.frameworks` 或所有覆盖率命令均失败
- **THEN** 报告中 `coverage` 字段为 `null`
- **AND** 不存在其他顶层 coverage 相关字段（`coverage_thresholds`、`coverage_pass` 等）

### Requirement: Executor 使用框架检测工具确定覆盖率命令

unit-test-executor SHALL 调用 `test_detect_frameworks` MCP 工具检测项目中使用的测试框架。
检测到框架后，Executor SHALL 直接从 `test_detect_frameworks` 返回的 `plan` 数组中读取每个框架的执行计划，包括工作目录（`directory`）、覆盖率命令（`coverage_cmd`）、覆盖率格式（`coverage_format`）、覆盖率输出路径（`coverage_output`）、覆盖率产物列表（`coverage_artifacts`）和清理列表（`coverage_cleanup`）。
Executor 不再单独调用 `test_get_framework_config` 工具，也不单独执行 `test_cmd`。
Executor SHALL 为 `plan` 中的每个条目，在其对应的 `directory` 工作目录下运行 `coverage_cmd`（覆盖率命令已包含测试运行），捕获 stdout/stderr 和退出码。

覆盖率命令执行成功后，Executor SHALL 执行产物移动步骤：根据 `coverage_artifacts` 中的文件路径（JSON 摘要文件）将产物从工作目录移动到统一位置 `openspec/changes/<change>/reports/coverage/<framework>/`。移动成功后，Executor SHALL 根据 `coverage_cleanup` 中的目录列表清理原始临时文件。移动失败时（如源路径不存在），Executor SHALL 记录错误到 findings 但不阻断流程。

移动成功后，Executor SHALL 更新 `coverage_output` 的解析路径指向统一位置（`reports/coverage/<framework>/` 下对应文件名），并从统一位置读取覆盖率数据。

Executor SHALL 根据 `plan` 中的 `coverage_format` 字段选择对应的解析策略，从覆盖率输出中提取 lines / branches / functions 三个维度的百分比数值：
- `istanbul` 格式：从 `coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 读取
- `llvm-cov` 格式：从 `llvm-cov` JSON 输出的 `data[0].totals.lines.percent`、`data[0].totals.branches.percent`、`data[0].totals.functions.percent` 读取
- `node-test` 格式：从 parser 产出的 `coverage-summary.json`（istanbul 兼容结构）的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 读取
- `go-cover` 格式：从 `func-summary.txt` 的 `total:` 行提取 lines 百分比；`branches` 设为 `null`；`functions` 设为 `null`
- `coverage-py` 格式：从 `coverage.json` 的 `totals.percent_covered` 读取 lines；`totals.percent_covered_branches` 读取 branches（若存在）；`functions` 设为 `null`

覆盖率命令执行失败时（非零退出码），Executor SHALL：
- 将该框架的 `measured` 三个维度均设为 0（写入 `coverage.by_framework` 对应条目）
- 记录错误信息到报告的 `findings` 字段
- 跳过该框架的产物移动和清理步骤
- 不阻塞整体测试报告写入

#### Scenario: Executor 检测 node-test 框架并运行两步覆盖率命令

- **WHEN** unit-test-executor 检测到 `node-test` 框架
- **THEN** 从 `plan` 读取 `coverage_cmd` 包含 `node --test --experimental-test-coverage` 和 `parse-node-test-coverage.mjs`
- **AND** 运行 script 后从 `reports/coverage/node-test/coverage-summary.json` 解析三维度覆盖率

#### Scenario: Executor 检测 go 框架并解析 func-summary.txt

- **WHEN** unit-test-executor 检测到 `go` 框架且覆盖率命令成功
- **THEN** 从 `func-summary.txt` 提取 lines 百分比
- **AND** `measured` 为 `{lines: <number>, branches: null, functions: null}`

#### Scenario: Executor 检测 pytest 框架并解析 coverage.json

- **WHEN** unit-test-executor 检测到 `pytest` 框架且覆盖率命令成功
- **THEN** 从 `coverage.json` 提取 lines 和 branches 百分比
- **AND** `measured` 为 `{lines: <number>, branches: <number>, functions: null}`

#### Scenario: Executor 检测 vitest 框架并运行覆盖率命令，产物移动到统一位置

- **WHEN** unit-test-executor 发现项目中存在 `*.test.ts` 文件
- **THEN** 它调用 `test_detect_frameworks` 工具检测到 `vitest` 框架
- **AND** 从 `plan` 中读取 `coverage_cmd`、`directory`、`coverage_artifacts`、`coverage_cleanup`
- **AND** 在项目根目录运行覆盖率命令
- **AND** 将 JSON 摘要移动到 `reports/coverage/vitest/coverage-summary.json`
- **AND** 从统一位置提取三个维度的覆盖率百分比
- **AND** 写入 `coverage.by_framework` 条目 `{framework: "vitest", measured: {...}}`

#### Scenario: 覆盖率命令执行失败不阻塞测试报告，跳过移动和清理

- **WHEN** 覆盖率命令因缺少依赖而以非零退出码退出
- **THEN** Executor 将该框架的 `measured` 三个维度均设为 0
- **AND** 在报告的 `findings` 中记录错误信息
- **AND** 跳过该框架的产物移动和清理步骤
- **AND** 继续完成报告写入

### Requirement: Executor 计算加权平均覆盖率和 coverage.pass

当项目使用多个测试框架时，unit-test-executor SHALL 按 lines / branches / functions 三个维度分别计算加权平均覆盖率。权重 SHALL 为各框架对应的源文件数量。

加权平均覆盖率计算公式（每个维度独立计算）：`sum(每个框架该维度非 null 覆盖率 × 该框架源文件数) / sum(该维度非 null 框架的源文件数)`

若某维度所有框架均为 null，则该维度的加权平均 SHALL 为 `null`。

加权平均覆盖率 SHALL 写入报告中的 `coverage.measured: {lines, branches, functions}` 字段（各维度为 `number | null`）。

Executor SHALL 从 openspec/config.json 读取 `test.coverage.thresholds` 和 `test.coverage.overrides`，写入 `coverage.thresholds` 和 `coverage.overrides`。

`coverage.pass` 判定（ALL 逻辑）：
1. 全局：对每个非 null 维度，`coverage.measured.<dim>` >= `coverage.thresholds.<dim>`；null 维度跳过比较（视为通过）
2. 对于 `coverage.overrides` 中每个条目，匹配的目录单独计算覆盖率写入 `measured`，与该条目的 `thresholds`（缺失项继承全局）比对（null 维度同样跳过），写入 `pass`
3. ALL 通过（全局 + 所有 overrides 分组）→ `coverage.pass` = true；任一组任一非 null 维度不达标 → false

如果所有框架的覆盖率命令均失败或未配置覆盖率，`coverage` SHALL 为 `null`（而非含 `pass: false` 的空对象）。

#### Scenario: 加权平均覆盖率按维度计算（含 null 跳过）

- **WHEN** vitest 框架 lines=90/branches=80/functions=85（3 源文件），go 框架 lines=70/branches=null/functions=null（2 源文件）
- **THEN** 加权平均 lines=`(90*3 + 70*2)/5 = 82`，branches=`80`（仅 vitest），functions=`85`（仅 vitest）
- **AND** `coverage.measured` 字段写入 `{lines: 82, branches: 80, functions: 85}`

#### Scenario: 全局三维度全部达标时 coverage.pass 为 true（含 null 跳过）

- **WHEN** Go-only 项目 `coverage.measured` 为 `{lines: 85, branches: null, functions: null}`，`coverage.thresholds` 为 `{lines: 80, branches: 70, functions: 75}`
- **THEN** lines 85≥80 ✓，branches/functions 跳过 ✓ → `coverage.pass` 为 `true`

#### Scenario: 某一非 null 维度不达标时 coverage.pass 为 false

- **WHEN** `coverage.measured` 为 `{lines: 82, branches: 60, functions: 81}`，`coverage.thresholds` 为 `{lines: 80, branches: 70, functions: 75}`
- **THEN** branches 60<70 ✗ → `coverage.pass` 为 `false`

### Requirement: MCP 工具 test_detect_frameworks 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册一个名为 `test_detect_frameworks` 的 MCP 工具。
该工具的输入 SHALL 包含：
- `files`: 字符串数组，文件路径列表（可选，不传时自动扫描项目中匹配 `test.frameworks[*].glob` 的测试文件）
- `project_root`: 字符串，项目根目录（可选）

该工具的输出 SHALL 包含以下字段：
- `detected`: 对象数组，每项包含 `file`（文件路径）、`framework`（框架名称或 "unknown"）
- `frameworks`: 检测到的唯一框架名称列表
- `plan`: 对象数组，每项包含：
  - `directory`: 字符串，该框架的执行工作目录（相对于项目根目录），由 glob 模式推导
  - `framework`: 字符串，框架名称
  - `coverage_cmd`: 字符串，覆盖率命令（包含测试执行）
  - `coverage_format`: 字符串，覆盖率输出格式（`"istanbul"`、`"llvm-cov"`、`"node-test"`、`"go-cover"` 或 `"coverage-py"`）
  - `coverage_output`: 字符串，覆盖率输出文件路径（相对于工作目录）
  - `coverage_artifacts`: 字符串数组，需要移动到统一位置的产物路径列表（相对于工作目录）
  - `coverage_cleanup`: 字符串数组，移动成功后需要删除的目录/文件名称列表（相对于工作目录）
  - `script`: 字符串，bash shell 脚本，包含清理历史覆盖率产物和执行覆盖率命令的完整步骤

检测逻辑 SHALL 为：
1. 从 openspec/config.json 读取 `test.frameworks` 字段
2. 若 `test.frameworks` 为字符串，将其归一化为单条配置 `[{"glob": "<框架默认测试文件模式>", "framework": "<字符串值>"}]`，其中默认 glob 模式由框架名决定
3. 对于每个输入文件（或自动扫描到的文件），按数组中 glob 的顺序首匹配
4. 匹配到的第一个 glob 对应的 framework 即为该文件的框架归属
5. 无匹配时返回 `"unknown"`

文件路径与 glob 模式的匹配 SHALL 通过 `plugins/dev-team/bin/src/lib/glob.ts` 导出的 `matchGlob` 函数完成，SHALL NOT 使用手写 glob-to-regex 转换。

当 `files` 参数省略时，自动扫描 SHALL 调用 `scanProjectFiles(projectRoot, mappingGlobs)` 获取候选文件列表，其中 `mappingGlobs` 为归一化后所有 `{glob, framework}` 映射的 glob 字符串数组；SHALL NOT 使用自定义递归目录遍历加逐文件 regex 测试的方式。

`plan` 数组的生成逻辑 SHALL 为：
1. 对 `test.frameworks` 配置（归一化后）中的每个 `{glob, framework}` 条目：
   - 调用 `deriveWorkingDirectory(glob)` 推导工作目录
   - 调用 `test_get_framework_config` 内部函数获取该框架的命令配置（包括 `coverage_artifacts` 和 `coverage_cleanup`）
   - 生成一条计划记录 `{directory, framework, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup}`
   - 调用 `generateScript()` 根据该条目的 `directory`、`coverage_cleanup`、`coverage_cmd` 生成 `script` 字段
2. 如果 `test.frameworks` 未配置或为空，`plan` 为空数组
3. `plan` 数组的顺序与 `test.frameworks` 配置顺序一致

该工具 SHALL 遵循 `test_` 域名前缀的下划线命名约定。

框架默认测试文件模式：
- `jest`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `vitest`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `vite-plus`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `bun`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `rust`: `**/tests/**/*.rs`
- `node-test`: `**/*.test.{mjs,js,cjs}`
- `go`: `**/*_test.go`
- `pytest`: `**/test_*.py`

#### Scenario: test_detect_frameworks 返回 go 框架 plan

- **WHEN** config.json 中 `test.frameworks` 为 `"go"`
- **THEN** 返回的 `plan` 包含 `{framework: "go", coverage_format: "go-cover", coverage_output: "coverage/func-summary.txt", ...}`

#### Scenario: test_detect_frameworks 返回 pytest 框架 plan

- **WHEN** config.json 中 `test.frameworks` 为 `"pytest"`
- **THEN** 返回的 `plan` 包含 `{framework: "pytest", coverage_format: "coverage-py", coverage_output: "coverage.json", ...}`

#### Scenario: test_detect_frameworks 返回 node-test 框架 plan

- **WHEN** config.json 中 `test.frameworks` 为 `"node-test"`
- **THEN** 返回的 `plan` 包含 `{framework: "node-test", coverage_format: "node-test", coverage_output: "coverage/coverage-summary.json", ...}`
- **AND** `coverage_cmd` 包含两步脚本（native coverage + parser）

#### Scenario: test_detect_frameworks 返回包含 coverage_artifacts 和 coverage_cleanup 的 plan

- **WHEN** `test_detect_frameworks` 收到参数 `{"files": ["plugins/dev-team/bin/src/test.ts"]}`
- **AND** config.json 中 `test.frameworks` 定义为 `[{"glob": "plugins/dev-team/bin", "framework": "vite-plus"}]`
- **THEN** 返回的 `plan` 数组包含完整条目含 `script` 字段
- **AND** `detected` 数组中包含框架为 `"vite-plus"` 的条目

#### Scenario: test_detect_frameworks 的 plan 在无配置时为空

- **WHEN** config.json 中无 `test.frameworks` 配置（或为空数组）
- **THEN** 返回的 `plan` 为空数组 `[]`

### Requirement: MCP 工具 test_get_framework_config 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册一个名为 `test_get_framework_config` 的 MCP 工具。
该工具的输入 SHALL 包含：
- `framework`: 字符串，框架名称（如 `"vitest"`、`"jest"`、`"go"`）

该工具的输出 SHALL 包含：
- `framework`: 框架名称
- `test_cmd`: 测试命令字符串
- `coverage_cmd`: 覆盖率命令字符串
- `coverage_format`: 覆盖率输出格式标识（`"istanbul"`、`"llvm-cov"`、`"node-test"`、`"go-cover"` 或 `"coverage-py"`）
- `coverage_output`: 覆盖率输出文件路径（相对于框架工作目录）
- `coverage_artifacts`: 字符串数组，覆盖率摘要文件路径列表（SHALL NOT 使用 `coverage/**` glob）
- `coverage_cleanup`: 字符串数组，移动完成后需要清理的目录/文件列表

框架命令注册表 SHALL 为硬编码实现（不在 config.json 中配置）。支持的框架名及其命令配置：

| 框架名 | test_cmd | coverage_cmd | coverage_format | coverage_output | coverage_artifacts | coverage_cleanup |
|--------|----------|--------------|-----------------|-----------------|-------------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage --coverageReporters=json-summary` | istanbul | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", ".nyc_output"]` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage --coverage.reporter=json-summary` | istanbul | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", ".nyc_output"]` |
| vite-plus | `vp test` | `vp test --coverage --coverage.reporter=json-summary` | istanbul | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", ".nyc_output"]` |
| bun | `bun test` | `bun test --coverage --coverageReporters=json-summary` | istanbul | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage"]` |
| rust | `cargo test` | `cargo llvm-cov --json` | llvm-cov | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", "target/llvm-cov"]` |
| node-test | `node --test` | `node --test --experimental-test-coverage 2>&1 \| tee coverage/node-test-output.txt && node plugins/dev-team/scripts/parse-node-test-coverage.mjs coverage/node-test-output.txt coverage/coverage-summary.json` | node-test | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage"]` |
| go | `go test ./...` | `go test -coverprofile=coverage.out -covermode=atomic ./... && mkdir -p coverage && go tool cover -func=coverage.out > coverage/func-summary.txt` | go-cover | `coverage/func-summary.txt` | `["coverage/func-summary.txt"]` | `["coverage", "coverage.out"]` |
| pytest | `pytest -v` | `pytest --cov=. --cov-report=json --cov-branch -q` | coverage-py | `coverage.json` | `["coverage.json"]` | `[".coverage", "htmlcov"]` |

未知框架名称 SHALL 返回错误。

#### Scenario: test_get_framework_config 返回 node-test 配置

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "node-test"}`
- **THEN** 返回 `test_cmd: "node --test"`
- **AND** 返回 `coverage_format: "node-test"`
- **AND** 返回 `coverage_cmd` 包含 `--experimental-test-coverage` 和 `parse-node-test-coverage.mjs`

#### Scenario: test_get_framework_config 返回 go 配置

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "go"}`
- **THEN** 返回 `coverage_format: "go-cover"`
- **AND** 返回 `coverage_output: "coverage/func-summary.txt"`

#### Scenario: test_get_framework_config 返回 pytest 配置

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "pytest"}`
- **THEN** 返回 `coverage_format: "coverage-py"`
- **AND** 返回 `coverage_output: "coverage.json"`

#### Scenario: test_get_framework_config 返回 vitest 配置（JSON-only）

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "vitest"}`
- **THEN** 返回 `coverage_cmd: "npx vitest run --coverage --coverage.reporter=json-summary"`
- **AND** 返回 `coverage_artifacts: ["coverage/coverage-summary.json"]`

#### Scenario: test_get_framework_config 返回未知框架错误

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "unknown-framework"}`
- **THEN** 返回错误信息指示未知框架名称

## Module Contract

### Function: parseNodeTestCoverage

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/scripts/parse-node-test-coverage.mjs` |
| **CLI** | `node parse-node-test-coverage.mjs <stdout-file> <output-json-path>` |
| **Input** | node:test `--experimental-test-coverage` stdout 文本文件 |
| **Output** | istanbul 兼容 `coverage-summary.json` |
| **Exit codes** | 0 = 成功；非 0 = 解析失败 |

### Type: CoverageMeasured

| Property | Description |
|----------|-------------|
| **Definition** | `{lines: number, branches: number \| null, functions: number \| null}` |
| **Usage** | `coverage.measured`、`coverage.by_framework[].measured`、`coverage.overrides[].measured` |

### Framework Command Registry (extended)

| Framework | test_cmd | coverage_format | coverage_output |
|-----------|----------|-----------------|-----------------|
| node-test | `node --test` | node-test | `coverage/coverage-summary.json` |
| go | `go test ./...` | go-cover | `coverage/func-summary.txt` |
| pytest | `pytest -v` | coverage-py | `coverage.json` |

### Function: runTestGetFrameworkConfig

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-get-framework-config.ts` |
| **Registry** | `FRAMEWORK_REGISTRY` — 8 框架硬编码映射 |
| **coverage_format type** | `'istanbul' \| 'llvm-cov' \| 'node-test' \| 'go-cover' \| 'coverage-py'` |

### Function: detect_test_framework (Python)

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/utils/test-framework.py` |
| **Go detection** | 存在 `go.mod` → `("go", "go test ./...")` |
| **Returns** | `Tuple[str, str]` — `(framework_name, runner_command)` |

### Function: run_tests (Python)

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/utils/test-runner.py` |
| **New runners** | `go` → `go test ./...`；`node-test` → `node --test` |
| **Returns** | `TestResult` dataclass |
