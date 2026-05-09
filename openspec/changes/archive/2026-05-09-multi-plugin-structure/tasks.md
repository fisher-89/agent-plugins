## 1. Directory Restructure

- [x] 1.1 Create `plugins/dev-team/` directory structure
- [x] 1.2 Move `plugin/hooks/` to `plugins/dev-team/hooks/`
- [x] 1.3 Move `plugin/skills/` to `plugins/dev-team/skills/`
- [x] 1.4 Move `plugin/utils/` to `plugins/dev-team/utils/`
- [x] 1.5 Move `plugin/templates/` to `plugins/dev-team/templates/`
- [x] 1.6 Move `plugin/agents/` to `plugins/dev-team/agents/`
- [x] 1.7 Move `plugin/.claude-plugin/` to `plugins/dev-team/.claude-plugin/`
- [x] 1.8 Remove empty `plugin/` directory

## 2. Plugin Manifest Update

- [x] 2.1 Update plugin name from "wps" to "dev-team" in `plugins/dev-team/.claude-plugin/plugin.json`

## 3. Path Reference Updates

- [x] 3.1 Update `sys.path` modifications in all hook scripts to use `plugins/dev-team/utils/`
- [x] 3.2 Update any hardcoded `plugin/` paths in utility modules
- [x] 3.3 Update any hardcoded `plugin/` paths in hook configuration files

## 4. Marketplace Configuration

- [x] 4.1 Create root `marketplace.json` with Claude plugin marketplace configuration
- [x] 4.2 Register dev-team plugin in marketplace.json

## 5. Documentation Updates

- [x] 4.1 Update CLAUDE.md with new directory structure paths
- [x] 4.2 Update any path references in README or other documentation files
