## REMOVED Requirements

### Requirement: config_context MCP tool removed

The `config_context` MCP tool (originally `config/context`) SHALL be removed from the system.

**Reason**: The tool duplicates functionality that is already covered by `config_get` (reading the context field) and manual editing of `config.json`. It was never used by any agent, skill, or workflow. Its existence added to MCP tool surface complexity without providing value beyond what the generic `config_get` offers.

**Migration**: Users who previously used `config_context` to read or write the context field can use `config_get` with key `"context"` to read, and directly edit `openspec/config.json` to write. The `config_get` tool preserves read access to all config fields including `context`.

### Requirement: config_context command file removed

The command file `plugins/dev-team/bin/src/commands/config-context.ts` SHALL be deleted.

**Reason**: The command implementation is no longer needed after the tool is removed.

**Migration**: No migration needed — the file is an internal implementation detail.

### Requirement: config_context schema files removed

The schema files `plugins/dev-team/bin/src/schemas/config-context.schema.ts` and its export from `schemas/index.ts` SHALL be deleted.

**Reason**: Schema definitions for the removed tool are no longer needed.

**Migration**: No migration needed — the schema files are internal implementation details.

## Module Contract

### Removed: config_context (MCP tool)

| Property | Status |
|----------|--------|
| **Tool name** | `config_context` |
| **Registration** | Removed from `mcp.ts` |
| **Command file** | `commands/config-context.ts` — deleted |
| **Schema file** | `schemas/config-context.schema.ts` — deleted |
| **Index export** | Removed from `schemas/index.ts` |

### Affected files

| File | Action |
|------|--------|
| `plugins/dev-team/bin/src/commands/config-context.ts` | DELETE |
| `plugins/dev-team/bin/src/schemas/config-context.schema.ts` | DELETE |
| `plugins/dev-team/bin/src/schemas/index.ts` | Remove `configContextInputSchema`, `configContextOutputSchema` exports |
| `plugins/dev-team/bin/src/mcp.ts` | Remove `registerConfigContextTool` call and function |
