## MODIFIED Requirements

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
- **THEN** those references SHALL use `__SKILL:`, `__AGENT:`, `__MCP:`, `__SKILL_SLASH:`, or `__BIN:` tokens (as applicable)
- **AND** they MUST NOT hard-code only the marketplace plugin qualified names when the same source feeds `cursorHome`

## ADDED Requirements

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
| `skills/` `agents/` `templates/` `utils/` | 占位符化内容 | 硬编码限定名 → `__<KIND>:<id>__` |
| `hooks/` | hooks 关键信息 | 成品 Claude/Cursor JSON 由 assemble 生成 |
| `bin/src/**` | TS；字符串内可含 token | 与 md 同一 assemble 管道 |
| `.claude-plugin/` / `.cursor-plugin/` | — | 仍禁止出现在源码 |
