## MODIFIED Requirements

### Requirement: hooks.json 声明 subagentStop hook 匹配 implementation-generator (command 路径更新)

`plugins/dev-team/hooks/hooks.json` SHALL 在 `hooks` 对象中包含 `subagentStop` 数组，包含一项 hook 配置，保持以下配置不变：

- `matcher`: `"implementation-generator"` — 仅匹配 implementation-generator subagent 的结束事件
- `hooks[0].type`: `"command"`
- `loop_limit`: `5` — 最多允许 5 次 followup 循环

`hooks[0].command` 字段 SHALL 从 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"` 变更为 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" static-check`。

`test-gen-generator` 对应的 subagentStop hook SHALL 同步更新 command 路径。

当 followup 循环次数达到 `loop_limit` 时，hook 框架 SHALL 停止阻止 subagent 结束，允许 `implementation-generator` 在静态检查仍未通过的情况下结束（降级策略，行为不变）。

顶层 `description` 字段 SHALL 仍说明静态检查 hook 用途。

#### Scenario: subagentStop hook 配置中 command 指向 dev-team-hooks.cjs

**WHEN** `plugins/dev-team/hooks/hooks.json` 被 `JSON.parse()` 解析
**THEN** 对象包含 `hooks.subagentStop` 数组，至少包含一项
**AND** 该项的 `hooks[0].command` 包含 `bin/dev-team-hooks.cjs" static-check`

#### Scenario: 其他 subagent 结束时不触发静态检查 hook（不变）

**WHEN** `proposal-planner` 或其他非 `implementation-generator` 的 subagent 尝试结束
**THEN** `static-check` hook 不被触发
**AND** subagent 正常结束

#### Scenario: loop_limit 耗尽后允许 subagent 结束（不变）

**WHEN** `implementation-generator` 连续 5 次尝试结束且静态检查均失败
**THEN** 第 5 次 followup 循环后，hook 框架不再阻止 subagent 结束
**AND** `implementation-generator` 被允许结束，即使静态检查仍未通过

### Requirement: static-check 子命令进程内调用替代 spawnSync

`static-check` 子命令在 `bin/src/hooks.ts` 中实现，SHALL 直接导入并调用 `runStaticAnalysis` 函数（来自 `bin/src/commands/run-static-analysis.ts`），替代原有的 `spawnSync` 子进程调用。

行为规则：

- `runStaticAnalysis()` exit code 为 `0`：向 stdout 输出空 JSON `{}`，脚本 exit `0`，允许 subagent 结束
- `runStaticAnalysis()` exit code 非 `0`：向 stdout 输出包含 `followup_message` 的 JSON，脚本 exit `0`（通过 JSON 字段传递 followup）
- `followup_message` SHALL 包含 `runStaticAnalysis` 的完整 stderr/stdout 输出及中文修复指令前缀「静态检查未通过，请修复以下错误后重新提交：」，要求 agent 修复静态检查错误后重新提交
- `runStaticAnalysis` 函数不可用时（如 import 失败）：向 stdout 输出含 `followup_message` 的 JSON（说明错误原因），脚本 exit `0`，不抛未捕获异常

脚本 SHALL NOT 生成 `reports/static_analysis.json` 或任何其他报告文件（行为不变）。

子命令 SHALL 从 stdin 接收 subagentStop 事件 JSON，从中提取 `workspace_roots[0]` 作为 `projectRoot` 传递给 `runStaticAnalysis`。

#### Scenario: 静态检查通过时 hook 放行

**WHEN** `runStaticAnalysis()` 返回 exit code `0`
**THEN** `static-check` 子命令向 stdout 输出 `{}`
**AND** subagent 正常结束

#### Scenario: 静态检查失败时 hook 返回 followup_message

**WHEN** `runStaticAnalysis()` 返回 exit code 非 `0`，且输出包含 lint 错误信息
**THEN** `static-check` 子命令向 stdout 输出 JSON，包含 `followup_message` 字段
**AND** `followup_message` 包含 CLI 的错误输出内容
**AND** subagent 不被允许结束，继续修复

#### Scenario: 未配置 static_analysis 时 hook 放行

**WHEN** `openspec/config.json` 未配置 `static_analysis` 字段
**THEN** `runStaticAnalysis()` 返回 exit code `0`
**AND** `static-check` 子命令输出 `{}`，subagent 正常结束

#### Scenario: static-check 子命令可被 node 直接执行

**WHEN** 执行 `node bin/dev-team-hooks.cjs static-check`
**AND** stdin 传入有效事件 JSON
**THEN** 命令 exit code 为 `0`
**AND** stdout 输出合法 JSON（`{}` 或含 `followup_message` 的对象）

#### Scenario: 函数 import 失败时返回 followup_message

**WHEN** `runStaticAnalysis` 因任何原因无法导入或执行
**THEN** `static-check` 子命令向 stdout 输出含 `followup_message` 的 JSON
**AND** 脚本 exit `0`（不抛出未捕获异常）

### Requirement: static-check hook 不影响现有 PreToolUse hook（不变）

`subagentStop` hook SHALL NOT 改变 `PreToolUse` hook 的配置结构或拦截语义。

`protect-files` hook 对 eval.json 的 Write/Edit/Bash 拦截行为 SHALL 保持不变。

#### Scenario: protect-files hook 在 static-check 迁移后仍生效

**WHEN** agent 尝试 Write 写入 `openspec/changes/test/eval.json`
**THEN** PreToolUse hook 仍返回 `permissionDecision: "deny"`
**AND** static-check hook 未被触发

### Requirement: 插件版本号递增（不变）

`plugins/dev-team/.claude-plugin/plugin.json` 的 `version` 字段 SHALL 递增（patch bump），以反映 hook 脚本变更。

#### Scenario: 插件版本号已升级

**WHEN** 读取 `plugins/dev-team/.claude-plugin/plugin.json`
**THEN** `version` 字段值遵循 semver

## Module Contract

### Hook 声明：`plugins/dev-team/hooks/hooks.json`

| 字段 | 类型 | 描述 | 变更 |
|------|------|------|------|
| `hooks.subagentStop` | `object[]` | subagentStop hook 配置数组 | 不变 |
| `hooks.subagentStop[0].matcher` | `string` | `"implementation-generator"` | 不变 |
| `hooks.subagentStop[0].loop_limit` | `number` | `5` | 不变 |
| `hooks.subagentStop[0].hooks[0].type` | `string` | `"command"` | 不变 |
| `hooks.subagentStop[0].hooks[0].command` | `string` | `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" static-check` | 从 `.mjs` 变更为 `dev-team-hooks.cjs` |
| `hooks.subagentStop[1].matcher` | `string` | `"test-gen-generator"` | 不变 |
| `hooks.subagentStop[1].hooks[0].command` | `string` | `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-hooks.cjs" static-check` | 从 `.mjs` 变更为 `dev-team-hooks.cjs` |

### Hook 子命令：`plugins/dev-team/bin/dev-team-hooks.cjs static-check`

| 方面 | 描述 |
|------|------|
| **运行时** | Node.js CJS bundle，通过 `node` 调用 |
| **输入** | stdin JSON（subagentStop 事件，从中提取 `workspace_roots[0]`） |
| **执行方式** | 进程内 `import { runStaticAnalysis } from './commands/run-static-analysis'`（替代 spawnSync） |
| **projectRoot 来源** | stdin 事件 JSON 的 `workspace_roots[0]` |
| **通过** | stdout `{}`，exit `0` |
| **失败** | stdout `{"decision": "block", "reason": "<错误输出 + 修复指令>"}`，exit `0` |
| **异常安全** | 顶层 try-catch，任何异常转为 `{ decision: "block", reason }` |
| **报告** | 不生成任何 report 文件 |

### vite.config.ts（新增 hooks 打包项）

| 属性 | 值 |
|------|-----|
| pack entry name | `hooks` |
| platform | `node` |
| entry | `src/hooks.ts` |
| output file | `dev-team-hooks.cjs` |
| format | `cjs` |
| minify | `true` |
| sourcemap | `true` |
