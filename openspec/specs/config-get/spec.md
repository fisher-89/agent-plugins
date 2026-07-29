## Requirements

### Requirement: config_get reads config.json key values

The system SHALL provide a `config_get` MCP tool (formerly `config/get`) that reads values from `openspec/config.json`.
The tool SHALL read and parse the JSON file using Node.js `fs` module and `JSON.parse`.
The tool SHALL use the `safeParseConfig` function from `schemas/config.schema` to validate the config structure; if validation fails, the tool SHALL still return key path results from the raw parsed data (graceful degradation).
The tool SHALL accept an input object with `key` (string, required) and `project_root` (string, required) fields.
The `key` field SHALL support dot-separated nested key paths (e.g., `test_scripts.unit`) to access nested JSON keys.
The tool SHALL resolve the project root through the shared MCP candidate/force entry (e.g. `resolveProjectRootForTool`) using the required `project_root` argument; it MUST NOT use startup lock / `requireLockedProjectRoot()`, and MUST NOT fall back to `process.cwd()`.
The tool SHALL return an output object with `key` (string), `value` (unknown, present only if key exists), and `exists` (boolean) fields.
The tool SHALL resolve the config file path as `<resolved_project_root>/openspec/config.json`.
When the key does not exist in the JSON file, `exists` SHALL be `false` and `value` SHALL be absent from the output.
When the config file does not exist, the tool SHALL return `exists: false` without throwing an error.
When `project_root` fails candidate/force validation, the tool SHALL return a structured error (including candidates / force hint when applicable) and MUST NOT read config relative to `process.cwd()`.
On auto-migration (JSON absent but YAML present), the tool SHALL read the YAML file, validate the content against the Zod schema, write the validated data to `config.json`, and delete the `config.yaml` file before performing the read.

#### Scenario: Read top-level key that exists

- **WHEN** the user calls `config_get` with `{"key": "schema", "project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project` is allowed by candidate/force resolution
- **AND** `/tmp/test-project/openspec/config.json` contains `{"schema": "spec-driven"}`
- **THEN** the tool returns `{"key": "schema", "value": "spec-driven", "exists": true}`

#### Scenario: Read nested key via dot path

- **WHEN** the user calls `config_get` with `{"key": "test_scripts.unit", "project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project` is allowed by candidate/force resolution
- **AND** `/tmp/test-project/openspec/config.json` contains `{"test_scripts": {"unit": "npm test"}}`
- **THEN** the tool returns `{"key": "test_scripts.unit", "value": "npm test", "exists": true}`

#### Scenario: Read non-existent key

- **WHEN** the user calls `config_get` with `{"key": "nonexistent", "project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project` is allowed by candidate/force resolution
- **THEN** the tool returns `{"key": "nonexistent", "exists": false}`

#### Scenario: Uses required project_root from MCP input

- **WHEN** candidates contain `/tmp/test-project`
- **AND** the user calls `config_get` with `{"key": "schema", "project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project/openspec/config.json` contains `{"schema": "spec-driven"}`
- **THEN** the tool returns `{"key": "schema", "value": "spec-driven", "exists": true}`

#### Scenario: Requires project_root in MCP input

- **WHEN** the user calls `config_get` with `{"key": "schema"}` (no `project_root`)
- **THEN** input validation SHALL fail because `project_root` is required

#### Scenario: Config file does not exist

- **WHEN** the user calls `config_get` with `{"key": "schema", "project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project` is allowed by candidate/force resolution
- **AND** the `openspec/config.json` file does not exist
- **AND** `openspec/config.yaml` also does not exist
- **THEN** the tool returns `{"key": "schema", "exists": false}`

#### Scenario: Auto-migration from config.yaml on first read

- **WHEN** the user calls `config_get` with `{"key": "schema", "project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project` is allowed by candidate/force resolution
- **AND** `openspec/config.json` does not exist
- **AND** `openspec/config.yaml` exists with valid content
- **THEN** the tool returns `{"key": "schema", "value": "spec-driven", "exists": true}`
- **AND** `openspec/config.json` is created with the migrated data
- **AND** `openspec/config.yaml` is deleted

#### Scenario: Fails when project_root is not allowed

- **WHEN** `/tmp/other` is not in candidates and is not a pending force confirmation
- **AND** the user calls `config_get` with `{"key": "schema", "project_root": "/tmp/other"}`
- **THEN** the tool SHALL return a structured error including candidates / force hint as applicable
- **AND** MUST NOT read `openspec/config.json` under `process.cwd()`

### Requirement: config_get gracefully handles invalid config via Zod validation

The read operation SHALL use `safeParseConfig` from `schemas/config.schema` to validate the parsed JSON.
If validation fails (e.g., schema field has wrong type), the tool SHALL still return key path results from the raw parsed data (graceful degradation) rather than throwing an error.
This ensures the tool remains usable even with a corrupted or partially-invalid config file.

#### Scenario: Graceful degradation on invalid JSON

- **WHEN** the user calls `config_get` with `{"key": "schema"}`
- **AND** `openspec/config.json` contains invalid JSON (e.g., `{invalid}`)
- **THEN** the tool returns `{"key": "schema", "exists": false}` without throwing

#### Scenario: Graceful degradation on schema-invalid config

- **WHEN** the user calls `config_get` with `{"key": "schema"}`
- **AND** `openspec/config.json` contains `{"schema": 123, "context": "valid"}` (schema field has wrong type)
- **THEN** the tool still returns a value for `context` (graceful degradation, raw data used)
- **AND** the tool does not throw an error

## Module Contract

### API: config_get (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config_get` (formerly `config/get`) |
| **Input schema** | `{ key: z.string(), project_root: z.string() }`（二者均必填） |
| **Output schema** | `{ key: z.string(), value: z.unknown().optional(), exists: z.boolean() }` |
| **Registration** | `mcp.ts` -- `server.registerTool('config_get', ...)` |
| **Handler** | `async (args) => { const projectRoot = resolveProjectRootForTool('config_get', args); ... return jsonContent(result); }` |
| **CLI / command** | `runConfigGet` MAY still accept `options.projectRoot` for tests; MCP 对外契约以必填 `project_root` 为准 |
