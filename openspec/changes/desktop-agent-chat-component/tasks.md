# 任务: desktop-agent-chat-component

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接（含 `core/agent` 注册表单测、`kernel_test.rs` 双轨同源、`infra/agent/worker_test.rs` 组合根随动、`commands/exec/mod_test.rs` 订阅命令六面、`lib/agent-adapter.test.ts` 计划层对拍与视野快照、`lib/agent-transport.test.ts` 订阅流、`hooks/use-agent-session.test.ts`、`components/agent/agent-session-view.test.tsx`、四个消费点视图测试随动、`use-session-transcript.test.ts` 删除），本列表只覆盖实现文件。
>
> 依赖序要点：注册表（阶段一）是内核写入单点（阶段二）的前置，`TurnOutcome.started_at`（阶段二）是订阅命令 `Record` 投影（阶段三）的前置；组合根签名变更（阶段三）连坐两处调用点（exec / change_flow）——须同批落地编译才绿；bindings 再生（阶段四）依赖 Rust 命令面定稿并先于前端消费；前端适配层 IR（阶段五）先于 transport 与 hook（阶段六 / 七，二者消费 `eventToChunk` 与簿记）；组件（阶段八）先于四消费点迁移（阶段九，`use-agent-chat` / `use-session-transcript` 删除在消费点全部切换之后）。

## 阶段一：core/agent 会话流注册表

- [x] 新增 `packages/desktop/src-tauri/crates/core/agent/src/stream.rs`：私有常量 `SESSION_STREAM_CAPACITY: usize = 4096`（doc：一轮 delta 突发定价，溢出即 lagged 由订阅侧断流重订兜底）；`pub enum SessionStreamItem { Event(AgentEvent), Finished(TurnOutcome) }`（derive `Debug, Clone`；不 derive `specta::Type`、不序列化——core MUST NOT 认识 IPC 信封）；`pub enum SessionStreamRecvError { Lagged(u64), Closed }`（derive `Debug, Clone, Copy, PartialEq, Eq`）
- [x] 新增 `stream.rs`：`pub struct SessionStreamRegistry { sessions: Mutex<HashMap<String, broadcast::Sender<SessionStreamItem>>> }`（derive `Default`）+ 四方法——`new()`、`begin(&self, session_id: &str)`（建容量 `SESSION_STREAM_CAPACITY` 的 broadcast，同键覆盖并注记无重入守卫沿 `StopRegistry`）、`publish(&self, session_id: &str, event: AgentEvent)`（无条目 / 无订阅者静默 `let _ =`）、`finish(&self, session_id: &str, outcome: TurnOutcome)`（先 `send(Finished)` 再从表除名）、`subscribe(&self, session_id: &str, since_seq: u64) -> Option<SessionStreamSubscription>`
- [x] 新增 `stream.rs`：`pub struct SessionStreamSubscription { receiver: broadcast::Receiver<SessionStreamItem>, since_seq: u64 }` + `pub async fn recv(&mut self) -> Result<SessionStreamItem, SessionStreamRecvError>`——`Event` 过滤 `event.seq > since_seq` 后继续等待、`Finished` 恒返；`Lagged(n)` → `Err(Lagged(n))`、`Closed` → `Err(Closed)`
- [x] 修改 `packages/desktop/src-tauri/crates/core/agent/src/lib.rs`：增 `mod stream;` 与 `pub use stream::{SessionStreamItem, SessionStreamRecvError, SessionStreamRegistry, SessionStreamSubscription};`（模块本身私有，仅类型出口）

## 阶段二：内核写入单点与轮收口形状

- [x] 修改 `packages/desktop/src-tauri/crates/core/agent/src/port.rs`：`TurnOutcome` 增 `pub started_at: i64`（doc：轮起点，wire `startedAt`；终态轮行装配与订阅面 `Record` 投影共用，store 侧 `finish_turn` 按 `turn_id` 更新、沿用存量行的 `started_at`，不写该列）
- [x] 修改 `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs`：`SessionKernel` 增私有字段 `streams: Arc<SessionStreamRegistry>`，`pub fn new(sink: Arc<dyn SessionSink>, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>) -> Self`
- [x] 修改 `kernel.rs`：`begin_turn` 在 `self.registry.register(...)` 之后、返回 `RunningTurn` 之前调 `self.streams.begin(&session_id)`（轮登记先于驱动，订阅方可即刻接入）；`RunningTurn` 增私有字段 `streams: Arc<SessionStreamRegistry>`，构造处透传
- [x] 修改 `kernel.rs`：`drive` 的 delta 臂改「先发布后出流」——`streams.publish(&session_id, event.clone()); on_output(KernelOutput::Observation(event)); continue;`；密封事件在 `sink.append_sealed` 成功分支之后、`on_output` 之前同样 `streams.publish(&session_id, event.clone())`（delta 与密封同源同戳，落库 → 发布 → 出流三序固定）
- [x] 修改 `kernel.rs`：`converge` 增 `started_at: i64` 与 `streams: &SessionStreamRegistry` 两参（保持 `#[allow(clippy::too_many_arguments)]`）；`outcome` 构造补 `started_at`；收口序改为 `sink.finish_turn` → `streams.finish(session_id, outcome.clone())`（投终态即除名）→ `registry.remove(session_id)` → `on_output(TurnFinished)`；`drive` 三个 `converge` 调用臂补 `started_at`（从 `RunningTurn` 解构取得，需在 `destructure` 里显式取 `started_at`）

## 阶段三：组合根、壳层托管与订阅命令

- [x] 修改 `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`：`compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, store: Arc<Store>, agent: Option<i64>) -> Result<ComposedTurn, String>`（doc 沿 `StopRegistry` 注入先例：壳层 `Arc` 单例经组合根下行内核）；`SessionKernel::new(Arc::new(StoreSink::new(store)), registry, streams)`
- [x] 修改 `packages/desktop/src-tauri/src/commands/exec/agent.rs`：`outcome_summary(outcome: &TurnOutcome, session_id: &str) -> TurnSummary`（去掉 `started_at` 入参，改取 `outcome.started_at`；doc 随动——终态收口已携起点）
- [x] 修改 `packages/desktop/src-tauri/src/commands/exec/mod.rs`：`agent_start_with` 的 `compose_turn` 调用增注册表实参（`Arc::clone(app.state::<Arc<SessionStreamRegistry>>().inner())`，先取 `Arc` 再进入 spawn，避免跨 await 持 `State`）；`TurnFinished` 臂的 `outcome_summary` 调用改两参形态，并删除随之失效的 `let started_at = running.started_at;`（终态起点已由 `TurnOutcome` 携带，避免未用变量告警）
- [x] 修改 `packages/desktop/src-tauri/src/commands/exec/mod.rs`：新增 `#[tauri::command] #[specta::specta] pub async fn agent_session_watch(app: AppHandle, on_event: Channel<AgentRunMessage>, root: String, session_id: String, since_seq: u64) -> Result<(), String>` 与泛型缝 `pub(crate) async fn agent_session_watch_with<R: tauri::Runtime>(...)`（沿 `agent_start_with` 先例）——`is_blank_root(&root)` → `Ok(())`；取 `Arc<SessionStreamRegistry>` → `subscribe(&session_id, since_seq)`，`None` → `Ok(())`；`Some(mut sub)` → `loop`：`Ok(Event(event))` → `on_event.send(AgentRunMessage::Event { event })` 失败即 break；`Ok(Finished(outcome))` → `on_event.send(AgentRunMessage::Record { record: agent::outcome_summary(&outcome, &session_id) })` 后 break；`Err(Lagged(_) | Closed)` → break（**不投伪终态**）；doc 注明「按轮收流，命令结束即订阅释放」
- [x] 修改 `packages/desktop/src-tauri/src/commands/mod.rs`：`all_commands!` 增 `$crate::commands::exec::agent_session_watch,`（builder 注册与 `generate_handler!` 单一登记面）
- [x] 修改 `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`：`compose_turn` 调用点注入注册表（`Arc::clone(app.state::<Arc<SessionStreamRegistry>>().inner())`）——change 轨 walker 会话同源进入同一注册表，`infra/agent/src/worker.rs` 零改动
- [x] 修改 `packages/desktop/src-tauri/src/main.rs`：setup 增 `app.manage(Arc::new(SessionStreamRegistry::default()));`（注记与 `StopRegistry` 同型：内核 `begin_turn` 登记 / `converge` 投终态除名 / `agent_session_watch` 订阅，进程内存态重启即清空）；`use agent::SessionStreamRegistry;` 导入

## 阶段四：bindings 再生与检查守卫

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`（新增 `agentSessionWatch: (onEvent: Channel<AgentRunMessage>, root: string, sessionId: string, sinceSeq: number) => __TAURI_INVOKE<null>`；非手改）
- [x] 修改 `packages/desktop/package.json`：`version` `0.5.16` → `0.5.17`；`check` 改 `pnpm run server:check && pnpm run bindings:check && pnpm run client:check`（`bindings:check` 与 `bindings:export` / `build` 不动）；先重导再启守卫（守卫首跑须工作区干净）

## 阶段五：前端适配层 IR 化

- [x] 修改 `packages/desktop/src/lib/agent-adapter.ts`：引入私有 `type BlockPlan`（`text` / `reasoning` / `tool`，`tool.output: null` 即未配对）、`type PairedResults = Map<string, ToolResultBlock>`、`const PairingScope = { global(events), local(blocks) } as const`（`global` 收编 `collectToolResults` 的跨消息预扫语义、`local` 仅当前 message 的 `toolUse` ↔ `toolResult` 匹配；两视野同产出 `PairedResults`）
- [x] 修改 `lib/agent-adapter.ts`：新增私有 `planBlock(block: AgentBlock, paired: PairedResults): BlockPlan | null`（唯一语义点：text / thinking → 对应变体；toolUse → `output` 取 `paired.get(id)`；toolResult → 已配对返 `null`、孤儿合成 `{ kind: 'tool', toolName: ORPHAN_TOOL_NAME, input: null, output }`；switch 后以 `const _exhaustive: never = block` 承接穷尽）+ `planBlocks(blocks, paired): BlockPlan[]`
- [x] 修改 `lib/agent-adapter.ts`：新增私有 `plansToParts(plans): AgentUIMessage['parts']` 与 `plansToChunks(plans, seq): AgentUIMessageChunk[]`（纯机械：text → `{type:'text',state:'done'}` / `text-start|delta|end`（id `t-${seq}-${i}`，i = 计划下标）；reasoning 同理 `r-${seq}-${i}`；tool `output=null` → 单 `input-available` / `tool-input-available`；`output≠null` → `output-available` 单部件 / `tool-input-available` + `tool-output-available` 两 chunk）；**不产事件头**（`start` / `reset-step` 仍归 `eventToChunk`）
- [x] 修改 `lib/agent-adapter.ts`：`eventsToUIMessages` 改 `PairingScope.global` 视野 + `eventToUIMessage` 内 `plansToParts(planBlocks(...))`；`eventToChunk` 的 message 臂改 `PairingScope.local` + `plansToChunks(plans, event.seq)`；`auxEventData` 的 `default` 改双层收口（`const _exhaustive: never = event` 编译期穷尽 + `JSON.stringify(_exhaustive)` 的 runtime `data-raw` 兜底）
- [x] 修改 `lib/agent-adapter.ts`：删除 `blocksToParts` / `blocksToChunks` / `toolUsePart`（判定与发射均已 IR 化；`collectToolResults` 语义整体迁入 `PairingScope.global`），模块 doc 补三层结构注记（视野层 / 语义层 / 发射层）与「AI SDK data part 与 data chunk 碰巧同形，v8 升级须复核」约定

## 阶段六：transport 双起点与簿记单点

- [x] 修改 `packages/desktop/src/lib/agent-transport.ts`：抽 `openEnvelopeStream(channel: Channel<AgentRunMessage>)` 返回 `{ stream, accept(message), end(), fail(cause) }`——`accept` 内收编现 `streamStart` 的全部逻辑（`event` 臂：`onEvent` 观测 + `eventChunks` 开件簿记；`record` 臂：`onRecord` 观测 + `enqueueRecord` + `closed = true` + `controller.close()`；未知信封忽略不断流）；`end()` 在未收尾时 `close()`（不投伪终态）；`fail()` 注入错误
- [x] 修改 `lib/agent-transport.ts`：`sendMessages` 改为「`openEnvelopeStream` + `invokeStart`」两段（`channel.onmessage = accept`；`invokeStart` 的 reject 走 `fail`；**invoke 提前 resolve 不关流**——发起流由 `Record` 收尾）
- [x] 修改 `lib/agent-transport.ts`：实现 `reconnectToStream(options: { chatId, abortSignal?, body? })`——逐字段读 `body`（`root` / `sessionId` 字符串、`sinceSeq` 数字，非法或缺席 → `null`）→ `commands.sessionDetail(root, sessionId)` 状态门（`detail === null` 或无 `turns.some(t => t.status === 'running')` → `null`）→ `new Channel<AgentRunMessage>()` + `openEnvelopeStream` → `commands.agentSessionWatch(channel, root, sessionId, sinceSeq).then(end).catch(fail)` → 返回 `stream`；`abortSignal` 的 `abort` 监听调 `end()`（doc：订阅流按轮划分、结束即释放；`null` = 无运行轮）
- [x] 修改 `lib/agent-transport.ts`：`invokeStart` 与 `TauriAgentTransport` 类注释随动（两出口同源同信封、共用簿记单点；`reconnectToStream` 不再是「恒 null / 重连由重放承担」）

## 阶段七：会话基建 hook（收编 use-agent-chat）

- [x] 新增 `packages/desktop/src/hooks/use-agent-session.ts`：迁移 `use-agent-chat.ts` 的镜像层（`useSessionMirrors` / `chainWrites` / `withTurns` / `sessionFromTurn` / `buildMessages` / `loadReplay` / `applyReplay`），并增镜像字段 `seqCursor`（ref）——`handleEvent` 与 `applyReplay` 均推高游标（含 delta 的已观测最大 `seq`，doc：订阅 `since_seq` 口径，仅取密封最大 seq 会重投已消费 delta）
- [x] 新增 `hooks/use-agent-session.ts`：参数 `UseAgentSessionParams { root, sessionId, source, refreshKey? }`；效果 A（deps `[root, sessionId]`）= 空参复位（null → 清镜像与消息，零取数零订阅）；效果 B（deps `[root, sessionId, refreshKey, chat.status]`）= 重放 / 收口收敛单点——`root === null` 或 `targetId === null`（`targetId = sessionId ?? currentSessionIdRef.current`）早退；`chat.status` 为 `submitted` / `streaming` 时早退（在挂流不中途重放）；否则 `loadReplay(root, targetId)` → `applyReplay`，失败回落 `buildMessages(mirrors.events, mirrors.turns)`（D16，不抛错不阻塞）
- [x] 新增 `hooks/use-agent-session.ts`：效果 C（订阅调度）——条件 `root !== null && targetId !== null && running（库态 chain 含 running 轮行）&& chat.status ∉ {submitted, streaming} && !loading && !blocked` 时以 `sinceSeq = seqCursor` 调 `chat.resumeStream({ body: { root, sessionId: targetId, sinceSeq } })`；`finally` 内判「无进展」（游标未前进且未收 `Record`）→ 置 `blockedRef = ${targetId}#${refreshKey ?? 0}#${订阅前游标}`（D15 自旋防护），有进展则清 `blockedRef`
- [x] 新增 `hooks/use-agent-session.ts`：动作面 `sendMessage`（body 携 `root` / `prompt` / `permissionMode` / `agent` / `sessionId: currentSessionIdRef ?? sessionId` / `source` / `sourceRef: null`；`chat.status` 非 idle 忽略）、`stop`（`commands.agentStop(root, currentSessionIdRef)`，MUST NOT 调 `chat.stop()` 截断前端流）、`reset`（清镜像 + 消息 + 游标）；返回 `UseAgentSessionState`，其中 `running` = 库态派生（MUST NOT 用 `chat.status`）
- [x] 删除 `packages/desktop/src/hooks/use-agent-chat.ts`（与 阶段九 的调用点切换同批提交；MUST NOT 保留双 hook）

## 阶段八：会话级渲染组件

- [x] 新增 `packages/desktop/src/components/agent/agent-session-view.tsx`：`SessionMeta`（自视图自持元信息，testid 沿用 `session-meta` / `session-meta-id` / `session-meta-status`（运行中 / 已收口）/ `session-meta-turns` / `session-meta-tokens`）；`AgentSessionView(props: AgentSessionViewProps)`——`session` 在场即注入形态，否则内部 `useAgentSession({ root, sessionId, source: 'debug', refreshKey })` 自持形态（注入形态下内部 hook 以 `sessionId: null` 空载：零取数、零订阅）；渲染序：元信息行 → 错误横幅（`agent-session-error`）→ `AgentMessages messages loading running`（空态沿用 `conversation-empty`）→ composer 插槽（`composer?.(actions)`，只读消费点不传即零动作面）；根节点 `data-testid` 缺省 `agent-session-view`
- [x] 新增 `agent-session-view.tsx`：导出 `AgentSessionViewProps` / `AgentSessionActions`（`{ running, loading, send, stop }`）；`sessionId === null` 渲染空态占位（`agent-session-empty`）且零取数零订阅（doc：槽位缺席 ≠ 虚构会话）
- [x] 修改 `packages/desktop/src/components/agent/index.tsx`：增 `export { AgentSessionView } from './agent-session-view';` 与 `export type { AgentSessionActions, AgentSessionViewProps } from './agent-session-view';`

## 阶段九：消费面迁移

- [x] 修改 `packages/desktop/src/views/agent/agent-debug-view.tsx`：`useAgentChat({ source: 'debug', sessionId: null, root })` → `useAgentSession({ source: 'debug', sessionId: null, root })`；时间线位改 `<AgentSessionView session={session} data-testid="agent-timeline" />`；`AgentRawStream events={session.events}`、`StreamToggle running={session.running} onStop={session.stop}`、`AgentRunForm disabled={session.running}` 与 `start`（`session.reset()` + `session.sendMessage(input)`）照旧；`<AgentRunHistory root={root} … />` 补 `root` 透传
- [x] 修改 `packages/desktop/src/views/agent/components/agent-run-history.tsx`：新增 `root: string | null` prop；`ReplayArea` 改为 `<AgentSessionView root={root} sessionId={state.selectedSessionId} data-testid="replay-area-session" />`（删 `eventsToUIMessages` 直调与 `events` prop），抽屉 `onClose` 接 `state.closeSession`
- [x] 修改 `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts`：`useReplay` 整段退役（`events` 状态与 `commands.agentSessionTranscript` 直调删除），`openSession(sessionId)` 只置 `selectedSessionId`，新增 `closeSession()`；`AgentRunHistoryState` 去 `events`、增 `closeSession`，`loading` / `error` 只取清单查询
- [x] 修改 `packages/desktop/src/views/explores/hooks/use-explore-session.ts`：改走 `useAgentSession({ root, sessionId, source: 'explore' })`；返回 `ExploreSessionState { session, sessionId, send, stop }`；`send` 保留「未就绪不发」守卫与首轮 stance 拼接（`session.chain.length === 0` 判据改读 `session.chain`）、`useAutoSend` 闸门改读 `session.loading` / `session.chain`
- [x] 修改 `packages/desktop/src/views/explores/explore-detail-view.tsx`：`ChatPane` 改 `<AgentSessionView session={session.session} composer={({ running }) => <ExploreComposer running={running} onSend={session.send} onStop={session.stop} />} data-testid="explore-chat" />`；`wasRunning` 终态定点重读逻辑改读 `session.session.running`（库态）
- [x] 修改 `packages/desktop/src/views/explores/components/explore-composer.tsx`：props 面不变，doc 随动（`running` 与 `onStop` 由 `AgentSessionView` 插槽注入，`onSend` 为来源侧 stance 包装；文案与 `explore-*` testid 前缀留在本文件）
- [x] 修改 `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`：删 `useSessionTranscript` 调用与本地 `SessionMeta`，改 `<AgentSessionView root={root} sessionId={roleRef.sessionId} refreshKey={refreshKey} data-testid="session-transcript" />`；保留 `session-transcript-panel` 与槽位缺席空态（`drawer-session-empty`：`roleRef === null` →「当前选中无关联会话」、`sessionId === null` →「此步无会话记录」）；`refreshKey` doc 改「会话状态重评估信号」
- [x] 修改 `packages/desktop/src/views/changes/flow/detail-drawer.tsx`：`transcriptRefreshKey` 改名 `sessionRefreshKey`（`DetailDrawerProps` 与 `DrawerContent` 两处形参 + 面板 `refreshKey` 传值），doc 改「会话状态重评估键（组件据其重评估是否挂流）」
- [x] 修改 `packages/desktop/src/views/changes/hooks/use-change-detail.ts`：删 `TRANSCRIPT_REFRESH_DEBOUNCE_MS` 常量与 `useDebouncedTick` / `transcriptTick` / `notifyTranscript`；`ChangeDetailState` 增 `refreshTick: number`（= 统一 tick）替代 `transcriptTick`；模块 doc 随动（`SessionEvent` 与其余通知同走 300ms 尾随去抖）
- [x] 修改 `packages/desktop/src/views/changes/change-detail-view.tsx`：`onNotice` 去 `sessionEvent` 分支（全部 kind → `notifyRefresh`）；`transcriptTick` → `refreshTick`；`transcriptRefreshKey` → `sessionRefreshKey`（`DetailContent` / `DetailLoaded` 形参同步改名）
- [x] 删除 `packages/desktop/src/views/changes/hooks/use-session-transcript.ts`（`liveEvents` / `assembleTranscript` / 直查装配一并退役；MUST NOT 保留第二套转录查询 hook）

## 阶段十：收尾与一致性

- [x] 复核 `components/agent/` 边界：`agent-messages.tsx` / `agent-blocks.tsx` / `tool-cards.tsx` / `agent-input.tsx` 零改动（组件族只新增会话级入口）
- [x] 复核 `do-not-touch` 面零改动：`crates/infra/agent/src/sdk/`、`crates/infra/agent/src/worker.rs`、store schema / `desktop-workspace-store`、`agent_start` / `agent_stop` / 查询类命令参数面、change 域 `RunUpdate` / `RunNotice` 变体集与 `ChangeFlowControl` 广播、`plugins/dev-team`
- [x] 全仓 grep 校验收编完成：零 `useAgentChat` / `useSessionTranscript` / `notifyTranscript` / `transcriptTick` / `eventsToUIMessages` 直调（视图层）残留、零 `@tauri-apps/api/event` 引入
- [x] 全管线对齐（守卫接线）：`check` 走 `bindings:check`（导出 + `git diff --exit-code -- src/types/generated`）、`knip` 零未用导出（IR 三层保持模块私有，禁 test-only export）、`build` 维持 `bindings:export`
