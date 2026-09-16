# protect-files-hook Specification

## Purpose

PreToolUse 写保护。评估历史并入 `workflow.json` 后，必须保护该文件；遗留 `eval.json` 不再属于内置保护集合。

## MODIFIED Requirements

### Requirement: Built-in default protections (迁移不变)

内置保护规则 SHALL 在无 `write_protection`、为空或 `config.json` 无法解析时仍强制执行。

内置保护文件列表：

- 任何匹配 `**/openspec/changes/**/workflow.json` 的文件（评估历史与 `workflow_type` 的权威存储；agent 不得用 Write/Edit/Shell 等改写）
- 任何匹配 `**/openspec/config.json` 的文件

内置集合 SHALL NOT 包含 `**/openspec/changes/**/eval.json`：该文件是过渡期只读回退源，受保护面收敛后不再拦截对它的 agent 写入（用户仍可通过 `write_protection.files` 自行加入）。

`workflow.json` 的默认 denial reason SHALL 提示使用 `phase_log` / `backtrack` / `change_create` MCP（或等价「不要用 Write 改评估存储」），并继续支持 `%s` / `%t` 占位符。

MCP 服务端经 Node `fs` 写入 `workflow.json`（`change_create` / `writeEvalJson`）不受此 hook 拦截。

#### Scenario: workflow.json 写入被内置保护拒绝

**WHEN** `protect-files` 接收到 stdin `{ "tool_name": "Write", "tool_input": { "file_path": "openspec/changes/test-change/workflow.json" } }`
**AND** `openspec/config.json` 不包含 `write_protection`
**THEN** 输出 `permissionDecision: "deny"`
**AND** denial reason 包含 `workflow.json`

#### Scenario: eval.json 不再被内置规则拒绝

**WHEN** stdin 为 Write `openspec/changes/test-change/eval.json`
**AND** 无 `write_protection`
**THEN** 内置 glob SHALL NOT 单独导致 deny
**AND** 若该类写入需要拦截，用户可在 `write_protection.files` 中自行配置

#### Scenario: config.json 写入被内置保护拒绝

**WHEN** stdin 为 Write `openspec/config.json`
**AND** 无 `write_protection`
**THEN** 输出 `permissionDecision: "deny"`

#### Scenario: config.json Bash 重定向被拒绝

**WHEN** stdin 中 `tool_name` 为 `Bash` 且 command 包含 `echo '{}' > openspec/config.json`
**AND** 无 `write_protection`
**THEN** 输出 `permissionDecision: "deny"`

#### Scenario: proposal.md 不被内置规则拒绝

**WHEN** stdin 为 Write `openspec/changes/test-change/proposal.md`
**THEN** 内置 glob SHALL NOT 单独导致 deny

#### Scenario: Bash 重定向 workflow.json 被拒绝

**WHEN** `tool_name` 为 `Bash` 或 `Shell` 且 command 含 `> openspec/changes/x/workflow.json`
**THEN** `permissionDecision: "deny"`

## Module Contract

### Module: `plugins/dev-team/bin/src/hooks.ts`

| 符号 | 变更 |
|------|------|
| `loadPatterns` 内置 glob | **ADDED** `**/openspec/changes/**/workflow.json`（reason 提示 `phase_log` / `backtrack` / `change_create`）；**KEEP** `**/openspec/config.json`；**REMOVED** `**/openspec/changes/**/eval.json` |
| user-defined glob 合并 | 不变（`write_protection.files` 仍按原逻辑追加，可自行保护 `eval.json`） |
