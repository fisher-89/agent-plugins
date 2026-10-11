# desktop-workflow-write-face Specification (Delta)

## MODIFIED Requirements

### Requirement: 相位表与 prompt 模板单源收敛

requirement 工作流相位表（相位序、executor / evaluator prompt 模板、**有序 step 序列、artifact 清单**）SHALL 单源驻 core/workflow：prompt 模板 SHALL 为 `core/workflow/src/prompts/` 全静态角色知识 md（清单与内容约定见 desktop-phase-prompts），经 `phase_table.rs` 编译期 `include_str!("prompts/<role>.md")` 显式字面路径装配，MUST NOT 按 `<role>` 动态读取（无运行时文件 IO / 目录遍历）；step 序列与 artifact 清单 SHALL 以 `PhaseDefinition.steps` / `PhaseDefinition.artifact_manifest` 编译期 const 表承载（见「相位定义有序 step 序列与 artifact 清单」），MUST NOT 落库、MUST NOT 经 port 缝运行时加载。walker MUST NOT 自持相位转移规则（每步过渡问写面 `phase_next`）；`PIPELINE_PHASES` 与 `detail.rs` 相位列保持纯布局身份 MUST NOT 上位为路由权威；TS 侧相位表（`lib/workflow.ts`）冻结，不再是桌面路径依赖，phase_table 与 `lib/workflow.ts` 的 AC-9 镜像对照口径随插件废弃终结。

#### Scenario: walker 不自持转移规则

- **WHEN** 审查 orchestration walker 源码对相位先后关系的编码
- **THEN** 零硬编码相位转移表；walker 每步过渡均以写面 `phase_next` 的返回为准

#### Scenario: 相位表单一来源

- **WHEN** 同一 requirement 工作流的相位序、step 序列与 artifact 清单在 core/workflow 与 walker 两侧分别查阅
- **THEN** 仅 core/workflow 相位表定义相位序、step 序列与 artifact 清单；walker 侧无第二份定义
- **AND** prompt 模板单源 = `prompts/*.md` 静态文件，经 `include_str!` 装配，无第二份 prompt 文本

## ADDED Requirements

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列（计划图节点）；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` 三类封闭集）、`role`（agent 步角色，如 executor / evaluator / decision）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，exec / gate 步可缺省）、`on_fail` 语义（feedback 循环 / 终态 / 升格相位 fail——形态由 design 定稿）；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；`ArtifactSpec` SHALL 携 `kind`（`markdown-doc` / `tasks-progress` 等契约 ID）与相对路径模式（`proposal.md`、`specs/**`、`design.md`、`tasks.md`、`test-reports/**` 等）与 `title`；产物发现 SHALL 按该清单相对路径枚举匹配（见 desktop-artifact-plugins）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：保持 `PhaseAgentSpec { prompt: &'static str, model_level }` 静态化形态不变（`agent_type` / `__CALL_AGENT` / `interpolate` 保持退役）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表，无 `<role>` 间接读取）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载（desktop-change-orchestration「计划图与执行图对齐与步粒度回退」）。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），`prompt_ref` 映射回 `include_str!` 静态表；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: artifact 清单权威归属

- **WHEN** 查阅 requirement 工作流各相位的 `artifact_manifest`
- **THEN** proposal.md / specs/** 属 proposal、design.md / tasks.md 属 dev-design、test-reports/** 属 test-execution（以清单声明为准）；清单之外文档不挂 phase 列（见 desktop-artifact-plugins「清单驱动发现」）

#### Scenario: PhaseAgentSpec 静态化不翻案

- **WHEN** 审查 `PhaseDefinition` 扩展后的 executor / evaluator spec
- **THEN** 仍为 `{ prompt: &'static str, model_level }`（`include_str!` 产物），无 `agent_type` / `__CALL_AGENT` / `interpolate` 残留；prompt 文本不进 db、不落 step 定义

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs` | 计划图单源（扩展） | `PhaseDefinition` 增 `steps: [StepDefinition]` + `artifact_manifest: [ArtifactSpec]`；`prompt_ref` 经 `include_str!` 静态映射；const 表，无运行时加载 |
| `crates/core/workflow/src/state.rs` | 计划图侧类型 | `StepDefinition` / `ArtifactSpec` / `StepKind`（exec/agent/gate）/ `on_fail` 语义；全静态 |
| `crates/core/workflow/src/write/phase_next.rs` | 步序列下发 | `phase_next` 响应携 next 相位 steps + manifest；路由 / 白名单 / 重试上限语义不变 |
| `crates/core/orchestration`（消费方） | plan-step 执行 | 经 port 缝进程内直调写面消费 steps / manifest；依赖方向 orchestration → workflow；walker 零自持转移规则 |
| `crates/infra/store`（零触点） | 定义不落库 | 计划图定义不进 workspace 库；store 只承载执行图节点 / 边（见 desktop-change-state-store） |
