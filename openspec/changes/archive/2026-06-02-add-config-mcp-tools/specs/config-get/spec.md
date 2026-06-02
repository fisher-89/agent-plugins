## ADDED Requirements

### Requirement: config/get reads config.yaml key values

The system SHALL provide a `config/get` MCP tool that reads values from `openspec/config.yaml`.
The tool SHALL accept an input object with `key` (string, required) and `project_root` (string, optional) fields.
The `key` field SHALL support dot-separated nested key paths (e.g., `test_scripts.unit`) to access nested YAML keys.
The `project_root` field SHALL default to `process.env.CLAUDE_PROJECT_DIR` or `process.cwd()` when not provided.
The tool SHALL return an output object with `key` (string), `value` (unknown, present only if key exists), and `exists` (boolean) fields.
The tool SHALL resolve the config file path as `<project_root>/openspec/config.yaml`.
When the key does not exist in the YAML file, `exists` SHALL be `false` and `value` SHALL be absent from the output.
When the config file does not exist, the tool SHALL return `exists: false` without throwing an error.

#### Scenario: Read top-level key that exists

- **WHEN** the user calls `config/get` with `{"key": "schema"}`
- **AND** `openspec/config.yaml` contains `schema: spec-driven`
- **THEN** the tool returns `{"key": "schema", "value": "spec-driven", "exists": true}`

#### Scenario: Read nested key via dot path

- **WHEN** the user calls `config/get` with `{"key": "test_scripts.unit"}`
- **AND** `openspec/config.yaml` contains `test_scripts: {unit: "npm test"}`
- **THEN** the tool returns `{"key": "test_scripts.unit", "value": "npm test", "exists": true}`

#### Scenario: Read non-existent key

- **WHEN** the user calls `config/get` with `{"key": "nonexistent"}`
- **THEN** the tool returns `{"key": "nonexistent", "exists": false}`

#### Scenario: Read key from custom project root

- **WHEN** the user calls `config/get` with `{"key": "schema", "project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project/openspec/config.yaml` contains `schema: spec-driven`
- **THEN** the tool returns `{"key": "schema", "value": "spec-driven", "exists": true}`

#### Scenario: Config file does not exist

- **WHEN** the user calls `config/get` with `{"key": "schema"}`
- **AND** the `openspec/config.yaml` file does not exist
- **THEN** the tool returns `{"key": "schema", "exists": false}`

### Requirement: config/get resolves project_root from environment

When `project_root` is not provided, the tool SHALL use `process.env.CLAUDE_PROJECT_DIR` if set, otherwise `process.cwd()`.
This matches the existing pattern in `resolveProjectRoot()` from `mcp.ts`.

#### Scenario: Use CLAUDE_PROJECT_DIR env var

- **WHEN** `CLAUDE_PROJECT_DIR` is set to `/env/project`
- **AND** the user calls `config/get` with `{"key": "schema"}` (no `project_root`)
- **THEN** the tool reads from `/env/project/openspec/config.yaml`

## Module Contract

### Function: readConfigValue

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `readConfigValue(projectRoot: string, key: string): ConfigGetResult` |
| **Input** | `projectRoot` — project root directory path; `key` — dot-separated key path |
| **Output** | `ConfigGetResult` — `{ key: string; value?: unknown; exists: boolean }` |
| **Behavior** | Loads YAML from `<project_root>/openspec/config.yaml`, navigates the key path, returns value or `exists: false` |

### Function: resolveProjectRoot

| Property | Description |
|----------|-------------|
| **Module** | `mcp.ts` |
| **Signature** | `resolveProjectRoot(cwd?: string): string` |
| **Input** | Optional directory hint |
| **Output** | Resolved absolute project root path |
| **Behavior** | Falls back to `CLAUDE_PROJECT_DIR` then `process.cwd()`. Already exists; reused for config tools. |

### API: config/get (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config/get` |
| **Input schema** | `{ key: z.string(), project_root: z.string().optional() }` |
| **Output schema** | `{ key: z.string(), value: z.unknown().optional(), exists: z.boolean() }` |
| **Registration** | `mcp.ts` — `server.registerTool('config/get', ...)` |
| **Handler** | `async (args) => { ... return jsonContent(result); }` |
