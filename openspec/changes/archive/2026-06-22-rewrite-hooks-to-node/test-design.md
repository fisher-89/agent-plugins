# 测试设计: rewrite-hooks-to-node

> **日期**: 2026-06-18

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `hooks.json` 中三处 hook command 均使用 `node` 调用 `.mjs` 脚本 | 集成测试 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json — PreToolUse / SubagentStop command 指向 .mjs` |
| AC-2 | Windows PowerShell 环境下 hook 可被 Cursor 正常调用 | — | — | 见不可测试项 |
| AC-3 | eval.json Write/Edit 拦截行为与迁移前一致 | 集成测试 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — Write / Edit 拦截` |
| AC-3 | eval.json Write/Edit 拦截行为与迁移前一致 | 单元测试 | `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `isEvalJsonPath — 路径匹配` |
| AC-4 | eval.json Bash 重定向/tee/heredoc 拦截与 python/node 豁免与迁移前一致 | 集成测试 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — Bash 写入检测与豁免` |
| AC-4 | eval.json Bash 重定向/tee/heredoc 拦截与 python/node 豁免与迁移前一致 | 单元测试 | `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — 写入模式与豁免边界` |
| AC-5 | 静态检查通过时 hook 输出 `{}` | 集成测试 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — CLI exit 0 时 stdout 为 {}` |
| AC-5 | 静态检查通过时 hook 输出 `{}` | 单元测试 | `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `formatOutput — CLI exit 0 返回 {}` |
| AC-6 | 静态检查失败时 hook 输出含 `followup_message` 的 JSON | 集成测试 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — followup 输出与 JSON 转义` |
| AC-6 | 静态检查失败时 hook 输出含 `followup_message` 的 JSON | 单元测试 | `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `buildFollowupMessage / formatOutput — 前缀与转义` |
| AC-7 | 旧 `.sh` 文件已删除，仓库中无残留引用 | 集成测试 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json — command 不含 .sh` |
| AC-7 | 旧 `.sh` 文件已删除，仓库中无残留引用 | 集成测试 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — 脚本存在性（.sh 不存在）` |
| AC-7 | 旧 `.sh` 文件已删除，仓库中无残留引用 | 集成测试 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 脚本存在性（.sh 不存在）` |
| AC-8 | `plugin.json` 版本号已递增 | 集成测试 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `plugin.json — version 大于 "2.6.22"` |
| AC-9 | 空 stdin 或缺少字段时 protect-eval 默认放行（fail-open） | 集成测试 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — fail-open 回归` |
| AC-9 | 空 stdin 或缺少字段时 protect-eval 默认放行（fail-open） | 单元测试 | `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `parseInput / main — 空 stdin、无效 JSON、缺失字段` |
| AC-10 | CLI 不存在时 static-check 返回 followup_message 而非崩溃 | 集成测试 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — CLI 缺失时 followup_message` |
| AC-10 | CLI 不存在时 static-check 返回 followup_message 而非崩溃 | 单元测试 | `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `handleMissingCli — CLI 路径不存在分支` |
| AC-3 | eval.json Write/Edit 拦截行为与迁移前一致 | 单元测试 | `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `extractChangeName — 正常变更名 / 无匹配` |
| AC-3 | eval.json Write/Edit 拦截行为与迁移前一致 | 单元测试 | `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `buildDenyReason — 拒绝原因格式` |
| AC-3 | eval.json Write/Edit 拦截行为与迁移前一致 | 单元测试 | `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `outputDeny — JSON 特殊字符` |
| AC-3 | eval.json Write/Edit 拦截行为与迁移前一致 | 集成测试 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — 无关文件放行` |
| AC-1 | `hooks.json` 中三处 hook command 均使用 `node` 调用 `.mjs` 脚本 | 集成测试 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json — PreToolUse 结构不变` |
| AC-6 | 静态检查失败时 hook 输出含 `followup_message` 的 JSON | 单元测试 | `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `buildFollowupMessage — CLI 输出合并` |
| AC-5, AC-6 | 静态检查通过/失败时 CLI 路径正确解析 | 单元测试 | `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `resolveCliPath — 正常路径 / 含空格根目录` |
| AC-9 | 空 stdin 或缺少字段时 protect-eval 默认放行（fail-open） | 单元测试 | `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `parseInput — 未知工具` |
| AC-5 | 静态检查通过时 hook 输出 `{}` | 集成测试 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 未配置放行` |
| AC-6 | 静态检查失败时 hook 输出含 `followup_message` 的 JSON | 集成测试 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 脚本 exit 0` |
| AC-7 | 旧 `.sh` 文件已删除，仓库中无残留引用 | 集成测试 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 脚本质量` |

---

## 单元测试

覆盖可在进程内验证的纯函数与输出格式化逻辑。实现时 SHOULD 从 `.mjs` 导出 `isEvalJsonPath`、`detectBashWrite`、`extractChangeName`、`buildDenyReason`、`buildFollowupMessage`、`formatOutput`、`resolveCliPath` 等函数供 `node:test` import；若保持单文件结构，则通过 `child_process` 黑盒调用脚本（与集成测试分层：单元测试聚焦边界输入，集成测试聚焦 AC 端到端）。

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `isEvalJsonPath — 相对路径` | 正向 | `openspec/changes/test-change/eval.json` 返回 true (AC-3) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `isEvalJsonPath — 绝对路径` | 正向 | `D:/Projects/wps-claude-plugin/openspec/changes/demo/eval.json` 返回 true (AC-3) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `isEvalJsonPath — 非 eval 文件` | 正向 | `openspec/changes/test/design.md` 返回 false (AC-3) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `isEvalJsonPath — changes 目录外` | 正向 | `plugins/dev-team/bin/src/commands/phase-log.ts` 返回 false (AC-3) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `isEvalJsonPath — 反斜杠归一化` | 边界 | `openspec\\changes\\test\\eval.json` 返回 true | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `isEvalJsonPath — 空字符串` | 边界 | `""` 返回 false | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `isEvalJsonPath — 超长路径` | 边界 | 超过 1000 字符仍含 `openspec/changes/.../eval.json` 后缀时返回 true | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — > 重定向` | 正向 | `echo '[]' > openspec/changes/test/eval.json` 返回 true (AC-4) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — >> 追加` | 正向 | `echo '[]' >> openspec/changes/test/eval.json` 返回 true (AC-4) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — tee` | 正向 | `echo '[]' \| tee openspec/changes/test/eval.json` 返回 true (AC-4) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — tee -a` | 边界 | `echo '[]' \| tee -a openspec/changes/test/eval.json` 返回 true | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — heredoc` | 正向 | `cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF` 返回 true (AC-4) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — >& 重定向` | 边界 | `echo '[]' >& openspec/changes/test/eval.json` 返回 true | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — 排除 ->` | 边界 | 含 `->` 但无 eval.json 写入时返回 false | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — python 豁免` | 正向 | `python plugins/dev-team/utils/eval-check.py --change test` 返回 false (AC-4) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — python3 豁免` | 边界 | `python3 -c "..."` 返回 false | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — node 豁免` | 正向 | `node scripts/write-eval.mjs` 返回 false (AC-4) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — 只读 cat` | 正向 | `cat openspec/changes/test/eval.json` 返回 false (AC-4) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — 无 eval.json` | 边界 | `ls -la` 返回 false | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `detectBashWrite — 空命令` | 边界 | `""` 返回 false | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `extractChangeName — 正常变更名` | 正向 | 从 `openspec/changes/my-feature/eval.json` 提取 `my-feature` | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `extractChangeName — 无匹配` | 边界 | 从 `src/utils/helper.ts` 返回空字符串 | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `buildDenyReason — 拒绝原因格式` | 正向 | 输出含 `phase_log`、`eval.json` 及变更名占位 | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `parseInput — 空 stdin` | 边界 | 空字符串触发 fail-open allow (AC-9) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `parseInput — 无效 JSON` | 异常 | `{not json` 触发 fail-open allow (AC-9) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `parseInput — 缺失 tool_name` | 边界 | `{}` 触发 allow (AC-9) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `parseInput — 缺失 tool_input.file_path` | 边界 | Write 无 `file_path` 时返回 allow (AC-9) | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `parseInput — 未知工具` | 边界 | `tool_name: "Read"` 返回 allow | 新增 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | `outputDeny — JSON 特殊字符` | 边界 | reason 含换行、引号、反斜杠时 stdout 可 `JSON.parse` | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `buildFollowupMessage — 前缀` | 正向 | 含「静态检查未通过，请修复以下错误后重新提交：」中文前缀 (AC-6) | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `buildFollowupMessage — CLI 输出合并` | 正向 | stdout 与 stderr 合并进 followup_message | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `formatOutput — 通过` | 正向 | CLI exit 0 时返回 `{}` (AC-5) | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `formatOutput — 失败` | 正向 | CLI exit 非 0 时返回含 `followup_message` 的对象 (AC-6) | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `formatOutput — JSON 转义` | 边界 | CLI 输出含 `\n`、`"`、`\t`、`\` 时 `JSON.stringify` 可解析 (AC-6) | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `resolveCliPath — 正常路径` | 正向 | `CLAUDE_PLUGIN_ROOT` 下 `bin/dev-team-cli.cjs` 路径拼接正确 | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `resolveCliPath — 含空格根目录` | 边界 | `CLAUDE_PLUGIN_ROOT` 含空格时 `path.join` 不截断 | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `handleMissingCli — CLI 不存在` | 异常 | 文件不存在时返回 followup_message 且不含未捕获异常 (AC-10) | 新增 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `handleMissingCli — 空 CLAUDE_PLUGIN_ROOT` | 边界 | 环境变量未设置时输出含路径说明的 followup_message | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | 无（纯函数） | 直接 import 导出函数，传入字符串参数断言返回值 | `isEvalJsonPath`、`detectBashWrite`、`extractChangeName` |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | stdin / stdout | 若未导出函数：`spawnSync(process.execPath, [scriptPath], { input })` 注入 JSON | fail-open、完整 hook 输出格式 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | `child_process.spawnSync` | 注入 stub 模块或 mock 函数，返回可控 `{ status, stdout, stderr }` | CLI exit 0/非 0、输出合并 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | 文件系统 | `fs.existsSync` stub 模拟 CLI 缺失 (AC-10) | CLI 不存在分支 |
| `plugins/dev-team/hooks/scripts/static-check.test.mjs` | 环境变量 | `process.env.CLAUDE_PLUGIN_ROOT` 设为临时目录 | 路径拼接与 CLI 定位 |

---

## 集成测试

黑盒验证 hook 脚本与 `hooks.json` / `plugin.json` 声明；测试 helper 从 `resolveBash()` + `.sh` 迁移为 `execFileSync('node', [scriptPath])` + `.mjs`，移除 `../helpers/resolve-bash` 依赖。

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json — PreToolUse Write\|Edit` | command 以 `node` 开头且引用 `protect-eval.mjs`，不含 `bash` 或 `.sh` | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json — PreToolUse Bash` | command 以 `node` 开头且引用 `protect-eval.mjs` | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json — SubagentStop` | command 以 `node` 开头且引用 `static-check.mjs`，`loop_limit` 仍为 `5` | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json — PreToolUse 结构不变` | 仍有两条 PreToolUse（Write\|Edit、Bash），matcher 与 type 不变 | 废弃 |
| AC-3 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — Write 拦截` | Write `openspec/changes/test-change/eval.json` 返回 `permissionDecision: "deny"` 且 reason 含 `phase_log` | 废弃 |
| AC-3 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — Edit 拦截` | Edit eval.json 路径返回 `deny` | 废弃 |
| AC-3 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — 无关文件放行` | Write `src/utils/helper.ts` 返回 `allow` | 废弃 |
| AC-4 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — Bash 重定向` | `echo '[]' > openspec/changes/test-change/eval.json` 返回 `deny` | 废弃 |
| AC-4 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — Bash tee` | `echo '[]' \| tee openspec/changes/test/eval.json` 返回 `deny` | 新增 |
| AC-4 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — Bash heredoc` | heredoc 写 eval.json 返回 `deny` | 新增 |
| AC-4 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — python 豁免` | `python ...` 命令返回 `allow` | 新增 |
| AC-4 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — node 豁免` | `node scripts/write-eval.mjs` 返回 `allow` | 新增 |
| AC-4 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — 只读 Bash` | `cat openspec/changes/test/eval.json` 返回 `allow` | 新增 |
| AC-9 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — 空 stdin` | 空 stdin 返回 `allow` | 新增 |
| AC-9 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — 无效 JSON` | 非 JSON stdin 返回 `allow` | 新增 |
| AC-9 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — 缺失 tool_input` | 无 `tool_input.file_path` 时 Write 返回 `allow` | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.mjs — 脚本可用性` | `protect-eval.mjs` 存在且非空；`protect-eval.sh` 不存在 | 新增 |
| AC-5 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — CLI 调用` | 设置 `CLAUDE_PLUGIN_ROOT` 后 stub CLI exit 0 时 stdout 为 `{}` | 废弃 |
| AC-5 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 放行输出` | CLI exit 0 时 stdout 精确为 `{}` | 废弃 |
| AC-5 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 未配置放行` | 无 `static_analysis` 配置时输出 `{}` | 废弃 |
| AC-6 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — followup 输出` | stub CLI exit 非 0 时 stdout 含合法 `followup_message` | 废弃 |
| AC-6 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 脚本 exit 0` | 检查失败时脚本自身 exit 0 | 废弃 |
| AC-6 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — followup 内容` | `followup_message` 含 CLI 错误输出及中文修复前缀 | 废弃 |
| AC-6 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — JSON 转义` | followup 含换行、引号等特殊字符时 stdout 可 `JSON.parse` | 废弃 |
| AC-10 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — CLI 缺失` | 临时 plugin root 无 `bin/dev-team-cli.cjs` 时输出含 `followup_message` 且不抛异常 | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 脚本质量` | 替换 `bash -n`：执行 `node static-check.mjs` 无未捕获异常；不生成 `reports/static_analysis.json` | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.mjs — 脚本可用性` | `static-check.mjs` 存在；`static-check.sh` 不存在 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `plugin.json — 版本号` | `version` 大于 `"2.6.22"` 且符合 semver | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `dev-team-cli.cjs` | 临时目录 stub CLI（`run_static_analysis` 参数校验 + 可控 exit code / stderr），`CLAUDE_PLUGIN_ROOT` 指向 stub 根；stub 复制 `static-check.mjs` 而非 `.sh` | CLI 调用、followup、AC-10 CLI 缺失 |
| `plugins/dev-team/bin/__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | Node 子进程 | `execFileSync('node', [scriptPath], { input, env })` 替代 `resolveBash()` | 全场景黑盒 |
| `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` | Node 子进程 | `execFileSync('node', [protect-eval.mjs], { input: stdinJson })` | PreToolUse 决策回归 |
| `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` | 无 | `fs.readFileSync` + `JSON.parse` / semver 比较 | hooks.json、plugin.json 结构 |
| `plugins/dev-team/bin/__tests__/helpers/resolve-bash.ts` | — | 本变更后无引用，可删除 helper（若全仓无其他使用者） | 移除 Windows Git Bash 依赖 |

---

## 不可测试项

- **AC-2（Windows PowerShell 下 Cursor 运行时 hook 调用）** — **原因**: Cursor Hook 框架在 IDE 内按 `hooks.json` 启动子进程，自动化测试仅覆盖 `node script.mjs` 等价命令行；无法在 CI 中模拟 Cursor 注入 `${CLAUDE_PLUGIN_ROOT}` 与 PreToolUse/subagentStop 事件时序。需在 Windows PowerShell 手动执行验收。
- **AC-7（全仓 grep 无 `.sh` 残留引用）** — **原因**: 静态 grep 属于实现阶段验证步骤（tasks Phase 4），非 Vitest 用例；集成测试覆盖 `hooks.json` 与测试 helper 路径不含 `.sh`，但文档或其他目录引用需人工/CI grep 确认。
- **`plugins/dev-team/hooks/hooks.json`** — **原因**: `test_resolve_paths` 返回 `Not a testable source file`；结构与 command 断言由 `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` 集成测试覆盖。
- **`plugins/dev-team/.claude-plugin/plugin.json`** — **原因**: `test_resolve_paths` 返回 `Not a testable source file`；版本号断言由 `plugins/dev-team/bin/__tests__/hooks-json-structure/hooks-json-structure.test.ts` 集成测试覆盖。
- **`dev-team-cli.cjs run_static_analysis` CLI 实现** — **原因**: 本变更明确不修改 CLI；CLI 行为由既有 `plugins/dev-team/bin/__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` 覆盖，hook 层仅通过 stub CLI 验证 spawn 与 exit code 传播。
- **SubagentStop `loop_limit` 耗尽后降级放行** — **原因**: 由 Claude Code hook 框架运行时计数，无法在 hook 脚本集成测试中模拟 5 次循环；`hooks.json` 中 `loop_limit: 5` 静态断言已覆盖配置存在性。
- **其他 subagent 结束时不触发 static-check hook** — **原因**: matcher 行为由 Cursor hook 框架在运行时根据 subagent 名称决定，非 hook 脚本逻辑；本变更仅迁移脚本实现语言。
