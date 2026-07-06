## MODIFIED Requirements

### Requirement: protect-files hook replaces protect-eval

The PreToolUse hook SHALL be renamed from `protect-eval.mjs` to `protect-files.mjs` and SHALL support config-driven file protection.

The hook SHALL intercept the following tool calls: `Write`, `Edit`, `Bash`, `PowerShell`.

The hook SHALL read `openspec/config.json` at invocation time to obtain the `write_protection` configuration.

#### Scenario: Hook script is renamed to protect-files.mjs

**WHEN** inspecting `hooks/hooks.json` for PreToolUse entries
**THEN** the script paths reference `hooks/scripts/protect-files.mjs` instead of `hooks/scripts/protect-eval.mjs`

#### Scenario: Hook reads config.json at each invocation

**WHEN** the hook is invoked
**THEN** it reads and parses `openspec/config.json` from the project root
**AND** extracts the `write_protection` configuration object

### Requirement: Built-in default protections

The hook SHALL always protect the following files regardless of `write_protection` configuration:

- Any file matching `openspec/changes/*/eval.json` (glob pattern `**/openspec/changes/*/eval.json`)
- Any file matching `openspec/config.json` (glob pattern `**/openspec/config.json`)

These built-in defaults SHALL be enforced even when `write_protection` is absent, empty, or when `config.json` cannot be parsed.

The default denial reason SHALL use `%s` for file path and `%t` for tool name placeholders.

#### Scenario: eval.json write is denied without write_protection config

**WHEN** the tool `Write` is called with `file_path` containing `openspec/changes/test-change/eval.json`
**AND** `openspec/config.json` does not contain `write_protection`
**THEN** the hook returns `permissionDecision: "deny"`
**AND** the denial reason mentions `eval.json` and the tool name

#### Scenario: config.json write is denied by built-in default

**WHEN** the tool `Write` is called with `file_path` containing `openspec/config.json`
**AND** `openspec/config.json` does not contain `write_protection`
**THEN** the hook returns `permissionDecision: "deny"`

#### Scenario: config.json write via Bash redirect is denied

**WHEN** the tool `Bash` is called with a command containing `echo '{}' > openspec/config.json`
**AND** `openspec/config.json` does not contain `write_protection`
**THEN** the hook returns `permissionDecision: "deny"`

### Requirement: User-defined glob protection

The hook SHALL iterate over `write_protection.files` and match the target file path against each entry's `glob` pattern using an inline `globToRegex` function.

Path normalization SHALL convert backslashes to forward slashes before matching to ensure cross-platform consistency.

When a match is found, the hook SHALL return `permissionDecision: "deny"`.

#### Scenario: User glob matches target file path

**WHEN** `openspec/config.json` contains `{ "write_protection": { "files": [{ "glob": "secrets/**" }] } }`
**AND** the tool `Edit` is called with `file_path` containing `secrets/keys.yml`
**THEN** the hook returns `permissionDecision: "deny"`

#### Scenario: File not matching any glob passes through

**WHEN** `openspec/config.json` contains `{ "write_protection": { "files": [{ "glob": "secrets/**" }] } }`
**AND** the tool `Write` is called with `file_path` containing `src/app.ts`
**THEN** the hook returns `permissionDecision: "allow"`

#### Scenario: Windows backslash path matches glob after normalization

**WHEN** `openspec/config.json` contains `{ "write_protection": { "files": [{ "glob": "secrets/**" }] } }`
**AND** the tool `Write` is called with `file_path` containing `secrets\keys.yml` (Windows backslash)
**THEN** the hook returns `permissionDecision: "deny"`

### Requirement: Custom denial reason

When a matched `write_protection.files` entry contains a `reason` field, the hook SHALL use that string as the denial reason, with the following placeholder substitutions:

- `%s` — the file path that was attempted to be written
- `%t` — the tool name (e.g., "Write", "Edit", "Bash", "PowerShell")

#### Scenario: Custom reason with file path placeholder

**WHEN** `openspec/config.json` contains `{ "write_protection": { "files": [{ "glob": "secrets/*", "reason": "File '%s' is protected. Use a dedicated tool." }] } }`
**AND** the tool `Write` is called with `file_path` containing `secrets/api.key`
**THEN** the hook returns `permissionDecision: "deny"`
**AND** the denial reason contains "File '...secrets/api.key' is protected."

#### Scenario: Custom reason with tool name placeholder

**WHEN** `openspec/config.json` contains `{ "write_protection": { "files": [{ "glob": "secrets/*", "reason": "Protected via %t" }] } }`
**AND** the tool `Write` is called with `file_path` containing `secrets/api.key`
**THEN** the denial reason contains "Protected via Write"

### Requirement: Fail-open behavior preserved

The hook SHALL default to `permissionDecision: "allow"` under any of the following conditions:

- Empty or missing stdin
- Invalid JSON stdin
- Missing `tool_name` field
- Missing `tool_input.file_path` for Write/Edit tools
- Missing `tool_input.command` for Bash/PowerShell tools
- `config.json` read or parse failure (SHALL fall back to built-in defaults)

These are the same fail-open guards as the original protect-eval.mjs.

#### Scenario: Empty stdin returns allow

**WHEN** the hook receives empty stdin
**THEN** it returns `permissionDecision: "allow"`

#### Scenario: Invalid JSON stdin returns allow

**WHEN** the hook receives invalid JSON stdin (`{not json`)
**THEN** it returns `permissionDecision: "allow"`

#### Scenario: Missing tool_input.file_path for Write returns allow

**WHEN** the hook receives `{ "tool_name": "Write", "tool_input": {} }`
**THEN** it returns `permissionDecision: "allow"`

#### Scenario: Config.json parse failure falls back to built-in defaults

**WHEN** `openspec/config.json` contains invalid JSON
**AND** the tool `Write` is called with `file_path` containing `openspec/changes/test/eval.json`
**THEN** the hook still returns `permissionDecision: "deny"` (built-in default protection applies)

### Requirement: Python/Node exemption preserved

The hook SHALL NOT deny Bash commands that start with `python`, `python3`, or `node`, matching the existing exemption behavior.

#### Scenario: Python command writing eval.json is allowed

**WHEN** the tool `Bash` is called with command `python scripts/deploy.py --eval openspec/changes/test/eval.json`
**THEN** the hook returns `permissionDecision: "allow"`

#### Scenario: Node command writing config.json is allowed

**WHEN** the tool `Bash` is called with command `node scripts/setup.js openspec/config.json`
**THEN** the hook returns `permissionDecision: "allow"`

## Module Contract

### Hook: PreToolUse — protect-files.mjs

| Property | Description |
|----------|-------------|
| **Location** | `plugins/dev-team/hooks/scripts/protect-files.mjs` |
| **Registration** | `plugins/dev-team/hooks/hooks.json` — PreToolUse for Write/Edit/Bash/PowerShell |
| **Input** | stdin JSON: `{ tool_name, tool_input: { file_path?, command? } }` |
| **Output** | stdout JSON: `{ hookSpecificOutput: { permissionDecision, permissionDecisionReason? } }` |
| **Config source** | `openspec/config.json` → `write_protection` field |
| **Built-in defaults** | `**/openspec/changes/*/eval.json`, `**/openspec/config.json` |
| **Glob library** | Inline `globToRegex` function (zero external dependencies) |
| **Path normalization** | Backslash (`\`) → forward slash (`/`) before pattern matching |
| **Fallback** | Config parse failure → built-in defaults only |
| **Fail-open** | Input anomalies → allow |

### hooks.json (updated)

| Property | Before | After |
|----------|--------|-------|
| PreToolUse script (Write/Edit) | `protect-eval.mjs` | `protect-files.mjs` |
| PreToolUse script (Bash) | `protect-eval.mjs` | `protect-files.mjs` |
| PreToolUse script (PowerShell) | `protect-eval.mjs` | `protect-files.mjs` |
