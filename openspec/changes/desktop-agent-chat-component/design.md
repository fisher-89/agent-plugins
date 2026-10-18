# 设计: desktop-agent-chat-component

> **变更**: desktop-agent-chat-component
> **日期**: 2026-10-17
> **基于**: proposal.md、explore.md（PoC 实测 V1–V6 + 差异矩阵九维）、specs/ 五份能力 delta

---

## 提案与规格同步状态

`proposal.md` 与五份 delta spec（`desktop-agent-chat-infra` / `desktop-agent-execution` / `desktop-explore-page` / `desktop-change-flow-view` / `desktop-ipc-type-bindings`）已定稿，本设计基于其文本展开，只收敛 proposal 遗留给 design 的形态面：**注册表模块名与容量常量**（D1）、**订阅面是否补发库内快照**（D3）、**composer 插槽形态**（D10）、**组件 `data-testid` 命名**（D9 与阶段八任务）、以及 spec Module Contract 里未定型的签名细节（D2 / D4 / D5）。

工作树实况（本变更起点，`packages/desktop` 0.5.16）：`SessionKernel` 只持 `sink` + `StopRegistry`（`kernel.rs:119-122`）；`RunningTurn::drive` 的 `on_output` 是事件唯一出口，delta 在 `kernel.rs:231-235` 跳过 `append_sealed`；exec 轨唯一实时出口是 `agent_start_with` 里 `KernelOutput` → `Channel<AgentRunMessage>` 的前向任务（`commands/exec/mod.rs:103-116`）；change 轨经 `KernelWorkerPort` → 同一 `drive` 回调（`infra/agent/src/worker.rs:58-75`），但只把事件塞进 `RunUpdate::SessionEvent` 且 IPC 面降位为 kind-only `RunNotice`；`compose_turn` 只有两处调用点（`commands/exec/mod.rs:96`、`commands/change_flow/mod.rs:142`）。前端 `reconnectToStream` 恒 `null`（`agent-transport.ts:243`），`useAgentChat.running` = `chat.status`（发送方视角），`useSessionTranscript` 的 `liveEvents` 参数零调用方，`notifyTranscript` / `transcriptTick` 是 150ms 专用去抖键。适配层 `blocksToParts`(:212) / `blocksToChunks`(:309) 各自含判定，`shapeOf`(:480) 是第三份手写投影。

本设计描述**本变更完成后的目标形态**：一个会话级注册表（进程内存态）、一个订阅命令、一个前端组件 + 一个 headless hook、一套 IR 化适配层、四个消费点收编、两处退役。零 store schema 迁移、零引擎协议改动、零 `emit`/`listen` 引入。

---

## 关键设计决策（proposal / spec 遗留形态面定稿）

| # | 问题 | 定稿 | 理由 | 备选方案 |
|---|------|------|------|----------|
| D1 | 注册表模块名与 broadcast 容量 | 模块 `crates/core/agent/src/stream.rs`；容量常量 `SESSION_STREAM_CAPACITY: usize = 4096`（私有于模块，`broadcast::channel` 单点消费） | delta 密度比 change 域通知（`UPDATE_CAPACITY = 512`）高一个量级（每 token 一条），容量须按「一轮回复的 delta 突发」定价（proposal 要求 ≥2048）；4096 覆盖 4k token 内的整轮突发且内存占用可控（条目为 `AgentEvent` 的 `Arc` 级拷贝） | 复用 change 域 512（一轮长回复必溢出，断流重订成为常态）；无界队列（慢消费者吃内存，失去 lagged 兜底信号） |
| D2 | 订阅句柄形态 | `subscribe(&self, session_id: &str, since_seq: u64) -> Option<SessionStreamSubscription>`；句柄 `recv()` 内部完成 `seq > since_seq` 过滤与 lagged 判别 | spec 的参数面（`session_id` + `since_seq`）与语义（不重投、溢出断流）都保留，坏味道（`since_seq` 只做形参）被消除：过滤单点驻 core、可单测（proposal 要求 core 单测覆盖 since_seq 与 lagged）；命令层保持三段薄包装 | 返回裸 `Receiver` + 命令层过滤（since_seq 语义散到壳层，core 侧无法单测）；`since_seq` 只作形参（自相矛盾） |
| D3 | 订阅面是否「先补发库内密封快照再续接 live」 | **不补发**：订阅面纯实时（注册表零 store 依赖），`seq > since_seq` 过滤后直投；重放 → 订阅窗口内的密封事件缺口由「收口以重放」修复 | 约束④已强制前端「先全量重放再续订」，窗口压到毫秒级且只影响过程态（proposal 风险 2 明示可接受）；快照补发需在命令层做「快照 + live 同 seq 空间去重排序」，与广播内到达顺序竞争，复杂度换不到终态正确性（终态本来由重放唯一保证） | 命令层双源拼接（去重 / 排序 / 竞态三处新错面）；core 注册表触库（破 `core/agent` 零持久化纪律） |
| D4 | 终态载荷如何装配 `TurnSummary`（`started_at` 从哪来） | `TurnOutcome` 增 `started_at: i64` 字段（内核 `converge` 落定），注册表条目 `Finished(TurnOutcome)`；命令层 `outcome_summary(outcome, session_id)` 投影 | spec 要求载体为「`AgentEvent` + `TurnOutcome`」中性类型，且 core MUST NOT 认识 IPC 信封；`started_at` 本是轮数据（`begin_turn` 已知），挪进收口形状去掉「终态不携带起点、由命令层回填」的历史绕行，`outcome_summary` 少一参（副作用为净简化）；store 侧 `finish_turn` 按 `turn_id` 更新、`started_at` 沿用存量行，零落库影响 | 注册表条目额外携带 `started_at`（同一份轮数据存两处）；`finished_at - duration_ms` 反推（`duration_ms` 缺席即失真） |
| D5 | `agent_session_watch` 命令完成语义 | **异步命令体 await 前向循环**：登记订阅 → `Channel` 前向 → 循环结束（轮终态 `Record` / lagged / Channel 关闭）后返回 `Ok(())`；无条目 / blank root 立即 `Ok(())` | spec 明文「轮终态以 `Record` 收尾并结束命令」，await 形态让「命令结束」与「订阅释放」是同一件事，前端拿得到「订阅面已终止」这个信号（D7 的兜底依赖它）；沿 `agent_start_with` 提前 resolve 先例，Channel 在 invoke 返回后仍可用 | `spawn` 前向任务 + 立即返回（沿 `change_flow_watch`）：前端无法区分「订阅结束」与「invoke 只是登记完」，零事件场景会悬挂流 |
| D6 | 两出口的簿记共用形态 | transport 内抽 `openEnvelopeStream()`：`Channel` 信封 → chunk 流的**开件簿记 + `Record` 收口 + 关流**单点，返回 `{ stream, accept, end, fail }`；`sendMessages` 与 `reconnectToStream` 各写「invoke 装配」薄层 | 约束③（订阅流复刻开件簿记）与 V6 的根因是「簿记写了两遍」；单点化后两出口共用一套 `start` + `reset-step` + part-start 逻辑，diff 面只剩「invoke 谁、结束时怎么收」 | 订阅路另写一份簿记（V6 复发面）；把 `sendMessages` 的 controller 参数外传（`streamStart` 现形态无法承载两条 invoke） |
| D7 | 订阅流结束 / 断流的收口 | `reconnectToStream` 前置状态门（`sessionDetail` 无 running 轮行 → 返回 `null`）；订阅流以 `close()` 结束（**不投伪 `data-run-record`**）；invoke 解析而流未收尾 → 关流；`abortSignal` abort → 关流 | 「无运行轮返回 null」是 spec Module Contract 明文；`close()` 即可让 ai-sdk 复位 `status` 并触发 `onFinish`（已核对 `makeRequest` 的 `finally`：状态复位与 `onFinish` 只依赖流 close，不依赖 `finish` chunk），伪终态会污染 `data-run-record`（spec 明令禁止） | 不关流（V4：`status` 永 `streaming`、`resumeStream` promise 永不 resolve）；补一个伪 `Record`（假终态，违反 lagged 兜底语义） |
| D8 | `since_seq` 取值口径 | 前端 `seqCursorRef` = **已观测事件最大 `seq`**（含 delta，经 transport `onEvent` 观测；重放装载同样推高），订阅时以其为 `since_seq` | spec 基线口径是「已装载转录最大 seq」，但 delta 也占 seq 空间：只取密封转录最大 seq 会在「已消费 delta → 断流 → 重订」路径重投若干 delta（V5 同类重复，表现是同一段文字双份）；游标取含 delta 的上界是严格更强的「不重投」 | 只取密封最大 seq（delta 重投）；持久化游标（跨轮无意义，delta 无库） |
| D9 | 组件双形态与 testid | `AgentSessionView` 支持**自持形态**（`root` + `sessionId`，内部建流，只读消费点用）与**注入形态**（`session` = `useAgentSession` 返回值，来源侧已建 hook 的交互页用）；注入形态内部 hook 以 `sessionId = null` 空载（零取数、零订阅、零第二条流） | 四个消费点里有三个需要 hook 数据在组件外（debug 的 `events` 原始转录、explore 的 `chain` / `loading` stance 与 auto-send 门、change 抽屉的 `refreshKey`）；让视图再自建一个 hook 会**双流双查询**，与「单流在挂」直接冲突。两形态共用同一渲染体与同一 hook，仍是唯一渲染入口 | 只留自持形态（debug/explore 必须把 stance 组装与原始转录下沉进组件，违反「来源侧组装」+ 页面级 chrome 纪律）；两个导出组件（「唯一入口」名存实亡） |
| D10 | composer 插槽形态 | **render prop**：`composer?: (actions: AgentSessionActions) => React.ReactNode`，`actions = { running, loading, send, stop }`；渲染位固定消息区下方 | 插槽内容需要运行态与动作面（explore 的 `AgentInput` 要 `running` / `onStop`），`ReactNode` 无法传值；`send` 虽由来源侧包装（stance），仍一并给出以保插槽自足 | `ReactNode`（来源侧只好再自建 hook）；`children`（位置语义混流） |
| D11 | 适配层 IR 三层的导出面 | IR 三层（`PairingScope` / `planBlock` / `plansToParts` / `plansToChunks`）**模块内私有**；导出面仍是 `eventsToUIMessages` / `eventToChunk` 两个公共入口 | AGENT.md 红线：`export` 不得为测试而加（knip 把 test-only export 判为死代码）；既有 `blocksToParts` / `blocksToChunks` 就是私有先例。计划层对拍经两公共入口达成——同一 `AgentEvent` 经 `eventsToUIMessages`（global 视野 + `plansToParts`）与 `eventToChunk` + reducer（local 视野 + `plansToChunks`）产出，二者内部走的是同一份 `BlockPlan[]` 生成器 | 导出 IR 层（knip 报未用导出，AGENT.md 明令禁止）；拆第二个文件（同一语义点跨文件，稳定性变差） |
| D12 | 会话元信息归属 | `SessionMeta`（id / 运行中徽章 / 轮数 / tokens）**上移进 `AgentSessionView`**，四消费点统一呈「对话 + 轮行 + 会话元信息」；`session-transcript-panel` 只保留「槽位缺席空态」与自身 wrapper testid | acceptance #1 要求「入参即会话（对话 + 轮行 + 会话元信息）」，只在一处渲染才不重复实现；testid（`session-meta*`）随组件走，change 域既有断言面最大保留 | 每个消费点各自渲染元信息（四份重复）；组件暴露 meta 渲染回调（无谓复杂度） |
| D13 | change 域通知分流 | `notifyTranscript` / `transcriptTick` 与 150ms 常数退役；`SessionEvent` 通知并入统一视图重查（300ms `notifyRefresh`）；`useChangeDetail` 增导出 `refreshTick`（统一 tick）作组件的 `refreshKey` | spec：`SessionEvent` 仅作会话状态重评估信号，delta 不落库故「通知 → 重查转录库」不再是实时面；两套节拍并存是 proposal 点名的死代码面相 | 保留 150ms 转录去抖（两套节拍 + 空转重查）；订阅组件自带去抖（第二套计时面） |
| D14 | 历史重放的自有查询 | `use-agent-run-history` 的 `useReplay` 整段退役（`events` / `agentSessionTranscript` 直调删除），`openSession` 只置选中 id、新增 `closeSession`；重放交 `AgentSessionView` 自持形态 | acceptance #1：视图层零 `eventsToUIMessages` 直调、零自有转录查询 hook；C 形态（纯静态裸折叠、无轮行交错）随之退役 | 保留 `events` 面（组件外第二套转录查询，C 形态残留） |
| D15 | 订阅自旋防护 | 订阅以 `(targetId, refreshKey, 订阅前游标)` 为键记「无进展结束」：一次订阅结束而游标未前进且未收到 `Record` → 阻断重订，直到 `sessionId` / `refreshKey` 变化才重试 | 「库态 running 但无实时流」有真实来源（应用重启遗留 running 轮行 / 收口与重放竞态）；不设闸会形成 invoke 自旋。阻断后仍受 `refreshKey` 重评估驱动，不引入轮询 | 无闸（自旋）；定时重试（= 引入轮询，违反「完成态零轮询」纪律的邻域） |
| D16 | 收口重放的失败面 | 收口重放查询失败 → 回落本地事件镜像折叠（`buildMessages(eventsRef, turnsRef)`），不抛错、不阻塞 | spec 明文「收口查询失败回落本地事件镜像折叠」；镜像折叠与重放折叠是**同一折叠入口**（`eventsToUIMessages` + 轮行交错），只是数据源降级 | 收口失败即报错（把可降级的查询失败升级为用户可见故障）；保留实时消息态（与重开形状不一致） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 会话级实时流注册表（新） | per-session 有界 broadcast：轮生命周期登记 / 事件与终态投递 / 订阅句柄（`since_seq` 过滤 + lagged 判别）/ 投终态即除名；载体为 core 中性类型，零 store、零 Tauri、零 IPC 信封知识 | `packages/desktop/src-tauri/crates/core/agent/src/stream.rs`（新，`lib.rs` 声明 `mod stream;` 并 re-export） | `tokio::sync::broadcast`（已在 workspace `sync` feature 内）、`crate::event::AgentEvent`、`crate::port::TurnOutcome` | Rust（sync 原语 + `Mutex<HashMap>`，与 `StopRegistry` 同型） |
| 内核写入单点 | `SessionKernel` / `RunningTurn` 注入 `Arc<SessionStreamRegistry>`：`begin_turn` 登记、`drive` 内 `Observation` 单点发布（delta 与密封同源）、`converge` 投终态并除名 | `crates/core/agent/src/kernel.rs` | stream、port、runner、state | Rust、tokio async |
| 轮收口形状 | `TurnOutcome` 增 `started_at`（终态轮行装配的起点半边） | `crates/core/agent/src/port.rs` | serde | Rust |
| 组合根注入 | `compose_turn` 增 `streams: Arc<SessionStreamRegistry>` 入参（沿 `StopRegistry` 先例），内核构造处透传；exec 轨与 change 轨（`KernelWorkerPort`）**消费方零改动**同源进入 | `crates/infra/agent/src/compose.rs` | core/agent、store | Rust |
| 订阅命令（新） | `agent_session_watch`：参数转换 → 注册表订阅 → `Channel<AgentRunMessage>` 前向 + `since_seq` 过滤 + lagged 断流 → 结束释放；blank root / 无条目幂等 `Ok(())` | `packages/desktop/src-tauri/src/commands/exec/mod.rs` | core/agent、tauri `Channel`、`commands/exec/agent.rs` 信封 | Rust、tauri async command |
| 壳层托管 | `app.manage(Arc::new(SessionStreamRegistry::default()))`（与 `StopRegistry` 同型空表） | `packages/desktop/src-tauri/src/main.rs` | core/agent | Rust |
| 事件适配层 IR | 视野层 `PairingScope`（`global` / `local`，两视野同形态 `PairedResults`）→ 语义层 `planBlock`（唯一判定点 + `const _exhaustive: never`）→ 发射层 `plansToParts` / `plansToChunks`（纯机械）；`eventsToUIMessages` 走 global 视野 + parts 发射，`eventToChunk` 走 local 视野 + chunks 发射 | `packages/desktop/src/lib/agent-adapter.ts` | `ai`（类型）、`types/generated/bindings` | TypeScript（纯函数） |
| 双起点 transport | `openEnvelopeStream` 簿记单点（`start` + `reset-step` + part-start + `Record` 收口）；`sendMessages` = 发起流（`agent_start`，Channel 先于 invoke）；`reconnectToStream` = 订阅流（`session_detail` 状态门 → `agent_session_watch`，invoke 解析 / abort 关流） | `packages/desktop/src/lib/agent-transport.ts` | `@tauri-apps/api/core`、`ai`、agent-adapter、bindings | TypeScript |
| 会话基建 hook（新，收编 `use-agent-chat`） | 重放装载（`sessionDetail` + `agentSessionTranscript` → 同一折叠入口）+ 状态驱动挂流（库态 running 且无在挂流才 `resumeStream`）+ 单流在挂 + 收口以重放收敛（失败回落镜像折叠）+ 断流重订（游标 / 无进展闸）+ 发送 / 停止 / 重置 | `packages/desktop/src/hooks/use-agent-session.ts`（新） | `@ai-sdk/react`、agent-adapter、agent-transport、bindings | TypeScript、React hook |
| 会话级渲染组件（新） | 元信息行 + 错误 / 加载 / 空态 + `AgentMessages` 对话透镜 + composer 插槽；自持 / 注入双形态共用同一渲染体 | `packages/desktop/src/components/agent/agent-session-view.tsx`（新，`components/agent/index.tsx` 导出） | agent-messages、hooks/use-agent-session | React、TypeScript |
| 消费面（改） | debug（交互）/ explore（交互）/ change 抽屉（只读）/ 历史重放（只读）全部经 `AgentSessionView`；交互页调 `useAgentSession` 后以注入形态渲染（来源侧保留 stance 组装与 auto-send 门） | `views/agent/*`、`views/explores/*`、`views/changes/flow/session-transcript-panel.tsx` | 上述组件与 hook | React、TypeScript |
| 通知面降位 | `notifyTranscript` / `transcriptTick` / 150ms 常数退役，`SessionEvent` 并入统一重查 tick；`useChangeDetail` 导出 `refreshTick` | `views/changes/hooks/use-change-detail.ts` + `views/changes/change-detail-view.tsx` | bindings | TypeScript |

数据流（目标形态）：

```
        kernel drive（唯一写入点，exec 轨 / change 轨同源）
              │  publish(Observation{delta|sealed})
              │  finish(TurnOutcome{started_at})
              ▼
   SessionStreamRegistry（per-session 有界 broadcast，轮生命周期）
              │  subscribe(session_id, since_seq)
              ▼
   agent_session_watch ── Channel<AgentRunMessage>（载荷直投）
              │
   前端 transport.reconnectToStream ── openEnvelopeStream（簿记单点）
              │
     use-agent-session（单流在挂 / 收口以重放收敛 / 断流重订）
              │
        AgentSessionView（唯一渲染入口：元信息 + 对话 + 轮行 + composer 插槽）
```

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/agent/src/stream.rs` | 会话级实时流注册表：`SessionStreamRegistry` + `SessionStreamItem` + `SessionStreamSubscription` + `SessionStreamRecvError` + 容量常量（公共项见「公共函数 / API」「类型定义」；配套 `stream_test.rs` 归测试相位） |
| `packages/desktop/src/hooks/use-agent-session.ts` | 统一会话基建 hook（自 `hooks/use-agent-chat.ts` 收编）：重放装载 + 状态驱动挂流 + 单流在挂 + 收口以重放收敛 + 断流重订 + 发送 / 停止 / 重置（配套 `use-agent-session.test.ts` 归测试相位，收编 `use-agent-chat.test.ts`） |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | 会话级唯一渲染入口：元信息行（`SessionMeta`）+ 错误 / 加载 / 空态 + `AgentMessages` + composer 插槽；自持 / 注入双形态（配套 `agent-session-view.test.tsx` 归测试相位） |

### 修改文件 — 后端（`packages/desktop/src-tauri`）

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/agent/src/lib.rs` | 增 `mod stream;`；`pub use stream::{SessionStreamItem, SessionStreamRecvError, SessionStreamRegistry, SessionStreamSubscription};` | 注册表出口（D1） |
| `crates/core/agent/src/port.rs` | `TurnOutcome` 增 `pub started_at: i64` 字段（doc：轮起点，wire camelCase `startedAt`；终态轮行装配与订阅面 `Record` 投影共用，store 侧沿用存量行不写该列） | 收口形状自足化（D4） |
| `crates/core/agent/src/kernel.rs` | `SessionKernel` 增 `streams: Arc<SessionStreamRegistry>` 字段与 `new(sink, registry, streams)`；`begin_turn` 在返回 `RunningTurn` 前 `streams.begin(&session_id)`；`RunningTurn` 增 `streams` 字段并在 `drive` 内：delta 臂 `streams.publish(&session_id, event.clone())` 紧邻 `on_output`（先发布后出流）、密封臂在 `append_sealed` 成功后同样发布；`converge` 增 `started_at` 与 `streams` 两参：`outcome.started_at` 落定 → `sink.finish_turn` → `streams.finish(&session_id, outcome.clone())`（投终态即除名）→ `registry.remove` → `on_output(TurnFinished)` | 写入单点（exec / change 双轨同源，`worker.rs` 零改动） |
| `crates/infra/agent/src/compose.rs` | `compose_turn(stores, registry, streams, store, agent)` 增第三参（`Arc<SessionStreamRegistry>`，doc 沿 `StopRegistry` 注入先例）；`SessionKernel::new(StoreSink::new(store), registry, streams)` | 组合根注入缝 |
| `src/commands/exec/mod.rs` | 新增 `agent_session_watch(app, on_event, root, session_id, since_seq) -> Result<(), String>` 与泛型缝 `agent_session_watch_with<R: tauri::Runtime>`：`is_blank_root` → `Ok(())`；`app.state::<Arc<SessionStreamRegistry>>()` clone → `subscribe(&session_id, since_seq)`，`None` → `Ok(())`；`Some(sub)` → 循环 `sub.recv().await`：`Event` → `on_event.send(AgentRunMessage::Event { event })`（失败即断）→ `Finished(outcome)` → `on_event.send(AgentRunMessage::Record { record: agent::outcome_summary(&outcome, &session_id) })` 后 break；`Err(Lagged(_) / Closed)` → break（不投伪终态）。`agent_start_with` 侧：`compose_turn` 增注册表实参、`outcome_summary` 改两参、删除已失效的 `let started_at` 局部 | 订阅命令薄包装（D5 / D7） |
| `src/commands/exec/agent.rs` | `outcome_summary` 签名去掉 `started_at` 入参（改取 `outcome.started_at`），doc 随动 | `TurnOutcome` 自足化的净简化（D4） |
| `src/commands/mod.rs` | `all_commands!` 增 `$crate::commands::exec::agent_session_watch` | 单一登记面（builder 与 `generate_handler!` 同源） |
| `src/commands/change_flow/mod.rs` | `compose_turn` 调用点注入 `Arc::clone(app.state::<Arc<SessionStreamRegistry>>().inner())` | change 轨同源订阅（`worker.rs` 零改动） |
| `src/main.rs` | setup 增 `app.manage(Arc::new(SessionStreamRegistry::default()));`（注记与 `StopRegistry` 同型：进程内存态、重启即清空） | 壳层托管 |
| `src/types/generated/bindings.ts` | 再生成（新增 `agentSessionWatch` typed 包装：`onEvent: Channel<AgentRunMessage>` + `sinceSeq: number`） | 生成物入库（禁手改） |

### 修改文件 — 前端（`packages/desktop/src`）

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `lib/agent-adapter.ts` | IR 三层落地：私有 `type BlockPlan`（`text` / `reasoning` / `tool`，`tool.output: null` = 未配对）、`type PairedResults`、`const PairingScope = { global(events), local(blocks) }`、`planBlock(block, paired)`（唯一判定点 + `const _exhaustive: never`）、`planBlocks(blocks, paired)`、`plansToParts(plans)`、`plansToChunks(plans, seq)`（part id 取计划下标 `t-${seq}-${i}` / `r-${seq}-${i}`）；`eventsToUIMessages` 改 `PairingScope.global` + `plansToParts`；`eventToChunk` 的 message 臂改 `PairingScope.local` + `plansToChunks`；`auxEventData` 的 `default` 改「`const _exhaustive: never` + runtime `data-raw` 兜底」双层；删除 `blocksToParts` / `blocksToChunks` / `collectToolResults` / `toolUsePart` | IR 化（D11） |
| `lib/agent-transport.ts` | 抽 `openEnvelopeStream(channel)` 簿记单点（内部持有 `openedParts` / `closed`，`accept(message)` 分派事件 / `Record`，`end()` 关流、`fail(cause)` 注错）；`sendMessages` 改为簿记单点 + `invokeStart`（异常走 `fail`）；`reconnectToStream(options)` 实现：解析 `body`（`root` / `sessionId` / `sinceSeq`，非法或 `root` / `sessionId` 缺席 → `null`）→ `commands.sessionDetail` 状态门（无 running 轮行 → `null`）→ 建 `Channel` + 簿记流 → `commands.agentSessionWatch(channel, root, sessionId, sinceSeq).then(end).catch(fail)` → 返回流；`abortSignal` abort → `end()` | 双起点 + 单簿记（D6 / D7） |
| `components/agent/index.tsx` | 增 `export { AgentSessionView } from './agent-session-view';` 与其 props / actions 类型导出 | 组件族出口 |
| `views/agent/agent-debug-view.tsx` | `useAgentChat` → `useAgentSession({ root, sessionId: null, source: 'debug' })`；时间线位改 `<AgentSessionView session={session} data-testid="agent-timeline" />`；`AgentRawStream events={session.events}`、`StreamToggle running={session.running}`、`stop={session.stop}`、表单 `disabled={session.running}` 与 `start`（`reset()` + `sendMessage`）照旧；`<AgentRunHistory root={root} … />` 透传 root | 交互形态经注入形态（D9） |
| `views/agent/components/agent-run-history.tsx` | 新增 `root: string \| null` prop；`ReplayArea` 改为 `<AgentSessionView root={root} sessionId={state.selectedSessionId} data-testid="replay-area-session" />`（删除 `eventsToUIMessages` 直调与 `events` prop）；`onClose` 接 `state.closeSession` | 只读重放经自持形态（D14） |
| `views/agent/hooks/use-agent-run-history.ts` | `useReplay` 退役（`events` 状态与 `agentSessionTranscript` 直调删除）；`openSession` 只置 `selectedSessionId`，新增 `closeSession()`；`AgentRunHistoryState` 去 `events`、增 `closeSession`，`loading` / `error` 只取清单查询 | 自有转录查询退役（D14） |
| `views/explores/hooks/use-explore-session.ts` | 改走 `useAgentSession`（`source: 'explore'`）；返回值收敛为 `{ session: UseAgentSessionState; sessionId: string \| null; send; stop }`（`send` 保留 stance 拼接 + 未就绪守卫，`useAutoSend` 闸门改读 `session.loading` / `session.chain`）；`ExploreSendInput` 不变 | 来源侧组装 / auto-send 门留存 |
| `views/explores/explore-detail-view.tsx` | `ChatPane` 改 `<AgentSessionView session={session.session} composer={({ running }) => <ExploreComposer running={running} onSend={session.send} onStop={session.stop} />} data-testid="explore-chat" />`；`session.running` 用于终态定点重读的逻辑不变（库态 running 转 false 即触发） | 交互形态 + 插槽（D9 / D10） |
| `views/explores/components/explore-composer.tsx` | props 面不变（`running` / `onSend` / `onStop`），注释随动：由 `AgentSessionView` 插槽注入（`running` 取自 `AgentSessionActions`） | 文案与 testid 前缀留来源侧 |
| `views/changes/flow/session-transcript-panel.tsx` | 删 `useSessionTranscript` 调用与本地 `SessionMeta`，改 `<AgentSessionView root={root} sessionId={roleRef.sessionId} refreshKey={refreshKey} data-testid="session-transcript" />`；保留 `session-transcript-panel` / `drawer-session-empty`（槽位缺席）两个 wrapper testid；doc 注明 `refreshKey` 语义 = 会话状态重评估 | 只读形态（D12） |
| `views/changes/flow/detail-drawer.tsx` | `transcriptRefreshKey` 改名 `sessionRefreshKey`（doc：会话状态重评估信号），透传至面板 `refreshKey` | 语义改名 |
| `views/changes/hooks/use-change-detail.ts` | 删 `TRANSCRIPT_REFRESH_DEBOUNCE_MS` / `useDebouncedTick` / `notifyTranscript` / `transcriptTick`；`ChangeDetailState` 增 `refreshTick: number`（统一 tick 导出）替代 `transcriptTick` | 去抖链退役（D13） |
| `views/changes/change-detail-view.tsx` | `onNotice` 不再分流（`SessionEvent` 与其余同走 `notifyRefresh`）；`transcriptTick` → `refreshTick`，下传 `sessionRefreshKey`；`DetailContent` / `DetailLoaded` 形参同步改名 | 通知仅作状态重评估（D13） |
| `packages/desktop/package.json` | `version` 0.5.16 → 0.5.17；`check` 改 `pnpm run server:check && pnpm run bindings:check && pnpm run client:check`（`bindings:check` 已存在，不改；`build` / `bindings:export` 维持导出） | 一致性守卫接入 |

### 删除文件

| 文件路径 | 原职责 | 去向 |
|----------|--------|------|
| `packages/desktop/src/hooks/use-agent-chat.ts` | 会话状态镜像 + 发送方独占流 | 收编进 `hooks/use-agent-session.ts`（MUST NOT 保留双 hook） |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 会话直查 + 转录重放 + `liveEvents` 死参数 | 收编进 `AgentSessionView` / `useAgentSession`（MUST NOT 保留第二套转录查询 hook） |

> 测试文件（新增 / 重塑 / 删除）不在本清单，由测试相位承接（见 proposal「变更范围 - 测试文件」）。

### 公共函数 / API

**Rust — `crates/core/agent/src/stream.rs`（新增）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `SessionStreamRegistry::new` | `stream.rs` | 新增 | `pub fn new() -> Self` | 空注册表（`Default` 实现，`Mutex<HashMap<String, broadcast::Sender<SessionStreamItem>>>`） |
| `SessionStreamRegistry::begin` | `stream.rs` | 新增 | `pub fn begin(&self, session_id: &str)` | 轮登记：为该会话建容量 `SESSION_STREAM_CAPACITY` 的 broadcast；同键重复登记直接覆盖（**无重入守卫**，沿 `StopRegistry` 既存形态，并发两轮不在本变更语义内——proposal 风险 5 留痕） |
| `SessionStreamRegistry::publish` | `stream.rs` | 新增 | `pub fn publish(&self, session_id: &str, event: AgentEvent)` | 事件投递（含 delta）；无条目 / 无订阅者静默丢弃（`let _ = sender.send(..)`），不产生错误 |
| `SessionStreamRegistry::finish` | `stream.rs` | 新增 | `pub fn finish(&self, session_id: &str, outcome: TurnOutcome)` | 投终态 `SessionStreamItem::Finished` 后**除名**（条目移除；既有订阅者排空缓冲后收到 `Closed`） |
| `SessionStreamRegistry::subscribe` | `stream.rs` | 新增 | `pub fn subscribe(&self, session_id: &str, since_seq: u64) -> Option<SessionStreamSubscription>` | 订阅句柄；无运行轮（无条目）返回 `None` —— 「幂等立即结束」的判据 |
| `SessionStreamSubscription::recv` | `stream.rs` | 新增 | `pub async fn recv(&mut self) -> Result<SessionStreamItem, SessionStreamRecvError>` | 只返 `seq > since_seq` 的事件（终态恒返）；丢弃不合格条目后继续等待；`Lagged` / `Closed` 显式区分 |

**Rust — `crates/core/agent/src/kernel.rs`（修改）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `SessionKernel::new` | `kernel.rs` | 修改 | `pub fn new(sink: Arc<dyn SessionSink>, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>) -> Self` | 注入流注册表（第三参） |
| `SessionKernel::begin_turn` | `kernel.rs` | 修改 | `pub fn begin_turn(&self, runner: Arc<dyn AgentRunner>, request: TurnRequest) -> Result<RunningTurn, AgentStartError>`（签名不变，内部增 `streams.begin`） | 轮登记先于驱动，订阅方可即刻接入 |
| `RunningTurn::drive` | `kernel.rs` | 修改 | `pub async fn drive<F>(self, on_output: F) -> TurnOutcome where F: FnMut(KernelOutput)`（签名不变，内部增单点发布） | `Observation` 单点发布（delta 与密封同源） |

**Rust — `crates/infra/agent/src/compose.rs`（修改）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `compose_turn` | `compose.rs` | 修改 | `pub fn compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, store: Arc<Store>, agent: Option<i64>) -> Result<ComposedTurn, String>` | 注册表注入缝（`ComposedTurn::begin` 签名不变） |

**Rust — `packages/desktop/src-tauri/src/commands/exec/mod.rs`（修改）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `agent_session_watch` | `commands/exec/mod.rs` | 新增 | `pub async fn agent_session_watch(app: AppHandle, on_event: Channel<AgentRunMessage>, root: String, session_id: String, since_seq: u64) -> Result<(), String>` | IPC 命令（薄包装）：登记订阅 → `Channel` 前向（`Event` / 末拍 `Record`）→ 结束释放；blank root / 无运行轮幂等 `Ok(())` |
| `agent_session_watch_with` | `commands/exec/mod.rs` | 新增 | `pub(crate) async fn agent_session_watch_with<R: tauri::Runtime>(app: AppHandle<R>, on_event: Channel<AgentRunMessage>, root: String, session_id: String, since_seq: u64) -> Result<(), String>` | 泛型测试缝（生产经 `agent_session_watch` 注入 Wry 句柄，测试经 `mock_app`，沿 `agent_start_with` 先例） |
| `outcome_summary` | `commands/exec/agent.rs` | 修改 | `pub(crate) fn outcome_summary(outcome: &TurnOutcome, session_id: &str) -> TurnSummary` | `started_at` 改取 `outcome.started_at` |

**Rust — `crates/core/agent/src/lib.rs`（修改）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `SessionStreamRegistry` / `SessionStreamItem` / `SessionStreamSubscription` / `SessionStreamRecvError` | `lib.rs` | 新增 | `pub use stream::{…}`（类型见下表） | 注册表出口（`stream` 模块本身私有：`mod stream;`） |

**TypeScript — `lib/agent-adapter.ts`（修改；IR 三层为模块私有，见 D11）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `eventsToUIMessages` | `lib/agent-adapter.ts` | 修改（实现） | `(events: AgentEvent[]) => AgentUIMessage[]` | 重放 / 收口折叠：`PairingScope.global` 视野 + `plansToParts` 发射（签名不变） |
| `eventToChunk` | `lib/agent-adapter.ts` | 修改（实现） | `(event: AgentEvent) => AgentUIMessageChunk[]` | 实时增量：`PairingScope.local` 视野 + `plansToChunks` 发射；事件头（`start` / `reset-step`）仍在此（签名不变） |
| `runRecordToUIMessage` | `lib/agent-adapter.ts` | 不变 | `(record: TurnSummary) => AgentUIMessage` | 终态轮行 → `data-run-record` 部件消息 |
| `provisionalMessageId` | `lib/agent-adapter.ts` | 不变 | `(parentToolUseId: string \| null) => string` | provisional 消息键 |
| `deltaPartId` | `lib/agent-adapter.ts` | 不变 | `(key: string, part: 'text' \| 'thinking') => string` | delta 累积部件 id |

**TypeScript — `lib/agent-transport.ts`（修改）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `TauriAgentTransport.sendMessages` | `lib/agent-transport.ts` | 修改（实现） | `(options: { trigger: 'submit-message' \| 'regenerate-message'; chatId: string; messageId: string \| undefined; messages: AgentUIMessage[]; abortSignal: AbortSignal \| undefined; body?: object }) => Promise<ReadableStream<AgentUIMessageChunk>>` | 发起流（`agent_start`，Channel 先于 invoke，零空洞）；簿记走 `openEnvelopeStream` 单点 |
| `TauriAgentTransport.reconnectToStream` | `lib/agent-transport.ts` | 修改（实现） | `(options: { chatId: string; abortSignal?: AbortSignal; body?: object }) => Promise<ReadableStream<AgentUIMessageChunk> \| null>` | 订阅流：`body` 含 `root` / `sessionId` / `sinceSeq`；无运行轮 / 参数非法 → `null`；invoke 解析或 abort → 关流 |

**TypeScript — `hooks/use-agent-session.ts`（新增，收编 `use-agent-chat.ts`）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `useAgentSession` | `hooks/use-agent-session.ts` | 新增（收编） | `(params: UseAgentSessionParams) => UseAgentSessionState` | 重放装载 + 状态驱动挂流 + 单流在挂 + 收口以重放收敛（失败回落镜像折叠）+ 断流重订 + 发送 / 停止 / 重置；`running` 为库态派生（轮行含 `running`），MUST NOT 以 `chat.status` 替代 |

**TypeScript — `components/agent/agent-session-view.tsx`（新增）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `AgentSessionView` | `components/agent/agent-session-view.tsx` | 新增 | `(props: AgentSessionViewProps) => React.JSX.Element` | 会话级唯一渲染入口（双形态；元信息 + 错误 / 加载 / 空态 + `AgentMessages` + composer 插槽） |

**TypeScript — 消费面 hook（修改）**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `useChangeDetail` | `views/changes/hooks/use-change-detail.ts` | 修改 | `(root: string \| null, id: string \| null) => ChangeDetailState` | 返回值去 `notifyTranscript` / `transcriptTick`，增 `refreshTick` |
| `useExploreSession` | `views/explores/hooks/use-explore-session.ts` | 修改 | `(root: string \| null, record: ExploreRecord \| null, autoPrompt?: string \| null) => ExploreSessionState` | 改走 `useAgentSession`；stance 组装与 auto-send 门留来源侧 |
| `useAgentRunHistory` | `views/agent/hooks/use-agent-run-history.ts` | 修改 | `(root: string \| null, source: AgentHistorySource) => AgentRunHistoryState` | `events` 退役、增 `closeSession` |

### 类型定义

**Rust**

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `SessionStreamRegistry` | `crates/core/agent/src/stream.rs` | 新增 | `pub struct`：`sessions: Mutex<HashMap<String, broadcast::Sender<SessionStreamItem>>>`；derive `Default`；方法见上表 |
| `SessionStreamItem` | `crates/core/agent/src/stream.rs` | 新增 | `#[derive(Debug, Clone)] pub enum SessionStreamItem { Event(AgentEvent), Finished(TurnOutcome) }`（core 中性载体，非 IPC 信封、不 derive `specta::Type`、不序列化） |
| `SessionStreamSubscription` | `crates/core/agent/src/stream.rs` | 新增 | `pub struct`：`receiver: broadcast::Receiver<SessionStreamItem>` + `since_seq: u64`（过滤与 lagged 判别的内聚点） |
| `SessionStreamRecvError` | `crates/core/agent/src/stream.rs` | 新增 | `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum SessionStreamRecvError { Lagged(u64), Closed }`（`Lagged` 携丢弃条数；两态都由订阅端断流收尾） |
| `SessionKernel` | `crates/core/agent/src/kernel.rs` | 修改 | 增字段 `streams: Arc<SessionStreamRegistry>` |
| `RunningTurn` | `crates/core/agent/src/kernel.rs` | 修改 | 增字段 `streams: Arc<SessionStreamRegistry>`（私有） |
| `TurnOutcome` | `crates/core/agent/src/port.rs` | 修改 | 增 `pub started_at: i64`（wire `startedAt`） |
| `AgentRunMessage` | `packages/desktop/src-tauri/src/commands/exec/agent.rs` | 不变 | 订阅命令复用同一信封（`Event` / `Record`），`event` / `record` 载荷为「载荷权威」直投 |

**TypeScript**

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `BlockPlan` | `lib/agent-adapter.ts` | 新增（模块私有） | `{ kind: 'text'; text: string } \| { kind: 'reasoning'; text: string } \| { kind: 'tool'; toolCallId: string; toolName: string; input: unknown; output: AgentToolOutput \| null }`（`output: null` = 未配对）；块语义唯一表达，不含发射格式 / id / 视野 |
| `PairedResults` | `lib/agent-adapter.ts` | 新增（模块私有） | `Map<string, ToolResultBlock>`（两视野同形态；差异只在产出方式） |
| `PairingScope` | `lib/agent-adapter.ts` | 新增（模块私有） | `{ global: (events: SealedAgentEvent[]) => PairedResults; local: (blocks: AgentBlock[]) => PairedResults }`（`as const`；视野差异唯一隔离点） |
| `UseAgentSessionParams` | `hooks/use-agent-session.ts` | 新增 | `{ root: string \| null; sessionId: string \| null; source: string; refreshKey?: number }` |
| `UseAgentSessionState` | `hooks/use-agent-session.ts` | 新增 | `{ messages: AgentUIMessage[]; events: AgentEvent[]; session: SessionSummary \| null; chain: TurnSummary[]; loading: boolean; running: boolean; error: string \| null; sendMessage: (input: AgentSessionSendInput) => void; stop: () => void; reset: () => void }`（`running` = 库态：`chain.some(t => t.status === 'running')`） |
| `AgentSessionSendInput` | `hooks/use-agent-session.ts` | 新增 | `{ prompt: string; permissionMode: AgentPermissionMode \| null; agent?: number \| null }`（原 `AgentChatSendInput` 平移） |
| `AgentSessionViewProps` | `components/agent/agent-session-view.tsx` | 新增 | `{ root?: string \| null; sessionId?: string \| null; refreshKey?: number; session?: UseAgentSessionState; composer?: (actions: AgentSessionActions) => React.ReactNode; 'data-testid'?: string }`（`session` 在场即注入形态，否则自持形态） |
| `AgentSessionActions` | `components/agent/agent-session-view.tsx` | 新增 | `{ running: boolean; loading: boolean; send: (input: AgentSessionSendInput) => void; stop: () => void }`（composer 插槽入参） |
| `ExploreSessionState` | `views/explores/hooks/use-explore-session.ts` | 修改 | `{ session: UseAgentSessionState; sessionId: string \| null; send: (input: ExploreSendInput) => void; stop: () => void }`（原 `messages` / `chain` / `events` / `loading` / `running` / `error` 平铺字段收敛进 `session`） |
| `AgentRunHistoryState` | `views/agent/hooks/use-agent-run-history.ts` | 修改 | 去 `events`，增 `closeSession: () => void`（`loading` / `error` 只取清单查询） |
| `ChangeDetailState` | `views/changes/hooks/use-change-detail.ts` | 修改 | 去 `notifyTranscript` / `transcriptTick`，增 `refreshTick: number` |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `SESSION_STREAM_CAPACITY` | `crates/core/agent/src/stream.rs` | 新增（常量） | `usize` | `4096` | per-session broadcast 容量（一轮 delta 突发定价，D1） |
| `TRANSCRIPT_REFRESH_DEBOUNCE_MS` | `views/changes/hooks/use-change-detail.ts` | 删除（常量） | — | — | 150ms 转录专用去抖退役（D13；`DETAIL_REFRESH_DEBOUNCE_MS = 300` 保留） |
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.17`（原 `0.5.16`） | 插件/桌面端版本纪律 |
| `check` | `packages/desktop/package.json` | 修改 | `string`（npm script） | `pnpm run server:check && pnpm run bindings:check && pnpm run client:check` | 一致性守卫（`bindings:check` = 导出 + `git diff --exit-code -- src/types/generated`） |
| `bindings:export` / `bindings:check` / `build` | `packages/desktop/package.json` | 不变 | `string` | — | `build` 维持导出（产物构建需最新生成物） |

---

## 数据模型

### 进程内存态 —— 会话流注册表（无持久化）

```
SessionStreamRegistry
└── sessions: Mutex<HashMap<session_id: String, broadcast::Sender<SessionStreamItem>>>
      ├── broadcast 容量 = SESSION_STREAM_CAPACITY (4096)
      └── 条目生命周期 = 一轮
            begin(session_id)                    ← kernel::begin_turn
            publish(session_id, AgentEvent)      ← kernel::drive（delta + 密封，单点）
            finish(session_id, TurnOutcome)      ← kernel::converge（投终态 → 除名）
```

- **键**：core session id（`ses-<count>-<millis>`，core 铸造，进程内全局唯一免复合键——与 `StopRegistry` 同键口径）。
- **值**：`SessionStreamItem`，两变体：`Event(AgentEvent)`（`seq > since_seq` 过滤，含 `messageDelta`）、`Finished(TurnOutcome)`（含 `started_at`，恒投递、末条、其后条目移除）。
- **持久化**：无。进程内存态，应用重启即清空（与 `StopRegistry` 同型）。MUST NOT 落库（delta 永不落库红线不变）；`TurnOutcome.started_at` 不写库列（store 侧 `finish_turn` 沿用存量行）。
- **关系**：`session 1 ── 0..1 条目`（一轮至多一个条目：并发两轮同键为覆盖语义，非本变更语义范围）；`条目 1 ── N 订阅者`（broadcast 扇出；订阅者慢消费触发 lagged → 各自断流）。

### 前端状态模型（`useAgentSession`）

```
镜像（ref 真相 + state 渲染双轨）
├── session: SessionSummary | null      ← replay / early-resolve 轮行回写
├── turns: TurnSummary[]                ← 轮行 upsert（turnId 键）
├── events: AgentEvent[]                ← 密封事件 append 保序（delta 不入镜像）
└── seqCursor: number                   ← 已观测事件最大 seq（含 delta；D8）

chat 状态（ai-sdk useChat）
├── messages: AgentUIMessage[]          ← 实时 chunk 累积 + 重放折叠重建（同一折叠入口）
└── status: submitted | streaming | ready | error   ← 在挂流判据（单流在挂）

派生
├── running = turns.some(t => t.status === 'running')   ← 库态（MUST NOT 用 chat.status）
└── loading ← 重放装载在途
```

- **单流在挂不变式**：`status ∈ {submitted, streaming}` 即「有在挂流」，订阅建立条件恒要求 `status ∉ {submitted, streaming}`（一条判据同时挡住双流与中途重放）。
- **订阅前游标**：`sinceSeq = seqCursor`；每次收到事件（含 delta）与每次重放装载都推高游标 → 重订不重投。
- **无进展闸**：`blocked = ${targetId}#${refreshKey}#${订阅前游标}`（D15）。

---

## 路由 / API 设计

无 HTTP 路由；新增 1 个 Tauri IPC 命令。

| method（IPC 命令） | path | 说明 | 输入 | 输出 | auth |
|--------------------|------|------|------|------|------|
| `agent_session_watch` | — | 会话级实时流订阅（薄包装：订阅 → `Channel` 前向 → 结束释放） | `onEvent: Channel<AgentRunMessage>`（typed）、`root: string`、`sessionId: string`、`sinceSeq: number`（Rust `u64`） | `Result<(), String>`（`Ok` 于轮终态 / 断流 / 无运行轮；`Err` 仅注册表不可用等异常） | 无（本地应用，沿既有命令口径） |

信封（复用 `AgentRunMessage`，`tag = "ipc"`，camelCase）：

```
{ ipc: 'event',  event:  AgentEvent }   // 载荷直投（含 messageDelta，seq 严格递增）
{ ipc: 'record', record: TurnSummary }  // 轮终态轮行（末拍；随后命令结束）
```

既有命令零改动：`agent_start`（`Channel` 实时出口保留，与订阅面同源）、`agent_stop`、`agent_sessions`、`agent_session_transcript`、`session_detail`。

---

## 依赖

运行时依赖：**无新增**。

| 依赖 | 变更 | 用途 |
|------|------|------|
| `tokio`（`sync` feature） | 不变 | `crates/core/agent` 已依赖 workspace `tokio`（`features = ["sync"]`），`broadcast` / `Mutex` 均在该 feature 内 |
| `tauri` / `tauri-specta` | 不变 | 订阅命令的 `Channel<AgentRunMessage>` typed 绑定（`all_commands!` 登记后自动纳入） |
| `ai` / `@ai-sdk/react` | 不变 | `ChatTransport.reconnectToStream` / `resumeStream({ body })` 已在 v7 面内（已核对签名与 `makeRequest` 收尾语义） |

构建 / 测试依赖：**无新增**（沿用 `vp check` / `knip` / `cargo clippy` / `vitest` / `cargo test`）。

---

## 待决问题

1. **lagged 升级路径**：若「一轮稠密 delta 反复溢出」在实测中成为常态（容量 4096 仍不够），spec 允许的「先补发库内密封快照再续接 live」实现路线（D3 的备选）可作为后续独立变更；本期以断流重订兜底。
2. **断流期的视觉跳动**：断流 → `close()` 使 `status` 复位 `ready` → 重放重订后重新 `streaming`，中间可能出现一帧「非运行中」徽章；是否需要「重连中」提示面未定（本期不做，等实测观感）。
3. **库态 running 但无实时流**（应用重启遗留 running 轮行）：受 D15 保护不形成 invoke 自旋，但 composer 会长期处于禁发态（停止命令 miss 幂等、可点）。是否需要「对账收敛」把无句柄的 running 轮行收口为 failed，属既有 `reconcile_stats` 面的扩展，另立变更。
4. **`TurnOutcome` 增字段的测试随动面**：`crates/core/agent/src/port_test.rs` 的 serde 往返极值断言与 `store_port_test.rs` 的终态夹具需补 `started_at`——由测试相位承接（proposal 已预告 `agent_test.rs`「如签名有变」的随动面）。
5. **半截文本窗口**：旁观中途接入时首个 delta 无前文（V6 只验「能归约」未验视觉连续性），按「过程近似」接受；若需视觉连续，须引入「快照补发」升级路径（与待决 1 同源）。
