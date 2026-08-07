## MODIFIED Requirements

### Requirement: Root build script packs and assembles dual platform products

The repository SHALL provide a root build entry under `scripts/` and/or the plugin `package.json` `build` script that builds installable Claude Code and Cursor plugin products **and** the `cursorHome` home image from source. The build SHALL:

1. Read the plugin version from `plugins/dev-team/package.json`
2. Run **one** JS packaging pass for mcp / cli / hooks CJS via **vite-plus** using **`vp pack`**, with cwd `plugins/dev-team`, writing staging CJS **without** env/token replacement (string literals retaining `__…__` tokens)
3. Assemble **three** products (`claude`, `cursor`, `cursorHome`) by applying env tokens, renames, and `hooksProfile` / mcp forks into each explicit `outDir`
4. Write platform manifests (where applicable) that inject the same `version` into all products

Assemble SHALL run inside the `vp pack` lifecycle (e.g. `hooks['build:done']` or an equivalent vite-plus/tsdown pack plugin trailer). The build MUST NOT multiply pack entries by product count (MUST NOT put `cursorHome` into a `SUPPORT_AGENTS`-style pack multiplier). The build MUST NOT perform per-env token `define`/transform during bundling.

The root/plugin build orchestration SHALL NOT introduce a second bundler. Packaging MUST remain vite-plus / `vp pack`.

#### Scenario: Build refreshes three product trees

- **WHEN** the plugin build is executed successfully
- **THEN** `claude-plugins/dev-team`, `cursor-plugins/dev-team`, and `cursor-home-image/dev-team` SHALL exist as assembled trees
- **AND** each tree SHALL include the packed JS artifacts required for its install path

#### Scenario: Same version injected into all products

- **WHEN** `plugins/dev-team/package.json` has `"version": "2.10.3"`
- **AND** the build completes successfully
- **THEN** `claude-plugins/dev-team/.claude-plugin/plugin.json` SHALL contain `"version": "2.10.3"`
- **AND** `cursor-plugins/dev-team/.cursor-plugin/plugin.json` SHALL contain `"version": "2.10.3"`
- **AND** the home image metadata SHALL record the same version

#### Scenario: Source remains free of platform manifests after build

- **WHEN** the build completes successfully
- **THEN** `plugins/dev-team` MUST NOT contain a `.claude-plugin/` or `.cursor-plugin/` directory

#### Scenario: JS pack uses a single vp pack pass without token replacement

- **WHEN** the build runs the JS packaging step
- **THEN** it SHALL invoke `vp pack` (vite-plus) with cwd `plugins/dev-team`
- **AND** staging CJS for mcp / cli / hooks SHALL retain unresolved `__…__` string tokens when present in source
- **AND** it MUST NOT invoke a different bundler for those CJS artifacts
- **AND** it MUST NOT run one full pack graph per product env

### Requirement: Dual products are committed marketplace sources

The directories `claude-plugins/`, `cursor-plugins/`, and `cursor-home-image/` SHALL be tracked in git (not ignored) so that cloning the repository yields installable marketplace sources and a copyable home image without a separate publish step.

#### Scenario: Product directories are not gitignored

- **WHEN** examining repository ignore rules
- **THEN** `claude-plugins/`, `cursor-plugins/`, and `cursor-home-image/` MUST NOT be ignored by `.gitignore`
- **AND** a successful build's product trees SHALL be eligible to commit

#### Scenario: Claude product exposes Claude plugin manifest

- **WHEN** examining `claude-plugins/dev-team`
- **THEN** the tree SHALL contain `.claude-plugin/plugin.json` with `"name": "dev-team"`

#### Scenario: Cursor product exposes Cursor plugin manifest

- **WHEN** examining `cursor-plugins/dev-team`
- **THEN** the tree SHALL contain `.cursor-plugin/plugin.json` with `"name": "dev-team"`

#### Scenario: Home image coexists with marketplace Cursor product

- **WHEN** examining repository product directories after build
- **THEN** `cursor-plugins/dev-team` and `cursor-home-image/dev-team` SHALL both exist
- **AND** neither product SHALL replace the other as the sole Cursor delivery

## ADDED Requirements

### Requirement: Unified env table and placeholder tokens

Source content SHALL use logical ids and placeholders of the form `__<KIND>:<logical-id>__` inside string literals only (markdown, JSON fragments, scripts, and TypeScript strings). Function-shaped placeholders (e.g. `skill('id')`) are forbidden. Placeholders MUST NOT appear as identifiers or import paths.

Supported name-class kinds and bindings:

| KIND | Expansion |
|------|-----------|
| `SKILL` | `namePrefix + id` |
| `AGENT` | `agentRefPrefix + id` |
| `MCP` | `mcpToolPrefix + id` (tool id REQUIRED) |
| `SKILL_SLASH` | `skillSlashPrefix + id` |
| `BIN` | `namePrefix + id + '.cjs'` (derived from `namePrefix`, not a separate rule) |

Path tokens `__DEV_TEAM_ROOT__` / `__DEV_TEAM_RUNTIME_ROOT__` bind to `contentRoot` / `runtimeRoot`. For `claude` / `cursor`, path tokens SHALL be expanded at assemble time. For `cursorHome`, path tokens SHALL remain until install.

Logical ids SHALL be discovered from the filesystem (`skills/<id>/`, `agents/<id>.md`, staging `bin/{mcp,cli,hooks}.cjs`) — the build MUST NOT maintain a hand-written id catalog as the authority. Token replacement SHALL scan wide include globs (skills/agents/hooks/templates/utils text, assembled JSON fragments, staging `bin/*.cjs`) with narrow excludes (e.g. `openspec-bundled.js`, binaries, sourcemaps).

On-disk skill/agent/bin filenames SHALL be derived from `namePrefix` (plugin envs → `mcp.cjs` / `cli.cjs` / `hooks.cjs`; `cursorHome` → `dev-team_mcp.cjs` etc.).

#### Scenario: Assemble expands skill token per env

- **WHEN** source contains `__SKILL:phase-proposal__`
- **AND** assemble runs for `claude` or `cursor` with `namePrefix=''`
- **THEN** the written text SHALL contain `phase-proposal`
- **AND** when assemble runs for `cursorHome` with `namePrefix='dev-team_'`
- **THEN** the written text SHALL contain `dev-team_phase-proposal`

#### Scenario: MCP token requires tool id

- **WHEN** source or frontmatter references MCP tools
- **THEN** it SHALL use `__MCP:<snake_tool_id>__`
- **AND** it MUST NOT use a server-only token or bare server allowlist without a tool id

#### Scenario: Bin filename follows namePrefix

- **WHEN** assemble writes bin artifacts for `cursor` (`namePrefix=''`)
- **THEN** output filenames SHALL be `mcp.cjs`, `cli.cjs`, and `hooks.cjs` (not `dev-team-mcp.cjs`)
- **AND** when assemble writes bin artifacts for `cursorHome`
- **THEN** output filenames SHALL be `dev-team_mcp.cjs`, `dev-team_cli.cjs`, and `dev-team_hooks.cjs`

#### Scenario: Build asserts no leftover name-class tokens

- **WHEN** assemble finishes for a product env
- **THEN** that product tree MUST NOT contain unresolved `__SKILL:`, `__AGENT:`, `__MCP:`, `__SKILL_SLASH:`, or `__BIN:` tokens
- **AND** for `cursorHome`, path tokens MAY still remain

### Requirement: hooksProfile assembles platform hook formats from canonical metadata

Source SHALL maintain hooks **canonical metadata** (logical events, matchers, command templates with `__BIN:` / `__DEV_TEAM_RUNTIME_ROOT__`, optional `loop_limit`, description). Assemble SHALL emit platform files from that metadata:

- `claudeNested` → nested `hooks` → `PreToolUse` / `SubagentStop` with inner `hooks[]` (used by `claude` and marketplace `cursor` envs) at `hooks/hooks.json`
- `cursorNative` → `version` + camelCase events with flat `command` / `matcher` / `loop_limit` (used by `cursorHome`) at image-root `hooks.json`

The build MUST NOT treat a finished Claude nested `hooks.json` as the transformative source for Cursor-native output (no “string-replace Claude JSON into Cursor JSON”). Matchers MAY differ per profile (e.g. Claude `Write|Edit` / `Bash` / `PowerShell` vs Cursor `Write|StrReplace` / `Shell`).

#### Scenario: Claude product receives nested hooks file

- **WHEN** assemble runs for `claude`
- **THEN** it SHALL write `hooks/hooks.json` in `claudeNested` form

#### Scenario: Home product receives native hooks file

- **WHEN** assemble runs for `cursorHome`
- **THEN** it SHALL write a root `hooks.json` in `cursorNative` form
- **AND** command templates MAY still contain path tokens for install-time expansion

#### Scenario: Canonical source is not Claude finished JSON authority

- **WHEN** examining the plugin source tree after this change
- **THEN** hooks canonical metadata SHALL be the authority for assembly
- **AND** a checked-in Claude finished `hooks/hooks.json` MUST NOT be the sole transformative input used to derive `cursorNative` via string replacement

### Requirement: Explicit outDir per env

Each env SHALL declare an explicit `outDir`. The build MUST NOT derive product directories solely by concatenating `` `${agent}-plugins` ``. At minimum:

| env | outDir |
|-----|--------|
| `claude` | `../../claude-plugins/dev-team` |
| `cursor` | `../../cursor-plugins/dev-team` |
| `cursorHome` | `../../cursor-home-image/dev-team` |

#### Scenario: cursorHome outDir is explicit

- **WHEN** assemble runs for `cursorHome`
- **THEN** outputs SHALL be written under `cursor-home-image/dev-team`
- **AND** the path MUST NOT be invented by appending `-plugins` to an agent key

## Module Contract

### Pack + Assemble pipeline

| 阶段 | 输入 | 输出 | 约束 |
|------|------|------|------|
| `vp pack`（一轮 multi-entry） | `bin/src/{mcp,cli,hooks}.ts` | staging `bin/{mcp,cli,hooks}.cjs` | **不**做 env/token 替换；minify 不得改写字符串字面量中的 token |
| assemble×3（pack 生命周期内） | staging CJS + 静态资产 + env 表 | 三产物 `outDir` | 同一 `applyEnvTokens`；FS 权威 rename；宽 globs |
| staging 目录 | — | 如 `.pack-staging/` | gitignore |

### Env 表（字段摘要）

| 字段 | `claude` | `cursor` | `cursorHome` |
|------|----------|----------|--------------|
| `contentRoot` / `runtimeRoot` | `${CLAUDE_PLUGIN_ROOT}` | `.` | 镜像保留；安装期绝对路径 |
| `mcpToolPrefix` | `mcp__plugin_dev-team_dev-team__` | 同左（现网） | `mcp__user-dev-team_mcp__` |
| `namePrefix` | `''` | `''` | `dev-team_` |
| `agentRefPrefix` | `dev-team:` | `dev-team:` | `dev-team_` |
| `skillSlashPrefix` | `/dev-team:` | `/dev-team:` | `/dev-team_` |
| `hooksProfile` | `claudeNested` | `claudeNested`（现网） | `cursorNative` |
| `outDir` | `../../claude-plugins/dev-team` | `../../cursor-plugins/dev-team` | `../../cursor-home-image/dev-team` |
| `pathReplacePhase` | `build` | `build` | `install` |

### Function: `applyEnvTokens(text, env)`

| 方面 | 描述 |
|------|------|
| **用途** | 对 md/json/脚本/CJS 文本统一展开名称与（按阶段）路径 token |
| **输入** | 源文本 + env 行 |
| **输出** | 展开后文本 |
| **禁止** | 函数形占位符；无 tool id 的 MCP server token |

### CLI: 构建入口（更新）

| 方面 | 描述 |
|------|------|
| **打包** | `vp pack` 一轮；cwd `plugins/dev-team` |
| **组装** | assemble(`claude`\|`cursor`\|`cursorHome`) |
| **输出** | 三产物目录；同一 `version` |
| **禁止** | 第二 bundler；按产品倍增 pack；bundle 期 token define |
