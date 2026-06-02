## MODIFIED Requirements

### Requirement: config/get reads config.yaml key values

The system SHALL provide a `config/get` MCP tool that reads values from `openspec/config.json`.
The tool SHALL read and parse the JSON file using Node.js `fs` module and `JSON.parse`.
The tool SHALL use the `safeParseConfig` function from `schemas/config.schema` to validate the config structure; if validation fails, the tool SHALL still return key path results from the raw parsed data (graceful degradation).
The tool SHALL accept an input object with `key` (string, required) and `project_root` (string, optional) fields.
The `key` field SHALL support dot-separated nested key paths (e.g., `test_scripts.unit`) to access nested JSON keys.
The `project_root` field SHALL default to `process.env.CLAUDE_PROJECT_DIR` or `process.cwd()` when not provided.
The tool SHALL return an output object with `key` (string), `value` (unknown, present only if key exists), and `exists` (boolean) fields.
The tool SHALL resolve the config file path as `<project_root>/openspec/config.json`.
When the key does not exist in the JSON file, `exists` SHALL be `false` and `value` SHALL be absent from the output.
When the config file does not exist, the tool SHALL return `exists: false` without throwing an error.
On auto-migration (JSON absent but YAML present), the tool SHALL read the YAML file, validate the content against the Zod schema, write the validated data to `config.json`, and delete the `config.yaml` file before performing the read.

#### Scenario: Read top-level key that exists

- **WHEN** the user calls `config/get` with `{"key": "schema"}`
- **AND** `openspec/config.json` contains `{"schema": "spec-driven"}`
- **THEN** the tool returns `{"key": "schema", "value": "spec-driven", "exists": true}`

#### Scenario: Read nested key via dot path

- **WHEN** the user calls `config/get` with `{"key": "test_scripts.unit"}`
- **AND** `openspec/config.json` contains `{"test_scripts": {"unit": "npm test"}}`
- **THEN** the tool returns `{"key": "test_scripts.unit", "value": "npm test", "exists": true}`

#### Scenario: Read non-existent key

- **WHEN** the user calls `config/get` with `{"key": "nonexistent"}`
- **THEN** the tool returns `{"key": "nonexistent", "exists": false}`

#### Scenario: Read key from custom project root

- **WHEN** the user calls `config/get` with `{"key": "schema", "project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project/openspec/config.json` contains `{"schema": "spec-driven"}`
- **THEN** the tool returns `{"key": "schema", "value": "spec-driven", "exists": true}`

#### Scenario: Config file does not exist

- **WHEN** the user calls `config/get` with `{"key": "schema"}`
- **AND** the `openspec/config.json` file does not exist
- **AND** `openspec/config.yaml` also does not exist
- **THEN** the tool returns `{"key": "schema", "exists": false}`

#### Scenario: Auto-migration from config.yaml on first read

- **WHEN** the user calls `config/get` with `{"key": "schema"}`
- **AND** `openspec/config.json` does not exist
- **AND** `openspec/config.yaml` exists with valid content
- **THEN** the tool returns `{"key": "schema", "value": "spec-driven", "exists": true}`
- **AND** `openspec/config.json` is created with the migrated data
- **AND** `openspec/config.yaml` is deleted

## ADDED Requirements

### Requirement: config/get gracefully handles invalid config via Zod validation

The read operation SHALL use `safeParseConfig` from `schemas/config.schema` to validate the parsed JSON.
If validation fails (e.g., schema field has wrong type), the tool SHALL still return key path results from the raw parsed data (graceful degradation) rather than throwing an error.
This ensures the tool remains usable even with a corrupted or partially-invalid config file.

#### Scenario: Graceful degradation on invalid JSON

- **WHEN** the user calls `config/get` with `{"key": "schema"}`
- **AND** `openspec/config.json` contains invalid JSON (e.g., `{invalid}`)
- **THEN** the tool returns `{"key": "schema", "exists": false}` without throwing

#### Scenario: Graceful degradation on schema-invalid config

- **WHEN** the user calls `config/get` with `{"key": "schema"}`
- **AND** `openspec/config.json` contains `{"schema": 123, "context": "valid"}` (schema field has wrong type)
- **THEN** the tool still returns a value for `context` (graceful degradation, raw data used)
- **AND** the tool does not throw an error

## Module Contract

### Function: readConfigValue

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `readConfigValue(projectRoot: string, key: string): ConfigGetResult` |
| **Input** | `projectRoot` — project root directory path; `key` — dot-separated key path |
| **Output** | `ConfigGetResult` — `{ key: string; value?: unknown; exists: boolean }` |
| **Behavior** | Reads JSON from `<project_root>/openspec/config.json`, validates with `safeParseConfig`, navigates the key path, returns value or `exists: false`. Falls back gracefully on invalid JSON or missing file. Auto-migrates from `config.yaml` if JSON absent |

### Function: resolveProjectRoot

| Property | Description |
|----------|-------------|
| **Module** | `mcp.ts` |
| **Signature** | `resolveProjectRoot(cwd?: string): string` |
| **Input** | Optional directory hint |
| **Output** | Resolved absolute project root path |
| **Behavior** | Falls back to `CLAUDE_PROJECT_DIR` then `process.cwd()`. Already exists; reused for config tools. (Unchanged) |

### Function: migrateYamlToJson

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` (internal) |
| **Signature** | `migrateYamlToJson(projectRoot: string, jsonPath: string, yamlPath: string): OpenSpecConfig` |
| **Input** | `projectRoot`, `jsonPath`, `yamlPath` — file paths for migration |
| **Output** | `OpenSpecConfig` — validated config object |
| **Behavior** | Dynamically loads `yaml` package via `require('yaml')`, parses YAML, validates with `parseConfig`, writes JSON, deletes YAML. Used only by `readConfig()` |

### API: config/get (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config/get` |
| **Input schema** | `{ key: z.string(), project_root: z.string().optional() }` |
| **Output schema** | `{ key: z.string(), value: z.unknown().optional(), exists: z.boolean() }` |
| **Registration** | `mcp.ts` — `server.registerTool('config/get', ...)` |
| **Handler** | `async (args) => { ... return jsonContent(result); }` |
