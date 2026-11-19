# 提案: provider-effort-knob

> **变更**: provider-effort-knob（provider 记录增加「同一模型想多深」的 effort 双档 + 模型 / effort 可选可填目录）
> **日期**: 2026-11-19
> **状态**: draft（explore 决议总览 ①–⑦ 与三轮追加裁定（预填方案 A / 兼容 v4→v5 / 会话快照不记 effort / field 标签中文化）已锁定；本提案补裁 explore 末节 5 项遗留，见「遗留项裁定」）
> **来源**: 本 change 目录 `explore.md`（requirement 类型；rig 0.43.0 源码级复核证据见其「落点」与「岔口 B 的新事实」两节）
> **性质**: **加列 + 加建议面**——provider 记录多一列（`efforts{high, low}`，一条 v4→v5 版本演进），运行期多一条旁路载荷（按协议 shaping 后进 `additional_params`），前端多一份**静态建议目录**（不参与保存期校验、不参与运行期解析）。**存量行为零变化**：`efforts` 留空 ⇒ 请求体与今天逐字相同。
> **不含**: 会话级 effort 审计（`SessionConfigSnapshot` 不动，决策 ⑨）、per-turn / 相位级 effort 覆盖、`AgentRunParams` 的 effort 参数与前端档位选择器（管理页边界条 1 的剩余半边）、目录的持久化自定义值记忆。

---

## 问题

### 现状链路（四段，门控单点先例已在）

```
AgentProviderRecord（store v4，全局库）
   models{high,medium,low} · protocol · context_length · max_tokens · web_search
        │
        │  compose::resolve_engine                 ← 门控单点（web_search && protocol==Anthropic）
        ▼
ResolvedEngine{ EngineConfig{protocol,api_key,base_url,model_high,model_low},   ← 连接必需项（封闭面）
                context_window, max_tokens, web_search }                        ← 旁路载荷
        │
        │  EngineFacade::with_context_window / with_max_tokens / with_web_search
        ▼
SdkRunner::open_session
   ctx.model_level → High→model_high / Low→model_low       （档位唯一消费点，runner.rs:297-300）
        │
        ▼
loop：CompletionRequest{ …, max_tokens: None, additional_params: None }   ← 现成的注入点
```

provider 记录今天只有「**选哪个模型**」（三档模型名），没有「**同一模型想多深**」。DeepSeek thinking_mode 与 Anthropic `output_config.effort` 把后者叫 effort；rig 0.43 的两条 wire 都有 `#[serde(flatten)] additional_params` 透传缝（openai chat 请求体顶层 / anthropic `AnthropicCompletionRequest`），故**不需要升 rig**。

### 病灶一 · 「深相位 / 快相位」只编码在模型名上，同一模型无法分深度

`phase_table` 已经把相位分野压进 `ModelLevel`（proposal / dev-design / test-design / code-review / acceptance = `High`；implement / test-gen = `Low`），但 `ModelLevel` 到引擎的唯一消费点只做「换模型名」。同一家 provider 若 high / low 填同一个模型（省钱、或只有一档可用），两档就退化为同一强度——**「想多深」这根轴在数据面不存在**。DeepSeek 与 Anthropic 都提供了这根轴的 wire 参数，本仓没有承载它的字段。

### 病灶二 · `medium` 档「只存不选」，但缺的正是它本该表达的东西

`AgentModelTiers` 有 high / medium / low 三档，`EngineConfig` 只有 `model_high` / `model_low`，表单 medium 的 placeholder 写着「暂未启用」。三档的直觉（快 / 中 / 深）落在模型切换上表达力有限：deepseek 的 `low/medium/high` 是**假三档**（`medium` 映射成 `high`），真正的深度档是 `low / high / max`。结论：深度不是第二次模型选择，而是**同一模型的参数**。

### 病灶三 · 模型名与 effort 全是「凭记忆手敲的字符串」，无处查建议

三档模型名是自由文本（今天只校验 `[1m]` 后缀），`context_length` / `max_tokens` 是凭记忆填的数字（`max_tokens < 4096` 会被保存期拒收，窗长决定输出上限分档）。本仓没有任何「这家协议的常用模型叫什么 / 它的标称窗长与最大输出是多少 / 它支持哪些 effort 值」的参考面——用户接上一个新端点的第一步就得翻 vendor 文档抄五六个字段。

### 现状实测（逐条在案）

| 事实 | 读数 / 位置 |
|---|---|
| effort 注入点现成 | `sdk/loop.rs` 请求构造 `additional_params: None`（每轮一条）；`sdk/compact.rs` 同为 `None` |
| 档位 → 模型的唯一消费点 | `sdk/runner.rs::open_session` 的一处 `match open.ctx.model_level`（语义上扩成 `(模型, effort)` 二元组即可，无第二个分发点） |
| 连接配置封闭面 | `sdk/config.rs::EngineConfig` 注释写明「可空调参 `context_length` / `max_tokens` MUST NOT 进本结构」——effort 属同一族，走门面旁路 |
| 落库编码 | native_model 默认 **bincode**（非自描述、按字段顺序）；源码纪律写明「字段集演进 MUST bump version 配 `from` 升级链，`#[serde(default)]` 不足以兼容更短旧行」 |
| `AgentModelTiers` 是**未版本化的嵌套 struct** | 往里加字段 = 改 bincode 布局却无 `from` 链可依 ⇒ effort **MUST 为 sibling 列**，MUST NOT 塞进 `models` |
| 构造器面 | `AgentProviderRecord::new` 有 44 处调用点（零改）；**结构体字面量 6 处**（`db_tool_test` / `describe_test` / `model_test` ×3 / `query_test`）需补 `efforts` 空值；`envelope_test` / `store_test` 的夹具走 `::new`，不受影响 |
| 命令面 | `save_agent_provider` 现 10 参（含 `State`；末位 `web_search`），`all_commands!` 单一登记面；**specta `SpectaFn` 只实现到 `fn(T1..T10)`（`State` 计入其一）——参数位已满**，四项可空调参平铺会越界（实现期回溯 1） |
| 前端控件 | `components/ui/` 有 `select.tsx`、无 `combobox.tsx`；`@base-ui/react@1.8` 已内置 `combobox` 目录 ⇒ 「可选可填」需新写底座包装 + 标准层薄壳 |
| anthropic 缓存 | `sdk/runner.rs::with_prompt_cache` → `wire.with_automatic_caching()`：**同一会话内改 top-level effort 会让缓存前缀失效** |
| rig 特殊行为 | `providers/openai/wire/chat.rs::refuse_tools_while_reasoning`：GPT-6 系在 Chat Completions 上带 tools 时**只接受** `reasoning_effort: "none"`（其余直接拒发） |

---

## 提案

### 决策执行表（explore 决议 + 本提案补裁；末列给出「为什么不是另一个」）

| # | 决策 | 要点 | 被拒备选与理由 |
| --- | --- | --- | --- |
| ① | effort **随档位**（A2） | `efforts{high, low}` sibling 列，与 `ModelLevel` 共用 `open_session` 那一个 `match`：`High → (model_high, effort_high)` / `Low → (model_low, effort_low)` | **被拒 A1（provider 单值）**：所有轮同一深度，等于放弃 `phase_table` 已经编码好的深 / 快分野。**被拒 A3（相位级进 `phase_table`）**：effort 变成工作流语义，16 个 prompt 各配一档，触面最大且与「provider 属性」的归属错位。**被拒 A4（medium 让位给 effort）**：结构性重构，需同时变形 `AgentModelTiers` 三档结构与 v5 升级链 |
| ② | 值域**不封闭** | 保存期只 `trim`（空串 = 留空 = 不发），MUST NOT 做封闭值域校验——自定义值必须能存 | **被拒封闭枚举 `low\|high\|max`**：两 vendor 值域不是同一集合（DeepSeek `low/high/max`；Anthropic 官方含 `medium` / `xhigh`），交集表达不了 `max`、超集会放行对方不认的值。**被拒「只留空 / 两档」**：目录化建议面已经承担了引导职责，再加一层枚举只为拦用户，收益低于成本 |
| ③ | `none`（关思考）**不进值域** | 关思考不属于 effort；DeepSeek 的开关是另一条 `thinking.type`，Anthropic 新模型禁关 | **被拒把 `none` 并进 effort**：同一字段在两家一个改强度一个改开关，语义必然打滑；且 anthropic 新模型各 effort 档传 `thinking:{type:"disabled"}` 即 400 |
| ④ | 目录真源 = **Rust 静态表 + IPC** | `crates/infra/agent/src/model_catalog.rs` 静态表 + `list_model_catalog()` 零参数零 IO 命令，出 bindings、进 archi 契约、后端单测可锚 | **被拒进 `crates/core/agent`**：该 crate 明纪律「MUST NOT 出现 agent / engine / 协议概念」，而目录按协议分组。**被拒进 `crates/infra/store`**：目录不是持久化形态，且「哪家模型认哪些 effort 值」应与 shaping 同处一 crate 才不漂移 |
| ⑤ | 目录厚度 = **名字 + effort 建议值域 + 能力元数据** | 条目携 `contextLength` / `maxTokens` / `supportsThinking`，并据此**预填**；`supportsThinking=false` 的条目把 effort 建议**收窄为空**（模型级覆盖协议级） | **被拒「两段并排清单」**（协议级 effort 建议不与模型条目关联）：`supportsThinking` 在运行期没有消费点，它唯一的真实价值就是收窄建议集。**被拒「目录只给名字」**：那样预填与窗长提示都无处可来，目录退化为自动补全 |
| ⑥ | 后端**不消费**目录 | 运行期仍走既有取值链（`spec.length` → `context_length` → 128K；`max_tokens` → rig 名单 → 窗长分档）；保存期不校验目录成员 | **被拒「目录进解析期」**：一旦目录参与运行期解析，「同一窗长两套算法」就成立，vendor 改档即变成正确性缺陷而非建议偏差 |
| ⑦ | 预填 = **方案 A（选中条目直接写入输入框）** | 四条纪律兜住语义污染（见下），UI 挂「来自目录」弱标记 | **被拒方案 B（只作 placeholder）**：用户不动就永远停在「未配置」，选择条目这个动作没有任何数据落点，目录的价值被砍半。**被拒方案 C（显式「采用建议」按钮）**：多一次点击、多一处状态，与「选中即带出」的直觉相反 |
| ⑧ | 存储演进 = **一条链（v4 → v5）** | 现 v4 形态改名 `AgentProviderRecordV4` 作中转，新记录 `version = 5` + `efforts` sibling 列 + 双向 `From`；`new()` 参数面不动（44 处零改）+ `with_efforts` | **被拒「不改版本、只靠 `#[serde(default)]`」**：bincode 非自描述，OLD 行更短 → 读到 EOF 即错（源码纪律原文）。**被拒「effort 塞进 `AgentModelTiers`」**：未版本化嵌套 struct 加字段使 bincode 布局静默错位 |
| ⑨ | 会话快照**不记** effort | `SessionConfigSnapshot` / `SessionRecord` 零改动（无 v2→v3） | **被拒「快照加 effort 列」**：快照唯一消费点是「跨引擎续会话拒收」，与 effort 无关；续会话重建源是会话全史转录而非快照（effort 本就该按当次档位重新解析）。多一条版本链换一份「当时用多深」的自证，性价比不成立 |
| ⑩ | 表单字段标签**中文化（仅两项）** | `context_length` → 「上下文窗长（token）」、`max_tokens` → 「输出上限（token）」；存储键 / IPC 参数名 / bindings 字段名一概不动，随提示文案同步改述 | **被拒「整表一次改齐」**：`model high` / `protocol` / `base_url` / `api_key` 属产品语气选择，不在本次问题域内；只改两项的代价是两条既有断言随动，最小切口 |

### 落地形状（A2 定稿）

```
provider 记录（v5）
 ├─ models:  { high, medium(只存不选), low }      ← 选哪个模型
 ├─ efforts: { high, low }  （空串 = 留空 = 不发）  ← 同一模型想多深（新）
 └─ protocol / context_length / max_tokens / web_search
                          │
   ModelLevel（相位决定，phase_table 供给）
                          │  唯一分发点 = SdkRunner::open_session 的那一个 match
                          ▼
   High ⇒ (model_high, efforts.high)      Low ⇒ (model_low, efforts.low)
                          │
                          ▼
   effort::params(protocol, effort) -> Option<serde_json::Value>   ← 协议 shaping 纯单点
        openai    → { "reasoning_effort": v }
        anthropic → { "output_config": { "effort": v } }
                          │  留空 ⇒ None（请求体零新增键）
                          ▼
   PumpPayload.effort_params → LoopTurn.effort_params → request.additional_params
                          │
                          ▼
   provider_tool（web_search）在同一个 additional_params 上按 key 合并（rig `merge_params`）
```

三条不变量（可单测锚）：

1. **协议判定单点**：`effort::params` 是 shell 形状的唯一分支，`loop` / `runner` / 门面 / 组合根一律持「已 shaping 的 wire 载荷」（与 `web_search` 的「已门控最终值」同构）。
2. **留空零痕迹**：两档皆空 ⇒ `effort_params = None` ⇒ `additional_params` 保持 `None` ⇒ 请求体与今天**逐字相同**（openai 臂不出现 `reasoning_effort`，anthropic 臂不出现 `output_config`）。
3. **effort 在一个会话内恒定**：档位在轮边界确定（反馈边续注同档、各相位各自开会话），故 anthropic 自动缓存的「top-level effort 恒定」纪律由结构保证，无需运行期检查。

### 模型目录（`ModelCatalog`）

```
ModelCatalog { protocols: [{
    protocol: "openai" | "anthropic",
    efforts: [ … ],                       ← 协议级**建议**值域（非校验值域）
    models: [{
      value: "deepseek-reasoner",         ← 完整模型原文（可含 [1m] 后缀）
      contextLength?, maxTokens?,         ← 能力元数据（预填用）
      supportsThinking: bool,
      efforts?: [ … ]                     ← 模型级覆盖（Some([]) = 显式「无建议」）
    }]
}]}
```

五条不变量（后端单测锚定）：

1. 各协议内 `value` 唯一；协议清单非空、每条协议 `models` 非空。
2. `efforts` 只作**建议**：保存期 MUST NOT 校验目录成员（自定义值必须能存）。
3. 目录 MUST NOT 携带凭据 / 端点（纯静态参考数据，`Debug` 可派生、无遮蔽需求）。
4. 条目 `value` 含 `[1m]` 后缀 ⇒ 其 `contextLength` MUST 为 `1_000_000`（目录内不自相矛盾；`value` 就是完整模型原文，不另造「后缀位」）。
5. `supportsThinking = false` 的条目 MUST 给 `efforts = Some([])`（显式空，MUST NOT 靠继承协议级建议表达「无建议」）。

目录内容（首版）由实现期按 vendor 当前文档手填，**形状与不变量是本 spec 的对象，名单不是**：首版必含 DeepSeek 系与 Anthropic 系的常用条目（含 `[1m]` 变体条目），`openai` 协议侧给出 `low / high / max` 建议、`anthropic` 协议侧给出官方 `low / medium / high / xhigh / max` 建议。GPT-6 系**不进 openai 协议建议**（rig `refuse_tools_while_reasoning` 使该族在 Chat Completions 带 tools 时只接受 `reasoning_effort: "none"`，而 `none` 不在本变更值域内，见风险 R7）。

### 预填语义（方案 A 的四条纪律，作为对「留空 = 未配置」的显式限定）

1. **只由「用户主动选择目录条目」触发**：打开编辑态 MUST NOT 自动预填——存量 `None` 行打开仍是空框，一保存仍是 `None`。
2. **清空即回未配置**：用户把预填值删空 → 保存 `None`（既有「留空 = 未配置」不破）。
3. **预填值随条目元数据走，不随缺省启发式走**：目录里 `contextLength` 是「该模型标称窗长」，不是 128K 启发式的拷贝；元数据缺席则该输入框不预填（保持空）。`openai` 协议下 `maxTokens` 不预填（该协议恒不写上限）。
4. **`[1m]` 一致性优先于输入框**：条目 `value` 含后缀时预填的 `contextLength` 与后缀语义一致（1M）；既有提示仍声明「后缀优先于 `context_length`」。

配套：表单给预填值挂一枚弱标记（「来自目录」小字），使「建议落值」在 UI 层可见——语义污染留在展示层，不扩存储。

### 半 A / 半 B 同批，内部按序落地

| 序 | 内容 | 完成即可用 |
|---|---|---|
| 1 | **半 A**：store v5（`efforts`）+ 命令参数 + compose / 门面 / runner / shaping / loop 透传 + 引擎日志 | 手填 effort 即生效（UI 还是文本框） |
| 2 | 目录：`model_catalog.rs` + `list_model_catalog` 命令 + bindings + archi 契约 | 前端可查建议 |
| 3 | **半 B**：`StandardCombobox` + 五处接线（model ×3 / effort ×2）+ 预填 + 中文标签 + 提示改述 | 可选可填、选中即带出 |

分批只影响实现顺序，不切变更：半 A 单独存在时「effort 生效但只能手敲」是**可用**状态，不是半成品。

### 明确不做（知情边界，非遗漏）

- **会话级 effort 审计 / 回放一致性**：`SessionConfigSnapshot` 不记 effort（决策 ⑨）。「这轮用什么深度跑的」查引擎日志（与 `protocol=… max_tokens=… web_search=…` 同列）。
- **per-turn / 相位级 effort 覆盖**：effort 随 `ModelLevel` 走，MUST NOT 在会话内换档；将来做 per-turn effort 会撞上 anthropic 缓存纪律（顶版 effort 变更使缓存前缀失效），不是免费功能。
- **`AgentRunParams` 的 effort 参数与前端档位选择器**：管理页边界条 1 的剩余半边保持为后续迭代（本次 effort 是 provider 属性，不是 per-run 参数）。
- **自定义值的记忆**：用户手填过的模型名 / effort 不回流建议（目录保持纯静态，不引入第二条存储）。
- **摘要请求带 effort**：`sdk/compact.rs` 保持 `additional_params: None`（摘要请求不重复主会话前缀，带 effort 只增成本）。
- **目录参与运行期解析 / 保存期校验**（决策 ⑥）。
- **`none`（关思考）入口**（决策 ③）：关思考属另一条参数族，不在本变更。
- **真机核验门**：本期无 DeepSeek / 官方 Anthropic 凭据，effort 的实际深度效果**不作实现期门项**，与其余手工项统一并入归档后人工验收（见「验收面口径」）。

---

## 能力

### 新增的能力

**无。** effort 是 provider 记录的字段语义、请求体的一条旁路载荷、前端表单的一项控件；目录是 provider 管理页的取数面；`StandardCombobox` 是既有标准组件层的一员。拆独立能力只会造出跨能力悬空契约。

### 修改的能力

- **desktop-agent-management** —
  - **ADDED**「模型目录（ModelCatalog）静态建议表与 IPC 命令面」：目录形状、五条不变量、`list_model_catalog` 零参数零 IO 命令、**目录不参与保存期校验与运行期解析**、零凭据。
  - **MODIFIED**「全局 agent 管理命令面」：provider 字段面增 `efforts{high, low}`（空串 = 留空；保存期只 `trim`，MUST NOT 做封闭值域校验）；四项可空调参（`context_length` / `max_tokens` / `web_search` / `efforts`）收成末位一个 `AgentProviderTuning` 组（参数位上限所迫，见实现期回溯 1）；存储 v4 → v5（现 v4 改名 `AgentProviderRecordV4` 作中转，`new()` 参数面不动 + `with_efforts`）——`models` 三档结构 MUST NOT 变形。
  - **MODIFIED**「Agent 管理页」：三档模型与两档 effort 改为 `StandardCombobox`（可选可填）、目录驱动的建议与预填（四条纪律）、`context_length` / `max_tokens` 展示名中文化、目录取数失败降级为「无建议」。
  - **MODIFIED**「本期边界与后续路线」：边界条 1 改写——effort 已由 provider `efforts`（随档位）落地，`AgentRunParams` 的 effort 参数与前端档位选择器仍为后续。
- **desktop-agent-execution** —
  - **ADDED**「SDK 引擎 effort（思考强度）旁路与协议 shaping」：`efforts{high, low}` → 档位 match 的 `(模型, effort)` 二元组 → `effort::params(protocol, effort)` 纯单点 → `LoopTurn.effort_params` → `request.additional_params`；留空 ⇒ 零新增键；两协议 wire 形态；引擎日志 `effort=…`；摘要请求不带 effort；会话内 effort 恒定（anthropic 缓存纪律）。
  - **MODIFIED**「引擎门面与 EngineKind 参数选择」：门面旁路载荷由三项扩为四项（增 `with_efforts`），`EngineConfig` 连接配置字段面与 `runner_for` 唯一 match 点仍不塌。
  - **MODIFIED**「SDK 租户 MVP 边界与已知限制」：增五条边界（`none` 不在值域 / `output_config` 与结构化输出的 typed 字段冲突 / 第三方网关 400 / openai 格式思考回放未验证 / 输出上限随 effort 漂移）。
- **desktop-standard-components** —
  - **ADDED**「StandardCombobox 标准组合框契约」：可选可填（自由文本 + 建议清单）、值透出、清空语义、弹层与输入同一控件。
  - **MODIFIED**「standard 组件单元测试纪律」：colocated 测试清单增 `combobox.test.tsx`，断言面补组合框分支。

### 其余能力零改动

- **desktop-workspace-store**：`SessionConfigSnapshot` / `SessionRecord` 零改动（决策 ⑨）；转录单表零改（effort 不进事件）。
- **desktop-ipc-type-bindings**：绑定生成机制零改，产物经 `bindings:export` 再生成。
- **desktop-frontend-module-entry**：新增文件不改变任何目录入口的导出面（`views/agents/index.ts` 仍只导出 `AgentsView`）。
- **desktop-crate-layout / repo-archi-model**：不新增 crate、不新增跨 crate 依赖边（`infra/agent` 已在依赖 store；目录 DTO 用 store 的 `AgentProviderProtocol`）。

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/infra/store/src/model.rs` | 增 `AgentEffortTiers{high, low: String}`（`serde` camelCase + `specta::Type`）；现 `AgentProviderRecord`（v4）改名 `AgentProviderRecordV4`（原样不动、`pub(crate)`、作升级链中转）+ 新 `AgentProviderRecord`（`#[native_model(id = 5, version = 5, from = AgentProviderRecordV4)]`，增 `#[serde(default)] efforts: AgentEffortTiers`）；补 V4↔V5 双向 `From`（升级补空、降级丢列）；`new(...)` 现有参数面不动 + `with_efforts(AgentEffortTiers)`；手写遮蔽 `Debug` 增 `efforts` 位（api_key 位仍遮蔽）；`AgentModelTiers` 的文档注释里「effort 为后续迭代」一句随本变更改写（**结构零改**） |
| `crates/infra/agent/src/sdk/effort.rs`（新） | 常量面（`OPENAI_EFFORT_KEY = "reasoning_effort"` / `ANTHROPIC_OUTPUT_CONFIG_KEY = "output_config"` / `ANTHROPIC_EFFORT_KEY = "effort"`）+ `params(protocol, effort: &str) -> Option<serde_json::Value>` 纯函数（空白 / 空串 ⇒ `None`；openai → `{reasoning_effort: v}`；anthropic → `{output_config: {effort: v}}`） |
| `crates/infra/agent/src/sdk/mod.rs` | 挂载 `effort` 模块与单测模块 |
| `crates/infra/agent/src/model_catalog.rs`（新） | `ModelCatalog` / `CatalogProtocol` / `CatalogModel` DTO + 静态表 + `catalog() -> ModelCatalog`（零 IO、纯静态）；五条不变量的单测 |
| `crates/infra/agent/src/lib.rs` | 挂载并导出 `model_catalog::{catalog, ModelCatalog, CatalogProtocol, CatalogModel}`；`EngineFacade` 增 `efforts: AgentEffortTiers` 字段 + `with_efforts`（四个构造点补齐） |
| `crates/infra/agent/src/compose.rs` | `ResolvedEngine` 增 `efforts: AgentEffortTiers`（provider 原样带出，**零门控**——两协议都有 effort）；`runner_for` 链加 `.with_efforts(resolved.efforts)` |
| `crates/infra/agent/src/sdk/runner.rs` | `SdkRunner` 增 `efforts` 字段 + `with_efforts`（`new` 签名不动）；`open_session` 的档位 `match` 扩为 `(model_field, model_name, effort)` 三元绑定（语义即「模型 + effort」二元组），`effort::params(protocol, effort)` 算一次 shaping；`PumpPayload` 增 `effort_params: Option<serde_json::Value>`；泵启动日志加 `effort=…`（留空记 `-`） |
| `crates/infra/agent/src/sdk/loop.rs` | `LoopTurn` 增 `effort_params: Option<serde_json::Value>`；请求构造 `additional_params: turn.effort_params.clone()`（`provider_tool` 在其上按 key 合并，次序无关）；loop 零协议判定 |
| `src/commands/agents/mod.rs` | 新增 IPC 入参组 `AgentProviderTuning { context_length, max_tokens, web_search, efforts }`；`save_agent_provider` 末位四参收成 `tuning: AgentProviderTuning`（参数位上限所迫，见实现期回溯 1），保存期 `efforts` **只 `trim`**（两档各自 `trim()`，空串即留空；MUST NOT 值域校验 / MUST NOT 拒收自定义值），构造后 `.with_efforts(efforts)`；新增 `list_model_catalog() -> Result<ModelCatalog, String>` 薄包装（零参数、零 State、恒 `Ok`） |
| `src/commands/mod.rs` | `all_commands!` 单一登记面增 `list_model_catalog` 一行 |
| `packages/desktop/package.json` | 版本 `0.6.54` → `0.6.55` |
| `packages/desktop/src-tauri/src/types/generated/*` | `bindings:export` 再生成（非手改）：`AgentEffortTiers` / provider 记录 `efforts` / `AgentProviderTuning` 组（`saveAgentProvider` 末位形参）/ `ModelCatalog` 三类型 + `listModelCatalog` |
| `packages/desktop/src/components/ui/combobox.tsx`（新） | `@base-ui/react/combobox` 的原语层包装（输入框 + 建议弹层 + 清空），与 `ui/select.tsx` 同层同风格 |
| `packages/desktop/src/components/standard/combobox.tsx`（新） | `StandardCombobox` 薄壳（见 desktop-standard-components delta 的 Module Contract） |
| `packages/desktop/src/views/agents/hooks/use-model-catalog.ts`（新） | `list_model_catalog` 取数收口（挂载一次、显式触发、无轮询）；失败降级为「无建议」（`catalog = null`，表单照常可用） |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | `AgentProviderSaveInput` 增 `efforts: AgentEffortTiers`；invoke 末位按 `{ contextLength, maxTokens, webSearch, efforts }` 组传参（形参面 = 命令签名逐位对齐） |
| `packages/desktop/src/views/agents/components/provider-panel.tsx` | `ProviderFormState` 增 `effortsHigh` / `effortsLow`；三档模型与两档 effort 改 `StandardCombobox`（建议项随协议 / 选中模型联动）；预填（四条纪律）+「来自目录」弱标记；`context_length` / `max_tokens` 标签中文化并与提示改述；清单行呈 `effort high/low`（留空不呈） |
| `.dev-team/architecture/models/06-frontend.toml` | 增元素 `App.Frontend.Components.Standard.Combobox`（`kind = "component"` + `path` + `summary`，与 `Select` 同形）+ 两条关系（→ `Components.Ui`、`Views.Agents` → 本元素） |
| `.dev-team/architecture/models/Frontend/**`、`graph/INDEX.toml`、`*.schema.json`、`*.i.ts` | 经 `api:export` 生成（**非手改**）；`api:check` 零 diff |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| v4 载荷字节经版本机制解码 ⇒ `efforts` 空、其余字段逐字段相等；v1→v2→v3→v4→**v5** 四级链与「直接产出 v5」逐字段一致；V5→V4 丢 `efforts` 再升回 ⇒ 空；版本头锚点 `native_model_version() == 5`、id 仍 5；JSON 缺 `efforts` 键读出空值、非法载荷 `Err`；遮蔽 Debug 补位 | `crates/infra/store/src/model_test.rs` | 新增用例 + 版本锚点随动（预期红） |
| 结构体字面量构造点（`db_tool_test` / `describe_test` / `model_test` ×3 / `query_test`，共 6 处）补 `efforts: AgentEffortTiers::default()`（`::new` 的 44 处调用点零改；`envelope_test` / `store_test` 走 `::new` 夹具，无需改动） | `describe_test.rs` / `query_test.rs` / `db_tool_test.rs` / `model_test.rs` | 机械补字段 |
| `params` 两臂形状（`{reasoning_effort: v}` / `{output_config:{effort: v}}`）；空 / 空白 ⇒ `None`；自定义值原样透传（不做值域判定） | `crates/infra/agent/src/sdk/effort_test.rs`（新） | 新建，逐臂断言 |
| 目录五条不变量（协议内 `value` 唯一 / 非空 / 零凭据 / `[1m]` 条目窗长等于 1M / `supportsThinking=false` ⇒ 显式空 `efforts`） | `crates/infra/agent/src/model_catalog_test.rs`（新） | 新建，逐不变量一条 |
| `efforts` 空 ⇒ `open_session` 产出的 `effort_params` 为 `None`（两协议逐臂）；两档同档位映射（High→high / Low→low）；日志行含 `effort=…` / `effort=-` | `crates/infra/agent/src/sdk/runner_test.rs`、`lib_test.rs` | 新增用例 |
| 请求形状：两协议 × （留空 / `high` / 自定义值）；`efforts` 空时 `additional_params` 恒 `None`（**存量逐字等价**）；非空时与 `web_search` 的 `tools` 键共存且互不覆盖 | `crates/infra/agent/src/sdk/loop_test.rs` | 新增用例（脚本化流缝） |
| `ResolvedEngine.efforts` 原样带出（两协议皆带出，零门控） | `crates/infra/agent/src/compose_test.rs` | 新增用例 + fixture 补字段 |
| 保存期只 `trim`（`"  low  "` → `"low"`、`"   "` → 空）、自定义值可存可回读、`v4` 存量行升级读空 | `src/commands/agents/mod_test.rs` | 新增用例 + 既有 `save_agent_provider` 调用点末位改收 `AgentProviderTuning` 组 |
| `list_model_catalog` 注册与出线形状（零参数、三类型在场） | `src/bindings/mod_test.rs`、`src/commands/agents/mod_test.rs` | 快照随再生成 |
| `StandardCombobox` 契约（自由输入回调 / 选条目回调 / 清空 / `data-value` 透出 / disabled） | `src/components/standard/combobox.test.tsx`（新） | 新建（colocated，role / 文本 / `data-testid` 查询） |
| 目录取数失败降级为无建议；选中条目预填四条纪律（不自动预填 / 清空回未配置 / 元数据缺席不预填 / `[1m]` 一致）；`supportsThinking=false` 条目不排 effort 建议；中文标签与改述后的提示 | `src/views/agents/components/provider-panel.test.tsx`、`hooks/use-model-catalog.test.ts`（新） | 新增用例 + 两条既有断言随文案改述（`留空 = 按 context_length 分档推导` / `后缀优先于 context_length`） |
| `efforts` 入参与位置参数逐位对齐 | `src/views/agents/hooks/use-agent-providers.test.ts` | 断言随动 |
| provider 夹具补 `efforts`；`saveAgentProvider` 调用改 7 形参（末位 `tuning` 组） | `views/agents/**` 测试、`lib/ipc.test.ts` | 机械补字段 + 断言随动 |

### 不要修改

- **`crates/infra/agent/src/sdk/compact.rs`**：摘要请求恒不带 effort（`additional_params: None` 逐字不动——该文件出现 diff 即说明偏离决策）。
- **`crates/core/agent/**`**：零改动。effort 不进事件词汇、不进会话内核、不进 `AgentStartError`（「effort 的值合不合法」由 provider 侧决定，引擎侧只在取值为空时不发）。
- **`crates/infra/store` 的 `SessionConfigSnapshot` / `SessionRecord`**：不记 effort，无 v2→v3（决策 ⑨）；`model_test.rs` 里 `SessionRecord` 的版本锚点保持不动。
- **`sdk/config.rs` 的 `EngineConfig`**：连接配置字段面不增字段（`efforts` 走门面旁路，与 `context_length` / `max_tokens` / `web_search` 同族）。
- **`AgentModelTiers`**：三档结构零改动（effort 是 sibling 列，MUST NOT 变形成 `{name, effort}`）。
- **`sdk/{normalize,context,resume,reasoning,policy,tools,model_spec,web_search}.rs`**：effort 既不影响事件词汇，也不影响回放重建与工具面/窗长解析。
- **`src/commands/agents` 的既有校验**：模型名后缀校验（`parse_model_spec`）与 `max_tokens >= 4096` 下限逐字保留；effort 不新增封闭校验分支。
- **`.dev-team/architecture/**` 的生成物**（`INDEX.toml` / `*.schema.json` / `*.i.ts`）：只经 `api:export` 产出，手改即零 diff 门失败。
- **`packages/desktop/src/lib/ipc.ts`**：零改动——`ipc` 是对生成物 `commands` 的 Proxy（注释原文「新增命令零改本文件」），`listModelCatalog` 与新的末位 `tuning` 形参自动获得参数面 strict 校验与结果面 soft-report。
- **`packages/desktop/src/lib/ipc-contract.test.ts`**：零改动——逐命令三表等价断言按 `Object.keys(commands)` 泛化遍历，新命令由生成物自动覆盖。
- **`openspec/changes/provider-effort-knob/explore.md`**：只读。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | 存储演进一条链：`AgentProviderRecord` 版本头 `5`（id 仍 5），`AgentProviderRecordV4` 为 `pub(crate)` 中转；V4↔V5 双向 `From` 在场；v4 载荷字节经版本机制解码成功且 `efforts` 为空、其余字段逐字段相等；v1→v5 四级链与直接产出 v5 逐字段一致；V5→V4 丢 `efforts` 且其余保真 | `model_test` 五条升级 / 降级用例 |
| AC-2 | 构造器面不动：`AgentProviderRecord::new` 参数面逐字不变（44 处调用点零改），新增走 `with_efforts`；手写遮蔽 `Debug` 含 `efforts` 位且 api_key 位仍遮蔽 | `cargo clippy` 零告警 + `model_test` Debug 用例 + 源码复核 |
| AC-3 | 保存期**只 `trim`**：两档各自裁剪两侧空白，空串 = 留空；自定义值（目录外字符串）可存可回读；不存在的值域不被拒收；`context_length` / `max_tokens` 的既有校验逐字保留 | `src/commands/agents/mod_test.rs` |
| AC-4 | 请求侧零痕迹：`efforts` 两档皆空时，两协议请求的 `additional_params` **恒 `None`**（openai 臂无 `reasoning_effort`、anthropic 臂无 `output_config`），与变更前逐字相同 | `loop_test` 精确形状断言 |
| AC-5 | 请求侧形状：openai 档位生效值 → `additional_params = {"reasoning_effort": v}`；anthropic → `{"output_config": {"effort": v}}`；与 `web_search` 同开时 `tools` 键与本载荷共存且互不覆盖 | `loop_test` + `effort_test` |
| AC-6 | 档位分发：`High` 取 `models.high` + `efforts.high`、`Low` 取 `models.low` + `efforts.low`；shaping 只在 `open_session` 一处（`loop` / 门面 / 组合根零协议判定），引擎日志含该轮生效 `effort=…`（留空记 `-`） | `runner_test` / `lib_test` + 源码复核 |
| AC-7 | 目录不变量五条全绿（唯一 / 非空 / 零凭据 / `[1m]` 条目窗长 = 1M / `supportsThinking=false` ⇒ 显式空 `efforts`）；`list_model_catalog` 零参数零 IO 且已进 `all_commands!` | `model_catalog_test` + `src/commands/agents/mod_test.rs` |
| AC-8 | 目录**不进**运行期解析、**不进**保存期校验：解析期取值链（`spec.length` → `context_length` → 128K；`max_tokens` → rig 名单 → 窗长分档）逐字不变，目录内容改动不影响任何请求形状 | `compose_test` / `runner_test` 零回归 + 源码复核 |
| AC-9 | 前端控件：三档模型与两档 effort 均为 `StandardCombobox`（自由文本 + 建议清单）；`StandardCombobox` 有 colocated 测试且查询只用 role / 文本 / `data-testid`；产品源码零因测改动 | `combobox.test.tsx` + `provider-panel.test.tsx` |
| AC-10 | 预填四条纪律逐条可考：打开编辑态不自动预填（存量 `None` 行仍空、保存仍 `None`）；清空即回未配置；元数据缺席不预填；`[1m]` 条目窗长与后缀语义一致（1M）且既有「后缀优先」提示仍在；openai 协议不预填 `maxTokens` | `provider-panel.test.tsx` |
| AC-11 | 文案：`context_length` / `max_tokens` 的表单标签为「上下文窗长（token）」/「输出上限（token）」，提示同步改述（不再裸写英文键）；存储键 / bindings 字段名零改动、IPC 载荷键名零改名（`contextLength` / `maxTokens` / `webSearch` / `efforts` 四键同名，仅由平铺改为 `tuning` 组内——见实现期回溯 1） | `provider-panel.test.tsx` + `grep` 复核键名未变 |
| AC-12 | 目录取数失败降级：`list_model_catalog` 抛错时表单照常可用（无建议、无阻断），且不轮询；提交载荷 `efforts` 恒携（空值以空串表达） | `use-model-catalog.test.ts` / `use-agent-providers.test.ts` |
| AC-13 | 出线与契约：`bindings:export` 连跑两次零 diff；`api:check` 零 diff（含新元素 `Combobox` 生成物）；`cargo clippy` 零告警；`vp check` / `knip` / 全量测试全绿；`packages/desktop/package.json` 版本 `0.6.55` | 静态检查链 + 全量测试 |

### 验证命令

```powershell
pnpm -C packages/desktop run server:check
pnpm -C packages/desktop run server:test
pnpm -C packages/desktop run bindings:export
pnpm -C packages/desktop run api:check
pnpm -C packages/desktop run client:check
pnpm -C packages/desktop run client:test
pnpm -C packages/desktop run check      # 四段链：server:check → bindings:export → api:check → client:check
```

> **验收面口径**：验收止于**单元测试 + 静态检查链**。effort 的**实际深度效果**（模型是否真的少想 / 省 token / 提速）不属自动化验证面：本期无 DeepSeek 与官方 Anthropic 凭据，MUST NOT 把真机核验转写为任务清单的前置门或阻塞项；该项以「已知限制」留痕（desktop-agent-execution 边界条），与其余手工 E2E 统一并入**归档后人工验收**（结论记入验收记录，不阻塞实现与归档）。

---

## 遗留项裁定（explore 末节 5 项）

| # | 遗留项 | 裁定 |
|---|---|---|
| 1 | 目录内容（收哪些模型族 / 是否含 `[1m]` 变体 / effort 建议值是否含 `xhigh`、`max`） | **形状进 spec，名单不进**：首版必含 DeepSeek 系与 Anthropic 系常用条目 + `[1m]` 变体条目；`openai` 建议 `low/high/max`、`anthropic` 建议官方五值（含 `xhigh` / `max`）；GPT-6 系不进 openai 建议（rig 的 `refuse_tools_while_reasoning`，见风险 R7）。名单手写真相会过时，其**唯一**后果是「建议不准」（AC-8 锁死它不参与解析与校验） |
| 2 | 自定义值是否记忆 | **不记忆**：目录保持纯静态；引入可变目录需第二条存储与一套回收语义，本次收益不成立（边界留痕） |
| 3 | effort 输入位：high / low 各一 vs 只给 high | **两档各一**：A2 的价值就是「深相位 / 快相位各自独立」；只给 High 会让 Low 档永远停在 provider 缺省（DeepSeek 默认 `high`——恰恰是最贵的那档），与「降 effort 省 token」的动机相反 |
| 4 | 半 A / 半 B 是否同批 | **同批、内部按序**（见「半 A / 半 B 同批」表）；半 A 单独完成即处于可用状态 |
| 5 | `compact` 摘要请求是否带 effort | **不带**：摘要请求的 prompt 形态与主会话前缀不同（不共享缓存条目），带 effort 只增成本；`compact.rs` 进「不要修改」清单（出现 diff 即偏离） |

---

## 实现期回溯（1 项：IPC 参数位上限）

### 回溯 1：`save_agent_provider` 的四项可空调参收成末位一个 `AgentProviderTuning` 组

- **原裁定**：`save_agent_provider` 末位追加第 10 参 `efforts: AgentEffortTiers`（九参平铺 + 一）。
- **实现期事实**：specta 的 `SpectaFn` 只实现到 `fn(T1..T10)`，且 `State` 计入参数位——现有签名已是 10 参（`state + id + name + protocol + base_url + api_key + models + context_length + max_tokens + web_search`），加第 11 个即 `bindings:export` 编译失败（E0277，指向 `all_commands!` 展开）；本仓 `agent_start` 恰好也停在 10 参，说明这是当前依赖面的硬上限。**任何加参方案都必须先减参。**
- **新裁定**：四项**同族**的可空调参（`context_length` / `max_tokens` / `web_search` / `efforts`——spec 与 design 一贯表述为「门面旁路载荷，MUST NOT 进 `EngineConfig`」）收成末位一个 IPC 入参组 `AgentProviderTuning`，命令降到 8 参：
  `save_agent_provider(stores, id, name, protocol, base_url, api_key, models, tuning)`；`tuning` 线形 `{ contextLength, maxTokens, webSearch, efforts }`。
- **语义面零变化**：保存期校验（模型名后缀 / `max_tokens >= 4096` / 留空 api_key 回填）、`web_search` 无条件接受、`efforts` 只 `trim`、错误串结构，全部与本文其余条款逐字一致；记录形态（v5，`efforts` sibling 列）不受影响。
- **被拒备选**：①只把 `web_search + efforts` 合成一组（其余八参不动）——同样能降到 10 参，但 `context_length` / `max_tokens` 仍是顶层平铺，四项链路载荷被拆成「组内 2 + 组外 2」，与 spec 反复表述的「四项同族」相反；②把 `efforts` 塞进 `AgentModelTiers` 入参——与决策 ⑧「effort MUST 为 sibling 列」直接冲突；③去掉 `State`——taint 命令体的库路由，不可行；④放弃 `#[specta::specta]`——丢失参数面编译期锚与运行期 zod 校验，与 R12 的缓解措施相反。
- **随动面**（本文其余章节已同步）：`design.md` 的类型 / 公共函数 / 路由三表、`tasks.md` 阶段三与阶段七、`specs/desktop-agent-management` 的命令面 Module Contract；测试面的「9/10 形参」断言改为「7 形参（末位 tuning 组）」。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 bincode 升级链写错**：v4→v5 需现 v4 改名作中转 + 双向 `From`；bincode 非自描述，漏 bump 或漏改名即让存量 provider 读不出 | 升级后管理页 provider 清单为空 / 打不开 | 中 | 沿 `AgentProviderRecordV2` / `V3` 现成先例（改名字段逐个改动、`pub(crate)`、不进库模型组）；AC-1 以「v4 载荷字节经版本机制解码 + 四级链逐字段一致 + 降级丢列」三条为口径；`#[serde(default)]` 只兜 JSON 面，升级语义由 `From` 承担 |
| **R2 协议 shaping 写错污染存量**：任何「留空也发一个缺省 effort」的实现都会往所有 openai 协议请求里塞 `reasoning_effort`，非 DeepSeek 网关可能 400 | 存量用户升级即不可用 | 中 | 三条不变量之一「留空零痕迹」写成 AC-4（两协议逐臂断言 `additional_params` 恒 `None`）；`effort::params` 是唯一构造点（`loop` 零字面量） |
| **R3 `output_config` 与 rig typed 字段撞车**：rig 的 `AnthropicCompletionRequest.output_config` 是 `OutputConfig{format}`（只由 `output_schema` 填），我们经 flatten 再写一份同键 | 日后开结构化输出时出现重复键 / effort 静默丢 | 低（本期 `output_schema` 恒 `None`） | 留痕为边界条（desktop-agent-execution）：MUST NOT 同时启结构化输出与 anthropic effort；实现期单测锚定 `output_schema: None` 恒成立这条前提 |
| **R4 第三方 anthropic 兼容网关不认 `output_config.effort`**（较新的官方参数） | 用户体感「填了 effort 就跑不起来」 | 中 | 留空即不发（默认零风险）；失败以既有 `api_error` 显式收敛（不静默重试、不自动去掉参数重发）；表单提示「留空 = 用 provider 缺省」 |
| **R5 effort × Anthropic 提示缓存**：改 top-level effort 会使缓存前缀失效 | 缓存命中率下降、成本上升 | 低（结构已保证恒定） | effort 随 `ModelLevel` 走、会话内不换档（A2 的结构后果，写成不变量而非运行期检查）；`compact` 不带 effort（避免第二份 effort 出现在同会话）；将来若做 per-turn effort 必须重审本条 |
| **R6 DeepSeek（openai 格式）思考块回放未验证**：`reasoning_content` 是否需要回传由 provider 决定，本仓未验证 | 续轮可能 400 或失忆 | 中 | 跨轮仍走既有 `reasoning::strip_all`（重建史剔除），**不新增回放路径**；同轮内由 rig 序列化 `reasoning_content`，失败以 `api_error` 显式暴露（不静默降级）；留痕为边界条 |
| **R7 rig 对 GPT-6 系的 `reasoning_effort` 强制**：`refuse_tools_while_reasoning` 使该族在 Chat Completions 带 tools 时只接受 `"reasoning_effort": "none"`，而 `none` 不在本变更值域 | 用户为该族填任何 effort 即被 rig 拒发（错误串已指名修法） | 中 | 目录**不给该族 openai 协议建议**（建议面不把人带进坑）；错误以既有 `api_error` 显式暴露（rig 的拒绝文案本身指明「用 Responses wire / 发 none」）；留痕为边界条 |
| **R8 输出上限随 effort 静默漂移**：DeepSeek 思考模式缺省 64K、`effort=max` 128K；本仓 openai 协议恒不写 `max_tokens` | `OutputCap` 截断判据的观感变化（同样的输入，深度档变了上限也变了） | 低 | 留痕为边界条（MUST NOT 视为缺陷）；anthropic 侧已有显式 `max_tokens` 取值链不受影响；用户可显式填 `max_tokens`（仅 anthropic 消费） |
| **R9 目录过时**（静态手写真相，vendor 改档即不准） | 建议偏差 | 高（时间函数） | 设计与 spec 双处写明「目录只作建议：不参与保存期校验（AC-3）、不参与运行期解析（AC-8）」——漂移代价上限是「建议不准」，不会变成「同一窗长两套算法」；目录内容单点在一张静态表 |
| **R10 预填污染「留空 = 未配置」语义**（方案 A 的已知代价：预填值保存后落成显式值，与 `None` 在数据上不可辨） | 用户以为「没配」其实配了；vendor 改档后两者行为分叉 | 中 | 四条纪律（决策 ⑦）+ UI「来自目录」弱标记，把可辨性留在展示层而不扩存储；对既有「MUST NOT 落缺省字面值」作**显式限定**（目录条目标称值不属于「缺省启发式字面」）；AC-10 逐条锁定 |
| **R11 新标准组件的规格成本与 base-ui 行为差**（弹层异步、输入法、自由值 vs 选中值） | 组件返工 / 测试脆 | 中 | `StandardCombobox` 契约先落 spec（值语义 / 清空 / 值透出），colocated 测试以 role / 文本 / `data-testid` 挂钩、异步用 `findByRole('listbox')` 收敛；产品源码 MUST NOT 因测试可测性改动（沿 `StandardSelect` 先例） |
| **R12 参数面增长**：`save_agent_provider` 末位改收 `tuning` 组（新增 `AgentProviderTuning` 与两个必填键 `efforts`）、`AgentProviderSaveInput` 多一字段 | 前端位置参数错位 / 旧夹具编译期红 | 中 | 位置参数逐位对齐由 `mod_test` / `use-agent-providers.test.ts` 锚定；`bindings.ts` 必填化使漏改**编译期**报错（不静默）；`bindings.zod.ts` 是 `strictObject`，旧夹具不补即红（预期红） |
| **R13 升级后不可回退（前端侧）**：新后端返回带 `efforts` 的记录，旧前端 `strictObject` 校验会拒（与 `web_search` 同族） | 版本混用（开发期）报 zod 漂移 | 低（桌面端前后端同包发布） | 变更说明标注「升级后不可回退」；不实现降级分支 |
| **R14 新 archi 元素增加一条 `docMissing`**：`App.Frontend.Components.Standard.Combobox` 为 `declared` 且暂无契约 md | `apiFindings` 计数 +1（`status = warnings`，不纳管 `passed`） | 确定（可预期） | `docMissing` 是 warn 且永不纳管门禁（desktop-archi-api-check 已有判据）；若实现期一并写契约 md 则计数不增，二者皆合法 |
