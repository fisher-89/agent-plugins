# cursor-home-image Specification

## Purpose
Committed Cursor home-image product at `cursor-home-image/dev-team/` with prefixed skills/agents, Node installer, managed hooks/mcp merge, and install-time path token expansion.

## Requirements

### Requirement: Home image product tree is committed and installable

The repository SHALL produce and track a third product tree at `cursor-home-image/dev-team/` (not ignored by git) so that cloning the repository yields a copyable home image without a separate publish step. The tree SHALL include prefixed skills/agents, `bin/`, `templates/`, `utils/`, Cursor-native `hooks.json` (or equivalent assembled home hooks file), an MCP merge fragment, `install.mjs`, and `manifest.json`. The product version SHALL match `plugins/dev-team/package.json`.

#### Scenario: Home image directory exists after build

- **WHEN** the plugin build completes successfully
- **THEN** `cursor-home-image/dev-team/` SHALL exist
- **AND** it SHALL contain `install.mjs` and `manifest.json`
- **AND** it SHALL contain prefixed skill directories under `skills/` and prefixed agent files under `agents/`

#### Scenario: Home image is not gitignored

- **WHEN** examining repository ignore rules
- **THEN** `cursor-home-image/` MUST NOT be ignored by `.gitignore`
- **AND** a successful build's home image tree SHALL be eligible to commit

#### Scenario: Home image version matches source package

- **WHEN** `plugins/dev-team/package.json` has a given `"version"`
- **AND** the plugin build completes successfully
- **THEN** the home image manifest or equivalent metadata SHALL record the same version

### Requirement: Home image layout mirrors plugin root under ~/.cursor

The home image layout SHALL place `templates/`, `utils/`, and `bin/` as siblings of `skills/` and `agents/` (same relative layout as the plugin root). After install, these SHALL land at `~/.cursor/templates`, `~/.cursor/utils`, and `~/.cursor/bin` respectively. The design MUST NOT introduce a discovery-isolation directory such as `~/.cursor/dev-team/templates`.

Skill and agent **names** in the image SHALL carry the `dev-team_` prefix (e.g. `dev-team_phase-acceptance`, `dev-team_proposal-planner.md`). Isolation of managed content SHALL rely on that prefix plus `manifest.json`, not on a dedicated top-level namespace directory under `~/.cursor`.

#### Scenario: Sibling layout in the image

- **WHEN** examining `cursor-home-image/dev-team/`
- **THEN** `templates/`, `utils/`, and `bin/` SHALL exist as direct children of the product root
- **AND** a `dev-team/` subdirectory MUST NOT be required under those paths for discovery

#### Scenario: Prefixed skill and agent names

- **WHEN** listing `cursor-home-image/dev-team/skills/` and `agents/`
- **THEN** managed skill directory names SHALL start with `dev-team_`
- **AND** managed agent file names SHALL start with `dev-team_`

### Requirement: Path tokens remain until install

For the `cursorHome` product, assemble SHALL expand all name-class tokens (`__SKILL:`, `__CALL_SKILL:`, `__AGENT:`, `__CALL_AGENT:`, `__MCP:`, `__BIN:`) and SHALL leave `__DEV_TEAM_ROOT__` / `__DEV_TEAM_RUNTIME_ROOT__` in the image for install-time absolute-path substitution. The installer SHALL replace both path tokens with the same resolved absolute install root.

#### Scenario: Image retains path tokens

- **WHEN** examining text files in a freshly built `cursor-home-image/dev-team/` that reference content or runtime roots
- **THEN** those files MAY still contain `__DEV_TEAM_ROOT__` and/or `__DEV_TEAM_RUNTIME_ROOT__`
- **AND** they MUST NOT retain unresolved name-class tokens such as `__SKILL:` or `__BIN:`

#### Scenario: Install expands path tokens to absolute paths

- **WHEN** `install.mjs` runs with install root `R` (default `path.resolve(os.homedir(), '.cursor')`)
- **THEN** written non-merge text files SHALL have path tokens replaced with the absolute path of `R`
- **AND** `__DEV_TEAM_ROOT__` and `__DEV_TEAM_RUNTIME_ROOT__` SHALL resolve to the same absolute value

### Requirement: Node installer syncs managed content and merges configs

The home image SHALL ship a cross-platform Node installer (`install.mjs`) that:

1. Resolves the install root (default `~/.cursor`, overridable by flag)
2. Replaces path tokens in image text before write
3. Syncs manifest-listed prefixed skills/agents and `bin` / `templates` / `utils` files
4. Merges `hooks.json` and `mcp.json` by managed-key rules only (MUST NOT blind-overwrite entire files)
5. Writes install state (version, root, managed inventory) and prompts the user to Reload Window

Managed recognition SHALL use the `dev-team_` prefix consistently:

- **hooks**: a managed entry's `command` (or optional name/id) contains `dev-team_`
- **mcp**: `mcpServers` keys start with `dev-team_` (e.g. `dev-team_mcp`)

Update/uninstall operations SHALL only touch managed entries. The installer MUST NOT rewrite hook event names/matchers, MUST NOT rewrite MCP tool-name prefixes, MUST NOT apply name prefixing, and MUST NOT register marketplace plugins.

For path separators: use `path.resolve` for the install root; write JSON via structured merge / `JSON.stringify`; normalize `\` to `/` in non-JSON text before write. Replacements SHALL always start from the image source, never re-tokenize already-installed absolute paths as a second template.

#### Scenario: Managed MCP merge preserves user servers

- **WHEN** the target `mcp.json` already contains a user server key that does not start with `dev-team_`
- **AND** the installer upserts `dev-team_mcp`
- **THEN** the user server key SHALL remain unchanged
- **AND** `dev-team_mcp` SHALL be present with install-root absolute paths

#### Scenario: Managed hooks merge preserves user entries

- **WHEN** the target `hooks.json` contains entries whose commands do not include `dev-team_`
- **AND** the installer merges home-image hooks
- **THEN** those user entries SHALL remain
- **AND** managed entries whose commands include `dev-team_` SHALL be upserted or refreshed from the image

#### Scenario: Installer refuses blind overwrite

- **WHEN** installing into a home that already has `hooks.json` and `mcp.json`
- **THEN** the installer MUST NOT replace those files wholesale with image copies
- **AND** it SHALL apply managed-key merge only

#### Scenario: Non-JSON path separator normalization

- **WHEN** the resolved install root on Windows contains backslashes
- **AND** the installer writes a non-JSON text file (e.g. agent markdown) that embeds the root path
- **THEN** embedded path separators in that text SHALL be normalized to `/`

### Requirement: cursorHome user-level reference prefixes

The `cursorHome` env SHALL use the following user-level prefixes (pinned by real-device observation):

| Field | Value |
|-------|--------|
| MCP server key | `dev-team_mcp` |
| `mcpToolPrefix` | `mcp__user-dev-team_mcp__` |
| `namePrefix` | `dev-team_` |
| `pluginPrefix` | `''` |

`__MCP:<tool>__` SHALL expand to `mcp__user-dev-team_mcp__<tool>`. `__CALL_SKILL:` / `__CALL_AGENT:` SHALL expand to `namePrefix + id` (empty `pluginPrefix`). Source and frontmatter MUST NOT use server-level MCP allowlist strings without a tool id.

#### Scenario: Home MCP tool token expansion

- **WHEN** assemble expands `__MCP:phase_next__` for `cursorHome`
- **THEN** the result SHALL be `mcp__user-dev-team_mcp__phase_next`

#### Scenario: Home call-skill / call-agent token expansion

- **WHEN** assemble expands `__CALL_SKILL:phase-proposal__` for `cursorHome`
- **THEN** the result SHALL be `dev-team_phase-proposal`
- **AND** when assemble expands `__CALL_AGENT:proposal-planner__` for `cursorHome`
- **THEN** the result SHALL be `dev-team_proposal-planner`

#### Scenario: No server-level MCP reference in source

- **WHEN** examining skill/agent `tools:` frontmatter after this change
- **THEN** entries MUST NOT be bare server namespaces such as `mcp__plugin_dev-team_dev-team` without a tool id
- **AND** MCP tool references SHALL use `__MCP:<tool_id>__` form in source

## Module Contract

### Product: `cursor-home-image/dev-team/`

| 路径 | 角色 | 约束 |
|------|------|------|
| `skills/` | 前缀化 skill 树 | 目录名以 `dev-team_` 开头 |
| `agents/` | 前缀化 agent 文件 | 文件名以 `dev-team_` 开头 |
| `hooks.json` | Cursor 原生 hooks | `hooksProfile=cursorNative`；路径 token 可保留至安装 |
| `bin/` | 打包 CJS | 文件名为 `dev-team_{mcp,cli,hooks}.cjs` |
| `templates/` / `utils/` | 与插件根同构 | 安装到 `~/.cursor` 同级路径 |
| `mcp.dev-team.json`（或等价片段） | mcp merge 源 | server key `dev-team_mcp` |
| `install.mjs` | Node 安装器 | 路径替换 + sync + managed merge |
| `manifest.json` | 版本与托管清单 | 约束更新/卸载范围 |

### CLI: `install.mjs`

| 方面 | 描述 |
|------|------|
| **运行时** | Node（跨平台） |
| **默认 root** | `path.resolve(os.homedir(), '.cursor')` |
| **输入** | 镜像树 + 可选 `--root` |
| **步骤** | 解析 root → 路径 token 替换 → 按 manifest 同步 → merge hooks/mcp → 写状态 → 提示 Reload |
| **merge 识别** | hooks：`command` 含 `dev-team_`；mcp：key 以 `dev-team_` 开头 |
| **禁止** | 整文件盲覆盖；名称前缀化；marketplace 注册；以已安装文件二次替换 |

### Env: `cursorHome`（摘要）

| 字段 | 值 |
|------|-----|
| `outDir` | `../../cursor-home-image/dev-team`（显式，禁止 `${agent}-plugins` 拼接臆造） |
| `layout` | `home-image` |
| `hooksProfile` | `cursorNative` |
| `pathReplacePhase` | `install`（仅路径两列） |
| `namePrefix` | `dev-team_` |
| `pluginPrefix` | `''` |
| `mcpToolPrefix` | `mcp__user-dev-team_mcp__` |
