# 提案: 将 Hook 脚本从 Bash 重写为 Node.js

> **变更**: rewrite-hooks-to-node
> **日期**: 2026-06-18
> **状态**: 提案

---

## 问题

dev-team 插件的两个 hook 脚本（`static-check.sh`、`protect-eval.sh`）通过 `hooks.json` 以 `bash` 命令调用。在 Windows 上，Cursor 默认使用 PowerShell 作为 shell，`bash` 不在 PATH 中，导致所有 hook 静默失败：

```
The term 'bash' is not recognized as the name of a cmdlet, function, script file, or operable program.
```

后果：

1. **eval.json 保护失效**：`protect-eval.sh` PreToolUse hook 无法运行，agent 可直接 Write/Edit/Bash 写入 `eval.json`，绕过 `phase_log` 校验。
2. **静态检查门禁失效**：`static-check.sh` subagentStop hook 无法运行，`implementation-generator` 结束时不会触发静态分析，lint/类型错误可能进入后续阶段。
3. **跨平台不一致**：在 macOS/Linux 上 hook 正常工作，在 Windows 上完全失效，开发者体验分裂。

现有集成测试通过 `resolveBash()` 在 Windows 上查找 Git Bash 来运行 `.sh` 脚本，但这仅覆盖 CI/本地测试路径，无法修复 Cursor 运行时 hook 调用失败的问题。

---

## 提案

将两个 hook 脚本从 Bash（`.sh`）重写为 Node.js ES Module（`.mjs`），并更新 `hooks.json` 使用 `node` 而非 `bash` 调用。Node.js 在 Cursor 环境中始终可用，与平台无关。

### 核心设计

1. **`plugins/dev-team/hooks/scripts/static-check.mjs`** — 替代 `static-check.sh`：
   - 使用 `child_process.spawnSync` 调用 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs" run_static_analysis`
   - 使用 `JSON.stringify` 输出 `{}` 或 `{ "followup_message": "..." }`
   - 行为与现有 bash 脚本完全一致（exit code 判断、中文 followup 前缀、stdout/stderr 合并）

2. **`plugins/dev-team/hooks/scripts/protect-eval.mjs`** — 替代 `protect-eval.sh`：
   - 从 stdin 读取 JSON，使用原生 `JSON.parse` 解析（移除 jq/grep 回退逻辑）
   - 保留全部拦截逻辑：Write/Edit 路径匹配、Bash 重定向/tee/heredoc 检测、python/node 豁免、fail-open 策略
   - 输出格式与现有 bash 脚本完全一致

3. **`plugins/dev-team/hooks/hooks.json`** — 三处 command 字段更新：
   - `bash "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.sh"` → `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs"`
   - `bash "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.sh"` → `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"`

4. **删除旧 `.sh` 文件**，避免维护两套实现。

5. **更新集成测试** — `protect-eval-regression.test.ts` 和 `static-check-hook-e2e.test.ts` 改为通过 `node` 调用 `.mjs` 脚本，移除对 `resolveBash()` 的依赖。

### 不采用的替代方案

| 方案 | 不采用原因 |
|------|-----------|
| 在 hooks.json 中使用 Git Bash 绝对路径 | 路径因安装位置而异，不可移植；未安装 Git 的用户仍无法使用 |
| 使用 PowerShell 脚本（`.ps1`） | 需维护 Windows/macOS/Linux 两套脚本；Node 已跨平台可用 |
| 保留 `.sh` 并添加 `.cmd` 包装器 | 增加复杂度，Node 单实现更简洁 |
| 将 hook 逻辑移入 `dev-team-cli.cjs` 子命令 | hook 协议要求独立 stdin/stdout 进程；CLI 子命令可行但不如独立 `.mjs` 清晰 |

---

## 能力

### 新增能力

（无）

### 修改的能力

- **static-check-hook** — hook 脚本实现语言从 Bash 改为 Node.js（`.mjs`），`hooks.json` command 从 `bash` 改为 `node`
- **eval-json-protection** — hook 脚本实现语言从 Bash 改为 Node.js（`.mjs`），`hooks.json` command 从 `bash` 改为 `node`

---

## 变更范围

### 实现以下特性

- `plugins/dev-team/hooks/scripts/static-check.mjs` — 新增，功能等价于 `static-check.sh`
- `plugins/dev-team/hooks/scripts/protect-eval.mjs` — 新增，功能等价于 `protect-eval.sh`
- `plugins/dev-team/hooks/hooks.json` — 三处 command 改为 `node ... .mjs`
- 删除 `plugins/dev-team/hooks/scripts/static-check.sh`
- 删除 `plugins/dev-team/hooks/scripts/protect-eval.sh`
- `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` — 改为调用 `.mjs`
- `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` — 改为调用 `.mjs`
- `plugins/dev-team/.claude-plugin/plugin.json` — 版本号 patch bump（`2.6.22` → `2.6.23`）

### 不要修改

- hook 拦截/放行逻辑的业务规则（路径匹配、Bash 写入检测、followup 消息格式）
- `dev-team-cli.cjs run_static_analysis` CLI 实现
- `phase_log` MCP 工具及 eval.json 写入校验逻辑
- 其他 bash 脚本（如 `plugins/dev-team/utils/openspec-cli.sh`）— 不在 hook 调用链中
- agent 定义、skill 文件、MCP server 代码

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `hooks.json` 中三处 hook command 均使用 `node` 调用 `.mjs` 脚本 | 读取 `hooks.json`，确认无 `bash` 引用，路径指向 `.mjs` |
| AC-2 | Windows PowerShell 环境下 hook 可被 Cursor 正常调用 | 在 Windows 上手动执行 `node plugins/dev-team/hooks/scripts/protect-eval.mjs` 和 `static-check.mjs`，确认 exit 0 且输出合法 JSON |
| AC-3 | eval.json Write/Edit 拦截行为与迁移前一致 | 运行 `protect-eval-regression.test.ts` 全部通过 |
| AC-4 | eval.json Bash 重定向/tee/heredoc 拦截与 python/node 豁免与迁移前一致 | 运行 `protect-eval-regression.test.ts` 全部通过 |
| AC-5 | 静态检查通过时 hook 输出 `{}` | 运行 `static-check-hook-e2e.test.ts` 对应场景通过 |
| AC-6 | 静态检查失败时 hook 输出含 `followup_message` 的 JSON | 运行 `static-check-hook-e2e.test.ts` 对应场景通过 |
| AC-7 | 旧 `.sh` 文件已删除，仓库中无残留引用 | `grep -r "static-check.sh\|protect-eval.sh" plugins/dev-team/` 无匹配（测试文件除外已更新） |
| AC-8 | `plugin.json` 版本号已递增 | 读取 plugin.json，确认 version > `"2.6.22"` |
| AC-9 | 空 stdin 或缺少字段时 protect-eval 默认放行（fail-open） | 集成测试覆盖空输入场景 |
| AC-10 | CLI 不存在时 static-check 返回 followup_message 而非崩溃 | 集成测试或手动验证 `CLAUDE_PLUGIN_ROOT` 指向无 CLI 的目录 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Node.js 重写遗漏 bash 脚本的边界行为 | 拦截漏网或误拦 | 中 | 保留现有集成测试套件，迁移后全部通过；protect-eval 逻辑逐段对照移植 |
| `${CLAUDE_PLUGIN_ROOT}` 在 Windows 路径含空格时解析失败 | hook 无法找到脚本 | 低 | hooks.json 已用双引号包裹路径；Node 脚本使用 `path.join` 拼接 |
| 删除 `.sh` 后依赖 bash 的外部文档或脚本失效 | 文档/脚本过时 | 低 | 变更范围内 grep 确认无残留引用；spec delta 更新 Module Contract |
| ESM `.mjs` 与 CommonJS CLI 混用 | 模块加载错误 | 低 | hook 脚本通过 `child_process` 调用 CLI，不直接 require CJS 模块 |
