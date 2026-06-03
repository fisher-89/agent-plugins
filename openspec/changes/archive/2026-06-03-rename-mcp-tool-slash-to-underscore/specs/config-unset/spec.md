## MODIFIED Requirements

### Requirement: config_unset removes key values from config.json

The system SHALL provide a `config_unset` MCP tool (formerly `config/unset`) that removes keys from `openspec/config.json`.
The tool SHALL read and parse the JSON file using Node.js `fs` module and `JSON.parse`.
The tool SHALL accept an input object with `key` (string, required) and `project_root` (string, optional) fields.
The `key` field SHALL support dot-separated nested key paths to remove values at arbitrary depth.
The tool SHALL return an output object with `key` (string) and `removed` (boolean) fields.
When the specified key exists and is successfully removed, `removed` SHALL be `true`.
When the specified key does not exist in the JSON file, `removed` SHALL be `false` (no error).
When a parent key becomes empty after removing the last nested key, the parent SHALL remain in the file as an empty object.
The tool SHALL use native `JSON.parse`/`JSON.stringify` for all file operations (not the `yaml` package).

#### Scenario: Remove existing top-level key

- **WHEN** the user calls `config_unset` with `{"key": "static_check"}`
- **AND** `openspec/config.json` contains `{"static_check": ["eslint src/"]}`
- **THEN** the tool returns `{"key": "static_check", "removed": true}`
- **AND** `static_check` is no longer present in the JSON file

#### Scenario: Remove nested key via dot path

- **WHEN** the user calls `config_unset` with `{"key": "test_scripts.e2e"}`
- **AND** `openspec/config.json` contains `{"test_scripts": {"unit": "npm test", "e2e": "npm run test:e2e"}}`
- **THEN** the tool returns `{"key": "test_scripts.e2e", "removed": true}`
- **AND** `test_scripts` still contains `"unit"` but no longer contains `"e2e"`

#### Scenario: Remove non-existent key returns removed: false

- **WHEN** the user calls `config_unset` with `{"key": "nonexistent"}`
- **THEN** the tool returns `{"key": "nonexistent", "removed": false}`

#### Scenario: Remove key preserves other content

- **WHEN** the user calls `config_unset` with `{"key": "static_check"}`
- **AND** the file contains `{"schema": "spec-driven", "static_check": ["eslint src/"], "context": "test"}`
- **THEN** `static_check` is removed but `schema` and `context` are preserved

### Requirement: config_unset handles missing config file gracefully

When the `openspec/config.json` file does not exist, the tool SHALL return `removed: false` without throwing an error.
The tool SHALL NOT create the config file on unset operations.

#### Scenario: Config file does not exist on unset

- **WHEN** the user calls `config_unset` with `{"key": "schema"}`
- **AND** `openspec/config.json` does not exist
- **THEN** the tool returns `{"key": "schema", "removed": false}`

## Module Contract

### API: config_unset (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config_unset` (formerly `config/unset`) |
| **Input schema** | `{ key: z.string(), project_root: z.string().optional() }` |
| **Output schema** | `{ key: z.string(), removed: z.boolean() }` |
| **Registration** | `mcp.ts` -- `server.registerTool('config_unset', ...)` |
| **Handler** | `async (args) => { ... return jsonContent(result); }` |
