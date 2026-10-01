# 设计: agent-core-session-kernel

> **变更**: agent-core-session-kernel
> **日期**: 2026-10-01

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 四个能力 delta（desktop-agent-execution / desktop-workspace-store / desktop-agent-chat-infra / desktop-agent-management）已由提案阶段写入并通过评审（workflow.json proposal attempt 1 verdict pass）。本文件只覆盖实现文件；提案与规格产物不在变更清单与任务列表内。探索稿（域根下 `explores/` 目录的 agent-core-session-kernel 与 rig-package-selection 两稿，只读输入）的「协议终形」节为协议形状权威来源。

## 设计裁定（对提案待决问题的定稿）

| 待决问题 | 裁定 | 理由 |
|---|---|---|
| core ↔ store crate 落点 | **core 定 port、infra 实现**。`crates/core/agent` 零 workspace 依赖保持不变，desktop-crate-layout 无需修订；`agent-runtime` 新增 `store` workspace 内依赖，持久面与查询面 port 由其 store 适配实现 | 执行能力 delta 明文「agent-runtime SHALL 实现内核 port（含 store 持久面与查询面要求的落地）」已授权此边；desktop-crate-layout 的依赖 scenario 为正向枚举且无 infra→infra 禁令，仅 core→store 需修订（未选择该形态） |
| 存量 run/事件数据处置 | **分层处置**：`SessionRecord` / `SessionEventRecord` 全新模型 additive 注册（零迁移）；`AgentRunRecord` 经 native_model 版本 3→4 原地演进（存量行升级为无会话归属孤儿轮行，统计字段保留）；旧事件模型 `AgentEventRecord`（id=3）定义自 workspace 组退役，旧行成惰性废弃死数据（援引旧单库惰性废弃先例），零迁移代码路径 | 与 workspace-store delta「additive 打开无迁移路径」+「既有记录 native_model 版本机制自动升级」两条 scenario 逐字对齐；避免为一次性历史数据引入迁移层 |
| delta 配对键 | **沿用 `parent_tool_use_id`**（`MessageDelta` 携带 `Option<String>`，SDK 引擎恒 `None`），不另立显式 correlator | 与既有子代理归因词汇同源，前端 provisional 键 = 配对键一处两用；SDK 无子代理，恒 None 即单 provisional 通道 |
| seq 语义 | **共享单调 seq 空间**：delta 占 seq（盖戳治理单点、传输/落库两路 seq 可比对）；库内重放为密封事件 seq 升序、容忍空洞（排序键语义合法） | 单一时钟线最简；store 查询按 seq 升序返回，空洞不破坏「seq 有序可重放」scenario |
| Stop 消息 vs RunHandle | **保留 `RunHandle` 薄包装**（竞态语义已验证，零改动）；停止注册（内存态）收内核治理面，键 = core session id（core 铸 id 进程内全局唯一，免复合键） | spec 明文「`RunHandle` 竞态语义保留为默认形态」；actor 统一为逻辑层：协议 open_session/ask 两步，CLI 每轮进程、SDK 每轮泵任务，会话为逻辑寻址单位 |
| ctx 首版字段面 | `SessionCtx { workspace_root: PathBuf, permission_mode: AgentPermissionMode }`；serde 不加 `deny_unknown_fields`（未知字段忽略），additive 演进。`SessionInjections { preamble, tools }` 中 tools 为切片③预留接口位，MVP 两引擎忽略（SDK 用自建六工具、CLI 用自有工具面） | 「可扩展 + 未知忽略」与 Raw 兜底同哲学；tools 注入延后不动协议形状 |
| 命令面命名 | **发起/终止保留 `agent_start` / `agent_stop` 语义升级**（stop 寻址键 run_id → session_id；start 参数 `resume_session_id`/`parent_run_id` → `session_id`）；**查询面更名会话域命令** `agent_sessions` / `agent_session_transcript`，退役 `agent_runs` / `agent_run_events` / `agent_run_chain` | start/stop 命名仍准确、前端调用点零迁移；查询换域换名避免 run/session 语义混淆（查询命令本就要随会话域全部重写） |
| TurnDone 与 RunResult 线格式 | **更名不别名**（serde 线值 `runResult` → `turnDone`）：旧事件行随惰性废弃无读取路径，无兼容读负担。`AgentRunParams` / `AgentRun` 结构随协议演进退役；`AgentStartError` 变体集不变（CliMissing / SpawnFailed / ConfigMissing） | management delta「变体集 MUST NOT 再增」+「core 无 engine 字样」；协议参数面由 SessionOpen/TurnQuestion/SessionCtx 取代 |
| SessionEventRecord 键位 | native_db 复合主键不受支持（model.rs 既有 Spike① 留痕）→ **合成打包 u128 主键 `(hash64(session_id) << 64) \| seq`**（大端序字典序 = seq 序）+ `session_id` String 非唯一二级索引保查询形态；serde 沿用十六进制串定式（`event_key_serde` 同款）。`SessionRecord` 字符串主键直用 core 铸 id（`WorkspaceRecord.root` 字符串主键既有先例，无需 spike） | 完全复用 `AgentEventRecord` 已验证的打包键 + 二级索引模式，查询形态与级联圈定不变 |
| 组合根落点 | `agent-runtime` 新模块 `compose.rs`：`resolve_agent_engine` 解析语义原样平移（默认/显式/sdk provider 组装/high 档/无默认 Err）+ Continue 快照校验 + port 装配；`store_port.rs` 承载 `SessionSink` / `SessionQuery` 的 store 实现 | 解析单点自命令层下沉、命令层零解析残留（management delta scenario）；引擎比对与 provider 组装只有 infra 可做（core 禁 engine 概念） |
| 密封替换机制（前端） | 依据 ai v7.0.118 reducer 实测语义钉死：`start` chunk 仅重命名在编消息（`state.message.id`）、`reset-step` 移除当前 step 部件。delta 路：配对键**首个 delta** 由 transport 流内簿记发 `start(provisional 键) + reset-step + text-start/reasoning-start`，后续 delta 直发 `text-delta` / `reasoning-delta`（稳定 part id 累积增长）；密封 Message 到达发 `start(evt-<seq>) + reset-step + 整块部件组`——在编消息重命名让位即「同键替换」。`eventToChunk` 保持纯逐事件签名；首 delta 簿记为流闭包内局部状态（非会话状态，不违 transport 无状态转换器约定——该约定禁的是会话参数进构造闭包/ref 插线） | reducer 源码逐 case 核实（`text-start` 重复推送重复部件、`text-delta` 按 activeTextParts 累积）；机制与既有逐事件 start+reset 模式同构，实现期由适配层纯函数测试锁定 |
| 前端镜像与重放 | hook 镜像**只收密封事件**（delta 只上 chunk 流不进镜像）→ `eventsToUIMessages` 归一与原始转录视图天然密封-only（delta 仅实时流可见）。重放合成 = 转录 `eventsToUIMessages` + 每个 TurnDone 事件位后插入对应轮统计行 `data-run-record` 部件（按轮序对齐） | 调试页 spec「delta 仅实时流可见，不进入原始转录视图」；终态部件两路同构（chat-infra delta scenario） |
| SDK resume 归属 | `owns_session` 前缀校验退役——归属与引擎路由凭 `SessionRecord.config_snapshot`（store 侧定型 `AgentEngineKind`）。Continue 由组合根读会话行并比对快照 engine：会话不存在 / 非 SDK 产出（快照 engine ≠ 解析 kind）/ `engine_session_id` 缺失 → `AgentStartError::ConfigMissing` 显式失败。SDK remote id 保持每轮 `sdk-` 前缀铸造（经 `RunStarted`/`TurnDone` 的 session_id 上报，内核写双 id 映射落库半边） | management delta「core 无 agent/engine 字样」决定比对只能在 infra；前缀是引擎自证，快照是记录自证——后者与配置快照裁定一致 |
| 盖戳归属 | **盖戳治理收内核**：引擎 → 内核通道载未盖戳 `AgentEventKind`，内核泵统一盖 seq/时间戳（`AgentEvent::stamp` 保留为盖戳原语）；store/前端词汇不变（`AgentEvent` 盖戳信封） | 执行能力 delta「seq/时间戳盖戳治理收内核」；统计口径唯一不变量由内核同点位兑现 |

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 事件词汇（delta/sealed 双层） | `AgentEvent` 盖戳信封、`AgentEventKind` 六变体（新增 `MessageDelta`、`RunResult`→`TurnDone`）、`AgentBlock` 四块、密封/增量判别 | `packages/desktop/src-tauri/crates/core/agent/src/event.rs`（修改） | serde / specta / serde_json | Rust 枚举（serde 内部 tag camelCase，线格式 = 落库形态（密封） = DTO 镜像） |
| P1 引擎协议 | `AgentRunner::open_session`（会话建立：injections + ctx + session 引用 + prior_handle）、`AgentSession`（观察回流 + 轮驱动 + 句柄）、`RunHandle` 保留 | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs`（修改） | tokio（mpsc/Notify）、serde / specta | Rust trait + 有界 mpsc（进程模型不可见，四不变量承载） |
| 轮收敛状态机 | running → completed/failed/stopped，`TurnDone` 驱动收敛，显式停止独立分支，终态幂等 | `packages/desktop/src-tauri/crates/core/agent/src/state.rs`（修改） | 无 | 纯状态机 |
| 会话域类型 | `SessionRef` 之外的会话记录契约类型：`SessionProvenance` / `SessionRow` / `NewSessionRow` / `SessionStats` / `SessionSummary` / 会话 id 铸造 | `packages/desktop/src-tauri/crates/core/agent/src/session.rs`（新增） | serde / specta / serde_json / std 时间原语 | 纯类型（core 铸 `ses-` 前缀 id） |
| 持久/查询 port | `SessionSink`（write-through：建会话/开轮/密封追加/轮收尾/双 id 落库）与 `SessionQuery`（清单聚合/转录重放/对账重导）契约 | `packages/desktop/src-tauri/crates/core/agent/src/port.rs`（新增） | 无（仅依赖本 crate 类型） | Rust trait（core 定契约、infra 实现） |
| 会话内核编排 | 治理面（盖戳/状态机驱动/`StopRegistry` 停止注册）+ 运行面驱动（open → 建行 → ask → 泵观察流：delta 只上传输、密封 write-through、TurnDone 统计收口）+ `begin_turn` 提前 resolve | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs`（新增） | 本 crate 各模块、tokio | 有状态内核（IO 一律经 port，无直接文件/进程/网络 IO） |
| crate 导出面 | 模块声明与 pub use 集合（纯度自查触点：无 claude/引擎/Tauri 字样、无 workspace 内依赖） | `packages/desktop/src-tauri/crates/core/agent/src/lib.rs`（修改） | — | Rust mod 树 |
| 组合根（解析单点） | 运行发起解析（自命令层平移：默认/显式 agent → `EngineKind` + `EngineConfig`，sdk 由 provider 组装取 high 档，无默认 Err 引导管理页）+ Continue 快照校验与 prior_handle 提取 + resume 装载缝闭合 + 内核装配 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`（新增） | agent、store、agent-runtime 门面 | Rust（`runner_for` / `EngineConfig` 零 diff 消费） |
| store port 适配 | `SessionSink` / `SessionQuery` 的 store 实现（write-through 原子操作、聚合现算、对账重导、delta 防御性忽略） | `packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs`（新增） | agent（port 契约）、store | Rust trait impl over `Arc<Store>` |
| 引擎门面 | `EngineKind` → 引擎构造唯一 match（`runner_for` 签名与 `EngineConfig` 三字段零 diff）；resume 装载缝语义平移为会话全史 | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`（修改） | agent、rig-core | Rust 门面 |
| SDK 流面归一化 | rig 流项 → 双层产出：`Text`/`ReasoningDelta`/完整 `Reasoning` → `MessageDelta`；完整 ToolCall → 轮末密封收口（不出中间事件）；`Unknown` → `Raw`；簿记项 → None | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs`（修改） | rig-core、agent | 纯函数 |
| SDK loop 密封收口 | 多轮 loop：delta 逐发、轮末 `choice` 收口为**恰一条**密封 `Message`（全部块收进）、ToolResult 密封、`TurnDone` 组装（统计字段面唯一出口）、停止/失败语义保留 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`（修改） | normalize/policy/sandbox/tools、agent、rig-core | 手搓 loop（rig 仅作 provider client + 类型层） |
| SDK 会话全史重建 | 会话全史转录 → rig 对话历史（顶层非 Raw 密封事件、tool 成对回灌）；`owns_session` 前缀校验退役（归属凭配置快照） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs`（修改） | agent、rig-core | 纯函数 |
| SDK runner | P1 协议实现：open 阶段配置三件套校验 + Continue 全史装载重建，ask 阶段泵 select（停止即 drop future） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`（修改） | agent、rig-core、ResumeTranscript 缝 | tokio 泵任务 |
| CLI runner | P1 协议实现（行为零回归）：open 无 IO、ask 按 flag 组装 spawn + 逐行泵 + 树杀缝；`--resume` 走 prior_handle | `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs`（修改） | agent、tokio::process、flags/jsonl | 进程租户（唯一 spawn 触点） |
| CLI flag 组装 | 入参形状平移（协议轮参数），flag 输出序列零变化（含 `--resume <id>` 尾追加） | `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs`（修改） | agent | 纯函数 |
| CLI jsonl 归一化 | 变体更名适配（`RunResult` → `TurnDone`），行解析语义零变化 | `packages/desktop/src-tauri/crates/infra/agent/src/cli/jsonl.rs`（修改） | agent | 纯函数 |
| SDK 模块文档 | 模块文档措辞随变体更名同步（`RunResult` → `TurnDone`） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（修改） | — | 文档 |
| store 模型面 | `SessionRecord`（新）、`SessionConfigSnapshot`（新嵌套 struct）、`SessionEventRecord`（新转录单表）、`AgentRunRecord` v4 轮统计行化（from_previous 演进） | `packages/desktop/src-tauri/crates/infra/store/src/model.rs`（修改） | agent（纯类型嵌装）、native_db / native_model / serde | native_db derive（core 类型 derive-free 不变） |
| store 操作面 | 会话域 write-through 原子操作 + 会话查询/聚合现算 + 对账重导 + explore 级联圈定平移会话 + workspace 模型组注册更新 + run 域旧 API 退役 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（修改） | model、native_db | 同步事务（红黑树库，单写多读） |
| store 导出面 | 模型与操作 re-export 同步（新模型出、退役 API 收） | `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（修改） | model / store | Rust pub use |
| 命令面（薄包装） | `agent_start` / `agent_stop` 语义升级 + `agent_sessions` / `agent_session_transcript` 新增 + run 域三查询退役；命令体三件事（参数转换 → 调用 → 错误映射） | `packages/desktop/src-tauri/src/commands/exec/mod.rs`（修改） | agent（内核公共 API）、agent-runtime（组合根）、store、tauri | Tauri command + specta |
| 命令编排退役位 | 保留 `AgentRunMessage` IPC 信封与 running/终态 `TurnSummary` 装配 helper；`find_events_by_session` / `resolve_agent_engine` / `start_agent_run*` / `drive_agent_run` / `RunStopRegistry` / `RunProvenance` 编排体退役下沉 | `packages/desktop/src-tauri/src/commands/exec/agent.rs`（修改） | agent、store | Tauri Channel 信封（app 层 IPC 类型，非 core 契约） |
| 托管状态挂载 | `RunStopRegistry` → 内核 `StopRegistry` 挂载点替换 | `packages/desktop/src-tauri/src/main.rs`（修改） | agent、store | Tauri managed state |
| 事件适配层（纯函数） | `eventsToUIMessages` 密封-only（防御跳过 delta）+ `eventToChunk` 增 delta 路（`text-delta` / `reasoning-delta`，稳定 part id）+ provisional 键 / part id 助手导出 + `turnDone` 映射 | `packages/desktop/src/lib/agent-adapter.ts`（修改） | ai（UIMessage / UIMessageChunk）、generated bindings | 纯函数（保真锁定：seq、`parentToolUseId`、`data-raw` 透传） |
| 传输适配 | body 参数会话域化（`sessionId` 取代 `resumeSessionId`/`parentRunId`）+ 流内首 delta 簿记（开 provisional 件）+ 密封/aux chunk 组不变 | `packages/desktop/src/lib/agent-transport.ts`（修改） | agent-adapter、generated bindings、@tauri-apps/api（Channel）、ai（ChatTransport） | `TauriAgentTransport`（流闭包局部簿记） |
| 会话基建 hook | 会话域状态：会话寻址重放装载（清单 + 转录）、密封镜像、Continue 组装、stop 会话寻址、收口归一 | `packages/desktop/src/hooks/use-agent-chat.ts`（修改） | agent-adapter / transport、generated bindings、@ai-sdk/react | `useChat` + 自定义 transport |
| 探索链消费面 | source/sourceRef 会话寻址与 Continue 语义等价切换（对外状态形状与行为不回退） | `packages/desktop/src/views/explores/hooks/use-explore-session.ts`（修改） | use-agent-chat、explore-stance | hook |
| Agent 调试页 | 会话历史区接线 + 流式呈现参数传递（页面级 chrome，时间线组件零改动复用） | `packages/desktop/src/views/agent/agent-debug-view.tsx`（修改） | components/agent、use-agent-chat | 视图 |
| 历史会话组件 | run 行列表 → 会话行列表（时间/轮数/终态）+ 点开转录重放（密封-only） | `packages/desktop/src/views/agent/components/agent-run-history.tsx`（修改） | agent-adapter、components/agent | 视图 |
| 历史会话 hook | `agent_sessions` 清单取数 + `agent_session_transcript` 重放取数（显式触发、无轮询） | `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts`（修改） | generated bindings | hook |
| IPC 类型镜像 | 信封（`MessageDelta` / `TurnDone` / `SessionRecord` 域类型 / 命令签名）再生成，非手改 | `packages/desktop/src/types/generated/bindings.ts`（再生成，经 `pnpm -C packages/desktop run bindings:export`） | export-bindings | 生成物（同输入零 diff） |
| 版本 bump | 用户可见变更（碎行修复 + 续会话修复）版本提升 | `packages/desktop/package.json`（修改） | — | 0.3.13 → 0.3.14 |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。 -->
<!-- 覆盖 proposal「变更范围 - 实现文件」全部 13 条；清单外补入条目（compose.rs / store_port.rs / Cargo.toml / cli jsonl+flags / sdk mod.rs / commands mod.rs / main.rs / store lib.rs）均为设计裁定的实现落点，逐条标注。 -->
<!-- 测试文件不在本清单（proposal「测试文件」由 test-design / test-gen / test-execution 阶段承接）。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | 会话域契约类型（`SessionProvenance` / `SessionRow` / `NewSessionRow` / `SessionStats` / `SessionSummary`）与 `ses-` 前缀会话 id 铸造；纯类型，serde camelCase + specta |
| `packages/desktop/src-tauri/crates/core/agent/src/port.rs` | `SessionSink` / `SessionQuery` port 契约（core 定接口、infra 实现）；`TurnOutcome` 共享收口类型 |
| `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 会话内核编排：`StopRegistry`（内存态停止注册）、`SessionKernel`（`begin_turn` 提前 resolve + `drive` 泵驱动）、`TurnRequest` / `RunningTurn` / `KernelOutput` |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | 内核组合根：运行发起解析单点（自命令层平移）、Continue 快照校验与 prior_handle 提取、resume 装载缝闭合、内核装配（清单外补入——解析下沉的落点文件） |
| `packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs` | `SessionSink` / `SessionQuery` 的 store 适配实现（清单外补入——port 实现的落点文件） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | `AgentEventKind` 增 `MessageDelta { parent_tool_use_id, delta: AgentDelta }`（ephemeral）；`RunResult` 更名 `TurnDone`（线值 `runResult` → `turnDone`）；新增 `AgentDelta`（Text/Thinking 内部 tag 枚举）；新增 `is_delta()` / `is_sealed()` 判别 | 协议双层词汇核心；`AgentEvent::stamp` 盖戳原语保留 |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | `AgentRunner::start` → `open_session(SessionOpen)`；新增 `SessionOpen` / `SessionInjections` / `SessionCtx` / `SessionRef` / `TurnQuestion` / `AgentSession`；`RunHandle` 与 `AgentStartError` 变体集零改动；退役 `AgentRunParams` / `AgentRun` / `AgentEnvMode`（随协议与 v4 轮行退役） | P1 引擎协议落地；trait 面无进程模型、无 agent/engine 选择参数 |
| `packages/desktop/src-tauri/crates/core/agent/src/state.rs` | `RunStateMachine::apply` 匹配变体 `RunResult` → `TurnDone`；收敛语义（首收敛生效、stopped 独立分支、终态幂等）零变化 | 纯更名适配 |
| `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` | 模块声明与导出面扩展（session / port / kernel）；退役类型出导出面；模块文档纯度措辞更新 | 纯度自查触点：源码无 claude/引擎字样、无 Tauri 依赖、Cargo.toml 零 workspace 内依赖 |
| `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml` | 新增 `store` workspace 内依赖（清单外补入——port 实现的依赖边） | infra→infra 边；desktop-crate-layout 无禁令（仅 core→store 受限且未采用） |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 挂载 compose / store_port 模块；`ResumeTranscript` 装载缝语义平移为会话全史转录（类型形状不变）；门面 `runner_for` / `EngineConfig` 零 diff | 门面唯一 match 点保持 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs` | 双层化：`Text` / `ReasoningDelta` / 完整 `Reasoning` → `MessageDelta`（text/thinking 可辨）；完整 `ToolCall` 不再立即出密封事件（轮末收口）；`Unknown` → `Raw`；`Final` / `ToolCallDelta` 簿记项照旧 None | 碎事件根因修复（传输半边）；纯函数签名演化（产出双层枚举） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | 轮末 `choice` 收口恰一条密封 `Message`（Text/Thinking/ToolUse 全部块收进）；`RunResult` → `TurnDone` 组装（统计字段面唯一出口、cost 恒 None）；空轮/熔断/API 失败收敛语义保留 | 碎事件根因修复（落库半边的引擎侧配合）；工具执行/policy/sandbox 代码不动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs` | `rebuild` 重建源语义平移为会话全史转录（顶层非 Raw 密封事件、tool 成对回灌不变）；`owns_session` / `SESSION_PREFIX` 前缀校验退役（归属凭配置快照，`sdk-` 前缀仅保留为 remote id 铸造格式） | 链式丢上下文根因修复的重建半边 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `start` → `open_session`/ask 两段：open 阶段配置三件套校验 + Continue 经装载缝取全史转录重建（Err/None → `ConfigMissing`）；ask 阶段泵 select（停止 drop future）；观察通道改载未盖戳 `AgentEventKind`（盖戳收内核） | 协议实现适配；sdk remote id 每轮铸造经 `RunStarted`/`TurnDone` 上报 |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs` | `start` → open/ask 两段：open 无 IO（参数留存），ask 按既有流程 spawn + 逐行泵 + EOF 合成收敛（`RunResult` → `TurnDone`）+ 树杀缝；行为零回归 | CLI 每轮进程 = 逻辑 actor 的 ask 实现 |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs` | `build_args` 入参自 `AgentRunParams` 平移为协议轮参数形状；flag 输出序列零变化（含 `--resume <id>` 尾追加规则） | 清单外补入——协议参数面更替的签名适配 |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/jsonl.rs` | 归一化产出变体 `RunResult` → `TurnDone`（含文档措辞）；行解析语义零变化 | 清单外补入——变体更名的机械适配 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 模块文档 `RunResult` 措辞同步 `TurnDone` | 清单外补入——更名归零（grep 无残留） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 新增 `SessionRecord`（id=7：core 铸 id 字符串主键 / `engine_session_id` / `config_snapshot` / `source` / `source_ref` / 时间戳）与 `SessionConfigSnapshot`（engine/model/permission 嵌套 struct，store 本地定型）；新增 `SessionEventRecord`（id=8：打包 u128 主键 + `session_id` 二级索引 + 密封 `AgentEvent` 嵌装，serde_json 编解码）；`AgentRunRecord` 版本 3→4 轮统计行化（`session_id: Option<String>` 挂 core 会话，from_previous 存量行置 None；prompt/cwd/env/permission_mode/source/source_ref/parent_run_id 随版本退役） | 转录单表 + 轮统计行；core 类型 derive-free 不变 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增会话域操作（`create_session` / `find_session` / `begin_agent_turn` / `append_session_events`（delta 防御性忽略）/ `finish_agent_turn` / `bind_session_remote` / `list_sessions`（聚合现算）/ `list_session_events` / `reconcile_session_stats`（对账重导显式入口））；workspace 模型组注册 `SessionRecord` + `SessionEventRecord`、移除 `AgentEventRecord` 注册；`delete_explore_record` 级联圈定自 runs 平移至会话（转录 + 轮统计行同事务删）；退役 `begin_agent_run` / `finish_agent_run` / `list_agent_runs` / `list_agent_run_events` / `append_agent_run_events` / `restore_run_chain` 与 `EXPLORE_RUN_SOURCE` 常量平移 | 双库布局骨架、注入式打开、零迁移纪律不变；全表读 + 内存过滤哲学沿用 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | 模型与操作 re-export 同步（`SessionRecord` / `SessionConfigSnapshot` / `SessionEventRecord` 出、退役 API 收） | 清单外补入——crate 根导出面同步 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `agent_start` 参数面（`resume_session_id`/`parent_run_id` → `session_id`）编排调用改走组合根 + 内核；`agent_stop` 寻址键 run_id → session_id（root 参数保留）；新增 `agent_sessions` / `agent_session_transcript`；退役 `agent_runs` / `agent_run_events` / `agent_run_chain`；blank root 守卫与 `Result<T, String>` 模板保留 | 命令体三件事纪律；无引擎/agent 解析残留 |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | 保留 `AgentRunMessage` IPC 信封（`Record` 臂载 `TurnSummary`）与 running/终态 `TurnSummary` 装配 helper（自 `TurnRequest` / `RunningTurn` / `TurnOutcome` 中性数据装配）；退役 `find_events_by_session` / `RunProvenance` / `RunSummary` / `resolve_agent_engine` / `ResolvedEngine` / `RunStopRegistry` / `start_agent_run*` / `drive_agent_run` / `running_record` 全量编排体（下沉内核） | 函数级退役位（proposal「删除文件」节兑现）；命令体薄包装的数据装配仅剩 IPC 镜像 |
| `packages/desktop/src-tauri/src/main.rs` | 托管状态 `RunStopRegistry` → `agent::StopRegistry` 挂载替换 | 清单外补入——治理面类型换源 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 export-bindings 再生成：`AgentEventKind`（+`messageDelta`、`turnDone`）、`SessionRow` / `SessionSummary` / `TurnSummary` 域类型、命令签名（`agentStart` / `agentStop` / `agentSessions` / `agentSessionTranscript`）；`AgentRunRecord` 出 IPC 镜像（store 内部类型） | 生成物非手改；同输入连续两次导出零 diff |
| `packages/desktop/src/lib/agent-adapter.ts` | `eventsToUIMessages` 密封-only（防御跳过 `messageDelta`）；`eventToChunk` 增 delta 路（`messageDelta` → `text-delta` / `reasoning-delta` chunk，稳定 part id 派生自配对键）；`runResult` 映射 → `turnDone`；新增 `provisionalMessageId` / `deltaPartId` 助手导出；`runRecordToUIMessage` 入参演进 `TurnSummary`；密封 Message 仍走 start+reset+整块组（同键让位替换）；未知事件 `data-raw` 透传不变 | 保真锁定不变（seq 进 id/metadata、`parentToolUseId` 进 metadata） |
| `packages/desktop/src/lib/agent-transport.ts` | body 参数会话域化（`sessionId: string \| null` 取代 `resumeSessionId` / `parentRunId`）；流闭包内首 delta 簿记（配对键 → 已开件标记，首个 delta 补发 start(provisional 键)+reset-step+part-start）；密封/aux chunk 组装与 Record 收尾不变 | 传输为无状态转换器约定不变（簿记为流内局部状态，非会话状态） |
| `packages/desktop/src/hooks/use-agent-chat.ts` | 状态域会话化：重放装载改 `agentSessions`（latest updated_at）+ `agentSessionTranscript`（全史密封转录）；镜像只收密封事件；`sendMessage` body 收口 `sessionId`（Continue）；`stop` → `agentStop(root, sessionId)`；收口归一重建含 `TurnSummary` 轮行 `data-run-record` 部件交错（TurnDone 事件位与轮序对齐） | 对外 `UseAgentChatState` 增 `session` 镜像、`chain` 平移为 `TurnSummary` 轮行列表；两路同构不变 |
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | 会话寻址语义等价切换（source/sourceRef 经基建消费）；对外返回形状（messages/chain/events/loading/running/error/send/stop）与行为不回退，stance 拼接留本层 | AC-11 承载面 |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 历史区改会话历史接线；每跑重置（New 会话）与流式呈现参数保持 | 页面级 chrome；时间线组件零改动复用 |
| `packages/desktop/src/views/agent/components/agent-run-history.tsx` | run 行列表 → 会话行列表（updated_at / 轮数 / 终态）+ 点开会话转录重放（密封-only `eventsToUIMessages`） | 组件文件名保留（内容会话域化） |
| `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 取数改 `agentSessions(root, source, sourceRef)` + `agentSessionTranscript(root, sessionId)`；显式触发、无轮询纪律保留 | 清单内文件语义平移 |
| `packages/desktop/package.json` | `version` 0.3.13 → 0.3.14 | 用户可见变更（碎行修复 + 续会话修复）；desktop 包不受插件 bump 规则约束 |

### 删除文件

<!-- 无整文件删除（proposal 已裁定）。函数级退役位见「修改文件」表：sdk/resume.rs 的 owns_session、commands/exec/agent.rs 的编排体全量、store.rs 的 run 域六 API、core runner.rs 的 AgentRunParams/AgentRun/AgentEnvMode、命令面三查询——均以就地退役承载，不产生整文件删除。 -->

### 公共函数 / API

<!-- identifier：模块级导出函数 / trait 契约方法 / Tauri 命令 / 导出 class 公开方法。trait impl 方法不重复列（实现于所在文件行说明）。 -->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `AgentRunner.open_session` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 修改 | `fn open_session(&self, open: SessionOpen) -> Result<AgentSession, AgentStartError>` | P1 协议唯一入口（取代 `start`）：会话建立（injections + ctx + session 引用 + prior_handle），启动阶段失败不产生任何记录 |
| `SessionKernel::new` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `pub fn new(sink: Arc<dyn SessionSink>, registry: Arc<StopRegistry>) -> Self` | 内核构造：持久面 port + 停止注册（组合根装配） |
| `SessionKernel::begin_turn` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `pub fn begin_turn(&self, runner: Arc<dyn AgentRunner>, request: TurnRequest) -> Result<RunningTurn, AgentStartError>` | 会话建立与轮注册的同步段：open（失败不落库）→ New 建会话行 → 开轮行 → 停止登记 → 返回 `RunningTurn`（提前 resolve 契约） |
| `RunningTurn::drive` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `pub async fn drive<F>(self, on_output: F) -> TurnOutcome where F: FnMut(KernelOutput)` | 泵驱动：盖戳 → delta 只上输出回调、密封 write-through（失败 failed 收敛记因）→ 双 id 落库 → TurnDone 统计收口 → 终态除名 → 流出 `TurnFinished` |
| `StopRegistry::register` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `pub fn register(&self, session_id: &str, handle: RunHandle)` | 运行中会话停止句柄登记（内存态，键 = core session id） |
| `StopRegistry::request_stop` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `pub fn request_stop(&self, session_id: &str) -> bool` | 置位终止信号；miss 幂等返回 false（不报错不改终态） |
| `StopRegistry::remove` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `pub fn remove(&self, session_id: &str)` | 收敛除名（除名后 stop 对该键幂等忽略） |
| `SessionSink` | `packages/desktop/src-tauri/crates/core/agent/src/port.rs` | 新增 | `trait SessionSink: Send + Sync` | write-through port：`create_session(NewSessionRow) -> Result<(), String>`、`begin_turn(session_id: &str, started_at: i64) -> Result<i64, String>`、`append_sealed(session_id: &str, event: &AgentEvent) -> Result<(), String>`、`finish_turn(turn_id: i64, outcome: &TurnOutcome) -> Result<(), String>`、`bind_remote_session(session_id: &str, remote: &str, updated_at: i64) -> Result<(), String>`（方法签名均定义于本 trait） |
| `SessionQuery` | `packages/desktop/src-tauri/crates/core/agent/src/port.rs` | 新增 | `trait SessionQuery: Send + Sync` | 查询契约：`list_sessions(source: Option<&str>, source_ref: Option<&str>) -> Result<Vec<SessionSummary>, String>`、`transcript(session_id: &str) -> Result<Vec<AgentEvent>, String>`、`reconcile_stats(session_id: &str) -> Result<SessionStats, String>`（方法签名均定义于本 trait） |
| `AgentEventKind::is_delta` | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | 新增 | `pub fn is_delta(&self) -> bool` | 增量/密封判别（内核泵分类与 store sink 防御共用） |
| `AgentEventKind::is_sealed` | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | 新增 | `pub fn is_sealed(&self) -> bool` | 密封判别（durable 词汇面） |
| `compose_turn` | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | 新增 | `pub fn compose_turn(stores: &WorkspaceStores, root: &str, agent: Option<i64>) -> Result<ComposedTurn, String>` | 解析单点：缺省解析默认 agent / 显式 agent → (`EngineKind`, `EngineConfig`)（sdk 由引用 provider 组装取 high 档、无默认 `Err` 引导管理页）；Continue 读会话行校验快照 engine 并提取 prior_handle；装配 sink/query/门面/内核 |
| `Store::create_session` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn create_session(&self, session: &SessionRecord) -> Result<SessionRecord, StoreError>` | 会话行落库（写事务，id 来自 core 铸造） |
| `Store::find_session` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn find_session(&self, session_id: &str) -> Result<Option<SessionRecord>, StoreError>` | 主键直查（Continue 校验与装配消费） |
| `Store::begin_agent_turn` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn begin_agent_turn(&self, session_id: &str, started_at: i64) -> Result<AgentRunRecord, StoreError>` | 轮统计行 begin（写事务内 max+1 分配，`session_id` 挂 core 会话，running 初值） |
| `Store::append_session_events` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn append_session_events(&self, session_id: &str, events: &[AgentEvent]) -> Result<(), StoreError>` | 密封转录追加（单事务；delta 防御性忽略不产生记录） |
| `Store::finish_agent_turn` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn finish_agent_turn(&self, turn_id: i64, record: &AgentRunRecord) -> Result<(), StoreError>` | 轮行终态整行替换（status / finished_at / 统计 / error） |
| `Store::bind_session_remote` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn bind_session_remote(&self, session_id: &str, remote: Option<&str>, updated_at: i64) -> Result<(), StoreError>` | 双 id 映射落库半边 + `updated_at` 刷新 |
| `Store::list_sessions` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn list_sessions(&self, source: Option<&str>, source_ref: Option<&str>) -> Result<Vec<SessionSummary>, String>` | 会话清单（来源过滤、`updated_at` 降序稳定序）+ 聚合统计从轮统计行现算（轮数/累计墙钟/累计 token，缺席合法缺省）+ 轮统计行随行返回 |
| `Store::list_session_events` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn list_session_events(&self, session_id: &str) -> Result<Vec<AgentEvent>, String>` | 转录重放：二级索引扫描 + seq 升序（空洞容忍），不要求运行进程存活 |
| `Store::reconcile_session_stats` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn reconcile_session_stats(&self, session_id: &str) -> Result<SessionStats, String>` | 对账纠偏重导显式入口：从转录 `TurnDone` 事件重算聚合（不改密封转录、不隐式挂读路径） |
| `Store::delete_explore_record` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 修改 | `pub fn delete_explore_record(&self, root: &str, name: &str) -> Result<bool, StoreError>` | 级联圈定自「explore 来源 runs + 事件」平移为「`(source, source_ref)` 归属会话 + 转录 + 轮统计行」同事务删；miss 幂等不变 |
| `agent_start` | `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 修改 | `pub async fn agent_start(app: AppHandle, on_event: Channel<AgentRunMessage>, root: String, prompt: String, permission_mode: AgentPermissionMode, session_id: Option<String>, source: Option<String>, source_ref: Option<String>, agent: Option<i64>) -> Result<TurnSummary, String>` | 发起命令语义升级：`session_id=None` 即 New、Some 即 Continue；来源缺省 debug；提前 resolve 返回 running 态轮行（`TurnSummary`） |
| `agent_stop` | `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 修改 | `pub fn agent_stop(registry: State<'_, StopRegistry>, root: String, session_id: String) -> Result<(), String>` | 终止命令会话寻址（root 寻址保留）；幂等忽略不变 |
| `agent_sessions` | `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 新增 | `pub fn agent_sessions(stores: State<'_, WorkspaceStores>, root: String, source: Option<String>, source_ref: Option<String>) -> Result<Vec<SessionSummary>, String>` | 会话清单 + 聚合统计查询（root → workspace 库直查 DTO） |
| `agent_session_transcript` | `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 新增 | `pub fn agent_session_transcript(stores: State<'_, WorkspaceStores>, root: String, session_id: String) -> Result<Vec<AgentEvent>, String>` | 会话转录重放查询（全史密封事件 seq 序） |
| `eventsToUIMessages` | `packages/desktop/src/lib/agent-adapter.ts` | 修改 | `function eventsToUIMessages(events: AgentEvent[]): AgentUIMessage[]` | 重放折叠：仅消费密封事件（`messageDelta` 防御跳过）；`turnDone` → `data-run-result` |
| `eventToChunk` | `packages/desktop/src/lib/agent-adapter.ts` | 修改 | `function eventToChunk(event: AgentEvent): AgentUIMessageChunk[]` | 实时增量增 delta 路：`messageDelta` → `text-delta` / `reasoning-delta`（稳定 part id = `deltaPartId(键, 类别)`）；密封 Message chunk 组保持 start+reset+整块（同键让位替换） |
| `provisionalMessageId` | `packages/desktop/src/lib/agent-adapter.ts` | 新增 | `function provisionalMessageId(parentToolUseId: string \| null): string` | provisional 消息键（配对键派生，adapter 与 transport 共用锚点） |
| `runRecordToUIMessage` | `packages/desktop/src/lib/agent-adapter.ts` | 修改 | `function runRecordToUIMessage(record: TurnSummary): AgentUIMessage` | 终态同构部件组装：入参随 IPC 轮行 DTO 演进（`AgentRunRecord` → `TurnSummary`），部件形状 `data-run-record` 不变 |
| `deltaPartId` | `packages/desktop/src/lib/agent-adapter.ts` | 新增 | `function deltaPartId(key: string, part: 'text' \| 'thinking'): string` | delta 累积部件的稳定 part id（首 delta 开件、后续累积共用） |
| `useAgentChat` | `packages/desktop/src/hooks/use-agent-chat.ts` | 修改 | `function useAgentChat(params: UseAgentChatParams): UseAgentChatState` | 会话域状态基建（重放装载/发送/停止/归一见「修改文件」行）；对外新增 `session` 镜像 |
| `useExploreSession` | `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | 修改 | `function useExploreSession(root: string \| null, record: ExploreRecord \| null)` | 对外签名与返回形状不变；内部经基建会话寻址语义等价切换 |
| `useAgentRunHistory` | `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 修改 | `function useAgentRunHistory(root: string \| null): AgentRunHistoryState` | 会话清单 + 转录重放取数轨道 |
| `TauriAgentTransport.sendMessages` | `packages/desktop/src/lib/agent-transport.ts` | 修改 | `async sendMessages(options: { trigger: 'submit-message' \| 'regenerate-message'; chatId: string; messageId: string \| undefined; messages: AgentUIMessage[]; abortSignal: AbortSignal \| undefined; body?: object }): Promise<ReadableStream<AgentUIMessageChunk>>` | 方法签名不变；body 契约会话域化（`sessionId`）+ 流内首 delta 簿记 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `MessageDelta`（`AgentEventKind` 变体） | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | 新增 | `{ parent_tool_use_id: Option<String>, delta: AgentDelta }`——token 级增量，ephemeral，store 永不见 |
| `AgentDelta` | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | 新增 | serde 内部 tag 枚举：`Text { text: String }` \| `Thinking { thinking: String }`（思考/回复可辨） |
| `TurnDone`（`AgentEventKind` 变体，原 `RunResult`） | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | 修改 | 字段面不变（subtype / is_error / num_turns / duration_ms / cost_usd / usage / session_id）；统计唯一口径，线值 `runResult` → `turnDone` |
| `SessionOpen` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 | `{ injections: SessionInjections, ctx: SessionCtx, session: SessionRef, prior_handle: Option<String> }`——会话建立参数（注入/轮分离不变量的会话级半边） |
| `SessionInjections` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 | `{ preamble: Option<String>, tools: Option<Vec<String>> }`——会话级注入（tools 为切片③预留位，MVP 引擎忽略） |
| `SessionCtx` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 | `{ workspace_root: PathBuf, permission_mode: AgentPermissionMode }`——首版字段面；不加 `deny_unknown_fields`（未知字段忽略演进） |
| `SessionRef` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 | `New \| Continue { id: String }`——会话引用（协议寻址单位） |
| `TurnQuestion` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 | `{ prompt: String }`——轮级驱动词汇 |
| `AgentSession` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 | `{ observations: mpsc::Receiver<AgentEventKind>, questions: mpsc::Sender<TurnQuestion>, handle: RunHandle }`——观察回流（未盖戳）+ 轮驱动 + 句柄三件套 |
| `SessionProvenance` | `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | 新增 | `{ source: String, source_ref: Option<String> }`——来源归属（缺省 debug） |
| `SessionRow` | `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | 新增 | `{ id, remote_session_id: Option<String>, config_snapshot: serde_json::Value, provenance, created_at, updated_at }`——会话记录契约（快照不透明，core 不解释装配概念） |
| `NewSessionRow` | `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | 新增 | `SessionRow` 的建行入参形态（id 由内核铸造后传入） |
| `SessionStats` | `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | 新增 | `{ turn_count: u64, total_duration_ms: Option<u64>, input_tokens: Option<u64>, output_tokens: Option<u64> }`——聚合统计（MVP 现算，不维护累计列，缺席合法） |
| `SessionSummary` | `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | 新增 | `{ row: SessionRow, stats: SessionStats, turns: Vec<TurnSummary> }`——清单项（轮行随行返回，重放部件与历史展示共用；core 契约类型，不引用 store 模型） |
| `TurnSummary` | `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | 新增 | `{ turn_id: i64, session_id: String, status: AgentRunStatus, started_at: i64, finished_at: Option<i64>, num_turns: Option<u64>, cost_usd: Option<f64>, duration_ms: Option<u64>, error: Option<String> }`——IPC 轮行 DTO（取代 `AgentRunRecord` 的 IPC 面）：agent_start 提前 resolve 返回、`AgentRunMessage::Record` 终态部件、`data-run-record` 部件数据、实时与重放两路同构；`AgentRunRecord`（store 模型）退为持久化内部类型，出 IPC 镜像 |
| `TurnRequest` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `{ session, question, injections, ctx, provenance, config_snapshot }`——内核轮请求（快照由组合根组装、内核不解释） |
| `RunningTurn` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `{ session_id: String, turn_id: i64, started_at: i64 }` + `drive`——提前 resolve 产物 |
| `KernelOutput` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | `Observation(AgentEvent)`（盖戳后全量观察，传输面）\| `TurnFinished(TurnOutcome)`（终态部件，与实时同构） |
| `TurnOutcome` | `packages/desktop/src-tauri/crates/core/agent/src/port.rs` | 新增 | `{ turn_id, status: AgentRunStatus, finished_at, num_turns, duration_ms, cost_usd, usage, error, remote_session_id }`——轮收口中性数据（store 落库与 IPC 装配共用） |
| `StopRegistry` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 新增 | 内存态停止注册（Mutex 表，键 = core session id；进程生命周期） |
| `SessionRecord` | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 新增 | native_db 模型（id=7）：字符串主键（core 铸 id）/ `engine_session_id` / `config_snapshot` / `source` / `source_ref` / `created_at` / `updated_at` |
| `SessionConfigSnapshot` | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 新增 | store 本地嵌套 struct：`{ engine: AgentEngineKind, model: Option<String>, permission_mode: AgentPermissionMode }`（快照非引用，实例改名/删除不伤历史） |
| `SessionEventRecord` | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 新增 | native_db 模型（id=8，serde_json 编解码）：打包 u128 主键（`hash64(session_id) << 64 \| seq`）+ `session_id` 二级索引 + 密封 `AgentEvent` 嵌装（转录单表；delta 永不落库） |
| `AgentRunRecord` | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 修改 | native_model 版本 3→4 轮统计行化：`{ id, session_id: Option<String>, status, started_at, finished_at, num_turns, cost_usd, duration_ms, error }`；from_previous 存量行 `session_id = None`；退为 store 持久化内部类型（IPC 面由 core `TurnSummary` 承载，出 bindings 镜像） |
| `UseAgentChatParams` / `UseAgentChatState` | `packages/desktop/src/hooks/use-agent-chat.ts` | 修改 | 入参不变（source/sourceRef/root）；状态面增 `session: SessionSummary \| null`，`chain` 语义平移为轮统计行列表 |
| `AgentRunHistoryState` | `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 修改 | `runs: AgentRunRecord[]` → 会话清单（`SessionSummary[]`）；重放轨道增 `selectedSessionId` |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.3.14"` | 用户可见变更（碎行修复 + 续会话修复）版本提升；desktop 包不受插件 bump 规则约束 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `SessionRecord`（workspace 维度） | `id`（core 铸 `ses-` 前缀字符串主键）、`engine_session_id: Option<String>`（双 id 映射落库半边）、`config_snapshot`（engine/model/permission 快照）、`source` / `source_ref`（来源归属，缺省 debug）、`created_at` / `updated_at`（UTC unix 毫秒） | 1:N → 轮统计行（经轮行 `session_id` 外键）、1:N → 转录事件（经 `SessionEventRecord.session_id` 二级索引）；禁跨库引用（快照代引用） | 所属 workspace 独立 db 文件（native_db，native_model id=7；随记录信封 API 零改动可浏览） |
| `SessionEventRecord`（转录单表） | `event_key`（`hash64(session_id) << 64 \| seq` 打包主键，hex 串 serde）、`session_id`（非唯一二级索引）、`event: AgentEvent`（密封 only 嵌装，含 `Raw` 逃生舱） | N:1 → `SessionRecord`；seq 升序即会话全史重放序（delta 占 seq 产生库内空洞，合法） | 同 workspace 库（native_model id=8，serde_json 编解码承载 serde flatten） |
| `AgentRunRecord`（轮统计行，v4，store 内部持久化模型） | `id`（库域自增主键）、`session_id: Option<String>`（新行恒 Some = core 会话 id；存量升级行为 None）、`status`（running/completed/failed/stopped）、`started_at` / `finished_at`、`num_turns` / `cost_usd` / `duration_ms`（TurnDone 字段面）、`error` | N:1 → `SessionRecord`（同 session 轮序列即链，`started_at` 有序）；链指针 `parent_run_id` 语义由会话归属取代；IPC 面由 core `TurnSummary` 承载（port 映射出） | 同 workspace 库（native_model id=2 版本 4 原地演进；存量行自动升级为孤儿轮行，无迁移层） |
| `StopRegistry`（内存态） | `session_id → RunHandle` 映射（Mutex 表） | 1:1 对应运行中会话活动轮 | 不持久化（进程生命周期，应用重启即清空） |
| 退役形态 | 旧 `AgentEventRecord`（id=3）定义移出注册，旧行惰性废弃不可读；v3 `AgentRunRecord` 行经版本机制升级保留 | — | 死数据留盘，零迁移代码路径 |

## 路由/API 设计

<!-- Tauri invoke 命令面（无 HTTP API）：Result<T, String> 模板、root 寻址与 blank 守卫保留。 -->

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| invoke | `agent_start` | 发起会话轮（New/Continue），提前 resolve running 轮行，事件与终态经 Channel 流出 | `on_event: Channel<AgentRunMessage>`、`root`、`prompt`、`permission_mode`、`session_id: Option<String>`、`source: Option<String>`、`source_ref: Option<String>`、`agent: Option<i64>` | `TurnSummary`（running）；错误 `Err(String)`（无默认 agent / CLI 不可发现 / 配置缺失 / 会话缺失 / 库打开失败） | 本机单用户（无认证） |
| invoke | `agent_stop` | 终止运行中会话活动轮，收敛 `stopped` 并经 Channel 流出终止终态部件 | `root`、`session_id` | `null`（幂等忽略不报错） | 同上 |
| invoke | `agent_sessions` | 会话清单 + 聚合统计（来源过滤，updated_at 降序） | `root`、`source: Option<String>`、`source_ref: Option<String>` | `SessionSummary[]` | 同上 |
| invoke | `agent_session_transcript` | 会话全史转录重放（密封事件 seq 序，不要求运行进程存活） | `root`、`session_id` | `AgentEvent[]` | 同上 |
| 退役 | `agent_runs` / `agent_run_events` / `agent_run_chain` | run 域查询三命令由会话域两查询取代 | — | — | — |

## 依赖

### 运行时依赖

- `rig-core` — SDK 引擎 provider client 与消息/工具类型层（恒编译、`default-features = false` + openai feature，既有足迹不回退）
- `tokio` — 内核与引擎的进程内存原语（有界 mpsc / Notify / 泵任务；core/agent 既有依赖延续，「无直接 IO」禁令不含内存原语）
- `native_db` / `native_model` — store 双库持久化（`SessionRecord` / `SessionEventRecord` 新模型、`AgentRunRecord` 版本演进；既有）
- `serde` / `serde_json` / `specta` — 信封线格式、DTO 镜像与 bindings 再生成（既有）
- `store`（workspace 内，**新增边**）— `agent-runtime` 组合根与 port 适配的持久面落点（infra→infra；core/agent 保持零 workspace 依赖）

### 构建/测试依赖

- 无新增。`sha2`（store 既有）复用于 `SessionEventRecord` 键打包的 hash64 派生。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `sdk/normalize.rs` 双层化（Text/ReasoningDelta → `MessageDelta`，text/thinking 可辨）+ `sdk/loop.rs` 轮末 `choice` 收口恰一条密封 Message；前端 provisional 路（adapter delta chunks 稳定 part id 累积 + transport 首 delta 簿记开件 + 密封 start+reset 同键让位）→ 调试页一轮回复单条连续增长消息、无碎行 |
| AC-2 | `SessionEventRecord` 转录单表（session 二级索引 + seq 序）+ 内核泵只对密封事件调 `append_sealed`（store sink 对 delta 防御性忽略）+ 每轮 assistant 产出恰一条密封 Message（loop 收口）→ store 中不存在任何 delta 记录 |
| AC-3 | `SessionRef::Continue{id}` 经 `Store::find_session` 取会话行 + `list_session_events` 全史转录 → `sdk/resume.rs` 全史重建（顶层非 Raw、tool 成对），第 N 轮重建史含全部前 N-1 轮；会话不存在 / 非同引擎产出 / remote 句柄缺失 → `ConfigMissing` 显式启动失败不落库 |
| AC-4 | core/agent 无直接 IO（`port.rs` 契约 + `store_port.rs` infra 实现）；`lib.rs` 导出面无 claude/引擎/Tauri 字样；`crates/core/agent/Cargo.toml` 零 workspace 内依赖（desktop-crate-layout 无需修订）；进程 spawn 仅存于 `cli/runner.rs` |
| AC-5 | P1 四不变量：观察词汇含 `MessageDelta` 且有界 mpsc（背压阻塞 send）；`TurnDone` 字段面唯一统计口径（内核收口写轮行，引擎无第二通道）；engine 侧 id 经 `RunStarted`/`TurnDone` session_id 上报 + `bind_session_remote` 双 id 映射落库 + `SessionRow.remote_session_id` 内存半边经 `SessionOpen.prior_handle` 回供；`SessionInjections`（会话级）与 `TurnQuestion`/`SessionCtx`（轮级）分离 |
| AC-6 | 内核泵密封到达即 `append_sealed` 原子落盘（write-through 单点）、轮收口即 `finish_turn`；`reconcile_session_stats` 对账重导显式入口（从转录 TurnDone 重算，不动转录）；统计与查询字段缺席合法缺省（cost 恒 None、usage 空值、token 缺席不计）降级不违约 |
| AC-7 | `StopRegistry`（内核治理面）+ `agent_stop(root, session_id)` 会话寻址：置位 `RunHandle`（竞态语义零改动）→ 引擎终止（CLI 树杀缝 / SDK drop future）→ 内核显式 `stopped` 收敛 + 终态部件流出；miss 幂等忽略；停止后重放 = 转录（含 stopped 轮行）与实时同构 |
| AC-8 | adapter `messageDelta` → provisional 增长、密封 start+reset+整块同键替换；hook 重放（会话转录折叠 + 轮统计行部件交错）与实时收口归一同形状；未知事件 `data-raw` 原文透传不丢 |
| AC-9 | 编排（open/建行/tee/落库/盖戳/收敛/除名）下沉 `kernel.rs`；命令体三件事（blank root 守卫与参数转换 → 组合根 + 内核调用 → `Err(String)` 映射）；组合根无默认 agent 显式 `Err` 引导管理页；`Result<T, String>` 模板与 root 寻址保留 |
| AC-10 | CLI 引擎 flag 组装（`flags.rs` 输出序列零变化）、jsonl 行解析（仅变体更名）、`--resume` 语义（prior_handle 平移）行为面零回归；回归护栏由测试阶段承接（design 只钉语义零变化） |
| AC-11 | `use-explore-session` 经基建 source/sourceRef 会话寻址 + Continue + 级联删除随 `delete_explore_record` 会话级联（转录 + 轮统计行同事务）——对外行为语义等价不回退 |

---

## 待决问题

- ai v7 reducer 密封替换的 chunk 级行为已按 `ai@7.0.118` 源码钉死（`start` 重命名 + `reset-step` 让位），实现期以适配层测试复核；若实测与推导不符，按提案风险表预案降级为「密封收口时 `setMessages` 归一兜底」（主机制与兜底都以适配层纯函数锁定）。
- `SessionStats` 的 token 聚合口径：对 `TurnDone.usage` 以数值键 `inputTokens` / `outputTokens`（serde camelCase 线值）鸭子类型求和，其余口径缺席——真实 usage 形状（rig `Usage` 序列化与 CLI 合成 usage）首次落库后复核键集是否需扩展。
- delta 路下 provisional 存续期内出现非密封事件组的让位行为：现状流序不产生此交错（SDK 轮内无 Notice、CLI 本期无增量），作为已知限制留痕，不为本期扩传输簿记。
- 跨引擎续会话（如 CLI 产出会话以 sdk 引擎 Continue）本期一律 `ConfigMissing` 显式拒绝；将来是否放开（转录重建跨引擎续写）留待产品裁定。
- 调试页会话清单的增长（转录无上限、debug 会话无清理）——workspace-store delta 已留痕 MVP 不做留存清理；清单分页/清理触发点延后。
- 旧事件行（惰性废弃死数据）的磁盘回收不做；如未来需要，经 db 维护工具另行处理，不在 store 纪律内开口子。
