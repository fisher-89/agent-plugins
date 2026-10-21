# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 发起命令参数面增位 + chat 清单命令 | `agent_start` / `agent_start_with` 尾部 `cwd: Option<String>`（位置索引 8，`None` → `root`）；`cwd` 只赋 `SessionCtx.workspace_root`，`stores.for_root(&root)` 恒不动（双 root 不变量）；`cwd` 非空做 `is_dir` 探测并以显式 `Err`（携路径）拒绝；`source_ref` 可携来源引用（`None` 保持既有缺省）；新增 `list_change_chat_sessions` 薄包装（`source = "change"` 常量 + `source_ref = change id` → `SessionPage`，blank root → 空页） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 命令单一登记面 | `all_commands!` 增 `list_change_chat_sessions`（builder 注册与 bindings 再生成随之） |
| `src/commands/exec/mod_test.rs` | 命令面直测 | `cwd` 三态（缺省回落 / 有效值进 `SessionCtx` / 非目录显式 `Err`）+ store 半边恒 root（断言会话行落 workspace 库）；chat 清单分页与来源不撞 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 绑定形状守卫 | `agentStart` 逐字签名断言以尾部 `cwd` 收尾；新命令在清单断言内 |
| `crates/infra/agent/src/{worker.rs,compose.rs,sdk/}` | 零改动（cwd 消费面） | `worker.rs` 已以 `turn.root` 赋 `SessionCtx.workspace_root`；沙箱根 / bash `current_dir` / `AGENT.md` 读取点均随该字段，注入装配仍只特判 `source == "explore"`（change 来源恒 default 七工具） |
| `crates/core/agent` / `crates/infra/store` | 零改动 | 会话内核契约、`SessionProvenance` 形状、store schema 与会话清单过滤语义（`source_ref` 精确相等）均不动 |

### API: `packages/desktop/src-tauri/src/commands/exec/mod.rs`（命令面随动）

| 命令 | 签名 | 描述 |
|------|------|------|
| `agent_start`（参数面增位） | `(on_event: Channel<AgentRunMessage>, root: String, prompt: String, permission_mode: AgentPermissionMode, session_id: Option<String>, source: Option<String>, source_ref: Option<String>, agent: Option<i64>, cwd: Option<String>) -> Result<TurnSummary, String>` | 提前 resolve 与发起流出口不变；`cwd` 只进 `SessionCtx.workspace_root`，store 恒 `for_root(root)`；`cwd` 非空先 `is_dir` 校验（显式 `Err`） |
| `list_change_chat_sessions`（新） | `(root: String, id: String, offset: u32, limit: u32) -> Result<SessionPage, String>` | 薄包装（参数转换 → `session_query` 分页 → 错误映射）：`source` 常量 `"change"` + `source_ref = id`；blank root → 空页信封 |
