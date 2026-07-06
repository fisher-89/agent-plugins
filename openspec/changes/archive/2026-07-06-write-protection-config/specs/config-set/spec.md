## REMOVED Requirements

### Requirement: config_set MCP tool removed

The `config_set` MCP tool (originally `config/set`) SHALL be removed from the system.

**Reason**: The tool provided a generic key-value write interface to `config.json`, but it was never used by any agent, skill, or workflow. The write operations it supported (setting arbitrary keys via dot paths) are better performed through direct file editing, which is more explicit and auditable. Its removal reduces MCP tool surface and maintenance burden.

**Migration**: Users who previously used `config_set` to write config values should directly edit `openspec/config.json` using the `Write` or `Edit` tools. The file uses standard JSON format with 2-space indentation.

### Requirement: config_set command file removed

The command file `plugins/dev-team/bin/src/commands/config-set.ts` SHALL be deleted.

**Reason**: The command implementation is no longer needed after the tool is removed.

**Migration**: No migration needed — the file is an internal implementation detail.

### Requirement: config_set schema files removed

The schema files `plugins/dev-team/bin/src/schemas/config-set.schema.ts` and its export from `schemas/index.ts` SHALL be deleted.

**Reason**: Schema definitions for the removed tool are no longer needed.

**Migration**: No migration needed — the schema files are internal implementation details.

## Module Contract

### Removed: config_set (MCP tool)

| Property | Status |
|----------|--------|
| **Tool name** | `config_set` |
| **Registration** | Removed from `mcp.ts` |
| **Command file** | `commands/config-set.ts` — deleted |
| **Schema file** | `schemas/config-set.schema.ts` — deleted |
| **Index export** | Removed from `schemas/index.ts` |

### Affected files

| File | Action |
|------|--------|
| `plugins/dev-team/bin/src/commands/config-set.ts` | DELETE |
| `plugins/dev-team/bin/src/schemas/config-set.schema.ts` | DELETE |
| `plugins/dev-team/bin/src/schemas/index.ts` | Remove `configSetInputSchema`, `configSetOutputSchema` exports |
| `plugins/dev-team/bin/src/mcp.ts` | Remove `registerConfigSetTool` call and function |
