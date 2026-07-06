# 设计: static-check-agent-hook

> **变更**: static-check-agent-hook
> **日期**: 2026-06-12
> **基于**: proposal.md, specs/static-check-hook/spec.md, specs/embedded-cli/spec.md, specs/phase-agents/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Hook 声明文件 | 注册 `subagentStop` hook，matcher 限定 `implementation-generator`，配置 `loop_limit` 与可选 `timeout` | `plugins/dev-team/hooks/hooks.json` | 无 | JSON (Claude Code hooks 协议) |
| 静态检查门禁脚本 | 调用 CLI 执行静态分析，根据 exit code 返回 `{}` 或 `{ decision: "block", reason: "..." }` | `plugins/dev-team/hooks/scripts/static-check.mjs` | Node.js, `dev-team-cli.cjs` | Node.js ESM (`.mjs`) |
| CLI 打包入口 | 使用 `cac` 注册子命令，`cli.parse()` 路由到命令实现 | `plugins/dev-team/bin/src/cli.ts` | `cac`, `run-static-analysis.ts` | TypeScript |
| run_static_analysis 命令 | 读取 `openspec/config.json` 的 `static_analysis` 字段，在项目根目录执行配置命令 | `plugins/dev-team/bin/src/commands/run-static-analysis.ts` | `lib/config.ts`, `child_process` | TypeScript |
| 配置读取库 | `ensureConfigFile()` / `getValue()` 读取 `static_analysis` 配置 | `plugins/dev-team/bin/src/lib/config.ts` | Zod schema | TypeScript（已有，复用） |
| CLI 打包产物 | hook 通过 `node` 调用的独立 CLI 二进制 | `plugins/dev-team/bin/dev-team-cli.cjs` | vite-plus pack | CommonJS bundle |
| MCP 打包产物 | MCP server 入口（不变） | `plugins/dev-team/bin/dev-team-mcp.cjs` | vite-plus pack | CommonJS bundle |
| implementation-generator | 编写实现代码、标记 tasks.md；不再负责静态检查 | `plugins/dev-team/agents/implementation-generator.md` | 无 | Markdown agent 定义 |
| implementation-evaluator | 基于 git diff 评估实现；不再验证 I7 静态检查项 | `plugins/dev-team/agents/implementation-evaluator.md` | 无 | Markdown agent 定义 |
| 插件元数据 | 版本号递增，反映 hook 与 CLI 变更 | `plugins/dev-team/.claude-plugin/plugin.json` | 无 | JSON |

### 组件图

```
implementation-generator 尝试结束
              |
              v
+---------------------------+    matcher: implementation-generator
| Claude Code subagentStop  | ----------------------------------+
| hook dispatch             |                                   |
+---------------------------+                                   v
              |                              +---------------------------+
              |                              | static-check.sh           |
              |                              | node dev-team-cli.cjs     |
              |                              |   run_static_analysis     |
              |                              +---------------------------+
              |                                         |
              |                              +----------+----------+
              |                              |                     |
              |                         exit 0               exit != 0
              |                              |                     |
              v                              v                     v
       允许 subagent 结束              stdout: {}          stdout: { decision: "block", reason: "..." }
                                              |                     |
                                              v                     v
                                       generator 正常结束     generator 继续修复
                                                            (最多 loop_limit=5 次)
```

现有 `PreToolUse` hook（`protect-eval.sh`）与新增的 `subagentStop` hook 并行存在，互不影响。

---

## 数据流

### 流程描述

1. **Hook 注册阶段**：Claude Code 加载 `plugins/dev-team/hooks/hooks.json`，注册 `subagentStop` hook，`matcher` 为 `"implementation-generator"`，`loop_limit` 为 `5`。

2. **Generator 结束拦截阶段**（`implementation-generator` 每次尝试结束时触发）：
   - Claude Code 将 subagentStop 事件 JSON 通过 stdin 传入 `static-check.sh`（脚本 MAY 忽略 stdin）
   - 脚本执行：`node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs" run_static_analysis`
   - 捕获 CLI 的 stdout/stderr 与 exit code

3. **CLI 执行阶段**（`run_static_analysis`）：
   - 确定项目根目录：优先 `process.env.CLAUDE_PROJECT_DIR`，否则 `process.cwd()`
   - 调用 `ensureConfigFile(projectRoot)` 确保 `openspec/config.json` 存在
   - 调用 `getValue(config, "static_analysis")` 读取配置
   - 若 `exists === false` 或 `value` 为空字符串：exit `0`（放行，不执行任何命令）
   - 若 `value` 为非空字符串：在项目根目录通过 `shell: true` 执行该命令（0 参数，不传 change name）
   - 命令 exit `0` → CLI exit `0`；命令 exit 非 `0` → CLI exit 相同非零 code，并将 stdout/stderr 输出到 CLI stderr

4. **Hook 决策阶段**：
   - CLI exit `0`：`static-check.sh` 向 stdout 输出 `{}`，脚本 exit `0` → subagent 允许结束
   - CLI exit 非 `0`：`static-check.mjs` 向 stdout 输出 `{"decision": "block", "reason": "<错误输出 + 中文修复指令>"}`，脚本 exit `0` → subagent 被阻止结束，agent 收到 reason 继续修复
   - 连续 5 次 followup 后，hook 框架不再阻止结束（有意的降级策略）

5. **Agent 职责变更**：
   - `implementation-generator` 不再调用 `config_get("static_analysis")`，不再生成 `reports/static_analysis.json`
   - `implementation-evaluator` 不再检查 I7 项，判定范围变为 I1–I6 和 I8

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| Hook 声明 | `description`: string, `hooks.subagentStop[]`: array | 每项引用 `static-check.sh`，含 `matcher`、`loop_limit` | `hooks.json` |
| Hook 输入 (stdin JSON) | subagentStop 事件字段（subagent 类型、会话上下文等） | 由 Claude Code 运行时生成；脚本可忽略 | 不持久化 |
| Hook 输出 (stdout JSON) | `{}` 或 `{ "decision": "block", "reason": string }` | reason 携带 CLI 错误输出 | 不持久化 |
| OpenSpec 配置 | `static_analysis?`: string（shell 命令，0 参数） | 由 `openspec/config.json` schema 定义，格式不变 | `openspec/config.json` |
| CLI 命令结果 | exit code: number, stderr: string | 由外部 lint/typecheck 命令产生 | 不持久化，不写入 report 文件 |

**不再使用的模型**：`openspec/changes/<change-name>/reports/static_analysis.json` — hook 直接根据 exit code 判断，无需中间报告文件。

---

## 路由/API 设计

### CLI 子命令

| 命令 | 子命令 | 描述 | 输入 | 输出 | 退出码 |
|------|--------|------|------|------|--------|
| `dev-team-cli.cjs` | (顶级) | 显示帮助信息 | `[--help]` | stdout: 可用子命令列表 | 0 |
| `dev-team-cli.cjs` | `run_static_analysis` | 读取配置并执行静态检查命令 | 无 CLI 参数；项目根由 `CLAUDE_PROJECT_DIR` 或 `cwd` 决定 | 成功时无输出；失败时 stderr 含命令输出 | 0（未配置/通过），非 0（命令失败） |

**调用示例**（hook 内部）：

```bash
node "${CLAUDE_PLUGIN_ROOT}/bin/dev-team-cli.cjs" run_static_analysis
```

### Hook 输入/输出规范

**输入** (stdin JSON，可选读取):

```json
{
  "subagent_type": "implementation-generator",
  "...": "其他 subagentStop 事件字段"
}
```

**检查通过时输出** (stdout JSON):

```json
{}
```

**检查失败时输出** (stdout JSON):

```json
{
  "decision": "block",
  "reason": "静态检查未通过，请修复以下错误后重新提交：\n\n<CLI stdout/stderr 完整输出>"
}
```

**Hook 声明** (`hooks.json`):

```json
{
  "hooks": {
    "subagentStop": [
      {
        "matcher": "implementation-generator",
        "loop_limit": 5,
        "hooks": [
          {
            "type": "command",
            "command": "bash \"${CLAUDE_PLUGIN_ROOT}/hooks/scripts/static-check.sh\""
          }
        ]
      }
    ]
  }
}
```

### subagentStop Hook 声明

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| subagentStop | `implementation-generator` | generator 结束时自动执行静态检查 | subagentStop 事件 JSON (stdin) | `{}` 或 `{ decision: "block", reason: "..." }` | 插件 hook 自动注册 |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 使用 `subagentStop` hook 而非 agent 指令执行静态检查 | 确定性执行，agent 无法跳过；与 generator 职责解耦 | **prompt hook**：仍依赖 LLM 遵从，与当前问题相同；**继续 agent 指令**：无强制性，token 紧张时易跳过 |
| D2 | 新增独立 `dev-team-cli.cjs` 打包入口 | hook 脚本通过 `node` 调用简单可靠；复用 `lib/config.ts` 避免重复 JSON 解析 | **hook 内 `node -e` 读 JSON**：与 config 逻辑重复、难测试；**MCP 调用**：stdio JSON-RPC 从 shell 调用过于复杂 |
| D3 | 根据 CLI exit code 直接判断，不生成 `reports/static_analysis.json` | 简化数据流，消除 report 缺失导致的 evaluator 误判 | **继续生成 report + hook 只检查 report**：仍依赖 agent 执行检查，不解决核心问题 |
| D4 | 未配置 `static_analysis` 时 exit 0 放行 | 与现有 generator 行为一致（无配置则跳过检查）；`ensureConfigFile()` 容错等同于未配置 | **未配置时 exit 非 0**：会阻塞所有未配置项目，破坏向后兼容 |
| D5 | `loop_limit: 5` 后允许 generator 结束 | 避免无法自动修复的错误导致无限循环；evaluator 仍可在 review 中发现代码质量问题 | **无 loop_limit**：可能无限循环消耗 token；**loop_limit 后 fail 整个 phase**：需要额外 workflow 变更，超出本变更范围 |
| D6 | matcher 仅匹配 `implementation-generator` | 本变更范围明确；`test-gen-generator` 留待未来扩展 matcher | **同时匹配 test-gen-generator**：proposal 明确不在本变更范围 |
| D7 | hook 脚本 exit 0，通过 JSON `decision: "block"` + `reason` 传递 followup | 符合 subagentStop hook 协议：followup 由 stdout JSON 字段传递，非脚本 exit code | **脚本 exit 非 0**：hook 框架可能将其视为脚本崩溃而非 followup |

---

## 依赖

### 运行时依赖

- **Node.js** — 执行 `dev-team-cli.cjs` 及外部 `static_analysis` 命令
- **`CLAUDE_PLUGIN_ROOT`** — hook 脚本定位插件内 CLI 与脚本路径
- **`CLAUDE_PROJECT_DIR`**（可选）— CLI 确定项目根目录
- **`openspec/config.json`** — `static_analysis` 字段提供检查命令（格式不变）
- **bash** — 与现有 `protect-eval.sh` 相同的 hook 调用模式

### 构建/测试依赖

- **vite-plus** (`vp pack`, `vp test`) — 打包 `dev-team-cli.cjs` 与 `dev-team-mcp.cjs`
- **cac** — CLI 子命令注册与 `--help` 生成（需加入 `plugins/dev-team/bin/package.json`）
- **vitest** (via vite-plus) — `run-static-analysis.test.ts` 单元测试
- **TypeScript** — CLI 源码编译

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `dev-team-cli.cjs` 未打包或路径错误 | hook 无法执行静态检查 | 低 | 构建流程在 `vp pack` 中生成两个产物；hook 脚本可检查 CLI 文件是否存在并输出明确错误 |
| 静态检查命令超时或卡住 | generator 被 hook 阻塞 | 低 | hooks.json 可选配置 `timeout` 限制脚本执行时间 |
| `loop_limit` 耗尽后代码仍有 lint 错误 | 带问题的代码进入 evaluator | 中 | 有意的降级策略；evaluator 仍可在 review 中发现质量问题 |
| `openspec/config.json` 不存在或损坏 | CLI 读取失败 | 低 | `ensureConfigFile()` / `readConfig()` 已有容错，等同于未配置，直接放行 |
| Windows 环境下 bash hook 兼容性 | hook 在 Windows 上无法运行 | 中 | 与现有 `protect-eval.sh` 使用相同 bash 调用模式，保持一致 |

---

## 迁移步骤

1. **实现 CLI**：新增 `src/cli.ts`、`src/commands/run-static-analysis.ts` 及单元测试；添加 `cac` 依赖。
2. **更新打包配置**：`vite.config.ts` 增加 `dev-team-cli.cjs` 打包入口，保持 `dev-team-mcp.cjs` 不变；执行 `pnpm run build` 验证双产物。
3. **实现 hook 脚本**：创建 `static-check.sh`，实现 CLI 调用与 followup JSON 输出；`bash -n` 语法检查；设置可执行权限。
4. **注册 hook**：更新 `hooks.json` 新增 `subagentStop` 配置，更新 `description`；不修改现有 `PreToolUse` 配置。
5. **精简 agent 定义**：从 `implementation-generator.md` 移除步骤 7–8 及 report 输出；从 `implementation-evaluator.md` 移除 I7 行。
6. **递增版本号**：`plugin.json` version 从 `2.6.9` patch bump（如 `2.6.10`）。
7. **验收验证**：手动运行 CLI、模拟 hook stdin/stdout、端到端验证 AC-1 至 AC-12。

---

## 待决问题

- 无。proposal 与三份 spec 已覆盖实现细节；`test-gen-generator` matcher 扩展明确留待后续变更。
