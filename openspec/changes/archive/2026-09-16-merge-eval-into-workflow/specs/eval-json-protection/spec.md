## MODIFIED Requirements

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

## Module Contract

### Module: `plugins/dev-team/bin/src/hooks.ts`

| 项 | 变更 |
|----|------|
| 内置 glob | 增加 `workflow.json`；**移除** `eval.json`；保留 `config.json` |
| user-defined glob 合并 | 不变（`write_protection.files` 仍按原逻辑追加） |
