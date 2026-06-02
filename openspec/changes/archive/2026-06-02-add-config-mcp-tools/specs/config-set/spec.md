## ADDED Requirements

### Requirement: config/set writes key values to config.yaml

The system SHALL provide a `config/set` MCP tool that writes values to `openspec/config.yaml`.
The tool SHALL accept an input object with `key` (string, required), `value` (unknown, required), and `project_root` (string, optional) fields.
The `key` field SHALL support dot-separated nested key paths to set values at arbitrary depth.
The tool SHALL return an output object with `key` (string) and `written` (boolean, always `true` on success) fields.
When writing to an existing YAML document, the tool SHALL preserve all existing keys, values, and comments that are not being modified.
The tool SHALL create the `openspec/config.yaml` file if it does not exist, initializing it with a minimal skeleton containing `schema: spec-driven`.

#### Scenario: Write new top-level key

- **WHEN** the user calls `config/set` with `{"key": "static_check", "value": ["eslint src/", "tsc --noEmit"]}`
- **THEN** the tool returns `{"key": "static_check", "written": true}`
- **AND** `openspec/config.yaml` contains `static_check` with the specified array value

#### Scenario: Write nested key via dot path

- **WHEN** the user calls `config/set` with `{"key": "test_scripts.unit", "value": "npm test"}`
- **THEN** the tool returns `{"key": "test_scripts.unit", "written": true}`
- **AND** `openspec/config.yaml` contains `test_scripts: {unit: "npm test"}`

#### Scenario: Overwrite existing key preserving other content

- **WHEN** the user calls `config/set` with `{"key": "schema", "value": "spec-driven-v2"}`
- **AND** the file already has `schema: spec-driven` with other keys and comments
- **THEN** only the `schema` key is updated to the new value
- **AND** all other keys and comments in the file are preserved

#### Scenario: Config file auto-created when missing

- **WHEN** the user calls `config/set` with `{"key": "static_check", "value": ["eslint src/"]}`
- **AND** `openspec/config.yaml` does not exist
- **THEN** the file is created with `schema: spec-driven` and the new key written

### Requirement: config/set uses yaml library for structured YAML manipulation

The tool SHALL use the `yaml` npm package (or equivalent) for parsing and serializing YAML, rather than string manipulation.
The YAML serialization SHALL use a configuration that preserves the existing document structure, including comments, key ordering, and formatting where possible.

#### Scenario: YAML comments preserved on write

- **WHEN** `openspec/config.yaml` contains a comment `# Project context (optional)` before the `context` key
- **AND** the user calls `config/set` with `{"key": "schema", "value": "spec-driven"}`
- **THEN** the comment is preserved in the output file

## Module Contract

### Function: writeConfigValue

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `writeConfigValue(projectRoot: string, key: string, value: unknown): ConfigSetResult` |
| **Input** | `projectRoot` — project root directory path; `key` — dot-separated key path; `value` — any JSON-serializable value |
| **Output** | `ConfigSetResult` — `{ key: string; written: boolean }` |
| **Behavior** | Reads YAML (or creates skeleton), sets value at key path, serializes back to file |

### Function: ensureConfigFile

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `ensureConfigFile(projectRoot: string): string` |
| **Input** | `projectRoot` — project root directory path |
| **Output** | Absolute path to the config file |
| **Behavior** | Creates `openspec/config.yaml` with `schema: spec-driven` skeleton if it does not exist; creates `openspec/` directory if needed |

### API: config/set (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config/set` |
| **Input schema** | `{ key: z.string(), value: z.unknown(), project_root: z.string().optional() }` |
| **Output schema** | `{ key: z.string(), written: z.boolean() }` |
| **Registration** | `mcp.ts` — `server.registerTool('config/set', ...)` |
| **Handler** | `async (args) => { ... return jsonContent(result); }` |
