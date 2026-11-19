# desktop-agent-management Specification（delta: provider-effort-knob）

## Purpose

本 delta 给 provider 记录加一列**思考强度**（`efforts{high, low}`，随 `ModelLevel` 分发，空串 = 留空 = 不发）并加一份**静态建议目录**（模型名 + effort 建议值域 + 能力元数据，`list_model_catalog` 出线）。三条边界先说清：①目录**不参与保存期校验、不参与运行期解析**（它只是前端参考数据，漂移代价上限是「建议不准」）；②`efforts` **不做封闭值域校验**（两 vendor 值域不是同一集合，保存期只 `trim`）；③存储只演进**一条链**（provider v4 → v5；会话装配快照不记 effort，见 desktop-agent-execution 边界条）。`AgentModelTiers` 三档结构、`context_length` / `max_tokens` 的既有校验与「留空 = 未配置」语义逐字保留（预填是**显式限定**，不是改写）。

## ADDED Requirements

### Requirement: 模型目录（ModelCatalog）静态建议表与 IPC 命令面

全局 agent 管理能力 SHALL 提供一份**静态模型目录**，作为「这家协议有哪些常用模型 / 它的标称窗长与最大输出是多少 / 它支持哪些 effort 值」的**建议面**。目录真源 SHALL 为 `crates/infra/agent` 内的**编译期静态表**（`model_catalog` 模块的纯 Rust 字面量），经 `commands/agents` 的 `list_model_catalog` 命令出线；MUST NOT 建表、MUST NOT 落库、MUST NOT 引入 IPC 之外的第二条取数路径。

目录形状 SHALL 为两级：

```text
ModelCatalog { protocols: [{
    protocol: openai | anthropic,
    efforts:  [ … ],                       ← 协议级建议值域（**建议**，非校验值域）
    models:  [{ value, contextLength?, maxTokens?, supportsThinking, efforts? }]
}]}
```

- `value` SHALL 为该模型的**完整原文**（可含 `[1m]` 后缀）；MUST NOT 另造「后缀位」或第二份裸名。
- `contextLength` / `maxTokens` 为**建议预填值**（可缺席），MUST NOT 被当作运行期取值链的一环（取值链仍为 `spec.length` → `context_length` → 128K 与 `max_tokens` → rig 名单 → 窗长分档，见 desktop-agent-execution）。
- `supportsThinking: bool` SHALL 只影响建议面（`false` ⇒ 该条目的 effort 建议为空）；MUST NOT 参与运行期判定。
- 模型级 `efforts` SHALL 为**覆盖**：`Some(list)`（含 `Some([])` = 显式「无建议」）优先于协议级，`None` = 继承协议级。

五条不变量 SHALL 由后端单测锚定：①同协议内 `value` 唯一、协议清单非空、每条协议 `models` 非空；②`efforts` 只作建议——保存期 MUST NOT 校验目录成员（自定义值必须能存）；③目录 MUST NOT 携带凭据 / 端点（纯静态参考数据，零遮蔽需求）；④条目 `value` 含 `[1m]` 后缀 ⇒ 其 `contextLength` SHALL 为 `1_000_000`（目录内不自相矛盾）；⑤`supportsThinking = false` 的条目 SHALL 给 `efforts = Some([])`（显式空，MUST NOT 靠「缺席 + 继承协议级」表达无建议）。

`list_model_catalog` SHALL 为零参数、零 IO、纯静态表读取的薄命令（`Result` 只为与同轨模板一致，恒 `Ok`），SHALL 在 `all_commands!` 单一登记面登记并随 `bindings:export` 出 `ModelCatalog` / 协议条目 / 模型条目三类型。目录内容（首版收哪些模型族、`[1m]` 变体条目的具体清单）SHALL 由实现期按 vendor 当前文档手填，**名单不是本 spec 的对象**（形状与不变量才是）。

#### Scenario: 目录出线与三类型

- **WHEN** 以零参数 invoke `list_model_catalog` 并查看生成的 bindings
- **THEN** 返回两级目录（协议 → effort 建议 + 模型条目）；`bindings` 含 `ModelCatalog` / 协议条目 / 模型条目三类型与 `listModelCatalog` 函数；命令在 `all_commands!` 清单内，`generate_handler` 与 `collect_commands` 同源

#### Scenario: 不变量逐条成立

- **WHEN** 遍历目录断言五条不变量
- **THEN** 各协议内 `value` 无重复、清单非空；目录对象不含 `api_key` / `base_url` 字段；每条含 `[1m]` 的条目 `contextLength == 1_000_000`；每条 `supportsThinking = false` 的条目 `efforts` 为显式空数组；任一条目值不在任何协议建议清单中也不影响保存（见「全局 agent 管理命令面」的 `efforts` 保存语义）

#### Scenario: 目录不改写解析与校验

- **WHEN** 改动目录内容（增删条目 / 改元数据）后发起一轮运行并保存一条 provider
- **THEN** 请求形状与运行期取值链逐字不变（目录零消费者）；保存期不因取值不在目录内而拒收

## MODIFIED Requirements

### Requirement: 全局 agent 管理命令面

命令层 SHALL 新增全局 agent 管理命令轨道（命名与文件分组由 design 定稿，落 `commands/` 全局轨，经 `WorkspaceStores::global()` 全局库实例执行，MUST NOT 误路由 workspace 库），承载两类实例的 CRUD 与默认标记：

- **provider 实例**：新增 / 更新 / 删除 / 清单。name SHALL 全局唯一（重名保存 SHALL 以 `Err` 报错）；**`protocol`**（模型协议，线值 `openai` | `anthropic`；缺省 `openai`——存量 v2 行升级即 `openai`，行为零变化；写入面接受两值，MUST NOT 因协议为 `anthropic` 拒收）、base_url、api_key、models 三档（high / medium / low）、**可空 `context_length`**（provider 上下文窗长，token 数；sdk 引擎上下文防线消费，缺席走 128K 缺省启发式——desktop-agent-execution「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）、**可空 `max_tokens`**（输出上限，token 数；`None` = 按上下文窗长分档推导（未配置窗长时 32000）；仅 anthropic 协议消费，openai 协议请求保持省略上限——desktop-agent-execution「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）、**`web_search: bool`**（是否启用 Anthropic 云端 `web_search` 服务端工具；缺省 `false`——存量行升级即关，行为零变化）、**`efforts: AgentEffortTiers{high, low}`**（同一模型在不同档位上的思考强度 / 深度：两档各为一自由字符串，**空串 = 留空 = 不发**；缺省两档皆空——存量行升级即留空，行为零变化）随记录保存；`context_length` 编辑语义 SHALL 为留空 = 未配置（存 `None`，MUST NOT 落 0 或把缺省启发式字面写库）；`max_tokens` 编辑语义 SHALL 为留空 = 未配置（存 `None`），非空值 MUST 满足 `>= 4096`（否则保存 SHALL 以 `Err` 拒收，MUST NOT 静默截断或夹到边界）；`web_search` 保存语义 SHALL 为**无条件接受**（MUST NOT 因 `protocol = openai` 拒收或清零——生效门在解析期，见下）；`efforts` 保存语义 SHALL 为**只 `trim`**：两档各自裁剪两侧空白后原样入库（`"  low  "` → `"low"`、纯空白 → 空串 = 留空），MUST NOT 做封闭值域校验、MUST NOT 因取值不在任何建议目录中而拒收（自定义值必须能存），MUST NOT 因 `protocol` 取值拒收或清零（两协议都有 effort 语义，wire 形状差异由引擎侧 shaping 单点吸收）；api_key 编辑语义 SHALL 为留空 = 保持原值不变（前端遮蔽占位配套，见「Agent 管理页」）；models 三档 SHALL 接受形如 `<模型名>[1m]` 的模型原文（后缀语义与解析规则归 desktop-agent-execution），保存期 SHALL 以引擎侧**同一**解析函数校验三档模型名——后缀非法（`[2m]`、`[8k]`、空方括号、方括号不在末尾等）SHALL 以 `Err` 拒收，MUST NOT 把带非法后缀的原文落库；
- **agent 实例**：新增 / 更新 / 删除 / 清单与默认标记。name 全局唯一；engine 线值域 `cli` | `sdk` 保留（`cli` 为存量只读解码值——`AgentEngineKind::Cli` 变体是 bincode 解码锚，线值不演进），写入面 SHALL 仅接受 `engine: sdk`：携 `engine: Cli` 的保存（新建与更新整行同拒）SHALL 以 `Err("CLI 租户已下线…")` 拒绝，存量 cli 实例可读、可删、不可改；provider 引用 SHALL 必选（`sdk` 唯一活档，CLI 引擎不消费 provider 的「cli 可空」语义随 CLI 租户退役）；
- **删除语义**：删被 agent 引用的 provider SHALL 阻止并报错（错误信息提示引用方，MUST NOT 级联删除）；删默认 agent SHALL 清空默认标记后删除（默认不顺延到其他 agent），MUST NOT 报错。

保存命令的**参数面** SHALL 为 `(id, name, protocol, base_url, api_key, models, tuning)`：四项可空调参（`context_length` / `max_tokens` / `web_search` / `efforts`）收成末位一个 `AgentProviderTuning` 组（线形 `{contextLength, maxTokens, webSearch, efforts}`，四键同名不改名）——IPC 参数位有硬上限（`State` 计入其一，平铺即越界），而四者同属「旁路载荷」一族，故收组而非拆族；记录形态不受此影响（`efforts` 仍是记录上的 sibling 列）。

错误约定沿用既有 `Result<T, String>` 模板（store `StoreError` MUST NOT 进入命令签名）；校验失败与 db 读写失败 MUST NOT 被静默吞掉。`context_length` / `max_tokens` / `web_search` / `efforts` 的消费接线 MUST NOT 经 `EngineConfig`（**连接配置字段面**承诺维持：`EngineConfig` 只承载连接必需项——协议、凭据、端点、双档模型；可空调参与引擎开关走门面旁路），抵达 sdk 引擎经 compose `ResolvedEngine` → `EngineFacade::with_context_window` / `with_max_tokens` / `with_web_search` / `with_efforts`。协议作为连接配置核心项 SHALL 逆入 `EngineConfig`——本变更是对原「`EngineConfig` 三字段结构不动」承诺的**显式修订**（协议决定 wire 语义，不在旁路列），修订后该承诺的形态为「连接配置字段面（含协议）MUST NOT 塌」而非「字段数恒三」。

`web_search` 的**生效门** SHALL 在解析期（内核组合根：`provider.web_search && protocol == Anthropic`，落点见 desktop-agent-execution「SDK 引擎 anthropic 云端 web_search 启用与开关链路」），MUST NOT 在命令层或保存期做协议判定——用户「先开开关后改协议」与「先改协议后开开关」两种操作序列 SHALL 收敛到同一记录形态（值原样入库，生效与否由解析期决定）。`efforts` 与 `web_search` 不同：它 **MUST NOT 有协议门控**（两协议都有 effort 语义），解析期只做原样带出，wire 形状差异归引擎侧 shaping 单点。

存储形态 SHALL 由 v4 演进到 v5：现 v4 形态改名为 `AgentProviderRecordV4` 作升级链中转（原样不动、不进库模型组、不出公共查询面），新增当前记录 `#[native_model(id = 5, version = 5, from = AgentProviderRecordV4)]` + `#[serde(default)] efforts: AgentEffortTiers`；V4 ↔ V5 双向 `From`（升级两档补空串，降级丢列）；`AgentProviderRecord::new(...)` 的**现有参数面 SHALL 不变**（新增走 `with_efforts(AgentEffortTiers)` 构造器）——既有调用点（44 处）MUST NOT 因本变更机械改动。`efforts` SHALL 为**sibling 列**：MUST NOT 并入 `AgentModelTiers`（该嵌套 struct 未被版本化，往里加字段等于改 bincode 布局却没有 `from` 链可依，存量行会静默错位或读到 EOF）。

#### Scenario: provider 重名报错

- **WHEN** 以已有 provider 的 name 再次保存 provider
- **THEN** 命令返回 `Err` 且指明重名，库中 provider 记录数量与内容不变

#### Scenario: sdk 必选 provider

- **WHEN** 保存 engine 为 `sdk` 且未选 provider 的 agent
- **THEN** 保存返回 `Err`；engine 为 `sdk` 且 provider 在选时保存成功

#### Scenario: cli 写入拒收

- **WHEN** 以 `engine: Cli` 新建 agent，或对存量 cli 实例发起更新（含改名、含改 engine 为 sdk 的整行更新）
- **THEN** 保存返回 `Err("CLI 租户已下线…")`，库中记录不变；存量 cli 实例仍可读取、可删除（「可读可删不可改」）

#### Scenario: 删被引用 provider 阻止

- **WHEN** 删除一个仍被某 agent 引用的 provider
- **THEN** 命令返回 `Err` 且错误信息含引用方提示，provider 与引用它的 agent 记录均原样保留（无级联）

#### Scenario: 删默认 agent 清标记

- **WHEN** 删除被标记为默认的 agent
- **THEN** 删除成功且默认标记消失（无其他 agent 被顺延标记）；此后缺省运行发起显式报错（见「默认 agent 与运行发起解析」）

#### Scenario: context_length 携带与留空语义

- **WHEN** 保存 provider 携带 `context_length = 200000`，随后读取清单
- **THEN** 记录携带该值原样返回；再次保存留空提交
- **AND** 记录的 `context_length` 为未配置（`None`），而非 0 或 128000 字面值

#### Scenario: protocol 保存与回读

- **WHEN** 保存 provider 携 `protocol = anthropic` 并读取清单，及读取一条 v2 版本存量行
- **THEN** 前者记录 `protocol` 原样返回 `anthropic`；后者升级读出 `openai`（存量行为零变化，无迁移脚本）；两值之外的值（非法线值）不入库

#### Scenario: max_tokens 边界与留空语义

- **WHEN** 保存 provider 携 `max_tokens = 4096`、`max_tokens = 8000`、`max_tokens = 4095`、留空四种形态
- **THEN** 前两者保存成功并按值返回；`4095` 返回 `Err`（指明下限 4096）且库中记录不变；留空保存为 `None`（非 0、非缺省字面值）

#### Scenario: 模型名后缀保存期校验

- **WHEN** 保存 provider 的三档模型名为 `claude-sonnet-4-6[1m]` / `glm-4.6`（合法）与 `claude-sonnet-4-6[2m]` / `x[8k]` / `x[]`（非法）两种情形
- **THEN** 合法形态保存成功并原样返回模型原文；非法形态返回 `Err` 且错误信息指明模型名与后缀非法（不拼接 api_key 明文），库中记录不变

#### Scenario: web_search 无条件保存与协议无关

- **WHEN** 分别保存 `protocol = anthropic` + `web_search = true` 与 `protocol = openai` + `web_search = true` 的两条 provider
- **THEN** 两者均保存成功且回读 `web_search = true`（保存期零协议判定、零报错、零静默清零）；openai 那条在运行解析期收敛为不生效（见 desktop-agent-execution 开关链路）

#### Scenario: efforts 只 trim 不校验值域

- **WHEN** 保存 provider 的 `efforts` 为 `{ high: "  low  ", low: "ultra-thinking" }`（两种都是目录外形态）
- **THEN** 保存成功，回读为 `{ high: "low", low: "ultra-thinking" }`（只裁剪空白、值域零校验、零拒收）；同一条 `protocol = openai` 与 `protocol = anthropic` 下均原样入库

#### Scenario: efforts 留空与清空语义

- **WHEN** 保存 provider 时 `efforts` 两档分别提交留空与纯空白串，随后再提交一次显式值又清空
- **THEN** 三次保存均成功，回读两档皆为空串（= 留空，非缺省字面值如 `"high"`、「未配置」以外的任何占位）；清空发生在该 provider 上时其余字段不变

#### Scenario: v4 存量行升级留空

- **WHEN** 读取一条 v4 版本存量 provider 行（无 `efforts` 列）
- **THEN** 升级读出两档皆空串（行为零变化、无迁移脚本）；新记录版本头为 5；`AgentProviderRecord::new` 的既有调用点（44 处）零改动

### Requirement: Agent 管理页

前端 SHALL 提供全局 Agent 管理页（路由 `/agents`，侧栏「系统工具」组新增入口，`data-testid="nav-agents"`，命名 design 可调）：

- **单页两栏**：Providers 清单 + Agents 清单，各自承载新建 / 编辑 / 删除交互；Agents 栏另承载默认标记切换（标记即切换，至多一个默认）；Providers 栏编辑表单 SHALL 增可空 `context_length` 数字字段（留空 = 未配置，跟随缺省启发式的语义提示在场）、**`protocol` 选择器**（复用既有 `StandardSelect`，选项 `openai` / `anthropic`，新建缺省 `openai`；编辑态允许改协议，与改 name / base_url 同权）、**可空 `max_tokens` 数字字段**（留空 = 按上下文窗长分档推导（未配置时 32000）；非正整数或 `< 4096` 拒绝提交并给出下限提示；字段提示 SHALL 声明「仅 anthropic 协议生效」）、**`web_search` 勾选项**（仅 `protocol === 'anthropic'` 渲染，`data-testid="provider-web-search"`，缺省不勾；提示 SHALL 声明「Anthropic 服务端网页搜索（按次计费）」；协议由 `anthropic` 切走时 SHALL 就地清零该勾选，切回时不自动恢复）、**三档模型与两档 effort 皆为「可选可填」控件**（`StandardCombobox`：输入框可自由键入，下拉给出该协议目录条目的建议；`data-testid` 分别为 `provider-model-high` / `provider-model-medium` / `provider-model-low` / `provider-effort-high` / `provider-effort-low`）；Providers 栏清单每行 SHALL 呈现协议徽标（`openai` / `anthropic` 可辨）与该记录的 effort（留空不呈）；
- **`base_url` 提示随协议切换**：`openai` 协议的 placeholder / 提示 SHALL 为含版本段的地址形态（`https://…/v1`）；`anthropic` 协议的提示 SHALL 为 `https://api.anthropic.com` 形态并说明「`/v1` 等后缀由引擎自动剥离后拼 `/v1/messages`」（不要求用户自行改写地址）；
- **模型名后缀提示**：表单 SHALL 提示三档模型名可带 `[1m]` 后缀（影响上下文窗长与 anthropic beta，且经窗长**间接**影响输出上限分档推导）；当某档带 `[1m]` 后缀且已填 `context_length` 时，提示 SHALL 声明该档窗长「已被模型名覆盖」（清除歧义，MUST NOT 让用户以为两者叠加）；
- **api_key 遮蔽展示**：列表与详情 SHALL 呈现遮蔽形态（`sk-***abc`）；编辑态输入框 SHALL 展示遮蔽占位，留空提交 = 保持原值不变；
- **错误呈现**：删除被引用 provider、保存重名、无默认发起等错误 SHALL 以既有 error 态呈现，MUST NOT 静默；
- **取数模型**：清单经取数 hooks 收口、显式动作触发，MUST NOT 轮询（管理数据无文件 watch 例外）。模型目录另经一条取数 hook 收口（挂载 / 打开表单时一次、无轮询），失败 SHALL 降级为「无建议」（表单照常可用、零阻断、零 error 态覆盖清单）；
- **字段展示名中文化（仅两项）**：`context_length` 的表单标签 SHALL 为「上下文窗长（token）」、`max_tokens` 的 SHALL 为「输出上限（token）」；提示文案 SHALL 同步改述（不再裸写英文键名），但存储键、IPC 参数名与 bindings 字段名 MUST NOT 变更。

页面 SHALL 不依赖当前 workspace root（全局语义，壳态下任意时刻可用）；页面为纯内容页，MUST NOT 触发任何 workspace 命令。

`web_search` 的呈现纪律：checkbox 为**布尔控件**，MUST NOT 混入文本字段的受控输入路径（表单状态的 `ProviderTextField` 键面 SHALL 排除该键）；提交载荷 SHALL 恒携 `webSearch`（含 openai 协议下的 `false`），MUST NOT 因协议为 openai 而在载荷中省略该字段（省略会与「保持原值」混淆）。四项可空调参经命令面末位的 `tuning` 组上送（键名仍是 `contextLength` / `maxTokens` / `webSearch` / `efforts`）。

**目录预填的四条纪律**（对「`context_length` 留空 = 未配置」的**显式限定**——目录条目的标称值不属于「缺省启发式字面」）：

1. **只由「用户主动选择目录条目」触发**：打开编辑态 MUST NOT 自动预填——存量 `None` 行打开仍是空框，一保存仍是 `None`；
2. **清空即回未配置**：用户把预填值删空 → 保存 `None`（「留空 = 未配置」不破）；
3. **预填值随条目元数据走**：取目录条目的 `contextLength` / `maxTokens` 原值，MUST NOT 用 128K 启发式或分档推导值冒充；元数据缺席则该输入框不预填（保持空）；`openai` 协议下 `maxTokens` MUST NOT 预填（该协议恒不写上限）；
4. **`[1m]` 一致性优先**：条目 `value` 含后缀时预填的 `contextLength` 与后缀语义一致（`1_000_000`），既有「后缀优先于 `context_length`」提示照旧在场。

表单 SHALL 给「由目录带出的值」挂一枚可见弱标记（如「来自目录」小字），使「建议落值」在展示层可辨——MUST NOT 为此扩存储字段。

effort 建议的取数规则：控件建议项 SHALL 为「该协议目录的 `efforts`」，选中模型条目若带模型级 `efforts` 覆盖（含显式空数组）SHALL 以覆盖为准；模型值为自定义（不在目录内）时 SHALL 回落到协议级建议。建议项 MUST NOT 成为取值约束（自由输入即可保存）。

#### Scenario: 路由与入口 active

- **WHEN** URL 为 `/agents`
- **THEN** 渲染管理页两栏视图，侧栏 `nav-agents` 呈现 active 态，`nav-agent` / `nav-db` 非 active

#### Scenario: 遮蔽与留空保持

- **WHEN** 编辑既有 provider 且 api_key 输入框留空提交
- **THEN** 保存成功且记录的 api_key 保持原值；列表中该 provider 的 key 呈现 `sk-***abc` 遮蔽形态而非明文

#### Scenario: context_length 编辑与清空

- **WHEN** 在 provider 编辑表单填 `context_length = 200000` 保存后重开编辑态
- **THEN** 字段显示 200000；清空该字段再保存
- **AND** 记录回到未配置态，表单呈现「跟随缺省」语义而非 0

#### Scenario: 删除阻止错误呈现

- **WHEN** 在管理页删除仍被 agent 引用的 provider
- **THEN** 页面以 error 态呈现引用阻止信息，Providers 清单不变

#### Scenario: 取数无轮询

- **WHEN** 停留在管理页不进行任何操作
- **THEN** 无定时取数发生（provider 清单与模型目录各自只在挂载 / 显式动作时取一次）；新建 / 编辑 / 删除等显式动作后清单刷新

#### Scenario: 协议选择与提示切换

- **WHEN** 新建 provider（缺省协议）并在表单把协议由 `openai` 切到 `anthropic`，保存后查看清单
- **THEN** 新建态 `base_url` placeholder 为 openai 形态（含 `/v1` 提示），切到 anthropic 后 placeholder 与提示改为 `https://api.anthropic.com` 形态且说明后缀自动剥离；保存记录的 `protocol` 为 `anthropic`，清单该行呈现 anthropic 协议徽标；两协议下 effort 控件均在（effort 无协议门控，建议项随协议切换）

#### Scenario: max_tokens 字段边界

- **WHEN** 在 provider 表单填 `max_tokens = 4096` 与 `max_tokens = 4095` 两种形态，及留空提交
- **THEN** `4096` 提交成功并回读为 4096；`4095` 拒绝提交（保存按钮不可用 / 提交被拦）并给出下限 4096 提示；留空提交后记录为未配置（表单呈现「按上下文窗长分档推导」语义而非 0）

#### Scenario: 模型名后缀提示

- **WHEN** 某档模型名填 `claude-sonnet-4-6[1m]` 且表单同时填有 `context_length`
- **THEN** 表单呈现「该档窗长已被模型名覆盖」提示；非法后缀（如 `[2m]`）在提交后以 error 态呈现保存失败原因

#### Scenario: web_search 仅 anthropic 可见

- **WHEN** 新建 provider（缺省 `openai`）后查看表单，再切协议到 `anthropic`
- **THEN** openai 协议下 `data-testid="provider-web-search"` 不在文档中、提交载荷 `webSearch = false`；切到 anthropic 后 checkbox 出现（缺省不勾）并带「按次计费」提示

#### Scenario: 切协议清零

- **WHEN** 在 anthropic 协议下勾选 `web_search`，随后把协议切回 `openai` 再保存
- **THEN** 勾选就地清零，载荷 `webSearch = false`；再切回 `anthropic` 时勾选不自动恢复（用户需重新勾选）

#### Scenario: 可选可填与建议联动

- **WHEN** 在表单展开 `provider-model-high` 与 `provider-effort-high` 的建议清单，并在模型框键入目录外的自定义模型名
- **THEN** 模型建议项为该协议目录条目；选中某条目后 `provider-effort-high` 的建议项切换为该条目的模型级 `efforts`（显式空数组则为空清单）；模型值为自定义时 effort 建议回落到协议级清单；两框均可保存目录外取值（提交不被拦）

#### Scenario: 预填四条纪律

- **WHEN** （a）打开一条 `contextLength` / `maxTokens` 皆未配置的存量 provider 编辑态但不动任何控件；（b）从目录选中一条带 `contextLength = 200000` 的条目后保存；（c）把该预填值删空再保存；（d）在 `openai` 协议下选中带 `maxTokens` 元数据的条目
- **THEN** （a）两框为空且保存后仍为未配置（零自动预填）；（b）两框带出该条目元数据（带「来自目录」弱标记）并原样落库为显式值；（c）保存后回到未配置（`None`，非 0）；（d）`上下文窗长（token）` 被预填、`输出上限（token）` 保持空（openai 协议不预填该值）

#### Scenario: 目录取数失败降级

- **WHEN** `list_model_catalog` 调用失败（`Err`）后打开 provider 表单
- **THEN** 表单照常渲染与提交（五个 combobox 退化为纯输入框、无建议项）；不呈现阻断性 error、不覆盖清单的 error 态、提交载荷含空 `efforts` 两档

#### Scenario: 字段展示名中文化且键名不变

- **WHEN** 查看表单标签与提交载荷
- **THEN** 两级标签呈「上下文窗长（token）」/「输出上限（token）」，提示文案不再裸写 `context_length` / `max_tokens`；提交载荷仍是 `contextLength` / `maxTokens` 字段，记录键名与 bindings 字段名零变更

### Requirement: 本期边界与后续路线

以下边界 SHALL 作为本期显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **model 三档的深度轴已由 provider `efforts` 偿还（本变更）**：三档模型仍存储完整，`efforts{high, low}` 承载「同一模型想多深」并随 `ModelLevel` 分发；`AgentRunParams` 增 **per-run** effort 参数（覆盖 provider 值）与前端档位选择器仍为后续迭代，本变更 MUST NOT 预建该参数位；
2. **workspace 关联 agent 后续改造**：届时运行解析顺序演变为 workspace 关联 → 全局默认；本期数据模型以稳定 id 主键预留支撑（关联引用 id，实体改名不破引用），MUST NOT 预建关联字段或解析分支；
3. **api_key 静态加密落盘（Windows DPAPI）不做**：单用户同机威胁模型下收益边际，未来有合规要求再启；
4. **provider 连通性测试按钮不做**（未要求）。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 四条边界均可考，后续变更无需重新论证是否知情；边界条 1 明确「provider 级 effort 已落地、per-run 覆盖仍为后续」，MUST NOT 出现「effort 尚未实现」的失真表述

## Module Contract

### Function: `packages/desktop/src-tauri/crates/infra/store`（provider 记录与升级链）

| 项 | 签名 / 形态 | 描述 |
|------|------|----------|
| `AgentEffortTiers` | `{ high: String, low: String }`（`serde` camelCase、`Default` 派生、`specta::Type`） | 两档思考强度；**空串 = 留空 = 不发**（与 `AgentModelTiers` 用空串表未配置同型）；纯嵌套 struct 不落独立模型 |
| `AgentProviderRecord` | `{ …, web_search: bool, efforts: AgentEffortTiers }`，`#[native_model(id = 5, version = 5, from = AgentProviderRecordV4)]` | 增 `efforts`（`#[serde(default)]` sibling 列）；手写遮蔽 `Debug` 字段面同步扩位 |
| `AgentProviderRecordV4` | 现 v4 形态（无 `efforts`），`#[native_model(id = 5, version = 4, from = AgentProviderRecordV3)]`，`pub(crate)` | 升级链中转（V4 → V5 逐级 `From`；升级两档补空串，降级丢列） |
| `AgentProviderRecord::new` | 签名不变（`id` 置 0 的构造器） | 现有参数面不动（44 处调用点零改）；`efforts` 缺省两档空串 |
| `AgentProviderRecord::with_efforts` | `fn(self, efforts: AgentEffortTiers) -> Self` | 构造器链式缝（`new` 签名不动的唯一实现路径，与 `with_web_search` 同型） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/model_catalog.rs`（新）

| 项 | 签名 / 形态 | 描述 |
|------|------|----------|
| `ModelCatalog` | `{ protocols: Vec<CatalogProtocol> }` | 两级目录根（`specta::Type`，出 IPC） |
| `CatalogProtocol` | `{ protocol: store::AgentProviderProtocol, efforts: Vec<String>, models: Vec<CatalogModel> }` | 协议级建议值域 + 模型条目；`protocol` 复用 store 枚举（线值单一真源） |
| `CatalogModel` | `{ value: String, context_length: Option<u64>, max_tokens: Option<u64>, supports_thinking: bool, efforts: Option<Vec<String>> }` | 条目 `value` 为完整模型原文（可含 `[1m]`）；`efforts` 为 `Some` 时覆盖协议级（`Some([])` = 显式无建议） |
| `catalog` | `fn() -> ModelCatalog` | 纯静态表读取，零 IO、零参数、零凭据 |

### API: `packages/desktop/src-tauri/src/commands/agents/mod.rs`（命令面）

| 命令 | 签名 | 描述 |
|------|------|----------|
| `save_agent_provider` | `(id, name, protocol, base_url, api_key, models, tuning: AgentProviderTuning) -> Result<AgentProviderRecord, String>` | 末位为四项可空调参组 `AgentProviderTuning { context_length, max_tokens, web_search, efforts }`（参数位上限所迫，见本文「保存命令的参数面」段）；`efforts` 保存期**只 `trim`**（两档各自裁空白；MUST NOT 值域校验、MUST NOT 协议门控）；构造后 `.with_efforts(efforts)`；既有校验（模型名后缀 / `max_tokens >= 4096` / 留空 api_key 回填）原样保留，错误串不含 api_key |
| `AgentProviderTuning` | `{ context_length: u64 \| null, max_tokens: u64 \| null, web_search: bool, efforts: AgentEffortTiers }`（`serde` camelCase，命令层 DTO） | **IPC 入参形态**（非记录形态）：四项旁路载荷收一组；`web_search` / `efforts` 恒携，两个可空数值位留空即 `null` |
| `list_model_catalog` | `() -> Result<ModelCatalog, String>` | 零参数零 IO 薄包装（恒 `Ok`）；`all_commands!` 登记；出 bindings 三类型 |
| `list_agent_providers` / `delete_agent_provider` | 签名不变 | 清单自动带出 `efforts`；删除引用阻止语义不变 |

### Component: `packages/desktop/src/views/agents/`（管理页）

| 项 | 变化 | 描述 |
|------|------|----------|
| `hooks/use-agent-providers.ts`（`AgentProviderSaveInput`） | 增 `efforts: { high: string; low: string }` | `commands.saveAgentProvider(...)` 实参按命令签名：末位一个 `tuning` 组 `{ contextLength, maxTokens, webSearch, efforts }`；错误双轨语义不变 |
| `hooks/use-model-catalog.ts`（新） | `useModelCatalog(): { catalog: ModelCatalog \| null; loading: boolean }` | 挂载一次取数、无轮询；失败置 `catalog = null`（降级为无建议，不抛 error 态） |
| `components/provider-panel.tsx`（`ProviderFormState`） | 增 `effortsHigh` / `effortsLow`；三档模型与两档 effort 走 `StandardCombobox`；`ProviderTextField` 仍排除 `protocol` / `webSearch` / 新控件键 | 建议项随协议与选中模型联动；预填四条纪律 +「来自目录」弱标记；`context_length` / `max_tokens` 标签中文化 + 提示改述；清单行呈 `effort high/low`（留空不呈）；提交恒携 `efforts`（空值以空串表达） |
