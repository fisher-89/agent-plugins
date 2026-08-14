## MODIFIED Requirements

### Requirement: Decide mode

The architecture agent SHALL support a decide mode that helps draft ADRs by gathering context and writing via MCP tool `archi_decide` with `action` `create`.
The agent prompt MUST NOT instruct callers to invoke `archi-decide.py` or any `python .../archi-decide.py` command.
List and status-update flows in decide mode SHALL use `archi_decide` with `action` `list` or `update` as needed.

#### Scenario: Create an ADR

- **WHEN** the user asks to record an architectural decision
- **THEN** the agent SHALL gather background from conversation, confirm scope, and invoke MCP `archi_decide` with `action` `create` and the appropriate arguments

#### Scenario: Agent does not call Python ADR script

- **WHEN** the architecture agent runs in decide mode
- **THEN** it MUST NOT call `archi-decide.py`
- **AND** ADR persistence SHALL go through MCP `archi_decide`

## Module Contract

### Component: architecture agent decide mode

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/agents/architecture.md` |
| **Surface** | Decide mode steps and tool inventory |
| **Calls** | MCP `archi_decide` (`create` / `list` / `update`) |
| **MUST NOT** | Reference or shell-out to `utils/archi-decide.py` |
| **Build outputs** | Claude / Cursor / cursor-home-image agent copies refreshed by plugin build |
