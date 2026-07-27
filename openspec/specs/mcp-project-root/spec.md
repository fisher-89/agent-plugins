## Requirements

### Requirement: Startup lock resolves a unique project root

The dev-team MCP server SHALL resolve and lock the workspace project root once after `server.connect(transport)` completes, using the following priority order. The first successful channel SHALL be cached for the process lifetime and MUST NOT change afterward.

1. `process.env.CLAUDE_PROJECT_DIR` when it is a non-empty, absolute filesystem path that exists on disk and does NOT contain an unexpanded `${...}` literal
2. MCP `roots/list` when the client declares the `roots` capability, the request succeeds, and it yields **exactly one** usable `file://` (or absolute path) root
3. `process.env.WORKSPACE_FOLDER_PATHS` when splitting on `,` or `;` (trim empty segments) yields **exactly one** absolute existing path

If none of the channels yields a unique usable root, the server SHALL leave the lock unset (strict failure). The server MUST NOT fall back to `process.cwd()` for MCP tool path resolution. When a channel yields multiple candidates (e.g. multi-value `WORKSPACE_FOLDER_PATHS`, or `roots/list` with more than one usable root and no higher-priority unique source), the server MUST NOT select `[0]` by guessing.

#### Scenario: CLAUDE_PROJECT_DIR absolute path locks first

- **WHEN** `process.env.CLAUDE_PROJECT_DIR` is set to an existing absolute path `/workspace/my-project`
- **AND** `roots/list` would return a different root
- **THEN** the locked project root SHALL be `/workspace/my-project`

#### Scenario: Unexpanded CLAUDE_PROJECT_DIR literal is rejected

- **WHEN** `process.env.CLAUDE_PROJECT_DIR` is the literal string `${workspaceFolder}` or `${CLAUDE_PROJECT_DIR}`
- **THEN** that channel SHALL NOT lock
- **AND** resolution SHALL continue to roots / `WORKSPACE_FOLDER_PATHS` or strict failure

#### Scenario: Exactly one MCP root locks when env unavailable

- **WHEN** `CLAUDE_PROJECT_DIR` is unset or unusable
- **AND** the client declares `roots` capability
- **AND** `listRoots()` returns exactly one usable root with URI `file:///D:/Projects/wps-claude-plugin`
- **THEN** the locked project root SHALL be the local absolute path equivalent to `D:\Projects\wps-claude-plugin`

#### Scenario: Multiple MCP roots do not guess first entry

- **WHEN** `CLAUDE_PROJECT_DIR` is unset or unusable
- **AND** `listRoots()` returns two or more usable roots
- **THEN** the roots channel SHALL NOT lock
- **AND** the server MUST NOT cache `roots[0]` from that result

#### Scenario: Single WORKSPACE_FOLDER_PATHS entry locks as fallback

- **WHEN** `CLAUDE_PROJECT_DIR` is unset or unusable
- **AND** roots are unavailable or fail
- **AND** `process.env.WORKSPACE_FOLDER_PATHS` is `d:\Projects\wps-claude-plugin`
- **THEN** the locked project root SHALL be that absolute path (platform-normalized)

#### Scenario: Comma-separated WORKSPACE_FOLDER_PATHS is split

- **WHEN** `CLAUDE_PROJECT_DIR` is unset or unusable
- **AND** roots are unavailable or fail
- **AND** `process.env.WORKSPACE_FOLDER_PATHS` is `d:\Projects\a,d:\Projects\b`
- **THEN** the server SHALL parse two candidates
- **AND** the lock SHALL remain unset (strict failure)
- **AND** the server MUST NOT treat the entire comma-joined string as one path

#### Scenario: No unique root does not fall back to cwd

- **WHEN** no channel yields a unique usable root
- **THEN** the MCP project root lock SHALL remain unset
- **AND** MCP tools MUST NOT resolve paths via `process.cwd()`

### Requirement: MCP tool handlers require a locked project root

Every MCP tool handler that reads or writes under the project tree (including `phase_log`, `phase_next`, `backtrack`, `change_list`, `config_get`, `archi_query`, `archi_validate`, `archi_write`, `archi_check`, `test_detect_frameworks`, and `test_resolve_paths`) SHALL obtain the project root through a shared locked-root entry point (e.g. `requireLockedProjectRoot()`).

When the lock is unset, the handler SHALL return a structured, observable error describing why resolution failed (e.g. missing unique root, multi-root ambiguity, rejected `${...}` literal) and MUST NOT perform filesystem writes under `process.cwd()`.

Connect-time initialization SHALL still eagerly attempt resolution and cache on success; tool entry validation is a synchronous assertion against that cache (no per-call `listRoots()`).

#### Scenario: phase_log fails when lock unset

- **WHEN** the project root lock is unset after connect
- **AND** `phase_log` is invoked
- **THEN** the tool SHALL return an error payload indicating the project root is not locked
- **AND** no `eval.json` SHALL be written under `process.cwd()`

#### Scenario: change_list succeeds with locked root

- **WHEN** the project root is locked to `/workspace/project`
- **AND** `change_list` is invoked with no `project_root` argument
- **THEN** the tool SHALL scan `/workspace/project/openspec/changes/`
- **AND** the returned `project_root` field SHALL equal `/workspace/project`

### Requirement: MCP input schemas omit project_root

MCP tool input schemas for `change_list`, `config_get`, `archi_query`, `archi_validate`, `archi_write`, `archi_check`, `test_detect_frameworks`, and `test_resolve_paths` SHALL NOT include a `project_root` field. Callers MUST NOT override the locked root via tool arguments.

Output fields named `project_root` (read-only echo) MAY remain where already present (e.g. `change_list`).

CLI commands and internal command helpers MAY continue to accept `options.projectRoot` / `project_root` for fixture injection; that path is outside the MCP external contract.

#### Scenario: change_list input rejects project_root field

- **WHEN** a client calls `change_list` with arguments including `project_root`
- **THEN** input validation SHALL fail (field not in schema) or the field SHALL be ignored per schema stripping
- **AND** path resolution SHALL use only the locked project root when the call proceeds

#### Scenario: CLI fixture projectRoot still works

- **WHEN** `runChangeList({ project_root: "/tmp/fixture" })` is invoked outside MCP tool dispatch
- **THEN** the command helper MAY use `/tmp/fixture` as its project root
- **AND** this SHALL NOT reintroduce `project_root` on the MCP input schema

### Requirement: Locked root is immutable for the process lifetime

After a successful lock, subsequent connect-time init calls, `listRoots()` results, or environment mutations MUST NOT change the cached project root for that process. The server SHALL NOT register a `notifications/roots/list_changed` handler. Workspace switches require the host to restart the MCP subprocess.

#### Scenario: Second init does not replace lock

- **WHEN** the project root is locked to `/old/workspace`
- **AND** `initProjectRootFromMcp` (or equivalent) is invoked again with a different unique root `/new/workspace`
- **THEN** `requireLockedProjectRoot()` / the MCP-facing root getter SHALL continue to return `/old/workspace`

### Requirement: MCP server resolves project root via roots/list

The dev-team MCP server MAY use the MCP protocol `roots/list` request as one channel in the startup lock resolution order when the connected client declares the `roots` capability.

After `server.connect(transport)` completes and the client initialization handshake finishes, if higher-priority channels did not lock and the client declares `roots`, the server SHALL call `listRoots()` on the underlying MCP `Server` instance (`McpServer.server`).

When `listRoots()` returns **exactly one** usable root, the server SHALL convert that root's `uri` (`file://` or bare absolute path, when provided) to a platform-local absolute filesystem path and lock it.

When `listRoots()` returns zero roots, more than one usable root, throws, or returns `-32601 Method not found` (capability advertised but unsupported), the roots channel SHALL NOT lock; resolution SHALL continue to lower-priority channels or strict failure. The server SHALL log a distinguishable failure (capability absent vs method-not-found vs empty/multi) to stderr when appropriate, and MUST continue accepting tool requests so handlers can return structured lock errors.

#### Scenario: Client provides exactly one workspace root via roots/list

- **WHEN** higher-priority channels did not lock
- **AND** the MCP client declares `roots` capability during initialization
- **AND** `listRoots()` returns `{ roots: [{ uri: "file:///D:/Projects/wps-claude-plugin", name: "wps-claude-plugin" }] }`
- **THEN** the locked project root SHALL be the local absolute path equivalent to `D:\Projects\wps-claude-plugin`
- **AND** tools that call the locked-root entry point (e.g. `phase_log`, `phase_next`, `change_list`) SHALL operate under that directory

#### Scenario: Client does not support roots capability

- **WHEN** higher-priority channels did not lock
- **AND** `getClientCapabilities()?.roots` is undefined
- **THEN** the server SHALL NOT call `listRoots()`
- **AND** resolution SHALL continue to `WORKSPACE_FOLDER_PATHS` or strict failure

#### Scenario: listRoots returns empty roots array

- **WHEN** higher-priority channels did not lock
- **AND** `listRoots()` succeeds but returns `{ roots: [] }`
- **THEN** the roots channel SHALL NOT lock
- **AND** the server SHALL NOT throw during initialization

#### Scenario: listRoots request fails with Method not found

- **WHEN** the client advertises `roots` capability
- **AND** `listRoots()` throws or returns JSON-RPC `-32601 Method not found`
- **THEN** the server SHALL log the failure in a way distinguishable from "no roots capability"
- **AND** the roots channel SHALL NOT lock
- **AND** the MCP server SHALL continue to accept tool requests

### Requirement: file URI to local path conversion

The server SHALL convert MCP root `uri` values from `file://` scheme to platform-local absolute paths before locking.

Conversion SHALL handle:

- Unix-style URIs: `file:///home/user/project`
- Windows-style URIs: `file:///D:/Projects/wps-claude-plugin` and `file:///D:/Projects/wps-claude-plugin/`

When a client returns a bare absolute filesystem path instead of a `file://` URI, the server MAY accept it if it is an existing absolute path.

The conversion function SHALL produce normalized absolute paths suitable for use with Node.js `path.join` and `fs` operations.

#### Scenario: Windows file URI is converted correctly

- **WHEN** a root URI is `file:///D:/Projects/wps-claude-plugin`
- **THEN** the locked project root SHALL resolve to `D:\Projects\wps-claude-plugin` (or platform-normalized equivalent)

#### Scenario: Unix file URI is converted correctly

- **WHEN** a root URI is `file:///home/user/projects/my-app`
- **THEN** the locked project root SHALL resolve to `/home/user/projects/my-app`

## Module Contract

### Function: initProjectRootFromMcp

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts` |
| **Signature** | `initProjectRootFromMcp(server: Server): Promise<void>` |
| **Input** | `server` — underlying MCP `Server` instance from `McpServer.server` |
| **Output** | `void` (side effect: may set module-level locked project root cache) |
| **Behavior** | Eagerly applies startup lock priority; does not exit process on failure; does not register `list_changed` |

### Function: requireLockedProjectRoot

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts`（或 MCP 侧等价入口） |
| **Signature** | `requireLockedProjectRoot(): string` |
| **Input** | None |
| **Output** | Absolute locked project root |
| **Behavior** | Returns cache when locked; throws / returns structured error signal when unset — used by all MCP tool handlers |

### Function: getProjectDir

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts`（可由 `utils/index.ts` re-export；**不得**再实现于 `utils/constant.ts`） |
| **Signature** | `getProjectDir(): string` |
| **Input** | None |
| **Output** | Absolute filesystem path |
| **Behavior** | Lives with MCP lock/cache in `project-root`. MCP-facing callers MUST prefer `requireLockedProjectRoot()`. CLI / command helpers MAY still use `getProjectDir()` with explicit `options.projectRoot` override. MCP path MUST NOT silently return `process.cwd()` when lock unset. |

### Module layout

| Path | Status |
|------|--------|
| `lib/project-root.ts` | Owns lock cache, `initProjectRootFromMcp`, `requireLockedProjectRoot`, `getProjectDir`, URI helpers |
| `utils/constant.ts` | **REMOVED** — former sole home of `getProjectDir` |

### Function: fileUriToPath

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts` |
| **Signature** | `fileUriToPath(uri: string): string`（实现细节，可不 export） |
| **Input** | MCP root `uri` string |
| **Output** | Platform-local absolute filesystem path |
| **Behavior** | Parses and normalizes `file://` URIs for Windows and Unix |

### Function: getChangeDir

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/change.ts` |
| **Signature** | `getChangeDir(changeName: string): string` |
| **Input** | `changeName` — kebab-case change identifier |
| **Output** | Absolute path to `openspec/changes/<changeName>/` |
| **Behavior** | Resolves under the locked project root when called from MCP handlers |

### MCP Server lifecycle (`plugins/dev-team/bin/src/mcp.ts`)

| Step | Description |
|------|-------------|
| `main()` | Creates `McpServer`, registers tools, connects transport |
| Post-connect | `await initProjectRootFromMcp(server.server)` — eager lock attempt |
| Tool handlers | Call `requireLockedProjectRoot()` (or equivalent); no `args.project_root` |
| Notifications | MUST NOT register `roots/list_changed` |

### Affected MCP tools

| Tool | project_root input | Resolution path |
|------|--------------------|-----------------|
| `phase_log` | No | locked root → `getChangeDir()` |
| `phase_next` | No | locked root → `getChangeDir()` |
| `backtrack` | No | locked root |
| `change_list` | No（已删除） | locked root；output 可回显 `project_root` |
| `config_get` | No（已删除） | locked root |
| `archi_query`, `archi_validate`, `archi_write`, `archi_check` | No（已删除） | locked root |
| `test_detect_frameworks`, `test_resolve_paths` | No（已删除） | locked root |
