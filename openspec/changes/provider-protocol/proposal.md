# 提案: provider-protocol

> **变更**: provider-protocol
> **日期**: 2026-10-21
> **状态**: 待评估（explore 决策表 Q1–Q5b 已全部锁定，无待确认项；本提案仅对一处 explore 遗留口径作裁定，见「风险 R9」）
> **来源**: `explore.md`（requirement 类型；rig 0.43.0 源码与 Claude Code 二进制均已实测核对）

---

## 问题

桌面端 agent provider 目前**只有一套「openai 兼容」形状**：记录字段（`base_url` / `api_key` / 三档 `models` / 可空 `context_length`）与引擎组装点同时假设 openai wire，用户无法接入 anthropic 协议的模型。

现状链路（已核对源码）：

```
AgentProviderRecord{ id, name, base_url, api_key, models{high,medium,low}, context_length? }
      │ find_agent_provider
      ▼
compose::resolve_agent_engine            ← 解析单点（compose.rs:208-262）
  ResolvedEngine{ kind, config: EngineConfig, context_window, agent_instance_id }
      │ EngineFacade::runner_for(kind, config)
      ▼
sdk::runner::build_model(config, model_name)     ← 唯一模型组装点（runner.rs:161-167）
      ▼
rig OpenAIConfig(base_url).client().chat(model).erase()
```

五处具体缺陷：

1. **协议不可选**：`sdk/runner.rs` 是唯一 `use rig::providers::openai::OpenAIConfig;` 处，全仓无 `AnthropicConfig` 引用；anthropic 协议端点（官方 Claude / anthropic 兼容网关）配置后在请求期被 openai wire 发出去，形态性失败。
2. **anthropic `max_tokens` 必填无字段可表达**：rig 的 anthropic wire 在请求 `max_tokens` 缺席时按内置模型名单给默认值（`claude-opus-4*` / `claude-sonnet-4*` / `claude-haiku-4-5` → 64K，少数 5 代 → 128K），**名单外模型直接 `EncodeError("max_tokens must be set for Anthropic")`**。provider 记录没有任何输出上限字段，用户遇到老代模型（`claude-3-5-sonnet-*`）或第三方 anthropic 网关（8K/16K 上限）时「配了就跑不起来」。
3. **`context_length` 顶不上输出上限**（且误用有静默副作用）：`context_length` 只喂上下文防线（`ContextDefense`：L2 75% / L3 90% / 保护窗 40k），请求输出上限恒为 `None`（`loop.rs:183`、`compact.rs:48`）。用户「填 8000 表达输出 8K」→ 请求仍发 32K/64K → 依旧 400，同时把 L2/L3 水位压到 6K/7.2K、保护窗 40k 反超整窗 → **上下文防线静默失效**。
4. **长上下文模型无法表达**：模型名按原样发 API，缺少 Claude Code 的 `<模型名>[1m]` 配置形态，1M 上下文模型无法在本地防线上被正确估算。
5. **两处 spec 硬承诺与本需求直接冲突**：`desktop-agent-management`「`EngineConfig` 三字段结构 MUST NOT 变化」、`desktop-agent-execution`「provider 首公民 SHALL 为 openai-compatible 端点；**anthropic provider 降为后续可选**」——本变更正是偿还后者，因此前者必须同批显式修订，否则实现必然违约。

## 提案

以「**provider 级协议 + provider 级输出上限 + 模型名 `[1m]` 后缀 + SDK 引擎双协议组装**」四面收口，一次 DB 版本演进（V2 → V3）把字段面定死：

1. **store 字段面（V2 → V3 一次演进）**：`AgentProviderRecord` 增
   - `protocol: AgentProviderProtocol`（线值 `openai` | `anthropic`，`#[serde(default)]` = `openai`；bincode 序号为解码锚 MUST NOT 重排）——存量 v2 行升级即 `openai`，行为零变化；
   - `max_tokens: Option<u64>`（输出上限，等价 Claude Code 的 `CLAUDE_CODE_MAX_OUTPUT_TOKENS`；`None` = 按协议缺省；`< 4096` 保存期拒收）；
   历史形态 `AgentProviderRecordV2` 作升级链中转（沿 `ExploreRecord` / `ChangeRecord` 多级链先例：V1 → V2 → V3 逐级 `From`）。
2. **协议抵达引擎（explore 方案 A）**：门面侧独立 `Protocol { Openai, Anthropic }` 枚举（沿 `AgentEngineKind` → `EngineKind` 双枚举映射先例），`EngineConfig` 增 `protocol` 字段；`sdk/runner::build_model` 变双臂——openai 臂保持逐字现状，anthropic 臂 `AnthropicConfig::new(key).with_base_url(url)[.with_beta(..)].client().completion(model)` 并定型 wire 默认 `max_tokens`，两臂同产 `DynModel<Completion>` → **loop / normalize / resume / core 契约零改动**（协议差异对 loop 不可见）。
3. **`[1m]` 后缀与输出上限定档（对齐 Claude Code）**：新增纯函数 `parse_model_spec(raw)`——末尾 `[1m]`（大小写不敏感）剥离出裸名并给出 `1_000_000`，其他方括号形态显式拒收（`[2m]` / `[200k]` 不做）；窗长优先级 **后缀 > `context_length` > 128K 缺省**，按档在 `open_session` 现算（`EngineFacade::with_context_window` 签名不变，语义降为「缺省来源」）；anthropic + 后缀在场 → 发 `anthropic-beta: context-1m-2025-08-07`；anthropic 输出上限取值链 **`provider.max_tokens` → rig 官方名单值 → 32_000 兜底**；openai 协议 MUST NOT 写 `max_tokens`（各网关超限行为不一）。
4. **命令面与管理页**：`save_agent_provider` 增 `protocol` / `max_tokens` 入参（协议紧跟 `name`——协议决定 `base_url` 语义，签名上显式）；保存期以引擎侧**同一个** `parse_model_spec` 校验三档模型名 + `max_tokens ≥ 4096`，非法即 `Err`（避免「保存成功但跑不起来」）；管理页表单增协议选择器（`StandardSelect`）、`max_tokens` 数字字段、`base_url` 提示随协议切换、列表协议徽标、后缀提示。

决策表（explore §14 全部锁定，本提案照抄执行）：

| 编号 | 决策 | 值 |
|---|---|---|
| Q1 | 协议归属 | provider 级（agent 实例零改动） |
| Q2 | 协议抵达引擎 | 方案 A：`EngineConfig` 增字段 + 门面 `Protocol` 枚举映射 |
| Q3 | 跨协议续会话 | 允许，不加一致性校验（`SessionConfigSnapshot` 不动） |
| Q4 | `[1m]` 语义 | 只改上下文窗长 + anthropic beta，**不改输出上限** |
| Q4b | 单位进制 | 1000（`1m = 1_000_000`） |
| Q4c | `[1m]` 是否发 beta | 发（`context-1m-2025-08-07`） |
| Q4d | 输出上限来源 | provider 新增可空 `max_tokens` |
| Q4e | 名单外兜底 | `32_000`（对齐 Claude Code 表外行为） |
| Q4f | `max_tokens` 下限 | `< 4096` 拒收 |
| Q5 | 协议视觉标识 | 做（列表徽标 + 表单选择器 + `base_url` 提示切换） |
| Q5b | `[2m]` | 不做，显式拒收 |

**明确不做**（均为知情边界，非遗漏）：

- 跨协议续会话一致性校验（Q3）：改协议后 `Continue` 以新 wire 重编码旧史，rig 的 `replayable_to` 只回放本 dialect issuer 的 reasoning 块——**thinking 静默丢弃**可接受；system 消息不进持久化史（`resume::rebuild` 过滤 + 每轮头部单独注入），故无残留 system 形态问题。
- `maxOutputTokensRecovery` 等价机制（Claude Code 撞上限后的续写注入）：撞上限即以既有 `finish` 语义收敛，用户调 `max_tokens` 后重跑。
- 模型元数据表：无 `default/upper`、无 `supports_1m_suffix` / `supports_1m_beta` 判定能力，故 `[1m]` 不做白名单校验、beta 一律发（第三方网关拒绝以 `api_error` 显式暴露，非静默）。
- openai 协议写 `max_tokens`（保留「省略由协议自决」现状）。
- 由 `context_length` 派生输出上限（语义混淆且埋雷，见「问题 3」）。

## 能力

### 新增的能力

无。协议选臂、`[1m]` 后缀解析与输出上限定档均落在既有 `desktop-agent-execution` 的 SDK 引擎面内，不引入新能力（不新建 `desktop-agent-protocol` 之类的能力边界——协议是 provider 的字段语义 + 既有引擎的组装分支，拆独立能力只会造出跨两能力的悬空契约）。

### 修改的能力

- **desktop-agent-management** —
  - **MODIFIED**「全局 agent 管理命令面」：provider 字段面增 `protocol`（缺省 `openai`）与可空 `max_tokens`（`< 4096` 拒收）；三档模型名可带 `[1m]` 后缀，保存期以引擎侧同一 parse 校验、非法后缀 `Err`；原「`context_length` 消费接线 MUST NOT 经 `EngineConfig`（三字段结构不动承诺维持）」改写为「**连接配置字段面**」——协议属连接配置核心项进 `EngineConfig`，可空调参（`context_length` / `max_tokens`）仍走门面旁路。
  - **MODIFIED**「默认 agent 与运行发起解析」：解析产物由 `EngineConfig{api_key, base_url, model}` 改为携 `protocol` 的连接配置字段面；`AgentProviderProtocol` → 门面 `Protocol` 双枚举映射；可空调参与模型名原文经 `ResolvedEngine` 旁路携带（窗长 / 上限 / 后缀解析全在引擎侧单点）。
  - **MODIFIED**「Agent 管理页」：Providers 表单增协议选择器与 `max_tokens` 字段、`base_url` 提示随协议切换、列表协议徽标、模型名后缀提示（含「后缀覆盖 `context_length`」提示）。
- **desktop-agent-execution** —
  - **ADDED**「SDK 引擎模型协议选臂（openai / anthropic）」：双臂组装、同产 `DynModel<Completion>`、`base_url` 语义差异（anthropic 自动剥 `/v1`）、认证头内建、泵启动日志留 `protocol=`。
  - **ADDED**「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」：parse 规则表、按档现算窗长、beta 条件发、anthropic 输出上限三档取值链、openai 保持省略、上报裸名。
  - **MODIFIED**「引擎门面与 EngineKind 参数选择」：`EngineConfig` 形态由「api_key / base_url / model 三件套」修订为「连接配置字段面（含 `protocol`）」；`runner_for` 唯一 match 点与 `EngineKind` 双值仍不塌（协议住配置、不进 kind）。
  - **MODIFIED**「SDK 引擎（rig 进程内租户）」：删「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」，改为双协议可选（本需求的偿还对象）；事件归一化条不再假设 openai 形状独有（rig 抽象层已抹平）。
  - **MODIFIED**「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」：窗长来源由「`context_length` 缺席 128K」改为「模型名后缀 > `context_length` > 128K」，且按档解析。
  - **MODIFIED**「SDK 租户 MVP 边界与已知限制」：增三条边界（无模型元数据表 / 不实现 `maxOutputTokensRecovery` / 跨协议续会话不做一致性校验）。

其余能力零改动，理由：

- **desktop-agent-chat-infra**：前端事件适配只消费 `AgentEvent` 双层词汇（delta 拼装 / 密封替换），协议差异被 rig 与 `normalize` 完全吸收——anthropic wire 的流事件经同一 rig 抽象到达，适配层零改。
- **desktop-workspace-store**：`SessionConfigSnapshot` 字段面不动（Q3 裁决：无协议一致性字段），会话转录单表与 `resume::rebuild` 零改。
- **desktop-ipc-type-bindings**：绑定生成机制零改，产物经 `pnpm run bindings:export` 再生成（新增枚举与字段自动带出）。
- **desktop-standard-components**：协议选择器复用既有 `StandardSelect`，不新增标准组件。
- **desktop-crate-layout / module-boundary-contracts**：不新增 crate、不新增跨 crate 依赖边（命令层已依赖 `agent-runtime`，复用其 parse 导出即可）。
- **crates/core/agent**：协议是 provider 属性、`EngineConfig` 属 infra 门面，core 侧零改动（`AgentStartError` 变体集不变，启动失败仍走 `ConfigMissing`）；`runner_for` 签名形态不变。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 新增 `AgentProviderProtocol { Openai = 0, Anthropic = 1 }`（`serde` camelCase 线值 `openai`/`anthropic` + `Type` + `Default = Openai`；bincode 序号为解码锚 MUST NOT 重排）；新增 `AgentProviderRecordV2`（现 v2 形态：`#[native_model(id = 5, version = 2, from = AgentProviderRecordV1)]` + 自有 `From<V1>`）；`AgentProviderRecord` 升 `version = 3, from = AgentProviderRecordV2`，增 `#[serde(default)] protocol` 与 `#[serde(default)] max_tokens: Option<u64>`；双向 `From`（升级补 `Openai` / `None`，降级丢列，沿 V1→V2 先例）；手写遮蔽 `Debug` 增 `protocol` / `max_tokens` 位；`new(...)` 增两参 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | 再导出 `AgentProviderProtocol`（命令层与绑定生成消费） |
| `packages/desktop/src-tauri/src/commands/agents/mod.rs` | `save_agent_provider` 增 `protocol` / `max_tokens` 入参（协议紧跟 `name`）；保存期校验：三档模型名经 `agent_runtime::parse_model_spec`（非法后缀 `Err`）+ `max_tokens` 存在时 `>= 4096`（否则 `Err`）；留空 key 回填原值逻辑不动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs`（新） | 纯函数 `parse(raw) -> Result<ModelSpec{name, length}, ModelSpecError>`：末尾 `[1m]`（大小写不敏感、两端空白裁剪）→ 裸名 + `1_000_000`；其余方括号形态（`[2m]` / `[8k]` / `[]` / 非末尾）→ `Err`；无后缀 → `name = trimmed`、`length = None` |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 挂载 `model_spec` 模块与单测模块 |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 公开导出 `parse_model_spec` / `ModelSpec` / `ModelSpecError`（命令层保存期校验复用同一函数，不造第二套规则）；门面增 `with_max_tokens(Option<u64>)` builder |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` | 门面 `Protocol { Openai, Anthropic }`；`EngineConfig` 增 `protocol: Protocol`（`empty()` 给 `Openai`）；`is_complete` 语义不变（协议恒有值，不参与校验） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `SdkRunner` 增 `max_tokens: Option<u64>` 字段 + `with_max_tokens(self, ...)` builder（`new` 签名不动——36 处调用点零改）；`open_session` 按档 `parse_model_spec(raw)`（`Err` → `ConfigMissing` 显式启动失败）、窗长改按档现算 `spec.length.or(self.context_window)`；`build_model` 双臂（anthropic 臂条件设 wire 默认 `max_tokens`、条件 `with_beta`）；`append_engine_log` 泵启动行加 `protocol=… model=裸名 window=… raw=原文` |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | `resolve_agent_engine` 的 sdk 臂：`provider.protocol → Protocol` 映射进 `EngineConfig`；`provider.max_tokens` 与 `context_length` 同型经 `ResolvedEngine` 旁路、由门面 builder 下传；`ResolvedEngine` 字段面增 `max_tokens`（与 `context_window` 并列） |
| `packages/desktop/src/types/generated/bindings.ts` | `pnpm run bindings:export` 再生成（非手改）：`AgentProviderProtocol` 枚举 + 记录 `protocol` / `maxTokens` + `saveAgentProvider` 新参数序 |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | `AgentProviderSaveInput` 增 `protocol` / `maxTokens`；`commands.saveAgentProvider(...)` 实参按新序 |
| `packages/desktop/src/views/agents/components/provider-panel.tsx` | `ProviderFormState` 增 `protocol` / `maxTokens`（新建缺省 `'openai'` / 空串 = 未配置）；协议选择器用 `StandardSelect`；`base_url` placeholder 与提示随协议切换；`maxTokens` 边界 parse（空 = `null`、`< 4096` 非法拒绝提交）；列表行协议徽标；后缀提示与「后缀覆盖 `context_length`」提示 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| v2 → v3 升级补 `openai` / `None`；v1 → v3 两级链仍可解；降级丢列；版本头 `3`；线值 `"openai"` / `"anthropic"`；`max_tokens` 往返 | `crates/infra/store/src/model_test.rs` | 新增 + 既有 `new(...)` 调用点（13 处）补参 |
| 升级链与信封往返（v1/v2 存量行） | `crates/infra/store/src/envelope_test.rs`（1 处）、`store_test.rs`（2 处） | 补参 + 新增升级锚 |
| 遮蔽 `Debug` 含 `protocol` / `max_tokens` 位且 `api_key` 无明文 | `crates/infra/store/src/model_test.rs` | 既有断言扩位 |
| 协议透传（provider `anthropic` → `EngineConfig.protocol = Anthropic`）；`max_tokens` 旁路携带 | `crates/infra/agent/src/compose.rs`（1 处 `EngineConfig` 字面量）+ `compose_test.rs`（2 处 `AgentProviderRecord::new` + 3 处 `resolve_agent_engine` 用例） | 补字段 + 新增断言 |
| `EngineConfig` 字面量补 `protocol`（4 处字面量 + `EngineConfig::empty()` 调用点无需改）；门面 `with_max_tokens` | `crates/infra/agent/src/lib_test.rs` | 补字段（缺省用 `Protocol::Openai`） |
| `empty()` / `is_complete` 语义不变 | `crates/infra/agent/src/sdk/config_test.rs`（6 处字面量） | 补字段 |
| 双臂组装：openai 臂 `DynModel::name() == "openai"`、anthropic 臂 `== "anthropic"` 且 `id()` = 裸名；anthropic `base_url` 带 `/v1` 不产生重复版本段；输出上限链（provider 值优先 / 名单内保留官方值 / 名单外 32K）；openai 臂不写 `max_tokens` | `crates/infra/agent/src/sdk/runner_test.rs`（7 处字面量 + 36 处 `SdkRunner::new` 调用点零改） | 补字段 + 新增双臂锚（不打网络：只断言 `DynModel` 描述面） |
| `parse_model_spec` 全形态表（`[1m]` / 顶层大小写 / `[2m]` 拒 / `[8k]` 拒 / `[]` 拒 / 非末尾拒 / 无后缀 / 空白裁剪） | `crates/infra/agent/src/sdk/model_spec_test.rs`（新） | 新建，逐行覆盖 |
| 按档窗长：`high: x[1m]` + `low: y` → 两档各 1M / `context_length` | `crates/infra/agent/src/sdk/runner_test.rs` + `context_test.rs` | 新增锚 |
| 保存期校验：非法后缀 `Err`、`max_tokens < 4096` 拒收、留空 `None`、错误串不含 api_key | `src/commands/agents/mod_test.rs` | 新增用例 |
| `saveAgentProvider` 实参序（含 `protocol` / `maxTokens`） | `src/views/agents/hooks/use-agent-providers.test.ts` | 断言随动 |
| 表单默认协议 `openai`、切协议后 `base_url` placeholder 变化、`maxTokens` 边界、协议徽标、保存入参 | `src/views/agents/components/provider-panel.test.tsx` | 新增用例 |
| fixture 补 `protocol` / `maxTokens` | `src/views/agents/agents-view.test.tsx`、`src/views/agents/components/agent-panel.test.tsx` | 补字段 |
| 变更 fixture 补字段 | `src/commands/change_flow/mod_test.rs`（1 处）、`src/commands/exec/mod_test.rs`（1 处）、`crates/infra/agent/src/worker_test.rs`（2 处） | 补字段 |
| 装置修复（非功能）：`checks` crate 既有 slot 用例的 shim 可达性 | `crates/infra/checks/src/static_check_test.rs`（3 处） | 补 `PathPrefix::enter(ws.path())` + 等价性用例补「真执行」断言，见「测试装置修复」 |

### 测试装置修复（非功能改动，test-execution 首轮暴露）

test-execution 首轮 `conclusion=fail` 的唯一真实失败为 `static_check_test::run_slot_baseline_fail_report_dir仍回填baseline锚`，其余 56 条为它在持 crate 级 `TEST_ENV_LOCK` 期间 panic 导致的 Mutex 毒化级联（`crate 环境锁不可中毒: PoisonError`；13 条 `static_check_test` + 43 条 `testexec` 连坐），并使 `cargo test --workspace` 在 `checks-runtime` 首个失败二进制处提前中止（首轮 `total=739`，基线 2219 → 后续 suite 未执行）。

根因与本次需求无关，属既有用例的环境敏感缺陷：`run_slot_baseline_pass_…` / `run_slot_baseline_fail_…` / `run槽位等价性_…` 用 `write_shim` 在临时 workspace 根落 `.cmd` 并以**裸名**登记 config，但未把该目录前置到 PATH——依赖 `cmd` 的 cwd 搜索。宿主环境置 `NoDefaultCurrentDirectoryInExePath=1` 时 cmd 不搜 cwd → `'baseline-lint.cmd' is not recognized…`（`ensure_program_resolvable` 因 Windows 显式查 cwd 而放行，spawn 期才炸）。同 crate 其余 shim 用例均持 `PathPrefix` 窗口（注释即写明「不依赖 cmd 的 cwd 搜索」），这三处漏挂。

处置边界：

- **只改测试装置，产品代码零改动**：`crates/infra/checks/src/static_check.rs`（`ensure_program_resolvable` / `execute`）不动——生产行为正确，问题在用例假设。
- 三处补 `let _path = PathPrefix::enter(ws.path());`（与其他 shim 用例同装置，环境免疫）。
- `run槽位等价性_…` 另补一条非空转断言（`diagnostics` 含 shim 输出）：原形态下两槽位「均不可达」时 `passed` / `diagnostics` 也等值，等价性会在双失败上空转通过（假绿）。
- 复跑口径：`cargo test --tests --workspace` 全绿（2294 passed，含本变更新增用例），`checks-runtime` 149/149。

### 不要修改

- **`crates/core/agent`**（全部）：`AgentEvent` 词汇、`AgentRunner` trait、`SessionInjections`、状态机、`AgentStartError` 变体集零改动；协议选择不进 core。
- **`sdk/loop.rs` / `normalize.rs` / `resume.rs`**：loop 保持协议中立（`max_tokens` 仍置 `None`，anthropic 侧由 wire 默认值兜底），normalize 的 openai fixture 只是测试形状不动，resume 重建口径不动。
- **`sdk/context.rs` / `compact.rs`**：防线水位算法、prune / compaction 语义零改（只换窗长来源，且来源在 `open_session` 解析后照旧传入）。
- **`store` 的 `SessionRecord` / `SessionConfigSnapshot`**：Q3 裁决不加协议一致性字段。
- **前端 `agent-adapter.ts` / `use-agent-session.ts` / `agent-debug-view.tsx` / `agent-run-form.tsx`**：事件适配、会话状态、调试页参数面零改。
- **`MaskedApiKey` 组件**：`sk-***abc` 前缀对 anthropic key（`sk-ant-…`）同样成立，不改。
- **`AgentProviderRecord` 的 api_key 机密边界**：明文存储 / 传输、仅展示层遮蔽的边界表不动，仅在其 Debug impl 增两位。
- **`crates/infra/checks/src/static_check.rs`（产品代码）**：`ensure_program_resolvable` / `execute` 零改动——首轮 slot 红是**用例**漏挂 PATH 窗口，非生产逻辑缺陷；只动同目录 `static_check_test.rs`。
- **`openspec/changes/provider-protocol/explore.md`**：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | store V2→V3 一次演进完成：新记录带 `protocol` / `max_tokens`；v2 存量行升级后 `protocol = openai`、`max_tokens = None`（行为零变化）；v1 存量行经 V1→V2→V3 两级链仍可读；版本头为 3；线值为 `"openai"` / `"anthropic"` | `model_test` / `envelope_test` / `store_test` 新增与既有升级锚 |
| AC-2 | `AgentProviderProtocol` 变体序号 `Openai = 0` / `Anthropic = 1` 且无重排；`AgentProviderRecordV2` 仅作升级链中转、不注册进库模型组、不出公共查询面 | 源码复核 + `lib.rs` 导出面 diff |
| AC-3 | 遮蔽 `Debug` 输出含 `protocol` / `max_tokens` 位、`api_key` 位为 `sk-***abc` 形态且无明文；`PartialEq` 比较行为不受影响 | `model_test` 遮蔽锚扩位 |
| AC-4 | `EngineConfig` 携 `protocol`（门面 `Protocol`），`is_complete` 语义不变（协议恒有值不参与校验）；`runner_for` 唯一 match 点与 `EngineKind` 双值不塌 | `config_test` + `lib_test` + 源码复核 |
| AC-5 | SDK 引擎双臂组装：openai 臂产物与现状逐字等价（`DynModel::name() == "openai"`、`id()` = 模型名）；anthropic 臂 `name() == "anthropic"`、`id()` = 裸名；两臂同型 `DynModel<Completion>`（loop 零改动可消费） | `runner_test` 双臂锚（只断言描述面，不打网络） |
| AC-6 | `parse_model_spec` 覆盖规则表全形态：`[1m]` 剥离且 `length = 1_000_000`、大小写不敏感、两端空白裁剪；`[2m]` / `[8k]` / `[]` / 非末尾方括号 → `Err`；无后缀 → `length = None` 且 `name` 为原文（trim 后） | `model_spec_test` 逐行表驱动 |
| AC-7 | 窗长优先级可考：模型名后缀 > `context_length` > 128K 缺省，且**按档**独立解析（`high` / `low` 可各不同）；`EngineFacade::with_context_window` 签名不变 | `runner_test` 按档锚 + `context_test` 既有缺省锚 |
| AC-8 | anthropic 输出上限取值链：`provider.max_tokens` 优先；缺席时 rig 名单内模型保留官方默认值（不被降级）；名单外模型补 `32_000`；openai 协议 MUST NOT 写入 `max_tokens`（请求保持省略） | `runner_test` 三档取值链锚 + 源码复核 `loop.rs` / `compact.rs` 零 diff |
| AC-9 | anthropic beta 条件发：`protocol = anthropic` 且模型名带 `[1m]` → 请求 `anthropic-beta` 含 `context-1m-2025-08-07`；openai 协议或后缀缺席 → 不发 | `runner_test`（断言 `AnthropicConfig.betas` 面，不打网络） |
| AC-10 | 保存期校验单点：非法后缀与 `max_tokens < 4096` 均 `Err` 且错误串不含 api_key；合法形态（含 `[1m]`、`max_tokens = 4096`）保存成功；校验复用引擎侧同一 `parse_model_spec`（无第二套规则） | `src/commands/agents/mod_test.rs` |
| AC-11 | 管理页：新建缺省协议 `openai`；切 anthropic 后 `base_url` placeholder / 提示与协议徽标同步；`maxTokens` 留空 = 未配置、`< 4096` 拒绝提交；保存入参携 `protocol` / `maxTokens`；列表行协议徽标可见 | `provider-panel.test.tsx` + `use-agent-providers.test.ts` |
| AC-12 | `bindings:check` 零 diff（绑定文件由再生成产出、非手改）；`pnpm -C packages/desktop run check` 与 `run test` 全绿 | 静态检查链 + 全量测试 |

> **验收面口径**：验收止于**单元测试 + 静态检查链**。真机冒烟（真实 openai 兼容端点 / anthropic 端点 + 凭据）不属本变更验收条件——本工作流的 test-execution 相位只能跑单元测试，不含端点联通与凭据资源。rig 编码差异与端点差异以 design 的静态锚（双臂组装描述面、三档取值链、`[1m]` 解析表）与守线 diff 承载，真实端点行为不在自动化验证面内。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 名单外 + 小输出上限模型仍会 400**：rig 只对名单内模型给官方值，名单外一律兜底 32_000；老代 `claude-3-5-sonnet-*`（8K）与多数第三方 anthropic 网关（8K/16K）不填 `max_tokens` 时首轮 400 | 用户体感「配了就跑不起来」 | 高（若主要用老代/第三方） | 本变更已提供 `max_tokens` 字段（等价 `CLAUDE_CODE_MAX_OUTPUT_TOKENS`）作为唯一正解；失败经 `api_error` 显式暴露（非静默）；边界以 DD5 / DD6 与 spec 边界条 + 表单提示留痕 |
| **R2 beta 一律发**：无模型元数据表，`[1m]` 不做 `supports_1m_beta` 白名单，第三方 anthropic 网关可能拒 `anthropic-beta` header | 带后缀请求失败 | 中 | 失败以 `api_error` 显式呈现；退路是用户去掉后缀（本地防线按 `context_length` / 128K 估算，行为退化为现状） |
| **R3 跨协议续会话 reasoning 静默丢弃**：`replayable_to` 只回放本 dialect issuer 的 reasoning 块；改协议后续旧会话的多轮工具结构依赖 rig Message 通用性编码 | 思考块丢失、上下文密度下降 | 中（用户改协议后续会话时） | Q3 已裁允许（与「改 `base_url` / `api_key` 同权」）；system 消息不进持久化史已核查（`resume::rebuild` 过滤 + 每轮头部单独注入），无残留 system 形态问题；边界写入 `desktop-agent-execution`「SDK 租户 MVP 边界」 |
| **R4 `[1m]` 对非 1M 模型是纯本地假设**：后缀只抬本地防线估算、不改变真实窗长，错配后请求真实超窗由 L4（provider 报错收敛）兜底 | 超窗轮失败 | 中 | 已知边界留痕（表单提示 + spec 边界条）；`context_length` 仍可精确表达；不自动发 beta 于 openai 协议以免二次伤害 |
| **R5 `EngineConfig` 增字段的面扩散**：18 处 `EngineConfig` 字面量需补 `protocol`，`SdkRunner::new` 36 处调用点若同步加参会放大 churn | 改动面膨胀、易漏改 | 中 | 字段用 `Protocol::Openai` 缺省收敛（`empty()` 已给缺省）；`max_tokens` **不进** `SdkRunner::new` 签名，改走 `with_max_tokens` builder（36 处零改）；`max_tokens` / `context_length` 保持门面旁路，`EngineConfig` 只承载连接必需项 |
| **R6 store 三级升级链写错即数据面报废**：`AgentProviderRecord` 是 V1→V2→V3 三级链，`from` 属性与双向 `From` 必须逐级对齐 | 存量 provider 读不出 / 版本冲突 | 中 | 沿 `ExploreRecord` / `ChangeRecord` 现成先例；AC-1 三条升级路径（v1 行 / v2 行 / 新行）各有用例；`bindings:check` 兜版本头漂移 |
| **R7 openai 侧回归**：协议缺省路径被 `match` 改造波及 | 既有 openai 兼容端点（GLM / DeepSeek / 自建）行为变化 | 中 | openai 臂逐字保留现状（`.chat(model).erase()`，不写 `max_tokens`）；AC-5 断言组装等价；`loop.rs` / `normalize.rs` / `resume.rs` 零 diff 可机械复核 |
| **R8 anthropic `base_url` 语义与 openai 不同**：openai 原样拼 `/chat/completions`（须含 `/v1`），anthropic 由 rig 剥 `/v1` / `/messages` 后缀再拼 `/v1/messages` | 用户按 openai 习惯填地址产生重复段 / 或第三方网关路径非 `/v1/messages` 时不可用 | 中 | rig 的 `normalize_base_url` 已剥后缀（填 `…/v1` 也安全），AC-5 有锚；非标准路径网关不支持属 rig 固定路径限制，写入管理页提示（`base_url` 随协议切换） |
| **R9 上报口径为提案裁定项**：explore §11.4 遗留「`RunStarted.model` / `SessionConfigSnapshot.model` 用裸名还是原文」未拍板 | 会话历史显示与实际请求不一致的观感 | 低 | 本提案裁定：引擎上报与请求用**裸名**（反映实际发送），`SessionConfigSnapshot.model` 保留配置原文（配置快照语义，含后缀即用户配置事实），引擎日志同时记 `model=裸名 … raw=原文`；若后续要求时间线显示原文，另立变更 |
| **R10 `max_tokens` 对 openai 协议静默不生效** | 用户为 openai 协议填了上限却发现无效 | 中 | spec 与表单提示显式声明「仅 anthropic 生效」；保存不拒收（字段仍记录，将来可扩展），但 UI 提示在位 |
| **R11 `[1m]` 单位进制 1000 与既有 128K 缺省（1024 基）不一致** | 同一字段两种进制，估算偏差 | 低 | Q4b 已定 1000（对齐 Anthropic 标称与 Claude Code）；`context_length` 缺省 128K = `128 * 1024` 不动（既有行为），文档与代码注释显式标注两口径来源 |
