# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 流程图节点模型与两分类

change 详情页 SHALL 以 react-flow 流程图承载执行史，节点模型 SHALL 收敛为**单分类（仅 runtime step 节点）**。转换层 SHALL 将运行执行图（`runs[].nodes` 库读史 ∪ 在飞 run 活面，`run-state.ts` 统一视图派生）归并为 runtime step 节点，MUST NOT 再归并 eval（phase attempt）节点或 active（`activePhase`）节点：

- **runtime step 节点**：按后端稳定身份键 `(phase, attempt, step, instanceNo)` 直渲染，running 态 pulse、failed 红态、stopped 灰态、passed 常态；
- **eval / active 相位节点整体退役**：`collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs` 删除，`sequence = [...runNodes]`；运行中由 running 态 runtime 节点承载；header Badge 仍读 `detail.activePhase`（保留，仅移除图上 active 节点）。

`interrupted[]` 留档 SHALL 保持不上图：读模型（Rust `Workflow` / `ChangeDetail`）MUST NOT 解析该字段，前端节点模型 MUST NOT 保留 interrupted 词汇（`FlowNodeKind` 概念整体退役，前端节点类型单一为 `runtime`）；磁盘上既有 `workflow.json` 的 `interrupted[]` 数据 SHALL 原样保留（serde 未知字段忽略），MUST NOT 迁移或清除，写入方（插件 CLI）MUST NOT 因本变更改动。10 站顺序 SHALL 以 `PIPELINE_PHASES` 固定序列为准（proposal → dev-design → test-design → implement → test-gen → test-execution → code-review → acceptance → code-analyze → archive），不依赖 eval 排列。

#### Scenario: 单分类节点呈现

- **WHEN** 打开一个含多 attempt 与 backtrack 的 v2 change 详情
- **THEN** 图上仅呈现 runtime step 节点（WorkerAgent / ToolStep / Gate / Git 类型可辨，running / failed / stopped / passed 状态如实），无 eval 节点、无 active 节点、无 dashed 灰显的 interrupted 节点

#### Scenario: 运行中由 running 态 runtime 节点承载

- **WHEN** 某 change run 运行至 implement 相位（含 active_phase 残留）
- **THEN** 当前执行位置的 runtime step 节点呈 pulse 运行态，图上无独立 active 节点；header Badge 仍读 `detail.activePhase` 呈现「运行中 · implement · attempt N」

#### Scenario: interrupted 留档不出图且数据保留

- **WHEN** 打开 workflow.json 含 `interrupted[]` 留档的存量 v2 change（v2-b 夹具形态：`interrupted` 的 test-gen#1 与 `active_phase` 的 implement#2 均不在 eval 序列内）
- **THEN** test-gen#1 不以任何节点形态出现在图上，磁盘 workflow.json 的 `interrupted[]` 数据原样保留、详情查询不报错

### Requirement: 10 列手工坐标布局

布局 SHALL 采用手工坐标，MUST NOT 引入 dagre / elk 等自动布局引擎：`x = PIPELINE_PHASES.indexOf(phase) × COL_W`（10 列恒定，未走过的站呈现空列），`y = 列内执行序 × ROW_H`。同 phase 的全部 step 节点 SHALL 纵向对齐成一列。phase 列 SHALL 以 react-flow v12 subflow 承载（`parentId` + `extent: 'parent'`），列头展示 phase 名与该站过程文档徽章（含 eval-checklist 素材，见「过程素材挂载规则」）。`PIPELINE_PHASES` SHALL 为 10 列（proposal → dev-design → test-design → implement → test-gen → test-execution → code-review → acceptance → code-analyze → archive，后端 `detail.rs` 与前端 `layout.ts` 双侧同步）。COL_W / ROW_H 常量取值由 design 定夺。视口 SHALL 默认 100% 缩放（MUST NOT 初始 fitView 适配），滚轮 SHALL 平移画布，显式刷新重绘时 SHALL 保留当前视口。

#### Scenario: 列对齐与空列恒定

- **WHEN** 某 change 只走到 dev-design 站（proposal 有 step 节点）
- **THEN** proposal 与 dev-design 的 step 节点各自纵向对齐成列，后续 8 站仍以空列呈现（含 code-analyze 占位列与 archive 列），列总数恒为 10

#### Scenario: archive 列常驻

- **WHEN** 打开已归档 change（含 archive 相位空 checklist PhaseRecord 与 10 步节点）
- **THEN** archive 列为第 10 列常驻呈现（10 步节点，无 pass eval 节点），与 test-execution 列同构，不因 refresh 蒸发

#### Scenario: backtrack 重做自动向左

- **WHEN** test-execution#1 fail 后 backtrack 回 test-gen 并重做 test-gen#2
- **THEN** 重做的 test-gen#2 step 节点位于 test-gen 列（Δx < 0），无需显式边分类即呈现为向左回跳

### Requirement: 过程素材挂载规则

素材挂载 SHALL 为**单层：文档挂列**（列头承载该站全部过程素材，含过程文档与 eval-checklist 信封）。

- 过程文档（`artifacts` 中 `markdown-doc` / `tasks-progress` 类）SHALL 经 **后端 phase 清单归属**挂到 phase 列（列头徽章，每站一份、不随 attempt 重复）：`proposal.md` 与 `specs/**` → proposal、`design.md` 与 `tasks.md` → dev-design、`test-reports/**` → test-execution——归属 SHALL 来自源码 phase 清单（desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）经 detail 出线，前端 `docColumn(source)` 字符串→列静态映射 SHALL 退役。清单未声明归属的文档 SHALL 落入「未归属」处理（design 定夺展示位置），MUST NOT 误挂到错误站。`explore.md` MUST NOT 入图（探索在流程之外，不属于任何站）。
- eval-checklist 信封（payload 自带 `phase` + `attempt` + `verdict` + `items`）SHALL 挂到对应 **phase 列头**（`artifactColumn` 已有 pipeline 外相位校验，`EvalChecklistRenderer` 已能渲染全套元数据，列头抽屉经 `ArtifactTabs` 即达），不再挂节点。

`nodeChecklists` SHALL 退役：`FlowMaterials` 仅保留 `columnDocs`；`NODE_PRECEDENCE` / `locateNode` 删除；`mountMaterials` 签名去 `graph` 参数（连带 `useFlowGraphData` 的 materials memo 依赖收窄为 `[detail, artifacts]`）。file_log 挂载 SHALL 保持退役（file_log 条目挂节点规则与 `scope='workflow'` 图外独立面板随载体退役一并删除）。

归属 SHALL 由后端 phase 清单供给，前端 MUST NOT 保留第二份 docColumn 静态映射；后端 `ChangeDetail` DTO 与 `PIPELINE_PHASES` MUST NOT 因本能力新增 file_log 相关字段。

#### Scenario: 文档按清单归列

- **WHEN** 某 change 的产物含 `proposal.md`、`design.md`、`tasks.md` 与 `test-reports/xxx.md`
- **THEN** proposal 列头徽章含 proposal.md，dev-design 列头徽章含 design.md 与 tasks.md，test-execution 列头徽章含该 test-reports 文档（归属自后端清单，无 docColumn 映射）

#### Scenario: eval-checklist 挂列头

- **WHEN** 某 change 的 implement 站存在带 `phase`/`attempt` payload 的 eval-checklist 信封
- **THEN** 该信封挂 implement 列头（列头抽屉经 `ArtifactTabs` 呈现 checklist 条目与 verdict），`FlowMaterials.nodeChecklists` 无此条目、`mountMaterials` 不接收 `graph`

#### Scenario: 清单未声明文档不误挂

- **WHEN** change 目录存在清单之外的 `.md`（如 `notes.md`）
- **THEN** 该文档不挂任何 phase 列（落入「未归属」处理），MUST NOT 被 docColumn 或扩展名猜测误挂到错误站

#### Scenario: explore.md 隐身与文件面板退役

- **WHEN** 打开任一 change 详情，产物清单含 `explore.md`
- **THEN** explore.md 不出现在图中，图上无文件徽章挂靠，无 workflow 独立面板渲染

### Requirement: 右侧抽屉单一交互入口

phase 列头与 runtime step 节点 SHALL 提供同一交互入口：点击打开右侧抽屉。抽屉 SHALL 为**单列布局**（宽度约 960px 并保留 `max-w-[85vw]`，遮罩点击关闭与右贴边定位不变），内容按选中类型分派：

- **列头**：该站过程文档 + eval-checklist 素材（`columnDocs`，经 `ArtifactTabs` 渲染）；
- **WorkerAgent step 节点**（`role !== null`）：单列会话转录（复用 `SessionTranscriptPanel`，`sessionId` 直查优先 / sourceRef 反查兜底）；
- **带 `outputRef` 的 step 节点**（static-check / test-execution）：单列 stdout 面板（新组件，见「节点点击判据与 stdout 输出面板」）。

抽屉 SHALL 删除 `DrawerEvalSection` + `InlineChecklist` 与 eval/active 分支；评估 report（`node.record.report`）MUST NOT 展示（eval 节点已退役）。无点击判据 SHALL 前置：`sessionId`/`role` 与 `outputRef` 皆 null 的 runtime 节点不可点击（不 `onSelect`），抽屉不会以无信息源选中打开。列头点击与节点点击 MUST NOT 出现两套并存的展开交互。

#### Scenario: WorkerAgent 节点点击单列转录

- **WHEN** 点击 executor / evaluator / decision / specSync / resolveConflicts step 节点（`role !== null`）
- **THEN** 抽屉单列呈该会话转录（`SessionTranscriptPanel`），无「本站文档」节、无「评估记录」节

#### Scenario: 带输出引用节点点击单列 stdout

- **WHEN** 点击 static-check 或 test-execution step 节点（`outputRef !== null`）
- **THEN** 抽屉单列呈 stdout 面板（经 `read_exec_output` 渲染脚本输出），无会话转录区、无文档节

#### Scenario: 列头点击同入口

- **WHEN** 点击 dev-design 列头
- **THEN** 打开与节点点击相同的右侧抽屉，单列呈该站过程文档（design.md / tasks.md）与 eval-checklist 素材，无会话转录区

#### Scenario: 双列结构已退役

- **WHEN** 打开任一节点 / 列头抽屉
- **THEN** 抽屉为单列（无左列会话区 + 右列文档区的双列切分），内容随选中类型分派，抽屉本体 MUST NOT 退回双列布局

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的 step 节点 SHALL 呈现运行态（pulse 视觉），ToolStep 节点、Git 节点与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（四类可区分）。运行态呈现 SHALL 与既有节点模型并存：step 节点 running / failed / stopped / passed 四态如实；`PIPELINE_PHASES` 10 列布局、边表读取规则与空列恒定 MUST NOT 因运行态呈现改变。eval 节点与 active 节点 SHALL 不再上图（相位节点整体退役，见「流程图节点模型与两分类」）。

运行收口后，图 SHALL 保持**步节点常驻渲染**：已落库执行图节点（`runs[].nodes` 十八值全词汇）自统一视图派生常驻上图，与运行中实时面走**同一条派生路径**；运行态视觉（pulse）褪去、终态样式呈现。常驻渲染 SHALL 取**全史叠加**（多次 run 的节点全部保留），archive 相位节点随当前 run 常驻。节点身份键 SHALL 为 `(phase, attempt, step, instanceNo)`（自后端直读），前端 `runStepNodes` 的「槽位配对 + `:seq` 后缀堆叠」机制 SHALL 退役。归档步词 SHALL 落到 `RunStepGroup` 的 git / exec / agent 分类（`GROUP_LABEL` 同步），`archive-panel` / `StageList` 六段清单 SHALL 退役（归档六段作为 archive 列 step 节点呈现，不设独立面板）。

#### Scenario: 运行态与四类并存

- **WHEN** implement#2 相位运行中且历史上有 fail→retry 与 active_phase 残留，archive 相位随后进入
- **THEN** 图上同时呈现：历史 step 节点（终态）、当前节点（pulse 运行态）及同相位的 ToolStep / Git / Gate 节点（类型可辨、状态如实），无 eval 节点、无 active 节点

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

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision / spec_sync / resolve_conflicts）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线，点击已收口的 WorkerAgent 节点 SHALL 打开转录重放。会话寻址 SHALL 记录 id 直查优先：节点记录暴露的会话槽位 id（`sessionId`）在场时按 session id 直查；槽位缺席时回退既有 sourceRef 定式反查（身份段为 change id；归档步两会话的 sourceRef 定式由 design 定稿，MUST 在同 phase 同 attempt 内唯一）。归档步节点（spec_sync / resolve_conflicts）SHALL 经其节点 `session_id` 直查转录（无 attempt 三槽位概念）。

eval 节点与 active 节点的三会话联动（executor / evaluator / decision 三 tab）SHALL 整体退役（eval / active 节点已移除）；ToolStep / Git / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果——无 role 且无 `outputRef` 的 ToolStep / Git / Gate 节点不可点击（`detail` 短文本已在节点内显示）；带 `outputRef` 的 static-check / test-execution 节点经 stdout 面板呈现输出（见「节点点击判据与 stdout 输出面板」）。

#### Scenario: 归档步节点转录直查

- **WHEN** 点击 archive 相位的 SpecSync 或 ResolveConflicts 节点（`session_id` 在案）
- **THEN** 抽屉单列经节点 `session_id` 直查该会话转录（实时流 / 重放一致）

#### Scenario: 槽位缺席回退反查

- **WHEN** 打开 WorkerAgent step 节点且其 `sessionId` 缺席（缺省落账）
- **THEN** 转录面板回退 sourceRef 定式反查（身份段 = change id）并照常重放，行为与直查路径一致可靠

#### Scenario: 运行中节点实时转录经转录库

- **WHEN** executor 会话运行中点击其 WorkerAgent 节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），数据经转录库重查呈现（通知触发重查，非内存 liveEvents 累积），与 Agent 调试页同款组件

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

#### Scenario: 记录 id 直查优先

- **WHEN** 某节点记录携带会话槽位 id，且同 sourceRef 定式下存在多条会话
- **THEN** 转录面板按槽位 id 精确直查该会话，MUST NOT 落入「同 ref 多会话取最近一条」的反查歧义

#### Scenario: 查看会话按钮不呈现

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点上无「查看会话」按钮，点击节点本体即打开抽屉查看转录；无独立会话 route 引入

### Requirement: 纯函数转换层与渲染薄层

`detail → { nodes, edges }` 转换 SHALL 为独立纯函数模块（`views/changes/` 下，目录与命名由 design 定夺），不依赖 react 宿主组件状态、不发起任何 invoke，vitest SHALL 直测其边界：前进 / 重试 / 回跳 / 节点派生（step 节点直读稳定身份键）/ attempt 缺号 / 时间戳缺失 / 文档归属映射 / workflow scope 剔除。react-flow SHALL 仅作渲染薄层，其库内布局与交互语义 MUST NOT 做逐项断言（符合「不逐项验证库语义」约定）。

#### Scenario: 转换层直测

- **WHEN** 以 v2-a / v2-b 夹具形状的 `ChangeDetail` 数据（含 fail→retry、backtrack）调用转换函数
- **THEN** 输出的节点坐标符合列索引 / 执行序公式，边集合与方向符合时间序推导，节点集仅含 runtime step 节点，无需挂载 react 组件即可断言

## ADDED Requirements

### Requirement: 节点点击判据与 stdout 输出面板

runtime step 节点点击 SHALL 按信息源分派判据：`role !== null` → 单列会话转录（复用 `SessionTranscriptPanel`）；`outputRef !== null` → 单列 stdout 面板（新组件）；两者皆 null → 不可点击（不 `onSelect`）。该判据 SHALL 为转换 / 交互层纯函数可测（不依赖 react 宿主）。

stdout 面板 SHALL 经 `hooks/use-exec-output.ts` 取数：以 `(root, changeName, outputRef)` 调 `commands.readExecOutput(root, changeName, execId)`（返回 `Option<String>`，UTF-8 lossy；miss=None），将脚本真实输出渲染为单列文本区；miss 或读回失败 SHALL 呈空态 / 错误态（不炸图）。`read_exec_output` 磁盘面按 change name 定位（与 `record_exec_output` 同口径），`DetailDrawer` SHALL 新增 `name` prop（取 `detail.name`，非 id）供 stdout 面板寻址。

#### Scenario: 有 role 节点分派到转录

- **WHEN** 点击 `role !== null` 的 runtime step 节点
- **THEN** 抽屉分派到会话转录单列，不发起 `read_exec_output`

#### Scenario: 有 outputRef 节点分派到 stdout

- **WHEN** 点击 `outputRef !== null` 的 runtime step 节点（static-check / test-execution）
- **THEN** 抽屉分派到 stdout 面板，经 `readExecOutput(root, name, outputRef)` 渲染脚本真实输出

#### Scenario: 双 null 节点不可点击

- **WHEN** 点击 phaseStart / phaseLog / verdictGate / retryGate / whitelistGate / commit / rebase / merge / finalize / preflight / seal 节点（`role` 与 `outputRef` 皆 null）
- **THEN** 不触发 `onSelect`、抽屉不打开，节点本身仍照常渲染

#### Scenario: stdout 面板空态与错误态

- **WHEN** `readExecOutput` 返回 `None`（exec_id 未知 / 非法）或读回失败
- **THEN** stdout 面板呈空态 / 错误态提示，流程图与抽屉不报错、不崩溃

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `flow/graph.ts` + `types.ts` | 节点模型单分类收敛 | `FlowNode = RuntimeFlowNode`；删 `FlowNodeKind` / `EvalFlowNode` / `ActiveFlowNode` / `collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs`；`sequence = [...runNodes]`；`pipeline` 参数保留（backtrack label） |
| `flow/attachments.ts` | 素材单层挂列 | `FlowMaterials` 仅 `columnDocs`；eval-checklist 挂列头；删 `NODE_PRECEDENCE` / `locateNode` / `nodeChecklists`；`mountMaterials(detail, artifacts)` |
| `flow/change-flow-graph.tsx` | 渲染薄层交互判据 | 删 `nodeTypes.event` 与 event 通道；`onNodeClick` 按 role / outputRef 双 null 判据拦截；runtime 节点照常上抛 |
| `flow/flow-event-node.tsx` | **（删除）** | 事件节点组件与测试随相位节点退役删除 |
| `flow/detail-drawer.tsx` | 抽屉单列分派 | 删 `DrawerEvalSection` / `InlineChecklist` / eval / active 分支；按 `role` → 转录、`outputRef` → stdout 分派；新增 `name` prop |
| `flow/stdout-panel.tsx` + `hooks/use-exec-output.ts` | stdout 展示 | `readExecOutput(root, name, execId)` 取数；UTF-8 lossy；miss 空态 / 错误态 |
| `change-detail-view.tsx` | 数据源与透传 | materials memo 依赖 `[detail, artifacts]`；`DetailDrawer` 传 `name={detail.name}` |
| `run-state.ts` | 零触点（除消费面） | 统一视图派生与词汇归一不变；`outputRef` / `sessionId` / `role` 已出线，零新增词汇 |
| `types/generated/bindings.ts` | 类型再生跟随 | `readExecOutput` 命令 typed 包装出线；一致性守卫拦截漂移 |
