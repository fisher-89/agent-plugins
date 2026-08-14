## Purpose

Defines how the dev-team MCP server discovers workspace root candidates at connect time and resolves the `project_root` argument required by every file-touching MCP tool — rejecting non-candidate paths with a force-escape flow, converting `file://` URIs to local paths, and never falling back to `process.cwd()`.

## Requirements

### Requirement: Connect collects workspace root candidates

After `server.connect(transport)` completes, the dev-team MCP server SHALL collect a deduplicated set of workspace root **candidates** from all usable host channels. Collection MUST NOT select a default project root, MUST NOT distinguish `len == 1` vs `len > 1` for locking, and MUST NOT call `requireLockedProjectRoot` semantics.

Channels (union, dedupe):

1. `process.env.CLAUDE_PROJECT_DIR` when it is a non-empty, absolute filesystem path that exists on disk and does NOT contain an unexpanded `${...}` literal
2. `process.env.WORKSPACE_FOLDER_PATHS` when splitting on `,` or `;` (trim empty segments): **every** usable absolute existing path enters candidates
3. MCP `roots/list` when the client declares the `roots` capability and the request succeeds: **every** usable `file://` (or bare absolute) root enters candidates; failure / method-not-found / empty MUST NOT block other channels

The server MUST NOT use `process.cwd()` as a candidate source for MCP. The server MUST NOT walk upward looking for `.git` or `openspec/` to rewrite a path. The server SHALL NOT register a `notifications/roots/list_changed` handler.

#### Scenario: Multiple channels merge and dedupe

- **WHEN** `CLAUDE_PROJECT_DIR` is `/workspace/a`
- **AND** `WORKSPACE_FOLDER_PATHS` is `/workspace/a,/workspace/b`
- **AND** `listRoots()` returns a usable root for `/workspace/b`
- **THEN** candidates SHALL be the set `{/workspace/a, /workspace/b}` (order unspecified)
- **AND** the server SHALL NOT lock a single default root

#### Scenario: Single usable path is still only a candidate

- **WHEN** only one usable path exists across all channels (e.g. sole `CLAUDE_PROJECT_DIR`)
- **THEN** that path SHALL be present in candidates
- **AND** MCP tools MUST still require an explicit `project_root` argument (no omit-and-default)

#### Scenario: Unexpanded CLAUDE_PROJECT_DIR literal is rejected from candidates

- **WHEN** `process.env.CLAUDE_PROJECT_DIR` is the literal string `${workspaceFolder}` or `${CLAUDE_PROJECT_DIR}`
- **THEN** that value SHALL NOT enter candidates
- **AND** collection SHALL continue with other channels

#### Scenario: Comma-separated WORKSPACE_FOLDER_PATHS all enter candidates

- **WHEN** `WORKSPACE_FOLDER_PATHS` is `d:\Projects\a,d:\Projects\b` and both exist as absolute paths
- **THEN** candidates SHALL include both paths
- **AND** the server MUST NOT treat the entire comma-joined string as one path
- **AND** the server MUST NOT guess only `[0]`

#### Scenario: roots/list failure does not wipe other candidates

- **WHEN** `CLAUDE_PROJECT_DIR` already contributed a usable path
- **AND** `listRoots()` throws or returns `-32601 Method not found`
- **THEN** candidates SHALL still contain the CLAUDE path
- **AND** the MCP server SHALL continue to accept tool requests

#### Scenario: Empty candidates after connect

- **WHEN** no channel yields a usable path
- **THEN** candidates MAY be empty
- **AND** MCP tools MUST NOT fall back to `process.cwd()`
- **AND** empty-candidate force escape (see resolve requirement) SHALL apply

#### Scenario: Shared mcp.json has no workspaceFolder literal

- **WHEN** the shared plugin `.mcp.json` for `dev-team` is inspected
- **THEN** it MUST NOT set `CLAUDE_PROJECT_DIR` to an unexpanded `${...}` literal
- **AND** it MUST NOT use `${workspaceFolder}` in a way that breaks Claude Code startup

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

### Requirement: file URI to local path conversion for candidates

The server SHALL convert MCP root `uri` values from `file://` scheme to platform-local absolute paths before adding them to candidates.

Conversion SHALL handle:

- Unix-style URIs: `file:///home/user/project`
- Windows-style URIs: `file:///D:/Projects/wps-claude-plugin` and `file:///D:/Projects/wps-claude-plugin/`

When a client returns a bare absolute filesystem path instead of a `file://` URI, the server MAY accept it if it is an existing absolute path.

The conversion function SHALL produce normalized absolute paths suitable for use with Node.js `path.join` and `fs` operations.

#### Scenario: Windows file URI is converted correctly

- **WHEN** a root URI is `file:///D:/Projects/wps-claude-plugin`
- **THEN** the candidate path SHALL resolve to `D:\Projects\wps-claude-plugin` (or platform-normalized equivalent)

#### Scenario: Unix file URI is converted correctly

- **WHEN** a root URI is `file:///home/user/projects/my-app`
- **THEN** the candidate path SHALL resolve to `/home/user/projects/my-app`

## Module Contract

### Function: initProjectRootFromMcp（或 collectCandidates 等价入口）

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts` |
| **Signature** | `initProjectRootFromMcp(server: Server): Promise<void>`（名称可随实现调整，职责为采集） |
| **Input** | `server` — underlying MCP `Server` instance from `McpServer.server` |
| **Output** | `void`（side effect: 填充 candidates；不锁定默认根） |
| **Behavior** | 合并去重 CLAUDE ∪ WORKSPACE ∪ roots；不 exit；不注册 `list_changed`；禁用 cwd |

### Function: resolveProjectRootForTool（名称可随实现调整）

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts` |
| **Signature** | `resolveProjectRootForTool(toolName: string, args: Record<string, unknown>): string` |
| **Input** | tool 名 + 完整 arguments（含必填 `project_root`） |
| **Output** | 绝对路径（放行时） |
| **Behavior** | 合法 → ∈ 候选 → pending force → 否则写 pending 并抛错/返回结构化错误；MCP MUST NOT 回退 cwd |

### Function: getProjectDir

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts` |
| **Signature** | `getProjectDir(): string` |
| **Input** | None |
| **Output** | Absolute filesystem path |
| **Behavior** | CLI / command / hooks 辅助入口；MAY 保留 cwd 回退。MCP tool handlers MUST NOT 依赖此函数的 cwd 回退作为项目根契约 |

### Function: fileUriToPath

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts` |
| **Signature** | `fileUriToPath(uri: string): string`（实现细节，可不 export） |
| **Input** | MCP root `uri` string |
| **Output** | Platform-local absolute filesystem path |
| **Behavior** | Parses and normalizes `file://` URIs for Windows and Unix |

### Removed APIs

| API | Status |
|-----|--------|
| `requireLockedProjectRoot()` | **REMOVED** — 由 resolve-against-candidates 入口替代 |
| Immutable `mcpProjectRootCache` lock | **REMOVED** — 改为 candidates + pending |

### Module layout

| Path | Status |
|------|--------|
| `lib/project-root.ts` | Owns candidates、pending/force、URI helpers、`getProjectDir` |
| `mcp.ts` | Post-connect collect；handlers 使用必填 `args.project_root` |

### MCP Server lifecycle (`plugins/dev-team/bin/src/mcp.ts`)

| Step | Description |
|------|-------------|
| `main()` | Creates `McpServer`, registers tools, connects transport |
| Post-connect | Collect candidates only（无默认根锁定） |
| Tool handlers | Validate required `project_root` via resolve entry |
| Notifications | MUST NOT register `roots/list_changed` |

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
