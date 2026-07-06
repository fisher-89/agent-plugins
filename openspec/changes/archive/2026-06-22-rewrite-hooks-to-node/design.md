# 设计: rewrite-hooks-to-node

> **变更**: rewrite-hooks-to-node
> **日期**: 2026-06-18
> **基于**: proposal.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Hook 声明 | 注册 PreToolUse / SubagentStop 命令 hook，将 `${CLAUDE_PLUGIN_ROOT}` 解析为插件根目录 | `plugins/dev-team/hooks/hooks.json` | Cursor Hook 框架 | JSON |
| protect-eval.mjs | PreToolUse hook：拦截 agent 对 `openspec/changes/**/eval.json` 的直接 Write/Edit/Bash 写入 | `plugins/dev-team/hooks/scripts/protect-eval.mjs` | Node.js 内置模块 | Node.js ESM (`.mjs`) |
| static-check.mjs | SubagentStop hook：`implementation-generator` / `test-gen-generator` 结束时调用静态分析 CLI，按结果输出 `{}` 或 `{ decision: "block", reason: "..." }` | `plugins/dev-team/hooks/scripts/static-check.mjs` | `dev-team-cli.cjs`、`child_process` | Node.js ESM (`.mjs`) |
| dev-team-cli.cjs | 执行 `run_static_analysis` 子命令（读取 `openspec/config.json`，运行 lint/类型检查） | `plugins/dev-team/bin/dev-team-cli.cjs` | `openspec/config.json` | Node.js CJS（不变） |
| 插件元数据 | 版本号标识 hook 运行时迁移 | `plugins/dev-team/.claude-plugin/plugin.json` | — | JSON |

### 组件图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Cursor Hook 运行时（跨平台）                          │
└─────────────────────────────────────────────────────────────────────────┘
         │                                    │
         │ PreToolUse                         │ SubagentStop
         │ (Write|Edit / Bash)                │ (implementation-generator)
         ▼                                    ▼
  node protect-eval.mjs              node static-check.mjs
         │                                    │
         │ stdin: tool JSON                   │ spawnSync
         │ stdout: permissionDecision         ▼
         │                              node dev-team-cli.cjs
         │                              run_static_analysis
         │                                    │
         │                                    │ exit 0 → stdout: {}
         │                                    │ exit ≠0 → stdout: { decision: "block", reason: "..." }
         ▼                                    ▼
  allow / deny                         允许或阻止 subagent 结束
```

**迁移前后唯一变化**：`hooks.json` 的 `command` 从 `bash ... .sh` 改为 `node ... .mjs`；hook 业务逻辑与 I/O 契约保持不变。

---

## 数据流

### 流程描述

#### PreToolUse — eval.json 保护（protect-eval.mjs）

1. Cursor 在 agent 调用 Write、Edit 或 Bash 工具前，执行 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs"`。
2. 脚本通过 `fs.readFileSync(0, 'utf-8')`（或等价 async 读取）消费 stdin 中的工具调用 JSON。
3. `JSON.parse` 提取 `tool_name`、`tool_input.file_path`、`tool_input.command`。
4. 按工具类型分支：
   - **Write/Edit**：归一化路径（`\` → `/`），匹配 `openspec/changes/.*eval\.json$` → deny；否则 allow。
   - **Bash**：归一化命令，豁免 `python`/`python3`/`node` 前缀；检测 `>`/`>>`/`tee`/heredoc/`>&` 写入模式 → deny；否则 allow。
5. 向 stdout 输出 `hookSpecificOutput` JSON，`process.exit(0)`。
6. 空 stdin、解析失败、缺少字段、未知工具 → **fail-open**（allow）。

#### SubagentStop — 静态检查（static-check.mjs）

1. `implementation-generator` 尝试结束时，Cursor 执行 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"`。
2. 脚本忽略 stdin（可读取但不依赖）。
3. 拼接 CLI 路径：`path.join(process.env.CLAUDE_PLUGIN_ROOT, 'bin', 'dev-team-cli.cjs')`。
4. CLI 不存在 → 输出 `{ decision: "block", reason: "dev-team CLI not found at ..." }`，exit 0。
5. `spawnSync(process.execPath, [cliPath, 'run_static_analysis'], { encoding: 'utf-8' })` 执行静态分析。
6. 合并 stdout + stderr 作为 CLI 输出。
7. CLI exit 0 → stdout `{}`；非 0 → stdout `{ decision: "block", reason: "<前缀><CLI输出>" }`；脚本始终 exit 0。

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| PreToolUse 输入 | `tool_name: string`；`tool_input.file_path?: string`；`tool_input.command?: string` | 由 Cursor 注入 stdin | 无（瞬态） |
| PreToolUse 输出 | `hookSpecificOutput.hookEventName`；`permissionDecision: "allow"\|"deny"`；`permissionDecisionReason?: string` | deny 时 reason 含变更名与 phase_log 指引 | 无 |
| SubagentStop 输出（通过） | `{}` | — | 无 |
| SubagentStop 输出（失败） | `decision: "block"`, `reason: string` | 含中文前缀 + CLI 完整输出 | 无 |
| hooks.json | `hooks.PreToolUse[]`、`hooks.SubagentStop[]`；每项含 `matcher`、`command`、`loop_limit?` | command 引用 `.mjs` 脚本 | `plugins/dev-team/hooks/hooks.json` |

---

## 路由 / Hook 协议设计

本变更无 HTTP API；以下为 Cursor Hook 命令契约。

| 事件 | Matcher | Command | 输入 (stdin) | 输出 (stdout) | 脚本 exit code |
|------|---------|---------|--------------|---------------|----------------|
| PreToolUse | `Write\|Edit` | `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs"` | 工具调用 JSON | `{ hookSpecificOutput: { permissionDecision, ... } }` | 始终 `0` |
| PreToolUse | `Bash` | 同上 | 工具调用 JSON | 同上 | 始终 `0` |
| SubagentStop | `implementation-generator` / `test-gen-generator`（`loop_limit: 5`） | `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"` | subagentStop 事件 JSON（可忽略） | `{}` 或 `{ decision: "block", reason: "..." }` | 始终 `0` |

**认证**：无；hook 由 Cursor 本地进程启动，依赖 `${CLAUDE_PLUGIN_ROOT}` 环境变量定位插件文件。

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 使用 Node.js ESM（`.mjs`）独立脚本，而非内联到 `dev-team-cli.cjs` | Hook 协议要求独立 stdin/stdout 进程；`.mjs` 与现有 CJS CLI 通过 `child_process` 隔离，避免模块系统混用 | 将 hook 逻辑作为 CLI 子命令 — 可行但混淆 hook 边界与 CLI 职责 |
| D2 | `hooks.json` command 使用 `node` 而非 `bash` | Cursor 在 Windows 默认 PowerShell，`bash` 不在 PATH；Node 在 Cursor 环境中始终可用 | Git Bash 绝对路径 — 不可移植；PowerShell `.ps1` — 需维护多平台两套脚本 |
| D3 | protect-eval 使用原生 `JSON.parse`，移除 jq/grep 回退 | Node 环境保证可用；简化实现，消除 bash 版三层解析回退的维护负担 | 保留 jq 回退 — 在 `.mjs` 中无意义 |
| D4 | static-check 使用 `JSON.stringify` 生成 followup JSON | 替代 bash 手工 `json_escape`，正确处理换行、引号、反斜杠 | 模板字符串拼接 — 易遗漏转义，已有 bash 版 bug 风险 |
| D5 | 删除 `.sh` 文件，不保留双实现 | 避免两套逻辑漂移；grep 确认无残留引用 | 保留 `.sh` 供 macOS/Linux — 增加维护成本且无运行时收益 |
| D6 | 业务逻辑逐函数对照 bash 移植，不改规则 | 现有集成测试是行为等价性的安全网；变更容易 review | 重写时"优化"正则 — 可能引入回归 |
| D7 | 路径拼接使用 `path.join(CLAUDE_PLUGIN_ROOT, ...)` | Windows 路径含反斜杠或空格时更安全；与 hooks.json 双引号包裹互补 | 字符串拼接 — 空格路径可能失败 |

---

## 依赖

### 运行时依赖

- **Node.js** — 执行 `.mjs` hook 脚本及 `dev-team-cli.cjs`（Cursor 内置，跨平台）
- **`CLAUDE_PLUGIN_ROOT` 环境变量** — Cursor 注入，指向插件根目录
- **`dev-team-cli.cjs`** — static-check 调用 `run_static_analysis`（本变更不修改）

### 构建/测试依赖

- **vite-plus/test（Vitest）** — 现有集成测试通过 `execFileSync('node', [scriptPath])` 黑盒验证 hook 行为
- **现有测试套件** — `protect-eval-regression.test.ts`、`static-check-hook-e2e.test.ts`、`hooks-json-structure.test.ts` 需同步更新路径与断言

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Node 重写遗漏 bash 边界行为（tee -a、heredoc、>& 等） | 拦截漏网或误拦 | 中 | 逐段对照 `protect-eval.sh` 移植正则；现有回归测试全部通过 |
| `${CLAUDE_PLUGIN_ROOT}` 含空格时路径解析失败 | hook 找不到脚本/CLI | 低 | hooks.json 双引号 + `path.join` |
| 删除 `.sh` 后文档/测试残留引用 | 混淆或 CI 失败 | 低 | grep 全仓；更新 hooks-json-structure 测试 |
| ESM 与 CJS 混用导致 require 错误 | hook 启动失败 | 低 | hook 仅通过 `spawnSync` 调用 CLI，不直接 import CJS |
| protect-eval heredoc 检测过宽（任何 `<<` 即 deny） | 误拦含 heredoc 但非写 eval 的命令 | 低 | 与 bash 版行为一致，不在本变更中修改规则 |

---

## 迁移步骤

1. **新增 `static-check.mjs`** — 对照 `static-check.sh` 实现 CLI 调用、followup 前缀、`JSON.stringify` 输出。
2. **新增 `protect-eval.mjs`** — 对照 `protect-eval.sh` 移植路径匹配、Bash 写入检测、deny reason 模板、fail-open 策略。
3. **更新 `hooks.json`** — 三处 `command` 改为 `node ... .mjs`。
4. **删除 `static-check.sh`、`protect-eval.sh`**。
5. **递增 `plugin.json` version** — `2.6.22` → `2.6.23`。
6. **更新集成测试** — 测试 helper 改用 `node` + `.mjs` 路径；移除 `resolveBash()` 依赖；`bash -n` 语法检查替换为 node 可执行性验证。
7. **全仓 grep** — 确认 `plugins/dev-team/` 无 `.sh` 残留引用。
8. **验收** — Windows PowerShell 手动执行两个 `.mjs`；运行集成测试套件。

---

## 待决问题

- 无。proposal 已明确范围与不采用方案；实现为 bash → Node 的等价移植。
