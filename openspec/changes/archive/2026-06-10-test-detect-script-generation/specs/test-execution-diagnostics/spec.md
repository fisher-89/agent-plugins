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

## ADDED Requirements

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

## Module Contract

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

### `PlanEntry` Interface Change

| Field | Status | Type | Description |
|-------|--------|------|-------------|
| `directory` | 不变 | `string` | 执行工作目录 |
| `framework` | 不变 | `string` | 框架名称 |
| `coverage_cmd` | 不变 | `string` | 覆盖率命令 |
| `coverage_format` | 不变 | `'istanbul' \| 'llvm-cov'` | 覆盖率格式 |
| `coverage_output` | 不变 | `string` | 覆盖率输出路径（移动前相对于工作目录） |
| `coverage_artifacts` | 不变 | `string[]` | 产物 glob 列表 |
| `coverage_cleanup` | 不变 | `string[]` | 清理列表 |
| `script` | **新增** | `string` | 生成的 bash 执行脚本，包含 shebang、set -e、cd、rm -rf、coverage_cmd |

### `test_detect_frameworks` Output Schema Change

| Field | Status | Type | Description |
|-------|--------|------|-------------|
| `detected` | 不变 | `{file, framework}[]` | 文件级别框架归属 |
| `frameworks` | 不变 | `string[]` | 唯一框架名列表 |
| `plan[].coverage_artifacts` | 不变 | `string[]` | 产物 glob 列表 |
| `plan[].coverage_cleanup` | 不变 | `string[]` | 清理列表 |
| `plan[].script` | **新增** | `string` | bash 执行脚本，必填字段 |

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
