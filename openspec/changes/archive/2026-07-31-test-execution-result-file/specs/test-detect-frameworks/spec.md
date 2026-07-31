## MODIFIED Requirements

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

## ADDED Requirements

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

## Module Contract

### Module: commands/test-detect-frameworks.ts

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` |
| Behavior change | detect 产出带 report 相关占位符的 script；不烤死 reportDir；不再注入 suite cwd `coverage_cleanup` |
| Plan fields | `coverage_output` 相对 reportDir；`coverage_format` 含 bun→`lcov` |
