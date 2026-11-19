# 模型配置增加 Effort 选项（DeepSeek thinking_mode）

workflow_type: requirement

## 一句话结论

provider 记录今天只有「**选哪个模型**」（high / low 两档模型名），没有「**同一模型想多深**」。DeepSeek thinking_mode 文档把后者叫 effort，并且给出了两条协议各自的 wire 形态；本仓锁的 rig 0.43 两条 wire 都有 `additional_params` 透传缝 —— **可行性已确认，不需要升 rig**。真问题不是「能不能发」，而是「effort 挂在哪个粒度上」。

## 决议总览（按裁定顺序；下文各节是推导过程）

| # | 问题 | 裁定 |
|---|---|---|
| 1 | effort 粒度 | **A2 随档位**：`efforts{high, low}`，跟 `ModelLevel` 走同一分发点 |
| 2 | 值域 | **不做封闭枚举**：目录给建议值 + 允许自由输入；保存期只 trim / 非空 |
| 3 | 目录真源 | **Rust 静态表 + IPC 命令**（`infra/agent`，出 bindings、进 archi 契约、后端单测锚定） |
| 4 | 目录厚度 | **名字 + effort 值域 + 能力元数据**（窗长 / 最大输出 / 是否支持思考），顺带预填 |
| 5 | 预填语义 | **方案 A**：选中目录条目直接写入输入框（附四条纪律，见「预填语义裁定」） |
| 6 | 后端是否消费目录 | **否**——运行期仍走既有取值链，目录只作前端参考建议 |
| 7 | 关思考（`none`） | **不进 effort 值域**（两家语义不同，且 Anthropic 新模型禁关） |


## 现状链路（四段，逐段都已存在一个「门控单点」先例）

```
AgentProviderRecord (store, 全局库)
   models{high,medium,low} · protocol · context_length · max_tokens · web_search
        │
        │  compose::resolve_engine        ← 门控单点（web_search && protocol==Anthropic）
        ▼
ResolvedEngine { EngineConfig{protocol,api_key,base_url,model_high,model_low},   ← 连接必需项
                 context_window, max_tokens, web_search }                        ← 旁路载荷
        │
        │  EngineFacade::with_context_window / with_max_tokens / with_web_search
        ▼
SdkRunner::open_session
   ctx.model_level  →  High→model_high / Low→model_low          （档位唯一消费点）
   parse_model_spec →  [1m] 后缀 → 窗长候选
   build_model(protocol 选臂: openai=chat / anthropic=completion)
        │
        ▼
loop：CompletionRequest{ …, max_tokens: None, additional_params: None }   ← 待注入点
```

关键观察：
- `EngineConfig` 是**连接必需项**的封闭面（协议 + 凭据 + 端点 + 双档模型），`context_length` / `max_tokens` / `web_search` 全部走**旁路 builder**，spec 里写死了「连接配置字段面 MUST NOT 塌」。effort 显然属于旁路那一族。
- `loop` 里 `additional_params: None` 是硬编码的空位（`sdk/loop.rs:328`），`compact.rs:54` 同样为空 —— 注入点现成。
- `web_search` 已经把「协议门控放在 compose、loop 零协议判定」这套写法跑通了；effort 的形状随协议不同，正好套同一套。

## DeepSeek 文档要点（https://api-docs.deepseek.com/zh-cn/guides/thinking_mode）

| | OpenAI 格式（Chat Completions） | Anthropic 格式（Messages） | Responses API |
|---|---|---|---|
| 思考开关 | `{"thinking":{"type":"enabled"/"disabled"}}`（SDK 里要放 `extra_body`） | —（表内空） | `{"reasoning":{"effort":"none/low/high/max"}}`（none = 关） |
| 思考强度 | `{"reasoning_effort":"low/high/max"}` | `{"output_config":{"effort":"low/high/max"}}` | — |

- 思考模式**默认打开**，effort **默认 high**。
- 传入 → 实际映射：`minimal→low`、`low→low`、`medium→high`、`high→high`、`xhigh→high`、`max→max`、`ultra→max`。
  → 换句话说 `low/medium/high` 这套「三档直觉」在 DeepSeek 上是**假三档**（medium 塌到 high）；只有 `low / high / max` 是真档。
- `max_tokens` 缺省随思考模式变：非思考 8K / 思考 64K / `effort=max` 128K —— 这条与本仓 `TruncationKind::OutputCap` 判据有交互。

本仓两条协议恰好一一对应：

```
protocol=openai     →  reasoning_effort: "low" | "high" | "max"     (+ 可选 thinking.type)
protocol=anthropic  →  output_config: { effort: "low" | "high" | "max" }
```

## 落点：rig 0.43 的两条 wire 都能透传（读过源码）

- `providers/openai/completion/mod.rs:1480` — `#[serde(flatten)] additional_params: Option<Value>`，落 Chat 请求体顶层 → `reasoning_effort` 走这里即可（`thinking` 同理）。
- `providers/anthropic/completion.rs:1633` — `AnthropicCompletionRequest` 同样 `#[serde(flatten)] additional_params` → `thinking` / `output_config` 可透。
  - 坑：`output_config` 在 rig 里是**typed 字段**（`Option<OutputConfig>`），只由 `output_schema` 填。本仓 `output_schema: None` 恒成立，所以目前不会双写；日后若开结构化输出，`output_config.effort` 与 `output_config.format` 就要合并成一份（flatten 让位给 typed 字段，effort 会静默丢）。
- rig 的 `ReasoningEffort`（含 `Max`）只在 **Responses API** 分支存在；本仓走的是 `chat()`，不走 Responses —— 不能直接复用 rig 枚举，只能自己发 JSON。

## 三条岔口（这次探索的真正问题）

### 岔口 A：effort 挂在哪个粒度？

```
(A1) provider 单值         models{high,medium,low} + effort: 一档        ← 最小切口
     所有随该 provider 的轮都是同一 effort

(A2) 随档位               effortHigh + effortLow（甚至 medium）        ← ✅ 已选
     proposal/dev-design=High→effort max；implement/test-gen=Low→effort low
     正好复刻 phase_table 已有的「深相位 / 快相位」分野

(A3) 相位级               进 phase_table 的 PhaseAgentSpec            ← 触面最大
     effort 成为工作流语义（16 个 prompt 各配一档）

(A4) 取代 medium 档       models 收回两档、medium 让位给 effort        ← 结构性重构
```
现状素材：`phase_table.rs` 里 proposal / dev-design / test-design / code-review / acceptance = `ModelLevel::High`，implement / test-gen = `ModelLevel::Low`；`ModelLevel` 只有 High/Low 两值，而 `AgentModelTiers` 有 high/medium/low **三档**，`medium` 今天**只存不选**（`EngineConfig` 只有 `model_high` / `model_low`，UI placeholder 写着「暂未启用」）。「medium 档该有的样子」很可能就是 effort。

### 岔口 B：值域怎么定？

1. **对齐 DeepSeek 真档**：`low | high | max`（+ 是否要 `none`=关思考）
2. **抽象三档**：`low | medium | high`（跨 provider 友好，但 DeepSeek 上 medium≡high，语义打滑）
3. **自由字符串透传**：不推荐 —— 本仓所有枚举都是封闭值域 + 保存期校验
4. **只留空 / 两档**：留空 = 不发（用 provider 默认 high），显式 `low` / `max` 两点足够 —— 与既有「留空 = 未配置」的 `context_length` / `max_tokens` 同族

### 岔口 C：留空 = 什么？

`Some(None)` 不发 / `Some(Default)` 发 high 是两种完全不同的承诺：前者让「非 DeepSeek 的 openai 兼容网关」保持今天的请求体逐字不变（行为零变化），后者会往所有 openai 协议请求里塞 `reasoning_effort`。按本仓惯例，**留空 = 不发**是唯一不破存量行为的选择。

## 触面清单（照 `anthropic-web-search` 那张谱抄）

| 层 | 要动什么 | 先例位置 |
|---|---|---|
| store 模型 | v4 → v5：现形态改名 `AgentProviderRecordV4`，新 `#[native_model(id=5, version=5, from=V4)]` + `#[serde(default)] effort`；V4↔V5 双向 `From`；`new()` **参数面不动**，加 `with_effort` | `crates/infra/store/src/model.rs:1263-1460` |
| 手写 Debug | 遮蔽 Debug 补 effort 位（api_key 位仍遮蔽） | 同上 `:1443` |
| 命令 | `save_agent_provider` 参数面 +1，DTO 带出 | `src/commands/agents/` |
| 生成物 | `bindings:export` 刷新 `src/types/generated/{bindings.ts,bindings.zod.ts}`（须整段 review diff） | AGENT.md |
| 解析 | compose `ResolvedEngine` + `EngineFacade::with_effort`（**门控单点**：按协议 shaping 成 wire JSON，或按协议在 loop 侧选臂） | `crates/infra/agent/src/compose.rs:414-432`、`lib.rs:42-70` |
| 执行 | `PumpPayload` → `LoopTurn` → `loop` `additional_params`；引擎日志行加 `effort=…`（与 `protocol=… max_tokens=… web_search=…` 同列） | `sdk/runner.rs` / `sdk/loop.rs:317-336` |
| 摘要请求 | `compact.rs` 是否也带 effort（思考模式下摘要会变贵） | `sdk/compact.rs:39` |
| 前端 | provider 表单加 `StandardSelect`（协议选择器同型），清单行展示；不得混入 `ProviderTextField`；切协议不清零（effort 两协议都有）vs 值域随协议不同时的清零问题 | `views/agents/components/provider-panel.tsx` |
| 规格 | `openspec/specs/desktop-agent-management/spec.md`（provider 字段面 + 存储演进 + 表单）、`desktop-agent-execution`（wire 语义） | 已读，字段面写得很细 |
| 架构契约 | `.dev-team/architecture/api/**` 对应模块的 `api-op` / `api-types`；`api:check` 零 diff 门 | AGENT.md |

## 需要额外留意的副作用

1. **思考本来已经开**：DeepSeek 思考默认开、effort 默认 high → 今天「不发参数」等价于 effort=high。所以这个功能的真实语义是「**降** effort」（省 token / 提速）而不是「开启思考」。
2. **max_tokens 缺省随 effort 变**：本仓 openai 协议**不写** `max_tokens`（`build_model` 对 openai 恒返回 `None`）。DeepSeek 在思考模式下缺省 64K、effort=max 时 128K —— 输出上限会随 effort 静默漂移，`OutputCap` 截断判据（`loop.rs:772-801`）的观感会变。
3. **思考块回放兼容性未验证**：本仓已有 `reasoning::replay`（anthropic 的 `signature` / `redacted` 密文凭据，`ISSUER="anthropic"`）与跨轮 `strip_all`。DeepSeek 的 OpenAI 格式思考内容怎么回放（是否需要 `reasoning_content` 字段回传）是本变更的**未知区**，需要单独一轮调研或明确「跨轮剔除」策略先兜住。
4. **非官方 anthropic 网关**：`output_config.effort` 是较新的官方参数，第三方兼容网关可能 400 —— 与 `web_search` 的「保存期无条件接受、生效门在解析期」同理会缓解，但仍是用户可感知的失败面。
5. **同义词撞车**：表单里将同时出现「模型三档」与「effort 档」，两者都读作「高低档」。文案必须切开：**档 = 选哪个模型；effort = 同一模型想多深**。

## 第二轮：值域从「封闭枚举」翻成「目录 + 自由值」（用户裁定）

**裁定**：常用模型与 effort 都维护一份枚举目录；模型与 effort 都支持**选择**与**自定义输入**；目录真源在 **Rust 静态表 + IPC 命令**（出 bindings 类型、进 archi 契约、后端单测可锚）。

### 后果一：变更自然劈成两半，可独立落地

```
半 A（后端，语义面）—— 必须先做，否则 effort 无处存
  effort 值域不再封闭 ⇒ store 字段是字符串（`Option<String>` / 空串=留空）
  保存期校验退化为 trim / 非空 / 长度（MUST NOT 做封闭值域校验）
  协议 shaping 照旧单点：openai → reasoning_effort:v ；anthropic → output_config:{effort:v}
  透传路径与 A2 一致（efforts{high,low} → open_session match → LoopTurn → additional_params）

半 B（前端 + 目录，选择面）—— 可与半 A 独立发布
  model high/medium/low 三框 + effort 两框 → 「可选可填」控件
  model 侧是纯前端增强（后端今天就是自由字符串，只校验 [1m] 后缀）
```

### 后果二：目录落点 = `crates/infra/agent`（推荐）而非 store / core

- **不能进 `crates/core/agent`**：该 crate 的既有明纪律是「MUST NOT 出现 agent / engine / 协议概念」，而目录本质上按协议分组（effort 值域两家不同）。
- **不宜进 `crates/infra/store`**：目录不是持久化形态，且 store 的 spec 纪律是「禁拖引擎门面 crate」；协议 wire 语义归引擎侧。
- **放 `crates/infra/agent` 的理由**：`Protocol` 与 `parse_model_spec`（`[1m]` 后缀、窗长候选）都在这里；「哪家模型认哪些 effort 值」与 shaping 函数同处一文件才不会漂移。`EngineKind` 已在此 derive `specta::Type`（`lib.rs:24`）——目录 DTO 出线有先例；静态知识表也有先例（`archi_knowledge.rs` 的 `include_str!` 骨架 + 派生词表）。

### 后果三：新增 IPC 命令要过的门（已核对）

| 门 | 具体要求 | 位置 |
|---|---|---|
| 命令登记 | `all_commands!` 宏单一登记面加一行（`generate_handler` 与 `collect_commands` 同源） | `src/commands/mod.rs:33` |
| 命令实现 | `#[tauri::command]` + `#[specta::specta]`，`Result<T, String>`（`StoreError` 不进签名） | `src/commands/agents/mod.rs` |
| 类型出线 | DTO `derive(specta::Type)` + `#[serde(rename_all="camelCase")]`；`disable_serde_phases()` 要求**线格式单一形态**（不能靠 `skip_serializing_if` 造相位差） | `src/bindings/mod.rs:41` |
| 生成物 | `bindings:export` 刷 `bindings.ts` + `bindings.zod.ts`（须 review diff） | AGENT.md |
| 契约 | `.dev-team/architecture/api/<Domain>/<FQN>.md` 新建/续写 `api-op` + `api-types`；`api:export` 生成 `.schema.json` / `INDEX.toml` / `.i.ts`；`api:check` 零 diff 门 | AGENT.md「模块 API 契约」 |
| 校验链 | `server:check` → `bindings:export` → `api:check` → `client:check` | 同上 |

### 后果四：新增标准组件的规格成本

`StandardSelect` 是**封闭选择**（无自由文本）。「可选可填」需要一个新标准组件（Base UI 1.8 已内置 `combobox` / `autocomplete` 目录，`components/ui/` 只有 `select.tsx`，需新写 `combobox.tsx` 包装 + `components/standard/` 薄壳）。`desktop-standard-components` spec 对每个标准组件有契约要求 + colocated 测试纪律（role / 可及名 / 文本 / `data-*` 查询，禁样式类名，产品源码零因测改动）。

### 目录的建议形状（待产品裁定内容，结构先定）

```
ModelCatalog                       ← 单次 IPC 取全量（静态、无参数、无 IO）
  protocols: [{
    protocol: "openai" | "anthropic",
    models: [{ value, label?, contextLength?, maxTokens?, efforts?: [..] }],
    efforts: [{ value, label? }]     ← 该协议的**建议**值域（非校验值域）
  }]
```

三条不变量（可写成后端单测）：
1. 各协议内 `value` 唯一；目录非空。
2. `efforts` 只作建议——保存期**不校验**目录成员（自定义值必须能存）。
3. 目录 MUST NOT 携带凭据 / 端点；它是纯静态参考数据（`Debug` 可派生、无遮蔽需求）。

### 值得提醒的设计张力（目录升级为领域数据的诱惑）

如果目录带**能力元数据**（上下文窗长 / 最大输出 / 是否支持思考 / effort 值域），它就能顺带把 `context_length` / `max_tokens` 从「凭记忆填数字」变成「选模型自动带出建议值」——正好治本仓 `max_tokens < 4096` 拒收与窗长分档推导那堆推导逻辑的体验痛点。代价：目录从「UI 建议」升级为「领域数据」，还要决定「建议值是否可被用户覆盖」（大概率是：预填建议、用户可改、空 = 未配置）。这条要不要一起做，是范围决策。

### 落地顺序建议（若照半 A / 半 B 切）

1. 半 A：store v4→v5（`efforts{high,low}`）+ 命令参数 + compose/facade + shaping 纯函数 + loop 透传 + 引擎日志 → 立刻可用（手填 effort 也能生效，UI 还是文本框）
2. 目录：`infra/agent` 静态表 + `list_model_catalog` 命令 + bindings + archi 契约
3. 半 B：`StandardCombobox` 标准组件 + `desktop-standard-components` spec 条目 + provider 表单接线
4. （可选）能力元数据预填 → 触碰 `context_length` / `max_tokens` 的编辑语义

## 第三轮：目录升到「名字 + effort 值域 + 能力元数据」（用户裁定）

裁定内容：目录条目 = 模型名 + 该协议 effort 建议值域 + 能力元数据（**上下文窗长 / 最大输出 / 是否支持思考**），并**顺带预填** `context_length` 与 `max_tokens`。

**明确没选的是第 4 档**：后端解析期不消费目录 —— 运行期仍走既有取值链（`spec.length`（`[1m]` 后缀）→ `context_length` → 128K 启发式；`max_tokens` → rig 名单值 → 窗长分档推导）。这条很重要：**运行期真相仍然单一**，目录只是前端参考数据，漂移的代价上限是「建议不准」，不会变成「同一窗长两套算法」。

### 目录形状随裁定细化

```
ModelCatalog
  protocols: [{
    protocol: "openai" | "anthropic",
    efforts: [ value, … ],                       ← 协议级建议值域
    models: [{
      value: "deepseek-reasoner",  label?: …,
      contextLength?: 128_000,     maxTokens?: 64_000,   ← 能力元数据（预填用）
      supportsThinking?: bool,
      efforts?: [ … ],             ← 模型级覆盖（支持思考=false 的模型不给 effort 建议）
    }]
  }]
```

三条不变量（后端单测可锚）：协议内 `value` 唯一、目录非空、`efforts` 只作建议不参与保存期校验（自定义值必须能存）。

### 撞点一：预填 vs「留空 = 未配置」（这是本次最需要裁的一处）

`context_length` / `max_tokens` 的既有语义是「留空 = 未配置（存 `None`）→ 走缺省推导」，spec 原文写着「MUST NOT 落 0 或缺省字面值」。预填会把这件事撞出三个口子：

1. **用户以为配了，其实没配**：预填的 128000 与未配置的生效值相同、语义不同；之后 vendor 改档，两者行为分叉。
2. **编辑态不能预填**：存量 `None` 行打开编辑时若显示预填数字，一保存就把「未配置」静默固化成显式值 —— 这是数据语义变更，不是 UI 便利。
3. **预填与 `[1m]` 后缀打架**：`spec.length.or(context_window)` 里后缀优先，用户填了 `claude-x[1m]` 又看到预填的 200000，会有两个数字同时"在场"的歧义（表单里 `suffixHint` 已经在处理这类「已被覆盖」提示，预填会把它放大）。

```
预填方案 A（写进输入框）      输入框显示 128000 → 保存即落库 Some(128000)   ※ 与「留空=未配置」冲突
预填方案 B（只作 placeholder） 输入框空着，占位符提示「留空 = 按 deepseek-reasoner 建议 128K」
                              → 用户不动 = None（语义零变化），动了才落值
预填方案 C（显式「采用建议」） 输入框空 + 一枚「按建议填 128K」按钮 → 一次性显式落值
```

B 与既有语义完全兼容、代价最低；C 让「采用了建议」这件事在数据上可辨；A 最"顺手"但会污染「未配置」语义。同理 `max_tokens` 还有一层：目录里的「最大输出」对 **openai 协议无意义**（`build_model` 对 openai 恒不写该字段）—— 按协议分组天然化解，但表单提示要跟上。

### 撞点二：`supportsThinking` 真正的用处是**收窄 effort 建议集**

刚否掉了把 `none`（关思考）塞进 effort 值域，所以「是否支持思考」在运行期没有消费点。它的真实价值在建议面：`supportsThinking=false` 的模型（如非思考型 chat 模型）不该在 effort 下拉里给建议值 —— 也就是 `efforts` 从**协议级**下沉到**模型级覆盖**（协议级给缺省，模型级可覆写）。这让目录从「两段并排的清单」变成「带覆盖关系的两级清单」，是目录结构里唯一有真逻辑的地方。

### 撞点三：`[1m]` 后缀 × 目录条目

最自洽的形态是**目录里直接给 `claude-x[1m]` 这种变体条目**，并把 `contextLength` 元数据填成 1M —— 这样「后缀在场」与「元数据说 1M」同源，用户选条目就等于选了后缀，不必自己手敲方括号。反过来若目录只给裸名 + 200000，用户再手加 `[1m]`，就会出现「目录说 200K、后缀说 1M、实际 1M」的三方不一致。→ 建议：**目录条目的 `value` 就是完整模型原文（可含后缀）**，不另造「后缀位」。

### 触面增补（相对前两轮）

| 层 | 增补内容 |
|---|---|
| 目录 DTO | 三块：协议 → effort 建议值域 → 模型条目（value / label / contextLength / maxTokens / supportsThinking / efforts 覆盖） |
| IPC | `list_model_catalog()`：零参数、零 IO、纯静态表读取（`Result` 只为一致性，实际恒 `Ok`） |
| 前端控件 | `StandardCombobox`（可选可填）替换 model 三框 + effort 两框；预填按撞点一的方案 |
| 前端提示 | 预填/建议值与「留空=未配置」的关系、`[1m]` 覆盖、`max_tokens` 仅 anthropic 生效 —— 三条 hint 都要重写 |
| spec | `desktop-standard-components`（新组件契约）+ `desktop-agent-management`（目录、表单、编辑语义）+ 可能的 `desktop-frontend-module-entry`（新模块入口） |
| 维护成本 | 静态表 = 手写真相，vendor 改档即过时 → MUST 在 spec 里写明「目录只作建议、不参与运行期解析、不参与保存期校验」 |

### 预填语义裁定：方案 A（选中目录条目 → 直接写入输入框）

接受的代价：预填值保存后落库为**显式值**，「未配置（`None`）」与「显式等于建议值」在数据上不可辨。为了让这件事可控，需要四条配套纪律（建议直接进 spec 的编辑语义条目，作为对既有「MUST NOT 落缺省字面值」的**显式修订**）：

1. **只由「用户主动选择目录条目」触发**。打开编辑态时 MUST NOT 自动预填——存量 `None` 行打开仍是空框，一保存仍是 `None`。唯一写入路径是用户点/选了建议值。
2. **清空即回未配置**。用户把预填值删空 → 保存 `None`（既有「留空 = 未配置」不破）。
3. **预填值随条目元数据走，不随「缺省启发式」走**。目录里 `contextLength` 是「该模型标称窗长」，不是 128K 启发式的拷贝；元数据缺席则该输入框不预填（保持空）。
4. **`[1m]` 一致性优先于输入框**：条目 `value` 含后缀时，预填的 `contextLength` MUST 与后缀语义一致（1M）；hint 仍声明后缀优先（`spec.length.or(context_window)`）。`max_tokens` 预填在 openai 协议下无意义（该协议恒不写上限）——按协议分组不预填，或预填但保留既有「仅 anthropic 生效」提示。

轻量可辨性建议（不改存储）：表单里给预填值挂一枚弱标记（如「来自目录」小字），让用户知道这是建议落值而非自己填的。这样「语义污染」留在 UI 层可见，不必扩存储。

### 这条裁定的连带影响

- `openspec/specs/desktop-agent-management/spec.md` 的「`context_length` 编辑语义 SHALL 为留空 = 未配置（存 `None`，MUST NOT 落 0 或把缺省启发式字面写库）」需要**加一条限定**：目录条目的标称窗长不属于「缺省启发式字面」，用户选择条目即显式配置。
- 前端 `/agents` 表单规格（同 spec「Agent 管理页」条目）需要补四个字段的控件形态（combobox）、预填触发时机、预填来源标记。


- ~~岔口 A 粒度~~ → **A2 随档位**
- ~~值域~~ → **目录 + 自由值，不做封闭校验**
- ~~目录层位~~ → **Rust 静态表 + IPC**
- ~~目录厚度~~ → **名字 + effort 值域 + 能力元数据（+ 预填）**；后端解析期不消费目录
- **预填语义选 B / C / A？**（placeholder 提示 / 显式采用建议 / 直接写值）
- 目录**内容**：收哪些模型（DeepSeek 系 / Anthropic 系 / GPT 系；是否含 `[1m]` 变体条目）；effort 建议值是否含 `xhigh` / `max`
- 自定义输入是否要记忆（会让目录从纯静态变可变 → 另择存储）
- 前端是否两处 effort 选择器（high / low 各一），还是先只给 High 档

## 决策记录

**A2 已定：effort 随档位（High 一套 / Low 一套）。** 理由自洽性很强 —— `phase_table` 已经把「深相位 / 快相位」编码进 `ModelLevel`，effort 只是把这条已有分野从「换模型」延伸到「同模型换深度」。两条正交轴落定：

```
provider
 ├─ models: { high: 模型名, medium: 模型名(只存不选), low: 模型名 }   ← 选哪个模型
 └─ efforts: { high: effort, low: effort }                        ← 同一模型想多深（新）
                                         │
   ModelLevel（相位决定，phase_table 供给）─┤ 唯一分发点 = SdkRunner::open_session
                                         ▼
   match model_level { High => (model_high, effort_high), Low => (model_low, effort_low) }
```

推论与待办：
- 存储形状建议 **sibling struct**（`efforts: { high, low }`）而不是把 `models` 改造成 `{name, effort}` 结构 —— 后者会让 `AgentModelTiers` 三档结构与 v5 升级链同时变形，还需要决定 `medium` 档的 effort 归属（而 `medium` 模型今天根本不参与选型）。sibling 则是一次干净加列。
- facade 旁路建议 `with_efforts(EffortsTiers)`（与 `with_max_tokens` / `with_web_search` 同型），`EngineConfig` 不动（连接字段面承诺不塌）。
- 档位 → (模型, effort) 的选取压回 `open_session` 那一个 `match`（现在 `runner.rs:203-206` 只选模型，扩成二元组即可）；`LoopTurn` 只收**已按协议 shaping 好的** wire 载荷（`Option<Value>`），loop 保持零协议判定——与 `web_search` 的「已门控最终值」纪律同构。
- shaping 纯函数建议单点放在 `sdk/`（如 `effort_params(protocol, effort) -> Option<Value>`），无 I/O、可单测，`loop` 只做 `request.additional_params = turn.effort_params.clone()`。

## 岔口 B 的新事实：两个 vendor 的 effort 值域**不是同一个集合**

| | 值域 | 默认 | 备注 |
|---|---|---|---|
| DeepSeek（OpenAI 格式 / Anthropic 格式） | `low / high / max` | `high` | `medium` 会被映射成 `high`（假三档） |
| Anthropic 官方 `output_config.effort` | `low / medium / high / xhigh / max` | `high`（Opus 5.5 / Haiku 5.5 为 `medium`） | 已 GA、无 beta header；`xhigh` 是较新加档，非所有支持 `max` 的模型都支持 `xhigh` |

→ 封闭枚举只能取**交集或超集**：`low|medium|high` 两边都认但表达不了 `max`；`low|medium|high|max` 两边都认（`xhigh` 留给未来加档）。开放字符串透传违背本仓「封闭值域 + 保存期校验」的既有纪律（`context_length` / `max_tokens` 都不透传）。
→ **`none`（关思考）不应塞进 effort 值域**：Anthropic 文档明确 `thinking:{type:"disabled"}` 在新模型各 effort 档都返回 400（自适应思考不可关），DeepSeek 的开关是另一条 `thinking.type` 参数。混进来的结果是「同一个字段在两个 vendor 上一个改强度、一个改开关」，语义必然打滑。

## 一个原本没预见的耦合：effort × 提示缓存（只影响 anthropic 臂）

- 本仓 anthropic 模型是**开着自动缓存**的（`sdk/runner.rs::with_prompt_cache` → `wire.with_automatic_caching()`）。
- Anthropic 官方文档：「**在依赖缓存的会话里要保持 top-level effort 恒定**，改 top-level effort 会让缓存前缀失效」。
- 现状恰好是安全的：effort 随 `ModelLevel` 走，而 `ModelLevel` 在一个会话内不换档（反馈边续注同档；相位各自开会话）。但两条边要写进设计约束：
  1. `compact`（摘要）请求若带**不同** effort，会让主会话的缓存前缀落地不一致 → 要么摘要请求沿用同档 effort，要么显式不带（今天的 `additional_params: None`）。
  2. 未来若做「同一会话内换 effort」（DeepSeek 支持 per-request，Anthropic 有 per-message beta），会撞上缓存纪律，不是免费功能。

## 建议的落地形状（A2 定稿后）

store 加 `efforts: EffortsTiers`（v4→v5 加列，`new()` 参数面不动 + `with_efforts`）；compose 原样带出；facade `with_efforts`；`open_session` 的 `match` 产出 `(model, effort)`；`effort_params(protocol, effort)` 单点 shaping；`LoopTurn.effort_params` → `request.additional_params`；引擎日志行加 `effort_high=… effort_low=…` 或该轮生效值 `effort=…`；前端 provider 表单两个 `StandardSelect`（high / low，各带「留空 = 用 provider 默认」）。规模仍接近一次 `anthropic-web-search`，只是从 1 个字段变 2 个字段。

## 仍待定（下一步可裁）

1. **目录内容**：收哪些模型族（DeepSeek 系 / Anthropic 系 / GPT 系）；是否直接给 `[1m]` 变体条目；effort 建议值是否含 `xhigh` / `max`。
2. **自定义值的记忆**：用户手填过的模型名 / effort 是否要「下次也出现在建议里」——会把目录从纯静态变可变，需另择存储（store 表 vs localStorage）。
3. **两个 effort 输入位**：high / low 各一个，还是先只给 High 档（Low 档默认留空 = 用 provider 默认）。
4. **是否同一批做**：半 A（后端 effort 列 + 透传）与半 B（目录 + combobox）可分两次变更落地；目录/预填属半 B。
5. **`compact` 摘要请求是否带 effort**（anthropic 臂的缓存纪律，见「effort × 提示缓存」节）。

## 附加裁定：`context_length` / `max_tokens` 改用中文标签

范围：仅前端 provider 表单的**可见标签**（`provider-panel.tsx` 的 `PROVIDER_FIELDS[].label`，当前分别是 `context_length` / `max_tokens`），存储键、IPC 参数名、bindings 字段名一概不动。

已核实的波及面（结论：比预想安全）：

| 面 | 现状 | 是否被这条改动影响 |
|---|---|---|
| 表单字段 id / `data-testid` | `provider-context-length` / `provider-max-tokens`（与 label 分离） | **不受影响**（测试全走 testid，零 `getByLabelText`） |
| 提示文案 | 三处字面量引用了英文键：`max_tokens：输出上限，留空 = 按 context_length 分档推导…`、`后缀优先于 context_length`、placeholder `留空 = 按 context_length 分档` | **受影响**：标签变中文后，提示里裸写英文键会读起来割裂 → 需同步改述 |
| 测试 | `provider-panel.test.tsx:685` `toContain('留空 = 按 context_length 分档推导')`、`:496` `toContain('后缀优先于 context_length')` | 若同步改提示文案，**这两条断言要跟着改** |
| spec | `desktop-agent-management` 第 125/127 行用 `context_length` / `max_tokens` 指代字段（**语义指代**，非 UI 标签承诺） | 可不动；若想让 spec 与 UI 对齐，补一句「字段展示名为中文，键名不变」 |

顺带一条同族观察（未裁定）：表单里还有 `model high` / `model medium` / `model low` / `base_url` / `api_key` / `protocol` 这些英文或混合标签，`API` 风格的键名属产品语气选择。若这次统一中文，建议一次改齐（同一 `PROVIDER_FIELDS` 表驱动，成本几乎为零）；若只改这两项，表单会呈「中文与英文混排」，反而更不整齐。

建议的文案（供裁定，未落字）：`context_length` → 「上下文窗长（token）」；`max_tokens` → 「输出上限（token）」。

## 追加裁定：向下兼容（升级后不破坏已有 Agent 配置）

「已有 Agent 配置」在库里共有**两个**持久面：provider 记录（全局库，`AgentProviderRecord`，native_model id 5 / v4）与 **会话装配快照**（workspace 库，`SessionConfigSnapshot` 嵌在 `SessionRecord`，id 7 / v2）。这仓的落库编码是 **native_model 默认 bincode（非自描述，按字段顺序编码）**，源码里写死了纪律：

> 「字段集演进 MUST 同时 bump version（配 `from` 升级链）——`#[serde(default)]` 不足以兼容更短的旧行（bincode 读到 EOF 即报错）」
> 「bincode 嵌套结构变布局必须**整结构换型**（serde `default` 不救 bincode 缺字段）」— `SessionConfigSnapshotV1` 就是为这句话存在的

所以兼容性要过四道关，逐关都有既有先例可抄：

### 关 1：provider 记录 v4 → v5（必做）

```
#[native_model(id = 5, version = 4)]  AgentProviderRecord      ← 现形态
   改名 → AgentProviderRecordV4（pub(crate)、不进库模型组、不出查询面）
新形态 #[native_model(id = 5, version = 5, from = AgentProviderRecordV4)]
   + efforts: EffortsTiers（sibling 字段，MUST NOT 塞进 AgentModelTiers）
双向 From：V4→V5 补 efforts 空值；V5→V4 丢 efforts（降级不作数据承诺，沿 web_search 先例）
```

**为什么不能把 effort 塞进 `AgentModelTiers`**：它是**未被版本化的嵌套 struct**（`models` 字段内嵌），往里面加字段等于改动 bincode 布局却没有 `from` 链可依——存量行读到会错位/EOF。这正是「sibling 字段」而非「改 models 结构」的硬理由。

### 关 2：会话快照要不要记 effort（本次真正的新发现）

`ComposedTurn::begin` **已经在做同一个 match**（`compose.rs:305-310`）：

```rust
let model = self.models.as_ref().map(|(high, low)| match ctx.model_level {
    agent::ModelLevel::High => high.clone(),
    agent::ModelLevel::Low  => low.clone(),
});
let snapshot = store::SessionConfigSnapshot { engine, agent_instance_id, model, permission_mode };
```

如果「档位 → (模型, effort)」在 `open_session` 里成形，而这个 match 不跟着扩，就会出现「快照记了模型、没记 effort」的不对称——续会话/审计看不到当时跑的深度。两条路：

| | 做法 | 代价 |
|---|---|---|
| **记** | `SessionConfigSnapshot` 加 `effort: Option<String>` + 新 `SessionConfigSnapshotV2` 旧形态 + `SessionRecord` v2→v3（id 7）+ 双向 From | 多一次版本演进；`model_test.rs` 里 SessionRecord 版本锚点跟着改 |
| **不记** | 快照不动，effort 只进请求体与引擎日志 | 零迁移；但「这轮用什么深度跑的」只能查引擎日志，续会话比对无据 |

快照的既有用途只有一条（`compose.rs:340`）：`record.config_snapshot.engine != self.store_kind` → 跨引擎续会话拒收。也就是说**不记 effort 不破坏任何现存行为**，只是少一份自证。倾向：**先不记**（守「本次变更只加一列」），把它写进将来「effort 审计」的动机里。

### 关 3：线格式（JSON / IPC）兼容

- 新字段挂 `#[serde(default)]` ⇒ 旧 JSON（前端夹具、旧快照 JSON、`restamp_session_config` 回灌）缺键读出空值。
- 但 `bindings.ts` 的 `AgentProviderRecord` 会多一个**必填**字段（`disable_serde_phases()` 保证单一形态，不出可选性分裂）⇒ 前端 19 处 `models: { high…}` 夹具与 `use-agent-providers.ts` 的 `AgentProviderSaveInput` 必须补 `efforts`，编译期集体报错、不会静默漏（这是好事）。
- `bindings.zod.ts` 是 `strictObject` ⇒ IPC 回来的记录必须带该键（升级后后端恒带），前端旧夹具不补就红。

### 关 4：运行期行为零变化（这才是「不破坏」的实证面）

判据要能写进测试：**存量 provider（`efforts` 全空）发起的请求体与今天逐字相同**。

- `efforts` 空 ⇒ `effort_params()` 返 `None` ⇒ `additional_params` 保持 `None` ⇒ wire 不多任何键（openai 臂不出现 `reasoning_effort`，anthropic 臂不出现 `output_config`）。这恰好也是「非 DeepSeek 的第三方兼容网关不被污染」的保障。
- `EngineFacade::with_efforts(默认空)` ⇒ `new()` 调用点零改（门面既有 builder 全是这种缺省安全的形态）。
- `AgentProviderRecord::new` 参数面不动（加 `with_efforts` 链式）⇒ Rust 侧 **44 处** `::new` 调用点零改；但 **16 处直接结构体字面量**（多为测试夹具，`describe_test.rs` / `envelope_test.rs` / `db_tool_test.rs`）会因缺字段而编译失败——必须逐个补 `efforts` 空值，这是升级链之外的机械面。

### 兼容性验证清单（建议直接充当 tasks 的验收项）

1. 用 v4 载荷字节 `native_model::decode::<AgentProviderRecord>` ⇒ 成功、`efforts` 空、其余字段逐字段相等（沿 `v3到v4升级读入web_search为false且其余字段逐字段相等` 先例）
2. v1→v2→v3→v4→**v5** 四级链与「版本机制直接产出」逐字段一致（沿 `v1到v2到v3到v4三级链升级对齐` 先例）
3. V5→V4 降级丢 efforts、其余保真；再升级回 V5 ⇒ efforts 空（降级不作承诺）
4. 版本头锚点：`<AgentProviderRecord as Model>::native_model_version() == 5`、id 仍 5、V4 中转形态 version 4 —— `model_test.rs` 里 39 处相关锚点会红，需同步（这是**预期**的红）
5. JSON 缺 `efforts` 键 ⇒ 读出空值；`efforts` 非法载荷（字符串 / 数字 / null）⇒ `Err`
6. 遮蔽 Debug 补 efforts 位，api_key 位仍遮蔽
7. 运行期：`efforts` 空 + 两协议 ⇒ `CompletionRequest.additional_params` 恒 `None`（请求体零新增键）
8. 前端 19 处夹具 + `AgentProviderSaveInput` 补字段后 `vp check` 全绿；`bindings:export` 连跑两次零 diff

### 版本纪律

`packages/desktop/package.json` patch +1（每个 desktop 源码变更都要）；`src-tauri/Cargo.toml` 的 crate 版本行**不跟随**；release 走 workflow，不手打 tag。

### 关 2 裁定：**不记**（会话装配快照不动）

```
SessionConfigSnapshot       不动（不加 effort 字段）
SessionRecord               不动（不加 v2→v3，id 7 / v2 保持）
SessionConfigSnapshotV1    不需新造旧形态
```

理由（落成判据，不只记结论）：

1. **不记不破坏任何现存行为**：快照当前唯一消费点是 `compose.rs:340` 的 `record.config_snapshot.engine != self.store_kind`（跨引擎续会话拒收）。该判定与 effort 无关，缺这一列也不会误判。
2. **快照不是续会话的输入**：续会话的重建源是**会话全史转录**（`resolve_resume` → `resume::rebuild`），不是快照。所以「当时用多深的 effort」不影响续跑正确性——续跑的 effort 由**当次档位重新解析**，本就该跟当前 provider 配置走，而非复刻历史。
3. **省掉一条版本链**：本次因此只有**一条**版本演进（provider v4→v5），`model_test.rs` 里 SessionRecord 的版本锚点（`native_model_version() == 2`）保持不动。
4. **可观测性不缺口**：该轮生效 effort 仍进引擎日志（与 `protocol=… max_tokens=… web_search=…` 同列），排查有据。

遗留动机（写进将来变更的引子，非本次范围）：日后若要做「会话级 effort 审计 / 回放一致性」，才是 `SessionConfigSnapshotV2` + `SessionRecord` v3 的正当理由；届时会撞上「历史行填 `None` 还是当时实际值」——答案是 `None`（历史行无从得知，MUST NOT 事后推测填充）。

**本次版本演进面收敛为：provider 记录 v4 → v5，一条链、两条 `From`。**
