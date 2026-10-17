# desktop-workflow-write-face Specification (Delta)

## MODIFIED Requirements

### Requirement: 相位表与 prompt 模板单源收敛

工作流相位表（相位序、executor / evaluator prompt 模板、**有序 step 序列、artifact 清单、前置依赖、回溯资格**）SHALL 单源驻 core/workflow：prompt 模板 SHALL 为 `core/workflow/src/prompts/` 全静态角色知识 md，经 `phase_table.rs` 编译期 `include_str!("prompts/<role>.md")` 显式字面路径装配，MUST NOT 按 `<role>` 动态读取；step 序列与 artifact 清单 SHALL 以 `PhaseDefinition.steps` / `PhaseDefinition.artifact_manifest` 编译期 const 表承载，MUST NOT 落库、MUST NOT 经 port 缝运行时加载。`phase_table(workflow_type)` SHALL 按类型分派：`"requirement"` → `REQUIREMENT_TABLE`（10 个路由相位：initialize / proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / archive）、`"test-only"` → `TEST_ONLY_TABLE`（7 个路由相位：initialize / proposal / code-analyze / test-design / test-gen / test-execution / archive），其余（含空白 / 未知 / bug-fix）→ `None`（**不退化**）。initialize 为两表共有的**表头一次性基线检查相位**（确定性相位：`executor=None` / `evaluator=None`，`INITIALIZE_STEPS` 见「相位定义有序 step 序列与 artifact 清单」，执行语义见 desktop-change-orchestration「initialize 基线检查相位执行体」）；archive 为两表共有的确定性末相位（`PhaseDefinition.executor=None` / `evaluator=None`，11 步序列见「相位定义有序 step 序列与 artifact 清单」）。walker MUST NOT 自持相位转移规则（每步过渡问写面 `phase_next`）；`phase_next` SHALL 覆盖各表末相位（requirement 9 个前序路由相位（initialize + 8）pass 后 next=archive，test-only 6 个前序路由相位（initialize + 5）pass 后 next=archive，archive 空 checklist 落账后 done）。`detail.rs` 相位列保持纯布局身份、列序随 `phase_table` 出线（见 desktop-change-queries），MUST NOT 上位为路由权威；TS 侧相位表冻结，不再是桌面路径依赖。

#### Scenario: walker 不自持转移规则

- **WHEN** 审查 orchestration walker 源码对相位先后关系的编码
- **THEN** 零硬编码相位转移表；walker 每步过渡均以写面 `phase_next` 的返回为准（含 initialize 表头路由与 archive → done）

#### Scenario: 相位表单一来源

- **WHEN** 同一 workflow_type 的相位序、step 序列、artifact 清单与前置依赖在 core/workflow 与 walker 两侧分别查阅
- **THEN** 仅 core/workflow 相位表定义相位序、step 序列、artifact 清单与前置依赖（requirement 10 相位、test-only 7 相位均在案，两表表头均为一次性 initialize）；walker 侧无第二份定义
- **AND** prompt 模板单源 = `prompts/*.md` 静态文件，经 `include_str!` 装配，无第二份 prompt 文本

#### Scenario: 两值分派与未知类型不退化

- **WHEN** 分别以 `"requirement"` / `"test-only"` / `"bug-fix"` / `""` 调用 `phase_table`
- **THEN** 前两者返回对应 `Some` 表（10 / 7 相位），后两者返回 `None`，零 fallback、零 requirement 退化

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` | `git` 四类封闭集，经 `StepNode::group()` 派生）、`role`（agent 步角色）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，非 agent 步可缺省）、`on_fail` 语义（feedback 循环 / 终态 / 升格相位 fail）；
- `prerequisites: &'static [&'static str]`：相位直接前置依赖（表内自描述，stale 传播与 allowed_backtrack 推导的唯一事实源；MUST NOT 存在第二份独立 match）；
- `backtrackable: bool`：相位回溯资格（表内自描述，沿 `prerequisites` 先例；`allowed_backtrack_phases` SHALL 按本属性过滤——`backtrackable=false` 的相位 MUST NOT 出现在任何相位的回溯白名单中，表头 initialize 恒 false（基线检查不可作为回溯目标），过滤 MUST NOT 以「跳过 idx==0」类位置推导实现——表结构变化即漂移）；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；archive 相位 SHALL 无 `artifact_manifest`（不产文档）；requirement 表 test-design SHALL 声明 `markdown-doc·test-design.md`、implement SHALL 声明 `tasks-progress·tasks.md`（实现期进度视图，服务 phase_next 下发面——发现面同路径按首命中去重，tasks.md 出列为 dev-design 的 markdown-doc 单候选）；test-only 表 code-analyze SHALL 声明 `markdown-doc·design.md`（源码分析）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：archive 与 initialize 相位 SHALL 为 `None` / `None`（确定性相位）。

`StepNode` 词汇 SHALL 为二十一值（`executor` / `evaluator` / `decision` / `static_check` / `test_execution` / `mutation_test` / `clean` / `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` / `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `cleanup` / `seal` / `finalize`），与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 全映射（`mutation_test`、`cleanup` 与 `clean` 线词入封闭集，18→19、19→20 与 20→21 均为白名单式扩展）；code-analyze 复用既有 `Executor` / `Evaluator` / `VerdictGate` / `PhaseStart` / `PhaseLog` 步词，MUST NOT 新增步词。`StepGroup` SHALL 保持四分类（`Exec` / `Agent` / `Gate` / `Git`），`mutation_test` 归 `Exec`、`cleanup` 与 `clean` 归 `Git`。两表表头 SHALL 声明共享 `INITIALIZE_STEPS`：`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`（一次性基线检查相位——Clean 为 vcs 直调基线恢复步、StaticCheck / TestExecution 步 `on_fail=Terminal`（失败即停，无反馈边——基线的债不属于本次变更）、`PhaseStart` / `Clean` / `PhaseLog` `on_fail=None`；零 `artifact_manifest`——基线报告树非文档素材，回查经节点 `output_ref`）。`REQUIREMENT_TABLE` 的 archive `PhaseDefinition` SHALL 声明 11 步序列：`PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog`（`ResolveConflicts` 为条件执行步，`Cleanup` 为尽力步——清理失败仅 step detail 警告、步恒 `passed`，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」；其余步 `on_fail=None`——步失败即 Terminal）。`TEST_ONLY_TABLE` SHALL 复用 requirement 同名 step 静态（proposal / test-design / test-gen / test-execution / archive），并新增 `CODE_ANALYZE_STEPS`：`PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog`（P→E，`on_fail=Terminal` 于 VerdictGate）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目（含 initialize、archive 与 code-analyze）
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），initialize 相位 5 步在案（表头 `INITIALIZE_STEPS` 两表共享）且 `executor=None` / `evaluator=None` / `backtrackable=false`，archive 相位 11 步在案且 `executor=None` / `evaluator=None`，code-analyze 5 步在案（P→E）；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: StepNode 二十一值与四分类分组

- **WHEN** 审查 `StepNode` / `RunStepKind` / `StepGroup` 定义
- **THEN** 词汇二十一值全映射（9 个归档步词（含 `cleanup`）、`mutation_test` 与 initialize 基线步词 `clean` 在案，code-analyze 零新步词）；`StepGroup` 含 Git，commit / rebase / merge / cleanup / finalize / clean 归 Git、preflight / seal / mutation_test 归 Exec、spec_sync / resolve_conflicts 归 Agent

#### Scenario: 回溯白名单按 backtrackable 过滤

- **WHEN** 对两表任一非表头相位调用 `allowed_backtrack_phases`
- **THEN** 白名单为其前序相位中 `backtrackable=true` 者的集合，initialize 恒不在白名单（`backtrackable=false`）；过滤依据为表内显式属性，无「跳过 idx==0」位置推导

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

step 复用 SHALL 为：initialize 复用 requirement 表头 `INITIALIZE_STEPS`（两表共享同一 static），proposal / test-design / test-gen 复用 requirement 同名 `*_STEPS`（含同名 executor / evaluator prompt），test-execution 复用 `TEST_EXECUTION_STEPS`（确定性门禁 `[PhaseStart, TestExecution, MutationTest, PhaseLog]`——拆步：测试 / 覆盖率修绿后才跑突变，MutationTest `on_fail=FeedbackLoop`，零 agent，不照搬插件侧 Executor→E），archive 复用 `ARCHIVE_STEPS`，code-analyze 用新增 `CODE_ANALYZE_STEPS`。test-only 的 `proposal` 描述 SHALL 为「测试向提案」（description 字段语义，prompt 主体暂复用 requirement proposal prompt，是否拆分独立 prompt 留待看效果）。

#### Scenario: test-only 七相位序与复用

- **WHEN** 读取 `phase_table("test-only")` 的相位序与各相位 `steps`
- **THEN** 序恰为 initialize / proposal / code-analyze / test-design / test-gen / test-execution / archive；initialize / proposal / test-design / test-gen / test-execution / archive 的 `steps` 指针与 requirement 同名 static 一致，code-analyze 为 `CODE_ANALYZE_STEPS`

#### Scenario: 前置依赖表内自描述

- **WHEN** 读取 `TEST_ONLY_TABLE` 各 `PhaseDefinition.prerequisites`
- **THEN** initialize=[]、code-analyze=[proposal]、test-design=[proposal, code-analyze]、test-gen=[test-design]、test-execution=[test-gen]、proposal/archive=[]；`dependents(TEST_ONLY_TABLE, "proposal")` 含 code-analyze 与 test-design

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs` | 表结构单源 | `PhaseDefinition` 增 `backtrackable: bool`（表内自描述）；`INITIALIZE_STEPS` 静态五步（`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`，StaticCheck / TestExecution `on_fail=Terminal`）+ 两表表头 `INITIALIZE` 表项（`executor=None` / `evaluator=None` / `prerequisites=[]` / `backtrackable=false` / 零 artifact_manifest）；`allowed_backtrack_phases` 按 `backtrackable` 过滤（无位置推导跳过）；`REQUIREMENT_TABLE` 10 相位 / `TEST_ONLY_TABLE` 7 相位 |
| `crates/core/workflow/src/state.rs`（StepNode / RunStepKind） | 二十一值词面单点 | `Clean` / `clean` 白名单式入封闭集（20→21）；`StepNode::group()` 将 clean 派生 `StepGroup::Git`、`role()` 为 None；与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 |
| `crates/core/workflow/src/write/phase_next.rs`（零改动） | 路由 | 表驱动「首个未 pass 相位」天然含 initialize 表头，零特判、零新规则 |
