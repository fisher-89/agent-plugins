# desktop-archi-api-check Specification

## Purpose

定义契约判据面：`ArchiApiFindingKind` 十三值封闭词汇与开放 `category`（七值含 `doc`）、按 `api` 状态分两支的十三判据首错求值顺序、判据级三值档位与元素 `gate` 解耦、报告并列槽位 `apiFindings`（入口文档承诺已成立元素的判据进 `status`，其中 `error` 档进 `passed`），以及判据的验证语料与报告数基线（第六段 `0 / 43`）。

## Requirements

### Requirement: 契约判据词汇封闭十三值与开放分类

契约判据 SHALL 归入**新平行封闭枚举** `ArchiApiFindingKind`（**十三值**，线格式 camelCase），MUST NOT 并入既有 `ArchiDriftKind`（四值）：

| 枚举值 | 线格式词 | 语义 |
|---|---|---|
| `ApiEntryMissing` | `apiEntryMissing` | **该元素没有可用入口**：无候选（未声明 `entry` 且无语言约定默认）**或**候选被判不可用（形态非法 / 落在元素 `path` 之外 / 文件不在盘上 / 与另一元素同一入口）。解析链与四条判据见 `desktop-archi-api`「生成物落点与入口查找」 |
| `ApiMissing` | `apiMissing` | **有约定无兑现**（**语义已收窄**）：契约声明的 op / 类型在代码里不存在。原「`api = declared` 而契约文件缺失」形态**已改名** `docMissing`（同一事实不占两个名字） |
| `ApiUndeclared` | `apiUndeclared` | 代码有 `external` 面但契约未声明（**唯一完备性判据**） |
| `ApiSignatureMismatch` | `apiSignatureMismatch` | 契约签名与代码签名（归一后）不一致 |
| `ApiUnexpected` | `apiUnexpected` | 两事实矛盾。两形态：① `api = none` 而派生契约路径上存在契约文件；② `api = declared` 而元素写了 `doc`（入口文档由派生落点给出，`doc` 是第二个事实） |
| `ApiShadowing` | `apiShadowing` | 父 `$defs` 与子元素 `$defs` 类型名交集非空 |
| `ApiRefMissing` | `apiRefMissing` | 跨模块 `$ref` 目标缺失 / 未 `declared` / 非直接子元素 |
| `ApiNestedCrossCrate` | `apiNestedCrossCrate` | 嵌套元素父子分属不同 crate |
| `ApiReexportDrift` | `apiReexportDrift` | markdown 散文列表与 `api-reexport` fence 键集不等 |
| `ApiConstructorDeclared` | `apiConstructorDeclared` | 契约 `### 操作` 声明了构造器（应由生成器产出） |
| `DocMissing` | `docMissing` | **入口文档未就位**（统一两种成因）：`api = declared` 而派生契约 md 不在盘 / `api = none` 而 `doc` 未声明 |
| `DocNotFound` | `docNotFound` | **声明指不到**（仅声明类有此形态）：`api = none` 元素的 `doc` 指向不存在的文件。**名字复用而非能力延续**——退役的 `WarningKind::DocNotFound` 读 `docs` 数组、只报不拦；本值读 `doc` 单值、`error` 档、纳管 `status` |
| `ApiDocMissing` | `apiDocMissing` | **有文档无约定**：`api = declared` 的派生契约 md 在盘，但正文无 api fence 语言标（契约尚未展开） |

- **`apiEntryMissing` 的四种不可用理由 SHALL 挤进同一个值**：入口的形态 / 落位 / 存在 / 跨元素唯一四条判据的违反一律报本值，文案 SHALL 点名具体理由（越界目标 / 缺失文件 / 冲突方 FQN / 形态非法），MUST NOT 只写通用「无入口」。**MUST NOT 新增第 14 个值**（如 `apiEntryInvalid`）：四条判据与本值同族同档同 `category = entry`，新增变体会牵动 `ALL` 长度、四面词汇对拍与金样，收益不足；「无可用入口」这一措辞本身已覆盖四种成因。
- **三值各只有一种成因**：`docMissing` = 「还没写」，`docNotFound` = 「声明写错」，`apiDocMissing` = 「承诺未展开」——同一个值 MUST NOT 承载两种缺口（这正是 `apiMissing` 拆分的理由）。
- **`.i` 生成物的存在 MUST NOT 作为入口的替代证据**（旧口径「既无生成物也无入口」的两条件判定已退役）：生成物路径是入口的函数，无入口即无生成物路径可派生。
- **`api = none` 元素退出入口判据**（本变更的语义变化，显式留痕）：入口解析的消费者是「契约面的代码取数」（`element_symbols` 需 `entry_state = Resolved`），而 `api = none` 元素的契约面整体不参与；故 `api = none` 的求值链只有 `apiUnexpected` / `docMissing` / `docNotFound` 三步，**MUST NOT** 再报 `apiEntryMissing`。原「`api = none` 不豁免本值」一句随之作废。
- **分立的理由**：`ArchiDriftKind` 是「模型 vs 代码」的漂移，本枚举是「契约 vs 代码」的漂移——事实源不同（relation 图 vs 契约文件）、定级语义不同（元素 `gate` vs 判据自带 `severity()`）、扩展节奏不同。并入会让既有 `match` 臂被迫处理不相关变体，并牵动既有 corpus 金样与前端 `driftMarks`。
- **`ALL` 与 `as_str` / `severity` 三方对拍**：`ArchiApiFindingKind::ALL: [Self; 13]` SHALL 与 `as_str` 逐值对拍、与 `severity()` 的取值面逐值对拍；新增变体未补入 `ALL`（长度断言）即测试红。
- **`category` SHALL 为开放值集字符串**（MUST NOT 设枚举、MUST NOT 设 `ALL`）：用于报告渲染分组；新增分类只改打标处，不动枚举、不动 `ALL`、不动金样词汇对拍。初始取值集 SHALL 为 **七值**——`entry`（`apiEntryMissing` / `apiUnexpected`）/ `contract`（`apiMissing` / `apiUndeclared`）/ `signature`（`apiSignatureMismatch`）/ `reexport`（`apiReexportDrift` / `apiShadowing` / `apiRefMissing`）/ `nesting`（`apiNestedCrossCrate`）/ `constructor`（`apiConstructorDeclared`）/ **`doc`（`docMissing` / `docNotFound` / `apiDocMissing`）**；拼错的防护靠「单点打标 + 契约测试覆盖已知分类」，MUST NOT 靠类型系统。
- **条目形状**：`ArchiApiFinding { kind, category, element: Option<String>, target: Option<String>, message: String, occurrences: Vec<ArchiDriftOccurrence> }`——`element` 对应既有条目的「引入方」语义、`target` 对应「被引入方」语义，`occurrences` SHALL 复用既有 `ArchiDriftOccurrence` 形状（不另立第二份命中点类型）。

#### Scenario: 十三值词汇三方对拍

- **WHEN** 遍历 `ArchiApiFindingKind::ALL`、`as_str` 与 `severity()`
- **THEN** 十三项一一对应、线格式词唯一且与语料词一致；从 `ALL` 删掉一项、新增变体未补 `ALL`、或 `severity()` 漏一个变体时测试红；入口四条判据的违反不新增第 14 值（`ALL` 长度仍为 13）

#### Scenario: category 七值含 doc

- **WHEN** 遍历十三个判据值并取 `category_of`
- **THEN** 三个新值（`docMissing` / `docNotFound` / `apiDocMissing`）一律落 `doc`；`apiUnexpected` 仍落 `entry`；七值为 `entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor` / `doc`，无第八值、无空串

#### Scenario: 与既有漂移词汇互不混用

- **WHEN** 审查 `ArchiApiFindingKind` 与 `ArchiDriftKind` / `ArchiCheckStatus` / `ArchiObservationKind` 的定义与 `match` 使用点
- **THEN** 四组词汇各自单点定义、互不复用；`ArchiDriftKind` 的 `ALL` 仍为四值、`match` 臂零改动；`WarningKind`（旧文档警告单值）已随 `docs` 三源退役而删除，MUST NOT 以「词汇组」身份出现

#### Scenario: 名字复用不等于能力延续

- **WHEN** 对比退役的 `WarningKind::DocNotFound` 与新的 `ArchiApiFindingKind::DocNotFound`
- **THEN** 前者读 `docs` 数组、只报不拦、不进 `status`（已删除）；后者读 `doc` 单值、`error` 档、纳管 `status`、参与 `passed`——spec 与代码注释 SHALL 写明「名字复用」，MUST NOT 写成「语义未变」

#### Scenario: 新分类不改枚举

- **WHEN** 给某值换一个 `category` 值（如把 `docNotFound` 归 `entry`）
- **THEN** 只改打标处，枚举、`ALL`、词汇对拍与金样零改动；渲染层按新分组聚合

#### Scenario: api = none 元素不再报入口缺失

- **WHEN** 构造一个 `api = none` 且无可用入口（无 `entry`、目录型根无 `index.ts(x)`）的元素
- **THEN** 该元素零 `apiEntryMissing`；若其 `doc` 未声明则仅一条 `docMissing`；若 `doc` 指向不存在文件则仅一条 `docNotFound`

#### Scenario: 四种入口不可用理由各报一条且指名理由

- **WHEN** 分别构造入口形态非法、落在元素 `path` 之外、文件不在盘上、与另一元素同一入口四个 `api = declared` 元素
- **THEN** 四者各产一条 `apiEntryMissing`（`category = entry`、`warn`），文案分别点名「形态非法」「越界目标」「缺失文件」「冲突方 FQN」

### Requirement: 十三判据的定义与求值互斥顺序

检查 SHALL 对每个 **`path` 非空**的元素求值，求值顺序 SHALL 固定、**按 `api` 状态分成两条互斥支链**，每支内部**首错短路**（一个元素锚点命中第一条即 `break`，不再求后续判据）：

```text
api = none 支（3 步；不依赖入口）
  ① apiUnexpected   派生契约路径在盘 ⇒ 留白被违反（与 doc 无关，必须先判）
  ② docMissing      `doc` 未声明（无入口文档）
  ③ docNotFound     `doc` 声明在案但指不到盘

api = declared 支（6 步）
  ④ docMissing      派生契约 md 不在盘（原 apiMissing 形态①，改名不改事实）
  ⑤ apiDocMissing   契约 md 在盘但正文无 api fence 语言标
  ⑥ apiUnexpected   元素写了 `doc`（入口文档已由派生落点给出 ⇒ 两事实矛盾）
  ⑦ apiEntryMissing entry_state = Absent / Invalid（code 面的结构前提）
  ⑧ apiMissing      契约声明 op / `$defs` 类型而代码导出集里没有
  ⑨ 一致性判据      apiSignatureMismatch / apiReexportDrift / apiConstructorDeclared

pair 锚点（不进元素循环，与元素锚点并列、互不挤占）
  ⓐ apiNestedCrossCrate → ⓑ apiRefMissing → ⓒ apiShadowing
```

- **分界不是排版而是依赖**：①–⑥ 只读模型 + 盘（`element.doc` 存活性、`contract_path` 存活性、fence 有无），**零导出集依赖**；⑦ 起经 `element_symbols` 需 `entry_state = Resolved`——入口非 `Resolved` 时导出集为空，`apiMissing` / `apiUndeclared` / `apiSignatureMismatch` 全部无对象可判，故 ⑦ 短路 code 面不是抢名额，是 code 面无对象。
- **两支互斥，跨支不比较**（同一元素只走一支）；支内序号即求值序，**顺序写死在 `check.rs` 函数体**，MUST NOT 由配置驱动（`drift/config.rs` 保持 `ignore` / `exclude` 两键封闭 + `deny_unknown_fields`，MUST NOT 引入配置键、MUST NOT 留空壳键或 TODO）。
- **`api = none` 支把 `apiUnexpected` 排第一的理由**：该判据与 `doc` 无关（判的是「派生路径上冒出了契约」），而 `docMissing` 是 `warn` 且不纳管——若让 `docMissing` 先短路，一条 `api = none` 的元素只要没写 README，就能把它那份放错位置的契约遮住。
- **首错短路的合法性依据**：①–⑥ 命中后，⑦–⑨ 的对象（`fence` / fence 声明的 op）尚不存在；故短路的不是真问题而是**尚不存在的对象**。
- **互斥的理由**：`apiEntryMissing` 比 `apiMissing` 更基础——**在契约生成之前就失效**。若两条判据对同一元素同时报，报告数无法单调对拍（消一条减一），且反馈会把 executor 引向「去写代码实现一个不存在的模块入口」。
- **⑦ 的判定 SHALL 单点取自 `paths::entry_state`**：`Resolved` ⇒ 继续 ⑧⑨；`Absent`（无候选 / 约定默认全不落盘）与 `Invalid(<problem>)`（形态 / 落位 / 存在 / 唯一任一违反）⇒ 一条 `apiEntryMissing`。判据面 MUST NOT 自行判文件存在性、MUST NOT 保留第二份入口探测（旧 `has_generated` 双条件判定已随落点变更退役）。
- **pair 锚点的形状**：三条跨元素判据 SHALL 产出 `(element, target)` 对，稳定排序键 `(kind, element, target)` 与之逐字一致。pair 锚点 MUST NOT 引用 `Model.relations`——父子在模型里没有边（层级由 FQN 最长前缀推导，`[[relation]]` 只承载依赖），`$ref` 目标可能根本不在模型内（写成 relation 会被 `graph.rs` 的 `unknownReference` / `dependsOnAncestor` 先判结构错）。pair 内序的理由：nested 是结构前提（跨包不可表达）→ ref 是目标可达性（够不够得着）→ shadowing 是命名冲突（够着了但重名）。
- **`apiMissing` 的声明集 / 导出集口径**：声明集 = 契约 `### 操作` 的 id 集（**构造器双向都不参与比对**——既不在契约声明集，也不计入导出集）；导出集 = 元素的 `external` 代码导出（Rust 经 `pub mod` 链或 `pub use` 白名单可达的 `pub fn`；TS 经**入口导出闭包**可达的导出——入口自身声明 + 沿 `export … from` 递归并入且目标归属在本元素 FQN 子树内的导出，定义见 `desktop-archi-api`「TS 导出闭包的取数面」，本能力 MUST NOT 另立第二份 TS 导出集口径）。签名比对 SHALL 在**归一后**进行（AST 归一单行、参数名与空白不参与）。
- **`apiUndeclared` 只读 `<FQN>.schema.json` 的 `$defs` 键集**，MUST NOT 并读 markdown；其**声明集前提**依赖 fence 存在（未过 ④ ⑤ 的元素没有声明集可对拍，故档位虽 `off`、语义上也无对象）。安全性由前序门禁保证：`$defs` 键集 = `api-types` ∪ `api-reexport`，而 `apiReexportDrift` 保证 fence 与散文一致、零 diff 门保证 schema 零手改自由度。
- **`apiDocMissing` 的判定口径必须廉价**：判据只需要**一个布尔**（契约正文有无 api fence），故 MUST NOT 接 `contract::parse`（那会连带 `schema::build_schema` + `contract::verify` + `schema::verify_defs`，且坏契约走 `Err` 而非判据，语义错位）。事实由 **load 阶段一次性装配**进 `ApiCheckInput.doc_facts: BTreeMap<Fqn, DocFact>`（`Absent` / `Missing` / `Present { has_api_fence }`），`check` 侧保持「纯读 `input`（零读盘、零写盘、零 spawn）」边界。fence 语言标 SHALL 单点复用 `contract::FENCE_OP` / `FENCE_TYPES`（本变更提升可见性），**MUST NOT 抄第二份字面量、MUST NOT 在检查侧补写半个 markdown 解析器**。已知精度上限（可接受）：只判「有无 fence 语言标」，不判 fence 是否合法 / `### <op-id>` 是否齐 / `api-types` 是否重复——那些是 `contract::parse` 的 `Err` 面（既有诊断），不归本判据。
- **`apiUnexpected` 双形态各自单向**：① 只判 `api = none` 而派生契约路径存在；② 只判 `api = declared` 而 `doc` 被写了。反方向（`api = declared` 而契约缺失）归 `docMissing`——两条判据 MUST NOT 对同一事实双报，否则报告数无法单调对拍。
- **`apiMissing` 只剩形态②（本变更收窄）**：契约在、但其声明的 op / `$defs` 类型在代码导出集里不存在；形态①（`api = declared` 而契约文件缺失）**已改名** `docMissing`（同一事实不占两个名字，见上条 requirement 的词汇表）；共同的排除面三条 = 构造器双向排除 / 透传纯 `$ref` alias 排除 / `x-class` 与 `enum` 计入类型面；结构前提 SHALL 含 `#[path = "..."] mod` 的模块定位与 `pub use <mod>::*` 的通配升格，缺任一条则本判据在自有生成物的元素上恒假阳。
- **`apiMissing` 天然支持渐进**：契约里只写已确定的 op 即可，未写的 op 归 `apiUndeclared`（关档），不落在 `apiMissing`。
- **`apiConstructorDeclared` 是「契约自身矛盾」的判据**（合法新增判据的范围）：构造器由生成器自动产出，写进契约必然导致 `apiMissing` 误报——试点纸面验证已实测该误报。
- **`apiRefMissing` 的悬空引用在 schema 层由外部校验器编译期捕获**（ajv 对 schema 层的 `$ref` 报 `can't resolve reference`），但 `x-function` 内的 `$ref` **不被解析**（`x-function` 是未知关键字，校验器不下沉）——故函数签名里的跨模块引用只能由本判据与生成器保证。该边界 SHALL 在信心中如实体现：`schema.json` 的定位是「类型层的可校验投影」。
- **零第二份判定**：十三条判据 SHALL 只有一处实现（`crates/core/archi/src/api/check.rs`），命令面与门禁面 MUST NOT 各写一份。
- **契约面启用条件（冷启动纪律，本变更收紧）**：`api::enabled` 的判据 SHALL 为「**存在任一 `path` 非空元素的入口文档载体在盘且正文含 api fence 语言标**」——即 `doc_facts` 中至少一个 `Present { has_api_fence: true }`，与判据面同源。判据 MUST NOT 退化为「`<archi_models_dir>` 递归任意 `*.md`」：`models/` 里放一份笔记 / 一份非载体 md MUST NOT 误判为启用（E-1 的 `enabled` 判据形态在此收紧，属行为变化）。启用后**全部 `path` 非空元素**（本仓 44 个）纳入判据，不存在「部分元素豁免」的中间态，也不设白名单或开关。

#### Scenario: declared 支首错短路

- **WHEN** 某 `api = declared` 元素的派生契约 md 不在盘
- **THEN** 只报一条 `docMissing`；同元素不出现 `apiDocMissing` / `apiUnexpected` / `apiEntryMissing` / `apiMissing` / `apiSignatureMismatch`（对象不存在，短路的不是真问题）

#### Scenario: none 支三步与 apiUnexpected 前置

- **WHEN** 构造三个 `api = none` 元素：一为派生契约路径有契约文件且 `doc` 未声明；一为派生路径干净且 `doc` 未声明；一为派生路径干净、`doc` 指向不存在文件
- **THEN** 三者分别恰报 `apiUnexpected`（且 MUST NOT 同时报 `docMissing`） / `docMissing` / `docNotFound`；三者均零 `apiEntryMissing`

#### Scenario: 有文档无约定是诚实的中间态

- **WHEN** `api = declared` 元素的派生契约 md 在盘但正文无 `api-op` / `api-types` fence
- **THEN** 只报一条 `apiDocMissing`（`category = doc`、`warn`、纳管 `status`）；同一元素不再报 `apiMissing`（无声明集可对拍）；该元素 `status` 至少 `warnings`

#### Scenario: 契约声明 op 而代码无

- **WHEN** 元素契约在案且有 fence、入口存在，契约声明 `parse` 与 `all_conclusions` 两个 op，代码只有 `parse`
- **THEN** 报一条 `apiMissing`，点名 `all_conclusions`；`apiUndeclared`（`off`）不产出条目；同元素不再出第二条判据（首错）

#### Scenario: declared 写了 doc 即报矛盾

- **WHEN** 某 `api = declared`（或省略 `api`）元素的模型里写了 `doc = "./README.md"`，且其派生契约 md 在盘含 fence、入口可解析、契约与代码一致
- **THEN** 只报一条 `apiUnexpected`（文案指明「入口文档由派生落点给出，`doc` 是第二个事实」）；该值 MUST NOT 参与任何其它判定

#### Scenario: 签名归一后比对

- **WHEN** 契约声明 `parse_cargo_test(input: string) -> SummaryReport`，代码把签名排成多行且参数名不同但类型序列相同
- **THEN** 归一后判定一致、零 `apiSignatureMismatch`；把返回类型改为 `Vec<SummaryReport>` 时报一条 `apiSignatureMismatch` 并给出两侧归一签名

#### Scenario: 构造器声明即契约自身矛盾

- **WHEN** 契约声明 `### new` 而代码的 `new()` 由生成器产出
- **THEN** 报一条 `apiConstructorDeclared`，**不**同时报 `apiMissing`（构造器不进声明集比对）

#### Scenario: pair 锚点不引用 relations

- **WHEN** 构造「父 `$defs` 与子元素 `$defs` 类型名交集非空」且父子在模型里没有 `[[relation]]` 边
- **THEN** 仍报一条 `apiShadowing`（`element` = 父、`target` = 子）；模型零结构诊断（MUST NOT 因判据要求而出现边）

#### Scenario: 启用判据与载体同源

- **WHEN** 观察根下 `graph/model/` 一级有六个 `NN-<域>.toml`，另有 `models/notes.md`（非任何元素的载体）与 `Backend/notes.md`（无 fence）
- **THEN** `enabled = false` 且 `apiFindings = []`；随后在某元素派生契约路径放入一份带 `api-op` fence 的 md ⇒ `enabled = true`、全部 `path` 非空元素纳入判据。两例共用同一判定函数（`doc_facts` 上的 `has_api_fence`），MUST NOT 在命令层另判一套

#### Scenario: entry 优先互斥

- **WHEN** 某 `api = declared` 元素既无可用入口（`Absent`，或候选被判 `Invalid`）、契约在案且声明了 2 个 op
- **THEN** 只报一条 `apiEntryMissing`（declared 支的 ⑦）；同元素不出现 `apiMissing` / `apiUndeclared` / `apiSignatureMismatch`（⑧⑨ 无对象可判）；契约文件缺失形态归 `docMissing`（④，在 ⑦ 之前）；`apiUnexpected` 仍可独立报

#### Scenario: TS 导出集取闭包且判据语义不变

- **WHEN** 某 TS 元素的入口是纯 re-export barrel（7 个导出名全部经 `export … from` 从本元素子树内的文件汇入），契约声明了其中 2 个 op 与 2 个类型
- **THEN** ⑧ `apiMissing` 对这 4 个声明零命中（导出集含闭包内符号），`apiUndeclared` 仍 `off` ⇒ 闭包扩容不产出任何新条目，⑨ 仍逐符号比对归一签名；判据种类（十三值）、两支首错求值序、判据级档位与报告形状逐字不变——**导出集变宽只可能减少 `apiMissing`，MUST NOT 出现「闭包扩张导致新增 finding」的路径**

#### Scenario: 留白与缺文件各归一条判据

- **WHEN** 分别构造「`api = none` 但派生契约路径存在契约文件」与「`api = declared` 但派生契约 md 缺失」
- **THEN** 前者报 `apiUnexpected`（文案指明多出的契约文件路径）；后者报 `docMissing`（文案指明缺失的契约文件路径）；同一元素 MUST NOT 同时出两条

#### Scenario: 生成物在盘不豁免入口缺失

- **WHEN** 某 `api = declared` 元素无可用入口（未声明 `entry`、根为目录型且无 `index.ts` / `index.tsx`），而盘上仍残留一份旧位 `.i.ts`
- **THEN** 仍报一条 `apiEntryMissing`（`.i` 的存在不再是入口的替代证据）；该残留文件同时由 `api:check` 的 `orphaned` 列报出

### Requirement: 判据级档位与元素 gate 解耦

十三条判据的档位 SHALL 由判据自身承载（`ArchiApiFindingKind::severity()`，**判据级档位**，**封闭三值** `off` / `warn` / `error`），MUST NOT 读元素 `gate`：

| 类别 | 判据 | 档位 |
|---|---|---|
| **入口文档未给出**（warn，不纳管） | `docMissing` | `warn` |
| **声明指不到**（纯笔误，无中间态可辩） | `docNotFound` | **`error`** |
| **有文档无约定**（补契约的合法中间态） | `apiDocMissing` | `warn` |
| **有约定无兑现 / 实现漂移** | `apiMissing` / `apiSignatureMismatch` | **`error`** |
| **判不了 / 两事实矛盾 / 结构债** | `apiEntryMissing` / `apiUnexpected` / `apiShadowing` / `apiRefMissing` / `apiNestedCrossCrate` / `apiReexportDrift` / `apiConstructorDeclared` | `warn` |
| **完备性判据**（代码有的契约都得有） | `apiUndeclared` | **`off`** |

- **分界线写死**：一致性判据报的是「债」（可枚举、可消除，可即开 `warn` 验证检查器是否工作）；完备性判据报的是「海」（迁移期必然爆炸，必须后置）。`apiUndeclared` 是唯一后者。
- **分档动机是接入成本**：初始只补部分模块文档（doc 面 `warn` 可渐进），补了文档之后代码漂移应尽快修（api 面 `error`）；`docNotFound` 因为「声明指不到」没有任何可辩解的中间态，故直接 `error`（放任它就是「把 `doc` 指向不存在文件」的消红后门）。
- **结构性安全网（不靠白名单）**：`error` 档只可能落在「入口文档已就位」的元素上（`apiDocMissing` / `apiMissing` / `apiSignatureMismatch` 都需要契约在盘且含 fence；`docNotFound` 需要 `doc` 已声明）⇒ **存量项目没补文档就永远不会红**。
- **唯一的刻意缺口**：`apiDocMissing` 是 `warn` 且占首错名额 ⇒ 它是 `apiMissing` 的**可达性上界**——「写了 doc 但没写 fence」的元素始终到不了 `error` 面。补偿是它纳管 `status`（该元素至少 `warnings`，不会静默 `clean`），使「没写 doc」与「写了 doc 没写契约」在状态词上可区分。
- **`apiEntryMissing` MUST NOT 收紧**：它报的是「**判不了**」而不是「**判错了**」——有对象、结论为否才该红。
- **与元素 `gate` 解耦的理由**：`ArchiDriftKind` 的定级看引入方元素的 `gate`；契约判据若也接 `gate`，收紧契约就要改模型，两套语料同时动、收紧节奏无法独立。故元素 `gate` 全 `off` 与本能力判据的档位表 SHALL 同时成立——判据级档位是契约面的独立收紧轴。
- **收紧路径**：收紧由 `severity()` 单值改动驱动（一类判据一个 change 收紧），MUST NOT 引入运行时开关、MUST NOT 引入配置键。
- **`warn` 报告即迁移 backlog**：`warn` 模式下的报告输出天然成为迁移待办表（当前唯一 backlog 是 `docMissing` 的 43 条：补入口文档或判 `api = declared`——三条都是真话，MUST NOT 一律按「去建入口文档」理解），比手写 todo 更可靠（直接来自代码事实）。每消一条，报告数减一，可单调验证。
- **MUST NOT 为消债而重构**：本能力 MUST NOT 要求建入口文件、改元素根路径去扩展名、挪子→父类型依赖或上移壳元素根路径，也 MUST NOT 为消 `docMissing` 而批量新建契约 md——存量债务只作检查器的验证语料；补文档的节奏归数据面变更。唯一例外是**为让契约有效而暴露已存在的私有逻辑**（改可见性 + 挪进 `impl`，不动实现逻辑）——判据不同：前者是「格式追代码」，后者是「代码追上契约应有内容」。入口面同理：`entry` 字段与解析链只让「本来存在的入口」可被表达，本能力 MUST NOT 在判据面要求补入口。

#### Scenario: 元素 gate 全 off 而判据仍报

- **WHEN** 模型内全部元素 `gate = off`（本仓现状）且合成语料中存在若干无可用入口元素
- **THEN** 每个无可用入口元素各产一条 `apiEntryMissing`（判据级 `warn`），与元素 `gate` 无关；把某元素 `gate` 改成 `strict` 后该数量与内容零变化

#### Scenario: 完备性判据保持 off

- **WHEN** 代码有 `external` 导出而契约未声明
- **THEN** `apiUndeclared` 档位为 `off`，报告不产出该条目（`apiFindings` 内零 `apiUndeclared`）；把档位改为 `warn` 属独立裁决

#### Scenario: 三档各自的代表值

- **WHEN** 遍历 `severity()` 的取值面
- **THEN** `error` 恰为 `docNotFound` / `apiMissing` / `apiSignatureMismatch` 三项；`off` 恰为 `apiUndeclared` 一项；其余九项 `warn`——新增变体落哪一档必须显式决定（`match` 无通配兜底、或兜底处留注释）

#### Scenario: 零运行时开关

- **WHEN** 审查判据档位的来源
- **THEN** 唯一来源是 `ArchiApiFindingKind::severity()`；无配置键、无环境变量、无命令行 flag 可改档位

### Requirement: 报告并列槽位 apiFindings

`ArchiDriftReport` SHALL 保留**并列**字段 `api_findings: Vec<ArchiApiFinding>`（线格式 `apiFindings`），MUST NOT 改造既有 `errors` / `warnings` / `suppressed` / `observations` / `status`：

- **既有字段零改动**：`ArchiDriftReport` 既有 14 字段 SHALL 逐字不变（字段名 / 类型 / 序列化形态 / 语义）；`REPORT_VERSION` **保持 3**（字段面无增删——本变更只换判据取值、档位与 `status` 推导口径，不足以为版本递增提供理由）。
- **纳管子集口径（本变更改写）**：`apiFindings` 对 `status` 的影响 SHALL 限于**纳管子集**，条目入选判据 SHALL 为——

  ```text
  纳管(entry) ⟺ entry.element 非空 ∧ (
        api = declared : 派生契约 md 在盘
      ∨ api = none     : `doc` 字段已声明          ← 声明本身即承诺（docNotFound 判的正是它）
  )
  ```

  纳管子集非空 ⇒ `status` 至少为 `warnings`，其中任一 `Error` 档条目 ⇒ `status = errors`。`docMissing` 在两侧都不成立（它是「承诺尚未给出」）⇒ **永不纳管**（否则 43 条读数会把 `status` 常规化为永久 `warnings`，状态词失去区分度）；`element = None` 的条目落选。
  - **口径理由（承诺即纳管）**：没写入口文档的元素，其 `docMissing` 是迁移 backlog（既有 requirement 明写「MUST NOT 为消债而重构」）；承诺已给出的元素，判据就是「自己刚承诺的东西没兑现」——只有后者该进状态词。
  - **为什么 none 支取「已声明」而 not「在盘」**：`docNotFound` 的成因恰是「声明在案、文件不在盘」——按「在盘」定义它永远落选，与本能力「`docNotFound` 是 `error` 且纳管」的档位表互斥。取「已声明」使档位表与纳管表同时成立。
  - **判据单点**：纳管子集筛选 SHALL 在 `crates/core/archi` 的单点函数（`api::gating_findings`）内完成，「载体在盘 / `doc` 已声明」的判定在该函数内一次性发生；`drift` 侧 MUST NOT 自行判文件存在性、MUST NOT 在前端或命令层二次筛选。
- **`passed` 判据（本变更改写，唯一跨进门禁面处）**：

  ```text
  passed = errors 空 ∧ model.structuralErrors 空 ∧ error 档 apiFindings 空
  ```

  `error` 档判据（`docNotFound` / `apiMissing` / `apiSignatureMismatch`）从此能阻断归档。这是既有 spec 自己规定的同批动作（「把某类判据改 `error` 档与改 `passed` 判据 SHALL 同批进行」）；其反向纪律「`apiFindings` MUST NOT 影响 `passed`」**随之作废**，MUST NOT 在任何 spec / 注释里残留。
  - **不变量**：`status == errors ⟺ passed == false` SHALL 恒成立——`error` 档判据的触发条件都蕴含纳管成立（见上），故不存在「打断了 `passed` 却不进 `status`」的旁路。
- **`derive_status` 分档**：`warnings` 分支条件扩为「`warnings[]` 非空 **或** 纳管子集非空（且其中无 `Error`）」，新增 `errors` 分支「纳管子集含任一 `Error`」；`skipped` / `errors`（结构错 / `errors[]`）/ `noChanges` 的先后序与语义逐字不变。
- **聚合与稳定序**：`apiFindings` SHALL 按 `(kind, element, target)` 稳定排序（同 `kind` 内按元素 FQN 字典序），使报告可逐字 diff；命中点复用 `occurrences` 聚合（同元素对多处命中聚在一条内，不按行铺开）。
- **词汇组表述校准**：报告自带封闭词汇由四组（`status` / 漂移条目 `kind` / 观察项 `kind` / 契约判据 `kind`）维持四组，但 `ArchiApiFindingKind` 由十值扩**十三值**；`WarningKind`（旧文档警告单值）已删除，MUST NOT 再出现在「互不混用」的对照清单里。
- **判定单源**：`api_findings` 的装配 SHALL 在 `crates/core/archi/src/drift/mod.rs` 的 `check()` 内单点完成（调 `api::check`）；命令面（`archi_drift`）与门禁面（`LocalArchiCheck`）产出 SHALL 逐字段相等（除 `timestamp` / `change` 与落盘路径）。
- **呈现面**：`apiFindings` SHALL 经架构页左栏「契约」视图与元素信息面板「契约」段呈现（`desktop-archi-contract-panel`），两处 MUST NOT 重算判据、MUST NOT 对同一报告二次 invoke。

#### Scenario: 存量 backlog 不拉黄灯

- **WHEN** 报告含 **43** 条 `docMissing`（本仓第六段读数：41 条改名自 `apiMissing` 形态① + 2 条 `api = none` 未声明），`errors` 为空且无结构错
- **THEN** 纳管子集为空 ⇒ `status = clean`、`passed = true`；MUST NOT 因这 43 条把状态词常规化为 `warnings`

#### Scenario: 入口文档在案的 error 判据进 status 与 passed

- **WHEN** 报告含 1 条 `apiMissing`（该元素契约 md 在案且有 fence）、`errors` 为空且无结构错
- **THEN** `status = errors`、门禁步 `passed = false`（两条同批成立，不变量保持）；把该元素的契约 md 移走后同一场景退化为 `docMissing` ⇒ `status = clean`、`passed = true`

#### Scenario: docNotFound 纳管而上游 docMissing 不纳管

- **WHEN** 分别构造「`api = none` 且 `doc` 指向不存在文件」与「`api = none` 且 `doc` 未声明」
- **THEN** 前者 `status = errors`、`passed = false`（`doc` 已声明 ⇒ 纳管，且 `docNotFound` 是 `error`）；后者 `status = clean`、`passed = true`（`docMissing` 不纳管、且是 `warn`）

#### Scenario: 既有字段与金样零改动（除首错改写）

- **WHEN** 对既有漂移语料跑检查
- **THEN** `errors` / `warnings` / `suppressed` / `observations` / `model` / `scan` 与 `"version": 3` 逐字不变；十六份金样的变化只允许来自「首错改写使同元素多报降为一条」与「纳管分档」，逐份变化 SHALL 在 change 留痕中说明成因

#### Scenario: 稳定序与聚合

- **WHEN** 同一元素对的三处签名不一致被三处命中
- **THEN** `apiFindings` 内该元素对恰一条，`occurrences` 含 3 项（各带 `file` / `line`）；整体排序为 `(kind, element, target)` 稳定序，两次运行逐字相同

#### Scenario: 命令面与门禁面同源

- **WHEN** 以同一 root 分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告的 `apiFindings` / `status` / `passed` 逐字段相等（含排序、`category` 与纳管结论）

### Requirement: 契约判据的验证语料与报告数基线

契约判据的正确性 SHALL 以**报告数逐项对拍**验证（存量债务即验证语料），并 SHALL 有语料守护：

- **基线表（本仓当前读数；分母 = `path` 非空的 46 个元素）**：`apiEntryMissing` **0**、`apiMissing` **0**（形态① 已改名 `docMissing`，只剩形态②，本仓零契约实现漂移）、**`docMissing` 36**（33 = `api = declared` 且派生契约 md 不在盘；3 = `api = none` 且 `doc` 未声明）、其余十值各 **0**、`apiUndeclared` 不适用（`off`）；`status = clean`、`passed = true`（36 条全 `warn` 且 `docMissing` 不纳管）。
- **本段的两项性质必须与原话一起写**：这不是「新增 36 条债」，而是——
  1. **改名**（更早一段）：`apiMissing` 的原形态①（契约 md 未写）改名为 `docMissing`，事实同一；
  2. **参与面 +3**：三个 `api = none` 元素进入判据面（过去契约面把它们整体排除，现在它们必须给出「本模块无 API + 入口文档在哪」）——两个工具 crate 与**本变更新增的生成物中继元素** `App.Frontend.GeneratedBindings`（见 `repo-archi-model`）；
  3. **分母与读数随仓演进**（本变更同批校正，见下条）。
- **既有读数的陈旧必须留痕（不得假装本次只 +1）**：本要求此前的「分母 44 / `docMissing` 43」是**两次变更之前**的读数——其间 ① 新增了一个 `path` 非空的共享组件元素（分母 44 → **45**，`docMissing` 41 → 42），② 9 份新契约 md 落地（`Foundation` + 8 份 Agent 组件，连同既有试点 `ArchiRuntime` 共 **10** 份契约在盘 ⇒ declared 侧读数 42 → **33**），故本变更前的真实读数是 **45 / 35（33 declared + 2 none）**。本变更再 +1 个 `api = none` 且无 `doc` 的元素 ⇒ **46 / 36（33 declared + 3 none）**。MUST NOT 把「43 → 36」写成「债务下降 7 条」：那是**分母 +1、契约 +10（既有）、参与面 +1（本次）**三种变化叠加后的净值，逐 `kind` 必须按本条口径解释。
- **两条分桶口径**：① `apiEntryMissing` 只由「无可用入口」产生，`api = none` 元素退出该判据 ⇒ 本仓该桶为 0；② 入口可解析的 `api = declared` 元素承担「有门无入口文档」的 `docMissing`（形态①），而 `api = none` 元素在 `doc` 未声明时同样落 `docMissing`——故 36 = 33（declared）+ 3（none），`apiMissing` 只剩「契约声明了而代码没有」一桶且本仓为 0。
- **历史链（七段，逐段成因在案）**：`28 + 13 = 41`（入口靠探测）→ `15 + 26 = 41`（入口判据落地）→ `12 + 27 = 39`（数据面换段）→ `12 + 29 = 41`（共享组件补建元素）→ `0 + 41 = 41`（12 个前端目录根补入口，整桶平移）→ **`0 + 43 = 43`（形态①改名 + `api = none` 两元素入面，当时分母 44）** → **`0 + 35 = 35`（分母 +1 的组件元素 + 10 份契约在盘，spec 未同批随动）** → **`0 + 36 = 36`（本变更：分母 46、`api = none` 三元素入面）**。末段 MUST NOT 被写成「入口债已清」或「契约债清零」。
- **判定口径**：**报告数与基线表逐项相等**即「检查能正确提示」。多于基线 = 假阳，少于基线 = 假阴，两者都是检查器缺陷——MUST NOT 以「报告少就是代码干净」自我解释；本段总数与分母都变过，故必须**逐 `kind` 对**，不能只看总数。
- **末行归零的前提（必须说准）**：「全 0」的前提是**按现有实现写契约**——契约里只声明已有的 op / 类型，未写的归 `apiUndeclared`（`off`，不产出）。若反过来「先写契约再补实现」，`apiMissing`（`error`）会立刻红——这正是 `error` 档的用途。
- **基线须显式留痕**：基线数字写入 change 报告（`reports/`）与实现期实测对照；数字变更 SHALL 在报告内说明成因，MUST NOT 直接在代码里改期望数。基线表本身**不写进断言**（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」。
- **语料守护（分两层，各有 owner，本变更**不新增文件**）**：① **判定面逐判据用例**由 `crates/core/archi/src/api/**` 的 **13 份** sidecar 用例承担——十三值各自正反例、两支互斥与首错、pair 锚点三条、`doc_facts` 三态、签名归一、嵌套与透传、生成物渲染两侧键集一致；三值新判据的用例 MUST 落在既有 sidecar（如 `check_test.rs` / `mod_test.rs`）内，**MUST NOT 新增 `api/**` 文件**——`golden_test.rs` 断言「`api/**` sidecar 恰 13 份」，新增文件即撞红。② **crate 级报告面语料**沿既有 `crates/core/archi/tests/corpus/drift/` + `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面驱动，快照为 `expected/<case>.<mode>.json`；既有 `api-mismatch` case SHALL 保持非空 `apiFindings`；新增 case 未登记进 `DRIFT_CASES` 即红、登记了但 case 目录不存在亦红。golden 更新 SHALL 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认（快照不参与判定）。
- **首错改写会改动金样读数**：一个元素命中第一条即 `break` ⇒ 「同元素多报」的 case（如 `apiMissing` + `apiReexportDrift`、`apiMissing` + `apiConstructorDeclared`）条目数下降乃**预期**；每条变化 SHALL 在 change 留痕中说明「哪一条因首错被短路」。代码注释里「与 ① 独立、MUST NOT 与 ② 双报」一类纪律句 MUST 同批改写，否则后人会按旧口径写回归用例。
- **语料覆盖的已知缺口（就地留痕）**：三条新判据**不新增 sidecar 文件**（撞上 13 份计数断言），其正反例落在既有文件内；因此「十三判据各自正反例」一句 MUST NOT 被读成「十三判据各有独立语料文件」。兜底只剩两层——crate 级 `corpus/drift/` 金样（不经本仓模型）与本仓实测留痕。
- **MUST NOT 把本仓模型与契约塞进 crate 语料**：本仓 `graph/**`（契约索引 + 模型声明）与 `models/**`（契约资产）是仓库数据资产，纳入 crate 测试语料会破坏「语料自足、可移植」；本仓核对以命令 + 留痕执行（落 change `reports/`）。`reports/archi/` 是**门禁运行时产物**——不入库、不计入交付面、不作判红面（`git status` 出现 `??` 属预期）。
- **生成器正确性由自身单测守护**：`verify`（零 diff + 编译器）覆盖不到生成器自身的 bug——渲染、装配、抽取三面各有逐字段断言。

#### Scenario: 契约面未启用时零判据

- **WHEN** 观察根下 `graph/model/` 存在且一级为六个 `NN-<域>.toml`，但没有任何 `path` 非空元素的载体在盘带 fence 时跑一次全量检查
- **THEN** `apiFindings = []`（零条任何契约判据），`errors` / `warnings` / `status` 与不加契约面时逐字相同；MUST NOT 产 `docMissing` / `apiEntryMissing`

#### Scenario: 试点契约落地后全量纳入

- **WHEN** 同一根下加入试点契约（`<archi_models_dir>/Infra/App.Backend.Infra.ArchiRuntime.md`，含 `api-op` / `api-types` fence）后重跑
- **THEN** 契约面转为启用（载体带 fence），全部 `path` 非空元素（46 个）参与判据：产出基线表所述 **0** 条 `apiEntryMissing` 与 **36** 条 `docMissing`（试点元素自身零命中）；试点元素不因「有契约」而被豁免于其它判据

#### Scenario: 报告数与基线表逐项相等

- **WHEN** 以本仓根跑一次全量检查并统计 `apiFindings` 按 `kind` 的分布
- **THEN** `docMissing = 36`（33 条为 `declared` 且派生契约 md 缺失、3 条为 `api = none` 且 `doc` 未声明——其中一条是本变更新增的生成物中继元素）、`apiEntryMissing = 0`、`apiMissing = 0`、其余十值各 0；10 份契约在盘的元素（`Foundation` / `ArchiRuntime` / 8 份 Agent 组件）零判据；`status = clean`、`passed = true`

#### Scenario: 陈旧读数与当前读数不得并排误读

- **WHEN** 审查 spec 与 change 报告内关于历史段 / 当前段的表述
- **THEN** 当前段 SHALL 写明「36 = 33（declared 契约未写）+ 3（`api = none` 未声明）、分母 46」，并把「此前写的 44 / 43 已陈旧」的成因（分母 +1 的组件元素 + 10 份契约在盘）单列一行；不出现「债务由 43 降到 36」一类表述；`apiMissing` 的归零 SHALL 被解释为「形态①移出」而非「实现漂移清零」

#### Scenario: 假阳与假阴都可辨

- **WHEN** 人为让某「派生契约 md 缺失」的 `declared` 元素漏报 `docMissing`（假阴），或让某契约在盘且含 fence 的元素也被判 `docMissing`（假阳）
- **THEN** 前者使 `docMissing` 由 36 变 35（少于基线）；后者使该桶变 37（多于基线）；两种偏离都判为检查器缺陷，MUST NOT 更新基线表放行

#### Scenario: 新元素进入判据面只增一条且不改其它桶

- **WHEN** 在模型中新增一个 `path` 非空、`api = none` 且未写 `doc` 的元素（本变更的生成物中继元素即此类）后重跑
- **THEN** 分母 45 → 46、`docMissing` 35 → 36，其余十二值仍 0；该元素零 `apiEntryMissing`（`api = none` 退出入口判据，尽管它 `entry_state = Absent`）；`status = clean`、`passed = true` 逐字不变

#### Scenario: sidecar 计数不变

- **WHEN** 运行 `golden_test.rs` 的「零第二套判据语料目录」用例
- **THEN** `crates/core/archi/src/api/**` 的 sidecar 仍恰 **13** 份（三值新判据的用例全在既有文件内），且 `tests/corpus/api/` 仍零存在

#### Scenario: 判定面用例与报告面语料各司其职

- **WHEN** 运行 `crates/core/archi/src/api/**` 的 sidecar 用例与 `crates/core/archi/tests/golden_test.rs` 的 `DRIFT_CASES` 驱动面
- **THEN** 前者逐判据覆盖正反例（判据产出由 `check::check` 直驱断言，不依赖快照）；后者逐 case 驱动 `scan → drift::check → 报告`，每 case 的 `apiFindings` 与 `expected/<case>.<mode>.json` 逐字段一致（含 `kind` / `category` / `element` / `target` / `occurrences` 计数）；`api-mismatch` case 产非空 `apiFindings`；MUST NOT 存在第三套判定语料目录

#### Scenario: 新 case 未登记即红

- **WHEN** 新增 `corpus/drift/<case>/` 却未登记进 `DRIFT_CASES`，或登记了而 case 目录不存在，或旧金样未重写而缺 `apiFindings` 键
- **THEN** 三种情形各报红并点名该 case（缺失键 MUST NOT 静默补默认值）；golden 变更只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认

### Requirement: 装载期名字清单闸与判据面的分工

名字清单的「散文表格复述」形态 SHALL 由**装载期闸**判定，MUST NOT 进入本能力的判据面；该闸 SHALL 让**判据面**零改动（`api/**` 的求值面——`check.rs` 的输入装配与求值序、`severity()` 三档、`gating_findings` 入选判据、纳管子集函数——逐字不变）。**零改动面的边界（本变更显式收窄）**：`drift/report.rs` 与 `drift/mod.rs` **不在**该面内——本变更在这两个文件里加的 `ArchiDriftKind::UndeclaredApi`（`ALL` 4 → 5）与两桶装配归 `desktop-archi-drift` 登记，属**漂移判定面**而非契约判据面；把整文件写成「逐字不变」即与同批 drift delta 自相矛盾。

- **分工单点**：第四条闸（散文表格名字 ∩ 契约名字集 = ∅）的定义与实现归 `desktop-archi-api`「契约资产形态与唯一手写真相」，错误面为 `ApiError::Contract`（`load_contracts` 中止）。分界理由：判据面报的是「契约 vs 代码的比较结论」（可枚举、可渐进消债、进报告数基线），而「契约文件内部两处持有同一个名字」是**契约资产自身的错**——它与代码导出集无关，也不该被算成一条债进入报告（否则一个错字会同时污染基线表与 `status`）。
- **零新增判据词汇**：`ArchiApiFindingKind` SHALL 保持**十三值**（`ALL` 长度断言不变），`category` 保持七值、`severity()` 保持三档；该闸 MUST NOT 产出 `apiFindings` 条目、MUST NOT 参与纳管子集与 `passed` 判据、MUST NOT 改动 `REPORT_VERSION`（仍 3）。
- **判据输入面 MUST NOT 含散文表格**：十三判据的输入 SHALL 仍为「模型 + 契约 fence / `<FQN>.schema.json` + 代码导出集 + `doc_facts`」；`apiUndeclared` 的声明集 SHALL 仍**只取 `<FQN>.schema.json` 的 `$defs` 键集**，MUST NOT 并读 markdown 散文、MUST NOT 新增表格行读取点（判据面 MUST NOT 补写半个 markdown 解析器——与「`apiDocMissing` 只判布尔、不接 `contract::parse`」同一条纪律）。
- **留白期零判据（显式留白，不是疏漏）**：8 个前端组件元素的契约三项（`ops` / `types` / `reexports`）当前全空，其散文「公共面」表共 **28** 条数据行（8 份契约逐份：6 / 2 / 2 / 3 / 3 / 2 / 2 / 8）、名字已存在于代码导出面而未进契约 —— 该形态在本能力**零判据**：MUST NOT 报 `apiUndeclared`（档位保持 `off`），MUST NOT 为此新增第十四值或第八个 `category`，MUST NOT 为「表里列了未声明名字」另立判据。**`apiUndeclared` MUST NOT 抬档**：留白期抬档会把临时债读成判据面债务，且与本能力「完备性判据报的是海、迁移期必然爆炸必须后置」的分档动机冲突；抬档是独立裁决，触发条件是组件契约到场（届时表格已由第四条闸强制退役）。
- **读数与语料零漂移（读数随仓演进，口径不变）**：本仓当前基线 SHALL 为——`docMissing` **36**（33 + 3）、`apiEntryMissing` **0**、`apiMissing` **0**、其余十值各 **0**、`status = clean`、`passed = true`（既有的「43（41 + 2）」是两次变更之前的读数，成因见「契约判据的验证语料与报告数基线」）；`api/**` sidecar SHALL 仍恰 **13** 份（本变更新用例落在既有 `drift/**` sidecar，MUST NOT 新建 `api/**` 文件）；`corpus/drift/**` 的**既有 16 份金样逐字节零变化**（语料契约 `App.Mod.md` 零表格行），本变更新增的 `api-*` case 是**新增文件**、MUST NOT 改写任何既有快照。
- **零改动面（同批断言）**：`api::check` 的输入面与求值序、`ArchiApiFindingKind::severity()` 三档（`apiUndeclared` 仍 `Off`）、`api::gating_findings` 的入选判据逐字不变；`archi_api` 命令响应与门禁 `LocalArchiCheck` 报告形状不变（`ArchiApiFindingKind` 十三值 / `category` 七值零新增 ⇒ 判据面无需重绑）；`pnpm api:check` / `pnpm check` 链的段序与命令串不变；呈现面 `contract-panel.tsx` / `model-info.tsx` 零重算、零改动。**不在此列的三处**（同批由其它能力登记）：`drift/report.rs` 的 `ArchiDriftKind` +1 值、`drift/mod.rs` 的两桶装配、以及 `bindings.ts` 里 `ArchiDriftKind` 类型的随动——`bindings:export` 的幂等只对**判据面**成立，MUST NOT 写成「生成物整体零 diff」。

#### Scenario: 表格复述契约名字只装载期中止，不进报告

- **WHEN** 某 `api = declared` 元素的契约已声明 op `is_set`，其散文段又有一张以 `` `is_set` `` 起首的表格行
- **THEN** 装载期判 `ApiError::Contract` 并中止；报告面 MUST NOT 产出任何新条目（`apiFindings` 内零该事实、`ALL` 长度仍 13、`category` 七值、`severity()` 三档不变）；`status` / `passed` 的推导路径零改动

#### Scenario: 留白期散文表格零判据

- **WHEN** 8 个前端组件元素的契约三项全空、散文表格共 **28** 条数据行，且这些名字在代码导出面成立
- **THEN** 装载零错误、`apiFindings` 零新增条目；`apiUndeclared` 不产出（档位仍 `off`）；把某元素契约补一个 `api-types` 键 `AgentInput` 后，报错来自装载期（`ApiError::Contract`）而**不是** `apiUndeclared`

#### Scenario: 判据输入面零表格读取

- **WHEN** 审查 `check.rs` 的输入装配、`ApiCheckInput` 的字段面与 `apiUndeclared` 的声明集来源
- **THEN** 零表格行读取点（散文表格的唯一读者是 `contract::verify`）；`apiUndeclared` 声明集只取 `<FQN>.schema.json` 的 `$defs` 键集；`doc_facts` 三态与 `entry_state` 口径逐字不变

#### Scenario: 当前段读数与语料守护零漂移

- **WHEN** 对本仓模型跑一次检查并核对语料面
- **THEN** `docMissing 36`、其余十二值各 0、`status = clean`、`passed = true` 逐项与基线表相等；`api/**` sidecar 仍 13 份；既有 16 份 drift 金样逐字节不变且新增 `api-*` case 全部登记在 `DRIFT_CASES`；判据面（`api/**` 的求值面与纳管子集函数）零差异——`drift/report.rs` / `drift/mod.rs` 的差异**仅**为本变更登记的 `ArchiDriftKind` +1 值与两桶装配，零其它改动（`apiFindings` 槽位、十三值、`severity()`、`status` / `passed` 推导逐字不变）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/report.rs` | 判据词汇与报告槽位 | `ArchiApiFindingKind`（**十三值** + `ALL` + `as_str` + `severity()`）、`ApiFindingSeverity`（**三值** `Off` / `Warn` / `Error` + `ALL` + `as_str`）、`ArchiApiFinding { kind, category, element, target, message, occurrences }`、`ArchiDriftReport.api_findings`、`REPORT_VERSION = 3`（不递增，纳管口径入 `status` 推导）；`ApiMissing` 的 doc 语义收窄为「有约定无兑现」，`DocMissing` / `DocNotFound` / `ApiDocMissing` 的 doc 语义各自单成因（`ApiEntryMissing` 的四理由同值，MUST NOT 加第 14 值） |
| `crates/core/archi/src/api/mod.rs::enabled` | 启用判据（**收紧，行为变化**） | `enabled(root, elements) -> bool` = 任一 `path` 非空元素的入口文档载体在盘且 `DocFact::Present { has_api_fence: true }`；MUST NOT 退化为「递归任意 `*.md`」；判据与判据面同源（同一 `doc_facts` / 同一 fence 语言标常量），本能力**不另判一套**（`desktop-archi-contract-panel` 的 `enabled` 字段直接取它） |
| `crates/core/archi/src/api/mod.rs::gating_findings` | 纳管子集筛选单点 | 入选 = `element` 非空 ∧（declared：派生契约 md 在盘 / none：`doc` 已声明）；`docMissing` 两侧都不成立 ⇒ 永不纳管；`drift` 侧 MUST NOT 自行判文件存在性 |
| `crates/core/archi/src/api/mod.rs::load_contracts` | load 阶段事实装配 | `ApiSnapshot` 增 `doc_facts: BTreeMap<Fqn, DocFact>`（`Absent` / `Missing` / `Present { has_api_fence }`），每份 doc 只读一次即弃（不缓存全文）；`ApiCheckInput` 同批携入 |
| `crates/core/archi/src/api/contract.rs` | fence 语言标单点 | `FENCE_OP` / `FENCE_TYPES` 提 `pub(crate)`，供 `apiDocMissing` 与 `enabled` 复用；MUST NOT 抄第二份字面量 |
| `crates/core/archi/src/api/check.rs` | 十三判据单点求值 | `check(ApiCheckInput) -> Vec<ArchiApiFinding>`：**两支首错短路**（none 支 `apiUnexpected` → `docMissing` → `docNotFound`；declared 支 `docMissing` → `apiDocMissing` → `apiUnexpected` → `apiEntryMissing` → `apiMissing` → 一致性）+ **pair 锚点**（`nested → ref → shadowing`，MUST NOT 引用 `relations`）；`category_of` **七值**（+ `doc`）；`missing_contracts` 只剩形态②；`apiDocMissing` 只读 `input.doc_facts`（零读盘、零 `contract::parse`）；声明集（op **与 `$defs` 类型**）vs 导出集、签名归一比对、嵌套透传五项语义逐字不变；**已删** `has_generated`（`.i` 存在不再充当入口证据）；`$defs` 只读 `schema.json`；零第二份判定；**导出集单点**：导出集由 `extract::extract_element` 给出（TS 侧即入口导出闭包）⇒ `apiMissing` 只减不增、`apiUndeclared` 仍 `off`、`apiSignatureMismatch` 仍逐符号归一比对；判据面 MUST NOT 自行解析 re-export、MUST NOT 内联 owner 归属判断（那是取数面的单点） |
| `crates/core/archi/src/drift/mod.rs` | 报告装配单点 | `check()` 内调 `api::check` 装配 `api_findings`（与模型面并列）并取纳管子集传 `derive_status`；`derive_status` 在纳管子集上**分档**（任一 `Error` ⇒ `errors`）；`passed` 判据追加「error 档 `apiFindings` 空」；`status == errors ⟺ passed == false` 不变量保持 |
| `crates/core/archi/src/api/check_test.rs` | 判据语料（本变更：夹具落点换段） | 契约夹具由 `<archi_api_dir>/<分层目录…>/` 换到 `<archi_models_dir>/<分层目录…>/`，每条判据的正反例与断言值逐字不变； 十三判据正反例（三值新判据的用例全在既有 sidecar 内）/ entry 互斥（含 `Absent` 与四种 `Invalid` 理由各一条）/ 「`.i` 在盘但无入口仍报 ①」/ `api = none` + `entry` 有值零判据 / 签名归一 / 留白双向 / 交集 / 非直接子元素 / 跨 crate / 透传漂移 / `severity()` 与 `ALL` 逐值对拍；`function()` / `type_symbol()` 夹具补 `ApiSymbol.file`（形状适配，断言值零变化，不新增判据用例） |
| `crates/core/archi/tests/corpus/drift/**` + `tests/golden_test.rs` | 报告面语料 | 十六份金样随「首错短路 + 纳管分档」显式重录并逐份留痕成因；`corpus/drift/<case>/{contracts,models,repo}` + `expected/<case>.<mode>.json`；`DRIFT_CASES` 登记面即驱动表（未登记 / 目录不存在 / 缺 `apiFindings` 键三种失配皆红）；`api-mismatch` case 保持非空 `apiFindings`；golden 沿 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 |
| `crates/core/archi/tests/vocabulary_test.rs` | 词汇对拍 | `ArchiApiFindingKind::ALL` 长度 **13** 与 `as_str` / `severity()` 逐值对拍；`WarningKind` / `DocKind` / `DocSource` 三组对拍删除；`SearchScope` 三值词面换 `doc` |
| `<change>/reports/archi/<timestamp>.json`（门禁留痕） | 运行时产物 | 报告由门禁进程写出并作移交留痕（`apiFindings` 长度 **36**、`status = clean`）；`reports/archi/` 不入库、不计入交付面、不作判红面（`??` 属预期） |

### API: `crates/core/archi/src/drift/mod.rs` + `src/api/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `api::gating_findings` | 改 | `(root, &[ArchiElement], &[ArchiApiFinding]) -> Vec<ArchiApiFinding>`：纳管子集筛选单点（口径由「契约文件在案」改为「入口文档承诺已成立」；`element = None` 落选） |
| `drift::check` | 改 | `api_findings` 装配后取纳管子集传给 `derive_status`；其余装配序与字段逐字不变 |
| `derive_status` | 改 | 纳管子集入参保留；新增 `errors` 分支（纳管子集含 `error` 档）；`warnings` 分支条件收为「纳管子集非空且其中无 `error` 档」；`skipped` / `noChanges` 的先后序与语义逐字不变 |
| `drift/report.rs` | 改 | `REPORT_VERSION` 2 → 3 与兼容说明；14 字段形状零改动 |

### Component: 呈现面（归 `desktop-archi-contract-panel`）

| 项 | 描述 |
|---|---|
| `views/architecture/components/contract-panel.tsx` | 判据轴按 `category` 分组呈现同一份 `apiFindings`（零重算） |
| `views/architecture/components/model-info.tsx` | 按 `element === fqn` 过滤呈现本元素判据（零二次 invoke） |

### 模块增量：报告数与入口面读数随动（relation-api-dependency-check）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/**` | 十三判据求值（零改动） | 判据种类、两支首错序、`severity()`、`gating_findings` 与 `doc_facts` 三态逐字不变；`api = none` 元素退出入口判据的语义不变（新元素因此零 `apiEntryMissing`） |
| `crates/core/archi/src/drift/report.rs` | 报告（判据面零改动） | `apiFindings` 槽位、`ArchiApiFindingKind` 十三值、`severity()` 三档、`status` / `passed` 推导零改动；同文件内的 `ArchiDriftKind` +1 值（`undeclaredApi`）与 `ArchiObservationKind` 产出条件由 `desktop-archi-drift` 登记，**不属**本能力的零改动面 |
| 本能力基线表（spec 面） | 改 | 分母 45 → **46**、`docMissing` 35 → **36**（33 declared + 3 none）；历史链补两段并写明「44 / 43 早已陈旧」的成因，MUST NOT 让旧读数与新读数并排而无解释 |
| 本仓读数留痕（change `reports/`） | 新增 | 一次全量 `archi_drift` 的逐 `kind` 计数表（与基线表逐项相等）+ `git status` 显示 `reports/archi/` 未入库；MUST NOT 写入 crate 语料 |
| `crates/core/archi/tests/corpus/drift/**` + `tests/golden_test.rs` | 语料面（只加不改） | 既有 16 份快照逐字节不变；新增 `api-*` case（声明满 / 缺一项 / 多一项 / 未填 `apis`）各自登记进 `DRIFT_CASES`；`api-mismatch` 仍非空 `apiFindings` |
| 门禁留痕行（spec 面既有行） | 读数随动 | 「`apiFindings` 长度 **43**」→ **36**；`status = clean` 与「不入库 / 不作判红面」口径逐字保留 |
