# desktop-archi-agent Specification（delta: module-doc-field-docs-retirement）

## Purpose

本 delta 把三个 agent 侧只读工具面随字段面与检索语料换面：`archi_module` 的关联文档位次（`docs` 四字段条目 + `warnings`）退役、改出**单值 `doc`**（只有 `api = none` 元素有值）；`archi_search` 的文档 scope 词由 `docs` 改 `doc`、语料改「在盘入口文档载体」、结果字段退化为路径串；架构会话知识文本随之改字段面（八项含 `doc`）、示例改两段（`api = "none"` + `doc` / declared 零 `doc` 行）、删掉「决策记录目录」一句。另把 `desktop-archi-module-docs` 退役后仍有效的三条检索面纪律以 ADDED requirement 承接过来。工具名集合、注入矩阵与只读档位零变化。

## MODIFIED Requirements

### Requirement: archi_module 只读工具的定位契约

SDK 引擎 SHALL 提供专用只读工具 `archi_module`（不在七通用工具 `TOOL_NAMES` 封闭集内，经工具名字清单受控注入），语义为**定位**（已知在看哪个模块）：

```jsonc
{ "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" } }
```

- 指定 `element` 时响应 SHALL 含：`element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine`）、`dependsOn`（`target` + `description`）、`dependedBy`（`source` + `description`）、`children`（直接子元素 FQN）、**`doc`（单值入口文档路径，仅 `api = none` 元素有值）**、**`api`（并列契约状态字段）**。`docs` 数组与 `warnings` 数组两个位次已退役。
- 省略 `element` 时 SHALL 返回全部元素的**薄概览**（`fqn` / `kind` / `gate` / 一行 `summary` / **`api.status`**，无邻接、无入口文档、无契约路径），并受既有单结果字节上限约束（超限截断留痕）。
- `modelFile` + `modelLine` SHALL 来自解析器已产出的声明位置（零额外实现），使 agent 能直接 `edit` 到该行而无需 grep 猜测。
- 元素不存在 SHALL 返回可诊断 `Err`（附近似 FQN 或总量提示），MUST NOT 返回空对象。
- 模型结构错时 SHALL 返回结构化诊断清单作为 `Err`（绝不 panic），使 agent 能看出"模型本身坏了"而不是"没有这个模块"。
- 响应 SHALL 经 `archi::dto` 单点投影产出（MUST NOT 在工具执行体内另拼一份 JSON）。
- **`doc` 并列字段（本变更换面）**：`doc` SHALL 为 repo 相对 POSIX 路径**单值**或 `null`（无 `title` / `kind` / `source` 四字段形态）。只有 `api = none` 的元素会填；`api = declared` 元素的入口文档走既有 `api.contractPath`，MUST NOT 在 `doc` 里重复出线。`doc` 指向的文件不存在时该元素的判据面报 `docNotFound`（`error` 档）——工具响应本身 MUST NOT 为此降级或补默认值。
- **`api` 并列字段**：单元素响应 SHALL 并出 `api` 对象（`status`（`declared` / `none`）/ `contractPath` / `schemaPath` / `generatedPath`），定位 SHALL 按 `<archi_models_dir>/<Domain>/<FQN>.md` 与相应的 `.schema.json` / 生成物路径**派生**（全经 `foundation::layout` 的派生族与 `api::paths` 单点，MUST NOT 行内拼路径）；文件不存在即 `status = none` 且三个路径字段为 `null`。该字段 MUST NOT 承载协议正文，MUST NOT 成为独立工具、MUST NOT 改变本工具的只读属性与注入矩阵。
- **描述面（`mount.rs`）**：`archi_module` 的工具描述 SHALL 去掉「关联文档（四字段条目）」与「`docNotFound` 警告」两句，改为「入口文档单值 `doc`（仅 `api = none` 元素有值；declared 走 `api.contractPath`）」，MUST NOT 残留 `docs` 词汇或警告语义。

#### Scenario: 定位单元素

- **WHEN** 模型会话中调用 `archi_module(element="App.Auth")`
- **THEN** 返回该元素的 `kind` / `gate` / `path` / `metadata` / `modelFile` / `modelLine`，其出边与入边（各含对端 FQN 与 `description`）、直接子元素清单、`doc`（单值或 `null`）；响应内 MUST NOT 出现 `docs` 数组

#### Scenario: 省略元素给薄概览并截断

- **WHEN** 在不传 `element` 的调用下模型含数百元素
- **THEN** 返回全元素薄概览（无邻接、无入口文档），超出单结果字节上限时截断并留痕（提示可指定元素收窄）

#### Scenario: 坏模型可辨

- **WHEN** 模型文件被写坏（语法错 / 环）后调用 `archi_module`
- **THEN** 返回结构化诊断（含 `file:line`）且 `is_error` 为真；执行体不 panic，run 不中断（agent 可转去修模型）

#### Scenario: api 并列字段两态可辨

- **WHEN** 分别对契约在案的元素（`App.Backend.Infra.ArchiRuntime`）与无契约元素调用 `archi_module(element = …)`
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `.dev-team/architecture/models/Infra/` 下的实际文件、`generatedPath` 指向入口同级生成物；后者 `api.status = "none"` 且三者 `null`

#### Scenario: doc 只在 api = none 元素上有值

- **WHEN** 分别对 `api = none` 且声明了 `doc` 的元素、`api = none` 未声明 `doc` 的元素、`api = declared` 的元素调用 `archi_module`
- **THEN** 依次为「路径串」/ `null` / `null`（字段不省略）；declared 元素的入口文档只能经 `api.contractPath` 读到——两处 MUST NOT 同时给出口 entry 文档（否则抽屉会出现两条）

#### Scenario: 薄概览带 api.status 但不带路径与 doc

- **WHEN** 省略 `element` 调用 `archi_module`
- **THEN** 每条薄概览含 `api.status`（`declared` / `none`），不含 `contractPath` / `schemaPath` / `generatedPath` / `doc`；超单结果字节上限时仍按既有纪律截断留痕

#### Scenario: 契约正文不经工具响应

- **WHEN** 审查 `archi_module` 响应体与体积
- **THEN** 只含 `api` 四字段（状态 + 三路径）与 `doc` 单值，零签名、零 `$defs`、零散文、零四字段文档条目；正文由 agent 用 `read` 按路径自取

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
- **THEN** 命中 FQN / summary 含该词的元素，与路径含该词的入口文档（如 `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md`）；结果含元素薄条目与路径串，MUST NOT 含四字段条目

#### Scenario: 驼峰分词

- **WHEN** 调用 `archi_search(query="AuthToken")`
- **THEN** 命中 `App.Auth.Token`（分词后 `auth` 与 `token` 两段均参与匹配）

#### Scenario: 正文、specs 与 ADR 不参与

- **WHEN** `query` 只出现在某文档正文内、或只出现在 `openspec/specs/` 的路径 / 内容内、或只出现在 `decisions/*.md` 内
- **THEN** 均不命中（刻意不做全文搜索，正文搜索归 agent 既有 `grep`；ADR 经入口文档链接 / 文件树 / 通用搜索抵达）

#### Scenario: scope 换词与空串拒绝

- **WHEN** 分别以 `scope = "elements"` / `scope = "doc"` / `scope = "docs"` / `query = ""` 调用
- **THEN** 前两者分别使另一半结果为空数组；`"docs"` 显式 `Err` 并列出 `elements` / `doc` / `all` 三个合法值；空 query 显式 `Err`（MUST NOT 返回全量）

### Requirement: 架构会话的系统知识注入

`source = "archi"` 的会话 SHALL 注入一段架构会话知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`），内容 SHALL 覆盖以下关注点：

- **模型位置与聚合**：模型文件位于架构域 `models/` 下、按文件名字典序聚合；目录路径 SHALL 为**派生所得**的 repo 相对路径（见 `workspace-layout-resolution`），MUST NOT 在产品源码或知识文本中手写目录字面量。
- **格式约定**：一元素一表头、键为带引号的完整 FQN 单键（禁点分嵌套写法）；元素字段面为 `kind`（必填）/ `gate` / `api` / `entry` / `path` / **`doc`** / `summary` / `metadata`（`docs` 已退役，MUST NOT 再出现在字段面清单里）；`entry` 是**单值**实现入口指针，只在约定默认指不到时显式写（目录型壳与二进制 crate），单文件根元素 MUST NOT 写；**`doc` 是单值入口文档指针，只有 `api = none` 的元素可写**（`api = declared` 的入口文档恒为派生落点 `<models>/<Domain>/<FQN>.md`，写了即 `apiUnexpected`）；关系为 `[[relation]]` 条目（`from` / `to` / `desc`）；`metadata` MUST 用内联表，MUST NOT 用子表（子表会被解析为另一个元素）。
- **层级与缺省**：层级由 FQN 最长前缀推导（子元素须先声明祖先）；新元素 `gate` 缺省 `off`（ratchet 逐块收紧）。
- **边界**：模型里 MUST NOT 写接口 / 导出符号（契约权威在契约资产）；`api` / `entry` / `doc` 是三个**指针**（有无公共 API / 入口在哪 / 入口文档读哪份），MUST NOT 由它们扩张出协议正文。
- **自检纪律**：改完 SHALL 立即复查——调用 `archi_module`（不带 `element`）取得薄概览或结构诊断清单（诊断带 `file:line`），按行修复；契约面另可调 `archi_api`（`action = "check"`）查两轴（生成物三类差异 + 契约判据）；生成物过期 SHALL 用既有命令（`pnpm api:export` / `cargo run -p archi-api-gen -- generate`）重生成，MUST NOT 手改生成物。该纪律 SHALL 为纯提示词约束，MUST NOT 依赖写盘校验或引擎自动校验回灌。
- **写面与可见性**：直改主仓（无 worktree、无归档确认），改动随 `git diff` 评审；只改架构域模型与 **`doc` 所指的入口文档**（`docs` 关联面已不存在），改源码须用户明确要求；写盘后画布不自动更新，须提示用户点「刷新」。
- **历史会话续接**：续接历史会话时 SHALL 先取当下模型现状（`archi_module` 无参概览或指定元素）再回答「现在是什么样」一类问题，MUST NOT 依赖会话早前的描述。

词表类内容 SHALL 从代码单点派生，MUST NOT 手抄：

- `kind` 四值 ← `ElementKind::ALL`；`gate` 三值 + 缺省档 ← `GateTier::ALL` / `GateTier::default()`；`api` 状态二值 + 缺省档 ← `ApiStatus::ALL` / `ApiStatus::default()`；结构诊断九类词 ← `DiagnosticKind::ALL`；目录相对路径 ← `foundation::layout` 的架构域相对路径派生族。
- `entry` 与 `doc` SHALL **不进**派生词表：两者的值域是任意 repo 相对路径（开放集），不是封闭词汇；知识文本只给纪律句，判据定义归 `desktop-archi-api` / `desktop-archi-api-check`，不在知识文本内复制判据长表。
- **派生的目录方位 SHALL 为三处**：模型目录（`archi_models_rel_dir`）、契约资产（`<models>/<Domain>/<FQN>.md`）、契约索引（`archi_graph_rel_dir`/`INDEX.toml`）。**「决策记录目录：{}」一句 SHALL 删除**（`archi_decisions_rel_dir` 已随 `docs` 三源退役删除；ADR 的定位改由入口文档链接与文件树承担）。
- 知识文本 SHALL 为**静态骨架 md（`include_str!`）+ 上述派生片段**在装配侧拼成的单条文本；新增 / 删除任一封闭词汇而知识未同步时，派生消费面测试 MUST 变红（四面词汇集一致性见 `desktop-archi-model`「消费面词汇集一致性对拍」）。

两条工程约束：

- **自证可解析**：知识文本 SHALL 含示例 TOML，测试 SHALL 以 `archi` 的解析面解析该片段并要求零结构诊断（字段名 / 内联 `metadata` / 键形 / `entry` 单值形态 / `doc` 单值形态任一漂移即红）。示例 SHALL 覆盖两段形态：① `api = "none"` + `doc = "<某 README 路径>"`；② declared 元素**零 `doc` 行**（入口文档 = 派生落点，示例内以注释指路）。
- **尽力增强、不炸 run**：知识拼装失败 SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符，体量 SHALL 受上限约束（测试锚定）。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 词表由代码派生

- **WHEN** 检查知识文本的 `kind` / `gate` / `api` / 诊断词清单，并新增一个结构诊断变体（同时补 `DiagnosticKind::ALL`）
- **THEN** 知识文本随即含该新词（无需改静态骨架）；若只加变体而未补 `ALL`，`ALL` 与 `as_str` 对拍测试红；`entry` / `doc` 不出现在任何派生词表行（开放值集，零词表）

#### Scenario: 示例片段可解析且两段形态在案

- **WHEN** 取出知识文本中的示例 TOML 片段并用 `archi` 解析
- **THEN** 零结构诊断；片段含一个 `api = "none"` + `doc = "..."` 的元素，且 declared 元素零 `doc` 行；把 `metadata` 改成子表、改掉任一字段名、把 `entry` 改成数组、把 `doc` 改成数组、或给 declared 元素补 `doc` 行后该测试红（最后一项由判据面 `apiUnexpected` 与示例断言双守）

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 三 archi 工具不变），MUST NOT 抛装配错误

#### Scenario: 自检纪律在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完调用 `archi_module` 复查并按其 `file:line` 修复」「契约面可调 `archi_api` 查两轴」「生成物过期用既有命令重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」五条；且知识文本内零 `<>` 占位符

#### Scenario: 字段面八项含 doc 且决策记录目录句已删

- **WHEN** 审查知识文本的「格式约定」段与派生段
- **THEN** 字段面逐项列出 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 八项（`docs` 零命中）；`entry` 的三条纪律与 `doc` 的「只有 `api = none` 可写」纪律在案；派生段含模型目录 / 契约资产 / 契约索引三处方位，且**零「决策记录目录」串**

## ADDED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/mount.rs` | 工具定义面 | `archi_module` 描述去「四字段文档条目」与 `docNotFound` 警告句、加「`doc` 仅 `api = none` 有值」；`archi_search` 的 `scope` enum → `["elements", "doc", "all"]`、描述内「四字段文档条目」→「入口文档路径」；工具名常量与三个定义的参数形状（`element` / `query`+`scope` / `action`+`element`）零变化 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具执行体 | 经 `LoopTurn` per-turn 上下文取 root → `archi::dto` 投影（`archi_module` 两形态含 `doc` 单值 / `archi_search` 三 scope 含 `doc` 词）；`archi_api` 的 `check` / `bootstrap` 零改动；命中既有单结果上限收口；坏模型 → `is_error` 不 panic |
| `crates/infra/agent/src/archi_knowledge.rs` | 骨架 + 派生段拼装 | 派生段删「决策记录目录：{}」一句（`archi_decisions_rel_dir` 已删）；保留模型目录 / 契约资产 / 契约索引三处方位；四面词表注入与降级为 `None` 的语义逐字不变 |
| `crates/infra/agent/src/archi-knowledge.md` | 知识静态骨架 | 「格式约定」字段面八项含 `doc`（`docs` 零命中）；新增 `doc` 的两条纪律（单值 / 只有 `api = none` 可写）；示例两段形态（`api = "none"` + `doc` / declared 零 `doc` 行）；「写面与可见性」段把「只改模型与 `docs` 关联文档」改为「只改模型与 `doc` 所指入口文档」；零 `<>` 占位符、体量受上限约束 |
| `crates/core/archi/src/dto.rs` | 投影单点 | `module_detail` 的 `doc` 与 `search` 的 `doc: Vec<String>` 由本 crate 单点产出（工具执行体零拼接、零二次读模型） |
| `crates/infra/agent/src/sdk/policy.rs` | 静态权限表 | `READONLY_TOOLS` 三工具名与三档全放行逐字不变（工具名集合零变化 ⇒ 注入矩阵零变化） |
