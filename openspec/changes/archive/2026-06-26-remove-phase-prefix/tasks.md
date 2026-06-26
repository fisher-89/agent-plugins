# 任务: remove-phase-prefix

> **变更**: remove-phase-prefix

---

## 阶段一：核心数据模型变更

- [x] 修改 `plugins/dev-team/bin/src/lib/workflow.ts`：将所有 phase 表（`PHASE_REQUIREMENT`, `PHASE_BUG_FIX`, `PHASE_TEST_ONLY`）中 phase 定义的 `id` 字段去前缀（`01-proposal` → `proposal`，`02-dev-design` → `dev-design`，以此类推）
- [x] 修改 `plugins/dev-team/bin/src/lib/workflow.ts`：将所有 prerequisite 表（`PHASE_PREREQUISITES`, `PHASE_BUG_FIX_PREREQUISITES`, `PHASE_TEST_ONLY_PREREQUISITES`）中的键名和数组值同步去前缀

## 阶段二：Engine 层适配

- [x] 修改 `plugins/dev-team/bin/src/commands/phase-next.ts`：在 `interpolatePrompt()` 函数中增加 `<phase>` 占位符替换逻辑，将 phase ID 注入到 agent prompt
- [x] 验证 `plugins/dev-team/bin/src/commands/phase-log.ts`：确认 `options.phase` 和 `options.backtrack_to` 已动态使用（无硬编码 phase ID），无需修改

## 阶段三：Phase SKILL 门控与 Verdict 读取

- [x] 修改 `plugins/dev-team/skills/phase-proposal/SKILL.md`：gate check 中 `next_phase` 断言和 verdict 读取路径的 phase ID 去前缀
- [x] 修改 `plugins/dev-team/skills/phase-dev-design/SKILL.md`：gate check、backtrack 检测、verdict 读取中的 phase ID 去前缀
- [x] 修改 `plugins/dev-team/skills/phase-test-design/SKILL.md`：gate check 和 verdict 读取中的 phase ID 去前缀
- [x] 修改 `plugins/dev-team/skills/phase-test-gen/SKILL.md`：gate check 和 verdict 读取中的 phase ID 去前缀
- [x] 修改 `plugins/dev-team/skills/phase-implement/SKILL.md`：gate check 和 verdict 读取中的 phase ID 去前缀
- [x] 修改 `plugins/dev-team/skills/phase-unit-test/SKILL.md`：no-op 跳过 entry 和 verdict 读取中的 phase ID 去前缀
- [x] 修改 `plugins/dev-team/skills/phase-integration-test/SKILL.md`：no-op 跳过 entry 和 verdict 读取中的 phase ID 去前缀
- [x] 修改 `plugins/dev-team/skills/phase-code-review/SKILL.md`：gate check 和 backtrack 检测中的 phase ID 去前缀
- [x] 修改 `plugins/dev-team/skills/phase-acceptance/SKILL.md`：backtrack 检测中的 phase ID 去前缀

## 阶段四：Workflow SKILL 硬编码数字修复

- [x] 修改 `plugins/dev-team/skills/workflow-requirement/SKILL.md`：确认无硬编码 phase 序号（已完成 review 确认无 `0[0-9]-` 模式引用）
- [x] 修改 `plugins/dev-team/skills/workflow-test-only/SKILL.md`：将 completion 描述中的 `"Total phases: 6"` 替换为 `"{total_phases}"` 动态值；将 `"All six test-only phases"` 等硬编码数字改为 `"All {total_phases} test-only phases"`

## 阶段五：Evaluator Agent 参数去前缀

- [x] 修改 `plugins/dev-team/agents/proposal-evaluator.md`：`phase_log` 调用中的 `phase: "01-proposal"` → 改用 `<phase>` 占位符或 `"proposal"`
- [x] 修改 `plugins/dev-team/agents/dev-design-evaluator.md`：`phase_log` 调用中的 `phase: "02-dev-design"` 去前缀
- [x] 修改 `plugins/dev-team/agents/test-design-evaluator.md`：`phase_log` 调用中的 `phase: "03-test-design"` 去前缀
- [x] 修改 `plugins/dev-team/agents/test-gen-evaluator.md`：`phase_log` 调用中的 `phase: "04-test-gen"` 去前缀
- [x] 修改 `plugins/dev-team/agents/implementation-evaluator.md`：`phase_log` 调用中的 `phase: "05-implement"` 去前缀
- [x] 修改 `plugins/dev-team/agents/unit-test-evaluator.md`：`phase_log` 调用中的 `phase: "06-unit-test"` 去前缀；同步更新所有 backtrack 目标（`"04-test-gen"` → `"test-gen"`，`"05-implement"` → `"implement"`，`"03-test-design"` → `"test-design"`，`"02-dev-design"` → `"dev-design"`）
- [x] 修改 `plugins/dev-team/agents/integration-test-evaluator.md`：`phase_log` 调用中的 `phase: "08-integration-test"` 去前缀；同步更新所有 backtrack 目标（`"04-test-gen"` → `"test-gen"`，`"05-implement"` → `"implement"`，`"03-test-design"` → `"test-design"`，`"02-dev-design"` → `"dev-design"`，`"08-integration-test"` → `"integration-test"`）
- [x] 修改 `plugins/dev-team/agents/code-analyze-evaluator.md`：`phase_log` 调用中的 `phase: "02-code-analyze"` 去前缀
- [x] 修改 `plugins/dev-team/agents/code-review-evaluator.md`：`phase_log` 调用中的 `phase: "07-code-review"` 去前缀
- [x] 修改 `plugins/dev-team/agents/acceptance-evaluator.md`：`phase_log` 调用中的 `phase: "09-acceptance"` 去前缀

## 阶段六：Executor Agent 与 Archive SKILL

- [x] 修改 `plugins/dev-team/agents/unit-test-executor.md`：报告 JSON 示例中的 `"phase": "06-unit-test"` 去前缀（注意这是示例文档，实际值由 agent 运行时动态写入）
- [x] 修改 `plugins/dev-team/agents/integration-test-executor.md`：报告 JSON 示例中的 `"phase": "08-integration-test"` 去前缀
- [x] 修改 `plugins/dev-team/skills/openspec-archive-change/SKILL.md`：`phase_check` 调用中的 `phase="09-acceptance"` 去前缀

## 阶段七：Version Bump 与验证

- [x] 将 `plugins/dev-team/.claude-plugin/plugin.json` 中的 `version` 从 `2.7.7` 递增到 `2.7.8`
- [x] 运行 TypeScript 编译验证所有代码修改通过类型检查
- [x] 执行全局 grep 搜索 `0[0-9]-` 模式，确认 plugins/dev-team 目录下无残留的旧 phase ID 引用（排除 `node_modules`、`.git` 和 `eval.json` 存量数据）

## 阶段八：测试文件更新

- [x] 更新 `plugins/dev-team/bin/src/commands/phase-next.test.ts`：所有 phase ID 断言字符串去前缀（约 100+ 处）
- [x] 更新 `plugins/dev-team/bin/src/commands/phase-log.test.ts`：所有 phase ID 断言字符串和 error message 断言去前缀
- [x] 更新 `plugins/dev-team/bin/src/commands/change-list.test.ts`：所有 eval entry 中的 phase 字段去前缀
- [x] 更新 `plugins/dev-team/bin/src/lib/workflow.test.ts`：所有 phase ID 断言字符串去前缀
