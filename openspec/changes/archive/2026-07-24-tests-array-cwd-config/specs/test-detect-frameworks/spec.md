## MODIFIED Requirements

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

---

## ADDED Requirements

### Requirement: Plan built from tests array suites

**ID**: REQ-TDF-SUITE-1
**Priority**: MUST
**Description**: `runTestDetectFrameworks` SHALL read `config.tests`（而非 `config.test.framework` / `config.test.overrides`）构建执行计划。每个 suite 产生一条 `plan` 条目（去重策略可保留：同一 absCwd + framework 可合并，但不得再使用 `deriveWorkingDirectory(glob)`）。

对每个 suite：
1. `absRoot = projectRoot / suite.root`
2. `absCwd = absRoot / (suite.cwd ?? ".")`
3. `plan.directory` = absCwd 相对于 `projectRoot` 的 POSIX 相对路径
4. `framework` / coverage 元数据 / mutation_framework 来自 `FRAMEWORK_REGISTRY`
5. `mutation_score` 来自该 suite 解析后的 `mutation.score`
6. include 匹配 glob = `suite.includes` 若存在，否则为框架 `default_glob`；匹配时相对 `suite.root`（实现可将 glob 拼为 projectRoot 相对形式 `root/includes`）

模块 SHALL 删除或停止调用 `deriveWorkingDirectory` 作为 cwd 来源。

#### Scenario: directory equals root when cwd is default

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin", framework: "vite-plus" }]`
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan` SHALL contain one entry with `directory: "plugins/dev-team/bin"`
**AND** `plan[0].framework` SHALL be `"vite-plus"`
**AND** `plan[0].mutation_score` SHALL equal the suite schema default mutation score

#### Scenario: directory resolves parent cwd

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin/src", cwd: "..", framework: "vite-plus" }]`
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan[0].directory` SHALL be `"plugins/dev-team/bin"`

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

When a suite declares `config` and the framework's `FrameworkConfig.config_flag` is a nonempty string, plan/script generation SHALL expand `{config_args}` to `` `${config_flag} ${relConfigPath}` `` where:
- `absConfig = absRoot / suite.config`
- `relConfigPath` = `path.relative(absCwd, absConfig)` 的 POSIX 形式（cwd=`"."` 时即为 `suite.config`）

When suite 无 `config`，`{config_args}` SHALL expand to an empty string（保持框架自动发现行为）。

When suite 声明了 `config` 但框架 `config_flag` 为 `null`/缺省，plan/script generation SHALL **fail with an explicit error**（MUST NOT 静默忽略，MUST NOT 向整段 script 盲 append）。

SHALL NOT append `config_flag` + path to the end of the entire `test_execution` string（链式脚本如 pytest/rust 会只作用于最后一段或产生错误参数）。

`script.shell` / `script.cmd` 仍先 `cd` 到 `plan.directory`（非 `"."` 时），再执行 cleanup 与已展开占位符的 test_execution。

#### Scenario: vite-plus suite with config expands {config_args}

**WHEN** suite is `{ root: "plugins/dev-team/bin", framework: "vite-plus", config: "vite.config.ts" }`
**AND** registry `config_flag` for vite-plus is `"--config"`
**AND** the vite-plus template contains `{config_args}`
**THEN** `plan[0].script.shell` SHALL contain `--config vite.config.ts`（或等价相对 cwd 路径）at the placeholder location
**AND** `plan[0].script.cmd` SHALL likewise contain the flag and path

#### Scenario: parent cwd rewrites config path relative to absCwd

**WHEN** suite is `{ root: "plugins/dev-team/bin/src", cwd: "..", framework: "vite-plus", config: "vite.config.ts" }`
**THEN** absCwd is `plugins/dev-team/bin` and expanded config path SHALL be `"vite.config.ts"`（relative to absCwd）
**AND** `script.shell` SHALL start with `cd plugins/dev-team/bin`（or equivalent）

#### Scenario: suite without config leaves {config_args} empty

**WHEN** suite is `{ root: "src", framework: "vitest" }` with no `config`
**THEN** `plan[0].script.shell` SHALL NOT contain `--config`

#### Scenario: config on framework without config_flag fails

**WHEN** suite is `{ root: ".", framework: "pytest", config: "pytest.ini" }`
**AND** registry `config_flag` for pytest is `null`
**THEN** plan/script generation SHALL fail with an explicit error mentioning unsupported `config` for that framework

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

---

## Module Contract

### Module: commands/test-detect-frameworks.ts

| Aspect | Detail |
|--------|--------|
| **Config source** | `config.tests: Suite[]` |
| **Removed** | `normalizeFrameworks(framework, overrides)` 旧签名；`deriveWorkingDirectory` 作为 cwd 来源 |
| **Plan fields** | `directory`（absCwd 相对项目根）, `framework`, coverage 元数据, `mutation_framework`, `mutation_score`, `script: { shell, cmd }` |
| **Script** | `cd` → cleanup → `test_execution` with `{config_args}` expanded（not whole-string append） |

### Function: buildPlanFromSuites（名称可调整）

| Property | Description |
|----------|-------------|
| **Input** | parsed `tests` suites + projectRoot |
| **Output** | `TestPlan[]` |
| **Side Effects** | None（纯函数；读 registry） |
