## REMOVED Requirements

### Requirement: config_unset MCP tool removed

The `config_unset` MCP tool (originally `config/unset`) SHALL be removed from the system.

**Reason**: The tool provided a generic key deletion interface to `config.json`, but it was never used by any agent, skill, or workflow. The removal complements the removal of `config_set` — both tools existed as a pair for arbitrary config manipulation that was never leveraged by the actual workflow. Direct file editing is the preferred approach.

**Migration**: Users who previously used `config_unset` to delete config keys should directly edit `openspec/config.json` using the `Edit` tool or manually remove the key.

### Requirement: config_unset command file removed

The command file `plugins/dev-team/bin/src/commands/config-unset.ts` SHALL be deleted.

**Reason**: The command implementation is no longer needed after the tool is removed.

**Migration**: No migration needed — the file is an internal implementation detail.

### Requirement: config_unset schema files removed

The schema files `plugins/dev-team/bin/src/schemas/config-unset.schema.ts` and its export from `schemas/index.ts` SHALL be deleted.

**Reason**: Schema definitions for the removed tool are no longer needed.

**Migration**: No migration needed — the schema files are internal implementation details.

## Module Contract

### Removed: config_unset (MCP tool)

| Property | Status |
|----------|--------|
| **Tool name** | `config_unset` |
| **Registration** | Removed from `mcp.ts` |
| **Command file** | `commands/config-unset.ts` — deleted |
| **Schema file** | `schemas/config-unset.schema.ts` — deleted |
| **Index export** | Removed from `schemas/index.ts` |

### Affected files

| File | Action |
|------|--------|
| `plugins/dev-team/bin/src/commands/config-unset.ts` | DELETE |
| `plugins/dev-team/bin/src/schemas/config-unset.schema.ts` | DELETE |
| `plugins/dev-team/bin/src/schemas/index.ts` | Remove `configUnsetInputSchema`, `configUnsetOutputSchema` exports |
| `plugins/dev-team/bin/src/mcp.ts` | Remove `registerConfigUnsetTool` call and function |
