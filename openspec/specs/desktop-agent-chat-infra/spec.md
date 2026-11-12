# desktop-agent-chat-infra Specification

## Purpose

定义 agent 会话统一基建的前端契约：ai-sdk v7 状态层（`UIMessage` 一等模型 + `TauriAgentTransport`）、事件适配层（`BlockPlan` IR 三层：`planBlock` 单语义点 + 视野层 `PairingScope` + 发射层 `plansToParts` / `plansToChunks`，保真锁定）、会话级统一渲染组件（`AgentSessionView`）、headless 会话基建（`use-agent-session`：重放装载 + 按会话状态挂流 + 发送组装 + 停止接线）、展示组件族（会话级入口 + 对话透镜 + composer + 工具卡注册表），以及 ai-sdk 依赖收口纪律。目标：今后所有 agent 会话页面（explore 会话 / agent 调试 / change 流程抽屉 / 历史重放 / 未来）统一消费本基建，一处实现「按 sessionId 渲染一个会话并按状态决定是否挂流」。

## Requirements

### Requirement: 统一会话状态基建(use-agent-chat)

前端 SHALL 提供 headless 会话基建 hook `use-agent-session`(置于 `packages/desktop/src/hooks/`;由 `use-agent-chat` 收编改名,职责扩为「重放装载 + 状态驱动挂流 + 发送组装 + 停止接线」),内部以 ai-sdk v7 `useChat` 承载对话状态,transport 为自定义 `TauriAgentTransport`(实现 v7 `ChatTransport`),把会话发起 invoke 与**会话订阅 invoke**(Tauri `Channel<AgentRunMessage>`:发起流与订阅流共用同一信封)适配为 `ReadableStream<UIMessageChunk>`。`UIMessage` SHALL 成为前端对话状态的一等模型。基建 SHALL 满足:

- **重放装载**: 打开会话时以会话 id **直查**会话单查命令 `session_detail(root, sessionId)`(携 root 寻址所属 workspace 库),再经会话转录重放取落库**密封转录**,经 `foldSessionMessages(events, turns)` 纯函数重建 UIMessage 状态(`setMessages`);会话 id 由来源侧显式提供(`sourceRef` 反查取最近一条会话的路径 SHALL 退役;按 `source` / `source_ref` 列出会话供用户显式挑选的**清单筛选**不在此禁令内——属探测语义,选中后仍以 `sessionId` 直查)。重放重建的状态与实时累积收口的状态 SHALL **终态同一形状**(重开恢复 = 重放;两路契约见「事件适配层」的「终态同构 + 过程就地补全」,终态一致性由收口以重放收敛保证,而非由两条发射实现碰巧一致);`sessionId` 为 null 时不发起任何重放查询、不建立任何订阅。
- **发送穿透**: 发送 SHALL 经 `sendMessage` 以 `ChatRequestOptions.body` 携带应用层会话参数(`root`、`permissionMode`、会话引用 `sessionId`(`Continue` 语义,链尾/当前会话提供,无会话为 null)、`source`、可选 `sourceRef`、可选 `cwd`);会话状态由应用层在发送时刻提供,transport SHALL 为无状态转换器(不进构造闭包、无 ref 插线);会话文本组装(如 explore stance 拼接)SHALL 留在来源侧,基建只收口会话参数。`sourceRef` SHALL 由来源侧按需提供(如 change 详情通用 agent 面板以 change id 归属,见 desktop-change-chat),仅作归属记录与清单筛选,**MUST NOT 用于寻址或重放**;来源未提供时为 null(调试页 / explore 链行为不变)。`cwd` SHALL 同样由来源侧透传(change 环境 = worktree 执行根),只作会话执行根,MUST NOT 参与 store 寻址;来源未提供时缺省 `root`(既有语义)。transport SHALL 对两字段做运行时校验(非法形态即 `Err`,不改写),且会话参数 SHALL 以具名解构进入发起 invoke——MUST NOT 依赖位置参数索引拼接(参数面尾部增位时不得错位)。
- **单消息回合**: 一轮运行(提问 + 思考 + 工具 + 回答)SHALL 呈现为**恰一条 assistant 消息**(气泡数一),部件数不受限——思考 / 正文 / 工具卡各占一个 part,按**首次到达顺序**排列,MUST NOT 重排、MUST NOT 刻意合并;MUST NOT 出现同一轮的第二个 assistant 气泡。
- **step 分段(增量流式)**: 实时路 SHALL 以「一次模型响应 = 一个 step」分段:`start` 每轮**仅发一次**(`messageId = turn-<turnId>`,`turnId` 取自该轮首个事件的信封 `AgentEvent.turn_id`);新响应首个 delta 前发 `start-step`;密封 `Message` 到达时先 `reset-step`(只清本 step 的临时部件)再追加整块部件,前面 step 的已密封块 MUST 保留;工具结果 SHALL 就地补全既有工具部件(只投 `tool-output-available`,不开新 step)。delta SHALL 经 `eventToChunk` 映射为 `text-delta` / `reasoning-delta` 累积进本轮唯一消息的当前 step 部件,MUST NOT 出现逐 delta 碎行;part id SHALL 由**该 step 内首个同类 delta 的 `seq`** 派生(`t-<seq>` / `r-<seq>`),同一 part 的后续 delta MUST 复用同一 id,跨 step MUST NOT 相撞。实时路的起点 SHALL 双态:发送期由发起流承载(Channel 先于 invoke 建立,零空洞),旁观 / 重挂期由会话订阅流承载(带 `since_seq`);两起点 SHALL 经同一适配与同一 step 分段簿记单点产生 chunk,MUST NOT 各自实现一份;同一会话同一时刻 SHALL 至多一条在挂流。
- **提问消息**: `role = user` 的密封 `Message` 事件 SHALL NOT 由 transport 转为 chunk(chunk 协议无 role 字段,硬造必产第二条 assistant 消息)。提问气泡 SHALL 由会话状态侧产生并**恰一条**,其**最终** id SHALL 为 `turn-<turnId>-user`(与 assistant 消息 `turn-<turnId>` 同族):发起流在发送时刻本地推入(同步先于任何 chunk,故结构上不可能出现「思考在提问之前」,此时可先持临时 id),该轮提问事件到达时把 id **归一**为 `turn-<turnId>-user`(事件不可达——断流 / 收口即到——时由收口 fold 整表替换对齐,用户无感);订阅流(旁观 / 重挂,无本地副本)在收到该轮提问事件时直接以 `turn-<turnId>-user` 追加。MUST NOT 让同一提问在收口后仍持临时 id 或出现两条气泡;MUST NOT 以 `seq` 猜测角色。
- **停止**: `stop` SHALL 触达 Rust 侧终止命令(MUST NOT 仅截断前端流);后端收敛后经由在挂流(发起流或订阅流)流出的 `Record` 信封照常进入状态,并落到该轮 assistant 消息的 `metadata.record`。
- **record 归元数据**: 终态轮行 SHALL 经 `finish` chunk 的 `messageMetadata` 挂为该轮 assistant 消息的 `metadata.record`(不产独立消息、不产 `data-run-record` 部件);重放路径 SHALL 由折叠按轮把轮行挂到同一位置;轮收口 SHALL 以转录库重放重建会话状态(终态以库为唯一权威;收口查询失败回落本地事件镜像折叠),续轮(取当前会话)从中取。`chain` 状态面 SHALL 照旧由轮行镜像维护(顶部失败横幅与 `running` 派生读它,不读 `metadata`)。

#### Scenario: 重放装载重建

- **WHEN** 打开一条已有两轮往复的会话(来源侧提供其 `sessionId`)
- **THEN** `use-agent-session` 以 `sessionId` 直查 `session_detail` 并取会话转录重放,`foldSessionMessages` 重建出的 UIMessage 序列与该会话收口后的状态形状一致(每轮提问 + 一条 assistant 消息在场,轮行挂在该轮 `metadata.record`,无缺轮);assistant 消息 id 为 `turn-<turnId>`、提问为 `turn-<turnId>-user`

#### Scenario: 会话参数穿透

- **WHEN** 在已有会话(会话 id=`s1`)中发送一条消息
- **THEN** `TauriAgentTransport.sendMessages` 收到的 options.body 含会话引用 `sessionId="s1"` 及 root / permissionMode / source,并原样进入会话发起 invoke 参数;transport 自身不持有会话状态

#### Scenario: 单消息回合与 step 分段

- **WHEN** 一轮含两次模型响应(响应间夹一次工具调用)的回复经实时流到达,投递顺序为 `RunStarted` → user `Message` → delta₁ → 密封 `Message`(A₁) → toolResult → delta₂ → 密封 `Message`(A₂) → `Record`
- **THEN** 归约后该轮恰一条 assistant 消息:id 为 `turn-<turnId>`,部件按 A₁ 段与 A₂ 段的到达顺序排列(思考 / 正文 / 工具卡各一 part),A₁ 段部件在 A₂ 段到达后仍在场;`start` 全轮仅一次、`start-step` 每段一次;该轮提问消息为右侧 `role = 'user'` 气泡且排在该 assistant 消息之前;轮末 `metadata.record` 为该轮轮行

#### Scenario: 订阅流起点与簿记等价

- **WHEN** 旁观 (非发送方) 打开一条 running 会话,订阅流自 `since_seq` 后的首个 delta 到达
- **THEN** 该流复刻发起流的同一 step 分段簿记(`start` + `start-step` + part-start + 以首个同类 delta 的 `seq` 派生 part id)后累积 delta,reducer 归约结果与发起流对同一事件序列的归约一致(无 `missing part` 错误),且两起点共用同一实现

#### Scenario: 停止触达后端

- **WHEN** 运行中调用 `stop`
- **THEN** 前端发起 Rust 侧终止命令(收敛引擎与轮状态),在挂流随后流出的 `Record` 信封进入会话状态并挂为该轮 `metadata.record`(status = `stopped`),MUST NOT 出现「前端已停、store 仍跑完」的偏差

#### Scenario: 来源引用与 cwd 穿透

- **WHEN** 来源侧以 `source = "change"`、`sourceRef = <change id>`、`cwd = <worktree>` 发送一条消息
- **THEN** `TauriAgentTransport.sendMessages` 收到的 body 含三字段并原样进入发起 invoke 参数(`source_ref` / `cwd` 位于参数面尾部);transport 无状态、零改写

#### Scenario: 未提供时缺省不漂移

- **WHEN** 调试页 / explore 链发送消息(不提供 `sourceRef` 与 `cwd`)
- **THEN** body 两字段为 null,发起 invoke 行为与改造前逐字一致(执行根 = `root`,归属 `sourceRef` 为 null)

#### Scenario: 清单筛选与寻址纪律并存

- **WHEN** 来源侧按 `source` / `source_ref` 拉取会话清单并点选其中一条
- **THEN** 清单仅作探测 / 选择入口;选中后的重放装载仍以 `sessionId` 直查 `session_detail` 与会话转录,零「按来源反查取最近一条」调用

#### Scenario: 提问气泡不靠 seq 猜角色

- **WHEN** 在已有会话中续问第二轮(运行中 transport 同时收到该轮 `role = user` 的密封 `Message` 事件)
- **THEN** 该轮的提问气泡为 `role = 'user'`(右侧)且排在 assistant 消息之前,id 为 `turn-<turnId>-user`;transport 对该 user 事件零 chunk 产出;若本地已推入该提问,事件到达后 id 归一到同一值(消息总数不增,无两条提问气泡)

#### Scenario: 旁观路径的提问气泡

- **WHEN** 旁观方(无本地提问副本)经会话订阅流收到某轮的 `role = user` 密封 `Message` 事件
- **THEN** 该提问以 `turn-<turnId>-user` 追加进消息列表并排在该轮 assistant 消息之前;收口后经 `foldSessionMessages` 整表替换,提问仍恰一条、id 不变

#### Scenario: 旧转录顺序分组回退

- **WHEN** 重放一条 `turnId` 全为 `null` 的存量转录(前置变更前落库)
- **THEN** 折叠按 `RunStarted` 出现顺序分组(组界 = 下一枚 `RunStarted` 或数组末尾),每轮仍产出提问 + 一条 assistant 消息,消息数与本变更前形状等价、不抛错;MUST NOT 引入第二套分组实现

### Requirement: 会话级统一渲染组件（sessionId 入参 + 状态驱动挂流）

前端 SHALL 提供会话级统一渲染组件 `AgentSessionView`（`packages/desktop/src/components/agent/agent-session-view.tsx`）与其 headless 基建 `useAgentSession`（`packages/desktop/src/hooks/use-agent-session.ts`），作为「按 sessionId 渲染一个 agent 会话」的唯一实现：全部会话消费点（Agent 调试页 / explore 详情 / change 流程抽屉 / 历史重放）SHALL 消费本组件，MUST NOT 在视图层各自拼装消息、轮行或事件流。组件入参 SHALL 为 `root` + `sessionId`（会话唯一寻址键；MUST NOT 以 `source` / `source_ref` 反查会话），可选 `refreshKey`（会话状态重评估信号）与 `composer` 插槽（发送 / 停止动作面的注入位）。契约：

- **通用接口渲染**：挂载或 `sessionId` 变更 SHALL 经通用查询面装配会话——`session_detail(root, sessionId)` 单查取会话行 + 聚合统计 + 轮行，`agent_session_transcript(root, sessionId)` 取密封转录，经适配层纯函数重建消息状态（重放与收口折叠同一入口）；`sessionId === null` SHALL 呈空态且零取数、零订阅。
- **按会话状态挂流**：会话「进行中 / 完成」SHALL 由通用查询面推导（轮行清单含 running 轮行即进行中，与库态判定既有先例同源），MUST NOT 以前端发送方视角（`chat.status`）替代库态。进行中且无在挂流 → SHALL 建立会话订阅（`agent_session_watch`，`since_seq` = 已装载转录最大 `seq`）；完成态 → MUST NOT 建立订阅（零轮询、零定时刷新、零事件订阅）。状态重评估 SHALL 由挂载 / `sessionId` 变更 / `refreshKey` 触发。
- **单流在挂**：同一会话同一时刻至多一条在挂流（发起流 = 发送方 `agent_start` 的 Channel，或会话订阅流）；已在挂时 MUST NOT 再订阅（防同轮增量重复注入）。流以轮终态收口关闭，关闭后 SHALL 依会话状态重评估：仍进行中 → 以新 `since_seq` 重挂；已收敛 → 不挂。
- **过程增量 + 终态收敛**：流内 `messageDelta` SHALL 经适配层映射为 `text-delta` / `reasoning-delta` chunk 由 ai-sdk reducer 累积（token 级连续增长、思考 / 回复可辨）；轮收口（流结束）SHALL 以**转录库重放**重建消息状态——终态同构由「同一来源」保证（重放与收口折叠同一入口），MUST NOT 依赖两条发射实现碰巧一致；收口重放查询失败 SHALL 回落本地事件镜像折叠（收口不阻塞、不抛错）。
- **订阅起点部件重建**：会话订阅流 SHALL 在首个实时事件到达时，先按该轮把**已装载转录里同轮的密封事件**预投进同一簿记（等同该轮历史上原本就会走的帧序列），再投该实时事件。动因：`resume-stream` 在 ai-sdk 中以空消息状态起步，首个 chunk 的 `start` 命中「替换末条」——不预投则运行轮已落库的部件会被增量独占覆盖，表现为「打开进行中会话时历史一闪而过只剩新流」。预投 MUST NOT 另起 reducer 消息、MUST NOT 重复计入镜像（既有事件早已在镜像中），且 MUST NOT 在跨轮事件到达时错配（按轮次匹配后才投）。
- **断流兜底**：订阅流异常中断或服务端滞后断流 SHALL 触发「重放 + 重订」——重查转录库刷新镜像后以新 `since_seq` 重挂；MUST NOT 静默续跑（无「缺一段增量仍装作在线」的假态）。
- **动作面可选**：发送 / 停止 SHALL 为可选动作面（headless 暴露、composer 插槽注入），只读消费点（change 抽屉 / 历史重放）MUST NOT 被迫接线；订阅流在挂期间 `chat.status` 处于 `streaming`，既有发送守卫 SHALL 自然拦下旁观误发（MUST NOT 以「旁观恒 idle」的发送方视角放行）。

#### Scenario: 挂载按通用接口渲染

- **WHEN** 以 `root` 与既有会话 `sessionId` 挂载 `AgentSessionView`（该会话已收口、含两轮往复）
- **THEN** 组件经 `sessionDetail` + `agentSessionTranscript` 装配并呈现两轮对话与轮行（无缺轮），且不发起任何会话流订阅

#### Scenario: 完成态零订阅

- **WHEN** 会话已收敛（无 running 轮行）且组件挂载或 `refreshKey` 变化
- **THEN** 零 `agent_session_watch` invoke、零轮询与定时刷新；仅状态重评估时重查查询面

#### Scenario: 进行中挂流呈 token 级增量

- **WHEN** 挂载时会话存在 running 轮行（旁观 / 页面重挂场景）
- **THEN** 组件以 `since_seq` = 已装载转录最大 seq 建立会话订阅，delta 经 reducer 连续增长为单条消息（思考 / 回复可辨），不出现逐 delta 碎行

#### Scenario: 运行中会话的既有部件不被增量覆盖

- **WHEN** 打开一条 running 会话（该轮已落库若干密封事件），随后订阅流到达同一轮的增量
- **THEN** 该轮已落库的部件（正文 / 思考 / 工具卡）与到达的增量同处一条 assistant 消息且全部在场；更早的轮次零串扰；MUST NOT 出现「历史先渲染再被新流冲掉」的中间态

#### Scenario: 单流在挂不重复增量

- **WHEN** 页面经组件发送一条消息（发起流在挂）后该会话转为进行中
- **THEN** 组件不为该会话再建立订阅（在挂流唯一），同轮增量只注入一份

#### Scenario: 收口以重放收敛

- **WHEN** 一轮运行收口（流以终态 Record 结束）
- **THEN** 组件重查该会话转录并以其折叠重建消息状态，重建结果与直接打开该会话重放的形状一致（同一条折叠入口）

#### Scenario: 断流重订兜底

- **WHEN** 会话订阅流因服务端滞后（broadcast 溢出）或异常中断
- **THEN** 组件重查转录库刷新镜像后以新 `since_seq` 重挂；期间不出现「丢失一段增量而状态仍显示流式在线」的假态

#### Scenario: 只读消费点零动作面

- **WHEN** change 流程抽屉或历史重放以只读方式消费组件（不传 composer 插槽）
- **THEN** 组件不呈现发送 / 停止入口、不发起 `agent_start`；运行中会话的增量照常只读流式呈现

### Requirement: 事件适配层(纯函数,保真)

基建 SHALL 提供纯函数适配层(置于 `packages/desktop/src/lib/agent-adapter.ts`),分三层:入口层、语义层(IR)、发射层:

- `foldSessionMessages(events: AgentEvent[], turns: TurnSummary[]): UIMessage[]` — 重放与收口折叠(**按轮分组**;仅消费**密封事件**:会话转录中不存在 delta,delta 不进重放路径)。分组键 SHALL 为信封 `AgentEvent.turn_id`(同一 `turn_id` 归同一轮);`turn_id` 为 `null` 的旧转录 SHALL 按 `RunStarted` 出现顺序分组(组界 = 下一枚 `RunStarted` 或数组末尾)。每轮 SHALL 产出:提问消息(该轮 `role = user` 密封事件;缺席即无提问消息)+ **恰一条** assistant 消息(id `turn-<turnId>`,parts 可为空)+ 逐事件 aux system 消息,且 `metadata.record` 为该轮轮行(按 `turnId` 匹配;轮行缺席为 `null`)。`TurnDone` MUST NOT 用作轮边界或轮行计数依据,仅在该轮轮行缺席时兜底 status(`isError ? failed : completed`)
- `eventToChunk(event: AgentEvent, ...): UIMessageChunk[]` — 实时增量的**块 chunk 单点**(delta 路 + 密封块路);**事件头与 step 帧(`start` / `start-step` / `reset-step` / `finish`)SHALL 归 transport**,本函数 MUST NOT 产。`role = user` 的密封事件 SHALL 零 chunk(提问由会话状态侧承载)。可选参为**流内跨消息配对视野**(transport 流内簿记:本轮已发出的工具调用 id 集),命中即就地补全
- **语义层 IR**: `BlockPlan` 为块语义的唯一表达(text / reasoning / tool 三变体;tool 的 `output: null` 即未配对),`planBlock(block, paired): BlockPlan | null` 为**唯一语义点**(五 block 变体 + 无主 result 合成在此收口;`null` = 已配对 result 不重复呈现);孤儿 result 与正常 tool 合并为同一 `tool` 变体(占位工具名 + `input: null`),MUST NOT 在两路各写一遍孤儿判定。`text` 变体 SHALL 携 `citations?: AgentCitation[]`(缺席 = 无引用),`tool` 变体 SHALL 为**唯一工具类承载位**——客户端工具与云端 `web_search` 服务端工具共用该变体(`toolName` 区分,MUST NOT 为搜索新增第四变体)
- **视野层**: `PairingScope` 隔离配对视野差异——折叠路 `global`(跨消息预扫全部 toolUse 的配对 result),实时路 `local`(仅当前 message 内配对);两视野 SHALL 产出同一类型 `Map<toolCallId, PairedOutcome>`(配对载体为工具结果块 **或搜索来源载荷**的联合,与块种类同构),差异 MUST NOT 渗入语义层与发射层。实时路另有**流内跨消息配对视野**(`eventToChunk` 可选参,transport 按「本轮最近开件 step 的消息」簿记并在新 step 开出时重武装):命中的 toolResult 块 SHALL 就地补全既有消息里的工具部件——只投 `tool-output-available`,MUST NOT 另起消息 / 重投 input / 开新 step;全部块均命中时该事件 MUST NOT 产任何块 chunk。**搜索块配对 SHALL 以块 `id` 为键沿用同一视野机制**:use 侧块(`rawUse != null`)与 result 侧块(`rawUse == null`)分处两条密封 message 时由 `global` 视野跨消息合并(`pause_turn` 拆分场景),`local` 视野下无配对 result 的 use 仍渲染(只显查询词)
- **发射层**: `plansToParts` / `plansToChunks` 为纯机械映射(无判定、无视野),两路的块呈现 SHALL 由同一 `BlockPlan[]` 驱动;两者均只产块相关产物(部件 / chunk),**事件头与 step 帧由 `eventToChunk` 之外的 transport 负责**。text 计划的 `citations` SHALL 由 `plansToParts` / `plansToChunks` 挂为同一份 `providerMetadata: { agent: { citations } }`(空数组 MUST NOT 挂;`text-start` 与 `text-delta` 带同一 metadata——ai-sdk 的 metadata 合并语义为**覆盖非累加**,故 citations 放一次即可,MUST NOT 指望多次 delta 累加)

映射 SHALL 为: `runStarted` → `data-run-started` 部件;`messageDelta` → 本轮 assistant 消息当前 step 的 `text-delta`(文本增量)/ `reasoning-delta`(思考增量)chunk,part id 由该 step 内首个同类 delta 的 `seq` 派生(同一 part 后续 delta 复用);密封 message 的 text block → text 部件(整块追加,`t-<seq>-<n>`);thinking block → reasoning 部件;toolUse block → tool 部件 input;同 id toolResult block → 该 tool 部件 output(配对收口于适配层,跨消息按 id 配对,MUST NOT 再由视图层全局扫描);**`webSearch` block → `tool-web_search` 部件**(`toolCallId` = 块 `id`、`input = { query }`、`output` 见下);`systemNotice` → `data-system-notice`;`turnDone`(现 runResult,随信封演进) → `data-run-result`(携带 finish 语义;转录内事件的保真面,与 `metadata.record` 是两个层级,MUST NOT 互相替代);`raw` → `data-raw` 原文透传。`eventToChunk` 的 delta 分支与 `agent-transport.ts` SHALL **零搜索改动**(无搜索实时 delta)。

搜索块的部件载荷口径: use 侧块 → `input = { query }`,`output` 为配对命中时的来源载荷或 `null`;result 侧块(孤儿,use 已被裁)→ `input = null` + `output` 携带来源载荷(信息不丢,与既有孤儿工具卡片同哲学);两者 `output` 形状 SHALL 为 `AgentToolOutput` 的**加位形态**——`content` 保留字符串位(搜索无文本结果,置空串),`isError` = `error != null`,`search?: { results: WebSearchSource[]; error: string | null }` 承载结构化来源与错误(加位可选,既有消费面零改)。

两路契约 SHALL 为**终态同构 + 过程就地补全**:折叠路(重放 / 收口)与实时路 SHALL 在轮收口后呈同一形状(MUST NOT 出现成对工具卡的分裂或重复);实时流式期间跨消息 tool 配对 SHALL 就地补全——结果事件不另起消息、input 与 output 合住同一条消息(与折叠路同形)。配对视野随新 step 开出重武装,失效残差(结果与 toolUse 之间插入其他 step / 订阅起点晚于 toolUse)SHALL 回退既有占位卡形态并由轮收口以重放折叠纠正,MUST NOT 以「实时路复跑折叠语义」的方式消除(reducer 的配对视野物理上限于当前 step)。视野差异 SHALL 由视野层快照测试显式锁定(同消息配对两视野相等、未携流内视野时跨消息配对两视野可辨差异),MUST NOT 由单一路径 fixture 掩饰。

**id 锚点** SHALL 两路同源:assistant 消息 id = `turn-<turnId>`、提问消息 id = `turn-<turnId>-user`(折叠路与实时路 MUST 算出同一 id,否则收口时 ai-sdk 的 `isContinuation` 判定失配、消息重挂);`turnId` 缺席(旧转录 / 无轮归属构造面)时实时路 SHALL 以 `evt-<首事件 seq>` 兜 id,收口折叠以整表替换对齐(MUST NOT 为旧转录造第二套折叠路径)。

保真 SHALL 锁定: `seq` 可考(进消息 metadata 与部件元数据,保序与 key 稳定;delta chunk 元数据携带源 seq);`parentToolUseId` 可考(进部件元数据,子代理归因依赖它);未知事件 MUST NOT 丢弃(一律 `data-raw` 透传原文)。aux 事件(`runStarted` / `systemNotice` / `raw` / `turnDone`)SHALL 维持**逐事件独立 system 消息**(MUST NOT 并入轮 metadata:保真透镜 `AgentRawStream` 依赖逐事件回放,并入即失去数据源)。

穷尽性 SHALL 编译期可探测: `auxEventData` 与 `planBlock` SHALL 以 `const _exhaustive: never` 承接变体穷尽(新增 `AgentEvent` / `AgentBlock` 变体时编译报错提醒,MUST NOT 仅靠 runtime 静默降级 `data-raw`);runtime 未知变体的 `data-raw` 兜底语义照常保留(编译期穷尽 + runtime 兜底两层)。手写事件载荷镜像(如 `AgentRunStartedData` / `AgentSystemNoticeData` / `AgentRawData`)SHALL 从生成绑定派生(`Extract<AgentEvent, { kind: … }>`),MUST NOT 保留无生成物守卫的重复定义(信封加变体时须编译报错)。

> 2026-10-23 回溯修订(无 change 流直修):实时路补流内跨消息配对视野(`eventToChunk` 可选参 `currentMessageToolCalls` + transport 按「最近开件消息」簿记),tool 结果到达即就地补全、不再另起第二条消息;视野失效残差(插入其他消息 / 订阅起点晚于 toolUse)维持占位卡回退,终态仍由轮收口以重放折叠保证。

> 2026-10-23 anthropic-web-search 修订:`AgentBlock` 增 `webSearch` 变体与 `text.citations` 字段;语义层/视野层/发射层各自扩面,**不新增第四种计划变体、不新增 data 部件成员、不改 delta 路与 transport**(搜索活动走既有工具卡管线,引用挂 text 部件 `providerMetadata`)。

> 2026-11-06 agent-stream-single-message 修订:入口 `eventsToUIMessages` → `foldSessionMessages(events, turns)`(按 `turn_id` 分组、轮行挂 `metadata.record`);实时路改「一轮一条 assistant 消息 + step 分段」,事件头与 step 帧收归 transport;`provisionalMessageId` / `runRecordToUIMessage` / `data-run-record` 退役;`deltaPartId` 改 `(seq, part)` 派生。

#### Scenario: 全变体映射

- **WHEN** 以密封五变体 `AgentEvent` 序列(runStarted / message(五 block 变体:text / thinking / toolUse / toolResult / webSearch)/ systemNotice / turnDone / raw)驱动 `foldSessionMessages`
- **THEN** 产出:该轮一条 assistant 消息(部件序为 text、reasoning、tool)、逐事件 aux system 消息(`data-run-started` / `data-system-notice` / `data-run-result` / `data-raw`),`metadata.record` 为该轮轮行;部件顺序与 seq 一致

#### Scenario: delta 路映射

- **WHEN** `eventToChunk` 先后收到 text delta 与 thinking delta 事件(同一 step 内)
- **THEN** 产出 `text-delta` 与 `reasoning-delta` chunk(思考/回复可辨),各自 part id 由该 step 内首个同类 delta 的 `seq` 派生且后续 delta 复用同一 id;本函数不产 `start` / `start-step` / `reset-step`

#### Scenario: 工具同 id 配对

- **WHEN** toolUse(id=`t1`)与 toolResult(id=`t1`)分处不同密封 message
- **THEN** 适配后 `t1` 收敛为单个 tool 部件(input 与 output 同住),视图层无需跨消息扫描配对

#### Scenario: 实时流跨消息工具结果就地补全

- **WHEN** 实时流中密封 assistant(携 toolUse id=`t1`)后,同 id toolResult 以另一事件到达,且其间未插入其他 step
- **THEN** toolResult 事件只产出 `tool-output-available`(无 `start` / `start-step` / `reset-step`、不重投 input),reducer 归约后 `t1` 的 input 与 output 合住同一条消息(无双卡、无第二条气泡、消息 id 仍为 `turn-<turnId>`)

#### Scenario: 搜索块两态与视图层零扫描

- **WHEN** 密封 message 含 `webSearch` use 侧块(id=`srvtoolu_1`)与（另一条消息中）同 id result 侧块
- **THEN** `planBlock` 分别产出 `tool` 计划（use 侧携 `output` 来源载荷、result 侧返回 `null` 不重复呈现）;`foldSessionMessages` 后 `srvtoolu_1` 收敛为单个 `tool-web_search` 部件（`input = { query }`、`output.search.results` 在场）,视图层无需跨消息扫描

#### Scenario: 引用挂 text 部件元数据

- **WHEN** 密封 text 块携 `citations`(两条同 url 的引用)
- **THEN** 对应 text 部件带 `providerMetadata.agent.citations`(两条原样,去重责任在呈现层);`citations` 为空数组或缺席时不挂该键(`providerMetadata` 不出现空壳)

#### Scenario: 保真字段可考

- **WHEN** 检查适配产出的任一 UIMessage
- **THEN** 可从消息 metadata 或部件元数据读出原事件 `seq`,message 部件元数据保留 `parentToolUseId`(含子代理消息的非空归因);aux 事件仍各为独立 system 消息且原文未丢

#### Scenario: 未知事件不丢

- **WHEN** 事件流含未知类型事件(未来引擎新增)
- **THEN** 产出 `data-raw` 部件且原文 JSON 完整保留,适配不报错、不丢事件

#### Scenario: 计划层对拍同形

- **WHEN** 同一 `BlockPlan[]`(text / reasoning / tool 已配对 / tool 未配对四形态)分别喂 `plansToParts` 与 `plansToChunks`(后者经 reducer 归约)
- **THEN** 两路呈现的形状一致(部件类型 / 顺序 / 工具态 / output 载荷逐一相等),对拍不再依赖第三份手写语义投影(`shapeOf` 类辅助退役)

#### Scenario: 视野差异显式锁定

- **WHEN** toolUse 与同 id toolResult 分处两条密封 message(跨消息配对场景)
- **THEN** 视野层快照显式断言: `global` 视野折叠为单个 `output-available` 计划;`local` 视野(未携流内跨消息视野时的纯函数面)产出 `input-available` 计划 + 孤儿卡 `output-available` 计划(差异具体形态被锁定,防扩大);同消息内配对两视野相等

#### Scenario: 收口折叠纠正残差双卡

- **WHEN** 实时流式期间配对视野已失效(结果与 toolUse 之间插入其他 step / 订阅起点晚于 toolUse),跨消息 tool 配对闪现「占位卡 + 结果卡」双卡
- **THEN** 轮收口以重放折叠纠正为单张 `output-available` 工具卡(终态同构),MUST NOT 出现收口后仍双卡或部件重复

#### Scenario: 变体穷尽编译期可探测

- **WHEN** 审查 `auxEventData` / `planBlock` 实现
- **THEN** 两者均以 `const _exhaustive: never` 承接穷尽;新增 `AgentEvent` / `AgentBlock` 变体而漏改时编译报错,runtime 未知事件的 `data-raw` 兜底仍保留

#### Scenario: 按轮分组不依赖 RunStarted

- **WHEN** 转录为一轮不含 `TurnDone` 的停止轮(其轮行 status = `stopped`)紧随一轮正常收口轮
- **THEN** 折叠按信封 `turn_id` 分组:停止轮的 assistant 消息与提问消息在该轮位置、其 `metadata.record` 为 `stopped` 轮行;后续正常轮的轮行 MUST NOT 错位(第 2 轮轮行落在第 2 轮);折叠实现零 `RunStarted` 计数逻辑

#### Scenario: 空内容轮恒产一条消息

- **WHEN** 某轮事件只有 `RunStarted` 与轮行(零 assistant 密封消息,如 ask 失败 / 立即停止)
- **THEN** 该轮仍产出恰一条 assistant 消息(id `turn-<turnId>`、`parts` 为空、`metadata.record` 为该轮轮行);MUST NOT 因 `parts` 为空而缺轮或抛错

#### Scenario: id 锚点两路一致

- **WHEN** 同一轮事件序列分别经实时路(transport chunk 归约)与折叠路(`foldSessionMessages`)处理
- **THEN** assistant 消息 id 均为 `turn-<turnId>`、提问均 `turn-<turnId>-user`,`turnId` 缺席时实时路以 `evt-<首事件 seq>` 兜 id 并由收口整表替换对齐

### Requirement: 展示组件族(对话透镜)

> 2026-10-10 回溯修订(无 change 流直修):保真透镜 `AgentTimeline` 撤编,组件族收敛为 `AgentMessages` 单对话透镜——辅助载体(system 角色消息:run-started / system-notice / raw / run-result;2026-11-06 起 `run-record` 退役,轮行改挂回合元数据)整条不进对话呈现;保真面由适配层状态部件(序列与归因字段见「事件适配层」requirement)与调试页页面级「原始 JSONL」切换承载,不再双透镜并行。

`packages/desktop/src/components/agent/` SHALL 提供共享展示组件族,全部 agent 会话页面(explore 会话 / agent 调试 / change 流程抽屉 / 历史重放 / 未来页面)统一消费:

- `AgentSessionView` 会话级唯一入口: 入参 `root` + `sessionId`(+ 可选 `refreshKey` / composer 插槽),内部经通用查询面装配会话并按会话状态挂流(契约见「会话级统一渲染组件」requirement);消费点 MUST NOT 自行拼装消息、轮行或事件流;
- `AgentMessages` 对话透镜: user / assistant 气泡(**一轮一条 assistant 气泡**,部件按到达顺序)、工具对卡、思考折叠、AskUserQuestion 静态卡(问题与选项原样可读);超长 user 提问 SHALL 默认折叠为预览(展开后回到原部件渲染,见「超长 user 提问默认折叠」);辅助载体消息(system 角色承载的 data 部件)MUST NOT 进对话呈现,对话透镜 SHALL 基于过滤后列表判定空态;空内容轮的可见性 SHALL 按该轮 `metadata.record.status` 判定而非 `parts` 是否为空(失败 / 停止轮呈轮内状态行,见「轮内状态行与空内容轮可见性」);
- `AgentInput` composer: prompt 输入（空串禁发;纯空白字符不单独拦截——trim 守卫不在组件侧）、permission-mode 档位（`StandardSelect` 选择器）、发送、停止入口;并 SHALL 提供**整面板禁用态**(`disabled` + 禁用理由文案——输入框与发送同时禁用,理由文案原样呈现),供来源侧按业务判据(如 change 流程运行中 / 归档只读,见 desktop-change-chat)整面板禁发;`running` 语义 SHALL 为「呈停止入口」——发送入口不禁用,重复发送由会话层 `isStreaming` 守卫忽略;停止入口 SHALL 只随 `running` 呈现且 MUST NOT 被 `disabled` 隐去(运行中即便整面板禁用仍可从面板停止);`disabled` MUST NOT 复用为「运行中」的第二表达;
- 工具卡注册表: 特化卡(AskUserQuestionCard 等)为可插拔位,未注册工具名回退默认对卡;
- 组件族 SHALL 共享底层块渲染件(文本 markdown / 思考折叠 / 工具卡一套实现);滚动 SHALL 由 `components/ui/message-scroller.tsx` 承载——该 shim 内部以 `@tanstack/react-virtual` 渲染**可见窗口切片**,并通过 sticky 吸附层让用户提问常驻(契约见「对话透镜虚拟窗口与用户消息顶部吸附」);消息状态仍留在应用层,MUST NOT 以分页窗口限制进入 DOM 的消息集合(MUST NOT 保留自研历史分页层)。上游 `message-scroller` primitives 退役后 `@shadcn/react` 在仓内再无**可达**消费者,该依赖与其零消费者残件(`components/ui/questionnaire.tsx`)SHALL 一并剪除(判据沿用 desktop-app-shell「knip 报出的未用导出 SHALL 删减」),MUST NOT 新增 `ignoreDependencies` / `ignore` 条目把未用面关成豁免。

调试页的 run 表单 / 历史列表 / JSONL 开关 SHALL 为页面级 chrome,包在共享核心外圈,MUST NOT 强行塞进组件族。

会话呈现形态 SHALL 归一为一个: 交互页(调试 / explore)在 composer 插槽注入自己的 composer(文本组装留来源侧),只读消费点(change 流程抽屉 / 历史重放)不接插槽;三形态历史差异(发起流独占 / 通知重查转录库 / 裸事件折叠无轮行)SHALL 随本次收编退役,MUST NOT 保留第二套会话渲染实现或第二套转录查询 hook。

#### Scenario: 对话透镜映射

- **WHEN** 会话状态含文本、思考、工具调用与工具结果
- **THEN** `AgentMessages` 按序呈现 markdown 气泡、可展开思考块、成对工具卡,AskUserQuestion 呈静态可读卡

#### Scenario: 辅助载体不进对话

- **WHEN** 会话状态含 run-started / system-notice / raw / run-result 等 system 载体消息
- **THEN** 对话透镜不呈现其任何 data 部件(空态判定基于过滤后列表),seq / `parentToolUseId` 保真仍可考于部件元数据

#### Scenario: composer 运行中重复发送与停止

- **WHEN** 会话处于运行中
- **THEN** `AgentInput` 呈现停止入口,发送入口保持可用;重复发送由会话层 `isStreaming` 守卫忽略(`agent_start` 零发起);停止后恢复可发送

#### Scenario: 会话级入口归一

- **WHEN** 检索全部会话呈现消费点(调试页 / explore 详情 / change 流程抽屉 / 历史重放)
- **THEN** 全部经 `AgentSessionView` 渲染,视图层无折叠入口(`foldSessionMessages`)直调、无自有转录查询 hook、无自有事件流装配

#### Scenario: 只读消费点形态

- **WHEN** change 流程抽屉以只读方式渲染某 WorkerAgent 会话
- **THEN** 组件呈对话透镜与各轮状态(轮行以回合元数据承载、由该轮消息呈现,不再是独立 system 消息),不呈发送 / 停止入口;运行中会话的 delta 流式呈现,收口后与重放形状一致

#### Scenario: 整面板禁用与理由呈现

- **WHEN** 来源侧以 `disabled = true` 与理由文案渲染 `AgentInput`
- **THEN** 输入框与发送按钮同时禁用(空串禁用之外再叠加整面板禁用)、理由文案可见(用户不会「打了字发不出去」);`running` 的停止入口 SHALL 照常呈现(两标志语义正交,禁用只收发送面不收停止面)

### Requirement: 轮内状态行与空内容轮可见性

对话透镜 SHALL 让每一轮在时间线上**可见且带轮状态**，异常与主动停止 MUST NOT 表现为「界面只是不再增长」：

- **每轮恒一条 assistant 消息**：折叠与实时两路 SHALL 都按轮产出恰一条 assistant 消息（id `turn-<turnId>`），空内容轮不例外；轮状态 SHALL 只从该消息的 `metadata.record.status` 读取（MUST NOT 从 `parts` 是否为空反推轮状态）。
- **状态行（`parts` 为空时）**：`status = failed` SHALL 呈现状态行——**友好错误标题 + 可展开的原始错误原文**（原文取自 `metadata.record.error` 与该轮 `api_error` 通知，缺失时回退通用标题）；`status = stopped` SHALL 呈现「已停止」行；`status = completed` 或轮行缺席 SHALL 不渲染。状态行 SHALL 带 `data-testid` 挂钩（供测试与来源侧定位），MUST NOT 依赖样式类名查询。
- **`parts` 非空时**：SHALL 只呈现正常气泡（思考 / 正文 / 工具卡），MUST NOT 额外叠加状态行；轮失败/停止的事实仍可由 `metadata.record` 与顶部提示读出。
- **顶部失败横幅边界**：会话级入口的失败横幅 SHALL 只表达「最新一轮**失败**」（`status = failed`），`stopped` MUST NOT 触发该横幅（用户主动停止不是运行失败）；两处提示并存不冲突（横幅 = 全局最新状态，状态行 = 该轮位置）。
- **过滤单一来源**：对话透镜的可见性判据 SHALL 只在一处维护（system 角色过滤 + 空内容轮按轮状态判定），MUST NOT 在其他组件重复实现第二份判据；该判据经 `src/lib/agent-virtual.ts` 的 `filterConversation` 单点实现，虚拟窗口的项数 SHALL 基于该过滤后列表（MUST NOT 把不渲染的空内容消息计入虚拟列表 `count`）。

#### Scenario: 失败轮空内容呈状态行

- **WHEN** 某轮以 `parts` 空收口，`metadata.record.status = failed`、`error` 携 provider 错误原文
- **THEN** 该轮在时间线该位置呈现状态行：友好标题可见，展开后原始错误原文可见；消息 id 为 `turn-<turnId>`（不参与虚拟列表项数与吸附层几何的错误计数）

#### Scenario: 停止轮空内容呈已停止行

- **WHEN** 用户在运行中点击停止，该轮以 `parts` 空收口、`metadata.record.status = stopped`
- **THEN** 该轮位置呈现「已停止」行（不再是「界面只是不再增长」）；顶部失败横幅 MUST NOT 出现（`stopped` 不算失败）

#### Scenario: 空内容 completed 轮不进时间线

- **WHEN** 某轮 `parts` 空且 `metadata.record.status = completed`（或该轮无轮行）
- **THEN** 对话透镜不渲染该轮（无空气泡、无状态行），虚拟列表 `count` 不计入

#### Scenario: 有内容的轮不叠加状态行

- **WHEN** 某轮 `parts` 非空且 `metadata.record.status = stopped`（停止前已产出部分回答）
- **THEN** 该轮只呈现正常气泡（思考 / 正文 / 工具卡按到达顺序），MUST NOT 追加「已停止」状态行；轮状态仍可从消息 metadata 读出

### Requirement: 超长 user 提问默认折叠

对话透镜的 user 气泡 SHALL 对**超长提问**默认折叠，避免长 prompt 独占一屏；短提问 MUST NOT 出现折叠入口：

- **判据**：折叠判定 SHALL 只基于 user 消息的正文文本（全部部件均为 `text` 时拼接成文；部件中含非 `text` 部件时 MUST NOT 折叠，部件原样呈现）。可见行数 SHALL 按「显式换行逐条计 + 单行按固定字符宽折算折行」估算，超过行预算（3 行）即折叠。
- **折叠态**：SHALL 呈按行预算截取的纯文本预览（`whitespace-pre-line`，超出部分以省略号收尾）+ 下方「展开」按钮；折叠态 MUST NOT 渲染 markdown 或部件。
- **展开态**：点开后 SHALL 回到与该气泡原始形态一致的部件渲染（与 assistant 共用同一套块渲染件），按钮文案改「收起」，再点回预览态。
- **气泡形态不变**：折叠不改变 user 气泡的容器（整宽 tinted 泡）与 `data-testid="chat-bubble"` / `data-role` 钩子；折叠入口 SHALL 带独立 `data-testid` 挂钩供测试与来源侧定位，MUST NOT 依赖样式类名查询。

#### Scenario: 短提问不折叠

- **WHEN** user 消息正文为一两行（未超行预算）
- **THEN** 气泡内直接呈正文，无折叠入口、无预览节点

#### Scenario: 超长提问折叠与展开

- **WHEN** user 消息正文超过行预算
- **THEN** 气泡内呈截断预览（尾部省略号）+ 「展开」；点开后呈全文与「收起」，再点回到预览态

#### Scenario: 非 text 部件的提问不折叠

- **WHEN** user 消息含 `text` 以外的部件
- **THEN** 不折叠，全部部件按原渲染件呈现

### Requirement: ai-sdk 依赖收口

ai-sdk(`ai` 包)SHALL 锚定 v7 并收口于基建层(适配层、transport、`use-agent-session`、`components/agent/`);业务视图层(`views/` 下页面与 hooks)MUST NOT 直接 import ai-sdk;Rust 层与 store 层 MUST NOT 出现 ai-sdk 概念(`UIMessage` 是纯前端状态模型,`AgentEvent` 信封「线格式 = 落库形态(密封) = DTO 镜像」三位一体随 desktop-agent-execution 能力演进——`MessageDelta` 仅存在于线格式与 DTO 镜像,不进落库形态)。`ChatTransport` / `UIMessageChunk` 等签名变动 SHALL 由适配层与组件族吸收(MUST NOT 波及视图层)。

#### Scenario: import 收口

- **WHEN** 检索 `ai` 包的 import 来源
- **THEN** 仅出现于基建层文件(`lib/agent-adapter.ts`、`lib/agent-transport.ts`、`hooks/use-agent-session.ts`、`components/agent/`),`views/` 与 Rust/store 无 ai-sdk import 或概念

#### Scenario: 信封演进同步

- **WHEN** 审查本变更的 Rust 侧 diff 与 `types/dto.ts`
- **THEN** `AgentEvent` 信封演进(增 `MessageDelta`、TurnDone 口径)经 export-bindings 再生成同步镜像(camelCase 一致),delta 变体不出现在任何落库形态,无 UIMessage 结构倒灌 Rust

### Requirement: web 搜索活动与引用来源呈现

前端 SHALL 把一次云端 `web_search` 呈现为**搜索卡 + 正文引用列表**，并保证「空内容轮按轮状态决定是否可见」：

- **承载部件**：搜索活动 SHALL 复用既有工具卡管线承载为 `tool-web_search` 部件（MUST NOT 新增 `AgentDataParts` 成员、MUST NOT 引入独立 `data-citations` 部件）——`planBlock` 产 `tool` 计划、`planToolPart` 产部件、`plansToChunks` 产 chunk、`BUILD_IN_TOOL_CARDS` 注册特化卡，四段均为既有机械管线（`plansToChunks` 与 `planToolPart` SHALL 零改动）。
- **搜索卡**：`WebSearchCard`（`packages/desktop/src/components/agent/agent-blocks.tsx`）标题行 SHALL 呈查询词（`input.query`，缺席时不显）——MUST NOT 出现「正在搜索…」态、MUST NOT 出现超时降级文案、MUST NOT 依赖流内状态切换；body SHALL 呈查询词、来源列表（`title ?? 主机名`，落定后）、错误码（`error` 非空时，与来源列表并存不互斥）。卡片根 SHALL 带 `data-testid="block-web-search"`，查询词行带 `data-testid="block-web-search-query"`，来源列表带 `data-testid="block-web-search-sources"`。判定为「未落定」还是「已落定」仅由 `output` 在场与否决定（无状态文案使两态视觉一致，用户不把「进行中」误读为「加载失败」）。
- **引用列表**：`CitationList` SHALL 挂在正文 text 部件**底部**，条目显示 `title ?? url`，**按 url 去重**；MUST NOT 做行内角标、MUST NOT 做原句高亮。数据来源 SHALL 为 text 部件的 `providerMetadata.agent.citations`（命名空间 `agent`——消费点在 provider 之外，MUST NOT 借用 `anthropic` 命名空间）。
- **外链**：`ExternalLink` SHALL 为**唯一外链调用点**（搜索卡来源与引用列表共用）：仅 `http:` / `https:` 目标的 url SHALL 渲染为可点击链接并经壳侧 opener 打开；其他 scheme（`javascript:`、`file:`、`mailto:` 等）MUST NOT 渲染为链接（降级为纯文本）。壳侧插件注册与 capabilities 权限面见 desktop-app-shell「外链打开插件与 capabilities 最小权限面」。
- **空内容轮可见性**：`AgentMessages` 的过滤判据（`src/lib/agent-virtual.ts` 的 `filterConversation`）SHALL 在既有「system 角色不进对话呈现」过滤之后，按**轮状态**判定空内容 assistant 消息（`parts` 为空）是否呈现——`metadata.record.status` 为 `failed` / `stopped` 时 SHALL 呈现轮内状态行（见「轮内状态行与空内容轮可见性」），为 `completed` 或轮行缺席时 MUST NOT 渲染空气泡；虚拟列表项数 SHALL 基于过滤后列表（不把不渲染的空内容消息计入虚拟列表 `count` 与吸附层几何）。MUST NOT 仅以 `message.parts.length > 0` 作为唯一判据（那会吞掉失败 / 停止轮）。
- **工具卡注册表**：`BUILD_IN_TOOL_CARDS` SHALL 注册内建工具特化卡——`AskUserQuestion`（静态卡）、`web_search`（`WebSearchCard`）、`bash` / `read` / `write`（终端 / 文件读写卡）；`Object.keys` 断言随注册表键面更新。未注册工具名 SHALL 回退默认对卡，且特化卡与默认对卡 SHALL 共用同一卡根钩子（`data-testid="block-tool-use"` + `data-tool-id`）与结果正文钩子（`data-testid="block-tool-result"`，展开后挂载），使消费方无需区分卡型即可定位。
- **两路一致**：实时路（流内）与重放路（库为准）SHALL 收敛到同一卡片形状——实时路 `pause_turn` 期间卡片只显查询词（`output` 未到），轮收口后由重放折叠补出来源列表；`pause_turn` 拆出的两段响应 SHALL 累积在**同一轮同一条 assistant 消息**内（MUST NOT 因拆段产生第二条气泡）；终态一致性由「收口以重放重建」保证（见「会话级统一渲染组件」），MUST NOT 为搜索另立一条收口路径。

> 2026-11-06 agent-stream-single-message 修订:空内容轮可见性判据由 `parts.length > 0` 改为按 `metadata.record.status`(空内容 `failed` / `stopped` 呈轮内状态行,`completed` 不渲染),并明确 `pause_turn` 两段响应累积于同一轮同一条 assistant 消息。搜索卡自身条款(标题行 = 查询词、`title ?? 主机名` 回退、`BUILD_IN_TOOL_CARDS` 注册面与 `block-web-search-query` / `block-web-search-sources` 钩子)按主基线现行口径保留——delta 中该段为较早副本,与本变更自述 `agent-blocks.tsx` / `tool-cards.tsx` 零改动一致,MUST NOT 据其回退。

#### Scenario: 搜索卡标题与两态

- **WHEN** 实时流先到达「仅 use 侧」的搜索块（`pause_turn` 进行中），随后轮收口重放补出配对结果
- **THEN** 两态下标题行均为该查询词；前态 body 只有查询词，后态 body 出现来源列表；全程无「正在搜索…」文案、无状态切换闪烁，且两态都在同一轮的同一条 assistant 消息内

#### Scenario: 来源列表与错误码

- **WHEN** 搜索块携 3 条来源（其中两条同 url）与 `error = "max_uses_exceeded"`
- **THEN** 卡片来源列表按 url 去重后呈 2 条（`title` 缺省时回退主机名），并呈现错误码文本（错误区与来源列表并存）

#### Scenario: 正文引用去重列表

- **WHEN** 一条 text 部件携 `providerMetadata.agent.citations`（含重复 url 与 `title` 缺省项）
- **THEN** 正文底部渲染去重后的引用列表（`title ?? url`），无行内角标、无原句高亮；`citations` 缺席或空数组时 MUST NOT 渲染列表、MUST NOT 渲染空容器

#### Scenario: 非 http 目标不渲为链接

- **WHEN** 搜索来源 url 为 `javascript:alert(1)`、`file:///etc/passwd` 或 `mailto:a@b.c`
- **THEN** 该条目以纯文本呈现（无 `<a>`、无点击打开），不触达 opener 插件

#### Scenario: 空内容 completed 轮不渲染

- **WHEN** 会话状态含一条 `parts` 为空、`metadata.record.status = completed` 的 assistant 消息
- **THEN** 对话透镜不渲染该条（无空白气泡），虚拟列表 `count` 不计入它；其余消息计数与呈现不变

### Requirement: 对话透镜虚拟窗口与用户消息顶部吸附

对话透镜 `AgentMessages` SHALL 以**虚拟窗口**承载任意规模的会话（MUST NOT 以分页窗口限制进入 DOM 的消息集合），并 SHALL 让用户提问以 sticky 吸附层常驻：

- **全量虚拟窗口**：过滤后消息列表 SHALL 整体作为虚拟器的 `count`（`getItemKey` = 消息 `id`，稳定不随滚动变化；`estimateSize` 按角色分型估值）；内容容器高度 SHALL 为虚拟器 `getTotalSize()`，虚拟项以绝对定位 + `translateY(item.start)` 落位，行距与内容容器内边距 SHALL 经虚拟器的 `gap` / `paddingStart` / `paddingEnd` 表达（MUST NOT 依赖 flex `gap` 或内容容器的 CSS padding——sticky 的被夹边界与绝对定位的包含块均为**包含块的 padding box**，而 `border-box` 下容器 `height = getTotalSize()` 会含 padding，两者叠加使末项越出容器、吸附层跨度对不上账目）。滚动条比例 SHALL 由全量账目导出（一次装载、无「加载更早」进度状态；`remaining` / `history-load-hint` / `history-load-complete` / 触顶 `IntersectionObserver` MUST NOT 存在）。
- **用户提问不进虚拟层**：`role = 'user'` 的索引 SHALL 在虚拟层渲染 `null`（不产节点），但其高度账目 SHALL 照常参与 `count` / `estimateSize` / `getTotalSize`；其真实高度 SHALL 由吸附层经 `ResizeObserver` 实测后以 `resizeItem(index, height)` 回填。实测尺寸 ≤ 0 时 SHALL 回落该索引的 `estimateSize`（虚拟项 MUST NOT 折叠为零高）。
- **sticky 吸附与顶走**：全部 `role = 'user'` 消息 SHALL 在吸附层无条件常驻（MUST NOT 按可见范围过滤其挂载；会话中用户提问量本就有界）。第 i 条用户消息的吸附容器 SHALL 为 `position: absolute` + **`top` 定位**（MUST NOT 使用 `transform`——transform 会创建新的包含块，使 sticky 的夹取基准被替换、气泡吸住后永不顶走），其高度 SHALL 为**该轮跨度** `span = 下一用户消息的 start − 本条 start`（末轮 = `getTotalSize() − start`）；容器内层气泡容器 SHALL 为 `position: sticky; top: 0`。于是「最近一条已完全滚出顶部的用户消息」停在视口顶端，「下一轮提问到达包含块底边」即把上一条顶走（**零缓冲**，不做 gutter）。
- **吸附层祖先链约束**：吸附层与其祖先链 MUST NOT 出现 `overflow: hidden` / `overflow: auto` 一类裁剪（会另立滚动容器，吸附基准由视口变成本层 ⇒ 完全不吸附）；滚动视口（`data-slot="message-scroller-viewport"`）的 `contain-content`（`contain: layout paint`）SHALL 保留（不创建包含块，不影响 sticky）。
- **零装饰**：吸附气泡 SHALL 与普通气泡**同一渲染件**（同一 tinted 变体），只可多一层不透明 `bg-card` 底衬以防圆角边缘透出下层滚动正文；MUST NOT 加顶部渐隐 / 阴影 / 「上方还有内容」提示层。视口既有的 `scroll-fade-b`（**底边**渐隐，`animation-timeline: scroll(self y)` 要求承载元素自身是滚动容器）SHALL 原样留在视口元素上，MUST NOT 迁到内容容器或吸附层。
- **吸附零滚动状态**：吸附判定 SHALL 为纯 CSS 行为——MUST NOT 为吸附引入 `scroll` 事件监听、MUST NOT 维护「当前吸附项」state、MUST NOT 依赖虚拟器可见范围（`range` / `onChange`）。滚回会话顶部时自然无气泡吸顶；无 `role = 'user'` 消息时不渲吸附层。
- **钩子延续**：`data-slot="message-scroller"` / `-viewport` / `-content` / `-item` / `-button` SHALL 保留（既有断言锚点），吸附层容器 SHALL 带独立钩子 `data-slot="message-scroller-sticky"`；气泡 SHALL 继续以 `data-testid="chat-bubble"` + `data-role` 暴露角色（供按角色分查的断言）；`data-autoscrolling`（滚动条透明）与 `data-pending-scroll`（首帧隐形防跳）SHALL 由滚动实例状态与「首次测量完成」标志驱动，样式语义不变；虚拟项 SHALL 保留 `[content-visibility:auto]` + `[contain-intrinsic-size]`（单条超长 assistant 仍为单虚拟项，两级机制叠加）。
- **贴底跟随**：新消息追加时的贴底跟随 SHALL 由虚拟器的端锚定能力（`anchorTo: 'end'` + `followOnAppend` + `scrollEndThreshold`）承担；用户上滑后 MUST NOT 被强制拽回底部，回到底部后恢复跟随。
- **首帧与换会话贴底**：对话透镜**首次挂载即非空、或由空变非空**（`0 → N`，历史重放 / 新会话首轮）与**切换会话**（同一实例的消息集合换成另一会话）两种情形，视口 SHALL 都落在内容末端（贴底）——端锚定只在选项变更且滚动元素已挂载时生效、首帧 `initialOffset` 缺省 0，故这两种情形的贴底 MUST NOT 依赖 `anchorTo: 'end'`，MUST NOT 依赖一次性「首次贴底」闩（那会把换会话漏掉）。复位 SHALL 由滚动承载 shim 依**复位键**发起：复位键由应用层经 `src/lib/agent-virtual.ts` 的 `conversationIdentity` 从消息集合导出（首条携 `metadata.record.sessionId` 的消息的会话 id，全体缺席时为 `null` = 未知），作为 shim 的可选入参 `resetKey` 传入。复位条件 SHALL 恰为二者之一：①消息集合首次非空或由空变非空；②**两端所给 `resetKey` 均非 `null` 且不相等**。命中即在 layout 阶段（浏览器绘制前）调一次 `scrollToIndex(count − 1, { align: 'end' })`，并由 `data-pending-scroll` 遮住这一跳。复位键的一侧为 `null`（首轮 `record` 落库导致 `null → 已知`，或收口重放导致 `已知 → null`）MUST NOT 触发复位；同一会话内追加（键不变）同样 MUST NOT 触发复位。该机制 MUST NOT 要求上游新增入参（`AgentMessagesProps` 形状不变），MUST NOT 引出 `use-agent-session*` / `agent-session-view` / `src/views/**` 任一文件的改动——复位机制 SHALL 全部落在本变更可改面（`agent-messages.tsx` 与 `components/ui/message-scroller.tsx`）内。

#### Scenario: 千条会话只渲可见切片

- **WHEN** 以 1000 条过滤后消息（含若干空内容 `completed` 轮）挂载对话透镜
- **THEN** DOM 中 assistant 气泡数远小于 1000（仅可见窗口 + overscan），内容容器高度为全量 `getTotalSize()`，零「加载更早」提示、零触顶观察器；空内容 `completed` 轮不参与项数

#### Scenario: 用户消息常驻且虚拟层零节点

- **WHEN** 会话含 12 条用户消息与若干 assistant 消息
- **THEN** 12 条用户气泡全部在吸附层 `data-slot="message-scroller-sticky"` 内（各恰一处），虚拟层虚拟项集合内零 `data-role="user"` 节点；高度账目中用户索引仍占位（`getTotalSize()` 含其高度）

#### Scenario: 完全出顶即吸顶

- **WHEN** 滚动使某轮用户提问整体离开视口顶部（其布局位置在视口之上）
- **THEN** 该气泡停在视口顶端（布局 `top` 不变、DOM 只一份），且吸附过程零 `scroll` 监听、零吸附态 state

#### Scenario: 下一轮顶走上一条

- **WHEN** 继续下滚，使下一轮用户提问容器到达吸附容器的底边
- **THEN** 上一轮吸附气泡被推出视口顶端，下一轮气泡接续吸顶（零缓冲、无重叠、无闪烁）；滚回会话顶部时无任何气泡吸顶

#### Scenario: 高度回填与账目一致

- **WHEN** 吸附层各气泡经 `ResizeObserver` 实测完成（吸顶状态下的实测仍反映布局盒高度）
- **THEN** `resizeItem` 把实测高度回填至对应用户索引，`getTotalSize()` 与实测总和一致，虚拟项之间无重叠、末项底边与内容容器底边对齐

#### Scenario: 零测量回落不塌陷

- **WHEN** 某项的实测尺寸为 0（jsdom 无布局 / 图片未加载的瞬时态）
- **THEN** 该项高度回落为其 `estimateSize` 估值，后续项的 `start` 仍连续、无零高项，不抛错

#### Scenario: 视觉一致且视口样式零改动

- **WHEN** 检查吸附气泡与普通气泡、以及滚动视口
- **THEN** 吸附气泡与普通用户气泡同渲染件（仅多一层不透明底衬），零渐变 / 零阴影 / 零提示层；视口仍携带 `scroll-fade-b` / `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content` 与 `data-slot="message-scroller-viewport"`、`aria-label="对话消息"`；吸附层容器样式不含 `transform`

#### Scenario: 上滑不被拽回底部

- **WHEN** 用户上滑离开底部后，会话继续追加新消息
- **THEN** 视口保持用户当前滚动位置（不被拽回底部）；用户滚回底部后恢复贴底跟随，新消息到达即跟到最新

#### Scenario: 首帧挂载即贴底

- **WHEN** 对话透镜挂载时消息集合为 N 条（N > 0，历史重放或新建会话首轮；含从空集合变为非空的情形）
- **THEN** 视口在浏览器绘制前落到内容末端（用户看到的是最新一轮而非会话开头），`data-pending-scroll` 遮住这一跳（无可见跳动）；该跳由滚动承载 shim 自行发起，MUST NOT 依赖 `anchorTo: 'end'` 的选项变更时机

#### Scenario: 切换会话复位贴底

- **WHEN** 同一对话透镜实例的消息集合从会话 A 换成会话 B（两侧复位键均为已知且不相等，中间不经过空集合，条数可相同）
- **THEN** 视口重新落到会话 B 的内容末端（MUST NOT 停留在 A 遗留的滚动位置）；判据为「两侧复位键均非 `null` 且不相等」——MUST NOT 要求上游新增入参（`AgentMessagesProps` 形状不变），MUST NOT 引出 `use-agent-session*` / `agent-session-view` / `src/views/**` 任一文件的改动；同一会话内追加消息（复位键不变）不触发该复位

#### Scenario: 首轮 record 落库不拽回上滑用户

- **WHEN** 新会话首轮流式期间用户已上滑，该轮收口时 `metadata.record` 首次落库（复位键由 `null` 变为会话 id，消息条数不变）
- **THEN** 视口 MUST NOT 被复位到末端（复位条件只认「首次非空 / 由空变非空」与「两侧键均已知且不等」）；用户当前位置保持，回到底部后恢复贴底跟随

## Module Contract

### Component: `packages/desktop/src/components/agent/agent-session-view.tsx`（新增）

| 组件 | Props | 描述 |
|------|-------|------|
| `AgentSessionView` | `root: string \| null`；`sessionId: string \| null`；`refreshKey?: number`；`composer?: React.ReactNode`（或 render prop）；`data-testid?` | 会话级唯一渲染入口：经 `sessionDetail` + `agentSessionTranscript` 装配会话，按会话状态（running 轮行）决定是否挂 `agent_session_watch`；单流在挂；轮收口以重放收敛；composer 插槽可选（只读消费点不接） |

### Function: `packages/desktop/src/hooks/use-agent-session.ts`（新增，收编 `use-agent-chat.ts`）

| 函数 | 签名 | 描述 |
|------|------|------|
| `useAgentSession` | `(params: { root: string \| null; sessionId: string \| null; source: string; refreshKey?: number }) => UseAgentSessionState` | 重放装载（密封转录 → `foldSessionMessages(events, turns)`）；状态驱动挂流（running 且无在挂流才订阅，`since_seq` = 镜像最大 seq）；单流在挂；收口以重放收敛（失败回落镜像折叠）；断流重订；`sendMessage` / `stop` 动作面；返回 `messages` / `events` / `session` / `chain` / `loading` / `running`（库态驱动）/ `error` |

### Function: `packages/desktop/src/hooks/use-agent-session.ts`（会话状态侧动作面）

| 函数 / 项 | 签名 / 变化 | 描述 |
|------|------|------|
| `buildMessages` | **删除** | 并入 `foldSessionMessages`；`TurnDone` 计数配轮行的错位实现退役 |
| `sendMessage` | 本地推提问消息（id `turn-<turnId>-user`，事件到达时归一） | 发起流路径的提问气泡来源；body 会话参数面零改动 |
| 订阅流提问 | 收到该轮 `role = user` 密封事件时以 `turn-<turnId>-user` 追加 | 旁观方无本地副本，跳过即丢提问气泡 |
| `UseAgentSessionState` | 形状不变（`messages` / `events` / `session` / `chain` / `loading` / `running` / `error`） | `chain` 仍为轮行镜像（顶部横幅与 `running` 派生读它）；`events` 仍供 `AgentRawStream` 逐事件回放 |

### 退役: `packages/desktop/src/hooks/use-agent-chat.ts`（删）

| 项 | 变化 | 描述 |
|------|------|------|
| `useAgentChat` | 收编进 `useAgentSession` | 单一会话基建；调用点（`views/agent`、`views/explores`）全量切换，MUST NOT 保留双 hook |

### API: `packages/desktop/src/lib/agent-transport.ts`

| 成员 | 签名 | 描述 |
|------|------|------|
| `sendMessages` | `(options) => Promise<ReadableStream<AgentUIMessageChunk>>` | 发送期发起流（`agent_start` + `Channel<AgentRunMessage>`），零空洞；信封 → chunk 经共享簿记单点 |
| `reconnectToStream` | `(options: { chatId, abortSignal?, body?: { root, sessionId, sinceSeq } }) => Promise<ReadableStream<AgentUIMessageChunk> \| null>` | 旁观 / 重挂期订阅流（`agent_session_watch`）；body 参数由 `chat.resumeStream({ body })` 透传，transport 保持无状态；无运行轮时返回 `null` |
| step 分段簿记 | 私有（`start-step` 触发条件：本轮已密封过响应后首个 delta） | 每轮一次 `start`（`turn-<turnId>`）；密封 `Message` → `reset-step` + 追加整块；工具结果就地补全不开新 step；`role = user` 密封事件零 chunk |
| part 簿记 | 私有（`kind → 首个 delta 的 seq`，作用域 = 当前 step） | 取代按 `parentToolUseId` 的跨消息开件簿记；新 step 开出时重武装 |
| `Record` 收口 | 私有（`finish` 携 `messageMetadata: { record }`） | 该轮消息不存在时先 `start` 建消息再 `finish`；删 `enqueueRecord` 独立消息组与 `data-run-record` 部件 |

### Function: `packages/desktop/src/lib/agent-adapter.ts`

| 函数 | 签名 | 描述 |
|------|------|------|
| `AgentToolOutput` | `{ content: string; isError: boolean; search?: { results: WebSearchSource[]; error: string | null } }` | 加位可选（既有消费面零改）；搜索类服务端工具的结构化载荷走 `search` |
| `BlockPlan` | `text` 变体增 `citations?: AgentCitation[]`；`tool` 变体承载搜索（`toolName: 'web_search'`、`input: { query } \| null`） | 三变体不扩面（MUST NOT 新增第四变体） |
| `PairedResults` | `Map<string, PairedOutcome>`（工具结果块 / 搜索来源载荷的联合） | 两视野同形态；块种类自洽 |
| `PairingScope.global` / `local` | 语义扩展（签名不变） | 并入 `webSearch` 块配对：use 侧（`rawUse != null`）为索引源、result 侧（`rawUse == null`）为配对产物；跨消息合并由 `global` 承担（`pause_turn` 拆分场景） |
| `planBlock` | 新增 `webSearch` 分支 | use 侧 → `tool` 计划（配对命中携 `output`，未命中 `output: null`）；result 侧且已配对 → `null`；result 侧且无配对 use → `tool` 计划（`input: null` + `output` 来源载荷） |
| `plansToParts` / `plansToChunks` | 扩 metadata 位（机械映射不变） | text 计划挂 `providerMetadata: { agent: { citations } }`（空数组不挂）；`textChunks` 的 `text-start` 带同一 metadata；tool 分支零改 |
| `foldSessionMessages` | `(events: AgentEvent[], turns: TurnSummary[]) => AgentUIMessage[]` | 重放 / 收口折叠唯一入口（`PairingScope.global`）：按 `turn_id` 分组（旧转录按 `RunStarted` 顺序），每轮提问 + 恰一条 assistant（id `turn-<turnId>`）+ 逐事件 aux system 消息，轮行挂 `metadata.record` |
| `deltaPartId` | `(seq: number, part: 'text' \| 'thinking') => string` | part id 由该 step 内首个同类 delta 的 `seq` 派生（`t-<seq>` / `r-<seq>`）；同一 part 后续 delta 复用（常量语义形态退役） |
| `AgentMessageMetadata` | `{ seq: number \| null; parentToolUseId: string \| null; record?: TurnSummary \| null }` | 消息元数据：保真 + 回合统计载体（`record` 取代 `data-run-record` 部件） |
| `AgentDataParts` | `run-started` / `system-notice` / `run-result` / `raw` | data 部件词汇收窄（`run-record` 成员退役）；`turnDone → data-run-result` 维持 |
| 事件载荷类型 | 由生成绑定派生（`Extract<AgentEvent, { kind: 'runStarted' }>` 等） | 8 个手写镜像退役，信封加变体时编译报错 |
| `eventToChunk` | `(event: AgentEvent, currentMessageToolCalls?: ReadonlySet<string>) => AgentUIMessageChunk[]` | 只产**块 chunk**（delta / 密封块 / 工具补全）；`role = user` 事件零 chunk；事件头与 step 帧（`start` / `start-step` / `reset-step` / `finish`）归 transport；搜索无实时 delta（`AgentDelta` 词汇不扩） |
| 退役 | `eventsToUIMessages` / `runRecordToUIMessage` / `provisionalMessageId` | 由 `foldSessionMessages` / `metadata.record` / `turn-<turnId>` 取代（`knip` 零未用导出） |

### Function: `packages/desktop/src/lib/agent-virtual.ts`（新增）

| 函数 | 签名 | 描述 |
|------|------|------|
| `hasVisibleParts` | `(message: AgentUIMessage) => boolean` | 可见部件判据（`text` / `reasoning` / `tool-<name>` 计入，`data-*` 与 `step-start` 不计）：自 `agent-messages.tsx` 迁出，两个生产消费点——`filterConversation` 与 `ChatBubbleBase` 的空部件状态行判据（避免判据在两处各写一份） |
| `filterConversation` | `(all: AgentUIMessage[]) => AgentUIMessage[]` | 对话透镜可见性判据单点（自原分页层抽出，语义零改）：剔除 `role === 'system'`；空内容 assistant 消息仅 `metadata.record.status` 为 `failed` / `stopped` 时保留，`completed` 或轮行缺席时剔除 |
| `userTurnSpans` | `(messages: AgentUIMessage[], measurements: { start: number }[], totalSize: number) => { index: number; start: number; span: number }[]` | 吸附层几何：仅为 `role === 'user'` 的索引产条；`span` = 下一条用户消息 `start` − 本条 `start`，末轮 = `totalSize − start`；无用户消息时返回空数组（不渲吸附层） |
| `conversationIdentity` | `(messages: AgentUIMessage[]) => string \| null` | 会话身份（贴底复位键）：取首条携 `metadata.record.sessionId` 的消息的会话 id；全体缺席（流式首轮未收口 / 尚无 `record`）返回 `null` = 未知。同一会话的追加 / 流式增长不改变返回值，换会话必变；`null` 不参与换会话判定（两侧均非 `null` 才比较），故首轮收口（`null → 已知`）不会把上滑用户拽回底部 |

### Component: `packages/desktop/src/components/ui/message-scroller.tsx`（改）

| 组件 / 导出 | Props / 变化 | 描述 |
|------|------|------|
| `MessageScrollerProvider` | 新：`count` / `estimateSize` / `getItemKey` / `renderItem` / `renderSticky` / `resetKey`（+ 可选 `overscan` / `gap` / `scrollEndThreshold`） | 持有 `useVirtualizer` 实例并向子层供 context；取代 `@shadcn/react` 的模式机与 `autoScroll` / `defaultScrollPosition` / `scrollPreviousItemPeek` 入参；首帧贴底与换会话复位由本件内的 layout 阶段 effect 发起（条件：消息集合首次非空 / 由空变非空，或两侧 `resetKey` 均非 `null` 且不等，见「首帧与换会话贴底」），复位能力不外溢为上游入参；`resetKey` 只影响复位时机，MUST NOT 参与虚拟器选项的身份判定 |
| `MessageScrollerViewport` | 结构不变（`data-slot` / `aria-label` / 类名全保留） | 滚动容器：`scroll-fade-b`（底边渐隐，必须留在本元素）/ `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content`；`data-autoscrolling` 由虚拟器 `isScrolling` 驱动、`data-pending-scroll` 由首测完成标志驱动 |
| `MessageScrollerContent` | 高度改由虚拟器 `getTotalSize()` 提供 | 绝对定位的项容器（`position: relative`）；行距与内边距由虚拟器的 `gap` / `paddingStart` / `paddingEnd` 承担（`flex gap-6` 与调用点传入的 `p-3` 一并退役——`border-box` 下 padding 计入 `height`，会使末项越出容器且吸附层跨度与 `getTotalSize()` 账目不符） |
| `MessageScrollerItem` | 保留 `data-slot` 与 `[content-visibility:auto]` / `[contain-intrinsic-size]`；定位改 `position:absolute` + `translateY(start)`；入参 `messageId` / `scrollAnchor` 退役（改由虚拟器索引驱动）；新增 `measureElement` 零测量回落估值 | 可见窗口切片项；零测量回落使 jsdom 与瞬时态不塌陷 |
| `MessageScrollerSticky`（新） | `index` / `start` / `span` / `children` | 吸附层容器：`position: absolute` + **`top`**（禁 `transform`）+ `data-slot="message-scroller-sticky"`；高度 = `span` |
| `MessageScrollerButton` | `data-active` 判据改 `getDistanceFromEnd() > 阈值` | 到底按钮：可见性与贴底状态互斥（语义等价于原「可滚动且未到底」判定） |
| 首帧/换会话复位 | 新增私有 layout effect：条件 =「首次非空 / 由空变非空」或「两侧 `resetKey` 均非 `null` 且不等」；命中即 `scrollToIndex(count − 1, { align: 'end' })` | 复位由 shim 自理，不外溢为上游入参（见「首帧与换会话贴底」）；`resetKey` 只作复位时机判据，MUST NOT 进入虚拟器选项 |
| 退役 | `useMessageScroller` / `useMessageScrollerScrollable` / `useMessageScrollerVisibility` / `scrollToMessage` 再导出、`@shadcn/react/message-scroller` primitives 依赖 | 三项 hook 与 `scrollToMessage` 在 `src/` 零外部消费者；跳转能力由 `scrollToIndex(index)` 承担（`messageId → index` 查表） |
| 退役（依赖面） | `packages/desktop/package.json` 的 `@shadcn/react` 依赖 + `packages/desktop/src/components/ui/questionnaire.tsx`（零消费者残件） | primitives 退役后该包在仓内再无**可达**消费者（`questionnaire.tsx` 全历史零 import，`knip.ignore` 只抑制报告）；按 desktop-app-shell「未用导出 SHALL 删减」一并剪除，MUST NOT 以 `ignoreDependencies` / `ignore` 条目豁免；二期接线问卷组件经 shadcn CLI 重新生成 |

### Component: `packages/desktop/src/components/agent/agent-messages.tsx`（改）

| 组件 / 项 | 变化 | 描述 |
|------|------|------|
| `useHistoryWindow` | **退役** | 过滤判据抽出为 `filterConversation`（`src/lib/agent-virtual.ts`）；分页窗口与 `remaining` 语义退役 |
| `useTopReachObserver` / `HistoryLoadStatus` / `MESSAGE_PAGE_SIZE` / `HISTORY_PRELOAD_MARGIN_PX` / `topSentinelRef` | **删除** | 触顶 `IntersectionObserver` 与 `history-load-hint` / `history-load-complete` 分支整体退役 |
| 虚拟窗口接线 | 新增 | `count` = `filterConversation(messages).length`；assistant 索引走 `renderItem`，user 索引渲 `null`；`estimateSize` 按角色分型（user 72 / assistant 240 起）；`resetKey` = `conversationIdentity(过滤后列表)`（贴底复位键，见「首帧与换会话贴底」） |
| 吸附层 | 新增 | `renderSticky` 渲染 `userTurnSpans` 的全部条目（每条 user 消息一个 `MessageScrollerSticky` + 内层 `position:sticky; top:0` 的 `ChatBubble`）；`ResizeObserver` 实测 → `resizeItem` |
| `ChatBubble` / `PartView` / `TurnStatusLine` / `ToolPartView` / `citationsOfPart` | 逐字不变 | 块渲染、轮内状态行、工具卡分派和 `memo` 语义零改动（吸附气泡复用同一 `ChatBubble`） |
| 三态保留 | 逐字不变 | `agent-messages-loading`（`loading` 且过滤后为空）/ `conversation-empty` / `agent-messages-running` 判据与挂钩不变 |

### Component: `packages/desktop/src/components/agent/`（呈现层）

| 组件 / 模块 | 变化 | 描述 |
|------|------|----------|
| `agent-blocks.tsx` | 新增 `WebSearchCard` / `CitationList` / `ExternalLink` | 搜索卡标题行为查询词（`data-testid="block-web-search"`）；引用底部去重列表；外链唯一调用点（仅 http(s)） |
| `tool-cards.tsx` | `BUILD_IN_TOOL_CARDS` 注册 `web_search` / `bash` / `read` / `write` | 注册表可插拔位扩展；未注册回退默认对卡；卡根与结果正文钩子与默认对卡同形 |
| `agent-messages.tsx` | `useHistoryWindow` 过滤判据改「system 过滤 + 按 `metadata.record.status` 判空内容轮」（原 `parts.length > 0` 判据退役） | 空内容 `failed` / `stopped` 轮 SHALL 可见（轮内状态行），空内容 `completed` 轮不渲染；虚拟列表项数基于过滤后列表（见上「`agent-messages.tsx`（改）」表）；状态行带 `data-testid` 挂钩（`failed` → 友好标题 + 可展开原文；`stopped` → 「已停止」） |

### 零改动面

| 文件 | 说明 |
|------|------|
| `packages/desktop/src/lib/agent-transport.ts` | 搜索面无新事件类型需转 chunk（本变更的 step 分段簿记与 `Record` 收口见「API」表） |
| `packages/desktop/src/hooks/use-agent-session.ts` | 收口语义（流结束按库内转录重建）已天然覆盖搜索卡两态；会话基建零改动（`messages` 形状、`loading` / `running` 语义不变） |
| `AgentDataParts` | MUST NOT 新增成员（无 `data-citations` / `data-web-search`） |
| `packages/desktop/src/components/agent/agent-error.ts` | `latestAgentFailure` 维持「`stopped` 不算失败」判据，MUST NOT 改造成失败横幅（`stopped` 由轮内状态行承接） |
| `packages/desktop/src/components/agent/agent-blocks.tsx` / `agent-input.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx` | 块渲染件、composer、工具卡注册表、面板骨架、会话级入口的入参与插槽形状零改动 |
| `packages/desktop/src/lib/agent-adapter.ts` / `agent-transport.ts` / `parse.ts` | 过滤判据**语义**搬去纯函数，适配层与传输层零改动（`foldSessionMessages` / `eventToChunk` / `deltaPartId` 签名不变） |
| `packages/desktop/src/components/ui/` 其余文件 | `bubble.tsx` / `message.tsx` / `trace.tsx` 等零改动（吸附气泡复用同一组件的 tinted 变体）；该目录本变更只改 `message-scroller.tsx`、只删 `questionnaire.tsx`（依赖剪枝的连带删除，见「`message-scroller.tsx`（改）」表「退役（依赖面）」行） |
| `.dev-team/architecture/**` | 架构模型零改动（纯函数落 `src/lib/**` 属封闭排除面；`components/agent/` 文件集合与 `components/ui/` 收敛口径不变，元素 / 关系 / 计数复述零漂移） |
| `packages/desktop/src/views/**` | 消费点接线零改动（仍以 `useAgentSession` / `AgentSessionView` 为唯一入口；`AgentRawStream` 继续消费 `state.events`） |
| `packages/desktop/src-tauri/**` / `packages/desktop/src/types/generated/**` | 后端与生成物零改动（信封已有 `turnId`；`Record` 信封与 `agent_session_watch` 语义不变） |

### 既有模块（更新）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/components/agent/` | 展示组件族 | 会话级入口 `AgentSessionView` + 对话透镜 `AgentMessages`(辅助载体 system 消息过滤 + 一轮一条 assistant 气泡 + 空内容轮按轮状态判定)+ `AgentInput` + 工具卡注册表;共享块渲染件;`message-scroller` 滚动承载 |
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | 探索链消费面 | 会话域参数(source/sessionId 会话引用)语义等价切换,行为不回退;`sourceRef` 反查退役 |
| Rust 会话域命令面(详见 desktop-agent-execution) | stop 与提前 resolve | 终止命令按运行中的会话活动轮寻址;发起命令提前 resolve;`stopped` 收敛;终态经 Channel 以 `Record` 信封流出;会话订阅命令 `agent_session_watch` 供旁观 / 重挂期挂流 |
| `packages/desktop/src/lib/agent-transport.ts` | 会话参数面透传 | `AgentStartSessionParams` 增 `sourceRef` / `cwd`(派生自生成绑定,无手写镜像);`readSessionParams` 增读两字段(`readNullableString` 复用,非法即 `Err`);`invokeStart` 改**具名解构**调用 `agentStart`(位置索引魔法 1/2/3/4/5/7 退役,尾部增位不再错位) |
| `packages/desktop/src/hooks/use-agent-session.ts` | 参数面与发送 body | `UseAgentSessionParams` 增 `sourceRef?: string \| null` / `cwd?: string \| null`;`sendMessage` body 透传两字段(原 `sourceRef: null` 常量退役,未提供仍为 null) |
| `packages/desktop/src/components/agent/agent-input.tsx` | 禁用态扩展 | 增 `disabled?: boolean` / `disabledReason?: string`(textarea 与发送同时禁用 + 理由文案呈现);`running` 只决定停止入口呈现(发送入口不禁,重复发送由会话层守卫);档位改用 `StandardSelect`;输入区改用 `Textarea` + `Ctrl/Cmd+Enter` 快捷发送 |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | 注入形态入参 | 会话状态面仍由来源侧建 hook 注入;如需向 composer 插槽传禁用态,仅在 `AgentSessionActions` 上增位(additive),MUST NOT 在组件内自判业务禁发 |
| `packages/desktop/src/lib/agent-transport.test.ts` / `hooks/use-agent-session.test.ts` | 透传直测 | 两字段按序到达 invoke;非法值拒绝;未提供时缺省 null 且既有行为不变 |

