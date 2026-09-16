## MODIFIED Requirements

### Requirement: phaseIdSchema updated to remove unit-test and integration-test, add test-execution

The `phaseIdSchema` enum in `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` SHALL be updated:

**Before**:
```typescript
export const phaseIdSchema = z
  .enum([
    'proposal',
    'dev-design',
    'test-design',
    'implement',
    'test-gen',
    'unit-test',
    'code-review',
    'integration-test',
    'acceptance',
    'code-analyze',
  ])
```

**After**:
```typescript
export const phaseIdSchema = z
  .enum([
    'proposal',
    'dev-design',
    'test-design',
    'implement',
    'test-gen',
    'test-execution',
    'code-review',
    'acceptance',
    'code-analyze',
  ])
```

#### Scenario: phase/log accepts test-execution phase

**WHEN** MCP phase_log is called with phase `test-execution`
**THEN** the call succeeds (no validation error)
**AND** the entry is appended to eval.json with phase `test-execution`

#### Scenario: phase/log rejects unit-test phase

**WHEN** MCP phase_log is called with phase `unit-test`
**THEN** the call fails with validation error (not in enum)
**AND** no entry is written to eval.json

#### Scenario: phase/log rejects integration-test phase

**WHEN** MCP phase_log is called with phase `integration-test`
**THEN** the call fails with validation error (not in enum)
**AND** no entry is written to eval.json

#### Scenario: phase/log still accepts code-analyze (unchanged)

**WHEN** MCP phase_log is called with phase `code-analyze`
**THEN** the call succeeds (code-analyze remains in enum)

### Requirement: phaseIdSchema length validation

The phaseIdSchema SHALL have exactly 9 enum values (was 10 before the change):

| Enum Value | Status |
|-----------|--------|
| `proposal` | Unchanged |
| `dev-design` | Unchanged |
| `test-design` | Unchanged |
| `implement` | Unchanged |
| `test-gen` | Unchanged |
| `test-execution` | ADDED |
| `unit-test` | REMOVED |
| `code-review` | Unchanged |
| `integration-test` | REMOVED |
| `acceptance` | Unchanged |
| `code-analyze` | Unchanged |

#### Scenario: phaseIdSchema has exactly 9 values

**WHEN** inspecting `phaseIdSchema.options`
**THEN** the array length is 9 (was 10)

## Module Contract

### phase-log.schema.ts

| Export | Change | Description |
|--------|--------|-------------|
| `phaseIdSchema` | MODIFIED | Enum: removed `unit-test` and `integration-test`; added `test-execution`. 10 values → 9 values. |
| `phaseLogSchema` | UNCHANGED | Schema structure unchanged — only validates against updated phaseIdSchema |
| `phaseLogInputSchema` | UNCHANGED | Input schema structure unchanged |

---

### Requirement: Protection scope extended from eval.json to config-driven file set

The PreToolUse protection hook SHALL extend its scope beyond `eval.json` to cover any file matching the built-in defaults or user-defined `write_protection.files` glob patterns.

The hook SHALL be renamed from `protect-eval.mjs` to `protect-files.mjs` to reflect the expanded scope.

`eval.json` SHALL NOT be a built-in default anymore. The remaining built-in defaults SHALL be:

- `openspec/changes/**/workflow.json` — 评估历史与 `workflow_type` 的权威文件，禁止 agent 直接写入
- `openspec/config.json`

The hook SHALL continue to deny Write/Edit/Bash/PowerShell（及 Cursor 的 Shell/StrReplace）when the target matches these globs. 需要拦截 `eval.json` 的项目 SHALL 通过 `write_protection.files` 自行声明。

#### Scenario: workflow.json protected as built-in default

**WHEN** the hook receives a Write tool call for `openspec/changes/test/workflow.json`
**AND** no `write_protection` config is present
**THEN** the hook returns `permissionDecision: "deny"`
**AND** the denial reason references `workflow.json`

#### Scenario: eval.json is no longer a built-in default

**WHEN** the hook receives a Write tool call for `openspec/changes/test/eval.json`
**AND** no `write_protection` config is present
**THEN** the built-in defaults SHALL NOT produce `permissionDecision: "deny"`

#### Scenario: eval.json can still be protected by user configuration

**WHEN** `write_protection.files` 含 glob `openspec/changes/*/eval.json`
**AND** the hook receives a Write tool call for `openspec/changes/test/eval.json`
**THEN** the hook returns `permissionDecision: "deny"`

#### Scenario: config.json protected as built-in default

**WHEN** the hook receives a Write tool call for `openspec/config.json`
**AND** no `write_protection` config is present
**THEN** the hook returns `permissionDecision: "deny"`

### Requirement: Common denial reason utility preserved and extended

The `buildDenyReason` utility SHALL be extended to support an optional custom reason template parameter where `%s` is replaced with file path and `%t` with tool name.

#### Scenario: Default reason format for built-in protection

**WHEN** evaluating `buildDenyReason("openspec/config.json", "Write")`
**THEN** the returned string includes the file path and tool name in the default format

#### Scenario: Custom reason with placeholder substitution

**WHEN** evaluating `buildDenyReason("secrets/key", "Bash", "File '%s' is protected from '%t'")`
**THEN** the returned string is `"File 'secrets/key' is protected from 'Bash'"`

### Requirement: PowerShell detection extended for config.json

The PowerShell write detection logic SHALL detect writes targeting any protected file (not just eval.json).

The same detection patterns (Set-Content, Out-File, Add-Content, redirect operators, .NET file methods) SHALL apply.

#### Scenario: PowerShell Out-File to config.json is denied

**WHEN** the tool `PowerShell` is called with command `'{"schema":"spec-driven"}' | Out-File openspec/config.json`
**THEN** the hook returns `permissionDecision: "deny"`

### Requirement: Bash detection extended for config.json

The Bash write detection logic SHALL detect writes targeting any protected file (not just eval.json).

The same detection patterns (redirect, tee, heredoc) and exemptions (python/node prefix) SHALL apply.

#### Scenario: Bash echo redirect to config.json is denied

**WHEN** the tool `Bash` is called with command `echo '{}' > openspec/config.json`
**THEN** the hook returns `permissionDecision: "deny"`

## Module Contract (protect-files extensions)

### Module: protect-files.mjs (formerly protect-eval.mjs)

| Export | Change | Description |
|--------|--------|-------------|
| `isEvalJsonPath` | RENAMED → `isProtected` | Now checks against both built-in defaults and user glob patterns |
| `buildDenyReason` | MODIFIED | Extended to accept optional custom reason template |
| `parseInput` | MODIFIED | Now reads config.json and iterates over write_protection.files |
| `detectBashWrite` | MODIFIED | Detection scope widened from eval.json to all protected files |
| `detectPowerShellWrite` | MODIFIED | Detection scope widened from eval.json to all protected files |

### Module: `plugins/dev-team/bin/src/hooks.ts`

| 项 | 变更 |
|----|------|
| 内置 glob | 增加 `workflow.json`；**移除** `eval.json`；保留 `config.json` |
| user-defined glob 合并 | 不变（`write_protection.files` 仍按原逻辑追加） |
