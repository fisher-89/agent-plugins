# desktop-change-flow-view — desktop-test-only-flow delta

## MODIFIED Requirements

### Requirement: 流程图节点模型与两分类

change 详情页 SHALL 以 react-flow 流程图承载执行史，节点模型 SHALL 收敛为**单分类（仅 runtime step 节点）**。转换层 SHALL 将运行执行图（`runs[].nodes` 库读史 ∪ 在飞 run 活面，`run-state.ts` 统一视图派生）归并为 runtime step 节点，MUST NOT 再归并 eval（phase attempt）节点或 active（`activePhase`）节点：

- **runtime step 节点**：按后端稳定身份键 `(phase, attempt, step, instanceNo)` 直渲染，running 态 pulse、failed 红态、stopped 灰态、passed 常态；
- **eval / active 相位节点整体退役**：`collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs` 删除，`sequence = [...runNodes]`；运行中由 running 态 runtime 节点承载；header Badge 仍读 `detail.activePhase`（保留，仅移除图上 active 节点）。

`interrupted[]` 留档 SHALL 保持不上图：读模型（Rust `Workflow` / `ChangeDetail`）MUST NOT 解析该字段，前端节点模型 MUST NOT 保留 interrupted 词汇（`FlowNodeKind` 概念整体退役，前端节点类型单一为 `runtime`）；磁盘上既有 `workflow.json` 的 `interrupted[]` 数据 SHALL 原样保留（serde 未知字段忽略），MUST NOT 迁移或清除，写入方（插件 CLI）MUST NOT 因本变更改动。相位列序 SHALL 以 `detail.pipeline` 出线序为准（requirement 9 列、test-only 6 列），不依赖 eval 排列、不依赖前端 `PIPELINE_PHASES` 常量。

#### Scenario: 两分类节点呈现

- **WHEN** 打开一个含多 attempt 与 backtrack 的 v2 change 详情
- **THEN** 图上呈现 eval 节点（pass 绿 / fail 红）与 pulse 的 active 节点，无 dashed 灰显的 interrupted 节点

#### Scenario: interrupted 留档不出图且数据保留

- **WHEN** 打开 workflow.json 含 `interrupted[]` 留档的存量 v2 change（v2-b 夹具形态：`interrupted` 的 test-gen#1 与 `active_phase` 的 implement#2 均不在 eval 序列内）
- **THEN** test-gen#1 不以任何节点形态出现在图上，磁盘 workflow.json 的 `interrupted[]` 数据原样保留、详情查询不报错

#### Scenario: stale 淡化

- **WHEN** 某站的最新 pass eval 被 backtrack 标记 `stale: true`
- **THEN** 该节点呈半透明淡化，表示废弃支线，但仍保留在图上

#### Scenario: 单分类节点呈现

- **WHEN** 打开一个含多 attempt 与 backtrack 的 v2 change 详情
- **THEN** 图上仅呈现 runtime step 节点（WorkerAgent / ToolStep / Gate / Git 类型可辨，running / failed / stopped / passed 状态如实），无 eval 节点、无 active 节点、无 dashed 灰显的 interrupted 节点

#### Scenario: 运行中由 running 态 runtime 节点承载

- **WHEN** 某 change run 运行至 implement 相位（含 active_phase 残留）
- **THEN** 当前执行位置的 runtime step 节点呈 pulse 运行态，图上无独立 active 节点；header Badge 仍读 `detail.activePhase` 呈现「运行中 · implement · attempt N」

### Requirement: 10 列手工坐标布局

布局 SHALL 采用手工坐标，MUST NOT 引入 dagre / elk 等自动布局引擎：`x = colIndex × COL_W`（`colIndex` = 该 phase 在 `detail.pipeline` 出线序中的下标，列总数 = 该 change `workflow_type` 相位表长度：requirement 9 列、test-only 6 列；未走过的站呈现空列），`y = 列内执行序 × ROW_H`。同 phase 的全部 step 节点 SHALL 纵向对齐成一列。phase 列 SHALL 以 react-flow v12 subflow 承载（`parentId` + `extent: 'parent'`），列头展示 phase 名与该站过程文档徽章（含 eval-checklist 素材，见「过程素材挂载规则」）。前端 `layout.ts` 的 `PIPELINE_PHASES` 常量 SHALL 退役（列序由 `detail.pipeline.map(p => p.phase)` 出线承载，MUST NOT 前端自持静态相位序）。COL_W / ROW_H 常量取值由 design 定夺。视口 SHALL 默认 100% 缩放（MUST NOT 初始 fitView 适配），滚轮 SHALL 平移画布，显式刷新重绘时 SHALL 保留当前视口。

#### Scenario: 列对齐与空列随 workflow_type 恒定

- **WHEN** 某 requirement change 只走到 dev-design 站（proposal 有 step 节点）
- **THEN** proposal 与 dev-design 的 step 节点各自纵向对齐成列，后续 7 站（test-design / implement / test-gen / test-execution / code-review / acceptance / archive）仍以空列呈现，列总数恒为 9（requirement）

#### Scenario: test-only 六列

- **WHEN** 打开某 test-only change（proposal 有 step 节点）
- **THEN** 图为 6 列（proposal / code-analyze / test-design / test-gen / test-execution / archive），code-analyze 为真实相位列（非占位列），后续未走到的站以空列呈现，列总数恒为 6

#### Scenario: archive 列常驻

- **WHEN** 打开已归档 change（含 archive 相位空 checklist PhaseRecord 与 10 步节点）
- **THEN** archive 列为该 workflow_type 相位表的末列（requirement 第 9 列 / test-only 第 6 列）常驻呈现（10 步节点，无 pass eval 节点），与 test-execution 列同构，不因 refresh 蒸发

#### Scenario: backtrack 重做自动向左

- **WHEN** test-execution#1 fail 后 backtrack 回 test-gen 并重做 test-gen#2
- **THEN** 重做的 test-gen#2 step 节点位于 test-gen 列（Δx < 0），无需显式边分类即呈现为向左回跳

## ADDED Requirements

### Requirement: 列序由 detail.pipeline 出线承载

前端列序 SHALL 单源取自 `detail.pipeline`：`buildFlowGraph` 的 columns SHALL 由 `detail.pipeline.map(...)` 派生（每项 colIndex = 数组下标、列 id = `col:<phase>`），MUST NOT 再读 `PIPELINE_PHASES`。`runStepNodes` SHALL 增加 `phaseOrder: string[]` 入参（显式传序，`colIndex = phaseOrder.indexOf(step.phase)`，`phaseOrder` 外相位不入图）；`attachments` 的 `artifactColumn` SHALL 以 pipeline 相位集 / phaseOrder 校验归属，MUST NOT 再读 `PIPELINE_PHASES`。`layout.ts` 的 `PIPELINE_PHASES` 常量 SHALL 退役为测试夹具专用常量（或彻底删除），MUST NOT 作为生产列序来源。

#### Scenario: 列序取自 pipeline

- **WHEN** 以 requirement（9 相位）与 test-only（6 相位）的 `ChangeDetail` 夹具调用 `buildFlowGraph`
- **THEN** 两图 `columns` 的相位序分别与各自 `detail.pipeline` 逐一一致，colIndex 为数组下标；无 `PIPELINE_PHASES` 常量消费

#### Scenario: runStepNodes 显式传序

- **WHEN** 以 `phaseOrder = ["proposal", "code-analyze", "test-design", "test-gen", "test-execution", "archive"]` 调用 `runStepNodes(steps, phaseOrder)`
- **THEN** code-analyze 节点 colIndex=1、archive 节点 colIndex=5；phaseOrder 外相位的 step 不入图

#### Scenario: 素材归属校验随出线序

- **WHEN** 以 test-only pipeline 相位集调用 `mountMaterials`，信封 payload 携带 `code-analyze` 或 pipeline 外相位
- **THEN** `code-analyze` 命中挂列，pipeline 外相位不挂列（未归属不误挂），零 `PIPELINE_PHASES.indexOf`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/layout.ts` | 布局常量与坐标纯函数 | `PIPELINE_PHASES` 退役为测试夹具常量（或删除）；`nodePosition` 的列坐标仍以 `colIndex` 计算 |
| `packages/desktop/src/views/changes/flow/graph.ts` | 转换层 | `columns` 改由 `detail.pipeline` 出线；`tableEdges` 仍读 `detail.pipeline`（backtrack label） |
| `packages/desktop/src/views/changes/flow/run-state.ts` | run 步节点转换 | `runStepNodes(steps, phaseOrder)` 显式传序；去 `PIPELINE_PHASES.indexOf` |
| `packages/desktop/src/views/changes/flow/attachments.ts` | 素材挂载 | `artifactColumn` 改收 pipeline 相位集 / phaseOrder；去 `PIPELINE_PHASES.indexOf` |
| `packages/desktop/src/views/changes/flow/` 测试 | 视图测试 | `layout.test.ts` / `graph.test.ts` / `run-state.test.ts` / `attachments.test.ts` 夹具改由 pipeline 出线（requirement 9 / test-only 6）；「恒 10 列」断言重写 |
