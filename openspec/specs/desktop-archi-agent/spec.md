# desktop-archi-agent Specification

## Purpose

定义 agent 侧的架构工具面：`archi_module`（定位，一次给全三份资产）/ `archi_search`（发现）/ `archi_api`（接口面自检、双草稿与引用计算）/ `archi_scaffold`（字段词典）/ `archi_drift`（漂移读数）五个专用只读工具与 `archi_element` / `archi_relation` 两个专用写工具的契约、以**工具名字清单**表达的 `(phase, role)` 注入矩阵与 `source = "archi"` 来源臂（全工具面 + 系统知识 preamble）、`archi` 工具档位归属与成员校验，以及「模型 = 源码」的两条写入口与写纪律（写工具 canonical + 手改 second path）。已退役的枚举式 toolset（`SessionToolset`）不在本能力描述范围内。

## Requirements

### Requirement: archi_module 只读工具的定位契约

SDK 引擎 SHALL 提供专用只读工具 `archi_module`（不在七通用工具 `TOOL_NAMES` 封闭集内，经工具名字清单受控注入），语义为**定位**（已知在看哪个模块），并 SHALL 一次给全该模块的**三份资产**：

```jsonc
{ "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" } }
```

- 指定 `element` 时响应 SHALL 含：`element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine` / `entry`）、`dependsOn`（`target` + `description`）、`dependedBy`（`source` + `description`）、`children`（直接子元素 FQN）、`doc`（模型 `doc` 单值，仅 `api = none` 元素可能有值）、`api`（并列契约状态字段）、`resolvedEntry`、`fileHash`、**`assets`（三份资产的结构面）**、**`reading`（一句话读法指引）**。`docs` 数组与 `warnings` 数组两个位次已退役。
- **`assets` 的三段 SHALL 各自独立 `present`，MUST NOT 全或全无**：

  ```jsonc
  "assets": {
    "topology":  { "file": "<模型声明文件>", "line": 105, "out": 0, "in": 3 },
    "interface": {                                       // 唯一凭据
      "path": "<models>/…/<叶段>.schema.json", "present": true, "bytes": 276,
      "ops": [], "types": [], "reexports": [],            // 键名即定义
      "abstract": "接口面留白（schema 在盘且无 op / 无类型）" },
    "doc": {
      "path": "<models>/…/<叶段>.md", "present": true, "bytes": 3680, "lines": 62,
      "headings": ["职能", "公共面", "不变量", "承载理由"] }
  }
  ```

  - `topology` 取模型事实（`modelFile` / `modelLine` + 出边数 / 入边数），零额外读盘；
  - `interface` **只给键名摘要，不给 content**：`ops` = 带 `x-function` 的 `properties` 键、`types` = `$defs` 本地定义键、`reexports` = 纯 `$ref` alias 键；`abstract` 是四形态的一句话判读（接口面留白 / 只有 op / 只有类型 / 有透传）；`present = false` 时三数组为空、`bytes = 0`；
  - `doc` 给 **`##` 段名 + 字节 + 行数**，MUST NOT 内联正文（14 KB 的文档灌进上下文会抢走 `read` 的翻页能力）；`present = false` 时 `headings` 为空。
- **`reading` SHALL 是工具输出的一部分**（静态文本，零变量插值）：内容须表达三条——① 理解本模块已足够（拓扑看 file/line 与出入边、接口看键名、文档看段名）；② 文档正文用 `read` **整读**（文档无结构，MUST NOT 用 `grep` / `offset` 拼凑）；③ 接口不必整读（`assets.interface` 的键名即定义）。理由是这段话写在 prompt 里会被 `AGENT.md` 的体量淹没。
- 省略 `element` 时 SHALL 返回全部元素的**薄概览**（`fqn` / `kind` / `gate` / 一行 `summary` / `api.status` / `hasPath`），并受既有单结果字节上限约束（超限截断留痕）。薄概览 MUST NOT 携带 `assets` / `reading` / `fileHash`（一元素多份读盘，不给全表付账）。
- **`resolvedEntry`（入口解析结果）**：SHALL 为 `{ path: string | null, via: "explicit" | "convention" | null, problem: string | null }` 三字段（字段不省略）——`path` 为 repo 相对 POSIX 入口路径，`via` 区分「模型显式 `entry`」与「约定默认命中」（单文件根 = 根文件自身 / 目录型根 = `<root>/index.ts(x)` / crate = `src/lib.rs`），`problem` 在入口不可用或不存在时给出人读成因。判据 SHALL 由 `api::paths::entry_state` **单点**求值（MUST NOT 在工具执行体或 `dto` 内另写入口解析、MUST NOT 复用 `apiFindings` 的判定分支）；该字段 MUST NOT 承载 `entry` 四条判据的长表（判据定义归 `desktop-archi-api` / `desktop-archi-api-check`）。
- **`hasPath`（`path` 事实）**：薄概览每条 SHALL 含 `hasPath: bool`（= 模型 `path` 数组非空，纯投影不补默认值），使「谁缺 `path`」一次调用即可判出；MUST NOT 以「`path` 字段是否省略」表达同义（显式字段不省略是既有投影纪律）。
- **`fileHash`（单文件内容哈希，本变更新增）**：单元素响应 SHALL 含 `fileHash: String`（形态 `sha256:<hex>`），值 SHALL 为「该元素声明文件（`modelFile`）的当前内容哈希」，与 `drift::file_hash` **同一函数产出**（MUST NOT 在执行体或 `dto` 内另算一份），`modelFile` 已在手 ⇒ 零额外读盘。该字段是**写面乐观锁的读侧闭环**：其值可直接作 `archi_element` / `archi_relation` 的 `expect`。语义边界两条：① MUST NOT 与 `archi_drift` 报告的 `modelHash` 比较（后者是全量聚合、非同粒度）；② MUST NOT 承担「模型是否被改动」的判定（那是 `assets_hash` 的用途）。薄概览 MUST NOT 携带该字段（一元素一次读盘，不给全表付账）。
- `modelFile` + `modelLine` SHALL 来自解析器已产出的声明位置（零额外实现），使 agent 能直接 `edit` 到该行而无需 grep 猜测。
- 元素不存在 SHALL 返回可诊断 `Err`（附近似 FQN 或总量提示），MUST NOT 返回空对象。
- 模型结构错时 SHALL 返回结构化诊断清单作为 `Err`（绝不 panic），使 agent 能看出"模型本身坏了"而不是"没有这个模块"。
- 响应 SHALL 经 `archi::dto` 单点投影产出（MUST NOT 在工具执行体内另拼一份 JSON）。
- **`api` 并列字段（换锚）**：SHALL 仍为四字段对象（`status` / `contractPath` / `schemaPath` / `generatedPath`），但——
  - `status` SHALL 为「接口定义在案」的事实值：`declared` ⟺ `<叶段>.schema.json` 在盘，否则 `none`；
  - 三条路径 SHALL **各自按自身在盘事实出线**（`contractPath` = 文档路径、`schemaPath` = 接口路径、`generatedPath` = 入口同级生成物路径，即便文件未生成也给路径串）；MUST NOT 再要求「文档与接口同时在盘」才报路径（旧的全或无投影会把三份资产的存在事实压成一个开关）；
  - 路径派生全经 `foundation::layout` 的派生族与 `api::paths` 单点产出（MUST NOT 行内拼路径），契约资产落在契约资产目录 `models/` 的 `<分层目录…>/` 子目录内；该字段 MUST NOT 承载协议正文（正文由 agent 用 `read` 按路径自取），MUST NOT 成为独立工具、MUST NOT 改变本工具的只读属性与注入矩阵；
  - 模型意图（`api = declared` / `none`）由 `element.api` 承载，MUST NOT 与 `api.status` 混说；字段名 MUST NOT 在本变更改名（`contractPath` 一直是文档路径，语义在 JSDoc 点明）。
- **`doc` 并列字段**：`doc` SHALL 为 repo 相对 POSIX 路径单值或 `null`（无 `title` / `kind` / `source` 四字段形态）；约定只有 `api = none` 元素会填（`api = declared` 元素的文档走 `api.contractPath`）——该约定**无机械判据**：`declared` 元素写了 `doc` 也**不报**（`apiUnexpected` 形态②已删除）。该字段**零判据**（写了不报、指不到不报），仅作 UI 的呈现事实（检索语料是两条资产路径，见「检索面纪律与迁移边界」）；工具响应原样投影该值，MUST NOT 为此降级或补默认值。
- **描述面（`mount.rs`）** SHALL 含五句：① 「**理解模块先调本工具**：三份资产的路径与结构面都在 `assets`；别的工具只给一面」；② 「接口的唯一凭据是 `.schema.json`」；③ 「文档正文用 `read` 整读（无结构），接口不必整读（键名即定义）」；④ 「`resolvedEntry` 给入口解析结果；`fileHash` 可直接作写工具的 `expect`」；⑤ 「`contractPath` 是文档路径、`schemaPath` 是接口路径」。MUST NOT 复制字段语法长表，MUST NOT 残留 `docs` 词汇。

#### Scenario: 定位单元素一次给全三资产

- **WHEN** 模型会话中调用 `archi_module(element="Backend.Infra.ArchiRuntime")`
- **THEN** 返回元素本体、出入边、子元素、`doc`、`api`、`resolvedEntry`、`fileHash` 与 `assets`（`topology` 的 `file` / `line` / 出入度数、`interface` 的键名摘要与 `abstract`、`doc` 的段名 / 字节 / 行数）与 `reading`；响应内 MUST NOT 出现文档正文与 schema 全文

#### Scenario: 三资产各自 present 独立

- **WHEN** 分别对「文档与接口都在盘」「只有接口在盘」「只有文档在盘」「两份都没有」四个元素调用
- **THEN** `assets.interface.present` 与 `assets.doc.present` 逐例为 `(true,true)` / `(true,false)` / `(false,true)` / `(false,false)`；不存在的资产其 `path` 仍给出派生串（或 `null` 仅当元素无 `path`）、`bytes = 0`、数组为空；MUST NOT 出现「缺一份则三段全空」

#### Scenario: 接口摘要四形态可辨

- **WHEN** 分别对空 schema、只有 op、只有类型、含透传 alias 的四个元素取 `assets.interface`
- **THEN** `abstract` 依次为「接口面留白（schema 在盘且无 op / 无类型）」/「只有操作」/「只有类型」/「含透传」；三数组逐键与 schema 逐项相等，且不含 `$defs` 内部的嵌套键

#### Scenario: 文档段名与整读指引

- **WHEN** 对一份 62 行、6 个 `##` 段的文档取 `assets.doc`
- **THEN** `headings` 恰为 6 个段名（不含 `#` 前缀）、`lines` / `bytes` 与磁盘一致；`reading` 含「整读」与「MUST NOT 用 `grep`」两条

#### Scenario: 薄概览不带三资产

- **WHEN** 省略 `element` 调用
- **THEN** 每条含 `fqn` / `kind` / `gate` / `summary` / `api.status` / `hasPath`，不含 `assets` / `reading` / `fileHash` / 三路径 / `doc` / `resolvedEntry`；超单结果字节上限时截断留痕（提示可指定元素收窄）

#### Scenario: 缺 path 一次判出

- **WHEN** 省略 `element` 调用，模型内既有声明了 `path` 的叶元素、也有无 `path` 的容器元素
- **THEN** 前者 `hasPath = true`、后者 `hasPath = false`（字段不省略）；`path` 数组本身只在单元素响应出线

#### Scenario: 入口三态可辨

- **WHEN** 分别对「显式 `entry` 命中」「目录型根由 `index.ts` 约定默认命中」「`path` 指向不存在目录而入口不可用」的三元素调用 `archi_module(element = …)`
- **THEN** 依次为 `resolvedEntry = { path: "<显式值>", via: "explicit", problem: null }` / `{ path: "<index 路径>", via: "convention", problem: null }` / `{ path: null, via: null, problem: "<不可用成因>" }`；判据与 `apiFindings` 的 `apiEntryMissing` 逐例一致（同一 `entry_state` 求值）

#### Scenario: 坏模型可辨

- **WHEN** 模型文件被写坏（语法错 / 环）后调用 `archi_module`
- **THEN** 返回结构化诊断（含 `file:line`）且 `is_error` 为真；执行体不 panic，run 不中断（agent 可转去修模型）

#### Scenario: api 三路径各自独立

- **WHEN** 对「接口在盘、文档缺失」与「接口缺失、文档在盘」两元素调用
- **THEN** 前者 `api.status = "declared"`、`schemaPath` 有串、`contractPath` 为 `null`；后者 `api.status = "none"`、`contractPath` 有串、`schemaPath` 为 `null`；两例的 `generatedPath` 都给路径串；`element.api`（模型意图）与 `api.status`（文件事实）分列可辨

#### Scenario: doc 字段原样投影且零判据

- **WHEN** 分别对 `api = none` 且声明了 `doc` 的元素、`api = none` 未声明 `doc` 的元素、`api = declared` 的元素调用 `archi_module`
- **THEN** 依次为「路径串」/ `null` / `null`（字段不省略）；`declared` 元素若写了 `doc` 也原样投影、零判据、零纳管影响（该字段零读者）；呈现面仍以 `api.contractPath` 承担其入口文档

#### Scenario: 薄概览带 api.status 但不带路径与 doc

- **WHEN** 省略 `element` 调用 `archi_module`
- **THEN** 每条薄概览含 `api.status`（`declared` / `none`）与 `hasPath`，不含 `contractPath` / `schemaPath` / `generatedPath` / `doc` / `resolvedEntry` / `fileHash`；超单结果字节上限时仍按既有纪律截断留痕

#### Scenario: fileHash 闭环可走通

- **WHEN** 先 `archi_module(element = X)` 取 `fileHash`，再调 `archi_element { action: "upsert", element: X, summary: "…", expect: <fileHash> }`，随后用响应里的 `hash` 再调一次
- **THEN** 第一次写成功；两次 `hash` 与各自落盘后 `drift::file_hash(声明文件)` 逐字相等；把 `archi_drift` 报告的 `modelHash` 当 `expect` 用则 `Err`（两个哈希不同粒度，MUST NOT 互比）

#### Scenario: 契约正文不经工具响应

- **WHEN** 审查 `archi_module` 响应体与体积
- **THEN** 只含 `api` 四字段（状态 + 三路径）、`doc` 单值、`resolvedEntry` 三字段、`fileHash` 与 `hasPath` 布尔，以及 `assets` 的键名 / 段名摘要（`interface.ops` / `types` / `reexports`、`doc.headings`）与 `reading` 静态一句；零签名、零 `$defs` 正文、零散文、零文档正文、零四字段文档条目；正文由 agent 用 `read` 按路径自取

#### Scenario: 契约路径零 `api` 段残留

- **WHEN** 对本仓契约在案元素（`Backend.Infra.ArchiRuntime`）取 `archi_module` 响应，并审查响应内的三个路径串
- **THEN** `contractPath` 为 `.dev-team/architecture/models/Infra/Backend.Infra.ArchiRuntime.md`、`schemaPath` 为同目录的 `.schema.json`；三串内 MUST NOT 出现 `architecture/api` 形态（旧段已退役）；派生点消失后仍由 `api::paths` 单点产出（工具执行体零路径拼接）

### Requirement: archi_search 只读工具的轻量检索契约

SDK 引擎 SHALL 提供专用只读工具 `archi_search`，语义为**发现**（还不知道这功能归谁管）：

```jsonc
{ "name": "archi_search",
  "parameters": {
    "query": "必填。关键词 / 元素名片段 / 资产路径片段",
    "scope": "可选。'elements' | 'doc' | 'all'（缺省 all）" } }
```

- 匹配对象 SHALL 恰为：元素 FQN、`summary`、`metadata` 值、**资产路径与文件名**（`path` 非空元素的在盘资产：派生 `<叶段>.md`（文档）与 `<叶段>.schema.json`（接口）两条固定规则，与 `api` 状态无关）。MUST NOT 匹配文档正文、MUST NOT 匹配 `openspec/specs/` 下任何路径或内容、MUST NOT 匹配 `decisions/*.md`（ADR 已退出自动检索语料）。
- 匹配方式 SHALL 为 ASCII 大小写不敏感子串 + **驼峰分词**（`AuthToken` → `auth` / `token`，两段均参与匹配）。
- `scope = "elements"` 时文档结果恒为空数组；`scope = "doc"` 时 `elements` 恒为空数组。**`scope` 的值名 SHALL 恰为 `elements` / `doc` / `all`**：`docs` 是非法值（`parse` 报错并列出三个合法值），MUST NOT 保留别名——双词会让 scope 语义二义。
- 文档结果条目 SHALL 退化为**一条 repo 相对路径字符串**（`ArchiSearchResult.doc: Vec<String>`），无 `title` / `kind` / `source` 四字段形态；排序 SHALL 按路径字典序（元素仍按 FQN 字典序），MUST NOT 引入相关度打分或来源强度序（来源概念已不存在）。
- 实现 SHALL 为每次现算（按两条固定规则派生 + 存在性判定），MUST NOT 建倒排索引或跨调用缓存（不引入第二事实源）。
- `query` 为空白串 SHALL 显式 `Err`（参数校验），MUST NOT 返回全量。
- 结果 SHALL 受既有单结果字节上限约束。
- **描述面（`mount.rs`）**：`scope` enum 与值名不变；描述内「入口文档路径」改为「资产路径（文档 / 接口）」。
- **语料面两条固定规则**（替换旧的「载体派生」口径）：文档语料 = 「`path` 非空且在盘的 `<叶段>.md`」∪「`path` 非空且在盘的 `<叶段>.schema.json`」。两条都不经 `api` 分支、都不经 `doc` 字段——`carrier_path` 已退役，检索面 MUST NOT 复活第二次载体派生；容器 / 包根仍不进语料（与判据面同一集合）。

#### Scenario: 关键词同时命中两资产

- **WHEN** 调用 `archi_search(query = "archi", scope = "all")`
- **THEN** 命中 FQN / summary 含该词的元素，与路径含该词的资产（如 `.dev-team/architecture/models/Infra/Backend.Infra.ArchiRuntime.md` **与** 同目录 `.schema.json` 两条都命中）；结果只含元素薄条目与路径串，MUST NOT 含四字段条目

#### Scenario: 语料与 api 状态无关

- **WHEN** 某 `api = none` 元素只有文档、某 `api = declared` 元素只有 schema
- **THEN** 前者文档命中、后者接口命中；两者都不因 `api` 状态被排除或纳入；`doc` 字段的值不参与语料

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

SDK 引擎 SHALL 提供专用**只读**工具 `archi_api`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**接口面自检、起草与引用计算**（`check` 自检、`bootstrap` 双草稿、`link` 算 `$ref`）：

```json
{ "name": "archi_api",
  "arguments": { "action": "check" | "bootstrap" | "link", "element": "<FQN>",
                 "from": "<FQN>", "to": "<FQN>", "name": "<导出名>" } }
```

- **三动作封闭**：`action` SHALL 只接受 `check` / `bootstrap` / `link` 三值（封闭值集；词汇外值以 `is_error` 回灌并点名三个合法值，MUST NOT 静默退化）。
- **`check` = 两轴结构化结果**：返回「生成物新鲜度」与「契约 vs 代码」两轴的并列内容——① 三类差异（`mismatched` / `orphaned` / `missing`，repo 相对串，取自与 IPC / CLI 同一份比对实现）；② 契约判据摘要（按 `category` 计数的映射 + 逐条 `{ kind, category, element, target, message }`）。响应 SHALL 复用既有出线形状（`ArchiApiCheck` 的字段与 `ArchiApiFinding` 的条目），MUST NOT 在工具执行体内另拼一份投影；MUST NOT 各自实现在第二处（判定单源）。**历史 `api-*` fence MUST NOT 产生任何计数或提示**（它已退化为普通文档内容）。
- **`bootstrap` = 双草稿（起草第一步）**：`element` 必填，返回 `{ schema, doc }` 两段草稿文本——与 CLI `bootstrap` 同一份内容（形态归 `desktop-archi-api`「bootstrap 双草稿（schema 骨架 + doc 骨架）」），本能力 MUST NOT 另立第二份形态判据。该动作是新建 / 补接口的**第一步**，描述面 SHALL 说出这句指路（否则 agent 只知它能出草稿、不知该先用它）。`element` 缺失 / 元素不存在 / 无实现入口 ⇒ `is_error` 并透传 `ApiError` 文案。
- **`link` = `$ref` 串计算（纯计算，零读盘写盘）**：
  - 跨元素形态：`{ from, to, name }` → `"<ref_relative(from, to)>#/$defs/<name>"`（例：`"../../Desktop/Components/Agent/AgentInput.schema.json#/$defs/AgentInputProps"`）；
  - 本模块形态：`{ element, name }` → `"#/$defs/<name>"`；
  - `from` / `to` 缺失或非 FQN 形态、`name` 非 ASCII 标识符 ⇒ `is_error` 点名成因；两形态互斥（同给 `to` 与省略 `to` 以外的一切组合都拒）。
  - 描述面 SHALL 明写「跨元素 `$ref` 用 `link` 算，勿手拼」——手算 `../` 层数少一层会绕过 `apiRefMissing`，到编译期才炸。
- **零写盘**：三动作 MUST NOT 创建 / 覆盖 / 删除任何文件——落盘仍由会话经 `write` / `edit`（写闸校验）执行，生成物过期由会话经既有命令重生成（命令名归属本仓文档面，产品 MUST NOT 内建具体命令名）。MUST NOT 存在「写生成物 / 写契约」的专用工具（`archi-runtime` 对 agent 面不可达，见 `desktop-crate-layout`）。
- **结构错以 `is_error` 回灌**：模型含任一结构诊断时三动作 SHALL 返回 `is_error` ToolResult（诊断清单带 `file:line`），MUST NOT 返回部分结果、MUST NOT 抛 panic。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）；超限时摘要（按 `category` 计数）优先保留。
- **上下文路由沿用**：执行体 SHALL 经 `LoopTurn` 的 per-turn `ArchiContext` 路由（root + 模型读面），MUST NOT 新增 `ToolExec` 变体、MUST NOT 把 store 句柄塞进七通用工具的 `execute` 签名。
- **无路径参数**：本工具不带 `path` 参数 ⇒ 路径沙箱对它恒原样返回（MUST NOT 因缺字段报错），写入保护对它零射程（读面全开放）。
- **提示面**：被下发本工具的相位 prompt SHALL 有一行说明（沿既有「额外可用只读工具」写法，当前为 `implementation-generator.md`），内容限于「改完代码可自查生成物新鲜度与契约判据；生成物过期用既有命令重生成，MUST NOT 手改生成物」。
- **描述面（`mount.rs`）**：`archi_api` 的工具描述 SHALL 含四句，且 MUST NOT 复制契约 md 的段形态长表（形态归 `desktop-archi-api`）、MUST NOT 内建本仓专用命令名（生成命令归属各仓文档面）——① `bootstrap` 是新建 / 补接口的第一步（给两份草稿）；② `link` 算 `$ref` 串（勿手拼）；③ **三类**生成物的点名（架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）连同「MUST NOT 手改生成物」；④ 冷启动口径「`artifacts.enabled = false` 即接口面未启用，此时 `findings` 恒空且**不等于**零问题」。MUST NOT 把 `.schema.json` 说成生成物。

#### Scenario: check 返回两轴且历史 fence 零计数

- **WHEN** 在 implement/executor 会话（或架构页 `source = "archi"` 会话）中调用 `archi_api(action = "check")`，某元素的文档仍含两个历史 `api-op` fence
- **THEN** 响应含三类差异与按 `category` 计数的判据摘要（逐条含 `kind` / `element` / `message`）；响应内零「fence 残留」类计数键；调用前后磁盘字节完全相同；`archi_api` 与 IPC 的 `archi_api_check` 报的三类差异集合逐条相等

#### Scenario: bootstrap 双草稿只出文本

- **WHEN** 调用 `archi_api(action = "bootstrap", element = "App.Auth")`
- **THEN** 返回 `{ schema, doc }`：`schema` 含 `$id` / `title` / `properties` 占位与 `$defs`，`doc` 只有 H1 + 一句指引（形态归 `desktop-archi-api`）；磁盘零变化；缺 `element` 或元素不存在 ⇒ `is_error` 且文案点名成因

#### Scenario: link 两形态各出正确串

- **WHEN** 分别以 `{ from: "Backend.Infra.ArchiRuntime", to: "Backend.Core.Foundation", name: "Summary" }` 与 `{ element: "Backend.Core.Foundation", name: "Summary" }` 调用
- **THEN** 前者出 `"../../Core/Foundation/Backend.Core.Foundation.schema.json#/$defs/Summary"`（前缀与 `paths::ref_relative` 逐字一致）、后者出 `"#/$defs/Summary"`；两次都零写盘

#### Scenario: link 参数面互斥与校验

- **WHEN** 以 `{ from, name }`（缺 `to` 且缺 `element`）、`{ from, to, name: "a.b" }`、`action = "generate"` 分别调用
- **THEN** 三者均 `is_error`：前者点名缺 `to` / `element`，中者点名 `name` 须为标识符，后者点名合法三值；零写盘

#### Scenario: 词汇外动作被拒

- **WHEN** 调用 `archi_api(action = "generate")` 或 `action = "write"`（词汇外值）
- **THEN** 返回 `is_error` 并点名合法值 `check` / `bootstrap` / `link`；零写盘、零副作用

#### Scenario: 结构错回灌而不崩

- **WHEN** 模型被写成语法错后调用 `archi_api(action = "check")`
- **THEN** 返回 `is_error` ToolResult（诊断带 `file:line`），run 不中断、进程不 panic

#### Scenario: 体量经单点上限

- **WHEN** 契约面有数十条判据时调用 `check`
- **THEN** 响应被 `tools::cap_result` 截断（沿既有 L1 上限），摘要保留；工具执行体内零第二份截断实现

### Requirement: archi_scaffold 只读工具的新元素模板契约

SDK 引擎 SHALL 提供专用只读工具 `archi_scaffold`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**给字段词典**（要写新元素但记不住字段形态）——草稿同时是最新字段面的可抄改模板，一个工具一个形状（MUST NOT 另设 `mode = "draft" | "dict"` 双形态，亦 MUST NOT 改出纯 Markdown 词典）：

```jsonc
{ "name": "archi_scaffold",
  "parameters": { "kind": "可选。元素 kind 四值之一（值集由 ElementKind::ALL 派生）；省略取 domain" } }
```

- **值集封闭且派生**：`kind` 的合法值 SHALL 由 `ElementKind::ALL` 派生（MUST NOT 手抄四值）；词汇外值 SHALL 以 `is_error` 回灌并**点名**合法值（MUST NOT 静默退化为缺省）。
- **响应形状**：`{ "draft": "<TOML 片段>" }`——**一段**元素表模板，字段按模型字段面顺序列出（`kind` 实值；`gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 以注释行给出并各附一行纪律），另附一条 `[[relation]]` 模板注释。字段顺序 SHALL 与 `archi::parse::ELEMENT_FIELD_NAMES` 同序（= `RawElement` 声明序，见 `desktop-archi-model`「元素字段面的机器可读清单」）。
- **描述面不枚举字段名**：工具定义（`mount.rs` 的 `archi_scaffold_definition()`）的描述 SHALL 以词典措辞表述（「字段词典 / 逐字段一行纪律 / 值域 / 关系四键」），MUST NOT 内联枚举具体字段名（如「字段序 / 内联 `metadata` / `path` 必填提示 / 关系模板」）——枚举会随字段面扩容静默陈旧，与 `archi_module` 描述「MUST NOT 复制字段语法长表」同一条纪律。
- **字段词典语义（每个字段一行纪律）**：草稿 SHALL 对八个字段面字段逐一到场并各给一行纪律，覆盖四类事实（字段各取其一或多项）——① 值域（`kind` 四值语义 / `gate` 三值 / `api` 二值）；② 缺省档（`gate` 缺省 `off` / `api` 缺省 `declared` / `entry` 与 `doc` 选填）；③ 基数与形状（`path` 随 `kind` 的基数、`./` 前缀、目录尾 `/`、文件到扩展名）；④ 单值性（`entry` / `doc` 是单值）。纪律 MUST 与模型侧判据同源：`metadata` SHALL 写明值 MUST 是字符串（数字 / 布尔等非字符串值被拒）；关系 `apis` SHALL 写明四条——只写名字（禁签名 / 类型形状 / `::` 限定路径 / 空白）、逐项非空、同名不重复、`apis = []` 与省略同义（显式空数组被拒）。
- **字段面全集对拍**：草稿 SHALL 含字段面全集的逐字段行——断言 SHALL 遍历 `archi::parse::ELEMENT_FIELD_NAMES` 做**行首匹配**（`{field} = ` / `# {field}：` / `# {field} = `），MUST NOT 退化为整串 `contains(field)`（`apis` 会让 `api` 假命中，`doc` 一类行内复述会掩盖整行缺失）。消费侧 MUST NOT 手抄八个字段名字面量。
- **值域派生对拍**：草稿的门 / 状态纪律行 SHALL 逐值覆盖 `GateTier::ALL`（`off` / `warn` / `strict`）与 `ApiStatus::ALL`（`declared` / `none`），且缺省档（`off` / `declared`）以「缺省」字样在场；`kind` 四值语义由 `ElementKind::ALL` 逐值覆盖。三组枚举新增变体而未补词典即红。
- **模板自证可解析且零结构诊断词**：草稿 SHALL 为合法 TOML 且经 `archi::parse` 的 `TomlParser` 解析后**零结构诊断**（机械锚：字段名 / 键形 / `metadata` 形态 / `kind` 四值语义任一漂移即红）。纪律性提示 SHALL 只出现在注释行——MUST NOT 以注释外的非法行（如 `path = "<待填>"`）破坏可解析性。草稿 MUST NOT 含结构诊断词（`invalidValue` / `unknownField` / `missingField` / `syntaxError` 零命中）：值面规则只写行为（「被拒」），诊断词表不进词典（与「结构诊断全量词表 MUST NOT 进知识文本」同源）。
- **零写盘**：本工具 MUST NOT 创建 / 覆盖 / 删除任何文件（与 `archi_api` 的 `bootstrap` 同款「只出 stdout 语义」）；MUST NOT 成为「写模型工具」——写模型的 canonical 入口是 `archi_element` / `archi_relation`，手改面仍是既有 `write` / `edit`（见「模型 = 源码（两条写入口与写纪律）」）。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）；草稿体量 SHALL 远低于该上限（字段词典是「一行一字段」，MUST NOT 演化成判据长表）。
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

#### Scenario: 草稿覆盖元素字段面全集（行首匹配，子串不顶替）

- **WHEN** 遍历 `archi::parse::ELEMENT_FIELD_NAMES`（八项：`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）并逐字段在草稿里找**行首形态**（`{field} = ` / `# {field}：` / `# {field} = `）
- **THEN** 每个字段名都恰有一个行首载体（`kind` 是唯一的实值行，其余为注释行）；删掉任一行首载体即红——删 `# api：` 行后 `apis` MUST NOT 顶替 `api`；新增字段而未补词典（草稿缺该字段的行首载体）同样即红

#### Scenario: 值域逐值在场且缺省档可辨

- **WHEN** 调用 `archi_scaffold` 并遍历 `GateTier::ALL` / `ApiStatus::ALL` 检查草稿
- **THEN** `gate` 纪律行逐值含 `off` / `warn` / `strict` 且 `off` 以「缺省」字样在场；`api` 纪律行逐值含 `declared` / `none` 且 `declared` 以「缺省」字样在场；新增任一变体而未补词典即红；同一次响应里草稿对 `invalidValue` / `unknownField` / `missingField` / `syntaxError` 四词零命中

#### Scenario: metadata 与 apis 的值面纪律在场

- **WHEN** 审查草稿的 `metadata` 与关系 `apis` 两段注释
- **THEN** `metadata` 段含「值 MUST 是字符串」与「非字符串值被拒」，且「子表与内联表等价」仍在场；`apis` 段含「只写名字」「逐项非空」「同名不重复」「`apis = []` 与省略同义」「显式空数组被拒」「生成物面留空」「`archi_drift`」；`entry` 的注释行以 `# entry：` 起首且含「单值」（`doc` 行的「单值」MUST NOT 顶替它）；三段均不点名诊断词

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

| phase | role | `archi_module` | `archi_search` | `archi_api` | `archi_drift` | `archi_scaffold` | `archi_element` | `archi_relation` |
|---|---|---|---|---|---|---|---|---|
| `proposal` | executor | ✅ | ✅ | **✅** | ❌ | ✅ | ✅ | ✅ |
| `dev-design` | executor | ✅ | ✅ | **✅** | ✅ | ✅ | ✅ | ✅ |
| `dev-design` | evaluator | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `test-design` | executor | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `implement` | executor | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `code-review` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 其余相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync 等） | 任意 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

- **`archi_api` 的两格扩面（本变更）**：`(proposal, executor)` / `(dev-design, executor)` 由 ❌ 改 ✅。理由：接口定义成了手写真相，`bootstrap` 双草稿与 `link` 的 `$ref` 计算价值恰在起草相位；不补则 `archi_scaffold` 描述里「接口草稿用 `archi_api` 的 `bootstrap`」指向一个未下发的工具。`archi_api` 是**只读**档，扩面不扩写权（两写工具的矩阵集与 `model_write_lock` 的同集纪律逐字不变）。
- **矩阵单源**：`(phase, role)` → archi 工具名清单的映射 SHALL 只在 `archi_matrix` 一处（未命中即零 archi 工具），MUST NOT 在别处再写一份同名映射。`dev-design` 的 executor / evaluator **分两行**（`drift` 与两写工具只给写模型的执行侧），MUST NOT 合并成一行后按 role 分支。
- **两写工具只给写权相位**（`proposal/executor`、`dev-design/executor`）与 archi 来源臂：矩阵下发写工具的 `(phase, role)` 集合 SHALL 与 `compose::model_write_lock` 判定为 `None`（不挂会话锁）的集合**逐格相等**——MUST NOT 出现「矩阵给了工具但写权锁着」或「锁开了却没有写工具」的错配（两条方向都设断言）。
- **`archi_drift` 只给两处直接产出漂移的相位**：`dev-design/executor`（写元素与边）与 `implement/executor`（改代码 ⇒ 未声明依赖）。`code-review` / `acceptance` MUST NOT 下发（越界判断仍只读模型面；「时机差」作为将来放宽的理由需独立裁决，MUST NOT 由本期实现顺手放开）。
- **`archi_scaffold` 只给两处写权相位**（`proposal/executor`、`dev-design/executor`）与 archi 来源臂——模型写权之外的下发等于许诺一个不会被允许落地的模板。
- **`archi_api` 只给三处并集**（`proposal/executor`、`dev-design/executor`、`implement/executor`）与 archi 来源臂：其余相位 MUST NOT 下发（生成物新鲜度由门禁与契约面板承担）。
- **archi 来源臂**：`source = "archi"` 的会话 SHALL 装配为七内建全量（含 `write` / `edit` / `bash`）+ **七个** archi 工具名（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation`）+ `preamble = Some(<架构会话知识文本>)`（知识内容见「架构会话的系统知识注入」）+ `before_done = Some(ArchiPreclose)`（收口前自检装配档位，见「收口前引擎自检（架构判据读数）」）。该臂 SHALL 以 `source` 短路命中，MUST NOT 进入 `source_ref` 相位解析——`source_ref` 为 null（一 workspace 一份历史）与该定式无关，且其取任意值（含畸形四段）MUST NOT 改变该臂的下发面，MUST NOT 因解析失败退化为零 archi 工具。
- `phase` / `role` SHALL 取自既有 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`（不新增契约字段）；解析 SHALL 取尾三段的定式（`attempt` → `role` → `phase`），段数 < 4 或末段非数字即 `None`。
- 解析不出该定式（畸形 / 短段 / 未知 phase）SHALL **fail-closed**：零 archi 工具（MUST NOT 退化为"给全部工具"）。
- 相位矩阵 SHALL 只对 `source = "change"` 生效（archi 来源臂不查矩阵、恒七工具全开）。
- `source = "explore"` 臂的 **archi 面 SHALL 恒为空**（零 archi 工具，逐字沿用）；该臂的**完整**名字清单（六工具收窄 + `create_explore_note` + 只读 DB 工具 `db_tables` / `db_query` 两件）单源见 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」与 `desktop-agent-db-tools`，本能力 MUST NOT 另写一份；其余**非 change、非 config** 来源（debug 等）SHALL 恒「七内建全量 + 零 archi 工具 + `ask = Generic`」的显式构造（`source = "config"` 臂的名字清单与位取值单源见 `desktop-config-agent` 与 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」；该臂零 archi 工具、`preamble` 为配置协议知识而非架构会话知识，本能力 MUST NOT 另写一份；MUST NOT 以默认构造兜底——默认值的 `ask = None` 即静默禁用）。
- **change 臂的知识段（本变更）**：`source = "change"` 会话在 `model_write_lock(provenance) == None`（即两个写权相位）时 SHALL 注入**同一份**架构会话知识（`archi_knowledge::knowledge()`，单来源）；其余相位（含 `implement/executor` —— 它有 archi 只读工具但不能写模型）SHALL 保持 `preamble = None`。判据 MUST NOT 另立一份矩阵命中判定（写权与知识同源）。知识拼装失败 SHALL 降级为 `None`，MUST NOT 影响工具面与 run。
- 矩阵只决定**下发面**，不决定 prompt 文本（prompt 侧见 `desktop-phase-prompts`）；`preamble` 位只由装配层按来源决定（生产者 = archi 臂 / change 写权相位 / config 臂），前端 / 自由字符串 MUST NOT 直接供给。

#### Scenario: 逐相位矩阵命中（两写权相位含 api）

- **WHEN** 分别以 8 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** 每行 definitions 中 archi 工具恰为矩阵勾选者（proposal 为 `module` / `search` / `api` / `scaffold` / `element` / `relation`；dev-design 执行为 `module` / `search` / `api` / `drift` / `scaffold` / `element` / `relation`、评估为 `module` / `search`；implement 为 `module` / `api` / `drift`；code-review / acceptance 只 `module`），`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 写工具与写权锁同集

- **WHEN** 遍历全部 `(phase, role)` 组合，对每个组合分别取矩阵结果与 `model_write_lock` 的返回值
- **THEN** 「矩阵含 `archi_element`」⟺「`model_write_lock` 返回 `None`」，两个方向逐格相等（archi 来源臂同判）；`archi_api` 的扩面 MUST NOT 改变该对拍结果；任一格不成立即测试红

#### Scenario: 知识段与写权同源

- **WHEN** 分别以 `(proposal, executor)` / `(dev-design, executor)` / `(implement, executor)` / `(code-review, evaluator)` 发起 change 会话并审查 `preamble` 位
- **THEN** 前两者 `preamble` 为架构会话知识（与 archi 臂逐字相同）；后两者 `preamble` 为 `None`；四者的工具面仍按矩阵下发

#### Scenario: 知识降级不影响工具面

- **WHEN** 知识拼装失败（派生异常）后以 `(dev-design, executor)` 发起会话
- **THEN** `preamble = None`，会话照常发起、矩阵命中的工具面与 `before_done` 位不变

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` / `(code-analyze, executor)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；该轮调用 archi 工具按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具（回落七内建全量）、`preamble = None`（写权判据亦不成立），MUST NOT 抛出装配错误、MUST NOT 给全量 archi 工具

#### Scenario: archi 来源全工具面加知识

- **WHEN** 以 `source = "archi"`（`source_ref = null`）发起会话并审查该轮请求组装
- **THEN** `tools` 位为七内建全量（含 `write`）另加**七个** archi 工具名、`preamble` 为架构会话知识文本、`before_done` 为 `Some(ArchiPreclose)`；definitions 含七个 archi 定义且 `RunStarted.tools` 逐名一致

#### Scenario: archi 臂不因 source_ref 变形漂移

- **WHEN** 分别以 `source_ref = null`、`source_ref = "some-id"`、`source_ref = "a/b/c/1"` 发起 `source = "archi"` 会话
- **THEN** 三者装配结果完全相同（同一 archi 臂：七工具全开 + 七内建全量 + 知识 preamble）；MUST NOT 因 `source_ref` 形态触发相位解析或 fail-closed

#### Scenario: explore 与 debug 来源零 archi 工具

- **WHEN** 分别以 `source="explore"` 与 `source="debug"` 发起会话
- **THEN** explore 为 read / grep / glob / ls / edit / bash + `create_explore_note` + `db_tables` / `db_query`（只读 DB 工具两件，清单单源见 desktop-agent-db-tools），debug 为七内建（含 `write`）；两者均无 archi 工具、`preamble` 均为 `None`

### Requirement: archi 工具档位归属与成员校验（五只读 + 两写）

七个 archi 工具 SHALL 按读写面分两档：`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` 归 **只读面**（挂载清单 `sdk/mount.rs` 中准入档 SHALL 为 `ToolTier::Read`，`policy::allows` 对三档——`default` / `acceptEdits` / `bypassPermissions`——**全部放行**）；`archi_element` / `archi_relation` 归 **写面**（准入档 SHALL 为 `ToolTier::Write`：`default` 档拒绝，`acceptEdits` / `bypassPermissions` 放行，与 `write` / `edit` 同档）。MUST NOT 为任一 archi 工具引入单独的放行名单（准入判据单点在挂载清单的准入档）。

以下既有语义 MUST NOT 变化：

- **成员校验先行**：调用名 ∉ 本轮实际下发 definitions 即 `SystemNotice{subtype:"tool_unavailable"}` + `is_error` ToolResult 回灌，run 不中断、不触执行体；
- **路径沙箱不介入**：七工具无路径参数，`sandbox` 改写面对其恒原样返回（MUST NOT 因缺 `path` 字段报错；两写工具的落点由 `file` / 父元素推得，`tools::input_path` 对其恒 `None`）；
- **写入保护按面分**：五只读工具为读面，`write_protection` 的守卫 MUST NOT 拦截；两**写**工具写盘前 SHALL 把解析出的目标文件过同一 `ProtectRules`（细则归 `desktop-write-protection`「会话级写入禁区（相位锁）」）；
- **未知工具仍拒**：词汇外的工具名在任何档位一律拒绝（封闭清单语义不变）。
- 工具执行上下文 SHALL 经 `LoopTurn` 的 per-turn 上下文路由（root + 模型读面），MUST NOT 把 store 句柄塞进七通用工具的 `execute` 纯函数签名；上下文缺席 SHALL 以 `is_error` 回灌（生产不可达）。铸造判据 SHALL 为「本轮名字清单含任一 archi 工具名」（**七名**逐一在场判断，MUST NOT 漏掉新增两名）。

#### Scenario: 默认档放行五个 archi 工具

- **WHEN** 以 `permissionMode = default` 在矩阵命中相位调用 `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`
- **THEN** 五者均放行并执行（无 `permission_denied`）；同档调用 `write` 与 `archi_element` / `archi_relation` 仍被拒（只读面未扩宽）

#### Scenario: 写工具在非默认档放行且先过成员校验

- **WHEN** 以 `permissionMode = acceptEdits` 在矩阵命中相位调用 `archi_element` / `archi_relation`；另在矩阵**未命中**相位以同一档位调用
- **THEN** 前者放行并执行；后者被成员校验拒绝（`tool_unavailable` + `is_error`，run 不中断、执行体零调用）——档位与成员校验是两道独立闸，任一不通过都不落到写盘

#### Scenario: 矩阵外调用被成员校验拒绝

- **WHEN** 在 `(test-gen, executor)` 会话中模型幻觉调用 `archi_search` 或 `archi_drift`
- **THEN** 收到 `tool_unavailable` + `is_error` ToolResult 回灌，run 不中断、执行体零调用

#### Scenario: 沙箱与写入保护按面分

- **WHEN** 在七个 archi 工具被下发的会话中审查 `sandbox` 改写与写入保护判定
- **THEN** 七者均原样通过沙箱（无 `path` 入参）；五只读工具零保护射程，两写工具只在解析出目标文件后受保护判定约束；受保护文件的 `write` / `edit` 行为逐字不变

#### Scenario: 挂载清单项数

- **WHEN** 审查 `mount::registry()` 的项数与归组
- **THEN** 恰 **18** 项：七内建 + `create_explore_note` + 五 archi 只读 + 两 archi 写 + 两 DB 只读 + `propose_config`；五只读项 `ToolTier::Read`、两写项 `ToolTier::Write`，七个 archi 项均 `ToolExec::Archi`

### Requirement: 模型 = 源码（两条写入口与写纪律）

架构模型文件 SHALL 与源码同权：agent SHALL 直改 `<archi_dir>/graph/model/*.toml`。**写模型有两个入口，MUST NOT 存在第三个**：

| 入口 | 形态 | 失败语义 | 定位 |
|---|---|---|---|
| 专用写工具 `archi_element` / `archi_relation` | 结构化参数面（`desktop-archi-agent`「archi 写工具面」） | **坏文本不可产出**：渲染 → 重解析自证 → 回读一致，任一失败即 `Err` 且零落盘 | **canonical**：键形 / 值域 / 祖先 / 计数 / 契约告警全在工具里 |
| `write` / `edit` 手改 | 自由文本 | **写坏模型仍是合法状态**：由读取面报错（结构诊断 `file:line`、画布停摆、`archi_preclose` 兜底） | **second path**：工具表达不了的（批量整理 / 同层手排序）才用，改完必须自查 |

两条与文件序相关的边界 SHALL 与本表同批成立（本变更新增）：

- **子树连续与关系组连续由写面维持**：专用写工具的产出恒满足「父先于子 / 子树连续 / 同一 `from` 组连续」（见 `desktop-archi-model`「写入序不变量」）；手改拆开的子树 MUST NOT 被承诺长期保留——下一次工具写会把它合回原位（同层兄弟的手排相对序则被保留）。
- **手改的正当余量 = 批量整理与同层手排序**：MUST NOT 把手改的适用边界再表述为「重排文件」——整份文件的规范化已由每次工具写承担。

写权（谁能写）与写入口（怎么写）是两件事，MUST NOT 混说：

- **写权 SHALL 有且只有两条通路**，且**通路 A 按相位收窄**：

| 通路 | 触发面 | 执行根 | 确认时机 | 归属能力 |
|---|---|---|---|---|
| A（既有，按相位收窄） | change 流程相位会话——**仅 `phase ∈ {proposal, dev-design}` ∧ `role = executor`** | `detail.worktree` | 回合末归档确认面（模型区块 + 冲突分流） | `desktop-change-archive` |
| B（既有） | 架构页通用会话（`source = "archi"`），不限相位 | workspace root（主仓，`cwd = null` 回落） | 事后 `git diff` 评审 + 用户手动刷新校验 | `desktop-archi-chat` |

- **写权是强制面问题，不是工具面问题**：`archi` crate 全程只读盘（模型唯一落盘点是 `write_atomic` 单点），故「写入口」才分工具 / 手改两种；写权政策实做是**掐掉其余相位对模型目录的射程**——由会话级写入禁区（第三规则来源）承载，见 `desktop-write-protection`「会话级写入禁区（相位锁）」。两写工具只在写权放开的相位下发（矩阵与写权锁同集，见「archi 工具面注入与 (phase, role) 矩阵」）；MUST NOT 出现「有工具但无写权」或「有写权但无工具」的错配。
- **两条失败语义 MUST NOT 混为一句**：写工具面 MUST NOT 产出坏文本（自证链使它结构上不可达）；手改面 MUST NOT 因加工具而收紧——**写坏模型仍由读取面报错**（`archi_module` 的结构诊断带 `file:line`、画布停摆态、`archi_preclose` 读数）。MUST NOT 把「写盘前校验」当写权护栏（`write` / `edit` 对**接口定义文件之外**的一切路径零预校验是既有承诺）。
- **两条推论（政策是绝对命题，不是步骤枚举）**：
  - **`dev-design/evaluator` 被锁**：白名单按 `(phase, role)` 而非 phase——评审批次不写模型。
  - **`explore` / `debug` 也被锁**：两者不是「步骤」，但政策覆盖一切来源；explore 的 `edit` 只服务笔记，锁模型零损失。
- **不依赖运行时审批**：写盘那一刻 MUST NOT 要求用户确认（工具分派链只有 `deny`，无挂起等确认能力）。通路 A 的设计权由「回合必须人工确认」承载；通路 B 的设计权由事后 diff 评审 + 手动刷新可见性承载（MUST NOT 承诺写盘前拦截）。
- **内置锁面仍不纳入**：`<archi_dir>/graph/model/` 与架构域配置 MUST NOT 进入 `write_protection` 的**内置**锁面（内置规则是相位无关的绝对路径 glob，纳入即等于两条通路都改不了模型）；相位条件的锁 SHALL 走**会话级**规则面（见 `desktop-write-protection`「会话级写入禁区（相位锁）」）。
- **两通路并存，互不替代**：MUST NOT 因通路 B 存在而移除通路 A（change 侧 worktree 隔离 + 归档确认仍是「改动随 change 一起评审」的唯一形态）；亦 MUST NOT 因通路 A 收窄而收紧通路 B（架构页会话的用途恰是「不进 change 也能维护模型」）。
- **场景归属**：冷启动生成骨架 / 增量维护（新增或重构模块）/ 按现状补声明 / 直接修复图或文档错误，四类场景 SHALL 可走任一条通路（走 change 时落 `dev-design` 阶段，走架构页时落通用会话），MUST NOT 强制单一入口。非白名单相位遇到「模型该改」的需求 SHALL 把需求写进产物（design / tasks）交 `dev-design` 落地，MUST NOT 尝试绕过。
- **新元素缺省档**：两入口生成的新元素均缺省 `gate = "off"`（ratchet 逐块收紧）。
- **通路 B 的边界（软约束，非写入面强制）**：会话知识 SHALL 要求「禁止修改架构目录之外的文件（源码 / 配置 / 脚本 / 域外文档一律不改，用户要求亦不越界；生成物重生成除外）」，但写入面 MUST NOT 为该约束设硬隔离——该缺口 SHALL 在能力 spec 中如实留痕，MUST NOT 被表述为「会话只能改模型」。
- **生成物同权但不可手改（本变更由四类改三类）**：`.schema.json` **不再是生成物**（它是手写真相，走 `write` / `edit`），生成物只剩 `INDEX.toml` / `.i.rs` / `.i.ts`。三类生成物与模型文件同属磁盘可写面，但**没有专用写工具**——两写工具的 `file` 只接受模型声明目录一级 `*.toml`，射程 MUST NOT 扩到生成物；会话知识 SHALL 要求「MUST NOT 手改生成物，用既有命令重生成」——该约束同样只有事后兜底（零 diff 门 + 契约面板），MUST NOT 表述为写入面拦截；手改通路对生成物仍零写前校验。
- **手写接口面有一处显式写前校验（本变更新增的唯一例外）**：`write` / `edit` 落 `<archi_models_dir>/**/<叶段>.schema.json` 时 SHALL 先过接口校验（坏文本 `Err` 零落盘，见 `desktop-agent-execution`「`write` / `edit` 的接口定义写闸」）。该例外 MUST NOT 被读成写权护栏或保护面：它只判「文件级自洽」，与谁能写无关；手改通路对**其余一切文件**（含 `INDEX.toml` / `.i.*` / 模型 TOML / 普通源码）仍零预校验。

#### Scenario: worktree 内直改模型成功（白名单相位）

- **WHEN** agent 在 `dev-design/executor` 会话中用 `write` 新建 `<archi_dir>/graph/model/03-billing.toml`，或用 `edit` 修改既有模型的 `gate` 行
- **THEN** 写入成功（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后读取面解析出该改动，主仓模型零变化

#### Scenario: 白名单外相位写模型被拒

- **WHEN** `implement/executor` / `code-review/evaluator` / `dev-design/evaluator` 会话对 `<archi_dir>/graph/model/*.toml` 调用 `write` 或 `edit`
- **THEN** 拒绝串含路径、工具名、指路（写进 design / tasks 交 `dev-design` 落地，或改用架构页会话）与「不要改用其他命令或工具尝试绕过」；run 不中断、文件零变化

#### Scenario: 主仓直改模型成功（架构页）

- **WHEN** agent 在架构页通用会话（`cwd = null` → 主仓根）内用 `write` / `edit` 改 `<archi_dir>/graph/model/*.toml`
- **THEN** 写入落在主仓该文件（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后同 workspace 的 `archi_module` 与画布取数读到该改动，改动保持未提交由用户 `git diff` 评审

#### Scenario: 手写接口面是唯一预校验面

- **WHEN** 分别以 `write` 落一份坏 `.schema.json`、一份坏 `INDEX.toml`、一份坏模型 `*.toml`
- **THEN** 第一者 `Err` 零落盘（写闸）；后两者原样落盘、由读取面 / 零 diff 门报错（写前零校验不变）

#### Scenario: 生成物只三类

- **WHEN** 审查写纪律文本与生成物清单
- **THEN** 只点名 `INDEX.toml` / `.i.rs` / `.i.ts` 三类；`.schema.json` 被表述为**手写真相**（走 `write` / `edit` + 写闸），MUST NOT 与三类并列

#### Scenario: 写坏模型不阻断写盘（手改面）

- **WHEN** agent 用 `write` / `edit`（不是写工具）写出语法错的模型文件（白名单相位或通路 B）
- **THEN** 写盘成功；随后 `archi_module` / 画布数据返回结构诊断（`is_error` / 停摆态）且进程不 panic；收口时 `archi_preclose` 读数含该结构错

#### Scenario: 写工具面不可产出坏文本

- **WHEN** 以任何合法入参调用 `archi_element` / `archi_relation`，并构造一个会渲染出诊断的边界（如 `apis = [""]` / 非法 `path` 形态）
- **THEN** 一律 `Err` 且**零落盘**（自证链在交文本之前拦下），盘上字节零变化；该失败语义不得被表述为「与手改面相同」

#### Scenario: 两入口并存且无第三条通路

- **WHEN** 审查执行面工具清单与写模型入口
- **THEN** 写模型恰有两个入口（专用写工具 canonical / `write`+`edit` second path），工具面在写权相位下发两个写工具且在非写权相位零下发；MUST NOT 存在第三条通路（如独立 IPC 写命令或画布写入口，含任何 `archi_sort` 一类手动排序入口）；`archi_set_gate` 与写工具共用同一 `plan_element_upsert`（同实现 ⇒ 同文本）

#### Scenario: 手改与写面对文件序的分工可辨

- **WHEN** 审查本 requirement 的手改出口表格行与两条「文件序边界」（骨架 md 侧只保留简写「工具表达不了的才手改」）；随后用 `edit` 把某元素的子表搬到其父之前，再调一次 `archi_element` 改该元素的 `summary`
- **THEN** 本条把边界表述为「批量整理 / 同层手排序」并写明子树连续与关系组连续由写面维持（「重排文件」不在其中）；子先于父不产生结构诊断（`orphanHierarchy` 判「父是否声明」，不判顺序）；`archi_element` 的响应 `reordered.moved ≥ 1`，块回到父的子树区间内

### Requirement: 架构会话的系统知识注入

`source = "archi"` 会话与 **change 臂的两写权相位** SHALL 注入同一段架构会话知识到 `SessionInjections.preamble`，内容 SHALL 覆盖以下**五段**（本变更是骨架的第三次改写：三资产分工与设计顺序取代语法类知识，散文只留不可脚本化的判断面）：

- **三资产分工（C）**：模型 TOML 答「谁在谁的里面、谁依赖谁」（拓扑）；`<叶段>.schema.json`（与文档同目录）答「跨过这条边时接触面长什么样」（接口，**唯一凭据**）；`<叶段>.md`（同目录）答「为什么这样切、有什么坑、什么不归它管」（理由）。**过度设计 = 回答自己不该回答的那个问题**：文档不列接口、schema 不写理由、TOML 不造空元素。**文档是产品面**——架构页「文档」段直接渲染给用户读，人 + AI 共读；接口由写闸校验、人经 UI 阅读。
- **设计顺序（B/C）**：① 定位现状（`archi_search` 发现 → `archi_module` 一次取全邻居 / 三份资产 / 声明位置）；② 画拓扑（`archi_element` / `archi_relation`）——**先边界、后接触面、最后写理由**；③ 补两份资产（`archi_api` 的 `bootstrap` 取双草稿 → 接口用 `write` / `edit` 落 `.schema.json`（坏文本零落盘）→ 理由落 `.md`，段落按内容自定）。
- **三条不可越界（C）**：① **文档不列接口**——导出名 / 签名 / 类型的唯一持有者是 schema；历史 `api-*` fence 是遗留内容（读面不读、清理中），MUST NOT 新增；② **schema 不写理由**——`x-function.description` 只写调用方要知道的语义（做什么 / 边界 / 为什么），MUST NOT 写实现细节、MUST NOT 复述签名；`x-` 面封闭为 `x-function` / `x-class` / `x-common` / `x-constructor` / `x-static` **五项**（逐名列出，MUST NOT 写成「见 spec」），MUST NOT 自造新前缀键；③ **TOML 不造空元素**——每个元素至少一条接触面（`path` / 边 / 导出）。
- **工具面（A/B，指路）**：字段面、键形、缺省、值域、祖先与计数全部由工具承担——MUST NOT 凭记忆手拼 TOML。读五件各一句「何时用」（`archi_search` 发现 / `archi_module` 定位与三资产 / `archi_drift` 漂移读数 / `archi_api` 接口三动作 / `archi_scaffold` 字段词典）；写两件（`archi_element` / `archi_relation`，参数面即字段清单、坏文本零落盘）；**手改出口（永久内容）**：工具表达不了的才手改 TOML（骨架保持这一简写；**适用边界由本能力裁定** = 批量整理 / 同层手排序——子树连续与关系组连续由写面自动维持，手改拆开的子树会在下一次工具写时被合回，故「重排文件」不在边界内），改完必须 `archi_module` 自查——坏模型由读取面报错（结构诊断带 `file:line`）；生成物过期用既有命令重生成、MUST NOT 手改。自检纪律：改完 `archi_drift` 非空即逐条修、`archi_api`（`check`）查两轴。契约资产 `config.json` 管 `ignore` / `exclude`，非法由 `archi_drift` 点名。
- **留给判断 + 兼容（C）**：`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——没有机械答案；**纯分组容器用 `composite`**；**接口面留白（无对外约定）就写一个空 schema（`$defs {}` + `properties {}`），不要编造导出**；旧元素可能仍把接口写在 `.md` 的 `api-*` fence 里——读面不读，接口一律写 `.schema.json`，MUST NOT 往 `.md` 新增 fence。

两条工程约束与下列「值域不进知识文本」纪律逐字沿用：

- `gate` / `api` / `kind` 的值域与缺省档 SHALL 由工具面承载（参数 `description` 与 `archi_scaffold` 词典），骨架 MUST NOT 再写值域行；键形与字段形态 MUST NOT 进骨架；结构诊断全量词表 MUST NOT 进知识文本（`orphanHierarchy` / `dependsOnAncestor` 亦不在骨架，祖先缺失由写工具 `Err`、越界依赖由 `archi_drift` 承载）。**文件序不进骨架**：子树连续与关系组连续是写面默认行为，骨架既不解释也不要求 agent 记得——骨架的手改出口句保持简写（「工具表达不了的才手改」），适用边界的裁定（批量整理 / 同层手排序）与「拆开会在下次写时合回」留在本节与工具描述，**不进骨架**。
- `entry` / `doc` / `apis` MUST NOT 进任何值域词表（开放集）；判据长表 MUST NOT 复制进知识文本。
- **协议词汇的枚举 MUST 逐名进骨架，判据面的事实 MUST NOT 进骨架**：`x-` 面五项是**协议词汇**（写 `.schema.json` 时要用到的名字），枚举本身 SHALL 在骨架内逐名列出；而「每个 `x-class` 至少一条构造通路」是跨元素判据面的事实（判定面在后续 change），MUST NOT 进骨架——两者的区别是「写的时候要用的词」vs「写完才判得出的事」。
- **体量锚（本次显式豁免抬锚）**：骨架 SHALL ≤ **1831 B**、全文 SHALL ≤ **2031 B**，实测值 SHALL 被冻结断言（测量口径沿用 `lf_bytes(trim_end)`——LF 归一、去尾随换行）。相对基线的四数（骨架 1777 B / 全文 1966 B / 门限 1800 / 2000）本变更按 `x-` 面枚举增量 **+31 B** 同批改为 **1808 / 1997 / 1831 / 2031**（两处精确 `assert_eq!` + 两处 `<=` 门限），余量仍为 23 B / 34 B。豁免是**逐次**的、MUST NOT 视为门限作废：往里加东西前仍应先问「这条能不能由工具面 / 参数 `description` 承载」；超限 SHALL 压缩同段冗余表述，MUST NOT 删除三资产分工 / 设计顺序 / 手改出口 / 留给判断四段任一，MUST NOT 借抬锚恢复语法教学（值域行 / 字段形态 / 诊断词表三类仍在禁令内）。
- **尽力增强、不炸 run**：知识拼装失败 SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 五段结构在骨架内可指认

- **WHEN** 审查骨架 md 的分段
- **THEN** 恰含五段语义：三资产分工（含「文档是产品面」）、设计顺序（三步 + 「先边界、后接触面、最后理由」）、三条不可越界（文档不列接口 / schema 不写理由 / TOML 不造空元素）、工具面（读五件的「何时用」+ 写两件的「参数面即字段清单」+ 手改出口一句 + 自检纪律）、留给判断 + 兼容；MUST NOT 出现语法教学段

#### Scenario: 语法知识零残留

- **WHEN** 对骨架做机械检查
- **THEN** 零示例 TOML 围栏（` ``` ` 计数为 0）、零 `[element.` 前缀、零字段值域行（`off` / `warn` / `strict` / `declared` / `none` 不同时作为值域枚举出现）、零结构诊断词（`orphanHierarchy` / `dependsOnAncestor` / `invalidValue` / `unknownField` / `missingField` / `syntaxError` 全零命中）、零 `entry` 三档长表

#### Scenario: 三条不可越界逐条在场

- **WHEN** 对骨架做关键词断言
- **THEN** 「文档不列接口」/「MUST NOT 复述签名」/「TOML 不造空元素」三条各至少一处；`x-` 面封闭**五项逐名**在场（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`），且该行内 MUST NOT 出现「见 spec」一类指路；「空 schema」与「不要编造导出」在场；「历史 fence 读面不读、MUST NOT 新增」在场

#### Scenario: 五项枚举有机械守卫

- **WHEN** 从骨架的 `x-` 面枚举里删掉一项（如删 `x-constructor`）后跑知识语料测试
- **THEN** 测试红（`指令表收窄且新行集可判` 的 needle 集逐名核对两新键的带反引号原样文本）；恢复该项后绿——枚举本身不得只靠人眼守

#### Scenario: 手改出口是永久内容

- **WHEN** 审查骨架「工具面」段的最后一句
- **THEN** 该句含三要素：①「工具表达不了的才手改」的适用边界；②「改完必须自查」（点名 `archi_module`）；③ 坏模型由读取面报错（带 `file:line`）；MUST NOT 被表述为过渡措施或待删项；且句内不含「重排」一类已被写面接管的动作

#### Scenario: 值域迁到工具面而非丢弃

- **WHEN** 分别审查骨架与两工具的 `parameters` / `archi_scaffold` 草稿
- **THEN** 骨架零值域行；`gate` / `api` 的合法值与缺省档在 `archi_element` 的对应 `description` 内逐值在场（由 `ALL` 派生）且在草稿词典各有一行；`kind` 四值语义在枚举描述与草稿词典内逐值在场——「散文删掉的」都能在工具面查到

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 七 archi 工具不变），MUST NOT 抛装配错误；收口前自检照常装配（`before_done` 档位与知识解耦；T1 / T2 皆假时仍零 notice）

#### Scenario: 自检纪律与可见性在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完可调 `archi_drift` 取读数并按条修」「写工具坏了/手改写坏 → 结构诊断经 `archi_module` 带 `file:line`」「契约面可调 `archi_api` 查两轴」「生成物过期用本仓的生成命令重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」六条；且知识文本内零 `<>` 占位符

#### Scenario: 体量与目录方位实测

- **WHEN** 取 `knowledge()` 输出与骨架字节
- **THEN** 骨架 ≤ 1831 B、全文 ≤ 2031 B 且实测值（**1808 B / 1997 B**）被冻结断言；四数与三处文案（模块头门限表述 / 测试 doc 的抬锚口径 / 两条失败文案）同批更新；程序追加段含模型声明目录与契约资产目录（文档 + 接口两条同目录资产）两处方位，零契约索引串，骨架零目录字面量

#### Scenario: 判据面事实不进骨架

- **WHEN** 审查骨架全文
- **THEN** 零「构造通路」/「至少一条」一类跨元素判据表述（它属 `desktop-archi-api` 与后续 change 的判据面）；零「object 等同 JSON 对象」的复述（写闸已强制，骨架再加只是复述）；抬锚增量恰为 `x-` 面枚举那一行的 +31 B（全文与骨架的增量相等）

### Requirement: 检索面纪律与迁移边界

`archi_search` 与 `archi_module` 两个只读工具面 SHALL 守住三条迁移边界，使将来的「模块入口文档逐步替代 `openspec/specs/`」迁移不被本期实现反向锁死：

| # | 边界 | 理由 |
|---|---|---|
| 1 | 检索语料不读 `openspec/specs/` | 一旦纳入，用户会开始写「给 archi 看的 spec」，迁移被反向锁死 |
| 2 | 检索与工具响应不返回文档正文 | 正文按路径自取（agent 用 `grep` / `read`，前端用 `read_file`）；返回正文会让工具响应体随文档膨胀且与单结果字节上限冲突 |
| 3 | 不提供跨元素的文档反向索引 | 反查「这份文档描述哪些模块」不在本期，避免用户开始写「给 archi 看的文档」 |

- **语料面封闭（本变更换面）**：检索语料 SHALL 恰为「`path` 非空元素的在盘资产」两条固定规则——在盘 `<叶段>.md`（文档）与在盘 `<叶段>.schema.json`（接口）；MUST NOT 新增第三条来源（不出 `specs`、不出 `decisions`、不扫元素 `path` 目录内的散落 md、不扫 `<archi_dir>` 其它子树、MUST NOT 读模型 `doc` 字段——`carrier_path` 已退役，载体派生不再存在）。两条规则都与 `api` 状态无关。
- **正文不返回（结构性成立）**：`ArchiSearchResult.doc` 只承载**路径字符串**（文档与接口各一条规则），模型 `doc` 字段只承载一条路径串——字段里没有 `title` / 摘要 / 正文 / 行区间的容身之处，故「不返回正文」不靠纪律而由形状保证。
- **反向索引零预铺**：MUST NOT 引入「文档 → 元素集合」的索引、缓存或 IPC；`grep` 与文件树是反向查找的既有面。
- **零 `specs` 读取路径**：产品源码 MUST NOT 出现 `openspec` 路径拼接（路径一律经 `foundation::layout`），本能力 MUST NOT 引入 `spec ↔ element` 映射。
- **「模块入口文档替代 specs」的迁移**（含 acceptance 可断言结构与归档 `SpecSync` 增量合并语义的对应物缺失问题）需独立变更裁决，本期零预铺。

#### Scenario: 检索不命中 specs 内容

- **WHEN** `query = "archi"` 且 `openspec/specs/` 内某文件路径含该词、某入口文档亦含
- **THEN** 命中项只来自模型元素与资产路径（文档 / 接口两类）；`openspec/specs/` 下任何路径与内容均不出现

#### Scenario: 正文零承载位

- **WHEN** 审查 `ArchiModuleDetail.doc` 与 `ArchiSearchResult.doc` 的字段面
- **THEN** 前者是单一路径串或 `null`、后者是路径串数组；两者都没有 `title` / 摘要 / 正文 / 行区间字段，也无「内容」一类可选位

#### Scenario: 关联来源封闭

- **WHEN** 审查文档检索语料来源清单
- **THEN** 语料只取两条规则（在盘 `<叶段>.md` ∪ 在盘 `<叶段>.schema.json`），与 `api` 状态、`doc` 字段均无关；`decisions/**` 与元素 `path` 目录内的散落 md 都不在语料内，MUST NOT 出现来源分支

### Requirement: 知识分层与工具面同批移动

架构会话知识文本的内容面 SHALL 按**可判定性 × 时机**两轴分四档，并把每一句的落档写进本能力（分层判据单点，MUST NOT 在别处另立一份分类）：

| 档 | 判据 | 承载 |
|---|---|---|
| **A 工具承载** | 机器能算 ∧ 动作后才需要 | 工具响应 / `Err`（MUST NOT 在知识文本里复述读数） |
| **B 上下文·一行指针** | 机器能算 ∧ 动作前就需要 | 知识文本**一行**，内容是「何时查哪个工具」或一个派生串 |
| **C 上下文·散文纪律** | 不可判定（语义 / 意图） | 知识文本散文 |
| **D 模板工具** | 不可判定 ∧ 动作后才需要（形态） | 模板工具输出（`archi_scaffold`） |

- **A 类句子 MUST 携带工具指路且工具面确有该字段**：知识文本里每条「来自 A 档」的纪律 SHALL 点名承载它的工具（`archi_module` / `archi_drift` / `archi_api` / `archi_scaffold` / `archi_element` / `archi_relation`），且该工具的响应或 `Err` 面 SHALL 确有对应字段 / 判据。任一方向缺失（句子里点了不存在的字段、工具加了字段但句子没指路）即违约——由机械断言表守（一行为「纪律句关键词 → 工具名 → 工具面字段」）。
- **分层第一判据（本变更新增）：工具可承载的知识 MUST NOT 留在散文**。某条知识若「机器能在动作前算出」且「写面是结构化的」，它 MUST 落在工具的参数 `description` / 响应 / `Err` / 词典（A / D 档），**MUST NOT** 以散文复述——散文里再写一份即「同一事实两份持有者」。语法知识的 C 档身份只允许残留于**手改面**（见补充规则①）。
- **四条补充规则 SHALL 逐条在案**：
  1. **形态错误不静默，失败面按入口分**：未引号点分表头 `[element.App.Auth]` 归 `unknownField`、**整份源解析失败**（全或无）；`metadata` 子表与内联表**等价**、零诊断。两者都有**行为**测试钉住（`parse/toml_test.rs` 的 `未引号点分表头被拒且不与合法形态合并` / `metadata子表与内联表等价` / `metadata子表成为唯一声明即缺kind`）。**归 C 档的依据**：手改面仍是自由文本 `write` / `edit`，结构诊断只在写盘后可得（动作前不可判定）；**结构化写面（`archi_element` / `archi_relation`）对工具通路消除了该前提**——工具渲染不可能产出未引号点分与点分嵌套，故这条知识在工具面是 A/D 档（由渲染与 parser 诊断承载），只在手改面残留为 C 档一句。MUST NOT 再表述为「会被静默读成另一个元素」或「`deny_unknown_fields` 不触发」——后者已被行为测试否证，不是任何档位的依据。
  2. **写入定位必须留一行**：新元素的模型落点（默认父元素所在文件、可由 `file` 覆盖）与手写契约的落点（契约资产目录）都在动作前需要，且新元素的契约三路径为 `null`（`api.contractPath` 指不到不存在的东西）⇒ 归 B 档，各留一行。落点的**推演规则**归工具（`archi_element` 的 `file` 参数描述），骨架只留「工具负责落点」的指路。
  3. **`kind` 的「选哪个」不可脚本化，「四值语义」是词表**：判断面（容器 = `domain` / crate、目录与单文件组件 = `module` / 包根 = `package`）归 C 档；四值语义是封闭词表 ⇒ 归工具参数 `description` 与 `archi_scaffold` 词典（MUST NOT 在散文里再抄一份）。
  4. **结构性事实归响应，不归散文**：祖先先声明（`orphanHierarchy`）与计数（`（N 元素 / M 关系）`）在写工具面分别是 `Err` 与响应 `counts`（工具重算并回填表头），故骨架 MUST NOT 写「子元素须先声明祖先」与「计数以工具读数为准并同批更新表头」两段——它们已是 A 档。
- **移动分层的原子性**：把某句从散文改判为 A/D 档时，**同批**必须落地「工具面确有该字段 / 模板 / 判据」与「散文句替换为指路句」两半——只删不加（知识缺失）与只加不删（知识重复）同样违约。
- **本变更的移动清单**（机械断言逐项在案）：键形与字段形态（438 + 827 B）→ 写工具渲染 + parser `Err` 转述 + `archi_scaffold` 词典；`kind` 四值语义（454 B）→ `ElementKind` 枚举描述 + 词典；层级与读数（330 B）→ 写工具 `Err`（祖先）/ 响应 `counts` + 表头回填（计数）/ `archi_drift` 报错点名（`config.json`）；保留面 = 职责、工具面指路、手改出口、留给判断（边界与自检 / 写面两段压成四句）。

#### Scenario: A 类句子逐句有工具可指

- **WHEN** 对知识文本的 A 档纪律句逐条取「关键词 → 工具名 → 工具面字段」三列核对
- **THEN** 每条纪律句都点到承载它的工具名，且该工具的响应 / `Err` 确有对应字段（`hasPath` / `resolvedEntry` / `fileHash` / 结构诊断 `kind` + `file:line` / `counts` / 七条 `warnings`）；任一列为空即测试红

#### Scenario: 删掉的散文都能在工具面查到

- **WHEN** 逐条取本变更移动清单（键形 / 字段形态 / `kind` 四值 / 祖先 / 计数 / `config.json`）在骨架里确认已无散文，再到工具面确认承载者在场
- **THEN** 六项全部两侧成立：骨架零残留、工具面有对应字段或判据（`archi_element` 参数描述 / `archi_scaffold` 草稿行 / `EditError` 判据 / 响应 `counts` / `archi_drift` 报错）；任一侧缺失即红

#### Scenario: 只删不加即违约

- **WHEN** 从知识文本删掉字段形态段而未同时落地写工具的参数面与渲染自证（或反之：加了写工具但散文仍抄一份字段纪律）
- **THEN** 视为分层未完成——机械断言表对「移动清单」逐项要求两侧同时在场（工具面 + 指路句），单侧缺失即红

#### Scenario: 手改面的 C 档残留可辨

- **WHEN** 审查骨架里与形态有关的那句话
- **THEN** 只剩「工具表达不了的才手改、改完自查、坏模型由读取面报错（`file:line`）」三要素；MUST NOT 出现「子表 / 点分嵌套会被读成另一个元素」的假因果，MUST NOT 出现「`metadata` 必须内联表」的禁令，MUST NOT 出现键形示例与值域行

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

### Requirement: archi 写工具面（元素 upsert / delete 与关系 upsert / delete）

SDK 引擎 SHALL 提供两个专用**写**工具（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验）：`archi_element`（元素 upsert / delete，可内联出边）与 `archi_relation`（单条关系 upsert / delete）。执行体 SHALL 在 `sdk/archi_tool.rs` 的 `ArchiContext`（第六、七臂），与五只读工具共用同一 per-turn 上下文与同一 `cap_result` 上限单点：

```jsonc
{ "name": "archi_element",
  "action":   "必填，封闭二值：upsert / delete",
  "element":  "必填，元素完整 FQN",
  "kind":     "enum ← ElementKind::ALL；新建必填，已存在时省略（给了即 ElementAlreadyExists）",
  "gate":     "enum ← GateTier::ALL", "api": "enum ← ApiStatus::ALL",
  "path":     "string[]（不传 = 不动；空数组 = 清键）",
  "entry":    "string | null（null = 清键）", "doc": "string | null",
  "summary":  "string | null", "metadata": "object<string,string>（空对象 = 清键）",
  "relations": "[{ to（必填）, desc?, apis?, action? }]（缺省 action = upsert，另一值 delete）",
  "expect":   "sha256:<hex>（可选：目标文件当前内容哈希；省略 = 不校验）",
  "file":     "模型声明目录一级 *.toml（新建落点覆盖；既有元素只允许同值）",
  "create":   "bool（新建文件需显式许可；缺省 false）",
  "cascade":  "bool（仅 action = delete；缺省 false）" }

{ "name": "archi_relation",
  "action": "必填，upsert / delete", "from": "必填", "to": "必填",
  "desc":   "string | null", "apis": "string[]", "expect": "sha256:<hex>（可选）" }
```

- **平铺 schema、逐字段 `description`**：两工具的 `parameters` SHALL 为平铺对象（MUST NOT 用 `oneOf`，MUST NOT 把元素面与关系面合成一个工具——两面字段完全不相交，合一只能靠大量互斥可选位）；每个字段的 `description` SHALL 写明缺省与陷阱（`path` 的基数随 `kind`、`metadata` 值须为字符串、`relations[].action` 缺省 `upsert`、`cascade` 仅 delete、`doc` 仅 `api = none` 可写、`file` 缺省落父元素文件）。
- **值集派生**：`action`（二值）/ `kind` / `gate` / `api` / `relations[].action` 的合法值 SHALL 由封闭词表派生（`ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / 两处二值词表），MUST NOT 手抄字面量。
- **下发面**：两工具 SHALL 只在 `(proposal, executor)` / `(dev-design, executor)` 与 `source = "archi"` 臂下发（矩阵见「archi 工具面注入与 (phase, role) 矩阵」）；其余相位与来源 MUST NOT 下发，调用由成员校验拒绝。
- **档位**：两者 SHALL 归 `ToolTier::Write`（`default` 档拒绝、`acceptEdits` / `bypassPermissions` 放行）——与 `write` / `edit` 同档；MUST NOT 为两者单开档位。
- **写面语义全在 `desktop-archi-model`**：三态（省略 = 不动 / `null` = 清空 / 值 = 设值）、内联出边的四条约束、落点四情形、`cascade` 的**单文件闭包**、`expect` 乐观锁与 `hash` 回传、**写入序不变量（子树连续 / 关系组连续 / 新边落组尾）**，SHALL 由 `archi::edit` 的写回原语族承载；本工具 MUST NOT 自拼 TOML、MUST NOT 重实现任一判据（含 `path` 形态、`kind × path` 基数、`apis` 值面），MUST NOT 自行计算搬迁块。执行体 SHALL 只做「入参 → `ElementSpec` / 关系三态 → `plan_*` → 落盘 → 响应装配」。
- **落盘单点**：执行体 SHALL 以 `write_atomic` 单点落盘（临时文件 + 重命名，与壳层命令面共用同一实现），MUST NOT 复制第二份「临时文件 + rename」逻辑，MUST NOT 在 `archi` crate 内出现写盘调用。
- **写保护射程**：解析出的目标文件 SHALL 过**同一** `ProtectRules`（内置 → 会话 → 用户三段序、首命中即拒、拒串沿用既有族），判定细则归 `desktop-write-protection`「会话级写入禁区（相位锁）」；MUST NOT 另立规则集或第二份文案。
- **成员校验与上下文**：两工具 SHALL 走既有成员校验（调用名 ∉ 本轮 definitions ⇒ `tool_unavailable` + `is_error` 回灌，run 不中断）；archi 上下文铸造判据 SHALL 由「名字清单含任一 archi 工具名」表达（七名逐一在场，MUST NOT 漏掉新增两名）。
- **工具描述面（本变更由三件事扩为四件事）**：两工具的描述 SHALL 说明——① 写盘是原子的且**坏文本不可产出**（自证链，零落盘）；② `expect` 的用法与 `hash` 的闭环（「上一轮响应的 `hash` 或 `archi_module` 的 `fileHash` 可直接作 `expect`」）；③ `cascade` 只作用于**同一模型文件**内的子树与边；④ **写入会把目标文件归一到规范序**（子树连续 + 关系组连续，见 `desktop-archi-model`「写入序不变量」）——手改拆开的子树会在下一次工具写时被合回，顺带搬家的块经响应 `reordered` 报出（`moved` 真值、`blocks` 有界）。描述 MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 词典与参数 `description`），MUST NOT 教学规范序算法。

#### Scenario: 两写工具在册且档位与下发面对齐

- **WHEN** 审查挂载清单与三个写权装配面的 definitions
- **THEN** `archi_element` / `archi_relation` 均在册且 `ToolTier::Write` / `ToolExec::Archi`；`(proposal, executor)` / `(dev-design, executor)` / `source = "archi"` 三处 definitions 含两者，`RunStarted.tools` 逐名一致；`(implement, executor)` / `(dev-design, evaluator)` / `explore` / `debug` 零下发且调用被成员校验拒绝（`default` 档下写权相位调用亦被档位拒绝——与 `write` 同款）
- **AND** 五只读工具的档位与下发面逐字不变

#### Scenario: 元素 upsert 走通工具面

- **WHEN** 调 `archi_element { action: "upsert", element: "Backend.Core.Billing", kind: "module", path: ["./src-tauri/crates/core/billing/src/"], summary: "计费域" }`
- **THEN** 模型文件新增该元素表（字段按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序），响应含 `created = true`、`changed` 覆盖给予的字段、`file` 落父元素所在文件、`hash` 为新值、`reordered.moved ≥ 1`（该元素的父已有后续兄弟，故新表被规范到父的子树区间内）；再以同值调一次 ⇒ `created = false`、`changed = []`、`reordered.moved = 0`、盘上字节零变化

#### Scenario: 内联出边与独立关系工具等价

- **WHEN** 对同一 `(from, to)` 分别用 `archi_element { relations: [{ to: X, desc: "…" }] }` 与 `archi_relation { action: "upsert", from, to, desc: "…" }`
- **THEN** 两者产出的模型文本**逐字节相同**（同一 `apply_*`），两响应的 `reordered` 也逐字段相同（元素面首元素 FQN 与关系面 `from` FQN 各自口径）；`relations` 四计数与入参数组长度守恒；内联边已声明在**别的**模型文件时前者 `Err` 点名该文件并指向 `archi_relation`，后者照常改写

#### Scenario: 级联只作用于同一模型文件

- **WHEN** 对含子元素与出入边、且整棵子树全在同一文件内的元素调 `archi_element { action: "delete", cascade: true }`；另对一个子元素或某条边声明在**另一**模型文件的元素作同样调用
- **THEN** 前者删净子树与相关边、该文件表头计数随动、其余内容逐字保留；后者 `Err` 点名跨向的文件与阻塞项、**零落盘**（盘上字节零变化）

#### Scenario: 值域与上限在入参面即拒

- **WHEN** 分别以 `action: "patch"` / `kind: "service"` / `gate: "high"` / `relations: [{ to: X, action: "keep" }]` / `cascade: true` 配 `action: "upsert"` / 超过八行上限的任一字段调用两工具
- **THEN** 一律 `Err` 且**零落盘**；值域错误点名该参数的合法值（由 `ALL` 派生），上限错误点名字段与上限值；`default` 档下更早由档位拒绝

#### Scenario: 描述面四件事齐备且不教算法

- **WHEN** 取两工具的 `description`
- **THEN** 各含「坏文本不可产出」「`expect` / `hash` 闭环」「`cascade` 同一模型文件内」「写入归一到规范序 + `reordered`」四组要点；零字段形态示例（`kind = ` / `[[relation]]` / `path = [` / `metadata = {`）、零规范序算法描述（不含「DFS」「`position`」「`ArrayOfTables`」一类实现词）

### Requirement: 写工具的响应体与错误面契约

两个写工具的输出 SHALL 走**三通道**（`Err` 阻塞零落盘 / `warnings` 已落盘有未阻断事实 / `changed` 幂等凭据），MUST NOT 增设 `hints` 一类第二通道（拆开后模型可能只看 message 不看 hint）：

```jsonc
{ "warnings": [ … ],                                   // 恒在场；空数组 = 无异常
  "action": "upsert", "element": "Backend.Core.Billing",
  "file": ".dev-team/architecture/graph/model/02-backend-core.toml",
  "hash": "sha256:9c2e…",                              // 写后新值 = 下一次 expect
  "created": true, "changed": ["kind", "summary"],
  "relations": { "created": 1, "updated": 0, "deleted": 1, "unchanged": 2 },
  "counts": { "elements": 10, "relations": 10 },       // 本文件（重解析结果）
  "model":  { "elements": 56, "relations": 129 },      // 全模型（前值 ± 增量）
  "reordered": { "moved": 3, "blocks": ["Desktop.Views.Changes.Flow",   // 搬迁块（cap 8）
                 "Desktop.Views.Changes.Renderers", "Desktop.Views.Changes.Agent"] } }
```

- **`warnings` 恒在场**：`archi_element` 的判据与降噪不变式归 `desktop-archi-model`「写面反馈判据」；本工具只负责把它序列化进响应（`kind` / `severity` / `message` 三字段，`kind` 与 `severity` MUST NOT 在此另作裁剪或重定档）。`archi_relation` 的 `warnings` SHALL 恒为空数组。**写入序规范化 MUST NOT 新增 warning 条目**（它不是判据）。
- **`changed`**：`action = upsert` 时 SHALL 为**参数面字段名的子集**（实际变化的字段；空数组 = 字段零变化）；`action = delete` 时为空数组。`archi_relation` 的 `changed` SHALL 为 `desc` / `apis` 的子集。**`changed` 空而盘上字节变化是正当形态**（首次把含历史错位的文件规范化），判据取 `changed` 空 **且** `reordered.moved = 0`。
- **`reordered` 恒在场（本变更新增）**：`{ moved, blocks }`——`moved` 为本次搬迁的**块数真值**（永不截断），`blocks` 为各块首元素 FQN（元素面）/ `from` FQN（关系面），按文件序、**cap 8 项**（超出即截断，MUST NOT 附「…等 N 项」文本）；零值时也写 `{"moved":0,"blocks":[]}`（MUST NOT 省略字段）。语义与 cap 归 `desktop-archi-model`「模型写回原语族」的产出形状条，本工具 MUST NOT 在执行体内另算。
- **`archi_relation` 的响应是薄形**：`{ warnings（恒空数组）, action, from, to, file, hash, created, changed, reordered }`——**键面恰九项**；MUST NOT 携带 `relations` 四计数与 `counts` / `model`（一次写只动一条边，计数另经 `archi_drift` 取，不为一行数据付全量重解析）。`reordered` 照带（规范化已在 `apply_*` 内完成，零额外解析）。
- **`relations` 四计数**：`created + updated + deleted + unchanged` SHALL 恒等于入参 `relations` 数组长度（可直接断言的守恒式）；未给 `relations` 时四项皆 `0`（字段不省略）。
- **`counts` / `model`**：`counts` SHALL 等于写后该文件的重解析计数（与表头括号数字同源），`model` SHALL 等于全模型计数（前值 ± 本次增量），两者 MUST NOT 独立于重解析结果另算。
- **`hash`**：SHALL 与 `drift::file_hash` 同一函数产出、同一 `sha256:` 前缀——与 `expect` 可互喂（连续两次写同一文件零额外读）。首次规范化会让 `hash` 随搬迁变化，这是正当结果（`reordered` 已解释变化来源）。
- **响应不含元素 / 关系全文**（要看现状用 `archi_module`）；不含协议正文。
- **成功响应永不触达截断**：`warnings` ≤ 7 条 × ≤200 B + `reordered.blocks` ≤ 8 × ≤256 B + 其余计数与短串 ⇒ 结构上界 ≈ 4 KB ≪ `MAX_RESULT_BYTES`（50 000）。故**不设键序断言**——`serde_json` 未开 `preserve_order`，把 `warnings` 写在对象首位不是机制，防御性排布亦不构成契约；真正需要体量纪律的是 `Err`（见下）。

**错误面分层**（判据一律来自既有单点，MUST NOT 另立词汇）：

| 层 | 场景 | 承载 |
|---|---|---|
| E1 入参 | `action` / `element` / `relations[].to` 缺失或空、四处封闭值域、**八行上限** | 执行体显式检查（`ok_or_else` 形态），文案点名参数与合法值 |
| E2 前置读数 | 模型已有结构诊断 | `dto::graph_diagnostics_error` 的**全量形**（MUST NOT 为写工具另写第二份诊断渲染） |
| E3 语义 | 祖先缺失 / 端点不存在 / 删除有引用 / `kind × path` 冲突 / 批量四条 / 跨文件内联边 | `EditError`（`EditErrorKind` + 事实段） |
| E4 落点 | `file` 不在模型目录一级 / 无 `create` 而目标文件缺席 / 目录缺席 | `EditError` |
| E5 删除 | 元素 / 关系（含内联 `delete`）不存在 | `EditError`（附候选或现有出边） |
| E6 自证 | 渲染后仍有诊断 / 回读不等 | `EditError` + **零落盘** + 「这是工具缺陷」语义 |
| E7 并发 | `expect` 与当前文件哈希不等 | `EditError`（expect / 当前双值 + 两条出路） |

- **四条硬纪律**：① 第一行自足（`Err` 含 `重试：` / `下一步：` / `改用 <工具名>` 之一，或含可复制 JSON 参数片段）；② 字面与词表同源（`gate` 不写「档位」、诊断词不译中文）；③ 不猜（给候选不代选）；④ 一次报全（MUST NOT 首错短路）。**序违例不在任何一层**——规范化没有 `Err` 臂，E1–E7 七层与 `EditErrorKind` 的 23 值面逐字不变。
- **入参上限是软提示 + 硬闸两段**：八行上限 SHALL 同时进 schema（模型侧提示：FQN ≤256 / `path` ≤32 项 × ≤1024 / `metadata` ≤32 项（键 ≤64、值 ≤512）/ `relations` ≤64 / `relations[].apis` ≤32 × ≤128 / `summary`·`desc` ≤512 / `entry`·`doc` ≤1024 / `file` ≤256）与执行体 E1 检查；两处数字 SHALL 取自**同一组常量**（MUST NOT 各写一份）。

#### Scenario: 三通道可辨

- **WHEN** 分别以「写坏模型」「只改 `summary` 的合法 upsert」「与现状同值的 upsert」「指向不存在关系的内联 `delete`」调用 `archi_element`
- **THEN** 依次为：`Err` 携诊断清单（零落盘）/ `warnings = []` 且 `changed = ["summary"]` / 幂等（`changed = []` ∧ `reordered.moved = 0`、盘上字节零变化）/ `Err` 列出同 `from` 的现有出边（零落盘）

#### Scenario: 降噪不变式在工具面成立

- **WHEN** 对 `api = declared` 且契约 md 缺失的元素分别调「只改 `gate`」与「改 `path`」两次 upsert
- **THEN** 前者 `warnings = []`；后者 `warnings` 含 `path` 诱发的条目；两次的 `warnings[].kind` 恒为既有两词汇集的 `as_str` 之一（`ArchiDriftKind` / `ArchiApiFindingKind`），severity 与判据面单点同值；规范化搬迁块不产任何 warning

#### Scenario: 四计数守恒与幂等

- **WHEN** 一次 upsert 传入三条 `relations`（一条新建、一条改 `desc`、一条 `delete`），再一次以完全相同的参数重放
- **THEN** 首次 `relations = { created: 1, updated: 1, deleted: 1, unchanged: 0 }`（四者之和 = 3）且 `changed` 非空；重放时该边已不存在或值已相同 ⇒ 逐项按实计（`unchanged` 吸收），四者之和仍 = 3，`changed = []`，盘上字节零变化

#### Scenario: 成功响应体量与计数可对拍

- **WHEN** 取任一成功响应并序列化，随后对该文件重解析、对全模型取 `archi_drift`
- **THEN** 响应长度 < `MAX_RESULT_BYTES`；`counts` 与文件重解析计数及表头括号数字三者相等；`model` 与 `archi_drift` 报告的 `model.elements` / `model.relations` 逐值相等

#### Scenario: Err 第一行自足且值域点名

- **WHEN** 以非法 `action` 与过期 `expect` 分别调用
- **THEN** 前者文案点名合法值（`upsert` / `delete`）与参数名；后者给 expect / 当前双值并含两条出路之一（`重试：` 或「省略 expect 直接覆盖」）；两例均零落盘

#### Scenario: 搬迁块在两写工具同形

- **WHEN** 新建一个父元素已有后续兄弟的元素；随后对一条 `from` 已有组的 `(from, to)` 调 `archi_relation { action: "upsert" }`；再各重放一次
- **THEN** 元素面响应 `reordered.moved ≥ 1` 且 `blocks` 含该新元素 FQN；关系面响应 `reordered.moved = 1`、`blocks = [<from>]`（新条目落组尾）；两次重放均 `moved = 0` 且盘上字节零变化；`archi_relation` 响应的键面恰九项（含 `reordered`，仍无 `counts` / `model` / `relations`）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/runner.rs` | 注入词表 | `SessionInjections` 六位（`preamble` / `tools` / `finish` / `ask` / `protect` / `before_done`）；`before_done: Option<BeforeDoneSpec>` 为收口前钩子档位（serde additive，`None` = 不装配；`BeforeDoneSpec::ArchiPreclose` 由 archi 臂唯一置位）；`SessionToolset` / `toolset` 已退役（产品源码零命中，`sdk/runner_test.rs` 有零命中断言），本表不再登记该枚举 |
| `crates/infra/agent/src/compose.rs` | 注入装配 | `assemble_injections`：explore 臂逐字不变 → **archi 臂**（`source == "archi"` 短路命中：`preamble = Some(knowledge())`、`tools = Some(七内建全量 + 五个 archi 工具名)`、`before_done = Some(ArchiPreclose)`，不碰相位解析）→ change 臂（7 行矩阵 + fail-closed，`archi_matrix` 为矩阵单点）→ default（显式构造，非默认值兜底）；`ARCHI_SOURCE` 常量为来源词单点 |
| `crates/infra/agent/src/archi_knowledge.rs` | 骨架 + 派生段拼装（本变更：目录方位串换段） | 注入段 SHALL 由「契约目录：`<archi_api_rel_dir>`」改为「契约资产：`<archi_models_rel_dir>/<分层目录…>/<叶段>.md`（索引 `<archi_graph_rel_dir>/INDEX.toml`）」——`api` 段串零残留；四面词表注入、纯函数零 IO、降级为 `None` 的语义逐字不变。原拼接面：`knowledge() -> Option<String>`：`include_str!("archi-knowledge.md")` + `ElementKind::ALL` / `GateTier::ALL` / `GateTier::default()` / `ApiStatus::ALL` / `DiagnosticKind::ALL` / `foundation::layout` 相对路径派生族；`entry` / `doc` MUST NOT 进派生词表（开放值集）；纯函数零 IO；降级为 `None` 而非 `Err` |
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

### 模块增量：草稿升格字段词典（archi-scaffold-field-dictionary）

主基线既有行逐字沿用；本变更只改词典正文与描述面（执行体、参数面、响应形状、`cap_result` 截断单点、只读档位与注入矩阵逐字不变），受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/archi_tool.rs` | 词典正文单点（本变更改五处注释 / 零改执行体） | `SCAFFOLD_TEMPLATE`：`entry` 行补「单值」；`metadata` 两行补「值 MUST 是字符串（数字 / 布尔等非字符串值被拒）」；`apis` 两行补「逐项非空 / 同名不重复 / `apis = []` 与省略同义（显式空数组被拒）」；`kind` 四值语义、`path` 基数、`doc` 指针纪律逐字不动 |
| `crates/infra/agent/src/sdk/mount.rs` | 工具定义面 | `archi_scaffold_definition().description` 改词典措辞（「字段词典 / 逐字段一行纪律 / 值域 / 关系四键」），零字段名枚举；`kind` 参数 enum 仍 `ElementKind::ALL.map(as_str)` |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | 词典断言面 | 新增三支：字段全集行首对拍（遍历 `archi::parse::ELEMENT_FIELD_NAMES`）/ 值域逐值 + 零诊断词 / `metadata`·`apis` 值面纪律；既有九支逐字保留 |
| `crates/infra/agent/src/sdk/mount_test.rs` | 描述断言面 | `archi_scaffold定义描述要素` 的 needle 换词典措辞，并加反向断言（四枚字段名零命中） |

### Function: `crates/infra/agent/src/sdk/archi_tool.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `scaffold_draft(kind)` | 零改动 | 仍为 `SCAFFOLD_TEMPLATE.replace("__KIND__", kind.as_str())`；值集仍由 `ElementKind::ALL` 派生 |
| `scaffold_payload(input)` | 零改动 | `kind` 可选 / 词汇外 `Err` 点名四值 / `{ "draft": … }` 形状不变 |
| `SCAFFOLD_TEMPLATE` 文本 | 改 | 五处注释：`entry`（+单值）、`metadata`（+值域，拆两行）、`apis`（+三条值面，拆两行）；无新增字段行、无新占位符、无裸 TOML 行 |

### API: 工具面

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_scaffold` 描述 | 改 | 由「新元素 TOML 模板（字段序 / 内联 `metadata` / `path` 必填提示 / 关系模板）」改为词典措辞；MUST NOT 再枚举字段名 |
| `archi_scaffold` 参数 / 响应 | 零改动 | `kind` 可选四值（`ElementKind::ALL` 派生）；`{ draft }` 形状、`is_error` 回灌面、`cap_result` 截断单点不变 |
| 五 archi 工具注入矩阵 | 零改动 | `(phase, role)` 矩阵、`ToolTier::Read`、挂载清单与成员校验不变 |

### CLI / Component: 零变化

| 项 | 变化 | 描述 |
|---|---|---|
| CLI / 前端组件 | 零改动 | 无 IPC / DTO / UI 变化；无新增命令 |
| `archi-knowledge.md` 与相位 prompt | 零改动 | 「取词典」措辞与字段纪律压缩归主篇切片 A（本变更只保证词典先就位） |

### 模块增量：三资产出线与接口三动作（archi-agent-context-review）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/dto.rs` | 投影单点（本变更加两字段 + 四类型） | `ArchiModuleDetail` 增 `assets: ArchiAssetSet` + `reading: String`；新增 `ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset`（4 个出线类型）；`module_detail(root, fqn)` 装配三资产（`topology` 取模型、`interface` 读 schema 键名与 `abstract`、`doc` 读 `##` 段名与行数）；`ArchiApiRef` 四字段名零变化、`ref_projection` 三路径各自独立出线、`status` 改判 schema 在盘 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具执行体 | `archi_module` 透传 `assets` / `reading`（零拼装）；`archi_api` 增 `link` 一支（`paths::ref_relative` + `#/$defs/<name>`，纯计算零读盘写盘）；`bootstrap` 回 `{ schema, doc }`；`SCAFFOLD_TEMPLATE` 的描述句改为「接口草稿用 `archi_api` 的 `bootstrap`」（词典正文不动） |
| `crates/infra/agent/src/sdk/mount.rs` | 工具定义面 | `archi_module_definition()` 五句（三资产一次给全 / 接口唯一凭据 / 文档整读 / `resolvedEntry` + `fileHash` / 两路径语义）；`archi_api_definition()` 四句（双草稿 / `link` / 三类生成物 / 冷启动口径）；`archi_scaffold_definition()` 一句（接口草稿指路）；工具名常量、参数 schema、`ToolTier::Read` / `ToolExec::Archi` 归属与 `build_registry` 项数零变化 |
| `crates/infra/agent/src/compose.rs` | 装配单点 | `archi_matrix` 两格追加 `archi_api`；change 臂在 `model_write_lock(provenance) == None` 时注入 `archi_knowledge::knowledge()`（失败降级 `None`）；archi 臂与 config 臂逐字不变 |
| `crates/infra/agent/src/archi-knowledge.md` | 知识静态骨架 | 五段（三资产分工 / 设计顺序 / 三条不可越界 / 工具面 / 留给判断 + 兼容）；零围栏、零值域行、零诊断词、零 `<>` 占位符；体量锚 1800 / 2000 B |
| `crates/infra/agent/src/archi_knowledge.rs` | 拼装 | 派生段改为「模型目录 + 契约资产（`<叶段>.md` 理由 + `<叶段>.schema.json` 接口，同目录）」；`build` 的占位符 / 空骨架校验与降级语义逐字不变 |
| `crates/infra/agent/src/sdk/policy.rs` | 静态权限表 | `READONLY_TOOLS` 三工具名与三档全放行逐字不变（`archi_api` 的矩阵扩面不改档位） |
| `crates/core/archi/src/api/paths.rs` | 路径单点（供 `link`） | `ref_relative` 签名与语义零变化；`carrier_path` / `doc_path` 退役后本能力零新依赖（`link` 只用 `ref_relative`） |

### CLI: `crates/infra/agent/src/sdk/archi_tool.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api` 的 `action` 值集 | 改 | `check` / `bootstrap` / `link` 三值（`mount.rs` 的 enum 与执行体解析同批） |
| `bootstrap` 响应 | 改 | `{ schema, doc }`；`cap_result` 单点截断不变 |
| `link` 响应 | 新增 | `{ ref: "<串>" }`；参数面 `from` / `to` / `name` / `element` 的互斥与校验见正文 |

### API: 工具面（响应形状）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_module` 单元素响应 | 加字段 | `assets`（三段各自 `present`）+ `reading`；`api` / `doc` / `resolvedEntry` / `fileHash` / `hasPath` 位置与形状不变 |
| `archi_module` 薄概览 | 零变化 | 仍为 `fqn` / `kind` / `gate` / `summary` / `api.status` / `hasPath` |
| `archi_search` 语料 | 换面 | 两条固定规则（在盘 `<叶段>.md` ∪ 在盘 `<叶段>.schema.json`），与 `api` 状态无关 |

### 模块增量：写入序描述面与 reordered 响应（toml-sorting-and-layout）

主基线既有行逐字沿用；本变更受影响模块如下：

### Function: `crates/infra/agent/src/sdk/`

| 项 | 变化 | 描述 |
|---|---|---|
| `sdk/mount.rs`（两写工具 `description`） | 改 | 各加一句「写入归一到规范序（子树连续 + 关系组连续）+ 手改拆开的子树会在下次工具写时被合回 + 顺带搬家经响应 `reordered` 报出」；`parameters` / `WRITE_LIMITS` / 挂载清单 18 项 / 档位 / 矩阵**零改动** |
| `sdk/archi_tool.rs` | 改 | `element_response` 与 `relation_payload` 各加 `"reordered": &outcome.reordered`（零额外解析、零计数重算）；保护闸 / 落盘 / 三通道其余键逐字不变 |
| `crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs` | **零改动** | 骨架 1777 B / 全文 1960 B 冻结值与 `1800 / 2000` 双门限逐字不变（净余 23 B 不花、不抬锚）；「重排」字样本就零命中 |
| `crates/infra/agent/src/sdk/policy.rs` / `compose.rs` / `sdk/runner.rs` / `sdk/tools.rs` / `sdk/protect/**` | **零改动** | 档位归属、注入矩阵、上下文铸造判据、`input_path` 名单、保护闸规则集逐字不变（本变更不新增工具） |

### API: 工具响应

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_element` 响应 | 新增字段 | `reordered: { moved, blocks }`（恒在场；`moved` 真值、`blocks` cap 8）；其余八键与取值面逐字不变 |
| `archi_relation` 响应 | 新增字段（薄形键集 8 → 9） | `reordered` 进场；`counts` / `model` / `relations` 仍不携带 |
| 工具 `parameters` / 档位 / 下发面 | 零改动 | schema 键值、`enum` 派生、`ToolTier::Write`、三处下发面逐字不变 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| 知识骨架与注入面 | `preamble` 拼装、派生路径段、降级语义逐字不变（本变更不加知识句） |
| 两份 planner prompt（`crates/core/workflow/src/prompts/`） | 零改动：现有句「工具表达不了的才手改」不含「重排」字样，与规范化相容；口径校正归 `desktop-phase-prompts` 的 delta |
| IPC 命令面 / 前端 | 零改动：`reordered` 只出现在 agent 工具响应，不进任何 Tauri 命令与 DTO |

### 模块增量：x- 面五项枚举与体量抬锚（archi-api-constructor-static-members）

主基线既有行逐字沿用；本变更受影响模块如下：

### Function: `crates/infra/agent/src/archi-knowledge.md`

| 项 | 变化 | 描述 |
|---|---|---|
| `x-` 面枚举行 | 改 | `x-` 面封闭三项 → **五项逐名**（`` `x-function` / `x-class` / `x-common` / `x-constructor` / `x-static` ``），+31 B；该行仍留在「三条不可越界」② 内，不新开小节 |
| 其余段落 | 零改动 | 五段结构、工具面指路、手改出口句、留给判断 + 兼容、零围栏 / 零值域行 / 零诊断词 / 零 `<>` 占位符逐字不变 |
| 体量 | 改 | 骨架 `lf_bytes(trim_end)` 1777 → **1808 B** |

### Function: `crates/infra/agent/src/archi_knowledge_test.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| 模块头 doc | 改 | 「体量冻结（骨架 ≤ 1800 B / 全文 ≤ 2000 B）」→ 新四数（1808 / 1831 / 1997 / 2031） |
| `骨架与全文双门限冻结` | 改 | 两条 `assert_eq!` 的实测值 1777 → 1808、1966 → 1997；两条 `<=` 门限 1800 → 1831、2000 → 2031；测试 doc 的「内容变更须同批更新，MUST NOT 抬锚」改为「抬锚须显式豁免并同批更新四数」；三条失败文案里写死的数值与括号说明同步 |
| `三条不可越界逐条在场` | 改 | needle 集由「`x-` 面封闭三项」改为**五项逐名**（并加断言：该行不含「见 spec」） |
| `指令表收窄且新行集可判` | 改 | needle 集补 `` `x-constructor` `` / `` `x-static` ``（带反引号原样）——删任一项即红 |

### Function: `crates/infra/agent/src/archi_knowledge.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `build` / `knowledge` / 模块头 | 零改动 | 拼装面（骨架 + 两处 `foundation::layout` 派生路径）、占位符与空骨架校验、降级语义逐字不变；模块头未写体量与枚举数 |

### API / Component: 零改动面

| 项 | 变化 | 描述 |
|---|---|---|
| `sdk/mount.rs` / `sdk/archi_tool.rs` / `sdk/policy.rs` / `sdk/runner.rs` / `compose.rs` | 零改动 | 工具清单 18 项、档位、`(phase, role)` 矩阵、上下文铸造判据、`archi_api` 三动作、`archi_scaffold` 词典逐字不变（无新工具、无参数变化） |
| `SessionInjections.preamble` 装配与降级 | 零改动 | 两处消费（`source = "archi"` 与 change 臂写权相位）与 `preamble = None` 降级逐字不变 |
| DTO / IPC / 前端生成物 | 零改动 | 知识文本不进 `dto`、不经 IPC 出线 |
