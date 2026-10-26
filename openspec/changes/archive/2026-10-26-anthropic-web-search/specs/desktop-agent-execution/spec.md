# desktop-agent-execution Specification (Delta)

## ADDED Requirements

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

### Requirement: SDK 租户 MVP 边界与已知限制

以下边界 SHALL 作为 SDK 租户 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 MCP**：SDK 引擎不接 MCP 服务器（`RunStarted.mcp_servers` 恒空）
2. **无子代理**：不派生子代理，`parent_tool_use_id` 恒空
3. **无交互审批**：静态三档起步，无审批反向通道（协议留独立 change）
4. **bash 无沙箱（sdk-engine-gap-closure 偿还「bash 缺席」后的知情边界）**：bash 已进工具面（Default 档拒绝、AcceptEdits / Bypass 放行），但授权/沙箱/安全机制不进本期——BypassPermissions 默认档下 bash 即模型可执行任意命令、护栏为零；命令级白/黑名单、真沙箱、交互审批留后续 change
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

#### Scenario: 兜底边界的失败面可见

- **WHEN** anthropic 协议下对「名单外 + 8K 输出上限」模型留空 `max_tokens`（取 32_000 兜底）发起运行
- **THEN** 请求以 provider 400 失败并经 `api_error` 走既有错误收敛（显式、可读诊断），不静默；用户填 `max_tokens = 8192` 后可跑通——该组合作为兜底策略边界的留痕证据

#### Scenario: 相邻 assistant 消息不设实现期核验门

- **WHEN** 审查实现期任务清单与验收面约定（边界条 16 的落点）
- **THEN** 无任何「真机 `pause_turn` 前置核验」的实现期门项或阻塞项；该依赖仅以已知限制留痕，确认动作落于**归档后人工验收**（官方端点 + 凭据，结论记入验收记录），本期无「合并相邻 assistant 消息」的实现分支

#### Scenario: 续跑过程仅 notice 可见

- **WHEN** 一轮较长的搜索触发多次 `pause_turn` 续跑
- **THEN** 实时面只见 `pause_turn_continued` / `pause_turn_capped` 两类 `SystemNotice`（无搜索相关 delta、无「正在搜索…」文案），搜索卡在轮收口后呈查询词与来源列表

## Module Contract

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
| 事件转录模型 | 零变更 | `AgentEvent` 新增块变体为自描述 JSON，无需版本演进 |
