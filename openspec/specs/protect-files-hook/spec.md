# protect-files-hook Specification

## Purpose
PreToolUse write-protection hook packaged as a hooks CJS subcommand, with Claude and Cursor tool-name parity (including Shell/StrReplace for home image). PreToolUse 写保护。评估历史并入 `workflow.json` 后，必须保护该文件；遗留 `eval.json` 不再属于内置保护集合。

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

### Requirement: PreToolUse 与 PostToolUse 共用 extractFileOps 提取器

`plugins/dev-team/bin/src/hooks.ts` SHALL 将 bash 侧 `extractBashWriteTargets` 与 PowerShell 侧 `extractPowerShellWriteTargets` 泛化为单一三态提取器 `extractFileOps(command) → Array<{ op: 'write' | 'delete' | 'revert', path }>`（分类规则见 `workflow-file-inventory` 规格），并让 PreToolUse 写保护改为消费该提取器：

- PreToolUse：提取路径 ∩ 保护 glob → deny（路径级精确匹配）
- PostToolUse：提取路径 → 过滤 → 归账文件清单

既有 python/node 命令豁免、fail-open 行为、内置与用户 glob 合并逻辑 SHALL 保持不变；`Write`/`Edit`/`Shell`/`StrReplace` 的既有拦截语义 SHALL 兼容。写保护由此从包含式命令判断升级为按提取路径匹配，对 `workflow.json` 的重定向写入仍 SHALL deny。

#### Scenario: 重定向写受保护路径仍被拒绝

- **WHEN** `tool_name` 为 `Bash` 且 command 为 `echo x > openspec/changes/x/workflow.json`
- **THEN** `permissionDecision: "deny"`

#### Scenario: 提取路径不匹配则放行

- **WHEN** `tool_name` 为 `Bash` 且 command 为 `echo x > src/notes.txt`
- **THEN** `permissionDecision: "allow"`

#### Scenario: 豁免语义不变

- **WHEN** command 为 `node scripts/setup.js openspec/config.json`
- **THEN** `permissionDecision: "allow"`（python/node 豁免保持）

### Requirement: 写保护扩展为写/删保护

PreToolUse 保护 SHALL 依据 `extractFileOps` 的 `delete` 分类同步拦截对受保护路径的删除：`rm <受保护路径>`、`Remove-Item`、`git rm` 等命中保护 glob 时 SHALL 输出 `permissionDecision: "deny"`，denial reason 沿用既有 `%s` / `%t` 占位符规则。

#### Scenario: rm 受保护文件被拒绝

- **WHEN** `tool_name` 为 `Bash` 且 command 为 `rm openspec/changes/x/workflow.json`
- **THEN** `permissionDecision: "deny"`
- **AND** denial reason 含该文件路径

#### Scenario: 删除非保护文件放行

- **WHEN** command 为 `rm src/tmp.txt`
- **THEN** `permissionDecision: "allow"`

### Requirement: 批量还原命令拦截

PreToolUse SHALL 拦截对 hook 不可见且对用户数据最危险的批量还原命令：`git stash`（含 `stash pop`/`stash apply` 对工作区的改写形态按 design 细化）与 `git clean`。拦截范围 SHALL 至少覆盖 `openspec/changes/**` 工作流产物（防止批量还原抹掉 proposal/design/reports 与文件清单）；`git restore` / `git checkout --` 对 `openspec/changes/**` 工作流产物的单路径还原 SHALL 同样被拦（作为清单机制认定的规范还原动作的保护区），对项目源码的 `git restore` SHALL 放行（它是可被记录的规范还原动作）。

#### Scenario: git clean 拦截

- **WHEN** `tool_name` 为 `Bash` 且 command 为 `git clean -fd openspec/changes/my-change`
- **THEN** `permissionDecision: "deny"`

#### Scenario: restore 工作流产物拦截

- **WHEN** command 为 `git restore openspec/changes/my-change/design.md`
- **THEN** `permissionDecision: "deny"`

#### Scenario: restore 源码放行

- **WHEN** command 为 `git restore src/a.ts`
- **THEN** `permissionDecision: "allow"`
- **AND** 该 revert 操作由 PostToolUse 记录器按折叠规则记录

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

### Module: `plugins/dev-team/bin/src/hooks.ts`

| 符号 | 变更 |
|------|------|
| `loadPatterns` 内置 glob | **ADDED** `**/openspec/changes/**/workflow.json`（reason 提示 `phase_log` / `backtrack` / `change_create`）；**KEEP** `**/openspec/config.json`；**REMOVED** `**/openspec/changes/**/eval.json` |
| user-defined glob 合并 | 不变（`write_protection.files` 仍按原逻辑追加，可自行保护 `eval.json`） |
