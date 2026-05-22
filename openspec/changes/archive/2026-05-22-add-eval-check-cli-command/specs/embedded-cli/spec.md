## ADDED Requirements

### Requirement: CLI registers eval-check subcommand
The TypeScript CLI SHALL register an `eval-check` subcommand via `registerEvalCheckCommand(cli)` in `bin/src/index.ts`.
The subcommand SHALL be registered using the `cac` declarative API pattern consistent with `eval-log`:
- `.command("eval-check", "...")` for the subcommand definition
- `.option("--change <name>", "...")` for the change name (required)
- `.option("--phase <phase>", "...")` for the phase identifier (required)
- `.option("--json", "...")` for structured JSON output (optional)

#### Scenario: eval-check appears in help output
- **WHEN** `dev-team --help` is executed
- **THEN** the output SHALL include "eval-check" in the list of available commands

#### Scenario: eval-check accepts required options
- **WHEN** `dev-team eval-check --change my-change --phase 03-dev-proposal` is invoked
- **THEN** the command SHALL parse --change and --phase and pass them to the action handler

#### Scenario: eval-check --json flag is optional
- **WHEN** `dev-team eval-check --change my-change --phase 03-dev-proposal --json` is invoked
- **THEN** the command SHALL pass the --json flag to the action handler for structured output
