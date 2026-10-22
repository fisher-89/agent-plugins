# 桌面端 agent provider 支持选择模型协议（openai / anthropic）

workflow_type: requirement

## 1. 需求与现状

需求：桌面端「agent provider」除现有连接字段外，可选模型协议 `openai` / `anthropic`，运行时按协议走对应 wire。

现状一句话：**provider 只有一套「openai 兼容」形状，SDK 引擎在唯一模型组装点硬编码 `OpenAIConfig + chat`。**

```
SQLite 全局库 (user 维度)
  AgentProviderRecord{ id, name, base_url, api_key,
                       models{high,medium,low}, context_length? }
        │ find_agent_provider
        ▼
compose::resolve_agent_engine            ← 解析单点（compose.rs:354-402）
  ResolvedEngine{ kind, config: EngineConfig, context_window, agent_instance_id }
        │ EngineFacade::runner_for(kind, config)      （lib.rs:95-109）
        ▼
sdk::runner::SdkRunner{ config, resume, context_window, explore_store }
        │ open_session → build_model(config, model_name)   （runner.rs:161-167）
        ▼
rig OpenAIConfig(base_url).client().chat(model).erase()
        ▼
DynModel<Completion> ──► sdk/loop.rs（协议中立，消费 rig 抽象事件）
```

关键事实（已核对源码）：

- `sdk/runner.rs:11` 唯一 `use rig::providers::openai::OpenAIConfig;`；全仓库无 `AnthropicConfig` 引用。
- `sdk/config.rs` `EngineConfig` 四字段：`api_key / base_url / model_high / model_low`（无 Debug，防凭据入日志）。
- `store/model.rs:1180+` `AgentProviderRecord` `#[native_model(id = 5, version = 2)]`，手写遮蔽 Debug，`new` 五参。
- `commands/agents/mod.rs` `save_agent_provider` 是散字段入参（id/name/base_url/api_key/models/context_length），specta 生成前端绑定。
- `sdk/loop.rs` / `normalize.rs` 只消费 rig 的 `StreamEvent` / `Completion` / `Message` 抽象，**协议差异对 loop 不可见**（normalize 的 openai fixture 只是测试形状，不是运行时解析）。

## 2. rig 0.43.0 的 anthropic 侧（可行性与细节）

`rig` 锁版 `=0.43.0`，`default-features = false` + `reqwest/native-tls/memory`。`providers::anthropic` **无 feature gate**，`rig::providers::anthropic` 可直接用。

对照两套组装 API：

| | openai（现状） | anthropic（新增） |
|---|---|---|
| config | `OpenAIConfig::new(key).with_base_url(url)` | `AnthropicConfig::new(key).with_base_url(url)` |
| client | `.client()` | `.client()`（同一个 reqwest 共享实例） |
| 模型 | `.chat(model)` → `Model<Chat>` | `.completion(model)` → `Model<Messages>` |
| erase | `Wire::Op = Completion` | `Wire::Op = Completion` ✅ 两者都可 `.erase() → DynModel<Completion>` |
| 认证头 | `Authorization: Bearer` | `x-api-key` + `anthropic-version`（rig 内建，`wire.rs:298-307`） |
| base_url 语义 | **原样**拼 `/chat/completions`（必须含 `/v1`） | `normalize_base_url` **剥掉** `/v1`、`/messages`、`/v1/messages` 后缀（`wire.rs:312-320`），再拼 `/v1/messages` |

⚠️ **最硬的坑：anthropic `max_tokens` 必填。** `Messages::default_max_tokens` 由 `MaxTokens::ByModel` 按 rig 内置模型名单给值（`claude-opus-5* / opus-4-x / sonnet-4-x / haiku-4-5` 等，`completion.rs:1098-1109`）；**名单外的模型名 → `None` → 编码期直接 `EncodeError("max_tokens must be set for Anthropic")`**（`wire.rs:452-460`）。第三方 anthropic 兼容网关或旧模型名（如 `claude-3-5-sonnet-*`）会踩中。

可行解法：`Model.wire` 是 pub 字段（`driver.rs:52-57`），组装后可覆写：

```rust
let mut model = AnthropicConfig::new(key).with_base_url(url).client().completion(name);
model.wire = model.wire.with_default_max_tokens(64_000); // 兜底值待定
model.erase()
```

另一个差异：resume 重建历史喂给新协议时，rig 做 `request.replayable_to(&issuers)`——**只回放本 dialect issuer 的 reasoning 块**（`anthropic/wire.rs:463+`）。跨协议续会话技术上能编码，但 thinking/reasoning 会静默丢弃，工具配对结构依赖 rig Message 通用性。

## 3. 改动面（按依赖顺序）

```
store/model.rs ─┬─► store/lib.rs re-export ─► commands/agents/mod.rs ─► bindings.ts ─┬─► use-agent-providers.ts
  (+V2→V3)      │                                                                   └─► provider-panel.tsx (+tests)
                └─► infra/agent/sdk/config.rs ─► runner.rs(build_model) ─► compose.rs
                          (protocol 放哪)               (双协议分支)         (解析填值)
                                                                              │
                                                            specs/desktop-agent-management
                                                            specs/desktop-agent-execution
```

### Rust 侧

1. **store**：新增 `AgentProviderProtocol`（`Openai=0 / Anthropic=1`，serde camelCase 线值 `"openai"/"anthropic"`；bincode 序号为解码锚，MUST NOT 重排）；`AgentProviderRecordV2` 历史形态 + `#[native_model(id = 5, version = 3, from = AgentProviderRecordV2)]`；双向 `From`（升级补 `Openai` 默认，降级丢列）；手写 Debug 增 `protocol` 位；`new` 增参（约 23 处调用点）。
2. **commands/agents**：`save_agent_provider` 增 `protocol` 参数（留空 key 回填逻辑不动）；list 自动带出。
3. **infra/agent**：`EngineConfig` 携协议（见 §4 取舍）；`build_model` match 双臂；anthropic 臂设 `default_max_tokens`；启动日志 `泵启动 … protocol=…` 便于诊断；`is_complete` 校验面基本不变（协议恒有值）。
4. `pnpm run bindings:export` 重新生成 `src/types/generated/bindings.ts`。

### TS 侧

5. `use-agent-providers.ts`：`AgentProviderSaveInput` 增 `protocol`，`commands.saveAgentProvider(...)` 实参追加。
6. `provider-panel.tsx`：表单态增 `protocol`；控件复用 `StandardSelect`（agent-run-form 同款）或 radio；`base_url` placeholder / 提示随协议切换（openai `https://…/v1`，anthropic `https://api.anthropic.com`）；列表行展示协议标签。
7. 测试：`provider-panel.test.tsx`（新建/编辑/列表/保存入参）、`use-agent-providers.test.ts`（invoke 参数）、`agents-view.test.tsx`（若 fixture 需协议字段）。

### spec 侧

8. `desktop-agent-management`：provider 字段面增 protocol（默认 `openai` 兼容存量行）；管理页表单/列表；解析产物携带协议。
9. `desktop-agent-execution`：现有条款「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」（spec.md:221）需修订为双协议可选；「SDK 引擎（rig 进程内租户）」增协议选臂与 anthropic `max_tokens` 兜底约束。

## 4. 设计取舍：protocol 住哪？

**方案 A：`EngineConfig` 增 `protocol` 字段（四→五字段）**
- 优点：连接配置内聚，解析单点产物完整；runner 无需第二通道。
- 代价：spec 现有「`EngineConfig` 三字段结构 MUST NOT 变化」条款需显式修订；约 10 处测试字面量构造（runner_test / lib_test / compose_test / config_test）逐处补字段。

**方案 B：仿 `context_length` 旁路——`EngineFacade::with_protocol(...)`**
- 优点：`EngineConfig` 承诺不破，改动更外科；沿既有先例。
- 代价：协议属连接配置核心，被拆到第二通道；`SdkRunner::new` 变五参；两个"旁路"载荷并列（context_window + protocol），后续易漂移。

**方案 C（排除）**：把协议编码进 `EngineKind`（`SdkOpenai/SdkAnthropic`）——污染租户语义、触 bincode 锚、且协议是 provider 属性。另 `AgentEngineKind`/`EngineKind` 的双枚举映射先例可借鉴到协议枚举（store `AgentProviderProtocol` → 门面 `Protocol`），也可直接让 `EngineConfig` 用 store 枚举（agent-runtime 已依赖 store）。

**决策（已定，见 §7）：方案 A**——`EngineConfig` 增 protocol 字段 + 门面侧独立 `Protocol` 枚举映射（store `AgentProviderProtocol` → 门面 `Protocol`，沿 `AgentEngineKind` → `EngineKind` 先例）；spec 里「三字段结构」措辞改写为「连接配置字段面（含协议）」。

## 5. 风险与开放问题

1. **跨协议续会话**：会话 `SessionConfigSnapshot` 只存 `engine/model/permission_mode`，不含 provider 或协议；用户改协议后 `Continue` 会用新 wire 重编码旧历史（reasoning 丢弃、协议契约差异）。是否要加「协议一致性」校验（快照加 protocol）？还是维持与「改 base_url/key 同权」的现状？
   **决策：允许（不加校验）**。补充核查：system 消息不进持久化历史（`resume::rebuild` 过滤非对话事件，loop 每轮在 `chat_history` 头部单独注入一条 `Message::system`），故跨协议历史里无残留 system；rig anthropic wire 以 `split_system_messages_from_history` 抽取 top-level system（`completion.rs:2138`），形态天然安全。真实差异只剩 reasoning 回放过滤（`replayable_to` 仅回放本 dialect issuer 的块）。
2. **anthropic `max_tokens` 兜底值**：命名未命中 rig 名单时必需。取值影响长回复/多工具轮（官方输出上限 64K/128K；第三方网关常限 8K/16K）。候选：64_000 / 16_000 / 8_192。
   **决策：64_000，且条件覆盖**——只在 `wire.default_max_tokens.is_none()` 时补（rig 名单命中时保留官方 128K/64K 值不被降级）。**残留风险（重要）**：名单外 ≠ 大输出上限。旧代 Claude 3.x（如 `claude-3-5-sonnet-20241022`，官方输出上限 8K）与多数第三方 anthropic 兼容网关（8K/16K）会在 64K 时 400。届时 loop 以 `api_error` 显式失败（可诊断、非静默），但用户体感是"配了就跑不起来"。缓解见 §8 末尾的备选。
3. **存量数据默认**：v2→v3 升级补 `openai`，行为零变化；需回归「旧库读入后仍可发起」。
4. **协议可编辑性**：编辑已建 provider 时允许改协议吗？建议允许（记录是连接档案），但需 UI 提示影响已建会话。**决策：允许**（与改名 / 改 base_url 同权）。
5. **UI 细节**：`MaskedApiKey` 前缀硬编码 `sk-***`（anthropic key 也是 `sk-ant-`，仍成立，可不改）；`base_url` 语义差异需提示；列表是否需要协议徽标（建议要）。**决策：做协议视觉标识**（列表徽标 + 表单选择器 + base_url 提示随协议切换）。
6. **上下文窗缺省 128K** 对 anthropic（实际 200K）——`context_length` 已可配，可不按协议改缺省。
7. **协议切换后的运行中会话**：运行中改 provider 不影响已打开 runner（配置快照在 compose 解析时定型），无需额外治理。

## 6. 验证路径

- 单测：store v2→v3 升级/往返/版本头 3/线值；compose 协议透传；runner 双臂组装（`DynModel` 的 name/id 或 wire 断言）；前端 save 参数与列表展示。
- 全量：`pnpm -C packages/desktop run check` + `run test`。
- 真机冒烟（必须，rig 编码差异只有真端点能验）——anthropic 侧三档：
  1. **名单内官方模型**（如 `claude-sonnet-4-6`）：rig 自带 64K 缺省，验证基本链路与工具调用；
  2. **名单外、大输出上限模型**：验证 64K 兜底生效且不被覆盖；
  3. **名单外、8K 上限模型**（如旧代 `claude-3-5-sonnet-20241022`）：预期 400 + `api_error` 可读诊断，作为「兜底策略边界」的证据留痕。
- openai 侧回归：兼容端点（自建/GLM/DeepSeek）跑一轮含工具调用，确认协议缺省行为零变化。
- 负例：anthropic + 名单外模型若未兜底 → 首轮 `EncodeError("max_tokens must be set for Anthropic")` 经 `api_error` 显式失败，确认不会静默。

## 7. 决策记录（用户已定夺）

| 编号 | 问题 | 决定 | 影响 |
|---|---|---|---|
| Q1 | 协议挂 provider 还是 agent 实例 | **provider 级** | 协议列落 `AgentProviderRecord`，agent 实例零改动 |
| Q2 | 协议如何抵达 engine | **方案 A**：`EngineConfig` 增 protocol | 修订 spec「三字段」承诺；约 17 处测试字面量补字段 |
| Q3 | 跨协议续会话 | **允许**（不加一致性校验） | `SessionConfigSnapshot` 不动；reasoning 丢弃可接受 |
| Q4 | anthropic `max_tokens` 兜底 | ~~64K，仅名单外生效~~ → **改由模型名后缀方案承载（§11）** | 见 §11；§9 备选作废 |
| Q5 | 协议视觉标识 | **做** | 列表徽标 + 表单选择器 + base_url 提示切换 |

## 8. 实施清单（定稿）

### Rust（依赖序）

1. **store/model.rs**
   - `AgentProviderProtocol { Openai = 0, Anthropic = 1 }`：`#[serde(rename_all = "camelCase")]` → 线值 `"openai"/"anthropic"`；derive `Type`（随 `AgentProviderRecord` 生成 TS）；`impl Default → Openai`（供 `#[serde(default)]`）；bincode 序号为解码锚，MUST NOT 重排。
   - `AgentProviderRecordV2`（= 当前 v2 形态，无 protocol）：`#[native_model(id = 5, version = 2)]`。
   - `AgentProviderRecord`：`version = 3, from = AgentProviderRecordV2`，增 `#[serde(default)] pub protocol: AgentProviderProtocol`。
   - 双向 `From`：升级补 `Openai`（存量行为零变化）；降级丢列（同 V1 先例）。
   - 手写遮蔽 Debug 增 `protocol` 位（沿用「字段面与 derive 对齐」约定）。
   - `AgentProviderRecord::new` 增 `protocol` 参 → 23 处调用点（含 6 个测试文件的 fixture）。
2. **commands/agents/mod.rs**：`save_agent_provider` 增 `protocol` 参（建议 `id, name, protocol, base_url, …`——协议决定 base_url 语义，签名上显式）；留空 key 回填原值逻辑不动；其余命令零改动。
3. `pnpm run bindings:export` → `src/types/generated/bindings.ts` 自动带出。
4. **infra/agent**
   - `sdk/config.rs`：门面 `Protocol { Openai, Anthropic }`；`EngineConfig` 增 `protocol: Protocol`；`empty()` 给 `Openai`；`is_complete` 不变。
   - `sdk/runner.rs::build_model` 双臂：
     ```rust
     match config.protocol {
         Protocol::Openai => OpenAIConfig::new(key).with_base_url(url).client().chat(model).erase(),
         Protocol::Anthropic => {
             let mut m = AnthropicConfig::new(key).with_base_url(url).client().completion(model);
             if m.wire.default_max_tokens.is_none() {          // 名单内保持 rig 官方值
                 m.wire = m.wire.with_default_max_tokens(MAX_TOKENS_FALLBACK); // 64_000
             }
             m.erase()
         }
     }
     ```
   - `import` 增 `rig::providers::anthropic::AnthropicConfig`（无 feature gate，`providers::anthropic` 恒可用）。
   - 泵启动日志加 `protocol={:?}`（诊断：同一 base_url 配错协议时一眼可见）。
   - 测试字面量补字段：`compose.rs` 1、`lib_test.rs` 4、`config_test.rs` 7、`runner_test.rs` 5（多数可 `..complete_config()`）。
5. **compose.rs**：`resolve_agent_engine` 的 sdk 臂映射 `provider.protocol → Protocol`；`ResolvedEngine` 语义不变（protocol 已在 `EngineConfig` 内）。

### TS

6. `use-agent-providers.ts`：`AgentProviderSaveInput` 增 `protocol: AgentProviderProtocol`；`commands.saveAgentProvider` 实参按新顺序。
7. `provider-panel.tsx`：
   - `ProviderFormState` 增 `protocol`；新建默认 `'openai'`；
   - 协议选择用 `StandardSelect`（`agent-run-form` 同款）；
   - `base_url` placeholder/提示随协议切换（openai `https://…/v1`；anthropic `https://api.anthropic.com`，提示 rig 会自动剥 `/v1`）；
   - 列表行加协议徽标；编辑态允许改协议（无需二次确认，与改 base_url 同权）。
8. 测试：`provider-panel.test.tsx`（默认值 / 切协议后 placeholder / 保存入参 / 徽标）、`use-agent-providers.test.ts`（invoke 实参）、`agents-view.test.tsx` / `agent-panel.test.tsx`（fixture 补 `protocol`）。

### spec

9. `desktop-agent-management`：provider 字段面增 `protocol`（缺省 `openai` 兼容存量行）；管理页表单/列表/徽标；解析产物携带协议；**改写「`EngineConfig` 三字段 MUST NOT 变化」为「连接配置字段面」**。
10. `desktop-agent-execution`：§「SDK 引擎（rig 进程内租户）」现有条款「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」（spec.md:221）改为双协议；增 anthropic `max_tokens` 约束（名单内用 rig 官方值，名单外补 64K）。

## 9. 残留风险与备选（Q4 相关；已被 §11 方案丁取代，保留为决策历史）

**风险**：64K 兜底对「名单外 + 小输出上限」模型（旧代 Claude 3.x、多数第三方 anthropic 网关）会 400。

- **备选 A（最小，当前决定）**：维持 64K；错误经 `api_error` 显式暴露，UI 不动。适用于「主要用官方新代模型」。
- **备选 B（推荐，成本约 +1 列）**：provider 增可空 `max_tokens` 列，anthropic 组装时优先用它，缺席才 64K。与 `context_length` 完全同型（V3 一次演进到位，避免再升 V4）。代价：多一列 + 表单多一字段 + spec 多一段。

若选 B，可在本次 change 内一次做完（省一次 DB 版本演进）；若选 A，则真机验证第 3 档（8K 模型）失败时再补。

## 10. 澄清：「8K 模型可在 Provider 指定长度」与「选 A」的字面冲突（已被 §11 覆盖）

用户指示：**选 A**，并称「8K 模型可以在 Provider 指定长度」。两句字面互斥，需一句确认。

### 为什么现有 provider 字段顶不上

已核对源码：`context_length` **只喂上下文防线，不喂请求输出上限**。

```
provider.context_length ──► compose::ResolvedEngine.context_window
                         ──► EngineFacade::with_context_window
                         ──► ContextDefense{ context_window,
                                             L2 = 75%、L3 = 90%、保护窗 40k }   （context.rs:22-48）
```

而发请求时输出上限恒为 `None`（`loop.rs:183`、`compact.rs:48`），即「不带字段，由协议自决」。所以：

- 把 `context_length` 填 8000 想表达「输出 8K」→ **请求仍发 `max_tokens: 64000`（兜底）→ 依旧 400**；
- 副作用是真的：L2 水位降到 6K、L3 降到 7.2K，保护窗 40k 反超整窗 → **上下文防线失效**（长会话被疯狂剪裁）。这是个静默的坑，不能靠用户「填对字段」规避。

### 三条可能解读

| 解读 | 数据面 | 8K 模型能否跑通 | 代价 |
|---|---|---|---|
| 甲·真 A | 不加列，固定 64K | ❌ 400 | 0 |
| 乙·加 `max_tokens` 列（=原备选 B） | provider 增可空列，表单多字段 | ✅ 精确 | +1 列（V3 同批） |
| 丙·由 `context_length` 派生输出上限 | 不加列，`cap = min(64K, context_length)` | ✅ 但语义混淆 | 0（但埋雷：200K 窗 + 8K 输出的模型无法表达） |

### 若选乙：实现比 §8 写的更省

不必动 rig 的 wire 私有面。`CompletionRequest.max_tokens` 两侧 wire 都认（openai 经 `OutputCap` 映射 `max_tokens` / `max_completion_tokens`；anthropic 仅在 `None` 时补缺省），所以：

```
provider.max_tokens ──► EngineConfig.max_tokens? ──► LoopTurn.max_tokens ──► request.max_tokens = Some(n)
```

组装侧只剩一句等价式（也覆盖名单内模型被用户显式降档的情形）：

```rust
let cap = provider_cap.or(wire.default_max_tokens).unwrap_or(64_000);
```

注意：**该上限只对 anthropic 生效**。openai 兼容端点当前省略 `max_tokens`（各网关对超限值行为不一，有的直接 400），MUST NOT 无条件写入。

### 待确认（一句）

「在 Provider 指定长度」指的是 **(乙) 新增输出上限字段**，还是 **(丙) 复用 context_length 兼作输出上限**，还是仅表示「这条路以后再说」（即真·甲）？

## 11. 方案丁：模型名后缀 `<model>[<length>]`（用户新指示，覆盖 §9）

用户指示：采用 Claude Code 的配置形态 `<模型名>[<长度>]`（例 `deepseek-flash[1m]`）；无中括号时用默认值。

### 11.1 先澄清一处语义

Claude Code 的 `sonnet[1m]` 里，`[1m]` 表示 **1M 上下文窗口**（Anthropic 的 1M context beta），**不是输出上限**。而 §9 里要解决的 8K 问题是**输出上限**（`max_tokens`）。两者是不同维度：

```
上下文窗口 (context window)  ──► 本地防线：L2 75% / L3 90% / 保护窗 40k
输出上限 (max_tokens)        ──► 请求体字段，anthropic 必填
```

好消息是：**同一个后缀可以安全地兼管两者**，因为「输出上限不可能超过模型总长」：

```rust
// anthropic 组装
let cap = spec.length.map(|n| n.min(64_000))       // [8k]→8K、[200k]→64K、[1m]→64K
    .or(wire.default_max_tokens)                    // rig 名单内官方值
    .unwrap_or(64_000);                             // 名单外兜底
```

局限（需要知情）：无法表达「窗 200K + 输出 8K」，因为只有一个数字。若真有这种模型，只能二选一（或以后再加独立列）。

### 11.2 解析规则（建议）

| 输入 | 裸模型名 | 长度 | 备注 |
|---|---|---|---|
| `deepseek-flash` | `deepseek-flash` | 无 | 走缺省：provider.context_length → 128K |
| `deepseek-flash[1m]` | `deepseek-flash` | 1,048,576（或 1,000,000，见 11.5） | |
| `claude-3-5-sonnet[8k]` | `claude-3-5-sonnet` | 8,192 | 输出上限同步压到 8K |
| `glm-4.6[200k]` | `glm-4.6` | 204,800 | |
| `model[131072]` | `model` | 131,072 | 纯数字 |
| `model[0]` / `model[]` / `model[8x]` / `model[1M]` | — | — | 非法（大小写可宽容，单位限 k/m/纯数字） |

规则：方括号须在**末尾**；单位大小写不敏感；裁剪两端空白；后缀非法 → 显式失败（见 11.4），MUST NOT 把带后缀的原文发给 API。

### 11.3 与 `context_length` 列的关系

优先级：**模型名后缀 > `provider.context_length` > 128K 缺省**。

```
model 名带 [len] ──► len 生效，context_length 列忽略（管理页提示「已被模型名覆盖」）
model 名不带     ──► 用 context_length，缺席 128K
```

保留 `context_length` 列的理由：存量记录继续可用、可表达「三档共享同一窗长」。不删列、不改语义，只让后缀就近覆盖。

### 11.4 落点与失败面

- 新增 `sdk/model_spec.rs` 纯函数：`parse(raw) -> Result<ModelSpec{name, length: Option<u64>}, ParseError>`，单测覆盖上表全部形态。
- 调用点在 `SdkRunner::open_session`：那里同时拿到 `model_level`（选档）与模型原文，**天然支持三档各自不同窗长**（`high: model[1m]` / `low: model[200k]`）。
  ```
  open_session:
    raw = config.model_high | model_low        (按档)
    spec = parse(raw)?                          // 非法 → ConfigMissing
    defense = ContextDefense::resolve(spec.length.or(self.context_window))
    model = build_model(config, spec.name)      // 请求只发裸名
  ```
  当前 `context_window` 在 runner 构造期固定、与档位无关；本方案把它降级为「缺省来源」，按档值在 `open_session` 现算，`EngineFacade::with_context_window` 签名不变。
- **保存期校验**：`src-tauri` 已依赖 `agent-runtime`（Cargo.toml:21），`save_agent_provider` 可先 `parse` 三档模型名，非法即 `Err`（避免「保存成功但跑不起来」）。管理页不做第二套正则。
- 上报口径建议：`RunStarted.model` 与 `SessionConfigSnapshot.model` 用**裸名**（反映实际发送）；引擎日志记 `model=裸名 window=… raw=原文`，便于诊断。若希望会话历史显示用户原文，则改报原文——需拍板。

### 11.5 待确认（四点）

1. **单位口径**：`k`/`m` 按 1024（与现有 `DEFAULT_CONTEXT_WINDOW = 128 * 1024` 一致，`1m` ≈ 1.05M）还是按 1000（与 Anthropic 标称 1M 一致）？建议 1024。
2. **`[1m]` 是否等价 Claude Code 的 1M beta**：官方 anthropic 端点需要 `anthropic-beta: context-1m-2025-08-07` 才真正开放 1M（rig 有 `with_beta`，`wire.rs:257`）；第三方网关不认这个 header、可能直接报错。建议**不自动加**（后缀只作本地防线估算），需要时再扩展；若要求完全等价 Claude Code，则仅在 `protocol = anthropic` 且长度 ≥ 1M 时加。
3. **后缀是否兼作输出上限**（11.1 的 `min(len, 64K)`）：建议是——否则 8K 模型仍 400，用户的原始诉求没解决。
4. **上报裸名还是原文**（11.4 末）。

## 12. 实证：Claude Code 实际如何处理（二进制逆向，可直接引用的原文）

从本机 `claude.exe`（307MB，Bun 单文件，内嵌 JS 可读）grep 出的原始实现。

### 12.1 输出上限：与模型名后缀完全解耦

```js
function gvt(e){                                  // 取该模型的输出上限配置
  let t, r, n = Fo(e), o = WE(n)?.max_output_tokens;
  if (o) { t = o.default; r = o.upper; }                       // ① 模型元数据表
  else if (n==="claude-3-opus"||n==="claude-3-haiku") { t=4096; r=4096; }
  else if (n==="claude-3-sonnet") { t=8192; r=8192; }
  else { t = N5y /* 32000 */; r = $5y /* 128000 */; }          // ② 表外兜底

  let i = B5y(n);                                  // ③ CLAUDE_CODE_MAX_OUTPUT_TOKENS
  if (i !== null) t = Math.min(i, r);                           //    夹到 upper

  let s = Oku(e);                                  // ④ 用户 settings 的 max_tokens
  if (s?.max_tokens && s.max_tokens >= 4096) { r = s.max_tokens; t = Math.min(t, r); }
  return { default: t, upperLimit: r };
}
```

元数据表实测值（`max_output_tokens {default, upper}`）：

| 模型 | default | upper | context |
|---|---|---|---|
| `claude-3-5-sonnet` | 8192 | 8192 | — |
| `claude-sonnet-4` / `4-5` | 32000 | 64000 | 200K, 1m beta + suffix |
| `claude-sonnet-4-6` | 32000 | **128000** | 200K, 1m beta + suffix |
| `claude-haiku-4-5` | 32000 | 64000 | 200K, 1m suffix（无 beta） |
| `claude-opus-4` / `4-1` | 32000 | 32000 | 200K, 1m suffix |
| `claude-opus-4-6` | **64000** | 128000 | 200K, 1m beta + suffix |
| 表外模型 | 32000 | 128000 | 200K |

**关键：请求带的是 `default`，不是 `upper`。**

`stop_reason === "max_tokens"` 时**不提高上限**，而是：

```js
if (dr === "max_tokens")
  yield { content: "Claude's response exceeded the ${nt} output token maximum.
           To configure this behavior, set the CLAUDE_CODE_MAX_OUTPUT_TOKENS
           environment variable." };
// 同轮注入 isMeta 续写消息，maxOutputTokensRecoveryCount+1 后 continue：
"output token limit hit. Resume directly — no apology, no recap of what you were
 doing. Pick up mid-thought if that is where the cut happened. Break remaining
 work into smaller pieces."
```

### 12.2 `[1m]` 后缀：只改上下文窗口 + beta，不改输出上限

```js
function dd(e){ return e.replace(/\[(1|2)m\]/gi, ""); }   // 剥离（支持 [1m] [2m]，大小写不敏感）
function PS(e){ return /\[1m\]/i.test(e); }               // 是否带后缀

function jku(e, t){                                        // 上下文窗口
  if (PS(e)) return 1e6;                                   // ① 带 [1m] → 1,000,000
  if (t?.includes(Tq.header) && Cq(e)) return 1e6;         // ② beta header 在 → 1M
  if (F$(e)) return 1e6;                                   // ③ 原生 1M 模型
  let n = Q.CLAUDE_CODE_MAX_CONTEXT_TOKENS;
  if (n > 0 && !Fo(ls(e)).startsWith("claude-")) return n; // ④ 非 claude 模型走 env
  return Ybr /* 200000 */;                                 // ⑤ 缺省 200K
}
```

- 用 `.test(/\[1m\]/i)` 判定，`replace(/\[(1|2)m\]/gi)` 剥离 → **裸名进 API**
- 是否接受后缀由元数据 `context.supports_1m_suffix` 把关
- 是否发 beta 由 `context.supports_1m_beta` 把关，header 值 `context-1m-2025-08-07`
- 单位：`U5y = 1e6`、`yfe = 200000` → **1000 进制**，与用户「按 Anthropic 标准」一致
- ④ 是给第三方模型的逃生口：**非 claude 模型可用 `CLAUDE_CODE_MAX_CONTEXT_TOKENS` 指定窗长**

### 12.3 对 §11 设计的修正（重要）

| 我上一轮写的 | Claude Code 实际 | 处置 |
|---|---|---|
| `[1m]` 兼作输出上限 `min(len, 64K)` | **不兼作**，完全解耦 | 改为不兼作（否则不「等价」） |
| 单位 1024 | **1000**（`1e6` / `200000`） | 改 1000 ✔（用户已定） |
| `[1m]` 发 beta | 发（模型 `supports_1m_beta` 时） | 保留 ✔（用户已定「完全等价」） |

**副作用：8K 模型问题回到原点。** Claude Code 自己对表外模型也用 32000，8K 模型照样 400，靠 `CLAUDE_CODE_MAX_OUTPUT_TOKENS` 手动降。所以「完全等价」要求我们**同时提供那个环境变量的等价物**——即当初的备选 B（provider 级可空 `max_tokens`）。这恰好就是用户说的「8K 模型可以在 Provider 指定长度」。

## 13. 落地结论（合并 §11 + §12 的全部决定）

### 13.1 字段面最终形态（provider 记录，V2 → V3 一次演进）

| 字段 | 类型 | 语义 |
|---|---|---|
| `protocol` | `Openai \| Anthropic` | 模型协议（§7 Q1/Q2） |
| `models.{high,medium,low}` | `String` | 可带 `[<len>]` 后缀的模型原文 |
| `context_length` | `Option<u64>` | 窗长缺省来源（后缀覆盖它） |
| `max_tokens` | `Option<u64>` **新增** | 输出上限（等价 `CLAUDE_CODE_MAX_OUTPUT_TOKENS`；留空 = 按协议缺省） |

`max_tokens` 是本轮新增的第四项，理由见 §12.3：不提供它，8K 模型在「完全等价」方案下无解。

### 13.2 解析规则（严格对齐 Claude Code）

```
models.high = "deepseek-flash[1m]"
      │
      ├─ 剥离：/[\[(1|2)m\]/gi → 裸名 "deepseek-flash"（其余形式不剥离）
      ├─ 窗长：带 [1m] → 1_000_000；否则 context_length；否则 128K
      └─ 输出上限（与后缀无关）：
            protocol=anthropic →
                provider.max_tokens
                  .or(rig 名单内 wire 官方值)
                  .unwrap_or(32_000)          // 表外兜底对齐 Claude Code 的 32000
            protocol=openai    → 保持 None（现行为不变）
      └─ beta：protocol=anthropic 且带 [1m] → with_beta("context-1m-2025-08-07")
```

千进制：`1m = 1_000_000`、`200k = 200_000`。

### 13.3 与 Claude Code 的已知差异（须留痕）

1. **没有模型元数据表**：我们不知道任意模型的 `default/upper`，也不能判断 `supports_1m_suffix` / `supports_1m_beta`。因此 `[1m]` 一律接受（不做模型白名单校验），beta 一律发（第三方网关可能拒 → `api_error` 暴露）。
2. **表外输出上限兜底取 32K**（对齐 Claude Code 表外行为），而非上一版设计的 64K。
3. **不实现 `max_tokens` 续写恢复**（Claude Code 的 `maxOutputTokensRecovery` 是交互式会话特性）：达到上限以既有 `finish` 语义收敛，用户调 `max_tokens` 后重跑。若后续要加，落点是 `sdk/loop.rs` 的收敛分支。
4. **`[2m]` 后缀**：Claude Code 的剥离正则接受但无 2M 窗长常量（只有 `1e6`）。建议**只认 `[1m]`**，其余显式拒绝，避免语义悬空。

### 13.4 待确认（最后两点）

1. `max_tokens` 必填校验：`<4096` 是否拒绝（Claude Code 的 `s.max_tokens >= 4096` 门槛）？
2. 表外输出上限兜底用 **32K**（对齐 Claude Code）还是 **64K**？——32K 更保守（老代模型 8K/16K 仍会 400，但概率低些）；64K 更少触发上限截断。

## 14. 最终决策表（全部锁定）

| 编号 | 决策 | 值 |
|---|---|---|
| Q1 | 协议归属 | provider 级 |
| Q2 | 协议抵达引擎 | 方案 A：`EngineConfig` 增字段 + 门面 `Protocol` 枚举映射 |
| Q3 | 跨协议续会话 | 允许，不加一致性校验 |
| Q4 | `[1m]` 语义 | 只改上下文窗长 + anthropic beta，**不改输出上限**（对齐 Claude Code） |
| Q4b | 单位进制 | **1000**（`1m = 1_000_000`、`200k = 200_000`） |
| Q4c | `[1m]` 是否发 beta | **发**（完全等价 Claude Code；`context-1m-2025-08-07`） |
| Q4d | 输出上限来源 | **provider 新增可空 `max_tokens`**（等价 `CLAUDE_CODE_MAX_OUTPUT_TOKENS`） |
| Q4e | 表外兜底 | **32_000**（对齐 Claude Code 表外行为） |
| Q4f | `max_tokens` 下限 | **`< 4096` 拒绝**（对齐 Claude Code 门槛） |
| Q5 | 协议视觉标识 | 做（列表徽标 + 表单选择器 + base_url 提示切换） |
| Q5b | `[2m]` | 不做，显式拒绝 |

## 15. spec 改动定位（精确到条款）

### desktop-agent-management/spec.md

| 行 | 现有条款 | 改动 |
|---|---|---|
| 13 | provider 字段面：`base_url、api_key、models 三档、可空 context_length` | 增 `protocol`（`openai`/`anthropic`，缺省 openai 兼容存量）与可空 `max_tokens`（输出上限，`<4096` 拒收） |
| 17 | 「`context_length` 的消费接线 MUST NOT 经 `EngineConfig`（三字段结构不动承诺维持）」 | **必改**：protocol 进 `EngineConfig` → 措辞改为「连接配置字段面（含协议）」，并说明 `context_length` / `max_tokens` 仍走旁路 |
| 58 | 「组装 `EngineConfig{api_key, base_url, model}`，model 固定取 high 档」 | 增协议；补「模型名 `[1m]` 后缀解析」 |
| 60 | 「`EngineConfig` 三字段结构体 MUST NOT 变化」 | 同上改写 |
| 87-93 | Agent 管理页字段面 | 增 protocol 选择器 + `max_tokens` 数字字段 + 列表协议徽标 + 模型名后缀提示 |
| 105 | Scenario: context_length 编辑与清空 | 增 `max_tokens` 同型 scenario + `<4096` 拒收 scenario + 后缀解析 scenario |

### desktop-agent-execution/spec.md

| 行 | 现有条款 | 改动 |
|---|---|---|
| 221 | 「provider 首公民 SHALL 为 openai-compatible 端点；**anthropic provider 降为后续可选**」 | **必须删除/改写**——本需求正是偿还这一条 |
| 217-244 | SDK 引擎（rig 进程内租户）约定块 | 增：协议选臂（openai `chat` / anthropic `completion`）、anthropic `max_tokens` 三档取值链、`[1m]` → `with_beta` |
| 733ish | 「engine_cfg 硬编码位（已偿还）」 | 无需改，仅确认叙述仍成立 |
| 746 | Module Contract 表 `EngineConfig{api_key, base_url, model}` | 增 protocol 字段与 `max_tokens` 旁路说明 |

### 新增可能需要留痕的边界

- 无模型元数据表 → `[1m]` 不做白名单校验、beta 一律发（第三方网关拒绝以 `api_error` 暴露）
- 不实现 `maxOutputTokensRecovery`（撞上限即收敛，用户调 `max_tokens` 后重跑）

## 16. 实施顺序（依赖拓扑，可直接转 tasks.md）

```
① store/model.rs          protocol + max_tokens + V2→V3 + new 六参      [23 处调用点]
② store/lib.rs            re-export 协议枚举
③ commands/agents         save_agent_provider 增参 + 保存期校验（<4096 / [2m] 拒绝）
④ bindings:export         src/types/generated/bindings.ts 再生成
⑤ infra/agent/sdk/config  Protocol 枚举 + EngineConfig 增字段
⑥ infra/agent/sdk/model_spec.rs  新增 parse（[1m] 剥离 + 长度解析）+ 单测
⑦ infra/agent/sdk/runner  open_session 按档 parse；build_model 双臂；anthropic max_tokens 链
⑧ infra/agent/compose     解析单点填 protocol；max_tokens 走旁路（同 context_length 先例）
⑨ 前端 hooks + panel       protocol/max_tokens 字段、徽标、后缀提示
⑩ specs 两处修订（§15）
⑪ 全量 check + test + 真机冒烟（§6 三档）
```

关键校验点：④⑨ 之前必须完成 ①③，否则 bindings 与前端类型不同步。
