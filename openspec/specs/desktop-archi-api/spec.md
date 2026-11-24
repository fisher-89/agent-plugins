# desktop-archi-api Specification

## Purpose

定义模块 API 契约的资产形态与生成链：以 `<archi_models_dir>/<分层目录…>/<FQN>.md` 为唯一手写真相（散文 + `api-op` / `api-types` / `api-reexport` 三类 fenced json），`<FQN>.schema.json` 与 `INDEX.toml` 为纯派生生成物，加上引用与命名规则、关键字集与类型映射、生成物落点与入口 `#[path]` 纪律、零 diff 门挂载、语言无关纪律、`verify` 闭环与 bootstrap 剪枝、模块嵌套与选择性透传、TS 导出闭包取数面。

## Requirements

### Requirement: 契约资产形态与唯一手写真相

模块公共 API 契约 SHALL 以**手写 markdown 为唯一真相**，落位 `<archi_models_dir>/<分层目录…>/<FQN>.md`（`<archi_dir>` 经 `foundation::layout::archi_dir`、契约目录取既有 `foundation::layout::archi_models_dir` —— `api` 段与 `archi_api_dir` 族已退役，MUST NOT 行内字面量、MUST NOT 新增第二个目录派生函数）。`<分层目录…>` SHALL 是元素 FQN 的**祖先链镜像**：取 FQN 段链去掉**叶段**后的逐段路径（`Backend.Infra.ArchiRuntime` → `Backend/Infra/`、`Desktop.Components.Agent.AgentInput` → `Desktop/Components/Agent/`、单段包根 `Backend` → 契约目录根零段），元素身份仍是 FQN（文件名恒为完整 FQN）、目录只是布局——同一末段的不同元素（`Backend.Core.Agent` 与 `Desktop.Components.Agent`）因祖先链不同而落在不同层次，不再混装同一目录。

契约文件的章节位置语义 SHALL 封闭为五类：

| 位置 | 承载 |
|---|---|
| H1 标题 | `<FQN>`（与文件名一致，作锚点校验） |
| `## <中文二标题>` | 散文区（人审主体：职能 / 边界 / 不变量 / 承载理由；MUST NOT 以表格复述契约名字——名字清单的唯一持有者是三类 fence，见下「名字清单唯一持有者与第四条 gate」） |
| `### <op-id>` + 紧随一个语言标为 `api-op` 的 json 代码块 | 操作锚点与签名（代码块归属于其前 heading，块内 MUST NOT 自包含 op 名） |
| 语言标为 `api-types` 的 json 代码块 | 本模块自有 `$defs` 定义 |
| 语言标为 `api-reexport` 的 json 代码块 | 透传清单（键 = 导出名，值 = 直接子元素 FQN） |

- **合写一份（不拆两文件）**：`<FQN>.json` + `<FQN>.md` 两份手写文件靠约定配对必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制 8 行后标题已分化为既有反例）。合写的收益不是省文件名，而是把「改契约必须同时改理由」从纪律变成**编辑动作**——改了 op 签名却不改不变量，在同一文件里是显眼的遗漏。
- **载体为 md 内含 fenced json，MUST NOT 用 JSON 内嵌 `description`（散文变整行超长字符串）与 YAML block scalar**：YAML 1.1 把 `off` / `on` / `yes` / `no` 解析为布尔，而 `off` 恰是本仓 `GateTier` 的真实枚举值，各语言解析器版本不一致会破坏跨语言一致性。
- **双向 gate（机械可校验）三条**：`### <op-id>` heading 集合 SHALL ≡ `api-op` fence 归属集合；`api-types` fence 的键集 SHALL ⊆ `<FQN>.schema.json` 的 `$defs` 键集；`api-reexport` fence 键集 SHALL ≡ 同文件散文区透传列表。任一不成立 SHALL 判为契约错误并点名缺失项。
- **名字清单唯一持有者与第四条 gate（机械可校验）**：名字清单的唯一持有者是契约 fence（`api-op` 的 op id / `api-types` 的键 / `api-reexport` 的键），散文段 SHALL NOT 以表格复述这些名字。判据闭合为三段：
  1. **抽取面**：散文段正文的**表格行**——去前导空白后以 `|` 起手，按 `|` 切出的首列去空白后**整体被一对反引号包裹**且内容是 ASCII 标识符（`is_identifier`）时计入；其余形态一律跳过。fence 内容天然不在散文正文里（解析期的 fence 状态机已隔断），**列表项与裸正文 MUST NOT 抽取**——列表项是既有 `api-reexport` 散文透传列表与留白登记的形态（其一致性归第三条 gate），裸正文是语义散文。
  2. **前置（留白豁免）**：契约非留白，即 `ops` / `types` / `reexports` 三项**任一非空**（取手写真假，MUST NOT 取生成的 `$defs`；只写透传的契约同样持有名字，故 `reexports` 与另两项同权）。三项全空即「契约面留白」，该元素的散文表格原样合法——这一支是前端 8 个组件元素当前形态的唯一合法通道，且属**临时债**（见下条）。
  3. **断言**：抽取集 ∩ 契约名字集（`api-op` 的 op id ∪ `api-types` 键 ∪ `api-reexport` 键）= **∅**。MUST NOT 写成「集合相等 / 散文必须镜像契约」——闸判的是「同一名字不得两处持有」；散文的职能 / 边界 / 不变量 / 承载理由仍是人审面，本闸**只判点名、不判语义**。
  违反即 `ApiError::Contract`（`load_contracts` 中止，零新增 finding kind、不进 `apiFindings`、不动十三值词汇 / 生成物 / 金样），文案 SHALL 点名文件与被复述的名字并给出路句（「名字归 fence，散文只留语义」）；行号不作承诺（与既有两条 gate 同档——段落正文未保留行号）。闸作用域为**全部 `##` 散文段**，MUST NOT 引入段名白名单或豁免清单（`## 公共面` 是临时形态，冻结成机器约定会把留白债写成结构）。
- **前端组件契约的留白是临时状态，退役由第四条 gate 机械强制**：8 个前端元素（`Desktop.Components.Agent` 与其 7 个子元素）的契约三项当前全空、散文「公共面」表共 27 条数据行 ⇒ 前置不成立、闸不生效、表格**原样合法**。它们的名字清单将来由组件契约变更迁入 `$defs` / `api-op`（须先解 `$ref` 归一对称 / 组件与 JSX 元素的契约词汇 / `.i.ts` 末尾空行三处缺口，属另起变更）；一旦某元素契约非留白而同一提交未清表格，装载期即失败——**退役不是纪律要求，是闸的机械后果**。MUST NOT 提前删表（27 条人读语义无处安放），也 MUST NOT 以 `apiUndeclared` 抬档代偿（留白期抬档会把临时债读成判据面债务）。
- **统一目录（不跟代码走）**：契约 MUST NOT 落元素主目录。审计型问题（「这批改动动了哪些模块的契约」）只有统一目录能答（`git diff --stat <archi>/models/<分层目录…>/` 即列出），且无替代物；伴随型问题（「我改的这行契约同步了吗」）由门禁机械回答，不需要文件相邻。代价 SHALL 显式留痕：在 crate 里读代码时看不到契约，只能经画布 / `archi_module` / 门禁失败提示 / 全局搜索四条路之一。
- **与模型分目录（模型声明在 `graph/model/`，契约资产在 `models/`）**：`<archi_models_dir>` SHALL 只容纳契约资产——**`<分层目录…>/` 子目录**内的 `<FQN>.md`（手写真相）+ `<FQN>.schema.json`（生成物）；模型声明（恰六个 `NN-<域>.toml`，见 `repo-archi-model`「基线落位与模型文件划分」）在另一目录 `<archi_graph_model_dir>`（`<archi_graph_dir>/model`）。两面的分离判据是**目录**：模型解析的收面只扫 `graph/model/` 一级，MUST NOT 因 `models/` 出现 md / schema / `.toml` 而改变；反向，契约面的收面 MUST NOT 把 `graph/model/*.toml` 当契约或启用证据。MUST NOT 引入目录级豁免清单（目录分家本身就是判据）。
- **契约 md = declared 元素的入口文档（把「落点唯一」变成结构性事实）**：`api = declared`（含缺省）元素的**入口文档就是这个派生落点**，故——
  1. 模型 MUST NOT 为该元素声明 `doc`：写了即 `apiUnexpected`（两事实矛盾），且该值不参与任何判定——`contract_path` 既是派生值又是契约装载源（`load_contracts` 只认它），若放行第二个可写事实，就会出现「`doc` 指向的带 fence 文件让判据以为契约在案、而装载器读的是派生路径」⇒ 零判据静默通过；
  2. `api = none` 元素的入口文档由模型 `doc` 单值声明（可选；不写 = 无入口文档，合法但产出 `docMissing` 读数），该元素 MUST NOT 在派生路径上留契约文件（留了即 `apiUnexpected`）。
  由此 `api` 状态的角色 SHALL 表述为**入口文档载体的开关**（`declared` → 派生落点、`none` → `doc` 声明），而非仅「契约有无」；两件事共用同一判据的事实面归 `desktop-archi-api-check`。
- **契约面启用判据（本变更收紧，行为变化）**：`api::enabled` SHALL 判「**存在任一 `path` 非空元素的入口文档载体在盘且正文含 api fence 语言标**」（`api-op` 或 `api-types`，语言标常量单点复用 `contract::FENCE_OP` / `FENCE_TYPES`）。判据 MUST NOT 退化为「`<archi_models_dir>` 递归任意 `*.md`」——`models/` 里放一份笔记（非任何元素的载体）或一份无 fence 的 `Backend/notes.md` MUST NOT 误判为启用。该判据与判据面同源（同一 `doc_facts` 事实），MUST NOT 在命令层另判一套。
- **不接入文档关联面（本变更改写）**：契约 MUST NOT 成为任何「文档关联来源」——原三条来源（`explicit` / `adjacent` / `text`）已随 `desktop-archi-module-docs` 整体退役，MUST NOT 复活为第四种来源，MUST NOT 新增 `kind = api` 之类的形态标记。契约正文的抵达路径只有三条：`api.contractPath` / `doc`（none 元素）/ 通用 `read_file` 按路径自取；检索面只收路径串、零正文。
- **不依赖语言概念**：资产形态、目录布局、schema 与索引中 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` 一类语言专有概念——语言特定部分只落取数器（`api/extract/{rust,ts}`）与渲染器（`api/emit/{rust,ts}`），见「语言无关纪律与语言特定落点」。

#### Scenario: 五类位置语义与 H1 锚点校验

- **WHEN** 解析一份含 H1 `<FQN>`、`## 职能` 散文、两个 `### <op-id>` + `api-op` fence、一个 `api-types` fence 的契约
- **THEN** 解析成功；两个 op 的 id 与 heading 逐字对应，fence 内不含 op 名；H1 与文件名不一致时报契约错误并指出两处原文

#### Scenario: 双向 gate 三处不成立各报错

- **WHEN** 分别构造「`### new` 有 heading 无 fence」「`api-types` 里有个键在生成的 `$defs` 缺失」「`api-reexport` fence 有 3 个键而散文只列 2 个」
- **THEN** 三次均判契约错误，错误文案分别点名缺 fence 的 op、缺定义的键、fence 与散文的差集；MUST NOT 静默通过

#### Scenario: 散文表格复述契约名字即装载期契约错误

- **WHEN** 某 `api = declared` 元素的契约已声明 op `is_set`（含 `api-op` fence 与 `### is_set` heading），其散文段又有一张表格以 `` `is_set` `` 起首的一行
- **THEN** 装载期判 `ApiError::Contract`，文案点名文件与 `is_set` 并给出路句（名字归 fence、散文只留语义）；`load_contracts` 中止、不产出半真报告；同一事实 MUST NOT 出现在 `apiFindings`（十三值零新增）

#### Scenario: 留白契约的公共面表原样合法，补契约即转红

- **WHEN** 某前端元素的契约只有 `## 公共面` 表格（7 行）与 `api-types` 的 `{}`（三项全空）
- **THEN** 第四条 gate 的前置不成立 ⇒ 装载零错误、表格原样合法；随后在该契约补一个 `api-types` 键 `AgentInput` 而表格仍列同名 ⇒ 装载期立刻报错（临时债变为违约）

#### Scenario: 抽取面不碰列表项与首列无引号的表格

- **WHEN** 某契约的散文用列表项登记 16 条留白 op 名（形如 `` - `resolve`：… ``），另有一张首列不带反引号的表格（`| is_set | 判定 | … |`）而契约已声明 op `is_set`
- **THEN** 抽取集为空（列表项与首列无引号的行零抽取）⇒ 闸通过；只有「表格行 + 首列整体反引号 + ASCII 标识符」构成抽取证据，MUST NOT 为覆盖面放宽该形态

#### Scenario: 拆两文件与 JSON 内嵌描述被拒

- **WHEN** 审查资产形态与提交内容
- **THEN** `<archi_dir>/models/<分层目录…>/` 下每个元素只有一份 `*.md` 手写真相（同目录的 `.schema.json` 为生成物且头部带警示；`INDEX.toml` 不在契约目录内，落 `<archi_dir>/graph/`）；不存在与 `.md` 配对的第二份手写 `.json`，散文不出现在 JSON 字符串里

#### Scenario: 契约与模型分目录互不干扰

- **WHEN** 在 `<archi_dir>/graph/model/` 放六个 `NN-<域>.toml`，同时在 `<archi_dir>/models/` 的 `Backend/Core/`、`Desktop/Components/Agent/` 三级分层目录下各放 `<FQN>.md` 与 `<FQN>.schema.json`，随后跑一次模型解析与一次契约装载
- **THEN** 模型解析结果与只放六个 toml 时逐字相同（`diagnostics` 空、元素 / 关系计数不变——收面判据是「`graph/model/` 一级 + `.toml` 后缀」）；契约装载恰好读到那份 md；任一方向 MUST NOT 因两目录并存产生诊断或判据；反向在 `models/` 一级放一份合法 `.toml` MUST NOT 进模型收面

#### Scenario: declared 元素写 doc 即两事实矛盾

- **WHEN** 某 `api = declared` 元素的模型里写了 `doc = "./somewhere/README.md"`，同时其派生契约 md 在盘含 fence
- **THEN** 判据面报一条 `apiUnexpected`（`category = entry`）；契约装载仍只读派生路径（`contract::parse` 的输入源零改动），`doc` 的值 MUST NOT 被装载器或判据引用；MUST NOT 出现「两份入口文档都在案」的中间态

#### Scenario: none 元素的入口文档由 doc 声明

- **WHEN** 某 `api = none` 元素的模型写 `doc = "./packages/desktop/src-tauri/crates/tools/export-bindings/README.md"` 且该文件在盘
- **THEN** 该元素的入口文档即该 README（`ArchiModuleDetail.doc` 出线该路径串）；它 MUST NOT 带 `api-op` / `api-types` fence 义务（无契约面）；派生路径上 MUST NOT 存在契约文件

#### Scenario: 启用判据不因杂项 md 误启用

- **WHEN** 观察根下 `graph/model/` 一级有六个 `NN-<域>.toml`，另有 `models/notes.md`（非任何元素的载体）与 `Backend/notes.md`（无 fence）
- **THEN** `api::enabled = false` 且 `apiFindings = []`；随后在某 `api = declared` 元素的派生契约路径放入一份带 `api-op` fence 的 md ⇒ `enabled = true`、全部 `path` 非空元素纳入判据

### Requirement: schema.json 与 INDEX.toml 为纯派生且不可手改

每个契约 SHALL 派生 `<FQN>.schema.json`（JSON Schema 投影，与 `<FQN>.md` 同目录，即 `<archi_models_dir>/<分层目录…>/`），并由全体契约派生出 `INDEX.toml`（落 `<archi_graph_dir>` 下、全局唯一一份，MUST NOT 落契约目录的根、MUST NOT 落模型声明目录 `graph/model/`）。两者都是生成物、都不可手改。

`<FQN>.schema.json` SHALL 的每一项字段都必须有 markdown 出处，生成器只做**装配**不做**翻译**：

| `schema.json` 字段 | 来源 |
|---|---|
| `$schema` / `$id` | 常量 + FQN（`$id` 取文件名 `<FQN>.schema.json`，不合成 URI） |
| `title` | H1 标题 |
| `$defs` 本地定义 | `api-types` fence |
| `$defs` 透传 alias | `api-reexport` fence（值 → 子元素契约的相对文件路径 + `#/$defs/<name>`；同目录用 `./`、跨目录用 `../`，路径由单点函数产出） |
| `properties` 操作项 | `api-op` fence 的键集 + `x-function` |
| `properties` 类型项 | `$defs` 键集 → `{"$ref": "#/$defs/<name>"}` |
| `$comment` | 常量警示文本 |

- **不可手改**：生成物 SHALL 带警示（`$comment` 文本 / 首行注释），文案须说明「由 `<FQN>.md` 生成、手改将在下次 generate 时被覆盖、且零 diff 门会失败」。MUST NOT 为任何内容开口手改——`api-types` / `api-op` fence 的内容本身就是原生 JSON Schema（`minLength` / `pattern` / `enum` / `anyOf` / `x-*` 均可在 md 内表达），表达不了即生成器能力不足，应修生成器。
- **必须提交**：两份生成物 SHALL 入库（MUST NOT `.gitignore`）——零 diff 门需要 git 基线、工具可离线消费、审计型 review 可 `git diff --stat` 一眼看全。
- **索引独立于契约目录与模型声明目录（本变更）**：`INDEX.toml` SHALL 落 `<archi_graph_dir>`，MUST NOT 落 `<archi_models_dir>` 根（契约目录根只住契约资产），亦 MUST NOT 落 `<archi_graph_model_dir>`（模型声明目录）。判据是机械的：模型聚合的收面是「`graph/model/` 一级 + `.toml` 后缀」（`desktop-archi-model`「多文件聚合与声明定位」）且单文件反序列化 `deny_unknown_fields`——索引落模型声明目录根会被当模型文件解析，产出 `unknownField` 诊断并使模型停摆；`drift` 的 `modelHash` 取同一收面，索引落根还会改变全部漂移金样。故索引路径 SHALL 由 `paths::index_path(root)` 单点派生为 `<archi_graph_dir>/INDEX.toml`，MUST NOT 由契约目录 `join` 得到。
- **review 口径**：每次改契约产生多份 diff；因 schema 零手改自由度，review SHALL 只读 `.md`，schema diff 视为机械噪声——该口径 SHALL 落 `INDEX.toml` 的说明段（不写进散文约定）。
- `INDEX.toml` SHALL 按元素列出：`fqn` / `api` 状态 / 类型数 / 操作数 / 生成物路径 / `degraded` 计数 / `category` 分布，并含 review 口径说明段；条目序 SHALL 为 FQN 字典序（稳定序，便于断言与 diff）。说明段的「契约面：目录 …」串 SHALL 取 `<archi_models_rel_dir>`（契约资产所在的相对路径，MUST NOT 取索引自身所在目录——索引与契约已分家，写成索引目录会指错地方）。
- **派生前置（消费方约束）**：读 `schema.json` 的工具 SHALL 以 **draft 2020-12 校验器**配合 `strict: "log"` 或 `false`（`x-function` / `x-common` / `x-class` 是未知关键字，strict 模式必然报错），并 SHALL 先按 `$id` 逐个注册被引用 schema（跨文件 `$ref` 是结构性的，`schema.json` 不自包含）。

#### Scenario: 派生完备性逐字段可回溯

- **WHEN** 对一份含 3 个 op、5 个 `$defs`、1 条透传的契约执行生成，逐字段比对 `.md` 与 `.schema.json`
- **THEN** `title` = H1、`$defs` = 5 个本地定义 + 1 个纯 `$ref` 透传条目、`properties` = 3 个 `x-function` 项 + 5 个类型 `$ref` 项、`$id` = 文件名、`$comment` 为警示文本；不存在无 markdown 出处的字段

#### Scenario: 手改提交后零 diff 门持久失败

- **WHEN** 直接编辑已提交的 `<FQN>.schema.json`（不经 generate）后执行 `api:check`
- **THEN** 生成差异非空、检查以非零退出并列出该文件；回退手改后检查恢复通过；MUST NOT 出现「手改被接受」的路径

#### Scenario: 孤儿生成物在任意深度均被检出

- **WHEN** 删除元素或改其根路径后，在 `crates/core/`、`views/`、`a/b/c/d/` 等任意深度留下 `*.i.rs` / `*.i.ts` 残留，随后执行 `api:check`
- **THEN** 三类偏差中的「应删未删」逐条列出这些文件并以非零退出（元素根同级生成物的落点分布在任意深度，MUST NOT 只扫仓库顶层）；`target` / `node_modules` / `dist` / `build` / `.git` 里的同名文件不参与（非生成物的合法落点），普通源文件（`*.ts` / `*.rs` / `*.md`）零误伤

#### Scenario: INDEX 为纯投影

- **WHEN** 修改一份契约的类型数（加 1 个 `$defs` 条目）后重生成
- **THEN** `INDEX.toml` 该元素的类型数与 `degraded` 计数随动，其余元素条目逐字不变；条目序为 FQN 字典序；说明段的「契约面：目录」取 `<archi_models_rel_dir>`（与索引自身所在的 `<archi_graph_rel_dir>` 不同串）

#### Scenario: 索引落模型声明目录根被拒

- **WHEN** 手工把 `INDEX.toml` 复制到 `<archi_graph_model_dir>/INDEX.toml`（模型声明目录根）后跑一次模型解析
- **THEN** 该文件被当模型文件解析并产出 `unknownField` 诊断（`[[entry]]` 非模型字段），模型进入停摆态；`drift` 的 `modelHash` 随该文件变化——两处判红即本条的存在理由；删除该副本后恢复零诊断、`modelHash` 回到原值

### Requirement: 引用与命名规则

契约内的类型身份与引用关系 SHALL 遵循五条硬规则：

| # | 规则 | 违反后果 |
|---|---|---|
| 1 | 身份 SHALL 为 `$defs` **键名**，MUST NOT 为 pointer 路径 | 同名两处定义 → 契约错误 |
| 2 | 每个具名类型 SHALL 在顶层 `$defs` 定义恰一次；嵌套 `$defs` 一律提升且**原名保留** | 提升名碰撞 → 契约错误，禁静默改名 |
| 3 | 所有 `$ref` target SHALL 为 `$defs` 顶层条目 | 指向 `properties` / `items` / `allOf/0` → 契约错误 |
| 4 | 多个 `$ref` 指向同一类型时 SHALL 只生成一个定义 | 重复定义 → 契约错误 |
| 5 | MUST NOT 按结构去重（字段类型序列相同不等于同一类型） | 合并即语义错 |

- 规则 2 的命名理由：作者写的是 `B` 就还叫 `B`；加父前缀 `AB` 会让类型名取决于它「恰好被谁嵌套」，换个位置就改名，是静默漂移。嵌套 `$defs` 只是书写分组，MUST NOT 构成作用域。
- 规则 3 的理由：`properties/attr` / `items` / `allOf/0` 这些位置没有稳定名字——名字取决于父级结构，父级改一个属性名就变了。
- 规则 5 的理由：仓库实测 17 组「字段类型序列完全相同」的类型，其中 `ArchiDependsOn` / `ArchiDependedBy` 是**出入边、语义相反**，`Diagnostic` / `ArchiDiagnostic` 是进程内 vs 线面。合并等于把两个语义变成同一个类型；身份靠名字、去重靠名字。
- **公共面** SHALL 定义为「第一层 `properties` + 第一层经 `$ref` 可达的闭包」，该约定自动排除不出现在任何签名里的纯内部类型。
- **位置语义**：第一层 `properties` 为模块导出成员（`x-function` 操作 / `$ref` 到具名类型）；`$defs` 为本模块自有类型与透传纯 `$ref` alias。
- **跨模块 `$ref` SHALL 用相对文件路径**（同目录 `./<FQN>.schema.json#/$defs/<name>`；跨目录的前缀由两侧分层目录逐段求出——先上溯到公共祖先（每层 `../`）再逐段下行（`./<段>/`），因此既有 `../Infra/` 形态（同层跨子树）与 `../../Desktop/Components/`（跨层）由同一函数给出），路径由**单点函数**产出，MUST NOT 手算、MUST NOT 用自定义 URI scheme（实测 ajv 对 `module://X#/$defs/Y` 报 `can't resolve reference`，只认标准 URI）。

#### Scenario: 嵌套提升原名保留且碰撞报错

- **WHEN** 契约在 `Parent` 的嵌套 `$defs` 内定义 `B`，另一处顶层也定义 `B`
- **THEN** 提升后产生两个顶层 `B` → 契约错误并点名；MUST NOT 自动改名为 `ParentB` 后静默通过

#### Scenario: 键空间撞名即契约错误（不静默覆盖）

- **WHEN** 契约同时把同一名字用作 op id 与 `$defs` 类型名（`### T` + `api-types` 里的 `T`），或 `api-types` 与 `api-reexport` 各自声明同名条目 `T`
- **THEN** 装配期判契约错误并点名该名与冲突的键空间（`properties` / `$defs`）；MUST NOT 由后写者静默覆盖成其中一种形态——撞名意味着契约内部自相矛盾，产物只能有一个赢家，而下游无从知道有冲突

#### Scenario: `$ref` 指向非顶层被拒

- **WHEN** 契约写 `$ref: "#/properties/attr"` 或 `#/properties/x/items`
- **THEN** 契约错误，文案指明 target 不是 `$defs` 顶层条目

#### Scenario: 同结构不同类型不合并

- **WHEN** 契约声明两个 `type: object` 且 `properties` 序列完全相同的类型 `TurnQuestion` 与 `ProjectConfig`
- **THEN** 生成两个独立定义、`$ref` 各自指向自己的键；MUST NOT 合并为一条

#### Scenario: 跨模块引用解析成功

- **WHEN** 以 draft 2020-12 校验器按 `$id` 注册父子两份 `schema.json` 后编译父契约（含 `./Agent/<子 FQN>.schema.json#/$defs/<name>` 一类逐段相对引用）
- **THEN** 编译通过、无 `can't resolve reference`；两种加载顺序（父先 / 子先）均通过；`strict: "log"` 下不因 `x-*` 关键字报错

### Requirement: 关键字集、类型映射与错误面

契约 SHALL 只用标准 JSON Schema 关键字加**两个**扩展构造：

| 关键字 | 语义 | 生成 |
|---|---|---|
| `object` / `enum` / `const` / `default` / `oneOf` / `$ref` | 标准关键字 | 见映射表 |
| `x-class` | 声明为 trait，**仅允许方法**（出现属性即契约错误） | `pub trait` / `export interface` |
| `x-function` | 函数签名：`params` / `returns` / `async` | `pub fn` / `export type Fn = (…) => R` |
| `x-common` | 自由注记（`unit` / `note` / `since` / `owner` / `tag`）+ `degraded`（取值受封闭四值词表校验，见「降级词表与真实债表」）；除 `degraded` 的取值外不驱动生成、不加强校验 | 不变换 |

映射 SHALL 为：

| 协议 | Rust | TS |
|---|---|---|
| `object`（具名） | `struct`（每具名 schema 一 struct，建立名义性） | `export interface X { … }` |
| `enum` | `enum`（派生 `Deserialize`） | `export const X = [...] as const` + `export type X = (typeof X)[number]` |
| `oneOf` / `anyOf` | 带负载 `enum`（`#[serde(tag = …)]` 或 `untagged`） | union 类型 |
| `x-class` | `pub trait` | `export interface`（不强制实现 → 记 `degraded`） |
| 标量（`type: integer` 等） | `newtype struct`（`pub struct TimestampMs(pub i64)`） | 别名（**按设计退化，不记 degraded**） |
| 非 `required` 字段 | `Option<T>` | `{field}?: T`（**不拼 `\| null`**） |
| 显式 null（nullable 三分支任一）**且有非空分支** | `Option<T>` | `T \| null`（至多一次） |
| 纯 `null` 形态（`non_null` 缺省） | 保持既有兜底 `serde_json::Value`（MUST NOT 出 `()`） | `null`（MUST NOT 出 `unknown`） |
| `x-function` | `pub fn`（入参位 `&str` / `&[T]`，返回位 `String` / `Vec<T>`） | `(…) => R` / `Promise<R>`（`async`） |
| 错误面 `Outcome` 信封 | `Result<T, String>`（std 类型，不生成新类型） | `{ ok: T } \| { err: string }`，或 `Promise` reject（不在类型系统内 → 记 `degraded`） |

- **可选与可空 SHALL 是两个独立事实，且各只有一个判定点**：
  - **可空轴（schema 形状）**：`type` 内含 `null`（三分支任一：`"type": "null"` / 含 `"null"` 的 `type` 数组 / `anyOf`-`oneOf` 内含 `{"type": "null"}` 分支）= 可空。形状识别 MUST 只有一处（`nullable_breakdown`），判据面（`schema_type`）与两侧渲染面（`emit::{ts,rust}::value_type`）MUST 经它取数，MUST NOT 保留「判据认得、渲染不认得」的不对称——那会让契约写了 nullable 而产物落 `unknown` / `serde_json::Value`（判据绿、产物错，比两侧都不支持更危险）。**含非空分支**时三面同一判定：判据面 `ApiType::Optional` ⟺ TS 文本含 `| null` ⟺ Rust 文本含 `Option<T>`；**纯 `null` 档**（无非空分支）的 `serde_json::Value` 兜底是显式期望值、不算该病，见下。
  - **可选轴（字段语境）**：`required` 缺省 = 可选（TS `{field}?:`、Rust `Option<T>`）。判定点是字段级的 `required_of`，**不在** `schema_type` 射程内——`schema_type` 只吃 schema 片段、无 `required` 语境，故同一片段在必填位与可选位判出**同值**逻辑类型。本轴 MUST NOT 被折算成判据面差异（`schema_type` 的签名比对结果逐值不变），也 MUST NOT 用「Rust 文本含 `Option<`」当可空判据、MUST NOT 让 `schema_type` 为可选轴扩参。
  - **纯 `null` 形态（`non_null` 缺省）是独立一档**：判据面仍出 `Optional(Unknown("null"))`（既有行为逐值不变）；TS 侧渲染 `null`；Rust 侧保持既有兜底类型（`serde_json::Value`）。该兜底是本档的**显式期望值**，MUST NOT 记为 nullable 泄漏，也 MUST NOT 凭空造类型（如 `()`）；本档 MUST NOT 参与上面的三面等价式。
  - 三个约束跟进：① **去重到至多一层**——可空与可选叠加时 TS `| null` 至多一次、Rust `Option<T>` 至多一层，`Option<Option<T>>` 是退化产物 MUST NOT 出现；② **符号各自只落一处**——TS 侧 `?` 只由 `interface_block` 出、`| null` 只由 `value_type` 出；③ **数组元素位为可空 MUST 加括号**（`(T | null)[]`，MUST NOT 出 `T | null[]` 这一优先级错误形态）。
- **两个扩展构造不可删**：`x-function` 是唯一能区分「模块操作」与「函数指针字段」的标记（JSON Schema 无表达函数的标准方式）；`x-class` 消灭「形状推导误判 trait」的静默故障。反向 SHALL 产**警告**（非错误）：`properties` 全为函数却未标 `x-class` → 提示可能意图为 trait（函数指针结构体是合法意图，不能一刀切拒绝）。
- **标量 MUST NOT 生成为 trait**：`pub trait TimestampMs {}` 的名义性为零（任何类型都能 impl）；`newtype` 才是零开销名义类型。名义化需求真实：实测 `i64`/`u64` 字段 162 处（时间戳簇 62、db id 18+、计数 21+、时长 4），这些 newtype SHALL 落**各模块 `$defs`**，MUST NOT 进 `common.json`。单位须进名称或 `x-common.unit`。
- **位置分派 SHALL 是生成器内部表，MUST NOT 成为对外门禁**：入参位 `string` → `&str`、返回位 → `String`、字段位 → `String`、入参位 `list<T>` → `&[T]`；偏离携带语义（入参位用 `String` 的多为存储型），须在契约登记理由；**但位置词汇与签名短形 MUST 各只有一份**：`Position`（`Param` / `Return` / `Field`）与短形写出点（`str` / `String` / `list<T>` / …）各恰一处，判据侧经 `schema_type` + `signature_of` 取数、MUST NOT 自建第二份位置表或第二份短形拼装（否则可选参数 `optional: true` 与代码 `Option<T>` 的归一必然漏分支、稳定假阳）；`emit` 仍独占「位置 × 语言 → 文本形」的映射。
- **构造器 MUST NOT 进契约**：契约 `### 操作` MUST NOT 声明构造器（`new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open`）——构造器是类型的推论，由生成器从 `$defs` 条目自动产出。违反 SHALL 报 `apiConstructorDeclared`（见 `desktop-archi-api-check`）。
- **枚举列举 SHALL 由函数承担**：需要运行期列举时声明普通函数（如 `all() -> list<LocalEnum>`），**函数体由枚举派生**（语义条件，非标记）；MUST NOT 设计 `x-singleSource` 一类标记，也 MUST NOT 让生成物产出 `pub const ALL`。
- **泛型 MUST NOT 支持**：契约写泛型参数即契约错误。
- **TS 侧 MUST NOT 做品牌类型**：`number & { __brand }` 会让 cast 传染整条调用链，收益只是防误传；TS 侧「实质同类型即可」。
- **`common.json` 的边界**：随工具发布、`include_str!` 编译期内嵌；它 SHALL 只承载**映射规则**（当前仅 `Outcome` 信封），生成时就地展开为各语言标准类型 ⇒ 零生成物、零 vendor 文件、零版本声明。一旦想往里加「带生成产物的类型」（而非映射规则），MUST 回到设计层重裁分发方案，MUST NOT 就地新增。降级词表同理 MUST NOT 进该资产（它是判据词表，不是映射规则）。
- **生成物头 SHALL 带工具版本与 `common.json` 摘要**（如 `// 生成于 archi-api 0.4.2 · common.json@sha256:1a2b…`），使「工具升级 → 批量重生成」的变更来源可分辨。
- **契约文本进生成器 SHALL 经两道人字闸，且 `x-function` 的落点 SHALL 封闭二处**：① 装载期（`contract::verify`）**七族名字**——op id / `$defs` 键 / 透传键 / `params[].name` / `$defs` 条目的 `properties` 键 / `api-op` 的 `owner` / `$ref` 末段名——MUST 是 ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*`，不成立即契约错误并点名族别与 fence 行；**同趟遍历另判 `x-function` 的落点**——MUST 只在 `api-op` 顶层签名与 `$defs` 条目第一层 `properties` 的值二处，其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 及其更深层）即契约错误并点名 JSON pointer（位置类错误的正确处置是**在契约里搬位置**，不是在渲染期兜底）；② 渲染期一切来自契约的**字符串字面量与注释** MUST 经该语言单点转义（单引号形态不变，MUST NOT 用会产出双引号的方式生成），Rust 侧枚举值经无损驼峰归一。MUST NOT 在渲染期重复校验名字或位置（装载期已保证，`emit/{rust,ts}.rs` 内零标识符判据、两侧 `value_type` 零 `x-function` 分支是它的机械判据）。
- **四条明确不做**：`x-interface` / `x-nominal` / `x-singleSource` 独立构造（分别由 `x-class`、标量形状、函数返回消解）；自造协议记法（`[op.xxx]` 信封 / `module://` scheme）；L/S 双层锁 + 归一函数 + 偏离登记表（`verify ≡ generate + 类型检查`）；Zod 作通用载体。

#### Scenario: 函数指针字段与模块操作可辨

- **WHEN** 契约同时含 `x-function` 操作 `parse` 与一个 `type: object` 且 `properties.parse` 为 `x-function` 的结构体字段
- **THEN** 前者生成为模块级 `pub fn`，后者生成为 `pub parse: fn(&str) -> …` 字段；两者不互相吞并

#### Scenario: `x-class` 带属性即契约错误

- **WHEN** 某 `$defs` 条目标 `x-class` 但同时声明 `properties: { "count": { "type": "integer" } }`
- **THEN** 契约错误（trait 仅允许方法）；去掉 `x-class` 改为普通 `object` 后通过，生成 `struct`

#### Scenario: 标量生成 newtype 而非 trait

- **WHEN** 契约声明 `TimestampMs: { "type": "integer", "x-common": { "unit": "ms" } }`
- **THEN** Rust 侧生成 `pub struct TimestampMs(pub i64)`（非 `trait`）；TS 侧生成 `export type TimestampMs = number` 且不记 `degraded`

#### Scenario: 契约声明构造器被拒

- **WHEN** 契约含 `### new` + `api-op` fence
- **THEN** 报 `apiConstructorDeclared` 并点名该 op；删掉该段后生成物仍自动产出 `new()`（构造器双向都不参与判据比对：既不在契约声明集，也不计入代码导出集）

#### Scenario: 错误面显式留痕

- **WHEN** 契约声明返回 `$ref: "#/$defs/Outcome"`，TS 侧实现以 `Promise` reject 表达错误
- **THEN** Rust 侧生成 `Result<T, String>`；TS 侧生成 union 且该类型记 `x-common.degraded`（错误通道不在类型系统内，必须显式留痕）；`INDEX.toml` 的 `degraded` 计数 +1

#### Scenario: 显式 null 六组上判据面与两渲染面同一判定

- **WHEN** 对六组**含非空分支的显式 null** 样例——`["string","null"]` 两元素 / `["string","integer","null"]` 三元素 / `anyOf` 含 null 分支 / `oneOf` 含 null 分支 / 非 `required` 且显式 null 叠加 / 数组元素位 nullable（在元素位对拍）——逐组取 `schema_type` 与两侧渲染文本
- **THEN** 六组逐组同真同假：`schema_type` 判 `ApiType::Optional` ⟺ TS 文本含 `| null` ⟺ Rust 文本含 `Option<`；`unknown` 与 `serde_json::Value` 均零命中（这一组对拍即「判据绿而产物错」的判红面）

#### Scenario: 可选轴不进判据面且各落自己的符号

- **WHEN** 同一 `$defs` 条目的两个字段使用**同一 schema 片段** `{ "type": "string" }`：`a` 在 `required` 内、`b` 不在
- **THEN** `schema_type` 对两片段取**同值**（`OwnedString`——`schema_type` 无 `required` 语境，可选轴 MUST NOT 折算成判据面差异）；TS 侧为 `a: string;` / `b?: string;`（`a` 零 `?`、`b` 零 `| null`）；Rust 侧为 `pub a: String,` / `pub b: Option<String>,`；本轴 MUST NOT 用「Rust 含 `Option<`」当可空判据

#### Scenario: 纯 null 组按显式兜底且不参与可空对拍

- **WHEN** 对样例 `{ "type": "null" }` 取 `schema_type` 与两侧渲染文本
- **THEN** 三侧分别为 `Optional(Unknown("null"))`（既有行为逐值不变）/ TS `null`（MUST NOT 是 `unknown`）/ Rust `serde_json::Value`（既有兜底，本档显式期望值，MUST NOT 记为泄漏，MUST NOT 出 `()`）；本档 MUST NOT 参与「`Optional` ⟺ `| null` ⟺ `Option<`」的对拍

#### Scenario: 可选与显式 null 各落自己的符号且叠加去重

- **WHEN** 同一契约声明三个字段：`a`（在 `required`、非 null）/ `b`（不在 `required`、非 null）/ `c`（不在 `required`、类型为 `["string","null"]`）
- **THEN** TS 侧为 `a: string;` / `b?: string;` / `c?: string | null;`（`c` 的 `| null` 恰一次）；Rust 侧为 `pub a: String,` / `pub b: Option<String>,` / `pub c: Option<String>,`——`Option<Option<` 零产出

#### Scenario: 数组元素位可空加括号

- **WHEN** 契约声明字段类型 `{ "type": "array", "items": { "type": ["string","null"] } }`
- **THEN** TS 侧渲染 `(string | null)[]`（带括号），MUST NOT 渲染 `string | null[]`（优先级不同、语义错）；Rust 侧渲染 `Vec<Option<String>>`

### Requirement: 生成物落点与入口查找

生成物 SHALL 落**实现入口文件同级**（`<入口主干>.i.rs` / `<入口主干>.i.ts`），其中 `<入口主干>` SHALL 为入口文件名的**去扩展名形态**（`crates/infra/archi/src/lib.rs` → `lib`、`views/welcome-view.tsx` → `welcome-view`、`packages/desktop/src-tauri/src/lib.rs` → `lib`）。入口 SHALL 由本节的三态解析链给出，生成物路径因此是**入口的单值函数**；元素根路径（`path.first()`）MUST NOT 再参与落点派生（它只回答「根在哪」）。

- **`<root>` 归一在模型解析层完成**：`<root>` SHALL 为 `path.first()` 经 `normalize_root` 归一后的形态（`OwnerIndex::normalize` 已处理 `./` 前缀与尾 `/`），MUST NOT 批量改写存量 models——实测存量 46 条 `path` 全部带 `./` 前缀（目录型根另带尾 `/`）、一律需要归一（文件型根的扩展名是「根即入口」的判据，故归一 MUST NOT 去扩展名）。
- **入口解析链封闭三态，MUST NOT 增分支**（候选派生**零 IO**，只做字符串运算）：

  ```
  entry_candidates(element) -> Vec<String>      纯文本、按优先序
    ① 显式 entry（归一后）                      → [entry]                          唯一候选
    ② 文件型根（末段扩展名 ∈ {rs, ts, tsx, js, jsx}）→ [<root>]                   根文件自身即模块
    ③ 目录型根（语言无关，按序取首个落盘者）
       → [<root>/src/lib.rs, <root>/mod.rs, <root>/index.ts, <root>/index.tsx]
    ④ 其余（`path` 空 / `<root>` 形态非法）      → []

  entry_state(root, elements, element) -> EntryState        读盘 + 模型集
    Resolved(<EntryForm>, <入口>)  首个落盘的候选，且落位 / 跨元素唯一两条判据通过（形态位由
                                    「入口与根的关系」派生，供 `EmitTarget.form`）
    Invalid(<problem>) 显式候选不在盘（`NotOnDisk`）/ 候选落在 `path` 之外（`OutsidePath`）
                       / 与另一元素声明的入口同一文件（`Duplicated`）/ 形态非法（`Malformed`）
    Absent             候选为空（未声明且无约定默认），或目录型根的默认候选全不落盘
  ```

  `<root>` SHALL 为 `path.first()` 经 `normalize_root` 归一的形态（`\` → `/`、剥 `./`、去尾 `/`，**MUST NOT 去扩展名**——文件型根的扩展名正是「根即入口」的判据）。**③ 不做语言判定**：目录型根没有扩展名（`language_of` 对它是 `Unknown`），故四种约定形态一律列出、按序取首个落盘者——四种形态互不相交（同一目录不可能既是 crate 又是视图），因此「按序」不构成语言猜测，也**不**退化成 R2 的列目录规则（R2 已明确否决：新增一个源文件就换默认入口，正是本变更要消灭的漂移源）。`<dir>/lib.rs` MUST NOT 在候选列内（crate 的 `lib.rs` 恒在 `src/` 下）；`<dir>.rs` / `<dir>.ts` / `<dir>.tsx` 三条 MUST NOT 在列（对目录型根它们只能指到 `path` 之外的兄弟文件，必违落位判据）。
- **`entry` 的四条判据**（字段定义见 `desktop-archi-model`「架构模型文件格式与字段封闭」）：**基准** repo 相对 POSIX 路径（与 `path` 同口径，`./` 前缀不参与身份）；**落位** MUST 等于某条 `path` 条目或落在某条**目录型** `path` 之内（单文件根元素声明「同目录的另一个文件」也判 `OutsidePath`）；**存在** 声明或默认命中的文件 MUST 在盘上；**跨元素** 任何两个元素 MUST NOT 解析到同一入口文件。四条违反一律走 `EntryState::Invalid`，由 `apiEntryMissing` 报出（语义见 `desktop-archi-api-check`），MUST NOT 进结构化诊断（九类诊断会让整图停摆，过重）。
- **「无入口」是合法终态**：`Absent` 即该元素不参与契约面（`apiEntryMissing` 的语义：**未声明且无约定默认**）。本仓现基线**恰一例**（本变更新增的生成物中继元素 `Desktop.GeneratedBindings`：`path` 指向生成目录，目录内是三个叶子文件、没有 `index.ts(x)`，且它 `api = none` 已退出入口判据 ⇒ **零 `apiEntryMissing`**）；其余元素由 ①②③ 命中。该状态仍然合法（新加目录元素在其入口落地前就会落此态），MUST NOT 用 `api = none` 冒充——两者是不同的事实（「没有门」vs「明说无公共 API」）；反向也 MUST NOT 为消一个 `Absent` 而给生成目录造 `index.ts`（生成目录不是模块入口，造了即造假入口）。
- **`<name>` 的旧口径只剩透传命名面**：`module_name(<root>)`（路径末段）SHALL 继续只服务 Rust / TS 透传命名（D40），MUST NOT 参与生成物文件名——两者的判据不同（`Backend.Infra.ArchiRuntime` 的 FQN 末段是 `ArchiRuntime`、路径末段是 `archi`、入口主干是 `lib`）。
- **不强制模块成目录，也不禁止目录成模块**：前端 14 个单文件根元素零模型改动——② 让「根文件自身」成为入口；目录型根的门由约定默认（③）或显式 `entry` 给出。反向同理：目录型根补一个 `index.ts(x)` 即由 ③ 命中，**无需**声明 `entry`（本仓 12 个目录模块入口即此形态，见 `repo-archi-model`「基线落位与模型文件划分」）；本能力 MUST NOT 因「目录型根没有入口」要求新增字段或解析分支。
- **入口编排 SHALL 固定为**：入口文件顶部 `#[path = "<文件名>"] mod api_gen; pub use api_gen::*;`，既有 `pub mod` 声明原样不动。生成类型经 `pub use` 对外可引用；与既有同名类型冲突时编译器直接报 E0255，**不会静默覆盖**。
- `#[path]` 的值 SHALL 为**字面量**，且 SHALL 恒为**入口同级的生成物文件名**——生成物与入口同目录，故值里零 `../`、零目录段：

  | 入口 | `#[path]` 基准 | `#[path]` 值 |
  |---|---|---|
  | `<dir>/src/lib.rs` | 同目录 | `"lib.i.rs"` |
  | `<dir>/index.ts` | 同目录 | `"index.i.rs"` / `"index.i.ts"` |
  | `<root>.tsx`（文件型根） | 同目录 | `"<主干>.i.ts"` |

  等值判据 SHALL 由 `paths_test` 直接断言「入口行的字面量 == `generated_path(entry).file_name()` == `<入口主干>.i.<后缀>`」——**已删**的 `paths::mod_path_attr` 不再是真值源（入口同级后属性值恒为文件名，再留一层函数只是间接：与 D40/D41「零二次派生」同一条纪律）。MUST NOT 手写与入口主干不等值的形态。**`<dir>/index.ts` 这一行在本仓仍零实例**（12 个目录模块的入口都是 TS 侧、无 `#[path]` 行），它描述的是规则而非现状。
- **`#[path]` 的读盘面 SHALL 与写盘面同一条限根纪律**：取数器定位 `#[path = "…"] mod` 时 MUST 满足三条——① **反斜杠归一在折叠之前**（否则反斜杠形态被当作单段名，`Path::join` 的纯拼接会把 `..` 原样带到 Windows 的路径解析里、逐级上溯到观察根之外）；② 分段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即拒）；③ **`join` 后复核**目标仍在观察根之下（`starts_with(root)`）。任一条不成立即**不命中该子树**（与「模块定位失败即跳过」同口径，MUST NOT 新增错误变体、MUST NOT panic）。理由是 `#[path]` 的值经**手写入口行**落地（属性的值只能是字面量，等值性由机械断言守），属可被写错的入参；放行即让「模块树被带出观察根」，与 `paths` 侧的「任意文件读」是同一件事。判据与 `scan` 的 `normalize_rel` / `docs` 的 `resolved_within` 同一条纪律。
- **TS 侧 `import type` 路径 SHALL 遵循同一「入口同级」规则**：同目录目标为 `./<目标入口主干>.i`；跨目录 SHALL 由**入口到入口**的相对路径派生（与 `ref_relative` 同族的单点函数，MUST NOT 手算 `../` 层数）。**本期显式留白**：`.i.ts` 生成物在当前仓库为零件（12 个目录模块入口**不产生** `.i.ts`——它们零契约文件）⇒ 该规则仍无实例可验；`paths::ts_import_specifier` 与 `EntryForm` 三值保留为**待接线形态**，其当前输出（`../<name>.i` 形态）MUST NOT 被当作「入口同级」口径的真值，接线与跨目录派生归前端契约变更。留白 MUST NOT 被写成已实现。
- **MUST NOT 生成 `_ext.rs`**：生成物只含类型定义（`struct` / `trait` / `enum` / newtype）与自动构造器；`impl` 块写在模块内任意手写文件（Rust 允许同 crate 内 `impl` 分散）。生成 / 手写分离靠**文件命名**（`.i.rs` / `.i.ts` 后缀）而非目录分离。
- **归属规则 SHALL 为「三候选并集」**：`.i.<ext>` 生成物（`ext ∈ {rs, ts, tsx, js, jsx}`，标记段恰为 `i` 且模块名非空）的归属 SHALL 由**三个剥离去向取并集**判定——

  ```
  候选 1  整组剥离    <name>.i.<ext>  →  <name>         archi.i.rs → archi
  候选 2  替换扩展名  <stem>.i.<ext>  →  <stem>.<ext>    archi.i.rs → archi.rs
  候选 3  主干对齐    <name>.i.<ext>  →  <name> 与**文件型 `path` 的去扩展名形态**全等
                                                       views/welcome-view.i.ts ↔ views/welcome-view.tsx
  命中判据 = 任一候选满足 rel == prefix 或 rel.startsWith(prefix + "/")
  ```

  理由（三种落点形态互不相交，单条规则覆盖不了）：生成器产出**入口同级**——目录型根下它仍在元素根之内（候选 1 命中，如 `crates/infra/archi/src/lib.i.rs`；`<dir>/<name>.i.<ext>` 的目录内形态由候选 1 一并覆盖）；**文件型根**下它是根的兄弟文件（`views/welcome-view.i.ts`），与 `path` 全名（`views/welcome-view.tsx`）既不相等也不互为前缀，只有候选 3 能命中；候选 2 保留为既有形态（`src/dir.ts` 根对 `src/dir.i.ts`）不做删除。目录型 `path` 的目录模块入口（本仓 12 个 `index.ts(x)`）其生成物为 `<dir>/index.i.ts`，由候选 1 命中（`<dir>` 前缀之内）——**该形态在本仓仍零实例**（无契约即无生成物）。判据改造 SHALL 只此一处（`OwnerIndex` 的剥离函数与其调用点），`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不动，既有非 `.i` 路径的归属语义零变化。
- **并集 MUST NOT 实现为候选短路**：命中集 SHALL 为「对每个候选各跑一遍最长前缀后合并去重」，再按前缀长度定档（同长仍归 `Ambiguous`）；MUST NOT 写成「候选 1 命中即返回」——那会让 `src/dir.i.ts`（候选 1 全等 `src/dir`、候选 2 得 `src/dir.ts`）只走一条路径，与并集语义不符。定档用例：元素根 `src/dir/` 与 `src/dir.ts` 同时在场、探针 `src/dir.i.ts` ⇒ 归更具体的单文件根（前缀更长者胜出）；`src/a.ts` 与 `src/a.tsx` 同时在场、探针 `src/a.i.ts` ⇒ 两文件型根的主干同长，归 `Ambiguous`（MUST NOT 任选）。
- **白名单外后缀零误剥**：`.i.py` / `.d.rs` / 大写 `.I.rs` / 无扩展名 `.i` SHALL 原样返回单个候选（不剥），归属与改动前逐值相同。
- **父子路径重叠不缩窄父契约**：子元素根路径是父元素根路径的前缀，最长前缀匹配已正确处理（`.../src/scan/x.rs` 归 `Scan`、`.../src/dto.rs` 归 `Archi`；`components/agent/` 与 7 个 `components/agent/*.tsx` 子元素的文件型根同理——子元素优先，`index.tsx` 归目录模块）。
- **无可用入口即 `apiEntryMissing`**：`EntryState` 为 `Absent` 或 `Invalid(_)` ⇒ 该元素不是可用模块 → 报一条 `apiEntryMissing`（定义与档位见 `desktop-archi-api-check`）。它比 `apiMissing` 更基础——**在契约生成之前就失效**，防止给不存在的模块写契约。`.i` 生成物的存在 MUST NOT 再当作入口的替代证据：生成物路径是入口的函数，无入口即无生成物路径可派生。**例外恒定**：`api = none` 元素退出入口判据（见 `desktop-archi-api-check`「`api = none` 元素退出入口判据」），故本仓唯一那例 `Absent`（生成物中继元素）零 `apiEntryMissing`。

#### Scenario: 入口解析链三态各命中一条

- **WHEN** 分别对「显式声明 `entry = "packages/desktop/src-tauri/src/lib.rs"` 的壳元素」、「根为文件型 `views/welcome-view.tsx` 的元素」、「根为目录型 `crates/infra/archi/` 且盘上有 `src/lib.rs` 的元素」、「根为目录型 `packages/desktop/src/views/changes/` 且盘上有 `index.ts` 的元素」取 `entry_state`
- **THEN** 四者均为 `Resolved`，入口分别为声明的路径、根文件自身、`crates/infra/archi/src/lib.rs`、`packages/desktop/src/views/changes/index.ts`（③ 的第三候选）；再对根为目录型而 `index.ts` / `index.tsx` 皆不在盘的元素取同一函数 → `Absent`；五例均不 panic、`Absent` 一例不产生成物路径

#### Scenario: 四条判据各一反例

- **WHEN** 分别构造四个反例：`entry = "/abs/x.rs"`（绝对路径）、`entry` 指向另一个元素的 `path` 内的文件、`entry` 指向不存在的文件、两个元素声明同一入口文件；再补一例「文件型根元素声明同目录的另一个文件」
- **THEN** 依次为 `Invalid(Malformed)` / `Invalid(OutsidePath)` / `Invalid(NotOnDisk)` / 双方各 `Invalid(Duplicated)` / `Invalid(OutsidePath)`；五例各自的 `apiEntryMissing` 文案点名具体理由（越界目标 / 缺失文件 / 冲突方 FQN），MUST NOT 只写通用「无入口」

#### Scenario: 生成物落入口同级且 #[path] 无目录段

- **WHEN** 对 crate 形态（入口 `crates/infra/archi/src/lib.rs`）、子目录模块形态（入口 `crates/core/archi/src/scan/mod.rs`）、文件型根（入口 `views/welcome-view.tsx`）分别取 `paths::generated_path` 与入口行字面量
- **THEN** 三者落点为 `crates/infra/archi/src/lib.i.rs`、`crates/core/archi/src/scan/mod.i.rs`、`views/welcome-view.i.ts`（= 各自 `<入口主干>.i.<后缀>`，与入口同目录）；`#[path]` 字面量逐字等于 `generated_path(...).file_name()`（`"lib.i.rs"` / `"mod.i.rs"`），含 `../` 或目录段的形态即判红；`paths::mod_path_attr` 在产品源码内零命中（函数已删）

#### Scenario: 三种落点形态各归其元素

- **WHEN** 分三组求归属：① 入口同级的目录型根落点（`crates/infra/archi/src/lib.i.rs` 对目录形前缀 `crates/infra/archi/`）；② 目录内形态（`views/architecture/architecture.i.ts`、`crates/core/archi/src/scan/scan.i.rs`）；③ 入口同级的文件型根落点（`views/welcome-view.i.ts` 对文件形前缀 `views/welcome-view.tsx`；目录模块入口 `components/agent/index.i.ts` 对目录形前缀 `components/agent/`，候选 1 命中）
- **THEN** 三组用例各归其元素（①③ 由整组剥离 + 主干对齐命中、② 由整组剥离 / 替换扩展名命中，三者并集）；非 `.i` 路径（如 `crates/core/archi/src/dto.rs`）归属结果与改造前逐字相同

#### Scenario: 并集定档不短路

- **WHEN** 元素根 `src/dir/` 与 `src/dir.ts` 同时在场，探针为 `src/dir.i.ts`；另一组为 `src/a.ts` 与 `src/a.tsx` 同时在场、探针 `src/a.i.ts`
- **THEN** 前者归更具体的单文件根（`src/dir.ts` 所在元素，前缀更长）；后者两文件型根主干同长 ⇒ `Ambiguous`（MUST NOT 任选）；若实现成「候选 1 命中即短路」，前者会误归目录形根——该差异即本 Scenario 的判红面

#### Scenario: 白名单外后缀不误剥

- **WHEN** 对 `src/mod/mod.i.py`、`src/mod/mod.d.rs`、`src/mod/mod.I.rs`、`src/mod/mod.i` 求归属（元素根 `src/mod/`）
- **THEN** 四者按原路径判定归属（不剥），结果与改动前逐值相同；`.i.<白名单外后缀>` 不产生第二 / 第三个候选

#### Scenario: 本仓入口面实测 46 / 45 / 1 / 0

- **WHEN** 对本仓 46 个 `path` 非空元素跑 `entry_state`
- **THEN** `Resolved` **45** 条：14 个自研 crate 由 ③ 命中 `src/lib.rs` + 多文件根 `Desktop.Shell` 由 ② 命中 `app.tsx` + **14** 个前端单文件根由 ② 命中自身（7 个 `components/agent/*.tsx` + 6 个 `standard/*.tsx`（含 `combobox.tsx`）+ `views/welcome-view.tsx`）——其中目录模块 `Desktop.Components.Agent` 改由 ③ 命中 `components/agent/index.tsx` + **12** 个目录模块根由 ③ 命中 `index.ts(x)` + 壳 `Backend.Shell` 与两个工具 crate 由 ① 命中显式 `entry` = **45**；`Absent` **1** 条（`Desktop.GeneratedBindings`：`path` 为生成目录、目录内无 `index.ts(x)`，且它 `api = none` ⇒ **零 `apiEntryMissing`**）、`Invalid` **0** 条。三处显式 `entry` 仍是本节 ① 在本仓的**唯一实例**（12 个目录模块入口走 ③，MUST NOT 为其补 `entry` 行；生成物中继元素也 MUST NOT 为它补 `entry`——生成目录的叶子文件不是模块入口）；反向判据：删掉任一目录的 `index.ts` 即该元素回落 `Absent` 并产一条 `apiEntryMissing`，说明「入口面清零」是**文件在盘**的结果而不是判据放宽

#### Scenario: 生成物中继元素的 Absent 不是债

- **WHEN** 对生成物中继元素（`kind = module` / `api = none` / `path = ./packages/desktop/src/types/generated/`）跑一次全量检查
- **THEN** 它零 `apiEntryMissing`（`api = none` 退出入口判据）、恰一条 `docMissing`（`doc` 未声明）；给它造一个 `index.ts` 会让 `entry_state` 转 `Resolved` 但**不减少任何判据条目**（`docMissing` 仍在）——故「造入口」不是可接受的处置，MUST NOT 在本变更或后续变更里为消 `Absent` 而造生成目录的 `index.ts`

#### Scenario: 生成物与手写 impl 共存编译

- **WHEN** 入口文件 `crates/infra/archi/src/lib.rs` 写 `#[path = "lib.i.rs"] mod api_gen; pub use api_gen::LocalArchiCheck;`，同级生成物 `crates/infra/archi/src/lib.i.rs` 含 `pub struct LocalArchiCheck;` + `impl` 内 `new()`，手写文件另有 `impl Default for LocalArchiCheck`
- **THEN** 编译通过（`rustc --edition 2021`），外部可经入口路径引用 `LocalArchiCheck::new()` 且签名受生成物约束

#### Scenario: 同名冲突不静默覆盖

- **WHEN** 生成物定义了元素内已手写的同名类型且入口 `pub use api_gen::*;`
- **THEN** 编译器报 E0255（重复定义），MUST NOT 静默采用其一

#### Scenario: 路径形态非法即拒收（安全边界）

- **WHEN** 元素 `<root>`、入口候选或契约 FQN 取绝对路径 / 含盘符 / 含 `..` 段 / 含空段 / 为空串（如 `path = ["/abs/elsewhere"]`、`path = ["crates/../../outside"]`、`entry = "../x.rs"`、`fqn = "App..X"`）
- **THEN** `contract_path` / `schema_path` / `generated_path` / `entry_state` 一律返回空或 `Absent`（`Malformed` 形态归 `Absent` / `Invalid`），不 panic、不落到仓外；`root.join(绝对路径)` 的「替换 base」与 `..` 的「上溯」因此结构上不可达——「证据在盘」的探测不会变成任意文件读，工具 crate 的写盘不会变成任意文件写。**第二层**：`archi-api-gen` 的写盘函数 SHALL 另以目标须落在观察根之下（`starts_with(root)`）复核，越界即拒绝并零副作用（两层判据交叉验证，任一层失效另一层仍拦得住）

### Requirement: 生成物入仓与零 diff 门挂载

四类生成物 SHALL 全部入仓并进零 diff 门，形态沿既有 `bindings.ts` 先例（MUST NOT 另造第二套守卫机制）：

| 生成物 | 落点 | 入库 | 门禁 |
|---|---|---|---|
| `<FQN>.schema.json` | `<archi_models_dir>/<分层目录…>/` | 是 | `api:check` 零 diff |
| `INDEX.toml` | `<archi_graph_dir>/`（全局唯一一份） | 是 | `api:check` 零 diff |
| `<入口主干>.i.rs` | **元素实现入口同级**（`crates/infra/archi/src/lib.i.rs`） | 是 | `api:check` 零 diff + `cargo check` |
| `<入口主干>.i.ts` | **元素实现入口同级**（`views/welcome-view.i.ts`） | 是 | `api:check` 零 diff + `tsc --noEmit` |

- **`INDEX.toml` 的 `generated` 列取入口同级路径**：无入口（`Absent` / `Invalid`）的元素该列留空——生成物路径是入口的函数，无入口即无路径可写。
- **孤儿扫描 SHALL 覆盖两个根**：契约目录侧的 `*.schema.json`（`<archi_models_dir>/**` 递归）与索引侧的唯一 `INDEX.toml`（`<archi_graph_dir>/`）。两处的枚举语义 SHALL 逐条同形——深度上限防环、符号链接不跟随、`read_dir` 只出现在有界枚举里、repo 相对路径串由同一 `display` 单点产出；MUST NOT 出现第二份手写递归（两处差异只允许是「根不同、收面后缀不同」）。
- **脚本面封闭两条**：`api:export`（`cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`，写盘）与 `api:check`（同一调用加 `--check`；生成到内存后比对、**零写入**，差异即非零退出；比对由工具内完成，**不依赖任何 `git diff` 段**）。`check` 链 SHALL 为四段：`server:check && bindings:export && api:check && client:check`；`bindings:export` 与 `client:check` 两段命令形态零变化，`server:check` 的 fmt 段为校验模式 `cargo fmt --all --check`。**第二段 SHALL 是 `bindings:export`**（`check` 的生成物零 diff 守卫已退役，见 `desktop-ipc-type-bindings`——`check` 只前置重导出、不比对）；规范内 MUST NOT 引用任何已退役的守卫脚本名；段名 SHALL 以 `package.json` 的 `scripts` 实存键为准（`grep -c '"bindings:export"' package.json` ≥ 1 即证该键在案）。
- **应用内两驱动面与脚本面同源**：生成物的渲染与三类差异比对 SHALL 各有唯一实现（`core/archi` 的渲染 / 比对纯函数），写盘 SHALL 有唯一实现（`infra/archi` 的写盘单点）；应用内两条 IPC（检查 / 生成，见 `desktop-archi-contract-panel`）与 CLI 两子命令 MUST NOT 各写一份渲染、比对或写盘——**四条消费路径共用同一实现**是「零 diff 门」与「应用内检查」结论一致的前提。脚本面两条命令形态本文逐字不变（仅落点换段，命令串不变）。
- **`knip` 边界**：`knip.json` 的 `ignore` SHALL 含 `src/**/*.i.ts`（生成物非手写代码；实测未被 import 的 `.i.ts` 会被报为 unused file，因 `project` glob 覆盖 `src/**/*.ts`）；MUST NOT 新增 `ignoreExportsUsedInFile` 或 `ignoreDependencies` 条目（那是关判据而非满足判据）。
- **类型绑定再生成**：`ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` 与报告字段 `apiFindings` SHALL 经 `bindings:export` 进 `bindings.ts` 与 `bindings.zod.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役）；本能力 MUST NOT 手写前端类型镜像。**本变更零 DTO 形状改动，但绑定文件不是零 diff**：`dto.rs` 的 doc 方位串换段（`ApiStatus` / `ArchiApiRef::contract_path`）经 specta 原样抄进 `bindings.ts` ⇒ 该文件恰 **2 行 doc 注释**随动（diff 全落在注释内），形状面（类型名 / 联合成员 / 字段名 / 字段序 / `null` 语义）SHALL 逐字不变、`bindings.zod.ts` SHALL 零 diff、`bindings:export` SHALL 幂等（重复导出不再产生第三行 diff）。**MUST NOT 把本变更表述为「零 DTO 形状改动 ⇒ 绑定零再生成」**——两者不是等价命题，出现形状面 diff 或 zod 面 diff 才是误动。
- **TS 侧形状校验的机制前提**：实现侧 SHALL `import type` 引用生成的 `.i.ts` 并标注返回类型（否则校验不发生）；`isolatedModules` 下重导出须写 `export type { … }`。
- **版本纪律**：本变更改 `packages/desktop` 源码 ⇒ `package.json` 的 `version` SHALL 相对**落地时 `HEAD` 的现值 +1 patch**（相对递增，规范内 MUST NOT 写死具体版本号——现值随每次源码改动滚动，写死即失效）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

#### Scenario: 四类生成物齐入仓且零 diff

- **WHEN** 执行 `api:export` 后再执行 `api:check`
- **THEN** 四类生成物均在盘且已在 git 内，第二次检查零差异、退出码 0；`git status` 干净

#### Scenario: 手改任一生成物即被拦

- **WHEN** 手改已提交的 `<FQN>.schema.json`、`INDEX.toml`、`.i.rs` 或 `.i.ts` 任一后执行 `api:check`
- **THEN** 检查以非零退出并列出被改文件（`--check` 阶段无写盘）；回退后恢复通过

#### Scenario: 落点迁移后旧位置零残留（机制与判红面分列）

- **WHEN** 落点换段完成后审查旧位置 `<archi_dir>/api/**` 与产品 `.rs` 的旧段名
- **THEN** `<archi_dir>/api` 目录不存在（`ls` 零命中）；产品 `.rs`（`*_test.rs` 除）内 `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir` 三符号、`"api"` 路径段引号字面量、`architecture/api` 与 `<archi_dir>/api/` 两串（含 `api/extract/mod.rs` 与 `model.rs` 的 doc 散文）全部零命中
- **AND** 机制边界 MUST 说准：孤儿扫描严格覆盖 `<archi_models_dir>/**` 的 `*.schema.json` 与 `<archi_graph_dir>/INDEX.toml` **两处派生**（见「生成物入仓与零 diff 门挂载」），故 `api/` 下的残留文件**不在 `api:check` 的枚举面内**，「旧位置清空」MUST NOT 被表述为「`api:check` 报应删未删 / 孤儿索引」——那在「索引与契约分家」之后不成立；清空由迁移动作（`git mv` 与代码改动同一提交内完成）、目录树读数与 `pilot_test` 的「`<archi_dir>/api` 不存在」负向断言保证。若日后确需机械守旧段缺席，应单开变更把枚举根放宽到 `<archi_dir>` 并同步改本条

#### Scenario: 换段后的字节差异恰为两行

- **WHEN** 迁移前后各执行一次 `api:export`，对 `INDEX.toml` 与全部 `.schema.json` 做逐字节比对
- **THEN** `INDEX.toml` 的差异恰为两行——说明段的「契约面：目录」串（`<archi_models_rel_dir>`）与已声明元素的 `schema` 列（repo 相对路径含 `models/<分层目录…>/`）；其余元素条目与全部 `.schema.json` / `.i.rs` 逐字不同为零；MUST NOT 把该差异记成「零 diff」（说明段与路径列按设计变化，见「schema.json 与 INDEX.toml 为纯派生且不可手改」）

#### Scenario: check 链四段且 fmt 段为校验模式

- **WHEN** 审查 `package.json` 的 `check` 脚本与 `server:check` / `bindings:export` / `api:check` / `client:check` 四段
- **THEN** `check` 逐字为 `pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check`（段序与段数零变化）；`bindings:export` 与 `client:check` 两段命令原文逐字不变；`server:check` 的 fmt 段为 `cargo fmt --all --check` 且其 clippy 段逐字不变；`api:check` 的调用形态（`cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- --check`）逐字不变

#### Scenario: 应用内检查与脚本面结论一致

- **WHEN** 在同一观察根分别调用 `archi_api_check`（IPC）与 `api:check`（CLI）
- **THEN** 两者报出的三类差异文件集合逐条相等；随后用 `archi_api_generate`（IPC）生成，再跑一次 `api:check` 得零差异——四条消费路径（两 IPC / 两 CLI 子命令）零第二份渲染、比对或写盘实现

### Requirement: 仓库根文档的契约纪律节

仓库根 `AGENT.md` SHALL 承载一节 `## 模块 API 契约` 作为 agent 侧的操作纪律入口，内容 SHALL 限于**指针与最简规则**，MUST NOT 复制契约正文、判据长表或 fenced json 示例：

- 该节 SHALL 恰含五条要点：① 手写真相位置 `<archi_models_dir>/<分层目录…>/<FQN>.md` 与三类 fence 的名字（`api-op` / `api-types` / `api-reexport`），及一句「散文段 MUST NOT 以表格复述契约名字（名字清单的唯一持有者是 fence）」；② 四类生成物勿手改（`.schema.json`（与 md 同目录）/ `INDEX.toml`（`<archi_graph_dir>`）/ **实现入口同级**的 `<入口主干>.i.rs` / `<入口主干>.i.ts`）与「已提交手改即零 diff 门红、未提交即被静默覆盖」；③ `api:export` / `api:check` 两脚本与 `check` 链的段序；④ `api = none` 是显式留白而非「还没写」；⑤ **入口文档承诺已成立**元素的判据进 `status`（`error` 档还可进 `passed`），索引与计数经 `<archi_graph_dir>/INDEX.toml` 查看，两轴经架构页「契约」视图一眼看全。
- 该节需换段的目录串 SHALL 恰为三条要点：第①条（`<archi_models_dir>/<分层目录…>/`）、第②条（`INDEX.toml` 方位 `<archi_graph_dir>`）、第⑤条（索引串 `<archi_graph_dir>/INDEX.toml`）；第③ / ④ 条零改动。**换段完成后该节 `grep 'architecture/api'` 与 `grep '契约目录根'` 均零命中**——第②条不含 `architecture/api` 字面量，只测字面量归零不足以证明换段完成。
- 第①条的目录串 SHALL 与「契约资产形态与唯一手写真相」的落位口径逐字一致（`<archi_models_dir>/<分层目录…>/`，零 `api` 段残留）；第②条的 `INDEX.toml` 方位 SHALL 写「`<archi_graph_dir>`」而非「契约目录根」——索引与契约已分家，写成契约目录根会指错路径。
- **第①条的名字清单纪律句**：该句 SHALL 只表达「散文不复述名字清单、唯一持有者是 fence」一条事实，MUST NOT 展开成第四条 gate 的判据条件、错误文案样例或豁免规则（定义归本条与 `desktop-archi-api-check`）；它是 agent 侧唯一可见的该纪律出处（架构会话知识文本体量受 3KB 骨架上限约束，不向其追加纪律句），故该句 MUST NOT 被省略。
- 第②条的**方位词与文件名形态 SHALL 与「生成物落点与入口查找」的落点口径一致**：「实现入口同级」+ `<入口主干>.i.<后缀>`（`crates/infra/archi/src/lib.i.rs` → `lib.i.rs`）。该条只说方位与命名，MUST NOT 复述入口解析链、四条判据或 `entry` 字段知识（字段面归 `desktop-archi-model`，agent 侧字段清单归 `desktop-archi-agent` 的知识文本）——文档与 spec 双写必然漂移。本条同步 MUST NOT 增删要点条目（条目数恒 5，`entry` 不得成为第六条）。
- 该节 MUST NOT 含契约示例正文、`ArchiApiFindingKind` 的十三值长表或判据定义（定义归本能力与 `desktop-archi-api-check`）——文档与 spec 双写必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制即既有反例）。
- 该节 MUST NOT 出现 `openspec/specs/**` 路径（spec 语料非 agent 的契约操作面），且 MUST NOT 把契约路径写进模型元素 `doc`（`api = declared` 元素零 `doc` 声明，见 `desktop-archi-api-check`「契约判据词汇封闭十三值与开放分类」）。
- `CLAUDE.md` SHALL 同步同一节且与 `AGENT.md` 保持既有同源纪律：两文件该节正文逐字相同，全文件差异 SHALL 限于节外既有差异（标题行、首行说明行、文件末尾换行）；**该节自身的两份副本若分化即违反本条**。

#### Scenario: 纪律节只指路不带正文

- **WHEN** 审查 `AGENT.md` 的 `## 模块 API 契约` 一节
- **THEN** 该节恰有 5 条要点、`grep -c '^## 模块 API 契约' = 1`；`grep 'apiEntryMissing'` 在该节零命中（判据长表未复制）；节内零 `openspec/specs/` 路径；节内无 fenced json 代码块

#### Scenario: 两份副本不分化

- **WHEN** 对 `AGENT.md` 与 `CLAUDE.md` 做 `diff`
- **THEN** 差异恰 2 处且全在 `## 模块 API 契约` 节外（`:1` 标题、`:3` 首行说明）；该节在两文件各恰一处且节内正文零 diff 行（逐字相同）

#### Scenario: 第 ① 条含名字清单纪律一句且要点数不变

- **WHEN** 审查 `AGENT.md` / `CLAUDE.md` 该节第 ① 条
- **THEN** 该条含「散文（表格）不复述契约名字 / 名字的唯一持有者是 fence」语义的一句；该节要点数仍恰 5（`grep -c '^## 模块 API 契约' = 1` 且条目数不变）；该句不含判据条件、错误文案样例或豁免清单（`grep 'ApiError::Contract'` 在节内零命中）

#### Scenario: 纪律节第 ⑤ 条与纳管口径一致

- **WHEN** 对照该节第 ⑤ 条与 `desktop-archi-api-check`「报告并列槽位 apiFindings」的纳管条款
- **THEN** 两处都表达「入口文档承诺已成立元素的判据影响 `status`，其中 `error` 档还影响 `passed`」，MUST NOT 残留「不进 `status`」或「`apiFindings` 不影响 `passed`」一类旧口径

#### Scenario: 第 ② 条方位词与真实落点一致

- **WHEN** 审查 `AGENT.md` 与 `CLAUDE.md` 该节第 ② 条，并对照入口同级的实盘生成物 `crates/infra/archi/src/lib.i.rs` 与索引实盘 `<archi_dir>/graph/INDEX.toml`
- **THEN** 该条写「实现入口同级」且文件名形态为 `<入口主干>.i.rs` / `<入口主干>.i.ts`，索引方位写 `graph` 目录；`grep '元素根同级'`、`grep '<root>.i'` 与 `grep 'architecture/api'` 在两文件该节零命中；第 ① / ② / ⑤ 三条各含新落点串（`models/<分层目录…>/<FQN>.md` / `graph` / `graph/INDEX.toml`）而第 ③ / ④ 条逐字不变，`grep '契约目录根'` 在节内零命中（该串无 `architecture/api` 前缀，字面量归零抓不到它）；该节要点数仍恰 5（措辞同步 MUST NOT 增删条目、MUST NOT 出现 `entry` 字段说明）；两文件全文 `diff` 仍恰 2 处且全在节外

### Requirement: 语言无关纪律与语言特定落点

契约资产层 SHALL 与语言无关，语言特定能力 SHALL 只落取数器与渲染器：

```
语言无关    models · api 契约 · docs · graph · 漂移与契约判据 · FQN · gate
────────────────────────────────────────────────────────────────
语言特定    api/extract/{rust,ts}.rs（取数器，仅 bootstrap）· api/emit/{rust,ts}.rs（生成物投影）
```

- `crate` 是 Rust 工具链概念，架构层 MUST NOT 依赖：`dto::ElementKind` 无语言字段的原则已被遵守，契约格式 MUST NOT 重复 models 注释里的历史错误（`02-backend-core.toml:2` 写「Cargo `[dependencies]`」、`06-frontend.toml:3` 写「import 证据」）。
- `<FQN>.md` / `<FQN>.schema.json` / `INDEX.toml` 内 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` / `TypeScript` 一类语言专有词（`x-common.note` 里说明具体语言的实现细节除外，且须显式标注为注记）。
- **生成物按模块落盘，MUST NOT 按 crate 聚合**：按 crate 聚合会让架构层依赖语言概念，且 diff 粒度串味。
- **跨语言语义标注**：语言能力缺失 SHALL 经 `x-common.degraded` 显式留痕，取值 MUST 来自**封闭四值词表**（`foreignElement` / `noElement` / `unrepresentable` / `crossLanguage`，见「降级词表与真实债表」）；旧口径的两类（错误通道走 `Promise` reject、TS 结构化类型无法强制实现）分别是该词表里的 `unrepresentable` 与 `crossLanguage` 两值，MUST NOT 作为并列的第二套分类并存。降级是机制属性，MUST NOT 靠散文掩盖。

#### Scenario: 资产面零语言词

- **WHEN** 审查三份手写 / 生成资产（`.md` / `.schema.json` / `INDEX.toml`）的全部文本
- **THEN** 零 `crate` / `Cargo.toml` / `Rust` / `TypeScript` 词（注记内标注为注记的语言说明除外）；语言词只出现在 `api/extract/**` 与 `api/emit/**` 的源码注释里

#### Scenario: 降级词表封闭且可枚举

- **WHEN** 取 `INDEX.toml` 的 `degraded` 计数与各契约内全部 `x-common.degraded` 出现点
- **THEN** 取值只来自四值词表（逐值可枚举）；词表外值与非字符串值在装载期即契约错误（文案并列四值）；新增第五值 MUST 改本 requirement 与「降级词表与真实债表」，MUST NOT 自由发挥

### Requirement: 生成器、verify 闭环与 bootstrap 剪枝

`verify` SHALL 定义为机械判定而非独立模式：**`generate`（`--check` 零写入，比对在工具内完成，等价于 git 零 diff）+ 语言编译器**。MUST NOT 引入归一函数、双层锁或语义 diff 比对器——契约与代码是否一致由「生成物是否最新（零 diff）」与「实现是否匹配生成物（编译器）」两步回答。

- **判定在 core，写盘在工具 crate**：契约解析、装配、判据求值与渲染 SHALL 全部落 `crates/core/archi/src/api/**`（纯读、零写盘、零 spawn，沿该 crate 既有纪律）；唯一写盘面 SHALL 为 `crates/tools/archi-api-gen`（CLI 工具，与既有 `export-bindings` 同族），允许写盘是该工具边界的正当属性、MUST NOT 反向稀释 `archi` 的零写盘纪律。
- **CLI 三子命令封闭**：`generate`（写四类生成物：`<FQN>.schema.json` / `INDEX.toml` / `<root>.i.rs` / `<root>.i.ts`）、`--check`（生成到内存后与磁盘比对，**零写入**，差异即非零退出并列文件名）、`bootstrap`（输出候选草稿到 stdout，不写盘）。`generate` SHALL 幂等（同输入同输出，重跑零 diff）。
- **`verify` 覆盖不到的三件事 SHALL 显式列出**：bootstrap 方向（代码 → 契约，属一次性迁移工具，非常态门禁）、跨模块语义不变量（散文 + 专项测试）、生成器自身正确性（生成器自己的单测）。MUST NOT 让人误以为零 diff 门能验这些。
- **bootstrap SHALL 走 AST，MUST NOT 正则抽签名**：Rust 侧实测多行签名占 `pub fn` 的 30–62%（`core/checks` 13/21、`core/archi` 16/27），`grep` 单行抽取会截半签名。TS 侧只需判定 `export function`——实测 `export const` 里零个箭头函数（0/132），故 MUST NOT 为「从初始化表达式推断函数」造启发式。
- **`visibility` SHALL 为封闭四值**（bootstrap 剪枝的机械判据）：`external`（跨元素可达，真承诺保留）/ `crateOnly`（仅同 crate 或同元素目录内可达，内部契约保留但降格）/ `unused`（全仓无引用者，**伪公共首选删除对象**）/ `testOnly`（仅测试树内引用，不进 API）。**词表封闭 ≠ 两语言都判得出四值，更 ≠ 本期生产面都产得出**：本期可控值域 SHALL 按语言分列——Rust 侧四值可判（`pub mod` 链或 `pub use` 白名单 / `pub(crate)` / 私有 `mod` 内的 `pub` / `#[cfg(test)]` 树）；TS 侧**本期生产面只产 `external`**（有 `export`），`unused` 的降级接口 `apply_knip_report` 已实现但**产品路径零调用者**、`archi-api-gen` 与 `api::bootstrap` 均不读 `knip` ⇒ `Visibility::Unused` 在**本期生产面不可达**，这是**显式留白**而非疏漏（MUST NOT 用「已实现」掩盖；接线是独立变更）；`crateOnly` / `testOnly` **本期留白**（判据需跨文件 import 反向事实，取数器只吃单文件 AST）。留白项 MUST NOT 以「文件是否在元素目录内」或「文件名含 `.test.`」充当 proxy（`.spec.*` / `__tests__/**` / `fixtures/**` 均不作 proxy）。
- **MUST NOT 为 TS 造第二套死代码扫描**：本期 SHALL NOT 读 `knip` 报告（`apply_knip_report` 产品路径零调用者，`archi-api-gen` 与 `api::bootstrap` 对 `knip` 零命中）⇒ `unused` 降级在本期生产面不可达，属显式留白；接线时 `unused` 判据 SHALL 取自 `knip` 报告（`client:check` 已含），且 `knip.json` 的 `ignore` 面与 archi 模型排除面高度一致，直接沿用其判定，MUST NOT 另造一套死代码扫描。
- **试点先行**：bootstrap 的首次落地 SHALL 只做**一个**元素（`Backend.Infra.ArchiRuntime`，14 个 crate 元素里公开面最小的 2 项），由人工或 agent 删除伪公共面形成初版契约；其余元素的契约 SHALL 按后续变更逐个追加（MUST NOT 一次性铺开）。
- **bootstrap 的近似误差 SHALL 如实留痕**：glob 导入（`use a::*`）、同名符号、宏生成引用都会让「有引用者」判定偏宽——这些是 bootstrap 的已知近似，MUST NOT 被伪装成精确结果。

#### Scenario: verify 三件套闭环

- **WHEN** 改动代码签名但不重生成生成物
- **THEN** `api:check` 的 `--check` 路径报差异并以非零退出（点名文件）；重生成后零 diff；若改动与契约不符则编译器报错（Rust 签名不匹配 / TS 形状不符）

#### Scenario: `--check` 零写盘

- **WHEN** 以 `--check` 在被修改过的仓库上运行（有差异）
- **THEN** 磁盘字节零变化（含生成物、`INDEX.toml` 与 mtime 语义上的「不重写」），退出码非零、输出列出差异文件

#### Scenario: 多行签名抽取不截半

- **WHEN** 对含多行 `pub fn`（参数逐行排列）的源文件跑 bootstrap 抽取
- **THEN** 归一签名含全部参数与返回类型；以 `grep` 单行抽取的对照输出被截半（该反例在案）

#### Scenario: visibility 四值判定

- **WHEN** 对同一 crate 的 `pub fn`（`pub mod` 链可达）/ `pub(crate) fn` / 私有 `mod` 内的 `pub fn` / `#[cfg(test)]` 树内被引用的 `pub fn` 跑 bootstrap
- **THEN** 依次判为 `external` / `crateOnly` / `crateOnly` / `testOnly`；TS 侧本期只产 `external` 一档（有 `export`），`Unused` 的降级接口 `apply_knip_report` 本期不接线（生产面不可达，显式留白），`crateOnly` / `testOnly` 同属本期留白（与 requirement 正文同口径，MUST NOT 在 Scenario 里给出更宽的承诺）

#### Scenario: bootstrap 只出 stdout 不落盘

- **WHEN** 执行 `bootstrap` 子命令
- **THEN** 候选草稿只写 stdout（含 `symbol` / `kind` / 定义位置 / 归一签名 / `visibility` / 引用方（`used_by`，本期留白）），工作区磁盘零变化；`git status` 干净

### Requirement: 模块嵌套与选择性透传

父子元素的契约关系 SHALL 遵循「父 → 子」单向：父元素契约 SHALL 逐项列出它透传的子元素导出，MUST NOT 由「子元素面的并集」自动推导。

透传 SHALL 同时表达于三处且互相一致：

| 载体 | 内容 | 用途 |
|---|---|---|
| `<FQN>.md` 的 `api-reexport` fence | 透传清单（键 = 导出名，值 = 直接子元素 FQN） | 人读 + 生成器输入 |
| `<FQN>.md` 散文区 | 透传意图 | 人读 |
| `<FQN>.schema.json` 的 `$defs` | 本地定义 **+ 透传项（纯 `$ref`）** | 机器检查（悬空引用编译期即报） |

六条结构规则：

1. `api-reexport` fence 键集 SHALL ≡ `schema.json` 的透传 `$defs` 键集（漂移即 `apiReexportDrift`）。
2. 父 `$defs` 的透传条目 MUST NOT 携带结构关键字（`type` / `properties` / `enum` / `const` / `items` / `oneOf`）——它是纯 `$ref` alias，携带即沦为重复定义。
3. 父 `$defs` 与任一子元素 `$defs` 的**类型名交集必须为空**（`apiShadowing`）。MUST NOT 依赖编译器兜住：实测 `pub use drift::*` + 同名 `struct` 只给 `ambiguous glob re-exports` 警告，不报错。
4. 透传目标与跨模块 `$ref` SHALL 是**直接子元素**（FQN 前缀相邻）且其契约存在且 `api = declared`；跨层引用（父直连孙辈）SHALL 报 `apiRefMissing`——层级语义被架空的信号。
5. 嵌套元素的父子 MUST 同属**一个 crate**（Rust 侧跨 crate 父子即 `cyclic package dependency`，不可表达）→ `apiNestedCrossCrate`；TS 侧无此约束。
6. 子元素 MUST NOT 依赖父元素（含类型）；共享类型 SHALL 下沉到兄弟元素或消解依赖。该规则在代码层由既有 `UndeclaredDependency` 判据报（父子关系不进 relation 图，故必然报），本能力 MUST NOT 新增第二条判据。

两条推论：

- **递归透传 MUST NOT 展开**：若 A ⊃ B ⊃ C，A 只声明透传 B；B 内部已透传 C 则自动带出。
- **透传是选择性的**（实测非假设）：`Desktop.Components.Agent` 有 **7** 个子元素，其 barrel（`components/agent/index.tsx`，即该元素的文件型 `path`）经 `export … from` 只重导出其中 **4** 个子元素的导出——`AgentInput`（`Agent.AgentInput`）、`AskAnswerActions`（`Agent.AgentMessages`）、`AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps`（`Agent.AgentSessionView`）、`SessionPanel` / `SessionPanelState`（`Agent.AgentSessionPanel`，该文件已是元素）；`AgentBlocks` / `AgentError` / `ToolCards` 三个子元素不出现。故「子元素存在」≠「父元素透传它」，父契约仍是「透传自子元素 / 自有 / 子元素但不导出」三类混合（本元素现态无自有导出），MUST NOT 自动推导——代码侧可达面由 `desktop-archi-api`「TS 导出闭包的取数面」给出，它**不构成**父契约可以自动生成的许可。
- **环依赖从此不可能**：跨元素 `$ref` 只剩「父→子」与「兄弟→兄弟」两类边，必然可拓扑排序。禁断子→父的最大价值不是禁断本身，而是让「把子目录提升为元素」变成一次真实的解耦决策（`infra/agent` 要把 `sdk/` 提升为元素，先得把 3 个类型从 `lib.rs` 挪出来）。
- **MUST NOT 新增契约层依赖判据**：契约不是依赖事实源，事实链是「契约 → 生成物 → 代码 import → `scan` 取数 → `OwnerIndex` 归属 → `drift::check` 对账既有 `UndeclaredDependency`」。判据建在事实源（代码）上，不在投影（契约）上；新增判据只在「契约自身内部矛盾」时成立。
- **契约记录「提供什么」，不记录「实现了什么」**：MUST NOT 在 `$defs` 条目上记 `implements` 注记（重复表达 model relation、污染契约语义、判据已被编译器与 `UndeclaredDependency` 双重覆盖）。推论：三个 runner 的契约面就是 `new()`——该薄面是代码形状的事实，处置是改代码（暴露已存在的私有纯函数）而非放宽格式。

#### Scenario: 父契约逐项声明而不自动推导

- **WHEN** 为 `Desktop.Components.Agent` 写契约：`api-reexport` 只声明 barrel 实际重导出的 7 个名字中的 2 个（`AgentInput` / `AgentSessionView`），其余重导出项（`AskAnswerActions` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`）后续逐条追加，`AgentBlocks` / `AgentError` / `ToolCards` 三个子元素不出现
- **THEN** `api-reexport` fence 恰 2 条、`$defs` 恰 2 条纯 `$ref` 条目、本次零本地定义（该元素现态无自有导出）；三个未导出子元素零出现；生成的 `.i.ts` 为 `export { AgentInput } from …` / `export { AgentSessionView } from …`；代码侧闭包可达 7 名**不构成**「契约应自动含 7 条」的判据——父契约逐项手写（MUST NOT 自动推导）

#### Scenario: 透传条目带结构关键字即错

- **WHEN** 父 `$defs` 的透传条目同时写 `$ref` 与 `type: object` / `properties`
- **THEN** 契约错误（透传条目必须是纯 `$ref` alias）；`apiShadowing` 亦报（与子元素类型名交集非空）

#### Scenario: glob 形态重复定义由契约层拦住

- **WHEN** 父元素代码用 `pub use child::*` 且手写同名 `struct`（编译器只给 warning）
- **THEN** `apiShadowing` 报出父子 `$defs` 类型名交集，检查失败；MUST NOT 以「编译器没报错」为由放行

#### Scenario: 跨层与跨 crate 引用各报其判据

- **WHEN** 分别构造「父契约 `$ref` 孙辈元素」与「父子元素分属两个 crate」
- **THEN** 前者报 `apiRefMissing`（非直接子元素），后者报 `apiNestedCrossCrate`；两者都点名涉事 FQN

#### Scenario: 子→父依赖由既有判据报

- **WHEN** 子元素代码 `use` 父元素直接定义的类型（父 `lib.rs` 里的 `EngineFacade` 一类）
- **THEN** 既有 `UndeclaredDependency` 报出该依赖（契约层不加新判据、不出现第二份判定实现）

### Requirement: 属性键与 required 的自洽闸

装载期（`api::contract::verify`）SHALL 在既有一趟遍历内续判一条**契约自洽**规则：`$defs.<name>.required` 的每个元素 MUST 是同条目 `$defs.<name>.properties` 的键；悬垂即 `ApiError::Contract`，文案 SHALL 带 JSON pointer `$defs.<name>.required[<i>]` 并列该元素。

- **不新增族别数**：属性键（首层 `properties` 键）已由闸一 ⑤ 族判 ASCII 标识符；闭合性成立即 `required` 元素的标识符性**自动传递**，故本规则 MUST NOT 引入第二个族别词、MUST NOT 拆分第二个公开入口、MUST NOT 落 `schema::verify_defs` 或 `emit`（`emit` 侧零 `is_identifier` 调用是既有 D45 纪律的机械判据）。
- **理由**：`required: ["nmae"]`（笔误）今天会静默把 `name` 渲染为**可选**——又一条「判据绿而产物错」。`required` 与 `properties` 两键集不闭合，产物就与作者意图分离，且两侧产物（`Option<T>` / `T?`）会一致地错，编译器兜不住。
- **出路文案 SHALL 族别化**：属性键族与 `required` 族的错误 MUST 带 pointer（`$defs.<name>.properties.<key>` / `$defs.<name>.required[<i>]`）并给出一条可执行出路——「属性键必须是标识符；`data-testid` 一类键不进契约（前端契约须避开 DOM 测试属性）；人类可读的措辞写散文区」。通用句（「MUST NOT 含空格 / 连字符 / 点」）只说「非法」不说「怎么办」，不足以让作者一次改对。
- **两条明确否决**（MUST NOT 实现，MUST NOT 由后续变更顺手加回而不改本 requirement）：

  | 否决项 | 理由 |
  |---|---|
  | TS 侧加引号形态（`'data-testid': string`） | 会让同一 schema 两侧产物形态不同（Rust 无「带引号字段名」形态，`r#` 只接受标识符）——正是 D47 纪律要消灭的不对称 |
  | 契约加 `x-key` 重命名映射 | 引入第二套命名体系（契约名 ≠ 源码名），判据面的名字闸与渲染期的插值点会各读一套名字，族别词随之失真 |

#### Scenario: `data-testid` 属性键在装载期被拦且文案给出路

- **WHEN** 契约的 `$defs.Widget.properties` 出现键 `data-testid`
- **THEN** 装载期返回 `ApiError::Contract`，文案含 pointer `$defs.Widget.properties.data-testid` 与「`data-testid` 一类键不进契约」的出路句；四类生成物零产出（`emit` 侧不产 `data-testid: string;` 一类不可编译文本）

#### Scenario: required 悬垂即契约错误

- **WHEN** 契约写 `properties: { "name": { "type": "string" } }` 而 `required: ["nmae"]`
- **THEN** 装载期报错并点名 `$defs.<name>.required[0]` 与 `nmae`；改为 `required: ["name"]` 后通过，且 `name` 两侧都渲染为必填（`pub name: String` / `name: string`）

#### Scenario: 自洽契约零误伤

- **WHEN** 对现网唯一契约（`Backend.Infra.ArchiRuntime`，`properties` 为空对象且零 `required`）与任意 `required ⊆ properties` 的契约执行装载
- **THEN** 两者均通过；装载产物与改动前逐字相同

### Requirement: 降级词表与真实债表

`x-common.degraded` 的取值 SHALL 封闭为**四值词表**，且该标记 SHALL 成为 `INDEX.toml` 的真实债表来源：

| 值 | 语义 |
|---|---|
| `foreignElement` | 跨域 / 非直接子元素引用（目标契约不在此元素的直接子元素面内） |
| `noElement` | 目标无模型元素（如 hooks / 纯工具库 / 裸类型模块） |
| `unrepresentable` | 目标语言类型系统表达不了（如 JSX 形态、错误通道走 `Promise` reject） |
| `crossLanguage` | 规则表降级（`common.json` 的换算在目标语言侧的表达损失） |

- **词表封闭**：新增第五值 MUST 改本 requirement（与既有「判据词表封闭」同一条纪律）；`x-common.degraded` 的值 MUST 是上表四值之一的**字符串**——非字符串（如 `true`）与词表外值一律 `ApiError::Contract`，文案并列四值并点名 JSON pointer。`true` 不携带原因信息，债表不可读，故不收。
- **词表落点**：词表 SHALL 为 `api::schema` 内的单个常量（与校验判据同处），MUST NOT 进 `common.json`——该资产的语义封闭为**映射规则**（见「关键字集、类型映射与错误面」的 `common.json` 边界条款），且其摘要进全部生成物头注释，加词表会无谓改动所有生成物头。
- **与 IPC 面的降级哨兵不是一套东西**：本词表只管**契约资产**内的 `x-common.degraded`；`bindings.zod.ts` 里的 `degraded("原因")`（见 `desktop-ipc-runtime-guard` / `desktop-ipc-type-bindings`）是**生成物 schema 面**的另一机制，两者的取值词表 MUST NOT 合并、MUST NOT 互相校验。
- **必须写的纪律**：凡「引用不可达 / 不可表达」的类型位置 MUST 写 `x-common.degraded = <词>`；`x-common.note` 只作**自由注记**（人读），MUST NOT 承担判据、MUST NOT 计入债表。bootstrap 草稿的 `待剪枝：…` note 因此零计入——「草稿未剪枝」与「结构性不可表达」是两件事，混为一谈会让债表被草稿噪音淹没。
- **计数口径 = 标记出现次数**：`INDEX.toml` 每元素的 `degraded` SHALL 等于该元素 `schema.json` 内 `x-common.degraded` 标记的**出现次数**，扫描面为**全深度**（`$defs` 条目自身、条目第一层 `properties` 的值、`properties` 里的 `x-function.params[].schema`，以及更深层的对象 / 数组）。口径由「`$defs` 条目数」改为「标记数」：真实降级落点在属性位（演练草案的 `session` / `sessions.items` / `send.input` 三处都在 `properties` 值上），条目级计数看不见它们。
- **单点扫描**：计数与取值校验 SHALL 共用同一个全深度扫描函数（`degraded_marks(schema) -> Vec<(pointer, value)>`），MUST NOT 存在第二套遍历——`verify_defs`（校验）与 `index_entries`（计数）各调一次，一处改口径两处随动。
- **显式留白**：把「必须写 `degraded`」**机械强制**（判「不可达却未写标记」）不在本期——它需要跨元素可达性知识（`$ref` 目标是否直接子元素 / 契约是否在案），属 `check` 面（见 `desktop-archi-api-check`）；本 requirement 只给词表、取值校验与计数。留白 MUST NOT 被伪装成已实现。
- **零影响**：现网唯一契约（`Backend.Infra.ArchiRuntime`）零 `degraded` 标记 ⇒ 现网计数仍为 `0`、`INDEX.toml` 逐字不变。

#### Scenario: 词表外值与非字符串值在装载期被拒

- **WHEN** 分别构造 `x-common.degraded = "typo"` 与 `x-common.degraded = true`
- **THEN** 两次均 `ApiError::Contract`，文案点名该标记的 JSON pointer 并列出四值；改为 `"noElement"` 后装载通过

#### Scenario: 债表按标记出现次数计

- **WHEN** 对演练草案形态的契约（`AgentSessionActions.send` 的参数、`AgentSessionViewProps.session`、`SessionPanelState.sessions.items` 三处标 `x-common.degraded`，其外层 `$defs` 条目自身无标记）执行 `render_all`
- **THEN** `INDEX.toml` 该元素的 `degraded = 3`；`note` 形态（`待剪枝：…`）出现任意次都零计入；同一份 `schema.json` 里三处标记也同时通过了词表校验（校验与计数同源）

#### Scenario: bootstrap 草稿零计入

- **WHEN** 对一个含 `x-common.note = "待剪枝：…"` 草稿条目的契约执行 `render_all`
- **THEN** `degraded` 计数为 `0`（草稿的 `note` 不升级为债）；该契约仍能装载（`note` 是自由注记，不加校验）

### Requirement: TS 导出闭包的取数面

`api::extract::extract_element` SHALL 对 TypeScript / JavaScript 元素产出**入口导出闭包**：从实现入口出发，沿 `export … from` 一跳或多跳并入目标文件的导出。闭包 SHALL 是该元素「代码导出集」的唯一来源（判据的导出集一侧与 `bootstrap` 草稿同源），MUST NOT 存在第二份 TS 导出集口径。

- **入口唯一全量起点**：只有从入口可达的导出进公共面——入口自身声明的导出，加上入口 re-export 可达链上的导出。MUST NOT 把元素目录下**未被入口聚合**的文件导出并入：那是「入口 = 公共面」之外的第二套定义。入口读不到 ⇒ 空集（既有语义逐字不变，缺口由 `apiEntryMissing` 报）。
- **具名名单精确**：`export { A } from './x'` / `export type { A } from './x'` SHALL 只把 `A` 入集，MUST NOT 把 `./x` 的全部导出并入（虚增面）。`export { A as B } from './x'` SHALL 在 `./x` 取 `A`、以 **`B`** 入集（改名如实反映，MUST NOT 以 `A` 入集）。
- **通配取全量面**：`export * from './x'` SHALL 取 `./x` 的导出面（`./x` 自身再含 re-export 时递归同理）；`export *` 与逐名 re-export 在闭包内 SHALL 等价——同一簇文件的两种写法产出同名同 kind 的集合。`components/ui/index.ts` 的 **39 条 `export *`** 即本仓该形态的唯一批量实例（`Components.Ui` 的导出集 = 39 个模块的导出面）。
- **环防键 =（目标文件, 名字集）**：visited SHALL 以「目标文件 + 本次请求的名字集」为键，键命中即返回空集。MUST NOT 只用文件作键——同一目标文件的两条具名语句（本仓 `components/agent/index.tsx` 的 `export { AgentSessionView } from './agent-session-view'` 与 `export type { AgentSessionActions, AgentSessionViewProps } from './agent-session-view'`）会被互相吞掉，静默丢名字且无任何报警。
- **终止性与确定性**：`a → b → a` 环与菱形（同一类型经两条路径到达）SHALL 由 visited 收敛，MUST NOT panic、MUST NOT 重复、MUST NOT 无限递归；终止性 MUST 由 visited 保证，MUST NOT 依赖深度上限（若实现另设上限作第二道保险，触顶 MUST NOT panic、MUST NOT 把部分面冒充完整面）。同一输入两次调用 SHALL 产出逐字相等的结果（稳定序 = 入口语句序 + 目标文件声明序，MUST NOT 依赖哈希序）。
- **去重键 =（name, kind）**：同一符号经多条路径到达 SHALL 只保留首次出现；去重 MUST NOT 按结构（两个同名字不同类型不得合并，与「禁按结构去重」同一条纪律）。
- **同元素约束 = owner 子树**：re-export 目标文件 SHALL 经 `scan::OwnerIndex::owner_of` 归属，仅当结果为 `Owner::Element { fqn }` 且 `fqn == element.fqn` 或以 `"{element.fqn}."` 为前缀时并入；`Owner::Ambiguous(_)` 与 `Owner::None` SHALL 一律跳过（MUST NOT 任选、MUST NOT 放行）。FQN SHALL 按 `.` 分段比较，MUST NOT 裸 `starts_with(element.fqn)`（否则 `…Components.Agent` 会误认 `…Components.AgentFoo` 为子树）。**理由**：父元素的公共面 ⊇ 可达子元素面正是 barrel 的语义，与目录型 `path` 前缀在结论上一致（不产生第二条规则分叉），MUST NOT 被读成虚增——虚增由具名名单纪律守、完备性由 `apiUndeclared` 守。故**目录型 `path` 的 barrel 元素**（本仓 12 个目录模块入口，例如 `Views.Changes` 的 `views/changes/index.ts` 聚合两个直属文件）不需要任何例外分支：入口在 `path` 之内、目标在本元素子树内，`within_path` 与 owner 子树同时成立。
- **specifier 解析单点复用**：目标文件解析 SHALL 复用 `scan::ts::TsResolver::resolve`（就近 tsconfig 的 `baseUrl` / `paths` / 别名 / 扩展名 / index 折叠），MUST NOT 造第二份 specifier 解析。解析失败（目标在仓外 / 不在盘 / 不可解析）SHALL 静默跳过该条 re-export：MUST NOT panic、MUST NOT 记 `unresolved`（`unresolved` 只属依赖边抽取的账）。`api::extract → scan::{ts, owner}` 的内部依赖为本能力新增，与 `api/diff.rs` / `api/paths.rs` 已有的 `scan` 依赖同族——理由是「specifier 解析与文件归属各只有一个实现」。
- **visibility 一律 `External`**：闭包内符号 SHALL 全部为 `External`（可达即公开），与 Rust 侧 `pub use` 升格的语义对齐；本能力 MUST NOT 新增 `crateOnly` / `testOnly` / `unused` 的产出路径（三值留白逐字不变）。
- **签名带元素与元素表**：`extract_element(root: &Path, elements: &[ArchiElement], element: &ArchiElement, entry: &str) -> Vec<ApiSymbol>`——同元素约束的唯一依据；与 `paths::entry_state(root, elements, element)` 同形。Rust 分支语义 SHALL 逐字不变（`mod` 树自锚，元素参数不参与，MUST NOT 因此改 `rust::extract_crate` 的抽取面）。
- **导出形态单一分派点**：`api/extract/ts.rs` 内 `match statement` SHALL 恰 1 处，一次分派产出三态（`Declaration` / `Named{local, exported, specifier}` / `Star(specifier)`），其余形态归 `None`。`ts::extract_file(file, source)` 的签名与产出语义 SHALL 逐字不变（只认 `export <声明>`，re-export 零符号）；新增的声明/名单抽取面只读源文本 + AST（零读盘、零写盘、零 spawn）。闭包（递归 + 读盘 + 解析 + 环防）SHALL 落在 `extract_element` 那一层，MUST NOT 下沉进 `ts.rs`。
- **定义位置带文件**：`ApiSymbol` SHALL 新增 `file`（定义所在文件，repo 相对 POSIX 路径）。`bootstrap` 的注记 SHALL 为 `<!-- <visibility> @ <file>:<line> -->` 且 `file` 取该符号的定义文件——MUST NOT 用入口串冒充（闭包一跳后行号属于另一个文件，注释指错文件比没有注释更坏）。Rust 侧 `file` SHALL 为该符号所在模块文件（内联 `mod` 与父同文件）。

**显式留白（MUST NOT 被伪装成已实现）**：

| 形态 | 处置 |
|---|---|
| `export { localA }`（无 `from`，导出本文件的非导出声明） | 不产出：需要「按名字取回本文件声明」的第二遍，与「取数面 = 导出面」的单遍纪律冲突；`api/extract/ts.rs` 内 `Statement::ExportNamedDeclaration` MUST 零命中（由源码断言钉住） |
| `export * as ns from './x'` | 跳过：产出命名空间对象而非符号，不猜 |
| 跨元素 / 仓外 re-export | 静默跳过，**零 finding**：取数器非权威，缺口由契约面判据显示；MUST NOT 为它新增判据（避免与 `apiRefMissing` 语义重复）。`desktop-frontend-module-entry` 的同名纪律（入口只导本元素直属文件）是**代码侧**要求，两者不是同一件事：取数器放行子元素目标，代码纪律不许写出这种目标 |
| `export default` | 不抽（既有纪律：对外名字由导入方决定） |

**预期边界（写进能力，不许口头约定）**：前端元素的真实导出面以**类型面与名面**为主——组件函数的入参是解构 props + 内联类型字面量，抽出的参数类型恒为 `Unknown`。故本能力 MUST NOT 被读成「前端行为契约可写」：`api-op` 面在前端继续留白，`bootstrap` 对前端的价值是「名与类型面的起点」，不是可直接落纸的签名。12 个目录模块入口落地后这一边界**不变**：它们把「名面」变得可机械取数（`flow` 的 11 名、`renderers` 的 2 名一类），不使 `api-op` 面变得可写。

#### Scenario: 精确名单与通配等价

- **WHEN** 对同一簇夹具（`./x` 导出 `A` / `B` / `C`）分别构造「逐名 re-export 三条语句」与「一条 `export * from './x'`」两个入口，各跑一次 `extract_element`
- **THEN** 两次产出逐字段相等（名字集与顺序均同，同为 3 个符号）；再构造「入口只 `export { A } from './x'`」⇒ 产出恰 1 个符号，`B` / `C` 零命中（精确名单，MUST NOT 虚增）

#### Scenario: 环与菱形确定收敛

- **WHEN** 夹具含 `a → b → a` 的 re-export 环，以及菱形（`entry` 同时 `export * from './l'` 与 `export * from './r'`，两者都 `export * from './leaf'`）
- **THEN** 两次 `extract_element` 均正常返回（零 panic、零无限递归）；环内符号各出现一次、菱形里 `leaf` 的符号按 `(name, kind)` 恰一条；同一输入连续两次调用结果逐字相等

#### Scenario: 同元素约束按 owner 子树裁

- **WHEN** 夹具含四条 re-export：①目标归属本元素子元素（`Owner::Element`，FQN 以 `"{本元素}."` 为前缀）、②目标归属另一个元素、③目标无元素认领（`Owner::None`，如 `@/lib/**` 形态）、④目标同长多命中（`Owner::Ambiguous`）
- **THEN** 只有 ① 入集；②③④ 全部跳过且零 finding、零 panic；把 ① 的 FQN 改成 `"{本元素}Foo"`（裸前缀相同但不分段相等）⇒ 不入集

#### Scenario: 本仓 Components.Agent 取数面恰七名

- **WHEN** 以本仓根对 `Desktop.Components.Agent`（`path = ./packages/desktop/src/components/agent/`，目录型；入口由 ③ 命中目录内 `index.tsx`）取数
- **THEN** 导出集恰 7 个名字 `AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`（3 个 `Function` + 4 个 `Type`）；`AgentInputProps` / `AgentMessagesProps` / `isToolPart` 零命中（未被 barrel 导出 ⇒ 具名名单不入集）；同一次取数在 `bootstrap` 草稿里表现为非空骨架。**路径形态翻转前后取数结果逐字相同**——入口仍是同一份 `index.tsx`，翻转只改 `kind` 与 `path` 的用法

#### Scenario: 本仓目录模块入口的取数面

- **WHEN** 分别对 `Desktop.Views.Changes`（入口 `views/changes/index.ts`）与 `Desktop.Views.Changes.Renderers`（入口 `views/changes/renderers/index.ts`）取数
- **THEN** 前者导出集恰 `ChangeDetailView` / `ChangeListView` 两名，后者恰 `ArtifactTabs` / `MarkdownDocRenderer` 两名；两元素的目录内其余公开符号（如 `renderers/registry.ts` 的成员、`changes/hooks/**`）**零命中**（未被入口聚合 ⇒ 不在公共面）；`Components.Ui` 的导出面按 39 条 `export *` 取全量（其「目录 = 模块」语义由 `desktop-frontend-module-entry` 定义）

#### Scenario: 留白形态零产出

- **WHEN** 分别对 `export { localA };`（无 `from`）、`export * as ns from './x'`、`export default function f() {}` 三种源文取数
- **THEN** 三者均零符号；`extract_file` 的产出语义不变（透传与 default 都不产符号），且 `api/extract/ts.rs` 内 `Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中（留白由源码断言钉住，不是靠「忘了写」）

#### Scenario: 注记行指向真实定义文件

- **WHEN** 对本仓 `Desktop.Components.Agent` 起草（`bootstrap`）
- **THEN** 每个符号的注记指向其真实定义文件与行（`AgentInput` → `packages/desktop/src/components/agent/agent-input.tsx:152`、`SessionPanel` → `…/session-panel.tsx:230`、`AskAnswerActions` → `…/agent-messages.tsx:33`），MUST NOT 出现把入口 `index.tsx` 与目标文件行号拼在一起的形态；Rust 侧同一条纪律（子模块 `pub fn` 的注记指向子模块文件）

#### Scenario: 解析失败静默跳过

- **WHEN** 入口 `export { usePanelRef } from 'react-resizable-panels'`（仓外包）、`export { X } from './not-on-disk'`（不在盘）两种形态
- **THEN** 两者都被静默跳过：`extract_element` 正常返回（零 panic、零符号、零 finding），`unresolved` 留痕为零（该账只属依赖边抽取）

### Requirement: bootstrap 草稿的六段骨架与惰性作者注

`api::bootstrap` 输出的契约 md 草稿 SHALL 是**可直接落盘并零错误装载**的骨架，并在骨架内自带起草引导：

- **六段骨架**：草稿 SHALL 依次含 `## 职能` / `## 不变量` / `## 操作` / `## 类型` / `## 边界` / `## 承载理由` 六个二级段；H1 SHALL 仍为元素 FQN（与文件名主干逐字相等，`file_title` 判据不变）。既有四段的相对次序与前缀 MUST NOT 变化，`## 边界` / `## 承载理由` 追加在 `## 类型` 之后——这两段对应契约资产形态里散文区已有的两个主题（职能 / 边界 / 不变量 / 承载理由），本 delta 只让草稿把它们占位出来，MUST NOT 引入散文的第七个主题。
- **占位句形态**：六段的占位正文 SHALL 是含 CJK 的「（待补：…）」句；MUST NOT 写成行首 `- <ASCII 标识符>` 的列表项（`collect_reexport_prose` 会把它抽成透传导出名，与 `api-reexport` 键集比对，不等即 `ApiError::Contract`），MUST NOT 写成首列整体反引号包裹的表格行（`collect_surface_table` 会把它抽成「散文表格复述」，与契约名字集相交即 `ApiError::Contract`）。
- **作者注 = 非 `api-*` fenced 块**：草稿 SHALL 在文件末尾带一个语言标非 `api-*` 的 fenced 块（实现取 `text`）承载起草引导，内容 SHALL 含四要点——① 三类 fence 的落点与唯一性（`api-op` 紧随 `### <op-id>`、`api-types` 为本模块自有 `$defs` 且同文件唯一、`api-reexport` 键 = 导出名 / 值 = 直接子元素 FQN 且同文件唯一）；② 散文只留语义、MUST NOT 用表格复述名字清单；③ `api-reexport` 形态与「散文透传列表须与键集逐名相等」；④ 起草后自查两轴（生成物新鲜度 / 契约判据），并写明契约面未启用时 `enabled = false` 且 `findings` 恒空。
- **作者注的落点封闭**：作者注 MUST NOT 放进裸正文、`## ` 段正文或 `### ` heading——散文区受上述两个抽取面扫描，`### ` 是 op 锚点且其下必须紧随一个 `api-op` fence；非 `api-*` fence 是解析期**唯一**被结构性排除的载体，故说明正文可含 `### `、表格与纯 ASCII 行而不影响装载。MUST NOT 为此在 `contract.rs` 增加「说明块」识别或段名白名单。
- **自证可解析（机械锚）**：有符号元素与无符号元素各取一份草稿，SHALL 经 `api::contract::parse` 零 `ApiError`；解析结果的 `sections` SHALL 恰为六段、`reexport_prose` SHALL 为空集、且六段 `body` 内 MUST NOT 出现作者注正文（说明块惰性的判据）。
- **MUST NOT 自动生成 `api-reexport` fence**：透传键值（导出名 ↔ 直接子元素 FQN）是设计判断；也 MUST NOT 以空 fence `{}` 占位冒充已声明。
- **零参数与零出口变化**：本 delta MUST NOT 增删 `bootstrap` 的入参、action 值集与错误面；CLI 与工具两条出口仍出**同一份**草稿文本，仍只出 stdout / 只回 `{ draft }`，零写盘。

#### Scenario: 六段骨架与尾部惰性说明

- **WHEN** 对任一有实现入口的元素调用 `bootstrap`
- **THEN** 草稿以 `# <FQN>` 起手，依次含六段（职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由），文件末尾是一个 `text` fenced 块；该块内含「api-reexport」与 `{ "导出名": "直接子元素 FQN" }` 两个形态要点；`## 边界` / `## 承载理由` 的正文为含 CJK 的「（待补：…）」占位句

#### Scenario: 草稿零错误装载且说明块零抽取

- **WHEN** 以草稿文本直调 `api::contract::parse(draft, fqn, "<FQN>.md")`
- **THEN** 解析成功（零 `ApiError`）；`sections.len() == 6`；`reexport_prose` 为空集；六段 `body` 内零「作者注」与零「三类 fence」字样（说明块不进任何散文段正文）

#### Scenario: 无符号元素仍出六段

- **WHEN** 对入口存在但抽不到任何函数与类型的元素调用 `bootstrap`
- **THEN** 六段齐全，`## 操作` 段为空、`api-types` fence 内容为 `{}`，草稿仍过 `parse` 零错误（第四条闸的前置不成立，占位散文原样合法）

#### Scenario: 说明错位即装载期报错

- **WHEN** 把作者注形态的行首 `- <ASCII 标识符>` 列表项或首列反引号表格行移出 fenced 块、放进任一 `## ` 段正文
- **THEN** 装载期判 `ApiError::Contract`（前者映射「`api-reexport` 键集与散文透传列表不等」、后者映射「散文表格复述契约名字」），文案带 `file` 与点名项——作者注因此 MUST 留在非 `api-*` fence 内

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/mod.rs` | 契约面装配与出口 | 出口恰八行（与实现相的函数表逐项相等）：`meta() -> ApiMeta`（生成载元单点）/ `enabled(root, elements) -> bool`（契约面启用判据，**本变更收紧**：= 任一 `path` 非空元素的入口文档载体在盘且 `DocFact::Present { has_api_fence: true }`，MUST NOT 退化为「递归任意 `*.md`」）/ `load_contracts(root, elements) -> Result<ApiSnapshot, ApiError>` / `render_all(root, snapshot) -> Result<Vec<GeneratedFile>, ApiError>` / `bootstrap(root, element) -> Result<String, ApiError>` / `check(input) -> Vec<ArchiApiFinding>`（判据语义归 `desktop-archi-api-check`，此处只登记它是契约面唯一出口）/ `api_ref` / `api_ref_of`（契约状态投影两入口）；MUST NOT 另立「比对 / 归一」函数——一致性由 `generate --check` 字节比对 + 编译器两步回答；纯读盘、零写盘、零 spawn。`ApiSnapshot` 为「契约 / 内存 schema / 代码符号集」三面，**不含缺契约清单**（缺口由判据 ② 从模型意图与 `contracts` 的差集直接得出，MUST NOT 另存第二份同一事实）；`index_entries` 的 `degraded` 经 `schema::degraded_marks`（零第二套遍历；口径改一处两处随动）；`render_all` / `generated_path_of` / `index_entries` / `element_symbols` / `bootstrap` 取 `paths::entry_state` 的 `Resolved` 入口（`path.first()` 退出落点面）；无可用入口的元素不产 `.i.*`、`INDEX.toml` 的 `generated` 留空；出口八行与 `ApiSnapshot` 三面零变化；两处调用点补 `elements` / `element`（`element_symbols` 与 `bootstrap`；两者本已持有）；`bootstrap_text` 注记改 `symbol.file`（`entry` 退出注记面）；`entry_state` 取数方式与「无入口不产 `.i.*`」逐字不变 |
| `crates/core/archi/src/api/mod_test.rs` | 索引语料 | `degraded` = 全深度标记出现次数（嵌套属性位计入）；演练 fixture 三处 ⇒ `3`；既有 `render_all的index_degraded派生` 的布尔 fixture 改为词表字符串；bootstrap 注记用例改为「符号定义文件」口径并新增子模块用例 |
| `crates/core/archi/src/api/check.rs` | 十三条判据单点求值（两支首错短路归 `desktop-archi-api-check`） | `check(ApiCheckInput) -> Vec<ArchiApiFinding>`：none 支（`apiUnexpected` → `docMissing` → `docNotFound`）与 declared 支（`docMissing` → `apiDocMissing` → `apiUnexpected` → `apiEntryMissing` → `apiMissing` → 一致性）各首错短路，`apiEntryMissing` 仍经 `entry_state`（`Absent` / `Invalid(_)`，文案按 `EntryProblem` 分列理由）；**已删** `has_generated`（`.i` 的存在不再是入口的替代证据）；`apiDocMissing` 只读 `input.doc_facts`；判据定义归 `desktop-archi-api-check` |
| `crates/core/archi/src/api/check_test.rs` | 判据语料 | `apiEntryMissing` 四理由各一条；`Resolved` 元素不报 ①；`api = none` + `entry` 有值 ⇒ 零判据；`.i` 在盘但无入口仍报 ①；夹具补 `file`（判据断言值零变化） |
| `crates/core/archi/src/api/diff.rs` | 三类差异比对 + 孤儿扫描两处派生 | `api_dir_artifacts(root)` 派生 `<archi_models_dir>/**` 的 `*.schema.json` 与 `<archi_graph_dir>/**` 的 `INDEX.toml`，两处共用**同一私有递归**（收面判据由「等于索引路径」换成「以 `suffix` 结尾」；深度上限 / 符号链接不跟随 / `read_dir` 只此一处照搬，MUST NOT 出现第二份手写递归）；`orphaned` / `mismatched` / `missing` 三列语义零变化（`orphaned` 仍按命名判据与落点深度无关） |
| `crates/core/archi/src/api/emit/mod.rs` + `emit/{rust,ts}.rs` | 渲染面 | 逐字不变（`EmitTarget.form` 仍由调用方给、渲染器零形态推导）；`EntryForm` 三值保留供 TS 侧形态位；`ts_import_specifier` 的接线归前端契约变更（显式留白） |
| `crates/core/archi/src/model.rs` / `parse/toml.rs` / `dto.rs` | 字段面（定义与解析见 `desktop-archi-model`） | `Element.entry` / `RawElement.entry` / `ArchiElement.entry: Option<String>`（缺省 `null`，显式字段不省略）；`deny_unknown_fields` 面内，拼错的字段仍 `unknownField` |
| `crates/core/archi/src/api/paths.rs` | 契约与生成物路径单点 | `normalize_root` / `module_name(element_root)`（Rust / TS 透传命名唯一产出点，**MUST NOT** 参与生成物文件名）/ `rust_module_path(element_root)`（Rust 透传模块路径唯一产出点，MUST NOT 由入口主干互推——`archi-runtime` / `archi` / `lib` 三名互不相等）/ `domain_of(fqn)` / `directory(root)`（转发 `foundation::layout::archi_models_dir`，原 `archi_api_dir` 转发点已退役） / `index_path(root)`（= `foundation::layout::archi_graph_dir(root).join(INDEX_FILE_NAME)`，MUST NOT 由 `directory(root).join(…)` 派生）/ `index_path(root)` / `language_of(file)` / `generated_kind(language)`（语言分派单点，输入改**入口**扩展名）/ `contract_path(root, fqn)` / `schema_path(root, fqn)` / `entry_candidates(element) -> Vec<String>`（**纯文本、零 IO**，三态解析链的唯一产出点）/ `EntryState` + `EntryProblem` + `entry_state(root, elements, element)`（首个落盘候选 + 四条判据的唯一判定点，读盘只在此）/ `EntryForm` 三值（`Crate` / `SubdirModule` / `SingleFile`）由「入口与根的关系」派生（入口 == 根 → `SingleFile`；入口以 `src/lib.rs` 收尾 → `Crate`；其余 → `SubdirModule`），只供 `EmitTarget.form`（TS 侧形态位，本期零 `.i.ts` 实例）/ `generated_path(root, entry, kind) -> Option<PathBuf>`（入参是**入口**而非元素根，产出 `<入口目录>/<入口主干>.i.<ext>`；无入口即 `None`）/ `ts_import_specifier(form, name)`（**待接线留白**：TS 侧入口同级规则的真值尚待前端契约变更实现，MUST NOT 当作既有真值）/ `ref_relative(from_fqn, to_fqn)`；**已删** `mod_path_attr`（`#[path]` 值恒为入口同级文件名，等值性由 `paths_test` 直接断言）；全经 `api::paths` 与 `foundation::layout` 派生（契约目录 = `archi_models_dir`、索引 = `archi_graph_dir`），零行内 `api` / `models` / `graph` 段与零手算相对路径；`module_name` 是纯派生零形态判据；**四类路径派生 MUST 做路径形态校验**（FQN / 元素 `<root>` / 入口非 repo 相对形态——绝对路径 / 盘符 / `..` 段 / 空段 / 空串——即 `None` / `Malformed`），否则 `Path::join` 会替换 base 或上溯，把在盘探测与工具写盘带出仓库；末段进目标语言源码时的形态闸按语言分列（Rust 由 `is_module_segment` 判「标识符且非关键字」，TS 零筛选走 `ts_string_literal` 转义） |
| `crates/core/archi/src/api/paths_test.rs` | 路径面语料 | 解析链三态（显式 / 文件型根 / 约定默认 / `Absent`）+ 四条判据各一反例 + 单文件根声明别的文件 → `OutsidePath`；`generated_path` 三形态落入口同级；`#[path]` 字面量 == `generated_path().file_name()` 自证；`mod_path_attr` 零命中（已删）；`ts_import_specifier` 既有两值在案并标注待接线 |
| `crates/core/archi/src/api/contract.rs` | `<FQN>.md` 解析 + 装载期闸 | H1 锚点校验、`### <op-id>` 分段、三类 fence 提取、双向 gate 三条校验、构造器声明检测；错误带文件与行；`verify` 在同趟遍历内续判 `required ⊆ properties`（族别数不变、不拆第二个公开入口、零 pointer-less 文案），属性键族与 `required` 族的错误带 JSON pointer（`$defs.<name>.properties.<key>` / `$defs.<name>.required[<i>]`）与族别化出路句（`data-testid` 一类键不进契约） |
| `crates/core/archi/src/api/contract_test.rs` | 装载期闸语料 | `data-testid` 首层键 ⇒ 报错含 pointer 与出路句；`required` 悬垂 ⇒ 报错含 `required[0]`；自洽契约通过且渲染为必填 |
| `crates/core/archi/src/api/schema.rs` | `$defs` 规则 + 装配 + 债表扫描（说明段目录串换段） | `build_index` 说明段的「契约面：目录 …」串 SHALL 取 `foundation::layout::archi_models_rel_dir()`（原 `archi_api_rel_dir()` 已删；MUST NOT 取索引自身所在目录）；`IndexEntry.schema_rel` 仍是 repo 相对路径（现含 `models/<分层目录…>/`）；`$defs` 身份 / 提升原名 / `$ref` 顶层 / 禁按结构去重 / 透传条目纯 `$ref`；`schema.json` 六字段装配（`$schema` / `$id` / `title` / `$defs` / `properties` / `$comment`）；`INDEX.toml` 装配；`DEGRADED_KINDS`（四值封闭常量，与校验同处，MUST NOT 进 `common.json`）；`degraded_marks(schema) -> Vec<(String, &str)>`（全深度单点扫描）；`verify_defs` 同趟校验取值（非字符串亦拒，文案点名 pointer 并列四值）；`IndexEntry.degraded` 语义 = **标记出现次数**（不再是 `$defs` 条目数） |
| `crates/core/archi/src/api/schema_test.rs` | 词表语料 | 四值逐个通过；词表外值与非字符串值拒收（文案列四值）；`note` 不计入 |
| `crates/core/archi/src/api/extract/mod.rs` | 取数器分派 + **TS 导出闭包** + nullable 形状识别单点（本变更：仅模块 doc 方位串） | 模块 doc 的契约权威位置改指 `<archi_models_dir>/<分层目录…>/<FQN>.md`（doc-only，代码零改动；该串既非引号字面量、也不会被按符号 / 字面量扫描抓到）； 按语言分派到 `rust` / `ts`；产出候选 `{ symbol, kind, location, signature, visibility }` 与引用方（`used_by`，本期留白）栏；仅 bootstrap 使用，非权威；`NullableBreakdown { non_null: Option<&Value>, explicit_null: bool }` + `nullable_breakdown(schema: &Value) -> Option<NullableBreakdown>`（三分支识别唯一实现，沿用既有「取首个非空分支」语义），`schema_type` 改经它取非空分支、**对外结果逐值不变**（`non_null: None` 即纯 null 档 → `Optional(Unknown("null"))`），`explicit_null` 是渲染面 `\| null` 的唯一依据（MUST NOT 在 `emit` 侧第二次判 null）；函数**不收 `required`**——可选轴不属它，`schema_type` 签名不因本变更扩参；`ApiSymbol` 新增 `file: String`（定义文件，repo 相对 POSIX；进程内类型，不进 `dto` / 不进 specta）；`extract_element(root, elements, element, entry)`：Rust 分支 = `rust::extract_crate` 逐字不变，TS 分支 = 入口派生闭包（递归 `surface` + visited(文件, 名字集) + `scan::ts::TsResolver::resolve` + `scan::owner::OwnerIndex::owner_of` 的 owner 子树 + `(name, kind)` 去重 + `A as B` 改名），入口读不到 / 语言未知 ⇒ 空集；`extract(element, file, source)` 仍是**显式留白（D52）零产品调用者**，本变更 MUST NOT 接线；边界 doc 更正为「`extract` / `ts` / 归一纯函数零读盘；`extract_element`（与 `rust::extract_crate`）按入口读盘、零写盘、零 spawn」 |
| `crates/core/archi/src/api/extract/mod_test.rs` | 单点对拍语料 | 9 组样例**三轴分列**（可空轴六组：`Optional` ⟺ `\| null` ⟺ `Option<`；可选轴两组：`schema_type` 同值、TS `a`/`b?` 分形、Rust `String`/`Option<String>` 分形；纯 null 一组：`Optional(Unknown("null"))` / `null` / `serde_json::Value`）；`schema_type` 结果与改动前逐值相同；闭包五类语料（精确名单 / 通配等价 / 环与菱形 / owner 子树四值 / 别名改名 / 稳定性）落在本文件 |
| `crates/core/archi/src/api/extract/rust.rs` | Rust 声明抽取 | `syn` 遍历 `pub struct` / `trait` / `enum` → `$defs`，`pub fn` → `x-function`；签名 AST 归一单行；`visibility` 四值（`pub mod` 链 / `pub(crate)` / 私有 `mod` 内的 `pub` / `#[cfg(test)]` 树）；`ApiSymbol.file` 填值为**当前模块文件**（贯穿 `walk_items` / `walk_modules`；内联 `mod` 与父同文件）；抽取面 / `visibility` 四值 / 签名归一 / `#[path]` 限根纪律逐字不变 |
| `crates/core/archi/src/api/extract/ts.rs` | TS 声明抽取 | `oxc` 遍历：`export function` → 操作（唯一判定），`export type` / `interface` / `class` → `$defs`，`export const` 按初始化表达式分类（`memo(...)` → 组件留白 / 对象与 Proxy → 排除 / 箭头函数 → **不作操作**，沿本 capability「MUST NOT 为从初始化表达式推断函数造启发式」）；`unused` 的降级接口 `apply_knip_report` 已实现但**本期未接线**（生产面不产出 `Unused`，见待决 5 / 12）；接线是独立变更；`extract_file(file, source)` 签名与产出语义逐字不变（`= extract_surface(..).symbols`）；新增 `extract_surface(file, source) -> ExportSurface { symbols, named: Vec<NamedExport{local, exported, specifier}>, star: Vec<String> }`（纯读源文本 + AST）；`exported_item(statement) -> ExportedItem` 三态单点（`Declaration` / `Named` / `Star` / `None`），`match statement` 恰 1 处；`Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中；每符号填 `file` |
| `crates/core/archi/src/api/extract/{ts_test,rust_test}.rs` | 源码自证 | `ts_test` 的闸 1（签名逐字）与闸 3（透传零符号）**保留不改**，闸 2 改为「`match statement` 恰 1 处 + 臂集合三值 + 两类留白零命中」，并新增 `extract_surface` 三面断言；`rust_test` 补「子模块 / 内联 `mod` 的 `file`」 |
| `crates/core/archi/src/api/emit/rust.rs` | `.i.rs` 渲染 | `struct` / `trait` / `enum` / newtype / 自动构造器 / 透传 `pub use`；头注释含工具版本与 `common.json` 摘要；纯函数出字符串；`value_type` 同样前置 nullable 分支（与 `schema_type` 同序，可空 → `Option<T>`）；`struct_block` 的可选包装经单点幂等函数 `optional_of`（已 `Option<…>` 即不二次包），`Option<Option<` 零产出 |
| `crates/core/archi/src/api/emit/ts.rs` | `.i.ts` 渲染 | `interface` / `as const` + `type` / 别名 / 透传 `export { } from`；ambient-free（MUST NOT 用 `.d.ts`）；头注释同上；`value_type` 在 `$ref` / 映射规则之后、`type` 分派之前处理 nullable（与 `schema_type` 同序）：可空 → `T \| null`（唯一产出点）、`array` 元素位可空 → `(T \| null)[]`、纯 null 形态 → `null`；`interface_block` 对非 `required` 字段只出 `{field}?:`，零 `\| null` 拼接；零标识符判据 |
| `crates/core/archi/src/api/emit/ts_test.rs` / `rust_test.rs` | 渲染语料 | `a: string` / `b?: string` / `c?: string \| null` 三态；纯 null 档 TS `null` / Rust `serde_json::Value`（不变）；`(T \| null)[]` 括号；`Option<Option<` 零命中；演练草案 fixture（D.1 改写为词表形态）金样零 `unknown` 泄漏 |
| `crates/core/archi/src/scan/ts.rs`（与 `scan/owner.rs` 的 `owner_of`） | 被复用的既有单点 | **本变更零改动**：`TsResolver::resolve(&mut self, …)`（只读缓存、无 `unresolved` 副作用）与 `OwnerIndex::owner_of`（三候选并集、最长前缀、`Ambiguous` 不任选）原样复用；`api → scan` 的内部依赖新增一条（与 `api/diff.rs` 的 `walk_source_files`、`api/paths.rs` 的 `is_source_file` 同族） |
| `crates/core/archi/src/scan/owner.rs` | 生成物归属（判据修正） | 剥离候选集由「双形态」扩为**三候选**：整组剥离 `<name>.i.<ext> → <name>` ∪ 替换扩展名 `<stem>.i.<ext> → <stem>.<ext>` ∪ **主干对齐**（`<name>` 与文件型 `path` 的去扩展名形态全等）；`owner_of` 对每候选各跑一遍最长前缀、按 `(fqn, 前缀长度)` 合并去重后定档（同长仍 `Ambiguous`），**MUST NOT 候选短路**；`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不变；非 `.i.<ext>` 路径归属零变化 |
| `crates/core/archi/src/scan/owner_test.rs` | 归属语料 | 三候选各正例（`crates/infra/archi/src/lib.i.rs` / `views/welcome-view.i.ts` / `components/agent/index.i.ts` / `src/dir.ts` 根对 `src/dir.i.ts`）；`src/a.ts` 与 `src/a.tsx` 同场 ⇒ `Ambiguous`；白名单外四条反例与 `src/dir.i.ts` 并集定档用例保持绿 |
| `crates/core/archi/src/scan/mod.rs` | 扫描编排（零判据改动） | `walk_source_files` / `is_source_file` 提为 `pub(crate)` 供文件树复用；`scan()` 的枚举 / 排除 / 归属 / 抽取 / 聚合六段语义逐字不变（`owner_of` 的调用点两处——源文件归属与目标归属——自动获得并集判据） |
| `crates/core/archi/src/tree.rs`（新增，见 `desktop-archi-file-tree`） | 文件树投影 | 归属与剥离一律经 `OwnerIndex`；本能力只要求「生成物按并集判据归属，树侧零第二份实现」 |
| `foundation::layout::archi_models_dir` / `archi_models_rel_dir`（消费者扩容，见 workspace-layout-resolution） | 契约资产目录派生 | 契约资产目录 == `archi_models_dir(root)`（同值、零新增函数——MUST NOT 新造 `archi_contract_dir` 一类别名）；函数体与返回值逐字不变 |
| `foundation::layout::archi_graph_dir` / `archi_graph_rel_dir`（新增，见 workspace-layout-resolution） | 契约索引派生 | `archi_graph_dir(root) -> PathBuf` = `<archi_dir>/graph`；`archi_graph_rel_dir() -> String` = `<archi_rel_dir>/graph`；全从既有派生族派生，纯推导零 IO；`API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir` 已删 |
| `crates/infra/archi/src/lib.rs` + `src/lib.i.rs` | 入口编排与生成物（落点迁移样本） | 入口行 `#[path = "lib.i.rs"] mod api_gen; pub use api_gen::*;`（值 = 入口同级文件名，零 `../`）；生成物在入口同级、内容逐字不变；旧 `crates/infra/archi.i.rs` 已删（否则 `api:check` 的 `orphaned` 非空） |
| `crates/tools/archi-api-gen/src/main.rs`（新，见 desktop-crate-layout） | 唯一写盘面 | CLI 三子命令 `generate` / `--check` / `bootstrap`；`--check` 零写入、有差异即非零退出并列文件；目录创建经 layout 派生 |
| `packages/desktop/package.json`（脚本面） | 生成与门禁驱动 | `api:export`（写盘）/ `api:check`（`--check` 零写入 + 工具内比对）；`check` 由三段扩为四段（`server:check && bindings:export && api:check && client:check`——第二段是 `bindings:export`，生成物零 diff 守卫已退役），`bindings:export` 与 `client:check` 两段原文不变，`server:check` 的 fmt 段为 `cargo fmt --all --check`（clippy 段逐字不变）；`version` 相对落地时 `HEAD` 现值 +1 patch（`tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动） |
| `packages/desktop/knip.json` | 生成物可见性边界 | `ignore` 增 `src/**/*.i.ts`；`ignoreExportsUsedInFile` / `ignoreDependencies` 零新增 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | IPC 类型再生成 | 含 `ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` / 报告 `apiFindings`；再生成后入库、过期由提交前人工核对（零 diff 守卫已退役） |
| `.dev-team/architecture/models/<分层目录…>/**`（契约资产新落点） | 契约与生成物资产 | `<分层目录…>/<FQN>.md`（唯一手写）+ `<分层目录…>/<FQN>.schema.json`（生成，与 md 同目录）；由 `git mv` 自 `<archi_dir>/api/<分层目录…>/**` 搬入，内容逐字不变；两者入库、进零 diff 门 |
| `.dev-team/architecture/graph/INDEX.toml`（契约索引新落点） | 契约索引 | 由 `git mv` 自 `<archi_dir>/api/INDEX.toml` 搬入后由 `api:export` 重生成；差异恰为说明段目录串（`<archi_models_rel_dir>`）与已声明元素的 `schema` 列（repo 相对路径含 `models/<分层目录…>/`）两行，其余条目逐字不变 |
| `.dev-team/architecture/api/**` | **删除** | `api` 段退役，迁移后该目录不存在；孤儿枚举严格两处派生（`<archi_models_dir>/**` + `<archi_graph_dir>/INDEX.toml`），故 `api/` 残留对 `api:check` 不可见——缺席由 `git mv`（同提交）+ 目录树读数 + `pilot_test` 负向断言守，MUST NOT 写成「`api:check` 报应删未删」 |
| `AGENT.md` + `CLAUDE.md`（仓库根文档，见「仓库根文档的契约纪律节」） | agent 侧操作纪律入口（本变更：第 ① / ② / ⑤ 条目录串换段） | 第 ① 条手写真相 `<archi_models_dir>/<分层目录…>/<FQN>.md`、第 ② 条 `INDEX.toml` 方位 `<archi_graph_dir>`（原写「契约目录根」，`grep 'architecture/api'` 抓不到它）、第 ⑤ 条索引串 `<archi_graph_dir>/INDEX.toml`；`## 模块 API 契约` 节恰 5 条要点（手写真相位置与三类 fence / 四类生成物勿手改 / 两脚本与 `check` 链段序 / `api = none` 留白 / 入口文档承诺已成立元素的判据进 `status`、其中 `error` 档进 `passed`）；第②条 = 「实现入口同级」+ `<入口主干>.i.rs` / `<入口主干>.i.ts`（与「生成物落点与入口查找」同口径），条目数恒 5、其余四条要点逐字不动；MUST NOT 复制契约正文、判据长表或 fence 示例；零 `entry` 字段说明（字段知识归 `desktop-archi-agent` 的知识文本）；节内零 `openspec/specs/**` 路径；两文件该节正文逐字相同（同源纪律） |

### API: 生成物驱动面（零命令形态变化，实现落点变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `packages/desktop/package.json` | 零改动 | `api:export` / `api:check` / `check` 三条命令串逐字不变；`version` 相对 `HEAD` +1 patch |
| `crates/core/archi/src/api/**` | 追加 | 渲染 / 比对纯函数（`workspace_render` / `compare`）为四条消费路径的共同实现；契约格式与判据面零改动 |
| `crates/infra/archi/src/api_artifacts.rs` | 新增 | 写盘单点（`write_artifacts`）；CLI 与 IPC 共用 |

### API: 取数器内部面（无对外 CLI / IPC 变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `api::extract::extract_element` | 改（签名 + TS 语义） | `(root, elements, element, entry)`；TS 分支由单文件扩为入口导出闭包；Rust 分支逐字不变 |
| `api::extract::ts::extract_surface` | 新增 | 声明 + 具名名单 + 通配名单三面；`extract_file` 委托它取声明面 |
| `api::extract::ts::extract_file` | 零变化 | 签名 `(file: &str, source: &str) -> Vec<ApiSymbol>` 与产出语义逐字不变（闸 1 / 闸 3 的判红面） |
| `api::extract::ApiSymbol` | 改 | 增 `file: String`；`PartialEq` / 字段序外延，既有构造点与夹具同批补值 |
| `api::bootstrap` | 零签名变化 | `bootstrap(root, element) -> Result<String, ApiError>` 不变；内部注记改按 `symbol.file` 输出 |
| `paths::ts_import_specifier` | 零变化 | 仍为待接线留白（`.i.ts` 零实例），MUST NOT 接线 |

### Component: 仓库根文档

| 项 | 变化 | 描述 |
|---|---|---|
| `AGENT.md` / `CLAUDE.md` | 改 | `## 模块 API 契约` 节第 ⑤ 条为纳管口径（入口文档承诺已成立元素的判据进 `status`、其中 `error` 档进 `passed`）；本变更再同步第 ① / ② / ⑤ 条的目录串（手写真相 `<archi_models_dir>/<分层目录…>/<FQN>.md`、索引方位 `<archi_graph_dir>`、索引串 `<archi_graph_dir>/INDEX.toml`），第 ③ / ④ 条零改动，条目数仍 5；两份副本该节正文逐字相同；第 ① 条另补一句「散文不复述名字清单（唯一持有者是 fence）」——要点数仍 5、其余四条逐字不动 |

### 模块增量：入口文档语义定位换面（module-doc-field-docs-retirement）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|------|
| `crates/core/archi/src/api/paths.rs` | 落点派生 | `directory(root)` == `archi_models_dir(root)`、`contract_path` == `<models>/<分层目录…>/<FQN>.md`、`schema_path` / `index_path` 逐字不变；**载体派生单点**：declared → `contract_path`、none → `doc` 归一后 join root（形态校验复用 `normalize_root` + `is_repo_relative`，后者提 `pub(crate)`）；MUST NOT 出现第二份路径归一实现（`docs::normalize_rel` 已随 `docs.rs` 删除） |
| `crates/core/archi/src/api/mod.rs::enabled` | 启用判据（**收紧**） | `enabled(root, elements)` = 任一 `path` 非空元素的载体在盘且 `DocFact::Present { has_api_fence: true }`；MUST NOT 退化为「递归任意 `*.md`」；命令层 MUST NOT 另判一套 |
| `crates/core/archi/src/api/mod.rs::load_contracts` | 契约装载（输入源零改动） | 只认 `contract_path` 派生落点（`doc` 不参与装载）；`ApiSnapshot` 增 `doc_facts`（`Absent` / `Missing` / `Present { has_api_fence }`），每份 doc 只读一次即弃 |
| `crates/core/archi/src/api/contract.rs` | 契约解析 | 五类位置语义、双向 gate、`$defs` 规则、落点闸逐字不变；仅 `FENCE_OP` / `FENCE_TYPES` 提 `pub(crate)` 供启用判据与 `apiDocMissing` 复用 |
| `crates/core/archi/src/api/{schema,render,emit,extract,diff}.rs` | 生成与取数 | 逐字不变（`doc` 不是生成物；`.i.*` 落点仍是入口的函数） |
| `crates/core/archi/src/dto.rs::ArchiApiRef` | 契约状态投影 | 四字段（`status` / `contractPath` / `schemaPath` / `generatedPath`）零改动；`declared` 元素的 `contractPath` 同时是它的**入口文档**路径（前端据此渲「契约」行，不得再渲第二条入口文档） |
| `.dev-team/architecture/models/**`（仓内资产） | 试点契约 | `.dev-team/architecture/models/Infra/Backend.Infra.ArchiRuntime.md` 与 `.schema.json` 逐字不变（它是启用判据的唯一在案证据） |

### 模块增量：装载期名字清单闸（archi-doc-api-surface-unification）

主基线既有行逐字沿用；本增量只增第四条 gate 的实现面，资产形态 / 目录布局 / schema / 索引 / `.i.*` 派生与判据词汇面零改动：

| 模块 | 职责 | 关键契约 |
|------|------|------|
| `crates/core/archi/src/api/contract.rs` | 契约解析 + **装载期闸群收口** | 五类位置语义、`### <op-id>` 分段、三类 fence 提取、构造器声明检测逐字不变；`verify` 在既有三闸（op/heading ≡ fence 归属、`api-types` ⊆ `$defs`、`api-reexport` ≡ 散文列表）+ 闸一（七族名字）+ 落点闸 + 自洽闸之后**追加第四条闸**（散文表格名字 ∩ 契约名字集 = ∅，前置为 `ops`/`types`/`reexports` 任一非空）；错误一律 `ApiError::Contract`、带文件与点名项；模块头注释的「三条双向 gate」表述 SHALL 同步为四条 |
| `crates/core/archi/src/api/contract_test.rs` | 装载期闸语料 | 既有闸语料逐字不变；新增抽取面正反例与第四条闸正反例（含留白豁免与「补契约即转红」两态）；新用例 MUST 落在本文件内，MUST NOT 新增 `api/**` sidecar 文件（计数断言恰 13 份） |
| `crates/core/archi/tests/pilot_test.rs` | crate 级全量装载守护 | 新增一条用例：`<archi_models_dir>` 递归全部 `*.md` 经 `parse` + `build_schema` + `verify` 全线通过（当前 10 份：后端 2 份闸生效且零表格行、前端 8 份走留白豁免） |

#### Function: 契约装载期抽取面（`crates/core/archi/src/api/contract.rs`，零新增公开出口）

| 函数 | 变化 | 语义 |
|---|---|---|
| `collect_surface_table(sections: &[ContractSection]) -> Vec<String>` | 新增（私有） | 扫 `## <中文二标题>` 段正文的表格行：去前导空白后以 `|` 起手 → 按 `|` 切首列 → 去空白后要求整体被一对反引号包裹且内容是 `is_identifier` ⇒ 计入；去重保序。fence 内容不在 `section.body` 内（解析期状态机保证），列表项与裸正文零抽取 |
| `verify(contract: &Contract, schema: &Value) -> Result<(), ApiError>` | 改（签名零变化） | 追加第四条闸：前置 `ops` / `types` / `reexports` 任一非空；断言抽取集 ∩（op id ∪ `api-types` 键 ∪ `api-reexport` 键）= ∅；非空即 `ApiError::Contract`，点名文件 + 交集名字 + 出路句（名字归 fence、散文只留语义）。既有各项（三闸 + 闸一 + 落点闸 + 自洽闸）逐字不变 |

- **零变化面（同批断言，不是省略）**：`api::load_contracts` / `api::enabled` / `api::check` / `api::bootstrap` / `api::render_all` / `api::api_ref` 签名与语义逐字不变（第四条闸经既有 `contract::verify` 调用点自动生效）；`Contract` / `ApiSnapshot` / `ArchiApiFinding` / `ArchiDriftReport` 零字段新增、零新 finding kind、`REPORT_VERSION = 3` 不变；IPC 命令面 / DTO / `src/types/generated/**` 零改动（`bindings:export` 幂等、`client:check` 零 diff）；`crates/tools/archi-api-gen` 与 `packages/desktop/package.json` 三条命令串零改动（闸只作用于装载期，不参与渲染 ⇒ `.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` 逐字节零 diff）；`views/architecture/components/contract-panel.tsx` / `model-info.tsx` 零改动（无 `kind` / `category` 增删）。

### 模块增量：入口面读数与生成物中继元素（relation-api-dependency-check）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/paths.rs` | 入口解析与落点派生（零改动） | `entry_state` 三态链、四条判据、`generated_path` 与 `ts_import_specifier` 逐字不变（新元素只走既有 ③ / ④ 分支） |
| `crates/core/archi/src/api/check.rs` | 入口判据（零改动） | `api = none` 退出 `apiEntryMissing` 的语义不变；`Absent` 只在 `api = declared` 时产条目 |
| `crates/core/archi/src/dto.rs`（`resolved_entry` 投影） | 入口读数（零改动） | `archi_module` 的 `resolvedEntry` 对 `Absent` 给 `via = null` + `problem` 文案；新元素因此可被 `archi_module` 直接判出「无入口」 |
| `.dev-team/architecture/graph/model/06-frontend.toml` | 数据面（见 `repo-archi-model`） | 新元素 `api = none` / 无 `entry` / `path` = 生成目录；不为其造 `index.ts`、不为其补 `entry` 行 |
| 本能力实测段（spec 面） | 改 | 入口面读数 44 / 0 / 0 → **46 / 45 / 1 / 0**，并把「`Absent` 一例为何不是债」写实 |

### Function: `crates/core/archi/src/api/paths.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `entry_state` | 零改动 | 三态链与四条判据逐字不变；新元素落 `Absent`（目录型根、默认候选全不落盘） |
| `generated_path` / `ts_import_specifier` | 零改动 | 落点派生与 TS 侧规则不变；`Absent` 不产生成物路径 |
| `carrier_path`（入口文档载体） | 零改动 | `api = none` 取 `doc`（新元素未声明 ⇒ `docMissing`），`declared` 取派生契约路径 |

### API: `archi_module.resolvedEntry`

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiModuleDetail.resolvedEntry` | 字段级（零改动） | 新元素呈 `path = null` + `via = null` + `problem`（「无可用入口」）；`archiEntryMissing` 不因它产出（`api = none` 退出入口判据） |

### 模块增量：契约起草引导与描述补位（archi-contract-authoring-guidance）

主基线既有行逐字沿用；本变更只把草稿形态由四段扩为六段骨架 + 尾部惰性作者注（契约资产形态、三类 fence 与装载期闸群、生成物派生与零 diff 门、取数面与判据、入口查找逐字不变），受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/mod.rs`（`bootstrap_text`） | 草稿骨架单点（本变更改形态） | 出 H1 + 六段（`## 边界` / `## 承载理由` 追加在 `## 类型` 之后）+ 末尾 `text` fence 作者注；两段占位句含 CJK；`bootstrap(root, element) -> Result<String, ApiError>` 签名、错误文案与「零写盘、只回 stdout 语义」逐字不变；`bootstrap_text(fqn, symbols)` 形参不变（`entry` 仍不进注记面） |
| `crates/core/archi/src/api/mod_test.rs` | 草稿断言面 | `bootstrap草稿形态` 扩到六段；新增「六段 + 过 `parse` 零错误 + `sections == 6` + `reexport_prose` 空 + 六段 body 零说明正文」；新增「无符号元素仍六段」；既有 `bootstrap_text` 签名断言与注记行用例保留 |
| `crates/core/archi/src/api/contract.rs` | 解析器（零改动） | 三类 `api-*` fence 归集、其余 fence `continue`、`collect_reexport_prose` / `collect_surface_table` 两个抽取面、装载期闸群（三条双向 gate + 闸一 + 落点闸 + 自洽闸 + 第四条闸）逐字不动——本变更只依赖它们，MUST NOT 增加「说明块」识别、段名白名单或第五类 fence |
| `crates/tools/archi-api-gen/src/main.rs` | CLI 出口（零改动） | `bootstrap <元素>` 仍直出 `api::bootstrap` 的同一份文本到 stdout、零写盘；`generate` / `--check` 两子命令与前两者共用的路径派生零改动 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具出口（零改动） | `bootstrap` 分支仍回 `{ draft }`（同一份文本），过 `cap_result` 单点截断 |

### Function: `crates/core/archi/src/api/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `bootstrap_text(fqn, symbols) -> String` | 改（私有，签名零变化） | 追加 `## 边界` / `## 承载理由` 两段占位与末尾 `text` fenced 块作者注；既有 H1 / 职能 / 不变量 / 操作 / 类型五处文本逐字保留；作者注为静态文本（零占位符、零变量插值） |
| `bootstrap(root, element) -> Result<String, ApiError>` | 零改动 | 入口解析与错误面（`Element` / 无入口 / 符号抽取失败）逐字不变；仍零写盘 |

### API / CLI: 出口形状（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api(action = "bootstrap")` 响应 | 形状零变化 | 仍 `{ draft: string }`；仅 `draft` 内容按新形态，`cap_result` 单点截断不变 |
| `archi-api-gen bootstrap` | 形状零变化 | 仍 stdout 单段文本、退出码零变化 |
| DTO / IPC / 生成物 | 零改动 | `Contract` / `ApiSnapshot` / `ArchiApiFinding` / `ArchiApiCheck` 零字段新增；`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` 逐字节零 diff；`bindings:export` 零差异 |

- **零变化面（同批断言，不是省略）**：`api::load_contracts` / `api::enabled` / `api::check` / `api::artifact_check` / `api::findings` / `api::render_all` / `api::api_ref` 签名与语义逐字不变；十三条 finding 判据词汇与 `REPORT_VERSION` 不变；存量的 10 份契约资产与 `.dev-team/architecture/graph/model/*.toml` 零改动（不回填作者注、不新增元素 / 关系）。
