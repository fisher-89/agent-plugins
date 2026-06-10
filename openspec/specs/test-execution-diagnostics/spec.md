## Requirements

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

## Requirements

### Requirement: Executor 使用框架检测工具确定覆盖率命令

unit-test-executor SHALL 调用 `test_detect_frameworks` MCP 工具检测项目中使用的测试框架。
检测到框架后，Executor SHALL 直接从 `test_detect_frameworks` 返回的 `plan` 数组中读取每个框架的执行计划，包括工作目录（`directory`）、覆盖率命令（`coverage_cmd`）、覆盖率格式（`coverage_format`）、覆盖率输出路径（`coverage_output`）、覆盖率产物列表（`coverage_artifacts`）和清理列表（`coverage_cleanup`）。
Executor 不再单独调用 `test_get_framework_config` 工具，也不单独执行 `test_cmd`。
Executor SHALL 为 `plan` 中的每个条目，在其对应的 `directory` 工作目录下运行 `coverage_cmd`（覆盖率命令已包含测试运行），捕获 stdout/stderr 和退出码。

覆盖率命令执行成功后，Executor SHALL 执行产物移动步骤：根据 `coverage_artifacts` 中的 glob 模式将产物从工作目录移动到统一位置 `openspec/changes/<change>/reports/coverage/<framework>/`。移动成功后，Executor SHALL 根据 `coverage_cleanup` 中的目录列表清理原始临时文件。移动失败时（如源路径不存在），Executor SHALL 记录错误到 findings 但不阻断流程。

移动成功后，Executor SHALL 更新 `coverage_output` 的解析路径指向统一位置（`reports/coverage/<framework>/coverage-summary.json`），并从统一位置读取覆盖率数据。

Executor SHALL 根据 `plan` 中的 `coverage_format` 字段选择对应的解析策略，从覆盖率输出中提取 lines / branches / functions 三个维度的百分比数值：
- `istanbul` 格式：从 `coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 读取
- `llvm-cov` 格式：从 `llvm-cov` JSON 输出的 `data[0].totals.lines.percent`、`data[0].totals.branches.percent`、`data[0].totals.functions.percent` 读取

覆盖率命令执行失败时（非零退出码），Executor SHALL：
- 将该框架的 `coverage` 三个维度均设为 0
- 记录错误信息到报告的 `findings` 字段
- 将该框架从 `html_reports` 中排除
- 跳过该框架的产物移动和清理步骤
- 不阻塞整体测试报告写入

#### Scenario: Executor 检测 vitest 框架并运行覆盖率命令，产物移动到统一位置

- **WHEN** unit-test-executor 发现项目中存在 `*.test.ts` 文件
- **THEN** 它调用 `test_detect_frameworks` 工具检测到 `vitest` 框架
- **AND** 从 `plan` 中读取 `coverage_cmd: "npx vitest run --coverage"`、`directory: "."`、`coverage_artifacts: ["coverage/**"]`、`coverage_cleanup: ["coverage", ".nyc_output"]`
- **AND** 在项目根目录运行 `npx vitest run --coverage`
- **AND** 将 `coverage/` 下的产物移动到 `reports/coverage/vitest/`
- **AND** 删除原始的 `coverage/` 和 `.nyc_output/` 目录
- **AND** 从 `reports/coverage/vitest/coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 提取三个维度的覆盖率百分比

#### Scenario: Executor 使用 plan 处理多框架多目录项目，产物按框架隔离

- **WHEN** `plan` 包含两条记录：`{directory: "plugins/dev-team/bin", framework: "vite-plus", ...}` 和 `{directory: ".", framework: "vitest", ...}`
- **THEN** Executor 先在 `plugins/dev-team/bin` 目录运行 `vp test --coverage`
- **AND** 将产物移动到 `reports/coverage/vite-plus/`
- **AND** 再在项目根目录运行 `npx vitest run --coverage`
- **AND** 将产物移动到 `reports/coverage/vitest/`
- **AND** `coverage_by_framework` 数组中 `html_report` 路径分别为 `"reports/coverage/vite-plus/index.html"` 和 `"reports/coverage/vitest/index.html"`
- **AND** `html_reports` 数组包含 `["reports/coverage/vite-plus/index.html", "reports/coverage/vitest/index.html"]`

#### Scenario: 覆盖率命令执行失败不阻塞测试报告，跳过移动和清理

- **WHEN** vitest 覆盖率命令因缺少 c8/istanbul 依赖而以非零退出码退出
- **THEN** Executor 将该框架的 `coverage` 三个维度均设为 0
- **AND** 在报告的 `findings` 中记录 "vitest coverage command failed: [错误信息]"
- **AND** 跳过该框架的产物移动和清理步骤
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
- **THEN** 它自动扫描项目中匹配 `test.frameworks[*].glob` 模式的文件
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
- `coverage_cmd`: 覆盖率命令字符串
- `coverage_format`: 覆盖率输出格式标识（`"istanbul"`、`"llvm-cov"`）
- `coverage_output`: 覆盖率输出文件路径（相对于框架工作目录）
- `coverage_artifacts`: 字符串数组，覆盖率产物 glob 列表（用于移动到统一位置）
- `coverage_cleanup`: 字符串数组，移动完成后需要清理的目录/文件列表

框架命令注册表 SHALL 为硬编码实现（不在 config.json 中配置）。支持的框架名及其命令配置：

| 框架名 | test_cmd | coverage_cmd | coverage_format | coverage_output | coverage_artifacts | coverage_cleanup |
|--------|----------|--------------|-----------------|-----------------|-------------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| vite-plus | `vp test` | `vp test --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| bun | `bun test` | `bun test --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| rust | `cargo test` | `cargo llvm-cov --all --coverage` | llvm-cov | `coverage/coverage-summary.json` | `["coverage/**", "target/llvm-cov/**"]` | `["coverage", "target/llvm-cov"]` |

未知框架名称 SHALL 返回错误。

#### Scenario: test_get_framework_config 返回 vitest 配置（含 artifact 字段）

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "vitest"}`
- **THEN** 返回包含 `coverage_artifacts: ["coverage/**"]`、`coverage_cleanup: ["coverage", ".nyc_output"]` 的对象
- **AND** 其他字段（`test_cmd`、`coverage_cmd`、`coverage_format`、`coverage_output`）保持不变

#### Scenario: test_get_framework_config 返回 rust 配置（含 artifact 字段）

- **WHEN** `test_get_framework_config` 收到参数 `{"framework": "rust"}`
- **THEN** 返回包含 `coverage_artifacts: ["coverage/**", "target/llvm-cov/**"]`、`coverage_cleanup: ["coverage", "target/llvm-cov"]` 的对象

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
3. 遍历 `coverage_artifacts` 中的每个 glob 模式：
   - 将匹配 glob 的所有文件和目录移动到目标路径下
   - 保持目录结构：如 `coverage/index.html` 移动到 `reports/coverage/<framework>/index.html`
4. 移动成功后，遍历 `coverage_cleanup` 中的每个条目：
   - 删除框架工作目录下对应的文件或目录（递归）
5. 更新 `coverage_output` 解析路径为 `reports/coverage/<framework>/coverage-summary.json`
6. 更新 `html_report` 路径为 `reports/coverage/<framework>/index.html`

安全性约定：
- 移动前检查源路径是否存在，不存在时跳过该条目并记录 warning 到 findings
- 移动失败时记录错误到 findings，不执行清理，不阻断流程
- 清理仅在移动成功后执行
- 清理路径限制在框架工作目录下，不允许跨目录清理
- `coverage_cleanup` 中的路径如果是目录，递归删除；如果是文件，仅删除该文件

如果框架的 `coverage_artifacts` 为空数组，Executor SHALL 跳过移动和清理步骤。

#### Scenario: vitest 框架产物移动到统一目录并清理

- **WHEN** vitest 覆盖率命令成功执行，生成了 `coverage/coverage-summary.json`、`coverage/index.html`、`coverage/lcov.info`、`.nyc_output/` 等文件
- **THEN** Executor 创建 `reports/coverage/vitest/` 目录
- **AND** 将 `coverage/` 下所有文件移动到 `reports/coverage/vitest/` 下
- **AND** 删除工作目录下的 `coverage/` 目录
- **AND** 删除工作目录下的 `.nyc_output/` 目录
- **AND** `coverage_output` 更新为 `reports/coverage/vitest/coverage-summary.json`
- **AND** `html_report` 更新为 `reports/coverage/vitest/index.html`

#### Scenario: 产物移动失败时不执行清理，不阻断流程

- **WHEN** coverage 命令执行成功但 `coverage/` 目录不存在（产物未生成）
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

### MCP Tools (`plugins/dev-team/bin/src/mcp.ts`)

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `test_detect_frameworks` | 检测文件所属的测试框架，返回检测结果和含 artifact 信息的执行计划 | `files?: string[]` — 文件路径列表（可选，不传则自动扫描）；`project_root?: string` | `{detected: {file, framework}[], frameworks: string[], plan: {directory, framework, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup, script}[]}` |
| `test_get_framework_config` | 获取指定框架的测试、覆盖率命令配置以及 artifact 管理配置 | `framework: string` — 框架名称 | `{framework, test_cmd, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup}` |

### `FrameworkConfig` Interface Change

| Field | Status | Type | Description |
|-------|--------|------|-------------|
| `framework` | 不变 | `string` | 框架名称 |
| `test_cmd` | 不变 | `string` | 测试命令 |
| `coverage_cmd` | 不变 | `string` | 覆盖率命令 |
| `coverage_format` | 不变 | `'istanbul' | 'llvm-cov'` | 覆盖率格式 |
| `coverage_output` | 不变 | `string` | 覆盖率输出文件路径 |
| `coverage_artifacts` | **新增** | `string[]` | 需要移动到统一位置的产物 glob 列表 |
| `coverage_cleanup` | **新增** | `string[]` | 移动成功后需要删除的目录/文件列表 |

### `PlanEntry` Interface Change

| Field | Status | Type | Description |
|-------|--------|------|-------------|
| `directory` | 不变 | `string` | 执行工作目录 |
| `framework` | 不变 | `string` | 框架名称 |
| `coverage_cmd` | 不变 | `string` | 覆盖率命令 |
| `coverage_format` | 不变 | `'istanbul' | 'llvm-cov'` | 覆盖率格式 |
| `coverage_output` | 不变 | `string` | 覆盖率输出路径（移动前相对于工作目录） |
| `coverage_artifacts` | **新增** | `string[]` | 产物 glob 列表 |
| `coverage_cleanup` | **新增** | `string[]` | 清理列表 |
| `script` | **新增** | `string` | 生成的 bash 执行脚本，包含 shebang、set -e、cd、rm -rf、coverage_cmd |

### `test_detect_frameworks` Output Schema Change

| Field | Status | Type | Description |
|-------|--------|------|-------------|
| `detected` | 不变 | `{file, framework}[]` | 文件级别框架归属 |
| `frameworks` | 不变 | `string[]` | 唯一框架名列表 |
| `plan[].coverage_artifacts` | **新增** | `string[]` | 产物 glob 列表 |
| `plan[].coverage_cleanup` | **新增** | `string[]` | 清理列表 |
| `plan[].script` | **新增** | `string` | bash 执行脚本，必填字段 |

### `test_get_framework_config` Output Schema Change

| Field | Status | Type | Description |
|-------|--------|------|-------------|
| `framework` | 不变 | `string` | 框架名称 |
| `test_cmd` | 不变 | `string` | 测试命令 |
| `coverage_cmd` | 不变 | `string` | 覆盖率命令 |
| `coverage_format` | 不变 | `string` | 覆盖率格式 |
| `coverage_output` | 不变 | `string` | 覆盖率输出路径 |
| `coverage_artifacts` | **新增** | `string[]` | 产物 glob 列表 |
| `coverage_cleanup` | **新增** | `string[]` | 清理列表 |

### `deriveWorkingDirectory` Function

| Aspect | Specification |
|--------|--------------|
| Module | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` |
| Export | `export function deriveWorkingDirectory(glob: string): string` |
| Input | `glob` — 测试框架的 glob 模式字符串 |
| Output | 工作目录路径字符串（相对于项目根目录，POSIX 风格） |
| Logic | 取第一个通配符（`*`、`?`、`{`）之前的路径前缀。无通配符时返回整个 glob。通配符在首个位置时返回 `"."` |
| Used by | `runTestDetectFrameworks` 内部调用，为每个 `test.frameworks` 条目生成 `plan` 中的 `directory` 字段 |

### `generateScript` 函数（新增）

| Aspect | Specification |
|--------|--------------|
| Module | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` |
| Export | `export function generateScript(params: { directory: string; coverage_cleanup: string[]; coverage_cmd: string }): string` |
| Input | `directory` — 执行工作目录（相对于项目根目录）；`coverage_cleanup` — 需要清理的目录/文件名称列表；`coverage_cmd` — 覆盖率命令 |
| Output | 完整的 bash 脚本字符串，包含 shebang、set -e、可选的 cd、rm -rf 清理行、覆盖率命令 |
| Deterministic | 是 — 相同输入始终产生相同输出 |
| Side effects | 无 — 纯函数，不访问文件系统或网络 |
| Used by | `runTestDetectFrameworks` 内部调用，为每个 plan 条目生成 `script` 字段 |

### Framework Command Registry

| Framework | test_cmd | coverage_cmd | coverage_format | coverage_output | coverage_artifacts | coverage_cleanup |
|-----------|----------|--------------|-----------------|-----------------|-------------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| vite-plus | `vp test` | `vp test --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| bun | `bun test` | `bun test --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| rust | `cargo test` | `cargo llvm-cov --all --coverage` | llvm-cov | `coverage/coverage-summary.json` | `["coverage/**", "target/llvm-cov/**"]` | `["coverage", "target/llvm-cov"]` |

> 注意：以上注册表保持不变。`test_cmd` 字段保留，但 Executor 不再直接使用它——Executor 统一使用 `coverage_cmd`（它已包含测试运行）。

### unit-test-executor Agent Process Change

| Step | Prior | New |
|------|-------|-----|
| 步骤 1 | 框架检测（test_detect_frameworks） | 框架检测（test_detect_frameworks），`plan` 包含 artifact 配置 |
| 步骤 2 | 逐目录执行覆盖率命令 | 逐目录执行覆盖率命令（不变） |
| 步骤 3 | 覆盖率解析 | **移动覆盖率产物到统一目录**：按 `coverage_artifacts` 移动产物，按 `coverage_cleanup` 清理 |
| 步骤 4 | 阈值判定 | 覆盖率解析（从统一位置读取） |
| 步骤 5 | 报告写入 | 阈值判定（不变） |
| 步骤 6 | — | 报告写入，路径使用统一位置 |

### Unified Coverage Artifacts Directory Structure

```
openspec/changes/<change>/
  reports/
    coverage/
      <framework-1>/          # 例如 vitest/
        coverage-summary.json  # Istanbul 格式覆盖率摘要
        index.html             # HTML 覆盖率报告
        lcov.info              # LCOV 格式覆盖率数据
        ...                    # 其他覆盖率产物
      <framework-2>/          # 例如 rust/
        coverage-summary.json  # llvm-cov 格式覆盖率摘要
        index.html             # HTML 覆盖率报告
        ...                    # 其他覆盖率产物
```

| Path Component | Description |
|----------------|-------------|
| `reports/coverage/` | 固定前缀，统一覆盖率产物根目录 |
| `<framework>/` | 框架名称目录，与 `plan[*].framework` 值一致 |
| `<filename>` | 原始产物文件名，保持原样迁移 |

### `generateScript` 脚本模板

```
#!/bin/bash
set -e
cd <directory>            # 仅当 directory != "." 时生成
rm -rf <cleanup[0]>       # 仅当 coverage_cleanup 非空时生成
rm -rf <cleanup[1]>       # ...
<coverage_cmd>            # 最后一行
```

| 行 | 条件 | 示例 |
|----|------|------|
| `#!/bin/bash` | 始终存在 | `#!/bin/bash` |
| `set -e` | 始终存在 | `set -e` |
| `cd <directory>` | 仅当 `directory !== "."` | `cd plugins/dev-team/bin` |
| `rm -rf <item>` | 对 `coverage_cleanup` 每项生成一行 | `rm -rf coverage` |
| `<coverage_cmd>` | 始终存在（最后一行） | `npx vitest run --coverage` |

### Script Generation Flow

```
runTestDetectFrameworks()
  |-- 对每个 test.frameworks 条目：
  |     |-- deriveWorkingDirectory(glob) → directory
  |     |-- runTestGetFrameworkConfig(framework) → { coverage_cmd, coverage_cleanup, ... }
  |     |-- generateScript({ directory, coverage_cleanup, coverage_cmd }) → script  <-- 新增步骤
  |     |-- plan.push({ directory, framework, coverage_cmd, ..., script })
  |
  v
test_detect_frameworks MCP tool output
  |-- plan[].script  ← 新字段
```

### 各框架脚本生成验证

| 框架 | directory | coverage_cleanup | coverage_cmd | 脚本含 cd | 脚本含 rm -rf |
|------|-----------|------------------|-------------|-----------|---------------|
| jest | 由 glob 推导 | `["coverage", ".nyc_output"]` | `npx jest --coverage` | 由 glob 决定 | 两行 |
| vitest | 由 glob 推导 | `["coverage", ".nyc_output"]` | `npx vitest run --coverage` | 由 glob 决定 | 两行 |
| vite-plus | 由 glob 推导 | `["coverage", ".nyc_output"]` | `vp test --coverage` | 由 glob 决定 | 两行 |
| bun | 由 glob 推导 | `["coverage"]` | `bun test --coverage` | 由 glob 决定 | 一行 |
| rust | 由 glob 推导 | `["coverage", "target/llvm-cov"]` | `cargo llvm-cov --all --coverage` | 由 glob 决定 | 两行 |

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
