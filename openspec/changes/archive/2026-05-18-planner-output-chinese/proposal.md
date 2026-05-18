## Why

3 个 Planner agent 生成的 `.md` 制品（proposal.md、test-design.md、design.md、tasks.md）目前全部为英文，而团队的工作语言为中文。这些制品是设计评审、需求确认、任务执行的核心文档——使用英文增加了沟通成本和阅读负担。将所有 Planner 输出切换为中文，使制品可直接用于团队协作，无需额外翻译。

## What Changes

- 在 3 个 Planner agent 的 prompt 中增加输出语言约束（中文），同时保留代码标识符、命令、文件路径等为英文
- 将 `templates/artifacts/` 下 3 个模板的章节标题和占位符标签翻译为中文
- 更新 `phase-agents` spec：增加 Planner 输出语言为中文的 requirement

## Capabilities

### New Capabilities
<!-- None -- this change modifies existing planner behavior, no new capability introduced -->

### Modified Capabilities
- `phase-agents`: 新增 Requirement —— Planner agent 必须使用中文输出 .md 制品文件

## Impact

- **Agent 定义文件**: `plugins/dev-team/agents/requirements-planner.md`、`test-design-planner.md`、`dev-proposal-planner.md` —— 增加中文输出约束
- **模板文件**: `plugins/dev-team/templates/artifacts/proposal.md.template`、`test-design.md.template`、`design.md.template` —— 章节标题汉化
- **Spec 文件**: `openspec/specs/phase-agents/spec.md` —— 新增中文输出 requirement
- **Evaluator agent**: 3 个 DESIGN Evaluator（E1-E3）的 checklist 可能需要调整为中文语境（可选，后续评估）
