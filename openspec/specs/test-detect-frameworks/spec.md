## Purpose

This capability defines the MCP tool `test_detect_frameworks` which detects test frameworks for given source files and returns a structured plan with per-directory framework configuration. Framework config data (coverage commands, output paths, globs) is sourced from the internal `lib/test-framework.ts` module.

---

## Module Contract

### Module: commands/test-detect-frameworks.ts (Modified — Import Path)

The `test-detect-frameworks.ts` command module SHALL have its internal import updated to reference the extracted `lib/test-framework.ts` instead of `commands/test-get-framework-config.ts`. Its external API (input/output schema, MCP registration, function signature) is unchanged.

| Aspect | Before | After |
|--------|--------|-------|
| Import source | `'./test-get-framework-config'` | `'../lib/test-framework'` |
| Exported symbols consumed | `runTestGetFrameworkConfig`, `getDefaultGlobForFramework` | Same symbols, same names |
| Function signature | `runTestDetectFrameworks(options)` | Unchanged |
| Module location | `commands/test-detect-frameworks.ts` | Unchanged |
| MCP registration | `registerTestDetectFrameworksTool` in `mcp.ts` | Unchanged |

### Module: lib/test-framework.ts (New Internal Module)

A new internal module SHALL contain the `FRAMEWORK_REGISTRY` constant and its associated query functions, extracted from `commands/test-get-framework-config.ts`. This module is NOT exposed as an MCP tool — it is a shared library consumed by `test-detect-frameworks.ts` and potentially other internal consumers.

| Function | Input | Output | Side Effects |
|----------|-------|--------|-------------|
| `runTestGetFrameworkConfig({framework})` | `{framework: TestFrameworks}` | `FrameworkConfig` | None — pure lookup |
| `getDefaultGlobForFramework(framework)` | `framework: string` | `string` (glob pattern) | None — pure lookup |

### MCP Tool: test_detect_frameworks (Unchanged)

The MCP tool `test_detect_frameworks` remains registered, its input/output schema is unchanged, and all existing consumers continue to work identically. The change is purely internal: the framework config lookup now comes from `lib/test-framework.ts` instead of `commands/test-get-framework-config.ts`.

| Aspect | Detail |
|--------|--------|
| Tool name | `test_detect_frameworks` |
| Input | `{files?: string[], projectRoot?: string}` |
| Output | `TestDetectFrameworksResult` (`{ detected: DetectedFile[], frameworks: string[], plan: PlanEntry[] }`) |
| Plan entry fields | `directory`, `framework`, `coverage_format`, `coverage_output`, `mutation_framework`, `mutation_config`, `mutation_score`, `script` |
| Registration | `registerTestDetectFrameworksTool` in `mcp.ts` — unchanged |
| Consumers | `test-gen-generator.md`, `unit-test-executor.md` — unchanged |

---

## Requirements

### Requirement: Internal import path updated to lib/test-framework

**ID**: REQ-TDF-1
**Priority**: MUST
**Description**: `commands/test-detect-frameworks.ts` SHALL import `runTestGetFrameworkConfig` and `getDefaultGlobForFramework` from `../lib/test-framework` instead of `./test-get-framework-config`. This is the only behavioral change to this module.

#### Scenario: import statement references new path

**WHEN** `commands/test-detect-frameworks.ts` is inspected
**THEN** the import line for `runTestGetFrameworkConfig` and `getDefaultGlobForFramework` SHALL read `from '../lib/test-framework'`
**AND** SHALL NOT read `from './test-get-framework-config'`

#### Scenario: unit tests pass with updated import

**WHEN** unit tests for `test-detect-frameworks.ts` are executed
**THEN** all existing test cases SHALL pass
**AND** no import-related errors SHALL be reported

### Requirement: External MCP API unchanged

**ID**: REQ-TDF-2
**Priority**: MUST
**Description**: The MCP tool `test_detect_frameworks` SHALL remain registered with the same tool name, input schema, and output schema. The MCP registration code in `mcp.ts` for this tool SHALL NOT be modified.

#### Scenario: tool still registered in mcp.ts

**WHEN** `mcp.ts` is inspected
**THEN** `registerTestDetectFrameworksTool` SHALL be called in the server initialization sequence
**AND** the tool SHALL be registered under the name `'test_detect_frameworks'`

#### Scenario: tool input/output schema unchanged

**WHEN** `test_detect_frameworks` is invoked with `{files: ["src/example.test.ts"], projectRoot: "/tmp/test-project"}`
**THEN** the result SHALL contain the `detected`, `frameworks`, and `plan` arrays
**AND** each entry in `plan` SHALL contain the fields `directory`, `framework`, `coverage_format`, `coverage_output`, and `script`

### Requirement: plan[] output carries all framework config fields

**ID**: REQ-TDF-3
**Priority**: MUST
**Description**: The `plan[]` array in `test_detect_frameworks` output SHALL continue to include framework config fields (`coverage_format`, `coverage_output`) sourced from the internal `lib/test-framework.ts` FRAMEWORK_REGISTRY. The `coverage_cmd` and `coverage_artifacts` fields have been removed — coverage commands are embedded within platform scripts, and coverage parsing uses `coverage_output` only.

**Changes from previous version**:
- `coverage_output` SHALL 为相对 **reportDir** 的垂直约定文件名（或可由 `coverage_format` 推导的等价约定），不再是 suite cwd 下的 `coverage/...` 旧路径
- 解析权威源为 execute 期 `reports/test/<planId>/`；detect 阶段可不展开绝对 report 路径

#### Scenario: plan entry coverage fields match framework registry

**WHEN** `test_detect_frameworks` is called on a project configured with framework `"vitest"`
**THEN** `plan[0].coverage_format` SHALL be `"istanbul"`
**AND** `plan[0].coverage_output` SHALL 为相对 reportDir 的约定名（如 `"coverage-summary.json"`）
**AND** `plan[0]` SHALL NOT contain `coverage_artifacts`
**AND** `plan[0].coverage_output` SHALL NOT 为 suite cwd 旧路径 `"coverage/coverage-summary.json"`

#### Scenario: node-test framework plan entry carries simplified fields

**WHEN** `test_detect_frameworks` is called on a project configured with framework `"node-test"`
**THEN** `plan[0].coverage_output` SHALL 指向垂直约定的结果/覆盖率文件名（可与测试结果文件相同）
**AND** `plan[0]` SHALL NOT contain `coverage_artifacts`

#### Scenario: bun plan entry uses lcov coverage format

**WHEN** `test_detect_frameworks` is called on a project configured with framework `"bun"`
**THEN** `plan[0].coverage_format` SHALL be `"lcov"`
**AND** `plan[0].coverage_output` SHALL 为相对 reportDir 的 `"lcov.info"`（或等价垂直约定名）

### Requirement: FRAMEWORK_REGISTRY extracted to shared lib module

**ID**: REQ-TDF-4
**Priority**: MUST
**Description**: The `FRAMEWORK_REGISTRY` constant, `runTestGetFrameworkConfig()` function, and `getDefaultGlobForFramework()` function SHALL be located in `lib/test-framework.ts` as a shared internal module. The `test-detect-frameworks.ts` SHALL be the sole consumer via internal import (not MCP). The `test-get-framework-config.ts` source file SHALL be deleted.

#### Scenario: lib/test-framework.ts exists and exports functions

**WHEN** `lib/test-framework.ts` is inspected
**THEN** it SHALL export `runTestGetFrameworkConfig`
**AND** it SHALL export `getDefaultGlobForFramework`
**AND** it SHALL contain the `FRAMEWORK_REGISTRY` constant with all 8 framework entries

#### Scenario: commands/test-get-framework-config.ts is deleted

**WHEN** the filesystem is inspected
**THEN** `commands/test-get-framework-config.ts` SHALL NOT exist

### Requirement: Excluded files filtered from framework detection

**ID**: REQ-TDF-5
**Priority**: MUST
**Description**: The `detectFrameworksForFiles` function in `commands/test-detect-frameworks.ts` SHALL filter out source files that are outside every suite scope or matched by suite `excludes` before building the `detected` array. After determining the relative file path, the function SHALL call `isFileExcluded(relativePath, config)` from `lib/test-exclude.ts`（或等价的 suite-scope 判定）。If the function returns `true`, the file SHALL be skipped entirely — it SHALL NOT appear in `detected` and SHALL NOT be treated as "unknown".

The module SHALL import `isFileExcluded` from `../lib/test-exclude` at the top of the file.

#### Scenario: excluded file is not in detected array

**WHEN** the project config has `tests: [{ root: "src", framework: "vitest", excludes: ["generated/**"] }]`
**AND** `runTestDetectFrameworks` is called with `files: ["src/app.ts", "src/generated/api.ts"]`
**THEN** `result.detected` SHALL contain an entry for `"src/app.ts"`（若其落入该 suite includes）
**AND** `result.detected` SHALL NOT contain any entry where `file` includes `"generated/api.ts"`

#### Scenario: excluded file is not treated as "unknown"

**WHEN** the project config has `tests: [{ root: "src", framework: "vitest", excludes: ["generated/**"] }]`
**AND** `runTestDetectFrameworks` is called with `files: ["src/generated/api.ts", "tools/unknown.py"]`
**THEN** `result.detected` SHALL NOT contain any entry for `"src/generated/api.ts"`
**AND** 若 `"tools/unknown.py"` 出现在 `detected` 中，其 framework SHALL 为 `"unknown"`（因其不在任何 suite scope）

#### Scenario: suite-level exclude also filtered

**WHEN** the project config has `tests: [{ root: "plugins/dev-team/bin", framework: "vite-plus", excludes: ["vendor/**"] }]`
**AND** `runTestDetectFrameworks` is called with `files: ["plugins/dev-team/bin/src/app.ts", "plugins/dev-team/bin/vendor/lib.ts"]`
**THEN** `result.detected` SHALL contain an entry for `"plugins/dev-team/bin/src/app.ts"`
**AND** `result.detected` SHALL NOT contain an entry for `"plugins/dev-team/bin/vendor/lib.ts"`

#### Scenario: no excludes configured — files in suite scope detected

**WHEN** the project config has `tests: [{ root: "src", framework: "vitest" }]` with no `excludes`
**AND** `runTestDetectFrameworks` is called with files under `src/` that match the suite includes（缺省为框架 `default_glob`）
**THEN** matching files SHALL appear in `result.detected` with framework `"vitest"`

#### Scenario: auto-scan mode respects suite excludes

**WHEN** the project config has `tests: [{ root: "src", framework: "vitest", excludes: ["generated/**"] }]`
**AND** `runTestDetectFrameworks` is called without a `files` argument (auto-scan mode)
**AND** the project filesystem contains `src/app.test.ts`, `src/generated/api.ts`
**THEN** `result.detected` SHALL NOT include `"src/generated/api.ts"`

### Requirement: Plan built from tests array suites

**ID**: REQ-TDF-SUITE-1
**Priority**: MUST
**Description**: `runTestDetectFrameworks` SHALL read `config.tests`（而非 `config.test.framework` / `config.test.overrides`）构建执行计划。每个 suite 产生一条 `plan` 条目（去重策略可保留：同一 absCwd + framework 可合并，但不得再使用 `deriveWorkingDirectory(glob)`）。

对每个 suite：
1. `absRoot = projectRoot / suite.root`
2. `absCwd = absRoot / (suite.cwd ?? ".")`
3. `plan.directory` = absCwd 相对于 `projectRoot` 的 POSIX 相对路径
4. `plan.scope` = absRoot 相对于 absCwd 的 POSIX 相对路径（二者相同时为 `"."`）
5. `framework` / coverage 元数据 / mutation_framework 来自 `FRAMEWORK_REGISTRY`
6. `mutation_score` 来自该 suite 解析后的 `mutation.score`
7. include 匹配 glob = `suite.includes` 若存在，否则为框架 `default_glob`；匹配时相对 `suite.root`（实现可将 glob 拼为 projectRoot 相对形式 `root/includes`）

模块 SHALL 删除或停止调用 `deriveWorkingDirectory` 作为 cwd 来源。

执行时：`{files}` 为空 SHALL 使用 `plan.scope` 作为 CLI 路径过滤（`scope === "."` 时展开为空，因 cwd 已等于 absRoot）；显式 `files` 列表优先于 `scope`。`{directory}`（go）SHALL 展开为 `./...` 或 `./<scope>/...`。rust 的 `cargo test` 不支持任意子目录 CLI 裁剪，SHALL 保持 crate 级行为（`scope` 不改变其 `test_execution`）。

#### Scenario: directory equals root when cwd is default

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin", framework: "vite-plus" }]`
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan` SHALL contain one entry with `directory: "plugins/dev-team/bin"`
**AND** `plan[0].scope` SHALL be `"."`
**AND** `plan[0].framework` SHALL be `"vite-plus"`
**AND** `plan[0].mutation_score` SHALL equal the suite schema default mutation score

#### Scenario: directory resolves parent cwd

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin/src", cwd: "..", framework: "vite-plus" }]`
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan[0].directory` SHALL be `"plugins/dev-team/bin"`
**AND** `plan[0].scope` SHALL be `"src"`

#### Scenario: empty files falls back to scope under parent cwd

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin/src", cwd: "..", framework: "vite-plus" }]`
**AND** the plan entry is executed with an empty `files` list
**THEN** the substituted command SHALL include the path filter `src` in place of `{files}`
**AND** SHALL NOT discover tests outside that suite root solely due to a wider absCwd

#### Scenario: empty tests yields empty plan

**WHEN** config has `tests: []` or omits tests（parse 后为空数组）
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan` SHALL be `[]`

#### Scenario: multiple suites produce multiple plan entries

**WHEN** config has two suites with different roots/frameworks
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan.length` SHALL be 2
**AND** each entry's `directory` SHALL reflect that suite's absCwd

### Requirement: Script generation injects optional framework config via template placeholder

**ID**: REQ-TDF-SUITE-2
**Priority**: MUST
**Description**: Framework `test_execution` templates SHALL contain the placeholder `{config_args}` in each command segment that must receive the framework config flag（单命令框架一段即可；链式框架若将来支持则每段都含占位）。

When a suite declares `config` and the framework's `FrameworkConfig.config_flag` is a nonempty string, plan/script generation at **execute**（经 `preparePlanArtifacts`）SHALL expand `{config_args}` to `` `${config_flag} ${relConfigPath}` `` where:
- `absConfig = absRoot / suite.config`
- `relConfigPath` = `path.relative(absCwd, absConfig)` 的 POSIX 形式（cwd=`"."` 时即为 `suite.config`）

When suite 无 `config`，`{config_args}` SHALL expand to an empty string（保持框架自动发现行为）。

When suite 声明了 `config` 但框架 `config_flag` 为 `null`/缺省，plan/script generation SHALL **fail with an explicit error**（MUST NOT 静默忽略，MUST NOT 向整段 script 盲 append）。

SHALL NOT append `config_flag` + path to the end of the entire `test_execution` string（链式脚本如 pytest/rust 会只作用于最后一段或产生错误参数）。

`script.shell` / `script.cmd` 仍先 `cd` 到 `plan.directory`（非 `"."` 时），再执行已展开占位符的 test_execution。

**Changes from previous version**:
- detect SHALL 保留 `{config_args}` / `{results_file}` / `{coverage_file}` / `{report_dir}` 等占位符，MUST NOT 在 detect 烤死 reportDir 相关路径
- SHALL NOT 再向 script 注入 suite cwd 内 `coverage_cleanup`（`rm -rf coverage` 等）；跑前清空改为 execute 期清空该 plan 的 `reportDir`

#### Scenario: vite-plus suite with config expands {config_args}

**WHEN** suite is `{ root: "plugins/dev-team/bin", framework: "vite-plus", config: "vite.config.ts" }`
**AND** registry `config_flag` for vite-plus is `"--config"`
**AND** the vite-plus template contains `{config_args}`
**AND** execute 期完成占位符展开
**THEN** 最终命令 SHALL contain `--config vite.config.ts`（或等价相对 cwd 路径）at the placeholder location

#### Scenario: parent cwd rewrites config path relative to absCwd

**WHEN** suite is `{ root: "plugins/dev-team/bin/src", cwd: "..", framework: "vite-plus", config: "vite.config.ts" }`
**THEN** absCwd is `plugins/dev-team/bin` and expanded config path SHALL be `"vite.config.ts"`（relative to absCwd）
**AND** `script.shell` SHALL start with `cd plugins/dev-team/bin`（or equivalent）

#### Scenario: suite without config leaves {config_args} empty

**WHEN** suite is `{ root: "src", framework: "vitest" }` with no `config`
**AND** execute 期展开占位符
**THEN** 最终命令 SHALL NOT contain `--config`

#### Scenario: config on framework without config_flag fails

**WHEN** suite is `{ root: ".", framework: "pytest", config: "pytest.ini" }`
**AND** registry `config_flag` for pytest is `null`
**THEN** plan/script generation SHALL fail with an explicit error mentioning unsupported `config` for that framework

#### Scenario: detect keeps report placeholders unexpanded

**WHEN** `test_detect_frameworks` 生成 plan script
**AND** 模板含 `{results_file}` / `{report_dir}`（或等价）
**THEN** 返回的 `plan[].script.shell` / `script.cmd` SHALL 仍包含这些占位符（未替换为具体 `reports/test/...` 路径）

#### Scenario: scripts do not embed suite cwd coverage_cleanup

**WHEN** 检查 detect 生成的 `script.shell` / `script.cmd`
**THEN** SHALL NOT 包含对 suite cwd 下 `coverage` / `.nyc_output` 等旧产物路径的 cleanup 命令
**AND** 产物清理职责 SHALL 由 execute 期清空 `reportDir`（及 tempPaths）承担

### Requirement: Script templates include file-channel placeholders

**ID**: REQ-TDF-TEF-1
**Priority**: MUST
**Description**: 各框架 `test_execution` 模板 SHALL 按需包含文件通道占位符：`{results_file}`、`{coverage_file}`、`{report_dir}`、`{config_args}`（及 go 等需要的 `{coverprofile_file}` 等）。原生 outputFile 族模板 SHALL 将输出旗标指向这些占位符；无原生结果文件的测试段由 execute 条件追加 `>`，模板本身可不硬编码重定向。

#### Scenario: jest template uses outputFile placeholder

**WHEN** 读取 jest 的 `test_execution` 模板
**THEN** 模板 SHALL 包含将 JSON 结果写到文件的旗标，且路径使用 `{results_file}`（或经 prepare 替换的等价占位）
**AND** 模板 SHALL 将 coverage 目录/文件指向 `{report_dir}` / `{coverage_file}`

#### Scenario: execute substitutes placeholders from preparePlanArtifacts

**WHEN** execute 持有 `preparePlanArtifacts` 返回的 placeholders
**THEN** SHALL 替换 script 中的 `{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` 等
**AND** 替换后的路径 SHALL 落在该 plan 的 `reportDir` 内（或指向其内文件）

### Requirement: File-to-framework matching uses suite scope

**ID**: REQ-TDF-SUITE-3
**Priority**: MUST
**Description**: 对显式 `files` 列表或 auto-scan，文件归属框架 SHALL 由 suite scope 决定：

`inScope(file, suite) = under(suite.root) ∧ match(includesRelRoot) ∧ ¬match(excludesRelRoot)`

其中 `includesRelRoot` 缺省为框架 `default_glob`。第一个匹配的 suite 决定 `detected[].framework`（与今日 overrides 顺序语义一致：数组顺序优先）。

#### Scenario: file under root matching includes gets suite framework

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin", framework: "vite-plus", includes: ["src/**/*.ts"] }]`
**AND** `runTestDetectFrameworks` is called with `files: ["plugins/dev-team/bin/src/foo.ts"]`
**THEN** `detected` SHALL include `{ file: "plugins/dev-team/bin/src/foo.ts", framework: "vite-plus" }`

#### Scenario: file outside all suite roots is unknown when files provided

**WHEN** config has `tests: [{ root: "src", framework: "vitest" }]`
**AND** `runTestDetectFrameworks` is called with `files: ["tools/cli.ts"]`
**THEN** `detected` SHALL include `{ file: "tools/cli.ts", framework: "unknown" }`

#### Scenario: default includes uses framework default_glob under root

**WHEN** config has `tests: [{ root: "src", framework: "vitest" }]` without `includes`
**AND** auto-scan finds `src/foo.test.ts` and `src/readme.md`
**THEN** `detected` SHALL include `src/foo.test.ts` with `"vitest"`
**AND** SHALL NOT include `src/readme.md` solely due to default_glob mismatch

### Requirement: Error and MCP copy reference tests configuration

**ID**: REQ-TDF-SUITE-4
**Priority**: SHOULD
**Description**: 用户可见提示（`test-execution` 无配置日志、`mcp.ts` 工具描述等）SHALL 引导配置 `tests`（或 `tests[].framework`），MUST NOT 再要求配置已移除的 `test.framework` / `test.overrides`。

#### Scenario: no suite config message mentions tests

**WHEN** `tests` 为空导致无 plan
**THEN** CLI/MCP 提示文案 SHALL mention `tests` in `openspec/config.json`

### Module Contract (tests-array suite model)

### Module: commands/test-detect-frameworks.ts

| Aspect | Detail |
|--------|--------|
| **Config source** | `config.tests: Suite[]` |
| **Removed** | `normalizeFrameworks(framework, overrides)` 旧签名；`deriveWorkingDirectory` 作为 cwd 来源 |
| **Plan fields** | `directory`（absCwd 相对项目根）, `scope`（absRoot 相对 absCwd）, `framework`, coverage 元数据（`coverage_output` 相对 reportDir；`coverage_format` 含 bun→`lcov`）, `mutation_framework`, `mutation_score`, `script: { shell, cmd }` |
| **Script** | `cd` → `test_execution` with report/file-channel placeholders kept unexpanded at detect；`{config_args}` / report paths expanded at execute（not whole-string append）；不再注入 suite cwd `coverage_cleanup` |
| **Behavior change** | detect 产出带 report 相关占位符的 script；不烤死 reportDir；不再注入 suite cwd `coverage_cleanup` |

### Function: buildPlanFromSuites（名称可调整）

| Property | Description |
|----------|-------------|
| **Input** | parsed `tests` suites + projectRoot |
| **Output** | `TestPlan[]` |
| **Side Effects** | None（纯函数；读 registry） |

---

## ADDED Requirements (Platform-Aware Framework Registry)

### Requirement: FRAMEWORK_REGISTRY entries use shell/cmd nested structure

**ID**: REQ-TDF-PLAT-1
**Priority**: MUST
**Description**: `FrameworkConfig` interface in `lib/test-framework.ts` SHALL be restructured into a two-level nested form. Platform-specific fields (`test_execution`, `coverage_cleanup`) SHALL be grouped under `shell` and `cmd` sub-objects. Platform-independent fields (`framework`, `coverage_format`, `coverage_output`, `default_glob`, `mutation_framework`) SHALL remain at the top level.

```typescript
interface FrameworkConfig {
  framework: TestFrameworks;
  shell: {
    test_execution: string;      // Shell (bash/POSIX) test command template
    coverage_cleanup: string[];  // Shell coverage cleanup paths (rm -rf)
  };
  cmd: {
    test_execution: string;      // Windows cmd.exe test command template
    coverage_cleanup: string[];  // Windows cmd.exe coverage cleanup paths (rmdir /s/q, del /q/f)
  };
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  default_glob: string;
  mutation_framework: string | null;
}
```

**Field mapping (old → new)**:
- `test_cmd` → `shell.test_execution`
- `coverage_cleanup` → `shell.coverage_cleanup`
- (new) → `cmd.test_execution`
- (new) → `cmd.coverage_cleanup`

#### Scenario: each framework entry has shell and cmd sub-objects

**WHEN** `FRAMEWORK_REGISTRY` is inspected
**THEN** each framework entry SHALL have `shell` and `cmd` sub-objects
**AND** `shell` SHALL contain `test_execution: string` and `coverage_cleanup: string[]`
**AND** `cmd` SHALL contain `test_execution: string` and `coverage_cleanup: string[]`
**AND** the number of framework entries SHALL remain 8

#### Scenario: shell sub-object preserves existing semantics

**WHEN** inspecting `FRAMEWORK_REGISTRY['jest'].shell`
**THEN** `shell.test_execution` SHALL equal the previous `test_cmd` value
**AND** `shell.coverage_cleanup` SHALL equal the previous `coverage_cleanup` value
**AND** the same SHALL be true for all 8 framework entries

#### Scenario: simple framework commands have identical test_execution across platforms

**WHEN** inspecting `FRAMEWORK_REGISTRY['jest']`
**THEN** `shell.test_execution` SHALL equal `cmd.test_execution`
**AND** the same SHALL be true for `vitest`, `vite-plus`, `bun`, `node-test`, and `go`

#### Scenario: chained framework commands have cmd.exe syntax in cmd.test_execution

**WHEN** inspecting `FRAMEWORK_REGISTRY['rust'].cmd.test_execution`
**THEN** it SHALL NOT contain `; _X=$?;`
**AND** it SHALL contain `%errorlevel%` or `errorlevel` for exit code handling
**AND** the same SHALL be true for `pytest`

#### Scenario: cmd.coverage_cleanup mirrors shell.coverage_cleanup paths

**WHEN** inspecting `FRAMEWORK_REGISTRY['jest'].cmd.coverage_cleanup`
**THEN** it SHALL have the same number of entries as `shell.coverage_cleanup`
**AND** each entry SHALL be a valid path or glob
**AND** `getFrameworkConfig('jest').cmd.coverage_cleanup` SHALL return a non-empty array

#### Scenario: getFrameworkConfig returns new nested structure

**WHEN** `getFrameworkConfig('vitest')` is called
**THEN** the returned object SHALL have `shell` and `cmd` sub-objects
**AND** `shell.test_execution` and `cmd.test_execution` SHALL be populated
**AND** `shell.coverage_cleanup` and `cmd.coverage_cleanup` SHALL be populated
**AND** platform-independent fields SHALL be at the top level

### Requirement: TestPlan.script restructured as { shell, cmd } object

**ID**: REQ-TDF-PLAT-2
**Priority**: MUST
**Description**: The `testPlanSchema` in `schemas/test-detect-frameworks.schema.ts` SHALL change the `script` field from `z.string()` to `z.object({ shell: z.string(), cmd: z.string() })`.

- `script.shell` — execution script for bash/POSIX shell (generated by `generateShellScript()` from `frameworkConfig.shell`)
- `script.cmd` — execution script for Windows cmd.exe (generated by `generateCmdScript()` from `frameworkConfig.cmd`)

#### Scenario: script field is an object with shell and cmd

**WHEN** `testPlanSchema` is inspected
**THEN** `script` SHALL be of type `z.ZodObject`
**AND** `script` SHALL contain `shell: z.ZodString` and `cmd: z.ZodString`

#### Scenario: TestPlan type includes script.shell and script.cmd

**WHEN** a `TestPlan` object is created with TypeScript
**THEN** `script.shell: string` SHALL be a required property
**AND** `script.cmd: string` SHALL be a required property
**AND** `script` SHALL be a plain object (not a string)

### Requirement: generateCmdScript produces Windows-compatible cmd.exe script

**ID**: REQ-TDF-PLAT-3
**Priority**: MUST
**Description**: A new function `generateCmdScript(directory: string, frameworkConfig: FrameworkConfig): string` SHALL be added to `commands/test-detect-frameworks.ts`. It SHALL read from `frameworkConfig.cmd` and produce a Windows cmd.exe script:

1. If `directory` is not `'.'`, prepend `cd /d <directory>`
2. For each item in `frameworkConfig.cmd.coverage_cleanup`, append `if exist <item> rmdir /s/q <item>` (directories) or `if exist <item> del /q /f <item>` (files)
3. Append `frameworkConfig.cmd.test_execution`
4. Join with `\r\n` + trailing `\r\n`

Input validation:
- `frameworkConfig` MUST NOT be null/undefined
- `directory` MUST be a string
- `frameworkConfig.cmd.test_execution` MUST be a string
- `frameworkConfig.cmd.coverage_cleanup` MUST be an array

The existing `generateScript()` SHALL be renamed to `generateShellScript()`.

#### Scenario: generateCmdScript validates inputs

**WHEN** `generateCmdScript('.', null)` is called
**THEN** it SHALL throw `TypeError`

**WHEN** `generateCmdScript(null as unknown as string, validConfig)` is called
**THEN** it SHALL throw `TypeError`

**WHEN** `generateCmdScript('.', configWithMissingCmdTestExecution)` is called
**THEN** it SHALL throw `TypeError`

#### Scenario: generateCmdScript produces cmd.exe script for non-root directory

**WHEN** `generateCmdScript('src/lib', frameworkConfig)` is called
**AND** `frameworkConfig.cmd.test_execution` is `'npx jest {files}'`
**AND** `frameworkConfig.cmd.coverage_cleanup` is `['coverage', '.nyc_output']`
**THEN** the result SHALL start with `cd /d src/lib\r\n`
**AND** the result SHALL contain `if exist coverage rmdir /s/q coverage\r\n`
**AND** the result SHALL contain `if exist .nyc_output rmdir /s/q .nyc_output\r\n`
**AND** the result SHALL end with `npx jest {files}\r\n`

#### Scenario: generateCmdScript for root directory skips cd

**WHEN** `generateCmdScript('.', frameworkConfig)` is called
**THEN** the result SHALL NOT contain `cd`
**AND** the result SHALL directly start with cleanup commands and cmd.test_execution

### Requirement: generateShellScript reads from frameworkConfig.shell

**ID**: REQ-TDF-PLAT-4
**Priority**: MUST
**Description**: The renamed `generateShellScript()` function SHALL read from `frameworkConfig.shell` (instead of old flat `frameworkConfig.test_cmd` / `frameworkConfig.coverage_cleanup`). Output SHALL be identical to the previous `generateScript()`.

#### Scenario: generateShellScript output matches old generateScript

**WHEN** `generateShellScript('.', jestConfig)` is called
**THEN** the output SHALL equal the previous `generateScript('.', jestConfig)` for the same inputs
**AND** the same SHALL be true for all 8 frameworks and both `directory === '.'` / `directory !== '.'` cases

### Requirement: buildPlanFromMappings produces script object

**ID**: REQ-TDF-PLAT-5
**Priority**: MUST
**Description**: `buildPlanFromMappings()` SHALL set `script` to `{ shell: generateShellScript(...), cmd: generateCmdScript(...) }` on each plan entry.

#### Scenario: plan entry script is an object

**WHEN** `runTestDetectFrameworks({})` is called
**THEN** each entry in `plan` SHALL have `script` of type object
**AND** `script.shell` SHALL be a non-empty string
**AND** `script.cmd` SHALL be a non-empty string
**AND** `script.cmd` SHALL NOT equal `script.shell` for frameworks with different command syntax (rust, pytest)

#### Scenario: script.cmd uses cmd.exe syntax

**WHEN** inspecting `plan[0].script.cmd` for framework `"jest"` with `directory === "."`
**THEN** `script.cmd` SHALL contain `rmdir /s/q` (not `rm -rf`)

## Module Contract (Platform-Aware Framework Config)

### Interface: FrameworkConfig (MODIFIED)

```typescript
export interface FrameworkConfig {
  framework: TestFrameworks;
  shell: {
    test_execution: string;
    coverage_cleanup: string[];
  };
  cmd: {
    test_execution: string;
    coverage_cleanup: string[];
  };
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  default_glob: string;
  mutation_framework: string | null;
}
```

### Schema: testPlanSchema (MODIFIED)

```typescript
const testPlanSchema = z.object({
  // ... all existing fields unchanged ...
  script: z.object({
    shell: z.string().describe('Shell (bash/POSIX) execution script'),
    cmd: z.string().describe('Windows cmd.exe execution script'),
  }),
});
```

### Module: commands/test-detect-frameworks.ts (MODIFIED)

| Function | Before | After |
|----------|--------|-------|
| `generateScript(d, cfg)` | Reads `cfg.test_cmd`, `cfg.coverage_cleanup` | Renamed to `generateShellScript()` — reads `cfg.shell` |
| `generateShellScript(d, cfg)` | (did not exist) | Former `generateScript()` — reads `cfg.shell` |
| `generateCmdScript(d, cfg)` | (did not exist) | New — reads `cfg.cmd`, produces cmd.exe script |
| `buildPlanFromMappings(maps, root)` | Sets `script: string` | Sets `script: { shell, cmd }` via both generators |
