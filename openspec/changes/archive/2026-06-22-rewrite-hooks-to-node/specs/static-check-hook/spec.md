## MODIFIED Requirements

### Requirement: hooks.json 声明 subagentStop hook 匹配 implementation-generator

`plugins/dev-team/hooks/hooks.json` SHALL 在 `hooks` 对象中包含 `subagentStop` 数组，包含一项 hook 配置：

- `matcher`: `"implementation-generator"` — 仅匹配 implementation-generator subagent 的结束事件
- `hooks[0].type`: `"command"`
- `hooks[0].command`: `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"`
- `loop_limit`: `5` — 最多允许 5 次 followup 循环

当 followup 循环次数达到 `loop_limit` 时，hook 框架 SHALL 停止阻止 subagent 结束，允许 `implementation-generator` 在静态检查仍未通过的情况下结束。此为有意的降级策略，避免无限循环。

顶层 `description` 字段 SHALL 说明静态检查 hook 用途。

`hooks.json` 中 PreToolUse hook 的 command SHALL 同样使用 `node` 调用 `.mjs` 脚本，以确保 Windows（PowerShell）环境下 hook 可正常执行。

#### Scenario: subagentStop hook 配置为合法 JSON

- **WHEN** `plugins/dev-team/hooks/hooks.json` 被 `JSON.parse()` 解析
- **THEN** 解析成功无错误
- **AND** 对象包含 `hooks.subagentStop` 数组，至少包含一项
- **AND** 该项的 `matcher` 为 `"implementation-generator"`
- **AND** 该项包含 `loop_limit: 5`
- **AND** 该项的 `hooks[0].command` 以 `node` 开头并引用 `static-check.mjs`

#### Scenario: 其他 subagent 结束时不触发静态检查 hook

- **WHEN** `proposal-planner` 或其他非 `implementation-generator` 的 subagent 尝试结束
- **THEN** `static-check.mjs` hook 不被触发
- **AND** subagent 正常结束

#### Scenario: loop_limit 耗尽后允许 subagent 结束

- **WHEN** `implementation-generator` 连续 5 次尝试结束且静态检查均失败（hook 每次返回 `decision: "block"`）
- **THEN** 第 5 次 followup 循环后，hook 框架不再阻止 subagent 结束
- **AND** `implementation-generator` 被允许结束，即使静态检查仍未通过

#### Scenario: Windows PowerShell 环境下 hook command 可执行

- **WHEN** 在 Windows 上执行 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"`（无需 bash）
- **THEN** 命令 exit code 为 `0`
- **AND** stdout 输出合法 JSON（`{}` 或含 `decision` / `reason` 的对象）

### Requirement: static-check.mjs 通过 CLI 执行静态检查并根据 exit code 决定放行或 followup

`plugins/dev-team/hooks/scripts/static-check.mjs` SHALL 为有效的 Node.js ES Module 脚本，从 stdin 接收 subagentStop 事件 JSON（脚本 MAY 忽略 stdin 内容）。

脚本 SHALL 通过 `child_process` 执行：

```javascript
node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs" run_static_analysis
```

行为规则：

- CLI exit code 为 `0`：向 stdout 输出空 JSON `{}`，脚本 exit `0`，允许 subagent 结束
- CLI exit code 非 `0`：向 stdout 输出包含 `decision: "block"` + `reason` 的 JSON，脚本 exit `0`（hook 协议要求通过 JSON 字段传递 followup，而非脚本 exit code）
- `reason` SHALL 包含 CLI 的完整 stderr/stdout 输出及中文修复指令前缀「静态检查未通过，请修复以下错误后重新提交：」，要求 agent 修复静态检查错误后重新提交
- CLI 文件不存在时：向 stdout 输出含 `decision: "block"` + `reason` 的 JSON（说明 CLI 路径），脚本 exit `0`

脚本 SHALL NOT 生成 `reports/static_analysis.json` 或任何其他报告文件。

脚本 SHALL 使用 `JSON.stringify` 生成输出，确保特殊字符正确转义。

#### Scenario: 静态检查通过时 hook 放行

- **WHEN** `dev-team-cli.cjs run_static_analysis` 以 exit code `0` 结束
- **THEN** `static-check.mjs` 向 stdout 输出 `{}`
- **AND** subagent 正常结束

#### Scenario: 静态检查失败时 hook 返回 decision: "block"

- **WHEN** `dev-team-cli.cjs run_static_analysis` 以 exit code 非 `0` 结束，且 stderr 包含 lint 错误信息
- **THEN** `static-check.mjs` 向 stdout 输出 JSON，包含 `decision: "block"` 和 `reason` 字段
- **AND** `reason` 包含 CLI 的错误输出内容
- **AND** subagent 不被允许结束，继续修复

#### Scenario: 未配置 static_analysis 时 hook 放行

- **WHEN** `openspec/config.json` 未配置 `static_analysis` 字段
- **THEN** CLI 以 exit code `0` 结束
- **AND** hook 输出 `{}`，subagent 正常结束

#### Scenario: static-check.mjs 可被 node 直接执行

- **WHEN** 执行 `node plugins/dev-team/hooks/scripts/static-check.mjs`
- **THEN** 命令 exit code 为 `0` 且无未捕获异常
- **AND** stdout 输出合法 JSON

#### Scenario: CLI 不存在时返回 decision: "block"

- **WHEN** `${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs` 文件不存在
- **THEN** `static-check.mjs` 向 stdout 输出含 `decision: "block"` + `reason` 的 JSON
- **AND** 脚本 exit `0`（不抛出未捕获异常）

### Requirement: static-check hook 不影响现有 PreToolUse hook

`subagentStop` hook 的迁移 SHALL NOT 改变 `PreToolUse` hook 的配置结构或拦截语义。

`protect-eval.mjs` 对 eval.json 的 Write/Edit/Bash 拦截行为 SHALL 保持不变。

#### Scenario: protect-eval hook 在 static-check hook 迁移后仍生效

- **WHEN** agent 尝试 Write 写入 `openspec/changes/test/eval.json`
- **THEN** PreToolUse hook 仍返回 `permissionDecision: "deny"`
- **AND** static-check hook 未被触发

### Requirement: 插件版本号递增

`plugins/dev-team/.claude-plugin/plugin.json` 的 `version` 字段 SHALL 递增（patch bump），以反映 hook 脚本从 Bash 迁移至 Node.js。实现完成后版本号 SHALL 高于 `"2.6.22"`。

#### Scenario: 插件版本号已升级

- **WHEN** 读取 `plugins/dev-team/.claude-plugin/plugin.json`
- **THEN** `version` 字段值 SHALL 大于 `"2.6.22"`
- **AND** 版本格式遵循 semver

## REMOVED Requirements

### Requirement: static-check.sh 通过 CLI 执行静态检查并根据 exit code 决定放行或 followup

**Reason:** Bash 脚本在 Windows PowerShell 环境下无法通过 `bash` 命令调用；已由 `static-check.mjs` 替代。

**Migration:** `hooks.json` command 改为 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"`；删除 `static-check.sh`。

## Module Contract

### Hook 声明：`plugins/dev-team/hooks/hooks.json`

| 字段 | 类型 | 描述 |
|------|------|------|
| `hooks.subagentStop` | `object[]` | subagentStop hook 配置数组 |
| `hooks.subagentStop[0].matcher` | `string` | `"implementation-generator"` |
| `hooks.subagentStop[0].loop_limit` | `number` | `5` |
| `hooks.subagentStop[0].hooks[0].type` | `string` | `"command"` |
| `hooks.subagentStop[0].hooks[0].command` | `string` | `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"` |

### Hook 脚本：`plugins/dev-team/hooks/scripts/static-check.mjs`

| 方面 | 描述 |
|------|------|
| **运行时** | Node.js ES Module（`.mjs`），通过 `node` 调用 |
| **输入** | stdin JSON（subagentStop 事件，可选读取） |
| **执行** | `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs" run_static_analysis` |
| **通过** | stdout `{}`，exit `0` |
| **失败** | stdout `{"decision": "block", "reason": "<错误输出 + 修复指令>"}`，exit `0` |
| **报告** | 不生成任何 report 文件 |

### 插件配置：`plugins/dev-team/.claude-plugin/plugin.json`

| 字段 | 描述 |
|------|------|
| `version` | patch bump，实现完成后 SHALL 大于 `"2.6.22"` |
