# 测试设计: anthropic-web-search

> **变更**: anthropic-web-search
> **角色**: test-design-planner（executor）
> **输入**: `proposal.md`（AC-1 – AC-13）+ `design.md`（变更清单 / D1 – D19 / 公共函数·API 表 / 类型定义表 / 配置表 / 验收标准回应）+ `specs/{desktop-agent-execution,desktop-agent-management,desktop-agent-chat-infra,desktop-app-shell}/spec.md`
> **范围**: 仅设计单元测试范围与用例形态，不写测试命令细节。静态脚本（`bindings:check` / `cargo clippy` / `knip` / `run test` 汇总）由桌面端执行，本文件不复制运行脚本体。
> **对齐修订（design 第三稿）**: design 已撤销真机核验的实现期强制门（`D17` / 待决问题 5 改为「已知限制留痕 + 归档后人工验收」）。本文件同步该口径：验收止于**单元测试 + 静态检查链**（AC-1..AC-13 的自动化面）；真机 `pause_turn` 项落「不可测试项」，**不设实现期核验门、不作任务阻塞项**；模块列表与用例面零删减。
> **对齐修订（design 第四稿 / D1 翻转）**: `AgentBlock` 的 `skip_serializing_if` 五处全部撤销，只留 `#[serde(default)]`——线格式**键恒在**（缺省 `citations` / `error` / `rawUse` / `rawResult` 落 `null`、`results` 落 `[]`），以换取 bindings 统一形态（零 `_Serialize` / `_Deserialize`）。本文件据此改写三处断言口径（均按 design「回溯修订受影响的既有断言」表）：`event.rs` 章节的「缺省键省略」与「旧 JSON 再序列化不落键」两条、`web_search.rs` 章节的错误 result「空 results 不落键」一条；`resume.rs` 章节的回放旁路断言与「不可测试项」中的 `src/bindings/mod_test.rs` 四条 golden 断言**零改动**（登记防误改）。实现语义面零变化（D2–D19 不涉改），用例面零删减。

## 测试框架与测试区域识别

- **后端 `packages/desktop/src-tauri/`**：Rust `cargo test --workspace`。测试文件与源文件**同目录、同前缀 + `_test.rs`**，经 `#[cfg(test)] mod xxx_test;` 挂载（`crates/infra/agent/src/sdk/mod.rs` 侧挂 SDK 子模块；`crates/infra/agent/src/compose.rs` 用 `#[cfg(test)] #[path = "compose_test.rs"] mod compose_test;` 就地挂载——子模块可触达父模块私有项）。断言库为内置 `assert!` / `assert_eq!` / `matches!`；用例名沿用仓库既有中文函数名风格。既有装置：
  - `crates/infra/agent/src/sdk/loop_test.rs`：rig `test_utils`（`MockScript` / `MockFrame` / `MockStreamEvent`）+ 脚本化 `Transport` 假流缝，逐轮预录流事件 + `Mutex` 捕获 `CompletionRequest`——**请求形状断言（AC-3）与 `pause_turn` 脚本驱动（AC-6）的既有唯一落点**；`#[tokio::test]` 驱动 `loop::run`，事件面经 `mpsc` 观察者断言。
  - `crates/infra/agent/src/sdk/runner_test.rs` / `lib_test.rs`：真实 `SdkRunner` / `EngineFacade` + `base_url` 指向本机 discard 端口（`http://127.0.0.1:9/v1`，恒拒连、不打网络）；`tempfile` 数据根；引擎日志为进程内可读文件（`append_engine_log`）。
  - `crates/infra/store/src/model_test.rs`：`native_model::encode` / `decode` **内存往返**（版本链解码锚，不落库）；`envelope_test.rs` 为真库 + 信封读取面装置。
  - `src/commands/agents/mod_test.rs`：`tauri::test::mock_app()` + `app.manage(WorkspaceStores::open(tempdir))`，`#[tauri::command]` 原函数直调。
- **前端 `packages/desktop/src/`**：Vitest（`vite-plus/test` 提供 `describe` / `it` / `expect` / `vi`）；hook 用 `renderHook` + `act` / `waitFor`，组件用 `render` / `screen` / `fireEvent`（`@testing-library/react`）+ jsdom。测试文件与源文件同目录同前缀：`.ts` → `.test.ts`（hook / 纯函数）、`.tsx` → `.test.tsx`（组件）。
- **映射约定**：`xxx.rs` → 同目录 `xxx_test.rs`；`src/commands/<域>/mod.rs` → `src/commands/<域>/mod_test.rs`；`xxx.ts` → 同目录 `xxx.test.ts`；`xxx.tsx` → 同目录 `xxx.test.tsx`。
- **测试文件落点**：新增两个测试文件——`crates/infra/agent/src/sdk/web_search_test.rs`（新建，并在 `sdk/mod.rs` 挂 `#[cfg(test)] mod web_search_test;`）与 `packages/desktop/src/components/agent/agent-blocks.test.tsx`（新建，首行 `// @vitest-environment jsdom`）；其余 14 个映射对均在既有测试文件上扩位（`agent-blocks` 此前无测试文件）。
- **待测功能来源约定**：每个 `#### 待测功能` 条目均溯源到 design.md 的声明表，节内首行标注来源——`公共函数 / API` 表（逐行映射）、`类型定义` 表（无公共函数行的模块）、`配置` 表（常量面）、`修改文件` 表 + 对应 AC。**design.md「公共函数 / API」表未列行的模块，节内以 HTML 注释显式标注**，条目改溯源其余声明表；一律不 grep 源文件导出声明、不虚构条目。
- **编译面随动（非本变更被测文件，不设用例设计）**：`AgentBlock::Text` 增 `citations` 与 `LoopTurn` 增 `web_search`、provider 记录增 `webSearch` 会让既有测试 fixture 字面量缺字段。机械补位清单（迭代性质 = `修改`，断言语义不变）：
  - Rust：`crates/core/agent/src/event_test.rs` / `lib_test.rs` / `usage_test.rs`、`crates/core/orchestration/src/{state_test,transcript_test}.rs`、`crates/infra/agent/src/{compose_test,worker_test}.rs`、`crates/infra/agent/src/sdk/{loop_test,resume_test,runner_test}.rs`、`crates/infra/store/src/model_test.rs`、`src/commands/exec/mod_test.rs` —— `AgentBlock::Text { text }` 补 `citations: None`；loop 侧 `LoopTurn { … }` 补 `web_search: false`。
  - 前端：`src/views/agents/agents-view.test.tsx`、`src/views/agents/components/agent-panel.test.tsx` —— provider fixture 补 `webSearch`（本变更范围内的 `provider-panel.test.tsx` / `use-agent-providers.test.ts` 在各自章节内记账）。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | `AgentBlock` 五变体（`WebSearch{ id, query, results, error, raw_use, raw_result }`）+ `Text.citations: Option<Vec<AgentCitation>>`；旧 `Text` JSON 无 `citations` 读出 `None`；序列化缺省时 `citations` / `error` / `raw_use` / `raw_result` 落 `null`、`results` 落 `[]`——**键恒在**（D1 第四稿：仅 `#[serde(default)]`，`skip_serializing_if` 撤销）；`AgentCitation` / `WebSearchSource` 经 `crates/core/agent/src/lib.rs` 公开导出 | `packages/desktop/src-tauri/crates/core/agent/src/event.rs`（`event_test.rs`）；`packages/desktop/src-tauri/crates/core/agent/src/lib.rs`（再导出面）与 `packages/desktop/src-tauri/crates/core/orchestration/src/transcript.rs`（`Text { text, .. }` 模式补位）见「不可测试项」 |
| AC-2 | 事件落库零 schema 版本升级：含 `WebSearch` 块的 `SessionEventRecord` JSON 往返保真（`raw_use` / `raw_result` 逐字节不变） | `packages/desktop/src-tauri/crates/infra/store/src/model.rs`（`model_test.rs` 往返用例 + 版本头断言） |
| AC-3 | 开关开 → `additional_params.tools` 恰为 `[{type:"web_search_20250305", name:"web_search", max_uses:5}]`；开关关 → 该键不存在；`RunStarted.tools` 两态均不含 `web_search`；`normalize.rs` 零 diff | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`（`loop_test.rs`）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs` 零 diff 由守线核验承载（见「不可测试项」） |
| AC-4 | 投影：use + 配对 result 收敛为恰一块（`id` 取 `server_tool_use.id`）；孤儿 result 单独成块（`raw_use: None`）；其他 raw server 块跳过；`results` 取 `url` / `title` / `page_age`；错误取 `error_code` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/web_search.rs`（`web_search_test.rs` 规则表逐条）+ `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`（不触发客户端工具执行）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（模块挂载）见「不可测试项」 |
| AC-5 | 回放保真：`WebSearch` → use → result 两条 `AssistantContent`（保原序、文本为空、raw 放回 `anthropic_content`）；双 `None` 跳过；assistant 位 `ToolResult` 仍跳过；`Text.citations` 以 `[c.raw, …]` 回填 `citations` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/web_search.rs`（`replay_raw` / `replay_text`）+ `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs`（`resume_test.rs` 逐字节等价） |
| AC-6 | `pause_turn` 续跑：续发请求 + 每次发 `SystemNotice{pause_turn_continued, payload:{continuations:n}}`；续跑前 `attempt = 0`（不吃重试预算）；usage 逐次累加；达 5 次按 `stop` 正常收敛并发 `pause_turn_capped`（`continuations: 5`）；空 choice 与客户端 `tool_calls` 不续跑；停止信号截停 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`（`loop_test.rs` 脚本化 `FinishReason::Other("pause_turn")` 驱动） |
| AC-7 | `sweep_orphans` 剔除 assistant 位无配对 use 的 server result 块；成对块保留；剔空消息整条移除；三处调用点一致生效 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs`（`context_test.rs`） |
| AC-8 | 开关五层穿透：store v4 读写 `webSearch`；`save_agent_provider` 第 9 参接受 openai 下的 `true`；`ResolvedEngine.web_search = provider.web_search && protocol == Anthropic` 三组映射；`EngineFacade::with_web_search` / `SdkRunner::with_web_search` 传递可考；`EngineConfig` 字段面与 `runner_for` 唯一 match 不变；`new` 七参不变 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs`、`packages/desktop/src-tauri/src/commands/agents/mod.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`（各对应 `_test.rs`） |
| AC-9 | 前端适配：`webSearch` → `tool-web_search` 两态（未配对 `input-available` / 配对 `output-available` 且 `output` 为 `AgentToolOutput` 加位形态 `{content:'', isError, search:{results,error}}`）；text 部件 `providerMetadata.agent.citations`（空数组不挂）；跨消息 use↔result 按 `id` 合并；`eventToChunk` 与 `agent-transport.ts` 零 diff | `packages/desktop/src/lib/agent-adapter.ts`（`agent-adapter.test.ts`）；`packages/desktop/src/lib/agent-transport.ts` 零 diff 由守线核验（见「不可测试项」） |
| AC-10 | 呈现：`WebSearchCard` summary 恒「网络搜索」（无状态文案）、body 呈查询词 / 来源列表 / 错误；`CitationList` 按 url 去重、`title ?? url`；`ExternalLink` 非 http(s) 不渲为链接；`DEFAULT_TOOL_CARDS` 含 `web_search`；空 parts 消息不渲染且不占分页计数 | `packages/desktop/src/components/agent/agent-blocks.tsx`、`packages/desktop/src/components/agent/tool-cards.tsx`、`packages/desktop/src/components/agent/agent-messages.tsx`（各对应 `.test.tsx`） |
| AC-11 | 管理页：checkbox 仅 `protocol === 'anthropic'` 渲染（默认关、切协议清零、切回不恢复）、提交载荷含 `webSearch`、invoke 位置参数逐位对齐 | `packages/desktop/src/views/agents/components/provider-panel.tsx`（`.test.tsx`）、`packages/desktop/src/views/agents/hooks/use-agent-providers.ts`（`.test.ts`） |
| AC-12 | 外链：`tauri-plugin-opener` 已注册（builder 链与 `dialog` 同位）；capabilities 只放 `http(s)`；前端外链调用点唯一（`ExternalLink`） | `packages/desktop/src/components/agent/agent-blocks.tsx`（`ExternalLink` 经 `agent-blocks.test.tsx` 驱动）；`packages/desktop/src-tauri/src/main.rs`（builder 链）、`packages/desktop/src-tauri/src/lib.rs`（零改动核对）、`packages/desktop/src-tauri/Cargo.toml`、`packages/desktop/src-tauri/capabilities/default.json`、`packages/desktop/package.json`（前端依赖）见「不可测试项」 |
| AC-13 | 静态检查与测试全绿：`cargo clippy` 零告警；`bindings:check` 零 diff；`run check` / `run test` 全绿；`package.json` 版本 `0.5.29`；generated bindings 回到**统一形态**（零 `_Serialize` / `_Deserialize`，`agentStart` 首参仍 `Channel<AgentRunMessage>`） | `packages/desktop/src/types/generated/bindings.ts`（再生成物）、`packages/desktop/src-tauri/src/bindings/mod_test.rs`（四条 golden 断言零改动即全绿——D1 第四稿的机械判据）、`packages/desktop/package.json`（版本）、`packages/desktop/src-tauri/Cargo.lock`、`packages/desktop/pnpm-lock.yaml`（锁文件）与静态检查链——见「不可测试项」AC-13 行（机械门禁由桌面端执行，本文件不复制脚本） |

> 本表覆盖 design「新增文件」「修改文件」两表的**全部条目**（含无进程内测试落点者，逐条在「不可测试项」说明原因）：`crates/infra/agent/src/sdk/mod.rs` 归 AC-4、`crates/core/orchestration/src/transcript.rs` 归 AC-1、`src-tauri/src/lib.rs` 归 AC-12、`src-tauri/src/bindings/mod_test.rs`（第四稿登记的零改动核对项）与 codegen / 锁文件归 AC-13。

---

## 单元测试

> **`路径类型` 取值**：`正向` / `异常` / `边界` / `组合`（跨模块链路用例按**链路发起方 / 最上层调用方**挂靠到入口模块章节，不另立集成层）。
> **`迭代类型` 取值**：`新增`（本轮新增用例）/ `修改`（既有用例随行为或字段面改写）/ `保留`（既有用例不变，仅作回归承载）。

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/web_search.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/web_search_test.rs

> 新建测试文件（design「新增文件」表 + `sdk/mod.rs` 侧加 `#[cfg(test)] mod web_search_test;`）。

#### 待测功能

> 来源：design.md「公共函数 / API」表 6 行 + 「配置」表 5 条常量。

- `provider_tool()`: 请求侧 `ProviderToolDefinition` 单点构造——`kind = "web_search_20250305"`、`name = "web_search"`、`max_uses = 5`；经 `CompletionRequest::provider_tool` 落 `additional_params.tools` 后形状恰为 `[{type,name,max_uses}]`
- `raw_content(&Text) -> Option<&Value>`: 取 `additional_params["anthropic_content"]`；`None` = 普通文本块（判别口径只此一处）
- `citations_of(&Text) -> Option<Vec<AgentCitation>>`: 缺失 / 非数组 / **空数组** → `None`；`raw` 取数组元素原值（不经 rig `Citation` 往返规范化）
- `project_choice(&[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>)`: 块投影唯一实现（use + 配对 result 收敛一块、孤儿 result 单独成块、未知 raw server 块跳过、其余块现状不变）
- `replay_raw(&Value) -> AssistantContent`: 产 `Text{ text: "", additional_params: {anthropic_content: raw} }`——**必须空文本**（rig 硬校验「raw 与文本不可共存」）
- `replay_text(&str, Option<&[AgentCitation]>) -> AssistantContent`: 非空 citations → `additional_params["citations"] = [c.raw, …]`；否则纯 `Text`
- 常量面 `WEB_SEARCH_TOOL_KIND` / `WEB_SEARCH_TOOL_NAME` / `WEB_SEARCH_MAX_USES` / `ANTHROPIC_RAW_CONTENT_KEY` / `CITATIONS_KEY`: 字面量单点持有（loop / resume 零重复），`max_uses` 为成本护栏常量、不可配置

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `provider_tool` 请求形状 | 正向 | 返回值经 `CompletionRequest::provider_tool` 后 `additional_params["tools"]` 深等于 `[{"type":"web_search_20250305","name":"web_search","max_uses":5}]`——恰一项、恰三键、无 `cache_control` / `allowed_domains` 等附加键；`type` / `name` 为字符串、`max_uses` 为 JSON 数字 `5`（非 `"5"`） | 新增 |
| `provider_tool` 常量面与纯函数性 | 边界 | `WEB_SEARCH_MAX_USES == 5` 且与 `PAUSE_TURN_MAX_CONTINUATIONS` 分名（两常量各自可考，命名不混用）；连续两次调用产出深相等值（无内部状态、无可变全局） | 新增 |
| `raw_content` 命中 | 正向 | `Text{ text: "", additional_params: Some({"anthropic_content": raw}) }` → `Some(raw)`，与入参 `serde_json::to_string` 逐字节相等（含 `encrypted_content` 等不确定字段） | 新增 |
| `raw_content` 缺席三形态 | 异常 | `additional_params == None` → `None`；`Some({})` → `None`；键名大小写不符 `{"ANTHROPIC_CONTENT": …}` → `None`（键名大小写敏感） | 新增 |
| `raw_content` 非对象值与多键共存 | 边界 | 键值分别为 `null` / 字符串 / 数字 / 数组 → 原样返回 `Some`（不做类型校验，`type` 判别归 `project_choice`，不 panic）；`additional_params` 同时含 `anthropic_content` 与 `citations` → 只取前者、不串键 | 新增 |
| `citations_of` 正常提取 | 正向 | 两条引用（其中一条缺 `title`）→ `Some(vec![…])`，逐项 `url` / `title` / `cited_text` 取自对应键、`raw` 与数组元素**逐字节相等**；缺键项对应字段 `None`（不报错、不丢元素）；`title` 缺席项不被规范化为 `null` 字符串 | 新增 |
| `citations_of` 空与缺失折叠 | 边界 | 键缺席 / `additional_params == None` / 空数组 `[]` → 一律 `None`（「空 citations 不挂 metadata」在 Rust 侧即成立，前端无需第二判定） | 新增 |
| `citations_of` 类型异常 | 异常 | 键值为对象 / 字符串 / `null`（非数组）→ `None`；数组元素为非对象（字符串 / 数字 / `null`）→ 该元素产 `AgentCitation{ url: None, title: None, cited_text: None, raw: 原元素 }`，元素数不丢、不 panic | 新增 |
| `project_choice` use + result 收敛 | 正向 | `server_tool_use{ id:"srvtoolu_1", name:"web_search", input:{query:"今日汇率"} }` + 配对 `web_search_tool_result{ tool_use_id:"srvtoolu_1", content:[web_search_result × 3] }` → `blocks` 恰 1 项 `WebSearch{ id:"srvtoolu_1", query:"今日汇率", results: 3 条, error: None, raw_use: Some, raw_result: Some }`；result 不另产块；第二返回值 `Vec<ToolCall>` 为空（server 块不进客户端工具面） | 新增 |
| `project_choice` 展示字段映射 | 正向 | `results` 逐条取 `web_search_result` 的 `url` / `title` / `page_age`（`title`、`page_age` 缺席 → `None`，`url` 逐字）；`query` 取自 `server_tool_use.input.query`；`input.query` 缺席 → `query` 空串（不 panic） | 新增 |
| `project_choice` 错误结果 | 异常 | result 内容为 `web_search_tool_result_error{ error_code:"max_uses_exceeded" }` → 收敛块 `error = Some("max_uses_exceeded")`、`results` 为空且序列化**落键** `"results": []`（D1 第四稿改写：不再断言「不落键」）；配对 use 在场时仍收敛为**一块**、不额外产错误块 | 修改 |
| `project_choice` 孤儿 result | 边界 | 前序无配对 use 的 `web_search_tool_result` → 单独成块：`raw_use == None`、`id` 取 `tool_use_id`、`query` 空串、`raw_result: Some`、`results` 照常映射；与同 choice 内的配对块并存时两块同 id 语义可辨（前端按 `rawUse` 判别）；缺 `tool_use_id` 的 wild result → 跳过（无 id 可承载），不产块、不 panic | 新增 |
| `project_choice` 结果先于 use | 边界 | choice 顺序为 result → use（同一 `tool_use_id`）→ 预索引仍配对：`blocks` 恰一块 `WebSearch`（位置取 use 的遍历位）、result 不另产块；`consumed` 标记按 result 下标生效 | 新增 |
| `project_choice` 未知 server 块跳过 | 异常 | 非 `web_search` 的 `server_tool_use`（如 `name:"code_execution"`）、`server_tool_use` 缺 `id` 或缺 `name`、`code_execution_tool_result`、`additional_params` 携未知 raw `type` 的 `Text` → 不产 `AgentBlock`、不落 raw、不 panic、不报错；同 choice 其余块照常投影且相对顺序保持 | 新增 |
| `project_choice` 既有分支不变 | 正向 | 无 raw 的 `Text` → `AgentBlock::Text{ text, citations: None }`；携 citations 的 `Text` → `citations: Some(…)`；`Reasoning` → `Thinking`（可显示文本口径不变）；`ToolCall` → `ToolUse` 且同条进第二返回值；`Image` → 不产块（现状） | 新增 |
| `project_choice` 容器边界 | 边界 | `&[]` → `(vec![], vec![])`；单元素 choice；100 条混合块（Text / use / result / 未知 raw 交错）→ 条数守恒、无丢块、无重排；use 与 result 之间夹 `Text` 时收敛块位置取 use 的顺序位 | 新增 |
| `replay_raw` 形状与保真 | 正向 | 任意对象 raw（含 `encrypted_content`、嵌套数组、`cache_control` 等原样字段）→ `AssistantContent::Text{ text: "", additional_params: Some({"anthropic_content": raw}) }`：`text` **必为空串**、`raw` 与入参逐字节相等；连续两次调用产出相等值 | 新增 |
| `replay_raw` 非对象载荷 | 边界 | raw 为 `null` / 数组 / 空对象 `{}` / 超深嵌套 → 原样包裹、不 panic、不校验 `type`（类型判别不在重建路径） | 新增 |
| `replay_text` 无引用 | 正向 | `citations = None` → `Text{ text: 原文, additional_params: None }`：不落 `citations` 键、不落 `anthropic_content` | 新增 |
| `replay_text` 携引用回填 | 正向 | `Some([c1, c2])` → `additional_params["citations"]` 深等于 `[c1.raw, c2.raw]`（取 `raw` 原值，**不以** `url` / `title` / `cited_text` 重组）；`text` 原文保真；构造 `url` 与 `raw.url` 不一致的 citation 时产物仍等于 `raw`（展示字段不参与重建的机械判据） | 新增 |
| `replay_text` 空引用与文本边界 | 边界 | 空切片 `Some(&[])` → 视同无引用（不落 `citations` 键）；`text = ""` + 非空 citations → `Text{text:"", additional_params 携 citations}`；超长文本（>1000 字符）/ 含 `\n` / `\0` / emoji → 逐字节保真、不 panic | 新增 |
| project → replay 往返语义不变 | 组合 | 原始 wire（use raw + result raw + 携 citations 的 text raw）→ `project_choice` → 块序列 → `raw_use` / `raw_result` 交 `replay_raw`、`citations` 交 `replay_text` → 重建的 `AssistantContent` 序列其 `additional_params` 与原始 wire **逐字节等价**，且顺序为 use → result（保原序） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯函数 + 值对象（`rig::message::{Text, AssistantContent}` 与 `serde_json::Value` 仅作构造载荷），零 I/O / 零 async / 零网络 / 零全局态，全部真实调用；rig 类型构造真实使用（不 mock rig） | 全部用例 |

### packages/desktop/src-tauri/crates/core/agent/src/event.rs -> packages/desktop/src-tauri/crates/core/agent/src/event_test.rs

> 来源：design.md「类型定义」表（`AgentBlock` / `AgentCitation` / `WebSearchSource`）+「修改文件」表 event.rs 行 + AC-1 / AC-2 事件面。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无本文件行）；以下条目溯源「类型定义」表 +「修改文件」表 + AC-1 / AC-2（未 grep 源文件导出声明）。 -->

#### 待测功能

- `AgentBlock::WebSearch { id, query, results, error, raw_use, raw_result }`: 第五变体；`results` / `error` / `raw_use` / `raw_result` 四字段**只**标 `#[serde(default)]`（D1 第四稿：`skip_serializing_if` 全部撤销）——线格式**键恒在**，缺省时 `results` 落 `[]`、`error` / `rawUse` / `rawResult` 落 `null`；tag `kind` + camelCase 线格式
- `AgentBlock::Text.citations: Option<Vec<AgentCitation>>`: 只标 `#[serde(default)]`——旧 JSON 读出 `None`（不报错），缺省再序列化落 `citations: null`（键恒在；TS 面仍出可选位，前端既有 fixture 零改动）
- `AgentCitation { url, title, cited_text, raw }`: 三展示字段 `Option` + `raw: Value`；`#[serde(rename_all = "camelCase")]`，`Option` 字段只标 `default`（固定 `null` 形态，键恒在）
- `WebSearchSource { url, title, page_age }`: `url` 必填、`title` / `page_age` 为 `Option`；`rename_all = "camelCase"`
- 属性面机械判据（D1 第四稿）: 五处 `skip_serializing_if` 缺席即达标——一旦回流，`AgentBlock` 在 specta unified 校验下分裂为 `_Serialize` / `_Deserialize` 并沿 `AgentEvent` / `AgentRunMessage` / `agentStart::Channel<>` 上溢，`src/bindings/mod_test.rs` 四条 golden 断言立即变红（该守卫的归属见「不可测试项」，本文件不重复设计用例）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `WebSearch` 序列化形状 | 正向 | 全字段在场 → JSON 键序无关的逐键核对：`kind == "webSearch"`、`id` / `query` 字符串、`results` 数组元素 `{url,title,pageAge}`、`error` 字符串、`rawUse` / `rawResult` 对象；`results` 非空保序 | 新增 |
| `WebSearch` 缺省键恒在（D1 第四稿改写） | 边界 | `results = vec![]` → 线格式落 `"results": []`（键在场）；`error` / `raw_use` / `raw_result = None` → 各自键在场且值为 `null`（`value.get(key)` 为 `Some(Value::Null)`）；`kind == "webSearch"`；反序列化回读与原值 `PartialEq`（往返等价）。反向判据：任一键缺席即说明 `skip_serializing_if` 回流 | 修改 |
| `WebSearch` raw 逐字节保真（AC-2） | 正向 | `raw_use` / `raw_result` 为含 `encrypted_content` 与嵌套数组的原始 wire 值 → `serde_json` 往返后 `to_string()` 逐字节相等（不做字段级规范化、不重排键语义） | 新增 |
| `Text.citations` 旧 JSON 兼容与恒在键 | 边界 | `{"kind":"text","text":"正文"}`（无 `citations` 键）→ 反序列化 `citations == None`（不报错）；再序列化输出**含** `citations: null`（键恒在，D1 第四稿；TS 面仍为可选位 → 前端既有 fixture 零改动）；显式输入 `"citations": null` 同样读出 `None` 且往返等价 | 修改 |
| `Text.citations` 在场形态 | 正向 | `citations = Some([c1, c2])` → 落 `citations` 数组、元素键 `url` / `title` / `citedText` / `raw`；`Some(vec![])` → 落 `"citations": []`（空折叠在 `citations_of` 侧，序列化层不折叠——两层职责分离可辨） | 新增 |
| `AgentCitation` null 形态 | 边界 | 四字段全空（三 `None` + `raw: Value::Null`）→ 三键呈 `null` 且**不省略**（D1 固定形态）；往返相等；`Clone` / `PartialEq` 派生可用 | 新增 |
| `WebSearchSource` 形态 | 正向 | `{url:"https://a", title:Some, page_age:Some}` → `{url, title, pageAge}`；`title` / `page_age` 缺席 → `null`（不省略）；`url` 空串合法（类型非 `Option`，构造不校验） | 新增 |
| 非法载荷拒收 | 异常 | `kind:"webSearch"` 缺 `id` → `Err`；`kind` 未知值（`"webSearchX"` / `""`）→ `Err`；`citations` 为对象 / 字符串 → `Err`（`citations: null` 则解为 `None`，`Option` 语义不报错）；`results` 为 `null` / 对象 → `Err`（非 `Option` 列表）；`raw` 键缺席 → `Err`（`Value` 无默认值，值域受控、不静默兜底） | 新增 |
| 字符串与列表边界 | 边界 | `query` 为 `""` / 超长（>1000 字符）/ 含 `\n` / `\0` / emoji → 往返逐字节；`results` 单元素与 100 条（超大列表）→ 保序无损；`raw` 为空对象 / 深嵌套 → 往返相等 | 新增 |
| 五变体信封整体往返 | 修改 | 既有 `variant_events()` 代表事件集补 `WebSearch` 块与携 citations 的 `Text` → 六事件信封 `AgentEvent` 整体 serde 往返逐字段相等；`AgentDelta` 词汇仍为 `Text` / `Thinking` 两变体（**无** `AgentDelta::Search`，决策 ⑦ 机械判据） | 修改 |
| 既有四变体回归与 fixture 补位 | 保留 | `Text{text}` / `Thinking` / `ToolUse` / `ToolResult` 既有往返、`stamp` seq / 时钟断言不变；文件内 `AgentBlock::Text { text }` 字面量补 `citations: None`（编译面随动，断言不变） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | serde / serde_json 真实实现（被测行为的载体本身）；时间源用既有 `millis_now` 镜像口径，不打桩 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

> 来源：design.md「修改文件」表 loop.rs 行 +「类型定义」表（`LoopTurn.web_search`）+「配置」表（`PAUSE_TURN_MAX_CONTINUATIONS`）+ AC-3 / AC-4 loop 侧 / AC-6。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无本文件行）；以下条目溯源「修改文件」表 +「类型定义」表 +「配置」表 + AC-3 / AC-4 / AC-6（未 grep 源文件导出声明）。 -->

#### 待测功能

- `LoopTurn.web_search: bool`: 请求构造期按需 `.provider_tool(web_search::provider_tool())`；`false` = `additional_params` 无 `tools` 键；`tools: definitions.clone()` 与 `RunStarted.tools` 零改
- 块转换换 `web_search::project_choice`: 内联四变体 match 删除，server 块产 `AgentBlock::WebSearch` + `Text.citations`，**不触发客户端工具执行**
- `pause_turn` 续跑循环（D5 分支次序）: 独立计数轴、每次续跑前 `attempt = 0`、usage 逐次累加、到顶发 `pause_turn_capped` 后按 `stop` 收敛、每次续跑发 `pause_turn_continued`（payload `{ "continuations": n }`，n 从 1 起逐次递增）、空 choice / 客户端 `tool_calls` 不续跑、续跑间隙 `handle.stop_requested()` 截停
- `PAUSE_TURN_MAX_CONTINUATIONS = 5`: 单轮续跑上限；与 `WEB_SEARCH_MAX_USES` 分名分注释（常量、日志字段不混用）
- `finish_reason` 判据（D6）: `Some(FinishReason::Other(r)) if r == "pause_turn"`；`None` 与非精确匹配值一律视为非 `pause_turn`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 开关开请求形状（AC-3） | 正向 | `LoopTurn{ web_search: true, .. }` → 捕获的 `CompletionRequest.additional_params["tools"]` 深等于 `[{"type":"web_search_20250305","name":"web_search","max_uses":5}]`（恰一项、恰三键）；`request.tools` 仍为 `tools::definitions` 派生的客户端定义集（不变） | 新增 |
| 开关关零形状（AC-3） | 边界 | `web_search: false` → `additional_params == None`，请求其余字段与改动前基线逐字一致（`temperature` / `max_tokens` / `tool_choice` / `output_schema` 全 `None`） | 新增 |
| `RunStarted.tools` 两态不含 web_search（AC-3） | 正向 | 开关开 / 关两态下 `RunStarted.tools` 均等于 definitions 派生名单、不含 `"web_search"`；`mcp_servers` 仍为空 | 新增 |
| server 块不触发工具执行（AC-4 loop 侧） | 正向 | 脚本化 choice 含 use + 配对 result → 恰一条 assistant 密封 `Message` 且 `blocks` 仅一块 `WebSearch`、无 `ToolUse` 块、无 user 位 `tool_result` 回灌；该轮以 `stop` 收口，历史长度按「prompt + 一条 assistant」增长 | 新增 |
| server 块与客户端工具混排 | 边界 | choice 含 `server_tool_use` + 真 `ToolCall` → `blocks` 为 `[WebSearch, ToolUse]`（原序）、`tool_calls` 非空 → 走工具执行分支（不续跑，续跑计数不增）、工具结果回灌后下一轮正常收口 | 新增 |
| `Text.citations` 落事件 | 正向 | choice 含携 citations 的 `Text` 与无 citations 的 `Text` → 密封 `Message.blocks` 中前者 `citations` 同原值（逐字节）、后者 `citations == None` | 新增 |
| 客户端 `web_search` 名调用被成员校验拒（spec Scenario） | 异常 | 开关开 + 模型发 `ToolCall{ name:"web_search" }` → 产 `SystemNotice{subtype:"tool_unavailable"}` + `ToolResult{is_error:true}`，run 不中断；`web_search` 不在 `TOOL_NAMES` / dispatch 面 | 新增 |
| `pause_turn` 续跑两次请求（AC-6） | 正向 | 第 1 帧 `FinishReason::Other("pause_turn")`（choice 仅 server 块）、第 2 帧 `Stop` → 恰两次流请求，第二次 `chat_history` 含前一轮 assistant server 块原样（`additional_params.anthropic_content` 在场）；事件流恰一条 `SystemNotice{pause_turn_continued, payload:{"continuations":1}}`；最终 `TurnDone{subtype:"success"}`、`is_error:false` | 新增 |
| 续跑不吃重试预算（AC-6） | 异常 | `retry.max_attempts = 2`：首轮 `pause_turn` → 续跑请求注入一次流起手失败 → 仍以完整预算重试成功、零 `api_error` 通知；反向判据：若续跑继承重试累积则本用例提前失败 | 新增 |
| 续跑上限到顶（AC-6） | 边界 | 连续 6 帧 `pause_turn` → 恰 5 条 `pause_turn_continued`（payload `{"continuations":1..5}` 逐条递增）+ 1 条 `pause_turn_capped`（payload `{"continuations":5}`），零第 7 次请求；收敛为 `TurnDone{subtype:"success"}` / `is_error:false`（**非** `api_error`）；`finish` 面按 `stop` 语义 | 新增 |
| usage 逐次累加（AC-6） | 正向 | 两帧各携不同 `Usage` → `TurnDone.usage` 为两者之和（不取末次、不抹零）；三帧同理线性累加 | 新增 |
| 空 choice 不续跑（AC-6） | 边界 | `finish_reason = Other("pause_turn")` + 空 choice → 走既有空轮收敛路径：零 `pause_turn_continued`、零第二次请求、`TurnDone` 正常 | 新增 |
| 续跑间隙停止截停（AC-6 停止语义） | 异常 | 续跑判定前置 `handle` 停止 → run 返回、零孤儿续跑请求、不发 `pause_turn_continued` / `capped`、历史无新增 assistant 消息 | 新增 |
| `finish_reason` 判据边界（D6） | 边界 | `Other("pause_turn_x")` / `Other("PAUSE_TURN")` / `Some(Stop)` / `None` 四形态 → 一律不续跑（精确相等匹配，非包含匹配；`None` 视为非 `pause_turn`）；恰 `Other("pause_turn")` 唯一续跑 | 新增 |
| 单轮边界与计数轴隔离 | 边界 | 恰 1 次续跑（下界）与恰 5 次（上限）与第 6 次（越界）三档分别锁定；`turn_index` 不因续跑自增、`RunStarted` 恰一条、`num_turns` 不变（续跑属同轮）；`PAUSE_TURN_MAX_CONTINUATIONS == 5` 且与 `WEB_SEARCH_MAX_USES` 不等名 | 新增 |
| 既有轮次行为回归 | 保留 | 无 `pause_turn` 的单轮文本、工具多轮、重试耗尽 `api_error`、空轮收敛、停止截停等既有用例全绿（`normalize` 调用点、`deny`、`outcome_result`、`finish*` 逐字不变） | 保留 |
| fixture 补位 | 修改 | 既有 `LoopTurn { … }` 字面量补 `web_search: false`；`AgentBlock::Text { text }` fixture 补 `citations: None`（断言不变） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 网络 / 提供者 IO | 沿用既有 rig 流缝装置：`rig::test_utils` 的 `MockScript` / `MockFrame` / `MockStreamEvent` + 脚本化 `Transport` / `DynModel`（`Opened` / `Opening` 自造），逐轮预录流事件，零真实网络；`base_url` 不参与（模型句柄直驱） | 全部用例 |
| 请求捕获 | `Mutex<Vec<CompletionRequest>>` 记录每次请求（AC-3 形状断言的唯一观测面） | 开关开 / 关、续跑请求数 |
| 文件系统 | `tempfile` 合成 cwd（`preamble` 读取真实走盘，不 mock） | 全部用例（`LoopTurn.cwd`） |
| 内部模块 | **不 mock**：`web_search::{provider_tool, project_choice}`、`context::sweep_orphans`、`compact`、`tools::definitions` / `execute`、`normalize` 全部真实组合（组合用例的链路发起方即本文件） | 全部用例 |
| 时钟 / 活性 | 沿用既有装置：`liveness` / `retry` 由测试注小值（被测 API 显式入参），不做全局时间打桩；`tokio::time` 真实推进（既有用例已按此形态稳定） | 续跑 / 重试 / 停止截停 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs

> 来源：design.md「修改文件」表 resume.rs 行 +「类型定义」表 + AC-5。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无本文件行）；以下条目溯源「修改文件」表 +「类型定义」表 + AC-5（未 grep 源文件导出声明）。 -->

#### 待测功能

- `assistant_content` 展开循环（D3）: 一条块**最多产两条** `AssistantContent`——`WebSearch` → `replay_raw(raw_use)` 后 `replay_raw(raw_result)`（保原序），两者皆 `None` 跳过；`Text{text, citations}` → `replay_text(text, citations.as_deref())`；`Thinking` / `ToolUse` 现状；assistant 位 `ToolResult` 仍跳过
- `text_content` 模式补位: user 位 `Text { text, .. }`——citations 不参与 user 侧重建
- 展示字段隔离: 重建只吃 `raw` / `text`，`query` / `results` / `error` 在类型上不进入重建路径

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `WebSearch` 重建为两条（AC-5） | 正向 | 单块 `WebSearch{ raw_use: Some, raw_result: Some }` 驱动 `rebuild` → 该 `Message::Assistant` 的 `content` 长度恰 2，两条同为 `Text{ text: "", additional_params: Some({"anthropic_content": raw}) }`，顺序 use → result；`additional_params` 与原 wire 逐字节相等（含 `encrypted_content`） | 新增 |
| 单侧 / 双侧缺席 | 边界 | `raw_use: Some` + `raw_result: None` → 恰 1 条（use）；`raw_use: None` + `raw_result: Some` → 恰 1 条（result）；双 `None` → 0 条（跳过，且该 assistant 消息因 `content` 空而不入史，沿用既有 `!content.is_empty()` 语义） | 新增 |
| citations 回填（AC-5） | 正向 | `Text{ text:"正文", citations: Some([c1, c2]) }` → `additional_params["citations"]` 深等于 `[c1.raw, c2.raw]`、`text == "正文"`；`citations: None` → `additional_params == None`（**rig 回放侧**语义：不挂空壳 `citations` 数组；与 serde 键恒在无关，D1 第四稿登记**零改动**）；构造 `url` 与 `raw.url` 不一致的 citation → 产物只等于 `raw`（展示字段不参与重建的断言口径） | 新增 |
| 混合块序 | 正向 | 一条 assistant 消息含 `[Text, WebSearch(use+result), Text]` → 展开为 4 条 `AssistantContent`、顺序为 text → use → result → text（保原序，无块重排） | 新增 |
| assistant 位 `ToolResult` 仍跳过 | 异常 | assistant 消息含 `ToolResult` 块 → 不产 `AssistantContent`（现状），不 panic、不误当 result 侧重建 | 新增 |
| user 位 citations 不参与 | 边界 | user 消息 `Text{ text:"补充", citations: Some([...]) }` → 恰一条 `UserContent` 文本、citations 丢弃；user 位 `WebSearch` 块（非法形态）→ 跳过不 panic | 新增 |
| project → resume 往返（AC-5 组合） | 组合 | wire choice → `web_search::project_choice` → 密封 `AgentBlock` 序列 → `rebuild` → 重建后 assistant `content` 的 `additional_params` 与原 wire **逐字节等价**；覆盖 text+citations、use+result、双 None 三形态 | 新增 |
| 既有重建行为回归 | 保留 | 相邻 assistant 处理、`pending` 未应答补齐、`ToolResult` 配对并单条 user 消息、`Thinking` → `Reasoning`（`LOCAL_ISSUER`）等既有用例全绿；重建为空史仍 `Err` | 保留 |
| 回放旁路不挂空壳（第四稿登记防误改） | 边界 | `citations: None` 的 `Text` 块重建产物 `additional_params == None`——既有断言**零改动**（rig 回放侧语义，与 serde 键恒在无关）；反向判据：若因「键恒在」把它改成「落空 `citations` 数组」，则改变了回放 wire 形状，属越界改写 | 保留 |
| fixture 补位 | 修改 | 文件内 `AgentBlock::Text { text }` 字面量补 `citations: None`（断言不变） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯函数（`Vec<AgentBlock>` → `Vec<Message>`），无 I/O / 网络 / store；`web_search::replay_raw` / `replay_text` **不 mock**（组合用例正是其真实组合面） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/context_test.rs

> 来源：design.md「公共函数 / API」表 `sweep_orphans` 行 +「修改文件」表 context.rs 行（D7）+ AC-7。

#### 待测功能

- `sweep_orphans(history: Vec<Message>) -> Vec<Message>`: 签名不变、语义扩展——单向前序扫描 assistant 消息，遇 raw `type == "server_tool_use"` 收 `id` 入集合；遇 raw `type == "web_search_tool_result"` 且 `tool_use_id ∉ 集合` → 剔除该 `Text` 项；剔除后 `content` 为空则整条 assistant 消息移除；`UserContent` 侧既有 `CallId` 配对逻辑逐字不动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 裁掉 use 后剔孤 result（AC-7 首 Scenario） | 正向 | 两条 assistant 消息（A 携 use raw、B 携 result raw 且 `tool_use_id` 指向 A），从史中移除 A 后调用 → B 中该 result `Text` 项被剔；B 因此变空则整条移除（历史条数 -1） | 新增 |
| 成对块保留（AC-7 末 Scenario） | 正向 | A + B 均在 → 两块原样保留，消息数不变、`content` 项数不变（不误剔 result 侧） | 新增 |
| 同消息成对 | 边界 | 单条 assistant 消息内 use 与 result（`pause_turn` 未拆分形态）→ 前序扫描命中，两项均保留；顺序不变 | 新增 |
| 剔空整条移除 / 部分保留 | 边界 | assistant 消息仅含一条孤儿 result → 整条移除；含孤儿 result + 普通 `Text` → 保留消息、仅剩 `Text`（项数 -1） | 新增 |
| result 先于 use（异常顺序） | 异常 | 同一 assistant 消息内 result 出现在 use 之前 → 单向前序扫描判孤并剔除（与既有 `ToolResult` 清扫同构），不 panic；后续 use 不受影响 | 新增 |
| 非 server raw 块零影响 | 边界 | 含 `code_execution_tool_result` raw / 无 raw 的普通 `Text` / `Thinking` / `additional_params` 非对象 / raw 缺 `type` → 原样保留（本期只认 `web_search_tool_result`，不扩面） | 新增 |
| 既有配对清扫零回归（AC-7 第三 Scenario） | 保留 | 既有孤儿 `tool_result` 清扫用例、L2 prune / L3 hard_prune / 摘要置换三处调用点用例全绿（三调用点零改自动生效） | 保留 |
| 空史与首条 user | 边界 | `vec![]` → `vec![]`；仅 system / user 单条 → 原样返回；首条 user 恒在场（既有 prune 不变式） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯 `Vec<Message>` 变换（rig 消息类型仅构造），零 I/O / 零 store；token 估算走既有 `estimate_*` 真实实现 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

> 来源：design.md「公共函数 / API」表 `SdkRunner::with_web_search` 行 +「修改文件」表 runner.rs 行（`PumpPayload` / `LoopTurn` / 启动日志）+ AC-8 后半。

#### 待测功能

- `SdkRunner::with_web_search(self, web_search: bool) -> Self`: builder 缝，`new` 四参签名不动；`new` 内缺省 `false`（「未装配 = 显式关」，`bool` 非 `Option`——省去三态歧义）
- `PumpPayload.web_search` → `LoopTurn.web_search` 传递: 泵装配期传递、启动日志增 `web_search=…` 字段（诊断面可考）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `with_web_search` 装配与传递 | 正向 | `SdkRunner::new(...).with_web_search(true)` → `open_session` 后泵启动日志行含 `web_search=true`（与 `protocol=` / `window=` 同列）；`false` 同理 | 新增 |
| 未装配缺省关 | 边界 | `SdkRunner::new(...)`（不带 builder）→ 日志 `web_search=false`；行为等价于显式 `.with_web_search(false)`（同一日志与事件面） | 新增 |
| 重复调用后者覆盖 | 边界 | `.with_web_search(true).with_web_search(false)` → `false`；反向链 → `true`（同字段覆盖，无累积） | 新增 |
| 与既有旁路载荷正交 | 边界 | `with_max_tokens` / `with_context_window` / `with_web_search` 三序链同构：三载荷各自保真、互不覆盖（既有「两载荷互不覆盖」用例扩位第三项） | 修改 |
| 启动校验不受影响 | 保留 | 空配置 `config_missing` 显式失败、续会话装载缝校验等既有用例不变；`build_model` / `anthropic_provider` / `resolve_output_tokens` 零改 | 保留 |
| 启动失败面不被开关改写 | 异常 | 空配置（缺 api_key / base_url / 模型档）+ `.with_web_search(true)` → 仍为 `config_missing` 显式失败、错误文案与未装配时逐字一致；开关不参与 `is_complete` 判定、不吞启动错误 | 新增 |
| fixture 补位 | 修改 | 文件内 `AgentBlock::Text` fixture 补 `citations: None`（编译面随动） | 修改 |

> 请求级形状（`additional_params.tools`）在 `loop_test.rs` 断言；runner 层以日志字段为传递可考点（遵循既有「组装面断言接缝」先例，不为测试提升 `session_pump` 的可见性）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 网络 | 沿用既有装置：`base_url` 指本机 discard 端口 `http://127.0.0.1:9/v1`（恒拒连、不打外网）；断言面取事件流与日志，不取响应体 | 全部用例 |
| 数据文件 / 日志 | `tempfile` 数据根；引擎日志真实写入后读取断言（`append_engine_log` 不 mock） | 传递 / 缺省用例 |
| 内部模块 | **不 mock**：`SdkRunner` / `PumpPayload` / `LoopTurn` 真实组合；`loop::run` 行为不在本文件断言（挂 `loop_test.rs`） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

> 来源：design.md「公共函数 / API」表 `EngineFacade::with_web_search` 行 +「修改文件」表（五处 `Self{}` 构造点补齐）+ AC-8。

#### 待测功能

- `EngineFacade::with_web_search(self, web_search: bool) -> Self`: 与 `with_max_tokens` 同型旁路缝；`new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_explore_note_store` 五处构造点补齐（缺省 `false`）；`runner_for` 的 sdk 臂追加 `.with_web_search(self.web_search)`
- 门面零协议判定: 协议门控在组合根单点（`compose`），门面与 runner 只传 `bool`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `with_web_search` 分发等价（AC-8） | 正向 | `EngineFacade::new().with_web_search(true).runner_for(Sdk, cfg)` 与「直构 `SdkRunner::new(...).with_web_search(true)`」行为等价（同事件面 + 同日志 `web_search=true`）；`false` 臂同理（沿既有 `with_max_tokens` 分发等价用例形态） | 新增 |
| 五处构造点缺省 false | 边界 | `new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_explore_note_store` 五条既有构造路径产出的门面均为「未装配 = 关」（日志 `web_search=false`，行为与现状一致）——五处 `Self{}` 补齐的机械判据 | 修改 |
| 三载荷互不覆盖 | 边界 | `with_context_window` / `with_max_tokens` / `with_web_search` 任意顺序链式 → 三载荷各自保真；同字段重复调用后者覆盖 | 新增 |
| 唯一 match 点不塌 | 保留 | `EngineKind` 双值穷举、Cli 臂退役文案逐字、`runner_for` 结构化返回面（引擎具体类型不出门面）等既有用例不变；`EngineConfig` 字段面零改 | 保留 |
| 退役臂不被开关改写 | 异常 | `EngineFacade::new().with_web_search(true).runner_for(Cli, cfg)` → 仍为 `Err("CLI 租户已下线")`（文案逐字）；开关不静默回退、不改走 sdk 臂 | 新增 |
| fixture 补位 | 修改 | 文件内 `AgentBlock::Text` fixture 补 `citations: None`（编译面随动） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 装载缝（被测 API 显式入参） | 沿用既有 `counting_loader` 闭包替身（`ResumeTranscript` 为注入依赖，`Arc<dyn Fn>` 可替身） | 续会话路径用例 |
| 数据文件 / DB | `tempfile` 数据根 + 真实 `store::Store`（既有 `temp_store` 装置），不 mock store | explore 句柄路径用例 |
| 网络 | discard 端点恒拒连（不打外网）；断言止于事件面与日志 | 分发等价用例 |
| 内部模块 | **不 mock**：`compose_turn` / `session_query` / `EngineFacade` / `SdkRunner` 真实组合 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

> 来源：design.md「修改文件」表 compose.rs 行（D8 门控单点）+「类型定义」表（`ResolvedEngine.web_search`）+ AC-8。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无本文件行）；以下条目溯源「修改文件」表 +「类型定义」表 + AC-8（未 grep 源文件导出声明）。 -->

#### 待测功能

- `ResolvedEngine.web_search: bool`: `provider.web_search && matches!(protocol, Protocol::Anthropic)`，在 `provider` 按值移动**之前**算出（`map_protocol` 先行）；门控是唯一需要看协议的位置
- 门面链传递: `resolve_agent_engine` 产物经 `.with_web_search(resolved.web_search)` 抵达 runner（`bool` 全链非 `Option<bool>`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 门控三组映射（AC-8） | 正向 | `(anthropic, true) → true`；`(openai, true) → false`；`(anthropic, false) → false`；补底格 `(openai, false) → false`——四格逐格断言（`ResolvedEngine.web_search`） | 新增 |
| 协议切换后开关原样保留（R11） | 边界 | 记录 `web_search = true` + `protocol = openai`（「先开开关后改协议」）→ 解析成功、`web_search == false`、记录值不被清零（store 侧仍 `true`）；再改回 anthropic → 解析为 `true` | 新增 |
| 门面链传递可考 | 正向 | 解析产物经门面 `runner_for` 装配后行为等价于「直构 runner + 同值 `web_search`」（日志 `web_search=true/false` 与事件面一致） | 新增 |
| 解析失败面不被开关改写 | 异常 | 引用缺失 / 记录不完整（`EngineConfig.is_complete()` 为假）/ `engine: Cli` 三形态 → 与改动前同一 `Err` 文案与变体（`ConfigMissing` / `Err("CLI 租户已下线")`）；`web_search` 不参与 `is_complete` 判定、不改变错误面 | 新增 |
| `EngineConfig` 字段面与 `is_complete` 不变 | 保留 | `EngineConfig` 构造字段逐字与现状一致（`web_search` 走旁路，不增连接配置字段）；`is_complete` 语义不变；Cli 臂仍在解析单点即拒 | 保留 |
| fixture 补位 | 修改 | 既有 `AgentProviderRecord::new(...)` 调用点零改（缺省 `false` 即编译通过）；需开关的 fixture 经 `.with_web_search(true)` 补位（不改 `new` 参数面） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 数据文件 / DB | 既有 `tempfile` 数据根 + 真实 `WorkspaceStores` / 真实 `native_db`（不 mock store；provider / instance 记录真实读写） | 全部用例 |
| 内部模块 | **不 mock**：`map_protocol`、`EngineFacade`、`SdkRunner`、`ResolvedEngine` 真实组合（解析面为纯组合链路） | 全部用例 |
| 网络 | 不发起请求（断言止于解析产物与日志，`base_url` 为占位值） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

> 来源：design.md「公共函数 / API」表 `AgentProviderRecord::with_web_search` 行 +「类型定义」表（`AgentProviderRecordV3` / `AgentProviderRecord` v4）+ AC-8 / AC-2。

#### 待测功能

- `AgentProviderRecord::with_web_search(self, web_search: bool) -> Self`: 构造器链式缝——`new(...)` 七参不动的唯一实现路径（38 处既有调用点零改）
- `AgentProviderRecord` v4 演进（D9）: 现 v3 改名 `AgentProviderRecordV3`（`pub(crate)`、去 `#[native_db]` / `#[primary_key]`）、当前记录 `native_model(id = 5, version = 4, from = AgentProviderRecordV3)` + `#[serde(default)] web_search: bool`；V3↔V4 双向 `From`
- 手写遮蔽 `Debug`: 字段面追加 `web_search` 位，`api_key` 位遮蔽边界不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `with_web_search` 构造 | 正向 | `new(...).with_web_search(true)` → `web_search == true` 且其余七字段逐字段保真、`id == 0`；`false` 臂同理 | 新增 |
| `new` 缺省关 | 边界 | `new(...)` 不带构造器 → `web_search == false`（「38 处调用点零改」语义依据）；`id` 仍置 0 | 新增 |
| 链式覆盖 | 边界 | `.with_web_search(true).with_web_search(false)` → `false`（后者覆盖，无累积） | 新增 |
| v3 → v4 升级（AC-8） | 正向 | `native_model::encode(AgentProviderRecordV3)` → `decode::<AgentProviderRecord>` 成功：`web_search == false`、其余字段逐字段相等（存量行行为零变化、无迁移脚本） | 新增 |
| v1 → v2 → v3 → v4 三级链 | 修改 | `AgentProviderRecordV1` 载荷 → `decode::<AgentProviderRecord>` 成功：`web_search == false`、`protocol == Openai`、`max_tokens == None`、`context_length == None`（逐级 `From` 对齐，无跳级） | 修改 |
| 降级半边 | 边界 | `AgentProviderRecord` → `AgentProviderRecordV3` 丢 `web_search`、其余保真；再降 V2 丢 `protocol` / `max_tokens`；降级产物不作数据承诺（可逆性不成立） | 新增 |
| 版本头 | 边界 | 新记录 `native_model` 版本头逐字 `4`；`AgentProviderRecordV3` 版本头 `3`、V2 为 `2`、V1 为 `1` | 修改 |
| 模型组注册（D9） | 边界 | 以既有 `WorkspaceStores::open(tempdir)` 装置打开真库 → 成功（即 `AgentProviderRecord` v4 为 id=5 的唯一注册形态；`AgentProviderRecordV3` 为 historic 形态、无 `#[native_db]` / `#[primary_key]`，重复注册会在此失败）；`MODEL_ENTRIES` 的 `agent_provider` 项仍指向 v4 类型（类型名不变、注册表零改） | 新增 |
| serde 线格式与缺列 | 正向 | `webSearch` 键在场出线为布尔；`webSearch: true/false` 往返相等；旧 JSON 缺 `webSearch` → `false`（`#[serde(default)]`） | 新增 |
| 非法载荷 | 异常 | `webSearch` 为字符串 `"true"` / 数字 `1` / `null` → 反序列化 `Err`（值域受控，`null` 不静默兜底为 `false`） | 新增 |
| 遮蔽 `Debug` 扩位 | 修改 | `format!("{:?}")` 含 `web_search: true` / `web_search: false` 位；`api_key` 位仍 `sk-***890` 形态、全文无明文 key；空串 / 长度 < 3 的 key 仍全遮蔽（既有兜底不回归） | 修改 |
| 含新块的 `SessionEventRecord` 往返（AC-2） | 正向 | 携 `WebSearch`（`raw_use` / `raw_result` 含 `encrypted_content` 与来源加密索引）与 `Text{citations}` 的 `SessionEventRecord` → `SerdeJsonCodec` / serde_json 往返后 raw 逐字节不变、`citations` 原样；store 转录模型版本头不变（**无** schema 版本升级）；旧 `Text` JSON（无 `citations`）反序列化读出 `None` | 新增 |
| 既有 provider 用例回归 | 保留 | 构造 / 往返 / 遮蔽 `Debug` / 模型组计数等既有用例不变（`new` 七参零改即 38 处调用点零改） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 序列化器 / 版本机制 | **不 mock**：`native_model::encode` / `decode`、serde / bincode 真实实现（被测行为的载体本身）；版本链走内存往返 | 升级 / 降级 / 版本头 / 线格式 |
| 数据文件 / DB | 真库路径复用既有 `envelope_test.rs` / `store_test.rs` 的 `tempfile` 数据根 + 真实 `native_db`（本设计不新增调用点；`SessionEventRecord` 落库读取侧沿用既有装置） | 模型组注册、信封往返 |
| 内部模块 | 不 mock：`AgentModelTiers`、`mask_api_key`、`AgentProviderProtocol` 真实组合 | 全部用例 |

### packages/desktop/src-tauri/src/commands/agents/mod.rs -> packages/desktop/src-tauri/src/commands/agents/mod_test.rs

> 来源：design.md「公共函数 / API」表 `save_agent_provider` 行（D10）+ AC-8 命令面。

#### 待测功能

- `save_agent_provider(..., web_search: bool) -> Result<AgentProviderRecord, String>`: 末位第 9 参；构造后 `.with_web_search(web_search)`；**零协议判定**（openai 也收、不清零、不报错）；既有校验（模型名后缀 / `max_tokens >= 4096` / 留空 api_key 回填）逐字保留、错误串不含 api_key

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| anthropic + true 落库 | 正向 | 保存成功，返回记录 `webSearch == true`；`list_agent_providers` 与重开库后回读均为 `true`（值原样入库） | 新增 |
| openai + true 无条件接受（AC-8 / R11） | 正向 | `protocol = openai` + `web_search = true` → `Ok`（零协议判定、零报错、零静默清零），回读 `true`；生效与否留给解析期 | 新增 |
| false 两协议 | 边界 | `(anthropic, false)` / `(openai, false)` → 均 `Ok` 且存 `false`；与「未传」无语义差别（`bool` 无三态） | 新增 |
| 编辑态翻转 | 修改 | 同 id 三次保存 `true → false → true`（协议保持不变）→ 每次按入参落库（不粘滞、不累积）；行数不增（upsert 语义） | 修改 |
| 与 api_key 回填正交 | 边界 | 留空 `api_key` + `webSearch = true` → 原值回填且开关落 `true`；非空 key + `false` → 两字段各自独立 | 修改 |
| 既有校验保留 | 保留 | 非法模型名后缀（`[2m]` / `[8k]` / `[]`）/ `max_tokens` 小于 4096（`0` / `1` / `4095`）仍 `Err`、`4096`（下界）通过、校验顺序（high → medium → low，模型名先于上限）与文案不变；任一 `Err` 串不含 api_key 明文 | 保留 |
| 第九参与既有失败面正交 | 异常 | 重名 provider / 空 name / 非法模型名后缀 / `max_tokens < 4096` 四种 `Err` 形态下，`web_search` 取 `true` 与 `false` 的错误文案逐字相同（开关不参与校验、不改变失败面）；失败时库内无新行 | 新增 |
| 位置参数补位 | 修改 | 既有 8 参直调点全部补第 9 参（编译面随动）；`web_search` 为 `bool`（不接 `Option<bool>`，避免「未传」与「显式关」混淆） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（Tauri runtime） | 沿用既有装置：`tauri::test::mock_app()`（MockRuntime，无窗口无事件循环）+ `app.manage(WorkspaceStores::open(tempdir))` 后 `app.state()` 取 `State`，不启真实 runtime | 全部用例 |
| 数据文件 / DB | `tempfile` 数据根 + 真实全局库（不 mock store；`upsert_agent_provider` 真实走盘） | 全部用例 |
| 内部模块 | 不 mock：`agent_runtime::parse_model_spec`、`MIN_OUTPUT_TOKENS`、`store::WorkspaceStores` 真实组合（复用同一校验单点即本用例的价值） | 全部用例 |

### packages/desktop/src/lib/agent-adapter.ts -> packages/desktop/src/lib/agent-adapter.test.ts

> 来源：design.md「类型定义」表（`AgentToolOutput` / `BlockPlan` / `PairedOutcome` / `PairedResults`）+「修改文件」表 agent-adapter.ts 行（D11 / D12）+ AC-9。

<!-- design.md「公共函数 / API」表未声明本文件的导出函数行（`planBlock` / 两视野 / `citationMetadata` 均为文件内私有）；以下条目溯源「类型定义」表 +「修改文件」表 + AC-9（未 grep 源文件导出声明）。 -->

#### 待测功能

- `BlockPlan` text 变体 `citations?: AgentCitation[]`: `planBlock` 的 text 分支透传块 `citations`（缺席 = 无引用，不改第三变体结构）
- `BlockPlan` tool 变体承载搜索: `planBlock` 的 `webSearch` 分支——use 侧（`rawUse != null`）产 `toolName: 'web_search'` + `input: { query }`，`output` 优先取自身 `rawResult`、否则取视野配对命中、两者皆无 → `null`；result 侧（`rawUse == null`）配对命中返回 `null`、未命中产孤儿 tool 计划（`input: null` + `output` 携来源）
- `AgentToolOutput.search?`: 加位形态 `{ content: '', isError: error != null, search: { results, error } }`——既有 `content` / `isError` 位语义不破
- `PairingScope.global` / `local`: 索引源扩为「`toolUse.id` ∪ `webSearch.rawUse != null` 的 `id`」，配对产物为 `PairedOutcome` 联合（`toolResult` / `webSearch`）；两视野签名与返回类型不变。两视野均为文件内私有，测试经公开入口驱动：`eventsToUIMessages`（global 折叠路）与 `eventToChunk`（local 实时路）
- `plansToParts` / `plansToChunks` / `textChunks`: `citationMetadata` helper 单点——`undefined` / 空数组 → 不挂，否则 `{ agent: { citations } }`；text 部件、`text-start` 与 `text-delta` 共用同一份 metadata，`text-end` 不带
- 统一形态的读取口径（D1 第四稿）: 入站线格式的缺省键**恒在**（`rawUse: null` / `rawResult: null` / `results: []` / `citations: null`），故判别一律取**值**——`rawUse != null` 判 use 侧、`citations` 为空数组或 `null` 同按无引用处理；**MUST NOT** 用「键是否存在」判别（键不再缺席，靠键有无的旧写法会永久失效）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 未配对 use 侧部件（AC-9） | 正向 | 单条 message 含 `webSearch{ id, query, rawUse: {…}, rawResult: null }`（无同 id 配对）→ 恰一个 `tool-web_search` 部件，`state === 'input-available'`、`input` 深等于 `{ query }`、无 `output` 键；不产任何 `data-*` 新部件（`AgentDataParts` 成员零新增） | 新增 |
| 同消息配对态（AC-9） | 正向 | use 块自带 `rawResult`（`pause_turn` 未拆分形态）→ `state === 'output-available'`，`output` 深等于 `{ content: '', isError: false, search: { results: 原样, error: null } }`；`content` 恒空串、`isError === (error != null)` | 新增 |
| 跨消息 use↔result 合并（AC-9） | 正向 | 消息 A 携 use 侧块、消息 B 携同 id result 侧块 → `eventsToUIMessages` 后该 id 收敛为**单个** `tool-web_search` 部件（`input` 为 query、`output.search.results` 在场），result 侧不重复呈现、视图层零扫描 | 新增 |
| 错误载荷 | 异常 | `error = "max_uses_exceeded"` → `isError === true`、`search.error === "max_uses_exceeded"`、`search.results` 为空数组（不是 `null`） | 新增 |
| 孤儿 result 侧 | 边界 | result 侧块（`rawUse: null`）无配对 use → 孤儿 tool 计划：`input === null`、`output.search` 携来源载荷（信息不丢，与既有孤儿 `toolResult` 同哲学）；`toolCallId` 取块 `id` | 新增 |
| 两视野差异显式锁定（D11） | 边界 | 同一 fixture（use 与 result 分处两消息），经两个**公开入口**对拍：`eventsToUIMessages`（global 折叠路）→ 单 `output-available` 计划；`eventToChunk`（local 实时路，仅当前 message 内配对）→ `input-available`（`output: null`）+ 孤儿 `output-available` 两计划；同消息配对时两入口产物相等 | 新增 |
| text 计划 citations（AC-9） | 正向 | text 块 `citations: [两条同 url]` → 计划 `citations` **原样两条**（去重责任在呈现层）；部件 `providerMetadata` 深等于 `{ agent: { citations } }`；不存在 `providerMetadata.anthropic` 键 | 新增 |
| 空 / 缺席 citations 不挂 | 边界 | `citations: []` → 部件 `providerMetadata === undefined`（不出现空壳）；`citations` 键缺席（旧事件）/ `citations: null`（D1 第四稿下旧事件的常态形态）→ 同 | 新增 |
| 恒在键全缺省形态不误判（D1 第四稿） | 边界 | `webSearch` 块以线格式全缺省形态入站（`results: []`、`error: null`、`rawUse: null`、`rawResult: null` 四键**全在场**）→ 按 `rawUse` **值**判为 result 侧：无配对时产孤儿 plan（`input: null`）、有配对时被消费返回 `null`；若实现按「`rawUse` 键是否存在」判别，本用例即失败（键恒在 → 误判为 use 侧） | 新增 |
| chunk 路 metadata（AC-9） | 正向 | `plansToChunks` 的 text 组：`text-start` 与 `text-delta` 带**同一份** metadata（引用相等或深相等）、`text-end` 不带；空 citations → 三者均不带；经 reducer 归约后 text 部件 metadata 与折叠路同形（计划层对拍） | 新增 |
| 既有语义零破 | 保留 | `AgentToolOutput.content` / `isError` 既有消费面零改；`eventToChunk` 的 delta 分支、`isConsumedResult` / `mergeableResults`、既有 text / reasoning / 客户端工具配对与「保真字段可考」用例全绿 | 保留 |
| 五变体事件序列整体映射（组合） | 组合 | 密封事件序列（`runStarted` → `message`{text(citations) / thinking / toolUse / toolResult / webSearch} → `systemNotice` → `turnDone` → `raw`）驱动 `eventsToUIMessages` → 部件类型集合为 `text` / `reasoning` / `tool-*` 与既有 `data-*` 成员，逐块顺序保原序；搜索 use 侧未配对 → `input-available`、配对 → `output-available`；`AgentDataParts` 成员零新增（无 `data-citations` / `data-web-search`）；链路由本文件（适配层）发起 | 新增 |
| 变体穷尽 | 异常 | `planBlock` 的 `const _exhaustive: never` 承接 `webSearch` 分支后编译通过；runtime 未知块变体仍返回 `null`（不抛错、不吞事件级 `data-raw` 兜底） | 新增 |
| 取值边界 | 边界 | `query` 空串 / 超长（>1000 字符）/ 含 emoji 与 `\n`；`results` 空数组与 100 条；`rawUse` 为空对象 `{}`（非 `null` → 仍判 use 侧，查询词取自块 `query` 字段而非 `rawUse`）→ 均不 panic，部件仍产出 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯函数模块（`ai` 仅类型引用 `ProviderMetadata` / `JSONValue`），无 IPC / 网络 / store；fixture 按 serde camelCase 线格式手写（`types/dto` 镜像），内部 helper 与视野层全部真实调用 | 全部用例 |

### packages/desktop/src/components/agent/agent-blocks.tsx -> packages/desktop/src/components/agent/agent-blocks.test.tsx

> 新建测试文件（design「修改文件」表 agent-blocks.tsx 行 + D13 导出边界 + AC-10 / AC-12）。

#### 待测功能

- `TextBlock({ text, citations })`: 正文 markdown 不变，底部挂引用列表（`CitationList` 私有）；`citations` 缺席 / 空 → 无列表、无空容器
- `WebSearchCard({ toolCallId, toolName, input, output })`: summary 恒「网络搜索」、`data-testid="block-web-search"`、body = 查询词 / 来源列表 / 错误码；无状态文案、无「正在搜索…」
- 文件内私有 `ExternalLink`（经 `WebSearchCard` / `TextBlock` 间接驱动）: 仅 `http:` / `https:` 渲染 `<a>`，其余 scheme 纯文本；点击经 `openUrl`，失败 `.catch(() => {})` 不上抛——AC-12 的唯一外链调用点

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 恒标题两态（AC-10） | 正向 | `output: null`（进行中）与 `output` 在场（落定）两态渲染 → summary 文本恒 `"网络搜索"`；`container.textContent` 不含「正在搜索」「加载」「失败」类状态文案；两态视觉结构一致 | 新增 |
| body 三区 | 正向 | `input = { query: "今日汇率" }` + `output.search.results` 3 条 + `search.error = null` → 查询词文本在场、来源列表 3 条、`block-web-search` 根在场；`toolCallId` / `toolName` 任意值不影响呈现 | 新增 |
| 来源去重与标题回退（AC-10 Scenario） | 边界 | 3 条来源其中两条同 url → 列表 2 条；`title` 为 `null` 的项回退显示 `url` | 新增 |
| 错误码呈现 | 异常 | `output.search.error = "max_uses_exceeded"` → 错误码文本在场；`isError === true` → 卡面出错标记在场，来源列表不被吞（同数据两区并存） | 新增 |
| 空载荷与孤儿形态 | 边界 | `input === null`（result 侧孤儿）→ 不显查询词、不 panic；`search.results = []` → 不渲空列表容器；`output === null` → 不显来源区；`input.query` 为空串 → 不显空标签 | 新增 |
| 非 http(s) 不渲链接（AC-10 / AC-12） | 异常 | `javascript:alert(1)` / `file:///etc/passwd` / `mailto:a@b.c` 三种 scheme → 条目为纯文本（无 `<a>`、无 `role="link"`），点击不触发 `openUrl`（替身零调用） | 新增 |
| http(s) 渲链接与打开 | 正向 | `https://example.com/a` → `<a href>` 在场；`fireEvent.click` → `openUrl` 以该 url 恰调用一次，且不触发默认导航（`preventDefault`） | 新增 |
| 打开失败不炸呈现（AC-12 失败面） | 异常 | `openUrl` 返回 reject → 无未捕获 Promise 拒绝、卡片与引用列表照常渲染、无错误态上抛 | 新增 |
| `TextBlock` 引用列表（AC-10） | 正向 | `citations` 两条（其中同 url）→ 正文下方列表 1 条去重项、文本为 `title ?? url`；`block-text` 仍在场且 markdown 正文渲染不变 | 新增 |
| `TextBlock` 无引用 | 边界 | `citations` 缺席 / `[]` → 无列表节点、无空容器；`text` 为空串 → 不 panic、不渲列表 | 新增 |
| 引用条目外链共用 | 边界 | 引用 url 为 `mailto:` → 纯文本；`https:` → 链接且点击走同一 `openUrl`（唯一调用点结构可辨） | 新增 |
| 既有块回归 | 保留 | `ThinkingBlock`（折叠）、`ToolPairCard`（input/output 成对、isError 高亮）、`isToolPart` 收窄语义不变 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 外壳 IPC（`@tauri-apps/plugin-opener`） | `vi.mock('@tauri-apps/plugin-opener', …)` 注入 `openUrl: vi.fn()`，可按用例切 resolve / reject；断言调用次数与实参 | 外链渲染 / 打开 / 失败面 |
| 内部模块 | **不 mock**：`TextBlock` / `WebSearchCard` / 私有 `CitationList` / 私有 `ExternalLink` / `react-markdown` + `remark-gfm` 真实组合（私有件按 D13 经公开组件间接驱动，不为测试新增导出） | 全部用例 |
| 无网络 / 无 store / 无 IPC 通道 | jsdom 环境（沿用 `tool-cards.test.tsx` 的 `// @vitest-environment jsdom` 约定） | 全部用例 |

### packages/desktop/src/components/agent/tool-cards.tsx -> packages/desktop/src/components/agent/tool-cards.test.tsx

> 来源：design.md「公共函数 / API」表 `DEFAULT_TOOL_CARDS` 行 + AC-10 注册表。

#### 待测功能

- `DEFAULT_TOOL_CARDS: Record<string, AgentToolCard>`: 追加 `web_search` 键（值 = `WebSearchCard`），既有 `AskUserQuestion` 项与未注册名的回退语义不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 注册表键面（AC-10） | 正向 | `Object.keys(DEFAULT_TOOL_CARDS)` 含 `'web_search'` 且值为 `WebSearchCard` 本体（同引用）；既有 `'AskUserQuestion'` 项仍在、数量与顺序断言随动 | 修改 |
| 键名精确匹配边界 | 边界 | 键名大小写 / 分隔符偏差（`'Web_Search'` / `'web-search'` / `''`）→ 均 miss 并回退默认卡；仅精确 `'web_search'` 命中（键面不做归一化）；`DEFAULT_TOOL_CARDS['web_search']` 取值后直接渲染不抛错 | 新增 |
| 命中渲染 | 正向 | 经注册表取卡渲染 `toolName="web_search"` + `output` 在场 → `block-web-search` 在场、标题「网络搜索」 | 新增 |
| 载荷形态异常不炸卡 | 异常 | 经注册表渲染时 `output: null` / `output` 缺 `search` 加位 / `output.search` 为 `null` / `input` 非对象（字符串 / 数组）→ 卡片仍渲染（`block-web-search` 在场、无查询词与来源区），不抛错、不返回 `null` | 新增 |
| 未注册名回退 | 保留 | 未注册工具名 miss → 默认 `ToolPairCard`（`block-tool-use`）语义与既有 `AskUserQuestion` 特化用例均不变 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯注册表 + 纯渲染；`agent-blocks` 与 `tool-cards` 之间无环、真实 import | 全部用例 |

### packages/desktop/src/components/agent/agent-messages.tsx -> packages/desktop/src/components/agent/agent-messages.test.tsx

> 来源：design.md「修改文件」表 agent-messages.tsx 行 ①②（D14）+ AC-10。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无本文件行）；以下条目溯源「修改文件」表 + AC-10 / D14（未 grep 源文件导出声明）。 -->

#### 待测功能

- `useHistoryWindow` 过滤（决策 ⑨ / D14）: `conversationMessages` 在既有「system 角色不进对话呈现」之后追加 `message.parts.length > 0`；`visible` 与 `remaining` 分页计数均基于过滤后列表
- `PartView` 的 citations 传递（D14）: text 分支读 `part.providerMetadata?.agent?.citations`（运行时宽解析，非数组即 `undefined`）交 `TextBlock`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 空部件消息不渲染（AC-10） | 正向 | 会话含 1 条 `parts: []` 的 assistant 消息 + 2 条正常消息 → `chat-bubble` 恰 2 个（无空白气泡）；该消息的 `metadata.seq` 不出现在 DOM | 新增 |
| 分页计数不计入空消息（AC-10） | 边界 | 同上形状下 `remaining` 与「只数非空消息」一致；触顶装载按过滤后列表递增（装载后可见条数与 `remaining` 自洽）；`loadMore` 幂等 | 新增 |
| 只含被消费孤儿搜索 result 的消息 | 异常 | `pause_turn` 第二段只有被 use 消费的孤儿 result → 适配后 `parts.length === 0` → 不进对话透镜；相邻轮卡片与气泡计数不变 | 新增 |
| citations 传递（AC-10 / D14） | 正向 | text 部件带 `providerMetadata: { agent: { citations } }` → `TextBlock` 底部引用列表渲染（`title ?? url`、按 url 去重）；同一消息内另一 text 部件无 metadata → 无列表（逐部件独立） | 新增 |
| metadata 宽解析边界 | 边界 | `providerMetadata` 缺席 / `agent` 缺席 / `agent.citations` 为对象 / 字符串 / `null` / 空数组 → 一律按无引用处理（不抛错、不渲空容器） | 新增 |
| 既有呈现回归 | 保留 | user/assistant 气泡左右与 `data-role`、`ToolPartView` 注册表命中与回退、system 角色过滤、`ChatBubble` 部件顺序、触顶装载既有用例全绿 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯渲染组件，消费方注入 `AgentUIMessage[]`；fixture 经**真实**适配层（`eventsToUIMessages` / `runRecordToUIMessage`）与 `types/dto` 线格式构造（沿既有装置，不手搓部件）；无 IPC / 网络 / store | 全部用例 |

### packages/desktop/src/views/agents/components/provider-panel.tsx -> packages/desktop/src/views/agents/components/provider-panel.test.tsx

> 来源：design.md「修改文件」表 provider-panel.tsx 行六条 +「类型定义」表 `ProviderFormState` + AC-11。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无本文件行）；以下条目溯源「修改文件」表 +「类型定义」表 + AC-11（未 grep 源文件导出声明）。 -->

#### 待测功能

- `ProviderFormState.webSearch: boolean` / `EMPTY_FORM` 缺省 `false` / `toFormState` 读 `editing.webSearch ?? false`；`ProviderTextField` 键面排除该键
- 协议门控渲染: 仅 `protocol === 'anthropic'` 渲染 `data-testid="provider-web-search"`，含「Anthropic 服务端网页搜索（按次计费）」提示
- `onProtocol` 切离 `anthropic` 就地清零（切回不自动恢复）
- `submit` 恒携 `webSearch`（含 openai 下的 `false`，不省略字段）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 仅 anthropic 可见（AC-11） | 正向 | 编辑 `protocol: 'openai'` + `webSearch: false` 记录 → `queryByTestId('provider-web-search')` 为空（不在文档中）；切到 anthropic → checkbox 出现、未勾、计费提示文本在场 | 新增 |
| 存量 openai + true 的回显边界 | 边界 | 编辑 `protocol: 'openai'` + `webSearch: true`（他端写入的存量态）→ openai 下不渲染勾选项、不就地改值；切到 anthropic → 按 `toFormState(editing.webSearch ?? false)` 回显为已勾（协议切换不静默清零，仅「切离 anthropic」方向清零） | 新增 |
| 新建缺省关 | 边界 | 新建表单（`EMPTY_FORM`）切到 anthropic → 未勾；不填不改直接保存 → 载荷 `webSearch === false` | 新增 |
| 编辑态回显 | 正向 | `editing.webSearch === true` + anthropic → 勾选态在场；`webSearch` 键缺席 / `null`（旧记录）→ 未勾（`?? false`，不抛错） | 新增 |
| 切协议清零（AC-11 Scenario） | 异常 | anthropic 下勾选 → 切 openai → 保存 → `state.save` 实参 `webSearch === false`；再切回 anthropic → 仍未勾（不自动恢复，需用户重勾） | 新增 |
| 提交载荷恒携（AC-11） | 正向 | openai 下保存 → 实参含 `webSearch: false`（键在场，不省略）；anthropic + 勾选 → `webSearch: true`；实参位置序与 `AgentProviderSaveInput` 逐位对齐 | 新增 |
| 布尔控件与文本字段分离 | 边界 | 文档内不存在 `data-testid` 为 `provider-web-search` 的 `<input type="text">` 形态（不混入 `ProviderTextField` 受控文本路径）；连续点击两次勾选态正确切换 | 新增 |
| 既有表单契约回归 | 保留 | 协议选择器 / `base_url` placeholder 与提示 / `max_tokens < 4096` 拒绝提交 / 模型名后缀提示 / 列表遮蔽 api_key / 新建与编辑提交载荷既有用例全绿 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 动作轨道（被测组件显式入参） | 沿用既有 `fakeState` 装置：`AgentProvidersState` 的 `save` / `remove` 为 `vi.fn()`，断言实参 | 提交载荷 / 切协议清零 |
| 内部模块 | **不 mock**：`MaskedApiKey`、`StandardSelect`、`@base-ui/react` checkbox、`ProviderTextField` 真实组合 | 全部用例 |
| 无 IPC / 网络 / store | 组件不触达 IPC（动作经注入的 state 替身） | 全部用例 |

### packages/desktop/src/views/agents/hooks/use-agent-providers.ts -> packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts

> 来源：design.md「修改文件」表 use-agent-providers.ts 行 +「类型定义」表 `AgentProviderSaveInput` + AC-11 invoke 位置参数。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无本文件行）；以下条目溯源「修改文件」表 +「类型定义」表 + AC-11（未 grep 源文件导出声明）。 -->

#### 待测功能

- `AgentProviderSaveInput.webSearch: boolean`（末位）与 `commands.saveAgentProvider(..., input.webSearch)` 位置参数逐位对齐（生成绑定的第 9 位置参）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 位置参数补位（AC-11） | 正向 | `save({ id, name, protocol, baseUrl, apiKey, models, contextLength, maxTokens, webSearch })` → `invokeMock` 以 `('save_agent_provider', { …九键… })` 恰调用一次，`webSearch` 为第 9 位、键名逐字 | 新增 |
| `false` 也透传 | 边界 | `webSearch: false` → 载荷仍含该键（不得省略、不得落 `undefined`——省略会与「保持原值」混淆） | 新增 |
| 可空列组合边界 | 边界 | `contextLength: null` / `maxTokens: null` 与 `webSearch` 的 `true` / `false` 四组合 → 九位实参逐位对齐，无错位；补 `contextLength: 0` / `maxTokens: 4096` / `Number.MAX_SAFE_INTEGER` 三档 → 数值原样透传（不夹取、不转字符串），`webSearch` 仍居末位 | 新增 |
| 错误双轨不变 | 异常 | `invoke` reject → `toast.error` 前缀「保存 provider 失败：」不变、返回 `null`；`webSearch` 不改变错误面与刷新时序 | 保留 |
| 记录形状随动 | 修改 | fixture 补 `webSearch` 字段（bindings 再生成后的必填位）；`list_agent_providers` 返回记录的 `webSearch` 原样透传给消费方 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（`@tauri-apps/api/core`） | `vi.mock` 注入 `invoke` 按命令名分发替身（既有 `invokeMock` 装置，可切 resolve / reject 并记录调用序列）；生成绑定的 `commands.*` 保持真实（其内部调 mock 的 `invoke`） | 全部用例 |
| 提示通道（`sonner`） | `vi.mock` 注入 `toast.error: vi.fn()`（既有装置） | 错误双轨用例 |
| 内部模块 | 不 mock：`useAgentProviders` 本体、`AgentModelTiers` 类型、hooks 内部状态机真实运行 | 全部用例 |

---

## 不可测试项

| 被测文件或模块 | 原因 |
|----------------|------|
| `packages/desktop/src/types/generated/bindings.ts` | codegen 生成物禁手改：五变体 `AgentBlock` + `citations`、`AgentCitation` / `WebSearchSource` 类型、provider 记录 `webSearch`、`saveAgentProvider` 第 9 位置参由 `pnpm -C packages/desktop run bindings:export` 再生。**第四稿口径**：产物为**统一形态**（单 `AgentBlock` / `AgentEvent` / `AgentEventKind` / `AgentRunMessage`，零 `_Serialize` / `_Deserialize` 伴生类型；`citations?: AgentCitation[] \| null` 等可选位来自 `#[serde(default)]`）。零漂移由 `bindings:check`（`git diff --exit-code -- src/types/generated`）与 `packages/desktop/src-tauri/src/bindings/mod_test.rs` 的快照守卫承接（既有用例「重导出后逐字节比对」，随再生成自动刷新，本变更不新增断言面）。无进程内测试落点 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | design「修改文件」表登记的**核对结论**：四条 golden 断言（`AgentEvent` 对象形态 + `} & AgentEventKind;` 摊平、`agentStart` 首参 `Channel<AgentRunMessage>`、十参同序含尾 `cwd`、产物不含 `_Deserialize`）**零代码改动**，即 AC-13 与 D1 第四稿的**机械判据**——`skip_serializing_if` 一旦回流，`AgentBlock` 相位分裂并上溢，四条断言立即变红。故本文件**不新增、不改写**任何用例：被测对象是仓库级生成契约（全体 DTO 的公共守卫），改写 golden 迁就属性即 weaken 守卫（D1 明令禁止）；断言随 `bindings:export` 再生成自动生效（`web_search.rs` / `event.rs` 章节已登记「键恒在」的反向判据） |
| `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` | 改动为单条 `pub use event::{…, AgentCitation, WebSearchSource};`（纯再导出、零运行时逻辑）。可达性由编译面承载（`event_test.rs` / `model_test.rs` 的构造与 `bindings:export` 消费同一导出面，漏导出即编译失败）；AC-1 的「公开导出」按 proposal 验证方法以**源码复核导出面**为准（既有 `crates/core/agent/src/lib_test.rs` 的「crate 根导出面锚定」可随导入清单扩位，属机械补位，不新增断言语义） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/transcript.rs` | 改动为模式补位 `AgentBlock::Text { text }` → `Text { text, .. }`——新字段**不参与** `final_assistant_text` 拼接，零行为变更、无新增断言面；既有 `transcript_test.rs` 全部用例（文本块拼接顺序、缺席形态、空白裁剪）作回归承载 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 改动为模块挂载声明（`pub(crate) mod web_search;` + 测试相位加 `#[cfg(test)] mod web_search_test;`），零运行时逻辑；挂载正确性由编译面与 `web_search_test.rs` 全部用例可编译可执行承载 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs`、`packages/desktop/src/lib/agent-transport.ts` | 决策 ⑦ 的机械判据是「零 diff」（无 `AgentDelta::Search`）——「不存在变更」是静态性质，无运行时断言落点；由守线任务核验 diff + 既有 `normalize_test.rs` / `agent-transport.test.ts` 全绿承载（本变更**零新增用例**，出现新增用例即说明实现偏离） |
| `packages/desktop/src-tauri/src/main.rs` | 改动为 builder 链单行 `.plugin(tauri_plugin_opener::init())`（与 `dialog` 同位）。Tauri `Builder` 无单测装置（`main` 为二进制目标、无测试 harness），AC-12 按 proposal 既定验证方法以**源码复核**（与 `dialog` 同位）为准；插件真机行为不属单元测试面 |
| `packages/desktop/src-tauri/src/lib.rs` | design「事实偏差 1」的核对结论：**零代码改动**（builder 链不在本文件，`lib.rs` 只挂 `pub mod bindings; pub mod commands;`）。无行为面可断言 |
| `packages/desktop/src-tauri/capabilities/default.json` | 声明式权限配置（`opener:allow-open-url` + scope 只放 `http://*` / `https://*`；`description` 同步）。校验主体是 Tauri 的 capability schema + 权限逐项审阅（AC-12 的验证方法明示「`capabilities/default.json` 逐项审阅」）；design 待决问题 1 已按插件自带 `permissions/autogenerated/reference.md` 与 `src/scope.rs` 结项（标识符存在、`*` 跨 `/` 匹配、无宽权限项），故本项唯一形态已定，但仍是声明式配置、无进程内断言落点 |
| `packages/desktop/package.json` / `packages/desktop/src-tauri/Cargo.toml` / `packages/desktop/src-tauri/Cargo.lock` / `packages/desktop/pnpm-lock.yaml` | 依赖与版本元数据（`version` `0.5.29`、`tauri-plugin-opener = "2"`、`@tauri-apps/plugin-opener ^2.0.0`；两个锁文件为 install 的产物、非手写文件）。验证归依赖解析 + 构建链 + 静态检查链（`run check`）与变更说明复核，无进程内断言落点 |
| AC-13 静态检查链（`cargo clippy --workspace --all-targets`、`bindings:check`、`pnpm -C packages/desktop run check` 含 `knip`、`run test` 汇总） | 机械门禁由桌面端执行，非本相位单元测试面；本文件只设计各模块用例形态，不复制测试运行脚本细节 |
| 真机 `pause_turn` 的「相邻同角色 assistant 消息被服务端接受」（R5 / spec 边界条 16） | 需官方 Anthropic 端点 + 凭据，非自动化验证面。**design 第三稿已撤销实现期前置核验门**：该依赖以**已知限制**留痕，确认动作并入**归档后人工验收**（真实 `pause_turn` 会话续轮），结论记入验收记录；**不设实现期核验门、不作任务阻塞项**。进程内可考的只有「回放按原序产 use → result 两条 `AssistantContent`」（`resume_test` 章节承载），服务端接受与否无确定性断言面；降级预案（合并相邻 assistant 消息）本期不预建、无实现分支 |
| `web_search` 服务端工具的按次计费、真实检索质量与网关兼容性（R7 / R10） | provider 侧行为：本地只能锁定请求侧形状（`max_uses = 5`，见 `web_search.rs` 章节）与失败经既有 `api_error` 显式暴露的结构；检索质量、计费额度与不支持 server tools 的网关行为无进程内断言落点 |
| 未知 raw server 块（`code_execution_tool_result` 等）的跨轮保真（design 待决问题 6） | 知情边界（决策 ⑤「其他 raw server 块跳过」）：这类块不产 `AgentBlock`、不入转录，续轮丢失是接受行为，无断言面；本期不扩面，亦不设回归用例（避免把边界固化成期望） |
| `sweep_orphans` 对 `code_execution_tool_result` 的清扫（design 待决问题 7） | 本期按 spec 指定索引源只认 `web_search_tool_result`；未接入其他 server 工具，故无该形态的配对清扫断言面（`context_test` 章节仅断言「非 server raw 块原样保留」，不预建未来分支） |
