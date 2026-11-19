# desktop-archi-api Specification（delta: module-doc-field-docs-retirement）

## Purpose

本 delta 只改**契约资产的语义定位**（E-2）：契约 md 从此不只是「契约正文」，而是 `api = declared`（缺省）元素的**入口文档本身**——落点唯一（= 派生 `contract_path`），`doc` 字段 MUST NOT 再指别处（指了即 `apiUnexpected`，两事实矛盾）；`api` 状态的角色由「契约有无」升格为**入口文档载体的开关**（`declared` → 派生落点、`none` → `doc` 声明）。E-1 已改完的资产形态、索引落点、统一目录 rationale 与生成物派生面**零改动**；本变更另把 `api::enabled` 的判据从「递归任意 `*.md`」收紧为「任一元素的载体在盘且含 api fence」（E-1 §E.6.3 明确移交的未定点，属行为变化）。

## MODIFIED Requirements

### Requirement: 契约资产形态与唯一手写真相

模块公共 API 契约 SHALL 以**手写 markdown 为唯一真相**，落位 `<archi_models_dir>/<Domain>/<FQN>.md`（`<archi_dir>` 经 `foundation::layout::archi_dir`、契约目录取既有 `foundation::layout::archi_models_dir` —— `api` 段与 `archi_api_dir` 族已退役，MUST NOT 行内字面量、MUST NOT 新增第二个目录派生函数）。`<Domain>` SHALL 取元素**父元素 FQN 末段**（`App.Backend.Infra.ArchiRuntime` → `Infra`），元素身份仍是 FQN、目录只是布局。

契约文件的章节位置语义 SHALL 封闭为五类：

| 位置 | 承载 |
|---|---|
| H1 标题 | `<FQN>`（与文件名一致，作锚点校验） |
| `## <中文二标题>` | 散文区（人审主体：职能 / 边界 / 不变量 / 承载理由） |
| `### <op-id>` + 紧随一个语言标为 `api-op` 的 json 代码块 | 操作锚点与签名（代码块归属于其前 heading，块内 MUST NOT 自包含 op 名） |
| 语言标为 `api-types` 的 json 代码块 | 本模块自有 `$defs` 定义 |
| 语言标为 `api-reexport` 的 json 代码块 | 透传清单（键 = 导出名，值 = 直接子元素 FQN） |

- **合写一份（不拆两文件）**：`<FQN>.json` + `<FQN>.md` 两份手写文件靠约定配对必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制 8 行后标题已分化为既有反例）。合写的收益不是省文件名，而是把「改契约必须同时改理由」从纪律变成**编辑动作**。
- **载体为 md 内含 fenced json，MUST NOT 用 JSON 内嵌 `description` 与 YAML block scalar**：YAML 1.1 把 `off` / `on` / `yes` / `no` 解析为布尔，而 `off` 恰是本仓 `GateTier` 的真实枚举值。
- **双向 gate（机械可校验）**：`### <op-id>` heading 集合 SHALL ≡ `api-op` fence 归属集合；`api-types` fence 的键集 SHALL ⊆ `<FQN>.schema.json` 的 `$defs` 键集；`api-reexport` fence 键集 SHALL ≡ 同文件散文区透传列表。任一不成立 SHALL 判为契约错误并点名缺失项。
- **统一目录（不跟代码走）**：契约 MUST NOT 落元素主目录。审计型问题（「这批改动动了哪些模块的契约」）只有统一目录能答，且无替代物；伴随型问题（「我改的这行契约同步了吗」）由门禁机械回答。代价 SHALL 显式留痕：在 crate 里读代码时看不到契约，只能经画布 / `archi_module` / 门禁失败提示 / 全局搜索四条路之一。
- **契约 md = declared 元素的入口文档（本变更新增，把「落点唯一」变成结构性事实）**：`api = declared`（含缺省）元素的**入口文档就是这个派生落点**，故——
  1. 模型 MUST NOT 为该元素声明 `doc`：写了即 `apiUnexpected`（两事实矛盾），且该值不参与任何判定——`contract_path` 既是派生值又是契约装载源（`load_contracts` 只认它），若放行第二个可写事实，就会出现「`doc` 指向的带 fence 文件让判据以为契约在案、而装载器读的是派生路径」⇒ 零判据静默通过；
  2. `api = none` 元素的入口文档由模型 `doc` 单值声明（可选；不写 = 无入口文档，合法但产出 `docMissing` 读数），该元素 MUST NOT 在派生路径上留契约文件（留了即 `apiUnexpected`）。
  由此 `api` 状态的角色 SHALL 表述为**入口文档载体的开关**（`declared` → 派生落点、`none` → `doc` 声明），而非仅「契约有无」；两件事共用同一判据的事实面归 `desktop-archi-api-check`。
- **与模型同树（`models/` 双身份）**：`<archi_models_dir>` SHALL 同时容纳两类住户，且两类互不参与对方的判据——**一级文件**是模型声明（恰六个 `NN-<域>.toml`）；**`<Domain>/` 子目录**是契约资产（`<FQN>.md` 手写真相 + `<FQN>.schema.json` 生成物）。模型解析的收面判据因此 SHALL 为「一级 + `.toml` 后缀」（不递归、不按目录名猜）；反向，契约面的收面判据 MUST NOT 把一级 `*.toml` 当契约或启用证据。文件命名因此承担了全部身份区分，MUST NOT 引入第二份目录级豁免清单。
- **契约面启用判据（本变更收紧，行为变化）**：`api::enabled` SHALL 判「**存在任一 `path` 非空元素的入口文档载体在盘且正文含 api fence 语言标**」（`api-op` 或 `api-types`，语言标常量单点复用 `contract::FENCE_OP` / `FENCE_TYPES`）。判据 MUST NOT 退化为「`<archi_models_dir>` 递归任意 `*.md`」——`models/` 里放一份笔记（非任何元素的载体）或一份无 fence 的 `<Domain>/notes.md` MUST NOT 误判为启用。该判据与判据面同源（同一 `doc_facts` 事实），MUST NOT 在命令层另判一套。
- **不接入文档关联面（本变更改写）**：契约 MUST NOT 成为任何「文档关联来源」——原三条来源（`explicit` / `adjacent` / `text`）已随 `desktop-archi-module-docs` 整体退役，MUST NOT 复活为第四种来源，MUST NOT 新增 `kind = api` 之类的形态标记。契约正文的抵达路径只有三条：`api.contractPath` / `doc`（none 元素）/ 通用 `read_file` 按路径自取；检索面只收路径串、零正文。
- **不依赖语言概念**：资产形态、目录布局、schema 与索引中 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` 一类语言专有概念——语言特定部分只落取数器（`api/extract/{rust,ts}`）与渲染器（`api/emit/{rust,ts}`），见「语言无关纪律与语言特定落点」。

#### Scenario: 五类位置语义与 H1 锚点校验

- **WHEN** 解析一份含 H1 `<FQN>`、`## 职能` 散文、两个 `### <op-id>` + `api-op` fence、一个 `api-types` fence 的契约
- **THEN** 解析成功；两个 op 的 id 与 heading 逐字对应，fence 内不含 op 名；H1 与文件名不一致时报契约错误并指出两处原文

#### Scenario: 双向 gate 三处不成立各报错

- **WHEN** 分别构造「`### new` 有 heading 无 fence」「`api-types` 里有个键在生成的 `$defs` 缺失」「`api-reexport` fence 有 3 个键而散文只列 2 个」
- **THEN** 三次均判契约错误，错误文案分别点名缺 fence 的 op、缺定义的键、fence 与散文的差集；MUST NOT 静默通过

#### Scenario: 拆两文件与 JSON 内嵌描述被拒

- **WHEN** 审查资产形态与提交内容
- **THEN** `<archi_dir>/models/<Domain>/` 下每个元素只有一份 `*.md` 手写真相（同目录的 `.schema.json` 为生成物且头部带警示；`INDEX.toml` 不在契约目录内，落 `<archi_dir>/graph/`）；不存在与 `.md` 配对的第二份手写 `.json`，散文不出现在 JSON 字符串里

#### Scenario: 契约与模型同树互不干扰

- **WHEN** 在 `models/` 于六个 `NN-<域>.toml` 之外同时放入 `<Domain>/<FQN>.md`、`<Domain>/<FQN>.schema.json` 与三级子目录，随后跑一次模型解析与一次契约装载
- **THEN** 模型解析结果与只放六个 toml 时逐字相同（`diagnostics` 空、元素 / 关系计数不变）；契约装载恰好读到那份 md；任一方向 MUST NOT 因同目录混居产生诊断或判据

#### Scenario: declared 元素写 doc 即两事实矛盾

- **WHEN** 某 `api = declared` 元素的模型里写了 `doc = "./somewhere/README.md"`，同时其派生契约 md 在盘含 fence
- **THEN** 判据面报一条 `apiUnexpected`（`category = entry`）；契约装载仍只读派生路径（`contract::parse` 的输入源零改动），`doc` 的值 MUST NOT 被装载器或判据引用；MUST NOT 出现「两份入口文档都在案」的中间态

#### Scenario: none 元素的入口文档由 doc 声明

- **WHEN** 某 `api = none` 元素的模型写 `doc = "./packages/desktop/src-tauri/crates/tools/export-bindings/README.md"` 且该文件在盘
- **THEN** 该元素的入口文档即该 README（`ArchiModuleDetail.doc` 出线该路径串）；它 MUST NOT 带 `api-op` / `api-types` fence 义务（无契约面）；派生路径上 MUST NOT 存在契约文件

#### Scenario: 启用判据不因杂项 md 误启用

- **WHEN** 观察根下 `models/` 一级有六个 `NN-<域>.toml`，另有 `models/notes.md`（非任何元素的载体）与 `<Domain>/notes.md`（无 fence）
- **THEN** `api::enabled = false` 且 `apiFindings = []`；随后在某 `api = declared` 元素的派生契约路径放入一份带 `api-op` fence 的 md ⇒ `enabled = true`、全部 `path` 非空元素纳入判据

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/paths.rs` | 落点派生 | `directory(root)` == `archi_models_dir(root)`、`contract_path` == `<models>/<Domain>/<FQN>.md`、`schema_path` / `index_path` 逐字不变；**载体派生单点**：declared → `contract_path`、none → `doc` 归一后 join root（形态校验复用 `normalize_root` + `is_repo_relative`，后者提 `pub(crate)`）；MUST NOT 出现第二份路径归一实现（`docs::normalize_rel` 已随 `docs.rs` 删除） |
| `crates/core/archi/src/api/mod.rs::enabled` | 启用判据（**收紧**） | `enabled(root, elements)` = 任一 `path` 非空元素的载体在盘且 `DocFact::Present { has_api_fence: true }`；MUST NOT 退化为「递归任意 `*.md`」；命令层 MUST NOT 另判一套 |
| `crates/core/archi/src/api/mod.rs::load_contracts` | 契约装载（输入源零改动） | 只认 `contract_path` 派生落点（`doc` 不参与装载）；`ApiSnapshot` 增 `doc_facts`（`Absent` / `Missing` / `Present { has_api_fence }`），每份 doc 只读一次即弃 |
| `crates/core/archi/src/api/contract.rs` | 契约解析 | 五类位置语义、双向 gate、`$defs` 规则、落点闸逐字不变；仅 `FENCE_OP` / `FENCE_TYPES` 提 `pub(crate)` 供启用判据与 `apiDocMissing` 复用 |
| `crates/core/archi/src/api/{schema,render,emit,extract,diff}.rs` | 生成与取数 | 逐字不变（`doc` 不是生成物；`.i.*` 落点仍是入口的函数） |
| `crates/core/archi/src/dto.rs::ArchiApiRef` | 契约状态投影 | 四字段（`status` / `contractPath` / `schemaPath` / `generatedPath`）零改动；`declared` 元素的 `contractPath` 同时是它的**入口文档**路径（前端据此渲「契约」行，不得再渲第二条入口文档） |
| `crates/core/archi/src/models/**`（仓内资产） | 试点契约 | `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md` 与 `.schema.json` 逐字不变（它是启用判据的唯一在案证据） |
