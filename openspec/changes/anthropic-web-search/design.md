# 设计: anthropic-web-search

> **变更**: anthropic-web-search
> **日期**: 2026-10-23
> **回溯修订（第四稿）**: 2026-10-25 —— **D1 裁定翻转**：`AgentBlock` 的 `skip_serializing_if` 全部撤销（保留 `#[serde(default)]`），以「**线格式键恒在**（`null` / `[]`）」换取「**bindings 统一形态**（无 `_Serialize` / `_Deserialize` 相位分裂）」。根因：`specta-serde 0.0.12` 的 unified 校验对 `skip_serializing_if` 硬拒（`validate.rs:544`），而 tauri-specta 导出走 phases 格式，令 `AgentBlock` 分裂为相位伴生类型并沿 `AgentEvent` / `AgentEventKind` / `AgentRunMessage` / `agentStart::Channel<>` 逐层上溢——与仓库 bindings golden 契约（`src/bindings/mod_test.rs` 四条断言）**不可同时成立**。本相位已就地实证（工作树已复原，改动留给实现任务）：仅删该属性后 `bindings:export` 产物**零** `_Serialize` / `_Deserialize`、`cargo test -p dev-team --lib -- bindings::mod_test` **33 项全绿**（此前 4 项失败）。同步补：变更清单（`event.rs` 属性面 / `bindings.ts` 统一形态 / `src/bindings/mod_test.rs` 零改动核对结论）、新增「规格与提案文案修订」段与「回溯修订受影响的既有断言」表、`tasks.md` 阶段 13。**实现语义面零变化**（D2–D19 不涉改）；**本轮回溯不再动前端（前端零 diff）**——`#[serde(default)]` 在 unified mode 下同样出**可选字段**（`citations?: AgentCitation[] | null`），成员形态与相位分裂时逐字相同，`agent-adapter.ts` 已按可空位消费（`emptyAsUndefined` / `?? []`）。
> 另 4 项前端失败（`agent-adapter` 的 error 夹具违背 D11「自身 `rawResult` 优先」、`provider-panel` 勾选点击缺 `PointerEvent` 兜底）归**测试相位夹具修正**，非设计根因——D11 语义不变，本设计仅补一句消歧（见 D11 行）。
> **修订对齐（第三稿）**: 2026-10-23 —— 对齐 `proposal.md`「回溯修订（第二轮）」：**撤销 R5 真机核验的实现期强制门**。`D17` 与待决问题 5 由「实现期前置核验」改写为「已知限制留痕 + 归档后人工验收」；新增「验收面口径」段；`tasks.md` 的原「阶段 0 · 前置核验（真机门）」整段删除。**代码与实现形态零变化**（D1–D19 均不涉改，变更清单与任务门位不变）。
> **基于**: `proposal.md`、`explore.md` 与 `specs/{desktop-agent-execution,desktop-agent-management,desktop-agent-chat-infra,desktop-app-shell}/spec.md`

---

## 提案与规格同步状态

proposal 的六面与四份 delta spec 已定稿（决策 ①–⑫ 全部锁定，R6 / R10 两处遗留已裁定）。本设计不重复论证，只做六件事：**逐文件落定实现形态**（含 rig 0.43.0 的 wire 事实、三处 `sweep_orphans` 调用点、五处 `Self{}` 构造点等必须随动的位置）、**把规格未定的成形缺口以决策留痕收口**（`pause_turn` 续跑循环的嵌套形态与事件发射次序、`sweep_orphans` 在 rig 历史里如何识别 server 块、前端 citations 的 JSON 值域断言点、呈现层组件的导出边界）、**给出 `WebSearch` 块的全链数据流**、**逐条对齐 AC-1..AC-13**、**划清验收面口径（自动化面 vs 归档后人工验收，不含任何实现期真机门）**、**记录提案与工作树的三处事实偏差**（第 3 条为第四稿回溯的根因，并触发 `specs` / `proposal` 文案同步修订）。

### 起点核实（工作树实况；路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）

- **块词汇现状**：`crates/core/agent/src/event.rs:19-36` `AgentBlock` 四变体（`Text{text}` / `Thinking` / `ToolUse` / `ToolResult`）；`AgentBlock` 的 serde 配置为 `tag = "kind"` + `rename_all`/`rename_all_fields = "camelCase"`（`:14-18`）；导出面 `crates/core/agent/src/lib.rs:10`。
- **loop 现状**：`LoopTurn`（`sdk/loop.rs:75-99`）、请求构造（`:177-188`，`additional_params: None`）、`'attempt` 重试循环与收口（`:176-306`，`break 'attempt (choice, attempt_usage)` 在 `:263`）、空 choice 收敛（`:308-312`）、**内联块转换**（`:317-340`）、`history.push(Message::Assistant)`（`:341-344`）、密封发射（`:345-359`）、`tool_calls.is_empty()` 早收口（`:360-363`）。`FinishReason` 在 `loop.rs` 中**当前完全未被读取**。
- **回放现状**：`sdk/resume.rs:79-85` `text_content`（`AgentBlock::Text { text }` 模式）、`:93-107` `assistant_content`（三分支 + `_ => None` 兜底）；`rebuild` 的相邻 assistant 处理见 `:52-67`。
- **防线现状**：`sdk/context.rs:237-263` `sweep_orphans` 只认 `UserContent::ToolResult ↔ CallId`；**三处调用点**：`context.rs:116`（L2）、`context.rs:143`（L3 硬裁）、`compact.rs:67`（摘要置换后）。
- **泵现状**：`sdk/runner.rs:47-58` `SdkRunner` 字段（`new` 四参 + `with_max_tokens` 一处 builder）、`:257-271` `PumpPayload`、`:306-315` 启动日志（`session= protocol= model= window= raw=`）、`:316-329` `LoopTurn` 构造。
- **门面现状**：`crates/infra/agent/src/lib.rs:40-51` 字段、`:55-62` `new`、`:65-72` `with_resume_transcript`、`:78-85` `with_context_window`、`:90-97` `with_max_tokens`、`:103-110` `with_explore_note_store`、`:117-134` `runner_for`——**五处 `Self{}` 构造点**（proposal 记「四个构造点」为概数；以工作树五处为准）。
- **组合根现状**：`crates/infra/agent/src/compose.rs:46-62` `ResolvedEngine`、`:122-128` 门面链、`:258-274` 解析产物构造（`provider` 按值移动，故门控值须在移动前算出）。
- **store 现状**：`crates/infra/store/src/model.rs:1231-1265` 现 `AgentProviderRecord`（`native_model(id = 5, version = 3, from = AgentProviderRecordV2)` + `#[native_db]` + `#[primary_key]`）、`:1267-1282` V2→V3 升级、`:1284-1295` V3→V2 降级、`:1297-1323` `new`（七参）、`:1325-1343` 手写遮蔽 `Debug`；`store.rs:327` `.define::<AgentProviderRecord>()`；信封 `MODEL_ENTRIES` 与 `SessionEventRecord`（`model.rs:549-591`，`with = SerdeJsonCodec`）**零改**。
- **调用点实量**：`AgentProviderRecord::new(` 全仓 **38 处**（`model_test.rs` 27 / `compose_test.rs` 3 / `worker_test.rs` 2 / `store_test.rs` 2 / `envelope_test.rs` 1 / 生产 3）；`AgentProviderRecord { … }` 结构体字面量只出现在测试（`model_test.rs` 3 处、测试 fixture 构造函数）——**生产侧只经 `new`**，`with_web_search` 方案零机械改动成立。
- **命令现状**：`src/commands/agents/mod.rs:50-96` `save_agent_provider` 八参（`id / name / protocol / base_url / api_key / models / context_length / max_tokens`）；`src/types/generated/bindings.ts:79` 位置实参镜像、`:409-443` `AgentProviderRecord` 形状（`contextLength?: number | null` 等）。
- **前端现状**：`src/lib/agent-adapter.ts:48-51` `AgentToolOutput`（两字段）、`:97-107` `BlockPlan`（三变体）、`:109-113` `PairedResults = Map<string, ToolResultBlock>`、`:113-148` `PairingScope.{global,local}`、`:229-263` `planBlock`（含 `const _exhaustive: never`）、`:294-307` `plansToParts`、`:354-361` `textChunks`、`:415-435` `plansToChunks`、`:438-483` `eventToChunk`；`components/agent/agent-blocks.tsx`（69 行，`TextBlock`/`ThinkingBlock`/`ToolPairCard`/`isToolPart`）、`tool-cards.tsx:110-112` `DEFAULT_TOOL_CARDS`）、`agent-messages.tsx:47-52` `PartView`、`:157-187` `useHistoryWindow`；`views/agents/components/provider-panel.tsx:12-39` `ProviderFormState`/`EMPTY_FORM`、`:65` `ProviderTextField`、`:171-186` `toFormState`、`:248-280` 字段行、`:300-346` `ProviderForm`；`views/agents/hooks/use-agent-providers.ts:21-30` `AgentProviderSaveInput`、`:90-99` invoke 实参。
- **壳层现状**：builder 链在 **`src/main.rs:42-43`**（`.plugin(tauri_plugin_dialog::init())`），`src/lib.rs` 只挂 `pub mod bindings; pub mod commands;`；依赖在 `src-tauri/Cargo.toml:35`（根，`{ workspace = true }`）与 `:106`（workspace，`"2"`）；`src-tauri/capabilities/default.json` 现为 `["core:default", "dialog:allow-open"]`，`description` 明写「无 fs / shell 等其他权限」。
- **前端依赖现状**：`packages/desktop/package.json` `version = 0.5.28`；`@tauri-apps/plugin-dialog ^2.0.0` 在列，`@tauri-apps/plugin-opener` **未安装**（`node_modules/@tauri-apps/` 只有 api / cli / plugin-dialog / plugin-updater），锁文件 `packages/desktop/pnpm-lock.yaml` 需随动。
- **rig 0.43.0 wire 事实（源码级）**：请求侧 `ProviderToolDefinition::new(kind).with_config(..)` 经 `CompletionRequest::provider_tool` 落 `additional_params.tools`，由 `providers/anthropic/completion.rs:2237-2252` 取出并入 wire `tools`；响应侧 `Content::ServerToolUse` / `WebSearchToolResult` / `CodeExecutionToolResult` 由 `providers/anthropic/streaming.rs:341-360,418-436` 包成 `Text{text:"", additional_params:{anthropic_content: <raw wire>}}`，citations 由 `:326-340` 包成 `Text{..., additional_params:{citations: [...]}}`；回放侧 `completion.rs:610-627` **校验「raw 与文本不可共存」**（非空文本 + `anthropic_content` → `ConversionError`），且只接受 `ServerToolUse` / `WebSearchToolResult` / `CodeExecutionToolResult` 三种 raw（`:644-651`）；`pause_turn` 由 `completion.rs:67-81` `map_finish_reason` 归为 `FinishReason::Other("pause_turn")`，`CompletionResponse::finish_reason()` 在 `completion/request.rs:256`。
- **ai-sdk 事实**：`ai@7` 的 `TextUIPart.providerMetadata?: ProviderMetadata`（`node_modules/ai/dist/index.d.ts:1996-2010`）、`text-start` chunk 同带 `providerMetadata`（`:2477-2481`）、`ProviderMetadata = Record<string, JSONObject>`（`@ai-sdk/provider/dist/index.d.ts:121`）——故 citations 载荷须落在 JSON 值域（见 D12）。

### 三处事实偏差（已在本文档裁定；第 3 条同步修订 proposal / spec 文案，见「规格与提案文案修订」）

1. **builder 链位置**：proposal「变更范围 - 实现文件」写 `packages/desktop/src-tauri/src/lib.rs` 增 `.plugin(tauri_plugin_opener::init())`，实况 builder 链在 **`src/main.rs`**（`lib.rs` 为命令面/绑定导出的库目标，无 `Builder`）。裁定：改 `src/main.rs`；`src/lib.rs` 记「核对结论：零改动」。
2. **`new` 调用点计数**：proposal 正文记 35 处、spec 与验收记 38 处；`grep` 实测 **38 处**（含 `src/commands/agents/mod.rs` 的生产一处）。裁定：以 38 为准，结论不受影响（`with_web_search` 方案两者皆零改）。
3. **`skip_serializing_if` 与 bindings golden 契约不可同时成立**（本轮回溯的根因，D1 第四稿裁定翻转）——源码级事实：`specta-serde 0.0.12` 的 `validate.rs:544-548` 在 `ApplyMode::Unified` 下对 `skip_serializing_if` 直接返回 `invalid_phased_type_usage`（"unified mode cannot represent conditional omission"）；`lib.rs:1194-1219`（`rewrite_field_for_phase`）只在 `PhaseRewrite::Serialize` 时把字段置 `optional`；`lib.rs:2270-2287`（`field_has_local_difference`）把 `skip_serializing_if` 列为局部相位差触发项之一；而 `default` **不在**该清单（`lib.rs:2493-2510` `field_is_optional_for_mode` 在 `Unified` 下把 `default` 字段置可选）。tauri-specta 的导出路径走 `PhasesFormat`（`src/bindings/mod.rs` 只配 `semantic_types` / `cast_bigints`，未改格式），故含该属性的 `AgentBlock` 必然分裂为 `_Serialize` / `_Deserialize`，并沿 `AgentEvent` → `AgentEventKind` → `AgentRunMessage` → `agentStart::Channel<>` 逐层上溢，`AgentEvent` 亦从「对象 + intersection」退化为「两支联合」。仓库 bindings golden 契约（`src/bindings/mod_test.rs:278` / `:317` / `:383` / `:464`）四条断言与之逐条对撞，实测 4 项失败。裁定：**删属性、保契约**（详见 D1）；本相位已实证「删属性后四条断言全绿」，工作树复原后由 `tasks.md` 阶段 13 落地。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | `AgentBlock` 变体的序列化属性 | `Text` 增 `citations: Option<Vec<AgentCitation>>`，**只**标 `#[serde(default)]`（MUST NOT 加 `skip_serializing_if`）；`WebSearch { id, query, results: Vec<WebSearchSource>, error: Option<String>, raw_use: Option<Value>, raw_result: Option<Value> }` 的 `results` / `error` / `raw_use` / `raw_result` 同样**只**标 `#[serde(default)]`；新值类型 `AgentCitation` / `WebSearchSource` 各自 `#[serde(rename_all = "camelCase")]`，其 `Option` 字段仅 `#[serde(default)]`（固定 `null` 形态，前端与断言口径一致） | **回溯修订（第四稿）翻转**：AC-1 的两条子句（旧 JSON 读出 `None`；缺省不落键）只有 `default` + `skip_serializing_if` 同时在场才两条都成立，但后者与仓库 bindings 契约硬冲突（见「事实偏差 3」：`validate.rs:544` 在 unified 校验直接拒，导出走 phases 后 `AgentBlock` / `AgentEvent` / `AgentEventKind` / `AgentRunMessage` / `agentStart::Channel<>` 全会分裂，`src/bindings/mod_test.rs` 四条 golden 断言 4 项失败）。取舍：**舍「条件省略」、保「统一形态」**——「前端既有 fixture 零改动」的实际承担者是 `default` 触发的**可选字段**（unified 下同样出 `citations?: AgentCitation[] \| null`），与是否落键无关；省略只省线格式体积（增量并入 R10「不设上限、不压缩」），而分裂会污染全量 DTO 消费面并迫使 weaken 仓库级守卫。**被拒备选**：①改 golden（删 `_Deserialize` 断言、`Channel<AgentRunMessage>` → `Channel<AgentRunMessage_Deserialize>`、`AgentEvent` 断言改成联合形态）——为一处字段 weaken **全体** DTO 的公共守卫，并把相位伴生类型名泄漏进生成物与后续类型引用；②为 `AgentBlock` 手写统一 `Type` impl 以维持省略——自造非既有约定，且与 serde 实际行为不一致（模型会省略、绑定说不能）；③保留 `skip_serializing_if` 且不导出 `AgentBlock`——`AgentEvent` 在 IPC 面不可回避，不成方案。**实证（本相位，工作树已复原）**：仅删该属性后 `bindings:export` 产物**零** `_Serialize` / `_Deserialize`（`AgentEvent` 回到对象 + intersection、`agentStart` 首参回到 `Channel<AgentRunMessage>`），`cargo test -p dev-team --lib -- bindings::mod_test` **33 项全绿**；`disable_serde_phases` 在含该属性时于导出期硬错。**线格式后果（记录）**：`text` 块恒落 `citations`（`null` 或数组），`webSearch` 块恒落 `results`（`[]` 或数组）与 `error` / `rawUse` / `rawResult`（`null` 或原值）；旧 JSON 缺 `citations` 仍读出 `None`；raw 逐字节保真与 AC-2 零影响 |
| D2 | 一次搜索的原子收敛点 | `web_search::project_choice(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)` 为**唯一**块投影实现：先按 `tool_use_id` 索引全部 `web_search_tool_result`（并记其下标为「已消费」），再顺序遍历 choice——`server_tool_use`（`name == WEB_SEARCH_TOOL_NAME`）产一块 `WebSearch` 并把配对 result 的 `results`/`error`/`raw_result` 并入；孤儿 result 单独成块（`raw_use: None`，`id` 取 `tool_use_id`，`query` 为空串）；非 `web_search` 的 raw server 块与 `code_execution_tool_result` 跳过；`Text` 无 raw → `Text{text, citations}`；`Reasoning`/`ToolCall`/`Image` 现状不变。loop 内联四变体 match **整体删除** | spec「投影规则」逐条；块语义单点后，loop 只剩「调用 + 发事件」，`resume` / 前端都以事件面为准，不各自再解 raw |
| D3 | 展示字段与回放字段的类型分家 | `query` / `results` / `error` 为展示字段，**不参与**任何重建；`raw_use` / `raw_result` 为回放唯一事实源。重建只经 `web_search::replay_raw(raw) -> AssistantContent` 一条路径，产 `Text { text: String::new(), additional_params: Some({anthropic_content: raw.clone()}) }`（**必须空文本**，rig 侧带文本即 `ConversionError`）。`resume::assistant_content` 改为「一条块最多产两条」的展开循环：`WebSearch` → `raw_use` 后 `raw_result`（保原序），两者皆 `None` 跳过 | R1 / AC-5：类型上把展示与 wire 分家，重建函数签名只吃 `Value`，物理上无法用 `results` 之类字段拼回放；空文本是 rig 的硬校验（`completion.rs:610-619`） |
| D4 | citations 的提取口径 | `citations_of(&Text) -> Option<Vec<AgentCitation>>`：键 `additional_params[CITATIONS_KEY]` 缺席 / 非数组 / **空数组** → `None`；逐项产 `AgentCitation { url: item["url"], title: item["title"], cited_text: item["cited_text"], raw: item.clone() }`（字段缺失/类型不符 → `None`，不报错）。`raw` **取数组元素原值**，不经 rig `Citation` 反序列化再序列化 | R2：rig 的 `Citation` 往返会规范化（`title: Option` → `null`），回填即失真；`None` 折叠空数组使「空 citations 不挂 metadata」（AC-9）在 Rust 侧即成立，前端无需第二判定 |
| D5 | `pause_turn` 续跑的结构 | 轮驱动主循环内新增**续跑循环**（嵌套，一层）：`continuation` 计数轴独立；每轮次执行「`defend`（每次发请求前，既有调用点原样保留）→ build chat_history → `'attempt` 重试循环 → `usage += turn_usage` → 空 choice 收敛 / 投影 + 入史 + 密封发射 → 分支」；分支次序为 **空 choice（收敛）→ 客户端 `tool_calls` 非空（走工具执行，续跑计数不增）→ `finish_reason` 非 `Other("pause_turn")`（收敛）→ 续跑上限（发 `pause_turn_capped` 后按 `stop` 收敛）→ 停止信号检查 → `continuation += 1` + 发 `pause_turn_continued` → 继续续跑循环**。`attempt` 在每次进入 `'attempt` 前重新置 0（续跑不继承重试预算） | AC-6 五条与 R3 两轴分名逐条落地；把「续跑」放在**同一 turn_index** 的嵌套循环而非外层轮循环，避免误发第二条 `RunStarted`、误增 `num_turns`、误跑 `turn_index += 1`；分支次序保证「工具执行优先收口」与「空轮既有收敛路径不变」 |
| D6 | `finish_reason` 的取值与判据 | `'attempt` 返回 `(choice, usage, finish_reason: Option<FinishReason>)`，取 `CompletionResponse::finish_reason()`——该方法签名为 `finish_reason(&self) -> Option<FinishReason>`，故须在解构 `CompletionResponse { choice, .. }` **之前**读（解构后 `choice` 已移出、无法再借用）：`let finish_reason = response.finish_reason();` 先行，再解构取 `choice` / `usage`；判据 `matches!(reason, Some(FinishReason::Other(reason)) if reason == "pause_turn")`——`None` 一律视为非 `pause_turn` | spec「rig 映射为 `FinishReason::Other("pause_turn")`」；不做字符串包含匹配（防 `Other("pause_turn_x")` 误判），不做 provider 特判（`core`/`infra` 不认协议） |
| D7 | `sweep_orphans` 如何识别 server 块 | 在 rig 历史中 server 块**只以** `AssistantContent::Text { text: "", additional_params: Some({anthropic_content: raw}) }` 形式存在（回放与轮内 `choice` 入史同形）。扩展后的清扫：单向前序扫描 assistant 消息，遇 raw `type == "server_tool_use"` 把 `id` 收进 `server_uses` 集合并保留；遇 raw `type == "web_search_tool_result"` 且 `tool_use_id ∉ server_uses` → 剔除该 `Text` 项，否则保留；剔除后 `content` 为空则整条 assistant 消息移除；`UserContent` 侧既有 `CallId` 配对逻辑逐字不动 | AC-7 与 spec「索引源 `WebSearch.id` / `raw_result.tool_use_id`」：历史里没有 `AgentBlock`，只有 raw `Text`，`WebSearch.id` 等价于 raw `server_tool_use.id`；单向前序扫描同时覆盖「同消息成对」与「`pause_turn` 跨消息成对」两形态（与既有 `ToolResult` 清扫同构）；三处调用点（L2 / L3 / 摘要置换）签名不变即自动生效 |
| D8 | 门控单点与载荷类型 | `ResolvedEngine.web_search: bool = provider.web_search && matches!(protocol, Protocol::Anthropic)`，在 `provider` 按值移动**之前**算出（`let protocol = map_protocol(provider.protocol);` 先行）；门面 `with_web_search(bool)`、runner `with_web_search(bool)`、`PumpPayload.web_search: bool`、`LoopTurn.web_search: bool`——**全链 `bool` 而非 `Option<bool>`**；门面与 runner 零协议判定 | spec「`web_search` 的语义为已按协议门控的最终开关」；`bool` 使「未装配 = 关」与「显式关」同义（`SdkRunner::new` 缺省 `false`），省去 `Option` 的三态歧义；门控在组合根是唯一需要看协议的地方（`AgentProviderProtocol` → `Protocol` 映射本就单点于此） |
| D9 | store v4 的演进形态 | 现 `AgentProviderRecord`（v3）改名 `AgentProviderRecordV3`，标 `pub(crate)`、`#[native_model(id = 5, version = 3, from = AgentProviderRecordV2)]`，**去掉 `#[native_db]` 与 `#[primary_key]`**（沿 `AgentProviderRecordV1` / `V2` 先例：historic 形态不进库模型组）；新增当前 `AgentProviderRecord`（`version = 4, from = AgentProviderRecordV3`）增 `#[serde(default)] pub web_search: bool`；补 V3→V4（补 `false`）与 V4→V3（丢列）双向 `From`；`new(...)` 七参**不动** + `with_web_search(bool)`；手写 `Debug` 追加 `.field("web_search", &self.web_search)` | R12 / AC-8：`#[primary_key]` 只在 `#[native_db]` 类型上有意义，historic 形态带住会重复注册；`#[serde(default)]` 使「旧 JSON 缺字段」与「升级链补 `false`」两条路径都成立，行为零变化（默认关） |
| D10 | 命令面第九参不校验协议 | `save_agent_provider(..., web_search: bool)` 末位追加，构造后 `.with_web_search(web_search)`；**不加** `protocol` 相关判定、不因 `openai` 拒收或清零 | spec「保存语义 SHALL 为无条件接受」；R11 的两种操作序列收敛到同一记录形态，纠偏只在解析期 |
| D11 | 前端搜索卡的配对来源与优先级 | `PairedOutcome` 改为联合 `{ kind: 'toolResult', block } \| { kind: 'webSearch', results, error }`；`PairingScope.global` 的索引源扩为「`toolUse.id` ∪ `webSearch.rawUse != null` 的 `id`」，配对产物为同 id 的 `toolResult` 块或 `webSearch.rawUse == null` 的块；`planBlock` 的 `webSearch` 分支：use 侧（`rawUse != null`）→ `output` 优先取**自身** `rawResult != null` 时的 `{ results, error }`（同消息收敛形态），否则取视野配对命中，两者皆无 → `null`；result 侧（`rawUse == null`）→ 配对命中返回 `null`（被 use 消费），未命中产孤儿 tool 计划（`input: null` + `output` 携带来源）。**消歧（第四稿补记，回落 4 项前端失败之一）**：所谓「自身 `rawResult` 优先」是**整份 payload 同源优先**——`rawResult != null` 时 `results` 与 `error` **成对**取自自身块，MUST NOT 逐字段与配对产物混合（用例把 `error` 放在同消息的 result 侧块、同时让 use 侧带 `rawResult` 时，`isError` 必为 `false`，这是 D11 的正确行为而非缺陷） | AC-9 两态 + spec Scenario「搜索块两态与视图层零扫描」；「自身 `rawResult` 优先」补齐 spec 未点名的**同消息形态**（`pause_turn` 未拆分时 use+result 同块），否则该形态会显示成「只有查询词」——这是规格「配对 = `output-available`」的必要补足（见待决问题 3） |
| D12 | citations 挂 text 部件的发射点 | 单点 helper `citationMetadata(citations)`：`undefined` / 空数组 → `undefined`；否则 `{ agent: { citations } }`，函数返回类型标 `ProviderMetadata \| undefined` 并对 `citations` 做**一次** `as unknown as JSONValue` 断言；`plansToParts` 的 text 部件、`plansToChunks` 的 `text-start` **与** `text-delta` 共用该 helper（同一份 metadata 对象；`text-end` 不带） | AC-9 / spec「空数组 MUST NOT 挂；`text-start` 与 `text-delta` 带同一 metadata」；ai-sdk 的 `ProviderMetadata = Record<string, JSONObject>` 要求 JSON 值域，而 `AgentCitation.raw` 在 TS 面是 `unknown`（`serde_json::Value` → specta `unknown`）——值来自 IPC 的 JSON 边界，故断言是类型面补齐而非运行时转换；放一处 helper 保证两路同形 |
| D13 | 呈现层组件边界与导出面 | `WebSearchCard` / `CitationList` / `ExternalLink` 均住 `agent-blocks.tsx`（spec 指定落点）；**仅 `WebSearchCard` 导出**（被 `tool-cards.tsx` 注册表跨文件消费），`CitationList` / `ExternalLink` 文件内私有；`WebSearchCard` 的 props 直接采用 `AgentToolCardProps` 的结构形状（`{ toolCallId, toolName, input, output }`），不引入跨文件类型依赖；`TextBlock` 扩参 `citations?: AgentCitation[]`，底部挂 `CitationList` | AGENT.md「No test-only exports」+ knip 死代码纪律：`CitationList` / `ExternalLink` 若导出而无跨文件消费者会被报告为未使用；测试经 `TextBlock`（引用列表）与 `WebSearchCard`（来源 / 外链）间接驱动，不为测试开口子。`agent-blocks` 不 import `tool-cards`，无环 |
| D14 | citations 从部件到 `TextBlock` 的传递点 | `agent-messages.tsx` 的 `PartView` text 分支改为 `<TextBlock text={part.text} citations={citationsOfPart(part)} />`，`citationsOfPart` 读 `part.providerMetadata?.agent?.citations`（运行时宽解析，非数组即 `undefined`） | 部件分发唯一在 `PartView`；`TextBlock` 保持纯展示。该改动**超出** proposal 对 `agent-messages.tsx` 的单一描述（原文只列空 parts 过滤），但服务于同一 AC-10「正文底部引用列表」——已列入变更清单并记为规格补足（待决问题 2） |
| D15 | 外链权限标识符与 scope 形态 | 以 **`opener:allow-open-url` + url scope** 为定稿形态（capability 条目：`{ "identifier": "opener:allow-open-url", "allow": [{ "url": "http://*" }, { "url": "https://*" }] }`）；**不用** `opener:default` / `opener:allow-default-urls`（连带 `mailto:` / `tel:`，与 AC-12 冲突）；`description` 同步改为「查询 IPC、文件夹选择器与外链打开（仅 http(s)）」 | R6 裁定「以只放 http(s) 为硬约束」；「`allow` 条目接受 `{ "url": … }` 对象」「`*` 跨 `/` 匹配」与「`opener:allow-open-url` 存在」三点已按插件自带 `permissions/autogenerated/reference.md` 与 `src/scope.rs` 源码核实通过（结论见待决问题 1），落地形态即上列条目，无第二形态 |
| D16 | 版本与产物 | `packages/desktop/package.json` `0.5.28 → 0.5.29`，同时增 `@tauri-apps/plugin-opener: ^2.0.0`；`bindings.ts` 由 `bindings:export` 再生成（不手改）；`Cargo.lock` 与 `packages/desktop/pnpm-lock.yaml` 随依赖安装刷新 | 用户可见功能变更（AC-13）；锁文件是 install 的产物、不是手写实现文件（proposal 未列，本设计补记） |
| D17 | 相邻 assistant 消息不在本期合并 | `resume::rebuild` 对两条 assistant 事件照常各产一条 `Message::Assistant`（与现状一致），不做同角色合并；边界条 16 的降级预案**不预建、无实现分支**；该依赖以**已知限制**留痕于边界条 16，确认动作并入**归档后人工验收**（真实 `pause_turn` 会话续轮，结论记入验收记录）——本期**不设实现期核验门**、**不作任务阻塞项**（无官方凭据，该门无法在实现期履行） | spec 边界条 16「本期不预建、无实现分支」+ proposal「回溯修订（第二轮）」；预建合并逻辑会在服务端本就接受时变成无谓的转录改写（破坏「一件块 → 一条消息」的调试可读性） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| core 块词汇 | `AgentBlock` 五变体 + `AgentCitation` / `WebSearchSource` 值类型；协议中立（不认识 anthropic / rig） | `crates/core/agent/src/event.rs`（改） | `serde`、`specta`（既有） | 既有 serde/specta 属性面 |
| core 导出面 | 导出两个新值类型 | `crates/core/agent/src/lib.rs`（改） | — | 既有 `pub use` |
| 引擎 web_search 能力 | 常量单点（kind / name / `max_uses` / raw key / citations key）+ 六个纯函数（请求侧定义、raw 提取、citations 提取、块投影、两条回放构造） | `crates/infra/agent/src/sdk/web_search.rs`（新） | `rig::completion::{message::{AdditionalParams, AssistantContent, Text, ToolCall}, ProviderToolDefinition}`、`agent::{AgentBlock, AgentCitation, WebSearchSource}` | 纯函数（零 IO / 零 async） |
| 模块挂载 | `pub(crate) mod web_search;`（测试模块行由测试相位随文件添加） | `crates/infra/agent/src/sdk/mod.rs`（改） | — | 既有模块形态 |
| loop 驱动 | `LoopTurn.web_search`；请求按需挂 provider tool；块投影换 `project_choice`；`'attempt` 带出 `finish_reason`；`pause_turn` 续跑循环（两轴 + 两条 notice） | `crates/infra/agent/src/sdk/loop.rs`（改） | `web_search` 模块、`rig::completion::FinishReason`、既有 `normalize` / `context` / `compact` | 既有 async loop 骨架 |
| 回放重建 | `assistant_content` 展开为逐块序列；`Text` 携 citations 回填；user 位模式补位 | `crates/infra/agent/src/sdk/resume.rs`（改） | `web_search::{replay_raw, replay_text}` | 既有纯函数 |
| 上下文防线 | `sweep_orphans` 扩 assistant 位 server 块配对（剔 result 侧孤儿、剔空整条移除） | `crates/infra/agent/src/sdk/context.rs`（改） | `web_search::{raw_content, ANTHROPIC_RAW_CONTENT_KEY}` | 既有 `HashSet` 单遍扫描 |
| 开关链路（store） | provider 记录 v4（`web_search: bool`）+ V3↔V4 升级链 | `crates/infra/store/src/model.rs`（改） | `native_model`、`serde`（既有） | 既有 native_model 版本机制 |
| 开关链路（命令） | `save_agent_provider` 第九参 | `src/commands/agents/mod.rs`（改） | `store::AgentProviderRecord` | 既有 `#[tauri::command]` |
| 开关链路（组合根） | `ResolvedEngine.web_search` 门控单点 + 门面链追加 | `crates/infra/agent/src/compose.rs`（改） | `Protocol`、`EngineFacade` | 既有 |
| 开关链路（门面） | `EngineFacade.web_search` + `with_web_search`（五处构造点补齐） | `crates/infra/agent/src/lib.rs`（改） | — | 既有 builder 形态 |
| 开关链路（runner） | `SdkRunner.web_search` + `with_web_search`；`PumpPayload` 字段；`LoopTurn` 传递；启动日志 | `crates/infra/agent/src/sdk/runner.rs`（改） | `web_search` 无关，仅布尔传递 | 既有 |
| 前端适配层 | `BlockPlan` 扩 citations / 搜索承载；`PairedOutcome` 联合；两视野扩 server 块配对；两发射层挂 metadata | `packages/desktop/src/lib/agent-adapter.ts`（改） | `ai`（`ProviderMetadata` / `JSONValue` 类型）、`bindings`（`AgentCitation` / `WebSearchSource`） | 既有纯函数三层 |
| 前端呈现层 | `WebSearchCard` / `CitationList` / `ExternalLink`；注册表项；空部件过滤 + citations 传递 | `components/agent/{agent-blocks,tool-cards,agent-messages}.tsx`（改） | `@tauri-apps/plugin-opener`（`openUrl`）、既有 `TextBlock` | React + 既有卡管线 |
| 管理页 | `webSearch` 勾选项（协议门控 + 切走清零）+ 载荷补位 | `views/agents/components/provider-panel.tsx`、`views/agents/hooks/use-agent-providers.ts`（改） | `@base-ui/react/checkbox`（既有依赖，见 D18） | 既有表单驱动 |
| 壳层外链 | 插件注册 + 最小权限面 | `src-tauri/Cargo.toml`、`src-tauri/src/main.rs`、`src-tauri/capabilities/default.json`（改） | `tauri-plugin-opener = "2"` | Tauri 2 插件机制 |

> **D18（补记）**：勾选控件用既有依赖 `@base-ui/react/checkbox`（`Checkbox.Root` + `Checkbox.Indicator`）在 `provider-panel.tsx` 内直接消费——`components/ui/` 是 shadcn 生成面、`components/standard/` 是业务标准件面，两者本变更均不新增文件（proposal「checkbox 用现有 shadcn 控件，不新增标准组件」）；`data-testid="provider-web-search"` 落在 `Checkbox.Root`。

---

## 变更清单

> 路径口径：新增文件表用仓库根相对全路径；修改文件表与 proposal「变更范围 - 实现文件」同口径（`crates/…` 前缀省略 `packages/desktop/src-tauri/`，`src/…` 指 `packages/desktop/src-tauri/src/…`）。测试文件由测试相位承接，不在本清单。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/web_search.rs` | 常量面（`WEB_SEARCH_TOOL_KIND` / `WEB_SEARCH_TOOL_NAME` / `WEB_SEARCH_MAX_USES` / `ANTHROPIC_RAW_CONTENT_KEY` / `CITATIONS_KEY`）+ 六个纯函数（`provider_tool` / `raw_content` / `citations_of` / `project_choice` / `replay_raw` / `replay_text`）；模块 doc 记录「raw 必须配空文本」「展示字段不参与回放」两条不变量与 D2/D3/D4 | 公共函数·API 表 `web_search::*` 六行；修改文件 `sdk/mod.rs` 行 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/agent/src/event.rs` | ① `AgentBlock` 增 `WebSearch { id, query, results, error, raw_use, raw_result }` 变体（属性见 D1）；② `Text` 增 `citations`（D1）；③ 新增 `AgentCitation { url, title, cited_text, raw }` 与 `WebSearchSource { url, title, page_age }`（`#[serde(rename_all = "camelCase")]` + `specta::Type`）；④ 枚举 doc 由「四变体」改「五变体」，并注明 server 块与 citations 的抗失真语义；⑤ **属性面按第四稿收敛为 `#[serde(default)]` 单属性**：`citations` / `results` / `error` / `raw_use` / `raw_result` 五处 `skip_serializing_if` 全部删除（现工作树 `:36` / `:65` / `:68` / `:71` / `:74`），`citations` / `results` 的行内 doc 由「序列化不落键 / 空数组不落键」改为「**键恒在**：缺省落 `null`、空数组落 `[]`（bindings 统一形态的前提）」，`WebSearch` 变体 doc 补一句「展示字段与 raw 字段一律落键，值可 `null`」 | AC-1（缺省不落键子句按「规格与提案文案修订」改写）；`AgentDelta` / `AgentEventKind` **零改**（无 `AgentDelta::Search`，决策 ⑦ 机械判据）；**五处属性的删除是本轮回溯的唯一代码改动** |
| `crates/core/agent/src/lib.rs` | `pub use event::{…, AgentCitation, WebSearchSource};` | AC-1 导出面；其余导出行逐字不动 |
| `crates/core/orchestration/src/transcript.rs` | `final_assistant_text` 的 `AgentBlock::Text { text }` 模式改 `Text { text, .. }` | 模式补位（新字段不参与拼接）；`failure_reason` 零改 |
| `crates/infra/agent/src/sdk/mod.rs` | 增 `pub(crate) mod web_search;`（`#[cfg(test)] mod web_search_test;` 由测试相位随文件添加） | 避免本相位留下未定义测试模块 |
| `crates/infra/agent/src/sdk/loop.rs` | ① `LoopTurn` 增 `pub web_search: bool`；② 请求构造改 `let mut request = CompletionRequest { … }; if turn.web_search { request = request.provider_tool(web_search::provider_tool()); }`（`tools: definitions.clone()` 不变）；③ `'attempt` 返回值改 `(choice, usage, finish_reason)`（D6）；④ 用户提示词块的 `AgentBlock::Text` 字面量（`:137`）补 `citations: None`（新字段的编译必需点）；⑤ 内联块转换（`:317-340`）替换为 `web_search::project_choice(&choice)`；⑥ 新增 `PAUSE_TURN_MAX_CONTINUATIONS = 5` 常量与续跑循环（D5），含 `pause_turn_continued` / `pause_turn_capped` 两条 `SystemNotice`（payload `{ "continuations": n }`）；⑦ 续跑间隙 `handle.stop_requested()` 截停 | AC-3 / AC-4 / AC-6；`normalize` 调用点、`deny`、`outcome_result`、`finish` / `finish_success` / `finish_error` 逐字不动；`defend` 函数体逐字不动（调用点随嵌套循环保留在每次发请求之前） |
| `crates/infra/agent/src/sdk/resume.rs` | ① `text_content` 模式改 `Text { text, .. }`；② `assistant_content` 改为展开循环（D3）：`Text { text, citations }` → `replay_text(text, citations.as_deref())`，`WebSearch { raw_use, raw_result, .. }` → `replay_raw` 依次展开（双 `None` 跳过），`Thinking` / `ToolUse` 现状，assistant 位 `ToolResult` 仍跳过 | AC-5；`rebuild` 的配对 / 中断补齐逻辑零改 |
| `crates/infra/agent/src/sdk/context.rs` | `sweep_orphans` 扩 assistant 位 server 块配对（D7）；签名不变 | AC-7；三处调用点（`context.rs:116` / `:143` / `compact.rs:67`）零改自动生效；`UserContent` 侧配对逻辑逐字不动 |
| `crates/infra/agent/src/sdk/runner.rs` | ① `SdkRunner` 增 `web_search: bool`（`new` 内缺省 `false`，签名不动）；② 增 `with_web_search(self, bool) -> Self`；③ `PumpPayload` 增 `web_search: bool`；④ `open_session` 装填 payload；⑤ `session_pump` 构造 `LoopTurn` 时传递；⑥ 启动日志追加 `web_search={}`（与 `protocol=` / `window=` 同列） | AC-8；`build_model` / `anthropic_provider` / `resolve_output_tokens` 零改 |
| `crates/infra/agent/src/lib.rs` | ① `EngineFacade` 增 `web_search: bool` 字段；② 五处 `Self{}` 构造点补齐；③ 增 `with_web_search(self, web_search: bool) -> Self`；④ `runner_for` 的 sdk 臂追加 `.with_web_search(self.web_search)` | AC-8；`EngineKind` 双值与 `runner_for` 唯一 match 结构不塌 |
| `crates/infra/agent/src/compose.rs` | ① `ResolvedEngine` 增 `web_search: bool`；② `resolve_agent_engine` 内先 `let protocol = map_protocol(provider.protocol);` 再算 `web_search: provider.web_search && matches!(protocol, Protocol::Anthropic)`（在 `provider` 字段移动前，D8）；③ 门面链加 `.with_web_search(resolved.web_search)` | AC-8 三组映射；`EngineConfig` 字段面与 `is_complete` 零改 |
| `crates/infra/store/src/model.rs` | ① 现 `AgentProviderRecord`（v3）改名 `AgentProviderRecordV3`（`pub(crate)`、去 `#[native_db]` / `#[primary_key]`，D9）；② 新增当前 `AgentProviderRecord`（`version = 4, from = AgentProviderRecordV3`，增 `#[serde(default)] pub web_search: bool`，字段 doc 声明「是否启用 Anthropic 云端 web_search 服务端工具；缺省 false」）；③ V3→V4（补 `false`）与 V4→V3（丢列）双向 `From`；④ `AgentProviderRecord::new(...)` 七参不动、内部 `web_search: false`；⑤ 增 `with_web_search(self, web_search: bool) -> Self`；⑥ 手写 `Debug` 追加 `web_search` 位 | AC-8 / R12；`store.rs` 的 `define` 与读写方法因类型名不变而零改；`SessionEventRecord` 零改（AC-2） |
| `src/commands/agents/mod.rs` | `save_agent_provider` 末位追加 `web_search: bool`（第九参），构造后 `.with_web_search(web_search)`；`#[allow(clippy::too_many_arguments)]` 保留；doc 补一句「不因协议拒收，生效门在解析期（D10）」 | AC-8；既有校验（模型名后缀 / `max_tokens >= 4096` / 留空 api_key 回填）逐字保留 |
| `packages/desktop/package.json` | ① `version` `0.5.28 → 0.5.29`；② `dependencies` 增 `"@tauri-apps/plugin-opener": "^2.0.0"`（与 `plugin-dialog` 同形态） | AC-12 / AC-13 |
| `packages/desktop/src/types/generated/bindings.ts` | `pnpm -C packages/desktop run bindings:export` 再生成（非手改）：`AgentBlock` 五变体 + `citations`、`AgentCitation` / `WebSearchSource` 类型、`AgentProviderRecord.webSearch?: boolean`、`saveAgentProvider` 第九位置参。**第四稿口径**：产物为**统一形态**——单 `AgentBlock` / `AgentEvent` / `AgentEventKind` / `AgentRunMessage`，**零** `_Serialize` / `_Deserialize` 伴生类型；`AgentEvent` 仍是「对象 + `& AgentEventKind`」intersection，`agentStart` 首参仍是 `Channel<AgentRunMessage>`；成员形态 `citations?: AgentCitation[] \| null` / `results?: WebSearchSource[]` / `error?: string \| null` / `rawUse?: unknown \| null` 逐字不变 | AC-13 的 `bindings:check` 零 diff 基线；`skip_serializing_if` 若回流，本行产物即分裂 → golden 立刻红（机械判据） |
| `src/bindings/mod_test.rs` | **无代码改动**（核对结论，登记防误改）：四条 golden 断言逐字保留——`AgentEvent` 出线为对象形态 + `} & AgentEventKind;` 摊平（`:383`）、`agentStart: (onEvent: Channel<AgentRunMessage>,`（`:278`）、`agentStart` 十参同序含尾 `cwd`（`:317`）、产物不得含 `_Deserialize`（`:464`） | AC-13 + 第四稿：本轮回溯的**判据就是这四条断言零改动即全绿**（本相位已实测 33 项全过），因此 MUST NOT 通过改写 golden 来迁就 `skip_serializing_if` |
| `packages/desktop/src/lib/agent-adapter.ts` | ① `AgentToolOutput` 增可选位 `search?: { results: WebSearchSource[]; error: string \| null }`；② `BlockPlan` 的 text 变体增 `citations?: AgentCitation[]`；③ `PairedOutcome` 联合 + `PairedResults` 类型替身（D11）；④ 两视野扩 server 块配对（`rawUse` 判别）；⑤ `planBlock` 增 `webSearch` 分支与 `text.citations`（`_exhaustive` 承接新变体）；⑥ `citationMetadata` helper（D12）；⑦ `plansToParts` / `textChunks` / `plansToChunks` 挂 metadata（`text-start` 与 `text-delta` 同一份；空数组不挂；`text-end` 不带） | AC-9；`eventToChunk` **零改**（含 `isConsumedResult` / `mergeableResults` 两 helper）；`agent-transport.ts` **零改** |
| `packages/desktop/src/components/agent/agent-blocks.tsx` | ① `TextBlock` 扩 `citations?: AgentCitation[]` 并在 markdown 下方挂 `CitationList`；② 新增私有 `ExternalLink`（仅 `http:`/`https:` 渲染 `<a>` + `preventDefault` → `openUrl` + `.catch(() => {})`）；③ 新增私有 `CitationList`（按 url 去重、`title ?? url ?? citedText`）；④ 新增导出 `WebSearchCard`（summary 恒「网络搜索」、`data-testid="block-web-search"`、body = 查询词 / 来源列表 / 错误码） | AC-10 / AC-12；`ToolPairCard` / `ThinkingBlock` / `isToolPart` 零改 |
| `packages/desktop/src/components/agent/tool-cards.tsx` | `DEFAULT_TOOL_CARDS['web_search'] = WebSearchCard`（自 `agent-blocks` 引入） | AC-10 注册表；`AskUserQuestion` 项与回退语义不变 |
| `packages/desktop/src/components/agent/agent-messages.tsx` | ① `useHistoryWindow` 的 `conversationMessages` 过滤追加 `message.parts.length > 0`（分页计数基于过滤后列表）；② `PartView` text 分支传 `citations`（D14，读 `providerMetadata?.agent?.citations`） | AC-10；`ToolPartView` / `ChatBubble` / 触顶装载零改 |
| `packages/desktop/src/views/agents/components/provider-panel.tsx` | ① `ProviderFormState` 增 `webSearch: boolean`、`EMPTY_FORM` 缺省 `false`；② `ProviderTextField` 排除 `webSearch`；③ `toFormState` 读 `editing.webSearch ?? false`；④ 仅 `protocol === 'anthropic'` 渲染勾选项（`data-testid="provider-web-search"` + 「Anthropic 服务端网页搜索（按次计费）」提示）；⑤ `onProtocol` 切离 `anthropic` 时就地清零（切回不恢复）；⑥ `submit` 恒携 `webSearch`（含 openai 下的 `false`） | AC-11；协议选择器 / `base_url` 提示 / `max_tokens` / 后缀提示零改 |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | `AgentProviderSaveInput` 增 `webSearch: boolean`；`commands.saveAgentProvider(...)` 末位追加 `input.webSearch` | AC-11 invoke 位置参数逐位对齐 |
| `packages/desktop/src-tauri/Cargo.toml` | ① workspace `[workspace.dependencies]` 增 `tauri-plugin-opener = "2"`（与 `tauri-plugin-dialog` 相邻）；② 根 `[dependencies]` 增 `tauri-plugin-opener = { workspace = true }` | AC-12；`[target.'cfg(not(any(...)))'.dependencies]` 段零改（opener 跨平台） |
| `packages/desktop/src-tauri/src/main.rs` | builder 链在 `.plugin(tauri_plugin_dialog::init())` 之后追加 `.plugin(tauri_plugin_opener::init())` | AC-12「与 dialog 同位」；见「事实偏差 1」 |
| `packages/desktop/src-tauri/src/lib.rs` | **无代码改动**（核对结论）：builder 链不在本文件，插件注册落 `src/main.rs` | 覆盖 proposal 实现文件表的该行；见「事实偏差 1」 |
| `packages/desktop/src-tauri/capabilities/default.json` | ① `permissions` 追加 `{ "identifier": "opener:allow-open-url", "allow": [{ "url": "http://*" }, { "url": "https://*" }] }`；② `description` 改「查询 IPC、文件夹选择器与外链打开（仅 http(s)）；无 fs / shell 等其他权限」 | AC-12；scope 形态细节见 D15 / 待决问题 1 |
| `packages/desktop/src-tauri/Cargo.lock` | 由 `cargo` 随 `tauri-plugin-opener` 依赖解析刷新（非手改） | D16；proposal 未列，属工具生成产物 |
| `packages/desktop/pnpm-lock.yaml` | 由 `pnpm install` / `pnpm add` 随 `@tauri-apps/plugin-opener` 刷新（非手改） | D16；proposal 未列，属工具生成产物 |

### 回溯修订受影响的既有断言（测试相位随动，不在本清单改）

> 第五处「删属性」的唯一代价是**序列化不再省略缺省键**，故既有断言里凡以「不落键」为口径者必须改写。下表只登记**口径**，具体用例改写由测试相位承接。

| 断言 / 用例 | 位置 | 随动口径 |
|---|---|---|
| `web_search变体缺省键省略且往返等价`（断言 `results` / `error` / `rawUse` / `rawResult` 四键不落线格式） | `crates/core/agent/src/event_test.rs:606-628` | 改为「缺省键**恒在**」：`results = []`、`error` / `rawUse` / `rawResult` = `null`；往返等价语义保留 |
| `text块旧json无citations键读出none且再序列化不落该键` | 同上 `:666-683` | 拆成两半：「旧 JSON 读出 `None`」保留；「再序列化落 `citations: null`」（替代「不落键」） |
| `project_choice` 错误 result 用例的「空 results 不落键」断言 | `crates/infra/agent/src/sdk/web_search_test.rs:485-490` | 改为「`results = []` 落键、`error` 为错误码」 |
| `citations: None` → `additional_params.is_none()`（回放旁路不挂空壳） | `crates/infra/agent/src/sdk/resume_test.rs:795-805` | **零改动**（rig 回放侧语义，与本轮 serde 属性无关；登记防误改） |
| `src/bindings/mod_test.rs` 四条 golden 断言 | `packages/desktop/src-tauri/src/bindings/mod_test.rs:278/317/383/464` | **零改动**（本轮回溯的判据；已实测全绿） |

<!-- 无新增目录 / 无新增 crate：本变更全部落在既有文件与一个既有模块目录下（sdk/）。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `provider_tool` | `crates/infra/agent/src/sdk/web_search.rs` | 新增 | `pub(crate) fn provider_tool() -> ProviderToolDefinition` | `ProviderToolDefinition::new(WEB_SEARCH_TOOL_KIND).with_config("name", json!(WEB_SEARCH_TOOL_NAME)).with_config("max_uses", json!(WEB_SEARCH_MAX_USES))`——落 `additional_params.tools` 后形状恰为 `[{type,name,max_uses}]` |
| `raw_content` | 同上 | 新增 | `pub(crate) fn raw_content(text: &Text) -> Option<&Value>` | 取 `additional_params[ANTHROPIC_RAW_CONTENT_KEY]`；`None` = 普通文本块 |
| `citations_of` | 同上 | 新增 | `pub(crate) fn citations_of(text: &Text) -> Option<Vec<AgentCitation>>` | 空 / 缺失 → `None`；`raw` 取原值（D4） |
| `project_choice` | 同上 | 新增 | `pub(crate) fn project_choice(choice: &[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)` | 块投影唯一实现（D2） |
| `replay_raw` | 同上 | 新增 | `pub(crate) fn replay_raw(raw: &Value) -> AssistantContent` | `Text{text:"", additional_params:{anthropic_content: raw}}`（D3） |
| `replay_text` | 同上 | 新增 | `pub(crate) fn replay_text(text: &str, citations: Option<&[AgentCitation]>) -> AssistantContent` | 非空 citations → `additional_params[CITATIONS_KEY] = [c.raw, …]`；否则纯 `Text` |
| `sweep_orphans` | `crates/infra/agent/src/sdk/context.rs` | 修改（语义扩展，签名不变） | `pub(crate) fn sweep_orphans(history: Vec<Message>) -> Vec<Message>` | 追加 assistant 位 server 块清扫（D7） |
| `EngineFacade::with_web_search` | `crates/infra/agent/src/lib.rs` | 新增 | `pub fn with_web_search(self, web_search: bool) -> Self` | 与 `with_max_tokens` 同型旁路缝（D8） |
| `SdkRunner::with_web_search` | `crates/infra/agent/src/sdk/runner.rs` | 新增 | `pub fn with_web_search(self, web_search: bool) -> Self` | `new` 签名不动（调用点零改） |
| `AgentProviderRecord::with_web_search` | `crates/infra/store/src/model.rs` | 新增 | `pub fn with_web_search(self, web_search: bool) -> Self` | `new` 七参不动的唯一实现路径（D9） |
| `save_agent_provider`（IPC 命令 `save_agent_provider`） | `src/commands/agents/mod.rs` | 修改 | `pub fn save_agent_provider(stores: State<'_, WorkspaceStores>, id: Option<i64>, name: String, protocol: AgentProviderProtocol, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>, max_tokens: Option<u64>, web_search: bool) -> Result<AgentProviderRecord, String>` | 末位第九参；零协议判定（D10） |
| `TextBlock` | `packages/desktop/src/components/agent/agent-blocks.tsx` | 修改 | `export function TextBlock({ text, citations }: { text: string; citations?: AgentCitation[] }): React.JSX.Element` | 引用列表挂正文底部（D13） |
| `WebSearchCard` | 同上 | 新增 | `export function WebSearchCard({ toolCallId, toolName, input, output }: { toolCallId: string; toolName: string; input: unknown; output: AgentToolOutput \| null }): React.JSX.Element` | summary 恒「网络搜索」；唯一新增导出的组件（被 `tool-cards.tsx` 消费） |
| `DEFAULT_TOOL_CARDS` | `packages/desktop/src/components/agent/tool-cards.tsx` | 修改 | `export const DEFAULT_TOOL_CARDS: Record<string, AgentToolCard>` | 追加 `web_search` 键 |

> 前端其余改动（`citationMetadata` 辅助、`citationsOfPart`、`ExternalLink` / `CitationList`）均为**文件内私有**，按 D13 不导出。

<!-- CLI 子命令：无（本变更不涉及 CLI）。HTTP 端点：无（见「路由 / API 设计」）。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `AgentBlock` | `crates/core/agent/src/event.rs` | 修改 | 四变体 → 五变体（增 `WebSearch`），`Text` 增 `citations`；缺省字段的属性面**只** `#[serde(default)]`（`skip_serializing_if` 撤销，D1 第四稿）→ 线格式键恒在、TS 面可选位 |
| `AgentCitation` | 同上 | 新增 | `{ url: Option<String>, title: Option<String>, cited_text: Option<String>, raw: Value }`；`raw` = 原始 wire 值（回放用），其余为展示字段 |
| `WebSearchSource` | 同上 | 新增 | `{ url: String, title: Option<String>, page_age: Option<String> }`；单条搜索来源（纯展示） |
| `LoopTurn` | `crates/infra/agent/src/sdk/loop.rs` | 修改 | 增 `pub web_search: bool` |
| `PumpPayload` | `crates/infra/agent/src/sdk/runner.rs` | 修改 | 增 `web_search: bool` |
| `SdkRunner` | 同上 | 修改 | 增私有字段 `web_search: bool` |
| `EngineFacade` | `crates/infra/agent/src/lib.rs` | 修改 | 增私有字段 `web_search: bool` |
| `ResolvedEngine` | `crates/infra/agent/src/compose.rs` | 修改 | 增 `web_search: bool`（已门控的最终开关） |
| `AgentProviderRecordV3` | `crates/infra/store/src/model.rs` | 新增（由现 `AgentProviderRecord` 改名） | v3 历史形态，`pub(crate)`，无 `#[native_db]` / `#[primary_key]`，升级链中转（D9） |
| `AgentProviderRecord` | 同上 | 修改 | v4：增 `#[serde(default)] web_search: bool`；`native_model(id = 5, version = 4, from = AgentProviderRecordV3)` |
| `AgentToolOutput` | `packages/desktop/src/lib/agent-adapter.ts` | 修改 | `{ content: string; isError: boolean; search?: { results: WebSearchSource[]; error: string \| null } }`——加位可选，既有消费面零改 |
| `BlockPlan` | 同上 | 修改 | text 变体增 `citations?: AgentCitation[]`；tool 变体承载搜索（`toolName: 'web_search'`、`input: { query } \| null`）；**不新增第四变体** |
| `PairedOutcome` | 同上 | 新增（私有） | `{ kind: 'toolResult'; block: ToolResultBlock } \| { kind: 'webSearch'; results: WebSearchSource[]; error: string \| null }` |
| `PairedResults` | 同上 | 修改 | `Map<string, PairedOutcome>`（两视野同形态，D11） |
| `ProviderFormState` | `packages/desktop/src/views/agents/components/provider-panel.tsx` | 修改 | 增 `webSearch: boolean`；`ProviderTextField` 排除该键 |
| `AgentProviderSaveInput` | `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | 修改 | 增 `webSearch: boolean`（末位） |
| `AgentBlock`（TS 镜像） | `packages/desktop/src/types/generated/bindings.ts` | 再生成 | 五变体 + `citations`；同时生成 `AgentCitation` / `WebSearchSource` 两个类型；**单形态**（无 `_Serialize` / `_Deserialize`）：`citations?: AgentCitation[] \| null`、`results?: WebSearchSource[]`、`error?: string \| null`、`rawUse?: unknown \| null` / `rawResult?: unknown \| null`（可选位来自 `#[serde(default)]`，D1 第四稿） |

<!-- 无 interface 形态的公共 API class：Rust 侧三个门面/记录类型的方法已在上表逐条列出。 -->

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `WEB_SEARCH_TOOL_KIND` | `crates/infra/agent/src/sdk/web_search.rs` | 新增 | `&str` 常量 | `"web_search_20250305"` | 请求侧 `type`；字面量单点（loop / resume 零重复） |
| `WEB_SEARCH_TOOL_NAME` | 同上 | 新增 | `&str` 常量 | `"web_search"` | 请求侧 `name` + `server_tool_use` 的配对判别值 |
| `WEB_SEARCH_MAX_USES` | 同上 | 新增 | `u32` 常量 | `5` | **单请求**搜索次数上限（成本护栏）；**不可配置**（配置化需 store v5） |
| `PAUSE_TURN_MAX_CONTINUATIONS` | `crates/infra/agent/src/sdk/loop.rs` | 新增 | `u32` 常量 | `5` | **单轮**续跑上限；与 `WEB_SEARCH_MAX_USES` 分名分注释分日志字段（R3） |
| `ANTHROPIC_RAW_CONTENT_KEY` | `web_search.rs` | 新增 | `&str` 常量 | `"anthropic_content"` | rig 的 raw server 块承载键（与 rig 内部常量同字面量，注释注明来源） |
| `CITATIONS_KEY` | 同上 | 新增 | `&str` 常量 | `"citations"` | rig 的 citations 承载键 |
| `web_search` | `crates/infra/store/src/model.rs` | 新增 | 记录列 `bool` | `false` | provider 属性；`#[serde(default)]`；不参与保存期协议校验 |
| `webSearch`（IPC 载荷位） | `src/commands/agents/mod.rs` / `bindings.ts` | 新增 | `boolean` | `false` | 命令第九位置参；载荷恒携（openai 下为 `false`，不省略） |
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.29`（原 `0.5.28`） | 用户可见功能变更上抬 |
| `@tauri-apps/plugin-opener` | 同上（`dependencies`） | 新增 | semver 串 | `^2.0.0` | 前端外链调用依赖 |
| `tauri-plugin-opener` | `packages/desktop/src-tauri/Cargo.toml` | 新增 | semver 串 | `"2"` | workspace + 根 crate 两处（与 `tauri-plugin-dialog` 同形态） |
| `permissions[]`（opener 项） | `packages/desktop/src-tauri/capabilities/default.json` | 新增 | capability 条目 | `{ "identifier": "opener:allow-open-url", "allow": [{ "url": "http://*" }, { "url": "https://*" }] }` | scope 只放 http(s)；MUST NOT 引 `opener:default` / `allow-default-urls`（D15） |
| `description` | 同上 | 修改 | `string` | 「查询 IPC、文件夹选择器与外链打开（仅 http(s)）；无 fs / shell 等其他权限」 | 与权限实况一致（AC-12） |

---

## 规格与提案文案修订（第四稿定稿，随实现任务同步）

> 本轮回溯只改**属性面与文案**，不改任何运行时语义。下列文案若不改，归档后的 spec / proposal 会与实现互相矛盾（spec 明文写着 `skip_serializing_if`），故与代码改动同批落地。逐条为定稿口径，`tasks.md` 阶段 13 第 3 项执行。

| 位置 | 现文（需替换） | 改为 |
|------|----------------|------|
| `proposal.md` AC-1 | 「旧 `Text` JSON（无 `citations`）读出 `None`，序列化时 `None` / 空 `results` 不落键」 | 「旧 `Text` JSON（无 `citations`）读出 `None`；序列化缺省时 `citations` / `error` / `raw_use` / `raw_result` 落 `null`、`results` 落 `[]`——**键恒在**（bindings 统一形态的前提，见 design D1）」 |
| `proposal.md`「六面收口」第 1 条 | `（#[serde(default, skip_serializing_if = "Option::is_none")]）` | `（仅 #[serde(default)]：键恒在、值可为 null；MUST NOT 加 skip_serializing_if——unified 相位模型无法表达条件省略，加则 bindings 分裂）` |
| `proposal.md`「变更范围 - 实现文件」`event.rs` 行 | 同上属性描述 | 同上改写 |
| `proposal.md`「变更范围 - 测试文件」event_test 行 | 「`results` 空数组不落键」 | 「缺省键落 `null` / `[]`」 |
| `specs/desktop-agent-execution/spec.md` 场景「旧事件读出兼容」（`:74-75`） | 「序列化 `None` 与空 `results` 时不落该键（前端既有 fixture 零改动）」 | 「序列化缺省时 `citations` 落 `null`、`results` 落 `[]`（前端既有 fixture 零改动——TS 字段仍为可选位）」 |
| 同上「AgentEvent 信封与 loop 可见性」正文（`:150`） | `citations` 为 `#[serde(default, skip_serializing_if = "Option::is_none")]` | `citations` 为 `#[serde(default)]`（**MUST NOT** 加 `skip_serializing_if`：unified 相位模型无法表达条件省略，加则 bindings 分裂为 `_Serialize` / `_Deserialize`，仓库 golden 契约不允） |
| 同上（`:151`） | 「`results` / `raw_*` 为 `#[serde(default, skip_serializing_if = …)]`」 | 「`results` / `error` / `raw_use` / `raw_result` 为 `#[serde(default)]`（键恒在）」 |
| 同上「Function: event.rs」表（`:260`） | 同 `:150` 写法 | 同 `:150` 改写 |

---

## 数据模型

### 事件块（落库形态 = 线格式）

```
AgentBlock (tag = "kind", camelCase)                                    ← 线格式：键恒在（缺省为 null / []）
├─ text        { text: String, citations: AgentCitation[] | null }
├─ thinking    { thinking: String }
├─ toolUse     { id, name, input: Value }                      ← 客户端工具（不变）
├─ toolResult  { id, content, isError }                        ← 客户端工具结果（不变）
└─ webSearch   { id, query, results: WebSearchSource[], error: String | null,
                 rawUse: Value | null, rawResult: Value | null }

AgentCitation      { url?, title?, citedText?, raw: Value }             ← 线格式键恒在，值可 null
WebSearchSource    { url: String, title?: String, pageAge?: String }
```

> **线格式 vs TS 面**：Rust 侧五处缺省字段只标 `#[serde(default)]`（无 `skip_serializing_if`，D1 第四稿），故**序列化一律落键**（`null` / `[]`）；specta unified mode 依 `default` 把 TS 字段出为**可选位**（`citations?: AgentCitation[] | null` 等），前端既有 fixture 与 `agent-adapter.ts` 的可空位消费零改动。此差异是绑定统一形态的代价，见「事实偏差 3」。

- **关系**：`webSearch.id` = `server_tool_use.id` = 配对 `web_search_tool_result.tool_use_id`（同一键贯穿投影、配对、清扫、前端合并）。
- **一次搜索 = 一块**：`pause_turn` 把 use 与 result 拆进两条 assistant 消息时，投出两块（use 侧 `rawUse` 有值、result 侧 `rawUse` 为 `None`），两者同 `id`；前端按 `id` 合并，转录仍是两块。
- **持久化**：`SessionEventRecord.event` 为 `SerdeJsonCodec` 自描述 JSON（`crates/infra/store/src/model.rs:549-560`），新增变体与带默认值字段**零 schema 版本升级**；`rawUse` / `rawResult` 逐字节保真往返（AC-2）。旧客户端读到 `webSearch` 块会在**整条事件解码**时失败（边界条 15，不可回退）。

### provider 记录（全局库，native_model id = 5）

```
v1 { id, name, base_url, api_key, models }
 └─ v2 { … + context_length: Option<u64> }
     └─ v3 { … + protocol: AgentProviderProtocol, max_tokens: Option<u64> }   → 改名 AgentProviderRecordV3（pub(crate)）
         └─ v4 { … + web_search: bool (#[serde(default)]) }                   → 当前 AgentProviderRecord
```

- **升级**：V3→V4 补 `web_search = false`（存量行行为零变化，无迁移脚本）；**降级**：V4→V3 丢该列（降级形态不作数据承诺，沿既有 `From` 注释口径）。
- **写入面**：`upsert_agent_provider` 的 name 查重 / max+1 主键 / 行存在校验逐字不动（类型名不变）；`web_search` 为普通 `bool` 列，不参与查重或校验。
- **会话快照不记该字段**：`SessionConfigSnapshot` 字段面不动——开关是 provider 属性，续轮按 provider 当前值生效（与改 `base_url` / `api_key` 同权）。

### 请求/回放形状（wire 事实）

```
请求（开关开）    additional_params.tools = [{type:"web_search_20250305", name:"web_search", max_uses:5}]
请求（开关关）    additional_params 无 tools 键
回放 use 侧      AssistantContent::Text{ text:"", additional_params:{ anthropic_content: <raw server_tool_use> } }
回放 result 侧   AssistantContent::Text{ text:"", additional_params:{ anthropic_content: <raw web_search_tool_result> } }
回放正文+引用    AssistantContent::Text{ text:<原文本>, additional_params:{ citations:[<raw>, …] } }
```

---

## 路由 / API 设计

<!-- 不涉及：本变更不新增 HTTP 路由 / 端点，也不新增命令名。命令面变化只有既有 save_agent_provider 的位置参追加，其 method / path 等价面为 Tauri IPC invoke("save_agent_provider")，签名见「公共函数 / API」表；auth 面沿用既有 IPC（本地壳内应用，无鉴权层）。 -->

---

## 依赖

### 运行时依赖

| 依赖 | 版本 | 用途 | 落点 |
|------|------|------|------|
| `rig`（workspace 既有 0.43.0） | 不升版 | `ProviderToolDefinition` / `Text::additional_params` / `FinishReason::Other` / `CompletionResponse::finish_reason()`——**源码级复核确认两半能力齐备，无需打补丁** | `crates/infra/agent`（既有） |
| `serde` / `serde_json` | 既有 | 新块词汇与 raw 载荷 | `crates/core/agent`、`crates/infra/agent`（既有） |
| `native_model` / `native_db` | 既有 | provider 记录 v4 版本机制 | `crates/infra/store`（既有） |
| `tauri-plugin-opener` | `2` | 壳侧外链打开（`openUrl` 的宿主实现） | `src-tauri/Cargo.toml`（新增） |
| `@tauri-apps/plugin-opener` | `^2.0.0` | 前端 `openUrl` 调用（唯一调用点在 `ExternalLink`） | `packages/desktop/package.json`（新增） |
| `@base-ui/react`（既有） | 既有 | `Checkbox.Root` 勾选控件（D18） | `provider-panel.tsx`（既有依赖） |
| `ai`（既有 `^7`） | 既有 | `ProviderMetadata` / `JSONValue` 类型（citations 挂载） | `agent-adapter.ts`（既有） |

### 构建 / 测试依赖

| 依赖 | 用途 | 备注 |
|------|------|------|
| `export-bindings`（workspace 既有 crate） | `bindings:export` 再生成 `bindings.ts` | 无新增 |
| `cargo` / `pnpm` 网络可达 | 新增 Rust crate 与 npm 包的首次解析（`Cargo.lock` / `pnpm-lock.yaml` 随动） | 离线环境需先联网安装（待决问题 4） |

> 零新增 crate 依赖边（`core/agent` 只增块词汇与值类型，不认 rig / anthropic；`desktop-crate-layout` / `module-boundary-contracts` 零改）。

---

## 验收标准回应

> **验收面口径**：验收止于**单元测试 + 静态检查链**（AC-1..AC-13 的自动化面）。真机 E2E（官方 Anthropic 端点 + 凭据 + 真实 `pause_turn`）不属本工作流自动化验证面，以 explore「验证」节的手工清单承载，并入**归档后人工验收**执行；其中「相邻 assistant 消息是否被服务端接受」按 R5 记作**已知限制**（边界条 16）——MUST NOT 被转写为任务清单的前置门或阻塞项，其结论不阻塞实现与归档。

| AC | 设计落点 |
|----|----------|
| AC-1 | `event.rs` 行（`WebSearch` 变体 + `Text.citations` + 两值类型，属性见 D1）；`lib.rs` 导出 `AgentCitation` / `WebSearchSource`；「数据模型 - 事件块」图给出键名与「线格式键恒在 / TS 面可选」两口径。**第四稿裁定**：AC-1 的「序列化时 `None` / 空 `results` 不落键」子句改为「缺省落 `null` / `[]`」（`skip_serializing_if` 与 bindings 契约不可同时成立，见「事实偏差 3」与「规格与提案文案修订」）；「旧 `Text` JSON 读出 `None`」与导出面**不变** |
| AC-2 | 事件转录零改（`SessionEventRecord` 走 `SerdeJsonCodec`，`model.rs:549-560`）——新变体与带默认值字段不加版本；`rawUse` / `rawResult` 为 `Value` 原样往返（D3 展示/回放分家保证不会被改写） |
| AC-3 | `web_search::provider_tool`（D2 + 公共函数表）+ loop 请求构造按需 `provider_tool(..)`；`tools: definitions.clone()` 与 `RunStarted.tools` 零改；`PAUSE_TURN_MAX_CONTINUATIONS` 与 `WEB_SEARCH_MAX_USES` 两常量分名（配置表）；`normalize.rs` / `agent-transport.ts` 记「零改」（机械判据） |
| AC-4 | `project_choice` 四规则（D2：use+result 收敛 / 孤儿 result / 未知 server 块跳过 / 文本与既有三变体）；`results` 取 `web_search_result` 三字段、`error` 取 `error_code`（D2 正文）；`id` 取 `server_tool_use.id` |
| AC-5 | `replay_raw` 空文本约束 + `replay_text` 的 `anthropic_content`/`citations` 原值回填（D3/D4）；`resume::assistant_content` 展开保原序、双 `None` 跳过、assistant 位 `ToolResult` 仍跳过；展示字段不参与重建（类型上只吃 `Value`） |
| AC-6 | 续跑嵌套循环与分支次序（D5）；`finish_reason` 精确判据（D6）；`attempt` 每轮次归零（续跑不吃重试预算）；`usage` 逐次累加（`usage += turn_usage` 位置在分支之前）；到顶发 `pause_turn_capped` 后走既有 `finish_success`（`stop` 语义、非 `api_error`）；空 choice 先于续跑判定、`tool_calls` 非空不续跑 |
| AC-7 | `sweep_orphans` 扩展（D7）：单向前序扫描、剔 result 侧孤儿、剔空整条移除；三处调用点（L2 / L3 / 摘要置换）签名不变即生效；`UserContent` 侧逐字不动（既有用例零回归） |
| AC-8 | store v4（D9）+ 命令第九参零协议判定（D10）+ 组合根门控单点（D8：`provider.web_search && protocol == Anthropic`）+ 门面五处构造点补齐 + runner 传递 + 启动日志；`EngineConfig` 字段面与 `runner_for` 唯一 match 不动；`new` 七参不动（38 处调用点零改） |
| AC-9 | `AgentToolOutput.search?` 加位 + `BlockPlan` 两扩位（类型表）+ `PairedOutcome` 联合与两视野扩配对（D11）+ `eventToChunk` / `agent-transport.ts` 零改；citations 挂 `agent` 命名空间、空数组不挂、`text-start` / `text-delta` 同 metadata（D12） |
| AC-10 | `WebSearchCard` summary 恒「网络搜索」（无状态文案）/ body 三区（D13 + `agent-blocks.tsx` 行）；`CitationList` 按 url 去重、`title ?? url`；`ExternalLink` 非 http(s) 纯文本；注册表 `web_search` 项；`useHistoryWindow` 追加 `parts.length > 0`（D14 + `agent-messages.tsx` 行，分页计数基于过滤后列表） |
| AC-11 | `ProviderFormState.webSearch` / `ProviderTextField` 排除 / 仅 anthropic 渲染 / 切走清零 / submit 恒携（`provider-panel.tsx` 行六条）；`AgentProviderSaveInput` 与 invoke 末位（`use-agent-providers.ts` 行） |
| AC-12 | 插件注册于 `src/main.rs`（与 `dialog` 同位）+ 两侧依赖（`Cargo.toml` 行）+ capability 仅 http(s)（D15 + `default.json` 行）+ 调用点收敛 `ExternalLink`（D13）；失败 `.catch(() => {})` 不上抛（`agent-blocks.tsx` 行 ②） |
| AC-13 | 版本 `0.5.29`（配置表）；`bindings.ts` 再生成（修改文件行）——**第四稿起产物必须回到统一形态**（零 `_Serialize` / `_Deserialize`，`src/bindings/mod_test.rs` 四条 golden 断言零改动即全绿，本相位已实测 33 项全过）；`Cargo.lock` / `pnpm-lock.yaml` 随动（D16）；静态检查链与全量测试由桌面端执行（本相位不重复执行） |

---

## 构建顺序约束（D19）

本变更的 Rust 侧改动**不得**在全部构造点补齐前要求编译通过——两处独立的不编译成因共存，且各自的补齐点分属不同阶段：

| 成因 | 断裂点在 | 补齐点在 |
|------|----------|----------|
| `AgentBlock::Text` 增 `citations`（阶段 1） | `sdk/loop.rs`（字面量 + 内联转换）、`sdk/resume.rs`（两处 `Text` 模式）、`core/orchestration/transcript.rs`（模式） | 阶段 3（loop）与阶段 4（resume / transcript） |
| `LoopTurn` 增 `web_search`（阶段 3） | `sdk/runner.rs:316-329` 的 `LoopTurn { … }` 字面量（唯一生产构造点，缺字段即编译失败） | 阶段 5（`session_pump` 传递 `payload.web_search`） |

裁定：**`cargo check`（默认目标）单点落在阶段 5 末尾**（`runner.rs` 构造点补齐之后），阶段 4 末只做源码级残留扫描；阶段 11 末尾（最后一处 Rust 改动）复核一次。理由：把门放在阶段 4 末会与「`LoopTurn` 字段的有效构造点」跨阶段，该门必失败；把 `runner.rs` 的传递前移到阶段 3 会让阶段 3 夹带开关链路的一半（`PumpPayload` 尚未有字段），造出两处半成品。任务拆解与门位以此为准（见 `tasks.md` 顶部不变量 1）。

**第五顺序约束（第四稿补）**：阶段 13（回溯修订）**不引入新的编译断点**——改动是「删 5 处属性 + 行内 doc + 文案」，Rust 侧只在 `event.rs`；落地后 `cargo check` 与 `bindings:export` 各复核一次即可（`src/bindings/mod_test.rs` 零改动即全绿，是本阶段的验收判据）。**契约优先级**：golden 契约（仓库级、覆盖全体 DTO）高于本变更的线格式紧凑（局部收益），故冲突时一律改属性、不改 golden。

---

## 待决问题

1. **opener 权限标识符与 scope glob 形态**（D15；**已核实结项**）：按 `tauri-plugin-opener` 2.x 自带的 `permissions/autogenerated/reference.md` 与 `src/scope.rs` 核实——①`opener:allow-open-url` 存在且不预置任何 URL 集；②`opener:allow-default-urls`（`opener:default` 含之）连带放开 `mailto:` / `tel:`，**必须排除**；③`allow` 条目经 `EntryRaw::Url { url, app }` 反序列化为 `glob::Pattern`，`*` 在默认 `MatchOptions`（`require_literal_separator = false`）下跨 `/` 匹配，故 `http://*` / `https://*` 覆盖整条 URL。结论：D15 定稿形态即正确形态，落地为 `capabilities/default.json` 的 `opener:allow-open-url` + 两条 http(s) scope，**无「只提供宽权限项」的回落情形**。
2. **`agent-messages.tsx` 的改动范围**（D14）：proposal 对 `agent-messages.tsx` 只列「空 parts 过滤」，本设计追加了一处 `PartView` → `TextBlock` 的 `citations` 传递（3 行）。理由：`CitationList` 必须挂在 text 部件底部，而部件分发唯一在 `PartView`。**已按 D14 形态落地**（`PartView` 读 `providerMetadata?.agent?.citations` 后传入 `TextBlock`）；替代方案（把读取挪进 `agent-blocks.tsx`）未采用，形态差异不影响 AC-10，留此条仅为提案行文的偏差留痕。
3. **同消息形态的 `output` 来源**（D11）：「use 侧块自带 `rawResult`（`pause_turn` 未拆分时 use 与 result 同处一条消息）」这一形态，规格只写了「配对命中携 `output`」，未点名自含形态。本设计裁定为「自身 `rawResult` 优先」，否则该形态会退化为「只有查询词」，且 `desktop-agent-execution` 的投影规则已保证未拆分时 use+result 同块携带 `raw_result`。**已按该优先级落地**；规格侧尚未显式写明该句（记为规格补足口径，不改实现）。
4. **依赖获取需网络**（**已满足**）：`tauri-plugin-opener`（crates.io）与 `@tauri-apps/plugin-opener`（npm）首次解析需联网；两侧依赖已落地、`Cargo.lock` / `pnpm-lock.yaml` 已随动刷新，静态检查链的前置条件成立。
5. **相邻 assistant 消息的归档后人工验收事项**（D17；**非待决阻塞项**）：`pause_turn` 天然产生「相邻同角色 assistant 消息」，rig 编码侧不做同角色合并，故回放依赖 Anthropic 服务端接受该形态。本期无官方端点凭据，该依赖以**已知限制**留痕于 `desktop-agent-execution` 边界条 16，确认动作落**归档后人工验收**（真实 `pause_turn` 会话续轮确认服务端接受，结论记入验收记录），**不阻塞实现与归档**。若人工验收确认被拒，降级预案为「回放时按 `pause_turn` 边界合并相邻 assistant 消息」——本期不预建、无实现分支，后续变更据边界条 16 偿还（与待决问题 6 / 7 同属知边界留痕，不构成本期开工条件）。
6. **未知 raw server 块（`code_execution_tool_result` 等）的跨轮保真缺口**：这类块在轮内 `choice` 入史时原样保留（同轮请求可再发），但不产 `AgentBlock` → 不入转录 → **续轮丢失**。属决策 ⑤ 的知情边界（「其他 raw server 块跳过」），本设计不扩面；建议后续变更在「SDK 租户 MVP 边界」补一条留痕，避免被误读为 bug。
7. **`sweep_orphans` 不识别 `code_execution_tool_result`**：本变更只按 `web_search_tool_result` 判孤儿（spec 指定的索引源）。若未来接入其他 server 工具，需按同一机制扩面；本期表内不预建分支。
8. **条件省略语义的偿还路径**（D1 第四稿；**本期不做，非阻塞项**）：若未来确需「缺省不落键」的线格式，需 specta 侧支持 unified 模式的条件省略（当前 `validate.rs:544` 硬拒），或为 `AgentBlock` 手写统一 `Type` impl 并自承「serde 实际省略 / 绑定声称恒在」的一致性风险；两条路径均不属本期，本期以「键恒在」定稿。线格式体积增量（每个 text 块多 `citations: null`、每个 webSearch 块多 `results: []` 与三个 `null`）与 R10「不设上限、不压缩」同列，量级可忽略。
