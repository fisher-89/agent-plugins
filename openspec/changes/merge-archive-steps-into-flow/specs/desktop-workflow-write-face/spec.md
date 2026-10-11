# desktop-workflow-write-face Specification (Delta)

## Purpose

core/workflow 保持 change 流程状态的 Rust 单一权威（读 + 写），持久化载体为 workspace 库；相位表与 prompt 模板单源驻 core/workflow，walker 经进程内直调消费，desktop 全链不读不写 workflow.json。相位表由 8 个路由相位扩为 9 个（追加 archive 确定性相位），step 词汇全封闭集由十值扩为十八值（8 个归档步词），step 分组三分类扩为四分类（Exec / Agent / Gate / Git）。

## MODIFIED Requirements

### Requirement: 相位表与 prompt 模板单源收敛

requirement 工作流相位表（相位序、executor / evaluator prompt 模板、**有序 step 序列、artifact 清单**）SHALL 单源驻 core/workflow：prompt 模板 SHALL 为 `core/workflow/src/prompts/` 全静态角色知识 md，经 `phase_table.rs` 编译期 `include_str!("prompts/<role>.md")` 显式字面路径装配，MUST NOT 按 `<role>` 动态读取；step 序列与 artifact 清单 SHALL 以 `PhaseDefinition.steps` / `PhaseDefinition.artifact_manifest` 编译期 const 表承载，MUST NOT 落库、MUST NOT 经 port 缝运行时加载。`REQUIREMENT_TABLE` SHALL 为 9 个路由相位（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / archive），archive 为确定性相位（`PhaseDefinition.executor=None` / `evaluator=None`，10 步序列见「相位定义有序 step 序列与 artifact 清单」）。walker MUST NOT 自持相位转移规则（每步过渡问写面 `phase_next`）；`phase_next` SHALL 覆盖 archive 相位（8 路由相位 pass 后 next=archive，archive 空 checklist 落账后 done）。`PIPELINE_PHASES` 与 `detail.rs` 相位列保持纯布局身份（10 列含 archive 与 code-analyze 占位列）MUST NOT 上位为路由权威；TS 侧相位表冻结，不再是桌面路径依赖。

#### Scenario: walker 不自持转移规则

- **WHEN** 审查 orchestration walker 源码对相位先后关系的编码
- **THEN** 零硬编码相位转移表；walker 每步过渡均以写面 `phase_next` 的返回为准（含 archive → done）

#### Scenario: 相位表单一来源

- **WHEN** 同一 requirement 工作流的相位序、step 序列与 artifact 清单在 core/workflow 与 walker 两侧分别查阅
- **THEN** 仅 core/workflow 相位表定义 9 相位序、step 序列与 artifact 清单（archive 相位 10 步在案）；walker 侧无第二份定义
- **AND** prompt 模板单源 = `prompts/*.md` 静态文件，经 `include_str!` 装配，无第二份 prompt 文本

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` | `git` 四类封闭集，经 `StepNode::group()` 派生）、`role`（agent 步角色）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，非 agent 步可缺省）、`on_fail` 语义（feedback 循环 / 终态 / 升格相位 fail）；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；archive 相位 SHALL 无 `artifact_manifest`（不产 proposal / design / tasks 类文档）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：archive 相位 SHALL 为 `None` / `None`（确定性相位）。

`StepNode` 词汇 SHALL 由十值扩为十八值（新增 `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `seal` / `finalize`），与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 全映射。`StepGroup` SHALL 由三分类扩为四分类（`Exec` / `Agent` / `Gate` / `Git`），`StepNode::group()` 派生：git 步（commit / rebase / merge / finalize）归 `Git`、内联步（preflight / seal）归 `Exec`、agent 步（spec_sync / resolve_conflicts 归 `Agent`，role 由 design 定稿）；MUST NOT 落独立冗余分组字段。`REQUIREMENT_TABLE` 的 archive `PhaseDefinition` SHALL 声明 10 步序列：`PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog`（`ResolveConflicts` 为条件执行步，`on_fail=None`——步失败即 Terminal）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目（含 archive）
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），archive 相位 10 步在案且 `executor=None` / `evaluator=None`；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: StepNode 十八值与四分类分组

- **WHEN** 审查 `StepNode` / `RunStepKind` / `StepGroup` 定义
- **THEN** 词汇十八值全映射（8 个归档步词在案）；`StepGroup` 含 Git，commit / rebase / merge / finalize 归 Git、preflight / seal 归 Exec、spec_sync / resolve_conflicts 归 Agent

#### Scenario: artifact 清单权威归属

- **WHEN** 查阅 requirement 工作流各相位的 `artifact_manifest`
- **THEN** proposal.md / specs/** 属 proposal、design.md / tasks.md 属 dev-design、test-reports/** 属 test-execution（以清单声明为准）；archive 相位零 artifact_manifest；清单之外文档不挂 phase 列

#### Scenario: PhaseAgentSpec 静态化不翻案

- **WHEN** 审查 `PhaseDefinition` 扩展后的 executor / evaluator spec
- **THEN** 仍为 `{ prompt: &'static str, model_level }`（`include_str!` 产物），无 `agent_type` / `__CALL_AGENT` / `interpolate` 残留；archive 相位 executor / evaluator 均 None

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/state.rs` | step 词汇与分组单点 | `StepNode` / `RunStepKind` 十八值（+8 归档步词）、`StepGroup` 四分类（+Git）、`StepNode::group()` / `role()` 派生；与 orchestration `ChangeStepKind` 同词面 |
| `crates/core/workflow/src/write/phase_table.rs` | 相位表单源 | `REQUIREMENT_TABLE` 9 相位（+archive 10 步序列 + executor/evaluator None）；prompt `include_str!` 装配；archive 无 artifact_manifest |
| `crates/core/workflow/src/write/phase_next.rs` | 路由权威 | 9 路由相位 → done；`has_phase_passed(archive)` 读 archive 列 PhaseRecord（空 checklist 落账识别）；零 archive 特判 |
| `crates/core/workflow/src/write/` 写面模块 | change 流程状态域权威写面 | `phase_next` / `phase_start` / `phase_log`（checklist 信封 + 会话槽位，单事务原子；空 checklist 合法）/ `backtrack` / `archive`（双写）；落库经 port 缝；sync 无 tokio |
| 落库 port 缝 | core/workflow → store 的进程内缝 | trait 定义落位 design 定稿；desktop-app 装配 store 实现；core/workflow 零 infra 依赖 |
| `crates/core/orchestration`（消费方） | walker 经 port 缝进程内直调写面 | 依赖方向 orchestration → workflow；walker 零自持转移规则；vcs 缝经 `ArchiveVcsPort` 下沉 infra/vcs |
| 插件 MCP / TS 写路径（彻底分离，不改） | CLI 工作流自有写通道 | 双向墙：desktop 不读不写 workflow.json；插件零改动 |
| `crates/infra/store`（workspace 库实例） | 状态落库载体 | ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunNodeRecord / RunEdgeRecord |
