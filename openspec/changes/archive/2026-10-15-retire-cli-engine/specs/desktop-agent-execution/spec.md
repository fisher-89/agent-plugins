# desktop-agent-execution（delta：CLI 租户退役）

## MODIFIED Requirements

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
- **查询会话**：`query_sessions(...)` 形状的会话记录查询——会话清单与聚合统计（提问轮数、累计墙钟、累计 token 消耗；MVP 从轮统计行现算，MUST NOT 维护累计列，字段面可扩展）。

session SHALL 为一等公民：会话 id 由 core 铸造；转录 = 会话全史（密封事件直挂 session，单表），resume 重建源 SHALL 为会话全史，MUST NOT 以单 run 事件拼凑转录。core SHALL 维护双 id 映射（core session id ↔ engine_handle）；engine_handle 经协议到达（spawn 响应或事件上报，见「P1 引擎协议不变量与缺省降级」）。双 id 映射 SHALL 定位为**留痕**（「这轮落在哪个引擎侧会话」的引擎中立观测/考古事实，落库半边保留）；续会话寻址 SHALL 以会话 id + 全史转录重建为准，MUST NOT 以引擎句柄缺失阻断续会话（寻址硬门槛随 CLI 租户退役——`begin()` 不再对缺 `engine_session_id` 的会话报「无法续接」）。会话 SHALL 存引擎/模型/权限档/实例外键的**配置快照**而非跨库引用（`AgentInstanceRecord` 在全局库、session 在 workspace 库，禁跨库引用；实例改名/删除不伤历史会话；`agent_instance_id` 为快照复制值的显式关联键，见 desktop-workspace-store「SessionRecord 模型与会话转录单表」）。会话 SHALL 承载来源归属（`SessionProvenance{source, source_ref}`，作 provenance/持久化记录用），但前端寻址与重放 SHALL 以 **`sessionId` 直查**（`session_detail`）为准，MUST NOT 经 `source` / `source_ref` 反查会话（`sourceRef` 反查取最近一条的路径退役；来源侧——如 explore 记录——持会话 id 并直接给出）。

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
- **THEN** core 记录 core session id ↔ engine_handle 双 id 映射（留痕落库）；续轮与恢复以会话 id + 全史转录寻址，映射缺失不阻断续会话

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

`agent-runtime` SHALL 作为 agent 引擎门面：对外一套应用协议（invoke 面 + 事件流 + 会话记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎（值域保留为多租户协议预留，`Cli` 为退役态）；协议统一 SHALL 落在内核协议层而非各引擎私有形态。门面 SHALL 提供 `runner_for(kind, engine_cfg)` 形状的构造入口——`EngineConfig` 结构体形态（api_key / base_url / model 三件套）由组合根注入，SDK 引擎用以组装 rig client；`EngineKind::Cli` 臂 SHALL 退役为显式 `Err("CLI 租户已下线")` 形态（runner 构造失败面，具体形态——`Result` 包装或等价——由 design 定稿），**唯一 match 点结构与 `EngineKind` 双值 MUST NOT 塌**（多租户机制保留，未来本机 CLI / 远程 API 租户仍落协议预留）。**解析单点自命令层下沉至内核组合根**（desktop-agent-management「默认 agent 与运行发起解析」的落点平移，解析语义不变）：缺省路径解析默认 agent、显式路径按所选 agent 解析、产物为 `EngineKind` + `EngineConfig`（sdk 时由引用 provider 组装、model 取 provider 三档 high 档）；解析遇 `engine: Cli` 实例（缺省与显式路径同）SHALL 显式 `Err("CLI 租户已下线")`，MUST NOT 静默回退。换解析落点时 `EngineConfig` 消费面 MUST NOT 变化。启动失败抹平：`AgentStartError` 的中性变体 `ConfigMissing` 承接 SDK 侧连接配置不齐等启动失败——core 唯一触碰点，变体集 MUST NOT 再增。

#### Scenario: 门面构造与 core 零污染

- **WHEN** 审查 `runner_for` 实现与 `crates/core/agent` 源码
- **THEN** 引擎分发唯一 match、返回引擎协议实现；core 无 engine / `EngineConfig` / rig / agent 概念，`AgentStartError` 变体集不变

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现与门面签名
- **THEN** 解析语义（默认 agent / 显式 agent / sdk provider 组装 / high 档 / 无默认 `Err` / cli 实例 `Err`）与设计一致，`runner_for` 唯一 match 点结构与 `EngineConfig` 结构体零塌，命令层无引擎解析残留

#### Scenario: cli 解析显式失败

- **WHEN** 解析目标实例 `engine` 为 `cli`（缺省默认实例或显式选择的存量 cli 实例）
- **THEN** 返回 `Err("CLI 租户已下线")`（文案引导管理页），不构造任何引擎、不落库不推流

### Requirement: SDK 引擎 bash 工具

SDK 引擎工具面 SHALL 增第七工具 `bash`，`RunStarted.tools` SHALL 报七工具：

- **档位归属**：Default 档拒绝、AcceptEdits / BypassPermissions 档放行（见「SDK 引擎静态权限与路径沙箱」的三档分化）；拒绝形态与 write/edit 同构（`SystemNotice{subtype:"permission_denied"}` + is_error=true ToolResult，run 不中断）
- **进程执行**：`tokio::process`，cwd = workspace root，环境继承当前进程；shell 底座 Windows SHALL 优先探测 git-bash（unix 语法对模型成功率最高），探测失败退 `cmd /C` 兜底（不损开箱即跑）；unix 侧 `sh -c`
- **停止语义**：`kill_on_drop(true)` + Windows 进程树清理沿 `taskkill /T /F` 形状（原 `cli/runner.rs` `kill_process_tree` 同形，形状随 retire-cli-engine 迁驻 `sdk/bash.rs` 唯一持有；future drop 不自动杀子进程树）
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

### Requirement: agent 执行命令面

`commands/exec/` SHALL 承载执行与查询命令，命令体 SHALL 收敛为内核公共 API 的薄包装（三件事纪律：参数转换 → 调用 → 错误映射）：

- 发起命令（现 `agent_start`）：参数转换后调用内核 `run_session` 形状 API——`New` 建立会话、`Continue{id}` 续轮（现 `resume_session_id` 语义平移）；编排（runner 组装、tee、落库、盖戳、收敛）SHALL 全部下沉内核，命令体 MUST NOT 编排。来源参数（`source` / `source_ref`）SHALL 写入会话归属（provenance 记录用）；`source` 不传时缺省 `debug`，`source_ref` 前端恒传 null（会话寻址改走 `sessionId` 直查，不再经来源反查）。cwd 恒为当前 workspace root（root 解析所属 workspace 库保留）；引擎组装 SHALL 经内核组合根（见「引擎门面与 EngineKind 参数选择」），命令体 MUST NOT 直接构造具体引擎。命令命名与参数面演进由 design 定稿（保留语义升级或更名会话域命令二选一，见提案待决问题）
- 查询命令（现 `agent_runs` / `agent_run_events` / `agent_run_chain`）：演进为会话域查询薄包装（会话清单含聚合统计 / 会话转录重放 / 源过滤清单），参数含 root → workspace 库解析 → store/内核查询 → DTO；前端寻址与重放 SHALL 走 `session_detail` 单查（见「session_detail 单查面」）而非按来源反查取最近一条；旧 run 域命令的退役/收窄路径由 design 定稿
- 终止命令（现 `agent_stop`）：见「agent_stop 终止与提前 resolve」；root 寻址保留

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

#### Scenario: 错误 reject 传达

- **WHEN** SDK 引擎配置缺失、解析目标为存量 cli 实例（CLI 租户已下线）、无默认 agent 可解析或所属 workspace 库打开失败时发起运行
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

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

## REMOVED Requirements

### Requirement: Claude CLI 租户（MVP）

**Reason**: CLI 租户（本机 Claude Code CLI 引擎）实现随 retire-cli-engine 整体退役——SDK 引擎（rig 进程内租户）已覆盖全部 CLI 独占语义（resume 走 store 全史转录重建、headless 无问收口走封闭工具面、model 双档、`engine_session_id` 仅留痕），`cli/` 模块（discover / flags / jsonl / runner，2251 行）删除；多租户机制（`AgentRunner` trait / `EngineKind` 参数面 / `runner_for` 唯一 match 点）与 `AgentEngineKind` 解码锚保留，非本 requirement 承载。

**Migration**: 新发起运行一律经 SDK 租户（默认 agent 解析）；存量 cli 会话历史照常可查可重放、续接显式 `Err("CLI 租户已下线")`；存量 cli 实例可读可删不可改（保存 / 设默认均拒）；`--resume` / `--disallowedTools` 等 CLI flag 组装语义由「引擎泵一轮一命与 headless 无问收口」「SDK 引擎 resume（store 即会话）」承接，无功能欠账。

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/` | **（退役删除）** | CLI 租户实现整目录删除（discover / flags / jsonl / runner + 各 `_test`）；`kill_process_tree` 的 `taskkill /T /F` 形状迁驻 `sdk/bash.rs` 唯一持有 |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 引擎门面（收窄） | `EngineKind` 双值保留（`Cli` 退役态）；`runner_for` 唯一 match 点结构不塌，`EngineKind::Cli` 臂显式 `Err("CLI 租户已下线")`（形态 design 定稿）；`ClaudeCliRunner` / `pub use cli::…` re-export 删除 |
| `crates/infra/agent/src/compose.rs` | 解析单点 + 快照组装 | `resolve_agent_engine` 的 `AgentEngineKind::Cli` 臂显式 `Err`（缺省/显式路径同）；`begin()` 引擎句柄寻址硬门槛退役（`prior_handle` Option 透传）；`ResolvedEngine` 携 `agent_instance_id` 入快照；跨引擎校验不动 |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现 | `is_cli_engine()` discover 轮前预检分支删除（`ConfigMissing` 归 `begin()` open 段） |
| `crates/core/agent/**` | **（零 diff 面）** | `AgentEvent` 信封 / `AgentRunner` / run 状态机 / `AgentStartError` 变体集（`CliMissing` 保留为协议预留词汇）/ `TurnRequest.prior_handle` 均不动 |
| `crates/infra/store/src/model.rs` | 快照字段面 | `SessionConfigSnapshot` 增 `agent_instance_id: Option<i64>`；`SessionRecord` v1→v2 `from` 链；`AgentEngineKind` 变体集与序号零改动（解码锚） |
