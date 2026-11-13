# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

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


### Requirement: 引擎泵一轮一命与 headless 无问收口

`agent-runtime` 的 `sdk/` 引擎会话泵任务（`session_pump`）SHALL 服务**恰一个提问**：轮终态产出后泵任务 SHALL 返回（观察通道发送端随之释放 → 内核观测通道关闭）。泵 MUST NOT 回到提问接收等待下一轮（多问循环 MUST NOT 残留）；spawn 失败与停止路径的既有单轮终止语义 SHALL 保持不变。会话史 SHALL 不依赖泵内跨轮回灌（SDK 泵的跨轮 `history_slot` 随一轮一命移除；会话史唯一来源仍是 store 全史转录重建）。

`core/agent` 的 `drive` SHALL 保持「观测通道关闭（引擎侧单边 EOF）→ converge」为**唯一收口缝**，MUST NOT 引入状态机 break、stop select 或第二收口路径；内核测试假 runner 的引擎侧单边 EOF（`close_stream`）即该契约的测试面，真实泵 SHALL 履行同一契约。

消费纪律：任何消费方对同一逻辑会话的每一轮 SHALL 经 `compose_turn → begin → open_session` 新开引擎会话（既有消费现实：chat Continue 与 walker 每步均如此）；泵返回后同一引擎会话通道上的提问送达 SHALL 走内核 ask 失败臂（failed 记因「轮提问送达失败（引擎侧已终止）」），MUST NOT 复活常驻泵。

headless 交互面（本变更修订）：**ask 为阻塞工具，不消耗整轮**——模型需用户决策时 SHALL 调用 `AskUserQuestion`（工具在 turn 内阻塞、应答经会话级 ask 注册表回流、同一 turn 内继续），MUST NOT 以「问题文本作为本轮最终消息收口 + 应答后 Continue 重开一整轮」作为交互实现；`ask` 位为 `None` 的会话不挂该工具（显式禁用），此时原「问题文本收口 + Continue 续轮」路径仍是该会话的可用退路，但 MUST NOT 被当作 ask 的实现。一轮一命语义不受影响：ask 阻塞发生在轮内工具执行段，不新开轮、不复活泵。

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
- **THEN** 组装面无 disallowedTools 概念（`--disallowedTools` 末位组装规则随 CLI 租户退役，历史 cli 会话转录不受影响）；工具面不再是「封闭清单」——`AskUserQuestion` 按 `ask` 位下发（见「ask 工具协议与阻塞应答通道」），`RunStarted.tools` 如实上报

#### Scenario: ask 阻塞不改一轮一命

- **WHEN** 一轮内模型调用 `AskUserQuestion`，用户应答后模型继续并在收口条件满足时结束
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
4. **交互工具边界（ask-tool-in-flow 修订，原「无 ask 类工具」限制已偿还）**：`AskUserQuestion` SHALL 为基线阻塞工具（`ToolTier::Protocol`，`Generic` / `Flow` 两档执行体，见「ask 工具协议与阻塞应答通道」），初始 `Generic` / `Flow` 之外的禁用场景仅以 `SessionInjections.ask = None` 表达且**首期不接线**（无禁用场景）；**已知限制两条**：① 一次 ask 单问题（`question` + 一组 `options`），多问需多次调用，`options` 必填非空（用户可用自由文本作答，但模型必须给选项）；② ask 阻塞期间无超时自动应答、无「稍后作答」的持久化（回答通道活在进程内，应用重启即失，`AskRegistry` 槽由轮收敛除名）——停等态因此仍不落库；`questionnaire` 组件接线为二期。
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
- **THEN** 「工具面为封闭清单，无 ask 类工具」一类的禁令表述不再出现；`AskUserQuestion` 在挂载清单与 `RunStarted.tools` 中可被观察到


## ADDED Requirements

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

- **WHEN** 模型正阻塞在 `AskUserQuestion`（`AskRegistry` 有该会话的等待方，且 `SteerRegistry` 条目仍在场），用户注入补充说明
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

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

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
