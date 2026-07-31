## ADDED Requirements

### Requirement: Registry aligns coverage paths to reportDir file channel

**ID**: REQ-TF-TEF-1
**Priority**: MUST
**Description**: `FRAMEWORK_REGISTRY` 中各框架的 `coverage_output` SHALL 改为相对 **reportDir** 的垂直约定文件名（不再使用 suite cwd 下 `coverage/...` 旧路径）。`coverage_format` 枚举 SHALL 支持 `'lcov'`（供 bun）。`shell` / `cmd` 下的 `coverage_cleanup` 列表语义 SHALL 废弃（产物不再落 suite cwd；execute 清空 reportDir）。

`test_execution` 模板 SHALL 含文件通道占位符（`{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` 等），并按框架选择原生文件输出旗标或交由 execute 条件 `>`。

#### Scenario: vitest coverage_output is reportDir-relative

**WHEN** 调用 `getFrameworkConfig({ framework: "vitest" })`
**THEN** `coverage_output` SHALL 为相对 reportDir 的约定名（如 `"coverage-summary.json"`）
**AND** SHALL NOT 为 `"coverage/coverage-summary.json"`

#### Scenario: jest template includes outputFile and coverage directory placeholders

**WHEN** 读取 jest `shell.test_execution` / `cmd.test_execution` 模板字符串
**THEN** 模板 SHALL 包含原生 JSON 文件输出旗标与 coverage 目录/reporter 旗标
**AND** 路径位置 SHALL 使用 `{results_file}` / `{report_dir}`（或等价占位符）

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

## MODIFIED Requirements

### Requirement: FrameworkConfig 增加可选 config_flag 与 {config_args} 占位

**Description**: `FrameworkConfig` SHALL 继续提供可选 `config_flag`。`test_execution` 模板 SHALL 使用 `{config_args}` 占位。此外本变更要求模板同时支持报告产物占位符（见 REQ-TF-TEF-1）。bun 的 `config_flag` SHALL 不再固定为 `null`（或由 prepare 特化等价支持显式 config），以便临时 bunfig。

#### Scenario: frameworks with config_flag expand via placeholder

**WHEN** framework 为 `jest` / `vitest` / `vite-plus`
**AND** suite 提供 `config`
**THEN** execute 展开后命令 SHALL 在 `{config_args}` 位置包含 `config_flag` 与相对 cwd 的 config 路径

#### Scenario: bun supports config injection for plan artifacts

**WHEN** framework 为 `bun`
**AND** prepare 需要临时 bunfig
**THEN** registry/prepare 路径 SHALL 允许将 `--config <temp>` 注入命令
**AND** 用户长期 bunfig SHALL 仅被只读 overlay，不被修改

## Module Contract

### Module: lib/test-framework.ts (Internal)

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/lib/test-framework.ts` |
| coverage_format | 增加 `'lcov'`；bun 使用 `lcov` |
| coverage_output | 相对 reportDir 的垂直文件名 |
| templates | 含 `{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` |
| coverage_cleanup | 语义废弃（不再驱动 suite cwd cleanup） |
| bun config_flag | 支持显式 config / prepare 特化 |

### Interface: FrameworkConfig（增量）

| Field | Change |
|-------|--------|
| `coverage_format` | 联合类型增加 `'lcov'` |
| `coverage_output` | 相对 reportDir |
| `shell.coverage_cleanup` / `cmd.coverage_cleanup` | 不再用于 suite cwd 清理（可删或忽略） |
| `config_flag` | bun 不再恒为 `null`（或 prepare 特化） |
