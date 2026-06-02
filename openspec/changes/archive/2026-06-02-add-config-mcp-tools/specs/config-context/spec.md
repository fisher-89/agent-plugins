## ADDED Requirements

### Requirement: config/context reads project context from config.yaml

The system SHALL provide a `config/context` MCP tool that reads the `context` field from `openspec/config.yaml`.
When called without the `context` input parameter (or with `context` absent/null/undefined), the tool SHALL return the current `context` string value from the config file.
The tool SHALL accept an optional `project_root` parameter (string) to specify the project directory.
The output SHALL be an object with a `context` field containing the string value.

#### Scenario: Read existing context

- **WHEN** the user calls `config/context` with no arguments
- **AND** `openspec/config.yaml` contains `context: "Tech stack: TypeScript, React"`
- **THEN** the tool returns `{"context": "Tech stack: TypeScript, React"}`

#### Scenario: Read context from custom project root

- **WHEN** the user calls `config/context` with `{"project_root": "/tmp/test-project"}`
- **AND** `/tmp/test-project/openspec/config.yaml` contains `context: "Tech stack: Python, Django"`
- **THEN** the tool returns `{"context": "Tech stack: Python, Django"}`

#### Scenario: Read context when context key is missing

- **WHEN** the user calls `config/context` with no arguments
- **AND** `openspec/config.yaml` does not contain a `context` key
- **THEN** the tool returns `{"context": ""}` (empty string)

#### Scenario: Read context when config file is missing

- **WHEN** the user calls `config/context` with no arguments
- **AND** `openspec/config.yaml` does not exist
- **THEN** the tool returns `{"context": ""}` (empty string)

### Requirement: config/context writes project context to config.yaml

When called with a `context` string parameter, the tool SHALL write the provided value to the `context` field in `openspec/config.yaml`.
The tool SHALL create the file with a minimal skeleton if it does not exist.
The output SHALL be an object with `context` (string, the written value) and `written` (boolean, `true`) fields.
The tool SHALL preserve all other keys and comments in the YAML file.

#### Scenario: Write new context

- **WHEN** the user calls `config/context` with `{"context": "Tech stack: Node.js, TypeScript"}`
- **AND** `openspec/config.yaml` contains existing keys
- **THEN** the tool returns `{"context": "Tech stack: Node.js, TypeScript", "written": true}`
- **AND** the `context` field in the YAML file is updated to the new value
- **AND** all other keys in the file are preserved

#### Scenario: Overwrite existing context

- **WHEN** the user calls `config/context` with `{"context": "New context value"}`
- **AND** `openspec/config.yaml` already has a `context` field
- **THEN** the tool returns `{"context": "New context value", "written": true}`
- **AND** only the `context` field is updated

#### Scenario: Write context with custom project root

- **WHEN** the user calls `config/context` with `{"context": "Custom context", "project_root": "/tmp/test-project"}`
- **THEN** the tool writes the context to `/tmp/test-project/openspec/config.yaml`

## Module Contract

### Function: readContext

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `readContext(projectRoot: string): { context: string }` |
| **Input** | `projectRoot` — project root directory path |
| **Output** | `{ context: string }` — context value (empty string if absent) |
| **Behavior** | Reads YAML, extracts `context` field, returns empty string if missing |

### Function: writeContext

| Property | Description |
|----------|-------------|
| **Module** | `lib/config.ts` |
| **Signature** | `writeContext(projectRoot: string, context: string): { context: string; written: boolean }` |
| **Input** | `projectRoot` — project root directory path; `context` — new context string value |
| **Output** | `{ context: string; written: boolean }` |
| **Behavior** | Reads or creates YAML, sets `context` field, serializes back, returns written value |

### API: config/context (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `config/context` |
| **Input schema** | `{ context: z.string().optional(), project_root: z.string().optional() }` |
| **Output schema** | `{ context: z.string(), written: z.boolean().optional() }` |
| **Registration** | `mcp.ts` — `server.registerTool('config/context', ...)` |
| **Handler** | `async (args) => { ... return jsonContent(result); }` |
