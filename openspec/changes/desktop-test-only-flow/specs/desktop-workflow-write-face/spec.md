# desktop-workflow-write-face — desktop-test-only-flow delta

## MODIFIED Requirements

### Requirement: 相位表与 prompt 模板单源收敛

工作流相位表（相位序、executor / evaluator prompt 模板、**有序 step 序列、artifact 清单、前置依赖**）SHALL 单源驻 core/workflow：prompt 模板 SHALL 为 `core/workflow/src/prompts/` 全静态角色知识 md，经 `phase_table.rs` 编译期 `include_str!("prompts/<role>.md")` 显式字面路径装配，MUST NOT 按 `<role>` 动态读取；step 序列与 artifact 清单 SHALL 以 `PhaseDefinition.steps` / `PhaseDefinition.artifact_manifest` 编译期 const 表承载，MUST NOT 落库、MUST NOT 经 port 缝运行时加载。`phase_table(workflow_type)` SHALL 按类型分派：`"requirement"` → `REQUIREMENT_TABLE`（9 个路由相位：proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / archive）、`"test-only"` → `TEST_ONLY_TABLE`（6 个路由相位：proposal / code-analyze / test-design / test-gen / test-execution / archive），其余（含空白 / 未知 / bug-fix）→ `None`（**不退化**）。archive 为两表共有的确定性相位（`PhaseDefinition.executor=None` / `evaluator=None`，10 步序列见「相位定义有序 step 序列与 artifact 清单」）。walker MUST NOT 自持相位转移规则（每步过渡问写面 `phase_next`）；`phase_next` SHALL 覆盖各表末相位（requirement 8 路由相位 pass 后 next=archive，test-only 5 路由相位 pass 后 next=archive，archive 空 checklist 落账后 done）。`detail.rs` 相位列保持纯布局身份、列序随 `phase_table` 出线（见 desktop-change-queries），MUST NOT 上位为路由权威；TS 侧相位表冻结，不再是桌面路径依赖。

#### Scenario: walker 不自持转移规则

- **WHEN** 审查 orchestration walker 源码对相位先后关系的编码
- **THEN** 零硬编码相位转移表；walker 每步过渡均以写面 `phase_next` 的返回为准（含 archive → done）

#### Scenario: 相位表单一来源

- **WHEN** 同一 workflow_type 的相位序、step 序列、artifact 清单与前置依赖在 core/workflow 与 walker 两侧分别查阅
- **THEN** 仅 core/workflow 相位表定义相位序、step 序列、artifact 清单与前置依赖（requirement 9 相位、test-only 6 相位均在案）；walker 侧无第二份定义
- **AND** prompt 模板单源 = `prompts/*.md` 静态文件，经 `include_str!` 装配，无第二份 prompt 文本

#### Scenario: 两值分派与未知类型不退化

- **WHEN** 分别以 `"requirement"` / `"test-only"` / `"bug-fix"` / `""` 调用 `phase_table`
- **THEN** 前两者返回对应 `Some` 表（9 / 6 相位），后两者返回 `None`，零 fallback、零 requirement 退化

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` | `git` 四类封闭集，经 `StepNode::group()` 派生）、`role`（agent 步角色）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，非 agent 步可缺省）、`on_fail` 语义（feedback 循环 / 终态 / 升格相位 fail）；
- `prerequisites: &'static [&'static str]`：相位直接前置依赖（表内自描述，stale 传播与 allowed_backtrack 推导的唯一事实源；MUST NOT 存在第二份独立 match）；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；archive / code-analyze 相位 SHALL 无 `artifact_manifest`（不产 proposal / design / tasks 类文档）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：archive 相位 SHALL 为 `None` / `None`（确定性相位）。

`StepNode` 词汇 SHALL 保持十八值（`executor` / `evaluator` / `decision` / `static_check` / `test_execution` / `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` / `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `seal` / `finalize`），与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 全映射；code-analyze 复用既有 `Executor` / `Evaluator` / `VerdictGate` / `PhaseStart` / `PhaseLog` 步词，MUST NOT 新增步词。`StepGroup` SHALL 保持四分类（`Exec` / `Agent` / `Gate` / `Git`）。`REQUIREMENT_TABLE` 的 archive `PhaseDefinition` SHALL 声明 10 步序列：`PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog`（`ResolveConflicts` 为条件执行步，`on_fail=None`——步失败即 Terminal）。`TEST_ONLY_TABLE` SHALL 复用 requirement 同名 step 静态（proposal / test-design / test-gen / test-execution / archive），并新增 `CODE_ANALYZE_STEPS`：`PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog`（P→E，`on_fail=Terminal` 于 VerdictGate）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目（含 archive 与 code-analyze）
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），archive 相位 10 步在案且 `executor=None` / `evaluator=None`，code-analyze 5 步在案（P→E）；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: StepNode 十八值与四分类分组

- **WHEN** 审查 `StepNode` / `RunStepKind` / `StepGroup` 定义
- **THEN** 词汇十八值全映射（8 个归档步词在案，code-analyze 零新步词）；`StepGroup` 含 Git，commit / rebase / merge / finalize 归 Git、preflight / seal 归 Exec、spec_sync / resolve_conflicts 归 Agent

#### Scenario: artifact 清单权威归属

- **WHEN** 查阅 requirement 与 test-only 工作流各相位的 `artifact_manifest`
- **THEN** proposal.md / specs/** 属 proposal、design.md / tasks.md 属 dev-design、test-reports/** 属 test-execution（以清单声明为准）；archive 与 code-analyze 相位零 artifact_manifest；清单之外文档不挂 phase 列

#### Scenario: PhaseAgentSpec 静态化不翻案

- **WHEN** 审查 `PhaseDefinition` 扩展后的 executor / evaluator spec
- **THEN** 仍为 `{ prompt: &'static str, model_level }`（`include_str!` 产物），无 `agent_type` / `__CALL_AGENT` / `interpolate` 残留；archive 相位 executor / evaluator 均 None

## ADDED Requirements

### Requirement: test-only 相位表与前置依赖

`TEST_ONLY_TABLE` SHALL 为 6 个路由相位，序固定：`proposal → code-analyze → test-design → test-gen → test-execution → archive`。每相位的前置依赖 SHALL 为：

| phase | prerequisites |
|-------|---------------|
| proposal | [] |
| code-analyze | [proposal] |
| test-design | [proposal, code-analyze] |
| test-gen | [test-design] |
| test-execution | [test-gen] |
| archive | [] |

step 复用 SHALL 为：proposal / test-design / test-gen 复用 requirement 同名 `*_STEPS`（含同名 executor / evaluator prompt），test-execution 复用 `TEST_EXECUTION_STEPS`（确定性门禁 `[PhaseStart, TestExecution, PhaseLog]`，零 agent，不照搬插件侧 Executor→E），archive 复用 `ARCHIVE_STEPS`，code-analyze 用新增 `CODE_ANALYZE_STEPS`。test-only 的 `proposal` 描述 SHALL 为「测试向提案」（description 字段语义，prompt 主体暂复用 requirement proposal prompt，是否拆分独立 prompt 留待看效果）。

#### Scenario: test-only 六相位序与复用

- **WHEN** 读取 `phase_table("test-only")` 的相位序与各相位 `steps`
- **THEN** 序恰为 proposal / code-analyze / test-design / test-gen / test-execution / archive；proposal / test-design / test-gen / test-execution / archive 的 `steps` 指针与 requirement 同名 static 一致，code-analyze 为 `CODE_ANALYZE_STEPS`

#### Scenario: 前置依赖表内自描述

- **WHEN** 读取 `TEST_ONLY_TABLE` 各 `PhaseDefinition.prerequisites`
- **THEN** code-analyze=[proposal]、test-design=[proposal, code-analyze]、test-gen=[test-design]、test-execution=[test-gen]、proposal/archive=[]；`dependents(TEST_ONLY_TABLE, "proposal")` 含 code-analyze 与 test-design

### Requirement: 写面严格类型分派与未知类型显式拒绝

`phase_next` / `phase_start` / `phase_log` / `backtrack` SHALL 以 `phase_table(record.workflow_type)` 的返回为唯一分派依据：返回 `None`（未知 / 空白 / bug-fix 等不受支持类型）SHALL 显式 `Err`（错误文案通吃 `{requirement, test-only}`，MUST NOT 再写死「V1 仅 requirement」），状态库零变更。写面 MUST NOT 对未知类型做 requirement 退化。存量 `ChangeRecord.workflow_type` 恒为 `"requirement"`（写面 V1 起硬编码），严格分派后存量记录天然合法，零迁移。

#### Scenario: 未知类型显式拒绝零副作用

- **WHEN** 对 `workflow_type="bug-fix"`（或空白）的 change 调用 `phase_next` / `phase_start` / `phase_log` / `backtrack`
- **THEN** 每次均显式 `Err`（文案含 `{requirement, test-only}` 支持面），状态库零变更、零 fallback requirement

#### Scenario: test-only 分派可走通

- **WHEN** 对 `workflow_type="test-only"` 的 change 依次走 `phase_next`（proposal → code-analyze → … → archive → done）
- **THEN** 相位推进严格按 `TEST_ONLY_TABLE` 序，backtrack stale 传播按表内 `prerequisites` 正确、与 requirement 表互不串扰

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs` | 相位表两值分派 + 计划图单源 | `PhaseDefinition.prerequisites`（新增）；`phase_table(workflow_type)` → `REQUIREMENT_TABLE` / `TEST_ONLY_TABLE` / `None`；`TEST_ONLY_TABLE` + `CODE_ANALYZE_STEPS`；`dependents` 遍历 `candidate.prerequisites.contains`；删独立 `prerequisites()` match |
| `crates/core/workflow/src/prompts/code-analyze-planner.md` / `code-analyze-evaluator.md` | 新增角色知识 md | 全静态零占位；`include_str!` 显式字面路径装配进 `CODE_ANALYZE_STEPS` 与 code-analyze `PhaseAgentSpec` |
| `crates/core/workflow/src/write/create.rs` | 建档类型入参 | `create(..., workflow_type: &str)`；前置校验合法值（`phase_table` 命中）；替换硬编码 `"requirement"` |
| `crates/core/workflow/src/write/promote.rs` | promote 类型透传 | `promote_explore(..., workflow_type: &str)` → `create` |
| `crates/core/workflow/src/write/{phase_next,phase_start,phase_log,backtrack}.rs` | 严格类型分派文案 | `phase_table` `None` → 显式 `Err`；文案通吃 `{requirement, test-only}`；零 requirement 退化 |
