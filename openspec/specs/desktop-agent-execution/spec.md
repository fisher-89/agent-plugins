# desktop-agent-execution Specification

## Purpose

定义 agent 执行能力（第五类边界）的完整契约：`core/agent` 中立契约（AgentEvent 信封、AgentRunner trait、run 状态机）、`infra/agent`（裸名 `agent-runtime`）的引擎门面与双引擎租户（`cli/` 本机 Claude CLI、`sdk/` rig 进程内 SDK）、workspace 维度运行记录落库与重放、exec 轨道首批命令与前端 Agent 调试页，以及两租户 MVP 边界留痕。

## Requirements

### Requirement: agent 边界与双 crate 分层

Desktop 后端 SHALL 将「agent 执行」确立为第五类边界（agent 边界），并按 core/infra 分离落地为两个 crate：

- `crates/core/agent`（裸名 `agent`）：**会话域内核**——会话域中立契约与内核编排：`AgentEvent` 词汇（delta/sealed 双层）、P1 引擎协议（会话建立 / question / stop / 观察回流）、会话状态机与收敛状态机（running → completed/failed/stopped）、seq/时间戳盖戳治理、停止注册（内存态）、write-through 持久化编排、查询契约与 port 定义。纯度原则修订（2026-10-01 裁定）：「纯类型无 IO」改写为「**无对外部世界的直接 IO、经 port**」——文件/进程/网络等直接 IO MUST NOT 出现，IO 一律经 core 定义的 port 由 infra 实现；进程内存原语（时钟盖戳、mpsc/Notify、内存注册表）不在禁令内。MUST NOT 依赖 Tauri，MUST NOT 执行进程 spawn，MUST NOT 认识 claude（不出现 claude 特有 flag 名与 JSONL 行解析），MUST NOT 认识引擎（不出现 engine / rig / openai 概念）；loop / memory / LLM 协议归 infra 不变。
- `crates/infra/agent`（裸名 `agent-runtime`）：agent 边界的引擎门面与实现——对外以 `EngineKind`（`cli` | `sdk`）参数选择引擎，引擎分发 SHALL 为门面内唯一 match 点；内部承载 `cli/` 引擎（CLI 发现（Windows `.cmd`）、进程 spawn、stdout 逐行泵、JSONL 解析归一化）与 `sdk/` 引擎（rig 进程内租户：agent loop、自建工具面、静态权限 policy、路径沙箱、事件归一化、会话转录恢复）。SHALL 实现内核 port（含 store 持久面与查询面要求的落地）；MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，非 shell 边界职责）；`rig`（rig-core 的 re-export facade，sdk-engine-gap-closure 自直连切换）SHALL 恒编译、MUST NOT 设 feature gate（SDK 引擎定位为不依赖本地环境的 agent，开箱即用，不提供 CLI-only 裁剪档；rig-core 经 facade 传递入树）。

依赖方向 SHALL 为：壳 crate → `agent` + `agent-runtime`，`agent-runtime → agent`；`agent` MUST NOT 依赖 workspace 内任何 crate（port 落点裁定的默认形态：core 定义 port、infra 实现，该规则保持不变；若 design 裁定 core 直依 store crate，须同步修订 desktop-crate-layout）。两 crate SHALL 注册进 `src-tauri/Cargo.toml` workspace members（rust 套件注册于 src-tauri 根，测试自动覆盖），且 MUST NOT 据此预建其他 infra 边界目录。单 crate 起步（`cli/`、`sdk/` 子模块收拢）；拆引擎子 crate + 门面 re-export 仅作膨胀后的逃生门，MUST NOT 预建。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查 `src-tauri/Cargo.toml` workspace members 与各 crate 依赖声明
- **THEN** members 含 `crates/core/agent` 与 `crates/infra/agent`，`agent-runtime` 依赖 `agent` 与 `rig`（facade），壳依赖两者
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

### Requirement: Claude CLI 租户（MVP）

`agent-runtime` 的 `cli/` 引擎 SHALL 以本机 Claude Code CLI（`-p` 无头 + `--output-format stream-json --verbose`）实现 `AgentRunner`：

- stream-json SHALL 是唯一线上格式（解析路径唯一）；text / json 格式 MUST NOT 进入实现
- 环境档位 MUST NOT 进运行参数面：`AgentRunParams` 无 env 字段，`agent_start` 无 env 参数，运行恒走完整环境（default）；`--bare` flag MUST NOT 组装（bare 档与其认证前提提示已从调试页 / explore 页参数面移除）。run 记录 `env` 列保留（store 既有 schema），落库恒为 `"default"`
- permission-mode SHALL 参数面三档（`default` / `acceptEdits` / `bypassPermissions`），默认 `bypassPermissions`（`--dangerously-skip-permissions`）——`-p` 默认 `default` 档下需审批工具直接被拒、看不到真实 loop，本机自有 repo 的调试页场景裁决以完整循环为默认
- cwd SHALL 为当前 workspace root（隐含，不设参数）；model MUST NOT 进 MVP 参数面（继承用户 CLI 默认）
- CLI 发现 SHALL 处理 Windows `.cmd` shim（`cmd /C` 包装或解析真实入口）；CLI 不可发现时 SHALL 显式报错（错误事件 / Err），MUST NOT 静默空转
- **resume 续会话 SHALL 实现**：`AgentRunParams` 增可选 `resume_session_id: Option<String>`，非空时 flag 组装 `--resume <id>`（与 `-p` 组合即「以该 session 续发新一轮」）；`None` 时行为与既有 one-shot 完全一致。`--continue`（隐式续最近会话）MUST NOT 实现——续话一律显式 `session_id`，语义明确可追溯
- 用户终止运行 SHALL 经 `agent_stop` 命令实现（进程树击杀与状态收敛见「agent_stop 终止与提前 resolve」，非 flag 面职责）；MVP MUST NOT 实现：`--continue` 隐式续会话、`--include-partial-messages` 增量流

#### Scenario: flag 组装

- **WHEN** 以 bypassPermissions 参数组装命令行
- **THEN** 含 `-p --output-format stream-json --verbose --dangerously-skip-permissions`，不含 `--bare` 与 `--resume`

#### Scenario: resume flag 组装

- **WHEN** 以 `resume_session_id = Some("sess-1")` 组装命令行
- **THEN** 参数含 `--resume sess-1`，其余 flag（`-p` / stream-json / `--verbose` / permission 档位）不变；`None` 时不出现 `--resume`

#### Scenario: JSONL 逐行泵

- **WHEN** 以多行 stream-json 输出 fixture 驱动 runner 解析（不经真实进程）
- **THEN** 逐行归一化为 AgentEvent 序列且行间无串扰，进程退出后 run 收敛

#### Scenario: Windows CLI 发现

- **WHEN** PATH 上的 `claude` 为 `.cmd` shim
- **THEN** spawn 成功（经 `cmd /C` 包装或解析后的真实入口）；CLI 不存在时得到显式错误而非静默空转

### Requirement: 引擎门面与 EngineKind 参数选择

`agent-runtime` SHALL 作为 agent 引擎门面：对外一套应用协议（invoke 面 + 事件流 + 会话记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎；协议统一 SHALL 落在内核协议层而非各引擎私有形态。门面 SHALL 提供 `runner_for(kind, engine_cfg)` 形状的构造入口——`EngineConfig` 结构体形态（api_key / base_url / model 三件套）由组合根注入，SDK 引擎用以组装 rig client；CLI 引擎不消费 engine_cfg。**解析单点自命令层下沉至内核组合根**（desktop-agent-management「默认 agent 与运行发起解析」的落点平移，解析语义不变）：缺省路径解析默认 agent、显式路径按所选 agent 解析、产物为 `EngineKind` + `EngineConfig`（sdk 时由引用 provider 组装、model 取 provider 三档 high 档）。换解析落点时 `EngineConfig` 消费面 MUST NOT 变化。启动失败抹平：`AgentStartError` 的中性变体 `ConfigMissing` 承接 SDK 侧连接配置不齐等启动失败——core 唯一触碰点，变体集 MUST NOT 再增。

#### Scenario: 门面构造与 core 零污染

- **WHEN** 审查 `runner_for` 实现与 `crates/core/agent` 源码
- **THEN** 引擎分发唯一 match、返回引擎协议实现；core 无 engine / `EngineConfig` / rig / agent 概念，`AgentStartError` 变体集不变

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现与门面签名
- **THEN** 解析语义（默认 agent / 显式 agent / sdk provider 组装 / high 档 / 无默认 `Err`）与下沉前一致，`runner_for` 签名与 `EngineConfig` 结构体零 diff，命令层无引擎解析残留

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 首公民 SHALL 为 openai-compatible 端点（自定义 base_url——GLM / DeepSeek / Kimi / 自部署第一公民）；anthropic provider 降为后续可选
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层
- **流面双层交付**：流内 `Text` / `ReasoningDelta` 增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面）；轮末 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：MVP 工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求 preamble SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections` 消费面维持现状
- **上下文管理**：喂 provider 的请求史 SHALL 经 L1-L3 上下文防线治理（见「SDK 引擎工具质量与单结果上限（L1）」「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）；store 转录全量不变
- 事件归一化：openai chat completions 形状的 `tool_calls` 数组 SHALL 映射为 ToolUse 块并与 ToolResult 成对（同 id）；`RunStarted` SHALL 各报各的（model 取 `EngineConfig.model`、tools 报自建七工具集）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
- 依赖足迹：workspace 依赖 SHALL 为 `rig` facade 条目（`=0.42.0` 锁版平移、MUST NOT 升版本；`default-features = false` + 显式 features：`reqwest` / `native-tls` / memory feature；rig-core 经 facade 传递入树，MUST NOT 保留直连残留——spike 定稿：双条目 pin，`rig-core` 条目保留为解析钉子，双条目强制锁 0.42.0）

#### Scenario: 事件归一化 fixture

- **WHEN** 以 openai 形状流 fixture（text delta × N、reasoning delta、tool_calls、未知字段、轮末 choice）驱动 normalize 纯函数
- **THEN** 产出 MessageDelta(text/thinking) 序列、Message(ToolUse) 与成对 Message(ToolResult)（同 id）、轮末恰一条密封 Message（块齐）、未知内容 Raw 透传，seq 单调递增；全程不打网络

#### Scenario: 无 CLI 环境可用

- **WHEN** 无本地 claude CLI 的机器上以 sdk 引擎发起运行（手填 EngineConfig）
- **THEN** 运行正常发起并完成完整 loop（CLI 发现路径不被触及）

#### Scenario: facade 依赖足迹

- **WHEN** 审查 workspace 依赖与 `crates/infra/agent` 源码
- **THEN** 依赖为 `rig` facade 条目（锁版 `=0.42.0`、`default-features = false` + 显式 features），`rig_core::` 路径零残留；恒编译、无 feature gate

### Requirement: SDK 引擎系统提示词（AGENT.md 注入）

SDK 引擎 SHALL 在起播前读取 workspace root 的 `AGENT.md` 单份文件作为系统提示词来源：文件存在 → 内容**逐字**注入请求 `preamble`（不包装、不改写）；文件缺席 → `preamble` 保持 `None`（现状不变）。读取范围 SHALL 严格限于 `AGENT.md`，MUST NOT 做 `CLAUDE.md` 等兜底回退。注入 SHALL **每轮重读**：会话中途修改 `AGENT.md`，改动自下一轮请求生效（不做起播快照——bash 引入后 agent 可改自己的系统提示词，该语义行为最简且可预期）。

该机制 SHALL 为引擎内部特性（P2 原则：跨会话持久上下文是 agent 产品特性非协议义务），`crates/core/agent` 零改动；`SessionInjections` 消费面维持现状（compose 恒传 default 不动）。preamble 自身体量上限 32KB（char 边界截断）、嵌套 `AGENT.md` 不级联。

#### Scenario: AGENT.md 在场逐字注入

- **WHEN** workspace root 存在 `AGENT.md` 时以 sdk 引擎发起运行
- **THEN** 请求 preamble 为该文件内容逐字（不包装、不改写），`crates/core/agent` 与 compose 面零 diff

#### Scenario: 缺席维持现状

- **WHEN** workspace root 无 `AGENT.md` 时以 sdk 引擎发起运行
- **THEN** preamble 为 `None`，请求形状与注入机制上线前一致

#### Scenario: 会话中修改下一轮生效

- **WHEN** 会话运行中途修改 `AGENT.md` 后续发下一轮
- **THEN** 已发轮次不受影响，下一轮请求注入修改后的内容（每轮重读，无起播快照）

#### Scenario: 无兜底回退

- **WHEN** workspace root 只有 `CLAUDE.md` 而无 `AGENT.md`
- **THEN** 不发生兜底读取，preamble 仍为 `None`（严格单文件语义）

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

SDK 引擎工具面 SHALL 增第七工具 `bash`，`RunStarted.tools` SHALL 报七工具：

- **档位归属**：Default 档拒绝、AcceptEdits / BypassPermissions 档放行（见「SDK 引擎静态权限与路径沙箱」的三档分化）；拒绝形态与 write/edit 同构（`SystemNotice{subtype:"permission_denied"}` + is_error=true ToolResult，run 不中断）
- **进程执行**：`tokio::process`，cwd = workspace root，环境继承当前进程；shell 底座 Windows SHALL 优先探测 git-bash（unix 语法对模型成功率最高），探测失败退 `cmd /C` 兜底（不损开箱即跑）；unix 侧 `sh -c`
- **停止语义**：`kill_on_drop(true)` + Windows 进程树清理复用 `cli/runner.rs` `kill_process_tree` 的 `taskkill /T /F` 形状（future drop 不自动杀子进程树；仅复用形状，`cli/` 文件零 diff）
- **超时**：缺省 120s、钳位 [1s, 600s]——活性护栏，非安全护栏
- **知情边界**：授权/沙箱/安全机制不进本期——BypassPermissions 默认档下 bash 即模型可执行任意命令、护栏为零；命令级白/黑名单、真沙箱、交互审批留后续 change（边界条款见「SDK 租户 MVP 边界与已知限制」）

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

### Requirement: SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）

SDK 引擎 SHALL 对**喂 provider 的请求史**（非 store 转录）实施两级防线，整体水位视角：L1 单结果上限恒常（归「SDK 引擎工具质量与单结果上限（L1）」管辖）→ L2 请求前确定性剪裁（约 70-75% 水位）→ L3 LLM compaction（约 85-90% 水位）→ L4 = 现状 provider 报错收敛兜底，**防线设计目标为 L4 永不抵达**。

- **窗长来源**：消费 provider 记录可空 `context_length` 列（desktop-agent-management）；缺席时 SHALL 以 128K 缺省启发式；token 估算 SHALL 用 UTF-8 字节启发式（rig-memory `HeuristicTokenCounter` 同款，中文重载比英文经验值准）
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

- **WHEN** provider 记录 `context_length` 缺席
- **THEN** 以 128K 启发式计算水位，行为可预期；填列后按填列值计算

### Requirement: SDK 引擎静态权限与路径沙箱

SDK 引擎 SHALL 以静态权限三档起步（无审批反向通道——事件流外的应答协议不进 MVP）：Default 档工具面 = 只读（read / grep / glob / ls），write / edit / bash 拒绝；AcceptEdits 档 = + write / edit / bash；BypassPermissions 档 = 全放行。bash 档位语义（Default 拒绝、AcceptEdits / BypassPermissions 放行）为现有三档对第七工具的自然延伸——只读档不给执行权；**AcceptEdits 与 BypassPermissions 自此首次真实分化**（此前两臂行为完全一致）。默认档 SHALL 为 BypassPermissions（与调试页既有默认一致——本机自有 repo 的调试场景以完整循环为默认）。拒绝 SHALL 经 `SystemNotice{subtype:"permission_denied"}` + is_error=true 的 ToolResult 流出（与 CLI `-p` 无头 Default 档行为同构）。policy SHALL 为纯决策表（档位 × 工具名 → 允许 / 拒绝）密集测试；交互审批协议（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）MUST NOT 实现（留独立 change）。

路径沙箱：write / edit / read 工具全链 SHALL 经 canonicalize 后做 workspace root 前缀校验（拦 `..\` 相对逃逸与符号链接绕过 root 外目标），校验 SHALL 为纯函数并可密集测试。BypassPermissions 默认档下路径沙箱 SHALL 仍是模型与任意**文件写**（write / edit）之间的唯一护栏，其实现质量为必过关卡；bash 进工具面后，**命令执行不在路径沙箱射程内**——BypassPermissions 默认档下 bash 即模型可执行任意命令、护栏为零，此为知情边界留痕（见「SDK 租户 MVP 边界与已知限制」）而非安全缺口遗漏；命令级白/黑名单 MUST NOT 作为安全手段（假安全），真沙箱（job object / 容器）与交互审批留后续 change。

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

### Requirement: session_detail 单查面

会话查询契约 SHALL 增加按 session id 的单查面：core `SessionQuery` trait SHALL 增加单查方法（按 id 返回会话行 + 聚合统计 + 轮行清单），infra 实现以 store `find_session` 为底座（store schema 零变更），轮行的运行状态 SHALL 自轮行清单推导（存在 running 轮行即 running，否则取终态）。命令面 SHALL 提供薄包装命令 `session_detail(root, session_id)`（经 builder 注册、bindings 再生成；命令体三件事纪律）。语义约定：不存在 session id SHALL 显式 `Err`（单查语义与清单空态区分，查无此 id 视为调用方错误）；blank root SHALL 返回空结果（与 `agent_sessions` / `agent_session_transcript` 同口径）；单查 SHALL 按 root 解析所属 workspace 库，跨库隔离语义与既有查询命令一致。既有 `agent_sessions` / `agent_session_transcript` 的签名与语义 MUST NOT 收窄或变更。

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

### Requirement: 调试页历史来源筛选

Agent 调试页历史区 SHALL 提供会话来源筛选：debug / change / 全部三态，默认 debug（现状不变）。筛选 SHALL 参数化既有取数（`agentSessions(root, source或null, null)`，去硬编码 `'debug'`），切换筛选即重查；取数模型 SHALL 保持显式刷新模式（挂载自动取数 + refresh 重取），MUST NOT 引入轮询、定时刷新或事件订阅。重放链路（点开会话 → 转录重放）SHALL 对三态来源同等可用，复用 `AgentRunHistory` 整套，MUST NOT 为 change 来源另建第二套清单/重放 UI。

#### Scenario: 默认 debug 现状不变

- **WHEN** 打开调试页（未动筛选）
- **THEN** 历史区仅呈现 debug 来源会话，与既有行为一致

#### Scenario: 切换筛选重查

- **WHEN** 筛选切到 change（或全部）
- **THEN** 以对应 source（或不过滤）重查会话清单并呈现 change 会话；再次显式刷新按当前筛选重取，无自动轮询

#### Scenario: 三态来源重放同等可用

- **WHEN** 从筛选结果中点开一条 change 来源会话
- **THEN** 经 `agent_session_transcript` 重放完整转录，与 debug 来源会话同一套重放链路

### Requirement: 引擎泵一轮一命与 headless 无问收口

`agent-runtime` 的 `cli/` 与 `sdk/` 引擎会话泵任务（`session_pump`）SHALL 服务**恰一个提问**：轮终态产出后泵任务 SHALL 返回（观察通道发送端随之释放 → 内核观测通道关闭）。泵 MUST NOT 回到提问接收等待下一轮（多问循环 MUST NOT 残留）；spawn 失败与停止路径的既有单轮终止语义 SHALL 保持不变。会话史 SHALL 不依赖泵内跨轮回灌（SDK 泵的跨轮 `history_slot` 随一轮一命移除；会话史唯一来源仍是 store 全史转录重建）。

`core/agent` 的 `drive` SHALL 保持「观测通道关闭（引擎侧单边 EOF）→ converge」为**唯一收口缝**，MUST NOT 引入状态机 break、stop select 或第二收口路径；内核测试假 runner 的引擎侧单边 EOF（`close_stream`）即该契约的测试面，真实泵 SHALL 履行同一契约。

消费纪律：任何消费方对同一逻辑会话的每一轮 SHALL 经 `compose_turn → begin → open_session` 新开引擎会话（既有消费现实：chat Continue 与 walker 每步均如此）；泵返回后同一引擎会话通道上的提问送达 SHALL 走内核 ask 失败臂（failed 记因「轮提问送达失败（引擎侧已终止）」），MUST NOT 复活常驻泵。

headless 禁 ask：CLI 引擎命令行组装 SHALL 追加 `--disallowedTools AskUserQuestion`，且 MUST 置于参数序列**末尾**（`--resume` 及其值之后——variadic flag 置中间会吞并后续参数）。无头 agent 需用户决策时 SHALL 以问题文本作为本轮最终消息收口（turnDone 正常收敛），MUST NOT 经 AskUserQuestion 工具中途提问；用户应答 SHALL 经既有 Continue 机制（`SessionRef::Continue` → store 取 `engine_session_id` → `--resume`）同会话续轮。SDK 引擎工具面为封闭清单（无 ask 类工具），MUST NOT 组装该 flag。

注释口径：kernel / runner 的「逐轮等待提问 / 回到等待下一轮 / 逐轮驱动（ask 语义）」注释 SHALL 更新为一轮一命口径。

#### Scenario: 泵单轮收口驱动内核 EOF converge

- **WHEN** 引擎泵产出轮终态事件（CLI 进程退出 / SDK loop 收敛）
- **THEN** 泵任务返回且观察通道发送端释放（观测通道 `recv` 返回 `None`），内核 `drive` 的观测循环以通道关闭退出并 converge（finish_turn + `TurnFinished` 流出、轮行落库终态、注册表除名）
- **AND** 泵任务不再回到提问接收等待下一轮

#### Scenario: 泵终止后提问按失败收敛

- **WHEN** 同一引擎会话通道在泵返回后再次收到提问送达
- **THEN** 内核 ask 失败臂收敛 failed（记因「轮提问送达失败（引擎侧已终止）」），系统中不存在挂起的常驻泵
- **AND** 消费方每轮经 `compose_turn → begin → open_session` 新开引擎会话（chat Continue 与 walker 每步既有路径零改动）

#### Scenario: disallowedTools flag 末尾组装

- **WHEN** 以任意 permission 档位（含带 `--resume` 的续会话）组装 CLI 命令行
- **THEN** 参数序列末尾两枚为 `--disallowedTools` 与 `AskUserQuestion`，位于 `--resume` 及其值之后，其余 flag（`-p` / stream-json / `--verbose` / permission 档位）不变
- **AND** SDK 引擎的组装面无 disallowedTools 概念（工具面封闭清单本无 ask 工具）

#### Scenario: 无问文本收口与同会话续轮

- **WHEN** 无头 CLI agent 需要用户决策
- **THEN** 该轮不发生 AskUserQuestion 工具调用，agent 以问题文本作为本轮最终消息收口（turnDone 正常 converge，终端消息到达消费方）
- **AND** 用户应答后经 Continue（store 取 `engine_session_id` → `--resume`）同会话续轮，前轮全史在场（walker 决策环与 chat 面既有路径）

#### Scenario: 停止与 spawn 失败语义保持

- **WHEN** 泵轮中途收到停止信号，或 spawn 失败（CLI 缺失）
- **THEN** 既有语义不变：停止路径不合成收敛、击杀缝被调、轮收敛 `stopped`；spawn 失败以合成收敛事件 failed 记因且泵终止
- **AND** 两路径均为单轮终止，与一轮一命语义同形，无回归

### Requirement: MVP 边界与已知限制

以下边界 SHALL 作为 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **kill 已实现（偿还「无 kill」限制）**：`agent_stop` 提供终止入口（见「agent_stop 终止与提前 resolve」）；残余限制——进程树击杀失败时 claude 子进程可能残留（Windows 尤甚），应用中途关闭时的孤儿进程仍为已知限制
2. **续会话经 resume 落地**：`--resume <session_id>` 随 explore 会话链实现（见「Claude CLI 租户（MVP）」）；`--continue` 隐式续会话 MUST NOT 实现；调试页自身仍不提供 resume 入口（续话仅 explore 会话链消费）
3. **留存无清理**：事件转录无上限增长
4. **交互工具限制（agent-turn-eof 修订）**：headless CLI 运行 SHALL 显式禁用 AskUserQuestion（`--disallowedTools AskUserQuestion`，见「引擎泵一轮一命与 headless 无问收口」）——agent 需用户决策时以问题文本作为本轮最终消息收口，应答经 Continue（`--resume`）同会话续轮；explore / chat 页对转录中既存 AskUserQuestion ToolUse 的静态可读卡渲染（问题/选项原样呈现）与 composer 文本输入续话呈现不变；questionnaire 组件接线为二期
5. **bypassPermissions 默认**：deny 规则仍生效；agent 运行可在 workspace 内无审批改动文件——本裁决限于本机自有 repo 的调试场景

#### Scenario: 终止入口可用

- **WHEN** 审查调试页与 explore 页
- **THEN** 运行中提供停止入口且触发 `agent_stop`；进程树击杀失败时的孤儿进程残余留痕于本 spec

#### Scenario: 限制留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

### Requirement: SDK 租户 MVP 边界与已知限制

以下边界 SHALL 作为 SDK 租户 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 MCP**：SDK 引擎不接 MCP 服务器（`RunStarted.mcp_servers` 恒空）
2. **无子代理**：不派生子代理，`parent_tool_use_id` 恒空
3. **无交互审批**：静态三档起步，无审批反向通道（协议留独立 change）
4. **bash 无沙箱（sdk-engine-gap-closure 偿还「bash 缺席」后的知情边界）**：bash 已进工具面（Default 档拒绝、AcceptEdits / Bypass 放行），但授权/沙箱/安全机制不进本期——BypassPermissions 默认档下 bash 即模型可执行任意命令、护栏为零；命令级白/黑名单、真沙箱、交互审批留后续 change
5. **cost 恒缺**：`RunResult.cost_usd` 恒 `None`（无价格表），汇总卡如实呈现
6. **engine_cfg 硬编码位（已偿还，desktop-agent-management）**：api_key / base_url / model 手填真机验证的限制已由全局 agent 管理偿还——engine_cfg 构造源换为管理数据解析（默认 agent / 显式 agent），连接配置经管理页维护，明文边界与遮蔽治理见 desktop-agent-management「api_key 机密边界」
7. **resume 保真度缺口**：Raw 丢弃 / 子代理压平 / 仅顶层重建（L2 请求前剪裁只缓解请求侧成本随历史增长，重建源仍为 store 全量转录）
8. **引擎归属留痕未裁**：run 记录暂不区分引擎（座位 design 裁定后偿还）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 八条边界均可考，后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/agent`（裸名 `agent`） | 会话域内核 | `AgentEvent` 双层词汇（+`MessageDelta`；TurnDone 统一口径）；P1 引擎协议（spawn/question/stop/观察回流 + 四不变量）；`run_session` / `query_sessions` 公共 API；治理面（盖戳/状态机/停止注册）；持久面（write-through）与查询契约；port 定义（core 定接口、infra 实现，落点 design 裁定）；无直接 IO、无 claude/引擎/Tauri、无 workspace 内依赖 |
| `crates/core/agent/src/kernel.rs` | 内核收口缝 | `drive` 以观测通道关闭（引擎侧单边 EOF）为唯一收口缝（无状态机 break / stop select / 第二收口路径，agent-turn-eof 裁定） |
| `packages/desktop/src-tauri/crates/infra/agent`（裸名 `agent-runtime`） | 引擎门面 + 双引擎实现 | `EngineKind`（`cli` \| `sdk`，specta camelCase）；`runner_for(kind, engine_cfg)`（唯一 match 分发）；`EngineConfig{api_key, base_url, model}` 由内核组合根注入（机密面放宽注释标记见 desktop-agent-management 边界表）；SHALL 实现内核 port（store 持久面与查询面落地）；`rig` facade 恒编译（`=0.42.0` 锁版 + 显式 features）；`EngineFacade::with_context_window` 承接 provider `context_length`（不经 `EngineConfig`） |
| `crates/infra/agent/src/cli/` | CLI 引擎协议实现 | `session_pump` 一轮一命（单问服务后返回 → 内核 EOF 收口；停止/spawn 失败单轮终止语义不动，agent-turn-eof）；flag 组装 `--disallowedTools AskUserQuestion` 恒居参数序列末位（`--resume` 之后）；jsonl 泵与 resume flag 行为延续；恢复走 engine_handle 原生 `--resume` |
| `crates/infra/agent/src/sdk/` | SDK 引擎协议实现 | `normalize.rs` 双层化（Text/ReasoningDelta → delta、轮末密封）；`loop.rs` 密封收口；`resume.rs` 会话全史重建；`runner.rs` 协议实现且 `session_pump` 一轮一命（跨轮史槽不存，会话史唯一源 = store 全史转录重建，agent-turn-eof）；工具面/权限/沙箱语义见 SDK 各专章（AGENT.md 注入 / L1 工具质量 / bash / 三档分化 / L2-L3 防线） |
| `crates/infra/agent/src/sdk/{loop,normalize,resume,runner,tools,policy,config}.rs` | facade 路径迁移 | `rig_core::` → `rig::`（纯 re-export 同名）；`rig_core::` 路径零残留 |
| `crates/infra/agent/src/sdk/preamble.rs` | AGENT.md 注入 | 起播前读 `<workspace_root>/AGENT.md` 逐字注入；每轮重读、缺席 `None`、无 `CLAUDE.md` 兜底、32KB 截断；零 core 改动 |
| `crates/infra/agent/src/sdk/tools.rs` + `bash.rs` | 七工具面与质量 | `TOOL_NAMES` 七项；read 截断 + offset 翻页；grep 正则 + 上下文行；全工具单结果字节上限（`MAX_RESULT_BYTES`）；bash `tokio::process` + git-bash 探测 / `cmd /C` 兜底 / unix `sh -c`；`kill_on_drop` + `taskkill /T /F` 形状清理；超时活性护栏（缺省 120s、钳位 [1s, 600s]） |
| `crates/infra/agent/src/sdk/policy.rs` | 三档决策表（bash 分化） | bash：Default 拒、AcceptEdits / Bypass 放；AcceptEdits ≠ Bypass 首次真实分化锁定测试 |
| `crates/infra/agent/src/sdk/{context,compact}.rs` | L2 / L3 防线 | 窗长 = provider `context_length` 缺省 128K 启发式；字节启发式 token 估算（bytes ÷ 4）；prune 占位符 + 轮对裁剪 + 保首条 user；LLM compaction（失败降级硬裁，run 不中断）；`context_pruned` / `context_compacted` SystemNotice；调用点 = resume 重建后 + 每请求前；store 转录全量不变 |
| `crates/core/agent/**`、`crates/infra/agent/src/cli/**`、`compose.rs` 的 `SessionInjections` 面 | 本变更零 diff 面 | core 契约 / CLI 引擎不动；`SessionInjections` 恒传 default 不动（compose 仅增 `context_length` 接线行） |
| `crates/infra/store` | 会话转录唯一家 + 查询面实现 | `SessionRecord` + 转录单表（session 键）+ 轮统计行；write-through 原子操作面；会话查询/对账 API；详见 desktop-workspace-store |
| `src/commands/exec/agent.rs` | 内核薄包装命令面 | 三件事纪律；编排退役下沉；会话域查询/终止命令；`Result<T, String>` 模板；命名与参数面 design 定稿 |
| 引擎门面 + 内核组合根 | 解析单点（自命令层下沉） | `runner_for` / `EngineConfig` 零 diff；默认 agent 解析语义不变（desktop-agent-management） |
| `packages/desktop/src/lib/agent-adapter.ts` | 事件适配双层 | delta → provisional（correlator）、密封替换；`eventsToUIMessages` 仅消费密封转录；详见 desktop-agent-chat-infra |
| `packages/desktop/src/lib/agent-transport.ts` + `src/hooks/use-agent-chat.ts` | 会话域传输与状态基建 | 会话域 invoke/流参数；重放装载走会话转录；详见 desktop-agent-chat-infra |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 调试页参数面 | agent 选择器（默认选中默认 agent，仅调试页暴露）；时间线 / 落库 / 重放组件零改动复用 |
| `packages/desktop/src/views/agent/components/agent-run-form.tsx` | run 表单 | agent 选择 state（选中 id 随会话发起 invoke 传参；`engine` state 与下拉退役） |
| `packages/desktop/src/components/app-sidebar.tsx` | 页面导航组 | [变更] [Agent 调试]；本地 state 切视图，无路由 |
| agent 会话域前端 hooks | 流订阅 + 查询 | Tauri Channel 实时订阅（执行流通道例外）+ invoke 会话查询重放；查询仍显式触发 |
| `packages/desktop/src-tauri/Cargo.toml`（workspace 依赖） | rig facade 切换 | `rig` 条目：`=0.42.0` 锁版、`default-features = false`、features `reqwest` + `native-tls` + memory；`rig-core` 条目保留为解析钉子（双条目强制锁 0.42.0）；`regex` 条目；members 行与目录名不变 |
| `packages/desktop/src-tauri/src/bindings/` | IPC 类型镜像 | 经 export-bindings 再生成（信封与命令面演进），非手改；快照测试同步再生成 |
| `crates/core/agent/src/port.rs` `SessionQuery` | 单查契约 | 增按 id 单查方法（会话行 + 统计 + 轮行，running 自轮行推导）；`list_sessions` / `transcript` 不动 |
| `crates/infra/agent/src/store_port.rs` | 单查实现 | 以 store `find_session` 为底座（store schema 零变更）；workspace 库隔离 |
| `src/commands/exec/mod.rs` | `session_detail` 命令 | 薄包装三件事纪律；不存在 id → `Err`；blank root → 空结果；builder 注册 + bindings 再生成 |
| `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 来源筛选参数化 | 去硬编码 `'debug'`；三态筛选重查；显式刷新模式不变 |
| Agent 调试页历史区组件 | 筛选 UI | debug / change / 全部，默认 debug；data-testid 挂钩，不以样式类名查询 |
