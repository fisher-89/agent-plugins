## ADDED Requirements

### Requirement: test-execution 命令描述去测试层级表述

`plugins/dev-team/bin/src/cli.ts` 中 `test-execution` 子命令的描述 SHALL 为 `Run all automated tests with coverage and generate execution report`，MUST NOT 含 `(unit + integration)` 层级表述。命令名、选项与行为不变。

#### Scenario: 命令描述已改写

- **WHEN** 读取 `cli.ts` 的 `test-execution` 命令注册
- **THEN** 描述为 `Run all automated tests with coverage and generate execution report`
- **AND** 不含 `unit + integration` 字样

#### Scenario: 命令行为不变

- **WHEN** `dev-team test-execution` 照常调用
- **THEN** 命令名、`--change` / `--project-root` / `--files` / `--framework` / `--skip-mutation` / `--force` 选项与执行路径均不变，仅描述文案改写

## Module Contract

### `plugins/dev-team/bin/src/cli.ts`

| 位置 | 变更 |
|------|------|
| `test-execution` 命令注册描述（原 19 行附近） | `Run all automated tests (unit + integration) with coverage and generate execution report` → `Run all automated tests with coverage and generate execution report` |
| 命令名 / 选项 / action | UNCHANGED |
