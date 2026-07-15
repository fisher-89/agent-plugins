## ADDED Requirements

### Requirement: buildTestCommand selects script by platform

**ID**: REQ-TEC-PLAT-1
**Priority**: MUST
**Description**: The `buildTestCommand()` function in `lib/test-runner.ts` SHALL select the test script from `entry.script` based on `process.platform`:

- On `process.platform === 'win32'`, SHALL use `entry.script.cmd`
- On other platforms, SHALL use `entry.script.shell`

Signature unchanged: `buildTestCommand(entry: TestPlan, projectRoot: string, files?: string[]): string`. Template substitution SHALL remain unchanged.

If `entry.script.cmd` is missing or empty on Windows, SHALL fall back to `entry.script.shell`.

#### Scenario: Windows selects script.cmd

**WHEN** `process.platform` is `'win32'`
**AND** `buildTestCommand(entry, ...)` is called
**AND** `entry.script.cmd` is a non-empty string
**THEN** the returned command SHALL be `substitutePlaceholders(entry.script.cmd, ...)`
**AND** SHALL NOT use `entry.script.shell`

#### Scenario: Unix selects script.shell

**WHEN** `process.platform` is `'linux'` or `'darwin'`
**AND** `buildTestCommand(entry, ...)` is called
**THEN** the returned command SHALL be `substitutePlaceholders(entry.script.shell, ...)`
**AND** SHALL NOT use `entry.script.cmd`

#### Scenario: Windows fallback when script.cmd is empty

**WHEN** `process.platform` is `'win32'`
**AND** `entry.script.cmd` is `''`
**THEN** `buildTestCommand()` SHALL fall back to `entry.script.shell`
**AND** SHOULD log a warning

### Requirement: resolveShell returns platform-appropriate shell

**ID**: REQ-TEC-PLAT-2
**Priority**: MUST
**Description**: `resolveShell()` SHALL return:

1. Non-Windows: `undefined` (system default shell)
2. Windows: `process.env.SHELL || process.env.COMSPEC || 'cmd.exe'`

#### Scenario: Unix returns undefined

**WHEN** `process.platform` is `'linux'`
**THEN** `resolveShell()` SHALL return `undefined`

#### Scenario: Windows with SHELL returns SHELL

**WHEN** `process.platform` is `'win32'`
**AND** `process.env.SHELL` is `'C:\\Program Files\\Git\\bin\\bash.exe'`
**THEN** `resolveShell()` SHALL return `'C:\\Program Files\\Git\\bin\\bash.exe'`

#### Scenario: Windows without SHELL returns COMSPEC

**WHEN** `process.platform` is `'win32'`
**AND** `process.env.SHELL` is `undefined`
**AND** `process.env.COMSPEC` is `'C:\\Windows\\system32\\cmd.exe'`
**THEN** `resolveShell()` SHALL return `'C:\\Windows\\system32\\cmd.exe'`

#### Scenario: Windows without SHELL and COMSPEC hardcodes cmd.exe

**WHEN** `process.platform` is `'win32'`
**AND** `process.env.SHELL` is `undefined`
**AND** `process.env.COMSPEC` is `undefined`
**THEN** `resolveShell()` SHALL return `'cmd.exe'`

### Requirement: runCommand passes selected shell to execSync

**ID**: REQ-TEC-PLAT-3
**Priority**: MUST
**Description**: `runCommand()` SHALL pass `resolveShell()` as the `shell` option to `execSync()`. No behavioral change — it already calls `resolveShell()`. The change is entirely in `resolveShell()` (REQ-TEC-PLAT-2).

Signature unchanged: `runCommand(cmd: string, cwd: string, timeout?: number)`

#### Scenario: runCommand passes shell from resolveShell

**WHEN** `runCommand(cmd, cwd, timeout)` is called
**THEN** `execSync()` SHALL receive `shell` set to `resolveShell()` return value
**AND** all other options (`cwd`, `encoding`, `timeout`, `maxBuffer`, `stdio`) SHALL remain unchanged

#### Scenario: runCommand on Windows cmd.exe

**WHEN** `process.platform` is `'win32'`
**AND** `SHELL` and `COMSPEC` are undefined
**AND** `runCommand()` is called with a cmd.exe-compatible script (from `script.cmd`)
**THEN** `execSync()` SHALL receive `{ shell: 'cmd.exe' }`

### Requirement: Stryker mutation command uses platform-aware shell

**ID**: REQ-TEC-PLAT-4
**Priority**: MUST
**Description**: `executeStrykerMutation()` calls `runCommand()` which inherits the updated `resolveShell()`. No separate shell logic needed.

#### Scenario: Stryker inherits shell from runCommand

**WHEN** `executeStrykerMutation()` is called on Windows without Git Bash
**THEN** `npx stryker run` SHALL execute under cmd.exe

#### Scenario: Stryker Unix behavior unchanged

**WHEN** `executeStrykerMutation()` is called on Linux
**THEN** `runCommand()` SHALL use shell `undefined` (system default)

### Requirement: Empty command check accounts for platform-aware selection

**ID**: REQ-TEC-PLAT-5
**Priority**: MUST
**Description**: The empty command check in `executePlanEntry()` SHALL work correctly since it calls `buildTestCommand()` which selects the platform-appropriate script.

#### Scenario: empty script.cmd falls back to script.shell

**WHEN** `process.platform` is `'win32'`
**AND** `entry.script.cmd` is `''`
**AND** `entry.script.shell` is non-empty
**THEN** execution SHALL proceed using `script.shell`

#### Scenario: both scripts empty returns error

**WHEN** `entry.script.shell` is `''` AND `entry.script.cmd` is `''`
**THEN** `buildTestCommand()` SHALL return `''`
**AND** `executePlanEntry()` SHALL return `'Empty test command'` error

## Module Contract

### Module: lib/test-runner.ts (MODIFIED)

| Function | Before | After |
|----------|--------|-------|
| `resolveShell()` | Win: `process.env.SHELL \|\| 'bash'`; Unix: `undefined` | Win: `process.env.SHELL \|\| process.env.COMSPEC \|\| 'cmd.exe'`; Unix: `undefined` |
| `buildTestCommand(entry, root, files)` | Uses `entry.script` (string) | Selects `entry.script.cmd` (Win) or `entry.script.shell` (Unix) |
| `runCommand(cmd, cwd, timeout)` | Passes `resolveShell()` to `execSync` | Unchanged |
| `executePlanEntry(entry, root, options)` | Calls `buildTestCommand()` → `runCommand()` | Unchanged |
| `executeStrykerMutation(entry, dir, sources)` | Calls `runCommand(strykerCmd, ...)` | Unchanged |

### Shell Selection Flow

```
executePlanEntry()
  → buildTestCommand() reads entry.script (object)
    → process.platform === 'win32' ? entry.script.cmd : entry.script.shell
  → substitutePlaceholders(selectedScript, ...)
  → runCommand(cmd)
    → resolveShell()
      → win32 ? (SHELL || COMSPEC || 'cmd.exe') : undefined
    → execSync(cmd, { shell, ... })
```

### Backward Compatibility Matrix

| Platform | SHELL set | Before | After |
|----------|-----------|--------|-------|
| Linux    | N/A       | `shell: undefined` | Unchanged |
| macOS    | N/A       | `shell: undefined` | Unchanged |
| Windows  | Yes       | `shell: '...bash.exe'` | Unchanged |
| Windows  | No        | `shell: 'bash'` → FAILS | `shell: 'cmd.exe'` → WORKS |
