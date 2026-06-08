# 提案: 保护 eval.json 不被 agent 直接写入

> **变更**: protect-eval-json
> **日期**: 2026-06-08
> **状态**: 提案

---

## 问题

`openspec/changes/<name>/eval.json` 是 PGE 工作流中记录各阶段评估结果的核心文件。它存储了每个 phase 的 verdict、检查项明细、回溯标记和跳过低级信息，是 `phase_check` 门控和 `phase_next` 决策的唯一数据源。

当前存在以下问题：

1. **无写入管控**：`eval.json` 应该**仅通过 MCP `phase_log` 工具写入**（该工具负责验证 verdict 合法性、回溯 stale 标记、自动计算 attempt 编号、审计时间戳），但没有任何机制阻止 agent 或 subagent 用内置 `Write`/`Edit` 工具或 Bash 重定向（如 `echo '...' > eval.json`）直接修改它。

2. **数据完整性风险**：直接写入可绕过 `phase_log` 的校验逻辑——包括 verdict 值域检查（必须为 `"pass"` 或 `"fail"`）、report 长度限制（500 字符）、回溯 stale 传播、`skipped` 与 `verdict` 的一致性约束。受损的 eval.json 会导致门控和回溯决策基于不可靠的数据。

3. **审计追溯失效**：`phase_log` 自动生成的时间戳和维护的 `attempt` 编号是审计链的关键。直接写入可伪造时间戳或重复 attempt 编号，破坏评估序列的可审计性。

4. **agent 行为校正缺失**：当 agent 尝试直接写入 eval.json 时，没有被拦截或引导去使用正确的 `phase_log` 工具。agent 不会自动知道应该改用哪个替代工具。

---

## 提案

在 **`plugins/dev-team/` 插件内部**增加拦截能力，而非在项目 `.claude/settings.local.json` 中配置。当插件被安装或启用时，保护自动激活，无需每个项目单独配置。

### 方案：插件 Hook 机制（PreToolUse + deny 决策）

利用 Claude Code 插件系统的 `hooks/hooks.json` 声明 PreToolUse hook，在 agent 即将调用 Write、Edit 或 Bash 工具时拦截对 eval.json 的写入操作。由于插件无法声明 `permissions.deny` 规则（`plugin.json` 无 `permissions` 字段），钩子脚本在检测到 eval.json 写入时返回 `permissionDecision: "deny"` 达到相同拦截效果。

具体设计：

1. **`plugins/dev-team/hooks/hooks.json`** — 声明 PreToolUse hook，匹配 `Write`、`Edit`、`Bash` 工具：
   - Write/Edit 钩子：拦截对 `openspec/changes/**/eval.json` 的写入和修改
   - Bash 钩子：拦截通过 `echo`、`cat`、`tee`、重定向（`>`、`>>`）等方式写入 eval.json 的命令，但放行以 `python` 或 `python3` 开头的命令

2. **`plugins/dev-team/hooks/scripts/protect-eval.sh`** — 统一拦截脚本，根据 `tool_name` 字段区分处理：
   - Write/Edit 路径：从 `tool_input.file_path` 提取路径，后缀匹配 `openspec/changes/**/eval.json` 时返回 deny
   - Bash 路径：从 `tool_input.command` 提取命令，检测写入模式（`>`、`>>`、`tee`、heredoc），`python`/`python3` 开头则放行
   - 返回 deny 时附带中文拒绝原因，推荐 `mcp__plugin_dev-team_dev-team__phase_log` 作为替代

3. **`plugins/dev-team/.claude-plugin/plugin.json`** — 版本升级（patch bump），标记此变更

插件级方案的独特优势：
- **自动传播**：启用 dev-team 插件的所有项目自动获得保护，无需逐项目配置
- **统一维护**：保护逻辑随插件版本更新，修复或优化时只需更新插件
- **与架构一致**：CLAUDE.md 明确 `Plugin provides hooks for report-driven workflow gates`，此变更直接对齐

---

## 能力

### 新增能力

- **eval-json-protection** — 通过插件 PreToolUse hook 保护 eval.json 不被 agent 或 subagent 直接写入，所有写入必须通过 MCP `phase_log` 工具

---

## 变更范围

### 实现以下特性

- `plugins/dev-team/hooks/hooks.json` — 新增 hook 声明文件，注册 PreToolUse hook：
  - Write/Edit 工具匹配 → `protect-eval.sh`
  - Bash 工具匹配 → `protect-eval.sh`（同一脚本，内部根据 `tool_name` 分流）
- `plugins/dev-team/hooks/scripts/protect-eval.sh` — 统一拦截脚本：
  - Write/Edit 路径：检查 `tool_input.file_path` 后缀匹配 `openspec/changes/**/eval.json`
  - Bash 路径：检查 `tool_input.command` 是否包含 eval.json 写入模式（`>`、`>>`、`tee`、heredoc）
  - `python`/`python3` 开头的命令放行
  - 匹配时返回 `permissionDecision: "deny"` 并输出中文拒绝原因和替代工具建议
- `plugins/dev-team/.claude-plugin/plugin.json` — 版本号从 `2.5.9` 升至 `2.5.10`

### 不要修改

- `mcp__plugin_dev-team_dev-team__phase_log` 工具的行为——hook 仅拦截内置工具（Write、Edit、Bash），不干扰 MCP 工具
- `eval.json` 的数据结构或 schema——不改变文件格式和字段定义
- `phase_check` 或 `phase_next` MCP 工具——不涉及门控和阶段决策逻辑
- 项目 `.claude/settings.json` 或 `.claude/settings.local.json`——保护由插件自动提供，无需项目级配置
- 其他非 eval.json 的文件保护——本变更仅针对 eval.json 的写入保护
- 现有 `plugins/dev-team/` 中的 skills、agents、utils 等目录——不触及现有功能代码

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | Write 工具写入 `openspec/changes/<name>/eval.json` 被 hook 拦截，写入失败 | 在 Claude Code 中向 agent 发出 "Write openspec/changes/test/eval.json 内容为 []" 的指令，确认操作被拒绝 | P0 |
| AC-2 | Edit 工具修改 `openspec/changes/<name>/eval.json` 被 hook 拦截，修改失败 | 向 agent 发出 "Edit openspec/changes/test/eval.json 追加一条记录" 的指令，确认操作被拒绝 | P0 |
| AC-3 | Bash 重定向（`echo '[]' > openspec/changes/test/eval.json`）被拦截 | 构造 Bash 命令 `echo '[]' > openspec/changes/test/eval.json`，确认操作被拒绝 | P0 |
| AC-4 | Bash heredoc 写入 eval.json（`cat > eval.json <<EOF`）被拦截 | 构造 Bash heredoc 写入 eval.json 的命令，确认操作被拒绝 | P1 |
| AC-5 | Python 脚本写入 eval.json 不被拦截（如 `python script.py --change test --phase 01`） | 执行 `python script.py` 其中脚本内部写入 eval.json，确认操作被允许 | P0 |
| AC-6 | hook 在拦截时输出友好的拒绝原因和替代工具建议（提及 `phase_log` 或 `mcp__plugin_dev-team_dev-team__phase_log`） | 执行 AC-1 到 AC-4 任一操作，hook 输出的拒绝信息中包含 "phase_log" 字样和替代建议 | P0 |
| AC-7 | `phase_log` MCP 工具写入 eval.json 不受 hook 影响 | 调用 `mcp__plugin_dev-team_dev-team__phase_log` 写入一条评估记录，确认 eval.json 被正常追加且内容完整 | P0 |
| AC-8 | `plugins/dev-team/hooks/hooks.json` 存在且格式正确，PreToolUse hook 声明有效 | 读取 `plugins/dev-team/hooks/hooks.json`，确认 JSON 格式正确且包含 Write、Edit、Bash 的 PreToolUse hook 配置 | P0 |
| AC-9 | hook 脚本可执行且语法正确 | 使用 `bash -n` 验证 `protect-eval.sh` 无语法错误 | P1 |
| AC-10 | 无 eval.json 操作的其他 Write/Edit/Bash 操作不受影响（不产生误拦截） | 执行 `Write test.txt`、`Edit unrelated-file.ts`、`Bash(ls)` 等操作，确认全部正常通过 | P1 |
| AC-11 | `plugins/dev-team/.claude-plugin/plugin.json` 版本号已递增 | 读取 plugin.json，确认 `version` 字段值高于 `2.5.9` | P1 |
| AC-12 | 在新项目中安装 dev-team 插件后，eval.json 保护自动生效 | 在其他项目（或临时测试目录）中启用 dev-team 插件，执行 AC-1 所述操作，确认仍被拦截 | P1 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| hook 脚本误拦截合法的 eval.json 写入（如归档流程中合规脚本写入） | 工作流中断，归档失败 | 低 | 归档流程应使用 `phase_log` MCP 工具而非直接文件写入；Python 脚本已在 Bash hook 中放行 |
| hook 脚本的 Bash 命令解析存在遗漏或误判 | 该拦截的命令未拦截，或不该拦截的被拦截 | 中 | 精细的正则匹配 + 严格的测试覆盖；AC-10 验证无 eval.json 操作不受影响 |
| 用户卸载或禁用插件导致保护失效 | eval.json 恢复无保护状态 | 低 | 这是预期行为——保护随插件存在而存在；用户在禁用插件时应知悉后果 |
| PreToolUse hook 失败导致 agent 执行延迟 | 用户体验下降 | 低 | hook 设计为快速执行（仅路径/命令匹配），无外部依赖；hook 失败时不影响原操作 |
| 插件更新后 hook 脚本路径变更导致 hooks.json 引用失效 | 保护失效 | 低 | hooks.json 和脚本文件在同一插件内维护，版本更新时同步调整，通过 CI 验证 |
