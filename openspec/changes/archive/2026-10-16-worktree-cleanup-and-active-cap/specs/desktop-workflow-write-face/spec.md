# desktop-workflow-write-face Specification (Delta)

## MODIFIED Requirements

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` | `git` 四类封闭集，经 `StepNode::group()` 派生）、`role`（agent 步角色）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，非 agent 步可缺省）、`on_fail` 语义（feedback 循环 / 终态 / 升格相位 fail）；
- `prerequisites: &'static [&'static str]`：相位直接前置依赖（表内自描述，stale 传播与 allowed_backtrack 推导的唯一事实源；MUST NOT 存在第二份独立 match）；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；archive 相位 SHALL 无 `artifact_manifest`（不产文档）；requirement 表 test-design SHALL 声明 `markdown-doc·test-design.md`、implement SHALL 声明 `tasks-progress·tasks.md`（实现期进度视图，服务 phase_next 下发面——发现面同路径按首命中去重，tasks.md 出列为 dev-design 的 markdown-doc 单候选）；test-only 表 code-analyze SHALL 声明 `markdown-doc·design.md`（源码分析）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：archive 相位 SHALL 为 `None` / `None`（确定性相位）。

`StepNode` 词汇 SHALL 为二十值（`executor` / `evaluator` / `decision` / `static_check` / `test_execution` / `mutation_test` / `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` / `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `cleanup` / `seal` / `finalize`），与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 全映射（`mutation_test` 与 `cleanup` 线词入封闭集，18→19 与 19→20 均为白名单式扩展）；code-analyze 复用既有 `Executor` / `Evaluator` / `VerdictGate` / `PhaseStart` / `PhaseLog` 步词，MUST NOT 新增步词。`StepGroup` SHALL 保持四分类（`Exec` / `Agent` / `Gate` / `Git`），`mutation_test` 归 `Exec`、`cleanup` 归 `Git`。`REQUIREMENT_TABLE` 的 archive `PhaseDefinition` SHALL 声明 11 步序列：`PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog`（`ResolveConflicts` 为条件执行步，`Cleanup` 为尽力步——清理失败仅 step detail 警告、步恒 `passed`，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」；其余步 `on_fail=None`——步失败即 Terminal）。`TEST_ONLY_TABLE` SHALL 复用 requirement 同名 step 静态（proposal / test-design / test-gen / test-execution / archive），并新增 `CODE_ANALYZE_STEPS`：`PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog`（P→E，`on_fail=Terminal` 于 VerdictGate）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目（含 archive 与 code-analyze）
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），archive 相位 11 步在案且 `executor=None` / `evaluator=None`，code-analyze 5 步在案（P→E）；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: StepNode 二十值与四分类分组

- **WHEN** 审查 `StepNode` / `RunStepKind` / `StepGroup` 定义
- **THEN** 词汇二十值全映射（9 个归档步词（含 `cleanup`）与 `mutation_test` 在案，code-analyze 零新步词）；`StepGroup` 含 Git，commit / rebase / merge / cleanup / finalize 归 Git、preflight / seal / mutation_test 归 Exec、spec_sync / resolve_conflicts 归 Agent

#### Scenario: artifact 清单权威归属

- **WHEN** 查阅 requirement 与 test-only 工作流各相位的 `artifact_manifest`
- **THEN** proposal.md / specs/** 属 proposal、design.md / tasks.md 属 dev-design、test-design.md 属 test-design、test-reports/** 属 test-execution（以清单声明为准）；implement 声明 `tasks-progress·tasks.md`（下发面，发现面同路径首命中去重不重复出列）；test-only 下 code-analyze 声明 design.md、test-design 声明 test-design.md；archive 相位零 artifact_manifest；清单之外文档不挂 phase 列

#### Scenario: PhaseAgentSpec 静态化不翻案

- **WHEN** 审查 `PhaseDefinition` 扩展后的 executor / evaluator spec
- **THEN** 仍为 `{ prompt: &'static str, model_level }`（`include_str!` 产物），无 `agent_type` / `__CALL_AGENT__` / `interpolate` 残留；archive 相位 executor / evaluator 均 None

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/state.rs`（StepNode / RunStepKind） | 二十值词面单点 | `Cleanup` / `cleanup` 白名单式入封闭集（19→20）；`StepNode::group()` 将 cleanup 派生 `StepGroup::Git`；与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 |
| `crates/core/workflow/src/write/phase_table.rs`（ARCHIVE_STEPS） | 11 步序列静态 | SpecSync 与 Seal 之间插 `Cleanup`（`prompt_ref=None`、尽力步语义驻 walker 执行体）；`ARCHIVE_STEP_ORDER` 断言 10 → 11 |
