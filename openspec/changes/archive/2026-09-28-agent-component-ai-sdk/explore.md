# agent 通用会话组件 + ai-sdk 基建(探索笔记)

> 主题:packages/desktop 按 `components/agent/` 骨架的思路封装通用 agent 会话组件;
> 引入 ai-sdk(v7)处理发送、流式解析,对接 Tauri API。目标:今后所有 agent
> 会话(查看/聊天)统一走这套基建。

## 一、现状地图

```
┌──────────────────────────── Rust (src-tauri) ────────────────────────────┐
│  core/agent(中立契约)             infra/agent-cli(CLI 租户)             │
│  ├ AgentEvent 信封(五变体)        ├ spawn CLI:-p + stream-json --verbose │
│  ├ AgentRunner trait             ├ JSONL → 信封归一化                    │
│  └ RunStateMachine               └ 无 --include-partial-messages         │
│                                     (无增量帧,整帧到达)                  │
│  run_agent 编排 = tee 双 sink:                                           │
│    事件 → store 落库(redb,seq 排序) ⊕ Channel 实时流 → 前端              │
│  命令:agent_start / agent_run_chain / agent_run_events 等                │
└──────────────────────────────┬───────────────────────────────────────────┘
                     invoke('agent_start') + Channel<AgentEvent>
                               ▼
┌────────────────────────── Frontend(React)──────────────────────────────┐
│  useAgentRun(调试)               useExploreSession(探索会话)            │
│  └ invoke+Channel 累积            └ 同款累积 + 链还原重放 + 续话 resume  │
│         │                                 │                             │
│  AgentDebugView                  ExploreDetailView                      │
│  ├ AgentRunForm                  ├ ExploreConversation(367 行)          │
│  ├ AgentEventTimeline(300 行)    │   ├ toEntries / collectResults       │
│  ├ AgentRawStream                │   ├ ChatBubble / ChatBlocks          │
│  └ AgentRunHistory               │   └ AskUserQuestionCard / ToolPair…  │
│                                                                   │
│  components/agent/ ← 骨架(手写于此,尚未接线)                          │
│  ├ index.tsx(只导出 AgentMessages)                                  │
│  ├ agent-messages.tsx(空壳滚动容器,EntryView 注释掉)                 │
│  └ agent-input.tsx(空文件)                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

痛点:两份近似双胞胎渲染逻辑(`explore-conversation.tsx` 与
`agent-event-timeline.tsx` 各自实现 Thinking 折叠/工具卡/ResultCard/raw
透传/entry 分发,约 670 行大半语义重复);两个 hook 重复 invoke+Channel
累积模式。

## 二、承重事实

1. **信封是三租户中立模型**:`event.rs` 明言信封为 CLI / SDK / API 三租户
   共享,"非任何线上格式直译"。前端新增状态模型时不应破坏它。
2. **双 sink ⇒ 实时流与重放天然同构**:Channel 推的与 store 落的是同一
   `AgentEvent` 形态;重开恢复 = 重放。任何前端状态模型都要能从事件序列
   **重建**(reducer/适配器的天然形状)。
3. **没有增量流**:CLI 启动 flag 明确禁用 `--include-partial-messages`
   (flags_test 的 forbidden 清单),整帧 message 到达,文本按消息粒度渲染。
4. **toolUse/toolResult 配对目前是前端全局扫描**(`collectResults` 跨消息
   按 id 配对,重复 id 取首个);ai-sdk 的 tool part 模型(input/output 状态
   同住一个 part)原生就是这个语义。

## 三、决策记录

### D1. ai-sdk 角色 = 方案 A:前端状态层接管

- 自定义 `TauriAgentTransport implements ChatTransport`;
- `AgentEvent` 保持"线格式 = 落库格式 = DTO 镜像"三位一体不动;
- `UIMessage` 成为前端对话状态的一等模型,适配器是新的缝;
- 否决方案 B(Rust 直出 UIMessageChunk:三格式一体裂开 + TS-first schema
  的 Rust 镜像版本耦合);否决方案 C(组件层不依赖 ai-sdk:什么都没赚到)。

### D2. 流式粒度:本次不解决

整帧渲染保持;不加 `--include-partial-messages`,不动信封。token 级流式
留作未来独立决策。

### D3. 链模型经 sendMessage 额外参数穿透(v7 已核实)

v7 `ChatTransport`(源码 main 分支)签名:

```ts
sendMessages(options: {
  trigger: 'submit-message' | 'regenerate-message';
  chatId: string;
  messageId: string | undefined;
  messages: UI_MESSAGE[];
  abortSignal: AbortSignal | undefined;
} & ChatRequestOptions): Promise<ReadableStream<UIMessageChunk>>;
reconnectToStream(...): Promise<ReadableStream<UIMessageChunk> | null>;
```

`ChatRequestOptions.body` 即每次调用的额外 JSON 通道:

```
chat.sendMessage(text, { body: {
  root, permissionMode,
  resumeSessionId: 链尾?.sessionId ?? null,
  parentRunId: 链尾?.id ?? null,
  source, sourceRef,
}})
  → transport.sendMessages(options)     // body 随 options 穿透
  → new ReadableStream<UIMessageChunk>
      start(): invoke('agent_start', { ...body, prompt: 末条用户文本 })
               Channel.onmessage = e => enqueue(eventToChunk(e))
      runResult → finish chunk → close()
```

链状态归应用层(session hook 在发送时刻知道链尾);transport 是无状态
转换器,不进构造闭包、无 ref 插线。`reconnectToStream` 返回 null 即可
(重连诉求由重放路径承担)。

### D4. 复用范围:一套状态模型,双透镜;debug 页共用

- 今后**所有** agent 会话(查看/聊天)统一这套基建;
- 保真信息必须活在 UIMessage 模型里:`seq` 进 message id/部件元数据、
  `parentToolUseId` 进部件元数据(时间线子代理归因靠它)、
  raw/systemNotice/runStarted/runResult 进 `data-*` 部件;
- 展示层双透镜共享底层块渲染件;
- debug 页的 run 表单/历史列表/JSONL 开关是页面级 chrome,包在共享核心外圈。

```
      所有 agent 会话页面(explore 会话 / debug 调试 / 未来一切)
                        │
                        ▼
      use-agent-chat(headless 基建)
      ├ useChat(chatId=run 链键, transport=TauriAgentTransport)
      ├ 重放装载:agent_run_chain + agent_run_events
      │   → eventsToUIMessages()(纯函数)→ setMessages
      └ 发送组装:stance 拼接 + body 续话参数(应用关注点收口)
                        │  UIMessage[](保真进部件/元数据)
                        ▼
      components/agent/(展示层)
      ├ AgentMessages  对话透镜:气泡/工具对卡/思考折叠/AskUserQuestion 卡
      ├ AgentTimeline  保真透镜:seq 序/子代理归因分组/raw 原文透传
      ├ AgentInput     composer(输入态/permission-mode/发送/停止)
      └ 工具卡注册表   特化卡(AskUserQuestionCard 等)可插拔位
```

事件 → 部件映射(初稿,design 阶段细化):

| AgentEvent                | UIMessage 部件/chunk                          |
|---------------------------|-----------------------------------------------|
| runStarted                | `data-run-started`                           |
| message.blocks: text      | text 部件(整块,无 delta 拆分)              |
| message.blocks: thinking  | reasoning 部件                                |
| message.blocks: toolUse   | tool 部件 input(output 等同 id 配对续上)    |
| message.blocks: toolResult| tool 部件 output(配对即 collectResults 消失)|
| systemNotice              | `data-system-notice`                         |
| runResult                 | `data-run-result`(+ finish)                 |
| raw                       | `data-raw`(原文透传)                       |

### D5. 版本锚定:ai-sdk v7

适配器层同时是版本变动隔离带;design 阶段以当时 stable v7 精确 API 为准
(ChatTransport 签名已对 main 源码核实)。

### D6. 补 `agent_stop` 命令(本次范围含 Rust)

`useChat.stop()` 只能 abort 前端流;Rust 侧无停止命令则 CLI 跑完落库,
前端截流后与 store 偏差。**决策:本次补 `agent_stop`**,让停止真实生效。

派生的**寻址问题**(进行中前端拿不到 run id——id 在落库时分配,而
`agent_start` 到 run 结束才 resolve):

- 方案 a(命令契约重构):`agent_start` 落库后**提前 resolve** running
  record(id 立即可知),run 在后台任务继续,终态 record 改经 Channel 流
  出。顺带服务 D7(record 与事件同一条通道)。IPC 形状更干净,但
  `commands/exec` 编排要改结构(后台任务持有 Channel 与 store 收尾)。
- 方案 b(旁路寻址):保持 invoke 长驻,`agent_stop` 按
  `(root, source, sourceRef)` 或客户端生成 run token 寻址;需要进程注册表。

倾向 a(与 D7 同构),proposal/design 阶段定。

### D7. 终态 record 与进行中会话状态一致

`agent_start` resolve 返回的终态 `AgentRunRecord` 不走旁路,尽量转成与
实时事件同构的状态更新(如 `data-run-record` 部件附进流);重放路径从链
记录(`agent_run_chain` 的 run 行)合成同款部件。两路(实时查看/聊天重放)
收敛到同一状态形状;链推进(续话取新链尾)也从中取。

### D8. stop 收敛语义(待 design)

store 状态是受控字符串(running/completed/failed)。停止后收敛为
failed+error="用户停止",还是新增受控字符串 `stopped`;`RunStateMachine`
目前仅由 RunResult 驱动收敛,stop 路径需要显式收敛分支(类似 EOF 无
result 的兜底)。Windows 侧 cmd /C 包装的进程树击杀(taskkill /T 或等价)
需在 design 阶段核实。

## 四、开放设计问题(进 proposal/design 收敛)

1. agent_stop 寻址与 `agent_start` 契约重构(D6 方案 a/b)。
2. stop 收敛语义与进程树击杀(D8)。
3. `data-*` 部件命名空间与类型泛型形状(`useChat<UIMessage<...>>`)。
4. session hook 从状态取链尾的具体机制(onFinish vs 状态订阅)。
5. debug 页 chrome(run 表单/历史/JSONL 开关)保留现状还是吸进组件族。
6. 测试策略:适配层 `eventsToUIMessages` / `eventToChunk` 是纯函数,高可测;
   transport 用假 Channel/invoke 测;展示层沿现有 vitest 组件测试模式。
7. knip:新增导出注意"No test-only exports"规则,经公共 API 测。

## 五、相关 spec 能力

- `specs/desktop-agent-execution`(信封/runner/状态机/exec 轨道首命令)
- `specs/desktop-explore-page`(探索会话页)
- `specs/desktop-explore-queries`(explore 记录查询)
- 本基建落地后 explore 会话页与调试页接线,涉及上述能力的演进或新能力
  (如 desktop-agent-chat-infra)。
