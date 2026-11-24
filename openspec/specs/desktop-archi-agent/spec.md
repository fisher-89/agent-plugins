# desktop-archi-agent Specification

## Purpose

定义 agent 侧的架构工具面：`archi_module`（定位）/ `archi_search`（发现）/ `archi_api`（契约面自检与草稿）三个专用只读工具的契约、以**工具名字清单**表达的 `(phase, role)` 注入矩阵与 `source = "archi"` 来源臂（全工具面 + 系统知识 preamble）、只读档位归属与成员校验不变，以及「模型 = 源码」的写纪律（无专用写工具，两条写通路并存）。已退役的枚举式 toolset（`SessionToolset`）不在本能力描述范围内。

## Requirements

### Requirement: archi_module 只读工具的定位契约

SDK 引擎 SHALL 提供专用只读工具 `archi_module`（不在七通用工具 `TOOL_NAMES` 封闭集内，经 `SessionToolset` 受控注入），语义为**定位**（已知在看哪个模块）：

```jsonc
{ "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" } }
```

- 指定 `element` 时响应 SHALL 含：`element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine` / `entry`）、`dependsOn`（`target` + `description`）、`dependedBy`（`source` + `description`）、`children`（直接子元素 FQN）、**`doc`（单值入口文档路径，仅 `api = none` 元素有值）**、**`api`（并列契约状态字段）**、**`resolvedEntry`（入口解析结果）**。`docs` 数组与 `warnings` 数组两个位次已退役。
- 省略 `element` 时 SHALL 返回全部元素的**薄概览**（`fqn` / `kind` / `gate` / 一行 `summary` / **`api.status`** / **`hasPath`**，无邻接、无入口文档、无契约路径），并受既有单结果字节上限约束（超限截断留痕）。
- **`resolvedEntry`（入口解析结果）**：SHALL 为 `{ path: string | null, via: "explicit" | "convention" | null, problem: string | null }` 三字段（字段不省略）——`path` 为 repo 相对 POSIX 入口路径，`via` 区分「模型显式 `entry`」与「约定默认命中」（单文件根 = 根文件自身 / 目录型根 = `<root>/index.ts(x)` / crate = `src/lib.rs`），`problem` 在入口不可用或不存在时给出人读成因。判据 SHALL 由 `api::paths::entry_state` **单点**求值（MUST NOT 在工具执行体或 `dto` 内另写入口解析、MUST NOT 复用 `apiFindings` 的判定分支）；该字段 MUST NOT 承载 `entry` 四条判据的长表（判据定义归 `desktop-archi-api` / `desktop-archi-api-check`）。
- **`hasPath`（`path` 事实）**：薄概览每条 SHALL 含 `hasPath: bool`（= 模型 `path` 数组非空，纯投影不补默认值），使「谁缺 `path`」一次调用即可判出；MUST NOT 以「`path` 字段是否省略」表达同义（显式字段不省略是既有投影纪律）。
- `modelFile` + `modelLine` SHALL 来自解析器已产出的声明位置（零额外实现），使 agent 能直接 `edit` 到该行而无需 grep 猜测。
- 元素不存在 SHALL 返回可诊断 `Err`（附近似 FQN 或总量提示），MUST NOT 返回空对象。
- 模型结构错时 SHALL 返回结构化诊断清单作为 `Err`（绝不 panic），使 agent 能看出"模型本身坏了"而不是"没有这个模块"。
- 响应 SHALL 经 `archi::dto` 单点投影产出（MUST NOT 在工具执行体内另拼一份 JSON）。
- **`api` 并列字段**：单元素响应 SHALL 并出 `api` 对象（`status`（`declared` / `none`）/ `contractPath` / `schemaPath` / `generatedPath`），定位 SHALL 按 `<archi_models_dir>/<分层目录…>/<FQN>.md` 与相应的 `.schema.json` / 生成物路径**派生**（全经 `foundation::layout` 的派生族与 `api::paths` 单点，MUST NOT 行内拼路径；`api` 段已退役，契约资产落在契约资产目录 `models/` 的 `<分层目录…>/` 子目录内）；文件不存在即 `status = none` 且三个路径字段为 `null`（零配置，「有契约 / 无契约」成为可枚举事实）。该字段 MUST NOT 承载协议正文（正文由 agent 用 `read` 按路径自取），MUST NOT 成为独立工具、MUST NOT 改变本工具的只读属性与注入矩阵。
- **`doc` 并列字段**：`doc` SHALL 为 repo 相对 POSIX 路径**单值**或 `null`（无 `title` / `kind` / `source` 四字段形态）。只有 `api = none` 的元素会填；`api = declared` 元素的入口文档走既有 `api.contractPath`，MUST NOT 在 `doc` 里重复出线。`doc` 指向的文件不存在时该元素的判据面报 `docNotFound`（`error` 档）——工具响应本身 MUST NOT 为此降级或补默认值。
- **描述面（`mount.rs`）**：`archi_module` 的工具描述 SHALL 含四句——「缺 `path` 的元素可由 `hasPath` 一次判出」「`resolvedEntry` 给入口解析结果（显式 / 约定默认 / 不可用成因）」「新元素字段形态用 `archi_scaffold` 取模板」「`api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）」；且 MUST NOT 复制字段语法长表（字段序 / 内联 `metadata` 的形态知识归模板工具）——MUST NOT 残留 `docs` 词汇或警告语义。

#### Scenario: 定位单元素

- **WHEN** 模型会话中调用 `archi_module(element="App.Auth")`
- **THEN** 返回该元素的 `kind` / `gate` / `path` / `metadata` / `modelFile` / `modelLine` / `entry`，其出边与入边（各含对端 FQN 与 `description`）、直接子元素清单、`doc`（单值或 `null`）、`resolvedEntry`；响应内 MUST NOT 出现 `docs` 数组

#### Scenario: 省略元素给薄概览并截断

- **WHEN** 在不传 `element` 的调用下模型含数百元素
- **THEN** 返回全元素薄概览（含 `hasPath`，无邻接、无入口文档），超出单结果字节上限时截断并留痕（提示可指定元素收窄）

#### Scenario: 缺 path 一次判出

- **WHEN** 省略 `element` 调用，模型内既有声明了 `path` 的叶元素、也有无 `path` 的容器元素
- **THEN** 前者 `hasPath = true`、后者 `hasPath = false`（字段不省略）；`path` 数组本身只在单元素响应出线

#### Scenario: 入口三态可辨

- **WHEN** 分别对「显式 `entry` 命中」「目录型根由 `index.ts` 约定默认命中」「`path` 指向不存在目录而入口不可用」的三元素调用 `archi_module(element = …)`
- **THEN** 依次为 `resolvedEntry = { path: "<显式值>", via: "explicit", problem: null }` / `{ path: "<index 路径>", via: "convention", problem: null }` / `{ path: null, via: null, problem: "<不可用成因>" }`；判据与 `apiFindings` 的 `apiEntryMissing` 逐例一致（同一 `entry_state` 求值）

#### Scenario: 坏模型可辨

- **WHEN** 模型文件被写坏（语法错 / 环）后调用 `archi_module`
- **THEN** 返回结构化诊断（含 `file:line`）且 `is_error` 为真；执行体不 panic，run 不中断（agent 可转去修模型）

#### Scenario: api 并列字段两态可辨

- **WHEN** 分别对契约在案的元素（`Backend.Infra.ArchiRuntime`）与无契约元素调用 `archi_module(element = …)`
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `.dev-team/architecture/models/Infra/` 下的实际文件、`generatedPath` 指向入口同级生成物；后者 `api.status = "none"` 且三者 `null`

#### Scenario: doc 只在 api = none 元素上有值

- **WHEN** 分别对 `api = none` 且声明了 `doc` 的元素、`api = none` 未声明 `doc` 的元素、`api = declared` 的元素调用 `archi_module`
- **THEN** 依次为「路径串」/ `null` / `null`（字段不省略）；declared 元素的入口文档只能经 `api.contractPath` 读到——两处 MUST NOT 同时给出口 entry 文档（否则元素信息面板会出现两条）

#### Scenario: 薄概览带 api.status 但不带路径与 doc

- **WHEN** 省略 `element` 调用 `archi_module`
- **THEN** 每条薄概览含 `api.status`（`declared` / `none`）与 `hasPath`，不含 `contractPath` / `schemaPath` / `generatedPath` / `doc` / `resolvedEntry`；超单结果字节上限时仍按既有纪律截断留痕

#### Scenario: 契约正文不经工具响应

- **WHEN** 审查 `archi_module` 响应体与体积
- **THEN** 只含 `api` 四字段（状态 + 三路径）、`doc` 单值、`resolvedEntry` 三字段与 `hasPath` 布尔；零签名、零 `$defs`、零散文、零四字段文档条目；正文由 agent 用 `read` 按路径自取

#### Scenario: 契约路径零 `api` 段残留

- **WHEN** 对本仓契约在案元素（`Backend.Infra.ArchiRuntime`）取 `archi_module` 响应，并审查响应内的三个路径串
- **THEN** `contractPath` 为 `.dev-team/architecture/models/Infra/Backend.Infra.ArchiRuntime.md`、`schemaPath` 为同目录的 `.schema.json`；三串内 MUST NOT 出现 `architecture/api` 形态（旧段已退役）；派生点消失后仍由 `api::paths` 单点产出（工具执行体零路径拼接）

### Requirement: archi_search 只读工具的轻量检索契约

SDK 引擎 SHALL 提供专用只读工具 `archi_search`，语义为**发现**（还不知道这功能归谁管）：

```jsonc
{ "name": "archi_search",
  "parameters": {
    "query": "必填。关键词 / 元素名片段 / 入口文档路径片段",
    "scope": "可选。'elements' | 'doc' | 'all'（缺省 all）" } }
```

- 匹配对象 SHALL 恰为：元素 FQN、`summary`、`metadata` 值、**入口文档路径与文件名**（`path` 非空元素的在盘载体：`api = declared` 取派生 `contractPath`、`api = none` 取 `doc`）。MUST NOT 匹配文档正文、MUST NOT 匹配 `openspec/specs/` 下任何路径或内容、MUST NOT 匹配 `decisions/*.md`（ADR 已退出自动检索语料）。
- 匹配方式 SHALL 为 ASCII 大小写不敏感子串 + **驼峰分词**（`AuthToken` → `auth` / `token`，两段均参与匹配）。
- `scope = "elements"` 时文档结果恒为空数组；`scope = "doc"` 时 `elements` 恒为空数组。**`scope` 的值名 SHALL 恰为 `elements` / `doc` / `all`**：`docs` 是非法值（`parse` 报错并列出三个合法值），MUST NOT 保留别名——双词会让 scope 语义二义。
- 文档结果条目 SHALL 退化为**一条 repo 相对路径字符串**（`ArchiSearchResult.doc: Vec<String>`），无 `title` / `kind` / `source` 四字段形态；排序 SHALL 按路径字典序（元素仍按 FQN 字典序），MUST NOT 引入相关度打分或来源强度序（来源概念已不存在）。
- 实现 SHALL 为每次现算（按元素载体派生 + 存在性判定，零遍历旁路），MUST NOT 建倒排索引或跨调用缓存（不引入第二事实源）。
- `query` 为空白串 SHALL 显式 `Err`（参数校验），MUST NOT 返回全量。
- 结果 SHALL 受既有单结果字节上限约束。
- **描述面（`mount.rs`）**：工具描述与 `scope` enum SHALL 改为 `["elements", "doc", "all"]`，描述内「四字段文档条目」的措辞改为「入口文档路径」。
- **语料面与判据面同源**：文档语料集合 = 「`path` 非空元素」这一判据面同一集合（容器 / 包根不进语料）；MUST NOT 为检索面另立第二份载体派生。

#### Scenario: 关键词命中元素与入口文档

- **WHEN** 调用 `archi_search(query = "archi", scope = "all")`
- **THEN** 命中 FQN / summary 含该词的元素，与路径含该词的入口文档（如 `.dev-team/architecture/models/Infra/Backend.Infra.ArchiRuntime.md`）；结果含元素薄条目与路径串，MUST NOT 含四字段条目

#### Scenario: 驼峰分词

- **WHEN** 调用 `archi_search(query="AuthToken")`
- **THEN** 命中 `App.Auth.Token`（分词后 `auth` 与 `token` 两段均参与匹配）

#### Scenario: 正文、specs 与 ADR 不参与

- **WHEN** `query` 只出现在某文档正文内、或只出现在 `openspec/specs/` 的路径 / 内容内、或只出现在 `decisions/*.md` 内
- **THEN** 均不命中（刻意不做全文搜索，正文搜索归 agent 既有 `grep`；ADR 经入口文档链接 / 文件树 / 通用搜索抵达）

#### Scenario: scope 换词与空串拒绝

- **WHEN** 分别以 `scope = "elements"` / `scope = "doc"` / `scope = "docs"` / `query = ""` 调用
- **THEN** 前两者分别使另一半结果为空数组；`"docs"` 显式 `Err` 并列出 `elements` / `doc` / `all` 三个合法值；空 query 显式 `Err`（MUST NOT 返回全量）

### Requirement: archi_api 只读工具的自检与草稿契约

SDK 引擎 SHALL 提供专用**只读**工具 `archi_api`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**契约面自检与草稿**（与本 requirement 标题同口径：`check` 自检、`bootstrap` 起草）：

```json
{ "name": "archi_api",
  "arguments": { "action": "check" | "bootstrap", "element": "<FQN>" } }
```

- **两动作封闭**：`action` SHALL 只接受 `check` / `bootstrap` 两值（封闭值集；词汇外值以 `is_error` 回灌并点名合法值，MUST NOT 静默退化）。
- **`check` = 两轴结构化结果**：返回「生成物新鲜度」与「契约 vs 代码」两轴的并列内容——① 三类差异（`mismatched` / `orphaned` / `missing`，repo 相对串，取自与 IPC / CLI 同一份比对实现）；② 契约判据摘要（按 `category` 计数的映射 + 逐条 `{ kind, category, element, target, message }`）。响应 SHALL 复用既有出线形状（`ArchiApiCheck` 的字段与 `ArchiApiFinding` 的条目），MUST NOT 在工具执行体内另拼一份投影；MUST NOT 各自实现在第二处（判定单源）。
- **`bootstrap` = 草稿文本（起草第一步）**：`element` 必填，返回与 CLI `bootstrap` 同一份草稿文本（契约 md 骨架，只出 stdout 语义）；该动作是新建 / 补契约的**第一步**，描述面 SHALL 说出这句指路（否则 agent 只知它能出草稿、不知该先用它）。草稿形态（六段骨架 + 尾部惰性作者注）归 `desktop-archi-api`「bootstrap 草稿的六段骨架与惰性作者注」，本能力 MUST NOT 另立第二份形态判据。`element` 缺失 / 元素不存在 / 无实现入口 ⇒ `is_error` 并透传 `ApiError` 文案。
- **零写盘**：两动作 MUST NOT 创建 / 覆盖 / 删除任何文件——生成仍由会话经既有 `bash` 工具执行本仓的生成命令（命令名归属本仓文档面，产品 MUST NOT 内建具体命令名）。MUST NOT 存在「写生成物」的专用工具（`archi-runtime` 对 agent 面不可达，见 `desktop-crate-layout`）。
- **结构错以 `is_error` 回灌**：模型含任一结构诊断时两动作 SHALL 返回 `is_error` ToolResult（诊断清单带 `file:line`），MUST NOT 返回部分结果、MUST NOT 抛 panic。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）；超限时摘要（按 `category` 计数）优先保留。
- **上下文路由沿用**：执行体 SHALL 经 `LoopTurn` 的 per-turn `ArchiContext` 路由（root + 模型读面），MUST NOT 新增 `ToolExec` 变体、MUST NOT 把 store 句柄塞进七通用工具的 `execute` 签名。
- **无路径参数**：本工具不带 `path` 参数 ⇒ 路径沙箱对它恒原样返回（MUST NOT 因缺字段报错），写入保护对它零射程（读面全开放）。
- **提示面**：被下发本工具的相位 prompt SHALL 有一行说明（沿既有「额外可用只读工具」写法，当前为 `implementation-generator.md`），内容限于「改完代码可自查生成物新鲜度与契约判据；生成物过期用既有命令重生成，MUST NOT 手改生成物」。
- **描述面（`mount.rs`）**：`archi_api` 的工具描述 SHALL 含三句，且 MUST NOT 复制契约 md 的段形态长表（形态归 `desktop-archi-api`）、MUST NOT 内建本仓专用命令名（生成命令归属各仓文档面）——① `bootstrap` 是新建 / 补契约的第一步；② 四类生成物的点名（契约 md 同目录 `<FQN>.schema.json`、架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）连同「MUST NOT 手改生成物」；③ 冷启动口径「`artifacts.enabled = false` 即契约面未启用，此时 `findings` 恒空且**不等于**零问题」。

#### Scenario: check 返回两轴且零写盘

- **WHEN** 在 implement/executor 会话（或架构页 `source = "archi"` 会话）中调用 `archi_api(action = "check")`
- **THEN** 响应含三类差异与按 `category` 计数的判据摘要（逐条含 `kind` / `element` / `message`）；调用前后磁盘字节完全相同；`archi_api` 与 IPC 的 `archi_api_check` 报的三类差异集合逐条相等

#### Scenario: bootstrap 只出草稿

- **WHEN** 调用 `archi_api(action = "bootstrap", element = "App.Auth")`
- **THEN** 返回契约 md 草稿文本（H1 + 六段：职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由，末尾带惰性作者注——形态归 `desktop-archi-api`），磁盘零变化；缺 `element` 或元素不存在 ⇒ `is_error` 且文案点名成因

#### Scenario: 词汇外动作被拒

- **WHEN** 调用 `archi_api(action = "generate")` 或 `action = "write"`（词汇外值）
- **THEN** 返回 `is_error` 并点名合法值 `check` / `bootstrap`；零写盘、零副作用

#### Scenario: 结构错回灌而不崩

- **WHEN** 模型被写成语法错后调用 `archi_api(action = "check")`
- **THEN** 返回 `is_error` ToolResult（诊断带 `file:line`），run 不中断、进程不 panic

#### Scenario: 体量经单点上限

- **WHEN** 契约面有数十条判据时调用 `check`
- **THEN** 响应被 `tools::cap_result` 截断（沿既有 L1 上限），摘要保留；工具执行体内零第二份截断实现

### Requirement: archi_scaffold 只读工具的新元素模板契约

SDK 引擎 SHALL 提供专用只读工具 `archi_scaffold`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**给模板**（要写新元素但记不住字段形态）：

```jsonc
{ "name": "archi_scaffold",
  "parameters": { "kind": "可选。元素 kind 四值之一（值集由 ElementKind::ALL 派生）；省略取 domain" } }
```

- **值集封闭且派生**：`kind` 的合法值 SHALL 由 `ElementKind::ALL` 派生（MUST NOT 手抄四值）；词汇外值 SHALL 以 `is_error` 回灌并**点名**合法值（MUST NOT 静默退化为缺省）。
- **响应形状**：`{ "draft": "<TOML 片段>" }`——**一段**元素表模板，字段按模型字段面顺序列出（`kind` 实值；`gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 以注释行给出并各附一行纪律），另附一条 `[[relation]]` 模板注释。
- **模板自证可解析**：草稿 SHALL 为合法 TOML 且经 `archi::parse` 的 `TomlParser` 解析后**零结构诊断**（机械锚：字段名 / 键形 / `metadata` 形态 / `kind` 四值语义任一漂移即红）。纪律性提示 SHALL 只出现在注释行——MUST NOT 以注释外的非法行（如 `path = "<待填>"`）破坏可解析性。
- **零写盘**：本工具 MUST NOT 创建 / 覆盖 / 删除任何文件（与 `archi_api` 的 `bootstrap` 同款「只出 stdout 语义」）；MUST NOT 成为「写模型工具」（写面仍是既有 `write` / `edit`，见「模型 = 源码（写纪律与不设专用写工具）」）。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）。
- **只读档位**：归 `ToolTier::Read`（三档全放行），无路径参数（沙箱原样返回、写入保护零射程）。

#### Scenario: 草稿可解析且含必填提示

- **WHEN** 调用 `archi_scaffold()`（省略 `kind`）与 `archi_scaffold(kind = "module")`
- **THEN** 两次均返回单段元素表草稿（后者的 `kind` 行取 `module`）；草稿经 `TomlParser` 解析零结构诊断；`path` 的「模块必填（可多条）/ 包至多一条包根 / `domain` · `composite` 选填」与 `metadata` 的「内联表与子表等价」两句以注释行在场，MUST NOT 再出现「子表会被读成另一个元素」一类假因果

#### Scenario: 模板 kind 行覆盖四值语义

- **WHEN** 遍历 `ElementKind::ALL` 四次调用 `archi_scaffold(kind = …)`
- **THEN** 每次草稿的 `kind` 纪律注释都列出四个值各自的语义（`package` 至多一条包根 / `domain` 领域容器 / `module` 模块 / `composite` 纯分组），MUST NOT 缺任一值——`composite` 不得再是「零承载」的第四值

#### Scenario: 词汇外 kind 被拒

- **WHEN** 调用 `archi_scaffold(kind = "service" )`（词汇外值）
- **THEN** 返回 `is_error` 并点名 `package` / `domain` / `module` / `composite` 四值；零写盘、零副作用

#### Scenario: 模板不写盘

- **WHEN** 在任一相位调用 `archi_scaffold` 并比对调用前后的架构域字节
- **THEN** 磁盘零变化（模型文件、契约资产、生成物均逐字不变）

### Requirement: archi_drift 只读工具的漂移读数契约

SDK 引擎 SHALL 提供专用**只读**工具 `archi_drift`（不在七通用工具 `TOOL_NAMES` 封闭集内，经工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**读漂移判据**（写完模型 / 改完代码后自查覆盖、归属与契约面）：

```jsonc
{ "name": "archi_drift",
  "parameters": { "scope": "可选。'summary'（缺省）| 'entries' | 'files'" } }
```

- **恒全量、参数面封闭**：本工具 SHALL 只以 `mode = Full` + `changed = []` + `change = None` 调判定（archi 臂 root = 会话 cwd = 主仓根，无 change 上下文 ⇒ `HEAD` 基准恒空 diff，增量本无意义）；参数面 MUST NOT 含 `mode`，MUST NOT 含 `root`（取 `PerTurn` 的 `ArchiContext.root`）。
- **三值 `scope`**：值名 SHALL 恰为 `summary` / `entries` / `files`（与 `archi_search` 的 `elements` / `doc` / `all` 同风格）；词汇外值 SHALL 以 `is_error` 回灌并点名三个合法值，MUST NOT 静默退化或缺省。
- **逐级追加的响应形状**（同一投影逐级加字段，MUST NOT 三份拼装）：
  - `summary`（缺省）：`mode` / `status` / `passed` / `modelHash` / `model`（元素数、关系数、`byGate` 分布、`structuralErrors` 计数与明细）/ `counts`（`errors` / `warnings` / `suppressed` / `observations` / 契约判据三档计数 / 扫描四项计数）/ **`errors` 与 `warnings` 与 `modelWarnings` 全量明细** / 契约判据 `byKind` 计数 / `durationMs`。
  - `entries`：追加 `observations` 全量与契约判据条目全量。
  - `files`：追加 `files`（`unmatched` / `matched` / `excluded` / `unresolved` 全量）。
- **`gate` 全 `off` 仓库的语义陷阱必须进描述**：`gate` 全 `off` 的仓库会把 `off` 元素覆盖的全部文件推入 `unmatchedFiles[]`（数百条），这是「`off` 元素不参与定级」的常态而非灾难——工具描述 SHALL 点名该事实，`summary` 里该数 SHALL 只出现在**计数位**（明细只在 `scope = files` 出现），MUST NOT 让模型把常态读成错误。
- **投影单点**：响应 SHALL 由 `sdk/archi_drift_view` 的 `drift_summary` （及 `drift_entries` / `drift_files`）投影产出，工具执行体 MUST NOT 另拼一份 JSON；`durationMs` SHALL 由执行体以 `std::time::Instant` 计时（MUST NOT 改 `core/archi` 的报告结构）。
- **错误面**：模型结构错 / 看护配置非法 / 扫描硬失败 SHALL 以 `is_error` 回灌（透传 `CheckError::message`，`Config` 文案含具体条目，如「`ignore` 第 N 条缺 `to`」），MUST NOT 返回部分结果、MUST NOT panic。
- **零跨调用缓存**：每次调用 SHALL 现算（沿 `archi_search` 纪律），MUST NOT 在进程内缓存报告；`durationMs` 是让重复调用成本可见的唯一手段。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result`（摘要优先保留：截断时计数仍在）；描述 SHALL 提示 `files` 只在排查归属问题时用。
- **只读档位**：归 `ToolTier::Read`（三档全放行），无路径参数（沙箱原样返回、写入保护零射程）。

#### Scenario: 三 scope 逐级追加

- **WHEN** 依次以 `scope = "summary"` / `"entries"` / `"files"` 调用 `archi_drift`
- **THEN** `summary` 含 `status` / `passed` / `counts` / `errors` / `warnings` / `modelWarnings` / 判据 `byKind` 计数与 `durationMs`，但无 `observations` / 判据条目 / 文件清单；`entries` 追加观察项与判据条目；`files` 再追加扫描四类清单；三者的 `counts` 逐字段相等

#### Scenario: unmatched 常态只在计数位

- **WHEN** 在本仓（`gate` 全 `off`）以缺省 scope 调用
- **THEN** `counts.scan.unmatched` 为数百条量级的**一个数**，响应内零 `unmatchedFiles` 路径串；工具描述含「`gate` 全 `off` 时 `unmatchedFiles` 是常态不是错误」一句；以 `scope = "files"` 时才列出路径（并经 `cap_result` 截断留痕）

#### Scenario: 词汇外 scope 与坏模型

- **WHEN** 分别以 `scope = "full"`（词汇外）与在模型被写坏后以缺省 scope 调用
- **THEN** 前者 `is_error` 并点名 `summary` / `entries` / `files` 三值；后者 `is_error` 且诊断带 `file:line`，run 不中断、进程不 panic

#### Scenario: 重复调用各自计时零缓存

- **WHEN** 在同一轮内连续两次调用 `archi_drift`
- **THEN** 两次响应各自含独立 `durationMs`、`modelHash` 逐字相同；源码复核无跨调用缓存的报告容器

### Requirement: archi 工具面注入与 (phase, role) 矩阵

archi 专用工具的**下发面** SHALL 由 `agent-runtime` 组合根以**工具名字清单**表达（`SessionInjections.tools: Option<Vec<String>>`；`None` = 七内建全量），MUST NOT 依赖已退役的枚举式 toolset 概念。清单内每个名字 SHALL 经 `mount::lookup` 成员校验（未知名显式 `Err` 携名与来源，不静默滤过）；自由字符串工具名 MUST NOT 进执行面。

会话装配（`crates/infra/agent/src/compose.rs`）SHALL 按 `(source, phase, role)` 决定下发面：

| phase | role | `archi_module` | `archi_search` | `archi_api` | `archi_drift` | `archi_scaffold` |
|---|---|---|---|---|---|---|
| `proposal` | executor | ✅ | ✅ | ❌ | ❌ | ✅ |
| `dev-design` | executor | ✅ | ✅ | ❌ | ✅ | ✅ |
| `dev-design` | evaluator | ✅ | ✅ | ❌ | ❌ | ❌ |
| `test-design` | executor | ❌ | ✅ | ❌ | ❌ | ❌ |
| `implement` | executor | ✅ | ❌ | ✅ | ✅ | ❌ |
| `code-review` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ |
| 其余相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync 等） | 任意 | ❌ | ❌ | ❌ | ❌ | ❌ |

- **矩阵单源**：`(phase, role)` → archi 工具名清单的映射 SHALL 只在 `archi_matrix` 一处（未命中即零 archi 工具），MUST NOT 在别处再写一份同名映射。`dev-design` 的 executor / evaluator **分两行**（`drift` 只给写模型的执行侧），MUST NOT 合并成一行后按 role 分支。
- **`archi_drift` 只给两处直接产出漂移的相位**：`dev-design/executor`（写元素与边）与 `implement/executor`（改代码 ⇒ 未声明依赖）。`code-review` / `acceptance` MUST NOT 下发（越界判断仍只读模型面；「时机差」作为将来放宽的理由需独立裁决，MUST NOT 由本期实现顺手放开）。
- **`archi_scaffold` 只给两处写权相位**（`proposal/executor`、`dev-design/executor`）与 archi 来源臂——模型写权之外的下发等于许诺一个不会被允许落地的模板。
- **`archi_api` 只给 implement/executor 与 archi 臂**：其余相位 MUST NOT 下发（生成物新鲜度由门禁与契约面板承担）。
- **archi 来源臂**：`source = "archi"` 的会话 SHALL 装配为七内建全量（含 `write` / `edit` / `bash`）+ 五个 archi 工具名（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`）+ `preamble = Some(<架构会话知识文本>)`（知识内容见「架构会话的系统知识注入」）+ `before_done = Some(ArchiPreclose)`（收口前自检装配档位，见「收口前引擎自检（架构判据读数）」）。该臂 SHALL 以 `source` 短路命中，MUST NOT 进入 `source_ref` 相位解析——`source_ref` 为 null（一 workspace 一份历史）与该定式无关，且其取任意值（含畸形四段）MUST NOT 改变该臂的下发面，MUST NOT 因解析失败退化为零 archi 工具。
- `phase` / `role` SHALL 取自既有 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`（不新增契约字段）；解析 SHALL 取尾三段的定式（`attempt` → `role` → `phase`），段数 < 4 或末段非数字即 `None`。
- 解析不出该定式（畸形 / 短段 / 未知 phase）SHALL **fail-closed**：零 archi 工具（MUST NOT 退化为"给全部工具"）。
- 相位矩阵 SHALL 只对 `source = "change"` 生效（archi 来源臂不查矩阵、恒五工具全开）。
- `source = "explore"` 臂的 **archi 面 SHALL 恒为空**（零 archi 工具，逐字沿用）；该臂的**完整**名字清单（六工具收窄 + `create_explore_note` + 只读 DB 工具 `db_tables` / `db_query` 两件）单源见 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」与 `desktop-agent-db-tools`，本能力 MUST NOT 另写一份；其余**非 change、非 config** 来源（debug 等）SHALL 恒「七内建全量 + 零 archi 工具 + `ask = Generic`」的显式构造（`source = "config"` 臂的名字清单与位取值单源见 `desktop-config-agent` 与 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」；该臂零 archi 工具、`preamble` 为配置协议知识而非架构会话知识，本能力 MUST NOT 另写一份；MUST NOT 以默认构造兜底——默认值的 `ask = None` 即静默禁用）。
- 矩阵只决定**下发面**，不决定 prompt 文本（prompt 侧见 `desktop-phase-prompts`）；`preamble` 位只由装配层按来源决定（生产者 = archi 臂与 config 臂），前端 / 自由字符串 MUST NOT 直接供给。

#### Scenario: 逐相位矩阵命中

- **WHEN** 分别以 8 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** 每行 definitions 中 archi 工具恰为矩阵勾选者（proposal 为 `module` / `search` / `scaffold`；dev-design 执行为 `module` / `search` / `drift` / `scaffold`、评估为 `module` / `search`；implement 为 `module` / `api` / `drift`；code-review / acceptance 只 `module`），`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` / `(code-analyze, executor)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；该轮调用 archi 工具按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具（回落七内建全量），MUST NOT 抛出装配错误、MUST NOT 给全量 archi 工具

#### Scenario: archi 来源全工具面加知识

- **WHEN** 以 `source = "archi"`（`source_ref = null`）发起会话并审查该轮请求组装
- **THEN** `tools` 位为七内建全量（含 `write`）另加五个 archi 工具名、`preamble` 为架构会话知识文本、`before_done` 为 `Some(ArchiPreclose)`；definitions 含五个 archi 定义且 `RunStarted.tools` 逐名一致

#### Scenario: archi 臂不因 source_ref 变形漂移

- **WHEN** 分别以 `source_ref = null`、`source_ref = "some-id"`、`source_ref = "a/b/c/1"` 发起 `source = "archi"` 会话
- **THEN** 三者装配结果完全相同（同一 archi 臂：五工具全开 + 七内建全量 + 知识 preamble）；MUST NOT 因 `source_ref` 形态触发相位解析或 fail-closed

#### Scenario: explore 与 debug 来源零 archi 工具

- **WHEN** 分别以 `source="explore"` 与 `source="debug"` 发起会话
- **THEN** explore 为 read / grep / glob / ls / edit / bash + `create_explore_note` + `db_tables` / `db_query`（只读 DB 工具两件，清单单源见 desktop-agent-db-tools），debug 为七内建（含 `write`）；两者均无 archi 工具、`preamble` 均为 `None`

### Requirement: 只读档位归属与成员校验不变

五个 archi 工具（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`）SHALL 归 **只读面**：挂载清单（`sdk/mount.rs`）中五者的准入档 SHALL 为 `ToolTier::Read`，由此 `policy::allows` 对三档（`default` / `acceptEdits` / `bypassPermissions`）**全部放行**——只读工具在 `default` 档亦可用，与 `read` / `grep` / `glob` / `ls` 同款；MUST NOT 为任一 archi 工具引入单独的放行名单（准入判据单点在挂载清单的准入档）。

以下既有语义 MUST NOT 变化：

- **成员校验先行**：调用名 ∉ 本轮实际下发 definitions 即 `SystemNotice{subtype:"tool_unavailable"}` + `is_error` ToolResult 回灌，run 不中断、不触执行体；
- **路径沙箱不介入**：五工具无路径参数，`sandbox` 改写面对其恒原样返回（MUST NOT 因缺 `path` 字段报错）；
- **写入保护不介入**：五工具为读面，`write_protection` 的守卫 MUST NOT 拦截（读面全开放是既有裁定；`archi_drift` 执行的判定本身零写盘）；
- **未知工具仍拒**：词汇外的工具名在任何档位一律拒绝（封闭清单语义不变）。
- 工具执行上下文 SHALL 经 `LoopTurn` 的 per-turn 上下文路由（root + 模型读面），MUST NOT 把 store 句柄塞进七通用工具的 `execute` 纯函数签名；上下文缺席 SHALL 以 `is_error` 回灌（生产不可达）。铸造判据 SHALL 为「本轮名字清单含任一 archi 工具名」（五名逐一在场判断，MUST NOT 漏掉新增两名）。

#### Scenario: 默认档放行五个 archi 工具

- **WHEN** 以 `permissionMode = default` 在矩阵命中相位调用 `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`
- **THEN** 五者均放行并执行（无 `permission_denied`）；同档调用 `write` 仍被拒（只读面未扩宽）

#### Scenario: 矩阵外调用被成员校验拒绝

- **WHEN** 在 `(test-gen, executor)` 会话中模型幻觉调用 `archi_search` 或 `archi_drift`
- **THEN** 收到 `tool_unavailable` + `is_error` ToolResult 回灌，run 不中断、执行体零调用

#### Scenario: 沙箱与写入保护零介入

- **WHEN** 在五个 archi 工具被下发的会话中审查 `sandbox` 改写与写入保护判定
- **THEN** 五者均原样通过（无路径参数、读面无保护射程）；受保护文件的 `write` / `edit` 行为逐字不变

#### Scenario: 挂载清单项数

- **WHEN** 审查 `mount::registry()` 的项数与归组
- **THEN** 恰 **16** 项：七内建 + `create_explore_note` + 五 archi 只读 + 两 DB 只读 + `propose_config`（`ToolTier::Read` / `ToolExec::Config`）；五 archi 项均 `ToolTier::Read` 且 `ToolExec::Archi`

### Requirement: 模型 = 源码（写纪律与不设专用写工具）

架构模型文件 SHALL 与源码同权：agent SHALL 经既有 `write` / `edit` 直接新建与修改 `<archi_dir>/graph/model/*.toml`，MUST NOT 存在专用「写模型」工具。写权 SHALL 有且只有**两条通路**，且**通路 A 按相位收窄**：

| 通路 | 触发面 | 执行根 | 确认时机 | 归属能力 |
|---|---|---|---|---|
| A（既有，本变更收窄） | change 流程相位会话——**仅 `phase ∈ {proposal, dev-design}` ∧ `role = executor`** | `detail.worktree` | 回合末归档确认面（模型区块 + 冲突分流） | `desktop-change-archive` |
| B（新增） | 架构页通用会话（`source = "archi"`），不限相位 | workspace root（主仓，`cwd = null` 回落） | 事后 `git diff` 评审 + 用户手动刷新校验 | `desktop-archi-chat` |

- **写权不是工具面问题，是强制面问题**：`archi` crate 全程只读盘（模型唯一落盘点是壳层 `write_atomic` 单点），故「写模型的工具」就是既有的 `write` / `edit`；本政策实做是**掐掉其余相位对模型目录的射程**——由会话级写入禁区（第三规则来源）承载，见 `desktop-write-protection`「会话级写入禁区（相位锁）」。MUST NOT 为写模型新增专用工具（`archi_scaffold` 是**只读模板工具**，零写盘，不在其列）。
- **两条推论（政策是绝对命题，不是步骤枚举）**：
  - **`dev-design/evaluator` 被锁**：白名单按 `(phase, role)` 而非 phase——评审批次不写模型。
  - **`explore` / `debug` 也被锁**：两者不是「步骤」，但政策覆盖一切来源；explore 的 `edit` 只服务笔记，锁模型零损失。
- **写入不校验，读取报错**：写盘路径 MUST NOT 增加模型校验插口；坏模型的表现是读取面返回结构诊断（见「archi_module 只读工具的定位契约」）与画布停摆，与"写坏 Rust 代码编译不过"同构。
- **不依赖运行时审批**：写盘那一刻 MUST NOT 要求用户确认（工具分派链只有 `deny`，无挂起等确认能力）。通路 A 的设计权由「回合必须人工确认」承载；通路 B 的设计权由事后 diff 评审 + 手动刷新可见性承载（MUST NOT 承诺写盘前拦截）。
- **内置锁面仍不纳入**：`<archi_dir>/graph/model/` 与架构域配置 MUST NOT 进入 `write_protection` 的**内置**锁面（内置规则是相位无关的绝对路径 glob，纳入即等于两条通路都改不了模型）；相位条件的锁 SHALL 走**会话级**规则面（见 `desktop-write-protection`「会话级写入禁区（相位锁）」）。
- **两通路并存，互不替代**：MUST NOT 因通路 B 存在而移除通路 A（change 侧 worktree 隔离 + 归档确认仍是「改动随 change 一起评审」的唯一形态）；亦 MUST NOT 因通路 A 收窄而收紧通路 B（架构页会话的用途恰是「不进 change 也能维护模型」）。
- **场景归属**：冷启动生成骨架 / 增量维护（新增或重构模块）/ 按现状补声明 / 直接修复图或文档错误，四类场景 SHALL 可走任一条通路（走 change 时落 `dev-design` 阶段，走架构页时落通用会话），MUST NOT 强制单一入口。非白名单相位遇到「模型该改」的需求 SHALL 把需求写进产物（design / tasks）交 `dev-design` 落地，MUST NOT 尝试绕过。
- **新元素缺省档**：两通路生成的新元素均缺省 `gate = "off"`（ratchet 逐块收紧）。
- **通路 B 的边界（软约束，非写入面强制）**：会话知识 SHALL 要求「禁止修改架构目录之外的文件（源码 / 配置 / 脚本 / 域外文档一律不改，用户要求亦不越界；生成物重生成除外）」，但写入面 MUST NOT 为该约束设硬隔离——该缺口 SHALL 在能力 spec 中如实留痕，MUST NOT 被表述为「会话只能改模型」。
- **生成物同权但不可手改**：四类契约生成物（`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts`）与模型文件同属磁盘可写面（零专用工具、零写前校验），但会话知识 SHALL 要求「MUST NOT 手改生成物，用既有命令重生成」——该约束同样只有事后兜底（零 diff 门 + 契约面板），MUST NOT 表述为写入面拦截。

#### Scenario: worktree 内直改模型成功（白名单相位）

- **WHEN** agent 在 `dev-design/executor` 会话中用 `write` 新建 `<archi_dir>/graph/model/03-billing.toml`，或用 `edit` 修改既有模型的 `gate` 行
- **THEN** 写入成功（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后读取面解析出该改动，主仓模型零变化

#### Scenario: 白名单外相位写模型被拒

- **WHEN** `implement/executor` / `code-review/evaluator` / `dev-design/evaluator` 会话对 `<archi_dir>/graph/model/*.toml` 调用 `write` 或 `edit`
- **THEN** 拒绝串含路径、工具名、指路（写进 design / tasks 交 `dev-design` 落地，或改用架构页会话）与「不要改用其他命令或工具尝试绕过」；run 不中断、文件零变化

#### Scenario: 主仓直改模型成功（架构页）

- **WHEN** agent 在架构页通用会话（`cwd = null` → 主仓根）内用 `write` / `edit` 改 `<archi_dir>/graph/model/*.toml`
- **THEN** 写入落在主仓该文件（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后同 workspace 的 `archi_module` 与画布取数读到该改动，改动保持未提交由用户 `git diff` 评审

#### Scenario: 写坏模型不阻断写盘

- **WHEN** agent 写出语法错的模型文件（白名单相位或通路 B）
- **THEN** 写盘成功；随后 `archi_module` / 画布数据返回结构诊断（`is_error` / 停摆态）且进程不 panic；收口时 `archi_preclose` 读数含该结构错

#### Scenario: 无专用写工具且两通路并存

- **WHEN** 审查执行面工具清单与写模型入口
- **THEN** 无「写模型」专用写工具（注入面可及的 archi 工具只有五个只读工具，且五者 `ToolExec::Archi` 零写盘）；写模型恰有两条通路（change worktree 白名单相位 / 架构页通用会话），MUST NOT 存在第三条（如独立写命令或画布写入口）

### Requirement: 架构会话的系统知识注入

`source = "archi"` 的会话 SHALL 注入一段架构会话知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`），内容 SHALL 覆盖以下关注点（**按 A/B/C/D 分层后**的清单，分层判据见「知识分层与工具面同批移动」）：

- **模型位置与聚合（B）**：模型文件位于架构域 `graph/model/` 下（契约资产在 `models/<分层目录…>/`，两目录分家）、按文件名字典序聚合；两处目录路径 SHALL 为**派生所得**的 repo 相对路径（见 `workspace-layout-resolution`），MUST NOT 在产品源码或知识文本中手写目录字面量。契约索引 `INDEX.toml` 的方位 MUST NOT 进知识文本（生成物，零手写面；需要时经 `archi_api` 的 `generatedPath` 取得）。
- **字段纪律（B + C）**：
  - `path`（B，一行 + 工具指路）：**模块必填（可多条 = 若干根的清单）；包至多一条包根（纯容器可留空；≥2 条即模型停摆）**、`domain` / `composite` 选填（缺省空）、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器填了即持文件，子元素依赖它会触发 `dependsOnAncestor`。「这个元素缺不缺 `path`」由 `archi_module` 的 `hasPath` 一次判出；容器空 `path` 的漂移读数按 kind 分档（`domain` / `composite` 零告警）见 `desktop-archi-drift`。
  - `entry`（B，一行 + 工具指路）：只在约定默认指不到时声明（单文件根 = 根文件自身 / 目录型根 = `<root>/index.ts(x)` / crate = `src/lib.rs`）；解析结果与不可用成因由 `archi_module` 的 `resolvedEntry` 给出，MUST NOT 在知识文本里复述三档长表。
  - `doc`（B，一行 + 判据指路）：单值、repo 相对，只有 `api = none` 的元素可写；`declared` 的入口文档恒为派生落点（按 FQN 段链去根与叶分目录、名为 `<FQN>.md`），写了即 `apiUnexpected`。判据长表归 `desktop-archi-api` / `desktop-archi-api-check`。
  - **字段形态与键形（C，散文必留）**：一元素一表头、键为带引号的完整 FQN 单键（未引号点分 `[element.App.Auth]` 使**整份源解析失败**——诊断点名非法子键 `Auth`，不是「读成另一个元素」；骨架 MUST NOT 写结构诊断词，见下「呈现源与对拍源分家」）；`metadata` 的**子表与内联表等价**（`[element."X".metadata]` 写在已声明元素之后零诊断；子表成为唯一声明则缺 `kind`、整份源不解析）；关系写 `[[relation]]`（`from` / `to` / `desc` / **`apis`**）。**字段序与各字段形态的完整模板不入散文**——用 `archi_scaffold` 取（D 档）。
  - **关系 `apis`（B，一行 + 判据指路，本变更新增）**：选填，**只写名字**——`to` 元素的代码导出名（禁签名 / 类型形状 / `::` 限定路径 / 空白），可留空即不声明 API 粒度，`apis = []` 与同名重复被拒；生成物面 MUST 留空（生成物是唯一真相，手写清单必然漂）。两桶结论（用了没声明 / 声明没用）由 `archi_drift` 的 `errors` / `warnings` / `observations` 给出，MUST NOT 在知识文本里复述判据长表，也 MUST NOT 让模型把它当协议正文写（正文仍在契约资产）。
  - **`kind` 四值语义（C）**：`package` = 包（最小编译单元 / 独立依赖管理；**至多一条包根 `path`，纯容器可留空**）/ `domain` = 领域（按业务实体边界聚合；`path` 选填）/ `module` = 目录型模块、视图或单文件组件（`path` 必填，可多条）/ `composite` = 无实际层级的组合（仅把过多的同级元素分类，可嵌套在任意类型之上；`path` 选填；**纯分层容器是它的标准形态**）。四值语义在 md 的「元素类型」段逐值给出（测试遍历 `ElementKind::ALL` 断言值 + 语义在场），不另设 kind 词汇行。
- **层级与缺省（B + C）**：层级由 FQN 最长前缀推导（子元素须先声明祖先——违反时结构诊断 `orphanHierarchy` 由 `archi_module` / `archi_drift` 的 `is_error` 与读数给出）；新元素 `gate` 缺省 `off`（ratchet 逐块收紧）。
- **计数复述单源（B，一行）**：模型文件表头注释里的元素 / 关系数量 SHALL 以工具读数（`archi_drift` 的 `model.elements` / `model.relations`）为准，改模型 SHALL 在同一批内更新表头；MUST NOT 让表头与模型本体各说各话。spec 的计数表与 ADR 的复算表属仓库文档面，知识文本 MUST NOT 复述任何具体数字。
- **架构域看护配置（B，一行）**：`<archi_dir>/config.json` 存在且管 `ignore`（元素对豁免）/ `exclude`（扫描排除），未知键与缺 `reason` 即配置非法——非法时 `archi_drift` 以 `is_error` 点名具体条目；MUST NOT 在知识文本里复述字段长表。
- **边界（C）**：模型里 MUST NOT 写接口 / 导出符号（契约权威在契约资产）；`api` / `entry` / `doc` 是三个**指针**（有无公共 API / 入口在哪 / 入口文档读哪份），MUST NOT 由它们扩张出协议正文；关系 `apis` 是**消费侧的依赖名字清单**（不是提供方的协议正文）——位置在关系、内容只有名字、权威仍在该元素的代码导出面。
- **自检纪律（A，指路）**：改完可**提前**自查——`archi_drift` 取覆盖 / 归属 / 契约读数（`errors` / `warnings` 通常为空，非空即逐条修）、`archi_module` 取结构诊断（带 `file:line`）、契约面 `archi_api`（`action = "check"`）查两轴；生成物过期用本仓既有的生成命令重生成，MUST NOT 手改生成物。**知识文本只给通用指路，MUST NOT 点名本仓特有的命令名**——产品对任意 workspace 生效，生成命令归属各自仓库的文档面。该纪律 SHALL 为纯提示词约束加工具指路；引擎在收口时另有一次兜底自检（见「收口前引擎自检（架构判据读数）」），故自检 SHALL NOT 被表述为「不调工具就不算完」的义务。
- **写面与可见性（C）**：直改主仓（无 worktree、无归档确认），改动随 `git diff` 评审；**禁止修改架构目录之外的文件**（源码 / 配置 / 脚本 / 域外文档一律不改，用户要求亦不越界；生成物重生成除外）；写盘后画布不自动更新，须提示用户点「刷新」。
- **历史会话续接（C）**：续接历史会话时 SHALL 先取当下模型现状（`archi_module` 无参概览或指定元素）再回答「现在是什么样」一类问题，MUST NOT 依赖会话早前的描述。

**呈现源与对拍源分家**：

- **`gate` / `api` 的值域与缺省档 SHALL 由静态骨架 md 的「字段纪律」段各一行承载**（可读性优先）：`gate`：off / warn / strict（缺省 off）；`api`：declared / none（缺省 declared）。测试 SHALL 遍历 `GateTier::ALL` / `ApiStatus::ALL` 与 `default()`，断言两行**同序相等**且缺省档在场；md 缺值、多值或改序即红。
- **`kind` 四值 SHALL 由 md 的「元素类型」段语义 bullet 承载**：每值一 bullet（值 + 语义）；测试遍历 `ElementKind::ALL` 断言值与语义在场，不另设 kind 词汇行。
- **结构诊断全量词表 MUST NOT 进知识文本**：工具按需返回注解与修复手段；散文只保留 `orphanHierarchy` / `dependsOnAncestor` 两个可执行词。知识文本因此**不是** `DiagnosticKind::ALL` 的词汇消费面——诊断词的三面对拍（进程内 / 工具响应 / IPC DTO）归 `desktop-archi-model`。
- **目录路径 SHALL 仍由代码派生**：程序追加段恰为两处——模型声明目录（`archi_graph_model_rel_dir`）与契约资产（`<archi_models_rel_dir>/<分层目录…>/<FQN>.md`）。骨架 MUST NOT 手写目录字面量；**契约索引方位句 SHALL 删除**（生成物，零手写面）。
- `entry` / `doc` / **`apis`** SHALL **不进**任何值域词表：三者的值域是任意 repo 相对路径 / 任意代码标识符（开放集），不是封闭词汇；md 只给纪律句 + 工具指路，判据定义归 `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-drift`，不在知识文本内复制判据长表。
- 知识文本 SHALL 为**静态骨架 md（`include_str!`）+ 两处目录路径段**在装配侧拼成的单条文本；新增 / 删除 `gate` / `api` 词汇而 md 行未同步时 md ↔ `ALL` 对拍红，`kind` 未同步时语义对拍红。

两条工程约束：

- **自证可解析**：字段形态的机械锚 SHALL 由 `archi_scaffold` 的草稿承担（`TomlParser` 零诊断，见「archi_scaffold 只读工具的新元素模板契约」）；知识文本本身 MUST NOT 再承载示例 TOML（示例是重复的第二事实源）。
- **尽力增强、不炸 run**：知识拼装失败（派生异常 / 读取失败）SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符（路径由派生注入），体量 SHALL 受上限约束（**骨架 ≤ 2700 B、全文 ≤ 2850 B，实测值冻结**；值域收敛为字段行后回落，防与 `AGENT.md` 合体后挤压上下文）。超限 SHALL 压缩同段冗余表述而不是抬高锚；收口自检的装配与本知识解耦（`before_done` 档位按来源置位），知识降级 MUST NOT 影响自检。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 字段值域由 md 承载并与 ALL 对拍

- **WHEN** 检查知识文本的 kind 语义段与 `gate` / `api` 字段行，并新增一个 `gate` 档位（同时补 `GateTier::ALL` 与 md 行）
- **THEN** md ↔ `ALL` 对拍断言 `gate` / `api` 行**同序相等**且缺省档在场；kind 由语义 bullet 对拍；md 少写 / 多写 / 改序 / 漏改缺省档均红；结构诊断全量词不得出现在知识文本（只允许 `orphanHierarchy` / `dependsOnAncestor` 两个可执行词）；`entry` / `doc` / `apis` 不出现在任何值域行（开放值集，零词表）

#### Scenario: 字段纪律走工具指路而非长表

- **WHEN** 审查知识文本的字段纪律段
- **THEN** `path` / `entry` / `gate` / `api` / `doc` 各为一行纪律 + 工具指路（`hasPath` / `resolvedEntry` / `apiUnexpected`）；零 `entry` 三档长表、零字段序清单、零示例 TOML 围栏；`kind` 四值语义、引号 FQN 单键、内联 `metadata`、层级先声明四条散文在场

#### Scenario: 关系键面与 apis 纪律在场

- **WHEN** 审查知识文本里关系相关的那一段
- **THEN** `[[relation]]` 的键面为四键（`from` / `to` / `desc` / `apis`）；`apis` 的纪律句含「只写名字」「可留空」「生成物面留空」三条关键词，且不含任何判据长表（两桶 / 档位 / 白名单长句零命中）；同样的句子里零示例 TOML 围栏（键名只作行内引述）

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 五 archi 工具不变），MUST NOT 抛装配错误；收口前自检照常装配（`before_done` 档位与知识解耦；T1 / T2 皆假时仍零 notice）

#### Scenario: 自检纪律与可见性在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完可调 `archi_drift` 取读数并按条修」「结构诊断经 `archi_module` 带 `file:line`」「契约面可调 `archi_api` 查两轴」「生成物过期用本仓的生成命令重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」六条；且知识文本内零 `<>` 占位符

#### Scenario: 体量与目录方位实测

- **WHEN** 取 `knowledge()` 输出与骨架字节
- **THEN** 骨架 ≤ 2700 B、全文 ≤ 2850 B 且实测值被冻结断言；md 含 kind 语义与 `gate` / `api` 值域及两处缺省档（结构诊断全量词表不得出现），程序追加段恰含模型声明目录与契约资产两处方位，**零契约索引串**

### Requirement: 检索面纪律与迁移边界

`archi_search` 与 `archi_module` 两个只读工具面 SHALL 守住三条迁移边界，使将来的「模块入口文档逐步替代 `openspec/specs/`」迁移不被本期实现反向锁死：

| # | 边界 | 理由 |
|---|---|---|
| 1 | 检索语料不读 `openspec/specs/` | 一旦纳入，用户会开始写「给 archi 看的 spec」，迁移被反向锁死 |
| 2 | 检索与工具响应不返回文档正文 | 正文按路径自取（agent 用 `grep` / `read`，前端用 `read_file`）；返回正文会让工具响应体随文档膨胀且与单结果字节上限冲突 |
| 3 | 不提供跨元素的文档反向索引 | 反查「这份文档描述哪些模块」不在本期，避免用户开始写「给 archi 看的文档」 |

- **语料面封闭**：文档检索语料 SHALL 恰为「`path` 非空元素的在盘入口文档载体」这一集合，MUST NOT 新增第四类来源（不出 `specs`、不出 `decisions`、不扫元素 `path` 目录内的散落 md、不扫 `<archi_dir>` 其它子树）。
- **正文不返回（结构性成立）**：`doc` 字段与 `ArchiSearchResult.doc` 都只承载**一条路径字符串**——字段里没有 `title` / 摘要 / 正文 / 行区间的容身之处，故「不返回正文」不靠纪律而由形状保证。
- **反向索引零预铺**：MUST NOT 引入「文档 → 元素集合」的索引、缓存或 IPC；`grep` 与文件树是反向查找的既有面。
- **零 `specs` 读取路径**：产品源码 MUST NOT 出现 `openspec` 路径拼接（路径一律经 `foundation::layout`），本能力 MUST NOT 引入 `spec ↔ element` 映射。
- **「模块入口文档替代 specs」的迁移**（含 acceptance 可断言结构与归档 `SpecSync` 增量合并语义的对应物缺失问题）需独立变更裁决，本期零预铺。

#### Scenario: 检索不命中 specs 内容

- **WHEN** `query = "archi"` 且 `openspec/specs/` 内某文件路径含该词、某入口文档亦含
- **THEN** 命中项只来自模型元素与他人入口文档路径；`openspec/specs/` 下任何路径与内容均不出现

#### Scenario: 正文零承载位

- **WHEN** 审查 `ArchiModuleDetail.doc` 与 `ArchiSearchResult.doc` 的字段面
- **THEN** 前者是单一路径串或 `null`、后者是路径串数组；两者都没有 `title` / 摘要 / 正文 / 行区间字段，也无「内容」一类可选位

#### Scenario: 关联来源封闭

- **WHEN** 审查文档检索语料来源清单
- **THEN** 语料只取「`path` 非空元素的在盘入口文档载体」一条规则（declared → 派生 `contractPath`、none → `doc`）；`decisions/**` 与元素 `path` 目录内的散落 md 都不在语料内，MUST NOT 出现第四条来源

### Requirement: 知识分层与工具面同批移动

架构会话知识文本的内容面 SHALL 按**可判定性 × 时机**两轴分四档，并把每一句的落档写进本能力（分层判据单点，MUST NOT 在别处另立一份分类）：

| 档 | 判据 | 承载 |
|---|---|---|
| **A 工具承载** | 机器能算 ∧ 动作后才需要 | 工具响应 / `Err`（MUST NOT 在知识文本里复述读数） |
| **B 上下文·一行指针** | 机器能算 ∧ 动作前就需要 | 知识文本**一行**，内容是「何时查哪个工具」或一个派生串 |
| **C 上下文·散文纪律** | 不可判定（语义 / 意图） | 知识文本散文 |
| **D 模板工具** | 不可判定 ∧ 动作后才需要（形态） | 模板工具输出（`archi_scaffold`） |

- **A 类句子 MUST 携带工具指路且工具面确有该字段**：知识文本里每条「来自 A 档」的纪律 SHALL 点名承载它的工具（`archi_module` / `archi_drift` / `archi_api` / `archi_scaffold`），且该工具的响应或 `Err` 面 SHALL 确有对应字段 / 判据。任一方向缺失（句子里点了不存在的字段、工具加了字段但句子没指路）即违约——由机械断言表守（一行为「纪律句关键词 → 工具名 → 工具面字段」）。
- **三条补充规则 SHALL 逐条在案**：
  1. **形态错误不静默，但发生在写盘之后**：未引号点分表头 `[element.App.Auth]` 归 `unknownField`、**整份源解析失败**（全或无）；`metadata` 子表与内联表**等价**、零诊断。两者都有**行为**测试钉住（`parse/toml_test.rs` 的 `未引号点分表头被拒且不与合法形态合并` / `metadata子表与内联表等价` / `metadata子表成为唯一声明即缺kind`）。归 C 档的理由仍成立但须换成**动作前不可判定**（写面是自由文本 `write` / `edit`，结构诊断只在写盘后可得；四档表的「不可判定」即此义），MUST NOT 再表述为「会被静默读成另一个元素」或「`deny_unknown_fields` 不触发」——后者已被行为测试否证，不是任何档位的依据。
  2. **写入定位必须留一行**：新元素的模型落点（模型目录）与手写契约的落点（契约资产目录）都在动作前需要，且新元素的契约三路径为 `null`（`api.contractPath` 指不到不存在的东西）⇒ 归 B 档，各留一行。
  3. **`kind` 语义不可脚本化**：「容器 = `domain` / crate、目录与单文件组件 = `module` / 包根 = `package`」是架构判断，工具只能事后判「和邻居不像」⇒ 归 C 档，散文必留。
- **移动分层的原子性**：把某句从散文改判为 A/D 档时，**同批**必须落地「工具面确有该字段 / 模板」与「散文句替换为指路句」两半——只删不加（知识缺失）与只加不删（知识重复）同样违约。
- **本变更的移动清单**（机械断言逐项在案）：示例 TOML → `archi_scaffold`；字段语法形态 → `archi_scaffold` + `archi_module` 字段清单；`entry` 三条纪律 → `archi_module.resolvedEntry`；`doc` 仅 `api = none` 可写 → 判据面 `apiUnexpected` + 一行指路；祖先先声明 → 结构诊断 `orphanHierarchy` + 一行指路；`path` 缺省判定 → `archi_module.hasPath` + 一行指路。

#### Scenario: A 类句子逐句有工具可指

- **WHEN** 对知识文本的 A 档纪律句逐条取「关键词 → 工具名 → 工具面字段」三列核对
- **THEN** 每条纪律句都点到承载它的工具名，且该工具的响应 / `Err` 确有对应字段（`hasPath` / `resolvedEntry` / 结构诊断 `kind` / 判据 `kind`）；任一列为空即测试红

#### Scenario: 形态纪律与 kind 语义留在散文

- **WHEN** 审查知识文本的 C 档
- **THEN** 带引号 FQN 单键纪律（成因写「未引号点分 → 整份源解析失败、诊断点名非法子键」）、`metadata` 子表与内联表等价的陈述、`kind` 四值语义逐值在场，且骨架零结构诊断词；MUST NOT 出现「子表 / 点分嵌套会被读成另一个元素」的假因果，也 MUST NOT 出现「`metadata` 必须内联表」的禁令

#### Scenario: 只删不加即违约

- **WHEN** 从知识文本删掉示例 TOML 而未同时落地 `archi_scaffold`（或反之：加了模板工具但散文仍抄一份示例）
- **THEN** 视为分层未完成——机械断言表对「移动清单」逐项要求两侧同时在场（模板工具 + 指路句），单侧缺失即红

### Requirement: 收口前引擎自检（架构判据读数）

archi 会话（装配层在 `SessionInjections.before_done` 置位 `ArchiPreclose` 档位）在**自然收敛**收口前 SHALL 由引擎执行一次架构判据自检，结果作为一条 `SystemNotice` 落转录并写引擎日志——**不回灌模型、不催轮次、不发新请求**：

- **唯一挂点**：只在 loop 的「无 tool_calls 且非 `pause_turn`」自然收敛支执行一次；空 choice / 输出截断 / 异常终局 / `stop_requested`（轮间 / 轮头 / 续跑间隙）/ `pause_turn` 续跑到顶五个出口 MUST NOT 执行（模型没给终答时读数无意义，停止是逃逸阀不能被拖慢）。
- **装配判据**：自检的装配 SHALL 由 `SessionInjections.before_done = Some(ArchiPreclose)` 档位表达，唯一置位点是 `source = "archi"` 臂（装配单点在 `compose::assemble_injections`）；MUST NOT 以 `preamble` 在场、`turn.archi.is_some()` 或工具名清单等旁证推断（config 臂同样生产 preamble；change 相位同样下发 archi 工具，皆会误命中）。知识降级（`preamble = None`）MUST NOT 影响自检装配——两者解耦。
- **触发两条件（析取）**：
  - **T1** 本轮写过架构域：`write` / `edit` 的 sandboxed `path`、`bash` 经 `protect::written_paths` 得到的写目标（复用同一提取面，MUST NOT 第二份写面解析）命中 `<archi_dir>/graph/**` 或 `<archi_dir>/models/**`（前缀经 `foundation::layout` 派生，MUST NOT 行内字面量）。
  - **T2** 资产变化：本轮起点与收口时的 `archi::drift::assets_hash(root)` 不相等。
  - 两条件皆假 ⇒ 只在**引擎日志**留一行 skip，MUST NOT 发 notice（纯问答不产生转录噪声）。
- **执行面**：判据 SHALL 为 `archi::drift::check(CheckInput { root, mode: Full, changed: &[], change: None, cancel })`（与命令面全量分支同源，零第二套判定）；SHALL 以 `tokio::task::spawn_blocking` 执行；取消标志 SHALL 为 `CancelFlag::never()`（MUST NOT 为几十毫秒扩 `RunHandle` 的公开面）。
- **载荷**：`SystemNotice{subtype: "archi_preclose", payload}`，载荷 = `drift_summary` 投影（与 `archi_drift` 工具 `summary` 共用单点）+ `changedAssets: bool`；整份 `Report` MUST NOT 入转录（一次几 KB 反而烧 token）。
- **事件序**：notice SHALL **先于** `finish_success`（即 `TurnDone`）流出（见 `desktop-agent-execution`「自然收敛前的收口前钩子与收敛事件序」）。
- **fail-open 三面**：任何 `CheckError` ⇒ `SystemNotice{subtype:"archi_preclose_skipped", payload:{reason}}` + 引擎日志，**照常收口**；MUST NOT 阻断收敛、MUST NOT 改写 `TurnDone` 的 `subtype` / `is_error`、MUST NOT 把成因回灌模型。
- **知识文本随动**：自检纪律 SHALL 由「模型义务」降为「可提前自查」（引擎已兜底），MUST NOT 出现「不调自检工具就不算完」一类义务句。
- **零缓存**：每次收口现算（沿 `archi_search` 纪律），MUST NOT 跨轮缓存报告或哈希以外的中间态。

#### Scenario: 写模型的会话收口带读数

- **WHEN** archi 页面会话中用 `write` 改 `<archi_dir>/graph/model/*.toml`，随后模型给出无工具调用的收口终答
- **THEN** 事件流在该轮 `TurnDone` **之前**含一条 `SystemNotice{subtype:"archi_preclose"}`，载荷含 `status` / `passed` / `counts` / `modelHash` / `changedAssets: true` / `durationMs`；请求数与本变更前相同（零额外请求）；对话历史无新增消息（不回灌）

#### Scenario: 纯问答零 notice

- **WHEN** archi 页面会话整轮零写盘、资产哈希前后相等，模型自然收敛
- **THEN** 事件流零 `archi_preclose` notice，引擎日志含一行 skip 记录（T1 / T2 皆假）；`TurnDone` 与既有语义逐字一致

#### Scenario: 判据失败不改收敛

- **WHEN** 收口时架构域看护配置非法（`CheckError::Config`）或模型结构坏（`CheckError::Scan`）
- **THEN** 流出 `SystemNotice{subtype:"archi_preclose_skipped"}`（载荷含成因），`TurnDone{subtype:"success", is_error:false}` 照常；run 不中断、模型未被催问

#### Scenario: 非 archi 会话零射程

- **WHEN** 以 `source = "change"`（`implement/executor`，工具面含 archi 工具）与 `source = "debug"` 发起会话并自然收敛
- **THEN** 两者零 `archi_preclose` / `archi_preclose_skipped` notice（未装配 `before_done` 档位即不跑自检）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/runner.rs` | 注入词表 | `SessionInjections` 六位（`preamble` / `tools` / `finish` / `ask` / `protect` / `before_done`）；`before_done: Option<BeforeDoneSpec>` 为收口前钩子档位（serde additive，`None` = 不装配；`BeforeDoneSpec::ArchiPreclose` 由 archi 臂唯一置位）；`SessionToolset` / `toolset` 已退役（产品源码零命中，`sdk/runner_test.rs` 有零命中断言），本表不再登记该枚举 |
| `crates/infra/agent/src/compose.rs` | 注入装配 | `assemble_injections`：explore 臂逐字不变 → **archi 臂**（`source == "archi"` 短路命中：`preamble = Some(knowledge())`、`tools = Some(七内建全量 + 五个 archi 工具名)`、`before_done = Some(ArchiPreclose)`，不碰相位解析）→ change 臂（7 行矩阵 + fail-closed，`archi_matrix` 为矩阵单点）→ default（显式构造，非默认值兜底）；`ARCHI_SOURCE` 常量为来源词单点 |
| `crates/infra/agent/src/archi_knowledge.rs` | 骨架 + 派生段拼装（本变更：目录方位串换段） | 注入段 SHALL 由「契约目录：`<archi_api_rel_dir>`」改为「契约资产：`<archi_models_rel_dir>/<分层目录…>/<FQN>.md`（索引 `<archi_graph_rel_dir>/INDEX.toml`）」——`api` 段串零残留；四面词表注入、纯函数零 IO、降级为 `None` 的语义逐字不变。原拼接面：`knowledge() -> Option<String>`：`include_str!("archi-knowledge.md")` + `ElementKind::ALL` / `GateTier::ALL` / `GateTier::default()` / `ApiStatus::ALL` / `DiagnosticKind::ALL` / `foundation::layout` 相对路径派生族；`entry` / `doc` MUST NOT 进派生词表（开放值集）；纯函数零 IO；降级为 `None` 而非 `Err` |
| `crates/infra/agent/src/archi-knowledge.md`（新） | 知识静态骨架（`include_str!` 内嵌） | 格式与键形约定（「格式约定」段的元素字段面为八项：`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`，并含 `entry` 三条纪律与 `doc` 的「只有 `api = none` 可写」纪律）/ 层级推导 / `gate` 缺省 / 边界 / 自检 / 写面与可见性 / 历史会话续接 + 两段形态的可解析示例 TOML（`api = "none"` + `doc` / declared 零 `doc` 行；经 `TomlParser` 零诊断）；零 `<>` 占位符、体量受上限约束、零 `include_str!` 之外的动态指令 |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 知识语料（本变更：断言串换段） | 「契约目录：」引导语断言改为「契约资产：」+「契约索引：」两条并各断言 `<archi_models_rel_dir>` / `<archi_graph_rel_dir>` 逐字注入；示例片段可解析（含 `entry` 单值形态）、四面词表在场、体量上限、纯函数性（三次调用逐字相等）四条语义不变 |
| `crates/core/archi/src/dto.rs` | 词汇单点（补 `ALL`） | `ArchiDiagnosticKind::ALL: [Self; 9]`，与 `as_str` 同源；既有 `ElementKind::ALL` / `GateTier::ALL` 复用 |
| `crates/core/archi/src/dto.rs`（`ArchiApiRef` / `ArchiElement.api` / `ArchiModuleDetail.api`） | 契约状态的线面投影 | `api: { status: ApiStatus, contractPath: Option<String>, schemaPath: Option<String>, generatedPath: Option<String> }`；`specta::Type`；三路径由 `api::paths` 派生后转 repo 相对形态（契约 md / schema 在 `<archi_models_dir>/<分层目录…>/`）；DTO 形状（字段名 / 顺序 / 派生）零变化。**绑定面说准**：两处 doc 方位串经 specta 抄进 `bindings.ts` ⇒ 恰 2 行 doc 注释随动、`bindings.zod.ts` 零 diff |
| `crates/core/foundation/src/layout/mod.rs` | 相对路径派生 | `archi_rel_dir()` / `archi_models_rel_dir()` / `archi_graph_rel_dir()`（`/` 分隔、零 IO、零行内字面量）；`api` 段一族（`API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir`）与 `decisions` 段一族（`DECISIONS_DIR_NAME` / `archi_decisions_dir` / `archi_decisions_rel_dir`）已删 |
| `crates/infra/agent/src/sdk/tools.rs` | 工具定义面 | 三个工具名常量 + 定义（schema：`element`；`query` + `scope`；`action` + `element`）；`definitions` 按本轮工具名字清单追加；`input_path` 对三者返回 `None`（不进路径沙箱） |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具执行体 | 经 `LoopTurn` per-turn 上下文取 root → `archi::dto` 投影（`archi_module` 两形态、`archi_search` 三 scope）与契约面自检（`archi_api` 的 `check` / `bootstrap` 两动作，零写盘）；命中既有 L1 单结果上限收口；坏模型 → `is_error` 不 panic |
| `crates/infra/agent/src/sdk/tools/archi_module.rs`（消费点） | 工具响应装配 | 只调 `archi::dto` 投影并序列化；零路径拼接、零契约读取、零协议正文；只读属性与注入矩阵零变化 |
| `crates/infra/agent/src/sdk/loop.rs` | 执行臂路由 + 系统消息消费 | 三个 archi 名与 `EXPLORE_NOTE_TOOL` 同式路由至执行体；上下文缺席以 `is_error` 回灌；知识 preamble 与 `AGENT.md` 合并为单条 System（由 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」承载） |
| `crates/infra/agent/src/sdk/policy.rs` | 静态权限表 | `READONLY_TOOLS` += `archi_module` / `archi_search` / `archi_api`（三档全放行）；准入判据单点在挂载清单的准入档（`ToolTier::Read`）；写面 / 执行面清单零变化 |
| `crates/infra/agent/Cargo.toml` | 依赖边 | `archi = { workspace = true }`（工具执行体复用同一投影，避免第二份 JSON）；用途注释在案 |
| `crates/core/workflow/src/prompts/*.md`（矩阵命中 7 份） | 角色知识 | 只补工具名与用法一句（何时用定位 / 何时用发现）；零占位符、零 MCP、其余 9 份零改动（见 `desktop-phase-prompts`） |

### Component: 仓库根文档

| 项 | 变化 | 描述 |
|---|---|---|
| `AGENT.md` / `CLAUDE.md` | 仅第②条措辞同步（条目增删与节结构冻结） | `## 模块 API 契约` 节的冻结面为**条目增删与节结构**：条目数恒 5，本变更 MUST NOT 增删要点、MUST NOT 加 `entry` 第六条（字段知识走知识文本与 `desktop-archi-model`），两份副本该节正文仍逐字相同；第②条的方位词与文件名形态（「实现入口同级」/ `<入口主干>.i.*`）属 `desktop-archi-api`「仓库根文档的契约纪律节」的实现面，本变更不写该节正文 |

### Function: `crates/infra/agent/src/sdk/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `mount.rs` | 改 | 静态清单 10 → 11 项：`ARCHI_API_TOOL = "archi_api"`（`ToolTier::Read`、`ToolExec::Archi`）；`ToolTier::Read` 注释与 `ToolExec::Archi` 臂名清单同步 |
| `archi_tool.rs` | 改 | `ArchiContext::execute` 加 `archi_api` 一臂：`check`（`graph_view` → `load_contracts` → `api::check` + `api::compare`，按 `category` 计数摘要）/ `bootstrap`（`api::bootstrap`）；输出过 `tools::cap_result`；零写盘 |
| `runner.rs` | 改 | `mint_archi_context` 的 present 判据加 `archi_api` |
| `archi-knowledge.md` | 改 | 自检纪律 bullet 补 `archi_api` 两轴查询与「生成物用既有命令重生成、MUST NOT 手改」 |

### Function: `crates/infra/agent/src/compose.rs`（矩阵与来源臂）

| 项 | 变化 | 描述 |
|---|---|---|
| 来源臂与矩阵 | 改 | `archi_matrix` 的 `(implement, executor)` 臂追加 `mount::ARCHI_API_TOOL`；archi 来源臂名字清单追加同名；其余相位与臂零变化；`core/agent` 零改动（注入面本就是工具名字清单，无枚举增字段） |

### CLI: `crates/core/workflow/src/prompts/implementation-generator.md`

| 项 | 变化 | 描述 |
|---|---|---|
| 只读工具行 | 改 | 补 `archi_api`（改完代码自查生成物新鲜度与契约判据）；其余相位 prompt 零改动（见 `desktop-phase-prompts`） |

### 模块增量：explore 臂 archi 面口径对齐与成员校验覆盖（agent-db-query-tools）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `openspec/specs/desktop-archi-agent`（表述对齐） | explore 臂口径 | 仅改「explore 臂逐字不变」的两处表述（矩阵 bullet 与 `explore 与 debug 来源零 archi 工具` scenario）：archi 面恒空不变，完整清单单源指向 desktop-agent-execution / desktop-agent-db-tools |
| `crates/infra/agent/src/compose.rs`（零 archi 语义改动） | 矩阵与两臂 | `archi_matrix` 7 行逐字不变；archi 臂三工具 + 知识 preamble 逐字不变；change 臂矩阵语义不变（DB 工具两名的恒 push 属 desktop-agent-execution 的装配面，不参与矩阵判定） |
| `crates/infra/agent/src/sdk/mount.rs`（零 archi 语义改动） | 档位与成员校验 | 三 archi 工具仍 `ToolTier::Read`；成员校验对新增两 DB 工具名同样生效（同一条判据，无第二份名单） |

### 模块增量：入口文档单值换面（module-doc-field-docs-retirement）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/agent/src/sdk/mount.rs` | 工具定义面 | `archi_module` 描述去「四字段文档条目」与 `docNotFound` 警告句、加「`doc` 仅 `api = none` 有值」；`archi_search` 的 `scope` enum → `["elements", "doc", "all"]`、描述内「四字段文档条目」→「入口文档路径」；工具名常量与三个定义的参数形状（`element` / `query`+`scope` / `action`+`element`）零变化 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具执行体 | 经 `LoopTurn` per-turn 上下文取 root → `archi::dto` 投影（`archi_module` 两形态含 `doc` 单值 / `archi_search` 三 scope 含 `doc` 词）；`archi_api` 的 `check` / `bootstrap` 零改动；命中既有单结果上限收口；坏模型 → `is_error` 不 panic |
| `crates/infra/agent/src/archi_knowledge.rs` | 骨架 + 派生段拼装 | 派生段删「决策记录目录：{}」一句（`archi_decisions_rel_dir` 已删）；保留模型声明目录 / 契约资产 / 契约索引三处方位；四面词表注入与降级为 `None` 的语义逐字不变 |
| `crates/infra/agent/src/archi-knowledge.md` | 知识静态骨架 | 「格式约定」字段面八项含 `doc`（`docs` 零命中）；`doc` 的两条纪律（单值 / 只有 `api = none` 可写）；示例两段形态（`api = "none"` + `doc` / declared 零 `doc` 行）；「写面与可见性」段把「只改模型与 `docs` 关联文档」改为「只改模型与 `doc` 所指入口文档」；零 `<>` 占位符、体量受上限约束 |
| `crates/core/archi/src/dto.rs` | 投影单点 | `module_detail` 的 `doc` 与 `search` 的 `doc: Vec<String>` 由本 crate 单点产出（工具执行体零拼接、零二次读模型） |
| `crates/infra/agent/src/sdk/policy.rs` | 静态权限表 | `READONLY_TOOLS` 三工具名与三档全放行逐字不变（工具名集合零变化 ⇒ 注入矩阵零变化） |

### 模块增量：知识分层、工具面与相位锁同步（archi-rule-agent-prompt-sync）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/dto.rs` | 投影单点（本变更加两字段） | `ArchiElementBrief.has_path: bool`；新增 `ArchiResolvedEntry { path, via, problem }` + `ArchiModuleDetail.resolved_entry`；`module_overview` 填 `hasPath`、`module_detail` 填 `resolvedEntry`（经 `api::paths::entry_state`，`via` 由 `element.entry.is_some()` 派生）；DTO 其余字段与派生零改动 |
| `crates/core/archi/src/drift/mod.rs` | 资产哈希 | `pub fn assets_hash(root: &Path) -> String`：覆盖面 = `graph/model/*.toml` + `models/**/*.md` + `*.schema.json` + `graph/INDEX.toml`；稳定序「相对路径 + 字节」累加、`sha256:` 前缀；`model_hash` 与报告 `modelHash` 语义零改动 |
| `crates/infra/agent/src/sdk/mount.rs` | 定义面 | 常量 `ARCHI_DRIFT_TOOL` / `ARCHI_SCAFFOLD_TOOL`；`archi_drift_definition()`（三 scope enum + gate 全 off 常态句）、`archi_scaffold_definition()`（`kind` enum ← `ElementKind::ALL`）；`build_registry` 13 → 15，五 archi 项均 `ToolTier::Read` / `ToolExec::Archi` |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 执行体 | `drift_payload`（`scope` 解析 → `Instant` 计时 → `drift::check(Full, &[], None, CancelFlag::never())` → 投影 → `cap_result`）；`scaffold_payload`（`kind` 解析 ← `ElementKind::ALL` → 模板）；`Err` 文案逐值点名；坏模型 `is_error` 不 panic |
| `crates/infra/agent/src/sdk/archi_drift_view.rs`（新） | 投影单点 | `drift_summary(&Report, duration_ms) -> Value` / `drift_entries` / `drift_files`；收口 notice 与工具响应共用（零第二份装配） |
| `crates/infra/agent/src/sdk/preclose.rs`（新） | 收口自检执行体 | `touched(name, input, cwd) -> bool`（写入面命中 `<archi_dir>/graph/**` 或 `models/**`，前缀经 `foundation::layout`）；`run(&sender, turn, touched)`（`assets_hash` 比对 → `spawn_blocking` → notice / 日志，fail-open） |
| `crates/infra/agent/src/archi_knowledge.rs` | 拼装（派生段收缩） | 派生段 = 四面词表 + 模型目录 + 契约资产（**删契约索引**）；降级为 `None` 语义逐字不变；体量锚由测试冻结 |
| `crates/infra/agent/src/archi-knowledge.md` | 知识骨架（分层重写） | 删示例 TOML / 字段序清单 / `entry` 三档 / `doc` 判据细节；新增 `kind` 四值语义、计数单源、看护配置三块；`path` / `entry` / `doc` 三行纪律 + 工具指路；自检纪律降为「可提前自查」 |
| `crates/infra/agent/src/compose.rs` | 装配单点 | archi 臂名字清单加两名；`archi_matrix` 8 行（`dev-design` 执行 / 评估分行，新增 `drift` / `scaffold` 两维）；`model_write_lock(provenance)` 单点（会话级锁的置位规则，见 `desktop-write-protection`） |
| `crates/infra/agent/src/sdk/loop.rs` | 挂点 | 自然收敛支调用已装配的 `before_done` 钩子（先于 `finish_success`）；T1 观测点在工具执行前（沙箱后入参在手时判定，零额外 IO）；其余四出口零调用 |
| `crates/infra/agent/src/sdk/preclose.rs` | 钩子实现 | `BeforeDoneHook` 三面（`start` 起点哈希 / `observe_tool` T1 观测 / `before_done` 判定 + notice）；`spawn_blocking`；`CancelFlag::never()`；fail-open → `archi_preclose_skipped` |
| `crates/infra/agent/src/sdk/runner.rs` | 铸造与装配 | `mint_archi_context` 的 any 判据加两名；`ProtectRules::load(...).with_session(...)` |
| `crates/core/workflow/src/prompts/*.md`（矩阵命中 7 份） | 角色知识 | 按新矩阵补 `archi_module` / `archi_drift` / `archi_scaffold` 的「何时用 / 返回什么」一句；两处写权相位另加写面与自查句（见 `desktop-phase-prompts`） |

### API: `crates/infra/agent/src/sdk/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `mount::registry` | 改 | 13 → 15 项；`ToolTier::Read` 注释与 `ToolExec::Archi` 臂名清单同步 |
| `archi_tool::ArchiContext::execute` | 改 | 加 `archi_drift` / `archi_scaffold` 两支；两支均零写盘、过 `cap_result` |
| `preclose` | 新增 | 唯一调用点 = `loop.rs` 自然收敛支；`CancelFlag::never()`；`CheckError` 不外抛 |

### CLI: `crates/core/workflow/src/prompts/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `proposal-planner.md` | 改 | 工具行补 `archi_module` / `archi_scaffold`；写面句（只为已确定元素落笔 + 写完自查） |
| `dev-design-planner.md` | 改 | 工具行补 `archi_drift` / `archi_scaffold`；写面自查句 |
| `implementation-generator.md` | 改 | 工具行补 `archi_drift`（改代码后自查未声明依赖） |
| 其余 11 份 | 改 | 各补「架构模型已锁」一句（中文措辞、零 ASCII `archi`、evaluator 零 `backtrack`） |

### 模块增量：config 来源臂的 archi 面归零（config-agent-session）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 类型 | 职责 / 契约 |
|------|------|-------------|
| `crates/infra/agent/src/compose.rs` | Function | archi 来源臂与矩阵逐字不动；新增的 `source == "config"` 臂**零 archi 工具**（其清单单源在 `desktop-agent-execution` 与 `desktop-config-agent`）；兜底臂表述收窄为「非 change、非 config 来源（debug 等）」 |
| `crates/infra/agent/src/archi_knowledge.rs` | Function | 零改动（archi 臂专属知识）；`preamble` 位新增的第二生产者（config）**MUST NOT** 复用本文件 |

### 模块增量：关系键面与 apis 纪律（relation-api-dependency-check）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/archi-knowledge.md` | 静态骨架（`include_str!`） | 「位置与键形」段的关系键面由三键改四键（`from` / `to` / `desc` / `apis`）；「字段纪律」段新增一行 `apis` 纪律（只写名字 / 可留空 / 生成物面留空 / 结论看 `archi_drift`）；零示例围栏、零判据长表、零 `<>` 占位符 |
| `crates/infra/agent/src/archi_knowledge.rs` | 派生段装配（零改动） | 四面词表与两处目录派生逐字不变；`apis` 不进派生的任何词表（与 `doc` / `entry` 同列） |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 知识面测试 | 新增锚：关系四键在场、`apis` 三关键词在场、`apis` 不出现在派生词表行、骨架字节仍 ≤ 2.2 KB；既有锚（零围栏 / 零字段形态教学 / 指路表 / 体量）逐字保留 |
| `crates/infra/agent/src/sdk/mount.rs` / `compose.rs` | 工具面（零改动） | 工具清单与 `(phase, role)` 矩阵不变（`archi_relation` 只走 IPC 命令面，不进 agent 工具面） |

### Function: `crates/infra/agent/src/archi_knowledge.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `knowledge()` | 零改动（内容随骨架） | 骨架 + 派生段拼装；失败降级 `None`；体量锚（骨架 ≤ 2.2 KB / 全文 ≤ 2.8 KB）仍由测试断言 |
| 派生词表 | 零改动 | `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / `DiagnosticKind::ALL` 与两处目录派生逐字不变；`apis` 不进词表 |

### 模块增量：kind × path 判据与静默陷阱清除（archi-model-path-and-silent-trap）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/archi_tool.rs` | 模板单点（本变更改三行注释） | `SCAFFOLD_TEMPLATE`：`kind` 行四值齐列（`package` 至多一条包根 / `domain` 领域容器 / `module` 模块 / `composite` 纯分组）、`path` 行按新基数（模块必填可多条 / 包至多一条包根 / `domain`·`composite` 选填）、`metadata` 行写「内联表与子表等价」；`scaffold_draft` 的 `__KIND__` 替换与 `ElementKind::ALL` 派生不变；草稿对四值仍零结构诊断 |
| `crates/infra/agent/src/archi-knowledge.md` | 知识骨架（三处改写 + 体量锚） | ①「位置与键形」键形句：未引号点分 → 整份源解析失败（诊断点名非法子键；骨架不写诊断词）；`metadata` 子表与内联表等价；② 字段纪律 `path` 首条：模块必填（可多条）/ 包至多一条包根（纯容器可空）/ `domain`·`composite` 选填；③ 元素类型 `composite` bullet 补「纯分层容器是标准用法」；骨架 2459 B / 全文 2602 B 两个冻结锚同批实测更新 |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 知识断言面 | `骨架静默陷阱四条在场` 改名（去「静默」）并把 `内联表` needle 换为 `子表与内联表等价`；四条 forbidden needle（`assert!(!source.contains(…))`）与说明注释**保留**——它们是「骨架零假因果」的防线，不属待清残留；A 档指路表、体量锚与降级用例的其余断言不变 |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | 模板断言面 | `archi_scaffold三kind逐值` 改遍历 `ElementKind::ALL`；`叶元素必填` needle 改 `module 必填`；新增/保留「kind 四值语义在场」断言 |

### Function: `crates/infra/agent/src/sdk/archi_tool.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `scaffold_draft(kind)` | 零改动 | 仍为 `SCAFFOLD_TEMPLATE.replace("__KIND__", kind.as_str())`；`kind` 值集仍由 `ElementKind::ALL` 派生 |
| 模板文本 | 改 | 三行注释（`kind` / `path` / `metadata`）；无新增字段行、无新占位符 |

### API: 工具面（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_scaffold` 参数 / 响应 | 零改动 | `kind` 可选四值（`ElementKind::ALL` 派生）；`{ draft }` 形状、`is_error` 回灌面、`cap_result` 截断单点不变 |
| 五 archi 工具注入矩阵 | 零改动 | `(phase, role)` 矩阵、`ToolTier::Read`、挂载清单与成员校验不变 |

### CLI / Component: 架构会话知识

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_knowledge::knowledge()` 装配 | 零改动 | 骨架 + 两处派生目录段的拼装、失败降级 `None` 与 `SessionInjections.preamble` 合并路径不变 |
| 前端组件 | 零改动 | 无 IPC / DTO / UI 变化 |

### 模块增量：契约起草引导与描述补位（archi-contract-authoring-guidance）

主基线既有行逐字沿用；本变更只补工具**描述面**与一处草稿场景口径（`action` 值集、响应形状、`cap_result` 截断单点、只读档位、注入矩阵与零写盘纪律逐字不变），受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/mount.rs` | 工具定义面（本变更改两个描述函数） | `archi_api_definition()` 描述补三句：① `bootstrap` 指路（新建 / 补契约第一步取草稿）；② 四类生成物点名（同目录 `.schema.json` / 架构域 `INDEX.toml` / 入口同级 `.i.rs` · `.i.ts`，保留「MUST NOT 手改生成物」原句）；③ 冷启动口径（`artifacts.enabled = false` ⇒ 契约面未启用，`findings` 恒空不等于零问题）。`archi_module_definition()` 描述补第四句：`api.schemaPath` / `api.generatedPath` 给另两条生成物路径。两处参数 schema、工具名常量、`ToolTier::Read` / `ToolExec::Archi` 归属与 `build_registry` 项数零变化 |
| `crates/infra/agent/src/sdk/mount_test.rs` | 描述断言面 | `archi_api定义描述与参数形状` 的 needle 扩到 `第一步` / `.schema.json` / `INDEX.toml` / `.i.` / `enabled` / `findings`（`bootstrap` 一词既有描述已有，不作新针；既有 `MUST NOT` / `手改生成物` / `重生成` 三条仍须绿）；`archi_module描述补三句指路` 的 needle 扩到 `schemaPath` / `generatedPath`；两条既有边界（`工具描述零项目专用命令`、`archi_module描述零字段形态教学` 的零代码围栏）保持绿 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具执行体（零改动） | `check` 仍回 `{ artifacts, findings: { total, byCategory, items } }`，`bootstrap` 仍回 `{ draft }`；`artifacts.enabled` 的出处（`api::artifact_check`）与判据语义不变——本变更只把「怎么读这个字段」写进描述 |
| `crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` | 注入知识（零改动） | 骨架与派生段逐字不变（2596 B / 2739 B 冻结锚）；四条描述补位 MUST NOT 进 preamble |
| `crates/core/workflow/src/prompts/*.md` | 相位 prompt（零改动） | 14 份（6 executor + 8 evaluator）逐字不变（含 `implementation-generator.md` 的 `archi_api` 一行）——描述面已足够，不进 prompt |

### Function: `crates/infra/agent/src/sdk/mount.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api_definition()` | 改（描述文本） | 在既有描述内加三句（指路 / 生成物点名 / 冷启动口径）；`parameters` 的 `action` 二值 enum 与 `element` 字段零变化 |
| `archi_module_definition()` | 改（描述文本） | 加一句（两条生成物路径）；其余句子与零代码围栏纪律逐字保留 |
| `lookup` / `build_registry` | 零改动 | 成员校验、档位与挂载清单项数不变 |

### API: `archi_api` / `archi_module`

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api` 响应形状 | 零改动 | `check` 两轴并列内容、`bootstrap` 的 `{ draft }`、`is_error` 回灌与 `cap_result` 截断单点逐字不变 |
| `archi_module` 响应形状 | 零改动 | `api` 三路径 / `doc` / `resolvedEntry` / `hasPath` 的投影与 DTO 零变化——本变更只让描述说明这些字段怎么用 |
| 注入矩阵 | 零改动 | `archi_api` 仍只落 `(implement, executor)` 与 `source = "archi"` 两处；不在本变更扩面 |
