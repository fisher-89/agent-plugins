# 测试设计: static-check-agent-hook

> **日期**: 2026-06-12

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `implementation-generator` 结束时自动触发静态检查 | 集成测试 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- subagentStop 触发时调用 CLI |
| AC-2 | 静态检查失败时 generator 不结束，收到 `decision / reason` 后继续修复 | 集成测试 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- CLI 非零 exit 返回 decision / reason |
| AC-3 | 静态检查通过时 generator 正常结束 | 单元测试 | `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 已配置且命令成功时 exit 0 |
| AC-3 | 静态检查通过时 generator 正常结束 | 集成测试 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- CLI exit 0 时 stdout 输出 `{}` |
| AC-4 | 未配置 `static_analysis` 时 generator 直接结束，hook 不阻塞 | 单元测试 | `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 未配置 static_analysis 时 exit 0 |
| AC-4 | 未配置 `static_analysis` 时 generator 直接结束，hook 不阻塞 | 集成测试 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- 未配置时 CLI 放行并输出 `{}` |
| AC-5 | `decision / reason` 包含具体的错误输出内容 | 集成测试 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- decision / reason 含 CLI stderr/stdout |
| AC-6 | 重试次数不超过 `loop_limit`（5 次） | — | — | 见不可测试项 |
| AC-7 | `dev-team-cli.cjs run_static_analysis` 正确读取并执行配置 | 单元测试 | `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 配置读取与命令执行 exit code 传播 |
| AC-7 | `dev-team-cli.cjs run_static_analysis` 正确读取并执行配置 | 集成测试 | `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` | `dev-team-cli.cjs` -- 打包产物可执行且子命令行为正确 |
| AC-8 | `implementation-generator.md` 不再包含静态检查步骤 | 集成测试 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-generator.md` -- Process/Output/frontmatter 静态检查 |
| AC-9 | `implementation-evaluator.md` 不再包含 I7 检查项 | 集成测试 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-evaluator.md` -- Static Checklist 不含 I7 |
| AC-10 | 现有 `protect-eval.sh` hook 不受影响 | 集成测试 | `__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.sh` -- PreToolUse 拦截行为回归 |
| AC-11 | 其他 subagent 类型的 stop 事件不受此 hook 影响 | 集成测试 | `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json` -- subagentStop matcher 仅匹配 implementation-generator |
| AC-12 | `plugin.json` 版本号已递增 | 集成测试 | `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `plugin.json` -- version 高于 2.6.9 |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 未配置放行 | 正向 | `openspec/config.json` 不存在时 exit 0，不执行外部命令 (AC-4, AC-7) | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 未配置放行 | 正向 | config 存在但无 `static_analysis` 字段时 exit 0 (AC-4) | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 空配置放行 | 边界 | `"static_analysis": ""` 时 exit 0，不执行外部命令 | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 空白字符串放行 | 边界 | `"static_analysis": "   "` 时 exit 0（仅空白视为空） | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 命令成功 | 正向 | 已配置且 mock 命令 exit 0 时 CLI exit 0 (AC-3, AC-7) | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 命令失败 | 异常 | 已配置且 mock 命令 exit 非 0 时 CLI exit 相同非零 code (AC-7) | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 错误输出 | 异常 | 命令失败时 CLI stderr 含命令 stdout/stderr 合并输出 (AC-5) | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- exit code 传播 | 边界 | mock 命令 exit 1、2、127 时 CLI 返回相同 exit code | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 项目根目录 | 正向 | 优先使用 `CLAUDE_PROJECT_DIR` 环境变量定位 config (AC-7) | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 项目根目录 | 边界 | 未设置 `CLAUDE_PROJECT_DIR` 时使用 `process.cwd()` | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- config 容错 | 边界 | `openspec/config.json` JSON 解析失败时等同未配置，exit 0 | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 工作目录 | 正向 | 已配置时在项目根目录执行命令（`cwd` 为 projectRoot） | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 无额外参数 | 正向 | 执行 `static_analysis` 命令时不传入 change name 或其他 CLI 参数 | 新增 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `runStaticAnalysis` -- 成功无输出 | 正向 | 命令 exit 0 时 CLI stdout 无多余输出 | 新增 |
| `plugins/dev-team/bin/vite.config.test.ts` | `vite.config.ts` -- 双打包入口 | 正向 | `pack.entry` 含 `src/mcp.ts` 与 `src/cli.ts`，分别输出 `dev-team-mcp.cjs` 与 `dev-team-cli.cjs` (AC-7) | 新增 |
| `plugins/dev-team/bin/vite.config.test.ts` | `vite.config.ts` -- MCP 配置不变 | 正向 | 现有 `dev-team-mcp.cjs` 打包选项（platform/format/minify）未被修改 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | 文件系统 | `fs.mkdtempSync` 创建临时项目，写入 `openspec/config.json` fixture，测试后 `fs.rmSync` 清理；不 mock `fs` 读写 config | 所有 config 读取场景 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | `child_process` | `vi.spyOn(child_process, 'spawnSync')` 或注入可替换的 `execCommand` 依赖，返回可控 exit code 与 stdout/stderr | 已配置命令执行、失败输出、exit code 传播 |
| `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` | 环境变量 | `vi.stubEnv('CLAUDE_PROJECT_DIR', tmpRoot)` / 删除后恢复 | 项目根目录优先级 |
| `plugins/dev-team/bin/vite.config.test.ts` | 无 | 直接 import `vite.config.ts` default export 断言 `pack` 配置结构 | 打包入口验证 |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- CLI 调用 | 设置 `CLAUDE_PLUGIN_ROOT` 后执行脚本，断言调用 `node .../dev-team-cli.cjs run_static_analysis` | 新增 |
| AC-2 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- followup 输出 | mock CLI exit 非 0 时 stdout 为合法 JSON 且含 `decision / reason` 字段 | 新增 |
| AC-2 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- 脚本 exit 0 | 检查失败时脚本自身 exit 0（followup 通过 JSON 传递，非脚本 exit code） | 新增 |
| AC-3 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- 放行输出 | mock CLI exit 0 时 stdout 精确为 `{}` | 新增 |
| AC-4 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- 未配置放行 | 临时项目无 `static_analysis` 配置时输出 `{}` | 新增 |
| AC-5 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- followup 内容 | `decision / reason` 含 CLI 错误输出及中文修复指令前缀 | 新增 |
| AC-5 | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- JSON 转义 | followup 含换行、引号等特殊字符时 stdout JSON 仍可 `JSON.parse` | 新增 |
| — | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- 语法检查 | `bash -n plugins/dev-team/hooks/scripts/static-check.sh` exit 0 | 新增 |
| — | `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `static-check.sh` -- 不生成报告 | 执行后不存在 `reports/static_analysis.json` | 新增 |
| AC-7 | `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` | `dev-team-cli.cjs` -- help | `node dev-team-cli.cjs --help` 输出含 `run_static_analysis` 子命令 | 新增 |
| AC-7 | `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` | `dev-team-cli.cjs` -- 打包产物 | `pnpm run build` 后 `dev-team-cli.cjs` 文件存在且可执行 | 新增 |
| AC-7 | `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` | `dev-team-cli.cjs run_static_analysis` -- 端到端 | 临时项目配置 `static_analysis: "node -e process.exit(0)"` 时 CLI exit 0 | 新增 |
| AC-7 | `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` | `dev-team-cli.cjs run_static_analysis` -- 端到端失败 | 配置 `static_analysis: "node -e process.exit(1)"` 时 CLI exit 1 且 stderr 非空 | 新增 |
| AC-7 | `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` | `dev-team-mcp.cjs` -- 回归 | build 后 `dev-team-mcp.cjs` 仍正常生成（双产物互不干扰） | 新增 |
| AC-8 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-generator.md` -- Process | Process 不含 `config_get`、`static_analysis`、步骤 7/8 | 新增 |
| AC-8 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-generator.md` -- Output | Output 不含 `reports/static_analysis.json` 及 JSON 报告模板 | 新增 |
| AC-8 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-generator.md` -- frontmatter | description 不含 `static-check` / `static_analysis` 引用 | 新增 |
| AC-8 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-generator.md` -- 最后步骤 | Process 最后一步为标记 tasks.md 完成（原步骤 6） | 新增 |
| AC-9 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-evaluator.md` -- I7 移除 | Static Checklist 表格无 ID 为 `I7` 的行 | 新增 |
| AC-9 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-evaluator.md` -- 关键字 | 全文不含 `static_analysis`、`reports/static_analysis.json` | 新增 |
| AC-9 | `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | `implementation-evaluator.md` -- 判定范围 | 判定规则仍为 `"pass" only if ALL items pass`，检查项为 I1–I6 与 I8 | 新增 |
| AC-10 | `__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.sh` -- Write 拦截 | Write `openspec/changes/test/eval.json` 返回 `permissionDecision: "deny"` | 新增 |
| AC-10 | `__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.sh` -- Edit 拦截 | Edit eval.json 路径仍被拦截 | 新增 |
| AC-10 | `__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.sh` -- Bash 重定向 | `echo '[]' > openspec/changes/test/eval.json` 仍被拦截 | 新增 |
| AC-10 | `__tests__/protect-eval-regression/protect-eval-regression.test.ts` | `protect-eval.sh` -- 无关文件放行 | Write 非 eval.json 文件返回 `allow` | 新增 |
| AC-11 | `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json` -- subagentStop 声明 | JSON 解析成功，`hooks.subagentStop` 数组至少一项 | 新增 |
| AC-11 | `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json` -- matcher | subagentStop 项 `matcher` 为 `"implementation-generator"` | 新增 |
| AC-11 | `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json` -- loop_limit | subagentStop 项 `loop_limit` 为 `5` | 新增 |
| AC-11 | `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json` -- command | hook command 指向 `static-check.sh` | 新增 |
| AC-10 | `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `hooks.json` -- PreToolUse 不变 | 现有 `PreToolUse` 两条 hook（Write\|Edit、Bash → protect-eval.sh）未被修改或移除 | 新增 |
| AC-12 | `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | `plugin.json` -- 版本号 | `version` 字段值大于 `"2.6.9"` 且符合 semver | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | `dev-team-cli.cjs` | 在临时目录放置 stub CLI 脚本（根据参数 exit 0/1 并输出固定 stderr），通过 `CLAUDE_PLUGIN_ROOT` 指向含 stub 的插件根 | CLI exit 0/非 0 分支、followup 内容 |
| `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` | bash 子进程 | `child_process.execFileSync('bash', [scriptPath])` 从 stdin 注入 subagentStop JSON（可忽略），收集 stdout | 端到端 hook 脚本黑盒测试 |
| `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` | 无（真实 build） | 测试前执行 `pnpm run build`，直接 `child_process.spawnSync('node', [cliPath, ...])` | 打包产物与子命令端到端 |
| `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` | 文件系统 | 临时项目目录 + `openspec/config.json` fixture | CLI 在不同 config 下的行为 |
| `__tests__/protect-eval-regression/protect-eval-regression.test.ts` | bash 子进程 | 与 protect-eval-json 变更相同模式：`execFileSync('bash', [protect-eval.sh])` + stdin JSON | PreToolUse 回归，不 mock 脚本内部 |
| `__tests__/hooks-json-structure/hooks-json-structure.test.ts` | 无 | `fs.readFileSync` + `JSON.parse` / semver 比较 | hooks.json、plugin.json 结构断言 |
| `__tests__/agent-definitions-static/agent-definitions-static.test.ts` | 无 | `fs.readFileSync` + 正则/字符串匹配 agent markdown 内容 | AC-8、AC-9 静态定义验证 |

---

## 不可测试项

- **AC-6（loop_limit 耗尽后允许结束）** — **原因**: `loop_limit: 5` 由 Claude Code hook 框架在运行时计数与降级，无法在单元/集成测试中模拟完整 subagent 重试循环；仅可在 `hooks.json` 中断言 `loop_limit: 5` 配置存在（AC-11 覆盖），端到端行为需手动验证。
- **AC-1 / AC-2 / AC-3（真实 generator subagent 生命周期）** — **原因**: 完整「运行 implementation-generator → subagentStop 触发 → agent 收到 followup 继续修复」流程依赖 Claude Code 运行时与 LLM agent，自动化测试仅覆盖 `static-check.sh` + CLI 子进程行为；真实 agent 循环需手动 E2E 验证。
- **AC-11（其他 subagent 运行时 hook 不触发）** — **原因**: hook 框架按 `matcher` 过滤 subagent 类型，测试环境无法启动其他 subagent；可通过 `hooks.json` 静态断言 matcher 值，运行时行为需手动验证。
- **`plugins/dev-team/bin/src/cli.ts`** — **原因**: `test_resolve_paths` 返回 `Path does not exist`（实现尚未创建）；CLI 注册与 `--help` 行为由 `__tests__/cli-run-static-analysis/cli-run-static-analysis.test.ts` 对打包产物进行集成覆盖。
- **`plugins/dev-team/hooks/scripts/static-check.sh`** — **原因**: `test_resolve_paths` 返回 `Path does not exist`（实现尚未创建）；行为由 `__tests__/static-check-hook-e2e/static-check-hook-e2e.test.ts` 集成测试覆盖。
- **`plugins/dev-team/bin/src/lib/config.ts`** — **原因**: `test_resolve_paths` 推导路径为 `plugins/dev-team/bin/src/lib/config.test.ts`，但本变更复用已有 config 库且不修改其逻辑；`static_analysis` 读取边界由 `run-static-analysis.test.ts` 间接覆盖，无需新增独立 config 单元测试文件。
- **`plugins/dev-team/.claude-plugin/plugin.json`** — **原因**: `test_resolve_paths` 返回 `Not a testable source file`；版本号断言由 `__tests__/hooks-json-structure/hooks-json-structure.test.ts` 集成测试覆盖。
