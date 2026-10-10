# desktop-change-flow-view Specification

## Purpose

定义 desktop change 详情页的 react-flow 流程图视图：attempt 级两分类节点（eval / active）、9 列手工坐标布局、时间序边推导、过程素材两层挂载、右侧抽屉单一交互入口与建档两态降级；转换层为纯函数、渲染薄层复用 react-flow v12。

## Requirements

### Requirement: 流程图节点模型与两分类

change 详情页 SHALL 以 react-flow 流程图承载执行史，节点粒度 SHALL 为 attempt 级。转换层 SHALL 将 `ChangeDetail` 的事件归并为两类节点：

- **eval 节点**：来自 `pipeline` 各站的 `attempts[]`（verdict / checklist / report / skipped / stale），实心呈现——pass 绿、fail 红；`stale: true` 的节点 SHALL 半透明淡化（废弃支线）；
- **active 节点**：来自 `activePhase`（运行中，无 verdict），pulse 运行中视觉，位于所在列的接流末端。

`interrupted[]` 留档 SHALL 不再上图：读模型（Rust `Workflow` / `ChangeDetail`）MUST NOT 解析该字段，前端节点模型 MUST NOT 保留 interrupted 词汇（`FlowNodeKind` 收敛 `'eval' | 'active'`）；磁盘上既有 `workflow.json` 的 `interrupted[]` 数据 SHALL 原样保留（serde 未知字段忽略），MUST NOT 迁移或清除，写入方（插件 CLI）MUST NOT 因本变更改动。9 站顺序 SHALL 以 `PIPELINE_PHASES` 固定序列为准（proposal → dev-design → test-design → implement → test-gen → test-execution → code-review → acceptance → code-analyze），不依赖 eval 排列。

#### Scenario: 两分类节点呈现

- **WHEN** 打开一个含多 attempt 与 backtrack 的 v2 change 详情
- **THEN** 图上呈现 eval 节点（pass 绿 / fail 红）与 pulse 的 active 节点，无 dashed 灰显的 interrupted 节点

#### Scenario: interrupted 留档不出图且数据保留

- **WHEN** 打开 workflow.json 含 `interrupted[]` 留档的存量 v2 change（v2-b 夹具形态：`interrupted` 的 test-gen#1 与 `active_phase` 的 implement#2 均不在 eval 序列内）
- **THEN** test-gen#1 不以任何节点形态出现在图上（"未收口"语义由 implement#2 的 active 节点承担），磁盘 workflow.json 的 `interrupted[]` 数据原样保留、详情查询不报错

#### Scenario: stale 淡化

- **WHEN** 某站的最新 pass eval 被 backtrack 标记 `stale: true`
- **THEN** 该节点呈半透明淡化，表示废弃支线，但仍保留在图上

### Requirement: 9 列手工坐标布局

布局 SHALL 采用手工坐标，MUST NOT 引入 dagre / elk 等自动布局引擎：`x = PIPELINE_PHASES.indexOf(phase) × COL_W`（9 列恒定，未走过的站呈现空列），`y = 列内执行序 × ROW_H`。同 phase 的全部事件 SHALL 纵向对齐成一列。phase 列 SHALL 以 react-flow v12 subflow 承载（`parentId` + `extent: 'parent'`），列头展示 phase 名与该站过程文档徽章。COL_W / ROW_H 常量取值由 design 定夺。视口 SHALL 默认 100% 缩放（MUST NOT 初始 fitView 适配），滚轮 SHALL 平移画布（滚轮缩放 MUST NOT 触发），显式刷新重绘时 SHALL 保留当前视口。

#### Scenario: 列对齐与空列恒定

- **WHEN** 某 change 只走到 dev-design 站（proposal 有 eval 记录）
- **THEN** proposal 与 dev-design 事件各自纵向对齐成列，后续 7 站仍以空列呈现，列总数恒为 9

#### Scenario: backtrack 重做自动向左

- **WHEN** test-execution#1 fail 后 backtrack 回 test-gen 并重做 test-gen#2
- **THEN** 重做的 test-gen#2 节点位于 test-gen 列（Δx < 0），无需显式边分类即呈现为向左回跳

### Requirement: 时间序边推导

边 SHALL 由 O(n) 扫描推导：eval 事件按追加序（即时间序）串联，active 事件按 startAt 归并插入；每个事件的入边 SHALL 指向时间上紧邻的前一事件。边的 Δx 符号 SHALL 表达类型：向右=前进、同列（Δx=0）=重试、向左=回跳；回跳边 SHALL 为虚线并以 `backtrackReason` 作边标签。attempt 缺号 SHALL 按 0 兜底（与后端 `detail.rs` 的 `unwrap_or(0)` 同款），startAt 缺失的事件 SHALL 按列表序归并、不丢节点。

#### Scenario: 前进 / 重试 / 回跳三类边

- **WHEN** 执行史为 proposal#1 → dev-design#1 fail → dev-design#2 pass（回跳重做）
- **THEN** proposal#1→dev-design#1 为向右前进边，dev-design#1→dev-design#2 为同列重试边；若失败边源自跨站 backtrack（如 test-execution#1 → test-gen#2），该边呈虚线且 `backtrackReason` 可读

#### Scenario: 缺号与缺时间戳兜底

- **WHEN** 某事件 `attempt` 为 null 或 startAt 为 null
- **THEN** 节点不丢失：attempt 以 0 兜底参与列内排序，startAt 缺失者按列表序归并，转换不报错

### Requirement: 过程素材挂载规则

素材挂载分两层：**文档挂列、记录挂节点**。

- 过程文档（`artifacts` 中 `markdown-doc` / `tasks-progress` 类）SHALL 经前端静态映射表归属到 phase 列（列头徽章，每站一份、不随 attempt 重复）：`proposal.md` 与 `specs/**` → proposal、`design.md` 与 `tasks.md` → dev-design、`test-reports/**` → test-execution；映射表外的文档 SHALL 落入「未归属」处理（design 定夺展示位置），MUST NOT 误挂到错误站。`explore.md` MUST NOT 入图（探索在流程之外，不属于任何站）。
- eval-checklist 信封（payload 自带 `phase` + `attempt`）SHALL 挂到对应 attempt 节点。

file_log 挂载 SHALL 退役：file_log 条目挂节点规则与 `scope='workflow'` 图外独立面板随载体退役一并删除（前端 file-log 面板组件与挂载分支移除）。

归属 SHALL 全部在前端派生，后端 `ChangeDetail` DTO 与 `PIPELINE_PHASES` MUST NOT 因本能力新增 file_log 相关字段。

#### Scenario: 文档按映射表归列

- **WHEN** 某 change 的产物含 `proposal.md`、`design.md`、`tasks.md` 与 `test-reports/xxx.md`
- **THEN** proposal 列头徽章含 proposal.md，dev-design 列头徽章含 design.md 与 tasks.md，test-execution 列头徽章含该 test-reports 文档

#### Scenario: explore.md 隐身与文件面板退役

- **WHEN** 打开任一 change 详情，产物清单含 `explore.md`
- **THEN** explore.md 不出现在图中，图上无文件徽章挂靠（节点文件区概念已退役），无 workflow 独立面板渲染

### Requirement: 右侧抽屉单一交互入口

phase 列头与 attempt 节点 SHALL 提供同一交互入口：点击打开右侧抽屉。抽屉 SHALL 为双列布局：左列（约 60% 宽）为会话信息区——上方呈现选中节点关联会话的元信息（session id、运行状态徽章等，具体字段清单由 design 定夺，数据取自会话查询的 `SessionSummary`），下方转录 tab 与时间线拉满左列高度滚动；右列为二分节【本站文档 | eval report+checklist】（原第三分节「文件表」随 file_log 退役删除），各节在右列内滚动。抽屉整体宽度 SHALL 保持约 960px 并保留 `max-w-[85vw]`；遮罩点击关闭与右贴边定位不变。无会话联动的选中（列头 / ToolStep / Gate / 无 role 运行步）左列 SHALL 呈空态占位，双列结构 MUST NOT 因选中类型跳变。本站文档节 SHALL 复用 `renderers/registry`（经 `ArtifactView` 渲染，新增产物 kind 仍只需在 registry 追加一行）；eval 节展示该 attempt 的 report 与 checklist 条目。列头点击与节点点击 MUST NOT 出现两套并存的展开交互。

#### Scenario: 节点点击抽屉双列内容

- **WHEN** 点击 implement#2（fail → retry pass 的第二个 attempt，会话记录在场）节点
- **THEN** 左列呈该 attempt 的会话元信息与 executor / evaluator / decision 转录 tab，右列自上而下为该站文档、attempt#2 的 report 与 checklist，无文件表节

#### Scenario: 列头点击同入口左列空态

- **WHEN** 点击 dev-design 列头
- **THEN** 打开与节点点击相同的右侧抽屉，右列本站文档节含 design.md / tasks.md，左列呈空态占位而非整列消失

#### Scenario: 双列各自滚动

- **WHEN** 某 attempt 的会话转录、report 与 checklist 内容均超出可视高度
- **THEN** 左列时间线在左列内滚动、右列二分节在右列内滚动，抽屉本体 MUST NOT 退回整列单滚

### Requirement: 三代际降级

视图 SHALL 按建档状态两态降级（inventory 代际词汇退役）：

- **已建档**（db `ChangeRecord` 在案）：SHALL 呈现完整状态面——流程图（eval / active 节点照常）、抽屉与产物区照常；
- **文档形态**（db 缺记录，存量 CLI change）：SHALL 呈现空图占位，产物区照常呈现（图区 MUST NOT 挤掉产物区），无状态面区块、无任何「无法解析」警示。

inventory 代际徽章（V0 / V1 / V2）与 `unparsable`（workflow.json 损坏）警示 SHALL 退役：清单与详情的前端词汇清零（组件、DTO 消费与测试断言一并移除）；error / loading / 未找到降级页 SHALL 沿用现状。

#### Scenario: 建档 change 完整状态面

- **WHEN** 打开一个 db 已建档、含多 attempt 与 backtrack 的 change 详情
- **THEN** 流程图正常绘制 eval / active 节点，无代际徽章出现，抽屉与产物区照常

#### Scenario: 文档形态空图占位

- **WHEN** 打开一个无 db 记录的存量 CLI change 详情
- **THEN** 图区呈现空图占位，产物区照常展示，无「无法解析」警示，页面不报错

#### Scenario: 代际与警示词汇清零

- **WHEN** 扫描清单 / 详情视图源码与测试对 `inventory` / `unparsable` / 「无法解析」的消费
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

`detail → { nodes, edges }` 转换 SHALL 为独立纯函数模块（`views/changes/` 下，目录与命名由 design 定夺），不依赖 react 宿主组件状态、不发起任何 invoke，vitest SHALL 直测其边界：前进 / 重试 / 回跳 / stale 淡化 / active 接流末端 / attempt 缺号 / 时间戳缺失 / 文档归属映射 / workflow scope 剔除。react-flow SHALL 仅作渲染薄层，其库内布局与交互语义 MUST NOT 做逐项断言（符合「不逐项验证库语义」约定）。

#### Scenario: 转换层直测

- **WHEN** 以 v2-a / v2-b 夹具形状的 `ChangeDetail` 数据（含 fail→retry、backtrack、stale）调用转换函数
- **THEN** 输出的节点坐标符合列索引 / 执行序公式，边集合与方向符合时间序推导，无需挂载 react 组件即可断言

### Requirement: desktop 版本升级交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.3.2` 升级为 `0.3.3`。`src-tauri/tauri.conf.json` SHALL 维持 `../package.json` 引用（版本自动跟随，MUST NOT 引入硬编码版本号）；`src-tauri/Cargo.toml` 的 `[package].version`（`0.1.0`）MUST NOT 随 desktop 版本变动（既有惯例：0.3.0 / 0.3.1 两次升级均只改 package.json）。

#### Scenario: 版本号升级与引用跟随

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` 的 `version` 为 `0.3.3`，`tauri.conf.json` 无版本号硬编码改动，`src-tauri/Cargo.toml` 版本保持 `0.1.0`

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的节点 SHALL 呈现运行态（pulse 视觉，与既有 active 节点同款），ToolStep 节点（phase-start / static-check / phase-log / backtrack 相位机步）与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（类型可区分）。运行态呈现 SHALL 与既有节点模型并存：eval（实心 pass 绿 / fail 红）、stale 淡化规则不变；`PIPELINE_PHASES` 列布局、时间序边推导规则与空列恒定 MUST NOT 因运行态呈现改变。

运行收口后，图 SHALL 保持**步节点常驻渲染**（「图回落 ChangeDetail 派生规则」语义退场）：已落库步节点（executor / evaluator / decision / static-check / test-execution，desktop-change-state-store 五词汇）自统一视图的 runs[].steps 派生常驻上图，与运行中实时面走**同一条派生路径**（同一转换函数消费合并查询结果，而非结果碰巧一样）；运行态视觉（pulse）褪去、终态样式呈现。常驻渲染 SHALL 取**全史叠加**（多次 run 的步节点全部保留，非仅最近 run）：attempt 经跨 run max+1 分配不撞号，全史叠加在同一列面、run 归属分层经 run 史可查；在飞 run 的活步叠加其上，步节点键 `phase:attempt:step` 天然不冲突。

#### Scenario: 运行态与两分类并存

- **WHEN** implement#2 相位运行中（executor 会话活跃）且历史上有 fail→retry 与 active_phase 残留
- **THEN** 图上同时呈现：历史 eval 节点（实心三色）、active_phase 的 active 节点（pulse）与当前 executor 节点（pulse 运行态）及同相位的 ToolStep / Gate 节点（类型可辨、状态如实）

#### Scenario: 收口后常驻不回落

- **WHEN** run 以全部相位 pass 收口（或 fail / stopped 收口）后刷新 / 重开详情
- **THEN** 已落库步节点常驻上图（终态样式、无 pulse），派生自统一视图库读史、与运行中同一转换函数；无「回落后步节点消失」的形态跳变

#### Scenario: 全史叠加与在飞叠加

- **WHEN** 某 change 历经两次 run 后第三次 run 运行中
- **THEN** 图上前两 run 的步节点全史常驻，第三 run 活步（pulse）叠加其上；同 phase 节点 attempt 递增不撞键，任一步节点可溯其 run 归属

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线（delta 流、可停止）——实时面数据源 SHALL 为**转录库重查**（变更通知触发、`use-session-transcript` 既有查询面消费；会话事件流式落库），前端 liveEvents 内存缓存随通知降位退役，实时与重放口径统一到转录库；点击已收口的 WorkerAgent 节点 SHALL 打开该会话的转录重放（不要求运行进程存活）。会话寻址 SHALL 记录 id 直查优先：attempt 记录暴露的会话槽位 id（desktop-change-queries，自 PhaseRecord 三槽位列直读）在场时，转录面板 SHALL 按 session id 直查（`session_detail` + `agent_session_transcript`）；槽位缺席（缺省落账，槽位列 None）时 SHALL 回退既有 sourceRef 定式反查（`<change>/<phase>/<role>/<attempt>`，同 ref 多会话取最近一条）。eval 节点与 active 节点的抽屉联动 SHALL 均覆盖三会话：executor / evaluator / decision 三转录 tab（形态由 design 定稿）；decision 槽位缺席时该 tab SHALL 呈空态，MUST NOT 虚构会话或误挂他 attempt 的会话。active 节点 SHALL 经 sourceRef 反查联动（`active_phase` 无会话槽位，sessionId 恒 null；`ActivePhase.attempt` 非空保证定式可组装）：会话建档即落库、sealed 事件流式 append，进行中会话的已流出转录经反查重放可见；未开跑角色的 tab SHALL 呈「（暂无该会话转录）」空态；active 节点联动 MUST NOT 依赖实时事件流（实时面统一为转录库重查，实时流由 runtime WorkerAgent overlay 节点承担，本 app run 场景图上并存）。转录呈现 SHALL 复用既有会话基建（`AgentTimeline` / 会话重放，desktop-agent-chat-infra），MUST NOT 为 change 场景另建第二套时间线组件。节点 MUST NOT 提供显式「查看会话」按钮（两列布局后点击节点即见左列转录，active 节点不新增）。ToolStep / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果（左列空态占位、右列展示步骤输出摘要；审计全量见 desktop-change-state-store 步骤审计查询，步节点史见 run_steps 读史）。

#### Scenario: 运行中节点实时转录经转录库

- **WHEN** executor 会话运行中点击其 WorkerAgent 节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），数据经转录库重查呈现（通知触发重查，非内存 liveEvents 累积），与 Agent 调试页同款组件

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

#### Scenario: 记录 id 直查优先

- **WHEN** 某 attempt 记录携带会话槽位 id，且同 sourceRef 定式下存在多条会话
- **THEN** 转录面板按槽位 id 精确直查该会话，MUST NOT 落入「同 ref 多会话取最近一条」的反查歧义

#### Scenario: 槽位缺席回退反查

- **WHEN** 打开槽位列 None 的 attempt 记录（缺省落账）
- **THEN** 转录面板回退 sourceRef 定式反查并照常重放，行为与直查路径一致可靠

#### Scenario: decision 第三 tab 与空态

- **WHEN** 点击某 eval 节点打开抽屉，该 attempt 的 decision 槽位在场（或缺席）两种形态分别驱动
- **THEN** 抽屉呈 executor / evaluator / decision 三转录 tab；decision 槽位在场时按 id 直查其转录，缺席时 decision tab 呈空态，executor / evaluator 两 tab 不受影响

#### Scenario: active 节点三会话反查联动

- **WHEN** 打开一个残留 active_phase 的 change（如 CLI 外部跑的 run、刷新重挂或 crash 残留），点击 active 节点
- **THEN** 抽屉左列呈 executor / evaluator / decision 三转录 tab，已开跑角色经 sourceRef 反查重放已落库转录（含进行中会话已流出部分），未开跑角色呈「（暂无该会话转录）」空态；全程不订阅实时事件流，亦不虚构未开跑角色的会话

#### Scenario: 查看会话按钮不呈现

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点上无「查看会话」按钮，点击节点本体即打开抽屉查看左列转录；无独立会话 route 引入

### Requirement: 运行控制入口与 ask 应答

change 详情页 SHALL 提供运行控制入口：发起 run（按 change 名）、停止运行中的 run、phase 间停等点的用户确认（继续 / 终止）。决策协议 `ask` 中断 SHALL 在页面呈现中断卡片（问题 + 选项），用户应答（选项或自由文本）后经应答命令回流驱动 run 继续；停止与发起 SHALL 沿运行控制命令面契约（提前 resolve、停止幂等）。控制入口状态 SHALL 如实反映 run 生命周期（可发起 / 运行中可停止 / ask 等待应答 / phase 间等待确认），MUST NOT 出现状态与可用操作错配（如已收口仍显停止入口为可用主操作）。

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

change 详情页运行控制面板 SHALL 在发起操作区提供「自动确认」开关（默认关）：开启后发起 run 以 `autoNextPhase=true` 调运行控制命令面（`useChangeFlowRun.start(autoNextPhase)` 传参），关闭则 `false`。开关 SHALL 为发起参数（发起时定格）：运行期间 MUST NOT 可切换（禁用或隐藏的呈现形态由 design 定夺）；节奏变更 = 停止后重发（自 `active_phase` 续走）。auto 模式 run MUST NOT 呈现 phase 间确认卡片（无 `ConfirmWait` 事件到达，`run-state.ts` 零改动）；ask 中断卡片 SHALL 照常呈现与应答（ask 不自动应答）。开关旁 SHALL 提供一行说明文案轻提示（auto 模式将跳过 phase 间确认连续推进，文案措辞由 design 定夺），MUST NOT 引入阻断式确认弹窗。开关 SHALL 以 data-testid 提供测试挂钩，测试 MUST NOT 以样式类名查询。

#### Scenario: 开关默认关与发起传参

- **WHEN** 打开运行控制面板（无运行 run / 终局后），开关保持默认关并点击发起
- **THEN** `changeFlowStart` 携 `autoNextPhase=false`，run 照常在相位落账后停等确认（确认卡片呈现）；开启开关后再发起则携 `autoNextPhase=true`

#### Scenario: auto run 不出确认卡片

- **WHEN** auto 模式 run 相位落账 pass 推进下一相位
- **THEN** 页面无确认卡片、状态徽章不经 `waitingConfirm`，图与步流照常增量刷新；run 直至终态收口

#### Scenario: auto run 的 ask 卡片照常

- **WHEN** auto 模式 run 中决策 ask 到来
- **THEN** ask 中断卡片照常呈现（问题 + 选项 + 自由文本），用户应答后 run 继续；确认卡片不因 auto 模式对 ask 生效

#### Scenario: 运行期定格与收口回归

- **WHEN** auto 模式 run 运行中尝试切换开关
- **THEN** 开关不可切换（定格为发起值）；run 收口后控制入口回到发起区，可按任意节奏再次发起

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/graph.ts` + `types.ts` | 转换层扩展 | 节点模型增运行态与节点类型可辨维度；时间序边推导规则、`PIPELINE_PHASES` 列布局、attempt 缺号兜底不变；纯函数直测保持绿 |
| ReactFlow 渲染薄层组件（新） | 图呈现 | `@xyflow/react` v12 subflow（`parentId` + `extent: 'parent'`）；9 列恒定 + 默认 100% 视口 + 滚轮平移（禁滚轮缩放）；eval 实心 / active pulse / stale 半透明 |
| `packages/desktop/src/views/changes/flow/` 节点 / 列组件 | 运行态渲染 | WorkerAgent / ToolStep / Gate 类型可辨；pulse 运行态与既有 active 视觉同款；收口后回落派生规则 |
| 右侧抽屉组件（新） | 素材详情 | 二分节【本站文档 / eval report+checklist】（文件表节随 file_log 退役删除）；文档节复用 `renderers/registry`（`ArtifactView`）；单一交互入口 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 控制入口组装与详情退役面 | 发起 / 停止 / phase 间确认 / ask 应答卡片；生命周期状态与可用操作对齐；代际徽章、损坏警示条、`WorkflowPanel`（file_log 图外面板）删除；建档两态分流 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | 清单降级两态与词汇清零 | `InventoryBadge` 与 unparsable 标注删除；文档形态条目照常入列 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 右列三分节 → 二分节 | 文件表节删除；左列会话区、滚动与单一交互入口不变 |
| `packages/desktop/src/views/changes/flow/file-log-table.tsx` | **（删除）** | file_log 文件表组件及测试随载体退役删除 |
| `packages/desktop/src/views/changes/flow/attachments.ts` | 挂载分支收缩 | file_log 挂节点与 scope='workflow' 分支删除；文档挂列 + checklist 挂节点保持 |
| 会话转录联动 | 节点 ↔ 转录 | 复用 `AgentMessages` / 会话重放基建（desktop-agent-chat-infra），无第二套对话渲染组件；运行中实时流、收口重放一致 |
| 运行状态 Channel 订阅 | 实时刷新例外 | 执行流通道例外（沿 agent 执行先例）；增量并入不触发 `get_change_detail` 全量重取；收口释放订阅 |
| `src/commands/` change-flow 命令组（desktop-change-orchestration） | IPC 面 | 发起 / 停止 / 应答 / 确认契约见 desktop-change-orchestration「运行控制命令面」 |
| `packages/desktop/package.json` | 新依赖与版本交付 | `@xyflow/react` v12（React 19 + Tailwind 4 兼容）；无第二图布局库（无 dagre / elk）；`version` 0.3.2 → 0.3.3（`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 详情线面收敛 | `ChangeDetail.interrupted` 与线面 `InterruptedEntry` 删；其余字段线面口径（=null + ISO 串、纯 derive 零字段属性）不变 |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | 发起开关与轻提示 | 发起区「自动确认」开关默认关 + 一行说明文案；运行期定格；data-testid 挂钩；确认卡片 / ask 卡片呈现逻辑不变 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | start 传参 | `start(autoNextPhase: boolean)` → `commands.changeFlowStart(channel, root, change, autoNextPhase)`；订阅生命周期不变 |
| `packages/desktop/src/views/changes/flow/run-state.ts`（不改） | 零触点 | 无新事件词汇，纯 reducer 与 `ChangeFlowRunState` 模型不动 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型再生跟随 | `ChangeDetail.interrupted` / `InterruptedEntry` / `inventory` / `fileLog` / `unparsable` 再生后消失；tsc 全量类型检查拦截前端消费漂移 |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | summary 面暴露 | transcript / running 之外返回 `SessionSummary`（row + stats + turns）；id 直查优先 + sourceRef 反查兜底、seq 归并逻辑不变 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 双列布局改造 | 左列会话区约 60%（元信息 + 转录拉满滚动）+ 右列三分节列内滚动；约 960px + `max-w-[85vw]`；`selectionRoleRefs` 补 active 三 role 反查（sessionId 恒 null）；无会话选中左列空态占位 |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 左列转录 tab 与元信息 | executor / evaluator / decision 三 tab 不变；新增会话元信息呈现（清单 design 定夺）；复用 `AgentMessages` 无第二套对话渲染 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` + `change-flow-graph.tsx` | 查看会话入口下线 | `onOpenSession` prop 与按钮删除；节点点击即抽屉联动，无独立会话 route |
| `packages/desktop/src/views/changes/flow/types.ts` + `graph.ts` + `flow-event-node.tsx` + `attachments.ts` | interrupted 前端词汇清除 | `FlowNodeKind` 收敛 `'eval' | 'active'`；`collectInterrupted` / `InterruptedBody` / dashed 分支删；`NODE_PRECEDENCE` 收敛 `['eval', 'active']` |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` + `parse/workflow_file.rs` | 磁盘读模型停解析（parse 模块已整体退役删除） | `InterruptedEntry` 等磁盘读模型随 parse 退役删除（见 desktop-change-orchestration 与 desktop-change-state-store）；存量 workflow.json 原样留档零触碰 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/golden/*.json` + `tests/corpus_golden_test.rs` | golden 显式重写 | detail 线面 golden 按冻结契约的显式重写流程更新（v2-b 含实数据）；`interruptedCount` 投影随动删除 |
| 磁盘形状与写入方（不改） | 零触点 | `workflow.json` 的 `interrupted[]` 原样保留；TS `workflow.schema.ts` 与插件 CLI 写入方不动 |
| 图结构与取数模型（不改） | 零触点 | `PIPELINE_PHASES` 布局、边推导、显式刷新 + Channel 例外口径不变 |
