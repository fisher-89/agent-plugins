# Anthropic 协议启用云端 Web Search

workflow_type: requirement

## Context

桌面端 SDK 引擎（`packages/desktop/src-tauri/crates/infra/agent`）目前只向模型下发七个客户端工具，Anthropic 协议下没有任何服务端工具。目标：**provider 协议为 `anthropic` 时，可按 provider 开关启用 Anthropic 云端 `web_search`，并在会话时间线展示搜索活动与引用来源**。

rig 0.43.0 已提供所需的两半能力，**无需升级或打补丁**：

- **请求侧**：`rig::completion::ProviderToolDefinition` + `CompletionRequest::provider_tool()` → 落到 `additional_params.tools` → Anthropic adapter 追加进 wire `tools` 数组。
- **响应侧**：`server_tool_use` / `web_search_tool_result` 解码为 `AssistantContent::Text { text: "", additional_params: { anthropic_content: <raw> } }`；citations 落在 `additional_params["citations"]`；回放时把 raw 放回 `additional_params` 即可原样重发。

真正缺口全在本仓库：provider 开关、事件契约、持久化与续轮回放、`pause_turn` 续跑、时间线渲染。当前 server 块会被存成空 `Text`，且每次续轮都从转录重建历史，后续轮次丢失搜索上下文。

## 决策总表

MVP 目标是**最终可见**：过程允许空白，实时搜索提示整条不做。

| # | 决策 | 要点 |
| --- | --- | --- |
| ① | citations 挂**正文部件** | 复用 text 部件的 `providerMetadata`，不设独立 `data-citations` 部件 |
| ② | `web_search: bool` | store v4；`AgentProviderRecord::new` 签名不动 + `with_web_search`；`Text.citations` 用 `Option<Vec<_>>` |
| ③ | 搜索卡文案**无状态** | summary 恒为「网络搜索」；无「正在搜索…」、无超时降级 |
| ④ | citations 命名空间**自有** | `providerMetadata: { agent: { citations } }`（provider 外消费即不借 provider 名） |
| ⑤ | 引用用**正文底部列表** | `title ?? url`，按 url 去重；不做角标、不做原句高亮 |
| ⑥ | **不加** `serverTools` | `RunStarted` 契约不变，工具面板看不见 server tool |
| ⑦ | **删实时 delta** | 无 `AgentDelta::Search`；`normalize.rs` 与 `agent-transport.ts` 零改动 |
| ⑧ | 搜索卡承载用 **`tool-web_search` 部件** | 复用 `planToolPart` / `plansToChunks` / `DEFAULT_TOOL_CARDS` 现成管线 |
| ⑨ | **空 parts 消息不渲染** | 渲染层 `useHistoryWindow` 过滤 `parts.length > 0` |
| ⑩ | 外链用**官方 opener 插件** | `tauri-plugin-opener`，scope 限 `http(s)` |
| ⑪ | **`sweep_orphans` 扩展** | 认 assistant 位 server 块配对（正确性硬缺口） |
| ⑫ | **`pause_turn` 续跑不计入重试预算** | 续跑独立计数轴；usage 照实累加；到顶正常收敛 |

非阻塞遗留（propose 阶段处理）：转录体积不设上限不压缩；opener 权限标识符与 scope 形状实现期核实。

## 最终改动面

### 1. core 事件契约 — `crates/core/agent/src/event.rs`

```rust
pub enum AgentBlock {
    Text {
        text: String,
        /// 空/None = 无引用（旧事件缺字段读出 None，前端 fixture 零改动）
        citations: Option<Vec<AgentCitation>>, // #[serde(default, skip_serializing_if = "Option::is_none")]
    },
    Thinking { thinking: String },
    ToolUse { id, name, input },
    ToolResult { id, content, is_error },
    /// 一次 server_tool_use 与其配对 result 收敛为一块
    WebSearch {
        id: String,                    // server_tool_use.id == result.tool_use_id
        query: String,
        results: Vec<WebSearchSource>, // #[serde(default, skip_serializing_if = "Vec::is_empty")]
        error: Option<String>,         // web_search_tool_result_error.error_code
        raw_use: Option<serde_json::Value>,    // 回放唯一事实源
        raw_result: Option<serde_json::Value>, // 保留 encrypted_content
    },
}

pub struct AgentCitation {
    pub url: Option<String>,
    pub title: Option<String>,
    pub cited_text: Option<String>,
    pub raw: serde_json::Value, // 原样 wire 引用，回放用
}

pub struct WebSearchSource {
    pub url: String,
    pub title: Option<String>,
    pub page_age: Option<String>,
}
```

其余必要改动：

- `crates/core/agent/src/lib.rs` 导出 `AgentCitation` / `WebSearchSource`。
- `crates/core/orchestration/src/transcript.rs` 的 `AgentBlock::Text { text }` 模式改 `{ text, .. }`。
- `AgentBlock::Text` 全仓命中 **29 处**（生产代码仅 `loop.rs` 两处、`resume.rs` 一处，其余为测试）。

> 事件 payload 是 `SerdeJsonCodec` 自描述 JSON，新增变体与带默认值字段**无需 store schema 版本升级**；旧事件缺 `citations` 读出 `None`。

### 2. 引擎能力 — 新建 `crates/infra/agent/src/sdk/web_search.rs`

注册到 `sdk/mod.rs`。

**常量**

```rust
WEB_SEARCH_TOOL_KIND  = "web_search_20250305"
WEB_SEARCH_TOOL_NAME  = "web_search"
WEB_SEARCH_MAX_USES   = 5          // provider 侧搜索次数上限（成本护栏）
ANTHROPIC_RAW_CONTENT_KEY = "anthropic_content"
CITATIONS_KEY         = "citations"
```

**函数**

| 函数 | 职责 |
| --- | --- |
| `provider_tool() -> ProviderToolDefinition` | `{type: web_search_20250305, name: web_search, max_uses: 5}` |
| `raw_content(&Text) -> Option<&Value>` | 取 `additional_params.anthropic_content` |
| `citations_of(&Text) -> Option<Vec<AgentCitation>>` | 展示字段自 raw 解，`raw` 保留原值 |
| `project_choice(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)` | 见下 |
| `replay_raw(&Value) -> AssistantContent` | `Text { text: "", additional_params: {anthropic_content: raw} }`（raw 块**必须**配空文本，rig 校验） |
| `replay_text(text, citations) -> AssistantContent` | 非空 citations → `additional_params.citations = [c.raw, ...]` |

**`project_choice` 规则**

```
先按 tool_use_id 索引全部 web_search_tool_result
顺序遍历 choice：
  server_tool_use(name=="web_search")  → AgentBlock::WebSearch{ id, query, raw_use }
  配对的 web_search_tool_result        → 不重复产出（已并入上面那块）
  孤儿 web_search_tool_result          → AgentBlock::WebSearch{ raw_use: None, … }
  其他 raw server 块                    → 跳过
  Text                                 → AgentBlock::Text{ text, citations }
  Reasoning / ToolCall / Image         → 现状不变
result content 数组 → 取 web_search_result 的 url/title/page_age
错误对象 → error_code
```

### 3. 循环 — `crates/infra/agent/src/sdk/loop.rs`

- `LoopTurn` 增加 `web_search: bool`。
- 请求构造后按需挂 provider tool：

```rust
let mut request = CompletionRequest { /* tools: definitions.clone() 不变 */ };
if turn.web_search {
    request = request.provider_tool(web_search::provider_tool());
}
```

- 用 `web_search::project_choice(&choice)` 替换现有内联块转换；`history.push(Message::Assistant { .. })` 与密封消息发送不变。
- `'attempt` 返回 `(choice, usage, finish_reason)`（`CompletionResponse::finish_reason()`）。

**`pause_turn` 续跑：两条独立计数轴**

```
attempt 轴（重试，预算 retry.max_attempts）        continuation 轴（续跑，预算 5）
──────────────────────────────────────────────────────────────────────────────
流失败  → attempt += 1，退避重发                    pause_turn → continuation += 1
成功    → 收口（工具执行 / 结束）                    └─ attempt 归零，不占重试预算
```

实现约束：

1. **续跑不占 `retry.max_attempts`**：每次续跑前 `attempt = 0`。否则一次 pause_turn 后续的网络抖动会继承前面积累，变相被续跑吃掉预算。
2. **usage 照实累加**：每次续跑 `usage += turn_usage`（真实计费请求，不合并、不抹零）。
3. **到顶正常收敛**：`PAUSE_TURN_MAX_CONTINUATIONS = 5` 到顶按 `stop` 收口（**不是** `api_error`），发 `SystemNotice { subtype: "pause_turn_capped", payload: { continuations: 5 } }`。
4. 每次续跑发 `SystemNotice { subtype: "pause_turn_continued" }`——这是**唯一的过程可观测点**。
5. 空 choice 不续跑；有客户端 `tool_calls` 时不续跑。

> `PAUSE_TURN_MAX_CONTINUATIONS`（单轮续跑上限）与 `WEB_SEARCH_MAX_USES`（单请求搜索次数上限）是**两个独立概念**，常量名、注释、日志字段不得混用。

### 4. 回放 — `crates/infra/agent/src/sdk/resume.rs`

- `text_content`：`Text { text, .. }`。
- `assistant_content` 改为可展开的循环（一个 `WebSearch` 最多产两条 `AssistantContent`）：

```
Text { text, citations }        → replay_text(text, citations)
Thinking / ToolUse              → 现状不变
WebSearch { raw_use, raw_result }
                                → 依次 replay_raw，保持 use → result 原始顺序
                                → 两者皆 None 的块跳过
assistant 位 ToolResult         → 仍跳过
```

### 5. 上下文防线 — `crates/infra/agent/src/sdk/context.rs`（决策 ⑪）

`sweep_orphans` 当前只认 `UserContent::ToolResult ↔ CallId`。`pause_turn` 把一次搜索拆成两条 assistant 消息时，L2 剪裁 / L3 硬裁 / 摘要置换都可能把 use 裁掉而留下 result → 回放被 Anthropic 拒。

扩展为**同时认 assistant 位 server 块配对**：

- 索引源：`WebSearch.id`（use 侧）/ `raw_result.tool_use_id`（result 侧）
- 清扫对象：assistant 位中 `raw_use == None` 且前序找不到对应 use 的块
- 剔除后消息 `content` 变空则整条移除
- 触发点不变（三处调用点自动生效）

`compact.rs` 仍保持「摘要请求无工具」。

### 6. provider 开关链路

| 层 | 改动 |
| --- | --- |
| **store** `model.rs` | 现 v3 改名 `AgentProviderRecordV3`（原样）；新增当前 `AgentProviderRecord`：`native_model(id=5, version=4, from=AgentProviderRecordV3)`，加 `#[serde(default)] pub web_search: bool`；补 V3↔V4 双向 `From`；`new(...)` **8 参不变**（默认 false）+ `with_web_search(bool)`；手写 `Debug` 加字段 |
| **command** `commands/agents/mod.rs` | `save_agent_provider` 末尾追加 `web_search: bool`（第 9 参），构造后 `.with_web_search(web_search)`；不因 openai 拒收，生效门在解析期 |
| **compose** `compose.rs` | `ResolvedEngine` 加 `web_search: bool` = `provider.web_search && protocol == Anthropic`；`runner_for` 链加 `.with_web_search(...)` |
| **facade** `lib.rs` | `EngineFacade` 加字段 + `with_web_search`（与 `with_max_tokens` 同型） |
| **runner** `sdk/runner.rs` | `SdkRunner` 字段默认 false；`with_web_search`；`PumpPayload` 加字段；`session_pump` 构造 `LoopTurn` 时传递；启动日志加 `web_search=…` |

> `AgentProviderRecord::new` 全仓 **35 处**调用。改签名 = 35 处机械改动；加构造器 = 0 处。

### 7. 前端

- **重新生成** `src/types/generated/bindings.ts`（`pnpm -C packages/desktop run bindings:export`，不手改）。
- **provider 表单** `views/agents/components/provider-panel.tsx`：
  - `ProviderFormState.webSearch: boolean`；`ProviderTextField` 排除该键
  - 仅 `protocol === 'anthropic'` 渲染 checkbox（`data-testid="provider-web-search"`），提示「Anthropic 服务端网页搜索（按次计费）」
  - 协议切走时清零；submit 传 `webSearch`
- **hook** `views/agents/hooks/use-agent-providers.ts`：`AgentProviderSaveInput` 与 positional invoke 末位追加 `webSearch`
- **adapter** `src/lib/agent-adapter.ts`：
  - **不新增** `AgentDataParts` 成员
  - `BlockPlan` 的 text 计划携带 `citations`；`plansToParts` 产出 `providerMetadata: { agent: { citations } }`（空数组不挂）；`textChunks` 的 `text-start` 带同一 metadata
  - `BlockPlan` 的 tool 计划复用承载搜索：`{ kind:'tool', toolCallId: id, toolName:'web_search', input:{query}, output:{results,error}|null }`
  - `planBlock` 处理 `webSearch` 块与 `text.citations`
  - 扩展 global/local 配对：按 `id` 把跨消息的孤儿 result 并入 use（`pause_turn` 拆分场景）；local 视野下无配对 result 的 use 仍渲染（只显查询词）
  - `eventToChunk` **零改动**
- **transport** `src/lib/agent-transport.ts`：**零改动**
- **消息容器** `components/agent/agent-messages.tsx`：`useHistoryWindow` 的 `conversationMessages` 过滤追加 `message.parts.length > 0`（覆盖实时 chunk 路与重放 adapter 路，顺带修正分页计数）
- **渲染** `components/agent/agent-blocks.tsx` 新增：
  - `WebSearchCard`（summary 恒「网络搜索」；body：查询词、来源列表、错误；`data-testid="block-web-search"`）
  - `CitationList`（正文底部，按 url 去重）
  - `ExternalLink`（仅 http(s) 渲染为链接，唯一外部调用点）
- **注册表** `components/agent/tool-cards.tsx`：`DEFAULT_TOOL_CARDS['web_search'] = WebSearchCard`（同步更新 `tool-cards.test.tsx` 的 `Object.keys` 断言）
- **外链能力**（决策 ⑩）：
  - `tauri-plugin-opener = "2"` 加入 workspace 与 `src-tauri/Cargo.toml` 根依赖
  - `src-tauri/src/main.rs` builder 链加 `.plugin(tauri_plugin_opener::init())`（与 `dialog` 同位）
  - `capabilities/default.json` 加 `opener:allow-open-url`，scope 只放 `http://*` / `https://*`
  - 前端加依赖 `@tauri-apps/plugin-opener`

> ⚠️ **实现期核实**：权限标识符可能只有 `opener:allow-default-urls`，后者连带放开 `mailto:` / `tel:`。若无法收窄则改回显式 `allow-open-url` + scope。

### 8. 版本

`packages/desktop/package.json` `0.5.26` → `0.5.27`（用户可见功能变更）。

## 关键机制图

### 数据往返

```
Anthropic SSE                  rig 解码                        本仓库
──────────────────────────────────────────────────────────────────────────────
server_tool_use          ──▶  Text{text:"", params:        ──▶  AgentBlock::WebSearch{
  {id, name, input}             anthropic_content}                id, query, raw_use
web_search_tool_result   ──▶  Text{text:"", params:        ──▶  同一块 += results/error
  {tool_use_id, content[]}      anthropic_content}                + raw_result
text + citations_delta   ──▶  Text{text, params.citations} ──▶  AgentBlock::Text{text, citations}
──────────────────────────────────────────────────────────────────────────────
store（SerdeJsonCodec 自描述 JSON，无 schema 版本升级）
──────────────────────────────────────────────────────────────────────────────
resume 重建  ──▶  Text{"", {anthropic_content: raw_use}}     ← replay_raw
                  Text{"", {anthropic_content: raw_result}}  ← replay_raw
                  Text{text, {citations: raw[]}}             ← replay_text
```

### 前端两路（决策 ⑦⑧：无实时 delta，过程空白）

```
实时路（流内）                          轮末重放（库为准）
────────────────────────────────────────────────────────────────
密封 WebSearch{id, query}              eventsToUIMessages
  local 配对：无 result                    global 配对：跨消息合并
      ▼                                        ▼
tool-input-available                    output-available
  output: null                            output: {results, error}
      ▼                                        ▼
卡片只显「查询词」                        卡片显「查询词 + 来源列表」
（pause_turn 期间可能数十秒）              （终态，用户最终看到的就是这个）
```

`use-agent-session` 的收口语义（流结束后按库内转录重建，终态以库为唯一权威）使「过程空白、最终可见」天然成立，**MVP 不需要额外机制**。

### 搜索卡

```
summary（恒）   网络搜索
body           查询：<query>
               来源：url / title 列表（落定后）
               错误：<error_code>（error_code 非空时）
```

## 测试

**Rust**

| 文件 | 用例 |
| --- | --- |
| `core/agent/src/event_test.rs` | `WebSearch`/`WebSearchSource`/`AgentCitation` serde 往返；旧 `Text` JSON 无 `citations` 读出 `None` |
| 新建 `sdk/web_search_test.rs` | use+result 合并、错误 result、孤儿 result、未知 server 块、citations 提取、`replay_raw`/`replay_text` 的 `additional_params` 逐字节等价 |
| `sdk/loop_test.rs` | 开关开/关时 `additional_params.tools` 精确形状；`RunStarted.tools` 不含 `web_search`；脚本化 server 块产出 `WebSearch` + `Text.citations` 且不触发工具执行；`pause_turn` 续跑两次请求、续跑不吃重试预算（续跑后失败仍有完整重试）、上限 5 次、到顶正常收敛 |
| `sdk/resume_test.rs` | `WebSearch` 重建为 use→result 两条 raw `AssistantContent`；citations 回填；project→resume 往返语义不变 |
| `sdk/context_test.rs` | `sweep_orphans` 剔除 assistant 位孤儿 `web_search_tool_result`；成对块保留；空消息整条移除 |
| `sdk/normalize_test.rs` | **无新增用例**（`End` 仍丢弃；改动该文件即说明实现偏离决策 ⑦） |
| `runner_test.rs` / `lib_test.rs` | 协议门控（Openai + true → false）与 facade 传递 |
| `compose_test.rs` | anthropic/true、openai/true、false 三组映射 |
| store `model_test.rs` | V3→V4 升级默认 false、降级丢字段、V4 版本头、旧 JSON 缺字段；含新块的 `SessionEventRecord` JSON 往返 |
| `commands/agents/mod_test.rs` | 保存/读取 `web_search`；更新全部 `save_agent_provider` 调用点 |

**前端**

| 文件 | 用例 |
| --- | --- |
| `agent-adapter.test.ts` | `webSearch` → tool 部件两态；text 计划 `providerMetadata.agent.citations`；跨消息 use↔result 合并；无配对 use 只出查询词 |
| `agent-messages.test.tsx` | **空 parts 消息不渲染**（新增），既有气泡计数不受影响 |
| `tool-cards.test.tsx` | `Object.keys(DEFAULT_TOOL_CARDS)` 含 `web_search` |
| 新 `agent-blocks` 用例 | `WebSearchCard`（恒标题、查询词、来源、错误）、`CitationList`（url 去重）、`ExternalLink`（非 http(s) 不渲染为链接） |
| `provider-panel.test.tsx` | 仅 anthropic 可见、默认关、切协议清零、提交载荷 |
| `use-agent-providers.test.ts` | invoke 位置参数补位 |

**不做**：`agent-transport.test.ts` 无需因本变更改动（transport 零改动）。

## 验证

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

手工 E2E（真机 / 官方 Anthropic 端点）：

1. 新建 anthropic provider，打开开关保存 → 记录持久化，重开应用仍为开。
2. 发起需要最新信息的问题 → 请求带 `web_search_20250305`；时间线出现标题「网络搜索」的卡片（落定后有来源）；点击来源用外部浏览器打开。
3. 关闭开关再问 → 请求不带工具，时间线无搜索卡。
4. 带搜索的会话结束后「续轮」再追问 → 历史重建成功，API 不报 server tool 缺失，上下文连续。
5. 触发一次 `pause_turn`（较长搜索）→ 确认续跑发生、`pause_turn_continued` notice 出现、最终卡片完整。
6. 用一个不支持 server tools 的兼容网关 → 失败以既有 `api_error` 显式暴露，不静默重试。

## 风险与注意

- **回放保真**：`encrypted_content` 等字段不可推导，`raw_use`/`raw_result` 必须原样保存；展示字段（query/results/error）**不得参与重建**。citations 同理——展示字段可用 rig 的 `anthropic_citations()`（`completion.rs:591`），但 **raw 必须取 `additional_params["citations"]` 原值**（`Citation` 反序列化再序列化会规范化，如 `title: Option` → `null`）。
- **`pause_turn` 跨消息**：一次搜索可能拆成两条 assistant 消息（use 在前、result 在后）。回放顺序不乱；前端靠轮末全局配对合并卡片。
- **实时空白（已接受）**：无实时 delta，`pause_turn` 期间时间线可能空白数十秒；唯一过程观测点是 `SystemNotice.pause_turn_continued`。
- **空 parts 消息**：实时路 `pause_turn` 第二段只有孤儿 result → 0 部件 assistant 消息，渲染层过滤（决策 ⑨）。
- **连续 assistant 消息**：`pause_turn` 天然产生 `Assistant → Assistant`；rig 编码侧不做同角色合并，依赖 Anthropic 服务端合并。**真机验证点应前置到实现期**（而非留到发布前）。
- **网关兼容**：开关默认关；不支持的网关以 `api_error` 失败，不做无工具静默重试。
- **成本**：`max_uses = 5` 为成本护栏常量；可配置化需 store v5 迁移（当前不做）。
- **降级**：旧客户端读含 `webSearch` 块的事件会在**整条事件解码**时失败（不是少显示一个卡）。仓库不支持降级，需在变更说明中标注「升级后不可回退」。
- **链接安全**：只允许 `http(s)`，非 http(s) 不渲染为链接；`ExternalLink` 是唯一外链调用点。
- **转录体积**：`raw_result` 含 `encrypted_content` + 每条来源加密索引，一次搜索数 KB，JSON 不压缩。已决定不设上限、不压缩。

## 附录 A：rig 0.43.0 源码级复核

| 断言 | 证据 |
| --- | --- |
| provider tool → `additional_params.tools` → wire `tools` | `completion/request.rs:700-740`、`providers/anthropic/completion.rs:2255-2281` |
| `ProviderToolDefinition::new(kind).with_config(k,v)` = `{type:<kind>, k:v}` | `completion/request.rs:88-116` |
| 流式终局携带 `pause_turn` | `completion/request.rs:256`、`providers/anthropic/streaming/tests.rs:1417-1428`（→ `FinishReason::Other`） |
| raw server 块 = `Text{text:"", params.anthropic_content}`，回放要求空文本 | `providers/anthropic/streaming.rs:340-352, 410-432`、`completion.rs:610-651`（带 raw 又带文本 → `ConversionError`） |
| text 块回放带回 citations | `completion.rs:621-626` |
| 流式多条 citation 数组 concat 不覆盖 | `streaming.rs:306-312` + `operation/completion.rs:481-520`（注释写明 "arrays concatenate (streamed citation deltas)"） |
| 测试可脚本化 `pause_turn` / server 块 | `operation/completion.rs:90-104`（`Finish.reason` pub）、`test_utils/streaming.rs`（`FinalResponse(Finish)`、`TextStart{additional_params}`、`TextAdditionalParams`） |

**ai SDK 7.0.118 侧（决策 ①④）**

| 断言 | 证据 |
| --- | --- |
| text-start/delta/end 都接受 `providerMetadata` | `node_modules/ai/dist/index.d.ts`（`UIMessageChunk` 三变体各带可选 `providerMetadata`） |
| 命名空间自定，值须 object | `ProviderMetadata = Record<string, JSONObject>`（`@ai-sdk/provider/dist/index.d.ts:121`）；chunk schema `z.looseObject` + `z.record(z.string(), z.record(z.string(), jsonValue))`（`ai/dist/index.js:2489`），未知键不剥 |
| 合并语义 = **覆盖**非 merge | `ai/dist/index.js:7669-7695`：`= chunk.providerMetadata ?? existing`；`text-start` 初始化，`text-delta`/`text-end` 覆盖 |

> 推论：citations **放一次即可**，不要指望多次 delta 累加。`providerMetadata` 只活在 UI 运行时，本仓库从不把 UIMessage 反推成 prompt，无需关心 `convertToModelMessages` 的剥离。

## 附录 B：探索过程纪要

以下是收敛过程中被推翻或修正的早期判断，留档供 propose 阶段避坑。

| 早期判断 | 最终结论 |
| --- | --- |
| 实时「正在搜索…」+ `AgentDelta::Search` | **删除**：MVP 只保证最终可见；`normalize.rs` / `agent-transport.ts` 零改动的收益大于实时提示 |
| 独立 `data-citations` 部件 | **改为**挂 text 部件 `providerMetadata` |
| `providerMetadata` 命名空间用 `anthropic` | **改自有 `agent`**：消费点在 provider 外 |
| `Text.citations: Vec<_>`（必填） | **改 `Option<Vec<_>>`**：specta 生成可选字段，14 个测试文件 / 34 处 fixture 零改动 |
| 「`compact.rs` / `sweep_orphans` 保持不变」 | **不成立**：`pause_turn` 跨消息 + 裁剪 → 孤儿 server result → 回放被拒，`sweep_orphans` 必须扩展 |
| 超时降级为「仍在搜索…」 | **不做**（文案已改无状态） |
| 加 `RunStarted.serverTools` | **不加** |
| 版本 `0.5.24 → 0.5.25` | **改为 `0.5.26 → 0.5.27`** |
| `AgentBlock::Text` 约 39 处 | 实际 **29 处** |
| 自写 `open_external_url` command | **改用官方 `tauri-plugin-opener`** |
| `max_uses` 做成可配置（`Option<u32>`） | **不做**，锁为常量 `5` |
