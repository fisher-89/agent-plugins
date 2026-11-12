# desktop-agent-chat-infra Specification (Delta)

## MODIFIED Requirements

### Requirement: 会话级统一渲染组件（sessionId 入参 + 状态驱动挂流）

前端 SHALL 提供会话级统一渲染组件 `AgentSessionView`（`packages/desktop/src/components/agent/agent-session-view.tsx`）与其 headless 基建 `useAgentSession`（`packages/desktop/src/hooks/use-agent-session.ts`），作为「按 sessionId 渲染一个 agent 会话」的唯一实现：全部会话消费点（Agent 调试页 / explore 详情 / change 流程抽屉 / 历史重放）SHALL 消费本组件，MUST NOT 在视图层各自拼装消息、轮行或事件流。组件入参 SHALL 为 `root` + `sessionId`（会话唯一寻址键；MUST NOT 以 `source` / `source_ref` 反查会话），可选 `refreshKey`（会话状态重评估信号）、`composer` 插槽（发送 / 停止动作面的注入位）与 `askAnswer`（ask 应答动作面的注入位，见「ask 工具卡活卡与应答动作面」）。契约：

- **通用接口渲染**：挂载或 `sessionId` 变更 SHALL 经通用查询面装配会话——`session_detail(root, sessionId)` 单查取会话行 + 聚合统计 + 轮行，`agent_session_transcript(root, sessionId)` 取密封转录，经适配层纯函数重建消息状态（重放与收口折叠同一入口）；`sessionId === null` SHALL 呈空态且零取数、零订阅。
- **按会话状态挂流**：会话「进行中 / 完成」SHALL 由通用查询面推导（轮行清单含 running 轮行即进行中，与库态判定既有先例同源），MUST NOT 以前端发送方视角（`chat.status`）替代库态。进行中且无在挂流 → SHALL 建立会话订阅（`agent_session_watch`，`since_seq` = 已装载转录最大 `seq`）；完成态 → MUST NOT 建立订阅（零轮询、零定时刷新、零事件订阅）。状态重评估 SHALL 由挂载 / `sessionId` 变更 / `refreshKey` 触发。
- **单流在挂**：同一会话同一时刻至多一条在挂流（发起流 = 发送方 `agent_start` 的 Channel，或会话订阅流）；已在挂时 MUST NOT 再订阅（防同轮增量重复注入）。流以轮终态收口关闭，关闭后 SHALL 依会话状态重评估：仍进行中 → 以新 `since_seq` 重挂；已收敛 → 不挂。
- **过程增量 + 终态收敛**：流内 `messageDelta` SHALL 经适配层映射为 `text-delta` / `reasoning-delta` chunk 由 ai-sdk reducer 累积（token 级连续增长、思考 / 回复可辨）；轮收口（流结束）SHALL 以**转录库重放**重建消息状态——终态同构由「同一来源」保证（重放与收口折叠同一入口），MUST NOT 依赖两条发射实现碰巧一致；收口重放查询失败 SHALL 回落本地事件镜像折叠（收口不阻塞、不抛错）。
- **订阅起点部件重建**：会话订阅流 SHALL 在首个实时事件到达时，先按该轮把**已装载转录里同轮的密封事件**预投进同一簿记（等同该轮历史上原本就会走的帧序列），再投该实时事件。动因：`resume-stream` 在 ai-sdk 中以空消息状态起步，首个 chunk 的 `start` 命中「替换末条」——不预投则运行轮已落库的部件会被增量独占覆盖，表现为「打开进行中会话时历史一闪而过只剩新流」。预投 MUST NOT 另起 reducer 消息、MUST NOT 重复计入镜像（既有事件早已在镜像中），且 MUST NOT 在跨轮事件到达时错配（按轮次匹配后才投）。
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
- `AgentMessages` 对话透镜: user / assistant 气泡(**一轮一条 assistant 气泡**,部件按到达顺序)、工具对卡、思考折叠、AskUserQuestion 卡(问题与选项原样可读;应答动作面在场时呈**活卡**、缺席时呈静态只读卡,见「ask 工具卡活卡与应答动作面」);辅助载体消息(system 角色承载的 data 部件)MUST NOT 进对话呈现,对话透镜 SHALL 基于过滤后列表判定空态;空内容轮的可见性 SHALL 按该轮 `metadata.record.status` 判定而非 `parts` 是否为空(失败 / 停止轮呈轮内状态行,见「轮内状态行与空内容轮可见性」);
- `AgentInput` composer: prompt 输入（空串禁发;纯空白字符不单独拦截——trim 守卫不在组件侧）、permission-mode 档位（`StandardSelect` 选择器）、发送、停止入口;并 SHALL 提供**整面板禁用态**(`disabled` + 禁用理由文案——输入框与发送同时禁用,理由文案原样呈现),供来源侧按业务判据(如 change 流程运行中 / 归档只读,见 desktop-change-chat)整面板禁发;`running` 语义 SHALL 为「呈停止入口」——发送入口不禁用,重复发送由会话层 `isStreaming` 守卫忽略;停止入口 SHALL 只随 `running` 呈现且 MUST NOT 被 `disabled` 隐去(运行中即便整面板禁用仍可从面板停止);`disabled` MUST NOT 复用为「运行中」的第二表达;
- 工具卡注册表: 特化卡(AskUserQuestionCard 等)为可插拔位,未注册工具名回退默认对卡;
- 组件族 SHALL 共享底层块渲染件(文本 markdown / 思考折叠 / 工具卡一套实现);滚动 SHALL 由 `components/ui/message-scroller.tsx` 承载——该 shim 内部以 `@tanstack/react-virtual` 渲染**可见窗口切片**,并通过 sticky 吸附层让用户提问常驻(契约见「对话透镜虚拟窗口与用户消息顶部吸附」);消息状态仍留在应用层,MUST NOT 以分页窗口限制进入 DOM 的消息集合(MUST NOT 保留自研历史分页层)。上游 `message-scroller` primitives 退役后 `@shadcn/react` 在仓内再无**可达**消费者,该依赖与其零消费者残件(`components/ui/questionnaire.tsx`)SHALL 一并剪除(判据沿用 desktop-app-shell「knip 报出的未用导出 SHALL 删减」),MUST NOT 新增 `ignoreDependencies` / `ignore` 条目把未用面关成豁免。

调试页的 run 表单 / 历史列表 / JSONL 开关 SHALL 为页面级 chrome,包在共享核心外圈,MUST NOT 强行塞进组件族。

会话呈现形态 SHALL 归一为一个: 交互页(调试 / explore)在 composer 插槽注入自己的 composer(文本组装留来源侧),只读消费点(change 流程抽屉 / 历史重放)不接插槽;三形态历史差异(发起流独占 / 通知重查转录库 / 裸事件折叠无轮行)SHALL 随本次收编退役,MUST NOT 保留第二套会话渲染实现或第二套转录查询 hook。

**应答动作面与 composer 插槽正交**：ask 卡的应答动作面 SHALL 为**第二个独立可选注入位**（`askAnswer`），与 `composer` 插槽互不前置——只读消费点（change 流程抽屉）MAY 只接 `askAnswer` 而不接 `composer`（它需要应答 ask，但仍不提供发送 / 停止），交互页 MAY 两者都接。未接 `askAnswer` 时全部工具卡 MUST NOT 出现应答入口（含历史重放场景）。

#### Scenario: 对话透镜映射

- **WHEN** 会话状态含文本、思考、工具调用与工具结果
- **THEN** `AgentMessages` 按序呈现 markdown 气泡、可展开思考块、成对工具卡,AskUserQuestion 呈问题与选项可读的卡（无应答动作面注入时为静态只读卡）

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

## ADDED Requirements

### Requirement: ask 工具卡活卡与应答动作面

`AskUserQuestion` 工具部件 SHALL 呈**双态卡**（同一组件、同一卡根钩子，按应答动作面在场与否切换）：

- **静态态（未注入动作面）**：问题与选项原样可读、零交互（与本次变更前逐字相同的可读卡形态）；文案 MUST NOT 承诺「请在下方输入框作答」这类该页面并不成立的引导。
- **活卡态（注入动作面）**：每个选项 SHALL 呈现为可点按钮（点击即以该选项文本作答）；同时 SHALL 保留自由文本输入 + 提交入口（应答取值域不限于选项）；提交后 SHALL 在本地即时禁用（防连点 / 重复应答），等待下一次 `activeRun` / 部件状态变化复位；空串与纯空白 MUST NOT 提交。
- **注入路径**：动作面 SHALL 经 `AgentSessionView` → `AgentMessages` → 工具部件三层**可选 prop** 下行（`askAnswer: { onAnswer: (text: string) => void; pendingToolCallId?: string | null }`）；`AgentToolCardProps` SHALL 增可选 `askAnswer` 位。缺席时各层 MUST NOT 改变任何既有行为（零新渲染、零新 invoke）。
- **活卡判据（精确对账优先）**：给出 `pendingToolCallId` 时，仅 `toolCallId` 与之相等的 ask 部件为活卡（其余为静态卡）；未给出时按「该部件仍为无结果态」判活。MUST NOT 以「工具结果缺席即认为在等」作为唯一判据用于流程抽屉（历史转录中崩溃残留的未配对 `tool_use` 同形，会误呈为可应答）。
- **应答出口**：应答 SHALL 经消费点注入的 `onAnswer` 回流（通用会话面 = `useAgentSession.answer` → `agent_answer`；change 流程抽屉 = `change_flow_answer`），组件族 MUST NOT 直接 `invoke` 任何命令（组件族不认识业务域，沿用 composer 插槽「动作面由来源侧注入」纪律）。
- **错误可归因**：应答被后端拒绝（`Err`，如「当前无等待中的 ask」）SHALL 走来源侧既有错误面呈现（会话错误横幅 / 运行控制面板错误行），组件族 MUST NOT 静默吞掉、MUST NOT 自行重试。
- **零回退面变更**：未注册工具名仍回退 `ToolPairCard`；卡根钩子（`data-testid="block-tool-use"` + `data-tool-id`）与结果正文钩子（`data-testid="block-tool-result"`）逐字不变；`finish_step` 卡与其余特化卡零改动。

#### Scenario: 活卡点选项即应答

- **WHEN** 以 `askAnswer` 注入动作面渲染一个 `input-available` 的 `AskUserQuestion` 部件（`options = ["A","B"]`）
- **THEN** 卡内呈 A / B 两个可点按钮与一个自由文本输入；点 A → `onAnswer("A")` 恰调用一次，按钮随即禁用（防连点）；MUST NOT 直接调用任何 IPC 命令

#### Scenario: 活卡自由文本应答

- **WHEN** 在活卡的自由文本输入框键入「用方案 C」并提交
- **THEN** `onAnswer("用方案 C")` 恰调用一次；输入为空或纯空白时提交入口不产生调用

#### Scenario: 未注入即静态只读

- **WHEN** 以不传 `askAnswer` 的方式渲染同一 `AskUserQuestion` 部件（历史重放 / 只读消费点）
- **THEN** 卡呈问题与选项的可读文本、零按钮零输入框、零 invoke；卡内文案不含「请在下方输入框作答」类误导引导

#### Scenario: pendingToolCallId 精确对账

- **WHEN** 一次渲染中有两个 `AskUserQuestion` 部件且 `pendingToolCallId` 只等于其中之一
- **THEN** 只有 `toolCallId` 相等的那一个是活卡（可应答），另一个为静态只读卡；`pendingToolCallId` 为 `null` 时全为静态

#### Scenario: 动作面只达 ask 卡

- **WHEN** 以 `askAnswer` 渲染含 `bash` / `read` / `finish_step` / `AskUserQuestion` 四类部件的会话
- **THEN** 仅 `AskUserQuestion` 卡获得应答动作面，其余三卡渲染与行为零变化（它们不接收 `askAnswer`）

#### Scenario: 应答被拒可归因

- **WHEN** 后端拒绝一次应答（返回 `Err`）
- **THEN** 该 `Err` 进入来源侧既有错误面（会话错误横幅 / 控制面板错误行）并可读；组件族不重试、不吞错、不改写卡状态

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Component / Function / Type） |
|------|------|------|
| `packages/desktop/src/components/agent/tool-cards.tsx`（扩） | ask 卡双态 | `interface AgentToolCardProps` 增 `askAnswer?: { onAnswer: (text: string) => void }`；`AskUserQuestionCard` 双态渲染（静态只读 / 活卡：选项按钮 + 自由文本 + 提交，提交后本地禁用），卡根沿用 `cardAttrs(toolCallId)`；`BUILD_IN_TOOL_CARDS` 键面零变更 |
| `packages/desktop/src/components/agent/agent-messages.tsx`（扩） | 动作面透传单点 | `AgentMessagesProps` 增 `askAnswer?: { onAnswer: (text: string) => void; pendingToolCallId?: string \| null }`；`ToolPartView` 仅对 `AskUserQuestion` 名透传（按 `pendingToolCallId` 判活），其余卡零影响 |
| `packages/desktop/src/components/agent/agent-session-view.tsx`（扩） | 两形态同透传 | `AgentSessionViewProps` 增可选 `askAnswer`；自持形态（内部 `useAgentSession`）与注入形态（`session` prop）SHALL 都把 `answer` 动作与 `askAnswer.pendingToolCallId` 合成为同一动作面下行（自持形态可由组件自行提供 `onAnswer = session.answer`，注入形态由来源侧提供） |
| `packages/desktop/src/components/agent/session-panel.tsx`（扩） | 面板骨架合成动作面 | 不需新增 `SessionPanelState` 字段——`SessionPanelState.session` 已是 `UseAgentSessionState`，骨架直接以 `state.session.answer` 合成 `askAnswer` 下传 `AgentSessionView`（archi / change-chat 两处 hook 与面板零改动即得活卡） |
| `packages/desktop/src/hooks/use-agent-session.ts`（扩） | 通用侧应答动作 | `UseAgentSessionState` 增 `answer: (text: string) => void`（`root` / `sessionId` 为 null 时不 invoke；`ipc.agentAnswer(root, sessionId, text)` 的 `Err` 进既有 `error` 面） |
| `packages/desktop/src/views/explores/explore-detail-pane.tsx` / `views/agent/agent-debug-view.tsx`（扩） | 通用会话面接线 | 以 `askAnswer={{ onAnswer: session.answer }}` 注入（archi 与 change-chat 两处经 `SessionPanel` 骨架零改动即得——骨架从 `state.session.answer` 合成） |
| `packages/desktop/src/components/agent/tool-cards.test.tsx` / `agent-messages.test.tsx` / `agent-session-view.test.tsx`（测试） | 双态与透传判据 | 活卡两路应答 / 空串不提交 / 提交后禁用 / 未注入即静态且无旧文案 / `pendingToolCallId` 精确对账 / 动作面只达 ask 卡 / 两形态透传 |
