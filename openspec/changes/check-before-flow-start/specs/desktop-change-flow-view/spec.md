# desktop-change-flow-view Specification (Delta)

## ADDED Requirements

### Requirement: initialize 相位列与 clean 步词呈现

initialize 相位（表头一次性基线检查相位，见 desktop-change-orchestration「initialize 基线检查相位执行体」）SHALL 以**零结构改动**方式呈现：initialize 相位列随 `detail.pipeline` 出线自动成为首列（列头相位名文案补 initialize 人读标签），Clean / StaticCheck / TestExecution / PhaseLog 步节点以既有 runtime step 节点渲染（四类分组徽章可辨、running / failed / stopped / passed 状态如实）；基线检查输出经节点 `outputRef` 的既有 stdout 面板回查（`read_exec_output` 命中 baseline 树）；initialize 失败的 run 终态 reason（Error / Fail 分文案 + 报告目录指引）经既有终态呈现面直达。前端步词呈现 SHALL 补 `clean` 线词：`STEP_LABEL` 穷尽 Record 补 `clean` 键（人读文案）、`normalizeKind` / `stepWord` 单点补 `clean` ↔ `clean` 映射、git 分类徽章覆盖（`RunStepGroup` / `GROUP_LABEL`），现有步骤渲染零改动直接吃。MUST NOT 为 initialize 引入新节点类型、新面板或独立交互入口。

#### Scenario: initialize 列与步节点零结构呈现

- **WHEN** 打开一个已跑过 initialize 相位（绿或红）的 change 详情
- **THEN** initialize 列为首列（相位名人读文案在案），Clean（git 分类徽章）/ StaticCheck / TestExecution / PhaseLog 步节点按序呈现、状态如实；无新节点类型、无独立面板

#### Scenario: 基线失败终态与回查

- **WHEN** initialize 相位以 Terminal 失败（static-check 红或 test-execution Error / Fail）收口的 run 详情被打开
- **THEN** run 终态 reason 呈现对应文案（Error / Fail 可辨、含 baseline 报告目录指引）；点击 StaticCheck / TestExecution 节点经既有 stdout 面板回查 baseline 树输出

#### Scenario: clean 步词穷尽补齐

- **WHEN** bindings 再生成后审查 `STEP_LABEL` / `normalizeKind` / `stepWord` / `GROUP_LABEL`
- **THEN** `clean` 键在案（人读文案非空）、snake ↔ camel 归一在单点可考、落 git 分类；穷尽 Record 键集与二十一词全词汇一致（编译期穷尽表兜底）

## MODIFIED Requirements

### Requirement: 流程图节点模型与两分类

change 详情页 SHALL 以 react-flow 流程图承载执行史，节点模型 SHALL 收敛为**单分类（仅 runtime step 节点）**。转换层 SHALL 将运行执行图（`runs[].nodes` 库读史 ∪ 在飞 run 活面，`run-state.ts` 统一视图派生）归并为 runtime step 节点，MUST NOT 再归并 eval（phase attempt）节点或 active（`activePhase`）节点：

- **runtime step 节点**：按后端稳定身份键 `(phase, attempt, step, instanceNo)` 直渲染，running 态 pulse、failed 红态、stopped 灰态、passed 常态；
- **eval / active 相位节点整体退役**：`collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs` 删除，`sequence = [...runNodes]`；运行中由 running 态 runtime 节点承载；header Badge 仍读 `detail.activePhase`（保留，仅移除图上 active 节点）。

`interrupted[]` 留档 SHALL 保持不上图：读模型（Rust `Workflow` / `ChangeDetail`）MUST NOT 解析该字段，前端节点模型 MUST NOT 保留 interrupted 词汇（`FlowNodeKind` 概念整体退役，前端节点类型单一为 `runtime`）；磁盘上既有 `workflow.json` 的 `interrupted[]` 数据 SHALL 原样保留（serde 未知字段忽略），MUST NOT 迁移或清除，写入方（插件 CLI）MUST NOT 因本变更改动。相位列序 SHALL 以 `detail.pipeline` 出线序为准（requirement 10 列、test-only 7 列，initialize 为首列），不依赖 eval 排列、不依赖前端 `PIPELINE_PHASES` 常量。

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

布局 SHALL 采用手工坐标，MUST NOT 引入 dagre / elk 等自动布局引擎：`x = colIndex × COL_W`（`colIndex` = 该 phase 在 `detail.pipeline` 出线序中的下标，列总数 = 该 change `workflow_type` 相位表长度：requirement 10 列、test-only 7 列；未走过的站呈现空列），`y = 列内执行序 × ROW_H`。同 phase 的全部 step 节点 SHALL 纵向对齐成一列。phase 列 SHALL 以 react-flow v12 subflow 承载（`parentId` + `extent: 'parent'`），列头展示 phase 名与该站过程文档徽章（含 eval-checklist 素材，见「过程素材挂载规则」）。前端 `layout.ts` 的 `PIPELINE_PHASES` 常量 SHALL 退役（列序由 `detail.pipeline.map(p => p.phase)` 出线承载，MUST NOT 前端自持静态相位序）。COL_W / ROW_H 常量取值由 design 定夺。视口 SHALL 默认 100% 缩放（MUST NOT 初始 fitView 适配），滚轮 SHALL 平移画布，显式刷新重绘时 SHALL 保留当前视口。

#### Scenario: 列对齐与空列随 workflow_type 恒定

- **WHEN** 某 requirement change 只走到 dev-design 站（initialize 与 proposal 有 step 节点）
- **THEN** initialize 与 proposal 与 dev-design 的 step 节点各自纵向对齐成列，后续 7 站（test-design / implement / test-gen / test-execution / code-review / acceptance / archive）仍以空列呈现，列总数恒为 10（requirement）

#### Scenario: test-only 七列

- **WHEN** 打开某 test-only change（initialize 与 proposal 有 step 节点）
- **THEN** 图为 7 列（initialize / proposal / code-analyze / test-design / test-gen / test-execution / archive），code-analyze 为真实相位列（非占位列），后续未走到的站以空列呈现，列总数恒为 7

#### Scenario: archive 列常驻

- **WHEN** 打开已归档 change（含 archive 相位空 checklist PhaseRecord 与 10 步节点）
- **THEN** archive 列为该 workflow_type 相位表的末列（requirement 第 10 列 / test-only 第 7 列）常驻呈现（10 步节点，无 pass eval 节点），与 test-execution 列同构，不因 refresh 蒸发

#### Scenario: backtrack 重做自动向左

- **WHEN** test-execution#1 fail 后 backtrack 回 test-gen 并重做 test-gen#2
- **THEN** 重做的 test-gen#2 step 节点位于 test-gen 列（Δx < 0），无需显式边分类即呈现为向左回跳

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的 step 节点 SHALL 呈现运行态（pulse 视觉），ToolStep 节点、Git 节点与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（四类可区分）。运行态呈现 SHALL 与既有节点模型并存：step 节点 running / failed / stopped / passed 四态如实；`PIPELINE_PHASES` 10 列布局、边表读取规则与空列恒定 MUST NOT 因运行态呈现改变。eval 节点与 active 节点 SHALL 不再上图（相位节点整体退役，见「流程图节点模型与两分类」）。

运行收口后，图 SHALL 保持**步节点常驻渲染**：已落库执行图节点（`runs[].nodes` 二十一值全词汇）自统一视图派生常驻上图，与运行中实时面走**同一条派生路径**；运行态视觉（pulse）褪去、终态样式呈现。常驻渲染 SHALL 取**全史叠加**（多次 run 的节点全部保留），archive 相位节点随当前 run 常驻。节点身份键 SHALL 为 `(phase, attempt, step, instanceNo)`（自后端直读），前端 `runStepNodes` 的「槽位配对 + `:seq` 后缀堆叠」机制 SHALL 退役。归档步词 SHALL 落到 `RunStepGroup` 的 git / exec / agent 分类（`GROUP_LABEL` 同步），`archive-panel` / `StageList` 六段清单 SHALL 退役（归档六段作为 archive 列 step 节点呈现，不设独立面板）。步词汇归一（库读史 snake 词 → 活面 camel 词，`normalizeKind` / `stepWord` 单点）SHALL 保持二十一值逐词对应：`mutation_test` ↔ `mutationTest`、`cleanup` ↔ `cleanup` 与 `clean` ↔ `clean` 在案、`mutationTest` 归 exec 分类、`cleanup` 与 `clean` 归 git 分类（现有步骤渲染直接吃，零渲染改动）。Cleanup 步的 detail 警告（清理失败 / 跳过注记）SHALL 随节点 detail 面呈现。

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

#### Scenario: initialize clean 步词汇归一

- **WHEN** initialize 相位 Clean 步上图（活面或库读史）
- **THEN** `clean` ↔ `clean` 归一在 `normalizeKind` / `stepWord` 单点可考，节点落 git 分类徽章（人读标签在案）；现有步骤渲染零改动消费

### Requirement: 自动确认发起开关

change 详情页运行控制面板 SHALL 在发起操作区提供「自动确认」开关（默认关）：开启后发起 run 以 `autoNextPhase=true` 调运行控制命令面，关闭则 `false`。开关 SHALL 为发起参数（发起时定格）：运行期间 MUST NOT 可切换；节奏变更 = 停止后重发。auto 模式 run MUST NOT 呈现 phase 间确认卡片（无 `ConfirmWait` 事件到达）；ask 中断卡片 SHALL 照常呈现与应答（ask 不自动应答）。开关旁 SHALL 提供一行说明文案轻提示。归档前停等（stop-before-archive）SHALL 独立于「自动确认」开关：无论 auto / 手动模式，run 走到 archive 前 SHALL 呈现归档确认卡片（含完成度警告 + delta specs + merge 目标），用户确认后才进入 archive 相位（归档不自动推进）。开关 SHALL 以 data-testid 提供测试挂钩。

#### Scenario: 开关默认关与发起传参

- **WHEN** 打开运行控制面板（无运行 run / 终局后），开关保持默认关并点击发起
- **THEN** `changeFlowStart` 携 `autoNextPhase=false`，run 照常在相位落账后停等确认（initialize 收口后照常停等）；开启开关后再发起则携 `autoNextPhase=true`

#### Scenario: auto run 不出 phase 间确认卡片但归档前停等

- **WHEN** auto 模式 run 相位落账 pass 推进下一相位，直至走完全部前序路由相位（含一次性 initialize）
- **THEN** 页面无 phase 间确认卡片；到达 archive 前 SHALL 呈现归档确认卡片（stop-before-archive），未确认不进 archive 相位；确认后 archive 相位执行至收口

#### Scenario: auto run 的 ask 卡片照常

- **WHEN** auto 模式 run 中决策 ask 到来
- **THEN** ask 中断卡片照常呈现，用户应答后 run 继续；确认卡片不因 auto 模式对 ask 生效

#### Scenario: 运行期定格与收口回归

- **WHEN** auto 模式 run 运行中尝试切换开关
- **THEN** 开关不可切换（定格为发起值）；run 收口后控制入口回到发起区，可按任意节奏再次发起

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 步词文案 | `STEP_LABEL` 穷尽 Record 补 `clean` 键（人读文案）；`GROUP_LABEL` git 分类覆盖 clean；零新节点类型 / 零新面板 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | 步词归一 | `normalizeKind` / `stepWord` 单点补 `clean` ↔ `clean` 映射（二十一值逐词对应） |
| `packages/desktop/src/types/generated/bindings.ts`（重生成） | 类型跟随 | `ChangeStepKind` / `RunStepKind` 新变体出线；`bindings:check` 守卫拦截过期生成物 |
| 转换层 / 抽屉 / stdout 面板（零改动） | 结构复用 | initialize 列与步节点经既有 `detail.pipeline` 出线与 runtime step 渲染直达；baseline 输出经既有 `outputRef` stdout 面板回查 |
