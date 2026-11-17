# desktop-agent-chat-infra Specification (Delta)

## MODIFIED Requirements

### Requirement: 会话级统一渲染组件（sessionId 入参 + 状态驱动挂流）

前端 SHALL 提供会话级统一渲染组件 `AgentSessionView`（`packages/desktop/src/components/agent/agent-session-view.tsx`）与其 headless 基建 `useAgentSession`（`packages/desktop/src/hooks/use-agent-session.ts`），作为「按 sessionId 渲染一个 agent 会话」的唯一实现：全部会话消费点（Agent 调试页 / explore 详情 / change 流程抽屉 / 历史重放）SHALL 消费本组件，MUST NOT 在视图层各自拼装消息、轮行或事件流。组件入参 SHALL 为 `root` + `sessionId`（会话唯一寻址键；MUST NOT 以 `source` / `source_ref` 反查会话），可选 `refreshKey`（会话状态重评估信号）、`composer` 插槽（发送 / 停止动作面的注入位）与 `askAnswer`（ask 应答动作面的注入位，见「ask 工具卡活卡与应答动作面」）。契约：

- **通用接口渲染**：挂载或 `sessionId` 变更 SHALL 经通用查询面装配会话——`session_detail(root, sessionId)` 单查取会话行 + 聚合统计 + 轮行，`agent_session_transcript(root, sessionId)` 取密封转录，经适配层纯函数重建消息状态（重放与收口折叠同一入口）；`sessionId === null` SHALL 呈空态且零取数、零订阅。
- **按会话状态挂流**：会话「进行中 / 完成」SHALL 由通用查询面推导（轮行清单含 running 轮行即进行中，与库态判定既有先例同源），MUST NOT 以前端发送方视角（`chat.status`）替代库态。进行中且无在挂流 → SHALL 建立会话订阅（`agent_session_watch`，`since_seq` = 已装载转录最大 `seq`）；完成态 → MUST NOT 建立订阅（零轮询、零定时刷新、零事件订阅）。状态重评估 SHALL 由挂载 / `sessionId` 变更 / `refreshKey` 触发。
- **单流在挂**：同一会话同一时刻至多一条在挂流（发起流 = 发送方 `agent_start` 的 Channel，或会话订阅流）；已在挂时 MUST NOT 再订阅（防同轮增量重复注入）。流以轮终态收口关闭，关闭后 SHALL 依会话状态重评估：仍进行中 → 以新 `since_seq` 重挂；已收敛 → 不挂。
- **过程增量 + 终态收敛**：流内 `messageDelta` SHALL 经适配层映射为 `text-delta` / `reasoning-delta` chunk 由 ai-sdk reducer 累积（token 级连续增长、思考 / 回复可辨）；轮收口（流结束）SHALL 以**转录库重放**重建消息状态——终态同构由「同一来源」保证（重放与收口折叠同一入口），MUST NOT 依赖两条发射实现碰巧一致；收口重放查询失败 SHALL 回落本地事件镜像折叠（收口不阻塞、不抛错）。
- **订阅起点部件重建**：会话订阅流 SHALL 在首个实时事件到达时，先按**该事件的轮**把**已装载转录里同轮的密封事件**预投进同一簿记（等同该轮历史上原本就会走的帧序列），再投该实时事件。**播种键 SHALL 由首个实时事件自证**（取该 `AgentEvent.turnId`）；`session_detail` 的轮行清单 SHALL 只用作「有无 `running` 轮行」的状态门，MUST NOT 用作选轮 / 播种键依据——库态 `running` 行可能是进程中断的残留（启动标定失败即存活）、也可能与实时轮并发，按行选轮一旦选错，预投就落在错误的轮上而实时轮已落库的部件照旧被增量覆盖。动因：`resume-stream` 在 ai-sdk 中以空消息状态起步，首个 chunk 的 `start` 命中「替换末条」——不预投则运行轮已落库的部件会被增量独占覆盖，表现为「打开进行中会话时历史一闪而过只剩新流」。预投 MUST NOT 另起 reducer 消息、MUST NOT 重复计入镜像（既有事件早已在镜像中），且 MUST NOT 在跨轮事件到达时错配（按轮次匹配后才投）。
- **断流兜底**：订阅流异常中断或服务端滞后断流 SHALL 触发「重放 + 重订」——重查转录库刷新镜像后以新 `since_seq` 重挂；MUST NOT 静默续跑（无「缺一段增量仍装作在线」的假态）。
- **动作面可选**：发送 / 停止 SHALL 为可选动作面（headless 暴露、composer 插槽注入），只读消费点（change 抽屉 / 历史重放）MUST NOT 被迫接线；订阅流在挂期间 `chat.status` 处于 `streaming`，既有发送守卫 SHALL 自然拦下旁观误发（MUST NOT 以「旁观恒 idle」的发送方视角放行）。**ask 应答动作面 SHALL 为同一性质的第二个可选注入位**：自持形态（组件内部 `useAgentSession`）MAY 由组件以 `session.answer` 自组装 `onAnswer`，注入形态 SHALL 由来源侧给出；两形态都把 `askAnswer` 原样下行 `AgentMessages`，缺席时全卡静态（**两个动作面互不前置**——只接 `askAnswer` 的只读消费点拿不到发送 / 停止，只接 `composer` 的拿不到 ask 应答）。
- **应答动作语义**：headless 基建 SHALL 暴露 `answer(text)`（`agent_answer(root, session_id, text)` 的薄包装，见 desktop-agent-execution「agent 执行命令面」）；`root` / `sessionId` 为 null 时 MUST NOT invoke；后端 `Err`（无等待中的 ask / 已应答）SHALL 进既有 `error` 面呈现，MUST NOT 静默吞掉。

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

#### Scenario: 播种键由首个实时事件自证

- **WHEN** 打开一条会话，库中残留一条较早的 `running` 轮行（进程中断遗留、标定未生效）而实时流属于其后开出的新一轮，订阅流首个事件到达
- **THEN** 播种按首个事件的 `turnId` 选中**实时轮**，只预投该轮的密封事件；该轮已落库部件与随后的增量同处一条 assistant 消息且全部在场；残留轮行不作播种依据、其事件零预投（MUST NOT 出现「历史被增量冲掉只剩新流」，也 MUST NOT 跨轮串扰）

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

- **WHEN** change 流程抽屉或历史重放以只读方式消费组件（不传 composer 插槽与 `askAnswer`）
- **THEN** 组件不呈现发送 / 停止入口、不发起 `agent_start`、ask 卡为静态只读；运行中会话的增量照常只读流式呈现

#### Scenario: 两形态同透传 askAnswer

- **WHEN** 分别以自持形态（只给 `root` + `sessionId`）与注入形态（给 `session` prop）挂载组件，且 `askAnswer` 均在场
- **THEN** 两形态下 `AgentMessages` 都收到同一 `askAnswer`（`pendingToolCallId` 一并透传），ask 卡可应答；自持形态的 `onAnswer` 由组件以 `session.answer` 组装、注入形态用来源侧给出的 `onAnswer`

#### Scenario: answer 动作的参数面与 null 守卫

- **WHEN** 调用 headless 基建的 `answer("A")`，分别以在场 / `null` 的 `sessionId` 或 `root`
- **THEN** 在场时 invoke `agent_answer` 且三参逐字（`root` / `sessionId` / `text`）；`root` 或 `sessionId` 为 null 时零 invoke、零副作用；`Err` 结果进入会话错误面

### Requirement: 展示组件族(对话透镜)

> 2026-10-10 回溯修订(无 change 流直修):保真透镜 `AgentTimeline` 撤编,组件族收敛为 `AgentMessages` 单对话透镜——辅助载体(system 角色消息:run-started / system-notice / raw / run-result;2026-11-06 起 `run-record` 退役,轮行改挂回合元数据)整条不进对话呈现;保真面由适配层状态部件(序列与归因字段见「事件适配层」requirement)与调试页页面级「原始 JSONL」切换承载,不再双透镜并行。

`packages/desktop/src/components/agent/` SHALL 提供共享展示组件族,全部 agent 会话页面(explore 会话 / agent 调试 / change 流程抽屉 / 历史重放 / 未来页面)统一消费:

- `AgentSessionView` 会话级唯一入口: 入参 `root` + `sessionId`(+ 可选 `refreshKey` / composer 插槽 / `askAnswer` 应答动作面),内部经通用查询面装配会话并按会话状态挂流(契约见「会话级统一渲染组件」requirement);消费点 MUST NOT 自行拼装消息、轮行或事件流;
- `AgentMessages` 对话透镜: user / assistant 气泡(**一轮一条 assistant 气泡**,部件按到达顺序)、工具对卡、思考折叠、AskUserQuestion 卡(问题与选项原样可读;应答动作面在场时呈**活卡**、缺席时呈静态只读卡,见「ask 工具卡活卡与应答动作面」);超长 user 提问 SHALL 默认折叠为预览(展开后回到原部件渲染,见「超长 user 提问默认折叠」);辅助载体消息(system 角色承载的 data 部件)MUST NOT 进对话呈现,对话透镜 SHALL 基于过滤后列表判定空态;空内容轮的可见性 SHALL 按该轮 `metadata.record.status` 判定而非 `parts` 是否为空(失败 / 停止轮呈轮内状态行,见「轮内状态行与空内容轮可见性」);运行中状态 MUST NOT 在透镜内另立标记(`agent-messages-running` 已退役——运行态由会话元信息徽标与 composer 停止入口承载);
- `AgentInput` composer: prompt 输入（空串禁发;纯空白字符不单独拦截——trim 守卫不在组件侧）、permission-mode 档位（`StandardSelect` 选择器）、发送、停止入口;并 SHALL 提供**整面板禁用态**(`disabled` + 禁用理由文案——输入框与发送同时禁用,理由文案原样呈现),供来源侧按业务判据(如 change 流程运行中 / 归档只读,见 desktop-change-chat)整面板禁发;`running` 语义 SHALL 为「呈停止入口」——发送入口不禁用,重复发送由会话层 `isStreaming` 守卫忽略;停止入口 SHALL 只随 `running` 呈现且 MUST NOT 被 `disabled` 隐去(运行中即便整面板禁用仍可从面板停止);`disabled` MUST NOT 复用为「运行中」的第二表达;
- 工具卡注册表: 特化卡(AskUserQuestionCard 等)为可插拔位,未注册工具名回退默认对卡;
- 部件运行态判据: 工具部件的「运行中」呈现 SHALL 以**该轮状态**为准（轮 `status === 'running'`）——轮已终结（`completed` / `failed` / `stopped` / `interrupted`）时，未配对的部件 MUST NOT 呈运行态（`TraceLine` 的运行标记 / shimmer 退场，取 `pending` 或无状态）,因为未配对只说明「没有结果落库」,不说明「正在跑」；`running` 轮内的未配对部件 SHALL 照常呈运行态。判据 MUST NOT 以「工具结果缺席」单独推断运行中;
- 组件族 SHALL 共享底层块渲染件(文本 markdown / 思考折叠 / 工具卡一套实现);滚动 SHALL 由 `components/ui/message-scroller.tsx` 承载——该 shim 内部以 `@tanstack/react-virtual` 渲染**可见窗口切片**,并通过 sticky 吸附层让用户提问常驻(契约见「对话透镜虚拟窗口与用户消息顶部吸附」);消息状态仍留在应用层,MUST NOT 以分页窗口限制进入 DOM 的消息集合(MUST NOT 保留自研历史分页层)。`@shadcn/react` 的 `message-scroller` primitives 已退役,但 `@shadcn/react/questionnaire` SHALL 保留为 ask 卡的表单基础件(由 `components/ui/questionnaire.tsx` shim 消费);该依赖的其余入口 MUST NOT 新增消费者。knip 报出的未用导出 SHALL 删减,且 MUST NOT 新增 `ignoreDependencies` / `ignore` 条目把未用面关成豁免。

调试页的 run 表单 / 历史列表 / JSONL 开关 SHALL 为页面级 chrome,包在共享核心外圈,MUST NOT 强行塞进组件族。

会话呈现形态 SHALL 归一为一个: 交互页(调试 / explore)在 composer 插槽注入自己的 composer(文本组装留来源侧),只读消费点(change 流程抽屉 / 历史重放)不接插槽;三形态历史差异(发起流独占 / 通知重查转录库 / 裸事件折叠无轮行)SHALL 随本次收编退役,MUST NOT 保留第二套会话渲染实现或第二套转录查询 hook。

**应答动作面与 composer 插槽正交**：ask 卡的应答动作面 SHALL 为**第二个独立可选注入位**（`askAnswer`），与 `composer` 插槽互不前置——只读消费点（change 流程抽屉）MAY 只接 `askAnswer` 而不接 `composer`（它需要应答 ask，但仍不提供发送 / 停止），交互页 MAY 两者都接。未接 `askAnswer` 时全部工具卡 MUST NOT 出现应答入口（含历史重放场景）。

#### Scenario: 对话透镜映射

- **WHEN** 会话状态含文本、思考、工具调用与工具结果
- **THEN** `AgentMessages` 按序呈现 markdown 气泡、可展开思考块、成对工具卡,AskUserQuestion 呈问题与选项可读的卡（无应答动作面注入时为静态只读卡）

#### Scenario: 终结轮的未配对部件不呈运行态

- **WHEN** 渲染一条 `interrupted`（或 `failed` / `stopped`）轮，其中含未配对的工具部件（`input-available`，无结果，如进程在 bash 执行中死亡）
- **THEN** 该部件卡壳不呈运行态（无 shimmer），卡内按「无结果」呈现；同一会话中 `running` 轮的同类部件仍呈运行态

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
- **THEN** 组件呈对话透镜与各轮状态(轮行以回合元数据承载、由该轮消息呈现,不再是独立 system 消息),不呈发送 / 停止入口;运行中会话的 delta 流式呈现,收口后与重放形状一致;若来源侧接了 `askAnswer`,该抽屉的 ask 卡可应答而其余动作面仍缺席

#### Scenario: 整面板禁用与理由呈现

- **WHEN** 来源侧以 `disabled = true` 与理由文案渲染 `AgentInput`
- **THEN** 输入框与发送按钮同时禁用(空串禁用之外再叠加整面板禁用)、理由文案可见(用户不会「打了字发不出去」);`running` 的停止入口 SHALL 照常呈现(两标志语义正交,禁用只收发送面不收停止面)

#### Scenario: 两个动作面互不前置

- **WHEN** 某消费点只传 `askAnswer` 而不传 `composer`（或反之）
- **THEN** 只接 `askAnswer` → ask 卡可应答、零发送 / 停止入口；只接 `composer` → 发送 / 停止照常、ask 卡保持静态只读；两者都不传 → 全静态（现状零变化）

### Requirement: ask 工具卡活卡与应答动作面

`AskUserQuestion` 工具部件 SHALL 呈**双态卡**（同一组件，按应答动作面在场与否切换），卡内表单以 `@shadcn/react/questionnaire` primitives（经 `components/ui/questionnaire.tsx` shim）承载：

- **卡壳**：卡 SHALL 由 `TraceLine`（标题「提问」，`output` 缺席且**该轮仍在运行**时为 running 图标、`output` 缺席而该轮已终结时为提问图标（运行态退场——未配对不等于在等）、落定后为提问图标 / 错误态）与 `Card` 包裹；问卷表单根 SHALL 带 `data-testid="ask-question-card"`，每问节点带 `data-testid="ask-question"`，每个选项行带 `data-testid="ask-answer-option"`。
- **静态态（未注入动作面）**：问题与选项原样可读、零应答出口——选项行 SHALL 照常呈现（表单行可选中，但无提交入口、无自由文本输入，故零 `onAnswer` 调用）；文案 MUST NOT 承诺「请在下方输入框作答」这类该页面并不成立的引导。
- **活卡态（注入动作面且该部件无结果，且为单问）**：选项 SHALL 呈现为可选的单选行，并 SHALL 提供自由文本输入（`data-testid="ask-answer-input"`）与提交入口（`data-testid="ask-answer-submit"`）；**选项点击本身 MUST NOT 提交**——须经提交入口（或表单 Enter）才投递；提交后 SHALL 在本地即时回落静态（提交与输入入口退场，防连点 / 重复应答），等待下一次 `activeRun` / 部件状态变化复位；空表单（未选项且未输入非空白文本）MUST NOT 提交。
- **应答载荷（单值原文）**：`onAnswer` SHALL 收到**用户原文**——选中的选项值或自由文本，**不是** JSON 字符串。自由文本值 MUST NOT 在组件内 trim 或改写。结构（`{status, answers}` 信封与 `name` 键位）SHALL 由后端 ask 执行体按工具声明铸造（单点，见 desktop-agent-execution「ask 工具协议与阻塞应答通道」）；组件族 MUST NOT 拼装应答 JSON——两处入口（本卡与右栏 `AskCard`）载荷同形。协议一次一问，故载荷恒单值。
- **多问形态只读**：历史转录里的 `questions` 数组形态（旧工具参数面）SHALL 仍可解析呈现（问题与选项逐项可读），即便注入了动作面也 MUST NOT 提供提交入口——当前协议一次一问、应答恒单值，投递一个值会静默丢弃其余问。
- **已应答回填**：工具结果正文为应答信封时，SHALL 按 `answers[<本问 name>]` 回填选中项（选中项可选、其余项禁用）且零应答入口；旧扁平 map（无 `status`，键为该问名）SHALL 同样回填；结果正文既非信封亦非扁平 map（旧线原文应答 / 错误文案）时 MUST NOT 抛错——按无匹配选项呈现选项行原样。
- **不可解析输入**：问题形态无法归一（既非单问也非 `questions` 数组）时 SHALL 呈破坏性提示（标题「无法解析问题」+ 原始输入 JSON），MUST NOT 渲染问卷卡根、MUST NOT 抛错。
- **注入路径**：动作面 SHALL 经 `AgentSessionView` → `AgentMessages` → 工具部件三层**可选 prop** 下行（`askAnswer: { onAnswer: (text: string) => void; pendingToolCallId?: string | null }`）；`AgentToolCardProps` SHALL 增可选 `askAnswer` 位。缺席时各层 MUST NOT 改变任何既有行为（零新渲染、零新 invoke）。
- **问题名来源**：卡内每问的 `name` SHALL 取输入声明的 `name` 字段（协议声明值）；历史转录输入无该字段时回落 `Q1`（与旧扁平 map 的键同源，保证回填仍命中）。
- **活卡判据（精确对账优先，轮状态兜底）**：给出 `pendingToolCallId` 时，仅 `toolCallId` 与之相等的 ask 部件为活卡（其余为静态卡）；未给出时 SHALL 以**该轮 `status === 'running'`** 判活（静态只读消费点无 id 对账面，轮状态是唯一权威）。MUST NOT 以「工具结果缺席即认为在等」判活——崩溃残留的未配对 `tool_use` 与之同形，会让已终结轮的卡呈可应答，用户点了才收到「当前无等待中的 ask」`Err`。该约束 SHALL 由结构保证（判据读轮状态），MUST NOT 仅靠调用侧纪律。
- **应答出口**：应答 SHALL 经消费点注入的 `onAnswer` 回流（通用会话面 = `useAgentSession.answer` → `agent_answer`；change 流程抽屉 = `change_flow_answer`），组件族 MUST NOT 直接 `invoke` 任何命令（组件族不认识业务域，沿用 composer 插槽「动作面由来源侧注入」纪律）。载荷为用户原文单值；后端把它铸成信封后作为工具结果回灌。
- **错误可归因**：应答被后端拒绝（`Err`，如「当前无等待中的 ask」）SHALL 走来源侧既有错误面呈现（会话错误横幅 / 运行控制面板错误行），组件族 MUST NOT 静默吞掉、MUST NOT 自行重试。
- **零回退面变更**：未注册工具名仍回退 `ToolPairCard`；`finish_step` 卡与其余特化卡零改动。

#### Scenario: 活卡选选项并提交

- **WHEN** 以 `askAnswer` 注入动作面渲染一个 `input-available` 的 `AskUserQuestion` 部件（单问，`name = "pick"`，`options = ["A","B"]`）
- **THEN** 卡内呈 A / B 两个可选项、一个自由文本输入与提交入口；只点 A 时 `onAnswer` 零调用；再点提交 → `onAnswer('A')` 恰调用一次（用户原文，非 JSON），提交与输入入口随即退场；MUST NOT 直接调用任何 IPC 命令

#### Scenario: 活卡自由文本应答

- **WHEN** 在活卡的自由文本输入框键入「用方案 C」并提交
- **THEN** `onAnswer('用方案 C')` 恰调用一次（组件不 trim、不改写原文、不拼 JSON）；未选项且未输入非空白文本时提交入口不产生调用

#### Scenario: 多问形态（历史转录）呈只读

- **WHEN** 以两问形态（`questions = [Q1, Q2]`，旧工具参数面）注入动作面渲染
- **THEN** 两问的问题与选项 SHALL 逐项可读，但 MUST NOT 出现提交入口或自由文本输入（点击选项不产生 `onAnswer` 调用）——当前协议一次一问，单值载荷无法表达多问应答

#### Scenario: 未注入即无应答出口

- **WHEN** 以不传 `askAnswer` 的方式渲染同一 `AskUserQuestion` 部件（历史重放 / 只读消费点）
- **THEN** 卡呈问题与选项的可读文本、零提交入口、零自由文本输入、零 invoke；卡内文案不含「请在下方输入框作答」类误导引导

#### Scenario: 已应答回填与历史原文兼容

- **WHEN** 分别在 `output.content = '{"status":"answered","answers":{"pick":"B"}}'`、`output.content = '{"Q1":"B"}'`（旧扁平 map）与 `output.content = '旧线原文'` 下渲染同一部件（`name = "pick"` / 无 `name` 两种输入各一）
- **THEN** 信封按 `answers[<name>]` 回填 B 为选中项、其余选项禁用且零应答入口；旧扁平 map 按同名键回填（输入无 `name` 时键回落 `Q1`）；`旧线原文` 不抛错（不触发 `JSON.parse` 异常），选项行按原样可读、零应答入口

#### Scenario: pendingToolCallId 精确对账

- **WHEN** 一次渲染中有两个 `AskUserQuestion` 部件且 `pendingToolCallId` 只等于其中之一
- **THEN** 只有 `toolCallId` 相等的那一个是活卡（有提交入口），另一个为静态卡（无提交 / 无输入入口）；`pendingToolCallId` 为 `null` 时全为静态

#### Scenario: 已终结轮的 ask 卡零应答入口

- **WHEN** 以注入形态（不传 `pendingToolCallId`）渲染一条 `interrupted` 轮，其中 `AskUserQuestion` 部件无结果（进程在停等期死亡遗留）
- **THEN** 该卡呈静态只读（问题与选项可读、无提交入口、无自由文本输入、卡壳无运行态）；点击/提交入口不产生 `onAnswer`；同一会话 `running` 轮的无结果 ask 卡仍为活卡

#### Scenario: 动作面只达 ask 卡

- **WHEN** 以 `askAnswer` 渲染含 `bash` / `read` / `finish_step` / `AskUserQuestion` 四类部件的会话
- **THEN** 仅 `AskUserQuestion` 卡获得应答动作面，其余三卡渲染与行为零变化（它们不接收 `askAnswer`）

#### Scenario: 应答被拒可归因

- **WHEN** 后端拒绝一次应答（返回 `Err`）
- **THEN** 该 `Err` 进入来源侧既有错误面（会话错误横幅 / 控制面板错误行）并可读；组件族不重试、不吞错、不改写卡状态

### Requirement: 轮内状态行与空内容轮可见性

对话透镜 SHALL 让每一轮在时间线上**可见且带轮状态**，异常与主动停止 MUST NOT 表现为「界面只是不再增长」：

- **每轮恒一条 assistant 消息**：折叠与实时两路 SHALL 都按轮产出恰一条 assistant 消息（id `turn-<turnId>`），空内容轮不例外；轮状态 SHALL 只从该消息的 `metadata.record.status` 读取（MUST NOT 从 `parts` 是否为空反推轮状态）。
- **状态行（`parts` 为空时）**：`status = failed` SHALL 呈现状态行——**友好错误标题 + 可展开的原始错误原文**（原文取自 `metadata.record.error` 与该轮 `api_error` 通知，缺失时回退通用标题）；`status = stopped` SHALL 呈现「已停止」行；`status = interrupted` SHALL 呈现「因应用重启中断」行（进程中断的客观终止，与用户主动停止可辨；MUST NOT 复用失败标题及其错误原文展开面——标定记因不是用户需要处置的故障）；`status = completed` 或轮行缺席 SHALL 不渲染。状态行 SHALL 带 `data-testid` 挂钩（供测试与来源侧定位），MUST NOT 依赖样式类名查询。
- **`parts` 非空时**：SHALL 只呈现正常气泡（思考 / 正文 / 工具卡），MUST NOT 额外叠加状态行；轮失败/停止的事实仍可由 `metadata.record` 与顶部提示读出。
- **顶部失败横幅边界**：会话级入口的失败横幅 SHALL 只表达「最新一轮**失败**」（`status = failed`），`stopped` MUST NOT 触发该横幅（用户主动停止不是运行失败）；两处提示并存不冲突（横幅 = 全局最新状态，状态行 = 该轮位置）。
- **过滤单一来源**：对话透镜的可见性判据 SHALL 只在一处维护（system 角色过滤 + 空内容轮按轮状态判定，状态白名单为 `failed` / `stopped` / `interrupted`），MUST NOT 在其他组件重复实现第二份判据；该判据经 `src/lib/agent-virtual.ts` 的 `filterConversation` 单点实现，虚拟窗口的项数 SHALL 基于该过滤后列表（MUST NOT 把不渲染的空内容消息计入虚拟列表 `count`）。

#### Scenario: 失败轮空内容呈状态行

- **WHEN** 某轮以 `parts` 空收口，`metadata.record.status = failed`、`error` 携 provider 错误原文
- **THEN** 该轮在时间线该位置呈现状态行：友好标题可见，展开后原始错误原文可见；消息 id 为 `turn-<turnId>`（不参与虚拟列表项数与吸附层几何的错误计数）

#### Scenario: 停止轮空内容呈已停止行

- **WHEN** 用户在运行中点击停止，该轮以 `parts` 空收口、`metadata.record.status = stopped`
- **THEN** 该轮位置呈现「已停止」行（不再是「界面只是不再增长」）；顶部失败横幅 MUST NOT 出现（`stopped` 不算失败）

#### Scenario: 中断轮空内容呈状态行

- **WHEN** 某轮以 `parts` 空收口（进程在工具调用后、首条正文落库前死亡），经启动标定后 `metadata.record.status = interrupted`、`error` 为标定记因
- **THEN** 该轮在时间线该位置呈「因应用重启中断」行（带 `data-testid` 与 `data-status="interrupted"` 挂钩）、可见性判据保留该轮；顶部失败横幅 MUST NOT 出现（中断不是运行失败）

#### Scenario: 空内容 completed 轮不进时间线

- **WHEN** 某轮 `parts` 空且 `metadata.record.status = completed`（或该轮无轮行）
- **THEN** 对话透镜不渲染该轮（无空气泡、无状态行），虚拟列表 `count` 不计入

#### Scenario: 有内容的轮不叠加状态行

- **WHEN** 某轮 `parts` 非空且 `metadata.record.status = stopped`（停止前已产出部分回答）
- **THEN** 该轮只呈现正常气泡（思考 / 正文 / 工具卡按到达顺序），MUST NOT 追加「已停止」状态行；轮状态仍可从消息 metadata 读出

## Module Contract

### Function: `packages/desktop/src/lib/agent-transport.ts`

| 项 | 签名 / 变化 | 描述 |
|------|------|------|
| `TauriAgentTransport.reconnectToStream` | 播种键改派 | 状态门沿用 `detail.turns`（有无 `running` 轮行）；播种键改由首个实时事件自证（`AgentEvent.turnId`），`detail.turns` MUST NOT 参与选轮 |
| `createPendingSeed` / `PendingSeed` | `expect` / `seedIfMatching` 契约调整（形态 design 定稿） | 登记「已装载镜像 + 游标」，首个实时事件到达时以该事件 `turnId` 过滤后一次性预投；仍只投一次、仍不进镜像、仍跨轮不串扰 |

### Function: `packages/desktop/src/lib/agent-virtual.ts`

| 项 | 变化 | 描述 |
|------|------|------|
| `filterConversation` | 可见性判据加一支 | 空内容轮的保留白名单由 `failed` / `stopped` 扩为 `failed` / `stopped` / `interrupted`（中断轮因此可见、状态行可读）；判据单点不变 |

### Component: `packages/desktop/src/components/agent/agent-messages.tsx`

| 项 | Props / 变化 | 描述 |
|------|------|------|
| `isLiveAsk` | 兜底分支改判据 | `pendingToolCallId === undefined` 时以该轮 `status === 'running'` 判活，MUST NOT 以 `part.state === 'input-available'`（= 结果缺席）兜底 |
| `TurnStatusLine` | 增 `interrupted` 支 | 呈「因应用重启中断」（`data-testid="turn-status-line"` / `data-status="interrupted"`），不进失败标题与错误原文展开面 |
| `PartView` / `ToolPartView` | 增「轮是否仍在运行」下行位 | 从该轮消息 `metadata.record.status` 取值穿到工具卡（缺省按运行中，保持无轮行的历史转录现状） |

### Component: `packages/desktop/src/components/agent/tool-cards.tsx`

| 项 | Props / 变化 | 描述 |
|------|------|------|
| `AgentToolCardProps` | 增可选「轮已终结」位 | 四张特化卡（Bash / Read / Write / AskUserQuestion）共用；缺席按运行中处理 |
| `TraceLine status`（四处同形状） | 判据加支 | 轮已终结且部件无结果时取 `pending`（或省略）而非 `running`，shimmer 退场；轮运行中沿用现状 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `packages/desktop/src/hooks/use-agent-session.ts` | `running` 派生已是「轮行清单含 `running`」⇒ `interrupted` 天然不派生运行中、不触发订阅；零改动 |
| `packages/desktop/src/components/agent/agent-error.ts` | `latestAgentFailure` 只认 `failed` 与「running 且失败 done」⇒ `interrupted` 天然不触顶部失败横幅；零改动（由测试固化该默认） |
| `packages/desktop/src/lib/agent-adapter.ts` | 事件适配层零改动：本变更不新增事件词汇（中断是库态事实，不是事件） |
| `packages/desktop/src/types/generated/**` | 仅随 `AgentRunStatus` 值域再生成（见 desktop-workspace-store），chat-infra 侧零手工改动 |
