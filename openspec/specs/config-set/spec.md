## ADDED Requirements

### Requirement: config/set writes key values to config.json

The system SHALL provide a `config/set` MCP tool that writes values to `openspec/config.json`.
The tool SHALL read and parse the JSON file using Node.js `fs` module and `JSON.parse`.
The tool SHALL accept an input object with `key` (string, required), `value` (unknown, required), and `project_root` (string, optional) fields.
The `key` field SHALL support dot-separated nested key paths to set values at arbitrary depth.
The tool SHALL return an output object with `key` (string) and `written` (boolean, always `true` on success) fields.
When writing to an existing JSON document, the tool SHALL preserve all existing keys and values that are not being modified.
The tool SHALL create the `openspec/config.json` file if it does not exist, initializing it with a minimal skeleton containing `{"schema": "spec-driven"}`.
After setting the value in the in-memory config object, the tool SHALL validate the entire config through `parseConfig` before writing to disk. If `parseConfig` throws, the tool SHALL reject the write and propagate the validation error without modifying the file on disk.

#### Scenario: Write new top-level key

- **WHEN** the user calls `config/set` with `{"key": "static_check", "value": ["eslint src/", "tsc --noEmit"]}`
- **THEN** the tool returns `{"key": "static_check", "written": true}`
- **AND** `openspec/config.json` contains `"static_check"` with the specified array value

#### Scenario: Write nested key via dot path

- **WHEN** the user calls `config/set` with `{"key": "test_scripts.unit", "value": "npm test"}`
- **THEN** the tool returns `{"key": "test_scripts.unit", "written": true}`
- **AND** `openspec/config.json` contains `{"test_scripts": {"unit": "npm test"}}`

#### Scenario: Overwrite existing key preserving other content

- **WHEN** the user calls `config/set` with `{"key": "schema", "value": "spec-driven"}`
- **AND** the file already has `{"schema": "spec-driven"}` with other keys
- **THEN** only the `schema` key is updated to the new value
- **AND** all other keys in the file are preserved

#### Scenario: Config file auto-created when missing

- **WHEN** the user calls `config/set` with `{"key": "static_check", "value": ["eslint src/"]}`
- **AND** `openspec/config.json` does not exist
- **THEN** the file is created with `{"schema": "spec-driven"}` and the new key written

#### Scenario: Validate against Zod schema before write

- **WHEN** the user calls `config/set` with `{"key": "schema", "value": null}`
- **THEN** the tool returns an error indicating validation failure (`schema` must be a string literal)
- **AND** the file on disk is NOT modified

#### Scenario: JSON output uses 2-space indentation

- **WHEN** the user calls `config/set` with `{"key": "schema", "value": "spec-driven"}`
- **AND** `openspec/config.json` does not exist
- **THEN** the created file contains valid JSON with 2-space indentation

## Module Contract

### Function: writeConfigValue

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `writeConfigValue(projectRoot: string, key: string, value: unknown): boolean` |
| **Input** | `projectRoot` — project root directory path; `key` — dot-separated key path; `value` — any JSON-serializable value |
| **Output** | `boolean` — `true` on success |
| **Behavior** | Reads JSON (or creates skeleton via `ensureConfigFile`), sets value at key path, validates full config with `parseConfig`, serializes with `JSON.stringify` (2-space indent), writes to file |

### Function: ensureConfigFile

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `ensureConfigFile(projectRoot: string): OpenSpecConfig` |
| **Input** | `projectRoot` — project root directory path |
| **Output** | `OpenSpecConfig` — typed config object |
| **Behavior** | Creates `openspec/config.json` with `{"schema": "spec-driven"}` skeleton if it does not exist (not YAML). Creates `openspec/` directory if needed. Returns parsed and validated config object |

### API: config/set (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config/set` |
| **Input schema** | `{ key: z.string(), value: z.unknown(), project_root: z.string().optional() }` |
| **Output schema** | `{ key: z.string(), written: z.boolean() }` |
| **Registration** | `mcp.ts` — `server.registerTool('config/set', ...)` |
| **Handler** | `async (args) => { ... return jsonContent(result); }` |
