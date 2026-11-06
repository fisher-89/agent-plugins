# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | 信封与盖戳入口 | `AgentEvent` 增 `turn_id: Option<i64>`（`#[serde(default)]`，无 `skip_serializing_if`，线格式顶层 `turnId`）；`AgentEvent::stamp(turn_id: Option<i64>, seq: u64, kind: AgentEventKind)` 首参承载轮归属——唯一生产入口，漏写即编译错误；文档显式区分「轮 id」与 `TurnDone.session_id`（引擎侧标识） |
| `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 盖戳治理单点 | `drive` 观察泵内 `AgentEvent::stamp(Some(turn_id), seq, kind)`（`turn_id` 取自 `RunningTurn` 解构绑定）；收敛 / 落库 / 注册表发布零改动；两实时出口同源同值 |
| `packages/desktop/src-tauri/crates/core/agent/src/event_test.rs`、`kernel_test.rs` | 契约直测 | 线格式 `turnId` 键恒在（`None` → `null`）/ 旧 JSON 缺键读出 `None` / 往返保真 / 同轮全事件同值 / 续轮不串轮 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/**` | 零改动（轮归属不进引擎） | `LoopTurn` / `SessionOpen` / `PumpPayload` / 观察通道载荷（`AgentEventKind`）签名与语义逐条不变；`turn_id` / `turnId` 零字样 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 出线形状守卫 | `agent_event出线为kind判别联合且seq摊平变体字面量camel_case` 追加 `turnId?: number \| null` 与 `turnId` 摊平断言；既有 `seq` / `timestampMs` / 六变体字面量断言零改动 |

### Function: `packages/desktop/src-tauri/crates/core/agent/src/event.rs`

| 项 | 签名 / 形态 | 描述 |
|------|------|------|
| `AgentEvent.turn_id` | `Option<i64>`（`#[serde(default)]`，线格式顶层 `turnId`，键恒在、值可 `null`） | 轮归属：持久层权威分配的轮 id；一批事件的轮内恒定；与 `TurnDone.session_id`（引擎侧标识）语义不同 |
| `AgentEvent::stamp` | `(turn_id: Option<i64>, seq: u64, kind: AgentEventKind) -> AgentEvent` | 唯一盖戳生产入口（首参即轮归属）；签名扩参使漏写轮归属成为编译错误；`timestamp_ms` 仍取 `now_millis()` |

### Function: `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|------|
| `RunningTurn::drive` | 盖戳调用改 `AgentEvent::stamp(Some(turn_id), seq, kind)` | `turn_id` 取自 `self` 解构绑定，零新增字段、零新增参数；delta 与密封共用同一盖戳点，两个实时出口（`streams.publish` / `on_output`）投递同一值 |

### API / Component

无。本变更零命令面演进、零组件、零可见 UI 面（出线只是既有 `AgentEvent` 的多一个可空键）。
