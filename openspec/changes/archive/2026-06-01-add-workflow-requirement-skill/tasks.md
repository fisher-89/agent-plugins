# 实现任务: add-workflow-requirement-skill

> **变更**: add-workflow-requirement-skill
> **日期**: 2026-06-01

---

## 阶段 1: 基础数据变更

- [x] 更新 `plugins/dev-team/templates/artifacts/eval.schema.json` 中的 phase description 和 examples，添加 `01-proposal` 作为合法 phase 标识符
- [x] 更新 `plugins/dev-team/bin/src/lib/workflow.ts` 将 `PHASES` 数组中的 `'01-requirements'` 改为 `'01-proposal'`
- [x] 更新 `plugins/dev-team/bin/src/lib/workflow.test.ts` 将 phase 名称断言从 `'01-requirements'` 改为 `'01-proposal'`

## 阶段 2: 重命名 requirements-evaluator 为 proposal-evaluator

- [x] 重命名文件 `plugins/dev-team/agents/requirements-evaluator.md` → `plugins/dev-team/agents/proposal-evaluator.md`
- [x] 更新 `plugins/dev-team/agents/proposal-evaluator.md` 中的所有 phase ID 引用: `01-requirements` → `01-proposal`
- [x] 更新 `plugins/dev-team/agents/proposal-evaluator.md` 中的 `name` 元数据和 description 中的 agent name
- [x] 检查所有引用 `requirements-evaluator` 的文件（skills、文档），更新为 `proposal-evaluator`

## 阶段 3: 新增 proposal-planner agent

- [x] 创建 `plugins/dev-team/agents/proposal-planner.md`:
  - `name: proposal-planner`, `model: opus`
  - 工具: Read, Write, Grep, Glob, Bash
  - 读取 `proposal.md.template`、`openspec spec list --json`、CLAUDE.md
  - 写入 `proposal.md` 和 `specs/<capability>/spec.md`
  - 接受可选的 `EXPLORE_CONTEXT_SUMMARY` prompt 参数
  - 语言规范: 简体中文叙述 + 英文标识符/路径/CLI/技术缩写/场景标记

## 阶段 4: 新增 eval/next MCP 工具

- [x] 创建 `plugins/dev-team/bin/src/schemas/eval-next.schema.ts`:
  - 定义 evalNextInputSchema: `change` (string, required), `workflow_type` (string, optional, default `"requirement"`)
  - 定义 evalNextOutputSchema: `done`, `error`, `next_phase`, `phase_pattern`, `planner`, `evaluator`, `auto_steps`, `total_phases`, `phase_index`, `round`
- [x] 在 `plugins/dev-team/bin/src/schemas/index.ts` 中导出 eval-next schema
- [x] 创建 `plugins/dev-team/bin/src/commands/eval-next.ts`:
  - 实现 `runEvalNext(options)` 核心函数，以及可测试的 `resolveNextPhase` 纯函数
  - 服务端逻辑:
    1. 读取 eval.json（处理空数组/不存在的情况）
    2. 计算 round 计数: eval.json 条目总数 + 1
    3. 若 round > 20: return error `"round_limit_exceeded"`
    4. 检查最新条目的 `backtrack_to`: 若非空，清空目标 phase 及后续条目，返回目标 phase
    5. 获取最后执行 phase: 若不存在条目，返回第一个 phase；若存在，检查其 verdict
    6. 若 verdict=fail 且 attempt < 5: 返回同一 phase (retry)
    7. 若 verdict=fail 且 attempt >= 5: return error `"max_retries_exceeded"`
    8. 若 verdict=pass: 返回下一个未 pass phase
    9. 若全部 pass: return done: true
  - 定义每个 workflow_type 的 phase 表:
    - `requirement`: 01-proposal ~ 09-acceptance (9 phases)
    - `bug-fix`: 01-proposal, 02-dev-design, 05-implement, 06-unit-test, 07-code-review, 09-acceptance
    - `refactor`: 01-proposal ~ 09-acceptance (9 phases)
  - 每个 phase 的 agent_type 和 prompt 根据 phase 标识符映射:
    - 01-proposal: planner=proposal-planner, evaluator=proposal-evaluator
    - 02-dev-design: planner=dev-design-planner, evaluator=dev-design-evaluator
    - ... 后续 phase 沿用现有映射
    - EVAL-ONLY phases (07-code-review, 09-acceptance): planner=null
- [x] 在 `plugins/dev-team/bin/src/mcp.ts` 中注册 eval/next 工具:
  - import `runEvalNext` 和 `evalNextInputSchema`、`evalNextOutputSchema`
  - `server.registerTool('eval/next', {...}, handler)`
- [x] 创建 `plugins/dev-team/bin/src/commands/eval-next.test.ts`:
  - 测试: 初始状态返回 phase 01-proposal
  - 测试: phase pass 后返回下一 phase
  - 测试: fail 后重试同一 phase
  - 测试: 5 次失败后返回 max_retries_exceeded
  - 测试: backtrack 清空后续条目并返回目标 phase
  - 测试: 所有 phase pass 后返回 done: true
  - 测试: round > 20 返回 round_limit_exceeded
  - 测试: 中断恢复（有 planner 无 evaluator 条目）
  - 测试: workflow_type bug-fix 跳过 test-design/test-gen/integration-test

## 阶段 5: 新增 phase-proposal skill

- [x] 创建 `plugins/dev-team/skills/phase-proposal/SKILL.md`:
  - 元数据: `name: phase-proposal`, `disable-model-invocation: true`
  - 与 phase-dev-design 保持一致的 P→E 步骤结构
  - Step 1: 解析 change name（复用 phase-requirements 的输入分类逻辑）
  - Step 2: Gate check: `eval/check` 使用 phase `"01-proposal"`
  - Step 3: Planner: Agent(`proposal-planner`)
  - Step 4: Evaluator: Agent(`proposal-evaluator`)
  - Step 5: Verdict 循环
  - Step 6: 输出报告

## 阶段 6: 删除 phase-requirements skill

- [x] 删除 `plugins/dev-team/skills/phase-requirements/` 目录（含 SKILL.md）

## 阶段 7: 新增 workflow-requirement skill

- [x] 创建 `plugins/dev-team/skills/workflow-requirement/SKILL.md`:
  - 元数据: `name: workflow-requirement`, `disable-model-invocation: true`
  - **不包含任何硬编码 phase 表**
  - **不引用任何具体 agent name**
  - Step 0: 解析 change name，检测 explore 上下文
  - Step 1: 若 change 目录不存在，scaffold
  - Step 2: 编排循环（eval/next → Agent → ...）
  - Step 3: 完成处理，通知用户手动 archive

## 阶段 8: 更新文档和清理

- [x] 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（2.4.12 → 2.5.0）
- [x] 确认无残留引用 `requirements-evaluator`（全部替换为 `proposal-evaluator`）
- [x] 确认无残留引用 `phase-requirements`（全部替换为 `phase-proposal`）
- [x] 运行全部测试确保通过:
  - 139 tests passed across 6 test files
  - Build: dev-team-mcp.cjs regenerated successfully

## 阶段 9: 代码审查修复（disallowedTools 安全加固规范化）

- [x] 修复 `plugins/dev-team/agents/implementation-generator.md`：移除 `disallowedTools: Write, Edit`——generator 必须保留 Write/Edit 工具权限以写入代码
- [x] 验证 `plugins/dev-team/agents/dev-design-evaluator.md` 存在 `disallowedTools: Write, Edit`
- [x] 验证 `plugins/dev-team/agents/test-design-evaluator.md` 存在 `disallowedTools: Write, Edit`
- [x] 验证 `plugins/dev-team/agents/test-gen-evaluator.md` 存在 `disallowedTools: Write, Edit`
- [x] 验证 `plugins/dev-team/agents/test-gen-generator.md` **不包含** `disallowedTools: Write, Edit`（generator，保留写权限）
- [x] 运行全部测试确保修复未引入回归: 139 tests passed across 6 test files
