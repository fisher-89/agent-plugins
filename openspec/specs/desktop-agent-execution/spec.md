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
- **查询会话**：`query_sessions(...)` 形状的会话记录查询——会话清单（分页信封：`offset` / `limit` 区间 + 过滤集全量 `total`）与聚合统计（提问轮数、累计墙钟、累计 token 消耗；MVP 从轮统计行现算，MUST NOT 维护累计列，字段面可扩展）。

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
- `Message` { role, blocks, parent_tool_use_id }（**密封，durable**）：唯一落库与重放单元；密封粒度 = 引擎一次 assistant 回应（全部块收进恰一条密封 Message）。其中 Block = Text | Thinking | ToolUse{id, name, input} | ToolResult{id, content, is_error} | **WebSearch{id, query, results, error, raw_use, raw_result}**
- `SystemNotice` { subtype, payload }（permission_denied / api_retry 等 system 事件，subtype 不做封闭枚举）
- `TurnDone`（现 `RunResult` 演进，更名/别名由 design 定稿）{ subtype, is_error, num_turns, duration_ms, cost, usage, session_id }：**统计唯一口径**（见「P1 引擎协议不变量与缺省降级」），唯一驱动收敛的变体
- `Raw` { type, 原文 JSON }（未知 / 未来事件类型透传）

块词汇自本条起为**五变体**（provider 服务端工具块入信封，词汇仍协议中立——core 不认识 anthropic / rig）：

- `Text { text, citations: Option<Vec<AgentCitation>> }`——`citations` 为 `#[serde(default)]`（**MUST NOT** 加 `skip_serializing_if`：unified 相位模型无法表达条件省略，加则 bindings 分裂为 `_Serialize` / `_Deserialize`，仓库 golden 契约不允）；`None` = 无引用、序列化落 `null`（键恒在）；旧事件缺该字段读出 `None`，前端既有 fixture 零改动；
- `WebSearch { id, query, results: Vec<WebSearchSource>, error: Option<String>, raw_use: Option<serde_json::Value>, raw_result: Option<serde_json::Value> }`——**一次 `server_tool_use` 与其配对 result 收敛为一块**（`id` 同两侧配对键）；`results` / `error` / `raw_use` / `raw_result` 为 `#[serde(default)]`（键恒在：空数组落 `[]`、缺省落 `null`）；`raw_use` / `raw_result` 为回放唯一事实源（`encrypted_content` 等不可推导字段原样保全）。块语义与投影规则见「web_search 服务端块的事件投影与回放保真」。

loop 调试关键信息 SHALL 在一等公民位：tool_use 与 tool_result 成对可见、子代理经 `parent_tool_use_id` 归因、TurnDone 携带统计字段。每事件 SHALL 携带 `seq`（单调序号）与时间戳（盖戳治理收内核）；delta 与密封的 seq 编号关系（共享空间容忍库内空洞 vs 独立编号）由 design 定稿。`Raw` 变体 SHALL 承接所有未识别事件：MUST NOT 因未知 type 丢弃事件或解析失败（永不丢事件、永不炸解析）。

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

- 发起命令 SHALL 在会话/轮记录落库并进入 running 后**提前 resolve**，返回 running 态记录（id 立即可知）；执行 SHALL 转入内核编排继续（内核持久面持有 store 收尾），终态记录经 Channel 以与实时事件同构的状态部件流出，MUST NOT 再依赖 invoke 返回携带终态。
- 终止命令 SHALL 按运行中的会话活动轮寻址: 将轮状态收敛为受控终态 `stopped`、尽力终止引擎执行体（经停止信号取消泵任务（drop future）——机制由 design 定稿核实；寻址键（run/轮 id 沿用或会话键，root 消解跨库歧义保留）由 design 定稿）、经 Channel 流出终止终态部件（与实时事件同构）。`stopped` SHALL 与 `completed` / `failed` 同列受控终态（用户主动终止与引擎失败语义区分，收敛后 MUST NOT 再接受状态变更）。停止注册（内存态）收内核治理面。
- 对非 running 目标（已终态或不存在）的终止 SHALL 幂等忽略，MUST NOT 报错崩溃或误改既有终态。
- 停止后的重放 SHALL 与实时一致: 重放路径从会话转录（含 `stopped` 终态）还原与实时查看相同的呈现（终态部件同构）。

#### Scenario: 提前 resolve

- **WHEN** 前端发起会话运行
- **THEN** invoke 在轮记录落库进入 running 后即 resolve 返回 running 态记录（id 可用），引擎事件随后经 Channel 持续流入，终态以状态部件经 Channel 流出

#### Scenario: stop 收敛与引擎终止

- **WHEN** 运行中 invoke 终止命令
- **THEN** 轮状态收敛为 `stopped`，Channel 流出终止终态部件；运行的泵任务被取消（事件流终止、future drop），无孤儿泵残留

#### Scenario: 幂等忽略

- **WHEN** 对一条已 `completed` 的轮 invoke 终止命令
- **THEN** 幂等忽略: 状态不变、不报错、不产生新事件

#### Scenario: 停止后重放一致

- **WHEN** 停止一轮运行后重新打开该会话查看
- **THEN** 重放呈现与停止时实时呈现一致（含 `stopped` 终态部件），续话以会话最新状态为准

### Requirement: 引擎门面与 EngineKind 参数选择

`agent-runtime` SHALL 作为 agent 引擎门面：对外一套应用协议（invoke 面 + 事件流 + 会话记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎（值域保留为多租户协议预留，`Cli` 为退役态）；协议统一 SHALL 落在内核协议层而非各引擎私有形态。门面 SHALL 提供 `runner_for(kind, engine_cfg)` 形状的构造入口——`EngineConfig` 结构体形态为**连接配置字段面**（`protocol` + api_key / base_url / model_high / model_low：协议决定 wire 语义故在此面内，可空调参 `context_length` / `max_tokens` / **`web_search`** MUST NOT 进本结构、一律走门面旁路 builder）由组合根注入，SDK 引擎用以组装 rig client；`EngineKind::Cli` 臂 SHALL 退役为显式 `Err("CLI 租户已下线")` 形态（runner 构造失败面，具体形态——`Result` 包装或等价——由 design 定稿），**唯一 match 点结构与 `EngineKind` 双值 MUST NOT 塌**（多租户机制保留，未来本机 CLI / 远程 API 租户仍落协议预留；模型协议 MUST NOT 编码进 `EngineKind`）。**解析单点自命令层下沉至内核组合根**（desktop-agent-management「默认 agent 与运行发起解析」的落点平移，解析语义不变）：缺省路径解析默认 agent、显式路径按所选 agent 解析、产物为 `EngineKind` + `EngineConfig`（sdk 时由引用 provider 组装、model 取 provider 三档 high 档、协议取 provider `protocol`）；解析遇 `engine: Cli` 实例（缺省与显式路径同）SHALL 显式 `Err("CLI 租户已下线")`，MUST NOT 静默回退。换解析落点时 `EngineConfig` 消费面 MUST NOT 变化。启动失败抹平：`AgentStartError` 的中性变体 `ConfigMissing` 承接 SDK 侧连接配置不齐等启动失败——core 唯一触碰点，变体集 MUST NOT 再增。

门面旁路载荷 SHALL 为三项且同型（`with_context_window` / `with_max_tokens` / `with_web_search` 三个 builder）：`web_search` 的语义为「**已按协议门控**的最终开关」（门控判定在组合根解析期完成，`provider.web_search && protocol == Anthropic`；门面与 runner MUST NOT 二次判定协议）。

#### Scenario: 门面构造与 core 零污染

- **WHEN** 审查 `runner_for` 实现与 `crates/core/agent` 源码
- **THEN** 引擎分发唯一 match、返回引擎协议实现；core 无 engine / `EngineConfig` / rig / agent / 协议概念（亦无 `web_search` 的协议语义，只有块词汇与值类型），`AgentStartError` 变体集不变

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现与门面签名
- **THEN** 解析语义（默认 agent / 显式 agent / sdk provider 组装 / high 档 / 无默认 `Err` / cli 实例 `Err`）与设计一致，`runner_for` 唯一 match 点结构与连接配置字段面零塌，命令层无引擎解析残留

#### Scenario: cli 解析显式失败

- **WHEN** 解析目标实例 `engine` 为 `cli`（缺省默认实例或显式选择的存量 cli 实例）
- **THEN** 返回 `Err("CLI 租户已下线")`（文案引导管理页），不构造任何引擎、不落库不推流

#### Scenario: 旁路与配置面的分工

- **WHEN** 审查 `EngineConfig` 字段面与门面 builder
- **THEN** `EngineConfig` 仅含连接必需项（`protocol` / `api_key` / `base_url` / `model_high` / `model_low`），`is_complete` 语义不含协议（协议恒有值）；`context_length` / `max_tokens` / `web_search` 经 `with_context_window` / `with_max_tokens` / `with_web_search` 旁路注入，MUST NOT 出现在 `EngineConfig` 中

#### Scenario: 门面不二次门控

- **WHEN** 以 `with_web_search(true)` 装配的门面构造 SDK runner（组合根已判定协议为 anthropic）
- **THEN** runner 收到 `web_search = true` 并原样下传 loop，门面与 runner 侧零协议判定；组合根留下的 `openai + true` 记录在解析期已收敛为 `false`，门面不参与纠偏

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 协议 SHALL **双值可选且双首公民**：`openai`（openai-compatible 端点——GLM / DeepSeek / Kimi / 自部署）与 `anthropic`（Anthropic Messages 端点及兼容网关）；协议由 provider 记录 `protocol` 声明、经 `EngineConfig.protocol` 抵达引擎（见「SDK 引擎模型协议选臂（openai / anthropic）」）——原「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」的限制在本变更偿还
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层
- **流面双层交付**：流内 `Text` / `Reasoning` 片段增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面；rig 0.43 流事件模型为部分生命周期——Start / Arguments / End 为簿记事件不出增量）；轮末 `finish()` 折叠的 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：默认（无会话注入）工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；会话级注入 SHALL 可收窄该面并追加受控专用工具（见「会话级工具面注入与 explore 笔记工具」）；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求系统提示词 SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections.preamble` 位维持不消费，`tools` / `toolset` 位按来源装配（见「会话级工具面注入与 explore 笔记工具」）
- **上下文管理**：喂 provider 的请求史 SHALL 经 L1-L3 上下文防线治理（见「SDK 引擎工具质量与单结果上限（L1）」「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）；store 转录全量不变；窗长来源含模型名 `[1m]` 后缀（见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）
- 事件归一化：归一化 SHALL 只消费 rig 抽象事件词汇（`StreamEvent` / `Completion` / `Message`），工具调用映射为 ToolUse 块并与 ToolResult 成对（同 id）；协议差异 MUST NOT 出现在归一化层（openai chat completions 形状 fixture 仅为测试形状，不是运行时协议判定；anthropic 经 Messages wire 产出同一抽象）；`RunStarted` SHALL 各报各的（model 报**裸名**——`[1m]` 后缀已剥离，tools 报本轮实际下发的工具面——默认七工具、注入面见「会话级工具面注入与 explore 笔记工具」）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
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

### Requirement: SDK 引擎系统提示词（AGENT.md 注入）

SDK 引擎 SHALL 在起播前读取 workspace root 的 `AGENT.md` 单份文件作为系统提示词来源：文件存在 → 内容**逐字**注入请求系统提示词位（rig 0.43 起 `preamble` 字段退役，以 chat_history 首条 `Message::System` 承载；不包装、不改写）；文件缺席 → 不注入 System 消息（现状语义不变）。读取范围 SHALL 严格限于 `AGENT.md`，MUST NOT 做 `CLAUDE.md` 等兜底回退。注入 SHALL **每轮重读**：会话中途修改 `AGENT.md`，改动自下一轮请求生效（不做起播快照——bash 引入后 agent 可改自己的系统提示词，该语义行为最简且可预期）。

该机制 SHALL 为引擎内部特性（P2 原则：跨会话持久上下文是 agent 产品特性非协议义务），`crates/core/agent` 零改动；`SessionInjections.preamble` 位维持不消费（系统提示词唯一来源仍为 `AGENT.md`，会话级系统前导不进 MVP），工具面注入位自「会话级工具面注入与 explore 笔记工具」起按 provenance 来源装配（compose 不再恒传 default）。注入体量上限 32KB（char 边界截断）、嵌套 `AGENT.md` 不级联。

#### Scenario: AGENT.md 在场逐字注入

- **WHEN** workspace root 存在 `AGENT.md` 时以 sdk 引擎发起运行
- **THEN** 请求系统提示词（chat_history 首条 System 消息）为该文件内容逐字（不包装、不改写）；AGENT.md 注入路径零 diff（compose 的注入装配仅工具面，见「会话级工具面注入与 explore 笔记工具」）

#### Scenario: 缺席维持现状

- **WHEN** workspace root 无 `AGENT.md` 时以 sdk 引擎发起运行
- **THEN** 请求不注入 System 消息，请求形状与注入机制上线前一致

#### Scenario: 会话中修改下一轮生效

- **WHEN** 会话运行中途修改 `AGENT.md` 后续发下一轮
- **THEN** 已发轮次不受影响，下一轮请求注入修改后的内容（每轮重读，无起播快照）

#### Scenario: 无兜底回退

- **WHEN** workspace root 只有 `CLAUDE.md` 而无 `AGENT.md`
- **THEN** 不发生兜底读取，系统提示词位仍空（严格单文件语义）

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 自本条起激活为引擎消费面（切片③收编），形态为「收窄 + 受控追加」双位：

- **core 契约扩展**：`SessionInjections` SHALL 新增受控枚举位 `toolset: Option<SessionToolset>`（值集 = `Explore`（explore 笔记工具面）与 `Archi { module: bool, search: bool }`（架构只读工具面，见 desktop-archi-agent））；既有 `tools: Option<Vec<String>>` 位激活为七工具子集收窄表达（值必须为七工具名子集，非法名在装配层 `Err`）。`preamble` 位维持不消费（系统提示词唯一来源仍为 `AGENT.md`，见「SDK 引擎系统提示词（AGENT.md 注入）」）。词汇封闭原则延续：专用工具仅经 `toolset` 枚举注入，自由字符串工具名 MUST NOT 进执行面；core 仅承载词汇，解释归 infra。新增 `Archi` 变体 MUST NOT 改变 `Explore` 臂的任何既有语义（`tools` 收窄位、成员校验、硬禁用、`RunStarted.tools` 上报四条零改写）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分两臂——① `provenance.source == "explore"`：注入 `tools = [read, grep, glob, ls, edit, bash]`（`write` 移除）+ `toolset = Explore`；② `provenance.source == "change"`：SHALL 按 `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}` 的尾三段解析 `(phase, role)` 并查 `(phase, role)` → archi 工具子集矩阵（矩阵单源见 desktop-archi-agent「SessionToolset::Archi 与 (phase, role) 注入矩阵」），命中行注入 `toolset = Archi { .. }` 且 `tools` 位保持 `None`（七工具全量），未命中行与**解析失败（畸形 / 短段 / 未知 phase）**SHALL 恒 `SessionInjections::default()`（fail-closed，零 archi 工具）。其余来源（debug 等）SHALL 恒 `SessionInjections::default()`（七工具面，零变化）。
- **loop 组装与硬禁用**：definitions SHALL 按注入面组装（收窄子集 + `toolset` 命中的专用工具定义——`Explore` 追加 `create_explore_note`、`Archi { module, search }` 按位追加以对应 archi 工具）；`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从 definitions 移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行（现分发链 `policy → sandbox → execute` 无此校验的缺口收口）。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root），sdk loop 经 `LoopTurn` 携带的上下文路由；`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **policy 档位归属**：`create_explore_note` SHALL 归写面档位（Default 拒绝、AcceptEdits / BypassPermissions 放行）——决策表纯函数扩展（见「SDK 引擎静态权限与路径沙箱」表）；两个 archi 只读工具 SHALL 归只读面（三档全放行，与 `read` / `grep` / `glob` / `ls` 同款），写面 / 执行面清单零变化（契约细节见 desktop-archi-agent「只读档位归属与成员校验不变」）。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note（无 `write`），`RunStarted.tools` 逐名一致上报该面

#### Scenario: 幻觉 write 调用被拒

- **WHEN** explore 会话中模型发出已从 definitions 移除的 `write` 工具调用
- **THEN** 成员校验拒绝：`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult（与 tool_use 同 id）回灌，run 不中断、不触执行体

#### Scenario: create 原子定名

- **WHEN** 占位记录（`draft-1760000000000`）会话中模型调用 `create_explore_note(name="api-retry", content=<初稿>)`
- **THEN** 磁盘出现 `openspec/explores/api-retry.md`（内容 = content），store 记录 `name="api-retry"`（`id` / `session_id` 不变），返回文本含笔记路径；占位名文件全程不存在

#### Scenario: create 拒绝面零半边残留

- **WHEN** 分别以非法 kebab 名、撞已有记录名、目标文件已存在（既有笔记）调用 `create_explore_note`
- **THEN** 均显式 `Err` 回灌（文件已存在臂附路径），磁盘与库内零变化（前置校验先于任何写动作）

#### Scenario: 已定名记录异名拦截

- **WHEN** 已定名记录（`api-retry`，笔记在盘）会话中模型调用 `create_explore_note(name="other-name", …)`
- **THEN** `Err` 附现有名 `api-retry` 与笔记路径，零改动；同名且文件缺失时补建成功（孤儿重建）

#### Scenario: 非 explore 来源零变化

- **WHEN** 以 `source="debug"`（或不传来源）发起会话
- **THEN** definitions 与 `RunStarted.tools` 仍为七工具（含 `write`），成员校验对七工具全放行，行为与注入机制上线前一致

#### Scenario: change 来源按矩阵注入 archi 工具面

- **WHEN** 以 `source="change"` 且 `source_ref` 为 `<change_id>/dev-design/executor/1` 发起会话
- **THEN** `toolset = Archi { module: true, search: true }` 且 `tools` 位为 `None`（七通用工具全量）；definitions 含 `archi_module` / `archi_search` 两定义，`RunStarted.tools` 逐名一致上报

#### Scenario: 矩阵未命中行与畸形 source_ref fail-closed

- **WHEN** 分别以 `<change_id>/test-gen/executor/1`、`<change_id>/implement/executor`（段数不足）与空 `source_ref` 发起 change 会话
- **THEN** 三者均落 `SessionInjections::default()`（七工具面、零 archi 工具），不抛装配错误、不回退为全量 archi 工具

#### Scenario: Explore 臂零回归

- **WHEN** 以 `source="explore"` 发起会话并审查装配产物
- **THEN** 六工具收窄子集与 `toolset = Explore` 逐字与本次变更前一致；新增 `Archi` 变体不改变该臂的任何可观察行为

### Requirement: SDK 引擎工具质量与单结果上限（L1）

SDK 引擎自建工具面 SHALL 完成两项质量升级与一项统一上限：

- **read 截断与翻页**：read 工具 SHALL 缺省截断 2000 行，截断时 SHALL 在结果尾部留痕（「已截断，可 offset 翻页」形态）并 SHALL 支持 offset / limit 参数取回后续窗口；现行 read 无大小上限的形态 MUST NOT 延续
- **grep 正则与上下文行**：grep 工具 SHALL 按正则语义匹配（替换现行子串字面匹配，`sdk/tools.rs` 既有形态）并 SHALL 支持上下文行参数（匹配行 ± N 行，缺省 0）；200 条结果上限维持
- **单结果字节上限（防线 L1 层）**：全工具 SHALL 施加统一单结果字节上限（`MAX_RESULT_BYTES` 30KB 量级），超限截断并留痕；bash 工具 stdout/stderr 合并输出同归此层

工具 schema 演进 SHALL 随工具面定义更新（新增参数进 input schema），事件面零改动（ToolUse / ToolResult 词汇不变）。

#### Scenario: read 截断与翻页

- **WHEN** read 一个超过截断阈值的大文件
- **THEN** 返回前 N 行 + 尾部截断留痕；带 offset 请求返回后续窗口，两段拼接覆盖全文

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

- **WHEN** BypassPermissions 档下先执行 `echo {} > openspec/config.json`，再执行 `echo {} > notes.txt`
- **THEN** 第一条被保护闸拒绝（`permission_denied`，run 不中断）；第二条正常执行——护栏只覆盖受保护文件面，不构成命令白/黑名单

### Requirement: SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）

SDK 引擎 SHALL 对**喂 provider 的请求史**（非 store 转录）实施两级防线，整体水位视角：L1 单结果上限恒常（归「SDK 引擎工具质量与单结果上限（L1）」管辖）→ L2 请求前确定性剪裁（约 70-75% 水位）→ L3 LLM compaction（约 85-90% 水位）→ L4 = 现状 provider 报错收敛兜底，**防线设计目标为 L4 永不抵达**。

- **窗长来源**：优先级 SHALL 为「该档模型名 `[1m]` 后缀 > provider 可空 `context_length` 列（desktop-agent-management）> 128K 缺省启发式」，且 SHALL 按档（`high` / `low`）独立解析（见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）；后缀在场时 `context_length` 被覆盖（MUST NOT 叠加或取小值）；`EngineFacade::with_context_window` 签名不变（语义降为「缺省来源」）；token 估算 SHALL 用 UTF-8 字节启发式（rig-memory `HeuristicTokenCounter` 同款，中文重载比英文经验值准）
- **L2 确定性剪裁**（请求前跑，两个调用点：resume 转录重建之后 + loop 每次发请求前）：先 prune 老工具结果——老 tool_result 内容替换为占位符，tool_use / tool_result 配对结构完整，保护最近窗口（40k 保护窗 / 20k prune 门槛）；不够再丢最老完整轮对（rig-memory 算法 + 孤儿配对清理兜底）。**SHALL 保首条 user**（phase agent 首条 user 即任务书）
- **L3 LLM compaction**：剪裁后水位仍升 → 以当前 model 自摘要老历史，新史 = `[摘要, 首条 user, 最近轮对]`；摘要要点集含关键技术决策**及其原因**（参考 opencode）；compaction 失败 SHALL 降级为 L2 硬裁 + SystemNotice 留痕，**MUST NOT 打断 run**（phase agent 场景宁可丢密度不能丢收敛，比 codex 指数退避保守）
- **可观测**：剪裁与压缩 SHALL 发 `SystemNotice{subtype:"context_pruned"|"context_compacted", payload:{before, after, layer}}`（subtype 开放词典现成）
- **不变量**：store 转录**永远全量**——剪裁/压缩只作用于请求史，重放 / 审计 / resume 重建源不受影响；「转录无上限增长」既有边界留痕不变
- **极限场景留痕**：首条 user 单块自身超窗（巨型 phase prompt + 小窗模型）与单轮即超窗，L1-L3 不覆盖，归 L4 兜底 + 错误细分

#### Scenario: prune 老工具结果

- **WHEN** 历史含多轮大型工具结果且估算水位过 L2 阈值
- **THEN** 发请求前老 tool_result 被替换为占位符（配对完整、最近窗口保护、首条 user 在场），`SystemNotice{context_pruned}` 流出，store 转录仍全量

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
- **THEN** 以 128K 启发式计算水位，行为可预期；填列后按填列值计算

#### Scenario: 后缀覆盖填列窗长

- **WHEN** 该档模型名带 `[1m]` 后缀且 provider 同时填有 `context_length = 200000`
- **THEN** 该档防线窗长按 1M 计算（后缀覆盖，不做叠加或取小值），其余档按各自解析结果计算；store 转录与防线算法零变化

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

会话查询契约 SHALL 增加按 session id 的单查面：core `SessionQuery` trait SHALL 增加单查方法（按 id 返回会话行 + 聚合统计 + 轮行清单），infra 实现以 store `find_session` 为底座（store schema 零变更），轮行的运行状态 SHALL 自轮行清单推导（存在 running 轮行即 running，否则取终态）。命令面 SHALL 提供薄包装命令 `session_detail(root, session_id)`（经 builder 注册、bindings 再生成；命令体三件事纪律）。语义约定：不存在 session id SHALL 显式 `Err`（单查语义与清单空态区分，查无此 id 视为调用方错误）；blank root SHALL 返回空结果（与 `agent_sessions` / `agent_session_transcript` 同口径）；单查 SHALL 按 root 解析所属 workspace 库，跨库隔离语义与既有查询命令一致。既有 `agent_session_transcript` 的签名与语义 MUST NOT 收窄或变更；`agent_sessions` SHALL 为分页查询面——`offset` / `limit` 入参、随页返回 `SessionPage { sessions, total }` 分页信封（`total` 为过滤集全量计数、越界 offset 空页恒 total、limit 超上限截断），来源过滤与 `updated_at` 降序稳定序语义不变。

#### Scenario: 单查返回会话行与轮行

- **WHEN** 以已存在（含 running 轮行与已终态两种形态）的 session id invoke `session_detail`
- **THEN** 返回该会话行、聚合统计与轮行清单，running 判定自轮行推导；结果仅来自该 root 所属 workspace 库

#### Scenario: 查无此 id 显式失败

- **WHEN** 以不存在的 session id invoke `session_detail`
- **THEN** 前端收到 `Err(String)`，无静默空态、无新会话或记录产生

#### Scenario: blank root 空结果

- **WHEN** 以空白 root invoke `session_detail`
- **THEN** 返回空结果不进入库解析链路，与既有查询命令口径一致

### Requirement: agent 执行命令面

`commands/exec/` SHALL 承载执行与查询命令，命令体 SHALL 收敛为内核公共 API 的薄包装（三件事纪律：参数转换 → 调用 → 错误映射）：

- 发起命令（现 `agent_start`）：参数转换后调用内核 `run_session` 形状 API——`New` 建立会话、`Continue{id}` 续轮（现 `resume_session_id` 语义平移）；编排（runner 组装、tee、落库、盖戳、收敛）SHALL 全部下沉内核，命令体 MUST NOT 编排。来源参数（`source` / `source_ref`）SHALL 写入会话归属（provenance 记录用）；`source` 不传时缺省 `debug`；`source_ref` SHALL 可携来源侧引用（如 change 详情通用 agent 面板传 change id——两段形态与 change 流程会话的四段 `{changeId}/{phase}/{role}/{attempt}` 精确匹配下不撞），仅用于归属记录与清单筛选，MUST NOT 用于寻址 / 重放（寻址仍走 `sessionId` 直查）；未传时为 null。cwd SHALL 由尾部可选参数 `cwd: Option<String>` 承载（位置索引 8，`None` → `root`）：`cwd` 只赋会话执行根（`SessionCtx.workspace_root`——工具路径沙箱根 / bash `current_dir` / 每轮 `AGENT.md` 读取根），`root` 的 store 身份半边 SHALL 恒为 workspace root（双 root 不变量，与 change 流程 `exec_root` / `store_root` 拆分同构——`cwd` MUST NOT 参与 `for_root`，否则会静默铸出第二份 workspace 库）；`cwd` 非空 SHALL 先做 `is_dir` 探测并以显式 `Err`（携路径）拒绝缺失目录，MUST NOT 让失败深埋进引擎工具调用；cwd MUST NOT 落会话配置快照（续话由来源侧恒显式携带；非来源侧续话不携 cwd 为已知缺口，见 desktop-change-chat）；引擎组装 SHALL 经内核组合根（见「引擎门面与 EngineKind 参数选择」），命令体 MUST NOT 直接构造具体引擎。命令命名与参数面演进由 design 定稿（保留语义升级或更名会话域命令二选一，见提案待决问题）
- 查询命令（现 `agent_runs` / `agent_run_events` / `agent_run_chain`）：演进为会话域查询薄包装（分页会话清单含聚合统计与 total 计数 / 会话转录重放 / 源过滤清单），参数含 root → workspace 库解析 → store/内核查询 → DTO；前端寻址与重放 SHALL 走 `session_detail` 单查（见「session_detail 单查面」）而非按来源反查取最近一条；旧 run 域命令的退役/收窄路径由 design 定稿；另 SHALL 提供 `list_change_chat_sessions(root, id, offset, limit) -> SessionPage` 薄包装（等价 `session_query(source="change", source_ref=<id>)` 的分页清单——`source` 词常量收在命令层，前端零来源词汇；blank root → 空页；见 desktop-change-chat「change 会话归属与新会话 / 历史会话」）
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

headless 无问收口：无头 agent 需用户决策时 SHALL 以问题文本作为本轮最终消息收口（turnDone 正常收敛），MUST NOT 经 ask 类工具中途提问（SDK 引擎工具面为封闭清单，无 ask 类工具、无 disallowedTools 等 CLI flag 组装概念）；用户应答 SHALL 经既有 Continue 机制（`SessionRef::Continue` → store 全史转录重建）同会话续轮。

注释口径：kernel / runner 的「逐轮等待提问 / 回到等待下一轮 / 逐轮驱动（ask 语义）」注释 SHALL 更新为一轮一命口径。

#### Scenario: 泵单轮收口驱动内核 EOF converge

- **WHEN** 引擎泵产出轮终态事件（SDK loop 收敛）
- **THEN** 泵任务返回且观察通道发送端释放（观测通道 `recv` 返回 `None`），内核 `drive` 的观测循环以通道关闭退出并 converge（finish_turn + `TurnFinished` 流出、轮行落库终态、注册表除名）
- **AND** 泵任务不再回到提问接收等待下一轮

#### Scenario: 泵终止后提问按失败收敛

- **WHEN** 同一引擎会话通道在泵返回后再次收到提问送达
- **THEN** 内核 ask 失败臂收敛 failed（记因「轮提问送达失败（引擎侧已终止）」），系统中不存在挂起的常驻泵
- **AND** 消费方每轮经 `compose_turn → begin → open_session` 新开引擎会话（chat Continue 与 walker 每步既有路径零改动）

#### Scenario: disallowedTools flag 末尾组装

- **WHEN** 审查 SDK 引擎的工具面与命令组装面
- **THEN** 工具面为封闭七工具清单（无 ask 类工具），组装面无 disallowedTools 概念（`--disallowedTools` 末位组装规则随 CLI 租户退役，历史 cli 会话转录不受影响）

#### Scenario: 无问文本收口与同会话续轮

- **WHEN** 无头 agent 需要用户决策
- **THEN** 该轮不发生 ask 类工具调用，agent 以问题文本作为本轮最终消息收口（turnDone 正常 converge，终端消息到达消费方）
- **AND** 用户应答后经 Continue（store 全史转录重建）同会话续轮，前轮全史在场（walker 决策环与 chat 面既有路径）

#### Scenario: 停止与 spawn 失败语义保持

- **WHEN** 泵轮中途收到停止信号，或 spawn 失败（SDK 启动失败）
- **THEN** 既有语义不变：停止路径不合成收敛、终止缝被调、轮收敛 `stopped`；spawn 失败以合成收敛事件 failed 记因且泵终止
- **AND** 两路径均为单轮终止，与一轮一命语义同形，无回归

### Requirement: MVP 边界与已知限制

以下边界 SHALL 作为 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **kill 已实现（偿还「无 kill」限制）**：`agent_stop` 提供终止入口（见「agent_stop 终止与提前 resolve」）；残余限制——bash 工具进程树清理失败时子进程可能残留，应用中途关闭时的孤儿进程仍为已知限制
2. **续会话经转录重建落地**：续会话以 store 全史转录重建为唯一重建源（见「SDK 引擎 resume（store 即会话）」）；`--continue` 隐式续会话语义（CLI 时代）随 CLI 租户退役；调试页自身仍不提供 resume 入口（续话仅 explore 会话链消费）
3. **留存无清理**：事件转录无上限增长
4. **交互工具限制（agent-turn-eof 修订）**：SDK 工具面为封闭清单（无 ask 类工具，见「引擎泵一轮一命与 headless 无问收口」）——agent 需用户决策时以问题文本作为本轮最终消息收口，应答经 Continue（store 全史转录重建）同会话续轮；explore / chat 页对转录中既存 AskUserQuestion ToolUse 的静态可读卡渲染（问题/选项原样呈现）与 composer 文本输入续话呈现不变；questionnaire 组件接线为二期
5. **bypassPermissions 默认**：deny 规则仍生效；agent 运行可在 workspace 内无审批改动文件——本裁决限于本机自有 repo 的调试场景

#### Scenario: 终止入口可用

- **WHEN** 审查调试页与 explore 页
- **THEN** 运行中提供停止入口且触发 `agent_stop`；进程树清理失败时的孤儿进程残余留痕于本 spec

#### Scenario: 限制留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

### Requirement: SDK 租户 MVP 边界与已知限制

以下边界 SHALL 作为 SDK 租户 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 MCP**：SDK 引擎不接 MCP 服务器（`RunStarted.mcp_servers` 恒空）
2. **无子代理**：不派生子代理，`parent_tool_use_id` 恒空
3. **无交互审批**：静态三档起步，无审批反向通道（协议留独立 change）
4. **bash 无沙箱（sdk-engine-gap-closure 偿还「bash 缺席」后的知情边界；desktop-write-protection 偿还「受保护文件写入面」半边）**：bash 已进工具面（Default 档拒绝、AcceptEdits / Bypass 放行）；受保护文件（内置工作区配置 + 用户 `write_protection` 规则）的写入经命令文本提取拦截，**但任意命令执行仍无沙箱**——BypassPermissions 默认档下 bash 可执行任意命令、可改动任何未受保护文件；命令级白/黑名单、真沙箱、交互审批留后续 change；已承认的静态判别漏面（`xargs` / `sh -c` 递归拆解 / 生成脚本再执行 / `-m` 模块 / 变量中转 / base64 编码）留痕于 desktop-write-protection
5. **cost 恒缺**：`RunResult.cost_usd` 恒 `None`（无价格表），汇总卡如实呈现
6. **engine_cfg 硬编码位（已偿还，desktop-agent-management）**：api_key / base_url / model 手填真机验证的限制已由全局 agent 管理偿还——engine_cfg 构造源换为管理数据解析（默认 agent / 显式 agent），连接配置经管理页维护，明文边界与遮蔽治理见 desktop-agent-management「api_key 机密边界」
7. **resume 保真度缺口**：Raw 丢弃 / 子代理压平 / 仅顶层重建（L2 请求前剪裁只缓解请求侧成本随历史增长，重建源仍为 store 全量转录）；**服务端块（`WebSearch`）自本条起不再属该缺口**——其 raw 原文经 `raw_use` / `raw_result` 原样保全并原样回放
8. **引擎归属留痕未裁**：run 记录暂不区分引擎（座位 design 裁定后偿还）
9. **无模型元数据表（协议与后缀的知情边界）**：不知道任意模型的 `default/upper` 输出上限，也无法判定 `supports_1m_suffix` / `supports_1m_beta`——故 `[1m]` 后缀一律接受（不做模型白名单校验）、`anthropic-beta` 一律发（第三方网关可能拒 → `api_error` 显式暴露，非静默）；名单外模型输出上限兜底恒 `32_000`，老代模型（8K/16K 上限）与第三方网关需用户在 provider 显式填 `max_tokens` 才可跑通
10. **不实现 `maxOutputTokensRecovery`**：撞输出上限即按既有 `finish` 语义收敛（不注入续写消息、不自动抬高上限），用户调 provider `max_tokens` 后重跑
11. **跨协议续会话不做一致性校验**：改 provider `protocol` 后以 `Continue` 续旧会话，旧史由新 wire 重编码——rig 只回放本 dialect issuer 的 reasoning 块，**thinking 会静默丢弃**；`SessionConfigSnapshot` 不记协议（与「改 base_url / api_key 同权」裁决一致）；system 消息不进持久化史（每轮头部单独注入），故无残留 system 形态问题
12. **云端 web_search 无过程可见性（知情边界）**：本变更不做搜索实时提示（无 `AgentDelta::Search`，`normalize.rs` 与前端 `agent-transport.ts` 零改动）——`pause_turn` 续跑期间时间线可能空白数十秒，唯一过程观测点为 `SystemNotice{pause_turn_continued}`；最终来源与引用在轮收口后可见（终态以库为唯一权威）
13. **搜索转录体积不设上限、不压缩**：`raw_result` 含 `encrypted_content` 与每条来源加密索引（一次搜索数 KB），与既有「转录无上限增长」边界同列；压缩会破坏 raw 逐字节保真，故明确不做
14. **`web_search` 服务端工具不可配置**：`max_uses = 5` 锁为常量（成本护栏），配置化需 store v5 迁移；openai 协议恒不生效（门控在解析期，不静默透传）
15. **升级后不可回退**：旧客户端读到含 `WebSearch` 块的事件会在**整条事件解码**时失败（非少显示一张卡），仓库不提供降级分支
16. **连续 assistant 消息依赖 Anthropic 服务端合并（`pause_turn` 的知情边界）**：`pause_turn` 把一次搜索拆成两条 assistant 消息（use 在前、result 在后）时，回放按原序重建会产出**相邻同角色消息**；rig 编码侧不做同角色合并，故 SHALL 依赖 Anthropic 服务端接受相邻 assistant 消息。该依赖为**已知限制**，本期 MUST NOT 为其设实现期核验门、MUST NOT 作为任务阻塞项（真机凭据不属自动化验证面，把无法履行的核验写成门只会堵塞实现）；确认动作 SHALL 并入**归档后人工验收**——以官方端点 + 凭据的真实 `pause_turn` 会话续轮确认服务端接受相邻同角色消息，结论记入验收记录，**不阻塞实现与归档**。若人工验收确认被拒，降级预案为「回放时按 `pause_turn` 边界**合并相邻 assistant 消息**」（合并对象 = 该轮重建产出的连续 assistant `AssistantContent` 序列，合为一条消息的序列）——**本期不预建、无实现分支**，后续变更据本条偿还

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 十六条边界均可考（含新增的搜索过程不可见 / 转录体积 / 服务端工具不可配置 / 不可回退 / 连续 assistant 消息五条），后续变更无需重新论证是否知情

#### Scenario: 第 4 条边界的精确分界

- **WHEN** 审查 bash 的护栏现状
- **THEN** 受保护文件写入面有护栏（保护闸）、任意命令执行与未受保护文件改动无护栏——两者在同一段边界文本内可分界，MUST NOT 出现「完全无保护」或「已沙箱化」两种失真的表述

#### Scenario: 兜底边界的失败面可见

- **WHEN** anthropic 协议下对「名单外 + 8K 输出上限」模型留空 `max_tokens`（取 32_000 兜底）发起运行
- **THEN** 请求以 provider 400 失败并经 `api_error` 走既有错误收敛（显式、可读诊断），不静默；用户填 `max_tokens = 8192` 后可跑通——该组合作为兜底策略边界的留痕证据

#### Scenario: 相邻 assistant 消息不设实现期核验门

- **WHEN** 审查实现期任务清单与验收面约定（边界条 16 的落点）
- **THEN** 无任何「真机 `pause_turn` 前置核验」的实现期门项或阻塞项；该依赖仅以已知限制留痕，确认动作落于**归档后人工验收**（官方端点 + 凭据，结论记入验收记录），本期无「合并相邻 assistant 消息」的实现分支

#### Scenario: 续跑过程仅 notice 可见

- **WHEN** 一轮较长的搜索触发多次 `pause_turn` 续跑
- **THEN** 实时面只见 `pause_turn_continued` / `pause_turn_capped` 两类 `SystemNotice`（无搜索相关 delta、无「正在搜索…」文案），搜索卡在轮收口后呈查询词与来源列表

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

窗长定档 SHALL 为**按档现算**（`high` / `low` 各自独立）：优先级 **模型名后缀 > provider 可空 `context_length` > 128K 缺省启发式**；后缀在场时 MUST NOT 忽略地叠加或夹取（后缀即该档窗长，`context_length` 被覆盖）。解析与防线装配 SHALL 发生在 `open_session`（那里同时拿到档位与模型原文），`EngineFacade::with_context_window` 签名 MUST NOT 变化——其语义降级为「缺省来源」。非法后缀在启动期 SHALL 以 `AgentStartError::ConfigMissing` 显式失败（MUST NOT 静默降级为裸文发送）。

anthropic 请求参数 SHALL 按以下规则定档：

- **`anthropic-beta`**：`protocol = anthropic` 且该档模型名带 `[1m]` → SHALL 追加 beta `context-1m-2025-08-07`（完全等价 Claude Code 的 1M 开关）；openai 协议或后缀缺席 → MUST NOT 发。
- **输出上限（`max_tokens`）取值链**：`provider.max_tokens`（显式值优先）→ rig 内置模型名单官方值（名单命中时**保留 rig 值、不被降级**）→ `32_000` 兜底（对齐 Claude Code 表外行为）。实现 SHALL 只在该链产出非 `None` 时定型 wire 默认 `max_tokens`（`Messages::with_default_max_tokens`），且**只在 `protocol = anthropic`**；openai 协议 MUST NOT 写入 `max_tokens`（保持「省略由协议自决」现状）。输出上限与 `[1m]` 后缀**完全解耦**（后缀不改输出上限）。
- **上报口径**：`RunStarted.model` 与请求 `model` 为裸名；`SessionConfigSnapshot.model` 保留配置原文（配置快照语义）；引擎日志记 `model=裸名 window=… raw=原文`。

边界知情：无模型元数据表 → `[1m]` 不做模型白名单校验、beta 一律发（第三方网关拒绝以 `api_error` 显式暴露）；不实现 `maxOutputTokensRecovery`（撞上限即按既有 `finish` 语义收敛，用户调 `max_tokens` 后重跑）。上述边界见「SDK 租户 MVP 边界与已知限制」。

#### Scenario: 后缀解析规则表

- **WHEN** 以 `claude-sonnet-4-6[1m]`、`CLAUDE-SONNET-4-6[1M]`、`glm-4.6`、`  glm-4.6  `、`x[2m]`、`x[8k]`、`x[]`、`x[1m]y` 驱动 `parse_model_spec`
- **THEN** 前两者 → 裸名 + `length = 1_000_000`（大小写不敏感）；`glm-4.6` 与带空白的同一名称 → 裸名 + `length = None`（空白裁剪）；其后四者 → `ModelSpecError`（显式拒收，MUST NOT 把原文透传）

#### Scenario: 按档窗长独立

- **WHEN** provider `models.high = "claude-sonnet-4-6[1m]"`、`models.low = "glm-4.6"` 且 `context_length = 200000`
- **THEN** high 档会话的防线窗长为 1M（后缀覆盖 `context_length`）、low 档会话为 200000；同一 provider 两档各自独立解析，`EngineFacade::with_context_window` 签名不变

#### Scenario: 后缀缺席回落到 context_length

- **WHEN** 三档模型名均不带后缀，`context_length` 分别填 200000 与留空
- **THEN** 填列时防线窗长取填列值、留空时取 128K 缺省启发式（既有行为零变化）

#### Scenario: beta 条件发

- **WHEN** `protocol = anthropic` + 模型名 `[1m]`；`protocol = anthropic` + 无后缀；`protocol = openai` + 模型名 `[1m]` 三种组合
- **THEN** 仅第一种请求含 `anthropic-beta: context-1m-2025-08-07`；后两者不发 beta（openai 协议的 `[1m]` 只影响本地防线窗长）

#### Scenario: 输出上限三档取值链

- **WHEN** anthropic 协议下分别以（a）`provider.max_tokens = 8192`、（b）`max_tokens` 留空 + 名单内官方模型（如 `claude-sonnet-4-6`）、（c）`max_tokens` 留空 + 名单外模型（如 `glm-4.6` / `claude-3-5-sonnet-*`）组装
- **THEN** （a）请求 `max_tokens = 8192`（不被 rig 官方值覆盖）；（b）请求取 rig 官方值（不被降级为兜底值）；（c）请求 `max_tokens = 32000`（兜底，请求不再触发 `max_tokens must be set for Anthropic` 的编码期失败）

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

- **常量单点**：`WEB_SEARCH_TOOL_KIND = "web_search_20250305"` / `WEB_SEARCH_TOOL_NAME = "web_search"` / `WEB_SEARCH_MAX_USES = 5` / `ANTHROPIC_RAW_CONTENT_KEY = "anthropic_content"` / `CITATIONS_KEY = "citations"` SHALL 由 `sdk/web_search.rs` 单点持有（MUST NOT 在 loop / resume 各写字面量）。
- **投影规则**（`project_choice(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)`）：先按 `tool_use_id` 索引全部 `web_search_tool_result`，再顺序遍历 choice——
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
- **装配时机**：保护规则 SHALL 每 run 装配一次（会话泵起播时 `protect::load`），随 `LoopTurn.protect` 下发（与 `defense` / `retry` / `liveness` 的「装配点算好、loop 纯消费」形态同构）；loop 内 MUST NOT 读盘、MUST NOT 每 tool_call 重读配置。
- **工作区根通道（新 builder 缝）**：root 只在命令层显式在手（store 句柄无 root 字段），故 SHALL 沿既有先例逐层下行：

```
compose_turn(stores, registry, streams, store, agent, workspace_root: &Path)
  → EngineFacade::with_workspace_root(root)
  → SdkRunner.workspace_root: Option<PathBuf>
  → PumpPayload.workspace_root → 泵起播时 protect::load(…)
  → LoopTurn.protect: ProtectRules
```

  `workspace_root` MUST 为**主仓** root（命令层 `root` 参数），MUST NOT 取会话执行根 `SessionCtx.workspace_root`（change 流程下为 worktree）——工作区配置的权威在主仓。
- **缺席语义**：`workspace_root` 为 `None`（未注入）时 SHALL 只装载内置规则（用户规则缺席），MUST NOT 关闭保护闸、MUST NOT 退化为空规则集。
- **射程边界**：专用工具 `create_explore_note` MUST NOT 经保护闸（专用工具不在保护射程内为既有裁定，一致性缺口留痕于 desktop-write-protection）。
- **错误归属次序**：沙箱拒绝（`sandbox_denied`）SHALL 优先于保护拒绝（`permission_denied`）——逃逸路径先按沙箱报错，不被保护闸抢先。

#### Scenario: 四段顺序与拒绝形态

- **WHEN** BypassPermissions 档下模型调用 `write` 指向 `<root>/openspec/config.json`（路径在 root 内）
- **THEN** 事件流含 `SystemNotice{subtype:"permission_denied", payload:{tool:"write", reason:…}}` 与同 id 的 is_error ToolResult（内容为同一拒绝串）；`tools::execute` 未被调用（文件未被写）；run 不中断，同轮后续工具调用照常执行

#### Scenario: 沙箱先于保护闸

- **WHEN** 模型调用 `write` 指向 `<root>/../outside/openspec/config.json`（越出 workspace root）
- **THEN** 拒绝理由为沙箱成因（`subtype:"sandbox_denied"`），而非 `permission_denied`——两段闸的错误归属不重叠、不抢报

#### Scenario: 每 run 一次装配与 loop 零 IO

- **WHEN** 在一次 run 进行中修改 `openspec/config.json` 的 `write_protection`，随后同一 run 内继续调用工具
- **THEN** 本轮行为不随修改改变（规则在泵起播时已装配）；`sdk/loop.rs` 内无 `config::load` / `fs` / `canonicalize` 调用点

#### Scenario: 主仓口径与 worktree 执行根

- **WHEN** change 流程会话的执行根为 worktree（`SessionCtx.workspace_root` = worktree 路径），主仓 root 另经 builder 缝注入
- **THEN** 用户规则取自主仓 `config.json`；写 worktree 内 `openspec/config.json` 仍被内置规则拒（内置 glob 不区分主仓 / worktree）

#### Scenario: 未注入只内置

- **WHEN** 门面未注入 `workspace_root`（`None`）
- **THEN** 保护闸仍生效且只含内置规则（写工作区配置被拒），用户配置的 glob 不生效

#### Scenario: 专用工具直通

- **WHEN** explore 会话调用 `create_explore_note`，且用户配置保护了该工具的目标目录
- **THEN** 调用不被保护闸拦截（专用工具直通为知情边界），`write` / `edit` 对同目录的写入仍被拒

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/agent`（裸名 `agent`） | 会话域内核 | `AgentEvent` 双层词汇（+`MessageDelta`；块词汇五变体含 `WebSearch` 与 `Text.citations`；TurnDone 统一口径）；P1 引擎协议（spawn/question/stop/观察回流 + 四不变量）；`run_session` / `query_sessions` 公共 API；治理面（盖戳/状态机/停止注册）；持久面（write-through）与查询契约；port 定义（core 定接口、infra 实现，落点 design 裁定）；无直接 IO、无 claude/引擎/Tauri、无 workspace 内依赖 |
| `crates/core/agent/src/kernel.rs` | 内核收口缝 | `drive` 以观测通道关闭（引擎侧单边 EOF）为唯一收口缝（无状态机 break / stop select / 第二收口路径，agent-turn-eof 裁定） |
| `packages/desktop/src-tauri/crates/infra/agent`（裸名 `agent-runtime`） | 引擎门面 + 双引擎实现 | `EngineKind`（`cli` \| `sdk`，specta camelCase）；`runner_for(kind, engine_cfg)`（唯一 match 分发）；`EngineConfig{protocol, api_key, base_url, model_high, model_low}`（连接配置字段面，协议在内）由内核组合根注入（机密面放宽注释标记见 desktop-agent-management 边界表）；SHALL 实现内核 port（store 持久面与查询面落地）；`rig` facade 恒编译（`=0.43.0` 锁版 + 显式 features）；`EngineFacade::with_context_window` / `with_max_tokens` / `with_web_search` 承接 provider `context_length` / `max_tokens` / `web_search`（不经 `EngineConfig`） |
| `crates/infra/agent/src/cli/` | CLI 引擎协议实现 | `session_pump` 一轮一命（单问服务后返回 → 内核 EOF 收口；停止/spawn 失败单轮终止语义不动，agent-turn-eof）；flag 组装 `--disallowedTools AskUserQuestion` 恒居参数序列末位（`--resume` 之后）；jsonl 泵与 resume flag 行为延续；恢复走 engine_handle 原生 `--resume` |
| `crates/infra/agent/src/sdk/` | SDK 引擎协议实现 | `normalize.rs` 双层化（Text/ReasoningDelta → delta、轮末密封）；`loop.rs` 密封收口；`resume.rs` 会话全史重建；`runner.rs` 协议实现且 `session_pump` 一轮一命（跨轮史槽不存，会话史唯一源 = store 全史转录重建，agent-turn-eof）；工具面/权限/沙箱语义见 SDK 各专章（AGENT.md 注入 / L1 工具质量 / bash / 三档分化 / L2-L3 防线） |
| `crates/infra/agent/src/sdk/{loop,normalize,resume,runner,tools,policy,config}.rs` | facade 路径迁移 | `rig_core::` → `rig::`（纯 re-export 同名）；`rig_core::` 路径零残留 |
| `crates/infra/agent/src/sdk/preamble.rs` | AGENT.md 注入 | 起播前读 `<workspace_root>/AGENT.md` 逐字注入；每轮重读、缺席 `None`、无 `CLAUDE.md` 兜底、32KB 截断；零 core 改动 |
| `crates/infra/agent/src/sdk/tools.rs` + `bash.rs` | 七工具面与质量 | `TOOL_NAMES` 七项；read 截断 + offset 翻页；grep 正则 + 上下文行；全工具单结果字节上限（`MAX_RESULT_BYTES`）；bash `tokio::process` + git-bash 探测 / `cmd /C` 兜底 / unix `sh -c`；`kill_on_drop` + `taskkill /T /F` 形状清理；超时活性护栏（缺省 120s、范围校验 [1s, 30min]，越界显式拒绝） |
| `crates/infra/agent/src/sdk/policy.rs` | 三档决策表（bash 分化） | bash：Default 拒、AcceptEdits / Bypass 放；AcceptEdits ≠ Bypass 首次真实分化锁定测试 |
| `crates/infra/agent/src/sdk/{context,compact}.rs` | L2 / L3 防线 | 窗长 = 该档模型名 `[1m]` 后缀 > provider `context_length` > 128K 启发式（按档独立）；字节启发式 token 估算（bytes ÷ 4）；prune 占位符 + 轮对裁剪 + 保首条 user；LLM compaction（失败降级硬裁，run 不中断）；`context_pruned` / `context_compacted` SystemNotice；调用点 = resume 重建后 + 每请求前；store 转录全量不变 |
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
| `packages/desktop/src/lib/agent-adapter.ts` | 事件适配双层 | delta → provisional（correlator）、密封替换；`eventsToUIMessages` 仅消费密封转录；详见 desktop-agent-chat-infra |
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
| `AgentBlock::Text` | `{ text: String, citations: Option<Vec<AgentCitation>> }` | 增 `citations`（`#[serde(default)]`，MUST NOT 加 `skip_serializing_if`：键恒在、序列化落 `null`）；旧 JSON 读出 `None` |
| `AgentBlock::WebSearch` | `{ id: String, query: String, results: Vec<WebSearchSource>, error: Option<String>, raw_use: Option<serde_json::Value>, raw_result: Option<serde_json::Value> }` | 一次 server use 与其配对 result 的原子表达；`raw_*` 为回放唯一事实源 |
| `AgentCitation` | `{ url: Option<String>, title: Option<String>, cited_text: Option<String>, raw: serde_json::Value }` | 引用（`raw` = 原始 wire 值，回放用） |
| `WebSearchSource` | `{ url: String, title: Option<String>, page_age: Option<String> }` | 单条搜索来源（展示字段） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/web_search.rs`（新）

| 项 | 签名 | 描述 |
|------|------|----------|
| 常量面 | `WEB_SEARCH_TOOL_KIND` / `WEB_SEARCH_TOOL_NAME` / `WEB_SEARCH_MAX_USES` / `ANTHROPIC_RAW_CONTENT_KEY` / `CITATIONS_KEY` | 字面量单点（loop / resume 零重复） |
| `provider_tool` | `fn() -> ProviderToolDefinition` | `{type: web_search_20250305, name: web_search, max_uses: 5}` |
| `raw_content` | `fn(&Text) -> Option<&serde_json::Value>` | 取 `additional_params.anthropic_content` |
| `citations_of` | `fn(&Text) -> Option<Vec<AgentCitation>>` | 展示字段自 raw 解，`raw` 保原值 |
| `project_choice` | `fn(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)` | 块投影唯一实现（use+result 收敛、孤儿 result、未知 server 块跳过） |
| `replay_raw` | `fn(&serde_json::Value) -> AssistantContent` | `Text{text:"", additional_params:{anthropic_content: raw}}`（raw 块必须配空文本） |
| `replay_text` | `fn(&str, Option<&[AgentCitation]>) -> AssistantContent` | 非空 citations → `additional_params.citations = [raw, …]` |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `LoopTurn` | 增 `web_search: bool` | runner 经 `PumpPayload` 下传的最终开关 |
| 请求构造 | `request.provider_tool(web_search::provider_tool())`（条件） | 开关为真才挂；`tools`（客户端面）不变 |
| 块转换 | 换 `web_search::project_choice(&choice)` | 删内联四变体 match（块语义单一实现） |
| `'attempt` 返回值 | `(choice, usage, finish_reason)` | `CompletionResponse::finish_reason()` |
| 续跑循环 | 常量 `PAUSE_TURN_MAX_CONTINUATIONS = 5` | 独立计数轴（`attempt` 每次续跑前归零）；两条 notice；到顶按 `stop` 收口 |

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
