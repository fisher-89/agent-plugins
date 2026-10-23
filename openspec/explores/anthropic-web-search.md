# Anthropic 协议启用云端 Web Search

workflow_type: requirement

## Context

桌面端 SDK 引擎（`packages/desktop/src-tauri/crates/infra/agent`）目前只向模型下发七个客户端工具，Anthropic 协议下没有服务端工具。用户希望：**provider 协议为 anthropic 时，可按 provider 开关启用 Anthropic 云端 `web_search`，并在会话时间线展示搜索活动与引用来源**。

rig 0.43.0 已具备两半能力，无需升级/打补丁：

- 请求侧：`rig::completion::ProviderToolDefinition` + `CompletionRequest::provider_tool(s)`，最终落到 `additional_params.tools`，由 Anthropic adapter 追加进 `tools` 数组（rig `providers/anthropic/completion.rs:2237-2281`）。
- 响应侧：`server_tool_use` / `web_search_tool_result` 被解码为 `AssistantContent::Text { text: "", additional_params: {"anthropic_content": <raw>} }`，citations 落在 `additional_params["citations"]`；回放时把 raw 放回 `additional_params` 即可原样重发。

真正的缺口都在本仓库：provider 开关、事件契约、持久化/续轮回放、`pause_turn` 续跑、时间线渲染。当前这些 server 块会被存成空 `Text`，且每次续轮都从转录重建历史，导致后续轮次丢失搜索上下文。

## 关键决策

- 开关按 provider，默认关；仅 `protocol === anthropic` 时展示并生效。
- 声明基础版：`{ "type": "web_search_20250305", "name": "web_search", "max_uses": 5 }`（兼容老模型与多数网关；`max_uses` 为常量，控制成本）。
- **server tool 绝不进入** `TOOL_NAMES`、`policy.rs`、`tools::definitions()`、`RunStarted.tools` 或 dispatch 白名单；只走 `additional_params.tools`。
- 搜索块以 raw provider JSON 作为回放唯一事实源；展示字段（query/results/error）只用于 UI，不参与重建。
- 时间线展示：搜索卡（标题恒为无状态文本「网络搜索」，body 含查询词 / 来源列表 / 错误，**无实时过程态**）+ 正文底部的引用列表。
- 来源链接可点击；只允许 `http(s)`，通过 `tauri-plugin-opener` 打开外部浏览器。

## 改动面

### 1. core 事件契约 — `crates/core/agent/src/event.rs`

1. `AgentBlock::Text` 增加字段（`Option` 形态：specta 生成可选字段，存量前端 fixture 零改动）：
   ```rust
   citations: Option<Vec<AgentCitation>> // #[serde(default, skip_serializing_if = "Option::is_none")]
   ```
2. 新增 `AgentCitation`：`url/title/cited_text: Option<String>` + `raw: serde_json::Value`（原样 wire 引用，回放用）。
3. 新增 `AgentBlock::WebSearch`（一次 server_tool_use 与其配对 result 收敛为一块）：
   ```rust
   WebSearch {
       id: String,            // server_tool_use.id == result.tool_use_id
       query: String,
       results: Vec<WebSearchSource>, // #[serde(default, skip_serializing_if = "Vec::is_empty")]
       error: Option<String>,         // web_search_tool_result_error.error_code
       raw_use: Option<serde_json::Value>,
       raw_result: Option<serde_json::Value>, // 保留 encrypted_content
   }
   ```
4. 新增 `WebSearchSource { url, title, page_age: Option<String> }`。
5. ~~`AgentDelta` 增加 `Search`~~ **不做**（见「决策落定（第五轮）」：MVP 只要最终可见，实时 delta 整条删除；`AgentDelta` 与 `normalize.rs` 均零改动）。
6. `crates/core/agent/src/lib.rs` 导出新类型；更新 `event_test.rs` 的 serde 往返/旧 JSON 兼容测试。
7. `crates/core/orchestration/src/transcript.rs` 的 `AgentBlock::Text` 模式改为 `{ text, .. }`。

> 事件 payload 是 `SerdeJsonCodec` 自描述 JSON，新增变体/带默认值的字段无需 store schema 版本升级；旧事件缺 `citations` 读出 `None`。`AgentBlock::Text` 在 Rust 侧命中 **29 处**（生产代码 `loop.rs` 两处、`resume.rs` 一处，其余为测试）。

### 2. 引擎：声明、投影、实时增量 — `crates/infra/agent/src/sdk/`

新建 `web_search.rs`（并注册到 `sdk/mod.rs`）：

- 常量：`WEB_SEARCH_TOOL_KIND = "web_search_20250305"`、`WEB_SEARCH_TOOL_NAME = "web_search"`、`WEB_SEARCH_MAX_USES = 5`、`ANTHROPIC_RAW_CONTENT_KEY = "anthropic_content"`、`CITATIONS_KEY = "citations"`。
- `provider_tool() -> ProviderToolDefinition`。
- `raw_content(&Text) -> Option<&Value>`、`citations_of(&Text) -> Vec<AgentCitation>`。
- `project_choice(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)`：
  - 先按 `tool_use_id` 索引 `web_search_tool_result`；
  - 顺序遍历：server_tool_use → 合并为 `WebSearch`；配对的 result 不重复产出；孤儿 result 产出 `raw_use = None` 的块；其他 raw server 块跳过；
  - 普通 Text → `AgentBlock::Text { text, citations }`；Reasoning/ToolCall/Image 逻辑保持现状；
  - result content 数组 → 取 `web_search_result` 的 url/title/page_age；错误对象 → `error_code`。
- `replay_raw(&Value) -> AssistantContent`：`Text { text: "", additional_params: {"anthropic_content": raw} }`（raw 块必须配空文本，rig 会校验）。
- `replay_text(text, citations) -> AssistantContent`：非空 citations → `additional_params.citations = raw 数组`。

`loop.rs`：

- `LoopTurn` 增加 `web_search: bool`。
- 请求构造后按需挂 provider tool：
  ```rust
  if turn.web_search { request = request.provider_tool(web_search::provider_tool()); }
  ```
  `tools: definitions.clone()` 不变。
- 用 `web_search::project_choice(&choice)` 替换 317-340 的内联转换；assistant 密封消息与 `history.push(Message::Assistant { .. })` 不变。
- `'attempt` 返回 `(choice, usage, finish_reason)`（`CompletionResponse::finish_reason()`）。
- `pause_turn` 续跑：`FinishReason::Other("pause_turn")` 且无客户端 `tool_calls` 时，追加 assistant 消息后直接发下一次请求，不做新用户消息；上限 `PAUSE_TURN_MAX_CONTINUATIONS = 5`，每次发 `SystemNotice.pause_turn_continued`，到顶发 `pause_turn_capped` 并正常收敛。空 choice 不续跑。

`normalize.rs`：**零改动**（不产实时搜索增量；`End` 仍丢弃）。

`compact.rs`：保持不变（摘要请求仍无工具）。

### 3. 续轮回放 — `crates/infra/agent/src/sdk/resume.rs`

- `user_content`：`Text { text, .. }`。
- `assistant_content` 改为可展开的循环（一个 `WebSearch` 最多产两条）：
  - `Text { text, citations }` → `replay_text(...)`；
  - `Thinking` / `ToolUse` 不变；
  - `WebSearch { raw_use, raw_result, .. }` → 依次 `replay_raw`，保持 use → result 原始顺序；两者皆空的块跳过；
  - assistant 位 `ToolResult` 仍跳过。

### 4. provider 开关链路

- **store** `crates/infra/store/src/model.rs`：
  - 当前 v3 记录改名为 `AgentProviderRecordV3`（字段/serde/`native_model(id=5, version=3, from=V2)` 原样）；
  - 新增当前 `AgentProviderRecord`：`native_model(id=5, version=4, from=AgentProviderRecordV3)`，增加 `#[serde(default)] web_search: bool`；
  - 补齐 V3→V4 双向 `From`；
  - `AgentProviderRecord::new(...)` 签名不变（默认 false），新增 `with_web_search(bool)`；手工 `Debug` 增加字段。
- **command** `src/commands/agents/mod.rs`：`save_agent_provider` 末尾追加 `web_search: bool`（第 9 参），构造时 `.with_web_search(web_search)`；不因 openai 拒收，生效门在解析期。
- **compose** `crates/infra/agent/src/compose.rs`：`ResolvedEngine` 增加 `web_search: bool`，赋值为 `provider.web_search && provider.protocol == AgentProviderProtocol::Anthropic`；`compose_turn` 链上 `.with_web_search(...)`。
- **facade/runner** `lib.rs` / `sdk/runner.rs`：仿 `with_max_tokens` 增加 `with_web_search`；`SdkRunner` 字段默认 false；`PumpPayload` → `LoopTurn` 传递；启动日志可加 `web_search=…`。

### 5. 前端

- 重新生成 `src/types/generated/bindings.ts`（不要手改）。
- provider 表单 `views/agents/components/provider-panel.tsx`：
  - `ProviderFormState.webSearch: boolean`；`ProviderTextField` 排除该键；
  - 仅 `protocol === 'anthropic'` 渲染 checkbox（`data-testid="provider-web-search"`），带提示「Anthropic 服务端网页搜索（按次计费）」；
  - 协议切走时清零；submit 传 `webSearch`；列表可加徽标。
- `views/agents/hooks/use-agent-providers.ts`：输入类型与 positional invoke 末位追加 `webSearch`。
- `src/lib/agent-adapter.ts`：
  - **不新增** `AgentDataParts` 成员；text 计划携带 `citations` 并挂 `providerMetadata: { agent: { citations } }`（空数组不挂）；
  - `BlockPlan` 复用 `tool` 承载搜索：`{ kind:'tool', toolCallId: id, toolName:'web_search', input:{query}, output:{results,error}|null }`；
  - `planBlock` 处理 `webSearch` 块与 `text.citations`；扩展 global/local 配对：按 `id` 把跨消息的孤儿 result 合并进 use（`pause_turn` 拆分场景）；local 视野下无配对 result 的 use 仍渲染（只显示查询词）；
  - `eventToChunk`：**零改动**（无搜索 delta）。
- `src/lib/agent-transport.ts`：**零改动**。
- `components/agent/agent-messages.tsx`：`useHistoryWindow` 的 `conversationMessages` 过滤条件追加 `message.parts.length > 0`（`pause_turn` 第二段只有孤儿 result 时实时路产出空 parts 消息，见「决策落定（第五轮）」）。
- 渲染：
  - `components/agent/agent-blocks.tsx` 新增 `WebSearchCard`（summary 恒「网络搜索」；body：查询词、来源列表、错误）、`CitationList`（正文底部、按 url 去重）、`ExternalLink`（校验 http/https）；
  - `components/agent/tool-cards.tsx` 的 `DEFAULT_TOOL_CARDS` 注册 `web_search` → `WebSearchCard`（同步更新 `tool-cards.test.tsx:89` 的键集合断言）。
- 外链能力：**用官方插件**（已裁定优先插件，不自写 command）：
  - `tauri-plugin-opener = "2"` 加入 workspace 依赖与 `src-tauri/Cargo.toml` 根依赖；
  - `src-tauri/src/main.rs` 的 builder 链加 `.plugin(tauri_plugin_opener::init())`（与 `dialog` 同位）；
  - `capabilities/default.json` 增加 `opener:allow-open-url`，scope 只放 `http://*` / `https://*`（**标识符与 scope 形状实现期核对**：也可能是 `opener:allow-default-urls`，后者含 mailto/tel，需确认能否收窄）；
  - 前端 `pnpm -C packages/desktop add @tauri-apps/plugin-opener`（当前 `node_modules/@tauri-apps/` 只有 api / cli / plugin-dialog / plugin-updater）；
  - 调用点收敛到单个 `ExternalLink` 组件，非 http(s) 直接不渲染为链接。

### 6. 版本

`packages/desktop/package.json` `0.5.26` → `0.5.27`（用户可见功能变更）。

## 测试

- `core/agent/src/event_test.rs`：`WebSearch`/`WebSearchSource`/`AgentCitation` serde 往返；旧 `Text` JSON 无 `citations` 读出 `None`；`AgentBlock` 未知变体与新变体的前后兼容。
- 新建 `sdk/web_search_test.rs`：use+result 合并、错误 result、孤儿 result、未知 server 块、citations 提取、`replay_raw`/`replay_text` 的 `additional_params` 逐字节等价。
- `sdk/normalize_test.rs`：**无新增用例**（End 仍丢弃；若改动该文件即说明实现偏离决策）。
- `sdk/loop_test.rs`：开关开/关时请求 `additional_params.tools` 精确形状、`RunStarted.tools` 不含 `web_search`；带 server 块的脚本化流产出 `WebSearch` + `Text.citations` 且不触发工具执行；`pause_turn` 续跑两次请求、上限 5 次、正常收敛。
- `sdk/resume_test.rs`：`WebSearch` 重建为 use→result 两条 raw `AssistantContent`；citations 回填；project→resume 往返语义不变。
- `runner_test.rs` / `lib_test.rs`：协议门控（Openai + true → false）与 facade 传递。
- `compose_test.rs`：anthropic/true、openai/true、false 三组映射。
- store `model_test.rs`：V3→V4 升级默认 false、降级丢字段、V4 版本头、旧 JSON 缺字段；`SessionEventRecord` 含新块的 JSON 往返。
- `commands/agents/mod_test.rs`：保存/读取 `web_search`；更新全部 `save_agent_provider` 调用点。
- 前端：
  - `agent-adapter.test.ts`：`webSearch` → tool 部件（`input-available`/`output-available`）、text 计划的 `providerMetadata.agent.citations`、跨消息 use↔result 配对合并、无配对 use 只出查询词；
  - `provider-panel.test.tsx`：仅 anthropic 可见、默认关、切协议清零、提交载荷；
  - `use-agent-providers.test.ts`：invoke 位置参数补位；
  - `agent-messages.test.tsx`：**空 parts 消息不渲染**（新增用例，且既有气泡计数不受影响）；
  - `tool-cards.test.tsx`：`Object.keys(DEFAULT_TOOL_CARDS)` 加入 `web_search`；
  - 新 `agent-blocks` 用例：`WebSearchCard`（恒「网络搜索」标题、查询词、来源列表、错误）、`CitationList`（按 url 去重）、`ExternalLink`（非 http(s) 不渲染为链接）。
- **不做**：`agent-transport.test.ts` 无需因本变更改动（transport 零改动）。

## 验证

```powershell
# 精确到层
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p store
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p dev-team
cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml

pnpm -C packages/desktop run bindings:export
pnpm -C packages/desktop run bindings:check
pnpm -C packages/desktop run client:test
pnpm -C packages/desktop run check
pnpm -C packages/desktop test   # 全量 Rust + 前端
```

手工 E2E（真机/官方 Anthropic 端点）：

1. 新建 anthropic provider，打开 web search 开关，保存后确认记录持久化、重开应用仍为开。
2. 发起需要最新信息的问题，确认请求带 `web_search_20250305`，时间线出现标题为「网络搜索」的卡片（先只有查询词，落定后有来源）；点击来源用外部浏览器打开。
3. 关闭开关再问，确认请求不带工具、时间线无搜索卡。
4. 带搜索的会话结束后点「续轮」再追问，确认历史重建成功、API 不报 server_tool 缺失、上下文连续。
5. 用一个不支持 server tools 的兼容网关验证：失败以既有 `api_error` 显式暴露，不静默重试。

## 风险与注意

- **回放保真**：`encrypted_content` 等字段不可推导，`raw_use`/`raw_result` 必须原样保存，展示字段不得参与重建。
- **pause_turn 跨消息**：搜索可能被拆成两条 assistant 消息（use 在前、result 在后）；回放顺序不乱，前端靠轮末全局配对合并卡片。
- **实时空白（已接受）**：无实时 delta，`pause_turn` 期间时间线可能空白数十秒；唯一过程观测点是 `SystemNotice.pause_turn_continued`。
- **空 parts 消息**：实时路 `pause_turn` 第二段只有孤儿 result，会产出一条 0 部件 assistant 消息——渲染层过滤 `parts.length > 0`（见决策 ⑨）。
- **连续 assistant 消息**：Anthropic 会合并同角色连续消息；若真机验证发现拒绝，再在 `rebuild` 中合并相邻 assistant。
- **网关兼容**：开关默认关；不支持的网关以 api_error 失败，不做无工具静默重试。
- **成本**：`max_uses = 5` 为成本护栏，发布前可按产品意见调整常量。
- **降级**：旧版本桌面无法解码含新字段/新变体的 JSON；仓库不支持降级，需在变更说明中标注。
- **链接安全**：只允许 http(s)，点击前 `preventDefault()`，禁止 `javascript:`/`file:`。

## 复核（rig 0.43.0 源码级）

上述方案的关键前提逐条核对过，成立；同时发现几处需要修正与讨论。

**成立的前提**

| 断言 | 证据（rig-core-0.43.0 / 本仓库） |
| --- | --- |
| provider tool 落到 `additional_params.tools`，再追加进 wire `tools` | `completion/request.rs:700-740`（`merge_provider_tools_into_additional_params` 构造 `{type, …config}`）、`providers/anthropic/completion.rs:2255-2281`（`build_tool_definitions` append） |
| `ProviderToolDefinition::new(kind).with_config(k,v)` 即 `{type:<kind>, k:v}` | `completion/request.rs:88-116` |
| 流式终局携带 `pause_turn` | `completion/request.rs:256`（`CompletionResponse::finish_reason`）、`providers/anthropic/streaming/tests.rs:1417-1428`（`pause_turn → FinishReason::Other`） |
| raw server 块以 `Text { text:"", additional_params.anthropic_content }` 承载，回放要求空文本 | `providers/anthropic/streaming.rs:340-352, 410-432`（`ServerToolUse` / `WebSearchToolResult` 走 `out.text()` + `text_params`）、`completion.rs:610-651`（回放：带 raw 时拒绝非空 text） |
| text 块回放会带回 citations | `completion.rs:621-626`（`Content::Text { text, citations }`） |
| citations 流式逐条到达并被数组 concat 累积（不会互相覆盖） | `streaming.rs:306-312`（`CitationsDelta`）+ `operation/completion.rs:481-520`（`AdditionalParams::merge` 注明 "arrays concatenate (streamed citation deltas)"） |
| 测试可脚本化 `pause_turn` | `operation/completion.rs:90-104`（`Finish.reason` pub）+ `MockStreamEvent::FinalResponse(Finish)` |
| 测试可伪造 server 块（无需真实 wire 帧） | `MockStreamEvent::TextStart { additional_params }` / `TextAdditionalParams` → `out.text_params` merge；空文本带 params 的 part 不会被丢弃 |

**需要修正的事实**

1. **版本号已过期**：`packages/desktop/package.json` 当前是 `0.5.26`，不是 `0.5.24` → 改为 `0.5.27`。
2. **改动面计数偏差**：`AgentBlock::Text` 在 Rust 侧命中 **29 处**（非 39）；真正的成本在前端——`{ kind: 'text', text }` 字面量在 `*.test.ts(x)` 里约 20+ 处，而 specta 对 `Vec<AgentCitation>` 生成的是**必填**字段（现状 `bindings.ts:274` 是 `{ kind: "text"; text: string }`），新增字段后这些 fixture 会集体编译失败。建议：前端加一个 `textBlock(text, citations = [])` fixture helper 收敛，或评估 `Option<Vec<_>>`（生成 `citations?: …`，旧代码零改动），再决定。
3. **`sweep_orphans` 遗漏（实质缺口）**：`sdk/context.rs:230+` 的孤儿清扫只认 `UserContent::ToolResult` ↔ `CallId`。`pause_turn` 会把同一次搜索拆成两条 assistant 消息（use 在前、result 在后），L3 摘要按 `history[1..protected]` 整段替换、L2 按完整轮对裁剪，边界一旦落在两条 assistant 之间，就会留下**没有 server_tool_use 的 web_search_tool_result** → 回放被 Anthropic 拒。方案里「compact 保持不变」不成立：需要让 `sweep_orphans` 感知 assistant 位的 server 块配对（从 raw 的 `id` / `tool_use_id` 建索引，剔除孤儿 result 文本块）。
4. **rig 有现成解析 API，不必手写**：`anthropic_citations(&message::Text) -> Vec<Citation>`（`completion.rs:591`）已把 `web_search_result_location` 解成 `{ cited_text, url, title, encrypted_index }`。但**展示字段用它是安全的，raw 必须直接取 `additional_params["citations"]` 的原值**——`Citation` 反序列化再序列化会补齐/规范化字段（如 `title: Option` 序列化为 `null`），逐字节保真只有原始 JSON 给得起。与「raw 是回放唯一事实源」的既有决策一致，只是实现路径要写明。
5. **外链打开可更轻**：`capabilities/default.json` 现在只有 `core:default` + `dialog:allow-open`。引入 `tauri-plugin-opener` 要加依赖 + 注册 + 新权限面。替代：写一个 `open_external_url` command，Rust 侧强校验 `http(s)` 后用系统 opener（`open` crate / `std::process`），前端 `invoke` 即可，权限面不扩、校验点在可信侧。二者都可，但要选一个并写清理由。

**值得讨论（产品/契约层面）**

- **引用的呈现形态**：方案是「正文之后的引用列表」+ `data-citations` 独立部件。另一条路是用 ai SDK text part 的 `providerMetadata` 把 citations 挂在**正文部件自己**上，排版与行内角标都好做，部件数不膨胀。代价是密封前拿不到（citations 只在 `End`/密封时确定）。
- ~~**搜索卡实时性**：…（超时降级）~~ **已决：不做降级，文案改无状态「网络搜索」**（见「决策落定 ③」）。`server_tool_use` 的 `End` 帧在 `content_block_stop` 才发（`streaming.rs:410-432`），其 `query` 是 `web_search_tool_result` 到达前**唯一**可见信息。
- **`RunStarted.tools`**：方案规定 server tool 不进 `tools`（正确），但前端「本轮可用工具」面板就完全看不到它。是否加一个 `serverTools: Vec<String>`（旧事件缺字段读出空 vec，成本低）？
- **pause_turn 与重试/用量**：续跑不应计入 `retry.max_attempts`（否则两次 pause_turn 就吃掉重试预算），但每次续跑都要 `usage += turn_usage`；`PAUSE_TURN_MAX_CONTINUATIONS = 5` 与 `max_uses = 5` 是两个不同概念，命名要区分，且到顶后是「正常收敛」还是「`api_error`」需明说。方案目前是前者，合理，但要在 SystemNotice 里写清到顶原因。
- **连续 assistant 消息**：`pause_turn` 天然产生 `Assistant → Assistant`。rig 编码侧不做同角色合并（`completion.rs` 的 message 转换逐条映射），依赖 Anthropic 服务端合并。本仓库不支持降级/多网关，风险可接受，但真机验证点应前置到实现期（而不是留到发布前）。
- **转录体积**：`raw_result` 含 `encrypted_content` + 每条来源的加密索引，一次搜索动辄数 KB；`ServerEventRecord` 是 JSON 自描述、无压缩，10 次搜索的会话可能多出几十 KB。是否接受（`db-inspector` / 导出功能会看到）？
- **旧版本降级窗口**：`SessionEventRecord` 用 `SerdeJsonCodec`，旧客户端读到含 `webSearch` 块的事件会在**整条事件解码**时失败，不是"少显示一个卡"，而是可能整会话加载报错。仓库不支持降级（既有约定），但这次要明确写进变更说明：升级后不可回退。
- **`max_uses = 5`**：成本护栏常量。要不要做成 provider 级可配置（表单再加一个输入）？若做，`web_search: bool` 就该是 `web_search: Option<u32>`（None = 关），一次决策省掉以后第二次改 store 版本。

## 数据结构往返（一图）

```
Anthropic SSE                rig 解码                     本仓库
─────────────────────────────────────────────────────────────────────────────
server_tool_use        ──▶  Text{text:"", params:        ──▶  AgentBlock::WebSearch{
  {id, name, input}          anthropic_content}                id, query, raw_use
                                                        (End 帧 → AgentDelta::Search)
web_search_tool_result ──▶  Text{text:"", params:        ──▶  同一个块 += results / error
  {tool_use_id, content[]}   anthropic_content}                + raw_result
text + citations_delta ──▶  Text{text, params.citations} ──▶  AgentBlock::Text{text, citations}
─────────────────────────────────────────────────────────────────────────────
store（JSON 自描述事件）
─────────────────────────────────────────────────────────────────────────────
resume 重建 ──▶  Text{text:"", params.anthropic_content: raw_use}   ← replay_raw
                 Text{text:"", params.anthropic_content: raw_result} ← replay_raw
                 Text{text,     params.citations: raw[]}             ← replay_text
```

> 以下为第四轮决策记录；上文「关键决策」「5. 前端」「风险与注意」中与之冲突的措辞以本节为准。

## 决策落定

### ① citations 挂正文部件（无独立 `data-citations`）

- `AgentDataParts` **不**新增 `'citations'`；text 部件多带一个 `providerMetadata`。
- `BlockPlan` text 计划携带 `citations`；`plansToParts` 产出
  `{ type:'text', text, state:'done', providerMetadata: { agent: { citations } } }`（空数组不挂）。
- 实时路 `textChunks` 的 `text-start` 带同一 metadata。
- `PartView` text 分支 → `<TextBlock text={part.text} citations={readCitations(part)} />`。

ai 7.0.118 侧三条约束已核实：

| 点 | 证据 |
| --- | --- |
| text-start/delta/end 都接受 `providerMetadata` | `node_modules/ai/dist/index.d.ts`（`UIMessageChunk` 三变体各带可选 `providerMetadata`） |
| 命名空间自定，值须 object | `ProviderMetadata = Record<string, JSONObject>`（`@ai-sdk/provider/dist/index.d.ts:121`）；chunk schema `z.looseObject` + `z.record(z.string(), z.record(z.string(), jsonValue))`（`ai/dist/index.js:2489`），未知键不剥 |
| 合并语义 = **覆盖**非 merge | `ai/dist/index.js:7669-7695`：`= chunk.providerMetadata ?? existing`；`text-start` 初始化，`text-delta`/`text-end` 覆盖 |

推论：**citations 放一次即可**，不要指望多次 delta 累加。

### ② `web_search: bool` → store v4 定型，`new` 签名不动

- `#[serde(default)] pub web_search: bool`；`AgentProviderRecord::new` **保持 8 参**（默认 false）+ `with_web_search(bool)`——`new` 全仓 **35 处**调用，改签名 = 35 处机械改动，加构造器 = 0 处。
- `save_agent_provider` 是 command，specta 生成**位置参数**（`bindings.ts:79` 末端 `maxTokens`），追加 `webSearch` 后前端调用点全部补位。
- `max_uses = 5` 锁为常量；将来可配置需 v5 迁移——现在接受。

顺带收敛：**`AgentBlock::Text.citations` 用 `Option<Vec<AgentCitation>>`**。现状 `bindings.ts:274` 是 `{kind:"text"; text:string}`，若 `citations` 必填，则 **14 个测试文件 / 34 处** `kind:'text'` 字面量集体编译失败；`Option<Vec<_>>` 经 specta 生成 `citations?: AgentCitation[] | null`（与 `contextLength?: number | null` 同式，`bindings.ts:436`），**零 fixture 改动**。`WebSearch.results` 是新类型无存量 fixture，可裸 `Vec` + `#[serde(default, skip_serializing_if)]`。

### ③ 搜索卡文案改为无状态「网络搜索」

```
summary（恒）   网络搜索
body          查询：<query>
              来源：url / title 列表（落定后）
              错误：<error_code>（error_code 非空时）
```

- 删除「正在搜索…」文案；**删除超时降级分支**（原方案该条整条作废）。
- 卡片与 `ToolPairCard` 同构（`details/summary`），`data-testid="block-web-search"`。

**pause_turn 的既定语义（必须写进 spec，否则会被当 bug）**：`pause_turn` 把一次搜索拆成两条 assistant 消息，实时路 `PairingScope.local` 只看当前消息——

- 第一段密封块：卡片有 `query`、无 `results`（未落定）；
- 第二段密封块：只有孤儿 result → 实时路 **return null**，靠轮末重放（`global` 配对）合并。

即**实时期间卡片停在未落定态直到轮末折叠**。要实时出结果只能让 Rust 延迟发送第一段 sealed Message——牵动事件顺序与持久化，不建议。

### ④ citations 命名空间 → 自有命名 `agent`

判定依据（「provider 外消费即自有命名」）：citations 唯一消费点在 UI（`TextBlock` / 底部列表），属 provider 外；且 `AgentBlock::Text.citations` 已在 Rust 侧归一化为协议中立结构。

- 仓库无 `@ai-sdk/anthropic` 依赖，不存在与官方 provider id 的键冲突。
- **`providerMetadata` 不落库、不进事件、不进 wire**——原始数据是 `AgentBlock::Text.citations`，metadata 由前端 adapter 现算。故命名**不是持久化契约，改名字成本≈0**。

### ⑤ 引用形态 → 正文底部列表

- `TextBlock` markdown 渲染不变，正文下方追加 `<CitationList />`。
- 列表项：`title ?? url`；链接走外链校验（只 `http(s)`）。
- **按 url 去重**（同页可被引多句；`cited_text` 保留首个）。
- 不做行内角标、不做原句高亮（`cited_text` 已在 raw 内，属后续打磨）。

### ⑥ 不加 `serverTools`

`RunStarted` 契约不变；「本轮可用工具」面板看不见 server tool——接受。

## 决策落定（第五轮）

用户裁定：**① MVP 只要最终可见，过程允许空白（选 (iii)：删实时 delta）；② `sweep_orphans` 扩展记入 tasks；③ 空 parts 顺手不渲染；④ 外链优先用官方 opener 插件。**

### ⑦ 删实时 delta —— 采纳 (iii)

未选项：(i) `data-web-search` 部件、(ii) `tool-web_search` 部件（均可保留「查询词提前可见」，代价是 provisional 分支与配对扩展）。

取舍理由：`pause_turn` 拆消息 + 本地视野配对使实时态本就无法完整（第二段只能靠轮末 global 配对合并），dedicated 实时通道的收益被结构性削平；MVP 以「最终可见」为目标，实时空白是显式接受。

**连带删除面（都是「不做」而非「改」）**：

| 原方案项 | 处置 |
| --- | --- |
| `AgentDelta::Search` 变体 | **删**，`AgentDelta` 保持两变体 |
| `web_search::stream_search_delta` | **删** |
| `normalize.rs` 的 `End` 分支 | **零改动**（`End` 仍丢弃） |
| `agent-transport.ts` provisional 搜索开件 | **零改动** |
| 搜索卡「正在搜索…」/ 超时降级 | 已删（第四轮） |

保留项：`AgentBlock::WebSearch`（密封可见的唯一来源）、`SystemNotice.pause_turn_continued`（续跑留痕，仍是唯一的过程可观测点）。

**可观测性补偿**：`pause_turn` 期间时间线可能空白数十秒，用户在既有「运行中…」标记之外无信息。已接受；保留 `pause_turn_continued` notice 供调试面与后续体检。

### ⑧ 部件承载最终形态 —— tool 部件（(ii) 的「无 delta 版」）

删除 delta ≠ 必须换承载方式。仍用 tool 部件，因为 `planToolPart` / `plansToChunks` / `DEFAULT_TOOL_CARDS` 全部现成：

```
AgentBlock::WebSearch ──planBlock──▶ { kind:'tool', toolCallId:id, toolName:'web_search',
                                       input:{query}, output:{results,error}|null }

实时（本地视野）：use 无配对 result → output:null → tool-input-available → 卡片只显查询词
轮末重放（全局视野）：孤儿 result 并入 use → output 有值 → 卡片显来源列表
```

语义代价：server tool 与客户端工具共用卡片视觉与注册表。可接受（`RunStarted.tools` 本就不含它，前端不做交叉校验）。

### ⑨ 空 parts 不渲染 —— 记入实现面

- 落点：`components/agent/agent-messages.tsx` 的 `useHistoryWindow`，`conversationMessages` 过滤追加 `message.parts.length > 0`；
- 理由：`pause_turn` 第二段（只有孤儿 server result）实时路产出一条 0 部件的 assistant 消息；放在渲染层过滤可同时覆盖「实时 chunk 路」与「重放 adapter 路」，且分页计数一并修正；
- 顺带收益：任何未来「无可见部件的事件消息」都不会再冒出空气泡。

### ⑩ 外链用官方 opener 插件

- Rust：`tauri-plugin-opener = "2"`（workspace + root）、`main.rs` builder 链 `.plugin(tauri_plugin_opener::init())`；
- 权限：`capabilities/default.json` 加 `opener:allow-open-url`，scope 限定 `http://*` / `https://*`（**实现期核对标识符与 scope 形状**；若只有 `opener:allow-default-urls` 可用，需确认它是否连带放开 `mailto:` / `tel:` —— 若是则改回显式 `allow-open-url` + scope）；
- 前端：新增依赖 `@tauri-apps/plugin-opener`（当前 `node_modules/@tauri-apps/` 仅 api / cli / plugin-dialog / plugin-updater）；
- 校验仍在可信侧：仅 `http(s)` 才渲染为链接，`ExternalLink` 是唯一调用点。

### ⑪ `sweep_orphans` 扩展 → 记入 tasks

`crates/infra/agent/src/sdk/context.rs` 的 `sweep_orphans` 从「只认 `UserContent::ToolResult ↔ CallId`」扩展为「同时认 assistant 位 `web_search_tool_result` ↔ `server_tool_use`」：

- 索引源：`AgentBlock::WebSearch` 的 `id`（use 侧）与 `raw_result.tool_use_id`（result 侧）；
- 清扫对象：assistant 位中 `raw_use == None` 且其 id 在前序中找不到 use 的块，整块剔除；剔除后若该 assistant 消息的 `content` 变空，整条消息移除；
- 触发点不变（L2 剪裁后 / L3 硬裁后 / 摘要置换后）——三处都调 `sweep_orphans`，扩展后自动生效。

## 仍未拍板（仅剩一项）

1. **`pause_turn` 续跑与重试/用量口径**：续跑 MUST NOT 计入 `retry.max_attempts`（否则两次 `pause_turn` 就吃掉重试预算），但每次续跑 `usage += turn_usage`；`PAUSE_TURN_MAX_CONTINUATIONS = 5` 与 `max_uses = 5` 是两个不同概念，命名须区分；到顶后按「正常收敛」处理，并在 `SystemNotice.pause_turn_capped` 写明原因。请确认这三条。
2. （非阻塞，可留给 propose 阶段）转录体积：`raw_result` 含 `encrypted_content`，一次搜索数 KB、JSON 不压缩——不设上限也不压缩，接受。
