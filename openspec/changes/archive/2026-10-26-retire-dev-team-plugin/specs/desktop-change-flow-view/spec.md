# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 流程图节点模型与两分类

change 详情页 SHALL 以 react-flow 流程图承载执行史，节点模型 SHALL 收敛为**单分类（仅 runtime step 节点）**。转换层 SHALL 将运行执行图（`runs[].nodes` 库读史 ∪ 在飞 run 活面，`run-state.ts` 统一视图派生）归并为 runtime step 节点，MUST NOT 再归并 eval（phase attempt）节点或 active（`activePhase`）节点：

- **runtime step 节点**：按后端稳定身份键 `(phase, attempt, step, instanceNo)` 直渲染，running 态 pulse、failed 红态、stopped 灰态、passed 常态；
- **eval / active 相位节点整体退役**：`collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs` 删除，`sequence = [...runNodes]`；运行中由 running 态 runtime 节点承载；header Badge 仍读 `detail.activePhase`（保留，仅移除图上 active 节点）。

`interrupted[]` 留档 SHALL 保持不上图：读模型（Rust `Workflow` / `ChangeDetail`）MUST NOT 解析该字段，前端节点模型 MUST NOT 保留 interrupted 词汇（`FlowNodeKind` 概念整体退役，前端节点类型单一为 `runtime`）；磁盘上既有 `workflow.json` 的 `interrupted[]` 数据 SHALL 原样保留（serde 未知字段忽略），MUST NOT 迁移或清除；外部写入方（插件期 CLI 工作流）已随插件退役删除，本条的冻结面 MUST NOT 因「兼容外部写入方」而解锁。相位列序 SHALL 以 `detail.pipeline` 出线序为准（requirement 10 列、test-only 7 列，initialize 为首列），不依赖 eval 排列、不依赖前端 `PIPELINE_PHASES` 常量。

#### Scenario: 冻结面不因写入方消失而解锁

- **WHEN** 审查 `interrupted[]` 的读模型与前端节点模型
- **THEN** 读模型仍 MUST NOT 解析该字段，前端节点模型仍 MUST NOT 保留 interrupted 词汇
- **AND** 无「为兼容外部写入方保留解析面 / 保留 schema 对齐」类断言或注释（写入方随插件退役）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/types.ts` + `graph.ts` + `flow-event-node.tsx` + `attachments.ts` | 前端节点词汇 | 节点类型单一 `runtime`；interrupted 词汇不回归（写入方消失不构成解锁理由） |
| `workflow.json` 的 `interrupted[]` 字段（磁盘留档） | 历史数据 | 原样保留：不迁移、不清除、不解析；自此纯留档，无写入方 |
| 插件期 TS `workflow.schema.ts` 与 CLI 写入方 | **（删除）** | 随插件整目录退役；Module Contract 原「磁盘形状与写入方（不改）」行整行删除——「双方各管其面」的对偶面已不存在 |
