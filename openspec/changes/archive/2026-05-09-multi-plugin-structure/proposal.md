## Why

Current plugin structure only supports a single plugin in `plugin/` directory. To enable plugin composability and allow multiple teams to develop their own plugins independently, we need a multi-plugin directory structure.

## What Changes

- **BREAKING**: Move `plugin/` to `plugins/dev-team/`
- Rename plugin from `wps` to `dev-team`
- Add root `marketplace.json` for Claude plugin marketplace configuration
- Update all path references throughout codebase
- Update CLAUDE.md documentation
- Update any hardcoded paths in hook scripts and utilities

## Capabilities

### New Capabilities

- `multi-plugin-layout`: Support for multiple plugins in `plugins/` directory, each with its own manifest, hooks, skills, and utilities
- `marketplace-config`: Root marketplace.json for Claude plugin marketplace integration

### Modified Capabilities

- `hook-output-import`: Update path references from `plugin/utils/` to `plugins/dev-team/utils/`

## Impact

- All existing plugin code moves from `plugin/` to `plugins/dev-team/`
- Plugin manifest name changes from "wps" to "dev-team"
- New `marketplace.json` at repository root for marketplace integration
- All import paths and sys.path modifications in hooks need updating
- CLAUDE.md documentation requires path updates
- No runtime behavior changes - purely structural refactoring
