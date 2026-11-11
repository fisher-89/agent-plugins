# desktop-workflow-write-face Specification (Delta)

## ADDED Requirements

### Requirement: verdict 推导单点（derive_verdict）

verdict 的推导规则 SHALL 为**全局单点**：`workflow::model::derive_verdict(checklist: &[ChecklistItem]) -> Verdict`——全 pass（含空 checklist）→ `Pass`，否则 `Fail`。该函数 SHALL 由 `phase_log` 落账路径与编排侧 `finish_step` 校验器**共用同一实现**（MUST NOT 两份实现、MUST NOT 由模型自报 `verdict`）。

- **迁位**：原 `crates/core/workflow/src/write/phase_log.rs` 的私有 `derive_verdict` SHALL 下沉至 `crates/core/workflow/src/model/domain.rs` 并经 `model` 模块导出；`phase_log.rs` 改调新单点，**行为逐字不变**（空 checklist → `Pass`、含任一 `pass = false` → `Fail`）。
- **校验期用途**：`finish_step` 的 evaluator 校验器 SHALL 以本函数从 checklist 推导 `verdict` 并填入 `EvaluatorChecklist`（模型不提交 `verdict` 字段；若提交则忽略），使「verdict 与 checklist 不一致」这一漂移形态在源头不可能出现。
- **落账期用途**：写面 `phase_log` SHALL 继续以本函数从 `input.checklist` 推导 verdict，`skipped=true` 时 `verdict` 必须为 `Pass` 的既有约束不变（空 checklist 落账 = 无条件通过，archive 相位依赖此语义）。
- **零形状变更**：`ChecklistItem { item, pass, evidence }` 三字段与 `ChecklistItemRecord` 落行形状 MUST NOT 变化；`item` 仍为人读正文（ID → md 静态清单「检查项」列的映射在编排侧工具边界完成，见 desktop-agent-execution「finish_step 工具协议与本步收口」与 desktop-phase-prompts「静态清单表为 ID 契约面」）。

#### Scenario: 迁位后语义不变

- **WHEN** 分别以「全 pass 三项」「含一项 pass=false」「空清单」调用 `workflow::model::derive_verdict`
- **THEN** 依次得 `Pass` / `Fail` / `Pass`；与迁位前 `phase_log` 私有实现逐态相同

#### Scenario: 单一实现无副本

- **WHEN** 审查 `crates/core/workflow/src/write/phase_log.rs` 与编排侧校验器
- **THEN** 两处均调用 `model::derive_verdict`，`grep` 下无第二份「全 pass 则 Pass」的实现；`phase_log.rs` 内无私有同名函数

#### Scenario: 模型自报 verdict 被忽略

- **WHEN** `finish_step` 的 evaluator 参数中夹带 `verdict: "pass"` 而 `items` 含一项 `pass=false`
- **THEN** 校验产出的 `EvaluatorChecklist.verdict` 为 `Fail`（由 checklist 推导），落账 verdict 同为 `Fail`；模型字段不参与推导

#### Scenario: 落账形状零变更

- **WHEN** 经 `finish_step` 收口后落账一次 eval 记录
- **THEN** `ChecklistItemRecord` 仍为 `item` / `pass` / `evidence` 三字段、`item` 为 md 静态清单「检查项」列正文；store schema 与前端 `checklist` 线面形状零变更

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/model/domain.rs` | verdict 推导单点 | `pub fn derive_verdict(checklist: &[ChecklistItem]) -> Verdict`（全 pass → Pass、空表 → Pass、否则 Fail）；`ChecklistItem` 三字段不变 |
| `crates/core/workflow/src/model/mod.rs` | 再导出 | `pub use domain::{ChecklistItem, Verdict, derive_verdict}` |
| `crates/core/workflow/src/write/phase_log.rs` | 落账调用点 | 私有 `derive_verdict` 删除、改调 `model::derive_verdict`；`skipped` 约束、report 上限校验、单事务原子落库零改动 |
| `crates/core/workflow/src/write/phase_log_test.rs` | 直测 | 三态（全 pass / 含 fail / 空表）经公开单点断言；既有落账用例断言面零改 |
| `crates/infra/store/**` | 零改动 | `ChecklistItemRecord` 形状与 `PhaseRecord` 落行不变（`item` 仍人读正文） |
| `openspec/specs/desktop-change-queries` 面 | 零改动 | checklist 线面三字段形状不变，前端 renderer 零改动 |
