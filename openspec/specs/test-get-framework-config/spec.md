## Purpose

This capability defined the `test_get_framework_config` MCP tool and its underlying `FRAMEWORK_REGISTRY` in `commands/test-get-framework-config.ts`. As of the `merge-framework-config-into-detect` change, the MCP tool is **removed** and the registry has been extracted to an internal lib module at `lib/test-framework.ts`, consumed by `test-detect-frameworks.ts` rather than exposed as a standalone MCP tool.

For framework configuration queries, consumers SHALL use `test_detect_frameworks` which returns coverage fields (`coverage_format`, `coverage_output`) in its `plan[]` entries. Platform cleanup paths live under generated `script.shell` / `script.cmd`.

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
| coverage_output | `string` | 覆盖率输出文件路径（供执行器解析） |
| coverage_cleanup | `string[]` | 清理列表（位于 `shell` / `cmd` 子对象） |
| default_glob | `string` | 默认测试文件 glob 模式 |

**注意**: `merge_mode`、`coverage_cmd` 和 `coverage_artifacts` 字段已移除。覆盖率解析仅使用 `coverage_output`。所有框架使用平台脚本执行（含覆盖率）。pytest 和 rust 通过链式命令模式在单条命令中完成测试+覆盖率两步。其他框架通过 `--coverage` 标志直接收集覆盖率。

---

### MCP Tool: test_get_framework_config (Removed)

| Aspect | Detail |
|--------|--------|
| Tool name | `test_get_framework_config` (removed) |
| Input | `{framework: TestFrameworks, project_root?: string}` |
| Reason | All output fields are already available in `test_detect_frameworks.plan[]`. No consumer actually needs `test_cmd`. |
| Migration | Call `test_detect_frameworks({files: [...]})` and use `plan[].framework` for framework name, `plan[].coverage_format`/`plan[].coverage_output` for config, and `plan[].script` for platform execution scripts. |

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
**THEN** each call SHALL return a valid `FrameworkConfig` object with non-empty platform `test_execution`, `coverage_output`, `coverage_cleanup`, `default_glob`
**AND** each returned object SHALL NOT have a `merge_mode`, `coverage_cmd`, or `coverage_artifacts` property

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
**Description**: `FrameworkConfig` SHALL 继续提供可选 `config_flag`。`test_execution` 模板 SHALL 使用 `{config_args}` 占位。此外本变更要求模板同时支持报告产物占位符（见 REQ-TF-TEF-1）。bun 的 `config_flag` SHALL 不再固定为 `null`（或由 prepare 特化等价支持显式 config），以便临时 bunfig。

#### Scenario: frameworks with config_flag expand via placeholder

**WHEN** framework 为 `jest` / `vitest` / `vite-plus`
**AND** suite 提供 `config`
**THEN** execute 展开后命令 SHALL 在 `{config_args}` 位置包含 `config_flag` 与带引号的绝对 POSIX config 路径（如 `--config "<abs>/vite.config.ts"`）

#### Scenario: bun supports config injection for plan artifacts

**WHEN** framework 为 `bun`
**AND** prepare 需要临时 bunfig
**THEN** registry/prepare 路径 SHALL 允许将 `--config "<absTempBunfig>"`（绝对 POSIX，带引号）注入命令
**AND** 用户长期 bunfig SHALL 仅被只读 overlay，不被修改

### Requirement: Registry aligns coverage paths to reportDir file channel

**ID**: REQ-TF-TEF-1
**Priority**: MUST
**Description**: `FRAMEWORK_REGISTRY` 中各框架的 `coverage_output` SHALL 改为相对 **reportDir** 的垂直约定文件名（不再使用 suite cwd 下 `coverage/...` 旧路径）。`coverage_format` 枚举 SHALL 支持 `'lcov'`（供 bun）。`shell` / `cmd` 下的 `coverage_cleanup` 列表语义 SHALL 废弃（产物不再落 suite cwd；execute 清空 reportDir）。

`test_execution` 模板 SHALL 含文件通道占位符（`{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` 等），并按框架选择原生文件输出旗标或交由 execute 条件 `>`。路径类旗标 SHALL 用引号包裹占位符（如 `--outputFile="{results_file}"`）。execute 展开后这些路径 SHALL 为绝对 POSIX 形式。

jest / vitest / vite-plus 模板 SHALL 用 CLI reporter 旗标覆盖用户配置中的 reporter（jest：`--reporters=default`；vitest / vite-plus：`--reporter=json`）。node-test 模板 SHALL 用 `--test-reporter-destination="{results_file}"`（原生落盘），MUST NOT 依赖 execute 段级 `>`。

#### Scenario: vitest coverage_output is reportDir-relative

**WHEN** 调用 `getFrameworkConfig({ framework: "vitest" })`
**THEN** `coverage_output` SHALL 为相对 reportDir 的约定名（如 `"coverage-summary.json"`）
**AND** SHALL NOT 为 `"coverage/coverage-summary.json"`

#### Scenario: jest template includes outputFile and coverage directory placeholders

**WHEN** 读取 jest `shell.test_execution` / `cmd.test_execution` 模板字符串
**THEN** 模板 SHALL 包含原生 JSON 文件输出旗标与 coverage 目录/reporter 旗标
**AND** 路径位置 SHALL 使用带引号的 `{results_file}` / `{report_dir}`（如 `--outputFile="{results_file}"`、`--coverageDirectory="{report_dir}"`）
**AND** 模板 SHALL 含 `--reporters=default` 以覆盖用户 jest reporter 配置

#### Scenario: vitest/vite-plus templates override reporters with json outputFile

**WHEN** 读取 vitest 或 vite-plus 的 `test_execution` 模板
**THEN** 模板 SHALL 含 `--reporter=json`
**AND** JSON 路径 SHALL 使用带引号的 `{results_file}`（如 `--outputFile="{results_file}"`）
**AND** SHALL NOT 含 `--outputFile.json=`
**AND** SHALL NOT 含 `{console_file}` / `--reporter=verbose`

#### Scenario: node-test template uses native reporter destination

**WHEN** 读取 node-test 的 `test_execution` 模板
**THEN** 模板 SHALL 含 `--test-reporter-destination="{results_file}"`
**AND** SHALL NOT 依赖 execute 追加的段级 `>`

### Requirement: bun registry uses lcov and explicit config

**ID**: REQ-TF-TEF-2
**Priority**: MUST
**Description**: bun 条目 SHALL：
- 移除伪 Jest 风格 `--coverageReporters=json-summary`
- `coverage_format` 为 `'lcov'`
- `coverage_output` 为相对 reportDir 的 `"lcov.info"`
- 支持显式 config（`config_flag` 非 null，或由 `preparePlanArtifacts` 特化注入 `--config`）以便加载临时 bunfig

覆盖率目录与 reporter SHALL 通过临时 bunfig（`coverageDir` + `coverageReporter=["lcov"]`）指向 planDir，而非 post-copy。

#### Scenario: bun coverage_format is lcov

**WHEN** 调用 `getFrameworkConfig({ framework: "bun" })`
**THEN** `coverage_format` SHALL 为 `"lcov"`
**AND** `coverage_output` SHALL 为 `"lcov.info"`（相对 reportDir）
**AND** `test_execution` 模板 SHALL NOT 包含 `--coverageReporters=json-summary`

#### Scenario: bun can load explicit config for temporary bunfig

**WHEN** execute 为 bun 生成临时 bunfig
**THEN** 最终命令 SHALL 能通过显式 `--config`（或 registry `config_flag` 等价物）加载该临时文件
**AND** MUST NOT 就地改写用户 `bunfig.toml`

---

## Module Contract (config_flag additions)

### Interface: FrameworkConfig（增量）

| Field | Change |
|-------|--------|
| `coverage_format` | 联合类型增加 `'lcov'` |
| `coverage_output` | 相对 reportDir |
| `shell.coverage_cleanup` / `cmd.coverage_cleanup` | 不再用于 suite cwd 清理（可删或忽略） |
| `config_flag` | bun 不再恒为 `null`（或 prepare 特化）；与 `{config_args}` 联用 |

### Module: lib/test-framework.ts (Internal)

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/lib/test-framework.ts` |
| coverage_format | 增加 `'lcov'`；bun 使用 `lcov` |
| coverage_output | 相对 reportDir 的垂直文件名 |
| templates | 含带引号路径占位的 `{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}`；execute 展开为绝对 POSIX |
| coverage_cleanup | 语义废弃（不再驱动 suite cwd cleanup） |
| bun config_flag | 支持显式 config / prepare 特化 |
| Consumers | `commands/test-detect-frameworks.ts` / execute `preparePlanArtifacts` |
| Non-goals | 不恢复 MCP `test_get_framework_config`；本 change 不实现 pytest `-c` / rust manifest |
