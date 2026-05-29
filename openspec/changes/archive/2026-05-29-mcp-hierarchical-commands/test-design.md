# 测试设计: mcp-hierarchical-commands

> **变更**: mcp-hierarchical-commands
> **日期**: 2026-05-29
> **基于**: proposal.md, design.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 静态验证脚本 | `plugins/dev-team/bin/src/mcp.ts` 中 6 个 tool 的 name 定义和 handleToolCall 路由 | Node.js 脚本 + grep/PowerShell | 100% 覆盖 TOOLS 数组和 switch case 的新名称 |
| 构建验证脚本 | `plugins/dev-team/bin/dev-team-mcp.cjs` 编译产物 | PowerShell diff/grep | 验证产物中 TOOLS 数组和 switch 语句使用 xx/yy 格式 |
| 引用完整性检查 | `plugins/dev-team/skills/*/SKILL.md` (10 个) + `plugins/dev-team/agents/*.md` (10 个) + `.claude/settings.local.json` | grep/Select-String | 零旧名称残留，所有引用已更新 |
| 功能等价性验证 | MCP server 工具路由（新名称 -> 原业务逻辑） | 验证脚本调用 MCP server 启动 | 确认新名称正确路由到原实现函数 |

---

## 2. 覆盖映射

所有测试脚本统一存放于 `openspec/changes/mcp-hierarchical-commands/tests/` 目录。

| 需求ID | 验收条件 | 测试文件 | 层级 | 覆盖类型 |
|--------|---------|----------|------|----------|
| AC-1 | 6 个 MCP tool 名称从 `xx_yy` 改为 `xx/yy` | `tests/verify_mcp_tool_names.js` | 静态验证 | 结构验证：解析 TOOLS 数组，检查所有 name 字段格式 |
| AC-2 | `handleToolCall` switch 分支匹配新名称 | `tests/verify_mcp_tool_names.js` | 静态验证 | 结构验证：提取 switch case 标签，与 TOOLS 数组交叉检查 |
| AC-3 | 编译产物 `dev-team-mcp.cjs` 反映新命名 | `tests/verify_build_output.ps1` | 构建验证 | 产物验证：检查编译产物的 TOOLS 字符串引用 |
| AC-4 | 所有 skill 文件引用新 MCP tool 名称 | `tests/check_skill_references.ps1` | 引用完整性 | 逐文件 grep 检查，确保无旧 FQN 残留 |
| AC-5 | 所有 agent 文件引用新 MCP tool 名称 | `tests/check_agent_references.ps1` | 引用完整性 | 逐文件 grep 检查，确保无旧 FQN 残留 |
| AC-6 | settings.local.json 权限配置更新 | `tests/check_settings_references.ps1` | 引用完整性 | 检查 allowlist 中的新旧名称 |
| AC-7 | 所有 `eval_log` 引用替换为 `eval/log` | `tests/check_global_old_names.ps1` | 引用完整性 | 全局 grep 旧名称模式 `eval_log` |
| AC-8 | 所有 `eval_check` 引用替换为 `eval/check` | `tests/check_global_old_names.ps1` | 引用完整性 | 全局 grep 旧名称模式 `eval_check` |
| AC-9 | 所有 `archi_*` 引用替换为 `archi/*` | `tests/check_global_old_names.ps1` | 引用完整性 | 全局 grep 旧名称模式 `archi_` |

---

## 3. 测试策略

### 3.1 方法

本变更为纯重命名重构，不涉及业务逻辑变更。测试策略以**静态分析验证**和**构建产物验证**为主，辅以**功能等价性验证**确保新名称正确路由到原实现。所有测试均为自动化脚本，可在 PowerShell 或 Node.js 环境中执行。

由于项目中无现有测试框架（无 package.json/tsconfig），测试脚本采用独立的 Node.js 脚本和 PowerShell 脚本，不依赖外部测试运行器。

### 3.2 测试分类

- **静态验证脚本**: 直接解析 `mcp.ts` 源码（通过正则提取或 AST），验证 tool 定义的 name 字段和 switch case 标签均使用 `xx/yy` 格式。不依赖外部服务或编译。

- **构建验证脚本**: 在重新构建 `dev-team-mcp.cjs` 后执行，检查编译产物中的 TOOLS 字符串和 switch 语句是否与源码一致。验证产物中无旧名称残留。

- **引用完整性检查**: 对 10 个 skill 文件、10 个 agent 文件、1 个 settings.local.json 文件逐一执行文本搜索，确认所有引用点均已更新为新的完全限定名（`mcp__plugin_dev-team_dev-team__eval/log`、`mcp__plugin_dev-team_dev-team__eval/check`、`mcp__plugin_dev-team_dev-team__archi/*`）。

- **功能等价性验证**: 通过启动 MCP server 并发送 `tools/list` 和 `tools/call` 请求，验证新名称的 tool 能被正确路由到对应的业务逻辑函数。

### 3.3 模拟策略

- **静态验证和引用完整性检查**: 不需要模拟。直接对文件执行文本分析和模式匹配。
- **构建验证**: 不需要模拟。直接检查产物文件内容。
- **功能等价性验证**: 需要模拟 MCP client 对 MCP server 发起 JSON-RPC 请求。使用 Node.js 的 child_process 启动 server 进程，通过 stdin/stdout JSON-RPC 通信，模拟实际调用场景。不启动真实 Claude Code 环境。

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| handleToolCall 收到未知 tool 名称 | 新名称以外的随机字符串（如 `"eval/export"`） | 抛出 `"Unknown tool: eval/export"` 错误 | `tests/verify_mcp_tool_names.js` |
| handleToolCall 收到旧名称 `"eval_log"` | 旧名称作为遗留调用传入 | 进入 default 分支，抛出 `"Unknown tool: eval_log"` 错误 | `tests/verify_mcp_tool_names.js` |
| handleToolCall 收到所有 6 个新名称 | `"eval/log"`, `"eval/check"`, `"archi/query"`, `"archi/validate"`, `"archi/write"`, `"archi/check"` | 各自路由到对应的业务逻辑函数（runEvalLog, runEvalCheck, queryModel, validateDsl, writeDsl, runCrossRefCheck） | `tests/verify_mcp_tool_names.js` |
| TOOLS 数组的 name 与 switch case 不一致 | TOOLS 中有 `"eval/log"` 但 switch 中缺少对应 case | 脚本检测到不匹配项并报错 | `tests/verify_mcp_tool_names.js` |
| 编译产物中旧名称仍出现 | `dev-team-mcp.cjs` 中包含 `"eval_log"`, `"eval_check"`, `"archi_"` 等子串 | 脚本检测到旧名称残留并报错 | `tests/verify_build_output.ps1` |
| settings.local.json 中只更新了部分名称 | 仅更新了 `eval_check` 但遗漏 `eval_log` | 脚本检测到旧名称 `mcp__plugin_dev-team_dev-team__eval_log` 仍存在 | `tests/check_settings_references.ps1` |
| skill 文件中使用短名称而非 FQN | 文件中出现 `eval/log` 但缺少 `mcp__` 前缀 | 此非本变更范围——变更只涉及 `mcp__plugin_dev-team_dev-team__` 完全限定名的重命名；短名称不会出现在 skill/agent 引用中 | `tests/check_skill_references.ps1` |
| 多个旧名称在同一文件中混合出现 | 同一文件同时包含 `eval_log` 和 `eval/log` | 脚本检测到 `eval_log` 旧名称存在，标记为残留 | `tests/check_global_old_names.ps1` |

---

## 5. 测试数据

本变更不引入新的测试数据。测试脚本直接操作源文件文本内容，不需要外部数据文件。

验证使用以下固定的名称映射表：

| 旧名称 | 新名称 |
|--------|--------|
| `eval_log` | `eval/log` |
| `eval_check` | `eval/check` |
| `archi_query` | `archi/query` |
| `archi_validate` | `archi/validate` |
| `archi_write` | `archi/write` |
| `archi_check` | `archi/check` |
| `mcp__plugin_dev-team_dev-team__eval_log` | `mcp__plugin_dev-team_dev-team__eval/log` |
| `mcp__plugin_dev-team_dev-team__eval_check` | `mcp__plugin_dev-team_dev-team__eval/check` |
| `mcp__plugin_dev-team_dev-team__archi_query` | `mcp__plugin_dev-team_dev-team__archi/query` |
| `mcp__plugin_dev-team_dev-team__archi_validate` | `mcp__plugin_dev-team_dev-team__archi/validate` |
| `mcp__plugin_dev-team_dev-team__archi_write` | `mcp__plugin_dev-team_dev-team__archi/write` |
| `mcp__plugin_dev-team_dev-team__archi_check` | `mcp__plugin_dev-team_dev-team__archi/check` |

---

## 6. 不可测试项

- **MCP tool 的参数 schema 正确性** — 原因: 本变更仅重命名 tool 名称，参数 schema 和业务逻辑实现保持不变。schema 正确性由各自的业务逻辑测试覆盖，不在本变更范围内。
- **`runEvalLog` / `runEvalCheck` / `queryModel` / `validateDsl` / `writeDsl` / `runCrossRefCheck` 的业务逻辑正确性** — 原因: 这些函数的实现未被修改。如果它们已有独立测试，保持不变；如果没有，不属于本变更的添加范围。
- **CLI 命令 `dev-team eval-check` / `dev-team eval-log`** — 原因: 根据 proposal.md 的明确说明，CLI 命令不受 MCP tool 重命名影响，不在变更范围内。
- **`bin/openspec-bundled.js`** — 原因: 打包的 CLI 文件使用 CLI 子命令，不受 MCP tool 命名影响。
- **`eval.json` 的数据格式和内容** — 原因: 数据格式不受 tool 名称变更影响。
- **`.cursor/commands/opsx-archive.md`** — 原因: 引用 JSON 结果字段名，非 MCP tool 名称。
