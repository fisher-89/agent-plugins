# 提案: 静态检查从 agent 指令迁移到 subagentStop Hook

> **变更**: static-check-agent-hook
> **日期**: 2026-06-12
> **状态**: 提案

---

## 问题

当前静态检查（lint、类型检查）逻辑嵌入在 `implementation-generator` agent 的步骤 7-8 中：agent 在写完代码后调用 `config_get("static_analysis")` 获取检查脚本并执行，然后生成 `reports/static_analysis.json` 报告。`implementation-evaluator` 的 I7 检查项通过读取该报告间接验证。

这种设计存在以下问题：

1. **无强制性**：静态检查是 agent 指令的一部分，agent 可能跳过、遗忘或执行不完整。特别是在 token 预算紧张或上下文被压缩时，agent 倾向跳过末尾步骤。

2. **关注点耦合**：generator 的核心职责是写代码，却额外承担了质量检查、报告生成的责任。这违反了单一职责原则，也增加了 agent 指令的复杂度。

3. **检查结果依赖报告文件**：当前流程要求 generator 生成 `reports/static_analysis.json`，evaluator 的 I7 检查项通过读取该文件间接判断。若 generator 未生成报告，evaluator 无法判断检查是否执行过。

4. **不可扩展**：相同的检查需求也适用于 `test-gen-generator`（todo.md 第 5 项），但当前方案需要在每个 generator agent 中重复相同的指令。

---

## 提案

利用 Cursor 的 `subagentStop` hook 事件，在 `implementation-generator` 结束时**自动、确定性地**执行静态检查。若检查未通过，hook 返回 `decision: "block"` + `reason` 阻止 agent 结束并要求其修复问题。

### 核心设计

1. **`plugins/dev-team/hooks/hooks.json`** — 新增 `subagentStop` hook，matcher 匹配 `implementation-generator`：
   - hook 脚本在 agent 尝试结束时触发
   - 使用 `loop_limit: 5` 控制最大重试次数，防止无限循环
   - 可选配置 `timeout` 限制检查脚本执行时间

2. **`plugins/dev-team/hooks/scripts/static-check.sh`** — 检查门禁脚本：
   - 调用 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs" run_static_analysis`
   - CLI exit code 为 0：返回空 JSON `{}`，允许 agent 结束
   - CLI exit code 非 0：返回 `decision: "block"` + `reason`，携带 CLI 的错误输出，要求 agent 修复

3. **`plugins/dev-team/bin/dev-team-cli.cjs`** — 新增 CLI 打包产物（入口 `src/cli.ts`）：
   - 子命令 `run_static_analysis` 复用 `lib/config.ts` 的 `ensureConfigFile()`、`getValue()` 读取 `openspec/config.json` 的 `static_analysis` 字段
   - 未配置时 exit 0（放行）
   - 已配置时在项目根目录执行该命令（0 参数，不传 change name）
   - 命令通过 exit 0，失败 exit 非 0 并输出 stderr/stdout

4. **`plugins/dev-team/agents/implementation-generator.md`** — 移除步骤 7-8（静态检查相关指令）、Output 中的报告生成要求，以及 frontmatter 中对 AUTO static-check 的引用

5. **`plugins/dev-team/agents/implementation-evaluator.md`** — 移除 I7 检查项（静态检查由 hook 层保证，evaluator 不再需要事后验证）

### 不采用的替代方案

| 方案 | 不采用原因 |
|------|-----------|
| prompt hook | 失去确定性，和当前 agent 指令方案问题类似 |
| hook 只做门禁（检查 report 文件） | 仍依赖 agent 执行检查和生成 report，不解决核心问题 |
| hook 脚本内 `node -e` 直接读 JSON | 与现有 config 读取逻辑重复，且难以测试；CLI 复用 `lib/config.ts` 更可靠 |
| 通过 MCP 调用获取配置 | MCP 是 stdio JSON-RPC 协议，从 shell 脚本调用过于复杂 |
| 继续生成 `reports/static_analysis.json` | hook 直接执行检查并根据 exit code 判断，无需中间报告文件 |

---

## 能力

### 新增能力

- **static-check-hook** — 通过 `subagentStop` hook 在 `implementation-generator` 结束时自动执行静态检查，未通过则阻止结束并要求修复

### 修改的能力

- **embedded-cli** — 新增 `dev-team-cli.cjs` 打包入口及 `run_static_analysis` 子命令，供 hook 和其他脚本调用
- **phase-agents** — `implementation-generator` 移除内嵌静态检查职责；`implementation-evaluator` 移除 I7 检查项

---

## 变更范围

### 实现以下特性

- `plugins/dev-team/hooks/hooks.json` — 新增 `subagentStop` hook 声明：
  - matcher: `implementation-generator`
  - command: `bash "${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.sh"`
  - `loop_limit`: 5
- `plugins/dev-team/hooks/scripts/static-check.sh` — 新增 hook 脚本：
  - 调用 `node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs" run_static_analysis`
  - 根据 CLI exit code 返回 `{}` 或 `{ decision: "block", reason: "..." }`
  - 脚本 SHALL 具有可执行权限（`chmod +x`）
- `plugins/dev-team/bin/src/cli.ts` — 新增 CLI 入口（使用 `cac` 注册子命令）
- `plugins/dev-team/bin/src/commands/run-static-analysis.ts` — 实现 `run_static_analysis` 逻辑
- `plugins/dev-team/bin/vite.config.ts` — 增加 `dev-team-cli.cjs` 打包配置
- `plugins/dev-team/bin/src/commands/run-static-analysis.test.ts` — 单元测试
- `plugins/dev-team/agents/implementation-generator.md` — 移除步骤 7-8、报告生成部分、frontmatter 中 static-check 引用
- `plugins/dev-team/agents/implementation-evaluator.md` — 移除 I7 检查项
- `plugins/dev-team/.claude-plugin/plugin.json` — 版本号递增

### 不要修改

- `openspec/config.json` 的 `static_analysis` 字段定义和 schema — 配置格式不变（字符串命令，0 参数）
- `plugins/dev-team/hooks/scripts/protect-eval.sh` — 现有 eval.json 保护 hook 不受影响
- `plugins/dev-team/skills/phase-implement/SKILL.md` — G→E 循环逻辑不变
- `plugins/dev-team/bin/dev-team-mcp.cjs` / `src/mcp.ts` — MCP server 打包入口不变
- `test-gen-generator` 的 hook matcher — 留待未来扩展

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `implementation-generator` 结束时自动触发静态检查 | 运行 generator，观察 subagentStop hook 触发并执行 `static_analysis` 命令 |
| AC-2 | 静态检查失败时 generator 不结束，收到 `decision: "block"` + `reason` 后继续修复 | 故意引入 lint 错误，确认 generator 被要求继续修复而非直接结束 |
| AC-3 | 静态检查通过时 generator 正常结束 | 代码无 lint 错误时，确认 generator 正常完成 |
| AC-4 | 未配置 `static_analysis` 时 generator 直接结束，hook 不阻塞 | 移除 `openspec/config.json` 中的 `static_analysis` 字段，确认 generator 正常完成 |
| AC-5 | `reason` 包含具体的错误输出内容 | 检查 hook 返回的 reason 中包含 CLI 的 stderr/stdout 错误信息 |
| AC-6 | 重试次数不超过 `loop_limit`（5 次） | 引入无法自动修复的错误，确认 5 次后 generator 被允许结束 |
| AC-7 | `dev-team-cli.cjs run_static_analysis` 正确读取并执行配置 | 手动运行 CLI，验证未配置时 exit 0、已配置时执行命令并返回对应 exit code |
| AC-8 | `implementation-generator.md` 不再包含静态检查步骤 | 读取 agent 文件，确认步骤 7-8 和报告生成部分已移除 |
| AC-9 | `implementation-evaluator.md` 不再包含 I7 检查项 | 读取 evaluator 文件，确认 I7 行已移除 |
| AC-10 | 现有 `protect-eval.sh` hook 不受影响 | 执行 eval.json 直接写入操作，确认仍被拦截 |
| AC-11 | 其他 subagent 类型的 stop 事件不受此 hook 影响 | 运行 proposal-planner 等其他 agent，确认 hook 不触发 |
| AC-12 | `plugin.json` 版本号已递增 | 读取 plugin.json，确认版本号高于 `2.6.9` |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `dev-team-cli.cjs` 未打包或路径错误 | hook 无法执行静态检查 | 低 | hook 脚本检查 CLI 文件是否存在；构建流程在 `vp pack` 中生成两个产物 |
| 静态检查命令本身超时或卡住 | generator 被 hook 阻塞 | 低 | hook 配置 `timeout` 限制脚本执行时间 |
| `loop_limit` 耗尽后 generator 结束，代码仍有 lint 错误 | 带问题的代码进入 evaluator | 中 | evaluator 仍可在 review 中发现代码质量问题；这是有意的降级策略，避免无限循环 |
| `openspec/config.json` 不存在或损坏 | CLI 读取失败 | 低 | `ensureConfigFile()` 已有容错逻辑，等同于未配置 `static_analysis`，直接放行 |
| Windows 环境下 bash hook 脚本兼容性 | hook 在 Windows 上无法运行 | 中 | 与现有 `protect-eval.sh` 使用相同 bash 调用模式，保持一致 |

---

## 未来扩展

此方案天然支持扩展到其他 generator agent：

- `test-gen-generator`：对应 todo.md 第 5 项「使用静态扫描工具检查测试文件」，只需在 `subagentStop` 的 matcher 中添加 `test-gen-generator` 即可复用同一 hook 脚本和 CLI
- 其他需要代码质量门禁的 agent：同理扩展 matcher
