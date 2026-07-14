## MODIFIED Requirements

### Requirement: protect-files hook 迁入 bin 构建体系

protect-files hook 脚本 SHALL 从 `hooks/scripts/protect-files.mjs` 迁入 `bin/src/hooks.ts`，作为 `dev-team-hooks.cjs` 的子命令 `protect-files` 暴露。

hook 的 PreToolUse 入口 SHALL 保留，通过 `hooks/hooks.json` 中 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" protect-files` 注册。

hook SHALL 拦截的 tool 范围不变：`Write`, `Edit`, `Bash`, `PowerShell`。

hook SHALL 读取 `openspec/config.json` 的方式从手写 `loadConfig` 改为复用 `bin/src/lib/config.ts` 的 `readConfig` 函数。

#### Scenario: hook 注册命令指向 dev-team-hooks.cjs

**WHEN** 检查 `hooks/hooks.json` 中 PreToolUse hook 的 command 字段
**THEN** 所有 command 引用 `bin/dev-team-hooks.cjs protect-files` 而非 `hooks/scripts/protect-files.mjs`

#### Scenario: hooks.ts protect-files 子命令可被调用

**WHEN** 执行 `node bin/dev-team-hooks.cjs protect-files` 并传入有效 stdin JSON
**THEN** 进程 exit code 为 0
**AND** stdout 输出含 `hookSpecificOutput.permissionDecision` 的 JSON

#### Scenario: config 读取复用 readConfig

**WHEN** protect-files 子命令启动
**THEN** 它使用 `import { readConfig } from './lib/config'` 读取配置
**AND** `readConfig` 执行 Zod schema 验证

### Requirement: Built-in default protections (迁移不变)

内置保护规则不变 — 该 requirement 整体保持，仅实现方式从手写 globToRegex 改为 picomatch。

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

该 requirement 整体保持不变，行为和占位符规则与迁移前一致。

#### Scenario: 自定义 reason 含文件路径占位符

**WHEN** `openspec/config.json` 包含 `{ "write_protection": { "files": [{ "glob": "secrets/*", "reason": "File '%s' is protected." }] } }`
**AND** stdin 中 `tool_name: "Write"` 且 `file_path` 为 `secrets/api.key`
**THEN** denial reason 包含 `"File '...secrets/api.key' is protected."`

#### Scenario: 自定义 reason 含工具名称占位符

**WHEN** `openspec/config.json` 包含 `{ "write_protection": { "files": [{ "glob": "secrets/*", "reason": "Protected via %t" }] } }`
**AND** stdin 中 `tool_name: "Write"` 且 `file_path` 为 `secrets/api.key`
**THEN** denial reason 包含 `"Protected via Write"`

### Requirement: Fail-open behavior preserved (不变)

fail-open 行为规则不变 — 与迁移前完全一致。

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

Python/Node 命令豁免规则不变 — 与迁移前完全一致。

#### Scenario: Python 命令写入 eval.json 豁免

**WHEN** stdin 中 `tool_name: "Bash"` 且 command 为 `python scripts/deploy.py --eval openspec/changes/test/eval.json`
**THEN** 输出 `permissionDecision: "allow"`

#### Scenario: Node 命令写入 config.json 豁免

**WHEN** stdin 中 `tool_name: "Bash"` 且 command 为 `node scripts/setup.js openspec/config.json`
**THEN** 输出 `permissionDecision: "allow"`

## Module Contract

### Hook: PreToolUse — dev-team-hooks.cjs protect-files

| Property | Description |
|----------|-------------|
| **Entry** | `plugins/dev-team/bin/src/hooks.ts` (子命令: `protect-files`) |
| **Bundle** | `plugins/dev-team/bin/dev-team-hooks.cjs` |
| **Registration** | `plugins/dev-team/hooks/hooks.json` — PreToolUse for Write/Edit/Bash/PowerShell |
| **Input** | stdin JSON: `{ tool_name, tool_input: { file_path?, command? } }` |
| **Output** | stdout JSON: `{ hookSpecificOutput: { permissionDecision, permissionDecisionReason? } }` |
| **Config source** | `openspec/config.json` → `write_protection` field (via `readConfig` from `lib/config.ts`) |
| **Built-in defaults** | `**/openspec/changes/*/eval.json`, `**/openspec/config.json` |
| **Glob library** | `picomatch` via `lib/glob.ts` `matchGlob` |
| **Path normalization** | `lib/glob.ts` `toForwardSlash` (backslash → forward slash) |
| **Fallback** | Config parse failure → built-in defaults only |
| **Fail-open** | Input anomalies → allow |

### hooks.json (updated)

| Property | Before | After |
|----------|--------|-------|
| PreToolUse command (Write/Edit) | `node ".../hooks/scripts/protect-files.mjs"` | `node ".../bin/dev-team-hooks.cjs" protect-files` |
| PreToolUse command (Bash) | `node ".../hooks/scripts/protect-files.mjs"` | `node ".../bin/dev-team-hooks.cjs" protect-files` |
| PreToolUse command (PowerShell) | `node ".../hooks/scripts/protect-files.mjs"` | `node ".../bin/dev-team-hooks.cjs" protect-files` |

### vite.config.ts (updated)

| Property | Before | After |
|----------|--------|-------|
| pack entries | `[mcp, cli]` | `[mcp, cli, hooks]` |
| hooks entry | (none) | `src/hooks.ts` |
| hooks output | (none) | `dev-team-hooks.cjs`, format: `cjs`, minify: `true` |
