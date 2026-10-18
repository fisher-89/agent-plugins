# 测试设计: desktop-agent-chat-component

> **变更**: desktop-agent-chat-component
> **角色**: test-design-planner（executor）
> **输入**: proposal.md / design.md（本轮为修订稿：补齐 10 处 `#### 待测功能` 条目、为 `components/agent/index.tsx` 与 `explore-composer.tsx` 两节补异常用例）
> **范围**: 仅设计单元测试范围与公共 API 签名，不写测试命令细节；静态脚本（`pnpm -C packages/desktop run check` / `bindings:check` / `knip` / `cargo test --workspace` / `vp test` 汇总执行）由桌面端执行，agent 内不重复。

## 测试框架与测试区域识别

- **后端 `packages/desktop/src-tauri/`**：Rust `cargo test --workspace`；测试文件与源文件同目录、同前缀 + `_test.rs`（经 `#[cfg(test)] mod xxx_test;` 内联挂载，命令目录惯例如 `src/commands/exec/mod.rs` → `src/commands/exec/mod_test.rs`），断言 `assert!` / `assert_eq!` / `matches!`。core 契约层沿既有 `FakeSink` / `FakeRunner` 进程内假件（`Arc<dyn SessionSink>` / `Arc<dyn AgentRunner>` 为被测 API 显式入参）；组合根沿 `DualEnv`（tempfile 数据根 + workspace 根双临时目录 + 真实 `WorkspaceStores`）与 sdk runner 真实装配（端点指向本机 discard 端口 / 非法 base_url，解析组装不触网络）；命令面沿 `tauri::test::mock_app()` + 真实 `WorkspaceStores` + `capturing_channel`（`Channel::new` 回调捕获序列化 JSON 信封）。异步同步原语为真实 `tokio`（`broadcast` / `mpsc`），「未投递 / 仍等待」以 `tokio::task::spawn` + `yield_now` 轮询断言（core/agent 的 tokio dev 特性仅 `rt` / `macros`，不新增 `time`）。
- **前端 `packages/desktop/src/`**：Vitest（`vite-plus/test` 提供 `describe` / `it` / `expect` / `vi`）；hook 用 `renderHook` + `act` / `waitFor`，组件用 `render` / `screen` / `within`（`@testing-library/react`）+ jsdom（`src/test-setup.ts` 已垫 `ResizeObserver` / `getAnimations` / `BASE_UI_ANIMATIONS_DISABLED`）。测试文件与源文件同目录同前缀，扩展名 `.test.ts`（hook / 纯函数）/ `.test.tsx`（组件）；进程边界 IPC 经 `vi.hoisted` + `vi.mock('@tauri-apps/api/core')`（`invokeMock` 按命令名 + 调用序可编程、`ChannelMock` 捕获 `onmessage` 供测试直投信封）；计时面（去抖 / 无进展闸）经 `vi.useFakeTimers` + `advanceTimersByTime`；`@ai-sdk/react` 的 `useChat` 为真实实现（既有对拍先例）。
- **映射约定**：`xxx.rs` → `xxx_test.rs`；`src/commands/<域>/mod.rs` → `src/commands/<域>/mod_test.rs`；`xxx.ts` → `xxx.test.ts`；`xxx.tsx` → `xxx.test.tsx`（与源文件同目录）。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 入参即会话：以 `root` + `sessionId` 挂载 `AgentSessionView` 即完整呈现该会话（对话 + 轮行 + 会话元信息）；四类消费点（debug / explore / change 抽屉 / 历史重放）全部经该组件，视图层零 `eventsToUIMessages` 直调、零自有转录查询 hook | `packages/desktop/src/components/agent/agent-session-view.tsx`、`packages/desktop/src/components/agent/index.tsx`、`packages/desktop/src/hooks/use-agent-session.ts`、`packages/desktop/src/views/agent/agent-debug-view.tsx`、`packages/desktop/src/views/agent/components/agent-run-history.tsx`、`packages/desktop/src/views/agent/hooks/use-agent-run-history.ts`、`packages/desktop/src/views/explores/explore-detail-view.tsx`、`packages/desktop/src/views/explores/hooks/use-explore-session.ts`、`packages/desktop/src/views/explores/components/explore-composer.tsx`、`packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`、`packages/desktop/src/views/changes/flow/detail-drawer.tsx` |
| AC-2 | 按状态挂流：会话有 running 轮行 → 建立恰一条 `agent_session_watch` 订阅并 token 级流式呈现（`messageDelta` 可见）；会话已收口 → 零订阅、零轮询、零定时刷新（invoke 计数可断言） | `packages/desktop/src/hooks/use-agent-session.ts`、`packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/components/agent/agent-session-view.tsx`、`packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src-tauri/crates/core/agent/src/stream.rs` |
| AC-3 | 单流在挂：发送期（发起流在挂）不再建立订阅流，同轮增量只注入一份（无重复文本 / 无重复气泡） | `packages/desktop/src/hooks/use-agent-session.ts`、`packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/views/explores/hooks/use-explore-session.ts` |
| AC-4 | 轮终态收流：订阅流在该轮 `Record` 后关闭；收口后消息状态与直接重开该会话的重放形状一致（含跨消息 tool 配对收敛为单卡，无双卡残留） | `packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/hooks/use-agent-session.ts`、`packages/desktop/src/lib/agent-adapter.ts`、`packages/desktop/src-tauri/crates/core/agent/src/stream.rs`、`packages/desktop/src-tauri/crates/core/agent/src/kernel.rs`、`packages/desktop/src-tauri/crates/core/agent/src/port.rs`、`packages/desktop/src-tauri/src/commands/exec/agent.rs`、`packages/desktop/src/components/agent/agent-session-view.tsx` |
| AC-5 | 双轨同源：change run 的 walker 驱动会话运行中，change 抽屉可旁观 token 级增量（delta 载荷直投），与 exec 轨会话同一订阅命令、同一信封 | `packages/desktop/src-tauri/crates/core/agent/src/stream.rs`、`packages/desktop/src-tauri/crates/core/agent/src/kernel.rs`、`packages/desktop/src-tauri/crates/core/agent/src/lib.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs`、`packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`、`packages/desktop/src/views/changes/flow/detail-drawer.tsx`、`packages/desktop/src/views/changes/hooks/use-change-detail.ts`、`packages/desktop/src/views/changes/change-detail-view.tsx`、`packages/desktop/src/hooks/use-agent-session.ts` |
| AC-6 | 幂等与兜底：对已收口会话 / 未知 id / blank root 调 `agent_session_watch` 立即成功结束（零事件、零副作用）；广播溢出断流后组件重放重订，不出现「丢段仍装作在线」的假态 | `packages/desktop/src-tauri/crates/core/agent/src/stream.rs`、`packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/hooks/use-agent-session.ts` |
| AC-7 | 适配层契约可验：计划层对拍（同一 `BlockPlan[]` 两路同形）通过；视野差异快照锁定「同消息相等 / 跨消息具体差异」；`auxEventData` / `planBlock` 穷尽守卫在位；delta 零落库不变 | `packages/desktop/src/lib/agent-adapter.ts`、`packages/desktop/src-tauri/crates/core/agent/src/kernel.rs`、`packages/desktop/src-tauri/crates/core/agent/src/port.rs` |
| AC-8 | 全管线绿：`pnpm -C packages/desktop run check`（含 `bindings:check` 守卫）与 `run test` 全绿；bindings 已重导入库且守卫通过；desktop 版本 0.5.17 | `packages/desktop/src-tauri/src/commands/mod.rs`、`packages/desktop/src-tauri/src/main.rs`、`packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/package.json`（守卫面见「不可测试项」） |

---

## 单元测试

> 说明：`路径类型` 取值——`正向` / `异常` / `边界` / `组合`（跨模块链路用例，按最上层调用方挂靠到入口模块章节）；`迭代类型` 取值——`新增`（本轮新增用例）、`修改`（既有测试文件随行为变化改写）、`保留`（既有用例不变）。每份 `#### 用例` 表对每个 `#### 待测功能` 条目至少覆盖正向 / 异常 / 边界三类。

### packages/desktop/src-tauri/crates/core/agent/src/stream.rs -> packages/desktop/src-tauri/crates/core/agent/src/stream_test.rs

#### 待测功能

- `SessionStreamRegistry::new(): Self` — 空注册表（`Default` 同语义；`Mutex<HashMap<String, broadcast::Sender<SessionStreamItem>>>`）
- `SessionStreamRegistry::begin(&self, session_id: &str)` — 轮登记：为该会话建容量 `SESSION_STREAM_CAPACITY` 的有界 broadcast；同键重复登记直接覆盖（无重入守卫）
- `SessionStreamRegistry::publish(&self, session_id: &str, event: AgentEvent)` — 事件投递（含 delta）；无条目 / 无订阅者静默丢弃（不产生错误）
- `SessionStreamRegistry::finish(&self, session_id: &str, outcome: TurnOutcome)` — 投终态 `SessionStreamItem::Finished` 后除名（既有订阅者排空后 `Closed`）
- `SessionStreamRegistry::subscribe(&self, session_id: &str, since_seq: u64) -> Option<SessionStreamSubscription>` — 无运行轮（无条目）返回 `None`（命令层「幂等立即结束」判据）
- `SessionStreamSubscription::recv(&mut self) -> Result<SessionStreamItem, SessionStreamRecvError>` — 只返 `seq > since_seq` 的事件（终态恒返）；不合格条目丢弃后继续等待；`Lagged` / `Closed` 显式区分

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `SessionStreamRegistry::new` | 正向 | `new()` 与 `Default::default()` 均为空表：对任意 id（含 `""` / 超长 / emoji）`subscribe` → `None`；`begin` 后方 `Some` | 新增 |
| `SessionStreamRegistry::begin` | 正向 | `begin("ses-1")` → `subscribe("ses-1", 0)` 返 `Some`；随后 `publish` 的事件可被该句柄 `recv`（登记即可订阅） | 新增 |
| `SessionStreamRegistry::begin` | 边界 | 同键重复 `begin` 覆盖：旧句柄的 sender 释放后 `recv` → `Err(Closed)`；新句柄可收覆盖后的新事件（无重入守卫的逐条锁定，proposal 风险 5 留痕） | 新增 |
| `SessionStreamRegistry::begin` | 边界 | 键边界：`""`（空串）、超长（>1000 chars）、特殊字符（`\n` / `\0` / emoji）均正常登记与寻址（HashMap 键逐字保真，不 panic） | 新增 |
| `SessionStreamRegistry::begin` | 边界 | 多会话隔离：`begin(a)` + `begin(b)` 后 `publish(a, …)` → `b` 的句柄零投递（spawn + yield 断言仍等待），`a` 句柄收一条 | 新增 |
| `SessionStreamRegistry::publish` | 正向 | `begin` → `subscribe(0)` → 连续 `publish` seq 1..3（含 `messageDelta`）→ 逐条 `recv` 顺序与载荷逐字相等（seq 严格递增、delta 载荷直投不解释） | 新增 |
| `SessionStreamRegistry::publish` | 异常 | 无条目时 `publish`（未 `begin` / `finish` 除名后）→ 静默 no-op 不 panic；随后 `begin` + `subscribe(0)` 收不到该历史事件（零残留） | 新增 |
| `SessionStreamRegistry::publish` | 异常 | 有条目无订阅者时 `publish` → 静默丢弃不 panic；事后订阅（`since_seq = 0`）只收订阅后 live 事件（broadcast 不回溯历史） | 新增 |
| `SessionStreamRegistry::finish` | 正向 | `begin` → `subscribe(0)` → `publish(e0)` → `finish(outcome)` → `recv` 依序得 `Event(e0)`、`Finished(outcome)`；`outcome` 字段逐字（`status` / `finished_at` / `started_at` / `error`） | 新增 |
| `SessionStreamRegistry::finish` | 正向 | 投终态即除名：`finish` 后 `subscribe(同 id, 0)` → `None`；已投终态的句柄排空缓冲后再 `recv` → `Err(Closed)`（条目移除的自然收尾） | 新增 |
| `SessionStreamRegistry::finish` | 异常 | 无条目 `finish`（未 `begin` / 重复 `finish`）→ 静默 no-op 不 panic；不产生伪终态（再次 `subscribe` 仍 `None`） | 新增 |
| `SessionStreamRegistry::finish` | 边界 | `outcome.started_at` 两极（`0` / `-1` / `i64::MAX`）与 `error` 的 `Some` / `None` 两形态逐字透传（D4 收口形状自足，不做回填） | 新增 |
| `SessionStreamRegistry::subscribe` | 异常 | 未知 id / 已除名 id / blank `""` id → 全部 `None`（幂等判据单点，对应命令「无运行轮立即结束」） | 新增 |
| `SessionStreamRegistry::subscribe` | 边界 | `since_seq` 三值：`0`（全投）/ 已发最大 seq（旧事件全丢不重投、新事件照投）/ `u64::MAX`（事件永不投递，终态仍恒投） | 新增 |
| `SessionStreamRegistry::subscribe` | 边界 | `seq = u64::MAX` 的事件：过滤器比较不溢出，`since_seq = u64::MAX` 时该事件被丢弃、`since_seq = u64::MAX - 1` 时投递（极值边界） | 新增 |
| `SessionStreamSubscription::recv` | 正向 | 过滤后继续等待：`since_seq = 2`，`publish` seq 1、2 → `recv` 仍 pending（spawn + yield 断言）；再 `publish` seq 3 → 返回 `Event(seq 3)`（丢弃不合格条目后继续等待，不 Err） | 新增 |
| `SessionStreamSubscription::recv` | 异常 | lagged：订阅后不消费，`publish` `SESSION_STREAM_CAPACITY + 1` 条 → `recv` → `Err(Lagged(n))` 且 `n >= 1`；断流信号不伪装成终态（不返 `Finished`） | 新增 |
| `SessionStreamSubscription::recv` | 异常 | `Closed` 与 `Lagged` 互不混同：`finish` 排空后 `Err(Closed)`；两变体互不相等、各自可 `PartialEq` 断言 | 新增 |
| `SessionStreamSubscription::recv` | 边界 | 容量边界（D1 = 4096）：恰 4096 条未消费仍可逐条消费（不 lagged）；第 4097 条未消费即 `Lagged`（常量模块私有，经行为断言） | 新增 |
| `SessionStreamRecvError` | 边界 | 变体契约：`Lagged(3) == Lagged(3)`、`Closed == Closed`、`Lagged(_) != Closed`；`Copy` / `Clone` 使丢弃条数可复制进断言（不吞载荷） | 新增 |
| `SessionStreamRegistry` | 正向 | 多订阅者扇出：3 个句柄同 session，`publish` 一条 → 三者各收一份互不吞噬；其中一个 lagged / closed 不影响其余继续收 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 无（新增面全真实） | 不 mock 任何内部模块：`tokio::sync::broadcast` / `Mutex<HashMap>` 为真实实现，注册表与订阅句柄经公共 API 真实组合 | 全部用例 |
| 时间 / 调度（运行环境） | 不引入 `tokio` `time` 特性：「仍等待 / 未投递」以 `tokio::task::spawn` 投递 `recv` future + `yield_now` 轮询 `JoinHandle::is_finished` 断言 | 过滤后继续等待 / 多会话隔离 / 无订阅者用例 |
| 数据载体 | `AgentEvent`（`AgentEvent::stamp` 构造）与 `TurnOutcome` 夹具直接构造，无 fixture 文件 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/agent/src/lib.rs -> packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs

#### 待测功能

- crate 根 re-export（`pub use stream::{SessionStreamItem, SessionStreamRecvError, SessionStreamRegistry, SessionStreamSubscription}`）— 注册表出口（`mod stream;` 模块本身私有，crate 根为唯一可达路径）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| crate 根 re-export | 正向 | 经 `agent::SessionStreamRegistry` 构造 + `begin` / `publish` / `subscribe` / `finish` 全链路可用（唯一出口可用性锚定） | 新增 |
| crate 根 re-export | 正向 | `agent::SessionStreamItem` 两变体经根路径 `match`（`Event` / `Finished`）、`agent::SessionStreamRecvError` 经根路径比较（`Lagged` / `Closed`）——类型出口与行为面同一 | 新增 |
| crate 根 re-export | 边界 | `Default`（根路径 `SessionStreamRegistry::default()`）与 `new()` 等价空表：两实例互不共享条目（一处 `begin` 不使另一处 `subscribe` 命中） | 新增 |
| crate 根 re-export | 异常 | 经根路径对未登记 id `subscribe` → `None`（re-export 不引入行为分支，语义仍由 stream 模块单点决定） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 无 | 不 mock：re-export 面为编译期可达性 + 既有行为，直接经 crate 根路径断言 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/agent/src/port.rs -> packages/desktop/src-tauri/crates/core/agent/src/port_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`TurnOutcome` 为类型定义面变更：增 `pub started_at: i64`，wire camelCase `startedAt`）；以下条目取自「类型定义」表该行与设计 D4 的修改内容，非源文件导出扫描。 -->

- TurnOutcome.started_at(i64): 轮起点串行化字段（wire camelCase `startedAt`），由内核 `converge` 落定；终态轮行装配与订阅面 `Record` 投影共用，store 侧沿用存量行不写该列（D4）
- TurnOutcome（serde 线格式）: 全字段往返保真与 `startedAt` 缺席 / 极值的反序列化契约（类型定义面变更的可验子面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `TurnOutcome` serde 往返 | 正向 | 全字段（含 `started_at: 1_727_000_000_000`）`serde_json` 往返逐字段相等；线格式键为 `startedAt`（与 `finishedAt` / `remoteSessionId` 同 camelCase 口径） | 修改 |
| `TurnOutcome` serde 往返 | 边界 | `started_at` 两极：`0` / `-1` / `i64::MAX` / `i64::MIN` 往返不变（不归一、不回填） | 修改 |
| `TurnOutcome` serde 往返 | 异常 | 线格式缺 `startedAt` → 反序列化 `Err`（MUST NOT 静默补 0——与 `num_turns` / `duration_ms` 等 `Option` 缺席合法区分） | 新增 |
| `TurnOutcome` serde 往返 | 边界 | 既有缺席两极与四 status 变体线格式用例随新字段重跑（夹具补 `startedAt`）；多余未知字段忽略行为回归 | 修改 |
| `TurnOutcome`（跨文件夹具随动） | 边界 | `crates/infra/agent/src/store_port_test.rs` 终态夹具补 `started_at` 后 `finish_turn` 断言仍绿（store 侧沿用存量行，零列写入——D4 明文） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 无 | 不 mock：`TurnOutcome` 为纯 serde 数据载体，直接构造 + `serde_json` 往返断言 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/agent/src/kernel.rs -> packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs

#### 待测功能

- `SessionKernel::new(sink: Arc<dyn SessionSink>, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>) -> Self` — 注入流注册表（第三参）
- `SessionKernel::begin_turn(&self, runner, request) -> Result<RunningTurn, AgentStartError>` — 签名不变，内部增 `streams.begin`（轮登记先于驱动，订阅方可即刻接入）
- `RunningTurn::drive(self, on_output) -> TurnOutcome` — 签名不变，`Observation` 单点发布（delta 与密封同源）；`converge` 投终态并除名

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `SessionKernel::new` | 正向 | 三参构造后 `begin_turn` → 返回 `RunningTurn`；`StopRegistry` 与 `SessionStreamRegistry` 两表互不替代（stop 登记命中为真 + streams 条目订阅为 `Some`） | 修改 |
| `SessionKernel::new` | 异常 | 构造后未 `begin_turn`：对任意 id `subscribe` → `None`（新内核不预置条目） | 新增 |
| `SessionKernel::new` | 边界 | 两个内核共享同一 `Arc<SessionStreamRegistry>`（exec 轨 / change 轨同源注入形态）→ 各自 `begin` 的会话条目并存、互不覆盖（每轮新建内核的 `ComposedTurn` 形态） | 新增 |
| `SessionKernel::begin_turn` | 正向 | `begin_turn` 返回前已 `streams.begin`：返回即 `subscribe(session_id, 0)` → `Some`（订阅方可即刻接入，D2 判据） | 新增 |
| `SessionKernel::begin_turn` | 异常 | open 失败 / 建会话行失败 / 开轮行失败（既有三注入面）→ `Err` 且注册表零条目（`subscribe` 任意 id → `None`，不留半成品） | 新增 |
| `SessionKernel::begin_turn` | 边界 | `SessionRef::New` 与 `SessionRef::Continue` 两形态均登记（Continue 沿用既有会话 id → `subscribe(id)` `Some`）；同会话次轮 `begin` 覆盖旧条目（无重入守卫，proposal 风险 5 留痕） | 新增 |
| `RunningTurn::drive` | 正向 | delta 与密封同源发布：一轮（runStarted + delta×N + message + turnDone）→ 订阅端依序收到与 `on_output` 完全相同的 `Observation` 事件序列（含 delta，载荷逐字、seq 严格递增） | 新增 |
| `RunningTurn::drive` | 正向 | 收口投终态并除名：`drive` 完成 → 订阅端得 `Finished(outcome)`，`outcome.started_at == RunningTurn.started_at`、`status` 与 `on_output` 的 `TurnFinished` 相等；`drive` 后 `subscribe` → `None` | 新增 |
| `RunningTurn::drive` | 异常 | `append_sealed` 注入故障 → 订阅端得 `Finished{status: Failed, error: Some("事件落库失败: …")}`（错误串前缀逐字）；此前 delta 已投递；条目除名 | 新增 |
| `RunningTurn::drive` | 异常 | ask 送达失败（FakeRunner 问题通道关闭）→ 订阅端仅得 `Finished(Failed)`，零伪事件、零伪成功终态 | 新增 |
| `RunningTurn::drive` | 边界 | 空观察流无停止信号 → `Finished(Failed「轮结束但未产出收敛事件」)` 投递后除名（不悬挂） | 新增 |
| `RunningTurn::drive` | 边界 | 无订阅者全程发布 → 静默不 panic；有订阅者但不消费（容量内）→ 驱动不被发布阻塞（同步 `send` 不等待消费端） | 新增 |
| `RunningTurn::drive` | 边界 | 订阅端 `since_seq` = 首轮已投递最大 seq，同会话次轮 `drive` → 只收次轮新增事件（覆盖后 seq 空间续排，不重投首轮） | 新增 |
| `RunningTurn::drive` | 边界 | delta 零落库不变（AC-7）：一轮含 delta + 密封 → `FakeSink.append_sealed` 调用序列只含密封事件；同轮 delta 在订阅端可见（实时面与持久面分流） | 新增 |
| `RunningTurn::drive` | 边界 | 停止置位（`RunHandle::request_stop`）→ stopped 收敛：订阅端 `Finished(Stopped)` 且 `error` 为 `None`（不记因回归） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `SessionSink`（被测 API 显式入参 `Arc<dyn SessionSink>`） | 沿既有 `FakeSink` 进程内假件：调用记录 + `append_failures_from` / `begin_turn_fails` / `create_session_fails` 可编程故障 | 落库故障 / 零落库 / 收口终态用例 |
| `AgentRunner`（被测 API 显式入参 `Arc<dyn AgentRunner>`） | 沿既有 `FakeRunner`：可投喂 `AgentEventKind` 观察流、`close_stream()` 造 EOF、`RunHandle` 置停止位 | 全部 drive 用例 |
| `SessionStreamRegistry` / `StopRegistry` | 不 mock：真实注册表与订阅句柄（新增面经真实组合驱动） | 全部用例 |
| 时间面（运行环境） | 不 mock 时钟、不引入 `tokio` `time` 特性：「仍等待 / 未投递」以 `tokio::task::spawn` + `yield_now` 轮询断言 | 过滤后继续等待 / 无订阅者用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

- `compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, store: Arc<Store>, agent: Option<i64>) -> Result<ComposedTurn, String>` — 注册表注入缝（`ComposedTurn::begin` 签名不变），内核构造处透传

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `compose_turn` | 正向 | 真实 tempdir 双库 + seed provider / instance → `compose_turn(..., streams)` 成功；`ComposedTurn::begin` 得 `RunningTurn` 后该 `session_id` 在传入的 `streams` 中可订阅（`subscribe` → `Some`，注入缝生效） | 修改 |
| `compose_turn` | 正向（组合，链路方向：compose_turn → ComposedTurn::begin → RunningTurn::drive → SessionStreamRegistry） | exec 轨形态真实驱动一轮（拒连端点即时失败收敛）→ 注册表订阅端收到该轮 `Finished`（`status = Failed`、`started_at` 与 running 轮行一致），驱动结束条目除名；与 `agent_start` 的 Channel 终态同源（同一 `compose_turn` 产物） | 新增 |
| `compose_turn` | 正向（组合，链路方向：KernelWorkerPort::run → ComposedTurn::begin → drive → SessionStreamRegistry） | change 轨形态：`KernelWorkerPort::new(composed, sink).run(...)`（`worker.rs` 零改动）驱动一轮失败收敛 → 同一 `streams` 上该会话条目被订阅端观得并收 `Finished`；`RunUpdate::SessionEvent` 出口照常（两出口同源，AC-5 后端半边） | 新增 |
| `compose_turn` | 异常 | 解析失败（无默认 agent / 显式不存在 id / 跨引擎续会话拒收）→ `Err` 文案既有逐字；不构造内核、不产生注册表条目（`subscribe` 该 id → `None`） | 修改 |
| `compose_turn` | 边界 | `StopRegistry` 与 `SessionStreamRegistry` 两表互不串台：stop 命中不影响 streams 条目；`streams.finish` 除名后 `request_stop` 仍 miss 幂等 `false` | 新增 |
| `compose_turn` | 边界 | 两次 `compose_turn` 共享同一 `Arc<SessionStreamRegistry>` → 各自 `begin` 的会话条目并存、互不覆盖（同源注册表多轨共存形态） | 新增 |
| `compose_turn` | 边界 | 既有装配面零回归：快照定型 / explore 注入面 / 续会话治理 / 注入实例消费等既有用例在第三参落位后全部保持 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `WorkspaceStores` / `store::Store`（进程边界 DB） | 不 mock：沿既有 `DualEnv`（tempfile 数据根 + workspace 根，`OnceLock` 缓存单句柄）+ 真实 store 种子 | 全部用例 |
| `AgentRunner`（经 `EngineFacade` 门面真实构造） | 不 mock：sdk 臂真实装配，端点指向本机 discard 端口 / 非法 base_url，解析组装不触网络，轮以 `api_error` 即时收敛 | exec 轨 / change 轨真实驱动用例 |
| `SessionStreamRegistry` / `StopRegistry` | 不 mock：真实实例以 `Arc` 注入（`Arc::new(...)` 每用例独立；共享实例用例显式复用） | 全部用例 |
| 时间面（运行环境） | `#[tokio::test(start_paused = true)]`（既有装置）驱动重试退避快进；不引入真实 sleep | 真实驱动收敛用例 |
| 进程环境（PATH） | 沿既有 commands 级 PATH 隔离锁（引擎不可达收敛窗口），测毕恢复 | 真实驱动收敛用例 |

---

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

- `agent_session_watch(app, on_event: Channel<AgentRunMessage>, root, session_id, since_seq: u64) -> Result<(), String>` — IPC 命令（薄包装）：`is_blank_root` → `Ok(())`；`subscribe` `None` → `Ok(())`；`Some(sub)` → `Event` 前向 / 末拍 `Record`（`outcome_summary` 投影）后 break / `Lagged`、`Closed` 断流 break
- `agent_session_watch_with(...)` — 泛型测试缝（生产经 `agent_session_watch` 注入 Wry 句柄，测试经 `mock_app` 注入 MockRuntime，沿 `agent_start_with` 先例）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `agent_session_watch_with` | 正向 | 预置条目（`begin` + `publish` seq 1..3），`since_seq = 1` invoke → Channel 先收 seq 2、3 的 `{ipc:'event'}`（载荷直投逐字）；再 `publish` seq 4 + `finish(outcome)` → 收 seq 4 与末拍 `{ipc:'record'}`（`TurnSummary` 字段与 `outcome_summary` 投影一致，含 `startedAt`），命令返回 `Ok(())` | 新增 |
| `agent_session_watch_with` | 正向 | 命令结束即订阅释放：`Ok(())` 返回后 Channel 不再有新信封；同 session 再 invoke → 立即 `Ok(())` 零事件（条目已除名，AC-4/AC-6） | 新增 |
| `agent_session_watch_with` | 正向 | `since_seq` 过滤口径：`since_seq = 已发最大 seq` → 零事件、只投末拍 `Record`；`since_seq = 0` → 全量实时事件（不重投语义单点） | 新增 |
| `agent_session_watch_with` | 异常 | blank root（`""` / `"  "` / `"\t"`）→ 立即 `Ok(())`：零事件、零订阅副作用（其后 `publish` 不入 Channel，条目仍在） | 新增 |
| `agent_session_watch_with` | 异常 | 未知 session id → 立即 `Ok(())`（`subscribe` 返 `None`），零事件、不 `Err`（与 `agent_stop` miss 幂等同口径） | 新增 |
| `agent_session_watch_with` | 异常 | 已收口会话（`finish` 除名后）→ 立即 `Ok(())` 零事件零副作用（幂等） | 新增 |
| `agent_session_watch_with` | 异常 | Channel 发送失败（捕获型 Channel 返回 `Err`）→ 前向断流、命令 `Ok` 结束（不 panic、不悬挂） | 新增 |
| `agent_session_watch_with` | 边界 | lagged 断流：订阅后不消费，`publish` > `SESSION_STREAM_CAPACITY` 条 → 命令 `Ok` 结束，Channel 零伪 `record`（不投伪终态，AC-6） | 新增 |
| `agent_session_watch_with` | 边界 | 信封线格式：`{ipc:'event'|'record'}` 判别 + camelCase 字段（`record.startedAt` / `record.finishedAt`）；`sinceSeq` 参数（Rust `u64`）由 `0` / 已观测最大 / `Number.MAX_SAFE_INTEGER` 边界值逐字承接 | 新增 |
| `agent_session_watch_with` | 边界 | delta 零落库（AC-7）：订阅期间收到 `messageDelta`，但 `agent_session_transcript` 查询面不含 delta 行（实时面可见、持久面零新增） | 新增 |
| `agent_session_watch`（生产 Wry 入口） | 异常 | Wry `AppHandle` 进程内不可构造：该入口为 `agent_session_watch_with` 的一行同参数序转发（编译期签名等价 + `all_commands!` 登记承载）；转发语义由 `_with` 缝的全量用例覆盖，本行锚定「生产入口存在且不另写逻辑」 | 新增 |
| `agent_session_watch_with` | 组合（链路方向：`agent_start_with` → compose_turn → drive → SessionStreamRegistry → `agent_session_watch`） | 发起一轮（拒连端点即时收敛）→ 以返回的 `session_id` invoke watch（`since_seq = 0`）→ 两个 Channel 均得末拍 `Record`，`startedAt` / `status` / `turnId` / `finishedAt` 逐字段相等（同源同信封，D4 单一来源；AC-4 / AC-5 命令面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri 托管态 / IPC Channel（进程边界） | 沿既有 `tauri::test::mock_app()` + `app.manage(Arc::new(SessionStreamRegistry::default()))`（与 `StopRegistry` 同托管形态）+ `capturing_channel()` 捕获序列化 JSON 信封（可编程返回 `Err` 造发送失败） | 全部用例 |
| `WorkspaceStores`（进程边界 DB） | 不 mock：既有 `Env` tempdir + 真实 `WorkspaceStores::open` + provider / instance / 会话种子 | 组合用例 / delta 零落库用例 |
| 注册表条目来源 | 命令层用例以注册表公共 API 预置（`begin` / `publish` / `finish`）真实构造「运行中一轮」；不 mock 注册表内部 | 前向 / 过滤 / 终态 / lagged 用例 |
| 内部模块 | 不 mock：`agent::outcome_summary` / `is_blank_root` / 命令薄包装真实组合；引擎走非法 base_url 即时收敛（不落网络） | 全部用例 |
| 时间面（运行环境） | `tokio::task::yield_now` + 既有 `wait_for` 轮询（不新增计时依赖） | 前向收敛等待 |

---

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

#### 待测功能

- `outcome_summary(outcome: &TurnOutcome, session_id: &str) -> TurnSummary` — 签名去 `started_at` 入参（改取 `outcome.started_at`，D4 收口形状自足化）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `outcome_summary` | 正向 | `outcome{started_at: 1_727_000_000_000, finished_at, status: Completed, num_turns/cost_usd/duration_ms}` → `TurnSummary.started_at == outcome.started_at`、`finished_at == Some(outcome.finished_at)`，其余统计字段逐字 | 修改 |
| `outcome_summary` | 正向 | `session_id` 逐字写入（core id `ses-…` 形态不一改）、`turn_id` 承接 | 修改 |
| `outcome_summary` | 异常 | `status: Failed` + `error: Some("事件落库失败: …")` → `error` 原样拷贝（非空、非截断）；`Stopped` 形态 `error: None` 保持 `None` | 修改 |
| `outcome_summary` | 边界 | `started_at` 两极（`0` / `-1` / `i64::MAX`）逐字透传（不因缺省回填 `finished_at` 或当前时刻） | 新增 |
| `outcome_summary` | 边界 | `num_turns` / `cost_usd` / `duration_ms` 全 `None`（缺席合法）→ `TurnSummary` 对应字段 `None`；四 status 变体逐变体透传 | 修改 |
| `running_summary`（既有） | 正向 | 签名未动：`RunningTurn` → running 态轮行（`finished_at: None`、`started_at` 承接）断言不变 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 无 | 不 mock：纯装配函数，`TurnOutcome` / `RunningTurn` 夹具直接构造并逐字段断言 | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`change_flow_start_with` / `change_flow_stop_with` 等签名与参数面零变化；本文件唯一改动为 `compose_turn` 调用点注入第三参 `Arc<SessionStreamRegistry>`）；以下条目取自「修改文件」表该文件行，非源文件导出扫描。 -->

- compose_turn(stores, registry, streams, store, agent)（change 轨调用点）: 注入 `Arc::clone(app.state::<Arc<SessionStreamRegistry>>().inner())`，使 change 轨与 exec 轨同源进入同一注册表
- KernelWorkerPort（change 轨消费方，`worker.rs`）: 零改动即同源流入 `drive` 单点发布面（`RunUpdate::SessionEvent` 出口语义不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| change 轨受理面（注入缝） | 正向 | `change_flow_start_with` 受理一轮（既有 PATH shim / test-only 步形态）→ 返回值、`change_flow_watch` 通知序列与 run 登记均与改前一致（第三参注入不改语义） | 修改 |
| change 轨受理面（注入缝） | 边界 | mock app 托管空表 `SessionStreamRegistry` 后受理路径零 panic/零额外 invoke；未托管形态的取用错误与 `StopRegistry` 同口径（托管契约由 `src/main.rs` 与各命令测试装置共同锚定） | 新增 |
| change 轨控制面 | 异常 | `change_flow_stop_with` / `answer_with` / `confirm_with` 的 blank / 无运行 run / 冲突键既有用例在注入后保持（错误串逐字、零副作用） | 修改 |
| change 轨通知面 | 边界 | `RunUpdate::SessionEvent` 仍记 run 级会话锚并出 kind-only `RunNotice::SessionEvent`（会话状态重评估信号语义不变）；通知载荷零变化 | 修改 |
| change 轨两轨同源 | 组合（链路方向：`change_flow_start` → `compose_turn(streams)` → walker → `KernelWorkerPort` → `drive` → `SessionStreamRegistry`） | 见「不可测试项」：真实 walker 执行 executor 步需真实引擎，进程内不可确定性构造；同源写入点由 `compose.rs` 的 `KernelWorkerPort::run` 真实驱动用例 + `kernel.rs` 的单点发布用例承载，本文件以受理 / 控制 / 通知面零回归断言注入缝不改语义 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri 托管态 | 不 mock：沿既有 `app_with`（`mock_app` + 真实 `WorkspaceStores` + `Arc<StopRegistry>` + `Arc<ChangeFlowControl>`），新增托管 `Arc<SessionStreamRegistry>`（与 `src/main.rs` 同型空表） | 全部用例 |
| 外部进程（static-check / 引擎） | 沿用既有 PATH shim 与 `TEST_PATH_LOCK` 串行化（进程全局变量边界），不 mock `Command` 抽象 | 受理面用例 |
| 磁盘 / DB（进程边界） | 不 mock：既有 `Env` tempdir 双根 + 真实 store 种子（change 建档 / 相位表 / run 登记） | 全部用例 |
| 内部模块 | 不 mock：`compose_turn` / walker / sink 桥接真实组合 | 全部用例 |

---

### packages/desktop/src/lib/agent-adapter.ts -> packages/desktop/src/lib/agent-adapter.test.ts

#### 待测功能

- `eventsToUIMessages(events: AgentEvent[]): AgentUIMessage[]` — 重放 / 收口折叠：`PairingScope.global` 视野 + `plansToParts` 发射（签名不变，实现 IR 化）
- `eventToChunk(event: AgentEvent): AgentUIMessageChunk[]` — 实时增量：`PairingScope.local` 视野 + `plansToChunks` 发射；事件头（`start` / `reset-step`）仍在此（签名不变）
- `runRecordToUIMessage(record: TurnSummary): AgentUIMessage` — 终态轮行 → `data-run-record` 部件消息（不变，回归锁定）
- `provisionalMessageId(parentToolUseId: string | null): string` — provisional 消息键（不变，回归锁定）
- `deltaPartId(key: string, part: 'text' | 'thinking'): string` — delta 累积部件 id（不变，回归锁定）
- IR 三层（`BlockPlan` / `planBlock` / `plansToParts` / `plansToChunks` / `PairingScope`）为模块私有（D11）：计划层对拍经上述两个公共入口达成，`shapeOf` 第三份手写投影退役

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `eventsToUIMessages` | 正向 | 五变体映射回归：`runStarted` / `systemNotice` / `message` / `turnDone` / `raw` 逐事件 → 消息（id / role / metadata.seq / parts）与 IR 化前逐字段相等（既有保真断言面整体重跑） | 修改 |
| `eventsToUIMessages` | 正向 | 计划层对拍（4 变体参数化矩阵：text+thinking / toolUse+同消息 result / 无主 toolResult / aux 事件）：同一 `AgentEvent[]` 经本函数（global + parts）与 `eventToChunk` + `useChat` reducer 折叠（local + chunks）产出，同消息场景两侧 parts 形状相等（`shapeOf` 退役后对拍下沉计划层，AC-7） | 新增 |
| `eventsToUIMessages` | 正向 | 视野快照（同消息相等）：单消息内 toolUse + toolResult → 两路均折叠为单卡 `output-available`（`input` / `output.content` / `isError` 逐字段快照断言） | 新增 |
| `eventsToUIMessages` | 正向 | 视野快照（跨消息差异锁定，MUST NOT 用单一路径 fixture 掩饰）：toolUse 在消息 A、toolResult 在消息 B → global 路收敛为单卡 `output-available`；local 路（`eventToChunk` + reducer）出「孤儿占位卡 + 后续 result 卡」的具体差异快照（能力差异显式锁定，AC-4 / AC-7） | 新增 |
| `eventsToUIMessages` | 正向 | 多 toolUse / toolResult 混合：同 id 重复 result 取首个、toolUse 与 result 在消息内顺序颠倒（result 在前）仍配对收敛单卡 | 修改 |
| `eventsToUIMessages` | 正向 | `delta` 事件（`messageDelta`）被防御跳过：输入含 delta → 输出消息数与纯密封一致（delta 不入重放折叠） | 保留 |
| `eventsToUIMessages` | 异常 | 运行期未知事件（`kind` 伪装成未来变体）：`auxEventData` 的 `const _exhaustive: never` 编译守卫 + `data-raw` 运行期兜底双层 → 输出 `data-raw`（`eventType: 'unknown'`、`rawJson` 为整事件 JSON），不丢失、不抛错 | 修改 |
| `eventsToUIMessages` | 边界 | 空输入 `[]` → `[]`；纯 delta 序列（无密封事件）→ `[]`（sealed-only 视图空态回归） | 保留 |
| `eventsToUIMessages` | 边界 | 无主 `toolResult`（无对应 toolUse）→ 孤儿占位部件（`tool-<ORPHAN_TOOL_NAME>` / `state: 'output-available'` / `input: null`），global 与 local 两路同形 | 修改 |
| `eventsToUIMessages` | 边界 | 超大输入（1000 条密封事件）不抛错、输出保序且 `metadata.seq` 单调；超长文本（>1000 chars）与特殊字符（`\n` / emoji）逐字保留 | 新增 |
| `eventToChunk` | 正向 | delta 路：text / thinking 两类 delta → 无事件头的累积 chunk（part id 由 `deltaPartId` 决定），同 id 多 delta 可累加 | 保留 |
| `eventToChunk` | 正向 | 密封路（IR 化后）：`message` 事件 → `start` + `reset-step` + 计划发射 chunk 组；部件 id 为计划下标形（`t-${seq}-${i}` / `r-${seq}-${i}`），tool 部件走 `tool-input-available` / `tool-output-available` | 修改 |
| `eventToChunk` | 正向 | aux 路：非 message 密封事件 → `start` + `reset-step` + 单 data 部件（载荷与 `eventsToUIMessages` 同事件的数据部件逐字段相等） | 保留 |
| `eventToChunk` | 异常 | 未知 aux 事件 → `start` + `reset-step` + `data-raw`（`eventType: 'unknown'`），chunk 流不中断 | 修改 |
| `eventToChunk` | 边界 | 空 `blocks` 的 message 事件 → 只有事件头两 chunk（零计划 chunk，不产生空部件） | 修改 |
| `runRecordToUIMessage` | 正向 | `TurnSummary` → `turn-<turnId>` 消息 + `data-run-record` 载荷原样（含 `startedAt`）、`metadata.seq === null`；running 形态轮行同样可折叠 | 保留 |
| `runRecordToUIMessage` | 异常 | 未来 / 非法 `status` 值与未知多余字段原样透传（不解释、不丢字段）；`error` 串含换行 / emoji 逐字保留 | 新增 |
| `runRecordToUIMessage` | 边界 | `turnId` 两极（`0` / `-1` / 超大整数）→ 消息 id `turn-<id>` 稳定且互异；统计字段全 `null` 形态可折叠 | 新增 |
| `provisionalMessageId` / `deltaPartId` | 正向 | 线格式逐字契约：`provisionalMessageId(null) === 'delta-none'`、`provisionalMessageId('tu_1') === 'delta-tu_1'`、`deltaPartId('tu_1','text') === 't-delta-tu_1'`、`deltaPartId('tu_1','thinking') === 'r-delta-tu_1'`（与 `eventToChunk` 产出的 part id 同源） | 保留 |
| `provisionalMessageId` / `deltaPartId` | 异常 | 空串键（`''`）与 `null` 键可区分（`delta-` ≠ `delta-none`）；非字符串 / 控制字符注入不抛错（键仍为稳定串拼接） | 保留 |
| IR 三层私有性（D11） | 边界 | 计划层（`BlockPlan` / `planBlock` / `plansToParts` / `plansToChunks` / `PairingScope`）不经模块出口暴露（模块导出面仍为 `eventsToUIMessages` / `eventToChunk` 两入口；`knip` 无新增未使用导出），对拍与视野快照经两入口达成 | 新增 |
| `provisionalMessageId` / `deltaPartId` | 边界 | 确定性 + 互异：同输入多次调用恒等；`parentToolUseId` 为 `null` / `""` / 特殊字符（emoji / 换行）/ 超长时两函数输出仍互不碰撞（配对键稳定性） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 无（纯函数 + 真实 reducer） | 不 mock：适配层为纯函数；对拍用例以真实 `@ai-sdk/react` `useChat`（`renderHook` + `act` 投喂 chunk，既有先例）做 local 路折叠，不 mock ai-sdk、不 mock reducer | 计划层对拍 / 视野快照用例 |
| 数据载体 | `AgentEvent` / `AgentBlock` / `TurnSummary` 夹具直接构造（沿用既有 fixture 风格），无外部 JSON 语料文件（真实 JSONL fixture 属显式出范围项） | 全部用例 |

---

### packages/desktop/src/lib/agent-transport.ts -> packages/desktop/src/lib/agent-transport.test.ts

#### 待测功能

- `sendMessages(options): Promise<ReadableStream<AgentUIMessageChunk>>` — 发起流（`agent_start`，`Channel` 先于 invoke 建立 → 发送方零空洞）；簿记 + `Record` 收口 + 关流走 `openEnvelopeStream` 单点
- `reconnectToStream(options): Promise<ReadableStream<AgentUIMessageChunk> | null>` — 订阅流：解析 `body`（`root` / `sessionId` / `sinceSeq`，非法或缺席 → `null`）→ `session_detail` 状态门（无 running 轮行 → `null`）→ `Channel` + 簿记流 → `agent_session_watch(...).then(end).catch(fail)`；`abortSignal` abort → `end()`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `reconnectToStream` | 正向 | body 透传：`{root, sessionId, sinceSeq}` → 先 `session_detail` 状态门（running 轮行在场）、再 `agent_session_watch(channel, root, sessionId, sinceSeq)` 参数逐字（生成绑定调用面，`sinceSeq` 为 number）；返回可读流非 `null` | 新增 |
| `reconnectToStream` | 正向 | 开件簿记（约束③ / V6）：`Channel` 直投 `messageDelta`（无 provisional `start`）→ 流先补 `start` + `reset-step` + `part-start`，后续同 kind delta 直发累积 chunk；与 `sendMessages` 的同类事件 chunk 序列逐条相等（两出口共用簿记单点） | 新增 |
| `reconnectToStream` | 正向 | 轮终态收流（AC-4）：`Channel` 投 `Record` → `data-run-record` 部件 + `finish` + 关流；命令 resolve 后流已关闭、后续信封零 chunk；流关闭使 `status` 复位并触发 `onFinish`（`resumeStream` promise 收尾） | 新增 |
| `reconnectToStream` | 正向 | 完成态零订阅（AC-2）：`session_detail` 返回无 running 轮行的会话 → 返回 `null`，且 `agent_session_watch` 零调用（invoke 计数：仅 `session_detail` 一次） | 新增 |
| `reconnectToStream` | 异常 | body 非法（缺 `root` / 缺 `sessionId` / `sinceSeq` 非 number / `body` 为 `undefined` 或缺席）→ 返回 `null`，零 `session_detail`、零 `agent_session_watch`，不抛错 | 新增 |
| `reconnectToStream` | 异常 | `session_detail` reject → 返回 `null`（不订阅、不抛，交由 hook 错误面呈现） | 新增 |
| `reconnectToStream` | 异常 | `agent_session_watch` reject → 流不悬挂：以 `fail(cause)` 注错或 `end()` 关流（消费端可判定结束，`status` 复位），文本不出现伪终态 `data-run-record` | 新增 |
| `reconnectToStream` | 边界 | `abortSignal` 已 abort（调用前）/ 订阅中被 abort → 流 `end()` 关闭、后续信封零 chunk、`agent_session_watch` resolve 迟到不重复关闭（幂等） | 新增 |
| `reconnectToStream` | 边界 | `sinceSeq` 边界：`0` / 已观测最大 seq / `Number.MAX_SAFE_INTEGER` 原样透传（不做本地裁剪、不重排），过滤语义留后端 | 新增 |
| `reconnectToStream` | 边界 | 空轮 / 零事件：`Channel` 无任何信封而命令 resolve → 流关闭且零 chunk（零伪终态，不投 `data-run-record`） | 新增 |
| `reconnectToStream` | 边界 | 重复 / 迟到信封：`Record` 后 `closed` 守卫使后续 `event` / 第二个 `Record` 零 chunk；未知 `ipc` 判别值忽略单条消息、流不断开 | 新增 |
| `sendMessages` | 正向 | 回归（簿记单点化后零行为变化）：body 会话参数逐字进入 `agent_start`、事件逐条 `eventToChunk` 保序、首 delta 补 provisional 开件组、`Record` 收口 + 关流 | 保留 |
| `sendMessages` | 异常 | 回归：`agent_start` reject / 非 `submit-message` trigger 的既有错误面保持（启动失败把错误灌入流、trigger 拒收） | 保留 |
| `sendMessages` | 边界 | 两出口共用收口单点（AC-3 / AC-4）：同一簿记实现下，发起流与订阅流的 `Record` 收口 chunk 组逐条相等；发起流在挂期间不额外建立订阅流（`agent_session_watch` 零调用——本文件以「两出口互不触发」断言，hook 层以 invoke 计数断言） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：`invokeMock` 按命令名分发（`session_detail` 可编程 running / completed / `null` / reject，`agent_session_watch` 可编程 resolve / reject 并记录参数），`ChannelMock` 捕获 `onmessage` 供测试直投 `AgentRunMessage` 信封；`beforeEach` 复位计数与实例表 | 全部用例 |
| `ReadableStream` / `AbortController` | 不 mock：真实 `ReadableStream` 与 `drain()` 消费（既有先例）、真实 `AbortController` 触发 `abortSignal` | 全部用例 |
| 内部模块 | 不 mock：`openEnvelopeStream` / `eventToChunk` / `readSessionParams` 真实组合（簿记单点与适配层同源经链路断言） | 全部用例 |

---

### packages/desktop/src/hooks/use-agent-session.ts -> packages/desktop/src/hooks/use-agent-session.test.ts

#### 待测功能

- `useAgentSession(params: UseAgentSessionParams): UseAgentSessionState` — 重放装载（`session_detail` + `agent_session_transcript` → 同一折叠入口）+ 状态驱动挂流（库态 running 且无在挂流才 `resumeStream`）+ 单流在挂 + 收口以重放收敛（失败回落 `buildMessages` 镜像折叠）+ 断流重订（游标 `seqCursor` / 无进展闸）+ 发送 / 停止 / 重置；`running` = `chain.some(t => t.status === 'running')`（MUST NOT 用 `chat.status` 替代）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `useAgentSession` | 正向 | 重放装载：`session_detail` + `agent_session_transcript` resolve → `session` / `chain` / `events` 落位、`messages` 为密封折叠 + 轮行交错（同一折叠入口）、`loading` 由 true 回 false；`running` 由轮行 `status` 派生 | 修改 |
| `useAgentSession` | 正向 | 状态驱动挂流（AC-2）：重放装载完成且存在 running 轮行 → 恰一次订阅（`session_detail` → `agent_session_watch` 各一次）；`Channel` 直投 `messageDelta` → `messages` 文本累积可见（token 级流式） | 新增 |
| `useAgentSession` | 正向 | 完成态零订阅零轮询（AC-2）：轮行全 `completed` → `agent_session_watch` 零调用；推进假计时 5min 后 invoke 计数不增（零定时刷新、零事件订阅） | 新增 |
| `useAgentSession` | 正向 | 单流在挂（AC-3）：`sendMessage` 发起（发起流在挂，`status = streaming`）→ 期间 `agent_session_watch` 零调用；同轮增量只注入一份（文本无重复、气泡数不翻倍）；轮 `Record` 收口后 `status` 复位 | 新增 |
| `useAgentSession` | 正向 | 收口以重放收敛（AC-4）：订阅流收 `Record` 关流 → 以重放（`session_detail` + `transcript`）重建 `messages`；重建结果与「直接挂载同一已收口会话」的重放形状相等（含跨消息 tool 配对收敛为单卡，无双卡残留） | 新增 |
| `useAgentSession` | 正向 | 断流重订（AC-6）：订阅流关闭（lagged / 空轮）而库态仍 running → 以推进后的 `sinceSeq` 重订恰一次（`agent_session_watch` 计数 +1，参数 `sinceSeq` = 已观测最大 seq），重订后文本不重投（同一段 delta 只出现一次） | 新增 |
| `useAgentSession` | 正向 | 发送守卫（proposal 附带效应）：库态 running 时 `sendMessage` → 零 `agent_start` invoke（旁观误发被拦，`status = streaming` 为在挂流判据的副产物） | 新增 |
| `useAgentSession` | 异常 | 收口重放失败回落（D16）：`Record` 后重放查询 reject → 回落 `buildMessages(eventsRef, turnsRef)` 镜像折叠；`messages` 与镜像一致、`error` 不被置位（不抛错、不阻塞） | 新增 |
| `useAgentSession` | 异常 | 装载失败：`session_detail` reject → `error` 置位、`loading` 复位 false、零订阅（不因错误态误挂流）；`agent_session_transcript` reject 同式 | 修改 |
| `useAgentSession` | 异常 | 无进展闸（D15）：订阅一次结束而游标未前进且未收 `Record` → 阻断重订（推进时间后 `agent_session_watch` 计数不增）；`refreshKey` / `sessionId` 变更后解阻并重试一次 | 新增 |
| `useAgentSession` | 异常 | `agent_session_watch` reject（网络 / 命令异常）→ 状态复位（不永处 `streaming`）、`error` 呈现、不形成自旋（计数有界） | 新增 |
| `useAgentSession` | 边界 | 参数边界：`root = null` 或 `sessionId = null` → 零取数、零订阅、`messages` 空、`loading` false（注入形态空载，D9）；`refreshKey` 缺省 undefined 与显式数字同语义 | 修改 |
| `useAgentSession` | 边界 | `sinceSeq` 游标口径（D8）：多 delta + 密封事件后断流重订 → 订阅参数 `sinceSeq` 取「已观测事件最大 seq（含 delta）」；重放装载同样推高游标（首次订阅 `sinceSeq` = 重放最大 seq，零事件则为 0） | 新增 |
| `useAgentSession` | 边界 | 停止动作：`stop()` → `agent_stop(root, sessionId)` 参数逐字；会话 id 未确立时 miss 不发（零 invoke） | 修改 |
| `useAgentSession` | 边界 | 重置 `reset()`：清 `session` / `chain` / `events` / `messages` / 游标；重置后切换 `sessionId` 可重新装载（新会话形状，旧镜像零残留） | 修改 |
| `useAgentSession` | 边界 | `refreshKey` 变更 = 状态重评估：重放重取一次；库态转 `running=false` 时拆订阅（不重订）；转 `true` 且无在挂流时挂一次 | 新增 |
| `useAgentSession` | 边界 | 卸载 / 切会话：unmount → 流 abort、零遗留计时器、零后续 invoke；`sessionId` 由 A 切 B 后旧会话迟到的 `Record` 不污染 B 的 `messages` / `chain` | 新增 |
| `useAgentSession` | 边界 | 发送组装：`sendMessage({prompt, permissionMode, agent?})` → `agent_start` body 逐字（`root` / `prompt` / `permissionMode` / `agent`（缺省 `null` 恒在）/ `source` / `sourceRef: null`）；会话 id 取当前镜像，镜像未确立时回落 `params.sessionId`（Continue 语义不新铸） | 修改 |
| `useAgentSession` | 边界 | 空轮 / 断流无 `Record` → 流关闭后 `status` 复位（不永处 `streaming`）、运行态回落库态；随后按库态决定是否重订（幂等不重复建流） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：`invokeMock` 按命令名 + 调用序可编程（`session_detail` 可返 running / completed / `null` / reject / 挂起门闩；`agent_session_transcript` 可编程；`agent_session_watch` resolve / reject；`agent_stop` / `agent_start` 记录参数与计数），`ChannelMock` 捕获 `onmessage` 供测试直投事件、`Record` 与断流 | 全部 describe |
| 计时面（运行环境） | `vi.useFakeTimers` + `advanceTimersByTime`（零轮询 / 无进展闸 / 去抖节奏）；`afterEach` 复位真计时器 | 零轮询 / 无进展闸用例 |
| 内部模块 | 不 mock：`TauriAgentTransport`（含 `openEnvelopeStream` 簿记）、`agent-adapter` 折叠、`@ai-sdk/react` `useChat` 全部真实组合（hook 即链路入口，端到端在进程内闭环） | 全部用例 |

---

### packages/desktop/src/components/agent/agent-session-view.tsx -> packages/desktop/src/components/agent/agent-session-view.test.tsx

#### 待测功能

- `AgentSessionView(props: AgentSessionViewProps): React.JSX.Element` — 会话级唯一渲染入口（双形态）：自持形态（`root` + `sessionId`，内部建 hook 与流）与注入形态（`session` = `useAgentSession` 返回值，内部 hook 以 `sessionId = null` 空载）；元信息行（`SessionMeta`）+ 错误 / 加载 / 空态 + `AgentMessages` 对话透镜 + composer 插槽（render prop `(actions) => ReactNode`）；`data-testid` 透传

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `AgentSessionView`（自持形态） | 正向 | `root` + `sessionId` 挂载 → 经 `session_detail` + `agent_session_transcript` 装配（各一次），呈现元信息 `session-meta` / `session-meta-id` / `session-meta-status`（「运行中」或「已收口」）/ `session-meta-turns` / `session-meta-tokens`、对话气泡与 `data-run-record` 轮行；`data-testid` 落在组件根 | 新增 |
| `AgentSessionView`（自持形态） | 正向 | 运行中会话挂载（AC-2）：轮行含 running → 建立恰一条 `agent_session_watch`；直投 `messageDelta` → 对话呈现累积文本（token 级流式） | 新增 |
| `AgentSessionView`（自持形态） | 正向 | 完成态零订阅（AC-2）：轮行全收口 → `agent_session_watch` 零调用、零轮询（推进时间 invoke 计数不增） | 新增 |
| `AgentSessionView`（注入形态） | 正向 | 传 `session`（真实 `useAgentSession` 返回值）→ 零额外会话查询、零第二条流（`session_detail` / `agent_session_watch` 计数 == 来源侧一次）；渲染体与自持形态一致（同一 `AgentMessages` + 元信息 + 轮行） | 新增 |
| `AgentSessionView`（composer 插槽） | 正向 | `composer={({running, loading, send, stop}) => …}` → 内容渲染在消息区下方；`running` / `loading` 取自会话（库态）；插槽内触发 `send` → `agent_start` 按会话 body 发出、触发 `stop` → `agent_stop` | 新增 |
| `AgentSessionView`（只读形态） | 正向 | 不传 `composer`（change 抽屉 / 历史重放形态）→ 无发送输入、无停止按钮、零动作面接线（AC-1：只读消费点不被强制接线） | 新增 |
| `AgentSessionView`（空态） | 边界 | `sessionId = null`（自持）或注入空 `messages` → 空态提示呈现、零取数零订阅；`loading` 在途呈现加载占位（不呈现伪空态） | 新增 |
| `AgentSessionView`（错误面） | 异常 | 会话查询 reject / `session.error` 非 null → 错误提示呈现（不抛错、不渲染伪气泡）；错误恢复后提示消失 | 新增 |
| `AgentSessionView`（会话不存在） | 异常 | `session_detail` 返回 `null` → 空态提示（不虚构会话元信息、零订阅） | 新增 |
| `AgentSessionView`（状态重评估） | 边界 | `refreshKey` 变更 → 重放重取一次；库态由 running 转收口时订阅不被重建（零重订），由收口转 running 时重建一次；全程并行流 ≤ 1 条 | 新增 |
| `AgentSessionView`（形态切换） | 边界 | props 由自持切注入（或注入 `session` 置空）→ 不产生并行第二条流（`agent_session_watch` 计数恒 ≤ 1）；卸载后流拆除、零后续 invoke（AC-3 组件面） | 新增 |
| `AgentSessionView`（组合，链路方向：AgentSessionView → useAgentSession → TauriAgentTransport.reconnectToStream → `session_detail` / `agent_session_watch` → Channel） | 组合 | 以 `root` + 运行中 `sessionId` 挂载 → 取数 → 状态门 → 订阅 → Channel 直投 delta / 密封 / `Record` → 收口重放 → 呈现与「重开同一会话」一致的形状（含跨消息 tool 单卡）；收口后订阅关闭且不再重订（AC-1 / AC-2 / AC-4 链路单点承载） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：`invokeMock` 按命令名分发（`session_detail` / `agent_session_transcript` / `agent_session_watch` / `agent_start` / `agent_stop` 可编程并计数），`ChannelMock` 捕获 `onmessage` 供直投信封 | 全部用例 |
| jsdom 环境垫片（运行环境） | 既有 `ResizeObserver` / `getAnimations` / `BASE_UI_ANIMATIONS_DISABLED` 垫片（环境垫片非业务 mock） | 全部渲染用例 |
| 计时面（运行环境） | `vi.useFakeTimers` + `advanceTimersByTime`（零轮询 / refreshKey 重评估节奏） | 完成态零订阅 / 状态重评估用例 |
| 内部模块 | 不 mock：`useAgentSession`（自持形态经真实 hook）、`AgentMessages` / `agent-blocks` / `tool-cards` 真实组合；注入形态的 `session` 为显式 props（优先用真实 hook 产出，必要时按 `UseAgentSessionState` 形状构造状态对象） | 全部用例 |

---

### packages/desktop/src/components/agent/index.tsx -> packages/desktop/src/components/agent/index.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（本文件仅增 `export { AgentSessionView } from './agent-session-view';` 与 props / actions 类型再导出，无函数体）；以下条目取自「修改文件」表该文件行，非源文件导出扫描。 -->

- AgentSessionView（组件族 barrel 再导出）: `export { AgentSessionView } from './agent-session-view'`，唯一渲染入口的单点出口
- AgentSessionViewProps / AgentSessionActions（类型再导出）: props 与 composer 插槽 actions 类型随组件同出口暴露
- AgentMessages / AgentInput（既有出口）: 消费者 import 路径零改动（barrel 出口集合不变式）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 组件族 barrel | 正向 | `import { AgentSessionView } from '@/components/agent'` 与直接 import 模块为同一引用（单一出口），经 barrel 渲染的最小 props 形态可挂载（冒烟） | 新增 |
| 组件族 barrel | 边界 | `AgentSessionViewProps` / `AgentSessionActions` 类型可自 barrel 导入并被测试文件引用（类型面出口编译期锚定） | 新增 |
| 组件族 barrel | 正向 | 既有 `AgentMessages` / `AgentInput` 出口不变（消费者 import 路径零改动） | 保留 |
| 组件族 barrel | 异常 | 命名出口缺失 / 名字漂移：barrel 运行时导出键集合逐字断言为 `AgentMessages` / `AgentInput` / `AgentSessionView`；若改名或漏导出则引用侧编译失败——本断言同时锁定「不存在同一组件的第二个导出名」（D9「两个导出组件」方案被排除） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core`（进程边界 IPC） | 与 agent-session-view.test 同型的最小 invoke / Channel mock（渲染冒烟所需） | barrel 渲染用例 |
| 内部模块 | 不 mock：barrel 即 re-export，直接经真实模块组合 | 全部用例 |

---

### packages/desktop/src/views/agent/agent-debug-view.tsx -> packages/desktop/src/views/agent/agent-debug-view.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`AgentDebugView` 签名不变；改动为 `useAgentChat` → `useAgentSession({ root, sessionId: null, source: 'debug' })` 与时间线位改注入形态 `AgentSessionView`）；以下条目取自「修改文件」表该文件行，非源文件导出扫描。 -->

- AgentDebugView({ root })（页面签名不变）: 会话来源由 `useAgentChat` 改 `useAgentSession({ root, sessionId: null, source: 'debug' })`（注入形态的来源侧 hook）
- 时间线位（`data-testid="agent-timeline"`）: `<AgentSessionView session={session} />` 注入形态承载对话 + 轮行 + 元信息
- AgentRawStream(events = session.events) / StreamToggle(running = session.running) / 发送表单（`start` = `reset()` + `sendMessage`，`disabled={session.running}`）: 动作面与原始转录面仍取 hook 数据

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 时间线接线 | 正向 | 渲染 → 时间线位为注入形态 `AgentSessionView`（`data-testid="agent-timeline"`），对话气泡 / 轮行由组件承载；`AgentRawStream events={session.events}` 仍是密封事件镜像（delta 不入镜像） | 修改 |
| 时间线接线 | 正向 | 发送表单提交 → `agent_start` body 逐字（`source='debug'`、`sessionId` 取当前会话）；`start` = `reset()` + `sendMessage` 语义保持 | 修改 |
| 流式呈现（AC-2） | 正向 | 发起一轮运行中 → 时间线呈现流式文本（发起流在挂，零订阅）；`StreamToggle` 的 `running` 取库态；停止入口 → `agent_stop` | 修改 |
| 运行中禁发 | 边界 | 库态 running → 表单 `disabled`、再次提交零 `agent_start` invoke；收口后恢复可发 | 修改 |
| 每跑重置 | 边界 | `reset` 后时间线清空、`events` 空、`session` null；重置后可再发一轮（不残留上一轮气泡） | 修改 |
| 历史区透传 root | 边界 | `<AgentRunHistory root={root} …>` 透传当前 root（自持形态可寻址）；root 为 null 时历史区空态、重放区不渲染 | 新增 |
| 启动失败 | 异常 | `agent_start` reject → 错误提示呈现（既有错误面），时间线不出现伪气泡、表单恢复可用 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`（`agent_start` / `agent_stop` / `agent_sessions` / `session_detail` / `agent_session_transcript` 可编程并计数；`ChannelMock` 直投事件与 `Record`） | 全部用例 |
| jsdom 环境垫片（运行环境） | 既有环境垫片（滚动 / 尺寸观测） | 全部渲染用例 |
| 内部模块 | 不 mock：`useAgentSession` / `AgentSessionView` / `AgentRawStream` / `AgentRunHistory` 真实组合（页面即链路入口） | 全部用例 |

---

### packages/desktop/src/views/agent/components/agent-run-history.tsx -> packages/desktop/src/views/agent/components/agent-run-history.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`ReplayArea` 为私有组件；改动为新增 `root` prop、重放区改自持形态 `AgentSessionView`、删除 `events` prop 与 `eventsToUIMessages` 直调）；以下条目取自「修改文件」表该文件行与 D14，非源文件导出扫描。 -->

- AgentRunHistory({ root, source })（新增 `root: string | null` prop）: root 自页面透传，供应自持形态寻址
- ReplayArea（私有）: 改 `<AgentSessionView root={root} sessionId={state.selectedSessionId} data-testid="replay-area-session" />`；删除 `events` prop 与 `eventsToUIMessages` 直调（自有转录查询退役）
- onClose（接线）: 接 `state.closeSession`（只置 `selectedSessionId = null`，不再清自持 events）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 只读重放（自持形态） | 正向 | 点开会话 → `replay-area-session` testid 的 `AgentSessionView` 以 `root` + `selectedSessionId` 挂载 → 经 `session_detail` + `agent_session_transcript` 呈现对话 + 轮行 + 元信息（AC-1）；本组件零 `eventsToUIMessages` 直调（`events` prop 已删） | 修改 |
| 只读重放（自持形态） | 正向 | 运行中会话重放 → 订阅流可见 token 级增量（只读旁观，无动作面）；收口后零重订 | 新增 |
| 关闭重放 | 正向 | `onClose` → `state.closeSession` → 重放区消失、`selectedSessionId` 清空，会话清单仍在（零额外清单查询） | 修改 |
| 队列 / 分页回归 | 正向 | 来源筛选、分页、刷新、清单空态 / 错误态既有断言不变 | 保留 |
| 重放区查询失败 | 异常 | 选中会话的查询 reject → 组件内错误提示呈现，不塌清单、不抛错 | 新增 |
| 无选中 / 无 root | 边界 | `selectedSessionId === null` → 不渲染重放区（零查询）；`root === null` → 清单空态 + 重放区缺席 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 mock：`agent_sessions` 分页应答、`session_detail` / `agent_session_transcript` 可编程（含 reject）并计数 | 全部用例 |
| 内部模块 | 不 mock：`useAgentRunHistory` / `AgentSessionView` / `AgentMessages` 真实组合 | 全部用例 |

---

### packages/desktop/src/views/agent/hooks/use-agent-run-history.ts -> packages/desktop/src/views/agent/hooks/use-agent-run-history.test.ts

#### 待测功能

- `useAgentRunHistory(root: string | null, source: AgentHistorySource): AgentRunHistoryState` — `events` 与 `agentSessionTranscript` 直调退役；`openSession` 只置选中 id、新增 `closeSession()`；`loading` / `error` 只取清单查询

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `useAgentRunHistory` | 正向 | 挂载 → 清单查询一次 → `sessions` / `total` / `totalPages` / `page` 落位，`loading` 起落 | 修改 |
| `useAgentRunHistory` | 正向 | `openSession(id)` → `selectedSessionId` 置位且零 `agent_session_transcript` invoke（自有查询退役，D14）；`closeSession()` → 置回 `null` | 修改 |
| `useAgentRunHistory` | 异常 | 清单查询 reject → `error` 置位、`loading` 复位；`refresh()` 成功后 `error` 清空 | 修改 |
| `useAgentRunHistory` | 边界 | 分页：跳页夹紧 `[1, totalPages]`、`nextPage` 末页不动、`prevPage` 首页不动；`root` / `source` 变更回第 1 页并重查 | 保留 |
| `useAgentRunHistory` | 边界 | 空闲零轮询：推进假计时 5min → 清单 invoke 计数不增；`closeSession()` 在未开选中时幂等（零副作用、零 invoke） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke（进程边界 IPC） | 既有 mock（`agent_sessions` 按参数分发应答 / reject 并计数） | 全部用例 |
| 计时面（运行环境） | `vi.useFakeTimers` + `advanceTimersByTime`（零轮询断言） | 空闲零轮询用例 |
| 内部模块 | 不 mock：hook 内分页 / 夹紧纯函数真实组合 | 全部用例 |

---

### packages/desktop/src/views/explores/hooks/use-explore-session.ts -> packages/desktop/src/views/explores/hooks/use-explore-session.test.ts

#### 待测功能

- `useExploreSession(root: string | null, record: ExploreRecord | null, autoPrompt?: string | null): ExploreSessionState` — 改走 `useAgentSession`（`source: 'explore'`）；返回值收敛为 `{ session, sessionId, send, stop }`；stance 拼接与 auto-send 门留来源侧（`send` 保留未就绪 / loading 守卫）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `useExploreSession` | 正向 | 记录携 `sessionId` → `session` 以 `source='explore'` 装载（`session_detail` / `agent_session_transcript` 带 explore 来源）、`sessionId` 承接记录值 | 修改 |
| `useExploreSession` | 正向 | `send(input)` → stance 前导 + prompt 拼接后经 `session.sendMessage` → `agent_start` body 中 `prompt` 逐字含 stance、`source='explore'`；`running` 取库态 | 修改 |
| `useExploreSession` | 正向 | auto-send 闸门：`autoPrompt` + 装载完成（已观测 loading 真值）+ `chain` 空 + id 就绪 → 自动发出一轮（恰一次，prompt 含 stance）；发出后 `chain > 0` 不再复发 | 修改 |
| `useExploreSession` | 异常 | 存量记录 `session_id` 空缺 → `ensure_explore_session` 补齐成功后承接其 id；补齐失败静默（零发送、下次挂载重试） | 修改 |
| `useExploreSession` | 边界 | 未就绪（`loading` / `sessionId` null / `root` null）时 `send` → 零 `agent_start` invoke（守卫在来源侧） | 修改 |
| `useExploreSession` | 边界 | `record = null` → 空载（`session` 空、零取数、零订阅） | 修改 |
| `useExploreSession` | 边界 | `autoPrompt` per-mount 一次性消费：back/forward 复现 → 新 mount 重新武装（首轮失败 `chain` 仍 0 可重发一次） | 保留 |
| `useExploreSession` | 边界 | 终态定点重读：库态 `running` true→false → 以 `refreshKey` 重评估重放一次（消费面逻辑保持） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 mock（`ensure_explore_session` / `session_detail` / `agent_session_transcript` / `agent_session_watch` / `agent_start` / `agent_stop` 可编程并计数；`ChannelMock` 直投信封） | 全部用例 |
| jsdom 环境垫片（运行环境） | 既有环境垫片（真实 `useAgentSession` 内 `useChat` 运行所需） | 全部用例 |
| 内部模块 | 不 mock：`useAgentSession` / `buildExplorePrompt`（stance）真实组合；`useAutoSend` 闸门经公共 API 行为断言 | 全部用例 |

---

### packages/desktop/src/views/explores/explore-detail-view.tsx -> packages/desktop/src/views/explores/explore-detail-view.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`ChatPane` 为私有组件、页面组件签名不变；改动为改经 `AgentSessionView` 注入形态 + `ExploreComposer` 插槽）；以下条目取自「修改文件」表该文件行与 D9 / D10，非源文件导出扫描。 -->

- ChatPane（私有）: 改 `<AgentSessionView session={session.session} composer={({ running }) => <ExploreComposer running={running} onSend={session.send} onStop={session.stop} />} data-testid="explore-chat" />`（注入形态 + 插槽）
- composer(actions)（render prop）: 插槽内容渲染在消息区下方，`running` 取 `AgentSessionActions`（库态），`onSend` / `onStop` 接来源侧 `session.send` / `session.stop`
- session.running（终态定点重读）: 库态 running 转 false 即触发重读，逻辑不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ChatPane 接线 | 正向 | 渲染 → `explore-chat` testid 的 `AgentSessionView` 注入形态挂载；`ExploreComposer` 由 composer 插槽渲染，`running` 取自 `AgentSessionActions`（库态），`onSend` / `onStop` 接来源侧 `session.send` / `session.stop` | 修改 |
| ChatPane 接线 | 正向 | 插槽发送 → `agent_start`（stance 拼接 + `source='explore'`）；运行中呈现停止入口 → `agent_stop` | 修改 |
| 运行中禁发 | 边界 | 库态 running → composer 发送禁用、停止可点；收口后恢复可发（终态定点重读触发重评估） | 修改 |
| 会话装载失败 | 异常 | 会话查询 reject → 对话区错误提示，页面其余区域（记录头 / 面板）不塌 | 修改 |
| 空记录 / 空会话 | 边界 | 无记录（`record = null`）→ 空态且零会话查询；有记录无转录 → 组件空态提示（不虚构气泡） | 修改 |
| 记录切换 | 边界 | 切换 explore 记录 → 新会话重放重取一次、旧会话流拆除（`agent_session_watch` 并行 ≤ 1） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 mock（记录读取 / 会话查询 / `agent_start` / `agent_stop` / `agent_session_watch` 可编程并计数） | 全部用例 |
| jsdom 环境垫片（运行环境） | 既有 `ResizeObserver` / 动画垫片（双栏布局必需） | 全部渲染用例 |
| 内部模块 | 不 mock：`useExploreSession` / `AgentSessionView` / `ExploreComposer` 真实组合（页面即链路入口） | 全部用例 |

---

### packages/desktop/src/views/explores/components/explore-composer.tsx -> packages/desktop/src/views/explores/components/explore-composer.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`ExploreComposerProps`（`running` / `onSend` / `onStop`）与 testid 前缀零变化；改动为注释随动：由 `AgentSessionView` 插槽注入、`running` 取自 `AgentSessionActions`）；以下条目取自「修改文件」表该文件行，非源文件导出扫描。 -->

- ExploreComposer({ running, onSend, onStop })（props 面不变）: 由 `AgentSessionView` 插槽注入，`running` 取自 `AgentSessionActions`（库态），`onSend(input)` / `onStop()` 自来源侧透传
- testid 前缀（`explore-prompt` / `explore-send` / `explore-stop` / `explore-permission-mode` / `explore-composer`）: 留在来源侧，文案与断言面零迁移

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ExploreComposer` | 正向 | `running = false`：输入 + 提交 → `onSend({prompt, permissionMode})` 逐字（缺省 `bypassPermissions`）；stop 入口零触发 | 修改 |
| `ExploreComposer` | 正向 | `running = true`：发送禁用（点击零 `onSend`）、停止入口可点 → `onStop` 恰一次 | 修改 |
| `ExploreComposer` | 边界 | 空 prompt / 纯空白提交 → 零 `onSend`；permission-mode 三档切换后提交，档位透传 | 修改 |
| `ExploreComposer` | 边界 | `explore-prompt` / `explore-send` / `explore-stop` / `explore-permission-mode` / `explore-composer` testid 保持（change / 历史消费面无关，但 explore 页面断言依赖） | 保留 |
| `ExploreComposer` | 异常 | 提交回调抛错（`onSend` 抛出同步异常或返回 rejected promise）→ 异常不被吞（测试可捕获、组件不崩、输入值不清空、`running` 外驱不变，可再次提交）；`onStop` 抛错同式（停止入口不因之失活） | 新增 |
| `ExploreComposer` | 异常 | 非法 / 缺失入参：`running` 为 `undefined`（`AgentSessionActions` 未就绪的插槽边界）按未运行处理（发送可用、停止隐藏）；`onSend` / `onStop` 缺席时不抛错（点击 no-op，不崩页面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 无内部模块 mock | 不 mock：`AgentInput`（组件族）真实组合；`onSend` / `onStop` 用 `vi.fn()` 观测（props 为显式入参） | 全部用例 |

---

### packages/desktop/src/views/changes/flow/session-transcript-panel.tsx -> packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`SessionTranscriptPanelProps`（`root` / `roleRef` / `refreshKey`）不变；改动为删除 `useSessionTranscript` 与本地 `SessionMeta`、改经自持形态 `AgentSessionView`、保留两个 wrapper testid）；以下条目取自「修改文件」表该文件行与 D12，非源文件导出扫描。 -->

- SessionTranscriptPanel({ root, roleRef, refreshKey })（props 面不变）: 删 `useSessionTranscript` 与本地 `SessionMeta`，改自持形态 `<AgentSessionView root sessionId={roleRef.sessionId} refreshKey data-testid="session-transcript" />`
- wrapper testid（`session-transcript-panel` / `drawer-session-empty`）: 面板外壳与槽位缺席空态仍留本文件
- refreshKey（语义）: 会话状态重评估信号，变更即重放重取（不再驱动转录专用去抖）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 面板接线 | 正向 | `roleRef` 在场 → `session-transcript-panel` wrapper + `session-transcript` testid 的 `AgentSessionView` 以 `root` + `roleRef.sessionId` + `refreshKey` 自持挂载；对话 + 轮行 + 会话元信息（`session-meta*`）由组件呈现（D12 上移后断言面不变） | 修改 |
| 运行中旁观（AC-5 前端半边） | 正向 | 运行中会话（running 轮行）→ 建立 `agent_session_watch` 订阅；`Channel` 直投 `messageDelta` → 面板文本 token 级增量可见（不再「通知 → 重查转录库」） | 新增 |
| 完成态零订阅（AC-2） | 正向 | 已收口会话 → `agent_session_watch` 零调用、零轮询（推进时间 invoke 计数不增） | 新增 |
| 会话查询失败 | 异常 | `session_detail` / 转录查询 reject → 组件错误提示（既有错误断言面随组件迁移，不塌面板框架） | 修改 |
| 空态 | 边界 | `roleRef = null` → `drawer-session-empty` 空态、零查询；`roleRef.sessionId = null` → 空态（不虚构会话） | 修改 |
| 状态重评估 | 边界 | `refreshKey` 变更 → 重放重取一次（`refreshKey` 语义 = 会话状态重评估）；running 转收口时订阅拆除、不重订（并行流 ≤ 1） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 mock（`session_detail` / `agent_session_transcript` / `agent_session_watch` 可编程并计数；`ChannelMock` 直投事件与 `Record`） | 全部用例 |
| jsdom 环境垫片（运行环境） | 既有环境垫片 | 全部渲染用例 |
| 内部模块 | 不 mock：`AgentSessionView` / `useAgentSession` / `AgentMessages` 真实组合；`SessionMeta` 由组件提供（本文件不再自定义） | 全部用例 |

---

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`DetailDrawer` 签名不变；改动为 `transcriptRefreshKey` 改名 `sessionRefreshKey` 并透传至面板 `refreshKey`，语义 = 会话状态重评估）；以下条目取自「修改文件」表该文件行，非源文件导出扫描。 -->

- DetailDrawer（签名不变，`sessionRefreshKey` 语义改名）: 原 `transcriptRefreshKey` 更名，doc 标注「会话状态重评估信号」
- SessionTranscriptPanel（透传）: `sessionRefreshKey` → 面板 `refreshKey`，与 `root` / `roleRef` 同步下传

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| refreshKey 透传 | 正向 | `sessionRefreshKey` 自增 → 面板 `refreshKey` 同步（面板重取一次）；`sessionRefreshKey` 缺省时面板以 `refreshKey = undefined` 挂载（零意外重取） | 修改 |
| 抽屉角色切换 | 边界 | 切换 `roleRef`（角色 / `sessionId`）→ 面板以新会话 key 重挂（旧订阅拆除，`agent_session_watch` 并行 ≤ 1） | 修改 |
| 无关联会话 | 边界 | `roleRef = null` → `drawer-session-empty` 空态（面板零查询）；抽屉关闭后再打开，订阅状态按当前 `roleRef` 重建 | 修改 |
| 面板错误 | 异常 | 会话查询 reject → 面板错误提示，抽屉框架（标题 / 关闭 / 其余区块）不塌 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 mock（`session_detail` / `agent_session_transcript` / `agent_session_watch` 可编程并计数） | 全部用例 |
| jsdom 环境垫片（运行环境） | 既有 `BASE_UI_ANIMATIONS_DISABLED` / 动画垫片（抽屉同步卸载语义） | 全部用例 |
| 内部模块 | 不 mock：`SessionTranscriptPanel` 真实组合（透传面经面板副作用断言） | 全部用例 |

---

### packages/desktop/src/views/changes/hooks/use-change-detail.ts -> packages/desktop/src/views/changes/hooks/use-change-detail.test.ts

#### 待测功能

- `useChangeDetail(root: string | null, id: string | null): ChangeDetailState` — 删 `TRANSCRIPT_REFRESH_DEBOUNCE_MS` / `useDebouncedTick` / `notifyTranscript` / `transcriptTick`；`refreshTick: number` 为统一重查 tick（`SessionEvent` 与其余通知同走 `notifyRefresh`，300ms 尾随去抖）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `useChangeDetail` | 正向 | 通知（`notifyRefresh`）→ 300ms 一拍：`refreshTick` 自增 1、`detail` / `activeRun` / `artifacts` 重查（`refreshTick` 初始 0，供组件 `refreshKey`） | 修改 |
| `useChangeDetail` | 正向 | `SessionEvent` 通知与其余通知同走统一 tick：连续两拍 → `refreshTick` 两次自增、`get_change_detail` 两次（不再有 150ms 第二套节拍） | 新增 |
| `useChangeDetail` | 异常 | 类型面退役：返回值不再含 `notifyTranscript` / `transcriptTick`（编译期 + 运行时键缺席断言）；调用方若残留直调即编译失败 | 修改 |
| `useChangeDetail` | 异常 | 刷新失败有界重试既有用例随动：退避 1s / 2s / 4s、last-good 保持、`refreshFailed` 横幅态在去抖链退役后仍成立（重试不入 300ms 去抖通道） | 修改 |
| `useChangeDetail` | 边界 | 去抖合并：300ms 窗内 5 次通知 → 恰一拍（尾随去抖，最后一拍必达）；卸载 / 换 id 后推进时间 → 零遗留计时器、零补发 | 修改 |
| `useChangeDetail` | 边界 | 既有回归保留：id / root 为 null 早退零 invoke；显式 `refresh()` 即时（不经去抖）；无失败时零轮询（推进 5min 计数不增） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：`get_change_detail` / `read_artifact` 按命令名与调用序可编程（成功 / reject / `null`）并计数 | 全部用例 |
| 计时面（运行环境） | `vi.useFakeTimers` + `advanceTimersByTime`（300ms 去抖 / 重试退避逐拍推进），`afterEach` 复位 | 去抖合并 / 重试用例 |
| 内部模块 | 不 mock：去抖与重试的文件私有实现真实组合（经公共 API 驱动） | 全部用例 |

---

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`ChangeDetailView({ root })` 签名不变；改动为 `onNotice` 不再分流（`SessionEvent` 与其余同走 `notifyRefresh`）、`transcriptTick` → `refreshTick` 并下传 `sessionRefreshKey`，`DetailContent` / `DetailLoaded` 形参改名）；以下条目取自「修改文件」表该文件行与 D13，非源文件导出扫描。 -->

- ChangeDetailView({ root })（页面签名不变）: `onNotice` 不再分流——`SessionEvent` 与其余通知同走 `notifyRefresh`（统一 300ms 重查 tick）
- refreshTick（统一 tick）: `transcriptTick` 退役，`refreshTick` 作 `sessionRefreshKey` 下传抽屉面板
- DetailContent / DetailLoaded（形参同步改名）: `transcriptTick` → `refreshTick`、`transcriptRefreshKey` → `sessionRefreshKey`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 通知分流收敛（AC-5） | 正向 | `Channel` 直投 `SessionEvent` 通知 → 与其余通知同走 300ms 统一重查：`refreshTick` 自增 1、抽屉面板 `refreshKey` 同步（原 150ms 转录 tick 断言改写）；不再出现「通知 → 重查转录库」独立节拍 | 修改 |
| 通知分流收敛（AC-5） | 正向 | 混合通知序列（`Step` + `SessionEvent` + `Finished`）→ 去抖窗内合并为恰一拍重查（`get_change_detail` 计数 1） | 新增 |
| last-good 横幅回归 | 正向 | 刷新失败轮保 `detail` / `activeRun` / `artifacts`、`refresh-stale-note` 呈现与消失、重试节奏既有用例不变 | 保留 |
| 抽屉接线 | 边界 | 选中角色 / 会话切换 → 面板 `sessionRefreshKey` 与 `roleRef` 同步；面板订阅随会话切换拆除（并行流 ≤ 1） | 修改 |
| 首载 / 不存在 | 异常 | 首载 reject → 整页错误兜底；重查返 `null` → 「未找到该 change」兜底（既有分流回归） | 保留 |
| 抽屉面板零动作面 | 边界 | 面板内不呈现发送输入 / 停止按钮（只读消费点，无 composer 插槽），`sessionRefreshKey` 变更仅触发重取 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / `Channel`（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`（`get_change_detail` / `read_artifact` / `agent_session_transcript` / `session_detail` / `agent_session_watch` 可编程并计数；`ChannelMock` 直投 `RunNotice` 与订阅信封） | 全部用例 |
| jsdom 环境垫片（运行环境） | 既有 `ResizeObserver` / `DOMMatrixReadOnly` / `getBBox` 等 xyflow 垫片 | 全部渲染用例 |
| 计时面（运行环境） | `vi.useFakeTimers` + `advanceTimersByTime`（300ms 统一去抖 / 重试退避） | 通知合并 / 横幅用例 |
| 内部模块 | 不 mock：`useChangeDetail` / `useChangeFlowRun` / `SessionTranscriptPanel` / `AgentSessionView` 真实组合 | 全部用例 |

---

## 不可测试项

| 被测文件或模块 | 原因 |
|----------------|------|
| `packages/desktop/src-tauri/src/main.rs` | 改动为壳层 `setup` 内 `app.manage(Arc::new(SessionStreamRegistry::default()))`（与 `StopRegistry` 同型空表托管）。Tauri `Builder::setup` 闭包依赖真实 `home_dir` 与 `run` 生命周期，无进程内测试路径；等价托管语义由 `src/commands/exec/mod_test.rs` / `src/commands/change_flow/mod_test.rs` 的 `mock_app` + `app.manage(...)` 装置承载 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 改动为 `all_commands!` 宏增 `agent_session_watch` 单一路径登记（纯路径注入、零运行时逻辑）。登记正确性由编译面（`generate_handler!` / builder 双消费同一宏）承载，typed 包装面由 `bindings:export` 再生成 + `bindings:check` 守卫承载；无独立单元测试路径 |
| `packages/desktop/src/types/generated/bindings.ts` | codegen 生成物禁手改：`agentSessionWatch` typed 包装（`onEvent: Channel<AgentRunMessage>` + `sinceSeq: number`）由 `pnpm -C packages/desktop run bindings:export` 再生，`bindings:check`（`git diff --exit-code -- src/types/generated`）承接零漂移（AC-8 静态半边，桌面端执行门禁） |
| `packages/desktop/package.json` | `version` 0.5.16 → 0.5.17 与 `check` 改走 `bindings:check` 为配置 / 脚本机械变更，无运行时逻辑；AC-8 的 `run check` / `run test` 全绿由桌面端静态门禁执行，无 agent 侧测试落点 |
| change 轨真实 walker 端到端（`change_flow/mod.rs` 链路） | `change_flow_start` → walker executor 步需要真实引擎与会话进程，进程内不可确定性构造（既有测试以 PATH shim / test-only 步规避）。同源写入点由 `crates/infra/agent/src/compose_test.rs` 的 `KernelWorkerPort::run` 真实驱动（change 轨形态）与 `kernel.rs` 的 `drive` 单点发布用例承载；`change_flow/mod_test.rs` 以受理 / 控制 / 通知面零回归断言注入缝不改语义（AC-5 后端半边组合承载） |
| 「视图层零 `eventsToUIMessages` 直调 / 零自有转录查询 hook」（AC-1 静态半边） | 该约束为「不存在某调用」的静态性质：`hooks/use-agent-chat.ts` 与 `views/changes/hooks/use-session-transcript.ts` 删除后由编译面 + `knip` 未使用导出报告承载（`eventsToUIMessages` 仅剩适配层内部与测试使用面），无单元测试落点；运行时面由四个消费点用例（debug / explore / change 抽屉 / 历史重放）断言「数据经 `AgentSessionView` 流入」间接锁定 |
| delta 视觉连续性（半截文本窗口、观赏中途接入无前文） | design 待决 5：`V6` 仅验证「能归约」，视觉连续性未验证且按「过程近似」接受；无确定性断言面，不在本期测试范围 |
| `agent_session_watch` 的 Wry 句柄构造路径（`src/commands/exec/mod.rs`） | `AppHandle<Wry>` 需真实 Tauri 运行时（webview）构造，进程内不可得；生产入口为一行转发，语义与参数面由 `agent_session_watch_with`（`mock_app` 注入 MockRuntime）全量用例覆盖，登记面由 `all_commands!` + `bindings:check` 承载 |
| lagged 实测频率 / 容量调优（`SESSION_STREAM_CAPACITY = 4096` 是否够） | 容量行为边界由 `stream_test.rs` 的 4096 / 4097 条用例锁定；真实 delta 密度下的溢出频率属真机实测（桌面端走查），不进进程内单测 |
