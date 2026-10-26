# 提案: anthropic-web-search

> **变更**: anthropic-web-search
> **日期**: 2026-10-23
> **状态**: 待评估（explore 决策表 ①–⑫ 已全部锁定，无待确认项；本提案对两处实现期遗留作裁定，见「风险 R6」「风险 R10」）
> **来源**: `explore.md`（requirement 类型；rig 0.43.0 源码级复核证据见其附录 A）
> **回溯修订**: 2026-10-23（三处自足化：①决策表补「被拒备选与理由」列并内联 ②/⑤ 否决理由，不依赖 explore 附录 B 即可逐条成证；②R5 的「spec 留痕」落实为 desktop-agent-execution 边界条 16；③搜索块 `output` 形状统一为 `AgentToolOutput` 加位形态，与 desktop-agent-chat-infra spec 对齐）
> **回溯修订（第二轮）**: 2026-10-23 —— **撤销 R5 的「真机核验前置到实现期」强制门**：本期无官方 Anthropic 凭据，该门无法在实现期履行，其「SHALL 前置核验、MUST NOT 留到发布前」语气不应留在下游任务清单里变成阻塞项。改为**已知限制留痕 + 归档后统一人工验收**（见「风险 R5」与「验收面口径」）；同步改 `specs/desktop-agent-execution` 边界条 16 及其新增 scenario。**代码与实现形态零变化**（D1–D18 不涉改）——下游仅需删 `tasks.md` 的前置核验门两行并同步 design `D17` / 待决问题 5 表述

---

## 问题

桌面端 SDK 引擎（`packages/desktop/src-tauri/crates/infra/agent`）目前**只向模型下发七个客户端工具**，Anthropic 协议下没有任何 provider 服务端工具——用户接上官方 Anthropic 端点后，模型**物理上无法联网**，凡需最新信息的问题只能凭训练数据作答或以「无法获取实时信息」收口。

rig 0.43.0 已备齐所需两半能力（请求侧 `ProviderToolDefinition` → `additional_params.tools` → wire `tools`；响应侧 `server_tool_use` / `web_search_tool_result` 解码为 `Text{text:"", additional_params:{anthropic_content}}`、citations 落 `additional_params["citations"]`、回放时把 raw 放回即可原样重发），**无需升版或打补丁**。缺口全在本仓库，共六处：

1. **无 provider 开关**：`AgentProviderRecord`（v3）字段面没有任何「启用云端工具」位；`EngineConfig` / `EngineFacade` / `SdkRunner` 的旁路载荷只有 `context_length` / `max_tokens` 两项，无第三条缝可承载。用户无法表达「这家 provider 该不该联网」。
2. **事件契约无搜索词汇**：`AgentBlock` 只有 `Text` / `Thinking` / `ToolUse` / `ToolResult` 四变体（`crates/core/agent/src/event.rs:19-38`）。Anthropic 的 server 块经 rig 到达时是**空文本 `Text`**——当前 loop 的内联块转换会把它压成 `AgentBlock::Text{text:""}`，落库后时间线上**什么都看不见**（一次真实搜索在转录里退化为一条空字符串）。
3. **引用无处安放**：citations 在 `additional_params["citations"]` 里随 text 块到达，`AgentBlock::Text` 只有一个 `text` 字段——引用在事件面即被丢弃，前端永不可见。
4. **续轮丢搜索上下文**：`sdk/resume.rs` 的 `assistant_content` 只按 `Text` / `Thinking` / `ToolUse` 三分支重建（`resume.rs:95`）；即便 server 块被保住，重建时也会以空文本重发——**Anthropic 侧认为 user 引用了不存在的 server tool 结果**，续轮要么报错要么失忆。`encrypted_content` 等字段不可推导，必须原样回放。
5. **`pause_turn` 无人接管**：rig 把 `pause_turn` 映射为 `FinishReason::Other("pause_turn")`（`providers/anthropic/completion/tests.rs:2804`），当前 loop 只识别 `Stop` / 工具调用两种收口——一次长搜索以 `pause_turn` 结束时会被当作**本轮自然结束**，搜索被腰斩、后续轮次无据可续。
6. **裁剪会造出回放毒块**：`sdk/context.rs` 的 `sweep_orphans` 只认 `UserContent::ToolResult ↔ CallId` 配对（`context.rs:237-260`）；`pause_turn` 把一次搜索拆成两条 assistant 消息（use 在前、result 在后）后，L2 剪裁 / L3 硬裁 / 摘要置换都可能裁掉 use 而留下 result → 回放被 Anthropic 拒（400）。**这是正确性硬缺口，不是体验优化**。

## 提案

以「**provider 开关 + 事件词汇扩容 + 回放保真 + 续跑接管 + 防线配对扩展 + 时间线呈现**」六面收口，MVP 目标锁定**最终可见**：过程允许空白，实时「正在搜索…」整条不做。

### 决策执行表（explore ①–⑫，本提案照抄执行；末列补齐「被拒备选与理由」，使每条决策在正文内自足成证）

| # | 决策 | 要点 | 被拒备选与理由 |
| --- | --- | --- | --- |
| ① | citations 挂**正文部件** | 复用 text 部件的 `providerMetadata`，不设独立 `data-citations` 部件 | **被拒：独立 `data-citations` 部件**——要新增 `AgentDataParts` 成员，并在折叠路与实时路各写一遍发射逻辑；text 部件的 `providerMetadata` 已能随部件承载任意 JSON（`ProviderMetadata = Record<string, JSONObject>`），新增部件类型是净成本 |
| ② | `web_search: bool` | store v4；`AgentProviderRecord::new` 签名不动 + `with_web_search`；`Text.citations` 用 `Option<Vec<_>>` | **被拒：`new(...)` 增第 9 参**——全仓 35 处调用点机械改动，也违背既有「可空参走构造器链」先例（`with_max_tokens`）。**被拒：`Text.citations: Vec<AgentCitation>` 必填**——specta 生成非可选字段，14 个测试文件 / 34 处 fixture 全要补字段，且旧事件缺该键直接解码失败；`Option` + `#[serde(default)]` 使两者零改动 |
| ③ | 搜索卡文案**无状态** | summary 恒为「网络搜索」；无「正在搜索…」、无超时降级 | **被拒：「正在搜索…」态 + 超时降级文案**——决策 ⑦ 已删除实时 delta，前端没有任何信号可驱动「进行中 → 已落定」迁移，只能靠计时器造一个与真实进度无关的假状态 |
| ④ | citations 命名空间**自有** | `providerMetadata: { agent: { citations } }`（provider 外消费即不借 provider 名） | **被拒：`providerMetadata.anthropic`**——消费点（`CitationList`）在 provider 之外，命名空间绑 provider 名会把协议中立的事件词汇误标为协议私有（`core/agent` 本就不认识 anthropic） |
| ⑤ | 引用用**正文底部列表** | `title ?? url`，按 url 去重；不做角标、不做原句高亮 | **被拒：行内角标**——需把 citation 与正文片段 / offset 对齐，rig 侧 raw citation 不保证可定位到渲染片段，错位即误导。**被拒：原句高亮**——除同源定位问题外还与正文增量渲染相冲（ai-sdk 的 `providerMetadata` 合并语义是**覆盖非累加**，无法随 delta 累加对齐点）。底部去重列表不需任何定位，信息无损且实现最简 |
| ⑥ | **不加** `serverTools` | `RunStarted` 契约不变，工具面板看不见 server tool | **被拒：`RunStarted.serverTools` 字段**——server tool 不由客户端执行、不进权限决策面，加字段要连带扩 `dispatch` 派生面与前端 `data-run-started` 消费面；呈现需求已由搜索卡（决策 ⑧）满足 |
| ⑦ | **删实时 delta** | 无 `AgentDelta::Search`；`normalize.rs` 与 `agent-transport.ts` 零改动 | **被拒：`AgentDelta::Search` + 实时搜索提示**——要同时改 `normalize.rs` 与 `agent-transport.ts`、扩 `AgentDelta` 词汇、再加前端进行态状态机；MVP 只保证最终可见，两文件零改动的收益大于实时提示 |
| ⑧ | 搜索卡承载用 **`tool-web_search` 部件** | 复用 `planToolPart` / `plansToChunks` / `DEFAULT_TOOL_CARDS` 现成管线 | **被拒：新 `data-web-search` 部件**——要新增部件类型 + 两路各写发射 + 渲染分叉；工具卡管线四段（`planBlock` → `planToolPart` → `plansToChunks` → 注册表）已是现成机械位 |
| ⑨ | **空 parts 消息不渲染** | 渲染层 `useHistoryWindow` 过滤 `parts.length > 0` | **被拒：在事件 / adapter 层就不产空消息**——要跨层判断「本轮是否只剩被消费的 result」，把渲染关切上推到引擎或适配层；渲染层一行过滤同时覆盖实时路与重放路，并顺带修正分页计数 |
| ⑩ | 外链用**官方 opener 插件** | `tauri-plugin-opener`，scope 限 `http(s)` | **被拒：自写 `open_external_url` command**——要自管 URL 校验、平台差异与新增 IPC 面；官方插件的 capabilities scope 可声明式收窄到 `http(s)` |
| ⑪ | **`sweep_orphans` 扩展** | 认 assistant 位 server 块配对（正确性硬缺口） | **被拒：保持不变**（早期判断「compact / sweep_orphans 不动」）——`pause_turn` 跨消息 + L2 剪裁 / L3 硬裁 / 摘要置换会裁掉 use 而留下 result → 回放被 Anthropic 拒（400），是正确性硬缺口而非体验项 |
| ⑫ | **`pause_turn` 续跑不计入重试预算** | 续跑独立计数轴；usage 照实累加；到顶正常收敛 | **被拒：续跑共用 `retry.max_attempts` 单轴**——一次 `pause_turn` 后的网络抖动会继承前面积累，搜索几次就把重试预算吃光，表现为「长得像网络故障的搜索失败」 |

> 决策 ② / ⑤ 的否决理由已在此表内联（不依赖 explore 附录 B）；①③④⑥⑦⑧⑨⑩⑪⑫ 同理，每条决策在正文内自带「为什么不是另一个」。

### 六面收口

1. **core 事件契约扩容**（`crates/core/agent/src/event.rs`）：`AgentBlock` 增 `WebSearch { id, query, results, error, raw_use, raw_result }` 变体，`Text` 增 `citations: Option<Vec<AgentCitation>>`（仅 `#[serde(default)]`：键恒在、值可为 `null`；MUST NOT 加 `skip_serializing_if`——unified 相位模型无法表达条件省略，加则 bindings 分裂）；新增 `AgentCitation { url, title, cited_text, raw }` 与 `WebSearchSource { url, title, page_age }`，`lib.rs` 导出。**一次 `server_tool_use` 与其配对 result 收敛为一块**——事件面上「一次搜索」是原子事实，回放与呈现都以此为唯一单元。
2. **引擎能力**（新建 `sdk/web_search.rs`）：常量单点（`WEB_SEARCH_TOOL_KIND = "web_search_20250305"` / `WEB_SEARCH_TOOL_NAME = "web_search"` / `WEB_SEARCH_MAX_USES = 5` / `ANTHROPIC_RAW_CONTENT_KEY` / `CITATIONS_KEY`）+ 六个纯函数（`provider_tool` / `raw_content` / `citations_of` / `project_choice` / `replay_raw` / `replay_text`）。`max_uses = 5` 为成本护栏常量，**不做可配置**（可配置化需 store v5 迁移）。
3. **循环**（`sdk/loop.rs`）：`LoopTurn` 增 `web_search: bool`，请求构造后按需 `.provider_tool(web_search::provider_tool())`；内联块转换替换为 `web_search::project_choice`；`'attempt` 返回 `(choice, usage, finish_reason)`；**新增 `pause_turn` 续跑循环**（两条独立计数轴，见决策 ⑫）。
4. **回放**（`sdk/resume.rs`）：`assistant_content` 改为可展开循环——一条 `WebSearch` 块最多产两条 `AssistantContent`（`raw_use` → `raw_result`，保原序）；`Text` 携 citations 时以 `additional_params.citations = [c.raw, …]` 回填。**展示字段（query / results / error）MUST NOT 参与重建**。
5. **开关链路**：store v4（`web_search: bool`）→ `save_agent_provider` 末位入参 → `compose::ResolvedEngine` → `EngineFacade::with_web_search` → `SdkRunner::with_web_search` → `PumpPayload` → `LoopTurn`。**生效门在解析期**（`provider.web_search && protocol == Anthropic`），保存期不因 openai 拒收（用户切协议后开关值原样保留）。
6. **前端**：`agent-adapter.ts` 的 `BlockPlan.text` 携 `citations`、`BlockPlan.tool` 复用承载搜索（`toolName: 'web_search'`、`input: { query }`、`output` 为 `AgentToolOutput` **加位形态**——`content: ''` + `isError` + `search?: { results, error }`，未配对时为 `null`）；`agent-blocks.tsx` 增 `WebSearchCard` / `CitationList` / `ExternalLink`；`tool-cards.tsx` 注册 `DEFAULT_TOOL_CARDS['web_search']`；`agent-messages.tsx` 过滤空 parts 消息；`provider-panel.tsx` 增协议门控 checkbox。`AgentDataParts` 成员零新增。

### 明确不做（知情边界，非遗漏）

- **实时搜索提示 / `AgentDelta::Search`**（决策 ⑦）：MVP 只保证最终可见；`pause_turn` 期间时间线可能空白数十秒，唯一过程观测点是 `SystemNotice{pause_turn_continued}`。
- **`RunStarted.serverTools`**（决策 ⑥）：工具面板看不见 server tool；`RunStarted` 契约与 `dispatch` 派生面零改。
- **独立 `data-citations` 部件**（决策 ①）：citations 挂 text 部件 `providerMetadata`，`AgentDataParts` 成员零新增。
- **角标 / 原句高亮引用**（决策 ⑤）：只做正文底部去重列表。
- **`max_uses` 可配置**：锁常量 `5`；配置化需 store v5 迁移，本期不做。
- **转录体积上限与压缩**：`raw_result` 含 `encrypted_content` + 每条来源加密索引，一次搜索数 KB，JSON 不压缩、不设上限。
- **openai 协议的服务端工具**：`web_search` 是 Anthropic 专有 server tool，openai 协议下开关解析恒 `false`（不静默透传、不报错）。
- **降级兼容**：旧客户端读到含 `webSearch` 块的事件会**整条事件解码失败**（不是少显示一张卡）——仓库不支持降级，变更说明须标「升级后不可回退」。
- **实现期真机核验门**（R5 / 边界条 16）：本期无官方 Anthropic 凭据，真机 `pause_turn` 续轮核验 MUST NOT 作为实现期门项或任务阻塞项；「相邻 assistant 消息是否被服务端接受」以已知限制留痕，与其余手工 E2E 统一并入**归档后人工验收**（结论记入验收记录，不阻塞实现与归档）。
- **新能力边界**：不新建 `desktop-agent-web-search` 之类能力——开关是 provider 的字段语义、搜索块是既有事件信封的块变体、卡片是既有工具卡注册表的一项，拆独立能力只会造出跨四能力的悬空契约。

## 能力

### 新增的能力

**无。** 本变更全部落在既有能力面内（事件词汇 / 引擎 / provider 管理 / 前端会话基建 / 壳插件面），不引入新能力边界——理由见「明确不做」末条。

### 修改的能力

- **desktop-agent-execution** —
  - **ADDED**「SDK 引擎云端 web_search 启用与开关链路」：常量面（kind / name / `max_uses = 5`）、`provider_tool()` 请求侧形状、开关五层穿透（store → 命令 → compose → 门面 → runner → loop）、**协议门控在解析期**（openai 恒 off）、`RunStarted.tools` 不含 `web_search`。
  - **ADDED**「web_search 服务端块的事件投影与回放保真」：`project_choice` 规则（use + 配对 result 收敛为一块、孤儿 result 单独成块、其他 raw server 块跳过）、`AgentCitation` / `WebSearchSource` 字段语义、`raw_use` / `raw_result` 为回放唯一事实源、`replay_raw` 必须配空文本（rig 校验）。
  - **ADDED**「`pause_turn` 续跑（独立计数轴）」：续跑预算 `PAUSE_TURN_MAX_CONTINUATIONS = 5`、续跑不吃 `retry.max_attempts`（每次续跑前 `attempt = 0`）、usage 照实累加、到顶按 `stop` 收口并发 `pause_turn_capped`、每次续跑发 `pause_turn_continued`、空 choice / 有客户端 `tool_calls` 不续跑。
  - **ADDED**「上下文防线 server 块孤儿清扫」：`sweep_orphans` 索引源扩为 `WebSearch.id` / `raw_result.tool_use_id`，清扫 assistant 位无配对 use 的 result 块，剔空后整条移除，三处调用点自动生效。
  - **MODIFIED**「AgentEvent 信封与 loop 可见性」：`AgentBlock` 词汇由四变体扩为五变体（`WebSearch`）+ `Text.citations`；明确事件 payload 为自描述 JSON、**无需 store schema 版本升级**、旧事件缺 `citations` 读出 `None`。
  - **MODIFIED**「引擎门面与 EngineKind 参数选择」：门面旁路载荷由「`context_length` / `max_tokens`」扩为三项（增 `web_search`）；`EngineConfig` 连接配置字段面与 `runner_for` 唯一 match 点仍不塌。
  - **MODIFIED**「SDK 租户 MVP 边界与已知限制」：增五条边界（无实时搜索提示 / 转录体积不设上限且不压缩 / 旧客户端不可回退 / `max_uses` 不可配置 / 连续 assistant 消息依赖 Anthropic 服务端合并）；其中**边界条 16（连续 assistant 消息）不设实现期核验门**——该依赖记作**已知限制**（真机凭据不在自动化验证面内），确认动作统一落在**归档后人工验收**，其降级预案作为该条落痕（非仅口头承诺）。
- **desktop-agent-management** —
  - **MODIFIED**「全局 agent 管理命令面」：provider 字段面增 `web_search: bool`（缺省 `false`，不参与保存期协议校验——生效门在解析期）；`save_agent_provider` 末位增第 9 参；store 由 v3 演进到 v4（现 v3 改名 `AgentProviderRecordV3` 作升级链中转，`new(...)` 现有参数面不动 + `with_web_search` 构造器，38 处既有调用点零改）。
  - **MODIFIED**「Agent 管理页」：provider 表单增协议门控 checkbox（仅 `protocol === 'anthropic'` 渲染，`data-testid="provider-web-search"`，提示「Anthropic 服务端网页搜索（按次计费）」，切协议走清零点）；hooks 动作入参与 positional invoke 末位追加 `webSearch`。
- **desktop-agent-chat-infra** —
  - **MODIFIED**「事件适配层(纯函数,保真)」：`BlockPlan` 的 text 计划携 `citations`、tool 计划承载搜索块（`toolName: 'web_search'`，`input: { query }`，`output` 为 `AgentToolOutput` 加位形态 `{ content: '', isError, search?: { results, error } }`；未配对 = `null`）；`plansToParts` 产出 `providerMetadata: { agent: { citations } }`（空数组不挂）；`PairingScope.global` / `local` 按 `id` 扩展 server 块配对（跨消息孤儿 result 并入 use）；`eventToChunk` 与 `agent-transport.ts` **零改动**。
  - **ADDED**「web 搜索活动与引用来源呈现」：`WebSearchCard`（summary 恒「网络搜索」，body = 查询词 / 来源列表 / 错误，`data-testid="block-web-search"`）、`CitationList`（正文底部，url 去重）、`ExternalLink`（仅 http(s) 渲染为链接，唯一外链调用点）；`DEFAULT_TOOL_CARDS['web_search']` 注册；`useHistoryWindow` 过滤空 parts 消息。
- **desktop-app-shell** —
  - **ADDED**「外链打开插件注册与 capabilities 最小权限面」：`tauri-plugin-opener` 加入 workspace 与 `src-tauri/Cargo.toml`、`lib.rs` builder 链注册（与 `dialog` 同位）、`capabilities/default.json` 权限**只放 http(s) 系**（MUST NOT 连带放开 `mailto:` / `tel:`）、前端唯一调用点收敛于 `ExternalLink`。

### 其余能力零改动

- **desktop-workspace-store**：`SessionConfigSnapshot` 字段面不动（开关为 provider 属性，不落会话快照——与「改 `base_url` / `api_key` 同权」先例一致）；会话转录单表与信封 API 零改（事件 payload 为自描述 JSON，无需 schema 版本升级）。
- **desktop-ipc-type-bindings**：绑定生成机制零改，产物经 `bindings:export` 再生成。
- **desktop-standard-components**：checkbox 用现有 shadcn 控件，不新增标准组件。
- **desktop-crate-layout / module-boundary-contracts**：不新增 crate、不新增跨 crate 依赖边（`core/agent` 只增块词汇与值类型）。
- **`crates/infra/checks`**、`crates/core/orchestration/src/transcript.rs`：仅 `AgentBlock::Text` 模式改 `{ text, .. }`（后者），checks 零改。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/agent/src/event.rs` | `AgentBlock` 增 `WebSearch` 变体，`Text` 增 `citations: Option<Vec<AgentCitation>>`（仅 `#[serde(default)]`：键恒在、值可为 `null`；MUST NOT 加 `skip_serializing_if`——unified 相位模型无法表达条件省略，加则 bindings 分裂）；新增 `AgentCitation { url, title, cited_text, raw }` 与 `WebSearchSource { url, title, page_age }` |
| `crates/core/agent/src/lib.rs` | 导出 `AgentCitation` / `WebSearchSource` |
| `crates/core/orchestration/src/transcript.rs` | `AgentBlock::Text { text }` → `{ text, .. }`（模式补位） |
| `crates/infra/agent/src/sdk/web_search.rs`（新） | 常量面 + `provider_tool` / `raw_content` / `citations_of` / `project_choice` / `replay_raw` / `replay_text` |
| `crates/infra/agent/src/sdk/mod.rs` | 挂载 `web_search` 模块与单测模块 |
| `crates/infra/agent/src/sdk/loop.rs` | `LoopTurn` 增 `web_search: bool`；请求按需挂 provider tool；块转换换 `project_choice`；`'attempt` 返回 `finish_reason`；新增 `pause_turn` 续跑循环（独立计数轴 + 两条 notice） |
| `crates/infra/agent/src/sdk/resume.rs` | `text_content` / `assistant_content` 扩 `citations` 与 `WebSearch` 展开（一条块最多两条 `AssistantContent`；双 `None` 跳过；assistant 位 `ToolResult` 仍跳过） |
| `crates/infra/agent/src/sdk/context.rs` | `sweep_orphans` 扩 assistant 位 server 块配对（索引源 `WebSearch.id` / `raw_result.tool_use_id`；剔空整条移除） |
| `crates/infra/agent/src/sdk/runner.rs` | `SdkRunner` 增 `web_search: bool` + `with_web_search`（`new` 签名不动）；`PumpPayload` 增字段；`session_pump` 构造 `LoopTurn` 时传递；启动日志加 `web_search=…` |
| `crates/infra/agent/src/lib.rs` | `EngineFacade` 增 `web_search` 字段 + `with_web_search`（与 `with_max_tokens` 同型；四个构造点补齐） |
| `crates/infra/agent/src/compose.rs` | `ResolvedEngine` 增 `web_search`（= `provider.web_search && protocol == Anthropic`）；`runner_for` 链加 `.with_web_search(...)` |
| `crates/infra/store/src/model.rs` | 现 `AgentProviderRecord` 改名 `AgentProviderRecordV3`（原样不动，`pub(crate)` 升级链中转）+ 新 `AgentProviderRecord`（`#[native_model(id = 5, version = 4, from = AgentProviderRecordV3)]`，增 `#[serde(default)] web_search: bool`）；补 V3↔V4 双向 `From`；`new(...)` 现有参数面不动 + `with_web_search(bool)`；手写遮蔽 `Debug` 增字段位 |
| `src/commands/agents/mod.rs` | `save_agent_provider` 末位追加 `web_search: bool`（第 9 参），构造后 `.with_web_search(web_search)`；**不加协议校验**（openai 也收，生效门在解析期） |
| `packages/desktop/package.json` | 版本 `0.5.28` → `0.5.29`（用户可见功能变更）；依赖增 `@tauri-apps/plugin-opener` |
| `packages/desktop/src/types/generated/bindings.ts` | `pnpm -C packages/desktop run bindings:export` 再生成（非手改）：`AgentBlock` 新变体 + `citations` + provider 记录 `webSearch` + `saveAgentProvider` 新参数序 |
| `packages/desktop/src/lib/agent-adapter.ts` | `BlockPlan` text 计划携 `citations`；tool 计划承载搜索块（`input: { query }`，配对时 `output` 为 `AgentToolOutput` 加位形态 `{ content: '', isError, search: { results, error } }`，未配对 `output: null`）；`plansToParts` 挂 `providerMetadata: { agent: { citations } }`（空数组不挂）；`planBlock` 处理 `webSearch` 与 `text.citations`；`PairingScope` 扩展按 `id` 的 server 块配对（含跨消息）；`textChunks` 的 `text-start` 带同一 metadata；`AgentToolOutput` 只加可选位 `search?`（既有消费面零改）；`eventToChunk` 零改 |
| `packages/desktop/src/components/agent/agent-blocks.tsx` | 增 `WebSearchCard` / `CitationList` / `ExternalLink`；text 块渲染时挂引用列表 |
| `packages/desktop/src/components/agent/tool-cards.tsx` | `DEFAULT_TOOL_CARDS['web_search'] = WebSearchCard` |
| `packages/desktop/src/components/agent/agent-messages.tsx` | `useHistoryWindow` 的 `conversationMessages` 过滤追加 `message.parts.length > 0` |
| `packages/desktop/src/views/agents/components/provider-panel.tsx` | `ProviderFormState.webSearch: boolean`；`ProviderTextField` 排除该键；仅 anthropic 渲染 checkbox（`data-testid="provider-web-search"` + 计费提示）；协议切走清零；submit 传 `webSearch` |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | `AgentProviderSaveInput` 与 positional invoke 末位追加 `webSearch` |
| `packages/desktop/src-tauri/Cargo.toml` | workspace 依赖增 `tauri-plugin-opener = "2"`；`src-tauri` 根依赖引用 |
| `packages/desktop/src-tauri/src/lib.rs` | builder 链增 `.plugin(tauri_plugin_opener::init())`（与 `dialog` 同位） |
| `packages/desktop/src-tauri/capabilities/default.json` | 权限增 opener 的 **URL 打开**项，scope 只放 `http://*` / `https://*` |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| `WebSearch` / `WebSearchSource` / `AgentCitation` serde 往返；旧 `Text` JSON 无 `citations` 读出 `None`；缺省键落 `null` / `[]`（键恒在） | `crates/core/agent/src/event_test.rs` | 新增用例 |
| use + result 合并为一块；错误 result（`error_code`）；孤儿 result；未知 raw server 块跳过；citations 提取（展示字段）；`replay_raw` / `replay_text` 的 `additional_params` 逐字节等价 | `crates/infra/agent/src/sdk/web_search_test.rs`（新） | 新建，逐规则覆盖 |
| 开关开 / 关时 `additional_params.tools` 精确形状；`RunStarted.tools` 不含 `web_search`；脚本化 server 块产出 `WebSearch` + `Text.citations` 且不触发工具执行；`pause_turn` 续跑（两次请求、续跑不吃重试预算、上限 5、到顶正常收敛） | `crates/infra/agent/src/sdk/loop_test.rs` | 新增用例（复用 rig `test-utils` 流缝） |
| `WebSearch` 重建为 use → result 两条 raw `AssistantContent`（原序）；双 `None` 跳过；citations 回填；project → resume 往返语义不变 | `crates/infra/agent/src/sdk/resume_test.rs` | 新增用例 |
| `sweep_orphans` 剔除 assistant 位孤儿 server result；成对块保留；剔空消息整条移除 | `crates/infra/agent/src/sdk/context_test.rs` | 新增用例 |
| `normalize_test.rs` | — | **零新增用例**（改动该文件即说明实现偏离决策 ⑦） |
| 协议门控（`anthropic + true → true` / `openai + true → false` / `false → false`）与 facade 传递 | `crates/infra/agent/src/sdk/runner_test.rs`、`lib_test.rs` | 新增用例 |
| `ResolvedEngine.web_search` 三组映射 | `crates/infra/agent/src/compose_test.rs` | 新增用例 + fixture 补 `with_web_search` |
| v3 → v4 升级默认 `false`、降级丢字段、版本头 `4`、旧 JSON 缺字段；含新块的 `SessionEventRecord` JSON 往返 | `crates/infra/store/src/model_test.rs`、`envelope_test.rs` | 新增用例（38 处 `new` 调用点零改） |
| 保存 / 读取 `web_search`；openai 协议下也接受该值 | `src/commands/agents/mod_test.rs` | 新增用例 + 既有 `save_agent_provider` 调用点补第 9 参 |
| `agentStart` 之外的绑定形状守卫随再生成刷新 | `src/bindings/mod_test.rs` | 快照随再生成 |
| `webSearch` → tool 部件两态（配对态 `output` 为 `AgentToolOutput` 加位形态：`content: ''`、`isError = error != null`、`search: { results, error }`；未配对 `output: null`）；text 计划 `providerMetadata.agent.citations`；跨消息 use↔result 合并；无配对 use 只出查询词；空 citations 不挂 metadata | `src/lib/agent-adapter.test.ts` | 新增用例 |
| 空 parts 消息不渲染；既有气泡计数不受影响 | `src/components/agent/agent-messages.test.tsx` | 新增用例 |
| `Object.keys(DEFAULT_TOOL_CARDS)` 含 `web_search` | `src/components/agent/tool-cards.test.tsx` | 既有断言扩位 |
| `WebSearchCard`（恒标题 / 查询词 / 来源 / 错误）、`CitationList`（url 去重）、`ExternalLink`（非 http(s) 不渲染为链接） | `src/components/agent/agent-blocks.test.tsx`（新） | 新建用例 |
| 仅 anthropic 可见、默认关、切协议清零、提交载荷 | `src/views/agents/components/provider-panel.test.tsx` | 新增用例 |
| invoke 位置参数补位 | `src/views/agents/hooks/use-agent-providers.test.ts` | 断言随动 |
| provider fixture 补 `webSearch` | `src/views/agents/agent*`（view / panel）测试 | 补字段 |

### 不要修改

- **`crates/infra/agent/src/sdk/normalize.rs`**：无 `AgentDelta::Search`（决策 ⑦ 的机械判据——该文件出现 diff 即说明实现偏离）。
- **`packages/desktop/src/lib/agent-transport.ts`**：零改动（无新事件类型需转 chunk）。
- **`crates/infra/agent/src/sdk/compact.rs`**：摘要请求恒无工具（server tool 不进 compaction 请求）。
- **`crates/infra/agent/src/sdk/policy.rs` / `sandbox.rs` / `tools.rs`**：`web_search` 是 provider 服务端工具，**不进客户端工具面**、不进权限决策表、不进 `TOOL_NAMES`。
- **`RunStarted` 契约**：`tools` 字段语义不变（本轮下发的**客户端**工具面），不加 `serverTools`。
- **`SessionConfigSnapshot`**：不记 `web_search`（开关为 provider 属性，续轮按 provider 当前值，与改 `base_url` / `api_key` 同权）。
- **`EngineConfig`**：连接配置字段面不增字段——`web_search` 走门面旁路（与 `context_length` / `max_tokens` 同型）。
- **`crates/infra/agent/src/sdk/model_spec.rs`**：`[1m]` 后缀与 `web_search` 无耦合，解析规则不动。
- **`AgentProviderRecord` 的 api_key 机密边界**：明文存储 / 传输、仅展示层遮蔽的边界表不动，仅 Debug impl 增 `web_search` 位。
- **`openspec/changes/anthropic-web-search/explore.md`**：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | `AgentBlock` 五变体：`WebSearch { id, query, results, error, raw_use, raw_result }` 在场，`Text` 带 `citations: Option<Vec<AgentCitation>>`；旧 `Text` JSON（无 `citations`）读出 `None`；序列化缺省时 `citations` / `error` / `raw_use` / `raw_result` 落 `null`、`results` 落 `[]`——**键恒在**（bindings 统一形态的前提，见 design D1）；`AgentCitation` / `WebSearchSource` 经 `crates/core/agent/src/lib.rs` 公开导出 | `event_test` serde 往返 + 源码复核导出面 |
| AC-2 | 事件落库**零 schema 版本升级**：含 `WebSearch` 块的 `SessionEventRecord` JSON 往返保真（`raw_use` / `raw_result` 逐字节不变） | `model_test` 往返用例 + store 版本头未变（仍无转录模型版本演进） |
| AC-3 | 请求侧：开关开 → `additional_params.tools` 恰为 `[{type:"web_search_20250305", name:"web_search", max_uses:5}]`；开关关 → 该键不存在；`RunStarted.tools` 两态均**不含** `web_search`；`normalize.rs` 零 diff | `loop_test` 精确形状断言 + `git diff` 复核 |
| AC-4 | 投影：同一请求内 `server_tool_use` 与其配对 `web_search_tool_result` 收敛为**恰一块** `WebSearch`（`id` 取 `server_tool_use.id`）；孤儿 result 单独成块（`raw_use: None`）；其他 raw server 块跳过不产块；`results` 取 `web_search_result` 的 url / title / page_age；错误对象取 `error_code` | `web_search_test` 规则表逐条 |
| AC-5 | 回放保真：`WebSearch` 重建为 use → result 两条 `AssistantContent`（**保原序**），`raw` 放回 `additional_params.anthropic_content` 且文本为空；双 `None` 块跳过；assistant 位 `ToolResult` 仍跳过；`Text.citations` 以 `[c.raw, …]` 回填 `additional_params.citations`（MUST NOT 用展示字段重新组装） | `web_search_test` + `resume_test` 逐字节等价断言 |
| AC-6 | `pause_turn` 续跑：每次续跑请求续发、发 `SystemNotice{subtype:"pause_turn_continued"}`；每次续跑前 `attempt = 0`（一次续跑后发生网络失败仍有完整重试预算，可测）；usage 逐次累加；达 `PAUSE_TURN_MAX_CONTINUATIONS = 5` 后按 `stop` 正常收敛（**非** `api_error`）并发 `pause_turn_capped`（payload 含 `continuations: 5`）；空 choice 与存在客户端 `tool_calls` 时不续跑 | `loop_test` 脚本化 `FinalResponse(Finish)` 驱动 |
| AC-7 | 防线配对：`sweep_orphans` 剔除 assistant 位「`raw_use == None` 且前序无对应 use」的块；成对块保留；剔除后 content 变空的消息整条移除；三处调用点行为一致 | `context_test` 新增用例 + 既有 prune / hard_prune 用例不回归 |
| AC-8 | 开关五层穿透：store v4 读写 `web_search`；`save_agent_provider` 第 9 参接受 openai 协议下的 `true`（不报错）；`ResolvedEngine.web_search = provider.web_search && protocol == Anthropic`（三组映射逐格）；`EngineFacade::with_web_search` 与 `SdkRunner::with_web_search` 传递可考；`EngineConfig` 字段面不变、`runner_for` 唯一 match 点不变；`AgentProviderRecord::new` 现有参数面不变（38 处调用点零改） | `model_test` / `mod_test` / `compose_test` / `lib_test` / `runner_test` + 源码复核 |
| AC-9 | 前端适配：`webSearch` 块 → `tool-web_search` 部件两态（未配对 = `input-available` 只显查询词、`output: null`；配对 = `output-available` 且 `output` 为 `AgentToolOutput` 加位形态——`content: ''`、`isError = error != null`、`search: { results, error }`，既有 `content` / `isError` 位语义不破）；text 部件的 `providerMetadata.agent.citations` 在场且空数组不挂；`agent` 命名空间（非 `anthropic`）；跨消息 use↔result 按 `id` 合并；`eventToChunk` 与 `agent-transport.ts` 零 diff | `agent-adapter.test.ts` + `agent-transport.test.ts` 零 diff 复核 |
| AC-10 | 呈现：`WebSearchCard` summary 恒「网络搜索」（无「正在搜索…」、无状态切换）、body 呈查询词 / 来源列表 / 错误；`CitationList` 按 url 去重、`title ?? url`；`ExternalLink` 非 http(s) 不渲染为链接；`DEFAULT_TOOL_CARDS` 含 `web_search`；空 parts 的 assistant 消息不进对话透镜且不占分页计数 | `agent-blocks.test.tsx` + `tool-cards.test.tsx` + `agent-messages.test.tsx` |
| AC-11 | 管理页：checkbox 仅 `protocol === 'anthropic'` 渲染（默认关，切协议清零），提交载荷含 `webSearch`，invoke 位置参数逐位对齐 | `provider-panel.test.tsx` + `use-agent-providers.test.ts` |
| AC-12 | 外链：`tauri-plugin-opener` 已注册（builder 链与 `dialog` 同位），capabilities 权限项**只放 http(s)**（不含 `mailto:` / `tel:` / 任意默认全开项）；前端外链调用点唯一（`ExternalLink`） | 源码复核 + `capabilities/default.json` 逐项审阅 |
| AC-13 | 静态检查与测试全绿：`cargo clippy` 零告警；`bindings:check` 零 diff（绑定由再生成产出）；`pnpm -C packages/desktop run check` 与 `run test` 全绿；`packages/desktop/package.json` 版本 `0.5.29` | 静态检查链 + 全量测试 |

### 验证命令

```powershell
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p store
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p dev-team
cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml

pnpm -C packages/desktop run bindings:export
pnpm -C packages/desktop run bindings:check
pnpm -C packages/desktop run client:test
pnpm -C packages/desktop run check
pnpm -C packages/desktop test
```

> **验收面口径**：验收止于**单元测试 + 静态检查链**。真机 E2E（官方 Anthropic 端点 + 凭据 + 真实 `pause_turn`）不属本工作流自动化验证面，以 explore「验证」节的手工清单承载，**MUST NOT 被转写为任务清单的前置门或阻塞项**——其中「相邻 assistant 消息是否被服务端接受」按「风险 R5」记作**已知限制**，与其余手工项统一并入**归档后人工验收**执行（不阻塞实现与归档）；「续跑计数轴」按「风险 R3」以脚本化用例锁定。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 回放保真写错即续轮 400**：`encrypted_content` 等字段不可推导，`raw_use` / `raw_result` 必须原样保存与重发；展示字段（query / results / error）**不得**参与重建 | 带搜索的会话续轮被 Anthropic 拒 / 失忆 | 中 | AC-5 以「`additional_params` 逐字节等价」为断言口径（不是「语义等价」）；`web_search.rs` 只暴露 `replay_raw` 一条重建路径，展示字段与 wire 字段在类型上分家 |
| **R2 citations 规范化漂移**：rig 的 `Citation` 反序列化再序列化会规范化（`title: Option` → `null`），用 `anthropic_citations()` 的结果回填会丢保真 | 引用回放形态与原始 wire 不一致 | 中 | spec 明确：展示字段可用 rig 解析，**raw 必须取 `additional_params["citations"]` 原值**；AC-5 逐字节断言覆盖 |
| **R3 `pause_turn` 跨消息配对**：一次搜索可能拆成两条 assistant 消息（use 在前、result 在后）；两条独立计数轴写错会把续跑吃进重试预算或反之 | 长搜索腰斩 / 重试预算被续跑吞掉 / 死循环 | 高（无真机手段时尤甚） | 常量与计数轴分名（`PAUSE_TURN_MAX_CONTINUATIONS` vs `WEB_SEARCH_MAX_USES`，命名、注释、日志字段不得混用）；AC-6 逐条锁定（含「续跑后失败仍有权重重试」的反向断言）；到顶按 `stop` 收敛而非 `api_error` |
| **R4 空 parts 消息**：实时路 `pause_turn` 第二段只有孤儿 result → 0 部件 assistant 消息，渲染成空气泡 | 时间线出现空白气泡 | 中 | 决策 ⑨：`useHistoryWindow` 过滤 `parts.length > 0`（顺带修正分页计数）；AC-10 显式断言 |
| **R5 连续 assistant 消息**：`pause_turn` 天然产生 `Assistant → Assistant`，rig 编码侧不做同角色合并，依赖 Anthropic 服务端合并 | 回放被拒（服务端行为不可控） | 中 | **不设实现期核验门**（真机凭据不属本期可用条件，把无法履行的核验写成门只会阻塞实现）：本期**接受该不确定性**，作为**已知限制**留痕于 `desktop-agent-execution`「SDK 租户 MVP 边界与已知限制」边界条 16；确认动作统一并入**归档后人工验收**——以官方端点 + 凭据的真实 `pause_turn` 会话续轮确认服务端接受相邻同角色消息，结论记入验收记录，且**不阻塞实现与归档**。若人工验收确认被拒，降级预案为回放时按 `pause_turn` 边界**合并相邻 assistant 消息**为一条 `AssistantContent` 序列——本期不预建、无实现分支，后续变更据边界条 16 偿还，无需重新论证 |
| **R6 opener 权限标识符形状**（explore 遗留项，本提案裁定） | 外链点击无反应或权限过宽（连带放开 `mailto:` / `tel:`） | 中 | 裁定：**授权以「只放 http(s)」为硬约束**——实现期优先核实 `opener:allow-open-url` + scope 形态；若插件只提供 `opener:allow-default-urls`（连带 `mailto:` / `tel:`）→ **回落显式 `allow-open-url` + scope**，宁可多一步配置也不接受宽权限。AC-12 以「权限项逐项审阅、无宽权限项」为验收口径 |
| **R7 网关兼容**：不支持 server tools 的 anthropic 兼容网关会在请求期失败 | 用户体感「开了开关就跑不起来」 | 中 | 开关默认关；失败以既有 `api_error` 显式暴露（不静默重试、不无工具降级）；表单提示「按次计费」并在 spec 边界条留痕 |
| **R8 前端两路形状差**：实时路 `pause_turn` 期间卡片只显查询词（`output: null`），轮末重放路显来源列表 | 用户以为卡片「加载失败」 | 中 | 卡片**无状态文案**（决策 ③：summary 恒「网络搜索」）使两态视觉一致；`use-agent-session` 的收口语义（流结束按库内转录重建，终态以库为唯一权威）天然给出终态一致；AC-9/AC-10 锁定两路产物形状 |
| **R9 不可回退**：旧客户端读到含 `webSearch` 块的事件会**整条事件解码失败**（不是少显示一张卡） | 版本混用时历史会话打不开 | 低（单机自升级） | 变更说明标注「升级后不可回退」；不实现降级分支（spec 边界条留痕）；`cargo` 侧 serde 新增变体为编译期可见改动，无静默路径 |
| **R10 转录体积**（explore 遗留项，本提案裁定） | `raw_result` 含 `encrypted_content` + 每条来源加密索引，一次搜索数 KB，库文件随搜索次数线性增长 | 低（本地 redb，写入量级远低于 token 流） | 裁定：**不设上限、不压缩**（压缩破坏 `raw` 逐字节保真，与 R1 直接冲突）；留痕于「SDK 租户 MVP 边界与已知限制」，与既有「转录无上限增长」边界同列 |
| **R11 开关与协议的组合面**：openai + `true` 的存量记录（用户先开开关再改协议） | 误期望 openai 端点也联网 | 中 | 生效门在**解析期**（`provider.web_search && protocol == Anthropic`）；表单层切协议清零 + 仅 anthropic 渲染；AC-8 三组映射逐格锁定 |
| **R12 存量 provider 记录升级**：v3 → v4 演进需现 v3 改名作升级链中转 | 存量 provider 读不出 | 中 | 沿 `AgentProviderRecordV2` 现成先例（改名为 `AgentProviderRecordV3`，原样不动、`pub(crate)`）；`web_search` 用 `#[serde(default)]` + 升级侧 `false`，行为零变化；AC-8 / model_test 覆盖三条路径（v3 存量行 / v4 新行 / 旧 JSON 缺字段）；38 处 `new` 调用点不改（`with_web_search` 构造器） |
