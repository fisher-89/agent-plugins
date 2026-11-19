# 设计: provider-effort-knob

> **变更**: provider-effort-knob
> **状态**: 设计定稿（基于已定稿 proposal + 3 份 delta spec）
> **基于**: `proposal.md`（决策执行表 ①–⑩ / 落地形状 / 目录五不变量 / 预填四纪律 / AC-1…AC-13 / 风险 R1–R14）、`specs/{desktop-agent-management,desktop-agent-execution,desktop-standard-components}/spec.md`、`explore.md`（rig 0.43.0 源码级复核）
> **性质**: **加列 + 加建议面**——store 一条版本链（provider v4 → v5，增 `efforts{high, low}` sibling 列）；引擎一条旁路载荷（`efforts` → 档位 `match` 的二元组 → `effort::params` 单点 shaping → `additional_params`）；前端一份静态建议目录（`model_catalog` 静态表 + `list_model_catalog`）+ 一枚标准组件（`StandardCombobox`）。**存量行为零变化**：`efforts` 两档留空 ⇒ 请求体与今天逐字相同。事件词汇 / 会话快照 / `crates/core/**` / `compact` 摘要路径零改动。
>
> **本篇边界**: `crates/infra/store`（记录 v5 + 双向 `From` + 出口）、`crates/infra/agent`（`sdk/effort.rs` / `model_catalog.rs` 两个新模块 + 门面与 runner 旁路 + loop 请求位）、`src-tauri/src/commands`（provider 可空调参组 `AgentProviderTuning` 与目录命令）、`packages/desktop/src`（标准组合框 + 管理页表单）、`.dev-team/architecture/models/06-frontend.toml`（一枚元素 + 两条关系）。**零新增 crate、零新增外部依赖、零新增依赖边**。

---

## 提案与规格同步状态

proposal 的十条决策、落地形状、目录五不变量、预填四纪律与 AC-1…AC-13 均已定稿；3 份 delta spec 把语义钉到了「字段名 / 键名 / 值语义 / 组件契约」粒度。本设计不重复论证，只做五件事：

1. **把 spec 已钉语义、未钉落点的形状收敛到具体签名与模块**（DD1–DD14）：v4 中转形态的 exact 形状、`effort::params` 的常量面与构造方式、日志诊断面的「同源双字段」、目录 DTO 的 derive 面、组合框的取值路线（`inputValue` 单真相）、预填留痕的数据形态。
2. **对 proposal 的六处措辞做工作树校准**（见「事实校准」）：`EngineFacade` 字面构造点是 **6** 处而非 4 处；`sdk/mod.rs` 的单测模块声明必须与 sidecar 同批；手写遮蔽 `Debug` 需从旧记录迁到新记录；`store/src/lib.rs` 的 `AgentEffortTiers` 出口是 proposal 遗漏的必需面；`#[serde(default)]` 与 bincode 升级链是两件事；前端两条既有断言的文案随动。
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略（用例与门禁由独立相位承接）。
4. **把三处「非显然正确」写成可实现形态**：rig 的 `provider_tools` 是 **按键合并**（`completion/request.rs:842-845` → `merge_provider_tools_into_additional_params`），故 effort 与 `tools` 共存无需自造合并；base-ui `ComboboxRoot` 在 `multiple = false` 时 `selectionMode` 恒 `single`（`ComboboxRoot.js:29`），且「输入框在弹层外」时选中条目会回填输入（`AriaCombobox.js:653`），故「可选可填」不需要自造文本回填；`single` 分支在关闭弹层时**不清空输入**（清空只在 `multiple` 分支，`AriaCombobox.js:624-631`），故受控 `inputValue` 不会被关闭动作吞掉。
5. **明确「必须一起动」的编译闭合面**（6 处结构体字面构造 + 13 处 `save_agent_provider` 调用点 + 前端形参位置对齐），使任一处字段面变化的收敛在同一阶段内完成。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **store 记录面**：`crates/infra/store/src/model.rs:1134` `AgentModelTiers`（三档，doc 内含「effort 进 run 参数与档位选择器为后续迭代」一句需改写）；`:1199` `AgentProviderRecordV1` / `:1219` `V2` / `:1270` `V3`（三者 `pub(crate)`、不进库模型组、无 `#[native_db]`）；`:1334` 当前 `AgentProviderRecord`（`#[native_model(id = 5, version = 4, from = AgentProviderRecordV3)]` + `#[native_db]`，尾部字段 `web_search: bool`）；`:1375` `From<V3> for AgentProviderRecord`、`:1394` `From<AgentProviderRecord> for V3`；`:1408` `impl AgentProviderRecord`（`new` 七参于 `:1411`、`with_web_search` 于 `:1437`）；`:1445` 手写遮蔽 `Debug`（字段面逐位列出，`api_key` 位走 `mask_api_key`）。
- **store 出口面**：`crates/infra/store/src/lib.rs:11` `pub use model::{…, AgentModelTiers, AgentProviderProtocol, AgentProviderRecord, …}`——`AgentEffortTiers` 必须同批入列（否则 `compose` / `src/commands` 取不到类型）。
- **字面构造面**：`AgentProviderRecord {` 的结构体字面量共 **6** 处——`crates/infra/store/src/describe_test.rs:89`、`crates/infra/agent/src/sdk/db_tool_test.rs:68`、`crates/infra/store/src/query_test.rs:218`、`crates/infra/store/src/model_test.rs:3805 / 3816 / 4018`；`envelope_test.rs:169` 与 `store_test.rs:3813` 的夹具走 `::new`（44 处 `::new` 调用点零改）。
- **命令面**：`src/commands/agents/mod.rs:54` `save_agent_provider`（10 参含 `State`，末位 `web_search: bool`；`:66-77` 模型名后缀与 `MIN_OUTPUT_TOKENS` 两条既有校验；`:78-97` 构造 → `with_web_search` → 留空 key 回填）；`save_agent_provider(` 全仓 **14** 处（1 定义 + 13 处 `mod_test.rs` 调用）。**参数位已满**：specta `SpectaFn` 只实现到 `fn(T1..T10)` 且 `State` 计入其一（`agent_start` 同样停在 10 参），加第 11 参即 `bindings:export` 编译失败——见校准 9。`src/commands/mod.rs:29` `all_commands!` 宏，agents 轨道占 `:43-49` 七行。
- **引擎穿透链**：`crates/infra/agent/src/sdk/runner.rs:67` `SdkRunner` 字段面（`config` / `resume` / `context_window` / `max_tokens` / `web_search` / `explore_store` / `workspace_root` / `db_global` / `db_workspace` / `ask_registry` / `ask_port`）；`:97` `new`（唯一字面构造，其余 builder 皆 `..self`）；`:138` `with_max_tokens` / `:155` `with_web_search`；`:203` `open_session`，`:207-210` 档位唯一分发点 `match open.ctx.model_level { High => ("model_high", …), Low => ("model_low", …) }`；`:390` `PumpPayload`（含 `output_tokens: Option<u64>` 与注释「日志列 MUST NOT 另走一遍取值链」）；`:527-535` 泵启动日志（`protocol=… model=… window=… max_tokens=… web_search=… raw=…`）；`:550-570` `LoopTurn` 铸造点。
- **loop 请求位**：`crates/infra/agent/src/sdk/loop.rs:120` `LoopTurn`（`:158` `web_search: bool` 注释写明「已按协议门控的最终值」）；`:317-329` `CompletionRequest` 构造（`:328` `additional_params: None`）；`:335-337` `if turn.web_search { request = request.provider_tool(web_search::provider_tool()) }`。`compact.rs:54` 摘要请求 `additional_params: None`。
- **门面 / 组合根**：`crates/infra/agent/src/lib.rs:42` `EngineFacade`；`:74` `new`；`:109` `with_context_window` / `:127` `with_max_tokens` / `:146` `with_web_search` / `:165` `with_explore_note_store` —— **这 6 处是逐字段字面构造**（`with_resume_transcript` 亦然；`with_ask_registry` / `with_ask_port` / `with_workspace_root` / `with_db_stores` 用 `..self`）；`:224` `runner_for`（唯一 match 点，Sdk 臂链 `.with_max_tokens(..).with_web_search(..)…`）。`compose.rs:169` `ResolvedEngine`（`kind` / `config` / `context_window` / `max_tokens` / `web_search` / `agent_instance_id`）；`:272-283` 门面链；`:379` `resolve_agent_engine`，Sdk 臂 `:397-437`（`web_search: provider.web_search && matches!(protocol, Protocol::Anthropic)` 为门控单点）。
- **协议词汇**：`sdk/config.rs:12` `Protocol { Openai, Anthropic }`（刻意不派生 serde / specta，引擎内部词汇）；`as_str()` 供日志线值。
- **命令面出口**：`src-tauri/src/bindings/mod.rs` 的 builder 链含 `.dangerously_cast_bigints_to_number()` 与 `.disable_serde_phases()`——目录的 `Option<u64>` 出线为 `number | null`（与 id / 时间戳同口径），`#[serde(default)]` 字段出线为**必填键**。
- **前端**：`packages/desktop/src/views/agents/components/provider-panel.tsx`（519 行）：`:13` `ProviderFormState`（九字段）、`:31` `EMPTY_FORM`、`:70` `ProviderTextField = Exclude<…, 'id' | 'protocol' | 'webSearch'>`、`:73` `PROVIDER_FIELDS`（含 `provider-model-*` 三档与 `context_length` / `max_tokens` 两项，`:121` placeholder「留空 = 按 context_length 分档」）、`:157` `suffixHint`（`:170` 结尾「后缀优先于 context_length」）、`:176` `toFormState`、`:284` `ProviderFormRows`（Input 映射渲染）、`:325` `ProviderHints`（`:330` 提示含 `max_tokens：… 留空 = 按 context_length 分档推导`）、`:406` `ProviderList`、`:469` `ProviderPanel`。`hooks/use-agent-providers.ts:22` `AgentProviderSaveInput`、`:92` `ipc.saveAgentProvider(...)` 九实参。`agents-view.tsx` 装配两取数 hook 下传 `ProviderPanel`。
- **底座可用性**：`packages/desktop/src/components/ui/` 有 `select.tsx`、无 `combobox.tsx`；`@base-ui/react/combobox` 目录已装（`Root` / `Input` / `Portal` / `Positioner` / `Popup` / `List` / `Item` / `Empty` / `Clear` 等 parts）。`components/standard/select.tsx`（25 行、props 面）是 `StandardCombobox` 的同层先例。
- **版本**：`packages/desktop/package.json:3` `version = "0.6.54"`；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不随动。
- **archi 模型**：`.dev-team/architecture/models/06-frontend.toml:141` `App.Frontend.Components.Standard`（summary「通用交互组件集合（对话框、抽屉、分页、选择）」）、`:164` `…Standard.Select`（`kind = "component"` / `gate = "off"` / `path`）；关系 `:322-325` `Select → Components.Ui`、`:367-370` `Views.Agents → Components.Standard.Select`。`archi_module` 实测 `Select` 的 `api.status = "none"`（无契约文件即 `none`）。

### 事实校准（proposal / spec 措辞 vs 工作树实测；不改需求意图，只定实现形态）

1. **`EngineFacade` 字面构造点是 6 处，不是 4 处**（proposal「改动」列写「四个构造点补齐」）。实测逐字段字面构造为 `new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_web_search` / `with_explore_note_store`；`with_efforts` 本身用 `..self`。加字段的机械闭合面以 **6 处**为准，漏一处即 `E0063` 编译错误（该错误即是闭合面清单，不需另立名单）。
2. **`store/src/lib.rs` 是 proposal「实现文件」表遗漏的必需面**：`AgentEffortTiers` 要被 `crates/infra/agent`（`ResolvedEngine` / `SdkRunner`）与 `src-tauri/src/commands/agents`（命令签名）取到，`pub use model::{…}` 必须同批入列。本设计把它列入「修改文件」。
3. **手写遮蔽 `Debug` 必须迁移而非复制**：`model.rs:1445` 的 `impl Debug for AgentProviderRecord` 逐位列出 v4 字段面。改名后该实现成了「v4 中转型的手写 Debug」——`V1`–`V3` 全是 `#[derive(Debug)]`，故 v4 中转形态回退为 `#[derive(Debug)]`（明文可见但只作升级链中转、不进任何日志位），手写遮蔽 `Debug` 迁到新 v5 记录并补 `efforts` 位。两份手写实现（遮蔽键 + 明文键）并存是刻意不做的事：遮蔽是**记录本体**的属性，中转形态没有日志面。
4. **`#[serde(default)]` 与升级链是两件事，不是重复**：`efforts` 上的 `#[serde(default)]` 只兜 JSON 面（缺键读出空值）；bincode 非自描述、旧行更短，升级语义**只由 `From<AgentProviderRecordV4>` 承担**。两者同处不是冗余，故不写「二选一」的注释。
5. **`sdk/mod.rs` 的「挂载 effort 模块与单测模块」分两相位落地**：实现相位只加 `pub(crate) mod effort;`；`#[cfg(test)] mod effort_test;` 必须与 `sdk/effort_test.rs` **同批**（先加声明会让 `cargo clippy --all-targets` / `cargo check --all-targets` 因缺文件失败）。`crates/infra/agent/src/lib.rs` 的 `mod model_catalog;` / `#[cfg(test)] mod model_catalog_test;` 同形处理。
6. **AC-10 与 AC-11 的两条既有断言互相牵扯**：`provider-panel.test.tsx:496`（`后缀优先于 context_length`）与 `:685`（`留空 = 按 context_length 分档推导`）指向的文案在 AC-11 下必须改述为不含英文键名的形态；「提示仍在」的判据落在**改述后的文案**上（`后缀优先于上下文窗长` / `留空 = 按上下文窗长分档推导`），不是逐字保留旧串。proposal 测试文件表已登记这两条为随动。
7. **`list_model_catalog` 的出线类型名**：specta 按结构体名生成，前端类型为 `ModelCatalog` / `CatalogProtocol` / `CatalogModel`，命令函数为 `listModelCatalog`（`bindings.ts` 内 `__TAURI_INVOKE("list_model_catalog", {})` ）。`ipc.ts` 是对生成物 `commands` 的 Proxy，零改动即获得参数 / 结果面校验。
8. **`provider-panel.tsx` 的模型三档不再走 `PROVIDER_FIELDS` 的 Input 渲染**：三档模型进组合框后，文本字段表只剩 `name` / `baseUrl` / `apiKey` / `contextLength` / `maxTokens`；`ProviderTextField` 的排除集必须同步收窄（否则 `String(form[field.key])` 仍把模型键当文本框渲染，diff 复核时会看到两套控件并存）。

9. **IPC 参数位有硬上限，四项可空调参必须收组**（实现期回溯，proposal「实现期回溯 1」）：specta 的 `SpectaFn` 只实现到 `fn(T1..T10)`，`State` 计入其一——`save_agent_provider` 现签名已占满 10 参（`agent_start` 同样停在 10 参），平铺加 `efforts` 会让 `bindings:export` 直接编译失败（E0277，指向 `all_commands!` 展开）。故四项**同族**可空调参（`context_length` / `max_tokens` / `web_search` / `efforts`——本设计一贯表述为「门面旁路载荷，MUST NOT 进 `EngineConfig`」）收成末位一个 IPC 入参组 `AgentProviderTuning`（DD15），命令降到 8 参。语义面零变化：`efforts` 只 `trim`、`web_search` 无条件接受、既有两条校验与错误串逐字不动；记录形态（v5）不受影响。

---

## 关键设计决策（DD1–DD15）

> 决策 ①–⑩ 见 proposal「决策执行表」（已定稿，本篇不重复）。下表是本设计在 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | v4 → v5 的 exact 形状 | 现记录整体改名为 `AgentProviderRecordV4`（`pub(crate)`，`#[native_model(id = 5, version = 4, from = AgentProviderRecordV3)]`，**去掉 `#[native_db]` 与其专属 `#[primary_key]` 助手属性**——该属性是 `native_db` 派生宏的助手，脱离 derive 即未知属性；V1–V3 中转形态同样无这两个属性，保留 `Serialize/Deserialize/Type/Clone/PartialEq/Eq`，`Debug` 回退为 derive）；新 `AgentProviderRecord` 为 `version = 5` + `from = AgentProviderRecordV4` + `#[native_db]`，`efforts` 列**追加在字段面末尾**（`web_search` 之后）；`From` 两向逐字段搬运（升级补空、降级丢列）；原 `From<V3>` / `From<…> for V3` 两臂改指 v4 | 改名而不复制：复制会让同一字段面出现两份真相（改一处忘另一处即静默丢列）。`efforts` 追加在末尾是 bincode 布局的硬要求（`From` 的字段构造序与解码序必须同步演进）；`#[native_db]` 必须随名字一起摘掉——留着等于把中转形态注册进库模型组（同一 `id = 5` 两个登记行） |
| DD2 | `AgentEffortTiers` 的 derive 面 | `#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]`，两字段 `high` / `low: String` | `Default`（两档空串）是 `AgentProviderRecord::new` 参数面不动的实现前提——构造器不新增参数，缺省值只能来自 `Default`。`Type` 是 IPC 入参 / 出参的要求（记录字段面与 `AgentProviderTuning.efforts` 双向出线）。**不落独立模型**（无 `#[native_model]`）：嵌装先例同 `AgentModelTiers` |
| DD3 | shaping 单点的签名与构造方式 | `pub(crate) fn params(protocol: Protocol, effort: &str) -> Option<serde_json::Value>`；三个常量 `OPENAI_EFFORT_KEY` / `ANTHROPIC_OUTPUT_CONFIG_KEY` / `ANTHROPIC_EFFORT_KEY`（`pub(crate) const &str`）；实现用 `serde_json::Map` 逐键插入（`json!` 宏的动态键需括号语法，`Map` 写法让常量天然成为唯一键源） | `json!({ "reasoning_effort": v })` 会引入第二份键名字面量（常量面即失效）；`Map` 构造让「键名单点」是结构性的而非纪律性的。`&str` 入参 + 内部 `trim()` 使「留空」判据只有一处（调用方不必先洗值）；返回 `Option` 而非空对象，是 AC-4「留空零痕迹」的类型化表达 |
| DD4 | 引擎日志的「生效 effort」从哪来 | `PumpPayload` 增**两个**字段：`effort: String`（该轮生效原值，日志 `effort=…`，留空记 `-`）与 `effort_params: Option<serde_json::Value>`（已 shaping 的 wire 载荷）；两者由 `open_session` 同一处三元绑定派生 | 日志要报**原值**（`max`），wire 要报**形状**（`{"reasoning_effort":"max"}`）——从载荷反推原值需要按协议再拆一次键（等于第二个协议判定点）。两字段同源于一处绑定，正是 `output_tokens` 的既有纪律（「日志列 MUST NOT 另走一遍取值链」）在 effort 上的同形复用。备选「只带 `effort_params`、日志打 JSON」被拒：诊断面会从 `effort=max` 退化成一行转义 JSON |
| DD5 | `model_catalog` 的模块落点 | 新建 `crates/infra/agent/src/model_catalog.rs`（crate 根，`mod model_catalog;` 挂在 `lib.rs`），不是 `sdk/` 子模块 | 目录不是 wire 词汇（不含协议 shaping、不被 `sdk` 任何模块消费），却要被 `src-tauri/src/commands` 以 crate 级路径取用；放 `sdk/` 会给它一个「引擎内部」的错误归属，也让 `src/commands` 不得不多穿一层 `sdk::model_catalog` 路径。目录与 shaping 同 crate（解决 proposal 决策 ④ 的「不漂移」诉求）已在 crate 粒度满足 |
| DD6 | 目录 DTO 的类型与 derive 面 | `ModelCatalog { protocols: Vec<CatalogProtocol> }`；`CatalogProtocol { protocol: store::AgentProviderProtocol, efforts: Vec<String>, models: Vec<CatalogModel> }`；`CatalogModel { value: String, context_length: Option<u64>, max_tokens: Option<u64>, supports_thinking: bool, efforts: Option<Vec<String>> }`；三者 `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]` + `#[serde(rename_all = "camelCase")]` | `protocol` 复用 store 的线值枚举：编一套平行的字符串枚举等于给「协议」造第二份真源（且前端 `AgentProviderProtocol` 已是同一联合类型，复用即零映射）。**不 derive `Deserialize`**：目录只出线、不解析（多一个 derive 就多一条「有人会把它当入参」的误读通道）。字段不做 `#[serde(default)]`：`Option` 缺席在出线上呈 `null`（zod `strictObject` 要求键在场），前端只需处理「`null` = 无建议」一态 |
| DD7 | 静态表的构造形态 | `pub fn catalog() -> ModelCatalog` 每次返回**新构造**的对象（纯字面量组装），不引 `static` / `OnceLock` / `include_str!` | 体量是几十条记录的常量级结构，构造成本低于一次 IPC 序列化；`OnceLock<ModelCatalog>` 会引入共享可变边界（且 `&'static` 返回要求调用方 clone，命令侧反而多一步）。零 IO、零锁、零生命周期负担是「纯静态参考数据」的最短实现 |
| DD8 | 前端目录取数的降级形态 | `useModelCatalog(): { catalog: ModelCatalog \| null; loading: boolean }`；挂载一次（`useEffect` + `cancelled` 标志），失败 `setCatalog(null)`、**不置 error 态、不 toast、不重试**；无 `tick` / `refresh` | 目录是建议面，失败的正确表现是「没有建议」而不是「页面报错」——spec 明令「不呈现阻断性 error、不覆盖清单的 error 态」。`loading` 只为避免首帧「无建议」与「取数中」混淆；不给刷新入口是因为目录是编译期常量，重取永远同值 |
| DD9 | 建议联动 / 预填的纯函数落点 | 全部作为 `provider-panel.tsx` 的**模块私有**函数（与 `BASE_URL_HINTS` / `parseMaxTokens` / `suffixHint` 同址）：`protocolCatalog(catalog, protocol)` / `catalogEntry(catalog, protocol, modelValue)` / `modelSuggestions(catalog, protocol)` / `effortSuggestions(catalog, protocol, modelValue)` / `prefillOf(entry, protocol)` | 不新建文件：这五个函数是同一份表单渲染的纯查表陪伴逻辑，拆文件只会让「表单字段定义」与「字段取值规则」分居两处。也不做成 hook：它们无状态、无副作用，测试经组件渲染即可覆盖 |
| DD10 | 组合框的取值路线（两个真相的消除） | `value`（选中值）**不受控且不读取**；`inputValue = value` 受控，`onInputValueChange(next) => onChange(next)`；条目按下由底座按 `single && !inputInsidePopup` 回填 label（`AriaCombobox.js:653`），同一条文本流收回；`items.length === 0` 时**不渲染弹层子树**（退化纯输入框） | 唯一真相 = 输入文本。若同时受控选中值，会出现「选中值 `low`、文本 `my-low`」的双真相，且任何「按选中值回写文本」的底座路径都会在 blur / 重开时把用户键入吞掉。`value` 不控制 ⇒ 底座无从回写；`single` 分支关闭弹层不清输入（`AriaCombobox.js:624-631` 的清空只在 `multiple`）⇒ 受控文本不会被关闭动作覆盖。空清单不渲染 `Popup` 是「目录取数失败即无建议」的物理落点（`listbox` 不在文档中） |
| DD11 | 表单状态字段面与文本字段收窄 | `ProviderFormState` 增 `effortsHigh` / `effortsLow`；新增 `type ProviderComboKey = 'high' \| 'medium' \| 'low' \| 'effortsHigh' \| 'effortsLow'`；`ProviderTextField = Exclude<keyof ProviderFormState, 'id' \| 'protocol' \| 'webSearch' \| ProviderComboKey>`；`PROVIDER_FIELDS` 五条（`name` / `baseUrl` / `apiKey` / `contextLength` / `maxTokens`），模型与 effort 另立 `PROVIDER_COMBO_FIELDS`（`{ id, label, key, kind: 'model' \| 'effort' }` 五项） | 组合框与 Input 的 props 面不同（`items` / `onChange(value: string)`），混在同一映射表里必然要 `if (kind)` 分叉渲染；两张表各自同构，`ProviderTextField` 的 `Exclude` 也就能表达「哪些键只能走 Input」这条不变量（漏收窄即 TS 编译错误，不会静默渲染两套控件） |
| DD12 | 预填留痕的形态与清除规则 | `ProviderForm` 内独立 state：`const [prefilled, setPrefilled] = useState<ReadonlySet<'contextLength' \| 'maxTokens'>>(new Set())`；选中目录条目时**只为实际写入的字段**打标记；用户手动编辑该字段即撤标记（`onField` 内 delete）；`toFormState`（打开编辑态）**恒空集**；提交成功 / 取消即随组件卸载丢弃 | 标记是**展示层**事实（spec：MUST NOT 为此扩存储），故不塞进 `ProviderFormState`（那个 state 是提交载荷的投影，多一个键就多一个「要不要提交」的歧义）。三态（未预填 / 预填后被手改 / 预填后未动）中只有「预填后未动」该显示弱标记，撤标记挂在 `onField` 是唯一正确的时机。打开编辑态恒空集是纪律 1（不自动预填）的实现落点 |
| DD13 | 中文标签与提示的**确切文案** | 标签：`上下文窗长（token）` / `输出上限（token）`；`context_length` placeholder 保持「默认 128K」、`max_tokens` placeholder 改「留空 = 按上下文窗长分档」；`max_tokens` 提示改「输出上限：留空 = 按上下文窗长分档推导（未配置时 32000）；仅 anthropic 协议生效（openai 协议请求保持省略上限）；显式填值优先且须 ≥ 4096」；后缀提示尾部改「… 档窗长已被模型名覆盖（后缀优先于上下文窗长）」；两处弱标记文本「来自目录」 | AC-11 要求「不再裸写英文键名」，故 `max_tokens：` 前缀也须去掉（只留「输出上限：」）；语义一字不减（分档口径、32000、4096 下限、协议边界全在）。后缀提示保留「覆盖 / 优先」语义（AC-10 的判据）只是把键名换成中文称谓 |
| DD14 | archi 元素的落法 | `06-frontend.toml` 增 `[element."App.Frontend.Components.Standard.Combobox"]`（`kind = "component"` / `gate = "off"` / `path` / `summary = "可选可填组合框（自由输入 + 建议清单）"`）+ 两条关系（`Combobox → App.Frontend.Components.Ui`、`App.Frontend.Views.Agents → Combobox`）；`Components.Standard` 的 summary 补「组合框」；生成物（`Frontend/**`、`graph/INDEX.toml`、`*.schema.json`、`*.i.ts`）只经 `pnpm -C packages/desktop run api:export` 产出 | 与 `Select` 同形是机械对称（同层组件、同 gate、同 kind）；两条关系是双向可读性的最小集（依赖原语层、被视图层消费）。`api = none` 不宜写：本层组件确实无公共 API，但缺省 `declared` + 无契约 md 即 `docMissing`（warn，永不纳管门禁，见 proposal R14）；若实现期一并写契约 md 则计数不增，两者皆合法，故**不写 `api` 字段**（保持与既有 `Select` / `Dailog` 等同形，不在此变更引入新的模型写法） |
| DD15 | 四项可空调参的 IPC 入参形态 | `save_agent_provider` 末位收一个 `AgentProviderTuning { context_length: Option<u64>, max_tokens: Option<u64>, web_search: bool, efforts: AgentEffortTiers }`（`serde` camelCase → `{contextLength, maxTokens, webSearch, efforts}`；`Deserialize + Serialize + specta::Type`，定义在命令模块内），命令降到 8 参 | 参数位上限（校准 9）决定了「必须先减参」。把四项收成一组而不是只收 `web_search + efforts`：四者本就是 spec 反复表述的同一族（「可空调参 / 门面旁路载荷四项，MUST NOT 进 `EngineConfig`」），拆成「组内 2 + 组外 2」会让同族概念在签名上分裂。备选「`efforts` 塞进 `AgentModelTiers` 入参」与决策 ⑧ 直接冲突；备选「去掉 `#[specta::specta]`」丢失参数面编译期锚与运行期 zod 校验（R12 的缓解措施），皆被拒 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| provider 记录演进（**改**） | `AgentEffortTiers` 新类型；v4 形态改名作中转 + 新 v5 记录（`efforts` sibling 列）+ 双向 `From`；手写遮蔽 `Debug` 迁到 v5 并补位；`new` 参数面不动 + `with_efforts` | `crates/infra/store/src/model.rs`、`crates/infra/store/src/lib.rs`（出口） | 既有 `native_model` / `native_db` / `serde` / `specta` | bincode 版本链（既有机制，与 V1→V4 逐级同形） |
| effort shaping 单点（**新增**） | 三个键名常量 + `params(protocol, effort)` 纯函数：裁剪后空 ⇒ `None`，两臂 wire 形状各自成形 | `crates/infra/agent/src/sdk/effort.rs`（新）+ `sdk/mod.rs` 一行 | `sdk::config::Protocol`、`serde_json` | 纯函数，零 IO、零协议外知识 |
| 引擎穿透链（**扩**） | `ResolvedEngine.efforts`（原样带出、零门控）→ `EngineFacade::with_efforts` → `SdkRunner::with_efforts` → `open_session` 三元绑定 + 一次 shaping → `PumpPayload{effort, effort_params}` → 日志 `effort=…` → `LoopTurn.effort_params` → `additional_params` | `crates/infra/agent/src/{compose,lib}.rs`、`sdk/{runner,loop}.rs` | 既有门面 / 泵 / loop | 旁路 builder（与 `with_web_search` 同型） |
| 与 `web_search` 共用请求位 | `additional_params = turn.effort_params.clone()` 先落，`provider_tool(..)` 后按 `tools` 键合并（rig `merge_provider_tools_into_additional_params`），两键互不覆盖 | `crates/infra/agent/src/sdk/loop.rs` | rig 0.43 merge 语义 | 既有 `provider_tool` 路径零改 |
| 模型目录（**新增**） | 三个 DTO + 静态表 + `catalog()`；五条不变量由后端测试锚定（测试相位） | `crates/infra/agent/src/model_catalog.rs`（新）+ `lib.rs` 的 `mod` / `pub use` | `store::AgentProviderProtocol`、`serde`、`specta` | 编译期静态字面量 |
| 命令面（**扩**） | `save_agent_provider` 末位改收 `tuning` 组（`efforts` 只 `trim`）+ `list_model_catalog`（零参零 IO） | `src/commands/agents/mod.rs`、`src/commands/mod.rs`（`all_commands!` 一行） | `store` / `agent_runtime` | 薄包装（既有模板） |
| 标准组合框（**新增**） | 原语层包装（输入 + 弹层 + 条目）+ 标准层薄壳（值语义 / 空串留空 / 空清单退化） | `packages/desktop/src/components/ui/combobox.tsx`、`components/standard/combobox.tsx`（均新） | `@base-ui/react/combobox`、`cn` | React + base-ui（与 `Select` 同层同风格） |
| 管理页目录面（**改**） | 目录取数收口 hook；表单三档模型 + 两档 effort 改组合框；建议联动；预填四纪律 + 弱标记；两项标签中文化；清单行呈 effort | `views/agents/hooks/use-model-catalog.ts`（新）、`hooks/use-agent-providers.ts`、`components/provider-panel.tsx`、`agents-view.tsx`（装配） | `ipc` 生成物、`StandardCombobox` | 既有取数 hook 模式（无轮询） |
| archi 模型（**改**） | 一枚元素 + 两条关系 + 域 summary 补词；生成物经 `api:export` | `.dev-team/architecture/models/06-frontend.toml` | `archi-api-gen` | 手写模型 + 机械生成物 |

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀。测试文件与用例归测试相位；本清单只列实现文件，另设「编译闭合批」登记必须随字段面 / 形参面收敛的既有夹具点位。

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `crates/infra/agent/src/sdk/effort.rs` | effort 协议 shaping 单点：`OPENAI_EFFORT_KEY` / `ANTHROPIC_OUTPUT_CONFIG_KEY` / `ANTHROPIC_EFFORT_KEY` 三常量 + `params(protocol, effort) -> Option<serde_json::Value>`。模块头写清「为什么键名必须单点 / 为什么留空返回 `None` 而不是空对象 / 为什么零值域校验」 |
| `crates/infra/agent/src/model_catalog.rs` | 模型目录：`ModelCatalog` / `CatalogProtocol` / `CatalogModel` 三 DTO + `catalog()` 静态表。模块头写清「目录只作建议——不参与保存期校验、不参与运行期解析；名单会过时是知情代价，形状与不变量才是契约」 |
| `packages/desktop/src/components/ui/combobox.tsx` | `@base-ui/react/combobox` 原语层包装：`Combobox`（Root）/ `ComboboxInput` / `ComboboxContent`（Portal + Positioner + Popup + List）/ `ComboboxItem`。只组装原语与样式，零业务语义（无建议来源、无留空策略） |
| `packages/desktop/src/components/standard/combobox.tsx` | `StandardCombobox` 薄壳：`value` / `onChange` / `items` / `placeholder` / `disabled` / `data-testid` / `id` / `className`；`data-value` 落输入框；`items` 空即退化纯输入框 |
| `packages/desktop/src/views/agents/hooks/use-model-catalog.ts` | `useModelCatalog(): ModelCatalogState`：挂载一次取数、无轮询、失败降级为 `catalog = null`（不置 error、不 toast） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/store/src/model.rs` | ①`AgentModelTiers` 之后新增 `AgentEffortTiers`（DD2）；②现 `AgentProviderRecord` 整体改名 `AgentProviderRecordV4`（`pub(crate)`、去 `#[native_db]`、`Debug` 回退 derive、`version = 4` 头保持、其余逐字不动）；③新增 v5 `AgentProviderRecord`（`#[native_model(id = 5, version = 5, from = AgentProviderRecordV4)]` + `#[native_db]`，字段面末尾追加 `#[serde(default)] pub efforts: AgentEffortTiers`）；④`From<V3> for …` 与反向两臂改指 V4，新增 `From<V4> for v5`（补空）/ `From<v5> for V4`（丢列）；⑤`new` 的字面构造补 `efforts: AgentEffortTiers::default()`（签名逐字不动）+ 新增 `with_efforts`；⑥手写遮蔽 `Debug` 迁到 v5 并补 `.field("efforts", &self.efforts)`；⑦`AgentModelTiers` 的 doc 里「effort 进 run 参数与档位选择器为后续迭代，本期只存不选」一句改写为「深度轴由 sibling 列 `efforts` 承担（provider-effort-knob），per-run effort 参数仍为后续」（**结构零改**） | 一条链 v4 → v5，不新增第二条版本链；`AgentModelTiers` 三档结构 MUST NOT 变形 |
| `crates/infra/store/src/lib.rs` | `pub use model::{…, AgentEffortTiers, …}`（字母序落在 `AgentEngineKind` **之前**——`AgentEffortTiers` < `AgentEngineKind`） | 校准 2：跨 crate 取用前提 |
| `crates/infra/agent/src/sdk/mod.rs` | `pub(crate) mod effort;`（字母序落在 `db_tool` 与 `explore_note` 之间） | 单测模块声明由测试相位与 sidecar 同批（校准 5） |
| `crates/infra/agent/src/lib.rs` | ①`mod model_catalog;`（`compose` 与 `sdk` 之间）+ `pub use model_catalog::{catalog, CatalogModel, CatalogProtocol, ModelCatalog};`；②`EngineFacade` 增 `efforts: store::AgentEffortTiers` 字段；③6 处字面构造（`new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_web_search` / `with_explore_note_store`）各补一行；④新增 `with_efforts`；⑤`runner_for` 的 Sdk 臂链追加 `.with_efforts(self.efforts.clone())` | 门面旁路由三项扩为四项；`runner_for` 签名与唯一 match 点零塌 |
| `crates/infra/agent/src/compose.rs` | ①`ResolvedEngine` 增 `efforts: store::AgentEffortTiers`；②`resolve_agent_engine` 的 Sdk 臂补 `efforts: provider.efforts,`（**原样带出、零门控**，位置紧随 `max_tokens`）；③`compose_turn` 的门面链在 `.with_web_search(..)` 之后追加 `.with_efforts(resolved.efforts)` | `web_search` 的门控判定行零改动（两协议都有 effort，MUST NOT 参与该门控） |
| `crates/infra/agent/src/sdk/runner.rs` | ①`SdkRunner` 增 `efforts: store::AgentEffortTiers` 字段（`new` 内 `AgentEffortTiers::default()`，签名不动）；②新增 `with_efforts`；③`open_session` 的档位 `match` 由二元扩三元：`High → ("model_high", model_high.clone(), self.efforts.high.clone())` / `Low → (…)`，紧随其后 `let effort_params = effort::params(self.config.protocol, &effort);`；④`PumpPayload` 增 `effort: String` / `effort_params: Option<serde_json::Value>` 并补构造；⑤泵启动日志格式串插入 ` effort={}`（留空记 `-`）；⑥`LoopTurn` 铸造点补 `effort_params` | shaping 只在 `open_session` 一处发生；`missing` 校验的 `(field, is_missing)` 循环仍只看三元组的前两项（effort 空是合法态，不参与配置齐备校验） |
| `crates/infra/agent/src/sdk/loop.rs` | ①`LoopTurn` 增 `pub effort_params: Option<serde_json::Value>`（紧随 `web_search`，doc 写明「已按协议 shaping 的 wire 载荷；loop 零协议判定、零键名字面量」）；②`additional_params: None` → `additional_params: turn.effort_params.clone()` | `provider_tool(..)` 的挂载代码逐字不动（rig 按键合并）；loop 内零协议分支、零键名字面量 |
| `src/commands/agents/mod.rs` | ①`use store::{…, AgentEffortTiers, …}`、`use agent_runtime::{catalog, ModelCatalog}`；②`save_agent_provider` 末位追加 `efforts: AgentEffortTiers`，函数体加「两档各自 `trim()`」并改为 `.with_web_search(web_search).with_efforts(efforts)`；doc 补 effort 保存语义（只 trim / 零值域校验 / 零协议门控）；③新增 `list_model_catalog() -> Result<ModelCatalog, String>` | 既有校验（模型名后缀、`MIN_OUTPUT_TOKENS`、留空 key 回填）逐字保留；错误面零变化 |
| `src/commands/mod.rs` | `all_commands!` 的 agents 轨道末行（`set_default_agent_instance` 之后）追加 `$crate::commands::agents::list_model_catalog,` | 单一登记面，`generate_handler` / `collect_commands` 同源 |
| `packages/desktop/package.json` | `version` `0.6.54` → `0.6.55` | 版本纪律；`dependencies` / `devDependencies` 零变化 |
| `packages/desktop/src/types/generated/{bindings.ts,bindings.zod.ts}`（+ `bindings.zod.check.ts` 由脚本产出） | 再生成（**非手改**）：`AgentEffortTiers` / provider 记录 `efforts` 键 / `AgentProviderTuning`（`saveAgentProvider` 末位形参）/ `ModelCatalog` + `CatalogProtocol` + `CatalogModel` / `listModelCatalog` | 经 `pnpm -C packages/desktop run bindings:export` |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | `AgentProviderSaveInput` 增 `efforts: AgentEffortTiers`；`ipc.saveAgentProvider(...)` 末位传 `{ contextLength, maxTokens, webSearch, efforts }` 组（入参序与八参签名逐位对齐） | 位置参数逐位对齐；错误双轨语义不变 |
| `packages/desktop/src/views/agents/components/provider-panel.tsx` | ①`ProviderFormState` 增 `effortsHigh` / `effortsLow`，`EMPTY_FORM` / `toFormState` 同步；②`ProviderComboKey` + `ProviderTextField` 收窄 + `PROVIDER_COMBO_FIELDS` 五项 + combo 渲染组件；③建议联动五个私有纯函数（DD9）；④预填四纪律 + `prefilled` 状态与弱标记（DD12）；⑤`context_length` / `max_tokens` 标签与提示中文化（DD13）；⑥提交载荷增 `efforts: { high, low }`；⑦`ProviderList` 行呈 `effort high … / low …`（留空不呈） | 三档模型的三条 `PROVIDER_FIELDS` 条目移除；`webSearch` 的布尔位与清零逻辑零改动 |
| `packages/desktop/src/views/agents/agents-view.tsx` | 装配 `const catalogState = useModelCatalog();` 并把 `catalog` 下传 `ProviderPanel` | 取数收口在视图根（与 `providersState` 同址），面板保持无取数职责 |
| `.dev-team/architecture/models/06-frontend.toml` | 增 `App.Frontend.Components.Standard.Combobox` 元素 + 两条关系；`Components.Standard` 的 summary 补「组合框」 | 手写模型；生成物不在此列 |
| `.dev-team/architecture/models/Frontend/**`、`.dev-team/architecture/graph/INDEX.toml`、`*.schema.json`、`*.i.ts` | 经 `pnpm -C packages/desktop run api:export` 再生成（**非手改**） | `api:check` 零 diff 是门 |

### 编译闭合批（随字段面 / 形参面收敛的既有夹具点位；MUST NOT 新增用例 / 改断言口径）

| 面 | 点位 | 改动 |
|----|------|------|
| `AgentProviderRecord` 结构体字面量 | `crates/infra/store/src/describe_test.rs:89`、`crates/infra/agent/src/sdk/db_tool_test.rs:68`、`crates/infra/store/src/query_test.rs:218`、`crates/infra/store/src/model_test.rs:3805 / 3816 / 4018`（共 6 处） | 各补 `efforts: AgentEffortTiers::default(),`；`envelope_test.rs:169` / `store_test.rs:3813` 走 `::new`（44 处调用点）零改动 |
| `save_agent_provider` 调用点 | `src/commands/agents/mod_test.rs`（13 处） | 各补第 10 实参（`AgentEffortTiers::default()` 或显式值）；既有断言语义零改动 |
| `saveAgentProvider` 前端形参 | `packages/desktop/src/lib/ipc.test.ts:332 / 335 / 443`（形参用例与两处调用）、`packages/desktop/src/lib/ipc-contract.test.ts:168`（形参名清单） | 七形参（末位 `tuning` 组）逐位对齐；`packages/desktop/src/lib/ipc.ts` 与 `ipc-contract.test.ts` 的泛化遍历零改动 |
| `EngineFacade` 字面构造 | `crates/infra/agent/src/lib.rs`（生产 6 处，见 DD / 校准 1） | 各补一行 `efforts: …` |
| `PumpPayload` / `LoopTurn` 字面构造 | `sdk/runner.rs`（生产各 1 处）+ 既有 `sdk/*_test.rs` 夹具（编译器可全量列出） | 各补 `effort` / `effort_params` 两字段 |
| provider 夹具补 `efforts` | `packages/desktop/src/views/agents/**` 测试、`crates/infra/agent/src/compose_test.rs` 等以记录字面量 / 断言对象构造的夹具 | 机械补字段（`efforts: { high: '', low: '' }`） |
| 类型名快照 | `src/bindings/mod_test.rs:249` | 类型名清单随再生成补 `AgentEffortTiers` / `ModelCatalog` / `CatalogProtocol` / `CatalogModel` |

> **闭合批的时点不变量**：任一处字段面 / 形参面变化，其**全部**构造点在**同一阶段批次**内收敛；阶段末「工作区可编译（编译器零错误）」是阶段划分的不变量——可编译性与整链门禁的最终确认归桌面端 static-check 相位。末两行的性质是「既有测试的期望值随再生成随动」：不新增用例、不改断言语义。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `AgentProviderRecord::with_efforts` | `crates/infra/store/src/model.rs` | 新增 | `pub fn with_efforts(self, efforts: AgentEffortTiers) -> Self` | 构造器链式缝（`new` 签名不动的唯一实现路径，与 `with_web_search` 同型）；保存期无条件接受，无协议门控 |
| `AgentProviderRecord::new` | 同上 | 修改（**签名不变**） | `pub fn new(name: String, protocol: AgentProviderProtocol, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>, max_tokens: Option<u64>) -> Self` | 参数面逐字不动（44 处调用点零改）；字段面补 `efforts: AgentEffortTiers::default()` |
| `effort::params` | `crates/infra/agent/src/sdk/effort.rs` | 新增（`pub(crate)`，crate 内跨模块消费者 `sdk::runner`） | `pub(crate) fn params(protocol: Protocol, effort: &str) -> Option<serde_json::Value>` | 纯函数：裁剪后空 ⇒ `None`；`Openai → {"reasoning_effort": v}`；`Anthropic → {"output_config": {"effort": v}}`；零值域校验、零 IO。键名只由本模块三常量持有 |
| `catalog` | `crates/infra/agent/src/model_catalog.rs` | 新增（经 `lib.rs` 重导出） | `pub fn catalog() -> ModelCatalog` | 纯静态表读取，零 IO、零参数、零凭据 |
| `EngineFacade::with_efforts` | `crates/infra/agent/src/lib.rs` | 新增 | `pub fn with_efforts(self, efforts: store::AgentEffortTiers) -> Self` | 门面旁路第四项（与 `with_web_search` 同型）；语义为「**未 shaping 的 provider 原值对**」，门面零协议分支 |
| `SdkRunner::with_efforts` | `crates/infra/agent/src/sdk/runner.rs` | 新增 | `pub fn with_efforts(self, efforts: store::AgentEffortTiers) -> Self` | 旁路 builder（`new` 签名不动）；`..self` 形态 |
| `save_agent_provider` | `src/commands/agents/mod.rs` | 修改 | `pub fn save_agent_provider(stores: State<'_, WorkspaceStores>, id: Option<i64>, name: String, protocol: AgentProviderProtocol, base_url: String, api_key: String, models: AgentModelTiers, tuning: AgentProviderTuning) -> Result<AgentProviderRecord, String>` | 末位收四项可空调参组（参数位上限所迫，DD15 / 校准 9）；`efforts` 两档各自 `trim()`（空串 = 留空），**零值域校验、零协议门控**；既有校验逐字保留；错误串结构上不含 api_key |
| `list_model_catalog` | 同上 | 新增 | `pub fn list_model_catalog() -> Result<ModelCatalog, String>` | 零参数、零 State、零 IO 薄包装（恒 `Ok`）；`all_commands!` 已登记 |
| `useModelCatalog` | `packages/desktop/src/views/agents/hooks/use-model-catalog.ts` | 新增 | `export function useModelCatalog(): ModelCatalogState` | 挂载一次取数、无轮询；失败 `catalog = null`（降级为无建议，不置 error、不 toast） |
| `StandardCombobox` | `packages/desktop/src/components/standard/combobox.tsx` | 新增 | `export function StandardCombobox(props: StandardComboboxProps): React.JSX.Element` | 可选可填单值控件；任意键入逐键回调原文；选中条目回填文本；清空回调空串；`items` 空退化纯输入框 |
| `Combobox.Input` 等原语包装 | `packages/desktop/src/components/ui/combobox.tsx` | 新增 | `export { Combobox, ComboboxContent, ComboboxInput, ComboboxItem }` | 原语层（零业务语义），只供标准层消费 |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `AgentEffortTiers` | `crates/infra/store/src/model.rs` | 新增 | `{ high: String, low: String }`（`serde` camelCase、`Default`、`specta::Type`、`Serialize/Deserialize`）；**空串 = 留空 = 不发**；纯嵌套 struct，不落独立模型 |
| `AgentProviderTuning` | `src-tauri/src/commands/agents/mod.rs` | 新增（命令层 DTO） | `{ context_length: Option<u64>, max_tokens: Option<u64>, web_search: bool, efforts: AgentEffortTiers }`（`serde` camelCase → `{contextLength, maxTokens, webSearch, efforts}`，`Deserialize + Serialize + specta::Type`）——**IPC 入参形态，不是记录形态**：四项可空调参收一组只因参数位上限（校准 9 / DD15），记录侧仍是 `efforts` sibling 列 + 三个既有列 |
| `AgentProviderRecordV4` | 同上 | 新增（= 现记录改名） | `pub(crate)` 升级链中转形态（`version = 4`，`from = AgentProviderRecordV3`）；无 `efforts` 列；去 `#[native_db]`；`Debug` 为 derive（明文可见，与 V1–V3 同型，只作降级 / 解码中转） |
| `AgentProviderRecord` | 同上 | 修改 | 版本头 `version = 4` → `5`（`id` 仍 5，`from = AgentProviderRecordV4`）；字段面末尾增 `#[serde(default)] pub efforts: AgentEffortTiers`；手写遮蔽 `Debug` 补 `efforts` 位 |
| `ModelCatalog` | `crates/infra/agent/src/model_catalog.rs` | 新增 | `{ protocols: Vec<CatalogProtocol> }`（`specta::Type` + `Serialize`，camelCase） |
| `CatalogProtocol` | 同上 | 新增 | `{ protocol: store::AgentProviderProtocol, efforts: Vec<String>, models: Vec<CatalogModel> }`；`efforts` 为协议级**建议**值域 |
| `CatalogModel` | 同上 | 新增 | `{ value: String, context_length: Option<u64>, max_tokens: Option<u64>, supports_thinking: bool, efforts: Option<Vec<String>> }`；`efforts = Some(list)`（含 `Some([])`）覆盖协议级，`None` = 继承 |
| `LoopTurn` | `crates/infra/agent/src/sdk/loop.rs` | 修改（`pub(crate)`） | 增 `pub effort_params: Option<serde_json::Value>`（已 shaping 的 wire 载荷） |
| `PumpPayload` | `crates/infra/agent/src/sdk/runner.rs` | 修改（私有） | 增 `effort: String`（日志原值）与 `effort_params: Option<serde_json::Value>`（wire 载荷）——同源双字段（DD4） |
| `ResolvedEngine` | `crates/infra/agent/src/compose.rs` | 修改（私有） | 增 `efforts: store::AgentEffortTiers`（provider 原值对，零门控） |
| `ProviderFormState` | `packages/desktop/src/views/agents/components/provider-panel.tsx` | 修改 | 增 `effortsHigh: string` / `effortsLow: string` |
| `ProviderComboKey` | 同上 | 新增 | `type ProviderComboKey = 'high' \| 'medium' \| 'low' \| 'effortsHigh' \| 'effortsLow'`（组合框键集合） |
| `ProviderTextField` | 同上 | 修改 | `Exclude<keyof ProviderFormState, 'id' \| 'protocol' \| 'webSearch' \| ProviderComboKey>`（只剩五个文本键） |
| `CatalogPrefillField`（内联字面量类型） | 同上 | 新增 | `'contextLength' \| 'maxTokens'`——预填留痕的键域（DD12） |
| `AgentProviderSaveInput` | `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | 修改 | 增 `efforts: AgentEffortTiers`（末位，与命令形参逐位对齐） |
| `ModelCatalogState` | `packages/desktop/src/views/agents/hooks/use-model-catalog.ts` | 新增（**模块内，不导出**） | `{ catalog: ModelCatalog \| null; loading: boolean }`（hook 的具名返回类型；不导出是 knip 纪律——同文件使用的类型若 `export` 会被记为 unused export，先例见 `StandardSelectProps`） |
| `StandardComboboxProps` | `packages/desktop/src/components/standard/combobox.tsx` | 新增（**模块内，不导出**） | `{ value: string; onChange: (value: string) => void; items: string[]; placeholder?: string; disabled?: boolean; 'data-testid'?: string; id?: string; className?: string }`——沿 `StandardSelectProps` 先例（props 接口不导出，组件是唯一出口） |

<!-- 配置：本变更零配置项——`efforts` 是记录字段而非配置键，`model_catalog` 是编译期静态表
     （无配置项、无环境变量、无 `.dev-team/config.json` 新键），`context_length` / `max_tokens` /
     `web_search` 的既有配置面零改动。故本子表整段省略。 -->

---

## 数据模型

### provider 记录（全局库 user 维度，`native_model(id = 5)`）

| 版本 | 形态 | 说明 |
|------|------|------|
| v3 | 历史中转（既有） | 有 `protocol` / `max_tokens`，无 `web_search` |
| v4 | **本次改名** | v3 + `web_search`（现记录原样改名，字段面逐字不动） |
| v5 | **本次新增** | v4 + `efforts: AgentEffortTiers`（尾部追加） |

升级 / 降级映射：

| 方向 | 变换 |
|------|------|
| v4 → v5（升级） | 全字段搬运 + `efforts: AgentEffortTiers::default()`（两档空串）——存量行读出即「留空 = 不发」，行为零变化、无迁移脚本 |
| v5 → v4（降级） | 全字段搬运，`efforts` 丢弃（只保升级语义真实性；降级形态不作数据承诺，与 v2/v3/v4 的既有同名约定一致） |

字段语义（v5）：

| 字段 | 线名（JSON） | 类型 | 落库 | 语义 |
|------|--------------|------|------|------|
| `efforts.high` | `efforts.high` | `String` | bincode 尾部嵌套 | `ModelLevel::High` 档的思考强度；空串 = 留空 = 不发 |
| `efforts.low` | `efforts.low` | `String` | 同上 | `ModelLevel::Low` 档的思考强度；空串 = 留空 = 不发 |

关系：`efforts` 与 `models` 是**兄弟列**（同一记录、无引用、无级联）；`AgentInstanceRecord.provider_id` 的外键语义零改动（记录 id 稳定，升级不重排主键）。

### 模型目录（非持久化）

静态编译期数据，**不落库、不建表、无第二条取数路径**：

```text
ModelCatalog
 └─ protocols: [CatalogProtocol]
      ├─ protocol: openai | anthropic          ← 复用 store 线值枚举
      ├─ efforts:  [String]                    ← 协议级建议值域（非校验值域）
      └─ models:   [CatalogModel]
            ├─ value: String                   ← 完整模型原文（可含 [1m] 后缀）
            ├─ context_length: u64 | null       ← 建议预填（缺席 = null = 不预填）
            ├─ max_tokens: u64 | null           ← 建议预填（openai 协议前端不预填）
            ├─ supports_thinking: bool          ← 只影响建议面
            └─ efforts: [String] | null         ← 模型级覆盖（Some([]) = 显式无建议）
```

首版内容口径（名单由实现期按 vendor 当前文档手填，非本设计的契约）：`openai` 协议侧收 DeepSeek 系常用条目（含 `[1m]` 变体与 `supportsThinking = false` 的条目），`efforts` 建议 `low / high / max`；`anthropic` 协议侧收 Claude 系常用条目（含 `[1m]` 变体），`efforts` 建议官方五值 `low / medium / high / xhigh / max`。GPT-6 系不进 `openai` 建议（rig `refuse_tools_while_reasoning`，proposal R7）。五条不变量（唯一 / 非空 / 零凭据 / `[1m]` ⇒ `contextLength = 1_000_000` / `supportsThinking = false` ⇒ `efforts = Some([])`）由后端测试相位锚定。

---

## 路由 / API 设计（IPC 命令面）

Tauri IPC（进程内、`all_commands!` 白名单登记、无鉴权；错误语义统一为 `Result<T, String>` → 前端 reject）：

| 命令（线名） | 前端函数 | 入参 | 出参 | 说明 |
|--------------|----------|------|------|------|
| `list_model_catalog` | `listModelCatalog()` | 无 | `ModelCatalog`（`protocols[].{protocol, efforts, models[]}`） | 零 IO 静态表读取；失败面结构性不存在（恒 `Ok`）。`bindings.ts` 出 `__TAURI_INVOKE("list_model_catalog", {})` |
| `save_agent_provider` | `saveAgentProvider(id, name, protocol, baseUrl, apiKey, models, tuning)` | 七形参（第 7 位 `AgentProviderTuning` 组：`{contextLength, maxTokens, webSearch, efforts}`） | `AgentProviderRecord`（含 `efforts`） | 保存期只 `trim`；`efforts` 空串 = 留空；零值域校验、零协议门控 |
| `list_agent_providers` / `delete_agent_provider` | 既有 | 既有 | 既有（清单自动带出 `efforts`） | 签名零改动 |

---

## 依赖

- **运行时依赖零新增**：`specta`（`crates/infra/agent` 已在 `[dependencies]`）、`serde_json`（同日）、`store`（同上，`AgentProviderProtocol` / `AgentEffortTiers` 的来源）、`@base-ui/react`（前端已在 `dependencies`，`combobox` 是其包内子路径）。`@base-ui/react` 版本 `^1.8.0` 已在锁内，零版本变化。
- **构建 / 测试依赖零新增**：目录静态表与 `efforts` shaping 都是纯 Rust 代码，无需 fixture 库；组合框复用既有 `@testing-library` 面（测试相位）。`archi-api-gen` / `export-bindings` 为既有工具，无版本变化。
- **生成物（非依赖）**：`bindings.ts` / `bindings.zod.ts`（`bindings:export`）、`Frontend/**` + `graph/INDEX.toml`（`api:export`）。

---

## 待决问题

1. **目录首版名单（具体条目 / `[1m]` 变体 / 各条目的标称窗长与最大输出）**：spec 明文「形状与不变量是本 spec 的对象，名单不是」，故由实现期按 vendor 当前文档手填。不阻塞设计：名单变更只影响建议准确度（AC-3 / AC-8 已锁死它不参与校验与解析）。
2. **`Combobox` 在极端路径（Escape 关闭 / 快速连续键入）下的输入行为**：已按 DD10 定下兜底路线（`value` 不受控 ⇒ 底座无从回写；`single` 分支关闭不清输入），但底座内部还有 `closeQuery` / `handleInterruptedReopen` 两条路径未逐行核完。判据放在测试相位的组合框契约分支（自由输入后 `data-value` 仍为键入原文），若发现回写则在标准层加「重开时以受控 `inputValue` 压回」的一行收口——不改本契约的对外语义。

---

## 验收标准落点

| AC | 落点（实现面） |
|----|----------------|
| AC-1 存储演进一条链 | `model.rs` 的 v4 改名 + v5 新记录 + 双向 `From`（DD1 / 数据模型表）；`native_model_version() == 5`、`id` 仍 5 |
| AC-2 构造器面不动 + 遮蔽 `Debug` | `new` 签名逐字不动（`efforts` 走 `Default`）+ `with_efforts`；手写 `Debug` 迁至 v5 并补 `efforts` 位（校准 3） |
| AC-3 保存期只 `trim` | `save_agent_provider` 的两档 `trim()`；零值域分支、零协议门控（既有两条校验逐字保留） |
| AC-4 请求侧零痕迹 | `effort::params` 空值返回 `None`（DD3）→ `LoopTurn.effort_params = None` → `additional_params` 保持 `None`（`loop.rs` 逐字路径） |
| AC-5 请求侧形状与 `web_search` 共存 | `effort.rs` 两臂（`Map` 常量键）+ `loop.rs` 先 `additional_params` 后 `provider_tool`（rig 按键合并，起点核实已核 `request.rs:842-845`） |
| AC-6 档位分发单一 + 日志 | `open_session` 三元 `match` 为唯一分发点（`loop` / 门面 / 组合根零协议判定与零键名字面量）；日志 `effort=…` / `-` 与 wire 同源（DD4） |
| AC-7 目录不变量 + 命令面 | `model_catalog.rs` 三 DTO 与静态表；`list_model_catalog` 零参零 IO；`all_commands!` 登记；五条不变量由测试相位锚定 |
| AC-8 目录不进解析 / 不进校验 | `resolve_agent_engine` / `build_model` / `save_agent_provider` 的取值链与校验零改动（目录的唯一消费者是命令面与前端） |
| AC-9 前端控件形态 | `StandardCombobox`（DD10）+ `provider-panel` 五处接线（DD11）；colocated 测试归测试相位 |
| AC-10 预填四纪律 | `prefillOf` + `prefilled` 状态（DD12）+ `toFormState` 恒空集 + `[1m]` 条目 `contextLength = 1_000_000`（目录不变量 ④）+ openai 不预填 `maxTokens` |
| AC-11 文案中文化且键名不变 | DD13 的确切文案；存储键 / IPC 形参 / bindings 字段名零改动（`grep` 可复核） |
| AC-12 目录取数失败降级 | DD8（`catalog = null`、无 error、无轮询）；提交载荷恒携 `efforts`（空串表达） |
| AC-13 出线与契约 | `bindings:export` 连跑零 diff + `api:check` 零 diff（含新元素生成物）+ `cargo clippy` 零告警 + `vp check` / `knip` 全绿 + `package.json` `0.6.55` |
