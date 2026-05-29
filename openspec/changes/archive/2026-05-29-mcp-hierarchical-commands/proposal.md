# 提案: mcp-hierarchical-commands

> **变更**: mcp-hierarchical-commands
> **日期**: 2026-05-29
> **状态**: draft

---

## 问题

当前 dev-team MCP server 中 6 个 tool 采用扁平命名（`eval_log`, `eval_check`, `archi_query`, `archi_validate`, `archi_write`, `archi_check`），如 `mcp__plugin_dev-team_dev-team__eval_log`。这种 `xx_yy` 格式将功能域和操作混在一个标识符中，缺乏层级结构，难以直观区分功能分组。

随着 MCP tool 数量增长（目前已 6 个，后续可能增加更多 eval/archi 子命令），扁平命名不利于可读性和组织性。

---

## 提案

将 6 个 MCP tool 从扁平 `xx_yy` 格式重命名为层级 `xx/yy` 格式：

| 当前名称 | 新名称 |
|----------|--------|
| `eval_log` | `eval/log` |
| `eval_check` | `eval/check` |
| `archi_query` | `archi/query` |
| `archi_validate` | `archi/validate` |
| `archi_write` | `archi/write` |
| `archi_check` | `archi/check` |

完全限定名相应变为：
- `mcp__plugin_dev-team_dev-team__eval/log`
- `mcp__plugin_dev-team_dev-team__eval/check`
- `mcp__plugin_dev-team_dev-team__archi/query`
- `mcp__plugin_dev-team_dev-team__archi/validate`
- `mcp__plugin_dev-team_dev-team__archi/write`
- `mcp__plugin_dev-team_dev-team__archi/check`

变更涵盖：
1. MCP server 源码 `bin/src/mcp.ts` — tool 定义和 `handleToolCall` switch
2. 编译产物 `bin/dev-team-mcp.cjs` — 重新构建
3. 10 个 skill 文件 — 更新 MCP tool 引用
4. 9 个 agent 文件 — 更新 MCP tool 引用
5. `.claude/settings.local.json` — 更新权限 allowlist

---

## 能力

### 新增能力

- `mcp-tool-namespace` — MCP tool 使用层级 `xx/yy` 命名规范，以 `/` 分隔功能域和操作

### 修改的能力

- `eval-check-cli` — eval_check/eval_log 重命名为 eval/check, eval/log
- `phase-skills` — 所有 skill 文件中的 MCP tool 引用更新为新命名
- `phase-agents` — 所有 agent 文件中的 MCP tool 引用更新为新命名

---

## 变更范围

### 实现以下特性

- 重命名 `bin/src/mcp.ts` 中 6 个 tool 定义名称
- 更新 `handleToolCall` switch 分支匹配新名称
- 重新构建 `bin/dev-team-mcp.cjs`
- 更新所有 skill 文件（10 个）中的 MCP tool 引用
- 更新所有 agent 文件（9 个）中的 MCP tool 引用
- 更新 `.claude/settings.local.json` 中的权限配置

### 不要修改

- MCP tool 的参数 schema — 仅改名，接口不变
- MCP tool 的业务逻辑（`commands/eval-log.ts`, `commands/eval-check.ts`, `lib/archi-*.ts`）— 实现不变
- CLI 命令 `dev-team eval-check`, `dev-team eval-log` — 这是独立的 CLI，不受影响
- `bin/openspec-bundled.js` — 打包的 CLI 文件使用 CLI 子命令（如 `dev-team eval-check`），不受 MCP tool 重命名影响
- eval.json 的数据格式和内容

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | 6 个 MCP tool 名称从 `xx_yy` 改为 `xx/yy` | 检查 `mcp.ts` 中 `TOOLS` 数组 | P0 |
| AC-2 | `handleToolCall` switch 分支匹配新名称 | 代码审查 | P0 |
| AC-3 | 编译产物 `dev-team-mcp.cjs` 反映新命名 | 构建对比 | P0 |
| AC-4 | 所有 skill 文件引用新 MCP tool 名称 | grep 检查无旧名称引用 | P0 |
| AC-5 | 所有 agent 文件引用新 MCP tool 名称 | grep 检查无旧名称引用 | P0 |
| AC-6 | settings.local.json 权限配置更新 | 手动检查 | P0 |
| AC-7 | 所有 eval_log 引用替换为 eval/log | grep 检查 | P0 |
| AC-8 | 所有 eval_check 引用替换为 eval/check | grep 检查 | P0 |
| AC-9 | 所有 archi_* 引用替换为 archi/* | grep 检查 | P0 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| MCP tool 名称含 `/` 导致 JSON-RPC 解析异常 | 工具调用失败 | 低 | MCP 协议规范允许任意字符串作为 tool name；先在本地验证 |
| skill/agent 文件遗漏未更新 | 运行时找不到 tool | 中 | 全局搜索替换并验证 |
| 编译产物不一致 | bin/dev-team-mcp.cjs 使用旧名称 | 中 | 重新构建并对比 |
