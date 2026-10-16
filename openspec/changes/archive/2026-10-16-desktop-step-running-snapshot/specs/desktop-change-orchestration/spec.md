# desktop-change-orchestration Specification (Delta)

## ADDED Requirements

### Requirement: 步级时间戳与 token 消耗随行

步状态行 `ChangeStepState` SHALL 增四字段并全链随行：`started_at: i64`（UTC unix 毫秒，域内 `now_millis()` 口径）、`ended_at: Option<i64>`（running 恒 None）、`token_input: Option<u64>` / `token_output: Option<u64>`（本步 turn 增量，仅 WorkerAgent 步终态有值，缺席合法缺省）。`StepSettle` SHALL 同步扩四字段（emit 载荷收窄面）。

**时间铸造 SHALL 收 `emit_node` 单点**（复用既有 `now_millis()`，MUST NOT 新增时钟注入缝——收口时刻铸造只在生产路径消费，corpus 确定性由测试直构 `ChangeStepState` 注入定值承载，沿 `walker.rs` `now_millis` 既有注释口径）：

- running emit：`started_at = now`、`ended_at = None`、token 两值 None；
- 终态 emit（passed / failed / stopped）：`ended_at = now`，随行 `started_at = now`（该值是否被采信由合并侧裁定）——`phaseStart` 等单 emit 瞬步由此自然得 `started_at == ended_at` 同刻。

**合并语义（`accumulate_node`）SHALL 为**：`started_at` 无条件保首见（running emit 落值后不被终态 emit 顶替，终态行携带的同刻值被丢弃；瞬步单 emit 首见即真值）；`ended_at` / `token_input` / `token_output` 与 `session_id` 同款「some 才覆写」。节点累积器（`RunEntry.nodes`）、活面快照（`ChangeRunSnapshot` / `ActiveRunView`）与收口整包（`finish_command` → `RunNodeEntry`）SHALL 逐字段同值透传，零逻辑分支。

**token 求和 SHALL 在 `run_worker` 终态 emit 前单次落值**：对 `outcome.transcript` 的 `AgentEventKind::TurnDone.usage` 求 in / out 两键之和（`worker.run(turn).await` 收口后 transcript 已含 TurnDone——终态单次落值即会话结束后累计结果，MUST NOT 建二次刷新通道）；usage 缺席（引擎未返回）= None；工具步 / Gate 步 / Git 步恒 None。token 求和函数 SHALL 唯一实现于 core/agent（`AgentEventKind::TurnDone` 定义所在 crate，orchestration 已依赖；agent crate 禁 workspace 内依赖而函数仅吃 `&[AgentEvent]` 天然合规），infra/store 的会话统计 SHALL 改引同一实现（依赖方向 infra → core 不可反向，MUST NOT 保留双实现）。`RunUpdate::Step` 载荷随 `ChangeStepState` 自动扩展，IPC 通知 kind-only 降位形态（载荷不出进程、客户端收通知重查统一视图）MUST NOT 改变。

#### Scenario: 双 emit 起止落值与首见保留

- **WHEN** 某 WorkerAgent 步经 running emit（时刻 T0）后于时刻 T1 以 passed 收口（终态 emit）
- **THEN** 合并后节点 `started_at == T0`（终态行携带的 T1 起始值被丢弃）、`ended_at == T1`；活面快照与收口整包同值

#### Scenario: 瞬步同刻

- **WHEN** `phaseStart` 等单 emit 直 passed 的瞬步收口（时刻 T）
- **THEN** 节点 `started_at == ended_at == T`（耗时 0ms），无 running 先行 emit 要求、无特判分支

#### Scenario: token 本步增量落值

- **WHEN** `run_worker` 收口的 `outcome.transcript` 含两条 TurnDone（usage 各 `input_tokens` / `output_tokens`）
- **THEN** 终态节点 `token_input` / `token_output` = 本步 transcript 内 TurnDone usage 各键求和；usage 全缺席的会话两值为 None；同 run 的 static-check 工具步节点两值恒 None

#### Scenario: 停止与崩溃面

- **WHEN** 用户停止 run 使在飞 WorkerAgent 步以 stopped 收口（或进程被杀、终态 emit 未达）
- **THEN** stopped 路径同有 `ended_at`；被杀路径节点留 running 形态（`ended_at` 恒 None），零补值逻辑

#### Scenario: 求和函数单源

- **WHEN** 审查 orchestration 与 infra/store 对 TurnDone usage 求和的实现触点
- **THEN** 唯一实现居 core/agent，两消费方同源引用；store 会话统计输出与上移前逐字段一致；orchestration 依赖面零 infra 新增

#### Scenario: 通知面零影响

- **WHEN** 运行中步状态变化触发 Channel 通知
- **THEN** IPC 通知仍为 kind-only（载荷不出进程），客户端经统一视图重查取到含四字段的节点行；通知契约零变化

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/state.rs`（`ChangeStepState`） | 活面契约单点 | 增 `started_at: i64` / `ended_at: Option<i64>` / `token_input: Option<u64>` / `token_output: Option<u64>`；doc 注释携口径与合并语义指针 |
| `crates/core/orchestration/src/walker.rs`（`StepSettle` + `emit_node` + `run_worker`） | 铸造与 token 落值 | `StepSettle` 扩四字段；`emit_node` 按 status 铸造单点（零新时钟缝）；`run_worker` 终态 emit 前 transcript TurnDone.usage 求和（in / out） |
| `crates/core/orchestration/src/control.rs`（`accumulate_node`） | 合并语义 | `started_at` 无条件保首见；`ended_at` / `token_input` / `token_output` some 才覆写（与 `session_id` 同款） |
| `crates/core/orchestration/src/run_history.rs`（`finish_command`） | 收口整包透传 | 四字段直复制进 `RunNodeEntry`（零过滤零改写） |
| `crates/core/agent/src/`（`sum_usage_tokens` 上移落位，design 定稿） | token 求和单源 | `pub fn`（`(&[AgentEvent], &str) -> Option<u64>` 同签名）；orchestration / infra/store 两消费方同源引用 |
| `crates/infra/store/src/store.rs`（会话统计） | 消费方改引 | 删除私有 `sum_usage_tokens`，改引 core/agent 实现；聚合行为零变化 |
