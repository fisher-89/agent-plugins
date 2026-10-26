# desktop-agent-management Specification

## Purpose

定义全局 agent 管理能力：provider / agent 实例的 CRUD 与默认标记命令面、运行发起的引擎与连接配置解析单点、Agent 管理页（`/agents` 两栏视图），以及 api_key 机密边界治理与本期显式边界留痕。

## Requirements

### Requirement: 全局 agent 管理命令面

命令层 SHALL 新增全局 agent 管理命令轨道（命名与文件分组由 design 定稿，落 `commands/` 全局轨，经 `WorkspaceStores::global()` 全局库实例执行，MUST NOT 误路由 workspace 库），承载两类实例的 CRUD 与默认标记：

- **provider 实例**：新增 / 更新 / 删除 / 清单。name SHALL 全局唯一（重名保存 SHALL 以 `Err` 报错）；**`protocol`**（模型协议，线值 `openai` | `anthropic`；缺省 `openai`——存量 v2 行升级即 `openai`，行为零变化；写入面接受两值，MUST NOT 因协议为 `anthropic` 拒收）、base_url、api_key、models 三档（high / medium / low）、**可空 `context_length`**（provider 上下文窗长，token 数；sdk 引擎上下文防线消费，缺席走 128K 缺省启发式——desktop-agent-execution「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）、**可空 `max_tokens`**（输出上限，token 数；`None` = 按协议缺省；仅 anthropic 协议消费，openai 协议请求保持省略上限——desktop-agent-execution「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）、**`web_search: bool`**（是否启用 Anthropic 云端 `web_search` 服务端工具；缺省 `false`——存量行升级即关，行为零变化）随记录保存；`context_length` 编辑语义 SHALL 为留空 = 未配置（存 `None`，MUST NOT 落 0 或把缺省启发式字面写库）；`max_tokens` 编辑语义 SHALL 为留空 = 未配置（存 `None`），非空值 MUST 满足 `>= 4096`（否则保存 SHALL 以 `Err` 拒收，MUST NOT 静默截断或夹到边界）；`web_search` 保存语义 SHALL 为**无条件接受**（MUST NOT 因 `protocol = openai` 拒收或清零——生效门在解析期，见下）；api_key 编辑语义 SHALL 为留空 = 保持原值不变（前端遮蔽占位配套，见「Agent 管理页」）；models 三档 SHALL 接受形如 `<模型名>[1m]` 的模型原文（后缀语义与解析规则归 desktop-agent-execution），保存期 SHALL 以引擎侧**同一**解析函数校验三档模型名——后缀非法（`[2m]`、`[8k]`、空方括号、方括号不在末尾等）SHALL 以 `Err` 拒收，MUST NOT 把带非法后缀的原文落库；
- **agent 实例**：新增 / 更新 / 删除 / 清单与默认标记。name 全局唯一；engine 线值域 `cli` | `sdk` 保留（`cli` 为存量只读解码值——`AgentEngineKind::Cli` 变体是 bincode 解码锚，线值不演进），写入面 SHALL 仅接受 `engine: sdk`：携 `engine: Cli` 的保存（新建与更新整行同拒）SHALL 以 `Err("CLI 租户已下线…")` 拒绝，存量 cli 实例可读、可删、不可改；provider 引用 SHALL 必选（`sdk` 唯一活档，CLI 引擎不消费 provider 的「cli 可空」语义随 CLI 租户退役）；
- **删除语义**：删被 agent 引用的 provider SHALL 阻止并报错（错误信息提示引用方，MUST NOT 级联删除）；删默认 agent SHALL 清空默认标记后删除（默认不顺延到其他 agent），MUST NOT 报错。

错误约定沿用既有 `Result<T, String>` 模板（store `StoreError` MUST NOT 进入命令签名）；校验失败与 db 读写失败 MUST NOT 被静默吞掉。`context_length` / `max_tokens` / `web_search` 的消费接线 MUST NOT 经 `EngineConfig`（**连接配置字段面**承诺维持：`EngineConfig` 只承载连接必需项——协议、凭据、端点、双档模型；可空调参与引擎开关走门面旁路），抵达 sdk 引擎经 compose `ResolvedEngine` → `EngineFacade::with_context_window` / `with_max_tokens` / `with_web_search`。协议作为连接配置核心项 SHALL 逆入 `EngineConfig`——本变更是对原「`EngineConfig` 三字段结构不动」承诺的**显式修订**（协议决定 wire 语义，不在旁路列），修订后该承诺的形态为「连接配置字段面（含协议）MUST NOT 塌」而非「字段数恒三」。

`web_search` 的**生效门** SHALL 在解析期（内核组合根：`provider.web_search && protocol == Anthropic`，落点见 desktop-agent-execution「SDK 引擎 anthropic 云端 web_search 启用与开关链路」），MUST NOT 在命令层或保存期做协议判定——用户「先开开关后改协议」与「先改协议后开开关」两种操作序列 SHALL 收敛到同一记录形态（值原样入库，生效与否由解析期决定）。

存储形态 SHALL 由 v3 演进到 v4：现 v3 形态改名为 `AgentProviderRecordV3` 作升级链中转（原样不动、不进库模型组、不出公共查询面），新增当前记录 `#[native_model(id = 5, version = 4, from = AgentProviderRecordV3)]` + `#[serde(default)] web_search: bool`；V3 ↔ V4 双向 `From`（升级补 `false`，降级丢列）；`AgentProviderRecord::new(...)` 的**现有参数面 SHALL 不变**（新增走 `with_web_search(bool)` 构造器）——既有调用点（38 处）MUST NOT 因本变更机械改动。

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

#### Scenario: v3 存量行升级默认关

- **WHEN** 读取一条 v3 版本存量 provider 行（无 `web_search` 列）
- **THEN** 升级读出 `web_search = false`（行为零变化、无迁移脚本）；新记录版本头为 4；`AgentProviderRecord::new` 的既有调用点（38 处）零改动

### Requirement: 默认 agent 与运行发起解析

agent 实例 SHALL 支持「默认」标记，全局至多一个；标记新默认 SHALL 原子清除旧默认（切换语义，无需先手动取消）。默认标记 SHALL 仅可落在 `sdk` 实例上：对存量 `cli` 实例设默认 SHALL 以 `Err("CLI 租户已下线…")` 拒绝（防造出必失败的缺省发起路径）。

运行发起的引擎与连接配置 SHALL 由**内核组合根**单点解析（desktop-agent-execution 会话内核落地，解析点自命令层下沉——解析语义与消费面承诺不变）：

- **缺省路径**（explore 页等一切不显式指定 agent 的发起）：解析默认 agent；无默认 agent SHALL 以 `Err` 显式失败并提示前往管理页配置，MUST NOT 静默回退硬编码引擎或空配置；默认实例 `engine` 为 `cli`（存量态）时同 SHALL 显式 `Err("CLI 租户已下线…")` 并提示管理页改默认（缺省发起不迁数据、不清 `is_default`——改标 sdk 默认经切换语义原子清旧）；
- **显式路径**（调试页 agent 选择器）：按所选 agent 解析；所选实例为存量 `cli` 实例时 SHALL 显式 `Err("CLI 租户已下线…")`，MUST NOT 静默降级；
- **解析产物**：`agent.engine` → `EngineKind`（值域 `cli` | `sdk` 保留为多租户协议预留，`Cli` 为退役态）；engine 为 `sdk` 时由引用 provider 组装 `EngineConfig{protocol, api_key, base_url, model_high, model_low}`——`protocol` SHALL 由 `AgentProviderProtocol` 映射为门面 `Protocol`（沿 `AgentEngineKind` → `EngineKind` 双枚举映射先例，两臂 match），model 固定取 provider 三档中的 **high** 档（档位可调点由 design 定稿）；provider 的可空 `context_length` / `max_tokens` 与三档模型名原文 SHALL 同型经解析产物旁路携带（窗长、输出上限与 `[1m]` 后缀解析全在引擎侧单点，MUST NOT 在命令层或组合根重复解析）。

解析 SHALL 收在内核组合根单点：`EngineConfig` 的**连接配置字段面**（协议 + 凭据 + 端点 + 双档模型）为唯一形态且 MUST NOT 塌（既有「换源零改动」承诺平移为「换解析落点零改动」；可空调参 `context_length` / `max_tokens` MUST NOT 进 `EngineConfig`，协议 MUST 进）；`EngineFacade::runner_for` 唯一 match 点结构 MUST NOT 塌（`EngineKind::Cli` 臂显式 `Err("CLI 租户已下线")`，签名形态由 design 定稿）；命令层 SHALL 无引擎/agent 解析残留。`crates/core/agent` MUST NOT 出现 agent / engine / 协议概念：协议参数面无 agent 字段，`AgentStartError` 不加变体（连接配置不齐走既有 `ConfigMissing` 显式启动失败）。

#### Scenario: 缺省解析默认 agent

- **WHEN** explore 页等不显式指定 agent 发起运行，且存在已标记默认的 sdk agent
- **THEN** 内核组合根解析该 agent 并经门面构造 SDK runner（`EngineConfig` 来自其引用 provider，model 取 high 档），发起路径与显式选择该 agent 完全一致

#### Scenario: 无默认显式报错

- **WHEN** 无默认 agent 时以缺省路径发起运行
- **THEN** 命令返回 `Err` 且提示前往管理页配置，不产生会话/轮记录、不推流、不静默回退

#### Scenario: cli 默认 agent 解析

- **WHEN** 默认 agent 为存量 `cli` 实例时以缺省路径发起运行
- **THEN** 显式 `Err("CLI 租户已下线…")`（文案提示管理页改默认或新建 sdk 实例），不产生会话/轮记录、不推流；用户改标 sdk 默认后缺省发起恢复（切换语义原子清旧默认），全程零数据迁移脚本

#### Scenario: cli 实例设默认拒收

- **WHEN** 对存量 cli 实例 invoke 设默认命令
- **THEN** 命令返回 `Err("CLI 租户已下线…")`，默认标记分布不变；对 sdk 实例设默认照常（切换语义原子清旧默认）

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现、命令层与 `crates/core/agent` 源码
- **THEN** 解析语义与设计一致（默认/显式/sdk 组装/high 档/无默认 `Err`/cli 显式 `Err`），`runner_for` 唯一 match 点结构与连接配置字段面零塌、命令层无解析残留；core 无 agent / engine / 协议字样且 `AgentStartError` 变体集不变

#### Scenario: 协议透传到引擎

- **WHEN** 默认或显式 agent 引用的 provider 记录 `protocol = anthropic`（并填有 `max_tokens`）
- **THEN** 解析产物的 `EngineConfig.protocol` 为门面 `Protocol::Anthropic`、`max_tokens` 经旁路随产物携带；provider `protocol = openai` 时对应为 `Protocol::Openai`；解析层不对协议做二次判断（选臂只在引擎组装点）

### Requirement: Agent 管理页

前端 SHALL 提供全局 Agent 管理页（路由 `/agents`，侧栏「系统工具」组新增入口，`data-testid="nav-agents"`，命名 design 可调）：

- **单页两栏**：Providers 清单 + Agents 清单，各自承载新建 / 编辑 / 删除交互；Agents 栏另承载默认标记切换（标记即切换，至多一个默认）；Providers 栏编辑表单 SHALL 增可空 `context_length` 数字字段（留空 = 未配置，跟随缺省启发式的语义提示在场）、**`protocol` 选择器**（复用既有 `StandardSelect`，选项 `openai` / `anthropic`，新建缺省 `openai`；编辑态允许改协议，与改 name / base_url 同权）、**可空 `max_tokens` 数字字段**（留空 = 按协议缺省；非正整数或 `< 4096` 拒绝提交并给出下限提示；字段提示 SHALL 声明「仅 anthropic 协议生效」）、**`web_search` 勾选项**（仅 `protocol === 'anthropic'` 渲染，`data-testid="provider-web-search"`，缺省不勾；提示 SHALL 声明「Anthropic 服务端网页搜索（按次计费）」；协议由 `anthropic` 切走时 SHALL 就地清零该勾选，切回时不自动恢复）；Providers 栏清单每行 SHALL 呈现协议徽标（`openai` / `anthropic` 可辨）；
- **`base_url` 提示随协议切换**：`openai` 协议的 placeholder / 提示 SHALL 为含版本段的地址形态（`https://…/v1`）；`anthropic` 协议的提示 SHALL 为 `https://api.anthropic.com` 形态并说明「`/v1` 等后缀由引擎自动剥离后拼 `/v1/messages`」（不要求用户自行改写地址）；
- **模型名后缀提示**：表单 SHALL 提示三档模型名可带 `[1m]` 后缀（只影响上下文窗长与 anthropic beta，不改输出上限）；当某档带 `[1m]` 后缀且已填 `context_length` 时，提示 SHALL 声明该档窗长「已被模型名覆盖」（清除歧义，MUST NOT 让用户以为两者叠加）；
- **api_key 遮蔽展示**：列表与详情 SHALL 呈现遮蔽形态（`sk-***abc`）；编辑态输入框 SHALL 展示遮蔽占位，留空提交 = 保持原值不变；
- **错误呈现**：删除被引用 provider、保存重名、无默认发起等错误 SHALL 以既有 error 态呈现，MUST NOT 静默；
- **取数模型**：清单经取数 hooks 收口、显式动作触发，MUST NOT 轮询（管理数据无文件 watch 例外）。

页面 SHALL 不依赖当前 workspace root（全局语义，壳态下任意时刻可用）；页面为纯内容页，MUST NOT 触发任何 workspace 命令。

`web_search` 的呈现纪律：checkbox 为**布尔控件**，MUST NOT 混入文本字段的受控输入路径（表单状态的 `ProviderTextField` 键面 SHALL 排除该键）；提交载荷 SHALL 恒携 `webSearch`（含 openai 协议下的 `false`），MUST NOT 因协议为 openai 而在载荷中省略该字段（省略会与「保持原值」混淆）。

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
- **THEN** 无定时取数发生；新建 / 编辑 / 删除等显式动作后清单刷新

#### Scenario: 协议选择与提示切换

- **WHEN** 新建 provider（缺省协议）并在表单把协议由 `openai` 切到 `anthropic`，保存后查看清单
- **THEN** 新建态 `base_url` placeholder 为 openai 形态（含 `/v1` 提示），切到 anthropic 后 placeholder 与提示改为 `https://api.anthropic.com` 形态且说明后缀自动剥离；保存记录的 `protocol` 为 `anthropic`，清单该行呈现 anthropic 协议徽标

#### Scenario: max_tokens 字段边界

- **WHEN** 在 provider 表单填 `max_tokens = 4096` 与 `max_tokens = 4095` 两种形态，及留空提交
- **THEN** `4096` 提交成功并回读为 4096；`4095` 拒绝提交（保存按钮不可用 / 提交被拦）并给出下限 4096 提示；留空提交后记录为未配置（表单呈现「按协议缺省」语义而非 0）

#### Scenario: 模型名后缀提示

- **WHEN** 某档模型名填 `claude-sonnet-4-6[1m]` 且表单同时填有 `context_length`
- **THEN** 表单呈现「该档窗长已被模型名覆盖」提示；非法后缀（如 `[2m]`）在提交后以 error 态呈现保存失败原因

#### Scenario: web_search 仅 anthropic 可见

- **WHEN** 新建 provider（缺省 `openai`）后查看表单，再切协议到 `anthropic`
- **THEN** openai 协议下 `data-testid="provider-web-search"` 不在文档中、提交载荷 `webSearch = false`；切到 anthropic 后 checkbox 出现（缺省不勾）并带「按次计费」提示

#### Scenario: 切协议清零

- **WHEN** 在 anthropic 协议下勾选 `web_search`，随后把协议切回 `openai` 再保存
- **THEN** 勾选就地清零，载荷 `webSearch = false`；再切回 `anthropic` 时勾选不自动恢复（用户需重新勾选）

### Requirement: api_key 机密边界

api_key SHALL 按「后端全链路明文 + 前端展示遮蔽」的机密边界表治理（单用户桌面应用裁决，2026-09-30）：

- **明文允许出现**：写向 IPC body、读向 IPC body、全局库文件、进程内存；
- **明文禁止出现**：`Debug` / 日志输出、git 追踪文件、错误串（含 `Err(String)` 返回面）。

`AgentProviderRecord` SHALL 破例不 derive `Debug`，手写遮蔽 Debug impl（api_key 位输出 `sk-***abc` 形态）——以结构保证明文不进日志，不依赖未来日志配置；key 的测试比较 SHALL 走 `PartialEq`（沿 `EngineConfig` 既有惯例）。机密边界 SHALL 以本 spec 为锚点（design.md 随 change 归档不承锚）；放宽点 SHALL 带统一可 grep 注释标记（固定 token）：`// 机密面有意放宽:…，边界表见 specs/desktop-agent-management`，放置于 `EngineConfig` 注释更新处、`AgentProviderRecord` 类型与 api_key 字段、读写 DTO 的 api_key 字段、前端遮蔽展示组件。

#### Scenario: 遮蔽 Debug

- **WHEN** 审查 `AgentProviderRecord` 类型定义并以含 api_key 的记录触发 `Debug` 格式化
- **THEN** 类型无 `derive(Debug)`、有手写 impl，输出中 api_key 位为 `sk-***abc` 形态且无明文；`assert_eq!` 失败信息与 `PartialEq` 比较行为不受影响

#### Scenario: 标记可 grep

- **WHEN** 以固定 token 检索放宽点注释
- **THEN** `EngineConfig` 注释、`AgentProviderRecord` 类型与 api_key 字段、读写 DTO 的 api_key 字段、前端遮蔽展示组件处标记在场，且各标记指向本 spec 边界表

#### Scenario: 错误串无明文

- **WHEN** 审查管理命令与运行发起的错误返回路径
- **THEN** `Err(String)` 内容不含 api_key 明文（校验与 db 错误信息不拼接机密字段）

### Requirement: 本期边界与后续路线

以下边界 SHALL 作为本期显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **model 三档只存不选**：provider 的 `models {high, medium, low}` 存储完整，消费固定取 high 档；`AgentRunParams` 增 effort 参数 + 前端档位选择器为后续迭代；
2. **workspace 关联 agent 后续改造**：届时运行解析顺序演变为 workspace 关联 → 全局默认；本期数据模型以稳定 id 主键预留支撑（关联引用 id，实体改名不破引用），MUST NOT 预建关联字段或解析分支；
3. **api_key 静态加密落盘（Windows DPAPI）不做**：单用户同机威胁模型下收益边际，未来有合规要求再启；
4. **provider 连通性测试按钮不做**（未要求）。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 四条边界均可考，后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store`（模型 + 操作面） | 管理记录持久化 | `AgentProviderRecord` / `AgentInstanceRecord` 落全局库；稳定 id 主键 + name 唯一二级索引；默认标记至多一（切换原子清旧）；引用删除阻止、删默认清标记；手写遮蔽 Debug（provider）；信封 API 零改动覆盖；`AgentProviderRecord` 可空列 `context_length` / `max_tokens` + `protocol` + `web_search`（serde 缺省读兼容旧记录，envelope V1 → V2 → V3 → V4 逐级 `From` 双向，不做 legacy 迁移层；`None` = 未配置语义贯穿存储层） |
| `commands/agents/`（新轨道） | 管理命令面 | provider / agent CRUD + 默认标记薄包装；全局库实例路由（`global()`）；`Result<T, String>` 模板；重名 / sdk 缺 provider / 引用删除均 `Err`；保存 / 清单 DTO 携带 `context_length`（留空 = `None`，MUST NOT 落 0 或缺省字面值）；`web_search` 无条件接受（保存期零协议判定、零静默清零） |
| `commands/exec`（消费接线） | 运行发起命令面 | 引擎与连接配置解析已下沉内核组合根（desktop-agent-execution），命令层无引擎/agent 解析残留；`DEFAULT_ENGINE` 与 `from_hardcoded_slot()` 退役 |
| 内核组合根（落点见 desktop-agent-execution） | 运行发起解析单点（自 `commands/exec` 下沉） | 缺省解析默认 agent / 显式 agent → (`EngineKind`, `EngineConfig`)；model 取 high 档、`protocol` 映射为门面 `Protocol`；可空调参旁路携带；无默认 `Err` 引导管理页；`runner_for` 签名零 diff、`EngineConfig` 连接配置字段面（含协议）零塌 |
| `packages/desktop/src/views/agents/`（新） | 管理页 | `/agents` 两栏 CRUD + 默认标记 + api_key 遮蔽展示（留空保持原值）；provider 表单 `context_length` 数字字段（留空 = 未配置 + 「跟随缺省 128K」提示，非正整数拒绝提交）+ `web_search` 勾选项（仅 anthropic 渲染、切协议就地清零、提交恒携 `webSearch`）；hooks 取数收口、无轮询 |
| `EngineConfig`（core 组合根） | 可空调参接线零侵入 | 连接配置字段面（协议 + 凭据 + 端点 + 双档模型）与 `runner_for` 签名不塌；`context_length` / `max_tokens` / `web_search` 经 compose `ResolvedEngine` → `EngineFacade::with_context_window` / `with_max_tokens` / `with_web_search` 抵达 sdk 引擎（MUST NOT 经 `EngineConfig`） |
| `packages/desktop/src/views/agent/components/agent-run-form.tsx` | 调试页发起面 | engine 下拉 → agent 选择器（默认选中默认 agent） |
| `packages/desktop/src-tauri/src/bindings/` | IPC 类型镜像 | 经 export-bindings 再生成（管理 DTO 与 agent 参数变化），非手改 |

### Function: `packages/desktop/src-tauri/crates/infra/store`（provider 记录与升级链）

| 项 | 签名 / 形态 | 描述 |
|------|------|----------|
| `AgentProviderProtocol` | `enum { Openai, Anthropic }`（`serde` camelCase 线值 `"openai"` / `"anthropic"`、`Default = Openai`、`specta::Type`） | provider 级模型协议；变体序号为 bincode 解码锚（MUST NOT 重排） |
| `AgentProviderRecord` | `{ id, name, protocol, base_url, api_key, models, context_length: Option<u64>, max_tokens: Option<u64>, web_search: bool }`，`#[native_model(id = 5, version = 4, from = AgentProviderRecordV3)]` | 增 `web_search`（`#[serde(default)]`；手写遮蔽 `Debug` 字段面同步扩位） |
| `AgentProviderRecordV3` | 现 v3 形态（无 `web_search`），`#[native_model(id = 5, version = 3, from = AgentProviderRecordV2)]`，`pub(crate)` | 升级链中转（V1 → V2 → V3 → V4 逐级 `From`；升级补 `false`，降级丢列） |
| `AgentProviderRecordV2` | 现 v2 形态（无 `protocol` / `max_tokens`），`#[native_model(id = 5, version = 2, from = AgentProviderRecordV1)]` | 升级链中转（V1 → V2 → V3 → V4 逐级 `From`；升级补 `Openai` / `None`，降级丢列） |
| `AgentProviderRecord::new` | 签名不变（`id` 置 0 的构造器） | 现有参数面不动（38 处调用点零改）；`web_search` 缺省 `false` |
| `AgentProviderRecord::with_web_search` | `fn(self, web_search: bool) -> Self` | 构造器链式缝（`new` 签名不动的唯一实现路径） |

### API: `packages/desktop/src-tauri/src/commands/agents/mod.rs`（命令面）

| 命令 | 签名 | 描述 |
|------|------|------|
| `save_agent_provider` | `(id: Option<i64>, name: String, protocol: AgentProviderProtocol, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>, max_tokens: Option<u64>, web_search: bool) -> Result<AgentProviderRecord, String>` | 末位追加第 9 参 `web_search`；构造后 `.with_web_search(web_search)`；**不加协议校验**（openai 也收，生效门在解析期）；既有校验（模型名后缀 / `max_tokens >= 4096` / 留空 api_key 回填）原样保留，错误串不含 api_key |
| `list_agent_providers` / `delete_agent_provider` | 签名不变 | 清单自动带出 `webSearch`；删除引用阻止语义不变 |

### Component: `packages/desktop/src/views/agents/`（管理页）

| 项 | 变化 | 描述 |
|------|------|----------|
| `hooks/use-agent-providers.ts`（`AgentProviderSaveInput`） | 增 `webSearch: boolean` | `commands.saveAgentProvider(...)` 实参按新序（末位 `webSearch`）；错误双轨语义不变 |
| `components/provider-panel.tsx`（`ProviderFormState`） | 增 `webSearch: boolean`；`ProviderTextField` 排除该键 | 仅 anthropic 渲染 checkbox（`data-testid="provider-web-search"` + 计费提示）；`onProtocol` 切离 anthropic 时就地清零；submit 恒携 `webSearch` |

### Component: `packages/desktop/src-tauri/crates/infra/agent`（解析产物与门面）

| 项 | 变化 | 描述 |
|------|------|----------|
| `compose::ResolvedEngine` | 增 `max_tokens: Option<u64>` / `web_search: bool`（与 `context_window` 并列的旁路载荷） | 解析单点把 provider `protocol` → `EngineConfig.protocol`、`max_tokens` / `web_search` → 旁路（`web_search` 门控为 `provider.web_search && protocol == Anthropic`） |
| `EngineFacade` | 增 `with_max_tokens(Option<u64>)` builder | 与 `with_context_window` 同型的旁路缝（MUST NOT 进 `EngineConfig`） |
