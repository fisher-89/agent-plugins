## MODIFIED Requirements

### Requirement: Executor 使用框架检测工具确定覆盖率命令

unit-test-executor SHALL 调用 `test_detect_frameworks` MCP 工具检测项目中使用的测试框架。
检测到框架后，Executor SHALL 直接从 `test_detect_frameworks` 返回的 `plan` 数组中读取每个框架的执行计划，包括工作目录（`directory`）、覆盖率命令（`coverage_cmd`）、覆盖率格式（`coverage_format`）和覆盖率输出路径（`coverage_output`）。
Executor 不再单独调用 `test_get_framework_config` 工具，也不单独执行 `test_cmd`。
Executor SHALL 为 `plan` 中的每个条目，在其对应的 `directory` 工作目录下运行 `coverage_cmd`（覆盖率命令已包含测试运行），捕获 stdout/stderr 和退出码。
Executor SHALL 根据 `plan` 中的 `coverage_format` 字段选择对应的解析策略，从覆盖率输出中提取 lines / branches / functions 三个维度的百分比数值：
- `istanbul` 格式：从 `coverage-summary.json` 的 `total.lines.pct`、`total.branches.pct`、`total.functions.pct` 读取
- `llvm-cov` 格式：从 `llvm-cov` JSON 输出的 `data[0].totals.lines.percent`、`data[0].totals.branches.percent`、`data[0].totals.functions.percent` 读取

覆盖率命令执行失败时（非零退出码），Executor SHALL：
- 将该框架的 `coverage` 三个维度均设为 0
- 记录错误信息到报告的 `findings` 字段
- 将该框架从 `html_reports` 中排除
- 不阻塞整体测试报告写入

#### Scenario: Executor 使用 plan 逐目录执行 vitest 覆盖率命令

- **WHEN** unit-test-executor 调用 `test_detect_frameworks` 并收到 `plan` 包含 `{directory: "src", framework: "vitest", coverage_cmd: "npx vitest run --coverage", coverage_format: "istanbul", coverage_output: "coverage/coverage-summary.json"}`
- **THEN** Executor 在 `src` 目录下（而非项目根目录）运行 `npx vitest run --coverage`
- **AND** 从 `src/coverage/coverage-summary.json` 中提取 lines / branches / functions 百分比
- **AND** 不再单独运行 `test_cmd`

#### Scenario: Executor 使用 plan 处理多框架多目录项目

- **WHEN** `plan` 包含两条记录：`{directory: "plugins/dev-team/bin", framework: "vite-plus", coverage_cmd: "vp test --coverage", ...}` 和 `{directory: ".", framework: "vitest", coverage_cmd: "npx vitest run --coverage", ...}`
- **THEN** Executor 先在 `plugins/dev-team/bin` 目录运行 `vp test --coverage`
- **AND** 再在项目根目录（`.`）运行 `npx vitest run --coverage`
- **AND** `coverage_by_framework` 数组包含两条记录，各框架的 `html_report` 路径相对于其工作目录
- **AND** 报告中的 `coverage` 为各框架覆盖率按源文件数量加权的加权平均

#### Scenario: 覆盖率命令执行失败不阻塞测试报告

- **WHEN** vitest 覆盖率命令因缺少 c8/istanbul 依赖而以非零退出码退出
- **THEN** Executor 将该框架的 `coverage` 三个维度均设为 0
- **AND** 在报告的 `findings` 中记录 "vitest coverage command failed: [错误信息]"
- **AND** 继续完成报告写入，`coverage_pass` 根据各维度加权平均结果和 thresholds 做 ALL 判定

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

检测逻辑 SHALL 为：
1. 从 openspec/config.json 读取 `test.frameworks` 字段
2. 若 `test.frameworks` 为字符串，将其归一化为单条配置 `[{"glob": "<框架默认测试文件模式>", "framework": "<字符串值>"}]`，其中默认 glob 模式由框架名决定
3. 对于每个输入文件（或自动扫描到的文件），按数组中 glob 的顺序首匹配
4. 匹配到的第一个 glob 对应的 framework 即为该文件的框架归属
5. 无匹配时返回 `"unknown"`

`plan` 数组的生成逻辑 SHALL 为：
1. 对 `test.frameworks` 配置（归一化后）中的每个 `{glob, framework}` 条目：
   - 调用 `deriveWorkingDirectory(glob)` 推导工作目录
   - 调用 `test_get_framework_config` 内部函数获取该框架的命令配置
   - 生成一条计划记录 `{directory, framework, coverage_cmd, coverage_format, coverage_output}`
2. 如果 `test.frameworks` 未配置或为空，`plan` 为空数组
3. `plan` 数组的顺序与 `test.frameworks` 配置顺序一致

该工具 SHALL 遵循 `test_` 域名前缀的下划线命名约定。

框架默认测试文件模式：
- `jest`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `vitest`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `vite-plus`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `bun`: `**/*.{test,spec}.{js,ts,jsx,tsx}`
- `rust`: `**/tests/**/*.rs`

#### Scenario: test_detect_frameworks 返回包含 plan 的完整结果

- **WHEN** `test_detect_frameworks` 收到参数 `{"files": ["plugins/dev-team/bin/src/test.ts"]}`
- **AND** config.json 中 `test.frameworks` 定义为 `[{"glob": "plugins/dev-team/bin", "framework": "vite-plus"}]`
- **THEN** 返回的 `detected` 数组中包含框架为 `"vite-plus"` 的条目
- **AND** `plan` 数组包含 `{directory: "plugins/dev-team/bin", framework: "vite-plus", coverage_cmd: "vp test --coverage", coverage_format: "istanbul", coverage_output: "coverage/coverage-summary.json"}`
- **AND** `frameworks` 包含 `["vite-plus"]`

#### Scenario: test_detect_frameworks 的 plan 在无配置时为空

- **WHEN** config.json 中无 `test.frameworks` 配置（或为空数组）
- **THEN** 返回的 `plan` 为空数组 `[]`
- **AND** `detected` 中所有文件均为 `"unknown"`

#### Scenario: test_detect_frameworks 按 glob 匹配文件到框架

- **WHEN** `test_detect_frameworks` 收到参数 `{"files": ["src/utils/helper.test.ts", "tests/test_auth.rs", "unknown.js"]}`
- **AND** config.json 中 `test.frameworks` 定义为 `[{"glob": "**/*.test.ts", "framework": "vitest"}, {"glob": "**/test_*.rs", "framework": "rust"}]`
- **THEN** 返回 `{"detected": [{"file": "src/utils/helper.test.ts", "framework": "vitest"}, {"file": "tests/test_auth.rs", "framework": "rust"}, {"file": "unknown.js", "framework": "unknown"}], "frameworks": ["vitest", "rust"], "plan": [{"directory": ".", "framework": "vitest", ...}, {"directory": ".", "framework": "rust", ...}]}`

#### Scenario: test_detect_frameworks 自动扫描文件

- **WHEN** `test_detect_frameworks` 收到参数 `{}`（无 files 参数）
- **THEN** 它自动扫描项目中匹配 `test.frameworks[*].glob` 模式的文件
- **AND** 返回所有匹配文件的框架归属及完整的 `plan` 数组

#### Scenario: test_detect_frameworks 首匹配规则

- **WHEN** 一个文件同时匹配两个 glob 模式（如 `tests/e2e/test_app.ts` 匹配 `**/*.ts` 和 `**/e2e/**`）
- **THEN** 返回 config.json 中 `frameworks` 数组里第一个匹配的框架

#### Scenario: test_detect_frameworks 处理字符串格式的 frameworks 配置

- **WHEN** config.json 中 `test.frameworks` 配置为字符串 `"vitest"`（而非数组）
- **THEN** `test_detect_frameworks` 内部将其归一化为 `[{"glob": "**/*.{test,spec}.{js,ts,jsx,tsx}", "framework": "vitest"}]`
- **AND** `plan` 包含 `{directory: ".", framework: "vitest", ...}`（默认 glob 的通配符在起始位置，工作目录为 `"."`）

## ADDED Requirements

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

## Module Contract

### MCP Tools (`plugins/dev-team/bin/src/mcp.ts`)

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `test_detect_frameworks` | 检测文件所属的测试框架，返回检测结果和执行计划 | `files?: string[]` — 文件路径列表（可选，不传则自动扫描）；`project_root?: string` | `{detected: {file, framework}[], frameworks: string[], plan: {directory, framework, coverage_cmd, coverage_format, coverage_output}[]}` |
| `test_get_framework_config` | 获取指定框架的测试和覆盖率命令配置（不变） | `framework: string` — 框架名称 | `{framework, test_cmd, coverage_cmd, coverage_format, coverage_output}` |

### Test Detect Frameworks Output Schema

| Field | Type | Description |
|-------|------|-------------|
| `detected` | `{file: string, framework: string}[]` | 文件级别的框架归属检测结果（不变） |
| `frameworks` | `string[]` | 检测到的唯一框架名称列表（不变） |
| `plan` | `{directory: string, framework: string, coverage_cmd: string, coverage_format: string, coverage_output: string}[]` | 执行计划数组，每项包含工作目录和覆盖率命令配置 |

### `deriveWorkingDirectory` Function

| Aspect | Specification |
|--------|--------------|
| Module | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` |
| Export | `export function deriveWorkingDirectory(glob: string): string` |
| Input | `glob` — 测试框架的 glob 模式字符串 |
| Output | 工作目录路径字符串（相对于项目根目录，POSIX 风格） |
| Logic | 取第一个通配符（`*`、`?`、`{`）之前的路径前缀。无通配符时返回整个 glob。通配符在首个位置时返回 `"."` |
| Used by | `runTestDetectFrameworks` 内部调用，为每个 `test.frameworks` 条目生成 `plan` 中的 `directory` 字段 |

### `runTestDetectFrameworks` Return Type Change

| Field | Status | Description |
|-------|--------|-------------|
| `detected` | 不变 | 文件级别的框架归属检测结果 |
| `frameworks` | 不变 | 检测到的唯一框架名称列表 |
| `plan` | **新增** | 执行计划数组，由 `deriveWorkingDirectory` + `test_get_framework_config` 生成 |

### Framework Command Registry

| Framework | test_cmd | coverage_cmd | coverage_format | coverage_output |
|-----------|----------|--------------|-----------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage` | istanbul | `coverage/coverage-summary.json` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage` | istanbul | `coverage/coverage-summary.json` |
| vite-plus | `vp test` | `vp test --coverage` | istanbul | `coverage/coverage-summary.json` |
| bun | `bun test` | `bun test --coverage` | istanbul | `coverage/coverage-summary.json` |
| rust | `cargo test` | `cargo llvm-cov --all --coverage` | llvm-cov | `coverage/coverage-summary.json` |

> 注意：以上注册表保持不变。`test_cmd` 字段保留，但 Executor 不再直接使用它——Executor 统一使用 `coverage_cmd`（它已包含测试运行）。

### unit-test-executor Agent Process Change

| Step | 原流程 | 新流程 |
|------|--------|--------|
| 步骤 1 | 框架检测（test_detect_frameworks） | 框架检测（test_detect_frameworks），返回包含 `plan` 的完整结果 |
| 步骤 2 | 命令解析（test_get_framework_config） | **移除**：直接从 `plan` 获取命令配置 |
| 步骤 3 | 测试执行（运行 test_cmd，全局根目录） | 逐目录执行覆盖率命令（运行 `plan` 中每个条目的 `coverage_cmd`，在其 `directory` 下） |
| 步骤 4 | 覆盖率执行（运行 coverage_cmd，全局根目录） | **合并到步骤 3**：覆盖率命令已包含测试运行 |
| 步骤 5 | 覆盖率解析 | 覆盖率解析（不变） |
| 步骤 6 | 阈值判定 | 阈值判定（不变） |
| 步骤 7 | 报告写入 | 报告写入（不变） |
