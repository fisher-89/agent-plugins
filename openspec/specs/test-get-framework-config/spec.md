## Purpose

This capability defined the `test_get_framework_config` MCP tool and its underlying `FRAMEWORK_REGISTRY` in `commands/test-get-framework-config.ts`. As of the `merge-framework-config-into-detect` change, the MCP tool is **removed** and the registry has been extracted to an internal lib module at `lib/test-framework.ts`, consumed by `test-detect-frameworks.ts` rather than exposed as a standalone MCP tool.

For framework configuration queries, consumers SHALL use `test_detect_frameworks` which returns all fields (`coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`) in its `plan[]` entries.

As of the `cli-unit-test-execute` change, `FRAMEWORK_REGISTRY` entries have `merge_mode` **removed**. pytest and rust `test_cmd` are updated to chained shell commands. `test_cmd` supports template placeholders `{files}`, `{directory}`, `{project_root}` for runtime substitution. As of the `remove-coverage-cmd` change, `coverage_cmd` field is **removed** from `FrameworkConfig` — all coverage is now embedded within `test_cmd` via the chained command pattern or `--coverage` flags.

---

## Module Contract

### Module: lib/test-framework.ts (Internal)

The `FRAMEWORK_REGISTRY` constant, `getFrameworkConfig()`, and `getDefaultGlobForFramework()` are located in `lib/test-framework.ts` as an internal (non-MCP) module.

| Function | Input | Output | Side Effects |
|----------|-------|--------|-------------|
| `getFrameworkConfig({framework})` | `{framework: TestFrameworks}` | `FrameworkConfig` | None — pure lookup |
| `getDefaultGlobForFramework(framework)` | `framework: string` | `string` (glob pattern) | None — pure lookup |

All eight framework entries are preserved: jest, vitest, vite-plus, bun, rust, node-test, go, pytest.

### Interface: FrameworkConfig

| Field | Type | Description |
|-------|------|-------------|
| framework | `TestFrameworks` | 框架标识 |
| test_cmd | `string` | 测试命令模板（含覆盖率步骤），支持 `{files}`、`{directory}`、`{project_root}` 占位符。pytest 和 rust 为链式命令 |
| coverage_format | `'istanbul' \| 'llvm-cov' \| 'node-test' \| 'go-cover' \| 'coverage-py'` | 覆盖率输出格式 |
| coverage_output | `string` | 覆盖率输出文件路径 |
| coverage_artifacts | `string[]` | 需要移动的产物路径列表 |
| coverage_cleanup | `string[]` | 清理列表 |
| default_glob | `string` | 默认测试文件 glob 模式 |

**注意**: `merge_mode` 和 `coverage_cmd` 字段已移除。所有框架使用单一 `test_cmd` 执行（含覆盖率）。pytest 和 rust 通过链式命令 `; _X=$?; ...; exit $_X` 模式在单条命令中完成测试+覆盖率两步。其他框架通过 `test_cmd` 中的 `--coverage` 标志直接收集覆盖率。

---

### MCP Tool: test_get_framework_config (Removed)

| Aspect | Detail |
|--------|--------|
| Tool name | `test_get_framework_config` (removed) |
| Input | `{framework: TestFrameworks, project_root?: string}` |
| Reason | All output fields are already available in `test_detect_frameworks.plan[]`. No consumer actually needs `test_cmd`. |
| Migration | Call `test_detect_frameworks({files: [...]})` and use `plan[].framework` for framework name, `plan[].coverage_format`/`plan[].coverage_output`/`plan[].coverage_artifacts`/`plan[].coverage_cleanup` for config. `plan[].test_cmd` is available for the `dev-team unit-test` CLI command. |

---

## REMOVED Requirements

### Requirement: FrameworkConfig 增加 merge_mode 字段（原 REQ-TF-5）

**ID**: REQ-TF-5 (removed)
**Priority**: MUST
**Description**: 原设计：`lib/test-framework.ts` 的 `FrameworkConfig` 接口 SHALL 增加 `merge_mode: boolean` 字段，各框架按分类设置。

**Reason**: merge_mode 字段已整体移除。所有框架使用单一 `test_cmd`，pytest 和 rust 使用链式命令模式。不再需要布尔字段区分执行策略。

**Migration**: `FrameworkConfig` 接口不再包含 `merge_mode` 字段。pytest 的 `test_cmd` 更新为 `"pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X"`，rust 的 `test_cmd` 更新为 `"cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X"`。

---

## ADDED Requirements

### Requirement: test_cmd 支持模板占位符（保留并更新）

**ID**: REQ-TF-6
**Priority**: MUST
**Description**: `FrameworkConfig` 的 `test_cmd` SHALL 为模板字符串，支持以下占位符：
- `{files}` — 当前 entry 覆盖的源文件或测试文件列表（空格分隔），由执行者替换
- `{directory}` — 当前 entry 的工作目录（相对于 project_root），由执行者替换
- `{project_root}` — 项目根目录绝对路径，由执行者替换

各框架 `test_cmd` SHALL 更新如下：

| 框架 | test_cmd |
|------|---------|
| jest | `npx jest --verbose --json --coverage --coverageReporters=json-summary {files}` |
| vitest | `npx vitest run --reporter=json --coverage --coverage.reporter=json-summary {files}` |
| vite-plus | `vp test --coverage --coverage.reporter=json-summary {files}` |
| bun | `bun test --coverage --coverageReporters=json-summary {files}` |
| rust | `cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X`（链式命令） |
| node-test | `node --test --experimental-test-coverage {files}` |
| go | `go test -json -coverprofile=coverage.out -covermode=atomic {directory}` |
| pytest | `pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X`（链式命令） |

注意：
- pytest 和 rust 使用链式命令 `; _X=$?; ...; exit $_X` 模式，在单条命令中完成测试执行 + 覆盖率收集
- vitest 的 `test_cmd` 增加了 `--reporter=json` 标志以输出 JSON 格式供 json-parser 解析
- go 的 `test_cmd` 增加了 `-json` 标志供 go-parser 解析

#### Scenario: test_cmd 含 {files} 占位符

**WHEN** `getFrameworkConfig("vitest")` 被调用
**THEN** `test_cmd` SHALL 包含 `{files}` 占位符
**AND** `test_cmd` SHALL 包含 `--reporter=json`

#### Scenario: go test_cmd 含 {directory} 占位符

**WHEN** `getFrameworkConfig("go")` 被调用
**THEN** `test_cmd` SHALL 包含 `{directory}` 占位符
**AND** SHALL 为 `"go test -json -coverprofile=coverage.out -covermode=atomic {directory}"`

#### Scenario: rust test_cmd 为链式命令

**WHEN** `getFrameworkConfig("rust")` 被调用
**THEN** `test_cmd` SHALL 包含 `; _X=$?;`
**AND** `test_cmd` SHALL 包含 `exit $_X`
**AND** `test_cmd` SHALL 包含 `--output-path coverage/coverage-summary.json`

#### Scenario: pytest test_cmd 为链式命令

**WHEN** `getFrameworkConfig("pytest")` 被调用
**THEN** `test_cmd` SHALL 包含 `; _X=$?; pytest --cov=`
**AND** `test_cmd` SHALL 包含 `exit $_X`
**AND** `test_cmd` SHALL 包含 `-q`（减少 stdout 输出量）

### Requirement: test-detect-frameworks plan[] 含 test_cmd（无 merge_mode）

**ID**: REQ-TF-7
**Priority**: MUST
**Description**: `commands/test-detect-frameworks.ts` 的 `buildPlanFromMappings` 函数 SHALL 在 `PlanEntry` 中包含 `test_cmd` 字段，从 `getFrameworkConfig()` 获取值。`PlanEntry` SHALL NOT 包含 `merge_mode` 字段。`test_cmd` SHALL 在 `plan` 中以模板字符串形式原样传递，占位符由调用方（test-runner）替换。

#### Scenario: plan entry 包含 test_cmd

**WHEN** `runTestDetectFrameworks({})` 在配置了 vitest 的项目上执行
**THEN** 返回的 `plan[0].test_cmd` SHALL 等于 `getFrameworkConfig("vitest").test_cmd`
**AND** `plan[0]` SHALL NOT 包含 `merge_mode` 属性

#### Scenario: plan entry 不替换占位符

**WHEN** `runTestDetectFrameworks({})` 返回 plan 条目
**THEN** `plan[0].test_cmd` SHALL 包含原始占位符字符串（如 `{files}`），未被替换
**AND** 替换由调用方（test-runner）执行

#### Scenario: PlanEntry 类型无 merge_mode 字段

**WHEN** `PlanEntry` 接口被检查
**THEN** SHALL NOT 包含 `merge_mode` 字段
**AND** SHALL 包含 `test_cmd: string` 字段

---

## Requirements (from prior changes)

### Requirement: lib/test-framework.ts internal module exists

**ID**: REQ-TF-1
**Priority**: MUST
**Description**: A new internal module `lib/test-framework.ts` SHALL contain the `FRAMEWORK_REGISTRY` constant and the `getFrameworkConfig()` and `getDefaultGlobForFramework()` functions, preserving all eight framework entries (jest, vitest, vite-plus, bun, rust, node-test, go, pytest) with their complete configuration.

#### Scenario: lib/test-framework.ts exports the registry functions

**WHEN** `import { getFrameworkConfig, getDefaultGlobForFramework } from '../lib/test-framework'`
**THEN** both functions SHALL be exported and callable
**AND** `getFrameworkConfig("jest")` SHALL return a `FrameworkConfig` object with `framework: "jest"` and all 7 fields (excluding `merge_mode` and `coverage_cmd`)

#### Scenario: all eight frameworks are registered

**WHEN** `getFrameworkConfig()` is called for each of `["jest", "vitest", "vite-plus", "bun", "rust", "node-test", "go", "pytest"]`
**THEN** each call SHALL return a valid `FrameworkConfig` object with non-empty `test_cmd`, `coverage_artifacts`, `coverage_cleanup`, `default_glob`
**AND** each returned object SHALL NOT have a `merge_mode` or `coverage_cmd` property

### Requirement: test-detect-frameworks imports from lib/test-framework

**ID**: REQ-TF-2
**Priority**: MUST
**Description**: `commands/test-detect-frameworks.ts` SHALL import `getFrameworkConfig` and `getDefaultGlobForFramework` from `../lib/test-framework` instead of `./test-get-framework-config`.

#### Scenario: import path updated

**WHEN** the source of `commands/test-detect-frameworks.ts` is inspected
**THEN** the import line SHALL read `from '../lib/test-framework'`
**AND** SHALL NOT read `from './test-get-framework-config'`
**AND** all existing `test-detect-frameworks` unit tests SHALL pass

#### Scenario: test-detect-frameworks plan[] includes test_cmd without merge_mode

**WHEN** `commands/test-detect-frameworks.ts` is inspected
**THEN** `PlanEntry` type SHALL include `test_cmd: string`
**AND** `PlanEntry` SHALL NOT include `merge_mode`
**AND** `buildPlanFromMappings` SHALL populate `test_cmd` from `getFrameworkConfig()`

### Requirement: MCP tool test_get_framework_config removed

**ID**: REQ-TF-3
**Priority**: MUST
**Description**: The MCP tool `test_get_framework_config` SHALL be removed from `mcp.ts`, including its registration function, schema import, and the associated schema file.

#### Scenario: MCP registration removed

**WHEN** `mcp.ts` is inspected
**THEN** there SHALL be no call to `registerTestGetFrameworkConfigTool`
**AND** there SHALL be no import from `'./commands/test-get-framework-config'`
**AND** `schemas/index.ts` SHALL NOT export `testGetFrameworkConfigInputSchema` or `testGetFrameworkConfigOutputSchema`

#### Scenario: schema file deleted

**WHEN** the filesystem is inspected
**THEN** `schemas/test-get-framework-config.schema.ts` SHALL NOT exist
**AND** `commands/test-get-framework-config.ts` SHALL NOT exist

### Requirement: test-gen-generator stops calling test_get_framework_config

**ID**: REQ-TF-4
**Priority**: MUST
**Description**: The `test-gen-generator.md` agent SHALL be updated to remove all references to `test_get_framework_config`. The agent SHALL use the `frameworks[]` or `plan[].framework` values from `test_detect_frameworks` to determine framework name for syntax selection.

#### Scenario: agent references removed

**WHEN** `plugins/dev-team/agents/test-gen-generator.md` is inspected
**THEN** it SHALL NOT contain the string `test_get_framework_config`
**AND** the framework resolution step SHALL reference framework names from `test_detect_frameworks` result only

#### Scenario: framework name still available for syntax selection

**WHEN** `test_detect_frameworks({})` is called
**AND** the result contains `frameworks: ["vitest"]`
**THEN** the agent SHALL use `"vitest"` from the result to select the vitest test syntax

### Requirement: FrameworkConfig 增加可选 config_flag 与 {config_args} 占位

**ID**: REQ-TF-CFG-1
**Priority**: MUST
**Description**: `lib/test-framework.ts` 的 `FrameworkConfig` 接口 SHALL 增加可选字段 `config_flag: string | null`（或 `string | undefined`），表示向测试命令注入框架配置文件时使用的 CLI flag。

`FRAMEWORK_REGISTRY` SHALL 为支持独立配置文件的框架填充该字段，第一版至少包括：
- `jest` → `"--config"`
- `vitest` → `"--config"`
- `vite-plus` → `"--config"`

对不适用、无稳定统一 flag、或命令为链式且本 change 不纳入适配的框架（`go`、`rust`、`bun`、`node-test`、`pytest`），`config_flag` SHALL 为 `null` 或省略。

对 `config_flag` 非空的框架，其 `shell.test_execution` 与 `cmd.test_execution` 模板 SHALL 包含占位符 `{config_args}`（位置由模板决定，不得依赖生成器对整段字符串末尾 append）。`config_flag` 为 null 的框架模板可不含该占位符。

pytest 的 `-c`、rust 的 manifest 等语义 **不在本 requirement 必达范围**；若 suite 对这些框架声明 `config`，由 `test-detect-frameworks` 显式失败（见 REQ-TDF-SUITE-2）。

#### Scenario: vite-plus 提供 --config flag 且模板含 {config_args}

**WHEN** `getFrameworkConfig("vite-plus")` 被调用
**THEN** 返回的 `FrameworkConfig.config_flag` SHALL 为 `"--config"`
**AND** `shell.test_execution` 与 `cmd.test_execution` SHALL 包含子串 `{config_args}`

#### Scenario: vitest 与 jest 提供 --config flag 且模板含 {config_args}

**WHEN** `getFrameworkConfig("vitest")` 与 `getFrameworkConfig("jest")` 被调用
**THEN** 两者的 `config_flag` SHALL 均为 `"--config"`
**AND** 两者的 `shell.test_execution` / `cmd.test_execution` SHALL 包含 `{config_args}`

#### Scenario: go / pytest / rust 无 config_flag

**WHEN** `getFrameworkConfig("go")`、`getFrameworkConfig("pytest")` 或 `getFrameworkConfig("rust")` 被调用
**THEN** `config_flag` SHALL 为 `null` 或 `undefined`

#### Scenario: getFrameworkConfig 仍返回完整 FrameworkConfig

**WHEN** `getFrameworkConfig("vite-plus")` 被调用
**THEN** 返回对象 SHALL 仍包含既有字段（`shell`/`cmd`/`coverage_format`/`default_glob`/`mutation_framework` 等）
**AND** SHALL 额外包含 `config_flag`

---

## Module Contract (config_flag additions)

### Interface: FrameworkConfig（MODIFIED）

| Field | Type | Description |
|-------|------|-------------|
| `config_flag` | `string \| null`（可选） | 与 `{config_args}` 联用的 CLI flag；`null`/缺省表示不支持 suite.`config` 注入 |

### Module: lib/test-framework.ts

| Aspect | Detail |
|--------|--------|
| **Change** | `FRAMEWORK_REGISTRY` 补充 `config_flag`；支持注入的框架模板嵌入 `{config_args}` |
| **Consumers** | `commands/test-detect-frameworks.ts` 展开占位符生成 script |
| **Non-goals** | 不恢复 MCP `test_get_framework_config`；本 change 不实现 pytest `-c` / rust manifest |
