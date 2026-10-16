# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的 step 节点 SHALL 呈现运行态（pulse 视觉），ToolStep 节点、Git 节点与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（四类可区分）。运行态呈现 SHALL 与既有节点模型并存：step 节点 running / failed / stopped / passed 四态如实；`PIPELINE_PHASES` 10 列布局、边表读取规则与空列恒定 MUST NOT 因运行态呈现改变。eval 节点与 active 节点 SHALL 不再上图（相位节点整体退役，见「流程图节点模型与两分类」）。

运行收口后，图 SHALL 保持**步节点常驻渲染**：已落库执行图节点（`runs[].nodes` 二十值全词汇）自统一视图派生常驻上图，与运行中实时面走**同一条派生路径**；运行态视觉（pulse）褪去、终态样式呈现。常驻渲染 SHALL 取**全史叠加**（多次 run 的节点全部保留），archive 相位节点随当前 run 常驻。节点身份键 SHALL 为 `(phase, attempt, step, instanceNo)`（自后端直读），前端 `runStepNodes` 的「槽位配对 + `:seq` 后缀堆叠」机制 SHALL 退役。归档步词 SHALL 落到 `RunStepGroup` 的 git / exec / agent 分类（`GROUP_LABEL` 同步），`archive-panel` / `StageList` 六段清单 SHALL 退役（归档六段作为 archive 列 step 节点呈现，不设独立面板）。步词汇归一（库读史 snake 词 → 活面 camel 词，`normalizeKind` / `stepWord` 单点）SHALL 保持二十值逐词对应：`mutation_test` ↔ `mutationTest` 与 `cleanup` ↔ `cleanup` 在案、`mutationTest` 归 exec 分类、`cleanup` 归 git 分类（现有步骤渲染直接吃，零渲染改动）。Cleanup 步的 detail 警告（清理失败 / 跳过注记）SHALL 随节点 detail 面呈现。

#### Scenario: 运行态与四类并存

- **WHEN** implement#2 相位运行中且历史上有 fail→retry 与 active_phase 残留，archive 相位随后进入
- **THEN** 图上同时呈现：历史 step 节点（终态）、当前节点（pulse 运行态）及同相位的 ToolStep / Git / Gate 节点（类型可辨、状态如实），无 eval 节点、无 active 节点

#### Scenario: archive 步节点上图

- **WHEN** archive 相位运行中（或已收口）
- **THEN** archive 列呈现 11 步节点：Preflight / Seal 为 exec、Commit / Rebase / Merge / Cleanup / Finalize 为 git、SpecSync / ResolveConflicts 为 agent、PhaseStart / PhaseLog 为 exec；git 步分类徽章可辨，无独立 ArchivePanel / StageList

#### Scenario: 收口后常驻不回落

- **WHEN** run 以 archive 相位完成收口（或 fail / stopped 收口）后刷新 / 重开详情
- **THEN** 已落库执行图节点常驻上图（含 archive 列，终态样式、无 pulse），派生自统一视图库读史、与运行中同一转换函数；无「回落后步节点消失」的形态跳变

#### Scenario: 稳定身份键渲染与 feedback 多实例

- **WHEN** 某 attempt 内 static-check 失败触发 executor 修复（feedback 重跑）后刷新详情
- **THEN** executor 首次与修复各为 `instanceNo` 递增的独立节点，feedback 边自边表呈现；无「槽位配对 + `:seq` 堆叠」逻辑残留，同键多实例不串扰

#### Scenario: 全史叠加与在飞叠加

- **WHEN** 某 change 历经两次 run 后第三次 run 运行中（含 archive 相位）
- **THEN** 图上前两 run 的节点全史常驻，第三 run 活节点（pulse）叠加其上；archive 列节点随 run 归属分层可溯

#### Scenario: mutationTest 步词汇归一

- **WHEN** test-execution 相位 MutationTest 步上图（活面或库读史）
- **THEN** `mutation_test` ↔ `mutationTest` 归一在 `normalizeKind` / `stepWord` 单点可考，节点落 exec 分类徽章；现有步骤渲染零改动消费

#### Scenario: cleanup 步词汇归一与警告呈现

- **WHEN** archive 相位 Cleanup 步上图（清理成功 / 失败警告 / 幂等跳过三态任一）
- **THEN** `cleanup` ↔ `cleanup` 归一在 `normalizeKind` / `stepWord` 单点可考，节点落 git 分类徽章（标签呈现）；失败警告或跳过注记随节点 detail 面可读，节点状态恒 `passed`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/run-state.ts` | 词归一单点 | `normalizeKind` / `stepWord` 补 `cleanup` ↔ `cleanup`；`cleanup` 归 git 分类 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 节点呈现 | `STEP_LABEL` 补 cleanup 词条；detail 面呈现清理警告 / 跳过注记（节点恒 passed） |
