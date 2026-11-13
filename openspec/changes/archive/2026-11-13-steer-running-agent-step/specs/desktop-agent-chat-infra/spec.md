# desktop-agent-chat-infra Specification (Delta)

## MODIFIED Requirements

### Requirement: 事件适配层(纯函数,保真)

基建 SHALL 提供纯函数适配层(置于 `packages/desktop/src/lib/agent-adapter.ts`),分三层:入口层、语义层(IR)、发射层:

- `foldSessionMessages(events: AgentEvent[], turns: TurnSummary[]): UIMessage[]` — 重放与收口折叠(**按轮分组**;仅消费**密封事件**:会话转录中不存在 delta,delta 不进重放路径)。分组键 SHALL 为信封 `AgentEvent.turn_id`(同一 `turn_id` 归同一轮);`turn_id` 为 `null` 的旧转录 SHALL 按 `RunStarted` 出现顺序分组(组界 = 下一枚 `RunStarted` 或数组末尾)。每轮 SHALL 产出:提问消息(**组内首条** `role = user` 密封事件,id `turn-<turnId>-user`;缺席即无提问消息)+ **补充说明消息**(组内其余 `role = user` 密封事件**各一条**,按 `seq` 升序排在提问之后、assistant 之前,id `evt-<seq>`;无则零条)+ **恰一条** assistant 消息(id `turn-<turnId>`,parts 可为空)+ 逐事件 aux system 消息,且 `metadata.record` 为该轮轮行(按 `turnId` 匹配;轮行缺席为 `null`)。`TurnDone` MUST NOT 用作轮边界或轮行计数依据,仅在该轮轮行缺席时兜底 status(`isError ? failed : completed`)。MUST NOT 把组内第二条及之后的 `role = user` 事件当提问（丢弃即补充说明在重放里不可见,复用提问 id 即两条消息撞 id）
- `eventToChunk(event: AgentEvent, ...): UIMessageChunk[]` — 实时增量的**块 chunk 单点**(delta 路 + 密封块路);**事件头与 step 帧(`start` / `start-step` / `reset-step` / `finish`)SHALL 归 transport**,本函数 MUST NOT 产。`role = user` 的密封事件 SHALL 零 chunk(提问由会话状态侧承载)。可选参为**流内跨消息配对视野**(transport 流内簿记:本轮已发出的工具调用 id 集),命中即就地补全
- **语义层 IR**: `BlockPlan` 为块语义的唯一表达(text / reasoning / tool 三变体;tool 的 `output: null` 即未配对),`planBlock(block, paired): BlockPlan | null` 为**唯一语义点**(五 block 变体 + 无主 result 合成在此收口;`null` = 已配对 result 不重复呈现);孤儿 result 与正常 tool 合并为同一 `tool` 变体(占位工具名 + `input: null`),MUST NOT 在两路各写一遍孤儿判定。`text` 变体 SHALL 携 `citations?: AgentCitation[]`(缺席 = 无引用),`tool` 变体 SHALL 为**唯一工具类承载位**——客户端工具与云端 `web_search` 服务端工具共用该变体(`toolName` 区分,MUST NOT 为搜索新增第四变体)
- **视野层**: `PairingScope` 隔离配对视野差异——折叠路 `global`(跨消息预扫全部 toolUse 的配对 result),实时路 `local`(仅当前 message 内配对);两视野 SHALL 产出同一类型 `Map<toolCallId, PairedOutcome>`(配对载体为工具结果块 **或搜索来源载荷**的联合,与块种类同构),差异 MUST NOT 渗入语义层与发射层。实时路另有**流内跨消息配对视野**(`eventToChunk` 可选参,transport 按「本轮最近开件 step 的消息」簿记并在新 step 开出时重武装):命中的 toolResult 块 SHALL 就地补全既有消息里的工具部件——只投 `tool-output-available`,MUST NOT 另起消息 / 重投 input / 开新 step;全部块均命中时该事件 MUST NOT 产任何块 chunk。**搜索块配对 SHALL 以块 `id` 为键沿用同一视野机制**:use 侧块(`rawUse != null`)与 result 侧块(`rawUse == null`)分处两条密封 message 时由 `global` 视野跨消息合并(`pause_turn` 拆分场景),`local` 视野下无配对 result 的 use 仍渲染(只显查询词)
- **发射层**: `plansToParts` / `plansToChunks` 为纯机械映射(无判定、无视野),两路的块呈现 SHALL 由同一 `BlockPlan[]` 驱动;两者均只产块相关产物(部件 / chunk),**事件头与 step 帧由 `eventToChunk` 之外的 transport 负责**。text 计划的 `citations` SHALL 由 `plansToParts` / `plansToChunks` 挂为同一份 `providerMetadata: { agent: { citations } }`(空数组 MUST NOT 挂;`text-start` 与 `text-delta` 带同一 metadata——ai-sdk 的 metadata 合并语义为**覆盖非累加**,故 citations 放一次即可,MUST NOT 指望多次 delta 累加)

映射 SHALL 为: `runStarted` → `data-run-started` 部件;`messageDelta` → 本轮 assistant 消息当前 step 的 `text-delta`(文本增量)/ `reasoning-delta`(思考增量)chunk,part id 由该 step 内首个同类 delta 的 `seq` 派生(同一 part 后续 delta 复用);密封 message 的 text block → text 部件(整块追加,`t-<seq>-<n>`);thinking block → reasoning 部件;toolUse block → tool 部件 input;同 id toolResult block → 该 tool 部件 output(配对收口于适配层,跨消息按 id 配对,MUST NOT 再由视图层全局扫描);**`webSearch` block → `tool-web_search` 部件**(`toolCallId` = 块 `id`、`input = { query }`、`output` 见下);`systemNotice` → `data-system-notice`;`turnDone`(现 runResult,随信封演进) → `data-run-result`(携带 finish 语义;转录内事件的保真面,与 `metadata.record` 是两个层级,MUST NOT 互相替代);`raw` → `data-raw` 原文透传。`eventToChunk` 的 delta 分支与 `agent-transport.ts` SHALL **零搜索改动**(无搜索实时 delta)。

搜索块的部件载荷口径: use 侧块 → `input = { query }`,`output` 为配对命中时的来源载荷或 `null`;result 侧块(孤儿,use 已被裁)→ `input = null` + `output` 携带来源载荷(信息不丢,与既有孤儿工具卡片同哲学);两者 `output` 形状 SHALL 为 `AgentToolOutput` 的**加位形态**——`content` 保留字符串位(搜索无文本结果,置空串),`isError` = `error != null`,`search?: { results: WebSearchSource[]; error: string | null }` 承载结构化来源与错误(加位可选,既有消费面零改)。

两路契约 SHALL 为**终态同构 + 过程就地补全**:折叠路(重放 / 收口)与实时路 SHALL 在轮收口后呈同一形状(MUST NOT 出现成对工具卡的分裂或重复);实时流式期间跨消息 tool 配对 SHALL 就地补全——结果事件不另起消息、input 与 output 合住同一条消息(与折叠路同形)。配对视野随新 step 开出重武装,失效残差(结果与 toolUse 之间插入其他 step / 订阅起点晚于 toolUse)SHALL 回退既有占位卡形态并由轮收口以重放折叠纠正,MUST NOT 以「实时路复跑折叠语义」的方式消除(reducer 的配对视野物理上限于当前 step)。视野差异 SHALL 由视野层快照测试显式锁定(同消息配对两视野相等、未携流内视野时跨消息配对两视野可辨差异),MUST NOT 由单一路径 fixture 掩饰。

**id 锚点** SHALL 两路同源:assistant 消息 id = `turn-<turnId>`、提问消息 id = `turn-<turnId>-user`(折叠路与实时路 MUST 算出同一 id,否则收口时 ai-sdk 的 `isContinuation` 判定失配、消息重挂);**补充说明消息 id = `evt-<seq>`**(两路同源;该 id 恒属该轮中段那条 user 事件,MUST NOT 复用 `turn-<turnId>-user`——一个 id 只能对应一条消息);`turnId` 缺席(旧转录 / 无轮归属构造面)时实时路 SHALL 以 `evt-<首事件 seq>` 兜 id,收口折叠以整表替换对齐(MUST NOT 为旧转录造第二套折叠路径)。

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
- **THEN** assistant 消息 id 均为 `turn-<turnId>`、提问均 `turn-<turnId>-user`,补充说明均 `evt-<seq>`,`turnId` 缺席时实时路以 `evt-<首事件 seq>` 兜 id 并由收口整表替换对齐

#### Scenario: 折叠路呈现轮中段补充说明

- **WHEN** 某轮密封事件序列为 首条 `role = user`(提问,`seq = 3`)→ assistant 密封块 → `role = user`(补充说明,`seq = 9`)→ assistant 密封块 → `TurnDone` 与轮行
- **THEN** 该轮产出三条对话消息且序为 提问(id `turn-<turnId>-user`)→ 补充说明(id `evt-9`,parts 为该 user 事件 text 块直映射)→ assistant(id `turn-<turnId>`,含两段密封块的全部部件);aux system 消息照常逐事件独立;MUST NOT 丢补充说明、MUST NOT 把两条 user 事件合成一条

#### Scenario: 补充说明与提问 id 族不撞

- **WHEN** 同一轮既有提问(user 密封事件)又有两条补充说明(user 密封事件)
- **THEN** 三条 user 消息 id 两两不等(`turn-<turnId>-user` / `evt-<seq₁>` / `evt-<seq₂>`),assistant 消息仍恰一条;收口后重放同一转录产出同一 id 序列

## ADDED Requirements

### Requirement: 补充说明动作面与 composer 变体（steer）

前端 SHALL 让**一条正在跑的会话**可被用户补一句（补充说明，steer），且不新开轮：动作走会话级 `agent_steer` 薄包装（见 desktop-agent-execution「运行中会话的补充说明注入（steer 通道）」与「agent 执行命令面」）。本条与「展示组件族(对话透镜)」的 `AgentInput` 契约是**加位关系**：既有默认变体（含 permission-mode 档位、`running` 呈停止、`disabled` 整面板禁用）逐字不变，本条只追加变体与动作位。

- **实时路的轮中段 user 消息**：会话状态侧接收 `role = user` 的密封事件时 SHALL 分工——该轮**首条**（该轮提问）沿用既有 id 归一路径（本地副本原地换 id 为 `turn-<turnId>-user`，消息数不增）；**其余** SHALL 以 `evt-<seq>` **追加**为独立 user 气泡，按到达顺序插入（MUST NOT 因「同轮已有同名 id」而丢弃、MUST NOT 复用提问的归一路径——`turn-<turnId>-user` 恒属该轮提问，一个 id 只能对应一条消息）。收口后仍以折叠整表替换收敛，形状与重放一致（折叠口径见「事件适配层(纯函数,保真)」）。
- **headless 动作面**：`UseAgentSessionState` SHALL 增 `steer: (text: string) => void` 与 `steering: boolean`——前者为 `agent_steer(root, session_id, text)` 的薄包装（`root` / `sessionId` 为 null 时 MUST NOT invoke），后者为「已提交、等 ack 回流」的 pending 态（提交置 `true`，ack 成功或 `Err` 落地置 `false`）；`Err` SHALL 进既有 `error` 面呈现（与重放 / 流错误同一条保序合并链），MUST NOT 静默吞掉。既有 `answer` / `stop` / `sendMessage` 语义零改动。
- **composer 插槽动作面**：`AgentSessionActions` SHALL 增 `steer` 与 `steering` 两个字段（**additive**——既有 `running` / `loading` / `send` / `stop` 零改动；未接字段的消费点零影响），使只接 composer 插槽的只读消费点能只取「补充说明 + 停止」两动作。
- **`AgentInput` 补充说明变体**：`AgentInputProps` SHALL 增两个可选位——`hidePermissionMode?: boolean`（缺省 `false`；`true` 时不呈 permission-mode 档位行，`onSend` 的 `permissionMode` 参数位保留但下游 MUST NOT 消费）与 `pending?: boolean`（缺省 `false`；`true` 时发送入口呈「排队中…」态并禁用防连点，提示文案 SHALL 可辨；输入框与停止入口不受影响，`running` 的停止面照常呈现）。默认变体（两位皆缺省）的渲染与语义 MUST NOT 变化。
- **吸附层不另立规则**：补充说明气泡为普通 `role = 'user'` 消息，既有「全部 user 消息无条件常驻吸附层」契约自然覆盖它（一轮内因此可能出现两次吸顶：提问被补充说明顶走、补充说明接续吸顶）——MUST NOT 为补充说明新增第二套吸附判据、MUST NOT 改动 `agent-virtual.ts` 的 `userTurnSpans`。
- **判据不预判**：补充说明的可用性 MUST NOT 由前端判据决定（MUST NOT 依库态 `running`、轮行状态或会话是否存在禁用输入与发送）——后端三种不可送达各有独立 `Err` 文案，前端只负责呈现；`pending` 是「等 ack」的过程态，MUST NOT 被实现成「按运行态禁用」。

#### Scenario: 实时路轮中段补充说明不被吞

- **WHEN** 会话运行中，发送方 / 旁观方经在挂流收到该轮第二条 `role = user` 密封事件（`seq = 9`），该轮提问气泡已在场
- **THEN** 消息列表追加一条 id `evt-9` 的 user 气泡（内容为该事件 text 块），位置按到达顺序（提问之后、后续 assistant 部件所在消息之前）；MUST NOT 因 `turn-<turnId>-user` 已存在而丢弃该事件、MUST NOT 出现两条提问气泡

#### Scenario: 收口后补充说明仍可见

- **WHEN** 含一条补充说明的轮收口，状态以转录重放重建
- **THEN** 重建后该轮仍有 提问 → 补充说明（`evt-<seq>`）→ assistant 三部分，与实时路呈现形状一致

#### Scenario: steer 动作的 null 守卫与 pending 复位

- **WHEN** 调用 headless 的 `steer("先跑 lint")`：分别以在场 / `null` 的 `root` 或 `sessionId`；另在 ack 回流成功与后端 `Err` 两种收尾下观察
- **THEN** 在场时 invoke `agent_steer` 且三参逐字（`root` / `sessionId` / `text`）并置 `steering = true`；`root` 或 `sessionId` 为 null 时零 invoke、零副作用（`steering` 不变）；ack 成功与 `Err` 两种收尾都把 `steering` 置回 `false`，`Err` 文案进 `error` 面

#### Scenario: 补充说明 composer 无档位且呈 pending

- **WHEN** 以 `hidePermissionMode = true`、`pending = true`、`running = true` 渲染 `AgentInput`
- **THEN** 无 permission-mode 档位行；发送入口呈「排队中…」且不可重复提交；停止入口照常呈现可点；输入框仍可编辑

#### Scenario: 已收口步仍可点发送并以后端 Err 说话

- **WHEN** 某步会话已收口（库态无 running 轮行），用户在该步面板输入补充说明并点发送
- **THEN** 发送入口未被前端禁用，调用到达后端并返回 `Err`（「会话已收口」类文案），该文案呈现在会话错误面；MUST NOT 以「此步已收口」类前端判据提前禁用

#### Scenario: 默认 composer 变体零变化

- **WHEN** 既有消费点（explore / archi / change-chat / 调试页）不传 `hidePermissionMode` 与 `pending` 渲染 `AgentInput`
- **THEN** 渲染与交互与本变更前逐字一致（档位行在场、发送空串禁用、`running` 呈停止、`disabled` 整面板禁用 + 理由文案）

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / Component） |
|------|------|------|
| `packages/desktop/src/lib/agent-adapter.ts`（扩） | 折叠路的轮中段 user 消息 | `foldSessionMessages` 不变；`entryToMessages` 的 user 分支改「组内首条 → 提问（`questionMessage`）／其余 → 补充说明（`evt-<seq>`，parts 为 text 块直映射）」，按 `seq` 序排在该轮 assistant 之前；`questionId` 仅用于首条；`assistantMessage` 的 user 跳过逻辑不变 |
| `packages/desktop/src/hooks/use-agent-session.ts`（扩） | headless 补充说明动作 | `UseAgentSessionState` 增 `steer: (text: string) => void` 与 `steering: boolean`；新增 `useSteer(mirrors, sessionId, root)`（与 `useAskAnswer` 同型：null 守卫 + `actionError` 进 `error` 保序链）；`applyUserQuestion` 分化为「首条提问归一 / 其余以 `evt-<seq>` 追加」 |
| `packages/desktop/src/components/agent/agent-session-view.tsx`（扩） | 动作面加位 | `AgentSessionActions` 增 `steer: (text: string) => void` 与 `steering: boolean`（由 `state.steer` / `state.steering` 组装；composer render prop 原样多给两字段，既有插槽忽略即可） |
| `packages/desktop/src/components/agent/agent-input.tsx`（扩） | 补充说明变体 | `AgentInputProps` 增 `hidePermissionMode?: boolean`（缺省 false）与 `pending?: boolean`（缺省 false）；`pending` 时发送呈「排队中…」并禁用、停止面不受影响；默认变体渲染零变化 |
