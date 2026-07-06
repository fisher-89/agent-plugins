## ADDED Requirements

### Requirement: Protection scope extended from eval.json to config-driven file set

The PreToolUse protection hook SHALL extend its scope beyond `eval.json` to cover any file matching the built-in defaults or user-defined `write_protection.files` glob patterns.

The hook SHALL be renamed from `protect-eval.mjs` to `protect-files.mjs` to reflect the expanded scope.

The hook SHALL continue to protect `openspec/changes/*/eval.json` as a built-in default, even when no `write_protection` configuration is present.

New built-in default protection SHALL be added for `openspec/config.json`.

#### Scenario: eval.json protected as built-in default (unchanged behavior)

**WHEN** the hook receives a Write tool call for `openspec/changes/test/eval.json`
**AND** no `write_protection` config is present
**THEN** the hook returns `permissionDecision: "deny"`
**AND** the denial reason references `eval.json`

#### Scenario: config.json protected as new built-in default

**WHEN** the hook receives a Write tool call for `openspec/config.json`
**AND** no `write_protection` config is present
**THEN** the hook returns `permissionDecision: "deny"`

### Requirement: Common denial reason utility preserved and extended

The `buildDenyReason` utility SHALL be extended to support two overloads:

1. For built-in default matches: `buildDenyReason(filePath, toolName)`
2. For user-defined matches with custom reason: `buildDenyReason(filePath, toolName, customReasonTemplate)` — where `%s` is replaced with file path and `%t` with tool name

#### Scenario: Default reason format for built-in protection

**WHEN** evaluating `buildDenyReason("openspec/config.json", "Write")`
**THEN** the returned string includes the file path and tool name in the default format

#### Scenario: Custom reason with placeholder substitution

**WHEN** evaluating `buildDenyReason("secrets/key", "Bash", "File '%s' is protected from '%t'")`
**THEN** the returned string is `"File 'secrets/key' is protected from 'Bash'"`

### Requirement: PowerShell detection extended for config.json

The PowerShell write detection logic SHALL be extended to detect writes targeting `config.json` in addition to `eval.json`.

The same detection patterns (Set-Content, Out-File, Add-Content, redirect operators, .NET file methods) SHALL apply.

#### Scenario: PowerShell Out-File to config.json is denied

**WHEN** the tool `PowerShell` is called with command `'{"schema":"spec-driven"}' | Out-File openspec/config.json`
**THEN** the hook returns `permissionDecision: "deny"`

### Requirement: Bash detection extended for config.json

The Bash write detection logic SHALL be extended to detect writes targeting `config.json` in addition to `eval.json`.

The same detection patterns (redirect, tee, heredoc) and exemptions (python/node prefix) SHALL apply.

#### Scenario: Bash echo redirect to config.json is denied

**WHEN** the tool `Bash` is called with command `echo '{}' > openspec/config.json`
**THEN** the hook returns `permissionDecision: "deny"`

## Module Contract

### Module: protect-files.mjs (formerly protect-eval.mjs)

| Export | Change | Description |
|--------|--------|-------------|
| `isEvalJsonPath` | RENAMED → `isProtectedPath` | Now checks against both built-in defaults and user glob patterns |
| `buildDenyReason` | MODIFIED | Extended to accept optional custom reason template |
| `parseInput` | MODIFIED | Now reads config.json and iterates over write_protection.files |
| `detectBashWrite` | MODIFIED | Detection scope widened from eval.json to all protected files |
| `detectPowerShellWrite` | MODIFIED | Detection scope widened from eval.json to all protected files |
