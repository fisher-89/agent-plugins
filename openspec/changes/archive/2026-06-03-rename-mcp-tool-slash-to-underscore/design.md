# 设计: rename-mcp-tool-slash-to-underscore

> **变更**: rename-mcp-tool-slash-to-underscore
> **日期**: 2026-06-03
> **基于**: proposal.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| MCP Server 源码 | 注册 11 个 MCP tool，定义 tool 名称、schema 和处理函数 | `plugins/dev-team/bin/src/mcp.ts` | `@modelcontextprotocol/sdk`, schema 文件, command 文件 | TypeScript (ES2020) |
| MCP Server 编译产物 | 构建后产物，包含下划线命名的 tool 定义和路由 | `plugins/dev-team/bin/dev-team-mcp.cjs` | `mcp.ts` 编译结果 | JavaScript (CommonJS), 经 vite-plus 打包 |
| Skill 文件 (11 个) | PGE 工作流阶段技能，通过 FQN 调用 MCP tool | `plugins/dev-team/skills/*/SKILL.md` | MCP Server 运行时 | Markdown (skill spec) |
| Agent 文件 (10 个) | 专业领域 agent，通过 FQN 调用 MCP tool | `plugins/dev-team/agents/*.md` | MCP Server 运行时 | Markdown (agent spec) |
| 权限配置 | 声明允许调用的 MCP tool 白名单 | `.claude/settings.local.json` | MCP Client 权限模型 | JSON |
| Spec 文件 (8 个) | OpenSpec 规范文档，引用 tool 名称 | `openspec/specs/*/spec.md` | — | Markdown |

### 组件图

```
技能/SKILL.md ──引用──> mcp__plugin_dev-team_dev-team__phase_check
                              mcp__plugin_dev-team_dev-team__phase_log
                              mcp__plugin_dev-team_dev-team__phase_next
                              (另有 archi_*, config_* 系列)

Agent/agent.md ──引用──> mcp__plugin_dev-team_dev-team__phase_log
                              mcp__plugin_dev-team_dev-team__archi_query
                              mcp__plugin_dev-team_dev-team__archi_validate
                              mcp__plugin_dev-team_dev-team__archi_write
                              mcp__plugin_dev-team_dev-team__archi_check

settings.local.json ──许可──> mcp__plugin_dev-team_dev-team__phase_check
                              mcp__plugin_dev-team_dev-team__phase_log

mcp.ts ──registerTool──> phase_log
                         phase_check
                         phase_next
                         archi_query
                         archi_validate
                         archi_write
                         archi_check
                         config_get
                         config_set
                         config_unset
                         config_context
```

---

## 数据流

### 流程描述

本次变更是纯重命名操作，不改动任何业务逻辑和数据结构。数据流保持不变，仅 tool 标识符发生变化：

1. MCP Client 发起 `tools/call` 请求，`params.name` 使用新名称（如 `"phase_log"`）
2. MCP Server 收到请求，根据 `name` 路由到对应的处理函数（路由逻辑不变，仅 switch/case 或 registerTool 的 key 变化）
3. 处理函数执行完成后返回结果
4. MCP Client 收到响应

各 tool 的输入/输出 schema 完全不变，仅名称变更。

### 数据模型

不涉及数据模型变更。以下文件内容被更新但结构不变：

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `mcp.ts` | 替换字符串 | 11 处 `registerTool` 名称从 `xx/yy` 改为 `xx_yy` |
| `dev-team-mcp.cjs` | 重新构建 | 构建产物反映新名称 |
| 11 个 `SKILL.md` 文件 | 替换字符串 | MCP tool FQN 引用中的 `/` 改为 `_` |
| 10 个 `agent.md` 文件 | 替换字符串 | MCP tool FQN 引用中的 `/` 改为 `_` |
| `settings.local.json` | 替换字符串 | 权限条目从 `eval/check`/`eval/log` 改为 `phase_check`/`phase_log` |
| 8 个 spec 文件 | 替换字符串 | 工具名称引用从 `xx/yy` 改为 `xx_yy` |

---

## 路由 / API 设计

不涉及 API 变更。MCP tool 的输入/输出 schema、HTTP 路由、CLI 命令均保持不变。唯一变更的是 tool 注册名称和全限定名（FQN）中的分隔符。

### 名称映射表

| 当前名称 | 新名称 | 变更文件数 |
|----------|--------|-----------|
| `phase/log` | `phase_log` | mcp.ts + 11 skills + 8 agents + 2 spec |
| `phase/check` | `phase_check` | mcp.ts + 9 skills + 2 spec |
| `phase/next` | `phase_next` | mcp.ts + 1 skill + 1 spec |
| `archi/query` | `archi_query` | mcp.ts + 1 agent + 1 spec |
| `archi/validate` | `archi_validate` | mcp.ts + 1 agent + 1 spec |
| `archi/write` | `archi_write` | mcp.ts + 1 agent + 1 spec |
| `archi/check` | `archi_check` | mcp.ts + 1 agent + 1 spec |
| `config/get` | `config_get` | mcp.ts + 1 spec |
| `config/set` | `config_set` | mcp.ts + 1 spec |
| `config/unset` | `config_unset` | mcp.ts + 1 spec |
| `config/context` | `config_context` | mcp.ts + 1 spec |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 使用下划线 `_` 作为 MCP tool 名称分隔符，替换 `/` | (1) 与 MCP FQN 的 `__` 分隔符风格一致，避免两种分隔符混用；(2) 避免 MCP 客户端在路由/URI 解析中对 `/` 的特殊处理问题；(3) MCP 生态主流命名趋势倾向于使用下划线 | 方案 A（保留 `/`）：保留现有层级命名，但无法解决 FQN 可读性和兼容性问题。方案 B（使用 `-` 连字符）：如 `phase-log`，但 MCP FQN 中的连字符在部分 shell 上下文中可能导致歧义 |
| D2 | 保持 build 流程不变，仅修改源码后重新构建 `dev-team-mcp.cjs` | 编译产物是源码的直接映射，修改源码后重新构建即可确保一致性，无需手动修改产物 | 方案 A（直接编辑 `.cjs` 文件）：手动搜索替换 `.cjs` 中的名称，但可能与源码不同步，且 `.cjs` 是混淆/压缩产物，手动维护困难 |
| D3 | 不在重命名变更中更新已归档的旧设计文档 | `openspec/changes/archive/2026-05-29-mcp-hierarchical-commands/` 中的设计文档是历史记录，反映了该变更做出时的状态。修改历史记录会破坏审计追踪 | 方案 A（更新归档）：虽然能保持一致性，但会破坏审计记录，不必要。用户可通过 git blame 追溯完整历史 |
| D4 | 所有 30+ 个文件（源码、技能、agent、权限、spec）使用系统化搜索替换统一处理 | 避免遗漏任何一个引用点，确保一致性。每个引用的替换模式高度可预测（`xx/yy` -> `xx_yy`） | 方案 A（逐个文件手动修改）：风险高，容易遗漏。方案 B（仅修改源码，不修改文档）：会导致技能/agent 文档不准确 |

---

## 依赖

### 构建依赖

- `vite-plus` (`vp`) — 构建工具，用于将 `mcp.ts` 打包为 `dev-team-mcp.cjs`
- `TypeScript ^6.0.3` — 类型检查
- `@modelcontextprotocol/sdk ^1.29.0` — MCP server SDK，运行时依赖

### 运行时依赖

- `node` — MCP server 运行环境
- `@modelcontextprotocol/sdk` — MCP server SDK

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 全局搜索替换遗漏部分文件 | 运行时 MCP tool 找不到（tool not found） | 低 | 使用 grep 遍历所有文件类型，验证无残留 `xx/yy` 格式 tool 名称 |
| settings.local.json 遗漏旧 `eval/` 条目清理 | 权限检测异常或冗余条目 | 中 | 清理旧条目（`eval/check`, `eval/log`），只保留新下划线格式条目 |
| 构建产物 dev-team-mcp.cjs 使用旧名称 | MCP Server 注册的 tool 名称仍为 `xx/yy` | 低 | 构建后用 grep 验证产物中无 `xx/yy` 格式 tool 名称 |
| spec 文件中的工具名称引用不一致 | 规范文档与实际行为脱节 | 低 | 所有 8 个 spec 文件逐一审查并更新 |
| 工作流执行过程中被中断，中间状态使用旧名称 | 已运行的技能流程调用旧名称失败 | 低 | 重命名应在工作流空闲时执行；用户重新触发工作流即可 |

---

## 迁移步骤

1. 修改 `plugins/dev-team/bin/src/mcp.ts` — 11 处 `registerTool` 名称从 `xx/yy` 改为 `xx_yy`
2. 执行 `cd plugins/dev-team/bin && npm run build` 重新构建 `dev-team-mcp.cjs`
3. 更新 11 个 skill 文件中所有 `mcp__plugin_dev-team_dev-team__*/*` 引用为 `mcp__plugin_dev-team_dev-team__*_*`
4. 更新 10 个 agent 文件中所有 `mcp__plugin_dev-team_dev-team__*/*` 引用为 `mcp__plugin_dev-team_dev-team__*_*`
5. 更新 `.claude/settings.local.json` — 将 `eval/check` 和 `eval/log` 替换为 `phase_check` 和 `phase_log`
6. 更新 8 个 spec 文件中的 tool 名称引用
7. 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号从 `2.5.4` 到 `2.6.0`
8. 全局 grep 验证无残留 `xx/yy` 格式的 MCP tool 名称

## 待决问题

- 无
