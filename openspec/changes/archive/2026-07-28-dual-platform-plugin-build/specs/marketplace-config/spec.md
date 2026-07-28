## MODIFIED Requirements

### Requirement: Root marketplace.json exists
The repository SHALL contain hand-written marketplace manifests for both Claude Code and Cursor:

- `.claude-plugin/marketplace.json` for Claude Code
- `.cursor-plugin/marketplace.json` for Cursor

Each file SHALL be valid JSON suitable for the respective host's plugin marketplace configuration.

#### Scenario: Claude marketplace file structure
- **WHEN** examining the repository root Claude marketplace path
- **THEN** `.claude-plugin/marketplace.json` SHALL exist with valid JSON structure

#### Scenario: Cursor marketplace file structure
- **WHEN** examining the repository root Cursor marketplace path
- **THEN** `.cursor-plugin/marketplace.json` SHALL exist with valid JSON structure

### Requirement: Marketplace registers available plugins
Each marketplace manifest SHALL list available plugins with metadata and a `source` path pointing at the corresponding **product** directory (not the editable `plugins/` source tree).

#### Scenario: Claude marketplace registers built product
- **WHEN** reading `.claude-plugin/marketplace.json`
- **THEN** it SHALL contain an entry for the `"dev-team"` plugin
- **AND** that entry's `source` SHALL be `"./claude-plugins/dev-team"`

#### Scenario: Cursor marketplace registers built product
- **WHEN** reading `.cursor-plugin/marketplace.json`
- **THEN** it SHALL contain an entry for the `"dev-team"` plugin
- **AND** that entry's `source` SHALL be `"./cursor-plugins/dev-team"`

### Requirement: Marketplace entries match plugin manifests
Each plugin entry in a marketplace manifest SHALL match the corresponding **product** platform manifest data (name and description at minimum).

#### Scenario: Claude marketplace entry consistency
- **WHEN** a plugin is listed in `.claude-plugin/marketplace.json`
- **THEN** the entry's name and description SHALL match that plugin's `claude-plugins/<name>/.claude-plugin/plugin.json`

#### Scenario: Cursor marketplace entry consistency
- **WHEN** a plugin is listed in `.cursor-plugin/marketplace.json`
- **THEN** the entry's name and description SHALL match that plugin's `cursor-plugins/<name>/.cursor-plugin/plugin.json`

## Module Contract

### Config: `.claude-plugin/marketplace.json`

| 字段 | 类型 | 描述 | 变更 |
|------|------|------|------|
| `name` | `string` | marketplace 名称（如 `"wps-ai"`） | 不变 |
| `plugins[].name` | `string` | 插件名 `"dev-team"` | 不变 |
| `plugins[].source` | `string` | 插件路径 | 从 `"./plugins/dev-team"` 改为 `"./claude-plugins/dev-team"` |
| `plugins[].description` | `string` | 与产物 manifest 一致 | 对齐产物 |

### Config: `.cursor-plugin/marketplace.json`

| 字段 | 类型 | 描述 | 变更 |
|------|------|------|------|
| `plugins[].name` | `string` | 插件名 `"dev-team"` | **新增文件** |
| `plugins[].source` | `string` | `"./cursor-plugins/dev-team"` | **新增** |
| `plugins[].description` | `string` | 与 Cursor 产物 manifest 一致 | **新增** |
