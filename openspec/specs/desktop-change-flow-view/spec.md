# desktop-change-flow-view Specification

## Purpose

定义 desktop change 详情页的 react-flow 流程图视图：attempt 级两分类节点（eval / active）、9 列手工坐标布局、时间序边推导、过程素材两层挂载、右侧抽屉单一交互入口与建档单态呈现；转换层为纯函数、渲染薄层复用 react-flow v12。

## Requirements

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

### Requirement: 时间序边推导

执行图边 SHALL 由后端边表（`runs[].edges`，desktop-change-state-store「run 执行图落库」）直接读取，**不再由列差 Δ 派生**：`deriveEdges` 按 `colIndex` 差派生 forward（Δ>0）/ retry（Δ=0）/ backtrack（Δ<0）的逻辑 SHALL 退役。边 SHALL 以 `edgeKind ∈ { forward, feedback, retry, backtrack }` 显式呈现（四值词汇为 `RunEdgeKind` 的解码契约，前端类型面不收缩）：

- **forward** 计划序推进（向右或同列计划步）；
- **feedback** gate 失败回到同 phase agent 步的 step 级回边（phase 内回边，新增）；
- **retry** phase fail 后同相位重跑——该语义已随决策协议退役（重做本相位改由回溯到自身表达，入口边为 `backtrack`），存量 `retry` 边由查询层投影屏蔽（desktop-change-queries「change 详情聚合」），投影面 MUST NOT 出现该 kind；前端样式分支保留默认渲染，MUST NOT 视为可达形态；
- **backtrack** 跨相位回跳（虚线 + `backtrackReason` 边标签；目标为失败相位自身即「重做本相位」的重做入口）。

边端点 SHALL 以节点稳定身份键 `(phase, attempt, step, instanceNo)` 寻址（取代「时间序扫描串联 + active 按 startAt 归并插入」）；节点身份键自 `runs[].nodes` 直读（desktop-change-queries「change 详情聚合」），MUST NOT 在前端重新派生节点序。attempt 缺号 SHALL 按 0 兜底（与后端 `detail.rs` 的 `unwrap_or(0)` 同款），startAt 缺失的事件 SHALL 按列表序归并、不丢节点。边端点缺失（历史投影被屏蔽的 `retry` 行、或节点未出线）SHALL 跳过该边且不报错——图面照常渲染其余边。

#### Scenario: 四类边自边表呈现

- **WHEN** 执行史含 proposal#1 → dev-design#1 fail → dev-design#2 pass（决策回溯到自身重做的同列入口边）、test-execution#1 → test-gen#2（跨站 backtrack）、implement 内 static-check fail → executor 修复（phase 内 feedback），且存量库含 `retry` 边
- **THEN** 图边自 `runs[].edges` 直读：同列重做入口呈 `backtrack`（虚线、带 `backtrackReason`）、跨站回跳呈 `backtrack`、phase 内回边呈 `feedback`、forward 边计划序推进；`retry` 边不出现（查询层已屏蔽，端点缺失亦跳过）；无 `deriveEdges` Δ 派生逻辑

#### Scenario: 缺号与缺时间戳兜底

- **WHEN** 某事件 `attempt` 为 null 或 startAt 为 null
- **THEN** 节点不丢失：attempt 以 0 兜底参与列内排序，startAt 缺失者按列表序归并，转换不报错

#### Scenario: retry 边缺席不误渲染

- **WHEN** 以含 `edgeKind: 'retry'` 的边集夹具调用转换函数（模拟旧库未屏蔽或夹具直构）
- **THEN** 该边按默认样式渲染或按端点缺失跳过，转换不 panic、不产生额外派生边；正常运行的图面不出现该 kind（投影面屏蔽为其唯一来源保证）

### Requirement: 过程素材挂载规则

素材挂载 SHALL 为**单层：文档挂列**（列头承载该站全部过程素材，含过程文档与 eval-checklist 信封）。

- 过程文档（`artifacts` 中 `markdown-doc` / `tasks-progress` 类）SHALL 经 **后端 phase 清单归属**挂到 phase 列（列头徽章，每站一份、不随 attempt 重复）：`proposal.md` 与 `specs/**` → proposal、`design.md` 与 `tasks.md` → dev-design、`test-reports/**` → test-execution——归属 SHALL 来自源码 phase 清单（desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）经 detail 出线，前端 `docColumn(source)` 字符串→列静态映射 SHALL 退役。清单未声明归属的文档 SHALL 落入「未归属」处理（design 定夺展示位置），MUST NOT 误挂到错误站。`explore.md` MUST NOT 入图（探索在流程之外，不属于任何站）。
- eval-checklist 信封（payload 自带 `phase` + `attempt` + `verdict` + `items`）SHALL 挂到对应 **phase 列头**（`artifactColumn` 已有 pipeline 外相位校验，`EvalChecklistRenderer` 已能渲染全套元数据，列头抽屉经 `ArtifactTabs` 即达），不再挂节点。

`nodeChecklists` SHALL 退役：`FlowMaterials` 仅保留 `columnDocs`；`NODE_PRECEDENCE` / `locateNode` 删除；`mountMaterials` 签名去 `graph` 参数（连带 `useFlowGraphData` 的 materials memo 依赖收窄为 `[detail, artifacts]`）。file_log 挂载 SHALL 保持退役（file_log 条目挂节点规则与 `scope='workflow'` 图外独立面板随载体退役一并删除）。

归属 SHALL 由后端 phase 清单供给，前端 MUST NOT 保留第二份 docColumn 静态映射；后端 `ChangeDetail` DTO 与 `PIPELINE_PHASES` MUST NOT 因本能力新增 file_log 相关字段。

#### Scenario: 文档按清单归列

- **WHEN** 某 change 的产物含 `proposal.md`、`design.md`、`tasks.md` 与 `test-reports/xxx.md`
- **THEN** proposal 列头徽章含 proposal.md，dev-design 列头徽章含 design.md 与 tasks.md，test-execution 列头徽章含该 test-reports 文档（归属自后端清单，无 docColumn 映射）

#### Scenario: 清单未声明文档不误挂

- **WHEN** change 目录存在清单之外的 `.md`（如 `notes.md`）
- **THEN** 该文档不挂任何 phase 列（落入「未归属」处理），MUST NOT 被 docColumn 或扩展名猜测误挂到错误站

#### Scenario: explore.md 隐身与文件面板退役

- **WHEN** 打开任一 change 详情，产物清单含 `explore.md`
- **THEN** explore.md 不出现在图中，图上无文件徽章挂靠，无 workflow 独立面板渲染

#### Scenario: eval-checklist 挂列头

- **WHEN** 某 change 的 implement 站存在带 `phase`/`attempt` payload 的 eval-checklist 信封
- **THEN** 该信封挂 implement 列头（列头抽屉经 `ArtifactTabs` 呈现 checklist 条目与 verdict），`FlowMaterials.nodeChecklists` 无此条目、`mountMaterials` 不接收 `graph`

### Requirement: 右侧抽屉单一交互入口

phase 列头与 runtime step 节点 SHALL 提供同一交互入口：点击打开右侧抽屉。抽屉 SHALL 为**单列布局**（宽度约 960px 并保留 `max-w-[85vw]`，右贴边定位不变），内容按选中类型分派：

- **列头**：该站过程文档 + eval-checklist 素材（`columnDocs`，经 `ArtifactTabs` 渲染）；
- **WorkerAgent step 节点**（`role !== null`）：单列会话转录（复用 `SessionTranscriptPanel`，按节点 `sessionId` 直查；槽位缺席时呈「此步无会话记录」空态）；
- **带 `outputRef` 的 step 节点**（static-check / test-execution）：单列 stdout 面板（新组件，见「节点点击判据与 stdout 输出面板」）。

抽屉 SHALL 删除 `DrawerEvalSection` + `InlineChecklist` 与 eval/active 分支；评估 report（`node.record.report`）MUST NOT 展示（eval 节点已退役）。点击判据 SHALL 前置：`role` 与 `outputRef` 皆 null 的 runtime 节点不可点击（不 `onSelect`）；`role !== null` 而 `sessionId === null` 的节点仍可点击，抽屉呈「此步无会话记录」空态占位（不再回退 sourceRef 反查）。列头点击与节点点击 MUST NOT 出现两套并存的展开交互。

抽屉 SHALL 为非模态形态（与左栏通用 agent 面板并读的前提，见 desktop-change-chat）：

- **无遮罩**：抽屉 SHALL 以非模态方式打开（`modal={false}`——遮罩仅在模态时渲染、非模态时容器不吃指针事件），MUST NOT 遮蔽页面其余区域；抽屉打开期间左栏通用 agent 面板的输入与发送 SHALL 保持可交互。
- **关闭入口**：非模态形态下无「点遮罩关闭」——关闭 SHALL 经页脚「关闭」按钮与 Esc；关闭仍等价于上游把选中置 null（`selection = null`），MUST NOT 引入第二套展开 / 折叠交互。
- **缺省不回归**：承载抽屉的标准组件 SHALL 以 `modal` 缺省 `true` 保持既有其它调用方行为（模态 + 遮罩）不变，仅 change 详情页的抽屉传 `modal={false}`。
- **上游不动**：抽屉仍由选中态驱动（`selection !== null`）、仍是列头与节点的唯一交互入口，内容分派（会话转录 / stdout / 列头素材）与宽度不变。
- **窄窗口取舍**：960px + `max-w-[85vw]` 在窄窗口仍可能盖住左栏 chat——此为既定取舍（需要并读时收起抽屉），MUST NOT 以「抽屉改为内嵌分栏」解决。

#### Scenario: 节点点击抽屉双列内容

- **WHEN** 点击 implement#2（fail → retry pass 的第二个 attempt，会话记录在场）节点
- **THEN** 左列呈该 attempt 的会话元信息与 executor / evaluator / decision 转录 tab，右列自上而下为该站文档、attempt#2 的 report 与 checklist，无文件表节

#### Scenario: 列头点击同入口左列空态

- **WHEN** 点击 dev-design 列头
- **THEN** 打开与节点点击相同的右侧抽屉，右列本站文档节含 design.md / tasks.md，左列呈空态占位而非整列消失

#### Scenario: 双列各自滚动

- **WHEN** 某 attempt 的会话转录、report 与 checklist 内容均超出可视高度
- **THEN** 左列时间线在左列内滚动、右列二分节在右列内滚动，抽屉本体 MUST NOT 退回整列单滚

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

#### Scenario: 非模态抽屉与左栏 chat 并读

- **WHEN** 打开某 WorkerAgent 节点抽屉（左侧通用 agent 面板正处于可发送态）后，在左栏输入并发送一条消息
- **THEN** 页面无遮罩节点（抽屉容器非模态）、左栏输入与发送全程可交互并正常发出；抽屉内容随选中类型照常呈现

#### Scenario: 非模态关闭入口

- **WHEN** 非模态抽屉打开时按 Esc 或点击页脚「关闭」按钮
- **THEN** 抽屉关闭（选中置 null），页面回到仅有分栏的形态；MUST NOT 需要点击遮罩（非模态下不存在遮罩）

#### Scenario: 其它调用方仍模态

- **WHEN** 审查标准抽屉组件的其它使用方（未显式传 `modal`）
- **THEN** 其行为保持模态（渲染遮罩、点遮罩关闭），不因 change 详情页的改动而变更

### Requirement: 三代际降级

视图 SHALL 按建档状态单态呈现（文档形态分支随清单 db 单源整体退役；inventory 代际词汇保持退役）：

- **已建档**（db `ChangeRecord` 在案——详情按 id 寻址，命中即恒有记录）：SHALL 呈现完整状态面——流程图（eval / active 节点照常）、抽屉与产物区照常；
- **未找到**（db 无该 id 记录）：error / loading / 未找到降级页 SHALL 沿用现状；MUST NOT 呈现文档形态空图占位（`flow-empty`）或其文案。

`detail.status` MUST NOT 再承担建档判别（详情 DTO 恒有记录，`status` 仅表达 active / archived）——「建档两态分流」语义随文档形态退役退场；`flow-empty` 占位分支、「（文档形态：未建档，仅产物清单）」文案与其测试断言 SHALL 一并删除。inventory 代际徽章（V0 / V1 / V2）与 `unparsable`（workflow.json 损坏）警示 SHALL 保持退役：清单与详情的前端词汇清零（组件、DTO 消费与测试断言一并移除）。

#### Scenario: 建档 change 完整状态面

- **WHEN** 打开一个 db 已建档、含多 attempt 与 backtrack 的 change 详情
- **THEN** 流程图正常绘制 eval / active 节点，无代际徽章出现，抽屉与产物区照常

#### Scenario: 归档 change 不落空面

- **WHEN** 打开归档 change 详情（磁盘目录名带日期前缀，经 id 寻址）
- **THEN** 流程图（含 runs 步节点）与产物区照常呈现，MUST NOT 出现「文档形态」空图占位或任何建档判别分支

#### Scenario: 未找到 id 降级

- **WHEN** 打开 db 无记录的 id（含磁盘存在同名目录的情形）
- **THEN** 呈现未找到降级页，无空图占位、无「文档形态」文案，页面不报错

#### Scenario: 代际与警示词汇清零

- **WHEN** 扫描清单 / 详情视图源码与测试对 `inventory` / `unparsable` / 「无法解析」/ `flow-empty` / 「文档形态」的消费
- **THEN** 零消费残留；knip 无新增豁免，全量类型检查绿

### Requirement: 取数模型与管线合规不变

视图改造 MUST NOT 改变取数契约：详情更新 SHALL 仅由显式 refresh（或选中 change 变化）触发，MUST NOT 引入 watch 订阅、定时轮询或事件订阅（live 刷图留作后续独立 change）。**例外（desktop-change-flow 升级）**：change run 运行期间，执行视图 SHALL 经运行状态 Channel 接收**变更通知**并触发统一视图重查（执行流通道例外，沿 agent 执行先例，不属轮询取数；通知仅失效信号、查询结果为权威——unify-run-state-persistence 推拉反转，原「Channel 刷新 MUST NOT 触发 `get_change_detail` 全量重取（节点状态为增量并入）」条款随之退场），run 收口后 MUST NOT 保持订阅。前端 run 状态机镜像 SHALL 解散：`applyRunUpdate` 载荷累积、liveEvents 事件缓存与 seq 去重随通知降位退役，客户端态退化为「查询缓存 + 失效重取」；两钩并一（`useChangeFlowRun` 缩为控制动作 + 通知订阅，统一视图取数由详情取数承接——合并查询契约见 desktop-change-orchestration「运行控制命令面」）。desktop 前端 SHALL 维持全管线通过：`vp check --fix`（fmt / lint，含 `max-lines-per-function: 50`）、knip（无未用导出残留）、`vp test` 全绿；测试 SHALL 以 data-testid 为查询挂钩，MUST NOT 以样式类名查询。

#### Scenario: 运行中通知触发重查

- **WHEN** change run 运行期间某相位推进（节点状态变化通知到达）
- **THEN** 执行视图重查统一视图（库读史 ∪ 在飞 run）并重绘，无前端载荷累积 / seq 去重逻辑；run 收口后订阅释放，后续变更回到显式刷新

#### Scenario: 运行外显式刷新仍是唯一更新途径

- **WHEN** 无运行中 change run 时，流程图呈现期间 change 的库状态被外部进程修改
- **THEN** 图不自动变化；点击「刷新详情」后经 `get_change_detail` 重取并重绘

#### Scenario: 全管线通过

- **WHEN** 运行 `pnpm -C packages/desktop run client:check` 与 `pnpm -C packages/desktop run test`
- **THEN** fmt / lint / knip / 测试全部通过，`package.json` 与 knip.json 无新增豁免条目

### Requirement: 纯函数转换层与渲染薄层

`detail → { nodes, edges }` 转换 SHALL 为独立纯函数模块（`views/changes/` 下，目录与命名由 design 定夺），不依赖 react 宿主组件状态、不发起任何 invoke，vitest SHALL 直测其边界：前进 / 重试 / 回跳 / 节点派生（step 节点直读稳定身份键）/ attempt 缺号 / 时间戳缺失 / 文档归属映射 / workflow scope 剔除。react-flow SHALL 仅作渲染薄层，其库内布局与交互语义 MUST NOT 做逐项断言（符合「不逐项验证库语义」约定）。

#### Scenario: 转换层直测

- **WHEN** 以 v2-a / v2-b 夹具形状的 `ChangeDetail` 数据（含 fail→retry、backtrack）调用转换函数
- **THEN** 输出的节点坐标符合列索引 / 执行序公式，边集合与方向符合时间序推导，节点集仅含 runtime step 节点，无需挂载 react 组件即可断言

### Requirement: desktop 版本升级交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.3.2` 升级为 `0.3.3`。`src-tauri/tauri.conf.json` SHALL 维持 `../package.json` 引用（版本自动跟随，MUST NOT 引入硬编码版本号）；`src-tauri/Cargo.toml` 的 `[package].version`（`0.1.0`）MUST NOT 随 desktop 版本变动（既有惯例：0.3.0 / 0.3.1 两次升级均只改 package.json）。

#### Scenario: 版本号升级与引用跟随

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` 的 `version` 为 `0.3.3`，`tauri.conf.json` 无版本号硬编码改动，`src-tauri/Cargo.toml` 版本保持 `0.1.0`

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

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision / spec_sync / resolve_conflicts）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线，点击已收口的 WorkerAgent 节点 SHALL 打开转录重放。会话寻址 SHALL 为节点 `sessionId` 直查：节点记录暴露的会话槽位 id（`sessionId`）经会话查询面精确寻址（`sessionDetail` + `agentSessionTranscript`），前端 MUST NOT 再以 `agentSessions` 按 `source_ref` 过滤反查会话（sourceRef 反查路径整体退役）。呈现 SHALL 经会话级统一组件 `AgentSessionView`（desktop-agent-chat-infra，只读消费点：不接 composer 插槽、不呈现发送 / 停止）：运行中会话的事件增量 SHALL 经会话订阅面 `agent_session_watch`（载荷直投，含 delta）呈现 token 级流式，MUST NOT 以「通知 → 时间去抖重查转录库」作为实时面（delta 不落库，重查只能见密封粒度）；轮收口 SHALL 以转录库重放收敛（收口后形状与重开一致）。变更通知中的会话事件（`SessionEvent`）SHALL 仅作会话状态重评估信号（触发详情重查 → 组件重评估是否挂流），MUST NOT 作为增量数据源。槽位缺席（`role !== null` 而 `sessionId === null`）时 SHALL 呈「此步无会话记录」步面空态占位，MUST NOT 回退反查、MUST NOT 虚构会话、MUST NOT 建立任何订阅。归档步节点（spec_sync / resolve_conflicts）SHALL 同样经其节点 `sessionId` 直查转录（无 attempt 三槽位概念）。后端 provenance `source_ref` 定式（`<id>/<phase>/<role>/<attempt>`）仍由 walker 在会话创建时写入并持久化于会话记录（见 desktop-change-orchestration「会话挂靠 provenance 与转录 session id 直查」），但前端转录面板不再以其作反向寻址依据。

eval 节点与 active 节点的三会话联动（executor / evaluator / decision 三 tab）SHALL 整体退役（eval / active 节点已移除）；ToolStep / Git / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果——无 role 且无 `outputRef` 的 ToolStep / Git / Gate 节点不可点击（`detail` 短文本已在节点内显示）；带 `outputRef` 的 static-check / test-execution 节点经 stdout 面板呈现输出（见「节点点击判据与 stdout 输出面板」）。

#### Scenario: 归档步节点转录直查

- **WHEN** 点击 archive 相位的 SpecSync 或 ResolveConflicts 节点（`session_id` 在案）
- **THEN** 抽屉单列经节点 `session_id` 直查该会话转录（实时流 / 重放一致）

#### Scenario: 槽位缺席呈步面空态

- **WHEN** 打开 WorkerAgent step 节点且其 `sessionId` 缺席（缺省落账）
- **THEN** 转录面板呈「此步无会话记录」空态占位（react 面零取数），MUST NOT 回退 sourceRef 定式反查、MUST NOT 发起 `agent_sessions` 反向查询

#### Scenario: 运行中节点实时转录经会话订阅面

- **WHEN** executor 会话运行中点击其 WorkerAgent 节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），数据经会话订阅面 `agent_session_watch` 载荷直投呈现（含 `messageDelta`），与 Agent 调试页同款组件；收口后以转录库重放收敛

#### Scenario: 通知仅作状态重评估

- **WHEN** 运行中收到会话事件通知（`SessionEvent`）而该会话订阅流已在挂
- **THEN** 通知只触发会话状态重评估（详情重查 / 是否重挂判断），MUST NOT 触发转录库重查以「追增量」、MUST NOT 重建第二条订阅流

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

#### Scenario: 记录 id 精确直查

- **WHEN** 某节点记录携带会话槽位 id
- **THEN** 转录面板按槽位 id 精确直查该会话，MUST NOT 落入「同 ref 多会话取最近一条」的反查歧义（前端不再持有 sourceRef 定式）

#### Scenario: decision 第三 tab 与空态

- **WHEN** 点击某 attempt 的 decision 槽位节点，该槽位在场（或缺席）两种形态分别驱动
- **THEN** 该槽位在场时按 id 直查其转录，缺席时呈「此步无会话记录」空态占位，其余 role 节点不受影响

#### Scenario: 会话槽位缺席不虚构

- **WHEN** 打开 `role !== null` 而 `session_id` 为空（未开跑 / 缺省落账）的 WorkerAgent 步节点
- **THEN** 抽屉呈「此步无会话记录」空态占位，MUST NOT 经 provenance 反查虚构会话、全程不订阅实时事件流

#### Scenario: 查看会话按钮不呈现

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点上无「查看会话」按钮，点击节点本体即打开抽屉查看转录；无独立会话 route 引入

### Requirement: 运行控制入口与 ask 应答

change 详情页 SHALL 提供运行控制入口：发起 run（按 change id——执行锚与磁盘 / git 面经 id → 记录解析，见 desktop-change-orchestration）、停止运行中的 run、phase 间停等点的用户确认（继续 / 终止）。决策协议 `ask` 中断 SHALL 在页面呈现中断卡片（问题 + 选项），用户应答（选项或自由文本）后经应答命令回流驱动 run 继续；停止与发起 SHALL 沿运行控制命令面契约（提前 resolve、停止幂等）。控制入口状态 SHALL 如实反映 run 生命周期（可发起 / 运行中可停止 / ask 等待应答 / phase 间等待确认），MUST NOT 出现状态与可用操作错配（如已收口仍显停止入口为可用主操作）。

#### Scenario: phase 间确认续走

- **WHEN** walker 完成一相位推进进入 phase 间停等点
- **THEN** 页面呈现等待确认状态；用户确认后 run 继续下一相位，选择终止则 run 收敛为受控终态

#### Scenario: ask 中断卡片应答

- **WHEN** 决策 agent 输出 ask 动作（问题 + 选项）后用户在卡片中选择某选项
- **THEN** 应答经命令回流 walker，run 按应答继续，中断卡片消失、控制入口回到运行态

#### Scenario: 状态与操作不错配

- **WHEN** run 已 fail 收口（或被用户停止）
- **THEN** 页面控制入口回到「可发起」态，停止入口不再作为可用主操作呈现

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

### Requirement: change 标题渲染

change 清单条目 SHALL 渲染 `summary.title`（人类可读标题，恒非空——自 `ChangeSummary` 直读）而非 `summary.name`；change 详情头 SHALL 渲染 `detail.title`（自 `ChangeDetail` 直读）而非 `detail.name`。`name` 保持裸名语义（kebab-case 身份面），SHALL 仍可经既有信息面 / 归档面板等以 `name` 参数使用；渲染标题时不引入 fallback（title 恒非空不变量由后端保证）。测试挂钩 SHALL 使用 data-testid，MUST NOT 以样式类名作查询挂钩。

#### Scenario: 清单渲染 title

- **WHEN** 某 change 的 `title` 为「API 重试调研」而 `name` 为 `api-retry`
- **THEN** 清单条目显示「API 重试调研」为标题，`name` 不承担标题展示

#### Scenario: 详情头渲染 title

- **WHEN** 打开该 change 详情
- **THEN** 详情头显示「API 重试调研」，归档 / 运行中徽标与 worktree 信息行照常呈现

#### Scenario: 无 fallback 分支

- **WHEN** 扫描清单 / 详情视图对 title 的渲染逻辑
- **THEN** 无「title 为空回退 name」的前端分支（后端 title 恒非空）；data-testid 挂钩在案，无样式类名查询挂钩

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

### Requirement: 通知重查失败降级与有界重试

「通知仅失效信号、查询结果权威」的取数模型（见「取数模型与管线合规不变」）维持不变；本 requirement 追加**失败轮的降级契约**：

**失败轮保 last-good（信号界限主条款）**：通知（或显式刷新）触发的统一视图重查失败轮（`get_change_detail` invoke reject——读 `Err`，见 desktop-change-queries「详情查询错误语义分流（None / Err）」）在 detail 在手时 SHALL 保 last-good：`detail` / `activeRun` / `artifacts` SHALL NOT 被清空或回退占位（无 nodes=0 空图、无加载占位塌面、无「未找到该 change」兜底页）；陈旧窗口内 SHALL 以**内联「刷新失败」提示**如实标示（呈现位置与 data-testid 挂钩由 design 定夺），MUST NOT 伪造新鲜态（无提示地呈现旧图如新）。保 last-good 期间以**最后一次成功查询**为权威：面板动作（停止 / phase 间确认 / ask 应答）SHALL 照常可用（动作为独立 invoke，不依赖前端态），运行控制入口状态 SHALL 与 last-good `activeRun` 一致。首载失败（detail 不在手）SHALL 维持既有整页错误兜底页；重查成功且返回 `None`（真「未找到」）SHALL 维持清面兜底页语义不变。

**有界重试**：失败轮 SHALL 自带重试——最多 3 次、退避 1s / 2s / 4s（常量单点）。重试为「自动补发的刷新意图」：任一时刻至多一个待生效的刷新意图，重试计时器与通知去抖 SHALL 共用同一条取消通道——任何 tick 变化（新通知到达、手动刷新、换 root / id、卸载）即作废挂起的重试并重置退避序列，新意图天然取代旧重试（不产生交错重复查询）；重试期间用户动作（停止 / 确认 / 应答）不受取消语义影响。重试穷尽后 SHALL 静默回落「靠下一条通知兜底」。终态竞态洞由内建重试封堵：walker 写序 `run_finished`（整包落库）→ `guard.finish`（发通知），`Finished` 为 run 最后一条通知——终态重查若失败且只靠通知恢复则永无下一条，故重试必须自带。

**陈旧窗口有界（时间界限辅条款）**：保 last-good 的陈旧窗口 SHALL 有界——≤ 3 次退避（1s / 2s / 4s）内自愈，或下一条通知到达即恢复，最终一致。

**订阅粘性**：失败轮保 last-good 期间 `activeRun` SHALL 维持在场值（不清）→ `activeRunPresent` 保持真值，`change_flow_watch` 订阅 MUST NOT 因单轮失败被 effect cleanup 拆除（订阅生命周期口径不变，粘性随保 last-good 自然达成；不做「mount 即订阅」等独立改造）。

**立法辨析**：desktop-change-queries「详情标示『运行中』SHALL 与实际在飞 run 一致 / MUST NOT 假象」条款强度不变——其禁杀的是**无界、静默**的假态（archive 后挂「运行中」的旧 bug）；本例外豁免的是**有界、带「刷新失败」内联提示、收敛保证**的过渡态，措辞上不稀释原条款。

#### Scenario: 失败轮保 last-good 与内联提示

- **WHEN** 运行中某通知触发的统一视图重查轮 invoke reject（store 读 `Err`）且 detail 在手（上一轮成功）
- **THEN** 图面 / 徽章 / 停止入口维持上一成功轮状态（nodes 不清零、无空图占位），页面呈现内联「刷新失败」提示（data-testid 可查）；MUST NOT 出现「未找到该 change」兜底页、MUST NOT 无提示呈现旧图如新

#### Scenario: 重试自愈吃回更新

- **WHEN** 失败轮后第 1 次退避重试（1s）成功，其间步迁移已落库
- **THEN** 内联提示消失、图面更新到最新查询结果（迁移间更新不被吃掉），`loading` 不闪烁整页占位

#### Scenario: 退避节奏与穷尽静默

- **WHEN** 连续失败轮注入（首败 + 3 次重试全败）
- **THEN** 自动补发的重试间隔依次为 1s / 2s / 4s 且至多 3 次；穷尽后无第 4 次自动查询（静默回落），内联提示保持，等待下一条通知或显式刷新兜底

#### Scenario: 新意图取代旧重试

- **WHEN** 重试计时器挂起期间新通知到达（或用户手动刷新、切换 root / id、卸载页面）
- **THEN** 挂起的旧重试作废、退避序列重置，以新意图触发的重查为准；全程不出现交错重复查询（invoke 计数可断言），卸载后零遗留计时器

#### Scenario: 终态重查失败自行收口

- **WHEN** `Finished` 通知（run 最后一条通知）触发的终态重查失败
- **THEN** 有界重试自行恢复终局（不依赖「下一条通知」——已无下一条）；恢复前 last-good 运行中态保持（有内联提示、有界、收敛）

#### Scenario: 首载失败兜底不变

- **WHEN** 首次加载 `get_change_detail` 即失败（detail 不在手）
- **THEN** 整页错误兜底页（既有语义）——保 last-good 仅适用于 detail 在手的刷新轮

#### Scenario: 真 None 清面兜底不变

- **WHEN** 重查成功但返回 `None`（真「未找到」——change 记录已不存在）
- **THEN** 清面呈现「未找到该 change」兜底页（既有语义）——`None` 与 `Err` 分流不混同

#### Scenario: 降级窗口动作可用

- **WHEN** 保 last-good 窗口内 run 处于 ask 停等（或 phase 间确认 / 运行中可停止）
- **THEN** 应答 / 确认 / 停止动作照常可发且生效（独立 invoke 不依赖前端降级态），控制入口状态与 last-good `activeRun` 一致、无状态与操作错配

#### Scenario: 订阅粘性不闪拆

- **WHEN** 失败轮保 last-good 期间（`activeRun` 维持在场值）
- **THEN** `activeRunPresent` 不翻 false，`change_flow_watch` 订阅不被 effect cleanup 拆除，下一通知照常抵达并触发重查（零重订、零丢通知）

### Requirement: 步级耗时与 token 节点呈现

runtime step 节点（`RunStepNode`）SHALL 在标题行下新增**常驻行**呈现步级耗时与 token 消耗（非 HoverCard，决策拍板）：耗时 = `endedAt - startedAt` 前端计算（后端不落 duration）；token 行仅 `group === 'workerAgent'` 节点渲染（`tokenInput` / `tokenOutput` 两值缩写呈现，缺席 "—"）。`RuntimeFlowNode` SHALL 透传 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput` 四字段（`runStepNodes` / `stepFromRecord` 直读透传，词汇归一单点口径不变）。

呈现语义 SHALL 为：

- **静态耗时**：已收口步（`endedAt !== null` 且 `startedAt > 0`）显格式化耗时；`phaseStart` 等瞬步 0ms 不显耗时缀；
- **running 走秒**：**仅活面（在飞 run）节点**且 `status === 'running'` 时以 `Date.now() - startedAt` 每秒刷新呈现（节点组件自持 interval 局部重渲染——前端全库首例，xyflow 节点独立渲染不惊动画布；节点离开 running 态或卸载即清 timer，收口后回落静态耗时）；库读史 `status=running` 残留节点（run 已死）MUST NOT 走秒，恒静态呈现；
- **缺席兜底**：`startedAt <= 0`（旧库行）或 `endedAt == null` 的非 running 态显 "-"（决策拍板：旧数据不从 turn 行回填）；token 缺席显 "—"。

毫秒 → 人读格式化 SHALL 为独立纯函数模块（新写，`1.2s` / `3m20s` 类分级；无既有先例可复用），vitest 直测边界；判定「缺席 → "-"」逻辑 SHALL 收格式化侧单点（MUST NOT 散落 `0` 魔数比较）。布局（10 列手工坐标 / subflow）、边表读取、节点点击判据与抽屉交互 MUST NOT 因常驻行改变；测试挂钩 SHALL 使用 data-testid。

#### Scenario: 常驻行静态耗时与 token

- **WHEN** 打开含已收口 executor 步（`startedAt=T0` / `endedAt=T1` / `tokenInput=12000` / `tokenOutput=3000`）的 change 详情
- **THEN** 该节点标题下常驻行呈现格式化耗时 `T1 - T0` 与 tokens in / out 缩写；无 HoverCard 依赖、点击判据不变

#### Scenario: token 行仅 WorkerAgent 组

- **WHEN** 图上同时呈现 executor 节点与 static-check 工具步节点（后者四字段中 token 恒 null）
- **THEN** 工具步 / Gate / Git 节点不渲染 token 行（仅耗时）；workerAgent 组节点渲染 tokens in / out 两值

#### Scenario: running 走秒与收口回落

- **WHEN** run 运行中某 WorkerAgent 节点 running（活面，`startedAt` 在案、`endedAt=null`）
- **THEN** 该节点耗时以每秒 tick 的走秒值呈现；该步收口（状态离开 running）后 timer 卸载、回落静态耗时；同一时刻其余节点零重渲染

#### Scenario: 库读史残留与旧数据兜底

- **WHEN** 打开详情呈现本变更前落库的 run 节点（`startedAt=0` / `endedAt=null`）或库读史 status=running 残留节点
- **THEN** 耗时显 "-"（不走秒、不报错）；token 显 "—"；其余节点呈现不受影响

#### Scenario: 格式化纯函数直测

- **WHEN** 以边界值（0ms / 999ms / 1s 级 / 分秒级 / 时分级）调用格式化函数
- **THEN** 输出符合分级规则（0ms 由调用侧省显、缺席入参出 "-"），无需挂载 react 组件即可断言

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/graph.ts` + `types.ts` | 转换层扩展 | 节点模型增运行态与节点类型可辨维度；`PIPELINE_PHASES` 列布局、attempt 缺号兜底不变；边转换改 `runs[].edges` 直读（见下两行 `graph.ts` / `graph.test.ts` 契约）；纯函数直测保持绿 |
| ReactFlow 渲染薄层组件（新） | 图呈现 | `@xyflow/react` v12 subflow（`parentId` + `extent: 'parent'`）；9 列恒定 + 默认 100% 视口 + 滚轮平移（禁滚轮缩放）；eval 实心 / active pulse / stale 半透明 |
| `packages/desktop/src/views/changes/flow/` 节点 / 列组件 | 运行态渲染 | WorkerAgent / ToolStep / Gate 类型可辨；pulse 运行态与既有 active 视觉同款；收口后回落派生规则 |
| 右侧抽屉组件（新） | 素材详情 | 二分节【本站文档 / eval report+checklist】（文件表节随 file_log 退役删除）；文档节复用 `renderers/registry`（`ArtifactView`）；单一交互入口 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | id 寻址与单态呈现 | `useParams<'id'>`；`flow-empty` 文档形态分支删除；页内身份源统一（URL id），无 `detail.name` 作寻址残留；归档 change 照常呈图；发起 / 停止 / phase 间确认 / ask 应答卡片；生命周期状态与可用操作对齐；代际徽章、损坏警示条、`WorkflowPanel`（file_log 图外面板）删除 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | 清单词汇清零与 db 单源 | `InventoryBadge` 与 unparsable 标注删除；清单 db 单源（文档形态条目不再入列，见 desktop-change-queries） |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 右列三分节 → 二分节 | 文件表节删除；左列会话区、滚动与单一交互入口不变 |
| `packages/desktop/src/views/changes/flow/file-log-table.tsx` | **（删除）** | file_log 文件表组件及测试随载体退役删除 |
| `packages/desktop/src/views/changes/flow/attachments.ts` | 挂载分支收缩 | file_log 挂节点与 scope='workflow' 分支删除；文档挂列 + checklist 挂节点保持 |
| 会话转录联动 | 节点 ↔ 转录 | 复用 `AgentSessionView`（desktop-agent-chat-infra，只读消费点：不接 composer、不呈发送 / 停止），无第二套对话渲染组件与第二套转录查询 hook；运行中实时流（会话订阅面载荷直投）、收口后转录库重放一致 |
| 运行状态 Channel 订阅 | 实时刷新例外 | 执行流通道例外（沿 agent 执行先例）；增量并入不触发 `get_change_detail` 全量重取；收口释放订阅 |
| `src/commands/` change-flow 命令组（desktop-change-orchestration） | IPC 面 | 发起 / 停止 / 应答 / 确认契约见 desktop-change-orchestration「运行控制命令面」 |
| `packages/desktop/package.json` | 新依赖与版本交付 | `@xyflow/react` v12（React 19 + Tailwind 4 兼容）；无第二图布局库（无 dagre / elk）；`version` 0.3.2 → 0.3.3（`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 详情线面收敛 | `ChangeDetail.interrupted` 与线面 `InterruptedEntry` 删；其余字段线面口径（=null + ISO 串、纯 derive 零字段属性）不变 |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | 发起开关与轻提示 | 发起区「自动确认」开关默认关 + 一行说明文案；运行期定格；data-testid 挂钩；确认卡片 / ask 卡片呈现逻辑不变 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | start 传参与 id 透传 | `start(autoNextPhase: boolean)` → `commands.changeFlowStart(channel, root, id, autoNextPhase)`；发起 / 停止 / 确认 / 应答命令携 change id；订阅生命周期不变 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 会话槽位直查分派 | 节点 `sessionId` 直查（`RoleSessionRef` 仅 `{ role, sessionId }`，无 sourceRef 字段）；槽位缺席呈「此步无会话记录」空态，前端零 sourceRef 反查 |
| `packages/desktop/src/views/changes/flow/`（graph.ts / attachments.ts / run-state.ts） | 零触点 | 转换层纯函数、挂载规则、reducer 不涉 change 身份；`PIPELINE_PHASES` 布局与边推导不变 |
| `packages/desktop/src/views/changes/flow/run-state.ts`（不改） | 零触点 | 无新事件词汇，纯 reducer 与 `ChangeFlowRunState` 模型不动 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型再生跟随 | `ChangeDetail.interrupted` / `InterruptedEntry` / `inventory` / `fileLog` / `unparsable` 再生后消失；tsc 全量类型检查拦截前端消费漂移 |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | **（退役：删）** | `useSessionTranscript` 收编进 `AgentSessionView` / `useAgentSession`；会话直查与重放装配由统一组件承担，`liveEvents` 死参数与 `sourceRef` 退役叙述随之删除，MUST NOT 保留第二套转录查询 hook |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 单列布局与槽位缺席空态 | 单列宽度约 960px + `max-w-[85vw]`；`selectionRoleRef` 收敛为 `{ role, sessionId }`（active 三 role 反查随 active 节点退役一并删除）；`role !== null` 而 `sessionId === null` 呈「此步无会话记录」空态占位（零反查） |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 抽屉单列会话面 | `SessionTranscriptPanel` 入参 `root` / `roleRef` / `refreshKey?`（会话状态重评估信号）：`roleRef.sessionId` 直查 + `AgentSessionView` 只读渲染（无 composer）；槽位缺席呈空态占位且零订阅；`SessionMeta`（`summary` / `running`）自组件状态读取，不再自带查询 |
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts`（通知键） | 转录专用去抖退役 | `notifyTranscript` / `transcriptTick`（150ms 去抖重查键）删除；会话事件通知改走统一视图重查 tick（组件据 `refreshKey` 重评估会话状态）；`notifyRefresh`（300ms 尾随去抖 + 有界重试）语义不动 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` + `change-flow-graph.tsx` | 查看会话入口下线 | `onOpenSession` prop 与按钮删除；节点点击即抽屉联动，无独立会话 route |
| `packages/desktop/src/views/changes/flow/types.ts` + `graph.ts` + `flow-event-node.tsx` + `attachments.ts` | interrupted 前端词汇清除 | `FlowNodeKind` 收敛 `'eval' | 'active'`；`collectInterrupted` / `InterruptedBody` / dashed 分支删；`NODE_PRECEDENCE` 收敛 `['eval', 'active']` |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` + `parse/workflow_file.rs` | 磁盘读模型停解析（parse 模块已整体退役删除） | `InterruptedEntry` 等磁盘读模型随 parse 退役删除（见 desktop-change-orchestration 与 desktop-change-state-store）；存量 workflow.json 原样留档零触碰 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/golden/*.json` + `tests/corpus_golden_test.rs` | golden 显式重写 | detail 线面 golden 按冻结契约的显式重写流程更新（v2-b 含实数据）；`interruptedCount` 投影随动删除 |
| 磁盘形状与写入方（不改） | 零触点 | `workflow.json` 的 `interrupted[]` 原样保留；TS `workflow.schema.ts` 与插件 CLI 写入方不动 |
| 图结构与取数模型（不改） | 零触点 | `PIPELINE_PHASES` 布局、边推导、显式刷新 + Channel 例外口径不变 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | 词归一单点 | `normalizeKind` / `stepWord` 补 `cleanup` ↔ `cleanup`；`cleanup` 归 git 分类 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 节点呈现 | `STEP_LABEL` 补 cleanup 词条；detail 面呈现清理警告 / 跳过注记（节点恒 passed） |
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 失败轮降级与重试单点 | `loadUnifiedView` 失败分流（reject 且 detail 在手 → 保 last-good + 内联失败态 + 退避重试调度；首载失败维持整页 error 兜底）；重试计时器与 tick / 通知去抖同生命周期（新意图取代旧重试、卸载清计时器）；退避常量（3 次 / 1s / 2s / 4s）单点；`None` 清面语义不变 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 内联提示与兜底分流 | 内联「刷新失败」提示（data-testid 挂钩）随 last-good 态呈现 / 成功轮消失；`error` 分支收敛为首载语义（detail 在手的刷新失败不再整页 `DetailFallback`）；首载错误 / 真 `None` 兜底页分支不变 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts`（零实现改动） | 订阅粘性验证 | `activeRunPresent` 派生源（统一查询 activeRun）不清 → 订阅不闪拆；仅补测试断言，订阅生命周期实现不动 |
| `packages/desktop/src/views/changes/flow/types.ts` + `run-state.ts` | 透传 | `RuntimeFlowNode` 增 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput`；`stepFromRecord` / `runStepNodes` 直读透传（`UnifiedRunStep` 单一形状） |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 常驻行与走秒 | 标题下常驻行（静态耗时 + workerAgent token 行）；running 走秒 interval 仅活面 running 节点挂载、卸载即清；缺席 / 旧数据 "—" 兜底 |
| `packages/desktop/src/views/changes/flow/`（格式化模块，新） | 毫秒 → 人读 | 纯函数分级格式化 + 缺席判定单点；vitest 直测边界 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `ChangeStepState` / `ChangeRunNodeRecord` 四字段再生成，tsc 全量类型检查拦截漂移 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 步词文案 | `STEP_LABEL` 穷尽 Record 补 `clean` 键（人读文案）；`GROUP_LABEL` git 分类覆盖 clean；零新节点类型 / 零新面板 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | 步词归一 | `normalizeKind` / `stepWord` 单点补 `clean` ↔ `clean` 映射（二十一值逐词对应） |
| `packages/desktop/src/types/generated/bindings.ts`（重生成） | 类型跟随 | `ChangeStepKind` / `RunStepKind` 新变体出线；`bindings:check` 守卫拦截过期生成物 |
| 转换层 / 抽屉 / stdout 面板（零改动） | 结构复用 | initialize 列与步节点经既有 `detail.pipeline` 出线与 runtime step 渲染直达；baseline 输出经既有 `outputRef` stdout 面板回查 |
| `src/views/changes/flow/graph.ts` | 边转换 | `runs[].edges` 直读、端点缺失 `continue`；零 `deriveEdges` Δ 派生；`retry` 无专属样式分支 |
| `src/views/changes/flow/graph.test.ts` | 转换层用例 | `retry` fixture 改写为「输入含 retry 边 → 输出图不含该边」的投影锚 |
| `packages/desktop/src/components/standard/drawer.tsx` | 模态开关透传 | 增 `modal?: boolean`（缺省 `true`，透传至底层 `Drawer`）；既有调用方零回归 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 非模态打开 | 传 `modal={false}`（`data-modal="false"`、无遮罩节点）；选中驱动、内容分派、单列宽度 `w-[960px] max-w-[85vw]` 与既有断言保持 |
| `packages/desktop/src/components/ui/drawer.tsx` | 零改动（能力已在位） | `modal === true` 才渲染 `DrawerOverlay`；非模态时 Viewport `pointer-events-none`、Popup 自身 `pointer-events-auto`（左侧 chat 可交互的机制来源） |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 分栏宿主席位 | 左栏通用 agent 面板（见 desktop-change-chat）+ 右栏详情；`DetailContent` 的抽屉装配与 `selection` 选中态语义不变 |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 零改动 | 只读会话形态与 `refreshKey`（会话状态重评估）语义不变；运行中仍经会话订阅面流式 |
