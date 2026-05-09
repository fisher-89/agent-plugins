## ADDED Requirements

### Requirement: Root marketplace.json exists
The repository SHALL contain a `marketplace.json` file at the root level for Claude plugin marketplace configuration.

#### Scenario: Marketplace file structure
- **WHEN** examining the repository root
- **THEN** a `marketplace.json` file exists with valid JSON structure

### Requirement: Marketplace registers available plugins
The `marketplace.json` SHALL list all available plugins in the `plugins/` directory with their metadata.

#### Scenario: Dev-team plugin registered
- **WHEN** reading `marketplace.json`
- **THEN** it contains an entry for the "dev-team" plugin with name, description, and path

### Requirement: Marketplace entries match plugin manifests
Each plugin entry in `marketplace.json` SHALL match the corresponding plugin's manifest data.

#### Scenario: Marketplace entry consistency
- **WHEN** a plugin is listed in `marketplace.json`
- **THEN** the entry's name and description match the plugin's `.claude-plugin/plugin.json`
