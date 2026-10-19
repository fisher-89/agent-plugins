# desktop-workflow-write-face Specification (Delta)

## MODIFIED Requirements

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` | `git` 四类封闭集，经 `StepNode::group()` 派生）、`role`（agent 步角色）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，非 agent 步可缺省）、`on_fail` 语义三变体（`FeedbackLoop { limit }` 定向反馈循环、`Terminal` 终态失败停给用户、`Escalate` 零会话落账升格交决策）；
- `prerequisites: &'static [&'static str]`：相位直接前置依赖（表内自描述，stale 传播与 allowed_backtrack 推导的唯一事实源；MUST NOT 存在第二份独立 match）；
- `backtrackable: bool`：相位回溯资格（表内自描述，沿 `prerequisites` 先例；`allowed_backtrack_phases` SHALL 按本属性过滤——`backtrackable=false` 的相位 MUST NOT 出现在任何相位的回溯白名单中，表头 initialize 恒 false（基线检查不可作为回溯目标），过滤 MUST NOT 以「跳过 idx==0」类位置推导实现——表结构变化即漂移）；白名单 SHALL 另按「可重做」裁定自身：`allowed_backtrack_phases` 中「当前相位自身在列」⟺ 该相位可重做（其 `steps` 含 `role()` 为 `Some` 的 agent 步）；零 agent 步的相位（test-execution / archive）自身 MUST NOT 入白名单——决策 agent 上下文里不存在「重做本相位」这个选项，判断 MUST NOT 以相位名硬编码；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；archive 相位 SHALL 无 `artifact_manifest`（不产文档）；requirement 表 test-design SHALL 声明 `markdown-doc·test-design.md`、implement SHALL 声明 `tasks-progress·tasks.md`（实现期进度视图，服务 phase_next 下发面——发现面同路径按首命中去重，tasks.md 出列为 dev-design 的 markdown-doc 单候选）；test-only 表 code-analyze SHALL 声明 `markdown-doc·design.md`（源码分析）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：archive 与 initialize 相位 SHALL 为 `None` / `None`（确定性相位）。

`StepNode` 词汇 SHALL 为二十一值（`executor` / `evaluator` / `decision` / `static_check` / `test_execution` / `mutation_test` / `clean` / `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` / `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `cleanup` / `seal` / `finalize`），与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 全映射（`mutation_test`、`cleanup` 与 `clean` 线词入封闭集，18→19、19→20 与 20→21 均为白名单式扩展）；code-analyze 复用既有 `Executor` / `Evaluator` / `VerdictGate` / `PhaseStart` / `PhaseLog` 步词，MUST NOT 新增步词。`StepGroup` SHALL 保持四分类（`Exec` / `Agent` / `Gate` / `Git`），`mutation_test` 归 `Exec`、`cleanup` 与 `clean` 归 `Git`。两表表头 SHALL 声明共享 `INITIALIZE_STEPS`：`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`（一次性基线检查相位——Clean 为 vcs 直调基线恢复步、StaticCheck / TestExecution 步 `on_fail=Terminal`（失败即停，无反馈边——基线的债不属于本次变更）、`PhaseStart` / `Clean` / `PhaseLog` `on_fail=None`；零 `artifact_manifest`——基线报告树非文档素材，回查经节点 `output_ref`）。两表共享的 `TEST_EXECUTION_STEPS` SHALL 为 `PhaseStart → TestExecution → MutationTest → PhaseLog` 四步：TestExecution / MutationTest 两步 `on_fail=Escalate`（零 WorkerAgent 会话——首败即落账后回 `phase_next` 交决策面，见 desktop-test-execution「失败即升格交决策（零 agent 修复）」与 desktop-mutation-testing「失败升格交决策（报告指引式信息面）」），`PhaseStart` / `PhaseLog` `on_fail=None`。同一步词在不同表项语义不同（initialize 表项的 TestExecution 为 `Terminal`、test-execution 表项为 `Escalate`），语义由表项声明承载，MUST NOT 由相位名推导；`Escalate` 归属 SHALL 由表测试守卫（只允许出现在 TestExecution / MutationTest 步上——四个执行体只按 `on_fail` 分派，误声明会静默失效）。`REQUIREMENT_TABLE` 的 archive `PhaseDefinition` SHALL 声明 11 步序列：`PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog`（`ResolveConflicts` 为条件执行步，`Cleanup` 为尽力步——清理失败仅 step detail 警告、步恒 `passed`，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」；其余步 `on_fail=None`——步失败即 Terminal）。`TEST_ONLY_TABLE` SHALL 复用 requirement 同名 step 静态（proposal / test-design / test-gen / test-execution / archive），并新增 `CODE_ANALYZE_STEPS`：`PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog`（P→E，`on_fail=Terminal` 于 VerdictGate）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目（含 initialize、archive、test-execution 与 code-analyze）
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），initialize 相位 5 步在案（表头 `INITIALIZE_STEPS` 两表共享）且 `executor=None` / `evaluator=None` / `backtrackable=false`，archive 相位 11 步在案且 `executor=None` / `evaluator=None`，test-execution 4 步在案（TestExecution / MutationTest 两步 `on_fail=Escalate`），code-analyze 5 步在案（P→E）；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: StepNode 二十一值与四分类分组

- **WHEN** 审查 `StepNode` / `RunStepKind` / `StepGroup` 定义
- **THEN** 词汇二十一值全映射（9 个归档步词（含 `cleanup`）、`mutation_test` 与 initialize 基线步词 `clean` 在案，code-analyze 零新步词）；`StepGroup` 含 Git，commit / rebase / merge / cleanup / finalize / clean 归 Git、preflight / seal / mutation_test 归 Exec、spec_sync / resolve_conflicts 归 Agent

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

### Requirement: test-only 相位表与前置依赖

`TEST_ONLY_TABLE` SHALL 为 7 个路由相位，序固定：`initialize → proposal → code-analyze → test-design → test-gen → test-execution → archive`。每相位的前置依赖 SHALL 为：

| phase | prerequisites |
|-------|---------------|
| initialize | [] |
| proposal | [] |
| code-analyze | [proposal] |
| test-design | [proposal, code-analyze] |
| test-gen | [test-design] |
| test-execution | [test-gen] |
| archive | [] |

step 复用 SHALL 为：initialize 复用 requirement 表头 `INITIALIZE_STEPS`（两表共享同一 static），proposal / test-design / test-gen 复用 requirement 同名 `*_STEPS`（含同名 executor / evaluator prompt），test-execution 复用 `TEST_EXECUTION_STEPS`（确定性门禁 `[PhaseStart, TestExecution, MutationTest, PhaseLog]`——拆步：测试 / 覆盖率修绿后才跑突变，TestExecution / MutationTest 两步 `on_fail=Escalate`（零 agent、首败落账交决策），不照搬插件侧 Executor→E），archive 复用 `ARCHIVE_STEPS`，code-analyze 用新增 `CODE_ANALYZE_STEPS`。test-only 的 `proposal` 描述 SHALL 为「测试向提案」（description 字段语义，prompt 主体暂复用 requirement proposal prompt，是否拆分独立 prompt 留待看效果）。

#### Scenario: test-only 七相位序与复用

- **WHEN** 读取 `phase_table("test-only")` 的相位序与各相位 `steps`
- **THEN** 序恰为 initialize / proposal / code-analyze / test-design / test-gen / test-execution / archive；initialize / proposal / test-design / test-gen / test-execution / archive 的 `steps` 指针与 requirement 同名 static 一致，code-analyze 为 `CODE_ANALYZE_STEPS`

#### Scenario: 前置依赖表内自描述

- **WHEN** 读取 `TEST_ONLY_TABLE` 各 `PhaseDefinition.prerequisites`
- **THEN** initialize=[]、code-analyze=[proposal]、test-design=[proposal, code-analyze]、test-gen=[test-design]、test-execution=[test-gen]、proposal/archive=[]；`dependents(TEST_ONLY_TABLE, "proposal")` 含 code-analyze 与 test-design

### Requirement: backtrack 白名单二次校验与 stale 语义

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验：`backtrack_to` 不在 `allowed_backtrack_phases` 内 SHALL 被拒绝且不落任何账（越权决议损坏不了状态，状态库零变更）；校验通过 SHALL 落回跳记录、对被跳过相位的 PhaseRecord 打 stale 标记并传播（与回跳落账同一事务语义），`reason` 超过 500 字符 SHALL 被拒绝。`backtrack_to` 与发起相位相同（自环）SHALL 在白名单含该相位时正常受理——这是「重做本相位」的唯一载体（白名单自含规则见「相位定义有序 step 序列与 artifact 清单」）：落账语义为改写最新条目 `backtrack_to` + 目标相位下游 stale 传播，`phase_next` 随后以新 attempt 路由回该相位；白名单不含自身时（零 agent 步相位）自环 SHALL 被拒且状态库零变更。该口径为桌面写面自有裁定（插件侧 `pipeline-backtrack` 的「目标索引必须小于当前相位」约束不适用于桌面路径，插件零改动、两份 spec 各管其面）。

#### Scenario: 白名单内回跳落账

- **WHEN** 以 `backtrack_to="test-gen"`（白名单内）、合法 reason 调用写面 backtrack
- **THEN** 回跳记录落库、目标相位可被 `phase_next` 路由至、被跳过相位的 PhaseRecord 带 stale 标记

#### Scenario: 目标为自身即重做本相位

- **WHEN** 以 `backtrack_to` 等于发起相位（如 `phase="implement"` / `to="implement"`，白名单含自身）调用写面 backtrack
- **THEN** 回跳落库成功、`phase_next` 随后路由回 implement 的新 attempt（重做本相位）；以 `phase="test-execution"` / `to="test-execution"`（白名单不含自身）调用时显式失败且状态库零变更

#### Scenario: 越权目标被拒绝

- **WHEN** 以 `backtrack_to` 不在 `allowed_backtrack_phases` 内调用写面 backtrack
- **THEN** 调用显式失败（错误抵达调用方），状态库零变更（无新行、无 stale 翻转、active_phase 不动）

#### Scenario: reason 超长被拒绝

- **WHEN** 以超过 500 字符的 reason 调用写面 backtrack
- **THEN** 调用显式失败，状态库零变更

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/workflow/src/state.rs` | `OnFail` 变体面 | 增 `Escalate`（无载荷）；三变体 `FeedbackLoop { limit }` / `Terminal` / `Escalate`，与缺省 `None` 各自语义见 `StepDefinition.on_fail` doc |
| `crates/core/workflow/src/write/phase_table.rs`（`TEST_EXECUTION_STEPS`） | 步声明单源 | TestExecution / MutationTest 两步 `on_fail: Some(OnFail::Escalate)`（原 `FeedbackLoop { limit: 5 }`） |
| `crates/core/workflow/src/write/phase_table.rs`（`allowed_backtrack_phases`） | 白名单推导 | `redoable = phase.steps.iter().any(|step| step.node.role().is_some())`；`table[..=idx]` 过滤 `backtrackable` 后 `redoable \|\| phase.id != current_phase` 自排除 |
| `crates/core/workflow/src/write/backtrack.rs` | 白名单二次校验 | `to == phase` 在白名单内时受理（重做本相位），否则拒绝且零写 |
| `crates/core/workflow/src/write/phase_next.rs` | 路由与幂等闸 | `fail_decided`（decision_session_id 在案 ⇒ 不再分叉）语义注释改写；删 `eval_only` 派生，`PhaseNextError` 两变体去 `eval_only` 字段 |
| `crates/core/workflow/src/write/phase_table_test.rs` | 声明守卫 | `Escalate` 仅允许出现在 TestExecution / MutationTest 步；两步 `on_fail` 期望值断言 |
