# embedded-cli Specification

## Purpose
TBD - created by archiving change embed-openspec-remove-likec4. Update Purpose after archive.

## Requirements

### Requirement: CLI registers eval-check subcommand
The TypeScript CLI SHALL register an `eval-check` subcommand via `registerEvalCheckCommand(cli)` in `bin/src/index.ts`.
The subcommand SHALL be registered using the `cac` declarative API pattern consistent with `eval-log`:
- `.command("eval-check", "...")` for the subcommand definition
- `.option("--change <name>", "...")` for the change name (required)
- `.option("--phase <phase>", "...")` for the phase identifier (required)
- `.option("--json", "...")` for structured JSON output (optional)

#### Scenario: eval-check appears in help output
- **WHEN** `dev-team --help` is executed
- **THEN** the output SHALL include "eval-check" in the list of available commands

#### Scenario: eval-check accepts required options
- **WHEN** `dev-team eval-check --change my-change --phase 02-dev-design` is invoked
- **THEN** the command SHALL parse --change and --phase and pass them to the action handler

#### Scenario: eval-check --json flag is optional
- **WHEN** `dev-team eval-check --change my-change --phase 02-dev-design --json` is invoked
- **THEN** the command SHALL pass the --json flag to the action handler for structured output

### Requirement: 打包产物 dev-team-cli.cjs 作为独立 CLI 入口

`plugins/dev-team/bin/vite.config.ts` 的 `pack` 配置 SHALL 增加第二个打包入口，生成 `plugins/dev-team/bin/dev-team-cli.cjs`。

- 入口文件：`src/cli.ts`
- 输出格式：`cjs`（与 `dev-team-mcp.cjs` 一致）
- 平台：`node`
- 打包后文件 SHALL 位于 `plugins/dev-team/bin/dev-team-cli.cjs`

`dev-team-mcp.cjs` 的现有打包配置（入口 `src/mcp.ts`）SHALL NOT 被修改或移除。

#### Scenario: vp pack 生成 dev-team-cli.cjs

- **WHEN** 在 `plugins/dev-team/bin/` 目录执行 `pnpm run build`
- **THEN** 生成 `dev-team-cli.cjs` 文件
- **AND** `dev-team-mcp.cjs` 仍正常生成

#### Scenario: dev-team-cli.cjs 可独立执行

- **WHEN** 执行 `node plugins/dev-team/bin/dev-team-cli.cjs --help`
- **THEN** 输出包含可用子命令列表
- **AND** 不启动 MCP server

### Requirement: CLI 注册 run_static_analysis 子命令

`src/cli.ts` SHALL 使用 `cac` 声明式 API 注册 `run_static_analysis` 子命令，实现位于 `src/commands/run-static-analysis.ts` 的 `runStaticAnalysis()` 函数。

子命令行为：

1. 确定项目根目录：优先 `process.env.CLAUDE_PROJECT_DIR`，否则 `process.cwd()`
2. 调用 `ensureConfigFile(projectRoot)` 确保 config 文件存在
3. 调用 `getValue(config, "static_analysis")` 读取配置
4. 若 `exists` 为 `false` 或 `value` 为空字符串：exit `0`（放行，不执行任何命令）
5. 若 `value` 为非空字符串：在项目根目录执行该命令（0 参数，不传 change name 或其他参数）
6. 命令 exit code 为 `0`：CLI exit `0`
7. 命令 exit code 非 `0`：CLI exit 相同非零 code，并将 stderr/stdout 输出到 CLI 的 stderr

配置读取 SHALL 复用 `lib/config.ts` 中的 `ensureConfigFile()` 和 `getValue()`，SHALL NOT 重复实现 JSON 解析逻辑。

#### Scenario: 未配置 static_analysis 时 CLI 放行

- **WHEN** `openspec/config.json` 不存在或未包含 `static_analysis` 字段
- **THEN** `node dev-team-cli.cjs run_static_analysis` exit code 为 `0`
- **AND** 不执行任何外部命令

#### Scenario: 已配置 static_analysis 时 CLI 执行命令

- **WHEN** `openspec/config.json` 包含 `"static_analysis": "pnpm run -C ./plugins/dev-team/bin check"`
- **THEN** CLI 在项目根目录执行该命令，不传入额外参数
- **AND** 命令 exit code 为 `0` 时 CLI exit `0`
- **AND** 命令 exit code 非 `0` 时 CLI exit 相同非零 code 并输出错误信息

#### Scenario: static_analysis 为空字符串时 CLI 放行

- **WHEN** `openspec/config.json` 包含 `"static_analysis": ""`
- **THEN** CLI exit code 为 `0`
- **AND** 不执行任何外部命令

#### Scenario: run_static_analysis 出现在 help 输出中

- **WHEN** 执行 `node dev-team-cli.cjs --help`
- **THEN** 输出包含 `run_static_analysis` 子命令

### Requirement: run_static_analysis 命令具备单元测试

`src/commands/run-static-analysis.test.ts` SHALL 覆盖以下场景：

- 未配置 `static_analysis` 时 exit `0`
- 已配置且 mock 命令成功时 exit `0`
- 已配置且 mock 命令失败时 exit 非 `0` 并输出错误信息
- 空字符串配置时 exit `0`

测试 SHALL 使用临时目录隔离 `openspec/config.json`，不依赖项目真实配置。

#### Scenario: 单元测试覆盖未配置场景

- **WHEN** 运行 `pnpm test` 在 `plugins/dev-team/bin/`
- **THEN** `run-static-analysis.test.ts` 中未配置场景的测试通过

#### Scenario: 单元测试覆盖命令失败场景

- **WHEN** 运行 `pnpm test` 在 `plugins/dev-team/bin/`
- **THEN** mock 命令返回非零 exit code 时，测试断言 CLI exit 非 `0`

### Requirement: Plugin no longer embeds openspec CLI

The plugin SHALL NOT include `openspec-bundled.js`, `bin/openspec`, or `bin/openspec.cmd` in its source tree or build products. The three previously-embedded commands (`new change`, `status --json`, `spec list --json`) are replaced by MCP tools.

#### Scenario: No openspec CLI files in source

- **WHEN** examining `plugins/dev-team/bin/` after the change
- **THEN** `openspec-bundled.js` SHALL NOT exist
- **AND** `bin/openspec` and `bin/openspec.cmd` SHALL NOT exist
- **AND** `utils/openspec-cli.sh` SHALL NOT exist

#### Scenario: No openspec CLI files in build products

- **WHEN** examining the build products (`claude-plugins/dev-team/`, `cursor-plugins/dev-team/`, `cursor-home-image/dev-team/`)
- **THEN** `openspec-bundled.js` SHALL NOT exist in any `bin/` directory
- **AND** `bin/openspec` and `bin/openspec.cmd` SHALL NOT exist

## Module Contract

### 删除的文件

| 文件 | 说明 |
|------|------|
| `plugins/dev-team/bin/openspec-bundled.js` | 2.7MB esbuild 打包的 @fission-ai/openspec 全量 |
| `plugins/dev-team/bin/openspec` | bash wrapper |
| `plugins/dev-team/bin/openspec.cmd` | Windows wrapper |
| `plugins/dev-team/utils/openspec-cli.sh` | openspec CLI 的 shell 封装 |

### 替代的 MCP 工具

| 旧命令 | 新工具 |
|--------|--------|
| `openspec new change "<name>"` | `__MCP:change_create__` |
| `openspec status --json` | `__MCP:change_list__`（含 `workflow_done`） |
| `openspec spec list --json` | `__MCP:spec_list__` |

### CLI 入口：`plugins/dev-team/bin/src/cli.ts`

| 函数/命令 | 描述 |
|-----------|------|
| `cli` | `cac` 实例，注册子命令 |
| `run_static_analysis` | 读取 config 并执行静态检查命令 |

### 命令实现：`plugins/dev-team/bin/src/commands/run-static-analysis.ts`

| 函数 | 签名 | 描述 |
|------|------|------|
| `runStaticAnalysis` | `(options?: { projectRoot?: string }) => number` | 读取 `static_analysis` 配置并执行；返回 exit code |

### 依赖模块：`plugins/dev-team/bin/src/lib/config.ts`

| 函数 | 用途 |
|------|------|
| `ensureConfigFile(projectRoot)` | 确保 config 文件存在，返回解析后的 config 对象 |
| `getValue(config, keyPath)` | 按点分路径读取配置值，返回 `{ value, exists }` |

### 打包产物

| 文件 | 描述 |
|------|------|
| `plugins/dev-team/bin/dev-team-cli.cjs` | CLI 打包产物，hook 通过 `node` 调用 |
| `plugins/dev-team/bin/dev-team-mcp.cjs` | MCP server 打包产物（不变） |
