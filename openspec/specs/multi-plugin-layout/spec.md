## ADDED Requirements

### Requirement: Plugins directory supports multiple plugins
The repository SHALL support a `plugins/` directory containing multiple plugin subdirectories, each following the standard plugin structure.

#### Scenario: Plugin directory structure
- **WHEN** examining the `plugins/` directory
- **THEN** each subdirectory represents a distinct plugin with its own `.claude-plugin/plugin.json`, `hooks/`, `skills/`, `utils/`, and `templates/`

### Requirement: Plugin manifest contains plugin name
Each plugin's manifest SHALL define a unique `name` field identifying the plugin.

#### Scenario: Dev-team plugin manifest
- **WHEN** reading `plugins/dev-team/.claude-plugin/plugin.json`
- **THEN** the `name` field equals "dev-team"

### Requirement: Plugin hooks reference correct utils path
Hook scripts SHALL reference their plugin's `utils/` directory using the correct path structure.

#### Scenario: Hook imports from plugin utils
- **WHEN** a hook script in `plugins/dev-team/hooks/` executes
- **THEN** it adds `plugins/dev-team/utils/` to `sys.path` before importing utility modules
