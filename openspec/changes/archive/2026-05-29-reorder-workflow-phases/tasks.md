# 实施任务: reorder-workflow-phases

> **变更**: reorder-workflow-phases
> **日期**: 2026-05-28

---

## 阶段一: 核心引擎变更

这些任务是所有其他任务的基础——`workflow.ts` 中的 PHASES 数组是阶段标识符的权威来源。

- [x] **T1.1**: 修改 `plugins/dev-team/bin/src/lib/workflow.ts` 中的 PHASES 数组，将 `"02-test-design"` 和 `"03-dev-proposal"` 替换为 `"02-dev-design"` 和 `"03-test-design"`，保持其他阶段不变
- [x] **T1.2**: 更新 `plugins/dev-team/bin/src/lib/workflow.test.ts` 中的预期值，反映新的 PHASES 顺序和重命名后的阶段标识符
  - `"02-test-design"` → `"02-dev-design"`
  - `"03-dev-proposal"` → `"03-test-design"`
  - 更新 `getPriorPhases` 测试用例中的前序阶段列表
- [x] **T1.3**: 运行单元测试验证 `workflow.test.ts` 通过

## 阶段二: Skill 目录重命名

- [x] **T2.1**: 将 `plugins/dev-team/skills/phase-dev-proposal/` 目录重命名为 `plugins/dev-team/skills/phase-dev-design/`
- [x] **T2.2**: 更新 `plugins/dev-team/skills/phase-dev-design/SKILL.md`:
  - `name: phase-dev-proposal` → `phase-dev-design`
  - `description` 中的 `dev-proposal-planner` → `dev-design-planner`, `dev-proposal-evaluator` → `dev-design-evaluator`
  - gate check phase: `"03-dev-proposal"` → `"02-dev-design"`
  - 用法命令: `phase-dev-proposal` → `phase-dev-design`
  - backtrack_to: `"03-dev-proposal"` → `"02-dev-design"`
  - subagent_type: `dev-team:dev-proposal-planner` → `dev-team:dev-design-planner`, `dev-team:dev-proposal-evaluator` → `dev-team:dev-design-evaluator`
  - eval-log phase 标识符 (verdict 读取): `"03-dev-proposal"` → `"02-dev-design"`
- [x] **T2.3**: 更新 `plugins/dev-team/skills/phase-test-design/SKILL.md`:
  - gate check phase: `"02-test-design"` → `"03-test-design"`
  - eval-log phase 标识符 (verdict 读取): `"02-test-design"` → `"03-test-design"`

## 阶段三: Agent 文件重命名和内容更新

- [x] **T3.1**: 将 `plugins/dev-team/agents/dev-proposal-planner.md` 重命名为 `plugins/dev-team/agents/dev-design-planner.md`
  - 更新 frontmatter `name: dev-proposal-planner` → `dev-design-planner`
  - 更新 `description` 中的 `dev-proposal-planner` → `dev-design-planner`, `phase-dev-proposal` → `phase-dev-design`
  - **移除** Input 中的 `test-design.md`（此时尚未生成）
  - 更新 Process 描述中的引用
- [x] **T3.2**: 将 `plugins/dev-team/agents/dev-proposal-evaluator.md` 重命名为 `plugins/dev-team/agents/dev-design-evaluator.md`
  - 更新 frontmatter `name: dev-proposal-evaluator` → `dev-design-evaluator`
  - 更新 `description` 中的 `dev-proposal-planner` → `dev-design-planner`, `dev-proposal-evaluator` → `dev-design-evaluator`, `phase-dev-proposal` → `phase-dev-design`
  - eval-log phase: `"03-dev-proposal"` → `"02-dev-design"`
  - 更新所有 checklist 和 Output 中的阶段引用
- [x] **T3.3**: 更新 `plugins/dev-team/agents/test-design-planner.md`:
  - **新增** Input 中读取 `design.md`（在 `proposal.md` 之后）
  - 更新 Process 描述，说明现在可访问 design.md 中的架构信息
- [x] **T3.4**: 更新 `plugins/dev-team/agents/test-design-evaluator.md`:
  - eval-log phase: `"02-test-design"` → `"03-test-design"`

## 阶段四: Backtrack 引用更新

- [x] **T4.1**: 更新 `plugins/dev-team/agents/code-review-evaluator.md`:
  - `backtrack_to` 引用: `"03-dev-proposal"` → `"02-dev-design"`
  - `description` 中的 `"03-dev-proposal"` → `"02-dev-design"`
  - Output 示例中的 backtrack_to 值
- [x] **T4.2**: 更新 `plugins/dev-team/agents/unit-test-evaluator.md`:
  - `backtrack_to "03-dev-proposal"` → `"02-dev-design"`
  - `backtrack_to "02-test-design"` → `"03-test-design"`
  - 诊断决策树中的阶段描述文字
- [x] **T4.3**: 更新 `plugins/dev-team/agents/integration-test-evaluator.md`:
  - `backtrack_to "03-dev-proposal"` → `"02-dev-design"`
  - `backtrack_to "02-test-design"` → `"03-test-design"`
  - 诊断决策树中的阶段描述文字
- [x] **T4.4**: 更新 `plugins/dev-team/skills/phase-unit-test/SKILL.md`:
  - backtrack 注释: `dev-proposal` → `dev-design`
- [x] **T4.5**: 更新 `plugins/dev-team/skills/phase-integration-test/SKILL.md`:
  - backtrack 注释: `dev-proposal` → `dev-design`
- [x] **T4.6**: 更新 `plugins/dev-team/skills/phase-code-review/SKILL.md`:
  - `backtrack_to "03-dev-proposal"` → `"02-dev-design"`
  - `/dev-team:phase-dev-proposal` → `/dev-team:phase-dev-design`
  - description 中的 `"03-dev-proposal"` → `"02-dev-design"`
- [x] **T4.7**: 更新 `plugins/dev-team/skills/openspec-propose/SKILL.md`:
  - 废弃提示中的 `phase-dev-proposal` → `phase-dev-design`

## 阶段五: MCP 工具定义和构建

- [x] **T5.1**: 更新 `plugins/dev-team/bin/src/mcp.ts` 中 `eval_check` 工具 description 的示例阶段: `"03-dev-proposal"` → `"02-dev-design"`
- [x] **T5.2**: 重建 `plugins/dev-team/bin/dev-team-mcp.cjs`（运行构建脚本，使 mcp.ts 的变更同步到编译后的 CJS 文件）

## 阶段六: Spec 文件更新

- [x] **T6.1**: 更新 `openspec/specs/pge-workflow-engine/spec.md`:
  - 阶段顺序表: 02-test-design → 02-dev-design, 03-dev-proposal → 03-test-design
- [x] **T6.2**: 更新 `openspec/specs/phase-skills/spec.md`:
  - 技能列表: `phase-dev-proposal` → `phase-dev-design`
  - SKILL 名称引用: `dev-proposal-planner` → `dev-design-planner`, `dev-proposal-evaluator` → `dev-design-evaluator`
  - prompt 模板表: phase/dev-proposal 引用
- [x] **T6.3**: 更新 `openspec/specs/phase-agents/spec.md`:
  - agent 名称: `dev-proposal-planner` → `dev-design-planner`, `dev-proposal-evaluator` → `dev-design-evaluator`
  - subagent 表: 两处 agent 名称更新
- [x] **T6.4**: 更新 `openspec/specs/pipeline-backtrack/spec.md`:
  - 所有 `"03-dev-proposal"` → `"02-dev-design"`
  - 所有 `"02-test-design"` → `"03-test-design"`
  - 回溯顺序链: `test-design < dev-proposal < ...` → `dev-design < test-design < ...`
- [x] **T6.5**: 更新 `openspec/specs/eval-check-cli/spec.md`:
  - 阶段顺序列表: `02-test-design, 03-dev-proposal` → `02-dev-design, 03-test-design`
- [x] **T6.6**: 更新 `openspec/specs/embedded-cli/spec.md`:
  - 命令示例中的 `--phase 03-dev-proposal` → `--phase 02-dev-design`
- [x] **T6.7**: 更新 `openspec/specs/intent-router/spec.md`:
  - `phase-dev-proposal` → `phase-dev-design`, `dev-proposal-planner` → `dev-design-planner`, `dev-proposal-evaluator` → `dev-design-evaluator`
- [x] **T6.8**: 更新 `openspec/specs/test-execution-diagnostics/spec.md`:
  - `"03-dev-proposal"` → `"02-dev-design"`
  - `"02-test-design"` → `"03-test-design"`
  - 回溯目标描述文字: `dev-proposal` → `dev-design`, test-design 阶段编号更新

## 阶段七: 项目配置

- [x] **T7.1**: 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（从 `2.4.9` 升到 `2.5.0`，因为包含 breaking change——阶段标识符变更）

## 阶段八: 验证

- [x] **T8.1**: 执行 grep 全量搜索 `dev-proposal`（排除 `openspec/changes/archive/` 和当前变更目录），确认无旧标识符残留
- [x] **T8.2**: 执行 grep 全量搜索 `03-dev-proposal`（排除 `openspec/changes/archive/` 和当前变更目录），确认无旧标识符残留
- [x] **T8.3**: 执行 grep 全量搜索 `02-test-design`（排除 `openspec/changes/archive/` 和当前变更目录），确认无旧标识符残留
- [x] **T8.4**: 运行 `workflow.test.ts` 单元测试，确认全部通过
- [x] **T8.5**: 验证 `skills/phase-dev-design/SKILL.md` gate check 使用 `02-dev-design`
- [x] **T8.6**: 验证 `skills/phase-test-design/SKILL.md` gate check 使用 `03-test-design`
- [x] **T8.7**: 验证旧目录 `skills/phase-dev-proposal/` 已不存在，旧 agent 文件 `dev-proposal-planner.md` 和 `dev-proposal-evaluator.md` 已不存在
- [x] **T8.8**: 验证 `dev-design-planner.md` 不读取 `test-design.md`，`test-design-planner.md` 读取 `design.md`
