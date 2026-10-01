# desktop-agent-execution Delta

## ADDED Requirements

### Requirement: 会话域内核公共 API 与会话一等公民

`core/agent` SHALL 以会话为公共 API 的寻址与生命周期单位，提供两件公共能力（用户愿景原文的契约化）：

- **运行会话**：`run_session(New | Continue{id}, prompt)` 形状的会话建立与轮驱动——`New` 建立新会话，`Continue{id}` 以既有会话续发新一轮；返回 token 级增量流（思考/回复可辨）与可终止句柄。会话过程流式传输、能区分思考和回复、可终止。
- **查询会话**：`query_sessions(...)` 形状的会话记录查询——会话清单与聚合统计（提问轮数、累计墙钟、累计 token 消耗；MVP 从轮统计行现算，MUST NOT 维护累计列，字段面可扩展）。

session SHALL 为一等公民：会话 id 由 core 铸造；转录 = 会话全史（密封事件直挂 session，单表），resume 重建源 SHALL 为会话全史，MUST NOT 以单 run 事件拼凑转录。core SHALL 维护双 id 映射（core session id ↔ engine_handle）；engine_handle 经协议到达（spawn 响应或事件上报，见「P1 引擎协议不变量与缺省降级」）。会话 SHALL 存引擎/模型/权限档的**配置快照**而非跨库引用（`AgentInstanceRecord` 在全局库、session 在 workspace 库，禁跨库引用；实例改名/删除不伤历史会话）。会话 SHALL 承载来源归属（source / source_ref），explore 链等消费方经会话归属寻址。

#### Scenario: 新建会话运行

- **WHEN** 以 `New` 发起一次含多轮工具调用的提问并等待收口
- **THEN** 返回的流内可观测 token 级增量（text / thinking 可辨），轮末以密封 Message 收口，终态以 TurnDone 收敛；句柄可在运行中请求终止并收敛 `stopped`

#### Scenario: 续会话全史重建

- **WHEN** 以 `Continue{id}` 对一个已有两轮往复的会话续发第三轮
- **THEN** 引擎侧重建源为该会话全部前两轮的密封转录（第一轮不丢），第三轮照常收口并追加为本会话新轮

#### Scenario: 会话查询与聚合统计

- **WHEN** 对某 workspace 查询会话清单
- **THEN** 返回会话记录及现算聚合统计（轮数/累计墙钟/累计 token，缺席字段合法缺省），无需维护累计列

#### Scenario: 双 id 映射

- **WHEN** 会话建立后引擎经 spawn 响应或事件上报 engine_handle
- **THEN** core 记录 core session id ↔ engine_handle 双 id 映射，续轮与恢复按映射寻址

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

## MODIFIED Requirements

### Requirement: agent 边界与双 crate 分层

Desktop 后端 SHALL 将「agent 执行」确立为第五类边界（agent 边界），并按 core/infra 分离落地为两个 crate：

- `crates/core/agent`（裸名 `agent`）：**会话域内核**——会话域中立契约与内核编排：`AgentEvent` 词汇（delta/sealed 双层）、P1 引擎协议（会话建立 / question / stop / 观察回流）、会话状态机与收敛状态机（running → completed/failed/stopped）、seq/时间戳盖戳治理、停止注册（内存态）、write-through 持久化编排、查询契约与 port 定义。纯度原则修订（2026-10-01 裁定）：「纯类型无 IO」改写为「**无对外部世界的直接 IO、经 port**」——文件/进程/网络等直接 IO MUST NOT 出现，IO 一律经 core 定义的 port 由 infra 实现；进程内存原语（时钟盖戳、mpsc/Notify、内存注册表）不在禁令内。MUST NOT 依赖 Tauri，MUST NOT 执行进程 spawn，MUST NOT 认识 claude（不出现 claude 特有 flag 名与 JSONL 行解析），MUST NOT 认识引擎（不出现 engine / rig / openai 概念）；loop / memory / LLM 协议归 infra 不变。
- `crates/infra/agent`（裸名 `agent-runtime`）：agent 边界的引擎门面与实现——对外以 `EngineKind`（`cli` | `sdk`）参数选择引擎，引擎分发 SHALL 为门面内唯一 match 点；内部承载 `cli/` 引擎（CLI 发现（Windows `.cmd`）、进程 spawn、stdout 逐行泵、JSONL 解析归一化）与 `sdk/` 引擎（rig 进程内租户：agent loop、自建工具面、静态权限 policy、路径沙箱、事件归一化、会话转录恢复）。SHALL 实现内核 port（含 store 持久面与查询面要求的落地）；MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，非 shell 边界职责）；`rig-core` SHALL 恒编译、MUST NOT 设 feature gate（SDK 引擎定位为不依赖本地环境的 agent，开箱即用，不提供 CLI-only 裁剪档）。

依赖方向 SHALL 为：壳 crate → `agent` + `agent-runtime`，`agent-runtime → agent`；`agent` MUST NOT 依赖 workspace 内任何 crate（port 落点裁定的默认形态：core 定义 port、infra 实现，该规则保持不变；若 design 裁定 core 直依 store crate，须同步修订 desktop-crate-layout）。两 crate SHALL 注册进 `src-tauri/Cargo.toml` workspace members（rust 套件注册于 src-tauri 根，测试自动覆盖），且 MUST NOT 据此预建其他 infra 边界目录。单 crate 起步（`cli/`、`sdk/` 子模块收拢）；拆引擎子 crate + 门面 re-export 仅作膨胀后的逃生门，MUST NOT 预建。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查 `src-tauri/Cargo.toml` workspace members 与各 crate 依赖声明
- **THEN** members 含 `crates/core/agent` 与 `crates/infra/agent`，`agent-runtime` 依赖 `agent` 与 `rig-core`，壳依赖两者
- **AND** `agent` 与 `agent-runtime` 的依赖中无 tauri 系条目，`agent` 无 workspace 内依赖（core 定义 port、infra 实现形态下）

#### Scenario: core 不认识 claude 与引擎且无直接 IO

- **WHEN** 审查 `crates/core/agent` 源码
- **THEN** 无 claude 特有概念（flag 名、JSONL 行解析、claude 事件 type 名），亦无引擎概念（engine / rig / openai 字样）；无文件/进程/网络直接 IO 触点（IO 全经 port 签名表达）；claude 细节全部关在 `agent-runtime` 的 `cli/` 引擎内，rig / openai 细节全部关在 `sdk/` 引擎内

#### Scenario: spawn 住在 infra

- **WHEN** 检索进程 spawn 调用（`std::process` / `tokio::process`）
- **THEN** 仅出现在 `crates/infra/agent` 的 `cli/` 引擎子模块；`core/agent` 与壳 command 层无 spawn

#### Scenario: 引擎唯一分发点

- **WHEN** 审查 `agent-runtime` 的引擎构造入口（`runner_for` 形状）
- **THEN** `EngineKind` → 引擎构造的 match 唯一，内核组合根仅做参数映射不经手引擎细节；新增引擎只动门面一处

### Requirement: AgentEvent 信封与 loop 可见性

`core/agent` SHALL 定义统一事件信封 `AgentEvent`，作为引擎共享的逻辑事件模型（非 stdout/线格式直译），词汇为 **delta/sealed 双层**：

- `RunStarted` { model, session_id, tools, mcp_servers, … }（源自 system/init；session_id 即引擎侧标识上报形态之一）
- `MessageDelta` { text/thinking 增量，配对键 }（**新增，ephemeral**）：token 级增量，思考/回复可辨；只上传输面，**store 永不见**；配对键形态（`parent_tool_use_id` vs 显式 correlator）由 dev-design 定稿
- `Message` { role, blocks, parent_tool_use_id }（**密封，durable**）：唯一落库与重放单元；密封粒度 = 引擎一次 assistant 回应（全部块收进恰一条密封 Message）。其中 Block = Text | Thinking | ToolUse{id, name, input} | ToolResult{id, content, is_error}
- `SystemNotice` { subtype, payload }（permission_denied / api_retry 等 system 事件，subtype 不做封闭枚举）
- `TurnDone`（现 `RunResult` 演进，更名/别名由 design 定稿）{ subtype, is_error, num_turns, duration_ms, cost, usage, session_id }：**统计唯一口径**（见「P1 引擎协议不变量与缺省降级」），唯一驱动收敛的变体
- `Raw` { type, 原文 JSON }（未知 / 未来事件类型透传）

loop 调试关键信息 SHALL 在一等公民位：tool_use 与 tool_result 成对可见、子代理经 `parent_tool_use_id` 归因、TurnDone 携带统计字段。每事件 SHALL 携带 `seq`（单调序号）与时间戳（盖戳治理收内核）；delta 与密封的 seq 编号关系（共享空间容忍库内空洞 vs 独立编号）由 design 定稿。`Raw` 变体 SHALL 承接所有未识别事件：MUST NOT 因未知 type 丢弃事件或解析失败（永不丢事件、永不炸解析）。

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

### Requirement: AgentRunner trait 与逻辑事件流抽象

`core/agent` SHALL 以 P1 引擎协议（参考 Actor，不照搬）承载引擎契约，trait 面 SHALL 仅暴露协议词汇与运行参数，进程模型（spawn、stdout/stdin、退出码）MUST NOT 出现在 trait 面上：

- **会话建立**（spawn 语义）：injections（prompt + tools）+ ctx（workspace root、部分 config、可扩展）+ session 引用（New | Continue{id}）
- **轮驱动**：question；**终止**：stop（`RunHandle` 竞态语义保留为默认形态，Stop 消息替代与否由 design 定稿）
- **观察回流**：Delta / Sealed / Notice / Raw / TurnDone（见「AgentEvent 信封与 loop 可见性」）

协议 SHALL 满足 P1 四不变量（流式可实现 / 统计口径唯一 / 唯一标识上报 / 注入轮分离，见「P1 引擎协议不变量与缺省降级」）。事件流 SHALL 以有界通道承载；抽象 SHALL 支持以假引擎（预录事件序列）替换真实引擎供内核与上层测试。多租户（本机 CLI / 进程内 SDK / 远程 API）SHALL 都能落在协议预留内——CLI 泵 stdout 生产流，SDK 进程内泵任务灌流，API 以轮询 / watch 模拟流；本变更实现 CLI 与 SDK（rig 进程内）两租户，远程 API 租户 MUST NOT 预建实现。协议参数面 MUST NOT 出现 engine / agent 概念——引擎选择住 infra 门面，core 契约零污染。

run 状态机 SHALL 位于 `core/agent`（治理面）：running → completed | failed | stopped，由 TurnDone（含 is_error）驱动收敛为 completed / failed，显式终止请求 SHALL 经独立收敛分支收敛为 `stopped`，收敛后 MUST NOT 再接受状态变更。

#### Scenario: 协议面无进程概念

- **WHEN** 审查引擎协议 trait 与参数 / 事件流类型签名
- **THEN** 无进程句柄、stdout/stdin、子进程退出码等进程模型类型，仅协议词汇与运行参数；且无 engine / agent 选择参数

#### Scenario: 假引擎可替换

- **WHEN** 内核编排测试以预录事件序列的假引擎注入
- **THEN** write-through 持久化 / 状态机收敛 / 会话轮推进全程可测，无需真实 CLI 进程或真实 SDK 调用

#### Scenario: 状态机收敛

- **WHEN** 事件流出现 `is_error=true` 的 TurnDone
- **THEN** run 状态收敛为 failed；success 时收敛为 completed；显式终止请求时收敛为 stopped；收敛后不再接受状态变更

#### Scenario: 双引擎协议同构

- **WHEN** SDK 与 CLI 引擎各自实现引擎协议并被同一内核编排消费
- **THEN** 内核编排代码无引擎分支（零改动复用）；停止信号对两引擎同样生效

### Requirement: agent_stop 终止与提前 resolve

命令面 SHALL 提供终止入口，发起命令 SHALL 维持提前 resolve 契约：

- 发起命令 SHALL 在会话/轮记录落库并进入 running 后**提前 resolve**，返回 running 态记录（id 立即可知）；执行 SHALL 转入内核编排继续（内核持久面持有 store 收尾），终态记录经 Channel 以与实时事件同构的状态部件流出，MUST NOT 再依赖 invoke 返回携带终态。
- 终止命令 SHALL 按运行中的会话活动轮寻址: 将轮状态收敛为受控终态 `stopped`、尽力终止引擎执行体（CLI 引擎为 Windows 进程树击杀（`cmd /C` 包装下 `taskkill /T` 或等价）；SDK 引擎为经停止信号取消泵任务（drop future）——机制由 design 定稿核实；寻址键（run/轮 id 沿用或会话键，root 消解跨库歧义保留）由 design 定稿）、经 Channel 流出终止终态部件（与实时事件同构）。`stopped` SHALL 与 `completed` / `failed` 同列受控终态（用户主动终止与引擎失败语义区分，收敛后 MUST NOT 再接受状态变更）。停止注册（内存态）收内核治理面。
- 对非 running 目标（已终态或不存在）的终止 SHALL 幂等忽略，MUST NOT 报错崩溃或误改既有终态。
- 停止后的重放 SHALL 与实时一致: 重放路径从会话转录（含 `stopped` 终态）还原与实时查看相同的呈现（终态部件同构）。

#### Scenario: 提前 resolve

- **WHEN** 前端发起会话运行
- **THEN** invoke 在轮记录落库进入 running 后即 resolve 返回 running 态记录（id 可用），引擎事件随后经 Channel 持续流入，终态以状态部件经 Channel 流出

#### Scenario: stop 收敛与引擎终止

- **WHEN** 运行中 invoke 终止命令
- **THEN** 轮状态收敛为 `stopped`，Channel 流出终止终态部件；CLI 运行的进程树被击杀（claude 子进程不再存活），SDK 运行的泵任务被取消（事件流终止、future drop）

#### Scenario: 幂等忽略

- **WHEN** 对一条已 `completed` 的轮 invoke 终止命令
- **THEN** 幂等忽略: 状态不变、不报错、不产生新事件

#### Scenario: 停止后重放一致

- **WHEN** 停止一轮运行后重新打开该会话查看
- **THEN** 重放呈现与停止时实时呈现一致（含 `stopped` 终态部件），续话以会话最新状态为准

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 rig-core 实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig-core` SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 首公民 SHALL 为 openai-compatible 端点（自定义 base_url——GLM / DeepSeek / Kimi / 自部署第一公民）；anthropic provider 降为后续可选
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层
- **流面双层交付**：流内 `Text` / `ReasoningDelta` 增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面）；轮末 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：MVP 工具集 = read / grep / glob / ls / write / edit；bash 工具 MUST NOT 进 MVP 工具面（权限三档表中 bash 拒绝语义为将来引入时的预留档位）；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- 事件归一化：openai chat completions 形状的 `tool_calls` 数组 SHALL 映射为 ToolUse 块并与 ToolResult 成对（同 id）；`RunStarted` SHALL 各报各的（model 取 `EngineConfig.model`、tools 报自建工具集）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
- 依赖足迹：SHALL 以 `default-features = false` + openai feature 最小化入树（既有实测留痕不回退）

#### Scenario: 事件归一化 fixture

- **WHEN** 以 openai 形状流 fixture（text delta × N、reasoning delta、tool_calls、未知字段、轮末 choice）驱动 normalize 纯函数
- **THEN** 产出 MessageDelta(text/thinking) 序列、Message(ToolUse) 与成对 Message(ToolResult)（同 id）、轮末恰一条密封 Message（块齐）、未知内容 Raw 透传，seq 单调递增；全程不打网络

#### Scenario: 无 CLI 环境可用

- **WHEN** 无本地 claude CLI 的机器上以 sdk 引擎发起运行（手填 EngineConfig）
- **THEN** 运行正常发起并完成完整 loop（CLI 发现路径不被触及）

#### Scenario: 依赖足迹不回退

- **WHEN** 审查 rig-core 入树配置
- **THEN** 恒编译、无 feature gate，`default-features = false` + openai feature 维持（体积与传递依赖结论沿用既有 spike 留痕）

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

事件投递 SHALL 双路：内核持久面（write-through 单点落库）+ Tauri Channel（命令作用域实时流，推前端时间线）；两路密封事件同源同构。历史查看 SHALL 走所属 workspace 库的会话查询重放（携 root），MUST NOT 依赖全局事件广播或要求运行进程存活。MVP MUST NOT 实现留存清理（转录无上限增长为已知限制留痕）。

#### Scenario: write-through 双路同源

- **WHEN** 运行产生密封事件
- **THEN** 每密封事件到达 Channel 与 store 且以同一盖戳同源；delta 只出现在 Channel 路、永不出现在 store

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

- 发起命令（现 `agent_start`）：参数转换后调用内核 `run_session` 形状 API——`New` 建立会话、`Continue{id}` 续轮（现 `resume_session_id` 语义平移）；编排（runner 组装、tee、落库、盖戳、收敛）SHALL 全部下沉内核，命令体 MUST NOT 编排。来源参数（`source` / `source_ref`）SHALL 写入会话归属；不传时缺省 `debug`。cwd 恒为当前 workspace root（root 解析所属 workspace 库保留）；引擎组装 SHALL 经内核组合根（见「引擎门面与 EngineKind 参数选择」），命令体 MUST NOT 直接构造具体引擎。命令命名与参数面演进由 design 定稿（保留语义升级或更名会话域命令二选一，见提案待决问题）
- 查询命令（现 `agent_runs` / `agent_run_events` / `agent_run_chain`）：演进为会话域查询薄包装（会话清单含聚合统计 / 会话转录重放 / 会话按来源寻址），参数含 root → workspace 库解析 → store/内核查询 → DTO；旧 run 域命令的退役/收窄路径由 design 定稿
- 终止命令（现 `agent_stop`）：见「agent_stop 终止与提前 resolve」；root 寻址保留

错误约定沿用既有模板 `Result<T, String>`：CLI 不可发现、spawn 失败、SDK 引擎配置缺失（经 `AgentStartError` 中性变体）、无默认 agent 可解析、workspace 库打开失败等 SHALL 以 `Err` 抵达前端，MUST NOT 静默吞掉。

#### Scenario: 命令体薄包装

- **WHEN** 审查发起命令实现
- **THEN** 命令体为参数转换 + 调用内核 API + 错误映射三段，编排（tee/落库/收敛/盖戳）在内核内，不膨胀命令体

#### Scenario: 来源与会话续接参数透传

- **WHEN** explore 页以 `source="explore"`、`source_ref=<记录键>` 且携带会话引用发起续轮
- **THEN** 生成会话归属携带对应字段且以该会话全史续接（SDK 走转录重建、CLI 走 engine_handle resume）；调试页 invoke（不传来源参数）生成的会话 `source="debug"`

#### Scenario: agent 缺省与解析

- **WHEN** invoke 不带 `agent` 参数（正式场景如 explore 链恒不传）
- **THEN** 内核组合根缺省解析默认 agent 并经门面构造对应引擎（无默认 agent 时显式 `Err`，不落库不推流、不静默回退）；调试页显式传 agent 时按所选解析；协议参数面全程无 agent / engine 概念（core 零污染）

#### Scenario: 查询与停止 root 寻址

- **WHEN** 前端 invoke 会话查询与终止命令
- **THEN** 命令无状态、按 root 解析所属 workspace 库直查/寻址返回 DTO，无领域解释；workspace A 的调用看不到 B 的会话

#### Scenario: 错误 reject 传达

- **WHEN** CLI 不可发现、SDK 引擎配置缺失、无默认 agent 可解析或所属 workspace 库打开失败时发起运行
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

### Requirement: 引擎门面与 EngineKind 参数选择

`agent-runtime` SHALL 作为 agent 引擎门面：对外一套应用协议（invoke 面 + 事件流 + 会话记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎；协议统一 SHALL 落在内核协议层而非各引擎私有形态。门面 SHALL 提供 `runner_for(kind, engine_cfg)` 形状的构造入口——`EngineConfig` 结构体形态（api_key / base_url / model 三件套）由组合根注入，SDK 引擎用以组装 rig client；CLI 引擎不消费 engine_cfg。**解析单点自命令层下沉至内核组合根**（desktop-agent-management「默认 agent 与运行发起解析」的落点平移，解析语义不变）：缺省路径解析默认 agent、显式路径按所选 agent 解析、产物为 `EngineKind` + `EngineConfig`（sdk 时由引用 provider 组装、model 取 provider 三档 high 档）。换解析落点时 `EngineConfig` 消费面 MUST NOT 变化。启动失败抹平：`AgentStartError` 的中性变体 `ConfigMissing` 承接 SDK 侧连接配置不齐等启动失败——core 唯一触碰点，变体集 MUST NOT 再增。

#### Scenario: 门面构造与 core 零污染

- **WHEN** 审查 `runner_for` 实现与 `crates/core/agent` 源码
- **THEN** 引擎分发唯一 match、返回引擎协议实现；core 无 engine / `EngineConfig` / rig / agent 概念，`AgentStartError` 变体集不变

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现与门面签名
- **THEN** 解析语义（默认 agent / 显式 agent / sdk provider 组装 / high 档 / 无默认 `Err`）与下沉前一致，`runner_for` 签名与 `EngineConfig` 结构体零 diff，命令层无引擎解析残留

### Requirement: Agent 调试页

前端 SHALL 提供 Agent 调试页（`AgentDebugView`），经侧栏「系统工具」组进入；视图切换 SHALL 沿用本地 state，MUST NOT 引入路由。页面 SHALL 包含：

- **参数面（最小集）**：prompt（必填）、permission-mode 三档下拉（默认 bypassPermissions）、agent 选择器（管理页 agent 实例清单，默认选中默认 agent——与组合根缺省解析一致；**仅调试页暴露**，正式场景无 agent 选择入口）；cwd 不设参数（隐含当前 workspace root）、model 不设参数、env 不设参数
- **事件时间线**：SHALL 经共享组件族 `AgentTimeline`（desktop-agent-chat-infra）以保真透镜呈现——**运行中 token 级流式**（delta → provisional 消息连续增长，思考/回复可辨，密封到达以密封为准替换，无碎行）、seq 序、tool_use / tool_result 成对、子代理归因、result 汇总卡（统计字段可复制、缺席如实缺省）；两引擎共用同一时间线组件
- **运行中停止**：运行中 SHALL 呈现停止入口，触发终止命令（见「agent_stop 终止与提前 resolve」）
- **原始转录切换**：落库密封转录（含 Raw）原文可见（与 store 一致）；delta 仅实时流可见，不进入原始转录视图
- **历史会话**：会话列表 → 点开自会话转录重放（invoke 查询）

页面状态 SHALL 经统一会话基建（`use-agent-chat`）承载；run 表单 / 历史列表 / 转录开关为页面级 chrome，包在共享核心外圈。实时流经 transport 走 Tauri Channel 订阅（执行流通道，不属轮询取数）；查询类取数（会话列表 / 转录重放）仍由用户显式动作触发。

#### Scenario: 参数面默认值

- **WHEN** 打开调试页
- **THEN** permission-mode 默认 bypassPermissions、agent 选择器默认选中默认 agent（与组合根缺省解析一致）、prompt 为空且必填；无 model / cwd / env / 引擎直选输入

#### Scenario: agent 切换

- **WHEN** agent 选择器选 sdk agent 发起一次含工具调用的运行
- **THEN** 运行走 SDK 引擎（连接配置来自所选 agent 的引用 provider），时间线 / 落库 / 重放组件零改动复用

#### Scenario: 流式连续呈现

- **WHEN** 一次含长文本回复与思考的 SDK 运行进行中
- **THEN** 时间线呈单条连续增长的 provisional 消息（token 级、思考/回复可辨），轮末密封到达后以密封内容替换，不出现逐 delta 碎行

#### Scenario: loop 可见性

- **WHEN** 一次含工具调用的运行完成
- **THEN** 时间线可按序看到 assistant(tool_use) → tool_result → … → result 汇总卡，统计字段可读可复制（SDK 的 cost 如实缺省），子代理消息归因到父工具调用

#### Scenario: 运行中停止

- **WHEN** 调试页运行中点击停止
- **THEN** 触发终止命令，事件流以终止终态收尾，轮状态收敛为 `stopped`

#### Scenario: 原始转录切换

- **WHEN** 切换原始转录视图
- **THEN** 落库密封转录（含 Raw 透传件）原文可见，与 store 一致；视图中无 delta 记录

#### Scenario: 历史重放

- **WHEN** 从历史会话列表点开一条已完成会话
- **THEN** 经 invoke 查询以会话全史转录渲染完整时间线，不要求原运行进程存活

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/agent`（裸名 `agent`） | 会话域内核 | `AgentEvent` 双层词汇（+`MessageDelta`；TurnDone 统一口径）；P1 引擎协议（spawn/question/stop/观察回流 + 四不变量）；`run_session` / `query_sessions` 公共 API；治理面（盖戳/状态机/停止注册）；持久面（write-through）与查询契约；port 定义（core 定接口、infra 实现，落点 design 裁定）；无直接 IO、无 claude/引擎/Tauri、无 workspace 内依赖 |
| `crates/infra/agent/src/sdk/` | SDK 引擎协议实现 | `normalize.rs` 双层化（Text/ReasoningDelta → delta、轮末密封）；`loop.rs` 密封收口；`resume.rs` 会话全史重建；`runner.rs` 协议实现；工具面/权限/沙箱语义不动 |
| `crates/infra/agent/src/cli/` | CLI 引擎协议实现 | 行为零回归（flag 组装/jsonl 泵/resume flag）；协议词汇适配；恢复走 engine_handle 原生 `--resume` |
| `crates/infra/store` | 会话转录唯一家 + 查询面实现 | `SessionRecord` + 转录单表（session 键）+ 轮统计行；write-through 原子操作面；会话查询/对账 API；详见 desktop-workspace-store delta |
| `src/commands/exec/agent.rs` | 内核薄包装命令面 | 三件事纪律；编排退役下沉；会话域查询/终止命令；`Result<T, String>` 模板；命名与参数面 design 定稿 |
| 引擎门面 + 内核组合根 | 解析单点（自命令层下沉） | `runner_for` / `EngineConfig` 零 diff；默认 agent 解析语义不变（desktop-agent-management） |
| `packages/desktop/src/lib/agent-adapter.ts` | 事件适配双层 | delta → provisional（correlator）、密封替换；`eventsToUIMessages` 仅消费密封转录；详见 desktop-agent-chat-infra delta |
| `packages/desktop/src/lib/agent-transport.ts` + `src/hooks/use-agent-chat.ts` | 会话域传输与状态基建 | 会话域 invoke/流参数；重放装载走会话转录；详见 desktop-agent-chat-infra delta |
| `packages/desktop/src-tauri/src/bindings/` | IPC 类型镜像 | 经 export-bindings 再生成（信封与命令面演进），非手改；快照测试同步再生成 |
