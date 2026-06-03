# 提案: rename-mcp-tool-slash-to-underscore

> **变更**: rename-mcp-tool-slash-to-underscore
> **日期**: 2026-06-03
> **状态**: draft

---

## 问题

当前 dev-team MCP server 中 11 个 tool 使用 `xx/yy` 层级命名格式（如 `phase/log`、`archi/query`、`config/get`）。该格式在 MCP 工具的全限定名（FQN）中引入了不一致性：

1. **FQN 分隔符不一致** — MCP FQN 使用双下划线 `__` 作为插件命名空间与工具名称的分隔符，如 `mcp__plugin_dev-team_dev-team__phase/log`。工具名称内部再使用 `/` 作为层级分隔符，导致 `__` 和 `/` 两种分隔符混用，可读性差。

2. **部分 MCP 客户端兼容性问题** — 某些 MCP 客户端实现可能在路由、缓存键或 URI 解析中未正确处理 `/` 字符，导致工具调用异常或性能下降。

3. **全限定名路径化歧义** — 带 `/` 的 FQN 在某些上下文中可能被误解为文件系统路径或 URL 路径，增加调试和配置的认知负担。

4. **与社区命名惯例不一致** — MCP 生态中主流工具命名倾向于使用下划线 `_` 作为单词分隔符，而非斜杠 `/`。

此前 `mcp-hierarchical-commands` 变更（2026-05-29）将工具从扁平 `xx_yy` 改为层级 `xx/yy` 格式，旨在增强可读性。实践中发现 `/` 引入的问题大于其带来的组织价值。

---

## 提案

将所有 11 个 MCP tool 从 `xx/yy` 格式重命名为 `xx_yy`（下划线）格式：

| 当前名称 | 新名称 |
|----------|--------|
| `phase/log` | `phase_log` |
| `phase/check` | `phase_check` |
| `phase/next` | `phase_next` |
| `archi/query` | `archi_query` |
| `archi/validate` | `archi_validate` |
| `archi/write` | `archi_write` |
| `archi/check` | `archi_check` |
| `config/get` | `config_get` |
| `config/set` | `config_set` |
| `config/unset` | `config_unset` |
| `config/context` | `config_context` |

全限定名相应变为：
- `mcp__plugin_dev-team_dev-team__phase_log`
- `mcp__plugin_dev-team_dev-team__phase_check`
- `mcp__plugin_dev-team_dev-team__phase_next`
- `mcp__plugin_dev-team_dev-team__archi_query`
- `mcp__plugin_dev-team_dev-team__archi_validate`
- `mcp__plugin_dev-team_dev-team__archi_write`
- `mcp__plugin_dev-team_dev-team__archi_check`
- `mcp__plugin_dev-team_dev-team__config_get`
- `mcp__plugin_dev-team_dev-team__config_set`
- `mcp__plugin_dev-team_dev-team__config_unset`
- `mcp__plugin_dev-team_dev-team__config_context`

变更涵盖：
1. MCP server 源码 `bin/src/mcp.ts` — 11 个 tool 的 `registerTool` 名称
2. 编译产物 `bin/dev-team-mcp.cjs` — 重新构建
3. 11 个 skill 文件 — 更新 MCP tool 引用
4. 10 个 agent 文件 — 更新 MCP tool 引用
5. `.claude/settings.local.json` — 更新权限 allowlist
6. 相关 spec 文件 — 更新命名约定和引用

---

## 能力

### 修改的能力

- `mcp-tool-namespace` — MCP tool 命名约定从 `xx/yy` 更改为 `xx_yy`（下划线格式）
- `eval-check-cli` — phase/check、phase/log 引用更新为 phase_check、phase_log
- `phase-skills` — 所有 skill 文件中的 MCP tool 引用更新为下划线格式
- `phase-agents` — 所有 agent 文件中的 MCP tool 引用更新为下划线格式
- `config-get` — config/get 重命名为 config_get
- `config-set` — config/set 重命名为 config_set
- `config-unset` — config/unset 重命名为 config_unset
- `config-context` — config/context 重命名为 config_context

---

## 变更范围

### 实现以下特性

- 重命名 `bin/src/mcp.ts` 中 11 个 tool 定义名称（`registerTool` 调用的第一个参数）
- 重新构建 `bin/dev-team-mcp.cjs`
- 更新 11 个 skill 文件中的 MCP tool 引用（全部在 `plugins/dev-team/skills/` 下）
- 更新 10 个 agent 文件中的 MCP tool 引用（全部在 `plugins/dev-team/agents/` 下）
- 更新 `.claude/settings.local.json` 中的权限配置（旧名称 `eval/check`、`eval/log` → `phase_check`、`phase_log`）
- 更新 `openspec/specs/mcp-tool-namespace/spec.md` 中的命名规范
- 更新 `openspec/specs/eval-check-cli/spec.md` 中的工具引用
- 更新 `openspec/specs/phase-skills/spec.md` 中的工具引用
- 更新 `openspec/specs/phase-agents/spec.md` 中的工具引用
- 更新 `openspec/specs/config-{get,set,unset,context}/spec.md` 中的工具名称

### 不要修改

- MCP tool 的参数 schema — 仅改名，接口不变
- MCP tool 的业务逻辑实现（`commands/*.ts`, `lib/*.ts`）— 实现完全不变
- CLI 命令（`dev-team eval-check`, `dev-team eval-log`）— 独立的 CLI，不受 MCP tool 重命名影响
- `bin/openspec-bundled.js` — 打包的 CLI 文件，不受 MCP tool 重命名影响
- eval.json 的数据格式和内容
- `mcp-tool-namespace` 之外的其他 spec 文件的非工具名称引用的内容（如 schema、行为描述）

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | 11 个 MCP tool 名称从 `xx/yy` 改为 `xx_yy` | 检查 `mcp.ts` 中所有 `registerTool` 调用的第一个参数 | P0 |
| AC-2 | `mcp-tool-namespace` spec 更新为下划线格式 | 代码审查 | P0 |
| AC-3 | 编译产物 `dev-team-mcp.cjs` 反映新命名 | 构建后 grep 检查无 `xx/yy` 工具名称 | P0 |
| AC-4 | 所有 11 个 skill 文件引用下划线格式 MCP tool 名称 | grep 检查无 `mcp__plugin_dev-team_dev-team__*/*` 引用 | P0 |
| AC-5 | 所有 10 个 agent 文件引用下划线格式 MCP tool 名称 | grep 检查无 `mcp__plugin_dev-team_dev-team__*/*` 引用 | P0 |
| AC-6 | settings.local.json 权限配置使用新名称 | 手动检查 | P0 |
| AC-7 | 所有 `phase/` 工具引用替换为 `phase_` | grep 检查 `mcp.ts` 和文档 | P0 |
| AC-8 | 所有 `archi/` 工具引用替换为 `archi_` | grep 检查 `mcp.ts` 和文档 | P0 |
| AC-9 | 所有 `config/` 工具引用替换为 `config_` | grep 检查 `mcp.ts` 和文档 | P0 |
| AC-10 | 所有 8 个相关 spec 文件中的工具名称已更新 | 文件列表检查 | P1 |
| AC-11 | settings.local.json 中 `eval/` 前缀的旧条目已清理 | grep 检查无 `eval/` 条目 | P1 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| skill/agent 文件遗漏未更新 | 运行时找不到 tool | 中 | 全局搜索替换并逐文件验证 |
| settings.local.json 中旧条目未清理 | 权限冲突或冗余 | 中 | 手动检查并清理 |
| 编译产物不一致 | bin/dev-team-mcp.cjs 使用旧名称 | 低 | 重新构建并在 CI 中验证 |
| 工作流在执行中被中断，中间状态引用旧名称 | 已运行的技能流程调用失败 | 低 | 重命名操作应在工作流空闲时执行；用户重新触发工作流即可 |
