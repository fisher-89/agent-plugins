# desktop-workflow-write-face Specification（delta: archi-check-and-gate）

## MODIFIED Requirements

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` | `git` 四类封闭集，经 `StepNode::group()` 派生）、`role`（agent 步角色）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，非 agent 步可缺省）、`on_fail` 语义三变体（`FeedbackLoop { limit }` 定向反馈循环、`Terminal` 终态失败停给用户、`Escalate` 零会话落账升格交决策）；
- `prerequisites: &'static [&'static str]`：相位直接前置依赖（表内自描述，stale 传播与 allowed_backtrack 推导的唯一事实源；MUST NOT 存在第二份独立 match）；
- `backtrackable: bool`：相位回溯资格（表内自描述，沿 `prerequisites` 先例；`allowed_backtrack_phases` SHALL 按本属性过滤——`backtrackable=false` 的相位 MUST NOT 出现在任何相位的回溯白名单中，表头 initialize 恒 false（基线检查不可作为回溯目标），过滤 MUST NOT 以「跳过 idx==0」类位置推导实现——表结构变化即漂移）；白名单 SHALL 另按「可重做」裁定自身：`allowed_backtrack_phases` 中「当前相位自身在列」⟺ 该相位可重做（其 `steps` 含 `role()` 为 `Some` 的 agent 步）；零 agent 步的相位（test-execution / archive）自身 MUST NOT 入白名单——决策 agent 上下文里不存在「重做本相位」这个选项，判断 MUST NOT 以相位名硬编码；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；archive 相位 SHALL 无 `artifact_manifest`（不产文档）；requirement 表 test-design SHALL 声明 `markdown-doc·test-design.md`、implement SHALL 声明 `tasks-progress·tasks.md`（实现期进度视图，服务 phase_next 下发面——发现面同路径按首命中去重，tasks.md 出列为 dev-design 的 markdown-doc 单候选）；test-only 表 code-analyze SHALL 声明 `markdown-doc·design.md`（源码分析）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：archive 与 initialize 相位 SHALL 为 `None` / `None`（确定性相位）。

`StepNode` 词汇 SHALL 为二十二值（`executor` / `evaluator` / `decision` / `static_check` / `test_execution` / `mutation_test` / `archi_check` / `clean` / `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` / `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `cleanup` / `seal` / `finalize`），与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 全映射（`mutation_test`、`cleanup`、`clean` 与 `archi_check` 线词入封闭集，18→19、19→20、20→21 与 21→22 均为白名单式扩展）；code-analyze 复用既有 `Executor` / `Evaluator` / `VerdictGate` / `PhaseStart` / `PhaseLog` 步词，MUST NOT 新增步词。`StepGroup` SHALL 保持四分类（`Exec` / `Agent` / `Gate` / `Git`），`mutation_test` 与 `archi_check` 归 `Exec`、`cleanup` 与 `clean` 归 `Git`。两表表头 SHALL 声明共享 `INITIALIZE_STEPS`：`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`（一次性基线检查相位——Clean 为 vcs 直调基线恢复步、StaticCheck / TestExecution 步 `on_fail=Terminal`（失败即停，无反馈边——基线的债不属于本次变更）、`PhaseStart` / `Clean` / `PhaseLog` `on_fail=None`；零 `artifact_manifest`——基线报告树非文档素材，回查经节点 `output_ref`）。两表共享的 `TEST_EXECUTION_STEPS` SHALL 为 `PhaseStart → TestExecution → MutationTest → PhaseLog` 四步：TestExecution / MutationTest 两步 `on_fail=Escalate`（零 WorkerAgent 会话——首败即落账后回 `phase_next` 交决策面，见 desktop-test-execution「失败即升格交决策（零 agent 修复）」与 desktop-mutation-testing「失败升格交决策（报告指引式信息面）」），`PhaseStart` / `PhaseLog` `on_fail=None`。requirement 表的 implement `PhaseDefinition` SHALL 声明 7 步序列：`PhaseStart → Executor → StaticCheck(FeedbackLoop{5}) → ArchiCheck(FeedbackLoop{3}) → Evaluator → VerdictGate(FeedbackLoop{5}) → PhaseLog`——`ArchiCheck` 步 MUST 位于 static-check 通过之后、evaluator 之前（先修机器能判的，再让评估面看已过门禁的产物），其 `on_fail` SHALL 为 `FeedbackLoop { limit: 3 }`（架构违例的反馈边上限独立于 static-check 的 5 次与 verdict 的 5 次，三段计数互不占用；见 desktop-change-orchestration「ArchiCheck 步接线与反馈边」）。同一步词在不同表项语义不同（initialize 表项的 TestExecution 为 `Terminal`、test-execution 表项为 `Escalate`），语义由表项声明承载，MUST NOT 由相位名推导；`Escalate` 归属 SHALL 由表测试守卫（只允许出现在 TestExecution / MutationTest 步上——四个执行体只按 `on_fail` 分派，误声明会静默失效）。`REQUIREMENT_TABLE` 的 archive `PhaseDefinition` SHALL 声明 11 步序列：`PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog`（`ResolveConflicts` 为条件执行步，`Cleanup` 为尽力步——清理失败仅 step detail 警告、步恒 `passed`，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」；其余步 `on_fail=None`——步失败即 Terminal）。`TEST_ONLY_TABLE` SHALL 复用 requirement 同名 step 静态（proposal / test-design / test-gen / test-execution / archive），并新增 `CODE_ANALYZE_STEPS`：`PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog`（P→E，`on_fail=Terminal` 于 VerdictGate）；`ArchiCheck` 步 MUST NOT 出现在 `TEST_ONLY_TABLE` 的任何相位（该表无 implement 相位，代码改动落在 code-analyze；且测试文件被架构扫描全排除，挂上只会空转）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目（含 initialize、implement、archive、test-execution 与 code-analyze）
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），initialize 相位 5 步在案（表头 `INITIALIZE_STEPS` 两表共享）且 `executor=None` / `evaluator=None` / `backtrackable=false`，implement 相位 7 步在案（`ArchiCheck` 位于 StaticCheck 与 Evaluator 之间、`on_fail=FeedbackLoop{3}`），archive 相位 11 步在案且 `executor=None` / `evaluator=None`，test-execution 4 步在案（TestExecution / MutationTest 两步 `on_fail=Escalate`），code-analyze 5 步在案（P→E）；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: StepNode 二十二值与四分类分组

- **WHEN** 审查 `StepNode` / `RunStepKind` / `StepGroup` 定义
- **THEN** 词汇二十二值全映射（9 个归档步词（含 `cleanup`）、`mutation_test`、`archi_check` 与 initialize 基线步词 `clean` 在案，code-analyze 零新步词）；`StepGroup` 含 Git，commit / rebase / merge / cleanup / finalize / clean 归 Git、preflight / seal / mutation_test / archi_check 归 Exec、spec_sync / resolve_conflicts 归 Agent

#### Scenario: 架构门禁步只在 implement

- **WHEN** 遍历 requirement 表与 test-only 表全部相位的 `steps`
- **THEN** `ArchiCheck` 步恰在 requirement 表的 implement 相位出现一次（序在其 StaticCheck 之后、Evaluator 之前）；acceptance / initialize / archive / test-gen / test-execution / test-only 全表零 `ArchiCheck` 步

#### Scenario: 回溯白名单按 backtrackable 过滤

- **WHEN** 对两表任一非表头相位调用 `allowed_backtrack_phases`
- **THEN** 白名单为其前序相位（含自身）中 `backtrackable=true` 者的集合，initialize 恒不在白名单（`backtrackable=false`）；过滤依据为表内显式属性，无「跳过 idx==0」位置推导；自身是否在列另由「可重做」裁定（见下条 scenario）

#### Scenario: 白名单自含即重做、零 agent 步自排除

- **WHEN** 分别对 implement 与 test-execution 调用 `allowed_backtrack_phases`
- **THEN** implement 的白名单含 implement 自身（`steps` 含 agent 步 ⇒ 可重做，「重做本相位」经回溯到自身表达）；test-execution 的白名单不含自身（`steps` 全为确定性步 ⇒ 不可重做，决策上下文无「重做本相位」选项），其余前序 `backtrackable` 相位照常在列

#### Scenario: artifact 清单权威归属

- **WHEN** 查阅 requirement 与 test-only 工作流各相位的 `artifact_manifest`
- **THEN** proposal.md / specs/** 属 proposal、design.md / tasks.md 属 dev-design、test-design.md 属 test-design、test-reports/** 属 test-execution（以清单声明为准）；implement 声明 `tasks-progress·tasks.md`（下发面，发现面同路径首命中去重不重复出列）；test-only 下 code-analyze 声明 design.md、test-design 声明 test-design.md；initialize 与 archive 相位零 artifact_manifest；清单之外文档不挂 phase 列

#### Scenario: PhaseAgentSpec 静态化不翻案

- **WHEN** 审查 `PhaseDefinition` 扩展后的 executor / evaluator spec
- **THEN** 仍为 `{ prompt: &'static str, model_level }`（`include_str!` 产物），无 `agent_type` / `__CALL_AGENT__` / `interpolate` 残留；archive 与 initialize 相位 executor / evaluator 均 None

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/state.rs`（`StepNode` / `RunStepKind` / `ChangeStepKind`） | 词面单点 | `ArchiCheck` / `archi_check` 白名单式入封闭集（21→22，三枚举同词面 1:1）；`StepNode::group()` 派生 `StepGroup::Exec`、`role()` 为 `None` |
| `crates/core/workflow/src/write/phase_table.rs`（`IMPLEMENT_STEPS`） | 计划图声明 | 7 步：`PhaseStart → Executor → StaticCheck{5} → ArchiCheck{3} → Evaluator → VerdictGate{5} → PhaseLog`；`ArchiCheck` 唯一的步词出处 |
| `crates/core/workflow/src/write/phase_table.rs`（`TEST_ONLY_TABLE`） | 表项守卫 | 零 `ArchiCheck` 步（该表无 implement 相位）；表测试断言两条表全相位扫描 |
