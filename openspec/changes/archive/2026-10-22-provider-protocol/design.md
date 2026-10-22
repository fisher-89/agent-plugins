# 设计: provider-protocol

> **变更**: provider-protocol
> **日期**: 2026-10-21
> **依据**: `proposal.md`（explore §14 决策表 Q1–Q5b 全部锁定）+ 本目录 `specs/desktop-agent-management` / `specs/desktop-agent-execution` delta

---

## 提案与规格同步状态（信息性）

- 两份能力 spec delta（`specs/desktop-agent-management/spec.md` / `specs/desktop-agent-execution/spec.md`）已随提案阶段写入本变更目录，属**提案层产物**，不进变更清单与任务；实现阶段只落代码面与生成物。
- 本设计不需新增/改写 spec delta。与 spec Module Contract 表格的**两处措辞级差异**（`build_model` 第三入参、`Messages::with_default_max_tokens` 的等价写法）已在「决策留痕 #1 / #2」显式裁定，行为面与 AC 一致，spec 文本归档时同步。
- 硬边界重申：`crates/core/agent` 全量零改动、`sdk/loop.rs` / `normalize.rs` / `resume.rs` 零 diff、`sdk/compact.rs` 零 diff、`store` 的 `SessionRecord` / `SessionConfigSnapshot` 零改动、前端 `agent-adapter.ts` / `use-agent-session.ts` 零改动。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| store provider 记录与升级链 | `AgentProviderProtocol` 枚举（线值 `openai` / `anthropic`，bincode 序号锚）、`AgentProviderRecordV2` 中转形态、`AgentProviderRecord` 升 v3（增 `protocol` / `max_tokens`）、V1→V2→V3 逐级 `From`、手写遮蔽 `Debug` 扩位 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | native_db / native_model / serde / specta | bincode 1.3 + native_model 版本链（`ExploreRecord` V1→V4 先例） |
| store 导出面 | re-export `AgentProviderProtocol`（命令层与绑定生成消费） | `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | — | Rust `pub use` |
| 模型名解析纯函数 | `parse_model_spec(raw)`：末尾 `[1m]`（大小写不敏感、两端裁剪）→ 裸名 + `1_000_000`；其余方括号形态 `Err`；无后缀 → 裸名 + `None`。保存期与启动期共用的**唯一**规则源 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs`（新） | 无（纯 std） | 纯函数 + 结构化错误 |
| 门面配置与协议枚举 | `Protocol { Openai, Anthropic }`（门面侧双值映射自 store 枚举）；`EngineConfig` 增 `protocol`（连接配置字段面）；`empty()` 给 `Openai`，`is_complete` 语义不变 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` | — | Rust enum + builder 零成本 |
| SDK 引擎组装双协议 | `build_model` 唯一分叉处：openai 臂逐字等价现状；anthropic 臂条件 `with_beta` + 输出上限定型；`open_session` 按档 `parse` + 按档现算窗长 + 裸名下发；泵启动日志扩位 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `rig` facade（`providers::openai` / `providers::anthropic`）、`context` 防线、`log` | rig 0.43 `DynModel<Completion>` 擦除型（两臂同型） |
| 防线只读访问器 | `ContextDefense::window()` 返回生效窗长（日志留痕用）；水位算法 / 阈值 / prune 语义零改 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs`（清单外补入） | — | Rust 只读 getter |
| 引擎门面 | 新增 `with_max_tokens(Option<u64>)` 旁路 builder（与 `with_context_window` 同型）、`runner_for` 的 sdk 臂转交旁路载荷、`parse_model_spec` / `ModelSpec` / `ModelSpecError` / `MIN_OUTPUT_TOKENS` 出 crate | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | store、sdk | builder 链 + `pub use` |
| 组合根解析单点 | `resolve_agent_engine` 的 sdk 臂：`provider.protocol → Protocol` 两臂映射进 `EngineConfig`；`provider.max_tokens` 经 `ResolvedEngine` 旁路 → `EngineFacade::with_max_tokens` | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | store（全局库）、门面 | 组合根单点匹配 |
| 命令面保存期校验 | `save_agent_provider` 增 `protocol` / `max_tokens` 入参；三档模型名经引擎侧同一 `parse_model_spec` 校验、`max_tokens < 4096` 拒收；留空 key 回填原值逻辑不动 | `packages/desktop/src-tauri/src/commands/agents/mod.rs` | store、agent-runtime（新消费 `parse_model_spec`）、tauri-specta | `Result<T, String>` 模板 |
| IPC 类型镜像（生成物） | `AgentProviderProtocol` 枚举 + 记录 `protocol` / `maxTokens` + `saveAgentProvider` 新参数序（由 `pnpm run bindings:export` 再生成，非手改） | `packages/desktop/src/types/generated/bindings.ts` | export-bindings crate | specta 重导出 |
| 管理页取数动作面 | `AgentProviderSaveInput` 增 `protocol` / `maxTokens`；`commands.saveAgentProvider(...)` 实参按新序 | `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | bindings | React hook |
| 管理页表单与列表 | 协议选择器（`StandardSelect`）、`maxTokens` 数字字段、`base_url` placeholder 与提示随协议切换、列表协议徽标、模型名后缀与「已覆盖」提示 | `packages/desktop/src/views/agents/components/provider-panel.tsx` | bindings、`StandardSelect`、既有 hooks | React 受控表单 |

---

## 设计定值（design 拍板）

| 编号 | 项 | 定值 | 依据 |
|------|----|------|------|
| DD1 | 门面 `Protocol` 形态 | `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] pub enum Protocol { #[default] Openai, Anthropic }` + `pub fn as_str(self) -> &'static str`（恒 `"openai"` / `"anthropic"`，日志与诊断用）；**不派生 serde / specta**（引擎内部词汇，不出 crate、不进 bindings） | 门面词汇与 IPC 词汇分层（`EngineKind` 出 bindings 因其是 `agent_start` 参数；`Protocol` 只住引擎）；`Protocol` 无凭据，可派生 `Debug`（与 `EngineConfig` 刻意不派生 `Debug` 的边界不冲突） |
| DD2 | `EngineConfig` 字段定序 | `{ protocol, api_key, base_url, model_high, model_low }`——`protocol` 置首（决定 `base_url` 语义与 wire）；`empty()` 显式给 `Protocol::Openai`；`is_complete()` **实现与语义均不变**（协议恒有值，不参与校验），仅 doc 口径改「连接配置字段面齐备检查」 | spec「旁路与配置面的分工」；R5（字段面收敛，18 处字面量补一个 `Protocol::Openai` 即够） |
| DD3 | `build_model` 签名 | `fn build_model(config: &EngineConfig, spec: &ModelSpec, max_tokens: Option<u64>) -> DynModel<Completion>`（第三个位置参数承载 `provider.max_tokens`；`MUST NOT` 进 `EngineConfig`）。仍为协议唯一分叉处 | 输出上限取值链需要 provider 旁路载荷入参，而 `EngineConfig` 由 DD2 / spec 明确禁止承载 `max_tokens`；spec Module Contract 该行是提案层速记，措辞差异见决策留痕 #1 |
| DD4 | anthropic 输出上限定型写法 | 组装后读 `model.wire.default_max_tokens`（`Option<u64>`，`Copy`）按链求值，再 `model.wire.default_max_tokens = Some(cap)` 字段写入；**不**用消费型 `with_default_max_tokens(self)`（`Model.wire` 为 pub 字段，消费型 builder 需从 `Model` 搬移 wire） | 与 spec 的「定型 wire 默认 `max_tokens`」等价（`Messages.default_max_tokens` 恒被定为非 `None`，wire 层 `encode` 在 `request.max_tokens.is_none()` 时取它，`loop.rs` / `compact.rs` 零改动）；见决策留痕 #2 |
| DD5 | 输出上限取值链 | `let cap = max_tokens.or(model.wire.default_max_tokens).unwrap_or(OUTPUT_TOKENS_FALLBACK);` 其中 `OUTPUT_TOKENS_FALLBACK = 32_000`（`sdk/runner.rs` 私有常量）——provider 显式值优先、rig 名单内保留官方值（不被降级）、名单外兜底 32K | AC-8 / Q4e；单行等价式覆盖 rig 名单内外与用户降档三态 |
| DD6 | `max_tokens` 保存期下限常量 | `pub const MIN_OUTPUT_TOKENS: u64 = 4_096;` 落 `sdk/runner.rs`，经 crate 根再导出供命令层消费（无第二套字面量） | AC-10 / Q4f；同「`parse_model_spec` 单点」纪律 |
| DD7 | `1m` 常量与 beta 值 | `MILLION_CONTEXT_WINDOW: u64 = 1_000_000`（1000 进制）、`BETA_CONTEXT_1M: &str = "context-1m-2025-08-07"`——均落 `sdk/model_spec.rs`（`pub(crate)`，与后缀语义同址） | Q4b / Q4c；beta 判定条件 = `protocol == Anthropic && spec.length == Some(MILLION_CONTEXT_WINDOW)`（即「后缀在场」的唯一可考表示） |
| DD8 | `parse_model_spec` 边界规则（规则表未覆盖项） | ①空串 / 全空白 → `Ok(ModelSpec{ name: "", length: None })`（**保现状**：provider 三档模型名可留空保存，齐备性由启动期 `ConfigMissing` 兜底，保存期不得新增拒绝面）；②`"[1m]"`（后缀在场但裸名为空）→ `Err`（方括号语义悬空）；③`"x[1m]y"`（方括号非末尾）→ `Err`；④`"[ 1m ]"`（内空白）→ `Err`；⑤剥离后裸名两端再次 `trim`（`"x [1m]"` → `"x"`） | AC-6；① 是「保存期校验不得扩大拒收面」的硬约束（现有 provider 允许空模型名落库），②–⑤ 为「其余方括号形态一律拒收」的自然枚举 |
| DD9 | `parse_model_spec` 后缀匹配实现 | 先 `trim`，若 `len >= 4` 且末 4 字节 `eq_ignore_ascii_case("[1m]")` → 剥离；否则若文本任一处含 `[` 或 `]` → `Err`；再对裸名 `trim` 后按 DD8① 判定 | 纯 ASCII 后缀可用字节切片（`]` 为 ASCII 边界，不会切开 UTF-8）；大小写不敏感只针对 ASCII，与 Claude Code 的 `/\[1m\]/i` 同口径 |
| DD10 | `ModelSpecError` 形态 | `#[derive(Debug, Clone, PartialEq, Eq)] pub struct ModelSpecError { pub raw: String, pub reason: String }` + `impl Display`（`模型名后缀非法: {raw}（{reason}）`）；**只承载模型名原文**，不可能含 api_key（AC-10 错误串约束由结构保证，非约定） | spec「携原文与原因」；保存期与启动期共用同一错误面 |
| DD11 | 按档窗长与 `with_context_window` | `open_session` 内 `let window = spec.length.or(self.context_window); let defense = ContextDefense::resolve(window);`——优先级 `后缀 > context_length > 128K` 由「`Option::or` + `ContextDefense::resolve` 的既有缺省」自然形成，`EngineFacade::with_context_window` 签名与语义（缺省来源）不动 | AC-7；按档现算（`high` / `low` 各取各自原文解析） |
| DD12 | 裸名上报与日志载荷 | `open_session` 用 `spec.name` 作为 `LoopTurn.model_name`（`RunStarted.model` = 裸名）；`SessionConfigSnapshot.model` 仍取 `config.model_high` / `model_low` **原文**（组合根现有装配零改动）；日志由泵启动行承载：`泵启动 session=… protocol={as_str} model={裸名} window={defense.window()} raw={原文}` | R9 / Q4 裁定；`window` 取**生效**窗长（含 128K 缺省），故需 `ContextDefense::window()` 只读访问器（DD13）；`PumpPayload` 增 `protocol` / `raw_model` 两个私有字段承载日志原料（`loop.rs` 零改动） |
| DD13 | `ContextDefense` 只读访问器 | `pub(crate) fn window(&self) -> u64`（返回 `self.context_window`）。水位比、`protected_tokens` / `prune_threshold`、prune / compaction 算法**零改** | 日志需生效窗长；在 runner 侧重算 128K 缺省会复制常量与「0 → 缺省」归一化逻辑（双源漂移），访问器是唯一不加规则的做法；列入变更清单（清单外补入） |
| DD14 | `SdkRunner` 旁路载荷 | 增私有字段 `max_tokens: Option<u64>` + `pub fn with_max_tokens(self, max_tokens: Option<u64>) -> Self`；**`new` 四参签名不动**（36 处调用点零改），`new` 内 `max_tokens: None` | R5 明文要求；与 `context_window` 并列的解码后载荷 |
| DD15 | 组合根映射点 | `compose.rs` 私有 `fn map_protocol(protocol: AgentProviderProtocol) -> Protocol`（两臂 match），`resolve_agent_engine` 的 sdk 臂 `EngineConfig { protocol: map_protocol(provider.protocol), … }`、`ResolvedEngine.max_tokens = provider.max_tokens`（与 `context_window` 并列） | 沿 `AgentEngineKind → EngineKind` 双枚举映射先例；解析层不对协议做二次判断（选臂只在 `build_model`） |
| DD16 | 保存期校验顺序 | `save_agent_provider` 内：①三档模型名按 `high → medium → low` 顺序 `parse_model_spec`，首个 `Err` 即返回（错误串含档位名与 `ModelSpecError` 原文，**不含 api_key**）；②`max_tokens` 为 `Some(n)` 且 `n < MIN_OUTPUT_TOKENS` → `Err`（文案含下限 `4096`）；③通过后执行既有留空 key 回填；④`upsert_agent_provider` | AC-10；校验前置避免「读库回填 → 才发现非法」的无谓 IO；重名 / 行存在校验仍归 store 单点 |
| DD17 | 记录字段定序与 `new` 形参 | `AgentProviderRecord` 字段序 `id, name, protocol, base_url, api_key, models, context_length, max_tokens`；`new(name, protocol, base_url, api_key, models, context_length, max_tokens)`（七参，`protocol` 紧跟 `name`——协议决定 `base_url` 语义） | spec Module Contract 明示形态；`protocol` 置 `name` 后、`max_tokens` 置 `context_length` 后（可空调参相邻） |
| DD18 | 升级链形态 | 删既有 V1↔V3 直连 `From` 两 impl，改为逐级四 impl：`V1 → V2`（补 `context_length = None`）、`V2 → V1`（丢列）、`V2 → V3`（补 `protocol = Openai` / `max_tokens = None`）、`V3 → V2`（丢两列）；`AgentProviderRecordV2` 为 `pub(crate)`（不注册进库模型组、不出公共查询面） | `ExploreRecord` V1→V4 四级链先例（无跳级 `From`）；AC-2 |
| DD19 | 前端协议控件与提示落点 | 新增 `ProtocolSelect` 子组件（`StandardSelect`，`items = [{value:'openai'},{value:'anthropic'}]`，`onChange` 收到 `null` 时保持现值）；`ProviderFormState` 增 `protocol: AgentProviderProtocol`（新建 `'openai'`）与 `maxTokens: string`；`base_url` placeholder 由常量表 `BASE_URL_HINTS: Record<AgentProviderProtocol, { placeholder, hint }>` 决定（openai `https://…/v1`；anthropic `https://api.anthropic.com` + 「`/v1` 等后缀由引擎自动剥离后拼 `/v1/messages`」）；提示区三个锚：`provider-base-url-hint` / `provider-max-tokens-hint` / `provider-suffix-hint`；列表行徽标锚 `provider-protocol-badge`；`maxTokens` 边界函数 `parseMaxTokens(value): number \| null \| undefined`（空 = `null`、非整数或 `< 4096` = `undefined` → `canSave` 拦） | AC-11；控件复用既有标准组件（`desktop-standard-components` 零新增）；锚名供测试相位消费 |
| DD20 | 前端保存入参序 | `commands.saveAgentProvider(id, name, protocol, baseUrl, apiKey, models, contextLength, maxTokens)`（与命令签名逐位对齐）；`AgentProviderSaveInput` 增 `protocol: AgentProviderProtocol; maxTokens: number \| null` | AC-11；实参序错位是本变更最易漏的机械点，故以签名对齐为唯一标准 |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此为边界。覆盖 proposal「变更范围 - 实现文件」全部 12 个文件；标注「清单外补入」者为级联触点（均附理由）。测试用例（proposal「测试文件」表）由 test-design → test-gen → test-execution 承接，不在本清单；但签名 / 字段面波及既有测试文件的**编译面机械随动**（保编译、不写断言，沿 2026-10-10-unify-run-state-persistence 先例）归各实现阶段，作为「编译面随动」行列出。唯一进一步例外是 proposal「测试装置修复」登记的 `checks` slot 用例环境敏感修复（test-execution 首轮暴露、阻塞 gate），单列为「测试装置修复」表。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs` | 模型名后缀解析纯函数单点：`parse_model_spec` / `ModelSpec` / `ModelSpecError` / `MILLION_CONTEXT_WINDOW` / `BETA_CONTEXT_1M`（DD7 / DD8 / DD9 / DD10）。**无 I/O、无网络、无 store 依赖**——保存期（命令层）与启动期（引擎）共用 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | ①新增 `pub enum AgentProviderProtocol { Openai, Anthropic }`（`Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type` + `#[serde(rename_all = "camelCase")]` + `Default = Openai`；变体声明序即 bincode 序号，`Openai`=0 / `Anthropic`=1，MUST NOT 重排，注释标解码锚）；②新增 `pub(crate) struct AgentProviderRecordV2`（现 v2 形态：`id/name/base_url/api_key/models/context_length`，`#[native_model(id = 5, version = 2, from = AgentProviderRecordV1)]`）；③`AgentProviderRecord` 改 `#[native_model(id = 5, version = 3, from = AgentProviderRecordV2)]`，增 `#[serde(default)] pub protocol`（置 `name` 后）与 `#[serde(default)] pub max_tokens: Option<u64>`（置 `context_length` 后，DD17）；④删 V1↔V3 直连 `From` 两 impl，改逐级四 impl（DD18）；⑤`AgentProviderRecord::new` 增 `protocol` / `max_tokens` 两参（七参，DD17）；⑥手写遮蔽 `Debug` 增 `protocol` / `max_tokens` 两位（字段面与 derive 对齐，`api_key` 位仍 `mask_api_key`）；⑦`AgentProviderRecordV1` doc 口径改「升级链中转至 V2」；`base_url` 字段注释由「openai 兼容端点」改「协议对应端点（openai 含版本段；anthropic 由引擎归一化）」 | AC-1 / AC-2 / AC-3；一次 DB 演进（V2 → V3）把字段面定死 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | `pub use model::{… AgentProviderProtocol …}`（插入 `AgentProviderRecord` 之后，字母序内） | 命令层与绑定生成消费（AC-1 / AC-12） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` | 新增 `Protocol` 枚举 + `as_str`（DD1）；`EngineConfig` 增 `protocol: Protocol`（置首，DD2）；`empty()` 给 `Protocol::Openai`；`is_complete()` 实现不变、doc 改「连接配置字段面齐备检查（协议恒有值不参与校验）」；模块 doc 补「连接配置字段面：协议 + 凭据 + 端点 + 双档模型；可空调参 `context_length` / `max_tokens` 走门面旁路」 | AC-4 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 挂载 `pub(crate) mod model_spec;`（按字母序插在 `log` 之后 / `normalize` 之前）。`#[cfg(test)] mod model_spec_test;` 与测试文件由测试相位一体落（决策留痕 #4） | 新模块挂载 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | ①`use` 增 `rig::providers::anthropic::AnthropicConfig`；②常量 `OUTPUT_TOKENS_FALLBACK: u64 = 32_000`（私有）与 `pub const MIN_OUTPUT_TOKENS: u64 = 4_096`（DD5 / DD6）；③`SdkRunner` 增 `max_tokens: Option<u64>` 字段 + `with_max_tokens(self, Option<u64>) -> Self`（`new` 四参签名不动，内给 `None`，DD14）；④`open_session`：选档后按档 `parse_model_spec(&raw)`（`Err` → `AgentStartError::ConfigMissing` 显式失败，文案含档位与 `ModelSpecError`）→ `spec.name` 作 `LoopTurn.model_name` → `build_model(&self.config, &spec, self.max_tokens)` → `ContextDefense::resolve(spec.length.or(self.context_window))`（DD11）；⑤`build_model(config, spec, max_tokens)` 双臂：openai 臂 `.chat(spec.name).erase()` 逐字等价现状（不写任何请求参数）；anthropic 臂 `AnthropicConfig::new(key).with_base_url(url)`[`protocol == Anthropic && spec.length == Some(MILLION_CONTEXT_WINDOW)` 时 `.with_beta(BETA_CONTEXT_1M)`]`.client().completion(spec.name)` + 输出上限定型（DD3 / DD4 / DD5）；⑥`PumpPayload` 增 `protocol: Protocol` / `raw_model: String` 私有字段；⑦泵启动日志改 `泵启动 session={} protocol={} model={裸名} window={defense.window()} raw={原文}`（DD12） | AC-5 / AC-7 / AC-8 / AC-9；loop / normalize / resume 零改动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs`（清单外补入·只读访问器） | 增 `pub(crate) fn window(&self) -> u64`（返回 `self.context_window`，DD13）：日志需生效窗长，避免在 runner 侧重算 128K 缺省与「0 → 缺省」归一化（双源漂移）。**水位比 / 保护窗 / prune 门槛 / prune 与 compaction 算法零改** | 级联触点：DD12 日志载荷的唯一不加规则来源 |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | ①`pub use sdk::model_spec::{parse_model_spec, ModelSpec, ModelSpecError};` 与 `pub use sdk::runner::MIN_OUTPUT_TOKENS;`（命令层保存期校验复用同一函数与同一门槛）；②`EngineFacade` 增 `max_tokens: Option<u64>` 字段 + `with_max_tokens(self, Option<u64>) -> Self`（与 `with_context_window` 同型；`new` / `with_resume_transcript` / 既有 builder 三处初始化补 `max_tokens: None`）；③`runner_for` sdk 臂改 `SdkRunner::new(…).with_max_tokens(self.max_tokens)`（`runner_for` 签名与唯一 match 点结构不变）；④`EngineFacade` / `context_window` / `with_context_window` / `runner_for` 的「`EngineConfig` 三字段」措辞改「连接配置字段面」 | AC-4 / AC-7 / AC-10 |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | ①新增私有 `fn map_protocol(protocol: AgentProviderProtocol) -> Protocol`（两臂 match，DD15）；②`ResolvedEngine` 增 `max_tokens: Option<u64>`（与 `context_window` 并列，注释标「旁路载荷」）；③`resolve_agent_engine` 的 sdk 臂 `EngineConfig { protocol: map_protocol(provider.protocol), … }` + `max_tokens: provider.max_tokens`（解析语义——默认 / 显式 / high 档 / cli 拒收——零变）；④`compose_turn` 的 runner 构造链插 `.with_max_tokens(resolved.max_tokens)`；⑤`use store::{… AgentProviderProtocol …}`；⑥字段注释「`context_length` 列旁路」改「可空调参旁路（窗长 / 输出上限）」 | AC-4 + 协议透传（spec「协议透传到引擎」） |
| `packages/desktop/src-tauri/src/commands/agents/mod.rs` | ①`use agent_runtime::{parse_model_spec, MIN_OUTPUT_TOKENS};`（新增对外依赖消费，crate 已依赖 `agent-runtime`，无 Cargo.toml 变更）；②`save_agent_provider(id: Option<i64>, name: String, protocol: AgentProviderProtocol, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>, max_tokens: Option<u64>)`；③校验按 DD16（三档 parse → `max_tokens` 下限 → 留空 key 回填 → upsert）；④doc 注释更新（协议紧跟 name 的理由、校验面、错误串不含 api_key）；`list` / `delete` 命令零改动（清单自动带出新字段） | AC-10 |
| `packages/desktop/src/types/generated/bindings.ts` | `pnpm -C packages/desktop run bindings:export` 再生成（非手改）：`AgentProviderProtocol` 枚举型、`AgentProviderRecord` 增 `protocol` / `maxTokens`、`saveAgentProvider` 八参新序 | AC-12（`bindings:check` 零 diff 的前提） |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | `AgentProviderSaveInput` 增 `protocol: AgentProviderProtocol` / `maxTokens: number \| null`；`commands.saveAgentProvider(...)` 实参按 DD20 新序；doc 注释补「maxTokens 留空 = 按协议缺省存 null」 | AC-11（保存入参） |
| `packages/desktop/src/views/agents/components/provider-panel.tsx` | ①`ProviderFormState` 增 `protocol: AgentProviderProtocol`（`EMPTY_FORM` = `'openai'`）/ `maxTokens: string`；②新增 `ProtocolSelect`（`StandardSelect`）+ row 0 前置渲染；`MAX_TOKENS` 进 `PROVIDER_FIELDS`（row 2，与 `contextLength` 相邻，placeholder「留空 = 按协议缺省」）；③`BASE_URL_HINTS` 常量驱动 `base_url` placeholder 与提示，随 `form.protocol` 切换（DD19）；④`parseMaxTokens` 边界函数（DD19）+ `canSave` 增该判定 + `submit` 增 `protocol` / `maxTokens` 实参；⑤`toFormState` 回填 `protocol` / `maxTokens`（`null` → `''`）；⑥提示区三锚（`provider-base-url-hint` / `provider-max-tokens-hint` / `provider-suffix-hint`，其中后缀提示在「某档带 `[1m]` 且 `contextLength` 非空」时追加「`<档位>` 档窗长已被模型名覆盖」）；⑦`ProviderList` 行增协议徽标 `provider-protocol-badge`；⑧模块 / 组件 doc 口径更新 | AC-11 |

**编译面机械随动**（保编译、不写断言；新增断言归测试相位）：

| 文件路径 | 修改内容 |
|----------|----------|
| `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs` | `AgentProviderRecord::new` 调用点（13 处）补 `protocol` / `max_tokens` 实参；`AgentProviderRecordV1` 升级往返用例的 `From` 面随逐级链随动（编译面） |
| `packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs`（1 处）、`store_test.rs`（2 处） | `AgentProviderRecord::new` 调用点补参 |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`（2 处）、`worker_test.rs`（2 处） | `AgentProviderRecord::new` 调用点补参；`resolve_agent_engine` 用例的 `EngineConfig` 断言面随字段面随动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs`（4 处 `EngineConfig` 字面量）、`sdk/config_test.rs`（6 处）、`sdk/runner_test.rs`（7 处，含 `complete_config()` 底座） | `EngineConfig` 字面量补 `protocol: Protocol::Openai`（缺省档，保持既有断言语义）；`SdkRunner::new` 36 处调用点**零改**（DD14） |
| `packages/desktop/src-tauri/src/commands/agents/mod_test.rs` | `save_agent_provider` 既有调用点补 `protocol` / `max_tokens` 实参（编译面；新增校验用例归测试相位） |
| `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`（1 处）、`commands/exec/mod_test.rs`（1 处） | `AgentProviderRecord::new` fixture 补参 |
| `packages/desktop/src/views/agents/agents-view.test.tsx`、`components/agent-panel.test.tsx`、`components/provider-panel.test.tsx`、`hooks/use-agent-providers.test.ts` | `AgentProviderRecord` fixture 补 `protocol` / `maxTokens`；`save` 入参断言面随新序随动（编译面 / 保绿面；新增用例归测试相位） |

**测试装置修复**（非功能；test-execution 首轮暴露，唯一例外——不属于本需求功能面，但阻塞 test-execution gate，故纳入变更范围）：

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs` | ①`run_slot_baseline_pass_报告与exec_outputs落baseline树` / `run_slot_baseline_fail_report_dir仍回填baseline锚` / `run槽位等价性_语义同锚点异且err面持衡` 三个用例各补 `let _path = PathPrefix::enter(ws.path());`；②`run槽位等价性_…` 增「两槽位 `diagnostics` 均含 shim 输出」断言 | 三处 `write_shim` 只落文件 + 裸名登记、未前置 PATH，依赖 `cmd` 的 cwd 搜索；宿主 `NoDefaultCurrentDirectoryInExePath=1` 时 spawn 期报「command not found」→ 唯一真实失败 + 持锁 panic 毒化级联（56 条伪失败 + `testexec` 连坐）；②防「双不可达」下等价性空转假绿。**产品代码 `static_check.rs` 零改**（生产逻辑正确，问题在用例假设） |

<!-- 删除文件：无整文件删除。被删的符号为 `model.rs` 内两条直连 `From` impl（`AgentProviderRecordV1 ↔ AgentProviderRecord`），随 DD18 改写为逐级四 impl，落在上表 model.rs 行，不另立删除子表。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `parse_model_spec` | `…/crates/infra/agent/src/sdk/model_spec.rs` | 新增 | `pub fn parse_model_spec(raw: &str) -> Result<ModelSpec, ModelSpecError>` | 后缀解析唯一规则源（DD8 / DD9）；经 crate 根再导出，命令层保存期校验复用（AC-6 / AC-10） |
| `ModelSpec::length` 消费面 | 同上 | 新增 | `pub struct ModelSpec { pub name: String, pub length: Option<u64> }` | `length = Some(1_000_000)` 即「后缀在场」，同时驱动窗长（DD11）与 beta（DD7） |
| `MIN_OUTPUT_TOKENS` | `…/crates/infra/agent/src/sdk/runner.rs`（经 `lib.rs` 再导出） | 新增 | `pub const MIN_OUTPUT_TOKENS: u64 = 4_096;` | provider `max_tokens` 保存期下限，与 `parse_model_spec` 同「单点」纪律（DD6） |
| `Protocol::as_str` | `…/crates/infra/agent/src/sdk/config.rs` | 新增 | `pub fn as_str(self) -> &'static str` | 日志 / 诊断面 `protocol=openai\|anthropic`（DD1 / DD12） |
| `EngineConfig::empty` | `…/crates/infra/agent/src/sdk/config.rs` | 修改（签名不变） | `pub fn empty() -> Self` | 体内补 `protocol: Protocol::Openai`（DD2） |
| `EngineConfig::is_complete` | 同上 | 修改（签名不变） | `pub fn is_complete(&self) -> bool` | 实现零改（协议恒有值不参与校验）；仅 doc 口径 |
| `EngineFacade::with_max_tokens` | `…/crates/infra/agent/src/lib.rs` | 新增 | `pub fn with_max_tokens(self, max_tokens: Option<u64>) -> Self` | 与 `with_context_window` 同型的旁路 builder（AC-7 配套；MUST NOT 进 `EngineConfig`） |
| `EngineFacade::with_context_window` | 同上 | 修改（签名不变） | `pub fn with_context_window(self, context_window: Option<u64>) -> Self` | 体内补 `max_tokens: self.max_tokens` 字段透传；语义降为「缺省来源」 |
| `EngineFacade::runner_for` | 同上 | 修改（签名不变） | `pub fn runner_for(&self, kind: EngineKind, engine_cfg: EngineConfig) -> Result<Box<dyn AgentRunner>, String>` | sdk 臂尾挂 `.with_max_tokens(self.max_tokens)`；唯一 match 点与 `EngineKind` 双值不塌（AC-4） |
| `SdkRunner::with_max_tokens` | `…/crates/infra/agent/src/sdk/runner.rs` | 新增（crate 内 API） | `pub fn with_max_tokens(self, max_tokens: Option<u64>) -> Self` | 旁路载荷注入；`SdkRunner::new` 四参不动（DD14） |
| `AgentProviderRecord::new` | `…/crates/infra/store/src/model.rs` | 修改 | `pub fn new(name: String, protocol: AgentProviderProtocol, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>, max_tokens: Option<u64>) -> Self` | 七参（DD17）；生产 1 处 + 测试 fixture 若干随参 |
| `save_agent_provider` | `…/src-tauri/src/commands/agents/mod.rs` | 修改 | `pub fn save_agent_provider(stores: State<'_, WorkspaceStores>, id: Option<i64>, name: String, protocol: AgentProviderProtocol, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>, max_tokens: Option<u64>) -> Result<AgentProviderRecord, String>` | 保存期校验单点（DD16）：三档模型名 `parse_model_spec` + `max_tokens >= 4096`；留空 key 回填原值不变（AC-10） |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `AgentProviderProtocol` | `…/crates/infra/store/src/model.rs` | 新增 | `enum { Openai, Anthropic }`；serde camelCase 线值 `"openai"` / `"anthropic"`；`Default = Openai`；`specta::Type`（随记录生成 TS）；**变体声明序 = bincode 解码锚（`Openai`=0 / `Anthropic`=1），MUST NOT 增删重排**（AC-2） |
| `AgentProviderRecord` | 同上 | 修改 | 升 `v3`：增 `protocol`（`#[serde(default)]`，置 `name` 后）与 `max_tokens: Option<u64>`（`#[serde(default)]`，置 `context_length` 后）；`#[native_model(id = 5, version = 3, from = AgentProviderRecordV2)]`（AC-1 / AC-3） |
| `AgentProviderRecordV2` | 同上 | 新增 | `pub(crate)`，现 v2 形态（无 `protocol` / `max_tokens`），`#[native_model(id = 5, version = 2, from = AgentProviderRecordV1)]`；仅作升级链中转，不注册进库模型组、不出公共查询面（AC-2 / DD18） |
| `AgentProviderRecordV1` | 同上 | 修改（注释级） | 变体集 / 字段面 / 版本号零改；doc 改「升级链中转至 V2」 |
| `Protocol` | `…/crates/infra/agent/src/sdk/config.rs` | 新增 | `enum { Openai, Anthropic }`（`Debug, Clone, Copy, PartialEq, Eq, Default`）；门面词汇，不派生 serde / specta（不出 crate、不进 bindings，DD1） |
| `EngineConfig` | 同上 | 修改 | 字段面 `{ protocol, api_key, base_url, model_high, model_low }`（连接配置字段面）；**刻意不派生 `Debug`**（凭据不入日志）不变；`max_tokens` / `context_length` MUST NOT 进本结构（AC-4） |
| `ModelSpec` | `…/crates/infra/agent/src/sdk/model_spec.rs` | 新增 | `{ name: String, length: Option<u64> }`；`Debug, Clone, PartialEq, Eq`；纯内存值、不持久化 |
| `ModelSpecError` | 同上 | 新增 | `{ raw: String, reason: String }` + `Display`；保存期与启动期共用；结构上不可能含 api_key（DD10） |
| `ResolvedEngine`（组合根内部结构） | `…/crates/infra/agent/src/compose.rs` | 修改 | 增 `max_tokens: Option<u64>`（旁路载荷，与 `context_window` 并列） |
| `PumpPayload`（引擎内部结构） | `…/crates/infra/agent/src/sdk/runner.rs` | 修改 | 增 `protocol: Protocol` / `raw_model: String`（泵启动日志原料；`loop.rs` 零改动） |
| `SdkRunner` | 同上 | 修改 | 增私有 `max_tokens: Option<u64>` + `with_max_tokens` builder |
| `ContextDefense` | `…/crates/infra/agent/src/sdk/context.rs` | 修改（只读） | 增 `pub(crate) fn window(&self) -> u64`（DD13）；算法与阈值零改 |
| `AgentProviderSaveInput`（前端内部接口） | `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | 修改 | 增 `protocol: AgentProviderProtocol` / `maxTokens: number \| null`（DD20） |
| `ProviderFormState`（前端内部接口） | `packages/desktop/src/views/agents/components/provider-panel.tsx` | 修改 | 增 `protocol: AgentProviderProtocol` / `maxTokens: string`（DD19） |

### 配置

<!-- 不涉及：本变更不新增 / 修改任何配置键（无 config schema、环境变量、快捷键或 package.json 变更）。beta 值 `context-1m-2025-08-07` 与 `max_tokens` 下限 `4096` 是引擎常量而非可配置项（Q4c / Q4f 已锁定）。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `AgentProviderRecord`（v3） | `id`(PK, `i64`)、`name`(唯一，写事务内查重)、`protocol: AgentProviderProtocol`（缺省 `Openai`）、`base_url: String`、`api_key: String`（明文，仅展示层遮蔽）、`models: AgentModelTiers { high, medium, low }`（可带 `[1m]` 后缀的模型原文）、`context_length: Option<u64>`（窗长缺省来源）、`max_tokens: Option<u64>`（输出上限，`None` = 按协议缺省） | 被 `AgentInstanceRecord.provider_id` 引用（删除被引用 provider 阻止）；`protocol` 决定 wire 与 `base_url` 语义，`max_tokens` 仅 anthropic 消费 | 全局库（user 维度），`#[native_model(id = 5, version = 3, from = AgentProviderRecordV2)]`，bincode 1.3；v1 行走 V1→V2→V3 逐级升级（`protocol = openai` / `max_tokens = None`，行为零变化），v2 行走 V2→V3 一级升级，**零迁移脚本** |
| `AgentProviderRecordV2`（中转） | `id`、`name`、`base_url`、`api_key`、`models`、`context_length` | 仅作版本链解码 / 降级目标 | 不落新库、不注册进 `global_models()` 模型组（`AgentProviderRecordV1` 同型先例） |
| `AgentProviderProtocol`（枚举） | `Openai`(=0) / `Anthropic`(=1) | 落 `AgentProviderRecord.protocol` | bincode 变体序号钉死（解码锚）；JSON 线值 `("openai" \| "anthropic")`，specta 生成 TS 枚举 |
| `ModelSpec`（值对象） | `name: String`、`length: Option<u64>` | 由 `parse_model_spec` 从三档模型原文派生；`length` → `ContextDefense` 窗长候选 + beta 判定 | **不持久化**（每轮 `open_session` 现算；`SessionConfigSnapshot` 保留配置原文，Q3 不加协议字段） |
| `Protocol`（门面枚举） | `Openai` / `Anthropic` | 由 `AgentProviderProtocol` 一对一映射（组合根单点）；住 `EngineConfig.protocol` | 不持久化（`EngineConfig` 是运行期组装产物） |

---

## 路由 / API 设计

无 HTTP 端点；变更面为 Tauri IPC 命令（本地进程，无 auth；`Err(String)` 由 Tauri 转前端 reject）。

| 命令 | 状态 | 输入 | 输出 | 说明 |
|------|------|------|------|------|
| `save_agent_provider` | 修改 | `id: i64 \| null, name: string, protocol: "openai" \| "anthropic", base_url: string, api_key: string, models: {high, medium, low}, context_length: number \| null, max_tokens: number \| null` | `AgentProviderRecord`（含 `protocol` / `maxTokens`）或 `Err` | 参数序 = 命令签名序（`protocol` 紧跟 `name`）；保存期校验：非法后缀 / `max_tokens < 4096` → `Err`（错误串不含 api_key） |
| `list_agent_providers` | 不变 | — | `AgentProviderRecord[]` | 记录新字段自动带出 |
| `delete_agent_provider` | 不变 | `id: i64` | `boolean` | 被 agent 引用时 `Err`（无级联）语义不变 |
| `list_agent_instances` / `save_agent_instance` / `delete_agent_instance` / `set_default_agent_instance` | 不变 | — | — | agent 实例零改动（协议挂 provider，Q1） |

---

## 依赖

### 运行时依赖

- **无新增、无删除**：`rig` facade 已锁版 `=0.43.0`，`providers::anthropic` 在 rig 0.43 **无 feature gate**，现有 features（`reqwest` / `native-tls` / `memory`）已满足 `.client()` 与 `.completion()` 的需要——MUST NOT 为此新增 feature 或同伴 crate 条目。
- `store` / `agent-runtime` / `src-tauri` 的 crate 依赖边零变化（`src-tauri` 已依赖 `agent-runtime`，本变更只是新增符号消费）。
- `native_db` / `native_model`（bincode 1.3 默认 codec）承接 V2→V3 版本链，零新增。
- 前端：无新增 npm 依赖（协议选择器复用既有 `StandardSelect`）。

### 构建 / 测试依赖

- 无新增：bindings 再生成走既有 `pnpm -C packages/desktop run bindings:export` 通道；测试依赖面归测试相位（`rig` 的 `test-utils` dev-dependency 已在位，本变更新增断言不打网络、只断言 `DynModel` 描述面）。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 数据模型表（v3 字段面 / V2 中转 / 逐级升级）+ model.rs 行 ②③④ + 升级用例归测试相位（`model_test` / `envelope_test` / `store_test`） |
| AC-2 | 类型定义 `AgentProviderProtocol`（序号锚注释）+ `AgentProviderRecordV2`（`pub(crate)`、不进 `global_models()`）+ store/lib.rs 导出面 diff（只增协议枚举） |
| AC-3 | model.rs 行 ⑥（手写遮蔽 `Debug` 增 `protocol` / `max_tokens` 位，`api_key` 位仍 `mask_api_key`）；`PartialEq` 派生面不变 |
| AC-4 | DD2 / DD3 / DD15 + config.rs 行 + lib.rs 行 ③（`runner_for` 唯一 match 点与 `EngineKind` 双值不塌）+ compose.rs 行 ③ |
| AC-5 | DD3 / DD5 + runner.rs 行 ⑤（openai 臂逐字等价、anthropic 臂同产 `DynModel<Completion>`）；断言归测试相位（只断言 `name()` / `id()` 描述面） |
| AC-6 | DD8 / DD9 / DD10 + model_spec.rs 新增文件 + lib.rs 行 ①（同一函数出 crate） |
| AC-7 | DD11 + runner.rs 行 ④ + lib.rs 行 ②（`with_context_window` 签名不变、语义降为缺省来源） |
| AC-8 | DD4 / DD5 / DD6 + runner.rs 行 ②⑤（三档取值链；openai 臂不写 `max_tokens`）；`loop.rs` / `compact.rs` 零 diff 由守线任务核验 |
| AC-9 | DD7 / DD12 + runner.rs 行 ⑤（`with_beta` 条件 = anthropic 且后缀在场） |
| AC-10 | DD16 + commands/agents/mod.rs 行 ②③ + DD10（错误串结构上不含 api_key）+ lib.rs 行 ①（同一 `parse_model_spec`） |
| AC-11 | DD19 / DD20 + provider-panel.tsx 行 ①–⑧ + use-agent-providers.ts 行 |
| AC-12 | bindings.ts 行（再生成、非手改）+ 守线任务（`bindings:check` / `server:check` / `client:check` / `run test` 由桌面端执行） |

> **验收面口径**：AC 集合止于单元测试与静态检查链，不含真机冒烟（真实端点 + 凭据不在本工作流 test-execution 相位能力内）。rig 编码差异 / 端点差异无进程内断言面，以静态锚（双臂组装描述面、三档取值链、`[1m]` 逐行表）与守线 diff 承载；DD5（32K 兜底）+ DD6（显式 `max_tokens` 是唯一正解）仍为已知边界的设计口径，实现不新增恢复机制（排除 `maxOutputTokensRecovery`，见「SDK 租户 MVP 边界」#10）。

---

## 决策留痕（边界张力与红线）

1. **`build_model` 第三入参与 spec Module Contract 措辞**（DD3）：spec 该行写作 `build_model(config: &EngineConfig, spec: &ModelSpec)`，但输出上限取值链必须拿到 `provider.max_tokens`，而 `EngineConfig` 被 DD2 / spec 明文禁止承载它。实作定 `build_model(config, spec, max_tokens)`——语义与「协议唯一分叉处」不变，仅补一个位置参数。spec 文本归档时同步；这不改任何 AC 行为（AC-8 的断言面仍是取值链三档）。**不做**的替代方案：把 `max_tokens` 塞进 `ModelSpec`（后缀与输出上限在 Q4 已裁定解耦，塞进去即制造语义混淆）。
2. **`Messages::with_default_max_tokens` 的等价写法**（DD4）：spec 写「定型 wire 默认 `max_tokens`（`Messages::with_default_max_tokens`）」。该方法是消费型 builder（`self -> Self`），而 `Model.wire` 是 pub 字段，直接从 `Model` 里 `model.wire.with_default_max_tokens(cap)` 属非法搬移；实作取 `model.wire.default_max_tokens = Some(cap)`（`Option<u64>` 为 `Copy`）。效果逐字等价（wire `encode` 在 `request.max_tokens.is_none()` 时取 `default_max_tokens`，rig `anthropic/wire.rs:453`），因此 `loop.rs` / `compact.rs` 的 `max_tokens: None` 保持零改动。
3. **空模型名边界**（DD8 ①）：provider 三档模型名目前允许留空保存（齐备性由启动期 `ConfigMissing` 承担），`parse_model_spec("")` 必须 `Ok`，否则本变更会新增「保存期拒收空模型名」的行为面（proposal 未授权）。`"[1m]"`（裸名空 + 后缀）反而 `Err`——那是「方括号语义悬空」，与 `[2m]` / `[]` 同类。
4. **`model_spec_test.rs` 与 `#[cfg(test)] mod` 声明归测试相位**：`sdk/mod.rs` 实现相位只挂载 `pub(crate) mod model_spec;`。避免实现相位产出空测试文件；测试相位补 `#[cfg(test)] mod model_spec_test;` 与逐行覆盖（AC-6 的验证方法）。此为对 proposal「`sdk/mod.rs`：挂载 `model_spec` 模块与单测模块」的相位切分细化，不改文件面归属。
5. **`context.rs` 只读访问器 vs「防线零改」红线**（DD13）：「不要修改」清单约束的是*水位算法 / prune / compaction 语义*。日志需要**生效**窗长（含 128K 缺省），在 runner 侧重算会复制 `DEFAULT_CONTEXT_WINDOW` 与「0 → 缺省」归一化 → 双源漂移。故加 `window()` getter（只读、无分支、不改任何阈值），并在变更清单显式标注「清单外补入」与理由。
6. **`max_tokens` 对 openai 协议静默不生效**（R10）：字段照常记录（保存不拒收，将来可扩展），但 openai 臂 MUST NOT 写请求参数；UI 提示（`provider-max-tokens-hint`）显式声明「仅 anthropic 协议生效」。这是知情边界，不是遗漏。
7. **双枚举不合一**（DD1 / DD15）：store 禁拖引擎门面 crate（零新增依赖边），门面 `Protocol` 需 `Copy` + `Debug` 且不进 IPC，故两侧各一枚、组合根单点映射（沿 `AgentEngineKind → EngineKind` 先例）。协议不进 `EngineKind`（租户语义 vs provider 语义），`runner_for` 唯一 match 点与双值不塌。
8. **跨协议续会话不加一致性校验**（Q3）：`SessionConfigSnapshot` 零改动；改协议后 `Continue` 以新 wire 重编码旧史，rig 只回放本 dialect issuer 的 reasoning 块（thinking 静默丢弃）——边界写入 `desktop-agent-execution`「SDK 租户 MVP 边界」#11，不在本变更补校验。
9. **`[1m]` 对非 1M 模型是纯本地假设**（R4）：后缀只抬本地防线估算与 beta 头，不改真实窗长；错配由 L4（provider 报错收敛）兜底，表单提示与 spec 边界条双留痕。
10. **不做模型元数据表**：无 `default/upper`、无 `supports_1m_suffix` / `supports_1m_beta` 判定能力 → `[1m]` 不白名单校验、beta 一律发（第三方网关拒绝以 `api_error` 显式暴露）。不实现 `maxOutputTokensRecovery`（撞上限即 `finish` 收敛）。

---

## 待决问题

1. **spec 文本归档同步**：决策留痕 #1（`build_model` 第三入参）与 #2（`with_default_max_tokens` 等价写法）需在两份 spec delta 归档进主 spec 时同步措辞；不影响本变更的实现与 AC 行为。
