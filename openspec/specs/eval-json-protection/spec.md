## ADDED Requirements

### Requirement: 插件 hooks.json 声明 PreToolUse hook 拦截 Write/Edit 工具

`plugins/dev-team/hooks/hooks.json` 应包含一个 PreToolUse hook 配置，用于拦截 `Write` 和 `Edit` 内置工具调用。该 hook 应使用 `command` 类型，并引用脚本 `${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.sh`。

matcher 应同时匹配 `Write` 和 `Edit` 两个工具，确保对 eval.json 的写入和编辑操作均被拦截。

#### Scenario: Write 和 Edit 工具被 hook 匹配

- **当** agent 调用 `Write` 或 `Edit` 工具操作任意文件时
- **则** PreToolUse hook 触发 `protect-eval.sh` 脚本
- **且** 脚本通过 stdin 接收工具调用详情

#### Scenario: Hook 配置为合法 JSON

- **当** `plugins/dev-team/hooks/hooks.json` 被 `JSON.parse()` 解析时
- **则** 解析成功无错误
- **且** 对象包含 `hooks.PreToolUse` 数组，至少包含两项（一项匹配 `Write|Edit`，一项匹配 `Bash`）
- **且** 每项包含 `matcher`、`hooks` 数组，每个 hook 包含 `type` 和 `command` 字段

### Requirement: 插件 hooks.json 声明 PreToolUse hook 拦截 Bash 工具

`plugins/dev-team/hooks/hooks.json` 应包含额外的 PreToolUse hook 配置用于拦截 `Bash` 工具调用。该 hook 应与 Write/Edit 指向同一个脚本 `${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.sh`。

脚本内部根据 `tool_name` 字段区分 Write/Edit/Bash 并采用对应的拦截逻辑。

#### Scenario: Bash 工具被 hook 匹配

- **当** agent 通过 `Bash` 工具执行任意命令时
- **则** PreToolUse hook 触发 `protect-eval.sh` 脚本
- **且** 脚本通过 `tool_input.command` 获取命令字符串

#### Scenario: hooks.json 包含描述字段

- **当** 读取 `plugins/dev-team/hooks/hooks.json` 时
- **则** 应包含顶层 `description` 字段说明 hook 用途
- **且** 描述中应提及 "eval.json" 和 "phase_log"

### Requirement: protect-eval.sh 拦截 Write/Edit 对 eval.json 的写入

`plugins/dev-team/hooks/scripts/protect-eval.sh` 脚本应从 stdin 读取工具调用 JSON，根据 `tool_name` 字段区分处理逻辑：

- 若 `tool_name` 为 `Write` 或 `Edit`：提取 `tool_input.file_path`，检查路径是否匹配 `openspec/changes/**/eval.json`
- 若 `tool_name` 为 `Bash`：提取 `tool_input.command`，检查命令是否试图写入 eval.json

路径匹配 eval.json 时，脚本应输出包含以下字段的 JSON：
- `hookSpecificOutput.hookEventName` 设为 `"PreToolUse"`
- `hookSpecificOutput.permissionDecision` 设为 `"deny"`
- `hookSpecificOutput.permissionDecisionReason` 包含中文说明，推荐使用 `phase_log` MCP 工具

路径不匹配时，脚本输出 `permissionDecision: "allow"` 放行操作。

脚本应同时处理相对路径（如 `openspec/changes/test/eval.json`）和绝对路径（如 `D:/Projects/.../openspec/changes/test/eval.json`），通过后缀匹配 `eval.json`。

#### Scenario: Write 写入 eval.json 被拒绝

- **当** 脚本收到 stdin JSON，其中 `tool_name: "Write"` 且 `tool_input.file_path: "openspec/changes/test/eval.json"`
- **则** 脚本输出 `permissionDecision: "deny"`
- **且** `permissionDecisionReason` 包含 "eval.json" 和 "phase_log"

#### Scenario: Edit 修改 eval.json 被拒绝

- **当** 脚本收到 stdin JSON，其中 `tool_name: "Edit"` 且 `tool_input.file_path: "openspec/changes/my-change/eval.json"`
- **则** 脚本输出 `permissionDecision: "deny"`

#### Scenario: Write 写入非 eval.json 文件被放行

- **当** 脚本收到 stdin JSON，其中 `tool_input.file_path: "openspec/changes/test/design.md"`
- **则** 脚本输出 `permissionDecision: "allow"`

#### Scenario: Write 写入 changes 目录外的文件被放行

- **当** 脚本收到 stdin JSON，其中 `tool_input.file_path: "plugins/dev-team/bin/src/commands/phase-log.ts"`
- **则** 脚本输出 `permissionDecision: "allow"`

#### Scenario: 绝对路径指向 eval.json 被拒绝

- **当** 脚本收到 stdin JSON，其中 `tool_input.file_path: "D:/Projects/wps-claude-plugin/openspec/changes/demo/eval.json"`
- **则** 脚本输出 `permissionDecision: "deny"`

#### Scenario: 拒绝原因中包含推断的变更名称

- **当** 脚本收到 stdin JSON，其中 `tool_input.file_path: "openspec/changes/my-feature/eval.json"`
- **则** `permissionDecisionReason` 输出包含推断的变更名称 "my-feature"

### Requirement: protect-eval.sh 拦截 Bash 对 eval.json 的写入，但不拦截 Python

当 `tool_name` 为 `Bash` 时，脚本应分析 `tool_input.command` 命令字符串，判断是否在写入 eval.json。

以下模式视为「写入 eval.json」：
- 包含 `>` 或 `>>` 后接以 `eval.json` 结尾的路径（重定向或追加）
- 包含 `tee` 后接以 `eval.json` 结尾的路径
- 包含 heredoc（`<<`）且内容重定向至 eval.json
- 使用 `echo`、`cat`、`printf` 等输出命令配合重定向写入 eval.json

以下情况不应视为「写入 eval.json」：
- 命令以 `python`、`python3` 或 `node` 开头（受信任的程序，内部文件操作由脚本自身控制）
- 命令仅读取 eval.json（如 `cat eval.json` 无重定向、`grep xxx eval.json`）

#### Scenario: Bash echo 重定向到 eval.json 被拒绝

- **当** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "echo '[]' > openspec/changes/test/eval.json"`
- **则** 脚本输出 `permissionDecision: "deny"`

#### Scenario: Bash cat heredoc 写入 eval.json 被拒绝

- **当** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF"`
- **则** 脚本输出 `permissionDecision: "deny"`

#### Scenario: Bash tee 写入 eval.json 被拒绝

- **当** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "echo '[]' | tee openspec/changes/test/eval.json"`
- **则** 脚本输出 `permissionDecision: "deny"`

#### Scenario: Python 命令写入 eval.json 被放行

- **当** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "python plugins/dev-team/utils/eval-check.py --change test"`
- **则** 脚本输出 `permissionDecision: "allow"`
- **且** 脚本不检查 Python 脚本内部的文件操作

#### Scenario: Bash 只读 eval.json 被放行

- **当** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "cat openspec/changes/test/eval.json"`
- **则** 脚本输出 `permissionDecision: "allow"`（只读访问不拦截）

#### Scenario: 无关 Bash 命令被放行

- **当** 脚本收到 stdin JSON，其中 `tool_name: "Bash"` 且 `tool_input.command: "ls -la"`
- **则** 脚本输出 `permissionDecision: "allow"`

### Requirement: Hook 拒绝原因引导 agent 使用 phase_log

PreToolUse hook 返回 `permissionDecision: "deny"` 时，`permissionDecisionReason` 字段应包含：
1. 明确说明 eval.json 只能通过 phase_log MCP 工具写入
2. 确切的 MCP 工具名 `mcp__plugin_dev-team_dev-team__phase_log`
3. 从路径（Write/Edit）或命令字符串（Bash）推断的变更名称
4. 简要说明为何不允许直接写入（数据完整性、审计追溯）

拒绝原因使用**中文（简体中文）**书写，与项目约定保持一致。

#### Scenario: 拒绝原因包含 phase_log 工具名和变更名称

- **当** protect-eval.sh 拒绝 Write 写入 `openspec/changes/my-feature/eval.json` 时
- **则** `permissionDecisionReason` 应包含：
  - 字符串 "mcp__plugin_dev-team_dev-team__phase_log"
  - 推断的变更名称 "my-feature"
  - 中文说明

#### Scenario: Bash 拒绝原因包含替代建议

- **当** protect-eval.sh 拒绝一条写入 eval.json 的 Bash 命令时
- **则** `permissionDecisionReason` 应包含 "phase_log" 或 "mcp__plugin_dev-team_dev-team__phase_log"
- **且** 说明使用中文

### Requirement: phase_log MCP 工具不受影响继续写入 eval.json

MCP 工具 `mcp__plugin_dev-team_dev-team__phase_log` 不应受 PreToolUse hook 影响。Hook 仅拦截内置工具（Write、Edit、Bash），MCP 工具不会被针对内置工具的 PreToolUse hook 拦截。

`phase_log` 工具应继续按设计读取、校验和写入 eval.json，所有现有校验逻辑（verdict 值域检查、report 长度限制、回溯 stale 传播、skipped 一致性检查、attempt 自动递增、时间戳生成）保持不变。

#### Scenario: Hook 激活后 phase_log 仍可写入 eval.json

- **当** agent 调用 `mcp__plugin_dev-team_dev-team__phase_log`，参数为 `{change: "test", phase: "01-requirements", verdict: "pass", report: "所有检查通过", items: [...]}`
- **则** 调用成功
- **且** eval.json 包含新条目
- **且** hook 未干扰 MCP 工具

#### Scenario: Agent 被 hook 拒绝后自我纠正使用 phase_log

- **当** agent 先尝试 Write eval.json（被 hook 拒绝并收到建议），随后改用 `mcp__plugin_dev-team_dev-team__phase_log`
- **则** `phase_log` 调用成功
- **且** 条目正确追加到 eval.json

### Requirement: Hook 脚本语法合法且可执行

`protect-eval.sh` 应是有效的 POSIX shell（或 bash）脚本，通过 `bash -n` 语法检查无错误。脚本应使用 `${CLAUDE_PLUGIN_ROOT}` 变量引用插件内部路径（如有需要）。

脚本应具有可执行权限（在 Unix-like 系统上设置 execute permission bit）。

#### Scenario: protect-eval.sh 通过语法检查

- **当** 执行 `bash -n plugins/dev-team/hooks/scripts/protect-eval.sh` 时
- **则** 命令退出码为 0 且无错误输出

### Requirement: Hook 优雅处理异常或空 stdin

PreToolUse hook 脚本应处理 stdin JSON 缺少预期字段的情况。若 `tool_input` 缺失或 `file_path`/`command`/`tool_name` 无法提取，脚本应默认为 `permissionDecision: "allow"`（失败放行），避免因意外输入格式阻塞合法操作。

#### Scenario: 缺少 tool_input 字段时默认为放行

- **当** 脚本收到 stdin JSON 不包含 `tool_input.file_path` 或值为空
- **则** 脚本输出 `permissionDecision: "allow"`

### Requirement: 插件版本号递增

`plugins/dev-team/.claude-plugin/plugin.json` 文件的 `version` 字段应递增（patch bump），以反映插件新增的 hook 文件。上一版本 `2.5.9` 应更新为更高版本号（如 `2.5.10`）。

#### Scenario: 插件版本号已升级

- **当** 读取 `plugins/dev-team/.claude-plugin/plugin.json` 时
- **则** `version` 字段值应大于 `"2.5.9"`
- **且** 版本格式遵循 semver

### Requirement: 插件安装时保护自动激活

eval.json 保护应在 dev-team 插件被启用的 Claude Code 项目中自动激活。无需逐项目配置、手动注册 hook 或修改 settings 文件即可生效。

Claude Code 应按照插件 hook 发现机制，自动发现 `plugins/dev-team/hooks/hooks.json` 中声明的 hook 并注册。

#### Scenario: 新项目安装插件后保护生效

- **当** 新 Claude Code 项目启用了 dev-team 插件时
- **则** `plugins/dev-team/hooks/hooks.json` 中的 hook 自动注册
- **且** 无需额外配置即可拒绝 Write/Edit 对 eval.json 的写入

## Module Contract

### 插件 Hook 声明：plugins/dev-team/hooks/hooks.json

| 字段 | 类型 | 描述 |
|------|------|------|
| `description` | `string` | hook 用途的人类可读描述 |
| `hooks.PreToolUse` | `object[]` | PreToolUse hook 配置数组 |
| `hooks.PreToolUse[0].matcher` | `string` | `"Write\|Edit"` — 匹配 Write 和 Edit 内置工具 |
| `hooks.PreToolUse[0].hooks[0].type` | `string` | `"command"` — hook 类型 |
| `hooks.PreToolUse[0].hooks[0].command` | `string` | `"\"${CLAUDE_PLUGIN_ROOT}\"/hooks/scripts/protect-eval.sh"` |
| `hooks.PreToolUse[1].matcher` | `string` | `"Bash"` — 匹配 Bash 工具 |
| `hooks.PreToolUse[1].hooks[0].type` | `string` | `"command"` — hook 类型 |
| `hooks.PreToolUse[1].hooks[0].command` | `string` | `"\"${CLAUDE_PLUGIN_ROOT}\"/hooks/scripts/protect-eval.sh"` |

### Hook 脚本：plugins/dev-team/hooks/scripts/protect-eval.sh

| 方面 | 描述 |
|------|------|
| **输入** | stdin JSON，包含 `tool_name: string` 和 `tool_input: { file_path?: string, command?: string }` |
| **逻辑（Write/Edit）** | 提取 `file_path`，后缀匹配 `openspec/changes/**/eval.json` 模式 |
| **逻辑（Bash）** | 提取 `command`，检测 eval.json 写入模式（`>`、`>>`、`tee`、heredoc）。`python`/`python3` 开头的命令豁免 |
| **命中动作** | 输出 JSON `hookSpecificOutput.permissionDecision: "deny"` + 中文拒绝原因 + `phase_log` 建议 |
| **未命中动作** | 输出 JSON `hookSpecificOutput.permissionDecision: "allow"` |
| **错误处理** | 缺少 `file_path`/`command`/`tool_name` 或输入格式异常 → 默认 `allow` |

### 插件配置：plugins/dev-team/.claude-plugin/plugin.json

| 字段 | 值 |
|------|-----|
| `version` | `"2.5.10"`（从 `"2.5.9"` 的 patch bump） |

### MCP 工具（不受此变更影响）

| 工具名 | 状态 | 描述 |
|--------|------|------|
| `mcp__plugin_dev-team_dev-team__phase_log` | 不变 | 继续写入 eval.json — MCP 工具不被内置工具 hook 拦截 |
| `mcp__plugin_dev-team_dev-team__phase_check` | 不变 | 读取 eval.json 进行门控校验 |
| `mcp__plugin_dev-team_dev-team__phase_next` | 不变 | 读取 eval.json 进行阶段决策 |
