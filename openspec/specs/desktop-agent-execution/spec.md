# desktop-agent-execution Specification

## Purpose

定义 agent 执行能力（第五类边界）的完整契约：`core/agent` 中立契约（AgentEvent 信封、AgentRunner trait、run 状态机）、`infra/agent`（裸名 `agent-runtime`）的引擎门面与双引擎租户（`cli/` 本机 Claude CLI、`sdk/` rig 进程内 SDK）、workspace 维度运行记录落库与重放、exec 轨道首批命令与前端 Agent 调试页，以及两租户 MVP 边界留痕。

## Requirements

### Requirement: agent 边界与双 crate 分层

Desktop 后端 SHALL 将「agent 执行」确立为第五类边界（agent 边界），并按 core/infra 分离落地为两个 crate：

- `crates/core/agent`（裸名 `agent`）：**会话域内核**——会话域中立契约与内核编排：`AgentEvent` 词汇（delta/sealed 双层）、P1 引擎协议（会话建立 / question / stop / 观察回流）、会话状态机与收敛状态机（running → completed/failed/stopped）、seq/时间戳盖戳治理、停止注册（内存态）、write-through 持久化编排、查询契约与 port 定义。纯度原则修订（2026-10-01 裁定）：「纯类型无 IO」改写为「**无对外部世界的直接 IO、经 port**」——文件/进程/网络等直接 IO MUST NOT 出现，IO 一律经 core 定义的 port 由 infra 实现；进程内存原语（时钟盖戳、mpsc/Notify、内存注册表）不在禁令内。MUST NOT 依赖 Tauri，MUST NOT 执行进程 spawn，MUST NOT 认识 claude（不出现 claude 特有 flag 名与 JSONL 行解析），MUST NOT 认识引擎（不出现 engine / rig / openai 概念）；loop / memory / LLM 协议归 infra 不变。
- `crates/infra/agent`（裸名 `agent-runtime`）：agent 边界的引擎门面与实现——对外以 `EngineKind`（`cli` | `sdk`）参数选择引擎（值域保留为多租户协议预留），引擎分发 SHALL 为门面内唯一 match 点；内部承载 `sdk/` 引擎（rig 进程内租户：agent loop、自建工具面、静态权限 policy、路径沙箱、事件归一化、会话转录恢复）。`cli/` 引擎（本机 Claude CLI 租户：CLI 发现（Windows `.cmd`）、进程 spawn、stdout 逐行泵、JSONL 解析归一化）实现 SHALL 随 retire-cli-engine 退役（`cli/` 模块删除；`EngineKind::Cli` 臂退役为显式 `Err("CLI 租户已下线")`，多租户机制保留——见「引擎门面与 EngineKind 参数选择」）。SHALL 实现内核 port（含 store 持久面与查询面要求的落地）；MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，非 shell 边界职责）；`rig`（rig-core 的 re-export facade，sdk-engine-gap-closure 自直连切换）SHALL 恒编译、MUST NOT 设 feature gate（SDK 引擎定位为不依赖本地环境的 agent，开箱即用，不提供 CLI-only 裁剪档；rig-core 经 facade 传递入树）。

依赖方向 SHALL 为：壳 crate → `agent` + `agent-runtime`，`agent-runtime → agent`；`agent` MUST NOT 依赖 workspace 内任何 crate（port 落点裁定的默认形态：core 定义 port、infra 实现，该规则保持不变；若 design 裁定 core 直依 store crate，须同步修订 desktop-crate-layout）。两 crate SHALL 注册进 `src-tauri/Cargo.toml` workspace members（rust 套件注册于 src-tauri 根，测试自动覆盖），且 MUST NOT 据此预建其他 infra 边界目录。单 crate 起步（`sdk/` 子模块收拢）；拆引擎子 crate + 门面 re-export 仅作膨胀后的逃生门，MUST NOT 预建。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查 `src-tauri/Cargo.toml` workspace members 与各 crate 依赖声明
- **THEN** members 含 `crates/core/agent` 与 `crates/infra/agent`，`agent-runtime` 依赖 `agent` 与 `rig`（facade），壳依赖两者
- **AND** `agent` 与 `agent-runtime` 的依赖中无 tauri 系条目，`agent` 无 workspace 内依赖（core 定义 port、infra 实现形态下）

#### Scenario: core 不认识 claude 与引擎且无直接 IO

- **WHEN** 审查 `crates/core/agent` 源码
- **THEN** 无 claude 特有概念（flag 名、JSONL 行解析、claude 事件 type 名），亦无引擎概念（engine / rig / openai 字样）；无文件/进程/网络直接 IO 触点（IO 全经 port 签名表达）；claude 细节零残留于源码（`cli/` 引擎已随 retire-cli-engine 退役），rig / openai 细节全部关在 `sdk/` 引擎内

#### Scenario: spawn 住在 infra

- **WHEN** 检索进程 spawn 调用（`std::process` / `tokio::process`）
- **THEN** 仅出现在 `crates/infra/agent`（`sdk/` bash 工具执行体）；`core/agent` 与壳 command 层无 spawn

#### Scenario: 引擎唯一分发点

- **WHEN** 审查 `agent-runtime` 的引擎构造入口（`runner_for` 形状）
- **THEN** `EngineKind` → 引擎构造的 match 唯一，内核组合根仅做参数映射不经手引擎细节；新增引擎只动门面一处

### Requirement: 会话域内核公共 API 与会话一等公民

`core/agent` SHALL 以会话为公共 API 的寻址与生命周期单位，提供两件公共能力（用户愿景原文的契约化）：

- **运行会话**：`run_session(New | Continue{id}, prompt)` 形状的会话建立与轮驱动——`New` 建立新会话，`Continue{id}` 以既有会话续发新一轮；返回 token 级增量流（思考/回复可辨）与可终止句柄。会话过程流式传输、能区分思考和回复、可终止。
- **查询会话**：`query_sessions(...)` 形状的会话记录查询——会话清单（分页信封：`offset` / `limit` 区间 + 过滤集全量 `total`）与聚合统计（提问轮数、累计墙钟、累计 token 消耗（输入 / 输出 / 缓存读取 / 缓存写入，缓存计数为 `TurnDone.usage` 鸭子键求和、输入 token 的子集）；MVP 从轮统计行现算，MUST NOT 维护累计列，字段面可扩展）。

session SHALL 为一等公民：会话 id 由 core 铸造；转录 = 会话全史（密封事件直挂 session，单表），resume 重建源 SHALL 为会话全史，MUST NOT 以单 run 事件拼凑转录。core SHALL 维护双 id 映射（core session id ↔ engine_handle）；engine_handle 经协议到达（spawn 响应或事件上报，见「P1 引擎协议不变量与缺省降级」）。双 id 映射 SHALL 定位为**留痕**（「这轮落在哪个引擎侧会话」的引擎中立观测/考古事实，落库半边保留）；续会话寻址 SHALL 以会话 id + 全史转录重建为准，MUST NOT 以引擎句柄缺失阻断续会话（寻址硬门槛随 CLI 租户退役——`begin()` 不再对缺 `engine_session_id` 的会话报「无法续接」）。会话 SHALL 存引擎/模型/权限档/实例外键的**配置快照**而非跨库引用（`AgentInstanceRecord` 在全局库、session 在 workspace 库，禁跨库引用；实例改名/删除不伤历史会话；`agent_instance_id` 为快照复制值的显式关联键，见 desktop-workspace-store「SessionRecord 模型与会话转录单表」）。会话 SHALL 承载来源归属（`SessionProvenance{source, source_ref}`，作 provenance/持久化记录用），但前端寻址与重放 SHALL 以 **`sessionId` 直查**（`session_detail`）为准，MUST NOT 经 `source` / `source_ref` 反查会话（`sourceRef` 反查取最近一条的路径退役；来源侧——如 explore 记录——持会话 id 并直接给出）。**清单筛选（探测语义）SHALL 被允许**：按 `source` / `source_ref` 列出会话供用户显式挑选（与「调试页历史来源筛选」同款语义），MUST NOT 被解读为「寻址 / 重放」违规——选中后一律以 `sessionId` 直查（`session_detail` + 转录重放）；change 详情通用 agent 面板的 chat 会话清单即此形态（见 desktop-change-chat）。

#### Scenario: 新建会话运行

- **WHEN** 以 `New` 发起一次含多轮工具调用的提问并等待收口
- **THEN** 返回的流内可观测 token 级增量（text / thinking 可辨），轮末以密封 Message 收口，终态以 TurnDone 收敛；句柄可在运行中请求终止并收敛 `stopped`

#### Scenario: 续会话全史重建

- **WHEN** 以 `Continue{id}` 对一个已有两轮往复的会话续发第三轮
- **THEN** 引擎侧重建源为该会话全部前两轮的密封转录（第一轮不丢），第三轮照常收口并追加为本会话新轮

#### Scenario: 会话查询与聚合统计

- **WHEN** 对某 workspace 分页查询会话清单（携 `offset` / `limit` 与来源过滤）
- **THEN** 返回该页会话记录及现算聚合统计（轮数/累计墙钟/累计 token，缺席字段合法缺省）与过滤集全量计数 `total`，无需维护累计列；越界 `offset` 返回空页而 `total` 恒为过滤集全量

#### Scenario: 双 id 映射

- **WHEN** 会话建立后引擎经 spawn 响应或事件上报 engine_handle
- **THEN** core 记录 core session id ↔ engine_handle 双 id 映射（留痕落库）；续轮与恢复以会话 id + 全史转录寻址，映射缺失不阻断续会话

#### Scenario: 清单筛选不改寻址纪律

- **WHEN** 前端按 `source = "change"` 与 `source_ref = <change id>` 列出某 change 的通用 agent 会话清单
- **THEN** 该查询仅为探测 / 选择入口（来源过滤的筛选语义），返回分页信封；用户点选后的装载与续轮仍以 `sessionId` 直查，全链零「按来源反查取最近一条」调用

### Requirement: AgentEvent 信封与 loop 可见性

`core/agent` SHALL 定义统一事件信封 `AgentEvent`，作为引擎共享的逻辑事件模型（非 stdout/线格式直译），词汇为 **delta/sealed 双层**：

- `RunStarted` { model, session_id, tools, mcp_servers, … }（源自 system/init；session_id 即引擎侧标识上报形态之一）
- `MessageDelta` { text/thinking 增量，配对键 }（**新增，ephemeral**）：token 级增量，思考/回复可辨；只上传输面，**store 永不见**；配对键形态（`parent_tool_use_id` vs 显式 correlator）由 dev-design 定稿
- `Message` { role, blocks, parent_tool_use_id }（**密封，durable**）：唯一落库与重放单元；密封粒度 = 引擎一次 assistant 回应（全部块收进恰一条密封 Message）。其中 Block = Text | Thinking{thinking, signature, redacted} | ToolUse{id, name, input} | ToolResult{id, content, is_error} | **WebSearch{id, query, results, error, raw_use, raw_result}**
- `SystemNotice` { subtype, payload }（permission_denied / api_retry 等 system 事件，subtype 不做封闭枚举）
- `TurnDone`（现 `RunResult` 演进，更名/别名由 design 定稿）{ subtype, is_error, num_turns, duration_ms, cost, usage, session_id }：**统计唯一口径**（见「P1 引擎协议不变量与缺省降级」），唯一驱动收敛的变体
- `Raw` { type, 原文 JSON }（未知 / 未来事件类型透传）

块词汇自本条起为**五变体**（provider 服务端工具块入信封，词汇仍协议中立——core 不认识 anthropic / rig）：

- `Text { text, citations: Option<Vec<AgentCitation>> }`——`citations` 为 `#[serde(default)]`（**MUST NOT** 加 `skip_serializing_if`：unified 相位模型无法表达条件省略，加则 bindings 分裂为 `_Serialize` / `_Deserialize`，仓库 golden 契约不允）；`None` = 无引用、序列化落 `null`（键恒在）；旧事件缺该字段读出 `None`，前端既有 fixture 零改动；
- `WebSearch { id, query, results: Vec<WebSearchSource>, error: Option<String>, raw_use: Option<serde_json::Value>, raw_result: Option<serde_json::Value> }`——**一次 `server_tool_use` 与其配对 result 收敛为一块**（`id` 同两侧配对键）；`results` / `error` / `raw_use` / `raw_result` 为 `#[serde(default)]`（键恒在：空数组落 `[]`、缺省落 `null`）；`raw_use` / `raw_result` 为回放唯一事实源（`encrypted_content` 等不可推导字段原样保全）。块语义与投影规则见「web_search 服务端块的事件投影与回放保真」。

信封 SHALL 增轮归属字段 `turn_id: Option<i64>`（与 `seq` 平级，非变体内级）：

- **语义**：值为**持久层权威分配的轮 id**（内核 `begin_turn` 经 `sink.begin_turn` 取得，与轮统计行主键同源），一轮内全部事件（含 `MessageDelta`）同值；`None` 仅出现在无轮归属的构造面（测试夹具 / 手工构造）。该字段与 `TurnDone.session_id`（引擎侧会话标识）语义完全不同，MUST NOT 互相替代或混用命名。
- **为何在信封级**：停止轮 / 失败轮 / 重订中途可能看不到 `RunStarted`，零事件轮更没有任何 `RunStarted` 可供顺序推断；塞进 6 个 `kind` 变体则重复 6 次，且 `MessageDelta` 同样需要轮归属。
- **属性面**：`#[serde(default)]`，**MUST NOT** 加 `skip_serializing_if`——键恒在、值可为 `null`（与 `Text.citations` / `WebSearch.{results,error,raw_use,raw_result}` 同款纪律：条件省略会令 bindings 分裂为 `_Serialize` / `_Deserialize` 相位伴生类型）；出线键为 camelCase `turnId`。
- **为何是 `Option`**：`AgentEvent` 为「线格式 = 落库形态 = 前端 DTO 基准」三位一体，旧转录载荷缺该键 SHALL 读出 `None`（裸 `i64` 会让旧事件**整条解码失败**，不是少一个字段）。
- **生产点唯一**：真实运行事件的盖戳 SHALL 只发生在内核 `drive` 的观察泵单点（`AgentEvent::stamp(Some(turn_id), seq, kind)`，`turn_id` 就在 `RunningTurn` 上）；命令层 / sink / 注册表 / walker MUST NOT 补写该字段——两个实时出口（发起流与订阅流）投递同一个 `AgentEvent` 值，故自动同源同值。
- **引擎零介入**：轮 id 是持久层分配的治理信息，引擎侧契约（`LoopTurn` / `SessionOpen` / 观察通道载荷 `AgentEventKind` / `PumpPayload`）SHALL 零改动、MUST NOT 出现 `turn_id` 概念；盖戳入口 `AgentEvent::stamp` 签名 SHALL 以轮归属为**首参**（`stamp(turn_id, seq, kind)`），使漏写轮归属退化为编译错误而非运行期静默 `null`（MUST NOT 另立第二条生产入口）。
- **无行为面**：本字段 MUST NOT 参与折叠、收敛、轮统计或转录扫描语义（`transcript()` 仍按 `(session_id, seq)`）；前端消费与按轮查询属后续变更。

loop 调试关键信息 SHALL 在一等公民位：tool_use 与 tool_result 成对可见、子代理经 `parent_tool_use_id` 归因、TurnDone 携带统计字段。每事件 SHALL 携带 `seq`（单调序号）、时间戳与 `turn_id`（轮归属；三者同属盖戳治理，收内核）；delta 与密封的 seq 编号关系（共享空间容忍库内空洞 vs 独立编号）由 design 定稿。`Raw` 变体 SHALL 承接所有未识别事件：MUST NOT 因未知 type 丢弃事件或解析失败（永不丢事件、永不炸解析）。

事件 payload SHALL 为自描述 JSON：新增块变体与带默认值的字段**无需 store schema 版本升级**（落库形态与线格式同为信封本体）。已知代价：旧客户端读到含 `WebSearch` 块的事件会在**整条事件解码**时失败（不是少显示一个卡），仓库不提供降级分支（见「SDK 租户 MVP 边界与已知限制」）。

#### Scenario: 六变体归一化

- **WHEN** 以真实流序列（init / text delta / thinking delta / 完整 tool call / system / 轮末收口 / 未知 type）驱动归一化
- **THEN** 分别产出 RunStarted / MessageDelta(text) / MessageDelta(thinking) / Message(ToolUse) / SystemNotice / 密封 Message / Raw，seq 单调递增

#### Scenario: 密封纪律

- **WHEN** 一轮 assistant 回应由 N 个 text delta 与 thinking delta 组成并在轮末收口
- **THEN** 传输面流出 N 个 MessageDelta 与恰一条密封 Message（全部块收进）；store 落库序列中该轮仅出现密封 Message，无任何 delta

#### Scenario: 子代理归因保留

- **WHEN** 事件携带非空 `parent_tool_use_id`（子代理消息）
- **THEN** 归一化后的 Message 保留该字段，消费方可按父工具调用分组

#### Scenario: 未知事件透传

- **WHEN** 输入包含官方新增的未知事件 type 或未知 system subtype
- **THEN** 产出 Raw 变体且原文 JSON 完整保留，解析不报错、不丢行

#### Scenario: 五变体块词汇与信封升级零版本

- **WHEN** 以含 `Text{citations}` 与 `WebSearch{raw_use, raw_result}` 的密封 Message 落库并回读
- **THEN** JSON 往返保真（raw 逐字节不变），store 转录模型版本头不变；以不含 `citations` 的旧 `Text` JSON 反序列化读出 `None`

#### Scenario: 轮归属全事件在场

- **WHEN** 驱动一轮（含 `RunStarted` / `MessageDelta` / 密封 `Message` / `SystemNotice` / `TurnDone`）并检查流出事件
- **THEN** 每枚事件的 `turn_id` 均为该轮 `RunningTurn.turn_id`（`Some(轮 id)`），delta 与密封一致，无一枚为 `None`

#### Scenario: 续轮不串轮

- **WHEN** 对同一会话以 `Continue{id}` 续发第二轮并驱动收口
- **THEN** 第二轮事件的 `turn_id` 为第二轮的新轮 id（与第一轮不同），两轮各自轮内恒定，重叠区间零事件

#### Scenario: 旧转录缺键读出 None

- **WHEN** 以不含 `turnId` 键的旧事件 JSON（存量转录载荷的形态）反序列化为 `AgentEvent`
- **THEN** `turn_id` 读出 `None`，其余字段与旧值逐字段相等，整条事件不因缺键解码失败；同载荷按旧版本落库的行因此无需任何迁移（转录侧口径见 desktop-workspace-store「信封新增字段的零迁移口径」）

#### Scenario: 键恒在且值可为 null

- **WHEN** 以 `turn_id = None` 的 `AgentEvent` 序列化为 JSON
- **THEN** 顶层含 `"turnId": null`（键在场，非省略），`kind` 仍扁平进顶层；源码属性面不含 `skip_serializing_if`

#### Scenario: 引擎零介入与唯一生产点

- **WHEN** 审查 `crates/infra/agent` 引擎侧源码与全仓 `turn_id` 赋值点
- **THEN** 引擎侧（`sdk/**`）零 `turn_id` / `turnId` 字样，`LoopTurn` / `SessionOpen` / 观察通道签名零改动；真实事件的 `turn_id` 只在 `SessionKernel::drive` 的盖戳单点写入一次

### Requirement: P1 引擎协议不变量与缺省降级

`core/agent` 定义、infra 引擎实现的 P1 引擎协议（参考 Actor，不照搬）SHALL 锁定以下四条不变量（机制不规定，通道形状 / 采集方式 / 整合方式由引擎自选）：

1. **流式可实现**：观察词汇含 delta（MessageDelta），通道有界背压；通道形状（mpsc / actor mailbox）引擎自选
2. **统计口径唯一**：TurnDone 字段面（num_turns / duration_ms / usage / cost）= 唯一统计口径，core 累计计算；引擎 MUST NOT 另立第二口径
3. **唯一标识上报**：engine_handle 在 spawn 响应或事件中告知（上报时机二选一），core 记双 id 映射
4. **注入/轮分离**：injections（prompt + tools，会话级）与 question/ctx（轮级）分离；整合方式（preamble / `--append-system-prompt` / 忽略）引擎自选

协议观察词汇与统计事件字段、查询 api 附带信息 SHALL **全部缺省化**（缺席合法）：某些 infra 无法实现时，放弃该统计口径或查询时现算补齐——**降级不违约**，协议 MUST NOT 因缺省拒绝或报错（与 Raw 兜底同哲学）。

#### Scenario: 流式可实现

- **WHEN** 任一引擎实现观察回流
- **THEN** 词汇含 MessageDelta 且通道有界（有界背压可考），引擎可自选 mpsc 或等价通道形状

#### Scenario: 统计口径唯一

- **WHEN** 审查两引擎的统计产出路径
- **THEN** 轮数/时长/usage/cost 仅经 TurnDone 字段面流出、由 core 累计，无引擎私有统计通道

#### Scenario: 唯一标识上报

- **WHEN** 任一引擎建立会话
- **THEN** engine_handle 经 spawn 响应或事件恰一路径到达 core，双 id 映射可考

#### Scenario: 注入与轮分离

- **WHEN** 以会话级 injections（prompt + tools）建立会话后续发多轮 question
- **THEN** 注入面不随轮重复提交，轮级 question/ctx 逐轮送达；引擎整合方式（preamble / flag / 忽略）不影响协议形状

#### Scenario: 缺席不违约

- **WHEN** 某 infra 无法产出 TurnDone 的 cost 字段（如 SDK 无价格表）或缺席某查询附带信息
- **THEN** 字段缺省（None）流出、查询照常返回，协议不拒绝、不报错、不降级为错误

### Requirement: AgentRunner trait 与逻辑事件流抽象

`core/agent` SHALL 以 P1 引擎协议（参考 Actor，不照搬）承载引擎契约，trait 面 SHALL 仅暴露协议词汇与运行参数，进程模型（spawn、stdout/stdin、退出码）MUST NOT 出现在 trait 面上：

- **会话建立**（spawn 语义）：injections（prompt + tools）+ ctx（workspace root、部分 config、可扩展）+ session 引用（New | Continue{id}）
- **轮驱动**：question；**终止**：stop（`RunHandle` 竞态语义保留为默认形态，Stop 消息替代与否由 design 定稿）
- **观察回流**：Delta / Sealed / Notice / Raw / TurnDone（见「AgentEvent 信封与 loop 可见性」）

协议 SHALL 满足 P1 四不变量（流式可实现 / 统计口径唯一 / 唯一标识上报 / 注入轮分离，见「P1 引擎协议不变量与缺省降级」）。事件流 SHALL 以有界通道承载；抽象 SHALL 支持以假引擎（预录事件序列）替换真实引擎供内核与上层测试。多租户（本机 CLI / 进程内 SDK / 远程 API）SHALL 都能落在协议预留内——CLI 泵 stdout 生产流，SDK 进程内泵任务灌流，API 以轮询 / watch 模拟流；当前实现 SDK（rig 进程内）单租户（CLI 租户实现已随 retire-cli-engine 退役、租户位保留），远程 API 租户 MUST NOT 预建实现。协议参数面 MUST NOT 出现 engine / agent 概念——引擎选择住 infra 门面，core 契约零污染。

run 状态机 SHALL 位于 `core/agent`（治理面）：running → completed | failed | stopped，由 TurnDone（含 is_error）驱动收敛为 completed / failed，显式终止请求 SHALL 经独立收敛分支收敛为 `stopped`，收敛后 MUST NOT 再接受状态变更。

#### Scenario: 协议面无进程概念

- **WHEN** 审查引擎协议 trait 与参数 / 事件流类型签名
- **THEN** 无进程句柄、stdout/stdin、子进程退出码等进程模型类型，仅协议词汇与运行参数；且无 engine / agent 选择参数

#### Scenario: 假引擎可替换

- **WHEN** 内核编排测试以预录事件序列的假引擎注入
- **THEN** write-through 持久化 / 状态机收敛 / 会话轮推进全程可测，无需任何真实引擎调用

#### Scenario: 状态机收敛

- **WHEN** 事件流出现 `is_error=true` 的 TurnDone
- **THEN** run 状态收敛为 failed；success 时收敛为 completed；显式终止请求时收敛为 stopped；收敛后不再接受状态变更

#### Scenario: 双引擎协议同构

- **WHEN** SDK 引擎与假引擎（预录序列，未来租户位）各自实现引擎协议并被同一内核编排消费
- **THEN** 内核编排代码无引擎分支（零改动复用）；停止信号对各租户实现同样生效（多租户协议预留可考）

### Requirement: agent_stop 终止与提前 resolve

命令面 SHALL 提供终止入口，发起命令 SHALL 维持提前 resolve 契约：

- 发起命令 SHALL 在会话/轮记录落库并进入 running 后**提前 resolve**，返回 running 态记录（id 立即可知）；执行 SHALL 转入内核编排继续（内核持久面持有 store 收尾），终态轮行 SHALL 经 Channel 以 `AgentRunMessage::Record` 信封（`{ ipc: 'record', record: TurnSummary }`）**收尾流出**，MUST NOT 再依赖 invoke 返回携带终态。后端 SHALL NOT 承诺该终态的**前端呈现载体形状**（部件类型、消息 id、是否独立消息）——前端把该轮行挂为对应回合消息的元数据属前端契约（见 desktop-agent-chat-infra「统一会话状态基建」的「record 归元数据」）。
- 终止命令 SHALL 按运行中的会话活动轮寻址: 将轮状态收敛为受控终态 `stopped`、尽力终止引擎执行体（经停止信号取消泵任务（drop future）——机制由 design 定稿核实；寻址键（run/轮 id 沿用或会话键，root 消解跨库歧义保留）由 design 定稿）、经 Channel 流出 `Record` 信封（`record.status = stopped`）。`stopped` SHALL 与 `completed` / `failed` 同列受控终态（用户主动终止与引擎失败语义区分，收敛后 MUST NOT 再接受状态变更）。停止注册（内存态）收内核治理面。**停止轮 SHALL 恒有轮行**：即便该轮零事件（`loop.rs` 停止路径直接返回、不发 `TurnDone`），内核 `converge` 仍铸 `stopped` 终态行——前端据此在该轮位置呈现「已停止」状态行（见 desktop-agent-chat-infra「轮内状态行与空内容轮可见性」），MUST NOT 表现为「界面只是不再增长」。
- 对非 running 目标（已终态或不存在）的终止 SHALL 幂等忽略，MUST NOT 报错崩溃或误改既有终态。
- 停止后的重放 SHALL 与实时一致: 重放路径从会话转录（含 `stopped` 轮行）还原与实时查看相同的呈现（同一轮分组入口折叠，轮行落同一 `metadata.record`）。

#### Scenario: 提前 resolve

- **WHEN** 前端发起会话运行
- **THEN** invoke 在轮记录落库进入 running 后即 resolve 返回 running 态记录（id 可用），引擎事件随后经 Channel 持续流入，终态以 `Record` 信封经 Channel 收尾

#### Scenario: stop 收敛与引擎终止

- **WHEN** 运行中 invoke 终止命令
- **THEN** 轮状态收敛为 `stopped`，Channel 流出 `Record` 信封（`status = stopped`）；运行的泵任务被取消（事件流终止、future drop），无孤儿泵残留；该轮即便零事件也恒有轮行（`stopped`），前端在该轮位置呈现「已停止」状态行

#### Scenario: 幂等忽略

- **WHEN** 对一条已 `completed` 的轮 invoke 终止命令
- **THEN** 幂等忽略: 状态不变、不报错、不产生新事件

#### Scenario: 停止后重放一致

- **WHEN** 停止一轮运行后重新打开该会话查看
- **THEN** 重放呈现与停止时实时呈现一致（含 `stopped` 轮行挂在该轮 `metadata.record`，同一折叠入口），续话以会话最新状态为准

### Requirement: 引擎门面与 EngineKind 参数选择

`agent-runtime` SHALL 作为 agent 引擎门面：对外一套应用协议（invoke 面 + 事件流 + 会话记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎（值域保留为多租户协议预留，`Cli` 为退役态）；协议统一 SHALL 落在内核协议层而非各引擎私有形态。门面 SHALL 提供 `runner_for(kind, engine_cfg)` 形状的构造入口——`EngineConfig` 结构体形态为**连接配置字段面**（`protocol` + api_key / base_url / model_high / model_low：协议决定 wire 语义故在此面内，可空调参 `context_length` / `max_tokens` / **`web_search`** / **`efforts`** MUST NOT 进本结构、一律走门面旁路 builder）由组合根注入，SDK 引擎用以组装 rig client；`EngineKind::Cli` 臂 SHALL 退役为显式 `Err("CLI 租户已下线")` 形态（runner 构造失败面，具体形态——`Result` 包装或等价——由 design 定稿），**唯一 match 点结构与 `EngineKind` 双值 MUST NOT 塌**（多租户机制保留，未来本机 CLI / 远程 API 租户仍落协议预留；模型协议 MUST NOT 编码进 `EngineKind`）。**解析单点自命令层下沉至内核组合根**（desktop-agent-management「默认 agent 与运行发起解析」的落点平移，解析语义不变）：缺省路径解析默认 agent、显式路径按所选 agent 解析、产物为 `EngineKind` + `EngineConfig`（sdk 时由引用 provider 组装、model 取 provider 三档 high 档、协议取 provider `protocol`）；解析遇 `engine: Cli` 实例（缺省与显式路径同）SHALL 显式 `Err("CLI 租户已下线")`，MUST NOT 静默回退。换解析落点时 `EngineConfig` 消费面 MUST NOT 变化。启动失败抹平：`AgentStartError` 的中性变体 `ConfigMissing` 承接 SDK 侧连接配置不齐等启动失败——core 唯一触碰点，变体集 MUST NOT 再增。

门面旁路载荷 SHALL 为四项且同型（`with_context_window` / `with_max_tokens` / `with_web_search` / `with_efforts` 四个 builder）：`web_search` 的语义为「**已按协议门控**的最终开关」（门控判定在组合根解析期完成，`provider.web_search && protocol == Anthropic`；门面与 runner MUST NOT 二次判定协议）；`efforts` 的语义为「**未 shaping 的 provider 原值对**」（两协议原样带出，协议 shaping 收在 runner 的 `sdk/effort.rs` 单点——门面 MUST NOT 做协议分支）。

#### Scenario: 门面构造与 core 零污染

- **WHEN** 审查 `runner_for` 实现与 `crates/core/agent` 源码
- **THEN** 引擎分发唯一 match、返回引擎协议实现；core 无 engine / `EngineConfig` / rig / agent / 协议概念（亦无 `web_search` / `efforts` 的协议语义，只有块词汇与值类型），`AgentStartError` 变体集不变

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现与门面签名
- **THEN** 解析语义（默认 agent / 显式 agent / sdk provider 组装 / high 档 / 无默认 `Err` / cli 实例 `Err`）与设计一致，`runner_for` 唯一 match 点结构与连接配置字段面零塌，命令层无引擎解析残留

#### Scenario: cli 解析显式失败

- **WHEN** 解析目标实例 `engine` 为 `cli`（缺省默认实例或显式选择的存量 cli 实例）
- **THEN** 返回 `Err("CLI 租户已下线")`（文案引导管理页），不构造任何引擎、不落库不推流

#### Scenario: 旁路与配置面的分工

- **WHEN** 审查 `EngineConfig` 字段面与门面 builder
- **THEN** `EngineConfig` 仅含连接必需项（`protocol` / `api_key` / `base_url` / `model_high` / `model_low`），`is_complete` 语义不含协议（协议恒有值）；`context_length` / `max_tokens` / `web_search` / `efforts` 经 `with_context_window` / `with_max_tokens` / `with_web_search` / `with_efforts` 旁路注入，MUST NOT 出现在 `EngineConfig` 中

#### Scenario: 门面不二次门控

- **WHEN** 以 `with_web_search(true)` 装配的门面构造 SDK runner（组合根已判定协议为 anthropic）
- **THEN** runner 收到 `web_search = true` 并原样下传 loop，门面与 runner 侧零协议判定；组合根留下的 `openai + true` 记录在解析期已收敛为 `false`，门面不参与纠偏

#### Scenario: efforts 无门控且零 shaping

- **WHEN** 以 `protocol = openai` 与 `protocol = anthropic` 两条 provider 的 `efforts`（各自非空）装配门面并构造 runner
- **THEN** 两条 provider 的 `efforts` 均原样抵达 runner（MUST NOT 因协议被清零或改写）；门面与 runner 字段面存的是 provider 原值对，wire 形状在 `sdk/effort.rs` 才成形

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 协议 SHALL **双值可选且双首公民**：`openai`（openai-compatible 端点——GLM / DeepSeek / Kimi / 自部署）与 `anthropic`（Anthropic Messages 端点及兼容网关）；协议由 provider 记录 `protocol` 声明、经 `EngineConfig.protocol` 抵达引擎（见「SDK 引擎模型协议选臂（openai / anthropic）」）——原「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」的限制在本变更偿还
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层；**工具执行体 SHALL 允许阻塞等待外部输入**（ask 工具的 turn 内应答等待，见「ask 工具协议与阻塞应答通道」），阻塞期间该轮不收敛、不产生额外模型请求
- **流面双层交付**：流内 `Text` / `Reasoning` 片段增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面；rig 0.43 流事件模型为部分生命周期——Start / Arguments / End 为簿记事件不出增量）；轮末 `finish()` 折叠的 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：默认（无会话注入）工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；会话级注入 SHALL 以**名字清单**收窄该面、以专用工具**名字**追加受控工具面（工具定义与准入档唯一真源在 infra 挂载清单，见「会话级工具面注入与 explore 笔记工具」）；**`ask_user_question` 为基线交互工具**——与七工具并列的能力，MUST NOT 进 `TOOL_NAMES`（它不是「名字清单可收窄的七件之一」），其在场与否由 `SessionInjections.ask` 位决定；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求系统提示词 SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections.preamble` 位按来源装配（生产者 = archi 臂与 config 臂），其余五位（`tools` / `finish` / `ask` / `protect` / `before_done`）同样按来源装配（见「会话级工具面注入与 explore 笔记工具」与「自然收敛前的收口前钩子与收敛事件序」；`toolset` 位已随枚举退役）
- **上下文管理**：喂 provider 的请求史 SHALL 经 L1-L3 上下文防线治理（见「SDK 引擎工具质量与单结果上限（L1）」「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）；store 转录全量不变；窗长来源含模型名 `[1m]` 后缀（见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）
- 事件归一化：归一化 SHALL 只消费 rig 抽象事件词汇（`StreamEvent` / `Completion` / `Message`），工具调用映射为 ToolUse 块并与 ToolResult 成对（同 id）；协议差异 MUST NOT 出现在归一化层（openai chat completions 形状 fixture 仅为测试形状，不是运行时协议判定；anthropic 经 Messages wire 产出同一抽象）；`RunStarted` SHALL 各报各的（model 报**裸名**——`[1m]` 后缀已剥离，tools 报本轮实际下发的工具面——默认七工具、注入面含 `finish_step` / `ask_user_question` 见「会话级工具面注入与 explore 笔记工具」）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
- 依赖足迹：workspace 依赖 SHALL 为 `rig` facade 单条目（`=0.43.0` 锁版——升版经显式修订而非静默漂移；`default-features = false` + 显式 features：`reqwest` / `native-tls` / memory feature，reqwest / native-tls 经 facade 转发到 rig-core）；代码 `rig_core::` 路径零残留。0.42 时代的 `rig-core` 双条目解析钉子退役（2026-10-06 修订）：0.43 家族拆出 rig-reqwest / rig-http / rig-tungstenite 等同伴 crate 后双条目只钉得住 rig-core 一个，全家族精确版本由 Cargo.lock 承担

#### Scenario: 事件归一化 fixture

- **WHEN** 以 openai 形状流 fixture（text delta × N、reasoning delta、tool_calls、未知字段、轮末 choice）驱动 normalize 纯函数
- **THEN** 产出 MessageDelta(text/thinking) 序列、Message(ToolUse) 与成对 Message(ToolResult)（同 id）、轮末恰一条密封 Message（块齐）、未知内容 Raw 透传，seq 单调递增；全程不打网络

#### Scenario: 无 CLI 环境可用

- **WHEN** 无本地 claude CLI 的机器上以 sdk 引擎发起运行（手填 EngineConfig）
- **THEN** 运行正常发起并完成完整 loop（CLI 发现路径不被触及）

#### Scenario: facade 依赖足迹

- **WHEN** 审查 workspace 依赖与 `crates/infra/agent` 源码
- **THEN** 依赖为 `rig` facade 单条目（锁版 `=0.43.0`、`default-features = false` + 显式 features），`rig_core::` 路径零残留；恒编译、无 feature gate

#### Scenario: 双协议均可发起

- **WHEN** 分别以 `protocol = openai` 与 `protocol = anthropic` 的 provider 发起运行
- **THEN** 两者都走同一 `sdk/loop.rs` 全链（工具面 / 防线 / 事件词汇一致），仅 wire 与请求参数不同；归一化层与 core 侧无协议分支

#### Scenario: 注入位词面为六位

- **WHEN** 审查 `SessionInjections` 字段面与本条工具面约定
- **THEN** 只出现 `preamble` / `tools` / `finish` / `ask` / `protect` / `before_done` 六位；产品源码（`crates/**` / `src/**`）中 `toolset` 与 `SessionToolset` 零命中

#### Scenario: 阻塞执行体不额外消耗请求

- **WHEN** 模型在一轮内调用 `ask_user_question` 并等待用户应答
- **THEN** 阻塞期间该轮不收敛、不发起新的模型请求；应答回灌后同一轮的下一次模型请求照常发出（`turn_index` 递增一次，`session_id` 不变）

### Requirement: SDK 引擎系统提示词（AGENT.md 注入）

SDK 引擎的系统提示词 SHALL 来自会话级注入位 `SessionInjections.preamble` 的**逐字**值：值在场 → chat_history 首条为**恰一条** `Message::System`（rig 0.43 起 `preamble` 字段退役，以首条 System 承载；不包装、不改写、不追加引擎侧自读内容）；值缺席 → 不注入 System 消息。引擎执行体（`sdk/loop.rs`）MUST NOT 为组装系统提示词读取任何文件（读取函数可仍驻 `sdk/` 模块、由装配层调用，但引擎执行体 MUST NOT 调用它）——`AGENT.md` 的读取与合并 SHALL 发生在装配层（内核组合根的会话首轮），产物为会话系统提示词快照并逐字复用于本会话后续每一轮（拍存 / 读回 / 存量补拍 / 写一次语义见「会话系统提示词快照的拍存与续轮复用」）。

`AGENT.md` SHALL 是快照的**成分之一**，读取口径逐条不变：读取根为会话执行根 `SessionCtx.workspace_root`（首轮一次读取，worktree 场景即分支版）；读取范围严格限于 `AGENT.md` 单份文件，MUST NOT 做 `CLAUDE.md` / `AGENTS.md` 等兜底回退；内容**逐字**（不删改、不重排）并以 `<agent_md path="…">…</agent_md>` 标签包裹；体量上限 32KB（char 边界截断，只作用于该文件自身）、嵌套 `AGENT.md` 不级联；文件缺席 → 快照不含该段（与注入机制上线前的语义一致）。

快照正文的合成次序 SHALL 为：会话 preamble 知识**在前**，`AGENT.md` 段在后，两段之间以空行分隔；**冻结指引常量**作为末段追加。**知识段的生产者三处**：`source = "archi"` 的架构会话知识（见 `desktop-archi-agent`「架构会话的系统知识注入」）、**`source = "change"` 在写权放开时的同一份架构会话知识**（判据 `model_write_lock(provenance) == None`，与 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」同源）、`source = "config"` 的配置协议知识（见 `desktop-config-agent`「配置协议知识的静态 preamble」）；其余来源恒无此段（「会话级系统前导不进 MVP」的裁定对它们继续生效）。合成产物即快照正文，单条 System 口径不变（工具面注入位仍自「会话级工具面注入与 explore 笔记工具」起按 provenance 来源装配，与本条的知识段来源判定同源）：

- **单侧在场**：仅知识在场 → 「知识 + 空行 + 指引常量」；仅 `AGENT.md` 在场 → 「`<agent_md …>` 段 + 空行 + 指引常量」；两者皆无 → 快照正文为空串、不注入 System 消息（指引常量 MUST NOT 单独成条）。
- **指引常量**：SHALL 为装配层静态常量（≤ 512 B、无路径字面量、无 `<>` 占位符），语义三点——系统提示词冻结于会话开始、项目规则真身为 `AGENT.md`、用户提示规则已更新时以 `read` 重读 `AGENT.md`；规则更新走「用户提示 + `read` 重读」通道（落在史尾，见下方场景）。
- **截断口径分立**：`AGENT.md` 的 32KB / char 边界截断口径不变；知识段与指引常量不在该截断逻辑内（体量由生产者侧测试锚定）。
- **来源约束**：注入位只由装配层按 provenance 决定并经快照供给，前端 / 自由字符串 MUST NOT 直接供给；本机制仍属引擎 / 装配侧特性（P2 原则：跨会话持久上下文是 agent 产品特性、非协议义务）。`crates/core/agent` 本变更**零结构改动**（六位字段面与词汇不变，`preamble` 仍为 `Option<String>` 不透明承载，仅字段 doc 语义改写为「会话系统提示词快照正文」）。
- **缓存纪律**：会话内 System 消息 SHALL 逐字恒定——除「本会话首轮拍存」与「存量会话首次补拍」两个时刻外，任何一轮 MUST NOT 因 `AGENT.md` 或二进制内容变化而改变 System 消息。`context` 防线（L2 剪裁 / L3 compaction）改写的是历史消息而非 System 前缀，故剪裁时刻之后的前缀重建属**已承认的既有代价**，不在本条款承诺内。

#### Scenario: 首轮逐字注入与常量末段

- **WHEN** workspace 执行根存在 `AGENT.md`（`source = "debug"`，无知识段）时以 sdk 引擎发起首轮
- **THEN** 请求系统提示词恰一条 System 消息，内容为「`<agent_md path="…">…</agent_md>` 段（内含文件内容逐字，不删改不重排）+ 空行 + 指引常量」；MUST NOT 出现第二条 System 消息

#### Scenario: 缺席维持现状

- **WHEN** 执行根无 `AGENT.md` 且来源无知识段时发起会话
- **THEN** 请求不注入 System 消息（指引常量 MUST NOT 单独成条），请求形状与该注入机制上线前一致（空正文不产生 System 消息）

#### Scenario: 三处生产者的单条 System

- **WHEN** 分别以 `source = "archi"`、`(dev-design, executor)` 的 change 会话、`source = "config"` 发起会话，且执行根存在 `AGENT.md`
- **THEN** 三者 chat_history 都恰有一条 System 消息，内容依次为「架构会话知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」/「架构会话知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」（与 archi 臂同一份知识、逐字相同）/「配置协议知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」；MUST NOT 出现第二条

#### Scenario: implement 臂零知识段

- **WHEN** 以 `(implement, executor)` 的 change 会话发起，执行根存在 `AGENT.md`
- **THEN** System 消息为「`<agent_md …>` 段 + 空行 + 指引常量」（无架构知识段）；工具面仍含 `archi_api` / `archi_drift`

#### Scenario: 会话中修改不改前缀

- **WHEN** 会话运行中途修改 `AGENT.md` 后续发下一轮
- **THEN** 已发轮次不受影响，下一轮也 MUST NOT 改变 System 消息（首轮拍存后冻结）；规则更新走「用户提示 + `read` 重读」通道落在史尾

#### Scenario: 无兜底回退

- **WHEN** 执行根只有 `CLAUDE.md` 或 `AGENTS.md` 而无 `AGENT.md`
- **THEN** 不发生兜底读取，快照不含该段（严格单文件语义，复数形态不在读取源内）

#### Scenario: config 臂同样合为单条 System

- **WHEN** 以 `source = "config"` 发起会话且执行根存在 `AGENT.md`
- **THEN** chat_history 恰有一条 System 消息，内容为「配置协议知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」；MUST NOT 出现第二条 System 消息

#### Scenario: 会话 preamble 与 AGENT.md 合为单条

- **WHEN** 以 `source = "archi"` 发起会话且执行根存在 `AGENT.md`
- **THEN** chat_history 恰有一条 System 消息，内容为「架构会话知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」；MUST NOT 出现第二条 System 消息

#### Scenario: 仅单侧在场

- **WHEN** 分别以 `source = "archi"`（无 `AGENT.md`）与 `source = "debug"`（有 `AGENT.md`）发起会话
- **THEN** 前者 System 消息为「知识 + 空行 + 指引常量」；后者为「`<agent_md …>` 段 + 空行 + 指引常量」；两者皆无时零 System 消息

#### Scenario: 引擎零文件读取

- **WHEN** 审查 `crates/infra/agent/src/sdk/loop.rs` 并跑一轮续会话（注入位给定非空正文）
- **THEN** 源码中 `preamble::load` 与 `AGENT.md` 字样零命中；该轮 System 消息逐字等于注入位给定值（MUST NOT 被任何文件内容覆盖或追写）

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 以**名字清单为唯一单源**（`toolset` 位与 `SessionToolset` 枚举已退役，专用工具不再有第二套注入机制），并自本变更起以 **`protect` 位**承载会话级写入禁区（第三规则来源；`ask` 位承载 ask 装配档位）：

- **core 契约扩展**：`SessionInjections` 形态 SHALL 为 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec>, ask: Option<AskMode>, protect: Option<Vec<SessionProtectRule>>, before_done: Option<BeforeDoneSpec> }`——`tools` 为**本轮下发的工具名字清单**（`None` = infra 内建七件：read / grep / glob / ls / write / edit / bash），专用工具（`create_explore_note` / `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation` / `db_tables` / `db_query` / `propose_config`）以**名字进清单**而不经枚举追加；`finish`（`FinishSpec { EvaluatorChecklist, Decision }`，`Copy + Eq` 闭合枚举）为 `#[serde(default)]` 的收口档位词汇，`ask`（`AskMode { Generic, Flow }`，`Copy + Eq` 闭合枚举，见「ask 工具协议与阻塞应答通道」）为 `#[serde(default)]` 的 ask 装配档位——**`None` 是显式禁用**（该会话拿不到对应工具），非「未实现」，装配层 MUST 逐臂显式置位。**`preamble` 位按来源装配，生产者三处**——`source = "archi"` 的架构会话知识、`source = "change"` 在**写权放开**时的同一份架构会话知识（判据 = `model_write_lock(provenance) == None`，与两写工具下发同集，见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」）、`source = "config"` 的配置协议知识；位的内容由 loop 与 `AGENT.md` 合并为单条 System 消息（见「SDK 引擎系统提示词（AGENT.md 注入）」）；前端与自由字符串 MUST NOT 直接供给该位。**`protect` 位**为 `#[serde(default)]` 的会话级写入禁区清单（`SessionProtectRule { glob: String, reason: String }`，`reason` 携 `%s` / `%t` 占位符），**`None` = 不追加**（语义与装配单点见 `desktop-write-protection`「会话级写入禁区（相位锁）」）；该位 SHALL NOT 派生 `specta::Type`（引擎侧会话词汇不出线 IPC，与 `FinishSpec` / `AskMode` 同款），为此**不进 TS bindings**。`before_done` 位见「自然收敛前的收口前钩子与收敛事件序」。**词汇封闭原则**：core 只承载**名字**、**档位词汇**与**纯数据规则串**，工具定义（description / parameters）与准入档位（tier）唯一真源在 infra 的挂载清单（定义与档位均由挂载清单给定，见本条「tier 档位归属」）；非法工具名 SHALL 在装配层（compose）显式 `Err`（携非法名），MUST NOT 流进执行面。`PartialEq` / `Eq` 派生链 SHALL 完好（`FinishSpec` / `AskMode` 均为闭合枚举、清单为字符串向量、`SessionProtectRule` 为两字符串纯数据——`ToolMount` 一类含 `serde_json::Value` 的结构 MUST NOT 进 core）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分四臂——① `source == "explore"`：`tools = Some([read, grep, glob, ls, edit, bash, create_explore_note, db_tables, db_query])`（`write` 移除）+ `ask = Some(Generic)`（两名只读 DB 工具见 desktop-agent-db-tools）；② `source == "archi"`：`tools = Some([read, grep, glob, ls, write, edit, bash, archi_module, archi_search, archi_api, archi_drift, archi_scaffold, archi_element, archi_relation])`（七内建全量 + **七个** archi 工具名，与 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」的 archi 来源臂单源一致）+ `preamble = Some(架构会话知识)` + `ask = Some(Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析（`source_ref` 取任意值不改变该臂）；③ `source == "change"`：按 `source_ref` 尾三段解析 `(phase, role)` 并查 `(phase, role)` → archi 工具子集矩阵（矩阵单源见 desktop-archi-agent「archi 工具面注入与 (phase, role) 矩阵」），命中行把矩阵勾选的 archi 工具名并入七工具名字清单，**`db_tables` / `db_query` 两名字在矩阵之后恒 push（不分 `phase` / 不分矩阵命中，与 `ask` 位恒 `Flow` 同构）**，`finish` 位按 `role` 置位（`evaluator` → `Some(EvaluatorChecklist)`、`decision` → `Some(Decision)`、其余 `None`），**`ask` 位恒 `Some(Flow)`（不分 role / 不分矩阵命中）**，未命中行与**解析失败（畸形 / 短段 / 未知 phase）**SHALL 恒零 archi 工具（fail-closed），**`preamble` 位 SHALL 为「`model_write_lock(provenance) == None` ⇒ `Some(架构会话知识)`，否则 `None`」**（单一判据、与写权同源，MUST NOT 另立一份矩阵命中判定；知识拼装失败降级 `None`、不影响工具面）；④ `source == "config"`：`tools = Some([read, grep, glob, ls, bash, propose_config])`（**无 `write` / `edit`、无 db 工具**）+ `preamble = Some(配置协议知识)`（生产者与内容见 `desktop-config-agent`「配置协议知识的静态 preamble」）+ `finish = None` + `ask = Some(Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析。其余来源（debug 等，即上述四臂之外者）SHALL 注入 `tools = None`（内建七件）+ `preamble = None` + `finish = None` + **`ask = Some(Generic)`**——MUST NOT 以 `SessionInjections::default()`（其 `ask` 为 `None`）作为兜底装配，否则通用会话静默失去 ask。**四臂的 `protect` 位 SHALL 由装配层单点 `model_write_lock(provenance)` 逐臂置位**（archi 臂与 change 臂的 `(proposal|dev-design, executor)` → `None`；其余一律 `Some(一条)`；规则 glob 由装配层经 `foundation::layout` 派生，`core` 不解释路径语义——判定矩阵归 `desktop-write-protection`）。四臂的每个工具名 SHALL 在返回前对 infra 挂载清单做成员校验（未知名 `Err`）。
- **loop 组装与硬禁用**：definitions SHALL 由名字清单查 infra 挂载清单组装（`definitions(injections, prompt)`），并按两个**互不依赖**的位追加动态挂载项——`ask` 位为 `Some(mode)` 时追加 `ask_user_question`（`ToolTier::Protocol`，执行体 `ToolExec::Ask(mode)`），`finish` 位为 `Some(spec)` 时追加 `finish_step`（`ToolTier::Protocol`，执行体 `ToolExec::FinishStep(spec)`）；两条追加分支 MUST NOT 互相引用（无合取判据——`finish = None` 的会话仍有 ask，`ask = None` 的会话仍有 `finish_step`）。`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从清单移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行。
- **三段逐字沿用**：`create_explore_note(name, content)` 契约、执行体装配缝、tier 档位归属三段逐字沿用（`archi_api` 的矩阵扩面只改下发表，不改档位：五 archi 只读工具仍 `ToolTier::Read`、三档全放行）。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具（含 ask）的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root + ask 注册表句柄 + core session id），sdk loop 经 `LoopTurn` 携带的上下文路由；per-turn 铸造判据 SHALL 为**名字清单包含**（`create_explore_note` 在场即铸笔记上下文；`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation` **任一**在场即铸 archi 上下文；`db_tables` / `db_query` 任一在场即铸 `DbContext`——全局库句柄经 `WorkspaceStores::global_shared()`、workspace 库句柄取会话 workspace root 的库实例，change 会话 cwd 是 worktree 但库身份锚恒主仓 root；`propose_config` 在场即铸 `ConfigContext`——其 `root` 取 **`PumpPayload.workspace_root`（主仓 root）**，MUST NOT 取 exec root / worktree，见 `desktop-config-agent`「`ConfigContext` 铸造与 root 语义」）与**位存在**（`ask` 位为 `Some` 即铸 ask 上下文），MUST NOT 用枚举比较或自由字符串嗅探。`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **tier 档位归属**：`create_explore_note` SHALL 归写面档位（`ToolTier::Write`：Default 拒绝、AcceptEdits / BypassPermissions 放行）；`archi_element` / `archi_relation` SHALL 同归写面档位（`ToolTier::Write`，与 `write` / `edit` 同款——两工具的写盘与保护射程见 `desktop-archi-agent`「archi 写工具面」与 `desktop-write-protection`「会话级写入禁区（相位锁）」）；五个 archi 只读工具（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`）SHALL 归只读面（`ToolTier::Read`：三档全放行）；`db_tables` / `db_query` SHALL 同归只读面（`ToolTier::Read`：三档全放行；无路径参数，沙箱改写面对其恒原样返回，写入保护零介入——契约细则见 desktop-agent-db-tools）；`propose_config` SHALL 同归只读面（`ToolTier::Read`：三档全放行；无路径参数，`tools::input_path` 对其恒 `None`——候选里的任意键名不进沙箱改写面，见 `desktop-config-agent`「`propose_config` 工具契约」）；`finish_step` 与 `ask_user_question` SHALL 归 `ToolTier::Protocol`（三档全放行——询问用户与投递决策不写盘、不执行命令，与权限档正交）。档位准入由 tier 比较单点表达（见「SDK 引擎静态权限与路径沙箱」），MUST NOT 恢复按工具名的常量清单。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note / db_tables / db_query / ask_user_question（无 `write`），`tools` 名字清单逐名一致（不含 ask 名——ask 按位追加，不占名字清单），`ask = Some(Generic)`，`RunStarted.tools` 逐名一致上报该面

#### Scenario: 幻觉 write 调用被拒

- **WHEN** explore 会话中模型发出已从清单移除的 `write` 工具调用
- **THEN** 成员校验拒绝：`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult（与 tool_use 同 id）回灌，run 不中断、不触执行体

#### Scenario: create 原子定名

- **WHEN** 占位记录（`draft-1760000000000`）会话中模型调用 `create_explore_note(name="api-retry", content=<初稿>)`
- **THEN** 磁盘出现 `.dev-team/explores/api-retry.md`（内容 = content），store 记录 `name="api-retry"`（`id` / `session_id` 不变），返回文本含笔记路径；占位名文件全程不存在

#### Scenario: create 拒绝面零半边残留

- **WHEN** 分别以非法 kebab 名、撞已有记录名、目标文件已存在（既有笔记）调用 `create_explore_note`
- **THEN** 均显式 `Err` 回灌（文件已存在臂附路径），磁盘与库内零变化（前置校验先于任何写动作）

#### Scenario: 已定名记录异名拦截

- **WHEN** 已定名记录（`api-retry`，笔记在盘）会话中模型调用 `create_explore_note(name="other-name", …)`
- **THEN** `Err` 附现有名 `api-retry` 与笔记路径，零改动；同名且文件缺失时补建成功（孤儿重建）

#### Scenario: 非 explore 来源仍获 ask

- **WHEN** 以 `source="debug"`（或不传来源）发起会话
- **THEN** definitions 与 `RunStarted.tools` 为内建七工具（含 `write`）+ `ask_user_question`，`finish` 位为 `None`（无 `finish_step`），`ask = Some(Generic)`，`protect = Some(相位锁一条)`（非 white-listed 来源一律挂锁）；成员校验对七工具与 ask 放行，七工具行为与注入机制上线前一致

#### Scenario: change 来源按矩阵注入 archi 工具面

- **WHEN** 以 `source="change"` 且 `source_ref` 为 `<change_id>/dev-design/executor/1` 发起会话
- **THEN** 名字清单为七工具 + `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation`（**七名** archi 工具，矩阵命中行勾选者）+ `db_tables` / `db_query`（名字清单共 16 名）；definitions 为该 16 项 + `ask_user_question`，`RunStarted.tools` 逐名一致上报；`finish` 位为 `None`、`ask = Some(Flow)`、`protect = None`（写权相位不挂锁）、`preamble = Some(架构会话知识)`

#### Scenario: change 臂知识段与写权同源

- **WHEN** 分别以 `<id>/proposal/executor/1`、`<id>/dev-design/executor/1`、`<id>/implement/executor/1`、`<id>/dev-design/evaluator/1`、`<id>/code-review/evaluator/1` 发起会话
- **THEN** 前两者 `preamble` 为架构会话知识（与 archi 臂逐字相同）且 `protect = None`；后三者 `preamble = None`（`implement/executor` 虽有 `archi_api` / `archi_drift`，但写权锁在位 ⇒ 不注入写模型纪律）；五者的 `protect` 与 `preamble` 两列逐格互不矛盾

#### Scenario: 模型写权位逐臂置位

- **WHEN** 分别以 `source="archi"`、`<id>/proposal/executor/1`、`<id>/dev-design/executor/1`、`<id>/dev-design/evaluator/1`、`<id>/implement/executor/1`、`<id>/test-gen/executor/1`、畸形 `source_ref`、`source="explore"`、`source="debug"` 发起会话
- **THEN** 前三者 `protect = None`（写权通路）；后六者 `protect = Some([一条模型目录锁])`（fail-closed 方向）；锁 glob 与拒绝文案归 `desktop-write-protection`

#### Scenario: 矩阵未命中行与畸形 source_ref fail-closed

- **WHEN** 分别以 `<change_id>/test-gen/executor/1`、`<change_id>/implement/executor`（段数不足）与空 `source_ref` 发起 change 会话
- **THEN** 三者均落零 archi 工具（七工具 + `db_tables` / `db_query` 名字清单）、`preamble = None`，不抛装配错误、不回退为全量 archi 工具；`ask` 仍为 `Some(Flow)`（ask 位不随矩阵）

#### Scenario: archi 来源装配面

- **WHEN** 以 `source="archi"`、`source_ref` 为 null 发起会话并审查该轮装配产物
- **THEN** `tools` 名字清单为七工具（含 `write`）+ `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation`（**七个** archi 工具，与 `desktop-archi-agent` 的 archi 来源臂单源一致）、`preamble` 为架构会话知识、`ask = Some(Generic)`；definitions 为该十四项 + `ask_user_question`，`RunStarted.tools` 逐名一致

#### Scenario: config 来源装配面

- **WHEN** 以 `source="config"`、`source_ref` 为 null 发起会话并审查该轮装配产物
- **THEN** `tools` 名字清单为 `read` / `grep` / `glob` / `ls` / `bash` / `propose_config` 六名（**无** `write` / `edit`、无 `db_tables` / `db_query`、无 archi 工具）、`preamble` 为配置协议知识、`finish` 位为 `None`、`ask = Some(Generic)`；definitions 为该六项 + `ask_user_question`，`RunStarted.tools` 逐名一致；该臂不进入 `source_ref` 相位解析（`source_ref` 取任意值不改变下发面）

#### Scenario: Explore 臂零回归

- **WHEN** 以 `source="explore"` 发起会话并审查装配产物
- **THEN** 六工具 + `create_explore_note` + `db_tables` / `db_query` 的名字清单与本次变更前 definitions 的可观察产出逐条一致（成员校验 / 硬禁用 / `RunStarted.tools` 上报三条语义零改写），差异恰为多下发的 `ask_user_question` 与只读 DB 工具两件

#### Scenario: 非法工具名装配层显式拒绝

- **WHEN** 以任何来源构造出含挂载清单外名字的注入面（如 `tools = Some(["read", "fly"])`）
- **THEN** 装配层显式 `Err`（文案携非法名与外来源），会话不建立、不进执行面

#### Scenario: ask 位与 finish 位四组合独立

- **WHEN** 分别以 `(ask, finish)` 的四种组合构造注入面
- **THEN** `(Some(_), Some(_))` 下发两项；`(Some(_), None)` 只下发 `ask_user_question`；`(None, Some(_))` 只下发 `finish_step`；`(None, None)` 两项都不下发——两分支无合取判据，任一缺席不牵连另一项

#### Scenario: DB 工具两臂与另两臂的分野

- **WHEN** 分别以 `source="explore"`、`source="change"`（含 `test-gen/executor` 一类矩阵未命中行与畸形 `source_ref`）、`source="archi"`、`source="config"`、`source="debug"` 发起会话并审查该轮名字清单
- **THEN** 前两臂含 `db_tables` / `db_query`（change 臂不分 `phase` / 不分矩阵命中，与 `ask` 位恒在同理），后三臂不含；未下发两工具的会话调用它们一律走成员校验拒绝（`tool_unavailable` + `is_error`，run 不中断、不触执行体）

### Requirement: SDK 引擎工具质量与单结果上限（L1）

SDK 引擎自建工具面 SHALL 完成两项质量升级与一项统一上限：

- **read 截断与翻页**：read 工具 SHALL 缺省截断 2000 行，截断时 SHALL 在结果尾部留痕（「已截断，可 offset 翻页」形态）并 SHALL 支持 offset / limit 参数取回后续窗口；现行 read 无大小上限的形态 MUST NOT 延续
- **read 大文件预览**：未给 `offset` / `limit` 且文件体量超过 100KB 时，read SHALL 只返回前 100 行并在尾部留痕（明示总行数与「用 offset / limit 分页或改用 grep 定位」）；显式给出 `offset` 或 `limit` 时 SHALL 恒按调用方窗口读取（预览收窄不生效）。工具描述 SHALL 同步声明该语义
- **grep 正则与上下文行**：grep 工具 SHALL 按正则语义匹配（替换现行子串字面匹配，`sdk/tools.rs` 既有形态）并 SHALL 支持上下文行参数（匹配行 ± N 行，缺省 0）；200 条结果上限维持
- **单结果字节上限（防线 L1 层）**：全工具 SHALL 施加统一单结果字节上限（`MAX_RESULT_BYTES` 50KB 量级），超限截断并留痕；bash 工具 stdout/stderr 合并输出同归此层。该上限 MUST 高于 L2 单结果 prune 门槛——L1 是「新结果封顶」、L2 是「老结果淘汰」，上限低于门槛会让 L2 第一层永不命中

工具 schema 演进 SHALL 随工具面定义更新（新增参数进 input schema），事件面零改动（ToolUse / ToolResult 词汇不变）。

#### Scenario: read 截断与翻页

- **WHEN** read 一个超过截断阈值的大文件
- **THEN** 返回前 N 行 + 尾部截断留痕；带 offset 请求返回后续窗口，两段拼接覆盖全文

#### Scenario: read 大文件预览与显式窗口

- **WHEN** 分别以「无 offset / limit 读一个 >100KB 的文件」与「带 offset 或 limit 读同一文件」执行 read
- **THEN** 前者只返回前 100 行 + 预览留痕（含总行数与分页提示），后者按调用方窗口返回且无预览留痕；≤100KB 的文件两形态行为一致（整读或按 2000 行上限截断）

#### Scenario: grep 正则与上下文

- **WHEN** grep 以正则 pattern（如 `foo.*bar`）与上下文行参数执行
- **THEN** 按正则语义匹配并携带匹配行上下文；子串字面匹配行为退役；结果超 200 条仍截断

#### Scenario: 单结果字节上限

- **WHEN** 任一工具单结果超统一字节上限
- **THEN** 结果截断至上限并留痕，ToolResult 事件面不炸、run 不中断

### Requirement: SDK 引擎 bash 工具

SDK 引擎工具面 SHALL 增第七工具 `bash`，`RunStarted.tools` SHALL 报**本轮实际下发的工具面**（无注入会话即七工具；注入面见「会话级工具面注入与 explore 笔记工具」）：

- **档位归属**：Default 档拒绝、AcceptEdits / BypassPermissions 档放行（见「SDK 引擎静态权限与路径沙箱」的三档分化）；拒绝形态与 write/edit 同构（`SystemNotice{subtype:"permission_denied"}` + is_error=true ToolResult，run 不中断）
- **进程执行**：`tokio::process`，cwd = workspace root，环境继承当前进程；shell 底座 Windows SHALL 优先探测 git-bash（unix 语法对模型成功率最高），探测失败退 `cmd /C` 兜底（不损开箱即跑）；unix 侧 `sh -c`
- **停止语义**：`kill_on_drop(true)` + Windows 进程树清理沿 `taskkill /T /F` 形状（原 `cli/runner.rs` `kill_process_tree` 同形，形状随 retire-cli-engine 迁驻 `sdk/bash.rs` 唯一持有；future drop 不自动杀子进程树）
- **超时**：缺省 120s；显式值须在 [1s, 30min] 内，越界显式拒绝（不静默钳位）——活性护栏，非安全护栏
- **调用意图参数 `intent`（定义面）**：`bash` 工具定义（挂载清单单源 `sdk/mount.rs`）SHALL 在 `parameters.properties` 增 `intent`（`type: "string"`）并 SHALL 将其并入 `parameters.required`（与 `command` 平级），使「每次调用都带一行意图」成为模型可见的协议要求；`intent` MUST NOT 声明 `maxLength`（长度是审美约束，加一个无执行体配合的上限只会造成假精确）。取值纪律与推荐长度 SHALL 在 `intent` 的字段 `description`（或工具级 `description`，二者分工单点、MUST NOT 各写一份独立措辞）中表达：一句话说明这条命令要干什么、折叠态用户只读这一行、建议不超过 30 字、写目的而非复述命令文本。其余六个内建工具定义 MUST NOT 变化。
- **调用意图参数 `intent`（入参闸）**：`sdk/bash.rs::execute` SHALL 在 `command` 校验**之后**（`resolve_timeout_ms` 之前）校验 `intent`——缺失 / 非字符串 SHALL `Err`（文案指明缺失或非字符串字段名 `intent`），`trim()` 后为空 SHALL `Err`（文案指明不得为空）；两类 Err 与既有 `command` 缺失 / 空白 Err 同形（执行体硬 `Err`、模型在同一 turn 内自纠重试）。校验序 SHALL 为 `command` 先报——`command` 与 `intent` 同缺时错误必须指向 `command`（既有错误语义与既有断言零漂移）。
- **长度软约束**：`intent` 超长（含远超 30 字的输入）MUST NOT 导致执行体 `Err`、MUST NOT 触发任何拒绝分支——长度只经 `description` 软提示，视觉截断由前端工具卡的省略链承载（见 desktop-agent-chat-infra「bash 工具卡折叠态意图行」）；若后续出现稳定的超长样本，升为硬闸属独立变更。
- **意图的消费边界**：`intent` SHALL 仅随 `ToolUseBlock.input`（`serde_json::Value` 原样）落转录与回放，`core/agent` 事件词汇 MUST NOT 扩展、store 落库形状 MUST NOT 变化（无新列、无编码改动）；执行、写保护裁定、路径沙箱、上下文剪裁 / compaction 全链 MUST NOT 消费 `intent`——保护闸的判定输入仍为 `command` 原文与会话执行根，`tools::input_path` 对 `bash` 仍恒为 `None`。
- **知情边界（desktop-write-protection 部分偿还）**：bash 命令对**受保护文件**的写入面 SHALL 由 loop 守卫链的写入保护闸覆盖（重定向 / 删除 / 移动 / 内联脚本等写法的命令文本提取，见「loop 守卫链写入保护闸与工作区根注入缝」与 desktop-write-protection）；**任意命令执行仍无沙箱**——BypassPermissions 默认档下 bash 仍可执行任意命令、可改动任何未受保护文件；命令级白/黑名单、真沙箱、交互审批留后续 change（边界条款见「SDK 租户 MVP 边界与已知限制」）

#### Scenario: 放行档执行回灌

- **WHEN** AcceptEdits（或 BypassPermissions）档下模型调用 bash 执行命令
- **THEN** 命令执行、ToolResult 含 stdout/stderr 合并输出，bash 事件与其他工具同构进出转录

#### Scenario: Default 档拒绝

- **WHEN** Default 档下模型请求 bash 工具
- **THEN** 拒绝流出（`permission_denied` SystemNotice + is_error=true ToolResult），run 不中断

#### Scenario: 超时与进程树清理

- **WHEN** bash 命令超时，或命令运行中会话被终止
- **THEN** 进程树被清理（`taskkill /T /F` 形状），无孤儿进程残留；超时以活性护栏错误收口，不挂死 run

#### Scenario: Windows 兜底探测

- **WHEN** Windows 环境无 git-bash
- **THEN** 经 `cmd /C` 兜底执行，开箱即跑不破坏；探测与兜底选择可测（探测函数纯化）

#### Scenario: 受保护写入被拒而任意命令仍可执行

- **WHEN** BypassPermissions 档下先执行 `echo {} > .dev-team/config.json`，再执行 `echo {} > notes.txt`
- **THEN** 第一条被保护闸拒绝（`permission_denied`，run 不中断）；第二条正常执行——护栏只覆盖受保护文件面，不构成命令白/黑名单

#### Scenario: 定义面声明意图为必填

- **WHEN** 审查 `bash` 工具定义（挂载清单单源）
- **THEN** `parameters.properties` 含 `intent`（`type: "string"`）且无 `maxLength` 键，`parameters.required` 逐值含 `command` 与 `intent`（`timeout_ms` 仍不在其中）；工具定义文本点出「折叠态只读这一行 / 建议 ≤30 字 / 写目的不复述命令」，且该引导与 `intent` 字段 description 分工单点不重复

#### Scenario: 意图缺失或空白硬拒且命令优先

- **WHEN** 以 `{}`、仅带 `command`、`intent` 为非字符串（数字 / 对象 / `null`）、`intent` 为纯空白串四种输入调用 `bash` 执行体
- **THEN** 四者均以硬 `Err` 收口且文案分别指明 `intent` 缺失 / 非字符串、`intent` 不得为空；其中 `{}` 的报错指向 `command`（校验序命令先报），既有 `command` 缺失 / 空白用例断言零漂移

#### Scenario: 超长意图不拒绝

- **WHEN** 模型传入合规 `command` 与 200 字长 `intent`
- **THEN** 命令照常执行并回灌合并输出，执行体零 `Err`；长度提示仅存在于定义面文本，折叠态截断由前端承载

### Requirement: SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）

SDK 引擎 SHALL 对**喂 provider 的请求史**（非 store 转录）实施两级防线，整体水位视角：L1 单结果上限恒常（归「SDK 引擎工具质量与单结果上限（L1）」管辖）→ L2 请求前确定性剪裁（约 70-75% 水位）→ L3 LLM compaction（约 85-90% 水位）→ L4 = 现状 provider 报错收敛兜底，**防线设计目标为 L4 永不抵达**。

- **窗长来源**：优先级 SHALL 为「该档模型名 `[1m]` 后缀 > provider 可空 `context_length` 列（desktop-agent-management）> 128K 缺省启发式」，且 SHALL 按档（`high` / `low`）独立解析（见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）；后缀在场时 `context_length` 被覆盖（MUST NOT 叠加或取小值）；同一标称窗长 SHALL 同时供防线装配与输出上限分档推导（单点取值，MUST NOT 造第二条解析链）；`EngineFacade::with_context_window` 签名不变（语义降为「缺省来源」）；token 估算 SHALL 用 UTF-8 字节启发式（rig-memory `HeuristicTokenCounter` 同款，中文重载比英文经验值准）
- **防线窗长折算（成本护栏）**：水位推导 SHALL 使用**生效窗长** = 标称窗长 × 60% 与 400k 取小——模型标称窗长再大，L2 / L3 水位 MUST NOT 随之抬到 1M。理由是请求史每轮整段重发，上下文体积是缓存命中 token 量的唯一乘数；`[1m]` 后缀自此只影响 anthropic beta 头与请求可达上限，MUST NOT 同时把防线水位抬到防线永不触发。泵启动日志的 `window=` SHALL 报生效窗长（调用方不重算）
- **L2 确定性剪裁**（请求前跑，两个调用点：resume 转录重建之后 + loop 每次发请求前）：先 prune 老工具结果——老 tool_result 内容替换为占位符，tool_use / tool_result 配对结构完整，保护最近窗口（40k 保护窗 / 8k prune 门槛）；不够再丢最老完整轮对（rig-memory 算法 + 孤儿配对清理兜底），直到回落至**目标水位**（触发位的 60%，迟滞带宽 = 触发位的 40%）。只剪到「刚压线」会让数个请求后再次触发，而每次剪裁都从首轮对起改写历史、缓存前缀断崖导致整段未命中——迟滞把剪裁频率压到百请求量级。**保护窗 + 首条 user 为地板**（目标不可达时止于地板；小窗长档目标位可能低于保护窗，迟滞带宽归零属既有取舍）。**SHALL 保首条 user**（phase agent 首条 user 即任务书）
- **L3 LLM compaction**：剪裁后水位仍升 → 以当前 model 自摘要老历史，新史 = `[摘要, 首条 user, 最近轮对]`；摘要要点集含关键技术决策**及其原因**（参考 opencode）；compaction 失败 SHALL 降级为 L2 硬裁 + SystemNotice 留痕，**MUST NOT 打断 run**（phase agent 场景宁可丢密度不能丢收敛，比 codex 指数退避保守）
- **可观测**：剪裁与压缩 SHALL 发 `SystemNotice{subtype:"context_pruned"|"context_compacted", payload:{before, after, target, layer}}`（subtype 开放词典现成；`target` 仅 L2 携带）。`TurnDone.usage` SHALL 附**逐请求样本** `requests: [{turn_index, defense, …usage 计数字段}]`（`defense ∈ none|pruned|compacted`，标记该请求前防线是否改写历史）——命中率断崖可直接归因到剪裁 / 压缩，不必靠轮级聚合反推；累计计数字段面不变（会话统计仍按顶层键鸭子求和），无用量报告的轮不挂 `requests` 键
- **不变量**：store 转录**永远全量**——剪裁/压缩只作用于请求史，重放 / 审计 / resume 重建源不受影响；「转录无上限增长」既有边界留痕不变
- **极限场景留痕**：首条 user 单块自身超窗（巨型 phase prompt + 小窗模型）与单轮即超窗，L1-L3 不覆盖，归 L4 兜底 + 错误细分

#### Scenario: prune 老工具结果

- **WHEN** 历史含多轮大型工具结果且估算水位过 L2 阈值
- **THEN** 发请求前老 tool_result 被替换为占位符（配对完整、最近窗口保护、首条 user 在场），`SystemNotice{context_pruned}` 流出，store 转录仍全量

#### Scenario: 过 L2 一次剪到目标水位（迟滞）

- **WHEN** 历史估算水位过 L2 触发位（窗长 75%）且占位符替换后仍过目标位
- **THEN** 丢最老完整轮对直到回落至**目标水位**（触发位的 60%），而非停在触发位下沿——迟滞带宽避免后续请求反复触发剪裁、每次整段重读缓存前缀；保护窗 + 首条 user 仍为地板（目标不可达时止于地板），`context_pruned` 载荷携 `target`

#### Scenario: compaction 摘要收窄

- **WHEN** prune 后水位仍过 L3 阈值
- **THEN** 老历史被摘要替换，新史 = [摘要, 首条 user, 最近轮对]，`SystemNotice{context_compacted}` 流出，run 继续

#### Scenario: compaction 失败降级

- **WHEN** compaction 摘要调用失败
- **THEN** 降级为 L2 硬裁 + SystemNotice 留痕，run 不中断、不失败收敛

#### Scenario: resume 重建后立即剪裁

- **WHEN** resume 转录重建出的历史即超 L2 水位
- **THEN** 重建后、首请求前执行一次 L2 剪裁（两个调用点之一可考）

#### Scenario: 窗长缺省启发式

- **WHEN** provider 记录 `context_length` 缺席且该档模型名不带 `[1m]` 后缀
- **THEN** 以 128K 启发式作为标称窗长、按 60% 折算出生效窗长后计算水位；填列后按填列值折算

#### Scenario: 后缀覆盖填列窗长

- **WHEN** 该档模型名带 `[1m]` 后缀且 provider 同时填有 `context_length = 200000`
- **THEN** 该档标称窗长按 1M 取（后缀覆盖，不做叠加或取小值），生效窗长经 60% × 400k 封顶为 400k；其余档按各自解析结果折算；store 转录与防线算法零变化

#### Scenario: 大窗长不抬水位

- **WHEN** 该档模型名带 `[1m]` 后缀（标称窗 1M）而请求史约 500k tokens
- **THEN** 防线按生效窗长 400k 推导水位（L2 ≈ 307k = 409,600 × 75%），该史已过线并被剪裁——1M 标称窗 MUST NOT 使防线在全库典型上下文体积下永不触发

### Requirement: SDK 引擎静态权限与路径沙箱

SDK 引擎 SHALL 以静态权限三档起步（无审批反向通道——事件流外的应答协议不进 MVP）：Default 档工具面 = 只读（read / grep / glob / ls），write / edit / bash 拒绝；AcceptEdits 档 = + write / edit / bash；BypassPermissions 档 = 全放行。bash 档位语义（Default 拒绝、AcceptEdits / BypassPermissions 放行）为现有三档对第七工具的自然延伸——只读档不给执行权；**AcceptEdits 与 BypassPermissions 自此首次真实分化**（此前两臂行为完全一致）。默认档 SHALL 为 BypassPermissions（与调试页既有默认一致——本机自有 repo 的调试场景以完整循环为默认）。拒绝 SHALL 经 `SystemNotice{subtype:"permission_denied"}` + is_error=true 的 ToolResult 流出（与 CLI `-p` 无头 Default 档行为同构）。policy SHALL 为纯决策表（档位 × 工具名 → 允许 / 拒绝）密集测试；交互审批协议（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）MUST NOT 实现（留独立 change）。

路径沙箱：write / edit / read 工具全链 SHALL 经 canonicalize 后做 workspace root 前缀校验（拦 `..\` 相对逃逸与符号链接绕过 root 外目标），校验 SHALL 为纯函数并可密集测试。BypassPermissions 默认档下路径沙箱 SHALL 是模型与 workspace root **之外**任意文件写之间的唯一护栏，其实现质量为必过关卡；workspace root **之内**的文件写另有第二道独立闸——写入保护闸（受保护文件面，见「loop 守卫链写入保护闸与工作区根注入缝」与 desktop-write-protection），二者射程不重叠：沙箱管「能否越出 root」，保护闸管「是否是受保护文件」。bash 进工具面后，**命令执行本身不在路径沙箱射程内**——路径沙箱不过问命令内容，作用于受保护文件的写入由保护闸经命令文本提取覆盖（含重定向 / 删除 / 移动 / 内联脚本）；未受保护文件的改动仍无护栏，真沙箱（job object / 容器）与交互审批留后续 change；命令级白/黑名单 MUST NOT 作为安全手段（假安全）。

#### Scenario: 三档决策表

- **WHEN** 以三档 × 工具名矩阵驱动 policy 纯函数
- **THEN** Default 档对 write/edit/bash 拒绝、AcceptEdits 档放行 write/edit/bash、BypassPermissions 档全放行，逐格断言通过

#### Scenario: AcceptEdits 与 Bypass 分化

- **WHEN** 以 AcceptEdits 档与 Default 档分别驱动 bash 工具名
- **THEN** AcceptEdits 放行、Default 拒绝——两档对同一工具首次给出不同决策，既有「两臂一致」行为退役（分化由锁定测试钉住）

#### Scenario: 拒绝流出形态

- **WHEN** Default 档下模型请求 write 工具
- **THEN** 事件流含 `SystemNotice{subtype:"permission_denied"}` 与 is_error=true 的 ToolResult（与 tool_use 同 id），run 不中断

#### Scenario: 逃逸拦截

- **WHEN** 工具入参路径含 `..\` 相对逃逸，或为符号链接指向 workspace root 外目标
- **THEN** canonicalize + 前缀校验拒绝执行（tempdir 驱动密集用例全拦）；root 内合法路径不受影响

#### Scenario: 两道闸并存不互相替代

- **WHEN** AcceptEdits 档下 root 内普通文件（未受保护）被写，以及 root 内受保护文件被写
- **THEN** 前者经沙箱放行后正常执行；后者沙箱放行、保护闸拒绝——沙箱不承担保护职责，保护闸不承担逃逸职责

### Requirement: SDK 引擎 resume（store 即会话）

SDK 引擎 MUST NOT 自建会话存储：resume 时 SHALL 从**会话全史转录**（直挂 session 的密封事件单表）重建对话历史喂 rig。重建源 SHALL 为该会话全部前轮的密封转录，MUST NOT 仅取最新单 run 事件拼凑（链式逐轮丢上下文根因修复）。重建口径：仅取顶层（`parent_tool_use_id = None`）非 Raw 密封事件（Raw / RunStarted / SystemNotice / delta 不进对话史，子代理事件压平；保真度缺口留痕不变）。引用不存在或非 SDK 产出的会话 SHALL 以 `AgentStartError` 中性变体显式启动失败，MUST NOT 静默当新会话。

#### Scenario: 多轮全史重建

- **WHEN** 以含三轮 user / assistant(tool_use)/tool_result 往复的会话转录 fixture 驱动重建
- **THEN** 重建出覆盖全部三轮的等价对话历史序列喂 rig（tool 往返保持成对），Raw 件与子代理消息不进入重建序列

#### Scenario: 链式续会话不丢首轮

- **WHEN** 会话已含第一、二轮，第三轮以 `Continue{id}` 续发
- **THEN** 重建史含第一轮内容（MUST NOT 出现只含第二轮的拼凑转录），provider 收到完整上下文

#### Scenario: 会话缺失显式失败

- **WHEN** 以不存在的 session id 调 sdk 引擎 resume
- **THEN** 启动阶段返回 `AgentStartError` 中性变体（`Err` 抵达前端），不产生轮记录、不静默降级为新会话

### Requirement: run 事件落库与重放（workspace 维度）

agent 会话持久化 SHALL 以 **workspace 维度**落盘于所属 workspace 的独立 db 文件（落位与寻址见 desktop-workspace-store），MUST NOT 落全局库。持久化纪律（write-through）：

- **密封唯一 durable**：store sink 只落密封类事件（Message / TurnDone / RunStarted / SystemNotice / Raw）；`MessageDelta` 永不落库（redb 不吃每 token 一行，碎事件根因的落库半边修复）
- **write-through**：内核收到密封事件/返回值即 store 原子操作落盘；落库失败 SHALL 显式失败收敛（failed 记因、尽力流出 `Record` 终态轮行），MUST NOT 静默
- **转录挂 session**：密封事件直挂 session（转录单表，键位形态由 design 定稿），run 退化为轮统计行（挂 session 外键）；转录 = 会话全史 = 重放与重建唯一来源
- **对账纠偏**：查询 api 对账重导（从转录重建统计/索引）保留为纠偏机制，非主路径

事件投递 SHALL 为「持久面 + 实时面」两面：内核持久面（write-through 单点落库）+ 会话实时面（同一事件源的两个出口——发起命令的 `Channel<AgentRunMessage>` 与会话订阅命令 `agent_session_watch` 的 `Channel<AgentRunMessage>`，同源同构、同一信封）；delta 只走实时面（`messageDelta` ephemeral，永不落库），密封事件两面同源同戳。历史查看 SHALL 走所属 workspace 库的会话查询重放（携 root），MUST NOT 依赖全局事件广播或要求运行进程存活。MVP MUST NOT 实现留存清理（转录无上限增长为已知限制留痕）。

轮归属 SHALL 为**消费侧的轮分组唯一依据**：每条事件在信封顶层携带 `turnId`（生产点唯一，见「AgentEvent 信封与 loop 可见性」），前端折叠与实时路 SHALL 以 `turnId` 分组；`RunStarted` MUST NOT 被用作轮边界或轮计数依据——停止轮 / 失败轮 / 重订中途可能看不到它，零事件轮（ask 送达失败）连一枚事件都没有，按它推断会令后续轮整体错位。`TurnDone` SHALL 只用于该轮轮行缺席时兜底 status（`isError ? failed : completed`），MUST NOT 用于轮边界或轮行配对。`turnId` 为 `null` 的旧转录 SHALL 按 `RunStarted` 出现顺序分组（等价既有行为），MUST NOT 因此新增数据迁移或第二套分组逻辑。后端对前端折叠方式零契约要求（信封字段与轮行字段均已在前置变更定稿，本变更零后端改动）。

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

#### Scenario: 轮归属可自证而不靠 RunStarted 计序

- **WHEN** 一条会话转录含三轮：第 1 轮正常收口（有 `RunStarted` 与 `TurnDone`）、第 2 轮用户运行中停止（停止路径不发 `TurnDone`、无 `turnDone` 事件）、第 3 轮正常收口
- **THEN** 每枚事件的信封 `turnId` 各自指向所属轮，消费方按 `turnId` 分组后三轮的轮行与事件一一对位（第 2 轮不缺席、第 3 轮不错位）；以 `RunStarted` 出现次数计序会得到错位结果，故该口径 MUST NOT 被采用；第 2 轮的 status 由轮行（`stopped`）而非 `TurnDone` 支撑

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

### Requirement: session_detail 单查面

会话查询契约 SHALL 增加按 session id 的单查面：core `SessionQuery` trait SHALL 增加单查方法（按 id 返回会话行 + 聚合统计 + 轮行清单），infra 实现以 store `find_session` 为底座（会话行与轮行为既有表；本变更另读快照表以携带系统提示词快照面，见下），轮行的运行状态 SHALL 自轮行清单推导（存在 running 轮行即 running，否则取终态）。命令面 SHALL 提供薄包装命令 `session_detail(root, session_id)`（经 builder 注册、bindings 再生成；命令体三件事纪律）。语义约定：不存在 session id SHALL 显式 `Err`（单查语义与清单空态区分，查无此 id 视为调用方错误）；blank root SHALL 返回空结果（与 `agent_sessions` / `agent_session_transcript` 同口径）；单查 SHALL 按 root 解析所属 workspace 库，跨库隔离语义与既有查询命令一致。既有 `agent_session_transcript` 的签名与语义 MUST NOT 收窄或变更；`agent_sessions` SHALL 为分页查询面——`offset` / `limit` 入参、随页返回 `SessionPage { sessions, total }` 分页信封（`total` 为过滤集全量计数、越界 offset 空页恒 total、limit 超上限截断），来源过滤与 `updated_at` 降序稳定序语义不变。

单查 SHALL 另携**会话系统提示词快照面**：`SessionSummary.preamble: SessionPreambleInfo | null`，`SessionPreambleInfo` 字段面为 `{ savedAt, content }`（`savedAt` = 拍存时刻 UTC unix 毫秒；`content` = 快照正文全文，可能为空串）。约束：

- **单查独携**：无快照的会话（存量会话、从未开轮的空会话）SHALL 返回 `null`，MUST NOT 以空正文伪造快照；
- **清单恒 `null` 且零读**：`agent_sessions` / `list_change_chat_sessions` / `list_archi_chat_sessions` / `list_config_chat_sessions` 与内核 `SessionQuery::list_sessions` 的返回项 SHALL 恒 `preamble = null`，且 MUST NOT 读取快照表（会话清单的全表扫描不因 37KB 级载荷膨胀，页宽 × 快照体量不进外线载荷）；
- **只读**：`preamble` 面 MUST NOT 引入写命令 / 编辑面（写一次与不可变见「会话系统提示词快照的拍存与续轮复用」）；`SessionRow` 字段面零变更。

#### Scenario: 单查返回会话行与轮行

- **WHEN** 以已存在（含 running 轮行与已终态两种形态）的 session id invoke `session_detail`
- **THEN** 返回该会话行、聚合统计与轮行清单，running 判定自轮行推导；结果仅来自该 root 所属 workspace 库

#### Scenario: 查无此 id 显式失败

- **WHEN** 以不存在的 session id invoke `session_detail`
- **THEN** 前端收到 `Err(String)`，无静默空态、无新会话或记录产生

#### Scenario: blank root 空结果

- **WHEN** 以空白 root invoke `session_detail`
- **THEN** 返回空结果不进入库解析链路，与既有查询命令口径一致

#### Scenario: 单查携快照面

- **WHEN** 对一条已拍快照的会话 invoke `session_detail`
- **THEN** `preamble` 非空且含 `savedAt` 与 `content`（`content` 为该会话快照正文全文，与首轮 System 消息逐字相同）

#### Scenario: 清单路径不带快照载荷

- **WHEN** invoke `agent_sessions`（或任一同源会话清单命令）
- **THEN** 返回项 `preamble` 恒为 `null`，且实现未读取快照表（清单载荷与扫描成本零增长）

#### Scenario: 无快照会话返回 null

- **WHEN** 对一条存量（本变更前创建、尚未续轮补拍）会话 invoke `session_detail`
- **THEN** `preamble` 为 `null`；不产生快照行、不现算正文（快照只在会话续轮时拍存）

### Requirement: agent 执行命令面

`commands/exec/` SHALL 承载执行与查询命令，命令体 SHALL 收敛为内核公共 API 的薄包装（三件事纪律：参数转换 → 调用 → 错误映射）：

- 发起命令（现 `agent_start`）：参数转换后调用内核 `run_session` 形状 API——`New` 建立会话、`Continue{id}` 续轮（现 `resume_session_id` 语义平移）；编排（runner 组装、tee、落库、盖戳、收敛）SHALL 全部下沉内核，命令体 MUST NOT 编排。来源参数（`source` / `source_ref`）SHALL 写入会话归属（provenance 记录用）；`source` 不传时缺省 `debug`；`source_ref` SHALL 可携来源侧引用（如 change 详情通用 agent 面板传 change id——两段形态与 change 流程会话的四段 `{changeId}/{phase}/{role}/{attempt}` 精确匹配下不撞），仅用于归属记录与清单筛选，MUST NOT 用于寻址 / 重放（寻址仍走 `sessionId` 直查）；未传时为 null。cwd SHALL 由尾部可选参数 `cwd: Option<String>` 承载（位置索引 8，`None` → `root`）：`cwd` 只赋会话执行根（`SessionCtx.workspace_root`——工具路径沙箱根 / bash `current_dir` / 每轮 `AGENT.md` 读取根），`root` 的 store 身份半边 SHALL 恒为 workspace root（双 root 不变量，与 change 流程 `exec_root` / `store_root` 拆分同构——`cwd` MUST NOT 参与 `for_root`，否则会静默铸出第二份 workspace 库）；`cwd` 非空 SHALL 先做 `is_dir` 探测并以显式 `Err`（携路径）拒绝缺失目录，MUST NOT 让失败深埋进引擎工具调用；cwd MUST NOT 落会话配置快照（续话由来源侧恒显式携带；非来源侧续话不携 cwd 为已知缺口，见 desktop-change-chat）；引擎组装 SHALL 经内核组合根（见「引擎门面与 EngineKind 参数选择」），命令体 MUST NOT 直接构造具体引擎；装配 SHALL 把壳层托管的 ask 注册表句柄注入组合根（见「ask 工具协议与阻塞应答通道」）。命令命名与参数面演进由 design 定稿（保留语义升级或更名会话域命令二选一，见提案待决问题）
- 查询命令（现 `agent_runs` / `agent_run_events` / `agent_run_chain`）：演进为会话域查询薄包装（分页会话清单含聚合统计与 total 计数 / 会话转录重放 / 源过滤清单），参数含 root → workspace 库解析 → store/内核查询 → DTO；前端寻址与重放 SHALL 走 `session_detail` 单查（见「session_detail 单查面」）而非按来源反查取最近一条；旧 run 域命令的退役/收窄路径由 design 定稿；另 SHALL 提供 `list_change_chat_sessions(root, id, offset, limit) -> SessionPage` 薄包装（等价 `session_query(source="change", source_ref=<id>)` 的分页清单——`source` 词常量收在命令层，前端零来源词汇；blank root → 空页；见 desktop-change-chat「change 会话归属与新会话 / 历史会话」）
- 终止命令（现 `agent_stop`）：见「agent_stop 终止与提前 resolve」；root 寻址保留
- **应答命令（新 `agent_answer`）**：签名 SHALL 为 `agent_answer(root: String, session_id: String, text: String) -> Result<(), String>`，命令体为**薄包装**——blank `root` / blank `session_id` SHALL 显式 `Err`（文案携字段名），否则转投壳层托管的 ask 注册表（见「ask 工具协议与阻塞应答通道」），命令体 MUST NOT 触碰会话状态机、MUST NOT 广播通知、MUST NOT 清理流程停等态。该命令的 miss 语义 SHALL 为**显式 `Err`**（「当前无等待中的 ask」）而非幂等 `Ok`——与 `agent_stop` 的 miss 幂等是不同语义：重复应答 / 迟到应答都是用户可感知的「卡已不在」事实，静默成功会掩盖双端不同步。命令 SHALL 落在 `commands/exec/` 轨道并登记于单一命令清单。
- **补充说明命令（新 `agent_steer`）**：签名 SHALL 为 `agent_steer(root: String, session_id: String, text: String) -> Result<(), String>`，命令体为**薄包装**——blank `root` / blank `session_id` / blank `text` SHALL 显式 `Err`（文案携字段名），否则按固定次序做三段（判据次序是硬要求，MUST NOT 合并或调换）：① 问 `AskRegistry::is_pending(session_id)`（有等待中的 ask → `Err`「等待应答中，补充说明不可送达」）；② 投 `SteerRegistry::request`（miss → `Err`「会话已收口，补充说明不可送达」）；③ `ack.await`（对端 drop → `Err`「未送达（会话在补充说明生效前结束）」）。三条 `Err` 文案 SHALL 各自独立（成因可辨，MUST NOT 归一成一句「发送失败」）。命令体 MUST NOT 新开会话或轮、MUST NOT 触碰会话状态机、MUST NOT 广播通知、MUST NOT 排队续跑；命令 SHALL 落在 `commands/exec/` 轨道并登记于单一命令清单。
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

#### Scenario: 应答命令三段纪律

- **WHEN** 审查 `agent_answer` 实现并以登记在案的 session id 调用
- **THEN** 命令体为参数校验 + 注册表转投 + 错误映射三段（零状态机操作、零通知广播、零停等态清理）；等待方收到原文、命令返回 `Ok`

#### Scenario: 应答命令 miss 显式失败

- **WHEN** 分别以 blank root、blank session_id、未登记（或已应答过）的 session id 调用 `agent_answer`
- **THEN** 三者均返回显式 `Err`（blank 臂携字段名、miss 臂文案含「无等待中的 ask」），MUST NOT 幂等 `Ok`

#### Scenario: 补充说明命令三段纪律

- **WHEN** 审查 `agent_steer` 实现并以一条正在跑的会话（无等待中的 ask）调用
- **THEN** 命令体为参数校验 + ask 槽判据 + 注册表投递 + ack 等待四段（零新开会话 / 轮、零状态机操作、零通知广播）；注册表投递成功后命令保持挂起直至 ack 回流，`Ok` 时该文本必已入该轮史

#### Scenario: 补充说明三种不可送达文案各自独立

- **WHEN** 分别以（a）存在等待中 ask 的会话、（b）无在跑轮的 session id、（c）在 ack 回流前已收口的会话调用 `agent_steer`，另以 blank root / blank session_id / blank text 调用
- **THEN** (a) `Err`「等待应答中，补充说明不可送达」、(b) `Err`「会话已收口，补充说明不可送达」、(c) `Err`「未送达（会话在补充说明生效前结束）」、blank 三臂各携字段名；六条 `Err` 文案可辨，MUST NOT 归一为同一句

#### Scenario: 错误 reject 传达

- **WHEN** SDK 引擎配置缺失、解析目标为存量 cli 实例（CLI 租户已下线）、无默认 agent 可解析或所属 workspace 库打开失败时发起运行
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

#### Scenario: cwd 与 store 身份双 root 不变量

- **WHEN** 以 `cwd = <change worktree>`（存在且为目录）与 `root = <workspace root>` invoke 发起命令
- **THEN** 该轮工具路径沙箱根 / bash `current_dir` / `AGENT.md` 读取根取 worktree，而会话行落于 workspace root 所属库（`for_root` 只受 `root` 影响，cwd 路径不派生第二份 workspace 库）；同会话可经 workspace 侧清单查到

#### Scenario: cwd 缺失显式失败

- **WHEN** 以不存在的目录作 `cwd` invoke 发起命令
- **THEN** 发起点返回显式 `Err`（错误串含该路径），不进入引擎、不产生会话行与转录、不留半截状态

#### Scenario: cwd 缺省零语义漂移

- **WHEN** 既有消费面（调试页 / explore 链）不传 `cwd` 发起运行
- **THEN** 执行根回落 `root`，行为与参数面增位前逐字一致（既有位置参数 1–7 语义不变）

#### Scenario: 来源引用归属与清单不撞

- **WHEN** 某 change 既跑过流程（会话 `source_ref` 为四段）又有面板 chat 会话（`source_ref` 为 change id 两段）
- **THEN** 以 `source = "change"` + `source_ref = <change id>` 的清单筛选只命中 chat 会话，四段来源的流程会话一条都不命中；`source_ref` 只用于归属与清单，装载仍以 `sessionId` 直查

### Requirement: Agent 调试页

前端 SHALL 提供 Agent 调试页（`AgentDebugView`），经侧栏「系统工具」组进入；视图切换 SHALL 沿用本地 state，MUST NOT 引入路由。页面 SHALL 包含：

- **参数面（最小集）**：prompt（必填）、permission-mode 三档下拉（默认 bypassPermissions）、agent 选择器（管理页 agent 实例清单，默认选中默认 agent——与组合根缺省解析一致；**仅调试页暴露**，正式场景无 agent 选择入口）；cwd 不设参数（隐含当前 workspace root）、model 不设参数、env 不设参数
- **事件时间线**：SHALL 经会话级统一组件 `AgentSessionView`（desktop-agent-chat-infra，对话透镜 `AgentMessages`）呈现——**运行中 token 级流式**（delta 累积进**该轮唯一 assistant 消息**的当前 step 部件，思考/回复可辨，密封到达追加整块而非另起消息，无碎行、无第二条气泡）、气泡序（按行到达序）、tool_use / tool_result 成对卡；run-started / system-notice / result / raw 等辅助载体逐事件各为独立 system 消息且不进对话呈现（保真原文经下方「原始转录切换」与部件元数据可考）；两引擎共用同一会话组件
- **运行中停止**：运行中 SHALL 呈现停止入口，触发终止命令（见「agent_stop 终止与提前 resolve」）
- **原始转录切换**：落库密封转录（含 Raw）原文可见（与 store 一致）；delta 仅实时面可见，不进入原始转录视图
- **历史会话**：会话列表 → 点开经同一 `AgentSessionView` 重放该会话（含轮行；invoke 查询 + 状态驱动挂流）
- **会话列表终态文案**：列表行的终态标签 SHALL 覆盖轮状态五值（`running`「运行中」/ `completed`「已完成」/ `stopped`「已停止」/ `failed`「失败」/ `interrupted`「已中断」），MUST NOT 把非失败终态落进失败文案——`interrupted` 是进程中断的客观终止，与用户主动停止同属「非故障」侧，词面沿 change 流程 `run-control-panel` 的 `RUN_STATUS_LABEL` 先例

页面状态 SHALL 经统一会话基建（`use-agent-session`）承载；run 表单 / 历史列表 / 转录开关为页面级 chrome，包在共享核心外圈。实时流经 transport 走 Tauri `Channel<AgentRunMessage>`（执行流通道，不属轮询取数）：发送期由发起流承载、历史会话点开（旁观 / 重挂）期由会话订阅面承载；查询类取数（会话列表 / 转录重放）仍由用户显式动作触发。

#### Scenario: 参数面默认值

- **WHEN** 打开调试页
- **THEN** permission-mode 默认 bypassPermissions、agent 选择器默认选中默认 agent（与组合根缺省解析一致）、prompt 为空且必填；无 model / cwd / env / 引擎直选输入

#### Scenario: agent 切换

- **WHEN** agent 选择器选 sdk agent 发起一次含工具调用的运行
- **THEN** 运行走 SDK 引擎（连接配置来自所选 agent 的引用 provider），对话视图 / 落库 / 重放组件零改动复用

#### Scenario: 流式连续呈现

- **WHEN** 一次含长文本回复与思考、且中途调用工具的 SDK 运行进行中
- **THEN** 对话视图呈单条连续增长的 assistant 消息（token 级、思考/回复可辨），第二个响应段的 delta 在同一消息内继续增长（不另起气泡），不出现逐 delta 碎行

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

#### Scenario: 中断轮在会话列表呈已中断

- **WHEN** 会话列表呈现一条最新轮为 `interrupted`（进程中断后经启动标定终结）的会话
- **THEN** 该行终态标签为「已中断」（非「失败」）、带 `data-status="interrupted"` 挂钩；点开后经同一 `AgentSessionView` 重放全史，该轮呈轮内「因应用重启中断」状态行

#### Scenario: 历史会话运行中旁观流式

- **WHEN** 从历史列表点开一条仍在运行的会话（有 running 轮行）
- **THEN** 组件以 `since_seq` 订阅该会话并 token 级流式呈现（不经发起流），轮收口后以重放收敛为与重开一致的形状

#### Scenario: 停止轮可观可见

- **WHEN** 在调试页运行中点击停止，该轮未产出任何 assistant 密封消息（零内容停止）
- **THEN** 时间线在该轮位置呈现「已停止」状态行（不再是「界面只是不再增长」）；顶部失败横幅 MUST NOT 出现（`stopped` 不算失败）；轮行 status 为 `stopped` 可从该轮消息 metadata 读出

### Requirement: 调试页历史来源筛选

Agent 调试页历史区 SHALL 提供会话来源筛选：debug / change / 全部三态，默认 debug（现状不变）。筛选 SHALL 参数化既有取数（`agentSessions(root, source或null, null)`，去硬编码 `'debug'`），切换筛选即重查；取数模型 SHALL 保持显式刷新模式（挂载自动取数 + refresh 重取），MUST NOT 引入轮询、定时刷新或事件订阅。重放链路（点开会话 → 转录重放）SHALL 对三态来源同等可用，复用 `AgentRunHistory` 整套，MUST NOT 为 change 来源另建第二套清单/重放 UI。

#### Scenario: 默认 debug 现状不变

- **WHEN** 打开调试页（未动筛选）
- **THEN** 历史区仅呈现 debug 来源会话，与既有行为一致

#### Scenario: 切换筛选重查

- **WHEN** 筛选切到 change（或全部）
- **THEN** 以对应 source（或不过滤）重查会话清单并呈现 change 会话；再次显式刷新按当前筛选与当前页重取，无自动轮询；翻页为用户显式动作

#### Scenario: 三态来源重放同等可用

- **WHEN** 从筛选结果中点开一条 change 来源会话
- **THEN** 经 `agent_session_transcript` 重放完整转录，与 debug 来源会话同一套重放链路

### Requirement: 引擎泵一轮一命与 headless 无问收口

`agent-runtime` 的 `sdk/` 引擎会话泵任务（`session_pump`）SHALL 服务**恰一个提问**：轮终态产出后泵任务 SHALL 返回（观察通道发送端随之释放 → 内核观测通道关闭）。泵 MUST NOT 回到提问接收等待下一轮（多问循环 MUST NOT 残留）；spawn 失败与停止路径的既有单轮终止语义 SHALL 保持不变。会话史 SHALL 不依赖泵内跨轮回灌（SDK 泵的跨轮 `history_slot` 随一轮一命移除；会话史唯一来源仍是 store 全史转录重建）。

`core/agent` 的 `drive` SHALL 保持「观测通道关闭（引擎侧单边 EOF）→ converge」为**唯一收口缝**，MUST NOT 引入状态机 break、stop select 或第二收口路径；内核测试假 runner 的引擎侧单边 EOF（`close_stream`）即该契约的测试面，真实泵 SHALL 履行同一契约。

消费纪律：任何消费方对同一逻辑会话的每一轮 SHALL 经 `compose_turn → begin → open_session` 新开引擎会话（既有消费现实：chat Continue 与 walker 每步均如此）；泵返回后同一引擎会话通道上的提问送达 SHALL 走内核 ask 失败臂（failed 记因「轮提问送达失败（引擎侧已终止）」），MUST NOT 复活常驻泵。

headless 交互面（本变更修订）：**ask 为阻塞工具，不消耗整轮**——模型需用户决策时 SHALL 调用 `ask_user_question`（工具在 turn 内阻塞、应答经会话级 ask 注册表回流、同一 turn 内继续），MUST NOT 以「问题文本作为本轮最终消息收口 + 应答后 Continue 重开一整轮」作为交互实现；`ask` 位为 `None` 的会话不挂该工具（显式禁用），此时原「问题文本收口 + Continue 续轮」路径仍是该会话的可用退路，但 MUST NOT 被当作 ask 的实现。一轮一命语义不受影响：ask 阻塞发生在轮内工具执行段，不新开轮、不复活泵。

注释口径：kernel / runner 的「逐轮等待提问 / 回到等待下一轮 / 逐轮驱动（ask 语义）」注释 SHALL 更新为一轮一命口径。

补充说明注入（steer）SHALL 同样在**轮内**闭合（通道与 drain 语义见「运行中会话的补充说明注入（steer 通道）」）：注入点为本轮请求边界，泵 MUST NOT 被复活、MUST NOT 收到第二条提问、MUST NOT 新开轮；泵返回（收口）即该会话全部在途补充说明的送达终点——剩余项随接收入端 drop 收 `Err`，MUST NOT 排队到下一轮。

#### Scenario: 泵单轮收口驱动内核 EOF converge

- **WHEN** 引擎泵产出轮终态事件（SDK loop 收敛）
- **THEN** 泵任务返回且观察通道发送端释放（观测通道 `recv` 返回 `None`），内核 `drive` 的观测循环以通道关闭退出并 converge（finish_turn + `TurnFinished` 流出、轮行落库终态、注册表除名）
- **AND** 泵任务不再回到提问接收等待下一轮

#### Scenario: 泵终止后提问按失败收敛

- **WHEN** 同一引擎会话通道在泵返回后再次收到提问送达
- **THEN** 内核 ask 失败臂收敛 failed（记因「轮提问送达失败（引擎侧已终止）」），系统中不存在挂起的常驻泵
- **AND** 消费方每轮经 `compose_turn → begin → open_session` 新开引擎会话（chat Continue 与 walker 每步既有路径零改动）

#### Scenario: 无 ask 禁令残留的组装面

- **WHEN** 审查 SDK 引擎的工具面与命令组装面
- **THEN** 组装面无 disallowedTools 概念（`--disallowedTools` 末位组装规则随 CLI 租户退役，历史 cli 会话转录不受影响）；工具面不再是「封闭清单」——`ask_user_question` 按 `ask` 位下发（见「ask 工具协议与阻塞应答通道」），`RunStarted.tools` 如实上报

#### Scenario: ask 阻塞不改一轮一命

- **WHEN** 一轮内模型调用 `ask_user_question`，用户应答后模型继续并在收口条件满足时结束
- **THEN** 全程恰一个泵任务、恰一个提问（`questions.recv` 一次）、恰一个轮终态；阻塞期间无第二条提问送达、无新会话建立

#### Scenario: 补充说明注入不改一轮一命

- **WHEN** 一轮内用户经 `agent_steer` 注入两条补充说明，模型在后续请求中继续并最终收口
- **THEN** 全程恰一个泵任务、恰一个提问（`questions.recv` 一次）、恰一个轮终态；两条补充说明各以一条 `role = user` 密封事件 + 一条 `Message::user` 进入本轮史（不新开会话、不新开轮）；收口后该会话残留的补充说明一律 `Err`，MUST NOT 触发第二次会话或第二轮

#### Scenario: 无问文本收口与同会话续轮

- **WHEN** 会话以 `ask = None` 装配（该工具不下发）且模型需用户决策
- **THEN** 该轮不发生 ask 类工具调用，模型只能以问题文本作为本轮最终消息收口（`turnDone` 正常 converge）；用户应答后经 Continue（store 全史转录重建）同会话续轮，前轮全史在场——该路径为**显式禁用 ask 的退路**，MUST NOT 被当作 ask 的实现

#### Scenario: 停止与 spawn 失败语义保持

- **WHEN** 泵轮中途收到停止信号，或 spawn 失败（SDK 启动失败）
- **THEN** 既有语义不变：停止路径不合成收敛、终止缝被调、轮收敛 `stopped`；spawn 失败以合成收敛事件 failed 记因且泵终止
- **AND** 两路径均为单轮终止，与一轮一命语义同形，无回归

### Requirement: MVP 边界与已知限制

以下边界 SHALL 作为 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **kill 已实现（偿还「无 kill」限制）**：`agent_stop` 提供终止入口（见「agent_stop 终止与提前 resolve」）；残余限制——bash 工具进程树清理失败时子进程可能残留，应用中途关闭时的孤儿进程仍为已知限制
2. **续会话经转录重建落地**：续会话以 store 全史转录重建为唯一重建源（见「SDK 引擎 resume（store 即会话）」）；`--continue` 隐式续会话语义（CLI 时代）随 CLI 租户退役；调试页自身仍不提供 resume 入口（续话仅 explore 会话链消费）
3. **留存无清理**：事件转录无上限增长
4. **交互工具边界（ask-tool-in-flow 修订，原「无 ask 类工具」限制已偿还）**：`ask_user_question` SHALL 为基线阻塞工具（`ToolTier::Protocol`，`Generic` / `Flow` 两档执行体，见「ask 工具协议与阻塞应答通道」），初始 `Generic` / `Flow` 之外的禁用场景仅以 `SessionInjections.ask = None` 表达且**首期不接线**（无禁用场景）；**已知限制两条**：① 一次 ask 单问题（`question` + 一组 `options`），多问需多次调用，`options` 必填非空（用户可用自由文本作答，但模型必须给选项）；② ask 阻塞期间无超时自动应答、无「稍后作答」的持久化（回答通道活在进程内，应用重启即失，`AskRegistry` 槽由轮收敛除名）——停等态因此仍不落库；`questionnaire` 组件已接线（见 desktop-agent-chat-infra「ask 工具卡活卡与应答动作面」）。
5. **bypassPermissions 默认**：deny 规则仍生效；agent 运行可在 workspace 内无审批改动文件——本裁决限于本机自有 repo 的调试场景
6. **补充说明（steer）的三条边界（steer-running-agent-step）**：① **送达只在请求边界**——drain 点为本轮每次请求发出前，故 ack 的最长等待 = 当前响应剩余时长 + 当前工具执行时长（模型正在跑数分钟的 bash 时，前端 `pending` 会同样持续数分钟，这是 drain 点位置的固有结果而非缺陷）；② **ask 阻塞期间不送达**——模型停在 `execute_ask` 的阻塞臂时够不到 drain 点，该窗口一律 `Err`（「等待应答中」），**不做排队**：用户须先答题再补话；③ **不续跑**——收口后送达一律 `Err`，MUST NOT 以 Continue 重开一轮把补充说明补上（那会污染 step 的「一轮一收口」语义与 evaluator / decision 的末条消息协议）。首期唯一消费点为 change 流程抽屉的 step 会话窗口；其余会话面（explore / archi / 调试）后端通道同样可用，但未接 composer。

#### Scenario: 终止入口可用

- **WHEN** 审查调试页与 explore 页
- **THEN** 运行中提供停止入口且触发 `agent_stop`；进程树清理失败时的孤儿进程残余留痕于本 spec

#### Scenario: 限制留痕可考

- **WHEN** 查阅本 spec
- **THEN** 六条边界均可考（含 ask 的两条已知限制与 steer 的三条边界），后续变更无需重新论证是否知情

#### Scenario: 原「无 ask 类工具」表述零残留

- **WHEN** 审查本 spec 与产品源码
- **THEN** 「工具面为封闭清单，无 ask 类工具」一类的禁令表述不再出现；`ask_user_question` 在挂载清单与 `RunStarted.tools` 中可被观察到

### Requirement: SDK 租户 MVP 边界与已知限制

以下边界 SHALL 作为 SDK 租户 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 MCP**：SDK 引擎不接 MCP 服务器（`RunStarted.mcp_servers` 恒空）
2. **无子代理**：不派生子代理，`parent_tool_use_id` 恒空
3. **无交互审批**：静态三档起步，无审批反向通道（协议留独立 change）
4. **bash 无沙箱（sdk-engine-gap-closure 偿还「bash 缺席」后的知情边界；desktop-write-protection 偿还「受保护文件写入面」半边）**：bash 已进工具面（Default 档拒绝、AcceptEdits / Bypass 放行）；受保护文件（内置工作区配置 + 用户 `write_protection` 规则）的写入经命令文本提取拦截，**但任意命令执行仍无沙箱**——BypassPermissions 默认档下 bash 可执行任意命令、可改动任何未受保护文件；命令级白/黑名单、真沙箱、交互审批留后续 change；已承认的静态判别漏面（`xargs` / `sh -c` 递归拆解 / 生成脚本再执行 / `-m` 模块 / 变量中转 / base64 编码）留痕于 desktop-write-protection
5. **cost 恒缺**：`RunResult.cost_usd` 恒 `None`（无价格表），汇总卡如实呈现
6. **engine_cfg 硬编码位（已偿还，desktop-agent-management）**：api_key / base_url / model 手填真机验证的限制已由全局 agent 管理偿还——engine_cfg 构造源换为管理数据解析（默认 agent / 显式 agent），连接配置经管理页维护，明文边界与遮蔽治理见 desktop-agent-management「api_key 机密边界」
7. **resume 保真度缺口**：Raw 丢弃 / 子代理压平 / 仅顶层重建（L2 请求前剪裁只缓解请求侧成本随历史增长，重建源仍为 store 全量转录）；**服务端块（`WebSearch`）自本条起不再属该缺口**——其 raw 原文经 `raw_use` / `raw_result` 原样保全并原样回放；**`Thinking` 块按凭证回放**：一条 provider 推理载荷一块（`thinking` 展示、`signature` / `redacted` 凭证，互斥；**逐块配对，MUST NOT 拼接**）。带凭证的块按 `reasoning::replay` 还原为 reasoning（issuer `anthropic`），空 `thinking` + 有签名仍是合法可回放形态（显示省略不等于无思考）。回放范围 = **当前 tool loop 一轮**：`loop` 把每轮 choice 原值入史 ⇒ 本轮内续接请求整段重发该史即完成回放；重建史（跨轮）在入口经 `reasoning::strip_all` 整段剔除——跨轮重发的思考块凭证无意义且占体积。剔除先于防线剪裁 ⇒ 水位估算只计实际发出的块
8. **引擎归属留痕未裁**：run 记录暂不区分引擎（座位 design 裁定后偿还）
9. **无模型元数据表（协议与后缀的知情边界）**：不知道任意模型的 `default/upper` 输出上限，也无法判定 `supports_1m_suffix` / `supports_1m_beta`——故 `[1m]` 后缀一律接受（不做模型白名单校验）、`anthropic-beta` 一律发（第三方网关可能拒 → `api_error` 显式暴露，非静默）；名单外模型输出上限**按标称窗长分段推导**（128K 缺省档 `32_000`，与既往兜底逐字相同；封顶 `64_000`），老代模型（8K/16K 上限）与第三方网关仍需用户在 provider 显式填 `max_tokens` 才可跑通
10. **不实现 `maxOutputTokensRecovery`**：撞输出上限即按既有 `finish` 语义收敛（不注入续写消息、不自动抬高上限），用户调 provider `max_tokens` 后重跑
11. **跨协议续会话不做一致性校验**：改 provider `protocol` 后以 `Continue` 续旧会话，旧史由新 wire 重编码——rig 只回放本 dialect issuer 的 reasoning 块；`SessionConfigSnapshot` 不记协议（与「改 base_url / api_key 同权」裁决一致）；system 消息不进持久化史（每轮头部单独注入），故无残留 system 形态问题。**thinking 层业已统一**：请求史本就不含 reasoning 块（`resume::without_reasoning` 单点），故跨协议时无「静默丢弃」的语义漂移
12. **云端 web_search 无过程可见性（知情边界）**：本变更不做搜索实时提示（无 `AgentDelta::Search`，`normalize.rs` 与前端 `agent-transport.ts` 零改动）——`pause_turn` 续跑期间时间线可能空白数十秒，唯一过程观测点为 `SystemNotice{pause_turn_continued}`；最终来源与引用在轮收口后可见（终态以库为唯一权威）
13. **搜索转录体积不设上限、不压缩**：`raw_result` 含 `encrypted_content` 与每条来源加密索引（一次搜索数 KB），与既有「转录无上限增长」边界同列；压缩会破坏 raw 逐字节保真，故明确不做
14. **`web_search` 服务端工具不可配置**：`max_uses = 5` 锁为常量（成本护栏），配置化需 store v5 迁移；openai 协议恒不生效（门控在解析期，不静默透传）
15. **升级后不可回退**：旧客户端读到含 `WebSearch` 块的事件会在**整条事件解码**时失败（非少显示一张卡），仓库不提供降级分支
16. **连续 assistant 消息依赖 Anthropic 服务端合并（`pause_turn` 的知情边界）**：`pause_turn` 把一次搜索拆成两条 assistant 消息（use 在前、result 在后）时，回放按原序重建会产出**相邻同角色消息**；rig 编码侧不做同角色合并，故 SHALL 依赖 Anthropic 服务端接受相邻 assistant 消息。该依赖为**已知限制**，本期 MUST NOT 为其设实现期核验门、MUST NOT 作为任务阻塞项（真机凭据不属自动化验证面，把无法履行的核验写成门只会堵塞实现）；确认动作 SHALL 并入**归档后人工验收**——以官方端点 + 凭据的真实 `pause_turn` 会话续轮确认服务端接受相邻同角色消息，结论记入验收记录，**不阻塞实现与归档**。若人工验收确认被拒，降级预案为「回放时按 `pause_turn` 边界**合并相邻 assistant 消息**」（合并对象 = 该轮重建产出的连续 assistant `AssistantContent` 序列，合为一条消息的序列）——**本期不预建、无实现分支**，后续变更据本条偿还
17. **effort 无 `none`（关思考）入口**：`none` 不属 effort 值域——DeepSeek 的开关是另一条 `thinking.type` 参数，Anthropic 新模型在任意 effort 档传 `thinking:{type:"disabled"}` 即 400；同一字段在两家一个改强度一个改开关必打滑。要关思考属另一条参数族，另立变更
18. **`output_config` 与结构化输出互斥（anthropic 臂）**：rig 的 `AnthropicCompletionRequest.output_config` 是 typed 字段（`OutputConfig{format}`，只由 `output_schema` 填），本仓 effort 经 `additional_params` 的 flatten 再写一份同键——本仓 `output_schema` 恒 `None` 故当前无冲突；**日后启结构化输出 MUST NOT 与 anthropic effort 同时启用**（否则出现重复键 / effort 静默丢），届时须把 effort 合并进同一份 `output_config`
19. **第三方 anthropic 兼容网关可能不认 `output_config.effort`**：失败以既有 `api_error` 显式收敛（MUST NOT 静默重试、MUST NOT 自动去掉参数重发）；留空即不发（默认零风险）
20. **openai 格式思考内容回放未验证**：DeepSeek 的 `reasoning_content` 是否需要/如何回传未在本仓验证——跨轮仍走既有 `reasoning::strip_all`（重建史剔除），同轮内由 rig 序列化 `reasoning_content`；若 provider 拒绝，以 `api_error` 显式暴露，MUST NOT 静默降级为「去掉思考块重发」
21. **输出上限随 effort 静默漂移（openai 协议）**：DeepSeek 思考模式缺省输出 64K、`effort=max` 时 128K，而本仓 openai 协议恒不写 `max_tokens` ⇒ 同一 provider 改 effort 会连带改变实际输出上限，`OutputCap` 截断判据的观感随之变化；该行为记作**已知限制**（anthropic 臂有显式取值链，不受影响）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 二十一条边界均可考（含新增的 `none` 不入值域 / `output_config` 与结构化输出互斥 / 网关 400 / openai 格式思考回放未验证 / 输出上限随 effort 漂移五条），后续变更无需重新论证是否知情

#### Scenario: 第 4 条边界的精确分界

- **WHEN** 审查 bash 的护栏现状
- **THEN** 受保护文件写入面有护栏（保护闸）、任意命令执行与未受保护文件改动无护栏——两者在同一段边界文本内可分界，MUST NOT 出现「完全无保护」或「已沙箱化」两种失真的表述

#### Scenario: 兜底边界的失败面可见

- **WHEN** anthropic 协议下对「名单外 + 8K 输出上限」模型留空 `max_tokens`、未配置 `context_length`（取 128K 缺省档 32_000）发起运行
- **THEN** 请求以 provider 400 失败并经 `api_error` 走既有错误收敛（显式、可读诊断），不静默；用户填 `max_tokens = 8192` 后可跑通——该组合作为兜底策略边界的留痕证据

#### Scenario: 相邻 assistant 消息不设实现期核验门

- **WHEN** 审查实现期任务清单与验收面约定（边界条 16 的落点）
- **THEN** 无任何「真机 `pause_turn` 前置核验」的实现期门项或阻塞项；该依赖仅以已知限制留痕，确认动作落于**归档后人工验收**（官方端点 + 凭据，结论记入验收记录），本期无「合并相邻 assistant 消息」的实现分支

#### Scenario: 续跑过程仅 notice 可见

- **WHEN** 一轮较长的搜索触发多次 `pause_turn` 续跑
- **THEN** 实时面只见 `pause_turn_continued` / `pause_turn_capped` 两类 `SystemNotice`（无搜索相关 delta、无「正在搜索…」文案），搜索卡在轮收口后呈查询词与来源列表

#### Scenario: effort 的深度效果不作实现期门项

- **WHEN** 审查实现期任务清单与验收面约定（边界条 17–21 的落点）
- **THEN** 无任何「真机核验 effort 是否真的少想 / 省 token」的实现期门项或阻塞项；effort 的验收止于**请求体形状单测**（形状、留空零痕迹、两键共存），实际深度效果并入**归档后人工验收**（无凭据不属自动化验证面）

### Requirement: SDK 引擎模型协议选臂（openai / anthropic）

SDK 引擎 SHALL 支持两种模型协议并按 provider 记录声明选臂：`openai`（openai-compatible 端点，现有唯一形态）与 `anthropic`（Anthropic Messages 端点及其兼容网关）。协议 SHALL 为 provider 记录属性（`AgentProviderProtocol`），经组合根映射为门面 `Protocol` 后进 `EngineConfig.protocol`（映射沿 `AgentEngineKind` → `EngineKind` 双枚举先例，两臂 match），MUST NOT 编码进 `EngineKind`（协议是 provider 语义，不是租户语义；`EngineKind` 双值与 `runner_for` 唯一 match 点 MUST NOT 塌）。

模型组装点（`build_model`）SHALL 为协议唯一分叉处，两臂 MUST 同产 `DynModel<Completion>`（`rig::operation::Completion` 擦除型），使 `sdk/loop.rs` / `normalize.rs` / `resume.rs` / `crates/core/agent` 全链**协议中立、零改动**：

- **openai 臂**：`OpenAIConfig::new(api_key).with_base_url(base_url).client().chat(model).erase()`——与现状逐字等价，MUST NOT 追加任何请求参数（`max_tokens` 保持省略，各网关对超限行为不一）；
- **anthropic 臂**：`AnthropicConfig::new(api_key).with_base_url(base_url)`[条件 `.with_beta(..)`]`.client().completion(model)`，定型 wire 默认 `max_tokens`（取值链见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）后 `.erase()`；`AnthropicConfig` SHALL 经 `rig::providers::anthropic` 直接引用（rig 0.43 该模块**无 feature gate**，MUST NOT 为此新增 crate feature）。

协议差异 SHALL 全部由 rig 吸收，引擎 MUST NOT 自拼协议头或改写端点路径：

- **认证头**：openai `Authorization: Bearer`；anthropic `x-api-key` + `anthropic-version`（rig 内建），引擎零手写；
- **`base_url` 语义**：openai 原样拼 `/chat/completions`（地址须含版本段，如 `https://…/v1`）；anthropic 由 rig 先剥 `/v1`、`/messages`、`/v1/messages` 后缀、再拼 `/v1/messages`——用户填 `https://api.anthropic.com` 与 `https://api.anthropic.com/v1` SHALL 等价可用（MUST NOT 产生 `/v1/messages/v1/messages` 重复段）。

诊断面：引擎泵启动日志 SHALL 记 `protocol=…`（同一 `base_url` 配错协议时可一眼定位）与 `model=裸名 window=… raw=原文`；`RunStarted.model` SHALL 报**裸名**（反映实际发送），协议本身 MUST NOT 在事件面新增字段或变体（词汇不扩，前端适配层零改）。

#### Scenario: openai 臂逐字等价

- **WHEN** `protocol = openai` 的 provider 发起运行，审查 `build_model` 产物描述面
- **THEN** 得到的 `DynModel<Completion>` 的 `name()` 为 `openai`、`id()` 为模型名，与变更前的组装路径逐字等价；no 请求参数被追加（`max_tokens` 仍省略）

#### Scenario: anthropic 臂组装

- **WHEN** `protocol = anthropic` 的 provider 发起运行（模型名为裸名）
- **THEN** 得到的 `DynModel<Completion>` 的 `name()` 为 `anthropic`、`id()` 为裸模型名；请求带 `x-api-key` + `anthropic-version` 头（rig 内建，引擎零手写）；组装全程不打网络（仅描述面断言）

#### Scenario: loop 协议中立

- **WHEN** 以两臂产物分别驱动同一 `sdk/loop.rs` 会话
- **THEN** loop / normalize / resume / core 契约零改动可消费（两臂同为 `DynModel<Completion>`，无协议分支）；`AgentEvent` 词汇与变体集不变

#### Scenario: base_url 归一化

- **WHEN** anthropic 协议下 `base_url` 填 `https://api.anthropic.com/v1`（或带 `/v1/messages` 后缀）
- **THEN** rig 归一化后实际请求 `/v1/messages` 单段版本路径，不出现重复版本段；填 `https://api.anthropic.com` 与填带后缀形态等价可用

#### Scenario: 协议进配置不进 kind

- **WHEN** 审查 `EngineConfig`、`EngineKind` 与 `runner_for` 实现
- **THEN** 协议以 `EngineConfig.protocol` 携带（门面 `Protocol` 双值），`EngineKind` 仍为 `cli` | `sdk` 双值、`runner_for` 唯一 match 点结构与签名形态不塌；core 侧无协议字样

### Requirement: SDK 引擎模型名 `[1m]` 后缀与输出上限定档

SDK 引擎 SHALL 支持 provider 三档模型名携带 `<模型名>[1m]` 后缀（对齐 Claude Code 配置形态），并据此定档上下文窗长与 anthropic 请求参数。解析 SHALL 收在**纯函数单点** `parse_model_spec(raw) -> Result<ModelSpec{name, length: Option<u64>}, ModelSpecError>`（`agent-runtime` 公开导出，命令层保存期校验复用同一函数，MUST NOT 造第二套规则）：

- 末尾 `[1m]`（大小写不敏感）→ 剥离出裸名、`length = 1_000_000`（**1000 进制**：`1m = 1_000_000`）；
- **其余方括号形态一律显式拒收**：`[2m]`（无 2M 窗长常量，语义悬空）、`[8k]` / `[200k]`（本变更不做长度单位族）、`[]` / `[ 1m ]` 内空白、方括号不在末尾 → `ModelSpecError`；
- 无后缀 → `name = trimmed(raw)`、`length = None`；
- 两端空白 SHALL 裁剪；MUST NOT 把带后缀的原文发给 provider（请求 `model` 恒为裸名）。

窗长定档 SHALL 为**按档现算**（`high` / `low` 各自独立）：优先级 **模型名后缀 > provider 可空 `context_length` > 128K 缺省启发式**；后缀在场时 MUST NOT 忽略地叠加或夹取（后缀即该档**标称**窗长，`context_length` 被覆盖）。标称窗长 SHALL 再经「× 60% 与 400k 取小」折算为防线生效窗长（见「SDK 引擎上下文窗防线」——成本护栏，标称值本身不参与水位判定）。解析与防线装配 SHALL 发生在 `open_session`（那里同时拿到档位与模型原文），`EngineFacade::with_context_window` 签名 MUST NOT 变化——其语义降级为「缺省来源」。非法后缀在启动期 SHALL 以 `AgentStartError::ConfigMissing` 显式失败（MUST NOT 静默降级为裸文发送）。

anthropic 请求参数 SHALL 按以下规则定档：

- **`anthropic-beta`**：`protocol = anthropic` 且该档模型名带 `[1m]` → SHALL 追加 beta `context-1m-2025-08-07`（完全等价 Claude Code 的 1M 开关）；openai 协议或后缀缺席 → MUST NOT 发。
- **前缀缓存**：`protocol = anthropic` 的 wire SHALL 开启自动前缀缓存（top-level `cache_control`，rig `with_automatic_caching`）——缓存命中 MUST NOT 只依赖网关的隐式行为；手动断点形态（`prompt_caching`）MUST NOT 同时启用（两套断点策略并存无收益）。openai 协议 MUST NOT 追加任何缓存参数。
- **输出上限（`max_tokens`）取值链**：`provider.max_tokens`（显式值优先）→ rig 内置模型名单官方值（名单命中时**保留 rig 值、不被降级**）→ **按标称上下文窗长分档推导**（见下）。实现 SHALL 只在该链产出非 `None` 时定型 wire 默认 `max_tokens`（`Messages::with_default_max_tokens`），且**只在 `protocol = anthropic`**；openai 协议 MUST NOT 写入 `max_tokens`（保持「省略由协议自决」现状）。分档推导输入 SHALL 为**标称**窗长（与该档防线装配同源：`spec.length.or(context_window)`），MUST NOT 用折算后的防线窗长——成本护栏与能力策略分离。
  - 分段表（编译期常量，非配置面；区间左闭右开 `[lo, hi)`）：`W < 32 768` → `4 096`；`32 768 ≤ W < 131 072` → `16 384`；`131 072 ≤ W < 262 144` → `32 000`；`262 144 ≤ W < 524 288` → `48 000`；`W ≥ 524 288` → `64 000`（封顶）。
  - 128K 档 `32 000` 是兼容硬约束：`DEFAULT_CONTEXT_WINDOW = 128 × 1024`，未配置任何字段的用户取值 MUST 与既往兜底逐字相同。封顶 `64 000` 而非 `128 000`：rig 名单内模型先命中其官方值，本表只服务名单外模型。
  - `[1m]` 后缀经窗长**间接**影响输出上限（不再「只抬输入不抬输出」）。
- **上报口径**：`RunStarted.model` 与请求 `model` 为裸名；`SessionConfigSnapshot.model` 保留配置原文（配置快照语义）；引擎日志记 `model=裸名 window=… max_tokens=… raw=原文`（`window` 报防线生效窗长、`max_tokens` 报生效输出上限，openai 协议记 `-`）。

边界知情：无模型元数据表 → `[1m]` 不做模型白名单校验、beta 一律发（第三方网关拒绝以 `api_error` 显式暴露）；不实现 `maxOutputTokensRecovery`（撞上限即按既有 `finish` 语义收敛，用户调 `max_tokens` 后重跑）。上述边界见「SDK 租户 MVP 边界与已知限制」。

#### Scenario: 后缀解析规则表

- **WHEN** 以 `claude-sonnet-4-6[1m]`、`CLAUDE-SONNET-4-6[1M]`、`glm-4.6`、`  glm-4.6  `、`x[2m]`、`x[8k]`、`x[]`、`x[1m]y` 驱动 `parse_model_spec`
- **THEN** 前两者 → 裸名 + `length = 1_000_000`（大小写不敏感）；`glm-4.6` 与带空白的同一名称 → 裸名 + `length = None`（空白裁剪）；其后四者 → `ModelSpecError`（显式拒收，MUST NOT 把原文透传）

#### Scenario: 按档窗长独立

- **WHEN** provider `models.high = "claude-sonnet-4-6[1m]"`、`models.low = "glm-4.6"` 且 `context_length = 200000`
- **THEN** high 档标称窗长为 1M（后缀覆盖 `context_length`）、low 档为 200000；折算后生效窗长分别为 400k（封顶）与 120k；同一 provider 两档各自独立解析与折算，`EngineFacade::with_context_window` 签名不变

#### Scenario: 后缀缺席回落到 context_length

- **WHEN** 三档模型名均不带后缀，`context_length` 分别填 200000 与留空
- **THEN** 填列时标称窗长取填列值（生效窗 120k）、留空时取 128K 缺省启发式（生效窗 78_643）

#### Scenario: 前缀缓存显式开启

- **WHEN** 以 `protocol = anthropic` 组装模型并发起请求
- **THEN** 请求体带 top-level `cache_control`（自动前缀缓存开启），缓存命中不依赖网关隐式行为；`protocol = openai` 的请求体零缓存参数

#### Scenario: beta 条件发

- **WHEN** `protocol = anthropic` + 模型名 `[1m]`；`protocol = anthropic` + 无后缀；`protocol = openai` + 模型名 `[1m]` 三种组合
- **THEN** 仅第一种请求含 `anthropic-beta: context-1m-2025-08-07`；后两者不发 beta（openai 协议的 `[1m]` 只影响本地防线窗长）

#### Scenario: 输出上限四档取值链

- **WHEN** anthropic 协议下分别以（a）`provider.max_tokens = 8192`、（b）`max_tokens` 留空 + 名单内官方模型（如 `claude-sonnet-4-6`）、（c）`max_tokens` 留空 + 名单外模型（如 `glm-4.6` / `claude-3-5-sonnet-*`）且未配置窗长、（d）`max_tokens` 留空 + 名单外模型 + `context_length = 400000` 组装
- **THEN** （a）请求 `max_tokens = 8192`（不被 rig 官方值或窗长推导覆盖）；（b）请求取 rig 官方值（不被降级为推导值）；（c）请求 `max_tokens = 32000`（128K 缺省档，请求不再触发 `max_tokens must be set for Anthropic` 的编码期失败）；（d）请求 `max_tokens = 48000`（`[262144, 524288)` 档）

#### Scenario: 分段表边界

- **WHEN** 以标称窗长 `32767` / `32768` / `131071` / `131072` / `262143` / `262144` / `524287` / `524288` / `u64::MAX` 及未配置（`None`）驱动分档推导
- **THEN** 依次为 `4096` / `16384` / `16384` / `32000` / `32000` / `48000` / `48000` / `64000` / `64000`；`None` 与 `0` 同落 128K 缺省档 `32000`；推导值恒不低于保存期下限 `4096` 且随窗长单调非减

#### Scenario: `[1m]` 后缀经窗长提升输出上限

- **WHEN** anthropic 协议下名单外模型名带 `[1m]` 后缀、`max_tokens` 留空
- **THEN** 输出上限按 1M 标称窗长落封顶档 `64000`（输入与输出预算同向提升）；防线窗长仍按 60% × 400K 折算（成本护栏不随输出上限放宽）

#### Scenario: openai 协议不写上限

- **WHEN** `protocol = openai` 且 provider 填有 `max_tokens`
- **THEN** 请求 `max_tokens` 保持省略（字段被记录但不生效），行为与变更前逐字一致

#### Scenario: 非法后缀启动期显式失败

- **WHEN** 存量记录（或绕过保存期校验的路径）携非法后缀模型名发起运行
- **THEN** 启动以 `AgentStartError::ConfigMissing` 显式失败并指明模型名与后缀非法，不产生会话/轮记录、不推流、不把带后缀原文发给 provider

#### Scenario: 裸名上报

- **WHEN** 以 `claude-sonnet-4-6[1m]` 发起一轮并查看事件与快照
- **THEN** `RunStarted.model` 为 `claude-sonnet-4-6`（裸名）；`SessionConfigSnapshot.model` 为配置原文 `claude-sonnet-4-6[1m]`；引擎日志含 `model=claude-sonnet-4-6 window=1000000 raw=claude-sonnet-4-6[1m]`

### Requirement: SDK 引擎 anthropic 云端 web_search 启用与开关链路

SDK 引擎 SHALL 支持按 provider 开关启用 Anthropic 云端 `web_search` 服务端工具，且**仅在 `protocol = anthropic` 时生效**：

- **开关载体**：`web_search: bool` 为 provider 记录属性（`desktop-agent-management`），生效门 SHALL 在**解析期**（组合根：`provider.web_search && protocol == Anthropic`），MUST NOT 在保存期按协议拒收（用户切协议后开关值原样保留）。
- **穿透链**：开关 SHALL 经 `ResolvedEngine.web_search` → `EngineFacade::with_web_search` → `SdkRunner::with_web_search` → `PumpPayload` → `LoopTurn.web_search` 抵达 loop；`EngineConfig` 连接配置字段面 MUST NOT 增字段（与 `context_length` / `max_tokens` 同型走门面旁路 builder），`runner_for` 唯一 match 点结构与签名 MUST NOT 塌。
- **请求侧形状**：开关为真时请求 SHALL 挂恰一个 provider tool（`ProviderToolDefinition::new("web_search_20250305")` + `name: "web_search"` + `max_uses: 5`）经 `CompletionRequest::provider_tool(..)` 落到 `additional_params.tools`；开关为假时该键 MUST NOT 出现。**`max_uses = 5` 为成本护栏常量，本期不提供配置入口**（配置化需 store v5 迁移）。
- **工具面隔离**：`web_search` 是 provider 服务端工具，MUST NOT 进入客户端工具面——`RunStarted.tools` 两态均**不含** `web_search`（`RunStarted` 契约 `tools` 语义仍为「本轮下发的客户端工具面」），MUST NOT 新增 `serverTools` 字段；`sdk/tools.rs` 的 `TOOL_NAMES` / `sdk/policy.rs` 决策表 / 成员校验 MUST NOT 认识该名（模型侧幻觉调用 `web_search` 仍按 `tool_unavailable` 拒绝）。
- **诊断面**：引擎泵启动日志 SHALL 记 `web_search=…`（与 `protocol=… model=… window=…` 同列），开启时一眼可定位「这家 provider 是否发了 server tool」。
- **无服务端工具时的失败面**：不支持 server tools 的 anthropic 兼容网关 SHALL 以既有 `api_error` 显式失败收敛，MUST NOT 静默降级为「去掉工具重发」。

#### Scenario: 开关开启的请求形状

- **WHEN** provider 记录 `protocol = anthropic` 且 `web_search = true`，以该 provider 发起一轮运行
- **THEN** 该轮请求的 `additional_params.tools` 恰为 `[{"type":"web_search_20250305","name":"web_search","max_uses":5}]`，`tools`（客户端工具面）与变更前逐字一致；`RunStarted.tools` **不含** `web_search`

#### Scenario: 开关关闭零痕迹

- **WHEN** provider 记录 `web_search = false`（或缺省）发起运行
- **THEN** 请求 `additional_params` 中不含 `tools` 键，行为与变更前逐字等价；引擎日志记 `web_search=false`

#### Scenario: 协议门控在解析期

- **WHEN** provider 记录 `protocol = openai` 且 `web_search = true`
- **THEN** 解析产物 `ResolvedEngine.web_search` 为 `false`（门面与 runner 收到的亦为 `false`），请求不带 provider tool；保存期该组合**不报错**（记录原样入库），用户改回 anthropic 后开关即刻生效

#### Scenario: 服务端工具不进客户端面

- **WHEN** 审查 `sdk/tools.rs` 的 `TOOL_NAMES`、`sdk/policy.rs` 决策表与 loop 成员校验，并在开关开启的会话中令模型发出名为 `web_search` 的**客户端**工具调用
- **THEN** 该名不在任何客户端面清单中；调用被成员校验以 `SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 拒绝，run 不中断

### Requirement: web_search 服务端块的事件投影与回放保真

引擎 SHALL 把 rig 解码出的 Anthropic 服务端块投影为 `AgentBlock::WebSearch`，并以 raw 原文作为回放唯一事实源：

- **常量单点**：`WEB_SEARCH_TOOL_KIND = "web_search_20250305"` / `WEB_SEARCH_TOOL_NAME = "web_search"` / `WEB_SEARCH_MAX_USES = 5` / `ANTHROPIC_RAW_CONTENT_KEY = "anthropic_content"` / `CITATIONS_KEY = "citations"` SHALL 由 `sdk/web_search.rs` 单点持有（MUST NOT 在 loop / blocks / context 各写字面量；server 块 wire `type` 判别亦经该模块的 `use_id` / `result_use_id` 原语，`context::sweep_orphans` 复用同一判别）。
- **投影规则**（`blocks::project_choice(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)`；密封层双向分派住 `sdk/blocks.rs`，与增量层 `normalize` 对称，`web_search` 只提供 wire 原语）：先按 `tool_use_id` 索引全部 `web_search_tool_result`，再顺序遍历 choice——
  - `server_tool_use`（`name == "web_search"`）→ 产 `AgentBlock::WebSearch{ id, query, raw_use }`，其**配对 result 不重复产出**（载荷并入该块）；
  - 孤儿 `web_search_tool_result`（前序无配对 use）→ 单独产 `WebSearch{ raw_use: None, … }`；
  - 其他 raw server 块 → 跳过（不产块、不报错）；
  - `Text` → 产 `AgentBlock::Text{ text, citations }`（citations 自 `additional_params["citations"]` 提取）；
  - `Reasoning` / `ToolCall` / `Image` → 现状不变。
- **展示字段**：`results` 取 result `content` 数组中 `web_search_result` 项的 `url` / `title` / `page_age`；`error` 取 `web_search_tool_result_error.error_code`；`query` 取 `server_tool_use.input.query`。展示字段 MUST NOT 参与回放重建。
- **citations**：`AgentCitation{ url, title, cited_text, raw }`——展示字段可经 rig 解析，`raw` MUST 取 `additional_params["citations"]` 的**原值**（`Citation` 反序列化再序列化会规范化，如 `title: Option` → `null`，不得作为回放源）。
- **回放**（`replay_raw` / `replay_text`）：`WebSearch` 块 SHALL 依次重发 `raw_use` → `raw_result` 两条 `AssistantContent`（保原始 use→result 顺序），形状为 `Text { text: "", additional_params: { anthropic_content: <raw> } }`——**raw 块必须配空文本**（rig 校验：带 raw 又带文本即 `ConversionError`）；两者皆 `None` 的块 SHALL 跳过。`Text` 携非空 citations 时 SHALL 以 `additional_params.citations = [c.raw, …]` 回填。assistant 位 `ToolResult` 现状仍跳过。
- **事件面中立**：`WebSearch` 块与 `citations` 字段 SHALL 为协议中立的块词汇（`core/agent` 不认识 anthropic / rig），新增变体与带默认值字段**无需 store schema 版本升级**（事件 payload 为 `SerdeJsonCodec` 自描述 JSON），旧事件缺 `citations` 读出 `None`。

#### Scenario: use 与 result 收敛为一块

- **WHEN** 一次搜索产出的 choice 含 `server_tool_use{ id: "srvtoolu_1", input: { query: "…" } }` 与其配对 `web_search_tool_result{ tool_use_id: "srvtoolu_1", content: [web_search_result × N] }`
- **THEN** 投影产**恰一块** `AgentBlock::WebSearch{ id: "srvtoolu_1", query, results: N 条, error: None, raw_use: Some, raw_result: Some }`；result 块不另产块

#### Scenario: 错误结果与孤儿结果

- **WHEN** choice 含 `web_search_tool_result` 其内容为 `web_search_tool_result_error{ error_code: "max_uses_exceeded" }`，以及一个前序无配对 use 的孤儿 result
- **THEN** 前者 `WebSearch.error = Some("max_uses_exceeded")`（`results` 空）；后者单独成块且 `raw_use = None`（`id` 取 result 的 `tool_use_id`）；两者均不报错、不丢块

#### Scenario: 未知 server 块跳过

- **WHEN** choice 含非 `web_search` 的 `server_tool_use`（或其他未来 server 块类型）
- **THEN** 该块被跳过（不产 `AgentBlock`、不落 `Raw`、不报错），同 choice 内其余块照常投影

#### Scenario: 回放逐字节保真

- **WHEN** 以含 `WebSearch`（`raw_use` / `raw_result` 为含 `encrypted_content` 的原始 wire 值）与携 citations 的 `Text` 的转录驱动 resume 重建
- **THEN** 重建出的 `AssistantContent` 序列为 use → result 两条 `Text{text:"", additional_params:{anthropic_content: 原值}}` 与 `Text{text: 原文本, additional_params:{citations: [原值…]}}`，`additional_params` 与原始 wire **逐字节等价**；展示字段（query / results / error）不参与该重建

#### Scenario: 旧事件读出兼容

- **WHEN** 以不含 `citations` 键的旧 `AgentBlock::Text` JSON 反序列化
- **THEN** 读出 `citations = None`（不报错）；序列化缺省时 `citations` 落 `null`、`results` 落 `[]`（前端既有 fixture 零改动——TS 字段仍为可选位）

### Requirement: pause_turn 续跑（独立计数轴）

引擎 SHALL 接管 Anthropic 的 `pause_turn` 终局（rig 映射为 `FinishReason::Other("pause_turn")`），以**两条独立计数轴**续跑，MUST NOT 把续跑计入流重试预算：

- **续跑预算**：`PAUSE_TURN_MAX_CONTINUATIONS = 5`（单轮续跑上限）；该常量与 `WEB_SEARCH_MAX_USES`（单请求搜索次数上限）为**两个独立概念**——常量名、注释、日志字段 MUST NOT 混用。
- **互不侵占**：每次续跑前 SHALL 将 `attempt`（流重试轴）归零——一次 `pause_turn` 后续的网络抖动 MUST 仍有完整的 `retry.max_attempts` 预算，MUST NOT 继承续跑前的累积。
- **usage 照实累加**：每次续跑为真实计费请求，`usage` SHALL 逐次累加（MUST NOT 合并、MUST NOT 抹零），最终随 `TurnDone` 流出。
- **到顶正常收敛**：达上限 SHALL 按 `stop` 语义正常收口（**不是** `api_error`），并发 `SystemNotice{ subtype: "pause_turn_capped", payload: { continuations: 5 } }`。
- **过程可观测**：每次续跑 SHALL 发 `SystemNotice{ subtype: "pause_turn_continued" }`——这是本变更**唯一的过程可观测点**（无搜索实时 delta，见「SDK 租户 MVP 边界与已知限制」）。
- **不续跑条件**：空 choice MUST NOT 续跑；本轮存在客户端 `tool_calls` 时 MUST NOT 续跑（工具执行优先收口）。
- **停止语义**：续跑循环 SHALL 与既有停止信号一致截停（续跑间隙检查 `handle.stop_requested()`），停后不留孤儿续跑。

#### Scenario: 续跑两次请求

- **WHEN** 脚本化一轮：首次响应 `finish_reason = Other("pause_turn")` 且 choice 仅含 server 块，第二次响应正常 `stop`
- **THEN** 该轮共发两次请求（第二次携带增量后的历史），事件流含恰一条 `SystemNotice{pause_turn_continued}`，最终以 `stop` 收口

#### Scenario: 续跑不吃重试预算

- **WHEN** 一次 `pause_turn` 续跑后，紧接着的请求发生网络失败
- **THEN** 该失败仍获得完整的 `retry.max_attempts` 重试预算（`attempt` 已在续跑前归零），MUST NOT 出现「续跑一次即耗尽重试」的提前失败

#### Scenario: 上限到顶正常收敛

- **WHEN** 连续触发 `pause_turn` 达 `PAUSE_TURN_MAX_CONTINUATIONS` 次
- **THEN** 引擎按 `stop` 语义正常收口（run 收敛为 `completed`，非 `failed`），发 `SystemNotice{pause_turn_capped}` 且 payload 含 `continuations: 5`；usage 为各次请求真实累加值

#### Scenario: 不续跑的两种情形

- **WHEN** `pause_turn` 终局伴随空 choice；或本轮 choice 含客户端 `ToolCall`
- **THEN** 前者按既有空轮收敛路径收口（不续跑）；后者进入工具执行流程（不续跑），续跑计数不增

### Requirement: 输出截断终局（不重试、不执行工具、显式留痕）

引擎 SHALL 识别两类「输出被截断」的终局并显式留痕，MUST NOT 将其静默当作自然结束：

- **`max_tokens` 撞顶**：rig 映射为 `FinishReason::Length`（Anthropic `stop_reason = "max_tokens"`）。
- **上下文窗溢出**：rig 映射为 `FinishReason::Other("model_context_window_exceeded")`（Anthropic 4.5+ 在「输入 + `max_tokens` 越过窗长且生成触顶」时返回该值；更早代际为请求期 400）。判据 SHALL 为精确等值（不做字符串包含匹配，防 `Other("..._x")` 误判），与 `pause_turn` 同型。

两类截断 SHALL：

- **不重试**：MUST NOT 进入 `api_error` 退避重发链——截断与 IO 失败语义不同，请求体一字不改的重发必然再撞同一堵墙，只消耗重试预算。
- **不执行工具、不续跑**：截断流的输出不完整，本轮声明的工具调用 MUST NOT 执行（tool input 可能是半截 JSON），`pause_turn` 续跑 MUST NOT 触发。
- **正常收敛 + 显式留痕**：SHALL 以 `TurnDone{subtype:"success", is_error:false}` 收口（不是 `api_error`/run 级失败——那会绕过相位 evaluator 与决策重试链），并发 `SystemNotice{ subtype: "output_truncated", payload: { kind, turn_index, output_tokens } }`，`kind` 为 `max_tokens` | `context_window_exceeded`；同因 SHALL 落引擎日志（消费端关闭时事件面失声，日志面保观测）。
- **判定权归属**：产物是否足以交付由相位 evaluator 判定（它知道该相位应产出什么），引擎只报「被截断了」。引擎 MUST NOT 注入续写消息、MUST NOT 自动抬高 `max_tokens`（续写等于绕过 provider 硬上限）。

#### Scenario: `max_tokens` 截断

- **WHEN** 一轮响应以 `FinishReason::Length` 收尾
- **THEN** 该轮共发一次请求（零重试）、零 `api_retry` / `api_error` 通知；产出恰一条 `SystemNotice{output_truncated}` 且 `kind = "max_tokens"`；`TurnDone` 为 `success` / `is_error = false`

#### Scenario: 上下文窗溢出截断

- **WHEN** 一轮响应以 `Other("model_context_window_exceeded")` 收尾
- **THEN** 行为同上，但 `kind = "context_window_exceeded"`（成因与用户可调性不同：前者调 `max_tokens` 可解，后者上限再大也会撞，属防线失守）

#### Scenario: 截断流里的工具调用被弃用

- **WHEN** 截断帧之前已流出一条客户端 `ToolCall`
- **THEN** 该调用 MUST NOT 执行（零工具结果事件），MUST NOT 因它再发第二次请求；本轮以截断收口

#### Scenario: 非截断终局不误判

- **WHEN** 终局为 `Stop` / `ToolCalls` / `None` / `Other("pause_turn")`
- **THEN** 零 `output_truncated` 通知，收敛语义与既有行为逐字一致

### Requirement: 异常终局 fail-closed（拒答与未知终局报错）

引擎 SHALL 以**白名单**判定正常终局，白名单之外一律按失败收敛，MUST NOT 静默当成功：

- **正常终局**：`Stop`（自然结束）、`ToolCalls`（模型要调工具）、`Other("pause_turn")`（服务端工具待续，由续跑循环消费），以及 **`None`**（provider 未声明终局——rig `Finish::default()` 即 `None`，部分网关不填 `finish_reason`；放行以免这类网关全线误报失败）。
- **拒答**：`FinishReason::ContentFilter`（rig 已把 Anthropic `stop_reason = "refusal"` 归一于此）SHALL 以 `TurnDone{subtype: "model_refused", is_error: true}` 失败收敛，并发 `SystemNotice{api_error}` 记因，成因文案 SHALL 声明「不可重试」并建议调整输入或更换模型。rig 0.43 的解析层丢弃 `stop_details`，故成因 MUST NOT 声明拒答分类（无数据即不编造）。
- **未知终局**：`Other(<未知值>)` SHALL 以 `TurnDone{subtype: "unexpected_finish", is_error: true}` 失败收敛，记因串 SHALL 携 provider 原值。
- **共处置**：两类均 MUST NOT 重试（同输入同判定）、MUST NOT 执行本轮声明的工具、MUST NOT 续跑；`choice` 为空时同样适用（**判定 SHALL 先于空轮收敛**，否则纯拒答被当成「无话可说」）。
- **显示与记录**：成因经 `SystemNotice{api_error}` 载荷流出——会话横幅（前端 `latestAgentFailure`）与 step detail（编排侧 `failure_reason` → `会话失败收敛: <成因>`）取同一串，MUST NOT 只有一处可见。

#### Scenario: 拒答以失败收敛

- **WHEN** 一轮响应以 `FinishReason::ContentFilter` 收尾
- **THEN** 该轮共发一次请求（零重试）；事件流含一条 `SystemNotice{api_error}` 且成因含「拒答」「安全分类器」「不可重试」；`TurnDone` 为 `subtype = model_refused` / `is_error = true`

#### Scenario: 零内容拒答不被空轮吃掉

- **WHEN** 拒答响应无任何内容块（`choice` 空）
- **THEN** 仍按拒答失败收敛，MUST NOT 走「空轮正常收敛」分支

#### Scenario: 未知终局 fail-closed

- **WHEN** provider 返回本地方案之外的 `stop_reason`（如第三方网关自造值）
- **THEN** `TurnDone{subtype: "unexpected_finish", is_error: true}`，记因串携 provider 原值；本轮工具不执行

#### Scenario: 终局缺席放行

- **WHEN** provider 未声明 `finish_reason`（`None`）且响应有内容
- **THEN** 按正常收敛（`success`），不误报失败

### Requirement: 上下文防线 server 块孤儿清扫

`sweep_orphans`（`sdk/context.rs`）SHALL 在既有「`UserContent::ToolResult ↔ CallId`」配对清扫之外，同时认 **assistant 位 server 块配对**：

- **索引源**：`WebSearch.id`（use 侧）与 `raw_result.tool_use_id`（result 侧）。
- **清扫对象**：assistant 位中 `raw_use == None` 且前序找不到对应 use 的 `WebSearch` 块 SHALL 剔除（回放侧缺 use 的 result 会被 Anthropic 拒）。
- **剔除后处置**：消息 `content` 变空则整条移除；非空则保留其余内容。
- **触发点不变**：L2 剪裁 / L3 硬裁 / 摘要置换三处既有调用点 SHALL 自动生效（MUST NOT 新增第四调用点）。
- **配对完整的块 MUST NOT 被剔**（含 use 与 result 同处一条消息、以及 `pause_turn` 拆分后分处两条消息但 use 仍在转录中的情形）。
- **compaction 请求**仍保持「摘要请求无工具」（`compact.rs` 零改动）。

#### Scenario: 裁掉 use 后剔除孤 result

- **WHEN** `pause_turn` 把一次搜索拆成两条 assistant 消息（消息 A 携 use 侧 `WebSearch{raw_use: Some}`，消息 B 携 result 侧 `WebSearch{raw_use: None, raw_result: Some}`），L2 剪裁把消息 A 整体裁掉
- **THEN** `sweep_orphans` 剔除消息 B 中的该 result 块；消息 B 若因此变空则整条移除；重建后的历史可被 Anthropic 接受

#### Scenario: 成对块保留

- **WHEN** 历史中 use 与 result 分处两条 assistant 消息且两条均未被裁
- **THEN** 两块的配对索引命中，`sweep_orphans` 原样保留（MUST NOT 误剔 result 侧）

#### Scenario: 既有工具配对零回归

- **WHEN** 以既有「`ToolResult` 孤儿」用例驱动扩展后的 `sweep_orphans`
- **THEN** 行为与扩展前逐字一致（原有用例全绿），两类清扫互不干扰

### Requirement: loop 守卫链写入保护闸与工作区根注入缝

SDK 引擎的 loop 分发链 SHALL 在既有三段守卫之后、执行之前增设第四段「写入保护闸」，并由一条与 `with_explore_note_store` 同构的 builder 缝注入主仓 root：

- **四段顺序不变量**：`① 成员校验 → ② 档位门（policy）→ ③ 路径沙箱（sandbox_rewrite）→ ④ 写入保护闸 → 执行`。保护闸 MUST 位于沙箱改写**之后**（此时 `path` 已是 canonical 绝对路径，判定无需再做 IO 级规范化），MUST 位于 `tools::execute` / `explore_note` 执行**之前**；MUST NOT 提前到沙箱之前，MUST NOT 下沉进 `tools::execute`（那是纯函数，塞入读配置会让签名长出 IO 依赖，且拒绝形态只能由 loop 合成）。
- **判定输入**：`write` / `edit` 取沙箱改写后的 `path` 字段；`bash` 取 `command` 原文（命令内容不可 canonicalize）与会话执行根 `LoopTurn.cwd`（相对路径解析基座）。
- **拒绝形态复用既有 `deny()`**：`SystemNotice{subtype:"permission_denied", payload:{tool, reason}}` + 与 tool_use 同 id 的 is_error `ToolResult`，**run 不中断**（模型换路自愈）。MUST NOT 新增 subtype、MUST NOT 新增事件词汇、MUST NOT 新增收敛退出码；保护闸 MUST NOT 自行发事件（拒绝串内容归 desktop-write-protection）。
- **档位面**：保护闸在任何 `permission_mode` 下均生效（Default 档本就拒绝 write / edit / bash；AcceptEdits 与 BypassPermissions 档下受保护文件仍被拒）。MUST NOT 修改 policy 三档决策表、MUST NOT 引入审批通道。
- **装配时机**：保护规则 SHALL 每 run 装配一次（会话泵起播时 `protect::load`）**并就地合并会话级规则**（`with_session(injections.protect)`，见下条），随 `LoopTurn.protect` 下发（与 `defense` / `retry` / `liveness` 的「装配点算好、loop 纯消费」形态同构）；loop 内 MUST NOT 读盘、MUST NOT 每 tool_call 重读配置。
- **规则来源三处与位次**：`ProtectRules` SHALL 由三个来源按次装配——**内置（工作区 `config.json`）→ 会话（相位锁）→ 用户（自选）**，首个命中即定（位次即优先级，MUST NOT 拼接多条 reason）。会话级规则的语义与范围归 `desktop-write-protection`「会话级写入禁区（相位锁）」；本能力只保证它经 `SessionInjections.protect` → `with_session` → `LoopTurn.protect` 同路抵达判定面（`load` 的签名与「唯一 IO 点」语义逐字不变）。
- **工作区根通道（新 builder 缝）**：root 只在命令层显式在手（store 句柄无 root 字段），故 SHALL 沿既有先例逐层下行：

```
compose_turn(stores, registry, streams, store, agent, workspace_root: &Path)
  → EngineFacade::with_workspace_root(root)
  → SdkRunner.workspace_root: Option<PathBuf>
  → PumpPayload.workspace_root → 泵起播时 protect::load(…).with_session(injections.protect)
  → LoopTurn.protect: ProtectRules
```

  `workspace_root` MUST 为**主仓** root（命令层 `root` 参数），MUST NOT 取会话执行根 `SessionCtx.workspace_root`（change 流程下为 worktree）——工作区配置的权威在主仓；会话规则的 glob 由装配层派生（与 root 无关，worktree 与主仓同样命中）。
- **缺席语义**：`workspace_root` 为 `None`（未注入）时 SHALL 只装载内置规则（用户规则缺席），MUST NOT 关闭保护闸、MUST NOT 退化为空规则集；`protect` 位为 `None` 时会话规则缺席（不追加一条空 glob）。
- **射程边界**：专用工具 `create_explore_note` MUST NOT 经保护闸（专用工具不在保护射程内为既有裁定，一致性缺口留痕于 desktop-write-protection）；五个 archi 只读工具同样不在射程（`_ => None` 分派臂）。
- **错误归属次序**：沙箱拒绝（`sandbox_denied`）SHALL 优先于保护拒绝（`permission_denied`）——逃逸路径先按沙箱报错，不被保护闸抢先。

#### Scenario: 四段顺序与拒绝形态

- **WHEN** BypassPermissions 档下模型调用 `write` 指向 `<root>/.dev-team/config.json`（路径在 root 内）
- **THEN** 事件流含 `SystemNotice{subtype:"permission_denied", payload:{tool:"write", reason:…}}` 与同 id 的 is_error ToolResult（内容为同一拒绝串）；`tools::execute` 未被调用（文件未被写）；run 不中断，同轮后续工具调用照常执行

#### Scenario: 沙箱先于保护闸

- **WHEN** 模型调用 `write` 指向 `<root>/../outside/.dev-team/config.json`（越出 workspace root）
- **THEN** 拒绝理由为沙箱成因（`subtype:"sandbox_denied"`），而非 `permission_denied`——两段闸的错误归属不重叠、不抢报

#### Scenario: 每 run 一次装配与 loop 零 IO

- **WHEN** 在一次 run 进行中修改 `.dev-team/config.json` 的 `write_protection` 与会话注入面，随后同一 run 内继续调用工具
- **THEN** 本轮行为不随修改改变（规则在泵起播时已装配）；`sdk/loop.rs` 内无 `config::load` / `fs` / `canonicalize` 调用点

#### Scenario: 主仓口径与 worktree 执行根

- **WHEN** change 流程会话的执行根为 worktree（`SessionCtx.workspace_root` = worktree 路径），主仓 root 另经 builder 缝注入
- **THEN** 用户规则取自主仓 `config.json`；写 worktree 内 `.dev-team/config.json` 仍被内置规则拒（内置 glob 不区分主仓 / worktree）；相位锁对 worktree 内的模型目录同样命中（会话规则 glob 与执行根无关）

#### Scenario: 未注入只内置

- **WHEN** 门面未注入 `workspace_root`（`None`）
- **THEN** 保护闸仍生效且只含内置规则（写工作区配置被拒），用户配置的 glob 不生效；会话规则若在位仍照常生效（其来源不是配置读取）

#### Scenario: 专用工具直通

- **WHEN** explore 会话调用 `create_explore_note`，且用户配置保护了该工具的目标目录
- **THEN** 调用不被保护闸拦截（专用工具直通为知情边界），`write` / `edit` 对同目录的写入仍被拒

#### Scenario: 会话规则位次可辨

- **WHEN** 用户配置显式保护了模型目录（`write_protection.files` 命中），且当前会话不在写权白名单内
- **THEN** 拒绝串取**会话规则**（相位锁文案携指路）而非用户规则文案——会话规则位次在内置之后、用户之前；内置 `config.json` 规则仍恒最先

### Requirement: 自然收敛前的收口前钩子与收敛事件序

SDK 引擎 loop SHALL 提供**可注入的收口前钩子**（`BeforeDoneHook`）：钩子实例按会话注入面档位（`SessionInjections.before_done`）由 runner per-turn 铸造，`None` = 不装配；loop 只承载扩展点与调用序，MUST NOT 认识任何具体钩子语义（判据归各实现，见 `desktop-archi-agent`「收口前引擎自检（架构判据读数）」）。已装配的钩子在**自然收敛**收口前执行一次，结果作为一条 `SystemNotice` 落在该轮 `TurnDone` **之前**；该机制 SHALL 对模型不可见（不回灌、不催轮、不发新请求），且 MUST NOT 改变任何收敛语义：

- **钩子三面**：`start`（loop 起手一次，建观测起点）/ `observe_tool`（每次工具调用，沙箱 ∧ 保护闸通过后、执行前——零额外 IO）/ `before_done`（自然收敛前一次）。三面 MUST NOT 返回错误、MUST NOT 阻断 run（fail-open 归实现自决）。
- **事件序**：`Message`（终答密封）→ `SystemNotice{subtype:"archi_preclose"}`（或 `archi_preclose_skipped`）→ `TurnDone`。notice MUST 先于 `finish_success` 发出；`TurnDone` 的 `subtype` / `is_error` / `num_turns` / `usage` 与本变更前逐字一致（钩子结论 MUST NOT 进入 `TurnDone`）。
- **挂点唯一**：只在「无 tool_calls ∧ 非 `pause_turn`」自然收敛支执行一次；空 choice / 输出截断 / 异常终局（拒答 / 未知终局）/ `stop_requested`（轮间 / 轮头 / 续跑间隙）/ `pause_turn` 续跑到顶五个出口 MUST NOT 执行。装配判据 SHALL 为 `SessionInjections.before_done` 档位——未装配的会话零射程；loop MUST NOT 以 `preamble` 在场 / `turn.archi` / 工具名清单等旁证自行推断（config 臂同样生产 preamble；change 相位同样下发 archi 工具）。
- **零请求增量**：钩子 MUST NOT 触发模型请求、MUST NOT 追加对话史消息（`history` 逐条不变）、MUST NOT 增加轮次（`turn_index` 不因它推进）。
- **阻塞面**：判据为 CPU / IO 型同步工作，SHALL 经 `tokio::task::spawn_blocking` 执行；loop 的 async 泵 MUST NOT 被其阻塞（自检期间不得阻断事件通道的其它在途发送）。
- **fail-open**：钩子任何失败（判据 `Err` / 任务 panic 兜底）SHALL 以 `SystemNotice{subtype:"archi_preclose_skipped", payload:{reason}}` 留痕后照常收敛；MUST NOT 阻断、MUST NOT 把失败升级为 `api_error`、MUST NOT 新增收敛退出码或事件词汇。
- **subtype 开放词典**：`archi_preclose` 与 `archi_preclose_skipped` 为 `SystemNotice` 开放词典的两个新词（与 `api_retry` / `context_pruned` / `output_truncated` 同列），MUST NOT 引入新的信封类型或事件变体。
- **未装配会话零射程**：`before_done = None` 时会话 MUST NOT 产生任何钩子 notice、MUST NOT 增加可观测事件（最多一行引擎日志）。

#### Scenario: notice 先于 TurnDone

- **WHEN** 触发判据命中的 archi 会话自然收敛
- **THEN** 该轮事件序为「密封 `Message` → `SystemNotice{archi_preclose}` → `TurnDone`」；`TurnDone` 字段与本变更前逐字一致；`history` 无新增消息

#### Scenario: 五出口只跑一支

- **WHEN** 分别构造自然收敛 / 空 choice / 输出截断 / 用户停止 / `pause_turn` 续跑到顶五个出口（触发判据置真）
- **THEN** 只有自然收敛支出 notice；其余四出口零 `archi_preclose` 事件，收敛语义与既有断言逐条一致

#### Scenario: 失败面照常收口

- **WHEN** 自检执行体返回 `Err`（如架构域配置非法）
- **THEN** 流出 `SystemNotice{archi_preclose_skipped}`（载荷含成因）后 `TurnDone{subtype:"success", is_error:false}` 照常；run 不中断、无 `api_error`、无额外请求

#### Scenario: 不阻塞事件通道

- **WHEN** 自检执行期间检查事件通道
- **THEN** 该轮其它在途事件照常送达（自检在 blocking 线程池执行，泵未被同步阻塞）；判据耗时经 notice 载荷的 `durationMs` 可见

### Requirement: ask 工具协议与阻塞应答通道

SDK 引擎 SHALL 提供基线阻塞工具 `ask_user_question`，其应答通道 SHALL 为**会话级注册表**（`AskRegistry`，与 `StopRegistry` 同址托管）：

- **工具契约**（定义单源在 `core/orchestration`，见 desktop-change-orchestration「流程内 ask 停等与双入口应答」）：工具名 `ask_user_question`，`parameters` 为 `{ name: string（必填，`pattern` = `^[A-Za-z][A-Za-z0-9_-]{0,63}$`）, question: string（必填）, options: string[]（必填，`minItems: 1`）}`——**一次问一个问题**，多问经多次调用表达；`name` 是**协议声明**的本问标识，即应答 map 的键（首字符须为字母，故控制键位不可能与之撞车）；`name` 非法或缺失、`options` 为空数组 SHALL 被工具校验 `Err` 拒绝（文案自足：字段名 + 期望 + 当前值）；用户应答 MAY 为任一选项文本或自由文本，工具 MUST NOT 限制应答取值域。**本工具要求必答**（无取消通路）：用户想改变主意时以自由文本表达，`status` 之外无第二种终态。
- **应答结果形状**（单点在 `infra/agent` 的 ask 执行体）：成功臂的 tool result 正文 SHALL 为**信封** `{"status":"answered","answers":{"<name>":"<应答原文>"}}`——`status` 为封闭枚举，本版仅 `answered`（`declined` 为预留位，见下条）；`answers` 为「问题名 → 用户原文」map，原文逐字保真（零 trim、零截断）。信封形状 MUST NOT 在前端铸造（前端两处入口一律上行用户原文，见 desktop-agent-chat-infra / desktop-change-flow-view）。多问扩展只让 `answers` 多键，信封 MUST NOT 变更；读侧 SHALL 兼容历史三种正文——旧扁平 map（无 `status`）、更早的自由文本原文、错误文案（后者不进信封，随 is_error 回灌）。`status` 字段 MUST NOT 因「本版只有一个取值」而被删除——它是取消通路的唯一扩展位。
- **取消通路（预留，未实现）**：若将来开放取消，结果 SHALL 为 `{"status":"declined"}`（无 `answers` 键），且 SHALL 以 `is_error: false` 回灌（用户主动取消不是工具故障）；`AskRegistry` 的投递类型相应扩为「应答 / 取消」两态。本条为形状预留，当前实现 MUST NOT 提供取消入口。
- **`AskRegistry`（`core/agent`）**：形态 SHALL 为 `{ slots: Mutex<HashMap<String, oneshot::Sender<String>>> }`，键 = **core 铸的会话 id**（`core/agent` 不认识 workspace root / change——两段寻址由壳层翻译）；方法三件：`register(session_id, tx) -> bool`（登记等待方，同键覆盖 = 上一问已被放弃，返回是否登记成功）、`answer(session_id, text) -> Result<(), String>`（**take 语义**：单次投递，重复 / miss → `Err`，不产生第二次回灌）、`remove(session_id)`（轮收敛除名，miss 幂等）。注册表 MUST NOT 携带任何编排域类型（无 change id、无 `RunEventSink`）。
- **会话 id 下传**：`open_session` 的入参 SHALL 携带 core 铸的会话 id（`SessionOpen.session_id`），使引擎侧能以自己的执行体上下文登记到正确键；逐轮的 `TurnQuestion` MUST NOT 重复携带该 id（会话级标识在会话建立时一次可知）。
- **两档执行体（同名覆盖）**：`ToolExec::Ask(AskMode)` 为单一挂载变体带档位，`TOOL_NAME` 与 `parameters` 两档共用（schema 单源），差别只在执行体：
  - `Generic`：`register(session_id, tx)` → 阻塞等待应答（无编排广播）→ 返回**应答信封**（形状见上条）作为 tool result；
  - `Flow`：经 `AskPort` 先广播「ask 已开始」（`AskPayload` 含 `tool_call_id` / `session_id`）→ `register` → 阻塞等待 → **两臂（应答 / 停止）返回后都广播「ask 已解除」**（流程停等态清理单点在此，见 desktop-change-orchestration）。
  两档 SHALL 与 `finish_step` **互不依赖**：`ask` 位与 `finish` 位独立置位，两模块代码互不引用（`grep` 可验的零互引，见 desktop-change-orchestration 同条）。
- **`ask` 位与挂载**：`SessionInjections.ask: Option<AskMode>` 决定是否挂载（`None` = 不挂 = 硬禁用：`RunStarted.tools` 无该名，模型调用即 `tool_unavailable`；`Some(_)` = 挂载，tier 恒 `Protocol`，三档权限全放行）。执行上下文 SHALL 经组合根以 per-turn 形态铸造（注册表句柄 + core session id + 可选 `AskPort`）；上下文缺席 / 登记失败 SHALL 以 is_error ToolResult 回灌（MUST NOT panic、MUST NOT 静默成功）。
- **停止语义**：ask 阻塞期间的用户停止 SHALL 由外层泵的 `select!` **轮中 drop 整个 loop future** 实现（既有机制，零新增）；因此 ask 执行体 MUST NOT 依赖「停止时回灌 is_error 让模型看见」——做不到，且转录会留下未配对的 `tool_use`，该缺口 SHALL 由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 补齐（既有机制）。`Flow` 档的 `select!` SHALL 显式带停止臂（`RunHandle::wait_requested()`）——外层 drop 不执行清理代码，故停等态清理必须落在执行体内的这个臂上。
- **停等态非 durable 与进程中断归属**：ask 停等态 SHALL 只活在进程内（`AskRegistry` 槽 + 阻塞中的 loop future），MUST NOT 落库——「仍在等待应答」在库中没有任何 durable 痕迹，注册表在进程新生时必然为空。因此进程在工具执行中死亡（关窗 / 崩溃 / 重启，含 ask 停等期与任意工具执行期）时，该轮 SHALL 被当作**客观终止**：由启动标定把残留 `running` 轮行翻为 `interrupted`（见 desktop-workspace-store「agent 轮启动标定与 running 残留终结」），未配对的 `tool_use` 保留在转录（与上条「停止语义」同形，provider 配对由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 兜底）。此后对同一 `session_id` 的 `agent_answer` SHALL 走既有 miss 臂显式 `Err`（「当前无等待中的 ask」）——MUST NOT 幂等 `Ok`、MUST NOT 改写该轮状态、MUST NOT 提供「复活停等」入口（会话接续见下条边界）。本 requirement MUST NOT 新增停等态持久化面或恢复面。
- **幂等与可归因**：同一 ask 二次应答 SHALL 返回 `Err`（文案「当前无等待中的 ask」）；会话已收口后的迟到应答 SHALL 同样 `Err`（注册表槽已由轮收敛除名）；两处 `Err` 都 MUST NOT 改写任何会话状态。

#### Scenario: 通用会话 turn 内阻塞问答

- **WHEN** `ask = Some(Generic)` 的会话里模型调用 `ask_user_question { name: "retry", question, options }`，用户经 `agent_answer` 投递应答「指数退避」
- **THEN** 该轮不收敛、不发额外模型请求；该 `tool_use` 的 tool result 正文为 `{"status":"answered","answers":{"retry":"指数退避"}}`（`is_error: false`），模型在同一 turn 内继续（同一 `session_id`、`turn_index` 递增一次）

#### Scenario: 同名覆盖只多广播

- **WHEN** `ask = Some(Flow)` 的 change 会话里模型调用同一工具
- **THEN** 下发的工具名与 `parameters` 与 `Generic` 档逐字相同（模型视角零差异、历史转录无需分辨来源），额外经 `AskPort` 先后广播「已开始（携 `tool_call_id` 与 `session_id`）」与「已解除」；应答以同一信封形状照常回灌（形状与档位无关）

#### Scenario: 显式禁用即硬禁用

- **WHEN** 会话以 `ask = None` 装配，模型仍发出 `ask_user_question` 调用
- **THEN** `RunStarted.tools` 不含该名；调用被成员校验拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌，run 不中断，注册表无登记

#### Scenario: 停止不改写协议配对

- **WHEN** ask 阻塞期间用户停止该会话
- **THEN** 轮收敛 `stopped`；转录保留该 `tool_use` 的未配对记录（无 is_error 回灌——future 已 drop）；以该会话续轮时 `resume::rebuild` 补 `INTERRUPTED_RESULT` 使 provider 配对协议不破；注册表该键已除名（其后 `answer` → `Err`）

#### Scenario: 重复应答只回灌一次

- **WHEN** 同一 ask 被连续应答两次（两入口或同入口重放）
- **THEN** 第一次成功且等待方收到原文，第二次返回 `Err`（「当前无等待中的 ask」），等待方 MUST NOT 收到第二条、转录 MUST NOT 出现第二条 tool result

#### Scenario: 重启后停等轮已终结且应答显式失败

- **WHEN** 一轮阻塞在 `ask_user_question` 时进程被终止（模拟中途死亡），重启后对该会话调用 `agent_answer`
- **THEN** 该轮行在所属 workspace 库首次打开时已标定为 `interrupted`（携标定记因、`finished_at` 在位）；`agent_answer` 返回显式 `Err`（文案含「无等待中的 ask」），MUST NOT 幂等 `Ok`、MUST NOT 改写该轮行；该 `tool_use` 在转录中保持未配对，续轮时由 `resume::rebuild` 补合成结果不破 provider 配对

#### Scenario: 上下文缺席回灌而非崩溃

- **WHEN** `ask` 位为 `Some` 但执行上下文未装配（测试构造 / 装配漂移）
- **THEN** 该工具调用以 is_error ToolResult 回灌（「ask 执行上下文未装配」类文案），run 不中断、不 panic

### Requirement: 运行中会话的补充说明注入（steer 通道）

`core/agent` SHALL 提供**会话级单向注入通道**，把用户的一句补充说明送进**正在跑的 turn**：语义为「对一条运行中的 turn 追加一条 user 消息」，模型在**下一个请求边界**看到它。该通道 MUST NOT 新开轮、MUST NOT 排队到轮后重开（无 Continue 续跑）、MUST NOT 打断在途请求、MUST NOT 触达 ask 阻塞臂。与同类通道的坐标：`ask` 是「模型发起、轮内阻塞、用户单向应答」，`finish_step` 是「模型发起、轮内收口」——steer 是三者中唯一**用户发起、非阻塞**的一条。

- **`SteerRegistry`（`core/agent/src/steer.rs`）**：形态 SHALL 为 `{ slots: Mutex<HashMap<String, mpsc::UnboundedSender<SteerMessage>>> }`，键 = **core 铸会话 id**（与 `StopRegistry` / `AskRegistry` 同址同键空间；`core/agent` 不认识 workspace root / change，两段寻址由壳层翻译）。方法三件：`new()`、`register(session_id, tx)`（内核开轮时登记，同键覆盖即上一轮的接收入端已 drop）、`request(session_id, msg) -> Result<(), String>`（**miss 即 `Err`**，文案「会话已收口，补充说明不可送达」；`send` 失败同归此臂——两者都是「该会话当前没有在跑的轮」）、`remove(session_id)`（轮收敛除名，miss 幂等）。注册表 MUST NOT 携带任何编排域类型。
- **`SteerMessage`**：`{ text: String, ack: tokio::sync::oneshot::Sender<()> }`。每条消息自带 ack 是「不合并」在类型上的载体——三条在途 = 三个 ack，逐条确认。
- **通道挂载**：`AgentSession` SHALL 增第四个字段 `steers: mpsc::UnboundedSender<SteerMessage>`（与 `observations` / `questions` / `handle` 并列）；接收入端由引擎侧建通道时铸造，随 `session_pump` 交 `loop::run`（**不进 `LoopTurn`**——`try_recv` 需 `&mut` 接收端，`LoopTurn` 以 `&` 携带）。内核 `begin_turn` SHALL 在停止登记之后以同一 `session_id` 登记该 sender；`converge` SHALL 在同一处除名（与 `StopRegistry::remove` / `AskRegistry::remove` 并列）。
- **drain 点与单条语义**：`loop::run` SHALL 在主循环顶部（停止快检之后、`defend` 与发请求之前）以 `try_recv` **循环**取走在途补充说明；每条依次执行：① 发一条密封事件 `AgentEventKind::Message { role: User, blocks: [Text], parent_tool_use_id: None }`（走既有 `observations → stamp → append_sealed → streams.publish` 单点，零新增落库面）；② 向 `history` 推入一条**新的** `Message::user(text)`；③ `ack.send(())`。循环而非单次 `recv` 是「不合并」的要求：一次边界把全部在途项取走，逐条独立事件 + 逐条独立 ack。MUST NOT 把补充说明文本并入 `tool_results` 那条消息——rig 0.43.0 不合并相邻同角色消息，openai 侧展开会把 user 文本排到 tool 之前（provider 400）。drain 点位置天然满足该约束：此时 `history` 尾部正是 `Message::tool_results`。
- **送达语义（drop-Err，零超时）**：`request` 成功后命令 `ack.await`。轮收口时接收入端随 `loop::run` 返回被 drop → 通道内缓冲消息随之 drop → ack sender 随之 drop → `ack.await` 收 `RecvError` → 命令返 `Err`（「未送达（会话在补充说明生效前结束）」）。MUST NOT 以超时代偿该机制、MUST NOT 额外清理表项。
- **三态判据（ask 优先）**：送达可否 SHALL 由后端单一判据链决定，前端零预判：① `AskRegistry::is_pending(session_id)` → 不可送达（「等待应答中，补充说明不可送达」）；② `SteerRegistry::request` miss → 不可送达（「会话已收口」）；③ ack 失败 → 不可送达（「生效前结束」）。**次序不可交换**：ask 阻塞期间 `SteerRegistry` 条目仍在场（会话未结束，只是 loop 卡在工具执行分支），故条目在场不能证明可送达——`is_pending` 是这个窗口的唯一真相。为此 `AskRegistry` SHALL 新增只读方法 `is_pending(&self, session_id: &str) -> bool`（`slots.contains_key`，O(1)；不新增第二张表、不改 `answer` 的 take 语义）。
- **锁纪律**：三段判据 SHALL 各自「块内取放」（每段取锁即释放，MUST NOT 任何两段嵌套），沿 `control.rs::answer` 的「块内取值、块外投递」既有纪律；与 `execute_ask` 的 `register`（引擎线程持 ask 锁）之间 MUST NOT 形成双向等待面。
- **停止**：MUST NOT 新增停止机制——`agent_stop` 仍经 `RunHandle` 杀伤该轮；在途补充说明随接收入端 drop 收 `Err`，转录不留伪事件。
- **留痕**：MUST NOT 新增 `SystemNotice` 变体——补充说明落成普通 `role = user` 密封事件即留痕。轮内中段出现 user 消息本身即是判据（一轮一命且 walker 只在轮首发提问，故「轮中段的 user 事件 = steer」）。
- **边界**：`ask` 阻塞期不 drain（本期明确取舍，不排队）；`finish_step` 收口后的同批残留一律 `Err`；上下文防线（L2 prune / L3 compaction）作用于 drain 之后的本轮史，补充说明与其它历史消息同等对待。

#### Scenario: 请求边界送达并进入本轮史

- **WHEN** 一轮内模型刚收完一批工具结果、下一请求尚未发出，用户经 `agent_steer` 注入一条补充说明
- **THEN** 下一请求的史内该补充说明以一条 `Message::user` 出现在 `tool_results` 之后（MUST NOT 并入 `tool_results` 那条消息）；转录内出现一条 `role = user` 的密封事件（seq 由内核单点盖戳、单调递增）；命令在 ack 回流后返回 `Ok`；全程同一 `session_id`、无新会话、无新轮

#### Scenario: 多条不合并

- **WHEN** 同一请求边界前到达三条补充说明
- **THEN** 该边界一次 drain 取走三条，产出三条独立 `role = user` 密封事件与三条独立 `Message::user`，并逐条 `ack.send(())`（三条命令各自返回 `Ok`）；MUST NOT 合并成一条消息、MUST NOT 让后两条滞留到下一个边界

#### Scenario: 收口即 Err 且不排队

- **WHEN** 轮已收口（`converge` 已执行、接收入端已 drop），用户再经 `agent_steer` 注入
- **THEN** 命令返回 `Err`「会话已收口，补充说明不可送达」；MUST NOT 排队、MUST NOT 以 Continue 重开一轮投递、会话状态零改动

#### Scenario: ask 阻塞期间不可送达且判据优先

- **WHEN** 模型正阻塞在 `ask_user_question`（`AskRegistry` 有该会话的等待方，且 `SteerRegistry` 条目仍在场），用户注入补充说明
- **THEN** 命令返回 `Err`「等待应答中，补充说明不可送达」；该文本 MUST NOT 进入通道、MUST NOT 在应答后补投（无排队），`SteerRegistry` 条目零变化

#### Scenario: 停止使在途补充说明收 Err

- **WHEN** 补充说明已入通道但尚未到达请求边界时用户停止该会话
- **THEN** 该轮收敛 `stopped`；命令收 `Err`（「会话在补充说明生效前结束」）；转录 MUST NOT 出现该文本的任何事件；注册表该键由 `converge` 除名（其后 `request` → `Err`）

#### Scenario: 注册表与停止表同址登记除名

- **WHEN** 开轮 → 送达 → 收口走一遍
- **THEN** `begin_turn` 后两表（`StopRegistry` / `SteerRegistry`）都能查到该会话键；`converge` 后两表都查不到，`request` 与 `request_stop` 分别按各自语义落空（前 `Err`、后 miss 幂等 `false`）

#### Scenario: 无 SystemNotice 变体

- **WHEN** 审查事件词汇与适配层分派表
- **THEN** `AgentEventKind` 零新增变体；补充说明只以 `Message{role: User}` 出现，`ask` / `finish_step` / `turn_done` 等既有通知词汇零改动

### Requirement: SDK 引擎 effort（思考强度）旁路与协议 shaping

SDK 引擎 SHALL 消费 provider 记录的 `efforts{high, low}`，把「同一模型想多深」作为**旁路载荷**送进请求体，且**两协议皆生效**（effort 无协议门控——两协议都有该语义，只是 wire 形状不同）：

- **载体与分发**：`efforts` 为 provider 记录属性（`desktop-agent-management`）。档位到 `(模型, effort)` 的分发 SHALL 复用**既有唯一分发点**——`SdkRunner::open_session` 里那一个 `match open.ctx.model_level` 扩为二元组：`High → (config.model_high, efforts.high)`、`Low → (config.model_low, efforts.low)`。MUST NOT 另立第二个档位分发点；MUST NOT 在会话内换档（effort 只在轮边界确定：反馈边续注同档、各相位各自开会话）。
- **shaping 单点**：`sdk/effort.rs` SHALL 持有唯一 shaping 纯函数 `params(protocol, effort: &str) -> Option<serde_json::Value>`：两侧空白裁剪后为空 ⇒ `None`（**留空 = 不发**，MUST NOT 填缺省 effort、MUST NOT 发空串）；`Protocol::Openai` ⇒ `{ "reasoning_effort": v }`；`Protocol::Anthropic` ⇒ `{ "output_config": { "effort": v } }`。四个键名（`reasoning_effort` / `output_config` / `effort` 与协议判别）SHALL 由该模块常量单点持有，`loop` / `runner` / 门面 / 组合根 MUST NOT 出现这些字面量。函数 MUST NOT 校验值域（自定义值原样透传，与保存期「只 `trim`」同一条纪律）。
- **穿透链**：`ResolvedEngine.efforts`（组合根原样带出，**零门控**）→ `EngineFacade::with_efforts` → `SdkRunner::with_efforts` → `PumpPayload.effort_params`（已 shaping 的 `Option<serde_json::Value>`）→ `LoopTurn.effort_params` → 请求 `additional_params`。`EngineConfig` 连接配置字段面 MUST NOT 增字段（与 `context_length` / `max_tokens` / `web_search` 同族走门面旁路），`runner_for` 唯一 match 点结构与签名 MUST NOT 塌。
- **与 `web_search` 共存**：两者共享 `additional_params` 这同一个位；实现 SHALL 依赖 rig 的按键合并（`provider_tool` 把 `tools` 并入既有 `additional_params`），两键 MUST NOT 互相覆盖，次序无关。
- **留空零痕迹**：两档皆空时 `effort_params = None` ⇒ `additional_params` 保持 `None` ⇒ 请求体 MUST NOT 新增任何键（openai 臂不出现 `reasoning_effort`，anthropic 臂不出现 `output_config`）——这是「非 DeepSeek 的第三方兼容网关不被污染」的保障，也是存量行为零变化的判据。
- **诊断面**：引擎泵启动日志 SHALL 记该轮生效 effort（与 `protocol=… max_tokens=… web_search=…` 同列），留空记 `-`（与 `max_tokens` 的 `-` 口径同型）。
- **摘要请求不带 effort**：`sdk/compact.rs` 的摘要请求 SHALL 保持 `additional_params: None`（摘要 prompt 形态与主会话前缀不同、不共享缓存条目，带 effort 只增成本）；本变更 MUST NOT 在摘要路径引入第二条 shaping 调用点。
- **缓存纪律**：anthropic 臂的自动前缀缓存（`with_automatic_caching`）要求会话内 top-level effort 恒定——本变更由「档位只在轮边界确定」结构保证；「同一会话内换 effort」属后续能力，落地时 MUST 重审本条（不是免费功能）。
- **事件面中立**：effort MUST NOT 进入 `AgentEvent` 词汇、MUST NOT 进入 `SessionConfigSnapshot`（会话快照不记 effort，续跑按当次档位重新解析）、MUST NOT 进入 `crates/core/agent`。

#### Scenario: 留空零痕迹

- **WHEN** provider 的 `efforts` 两档皆空（或纯空白）时分别以 `protocol = openai` 与 `protocol = anthropic` 发起一轮
- **THEN** 两次请求的 `additional_params` 恒为 `None`（openai 臂无 `reasoning_effort`、anthropic 臂无 `output_config`），请求体与变更前**逐字相同**；引擎日志记 `effort=-`

#### Scenario: 两臂 wire 形状

- **WHEN** 档位 `High` 且 `efforts.high = "max"`，分别以两协议发起
- **THEN** openai 臂请求 `additional_params` 恰为 `{"reasoning_effort":"max"}`；anthropic 臂恰为 `{"output_config":{"effort":"max"}}`；两者均无多余键

#### Scenario: 档位分发单一

- **WHEN** `efforts = { high: "max", low: "low" }`，分别以 `ModelLevel::High` 与 `ModelLevel::Low` 发起
- **THEN** 前者请求携 `max`、后者携 `low`（各自与同档模型名成对）；审查源码时档位到 `(模型, effort)` 的分发只出现在 `open_session` 一处，`loop` / 门面 / 组合根零协议判定与零档位判定

#### Scenario: 与 web_search 共存

- **WHEN** `protocol = anthropic` 且 `web_search = true`、`efforts.high = "high"`，以 `High` 档发起
- **THEN** 请求 `additional_params` 同时含 `output_config.effort` 与 `tools`（`web_search` provider tool），两键互不覆盖；关掉 `web_search` 后仅 `tools` 键消失、effort 键原样在场

#### Scenario: 自定义值原样透传

- **WHEN** `efforts.high = "  ultra-thinking  "`（目录外取值、带空白）
- **THEN** 请求携 `ultra-thinking`（裁剪空白后原样发送，引擎侧零值域判定、零拒收）；provider 侧 400 以既有 `api_error` 显式暴露

#### Scenario: 摘要请求不带 effort

- **WHEN** 一轮长历史触发 L3 摘要（`compact::summarize`）
- **THEN** 摘要请求的 `additional_params` 仍为 `None`（不带 effort、不带 provider tool），主请求的 effort 载荷不受摘要影响

#### Scenario: 事件与快照零扩展

- **WHEN** 以非空 `efforts` 发起一轮并查看事件流与 `SessionConfigSnapshot`
- **THEN** `AgentEvent` 词汇与字段面零变化（无 effort 字段）、`SessionConfigSnapshot` 结构不变（不记 effort）；「本轮用什么深度」只出现在引擎日志

### Requirement: 会话系统提示词快照的拍存与续轮复用

会话的系统提示词 SHALL 以**一段会话一份快照**为语义：快照在会话首轮拍存、此后逐字复用，MUST NOT 逐轮重算。持久载体为 store 的 `SessionPreambleRecord`（workspace 维度、主键 `session_id`；模型形态与写一次语义单源见 `desktop-workspace-store`「会话系统提示词快照记录（写一次冻结）」）。三处判据与落点：

- **首轮判据**：`SessionRef::New`，或 `SessionRef::Continue{id}` 且 `Store::is_unstarted_session(id)` 为真（无引擎句柄 + 无轮行 + 无转录——与既有「首轮定型」`restamp_session_config` 同一判据来源，MUST NOT 另立第二套）。首轮 SHALL 在 `SessionCtx.workspace_root`（会话执行根；worktree 场景即分支版）读取一次 `AGENT.md`，与装配层知识合成快照正文（合成次序与截断口径见「SDK 引擎系统提示词（AGENT.md 注入）」），本轮直接使用该内存产物。
- **落库时机**：快照 MUST 在会话行与轮行建立之后落库（`New` 路径只有此刻才知会话 id），MUST NOT 早于会话建立；本轮 MUST NOT 依赖读回自己刚写的快照。落库经内核组合根直写 store（沿 `restamp_session_config` 同处同型先例），MUST NOT 为此扩 `SessionSink` 端口契约。
- **读点（续轮复用）**：`Continue` 且会话已开轮 SHALL 以 `Store::find_session_preamble(id)` 的正文**覆盖** `SessionInjections.preamble`；MUST NOT 读取 `AGENT.md`、MUST NOT 重算知识段。快照行存在而正文为空串 → 覆盖为 `None`（该会话零 System 消息，见「SDK 引擎系统提示词（AGENT.md 注入）」）。
- **存量补拍**：已开轮但无快照行的会话（本变更前创建）SHALL 在其下一次 `Continue` 时按首轮口径现算 + 落库 + 使用；此为首轮之外的**唯一**现算入口，且至多发生一次（写一次）。该次切换令该会话前缀一次性重建（缓存 miss 一次），属显式接受的代价。
- **写一次与不可变**：`stamp_session_preamble` SHALL 为写一次语义——行已存在即不再写入；快照一经落库 MUST NOT 被覆盖，MUST NOT 提供重载 / 刷新 / 编辑的命令面或前端写面。
- **失败降级**：快照落库失败 SHALL NOT 中断本轮（本轮使用内存中已装配的正文，日志留痕——沿 `bind_remote_session` 尽力语义先例）；该会话此后按存量补拍路径再尝试一次。
- **知识版本语义**：应用升级后旧会话 SHALL 沿用拍存时刻的知识文本与 `AGENT.md` 内容（前缀稳定优先），新会话自然取新内容；`AGENT.md` 缺席时拍下的快照 MUST NOT 因该文件后来出现而补入该段。
- **元信息**：快照 SHALL 落库 `saved_at`（拍存时刻，由 store 侧时钟取，沿「时间戳归落库侧」口径）以便出线呈现「冻结于何时」；其余元信息 MUST NOT 冗余落库——体量按正文长度导出即可；「用户改了 `AGENT.md` 但本会话未生效」的检测与提示面不在本能力内（未来落地时按既有 native_model 版本链演进字段面，另立变更）。
- **core 边界**：快照的合成 / 落库 / 读回全在 infra（内核组合根 + store）；`crates/core/agent` 只承载字符串（`SessionInjections.preamble`），MUST NOT 出现「快照」/「AGENT.md」概念，字段面零结构改动。
- **载荷纪律**：快照正文 MUST NOT 随会话清单返回（出口见「session_detail 单查面」）；清单读面 MUST NOT 触碰快照表。

#### Scenario: 首轮拍存与正文合成

- **WHEN** 以 `source = "archi"` 发起 `New` 会话且 workspace 执行根存在 `AGENT.md`
- **THEN** 会话行与轮行建立后库中出现该会话的 `session_preamble` 行，正文为「架构会话知识 + 空行 + `<agent_md …>` 段」，`saved_at` 在场；本轮请求的 System 消息逐字等于该正文

#### Scenario: 续轮逐字复用且零文件读取

- **WHEN** 某会话首轮收口后修改（或删除）`AGENT.md`，随后以 `Continue` 续发下一轮
- **THEN** 下一轮 System 消息与首轮逐字相同（本轮不读文件、不重算知识）；即便该文件已不可读，续轮照常完成

#### Scenario: 存量会话补拍一次后冻结

- **WHEN** 一条本变更前创建、已开轮且无快照行的会话首次 `Continue`，随后修改库中该快照正文再次 `Continue`
- **THEN** 首次续轮现算并把正文落库（该轮即用新正文，前缀一次性重建）；第二次续轮仍使用首次落库的正文（写一次语义，二次写入不生效）

#### Scenario: 空快照不注入也不补入

- **WHEN** `source = "debug"`（无知识）且执行根无 `AGENT.md` 时首轮，之后在同一执行根创建 `AGENT.md` 再续轮
- **THEN** 首轮快照正文为空串、请求零 System 消息；续轮读回空正文仍零 System 消息，MUST NOT 因文件后来出现而改变本会话前缀

#### Scenario: 落库失败不中断本轮

- **WHEN** 首轮快照落库失败（库写错误注入）
- **THEN** 本轮照常跑完（System 消息使用内存中已装配的正文），日志留痕，MUST NOT 抛发起点 `Err`、MUST NOT 产生半成品会话

#### Scenario: 升级后旧会话沿用旧知识

- **WHEN** 某知识注入会话首轮拍存后，应用的 `archi` / `config` 知识文本随版本变化（测试缝替换生产者）再续轮，同时开一个新会话
- **THEN** 旧会话续轮 System 仍为首轮拍存的正文（旧知识）；新会话取其时刻的新知识

### Requirement: `write` / `edit` 的接口定义写闸（坏文本不可产出）

`write` 与 `edit` 两个内建工具 SHALL 在落盘前对**接口定义文件**做一次内存校验，失败即 `Err` 且零落盘：

```text
落盘前：目标路径（sandbox 之后的绝对路径）落在 <root>/<archi_models_rel_dir>/ 之下
        ∧ 文件名主干是该目录内某元素的 FQN（形如 <FQN>.schema.json）？
  ├─ 否 → 原样落盘（零改动、零额外 IO）
  └─ 是 → 调 archi::api::schema::validate_schema(<FQN>, <候选文本>)
           ├─ Err → 工具 Err（第一行自足：文件 + 期望值 + 一条出路），零落盘
           └─ Ok  → 落盘，响应追加「接口已校验」一行
```

- **同一实现**：写闸 SHALL 调 `desktop-archi-api`「接口定义文件的装载校验与写闸（零第二份校验）」定义的**同一个** `validate_schema`；装载期（`load_contracts`）也调它。MUST NOT 在本层另写一份 JSON / JSON Schema 校验，MUST NOT 只校验部分面。
- **射程封闭**：只有 `<archi_models_dir>/**/<FQN>.schema.json` 进校验面；其余一切路径（普通源码 / 文档 / 模型 TOML / `INDEX.toml` / `.i.rs` / `.i.ts`）**写前零校验**的既有承诺逐字不变。目录前缀 SHALL 经 `foundation::layout` 派生（MUST NOT 行内 `models` / `archi` 字面量）。
- **`root` 穿参**：`write` / `edit` 的执行体 SHALL 接收会话执行根 `root`（`tools::execute(root, name, input)` 已持有），用于派生接口目录前缀；`tools::execute` 的对外签名 MUST NOT 改动。
- **`edit` 校验替换后的正文**：校验对象 SHALL 是 `old_string` → `new_string` 替换后的完整候选文本，MUST NOT 是替换前的旧文。
- **失败语义**：`Err` 文案第一行自足（含文件、期望值、出路之一），MUST NOT 只写「非法」；`Err` 由模型在同一 turn 内自纠重试，**run 不中断**（与既有工具 `Err` 同构）。
- **不属保护面**：本闸判「文件级自洽」，MUST NOT 触碰 `ProtectRules`、`policy` 三档、路径沙箱或写入保护的任何判定；写权仍只由相位锁表达（见 `desktop-write-protection`）。工具档位不变（`write` / `edit` 的既有档位语义逐字不动）。

#### Scenario: 三类坏输入各一条 Err 且零落盘

- **WHEN** 分别以「截断的 JSON」「`$id` 与文件名不符」「`T` 同时作 op 键与 `$defs` 键」经 `write` 落 `<models>/…/<FQN>.schema.json`
- **THEN** 三次均 `Err`（文案分别点名解析位置、期望 `$id`、冲突键与两个键空间），盘上字节零变化（新建文件不产生、既有文件不被截断或半写）

#### Scenario: 合法落盘并回执

- **WHEN** 以 `write` 落一份合法 schema
- **THEN** 落盘成功，响应含「接口已校验」；随后 `load_contracts` 装载该文件零错误

#### Scenario: edit 校验候选正文

- **WHEN** 以 `edit` 把一份合法 schema 的 `title` 改成另一名字
- **THEN** `Err`（identity 闸），盘上字节零变化；改成正确 `title` 后落盘成功并回执

#### Scenario: 非接口路径零校验

- **WHEN** 以 `write` 落一份内容含坏 JSON 的 `*.md` / `*.ts` / `INDEX.toml` / 模型 `*.toml`
- **THEN** 四者均原样落盘、零校验、零「接口已校验」行（射程封闭）；坏模型仍由读取面报结构诊断（既有语义逐字不变）

#### Scenario: 写闸不越权判跨元素

- **WHEN** 以 `write` 落一份 `$ref` 指向「不在模型内的 FQN」的 schema（前缀合法）
- **THEN** 写闸放行（跨元素事实不属文件级自洽）；该问题由后续检查以 `apiRefMissing` 报出——MUST NOT 在写闸里产出第二份跨元素判据

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/agent`（裸名 `agent`） | 会话域内核 | `AgentEvent` 双层词汇（+`MessageDelta`；块词汇五变体含 `WebSearch` 与 `Text.citations`；TurnDone 统一口径）；P1 引擎协议（spawn/question/stop/观察回流 + 四不变量）；`run_session` / `query_sessions` 公共 API；治理面（盖戳/状态机/停止注册）；持久面（write-through）与查询契约；port 定义（core 定接口、infra 实现，落点 design 裁定）；无直接 IO、无 claude/引擎/Tauri、无 workspace 内依赖 |
| `crates/core/agent/src/kernel.rs` | 内核收口缝 | `drive` 以观测通道关闭（引擎侧单边 EOF）为唯一收口缝（无状态机 break / stop select / 第二收口路径，agent-turn-eof 裁定） |
| `packages/desktop/src-tauri/crates/infra/agent`（裸名 `agent-runtime`） | 引擎门面 + 双引擎实现 | `EngineKind`（`cli` \| `sdk`，specta camelCase）；`runner_for(kind, engine_cfg)`（唯一 match 分发）；`EngineConfig{protocol, api_key, base_url, model_high, model_low}`（连接配置字段面，协议在内）由内核组合根注入（机密面放宽注释标记见 desktop-agent-management 边界表）；SHALL 实现内核 port（store 持久面与查询面落地）；`rig` facade 恒编译（`=0.43.0` 锁版 + 显式 features）；`EngineFacade::with_context_window` / `with_max_tokens` / `with_web_search` / `with_efforts` 承接 provider `context_length` / `max_tokens` / `web_search` / `efforts`（不经 `EngineConfig`） |
| `crates/infra/agent/src/cli/` | CLI 引擎协议实现 | `session_pump` 一轮一命（单问服务后返回 → 内核 EOF 收口；停止/spawn 失败单轮终止语义不动，agent-turn-eof）；flag 组装 `--disallowedTools ask_user_question` 恒居参数序列末位（`--resume` 之后）；jsonl 泵与 resume flag 行为延续；恢复走 engine_handle 原生 `--resume` |
| `crates/infra/agent/src/sdk/` | SDK 引擎协议实现 | `normalize.rs` 双层化（Text/ReasoningDelta → delta、轮末密封）；`loop.rs` 密封收口；`resume.rs` 会话全史重建；`runner.rs` 协议实现且 `session_pump` 一轮一命（跨轮史槽不存，会话史唯一源 = store 全史转录重建，agent-turn-eof）；工具面/权限/沙箱语义见 SDK 各专章（AGENT.md 注入 / L1 工具质量 / bash / 三档分化 / L2-L3 防线） |
| `crates/infra/agent/src/sdk/{loop,normalize,resume,runner,tools,policy,config}.rs` | facade 路径迁移 | `rig_core::` → `rig::`（纯 re-export 同名）；`rig_core::` 路径零残留 |
| `crates/infra/agent/src/sdk/preamble.rs` | 系统提示词快照合成（**行替换**：原「起播前读 + 每轮重读」表述退役） | `load(root)` 读取 + 32KB char 边界截断 + `<agent_md>` 包装（签名与口径零改动，调用方由引擎改为组合根）；`freeze(knowledge, agent_md)` 合成单点（知识 → 空行 → `AGENT.md` 段，空段跳过、全空 → 空串）；零 core 改动 |
| `crates/infra/agent/src/sdk/tools.rs` + `bash.rs` | 七工具面与质量 | `TOOL_NAMES` 七项；read 截断 + offset 翻页 + >100KB 无分页参数时前 100 行预览；grep 正则 + 上下文行；全工具单结果字节上限（`MAX_RESULT_BYTES` 50KB）；bash `tokio::process` + git-bash 探测 / `cmd /C` 兜底 / unix `sh -c`；`kill_on_drop` + `taskkill /T /F` 形状清理；超时活性护栏（缺省 120s、范围校验 [1s, 30min]，越界显式拒绝） |
| `crates/infra/agent/src/sdk/policy.rs` | 三档决策表（bash 分化） | bash：Default 拒、AcceptEdits / Bypass 放；AcceptEdits ≠ Bypass 首次真实分化锁定测试 |
| `crates/infra/agent/src/sdk/{context,compact}.rs` | L2 / L3 防线 | 标称窗长 = 该档模型名 `[1m]` 后缀 > provider `context_length` > 128K 启发式（按档独立）；生效窗长 = 标称 × 60% 与 400k 取小（水位一律据此推导）；字节启发式 token 估算（bytes ÷ 4）；prune 占位符（8k 门槛）+ 轮对裁剪（迟滞：剪到触发位 60% 目标，保护窗 + 首条 user 为地板）+ 保首条 user；LLM compaction（失败降级硬裁，run 不中断）；`context_pruned` / `context_compacted` SystemNotice；`TurnDone.usage.requests` 逐请求样本（`defense` 归因）；调用点 = resume 重建后 + 每请求前；store 转录全量不变 |
| `crates/infra/agent/src/sdk/reasoning.rs`（新） | 思考块投影与回放 | `ISSUER = "anthropic"`；`project(&Sealed<Reasoning>) -> Vec<AgentBlock>`（逐载荷成块，不拼接）；`replay(thinking, signature, redacted) -> Option<AssistantContent>`（凭证即回放权，redacted 优先）；`strip_all(history)` 跨轮剔除（重建史入口单点，滤空消息原样保留防空白消息）；`without_reasoning(message)` 单条剔除口径（摘要转录用） |
| `crates/infra/agent/src/sdk/blocks.rs`（新） | 密封块双向分派 | `project_choice` / `assistant_content` / `user_content` 三入口；与增量层 `normalize` 对称——遍历与变体分派单点，域模块只持 wire 原语 |
| `crates/infra/agent/src/sdk/resume.rs` | 重建与思考回放口径 | `assistant_content` 带凭证的 `Thinking` 块经 `reasoning::replay` 回放；跨轮剔除不在重建侧（`runner::open_session` 重建后以 `reasoning::strip_all` 单点执行） |
| `crates/infra/agent/src/sdk/runner.rs` | 模型装配 | anthropic 臂开启自动前缀缓存（top-level `cache_control`）；`[1m]` 后缀追加 beta；输出上限取值链（末档按标称窗长分档推导）；`window=` / `max_tokens=` 日志报生效值 |
| `crates/core/agent/src/runner.rs` | 会话级注入面词汇 | `SessionInjections` 增 `toolset: Option<SessionToolset>`（首值 `Explore`）；`tools` 位激活为七工具子集收窄表达；`preamble` 位维持不消费；serde additive（缺省读兼容旧线格式） |
| `crates/infra/agent/src/compose.rs` | 注入装配 | `begin` 按 `provenance.source == "explore"` 装配注入（六工具收窄 + `Explore` 枚举）；per-turn 专用工具执行上下文铸造（store 句柄 + root）；其余来源恒 default |
| `crates/infra/agent/src/sdk/loop.rs` | 注入面组装与硬禁用 | definitions 按注入面组装；`RunStarted.tools` 报实际下发面；执行前成员校验（`tool_unavailable` 拒绝，run 不中断）；专用工具经 `LoopTurn` 上下文路由 |
| `crates/infra/agent/src/sdk/tools.rs` | 定义面参数化 | 定义面组装参数化 + `create_explore_note` 定义（name / content 入参）；`execute` 对七通用工具保持 `(root, name, input)` 纯函数 |
| `crates/infra/agent/src/sdk/policy.rs`（create 输入档位） | 写面档位扩展 | `create_explore_note` 归写面档位（Default 拒绝、AcceptEdits / BypassPermissions 放行） |
| `crates/infra/agent/src/sdk/`（create 执行体，新） | 定名执行 | 执行时按会话指针圈定当前 explore 记录；占位 → 前置校验 + 落盘 + in-place 改名；已定名 → 同名补建 / 异名 `Err` 附路径；失败面显式 `Err` 附恢复指引 |
| `crates/core/agent/**`（注入面除外）、`crates/infra/agent/src/cli/**` | 零 diff 面 | CLI 引擎不动；core 仅注入面词汇增位（`toolset`），其余契约不变 |
| `crates/infra/store` | 会话转录唯一家 + 查询面实现 | `SessionRecord` + 转录单表（session 键）+ 轮统计行；write-through 原子操作面；会话查询/对账 API；详见 desktop-workspace-store |
| `src/commands/exec/agent.rs` | 内核薄包装命令面 | 三件事纪律；编排退役下沉；会话域查询/终止命令；`Result<T, String>` 模板；命名与参数面 design 定稿 |
| 引擎门面 + 内核组合根 | 解析单点（自命令层下沉） | `runner_for` 签名零 diff、`EngineConfig` 连接配置字段面（含协议）零塌；默认 agent 解析语义不变（desktop-agent-management） |
| `packages/desktop/src/lib/agent-adapter.ts` | 事件适配双层 | delta → 本轮唯一 assistant 消息的 step 部件、密封整块追加；`foldSessionMessages` 按轮分组（轮行挂 `metadata.record`）且仅消费密封转录；详见 desktop-agent-chat-infra |
| `packages/desktop/src/lib/agent-transport.ts` + `src/hooks/use-agent-session.ts` | 会话域传输与状态基建 | 会话域 invoke/流参数（发送期发起流 / 旁观期订阅流双起点）；重放装载走会话转录；详见 desktop-agent-chat-infra |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 调试页参数面 | agent 选择器（默认选中默认 agent，仅调试页暴露）；对话视图 / 落库 / 重放组件零改动复用 |
| `packages/desktop/src/views/agent/components/agent-run-form.tsx` | run 表单 | agent 选择 state（选中 id 随会话发起 invoke 传参；`engine` state 与下拉退役） |
| `packages/desktop/src/components/app-sidebar.tsx` | 页面导航组 | [变更] [Agent 调试]；本地 state 切视图，无路由 |
| agent 会话域前端 hooks | 流订阅 + 查询 | Tauri Channel 实时订阅（执行流通道例外）+ invoke 会话查询重放；查询仍显式触发 |
| `packages/desktop/src-tauri/Cargo.toml`（workspace 依赖） | rig facade 切换 | `rig` 条目：`=0.43.0` 锁版、`default-features = false`、features `reqwest` + `native-tls` + memory（单条目定稿，双条目解析钉子退役——见依赖足迹条目）；`regex` 条目；members 行与目录名不变；测试构建另经 agent-runtime 的 dev-dependencies 加 `rig` `test-utils` feature（流缝 mock 装置仅测试启用，不入生产构建） |
| `packages/desktop/src-tauri/src/bindings/` | IPC 类型镜像 | 经 export-bindings 再生成（信封与命令面演进），非手改；快照测试同步再生成 |
| `crates/core/agent/src/port.rs` `SessionQuery` | 单查契约 | 增按 id 单查方法（会话行 + 统计 + 轮行，running 自轮行推导）；`list_sessions` 演进为分页签名（`offset` / `limit` → `SessionPage`）；`transcript` 不动 |
| `crates/infra/agent/src/store_port.rs` | 单查实现 | 以 store `find_session` 为底座（store schema 零变更）；workspace 库隔离 |
| `src/commands/exec/mod.rs` | `session_detail` 命令 | 薄包装三件事纪律；不存在 id → `Err`；blank root → 空结果；builder 注册 + bindings 再生成 |
| `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 来源筛选参数化 | 去硬编码 `'debug'`；三态筛选重查；显式刷新模式不变 |
| Agent 调试页历史区组件 | 筛选 UI | debug / change / 全部，默认 debug；data-testid 挂钩，不以样式类名查询 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 发起命令参数面增位 + chat 清单命令 | `agent_start` / `agent_start_with` 尾部 `cwd: Option<String>`（位置索引 8，`None` → `root`）；`cwd` 只赋 `SessionCtx.workspace_root`，`stores.for_root(&root)` 恒不动（双 root 不变量）；`cwd` 非空做 `is_dir` 探测并以显式 `Err`（携路径）拒绝；`source_ref` 可携来源引用（`None` 保持既有缺省）；新增 `list_change_chat_sessions` 薄包装（`source = "change"` 常量 + `source_ref = change id` → `SessionPage`，blank root → 空页） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 命令单一登记面 | `all_commands!` 增 `list_change_chat_sessions`（builder 注册与 bindings 再生成随之） |
| `src/commands/exec/mod_test.rs` | 命令面直测 | `cwd` 三态（缺省回落 / 有效值进 `SessionCtx` / 非目录显式 `Err`）+ store 半边恒 root（断言会话行落 workspace 库）；chat 清单分页与来源不撞 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 绑定形状守卫 | `agentStart` 逐字签名断言以尾部 `cwd` 收尾；新命令在清单断言内 |
| `crates/infra/agent/src/{worker.rs,compose.rs,sdk/}` | 零改动（cwd 消费面） | `worker.rs` 已以 `turn.root` 赋 `SessionCtx.workspace_root`；沙箱根 / bash `current_dir` / `AGENT.md` 读取点均随该字段，注入装配仍只特判 `source == "explore"`（change 来源恒 default 七工具） |
| `crates/core/agent` / `crates/infra/store` | 零改动 | 会话内核契约、`SessionProvenance` 形状、store schema 与会话清单过滤语义（`source_ref` 精确相等）均不动 |
| `crates/infra/agent/src/sdk/loop.rs` | 守卫链第四段 | 顺序 `成员校验 → policy → sandbox_rewrite → 写入保护闸 → 执行`；闸门读 `turn.protect.evaluate(name, input, cwd)`，命中走既有 `deny(sender, "permission_denied", name, reason, id)` 后 `continue`；零 IO、零新事件词汇 |
| `crates/infra/agent/src/sdk/loop.rs`（`LoopTurn`） | 规则携带 | 新增 `protect: ProtectRules` 字段（与 `defense` / `retry` / `liveness` 同列，装配点算好、loop 纯消费） |
| `crates/infra/agent/src/sdk/runner.rs` | 装配点 + root 持有 | `SdkRunner.workspace_root: Option<PathBuf>`；`with_workspace_root(self, PathBuf) -> Self`（与 `with_max_tokens` 同型旁路缝）；`PumpPayload.workspace_root`；泵起播时 `protect::load(payload.workspace_root.as_deref())` 装入 `LoopTurn` |
| `crates/infra/agent/src/lib.rs`（`EngineFacade`） | 门面缝 | `workspace_root: Option<PathBuf>` + `with_workspace_root`；`runner_for` sdk 臂透传给 `SdkRunner`；`EngineKind::Cli` 臂与唯一 match 点结构零改 |
| `crates/infra/agent/src/compose.rs` | 组合根签名 | `compose_turn(stores, registry, streams, store, agent, workspace_root: &Path)`（新增尾参）；只做透传，零解析残留 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 调用点（exec 轨） | `compose_turn` 补 `root`（主仓 root；`exec_root` 仍只进 `SessionCtx.workspace_root`） |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | 调用点（change 轨） | 同上：`compose_turn` 补 `root`，MUST NOT 传 `exec_root`（worktree） |
| `crates/infra/agent/src/sdk/protect/**` | 规则与判定 | 契约见 desktop-write-protection「Module Contract」；本能力只负责接线，MUST NOT 在其内实现第二份 glob / 文案 |
| `crates/core/agent`、`crates/infra/agent/src/sdk/{bash,tools,policy,sandbox,explore_note,context,compact,resume,model_spec}.rs` | 本变更零语义改动 | policy 三档矩阵 / 沙箱逃逸判定 / bash 执行体 / 专用工具执行面逐条不变；`SessionCtx` / `SessionOpen` / 事件词汇零扩展 |
| `crates/core/agent/src/runner.rs`（`SessionToolset` 词表） | 注入词表扩展 | 值集 = `Explore` \| `Archi { module: bool, search: bool }`；serde camelCase additive（旧线缺字段读 `None`） |
| `crates/infra/agent/src/compose.rs`（注入装配双臂） | 注入装配矩阵 | explore 臂逐字不变；change 臂按 `source_ref` 尾三段 `{phase}/{role}/{attempt}` 查矩阵；未命中 / 解析失败 fail-closed；命中时 `tools = None` + `toolset = Archi{..}` |
| `crates/infra/agent/src/sdk/tools.rs`（definitions 组装） | definitions 组装 | 按 `toolset` 位追加 archi 工具定义；`RunStarted.tools` 与成员校验自 definitions 派生（下发面单源不变） |
| `crates/infra/agent/src/sdk/policy.rs`（只读面扩展） | 只读面扩展 | `READONLY_TOOLS` += `archi_module` / `archi_search`；写面 / 执行面清单零变化 |
| `crates/infra/agent/src/sdk/loop.rs`（执行臂路由） | 执行臂路由 | 两个 archi 名路由至专用执行体（上下文缺席以 `is_error` 回灌），成员校验先于 policy 的次序不变 |
| `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | 信封与盖戳入口 | `AgentEvent` 增 `turn_id: Option<i64>`（`#[serde(default)]`，无 `skip_serializing_if`，线格式顶层 `turnId`）；`AgentEvent::stamp(turn_id: Option<i64>, seq: u64, kind: AgentEventKind)` 首参承载轮归属——唯一生产入口，漏写即编译错误；文档显式区分「轮 id」与 `TurnDone.session_id`（引擎侧标识） |
| `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 盖戳治理单点 | `drive` 观察泵内 `AgentEvent::stamp(Some(turn_id), seq, kind)`（`turn_id` 取自 `RunningTurn` 解构绑定）；收敛 / 落库 / 注册表发布零改动；两实时出口同源同值 |
| `packages/desktop/src-tauri/crates/core/agent/src/event_test.rs`、`kernel_test.rs` | 契约直测 | 线格式 `turnId` 键恒在（`None` → `null`）/ 旧 JSON 缺键读出 `None` / 往返保真 / 同轮全事件同值 / 续轮不串轮 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/**` | 零改动（轮归属不进引擎） | `LoopTurn` / `SessionOpen` / `PumpPayload` / 观察通道载荷（`AgentEventKind`）签名与语义逐条不变；`turn_id` / `turnId` 零字样 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 出线形状守卫 | `agent_event出线为kind判别联合且seq摊平变体字面量camel_case` 追加 `turnId?: number \| null` 与 `turnId` 摊平断言；既有 `seq` / `timestampMs` / 六变体字面量断言零改动 |

### API: `packages/desktop/src-tauri/src/commands/exec/mod.rs`（命令面）

| 命令 | 签名 | 描述 |
|------|------|------|
| `agent_session_watch` | `(on_event: Channel<AgentRunMessage>, root: String, session_id: String, since_seq: u64) -> Result<(), String>` | 会话级实时流订阅（薄包装）：注册表订阅 → Channel 前向（`Event` / 末拍 `Record`）→ 结束释放；无运行轮 / blank root 幂等立即结束 |
| `agent_start`（参数面增位） | `(on_event: Channel<AgentRunMessage>, root, prompt, permission_mode, session_id, source, source_ref, agent, cwd) -> Result<TurnSummary, String>` | 提前 resolve 保持；发起流作为发送方实时出口保留（与订阅面同源）；`cwd` 尾部可选参数（位置索引 8，`None` → `root`），只赋 `SessionCtx.workspace_root`、store 半边恒 `for_root(root)`；`cwd` 非空先 `is_dir` 校验并显式 `Err`（携路径） |
| `list_change_chat_sessions`（新） | `(root: String, id: String, offset: u32, limit: u32) -> Result<SessionPage, String>` | 薄包装（参数转换 → `session_query` 分页 → 错误映射）：`source` 常量 `"change"` + `source_ref = id`；blank root → 空页信封 |

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

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `build_model` | `fn build_model(config: &EngineConfig, spec: &ModelSpec) -> DynModel<Completion>` | 协议唯一分叉处：openai 臂现状等价；anthropic 臂条件 `with_beta` + 定型 wire 默认 `max_tokens`；两臂同产 `DynModel<Completion>` |
| `SdkRunner::open_session` | 签名不变 | 按档 `parse_model_spec(raw)`（`Err` → `ConfigMissing`）、窗长按档现算 `spec.length.or(self.context_window)`、裸名下发 |
| `SdkRunner::with_max_tokens` | `fn with_max_tokens(self, max_tokens: Option<u64>) -> Self` | 旁路 builder（`new` 签名不动，既有调用点零改）；与 `context_window` 并列的解码后载荷 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs`（新）

| 项 | 签名 | 描述 |
|------|------|----------|
| `parse_model_spec` | `fn(raw: &str) -> Result<ModelSpec, ModelSpecError>` | 纯函数：末尾 `[1m]`（大小写不敏感）→ 裸名 + `1_000_000`；其余方括号形态显式 `Err`；无后缀 → 裸名 + `None`；空白裁剪 |
| `ModelSpec` | `{ name: String, length: Option<u64> }` | 解析产物（`length` 为窗长候选，`None` = 走 `context_length` / 128K） |
| `ModelSpecError` | 携原文与原因 | 保存期与启动期共用（错误串 MUST NOT 含 api_key） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `Protocol` | `enum { Openai, Anthropic }` | 门面协议枚举（映射自 store `AgentProviderProtocol`；两臂 match 于 `build_model`） |
| `EngineConfig` | `{ protocol, api_key, base_url, model_high, model_low }` | 连接配置字段面（协议在内）；`empty()` 给 `Openai`；`is_complete` 语义不变（协议不参与校验）；`context_length` / `max_tokens` MUST NOT 进本结构 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` + `compose.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `EngineFacade::with_max_tokens` | `fn(self, Option<u64>) -> Self` | 与 `with_context_window` 同型旁路缝 |
| `EngineFacade::runner_for` | 签名不变（`type` 双值 / 唯一 match 点） | sdk 臂把旁路载荷转入 `SdkRunner` |
| `resolve_agent_engine` | 内部实现变化 | `provider.protocol` → `Protocol`；`provider.max_tokens` → `ResolvedEngine` 旁路；解析语义（默认 / 显式 / high 档 / cli 拒收）零变 |
| `parse_model_spec` 再导出 | `pub use sdk::model_spec::{parse_model_spec, ModelSpec, ModelSpecError}` | 命令层保存期校验复用同一函数（MUST NOT 造第二套规则） |

### Function: `packages/desktop/src-tauri/crates/core/agent/src/event.rs`

| 项 | 签名 / 形态 | 描述 |
|------|------|----------|
| `AgentEvent.turn_id` | `Option<i64>`（`#[serde(default)]`，线格式顶层 `turnId`，键恒在、值可 `null`） | 轮归属：持久层权威分配的轮 id；一批事件的轮内恒定；与 `TurnDone.session_id`（引擎侧标识）语义不同 |
| `AgentEvent::stamp` | `(turn_id: Option<i64>, seq: u64, kind: AgentEventKind) -> AgentEvent` | 唯一盖戳生产入口（首参即轮归属）；签名扩参使漏写轮归属成为编译错误；`timestamp_ms` 仍取 `now_millis()` |
| `AgentBlock::Text` | `{ text: String, citations: Option<Vec<AgentCitation>> }` | 增 `citations`（`#[serde(default)]`，MUST NOT 加 `skip_serializing_if`：键恒在、序列化落 `null`）；旧 JSON 读出 `None` |
| `AgentBlock::WebSearch` | `{ id: String, query: String, results: Vec<WebSearchSource>, error: Option<String>, raw_use: Option<serde_json::Value>, raw_result: Option<serde_json::Value> }` | 一次 server use 与其配对 result 的原子表达；`raw_*` 为回放唯一事实源 |
| `AgentCitation` | `{ url: Option<String>, title: Option<String>, cited_text: Option<String>, raw: serde_json::Value }` | 引用（`raw` = 原始 wire 值，回放用） |
| `WebSearchSource` | `{ url: String, title: Option<String>, page_age: Option<String> }` | 单条搜索来源（展示字段） |

### Function: `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `RunningTurn::drive` | 盖戳调用改 `AgentEvent::stamp(Some(turn_id), seq, kind)` | `turn_id` 取自 `self` 解构绑定，零新增字段、零新增参数；delta 与密封共用同一盖戳点，两个实时出口（`streams.publish` / `on_output`）投递同一值 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/web_search.rs`

| 项 | 签名 | 描述 |
|------|------|----------|
| 常量面 | `WEB_SEARCH_TOOL_KIND` / `WEB_SEARCH_TOOL_NAME` / `WEB_SEARCH_MAX_USES` / `ANTHROPIC_RAW_CONTENT_KEY` / `CITATIONS_KEY` | 字面量单点（loop / blocks / context 零重复） |
| `provider_tool` | `fn() -> ProviderToolDefinition` | `{type: web_search_20250305, name: web_search, max_uses: 5}` |
| `raw_content` | `fn(&Text) -> Option<&serde_json::Value>` | 取 `additional_params.anthropic_content` |
| `citations_of` | `fn(&Text) -> Option<Vec<AgentCitation>>` | 展示字段自 raw 解，`raw` 保原值 |
| `use_id` | `fn(&serde_json::Value) -> Option<&str>` | `web_search` 的 `server_tool_use` → 配对键（非本域 server 块为 `None`） |
| `result_use_id` | `fn(&serde_json::Value) -> Option<&str>` | `web_search_tool_result` → `tool_use_id`（wild 块为 `None`） |
| `server_block` | `fn(&serde_json::Value, Option<&serde_json::Value>) -> AgentBlock` | use 侧成块，配对 result 载荷并入 |
| `orphan_block` | `fn(&serde_json::Value) -> Option<AgentBlock>` | 孤儿 result 成块（无 `tool_use_id` 返回 `None`） |
| `replay_raw` | `fn(&serde_json::Value) -> AssistantContent` | `Text{text:"", additional_params:{anthropic_content: raw}}`（raw 块必须配空文本） |
| `replay_text` | `fn(&str, Option<&[AgentCitation]>) -> AssistantContent` | 非空 citations → `additional_params.citations = [raw, …]` |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/blocks.rs`（新）

| 项 | 签名 | 描述 |
|------|------|----------|
| `project_choice` | `fn(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)` | 密封块投影唯一实现（遍历 + 变体分派；use+result 收敛、孤儿 result、未知 server 块跳过；`Reasoning` 委托 `reasoning::project`） |
| `assistant_content` | `fn(&[AgentBlock]) -> Vec<AssistantContent>` | 事件块 → 重建载荷（`resume::rebuild` 调用；`WebSearch` 一条块产 use/result 两条，保原序） |
| `user_content` | `fn(&[AgentBlock]) -> Vec<UserContent>` | user 位文本块映射（其余变体跳过） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/reasoning.rs`（新）

| 项 | 签名 | 描述 |
|------|------|----------|
| `ISSUER` | `&str = "anthropic"` | 思考凭证签发方（anthropic wire 的回放判别值） |
| `project` | `fn(&Sealed<Reasoning>) -> Vec<AgentBlock>` | 逐载荷成块（不拼接）：`Text`→文本+签名、`Summary`→仅文本、`Redacted`→密文、`Encrypted`→不产块 |
| `replay` | `fn(&str, Option<&str>, Option<&str>) -> Option<AssistantContent>` | 凭证 → rig reasoning（redacted 优先；无凭证 `None`） |
| `strip_all` | `fn(Vec<Message>) -> Vec<Message>` | 重建史入口整段剔除思考块（滤空消息原样保留） |
| `without_reasoning` | `fn(Message) -> Option<Message>` | 单条剔除口径（摘要转录用） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `LoopTurn` | 增 `web_search: bool` | runner 经 `PumpPayload` 下传的最终开关 |
| 请求构造 | `request.provider_tool(web_search::provider_tool())`（条件） | 开关为真才挂；`tools`（客户端面）不变 |
| 块转换 | 换 `blocks::project_choice(&choice)` | 删内联四变体 match（块语义单一实现） |
| `'attempt` 返回值 | `(choice, usage, finish_reason)` | `CompletionResponse::finish_reason()` |
| 续跑循环 | 常量 `PAUSE_TURN_MAX_CONTINUATIONS = 5` | 独立计数轴（`attempt` 每次续跑前归零）；两条 notice；到顶按 `stop` 收口 |
| `truncation_kind` | `fn(Option<&FinishReason>) -> Option<TruncationKind>` | 精确等值判定 `Length` → `OutputCap`、`Other("model_context_window_exceeded")` → `ContextWindow`；其余 `None`（不误判） |
| `emit_truncated` | `async fn(&Sender, &LoopTurn, TruncationKind, u64, &Usage)` | `SystemNotice{output_truncated}` + 引擎日志双写；载荷 `{kind, turn_index, output_tokens}` |
| 截断收口点 | 位于密封入史之后、`tool_calls` 执行判定之前 | 不重试、不执行工具、不续跑；`finish_success` 收口 |
| `abnormal_finish` | `fn(Option<&FinishReason>) -> Option<(String, String)>` | 白名单判正常终局（`Stop` / `ToolCalls` / `pause_turn` / `None`）；`ContentFilter` → `model_refused`、未知 `Other` → `unexpected_finish` |
| `fail_on_abnormal_finish` | `async fn(&Sender, &LoopTurn, Option<&FinishReason>, u64, Instant, &Usage) -> bool` | 异常终局统一收口（空 choice 与有内容两路径共用）；命中即 `finish_error` 并返回 `true` |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/{resume,context}.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `resume::text_content` | 模式改 `Text { text, .. }`（user 位） | user 侧 citations 不参与 |
| `resume::assistant_content` | 返回值改为逐块可展开的序列 | `WebSearch` → `replay_raw(raw_use)` + `replay_raw(raw_result)`（保原序，双 `None` 跳过）；`Text{text, citations}` → `replay_text` |
| `context::sweep_orphans` | 语义扩展（签名不变） | 索引源 `WebSearch.id` / `raw_result.tool_use_id`；剔 assistant 位无配对 use 的 result 块；剔空整条移除 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/{lib,compose,runner}.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `EngineFacade::with_web_search` | `fn(self, bool) -> Self` | 与 `with_context_window` / `with_max_tokens` 同型旁路缝（四个构造点补齐字段） |
| `compose::ResolvedEngine` | 增 `web_search: bool` | `provider.web_search && protocol == Anthropic`（门控判定单点） |
| `compose::runner_for` 链 | 加 `.with_web_search(..)` | 旁路载荷下传 sdk 臂 |
| `SdkRunner::with_web_search` | `fn(self, bool) -> Self` | `new` 签名不动（调用点零改） |
| `PumpPayload` / `session_pump` | 增 `web_search: bool` | 构造 `LoopTurn` 时传递；启动日志加 `web_search=…` |

### API: `packages/desktop/src-tauri/crates/infra/store`（消费面）

| 项 | 变化 | 描述 |
|------|------|----------|
| `AgentProviderRecord` | 增 `web_search: bool`（v4） | 见 desktop-agent-management Module Contract；本能力只读消费 |
| `AgentProviderProtocol` | 新枚举 | 与本能力门面 `Protocol` 一对一映射（映射点 = 组合根解析单点） |
| 事件转录模型 | 零变更 | `AgentEvent` 新增块变体为自描述 JSON，无需版本演进 |

### 前端消费口径（本变更 delta 面，详述见 desktop-agent-chat-infra）

| 模块 | 变化 | 描述 |
|------|------|----------|
| `packages/desktop/src/lib/agent-transport.ts` | `Record` → `finish` chunk 的 `messageMetadata` | 终态轮行落该回合消息元数据；后端 `Record` 信封载荷形状零改动 |
| `packages/desktop/src/lib/agent-adapter.ts` | 折叠按 `turnId` 分组、轮行挂 `metadata.record` | 轮归属口径的唯一实现点 |
| `packages/desktop/src/hooks/use-agent-session.ts` | `buildMessages` 删除，统一走 `foldSessionMessages` | 消除 `TurnDone` 计数配轮行的错位路径 |
| `packages/desktop/src/components/agent/agent-messages.tsx` | 空内容轮按 `metadata.record.status` 判定 | 停止轮 / 失败轮不再从时间线消失 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `packages/desktop/src-tauri/**` | **零改动**：信封 `turn_id`（出线 `turnId`）、`AgentEvent::stamp` 单点、`SessionStreamRegistry`、`agent_session_watch` 与 `since_seq` 过滤、`agent_stop` 幂等、`converge` 铸 `stopped` 轮行全部不变；本变更只改前端如何消化同一批信封 |
| `crates/infra/agent/src/sdk/{loop,normalize,runner}.rs` / `crates/core/agent/src/kernel.rs` | **零改动**：停止路径不发 `TurnDone` 的既有行为保留（由内核 `converge` 铸 `stopped` 轮行兜底），MUST NOT 为「前端轮分组」而新增事件或信封字段 |
| `packages/desktop/src/types/generated/**` | **零改动**：`bindings.ts` / `bindings.zod.ts` MUST NOT 重导出（本变更零信封字段增量） |
| `src/commands/exec/**` | **零改动**：`agent_start` / `agent_session_watch` / `agent_stop` / `session_detail` 签名与语义不变 |

### 模块增量：ask 工具协议与阻塞应答通道（ask-tool-in-flow）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/agent/src/ask.rs`（新） | 会话级应答注册表 | `AskRegistry::new() -> Self`；`register(&self, session_id: &str, tx: tokio::sync::oneshot::Sender<String>) -> bool`；`answer(&self, session_id: &str, text: String) -> Result<(), String>`（take 语义，重复 / miss → `Err`）；`remove(&self, session_id: &str)`（miss 幂等）。`Default` 派生；键 = core session id；零编排域类型 |
| `crates/core/agent/src/runner.rs`（扩） | 注入位与 ask 档位词汇 | `pub enum AskMode { Generic, Flow }`（`Copy + Eq + Serialize + Deserialize`，serde camelCase，**不派生 `specta::Type`**）；`SessionInjections` 增 `#[serde(default)] pub ask: Option<AskMode>`（键恒在，四字段 `PartialEq + Eq` 链完好）；`SessionOpen` 增 `pub session_id: String`（core 铸 id 下传，用途 = ask 登记键与引擎侧归属） |
| `crates/core/agent/src/kernel.rs`（扩） | 内核持注册表 + 轮收敛除名 | `SessionKernel::new(sink, registry, streams, ask_registry: Arc<AskRegistry>)`（第 4 参）；`RunningTurn` 携该 `Arc`，`converge` 内 `ask_registry.remove(session_id)`（与 `registry.remove` 同址兜底）；`begin_turn` 把 session id 填进 `SessionOpen` |
| `crates/core/agent/src/lib.rs`（扩） | 出口 | 导出 `AskRegistry`（`AskMode` 随 `runner` 出口） |
| `crates/infra/agent/src/sdk/mount.rs`（扩） | 挂载单源追加 ask 项 | `ToolExec` 增 `Ask(agent::AskMode)`；`definitions(injections, prompt)` 在 `injections.ask` 为 `Some(mode)` 时追加 `ask_user_question`（`description` / `parameters` 取自 `orchestration::ask`，`tier = ToolTier::Protocol`，`exec = Ask(mode)`）；与 `finish` 位两段**独立** `if`（MUST NOT 合取）；doc 增 provider 侧不变量（原生 ask 工具一律不挂 = 覆盖） |
| `crates/infra/agent/src/sdk/loop.rs`（扩） | ask 执行体两臂与阻塞 | `LoopTurn` 增 `ask: Option<AskContext>`（`AskContext { registry: Arc<AskRegistry>, core_session_id: String, port: Option<Arc<dyn orchestration::port::AskPort>> }`——`core_session_id` 是**注册表键**（core 铸 id，与 `LoopTurn.session_id` 的引擎侧 `sdk-…` id 同名不同义，MUST NOT 复用后者作键））；`ToolExec::Ask(Generic)` = `register` + `select!(oneshot, handle.wait_requested())`；`ToolExec::Ask(Flow)` = `port.ask_started(AskPayload{..})` + `register` + `select!` + `port.ask_resolved()`（两臂返回后都执行）；`register` 失败或上下文缺席 → is_error 回灌 |
| `crates/infra/agent/src/sdk/runner.rs`（扩） | per-turn 铸造与透传 | `PumpPayload` 增 `ask_registry: Arc<AskRegistry>` 与 `ask_port: Option<Arc<dyn AskPort>>`；`session_pump` 按 `injections.ask` 位铸 `AskContext`（`Flow` 档把 `ask_port` 一并放入；`Generic` 档为 `None`；`ask_port` 缺席而档位为 `Flow` 时铸造出的上下文为「无 port」形态 → 执行体 is_error 回灌） |
| `crates/infra/agent/src/lib.rs` / `compose.rs`（扩） | 注入缝与装配 | `EngineFacade::with_ask_registry(Arc<AskRegistry>)` + `with_ask_port(Option<Arc<dyn orchestration::port::AskPort>>)`（与 `with_explore_note_store` 同型的两条 builder 缝）；`compose_turn(..., ask_registry: Arc<AskRegistry>, ask_port: Option<Arc<dyn AskPort>>, ...)`——前者透传内核与门面，后者仅门面（编排侧状态面的实现方是壳层 sink，内核侧不需要它）；`assemble_injections` 逐臂置 `ask` 位（explore / archi → `Generic`，change → `Flow`，其余来源**显式** `Generic`，MUST NOT 走 `SessionInjections::default()`） |
| `src-tauri/src/main.rs`（扩） | 壳层托管 | `app.manage(Arc::new(AskRegistry::default()))`（与 `StopRegistry` / `SessionStreamRegistry` 三层同址）；`ChangeFlowControl` 构造传入该 `Arc` |
| `src-tauri/src/commands/exec/mod.rs`（扩） | 应答命令（CLI 面） | `agent_answer(root: String, session_id: String, text: String) -> Result<(), String>`（tauri command；blank → `Err`、miss → `Err`、命中 → 转投注册表）；`agent_start` 把注册表句柄传入 `compose_turn` |
| `packages/desktop/src/types/generated/bindings.ts`（生成物） | 前端调用面 | `agentAnswer: (root: string, sessionId: string, text: string) => Promise<null>`；`AskPayload` 增 `toolCallId: string \| null` / `sessionId: string \| null` |
| `packages/desktop/src/hooks/use-agent-session.ts`（扩） | 会话动作面 | `UseAgentSessionState` 增 `answer: (text: string) => void`（`root` / `sessionId` 为 null 时不 invoke；`Err` 经既有错误面呈现） |

### 模块增量：运行中会话的补充说明注入（steer 通道）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/agent/src/steer.rs`（新） | 会话级补充说明注册表 | `pub struct SteerRegistry`（`Default` 派生，`slots: Mutex<HashMap<String, mpsc::UnboundedSender<SteerMessage>>>`）；`new() -> Self`；`register(&self, session_id: &str, tx: UnboundedSender<SteerMessage>)`；`request(&self, session_id: &str, msg: SteerMessage) -> Result<(), String>`（miss / send 失败 → `Err`）；`remove(&self, session_id: &str)`（miss 幂等）。`pub struct SteerMessage { pub text: String, pub ack: tokio::sync::oneshot::Sender<()> }`。键 = core session id；零编排域类型 |
| `crates/core/agent/src/ask.rs`（扩） | 只读判据 | 新增 `pub fn is_pending(&self, session_id: &str) -> bool`（`slots.contains_key`，O(1)）；`register` / `answer`（take 语义）/ `remove` 逐字不变 |
| `crates/core/agent/src/runner.rs`（扩） | 通道挂载 | `AgentSession` 增第四字段 `pub steers: tokio::sync::mpsc::UnboundedSender<SteerMessage>`（与 `observations` / `questions` / `handle` 并列；doc 注明「用户 → turn 的单向注入，一轮一命不破」） |
| `crates/core/agent/src/kernel.rs`（扩） | 登记 / 除名 | `SessionKernel::new(sink, registry, streams, ask_registry, steer_registry: Arc<SteerRegistry>)`（第 5 参）；`RunningTurn` 携该 `Arc`；`begin_turn` 在 `registry.register` / `streams.begin` 之后 `steer_registry.register(&session_id, agent_session.steers.clone())`；`converge` 内 `steer_registry.remove(&session_id)`（与 `registry.remove` / `ask_registry.remove` 同址） |
| `crates/core/agent/src/lib.rs`（扩） | 出口 | 导出 `SteerRegistry` / `SteerMessage`（与 `AskRegistry` 同址一行） |
| `crates/infra/agent/src/sdk/runner.rs`（扩） | 建通道 + 传接收端 | `open_session` 内 `mpsc::unbounded_channel::<agent::SteerMessage>()`；接收入端交 `session_pump` 再交 `loop::run`（新增位置参数）；`AgentSession.steers` 回填 sender。`PumpPayload` / `EngineFacade` **零改动**（引擎侧不需要注册表——登记发生在内核） |
| `crates/infra/agent/src/sdk/loop.rs`（扩） | 请求边界 drain | `run(model, turn, history, steers: mpsc::UnboundedReceiver<agent::SteerMessage>, sender, handle)`（新增第 4 位置参数）；主循环顶部（停止快检之后、`defend` 之前）`while let Ok(msg) = steers.try_recv()`：发 `AgentEventKind::Message{role: User, blocks:[Text], parent_tool_use_id: None}` → `history.push(Message::user(text))` → `steer.ack.send(())`；事件通道关闭即停止 drain 并返回（剩余项随接收端 drop 收 `Err`）。`LoopTurn` **零改动** |
| `crates/infra/agent/src/compose.rs`（扩） | 组合根注入 | `compose_turn(stores, registry, streams, ask_registry, steer_registry: Arc<agent::SteerRegistry>, store, agent, workspace_root, ask_port)`（新实参紧跟 `ask_registry`；透传内核；门面零改动） |
| `src-tauri/src/main.rs`（扩） | 壳层托管 | `app.manage(Arc::new(SteerRegistry::default()))`（与 `StopRegistry` / `SessionStreamRegistry` / `AskRegistry` 同址）；`ChangeFlowControl` 构造面零改动（它不认识 steer） |
| `src-tauri/src/commands/exec/mod.rs`（扩） | 补充说明命令（CLI 面） | `#[tauri::command] pub async fn agent_steer(steer: State<Arc<SteerRegistry>>, ask: State<Arc<AskRegistry>>, root: String, session_id: String, text: String) -> Result<(), String>`：blank 三参 `Err` → `ask.is_pending` 判据 → `steer.request` → `ack.await`；两个 `State` 先 `Arc::clone` 再 await（不跨 await 持 `State` 借用）；`agent_start` / `change_flow_start` 把注册表句柄传入 `compose_turn` |
| `src-tauri/src/commands/mod.rs`（扩） | 命令登记 | `all_commands!` 登记 `agent_steer`（紧随 `agent_answer`，`commands/exec` 轨道） |
| `packages/desktop/src/types/generated/bindings.ts`（生成物） | 前端调用面 | `agentSteer: (root: string, sessionId: string, text: string) => Promise<null>`（`bindings:export` 再生成，勿手改） |

### 模块增量：只读 DB 工具面装配与 per-turn 上下文铸造（agent-db-query-tools）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/agent/src/compose.rs`（扩） | 注入装配两臂 | explore 臂名字清单 7 → 9（+ `mount::DB_TABLES_TOOL` / `mount::DB_QUERY_TOOL`）；change 臂在矩阵并入之后恒 push 两名（矩阵未命中行、畸形 `source_ref` 同样命中）；archi 臂（七内建 + `archi_module` / `archi_search` / `archi_api` 三件，枚举与 `desktop-archi-agent` 单源对齐）与 debug 臂逐字不变；成员校验语义不变 |
| `crates/infra/agent/src/sdk/mount.rs`（扩） | 挂载清单 | `DB_TABLES_TOOL = "db_tables"` / `DB_QUERY_TOOL = "db_query"` 常量 + 两个 `ToolDefinition` + `ToolExec::Db` 变体 + `ToolTier::Read`；挂载清单 11 → 13 项；`TOOL_NAMES` 七件不动（`tools = None` 语义不变） |
| `crates/infra/agent/src/sdk/loop.rs`（扩） | 执行分派 | `LoopTurn.db: Option<DbContext>`（与 `explore_note` / `archi` / `ask` 同列）；`ToolExec::Db` 臂路由至 `db_tool::DbContext::execute`；上下文缺席以 `is_error` 回灌（生产不可达） |
| `crates/infra/agent/src/sdk/runner.rs`（扩） | per-turn 铸造 | `PumpPayload` 增双库句柄（全局 + workspace）；泵起播以 `contains_tool` 判据铸造 `DbContext`（与 `mint_archi_context` 并列的第二个名字判据） |
| `crates/infra/agent/src/lib.rs`（`EngineFacade`） | builder 缝 | `with_db_stores(...)`（与 `with_explore_note_store` 同型旁路缝），`compose_turn` 透传；`runner_for` 签名零改动 |
| `crates/infra/agent/src/sdk/db_tool.rs`（新） | 两工具执行体 | 契约见 desktop-agent-db-tools「Module Contract」（scope 解析 / 字节预算 / `api_key` 打码 / 错误文案） |
| `crates/infra/agent/src/sdk/tools.rs`（扩） | 沙箱豁免 | `input_path` 增 `db_tables` / `db_query` 显式 `None` 臂；七通用工具 `execute(root, name, input)` 纯函数语义零改动 |
| `crates/core/agent/**`、`crates/infra/agent/src/sdk/{policy,sandbox,protect}` | 零改动面 | `SessionInjections` 四字段形态零扩展（DB 工具走名字清单通道）；`Read` 档三档放行天然覆盖；MUST NOT 出现第二份按名清单；`policy_test.rs` 的逐工具三档等价表随挂载名数 11 → 13 补齐（期望随动，不改判据） |

### 模块增量：中断轮归属与列表终态五值（session-midturn-history-overwrite）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / Component） |
|------|------|------|
| `crates/core/agent/src/ask.rs`（`AskRegistry`） | 零改动面 | `register` / `answer` / `remove` / `is_pending` 零改动：「重启后无等待方」由进程内形态天然保证——注册表空 ⇒ `answer` 走既有 miss 臂 `Err`；不新增停等态持久化 / 恢复面 |
| `src-tauri/src/commands/exec/mod.rs`（`agent_answer`） | 零改动面 | 行为由启动标定保证：对已标定会话返回显式 `Err`；命令体仍是参数校验 + 注册表投递 + 错误映射三段 |
| `src-tauri/src/commands/exec/mod.rs`（`agent_stop`） | 零改动面 | miss 幂等 `Ok` 沿用（不改行、不报错）；轮行终结由启动标定承担，不在命令面新增对账 |
| `packages/desktop/src/views/agent/components/agent-run-history.tsx`（改） | 列表终态文案 | `statusLabel` / 行色增 `interrupted` 档：「已中断」；`data-status` 挂钩直出状态词；MUST NOT 落进既有 `failed` 兜底文案 |
| `crates/infra/agent/src/sdk/resume.rs` | 零改动面 | `INTERRUPTED_RESULT` 合成结果零改动（续轮 provider 配对修复的既有机制） |
| 会话接续（`resume_turn` / loop 第三入口 / cwd 入档） | 范围外 | 明确不在本次范围：中断的轮按客观终止处理，用户重发即可 |
| ask 取消通路（`declined` 预留位） | 范围外 | 照旧不开（形状预留保持不变） |

### 模块增量：bash 工具调用意图参数（bash-tool-description-param）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/agent/src/sdk/mount.rs`（`generic_definition("bash")`，改） | `properties` / `required` / `description` 扩位 | `properties` 增 `intent`（`type: "string"`，**无 `maxLength`**）；`required` 由 `["command"]` 改为 `["command", "intent"]`；工具级 `description` 增一句「每次调用必须给一行意图」的引导（与字段 description 分工单点）。其余六工具定义逐字不变，`TOOL_NAMES` 七件与注册序不变 |
| `crates/infra/agent/src/sdk/mount.rs`（`ToolDefinition` / `definitions(...)` 装配面） | 零改动面 | 签名、注入面（名字清单 / `finish` / `ask` 位）与 `RunStarted.tools` 上报语义零变化——本变更只动一个工具定义的字面内容 |
| `crates/infra/agent/src/sdk/bash.rs`（`execute(root, input)`，改） | 新增意图闸 | `command` 闸之后、`resolve_timeout_ms` 之前：`input.get("intent").and_then(Value::as_str)` 缺失 / 非字符串 → `Err`（文案含 `intent`）；`trim().is_empty()` → `Err`（文案含 `intent`）。不新增长度分支 |
| `crates/infra/agent/src/sdk/bash.rs`（`resolve_timeout_ms` / `build_command` / `drain` / `merge_output`） | 零改动面 | 超时解析、底座组装、双管道排水与合并口径、退出码分支逐字不变；`intent` 仅用于校验，不进子进程命令行、不进 ToolResult、不进日志 |
| `crates/infra/agent/src/sdk/protect/**` | 零改动面 | 保护闸判定输入仍只有 `input["command"]` 与会话执行根；`intent` 不进判别链（出现 diff 即越界） |
| `crates/infra/agent/src/sdk/{sandbox.rs,tools.rs}` | 零改动面 | 路径沙箱与 `tools::input_path` 零改动（bash 恒 `None`）；`tools::execute` 分派表零改动 |
| `crates/infra/agent/src/sdk/{context.rs,compact.rs}` | 零改动面 | 不裁剪 tool input，不为 `intent` 加豁免规则 |
| `crates/infra/agent/src/sdk/policy.rs` | 零改动面 | 三档决策表零改动（bash 档位语义不变） |
| `crates/core/agent/**` | 零改动面 | `ToolUseBlock.input` 仍为 `serde_json::Value`，事件词汇与块词汇零扩展 |
| `crates/infra/store/**` | 零改动面 | 转录与轮记录形状零变更（`intent` 藏在既有 input JSON 内） |
| `src/commands/**` | 零改动面 | 命令层零改动（`intent` 是工具载荷字段，不出线 IPC 类型） |

### 模块增量：provider effort 旁路与协议 shaping（provider-effort-knob）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|----------|
| `crates/infra/agent/src/sdk/effort.rs`（新） | 协议 shaping 单点 | 常量 `OPENAI_EFFORT_KEY = "reasoning_effort"` / `ANTHROPIC_OUTPUT_CONFIG_KEY = "output_config"` / `ANTHROPIC_EFFORT_KEY = "effort"`（`loop` / `runner` / 门面 / 组合根零重复）；`params(protocol: Protocol, effort: &str) -> Option<serde_json::Value>`：裁剪后空 ⇒ `None`（留空 = 不发）、`Openai` → `{reasoning_effort: v}`、`Anthropic` → `{output_config: {effort: v}}`；零值域校验、零 IO |
| `crates/infra/agent/src/sdk/runner.rs`（扩） | 档位分发扩二元组 + 旁路载荷 | `SdkRunner.efforts: AgentEffortTiers` + `with_efforts(self, AgentEffortTiers) -> Self`（`new` 签名不动、既有调用点零改）；`open_session` 的档位 `match` 扩为 `(model_field, model_name, effort)` 三元组（唯一分发点，`effort::params` 只算一次）；`PumpPayload` / `session_pump` 增 `effort_params: Option<serde_json::Value>` 并于构造 `LoopTurn` 时传递；启动日志加 `effort=…`（留空 `-`） |
| `crates/infra/agent/src/sdk/loop.rs`（扩） | wire 载荷消费 | `LoopTurn.effort_params: Option<serde_json::Value>`（**已按协议 shaping**，loop 零协议判定、零键名字面量）；请求构造取 `additional_params: turn.effort_params.clone()`，与既有 `request.provider_tool(..)` 在同一 `additional_params` 上按键合并（rig `merge_params`），两键互不覆盖 |
| `crates/infra/agent/src/{lib,compose}.rs`（扩） | 门面缝与组合根 | `EngineFacade.efforts` + `with_efforts(AgentEffortTiers)`（与 `with_context_window` / `with_max_tokens` / `with_web_search` 同型，四个构造点补齐字段）；`compose::ResolvedEngine` 增 `efforts: AgentEffortTiers`（provider 原样带出、**零门控**——两协议都有 effort 语义）；`runner_for` 链加 `.with_efforts(..)`，签名与唯一 match 点零塌 |
| `crates/infra/agent/src/sdk/{config,compact}.rs` | 零改动面 | `EngineConfig` 连接配置字段面不增字段（`efforts` MUST NOT 出现在本结构，走门面旁路）；`compact::summarize` 的 `additional_params: None` 逐字保留（不带 effort、不带 provider tool）——`compact.rs` 出现 diff 即说明偏离本变更新增纪律 |
| `crates/infra/store`（消费面） | 只读消费 | `AgentEffortTiers` / `AgentProviderRecord.efforts`（v5 新列）原样带出、零校验；见 desktop-agent-management Module Contract |
| `crates/core/agent/**` | 零改动面 | `AgentEvent` 词汇与 `SessionConfigSnapshot` 形状零扩展（无 effort 字段）；core 不认识 effort / shaping 概念 |

### 模块增量：收口前自检与相位锁注入面（archi-rule-agent-prompt-sync）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/runner.rs` | 会话契约（第六位） | `SessionInjections { preamble, tools, finish, ask, protect, before_done }`；`protect: Option<Vec<SessionProtectRule>>` 为 `#[serde(default)]` 的纯数据（`SessionProtectRule { glob, reason }`）；`before_done: Option<BeforeDoneSpec>` 为收口前钩子档位（`#[serde(default)]`、`None` = 不装配，行为归 infra）；均 `Clone + Eq`、**无 `specta::Type`**（不进 TS bindings）；`load` / round-trip / 缺字段读 `None` 语义 additive |
| `crates/infra/agent/src/compose.rs` | 装配单点 | `assemble_injections` 三臂 + `injected(...)` 签名携带 `protect`；`model_write_lock(provenance)` 为唯一置位点（archi 臂与 `(proposal｜dev-design, executor)` → `None`，其余 → `Some([一条])`）；`archi_matrix` 8 行含两新工具列；archi 臂名字清单五 archi 名 |
| `crates/infra/agent/src/sdk/runner.rs` | 装配挂载 | `ProtectRules::load(workspace_root).with_session(injections.protect)`；`mint_archi_context` 判据含五名；`with_workspace_root` 通道逐字不变 |
| `crates/infra/agent/src/sdk/loop.rs` | 守卫链 + 收敛挂点 | 四段顺序（成员校验 → 档位 → 沙箱 → 保护闸）逐字不变；自然收敛支调用已装配的 `before_done` 钩子（先于 `finish_success`）；其余四出口零调用 |
| `crates/infra/agent/src/sdk/preclose.rs`（新） | 收口自检钩子实现 | `BeforeDoneHook` 三面：`start`（起点资产哈希）/ `observe_tool`（T1 写面观测）/ `before_done`（判定 + notice）；`spawn_blocking`；`CancelFlag::never()`；fail-open → `archi_preclose_skipped`；零额外请求、零历史追加 |
| `crates/infra/agent/src/sdk/protect/{mod,rules,reason}.rs` | 第三规则来源（详见 `desktop-write-protection`） | `with_session` 合并 + `ReasonKind::Session` + 模板出口；`load` 签名不变 |

### 模块增量：config 会话臂与 `propose_config` 工具面（config-agent-session）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|----------|
| `crates/infra/agent/src/compose.rs`（扩） | 注入装配第四臂 | `assemble_injections` 增 `source == "config"` 臂（`CONFIG_SOURCE`）：`tools = Some([read, grep, glob, ls, bash, propose_config])` + `preamble = config_knowledge::knowledge()` + `finish = None` + `ask = Some(Generic)`；签名不变、成员校验语义不变；doc 三臂转四臂 |
| `crates/infra/agent/src/config_knowledge.rs` + `config-knowledge.md`（新） | 知识 preamble 生产者 | `knowledge() -> Option<String>`：静态骨架（`include_str!`）+ 派生词表（`config::known_keys` / `TestFramework::ALL`）+ 使用指引；零入参、零 IO、零占位符；失败只丢 `preamble` 位 |
| `crates/infra/agent/src/sdk/config_propose.rs`（新） | 提案工具执行体 | 两模式（只读盘面 / 候选校验回环）；输出单条 file-face JSON 字符串；`changes` / `summary` 后端算；零副作用 |
| `crates/infra/agent/src/sdk/mount.rs`（扩） | 挂载清单 | `PROPOSE_CONFIG_TOOL` 常量 + 第 16 项挂载（`ToolTier::Read`、`ToolExec::Config`）；`with_capacity(15)` → 16 与两段 doc 随动；`TOOL_NAMES` 七件不动 |
| `crates/infra/agent/src/sdk/tools.rs`（扩） | 沙箱豁免 | `input_path` 无路径白名单补 `propose_config`（候选中任意键名不被当路径改写） |
| `crates/infra/agent/src/sdk/runner.rs` + `sdk/loop.rs`（扩） | per-turn 铸造与分派 | `ConfigContext { root }` + `mint_config_context(names, payload.workspace_root)`（名字判据）；`LoopTurn` 增字段 + `ToolExec::Config` 分派臂；上下文缺席以 `is_error` 回灌 |
| `crates/core/agent/**` | 零改动面 | `SessionInjections` 字段面零扩展（`propose_config` 走名字清单通道）；事件词汇零扩展 |
| `crates/infra/agent/src/sdk/{policy,sandbox,protect}` | 零改动面 | 三档决策表判据零改（期望随挂载名数 15 → 16 随动）；`Read` 档天然覆盖 `propose_config`；写保护零改动 |

### API: `SessionInjections`（`crates/core/agent/src/runner.rs`）

| 项 | 变化 | 描述 |
|---|---|---|
| `SessionInjections.protect` | 新增（第五位） | `Option<Vec<SessionProtectRule>>`，serde additive；`None` = 不追加会话规则 |
| `SessionProtectRule` | 新增类型 | `{ glob: String, reason: String }`；`reason` 携 `%s` / `%t` 占位符；core 不解释路径语义 |
| `FinishSpec` / `AskMode` / `tools` | 零改动 | 词汇与语义逐字不变 |

### Function: `crates/infra/agent/src/sdk/loop.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `run`（自然收敛支） | 改 | 先调用已装配的 `before_done` 钩子再 `finish_success`；T1 由钩子 `observe_tool` 在工具执行前置位 |
| 其余四出口 | 零改动 | 空 choice / 截断 / 异常终局 / 停止 / `pause_turn` 到顶的收敛路径与文案逐字不变 |
| 守卫链 | 零改动 | 四段顺序、拒绝形态、`deny()` 双面传递逐字不变（会话规则只是 `ProtectRules` 的一条来源） |

### 模块增量：会话系统提示词快照拍存与单查快照面（session-preamble-snapshot）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / Component） |
|------|------|----------|
| `crates/infra/agent/src/sdk/preamble.rs` | 系统提示词快照合成（**行替换**：原「起播前读 + 每轮重读」行以本行为准） | `load(root)` 读取 + 32KB char 边界截断 + `<agent_md>` 包装（签名与口径零改动，仅调用方由引擎改为组合根）；`freeze(knowledge, agent_md)` 合成单点（知识 → 空行 → `AGENT.md` 段，空段跳过、全空 → 空串）；模块可见性保持 `pub(crate)`，零新增第三方依赖 |
| `crates/infra/agent/src/compose.rs` | 快照拍存 / 读回 / 存量补拍单点 | `ComposedTurn::begin`：首轮判据 = `New` ∨ `is_unstarted_session`；首轮读 `ctx.workspace_root` 的 `AGENT.md` → `preamble::freeze` → `kernel.begin_turn` 后经 `self.query.store()` 落库（不扩 `SessionSink`），失败仅日志；续轮 `find_session_preamble` 覆盖 `injections.preamble`（空正文 → `None`）；存量补拍同首轮口径 |
| `crates/infra/agent/src/sdk/loop.rs` | 系统消息一元消费 | 删 `preamble::load` 调用与二元 `system_prompt(knowledge, agent_md)` 形参；`turn.injections.preamble` 非空 → 恰一条 `Message::system`；`AGENT.md` / `preamble::load` 字样零残留（机械可查） |
| `crates/core/agent/src/session.rs` | 出线 DTO 扩面 | `SessionPreambleInfo { saved_at, content }`（`specta::Type`）+ `SessionSummary.preamble: Option<SessionPreambleInfo>`；`SessionRow` / `SessionStats` 零变更 |
| `crates/core/agent/src/runner.rs` | 注入位语义（零结构改动） | `SessionInjections.preamble` 的 doc 由「会话级系统前导」改写为「会话系统提示词快照正文」；六位字段面、`serde` 形态、`PartialEq/Eq` 派生链零变更 |
| `crates/infra/store/src/{model,store,envelope}.rs` | 持久载体与操作面 | 见 `desktop-workspace-store`「会话系统提示词快照记录（写一次冻结）」；本能力只作消费者，MUST NOT 另写第二份模型或写面 |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | 快照只读可见面 | 见 `desktop-agent-chat-infra`「会话系统提示词快照的只读可见面」；本能力只作消费者 |
| `packages/desktop/src/types/generated/`（生成物） | 再生成 | `bindings.ts` / `bindings.zod.ts` 增 `SessionPreambleInfo` 类型与 `SessionSummary.preamble`（`nullable`）；非手改，`bindings:export` 产出 |
| `preamble::load` / `truncate` / `freeze`（`crates/infra/agent/src/sdk/preamble.rs`） | 函数面 | `async fn load(root: &Path) -> Option<String>`（签名与口径逐字不变，调用方由 `loop` 改为 `compose`）/ `fn truncate(text: &str) -> String`（32KB char 边界截断单点，`load` 唯一消费）/ `fn freeze(knowledge: Option<&str>, agent_md: Option<&str>) -> String` |
| `ComposedTurn::begin` 快照分支（`crates/infra/agent/src/compose.rs`） | 内部形态 | 签名零变更（`session` / `question` / `ctx` / `provenance`），内部增首轮 / 续轮 / 补拍三分支；`ctx.workspace_root` 在移入 `TurnRequest` 前读取；首轮 `preamble::load` → `preamble::freeze` → 送入 `injections.preamble` → `begin_turn` 后 `store.stamp_session_preamble` |

### 模块增量：接口定义写闸与知识段第三生产者（archi-agent-context-review）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/tools.rs` | 七通用工具执行体 | `write(input)` / `edit(input)` 增 `root: &Path` 入参（两个调用点机械改动）；落盘前判「是否 `<archi_models_rel_dir>/**/<FQN>.schema.json`」→ 是则调 `archi::api::schema::validate_schema`，`Err` 零落盘、成功追加「接口已校验」；`edit` 校验替换后正文；其余工具与 `tools::execute(root, name, input)` 对外签名零改动 |
| `crates/infra/agent/src/compose.rs` | 装配单点 | `assemble_injections` 的 change 臂：`preamble = model_write_lock(provenance).is_none().then(knowledge).flatten()`（单一判据、失败降级 `None`）；`archi_matrix` 两格追加 `ARCHI_API_TOOL`；archi 臂与 config 臂逐字不变 |
| `crates/infra/agent/src/archi_knowledge.rs` | 知识生产 | `knowledge()` 由 archi 臂与 change 臂共用（同一函数、同一文本）；降级 `None` 语义不变 |
| `crates/core/agent/src/runner.rs` | 注入词表 | `SessionInjections` 六位逐字不变（`preamble: Option<String>` 的语义由「archi / config 两处」扩为「archi / change 写权相位 / config 三处」——位形态零改动） |
| `crates/core/archi/src/api/schema.rs::validate_schema` | 校验单点（跨 crate 依赖） | `infra/agent` 经 `archi` crate 的公开面引用（`archi` 已在其依赖内）；MUST NOT 在 `infra/agent` 复写校验 |
| `crates/infra/agent/src/sdk/tools_test.rs`（或既有工具语料位） | 写闸语料 | 三类坏输入各一条 `Err` + 盘面零变化；合法落盘回执；非接口路径零校验；跨元素不越权 |

### API: 工具面（`write` / `edit`）

| 项 | 变化 | 描述 |
|---|---|---|
| `write` / `edit` 的 `parameters` | 零变化 | 仍为 `path` / `content` 与 `path` / `old_string` / `new_string` / `replace_all` |
| 成功响应 | 加一行（条件） | 仅当目标是接口定义文件时追加「接口已校验」；其他路径响应逐字不变 |
| `Err` 面 | 新增一类 | 接口校验失败（第一行自足：文件 + 期望值 + 出路）；既有 `Err`（缺字段 / `old_string` 未命中 / 写失败）逐字不变 |
