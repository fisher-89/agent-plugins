# desktop-agent-chat-infra Specification

## Purpose

定义 agent 会话统一基建的前端契约:ai-sdk v7 状态层(`UIMessage` 一等模型 + `TauriAgentTransport`)、事件适配层(`eventsToUIMessages` / `eventToChunk` 纯函数,保真锁定)、headless 会话 hook(`use-agent-chat`:重放装载 + 发送组装 + 停止接线)、展示组件族(双透镜 + composer + 工具卡注册表),以及 ai-sdk 依赖收口纪律。目标:今后所有 agent 会话页面(explore 会话 / agent 调试 / 未来)统一消费本基建。

## Requirements

### Requirement: 统一会话状态基建(use-agent-chat)

前端 SHALL 提供 headless 会话基建 hook `use-agent-chat`(置于 `packages/desktop/src/hooks/`),内部以 ai-sdk v7 `useChat` 承载对话状态,transport 为自定义 `TauriAgentTransport`(实现 v7 `ChatTransport`),把会话发起 invoke + Tauri `Channel<AgentEvent>` 适配为 `ReadableStream<UIMessageChunk>`。`UIMessage` SHALL 成为前端对话状态的一等模型。基建 SHALL 满足:

- **重放装载**: 打开会话时经会话域查询命令(会话清单 + 会话转录重放,携 root)取落库**密封转录**,经 `eventsToUIMessages` 纯函数重建 UIMessage 状态(`setMessages`);重放重建的状态与实时累积收口的状态 SHALL 同一形状(重开恢复 = 重放,两路同构)。
- **发送穿透**: 发送 SHALL 经 `sendMessage` 以 `ChatRequestOptions.body` 携带应用层会话参数(`root`、`permissionMode`、会话引用(`Continue{id}`,链尾/当前会话提供)、`source`、`sourceRef`);会话状态由应用层在发送时刻提供,transport SHALL 为无状态转换器(不进构造闭包、无 ref 插线);会话文本组装(如 explore stance 拼接)SHALL 留在来源侧,基建只收口会话参数。
- **增量流式**: 实时路 SHALL 消费 `MessageDelta` 事件——经 `eventToChunk` 映射为 ai-sdk 原生 `text-delta` / `reasoning-delta` chunk 挂 provisional 消息(思考/回复可辨),密封 Message 到达后以密封内容替换;一轮回复 SHALL 呈现为单条连续增长的消息, MUST NOT 出现逐 delta 碎行。
- **停止**: `stop` SHALL 触达 Rust 侧终止命令(MUST NOT 仅截断前端流);后端收敛后 Channel 流出的终止终态部件照常进入状态。
- **终态同构**: 终态记录 SHALL 转为与实时事件同构的状态部件(`data-run-record`)附进流;重放路径 SHALL 从会话记录合成同款部件,两路收敛到同一状态形状,续轮(取当前会话)从中取。

#### Scenario: 重放装载重建

- **WHEN** 打开一条已有两轮往复的会话
- **THEN** `use-agent-chat` 经会话域查询取落库密封转录,`eventsToUIMessages` 重建出的 UIMessage 序列与该会话实时运行收口后的状态形状一致(第一轮内容在场,无缺轮)

#### Scenario: 会话参数穿透

- **WHEN** 在已有会话(会话 id=`s1`)中发送一条消息
- **THEN** `TauriAgentTransport.sendMessages` 收到的 options.body 含会话引用 `s1` 及 root / permissionMode / source / sourceRef,并原样进入会话发起 invoke 参数;transport 自身不持有会话状态

#### Scenario: 增量流式与密封替换

- **WHEN** 一轮回复的 text delta 逐个到达后密封 Message 收口
- **THEN** provisional 消息随 delta 连续增长(`text-delta` / `reasoning-delta`),密封到达后被密封内容替换,无碎行、无重复呈现

#### Scenario: 停止触达后端

- **WHEN** 运行中调用 `stop`
- **THEN** 前端发起 Rust 侧终止命令(收敛引擎与轮状态),Channel 随后流出的终止终态部件进入会话状态,MUST NOT 出现「前端已停、store 仍跑完」的偏差

### Requirement: 事件适配层(纯函数,保真)

基建 SHALL 提供纯函数适配层(置于 `packages/desktop/src/lib/agent-adapter.ts`):

- `eventsToUIMessages(events: AgentEvent[]): UIMessage[]` — 重放重建(仅消费**密封事件**:会话转录中不存在 delta,delta 不进重放路径)
- `eventToChunk(event: AgentEvent): UIMessageChunk` — 实时增量(含 delta 路)

映射 SHALL 为: `runStarted` → `data-run-started` 部件;`messageDelta` → provisional 消息上的 `text-delta`(文本增量)/ `reasoning-delta`(思考增量)chunk,配对键挂部件/消息元数据(键形态随信封定稿);密封 message 的 text block → text 部件(整块收口,替换同键 provisional);thinking block → reasoning 部件;toolUse block → tool 部件 input;同 id toolResult block → 该 tool 部件 output(配对收口于适配层,跨消息按 id 配对,MUST NOT 再由视图层全局扫描);`systemNotice` → `data-system-notice`;`turnDone`(现 runResult,随信封演进) → `data-run-result`(携带 finish 语义);`raw` → `data-raw` 原文透传。

保真 SHALL 锁定: `seq` 可考(进 message id 或部件元数据,保序与 key 稳定;delta chunk 元数据携带源 seq);`parentToolUseId` 可考(进部件元数据,子代理归因依赖它);未知事件 MUST NOT 丢弃(一律 `data-raw` 透传原文)。

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

### Requirement: 展示组件族(双透镜)

`packages/desktop/src/components/agent/` SHALL 提供共享展示组件族,全部 agent 会话页面(explore 会话 / agent 调试 / 未来页面)统一消费:

- `AgentMessages` 对话透镜: user / assistant 气泡、工具对卡、思考折叠、AskUserQuestion 静态卡(问题与选项原样可读);
- `AgentTimeline` 保真透镜: seq 序呈现、子代理按 `parentToolUseId` 分组归因、raw 原文透传;
- `AgentInput` composer: prompt 输入、permission-mode 档位、发送(运行中禁发或忽略)、停止入口;
- 工具卡注册表: 特化卡(AskUserQuestionCard 等)为可插拔位;
- 双透镜 SHALL 共享底层块渲染件(文本 markdown / 思考折叠 / 工具卡一套实现);滚动 SHALL 沿用 shadcn `message-scroller` 组合,消息状态留在应用层。

调试页的 run 表单 / 历史列表 / JSONL 开关 SHALL 为页面级 chrome,包在共享核心外圈,MUST NOT 强行塞进组件族。

#### Scenario: 对话透镜映射

- **WHEN** 会话状态含文本、思考、工具调用与工具结果
- **THEN** `AgentMessages` 按序呈现 markdown 气泡、可展开思考块、成对工具卡,AskUserQuestion 呈静态可读卡

#### Scenario: 保真透镜归因

- **WHEN** 事件流含 `parentToolUseId` 非空的子代理消息
- **THEN** `AgentTimeline` 将其分组归因到父工具调用下,seq 序与 raw 透传可读

#### Scenario: composer 运行中禁发与停止

- **WHEN** 会话处于运行中
- **THEN** `AgentInput` 忽略重复发送并呈现停止入口;停止后恢复可发送

### Requirement: ai-sdk 依赖收口

ai-sdk(`ai` 包)SHALL 锚定 v7 并收口于基建层(适配层、transport、`use-agent-chat`、`components/agent/`);业务视图层(`views/` 下页面与 hooks)MUST NOT 直接 import ai-sdk;Rust 层与 store 层 MUST NOT 出现 ai-sdk 概念(`UIMessage` 是纯前端状态模型,`AgentEvent` 信封「线格式 = 落库形态(密封) = DTO 镜像」三位一体随 desktop-agent-execution 能力演进——`MessageDelta` 仅存在于线格式与 DTO 镜像,不进落库形态)。`ChatTransport` / `UIMessageChunk` 等签名变动 SHALL 由适配层与组件族吸收(MUST NOT 波及视图层)。

#### Scenario: import 收口

- **WHEN** 检索 `ai` 包的 import 来源
- **THEN** 仅出现于基建层文件(`lib/agent-adapter.ts`、`lib/agent-transport.ts`、`hooks/use-agent-chat.ts`、`components/agent/`),`views/` 与 Rust/store 无 ai-sdk import 或概念

#### Scenario: 信封演进同步

- **WHEN** 审查本变更的 Rust 侧 diff 与 `types/dto.ts`
- **THEN** `AgentEvent` 信封演进(增 `MessageDelta`、TurnDone 口径)经 export-bindings 再生成同步镜像(camelCase 一致),delta 变体不出现在任何落库形态,无 UIMessage 结构倒灌 Rust

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/lib/agent-adapter.ts` | 事件适配纯函数(双层) | `eventsToUIMessages`(仅密封转录)/ `eventToChunk`(+delta 路:`text-delta` / `reasoning-delta` → provisional,密封替换);保真锁定(seq、`parentToolUseId`、配对键、`data-*` 部件、未知透传);工具同 id 配对收口 |
| `packages/desktop/src/lib/agent-transport.ts` | `ChatTransport` 适配 | 会话域 body 参数穿透发起 invoke;Channel 事件 → `UIMessageChunk` 流(含 delta);终态部件 + finish 收流;`reconnectToStream` 返回 null(重连由会话重放承担) |
| `packages/desktop/src/hooks/use-agent-chat.ts` | headless 会话基建 | `useChat`(ai v7)+ transport;重放装载(会话查询 → 密封转录 → `setMessages`);发送组装(会话参数收口,文本组装留来源侧);`stop` → 终止命令;密封收口归一 |
| `packages/desktop/src/components/agent/` | 展示组件族 | 语义不变(provisional/密封差异由状态层吸收);双透镜共享块渲染件;`message-scroller` 滚动承载 |
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | 探索链消费面 | 会话域参数(source/sourceRef/会话引用)语义等价切换,行为不回退 |
| Rust 会话域命令面(详见 desktop-agent-execution) | stop 与提前 resolve | 终止命令按运行中的会话活动轮寻址;发起命令提前 resolve;`stopped` 收敛;终态经 Channel 同构部件流出 |
