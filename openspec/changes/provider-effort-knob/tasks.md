# 任务: provider-effort-knob

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 19 项（5 新增文件 + 13 修改面 + 生成物）+ 设计补充的 `crates/infra/store/src/lib.rs`（`AgentEffortTiers` 出口，见 design 校准 2）+ 机械改动（6 处 `AgentProviderRecord` 字面构造、13 处 `save_agent_provider` 调用、`EngineFacade` 6 处字面构造、`PumpPayload` / `LoopTurn` 夹具、前端形参对齐，见 design「编译闭合批」）。
>
> **本相位不改测试文件的用例与断言，也不执行测试回归**：proposal「测试文件」表逐行的新增 / 改造用例归 test-design / test-gen / test-execution 相位。本相位对测试文件的唯一动作是**让工作区随字段面 / 形参面变化编译通过**（补字段、补实参、补形参），MUST NOT 新增用例、MUST NOT 改断言口径。
>
> **依赖序（八阶段）**：
>
> - **阶段一**（store 记录演进：`AgentEffortTiers` + v4 改名 + v5 + 双向 `From` + `Debug` 迁移 + 出口）零 API 面变化（`new` 签名不动），是后续一切「读到 `efforts`」的前提。
> - **阶段二**（引擎：`sdk/effort.rs` + 穿透链 `compose` / `lib` / `runner` / `loop`）依赖阶段一（`store::AgentEffortTiers` 在场）。
> - **阶段三**（命令面 `save_agent_provider` 末位收 `AgentProviderTuning` 组 + `efforts` 只 `trim`）依赖阶段一。至此**半 A 完成**：手填 effort 即生效（前端仍是文本框）。
> - **阶段四**（目录静态表 `model_catalog.rs` + crate 导出）依赖阶段一（复用 `store::AgentProviderProtocol`）。
> - **阶段五**（命令面 `list_model_catalog` + `all_commands!` + `bindings:export`）依赖阶段三与阶段四（命令签名与目录 DTO 同时在场才能一次生成 bindings）。
> - **阶段六**（前端原语层 + 标准层组合框）依赖零后端面（纯组件）。
> - **阶段七**（目录取数 hook + 表单接线：组合框 ×5 / 建议联动 / 预填 / 文案 / 清单行）依赖阶段五（生成物有 `listModelCatalog` 与 `efforts` 第十形参）与阶段六（`StandardCombobox` 在场）。
> - **阶段八**（archi 模型 + 生成物 + 版本 + 边界自查 + 交接）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ⓪ **三条不变量**（design / spec 的口径，贯穿）：①协议判定单点 = `sdk/effort.rs`，`loop` / 门面 / 组合根一律持「已 shaping 的载荷」且**零键名字面量**（`grep -rn "reasoning_effort\|output_config"` 在 `loop.rs` / `runner.rs` / `lib.rs` / `compose.rs` 必须为 0）；②留空零痕迹 = 两档皆空 ⇒ `effort_params = None` ⇒ `additional_params` 保持 `None`；③会话内 effort 恒定 = 只在 `open_session` 落值，`loop` 内零档位判定。
> ① **逐字不动面**（`git diff` 出现即越界）：`crates/core/**`（`AgentEvent` 词表 / `SessionInjections` / `AgentStartError` 变体集 / 16 份相位提示词 md）；`crates/infra/store/src/model.rs` 的 `SessionConfigSnapshot` / `SessionRecord`（无 v2→v3）；`crates/infra/agent/src/sdk/{config,compact,normalize,context,resume,reasoning,policy,tools,model_spec,web_search}.rs`（`config.rs` 的 `EngineConfig` 字段面零增、`compact.rs` 的 `additional_params: None` 逐字保留——该文件出现 diff 即偏离决策）；`AgentModelTiers` 三档结构；`src/commands/agents/mod.rs` 的既有两条保存期校验（`parse_model_spec` / `MIN_OUTPUT_TOKENS`）；`packages/desktop/src/lib/{ipc.ts,ipc-contract.test.ts}`（Proxy 与泛化遍历零改动）；`packages/desktop/src/components/standard/{dailog,drawer,select,pagination}.tsx` 与 `hooks/use-open.ts`。
> ② **目录零消费**：`model_catalog` MUST NOT 被 `compose` / `runner` / `save_agent_provider` 任何校验或取值链引用（`grep -rn "model_catalog\|catalog()" crates/infra/agent/src/{compose,lib}.rs crates/infra/agent/src/sdk src/commands/agents/mod.rs` 只允许 `lib.rs` 的 `mod` / `pub use` 与 `mod.rs` 的命令体两处）。
> ③ **值域零校验**：`efforts` 在保存期与引擎侧都 MUST NOT 出现枚举 / 白名单 / 成员判定（`grep -rn "efforts"` 复核无 `match` 值域分支）。
> ④ **生成物不手改**：`packages/desktop/src/types/generated/**`、`.dev-team/architecture/models/Frontend/**`、`graph/INDEX.toml`、`*.schema.json`、`*.i.ts` 一律只经 `bindings:export` / `api:export` 产出。
> ⑤ **门禁分工**：`pnpm -C packages/desktop run check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**由桌面端 static-check / test-execution 相位执行，本相位不运行、不重复执行。自查项只收源码级静态判据（`grep` 零命中、读码复核、`git diff` 边界比对）；编译与整链门一律写成「交接 …」条目。
> ⑥ **测试声明与 sidecar 同批**：本相位**不**在 `crates/infra/agent/src/sdk/mod.rs` 加 `#[cfg(test)] mod effort_test;`、不在 `crates/infra/agent/src/lib.rs` 加 `#[cfg(test)] mod model_catalog_test;`（先加声明会让工作区因缺文件编译失败）；声明由 test-gen 创建 sidecar 时同批落地。

## 阶段一：store 记录演进（`AgentEffortTiers` + v4 改名 + v5 + 双向 `From` + 出口）

> 依赖无。本阶段是关键风险面（proposal R1：bincode 升级链写错即让存量 provider 读不出 / 管理页空白），故单列一段、逐条可复核。

- [x] `crates/infra/store/src/model.rs`：在 `AgentModelTiers` 之后新增两档 effort 类型（design DD2）
  ```rust
  /// provider 两档思考强度（effort）档位（agent 管理域，纯嵌套 struct 不落独立
  /// 模型）：与 [`AgentModelTiers`] 成 **sibling 列**——MUST NOT 并入三档模型
  /// struct（该 struct 未版本化，往里加字段即改 bincode 布局却无 `from` 链可依）。
  /// 语义为「同一模型在不同档位想多深」：`high` 供 `ModelLevel::High`、`low` 供
  /// `ModelLevel::Low`；**空串 = 留空 = 不发**（与三档模型名用空串表未配置同型）。
  /// 值域不封闭：保存期只 `trim()`，自定义值原样透传。
  #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
  #[serde(rename_all = "camelCase")]
  pub struct AgentEffortTiers {
      /// 深相位档（proposal / 设计 / 检视 / 验收）的思考强度
      pub high: String,
      /// 快相位档（实现 / 测试生成）的思考强度
      pub low: String,
  }
  ```
- [x] `crates/infra/store/src/model.rs`：现 `AgentProviderRecord`（`version = 4`）整体改名为 `AgentProviderRecordV4`，并只改四处：`pub struct` → `pub(crate) struct`、摘掉 `#[native_db]`（连带摘掉其专属助手属性 `#[primary_key]`——脱离 derive 即未知属性，V1–V3 中转形态同形）、`impl Debug` 手写块回退为 `#[derive(Debug)]`（field 面、`#[native_model(id = 5, version = 4, from = AgentProviderRecordV3)]` 头、`#[serde]` 属性逐字不动）；doc 改述为「`AgentProviderRecord` 的 version 4 历史形态（仅作 native_model 升级链的解码目标，不注册进库模型组、不出公共查询面）：无 `efforts` 列」
- [x] `crates/infra/store/src/model.rs`：把原 `From<AgentProviderRecordV3> for AgentProviderRecord` 与 `From<AgentProviderRecord> for AgentProviderRecordV3` 两臂**改指 `AgentProviderRecordV4`**（`Self` / 目标类型名替换，字段搬运逐字不动）
- [x] `crates/infra/store/src/model.rs`：新增 v5 当前记录（design DD1 / 数据模型表）——与 v4 同一字段面 + 末尾 `efforts`，`#[native_model(id = 5, version = 5, from = AgentProviderRecordV4)]` + `#[native_db]`；`efforts` 字段 doc 写明「空串 = 留空 = 不发 / 值域不封闭 / 无协议门控 / 与 `models` 成 sibling 列」
- [x] `crates/infra/store/src/model.rs`：新增双向 `From`：`From<AgentProviderRecordV4> for AgentProviderRecord`（全字段搬运 + `efforts: AgentEffortTiers::default()`）与 `From<AgentProviderRecord> for AgentProviderRecordV4`（全字段搬运，`efforts` 丢弃）；两臂 doc 沿用既有「升级半边 / 降级半边」措辞
- [x] `crates/infra/store/src/model.rs`：`AgentProviderRecord::new` 的字面构造补 `efforts: AgentEffortTiers::default(),`（**签名逐字不动**）；doc 补一句「`efforts` 缺省两档空串，开走 [`Self::with_efforts`]——现有参数面不动的唯一实现路径」
- [x] `crates/infra/store/src/model.rs`：新增 `with_efforts`（与 `with_web_search` 并列、`..self` 形态）：
  ```rust
  /// 两档思考强度（构造器链式缝，与 `with_web_search` 同型）：保存期无条件接受
  /// 两档原值（零值域校验、零协议门控），wire 形状由引擎侧 shaping 单点吸收。
  pub fn with_efforts(self, efforts: AgentEffortTiers) -> Self {
      Self { efforts, ..self }
  }
  ```
- [x] `crates/infra/store/src/model.rs`：手写遮蔽 `Debug` **迁到新 v5 记录**（字段面逐位对齐 + `web_search` 之后补 `.field("efforts", &self.efforts)`；`api_key` 位仍 `mask_api_key`；doc 保留「字段面与 derive 形态逐位对齐」口径）
- [x] `crates/infra/store/src/model.rs`：`AgentModelTiers` 的 doc 内「（effort 进 run 参数与档位选择器为后续迭代，本期只存不选）」改写为「（深度轴由 sibling 列 `efforts` 承担，见 provider-effort-knob；per-run effort 参数与前端档位选择器仍为后续迭代）」——**结构零改**
- [x] `crates/infra/store/src/lib.rs`：`pub use model::{…}` 的名单在 `AgentEngineKind` **之前**插入 `AgentEffortTiers`（字母序：`AgentEffortTiers` < `AgentEngineKind`）
- [x] 编译闭合（本阶段，机械补位、不新增用例）：`crates/infra/store/src/describe_test.rs`、`crates/infra/agent/src/sdk/db_tool_test.rs`、`crates/infra/store/src/query_test.rs`、`crates/infra/store/src/model_test.rs`（三处：`3805` / `3816` / `4018`）共 6 处 `AgentProviderRecord { … }` 字面构造各补 `efforts: AgentEffortTiers::default(),`（`use` 面按需补 `AgentEffortTiers`）
- [x] 自查 阶段一（源码级静态判据）：
  - `grep -c "version = 5" crates/infra/store/src/model.rs` 命中 v5 记录头；`grep -n "native_model(id = 5" crates/infra/store/src/model.rs` 逐级呈现 `version = 1/2/3/4/5` 五条链（id 全为 5）
  - `grep -n "pub(crate) struct AgentProviderRecordV4" -A 4 crates/infra/store/src/model.rs` 上方**无** `#[native_db]`、下方**无** `#[native_db]`
  - `grep -c "impl std::fmt::Debug for AgentProviderRecord" crates/infra/store/src/model.rs` 为 **1**（只有 v5 一份手写），`grep -n "derive(Debug" -A 3 crates/infra/store/src/model.rs` 可见 `AgentProviderRecordV4` 走 derive
  - `grep -n "fn with_efforts" crates/infra/store/src/model.rs` 为 1；`grep -n "pub fn new(" -A 9 crates/infra/store/src/model.rs` 参数面与改动前逐字一致（7 参）
  - `grep -n "efforts" crates/infra/store/src/model.rs` 的 `impl Debug` 段落含 `.field("efforts"`；`grep -c "AgentEffortTiers" crates/infra/store/src/lib.rs` 为 1
  - `git diff -- crates/infra/store/src/model.rs` 读 diff 复核：`AgentProviderRecordV4` 块内除类型名 / 可见性 / `#[native_db]` / `Debug` 四处外**零改动**（字段与 `#[serde]` 属性逐字相同）
  - `git diff -- crates/infra/store/src/model.rs` 的 `SessionConfigSnapshot` / `SessionRecord` / `AgentInstanceRecord` 段为零 hunk
- [x] 交接 阶段一（门禁与测试回归，本相位不执行）：v4 载荷字节经版本机制解码 / 四级链逐字段一致 / 降级丢列 / 版本头锚点 / JSON 缺键读空 / 遮蔽 `Debug` 补位等用例由桌面端 test-design / test-gen / test-execution 相位承接；编译与 `server:check` 归 static-check。**交付物 = 期望登记与移交**

## 阶段二：引擎 effort shaping 与穿透（`effort.rs` + `compose` / `lib` / `runner` / `loop`）

> 依赖阶段一。本阶段末「手填 effort 即生效」（半 A 的引擎半边）：`efforts` 为空时全链行为与今天逐字相同。

- [x] 新建 `crates/infra/agent/src/sdk/effort.rs`（design DD3）：
  - 模块头 doc：三句——①effort 的协议 shaping **唯一单点**（`loop` / 门面 / 组合根一律持已 shaping 的载荷，键名字面量只在本文件）；②留空返回 `None` 而非空对象（「留空零痕迹」的类型化表达，避免往非 DeepSeek 网关塞 `reasoning_effort`）；③零值域校验（自定义值原样透传，与保存期「只 `trim`」同一条纪律）
  - 常量面（`pub(crate) const &str`）：`OPENAI_EFFORT_KEY = "reasoning_effort"` / `ANTHROPIC_OUTPUT_CONFIG_KEY = "output_config"` / `ANTHROPIC_EFFORT_KEY = "effort"`
  - `pub(crate) fn params(protocol: Protocol, effort: &str) -> Option<serde_json::Value>`：`let value = effort.trim();` 空即 `None`；否则用 `serde_json::Map` 逐键插入（`Openai` → `{OPENAI_EFFORT_KEY: value}`；`Anthropic` → `{ANTHROPIC_OUTPUT_CONFIG_KEY: {ANTHROPIC_EFFORT_KEY: value}}`），返回 `Value::Object`
- [x] `crates/infra/agent/src/sdk/mod.rs`：`pub(crate) mod effort;`（`db_tool` 与 `explore_note` 之间，字母序）；**不加** `#[cfg(test)] mod effort_test;`（约束 ⑥）
- [x] `crates/infra/agent/src/sdk/runner.rs`：
  - `SdkRunner` 增 `efforts: store::AgentEffortTiers` 字段（doc：provider 原值对、**未 shaping**；`new` 内 `AgentEffortTiers::default()`）
  - 新增 builder（`..self` 形态）：`pub fn with_efforts(self, efforts: store::AgentEffortTiers) -> Self`，doc 写明「值已由组合根原样带出（两协议皆在），runner 只在 `open_session` 做一次 shaping；`new` 签名不动」
  - `open_session` 的档位 `match` 扩三元：`ModelLevel::High => ("model_high", self.config.model_high.clone(), self.efforts.high.clone())`、`ModelLevel::Low => ("model_low", self.config.model_low.clone(), self.efforts.low.clone())`；随后 `let effort_params = crate::sdk::effort::params(self.config.protocol, &effort);`
  - `PumpPayload` 增两字段（design DD4）：`effort: String`（日志原值；doc 写「与 `effort_params` 同源于 `open_session` 的三元绑定，日志 MUST NOT 另走取值链」）与 `effort_params: Option<serde_json::Value>`（wire 载荷）；`open_session` 构造 `PumpPayload { …, effort, effort_params }` 补两位
  - 泵启动日志格式串在 `max_tokens={output}` 之后插入 ` effort={}`，实参 `if payload.effort.trim().is_empty() { "-" } else { payload.effort.as_str() }`（与 `max_tokens` 的 `-` 口径同型）
  - `LoopTurn` 铸造点补 `effort_params: payload.effort_params.clone(),`
- [x] `crates/infra/agent/src/sdk/loop.rs`：`LoopTurn` 增 `pub effort_params: Option<serde_json::Value>`（紧随 `web_search`；doc：「**已按协议 shaping** 的 wire 载荷——shaping 在 `sdk/effort.rs` 单点完成，本模块零协议判定、零键名字面量」）；`CompletionRequest` 构造的 `additional_params: None` → `additional_params: turn.effort_params.clone()`（`provider_tool(..)` 的挂载代码逐字不动）
- [x] `crates/infra/agent/src/lib.rs`：
  - `mod model_catalog;` 与 `pub use model_catalog::{catalog, CatalogModel, CatalogProtocol, ModelCatalog};` **本阶段先不落**（目录在阶段四创建，先挂声明会因缺文件编译失败）——本阶段只做门面字段面
  - `EngineFacade` 增 `efforts: store::AgentEffortTiers` 字段（doc：「**未 shaping 的 provider 原值对**（两协议原样带出）；协议 shaping 收在 runner 的 `sdk/effort.rs` 单点，门面 MUST NOT 做协议分支」）
  - 6 处逐字段字面构造各补一行 `efforts: <同值或默认>`：`new` / `with_resume_transcript` → `AgentEffortTiers::default()`；`with_context_window` / `with_max_tokens` / `with_web_search` / `with_explore_note_store` → `efforts: self.efforts`
  - 新增 `pub fn with_efforts(self, efforts: store::AgentEffortTiers) -> Self`（`..self` 形态）
  - `runner_for` 的 Sdk 臂链在 `.with_web_search(self.web_search)` 之后追加 `.with_efforts(self.efforts.clone())`
- [x] `crates/infra/agent/src/compose.rs`：
  - `ResolvedEngine` 增 `efforts: store::AgentEffortTiers`（doc：「provider 原值对原样带出，**零门控**——两协议都有 effort 语义；wire 形状差异归引擎侧 shaping 单点」）
  - `resolve_agent_engine` 的 Sdk 臂在 `max_tokens: provider.max_tokens,` 之后补 `efforts: provider.efforts,`（`web_search` 的门控行逐字不动）
  - `compose_turn` 门面链在 `.with_web_search(resolved.web_search)` 之后追加 `.with_efforts(resolved.efforts)`
- [x] 编译闭合（本阶段，机械补位）：既有 `PumpPayload` / `LoopTurn` 字面构造夹具补两字段（`effort: String::new()` / `effort_params: None`），编译器可全量列出点位
- [x] 自查 阶段二（源码级静态判据）：
  - `grep -rn "reasoning_effort\|output_config" crates/infra/agent/src --include=*.rs | grep -v "_test.rs"` 只命中 `sdk/effort.rs`（键名单点）
  - `grep -c "effort::params" crates/infra/agent/src/sdk/runner.rs` 为 **1**（唯一 shaping 调用点）；`grep -rn "effort::params" crates/infra/agent/src/sdk/{loop,compact}.rs crates/infra/agent/src/{lib,compose}.rs` 为 **0**
  - `grep -n "effort_params" crates/infra/agent/src/sdk/loop.rs` 只出现在 `LoopTurn` 字段、请求构造两处；`grep -n "web_search" -A 3 crates/infra/agent/src/sdk/loop.rs` 的 `provider_tool` 段落零改动
  - `grep -c "efforts" crates/infra/agent/src/lib.rs` ≥ 10（字段 + 6 处构造 + builder + `runner_for` + doc 行按读码复核）；`grep -n "with_efforts" crates/infra/agent/src/{lib,compose}.rs crates/infra/agent/src/sdk/runner.rs` 三文件各命中
  - `grep -n "web_search: provider.web_search" crates/infra/agent/src/compose.rs` 一行逐字未变（门控单点零 diff）
  - `git diff -- crates/infra/agent/src/sdk/{config,compact}.rs` 为空（约束 ①）
- [x] 交接 阶段二（门禁与测试回归，本相位不执行）：`effort_test`（两臂形状 / 空与空白 ⇒ `None` / 自定义值原样）、`runner_test`（`efforts` 空 ⇒ `effort_params = None`；两档映射；日志 `effort=…`）、`loop_test`（请求形状 / 留空零痕迹 / 与 `web_search` 共存）、`compose_test`（`efforts` 原样带出）由测试相位承接。**交付物 = 期望登记与移交**

## 阶段三：命令面 `save_agent_provider` 收 `AgentProviderTuning` 组（只 `trim`）

> 依赖阶段一。本阶段末**半 A 可用**：手填 effort 即生效（UI 仍是文本框，目录与组合框在后续阶段）。

> **阶段三口径已随「实现期回溯 1」调整**（proposal「实现期回溯（1 项：IPC 参数位上限）」/ design 校准 9 + DD15）：specta `SpectaFn` 只实现到 `fn(T1..T10)` 且 `State` 计入其一，四项可空调参平铺会越界（`bindings:export` E0277），故四项收成末位一个 `AgentProviderTuning` 组。

- [x] `src/commands/agents/mod.rs`：
  - `use store::{…}` 名单插入 `AgentEffortTiers`
  - 新增 IPC 入参组 `AgentProviderTuning { context_length: Option<u64>, max_tokens: Option<u64>, web_search: bool, efforts: AgentEffortTiers }`（`#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]` + `#[serde(rename_all = "camelCase")]`，落命令模块内）
  - `save_agent_provider` 末位四参（`context_length` / `max_tokens` / `web_search` / `efforts`）收成 `tuning: AgentProviderTuning`（`#[allow(clippy::too_many_arguments)]` 保留，注释改「八参为 IPC 命令面定稿签名」）
  - 函数体：既有两条校验（`tuning.max_tokens` 参与下限校验）之后、构造之前加两档裁剪（**唯一新增语义**）：
    ```rust
    // 两档各自裁剪两侧空白（空串 = 留空 = 不发）：值域零校验——自定义值必须能存，
    // 目录成员与否不参与判定（两 vendor 值域不是同一集合）
    let efforts = AgentEffortTiers {
        high: tuning.efforts.high.trim().to_owned(),
        low: tuning.efforts.low.trim().to_owned(),
    };
    ```
  - 构造链改为 `.with_web_search(tuning.web_search).with_efforts(efforts)`（`new(...)` 的 `context_length` / `max_tokens` 实参取 `tuning.*`）
  - doc 补段：`efforts` **只 `trim`**（MUST NOT 值域校验、MUST NOT 因协议拒收或清零；两协议都有 effort 语义，wire 形状由引擎侧 shaping 吸收）
- [x] 编译闭合（机械补位）：`src/commands/agents/mod_test.rs` 的 11 处真实调用点（另有 2 处注释提及不算调用）末位四实参改收 `AgentProviderTuning { context_length, max_tokens, web_search, efforts }`（两档取 `AgentEffortTiers::default()` 或显式值），既有断言语义零改动
- [x] 自查 阶段三：
  - `grep -n "pub fn save_agent_provider" -A 12 src/commands/agents/mod.rs` 八形参逐位呈现、末位为 `tuning: AgentProviderTuning`
  - `grep -c "pub struct AgentProviderTuning" src/commands/agents/mod.rs` 为 1，四字段齐备
  - `grep -c "trim()" src/commands/agents/mod.rs` 为 2（两档各一）；`grep -n "with_efforts\|with_web_search" src/commands/agents/mod.rs` 两处相邻
  - `grep -n "parse_model_spec\|MIN_OUTPUT_TOKENS" src/commands/agents/mod.rs` 的既有校验块零 hunk（`git diff` 复核）
  - `grep -rn "match .*efforts\|== \"low\"\|== \"high\"" src/commands/agents/mod.rs` 为 0（值域零校验）
- [x] 交接 阶段三（测试回归）：`efforts` 只 `trim` / 自定义值可存可回读 / 留空三次保存 / v4 存量行升级读空等用例由测试相位承接。**交付物 = 期望登记与移交**

## 阶段四：模型目录静态表（`model_catalog.rs` + crate 导出）

> 依赖阶段一（复用 `store::AgentProviderProtocol`）。本阶段零命令面、零前端——目录此时只有 `catalog()` 内部出口。

- [x] 新建 `crates/infra/agent/src/model_catalog.rs`（design DD5 / DD6 / DD7）：
  - 模块头 doc：三句——①目录是**建议面**（「这家协议有哪些常用模型 / 标称窗长与最大输出 / 支持哪些 effort 值」）；②目录**不参与保存期校验、不参与运行期解析**——漂移代价上限是「建议不准」，不会变成「同一窗长两套算法」；③零凭据 / 零端点（纯静态参考数据），名单过时是知情代价
  - 三 DTO（`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]` + `#[serde(rename_all = "camelCase")]`）：`ModelCatalog { protocols: Vec<CatalogProtocol> }`、`CatalogProtocol { protocol: store::AgentProviderProtocol, efforts: Vec<String>, models: Vec<CatalogModel> }`、`CatalogModel { value: String, context_length: Option<u64>, max_tokens: Option<u64>, supports_thinking: bool, efforts: Option<Vec<String>> }`（字段 doc 逐条说明「建议预填 / 显式空 = 无建议 / `[1m]` 条目的窗长必须 1M」）
  - `pub fn catalog() -> ModelCatalog`：纯字面量组装（每次新构造，不引 `OnceLock`），doc 写明「零 IO、零参数、零凭据；体量是常量级结构，构造成本低于一次序列化」
  - 静态表内容：`openai` 协议段（DeepSeek 系常用条目 + `[1m]` 变体 + 至少一条 `supports_thinking: false` 且 `efforts: Some(vec![])` 的条目；协议级 `efforts` = `low / high / max`）；`anthropic` 协议段（Claude 系常用条目 + `[1m]` 变体；协议级 `efforts` = `low / medium / high / xhigh / max`）；GPT-6 系**不收**（rig `refuse_tools_while_reasoning` 只接受 `none`，而 `none` 不在本变更值域）。名单按 vendor 当前文档手填
- [x] `crates/infra/agent/src/lib.rs`：`mod model_catalog;`（`compose` 与 `sdk` 之间）+ `pub use model_catalog::{catalog, CatalogModel, CatalogProtocol, ModelCatalog};`（与既有 `pub use` 块同址）；**不加** `#[cfg(test)] mod model_catalog_test;`（约束 ⑥）
- [x] 自查 阶段四：
  - `grep -c "1_000_000" crates/infra/agent/src/model_catalog.rs` ≥ 1，且每条 `value` 含 `[1m]` 的条目其 `context_length` 均为 `Some(1_000_000)`（读码逐条复核）
  - `grep -n "supports_thinking: false" -A 8 crates/infra/agent/src/model_catalog.rs` 每条都带 `efforts: Some(Vec::new())`（或 `Some(vec![])`）
  - `grep -n "api_key\|base_url\|https://" crates/infra/agent/src/model_catalog.rs` 为 **0**（零凭据 / 零端点）
  - `grep -c "pub fn catalog" crates/infra/agent/src/model_catalog.rs` 为 1；`grep -c "OnceLock\|static " crates/infra/agent/src/model_catalog.rs` 为 0（DD7）
  - `grep -rn "model_catalog\|model_catalog::" crates/infra/agent/src/{compose,lib}.rs crates/infra/agent/src/sdk --include=*.rs | grep -v _test` 只命中 `lib.rs` 的 `mod` / `pub use` 两行（约束 ②）
- [x] 交接 阶段四（测试回归）：五条不变量的逐条断言由测试相位承接（新建 `model_catalog_test.rs` 与 `lib.rs` 的 `#[cfg(test)] mod model_catalog_test;` 同批落地）。**交付物 = 期望登记与移交**

## 阶段五：命令面 `list_model_catalog` + 登记 + bindings 再生成

> 依赖阶段三（十形参签名）与阶段四（目录 DTO）。本阶段是「后端出线面齐备」的收口点。

- [x] `src/commands/agents/mod.rs`：新增命令（置于 `delete_agent_provider` 之后）：
  ```rust
  /// 模型目录（静态建议表）：零参数、零 IO、零状态薄包装（`Result` 仅为与同轨
  /// 模板一致，恒 `Ok`）。目录**不参与保存期校验、不参与运行期解析**——它只为
  /// 「这家协议有哪些常用模型 / 可填哪些 effort 建议值」提供参考面。
  #[tauri::command]
  #[specta::specta]
  pub fn list_model_catalog() -> Result<ModelCatalog, String> {
      Ok(catalog())
  }
  ```
  并补 `use agent_runtime::{catalog, ModelCatalog};`
- [x] `src/commands/mod.rs`：`all_commands!` 的 agents 轨道末行（`set_default_agent_instance` 之后）追加 `$crate::commands::agents::list_model_catalog,`
- [x] `packages/desktop/package.json`：`version` `0.6.54` → `0.6.55`（版本纪律；`tauri.conf.json` 经 `"../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行不动）
- [x] 生成物（非手改）：`pnpm -C packages/desktop run bindings:export` 产出 `src/types/generated/{bindings.ts,bindings.zod.ts}` 更新（`AgentEffortTiers` / provider `efforts` 键 / `AgentProviderTuning` 组（`saveAgentProvider` 末位形参）/ `ModelCatalog` + `CatalogProtocol` + `CatalogModel` / `listModelCatalog`）
- [x] 自查 阶段五：
  - `grep -c "list_model_catalog" src/commands/mod.rs` 为 1；`grep -c "pub fn list_model_catalog" src/commands/agents/mod.rs` 为 1
  - `grep -n "listModelCatalog\|AgentEffortTiers\|ModelCatalog" packages/desktop/src/types/generated/bindings.ts` 各命中；`grep -n "efforts" packages/desktop/src/types/generated/bindings.ts` 同时命中 `AgentProviderRecord` 与 `saveAgentProvider` 形参
  - `git diff --stat -- packages/desktop/src/types/generated` 只含这两（三）个生成文件；`git status --short` 无手改痕迹（读 diff 复核：无人工排版痕迹）
  - `git diff -- packages/desktop/src-tauri/tauri.conf.json` 为空；`git diff -- packages/desktop/src-tauri/Cargo.toml` 为空
- [x] 交接 阶段五（门禁）：`bindings:export` 连跑两次零 diff 与 `api:check` 零 diff 归桌面端 static-check 相位；`src/bindings/mod_test.rs` 的类型名快照与 `src/lib/ipc*.test.ts` 的形参断言随再生成随动归测试相位（本相位只在编译不可通过时补夹具字段，不改断言语义）。**交付物 = 期望登记与移交**

## 阶段六：标准组合框（原语层 + 标准层）

> 依赖无后端面。两层各自单文件，先底座后薄壳。

- [x] 新建 `packages/desktop/src/components/ui/combobox.tsx`（design DD10；与 `ui/select.tsx` 同层同风格）：
  - `import { Combobox as ComboboxPrimitive } from "@base-ui/react/combobox";`、`cn`、图标
  - 导出四件：`Combobox`（= `ComboboxPrimitive.Root`）、`ComboboxInput`（带 `data-slot="combobox-input"` 与输入框样式，`className` 合并）、`ComboboxContent`（`Portal` + `Positioner`（`side="bottom"` / `sideOffset=4` / `align="start"`）+ `Popup` + `List`，样式沿 `select-content` 的 popover 口径）、`ComboboxItem`（`data-slot="combobox-item"`，`ItemText` + 高亮态样式）
  - doc：只组装原语与样式，**零业务语义**（无建议来源、无留空策略）；产品视图 MUST NOT 直引本文件
- [x] 新建 `packages/desktop/src/components/standard/combobox.tsx`（spec Module Contract）：
  - `interface StandardComboboxProps { value: string; onChange: (value: string) => void; items: string[]; placeholder?: string; disabled?: boolean; 'data-testid'?: string; id?: string; className?: string }`（**不导出**——沿 `StandardSelectProps` 先例，避免 knip 把只在同文件使用的 props 类型记为 unused export）
  - 实现：`<Combobox inputValue={value} onInputValueChange={(next) => onChange(next)} disabled={disabled}>` + `<ComboboxInput id data-testid data-value={value} className placeholder />` + `items.length > 0 && <ComboboxContent>{items.map((item) => <ComboboxItem key={item} value={item}>{item}</ComboboxItem>)}</ComboboxContent>`
  - **不读、不控制**选中值（DD10）；`placeholder` 缺省 `'可输入或选择'`；`data-value` 恒在场（空串即留空，合法状态）
  - doc：值语义（空串 = 留空，与 `StandardSelect` 的 `null` 表未选择分开）、建议不构成约束、`items` 空即退化纯输入框、弹层开合由底座管理（本层零第二份开关状态）
- [x] 自查 阶段六：
  - `grep -n "value=" packages/desktop/src/components/standard/combobox.tsx` 无受控 `value={…}`（只有 `inputValue` / `data-value`）；`grep -c "useState" packages/desktop/src/components/standard/combobox.tsx` 为 **0**（零第二份开关状态）
  - `grep -n "items.length" packages/desktop/src/components/standard/combobox.tsx` 命中（空清单不渲染弹层）
  - `grep -rn "components/ui/combobox" packages/desktop/src/views --include=*.tsx` 为 **0**（产品视图不直引原语层）
  - `git diff -- packages/desktop/src/components/standard/{dailog,drawer,select,pagination}.tsx packages/desktop/src/components/standard/hooks/use-open.ts` 为空
- [x] 交接 阶段六（测试回归）：`components/standard/combobox.test.tsx` 的五个契约分支（自由输入 / 选中回填 / 清空即留空 / 空清单退化 / disabled）由测试相位承接（colocated、role / 文本 / `data-testid` 查询、弹层 `findByRole('listbox')` 异步收敛）。**交付物 = 期望登记与移交**

## 阶段七：管理页目录面（取数 hook + 表单接线 + 预填 + 文案 + 清单行）

> 依赖阶段五（`listModelCatalog` 与 `efforts` 第十形参在生成物里）与阶段六（`StandardCombobox`）。

- [x] 新建 `packages/desktop/src/views/agents/hooks/use-model-catalog.ts`（design DD8）：
  ```ts
  interface ModelCatalogState {
    catalog: ModelCatalog | null;
    loading: boolean;
  }
  export function useModelCatalog(): ModelCatalogState
  ```
  实现要点：`useEffect` + `cancelled` 标志，挂载 `ipc.listModelCatalog()` **一次**；成功 `setCatalog(result)`；失败 `setCatalog(null)`（**不置 error、不 toast、不重试**）；无 `tick` / `refresh`。`ModelCatalogState` **不导出**（knip：同文件使用的导出类型会被记为 unused export）。doc 写明「目录是建议面：失败的正确表现是『没有建议』，不是『页面报错』」
- [x] `packages/desktop/src/views/agents/hooks/use-agent-providers.ts`：`AgentProviderSaveInput` 增 `efforts: AgentEffortTiers`（doc 补「恒携两档，空串 = 留空；值域零校验在表单侧不做、后端只 trim」）；`ipc.saveAgentProvider(...)` 末位传 `{ contextLength, maxTokens, webSearch, efforts }` 组（`AgentProviderTuning` 的线形）
- [x] `packages/desktop/src/views/agents/agents-view.tsx`：`const catalogState = useModelCatalog();` 并把 `catalog={catalogState.catalog}` 传给 `ProviderPanel`（doc 补「目录取数收口在视图根，面板保持无取数职责」）
- [x] `packages/desktop/src/views/agents/components/provider-panel.tsx`（改动面按 design DD9 / DD11 / DD12 / DD13 展开）：
  - **状态面**：`ProviderFormState` 增 `effortsHigh` / `effortsLow: string`；`EMPTY_FORM` 与 `toFormState`（`editing.efforts.high` / `.low`，缺列兜 `''`）同步
  - **键集合**：新增 `type ProviderComboKey = 'high' | 'medium' | 'low' | 'effortsHigh' | 'effortsLow'`；`ProviderTextField` 收窄为 `Exclude<…, 'id' | 'protocol' | 'webSearch' | ProviderComboKey>`；`PROVIDER_FIELDS` 移除三条模型条目（只剩 `name` / `baseUrl` / `apiKey` / `contextLength` / `maxTokens`），`ProviderTextField` 的类型检查即「哪些键只能走 Input」的编译期锚
  - **组合框字段表**：`PROVIDER_COMBO_FIELDS: { id: string; label: string; key: ProviderComboKey; kind: 'model' | 'effort' }[]` 五项——`provider-model-high/medium/low`（`kind: 'model'`，label 与 placeholder 沿用现值）、`provider-effort-high/low`（`kind: 'effort'`，placeholder「留空 = 用 provider 缺省」）
  - **建议联动（五个模块私有纯函数，DD9）**：`protocolCatalog(catalog, protocol)` / `modelSuggestions(catalog, protocol)` / `catalogEntry(catalog, protocol, modelValue)`（按 `value.trim()` 匹配）/ `effortSuggestions(catalog, protocol, modelValue)`（条目 `efforts` 为 `Some` 即覆盖、`None` 回落协议级；目录缺席 → `[]`）/ `prefillOf(entry, protocol)`（返回 `{ contextLength?: string; maxTokens?: string }`；`openai` 不返回 `maxTokens`；元数据缺席即不含该键）
  - **预填四纪律（DD12）**：`const [prefilled, setPrefilled] = useState<ReadonlySet<'contextLength' | 'maxTokens'>>(new Set())`；仅在「用户选中模型条目」回调里写入 `prefillOf` 命中且非空的字段并打标记；`onField` 对 `contextLength` / `maxTokens` 撤标记；打开编辑态恒空集（`toFormState` 不参与预填）；清空输入即空串（保存为 `None`）
  - **弱标记**：`PROVIDER_FIELDS` 渲染时对 `prefilled` 命中的字段追加 `<span data-testid={`${field.id}-catalog-mark`}>来自目录</span>`
  - **文案（DD13）**：`contextLength` 标签「上下文窗长（token）」、`maxTokens` 标签「输出上限（token）」；`maxTokens` placeholder 改「留空 = 按上下文窗长分档」；`ProviderHints` 的 `max_tokens` 提示改「输出上限：留空 = 按上下文窗长分档推导（未配置时 32000）；仅 anthropic 协议生效（openai 协议请求保持省略上限）；显式填值优先且须 ≥ 4096」；`suffixHint` 尾部改「… 档窗长已被模型名覆盖（后缀优先于上下文窗长）」——**存储键 / IPC 形参 / bindings 字段名零改动**
  - **提交载荷**：`state.save({ …, efforts: { high: form.effortsHigh, low: form.effortsLow } })`（恒携，空串表达留空）
  - **清单行**：`ProviderList` 在模型三档之后追加 effort 展示（`data-testid="provider-effort"`；单档为空即不呈该档，两档皆空则整段不渲染）
  - **doc**：`ProviderPanel` 的段落职责说明补一句「三档模型与两档 effort 为可选可填（目录建议 + 自由输入），目录条目选中时按条目元数据预填窗长 / 输出上限（带『来自目录』弱标记）」
- [x] 编译闭合（机械补位，不新增用例）：`src/views/agents/**` 既有测试夹具补 `efforts`（记录字面量）与 `catalog` prop；`src/lib/ipc.test.ts` 的形参用例改七形参（末位 `tuning` 组，位置逐位对齐）、`src/lib/ipc-contract.test.ts:168` 的形参清单改 `id / name / protocol / baseUrl / apiKey / models / tuning`
- [x] 自查 阶段七：
  - `grep -c "provider-effort-high\|provider-effort-low" packages/desktop/src/views/agents/components/provider-panel.tsx` ≥ 2；`grep -c "StandardCombobox" …/provider-panel.tsx` ≥ 2（import + 渲染）
  - `grep -n "PROVIDER_FIELDS" -A 40 …/provider-panel.tsx` 的五条字段不含 `high` / `medium` / `low`（模型三档已移出）
  - `grep -n "上下文窗长（token）\|输出上限（token）" …/provider-panel.tsx` 各命中 1；`grep -n "context_length\|max_tokens" …/provider-panel.tsx` 只允许出现在 doc / 注释与 `data-testid`/键名（`provider-context-length` / `provider-max-tokens`）与 `parse*` 函数名层——**文案字符串内零命中**（读码复核）
  - `grep -rn "setInterval\|setTimeout" packages/desktop/src/views/agents/hooks/use-model-catalog.ts` 为 0（无轮询）；`grep -c "toast" …/use-model-catalog.ts` 为 0（失败不 toast）
  - `grep -n "prefilled" …/provider-panel.tsx` 覆盖 state 声明、选中写入、`onField` 撤标记、渲染判定四处；`grep -n "catalog-mark" …/provider-panel.tsx` 命中 1
  - `grep -rn "reasoning_effort\|output_config" packages/desktop/src` 为 0（前端零 wire 知识）
  - `git diff -- packages/desktop/src/lib/{ipc.ts,ipc-contract.test.ts}` 为空（约束 ①）
- [x] 交接 阶段七（测试回归）：`use-model-catalog.test.ts`（取数一次 / 失败降级 / 无轮询）、`provider-panel.test.tsx`（组合框建议联动 / 预填四纪律 / 中文标签 / 清单行 effort / 两条既有断言随文案改述）、`use-agent-providers.test.ts`（第 10 位实参对齐）由测试相位承接。**交付物 = 期望登记与移交**

## 阶段八：archi 模型、生成物、版本核验与边界自查

> 依赖全部。本阶段的生成物是 `api:check` 零 diff 门的前提。

- [x] `.dev-team/architecture/models/06-frontend.toml`：
  - `App.Frontend.Components.Standard` 的 `summary` 改「通用交互组件集合（对话框、抽屉、分页、选择、组合框）」
  - `App.Frontend.Components.Standard.Select` 元素块之后追加：
    ```toml
    [element."App.Frontend.Components.Standard.Combobox"]
    kind = "component"
    gate = "off"
    path = ["./packages/desktop/src/components/standard/combobox.tsx"]
    summary = "可选可填组合框（自由输入 + 建议清单）"
    ```
  - 关系表两条：`Standard.Combobox → App.Frontend.Components.Ui`（desc「以组合框原语组装可选可填输入控件」，紧随 `Standard.Select → Ui` 之后）；`App.Frontend.Views.Agents → App.Frontend.Components.Standard.Combobox`（desc「供应商表单的模型与思考强度可选可填输入」，紧随 `Views.Agents → Standard.Select` 之后）
  - **不写 `api` 字段**（保持与 `Select` / `Dailog` 同形；新增 `docMissing` warn 是已知且永不纳管门禁的预期，见 proposal R14）
- [x] 生成物（非手改）：`pnpm -C packages/desktop run api:export` 产出 `.dev-team/architecture/models/Frontend/**`、`graph/INDEX.toml` 与相关 `*.schema.json` / `*.i.ts`
- [x] 自查 阶段八（边界与判据，全部源码级）：
  - **边界零 diff**（逐项 `git diff --stat`）：`crates/core/**`（含 16 份 prompt md）、`crates/infra/agent/src/sdk/{config,compact,normalize,context,resume,reasoning,policy,tools,model_spec,web_search}.rs`、`crates/infra/store/src/{change_port,canonical}.rs`、`src/**`（除 `commands/agents/mod.rs`、`commands/mod.rs`、`bindings/**` 生成物）、`packages/desktop/src/lib/{ipc.ts,ipc-contract.test.ts}`、`packages/desktop/src/components/standard/{dailog,drawer,select,pagination}.tsx` 与 `hooks/use-open.ts`。**登记的期望随动除外**：6 处 `AgentProviderRecord` 字面构造、13 处 `save_agent_provider` 调用、`src/bindings/mod_test.rs` 类型名快照、`packages/desktop/src/lib/ipc*.test.ts` 形参断言、`provider-panel.test.tsx` 两条文案断言——非零 diff 且不属越界（见 design「编译闭合批」）
  - **AC-1 / AC-2**：`native_model_version` 链 1→5 齐、`AgentProviderRecordV4` `pub(crate)` 且无 `#[native_db]`、`new` 七参不变、手写 `Debug` 唯一且含 `efforts`
  - **AC-3 / AC-4 / AC-5**：两档 `trim` 无值域分支；`additional_params: turn.effort_params.clone()`；键名字面量只在 `sdk/effort.rs`
  - **AC-6**：档位三元 `match` 只在 `open_session`；`loop` / 门面 / 组合根零协议判定
  - **AC-7 / AC-8**：`model_catalog.rs` 五不变量静态可考（无凭据 / `[1m]` 窗长 / 显式空 `efforts` / 无 `OnceLock`）；目录零消费者（除命令体）
  - **AC-9 / AC-10 / AC-11 / AC-12**：`StandardCombobox` 零状态零受控选中值；`prefilled` 四处；中文标签与提示、键名零改；`useModelCatalog` 零轮询零 toast
  - **AC-13**：`packages/desktop/package.json` 的 `version` 为 `0.6.55`；`git diff -- packages/desktop/src/types/generated` 全为生成物
- [x] 登记计划外收敛（若有）：实现中若出现「任务未列、实现必需」的最小面，如实留痕于本节（说明为何必需、为何不改变 design 任何裁决的语义面）
  - **收敛 1（已回溯到 proposal / design / spec，见 proposal「实现期回溯 1」、design 校准 9 + DD15）**：`save_agent_provider` 的参数位已满（specta `SpectaFn` 只实现到 `fn(T1..T10)`，`State` 计入其一；`agent_start` 同样停在 10 参），平铺加 `efforts` 使 `bindings:export` 编译失败（E0277，指向 `all_commands!` 展开）。四项可空调参收成末位 `AgentProviderTuning` 组，命令 10 → 8 参；`efforts` 只 `trim`、`web_search` 无条件接受、既有两条校验与错误串零变化，**design 语义面零改**。
  - **收敛 2（既有测试的机械随动，不改断言语义）**：`crates/infra/store/src/model_test.rs` 的八处 `From` 逐级链各补一跳 `AgentProviderRecordV4::from(..)`（V3 → V4 → V5，无跳级 `From` 的既有口径），并补入 `AgentProviderRecordV4` 导入；`src/bindings/mod_test.rs` 的 `COMMAND_NAMES` / `COMMAND_WRAPPERS` 各补一条（53 → 54）、`DTO_TYPES` 补五条（99 → 104），计数断言随动。
  - **收敛 3（v4 中转形态的属性面）**：原名记录的 `#[primary_key]` 是 `native_db` 派生宏的助手属性，随 `#[native_db]` 一并摘掉才能编译（V1–V3 同形）；其余字段与 `#[serde]` 属性逐字未动。
  - **收敛 4（新增文件落点）**：`AgentProviderTuning` 落在 `src/commands/agents/mod.rs`（命令层 DTO，与 `changes/mod.rs` 的 `ActiveRunView` 等同址先例一致；store 面零新增）。
- [x] 交接交付门（本相位不执行，只登记期望）：`pnpm -C packages/desktop run check` 整链（期望 `bindings:export` 零 diff、`api:check` 零 diff、`client:check` 全绿）归 **static-check**；`cargo nextest --workspace` 与 `vp test` 全量归 **test-execution**；effort 的实际深度效果（是否真的少想 / 省 token）**不作实现期门项**，与其余真机项统一并入归档后人工验收（proposal 验收面口径）。**交付物 = 期望登记与移交**
- [x] 交接备忘（发给 test-design / test-gen / test-execution）：
  - **4 个新建测试文件**：`crates/infra/agent/src/sdk/effort_test.rs`、`crates/infra/agent/src/model_catalog_test.rs`、`packages/desktop/src/components/standard/combobox.test.tsx`、`packages/desktop/src/views/agents/hooks/use-model-catalog.test.ts`
  - **既有 sidecar 追加**：`store/model_test.rs`（升级链 / 版本锚点 / Debug 位）、`store/describe_test.rs` / `query_test.rs`（夹具补字段后的既有语义）、`agent/src/sdk/{runner_test,loop_test}.rs`、`agent/src/{compose_test,lib_test}.rs`、`src/commands/agents/mod_test.rs`（trim / 自定义值 / 第 8 位 `tuning` 组 / 目录命令出线）、`src/bindings/mod_test.rs`（类型名快照）、`src/views/agents/components/provider-panel.test.tsx`、`hooks/use-agent-providers.test.ts`、`src/lib/ipc.test.ts`
  - **两个测试模块声明须与 sidecar 同批**：`sdk/mod.rs` 的 `#[cfg(test)] mod effort_test;`、`agent/src/lib.rs` 的 `#[cfg(test)] mod model_catalog_test;`
  - **两条既有断言随文案改述**：`provider-panel.test.tsx` 的「后缀优先于 context_length」→「后缀优先于上下文窗长」、「留空 = 按 context_length 分档推导」→「留空 = 按上下文窗长分档推导」
  - **本相位已跑的生成 / 编译动作（非门禁）**：`cargo run -p export-bindings`（产出 `bindings.ts` / `bindings.zod.ts`）、`cargo run -p archi-api-gen -- generate`（产出 `graph/INDEX.toml`）、`cargo fmt --all`、`cargo check --workspace --all-targets`（零错零警）、`npx tsc --noEmit`（零错）。门禁脚本（`pnpm check` 四段链）与测试回归**未**在本相位运行，归 static-check / test-execution。
