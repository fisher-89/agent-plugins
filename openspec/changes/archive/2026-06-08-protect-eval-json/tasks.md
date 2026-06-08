# 任务: protect-eval-json

> **变更**: protect-eval-json
> **日期**: 2026-06-08

---

## 阶段 1: 创建 hooks 目录结构

- [x] 创建 `plugins/dev-team/hooks/` 目录
- [x] 创建 `plugins/dev-team/hooks/scripts/` 目录

## 阶段 2: 实现 Hook 声明文件

- [x] 创建 `plugins/dev-team/hooks/hooks.json`，包含顶层 `description` 字段（提及 "eval.json" 和 "phase_log"）
- [x] 在 `hooks.json` 中声明第一项 PreToolUse hook，`matcher` 设为 `"Write|Edit"`，引用 `protect-eval.sh`
- [x] 在 `hooks.json` 中声明第二项 PreToolUse hook，`matcher` 设为 `"Bash"`，引用 `protect-eval.sh`
- [x] 确保 `hooks.json` JSON 格式正确（通过 `JSON.parse()` 验证）

## 阶段 3: 实现拦截脚本 (protect-eval.sh)

- [x] 创建 `plugins/dev-team/hooks/scripts/protect-eval.sh`，设置 shebang 为 `#!/usr/bin/env bash`
- [x] 实现从 stdin 读取完整 JSON 并提取 `tool_name`、`tool_input.file_path`、`tool_input.command` 的逻辑
- [x] 实现 Write/Edit 路径检测逻辑：提取 `file_path`，检查是否以 `openspec/changes/**/eval.json` 结尾（支持相对路径和绝对路径）
- [x] 实现 Bash 命令检测逻辑：检查是否包含写入 eval.json 的模式（`>`、`>>`、`tee`、heredoc）
- [x] 实现 Bash 命令豁免逻辑：以 `python`、`python3` 或 `node` 开头的命令放行
- [x] 实现 Bash 只读操作放行逻辑：仅读取 eval.json（如 `cat eval.json`）不拦截
- [x] 实现命中时的 deny 输出格式：`hookSpecificOutput.hookEventName`、`permissionDecision: "deny"`、`permissionDecisionReason`（中文，包含 `phase_log` 工具名和推断的变更名称）
- [x] 实现未命中时的 allow 输出
- [x] 实现异常处理：`tool_input` 缺失或字段为空时默认输出 `allow`
- [x] 设置脚本的可执行权限（Unix-like: `chmod +x`）

## 阶段 4: 验证脚本正确性

- [x] 使用 `bash -n` 验证 `protect-eval.sh` 无语法错误
- [x] 构造 Write 写入 eval.json 的 stdin 输入，模拟验证被拒绝
- [x] 构造 Edit 修改 eval.json 的 stdin 输入，模拟验证被拒绝
- [x] 构造 Write 写入非 eval.json 文件的 stdin 输入，模拟验证被放行
- [x] 构造 Write 绝对路径指向 eval.json 的 stdin 输入，模拟验证被拒绝
- [x] 构造 Bash echo 重定向到 eval.json 的 stdin 输入，模拟验证被拒绝
- [x] 构造 Bash heredoc 写入 eval.json 的 stdin 输入，模拟验证被拒绝
- [x] 构造 Bash tee 写入 eval.json 的 stdin 输入，模拟验证被拒绝
- [x] 构造 Bash Python 命令写入 eval.json 的 stdin 输入，模拟验证被放行
- [x] 构造 Bash 只读 eval.json 命令的 stdin 输入，模拟验证被放行
- [x] 构造 Bash 无关命令的 stdin 输入，模拟验证被放行
- [x] 构造缺少字段的异常 stdin 输入，验证默认放行
- [x] 验证拒绝原因中包含 "phase_log" 或 "mcp__plugin_dev-team_dev-team__phase_log"
- [x] 验证拒绝原因中包含从路径推断的变更名称

## 阶段 5: 更新插件版本

- [x] 更新 `plugins/dev-team/.claude-plugin/plugin.json`，将 `version` 从 `"2.5.9"` 升至 `"2.5.10"`

## 阶段 6: 端到端验证验收条件

- [x] 验证 AC-1: Write 工具写入 `openspec/changes/<name>/eval.json` 被拦截
- [x] 验证 AC-2: Edit 工具修改 `openspec/changes/<name>/eval.json` 被拦截
- [x] 验证 AC-3: Bash 重定向写入 eval.json 被拦截
- [x] 验证 AC-4: Bash heredoc 写入 eval.json 被拦截
- [x] 验证 AC-5: Python 脚本写入 eval.json 不被拦截
- [x] 验证 AC-6: 拦截时输出友好的拒绝原因和 `phase_log` 替代建议
- [x] 验证 AC-7: `phase_log` MCP 工具写入 eval.json 不受 hook 影响（设计保障：MCP 工具不经过 PreToolUse hook）
- [x] 验证 AC-8: `hooks.json` 格式正确且 PreToolUse hook 声明有效
- [x] 验证 AC-9: hook 脚本语法正确（`bash -n` 通过）
- [x] 验证 AC-10: 无 eval.json 操作的其他 Write/Edit/Bash 操作不受影响
- [x] 验证 AC-11: `plugin.json` 版本号已递增（`2.5.10`）
- [x] 验证 AC-12: 新项目安装 dev-team 插件后 eval.json 保护自动生效（设计保障：插件级 hook 声明自动注册于 Claude Code 运行时）
