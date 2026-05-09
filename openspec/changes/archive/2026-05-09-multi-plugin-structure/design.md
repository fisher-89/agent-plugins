## Context

The current plugin structure uses a single `plugin/` directory containing all plugin code (hooks, skills, utils, templates). This works for a single plugin but doesn't scale to multiple plugins developed by different teams.

Current structure:
```
plugin/
├── .claude-plugin/plugin.json
├── hooks/
├── skills/
├── utils/
└── templates/
```

Target structure:
```
plugins/
└── dev-team/
    ├── .claude-plugin/plugin.json
    ├── hooks/
    ├── skills/
    ├── utils/
    └── templates/
marketplace.json
```

## Goals / Non-Goals

**Goals:**
- Restructure to support multiple plugins in `plugins/` directory
- Rename plugin from "wps" to "dev-team"
- Add marketplace.json for Claude plugin marketplace integration
- Maintain all existing functionality without behavior changes
- Update all path references to work with new structure

**Non-Goals:**
- Plugin discovery or loading mechanism (future work)
- Plugin dependency management between plugins
- Multiple plugin activation in single session

## Decisions

### Directory Structure
- **Decision**: Use `plugins/<plugin-name>/` pattern
- **Rationale**: Follows common patterns (e.g., VS Code extensions, Node modules). Each plugin is self-contained with its own manifest, hooks, skills, and utilities.
- **Alternative**: Use `plugin-<name>/` flat structure - rejected as less scalable

### Plugin Naming
- **Decision**: Rename "wps" to "dev-team"
- **Rationale**: More descriptive name indicating the plugin's purpose (development team workflow). Avoids vendor-specific naming.
- **Alternative**: Keep "wps" - rejected as not descriptive

### Path Migration Strategy
- **Decision**: Update all hardcoded paths atomically
- **Rationale**: Prevents broken intermediate states. All references must be updated together.
- **Files affected**:
  - Hook scripts: `sys.path` modifications, file imports
  - CLAUDE.md: Documentation paths
  - Any template references

### Marketplace Configuration
- **Decision**: Add root `marketplace.json` to register available plugins
- **Rationale**: Enables Claude plugin marketplace integration. Provides a single source of truth for plugin discovery.
- **Structure**:
  ```json
  {
    "plugins": [
      {
        "name": "dev-team",
        "path": "plugins/dev-team",
        "description": "...",
        "version": "..."
      }
    ]
  }
  ```
- **Alternative**: Per-plugin marketplace metadata files - rejected as less centralized

## Risks / Trade-offs

- **Risk**: External consumers expecting `plugin/` path
  → Mitigation: This is internal refactoring; no external API contracts exist

- **Risk**: Hook scripts may have hardcoded `plugin/` paths
  → Mitigation: Grep for all occurrences and update systematically

- **Trade-off**: Longer import paths in code
  → Acceptable since Python handles this via `sys.path` setup
