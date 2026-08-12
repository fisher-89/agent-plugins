## 权威边界

Planner 模板与共享 eval/checklist schema 形状。Agent 命名以 `phase-skills` 为准；回溯写入以 `pipeline-backtrack` 为准（决议 C1 命名对齐、C3=A）。

## ADDED Requirements

### Requirement: Markdown templates for Planner artifacts

系统 SHALL 在 `templates/artifacts/` 提供建议模板（非强制结构）：

- `proposal.md.template` — Problem, Scope, Risks, Acceptance Criteria
- `test-design.md.template` — Test Levels, Coverage Map, Test Strategy, Boundary Cases
- `design.md.template` — Architecture Components, Change Inventory, Data Model, Route/API, Dependencies, Open Questions

Evaluators 检查内容质量，不强制章节顺序。

#### Scenario: proposal-planner 覆盖建议章节

- **WHEN** `proposal-planner` 写 proposal.md
- **THEN** 输出覆盖 Problem、Scope、Risks、Acceptance Criteria（可增删章节）

### Requirement: Shared JSON schemas for evaluation

系统 SHALL 定义共享 schema：

- `eval.schema.json`：`phase`（无前缀 ID，如 `"proposal"`）、`timestamp`、`attempt`、`verdict`、`report`、`items`、可选 `backtrack_to` / `backtrack_reason` / `stale`、`schema_version` 等。单文件 `eval.json` 数组，无 per-phase 文件。
- `checklist.schema.json`：`{id, criterion, required, evidence_hint}` 参考格式

**写入方**：新条目由 `phase_log` 追加时 MUST NOT 带 `backtrack_to`/`backtrack_reason`；这两字段仅由 `backtrack` 工具写入（schema 保留 optional 以解析旧文件与回溯后状态）。

#### Scenario: 条目符合 schema

- **WHEN** 任意 evaluator 经 `phase_log` 追加条目
- **THEN** 条目符合 `eval.schema.json`，且创建时无回溯字段

#### Scenario: eval.json 只追加

- **WHEN** 同 phase 在 fail 后再次评估
- **THEN** 新结果追加；旧条目保留

#### Scenario: 按 timestamp 取最新

- **WHEN** 校验或 `phase_next` 需要某 phase 最新结果
- **THEN** 按 `phase` 分组取最大 `timestamp`（并尊重 stale 规则，见引擎/回溯规格）
