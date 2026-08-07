## MODIFIED Requirements

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

## ADDED Requirements

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
