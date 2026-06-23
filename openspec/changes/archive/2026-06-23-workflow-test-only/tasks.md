# 实现任务: workflow-test-only

> **变更**: workflow-test-only
> **日期**: 2026-06-22

---

## 阶段 1: change-config — workflow.json 读取层

- [x] 新建 `plugins/dev-team/bin/src/lib/change-config.ts`：
  - `readWorkflowConfig(change: string): Record<string, unknown>` — 解析 `openspec/changes/<change>/workflow.json`；文件不存在返回 `{}`
  - `getWorkflowType(change: string): string` — 返回 `workflow_type` 字段；缺省 `"requirement"`
  - 支持值：`"requirement"` | `"bug-fix"` | `"refactor"` | `"test-only"`
  - 无效 JSON 抛出可读错误

## 阶段 2: workflow.ts — test-only 阶段表与 prerequisite

- [x] 在 `plugins/dev-team/bin/src/lib/workflow.ts` 新增 `PHASE_TEST_ONLY` 数组（6 阶段）：
  - `01-proposal`：复用 proposal agent；planner prompt 引导测试覆盖缺口、测试策略、已有代码验收标准（与 requirement 的 01-proposal prompt 区分）
  - `02-code-analyze`：`code-analyze-planner` / `code-analyze-evaluator`
  - `03-test-design`、`04-test-gen`：agent 与 prompt 与 `PHASE_REQUIREMENT` 对应 phase 一致
  - `06-unit-test`、`08-integration-test`：agent 与 requirement 一致；evaluator prompt 追加 WORKFLOW_CONTEXT 自适应指令
- [x] 新增 `PHASE_TEST_ONLY_PREREQUISITES`：
  ```
  01-proposal: []
  02-code-analyze: [01-proposal]
  03-test-design: [01-proposal, 02-code-analyze]
  04-test-gen: [03-test-design]
  06-unit-test: [04-test-gen]
  08-integration-test: [04-test-gen]
  ```
- [x] 注册到 `PHASE_TABLES['test-only']` 与 `PHASE_PREREQUISITES_TABLES['test-only']`
- [x] 确认 `PHASE_REQUIREMENT`、`PHASE_BUG_FIX`、`PHASE_REFACTOR` 及对应 prerequisite 表未被修改

## 阶段 3: phase_next — 从 workflow.json 解析 workflow_type

- [x] 更新 `plugins/dev-team/bin/src/schemas/phase-next.schema.ts`：
  - **移除** `workflow_type` 字段；input 仅保留 `change`（AC-17）
- [x] 更新 `plugins/dev-team/bin/src/commands/phase-next.ts`：
  - `PhaseNextOptions` 移除 `workflow_type?`
  - `runPhaseNext()` 调用 `getWorkflowType(options.change)` 替代 `options.workflow_type ?? DEFAULT_WORKFLOW`
- [x] 更新 `plugins/dev-team/bin/src/mcp.ts`：
  - `registerPhaseNextTool` 不再转发 `workflow_type`

## 阶段 4: phase_log — 工作流感知 backtrack 校验

- [x] 确认 `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` **不含** `workflow_type` 字段（AC-17）
- [x] 更新 `plugins/dev-team/bin/src/commands/phase-log.ts`：
  - `handleBacktrackMarking()` 调用 `getWorkflowType(options.change)` 获取 workflow 变体
  - 用 `getPhaseTable(workflowType)` 的 phase ID 集合校验 `backtrack_to` 目标（**替换**全局 `getPhaseIndex()` 成员校验）
  - 保留「不支持回溯到当前或未来 phase」规则（基于当前 workflow 的 phase 表顺序）
  - 非法目标抛错：`当前工作流 {workflow_type} 不包含 phase '{target}'。可用的 phases: {list}`，且不写入 eval.json
- [x] 更新 `plugins/dev-team/bin/src/mcp.ts`：
  - `registerPhaseLogTool` 确认不转发 `workflow_type`

## 阶段 5: code-analyze agent 对

- [x] 创建 `plugins/dev-team/agents/code-analyze-planner.md`：
  - frontmatter: `name: code-analyze-planner`, `model: opus-4.6`, Tools: Read, Write, Grep, Glob, Bash
  - 读取 proposal.md、design.md.template、CLAUDE.md 与现有代码库
  - 逆向分析并写入 `design.md`（架构组件含文件路径、数据流、路由/API、已观察决策、模块边界）
  - **不**写入 `tasks.md`；**不**包含测试策略章节
  - 语言规范：简体中文叙述 + 英文标识符/路径/CLI
- [x] 创建 `plugins/dev-team/agents/code-analyze-evaluator.md`：
  - frontmatter: `name: code-analyze-evaluator`, `disallowedTools: Write, Edit`
  - 读取 design.md + proposal.md（**不**读 tasks.md）
  - 静态 checklist：组件表完整、数据流具体、覆盖 proposal AC、无测试策略章节、不要求 tasks.md
  - 调用 `phase_log(phase: "02-code-analyze")` — **不传** `workflow_type`
  - 参考 `dev-design-evaluator.md` 结构，省略 tasks 相关检查项

## 阶段 6: proposal-planner — workflow_type 确认

- [x] 更新 `plugins/dev-team/agents/proposal-planner.md` Process 开头：
  - 读取 `openspec/changes/<change>/workflow.json`
  - 文件不存在或缺少 `workflow_type` → AskQuestion 展示 `requirement` / `bug-fix` / `refactor` / `test-only` 及简要说明
  - 写入 `{"workflow_type": "<选择>"}` 到 `workflow.json`
  - 已设置 → 跳过确认，直接写 proposal.md 与 specs/（AC-18）

## 阶段 7: workflow skills — scaffold 写 workflow.json

- [x] 更新 `plugins/dev-team/skills/workflow-requirement/SKILL.md`：
  - Step 1 scaffold 后写入 `openspec/changes/<name>/workflow.json`：`{"workflow_type": "requirement"}`
  - Step 2 循环改为 `phase_next(change=<name>)` — **移除** `workflow_type="requirement"` 参数
- [x] 创建 `plugins/dev-team/skills/workflow-test-only/SKILL.md`：
  - 复制 `workflow-requirement/SKILL.md` 结构（Step 0 change 解析、Step 1 scaffold、Step 2 循环、Step 3 完成）
  - frontmatter: `name: workflow-test-only`, `disable-model-invocation: true`
  - Step 1 scaffold 后写入 `{"workflow_type": "test-only"}` 到 `workflow.json`（AC-19）
  - Step 2 循环调用 `phase_next(change=<name>)` — **无** `workflow_type` 参数（AC-8, AC-17）
  - 使用 underscore MCP 工具名 `mcp__plugin_dev-team_dev-team__phase_next`
  - **无**硬编码 phase 表或 agent 名；**无** `phase_log` 调用含 `workflow_type`
  - Step 3 完成摘要：`total_phases: 6`，不提及 `09-acceptance`，提示手动 `/dev-team:openspec-archive-change`
  - bug 发现处理：evaluator 返回 `verdict: "fail"`, `backtrack_to: null` 且 report 含代码 bug 时：
    1. 写入 `openspec/changes/<name>/reports/code-bugs-found.md`
    2. 通知用户
    3. 询问继续（如 integration-test）或终止 workflow（AC-15）

## 阶段 8: eval.schema.json（按需）

- [x] 检查 `plugins/dev-team/templates/artifacts/eval.schema.json` 的 phase 枚举是否包含 `02-code-analyze`
- [x] 若 MCP/CLI 校验拒绝该 phase ID，则同步新增枚举值与 description（文件不存在；MCP 使用 Zod schema 无 phase 枚举限制，无需变更）

## 阶段 9: 版本与构建

- [x] 递增 `plugins/dev-team/.claude-plugin/plugin.json` 版本号
- [x] 运行 `plugins/dev-team` 构建，确认 MCP bundle 重新生成
