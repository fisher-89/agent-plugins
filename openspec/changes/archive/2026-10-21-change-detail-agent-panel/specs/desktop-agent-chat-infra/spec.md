# desktop-agent-chat-infra Specification (Delta)

## MODIFIED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/lib/agent-transport.ts` | 会话参数面透传 | `AgentStartSessionParams` 增 `sourceRef` / `cwd`(派生自生成绑定,无手写镜像);`readSessionParams` 增读两字段(`readNullableString` 复用,非法即 `Err`);`invokeStart` 改**具名解构**调用 `agentStart`(位置索引魔法 1/2/3/4/5/7 退役,尾部增位不再错位) |
| `packages/desktop/src/hooks/use-agent-session.ts` | 参数面与发送 body | `UseAgentSessionParams` 增 `sourceRef?: string \| null` / `cwd?: string \| null`;`sendMessage` body 透传两字段(原 `sourceRef: null` 常量退役,未提供仍为 null) |
| `packages/desktop/src/components/agent/agent-input.tsx` | 禁用态扩展 | 增 `disabled?: boolean` / `disabledReason?: string`(textarea 与发送同时禁用 + 理由文案呈现;`running` 语义与停止入口不变) |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | 注入形态入参 | 会话状态面仍由来源侧建 hook 注入;如需向 composer 插槽传禁用态,仅在 `AgentSessionActions` 上增位(additive),MUST NOT 在组件内自判业务禁发 |
| `packages/desktop/src/lib/agent-transport.test.ts` / `hooks/use-agent-session.test.ts` | 透传直测 | 两字段按序到达 invoke;非法值拒绝;未提供时缺省 null 且既有行为不变 |
