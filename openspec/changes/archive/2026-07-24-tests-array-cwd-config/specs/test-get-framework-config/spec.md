## ADDED Requirements

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

## Module Contract

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
