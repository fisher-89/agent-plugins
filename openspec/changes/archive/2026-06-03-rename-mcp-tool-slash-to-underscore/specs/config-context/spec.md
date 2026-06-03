## MODIFIED Requirements

### Requirement: config_context reads project context from config.json

The system SHALL provide a `config_context` MCP tool (formerly `config/context`) that reads the `context` field from `openspec/config.json`.
The tool SHALL read and parse the JSON file using Node.js `fs` module and `JSON.parse`.
The tool SHALL use the `safeParseConfig` function from `schemas/config.schema` to validate the full config structure and return a typed `OpenSpecConfig` object.
When called without the `context` input parameter (or with `context` absent/null/undefined), the tool SHALL return the current `context` string value from the config file.
The tool SHALL accept an optional `project_root` parameter (string) to specify the project directory.
The output SHALL be an object with a `context` field containing the string value.
If `config.json` does not exist, the tool SHALL attempt to read `config.yaml` instead. If `config.yaml` exists, the tool SHALL migrate it to `config.json` (read YAML, validate with Zod, write JSON, delete YAML) before returning the context value.

#### Scenario: Read existing context from config.json

- **WHEN** the user calls `config_context` with no arguments
- **AND** `openspec/config.json` contains `{"schema": "spec-driven", "context": "Tech stack: TypeScript, React"}`
- **THEN** the tool returns `{"context": "Tech stack: TypeScript, React"}`

#### Scenario: Read context from custom project root

- **WHEN** the user calls `config_context` with `{"project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project/openspec/config.json` contains `{"schema": "spec-driven", "context": "Tech stack: Python, Django"}`
- **THEN** the tool returns `{"context": "Tech stack: Python, Django"}`

#### Scenario: Read context when context key is missing

- **WHEN** the user calls `config_context` with no arguments
- **AND** `openspec/config.json` contains `{"schema": "spec-driven"}` (no `context` key)
- **THEN** the tool returns `{"context": ""}` (empty string)

#### Scenario: Read context when config file is missing

- **WHEN** the user calls `config_context` with no arguments
- **AND** `openspec/config.json` does not exist
- **AND** `openspec/config.yaml` also does not exist
- **THEN** the tool returns `{"context": ""}` (empty string)

#### Scenario: Auto-migrate from config.yaml when config.json missing

- **WHEN** the user calls `config_context` with no arguments
- **AND** `openspec/config.json` does not exist
- **AND** `openspec/config.yaml` exists with `context: "Legacy context"`
- **THEN** the tool returns `{"context": "Legacy context"}`
- **AND** `openspec/config.json` is created with the migrated data
- **AND** `openspec/config.yaml` is deleted

### Requirement: config_context writes project context to config.json

When called with a `context` string parameter, the tool SHALL write the provided value to the `context` field in `openspec/config.json`.
The tool SHALL validate the full config object through `parseConfig` before writing.
The tool SHALL create the file with a minimal skeleton (`{"schema": "spec-driven"}`) if it does not exist.
The output SHALL be an object with `context` (string, the written value) and `written` (boolean, `true`) fields.
The tool SHALL preserve all other keys in the JSON file.

#### Scenario: Write new context

- **WHEN** the user calls `config_context` with `{"context": "Tech stack: Node.js, TypeScript"}`
- **AND** `openspec/config.json` contains existing keys
- **THEN** the tool returns `{"context": "Tech stack: Node.js, TypeScript", "written": true}`
- **AND** the `context` field in `config.json` is updated to the new value
- **AND** all other keys in the file are preserved

#### Scenario: Overwrite existing context

- **WHEN** the user calls `config_context` with `{"context": "New context value"}`
- **AND** `openspec/config.json` already has a `context` field
- **THEN** the tool returns `{"context": "New context value", "written": true}`
- **AND** only the `context` field is updated

#### Scenario: Write context with custom project root

- **WHEN** the user calls `config_context` with `{"context": "Custom context", "project_root": "/tmp/test-project"}`
- **THEN** the tool writes the context to `/tmp/test-project/openspec/config.json`

#### Scenario: Zod validation on context write with invalid config

- **WHEN** the user calls `config_context` with `{"context": "New context"}`
- **AND** `config.json` already contains an invalid value that cannot pass `parseConfig`
- **THEN** the tool returns a validation error and does not modify the file

## Module Contract

### API: config_context (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config_context` (formerly `config/context`) |
| **Input schema** | `{ context: z.string().optional(), project_root: z.string().optional() }` |
| **Output schema** | `{ context: z.string(), written: z.boolean().optional() }` |
| **Registration** | `mcp.ts` -- `server.registerTool('config_context', ...)` |
| **Handler** | `async (args) => { ... return jsonContent(result); }` |
