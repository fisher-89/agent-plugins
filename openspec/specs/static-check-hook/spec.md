# static-check-hook Specification

## Purpose
Claude SubagentStop static-check gating for implementation/test-gen generators, with Cursor products omitting `subagentStop` and relying on include-based soft gates.

## Requirements

### Requirement: hooks.json 声明 subagentStop hook 匹配 implementation-generator (command 路径更新)

Canonical 源 `plugins/dev-team/hooks/hooks.canonical.json` SHALL 继续声明两条 `subagentStop` 条目，分别匹配 `implementation-generator` 与 `test-gen-generator`，`commandTemplate` 指向 `static-check` 子命令，并保留 `loop_limit: 5`。

对 **Claude** 产物（`buildClaudeNested` → `claude-plugins/dev-team/hooks/hooks.json`）：

- `hooks.SubagentStop` SHALL 包含上述两项
- matcher SHALL 使用 Claude 侧 agent 引用形式（经 token 展开后的 `__CALL_AGENT:…__` 结果）
- 每项内层 `hooks[0].type` SHALL 为 `"command"`
- `hooks[0].command` SHALL 调用 hooks 二进制的 `static-check` 子命令（经 `__BIN:hooks__` / 路径 token 展开）

当 followup 循环次数达到 `loop_limit` 时，Claude hook 框架 SHALL 停止阻止 subagent 结束，允许 generator 在静态检查仍未通过的情况下结束（降级策略，行为不变）。

对 **Cursor** 产物（`cursor` 与 `cursorHome`）：本 requirement 的 `subagentStop` / `SubagentStop` 发射要求 **不适用**——见 ADDED「Cursor 产物省略 subagentStop」。canonical 中对应条目的 `matchers.cursor` SHALL 为 `null`。

顶层 `description` 字段 SHALL 仍说明静态检查 hook 用途（可注明主要作用于 Claude SubagentStop）。

#### Scenario: Claude 产物 subagentStop hook 配置中 command 指向 hooks 二进制 static-check

**WHEN** `claude-plugins/dev-team/hooks/hooks.json` 被 `JSON.parse()` 解析
**THEN** 对象包含 `hooks.SubagentStop` 数组，至少包含两项（implementation-generator 与 test-gen-generator）
**AND** 各项的内层 `hooks[0].command` 包含 hooks 二进制名与 `static-check`

#### Scenario: 其他 subagent 结束时不触发静态检查 hook（不变）

**WHEN** `proposal-planner` 或其他非目标 matcher 的 subagent 在 Claude 侧尝试结束
**THEN** `static-check` hook 不被触发
**AND** subagent 正常结束

#### Scenario: loop_limit 耗尽后允许 subagent 结束（不变）

**WHEN** Claude 侧 `implementation-generator` 连续 5 次尝试结束且静态检查均失败
**THEN** 第 5 次 followup 循环后，hook 框架不再阻止 subagent 结束
**AND** `implementation-generator` 被允许结束，即使静态检查仍未通过

#### Scenario: Cursor canonical matcher 为 null

**WHEN** 读取 `plugins/dev-team/hooks/hooks.canonical.json` 的 `subagentStop` 条目
**THEN** 每条的 `matchers.cursor` SHALL 为 `null`
**AND** `matchers.claude` SHALL 仍为非 null 的 generator 引用

### Requirement: Cursor 产物省略 subagentStop 整键

`buildCursorNative`（及由此产生的 `cursor-plugins/dev-team` 与 `cursor-home-image/dev-team` hooks 文件）SHALL NOT 在 `hooks` 对象下写入 `subagentStop` 键。

过滤规则 SHALL 与 `preToolUse` 对齐：`matchers.cursor == null` 的条目不发射；若过滤后 `subagentStop` 列表为空，MUST 省略该键（MUST NOT 写 `subagentStop: []`）。

Cursor 侧静态检查门禁改由 agent 正文 `__INCLUDE:static-analysis-gate__` 软约束承担（见能力 `include-fragments` / `phase-agents`），本能力 MUST NOT 要求 Cursor 上 hook 硬阻断与 Claude 同强度。

#### Scenario: marketplace cursor hooks 无 subagentStop 键

**WHEN** 解析 `cursor-plugins/dev-team/hooks/hooks.json`
**THEN** `hooks` 对象 MUST NOT 拥有 `subagentStop` 属性

#### Scenario: cursorHome hooks 无 subagentStop 键

**WHEN** 解析 `cursor-home-image/dev-team/hooks.json`
**THEN** `hooks` 对象 MUST NOT 拥有 `subagentStop` 属性

#### Scenario: 空列表不写成空数组

**WHEN** canonical 中全部 `subagentStop` 的 `matchers.cursor` 为 `null`
**AND** `buildCursorNative` 生成 hooks 文档
**THEN** 输出 JSON 的 `hooks` MUST NOT 包含键 `subagentStop`

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

### Hook 声明：canonical → 产物

| 字段 / 产物 | 类型 | 描述 | 变更 |
|------|------|------|------|
| `hooks.canonical.json` `subagentStop[].matchers.claude` | `string` | Claude matcher（CALL_AGENT token） | 保留 |
| `hooks.canonical.json` `subagentStop[].matchers.cursor` | `null` | Cursor 不发射 | 改为 null |
| `hooks.canonical.json` `subagentStop[].loop_limit` | `number` | `5`（Claude 侧有效） | 保留 |
| `hooks.canonical.json` `subagentStop[].commandTemplate` | `string` | hooks 二进制 + `static-check` | 保留 |
| Claude 产物 `hooks.SubagentStop` | `object[]` | nested command hooks | 保留发射 |
| Cursor / cursorHome `hooks.subagentStop` | — | 不存在该键 | 省略整键 |

### Hook 子命令：`static-check`（不变摘要）

| 方面 | 描述 |
|------|------|
| **运行时** | Node.js CJS hooks 包 |
| **输入** | stdin JSON（SubagentStop 事件） |
| **执行** | 进程内 `runStaticAnalysis` |
| **适用范围** | Claude 产物 hook 路径；Cursor 不再经此事件触发 |
| **projectRoot 来源** | stdin 事件 JSON 的 `workspace_roots[0]` |
| **通过** | stdout `{}`，exit `0` |
| **失败** | stdout 含 `followup_message` 的 JSON，exit `0` |
| **异常安全** | 顶层 try-catch，任何异常转为 followup JSON |
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
