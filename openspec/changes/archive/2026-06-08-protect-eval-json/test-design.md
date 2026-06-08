# 测试设计: protect-eval-json

> **变更**: protect-eval-json
> **日期**: 2026-06-08
> **基于**: proposal.md, design.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | `protect-eval.sh` 脚本的路径匹配逻辑、命令分析逻辑、JSON 输出格式化、异常输入处理；`hooks.json` 结构和语义验证；`plugin.json` 版本号验证 | Vitest (vite-plus/test) + bash 子进程调用 | 覆盖全部 12 个 AC 的原子判定逻辑，确保每种 Write/Edit/Bash 输入模式产生正确的 allow/deny 决策 |
| 集成测试 | 脚本作为子进程从 stdin 接收完整工具调用 JSON、输出 stdout JSON 的端到端流程；拒绝原因中包含正确的变更名称推断和 `phase_log` 推荐；`phase_log` MCP 工具不经过 hook 不受影响 | Vitest (vite-plus/test) + child_process 调用 | 验证脚本在真实输入格式下的完整行为，包括拒绝信息的中文可读性和可操作性 |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `plugins/dev-team/bin/src/hooks/protect-eval.test.ts` | 单元测试 | Write 路径匹配 — 确认 `openspec/changes/<name>/eval.json` 被拒绝 |
| AC-2 | `plugins/dev-team/bin/src/hooks/protect-eval.test.ts` | 单元测试 | Edit 路径匹配 — 确认与 Write 同规则拦截 |
| AC-3 | `plugins/dev-team/bin/src/hooks/protect-eval.test.ts` | 单元测试 | Bash 重定向检测 — `>` / `>>` 后接 eval.json 被拒绝 |
| AC-4 | `plugins/dev-team/bin/src/hooks/protect-eval.test.ts` | 单元测试 | Bash heredoc 检测 — `<<` + 写 eval.json 被拒绝 |
| AC-5 | `plugins/dev-team/bin/src/hooks/protect-eval.test.ts` | 单元测试 | Python/Node 放行 — `python` / `python3` / `node` 开头命令被允许 |
| AC-6 | `plugins/dev-team/bin/src/hooks/protect-eval.test.ts` | 单元测试 | 拒绝原因格式 — 包含中文说明和 `mcp__plugin_dev-team_dev-team__phase_log` |
| AC-7 | `openspec/changes/protect-eval-json/tests/phase-log-unaffected.test.ts` | 集成测试 | `phase_log` MCP 工具不受 hook 影响，仍可正常写入 eval.json |
| AC-8 | `plugins/dev-team/bin/src/hooks/hooks-json.test.ts` | 单元测试 | `hooks.json` 文件存在且 JSON 格式正确，包含 Write\|Edit 和 Bash 两条 PreToolUse hook |
| AC-9 | 验证步骤在 CI/预处理阶段执行 | 静态分析 | `bash -n plugins/dev-team/hooks/scripts/protect-eval.sh` 语检查 |
| AC-10 | `plugins/dev-team/bin/src/hooks/protect-eval.test.ts` | 单元测试 | 非 eval.json 的 Write/Edit/Bash 操作返回 `allow`，无误拦截 |
| AC-11 | `plugins/dev-team/bin/src/hooks/hooks-json.test.ts` | 单元测试 | `plugin.json` 版本号高于 `2.5.9` |
| AC-12 | `openspec/changes/protect-eval-json/tests/auto-activation.test.ts` | 集成测试 | 插件自动发现 hooks.json 并注册 hook（模拟 Claude Code 启动加载流程） |

---

## 3. 正向 AC（Forward ACs）

| AC ID | 需求描述 | 测试层级 |
|-------|---------|----------|
| AC-1 | agent 调用 `Write` 工具写入 `openspec/changes/<name>/eval.json` 时被 hook 拦截，输出 `permissionDecision: "deny"` | 单元测试 |
| AC-2 | agent 调用 `Edit` 工具修改 `openspec/changes/<name>/eval.json` 时被 hook 拦截，输出 `permissionDecision: "deny"` | 单元测试 |
| AC-3 | agent 通过 Bash 命令 `echo '[]' > openspec/changes/<name>/eval.json` 写入 eval.json 时被拦截 | 单元测试 |
| AC-4 | agent 通过 Bash heredoc `cat > openspec/changes/<name>/eval.json <<EOF` 写入时被拦截 | 单元测试 |
| AC-5 | agent 通过 `python script.py --change test`（命令以 `python` 开头）写入 eval.json 时被放行，输出 `permissionDecision: "allow"` | 单元测试 |
| AC-7 | agent 调用 `mcp__plugin_dev-team_dev-team__phase_log` 写入 eval.json 时，hook 不生效，写入成功 | 集成测试 |
| AC-8 | `plugins/dev-team/hooks/hooks.json` 文件存在，JSON 解析有效，包含 description 字段和两条 PreToolUse hook | 单元测试 |
| AC-9 | `protect-eval.sh` 通过 `bash -n` 语法检查，退出码为 0 | 静态分析 |
| AC-10 | 对非 eval.json 文件的 `Write test.txt`、`Edit unrelated-file.ts`、`Bash(ls -la)` 等操作正常通过，返回 `allow` | 单元测试 |
| AC-11 | `plugins/dev-team/.claude-plugin/plugin.json` 中 `version` 字段值高于 `2.5.9` | 单元测试 |
| AC-12 | 在新项目中安装 dev-team 插件后，eval.json 保护自动激活无需额外配置 | 集成测试 |

---

## 4. 反向 AC（Reverse ACs）

| AC ID | 场景描述 | 测试层级 |
|-------|---------|----------|
| AC-6 | 拦截 eval.json 写入时，拒绝原因包含中文说明、明确提及 `mcp__plugin_dev-team_dev-team__phase_log` 作为替代工具、包含推断的变更名称 | 单元测试 |
| AC-6 | Bash 命令被拦截时，拒绝原因中包含 `phase_log` 建议和变更名称推断，使用中文书写 | 单元测试 |
| AC-3 | Bash 追加重定向 `echo '[]' >> openspec/changes/<name>/eval.json` 拦截（`>>` 与 `>` 同规则） | 单元测试 |
| AC-3 | Bash `tee` 管道写入 `echo '[]' \| tee openspec/changes/<name>/eval.json` 被拦截 | 单元测试 |
| AC-5 | `node script.js` 开头命令写入 eval.json 被放行（与 Python 同属受信任执行器） | 单元测试 |
| AC-5 | `python3 script.py > openspec/changes/test/eval.json` — 命令以 `python3` 开头，即使包含重定向也被放行 | 单元测试 |
| AC-10 | Bash `cat openspec/changes/test/eval.json` 只读操作（无重定向符号）被放行 | 单元测试 |
| AC-10 | Bash `grep "pass" openspec/changes/test/eval.json` 只读操作被放行 | 单元测试 |
| AC-6 | Write 拒绝原因从 `file_path` 推断变更名称（如 `my-feature`）；Bash 拒绝原因从命令字符串中的路径推断变更名称 | 单元测试 |
| — | 脚本输入 stdin 缺少 `tool_input` 或 `tool_input.file_path`/`tool_input.command` 字段时，默认输出 `allow`（失败放行策略） | 单元测试 |
| — | 脚本收到非 JSON 格式的 stdin 时，默认输出 `allow`（容错） | 单元测试 |
| — | 绝对路径匹配：`D:/Projects/wps-claude-plugin/openspec/changes/demo/eval.json` 被拒绝 | 单元测试 |
| — | 正斜杠与反斜杠路径：`openspec\\changes\\test\\eval.json` 应被正确识别 | 单元测试 |
| — | `hookSpecificOutput.hookEventName` 必须为 `"PreToolUse"`，格式与 Claude Code hook 协议一致 | 单元测试 |

---

## 5. 测试策略

### 5.1 方法

本变更的核心产出是 (1) 一个静态 JSON hook 声明文件和 (2) 一个 bash 拦截脚本。测试采用「TypeScript 驱动 + bash 子进程黑盒验证」的策略。对于 `protect-eval.sh`，通过 Vitest 在 `beforeAll` 中启动子进程执行 bash 脚本、从 stdin 注入工具调用 JSON、收集 stdout JSON 并断言决策结果。对于 `hooks.json` 和 `plugin.json`，直接解析 JSON 并断言字段存在性和合法性。这种策略无需模拟框架，因为脚本无外部依赖（只依赖 `jq` 和标准 POSIX 工具）。

### 5.2 测试分类

- **单元测试**: `plugins/dev-team/bin/src/hooks/protect-eval.test.ts` 覆盖路径匹配、命令分析、输出格式、容错处理；`plugins/dev-team/bin/src/hooks/hooks-json.test.ts` 覆盖 hooks.json 和 plugin.json 的结构验证。这些测试不依赖外部服务，使用 `child_process.execFileSync` 调用 `bash` 执行脚本。
- **集成测试**: `openspec/changes/protect-eval-json/tests/phase-log-unaffected.test.ts` 验证 `phase_log` MCP 工具依然正常工作；`openspec/changes/protect-eval-json/tests/auto-activation.test.ts` 验证插件加载后 hook 自动注册。集成测试同样使用 vitest，但涉及更完整的流程（如创建临时目录、写入 eval.json、调用 `phase-log` 模块函数）。
- **静态分析**: `bash -n plugins/dev-team/hooks/scripts/protect-eval.sh` 作为 CI 或预处理步骤，验证脚本语法。

### 5.3 模拟策略

| 测试目标 | 模拟策略 |
|---------|---------|
| `protect-eval.sh` 决策逻辑 | 不模拟。直接通过 `child_process.execFileSync('bash', [scriptPath])` 执行脚本，stdin 传入 JSON，stdout 读取决策。脚本的运行时依赖 (`jq`, `grep`, `sed`) 为系统工具，不做模拟。 |
| `phase_log` MCP 工具不受影响 | 模拟 `fs.readFileSync` / `fs.writeFileSync` 以避免写入真实文件系统，或使用 `fs.mkdtempSync` 创建临时测试目录。 |
| 插件自动激活 | 验证 hooks.json 的存在和格式正确性即可（自动激活是 Claude Code 运行时的行为，不在测试环境中模拟）。 |

---

## 6. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 路径深度边界 — 多级 changes 嵌套 | `tool_input.file_path: "project/sub/changes/test/eval.json"` | 应拒绝（路径包含 `/changes/.../eval.json` 后缀） | `protect-eval.test.ts` |
| 路径深度边界 — 无 changes 段 | `tool_input.file_path: "data/some/eval.json"` | 应放行（不匹配 `openspec/changes/**/eval.json` 模式） | `protect-eval.test.ts` |
| 变更名含特殊字符 | `tool_input.file_path: "openspec/changes/my-feature.v2/eval.json"` | 应拒绝（泛匹配，变更名含点号/连字符正常） | `protect-eval.test.ts` |
| Bash — 带引号路径重定向 | `tool_input.command: 'echo "[]" > "openspec/changes/test/eval.json"'` | 应拒绝（引号不影响路径后缀匹配） | `protect-eval.test.ts` |
| Bash — 变量展开重定向 | `tool_input.command: 'f=openspec/changes/test/eval.json; echo "[]" > $f'` | 若脚本不做变量展开解析，可能无法检测（预期为 `allow` — 遵循失败放行保守策略） | `protect-eval.test.ts` |
| Bash — 组合命令重定向 | `tool_input.command: 'echo "a" > /tmp/tmp.log && cat /tmp/tmp.log > openspec/changes/test/eval.json'` | 应拒绝（组合命令中包含写入 eval.json 的重定向） | `protect-eval.test.ts` |
| Bash — 文件描述符合并重定向 | `tool_input.command: 'echo "[]" >& openspec/changes/test/eval.json'` | 应拒绝（`>&` 语法指向 eval.json） | `protect-eval.test.ts` |
| Bash — Python 内部含重定向 | `tool_input.command: 'python -c "import json; json.dump([], open(\"openspec/changes/test/eval.json\",\"w\"))"'` | 应放行（命令以 `python` 开头，不分析 python 内部操作） | `protect-eval.test.ts` |
| Bash — 混合受信任/非受信任命令 | `tool_input.command: 'node build.js > /dev/null && echo "[]" > openspec/changes/test/eval.json'` | 以后续命令逻辑为准。若以 `node` 开头且后续有重定向：`node` 开头整条命令放行（整行判定） | `protect-eval.test.ts` |
| Bash — `cat` 无重定向 | `tool_input.command: 'cat openspec/changes/test/eval.json'` | 应放行（无 `>`、`>>`、`tee`、heredoc 等写模式） | `protect-eval.test.ts` |
| Bash — `tee -a` 追加 | `tool_input.command: 'echo "[]" \| tee -a openspec/changes/test/eval.json'` | 应拒绝（`tee -a` 追加写入同属写入操作） | `protect-eval.test.ts` |
| Bash — 命令以空格开头后跟 python | `tool_input.command: '  python script.py'` | 应放行（修剪前导空格后以 `python` 开头） | `protect-eval.test.ts` |
| 空 tool_input.file_path | `tool_input: { "file_path": "" }` | 应放行（路径为空无法匹配，默认 allow） | `protect-eval.test.ts` |
| 缺失 tool_input 字段 | `tool_input: {}`（无 file_path 也无 command） | 应放行（失败放行策略） | `protect-eval.test.ts` |
| Write 到 root 路径的 eval.json | `tool_input.file_path: "/openspec/changes/test/eval.json"` | 应拒绝（绝对路径以 `/openspec/changes/` 开头，后缀匹配 eval.json） | `protect-eval.test.ts` |
| 拒绝原因最大长度 | 输出 `permissionDecisionReason` 不应超过合理长度（建议 < 500 字符） | 拒绝原因应在 200-500 字符内，包含文案要点即可 | `protect-eval.test.ts` |

---

## 7. 测试数据

### 7.1 预置测试路径

| 测试路径 | 预期结果 | 说明 |
|---------|---------|------|
| `openspec/changes/test/eval.json` | deny | 标准相对路径 |
| `D:/Projects/wps-claude-plugin/openspec/changes/demo/eval.json` | deny | Windows 绝对路径 |
| `/home/user/project/openspec/changes/feat-x/eval.json` | deny | Unix 绝对路径 |
| `openspec/changes/test/design.md` | allow | changes 下但非 eval.json |
| `plugins/dev-team/bin/src/commands/phase-log.ts` | allow | changes 外的文件 |
| `openspec/changes/.hidden/eval.json` | deny | 隐藏变更目录 |
| `node_modules/openspec/changes/nested/eval.json` | — | 设计未明确要求，取决于脚本是否只匹配 `openspec/changes/...` 前缀 |

### 7.2 预置 Bash 命令

| 测试命令 | 预期结果 | 说明 |
|---------|---------|------|
| `echo '[]' > openspec/changes/test/eval.json` | deny | 标准重定向 |
| `echo '[]' >> openspec/changes/test/eval.json` | deny | 追加重定向 |
| `cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF` | deny | heredoc |
| `echo '[]' \| tee openspec/changes/test/eval.json` | deny | tee 写入 |
| `python script.py --change test` | allow | 受信任脚本 |
| `python3 -c "..."` | allow | Python3 受信任 |
| `node build.js` | allow | Node 受信任（如设计支持） |
| `cat openspec/changes/test/eval.json` | allow | 只读 |
| `ls -la` | allow | 无关命令 |

---

## 8. 不可测试项

- **PreToolUse hook 在 Claude Code 运行时的实际调度** — 真实 hook 调度发生在 Claude Code harness 内部，需要在 Claude Code 中手动验证。测试只能验证脚本本身的逻辑正确性，不能验证 harness 是否正确调用了 hook。验证方式：手动执行验收标准 AC-1 至 AC-4 在 Claude Code 中确认拦截生效。
- **jq 工具在用户环境中的可用性** — 脚本依赖 `jq` 解析 JSON。不同用户环境可能未安装 `jq`。测试可以假设 `jq` 已安装（在 CI 环境中预置），但在用户真实环境中是否可用不在自动测试范围内。设计中已有降级说明（使用 `grep`/`sed` 备用）。
- **插件自动激活的用户体验** — 新项目安装 dev-team 插件后保护是否自动激活，依赖于 Claude Code 的插件发现机制。自动测试可验证 `hooks.json` 存在于正确路径且格式正确，但无法模拟 Claude Code 的插件加载流程。AC-12 需通过手动测试验证。
- **`phase_log` MCP 工具的行为正确性** — 此变更不修改 `phase_log` 工具的行为（设计明确 `不要修改`）。`phase_log` 的现有测试在 `eval-json.test.ts` 和 `phase-next.test.ts` 中已有覆盖，本变更不重复测试这些功能。
