# 测试设计: agent-core-session-kernel

> **日期**: 2026-10-01

---

## 验收范围

<!-- AC-4 为静态结构约束（无直接 IO / 源码字样 / 依赖方向），非进程内可断言行为，落不可测试项 1。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 一轮含文本+思考的 SDK 回复：实时流呈 token 级增量（text/thinking 可辨），轮末恰一条密封 Message；调试页一轮回复为单条连续增长的消息，无碎行 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize_test.rs |
| AC-2 | 密封事件记录以 session 为键直挂会话；每轮 assistant 产出恰一条密封 Message；store 中不存在任何 delta 记录 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-3 | 第 N 轮续会话的重建史含全部前 N-1 轮 user/assistant 往返（第一轮不再丢失）；会话不存在或非 SDK 产出时显式启动失败 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs |
| AC-4 | core/agent 无对外部世界的直接 IO（文件/进程/网络），IO 全经 port 由 infra 实现；源码无 claude/引擎字样、无 Tauri 依赖；（port 落点裁定前）无 workspace 内 crate 依赖 | —（见不可测试项 1） |
| AC-5 | 观察词汇含 delta 且通道有界背压；TurnDone 字段面为唯一统计口径且由 core 累计（引擎无第二口径）；engine_handle 经 spawn 响应或事件上报且 core 记双 id 映射；injections（会话级）与 question/ctx（轮级）分离 | packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs |
| AC-6 | 密封事件/返回值到达即 store 原子操作落盘；查询对账重导可校正统计/索引；统计与查询附带字段缺席不报错（降级不违约） | packages/desktop/src-tauri/crates/infra/agent/src/store_port_test.rs |
| AC-7 | 会话活动轮可终止并收敛 `stopped`；对非运行中目标幂等忽略；停止后重放与实时一致；`RunHandle` 已验证竞态语义无回退 | packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs |
| AC-8 | delta → provisional 消息、密封到达替换；重放（会话转录折叠）与实时收口归一后的状态同形状；未知事件仍 `data-raw` 透传不丢 | packages/desktop/src/lib/agent-adapter.test.ts |
| AC-9 | 编排（tee/落库/收敛/盖戳）自 commands 下沉内核；命令体保持参数转换 → 调用 → 错误映射三件事；无默认 agent 时显式 `Err` 引导管理页；root 寻址与 `Result<T, String>` 模板保留 | packages/desktop/src-tauri/src/commands/exec/mod_test.rs |
| AC-10 | CLI 引擎 flag 组装 / jsonl 解析 / `--resume` 行为不变，既有 CLI 测试全绿 | packages/desktop/src-tauri/crates/infra/agent/src/cli/flags_test.rs |
| AC-11 | explore 会话链续话、历史还原、记录级联删除经会话域语义等价工作（行为不回退） | packages/desktop/src/views/explores/hooks/use-explore-session.test.ts |

---

## 单元测试

<!-- Rust 侧套件为 src-tauri 工作区根注册的 cargo 测试（`*_test.rs` 共置模块，#[test] / #[tokio::test]），前端为 packages/desktop 的 vite-plus 测试运行器（`*.test.ts(x)`）。
   跨模块组合用例挂靠链路入口模块：内核链路（命令面 → 组合根 → 内核 → port）挂 compose_test / kernel_test / mod_test；
   前端链路（transport → adapter → useChat）挂 use-agent-chat.test / agent-debug-view.test。不设独立集成测试章节。 -->

### packages/desktop/src-tauri/crates/core/agent/src/event.rs -> packages/desktop/src-tauri/crates/core/agent/src/event_test.rs

#### 待测功能

- AgentEventKind.is_delta(): 增量/密封判别（内核泵分类与 store sink 防御共用）
- AgentEventKind.is_sealed(): 密封判别（durable 词汇面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| messageDelta 增量变体 | 正向 | MessageDelta（parent_tool_use_id=None、delta=Text）构造并盖戳：is_delta() 为 true，serde 线值判别逐字为 messageDelta，parentToolUseId/delta 驼峰键在场，往返逐字段相等 | 新增 |
| messageDelta 增量变体 | 边界 | parent_tool_use_id=Some("tu_1") 与 delta=Thinking 两形态：判别与 serde 往返保真（子代理归因词汇同源、思考/回复可辨） | 新增 |
| 判别函数全变体矩阵 | 正向 | 六变体逐一断言：仅 MessageDelta is_delta()=true；Message/SystemNotice/RunStarted/TurnDone/Raw 五变体 is_sealed()=true 且 is_delta()=false（互补不重叠） | 新增 |
| turnDone 更名线格式 | 正向 | TurnDone 构造（字段面不变）：序列化判别值逐字为 turnDone（runResult 线值退役），isError/numTurns/durationMs/costUsd/sessionId 驼峰键回归 | 新增 |
| turnDone 更名线格式 | 异常 | 旧线值 "runResult" 反序列化返回 Err（更名不别名，无兼容读负担） | 新增 |
| turnDone 更名线格式 | 边界 | TurnDone 全字段缺席形态（usage null、cost/numTurns/durationMs/sessionId null）serde 往返无损 | 新增 |
| AgentDelta 内部 tag 枚举 | 正向 | Text/Thinking 两变体序列化判别值逐字为 text/thinking，载荷驼峰键，往返一致 | 新增 |
| 盖戳原语 stamp 回归 | 边界 | stamp 对 u64::MAX seq 与当前时钟毫秒边界：seq/时间戳盖戳行为零回归（时间戳落在调用区间） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 全部 fixture 以 AgentEventKind/serde_json 内存构造；stamp 内部时钟以调用区间断言，不注入时钟 | 全部 describe |

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

- AgentRunner.open_session(): P1 协议唯一入口（取代 start）：会话建立（injections + ctx + session 引用 + prior_handle），启动阶段失败不产生任何记录

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| open_session New 会话建立 | 正向 | 假 runner 经 SessionRef::New + SessionCtx + SessionInjections 打开会话：返回 AgentSession 三件套（observations/questions/handle），injections.preamble/tools 与 ctx.workspace_root/permission_mode 原样到达实现（捕获缝断言）——injections（会话级）与轮级分离不变量的会话半边 | 新增 |
| open_session Continue 引用 | 正向 | SessionRef::Continue 携 prior_handle：假 runner 捕获 id 与 prior_handle 原样传递（双 id 映射的回供半边） | 新增 |
| open_session 启动失败 | 异常 | 假 runner 分别返回 ConfigMissing/CliMissing/SpawnFailed：Err 原样传播且三变体互不重合（变体集零改动回归），无任何记录产物 | 新增 |
| open_session trait object 形态 | 边界 | Box<dyn AgentRunner> 可打开会话且满足 Send+Sync；重复 open_session 互不共享运行态 | 新增 |
| 协议类型 serde 线格式 | 边界 | SessionRef 内部 tag 线格式（new/continue）、SessionCtx 驼峰键且未知字段忽略（不加 deny_unknown_fields：多余 JSON 键反序列化成功、additive 演进）、SessionInjections tools None 形态——经 open_session 入参类型的 serde 面锁定 | 新增 |
| RunHandle 竞态语义回归 | 边界 | 置位/同步观测/Clone 共享/wait_requested 短路与唤醒：零改动承诺回归锁定 | 新增 |
| AgentStartError 三变体 | 边界 | CliMissing/SpawnFailed/ConfigMissing Display 文案携带原因串（变体集 MUST NOT 再增的回归锚） | 新增 |
| start 旧协议入口 | 废弃 | AgentRunner::start / AgentRunParams / AgentRun / AgentEnvMode 随协议退役：既有 start 事件流用例与 AgentEnvMode 线格式用例、AgentRunParams cwd 保真用例删除（源内残留引用即编译失败） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| AgentRunner 假实现（注入依赖，入参例外） | 预录观察经 mpsc 回流 + Mutex 捕获 SessionOpen 入参 + 可编程 open Err；无文件/网络 | 全部 describe |

### packages/desktop/src-tauri/crates/core/agent/src/state.rs -> packages/desktop/src-tauri/crates/core/agent/src/state_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的模块级导出变更；本节用例锚定「修改文件」表行：RunStateMachine::apply 匹配变体 RunResult → TurnDone 纯更名适配，收敛语义（首收敛生效、stopped 独立分支、终态幂等）零变化。 -->

（design.md 未声明该文件的公共 API 变更——apply 更名适配的回归锁定见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| TurnDone 驱动收敛 | 正向 | TurnDone（is_error=false）收敛 completed、（is_error=true）收敛 failed——更名后 apply 匹配变体回归 | 新增 |
| 非收敛事件不改变状态 | 边界 | RunStarted/Message/MessageDelta/SystemNotice/Raw 逐枚 apply 保持 running（新增 delta 词汇不驱动收敛） | 新增 |
| stopped 独立分支 | 正向 | running 调 stop 收敛 stopped；stopped 后 TurnDone 不改写终态 | 新增 |
| 终态幂等与首收敛生效 | 边界 | 收敛后任意事件终态不改写；连续两个 TurnDone 首个生效；stop 与 TurnDone 双向竞态首个收敛生效 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 事件以 AgentEvent::stamp 内存构造，纯状态机真实组合 | 全部 describe |

### packages/desktop/src-tauri/crates/core/agent/src/lib.rs -> packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的模块级导出条目；本节锚定「修改文件」表行：模块声明与导出面扩展（session / port / kernel）、退役类型出导出面（AgentRunParams / AgentRun / AgentEnvMode）、模块文档纯度措辞更新。 -->

（design.md 未声明该文件的公共 API 变更——导出面以编译期锚定用例承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| crate 根导出面锚定 | 正向 | session/port/kernel 新模块类型经 crate 根 use 可达（SessionRow/SessionSink/SessionKernel 等以用例内使用锚定，任一 re-export 破坏即编译失败） | 新增 |
| 退役类型出导出面 | 边界 | AgentRunParams/AgentRun/AgentEnvMode 退役后 crate 根不再导出：全 crate 无残留引用由编译承载，用例内不出现旧类型标识 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 编译期锚定用例，仅 use 导入与构造形态断言 | 全部 describe |

### packages/desktop/src-tauri/crates/core/agent/src/session.rs -> packages/desktop/src-tauri/crates/core/agent/src/session_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的模块级函数；条目取自「新增文件」表（ses- 前缀会话 id 铸造）与「类型定义」表（会话域契约类型，serde camelCase + specta）。 -->

- new_session_id(): ses- 前缀会话 id 铸造（core 铸 id，进程内唯一）
- SessionProvenance/SessionRow/NewSessionRow: 会话记录契约类型（serde camelCase + specta）
- SessionStats/SessionSummary: 聚合统计与清单项契约（缺席合法缺省）
- TurnSummary: IPC 轮行 DTO（running/终态两形态）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 会话 id 铸造 | 正向 | 连续铸造多枚 id：ses- 前缀逐字命中且两两互异（core 铸 id 进程内全局唯一） | 新增 |
| 会话 id 铸造 | 边界 | 千次铸造零碰撞；id 形态稳定（前缀+唯一后缀，无空白与非法字符） | 新增 |
| SessionRow serde 往返 | 正向 | 全字段（id/remoteSessionId/configSnapshot/provenance/createdAt/updatedAt）camelCase 线格式往返逐字段相等 | 新增 |
| SessionRow serde 往返 | 边界 | remoteSessionId null 与有值两形态可区分；configSnapshot 不透明 Value 任意 JSON 保真；超长 sourceRef（大于 1000 字符）原样往返 | 新增 |
| SessionRow serde 往返 | 异常 | 缺必填字段（id/provenance）反序列化 Err，不产半成品 | 新增 |
| SessionStats 缺席缺省 | 边界 | 全 None 形态（turnCount 0、duration/tokens null）与全有值形态均往返无损（降级不违约的契约面） | 新增 |
| TurnSummary 两形态 | 正向 | running 形态（finishedAt/numTurns/costUsd/durationMs/error 全 null）与终态形态（统计齐备 + error 记因）serde 往返；status 四值线格式 running/completed/failed/stopped | 新增 |
| SessionSummary 组装 | 边界 | row+stats+turns 组装往返；turns 空数组与多轮行数组两形态 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | serde_json/specta 内存构造，时间戳以显式固定值入参 | 全部 describe |

### packages/desktop/src-tauri/crates/core/agent/src/port.rs -> packages/desktop/src-tauri/crates/core/agent/src/port_test.rs

#### 待测功能

- SessionSink: write-through port 契约——create_session/begin_turn/append_sealed/finish_turn/bind_remote_session 五方法
- SessionQuery: 查询契约——list_sessions/transcript/reconcile_stats 三方法

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| SessionSink trait 面 | 正向 | 假 sink 实现五方法并经 Arc<dyn SessionSink> 注入：object safety 与方法签名编译期锚定，调用序与载荷原样记录 | 新增 |
| SessionQuery trait 面 | 正向 | 假 query 实现三方法并经 Arc<dyn SessionQuery> 注入：Vec<SessionSummary>/Vec<AgentEvent>/SessionStats 返回形态编译锚定 | 新增 |
| TurnOutcome 共享收口类型 | 正向 | 全字段（turnId/status/finishedAt/numTurns/durationMs/costUsd/usage/error/remoteSessionId）serde camelCase 往返逐字段相等 | 新增 |
| TurnOutcome 缺席形态 | 边界 | usage/cost/remote/error 全 null 与 error 记因形态两极往返；status 四值线格式 | 新增 |
| 契约错误语义 | 异常 | 假 sink 返回 Err(String)：Result 语义原样传播（内核泵 failed 收敛记因的消费前提） | 新增 |
| Send+Sync 边界 | 边界 | dyn SessionSink / dyn SessionQuery 满足 Send+Sync（内核泵跨 await 持有的前提） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| SessionSink/SessionQuery 内存假实现（注入依赖，入参例外） | Vec 记录调用序与载荷、可编程 Err；trait 本身即注入面，真实组合在 store_port_test | 全部 describe |

### packages/desktop/src-tauri/crates/core/agent/src/kernel.rs -> packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs

#### 待测功能

- SessionKernel.new(): 内核构造（持久面 port + 停止注册，组合根装配）
- SessionKernel.begin_turn(): 会话建立与轮注册同步段（open 失败不落库 → New 建会话行 → 开轮行 → 停止登记 → RunningTurn 提前 resolve）
- RunningTurn.drive(): 泵驱动（盖戳 → delta 只上输出回调、密封 write-through → 双 id 落库 → TurnDone 统计收口 → 终态除名 → TurnFinished）
- StopRegistry.register(): 运行中会话停止句柄登记（内存态，键 = core session id）
- StopRegistry.request_stop(): 置位终止信号；miss 幂等返回 false
- StopRegistry.remove(): 收敛除名（除名后 stop 对该键幂等忽略）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| StopRegistry 登记与置位 | 正向 | register(session, handle) 后 request_stop 命中返回 true 且 RunHandle.stop_requested 同步观测为 true（AC-7 停止链置位半边） | 新增 |
| StopRegistry miss 幂等 | 异常 | 未登记 session_id 调 request_stop 返回 false 不报错不改终态；remove 后再 stop 幂等忽略（AC-7 幂等半边） | 新增 |
| StopRegistry 键隔离 | 边界 | 多 session 并行登记互不串扰；同键重复 register 以新 handle 覆盖 | 新增 |
| begin_turn New 会话同步段 | 正向 | 假 runner open 成功 + 假 sink：sink 调用序恰为 create_session → begin_turn（返回 turn_id）→ register；RunningTurn 的 session_id/turn_id/started_at 逐字段承接（提前 resolve 契约——AC-9 编排下沉半边） | 新增 |
| begin_turn open 失败不落库 | 异常 | 假 runner open 返回 ConfigMissing：Err 传播且假 sink 零调用（启动失败不产生任何记录） | 新增 |
| begin_turn sink 失败 | 异常 | create_session / begin_turn 返回 Err：Err 映射传播（错误映射单点在内核） | 新增 |
| begin_turn Continue | 边界 | SessionRef::Continue 不重复建会话行（create_session 不调用）而轮行照开；同 session 串联多轮 begin_turn 轮 id 递增承接 sink 分配序 | 新增 |
| drive delta 只上传输面 | 正向 | 假 runner 回流未盖戳 MessageDelta：on_output 收到盖戳后 KernelOutput::Observation（seq/时间戳由内核补齐、单调无跳号），假 sink append_sealed 零调用（store 永不见 delta——AC-2 内核半边、AC-5 盖戳治理单点） | 新增 |
| drive 密封 write-through | 正向 | 回流密封 Message：append_sealed 恰一次且载荷为盖戳后事件；多密封事件按序逐条落盘（AC-6 write-through 单点） | 新增 |
| drive 双 id 映射落库 | 正向 | RunStarted/TurnDone 上报 remote session id：bind_remote_session 携 remote 与 updated_at 调用（AC-5 双 id 映射半边） | 新增 |
| drive TurnDone 统计收口 | 正向 | TurnDone（numTurns/durationMs/usage）到达：TurnOutcome 统计字段唯一口径承接、finish_turn 调用、终态除名（remove 被调）、KernelOutput::TurnFinished 流出且与实时同构（AC-5 统计口径半边） | 新增 |
| drive append 失败收敛 | 异常 | 假 sink append_sealed 返回 Err：TurnOutcome.status=Failed 且 error 记因、finish_turn 尽力调用、除名后流出 TurnFinished（落库失败不可静默） | 新增 |
| drive 停止收敛 stopped | 异常 | StopRegistry 置位后假 runner 观察流关闭：TurnOutcome.status=Stopped、error 不记因（用户主动终止非失败）、TurnFinished 流出（AC-7） | 新增 |
| drive 终态幂等 | 边界 | TurnDone 后不再有观察产出与二次 sink 写入；空观察流（引擎立即 EOF）按停止信号/兜底 failed 语义收敛不悬挂 | 新增 |
| drive 有界通道背压 | 边界 | 观察洪峰（大于通道容量）：背压阻塞 send 全量送达零丢失、seq 全程单调（delta 占 seq 共享单调空间——AC-5 有界背压半边） | 新增 |
| drive on_output 与落库解耦 | 边界 | on_output 回调慢速消费：密封 write-through 与收口时序不受回调阻塞影响（tee 双路独立） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| SessionSink/SessionQuery 内存假实现（注入依赖，入参例外） | Arc+Mutex Vec 记录 create/begin/append/finish/bind 调用序与载荷、可编程 Err | begin_turn/drive 全部 describe |
| AgentRunner 假实现（注入依赖，入参例外） | 预录未盖戳 AgentEventKind 经 mpsc observations 回流（可编程 open Err、流关闭时序）；trait object 注入 | begin_turn/drive |
| RunHandle 真实实现 | tokio 原语（Notify/AtomicBool）真实参与，置位/观测不 mock | StopRegistry 与 drive 停止 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件新增条目（门面 runner_for / EngineConfig 零 diff）；本节锚定「修改文件」表行：挂载 compose / store_port 模块、ResumeTranscript 装载缝语义平移为会话全史转录（类型形状不变）、门面唯一 match 点保持。 -->

（design.md 未声明该文件的公共 API 变更——门面零 diff 回归与导出面锚定见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| crate 根导出面锚定 | 正向 | compose/store_port 模块经 crate 根可达（compose_turn/ComposedTurn 以用例内使用锚定）；既有门面 re-export（EngineKind/EngineConfig/ClaudeCliRunner/flags/jsonl/discover）零 diff 回归 | 新增 |
| ResumeTranscript 缝语义平移 | 边界 | 装载缝类型形状不变（入参 str、返回 Result<Option<Vec<AgentEvent>>, String>）：闭包替身注入后 CLI 分发路径持有不消费（调用计数恒零回归），语义注释平移为会话全史转录 | 新增 |
| 门面双臂分发回归 | 正向 | runner_for(Cli) 隔离 PATH 下 CliMissing、runner_for(Sdk) 空配置 ConfigMissing：签名与 EngineConfig 三字段零改动承诺回归 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| PATH 环境变量（进程全局变量边界） | 空 PATH 目录替换 + 共享互斥锁串行化（沿 cli/discover_test 先例），测毕恢复 | 门面双臂分发回归 |
| ResumeTranscript 闭包替身（注入依赖，入参例外） | 内存三形态闭包 + AtomicUsize 调用计数 | 缝语义平移用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

- compose_turn(): 解析单点——缺省/显式 agent 解析（sdk 由引用 provider 组装取 high 档、无默认 Err 引导管理页）+ Continue 快照校验与 prior_handle 提取 + 装配 sink/query/门面/内核

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 缺省解析默认 agent | 正向 | 真库装置登记默认 sdk 实例 + provider：compose_turn 产出 EngineKind::Sdk 且 EngineConfig 三字段 = provider api_key/base_url/models.high（agent=None 缺省解析语义原样平移——AC-9 解析单点下沉） | 新增 |
| 显式 agent 解析 | 正向 | agent=Some(id)：sdk 臂 provider 组装、cli 臂 EngineConfig::empty()（cli 实例不消费配置） | 新增 |
| 无默认 agent Err | 异常 | 空库缺省解析：Err 文案含管理页引导语义（显式 Err 不静默，AC-9） | 新增 |
| agent 不存在 Err | 异常 | agent=Some(404)：Err 携 id 记因 | 新增 |
| sdk provider 缺失 Err | 异常 | 实例缺 provider_id / provider_id 悬空：Err 记因区分两成因（引导管理页修正） | 新增 |
| Continue 快照校验通过 | 正向 | 真库会话行快照 engine 与解析 kind 一致且 engine_session_id 在场：校验通过且 prior_handle = engine_session_id 提取（双 id 回供） | 新增 |
| Continue 会话不存在 | 异常 | find_session miss：AgentStartError::ConfigMissing（会话缺失成因）显式启动失败（AC-3 显式半边） | 新增 |
| Continue 跨引擎拒绝 | 异常 | 快照 engine 与解析 kind 不一致（如 cli 产出会话以 sdk Continue）：ConfigMissing 显式拒绝（跨引擎续会话本期一律拒绝） | 新增 |
| Continue remote 缺失 | 异常 | engine_session_id 为 None：ConfigMissing（remote 句柄缺失成因），三成因消息互不重合 | 新增 |
| 装配产物组合 | 边界 | 产物组合完整（sink/query/门面/内核真实装配）：begin_turn/drive 可直接驱动（与 kernel_test 装置互锚）；provenance source 缺省 debug 承接 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 store mock（DB 进程边界以真实库组合） | tempdir 真库 WorkspaceStores 装置（沿 store_test 先例）；agent/provider/实例/会话行经真库 API 落库为 fixture | 全部 describe |
| sdk 解析不触网络 | 解析仅组装 EngineConfig（api_key/base_url/model），不构造 provider client、无 socket 参与 | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs -> packages/desktop/src-tauri/crates/infra/agent/src/store_port_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目（trait impl 落点文件）；本节锚定「新增文件」表行与 port 契约：SessionSink / SessionQuery 的 store 实现（write-through 原子操作、聚合现算、对账重导、delta 防御性忽略）。 -->

- SessionSink（store 实现）: create_session/begin_turn/append_sealed/finish_turn/bind_remote_session 的 write-through 原子操作
- SessionQuery（store 实现）: list_sessions/transcript/reconcile_stats 的查询与对账

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| SessionSink create/bind 往返 | 正向 | create_session 落库后 bind_remote_session：直查 remote_session_id 与 updated_at 刷新承接（双 id 映射落库半边——AC-5） | 新增 |
| SessionSink 轮生命周期 | 正向 | begin_turn 分配轮 id → finish_turn 终态整行替换（status/finished_at/统计/error），轮行挂 core 会话 | 新增 |
| SessionSink append_sealed 密封追加 | 正向 | 密封事件逐条追加：transcript 按 seq 升序全量重放（write-through 原子落盘——AC-6） | 新增 |
| SessionSink delta 防御性忽略 | 异常 | append_sealed 批次混入 MessageDelta：返回 Ok 且零记录产生（store 中不存在 delta 记录的防御半边——AC-2） | 新增 |
| SessionSink Err 传播 | 异常 | 会话行不存在的 append/begin：Err 传播（内核 failed 收敛记因的消费前提） | 新增 |
| SessionQuery transcript 重放 | 正向 | 跨多轮密封转录：seq 升序全史返回（不要求运行进程存活）；delta 占 seq 造成的密封 seq 空洞容忍不破坏有序（AC-2 查询半边） | 新增 |
| SessionQuery transcript 边界 | 边界 | 不存在会话返回空 Vec；跨 session 隔离（A 会话事件不出现在 B 转录） | 新增 |
| SessionQuery list_sessions 聚合现算 | 正向 | 多会话多轮行：轮数/累计墙钟/累计 token 现算正确、updated_at 降序稳定、source/source_ref 过滤生效、轮统计行随行返回 | 新增 |
| SessionQuery list_sessions 缺席缺省 | 边界 | 无轮行会话统计缺省（降级不违约——AC-6）；空库空清单 | 新增 |
| SessionQuery reconcile_stats 重导 | 正向 | 轮统计行被篡改后重导：从转录 TurnDone 重算聚合校正，密封转录逐字节不变（不改转录纪律——AC-6 对账半边） | 新增 |
| SessionQuery reconcile_stats 边界 | 边界 | 转录无 TurnDone：缺省统计返回不报错 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 store mock（DB 进程边界以真实库组合） | tempdir 真库 Store/WorkspaceStores 装置；时间戳以显式 i64 实参注入（updated_at/started_at 不等真实时钟） | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目；本节锚定「修改文件」表行：双层化——Text / ReasoningDelta / 完整 Reasoning → MessageDelta（text/thinking 可辨），完整 ToolCall 不再立即出密封事件（轮末收口），Unknown → Raw，Final / ToolCallDelta 簿记项照旧 None。 -->

- stream_item(): rig 流项 → 双层产出归一化纯函数（AC-1 传输半边核心）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| Text 流项双层化 | 正向 | Text 流项归一化为 MessageDelta（delta=Text）：文本逐字保真（含中文/emoji），与密封 Message 变体判别分明（碎事件根因修复的传输半边——AC-1） | 新增 |
| ReasoningDelta 流项双层化 | 正向 | ReasoningDelta 增量片段归一化为 MessageDelta（delta=Thinking）：思考增量可辨（原「等完整块」语义改为逐片段出 delta） | 新增 |
| 完整 Reasoning 流项 | 边界 | 完整 Reasoning 聚合块同样归一化为 Thinking delta：双层词汇无第二密封通道 | 新增 |
| 完整 ToolCall 轮末收口 | 正向 | 完整 ToolCall 不再立即出密封 Message 事件：产物为轮末收口聚合（由 loop 轮末统一收进恰一条密封 Message） | 新增 |
| Unknown 流项 Raw 透传 | 异常 | 未知流项 event_type 恒 sdk_stream、raw_json 载荷原样透传不丢不炸解析 | 新增 |
| 簿记项 None | 边界 | Final / ToolCallDelta 簿记项返回 None 不出事件（终端记录由 loop 直接消费，双发即重复） | 新增 |
| 纯函数无副作用 | 边界 | 同一流项重复归一化结果一致；空文本 delta 形态合法 | 新增 |
| 旧密封语义断言 | 废弃 | 既有「文本流项归一化为 assistant Message 的 text 块」「reasoning 全块归一化为 thinking 块」「tool_call 流项归一化为 tool_use 块」三组密封产物断言随双层化删除（改写为 delta/收口断言） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | rig 流项类型（Text/Reasoning/ToolCall/Unknown/Final）内存构造，无 provider client、无网络触达 | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目；本节锚定「修改文件」表行：轮末 choice 收口恰一条密封 Message（Text/Thinking/ToolUse 全部块收进）、ToolResult 密封、TurnDone 组装（统计字段面唯一出口、cost 恒 None）、空轮/熔断/API 失败收敛语义保留。 -->

- loop::run(): 多轮 loop（delta 逐发、轮末密封收口、TurnDone 组装、停止/失败语义）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 单轮密封收口 | 正向 | 一轮文本+思考假流：轮内 MessageDelta 逐发（流序一致）、轮末 choice 收口恰一条密封 Message（Text+Thinking 两块收进同一条、parentToolUseId=None）——AC-1 落库半边、AC-2 引擎半边 | 新增 |
| 工具轮密封收口 | 正向 | 两轮假流（ToolCall → 续轮）：assistant 密封 Message 收 ToolUse 块、ToolResult 密封、续轮密封 Message；每 assistant 回应恰一条密封 Message | 新增 |
| TurnDone 组装唯一口径 | 正向 | 收口事件变体为 TurnDone：numTurns 逐轮累计、durationMs 在场、costUsd 恒 None、sessionId 收口、usage 承接 Final——引擎无第二统计口径（AC-5） | 新增 |
| API 失败收敛 | 异常 | 假流 stream Err：SystemNotice（api_error 记因）+ TurnDone（is_error=true、subtype=api_error）收敛且恒为最后一事件 | 新增 |
| 空轮/熔断语义 | 边界 | 空流轮/连续空轮熔断：收敛语义保留，不悬挂不重复收敛 | 新增 |
| 停止先置位不合成收口 | 异常 | 停止信号先置位：泵终止、不合成 TurnDone（为编排侧显式 stopped 收敛让路）、已产出 delta/密封事件原样保留 | 新增 |
| 消费端关闭自行退出 | 边界 | 接收端先行 drop：loop 自行退出不悬挂 | 新增 |
| delta 占 seq 共享单调 | 边界 | delta 与密封混合流：seq 全程单调无跳号（共享单调 seq 空间裁定） | 新增 |
| 洪峰背压零丢失 | 边界 | 单轮大于 300 delta 洪峰经容量 256 通道：背压阻塞全量送达零丢失 | 新增 |
| 工具面零回归 | 边界 | 工具轮回灌请求史含成对 ToolResult；policy 拒绝/沙箱拦截两用例沿用（改造限于流面，工具执行/policy/sandbox 代码不动） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| FakeModel 假流缝模型（网络边界 mock） | rig CompletionModel trait 内存替身：逐轮预录 RawStreamingChoice、捕获 CompletionRequest、可编程 stream Err | 全部 describe |
| tempdir cwd（文件边界） | 工具真实执行于 tempfile 目录（read/write 真实读写，测毕自动清理）；policy/sandbox/tools 真实组合不 mock | 工具轮/policy/沙箱 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目；本节锚定「修改文件」表行：rebuild 重建源语义平移为会话全史转录（顶层非 Raw 密封事件、tool 成对回灌不变）；owns_session / SESSION_PREFIX 前缀校验退役（归属凭配置快照，sdk- 前缀仅保留为 remote id 铸造格式）。 -->

- rebuild(): 会话全史转录 → rig 对话历史（链式丢上下文根因修复的重建半边——AC-3）
- owns_session(): 退役（前缀校验不再是归属判据）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 全史重建多轮往返 | 正向 | 三轮全史转录（user/assistant 密封往返 x3）：重建史含全部三轮往返、逐条保序（第 N 轮重建史含前 N-1 轮——第一轮不再丢失的 AC-3 正解断言） | 新增 |
| 密封 Message 多块重建 | 正向 | 单条密封 Message 收 Text+Thinking+ToolUse 三块：重建为单条 rig assistant 消息（块保真不拆分） | 新增 |
| ToolResult 成对回灌 | 边界 | is_error 结果与工具名回溯保真（既有语义回归） | 新增 |
| 非对话事件丢弃 | 边界 | RunStarted/TurnDone/Raw/子代理归因密封事件不进对话史（TurnDone 更名后断言适配） | 新增 |
| 空史显式失败 | 异常 | 空转录/仅 Raw/仅 RunStarted/仅子代理：Err「重建历史为空」语义保留 | 新增 |
| 千级长转录保真 | 边界 | 1000 密封事件长转录：不丢消息不乱序（会话全史规模护栏） | 新增 |
| 前缀校验退役 | 废弃 | owns_session 非 sdk- 前缀显式失败用例删除（归属校验点移 compose 快照比对；残留引用即编译失败） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 转录以内存 Vec<AgentEvent> fixture 构造，rig 类型内存构造，store 本体不引入不 mock | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目；本节锚定「修改文件」表行：start → open_session/ask 两段（open 阶段配置三件套校验 + Continue 经装载缝取全史转录重建，Err/None → ConfigMissing；ask 阶段泵 select 停止 drop future；观察通道改载未盖戳 AgentEventKind；sdk remote id 每轮铸造经 RunStarted/TurnDone 上报）。 -->

- SdkRunner.open_session(): P1 协议两段式 SDK 实现（会话建立校验 + 全史装载）
- AgentSession ask 泵: select 泵（停止 drop future）、观察通道载未盖戳 AgentEventKind

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| open 配置三件套校验 | 异常 | 空缺省 EngineConfig：open_session 返回 ConfigMissing 且消息区分 api_key/base_url/model 成因 | 新增 |
| Continue 装载缝三形态 | 异常 | loader None（会话缺失）/ loader Err（库读取失败记因并入）/ 重建空史：均 ConfigMissing 且成因可区分 | 新增 |
| Continue 全史装载重建 | 正向 | loader 返回多轮全史转录：open 成功且重建史注入首轮请求（捕获缝断言首请求含全史）——丢上下文修复的引擎半边（AC-3） | 新增 |
| ask 泵未盖戳观察 | 正向 | 假流一轮：observations 收未盖戳 AgentEventKind（无 seq/时间戳——盖戳收内核），变体序 = delta 逐发 + 密封 + TurnDone（AC-5 观察词汇含 delta 半边） | 新增 |
| 停止 drop future | 边界 | 停止先置位：泵 select 停止臂命中、future drop、不合成 TurnDone、事件以先导收尾 | 新增 |
| 停止晚于 EOF | 边界 | EOF 收敛后停止：无二次收敛事件 | 新增 |
| sdk remote id 铸造上报 | 边界 | 每轮 sdk- 前缀 remote id 经 RunStarted/TurnDone 的 session_id 上报（内核写双 id 映射的来源半边——AC-5） | 新增 |
| 门面分发一致性 | 边界 | runner_for(Sdk) 产物 open_session 校验行为与直构 runner 一致 | 新增 |
| 旧 start 入口用例 | 废弃 | 既有 start 型启动/事件流用例与非 sdk 前缀启动拒绝用例随协议平移与归属裁定退役 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| FakeModel 假流缝模型（网络边界 mock） | rig CompletionModel trait 内存替身（逐轮预录、请求捕获、可编程 Err），不触真实 provider | 泵/收口/停止 describe |
| ResumeTranscript 闭包替身（注入依赖，入参例外） | 内存转录三形态（Ok(Some 全史)/None/Err）闭包 | Continue 校验 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/cli/runner_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目；本节锚定「修改文件」表行：start → open/ask 两段（open 无 IO 参数留存，ask 按既有流程 spawn + 逐行泵 + EOF 合成收敛 + 树杀缝），行为零回归（AC-10）。 -->

- pump_lines(): stdout 逐行泵（协议平移不改行为面，合成收敛变体更名 TurnDone）
- ClaudeCliRunner: P1 协议两段式 CLI 实现（唯一 spawn 触点，--resume 走 prior_handle）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 泵完整会话零回归 | 正向 | 内存行流四行 fixture：事件按序、seq 从 0 单调、线格式可被 core 表达（合成收敛断言适配 TurnDone 更名——AC-10） | 新增 |
| open/ask 两段形态 | 正向 | open 无 IO（参数留存不 spawn）、ask 阶段 spawn + 逐行泵 + 树杀缝：协议平移不改变 CLI 租户行为面 | 新增 |
| EOF 合成收敛 | 边界 | EOF 无 result + 退出码：合成收敛 is_error、退出码记入 usage、占下一 seq（TurnDone 断言适配） | 新增 |
| 停止路径 | 异常 | 停止信号中止泵、击杀缝恰一次携 pid、剩余行不再产出、不合成收敛、无 pid 尽力语义、信号晚于 EOF 无二次合成且击杀缝不被调 | 新增 |
| 洪峰背压与 Raw 透传 | 边界 | 315 行洪峰经容量 256 通道零丢失；未知/非 JSON 行 Raw 占 seq 透传 | 新增 |
| 旧 start 单段用例 | 废弃 | 既有以 start 单段入口驱动的用例随 open/ask 两段平移改写 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 内存行流 + 退出码缝（子进程边界 mock） | Cursor/duplex 行流替身 + 可注入退出码 future + 捕获型击杀缝闭包（不 spawn 真实进程） | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs -> packages/desktop/src-tauri/crates/infra/agent/src/cli/flags_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目；本节锚定「修改文件」表行：build_args 入参自 AgentRunParams 平移为协议轮参数形状，flag 输出序列零变化（含 --resume 尾追加规则，AC-10）。 -->

- build_args(): 入参形状平移（协议轮参数），flag 输出序列零变化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 四要素 flag 序列零回归 | 正向 | 三档 permission 全组合：组装序列与既有基线逐项相等（-p/prompt/stream-json/verbose + 档位 flag） | 新增 |
| resume 尾追加 | 正向 | prior_handle Some：--resume 与 id 恰在尾部、前缀与 None 组装逐项相等；None 无 --resume（--resume 语义平移不变） | 新增 |
| prompt 保真 | 边界 | 空 prompt/空格引号换行 emoji/超长（1200 字符）：单 arg 原样保留不拆分 | 新增 |
| resume id 保真 | 边界 | 空串/特殊字符/超长 prior_handle：--resume 后单 arg 原样保留 | 新增 |
| 禁用 flag 负向 | 边界 | --include-partial-messages/--model 恒不出现（增量翻译不进本期） | 新增 |
| cwd 不产生 flag | 边界 | cwd 变化不影响组装序列（仅经进程工作目录传递） | 新增 |
| AgentRunParams 入参形态 | 废弃 | 旧 AgentRunParams 构造的用例随入参形状平移改写（协议轮参数） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 纯函数入参内存构造，基线对照装置沿用 | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/cli/jsonl.rs -> packages/desktop/src-tauri/crates/infra/agent/src/cli/jsonl_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目；本节锚定「修改文件」表行：归一化产出变体 RunResult → TurnDone（含文档措辞），行解析语义零变化（AC-10）。 -->

- normalize_line(): 单行归一化（result 行产出变体更名为 TurnDone，其余语义零变化）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 五类行归一化零回归 | 正向 | init → RunStarted、其余 system → SystemNotice、assistant/user → Message、result → TurnDone（更名后断言）、未知/非 JSON → Raw | 新增 |
| result 行字段面 | 正向 | 全字段 result 行：isError/numTurns/durationMs/costUsd/usage/sessionId 承接进 TurnDone 变体 | 新增 |
| 空白行跳过 | 边界 | 空白行不占 seq（泵层 seq 语义的归一化回归锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 行 fixture 以内嵌常量构造 | 全部 describe |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件条目；本节锚定「修改文件」表与「类型定义」表：SessionRecord（id=7）/ SessionConfigSnapshot / SessionEventRecord（id=8，打包 u128 主键 + session_id 二级索引 + 密封 AgentEvent 嵌装）、AgentRunRecord 版本 3→4 轮统计行化（from_previous 存量行置 None）。 -->

- SessionEventRecord 键打包: hash64(session_id) 左移 64 位或 seq 的合成打包 u128 主键（大端序字典序 = seq 序，十六进制串 serde）
- SessionRecord/SessionConfigSnapshot: 会话模型与配置快照（serde 往返）
- AgentRunRecord: native_model 版本 3→4 原地演进（零迁移）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| SessionEventRecord 键打包保序 | 正向 | 同 session seq 增大则 event_key 严格增大；serde 十六进制串往返（event_key_serde 同款定式） | 新增 |
| SessionEventRecord 跨 session 隔离 | 边界 | 不同 session_id 同 seq：键区间经 hash64 高 64 位隔离不串键；seq=u64::MAX 极值不溢出 | 新增 |
| SessionEventRecord 密封嵌装 | 正向 | 密封 AgentEvent（含 Raw 逃生舱）serde_json 编解码往返逐字段相等（serde flatten 承载） | 新增 |
| SessionRecord serde 往返 | 正向 | 全字段往返（id 字符串主键/engineSessionId Option/configSnapshot 嵌套/source/sourceRef/时间戳） | 新增 |
| SessionConfigSnapshot | 正向 | engine/model/permission 三字段嵌套 struct 往返；model None 形态 | 新增 |
| AgentRunRecord v4 轮统计行 | 正向 | native_model 编解码版本断言 4；线格式含 sessionId Option 与统计字段，退役字段（prompt/cwd/env/permission_mode/source/sourceRef/parentRunId）不在线格式 | 新增 |
| AgentRunRecord from_previous | 边界 | 存量行经版本机制自动升级：session_id 置 None（孤儿轮行）、统计字段保留（AC-2 存量处置半边） | 新增 |
| 旧 AgentEventRecord 退役 | 废弃 | 既有 AgentEventRecord 键打包/嵌装用例随模型退役删除（id=3 定义移出注册，旧行惰性废弃） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | native_model 内存往返（不经 db 文件）；AgentEvent fixture 固定时间戳构造（等值断言与钟面无关） | 全部 describe |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- Store.create_session(): 会话行落库（写事务，id 来自 core 铸造）
- Store.find_session(): 主键直查（Continue 校验与装配消费）
- Store.begin_agent_turn(): 轮统计行 begin（写事务内 max+1 分配，session_id 挂 core 会话，running 初值）
- Store.append_session_events(): 密封转录追加（单事务；delta 防御性忽略不产生记录）
- Store.finish_agent_turn(): 轮行终态整行替换（status/finished_at/统计/error）
- Store.bind_session_remote(): 双 id 映射落库半边 + updated_at 刷新
- Store.list_sessions(): 会话清单（来源过滤、updated_at 降序稳定序）+ 聚合统计现算 + 轮统计行随行返回
- Store.list_session_events(): 转录重放（二级索引扫描 + seq 升序空洞容忍，不要求运行进程存活）
- Store.reconcile_session_stats(): 对账纠偏重导显式入口（从转录 TurnDone 重算，不改密封转录）
- Store.delete_explore_record(): 级联圈定自 runs 平移至会话（转录 + 轮统计行同事务删）；miss 幂等不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| create_session/find_session | 正向 | 会话行落库后主键直查逐字段相等；跨 workspace 库隔离（for_root 双库各开各库） | 新增 |
| create_session 主键冲突 | 异常 | 同 id 重复创建：Err（写事务原子性，不产半行） | 新增 |
| find_session miss | 边界 | 不存在 id：Ok(None)（Continue 校验消费形态） | 新增 |
| begin_agent_turn 分配 | 正向 | 同 session 连续三轮：轮 id max+1 递增、status running 初值、session_id 挂 core 会话（AC-2 键位半边） | 新增 |
| begin_agent_turn 跨 session | 边界 | 两 session 轮序独立分配互不串号 | 新增 |
| append_session_events 密封追加 | 正向 | 批量密封事件单事务追加：list_session_events 按 seq 升序全量重放（AC-2 落库半边、AC-6） | 新增 |
| append_session_events delta 忽略 | 异常 | 混入 MessageDelta 的批次：Ok 且 delta 零记录（store 中不存在任何 delta 记录的断言半边——AC-2） | 新增 |
| append_session_events 空批次 | 边界 | 空数组：Ok 无副作用 | 新增 |
| finish_agent_turn 整行替换 | 正向 | 终态行 status/finishedAt/统计/error 逐字段替换、id 与 started_at 不变 | 新增 |
| finish_agent_turn 边界 | 边界 | stopped 终态与 error=None 形态替换无损 | 新增 |
| bind_session_remote | 正向 | remote Some 刷新 engine_session_id + updated_at；remote None 清空形态 | 新增 |
| list_sessions 清单与聚合 | 正向 | 多会话多轮行：updated_at 降序稳定、source/source_ref 过滤、轮数/累计墙钟/累计 token 现算、轮统计行随行返回（AC-2 清单半边） | 新增 |
| list_sessions 缺席缺省 | 边界 | 无轮行/统计字段缺席：缺省聚合不报错（降级不违约）；空库空清单 | 新增 |
| list_session_events 重放 | 正向 | 空洞容忍（seq 1,3 密封、2 为 delta 占位）：升序返回不补洞 | 新增 |
| list_session_events 隔离 | 边界 | 跨 session 转录互不串；不存在会话空 Vec | 新增 |
| reconcile_session_stats 重导 | 正向 | 轮统计行被篡改后重导：从转录 TurnDone 重算校正（turnCount/累计 token），密封转录逐字节不变 | 新增 |
| reconcile_session_stats 无 TurnDone | 边界 | 缺 TurnDone 转录：缺省统计不报错 | 新增 |
| delete_explore_record 会话级联 | 正向 | (source, source_ref) 归属会话 + 其转录 + 轮统计行同事务删（AC-11 级联半边）；非归属会话与轮行保留 | 新增 |
| delete_explore_record miss 幂等 | 边界 | 不存在 explore：Ok(false) 无副作用（幂等不变回归） | 新增 |
| 模型注册组平移 | 边界 | workspace 组注册 agent_run/agent_session/agent_session_event/explore 且不再含 agent_event（AgentEventRecord 退役出注册）——list_models 断言更新 | 新增 |
| run 域旧 API 退役 | 废弃 | begin_agent_run/finish_agent_run/list_agent_runs/list_agent_run_events/append_agent_run_events/restore_run_chain 既有用例删除（残留引用即编译失败） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 store mock（DB 进程边界以真实库组合） | tempdir 真库 Env/StoresEnv 装置（既有）；时间戳显式 i64 实参注入 | 全部 describe |

### packages/desktop/src-tauri/crates/infra/store/src/lib.rs -> packages/desktop/src-tauri/crates/infra/store/src/lib_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的公共 API 变更（纯 re-export 同步：新模型出、退役 API 收）。crate 根可达性由 model_test/store_test 的 crate 根 use 导入编译期承载；本 change 不建独立 lib_test.rs（防空套件），见不可测试项 3。 -->

（design.md 未声明该文件的公共 API 变更——不建测试文件，可达性经 crate 根导入编译期承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 3） | — | re-export 同步无独立行为断言：model_test/store_test 的 crate 根 use 导入任一破坏即编译失败 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

- agent_start(): 发起命令语义升级（session_id=None 即 New、Some 即 Continue；来源缺省 debug；提前 resolve 返回 running 态轮行 TurnSummary）
- agent_stop(): 终止命令会话寻址（root 寻址保留）；幂等忽略不变
- agent_sessions(): 会话清单 + 聚合统计查询（root → workspace 库直查 DTO）
- agent_session_transcript(): 会话全史转录重放查询（全史密封事件 seq 序）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| agent_sessions 清单查询 | 正向 | mock app 托管真库：root 直查 DTO、source/source_ref 过滤透传、返回 SessionSummary 数组（AC-9 查询面新增） | 新增 |
| agent_session_transcript 重放 | 正向 | 落库密封转录后查询：全史密封事件按 seq 序返回（AC-9） | 新增 |
| agent_session_transcript 边界 | 边界 | 不存在会话：空数组；blank root 守卫 Err（root 寻址与 Result 模板保留） | 新增 |
| agent_start 无默认 agent Err | 异常 | 空库缺省：Err 文案含管理页引导语义（组合根解析单点经真实组合——AC-9） | 新增 |
| agent_start blank root 守卫 | 异常 | blank root：Err（守卫保留） | 新增 |
| agent_start Err 映射 | 异常 | CLI 臂 PATH 隔离 CliMissing / SDK 臂空配置 ConfigMissing：AgentStartError → Err(String) 映射直抵前端（三件事纪律的错误映射半边） | 新增 |
| agent_start 提前 resolve | 正向 | 成功链路（组合根/命令薄入口 runner 注入缝 + 假 runner）：返回 running 态 TurnSummary（finishedAt null）且会话行/轮行已落库（begin_turn 同步段） | 新增 |
| agent_start 参数转换 | 边界 | session_id None→New / Some→Continue、source 缺省 debug、sourceRef/agent 透传：经捕获缝断言参数转换（三件事纪律的参数转换半边） | 新增 |
| agent_stop 会话寻址 | 正向 | StopRegistry 登记后 agent_stop(root, session_id)：置位成功返回 Ok 且引擎收敛 stopped 经 Channel 流出终态部件（AC-7 命令半边） | 新增 |
| agent_stop 幂等 | 边界 | 未运行会话/不存在 session_id：Ok 不报错；blank root 天然 miss | 新增 |
| 退役查询命令 | 废弃 | agent_runs/agent_run_events/agent_run_chain 既有用例删除（函数级退役，残留引用即编译失败） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri mock_app（MockRuntime，Tauri 运行环境替身） | 托管真实 WorkspaceStores（tempdir 真库）与 StopRegistry，State 取用（沿既有装置） | 全部 describe |
| PATH 环境变量（进程全局变量边界） | 空 PATH 目录替换 + 共享互斥锁串行化，测毕恢复；触发 CliMissing 分支 | agent_start Err 映射 |
| Channel 捕获（IPC 边界） | tauri::ipc::Channel::new 捕获回调（或返回 Err 构造「页面已关」） | agent_start/agent_stop |
| 假 runner 注入缝（注入依赖，入参例外） | 组合根/命令薄入口泛型缝注入预录事件假 runner（成功链路正向；实现期沿 start_agent_run_with 泛型缝先例择缝） | agent_start 提前 resolve/参数转换 |

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的模块级导出条目；本节锚定「修改文件」表行：保留 AgentRunMessage IPC 信封（Record 臂载 TurnSummary）与 running/终态 TurnSummary 装配 helper（自 TurnRequest/RunningTurn/TurnOutcome 中性数据装配）；find_events_by_session / RunProvenance / RunSummary / resolve_agent_engine / ResolvedEngine / RunStopRegistry / start_agent_run* / drive_agent_run / running_record 编排体全量退役下沉。 -->

（design.md 未声明该文件的公共 API 变更——保留面为 IPC 信封与 TurnSummary 数据装配 helper，见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentRunMessage 信封双变体 | 正向 | Event（载 AgentEvent）/ Record（载 TurnSummary）序列化 tag=ipc 判别、键 camelCase（IPC 镜像回归——AC-8 信封前提） | 新增 |
| TurnSummary running 装配 | 正向 | 自 TurnRequest/RunningTurn 中性数据装配：running 形态（status=running、finishedAt/统计 None） | 新增 |
| TurnSummary 终态装配 | 正向 | 自 TurnOutcome 装配：status/finishedAt/统计/error 逐字段承接（IPC 终态部件与重放同构前提——AC-8） | 新增 |
| 编排体退役 | 废弃 | find_events_by_session/resolve_agent_engine/start_agent_run*/drive_agent_run/RunStopRegistry/RunProvenance 既有用例删除（下沉 kernel/compose，残留引用即编译失败；单 run 拼凑转录链路断言随之退役） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 信封与装配 helper 为纯数据构造/序列化，serde_json 内存断言 | 全部 describe |

### packages/desktop/src-tauri/src/main.rs -> packages/desktop/src-tauri/src/main_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的公共 API 变更（托管状态 RunStopRegistry → agent::StopRegistry 挂载替换）。挂载行为需完整 Tauri 运行时（run() 消费进程），进程内单测不可行；本 change 不建 main_test.rs，见不可测试项 2。挂载类型语义经 kernel_test 锁定。 -->

（design.md 未声明该文件的公共 API 变更——不建测试文件，挂载语义经 kernel_test 承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 2） | — | 托管状态挂载替换无进程内断言落点 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src/lib/agent-adapter.ts -> packages/desktop/src/lib/agent-adapter.test.ts

#### 待测功能

- eventsToUIMessages(): 重放折叠——仅消费密封事件（messageDelta 防御跳过）；turnDone → data-run-result
- eventToChunk(): 实时增量增 delta 路（messageDelta → text-delta/reasoning-delta，稳定 part id = deltaPartId）；密封 Message chunk 组保持 start+reset+整块（同键让位替换）
- provisionalMessageId(): provisional 消息键（配对键派生，adapter 与 transport 共用锚点）
- deltaPartId(): delta 累积部件的稳定 part id
- runRecordToUIMessage(): 终态同构部件组装（入参随 IPC 轮行 DTO 演进为 TurnSummary）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| eventsToUIMessages 密封-only | 异常 | 转录含 messageDelta：防御跳过不产部件不炸；纯 delta 序列折叠为空（重放视图天然密封-only——AC-8） | 新增 |
| eventsToUIMessages turnDone 映射 | 正向 | turnDone → data-run-result 部件（runResult 线值退役后断言适配）；runStarted/message/systemNotice/raw 既有映射回归 | 新增 |
| eventToChunk delta 路 | 正向 | messageDelta（delta=Text）→ text-delta chunk、（delta=Thinking）→ reasoning-delta；part id = deltaPartId(键, 类别) 恒定累积（AC-8 delta 半边） | 新增 |
| eventToChunk 密封同键让位 | 正向 | 密封 Message chunk 组：start（id=evt-seq）+ reset-step + 整块部件组——与 provisional 键同键让位（替换语义的 chunk 级表达，AC-8） | 新增 |
| eventToChunk 透传与保真 | 边界 | 未知事件 data-raw 原文透传不丢；seq 进 id/metadata、parentToolUseId 进 metadata 保真回归 | 新增 |
| provisionalMessageId 派生 | 正向 | 配对键 null → 单 provisional 通道键（SDK 恒 None）；"tu_1" → 键携 tu_1 可辨 | 新增 |
| provisionalMessageId 边界 | 边界 | 空串配对键、含特殊字符键：键生成稳定不炸 | 新增 |
| deltaPartId 稳定性 | 边界 | 同键同类重复调用幂等；text/thinking 两类互异；不同键互异 | 新增 |
| runRecordToUIMessage TurnSummary | 正向 | TurnSummary 入参 → data-run-record 部件形状不变；running（finishedAt null）与终态两形态 | 新增 |
| 两路同构（重放 vs 实时归一） | 正向 | 真实组合 eventsToUIMessages 与 eventToChunk：同一密封序列的重放折叠产物与实时 delta+密封 chunk 组的目标归一形状同构（AC-8 同构断言锚定） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 纯函数模块（ai 仅类型引用）：fixture 对齐 serde camelCase 线格式（messageDelta/turnDone 新线值），内存构造 | 全部 describe |

### packages/desktop/src/lib/agent-transport.ts -> packages/desktop/src/lib/agent-transport.test.ts

#### 待测功能

- TauriAgentTransport.sendMessages(): 方法签名不变；body 契约会话域化（sessionId 取代 resumeSessionId/parentRunId）+ 流内首 delta 簿记（配对键首个 delta 补发 start(provisional 键)+reset-step+part-start）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| body 会话域化 | 正向 | invoke("agentStart") 入参含 sessionId（null=New/串=Continue），resumeSessionId/parentRunId 不再出现；source/sourceRef/permissionMode 透传 | 新增 |
| 首 delta 簿记开件 | 正向 | 流内首个 messageDelta：补发 start（provisionalMessageId(键)）+ reset-step + part-start，随后 delta 直发 text-delta/reasoning-delta（稳定 part id 累积增长——AC-1 前端半边） | 新增 |
| 每配对键独立簿记 | 边界 | 键 null 与有值并行：各键独立开件互不串件 | 新增 |
| 密封与 aux chunk 组不变 | 正向 | 密封 Message → start（evt-seq）+reset-step+整块组；Record 收尾 → 终态部件；systemNotice/raw chunk 组回归 | 新增 |
| 无状态转换器约定 | 边界 | 同一 transport 两次 sendMessages：簿记为流闭包局部状态，第二流首 delta 再次补发开件组（跨流不残留） | 新增 |
| invoke reject | 异常 | agentStart reject：流 error 传播（消费端可感知）；Channel 发送失败不中断落库语义不回归 | 新增 |
| 旧 body 契约 | 废弃 | resumeSessionId/parentRunId 断言用例删除（字段退役） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke/Channel（@tauri-apps/api/core，进程边界） | invoke 按命令名分发（可切换 resolve/reject 并记录入参）；Channel 可编程 class 捕获 onmessage，测试直接投递信封驱动流 | 全部 describe |
| ReadableStream 与 chunk 消费 | 真实实现不 mock；adapter 真实组合 | 全部 describe |

### packages/desktop/src/hooks/use-agent-chat.ts -> packages/desktop/src/hooks/use-agent-chat.test.ts

#### 待测功能

- useAgentChat(): 会话域状态基建（重放装载/发送/停止/归一；对外新增 session 镜像，chain 平移为轮统计行列表）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 重放装载会话域 | 正向 | invoke("agentSessions") 取最新 updated_at 会话 + invoke("agentSessionTranscript") 全史转录：messages 折叠装载、session 镜像确立、chain 为轮统计行列表 | 新增 |
| 镜像只收密封事件 | 边界 | 实时流投递 messageDelta：镜像不进 delta（仅 chunk 流可见）；密封到达镜像更新——镜像与原始转录视图密封-only 同构（AC-8 hook 半边） | 新增 |
| sendMessage Continue 收口 | 正向 | 既有 session 镜像时发送：body sessionId = 当前会话 id（Continue）；首轮发送后 session 镜像确立（New → 承接） | 新增 |
| stop 会话寻址 | 正向 | stop() → invoke("agentStop", root, sessionId)；running 态可停 | 新增 |
| 收口归一轮行交错 | 正向 | TurnDone 事件位插入对应轮 data-run-record 部件、与轮序对齐；重放与实时归一终态同形状（AC-8 收口半边） | 新增 |
| 装载/发送失败 | 异常 | agentSessions/agentSessionTranscript/agentStart reject：error 态呈现、可重试（行为不回退） | 新增 |
| run 链装载 | 废弃 | agent_run_chain/agent_run_events 装载与 parentRunId 链断言用例删除（会话域取代） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke/Channel（@tauri-apps/api/core，进程边界） | invoke 按命令名分发（agentSessions/agentSessionTranscript/agentStart/agentStop，可切换 resolve/reject 并记录入参）；Channel 可编程 class 投递信封驱动实时流与终态回流 | 全部 describe |
| 适配层与 transport | 真实实现参与，不 mock ai 的 useChat（内部模块不 mock 原则） | 全部 describe |

### packages/desktop/src/views/explores/hooks/use-explore-session.ts -> packages/desktop/src/views/explores/hooks/use-explore-session.test.ts

#### 待测功能

- useExploreSession(): 对外签名与返回形状不变（messages/chain/events/loading/running/error/send/stop）；内部经基建会话寻址语义等价切换（stance 拼接留本层）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 返回形状语义等价 | 正向 | renderHook 断言对外返回键集与形状不回退（messages/chain/events/loading/running/error/send/stop）——AC-11 契约面 | 新增 |
| 续话语义等价 | 正向 | send 拼接 buildExplorePrompt（stance 真实参与）→ agentStart 携 source/sourceRef/sessionId；连续两轮 Continue 链上历史还原（全史转录） | 新增 |
| 历史还原 | 正向 | 挂载重放：agentSessionTranscript 全史折叠装载（与原 run 链拼凑产物形状等价） | 新增 |
| 失败与停止语义 | 异常 | agentStart reject / stop 会话寻址：error/running 态机行为不回退 | 新增 |
| run 链断言 | 废弃 | agent_run_chain 链拼凑与 parentRunId 断言用例删除（会话域取代） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke/Channel（@tauri-apps/api/core，进程边界） | invoke 按命令名分发（记录入参、可切换 reject/pending）；Channel 可编程 class 投递信封 | 全部 describe |
| buildExplorePrompt（stance 前导） | 真实实现参与 send 拼接语义断言，不 mock（内部模块不 mock 原则） | 续话 describe |

### packages/desktop/src/views/agent/agent-debug-view.tsx -> packages/desktop/src/views/agent/agent-debug-view.test.tsx

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的导出函数；本节锚定「修改文件」表行：历史区改会话历史接线、每跑重置（New 会话）与流式呈现参数保持——页面级 chrome，时间线组件零改动复用。 -->

（design.md 未声明该文件的公共 API 变更——页面级接线行为见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 历史区会话接线 | 正向 | mock IPC 驱动真实 useAgentRunHistory：会话行列表渲染（时间/轮数/终态）、点开转录重放 | 新增 |
| 每跑重置 New 会话 | 边界 | New 会话发起后再次发起：不携带旧 sessionId（每次 New 会话） | 新增 |
| 流式呈现参数 | 边界 | mock Channel 投递 delta+密封信封：时间线呈单条连续增长消息（碎行不复现——AC-1 UI 半边） | 新增 |
| run 域取数驱动 | 废弃 | agent_runs/agent_run_events 驱动的既有用例删除（会话域取代） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke/Channel（@tauri-apps/api/core，进程边界） | 唯一 mock 点：invoke 按命令名分发 + Channel 可编程 class（沿既有最小 mock 修订） | 全部 describe |
| use-agent-chat / use-agent-run-history | 进程内内部协作者不 mock，fixture 经 mock IPC 流入真实 hook | 全部 describe |

### packages/desktop/src/views/agent/components/agent-run-history.tsx -> packages/desktop/src/views/agent/components/agent-run-history.test.tsx

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的导出函数；本节锚定「修改文件」表行：run 行列表 → 会话行列表（updated_at/轮数/终态）+ 点开会话转录重放（密封-only eventsToUIMessages）。 -->

（design.md 未声明该文件的公共 API 变更——组件呈现行为见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 会话行列表 | 正向 | state fixture（SessionSummary 形状）：updated_at/轮数/终态三要素渲染；终态标签四态（running/completed/failed/stopped）不回归 | 新增 |
| 点开转录重放 | 正向 | openSession 触发后渲染密封转录折叠产物（eventsToUIMessages 真实参与）；空转录形态 | 新增 |
| run 行列表 | 废弃 | AgentRunRecord 行（prompt/#id/run-status）断言用例删除（会话行取代） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | state 对象以 fixture 直传（AgentRunHistoryState 演进形状：sessions 取代 runs） | 全部 describe |

### packages/desktop/src/views/agent/hooks/use-agent-run-history.ts -> packages/desktop/src/views/agent/hooks/use-agent-run-history.test.ts

#### 待测功能

- useAgentRunHistory(): 会话清单 + 转录重放取数轨道（显式触发、无轮询纪律保留；重放轨道增 selectedSessionId）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 会话清单取数 | 正向 | 挂载/refresh → invoke("agentSessions", 带 root/source/sourceRef)：清单承接、loading/error 态机 | 新增 |
| 转录重放取数 | 正向 | openSession(id) → invoke("agentSessionTranscript", 带 root/sessionId)：events 承接、selectedSessionId 状态 | 新增 |
| 显式触发无轮询 | 边界 | 无定时器、无自动重放：仅显式 refresh 与点开触发（纪律回归） | 新增 |
| 取数失败 | 异常 | invoke reject：error 呈现、清单保留旧值 | 新增 |
| run 域取数 | 废弃 | invoke("agentRuns"/"agentRunEvents") 既有用例删除（会话域取代） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（@tauri-apps/api/core，进程边界） | invoke 按命令名分发（agentSessions/agentSessionTranscript，可切换 resolve/reject 并记录入参） | 全部 describe |

---

## 不可测试项

- AC-4 内核纯度（无直接 IO、源码无 claude/引擎字样、无 Tauri 依赖、core/agent 零 workspace 内 crate 依赖） — **原因**: 静态结构约束而非进程内可断言行为：依赖方向由 Cargo.toml（零 workspace 内依赖）与 cargo 依赖图承载、字样与 Tauri 引用由源码文本扫描（grep）在评审/验收阶段静态核查；行为半边（IO 一律经 port）由 kernel_test 的假 sink 注入与「无文件/网络依赖即可全测」结构性锁定。
- main.rs 托管状态挂载替换（RunStopRegistry → agent::StopRegistry） — **原因**: 挂载行为需完整 Tauri 运行时启动（run() 消费进程），进程内单测不可行；不建 main_test.rs（防空套件），挂载类型语义经 kernel_test 锁定，端到端挂载经应用人工验收。
- store/src/lib.rs re-export 同步（新模型出、退役 API 收） — **原因**: 纯导出面无独立行为；不建 lib_test.rs（防空套件），crate 根可达性由 model_test/store_test 的 crate 根 use 导入编译期承载。
- sdk/mod.rs 模块文档措辞同步（RunResult → TurnDone） — **原因**: 纯文档零行为，无运行时可断言语义；更名归零由全 crate 编译（grep 无残留）与 jsonl/loop/resume 断言面间接锁定。
- packages/desktop/src/types/generated/bindings.ts 再生成 — **原因**: 生成物非手改，无独立单测落点；正确性由「同输入连续两次导出零 diff」自检与前端套件经生成类型镜像的编译/运行间接锁定。
- packages/desktop/package.json 版本 bump 与 crates/infra/agent/Cargo.toml 依赖边新增 — **原因**: 清单/配置项无可执行行为；版本值与依赖边经评审静态核查。
- 真实外部进程与真实 provider 流的端到端（真实 claude spawn、真实 SSE 流、真实 provider 流驱动调试页的最终视觉呈现） — **原因**: 进程/网络边界不可进进程内单测；CLI 泵以内存行流替身、SDK 以假流缝模型锁定行为面（既有先例），端到端呈现属验收阶段人工核查。
- 存量库旧行惰性废弃（旧 AgentEventRecord 行不可读、死数据留盘、零迁移冷启动） — **原因**: 「旧行不可读且不影响新模型」是 absence 断言（新库无旧行、存量库需预置旧版本字节），进程内单测无自然落点；版本演进半边由 model_test 的 from_previous/版本断言承载，存量库冷启动经人工验收。

