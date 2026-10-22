# 任务: provider-protocol

> 实现边界以 `design.md` 变更清单为准。**新增覆盖**（`parse_model_spec` 逐行表、双臂组装锚、取值链三档、保存期校验用例、管理页新用例等）由 test-design → test-gen → test-execution 阶段承接，本列表不含测试编写任务；但签名 / 字段面波及既有测试文件的**编译面机械随动**（保编译、不写断言，沿 2026-10-10-unify-run-state-persistence 先例）归各实现阶段。**唯一例外**：test-execution 首轮暴露的 `checks` slot 用例环境敏感修复（proposal「测试装置修复」）属纯装置修复、不计新增用例，单列阶段九。顺序按依赖排列：store 数据面 → 解析纯函数与门面协议枚举 → 引擎组装 → 组合根 → 命令面校验 → bindings 再生成 → 管理页 → 守线。静态脚本（`server:check` / `client:check` / `bindings:check` / `run test`）由桌面端执行，本列表只登记必须满足的静态边界。

## 阶段一：store 数据面（AC-1 / AC-2 / AC-3）

- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：新增 `pub enum AgentProviderProtocol { Openai, Anthropic }`——`#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]`（线值 `"openai"` / `"anthropic"`）+ `Default = Openai`；doc 注明「变体声明序 = bincode 解码锚（`Openai`=0 / `Anthropic`=1），MUST NOT 增删重排」（design 类型定义 / AC-2）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：新增 `pub(crate) struct AgentProviderRecordV2`（现 v2 形态：`id` / `name` / `base_url` / `api_key` / `models` / `context_length`，`#[native_model(id = 5, version = 2, from = AgentProviderRecordV1)]`）；`AgentProviderRecord` 改 `#[native_model(id = 5, version = 3, from = AgentProviderRecordV2)]`，字段面增 `#[serde(default)] pub protocol: AgentProviderProtocol`（置 `name` 之后）与 `#[serde(default)] pub max_tokens: Option<u64>`（置 `context_length` 之后）；`AgentProviderRecordV1` doc 口径改「升级链中转至 V2」（design DD17 / DD18）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：删既有 V1↔V3 直连 `From` 两 impl，改逐级四 impl——`V1 → V2`（补 `context_length = None`）、`V2 → V1`（丢列）、`V2 → V3`（补 `protocol = Openai` / `max_tokens = None`）、`V3 → V2`（丢两列），每个 impl 附「升级补缺 / 降级丢列」doc（design DD18，`ExploreRecord` 多级链先例）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：`AgentProviderRecord::new` 改七参 `(name, protocol, base_url, api_key, models, context_length, max_tokens)`；手写遮蔽 `Debug` 增 `protocol` / `max_tokens` 两位（字段序与 derive 对齐，`api_key` 位仍 `mask_api_key`）；`base_url` 字段注释由「openai 兼容端点」改「协议对应端点」口径（design DD17 / AC-3）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`：`pub use model::{… AgentProviderProtocol …}`（命令层与绑定生成消费）
- [x] 编译面机械随动（保编译、不写断言）：`crates/infra/store/src/model_test.rs`（13 处 `AgentProviderRecord::new` 补 `protocol` / `max_tokens` 实参、V1 往返用例的 `From` 面随逐级链）、`crates/infra/store/src/envelope_test.rs`（1 处）、`crates/infra/store/src/store_test.rs`（2 处）、`crates/infra/agent/src/compose_test.rs`（2 处）、`crates/infra/agent/src/worker_test.rs`（2 处）、`src-tauri/src/commands/change_flow/mod_test.rs`（1 处）、`src-tauri/src/commands/exec/mod_test.rs`（1 处）

## 阶段二：解析纯函数与门面协议枚举（AC-4 配置面 / AC-6）

- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs`（新）：`pub fn parse_model_spec(raw: &str) -> Result<ModelSpec, ModelSpecError>`——`trim` 后末尾 `[1m]`（ASCII 大小写不敏感）→ 剥离出裸名 + `length = Some(MILLION_CONTEXT_WINDOW)`；其余方括号形态（`[2m]` / `[8k]` / `[]` / 内空白 / 非末尾）→ `Err`；无后缀 → `name = trimmed`、`length = None`；空串 / 全空白 → `Ok(name = "", length = None)`（design DD8 / DD9）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs`：`pub(crate) const MILLION_CONTEXT_WINDOW: u64 = 1_000_000`（1000 进制）与 `pub(crate) const BETA_CONTEXT_1M: &str = "context-1m-2025-08-07"`；`pub struct ModelSpec { pub name: String, pub length: Option<u64> }`（`Debug, Clone, PartialEq, Eq`）；`pub struct ModelSpecError { pub raw: String, pub reason: String }`（`Debug, Clone, PartialEq, Eq`）+ `impl Display`（`模型名后缀非法: {raw}（{reason}）`，结构上不含 api_key）（design DD7 / DD10）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`：挂载 `pub(crate) mod model_spec;`（字母序插在 `log` 之后）；`#[cfg(test)] mod model_spec_test;` 与测试文件由测试相位一体落（design 决策留痕 #4）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`：`pub use sdk::model_spec::{parse_model_spec, ModelSpec, ModelSpecError};`（命令层保存期校验复用同一函数，不造第二套规则）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs`：新增 `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] pub enum Protocol { #[default] Openai, Anthropic }` + `pub fn as_str(self) -> &'static str`（`"openai"` / `"anthropic"`，日志用）；`EngineConfig` 增 `pub protocol: Protocol`（置首）；`empty()` 给 `Protocol::Openai`；`is_complete()` 实现零改、doc 改「连接配置字段面齐备检查（协议恒有值不参与校验）」；模块 doc 补「可空调参 `context_length` / `max_tokens` 走门面旁路」（design DD1 / DD2）
- [x] 编译面机械随动：`crates/infra/agent/src/sdk/config_test.rs`（6 处 `EngineConfig` 字面量）、`crates/infra/agent/src/sdk/runner_test.rs`（7 处，含 `complete_config()` 底座）、`crates/infra/agent/src/lib_test.rs`（4 处）补 `protocol: Protocol::Openai`（保既有断言语义）

## 阶段三：引擎组装双协议与旁路载荷（AC-5 / AC-7 / AC-8 / AC-9）

- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`：`use rig::providers::anthropic::AnthropicConfig;`；增常量 `OUTPUT_TOKENS_FALLBACK: u64 = 32_000`（私有）与 `pub const MIN_OUTPUT_TOKENS: u64 = 4_096`（design DD5 / DD6）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`：`SdkRunner` 增私有 `max_tokens: Option<u64>` 字段 + `pub fn with_max_tokens(self, max_tokens: Option<u64>) -> Self`；`new` 四参签名不动（内给 `max_tokens: None`，36 处调用点零改）（design DD14）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`：`open_session` 按档序——既有配置齐备校验 → `parse_model_spec(&raw)`（`Err` → `AgentStartError::ConfigMissing`，文案含档位与 `ModelSpecError`）→ `resolve_resume` → `build_model(&self.config, &spec, self.max_tokens)` → `ContextDefense::resolve(spec.length.or(self.context_window))`；`LoopTurn.model_name` 用 `spec.name`（裸名上报）（design DD11 / DD12）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`：`build_model(config: &EngineConfig, spec: &ModelSpec, max_tokens: Option<u64>) -> DynModel<Completion>` 双臂——openai 臂 `OpenAIConfig::new(..).with_base_url(..).client().chat(spec.name).erase()`（逐字等价现状、不追加任何请求参数）；anthropic 臂 `AnthropicConfig::new(..).with_base_url(..)`[`spec.length == Some(MILLION_CONTEXT_WINDOW)` 时 `.with_beta(BETA_CONTEXT_1M)`]`.client().completion(spec.name)`，再 `let cap = max_tokens.or(model.wire.default_max_tokens).unwrap_or(OUTPUT_TOKENS_FALLBACK); model.wire.default_max_tokens = Some(cap);` 后 `erase()`（design DD3 / DD4 / DD5 / DD7）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`：`PumpPayload` 增 `protocol: Protocol` / `raw_model: String`；泵启动日志改 `泵启动 session={} protocol={} model={裸名} window={defense.window()} raw={原文}`（`loop.rs` 零改动）（design DD12）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs`：增 `pub(crate) fn window(&self) -> u64` 只读访问器（返回生效窗长；水位比 / 保护窗 / prune 门槛 / prune 与 compaction 算法零改——design DD13）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`：`EngineFacade` 增 `max_tokens: Option<u64>` 字段（`new` / `with_resume_transcript` / `with_explore_note_store` 三处初始化补 `None`）+ `pub fn with_max_tokens(self, max_tokens: Option<u64>) -> Self`（`with_context_window` 同型透传 `max_tokens: self.max_tokens`）；`runner_for` sdk 臂改 `SdkRunner::new(..).with_max_tokens(self.max_tokens)`（签名与唯一 match 点结构不变）；`pub use sdk::runner::MIN_OUTPUT_TOKENS;`；`EngineFacade` / `context_window` / `runner_for` 的「`EngineConfig` 三字段」措辞改「连接配置字段面」

## 阶段四：组合根解析（协议透传与 `max_tokens` 旁路）

- [x] `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`：新增私有 `fn map_protocol(protocol: AgentProviderProtocol) -> Protocol`（两臂 match）；`ResolvedEngine` 增 `max_tokens: Option<u64>`（与 `context_window` 并列，注释标「可空调参旁路」）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`：`resolve_agent_engine` 的 sdk 臂 `EngineConfig { protocol: map_protocol(provider.protocol), api_key, base_url, model_high, model_low }` + `max_tokens: provider.max_tokens`（解析语义——默认 / 显式 / high 档 / cli 拒收——零变，design DD15）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`：`compose_turn` 的 runner 构造链插 `.with_max_tokens(resolved.max_tokens)`；`use store::{… AgentProviderProtocol …}`；字段注释「`context_length` 列旁路」改「可空调参旁路（窗长 / 输出上限）」

## 阶段五：命令面保存期校验（AC-10）

- [x] `packages/desktop/src-tauri/src/commands/agents/mod.rs`：`use agent_runtime::{parse_model_spec, MIN_OUTPUT_TOKENS};`（crate 已依赖 `agent-runtime`，Cargo.toml 零改动）
- [x] `packages/desktop/src-tauri/src/commands/agents/mod.rs`：`save_agent_provider` 改八参 `(id, name, protocol, base_url, api_key, models, context_length, max_tokens)`；校验按序——①三档模型名 `high → medium → low` 逐个 `parse_model_spec`（首个 `Err` 返回，文案含档位与错误原文、不含 api_key）②`max_tokens` 为 `Some(n)` 且 `n < MIN_OUTPUT_TOKENS` → `Err`（文案含下限 4096）③既有留空 key 回填 ④`upsert_agent_provider`（design DD16）
- [x] `packages/desktop/src-tauri/src/commands/agents/mod.rs`：doc 注释补「协议紧跟 `name`（协议决定 `base_url` 语义）」「保存期校验面」「错误串不含 api_key」口径；`list` / `delete` 命令零改动
- [x] 编译面机械随动：`src-tauri/src/commands/agents/mod_test.rs` 既有 `save_agent_provider` 调用点补 `protocol` / `max_tokens` 实参（新增校验用例归测试相位）

## 阶段六：IPC 绑定再生成（AC-12 前置）

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`（非手改）：预期带出 `AgentProviderProtocol` 枚举型、`AgentProviderRecord` 的 `protocol` / `maxTokens` 字段、`saveAgentProvider` 八参新序（`id, name, protocol, baseUrl, apiKey, models, contextLength, maxTokens`）

## 阶段七：管理页（AC-11）

- [x] `packages/desktop/src/views/agents/hooks/use-agent-providers.ts`：`AgentProviderSaveInput` 增 `protocol: AgentProviderProtocol` / `maxTokens: number | null`；`commands.saveAgentProvider(...)` 实参按 `(id, name, protocol, baseUrl, apiKey, models, contextLength, maxTokens)`；doc 注释补「`maxTokens` 留空 = 按协议缺省存 `null`」（design DD20）
- [x] `packages/desktop/src/views/agents/components/provider-panel.tsx`：`ProviderFormState` 增 `protocol: AgentProviderProtocol`（`EMPTY_FORM` = `'openai'`）/ `maxTokens: string`；`toFormState` 回填两字段（`maxTokens == null` → `''`）；`parseMaxTokens(value): number | null | undefined`（空 = `null`、非整数或 `< 4096` = `undefined`）并接入 `canSave` 与 `submit` 实参（design DD19 / DD20）
- [x] `packages/desktop/src/views/agents/components/provider-panel.tsx`：新增 `ProtocolSelect` 子组件（`StandardSelect`，`items = [{value:'openai'},{value:'anthropic'}]`，`onChange` 收到 `null` 保持现值）并在 row 0 前置渲染；`maxTokens` 进 `PROVIDER_FIELDS`（row 2，与 `contextLength` 相邻，placeholder「留空 = 按协议缺省」）
- [x] `packages/desktop/src/views/agents/components/provider-panel.tsx`：新增 `BASE_URL_HINTS: Record<AgentProviderProtocol, { placeholder: string; hint: string }>` 常量（openai `https://…/v1`；anthropic `https://api.anthropic.com` + 「`/v1` 等后缀由引擎自动剥离后拼 `/v1/messages`」）；`base_url` placeholder 与提示随 `form.protocol` 切换；提示区三锚 `provider-base-url-hint` / `provider-max-tokens-hint`（声明「仅 anthropic 协议生效」+ 下限 4096）/ `provider-suffix-hint`（模型名可带 `[1m]`；某档带后缀且 `contextLength` 非空时追加「`<档位>` 档窗长已被模型名覆盖」）
- [x] `packages/desktop/src/views/agents/components/provider-panel.tsx`：`ProviderList` 行增协议徽标 `provider-protocol-badge`（`record.protocol` 文本）；模块 / 组件 doc 口径更新（清单 → 协议徽标与 `max_tokens` 边界）
- [x] 编译面机械随动（保 tsc / 保绿）：`src/views/agents/agents-view.test.tsx`、`src/views/agents/components/agent-panel.test.tsx`、`src/views/agents/components/provider-panel.test.tsx`、`src/views/agents/hooks/use-agent-providers.test.ts` 的 `AgentProviderRecord` fixture 补 `protocol` / `maxTokens`，`save` 入参断言面随新序随动（新增用例归测试相位）

## 阶段八：守线（静态边界自查，AC-2 / AC-4 / AC-5 / AC-7 / AC-8 / AC-12）

- [x] 零 diff 边界核验：`git diff --exit-code -- packages/desktop/src-tauri/crates/core/agent`（core 零 diff，`AgentStartError` 变体集不变）；`sdk/loop.rs` / `sdk/normalize.rs` / `sdk/resume.rs` / `sdk/compact.rs` 零 diff（`max_tokens: None` 与协议中立性不变）；`store` 的 `SessionRecord` / `SessionConfigSnapshot` / `store.rs` 零 diff（Q3 不加协议字段、模型组注册面不变）；前端 `agent-adapter.ts` / `use-agent-session.ts` / `agent-debug-view.tsx` / `agent-run-form.tsx` / `masked-api-key.tsx` 零 diff
- [x] 结构面目视核验：`EngineKind` 双值与 `runner_for` 唯一 match 点结构不塌、`Protocol` 未进 `EngineKind`、`EngineConfig` 无 `max_tokens` / `context_length` 字段、`AgentProviderRecordV2` 为 `pub(crate)` 且未进 `global_models()`
- [x] 残留 grep 自查（产品代码零命中）：`OpenAIConfig` 仅在 `sdk/runner.rs` 的 openai 臂；`with_beta` / `AnthropicConfig` 仅 anthropic 臂；`4096` / `32_000` 字面量只在常量定义处出现（无第二套门槛）；`[1m]` 解析逻辑只在 `model_spec.rs`
- [x] 静态检查链（由桌面端执行，本相位登记）：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy）、`run client:check`（vp check + knip 零未用导出）、`run bindings:check`（再生成后零新增 diff）
- [x] 变更清单双向核对：`design.md` 变更清单与实际触达文件一致（12 个 proposal 实现文件全覆盖 + 清单外补入 `sdk/context.rs` 一行 + 编译面随动行；清单外零改动、清单内零遗漏）

## 阶段九：测试装置修复（test-execution 首轮暴露，非功能面）

- [x] `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs`：`run_slot_baseline_pass_报告与exec_outputs落baseline树` / `run_slot_baseline_fail_report_dir仍回填baseline锚` / `run槽位等价性_语义同锚点异且err面持衡` 三处各补 `let _path = PathPrefix::enter(ws.path());`（shim 裸名经 PATH 解析——宿主 `NoDefaultCurrentDirectoryInExePath=1` 时 `cmd` 不搜 cwd，裸名 spawn 期报错；同 crate 其余 shim 用例均持该窗口）（proposal「测试装置修复」/ design「测试装置修复」表）
- [x] `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs`：`run槽位等价性_…` 增非空转断言（两槽位 `diagnostics` 均含 shim 输出）——原形态下「双不可达」时 `passed` / `diagnostics` 亦等值，等价性会假绿通过
- [x] 守线：产品代码 `crates/infra/checks/src/static_check.rs` 零 diff（`ensure_program_resolvable` / `execute` 不动）；复跑 `cargo test --tests --workspace` 全绿（2294 passed，其中 `checks-runtime` 149/149）——首轮 `total=739 / failed=57` 中的 56 条为持锁 panic 毒化级联，随唯一真实失败消解
