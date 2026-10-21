# desktop-agent-chat-infra Specification

## Purpose

定义 agent 会话统一基建的前端契约：ai-sdk v7 状态层（`UIMessage` 一等模型 + `TauriAgentTransport`）、事件适配层（`BlockPlan` IR 三层：`planBlock` 单语义点 + 视野层 `PairingScope` + 发射层 `plansToParts` / `plansToChunks`，保真锁定）、会话级统一渲染组件（`AgentSessionView`）、headless 会话基建（`use-agent-session`：重放装载 + 按会话状态挂流 + 发送组装 + 停止接线）、展示组件族（会话级入口 + 对话透镜 + composer + 工具卡注册表），以及 ai-sdk 依赖收口纪律。目标：今后所有 agent 会话页面（explore 会话 / agent 调试 / change 流程抽屉 / 历史重放 / 未来）统一消费本基建，一处实现「按 sessionId 渲染一个会话并按状态决定是否挂流」。

## Requirements

### Requirement: 统一会话状态基建(use-agent-chat)

前端 SHALL 提供 headless 会话基建 hook `use-agent-session`(置于 `packages/desktop/src/hooks/`;由 `use-agent-chat` 收编改名,职责扩为「重放装载 + 状态驱动挂流 + 发送组装 + 停止接线」),内部以 ai-sdk v7 `useChat` 承载对话状态,transport 为自定义 `TauriAgentTransport`(实现 v7 `ChatTransport`),把会话发起 invoke 与**会话订阅 invoke**(Tauri `Channel<AgentRunMessage>`:发起流与订阅流共用同一信封)适配为 `ReadableStream<UIMessageChunk>`。`UIMessage` SHALL 成为前端对话状态的一等模型。基建 SHALL 满足:

- **重放装载**: 打开会话时以会话 id **直查**会话单查命令 `session_detail(root, sessionId)`(携 root 寻址所属 workspace 库),再经会话转录重放取落库**密封转录**,经 `eventsToUIMessages` 纯函数重建 UIMessage 状态(`setMessages`);会话 id 由来源侧显式提供(`sourceRef` 反查取最近一条会话的路径 SHALL 退役;按 `source` / `source_ref` 列出会话供用户显式挑选的**清单筛选**不在此禁令内——属探测语义,选中后仍以 `sessionId` 直查)。重放重建的状态与实时累积收口的状态 SHALL **终态同一形状**(重开恢复 = 重放;两路契约见「事件适配层」的「终态同构 + 过程近似」,终态一致性由收口以重放收敛保证,而非由两条发射实现碰巧一致);`sessionId` 为 null 时不发起任何重放查询、不建立任何订阅。
- **发送穿透**: 发送 SHALL 经 `sendMessage` 以 `ChatRequestOptions.body` 携带应用层会话参数(`root`、`permissionMode`、会话引用 `sessionId`(`Continue` 语义,链尾/当前会话提供,无会话为 null)、`source`、可选 `sourceRef`、可选 `cwd`);会话状态由应用层在发送时刻提供,transport SHALL 为无状态转换器(不进构造闭包、无 ref 插线);会话文本组装(如 explore stance 拼接)SHALL 留在来源侧,基建只收口会话参数。`sourceRef` SHALL 由来源侧按需提供(如 change 详情通用 agent 面板以 change id 归属,见 desktop-change-chat),仅作归属记录与清单筛选,**MUST NOT 用于寻址或重放**;来源未提供时为 null(调试页 / explore 链行为不变)。`cwd` SHALL 同样由来源侧透传(change 环境 = worktree 执行根),只作会话执行根,MUST NOT 参与 store 寻址;来源未提供时缺省 `root`(既有语义)。transport SHALL 对两字段做运行时校验(非法形态即 `Err`,不改写),且会话参数 SHALL 以具名解构进入发起 invoke——MUST NOT 依赖位置参数索引拼接(参数面尾部增位时不得错位)。
- **增量流式**: 实时路 SHALL 消费 `MessageDelta` 事件——经 `eventToChunk` 映射为 ai-sdk 原生 `text-delta` / `reasoning-delta` chunk 挂 provisional 消息(思考/回复可辨),密封 Message 到达后以密封内容替换;一轮回复 SHALL 呈现为单条连续增长的消息, MUST NOT 出现逐 delta 碎行。实时路的起点 SHALL 双态:发送期由发起流承载(Channel 先于 invoke 建立,零空洞),旁观 / 重挂期由会话订阅流承载(带 `since_seq`);两起点 SHALL 经同一适配与开件簿记单点产生 chunk,MUST NOT 各自实现一份;同一会话同一时刻 SHALL 至多一条在挂流。
- **停止**: `stop` SHALL 触达 Rust 侧终止命令(MUST NOT 仅截断前端流);后端收敛后经由在挂流(发起流或订阅流)流出的终止终态部件照常进入状态。
- **终态同构**: 终态记录 SHALL 转为与实时事件同构的状态部件(`data-run-record`)附进流;重放路径 SHALL 从会话记录合成同款部件,且轮收口 SHALL 以转录库重放重建会话状态(终态以库为唯一权威;收口查询失败回落本地事件镜像折叠),续轮(取当前会话)从中取。

#### Scenario: 重放装载重建

- **WHEN** 打开一条已有两轮往复的会话(来源侧提供其 `sessionId`)
- **THEN** `use-agent-session` 以 `sessionId` 直查 `session_detail` 并取会话转录重放,`eventsToUIMessages` 重建出的 UIMessage 序列与该会话收口后的状态形状一致(第一轮内容在场,无缺轮)

#### Scenario: 会话参数穿透

- **WHEN** 在已有会话(会话 id=`s1`)中发送一条消息
- **THEN** `TauriAgentTransport.sendMessages` 收到的 options.body 含会话引用 `sessionId="s1"` 及 root / permissionMode / source,并原样进入会话发起 invoke 参数;transport 自身不持有会话状态

#### Scenario: 增量流式与密封替换

- **WHEN** 一轮回复的 text delta 逐个到达后密封 Message 收口
- **THEN** provisional 消息随 delta 连续增长(`text-delta` / `reasoning-delta`),密封到达后被密封内容替换,无碎行、无重复呈现

#### Scenario: 订阅流起点与簿记等价

- **WHEN** 旁观 (非发送方) 打开一条 running 会话,订阅流自 `since_seq` 后的首个 delta 到达
- **THEN** 该流复刻发起流的开件簿记(`start` + `reset-step` + part-start)后累积 delta,reducer 归约结果与发起流对同一事件序列的归约一致(无 `missing part` 错误),且两起点共用同一簿记实现

#### Scenario: 停止触达后端

- **WHEN** 运行中调用 `stop`
- **THEN** 前端发起 Rust 侧终止命令(收敛引擎与轮状态),在挂流随后流出的终止终态部件进入会话状态,MUST NOT 出现「前端已停、store 仍跑完」的偏差

#### Scenario: 来源引用与 cwd 穿透

- **WHEN** 来源侧以 `source = "change"`、`sourceRef = <change id>`、`cwd = <worktree>` 发送一条消息
- **THEN** `TauriAgentTransport.sendMessages` 收到的 body 含三字段并原样进入发起 invoke 参数(`source_ref` / `cwd` 位于参数面尾部);transport 无状态、零改写

#### Scenario: 未提供时缺省不漂移

- **WHEN** 调试页 / explore 链发送消息(不提供 `sourceRef` 与 `cwd`)
- **THEN** body 两字段为 null,发起 invoke 行为与改造前逐字一致(执行根 = `root`,归属 `sourceRef` 为 null)

#### Scenario: 清单筛选与寻址纪律并存

- **WHEN** 来源侧按 `source` / `source_ref` 拉取会话清单并点选其中一条
- **THEN** 清单仅作探测 / 选择入口;选中后的重放装载仍以 `sessionId` 直查 `session_detail` 与会话转录,零「按来源反查取最近一条」调用

### Requirement: 会话级统一渲染组件（sessionId 入参 + 状态驱动挂流）

前端 SHALL 提供会话级统一渲染组件 `AgentSessionView`（`packages/desktop/src/components/agent/agent-session-view.tsx`）与其 headless 基建 `useAgentSession`（`packages/desktop/src/hooks/use-agent-session.ts`），作为「按 sessionId 渲染一个 agent 会话」的唯一实现：全部会话消费点（Agent 调试页 / explore 详情 / change 流程抽屉 / 历史重放）SHALL 消费本组件，MUST NOT 在视图层各自拼装消息、轮行或事件流。组件入参 SHALL 为 `root` + `sessionId`（会话唯一寻址键；MUST NOT 以 `source` / `source_ref` 反查会话），可选 `refreshKey`（会话状态重评估信号）与 `composer` 插槽（发送 / 停止动作面的注入位）。契约：

- **通用接口渲染**：挂载或 `sessionId` 变更 SHALL 经通用查询面装配会话——`session_detail(root, sessionId)` 单查取会话行 + 聚合统计 + 轮行，`agent_session_transcript(root, sessionId)` 取密封转录，经适配层纯函数重建消息状态（重放与收口折叠同一入口）；`sessionId === null` SHALL 呈空态且零取数、零订阅。
- **按会话状态挂流**：会话「进行中 / 完成」SHALL 由通用查询面推导（轮行清单含 running 轮行即进行中，与库态判定既有先例同源），MUST NOT 以前端发送方视角（`chat.status`）替代库态。进行中且无在挂流 → SHALL 建立会话订阅（`agent_session_watch`，`since_seq` = 已装载转录最大 `seq`）；完成态 → MUST NOT 建立订阅（零轮询、零定时刷新、零事件订阅）。状态重评估 SHALL 由挂载 / `sessionId` 变更 / `refreshKey` 触发。
- **单流在挂**：同一会话同一时刻至多一条在挂流（发起流 = 发送方 `agent_start` 的 Channel，或会话订阅流）；已在挂时 MUST NOT 再订阅（防同轮增量重复注入）。流以轮终态收口关闭，关闭后 SHALL 依会话状态重评估：仍进行中 → 以新 `since_seq` 重挂；已收敛 → 不挂。
- **过程增量 + 终态收敛**：流内 `messageDelta` SHALL 经适配层映射为 `text-delta` / `reasoning-delta` chunk 由 ai-sdk reducer 累积（token 级连续增长、思考 / 回复可辨）；轮收口（流结束）SHALL 以**转录库重放**重建消息状态——终态同构由「同一来源」保证（重放与收口折叠同一入口），MUST NOT 依赖两条发射实现碰巧一致；收口重放查询失败 SHALL 回落本地事件镜像折叠（收口不阻塞、不抛错）。
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

- `eventsToUIMessages(events: AgentEvent[]): UIMessage[]` — 重放重建(仅消费**密封事件**:会话转录中不存在 delta,delta 不进重放路径)
- `eventToChunk(event: AgentEvent): UIMessageChunk[]` — 实时增量(含 delta 路;返回**chunk 组**:事件头 + 块 chunk)
- **语义层 IR**: `BlockPlan` 为块语义的唯一表达(text / reasoning / tool 三变体;tool 的 `output: null` 即未配对),`planBlock(block, paired): BlockPlan | null` 为**唯一语义点**(四 block 变体 + 无主 toolResult 合成在此收口;`null` = 已配对 result 不重复呈现);孤儿 result 与正常 tool 合并为同一 `tool` 变体(占位工具名 + `input: null`),MUST NOT 在两路各写一遍孤儿判定
- **视野层**: `PairingScope` 隔离配对视野差异——折叠路 `global`(跨消息预扫全部 toolUse 的配对 result),实时路 `local`(仅当前 message 内配对);两视野 SHALL 产出同一类型 `Map<toolCallId, ToolResultBlock>`,差异 MUST NOT 渗入语义层与发射层
- **发射层**: `plansToParts` / `plansToChunks` 为纯机械映射(无判定、无视野),两路的块呈现 SHALL 由同一 `BlockPlan[]` 驱动;`plansToChunks` 只产块相关 chunk,事件头(`start` / `reset-step`)仍由 `eventToChunk` 负责

映射 SHALL 为: `runStarted` → `data-run-started` 部件;`messageDelta` → provisional 消息上的 `text-delta`(文本增量)/ `reasoning-delta`(思考增量)chunk,配对键挂部件/消息元数据(键形态随信封定稿);密封 message 的 text block → text 部件(整块收口,替换同键 provisional);thinking block → reasoning 部件;toolUse block → tool 部件 input;同 id toolResult block → 该 tool 部件 output(配对收口于适配层,跨消息按 id 配对,MUST NOT 再由视图层全局扫描);`systemNotice` → `data-system-notice`;`turnDone`(现 runResult,随信封演进) → `data-run-result`(携带 finish 语义);`raw` → `data-raw` 原文透传。

两路契约 SHALL 为**终态同构 + 过程近似**:折叠路(重放 / 收口)与实时路 SHALL 在轮收口后呈同一形状(MUST NOT 出现成对工具卡的分裂或重复);实时流式期间的形状差异(跨消息 tool 配对在实时路只可见 `input-available`、result 到达时闪现第二张占位卡)SHALL 视为**过程近似**——由轮收口以重放折叠纠正,MUST NOT 以「实时路复跑折叠语义」的方式消除(reducer 的配对视野物理上限于当前消息,复用即降级折叠路能力)。视野差异 SHALL 由视野层快照测试显式锁定(同消息配对两视野相等、跨消息配对两视野可辨差异),MUST NOT 由单一路径 fixture 掩饰。

保真 SHALL 锁定: `seq` 可考(进 message id 或部件元数据,保序与 key 稳定;delta chunk 元数据携带源 seq);`parentToolUseId` 可考(进部件元数据,子代理归因依赖它);未知事件 MUST NOT 丢弃(一律 `data-raw` 透传原文)。

穷尽性 SHALL 编译期可探测: `auxEventData` 与 `planBlock` SHALL 以 `const _exhaustive: never` 承接变体穷尽(新增 `AgentEvent` / `AgentBlock` 变体时编译报错提醒,MUST NOT 仅靠 runtime 静默降级 `data-raw`);runtime 未知变体的 `data-raw` 兜底语义照常保留(编译期穷尽 + runtime 兜底两层)。

#### Scenario: 全变体映射

- **WHEN** 以密封五变体 `AgentEvent` 序列(runStarted / message(四 block)/ systemNotice / turnDone / raw)驱动 `eventsToUIMessages`
- **THEN** 产出对应的 UIMessage 部件序列:text、reasoning、tool、`data-run-started`、`data-system-notice`、`data-run-result`、`data-raw`,顺序与 seq 一致

#### Scenario: delta 路映射

- **WHEN** `eventToChunk` 先后收到 text delta 与 thinking delta 事件
- **THEN** 产出挂同一 provisional 消息键的 `text-delta` 与 `reasoning-delta` chunk(思考/回复可辨),后续密封 message 到达时整块替换 provisional 内容

#### Scenario: 工具同 id 配对

- **WHEN** toolUse(id=`t1`)与 toolResult(id=`t1`)分处不同密封 message
- **THEN** 适配后 `t1` 收敛为单个 tool 部件(input 与 output 同住),视图层无需跨消息扫描配对

#### Scenario: 保真字段可考

- **WHEN** 检查适配产出的任一 UIMessage
- **THEN** 可从 message id 或部件元数据读出原事件 `seq`,message 部件元数据保留 `parentToolUseId`(含子代理消息的非空归因)

#### Scenario: 未知事件不丢

- **WHEN** 事件流含未知类型事件(未来引擎新增)
- **THEN** 产出 `data-raw` 部件且原文 JSON 完整保留,适配不报错、不丢事件

#### Scenario: 计划层对拍同形

- **WHEN** 同一 `BlockPlan[]`(text / reasoning / tool 已配对 / tool 未配对四形态)分别喂 `plansToParts` 与 `plansToChunks`(后者经 reducer 归约)
- **THEN** 两路呈现的形状一致(部件类型 / 顺序 / 工具态 / output 载荷逐一相等),对拍不再依赖第三份手写语义投影(`shapeOf` 类辅助退役)

#### Scenario: 视野差异显式锁定

- **WHEN** toolUse 与同 id toolResult 分处两条密封 message(跨消息配对场景)
- **THEN** 视野层快照显式断言: `global` 视野折叠为单个 `output-available` 计划、`local` 视野产出 `input-available` 计划 + 孤儿卡 `output-available` 计划(差异具体形态被锁定,防扩大);同消息内配对两视野相等

#### Scenario: 收口折叠纠正双卡

- **WHEN** 实时流式期间跨消息 tool 配对闪现「占位卡 + 结果卡」双卡
- **THEN** 轮收口以重放折叠纠正为单张 `output-available` 工具卡(终态同构),MUST NOT 出现收口后仍双卡或部件重复

#### Scenario: 变体穷尽编译期可探测

- **WHEN** 审查 `auxEventData` / `planBlock` 实现
- **THEN** 两者均以 `const _exhaustive: never` 承接穷尽;新增 `AgentEvent` / `AgentBlock` 变体而漏改时编译报错,runtime 未知事件的 `data-raw` 兜底仍保留

### Requirement: 展示组件族(对话透镜)

> 2026-10-10 回溯修订(无 change 流直修):保真透镜 `AgentTimeline` 撤编,组件族收敛为 `AgentMessages` 单对话透镜——辅助载体(system 角色消息:run-started / system-notice / raw / run-record)整条不进对话呈现;保真面由适配层状态部件(序列与归因字段见「事件适配层」requirement)与调试页页面级「原始 JSONL」切换承载,不再双透镜并行。

`packages/desktop/src/components/agent/` SHALL 提供共享展示组件族,全部 agent 会话页面(explore 会话 / agent 调试 / change 流程抽屉 / 历史重放 / 未来页面)统一消费:

- `AgentSessionView` 会话级唯一入口: 入参 `root` + `sessionId`(+ 可选 `refreshKey` / composer 插槽),内部经通用查询面装配会话并按会话状态挂流(契约见「会话级统一渲染组件」requirement);消费点 MUST NOT 自行拼装消息、轮行或事件流;
- `AgentMessages` 对话透镜: user / assistant 气泡、工具对卡、思考折叠、AskUserQuestion 静态卡(问题与选项原样可读);辅助载体消息(system 角色承载的 data 部件)MUST NOT 进对话呈现,对话透镜 SHALL 基于过滤后列表判定空态;
- `AgentInput` composer: prompt 输入、permission-mode 档位、发送(运行中禁发或忽略)、停止入口;并 SHALL 提供**整面板禁用态**(`disabled` + 禁用理由文案——输入框与发送同时禁用,理由文案原样呈现),供来源侧按业务判据(如 change 流程运行中 / 归档只读,见 desktop-change-chat)整面板禁发;`running` 语义 SHALL 保持不变(禁发 + 呈停止入口),`disabled` MUST NOT 复用为「运行中」的第二表达;
- 工具卡注册表: 特化卡(AskUserQuestionCard 等)为可插拔位,未注册工具名回退默认对卡;
- 组件族 SHALL 共享底层块渲染件(文本 markdown / 思考折叠 / 工具卡一套实现);滚动 SHALL 沿用 shadcn `message-scroller` 组合,消息状态留在应用层。

调试页的 run 表单 / 历史列表 / JSONL 开关 SHALL 为页面级 chrome,包在共享核心外圈,MUST NOT 强行塞进组件族。

会话呈现形态 SHALL 归一为一个: 交互页(调试 / explore)在 composer 插槽注入自己的 composer(文本组装留来源侧),只读消费点(change 流程抽屉 / 历史重放)不接插槽;三形态历史差异(发起流独占 / 通知重查转录库 / 裸事件折叠无轮行)SHALL 随本次收编退役,MUST NOT 保留第二套会话渲染实现或第二套转录查询 hook。

#### Scenario: 对话透镜映射

- **WHEN** 会话状态含文本、思考、工具调用与工具结果
- **THEN** `AgentMessages` 按序呈现 markdown 气泡、可展开思考块、成对工具卡,AskUserQuestion 呈静态可读卡

#### Scenario: 辅助载体不进对话

- **WHEN** 会话状态含 run-started / system-notice / raw / run-record 等 system 载体消息
- **THEN** 对话透镜不呈现其任何 data 部件(空态判定基于过滤后列表),seq / `parentToolUseId` 保真仍可考于部件元数据

#### Scenario: composer 运行中禁发与停止

- **WHEN** 会话处于运行中
- **THEN** `AgentInput` 忽略重复发送并呈现停止入口;停止后恢复可发送

#### Scenario: 会话级入口归一

- **WHEN** 检索全部会话呈现消费点(调试页 / explore 详情 / change 流程抽屉 / 历史重放)
- **THEN** 全部经 `AgentSessionView` 渲染,视图层无 `eventsToUIMessages` 直调、无自有转录查询 hook、无自有事件流装配

#### Scenario: 只读消费点形态

- **WHEN** change 流程抽屉以只读方式渲染某 WorkerAgent 会话
- **THEN** 组件呈对话透镜与轮行,不呈发送 / 停止入口;运行中会话的 delta 流式呈现,收口后与重放形状一致

#### Scenario: 整面板禁用与理由呈现

- **WHEN** 来源侧以 `disabled = true` 与理由文案渲染 `AgentInput`
- **THEN** 输入框与发送按钮同时禁用、理由文案可见(用户不会「打了字发不出去」);禁用态不呈停止入口,`running` 的既有呈现不被顶替

### Requirement: ai-sdk 依赖收口

ai-sdk(`ai` 包)SHALL 锚定 v7 并收口于基建层(适配层、transport、`use-agent-session`、`components/agent/`);业务视图层(`views/` 下页面与 hooks)MUST NOT 直接 import ai-sdk;Rust 层与 store 层 MUST NOT 出现 ai-sdk 概念(`UIMessage` 是纯前端状态模型,`AgentEvent` 信封「线格式 = 落库形态(密封) = DTO 镜像」三位一体随 desktop-agent-execution 能力演进——`MessageDelta` 仅存在于线格式与 DTO 镜像,不进落库形态)。`ChatTransport` / `UIMessageChunk` 等签名变动 SHALL 由适配层与组件族吸收(MUST NOT 波及视图层)。

#### Scenario: import 收口

- **WHEN** 检索 `ai` 包的 import 来源
- **THEN** 仅出现于基建层文件(`lib/agent-adapter.ts`、`lib/agent-transport.ts`、`hooks/use-agent-session.ts`、`components/agent/`),`views/` 与 Rust/store 无 ai-sdk import 或概念

#### Scenario: 信封演进同步

- **WHEN** 审查本变更的 Rust 侧 diff 与 `types/dto.ts`
- **THEN** `AgentEvent` 信封演进(增 `MessageDelta`、TurnDone 口径)经 export-bindings 再生成同步镜像(camelCase 一致),delta 变体不出现在任何落库形态,无 UIMessage 结构倒灌 Rust

## Module Contract

### Component: `packages/desktop/src/components/agent/agent-session-view.tsx`（新增）

| 组件 | Props | 描述 |
|------|-------|------|
| `AgentSessionView` | `root: string \| null`；`sessionId: string \| null`；`refreshKey?: number`；`composer?: React.ReactNode`（或 render prop）；`data-testid?` | 会话级唯一渲染入口：经 `sessionDetail` + `agentSessionTranscript` 装配会话，按会话状态（running 轮行）决定是否挂 `agent_session_watch`；单流在挂；轮收口以重放收敛；composer 插槽可选（只读消费点不接） |

### Function: `packages/desktop/src/hooks/use-agent-session.ts`（新增，收编 `use-agent-chat.ts`）

| 函数 | 签名 | 描述 |
|------|------|------|
| `useAgentSession` | `(params: { root: string \| null; sessionId: string \| null; source: string; refreshKey?: number }) => UseAgentSessionState` | 重放装载（密封转录 → `eventsToUIMessages`）；状态驱动挂流（running 且无在挂流才订阅，`since_seq` = 镜像最大 seq）；单流在挂；收口以重放收敛（失败回落镜像折叠）；断流重订；`sendMessage` / `stop` 动作面；返回 `messages` / `events` / `session` / `chain` / `loading` / `running`（库态驱动）/ `error` |

### 退役: `packages/desktop/src/hooks/use-agent-chat.ts`（删）

| 项 | 变化 | 描述 |
|------|------|------|
| `useAgentChat` | 收编进 `useAgentSession` | 单一会话基建；调用点（`views/agent`、`views/explores`）全量切换，MUST NOT 保留双 hook |

### API: `packages/desktop/src/lib/agent-transport.ts`

| 成员 | 签名 | 描述 |
|------|------|------|
| `sendMessages` | `(options) => Promise<ReadableStream<AgentUIMessageChunk>>` | 发送期发起流（`agent_start` + `Channel<AgentRunMessage>`），零空洞；信封 → chunk 经共享簿记单点 |
| `reconnectToStream` | `(options: { chatId, abortSignal?, body?: { root, sessionId, sinceSeq } }) => Promise<ReadableStream<AgentUIMessageChunk> \| null>` | 旁观 / 重挂期订阅流（`agent_session_watch`）；body 参数由 `chat.resumeStream({ body })` 透传，transport 保持无状态；无运行轮时返回 `null` |

### Function: `packages/desktop/src/lib/agent-adapter.ts`

| 函数 | 签名 | 描述 |
|------|------|------|
| `planBlock` | `(block: AgentBlock, paired: PairedResults) => BlockPlan \| null` | 块语义唯一表达（四变体 + 无主 result 合成为 `tool` + `input: null`；`null` = 已配对 result 不重复呈现）；`const _exhaustive: never` 穷尽守卫 |
| `plansToParts` | `(plans: BlockPlan[]) => AgentUIMessage['parts']` | 折叠路发射（纯机械） |
| `plansToChunks` | `(plans: BlockPlan[], seq: number) => AgentUIMessageChunk[]` | 实时路发射（纯机械 + part id 派生） |
| `eventsToUIMessages` | `(events: AgentEvent[]) => AgentUIMessage[]` | 重放 / 收口折叠（`PairingScope.global`） |
| `eventToChunk` | `(event: AgentEvent) => AgentUIMessageChunk[]` | 实时增量（`PairingScope.local`，含 delta 路与事件头） |

### 既有模块（更新）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/components/agent/` | 展示组件族 | 会话级入口 `AgentSessionView` + 对话透镜 `AgentMessages`(辅助载体 system 消息过滤)+ `AgentInput` + 工具卡注册表；provisional/密封差异由状态层吸收;共享块渲染件;`message-scroller` 滚动承载 |
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | 探索链消费面 | 会话域参数(source/sessionId 会话引用)语义等价切换,行为不回退;`sourceRef` 反查退役 |
| Rust 会话域命令面(详见 desktop-agent-execution) | stop 与提前 resolve | 终止命令按运行中的会话活动轮寻址;发起命令提前 resolve;`stopped` 收敛;终态经 Channel 同构部件流出;会话订阅命令 `agent_session_watch` 供旁观 / 重挂期挂流 |
| `packages/desktop/src/lib/agent-transport.ts` | 会话参数面透传 | `AgentStartSessionParams` 增 `sourceRef` / `cwd`(派生自生成绑定,无手写镜像);`readSessionParams` 增读两字段(`readNullableString` 复用,非法即 `Err`);`invokeStart` 改**具名解构**调用 `agentStart`(位置索引魔法 1/2/3/4/5/7 退役,尾部增位不再错位) |
| `packages/desktop/src/hooks/use-agent-session.ts` | 参数面与发送 body | `UseAgentSessionParams` 增 `sourceRef?: string \| null` / `cwd?: string \| null`;`sendMessage` body 透传两字段(原 `sourceRef: null` 常量退役,未提供仍为 null) |
| `packages/desktop/src/components/agent/agent-input.tsx` | 禁用态扩展 | 增 `disabled?: boolean` / `disabledReason?: string`(textarea 与发送同时禁用 + 理由文案呈现;`running` 语义与停止入口不变) |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | 注入形态入参 | 会话状态面仍由来源侧建 hook 注入;如需向 composer 插槽传禁用态,仅在 `AgentSessionActions` 上增位(additive),MUST NOT 在组件内自判业务禁发 |
| `packages/desktop/src/lib/agent-transport.test.ts` / `hooks/use-agent-session.test.ts` | 透传直测 | 两字段按序到达 invoke;非法值拒绝;未提供时缺省 null 且既有行为不变 |

