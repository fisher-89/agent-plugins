# 设计: protect-eval-json

> **变更**: protect-eval-json
> **日期**: 2026-06-08
> **基于**: proposal.md, specs/eval-json-protection/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Hook 声明文件 | 向 Claude Code 注册 PreToolUse hook，指定匹配的工具和调用的脚本 | `plugins/dev-team/hooks/hooks.json` | 无 | JSON (Claude Code hooks 协议) |
| 保护拦截脚本 | 从 stdin 接收工具调用 JSON，判断是否涉及 eval.json 写入，返回 allow/deny 决策 | `plugins/dev-team/hooks/scripts/protect-eval.sh` | jq (JSON 解析) | POSIX shell (bash) |
| 插件元数据 | 声明插件的版本号，hook 机制通过插件发现自动激活 | `plugins/dev-team/.claude-plugin/plugin.json` | 无 | JSON |
| MCP phase_log 工具 | 验证、构建并写入 eval.json 条目的官方途径 | `plugins/dev-team/bin/src/commands/phase-log.ts` | eval-json.ts, Zod schemas | TypeScript / MCP SDK |

### 组件图

Claude Code 在 agent 调用内置工具 (Write/Edit/Bash) 前，检查 `plugins/dev-team/hooks/hooks.json` 中注册的 PreToolUse hook。匹配工具的调用会执行 `protect-eval.sh` 脚本，脚本返回 deny 决策时该调用被阻止。MCP 工具调用（包括 `phase_log`）不经过 PreToolUse hook，始终不受影响。

```
Agent 调用工具
       |
       v
+------------------+    matched     +----------------------+
| Claude Code      | ------------> | protect-eval.sh      |
| PreToolUse hook  | <------------ | stdin JSON 分析      |
| dispatch         | allow/deny    | 路径/命令匹配        |
+------------------+               +----------------------+
       |
       | (allow)          | (deny)
       v                  v
+--------------+    +-------------------+
| 工具正常执行  |    | 工具调用被拒绝     |
| (MCP/非eval) |    | 返回拒绝原因       |
+--------------+    +-------------------+
       ^
       | (不受 hook 影响)
+----------------------+
| phase_log MCP 工具   |
| (eval.json 唯一入口) |
+----------------------+
```

---

## 数据流

### 流程描述

1. **Hook 注册阶段**：Claude Code 启动时（或插件加载时），读取 `plugins/dev-team/hooks/hooks.json`，注册两条 PreToolUse hook 规则：
   - 规则 1：匹配 `Write` 和 `Edit` 工具，指向 `protect-eval.sh`
   - 规则 2：匹配 `Bash` 工具，指向 `protect-eval.sh`（同一脚本）

2. **工具调用拦截阶段**（每次 agent 调用 Write/Edit/Bash 时触发）：
   - Claude Code 将工具调用详情（JSON，包含 `tool_name`、`tool_input.file_path`/`tool_input.command`）通过 stdin 传入 `protect-eval.sh`
   - 脚本解析 stdin JSON：
     - 若 `tool_name` 为 `Write` 或 `Edit`：提取 `tool_input.file_path`，检查路径是否以 `openspec/changes/**/eval.json` 结尾（支持相对路径和绝对路径）
     - 若 `tool_name` 为 `Bash`：提取 `tool_input.command`，检测是否包含写入 eval.json 的模式（`>`、`>>`、`tee`、heredoc），排除以 `python`/`python3`/`node` 开头的命令
   - 路径或命令命中规则：输出 `permissionDecision: "deny"` + 中文拒绝原因（提及 `phase_log` 和推断的变更名称）
   - 未命中规则：输出 `permissionDecision: "allow"`

3. **正常写入阶段**：`phase_log` MCP 工具调用时，不经过 PreToolUse hook，直接执行 `eval-json.ts` 中的逻辑（验证 ->回溯标记->构建条目->写入文件），正常修改 `eval.json`

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| Hook 声明 | `description`: string, `hooks.PreToolUse[]`: array | 每个条目引用 `protect-eval.sh` 脚本 | `hooks.json` 文件 |
| Hook 输入 (stdin JSON) | `tool_name`: string, `tool_input.file_path?`: string, `tool_input.command?`: string | 由 Claude Code 运行时生成 | 不持久化，仅运行时 |
| Hook 输出 (stdout JSON) | `hookSpecificOutput.permissionDecision`: "allow"\|"deny", `hookSpecificOutput.permissionDecisionReason?`: string | 拒绝时输出原因字段 | 不持久化，仅运行时 |
| 插件元数据 | `name`, `version`, `description`, `bin`, `openspecVersion` | `version` 字段从 `2.5.9` 升至 `2.5.10` | `plugin.json` 文件 |

---

## 路由/API 设计

### Hook 声明 (PreToolUse)

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| PreToolUse | Write/Edit | 拦截对 `openspec/changes/**/eval.json` 的写入和编辑 | `tool_input.file_path` | `permissionDecision: "deny"` + 原因 | 自动注册 |
| PreToolUse | Bash | 拦截通过 shell 命令写入 eval.json 的操作（排除 python/node） | `tool_input.command` | `permissionDecision: "deny"` + 原因 | 自动注册 |

### Hook 输入/输出规范

**输入** (stdin JSON):
```json
{
  "tool_name": "Write | Edit | Bash",
  "tool_input": {
    "file_path": "openspec/changes/test/eval.json",    // Write/Edit 时存在
    "command": "echo '[]' > eval.json"                  // Bash 时存在
  }
}
```

**命中时输出** (stdout JSON):
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "拒绝原因（中文）：eval.json 只能通过 phase_log MCP 工具写入。推断的变更名称：test。建议使用 mcp__plugin_dev-team_dev-team__phase_log 替代。"
  }
}
```

**未命中时输出** (stdout JSON):
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow"
  }
}
```

### MCP 工具（不受此变更影响）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| phase_log | MCP | 唯一合法的 eval.json 写入途径 | `change`, `phase`, `verdict`, `report`, `items` | `{ written, phase, attempt }` | MCP |
| phase_check | MCP | 门控校验（只读 eval.json） | `change`, `phase` | `{ passed, missing }` | MCP |
| phase_next | MCP | 阶段决策（只读 eval.json） | `change`, `workflow_type?` | `{ phase, config, ... }` | MCP |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **插件级 Hook 机制**：使用 `plugins/dev-team/hooks/hooks.json` 声明 PreToolUse hook，而非项目 `.claude/settings.local.json` 配置 | 插件级方案自动传播到所有启用了 dev-team 插件的项目，无需逐项目配置；保护逻辑随插件版本更新统一维护；与 CLAUDE.md "Plugin provides hooks" 架构一致 | **备选 A**: 在 `.claude/settings.local.json` 中配置 `permissions.deny` 规则。**拒绝理由**: 需要每个项目单独配置，无法随插件自动激活；`plugin.json` 不支持 `permissions` 字段；维护成本高 |
| D2 | **单一脚本处理所有工具**：Write/Edit/Bash 共用 `protect-eval.sh`，脚本内通过 `tool_name` 分流 | 减少文件数量，降低维护成本；拦截逻辑本质上都是检测 eval.json 写入意图，共享工具函数和输出格式 | **备选 B**: 为每个工具写独立脚本（如 `protect-eval-write.sh`、`protect-eval-bash.sh`）。**拒绝理由**: 增加了 3 个脚本文件，除了更好的关注点分离外无实质收益；脚本间有大量重复逻辑（JSON 解析、输出格式化） |
| D3 | **Bash 命令检测使用正则匹配写模式**（`>`、`>>`、`tee`、heredoc），而非完全禁止所有含 `eval.json` 的命令 | 允许 `cat eval.json` 等只读读取操作正常执行；只禁止写入意图明显的命令模式，减少误拦截 | **备选 C**: 只要命令字符串包含 `eval.json` 就拦截所有包含该路径的 Bash 命令。**拒绝理由**: 会误拦截 `cat eval.json` 等合法只读操作 |
| D4 | **Python/Node 命令放行**：以 `python`/`python3`/`node` 开头的 Bash 命令不拦截 | dev-team 插件的工具类 Python 脚本和部分 Node.js 脚本可能需要在合规流程中写入 eval.json，这些脚本自身有验证逻辑，不应被 hook 拦截 | **备选 D**: 对所有 Bash 命令一视同仁，不放行 Python/Node。**拒绝理由**: 当工作流中有合规的脚本需要写入 eval.json 时会被拦截，破坏流程（可在后需迭代中添加 allowlist 机制） |
| D5 | **失败放行策略**：输入异常（`tool_input` 缺失、字段为空等）时默认输出 `allow` | 防止因输入格式异常阻塞合法操作；与 Claude Code 安全设计原则一致（hook 故障时不应阻止正常操作） | **备选 E**: 输入异常时默认输出 `deny` 以保守保护 eval.json。**拒绝理由**: 若 hook 脚本本身实现有 bug 或输入格式变化，会导致大量误拦截，影响整个工作流 |
| D6 | **拒绝原因包含推断的变更名称**：从路径或命令中提取 `changes/<name>/` 片段 | 帮助 agent 理解具体是哪个变更的 eval.json 被拦截，提升错误信息的可操作性 | **备选 F**: 仅说明 "eval.json 必须通过 phase_log 写入"。**拒绝理由**: agent 可能同时操作多个变更，没有变更名称不利于 agent 自动纠错 |

---

## 依赖

### 运行时依赖

- **jq** — hook 脚本中用于解析 stdin JSON。Windows 环境下需确保用户安装了 jq，或使用内置的 bash JSON 解析工具（如 `grep`/`sed` 提取字段）

### 构建/测试依赖

- **bash** — 用于验证 hook 脚本语法：`bash -n plugins/dev-team/hooks/scripts/protect-eval.sh`
- **无额外 npm 依赖** — hook 机制不涉及 Node.js 或 TypeScript 编译

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| hook 脚本误拦截合法的 eval.json 写入（如归档流程中合规脚本写入） | 工作流中断，归档失败 | 低 | 归档流程应使用 `phase_log` MCP 工具而非直接文件写入；Python/Node 脚本已在 Bash hook 中放行 |
| hook 脚本的 Bash 命令解析存在遗漏或误判 | 该拦截的命令未拦截，或不该拦截的被拦截 | 中 | 精细的正则匹配 + 严格的场景覆盖（spec 中 8 个 Bash 场景）；验收标准 AC-10 验证无 eval.json 操作不受影响 |
| 用户卸载或禁用插件导致保护失效 | eval.json 恢复无保护状态 | 低 | 这是预期行为——保护随插件存在而存在；用户在禁用插件时应知悉后果 |
| PreToolUse hook 失败导致 agent 执行延迟 | 用户体验下降 | 低 | hook 设计为快速执行（仅路径/命令匹配），无外部依赖；hook 失败（返回非 0 退出码）时不影响原工具调用 |
| 插件更新后 hook 脚本路径变更导致 hooks.json 引用失效 | 保护失效 | 低 | hooks.json 和脚本文件在同一插件内维护，版本更新时同步调整；路径使用 `${CLAUDE_PLUGIN_ROOT}` 变量引用 |

---

## 迁移步骤

1. 创建 `plugins/dev-team/hooks/` 目录（目前不存在）
2. 创建 `plugins/dev-team/hooks/hooks.json` 文件，声明两条 PreToolUse hook（Write/Edit 一条，Bash 一条）
3. 创建 `plugins/dev-team/hooks/scripts/` 目录
4. 创建 `plugins/dev-team/hooks/scripts/protect-eval.sh` 脚本，实现 Write/Edit/Bash 三种工具的拦截逻辑
5. 使用 `bash -n` 验证脚本语法正确性
6. 在 Unix-like 系统上设置 `protect-eval.sh` 的可执行权限
7. 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号从 `2.5.9` 升至 `2.5.10`
8. 验证 AC-1 至 AC-12 验收条件

---

## 待决问题

- 无。所有设计决策已在本文件中覆盖。
