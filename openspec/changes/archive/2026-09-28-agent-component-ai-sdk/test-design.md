# 测试设计: agent-component-ai-sdk

> **日期**: 2026-09-28（design 回溯修订版重写；取代首轮 test-design）

---

## 验收范围

<!-- 框架识别结论（test_detect_frameworks）：packages/desktop/src 下 TS/TSX 全部为 vite-plus（@testing-library/react + jsdom，既有共置 *.test.ts(x) 模式）；Rust 侧文件级判 unknown，套件注册在 packages/desktop/src-tauri 工作区根（cargo 套件，共置 *_test.rs 模块，workspace 级自动覆盖）；packages/desktop/package.json 为 manifest 判 unknown（归不可测试项）。测试文件存在性现状：Rust 侧与既有视图套件（use-explore-session / agent-debug-view / agent-run-history / agent-raw-stream / app）首轮已落地在库，新基建 TS 测试文件（agent-adapter / agent-transport / use-agent-chat / components/agent 下各件）待 test-gen 生成。所有用例共置，不设独立组合测试区。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 适配层 `eventsToUIMessages` / `eventToChunk`：单测覆盖五变体映射、toolUse/toolResult 同 id 配对成单 tool 部件、seq 与 `parentToolUseId` 保真可考、未知事件 `data-raw` 透传不丢 | `packages/desktop/src/lib/agent-adapter.test.ts` |
| AC-2 | `TauriAgentTransport`：假 Channel/invoke 测试：body 链参数原样透传 `agent_start`、事件逐条转 chunk、终态部件 + finish 后流关闭 | `packages/desktop/src/lib/agent-transport.test.ts` |
| AC-3 | `use-agent-chat`：测试：重放装载重建的 UIMessage 与实时形状一致；发送携带链尾 `resumeSessionId` / `parentRunId`；运行中重复发送忽略；`stop` 触达 `agent_stop` | `packages/desktop/src/hooks/use-agent-chat.test.ts` |
| AC-4 | `components/agent/` 组件族：组件测试：对话透镜四变体渲染、保真透镜 seq 序与子代理归因分组、composer 运行中禁发且停止入口可用 | `packages/desktop/src/components/agent/agent-messages.test.tsx`、`agent-timeline.test.tsx`、`agent-input.test.tsx`、`tool-cards.test.tsx` |
| AC-5 | explore 页接线：既有会话测试迁移通过：重开双恢复、stance 拼接、链尾续话、四变体气泡呈现与迁移前一致 | `packages/desktop/src/views/explores/hooks/use-explore-session.test.ts`（组合入口）、`packages/desktop/src/components/agent/agent-messages.test.tsx`（四变体气泡基线） |
| AC-6 | debug 页接线：loop 可见性场景保持（时间线经 `AgentTimeline`）；运行中呈现停止入口且触发后事件流与状态收敛 | `packages/desktop/src/views/agent/agent-debug-view.test.tsx`（组合入口）、`packages/desktop/src/components/agent/agent-timeline.test.tsx`、`packages/desktop/src/views/agent/components/agent-run-history.test.tsx` |
| AC-7 | Rust `agent_stop` + 提前 resolve：`agent_start` 落库后 resolve running 记录；`agent_stop` 收敛 `stopped` 并经 Channel 流出终态部件；对非 running run 幂等 | `packages/desktop/src-tauri/src/commands/exec/agent_test.rs`、`packages/desktop/src-tauri/src/commands/exec/mod_test.rs`、`packages/desktop/src-tauri/crates/core/agent/src/state_test.rs`、`packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/runner_test.rs` |
| AC-8 | ai-sdk 依赖收口：`ai` 锚定 v7；视图层（`views/`）与 Rust/store 无 ai-sdk import 或概念，仅基建层（适配层 / transport / 组件族 / hook）持有 | —（见不可测试项 1） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust 共置 *_test.rs 模块、前端 vite-plus 纯函数/hook/组件 mock 测试）。
  每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节，该文件的所有测试用例和 Mock 策略均在对应章节内集中描述。
  跨模块组合用例挂靠链路入口模块（链路发起方 / 最上层调用方）的 `#### 用例` 表，describe 标题写链路方向；不设独立集成测试章节。
  回溯修订（design 第 2 版）对测试设计的落点：① AgentMessages 渲染入口过滤 system 消息（首轮「辅助行 / record 行」断言废弃）；② AgentTimeline 接入 message-scroller 区域滚动；③ 壳层 flex 高度链（app.tsx + 两场景根节点）与流视图区 / 重放区滚动容器为新增断言面。Rust 契约回溯零触碰，首轮用例保持全绿。 -->

### packages/desktop/src/lib/agent-adapter.ts -> packages/desktop/src/lib/agent-adapter.test.ts

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src/lib/agent-adapter.ts` 的行。 -->

- eventsToUIMessages(): 事件序列折叠重建为 UIMessage 序列；toolUse/toolResult 同 id 配对就地收敛
- eventToChunk(): 单事件产出 chunk 组（实时增量，多部件事件产出消息级 start/部件/收尾组）
- runRecordToUIMessage(): run 记录 → `data-run-record` 部件消息（重放与实时两路共用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| eventsToUIMessages：五变体映射 | 正向 | runStarted→`data-run-started`、message.text→text 部件、message.thinking→reasoning 部件、message.toolUse→tool 部件（toolCallId=block.id）、systemNotice→`data-system-notice`、runResult→`data-run-result`、raw→`data-raw`，各变体载荷字段（model/sessionId/tools/mcpServers、subtype/usage 等）保真进入 data 部件 | 新增 |
| eventsToUIMessages：消息形状约定 | 正向 | 每个 message 事件产出一个 UIMessage（id=`evt-${seq}`、role 收窄 user/assistant、metadata 携带 seq 与 `parentToolUseId`）；非 message 事件各产出一条 system 角色消息携带单个 data 部件（seq 保序）——system 载体消息照产（回溯注记：状态模型不动，呈现过滤归透镜层） | 新增 |
| eventsToUIMessages：工具同 id 配对 | 正向 | toolUse 块与跨消息同 id toolResult 合并为单个 tool 部件（input + output-available 形态）；重复 id 的 result 取首个，不重复续 output | 新增 |
| eventsToUIMessages：无主 toolResult | 边界 | 无配对 toolUse 的 toolResult 就地合成 tool 部件（name 占位「工具调用」），result 数据不丢 | 新增 |
| eventsToUIMessages：空输入 | 边界 | 空事件数组 → 空消息序列；blocks 为空数组的 message 事件仍产出无部件的 UIMessage（消息不丢） | 新增 |
| eventsToUIMessages：未知事件透传 | 边界 | raw 变体（未知 event_type）→ `data-raw` 部件原文透传，原文 JSON 不被解释改写 | 新增 |
| runRecordToUIMessage：终态同构 | 正向 | 任意 `AgentRunRecord` → 单条消息携带 `data-run-record` 部件（data 即 record 整行），metadata.seq 为 null | 新增 |
| runRecordToUIMessage：终态字段保真 | 边界 | completed / failed / stopped 三态 record 的 status、error、finishedAt、sessionId 均可在 record 部件 data 中考证 | 新增 |
| eventToChunk：两路同构不变量 | 正向 | 同一事件序列分别经 `eventsToUIMessages` 折叠与经 `eventToChunk` 逐条流入 useChat reducer 归约，终态 UIMessage 序列形状一致（AC-3 同构硬不变量的锁定点） | 新增 |
| eventToChunk：无主 result 增量 | 边界 | 实时到达的无主 toolResult 产出就地合成 tool 部件的 chunk 组，与折叠路径语义一致 | 新增 |
| eventToChunk：空与未知 | 边界 | 空 blocks 事件产出不含部件的 chunk 组；raw 事件产出 `data-raw` chunk，不抛错 | 新增 |

#### Mock策略

<!-- 纯函数模块，无外部依赖（`ai` 仅类型引用），不需要 Mock。 -->

### packages/desktop/src/lib/agent-transport.ts -> packages/desktop/src/lib/agent-transport.test.ts

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src/lib/agent-transport.ts` 的行。 -->

- TauriAgentTransport.constructor(): 注入 `options.onEvent` / `options.onRecord` 透传观测点
- TauriAgentTransport.sendMessages(): body 链参数 + 末条用户文本 → `invoke("agent_start")`；Channel 消息（Event/Record）逐条转 chunk；Record → record 部件 + finish + 关流
- TauriAgentTransport.reconnectToStream(): 恒 `null`（重连由重放承担）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| sendMessages：body 穿透 | 正向 | `options.body` 中的链参数（root/prompt/permissionMode/resumeSessionId/source/sourceRef/parentRunId）与末条用户文本原样进入 `invoke("agent_start")` 参数，transport 不增删改写任何字段 | 新增 |
| sendMessages：事件转 chunk | 正向 | Channel 收到 `ipc:"event"` 信封逐条经 `eventToChunk` 转 chunk 流出，顺序保序不重排 | 新增 |
| sendMessages：Record 收流 | 正向 | Channel 收到 `ipc:"record"` 信封 → `data-run-record` 部件 chunk + finish chunk 后可读流关闭（controller.close），后续无 chunk | 新增 |
| constructor：观测点透传 | 正向 | 注入 `onEvent`/`onRecord` 后逐条透传（含 record）；默认构造（无 options）不崩、不透传 | 新增 |
| sendMessages：启动失败 | 异常 | `invoke("agent_start")` reject → sendMessages promise 以错误 reject，不产出任何 chunk | 新增 |
| sendMessages：空流即终 | 边界 | Channel 无 Event 直达 Record → 仅 record 部件 + finish 关流 | 新增 |
| sendMessages：未知信封判别 | 边界 | `ipc` 判别值既非 event 也非 record 的消息被忽略，流不断开 | 新增 |
| reconnectToStream | 边界 | 任意 chatId 调用恒返回 `null` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（invoke / Channel） | `vi.mock` 替换为可编程 mock：invoke 按命令名分发并可切换 resolve/reject；Channel mock 为可编程 class（捕获 onmessage，测试中直接向其投递 `AgentRunMessage` 信封）——沿 use-explore-session.test.ts 既有进程边界 mock 模式 | sendMessages 全部用例（进程边界，唯一 mock 点；ReadableStream 与 chunk 消费用真实实现） |

### packages/desktop/src/hooks/use-agent-chat.ts -> packages/desktop/src/hooks/use-agent-chat.test.ts

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src/hooks/use-agent-chat.ts` 的行。 -->

- useAgentChat(): headless 会话基建 hook（重放装载 / 发送组装 / stop → agent_stop / `messages`、`events`、`chain`、`currentRunId` 镜像）

#### 用例

<!-- 本文件是「事件流 → transport → useChat 状态」基建链路与「重放查询 → 适配 → 装载」链路的入口模块，链路方向组合用例挂靠此处；适配层与 transport 以真实实现参与。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useAgentChat：重放装载 | 正向 | 挂载发起 `agent_run_chain`（source 二元组）+ 逐 run `agent_run_events`，经适配层重建 `setMessages`；`chain` / `events` / `currentRunId` 链镜像与查询结果一致 | 新增 |
| 重放装载 → useChat 状态（真实适配层 + transport 组合） | 正向 | 链路方向用例：重放装载重建的 messages 与「同批事件经 transport 实时流入 useChat」的终态形状一致（AC-3 同构不变量经真实组合复核，不只靠适配层纯函数对拍） | 新增 |
| useAgentChat：发送组装 | 正向 | sendMessage 经 body 携带链尾**非 running** 记录的 `resumeSessionId` / `parentRunId`，source/sourceRef/root 透传 `agent_start` | 新增 |
| useAgentChat：stop 接线 | 正向 | `stop()` 调 `invoke("agent_stop", { runId: currentRunId })` 且**不**调 `chat.stop()` 截断前端流；Record 部件回流后 running 转否、composer 语义复位 | 新增 |
| useAgentChat：运行中禁发 | 异常 | 状态 submitted/streaming 期间重复 sendMessage 被忽略，不并发第二条 run | 新增 |
| useAgentChat：启动失败 | 异常 | `agent_start` reject → error 置位、running 复位、既有 messages 不丢 | 新增 |
| useAgentChat：链尾 running 过滤 | 边界 | 链尾记录为 running（stop 后终态 record 未达）→ 发送取上一条非 running 记录的链尾参数（防御竞态） | 新增 |
| useAgentChat：空链首发 | 边界 | 空链时 resume/parent 参数不携带（null），来源三元组仍齐全 | 新增 |
| useAgentChat：null 守卫 | 边界 | root 为 null 或 sourceRef 为 null（debug 场景）时按入参约定降级：不发起或以 null 穿透，不崩 | 新增 |
| useAgentChat：reset | 边界 | `reset()` 清空 messages/events/chain/currentRunId/error（debug 页每跑重置场景） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（invoke / Channel） | `vi.mock` 可编程 mock（按命令名分发 agent_run_chain / agent_run_events / agent_start / agent_stop，可切换 resolve/reject；Channel class 捕获 onmessage） | 全部用例（进程边界；适配层与 transport 以真实实现参与，不 mock `ai` 的 useChat） |

### packages/desktop/src/components/agent/agent-messages.tsx -> packages/desktop/src/components/agent/agent-messages.test.tsx

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src/components/agent/agent-messages.tsx` 的行（回溯修订语义）。 -->

- AgentMessages(): 对话透镜——**渲染入口过滤 `role === 'system'` 消息**，仅呈现 user / assistant 气泡（text / reasoning / tool 部件 + AskUserQuestion 静态卡）+ message-scroller 组合 + loading/running/empty 态（empty 判定基于过滤后列表）

#### 用例

<!-- 回溯修订重写本章节：首轮的「辅助行与 record 行」用例（data-run-started / data-system-notice 辅助行与 data-run-record 状态行经 event-info / event-run-record 在对话透镜呈现）被回溯需求 1 推翻——辅助行 / raw / record 只在保真透镜（AgentTimeline）呈现，旧断言以废弃行保留防复辟。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentMessages：四变体气泡 | 正向 | user 文本、assistant 文本、reasoning（思考折叠）、tool 对卡四类部件分别呈现；AskUserQuestion 经注册表呈现静态卡；既有 data-testid 保留（随迁 explore-conversation.test.tsx 既有用例基线，呈现一致基线 = 四变体气泡） | 新增 |
| AgentMessages：system 消息过滤 | 正向 | 入参含 `role === 'system'` 载体消息（data-run-started / data-system-notice / data-raw / data-run-record 部件）→ 整条不进渲染列表（无 event-info / event-raw / event-run-record 节点），user / assistant 气泡不受影响；过滤不回写入参数组 | 新增 |
| AgentMessages：容器组合 | 正向 | message-scroller 组合与滚动容器结构与迁移前一致（AC-5 呈现一致的载体） | 新增 |
| AgentMessages：辅助行与 record 行 | 异常 | （首轮断言，已推翻）对话透镜呈现 `data-run-started` / `data-system-notice` 辅助行与 `data-run-record` 状态行——回溯需求 1 收窄为仅保真透镜呈现，本用例由「system 消息过滤」反向断言取代 | 废弃 |
| AgentMessages：空态 | 边界 | messages 为空、或全为 system 载体消息（过滤后为空）→ empty 态呈现，不抛错（判定基于过滤后列表） | 新增 |
| AgentMessages：运行中态 | 边界 | running 且无终态 record → loading/running 态呈现 | 新增 |

#### Mock策略

<!-- 纯渲染组件，消费方注入 AgentUIMessage[]（由适配层真实输出构造 fixture），无进程边界，不需要 Mock。 -->

### packages/desktop/src/components/agent/agent-timeline.tsx -> packages/desktop/src/components/agent/agent-timeline.test.tsx

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src/components/agent/agent-timeline.tsx` 的行（回溯修订语义）。 -->

- AgentTimeline(): 保真透镜——seq 序、子代理按 `parentToolUseId` 归因分组、raw 原文透传、result 汇总可复制；**全量呈现（含 system 载体消息）**；回溯补齐 flex 填充 + message-scroller 内部滚动（流式跟随 + 到底按钮）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentTimeline：seq 序 | 正向 | 乱序输入按 metadata.seq 升序呈现；同 seq 保输入序（随迁 agent-event-timeline.test.tsx 既有用例基线） | 新增 |
| AgentTimeline：子代理归因分组 | 正向 | 携带 `parentToolUseId` 的消息归入对应子代理分组呈现，归因头与组内顺序正确；`MessageScrollerItem` 以 top-level 消息为单位，嵌套组随父项滚动 | 新增 |
| AgentTimeline：raw 透传与可复制 | 正向 | `data-raw` 原文透传呈现；`data-run-result` 汇总卡内容可复制（CopyValue 语义） | 新增 |
| AgentTimeline：全量呈现含 system | 正向 | 入参含 system 载体消息（含 `data-run-record` 终态）→ 全量呈现（run-started / system-notice / raw / record 部件均可见），与对话透镜过滤口径互补（AC-6 loop 可见性的数据面） | 新增 |
| AgentTimeline：区域滚动组合 | 边界 | section 呈 flex 填充（`flex min-h-0 flex-1 flex-col`）且内部为 message-scroller 组合（Provider autoScroll + 视口 + 到底按钮在场）；历史重放嵌入态（普通块容器内）同组件无 props 分叉——jsdom 以结构 / className 为代理断言 | 新增 |
| AgentTimeline：空态 | 边界 | 空输入 → 空态呈现，不抛错 | 新增 |

#### Mock策略

<!-- 纯渲染组件，无进程边界，不需要 Mock。 -->

### packages/desktop/src/components/agent/agent-input.tsx -> packages/desktop/src/components/agent/agent-input.test.tsx

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src/components/agent/agent-input.tsx` 的行。 -->

- AgentInput(): composer——输入 / permission-mode 档位 / 发送（运行中禁发）/ 停止入口；id 与 testid 经 `idPrefix` 前缀化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentInput：发送 | 正向 | 输入文本后点击发送，回调携带 `{ text, permissionMode }`，输入清空 | 新增 |
| AgentInput：档位 | 正向 | permission-mode 三档切换生效，默认档 bypassPermissions（沿 explore-composer 的 ModeSelect 语义） | 新增 |
| AgentInput：停止入口 | 正向 | 运行中呈现停止入口，点击触发 onStop 回调 | 新增 |
| AgentInput：运行中禁发 | 异常 | running 时发送按钮禁用，回调不触发 | 新增 |
| AgentInput：空文本 | 边界 | 空文本（含纯空白）时发送禁用 | 新增 |
| AgentInput：testid 前缀化 | 边界 | 经 `idPrefix` 注入的前缀（如 explore-*）应用到输入、档位下拉与按钮的 id / data-testid | 新增 |

#### Mock策略

<!-- 纯渲染组件，回调经 props 注入，无进程边界，不需要 Mock。 -->

### packages/desktop/src/components/agent/tool-cards.tsx -> packages/desktop/src/components/agent/tool-cards.test.tsx

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src/components/agent/tool-cards.tsx` 的行。 -->

- DEFAULT_TOOL_CARDS(): 默认工具卡注册表（AskUserQuestion → 静态卡；可插拔）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| DEFAULT_TOOL_CARDS：AskUserQuestion 特化 | 正向 | toolName 为 AskUserQuestion 时命中注册表渲染静态卡（input 的问题/选项结构化呈现） | 新增 |
| DEFAULT_TOOL_CARDS：未注册回退 | 边界 | 未注册的 toolName 查找返回回退（渲染或 null，由消费方落默认工具对卡），不抛错 | 新增 |
| DEFAULT_TOOL_CARDS：错误结果 | 边界 | isError 的工具结果以错误形态呈现 | 新增 |

#### Mock策略

<!-- 纯渲染注册表，无进程边界，不需要 Mock。 -->

### packages/desktop/src/components/agent/agent-blocks.tsx -> packages/desktop/src/components/agent/agent-blocks.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（共享块渲染件 TextBlock/ThinkingBlock/ToolPairCard/ResultCard/CopyValue/record 状态行为内部实现，公共面仅经双透镜消费）。 -->

#### 用例

<!-- 不设独立用例：块渲染行为经 AgentMessages / AgentTimeline 两透镜的用例覆盖（含 data-testid 保留断言）。不应创建该测试文件，避免零断言套件。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src/components/agent/index.tsx -> packages/desktop/src/components/agent/index.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（纯导出桶：扩充 AgentMessages / AgentTimeline / AgentInput / 工具卡注册表导出面，无运行时行为）。 -->

#### 用例

<!-- 不设用例：导出面正确性由消费方（视图层）导入与 knip 死代码报告在编译期/守线承接。不应创建该测试文件（纯 re-export 模块，零用例套件会红）。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src/types/dto.ts -> packages/desktop/src/types/dto.test.ts

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅 `AgentRunStatus` union 增 `'stopped'` 的纯类型扩展，零运行时行为）。 -->

#### 用例

<!-- 不设用例：不建该测试文件（纯类型模块）。类型正确性由消费方（transport 适配、record 状态行、agent-run-history statusLabel）的用例在编译期覆盖。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src/views/explores/hooks/use-explore-session.ts -> packages/desktop/src/views/explores/hooks/use-explore-session.test.ts

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（内部改走 use-agent-chat：stance 拼接留来源侧、增 stop 委托、删本地 Channel/invoke/链还原实现；对外状态语义不变）。 -->

#### 用例

<!-- 本文件是「use-explore-session → use-agent-chat 基建接线」链路的入口模块，组合用例挂靠此处。既有套件（首轮已随迁改造并落地在库，行为基线不回归）：双恢复（链还原 + 逐 run 重放）、stance 拼接、链尾续话与链尾推进、失败语义、root/record null 守卫、记录切换竞态取消抑制——迭代类型均为新增（相对变更基线），保持全绿即可。回溯注记：对话区呈现一致基线 = 四变体气泡，辅助行断言不再适用（hook 套件本无 DOM 断言，不受影响）。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useExploreSession → 基建接线：双恢复重放 | 正向 | record 有既有链：挂载发起 `agent_run_chain`（来源二元组）恰一次 + 逐 run `agent_run_events` 重放，`chain` / `events` 镜像与查询一致（既有用例随迁） | 新增 |
| useExploreSession → 基建接线：stance 拼接与链尾续话 | 正向 | send 以 stance 前导开头、resumeSessionId / parentRunId 取链尾非 running 记录、三元组与 root/permissionMode 齐全；run 终态后链尾推进、二次 send 以新链尾续话（既有用例随迁） | 新增 |
| useExploreSession → 基建接线：stop 透传 | 正向 | 运行中 `stop()` → `invoke("agent_stop", { runId })` 触达后端；前端流不截断（Channel 仍可投递事件 / 终态 record），running 经 Record 回流复位 | 新增 |
| useExploreSession → 基建接线：运行中禁发与失败语义 | 异常 | 运行中重复 send 被抑制；`agent_start` reject → error 置位、running 复位、既有事件不丢（既有用例随迁） | 新增 |
| useExploreSession → 基建接线：守卫与竞态 | 边界 | root null / record null 时 send 为 no-op 不崩；链中 run 无 sessionId 时不携带 resume 而非传空串；旧记录迟到 resolve/reject 不污染新链（既有用例随迁） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（invoke / Channel） | 沿既有套件进程边界 mock：invoke 按命令名分发（agent_run_chain / agent_run_events / agent_start / agent_stop）可切换 resolve/reject；Channel 可编程 class 捕获 onmessage | 全部用例；use-agent-chat、适配层、transport、buildExplorePrompt 以真实实现参与 |

### packages/desktop/src/views/explores/explore-detail-view.tsx -> packages/desktop/src/views/explores/explore-detail-view.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（页面级组合：对话区换 AgentMessages、composer 停止入口接线；回溯修订根节点 `flex-1` 接入壳层滚动框架；双栏/预览/watch 生命周期不变）。 -->

#### 用例

<!-- 不建该测试文件（解析出的测试路径当前不存在，避免零用例套件）：页面变更面由 use-explore-session.test.ts（组合入口）与 agent-messages.test.tsx（四变体气泡基线 + system 过滤）承载；根节点 flex-1 属单类名接入，其高度链由 app.test.tsx 壳层框架断言与 AgentMessages scroller 组合断言两端夹持（见不可测试项 9）；未变更的双栏/预览/watch 行为不引入新套件。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src/views/explores/components/explore-composer.tsx -> packages/desktop/src/views/explores/components/explore-composer.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（薄适配：保留探索页 placeholder / label / explore-* testid 前缀，渲染与档位逻辑收编 AgentInput）。 -->

#### 用例

<!-- 不建该测试文件（解析出的测试路径当前不存在）：composer 行为（档位/禁发/停止/前缀）由 agent-input.test.tsx 承载（含 explore-* 前缀注入用例），见单元测试 AgentInput 章节。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src/views/agent/agent-debug-view.tsx -> packages/desktop/src/views/agent/agent-debug-view.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（页面接线：改走 use-agent-chat、时间线换 AgentTimeline、raw 流改消费 events 镜像、增停止入口 chrome；回溯修订根节点改 flex 填充、流视图区 flex-1；run 表单/历史列表/JSONL 开关外圈保留）。 -->

#### 用例

<!-- 本文件是「agent-debug-view → AgentTimeline → 停止收敛」链路的入口模块，组合用例挂靠此处。既有套件（首轮已随迁落地在库）：页面骨架四区块、JSONL 切换二选一、root null 提示、running 中表单禁用、非运行态无停止入口、error 横幅、终态汇总——断言面保持，驱动方式按本节 Mock策略修订随迁：首轮 vi.mock 双 hook + 直构 fixture 注入的隔离方案废弃（use-agent-chat / use-agent-run-history 为内部协作者不 mock），fixture 改经 mock invoke / Channel 信封流入真实 useAgentChat（eventsToUIMessages / runRecordToUIMessage 不再在套件内直构），待 test-gen 改造落地。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentDebugView → AgentTimeline 接线 | 正向 | 时间线区经 `AgentTimeline` 呈现（agent-timeline testid）；终态后事件重放 + record 部件在保真透镜呈现（event-result / event-run-record 可见）——loop 可见性场景保持（既有用例随迁） | 新增 |
| AgentDebugView：运行中停止入口 | 正向 | running=true → chrome 停止按钮（run-stop testid）在场，点击调 `stop` 恰一次；非运行态不在场（既有反向用例随迁） | 新增 |
| AgentDebugView：停止后收敛呈现 | 正向 | stop 后终态 record 部件回流 fixture → event-run-record 经 AgentTimeline 呈现、running 复位、表单恢复可用（AC-6 触发后事件流与状态收敛的呈现半边；链路半边归 use-agent-chat 停止接线用例） | 新增 |
| AgentDebugView：raw 流数据源 | 边界 | JSONL 切换二选一；AgentRawStream 消费 chat 的 `events` 镜像逐事件 dump（既有用例随迁） | 新增 |
| AgentDebugView：区域滚动框架 | 边界 | 根节点呈 `flex min-h-0 flex-1 flex-col`、流视图区（Timeline / RawStream 条件二选一）`flex-1` 填充——className 结构断言（jsdom 代理，见不可测试项 4） | 新增 |

#### Mock策略

<!-- 修订（最小 mock 原则）：use-agent-chat / use-agent-run-history 为进程内内部协作者（经 import 组合、非进程边界亦非注入入参），不 mock；唯一 mock 点为 @tauri-apps/api/core 进程边界，沿 use-explore-session 章节同款模式。 -->

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（invoke / Channel） | 唯一 mock 点（进程边界），沿 use-explore-session.test.ts 既有模式：invoke 按命令名分发（agent_start / agent_stop / agent_runs / agent_run_events）可切换 resolve/reject、记录入参供断言；Channel 可编程 class 捕获 onmessage（测试直接投递 Event/Record 信封驱动实时流与终态回流） | 全部用例；useAgentChat、useAgentRunHistory、适配层、transport、useChat 以真实实现参与（hook 自身行为归各自专属套件，页面接线经真实组合验证） |

### packages/desktop/src/views/agent/components/agent-run-history.tsx -> packages/desktop/src/views/agent/components/agent-run-history.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（重放区组件级联替换 AgentEventTimeline → AgentTimeline（`eventsToUIMessages` 转换）；statusLabel 增 stopped「已停止」；回溯修订 replay-area 限高 `max-h-96 overflow-y-auto`）。 -->

#### 用例

<!-- 既有套件（首轮已随迁落地在库，保持全绿）：run 列表三要素与错误行、openRun/refresh 触发、重放区还原时间线（与实时流同组件）、空态 / loading / error。被取代断言：首轮前对 AgentEventTimeline 的渲染断言已随组件删除废弃，重放区断言改经 AgentTimeline。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentRunHistory：statusLabel stopped | 正向 | status 为 `stopped` 的 run 行状态标签呈「已停止」，其余三态标签不回归（既有 statusLabel 用例组内补行） | 新增 |
| AgentRunHistory：重放区限高滚动 | 边界 | replay-area 限高容器呈 `max-h-96 overflow-y-auto`（重放区不再撑高页面）；内嵌 AgentTimeline 在普通块容器内自然高、无 props 分叉——className 结构断言 | 新增 |

#### Mock策略

<!-- 沿既有套件 mock 方案（AgentRunHistoryState fixture 直传 props），无新增进程边界。 -->

### packages/desktop/src/views/agent/components/agent-raw-stream.tsx -> packages/desktop/src/views/agent/components/agent-raw-stream.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（回溯修订：section 改 `flex min-h-0 flex-1 flex-col`；事件 `<pre>` 列表包 `min-h-0 flex-1 overflow-y-auto` 滚动容器。逐事件 JSON dump 行为不变）。 -->

#### 用例

<!-- 既有套件（首轮落地在库，保持全绿）：逐事件 camelCase dump、raw 原文不二次转义、空态、中文/emoji/换行保真、300 条大列表全量在场。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentRawStream：滚动容器结构 | 边界 | section 呈 `flex min-h-0 flex-1 flex-col`，事件列表包 `min-h-0 flex-1 overflow-y-auto` 滚动容器（debug 流视图二选一的另一半不再撑高页面）——className 结构断言（jsdom 代理，见不可测试项 4） | 新增 |

#### Mock策略

<!-- 纯 props 渲染，无进程边界，不需要 Mock。 -->

### packages/desktop/src/app.tsx -> packages/desktop/src/app.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（回溯新增修改：路由内容包裹层增 `flex min-h-0 flex-col` 立壳层滚动框架；其余路由页为内容自适应 flex item，呈现不变）。 -->

#### 用例

<!-- 本文件是「壳层高度链 → 场景根 → 消息区」滚动框架链路的入口模块。既有套件（壳层布局 / 路由切换 / 欢迎屏 / 错误双轨等）保持全绿即为「其余路由页呈现不变」守线。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| App：壳层滚动框架 | 正向 | 壳态路由内容包裹层（既有 `mx-auto w-full flex-1 px-4 py-4` 容器）增 `flex min-h-0 flex-col` 类（高度链起点）——沿既有套件 jsdom className 拓扑断言模式 | 新增 |
| App：非滚动场景页不受框架约束 | 边界 | changes / db 等路由页根为内容自适应 flex item：切页往返既有布局断言（清单/详情/数据库页呈现）不回归 | 新增 |

#### Mock策略

<!-- 沿既有 app.test.tsx mock 方案：@tauri-apps/api（invoke/app）、plugin-dialog、plugin-updater 进程边界 mock；sonner 不 mock。 -->

### packages/desktop/src-tauri/crates/core/agent/src/state.rs -> packages/desktop/src-tauri/crates/core/agent/src/state_test.rs

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = crates/core/agent/src/state.rs` 的行。Rust 契约回溯零触碰；`AgentRunState::Stopped` 变体与 `stop()` 已在首轮落地，state_test.rs 尚无 stop 用例组，待 test-gen 补齐。 -->

- RunStateMachine.stop(): 显式终止收敛 Running → Stopped；已收敛幂等原样返回

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| stop：收敛 Stopped | 正向 | Running 状态调用 stop() 返回 Stopped，current 观测一致 | 新增 |
| stop：终态不可改写 | 正向 | stopped 收敛后再 apply 任意事件（含 is_error 的 RunResult）终态保持 Stopped | 新增 |
| stop：已收敛幂等 | 边界 | 已 Completed / 已 Failed 状态调用 stop() 原样返回原终态（不改写） | 新增 |
| stop：连续调用 | 边界 | 连续两次 stop()，首个收敛生效、第二次原样返回 Stopped | 新增 |
| stop：与 RunResult 竞态 | 边界 | stop 先到 → Stopped 且后续 RunResult 不改写；RunResult 先到 → Completed/Failed 且后续 stop 不改写（首个收敛生效语义） | 新增 |

#### Mock策略

<!-- 纯状态机，无外部依赖，不需要 Mock（沿 state_test.rs 既有 fixture 风格）。 -->

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = crates/core/agent/src/runner.rs` 的行。RunHandle 填实已首轮落地，runner_test.rs 尚无句柄用例组，待 test-gen 补齐。 -->

- RunHandle.request_stop(): 置位终止请求并唤醒等待方（幂等）
- RunHandle.stop_requested(): 同步观测（编排侧 EOF 收敛判定）
- RunHandle.wait_requested(): 异步等待（租户泵 select 半边）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunHandle：置位与观测 | 正向 | request_stop() 后 stop_requested() 返回 true | 新增 |
| RunHandle：等待被唤醒 | 正向 | wait_requested() 在 request_stop() 调用后完成（Notify 唤醒） | 新增 |
| RunHandle：初始态 | 边界 | Default / 初始句柄 stop_requested() 为 false | 新增 |
| RunHandle：未请求时挂起 | 边界 | 未请求终止时 wait_requested() 挂起不完成（tokio time 超时控制下断言） | 新增 |
| RunHandle：请求幂等 | 边界 | 重复 request_stop() 不 panic、状态不变、等待方不二次异常唤醒 | 新增 |
| RunHandle：Clone 共享信号 | 边界 | Clone 出的句柄置位后原句柄 stop_requested() 可见（共享 AtomicBool 语义） | 新增 |

#### Mock策略

<!-- 无外部依赖；tokio sync（Notify/AtomicBool）与 tokio time 以真实实现参与，不需要 Mock。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/runner_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`discover` / `flags` / `jsonl` 为禁区不动）。被测对象为 crate 内 `pub(crate)` 泵缝 `pump_lines`（select 行流 vs 停止信号 + 注入击杀缝）——design「CLI 租户击杀泵」组件的行为面；`kill_process_tree` 为租户实现细节，经注入缝以调用参数断言。 -->

- pump_lines()（pub(crate) 泵缝）: 行流 vs 停止信号 select；信号到达即中止、调击杀缝、不合成 RunResult；未请求时 EOF 语义与首轮一致

#### 用例

<!-- 泵是「RunHandle 停止信号 → CLI 租户泵收敛」链路的承载模块：编排队列的停止全链（agent_test.rs 假 runner 端到端）走自研泵模拟，不经真实 pump_lines——本文件用例是真实泵停止分支的唯一自动化落点。既有 pump_lines 用例（完整会话逐行、seq 单调、EOF 合成 result、空流、洪峰背压、raw 透传、实例隔离）保持全绿不回归。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| pump_lines：停止信号中止 | 正向 | 预置句柄 request_stop() → select 分支命中：泵中止、击杀缝以捕获的 pid 被调恰一次、行流剩余行不再产出 | 新增 |
| pump_lines：停止路径不合成 result | 正向 | 停止中止后不合成 `error_process_exit` RunResult（给编排侧 stopped 显式收敛让路），已产出事件原样保留 | 新增 |
| pump_lines：无 pid 击杀缝容错 | 边界 | 捕获 pid 为 None（spawn 未达）时停止路径仍收敛、击杀缝以 None 被调不 panic（尽力语义） | 新增 |
| pump_lines：信号晚于 EOF | 边界 | 行流先 EOF 且 stop 未请求 → 既有 EOF 合成 result 语义不回归；stop 请求在 EOF 后才置位 → 泵已退出、无二次合成 | 新增 |
| pump_lines：EOF 语义不回归 | 边界 | 未请求停止的正常会话 / 空流 / 未知行透传等既有用例行为逐字不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 行流来源（真实 claude CLI 子进程 stdout） | 内存行流 / mpsc 注入（沿既有 runner_test.rs fixture 风格），EOF 时序可控 | 全部用例（进程边界） |
| 击杀缝（`taskkill /T` / `start_kill`） | 经 `pump_lines` 注入缝以捕获闭包替身：记录调用次数与 pid 入参，不触真实进程 | 停止路径用例（进程边界） |

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src-tauri/src/commands/exec/agent.rs` 的行（`run_agent` / `run_agent_with` 为删除项，不入待测范围）。编排拆分与首轮用例已落地在库，回溯零触碰，保持全绿。 -->

- start_agent_run(): 编排同步段——runner start → begin 落 running 行 → 注册停止句柄 → spawn 后台任务 → 返回 running 记录
- drive_agent_run(): 后台任务体——tee 双 sink → EOF 收敛（状态机已收敛以状态机为准；否则 stop 请求 → `stopped`；兜底 failed）→ finish 落库 → Channel 流出 Record → 注销注册表

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| start_agent_run：提前 resolve | 正向 | 返回 running 记录（id 非零、status=running、startedAt 落值）；store 已落 running 行；RunStopRegistry 已注册该 id 句柄；命令返回时后台任务尚未收敛也不阻塞调用方 | 新增 |
| start_agent_run：启动失败 | 异常 | 假 runner 返回启动错误 → Err(String) 且 store 零 run 行、registry 无残留（既有用例随迁改造） | 新增 |
| drive_agent_run：RunResult 收敛 | 正向 | RunResult（is_error=false）驱动 → completed 整行替换落库、事件全量落库、Channel 事件序列与落库逐条一致、收尾 Record 流出、registry 除名（既有 tee 用例随编排拆分改造） | 新增 |
| drive_agent_run：failed 收敛 | 正向 | is_error=true → failed 记录 Ok 返回（既有用例随迁） | 新增 |
| drive_agent_run：stopped 收敛 | 正向 | 预置 stop 请求置位 + 事件流 EOF 无 RunResult → 状态收敛 stopped、error 为 None、finished_at 落值、终态 Record 经 Channel 流出、registry 除名 | 新增 |
| drive_agent_run：状态机优先 | 边界 | EOF 时状态机已由 RunResult 收敛（completed/failed）→ 以状态机为准，stop 请求晚到不改写 | 新增 |
| drive_agent_run：兜底 failed | 边界 | 既无 RunResult 又无 stop 请求的 EOF（进程异常退出路径）→ 兜底收敛 failed 且 error 记因 | 新增 |
| drive_agent_run：store 写失败 | 异常 | 事件落库失败 → 收敛 failed、error 记因、终态行尽力落库且仍尽力流出 Record（既有 abort 助手用例随编排拆分改造） | 新增 |
| drive_agent_run：Channel 已关 | 边界 | Channel 接收端先行关闭 → 落库完整不中断、收敛与注销照常（既有用例随迁） | 新增 |
| drive_agent_run：seq 乱序透传 | 边界 | seq 缺口乱序 tee 透传不重排、落库 key 与事件自带 seq 一致（既有用例随迁） | 新增 |
| 编排 → 停止全链（start → 注册表 → 租户 → stopped 收敛） | 正向 | 链路方向用例：start 提前 resolve running → 经注册表 request_stop 触达租户 → 后台收敛 stopped → Record 流出 → 除名后停止寻址 miss（首轮已落地的 spawn 端到端用例，AC-7 全链锁定点） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `AgentRunner`（真实 claude CLI 进程） | 假 runner 注入泛型缝：预录事件经 mpsc 后台投递（可控制投递时机与 EOF 时序，含 `with_events_held_until_stop` 停止时机装置）、可注入启动失败、暴露 start 入参捕获——沿 agent_test.rs 既有 FakeRunner 装置 | drive_agent_run / start_agent_run / 全链用例 |
| `Channel<AgentRunMessage>`（IPC 出口） | `tauri::ipc::Channel::new` 捕获回调收集序列化 JSON（或返回 Err 构造「页面已关」） | 全部用例的 Channel 断言半边（进程边界） |
| store | 不 mock：tempdir 真库（redb）沿既有 temp_store 装置 | 全部用例的落库断言半边 |

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

<!-- 来源：design.md `### 公共函数 / API` 表中 `所在文件 = src-tauri/src/commands/exec/mod.rs` 的行。首轮用例已落地在库（含 agent_stop 命中 running 句柄用例），回溯零触碰。 -->

- agent_start(): 契约演进——begin 落库后即 resolve running 记录；`on_event` 参数升为 `Channel<AgentRunMessage>`；IPC 参数面不变
- agent_stop(): 按 id 寻址 request_stop；非 running / 不存在幂等 `Ok`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| agent_start：组装透传 | 正向 | 命令面把 root/prompt/permissionMode/resumeSessionId/source/sourceRef/parentRunId 原样组装进 runner 入参（命令体参数转换段经逐行镜像 + 泛型缝全链观测，沿既有装置；全参 / 缺省 / 仅 resume / 仅 source 各形态不串线） | 新增 |
| agent_start：隔离 PATH 失败 | 异常 | PATH 隔离走完整链 → Err 透传 CLI 缺失文案、store 无 run 行、Channel 零推送（既有用例随迁，PATH 锁串行化保持） | 新增 |
| agent_stop：不存在 id | 正向 | 对 registry 中不存在的 run_id 调用 → Ok（幂等），无副作用 | 新增 |
| agent_stop：已终态 id | 正向 | 对已收敛除名的 run_id 调用 → Ok（幂等） | 新增 |
| agent_stop：命中 running 句柄 | 正向 | registry 中登记的 running 句柄被置位停止信号（经 RunStopRegistry 触达 RunHandle.request_stop）；「命中 running 句柄后的收敛全链」由 agent_test.rs 编排全链用例承载 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| PATH 环境变量 | 隔离 PATH 至空目录 + 既有 PATH_LOCK 串行化（沿 mod_test 装置） | agent_start 失败路径用例 |
| 真实 claude CLI 进程 | 不引入；命令面成功路径的编排时序由 agent_test.rs 假 runner 泛型缝承载 | 全部用例 |

### packages/desktop/src-tauri/src/main.rs -> packages/desktop/src-tauri/src/main_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（纯声明式接线：invoke_handler 注册 agent_stop、app.manage(RunStopRegistry::default())）。 -->

#### 用例

<!-- 不建该测试文件（解析出的测试路径当前不存在，避免零用例套件）：命令注册正确性由编译期宏校验与命令自身用例承载；托管状态挂载由 agent_test.rs 编排用例覆盖。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅 `AgentRunRecord.status` 字段注释受控清单扩展 `stopped`，零行为、零 schema 迁移）。 -->

#### 用例

<!-- 不设新用例：注释级扩展无运行时行为可断言；`stopped` 终态的整行替换落库语义由 agent_test.rs 的 drive_agent_run stopped 收敛用例以真库承载。 -->

#### Mock策略

<!-- 不适用（无新行为用例）。 -->

---

## 不可测试项

<!-- 列出 proposal 范围内但无法通过自动化测试验证的条目，每项说明原因。 -->

- AC-8 ai-sdk 依赖收口（`ai` 锚定 v7；`views/` 与 Rust/store 无 ai-sdk import 或概念） — **原因**: 纯静态 import 边界约束，非运行时行为，无法表达为用例；由守线任务静态核对（`from 'ai'` 仅四类基建文件）与 knip / `client:check` 承接。版本锚定半边由 `packages/desktop/package.json` 依赖声明 + 安装锁定承接（manifest 文件，test_detect_frameworks 判 unknown）。
- 真实 claude CLI 进程树击杀端到端（Windows `taskkill /PID <cmd_pid> /T /F` 对真实进程树的连带击杀、`CREATE_NO_WINDOW` 窗口抑制） — **原因**: 需要真实 spawn 子进程树并验证 OS 级进程连带终止，进程内测试仅能以注入缝断言击杀调用参数（见 infra pump_lines 用例）；失败情形由 spec「进程树击杀失败留痕已知限制」承接（尽力语义）。
- 真实 Tauri IPC 跨进程传输与 GUI 生命周期（Channel 消息真实送达前端、AppHandle 托管状态下后台任务取 `state::<Store>()`、页面关闭后 Channel 失效的真实时序） — **原因**: 需要运行中的桌面应用进程；TS 侧以可编程 Channel mock、Rust 侧以捕获型 `Channel::new` 各自锁定半边契约，跨进程组合不可自动化验证。
- 区域滚动的实际视口滚动行为（消息区撑满窗口剩余区域、超出时内部滚动的真实观感） — **原因**: jsdom 无布局引擎，滚动位置与像素级 flex 分配不可断言；壳层高度链 / 场景根 / 滚动容器以 className 与 scroller 组合在场的结构断言为代理（app.test.tsx / agent-debug-view.test.tsx / agent-raw-stream.test.tsx / agent-run-history.test.tsx / agent-timeline.test.tsx），真实观感归人工验收。
- `packages/desktop/src/types/dto.ts`（`AgentRunStatus` union 增 `'stopped'`） — **原因**: 纯类型模块，编译期约束；不应创建 `dto.test.ts`（零用例套件会红），类型正确性由消费方（transport 适配、record 状态行、statusLabel）用例编译期覆盖。
- `packages/desktop/src/components/agent/index.tsx` 导出桶扩充 — **原因**: 纯 re-export 无运行时行为，不应创建 `index.test.tsx`；导出正确性由消费方导入编译期承载，导出面完整性由 knip 死代码报告守线。
- `packages/desktop/src-tauri/src/main.rs`（invoke_handler 注册 `agent_stop`、`app.manage(RunStopRegistry::default())`） — **原因**: 纯声明式接线，无分支逻辑；不应创建 `main_test.rs`，注册错误由编译期宏校验暴露，托管状态行为由 agent_test.rs 编排用例覆盖。
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs`（status 注释受控清单扩展） — **原因**: 仅 doc 注释扩展，零行为、零 schema 变更，无可断言对象；`stopped` 终态的落库语义由 agent_test.rs 以真库承载。
- explore-detail-view 页面组合不变量与 explore 侧滚动链根节点半边（双栏布局、文档预览、watch 生命周期不回归；根节点 `flex-1` 单类名接入） — **原因**: 属未变更行为 + 单类名接入的既有页面组合，仓库无该页面测试且变更面（对话区/停止入口/气泡基线）已由 use-explore-session.test.ts 与 agent-messages.test.tsx 承载，高度链由 app.test.tsx 壳层断言与 AgentMessages scroller 断言两端夹持；不为此新建页面套件（避免零价值快照式用例）。
