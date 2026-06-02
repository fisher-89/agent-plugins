## ADDED Requirements

### Requirement: config/unset removes key values from config.yaml

The system SHALL provide a `config/unset` MCP tool that removes keys from `openspec/config.yaml`.
The tool SHALL accept an input object with `key` (string, required) and `project_root` (string, optional) fields.
The `key` field SHALL support dot-separated nested key paths to remove values at arbitrary depth.
The tool SHALL return an output object with `key` (string) and `removed` (boolean) fields.
When the specified key exists and is successfully removed, `removed` SHALL be `true`.
When the specified key does not exist in the YAML file, `removed` SHALL be `false` (no error).
When a parent key becomes empty after removing the last nested key, the parent SHALL remain in the file as an empty object.

#### Scenario: Remove existing top-level key

- **WHEN** the user calls `config/unset` with `{"key": "static_check"}`
- **AND** `openspec/config.yaml` contains `static_check: ["eslint src/"]`
- **THEN** the tool returns `{"key": "static_check", "removed": true}`
- **AND** `static_check` is no longer present in the YAML file

#### Scenario: Remove nested key via dot path

- **WHEN** the user calls `config/unset` with `{"key": "test_scripts.e2e"}`
- **AND** `openspec/config.yaml` contains `test_scripts: {unit: "npm test", e2e: "npm run test:e2e"}`
- **THEN** the tool returns `{"key": "test_scripts.e2e", "removed": true}`
- **AND** `test_scripts` still contains `unit` but no longer contains `e2e`

#### Scenario: Remove non-existent key returns removed: false

- **WHEN** the user calls `config/unset` with `{"key": "nonexistent"}`
- **THEN** the tool returns `{"key": "nonexistent", "removed": false}`

#### Scenario: Remove key preserves other content and comments

- **WHEN** the user calls `config/unset` with `{"key": "static_check"}`
- **AND** the file contains a comment `# Dev-team extensions:` before `static_check`
- **THEN** the `static_check` key is removed but the comment and all other keys are preserved

### Requirement: config/unset handles missing config file gracefully

When the `openspec/config.yaml` file does not exist, the tool SHALL return `removed: false` without throwing an error.
The tool SHALL NOT create the config file on unset operations.

#### Scenario: Config file does not exist on unset

- **WHEN** the user calls `config/unset` with `{"key": "schema"}`
- **AND** `openspec/config.yaml` does not exist
- **THEN** the tool returns `{"key": "schema", "removed": false}`

## Module Contract

### Function: unsetConfigValue

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `unsetConfigValue(projectRoot: string, key: string): ConfigUnsetResult` |
| **Input** | `projectRoot` — project root directory path; `key` — dot-separated key path |
| **Output** | `ConfigUnsetResult` — `{ key: string; removed: boolean }` |
| **Behavior** | Reads YAML, removes value at key path, serializes back. Returns `removed: false` if key or file does not exist |

### API: config/unset (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config/unset` |
| **Input schema** | `{ key: z.string(), project_root: z.string().optional() }` |
| **Output schema** | `{ key: z.string(), removed: z.boolean() }` |
| **Registration** | `mcp.ts` — `server.registerTool('config/unset', ...)` |
| **Handler** | `async (args) => { ... return jsonContent(result); }` |
