# desktop-agent-execution Specification（delta: provider-effort-knob）

## Purpose

本 delta 让 sdk 引擎消费 provider 的 `efforts{high, low}`：档位 `match`（`ModelLevel` 唯一分发点）产出 `(模型, effort)` 二元组，经**纯函数单点** `effort::params(protocol, effort)` shaping 成 wire 载荷后落到 `CompletionRequest.additional_params`。三条纪律先说清：①**留空零痕迹**（两档皆空 ⇒ `additional_params` 保持 `None`，请求体与今天逐字相同）；②**协议判定单点**（shaping 只在 `sdk/effort.rs`，`loop` / 门面 / 组合根一律持「已 shaping 的载荷」，与 `web_search` 的「已门控最终值」同构）；③**会话内 effort 恒定**（档位在轮边界确定，故 anthropic 自动缓存的「top-level effort 恒定」由结构保证）。摘要请求不带 effort、事件词汇零扩展、`EngineConfig` 连接配置字段面不塌。

## ADDED Requirements

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

## MODIFIED Requirements

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

## Module Contract

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/effort.rs`（新）

| 项 | 签名 / 形态 | 描述 |
|------|------|----------|
| 常量面 | `OPENAI_EFFORT_KEY = "reasoning_effort"` / `ANTHROPIC_OUTPUT_CONFIG_KEY = "output_config"` / `ANTHROPIC_EFFORT_KEY = "effort"` | 键名字面量单点（`loop` / `runner` / 门面 / 组合根零重复） |
| `params` | `fn(protocol: Protocol, effort: &str) -> Option<serde_json::Value>` | 纯函数：裁剪后空 ⇒ `None`；`Openai` → `{reasoning_effort: v}`；`Anthropic` → `{output_config: {effort: v}}`；零值域校验、零 IO |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `SdkRunner.efforts` | `AgentEffortTiers`（新字段） | provider 原值对（未 shaping） |
| `SdkRunner::with_efforts` | `fn(self, efforts: AgentEffortTiers) -> Self` | 旁路 builder（`new` 签名不动，既有调用点零改） |
| `SdkRunner::open_session` | 档位 `match` 扩为 `(model_field, model_name, effort)` 三元组 | 唯一分发点：`High → (model_high, efforts.high)` / `Low → (model_low, efforts.low)`；`effort::params(protocol, effort)` 算一次 shaping |
| `PumpPayload` / `session_pump` | 增 `effort_params: Option<serde_json::Value>` | 构造 `LoopTurn` 时传递；启动日志加 `effort=…`（留空 `-`） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `LoopTurn` | 增 `effort_params: Option<serde_json::Value>` | 已按协议 shaping 的 wire 载荷（loop 零协议判定、零字面量） |
| 请求构造 | `additional_params: turn.effort_params.clone()` | 与既有 `request.provider_tool(..)` 在同一个 `additional_params` 上按键合并（rig `merge_params`），两键互不覆盖 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/{lib,compose}.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `EngineFacade.efforts` + `with_efforts` | `fn(self, AgentEffortTiers) -> Self` | 与 `with_context_window` / `with_max_tokens` / `with_web_search` 同型旁路缝（四个构造点补齐字段） |
| `compose::ResolvedEngine` | 增 `efforts: AgentEffortTiers` | provider 原样带出（**零门控**——两协议都有 effort 语义） |
| `compose::runner_for` 链 | 加 `.with_efforts(..)` | 旁路载荷下传 sdk 臂；`runner_for` 签名与唯一 match 点零塌 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs`（零改动面）

| 项 | 变化 | 描述 |
|------|------|------|
| `EngineConfig` | **零改动** | 连接配置字段面不增字段；`efforts` MUST NOT 出现在本结构（走门面旁路） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/compact.rs`（零改动面）

| 项 | 变化 | 描述 |
|------|------|----------|
| `summarize` | **零改动** | 摘要请求 `additional_params: None` 逐字保留（不带 effort、不带 provider tool）；该文件出现 diff 即说明偏离本 delta |

### API: `packages/desktop/src-tauri/crates/infra/store`（消费面）

| 项 | 变化 | 描述 |
|------|------|----------|
| `AgentEffortTiers` / `AgentProviderRecord.efforts` | 新类型 + v5 新列 | 见 desktop-agent-management Module Contract；本能力只读消费（原样带出，零校验） |
