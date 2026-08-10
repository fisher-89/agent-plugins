# multi-plugin-layout Specification

## Purpose
Repository layout for multiple plugin sources under `plugins/` and installable product trees under Claude/Cursor marketplace dirs plus `cursor-home-image/`.

## Requirements

### Requirement: Plugins directory supports multiple plugins

The repository SHALL support a `plugins/` directory containing multiple plugin **source** subdirectories. Each source subdirectory SHALL hold the editable plugin content (`package.json`, `bin/`, `hooks/` canonical metadata, `skills/`, `agents/`, `utils/`, `templates/`, etc.) and MUST NOT contain platform install manifests (`.claude-plugin/` or `.cursor-plugin/`). Installable trees SHALL live under `claude-plugins/<name>/`, `cursor-plugins/<name>/`, and — for home delivery — `cursor-home-image/<name>/` after build.

Source content intended for cross-product delivery SHALL use `__<KIND>:<id>__` placeholders (and path tokens) rather than hard-coding a single product's final qualified names.

#### Scenario: Plugin source directory structure

- **WHEN** examining the `plugins/` directory
- **THEN** each subdirectory represents a distinct plugin source tree with `package.json`, `hooks/`, `skills/`, `utils/`, and `templates/` as applicable
- **AND** the source tree MUST NOT contain `.claude-plugin/` or `.cursor-plugin/`

#### Scenario: Installable products are outside plugins/

- **WHEN** examining installable locations after a successful build
- **THEN** Claude Code products SHALL exist under `claude-plugins/`
- **AND** Cursor marketplace products SHALL exist under `cursor-plugins/`
- **AND** Cursor home-image products SHALL exist under `cursor-home-image/`

#### Scenario: Source uses placeholders instead of single-product literals

- **WHEN** examining cross-product references in `plugins/dev-team` skills, agents, or bin TypeScript strings after this change
- **THEN** those references SHALL use `__SKILL:`, `__CALL_SKILL:`, `__AGENT:`, `__CALL_AGENT:`, `__MCP:`, or `__BIN:` tokens (as applicable)
- **AND** they MUST NOT hard-code only the marketplace plugin qualified names when the same source feeds `cursorHome`

### Requirement: Plugin manifest contains plugin name

Each **installable** plugin product's platform manifest SHALL define a unique `name` field identifying the plugin. Source trees SHALL NOT be the location of platform manifests; the build writes manifests into product trees.

#### Scenario: Dev-team Claude product manifest

- **WHEN** reading `claude-plugins/dev-team/.claude-plugin/plugin.json`
- **THEN** the `name` field equals `"dev-team"`

#### Scenario: Dev-team Cursor product manifest

- **WHEN** reading `cursor-plugins/dev-team/.cursor-plugin/plugin.json`
- **THEN** the `name` field equals `"dev-team"`

### Requirement: Plugin hooks reference correct utils path

Hook scripts SHALL reference their plugin's `utils/` directory using the correct path structure relative to the plugin root in which they execute (source during development, or the assembled product tree at runtime).

#### Scenario: Hook imports from plugin utils

- **WHEN** a hook script under the plugin's `hooks/` directory executes in an installed or assembled plugin root
- **THEN** it SHALL resolve that plugin root's `utils/` directory before importing utility modules

### Requirement: Plugin package.json is the version authority

Each plugin source tree under `plugins/<name>/` SHALL have a root `package.json` whose `version` field is the authoritative plugin semver. The former `plugins/dev-team/bin/package.json` SHALL be moved up to `plugins/dev-team/package.json`. When migrating, the `version` MUST be set to the current published plugin version (`2.10.3`) or the next agreed semver — it MUST NOT retain the historical `bin/package.json` value `1.0.0` as the published plugin version. Documentation (`CLAUDE.md`) SHALL instruct contributors to bump `plugins/<name>/package.json` and rebuild both products after changing plugin code.

#### Scenario: package.json lives at plugin source root

- **WHEN** examining `plugins/dev-team/`
- **THEN** `plugins/dev-team/package.json` SHALL exist
- **AND** `plugins/dev-team/bin/package.json` MUST NOT exist

#### Scenario: Migrated version is not 1.0.0

- **WHEN** reading `plugins/dev-team/package.json` after the layout migration
- **THEN** the `version` field SHALL be `2.10.3` or a newer agreed semver
- **AND** the `version` MUST NOT be `1.0.0` solely due to copying the old bin package metadata

#### Scenario: CLAUDE.md version bump rule points at package.json

- **WHEN** reading the project `CLAUDE.md` version upgrade rule
- **THEN** it SHALL require bumping `plugins/<name>/package.json` and rebuilding dual products after plugin code changes

### Requirement: Documentation covers three-product rebuild

Project guidance (`CLAUDE.md`) SHALL instruct contributors to bump `plugins/<name>/package.json` and rebuild **all** products (Claude marketplace, Cursor marketplace, and cursor home image) after changing plugin source.

#### Scenario: CLAUDE.md mentions home image rebuild

- **WHEN** reading the project `CLAUDE.md` version upgrade rule
- **THEN** it SHALL require bumping `plugins/<name>/package.json`
- **AND** it SHALL require rebuilding products including `cursor-home-image/` (not only the dual marketplace trees)

## Module Contract

### Layout: 产物目录（更新）

| 路径 | 角色 | 变更 |
|------|------|------|
| `claude-plugins/dev-team/` | Claude Code 可安装树 | 保留；assemble 刷新 |
| `cursor-plugins/dev-team/` | Cursor marketplace 可安装树 | 保留；与 home 并存 |
| `cursor-home-image/dev-team/` | `~/.cursor` 镜像 + 安装器 | **新增**；可 commit |

### Layout: `plugins/dev-team/`（源码，更新）

| 路径 | 角色 | 变更 |
|------|------|------|
| `package.json` | 权威 `version` 与 Node 包元数据 | **新增（上移）**；删除 `bin/package.json` |
| `skills/` `agents/` `templates/` `utils/` | 占位符化内容 | 硬编码限定名 → `__<KIND>:<id>__` |
| `hooks/` | hooks 关键信息 | 成品 Claude/Cursor JSON 由 assemble 生成 |
| `bin/` | TS 源、vite-plus / `vp pack` 配置与打包中间产物 | 不再自带 `package.json`；pack 由根脚本在源码根调用 `vp pack` |
| `bin/src/**` | TS；字符串内可含 token | 与 md 同一 assemble 管道 |
| `.claude-plugin/` / `.cursor-plugin/` | — | 仍禁止出现在源码 |
