# Anthropic 协议启用云端 Web Search

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
- 时间线展示：实时「正在搜索…」增量 + 落定后的搜索卡（查询词、来源列表、错误）+ 正文引用列表。
- 来源链接可点击；只允许 `http(s)`，通过 `tauri-plugin-opener` 打开外部浏览器。

## 改动面

### 1. core 事件契约 — `crates/core/agent/src/event.rs`

1. `AgentBlock::Text` 增加字段：
   ```rust
   citations: Vec<AgentCitation> // #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
5. `AgentDelta` 增加 `Search { id: String, query: String }`（ephemeral，仅实时提示）。
6. `crates/core/agent/src/lib.rs` 导出新类型；更新 `event_test.rs` 的 serde 往返/旧 JSON 兼容测试。
7. `crates/core/orchestration/src/transcript.rs` 的 `AgentBlock::Text` 模式改为 `{ text, .. }`。

> 事件 payload 是 `SerdeJsonCodec` 自描述 JSON，新增变体/带默认值的字段无需 store schema 版本升级；旧事件缺 `citations` 读出空 vec。加字段会带来约 39 处 `AgentBlock::Text` 构造/模式的机械修改（生产代码仅 `loop.rs` 两处、`resume.rs` 一处）。

### 2. 引擎：声明、投影、实时增量 — `crates/infra/agent/src/sdk/`

新建 `web_search.rs`（并注册到 `sdk/mod.rs`）：

- 常量：`WEB_SEARCH_TOOL_KIND = "web_search_20250305"`、`WEB_SEARCH_TOOL_NAME = "web_search"`、`WEB_SEARCH_MAX_USES = 5`、`ANTHROPIC_RAW_CONTENT_KEY = "anthropic_content"`、`CITATIONS_KEY = "citations"`。
- `provider_tool() -> ProviderToolDefinition`。
- `raw_content(&Text) -> Option<&Value>`、`citations_of(&Text) -> Vec<AgentCitation>`。
- `stream_search_delta(&AssistantContent) -> Option<AgentDelta::Search>`：`End` 携带的 raw `server_tool_use` 且 `name == "web_search"` 时取 `input.query`。
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

`normalize.rs`：`StreamEvent::End { content, .. }` → `web_search::stream_search_delta(content).map(message_delta)`；`Start/Arguments` 仍丢弃。

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
  - `AgentDataParts` 增加 `'web-search'` / `'citations'`；
  - `BlockPlan` 增加 `search`，text 计划携带 citations；
  - `planBlock` 处理 `webSearch` / `text.citations`；扩展 global/local 配对：按 `id` 把跨消息的孤儿 result 合并进 use（`pause_turn` 拆分场景）；
  - `plansToParts` / `plansToChunks` 产出 `data-web-search` / `data-citations`；
  - `eventToChunk` 处理 `AgentDelta::Search`（`status: 'searching'`，稳定 id）。
- `src/lib/agent-transport.ts`：search delta 首次出现时只开 provisional 消息（`start` + `reset-step`，不发 text/reasoning part-start）。
- 渲染：
  - `components/agent/agent-blocks.tsx` 新增 `WebSearchCard`（折叠：查询词、正在搜索、错误、来源列表）、`CitationList`、`ExternalLink`（校验 http/https）；
  - `components/agent/agent-messages.tsx` 的 `PartView` 分发 `data-web-search` / `data-citations`。
- 外链能力（当前仓库没有 opener 插件）：
  - `tauri-plugin-opener = "2"` 加入 workspace 与 root 依赖；`main.rs` 注册 `tauri_plugin_opener::init()`；
  - `capabilities/default.json` 增加 `opener:allow-open-url`（限定 `http://*` / `https://*`）；
  - 前端安装 `@tauri-apps/plugin-opener`。

### 6. 版本

`packages/desktop/package.json` `0.5.24` → `0.5.25`（用户可见功能变更）。

## 测试

- `core/agent/src/event_test.rs`：`WebSearch`/`AgentCitation`/`AgentDelta::Search` serde 往返；旧 `Text` JSON 无 citations 兼容。
- 新建 `sdk/web_search_test.rs`：use+result 合并、错误 result、孤儿 result、未知 server 块、citations 提取、`replay_raw`/`replay_text` 的 `additional_params` 逐字节等价。
- `sdk/normalize_test.rs`：server_tool_use End → `Search` delta；普通 text/result End → None。
- `sdk/loop_test.rs`：开关开/关时请求 `additional_params.tools` 精确形状、`RunStarted.tools` 不含 `web_search`；带 server 块的脚本化流产出 `WebSearch` + `Text.citations` 且不触发工具执行；`pause_turn` 续跑两次请求、上限 5 次、正常收敛。
- `sdk/resume_test.rs`：`WebSearch` 重建为 use→result 两条 raw `AssistantContent`；citations 回填；project→resume 往返语义不变。
- `runner_test.rs` / `lib_test.rs`：协议门控（Openai + true → false）与 facade 传递。
- `compose_test.rs`：anthropic/true、openai/true、false 三组映射。
- store `model_test.rs`：V3→V4 升级默认 false、降级丢字段、V4 版本头、旧 JSON 缺字段；`SessionEventRecord` 含新块的 JSON 往返。
- `commands/agents/mod_test.rs`：保存/读取 `web_search`；更新全部 `save_agent_provider` 调用点。
- 前端：`agent-adapter.test.ts`（部件/chunk/delta/跨消息配对）、`agent-transport.test.ts`（provisional 开件）、`provider-panel.test.tsx`（仅 anthropic 可见、默认关、切协议清零、提交载荷）、`use-agent-providers.test.ts`（invoke 参数）、`agent-messages`/新 `agent-blocks` 测试（卡片、引用、外链拦截非 http(s)）。

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
2. 发起需要最新信息的问题，确认请求带 `web_search_20250305`，时间线先出现「正在搜索…」，随后出现搜索卡与来源；点击来源用外部浏览器打开。
3. 关闭开关再问，确认请求不带工具、时间线无搜索卡。
4. 带搜索的会话结束后点「续轮」再追问，确认历史重建成功、API 不报 server_tool 缺失、上下文连续。
5. 用一个不支持 server tools 的兼容网关验证：失败以既有 `api_error` 显式暴露，不静默重试。

## 风险与注意

- **回放保真**：`encrypted_content` 等字段不可推导，`raw_use`/`raw_result` 必须原样保存，展示字段不得参与重建。
- **pause_turn 跨消息**：搜索可能被拆成两条 assistant 消息（use 在前、result 在后）；回放顺序不乱，前端用跨消息配对合并卡片。
- **连续 assistant 消息**：Anthropic 会合并同角色连续消息；若真机验证发现拒绝，再在 `rebuild` 中合并相邻 assistant。
- **网关兼容**：开关默认关；不支持的网关以 api_error 失败，不做无工具静默重试。
- **成本**：`max_uses = 5` 为成本护栏，发布前可按产品意见调整常量。
- **降级**：旧版本桌面无法解码含新字段/新变体的 JSON；仓库不支持降级，需在变更说明中标注。
- **链接安全**：只允许 http(s)，点击前 `preventDefault()`，禁止 `javascript:`/`file:`。
