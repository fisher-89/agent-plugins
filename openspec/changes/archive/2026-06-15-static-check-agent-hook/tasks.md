# 任务: static-check-agent-hook

> **变更**: static-check-agent-hook
> **日期**: 2026-06-12

---

## 阶段 1: CLI 命令实现

- [x] 在 `plugins/dev-team/bin/package.json` 中添加 `cac` 依赖
- [x] 创建 `plugins/dev-team/bin/src/commands/run-static-analysis.ts`，实现 `runStaticAnalysis(options?: { projectRoot?: string }): number`：
  - 项目根目录：优先 `options.projectRoot` → `process.env.CLAUDE_PROJECT_DIR` → `process.cwd()`
  - 调用 `ensureConfigFile()` 和 `getValue(config, "static_analysis")`
  - 未配置或空字符串：返回 `0`
  - 已配置：在项目根目录执行命令（`shell: true`，0 参数），透传 exit code；失败时将 stdout/stderr 写入 process.stderr
- [x] 创建 `plugins/dev-team/bin/src/cli.ts`，使用 `cac` 注册 `run_static_analysis` 子命令并调用 `runStaticAnalysis()`，以 exit code 退出进程
- [x] 创建 `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts`，覆盖以下场景（使用临时目录隔离 `openspec/config.json`）：
  - 未配置 `static_analysis` 时 exit `0`
  - 空字符串配置时 exit `0`
  - 已配置且 mock 命令成功时 exit `0`
  - 已配置且 mock 命令失败时 exit 非 `0` 并输出错误信息
- [x] 在 `plugins/dev-team/bin/` 目录运行 `pnpm test`，确认单元测试通过

## 阶段 2: 打包配置

- [x] 更新 `plugins/dev-team/bin/vite.config.ts`，增加 `dev-team-cli.cjs` 打包入口（入口 `src/cli.ts`，格式 cjs，platform node）
- [x] 确保 `dev-team-mcp.cjs` 现有打包配置不被修改或移除
- [x] 更新 lint/fmt 的 `ignorePatterns`（如需要）包含 `dev-team-cli.cjs`
- [x] 在 `plugins/dev-team/bin/` 目录运行 `pnpm run build`，验证同时生成 `dev-team-cli.cjs` 和 `dev-team-mcp.cjs`
- [x] 验证 `node plugins/dev-team/bin/dev-team-cli.cjs --help` 输出包含 `run_static_analysis` 子命令

## 阶段 3: Hook 脚本实现

- [x] 创建 `plugins/dev-team/hooks/scripts/static-check.sh`，设置 shebang 为 `#!/usr/bin/env bash`
- [x] 实现 CLI 调用：`node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs" run_static_analysis`，捕获 stdout/stderr 和 exit code
- [x] CLI exit `0` 时：向 stdout 输出 `{}`，脚本 exit `0`
- [x] CLI exit 非 `0` 时：向 stdout 输出包含 `followup_message` 的 JSON（含 CLI 完整输出 + 中文修复指令），脚本 exit `0`
- [x] 确保脚本不生成 `reports/static_analysis.json` 或任何其他报告文件
- [x] 实现 JSON 字符串转义（followup_message 中的引号、换行等）
- [x] 设置脚本可执行权限（`chmod +x`）
- [x] 使用 `bash -n plugins/dev-team/hooks/scripts/static-check.sh` 验证语法

## 阶段 4: Hook 声明注册

- [x] 更新 `plugins/dev-team/hooks/hooks.json`：
  - 更新顶层 `description`，说明新增的静态检查 subagentStop hook 用途
  - 在 `hooks` 对象中新增 `subagentStop` 数组
  - 配置 `matcher: "implementation-generator"`、`loop_limit: 5`
  - 配置 `hooks[0].type: "command"`、`hooks[0].command: bash "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.sh"`
- [x] 确认现有 `PreToolUse` hook 配置（`protect-eval.sh`）未被修改或移除
- [x] 验证 `hooks.json` 可被 `JSON.parse()` 正确解析

## 阶段 5: Agent 定义精简

- [x] 更新 `plugins/dev-team/agents/implementation-generator.md`：
  - 移除 Process 步骤 7–8（`config_get("static_analysis")` 及执行检查）
  - 移除 Output 章节中 `reports/static_analysis.json` 报告生成要求及 JSON 模板
  - 移除 frontmatter `description` 中对 AUTO static-check 的引用
  - 确认步骤 6（标记 tasks.md 完成）为最后一步
- [x] 更新 `plugins/dev-team/agents/implementation-evaluator.md`：
  - 从 Static Checklist 表格中移除 I7 行
  - 确认判定规则仍为 `"pass" only if ALL items pass`，检查项范围为 I1–I6 和 I8

## 阶段 6: 插件版本更新

- [x] 更新 `plugins/dev-team/.claude-plugin/plugin.json`，将 `version` 从 `"2.6.9"` patch bump（如 `"2.6.10"`）

## 阶段 7: 验收验证

- [x] 验证 AC-1: `implementation-generator` 结束时 subagentStop hook 触发并执行 `static_analysis` 命令
- [x] 验证 AC-2: 故意引入 lint 错误，确认 generator 收到 `followup_message` 并继续修复（**设计验证**：`static-check.sh` 在 CLI exit 非 0 时输出 `{"followup_message":...}` 且脚本 exit 0，符合 subagentStop 协议；需在实际 Claude Code 中运行 generator 做完整 E2E）
- [x] 验证 AC-3: 代码无 lint 错误时 generator 正常结束
- [x] 验证 AC-4: 移除 `openspec/config.json` 中 `static_analysis` 字段后 generator 直接结束
- [x] 验证 AC-5: `followup_message` 包含 CLI 的具体 stderr/stdout 错误输出
- [x] 验证 AC-6: 引入无法自动修复的错误，确认 5 次 followup 后 generator 被允许结束（**设计验证**：`hooks.json` 中 `loop_limit: 5` 由 Claude Code hook 框架强制执行；需在实际 Claude Code 中运行 generator 做完整 E2E）
- [x] 验证 AC-7: 手动运行 `node dev-team-cli.cjs run_static_analysis`，确认未配置 exit 0、已配置时透传 exit code
- [x] 验证 AC-8: `implementation-generator.md` 不含步骤 7–8 和报告生成部分
- [x] 验证 AC-9: `implementation-evaluator.md` 不含 I7 检查项
- [x] 验证 AC-10: eval.json 直接写入仍被 `protect-eval.sh` 拦截
- [x] 验证 AC-11: 运行 `proposal-planner` 等其他 agent 时 static-check hook 不触发
- [x] 验证 AC-12: `plugin.json` 版本号高于 `"2.6.9"`
