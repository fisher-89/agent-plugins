# 设计: agent-component-ai-sdk

> **变更**: agent-component-ai-sdk
> **日期**: 2026-09-28（第 2 次尝试，回溯修订版）

---

## 回溯修订说明（本版差异）

本次为 implement 首轮通过后的回溯修订（回到 dev-design 层），`proposal.md` 与 `specs/` 未变。两条新增设计需求：

1. **对话消息不呈现 system 角色**：聊天消息列表渲染时，role 为 `system` 的消息（适配层为非 message 事件与终态 record 合成的载体消息）不应出现在对话呈现中。
2. **explore 与 debug 场景区域滚动**：两个场景当前消息区域未实现区域滚动，期望消息区域撑满窗口剩余区域、超出时在区域内部滚动，而非撑开页面。

| # | 主题 | 上一版 | 本版 |
|---|------|--------|------|
| 1 | 对话透镜呈现口径 | data 部件 → 辅助行 / 汇总卡 / raw / record 行（system 角色消息在对话透镜直出） | system 角色消息在 `AgentMessages` 渲染入口**整体过滤**；对话透镜仅呈现 user / assistant 气泡（text / reasoning / tool 部件 + AskUserQuestion 静态卡）；辅助行 / raw / record 只在保真透镜呈现 |
| 2 | `AgentMessages` 结构 | `MessageView` 含 system / onlyData 直出分支，`InfoRow` / `RawView` 挂组件内 | 渲染入口过滤 `role === 'system'`；删除 system / onlyData 分支与 `InfoRow` / `RawView`；empty / loading 判定基于过滤后列表 |
| 3 | `AgentTimeline` 滚动 | 纯堆叠 section（无滚动承载，内容撑高页面） | flex 填充 + message-scroller 组合内部滚动（流式跟随 + 到底按钮）——补齐 spec「双透镜滚动 SHALL 沿用 message-scroller」在保真透镜的落实 |
| 4 | 壳层高度链 | 未定义（`ExploreDetailView` 根 flex col 无高度来源；debug 根普通块级 div） | `app.tsx` 路由内容包裹层 flex 框架化（`flex min-h-0 flex-col`）；两场景根 `flex-1`；高度链逐级 `flex min-h-0` |
| 5 | debug 流区 / 重放区 | 自然高堆叠 | 流视图区（Timeline / RawStream 条件二选一）`flex-1` 填充 + 内部滚动；`replay-area` 限高内部滚动 |
| 6 | AC 对齐措辞 | AC-4 行含「辅助行」呈现表述 | 收窄为「user / assistant 气泡 + 工具卡」口径；AC-5 的呈现一致基线明确为**四变体气泡**（辅助行不呈现属本回溯显式收窄） |

**保持不变（首轮已落地、不回退）**：适配层 system 角色载体与两路同构不变量、transport / hook 契约、Rust 终止与提前 resolve 全链、双透镜分工、删除清理、ai 依赖收口纪律。

---

## 提案与规格同步状态

`proposal.md` 与 `specs/`（desktop-agent-chat-infra 新增、desktop-agent-execution / desktop-explore-page 演进）已由提案阶段写入并通过，本 design 不再将其列入变更清单与任务。

本回溯两条需求与 spec 相容，无需规格增量：desktop-agent-chat-infra 的对话透镜条目本就只列「user / assistant 气泡、工具对卡、思考折叠、AskUserQuestion 静态卡」（system 过滤与 spec 对齐，首轮实现超出该口径的部分即本轮收窄）；同 spec 滚动条款「双透镜 SHALL 沿用 message-scroller 组合」首轮仅对话透镜落实，本轮把保真透镜补齐。设计负责把 proposal「待决问题」逐项定稿（见「关键设计决策」），并给出实现文件级的落点。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 事件适配层 | `eventsToUIMessages`（重放重建）/ `eventToChunk`（实时增量）/ `runRecordToUIMessage`（终态同构部件）纯函数；toolUse/toolResult 同 id 配对收口（消灭视图层 `collectResults` 全局扫描）；`data-*` 部件命名空间、`AgentMessageMetadata`（seq、`parentToolUseId` 保真）定义 | `packages/desktop/src/lib/agent-adapter.ts`（新） | `types/dto`、`ai`（`UIMessage` / `UIMessageChunk` 类型） | TS 纯函数 + ai v7 类型 |
| Tauri 传输适配 | `TauriAgentTransport implements ChatTransport`：`sendMessages` 经 `options.body` 链参数穿透 `invoke("agent_start")`，Tauri `Channel` 消息（事件 / 终态记录）转 `UIMessageChunk` 流，终态 record 部件 + finish 后关流；`reconnectToStream` 恒 `null`（重连由重放承担）；`onEvent` / `onRecord` 透传观测点 | `packages/desktop/src/lib/agent-transport.ts`（新） | 适配层、`@tauri-apps/api/core`、`ai` | class implements ChatTransport + ReadableStream |
| headless 会话 hook | `useChat`（ai v7）+ transport 承载 `UIMessage` 状态；重放装载（`agent_run_chain` + `agent_run_events` → 适配 → `setMessages`，逐 run 交错合成 record 部件）；发送组装（链尾参数经 body 穿透、运行中忽略）；`stop` → `invoke("agent_stop")`（不截断前端流）；`events` / `chain` / `currentRunId` 镜像 | `packages/desktop/src/hooks/use-agent-chat.ts`（新） | 适配层、transport、`@tauri-apps/api/core`、`ai`（useChat） | React hook |
| 展示组件族（双透镜） | `AgentMessages` 对话透镜（**仅 user / assistant 气泡**——system 角色消息渲染入口过滤：气泡 / 工具对卡 / 思考折叠 / AskUserQuestion 静态卡，message-scroller 内部滚动）、`AgentTimeline` 保真透镜（seq 序 / 子代理归因分组 / raw 透传，**flex 填充 + message-scroller 内部滚动**）、`AgentInput` composer（输入 / permission-mode / 发送 / 停止）、共享块渲染件、工具卡注册表；全部 agent 会话页面统一消费 | `packages/desktop/src/components/agent/` | 适配层类型、`ui/message`、`ui/message-scroller` | React + shadcn |
| 壳层滚动框架（回溯新增） | 路由内容包裹层立「高度受限的 flex 列框架」，为两场景的区域滚动提供高度来源；其余路由页为内容自适应 flex item，呈现不变 | `packages/desktop/src/app.tsx` | `ui/sidebar` | Tailwind flex 链 |
| Rust 状态机 `stopped` 收敛 | `AgentRunState` 增 `Stopped` 变体；`RunStateMachine::stop()` 显式收敛分支（区别于 RunResult 驱动）；收敛后幂等不变 | `packages/desktop/src-tauri/crates/core/agent/src/state.rs` | core/agent event | Rust enum |
| Rust 运行句柄终止信号 | `RunHandle` 挂点填实：逻辑终止请求（`request_stop` / `stop_requested` / 异步等待），trait 面零进程类型（kill 机制归租户实现） | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | tokio sync（`Notify` + `AtomicBool`） | Rust |
| CLI 租户击杀泵 | 泵任务 `select` 行流 vs 停止信号；信号到达即进程树击杀（Windows `taskkill /PID <cmd_pid> /T /F`，其余 `start_kill()`）；停止路径不合成 `error_process_exit` result | `packages/desktop/src-tauri/crates/infra/agent/src/runner.rs` | core/agent、tokio process | Rust + taskkill /T |
| exec 编排（提前 resolve + 后台 drive） | `AgentRunMessage` IPC 信封；`start_agent_run`（start → begin 落 running 行 → 注册停止句柄 → spawn 后台任务 → 提前 resolve）；`drive_agent_run`（tee 双 sink → EOF 收敛 → finish 落库 → Channel 流出 Record → 注销）；`RunStopRegistry` 运行中句柄注册表 | `packages/desktop/src-tauri/src/commands/exec/agent.rs` | store、core/agent、agent-cli、tauri Channel/AppHandle | tokio::spawn |
| 命令面 | `agent_start` 契约演进（提前 resolve running 记录）；`agent_stop(run_id)` 新增（幂等）；命令注册与托管状态挂载 | `packages/desktop/src-tauri/src/commands/exec/mod.rs`、`src/main.rs` | exec/agent | tauri::command |
| 页面接线 | explore 会话 hook 改走基建（stance 留来源侧）、双栏换共享组件族；调试页走 `AgentTimeline` + 停止入口（chrome 外圈保留）；**两场景根节点接入壳层 flex 链实现区域滚动（回溯）** | `packages/desktop/src/views/explores/*`、`packages/desktop/src/views/agent/*` | 组件族、use-agent-chat | React |

---

## 关键设计决策（proposal 待决问题定稿 + 回溯新增定稿）

| 待决问题（proposal） | 定稿 | 理由 |
|----------------------|------|------|
| 进程树击杀机制 | 停止信号经 `RunHandle` 到达 CLI 租户泵任务；Windows 下 `cmd /C` 包装时对 cmd 的 pid 执行 `taskkill /PID <pid> /T /F`（树杀，连带 claude 孙进程），非 Windows `child.start_kill()`；尽力语义，失败不重试不报错（残余限制 spec 已留痕）。停止路径泵不再合成 `error_process_exit` RunResult | `cmd /C` 下 child pid 即 cmd 进程，`/T` 覆盖整棵树；不合成 result 才能避免状态机被驱动成 failed，给 `stopped` 显式收敛让路 |
| `data-*` 部件命名空间与 `useChat` 泛型形状 | 五个 data 部件：`data-run-started` / `data-system-notice` / `data-run-result` / `data-raw` / `data-run-record`（data 即 `AgentRunRecord`）；消息元数据 `AgentMessageMetadata { seq, parentToolUseId }`；`AgentUIMessage = UIMessage<AgentMessageMetadata, AgentDataParts>`，定义收口适配层 | 保真字段（seq、归因、原文）全部可在状态模型中考证；record 部件承载链推进（两路同构的载体） |
| session hook 取链尾机制 | transport 构造注入 `onEvent` / `onRecord` 透传回调（观测点，不承载状态）：hook 经 `onRecord` 维护链状态（running 记录追加、终态按 id 替换），链尾取最后一条**非 running** 记录；不走 onFinish 状态猜测 | transport 保持无状态转换器；链状态归应用层（与 explore D3 一致）；非 running 过滤防御 stop 后 record 未达的竞态 |
| debug 页 chrome 去留 | 保留外圈：`AgentRunForm` / `AgentRunHistory` / `AgentRawStream` / JSONL 开关不动，仅时间线换 `AgentTimeline`、新增停止入口按钮（chrome） | 与 spec「页面级 chrome 包在共享核心外圈，MUST NOT 强行塞进组件族」一致 |
| `agent_start` 后台任务结构形态 | 编排拆两段：`start_agent_run`（同步段：runner start → store begin → 注册句柄 → spawn）+ `drive_agent_run`（后台任务体：tee 双 sink → EOF 收敛 → finish → Channel 流出 Record → 注销注册表，`&Store` 入参保持可测）；后台任务经命令注入的 `AppHandle` 在任务内 `state::<Store>()` / `state::<RunStopRegistry>()` 取托管句柄 | `run_agent_with` 旧形态（阻塞到终态）与提前 resolve 冲突；拆分后后台体仍以 `&Store` 可注入测试；`native_db::Database` 非 Clone，AppHandle 取 State 是零侵入方案（备选：store 句柄内部 Arc 化） |
| `stopped` 终态受控字符串扩展点 | 三处同步：`commands/exec/agent.rs` 增 `STATUS_STOPPED` 常量（与既有三常量同列）；`store/model.rs` `AgentRunRecord.status` 注释受控清单扩展（仅 doc，零 schema 迁移）；`types/dto.ts` `AgentRunStatus` union 增 `'stopped'` | status 是受控字符串非枚举列，扩展无迁移；三处是「Rust 常量 / 落库注释清单 / TS 镜像」的既有清单落点 |
| 终态记录过通道的形态 | `Channel<AgentEvent>` 升为 `Channel<AgentRunMessage>`：新增 IPC 信封 `AgentRunMessage { Event { event } | Record { record } }`（app 层类型，TS 镜像放 transport） | spec 要求「终态记录经 Channel 流出」；信封五变体不动（信封不含 record 语义，塞进 `usage` 是反模式）；类型化双变体让 transport 可判别收流 |
| **system 消息呈现口径（回溯新增）** | 过滤缝在 `AgentMessages` 渲染入口：`role === 'system'` 的消息整条不进渲染列表。适配层照产 system 载体消息（状态模型不动），hook 的 `messages` 不滤（保真透镜消费同一数组），保真透镜照呈全量 | system 角色是适配层「两路同构 + seq 保真 + id 稳定」的锚点，改适配层须连动保真透镜与重放状态形状，破坏面大且无收益；呈现口径是视图层关切，缝最小。不滤 hook 是因为 debug 保真透镜（AC-6 loop 可见性）依赖全量消息 |
| **区域滚动布局定式（回溯新增）** | 三级 flex 链：路由内容包裹层 `flex min-h-0 flex-col`（壳层立框架）→ 场景根 `flex min-h-0 flex-1 flex-col`（explore / debug 各自）→ 消息区 `min-h-0 flex-1` + 内部滚动载体（对话透镜与保真透镜沿用 message-scroller 组合；原始 JSONL 用 `overflow-y-auto`；历史重放区限高 `max-h` + `overflow-y-auto`） | 「撑满剩余区域 + 超出内部滚动」= 高度受限链上 flex 分配 + 滚动容器；不采用 `h-full` 百分比链（经 flex item 的百分比高度解析脆弱）；壳层一次性立框架，其余路由页为 flex-grow 0 的内容自适应 item，呈现不变 |

### 事件 → UIMessage 映射（design 定稿）

| AgentEvent | UIMessage 部件 / chunk |
|------------|------------------------|
| runStarted | `data-run-started` 部件（model/sessionId/tools/mcpServers + seq/timestampMs） |
| message.blocks: text | text 部件（整块，text-start/delta/end 组，无 token 拆分） |
| message.blocks: thinking | reasoning 部件 |
| message.blocks: toolUse | tool 部件（toolCallId=`block.id`，input；output 等同 id result 续上 → `output-available`） |
| message.blocks: toolResult | 无主 result：就地合成 tool 部件（name 占位「工具调用」）；有主：并入既有 tool 部件 output（`tool-output-available` chunk，跨消息按 id） |
| systemNotice | `data-system-notice` 部件 |
| runResult | `data-run-result` 部件 |
| raw | `data-raw` 部件（原文透传） |
| （终态 record，非事件） | `data-run-record` 部件（data = `AgentRunRecord`）+ finish chunk |

约定：每个 `message` 事件产出一个 UIMessage（id=`evt-${seq}`，role 收窄为 user/assistant，metadata 携带 seq 与 `parentToolUseId`）；非 message 事件各产出一条 system 角色 UIMessage 携带单个 data 部件（seq 保序、key 稳定）。`eventsToUIMessages`（折叠重建）与 `eventToChunk`（chunk 流经 useChat reducer）语义等价是硬不变量——重放与实时同一状态形状（AC-3 第一句），由适配层测试锁定。

**呈现过滤（回溯新增）**：上表产出的 system 角色 UIMessage 是**状态模型载体**（seq 保真、两路同构、保真透镜数据源），不是**对话呈现项**——`AgentMessages` 渲染入口按 `role !== 'system'` 过滤后呈现。过滤不回写状态、不回写适配层。

---

## 布局层级设计（区域滚动，回溯新增）

### 壳层滚动框架（`src/app.tsx`）

```
SidebarProvider（flex min-h-svh，既有）
└─ SidebarInset（main · flex w-full flex-1 flex-col，既有）
   ├─ ShellHeader（自然高，既有）
   └─ 路由内容包裹层  `mx-auto w-full flex-1 px-4 py-4`（既有）
                       + `flex min-h-0 flex-col`（本版新增——滚动框架起点）
```

包裹层 flex 列化后，各路由页根节点成为 flex item：`flex-grow: 0` + 内容自适应高度，change / db / explore 清单等页呈现不变（守线静态核对项）。高度约束只对显式 `flex-1` 接入的场景生效。

### explore 场景（`explore-detail-view.tsx`）

```
路由内容包裹层（flex col，本版起有高度约束）
└─ ExploreDetailView 根  `flex min-h-0 flex-col`（既有）
                          + `flex-1`（本版新增——缺的高度来源正是这一环）
   └─ ResizablePanelGroup `min-h-0 flex-1`（既有）
      └─ 左栏内层 `flex h-full min-h-0 flex-col gap-3`（既有）
         ├─ AgentMessages section `flex min-h-0 flex-1 flex-col`（既有）
         │  └─ MessageScroller 组合内部滚动（既有，流式跟随 + 到底按钮）
         └─ ExploreComposer（自然高，既有）
```

首轮已把对话透镜内部结构做对（flex-1 + min-h-0 + message-scroller），缺口只在祖先链无高度——本版补 `flex-1` 一环即达成「撑满左栏剩余区域、超出内部滚动」。右栏 `ExplorePreview` 的 `h-full` 链既有成立，不动。

### debug 场景（`agent-debug-view.tsx`）

```
路由内容包裹层（flex col，本版起有高度约束）
└─ AgentDebugView 根  `<div>`（既有普通块）
                       → `flex min-h-0 flex-1 flex-col`（本版改）
   ├─ AgentRunForm / RunErrorBanner / StreamToggle（自然高，既有）
   ├─ 流视图区（条件渲染二选一，本版均改 flex 填充）：
   │  ├─ AgentTimeline section  `flex min-h-0 flex-1 flex-col`
   │  │  ├─ h2 头（自然高）
   │  │  └─ MessageScroller 组合（min-h-0 flex-1，流式跟随 + 到底按钮）
   │  └─ AgentRawStream section `flex min-h-0 flex-1 flex-col`
   │     └─ 事件 <pre> 列表包 `min-h-0 flex-1 overflow-y-auto`（调试 dump，不做跟随）
   └─ AgentRunHistory（自然高随后；replay-area 本版限高，见下）
```

### 组件内部结构定稿

- **`AgentTimeline`**（回溯改）：section 增 `flex min-h-0 flex-1 flex-col`；消息体由裸堆叠改为 message-scroller 组合——`MessageScrollerProvider autoScroll defaultScrollPosition="end"` → `MessageScroller class="min-h-0 flex-1"` → `MessageScrollerViewport` → `MessageScrollerContent`（`topLevel` 消息逐条 `MessageScrollerItem` 包裹）→ `MessageScrollerButton direction="end"`。子代理归因嵌套在 `TimelineMessage` 内部，`MessageScrollerItem` 以 top-level 消息为单位，嵌套组随父项滚动。
- **历史重放嵌入态**：`AgentRunHistory` 的 `replay-area` 限高 `max-h-96 overflow-y-auto`；此时 `AgentTimeline` section 处于普通块容器内（`flex-1` 无 flex 语境）→ 自然高，滚动由 replay-area 限高容器承载，内层 MessageScroller 视口与内容等高、自动跟随静默 no-op——两态共用一个组件，无需 props 分叉。
- **`AgentMessages`**（回溯改）：section 结构与 scroller 不动；渲染入口过滤 system 消息（见关键设计决策），empty / loading 判定改用过滤后列表。

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/lib/agent-adapter.ts` | 事件适配纯函数（`eventsToUIMessages` / `eventToChunk` / `runRecordToUIMessage`）、`AgentUIMessage` / 元数据 / data 部件类型、工具同 id 配对收口 |
| `packages/desktop/src/lib/agent-transport.ts` | `TauriAgentTransport implements ChatTransport`、`AgentRunMessage` TS 镜像、body 链参数穿透 |
| `packages/desktop/src/hooks/use-agent-chat.ts` | headless 会话基建 hook（重放装载 + 发送组装 + 停止接线 + 镜像状态） |
| `packages/desktop/src/components/agent/agent-timeline.tsx` | 保真透镜：seq 序、子代理按 `parentToolUseId` 归因分组、raw 原文透传、result 汇总可复制（取代 `views/agent/components/agent-event-timeline.tsx`）；**回溯补齐：flex 填充 + message-scroller 内部滚动** |
| `packages/desktop/src/components/agent/agent-blocks.tsx` | 双透镜共享底层块渲染件：TextBlock（markdown）/ ThinkingBlock / ToolPairCard / ResultCard / CopyValue / record 状态行（自 explore-conversation 收编，data-testid 保留） |
| `packages/desktop/src/components/agent/tool-cards.tsx` | 工具卡注册表：`AgentToolCard` 类型、AskUserQuestionCard 收编、默认注册表（可插拔） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/package.json` | `dependencies` 增 `ai`（v7 主版本锚定） | ai-sdk 依赖收口（AC-8） |
| `packages/desktop/src/types/dto.ts` | `AgentRunStatus` union 增 `'stopped'` | 受控字符串 TS 镜像扩展，零 schema 变更 |
| `packages/desktop/src/components/agent/agent-messages.tsx` | 骨架填实为对话透镜（`Entry` 注释态替换为 `AgentUIMessage` 渲染 + message-scroller 组合 + loading/running/empty 态）；**本版呈现口径：渲染入口过滤 `role === 'system'` 消息，仅 user / assistant 气泡（text / reasoning / tool 部件 + AskUserQuestion 静态卡）——删除 `MessageView` 的 system / onlyData 直出分支与 `InfoRow` / `RawView` 局部渲染件，empty / loading 判定基于过滤后列表** | 对话透镜，explore 页消费；首轮曾直出辅助行 / raw / record 行，本回溯按需求收窄（保真呈现归 `AgentTimeline`） |
| `packages/desktop/src/components/agent/agent-input.tsx` | 空文件填实 composer：输入 / permission-mode 档位 / 发送（运行中禁发）/ 停止入口；id 与 testid 前缀化 | 沿 explore-composer 的 ModeSelect 语义与默认档 bypassPermissions |
| `packages/desktop/src/components/agent/index.tsx` | 导出面扩充（AgentMessages / AgentTimeline / AgentInput / 工具卡注册表） | 基建层统一出口 |
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | 内部改走 `use-agent-chat`（source=`explore`、sourceRef=记录 id）；`send` 只做 stance 拼接 + 委托；增 `stop`；删本地 Channel/invoke/链还原实现 | stance 文本组装留来源侧；对外状态语义（双恢复/续话/running/error）不变；`messages` 全量透传（过滤在透镜层，不在此滤） |
| `packages/desktop/src/views/explores/explore-detail-view.tsx` | 对话区换 `AgentMessages`；composer 停止入口接线；**回溯修订：根节点 `flex min-h-0 flex-col` 增 `flex-1`，接入壳层滚动框架** | 双栏、预览、watch 生命周期、run 终态（running true→false）定点重读文档不变 |
| `packages/desktop/src/views/explores/components/explore-composer.tsx` | 改为 `AgentInput` 薄适配（保留探索页 placeholder / label / explore-* testid 前缀） | 渲染与档位逻辑收编组件族 |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 改走 `use-agent-chat`（source=`debug`、sourceRef=null，每跑 `reset()`）；时间线换 `AgentTimeline`；raw 流改消费 chat 的 `events` 镜像；运行中增停止入口（chrome 按钮，调 `stop`）；**回溯修订：根节点改 `flex min-h-0 flex-1 flex-col`，流视图区（Timeline / RawStream）`flex-1` 填充，表单 / 横幅 / 切换行 / 历史区自然高** | run 表单 / 历史列表 / JSONL 开关外圈保留（AC-6） |
| `packages/desktop/src/views/agent/components/agent-run-history.tsx` | 重放区 `AgentEventTimeline` → `AgentTimeline`（`eventsToUIMessages` 转换）；`statusLabel` 增 `stopped`（已停止）；**回溯修订：`replay-area` 限高 `max-h-96 overflow-y-auto` 内部滚动** | 被删组件的引用级联；停止态历史可读；重放区不再撑高页面 |
| `packages/desktop/src/views/agent/components/agent-raw-stream.tsx` | **回溯新增修改**：section 改 `flex min-h-0 flex-1 flex-col`；事件 `<pre>` 列表包 `min-h-0 flex-1 overflow-y-auto` | debug 流视图二选一的另一半，不同步则 raw 态仍撑高页面 |
| `packages/desktop/src/app.tsx` | **回溯新增修改**：路由内容包裹层 `mx-auto w-full flex-1 px-4 py-4` 增 `flex min-h-0 flex-col` | 壳层滚动框架（区域滚动高度链起点）；其余路由页为内容自适应 flex item，呈现不变（守线静态核对项） |
| `packages/desktop/src-tauri/crates/core/agent/src/state.rs` | `AgentRunState` 增 `Stopped`；`RunStateMachine::stop()` 显式收敛分支；模块文档更新 | running → completed / failed / stopped |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | `RunHandle` 挂点填实：`request_stop` / `stop_requested` / 异步等待原语（`Notify` + `AtomicBool`，Clone + Default） | 逻辑终止信号，trait 面零进程类型 |
| `packages/desktop/src-tauri/crates/infra/agent/src/runner.rs` | 泵任务 `select` 行流 vs 停止信号；击杀实现（Windows `taskkill /PID <pid> /T /F`，其余 `start_kill()`）；停止路径跳过 `error_process_exit` 合成；`pump_lines` 缝签名随迁 | `discover` / `flags` / `jsonl` 不动（禁区） |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | `AgentRunMessage` IPC 信封、`STATUS_STOPPED` 常量、`RunStopRegistry`；`run_agent` / `run_agent_with` / `RunSummary` 阻塞形态拆分为 `start_agent_run` + `drive_agent_run`（提前 resolve + 后台 tee + EOF 收敛含 stopped 分支 + Channel 流出 Record + 注销）；store 写失败路径同样流 Record | proposal「Rust `commands::exec` + `run_agent()` 编排」的编排半边；微形态与 `*_inner` 同列的纪律保持 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `agent_start` 改经 `start_agent_run`（提前 resolve running 记录，`on_event` 参数类型升为 `Channel<AgentRunMessage>`）；新增 `agent_stop` 命令；模块文档更新 | proposal「Rust `commands::exec`」的命令面半边；契约演进（desktop-agent-execution delta） |
| `packages/desktop/src-tauri/src/main.rs` | `invoke_handler` 注册 `agent_stop`；`app.manage(RunStopRegistry::default())` | 与 `WatchRegistry` 同型托管 |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `AgentRunRecord.status` 字段注释受控清单扩展 `stopped`（仅 doc） | 「stopped 受控字符串清单扩展点」定稿落点；v1 历史结构不动 |

清单外补入说明（实现必然级联，proposal 未逐条列出）：`types/dto.ts`（受控字符串镜像）、`agent-run-history.tsx`（被删组件引用级联）、`use-agent-run.ts` 及其测试（proposal 风险表「两个 hook 同批改造走新基建」——调试 hook 由 `use-agent-chat` 取代删除，explore hook 改造保留）、`store/model.rs`（proposal 待决问题的清单扩展点）；**回溯新增两项**：`app.tsx`（壳层滚动框架，回溯需求 2 的高度链起点，proposal 文件面未预见壳层）与 `agent-raw-stream.tsx`（debug 流视图条件二选一的另一半，只改 Timeline 则 raw 态仍撑高页面，违背需求 2 的场景完整性）。

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/views/explores/components/explore-conversation.tsx` | 渲染收编进 `components/agent/`（agent-messages + agent-blocks + tool-cards）；既有用例随迁组件族测试 |
| `packages/desktop/src/views/explores/components/explore-conversation.test.tsx` | 随组件删除（用例随迁，测试阶段承接） |
| `packages/desktop/src/views/agent/components/agent-event-timeline.tsx` | 由 `components/agent/agent-timeline.tsx` 取代；既有用例随迁 |
| `packages/desktop/src/views/agent/components/agent-event-timeline.test.tsx` | 随组件删除（用例随迁，测试阶段承接） |
| `packages/desktop/src/views/agent/hooks/use-agent-run.ts` | 由 `use-agent-chat` 取代（调试页直接消费统一基建） |
| `packages/desktop/src/views/agent/hooks/use-agent-run.test.ts` | 随 hook 删除 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `eventsToUIMessages` | `src/lib/agent-adapter.ts` | 新增 | `function eventsToUIMessages(events: AgentEvent[]): AgentUIMessage[]` | 重放重建：事件序列折叠为 UIMessage 序列；工具同 id 配对就地收敛 |
| `eventToChunk` | `src/lib/agent-adapter.ts` | 新增 | `function eventToChunk(event: AgentEvent): UIMessageChunk[]` | 实时增量：单事件产出 chunk 组（spec 单 chunk 记法为简写，多部件事件需消息级 start/部件/收尾组） |
| `runRecordToUIMessage` | `src/lib/agent-adapter.ts` | 新增 | `function runRecordToUIMessage(record: AgentRunRecord): AgentUIMessage` | 终态同构：run 记录 → `data-run-record` 部件消息（重放与实时两路共用） |
| `TauriAgentTransport` | `src/lib/agent-transport.ts` | 新增 | `class TauriAgentTransport implements ChatTransport<AgentUIMessage>` | 见下行方法；无状态转换器 |
| `TauriAgentTransport.constructor` | `src/lib/agent-transport.ts` | 新增 | `constructor(options?: TauriAgentTransportOptions)` | `options.onEvent` / `options.onRecord` 透传观测点（hook 注入，不承载状态） |
| `TauriAgentTransport.sendMessages` | `src/lib/agent-transport.ts` | 新增 | `sendMessages(options: { trigger: 'submit-message' \| 'regenerate-message'; chatId: string; messageId: string \| undefined; messages: AgentUIMessage[]; abortSignal: AbortSignal \| undefined } & ChatRequestOptions): Promise<ReadableStream<UIMessageChunk>>` | body 链参数 + 末条用户文本 → `invoke("agent_start")`；Channel 消息逐条转 chunk；Record → record 部件 + finish + 关流 |
| `TauriAgentTransport.reconnectToStream` | `src/lib/agent-transport.ts` | 新增 | `reconnectToStream(options: { chatId: string }): Promise<ReadableStream<UIMessageChunk> \| null>` | 恒 `null`；重连诉求由重放装载承担 |
| `useAgentChat` | `src/hooks/use-agent-chat.ts` | 新增 | `function useAgentChat(params: UseAgentChatParams): UseAgentChatState` | headless 会话基建（重放装载 / 发送组装 / stop → agent_stop / 镜像状态） |
| `AgentMessages` | `src/components/agent/agent-messages.tsx` | 修改 | `function AgentMessages(props: AgentMessagesProps): React.JSX.Element` | 对话透镜（骨架填实）；**回溯修订语义：`props.messages` 接受全量（含 system 载体消息），组件渲染入口过滤 `role === 'system'`，呈现仅 user / assistant 气泡** |
| `AgentTimeline` | `src/components/agent/agent-timeline.tsx` | 新增 | `function AgentTimeline(props: AgentTimelineProps): React.JSX.Element` | 保真透镜；**回溯修订语义：`props.messages` 全量呈现（含 system），区域滚动内承载（message-scroller）** |
| `AgentInput` | `src/components/agent/agent-input.tsx` | 新增 | `function AgentInput(props: AgentInputProps): React.JSX.Element` | composer（发送 / 档位 / 停止） |
| `DEFAULT_TOOL_CARDS` | `src/components/agent/tool-cards.tsx` | 新增 | `const DEFAULT_TOOL_CARDS: Record<string, AgentToolCard>` | 默认工具卡注册表（AskUserQuestion → 静态卡） |
| `RunStateMachine.stop` | `crates/core/agent/src/state.rs` | 新增 | `pub fn stop(&mut self) -> AgentRunState` | 显式终止收敛：Running → Stopped；已收敛幂等原样返回 |
| `RunHandle.request_stop` | `crates/core/agent/src/runner.rs` | 新增 | `pub fn request_stop(&self)` | 置位终止请求并唤醒等待方（幂等） |
| `RunHandle.stop_requested` | `crates/core/agent/src/runner.rs` | 新增 | `pub fn stop_requested(&self) -> bool` | 同步观测（编排侧 EOF 收敛判定） |
| `RunHandle.wait_requested` | `crates/core/agent/src/runner.rs` | 新增 | `pub async fn wait_requested(&self)` | 异步等待（租户泵 select 半边） |
| `agent_start` | `src-tauri/src/commands/exec/mod.rs` | 修改 | `pub async fn agent_start(store: State<'_, Store>, on_event: Channel<AgentRunMessage>, root: String, prompt: String, permission_mode: AgentPermissionMode, resume_session_id: Option<String>, source: Option<String>, source_ref: Option<String>, parent_run_id: Option<i64>) -> Result<AgentRunRecord, String>` | 返回时序演进：begin 落库后即 resolve **running** 记录；终态经 Channel Record 流出；IPC 参数面不变 |
| `agent_stop` | `src-tauri/src/commands/exec/mod.rs` | 新增 | `pub fn agent_stop(registry: State<'_, RunStopRegistry>, run_id: i64) -> Result<(), String>` | 按 id 寻址 request_stop；非 running / 不存在幂等 `Ok` |
| `start_agent_run` | `src-tauri/src/commands/exec/agent.rs` | 新增 | `pub(crate) fn start_agent_run(app: AppHandle, store: &Store, on_event: Channel<AgentRunMessage>, params: AgentRunParams, provenance: RunProvenance) -> Result<AgentRunRecord, String>` | 编排同步段：start → begin 落 running 行 → 注册句柄 → spawn 后台任务 → 返回 running 记录 |
| `drive_agent_run` | `src-tauri/src/commands/exec/agent.rs` | 新增 | `pub(crate) async fn drive_agent_run<R: AgentRunner>(store: &Store, on_event: Channel<AgentRunMessage>, run: AgentRun, record: AgentRunRecord, registry: &RunStopRegistry) -> AgentRunRecord` | 后台任务体：tee 双 sink → EOF 收敛（状态机已收敛以状态机为准；否则 stop 请求 → `stopped`；兜底 failed）→ finish → 流出 Record → 注销 |
| `run_agent` / `run_agent_with` | `src-tauri/src/commands/exec/agent.rs` | 删除 | — | 阻塞至终态的旧编排，由上两行取代 |

回溯修订不新增公共 API：system 过滤是 `AgentMessages` 组件内私有行为（不引公共 helper——单一消费方，避免 test-only 导出污染 knip）；区域滚动全是 className / 结构调整，无签名变化。

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `AgentUIMessage` | `src/lib/agent-adapter.ts` | 新增 | `UIMessage<AgentMessageMetadata, AgentDataParts>` 别名；前端对话状态一等模型 |
| `AgentMessageMetadata` | `src/lib/agent-adapter.ts` | 新增 | `{ seq: number \| null; parentToolUseId: string \| null }`；seq 保真与 key 稳定，归因字段；record 合成消息 seq 为 null |
| `AgentDataParts` | `src/lib/agent-adapter.ts` | 新增 | 五个 data 部件形状：`data-run-started` / `data-system-notice` / `data-run-result` / `data-raw` / `data-run-record` |
| `AgentRunStartedData` / `AgentSystemNoticeData` / `AgentRunResultData` / `AgentRawData` | `src/lib/agent-adapter.ts` | 新增 | 各 data 部件载荷：源事件字段 + `seq` / `timestampMs` 保真；`data-run-record` 的 data 即 `AgentRunRecord` |
| `TauriAgentTransportOptions` | `src/lib/agent-transport.ts` | 新增 | `{ onEvent?: (event: AgentEvent) => void; onRecord?: (record: AgentRunRecord) => void }` |
| `AgentRunMessage`（TS 镜像） | `src/lib/agent-transport.ts` | 新增 | `{ ipc: 'event'; event: AgentEvent } \| { ipc: 'record'; record: AgentRunRecord }`（对齐 Rust serde tagged 形态） |
| `UseAgentChatParams` / `UseAgentChatState` / `AgentChatSendInput` | `src/hooks/use-agent-chat.ts` | 新增 | 入参 `{ source: string; sourceRef: string \| null; root: string \| null }`；状态含 messages / events / chain / loading / running / error / currentRunId / sendMessage / stop / reset |
| `AgentMessagesProps` / `AgentTimelineProps` / `AgentInputProps` | `src/components/agent/agent-messages.tsx` / `agent-timeline.tsx` / `agent-input.tsx` | 新增 | 组件 props；透镜消费 `AgentUIMessage[]`（**回溯语义注记：`AgentMessagesProps.messages` 为全量入参、system 过滤在组件内**）；composer 发送入参 `{ text: string; permissionMode: AgentPermissionMode }` |
| `AgentToolCard` | `src/components/agent/tool-cards.tsx` | 新增 | 特化工具卡可插拔位类型（toolCallId / toolName / input / output / isError → 渲染或 null 回退） |
| `AgentRunStatus` | `src/types/dto.ts` | 修改 | `'running' \| 'completed' \| 'failed'` 增 `'stopped'` |
| `AgentRunState` | `crates/core/agent/src/state.rs` | 修改 | enum 增 `Stopped` 变体 |
| `RunHandle` | `crates/core/agent/src/runner.rs` | 修改 | 由空挂点结构填实为终止信号句柄（Clone + Default）；仍是逻辑面，零进程类型 |
| `AgentRunMessage` | `src-tauri/src/commands/exec/agent.rs` | 新增 | `#[serde(tag = "ipc", rename_all = "camelCase")] enum { Event { event: AgentEvent }, Record { record: AgentRunRecord } }`；`Channel` 消息信封（app 层 IPC 类型，非 core 契约） |
| `RunStopRegistry` | `src-tauri/src/commands/exec/agent.rs` | 新增 | `Mutex<HashMap<i64, RunHandle>>` 托管状态（`WatchRegistry` 同型）：run id → 停止句柄，终态即除名 |

回溯修订不新增 / 不修改任何类型定义。

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `dependencies.ai` | `packages/desktop/package.json` | 新增 | string（semver range，`^7` 起步） | ai-sdk v7 主版本锚定；精确版本以接入时 stable v7 为准；适配层为版本变动隔离带 |

<!-- 其余配置文件（plugin.json / config.json / hooks.json / settings.json 等）本变更不涉及，省略；回溯修订不涉及配置 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `AgentRunRecord`（store） | 既有 16 字段不动；`status` 受控字符串清单扩展 `stopped`（running / completed / failed / stopped） | `source` + `source_ref` + `parent_run_id` 构成链（`restore_run_chain` 回溯）；事件经 `run_id` 归属 | redb `user_agent_runs`；**schema 零变更**（status 为字符串列，无迁移）；`finish_agent_run` 整行替换语义对 `stopped` 终态同样适用 |
| `AgentEvent`（信封） | 五变体 + seq + timestampMs，**不动** | run_id 归属；seq 每 run 从 0 单调 | redb `user_agent_run_events`；零变更；停止路径不新增信封变体、不合成 result 事件 |
| `AgentRunMessage`（IPC 信封） | `Event { event }` \| `Record { record }`，tag `ipc` | `agent_start` 命令作用域 Channel 的消息体 | 无（瞬时 IPC） |
| `AgentUIMessage`（前端状态模型） | id（`evt-${seq}`）/ role（user / assistant / system）/ parts（text / reasoning / tool / `data-*`）/ metadata（seq、parentToolUseId） | 由 `AgentEvent` 序列经适配层重建；工具 input/output 同住一个 tool part；**system 角色是状态模型载体，呈现过滤只发生在对话透镜渲染入口，状态形状不受影响（回溯注记）** | 无（瞬态；重开恢复 = 重放重建，不落库） |
| `RunStopRegistry`（托管状态） | `HashMap<run_id, RunHandle>` | `agent_start` 登记、`drive_agent_run` 终态除名、`agent_stop` 查询 | 无（内存；应用重启即清空，与进程同生命周期） |

回溯修订对数据模型零变更（两条需求均为呈现层 / 布局层）。

---

## 路由/API 设计（Tauri IPC 命令）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| invoke | `agent_start`（修改） | 发起运行；**落库即提前 resolve running 记录**，run 转后台任务；终态经 Channel Record 流出 | `onEvent: Channel<AgentRunMessage>`、`root`、`prompt`、`permissionMode`、`resumeSessionId?`、`source?`、`sourceRef?`、`parentRunId?` | `Promise<AgentRunRecord>`（running 态，id 立即可用）；Channel 逐条 `AgentRunMessage` | 本机应用 |
| invoke | `agent_stop`（新增） | 终止运行中 run：置停止信号 → 泵击杀进程树 → 编排收敛 `stopped` → Channel 流出终态 Record | `runId: number` | `Promise<void>`；非 running / 不存在幂等 `Ok` | 本机应用 |
| invoke | `agent_runs` / `agent_run_events` / `agent_run_chain`（不变） | 查询薄包装（清单 / 单 run 事件重放 / 来源单链还原） | 既有参数 | 既有 DTO | 本机应用 |

停止时序（前端 → 后端）：`use-agent-chat.stop()` → `invoke("agent_stop")`（**不调 `chat.stop()` 截断前端流**）→ 泵击杀 → EOF → 编排收敛 `stopped` + finish 落库 → Channel 流出 `Record` → transport 转 `data-run-record` chunk + finish → useChat 状态收口（running 转否、composer 恢复可发）。前端截断会让 record 部件与 finish 丢失，正因如此停止必须由后端闭流收尾（spec「MUST NOT 仅截断前端流」）。

回溯修订对 IPC 契约零变更。

---

## 依赖

### 运行时依赖

- `ai`（v7 主版本锚定，新增）— 前端状态层：`useChat` / `ChatTransport` / `UIMessage` / `UIMessageChunk`。收口纪律（AC-8）：仅适配层、transport、`use-agent-chat`、`components/agent/` 四类基建文件持有 import；视图层经 `ReturnType<typeof useAgentChat>` 等结构化推导流转类型，不直接 import `ai`。
- `@tauri-apps/api`（既有，不新增）— `invoke` / `Channel`。
- `@shadcn/react` 的 message-scroller 原语经既有 vendored 包装 `ui/message-scroller` 消费（既有，不新增依赖）；回溯修订将保真透镜接入该既有包装。

### 构建/测试依赖

- 无新增 — 既有 `vite-plus` / `knip` / `typescript` 工具链、`@testing-library/react`、`jsdom`、Rust 侧 `tokio`（`sync` 特性已被 core/agent 依赖，无需新特性）。测试编写与执行归 test-design / test-gen / test-execution 阶段承接。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 适配层 | `lib/agent-adapter.ts`：五变体映射表（决策节）、toolUse/toolResult 同 id 跨消息配对收口（`collectResults` 随删除消灭）、seq / `parentToolUseId` 进 `AgentMessageMetadata`、未知事件 `data-raw` 透传；纯函数可测。回溯修订零触碰（system 载体消息照产） |
| AC-2 transport | `lib/agent-transport.ts`：body 链参数原样进 `agent_start` invoke 参数（transport 不持链状态）；Channel 事件逐条 `eventToChunk`；Record → record 部件 + finish 后 `controller.close()`；假 Channel/invoke 可测（tauri API mock 缝既有模式）。回溯修订零触碰 |
| AC-3 use-agent-chat | 重放装载（chain + events → 适配 → setMessages，与实时同一状态形状）；发送 body 携带链尾（非 running 记录）`resumeSessionId` / `parentRunId`；运行中（status streaming/submitted）重复发送忽略；`stop` → `invoke("agent_stop")` 触达后端。回溯修订零触碰（`messages` 全量不滤，过滤归透镜层） |
| AC-4 组件族 | `AgentMessages` 四变体气泡 + AskUserQuestion 静态卡（注册表），**渲染入口过滤 system 角色消息（回溯：辅助行 / raw / record 不在对话透镜呈现，仅保真透镜）**；`AgentTimeline` seq 序 + `metadata.parentToolUseId` 归因分组 + 全量呈现 + message-scroller 区域滚动；`AgentInput` 运行中禁发 + 停止入口；既有 data-testid 保留支撑迁移用例 |
| AC-5 explore 接线 | `use-explore-session` 改走基建（stance 留来源侧、链尾续话语义不变）；`AgentMessages` 沿用四变体气泡映射与 message-scroller 组合——**呈现一致基线 = 四变体气泡与迁移前一致（回溯注记：辅助行不再呈现属需求显式收窄，若迁移用例基线含辅助行断言，以回溯需求为准收窄，测试阶段承接）**；根节点接入壳层 flex 链实现区域滚动；用例随迁由测试阶段验证 |
| AC-6 debug 接线 | `agent-debug-view` 时间线换 `AgentTimeline`（loop 可见性场景：工具成对、result 汇总可复制、子代理归因保持）；运行中呈现停止入口（chrome）且停止后经 Record 部件收敛；**回溯：流视图区 flex-1 填充 + 内部滚动（Timeline 经 message-scroller、RawStream 经 overflow-y-auto），重放区限高内部滚动** |
| AC-7 Rust stop + 提前 resolve | `start_agent_run` / `drive_agent_run` 拆分：begin 后即返回 running 记录；`agent_stop` → 信号 → 击杀 → `RunStateMachine::stop()` 收敛 `stopped` → finish 落库 → Channel Record；registry miss / 已终态幂等 `Ok`；假 runner 注入缝保留（`&Store` + `&RunStopRegistry` 入参）。回溯修订零触碰 |
| AC-8 ai-sdk 收口 | 依赖表收口纪律 + 守线任务静态核对（`from 'ai'` 仅四类基建文件；Rust/store 零概念）；信封与 store schema 零 diff。回溯修订不新增 `ai` import 面（agent-timeline 接入的 message-scroller 走 vendored 包装，不经 `ai`） |

---

## 待决问题

- **已随首轮实现落定（备案，不再是待决）**：ai v7 `UIMessageChunk` 逐块编排（实现以 `start` + `reset-step` 组化解 reducer 单消息累积，两路同构由实现自检）；Store 句柄入后台任务（定稿并落地：命令注入 `AppHandle`、任务内 `state::<Store>()`）；`taskkill` 控制台窗口抑制（`CREATE_NO_WINDOW` 已落地）；record 部件呈现粒度（**被本回溯取代**：对话透镜不再呈现 record 行，仅保真透镜 `RunRecordRow`）。
- **迁移用例基线**：若首轮迁移的组件测试含对话区辅助行断言（对话透镜内的 `event-info` / `event-raw` 等），按回溯需求收窄用例基线——归测试阶段承接，非本设计实现任务。
- **`AgentTimeline` 的 message-scroller 跟随粒度**：嵌套子代理组展开时的 anchor 行为以既有 primitive 语义为准；若实现期评估干扰调试观测，降级路径为撤下 scroller 组合改 `min-h-0 flex-1 overflow-y-auto`（区域滚动达成不变，仅失去流式跟随）。
- **debug 页 run 列表自然高**：本版仅对 `replay-area` 限高；run 列表本身仍随条目数自然增高，若实现期评估列表过长干扰流视图，再议列表区限高（不阻塞本轮，属 chrome 内微调）。
