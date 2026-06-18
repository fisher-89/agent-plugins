## ADDED Requirements

### Requirement: MCP server resolves project root via roots/list

The dev-team MCP server SHALL resolve the workspace project root directory using the MCP protocol `roots/list` request when the connected client declares the `roots` capability.

After `server.connect(transport)` completes and the client initialization handshake finishes, the server SHALL call `listRoots()` on the underlying MCP `Server` instance (`McpServer.server`).

When `listRoots()` returns one or more roots, the server SHALL use the **first** root entry's `uri` as the project root, converting the `file://` URI to a platform-local absolute filesystem path.

The resolved path SHALL be cached in module-level state and returned by `getProjectDir()` for all subsequent synchronous lookups until the cache is refreshed or cleared.

#### Scenario: Client provides workspace root via roots/list

- **WHEN** the MCP client declares `roots` capability during initialization
- **AND** `listRoots()` returns `{ roots: [{ uri: "file:///D:/Projects/wps-claude-plugin", name: "wps-claude-plugin" }] }`
- **THEN** `getProjectDir()` SHALL return the local absolute path equivalent to `D:\Projects\wps-claude-plugin`
- **AND** tools that call `getProjectDir()` indirectly (e.g. `phase_log`, `phase_next`, `change_list` without explicit `project_root`) SHALL operate under that directory

#### Scenario: Client does not support roots capability

- **WHEN** `getClientCapabilities()?.roots` is undefined
- **THEN** the server SHALL NOT call `listRoots()`
- **AND** `getProjectDir()` SHALL fall back to the legacy resolution chain

#### Scenario: listRoots returns empty roots array

- **WHEN** `listRoots()` succeeds but returns `{ roots: [] }`
- **THEN** `getProjectDir()` SHALL fall back to the legacy resolution chain
- **AND** the server SHALL NOT throw during initialization

#### Scenario: listRoots request fails

- **WHEN** `listRoots()` throws or returns an error (e.g. capability not supported at runtime)
- **THEN** the server SHALL log the failure to stderr if appropriate
- **AND** `getProjectDir()` SHALL fall back to the legacy resolution chain
- **AND** the MCP server SHALL continue to accept tool requests

### Requirement: Legacy fallback chain for project root resolution

When MCP roots resolution is unavailable, not yet initialized, or yields no usable path, `getProjectDir()` SHALL resolve the project root using the following priority order:

1. `process.env.CLAUDE_PROJECT_DIR` (if set and non-empty)
2. `process.env.CURSOR_PROJECT_DIR` (if set and non-empty)
3. `process.cwd()`

This fallback chain SHALL preserve compatibility with Claude Code plugin mode, CLI invocation, and test environments that do not provide MCP roots.

#### Scenario: Environment variable takes precedence over cwd when roots unavailable

- **WHEN** MCP roots cache is not set
- **AND** `process.env.CLAUDE_PROJECT_DIR` is set to `/workspace/my-project`
- **THEN** `getProjectDir()` SHALL return `/workspace/my-project`

#### Scenario: cwd used as last resort

- **WHEN** MCP roots cache is not set
- **AND** neither `CLAUDE_PROJECT_DIR` nor `CURSOR_PROJECT_DIR` is set
- **THEN** `getProjectDir()` SHALL return `process.cwd()`

### Requirement: file URI to local path conversion

The server SHALL convert MCP root `uri` values from `file://` scheme to platform-local absolute paths before caching.

Conversion SHALL handle:

- Unix-style URIs: `file:///home/user/project`
- Windows-style URIs: `file:///D:/Projects/wps-claude-plugin` and `file:///D:/Projects/wps-claude-plugin/`

The conversion function SHALL produce normalized absolute paths suitable for use with Node.js `path.join` and `fs` operations.

#### Scenario: Windows file URI is converted correctly

- **WHEN** a root URI is `file:///D:/Projects/wps-claude-plugin`
- **THEN** the cached project root SHALL resolve to `D:\Projects\wps-claude-plugin` (or platform-normalized equivalent)

#### Scenario: Unix file URI is converted correctly

- **WHEN** a root URI is `file:///home/user/projects/my-app`
- **THEN** the cached project root SHALL resolve to `/home/user/projects/my-app`

### Requirement: Project root cache refreshes on roots/list_changed notification

When the MCP client declares `roots.listChanged: true`, the server SHALL register a notification handler for `notifications/roots/list_changed`.

Upon receiving the notification, the server SHALL re-invoke `listRoots()` and update the cached project root using the same first-root selection and URI conversion rules.

If the refresh fails or returns empty roots, the server SHALL retain the previous cache if one exists; otherwise it SHALL fall back to the legacy chain on subsequent `getProjectDir()` calls.

#### Scenario: Roots list change updates cached project root

- **WHEN** the cached project root is `/old/workspace`
- **AND** the client sends `notifications/roots/list_changed`
- **AND** a subsequent `listRoots()` returns a root with URI `file:///new/workspace`
- **THEN** `getProjectDir()` SHALL return `/new/workspace`

#### Scenario: Refresh failure preserves existing cache

- **WHEN** a valid project root is already cached
- **AND** `notifications/roots/list_changed` is received
- **AND** the subsequent `listRoots()` call fails
- **THEN** `getProjectDir()` SHALL continue to return the previously cached path

### Requirement: Explicit project_root tool parameter overrides MCP resolution

For MCP tools that accept an optional `project_root` input parameter, the tool handler SHALL use the explicitly provided value when present and non-empty, without consulting the MCP roots cache for that invocation.

Tools without a `project_root` parameter (including `phase_log` and `phase_next`) SHALL rely entirely on `getProjectDir()` / `getChangeDir()` for path resolution.

#### Scenario: change_list with explicit project_root

- **WHEN** `change_list` is called with `project_root: "/custom/root"`
- **THEN** the tool SHALL scan `/custom/root/openspec/changes/`
- **AND** the returned `project_root` field SHALL be `/custom/root`

#### Scenario: phase_log uses MCP-resolved root

- **WHEN** MCP roots cache is set to `/workspace/project`
- **AND** `phase_log` is called without any `project_root` parameter
- **THEN** `eval.json` SHALL be read and written at `/workspace/project/openspec/changes/<change>/eval.json`

## Module Contract

### Function: getProjectDir

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/utils/constant.ts` (re-exported via `utils/index.ts`) |
| **Signature** | `getProjectDir(): string` |
| **Input** | None |
| **Output** | Absolute filesystem path to the project root |
| **Behavior** | Returns MCP roots cache when set; otherwise `CLAUDE_PROJECT_DIR \|\| CURSOR_PROJECT_DIR \|\| process.cwd()` |

### Function: initProjectRootFromMcp

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts` |
| **Signature** | `initProjectRootFromMcp(server: Server): Promise<void>` |
| **Input** | `server` — underlying MCP `Server` instance from `McpServer.server` |
| **Output** | `void` (side effect: sets module-level project root cache) |
| **Behavior** | Checks client roots capability; calls `listRoots()`; converts first root URI; registers `roots/list_changed` handler when supported |

### Function: fileUriToPath

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/project-root.ts` |
| **Signature** | `fileUriToPath(uri: string): string` |
| **Input** | MCP root `uri` string (`file://...`) |
| **Output** | Platform-local absolute filesystem path |
| **Behavior** | Parses and normalizes `file://` URIs for Windows and Unix |

### Function: getChangeDir

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/bin/src/lib/change.ts` |
| **Signature** | `getChangeDir(changeName: string): string` |
| **Input** | `changeName` — kebab-case change identifier |
| **Output** | Absolute path to `openspec/changes/<changeName>/` |
| **Behavior** | `path.resolve(getProjectDir(), 'openspec', 'changes', changeName)` — benefits from MCP root resolution automatically |

### MCP Server lifecycle (`plugins/dev-team/bin/src/mcp.ts`)

| Step | Description |
|------|-------------|
| `main()` | Creates `McpServer`, registers tools, connects transport |
| Post-connect | `await initProjectRootFromMcp(server.server)` before or as part of startup sequence |
| `resolveProjectRoot(cwd?)` | Returns `cwd` if provided; otherwise delegates to `getProjectDir()` |

### Affected MCP tools (read-only contract — no schema changes)

| Tool | project_root param | Resolution path |
|------|-------------------|-----------------|
| `phase_log` | No | `getChangeDir()` → `getProjectDir()` |
| `phase_next` | No | `getChangeDir()` → `getProjectDir()` |
| `change_list` | Optional | `resolveProjectRoot(args.project_root)` |
| `config_get`, `config_set`, `config_unset`, `config_context` | Optional | `resolveProjectRoot(args.project_root)` |
| `archi_query`, `archi_validate`, `archi_write`, `archi_check` | Optional | `resolveProjectRoot(args.project_root)` |
| `test_detect_frameworks`, `test_resolve_paths` | Optional | `resolveProjectRoot(args.project_root)` |
