## MODIFIED Requirements

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

## Module Contract

### Function: runTestDetectFrameworks (glob integration)

| Property            | Description                                                            |
| ------------------- | ---------------------------------------------------------------------- |
| **Module**          | `commands/test-detect-frameworks.ts`                                   |
| **Glob dependency** | `matchGlob` from `lib/glob.ts` — per-file framework detection          |
| **Glob dependency** | `scanProjectFiles` from `lib/glob.ts` — auto-scan when `files` omitted |
| **Removed**         | Private `globToRegex`, `matchGlob`, `collectFiles` functions           |
| **Unchanged**       | `deriveWorkingDirectory`, `normalizeFrameworks`, `generateScript`      |
