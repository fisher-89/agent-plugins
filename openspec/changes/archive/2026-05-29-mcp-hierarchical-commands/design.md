# 设计: mcp-hierarchical-commands

> **变更**: mcp-hierarchical-commands
> **日期**: 2026-05-29
> **基于**: proposal.md, specs/mcp-tool-namespace/spec.md, specs/phase-skills/spec.md, specs/phase-agents/spec.md, specs/eval-check-cli/spec.md

---

## 架构组件

本变更不引入新组件，也不修改业务逻辑——仅对 6 个 MCP tool 名称进行重命名（扁平 `xx_yy` 格式改为层级 `xx/yy` 格式），并更新所有引用点。

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| MCP Server | 注册 6 个 tool 的 `name` 字段和 `handleToolCall` 路由映射 | `plugins/dev-team/bin/src/mcp.ts` | `commands/eval-log.ts`, `commands/eval-check.ts`, `lib/archi-query.ts`, `lib/archi-validate.ts`, `lib/archi-write.ts`, `lib/c4-cross-ref.ts` | TypeScript |
| MCP 编译产物 | 编译后的 MCP server 可执行文件 | `plugins/dev-team/bin/dev-team-mcp.cjs` | `bin/src/mcp.ts`（源文件） | JavaScript (CJS) |
| Phase Skills (10 个) | 工作流阶段编排器，通过 MCP tool 进行 gate check 和 eval-logging | `plugins/dev-team/skills/*/SKILL.md` | `mcp__plugin_dev-team_dev-team__eval/check`, `mcp__plugin_dev-team_dev-team__eval/log` | Markdown (skill) |
| Subagent (10 个) | 专业领域 agent，通过 MCP tool 执行架构操作和评估记录 | `plugins/dev-team/agents/*.md` | `mcp__plugin_dev-team_dev-team__eval/log`, `mcp__plugin_dev-team_dev-team__archi/*` | Markdown (agent) |
| 权限配置 | Claude Code 的 MCP tool 权限 allowlist | `.claude/settings.local.json` | MCP server 注册的 tool 名称 | JSON |

### 组件关系

```
bin/src/mcp.ts (tool name 定义 + 路由) ──构建──> bin/dev-team-mcp.cjs (编译产物)
       │
       ├── skills/*/SKILL.md 引用 ── mcp__plugin_dev-team_dev-team__eval/check
       ├── skills/*/SKILL.md 引用 ── mcp__plugin_dev-team_dev-team__eval/log
       ├── agents/architecture.md 引用 ── mcp__plugin_dev-team_dev-team__archi/*
       ├── agents/*-evaluator.md 引用 ── mcp__plugin_dev-team_dev-team__eval/log
       └── .claude/settings.local.json 权限 allowlist
```

---

## 数据流

### 重命名映射

6 个 tool 执行一对一的命名变更。参数 schema、返回值、业务逻辑均保持不变。

| 作用域 | 旧名称 | 新名称 |
|--------|--------|--------|
| Tool name (简短) | `eval_log` | `eval/log` |
| Tool name (简短) | `eval_check` | `eval/check` |
| Tool name (简短) | `archi_query` | `archi/query` |
| Tool name (简短) | `archi_validate` | `archi/validate` |
| Tool name (简短) | `archi_write` | `archi/write` |
| Tool name (简短) | `archi_check` | `archi/check` |
| FQN (完全限定) | `mcp__plugin_dev-team_dev-team__eval_log` | `mcp__plugin_dev-team_dev-team__eval/log` |
| FQN (完全限定) | `mcp__plugin_dev-team_dev-team__eval_check` | `mcp__plugin_dev-team_dev-team__eval/check` |
| FQN (完全限定) | `mcp__plugin_dev-team_dev-team__archi_query` | `mcp__plugin_dev-team_dev-team__archi/query` |
| FQN (完全限定) | `mcp__plugin_dev-team_dev-team__archi_validate` | `mcp__plugin_dev-team_dev-team__archi/validate` |
| FQN (完全限定) | `mcp__plugin_dev-team_dev-team__archi_write` | `mcp__plugin_dev-team_dev-team__archi/write` |
| FQN (完全限定) | `mcp__plugin_dev-team_dev-team__archi_check` | `mcp__plugin_dev-team_dev-team__archi/check` |

### 变更波及范围

变更按依赖关系分层传播：

1. **源定义层** (`mcp.ts`) — TOOLS 数组的 `name` 字段和 `handleToolCall` switch case 更新
2. **编译层** (`dev-team-mcp.cjs`) — 重新构建以包含源文件变更
3. **引用层 — Skills** (10 个文件) — eval/check 和 eval/log 的完全限定名引用更新
4. **引用层 — Agents** (10 个文件) — eval/log 和 archi/* 的完全限定名引用更新
5. **配置层** (`settings.local.json`) — 权限 allowlist 中 eval/check 和 eval/log 的完全限定名更新

变更不会影响：
- 底层命令实现（`commands/eval-log.ts`, `commands/eval-check.ts`, `lib/archi-*.ts`）
- CLI 命令 `dev-team eval-check`, `dev-team eval-log`
- `bin/openspec-bundled.js`（使用 CLI 子命令，不受 MCP tool 名影响）
- `eval.json` 数据格式和内容
- `.cursor/commands/opsx-archive.md`（引用 JSON 结果字段名，非 MCP tool 名称）

---

## 路由/API 设计

本变更不新增或修改 API 端点。MCP tool 名称变更对照如下：

| MCP Method | 旧 Tool Name | 新 Tool Name | 变更内容 |
|------------|-------------|-------------|----------|
| `tools/call` | `eval_log` | `eval/log` | `handleToolCall` case 标签 |
| `tools/call` | `eval_check` | `eval/check` | `handleToolCall` case 标签 |
| `tools/call` | `archi_query` | `archi/query` | `handleToolCall` case 标签 |
| `tools/call` | `archi_validate` | `archi/validate` | `handleToolCall` case 标签 |
| `tools/call` | `archi_write` | `archi/write` | `handleToolCall` case 标签 |
| `tools/call` | `archi_check` | `archi/check` | `handleToolCall` case 标签 |
| `tools/list` | TOOLS 数组 | TOOLS 数组 | 6 个 tool 的 `name` 字段 |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 使用 `/` 作为层级分隔符 | MCP 协议规范允许任意字符串作为 tool name，`/` 是 URL 路径的惯用层级分隔符，语义清晰。Claude Code UI/CLI 中已在其他位置使用 `/` 作为层级分隔符（如 skill 名称 `dev-team:phase-requirements`），具有一致性 | `:` 分隔符（如 `eval:log`）：与 plugin 命名空间分隔符 `:` 冲突，容易混淆；`.` 分隔符（如 `eval.log`）：可能被误解为属性访问或文件扩展名 |
| D2 | 保持参数 schema 和返回值不变 | 变更范围最小化，避免下游 consumer（skills, agents, 脚本）需要同步调整参数处理逻辑。改名后所有现有调用代码只需更新 tool name 字符串，参数传法完全一致 | 同时清理重构参数名：引入破坏性变更，需要同步修改所有引用点和下游处理逻辑，风险更高 |
| D3 | 一次性更新所有引用文件（而非分阶段兼容过渡） | 旧名称被删除后，任何未更新的引用都会导致运行时 `Unknown tool` 错误。一次性搜索替换并验证可以确保原子性，避免遗留旧引用 | 旧名+新名同时注册兼容期：增加 MCP server 复杂度，且过渡期价值不大（所有调用方都在同一仓库中） |

---

## 依赖

### 构建/测试依赖

- `node` / `npm` — 用于重新构建 `bin/dev-team-mcp.cjs`
- `grep` / `Select-String` — 用于验证无旧名称引用残留

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| MCP tool 名称含 `/` 导致 JSON-RPC 解析异常 | 工具调用失败 | 低 | MCP 协议规范允许任意字符串作为 tool name；在 Claude Code 中进行本地端到端验证 |
| skill/agent 文件遗漏未更新 | 运行时调用旧名称返回 `Unknown tool` 错误 | 中 | 在 tasks.md 中设专门的验证步骤，使用 grep 全局搜索旧名称模式确保无残留 |
| 编译产物不一致 | `dev-team-mcp.cjs` 使用旧名称 | 中 | 构建后验证产物中 `TOOLS` 数组使用 `/` 格式、无 `_` 格式的 name；使用 diff/look 快速确认 |
| `archi/query` 等 archi tool 未被任何 skill 引用，仅 agent 引用 | 遗漏更新 architecture.md 的风险隐蔽 | 低 | 验证步骤中 grep 所有旧名称，archi_* 均会暴露 |

---

## 迁移步骤

1. 修改 `plugins/dev-team/bin/src/mcp.ts` 中的 6 个 tool name 定义
2. 更新 `handleToolCall` 中 6 个 switch case 标签
3. 重新构建 `plugins/dev-team/bin/dev-team-mcp.cjs`
4. 更新 10 个 skill 文件中的 MCP tool 引用
5. 更新 10 个 agent 文件中的 MCP tool 引用
6. 更新 `.claude/settings.local.json` 中的权限配置
7. 全局 grep 验证无旧名称残留

---

## 待决问题

- 无（所有变更路径已明确）
