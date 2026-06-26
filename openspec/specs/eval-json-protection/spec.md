## Requirements

### Requirement: 插件 hooks.json 声明 PreToolUse hook 拦截 Write/Edit 工具

`plugins/dev-team/hooks/hooks.json` 应包含一个 PreToolUse hook 配置，用于拦截 `Write` 和 `Edit` 内置工具调用。该 hook 应使用 `command` 类型，并引用脚本 `${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs`，通过 `node` 调用：

```
node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs"
```

matcher 应同时匹配 `Write` 和 `Edit` 两个工具，确保对 eval.json 的写入和编辑操作均被拦截。

#### Scenario: Write 和 Edit 工具被 hook 匹配

- **WHEN** agent 调用 `Write` 或 `Edit` 工具操作任意文件时
- **THEN** PreToolUse hook 触发 `protect-eval.mjs` 脚本
- **AND** 脚本通过 stdin 接收工具调用详情

#### Scenario: Hook 配置为合法 JSON

- **WHEN** `plugins/dev-team/hooks/hooks.json` 被 `JSON.parse()` 解析时
- **THEN** 解析成功无错误
- **AND** 对象包含 `hooks.PreToolUse` 数组，至少包含两项（一项匹配 `Write|Edit`，一项匹配 `Bash`）
- **AND** 每项包含 `matcher`、`hooks` 数组，每个 hook 包含 `type` 和 `command` 字段
- **AND** 所有 hook 的 `command` 字段以 `node` 开头并引用 `.mjs` 脚本

#### Scenario: Windows PowerShell 环境下 PreToolUse hook 可执行

- **WHEN** 在 Windows 上执行 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs"` 并传入合法 stdin JSON
- **THEN** 命令 exit code 为 `0`
- **AND** stdout 输出合法 JSON，包含 `hookSpecificOutput.permissionDecision`

### Requirement: 插件 hooks.json 声明 PreToolUse hook 拦截 Bash 工具

`plugins/dev-team/hooks/hooks.json` 应包含额外的 PreToolUse hook 配置用于拦截 `Bash` 工具调用。该 hook 应与 Write/Edit 指向同一个脚本 `${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs`，通过 `node` 调用。

脚本内部根据 `tool_name` 字段区分 Write/Edit/Bash 并采用对应的拦截逻辑。

#### Scenario: Bash 工具被 hook 匹配

- **WHEN** agent 通过 `Bash` 工具执行任意命令时
- **THEN** PreToolUse hook 触发 `protect-eval.mjs` 脚本
- **AND** 脚本通过 `tool_input.command` 获取命令字符串

#### Scenario: hooks.json 包含描述字段

- **WHEN** 读取 `plugins/dev-team/hooks/hooks.json` 时
- **THEN** 应包含顶层 `description` 字段说明 hook 用途
- **AND** 描述中应提及 "eval.json" 和 "phase_log"

### Requirement: protect-eval.mjs 拦截 Write/Edit 对 eval.json 的写入

`plugins/dev-team/hooks/scripts/protect-eval.mjs` 脚本应从 stdin 读取工具调用 JSON，使用 `JSON.parse` 解析，根据 `tool_name` 字段区分处理逻辑：

- 若 `tool_name` 为 `Write` 或 `Edit`：提取 `tool_input.file_path`，检查路径是否匹配 `openspec/changes/**/eval.json`
- 若 `tool_name` 为 `Bash`：提取 `tool_input.command`，检查命令是否试图写入 eval.json

路径匹配 eval.json 时，脚本应输出包含以下字段的 JSON：
- `hookSpecificOutput.hookEventName` 设为 `"PreToolUse"`
- `hookSpecificOutput.permissionDecision` 设为 `"deny"`
- `hookSpecificOutput.permissionDecisionReason` 包含中文说明，推荐使用 `phase_log` MCP 工具

路径不匹配时，脚本输出 `permissionDecision: "allow"` 放行操作。

脚本应同时处理相对路径（如 `openspec/changes/test/eval.json`）和绝对路径（如 `D:/Projects/.../openspec/changes/test/eval.json`），通过后缀匹配 `eval.json`。路径比较前 SHALL 将反斜杠归一化为正斜杠。

#### Scenario: Write 写入 eval.json 被拒绝

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Write"` 且 `tool_input.file_path: "openspec/changes/test/eval.json"`
- **THEN** 脚本输出 `permissionDecision: "deny"`
- **AND** `permissionDecisionReason` 包含 "eval.json" 和 "phase_log"

#### Scenario: Edit 修改 eval.json 被拒绝

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Edit"` 且 `tool_input.file_path: "openspec/changes/my-change/eval.json"`
- **THEN** 脚本输出 `permissionDecision: "deny"`

#### Scenario: Write 写入非 eval.json 文件被放行

- **WHEN** 脚本收到 stdin JSON，其中 `tool_input.file_path: "openspec/changes/test/design.md"`
- **THEN** 脚本输出 `permissionDecision: "allow"`

#### Scenario: Write 写入 changes 目录外的文件被放行

- **WHEN** 脚本收到 stdin JSON，其中 `tool_input.file_path: "plugins/dev-team/bin/src/commands/phase-log.ts"`
- **THEN** 脚本输出 `permissionDecision: "allow"`

#### Scenario: 绝对路径指向 eval.json 被拒绝

- **WHEN** 脚本收到 stdin JSON，其中 `tool_input.file_path: "D:/Projects/wps-claude-plugin/openspec/changes/demo/eval.json"`
- **THEN** 脚本输出 `permissionDecision: "deny"`

#### Scenario: 拒绝原因中包含推断的变更名称

- **WHEN** 脚本收到 stdin JSON，其中 `tool_input.file_path: "openspec/changes/my-feature/eval.json"`
- **THEN** `permissionDecisionReason` 输出包含推断的变更名称 "my-feature"

### Requirement: protect-eval.mjs 拦截 Bash 对 eval.json 的写入，但不拦截 Python/Node

当 `tool_name` 为 `Bash` 时，脚本应分析 `tool_input.command` 命令字符串，判断是否在写入 eval.json。

以下模式视为「写入 eval.json」：
- 包含 `>` 或 `>>` 后接以 `eval.json` 结尾的路径（重定向或追加，排除 `->` 和 `<<>`）
- 包含 `tee` 后接以 `eval.json` 结尾的路径
- 包含 heredoc（`<<`）且命令中包含 eval.json
- 使用 `>&` 重定向至 eval.json

以下情况不应视为「写入 eval.json」：
- 命令以 `python`、`python3` 或 `node` 开头（受信任的程序，内部文件操作由脚本自身控制）
- 命令仅读取 eval.json（如 `cat eval.json` 无重定向、`grep xxx eval.json`）

#### Scenario: Bash echo 重定向到 eval.json 被拒绝

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "echo '[]' > openspec/changes/test/eval.json"`
- **THEN** 脚本输出 `permissionDecision: "deny"`

#### Scenario: Bash cat heredoc 写入 eval.json 被拒绝

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF"`
- **THEN** 脚本输出 `permissionDecision: "deny"`

#### Scenario: Bash tee 写入 eval.json 被拒绝

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "echo '[]' | tee openspec/changes/test/eval.json"`
- **THEN** 脚本输出 `permissionDecision: "deny"`

#### Scenario: Python 命令写入 eval.json 被放行

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "python plugins/dev-team/utils/eval-check.py --change test"`
- **THEN** 脚本输出 `permissionDecision: "allow"`
- **AND** 脚本不检查 Python 脚本内部的文件操作

#### Scenario: Node 命令被放行

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "node scripts/write-eval.mjs"`
- **THEN** 脚本输出 `permissionDecision: "allow"`

#### Scenario: Bash 只读 eval.json 被放行

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "cat openspec/changes/test/eval.json"`
- **THEN** 脚本输出 `permissionDecision: "allow"`（只读访问不拦截）

#### Scenario: 无关 Bash 命令被放行

- **WHEN** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "ls -la"`
- **THEN** 脚本输出 `permissionDecision: "allow"`

### Requirement: Hook 脚本可被 Node.js 直接执行

`protect-eval.mjs` 应是有效的 Node.js ES Module 脚本，可通过 `node plugins/dev-team/hooks/scripts/protect-eval.mjs` 直接执行，无需 bash 或额外依赖。脚本应使用 `${CLAUDE_PLUGIN_ROOT}` 环境变量引用插件内部路径（如有需要）。

#### Scenario: protect-eval.mjs 可被 node 直接执行

- **WHEN** 执行 `node plugins/dev-team/hooks/scripts/protect-eval.mjs` 并传入合法 stdin JSON
- **THEN** 命令退出码为 0 且无未捕获异常
- **AND** stdout 输出合法 JSON

### Requirement: Hook 优雅处理异常或空 stdin

PreToolUse hook 脚本应处理 stdin JSON 缺少预期字段的情况。若 stdin 为空、`tool_input` 缺失或 `file_path`/`command`/`tool_name` 无法提取，脚本应默认为 `permissionDecision: "allow"`（失败放行），避免因意外输入格式阻塞合法操作。

#### Scenario: 缺少 tool_input 字段时默认为放行

- **WHEN** 脚本收到 stdin JSON 不包含 `tool_input.file_path` 或值为空
- **THEN** 脚本输出 `permissionDecision: "allow"`

#### Scenario: 空 stdin 时默认为放行

- **WHEN** 脚本收到空 stdin
- **THEN** 脚本输出 `permissionDecision: "allow"`

#### Scenario: 无效 JSON 时默认为放行

- **WHEN** 脚本收到无法 `JSON.parse` 的 stdin 内容
- **THEN** 脚本输出 `permissionDecision: "allow"`

### Requirement: Hook 拒绝原因引导 agent 使用 phase_log

PreToolUse hook 返回 `permissionDecision: "deny"` 时，`permissionDecisionReason` 字段应包含：
1. 明确说明 eval.json 只能通过 phase_log MCP 工具写入
2. 确切的 MCP 工具名 `mcp__plugin_dev-team_dev-team__phase_log`
3. 从路径（Write/Edit）或命令字符串（Bash）推断的变更名称
4. 简要说明为何不允许直接写入（数据完整性、审计追溯）

拒绝原因使用**中文（简体中文）**书写，与项目约定保持一致。

#### Scenario: 拒绝原因包含 phase_log 工具名和变更名称

- **WHEN** protect-eval.mjs 拒绝 Write 写入 `openspec/changes/my-feature/eval.json` 时
- **THEN** `permissionDecisionReason` 应包含：
  - 字符串 "mcp__plugin_dev-team_dev-team__phase_log"
  - 推断的变更名称 "my-feature"
  - 中文说明

#### Scenario: Bash 拒绝原因包含替代建议

- **WHEN** protect-eval.mjs 拒绝一条写入 eval.json 的 Bash 命令时
- **THEN** `permissionDecisionReason` 应包含 "phase_log" 或 "mcp__plugin_dev-team_dev-team__phase_log"
- **AND** 说明使用中文

### Requirement: 插件版本号递增

`plugins/dev-team/.claude-plugin/plugin.json` 文件的 `version` 字段应递增（patch bump），以反映 hook 脚本变更。

#### Scenario: 插件版本号已升级

- **WHEN** 读取 `plugins/dev-team/.claude-plugin/plugin.json` 时
- **THEN** `version` 字段值遵循 semver

### Requirement: phase_log MCP 工具不受影响继续写入 eval.json

MCP 工具 `mcp__plugin_dev-team_dev-team__phase_log` 不应受 PreToolUse hook 影响。Hook 仅拦截内置工具（Write、Edit、Bash），MCP 工具不会被针对内置工具的 PreToolUse hook 拦截。

`phase_log` 工具应继续按设计读取、校验和写入 eval.json，所有现有校验逻辑（verdict 值域检查、report 长度限制、回溯 stale 传播、skipped 一致性检查、attempt 自动递增、时间戳生成）保持不变。

#### Scenario: Hook 激活后 phase_log 仍可写入 eval.json

- **WHEN** agent 调用 `mcp__plugin_dev-team_dev-team__phase_log`，参数为 `{change: "test", phase: "proposal", verdict: "pass", report: "所有检查通过", items: [...]}`
- **THEN** 调用成功
- **AND** eval.json 包含新条目
- **AND** hook 未干扰 MCP 工具

#### Scenario: Agent 被 hook 拒绝后自我纠正使用 phase_log

- **WHEN** agent 先尝试 Write eval.json（被 hook 拒绝并收到建议），随后改用 `mcp__plugin_dev-team_dev-team__phase_log`
- **THEN** `phase_log` 调用成功
- **AND** 条目正确追加到 eval.json

### Requirement: 插件安装时保护自动激活

eval.json 保护应在 dev-team 插件被启用的 Claude Code 项目中自动激活。无需逐项目配置、手动注册 hook 或修改 settings 文件即可生效。

Claude Code 应按照插件 hook 发现机制，自动发现 `plugins/dev-team/hooks/hooks.json` 中声明的 hook 并注册。

#### Scenario: 新项目安装插件后保护生效

- **WHEN** 新 Claude Code 项目启用了 dev-team 插件时
- **THEN** `plugins/dev-team/hooks/hooks.json` 中的 hook 自动注册
- **AND** 无需额外配置即可拒绝 Write/Edit 对 eval.json 的写入

## MODIFIED Requirements

### Requirement: phase_log backtrack validation uses prefix-free phase IDs
When `runPhaseLog()` validates backtrack targets against the workflow phase table, both the `phase` parameter and `backtrack_to` targets SHALL use prefix-free phase IDs.

The `handleBacktrackMarking()` function SHALL use the new phase table (with prefix-free IDs) to validate:
1. `options.phase` exists in the phase table
2. Each backtrack target exists in the phase table
3. Each backtrack target precedes `options.phase` in array order

When a backtrack target is not found in the prefix-free phase table, the error message SHALL list available phases using the new IDs.

#### Scenario: phase_log validates backtrack with prefix-free IDs
- **WHEN** `phase_log` is called with `phase: "unit-test"`, `backtrack_to: "test-gen"`
- **AND** workflow phase table uses prefix-free IDs
- **THEN** `handleBacktrackMarking()` validates `"test-gen"` exists in the table
- **AND** the call succeeds (phase exists and precedes current)

#### Scenario: phase_log rejects invalid backtrack with prefix-free ID in error
- **WHEN** `phase_log` is called with `phase: "unit-test"`, `backtrack_to: "old-05-implement"`
- **AND** the old ID `"old-05-implement"` is not in the prefix-free phase table
- **THEN** the call is rejected with an error
- **AND** the error message lists available phases using prefix-free IDs (e.g., `proposal`, `dev-design`, `implement`, etc.)

## Module Contract

### 插件 Hook 声明：plugins/dev-team/hooks/hooks.json

| 字段 | 类型 | 描述 |
|------|------|------|
| `description` | `string` | hook 用途的人类可读描述 |
| `hooks.PreToolUse` | `object[]` | PreToolUse hook 配置数组 |
| `hooks.PreToolUse[0].matcher` | `string` | `"Write\|Edit"` — 匹配 Write 和 Edit 内置工具 |
| `hooks.PreToolUse[0].hooks[0].type` | `string` | `"command"` — hook 类型 |
| `hooks.PreToolUse[0].hooks[0].command` | `string` | `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs"` |
| `hooks.PreToolUse[1].matcher` | `string` | `"Bash"` — 匹配 Bash 工具 |
| `hooks.PreToolUse[1].hooks[0].type` | `string` | `"command"` — hook 类型 |
| `hooks.PreToolUse[1].hooks[0].command` | `string` | `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs"` |

### Hook 脚本：plugins/dev-team/hooks/scripts/protect-eval.mjs

| 方面 | 描述 |
|------|------|
| **运行时** | Node.js ES Module（`.mjs`），通过 `node` 调用 |
| **输入** | stdin JSON，包含 `tool_name: string` 和 `tool_input: { file_path?: string, command?: string }` |
| **解析** | `JSON.parse` 读取 stdin；解析失败时 fail-open |
| **逻辑（Write/Edit）** | 提取 `file_path`，后缀匹配 `openspec/changes/**/eval.json` 模式 |
| **逻辑（Bash）** | 提取 `command`，检测 eval.json 写入模式（`>`、`>>`、`tee`、heredoc、`>&`）。`python`/`python3`/`node` 开头的命令豁免 |
| **命中动作** | 输出 JSON `hookSpecificOutput.permissionDecision: "deny"` + 中文拒绝原因 + `phase_log` 建议 |
| **未命中动作** | 输出 JSON `hookSpecificOutput.permissionDecision: "allow"` |
| **错误处理** | 空 stdin、缺少 `file_path`/`command`/`tool_name` 或 JSON 解析失败 → 默认 `allow` |

### phase-log.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `runPhaseLog()` | UNCHANGED | No API change; references phase table from `workflow.ts` which now has prefix-free IDs |
| `handleBacktrackMarking()` | UNCHANGED | No logic change; validates against whatever phase table `getPhaseTable()` returns |
| `resolveVerdict()` | UNCHANGED | No phase ID dependency |

### MCP 工具（不受此变更影响）

| 工具名 | 状态 | 描述 |
|--------|------|------|
| `mcp__plugin_dev-team_dev-team__phase_log` | 不变 | 继续写入 eval.json — MCP 工具不被内置工具 hook 拦截 |
| `mcp__plugin_dev-team_dev-team__phase_check` | 不变 | 读取 eval.json 进行门控校验 |
| `mcp__plugin_dev-team_dev-team__phase_next` | 不变 | 读取 eval.json 进行阶段决策 |
