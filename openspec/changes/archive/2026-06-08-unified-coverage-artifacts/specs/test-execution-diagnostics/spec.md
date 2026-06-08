## MODIFIED Requirements

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
- **THEN** 返回的 `plan` 数组包含 `{directory: "plugins/dev-team/bin", framework: "vite-plus", coverage_cmd: "vp test --coverage", coverage_format: "istanbul", coverage_output: "coverage/coverage-summary.json", coverage_artifacts: ["coverage/**"], coverage_cleanup: ["coverage", ".nyc_output"]}`
- **AND** `detected` 数组中包含框架为 `"vite-plus"` 的条目
- **AND** `frameworks` 包含 `["vite-plus"]`

#### Scenario: test_detect_frameworks 的 plan 在无配置时为空

- **WHEN** config.json 中无 `test.frameworks` 配置（或为空数组）
- **THEN** 返回的 `plan` 为空数组 `[]`
- **AND** `detected` 中所有文件均为 `"unknown"`

#### Scenario: test_detect_frameworks 按 glob 匹配文件到框架

- **WHEN** `test_detect_frameworks` 收到参数 `{"files": ["src/utils/helper.test.ts", "tests/test_auth.rs", "unknown.js"]}`
- **AND** config.json 中 `test.frameworks` 定义为 `[{"glob": "**/*.test.ts", "framework": "vitest"}, {"glob": "**/test_*.rs", "framework": "rust"}]`
- **THEN** 返回 `{"detected": [{"file": "src/utils/helper.test.ts", "framework": "vitest"}, {"file": "tests/test_auth.rs", "framework": "rust"}, {"file": "unknown.js", "framework": "unknown"}], "frameworks": ["vitest", "rust"], "plan": [{"directory": ".", "framework": "vitest", ..., "coverage_artifacts": ["coverage/**"], "coverage_cleanup": ["coverage", ".nyc_output"]}, {"directory": ".", "framework": "rust", ..., "coverage_artifacts": ["coverage/**", "target/llvm-cov/**"], "coverage_cleanup": ["coverage", "target/llvm-cov"]}]}`

#### Scenario: test_detect_frameworks 自动扫描文件

- **WHEN** `test_detect_frameworks` 收到参数 `{}`（无 files 参数）
- **THEN** 它自动扫描项目中匹配 `test.frameworks[*].glob` 模式的文件
- **AND** 返回所有匹配文件的框架归属及包含 `coverage_artifacts` 和 `coverage_cleanup` 的完整 `plan` 数组

#### Scenario: test_detect_frameworks 首匹配规则

- **WHEN** 一个文件同时匹配两个 glob 模式（如 `tests/e2e/test_app.ts` 匹配 `**/*.ts` 和 `**/e2e/**`）
- **THEN** 返回 config.json 中 `frameworks` 数组里第一个匹配的框架

#### Scenario: test_detect_frameworks 处理字符串格式的 frameworks 配置

- **WHEN** config.json 中 `test.frameworks` 配置为字符串 `"vitest"`（而非数组）
- **THEN** `test_detect_frameworks` 内部将其归一化为 `[{"glob": "**/*.{test,spec}.{js,ts,jsx,tsx}", "framework": "vitest"}]`
- **AND** `plan` 包含 `{directory: ".", framework: "vitest", coverage_artifacts: ["coverage/**"], coverage_cleanup: ["coverage", ".nyc_output"], ...}`

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

---

## ADDED Requirements

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

---

## Module Contract

### MCP Tools (`plugins/dev-team/bin/src/mcp.ts`)

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `test_detect_frameworks` | 检测文件所属的测试框架，返回检测结果和含 artifact 信息的执行计划 | `files?: string[]` — 文件路径列表（可选，不传则自动扫描）；`project_root?: string` | `{detected: {file, framework}[], frameworks: string[], plan: {directory, framework, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup}[]}` |
| `test_get_framework_config` | 获取指定框架的测试、覆盖率命令配置以及 artifact 管理配置 | `framework: string` — 框架名称 | `{framework, test_cmd, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup}` |

### Framework Command Registry

| Framework | test_cmd | coverage_cmd | coverage_format | coverage_output | coverage_artifacts | coverage_cleanup |
|-----------|----------|--------------|-----------------|-----------------|-------------------|-----------------|
| jest | `npx jest --verbose` | `npx jest --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| vitest | `npx vitest run --reporter=verbose` | `npx vitest run --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| vite-plus | `vp test` | `vp test --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| bun | `bun test` | `bun test --coverage` | istanbul | `coverage/coverage-summary.json` | `["coverage/**"]` | `["coverage", ".nyc_output"]` |
| rust | `cargo test` | `cargo llvm-cov --all --coverage` | llvm-cov | `coverage/coverage-summary.json` | `["coverage/**", "target/llvm-cov/**"]` | `["coverage", "target/llvm-cov"]` |

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

### `test_detect_frameworks` Output Schema Change

| Field | Status | Type | Description |
|-------|--------|------|-------------|
| `detected` | 不变 | `{file, framework}[]` | 文件级别框架归属 |
| `frameworks` | 不变 | `string[]` | 唯一框架名列表 |
| `plan[].coverage_artifacts` | **新增** | `string[]` | 产物 glob 列表 |
| `plan[].coverage_cleanup` | **新增** | `string[]` | 清理列表 |

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

### unit-test-executor Agent Process Change

| Step | Prior | New |
|------|-------|-----|
| 步骤 1 | 框架检测（test_detect_frameworks） | 框架检测（test_detect_frameworks），`plan` 包含 artifact 配置 |
| 步骤 2 | 逐目录执行覆盖率命令 | 逐目录执行覆盖率命令（不变） |
| 步骤 3 | 覆盖率解析 | **移动覆盖率产物到统一目录**：按 `coverage_artifacts` 移动产物，按 `coverage_cleanup` 清理 |
| 步骤 4 | 阈值判定 | 覆盖率解析（从统一位置读取） |
| 步骤 5 | 报告写入 | 阈值判定（不变） |
| 步骤 6 | — | 报告写入，路径使用统一位置 |
