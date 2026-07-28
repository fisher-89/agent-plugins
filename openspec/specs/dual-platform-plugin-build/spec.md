## ADDED Requirements

### Requirement: Root build script packs and assembles dual platform products

The repository SHALL provide a root build entry under `scripts/` (e.g. `scripts/build-plugins.mjs`) that builds installable Claude Code and Cursor plugin products from source. The build SHALL:

1. Read the plugin version from `plugins/dev-team/package.json`
2. Run JS packaging for mcp / cli / hooks CJS via **vite-plus** using the command **`vp pack`**, with the working directory set to the plugin source root `plugins/dev-team` (after `package.json` has been moved to that root)
3. Assemble plugin content into `claude-plugins/dev-team` and `cursor-plugins/dev-team`
4. Write platform manifests that inject the same `version` into both products

The root script SHALL orchestrate `vp pack` and product assembly; it MUST NOT introduce a second bundler. Packaging MUST remain consistent with the existing `bin` vite-plus / `vp pack` approach, migrating only the working directory and product assembly paths.

This iteration MAY keep skills / agents / hooks content isomorphic (or apply only minimal installability differences) between the two products. The build MUST NOT require a full tool-name or hooks-response replacement table.

#### Scenario: Build refreshes both product trees

- **WHEN** the root build script is executed successfully
- **THEN** `claude-plugins/dev-team` and `cursor-plugins/dev-team` SHALL exist as assembled plugin trees
- **AND** both trees SHALL include the packed JS artifacts required for installation

#### Scenario: Same version injected into both manifests

- **WHEN** `plugins/dev-team/package.json` has `"version": "2.10.3"`
- **AND** the root build script completes successfully
- **THEN** `claude-plugins/dev-team/.claude-plugin/plugin.json` SHALL contain `"version": "2.10.3"`
- **AND** `cursor-plugins/dev-team/.cursor-plugin/plugin.json` SHALL contain `"version": "2.10.3"`

#### Scenario: Source remains free of platform manifests after build

- **WHEN** the root build script completes successfully
- **THEN** `plugins/dev-team` MUST NOT contain a `.claude-plugin/` or `.cursor-plugin/` directory

#### Scenario: JS pack uses vp pack at plugin source root

- **WHEN** the root build script runs the JS packaging step
- **THEN** it SHALL invoke `vp pack` (vite-plus) with cwd `plugins/dev-team`
- **AND** it MUST NOT invoke a different bundler (e.g. webpack, esbuild CLI, rollup, tsup) for mcp / cli / hooks CJS packaging

### Requirement: Dual products are committed marketplace sources

The directories `claude-plugins/` and `cursor-plugins/` SHALL be tracked in git (not ignored) so that cloning the repository yields installable marketplace sources without a separate publish step.

#### Scenario: Product directories are not gitignored

- **WHEN** examining repository ignore rules
- **THEN** `claude-plugins/` and `cursor-plugins/` MUST NOT be ignored by `.gitignore`
- **AND** a successful build's product trees SHALL be eligible to commit

#### Scenario: Claude product exposes Claude plugin manifest

- **WHEN** examining `claude-plugins/dev-team`
- **THEN** the tree SHALL contain `.claude-plugin/plugin.json` with `"name": "dev-team"`

#### Scenario: Cursor product exposes Cursor plugin manifest

- **WHEN** examining `cursor-plugins/dev-team`
- **THEN** the tree SHALL contain `.cursor-plugin/plugin.json` with `"name": "dev-team"`

### Requirement: Cursor plugin directory is not ignored by dot-directory rules

The repository `.gitignore` SHALL explicitly un-ignore `.cursor-plugin` (e.g. `!.cursor-plugin`) so that the Cursor marketplace manifest can be versioned despite a broad `.*` ignore rule.

#### Scenario: .cursor-plugin is trackable

- **WHEN** `.cursor-plugin/marketplace.json` is added under the repository root
- **THEN** git ignore rules SHALL allow tracking that path
- **AND** `.gitignore` SHALL contain an exception for `.cursor-plugin`

## Module Contract

### CLI: `scripts/build-plugins.*`

| 方面 | 描述 |
|------|------|
| **入口** | 仓库根 `scripts/` 下的构建脚本（推荐 `scripts/build-plugins.mjs`） |
| **输入** | `plugins/dev-team/package.json` 的 `version`；`plugins/dev-team` 源码树 |
| **步骤** | (1) 读取 version (2) 在 `plugins/dev-team` 执行 `vp pack`（vite-plus）打包 mcp / cli / hooks CJS (3) 组装双产物目录 (4) 写入平台 `plugin.json` |
| **打包工具** | **锁定** vite-plus / `vp pack`；MUST NOT 引入第二套 bundler |
| **输出** | `claude-plugins/dev-team/`、`cursor-plugins/dev-team/` |
| **版本契约** | 两边产物 `plugin.json` 的 `version` MUST 等于源 `package.json` 的 `version` |
| **本迭代非目标** | 全量工具名替换、hooks 反参适配、强制写入 Cursor `mcp.json`（MAY 预留组装钩子） |

### Pack: `vp pack`（vite-plus）

| 方面 | 描述 |
|------|------|
| **命令** | `vp pack` |
| **工具** | vite-plus（与现有 `bin` 打包一致） |
| **cwd** | `plugins/dev-team`（`package.json` 上移后的插件源码根） |
| **产物** | mcp / cli / hooks 的 CJS 打包结果 |
| **禁止** | 为本迭代引入 webpack / esbuild / rollup / tsup 等替代打包链路 |

### Manifest: `claude-plugins/dev-team/.claude-plugin/plugin.json`

| 字段 | 类型 | 描述 | 变更 |
|------|------|------|------|
| `name` | `string` | 插件名，值为 `"dev-team"` | 由构建写入产物 |
| `version` | `string` | semver，与源 `package.json` 一致 | 由构建注入 |
| 其他元数据 | — | description / author / bin 等 | 由构建从源约定生成或复制 |

### Manifest: `cursor-plugins/dev-team/.cursor-plugin/plugin.json`

| 字段 | 类型 | 描述 | 变更 |
|------|------|------|------|
| `name` | `string` | 插件名，值为 `"dev-team"` | 由构建写入产物 |
| `version` | `string` | semver，与源 `package.json` 一致 | 由构建注入 |
| 其他元数据 | — | 平台所需字段 | 由构建写入；本迭代允许与 Claude 侧内容同构 |
