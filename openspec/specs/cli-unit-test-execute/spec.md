## MODIFIED Requirements

### Requirement: dev-team test-execution CLI command (renamed from unit-test)

`plugins/dev-team/bin/src/cli.ts` SHALL register a `test-execution` subcommand instead of `unit-test`, associated with `commands/test-execution.ts` action handler. The command SHALL support `--project-root <path>` option. When executed, SHALL call `runTestDetectFrameworks({})` to get the framework plan.

**Changes from previous version**:
- CLI command name: `unit-test` → `test-execution`（保持不变）
- Report output paths: `reports/test-execution.json` → `reports/test/summary.json`
- Sub-report paths: `reports/test-execution/<fw>.json` → `reports/test/<planId>/report.json`
- Schema file: `schemas/test-execution-output.schema.ts`（保持）；summary 增加 `plans[]`

#### Scenario: test-execution subcommand registered

**WHEN** `dev-team test-execution` is called
**THEN** CLI SHALL parse the subcommand and route to `runTestExecution` function
**AND** `runTestExecution` SHALL call `runTestDetectFrameworks({})` to get the framework plan
**AND** SHALL execute each framework in the plan

#### Scenario: --project-root option support

**WHEN** `dev-team test-execution --project-root /custom/path` is called
**THEN** SHALL use `/custom/path` as project root for all internal functions
**AND** test report files SHALL be written under `/custom/path/reports/test/`（无 `--change` 时）

#### Scenario: deprecated unit-test command

**WHEN** `dev-team unit-test` is called
**THEN** CLI SHALL output a deprecation message: "`unit-test` has been renamed to `test-execution`. Please use `dev-team test-execution`."
**AND** SHALL still route to `runTestExecution` (backward compatibility)

#### Scenario: writes summary and plan directories under reports/test

**WHEN** `dev-team test-execution` completes（无 `--change`）
**THEN** summary report SHALL exist at `reports/test/summary.json`
**AND** each attempted plan SHALL have atomic report at `reports/test/<planId>/report.json`
**AND** SHALL NOT write `reports/test-execution.json` or `reports/test-execution/<planId>.json`

### Requirement: Schema file renamed and updated

`schemas/unit-test-output.schema.ts` SHALL be renamed to `schemas/test-execution-output.schema.ts`. The `phase` field in the summary report schema SHALL be updated to accept `"test-execution"` instead of `"06-unit-test"` or `"unit-test"`.

#### Scenario: schema phase field updated

**WHEN** reading `schemas/test-execution-output.schema.ts`
**THEN** the SummaryReport schema `phase` field references `"test-execution"`
**AND** the SubReport schema `framework` field is unchanged

## ADDED Requirements

### Requirement: CLI source file renamed

**ID**: REQ-TEC-1
**Priority**: MUST
**Description**: `commands/unit-test.ts` SHALL be renamed to `commands/test-execution.ts`. The exported interface `UnitTestOptions` SHALL be renamed to `TestExecutionOptions`. The exported function `runUnitTest` SHALL be renamed to `runTestExecution`. All internal type names SHALL follow the same naming convention.

#### Scenario: renamed exports exist

**WHEN** importing from `commands/test-execution.ts`
**THEN** `runTestExecution` is available (not `runUnitTest`)
**AND** `TestExecutionOptions` type is available (not `UnitTestOptions`)

### Requirement: Plan artifact directory layout and planId

**ID**: REQ-TEF-LAYOUT-1
**Priority**: MUST
**Description**: CLI SHALL 将每个 plan 的产物写入 `reports/test/<planId>/`（有 `--change` 时前缀为 `openspec/changes/<change>/reports/test/<planId>/`）。`planId` SHALL 由 `plan.root` 经既有消毒算法生成目录 id（不含 `.json`）：
- `root === '.'` → `planId = "<framework>"`（无前缀、无前导 `_`）
- 其他 → `sanitize(root) + "_" + framework`（路径分隔符替换为 `_`）

跨框架强制统一文件名仅：
- `reports/test/summary.json`（聚合报告）
- `reports/test/<planId>/report.json`（原子报告）

测试结果 / 覆盖率 / mutation 文件名由各框架垂直定义。CLI / phase 名仍为 `test-execution`。

#### Scenario: root suite planId is framework name

**WHEN** plan entry `root` 为 `"."` 且 `framework` 为 `"vitest"`
**THEN** `planId` SHALL 为 `"vitest"`
**AND** reportDir SHALL 为 `reports/test/vitest/`（相对对应 reports 根）

#### Scenario: nested suite planId sanitizes root

**WHEN** plan entry `root` 为 `"plugins/dev-team/bin/src"`、`cwd` 为 `"plugins/dev-team/bin"` 且 `framework` 为 `"vite-plus"`
**THEN** `planId` SHALL 为 `"plugins_dev-team_bin_src_vite-plus"`
**AND** atomic report SHALL 写入 `reports/test/plugins_dev-team_bin_src_vite-plus/report.json`

#### Scenario: explicit --files skips suites with no matching root

**WHEN** config 含两个 suite（root `pkg/a` 与 `pkg/b`）
**AND** CLI 以 `--files pkg/a/foo.test.ts` 执行
**THEN** 仅 `pkg/a` 对应 plan SHALL 被执行并写入 `report.json`
**AND** `pkg/b` plan SHALL 被跳过（不得因空 files 回落全 suite 发现）

#### Scenario: change-scoped reports root

**WHEN** `dev-team test-execution --change my-feature` 执行完成
**THEN** summary SHALL 位于 `openspec/changes/my-feature/reports/test/summary.json`
**AND** plan 目录 SHALL 位于 `openspec/changes/my-feature/reports/test/<planId>/`

### Requirement: summary.plans path index

**ID**: REQ-TEF-PLANS-1
**Priority**: MUST
**Description**: `summary.json` SHALL 保留既有聚合字段，并新增 `plans[]` 数组。每个元素 SHALL 包含且仅作为路径索引：
- `id`：与 `planId` / 目录名相同
- `framework`
- `root`：plan entry 的 `root`（与 planId 同源）
- `path`：相对 **project root** 的 plan 目录（无 change：`reports/test/<planId>`；有 change：`openspec/changes/<change>/reports/test/<planId>`）

每个尝试执行的 plan SHALL 进入 `plans[]`（含 prepare/执行失败）。`plans[]` SHALL NOT 携带 status；成败与原因落在对应 `report.json` 以及 summary 的 `conclusion` / `problems`。

#### Scenario: plans index lists attempted plans with project-relative path

**WHEN** CLI 执行两个 plan（一个成功、一个失败）后写 summary
**THEN** `plans` SHALL 长度为 2
**AND** 每个元素 SHALL 含 `id`、`framework`、`root`、`path`
**AND** `path` SHALL 为相对 project root 的 POSIX 风格路径
**AND** 失败 plan 仍出现在 `plans[]`

#### Scenario: plans index does not embed status

**WHEN** 读取 `summary.json` 的 `plans[]` 元素
**THEN** 元素 SHALL NOT 包含 `status` / `conclusion` / `exit_code` 字段

### Requirement: preparePlanArtifacts before execute

**ID**: REQ-TEF-PREP-1
**Priority**: MUST
**Description**: `executePlanEntry` SHALL 在运行测试命令前调用 `preparePlanArtifacts`：
1. 决议 `reportDir`；`mkdir`；清空**该** `reportDir` 内既有内容（不影响其他 plan）
2. 返回 `configArgs`、`placeholders`（至少含 `report_dir`、`results_file`、`coverage_file`）、`redirectStdoutToResults`、`tempPaths`
3. `placeholders` 中的路径 SHALL 为指向 `reportDir`（或其内文件）的**绝对** POSIX 路径（`/` 分隔）；`configArgs`（含用户 config 与 bun 临时 bunfig）SHALL 为 `` `${config_flag} "<absPosix>"` ``
4. 展开占位符后，若 `redirectStdoutToResults === true`，SHALL 对测试结果段追加壳层 `> "{results_file}"`；若为 false（原生 outputFile 族），SHALL NOT 追加
5. 执行结束后 best-effort 删除 `tempPaths`（失败不阻断报告）

临时测试 config / bunfig SHALL 仅在 CLI 无法把产物指到 plan 目录时创建（默认仅 bun）；临时文件落在 suite cwd，用后删除；MUST NOT 修改用户长期 config / bunfig。bun 临时 bunfig 的 `coverageDir` SHALL 使用绝对 POSIX `report_dir`。

prepare 失败时该 plan SHALL 记为 `execution_error`（或等价），仍写 `report.json` 并进入 `plans[]`。

#### Scenario: clears only the current plan directory before run

**WHEN** `reports/test/vitest/` 与 `reports/test/jest/` 均已存在旧文件
**AND** 即将执行 planId=`vitest`
**THEN** SHALL 清空 `reports/test/vitest/`
**AND** SHALL NOT 删除或清空 `reports/test/jest/`

#### Scenario: native outputFile frameworks do not redirect stdout

**WHEN** framework 为 `jest`（或其它 `redirectStdoutToResults=false` 的框架）
**AND** `preparePlanArtifacts` 返回后拼命令
**THEN** 最终命令 SHALL NOT 以壳层 `> "{results_file}"` 追加测试结果段
**AND** 命令 SHALL 含框架原生文件输出旗标，且路径为指向 planDir 内垂直结果文件的绝对 POSIX 值（模板侧带引号）

#### Scenario: placeholders and configArgs use absolute POSIX paths

**WHEN** `preparePlanArtifacts` 为任一框架返回 placeholders / configArgs
**THEN** `report_dir` / `results_file` / `coverage_file`（及适用的 `coverprofile_file`）SHALL 为绝对 POSIX 路径
**AND** 若注入 `--config`，路径 SHALL 为带引号的绝对 POSIX 路径
**AND** MUST NOT 将上述路径展开为相对 absCwd 的形式（避免 jest/vitest 等按 config root 二次解析产生歧义）

#### Scenario: redirect frameworks append shell redirect

**WHEN** framework 为 `bun` / `go` / `rust` / `pytest` 等无原生结果文件输出的测试段
**AND** `redirectStdoutToResults` 为 `true`
**THEN** 最终命令的测试结果段 SHALL 追加 `> "{results_file}"`（或 Windows cmd 等价重定向；`results_file` 为绝对 POSIX）

#### Scenario: temp files cleaned up after execute

**WHEN** prepare 创建了临时 bunfig 或临时 Stryker config（`cleanup`/tempPaths 非空）
**THEN** execute 结束（成功或失败）后 SHALL best-effort 删除这些临时文件
**AND** MUST NOT 修改用户仓库内长期 `bunfig.toml` / 长期 Stryker config

### Requirement: File-channel result collection and vertical parsers

**ID**: REQ-TEF-PARSE-1
**Priority**: MUST
**Description**: 测试用例结果采集 SHALL 优先使用框架原生输出到文件；无原生能力时使用段级重定向文件。Runner SHALL 通过垂直 `parsePlanArtifacts(framework, reportDir)`（或等价）从 plan 目录读取约定文件，MUST NOT 对整段脏 stdout 做 `JSON.parse`，MUST NOT 做括号扫描抽 JSON，MUST NOT 做 tee。

覆盖率：有侧车文件时从 plan 目录解析；无则 `coverage=null`。MUST NOT 从 jest 结果内 `coverageMap` fallback；MUST NOT 读取 suite cwd 下旧 `coverage/` 等路径。

垂直命名原则（design 可微调文件名，通道原则不变）：

| 框架 | 测试结果（示意） | 覆盖率（示意） | 采集 |
|------|------------------|----------------|------|
| jest | `results.json` | `coverage-summary.json` | 原生 outputFile / coverageDirectory；`--reporters=default` 覆盖用户 reporter |
| vitest / vite-plus | `results.json` | `coverage-summary.json` | 原生 reporter/outputFile；CLI `--reporter=json` 覆盖用户 reporter |
| bun | `results.txt` | `lcov.info` | 测试 `>`；覆盖率临时 bunfig |
| go | `results.ndjson` | `func-summary.txt` (+ `coverage.out`) | 测试 `>`；coverprofile 原生路径 |
| rust | `results.txt` | `coverage-summary.json` | 测试 `>`；llvm-cov `--output-path` |
| pytest | `results.txt` | `coverage.json` | 测试段 `>`；`--cov-report=json:path` |
| node-test | `results.txt` | （可含于同一文本） | 原生 `--test-reporter-destination`（无段级 `>`） |

横向 coverage-parser SHALL 保留为库函数，由垂直模块调用。

#### Scenario: jest reads results.json from planDir not stdout

**WHEN** jest plan 执行完成且 `<planDir>/results.json` 存在合法 JSON
**THEN** 垂直 parser SHALL 从该文件解析 testCases
**AND** SHALL NOT 对捕获的 stdout 整段执行 `JSON.parse`

#### Scenario: missing results file becomes execution_error

**WHEN** 测试命令退出码非 0
**AND** 约定的结果文件缺失、为空或无法解析
**THEN** 该 plan 的 `report.json` SHALL 记录 `execution_error`（或等价）
**AND** summary `problems` SHALL 计入该失败
**AND** 该 plan 仍出现在 `summary.plans[]`

#### Scenario: no coverage sidecar yields null coverage

**WHEN** plan 目录内无覆盖率侧车文件
**THEN** `ExecutionResult.coverage` / 原子报告 coverage SHALL 为 `null`
**AND** parser SHALL NOT 回退读取 suite cwd 旧路径
**AND** SHALL NOT 使用 jest `coverageMap` fallback

#### Scenario: no tee during execution

**WHEN** 执行任一框架测试命令
**THEN** runner SHALL NOT 实现 tee（同时写文件并保留完整实时 stdout 镜像）
**AND** 失败诊断 SHALL 依赖 plan 目录产物与 `report.json`

#### Scenario: node-test uses native reporter destination not shell redirect

**WHEN** framework 为 `node-test`
**AND** `preparePlanArtifacts` 返回后拼命令
**THEN** `redirectStdoutToResults` SHALL 为 `false`
**AND** 最终命令 SHALL 含原生 `--test-reporter-destination`（路径为 planDir 内 `results.txt` 的绝对 POSIX 值）
**AND** SHALL NOT 以壳层 `> "{results_file}"` 追加测试结果段

#### Scenario: CLI reporter flags override user-configured reporters

**WHEN** 执行 `jest` / `vitest` / `vite-plus` plan
**THEN** 最终命令 SHALL 显式指定 reporter，以覆盖用户框架配置中的 reporter（避免用户 html/junit 等仍触发）
**AND** 结构化结果仍写入 planDir 约定文件（如 `results.json`）

### Requirement: ExecutionResult carries plan artifact fields

**ID**: REQ-TEF-ER-1
**Priority**: MUST
**Description**: `ExecutionResult` SHALL 在现有字段上增量扩展（非替换类型）：
- `planId`：目录 id
- `reportDir`：plan 目录路径
- `resultsFile?`：实际读取的测试结果文件（垂直名）
- `error?`：保留；prepare / 解析失败原因

summary 的 `plans[]` SHALL 仅从 `ExecutionResult` 投影索引字段（`id` / `framework` / `root` / `path`）。

#### Scenario: execution result exposes planId and reportDir

**WHEN** `executePlanEntry` 成功返回
**THEN** `ExecutionResult.planId` SHALL 等于目录 id
**AND** `ExecutionResult.reportDir` SHALL 指向该 plan 产物目录

## REMOVED Requirements

### Requirement: dev-team unit-test CLI command exists
**Reason**: CLI command renamed from `unit-test` to `test-execution`.

**Migration**: Backward-compatible alias provided. All internal references updated to new command name.

### Requirement: Schema file at unit-test-output.schema.ts
**Reason**: Schema file renamed to `test-execution-output.schema.ts`.

**Migration**: All imports updated to new file path.

## Module Contract

### Module: commands/test-execution.ts (CLI Action Handler)

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/commands/test-execution.ts` |
| Exports | `runTestExecution(options: TestExecutionOptions): Promise<TestExecutionExitCode>` |
| Input | `TestExecutionOptions`: `{ projectRoot?: string }` |
| Output | `TestExecutionExitCode`: `0` / `1` |
| Side Effects | 读写 `reports/test/summary.json` 与 `reports/test/<planId>/report.json`（及 plan 目录内垂直产物） |

### Module: lib/test-report.ts

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/lib/test-report.ts` |
| Key APIs | `derivePlanId(root, framework)` → 目录 id（无 `.json`）；`generateSubReport` → 写 `<planId>/report.json`；`generateSummaryReport` → 写 `summary.json`（含 `plans[]`） |

### Module: lib/test-runner.ts

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/lib/test-runner.ts` |
| Key APIs | `preparePlanArtifacts(...)`；`executePlanEntry(...)`；`parsePlanArtifacts(framework, reportDir)`（或等价垂直分发） |
| Flow | mkdir/clear reportDir → prepare → expand placeholders → optional `>` → run → vertical parse → report.json → cleanup tempPaths |

### Module: schemas/test-execution-output.schema.ts (renamed)

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` |
| Exports | `testExecutionSubReportSchema`, `testExecutionSummaryReportSchema`, `TestExecutionSubReport`, `TestExecutionSummaryReport` |
| Phase field | `"test-execution"` (was `"06-unit-test"` or `"unit-test"`) |

---

## ADDED Requirements (Platform-Aware Test Execution)

### Requirement: buildTestCommand selects script by platform

**ID**: REQ-TEC-PLAT-1
**Priority**: MUST
**Description**: The `buildTestCommand()` function in `lib/test-runner.ts` SHALL select the test script from `entry.script` based on `process.platform`:

- On `process.platform === 'win32'`, SHALL use `entry.script.cmd`
- On other platforms, SHALL use `entry.script.shell`

Signature unchanged: `buildTestCommand(entry: TestPlan, projectRoot: string, files?: string[]): string`. Template substitution SHALL remain unchanged.

If `entry.script.cmd` is missing or empty on Windows, SHALL fall back to `entry.script.shell`.

#### Scenario: Windows selects script.cmd

**WHEN** `process.platform` is `'win32'`
**AND** `buildTestCommand(entry, ...)` is called
**AND** `entry.script.cmd` is a non-empty string
**THEN** the returned command SHALL be `substitutePlaceholders(entry.script.cmd, ...)`
**AND** SHALL NOT use `entry.script.shell`

#### Scenario: Unix selects script.shell

**WHEN** `process.platform` is `'linux'` or `'darwin'`
**AND** `buildTestCommand(entry, ...)` is called
**THEN** the returned command SHALL be `substitutePlaceholders(entry.script.shell, ...)`
**AND** SHALL NOT use `entry.script.cmd`

#### Scenario: Windows fallback when script.cmd is empty

**WHEN** `process.platform` is `'win32'`
**AND** `entry.script.cmd` is `''`
**THEN** `buildTestCommand()` SHALL fall back to `entry.script.shell`
**AND** SHOULD log a warning

### Requirement: resolveShell returns platform-appropriate shell

**ID**: REQ-TEC-PLAT-2
**Priority**: MUST
**Description**: `resolveShell()` SHALL return:

1. Non-Windows: `undefined` (system default shell)
2. Windows: `process.env.SHELL || process.env.COMSPEC || 'cmd.exe'`

#### Scenario: Unix returns undefined

**WHEN** `process.platform` is `'linux'`
**THEN** `resolveShell()` SHALL return `undefined`

#### Scenario: Windows with SHELL returns SHELL

**WHEN** `process.platform` is `'win32'`
**AND** `process.env.SHELL` is `'C:\\Program Files\\Git\\bin\\bash.exe'`
**THEN** `resolveShell()` SHALL return `'C:\\Program Files\\Git\\bin\\bash.exe'`

#### Scenario: Windows without SHELL returns COMSPEC

**WHEN** `process.platform` is `'win32'`
**AND** `process.env.SHELL` is `undefined`
**AND** `process.env.COMSPEC` is `'C:\\Windows\\system32\\cmd.exe'`
**THEN** `resolveShell()` SHALL return `'C:\\Windows\\system32\\cmd.exe'`

#### Scenario: Windows without SHELL and COMSPEC hardcodes cmd.exe

**WHEN** `process.platform` is `'win32'`
**AND** `process.env.SHELL` is `undefined`
**AND** `process.env.COMSPEC` is `undefined`
**THEN** `resolveShell()` SHALL return `'cmd.exe'`

### Requirement: runCommand passes selected shell to execSync

**ID**: REQ-TEC-PLAT-3
**Priority**: MUST
**Description**: `runCommand()` SHALL pass `resolveShell()` as the `shell` option to `execSync()`. No behavioral change — it already calls `resolveShell()`. The change is entirely in `resolveShell()` (REQ-TEC-PLAT-2).

Signature unchanged: `runCommand(cmd: string, cwd: string, timeout?: number)`

#### Scenario: runCommand passes shell from resolveShell

**WHEN** `runCommand(cmd, cwd, timeout)` is called
**THEN** `execSync()` SHALL receive `shell` set to `resolveShell()` return value
**AND** all other options (`cwd`, `encoding`, `timeout`, `maxBuffer`, `stdio`) SHALL remain unchanged

#### Scenario: runCommand on Windows cmd.exe

**WHEN** `process.platform` is `'win32'`
**AND** `SHELL` and `COMSPEC` are undefined
**AND** `runCommand()` is called with a cmd.exe-compatible script (from `script.cmd`)
**THEN** `execSync()` SHALL receive `{ shell: 'cmd.exe' }`

### Requirement: Stryker mutation command uses platform-aware shell

**ID**: REQ-TEC-PLAT-4
**Priority**: MUST
**Description**: `executeStrykerMutation()` calls `runCommand()` which inherits the updated `resolveShell()`. No separate shell logic needed.

#### Scenario: Stryker inherits shell from runCommand

**WHEN** `executeStrykerMutation()` is called on Windows without Git Bash
**THEN** `npx stryker run` SHALL execute under cmd.exe

#### Scenario: Stryker Unix behavior unchanged

**WHEN** `executeStrykerMutation()` is called on Linux
**THEN** `runCommand()` SHALL use shell `undefined` (system default)

### Requirement: Empty command check accounts for platform-aware selection

**ID**: REQ-TEC-PLAT-5
**Priority**: MUST
**Description**: The empty command check in `executePlanEntry()` SHALL work correctly since it calls `buildTestCommand()` which selects the platform-appropriate script.

#### Scenario: empty script.cmd falls back to script.shell

**WHEN** `process.platform` is `'win32'`
**AND** `entry.script.cmd` is `''`
**AND** `entry.script.shell` is non-empty
**THEN** execution SHALL proceed using `script.shell`

#### Scenario: both scripts empty returns error

**WHEN** `entry.script.shell` is `''` AND `entry.script.cmd` is `''`
**THEN** `buildTestCommand()` SHALL return `''`
**AND** `executePlanEntry()` SHALL return `'Empty test command'` error

## Module Contract (Platform-Aware Test Runner)

### Module: lib/test-runner.ts (MODIFIED)

| Function | Before | After |
|----------|--------|-------|
| `resolveShell()` | Win: `process.env.SHELL \|\| 'bash'`; Unix: `undefined` | Win: `process.env.SHELL \|\| process.env.COMSPEC \|\| 'cmd.exe'`; Unix: `undefined` |
| `buildTestCommand(entry, root, files)` | Uses `entry.script` (string) | Selects `entry.script.cmd` (Win) or `entry.script.shell` (Unix) |
| `runCommand(cmd, cwd, timeout)` | Passes `resolveShell()` to `execSync` | Unchanged |
| `executePlanEntry(entry, root, options)` | Calls `buildTestCommand()` → `runCommand()` | Unchanged |
| `executeStrykerMutation(entry, dir, sources)` | Calls `runCommand(strykerCmd, ...)` | Unchanged |

### Shell Selection Flow

```
executePlanEntry()
  → buildTestCommand() reads entry.script (object)
    → process.platform === 'win32' ? entry.script.cmd : entry.script.shell
  → substitutePlaceholders(selectedScript, ...)
  → runCommand(cmd)
    → resolveShell()
      → win32 ? (SHELL || COMSPEC || 'cmd.exe') : undefined
    → execSync(cmd, { shell, ... })
```

### Backward Compatibility Matrix

| Platform | SHELL set | Before | After |
|----------|-----------|--------|-------|
| Linux    | N/A       | `shell: undefined` | Unchanged |
| macOS    | N/A       | `shell: undefined` | Unchanged |
| Windows  | Yes       | `shell: '...bash.exe'` | Unchanged |
| Windows  | No        | `shell: 'bash'` → FAILS | `shell: 'cmd.exe'` → WORKS |
