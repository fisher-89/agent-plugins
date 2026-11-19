# desktop-archi-api Specification（delta: archi-doc-api-surface-unification）

## Purpose

本 delta 只改**文档内容面的名字清单归属**；资产形态、目录布局、schema / 索引 / `.i.*` 三面派生与判据词汇面**零改动**：

- 名字清单（op id / `api-types` 键 / `api-reexport` 键）的唯一持有者是契约 fence；散文段 MUST NOT 以**表格**复述这些名字——新增第四条装载期闸（抽取表格首列的 ASCII 标识符，与契约名字集求交，非空即 `ApiError::Contract`），前置为「契约非留白」（`ops` / `types` / `reexports` 任一非空）。
- 闸的判据方向是**交集为空**（不是「散文镜像契约」）：闸只判点名、不判语义，散文的职能 / 边界 / 不变量 / 承载理由仍是人审面。
- 前端 8 个组件元素的契约当前全空 ⇒ 留白豁免成立，其「公共面」表（27 条数据行）**原样合法**，属**临时债**；它们的名字将来随组件契约变更迁入 `$defs` / `api-op`，届时同提交不清表即装载期失败——退役由第四条闸机械强制，不是纪律要求。
- 仓库根文档的契约纪律节第 ① 条补一句该纪律（条目数仍 5、两文件同源）——架构会话知识文本因 3KB 骨架上限贴边（3071 / 3072 字节）本变更不动。

## MODIFIED Requirements

### Requirement: 契约资产形态与唯一手写真相

模块公共 API 契约 SHALL 以**手写 markdown 为唯一真相**，落位 `<archi_models_dir>/<分层目录…>/<FQN>.md`（`<archi_dir>` 经 `foundation::layout::archi_dir`、契约目录取既有 `foundation::layout::archi_models_dir` —— `api` 段与 `archi_api_dir` 族已退役，MUST NOT 行内字面量、MUST NOT 新增第二个目录派生函数）。`<分层目录…>` SHALL 是元素 FQN 的**祖先链镜像**：取 FQN 段链去掉**最外层根段**与**叶段**后的逐段路径（`App.Backend.Infra.ArchiRuntime` → `Backend/Infra/`、`App.Frontend.Components.Agent.AgentInput` → `Frontend/Components/Agent/`、`App.Backend` 与单段 `App` → 契约目录根零段），元素身份仍是 FQN（文件名恒为完整 FQN）、目录只是布局——同一末段的不同元素（`Backend.Core.Agent` 与 `Frontend.Components.Agent`）因祖先链不同而落在不同层次，不再混装同一目录。

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
- **名字清单唯一持有者与第四条 gate（本变更新增，机械可校验）**：名字清单的唯一持有者是契约 fence（`api-op` 的 op id / `api-types` 的键 / `api-reexport` 的键），散文段 SHALL NOT 以表格复述这些名字。判据闭合为三段：
  1. **抽取面**：散文段正文的**表格行**——去前导空白后以 `|` 起手，按 `|` 切出的首列去空白后**整体被一对反引号包裹**且内容是 ASCII 标识符（`is_identifier`）时计入；其余形态一律跳过。fence 内容天然不在散文正文里（解析期的 fence 状态机已隔断），**列表项与裸正文 MUST NOT 抽取**——列表项是既有 `api-reexport` 散文透传列表与留白登记的形态（其一致性归第三条 gate），裸正文是语义散文。
  2. **前置（留白豁免）**：契约非留白，即 `ops` / `types` / `reexports` 三项**任一非空**（取手写真假，MUST NOT 取生成的 `$defs`；只写透传的契约同样持有名字，故 `reexports` 与另两项同权）。三项全空即「契约面留白」，该元素的散文表格原样合法——这一支是前端 8 个组件元素当前形态的唯一合法通道，且属**临时债**（见下条）。
  3. **断言**：抽取集 ∩ 契约名字集（`api-op` 的 op id ∪ `api-types` 键 ∪ `api-reexport` 键）= **∅**。MUST NOT 写成「集合相等 / 散文必须镜像契约」——闸判的是「同一名字不得两处持有」；散文的职能 / 边界 / 不变量 / 承载理由仍是人审面，本闸**只判点名、不判语义**。
  违反即 `ApiError::Contract`（`load_contracts` 中止，零新增 finding kind、不进 `apiFindings`、不动十三值词汇 / 生成物 / 金样），文案 SHALL 点名文件与被复述的名字并给出路句（「名字归 fence，散文只留语义」）；行号不作承诺（与既有两条 gate 同档——段落正文未保留行号）。闸作用域为**全部 `##` 散文段**，MUST NOT 引入段名白名单或豁免清单（`## 公共面` 是临时形态，冻结成机器约定会把留白债写成结构）。
- **前端组件契约的留白是临时状态，退役由第四条 gate 机械强制**：8 个前端元素（`App.Frontend.Components.Agent` 与其 7 个子元素）的契约三项当前全空、散文「公共面」表共 27 条数据行 ⇒ 前置不成立、闸不生效、表格**原样合法**。它们的名字清单将来由组件契约变更迁入 `$defs` / `api-op`（须先解 `$ref` 归一对称 / 组件与 JSX 元素的契约词汇 / `.i.ts` 末尾空行三处缺口，属另起变更）；一旦某元素契约非留白而同一提交未清表格，装载期即失败——**退役不是纪律要求，是闸的机械后果**。本变更 MUST NOT 提前删表（27 条人读语义无处安放），也 MUST NOT 以 `apiUndeclared` 抬档代偿（留白期抬档会把临时债读成判据面债务）。
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

- **WHEN** 在 `<archi_dir>/graph/model/` 放六个 `NN-<域>.toml`，同时在 `<archi_dir>/models/` 的 `Backend/Core/`、`Frontend/Components/Agent/` 三级分层目录下各放 `<FQN>.md` 与 `<FQN>.schema.json`，随后跑一次模型解析与一次契约装载
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

### Requirement: 仓库根文档的契约纪律节

仓库根 `AGENT.md` SHALL 承载一节 `## 模块 API 契约` 作为 agent 侧的操作纪律入口，内容 SHALL 限于**指针与最简规则**，MUST NOT 复制契约正文、判据长表或 fenced json 示例：

- 该节 SHALL 恰含五条要点：① 手写真相位置 `<archi_models_dir>/<分层目录…>/<FQN>.md` 与三类 fence 的名字（`api-op` / `api-types` / `api-reexport`），及一句「散文段 MUST NOT 以表格复述契约名字（名字清单的唯一持有者是 fence）」；② 四类生成物勿手改（`.schema.json`（与 md 同目录）/ `INDEX.toml`（`<archi_graph_dir>`）/ **实现入口同级**的 `<入口主干>.i.rs` / `<入口主干>.i.ts`）与「已提交手改即零 diff 门红、未提交即被静默覆盖」；③ `api:export` / `api:check` 两脚本与 `check` 链的段序；④ `api = none` 是显式留白而非「还没写」；⑤ **入口文档承诺已成立**元素的判据进 `status`（`error` 档还可进 `passed`），索引与计数经 `<archi_graph_dir>/INDEX.toml` 查看，两轴经架构页「契约」视图一眼看全。
- 该节需换段的目录串 SHALL 恰为三条要点：第①条（`<archi_models_dir>/<分层目录…>/`）、第②条（`INDEX.toml` 方位 `<archi_graph_dir>`）、第⑤条（索引串 `<archi_graph_dir>/INDEX.toml`）；第③ / ④ 条零改动。**换段完成后该节 `grep 'architecture/api'` 与 `grep '契约目录根'` 均零命中**——第②条不含 `architecture/api` 字面量，只测字面量归零不足以证明换段完成。
- 第①条的目录串 SHALL 与「契约资产形态与唯一手写真相」的落位口径逐字一致（`<archi_models_dir>/<分层目录…>/`，零 `api` 段残留）；第②条的 `INDEX.toml` 方位 SHALL 写「`<archi_graph_dir>`」而非「契约目录根」——索引与契约已分家，写成契约目录根会指错路径。
- **第①条的名字清单纪律句（本变更新增）**：该句 SHALL 只表达「散文不复述名字清单、唯一持有者是 fence」一条事实，MUST NOT 展开成第四条 gate 的判据条件、错误文案样例或豁免规则（定义归本条与 `desktop-archi-api-check`）；它是 agent 侧唯一可见的该纪律出处（架构会话知识文本体量受 3KB 骨架上限约束，本变更不向其追加纪律句），故该句 MUST NOT 被省略。
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

## Module Contract

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `crates/core/archi/src/api/contract.rs` | 契约解析 + **装载期闸群收口** | 五类位置语义、`### <op-id>` 分段、三类 fence 提取、构造器声明检测逐字不变；`verify` 在既有三闸（op/heading ≡ fence 归属、`api-types` ⊆ `$defs`、`api-reexport` ≡ 散文列表）+ 闸一（七族名字）+ 落点闸 + 自洽闸之后**追加第四条闸**（散文表格名字 ∩ 契约名字集 = ∅，前置为 `ops`/`types`/`reexports` 任一非空）；错误一律 `ApiError::Contract`、带文件与点名项；模块头注释的「三条双向 gate」表述 SHALL 同步为四条 |
| `crates/core/archi/src/api/contract_test.rs` | 装载期闸语料 | 既有闸语料逐字不变；新增抽取面正反例与第四条闸正反例（含留白豁免与「补契约即转红」两态）；新用例 MUST 落在本文件内，MUST NOT 新增 `api/**` sidecar 文件（计数断言恰 13 份） |
| `crates/core/archi/tests/pilot_test.rs` | crate 级全量装载守护 | 新增一条用例：`<archi_models_dir>` 递归全部 `*.md` 经 `parse` + `build_schema` + `verify` 全线通过（当前 10 份：后端 2 份闸生效且零表格行、前端 8 份走留白豁免） |

### Function: 契约装载期抽取面（`crates/core/archi/src/api/contract.rs`，零新增公开出口）

| 函数 | 变化 | 语义 |
|---|---|---|
| `collect_surface_table(sections: &[ContractSection]) -> Vec<String>` | 新增（私有） | 扫 `## <中文二标题>` 段正文的表格行：去前导空白后以 `|` 起手 → 按 `|` 切首列 → 去空白后要求整体被一对反引号包裹且内容是 `is_identifier` ⇒ 计入；去重保序。fence 内容不在 `section.body` 内（解析期状态机保证），列表项与裸正文零抽取 |
| `verify(contract: &Contract, schema: &Value) -> Result<(), ApiError>` | 改（签名零变化） | 追加第四条闸：前置 `ops` / `types` / `reexports` 任一非空；断言抽取集 ∩（op id ∪ `api-types` 键 ∪ `api-reexport` 键）= ∅；非空即 `ApiError::Contract`，点名文件 + 交集名字 + 出路句（名字归 fence、散文只留语义）。既有各项（三闸 + 闸一 + 落点闸 + 自洽闸）逐字不变 |

### API: 对外面（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `api::load_contracts` / `api::enabled` / `api::check` / `api::bootstrap` / `api::render_all` / `api::api_ref` | 零改动 | 签名与语义逐字不变；第四条闸经既有 `contract::verify` 调用点自动生效（`load_contracts` 的装载序零改动） |
| `Contract` / `ApiSnapshot` / `ArchiApiFinding` / `ArchiDriftReport` | 零改动 | 零字段新增（抽取在 `verify` 期读既有 `sections`）；零新 finding kind；`REPORT_VERSION = 3` 不变 |
| IPC 命令面 / DTO / `src/types/generated/**` | 零改动 | 无新字段 ⇒ `bindings:export` 幂等、`client:check` 零 diff |

### CLI: 生成链（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `crates/tools/archi-api-gen`（`generate` / `--check` / `bootstrap`） | 零改动 | 闸只作用于装载期，不参与渲染 ⇒ `.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` 逐字节零 diff、`api:check` 零差异退出 |
| `packages/desktop/package.json` 脚本面 | 零改动（`version` +1 patch） | `api:export` / `api:check` / `check` 三条命令串逐字不变 |

### Component: 仓库根文档

| 项 | 变化 | 描述 |
|---|---|---|
| `AGENT.md` / `CLAUDE.md` | 改 | `## 模块 API 契约` 第 ① 条补一句「散文不复述名字清单（唯一持有者是 fence）」；要点数仍 5、其余四条逐字不动、两文件该节正文逐字相同 |
