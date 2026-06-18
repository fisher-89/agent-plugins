## Requirements

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
  - `pass`: 布尔值，全局 + 所有 overrides 分组 ALL 达标（lines AND branches AND functions）
  - `measured`: `{lines: number, branches: number, functions: number}` — 加权平均测量值
  - `thresholds`: `{lines: number, branches: number, functions: number}` — 门禁阈值（从 config.json 读取）
  - `by_framework`: 各框架覆盖率详情数组（可能为空），每项包含 `framework`（string）、`measured`（`{lines, branches, functions}`）
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

### Requirement: 测试执行覆盖率检查

unit-test-evaluator 和 integration-test-evaluator SHALL 从 executor 产出的报告中读取覆盖率数据。
Evaluator SHALL 优先使用报告中的 `coverage.pass` 布尔值进行门控判断（当 `coverage` 非 null 时）。`coverage.pass` 为 true 当且仅当 `coverage.measured` 的 lines / branches / functions 三个维度全部达到 `coverage.thresholds` 各自阈值，且 `coverage.overrides` 中所有分组也全部通过（ALL 判定）。
如果 `coverage` 非 null 且 `coverage.pass` 为 false，Evaluator SHALL 将该 checklist 项标记为 fail，evidence 包含 `coverage.measured` 各维度百分比、对应 `coverage.thresholds`，以及 `coverage.overrides` 中的不达标详情。
如果 `coverage` 非 null 且 `coverage.pass` 为 true，Evaluator SHALL 将该 checklist 项标记为 pass，evidence 包含各维度覆盖率百分比。
如果报告中的 `coverage` 为 null（覆盖率工具未配置或覆盖率生成失败），Evaluator SHALL 将该 checklist 项标记为 pass 并在 evidence 中注明 "覆盖率检查未配置或生成失败，跳过"。
覆盖率阈值由 openspec/config.json 中的 `test.coverage.thresholds` 定义，默认 {lines: 80, branches: 70, functions: 75}。Executor 在计算 `coverage.pass` 时 SHALL 使用该阈值。

#### Scenario: 所有维度覆盖率满足阈值

- **WHEN** 报告显示 `coverage.pass` 为 true，`coverage.measured` 为 lines=85（阈值 80）、branches=75（阈值 70）、functions=80（阈值 75）
- **THEN** checklist 中覆盖率条目标记为 pass，evidence 包含三个维度的覆盖率百分比

#### Scenario: 某一维度覆盖率低于阈值

- **WHEN** 报告显示 `coverage.pass` 为 false，`coverage.measured` 为 lines=85（阈值 80 ✓）、branches=60（阈值 70 ✗）、functions=80（阈值 75 ✓）
- **THEN** checklist 中覆盖率条目标记为 fail，evidence 指出 "branches coverage 60% below threshold 70%"

#### Scenario: overrides 分组不通过导致 coverage.pass 为 false

- **WHEN** 全局 lines/branches/functions 均达标，但 `coverage.overrides` 中 `core/**` 条目设 lines=90，该目录实际 lines=85
- **THEN** `coverage.pass` 为 false，evidence 包含 "core/** lines=85 below override threshold 90"

#### Scenario: 覆盖率工具未配置或生成失败

- **WHEN** 报告中的 `coverage` 为 null（覆盖率生成未执行或失败）
- **THEN** checklist 中覆盖率条目标记为 pass，evidence 注明 "覆盖率检查未配置或生成失败，跳过"

### Requirement: Executor 使用框架检测工具确定覆盖率命令

unit-test-executor SHALL 调用 `test_detect_frameworks` MCP 工具检测项目中使用的测试框架。
检测到框架后，Executor SHALL 直接从 `test_detect_frameworks` 返回的 `plan` 数组中读取每个框架的执行计划，包括工作目录（`directory`）、覆盖率命令（`coverage_cmd`）、覆盖率格式（`coverage_format`）、覆盖率输出路径（`coverage_output`）、覆盖率产物列表（`coverage_artifacts`）和清理列表（`coverage_cleanup`）。
Executor 不再单独调用 `test_get_framework_config` 工具，也不单独执行 `test_cmd`。
Executor SHALL 为 `plan` 中的每个条目，在其对应的 `directory` 工作目录下运行 `coverage_cmd`（覆盖率命令已包含测试运行），捕获 stdout/stderr 和退出码。

覆盖率命令执行成功后，Executor SHALL 执行产物移动步骤：根据 `coverage_artifacts` 中的文件路径（JSON 摘要文件）将产物从工作目录移动到统一位置 `openspec/changes/<change>/reports/coverage/<framework>/`。移动成功后，Executor SHALL 根据 `coverage_cleanup` 中的目录列表清理原始临时文件。移动失败时（如源路径不存在），Executor SHALL 记录错误到 findings 但不阻断流程。

移动成功后，Executor SHALL 更新 `coverage_output` 的解析路径指向统一位置（`reports/coverage/<framework>/coverage-summary.json`），并从统一位置读取覆盖率数据。

Executor SHALL 根据 `plan` 中的 `coverage_format` 字段选择对应的解析策略，从覆盖率输出中提取 lines / branches / functions 三个维度的百分比数值：
- `istanbul` 格式：从 `coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 读取
- `llvm-cov` 格式：从 `llvm-cov` JSON 输出的 `data[0].totals.lines.percent`、`data[0].totals.branches.percent`、`data[0].totals.functions.percent` 读取

覆盖率命令执行失败时（非零退出码），Executor SHALL：
- 将该框架的 `measured` 三个维度均设为 0（写入 `coverage.by_framework` 对应条目）
- 记录错误信息到报告的 `findings` 字段
- 跳过该框架的产物移动和清理步骤
- 不阻塞整体测试报告写入

#### Scenario: Executor 检测 vitest 框架并运行覆盖率命令，产物移动到统一位置

- **WHEN** unit-test-executor 发现项目中存在 `*.test.ts` 文件
- **THEN** 它调用 `test_detect_frameworks` 工具检测到 `vitest` 框架
- **AND** 从 `plan` 中读取 `coverage_cmd: "npx vitest run --coverage --coverage.reporter=json-summary"`、`directory: "."`、`coverage_artifacts: ["coverage/coverage-summary.json"]`、`coverage_cleanup: ["coverage", ".nyc_output"]`
- **AND** 在项目根目录运行 `npx vitest run --coverage --coverage.reporter=json-summary`
- **AND** 将 `coverage/coverage-summary.json` 移动到 `reports/coverage/vitest/coverage-summary.json`
- **AND** 删除原始的 `coverage/` 和 `.nyc_output/` 目录
- **AND** 从 `reports/coverage/vitest/coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 提取三个维度的覆盖率百分比
- **AND** 写入 `coverage.by_framework` 条目 `{framework: "vitest", measured: {...}}`

#### Scenario: Executor 使用 plan 处理多框架多目录项目，产物按框架隔离

- **WHEN** `plan` 包含两条记录：`{directory: "plugins/dev-team/bin", framework: "vite-plus", ...}` 和 `{directory: ".", framework: "vitest", ...}`
- **THEN** Executor 先在 `plugins/dev-team/bin` 目录运行 `vp test --coverage --coverage.reporter=json-summary`
- **AND** 将 JSON 摘要移动到 `reports/coverage/vite-plus/coverage-summary.json`
- **AND** 再在项目根目录运行 `npx vitest run --coverage --coverage.reporter=json-summary`
- **AND** 将 JSON 摘要移动到 `reports/coverage/vitest/coverage-summary.json`
- **AND** `coverage.by_framework` 数组包含两条框架条目（仅 `framework` 和 `measured`，无 `html_report`）
- **AND** 报告不存在 `html_reports` 顶层数组或 `html_report` 字段

#### Scenario: 覆盖率命令执行失败不阻塞测试报告，跳过移动和清理

- **WHEN** vitest 覆盖率命令因缺少 c8/istanbul 依赖而以非零退出码退出
- **THEN** Executor 将该框架的 `measured` 三个维度均设为 0
- **AND** 在报告的 `findings` 中记录 "vitest coverage command failed: [错误信息]"
- **AND** 跳过该框架的产物移动和清理步骤
- **AND** 继续完成报告写入，`coverage.pass` 根据 `coverage.measured` 加权平均结果和 `coverage.thresholds` 做 ALL 判定

### Requirement: Executor 计算加权平均覆盖率和 coverage.pass

当项目使用多个测试框架时，unit-test-executor SHALL 按 lines / branches / functions 三个维度分别计算加权平均覆盖率。权重 SHALL 为各框架对应的源文件数量。

加权平均覆盖率计算公式（每个维度独立计算）：`sum(每个框架该维度覆盖率 × 该框架源文件数) / 总源文件数`

加权平均覆盖率 SHALL 写入报告中的 `coverage.measured: {lines, branches, functions}` 字段。

Executor SHALL 从 openspec/config.json 读取 `test.coverage.thresholds` 和 `test.coverage.overrides`，写入 `coverage.thresholds` 和 `coverage.overrides`。

`coverage.pass` 判定（ALL 逻辑）：
1. 全局：`coverage.measured.lines` >= `coverage.thresholds.lines` AND `coverage.measured.branches` >= `coverage.thresholds.branches` AND `coverage.measured.functions` >= `coverage.thresholds.functions`
2. 对于 `coverage.overrides` 中每个条目，匹配的目录单独计算覆盖率写入 `measured`，与该条目的 `thresholds`（缺失项继承全局）比对，写入 `pass`
3. ALL 通过（全局 + 所有 overrides 分组）→ `coverage.pass` = true；任一组任一维度不达标 → false

如果所有框架的覆盖率命令均失败或未配置覆盖率，`coverage` SHALL 为 `null`（而非含 `pass: false` 的空对象）。

#### Scenario: 加权平均覆盖率按维度计算

- **WHEN** vitest 框架 lines=90/branches=80/functions=85（对应 3 个源文件），pytest 框架 lines=70/branches=65/functions=75（对应 2 个源文件）
- **THEN** 加权平均 lines=`(90*3 + 70*2)/5 = 82`，branches=`(80*3 + 65*2)/5 = 74`，functions=`(85*3 + 75*2)/5 = 81`
- **AND** `coverage.measured` 字段写入 `{lines: 82, branches: 74, functions: 81}`

#### Scenario: 全局三维度全部达标时 coverage.pass 为 true

- **WHEN** `coverage.measured` 为 `{lines: 82, branches: 74, functions: 81}`，`coverage.thresholds` 为 `{lines: 80, branches: 70, functions: 75}`
- **THEN** lines 82≥80 ✓，branches 74≥70 ✓，functions 81≥75 ✓ → `coverage.pass` 为 `true`

#### Scenario: 某一维度不达标时 coverage.pass 为 false

- **WHEN** `coverage.measured` 为 `{lines: 82, branches: 60, functions: 81}`，`coverage.thresholds` 为 `{lines: 80, branches: 70, functions: 75}`
- **THEN** branches 60<70 ✗ → `coverage.pass` 为 `false`

#### Scenario: overrides 分组校验 coverage.pass

- **WHEN** 全局 lines=85/branches=75/functions=80（阈值 80/70/75），但 `core/**` override 设 lines=90，该目录 lines=85
- **THEN** overrides 分组 lines 85<90 ✗ → `coverage.pass` 为 `false`
- **AND** `coverage.overrides` 包含一条 `{glob: "core/**", thresholds: {lines: 90, branches: 70, functions: 75}, measured: {lines: 85, ...}, pass: false}`

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
  - `coverage_format`: 字符串，覆盖率输出格式（`"istanbul"` 或 `"llvm-cov"`）
  - `coverage_output`: 字符串，覆盖率输出文件路径（相对于工作目录）
  - `coverage_artifacts`: 字符串数组，需要移动到统一位置的产物 glob 模式列表（相对于工作目录）
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

#### Scenario: test_detect_frameworks 返回包含 coverage_artifacts 和 coverage_cleanup 的 plan

- **WHEN** `test_detect_frameworks` 收到参数 `{"files": ["plugins/dev-team/bin/src/test.ts"]}`
- **AND** config.json 中 `test.frameworks` 定义为 `[{"glob": "plugins/dev-team/bin", "framework": "vite-plus"}]`
- **THEN** 返回的 `plan` 数组包含 `{directory: "plugins/dev-team/bin", framework: "vite-plus", coverage_cmd: "vp test --coverage", coverage_format: "istanbul", coverage_output: "coverage/coverage-summary.json", coverage_artifacts: ["coverage/**"], coverage_cleanup: ["coverage", ".nyc_output"], script: "#!/bin/bash\\nset -e\\ncd plugins/dev-team/bin\\nrm -rf coverage\\nrm -rf .nyc_output\\nvp test --coverage"}`
- **AND** `detected` 数组中包含框架为 `"vite-plus"` 的条目
- **AND** `frameworks` 包含 `["vite-plus"]`

#### Scenario: test_detect_frameworks 的 plan 在无配置时为空

- **WHEN** config.json 中无 `test.frameworks` 配置（或为空数组）
- **THEN** 返回的 `plan` 为空数组 `[]`
- **AND** `detected` 中所有文件均为 `"unknown"`

#### Scenario: test_detect_frameworks 按 glob 匹配文件到框架

- **WHEN** `test_detect_frameworks` 收到参数 `{"files": ["src/utils/helper.test.ts", "tests/test_auth.rs", "unknown.js"]}`
- **AND** config.json 中 `test.frameworks` 定义为 `[{"glob": "**/*.test.ts", "framework": "vitest"}, {"glob": "**/test_*.rs", "framework": "rust"}]`
- **THEN** 返回 `{"detected": [{"file": "src/utils/helper.test.ts", "framework": "vitest"}, {"file": "tests/test_auth.rs", "framework": "rust"}, {"file": "unknown.js", "framework": "unknown"}], "frameworks": ["vitest", "rust"], "plan": [{"directory": ".", "framework": "vitest", ..., "script": "#!/bin/bash\\nset -e\\nrm -rf coverage\\nrm -rf .nyc_output\\nnpx vitest run --coverage"}, {"directory": ".", "framework": "rust", ..., "script": "#!/bin/bash\\nset -e\\nrm -rf coverage\\nrm -rf target/llvm-cov\\ncargo llvm-cov --all --coverage"}]}`

#### Scenario: test_detect_frameworks 自动扫描文件

- **WHEN** `test_detect_frameworks` 收到参数 `{}`（无 files 参数）
- **THEN** 它通过 `scanProjectFiles` 自动扫描项目中匹配 `test.frameworks[*].glob` 模式的文件
- **AND** 返回所有匹配文件的框架归属及包含 `script` 的完整 `plan` 数组

#### Scenario: test_detect_frameworks 首匹配规则

- **WHEN** 一个文件同时匹配两个 glob 模式（如 `tests/e2e/test_app.ts` 匹配 `**/*.ts` 和 `**/e2e/**`）
- **THEN** 返回 config.json 中 `frameworks` 数组里第一个匹配的框架

#### Scenario: test_detect_frameworks 处理字符串格式的 frameworks 配置

- **WHEN** config.json 中 `test.frameworks` 配置为字符串 `"vitest"`（而非数组）
- **THEN** `test_detect_frameworks` 内部将其归一化为 `[{"glob": "**/*.{test,spec}.{js,ts,jsx,tsx}", "framework": "vitest"}]`
- **AND** `plan` 包含 `{directory: ".", framework: "vitest", coverage_artifacts: ["coverage/**"], coverage_cleanup: ["coverage", ".nyc_output"], script: "#!/bin/bash\\nset -e\\nrm -rf coverage\\nrm -rf .nyc_output\\nnpx vitest run --coverage", ...}`

### Requirement: generateScript 函数生成 bash 执行脚本

`plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` SHALL 导出 `generateScript` 函数，该函数根据 PlanEntry 的 `directory`、`coverage_cleanup`、`coverage_cmd` 字段生成完整的 bash shell 脚本。

函数签名 SHALL 为：
```typescript
export function generateScript(params: {
  directory: string;
  coverage_cleanup: string[];
  coverage_cmd: string;
}): string
```

生成的脚本 SHALL 遵循以下模板规则：
1. 第一行 SHALL 为 `#!/bin/bash`
2. 第二行 SHALL 为 `set -e`
3. 当 `directory` 不为 `"."` 时，SHALL 插入 `cd <directory>` 行（POSIX 路径格式）
4. 当 `directory` 为 `"."` 时，SHALL 跳过 `cd` 行
5. 对 `coverage_cleanup` 数组中的每个条目，SHALL 生成 `rm -rf <条目>` 行，顺序与数组一致
6. 当 `coverage_cleanup` 为空数组时，SHALL 不生成任何 `rm -rf` 行
7. 最后一行 SHALL 为 `coverage_cmd` 内容
8. 所有行之间 SHALL 使用 `\n`（换行符）分隔
9. 脚本末尾 SHALL 包含一个换行符

该函数 SHALL 为纯函数（deterministic）：相同输入始终产生相同输出。
该函数 SHALL 仅生成 bash 脚本，不支持 PowerShell 或其他 shell 变体。

生成逻辑 SHALL 仅在 PlanEntry 内部数据之间组合，不访问文件系统、网络或外部配置。

#### Scenario: 为 directory="." 的 vitest 框架生成脚本

- **WHEN** `generateScript({directory: ".", coverage_cleanup: ["coverage", ".nyc_output"], coverage_cmd: "npx vitest run --coverage"})`
- **THEN** 返回以下字符串：
```
#!/bin/bash
set -e
rm -rf coverage
rm -rf .nyc_output
npx vitest run --coverage
```
- **AND** 第一行为 `#!/bin/bash`
- **AND** 第二行为 `set -e`
- **AND** 不含 `cd` 行
- **AND** 包含两行 `rm -rf` 分别对应 coverage 和 .nyc_output
- **AND** 最后一行为 `npx vitest run --coverage`

#### Scenario: 为 directory="plugins/dev-team/bin" 的 vite-plus 框架生成脚本

- **WHEN** `generateScript({directory: "plugins/dev-team/bin", coverage_cleanup: ["coverage", ".nyc_output"], coverage_cmd: "vp test --coverage"})`
- **THEN** 返回的脚本包含 `cd plugins/dev-team/bin` 行
- **AND** 不含 `cd .` 行

#### Scenario: 为 rust 框架生成脚本包含目标目录清理

- **WHEN** `generateScript({directory: ".", coverage_cleanup: ["coverage", "target/llvm-cov"], coverage_cmd: "cargo llvm-cov --all --coverage"})`
- **THEN** 返回的脚本包含 `rm -rf coverage` 和 `rm -rf target/llvm-cov`
- **AND** 最后一行为 `cargo llvm-cov --all --coverage`

#### Scenario: coverage_cleanup 为空数组时跳过 rm -rf

- **WHEN** `generateScript({directory: ".", coverage_cleanup: [], coverage_cmd: "npx vitest run --coverage"})`
- **THEN** 返回的脚本不含任何 `rm -rf` 行
- **AND** 脚本内容为：
```
#!/bin/bash
set -e
npx vitest run --coverage
```

#### Scenario: 相同输入产生相同输出

- **WHEN** 两次调用 `generateScript` 使用完全相同的一组参数
- **THEN** 返回的字符串完全相同（===）

### Requirement: MCP 工具 test_get_framework_config 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册一个名为 `test_get_framework_config` 的 MCP 工具。
该工具的输入 SHALL 包含：
- `framework`: 字符串，框架名称（如 `"vitest"`、`"jest"`）

该工具的输出 SHALL 包含：
- `framework`: 框架名称
- `test_cmd`: 测试命令字符串
- `coverage_cmd`: 覆盖率命令字符串（SHALL 使用 JSON-only reporter 参数）
- `coverage_format`: 覆盖率输出格式标识（`"istanbul"`、`"llvm-cov"`）
- `coverage_output`: 覆盖率输出文件路径（相对于框架工作目录）
- `coverage_artifacts`: 字符串数组，覆盖率 JSON 摘要文件路径列表（SHALL NOT 使用 `coverage/**` glob）
- `coverage_cleanup`: 字符串数组，移动完成后需要清理的目录/文件列表

框架命令注册表 SHALL 为硬编码实现（不在 config.json 中配置）。支持的框架名及其命令配置：

| 框架名 | test_cmd | coverage_cmd | coverage_format | coverage_output | coverage_artifacts | coverage_cleanup |
|--------|----------|--------------|-----------------|-----------------|-------------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage --coverageReporters=json-summary` | istanbul | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", ".nyc_output"]` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage --coverage.reporter=json-summary` | istanbul | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", ".nyc_output"]` |
| vite-plus | `vp test` | `vp test --coverage --coverage.reporter=json-summary` | istanbul | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", ".nyc_output"]` |
| bun | `bun test` | `bun test --coverage --coverageReporters=json-summary` | istanbul | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", ".nyc_output"]` |
| rust | `cargo test` | `cargo llvm-cov --json` | llvm-cov | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage", "target/llvm-cov"]` |

未知框架名称 SHALL 返回错误。

#### Scenario: test_get_framework_config 返回 vitest 配置（JSON-only）

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "vitest"}`
- **THEN** 返回 `coverage_cmd: "npx vitest run --coverage --coverage.reporter=json-summary"`
- **AND** 返回 `coverage_artifacts: ["coverage/coverage-summary.json"]`
- **AND** 返回 `coverage_cleanup: ["coverage", ".nyc_output"]`

#### Scenario: test_get_framework_config 返回 rust 配置（JSON-only）

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "rust"}`
- **THEN** 返回 `coverage_cmd: "cargo llvm-cov --json"`
- **AND** 返回 `coverage_artifacts: ["coverage/coverage-summary.json"]`
- **AND** 返回 `coverage_cleanup: ["coverage", "target/llvm-cov"]`

#### Scenario: test_get_framework_config 返回未知框架错误

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "unknown-framework"}`
- **THEN** 返回错误信息指示未知框架名称

### Requirement: deriveWorkingDirectory 从 glob 模式推导执行目录

`plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` SHALL 导出 `deriveWorkingDirectory(glob: string): string` 函数。
该函数 SHALL 根据 glob 模式推导出命令执行的工作目录路径（相对于项目根目录）。

推导规则 SHALL 为：
1. 查找 glob 模式中第一个通配符字符的索引位置。通配符字符包括：`*`（星号）、`?`（问号）、`{`（左花括号）
2. 取该索引之前的路径前缀作为工作目录
3. 如果路径前缀以路径分隔符结尾（即通配符紧跟分隔符之后或位于首个位置），则返回 `"."`
4. 如果 glob 模式中不包含任何通配符，则整个 glob 字符串即工作目录
5. 路径分隔符统一使用 POSIX 风格的正斜杠 `/`（Windows 下将反斜杠归一化为正斜杠）

该函数 SHALL 归一化路径分隔符：
- Windows 反斜杠 `\` SHALL 被替换为正斜杠 `/`
- 多余的分隔符（如连续的 `/`）SHALL 被合并

#### Scenario: 无通配符的 glob 返回自身作为目录

- **WHEN** `deriveWorkingDirectory("plugins/dev-team/bin")`
- **THEN** 返回 `"plugins/dev-team/bin"`

#### Scenario: 含 `**` 通配符的 glob 取第一个通配符前路径

- **WHEN** `deriveWorkingDirectory("src/**/*.test.ts")`
- **THEN** 返回 `"src"`

#### Scenario: 通配符在起始位置的 glob 返回 `"."`

- **WHEN** `deriveWorkingDirectory("**/*.test.ts")`
- **THEN** 返回 `"."`

#### Scenario: 含 `{` 花括号的 glob 取花括号前路径

- **WHEN** `deriveWorkingDirectory("{src,lib}/*.test.ts")`
- **THEN** 返回 `"."`

#### Scenario: 含 `?` 通配符的 glob 取问号前路径

- **WHEN** `deriveWorkingDirectory("tests/?nit/*.test.ts")`
- **THEN** 返回 `"tests"`

#### Scenario: 含 `*` 通配符的 glob 取星号前路径

- **WHEN** `deriveWorkingDirectory("packages/*/src/__tests__/*.test.ts")`
- **THEN** 返回 `"packages"`

#### Scenario: Windows 反斜杠路径被归一化

- **WHEN** `deriveWorkingDirectory("plugins\\dev-team\\bin")`
- **THEN** 返回 `"plugins/dev-team/bin"`

### Requirement: Executor 移动覆盖率产物到统一目录并清理临时文件

unit-test-executor SHALL 在每条覆盖率命令执行成功后、覆盖率数据解析之前，将该框架的覆盖率产物移动到统一目录。

统一目录路径格式 SHALL 为：
```
openspec/changes/<change>/reports/coverage/<framework>/
```

其中 `<change>` 为当前工作变更名称，`<framework>` 为 `plan` 条目中的框架名称。

移动步骤 SHALL 如下：
1. 确定目标路径：`reports/coverage/<framework>/`（相对于变更目录）
2. 创建目标目录（如果不存在）
3. 遍历 `coverage_artifacts` 中的每个文件路径：
   - 将该 JSON 摘要文件移动到目标路径下
   - 保持文件名：如 `coverage/coverage-summary.json` 移动到 `reports/coverage/<framework>/coverage-summary.json`
4. 移动成功后，遍历 `coverage_cleanup` 中的每个条目：
   - 删除框架工作目录下对应的文件或目录（递归）
5. 更新 `coverage_output` 解析路径为 `reports/coverage/<framework>/coverage-summary.json`

安全性约定：
- 移动前检查源路径是否存在，不存在时跳过该条目并记录 warning 到 findings
- 移动失败时记录错误到 findings，不执行清理，不阻断流程
- 清理仅在移动成功后执行
- 清理路径限制在框架工作目录下，不允许跨目录清理
- `coverage_cleanup` 中的路径如果是目录，递归删除；如果是文件，仅删除该文件

如果框架的 `coverage_artifacts` 为空数组，Executor SHALL 跳过移动和清理步骤。

Executor SHALL NOT 移动、记录或引用 HTML 覆盖率报告（如 `index.html`）。

#### Scenario: vitest 框架 JSON 摘要移动到统一目录并清理

- **WHEN** vitest 覆盖率命令成功执行，生成了 `coverage/coverage-summary.json` 及 `.nyc_output/` 等临时文件
- **THEN** Executor 创建 `reports/coverage/vitest/` 目录
- **AND** 将 `coverage/coverage-summary.json` 移动到 `reports/coverage/vitest/coverage-summary.json`
- **AND** 删除工作目录下的 `coverage/` 目录
- **AND** 删除工作目录下的 `.nyc_output/` 目录
- **AND** `coverage_output` 更新为 `reports/coverage/vitest/coverage-summary.json`
- **AND** 报告不包含 `html_report` 或 `html_reports` 字段

#### Scenario: 产物移动失败时不执行清理，不阻断流程

- **WHEN** coverage 命令执行成功但 `coverage/coverage-summary.json` 不存在（产物未生成）
- **THEN** Executor 检查 `coverage/coverage-summary.json` 不存在
- **AND** 记录 "vitest coverage artifacts not found, skipping move" 到 findings
- **AND** 不执行清理
- **AND** 不阻断整体流程，`coverage` 设为 null
- **AND** 继续处理下一个框架

#### Scenario: coverage_artifacts 为空数组时跳过移动

- **WHEN** `plan` 条目的 `coverage_artifacts` 为空数组 `[]`
- **THEN** Executor 跳过该框架的移动和清理步骤
- **AND** 直接从原始 `coverage_output` 路径解析覆盖率数据（如果存在）

## Module Contract

### Coverage Report Schema (`reports/unit-test-execution.json`)

| Field | Type | Description |
|-------|------|-------------|
| `phase` | string | 阶段标识（06-unit-test 或 08-integration-test） |
| `command` | string | 实际执行的测试命令 |
| `timestamp` | string (ISO 8601) | 报告生成时间 |
| `total` | number | 总测试用例数 |
| `passed` | number | 通过数 |
| `failed` | number | 失败数 |
| `skipped` | number | 跳过数 |
| `duration_seconds` | number | 执行时长（秒） |
| `test_cases` | TestCase[] | 用例详情；失败条目含错误字段 |
| `coverage` | CoverageObject \| null | 嵌套覆盖率对象；null 表示未生成 |
| `integration_test` | IntegrationTestReport \| null | 集成测试子报告 |
| `findings` | string (optional) | 诊断信息 |

#### `TestCase` Type

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | 用例名称 |
| `file` | string | yes | 测试文件路径 |
| `duration_ms` | number | yes | 执行时长（毫秒） |
| `status` | `"passed"` \| `"failed"` \| `"skipped"` | yes | 用例状态 |
| `line` | number | when failed | 失败行号 |
| `error_type` | string | when failed | 错误类型（如 AssertionError） |
| `error_message` | string | when failed | 错误消息 |
| `stack_trace` | string | when failed | 堆栈跟踪 |
| `design_ref` | string | optional | 关联 test-design 引用 |

#### `CoverageObject` Type

| Field | Type | Description |
|-------|------|-------------|
| `pass` | boolean | 全局 + overrides ALL 达标 |
| `measured` | `{lines, branches, functions}` | 加权平均测量值 |
| `thresholds` | `{lines, branches, functions}` | 门禁阈值 |
| `by_framework` | `{framework, measured}[]` | 各框架详情 |
| `overrides` | `{glob, thresholds, measured, pass}[]` | overrides 校验结果 |

#### Removed Fields (deprecated)

| Field | Replacement |
|-------|-------------|
| `coverage_thresholds` | `coverage.thresholds` |
| `coverage_pass` | `coverage.pass` |
| `coverage_by_framework` | `coverage.by_framework`（`coverage` → `measured`） |
| `coverage_overrides` | `coverage.overrides`（`coverage` → `measured`） |
| `html_reports` | 删除（覆盖率仅 JSON 输出） |
| `failures` | `test_cases[]` where `status === "failed"` |

### Framework Command Registry

| Framework | coverage_cmd | coverage_artifacts |
|-----------|--------------|-------------------|
| jest | `npx jest --coverage --coverageReporters=json-summary` | `["coverage/coverage-summary.json"]` |
| vitest | `npx vitest run --coverage --coverage.reporter=json-summary` | `["coverage/coverage-summary.json"]` |
| vite-plus | `vp test --coverage --coverage.reporter=json-summary` | `["coverage/coverage-summary.json"]` |
| bun | `bun test --coverage --coverageReporters=json-summary` | `["coverage/coverage-summary.json"]` |
| rust | `cargo llvm-cov --json` | `["coverage/coverage-summary.json"]` |

### Unified Coverage Artifacts Directory Structure

```
openspec/changes/<change>/
  reports/
    coverage/
      <framework-1>/          # 例如 vitest/
        coverage-summary.json  # Istanbul / llvm-cov JSON 摘要（唯一保留产物）
      <framework-2>/          # 例如 rust/
        coverage-summary.json
```

### unit-test-executor Agent Report Writing Change

| Step | Prior | New |
|------|-------|-----|
| 覆盖率写入 | 6 个扁平字段 + `html_reports` | 单一嵌套 `coverage` 对象或 `null` |
| 失败详情 | 顶层 `failures[]` + `test_cases[]` | 仅 `test_cases[]`，失败条目含错误字段 |
| integration_test | 含 `failures[]` | 仅 `test_cases[]`，同上 |
| 产物移动 | 移动 `coverage/**` 含 HTML | 仅移动 JSON 摘要文件 |

### unit-test-evaluator Agent Consumption Change

| Check | Prior | New |
|-------|-------|-----|
| U1 必需字段 | 含 `coverage_thresholds`, `coverage_pass`, `coverage_by_framework`, `html_reports`, `failures` | 含 `coverage`（object\|null）、`test_cases` |
| U3 覆盖率 | `coverage_pass === true` 或 `coverage === null` | `coverage.pass === true` 或 `coverage === null` |
| 决策树输入 | `failures[]` | `test_cases` 中 `status: "failed"` 条目 |
| findings | `coverage_pass`, `coverage_by_framework`, `html_reports` | `coverage.pass`, `coverage.measured`, `coverage.by_framework[].measured` |

### Function: runTestDetectFrameworks (glob integration)

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-detect-frameworks.ts` |
| **Glob dependency** | `matchGlob` from `lib/glob.ts` — per-file framework detection |
| **Glob dependency** | `scanProjectFiles` from `lib/glob.ts` — auto-scan when `files` omitted |
| **Removed** | Private `globToRegex`, `matchGlob`, `collectFiles` functions |
| **Unchanged** | `deriveWorkingDirectory`, `normalizeFrameworks`, `generateScript` |
