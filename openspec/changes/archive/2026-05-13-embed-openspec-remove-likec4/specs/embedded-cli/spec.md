## ADDED Requirements

### Requirement: Plugin bundles openspec CLI in bin directory
The plugin SHALL include the `@fission-ai/openspec` npm package installed at `plugins/dev-team/bin/node_modules/@fission-ai/openspec/`, with a wrapper executable at `plugins/dev-team/bin/openspec` that forwards CLI arguments.

#### Scenario: openspec wrapper forwards commands
- **WHEN** the wrapper is invoked with arguments (e.g., `openspec list --json`)
- **THEN** it SHALL execute `node <bin>/node_modules/@fission-ai/openspec/bin/openspec.js` with the same arguments
- **AND** exit with the same exit code

#### Scenario: openspec CLI not found triggers auto-install
- **WHEN** `plugins/dev-team/bin/node_modules/@fission-ai/openspec/` does not exist
- **THEN** the SessionStart hook SHALL attempt to run `npm install @fission-ai/openspec@<version>` in `plugins/dev-team/bin/`
- **AND** report success or failure to the user

### Requirement: SessionStart hook checks embedded openspec instead of global
The SessionStart hook SHALL verify the embedded openspec at `plugins/dev-team/bin/node_modules/@fission-ai/openspec/` is available, rather than checking for a globally installed `openspec` command.

#### Scenario: Embedded openspec is ready
- **WHEN** the wrapper at `plugins/dev-team/bin/openspec` exists and is executable
- **THEN** the hook SHALL pass without issuing any warning

#### Scenario: Embedded openspec not found and auto-install fails
- **WHEN** the embedded directory does not exist and `npm install` fails (network error, permission, etc.)
- **THEN** the hook SHALL output a clear error message with manual installation instructions
