# protect-files-hook Specification

## Purpose
PreToolUse write-protection hook packaged as a hooks CJS subcommand, with Claude and Cursor tool-name parity (including Shell/StrReplace for home image).

## Requirements

### Requirement: protect-files hook 迁入 bin 构建体系

protect-files hook 脚本 SHALL 从 `hooks/scripts/protect-files.mjs` 迁入 `bin/src/hooks.ts`，作为 hooks CJS bundle 的子命令 `protect-files` 暴露（落盘文件名由 assemble/`namePrefix` 决定：插件侧 `{hooks}.cjs` 形态，home 侧带 `dev-team_` 前缀）。

hook 的 PreToolUse 入口 SHALL 保留，通过组装后的 hooks 配置注册（Claude nested 或 Cursor native），command 指向对应产物中的 hooks bin + `protect-files`。

hook SHALL 拦截的 tool 范围包括：`Write`, `Edit`, `Bash`, `PowerShell`，以及 Cursor 侧工具名 `Shell` 与 `StrReplace`。`Shell` SHALL 与 `Bash` 使用同一命令写入检测逻辑；`StrReplace` SHALL 与 `Edit` 使用同一文件路径保护逻辑。既有 Claude 工具名行为 MUST 保持兼容。

hook SHALL 读取 `openspec/config.json` 的方式从手写 `loadConfig` 改为复用 `bin/src/lib/config.ts` 的 `readConfig` 函数。

#### Scenario: hook 注册命令指向 hooks CJS protect-files

- **WHEN** 检查组装后的 hooks 配置中 PreToolUse hook 的 command 字段
- **THEN** 所有 protect-files 相关 command 引用产物 `bin` 下的 hooks CJS 与 `protect-files` 子命令
- **AND** MUST NOT 引用已删除的 `hooks/scripts/protect-files.mjs`

#### Scenario: hooks.ts protect-files 子命令可被调用

- **WHEN** 执行 `node <hooks.cjs> protect-files` 并传入有效 stdin JSON
- **THEN** 进程 exit code 为 0
- **AND** stdout 输出含 `hookSpecificOutput.permissionDecision` 的 JSON

#### Scenario: config 读取复用 readConfig

- **WHEN** protect-files 子命令启动
- **THEN** 它使用 `import { readConfig } from './lib/config'` 读取配置
- **AND** `readConfig` 执行 Zod schema 验证

#### Scenario: Cursor Shell 与 Bash 同等拦截

- **WHEN** stdin 中 `tool_name` 为 `Shell` 且 command 包含对受保护路径的 shell 重定向写入
- **THEN** hook 输出 `permissionDecision: "deny"`
- **AND** 行为与同等 command 在 `tool_name: "Bash"` 下一致

#### Scenario: Cursor StrReplace 与 Edit 同等拦截

- **WHEN** stdin 中 `tool_name` 为 `StrReplace` 且 `file_path` 匹配内置或用户 write_protection glob
- **THEN** hook 输出 `permissionDecision: "deny"`
- **AND** 行为与同等路径在 `tool_name: "Edit"` 下一致

#### Scenario: 既有 Claude 工具名仍然有效

- **WHEN** stdin 中 `tool_name` 为 `Write`、`Edit`、`Bash` 或 `PowerShell` 且命中保护规则
- **THEN** hook 仍输出 `permissionDecision: "deny"`

### Requirement: Built-in default protections (迁移不变)

内置保护规则 SHALL 保持不变 — 仅实现方式从手写 globToRegex 改为 picomatch。

内置保护文件列表不变：
- 任何匹配 `openspec/changes/*/eval.json` 的文件（glob 模式 `**/openspec/changes/*/eval.json`）
- 任何匹配 `openspec/config.json` 的文件（glob 模式 `**/openspec/config.json`）

这些内置默认保护在 `write_protection` 不存在、为空或 `config.json` 无法解析时 SHALL 仍被强制执行。

默认 denial reason SHALL 继续使用 `%s`（文件路径）和 `%t`（工具名称）占位符。

#### Scenario: eval.json 写入被内置保护拒绝

**WHEN** `dev-team-hooks.cjs protect-files` 接收到 stdin `{ "tool_name": "Write", "tool_input": { "file_path": "openspec/changes/test-change/eval.json" } }`
**AND** `openspec/config.json` 不包含 `write_protection`
**THEN** 输出 `permissionDecision: "deny"`
**AND** denial reason 包含 `eval.json` 和工具名称

#### Scenario: config.json 写入被内置保护拒绝

**WHEN** `dev-team-hooks.cjs protect-files` 接收到 stdin `{ "tool_name": "Write", "tool_input": { "file_path": "openspec/config.json" } }`
**AND** `openspec/config.json` 不包含 `write_protection`
**THEN** 输出 `permissionDecision: "deny"`

#### Scenario: config.json Bash 重定向被拒绝

**WHEN** `dev-team-hooks.cjs protect-files` 接收到 stdin 中 `tool_name: "Bash"` 且 command 包含 `echo '{}' > openspec/config.json`
**AND** `openspec/config.json` 不包含 `write_protection`
**THEN** 输出 `permissionDecision: "deny"`

### Requirement: User-defined glob protection (改用 picomatch)

matchGlob 实现 SHALL 从手写 `globToRegex` 替换为 `bin/src/lib/glob.ts` 的 `matchGlob` 函数，基于 picomatch 库。

路径归一化逻辑 SHALL 继续使用 `toForwardSlash`（源自 `lib/glob.ts`），将反斜杠转换为正斜杠。

当目标文件路径匹配任一用户配置的 `write_protection.files[].glob` 时，hook SHALL 返回 `permissionDecision: "deny"`。

#### Scenario: 用户 glob 匹配目标文件路径

**WHEN** `openspec/config.json` 包含 `{ "write_protection": { "files": [{ "glob": "secrets/**" }] } }`
**AND** stdin 中 `tool_name: "Edit"` 且 `file_path` 为 `secrets/keys.yml`
**THEN** hook 输出 `permissionDecision: "deny"`

#### Scenario: 未匹配任何 glob 的文件放行

**WHEN** `openspec/config.json` 包含 `{ "write_protection": { "files": [{ "glob": "secrets/**" }] } }`
**AND** stdin 中 `tool_name: "Write"` 且 `file_path` 为 `src/app.ts`
**THEN** hook 输出 `permissionDecision: "allow"`

#### Scenario: Windows 反斜杠路径匹配 glob

**WHEN** `openspec/config.json` 包含 `{ "write_protection": { "files": [{ "glob": "secrets/**" }] } }`
**AND** stdin 中 `tool_name: "Write"` 且 `file_path` 为 `secrets\keys.yml`
**THEN** hook 输出 `permissionDecision: "deny"`

### Requirement: Custom denial reason (不变)

自定义 denial reason 行为与占位符规则 SHALL 与迁移前保持一致。

#### Scenario: 自定义 reason 含文件路径占位符

**WHEN** `openspec/config.json` 包含 `{ "write_protection": { "files": [{ "glob": "secrets/*", "reason": "File '%s' is protected." }] } }`
**AND** stdin 中 `tool_name: "Write"` 且 `file_path` 为 `secrets/api.key`
**THEN** denial reason 包含 `"File '...secrets/api.key' is protected."`

#### Scenario: 自定义 reason 含工具名称占位符

**WHEN** `openspec/config.json` 包含 `{ "write_protection": { "files": [{ "glob": "secrets/*", "reason": "Protected via %t" }] } }`
**AND** stdin 中 `tool_name: "Write"` 且 `file_path` 为 `secrets/api.key`
**THEN** denial reason 包含 `"Protected via Write"`

### Requirement: Fail-open behavior preserved (不变)

fail-open 行为规则 SHALL 与迁移前完全一致。

#### Scenario: 空 stdin 返回 allow

**WHEN** hook 接收到空 stdin
**THEN** 输出 `permissionDecision: "allow"`

#### Scenario: 无效 JSON stdin 返回 allow

**WHEN** stdin 为 `{not json`
**THEN** 输出 `permissionDecision: "allow"`

#### Scenario: 缺少 tool_input.file_path 返回 allow

**WHEN** stdin 为 `{ "tool_name": "Write", "tool_input": {} }`
**THEN** 输出 `permissionDecision: "allow"`

#### Scenario: config.json 解析失败仍执行内置保护

**WHEN** `openspec/config.json` 包含无效 JSON
**AND** stdin 中 `tool_name: "Write"` 且 `file_path` 为 `openspec/changes/test/eval.json`
**THEN** 内置保护仍生效，输出 `permissionDecision: "deny"`

### Requirement: Python/Node exemption preserved (不变)

Python/Node 命令豁免规则 SHALL 与迁移前完全一致。

#### Scenario: Python 命令写入 eval.json 豁免

**WHEN** stdin 中 `tool_name: "Bash"` 且 command 为 `python scripts/deploy.py --eval openspec/changes/test/eval.json`
**THEN** 输出 `permissionDecision: "allow"`

#### Scenario: Node 命令写入 config.json 豁免

**WHEN** stdin 中 `tool_name: "Bash"` 且 command 为 `node scripts/setup.js openspec/config.json`
**THEN** 输出 `permissionDecision: "allow"`

### Requirement: hooksProfile matchers include Cursor tool names for home

When assembling `cursorNative` / Cursor-oriented protect-files matchers for the home image (and any profile that targets Cursor tool names), matcher lists SHALL include `Shell` and `StrReplace` in addition to or instead of Claude-only names as appropriate for that profile, so that protect-files is actually invoked under Cursor user-level hooks.

#### Scenario: cursorHome protect-files matcher covers Shell and StrReplace

- **WHEN** assemble emits `cursorHome` `hooks.json` entries for protect-files
- **THEN** the matcher coverage SHALL include `Shell` and `StrReplace` (directly or via an equivalent Cursor matcher list)
- **AND** the command SHALL invoke the home-prefixed hooks bin `protect-files` subcommand

## Module Contract

### Hook: PreToolUse — hooks CJS `protect-files`（更新）

| Property | Description |
|----------|-------------|
| **Entry** | `plugins/dev-team/bin/src/hooks.ts`（子命令: `protect-files`） |
| **Bundle** | assemble 后的 hooks CJS（plugin: `hooks.cjs`；home: `dev-team_hooks.cjs`） |
| **Tools** | `Write`, `Edit`, `Bash`, `PowerShell`, `Shell`, `StrReplace` |
| **Shell 语义** | 与 `Bash` 相同的 command 写入检测 |
| **StrReplace 语义** | 与 `Edit` 相同的 `file_path` 保护 |
| **Input** | stdin JSON: `{ tool_name, tool_input: { file_path?, command? } }` |
| **Output** | stdout JSON: `{ hookSpecificOutput: { permissionDecision, permissionDecisionReason? } }` |
| **Config source** | `openspec/config.json` → `write_protection`（via `readConfig`） |
| **Fail-open** | 输入异常 → allow |

### Built-in defaults (preserved)

| Property | Description |
|----------|-------------|
| **Built-in defaults** | `**/openspec/changes/*/eval.json`, `**/openspec/config.json` |
| **Glob library** | `picomatch` via `lib/glob.ts` `matchGlob` |
| **Path normalization** | `lib/glob.ts` `toForwardSlash` (backslash → forward slash) |
| **Fallback** | Config parse failure → built-in defaults only |
