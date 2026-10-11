# desktop-change-flow-view Specification (Delta)

## Purpose

定义 desktop change 详情页的 react-flow 流程图视图：attempt 级两分类节点（eval / active）、10 列手工坐标布局、时间序边推导（读边表）、过程素材两层挂载、右侧抽屉单一交互入口与建档单态呈现；转换层为纯函数、渲染薄层复用 react-flow v12。归档并入流程图：archive 为第 10 列，归档 10 步节点上图（四类节点可辨，git 步独立分类），`ArchivePanel` / `StageList` 六段清单退役。

## RENAMED Requirements

- FROM: `### Requirement: 9 列手工坐标布局`
- TO: `### Requirement: 10 列手工坐标布局`

## MODIFIED Requirements

### Requirement: 10 列手工坐标布局

布局 SHALL 采用手工坐标，MUST NOT 引入 dagre / elk 等自动布局引擎：`x = PIPELINE_PHASES.indexOf(phase) × COL_W`（10 列恒定，未走过的站呈现空列），`y = 列内执行序 × ROW_H`。同 phase 的全部事件 SHALL 纵向对齐成一列。phase 列 SHALL 以 react-flow v12 subflow 承载（`parentId` + `extent: 'parent'`），列头展示 phase 名与该站过程文档徽章。`PIPELINE_PHASES` SHALL 为 10 列（proposal → dev-design → test-design → implement → test-gen → test-execution → code-review → acceptance → code-analyze → archive，后端 `detail.rs` 与前端 `layout.ts` 双侧同步）。COL_W / ROW_H 常量取值由 design 定夺。视口 SHALL 默认 100% 缩放（MUST NOT 初始 fitView 适配），滚轮 SHALL 平移画布，显式刷新重绘时 SHALL 保留当前视口。

#### Scenario: 列对齐与空列恒定

- **WHEN** 某 change 只走到 dev-design 站（proposal 有 eval 记录）
- **THEN** proposal 与 dev-design 事件各自纵向对齐成列，后续 8 站仍以空列呈现（含 code-analyze 占位列与 archive 列），列总数恒为 10

#### Scenario: archive 列常驻

- **WHEN** 打开已归档 change（含 archive 相位空 checklist PhaseRecord 与 10 步节点）
- **THEN** archive 列为第 10 列常驻呈现（1 个 pass eval 节点 + 10 步节点），与 test-execution 列同构，不因 refresh 蒸发

#### Scenario: backtrack 重做自动向左

- **WHEN** test-execution#1 fail 后 backtrack 回 test-gen 并重做 test-gen#2
- **THEN** 重做的 test-gen#2 节点位于 test-gen 列（Δx < 0），无需显式边分类即呈现为向左回跳

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的节点 SHALL 呈现运行态（pulse 视觉，与既有 active 节点同款），ToolStep 节点、Git 节点与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（四类可区分）。运行态呈现 SHALL 与既有节点模型并存：eval（实心 pass 绿 / fail 红）、stale 淡化规则不变；`PIPELINE_PHASES` 10 列布局、边表读取规则与空列恒定 MUST NOT 因运行态呈现改变。

运行收口后，图 SHALL 保持**步节点常驻渲染**：已落库执行图节点（`runs[].nodes` 十八值全词汇）自统一视图派生常驻上图，与运行中实时面走**同一条派生路径**；运行态视觉（pulse）褪去、终态样式呈现。常驻渲染 SHALL 取**全史叠加**（多次 run 的节点全部保留），archive 相位节点随当前 run 常驻。节点身份键 SHALL 为 `(phase, attempt, step, instanceNo)`（自后端直读），前端 `runStepNodes` 的「槽位配对 + `:seq` 后缀堆叠」机制 SHALL 退役。归档步词 SHALL 落到 `RunStepGroup` 的 git / exec / agent 分类（`GROUP_LABEL` 同步），`archive-panel` / `StageList` 六段清单 SHALL 退役（归档六段作为 archive 列 step 节点呈现，不设独立面板）。

#### Scenario: 运行态与四类并存

- **WHEN** implement#2 相位运行中且历史上有 fail→retry 与 active_phase 残留，archive 相位随后进入
- **THEN** 图上同时呈现：历史 eval 节点（实心三色）、active_phase 的 active 节点（pulse）与当前节点（pulse 运行态）及同相位的 ToolStep / Git / Gate 节点（类型可辨、状态如实）

#### Scenario: archive 步节点上图

- **WHEN** archive 相位运行中（或已收口）
- **THEN** archive 列呈现 10 步节点：Preflight / Seal 为 exec、Commit / Rebase / Merge / Finalize 为 git、SpecSync / ResolveConflicts 为 agent、PhaseStart / PhaseLog 为 exec；git 步分类徽章可辨，无独立 ArchivePanel / StageList

#### Scenario: 收口后常驻不回落

- **WHEN** run 以 archive 相位完成收口（或 fail / stopped 收口）后刷新 / 重开详情
- **THEN** 已落库执行图节点常驻上图（含 archive 列，终态样式、无 pulse），派生自统一视图库读史、与运行中同一转换函数；无「回落后步节点消失」的形态跳变

#### Scenario: 稳定身份键渲染与 feedback 多实例

- **WHEN** 某 attempt 内 static-check 失败触发 executor 修复（feedback 重跑）后刷新详情
- **THEN** executor 首次与修复各为 `instanceNo` 递增的独立节点，feedback 边自边表呈现；无「槽位配对 + `:seq` 堆叠」逻辑残留，同键多实例不串扰

#### Scenario: 全史叠加与在飞叠加

- **WHEN** 某 change 历经两次 run 后第三次 run 运行中（含 archive 相位）
- **THEN** 图上前两 run 的节点全史常驻，第三 run 活节点（pulse）叠加其上；archive 列节点随 run 归属分层可溯

### Requirement: 自动确认发起开关

change 详情页运行控制面板 SHALL 在发起操作区提供「自动确认」开关（默认关）：开启后发起 run 以 `autoNextPhase=true` 调运行控制命令面，关闭则 `false`。开关 SHALL 为发起参数（发起时定格）：运行期间 MUST NOT 可切换；节奏变更 = 停止后重发。auto 模式 run MUST NOT 呈现 phase 间确认卡片（无 `ConfirmWait` 事件到达）；ask 中断卡片 SHALL 照常呈现与应答（ask 不自动应答）。开关旁 SHALL 提供一行说明文案轻提示。归档前停等（stop-before-archive）SHALL 独立于「自动确认」开关：无论 auto / 手动模式，run 走到 archive 前 SHALL 呈现归档确认卡片（含完成度警告 + delta specs + merge 目标），用户确认后才进入 archive 相位（归档不自动推进）。开关 SHALL 以 data-testid 提供测试挂钩。

#### Scenario: 开关默认关与发起传参

- **WHEN** 打开运行控制面板（无运行 run / 终局后），开关保持默认关并点击发起
- **THEN** `changeFlowStart` 携 `autoNextPhase=false`，run 照常在相位落账后停等确认；开启开关后再发起则携 `autoNextPhase=true`

#### Scenario: auto run 不出 phase 间确认卡片但归档前停等

- **WHEN** auto 模式 run 相位落账 pass 推进下一相位，直至走完 8 个路由相位
- **THEN** 页面无 phase 间确认卡片；到达 archive 前 SHALL 呈现归档确认卡片（stop-before-archive），未确认不进 archive 相位；确认后 archive 相位执行至收口

#### Scenario: auto run 的 ask 卡片照常

- **WHEN** auto 模式 run 中决策 ask 到来
- **THEN** ask 中断卡片照常呈现，用户应答后 run 继续；确认卡片不因 auto 模式对 ask 生效

#### Scenario: 运行期定格与收口回归

- **WHEN** auto 模式 run 运行中尝试切换开关
- **THEN** 开关不可切换（定格为发起值）；run 收口后控制入口回到发起区，可按任意节奏再次发起

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision / spec_sync / resolve_conflicts）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线，点击已收口的 WorkerAgent 节点 SHALL 打开转录重放。会话寻址 SHALL 记录 id 直查优先：attempt 记录暴露的会话槽位 id 在场时按 session id 直查；槽位缺席时回退既有 sourceRef 定式反查（身份段为 change id；归档步两会话的 sourceRef 定式由 design 定稿，MUST 在同 phase 同 attempt 内唯一）。eval 节点与 active 节点的抽屉联动 SHALL 均覆盖三会话（executor / evaluator / decision）；归档步节点（spec_sync / resolve_conflicts）SHALL 经其节点 `session_id` 直查转录（无 attempt 三槽位概念）。ToolStep / Git / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果（左列空态占位、右列展示步骤输出摘要）。

#### Scenario: 归档步节点转录直查

- **WHEN** 点击 archive 相位的 SpecSync 或 ResolveConflicts 节点（`session_id` 在案）
- **THEN** 抽屉左列经节点 `session_id` 直查该会话转录（实时流 / 重放一致），ToolStep / Git 步节点左列空态、右列呈现步骤输出摘要

#### Scenario: 槽位缺席回退反查

- **WHEN** 打开槽位列 None 的 attempt 记录（缺省落账）
- **THEN** 转录面板回退 sourceRef 定式反查（身份段 = change id）并照常重放，行为与直查路径一致可靠

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/layout.ts` | 10 列布局常量 | `PIPELINE_PHASES` 10 列（追加 archive，与后端 `detail.rs` 双侧同步）；`x = colIndex × COL_W`、`y = 执行序 × ROW_H`；COL_W / ROW_H 常量由 design 定夺 |
| `packages/desktop/src/views/changes/flow/graph.ts` + `types.ts` | 转换层扩展 | 节点模型增运行态与四类节点可辨维度（git 步分类）；边读边表；`PIPELINE_PHASES` 10 列布局、attempt 缺号兜底不变；纯函数直测保持绿 |
| ReactFlow 渲染薄层组件 | 图呈现 | `@xyflow/react` v12 subflow；10 列恒定 + 默认 100% 视口 + 滚轮平移；eval 实心 / active pulse / stale 半透明 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | 运行态步词与分组 | `RunStepGroup` + `GROUP_LABEL` 加 git 分类；8 个归档步词落到 exec / agent / git 分支；`runStepNodes` 槽位配对退役 |
| `packages/desktop/src/views/changes/flow/archive-panel.tsx` + `archive-state.ts` + `hooks/use-archive-flow.ts` | **（删除）** | 六段清单、`ARCHIVE_STAGES`、`applyArchiveUpdate`、`useArchiveFlow` 随归档并入流程图退役 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | id 寻址与单态呈现 | 归档列上图；stop-before-archive 停等面承载归档确认对话；`ArchivePanel` 装配移除；发起 / 停止 / 确认 / ask 应答卡片 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 抽屉双列 | 归档步节点 session_id 直查 / 空态占位；右列二分节不变 |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 转录反查 | `agent_sessions(root, 'change', sourceRef)` 身份段 = change id；归档步节点经 session_id 直查优先 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型再生跟随 | `ChangeDetail` 10 列 pipeline / nodes 十八值再生后同步；tsc 全量类型检查拦截漂移 |
| `packages/desktop/package.json` | 版本交付 | `version` 0.4.36 → 0.4.37（`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动） |
