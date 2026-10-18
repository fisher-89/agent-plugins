# desktop-agent-execution Specification (Delta)

## ADDED Requirements

### Requirement: 会话级实时流订阅面（会话注册表 + agent_session_watch）

后端 SHALL 提供**会话级实时流订阅面**，使非发起方（旁观视图 / 页面重挂 / 第二窗口）能拿到同一会话的实时事件流（含 ephemeral delta）：

- **注册表**: `core/agent` SHALL 提供 per-session 流注册表（键 = core session id，进程内存态，与 `StopRegistry` 同型托管为壳层 `Arc` 单例、经组合根注入内核；应用重启即清空），承载有界 broadcast（容量常量单点，可承载一轮 delta 突发）。条目生命周期 SHALL 为轮：`begin_turn` 登记、`converge` 投终态后除名。
- **写入单点**: 实时事件与轮终态 SHALL 由内核 `drive` 单点发布（`KernelOutput::Observation` 含 delta、`TurnFinished` 为终态），exec/chat 轨与 change 轨（walker 经 `KernelWorkerPort` 驱动）SHALL 同源进入同一注册表，MUST NOT 由各消费方（命令层 / worker / sink）各自补发（消费方零改动可观测）。注册表投递载体 SHALL 为 core 中性类型（`AgentEvent` + `TurnOutcome`），app 层信封（`AgentRunMessage`）由命令层投影——core MUST NOT 认识 IPC 信封。
- **订阅命令**: 命令面 SHALL 提供 `agent_session_watch(on_event: Channel<AgentRunMessage>, root, session_id, since_seq)`，命令体为薄包装（登记订阅 → Channel 前向 → 结束释放）。信封 SHALL 复用 `AgentRunMessage`（`Event` / `Record` 双变体；载荷直投，含 `messageDelta`——delta 无查询结果可依，ephemeral 数据 MUST 走「载荷权威」而非「通知 + 重查」）。
- **since_seq 过滤**: 订阅 SHALL 只投递 `seq > since_seq` 的事件（MUST NOT 重投已装载的旧事件）；实现 MAY 以「先补发库内密封快照（同样 `seq > since_seq`）、再续接 live」达成无空洞，MUST NOT 因快照与 live 衔接产生重复或空洞（以 seq 为准）。
- **按轮收流**: 一轮终态 SHALL 以 `Record` 信封收尾并结束命令（订阅流按轮划分，MUST NOT 长连接跨轮保持——长连接会同时杀掉客户端收口闸与状态复位）；无运行轮（会话未运行 / 已终态 / 未知 id / blank root）SHALL 幂等立即结束（零事件、零副作用、不 `Err`）。
- **滞后兜底**: 有界 broadcast 溢出（lagged）SHALL 以断流（结束命令、不投递伪终态）兜底，MUST NOT 静默丢事件后继续投递；客户端据断流重放重订（见 desktop-agent-chat-infra「会话级统一渲染组件」）。
- **落库纪律不变**: 订阅面投递的 `messageDelta` MUST NOT 落库；订阅面与持久面无交叉（delta 仅存在于传输面）。

#### Scenario: 旁观订阅运行中会话

- **WHEN** 某会话一轮运行中（会话由 change walker 或另一视图发起），旁观方 invoke `agent_session_watch(root, sessionId, sinceSeq)`
- **THEN** 该 Channel 持续收到该轮事件（含 `messageDelta`，seq 严格递增），轮收口时收到恰一条 `Record`（终态轮行）后命令结束

#### Scenario: since_seq 过滤与重挂无重复

- **WHEN** 订阅方已装载到 `seq = 12` 的转录，以 `since_seq = 12` 订阅
- **THEN** Channel 只投递 `seq ≥ 13` 的事件（已装载事件零重投）；重挂（再次订阅）行为一致，无重复注入

#### Scenario: 无运行轮幂等结束

- **WHEN** 对一条已收口会话（或未知 session id、blank root）invoke `agent_session_watch`
- **THEN** 命令立即成功结束、零事件、不报错、不产生任何记录或副作用

#### Scenario: 双轨同源可订阅

- **WHEN** change run 的 walker 驱动的 WorkerAgent 会话运行中，与 exec 轨发起的会话运行中
- **THEN** 两者的实时事件均经同一注册表可订阅（写入点在 kernel `drive`），watch 命令与发起方业务零耦合

#### Scenario: 滞后断流不装作在线

- **WHEN** 订阅方消费慢于广播（窗口溢出触发 lagged）
- **THEN** 命令结束该订阅（不投递伪终态、不继续投递残缺序列），客户端随后以重放重订恢复

#### Scenario: delta 投递不落库

- **WHEN** 一轮含 N 个 text delta 的回复经订阅面实时流出后收口
- **THEN** 订阅面收到 N 个 `messageDelta` 与完整密封事件；所属 workspace 库该轮落库序列仍只有密封事件（零 delta 记录）

## MODIFIED Requirements

### Requirement: run 事件落库与重放（workspace 维度）

agent 会话持久化 SHALL 以 **workspace 维度**落盘于所属 workspace 的独立 db 文件（落位与寻址见 desktop-workspace-store），MUST NOT 落全局库。持久化纪律（write-through）：

- **密封唯一 durable**：store sink 只落密封类事件（Message / TurnDone / RunStarted / SystemNotice / Raw）；`MessageDelta` 永不落库（redb 不吃每 token 一行，碎事件根因的落库半边修复）
- **write-through**：内核收到密封事件/返回值即 store 原子操作落盘；落库失败 SHALL 显式失败收敛（failed 记因、尽力流出终态部件），MUST NOT 静默
- **转录挂 session**：密封事件直挂 session（转录单表，键位形态由 design 定稿），run 退化为轮统计行（挂 session 外键）；转录 = 会话全史 = 重放与重建唯一来源
- **对账纠偏**：查询 api 对账重导（从转录重建统计/索引）保留为纠偏机制，非主路径

事件投递 SHALL 为「持久面 + 实时面」两面：内核持久面（write-through 单点落库）+ 会话实时面（同一事件源的两个出口——发起命令的 `Channel<AgentRunMessage>` 与会话订阅命令 `agent_session_watch` 的 `Channel<AgentRunMessage>`，同源同构、同一信封）；delta 只走实时面（`messageDelta` ephemeral，永不落库），密封事件两面同源同戳。历史查看 SHALL 走所属 workspace 库的会话查询重放（携 root），MUST NOT 依赖全局事件广播或要求运行进程存活。MVP MUST NOT 实现留存清理（转录无上限增长为已知限制留痕）。

#### Scenario: write-through 同源投递

- **WHEN** 运行产生密封事件
- **THEN** 每密封事件到达实时面（发起流与订阅流同源）与 store 且以同一盖戳同源；delta 只出现在实时面、永不出现在 store

#### Scenario: 两条实时出口载荷一致

- **WHEN** 同一轮运行既有发起方在听、又有旁观方经 `agent_session_watch` 订阅
- **THEN** 两个出口收到同一事件序列（seq / 载荷逐一相同，含 delta），无出口专属加工

#### Scenario: delta 零落库

- **WHEN** 一轮含 N 个 text delta 的回复收口后审查所属 workspace 库
- **THEN** 该轮落库序列恰一条密封 Message，无 delta 记录；轮统计行收 TurnDone 字段面

#### Scenario: 重放不依赖进程

- **WHEN** 会话收口后（或应用重启后）在原 workspace 下点开历史会话
- **THEN** 经会话查询（携 root）还原全史密封转录时间线，与实时流内容一致

#### Scenario: workspace 库落位与隔离

- **WHEN** workspace A 与 B 各有会话历史后审查两库内容与查询结果
- **THEN** A 的会话/转录/统计仅存在于 A 的 workspace 库文件，B 的任何查询不可见（反之亦然）；全局库无会话数据

### Requirement: agent 执行命令面

`commands/exec/` SHALL 承载执行与查询命令，命令体 SHALL 收敛为内核公共 API 的薄包装（三件事纪律：参数转换 → 调用 → 错误映射）：

- 发起命令（现 `agent_start`）：参数转换后调用内核 `run_session` 形状 API——`New` 建立会话、`Continue{id}` 续轮（现 `resume_session_id` 语义平移）；编排（runner 组装、tee、落库、盖戳、收敛）SHALL 全部下沉内核，命令体 MUST NOT 编排。来源参数（`source` / `source_ref`）SHALL 写入会话归属（provenance 记录用）；`source` 不传时缺省 `debug`，`source_ref` 前端恒传 null（会话寻址改走 `sessionId` 直查，不再经来源反查）。cwd 恒为当前 workspace root（root 解析所属 workspace 库保留）；引擎组装 SHALL 经内核组合根（见「引擎门面与 EngineKind 参数选择」），命令体 MUST NOT 直接构造具体引擎。命令命名与参数面演进由 design 定稿（保留语义升级或更名会话域命令二选一，见提案待决问题）
- 查询命令（现 `agent_runs` / `agent_run_events` / `agent_run_chain`）：演进为会话域查询薄包装（分页会话清单含聚合统计与 total 计数 / 会话转录重放 / 源过滤清单），参数含 root → workspace 库解析 → store/内核查询 → DTO；前端寻址与重放 SHALL 走 `session_detail` 单查（见「session_detail 单查面」）而非按来源反查取最近一条；旧 run 域命令的退役/收窄路径由 design 定稿
- 终止命令（现 `agent_stop`）：见「agent_stop 终止与提前 resolve」；root 寻址保留
- 订阅命令（新 `agent_session_watch`）：见「会话级实时流订阅面」；参数 `(root, session_id, since_seq)` + `Channel<AgentRunMessage>`，命令体为薄包装（登记订阅 → Channel 前向 → 结束释放），MUST NOT 编排会话生命周期；blank root / 无运行轮幂等结束（与 `agent_stop` miss 幂等同口径）
- 发起命令 SHALL 保持 `Channel<AgentRunMessage>` 实时出口与提前 resolve 契约（发送方流零空洞：Channel 先于 invoke 建立）；MUST NOT 因新增订阅命令而收窄或移除该出口（两出口同源，非二选一）

错误约定沿用既有模板 `Result<T, String>`：spawn 失败、SDK 引擎配置缺失（经 `AgentStartError` 中性变体）、无默认 agent 可解析、存量 cli 实例解析（CLI 租户已下线）、workspace 库打开失败等 SHALL 以 `Err` 抵达前端，MUST NOT 静默吞掉。

#### Scenario: 命令体薄包装

- **WHEN** 审查发起命令实现
- **THEN** 命令体为参数转换 + 调用内核 API + 错误映射三段，编排（tee/落库/收敛/盖戳）在内核内，不膨胀命令体

#### Scenario: 来源与会话续接参数透传

- **WHEN** explore 页以 `source="explore"`、`source_ref=null` 且携带记录回写的 `sessionId` 发起续轮
- **THEN** 生成会话归属 `source="explore"` 且以该会话全史续接（store 全史转录重建）；调试页 invoke（不传来源参数）生成的会话 `source="debug"`

#### Scenario: agent 缺省与解析

- **WHEN** invoke 不带 `agent` 参数（正式场景如 explore 链恒不传）
- **THEN** 内核组合根缺省解析默认 agent 并经门面构造对应引擎（无默认 agent 时显式 `Err`，不落库不推流、不静默回退）；调试页显式传 agent 时按所选解析；协议参数面全程无 agent / engine 概念（core 零污染）

#### Scenario: 查询与停止 root 寻址

- **WHEN** 前端 invoke 会话查询与终止命令
- **THEN** 命令无状态、按 root 解析所属 workspace 库直查/寻址返回 DTO，无领域解释；workspace A 的调用看不到 B 的会话

#### Scenario: 订阅命令薄包装与幂等

- **WHEN** 审查 `agent_session_watch` 实现并对一条已收口会话 invoke
- **THEN** 命令体为参数转换 + 注册表订阅 + Channel 前向三段（无编排、无落库）；已收口会话调用立即成功结束（零事件、无副作用）

#### Scenario: 错误 reject 传达

- **WHEN** SDK 引擎配置缺失、解析目标为存量 cli 实例（CLI 租户已下线）、无默认 agent 可解析或所属 workspace 库打开失败时发起运行
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

### Requirement: Agent 调试页

前端 SHALL 提供 Agent 调试页（`AgentDebugView`），经侧栏「系统工具」组进入；视图切换 SHALL 沿用本地 state，MUST NOT 引入路由。页面 SHALL 包含：

- **参数面（最小集）**：prompt（必填）、permission-mode 三档下拉（默认 bypassPermissions）、agent 选择器（管理页 agent 实例清单，默认选中默认 agent——与组合根缺省解析一致；**仅调试页暴露**，正式场景无 agent 选择入口）；cwd 不设参数（隐含当前 workspace root）、model 不设参数、env 不设参数
- **事件时间线**：SHALL 经会话级统一组件 `AgentSessionView`（desktop-agent-chat-infra，对话透镜 `AgentMessages`）呈现——**运行中 token 级流式**（delta → provisional 消息连续增长，思考/回复可辨，密封到达以密封为准替换，无碎行）、气泡序（seq 序）、tool_use / tool_result 成对卡；run-started / result / record 等辅助载体不进对话呈现（保真原文经下方「原始转录切换」与部件元数据可考）；两引擎共用同一会话组件
- **运行中停止**：运行中 SHALL 呈现停止入口，触发终止命令（见「agent_stop 终止与提前 resolve」）
- **原始转录切换**：落库密封转录（含 Raw）原文可见（与 store 一致）；delta 仅实时面可见，不进入原始转录视图
- **历史会话**：会话列表 → 点开经同一 `AgentSessionView` 重放该会话（含轮行；invoke 查询 + 状态驱动挂流）

页面状态 SHALL 经统一会话基建（`use-agent-session`）承载；run 表单 / 历史列表 / 转录开关为页面级 chrome，包在共享核心外圈。实时流经 transport 走 Tauri `Channel<AgentRunMessage>`（执行流通道，不属轮询取数）：发送期由发起流承载、历史会话点开（旁观 / 重挂）期由会话订阅面承载；查询类取数（会话列表 / 转录重放）仍由用户显式动作触发。

#### Scenario: 参数面默认值

- **WHEN** 打开调试页
- **THEN** permission-mode 默认 bypassPermissions、agent 选择器默认选中默认 agent（与组合根缺省解析一致）、prompt 为空且必填；无 model / cwd / env / 引擎直选输入

#### Scenario: agent 切换

- **WHEN** agent 选择器选 sdk agent 发起一次含工具调用的运行
- **THEN** 运行走 SDK 引擎（连接配置来自所选 agent 的引用 provider），对话视图 / 落库 / 重放组件零改动复用

#### Scenario: 流式连续呈现

- **WHEN** 一次含长文本回复与思考的 SDK 运行进行中
- **THEN** 对话视图呈单条连续增长的 provisional 消息（token 级、思考/回复可辨），轮末密封到达后以密封内容替换，不出现逐 delta 碎行

#### Scenario: loop 可见性

- **WHEN** 一次含工具调用的运行完成
- **THEN** 对话视图可按序看到 assistant 思考/回复气泡与 tool_use → tool_result 成对卡；result / raw 原文经「原始 JSONL」切换可读（统计字段如实呈现，SDK 的 cost 如实缺省），子代理归因字段可考于部件元数据

#### Scenario: 运行中停止

- **WHEN** 调试页运行中点击停止
- **THEN** 触发终止命令，事件流以终止终态收尾，轮状态收敛为 `stopped`

#### Scenario: 原始转录切换

- **WHEN** 切换原始转录视图
- **THEN** 落库密封转录（含 Raw 透传件）原文可见，与 store 一致；视图中无 delta 记录

#### Scenario: 历史重放

- **WHEN** 从历史会话列表点开一条已完成会话
- **THEN** 经 invoke 查询以会话全史转录渲染完整对话视图（含轮行，经同一 `AgentSessionView`），不要求原运行进程存活

#### Scenario: 历史会话运行中旁观流式

- **WHEN** 从历史列表点开一条仍在运行的会话（有 running 轮行）
- **THEN** 组件以 `since_seq` 订阅该会话并 token 级流式呈现（不经发起流），轮收口后以重放收敛为与重开一致的形状

## Module Contract

### API: `packages/desktop/src-tauri/src/commands/exec/mod.rs`（命令面）

| 命令 | 签名 | 描述 |
|------|------|------|
| `agent_session_watch` | `(on_event: Channel<AgentRunMessage>, root: String, session_id: String, since_seq: u64) -> Result<(), String>` | 会话级实时流订阅（薄包装）：注册表订阅 → Channel 前向（`Event` / 末拍 `Record`）→ 结束释放；无运行轮 / blank root 幂等立即结束 |
| `agent_start`（不变语义） | `(on_event: Channel<AgentRunMessage>, root, prompt, permission_mode, session_id, source, source_ref, agent) -> Result<TurnSummary, String>` | 提前 resolve 保持；发起流作为发送方实时出口保留（与订阅面同源） |

### Function: `packages/desktop/src-tauri/crates/core/agent`（会话流注册表）

| 项 | 签名 | 描述 |
|------|------|------|
| `SessionStreamRegistry` | `new() -> Self`；`begin(session_id: &str)`；`publish(session_id: &str, event: AgentEvent)`；`finish(session_id: &str, outcome: TurnOutcome)`；`subscribe(session_id: &str, since_seq: u64) -> Option<Receiver<SessionStreamItem>>` | per-session 有界 broadcast（载体 = core 中性 `AgentEvent` / `TurnOutcome`）；轮生命周期登记 / 除名；终态投递后除名（订阅端自然收尾） |
| `SessionKernel` / `RunningTurn` | 注入 `Arc<SessionStreamRegistry>` | `drive` 内 `Observation` 单点发布、`converge` 投终态并除名（exec / change 两轨同源） |
| `compose_turn`（infra 组合根） | `compose_turn(stores, stop_registry, stream_registry, store, agent) -> Result<ComposedTurn, String>` | 注册表注入缝（沿 `StopRegistry` 先例） |

### Function: `packages/desktop/src/hooks/use-agent-session.ts`（改：自 `use-agent-chat.ts` 收编）

| 项 | 变化 | 描述 |
|------|------|------|
| 会话状态与挂流 | 扩职责 | 调试页状态面改经本 hook；实时流双起点（发送期发起流 / 旁观期订阅流）；`src/hooks/use-agent-chat.ts` 退役（详见 desktop-agent-chat-infra） |
