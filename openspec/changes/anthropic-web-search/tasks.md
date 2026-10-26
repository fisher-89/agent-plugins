# 任务: anthropic-web-search

> 依据 `design.md`（D1–D19，含「构建顺序约束」与「验收面口径」）与 `proposal.md` AC-1..AC-13。任务按依赖排序：前置任务解锁后续任务；每项落定后其后续项方可开工。
> 测试编写 / 单元测试 / 集成测试任务不在本清单（由测试相位承接）；`static-check` / `test-execution` 等机械脚本由桌面端执行，本清单不重复。
>
> **五条跨阶段依赖不变量（顺序硬约束）**：
> 1. **编译门单点后移到开关链路补齐之后（阶段 5 末尾）**：两处不编译成因——①`AgentBlock::Text` 增 `citations` 后，`sdk/loop.rs`（字面量 `:137` + 内联转换 `:319`）与 `sdk/resume.rs`（`:84` / `:95` 模式）的 `Text { text }` 单字段形态失效（阶段 1 引入、阶段 3 / 阶段 4 补位）；②`sdk/loop.rs` 增 `LoopTurn.web_search`（阶段 3）后，`sdk/runner.rs:316-329` 的 `LoopTurn { … }` 字面量缺字段（该构造点由阶段 5 的 `session_pump` 传递补齐）。故「`cargo check` 全绿」**不得早于阶段 5 末尾**：阶段 4 末只做源码级残留扫描（不编译），阶段 5 末执行唯一编译门；**阶段 1–5 期间工作树预期不编译**，阶段 1 不设编译门。
> 2. **`@tauri-apps/plugin-opener` 依赖先于引用**：`package.json` 依赖加入、安装与 `pnpm-lock.yaml` 刷新在**阶段 7**（前端呈现层之前），否则阶段 9 的 `ExternalLink` 无法 import `openUrl`。壳侧 `tauri-plugin-opener`（Rust）仍留在阶段 11。
> 3. **版本号抬升排在功能落地之后**：`packages/desktop/package.json` 的 `version → 0.5.29` 落在**阶段 12**（静态检查链由桌面端随后执行），与阶段 7 的依赖落地分开，避免同文件两处改动互相牵连。
> 4. **无前置真机门**：本清单 MUST NOT 含任何真机核验前置项或阻塞项。`pause_turn` 的「相邻同角色 assistant 消息是否被 Anthropic 服务端接受」按 `proposal.md`「回溯修订（第二轮）」与 `desktop-agent-execution` 边界条 16 记作**已知限制**，确认动作并入**归档后人工验收**（官方端点 + 凭据，结论记入验收记录），不阻塞实现与归档。原「阶段 0 · 前置核验（真机门）」已按该口径整段删除，**阶段编号沿用原稿（阶段 1 起）不重排**，以便与 `design.md`「构建顺序约束（D19）」的阶段引用一一对应。
> 5. **第四稿回溯：golden 契约优先于线格式紧凑**：`AgentBlock` 的 `skip_serializing_if` MUST NOT 回流（`specta-serde validate.rs:544` 硬拒 → 相位分裂 → `src/bindings/mod_test.rs` 四条断言必红）。缺省键恒在（`null` / `[]`）是定稿线语义；若为此改 golden 或改 `Channel<AgentRunMessage>` 类型名，等于 weaken 仓库级守卫，一律拒。落地见**阶段 13**。

---

## 阶段 1 · core 事件契约（解锁引擎与前端全部下游）

- [x] `crates/core/agent/src/event.rs`：`AgentBlock::Text` 增 `citations: Option<Vec<AgentCitation>>`（`#[serde(default, skip_serializing_if = "Option::is_none")]`）。
- [x] `crates/core/agent/src/event.rs`：`AgentBlock` 增 `WebSearch { id: String, query: String, results: Vec<WebSearchSource>, error: Option<String>, raw_use: Option<serde_json::Value>, raw_result: Option<serde_json::Value> }`（`results` 空省略、其余 `Option` 省略，见 D1）。
- [x] `crates/core/agent/src/event.rs`：新增 `AgentCitation { url: Option<String>, title: Option<String>, cited_text: Option<String>, raw: serde_json::Value }` 与 `WebSearchSource { url: String, title: Option<String>, page_age: Option<String> }`（各自 `#[serde(rename_all = "camelCase")]` + `specta::Type`，D1）。
- [x] `crates/core/agent/src/event.rs`：更新 `AgentBlock` 的枚举 doc（四变体 → 五变体，注明 server 块与 citations 的抗失真语义、以及旧客户端整条解码失败的代价）。
- [x] `crates/core/agent/src/lib.rs`：`pub use event::{…, AgentCitation, WebSearchSource};`。
- [x] `crates/core/orchestration/src/transcript.rs`：`final_assistant_text` 的 `AgentBlock::Text { text }` 模式补位为 `Text { text, .. }`。

> **回溯修订（第四稿）**：本阶段第 1–2 项记的 `skip_serializing_if` 属性已由**阶段 13** 撤销（D1 翻转，保留 `#[serde(default)]`）——属性面以阶段 13 为准，本阶段其余各项（变体、值类型、导出、doc、模式补位）不变。
>
> **本阶段无编译门**：`crates/infra/agent/src/sdk/{loop,resume}.rs` 仍持 `Text { text }` 单字段形态（阶段 3 / 阶段 4 才补位），整 workspace 此刻**预期不编译**。缺失的编译点已在本清单内逐项登记，无遗漏项：`AgentBlock::Text` 侧 loop.rs `:137` 字面量 / loop.rs `:319` 内联转换 / resume.rs `:84` 与 `:95` 模式（阶段 3 / 阶段 4 补位）；`LoopTurn` 侧 runner.rs `:316-329` 的 `LoopTurn { … }` 字面量（阶段 3 增字段后缺 `web_search`，阶段 5 补齐）。源码级残留扫描见阶段 4 末尾，`cargo check` 编译门见阶段 5 末尾。

## 阶段 2 · 引擎 web_search 能力模块（解锁 loop / resume / context）

- [x] 新建 `crates/infra/agent/src/sdk/web_search.rs`：写模块 doc（raw 必须配空文本；展示字段不参与回放；常量单点；两轴上界分名）。
- [x] `web_search.rs`：常量面 `WEB_SEARCH_TOOL_KIND = "web_search_20250305"` / `WEB_SEARCH_TOOL_NAME = "web_search"` / `WEB_SEARCH_MAX_USES = 5` / `ANTHROPIC_RAW_CONTENT_KEY = "anthropic_content"` / `CITATIONS_KEY = "citations"`。
- [x] `web_search.rs`：`provider_tool() -> ProviderToolDefinition`（`new(kind)` + `name` + `max_uses` 两条 config）。
- [x] `web_search.rs`：`raw_content(&Text) -> Option<&Value>` 与 `citations_of(&Text) -> Option<Vec<AgentCitation>>`（空 / 缺失 → `None`；`raw` 取元素原值，D4）。
- [x] `web_search.rs`：`project_choice(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)`（先索引 result、再顺序投影；use+result 收敛为一块、孤儿 result 单块、未知 server 块跳过、`Text`/`Reasoning`/`ToolCall`/`Image` 按 D2）。
- [x] `web_search.rs`：`replay_raw(&Value) -> AssistantContent`（**空文本** + `anthropic_content`）与 `replay_text(&str, Option<&[AgentCitation]>) -> AssistantContent`（非空 citations 才挂 `citations` 数组）。
- [x] `crates/infra/agent/src/sdk/mod.rs`：挂 `pub(crate) mod web_search;`（**不**加 `#[cfg(test)] mod web_search_test;`——该行随测试文件由测试相位添加）。

## 阶段 3 · loop 驱动（字面量补位 + 投影接入 + `pause_turn` 续跑）

- [x] `crates/infra/agent/src/sdk/loop.rs`：`LoopTurn` 增 `pub web_search: bool`；常量区增 `PAUSE_TURN_MAX_CONTINUATIONS = 5`（注释写明与 `WEB_SEARCH_MAX_USES` 是两个概念）。
- [x] `loop.rs`：用户提示词块的 `AgentBlock::Text { text: turn.question.clone() }` 字面量补位为 `{ text: …, citations: None }`（用户消息无引用；新字段的编译必需点，位置约 `:137`）。
- [x] `loop.rs`：请求构造改为可变的 `CompletionRequest` + `if turn.web_search { request = request.provider_tool(web_search::provider_tool()); }`（`tools: definitions.clone()` 不动）。
- [x] `loop.rs`：`'attempt` 的 break 值改为 `(choice, usage, finish_reason)`；`finish_reason` **在解构 `response.finish()` 的 `Ok` 值之前**读——`let finish_reason = response.finish_reason();` 先行，再 `let rig::completion::CompletionResponse { choice, usage: attempt_usage, .. } = response;`（`CompletionResponse::finish_reason(&self) -> Option<FinishReason>` 需借用，解构后 `choice` 已移出，故必须先读；D6）。
- [x] `loop.rs`：删除内联四变体块转换（含 `:317-340` 的 `AgentBlock::Text` 字面量），改调 `web_search::project_choice(&choice)`；保留 `history.push(Message::Assistant { content: choice })` 与密封 `Message` 发射的既有次序。
- [x] `loop.rs`：重构轮体为「外层轮循环 + 内层续跑循环」——每轮次次序固定为「`defend`（每次发请求前，既有防线调用点保留）→ build chat_history → `'attempt`（每次进入前 `attempt = 0`）→ `usage += turn_usage` → 空 choice 走既有收敛 → 投影 + 入史 + 密封发射 → 分支」。
- [x] `loop.rs`：实现续跑分支（D5 次序）：客户端 `tool_calls` 非空 → 跳出续跑循环走既有工具执行；`finish_reason` 非 `Other("pause_turn")` → `finish_success`；`continuation >= PAUSE_TURN_MAX_CONTINUATIONS` → 发 `SystemNotice{subtype:"pause_turn_capped", payload:{"continuations":5}}` 后 `finish_success`；`handle.stop_requested()` → 直接返回；否则 `continuation += 1` + 发 `SystemNotice{subtype:"pause_turn_continued", payload:{"continuations":n}}` 并继续。
- [x] `loop.rs`：自查 `turn_index` 只在工具轮后递增（续跑不计轮数）、`RunStarted` 只发一次、`normalize` / `deny` / `outcome_result` / `finish*` 逐字未改、`defend` 函数体逐字未改（调用点保留在每次发请求之前）。
- [x] 自查（预期不编译的登记项）：本阶段给 `LoopTurn` 增 `web_search` 后，`crates/infra/agent/src/sdk/runner.rs:316-329` 的 `LoopTurn { … }` 字面量缺字段——**属预期不编译**，该构造点的传递在阶段 5（`session_pump`），本阶段**不动 `runner.rs`**；`cargo check` 也不在本阶段执行。

## 阶段 4 · 回放与上下文防线（阶段末只做源码级残留自查）

- [x] `crates/infra/agent/src/sdk/resume.rs`：`text_content` 的 `Text` 模式补位为 `Text { text, .. }`。
- [x] `crates/infra/agent/src/sdk/resume.rs`：`assistant_content` 改为逐块展开循环——`Text { text, citations }` → `web_search::replay_text(text, citations.as_deref())`；`WebSearch { raw_use, raw_result, .. }` → 依次 `replay_raw`（先 use 后 result、双 `None` 跳过）；`Thinking` / `ToolUse` 现状；assistant 位 `ToolResult` 仍跳过。
- [x] `crates/infra/agent/src/sdk/context.rs`：`sweep_orphans` 扩 assistant 位 server 块配对（D7：`Text` 的 raw `server_tool_use.id` 入集合；raw `web_search_tool_result` 的 `tool_use_id` 不在集合中即剔除该 `Text` 项；剔空后整条 assistant 消息移除），签名不变。
- [x] `crates/infra/agent/src/sdk/context.rs`：自查三处调用点（`prune` / `hard_prune` / `compact::summarize` 后）零改动、`UserContent` 侧配对逻辑逐字未改、`compact.rs` 零 diff。
- [x] **单字段模式残留自查（本阶段末尾；逐文件源码扫描，不跑 `cargo check`）**：确认生产源无 `AgentBlock::Text { text }` 单字段形态残留——`crates/core/orchestration/src/transcript.rs`（`Text { text, .. }`）、`sdk/loop.rs`（`:137` 已补 `citations: None`、内联字面量已随 `project_choice` 删除）、`sdk/resume.rs`（`Text { text, .. }` 两处）三处均已补位。测试文件内的 `AgentBlock::Text { … }` fixture 字面量由测试相位随新字段补位，不在本范围。**编译门不在本阶段**：`runner.rs:316-329` 的 `LoopTurn` 字面量仍缺 `web_search`（阶段 5 补齐），故 `cargo check` 单点后移至阶段 5 末尾。

## 阶段 5 · provider 开关链路（store → 命令 → 组合根 → 门面 → runner）

- [x] `crates/infra/store/src/model.rs`：现 `AgentProviderRecord`（v3）改名 `AgentProviderRecordV3`，标 `pub(crate)`、保留 `native_model(id = 5, version = 3, from = AgentProviderRecordV2)`、去掉 `#[native_db]` 与 `#[primary_key]`（D9），补 doc「升级链中转，不进库模型组、不出公共查询面」。
- [x] `crates/infra/store/src/model.rs`：新增当前 `AgentProviderRecord`（`native_model(id = 5, version = 4, from = AgentProviderRecordV3)` + `#[native_db]` + `#[primary_key] id`），字段面 = v3 + `#[serde(default)] pub web_search: bool`（doc 写明缺省 `false`、生效门在解析期）。
- [x] `crates/infra/store/src/model.rs`：补 `impl From<AgentProviderRecordV3> for AgentProviderRecord`（`web_search: false`）与 `impl From<AgentProviderRecord> for AgentProviderRecordV3`（丢列），注释沿既有「降级不作数据承诺」口径。
- [x] `crates/infra/store/src/model.rs`：`AgentProviderRecord::new(...)` 七参不动、内部 `web_search: false`；新增 `with_web_search(self, web_search: bool) -> Self`；手写 `Debug` 追加 `web_search` 位。
- [x] `crates/infra/store/src/model.rs`：自查 `store.rs` 的 `.define::<AgentProviderRecord>()` 与读写方法因类型名不变而零改；`SessionEventRecord` 零改。
- [x] `src/commands/agents/mod.rs`：`save_agent_provider` 末位追加 `web_search: bool`（第九参），构造后 `.with_web_search(web_search)`；保留 `#[allow(clippy::too_many_arguments)]`；doc 补「不因协议拒收，生效门在解析期」。
- [x] `crates/infra/agent/src/compose.rs`：`ResolvedEngine` 增 `web_search: bool`；`resolve_agent_engine` 内先算 `let protocol = map_protocol(provider.protocol);`，再填 `web_search: provider.web_search && matches!(protocol, Protocol::Anthropic)`（在字段按值移动前，D8）。
- [x] `crates/infra/agent/src/lib.rs`：`EngineFacade` 增 `web_search: bool`；补齐五处 `Self{}` 构造点（`new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_explore_note_store`）；新增 `with_web_search(self, bool) -> Self`；`runner_for` sdk 臂追加 `.with_web_search(self.web_search)`。
- [x] `crates/infra/agent/src/compose.rs`：门面链追加 `.with_web_search(resolved.web_search)`（与 `with_max_tokens` 相邻）。
- [x] `crates/infra/agent/src/sdk/runner.rs`：`SdkRunner` 增私有 `web_search: bool`（`new` 内 `false`，签名不动）+ `with_web_search(self, bool) -> Self`；`PumpPayload` 增字段并在 `open_session` 装填；`session_pump` 构造 `LoopTurn` 时传递；启动日志追加 `web_search={}`。
- [x] 自查：门面与 runner 侧零协议判定（`Protocol` 只出现在 `compose` 与 `runner::build_model`）；`session_pump` 的 `LoopTurn { … }` 字面量已补 `web_search: payload.web_search`（阶段 3 起悬空的唯一构造点，缺它则本阶段无法编译）。
- [x] **编译门（本阶段末尾的唯一单点，取代阶段 4 的早期版本）**：`cargo check --manifest-path packages/desktop/src-tauri/Cargo.toml` 全绿（默认目标，不含 `--all-targets`）；确认阶段 1 的 `citations` 字段面与阶段 3 的 `LoopTurn.web_search` 在 loop / resume / context / runner / lib / compose / store / commands 全链已闭合，且生产源无 `AgentBlock::Text { text }` 单字段形态残留（与阶段 4 的源码扫描互为复核）。**若本门失败，禁止以回退阶段 4 / 3 的补位来绕过**。

## 阶段 6 · bindings 再生成

> **回溯修订（第四稿）**：本阶段的产物当时含 `AgentBlock` / `AgentEvent` / `AgentRunMessage` 的相位伴生类型（`skip_serializing_if` 所致），**已由阶段 13 撤销并再生成**——生成物口径以阶段 13 为准（统一形态、`src/bindings/mod_test.rs` 零改动）。本阶段的两项历史结论（类型名 / 可选位在场）不撤回。
>
> 前置条件：`export-bindings` 需要 Rust 侧可编译——**阶段 5 末尾的编译门必须已过**（含 store v4 字段面、门面 / runner 新字段、`LoopTurn.web_search` 传递闭合）；前端适配层（阶段 8）的 `citations` / `webSearch` 类型导入以本阶段产物为唯一来源。

- [x] 运行 `pnpm -C packages/desktop run bindings:export`，确认 `src/types/generated/bindings.ts` 新增 `AgentBlock` 五变体 + `citations`、`AgentCitation` / `WebSearchSource`、`AgentProviderRecord.webSearch?: boolean`、`saveAgentProvider` 第九位置参；**不手改生成文件**。
- [x] 逐项核对生成形态（`webSearch?: boolean` 可选、`rawUse?` / `rawResult?` 可选、`results?` 可选），与前端下行任务的前置假设一致后再继续。

## 阶段 7 · 前端依赖前置（`@tauri-apps/plugin-opener`，解锁阶段 9 的 `ExternalLink`）

- [x] `packages/desktop/package.json`：`dependencies` 增 `"@tauri-apps/plugin-opener": "^2.0.0"`（与 `@tauri-apps/plugin-dialog` 同形态）；**本阶段只动依赖，不动 `version`**（版本抬升见阶段 12）。
- [x] 安装并刷新锁文件：`pnpm -C packages/desktop add @tauri-apps/plugin-opener@^2`（或改包后 `pnpm -C packages/desktop install`），确认 `packages/desktop/pnpm-lock.yaml` 随动。
- [x] 自查前置条件：`packages/desktop/node_modules/@tauri-apps/plugin-opener` 存在且 `openUrl` 可解析——阶段 9 的 `ExternalLink` 以此为唯一 import 面；若离线取包失败，停下并回报（design 待决问题 4），不得先写引用后补依赖。

## 阶段 8 · 前端适配层（`agent-adapter.ts`）

- [x] `packages/desktop/src/lib/agent-adapter.ts`：`AgentToolOutput` 增可选位 `search?: { results: WebSearchSource[]; error: string | null }`；从 `types/dto` 追加 `AgentCitation` / `WebSearchSource` 类型导入。
- [x] `agent-adapter.ts`：`BlockPlan` 的 text 变体增 `citations?: AgentCitation[]`；`planBlock` 的 `text` 分支在有非空 citations 时携带（空数组折叠为 `undefined`）。
- [x] `agent-adapter.ts`：`PairedOutcome` 联合（`toolResult` 块 / `webSearch` 来源载荷）落地，`PairedResults` 改为 `Map<string, PairedOutcome>`；`PairingScope.global` 与 `local` 按 D11 扩 server 块配对（use 侧 `rawUse != null` 为索引源，result 侧 `rawUse == null` 为配对产物；跨消息由 `global` 承担）。
- [x] `agent-adapter.ts`：`planBlock` 增 `webSearch` 分支（D11 优先级：自身 `rawResult` → 视野配对 → `null`；result 侧已配对 → `null`；result 侧孤儿 → 孤儿 tool 计划 `input: null`）；`toolUse` / `toolResult` 分支改为消费 `PairedOutcome` 的 `kind` 判别；`const _exhaustive: never` 承接新变体。
- [x] `agent-adapter.ts`：新增私有 `citationMetadata(citations)`（空 / 缺省 → `undefined`；否则 `{ agent: { citations } }`，含单次 `as unknown as JSONValue` 断言与注释，D12）；`plansToParts` 的 text 部件、`textChunks` 的 `text-start` 与 `text-delta` 共用它（同一份 metadata 对象）。
- [x] `agent-adapter.ts`：`plansToChunks` 把 citations 透传给 `textChunks`；`text-start` 与 `text-delta` 带同一份 metadata，`text-end` 不带。
- [x] 自查：`eventToChunk`（含 `isConsumedResult` / `mergeableResults`）与 `packages/desktop/src/lib/agent-transport.ts` **零 diff**。

## 阶段 9 · 前端呈现层

- [x] `packages/desktop/src/components/agent/agent-blocks.tsx`：新增文件内私有 `ExternalLink({ url, children })`——仅 `http:` / `https:` 渲染 `<a href>` 且 `onClick` `preventDefault()` 后调 `openUrl`（`@tauri-apps/plugin-opener`，依赖已在阶段 7 落地），失败 `.catch(() => {})`；其余 scheme / `null` 渲染为纯文本。
- [x] `agent-blocks.tsx`：新增文件内私有 `CitationList({ citations })`——按 url 去重（无 url 条目逐条保留）、显示 `title ?? url ?? citedText`，条目经 `ExternalLink` 渲染。
- [x] `agent-blocks.tsx`：`TextBlock` 扩参 `citations?: AgentCitation[]`，非空时在 markdown 下方挂 `CitationList`；空 / 缺省不渲染容器。
- [x] `agent-blocks.tsx`：新增导出 `WebSearchCard({ toolCallId, toolName, input, output })`——`data-testid="block-web-search"`，summary 恒「网络搜索」（无状态文案、无进行中/超时降级），body 呈查询词（`input.query`，`input` 为 `null` 时省略该行）、来源列表（`title ?? url`，`output.search.results` 按 url 去重）、错误码（`output.search.error` 非空时）。
- [x] `packages/desktop/src/components/agent/tool-cards.tsx`：`DEFAULT_TOOL_CARDS['web_search'] = WebSearchCard`（自 `./agent-blocks` 引入，无环）。
- [x] `packages/desktop/src/components/agent/agent-messages.tsx`：`useHistoryWindow` 的 `conversationMessages` 过滤追加 `message.parts.length > 0`（`remaining` 与 `loadMore` 基于过滤后列表）。
- [x] `agent-messages.tsx`：`PartView` 的 text 分支改为 `<TextBlock text={part.text} citations={citationsOfPart(part)} />`，新增文件内私有 `citationsOfPart(part)` 宽解析 `part.providerMetadata?.agent?.citations`（非数组 → `undefined`）。
- [x] 自查：`AgentDataParts` 成员零新增（无 `data-citations` / `data-web-search`）；`@tauri-apps/plugin-opener` 的 import 全仓仅此一处。

## 阶段 10 · 管理页（provider 勾选项）

- [x] `packages/desktop/src/views/agents/hooks/use-agent-providers.ts`：`AgentProviderSaveInput` 增 `webSearch: boolean`（末位）；`commands.saveAgentProvider(...)` 末位追加 `input.webSearch`。
- [x] `packages/desktop/src/views/agents/components/provider-panel.tsx`：`ProviderFormState` 增 `webSearch: boolean`；`EMPTY_FORM` 缺省 `false`；`ProviderTextField` 排除 `webSearch`；`toFormState` 读 `editing.webSearch ?? false`。
- [x] `provider-panel.tsx`：仅 `protocol === 'anthropic'` 渲染勾选项（`@base-ui/react/checkbox` 的 `Checkbox.Root` + `Checkbox.Indicator`，`data-testid="provider-web-search"`，标签与提示「Anthropic 服务端网页搜索（按次计费）」）。
- [x] `provider-panel.tsx`：`onProtocol` 在切离 `anthropic` 时就地清零 `webSearch`（切回不恢复）；`submit` 的载荷恒携 `webSearch`（openai 下为 `false`，不省略）。

## 阶段 11 · 壳层外链能力（Rust 侧）

> 前端侧依赖（`@tauri-apps/plugin-opener` + `pnpm-lock.yaml`）已在阶段 7 落地，本阶段只做 Rust 插件注册与权限面。

- [x] `packages/desktop/src-tauri/Cargo.toml`：workspace `[workspace.dependencies]` 增 `tauri-plugin-opener = "2"`（与 `tauri-plugin-dialog` 相邻）；根 `[dependencies]` 增 `tauri-plugin-opener = { workspace = true }`。
- [x] 安装 Rust 依赖并刷新 `Cargo.lock`（`cargo metadata` 或首次 `cargo check`）。
- [x] 依赖落地后立即核实权限标识符与 scope 形态：读 `tauri-plugin-opener` 自带的 `permissions/autogenerated/reference.md` 与 `schemas/`，确认 `opener:allow-open-url` 存在、`allow` 条目形状与 `http(s)` glob 写法（D15 / 待决问题 1）。**若只存在连带 `mailto:` / `tel:` 的宽权限项** → 停下并回报，不得自行选择宽权限。
- [x] `packages/desktop/src-tauri/src/main.rs`：builder 链在 `.plugin(tauri_plugin_dialog::init())` 之后追加 `.plugin(tauri_plugin_opener::init())`。
- [x] `packages/desktop/src-tauri/capabilities/default.json`：`permissions` 追加 `{ "identifier": "opener:allow-open-url", "allow": [{ "url": "http://*" }, { "url": "https://*" }] }`（glob 形态按上一步核实结果定稿，仍**只放 http(s)**）；`description` 同步改为「查询 IPC、文件夹选择器与外链打开（仅 http(s)）；无 fs / shell 等其他权限」。
- [x] 逐项审阅 capability：无 `opener:default` / `opener:allow-default-urls` / `mailto:` / `tel:` / `file:` / 任意「默认 URL 集」类条目。
- [x] 自查（本阶段末尾）：`cargo check --manifest-path packages/desktop/src-tauri/Cargo.toml` 仍全绿（`tauri-plugin-opener` 落地与 `main.rs` 插件注册后复核阶段 5 编译门结论；本阶段是最后一处 Rust 改动点）。

## 阶段 12 · 收尾自查（不含测试执行）

- [x] `packages/desktop/package.json`：`version` `0.5.28` → `0.5.29`（用户可见功能变更；依赖项已在阶段 7 落地，此处只抬版本，`pnpm-lock.yaml` 无需再动）。
- [x] 逐条走查 proposal「不要修改」清单，确认零 diff：`sdk/normalize.rs`、`lib/agent-transport.ts`、`sdk/compact.rs`、`sdk/policy.rs` / `sandbox.rs` / `tools.rs`、`RunStarted` 契约、`SessionConfigSnapshot`、`EngineConfig` 字段面、`sdk/model_spec.rs`、`AgentProviderRecord` 的 api_key 机密边界表、`openspec/changes/anthropic-web-search/explore.md`。
- [x] 逐条核对 AC-1..AC-13 与 design「验收标准回应」表，标注未覆盖项并回报（不代测试相位下结论）；核对口径按 design「验收面口径」段——验收止于单元测试 + 静态检查链。
- [x] 核对 `packages/desktop/src-tauri/src/lib.rs` 保持零改动（builder 链在 `main.rs`，见 design「事实偏差 1」）。
- [x] 确认 `plugins/**` 零改动、无插件版本 bump（本变更不触插件源）。
- [x] 留痕确认（非实现任务）：真机手工项（含 `pause_turn` 相邻 assistant 消息核验）**不入本清单**，统一并入**归档后人工验收**；边界条 16 的已知限制与 R9「升级后不可回退」在 spec 与 `implementation-notes.md`（change 目录实现备注）留痕，本清单无对应门项或阻塞项（不变量 4）。
- [x] 写出 change 目录实现备注 `implementation-notes.md`：§2 记 `pause_turn` 相邻 assistant 消息的核验口径（实现期未执行、已知限制、归档后人工验收确认动作 + 降级预案 + **结论栏待回填**，不预置结论）、§3 记自动化面自查证据、§4 记边界条 12 / 13 / 14 / 15 的留痕对照、§5 记归档后人工验收待办（无核验动作、无阻塞项，不违反不变量 4）。

## 阶段 13 · 回溯修订（D1 第四稿：删 `skip_serializing_if`，保 bindings 统一形态）

> 触发：test-execution 的 4 项 rust 失败**同源**——`AgentBlock` 的 `skip_serializing_if` 令 specta 相位分裂，产物 `_Serialize` / `_Deserialize` 与 `src/bindings/mod_test.rs` 四条 golden 断言对撞（根因与源码级证据见 design「事实偏差 3」与 D1 第四稿；design 已实证「删属性即全绿」，工作树当时已复原）。本阶段是**本轮回溯的唯一代码改动**（删 5 处属性）+ 文案同步：前端零 diff、D2–D19 零改、`src/bindings/mod_test.rs` 零改。
>
> 前置：阶段 1–12 已在工作树落地，可直接开工；**无需**回访任何已完成阶段（属性删除不引入新的编译断点，见 design「构建顺序约束（D19）」第五顺序约束）。

- [x] `crates/core/agent/src/event.rs`：删除 5 处 `skip_serializing_if`——`Text.citations`（现 `:36`）、`WebSearch.results`（现 `:65`）、`WebSearch.error`（现 `:68`）、`WebSearch.raw_use`（现 `:71`）、`WebSearch.raw_result`（现 `:74`）；**保留** `#[serde(default)]`（其承担「旧 JSON 读出 `None`」与 TS 可选位两件事）。删后确认 `Option` 落 `null`、`Vec` 落 `[]`（键恒在）。
- [x] `crates/core/agent/src/event.rs`：行内 doc 同步——`Text.citations` 的「`None` / 空 = 无引用，序列化不落键——旧 JSON 读出 `None`」改为「`None` = 无引用；**序列化落 `null`，键恒在**——旧 JSON 缺键仍读出 `None`」；`WebSearch.results` 的「空数组序列化不落键」改为「空数组落 `[]`，键恒在」；`WebSearch` 变体 doc 补一句「展示字段与 raw 字段一律落键、值可 `null`（bindings 统一形态的前提，D1）」。全文件 **MUST NOT** 再出现「不落键」表述。
- [x] `packages/desktop/src/types/generated/bindings.ts`：`pnpm -C packages/desktop run bindings:export` 再生成（**不手改**），逐项核对四件事：① 产物**零** `_Serialize` / `_Deserialize`；② `export type AgentEvent = {` 且含 `} & AgentEventKind;`；③ `agentStart: (onEvent: Channel<AgentRunMessage>,`；④ 成员形态 `citations?: AgentCitation[] | null`、`results?: WebSearchSource[]`、`error?: string | null`、`rawUse?: unknown | null` / `rawResult?: unknown | null`。
      - 落地注记：doc 文案刻意**不写** `_Serialize` / `_Deserialize` 字面量——这两个词会随 doc 进入生成物并命中 `mod_test.rs:464` 的 `!content.contains("_Deserialize")` 断言；统一形态由「属性面只标 `default`」保证，doc 只述「相位伴生类型」。
- [x] **golden 契约零改动核对（产物口径，不含测试执行）**：`src/bindings/mod_test.rs`（`packages/desktop/src-tauri/src/bindings/mod_test.rs`）保持**零改动**——该文件 `git diff` 为空，四条 golden 断言逐字仍在原位：`AgentEvent` 出线为对象形态 + `} & AgentEventKind;` 摊平（`mod_test.rs:383`）、`agentStart: (onEvent: Channel<AgentRunMessage>,`（`:278`）、`agentStart` 十参同序含尾 `cwd`（`:317`）、产物不得含 `_Deserialize`（`:464`）。该套件（33 项）全绿由桌面端 `test-execution` 复核，本清单**不登记为任务、不作本阶段判据**。**本阶段主判据 = 上一条的四项产物形态核对全过 + 本阶段复核门的 `bindings:check` 零 diff**（同门记录的 `cargo check` 零错 / `cargo clippy -p agent` 零告警一并纳入）。**MUST NOT** 改 golden、**MUST NOT** 把 `Channel<AgentRunMessage>` 改成相位伴生类型名来迁就 `skip_serializing_if`。
- [x] 文案同步（逐行照 design「规格与提案文案修订」表）：`proposal.md`（AC-1 / 六面收口第 1 条 / 变更范围「实现文件」`event.rs` 行 / 「测试文件」event_test 行）；`specs/desktop-agent-execution/spec.md`（场景「旧事件读出兼容」`:74-75`、正文 `:150` 与 `:151`、「Function: event.rs」表 `:260`）。文案口径统一为「键恒在（`null` / `[]`）；MUST NOT 加 `skip_serializing_if`」。
- [x] 自查前端零 diff：`packages/desktop/src/lib/agent-adapter.ts`、`agent-transport.ts` 及全部前端源与 `bindings.ts` 再生成前的行为口径一致（成员可选位未变；`emptyAsUndefined` / `?? []` 已按可空位消费），无需任何改动。
- [x] 复核门：`cargo check --manifest-path packages/desktop/src-tauri/Cargo.toml`（默认目标）零错、`cargo clippy -p agent` 零告警、`pnpm -C packages/desktop run bindings:check` 零 diff。
      - 落地注记：`bindings:export` 连续两次导出逐字节一致，且 `git diff --exit-code -- packages/desktop/src/types/generated` 为零（等价 `bindings:check`）；Rust 侧可编译由 `export-bindings` 的构建（含 `dev-team` 库目标）就地验证。`cargo check` / `cargo clippy` / 全量测试由桌面端静态检查链与 test-execution 执行，本相位不重复跑。
- [x] 登记（**不改实现**）：`crates/core/agent/src/event_test.rs` 的 `web_search变体缺省键省略且往返等价`（`:606-628`）与 `text块旧json无citations键读出none且再序列化不落该键`（`:666-683`）、`crates/infra/agent/src/sdk/web_search_test.rs:485-490` 的「空 results 不落键」断言，按 design「回溯修订受影响的既有断言」表改写，归**测试相位随动复跑**；本清单不保留 `skip_serializing_if` 以迁就旧断言。

## 阶段 14 · 回溯修订（`project_choice` 倒序配对：预索引期即记已消费）

> 触发：测试相位的「`project_choice` 结果先于 use」用例实录与 `design.md` D2 相左——原实现只在顺序遍历**到达 use** 时才置 `consumed`，故 choice 倒序（result 先于 use，`pause_turn` 拆分会产此形态）时 result 先以孤儿成块、随后 use 又配出第二块，同一 `WebSearch.id` 重复（违背 D2「预索引期即记已消费」与 `test-design.md`「`blocks` 恰一块、result 不另产块」）。修法限定在 `project_choice` 的预索引阶段：类型面 / 契约面 / `loop` / `resume` / `context` / 前端均零 diff。
>
> 前置：阶段 1–13 已在工作树落地；本阶段是**唯一代码改动点**（`web_search.rs` 一个函数），无需回访任何已完成阶段。

- [x] `crates/infra/agent/src/sdk/web_search.rs::project_choice`：预索引阶段先收集 `server_tool_use`（`name == "web_search"`）的 id 集合，再索引 `tool_use_id → （首个 result 下标, result 原文）`；随后仅为「**有配对 use** 的首个 result 下标」预置 `consumed`——无配对 use 的 result 不预置（孤儿规则原样），同 id 的第二条 result 亦不预置（照常成块）。
- [x] 顺序遍历阶段删除「遍历到 use 时才写 `consumed`」的原地标记（消费点单点收敛到预索引期），use 侧只取值不改标记；`consumed` 判据与孤儿分支的产块形态逐字保留（`raw_use: None` / `id` 取 `tool_use_id` / `query` 空串）。
- [x] 自查零外溢：`project_choice` 签名、`loop.rs` 调用点（`:345`）、`resume.rs` / `context.rs` 的 raw 处理、前端 `agent-adapter.ts` 的 `webSearch` 两态判别**零 diff**（纠正只落在「use + result 恰一块」）。
- [x] 复核：`cargo check -p agent-runtime --lib` 零错、`rustfmt --check --edition 2021 crates/infra/agent/src/sdk/web_search.rs` 零 diff（完整 `cargo check` / `clippy` / 全量测试由桌面端静态检查链与 test-execution 执行，本相位不重复跑）。

## 阶段 15 · 回溯修订（前端源码冗余收敛：零接口零行为变更）

> 触发：code-review C5 唯一失项属**源码冗余**（重复去重实现 + 三份同型 `asRecord` / `textOf` 副本），非死代码、非审查口径问题。本阶段是**纯重构**：删重复、收敛同型原语，导出面与行为口径逐字不变（无新增 test-only export，`knip` 零报告）。Rust 侧、`bindings.ts`、`agent-adapter.ts`、`agent-transport.ts` 零 diff。

- [x] 新建 `packages/desktop/src/lib/parse.ts`（与 `lib/explore-*.ts` 同族的纯函数模块）：导出 `asRecord(value: unknown): Record<string, unknown>`（非对象 **/ 数组**一律空对象）与 `textOf(value: unknown): string | null`；模块 doc 写明「未知形态收敛口径单点，MUST NOT 就地再下沉副本」，并记数组并入非对象的理由（索引键不是载荷字段名，消费点缺省分支由同一判据触发）。
- [x] `packages/desktop/src/components/agent/agent-blocks.tsx`：删文件内私有 `asRecord` / `textOf`（原 `:15-25`），改从 `../../lib/parse` 引入；`searchQuery` 调用形态不变。
- [x] `agent-blocks.tsx`：删私有 `dedupeSources`（原 `:176-186`），`WebSearchCard` 改调同文件泛型 `dedupeByUrl`——`WebSearchSource.url` 必填，语义等价（仅多一层「`url === null` 不参与去重」分支，对该类型恒不命中）；`WebSearchSource` 类型导入随删除一并移除。
- [x] `packages/desktop/src/components/agent/agent-messages.tsx`：删文件内私有 `asRecord` / `textOf`（原 `:48-58`），改从 `../../lib/parse` 引入；`parseCitation` / `citationsOfPart` 逐字不动。
- [x] `packages/desktop/src/components/agent/tool-cards.tsx`：删文件内私有 `asRecord`（原 `:33-38`），改从 `../../lib/parse` 引入；统一为**含 `Array.isArray` 判定**的版本——核对消费点行为不变：`parseAskInput` 的入参为数组时，旧版按索引键读 `.questions` 得 `undefined`（回退 JSON），新版直接空对象同样 `undefined`；`parseOptions` 与 `questions.map` 的元素为数组时，`label` / `description` 两处取键在两版下均取不到（`''` / `null`）。
- [x] 自查：`DEFAULT_TOOL_CARDS` / `TextBlock` / `WebSearchCard` 的 props 与导出面零变化；`src/lib/agent-adapter.ts`、`src/lib/agent-transport.ts`、`src/types/generated/bindings.ts` 零 diff；无 test-only export（`parse.ts` 的两个导出均被三处生产消费点使用，`knip` 零报告）。
- [x] 复核（本相位就地执行，非门禁脚本替代）：`tsc --noEmit` 零错、`vp check` 全量格式与 lint/类型零告警、`knip` 零报告、`vp test` 83 文件 / 1578 项全绿。
