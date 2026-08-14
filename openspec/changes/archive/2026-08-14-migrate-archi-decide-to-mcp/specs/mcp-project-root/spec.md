## MODIFIED Requirements

### Requirement: MCP tools require project_root and resolve against candidates

Every MCP tool handler that reads or writes under the project tree (including `phase_log`, `phase_next`, `backtrack`, `change_list`, `config_get`, `archi_query`, `archi_validate`, `archi_write`, `archi_check`, `archi_decide`, `test_detect_frameworks`, and `test_resolve_paths`) SHALL:

1. Declare `project_root` as a **required** string field on the MCP input schema
2. Resolve the effective root via a shared entry point using validation order:
   1. Path validity: absolute, exists on disk, no unexpanded `${...}` → else invalid error
   2. Membership: `project_root` ∈ candidates → clear pending, allow
   3. Pending force: pending key equals `f(toolName, fullArguments)` → add `project_root` to candidates, clear pending, allow
   4. Otherwise: set pending to that key, return error including current candidates and a force hint that resubmitting the **same complete arguments** will force-add and allow

There is NO independent `force` boolean field. Force comparison scope is **tool name + complete arguments** (stable serialization). The server SHALL retain at most one pending entry (most recent). Success or successful force SHALL clear pending.

When candidates are empty, the first valid call MUST be rejected (writes pending); a second call with the same complete arguments SHALL add the path to candidates and allow (empty-candidate escape).

MCP resolution MUST NOT use `process.cwd()`. Output fields MAY echo the effective `project_root` read-only. CLI / command helpers MAY continue to accept `options.projectRoot` / `project_root` outside the MCP external contract; `getProjectDir()` MAY retain cwd fallback for CLI only.

#### Scenario: Member of candidates succeeds on first call

- **WHEN** candidates contain `/workspace/project`
- **AND** `change_list` is invoked with `{"project_root": "/workspace/project"}`
- **THEN** the tool SHALL scan `/workspace/project/openspec/changes/`
- **AND** pending SHALL be cleared if set
- **AND** the returned `project_root` field (when present) SHALL equal `/workspace/project`

#### Scenario: Non-member rejects with candidates and force hint

- **WHEN** candidates are `{/workspace/a, /workspace/b}`
- **AND** `config_get` is invoked with `{"key": "schema", "project_root": "/workspace/other"}` where `/workspace/other` exists
- **THEN** the tool SHALL return an error
- **AND** the error payload SHALL include the candidates
- **AND** the error payload SHALL explain that resubmitting the same complete arguments will force-add the path
- **AND** no config file SHALL be read under `process.cwd()`

#### Scenario: Same full arguments force-adds and allows

- **WHEN** a prior call set pending for tool `change_list` with arguments `{"project_root": "/workspace/other"}`
- **AND** `change_list` is invoked again with exactly the same complete arguments
- **THEN** `/workspace/other` SHALL be added to candidates
- **AND** the tool SHALL execute successfully
- **AND** pending SHALL be cleared

#### Scenario: Different arguments do not force

- **WHEN** pending was set for `change_list` with `{"project_root": "/workspace/other"}`
- **AND** `change_list` is invoked with `{"project_root": "/workspace/third"}`
- **THEN** the call SHALL NOT force-add `/workspace/other`
- **AND** pending SHALL be replaced with the new key
- **AND** the call SHALL error (unless `/workspace/third` ∈ candidates)

#### Scenario: Omitting project_root fails schema validation

- **WHEN** a client calls `phase_log` without `project_root`
- **THEN** input validation SHALL fail
- **AND** no `eval.json` SHALL be written under `process.cwd()`

#### Scenario: Invalid path is rejected before membership

- **WHEN** `project_root` is a relative path, a missing path, or contains `${workspaceFolder}`
- **THEN** the tool SHALL return an invalid-path error
- **AND** pending/force membership logic SHALL NOT treat it as a force confirmation

#### Scenario: Empty candidates first call rejects, second same args allow

- **WHEN** candidates are empty after connect
- **AND** `change_list` is invoked with `{"project_root": "/abs/exists"}` where the path is valid
- **THEN** the first call SHALL error and set pending
- **AND** a second identical full-arguments call SHALL add `/abs/exists` to candidates and succeed

#### Scenario: Multi workspace agent picks listed path

- **WHEN** candidates are `{/ws/app-a, /ws/app-b}`
- **AND** `change_list` is invoked with `{"project_root": "/ws/app-b"}`
- **THEN** the tool SHALL succeed on the first call without force

#### Scenario: Host folder under git root is used as-is

- **WHEN** the host workspace folder is `/repo/packages/pkg` and `/repo/.git` exists
- **AND** `/repo/packages/pkg` is in candidates and passed as `project_root`
- **THEN** the effective root SHALL be `/repo/packages/pkg`
- **AND** the server MUST NOT rewrite it to `/repo`

#### Scenario: CLI fixture projectRoot still works

- **WHEN** `runChangeList({ project_root: "/tmp/fixture" })` is invoked outside MCP tool dispatch
- **THEN** the command helper MAY use `/tmp/fixture` as its project root
- **AND** this SHALL NOT remove required `project_root` from the MCP input schema

#### Scenario: archi_decide requires project_root like other archi tools

- **WHEN** `archi_decide` is invoked without `project_root`
- **THEN** input validation SHALL fail
- **AND** no ADR file SHALL be written under `process.cwd()`

#### Scenario: archi_decide resolves against candidates

- **WHEN** candidates contain `/workspace/project`
- **AND** `archi_decide` is invoked with `{"project_root": "/workspace/project", "action": "list"}`
- **THEN** the tool SHALL list ADRs under `/workspace/project/openspec/architecture/decisions/`
- **AND** pending SHALL be cleared if set

## Module Contract

### Affected MCP tools

| Tool | project_root input | Resolution path |
|------|--------------------|-----------------|
| `phase_log` | Required | candidates / force → `getChangeDir()` |
| `phase_next` | Required | candidates / force → `getChangeDir()` |
| `backtrack` | Required | candidates / force |
| `change_list` | Required | candidates / force；output 可回显 |
| `config_get` | Required | candidates / force |
| `archi_query`, `archi_validate`, `archi_write`, `archi_check`, `archi_decide` | Required | candidates / force |
| `test_detect_frameworks`, `test_resolve_paths` | Required | candidates / force |
