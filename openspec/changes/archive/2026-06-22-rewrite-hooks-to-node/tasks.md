# 任务: rewrite-hooks-to-node

> **变更**: rewrite-hooks-to-node
> **基于**: design.md, proposal.md

---

## Phase 1: static-check.mjs

- [x] 创建 `plugins/dev-team/hooks/scripts/static-check.mjs`，添加文件头注释说明 hook 用途、输入/输出契约
- [x] 使用 `path.join(process.env.CLAUDE_PLUGIN_ROOT, 'bin', 'dev-team-cli.cjs')` 解析 CLI 路径
- [x] 实现 CLI 文件不存在时的 followup 分支（消息含完整路径，脚本 exit 0）
- [x] 使用 `spawnSync(process.execPath, [cliPath, 'run_static_analysis'], { encoding: 'utf-8' })` 调用 CLI
- [x] 合并 stdout + stderr 作为 CLI 输出；exit 0 时输出 `{}`，非 0 时输出 `{ followup_message }`
- [x] followup_message 前缀使用「静态检查未通过，请修复以下错误后重新提交：\n\n」（与 bash 版一致）
- [x] 使用 `JSON.stringify({ followup_message })` 生成输出，确保特殊字符正确转义
- [x] 脚本在所有路径下均以 exit code 0 结束（followup 通过 JSON 字段传递，非 exit code）

## Phase 2: protect-eval.mjs

- [x] 创建 `plugins/dev-team/hooks/scripts/protect-eval.mjs`，添加文件头注释
- [x] 从 stdin 读取完整输入；空输入 → outputAllow()
- [x] 使用 `JSON.parse` 解析；解析失败 → outputAllow()（fail-open）
- [x] 实现 `outputAllow()` / `outputDeny(reason)`，输出格式与 bash 版完全一致（含缩进与字段名）
- [x] 移植 `DENY_REASON_TEMPLATE` 常量及 `extractChangeName(path)` 逻辑
- [x] 移植 `isEvalJsonPath(path)`：反斜杠归一化 + `/openspec/changes/.*eval\.json$/` 匹配
- [x] 移植 `detectBashWrite(cmd)` 全部检测模式：
  - python/python3/node 前缀豁免
  - `>`/`>>` 重定向（排除 `->`、`<<>`）
  - `tee` 管道写入
  - heredoc（`<<` 且非 `<<`）
  - `>&` 文件描述符重定向
- [x] 实现 main 分支：Write/Edit 检查 file_path；Bash 检查 command；未知 tool → allow
- [x] 缺少 `tool_name`、`file_path` 或 `command` 时 fail-open

## Phase 3: hooks.json 与清理

- [x] 更新 `plugins/dev-team/hooks/hooks.json` PreToolUse Write|Edit command 为 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-eval.mjs"`
- [x] 更新 `plugins/dev-team/hooks/hooks.json` PreToolUse Bash command 为同上
- [x] 更新 `plugins/dev-team/hooks/hooks.json` SubagentStop command 为 `node "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.mjs"`
- [x] 确认 hooks.json 中无任何 `bash` 引用及 `.sh` 路径
- [x] 删除 `plugins/dev-team/hooks/scripts/static-check.sh`
- [x] 删除 `plugins/dev-team/hooks/scripts/protect-eval.sh`

## Phase 4: 版本与引用清理

- [x] 将 `plugins/dev-team/.claude-plugin/plugin.json` 的 `version` 从 `2.6.22` bump 至 `2.6.23`
- [x] 在 `plugins/dev-team/` 下 grep 确认无 `static-check.sh` / `protect-eval.sh` 残留引用

## Phase 5: 集成测试迁移

- [x] 更新 `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts`：
  - `scriptPath` 指向 `static-check.mjs`
  - `runStaticCheckHook` 使用 `execFileSync('node', [actualScript])` 替代 `resolveBash()`
  - stub 目录复制 `.mjs` 而非 `.sh`
  - 移除 `bash -n` 语法检查，替换为 node 直接执行验证
  - 更新 describe 块注释中的脚本名
- [x] 更新 `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts`：
  - `scriptPath` 指向 `protect-eval.mjs`
  - `runProtectEval` 使用 `execFileSync('node', [scriptPath])` 替代 `resolveBash()`
  - 更新 describe 块注释中的脚本名
- [x] 更新 `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts`：
  - SubagentStop command 断言改为匹配 `static-check.mjs` 且以 `node` 开头
  - PreToolUse command 断言改为匹配 `protect-eval.mjs` 且以 `node` 开头
- [x] 若 `plugins/dev-team/bin/__tests__/helpers/resolve-bash.ts` 无其他引用，删除该文件

## Phase 6: 验收

- [x] 在 Windows PowerShell 手动执行 `node plugins/dev-team/hooks/scripts/protect-eval.mjs`（传入合法 stdin JSON），确认 exit 0 且 stdout 为合法 JSON
- [x] 在 Windows PowerShell 手动执行 `node plugins/dev-team/hooks/scripts/static-check.mjs`（设置 `CLAUDE_PLUGIN_ROOT`），确认 exit 0 且 stdout 为 `{}` 或含 `followup_message`
- [x] 运行 `plugins/dev-team/bin` 下集成测试，确认全部通过（AC-1 至 AC-10）
