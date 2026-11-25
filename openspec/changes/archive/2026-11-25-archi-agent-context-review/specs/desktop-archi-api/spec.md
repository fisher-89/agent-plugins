# desktop-archi-api Specification（delta: archi-agent-context-review）

## Purpose

本 delta 把模块资产拆成三份分工并把**接口定义升为手写真相**：`<FQN>.schema.json` 是接口唯一凭据（读面零 md 回落、坏 schema 停摆），`<FQN>.md` 退为纯散文文档（无结构、历史 fence 惰性），`INDEX.toml` / `.i.rs` / `.i.ts` 仍是三类生成物。

- **读面换真相**：`load_contracts` 只读 schema（`Contract::from_schema` 解包 `x-function`）；`contract::parse`（约 600 行 md 解析器）与 `schema::build_schema` 退役，七族名字闸 / 落点闸 / `required ⊆ properties` 与 `verify_defs` 零改造复用。
- **写面补闸**：新增 `validate_schema(fqn, text)`（九道闸），`write` / `edit` 与装载期共用同一实现（只差错误映射）——资产② 与资产③ 同一条「坏文本不可产出」纪律。
- **形态与生成面**：`properties.<k>` 二选一形态白名单、`$defs` 镜像、跨元素 `$ref` 前缀校验、零未知顶层键；`GENERATED_NOTICE` 语义反转为写者纪律；`bootstrap` 改双草稿（schema + doc）、新增 `link`。
- **规则面同步**：`AGENT.md` / `CLAUDE.md` 的「模块 API 契约」节五条要点重写（三资产分工 / 三类生成物 / 留白 = 空 schema / 纳管口径）。

零模型改动、零 `.i.*` 逐字节变化、零新 crate、零新路径派生函数。

## MODIFIED Requirements

### Requirement: 契约资产形态与唯一手写真相

模块资产 SHALL 按**三份分工**落位，每份只回答一个问题，MUST NOT 相互兼答：

| # | 资产 | 载体 | 结构 | 回答的问题 |
|---|---|---|---|---|
| ① | 文档 | `<archi_models_dir>/<分层目录…>/<FQN>.md` | **无结构**（段落自定） | 为什么这样切、有什么坑、什么不归它管 |
| ② | 接口 | `<archi_models_dir>/<分层目录…>/<FQN>.schema.json` | JSON Schema（强结构） | 这条边跨过去时接触面长什么样 |
| ③ | 拓扑 | `<archi_graph_model_dir>/NN-<域>.toml` | TOML（强结构，见 `repo-archi-model`） | 谁在谁的里面、谁依赖谁 |

`<archi_dir>` 经 `foundation::layout::archi_dir`、资产目录取既有 `foundation::layout::archi_models_dir`（MUST NOT 行内字面量、MUST NOT 新增第二个目录派生函数）。`<分层目录…>` SHALL 是元素 FQN 的**祖先链镜像**（取 FQN 段链去掉叶段后的逐段路径：`Backend.Infra.ArchiRuntime` → `Backend/Infra/`），元素身份仍是 FQN（文件名恒为完整 FQN），目录只是布局。

- **接口的唯一凭据是 `<FQN>.schema.json`，零 md 回落**：读面（装载 / 判据 / 生成物投影）SHALL 只从 schema 取接口事实，MUST NOT 解析 md 的 fenced json；schema 不在盘 ⇒ 该元素「接口未写」（`present = false`，无判据）；schema 在盘但装载校验失败 ⇒ `ApiError` **停摆**（与模型结构错同族，MUST NOT 产出半真报告）。`schemaInvalid` / `schemaMissing` 两条判据不在本期（后续 change），本期以「停摆 / 跳过」表达。
- **文档无结构**：`<FQN>.md` MUST NOT 被任何解析器赋予段语义——零 `##` 段名白名单、零段数约束、零 fence 义务、零 H1 校验。`bootstrap` 的 doc 草稿给出 H1，但它不被任何闸校验。文档是**产品面**：架构页右栏「文档」段直接把路径交给 `FileViewerBlock` 渲染给用户读（人 + AI 共读），故它 MUST 可读、MUST 只留语义（职能 / 边界 / 不变量 / 承载理由 / 坑）。
- **名字清单的唯一持有者是 schema**：导出名 / 签名 / 类型形状的唯一持有者为 `properties` 与 `$defs` 的键。文档 MUST NOT 枚举导出名（含表格、列表、裸正文三种形态）——该纪律**只有 prompt 与知识文本承载，零机械闸**（闸随 md 解析器一起退役）。历史 `api-op` / `api-types` / `api-reexport` fence SHALL 被当作**普通文档内容**：读面零解析、`archi_api check` 零新增计数、MUST NOT 设 `legacyFenceFiles` 一类读数；其清理归专门一轮，MUST NOT 为「fence 是否残留」新增判据。
- **`api` 字段只表达资产②的存在意图，二值**：`declared`（承诺有接口）/ `none`（显式无接口面）。MUST NOT 新增第三值（如 `deferred`）。
  - `api = none` 而 `<FQN>.schema.json` 在盘 ⇒ `apiUnexpected`（留白被违反）。
  - `api = declared` 而模型写了 `doc` ⇒ **不检查**（`doc` 字段零读者，见下）。
  - 留白（有导出却无对外约定）的**正规形态**是 `api = declared` + 空 schema（`$defs {}` + `properties {}`）——它是「看过、确实没有要声明的接口」的机器可读版本，与「还没写（schema 缺）」严格可分。
- **`doc` 字段保留但零判据读者**：判据面 / 纳管面 / 路径派生面 / 检索语料面都不读它（指不到盘不报、写了不报、`api = declared` 元素写了也不报）；保留是为了外部工作区写了 `doc` 不突然变成 `unknownField` 结构错。`api = none` 元素的入口文档仍可由它声明，`archi_module` 会照原样投影、画布「入口文档」段照旧渲染——那是呈现事实，不是判据。
- **接口面启用判据（换面）**：`api::enabled` SHALL 判「存在任一 `path` 非空 ∧ `api = declared` 元素的 `<FQN>.schema.json` 在盘」。MUST NOT 退化为「`<archi_models_dir>` 递归任意 `*.md` / `*.schema.json`」——非任何元素载体的散落文件 MUST NOT 构成启用证据；`api = none` 元素在盘的 schema 是违约事实（`apiUnexpected`），MUST NOT 构成启用证据。该判据与判据面同源，MUST NOT 在命令层另判一套。
- **不接入文档关联面**：资产 MUST NOT 成为任何「文档关联来源」（`explicit` / `adjacent` / `text` 三源已退役，MUST NOT 复活为第四种），MUST NOT 新增 `kind = api` 一类形态标记。正文抵达路径只有三条：`api.contractPath`（doc 路径）/ `api.schemaPath` / 通用 `read_file` 按路径自取；检索面只收路径串、零正文。
- **与模型分目录**：`<archi_models_dir>` SHALL 只容纳文档与接口两份资产；模型声明在 `<archi_graph_model_dir>`。两面的分离判据是**目录**（模型收面只扫 `graph/model/` 一级 + `.toml`），MUST NOT 引入目录级豁免清单。
- **不依赖语言概念**：资产形态、目录布局、索引中 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` 一类语言专有概念——语言特定部分只落取数器（`api/extract/**`）与渲染器（`api/emit/**`）。
- **`INDEX.toml` 的 form status 值域不动**：`declared` / `absent` 两词保留，`present`（`declared` 的判定）改判「schema 在盘」；命名债（`declared` 读起来像模型意图）并入清理轮，MUST NOT 在本变更改值域（改值域会让 `INDEX.toml` 全条目 diff，破坏「生成物逐字节不变」的验收锚）。

#### Scenario: 三份资产各答一个问题且同目录落位

- **WHEN** 审查 `Backend.Infra.ArchiRuntime` 的三份资产
- **THEN** `Backend/Infra/Backend.Infra.ArchiRuntime.md` 是散文（任意段名 / 段数均可）、`Backend/Infra/Backend.Infra.ArchiRuntime.schema.json` 是接口真相（`properties` / `$defs` 键即导出名）、`graph/model/*.toml` 只含元素与关系；MUST NOT 出现「md 内 fenced json 被当作接口真相」的读取路径

#### Scenario: 文档零结构与历史 fence 惰性

- **WHEN** 某文档有 10 个自定 `##` 段 + 两张表格 + 两个历史 `api-op` fence，另一文档只有 H1 与一段散文
- **THEN** 两者装载路径完全相同（零解析、零错误、零判据、零计数）；`archi_api check` 的响应不含任何「fence 残留」计数键

#### Scenario: 启用判据与散落文件

- **WHEN** 观察根下 `models/notes.md`（非任何元素载体）与 `models/Backend/notes.schema.json`（非 `<FQN>` 主干）存在，而全部 `declared` 元素都无 schema
- **THEN** `enabled = false`、`apiFindings = []`；随后给任一 `api = declared` 元素补一份合法 schema ⇒ `enabled = true`

#### Scenario: 留白是空 schema 而非缺文件

- **WHEN** 某 `api = declared` 元素的 schema 为 `{"$defs": {}, "properties": {}}`，另一同类元素无 schema
- **THEN** 前者 `present = true` 且零判据（这是「已看过、无对外约定」：空清单是**已声明**的空）；后者 `present = false` ⇒ 「接口未写」**零判据**（`schemaMissing` 归后续 change，本期不以任何别名预铺）——两者 MUST NOT 被压成同一读数（空 schema 与缺文件是两个不同事实，`docMissing` 只判派生 md）

#### Scenario: none 元素带 schema 即违约

- **WHEN** 某 `api = none` 元素目录下存在 `<FQN>.schema.json`
- **THEN** 判据面报一条 `apiUnexpected`（`category = entry`、`warn`）；装载面零停摆（违约是判据，不是结构错）；该元素 MUST NOT 因此进入启用判据的成立面

#### Scenario: doc 字段零判据读者

- **WHEN** 某 `api = declared` 元素写了 `doc = "./README.md"`，同时其 schema 在盘；另一 `api = none` 元素的 `doc` 指向不存在的文件
- **THEN** 两者均零判据、零纳管影响、零装载影响（`doc` 不被判据 / 纳管 / 派生 / 检索消费）；条数与 `status` 与该字段缺席时逐字相同；`archi_module` 仍原样投影该值

### Requirement: 引用与命名规则

接口定义内的类型身份与引用关系 SHALL 遵循五条硬规则（来源面由 md fence 换为 schema 本体，规则内容不变）：

| # | 规则 | 违反后果 |
|---|---|---|
| 1 | 身份 SHALL 为 `$defs` **键名**，MUST NOT 为 pointer 路径 | 同名两处定义 → `ApiError` |
| 2 | 每个具名类型 SHALL 在顶层 `$defs` 定义恰一次；嵌套 `$defs` 一律提升且**原名保留** | 提升名碰撞 → `ApiError`，禁静默改名 |
| 3 | 所有 `$ref` target SHALL 为 `$defs` 顶层条目 | 指向 `properties` / `items` / `allOf/0` → `ApiError` |
| 4 | 多个 `$ref` 指向同一类型时 SHALL 只生成一个定义 | 重复定义 → `ApiError` |
| 5 | MUST NOT 按结构去重（字段类型序列相同不等于同一类型） | 合并即语义错 |

- **位置语义**：第一层 `properties` 为模块导出成员（`x-function` 操作 / `$ref` 到具名类型）；`$defs` 为本模块自有类型与透传纯 `$ref` alias。**公共面** = 第一层 `properties` + 第一层经 `$ref` 可达的闭包（自动排除不出现在任何签名里的纯内部类型）。
- **键空间互斥**：`properties` 键集与 `$defs` 键集 MUST 无重名（op id 与类型名不得撞）；任一冲突即 `ApiError`，文案点名该名与冲突的键空间，MUST NOT 由后写者静默覆盖（JSON 对象会折叠成「后者胜」，丢一个导出而无任何报警）。
- **跨元素 `$ref` SHALL 用相对文件路径**：同目录 `./<FQN>.schema.json#/$defs/<name>`；跨目录的前缀由两侧分层目录逐段求出（先上溯到公共祖先、再逐段下行），由**单点函数 `paths::ref_relative(from_fqn, to_fqn)`** 产出，MUST NOT 手算、MUST NOT 用自定义 URI scheme（实测 ajv 对 `module://X#/$defs/Y` 报 `can't resolve reference`，只认标准 URI）。
- **跨元素 `$ref` 的前缀 SHALL 被逐字校验**：前缀 MUST 逐字等于 `ref_relative(本FQN, 目标FQN)`。该判据今天完全不存在（反查只取 `#` 前文件名的末段去后缀，前缀从不校验），少一个 `../` 会绕过 `apiRefMissing` 并在 `.i.ts` 里 import 一个不存在的路径、到编译期才炸。校验点 SHALL 是写闸与装载校验的**同一个** `validate_schema`（见 ADDED「接口定义文件的装载校验与写闸」），文案 SHALL 给出期望串。
- **透传 alias 形态**：`$defs` 的透传条目 SHALL 是纯 `$ref` alias（值指向**直接子元素**的 schema 文件 + `#/$defs/<name>`），MUST NOT 携带结构关键字（`type` / `properties` / `enum` / `const` / `items` / `oneOf`）——携带即沦为重复定义。透传目标 FQN 由 `$ref` 的文件主干反查（文件名即身份）。
- **`$id` / `title` / 文件名三者一致**：`$id` MUST 等于 `<FQN>.schema.json`、`title` MUST 等于 FQN、文件主干 MUST 等于 FQN；任一漂移即 `ApiError`（三处持有同一事实，漂移会让反查与报告文案指向不同元素）。

#### Scenario: 键空间撞名即报错（不静默覆盖）

- **WHEN** 某 schema 同时把 `T` 用作 `properties.T` 的 op 键与 `$defs.T` 的类型键
- **THEN** 写闸（`write` / `edit` 落盘前）与装载校验各报 `ApiError` 并点名 `T` 与两个键空间；盘上字节零变化（写闸分支）

#### Scenario: `$ref` 指向非顶层被拒

- **WHEN** schema 写 `$ref: "#/properties/attr"` 或 `#/properties/x/items`
- **THEN** 报错，文案指明 target 不是 `$defs` 顶层条目

#### Scenario: 跨元素 `$ref` 前缀逐字校验

- **WHEN** 某元素对子元素的透传写成 `"../Agent/Child.schema.json#/$defs/Y"`，而正确串是 `"../../Desktop/Agent/Child.schema.json#/$defs/Y"`
- **THEN** 写闸 `Err` 零落盘，文案并列「当前串」与「期望串」；改成正确串后落盘成功

#### Scenario: 同结构不同类型不合并

- **WHEN** schema 声明两个 `type: object` 且 `properties` 序列完全相同的类型 `TurnQuestion` 与 `ProjectConfig`
- **THEN** 生成两个独立定义、`$ref` 各自指向自己的键；MUST NOT 合并为一条

### Requirement: 关键字集、类型映射与错误面

接口定义 SHALL 只用标准 JSON Schema 关键字加**三个**扩展构造（`x-` 面封闭，MUST NOT 自造新的 `x-` 前缀键）：

| 关键字 | 语义 | 生成 |
|---|---|---|
| `object` / `enum` / `const` / `default` / `oneOf` / `$ref` | 标准关键字 | 见映射表 |
| `x-class` | 声明为 trait，**仅允许方法**（出现属性即 `ApiError`） | `pub trait` / `export interface` |
| `x-function` | 函数签名：`params` / `returns` / `async` / `owner` / `description` | `pub fn` / `export type Fn = (…) => R` |
| `x-common` | 自由注记（`unit` / `note` / `since` / `owner` / `tag`）+ `degraded`（取值受封闭四值词表校验） | 不变换 |

- **`x-function` 的落点 SHALL 封闭二处**：`properties.<k>.x-function`（模块操作）与 `$defs.<T>.properties.<k>.x-function`（函数指针字段）；其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 及其更深层）即 `ApiError` 并点名 JSON pointer。位置类错误的正确处置是**搬位置**，不是在渲染期兜底。
- **`properties.<k>` 的形态 SHALL 封闭二选一**：`{ "x-function": { … } }` 或 `{ "$ref": "#/$defs/<名>" }` 且 `<名> ∈ $defs`。其余形态（内联 `object` / 数组 / `oneOf` / 裸 `type`）即 `ApiError`——内联 object 会经 `schema_type` 的 type 分派静默退化为 `Record<string, unknown>`（编译通过、类型全丢），这是本闸存在的唯一理由。「想复用类型就放进 `$defs`，想声明操作就用 `x-function`」。
- **`x-function` 内层是自由形状**：闸只判外层形态，MUST NOT 对 `x-function` 内层（`async` / `description` / 将来可能有的 `deprecated`）做未知键校验。
- **op 的散文说明 SHALL 落在 `x-function.description`**：写**调用方要知道的语义**（它做什么 / 边界在哪 / 为什么），MUST NOT 写实现细节、MUST NOT 复述签名（签名就在同一处，复述是第二事实源）。`Contract::from_schema` 装配 `ContractOp.signature` 时 SHALL **解包** `x-function` 并**丢弃** `description`（`emit` 只读 `params` / `returns` / `owner`，渲染无关字段不进 emit 输入）。
- **`$defs.<T>.description` 承载类型语义**；`$comment` 只承载**写者纪律**（人读、不生成），MUST NOT 被任何判据读取。
- **注解载体三选封闭**：`description`（进 `.i.*` 文档注释的语义）/ `$comment`（只在源文件）/ `x-*`（只在本表三项内）。JSONC 行注释 MUST NOT 采纳（解析器多一份真相）。
- 映射表（`object` → `struct` / `interface`；`enum` → Rust `enum` + TS `as const` + `type`；`oneOf`/`anyOf` → 带负载 `enum` / union；`x-class` → `pub trait` / `export interface`；标量 → newtype / 别名；非 `required` 字段 → `Option<T>` / `{field}?: T`；显式 null 三分支 → `Option<T>` / `T | null`；`x-function` → `pub fn` / `(…) => R`；错误面 `Outcome` 信封 → `Result<T, String>` / union 或 `Promise` reject）与两条位置分派纪律 SHALL 逐条不变。
- **可选与可空两轴、纯 null 档**、**数组元素位可空加括号**、**标量必 newtype 不 trait**、**构造器 MUST NOT 进契约**（`properties` 键出现 `new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open` 即 `apiConstructorDeclared`）、**枚举列举由函数承担**、**泛型不支持**、**TS 侧不做品牌类型**、**`common.json` 只承载映射规则** —— 以上各条逐字沿用。
- **两道人字闸（改写落点，判据内容不变）**：① **装载期**（`validate_schema` 同一实现）**七族名字**——`properties` 键 / `$defs` 键 / 透传键 / `x-function.params[].name` / `$defs` 条目的第一层 `properties` 键 / `x-function.owner` / `$ref` 末段名——MUST 是 ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*`，不成立即 `ApiError` 并点名族别与 pointer；同趟遍历另判 `x-function` 落点与 `required ⊆ properties`。② **渲染期**一切来自契约的字符串字面量与注释 MUST 经该语言单点转义；`emit` 内零标识符判据、两侧 `value_type` 零 `x-function` 分支是它的机械判据。
- **四条明确不做**逐字沿用（`x-interface` / `x-nominal` / `x-singleSource` 独立构造、自造协议记法、L/S 双层锁、Zod 作通用载体）。

#### Scenario: 内联 object 在写闸即被拒

- **WHEN** `write` 落一份把 `properties.send` 写成 `{"type": "object", "properties": {...}}` 的 schema
- **THEN** `Err` 零落盘，文案给出二选一形态与「想复用类型就放进 `$defs`」的出路；该内联形态 MUST NOT 抵达 `emit`（否则 TS 侧静默退化为 `Record<string, unknown>`）

#### Scenario: 函数指针字段与模块操作可辨

- **WHEN** schema 同时含 `properties.parse` 的 `x-function` 与 `$defs.Widget.properties.parse` 的 `x-function`
- **THEN** 两处落点均合法（封闭二处）；前者生成为模块级 `pub fn`，后者生成为 `pub parse: fn(&str) -> …` 字段，互不吞并

#### Scenario: `x-function.description` 进契约但不进 emit

- **WHEN** 某 op 的 `x-function` 带 `description` 与 `returns`，装载后取 `ContractOp.signature`
- **THEN** `signature` 恰为解包后的 `{params, returns, owner?}`（`description` 已丢弃）；`.i.rs` 的签名见证与不带 `description` 时逐字节相同

#### Scenario: `x-class` 带属性即报错

- **WHEN** 某 `$defs` 条目标 `x-class` 但同时声明 `properties: { "count": { "type": "integer" } }`
- **THEN** `ApiError`（trait 仅允许方法）；去掉 `x-class` 改为普通 `object` 后通过

#### Scenario: 自造 `x-` 键被拒

- **WHEN** schema 顶层出现 `x-note` / `x-owner` 一类新前缀键，或 `properties.<k>` 出现 `x-deprecated`
- **THEN** 前者触发「零未知顶层键」`ApiError`；后者触发形态白名单 `ApiError`（`properties.<k>` 只允许 `x-function` 或 `$ref`）

#### Scenario: 构造器声明即契约自身矛盾

- **WHEN** schema 的 `properties` 出现键 `new`（带 `x-function`）
- **THEN** 报 `apiConstructorDeclared` 并点名该 op；删掉该键后生成物仍自动产出 `new()`

### Requirement: 生成物入仓与零 diff 门挂载

**三类**生成物 SHALL 全部入仓并进零 diff 门，形态沿既有 `bindings.ts` 先例（MUST NOT 另造第二套守卫机制）：

| 生成物 | 落点 | 入库 | 门禁 |
|---|---|---|---|
| `INDEX.toml` | `<archi_graph_dir>/`（全局唯一一份） | 是 | `api:check` 零 diff |
| `<入口主干>.i.rs` | **元素实现入口同级**（`crates/infra/archi/src/lib.i.rs`） | 是 | `api:check` 零 diff + `cargo check` |
| `<入口主干>.i.ts` | **元素实现入口同级**（`views/welcome-view.i.ts`） | 是 | `api:check` 零 diff + `tsc --noEmit` |

- **`<FQN>.schema.json` 不再是生成物**：它是手写真相（第二份资产），MUST NOT 进 `api:check` 的应生成集、MUST NOT 进 `orphaned` 列、MUST NOT 由任何生成器写出。它同样入库（git 基线是人工 review 与审计的入口）。
- **`INDEX.toml` 的 `generated` 列取入口同级路径**：无入口（`Absent` / `Invalid`）的元素该列留空——生成物路径是入口的函数，无入口即无路径可写。
- **孤儿扫描 SHALL 只剩一个根**：`<archi_graph_dir>/INDEX.toml`（全局唯一）。`*.schema.json` MUST NOT 进孤儿枚举——不去掉就是「11 份手写文件全进 `orphaned`」，症状是「全红」而不是「少一条」。深度上限防环、符号链接不跟随、`read_dir` 只出现在有界枚举里、repo 相对路径串由同一 `display` 单点产出，四条纪律逐字沿用。
- **脚本面封闭两条**：`api:export`（写盘）与 `api:check`（生成到内存后比对、**零写入**，差异即非零退出；比对由工具内完成，不依赖任何 `git diff` 段）。`check` 链 SHALL 为四段：`server:check && bindings:export && api:check && client:check`——段序与段数零变化，`bindings:export` 与 `client:check` 两段命令形态逐字不变。
- **应用内两驱动面与脚本面同源**：渲染与三类差异比对各有唯一实现（`core/archi` 纯函数），写盘有唯一实现（`infra/archi` 的 `write_artifacts`）；四条消费路径（两 IPC / 两 CLI 子命令）MUST NOT 各写一份。
- **`knip` 边界**：`knip.json` 的 `ignore` SHALL 含 `src/**/*.i.ts`；MUST NOT 新增 `ignoreExportsUsedInFile` 或 `ignoreDependencies`。
- **类型绑定再生成**：`ArchiApiFindingKind`（十值）/ `ArchiApiFinding` / 报告字段 `apiFindings` / `ArchiModuleDetail` 的新字段 SHALL 经 `bindings:export` 进 `bindings.ts` 与 `bindings.zod.ts`（再生成后入库，过期由提交前人工核对）；本能力 MUST NOT 手写前端类型镜像。
- **TS 侧形状校验的机制前提**：实现侧 SHALL `import type` 引用生成的 `.i.ts` 并标注返回类型；`isolatedModules` 下重导出须写 `export type { … }`。
- **版本纪律**：本变更改 `packages/desktop` 源码 ⇒ `package.json` 的 `version` SHALL 相对**落地时 `HEAD` 的现值 +1 patch**（规范内 MUST NOT 写死具体版本号）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

#### Scenario: 三类生成物齐入仓且零 diff

- **WHEN** 执行 `api:export` 后再执行 `api:check`
- **THEN** 三类生成物均在盘且已在 git 内，第二次检查零差异、退出码 0；`git status` 干净

#### Scenario: 手改 `.schema.json` 不被 `api:check` 判红

- **WHEN** 手改一份已提交的 `<FQN>.schema.json`（合法改动）后执行 `api:check`
- **THEN** 差异面只报 `.i.rs` / `.i.ts` / `INDEX.toml` 的应随动项（若有），**不报该 schema 为 `orphaned` / `mismatched`**——它不是生成物；回退 .i.* 后检查恢复通过

#### Scenario: 手改真生成物即被拦

- **WHEN** 手改已提交的 `INDEX.toml`、`.i.rs` 或 `.i.ts` 任一后执行 `api:check`
- **THEN** 检查以非零退出并列出被改文件（`--check` 阶段无写盘）；回退后恢复通过

#### Scenario: 孤儿生成物在任意深度均被检出

- **WHEN** 删除元素或改其根路径后，在任意深度留下 `*.i.rs` / `*.i.ts` 残留，随后执行 `api:check`
- **THEN** 「应删未删」逐条列出这些文件并以非零退出；`target` / `node_modules` / `dist` / `build` / `.git` 里的同名文件不参与；普通源文件（`*.ts` / `*.rs` / `*.md` / `*.schema.json`）零误伤

#### Scenario: check 链四段且 fmt 段为校验模式

- **WHEN** 审查 `package.json` 的 `check` 脚本与四段
- **THEN** `check` 逐字为 `pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check`；`api:check` 的调用形态逐字不变；`server:check` 的 fmt 段为 `cargo fmt --all --check`

### Requirement: 仓库根文档的契约纪律节

仓库根 `AGENT.md` SHALL 承载一节 `## 模块 API 契约` 作为 agent 侧的操作纪律入口，内容 SHALL 限于**指针与最简规则**，MUST NOT 复制契约正文、判据长表或 fenced json 示例：

- 该节 SHALL 恰含**五条要点**：① **三资产分工与落位**——`<archi_models_dir>/<分层目录…>/<FQN>.md` 写理由（文档，无结构）、同目录 `<FQN>.schema.json` 写接口（唯一凭据），并附一句「文档 MUST NOT 枚举导出名（名字清单的唯一持有者是 schema）」；② **三类生成物勿手改**（`INDEX.toml` 在 `<archi_graph_dir>`、**实现入口同级**的 `<入口主干>.i.rs` / `<入口主干>.i.ts`）与「已提交手改即零 diff 门红、未提交即被静默覆盖」；③ `api:export` / `api:check` 两脚本与 `check` 链的段序；④ `api = none` 是显式留白而非「还没写」，留白的正规形态是空 schema（`$defs {}` + `properties {}`）；⑤ **纳管口径**——`api = declared` ∧ schema 在盘的元素，其判据影响 `status`（`error` 档还进 `passed`），文档债（`docMissing`）恒 `warn`、永不纳管；索引与计数经 `<archi_graph_dir>/INDEX.toml` 查看。
- **历史 fence 的措辞 SHALL 只在第 ① 条出现一句**：「历史 `api-*` fence 是遗留内容（读面不读，清理中），MUST NOT 新增」——它是 agent 侧唯一可见的该事实出处；MUST NOT 展开成判据条件、错误文案样例或清理计划。
- 该节 MUST NOT 含契约示例正文、`ArchiApiFindingKind` 的十值长表或判据定义（定义归本能力与 `desktop-archi-api-check`）——文档与 spec 双写必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制即既有反例）。
- 该节 MUST NOT 出现 `openspec/specs/**` 路径，且 MUST NOT 把契约路径写进模型元素 `doc`。
- **条目数恒 5**：本变更 MUST NOT 增删要点（`entry` 字段、写闸细节、schema 校验九条都不成为第六条）——写闸与校验的细节归工具面与知识文本。
- `CLAUDE.md` SHALL 同步同一节且与 `AGENT.md` 保持既有同源纪律：两文件该节正文逐字相同，全文件差异 SHALL 限于节外既有差异（标题行、首行说明行、文件末尾换行）。

#### Scenario: 五条要点与分工表述在场

- **WHEN** 审查 `AGENT.md` 的 `## 模块 API 契约` 一节
- **THEN** `grep -c '^## 模块 API 契约' = 1`；节内恰 5 条要点；第 ① 条含「`.md` 写理由 / `.schema.json` 写接口」与「文档 MUST NOT 枚举导出名」两句；第 ② 条写「实现入口同级」+ `<入口主干>.i.rs` / `.i.ts`；第 ④ 条含空 schema 形态；第 ⑤ 条含「`docMissing` 永不纳管」

#### Scenario: 纪律节零正文与零 spec 路径

- **WHEN** 审查该节全文
- **THEN** `grep 'apiEntryMissing'` / `grep 'apiSignatureMismatch'` 在该节零命中；零 fenced json 代码块；零 `openspec/specs/` 路径；零 `entry` 字段说明

#### Scenario: 两份副本不分化

- **WHEN** 对 `AGENT.md` 与 `CLAUDE.md` 做 `diff`
- **THEN** 差异恰 2 处且全在 `## 模块 API 契约` 节外；该节在两文件各恰一处且节内正文零 diff 行

#### Scenario: 旧口径零残留

- **WHEN** 对该节做反向断言
- **THEN** `grep '四类生成物'` / `grep '唯一手写真相：.*\.md'` / `grep '三类 fenced json'` 在该节零命中；第 ② 条不再出现 `.schema.json`（它不是生成物）

### Requirement: 生成器、verify 闭环与 bootstrap 剪枝

`verify` SHALL 定义为机械判定而非独立模式：**`generate`（`--check` 零写入，比对在工具内完成）+ 语言编译器**，另加**写闸**（接口文件上盘即合法）。MUST NOT 引入归一函数、双层锁或语义 diff 比对器。

- **判定在 core，写盘在工具 crate**：接口文件的装载校验、生成物渲染、判据求值 SHALL 全部落 `crates/core/archi/src/api/**`（纯读、零写盘、零 spawn）；唯一写盘面 SHALL 为 `crates/tools/archi-api-gen`（CLI）与 `crates/infra/archi` 的 `write_artifacts` 单点。
- **CLI 三子命令封闭**：`generate`（写**三类**生成物：`INDEX.toml` / `<入口主干>.i.rs` / `<入口主干>.i.ts`）、`--check`（生成到内存后与磁盘比对，零写入，差异即非零退出并列文件名）、`bootstrap`（输出**双草稿**到 stdout，不写盘）。`generate` SHALL 幂等。MUST NOT 让 `generate` 写 `.schema.json`（它是手写真相）。
- **`bootstrap` 的出口形状**：`api::bootstrap(root, element)` SHALL 返回两段文本（schema 骨架 + doc 骨架）；CLI 与 agent 工具两条出口 SHALL 出**同一份**内容（CLI 打印单行 JSON `{ "schema": "…", "doc": "…" }`，工具回同名对象）。
- **`verify` 覆盖不到的三件事 SHALL 显式列出**：bootstrap 方向（代码 → 接口草稿，一次性迁移工具，非常态门禁）、跨模块语义不变量（散文 + 专项测试）、生成器自身正确性（生成器自己的单测）。
- **bootstrap SHALL 走 AST，MUST NOT 正则抽签名**（Rust 多行签名占 30–62%，`grep` 单行抽取会截半）；TS 侧只判 `export function`（实测 `export const` 里零箭头函数）。
- **`visibility` 封闭四值**（`external` / `crateOnly` / `unused` / `testOnly`）与其本期值域留白（TS 生产面只产 `external`，`unused` 降级接口零调用者）逐字沿用；MUST NOT 为 TS 造第二套死代码扫描。
- **试点先行与近似误差留痕**逐字沿用。

#### Scenario: verify 三件套闭环

- **WHEN** 改动代码签名但不重生成生成物
- **THEN** `api:check` 报差异并以非零退出（点名文件）；重生成后零 diff；若改动与接口定义不符则编译器报错

#### Scenario: `--check` 零写盘

- **WHEN** 以 `--check` 在有过期生成物的仓库上运行
- **THEN** 磁盘字节零变化（含生成物、`INDEX.toml`），退出码非零、输出列出差异文件

#### Scenario: generate 不产接口文件

- **WHEN** 删除一份 `<FQN>.schema.json` 后执行 `api:export`
- **THEN** 该文件**不被重建**（生成器无该产物）；`INDEX.toml` 的 `declared` 列对该元素转为 `absent`（schema 不在盘）

#### Scenario: bootstrap 双草稿两出口同源

- **WHEN** 分别执行 CLI `bootstrap <FQN>` 与调 `archi_api(action = "bootstrap", element = <FQN>)`
- **THEN** 两者的 schema 骨架与 doc 骨架逐字相同；CLI 输出为单行 JSON 两键，工具响应为同名两键；两处均零写盘

#### Scenario: visibility 四值判定

- **WHEN** 对同一 crate 的 `pub fn`（`pub mod` 链可达）/ `pub(crate) fn` / 私有 `mod` 内的 `pub fn` / `#[cfg(test)]` 树内被引用的 `pub fn` 跑 bootstrap
- **THEN** 依次判为 `external` / `crateOnly` / `crateOnly` / `testOnly`；TS 侧本期只产 `external` 一档（显式留白，MUST NOT 在 Scenario 里给出更宽的承诺）

### Requirement: 模块嵌套与选择性透传

父子元素的接口关系 SHALL 遵循「父 → 子」单向：父元素的 `$defs` SHALL **逐项**列出它透传的子元素导出，MUST NOT 由「子元素面的并集」自动推导。

透传 SHALL 同时表达于两处且互相一致：

| 载体 | 内容 | 用途 |
|---|---|---|
| 父 `<FQN>.schema.json` 的 `$defs` | 本地定义 **+ 透传项（纯 `$ref` alias，目标 = 直接子元素 schema + `#/$defs/<name>`）** | 机器检查 + 生成物（悬空引用编译期即报） |
| 父 `<FQN>.md` 散文 | 透传意图（人读，MUST NOT 枚举名字清单） | 人读 |

六条结构规则：

1. 透传条目 SHALL 是**纯 `$ref` alias**（`$ref` 指向子元素 schema 的 `$defs` 条目），MUST NOT 携带结构关键字；`api-reexport` fence 与「散文透传列表 ≡ fence 键集」的旧规则（及其 `apiReexportDrift` 判据）**随之退役**——散文面无结构、fence 面不再被读取。
2. 父 `$defs` 与任一子元素 `$defs` 的**类型名交集必须为空**（`apiShadowing`）；MUST NOT 依赖编译器兜住（实测 `pub use drift::*` + 同名 `struct` 只给 warning）。
3. 透传目标与跨模块 `$ref` SHALL 是**直接子元素**（FQN 前缀相邻）且其 schema 在盘且该元素 `api = declared`；跨层引用（父直连孙辈）报 `apiRefMissing`。
4. 嵌套元素的父子 MUST 同属**一个 crate**（Rust 侧跨 crate 父子即 `cyclic package dependency`）⇒ `apiNestedCrossCrate`；TS 侧无此约束。
5. 子元素 MUST NOT 依赖父元素（含类型）；共享类型 SHALL 下沉到兄弟元素或消解依赖——该规则由既有 `UndeclaredDependency` 判据报，本能力 MUST NOT 新增第二条判据。
6. 透传**可递归但不展开**：A ⊃ B ⊃ C 时 A 只声明透传 B，B 内部已透传 C 则自动带出。

两条推论（逐字沿用）：**透传是选择性的**（父契约逐项手写，MUST NOT 自动推导）；**环依赖从此不可能**（跨元素 `$ref` 只剩父→子与兄弟→兄弟）。另两条明确否决逐字沿用：MUST NOT 新增契约层依赖判据；契约记录「提供什么」不记录「实现了什么」。

#### Scenario: 父契约逐项声明而不自动推导

- **WHEN** 为 `Desktop.Components.Agent` 写接口：`$defs` 只声明 barrel 实际重导出的 7 个名字中的 2 个（`AgentInput` / `AgentSessionView`），其余重导出项后续逐条追加，`AgentBlocks` / `AgentError` / `ToolCards` 三个子元素不出现
- **THEN** `$defs` 恰 2 条纯 `$ref` 条目、零本地定义；三个未导出子元素零出现；生成的 `.i.ts` 为 `export { AgentInput } from …` / `export { AgentSessionView } from …`；代码侧闭包可达 7 名**不构成**「应自动含 7 条」的判据

#### Scenario: 透传条目带结构关键字即错

- **WHEN** 父 `$defs` 的透传条目同时写 `$ref` 与 `type: object` / `properties`
- **THEN** 写闸与装载校验各报 `ApiError`（透传条目必须是纯 `$ref` alias）；`apiShadowing` 亦报（与子元素类型名交集非空）

#### Scenario: 散文不再承担透传键集

- **WHEN** 父元素文档的散文列了 5 条透传导出名而 `$defs` 只声明 2 条
- **THEN** 零判据、零错误、零计数（散文无结构、名字清单唯一持有者是 schema）；该不一致属 prompt 纪律面，MUST NOT 被任何闸或判据抓取

#### Scenario: 跨层与跨 crate 引用各报其判据

- **WHEN** 分别构造「父接口 `$ref` 孙辈元素」与「父子元素分属两个 crate」
- **THEN** 前者报 `apiRefMissing`（非直接子元素），后者报 `apiNestedCrossCrate`；两者都点名涉事 FQN

## ADDED Requirements

### Requirement: bootstrap 双草稿（schema 骨架 + doc 骨架）

`api::bootstrap` SHALL 一次产出**两段**草稿文本（`{ schema, doc }`），两段都是「可直接落盘」的骨架，且都**不含任何段模板**：

- **schema 骨架** SHALL 含正确的 `$schema` / `$id`（= `<FQN>.schema.json`）/ `title`（= FQN），`properties` 由代码符号生成的占位项（每个 op 一项 `{ "x-function": { "description": "", "params": [...], "returns": … } }`；每个类型一项 `{ "$ref": "#/$defs/<名>" }`），`$defs` 由代码符号生成；`description` 占位为空串 `""` 而 MUST NOT 写「（待补）」一类提示语（空串与缺省语义等价，提示语会被 `description` 的读者当正文渲染）。写闸 MUST NOT 校验 `description` 是否非空（留空是合法中间态）。
- **schema 骨架 SHALL 可直接落盘并通过写闸**：它 MUST 满足 identity / 键空间 / 形态白名单 / 镜像 / `$ref` 前缀 / 零未知顶层键 / 复用三闸（含 `required ⊆ properties`）。MUST NOT 产 `api-reexport` 形态的占位（透传键值是设计判断，MUST NOT 以空条目冒充已声明）。
- **doc 骨架 SHALL 只有 H1 与一句指引**：正文恰为 `# <FQN>` + 一句「段落按内容自定，先例见同目录同类元素」；MUST NOT 给任何 `##` 段名、MUST NOT 给 fence、MUST NOT 给作者注块。
- **零写盘、零新出口**：两段只经既有两条出口流出（CLI stdout 单行 JSON / agent 工具 `{ schema, doc }`），MUST NOT 创建 / 覆盖 / 删除任何文件。
- **零参数与错误面不变**：`bootstrap` 的入参（`element`）、元素不存在 / 无实现入口 / 符号抽取失败的 `Err` 面逐字不变。

#### Scenario: schema 骨架可直接落盘并过写闸

- **WHEN** 对某有符号元素调 `bootstrap`，把返回的 `schema` 原样经 `write` 落到 `<models>/…/<FQN>.schema.json`
- **THEN** 写闸通过、落盘成功、响应含「接口已校验」；随后装载零错误、`present = true`

#### Scenario: doc 骨架零段名

- **WHEN** 取同一响应的 `doc` 段
- **THEN** 正文只有两行（`# <FQN>` 与一句指引）；`grep '^## '` 零命中；`grep '```'` 零命中

#### Scenario: 无符号元素仍出两份骨架

- **WHEN** 对入口存在但抽不到任何函数与类型的元素调 `bootstrap`
- **THEN** schema 骨架的 `properties` 与 `$defs` 均为空对象且可直接落盘（等价于留白形态）；doc 骨架仍只有 H1 + 一句指引

### Requirement: 接口定义文件的装载校验与写闸（零第二份校验）

接口定义文件的合法性与「接口已校验」SHALL 由**同一个函数**（`validate_schema(fqn, text) -> Result<Contract, ApiError>`）判定，两处消费只差错误映射：

```text
① 文本层：重复键扫描（serde_json 会把重复键折叠成「后者胜」，手写「同名两处定义」会静默）
② JSON 可解析
③ identity：$id == <文件名> / title == <FQN> / 文件主干 == <FQN>
④ 键空间：$defs ∪ properties 无重名（op id 与类型名不撞）
⑤ 形态白名单：properties.<k> 只允许 {x-function: …} 或 {$ref: "#/$defs/<名>"}，且后者 <名> ∈ $defs
⑥ 镜像：$defs 每个键在 properties 有指向自己的 $ref
⑦ $ref 前缀：跨元素串逐字等于 ref_relative(本FQN, 目标FQN)
⑧ 零未知顶层键：$schema / $id / title / $comment / $defs / properties 之外一律拒；
   $id / title / $defs / properties 四键 MUST 在场
⑨ 复用三闸：七族名字 ASCII / x-function 落点封闭二处 / required ⊆ properties
```

- **装载消费**：`load_contracts` 读盘后调本函数，失败 ⇒ `ApiError` **停摆**（MUST NOT 产半真报告、MUST NOT 跳过该元素继续）。
- **写闸消费**：`write` / `edit` 落 `<archi_models_dir>/**/<FQN>.schema.json` 时先调本函数，失败 ⇒ `Err` 且**零落盘**；成功 ⇒ 落盘并在响应追加「接口已校验」一行。路径判定以 sandbox 后的绝对路径落在 `<root>/<archi_models_rel_dir>/` 之下为准（目录前缀经 `foundation::layout` 派生，MUST NOT 行内字面量）；非该路径原样落盘、零校验。
- **文件级自洽，零跨元素**：本函数 MUST 只吃「本文件 + 本 FQN」。跨元素的一切（`$ref` 目标是否在模型内 / 是否直接子元素 / 目标是否 `api = none` / 目标文件在不在盘）仍归 `apiRefMissing` 既有判据面——写闸做它就变成第二份判据。唯一例外是 ⑦ 的**前缀**：它只需两个 FQN 做纯计算，故留在写闸。
- **错误面**：`Err` 的第一行自足（含文件 + 期望值 + 一条出路），MUST NOT 只写「非法」。`edit` 校验的是**替换后的正文**（不是替换前的旧文）。
- **零第二份实现**：MUST NOT 在 `write` / `edit` 侧另写一份 JSON 校验、MUST NOT 在装载侧再写一份；两处 MUST 调同一函数。

#### Scenario: 三类坏输入各一条 Err 且零落盘

- **WHEN** 分别以「截断的 JSON」「`$id` 与文件名不符」「`T` 同时作 op 键与 `$defs` 键」调 `write` 落 schema
- **THEN** 三次均 `Err`（文案分别点名解析位置、期望 `$id`、冲突键与两个键空间），盘上字节零变化（新建文件不产生、既有文件不被截断）

#### Scenario: 同源保证坏 schema 上不了盘

- **WHEN** 把写闸放行的合法 schema 手工改成坏 schema（绕过 `write`）后跑 `archi_drift`
- **THEN** 检查以 `ApiError` 停摆（文案与写闸共用同一判据、同一措辞族）；MUST NOT 产出「该元素跳过」的半真报告

#### Scenario: 非 schema 路径零校验

- **WHEN** 以 `write` 落一份普通 `*.ts` 或 `*.md`，内容含坏 JSON
- **THEN** 原样落盘、零校验、零「接口已校验」行（写闸射程只有 `<archi_models_dir>/**/<FQN>.schema.json`）

#### Scenario: edit 校验替换后正文

- **WHEN** 以 `edit` 把一份合法 schema 的 `title` 改成另一名字
- **THEN** `Err`（identity 闸），盘上字节零变化；改成正确 `title` 后落盘成功

## REMOVED Requirements

### Requirement: schema.json 与 INDEX.toml 为纯派生且不可手改

**Reason**: 真相搬移——`<FQN>.schema.json` 由生成物升为**手写真相**（第二份资产），“纯派生且不可手改”与本变更的核心语义相反；`INDEX.toml` 仍是纯派生生成物，其口径由同批 ADDED/MODIFIED 条文（「生成物入仓与零 diff 门挂载」「契约资产形态与唯一手写真相」）承接。旧条文里的两处具体口径（`$comment` 警示文本、`GENERATED_NOTICE`、`build_schema` 的逐字段装配表）随生成器一起退役。

**Migration**: 存量 11 份 `.schema.json` 的内容与对应 fence 逐项等价 ⇒ **零迁移**（不搬迁、不改写）。其 `$comment` 仍带旧「生成物、勿手改」文案，本变更不批量改写（它不参与任何判据；写闸不校验 `$comment`），由后续清理轮统一替换为「写者纪律」注解。新的写者纪律由 `bootstrap` 的 schema 骨架与 `x-function.description` 规范承载（见 MODIFIED「关键字集、类型映射与错误面」）。`INDEX.toml` 的「review 只读 md、schema diff 视为机械噪声」口径反转为「review 读 schema 与 md、`INDEX.toml` 与 `.i.*` 的 diff 视为机械噪声」——该句落 `INDEX.toml` 头注释。

### Requirement: bootstrap 草稿的六段骨架与惰性作者注

**Reason**: 六段骨架与尾部作者注是复盘 #1 的直接成因（模型把草稿当规格写）。真相搬移后 md 无结构，草稿再给段名等于把一条已 abolish 的约定重新推荐；作者注的三条 fence 指引也随 fence 退役而失效。

**Migration**: 由 ADDED「bootstrap 双草稿（schema 骨架 + doc 骨架）」取代：doc 骨架只给 H1 + 一句「段落按内容自定」，零段名零 fence 指引；写者引导迁到 schema 骨架的 `x-function.description` 空位与工具描述面。既有 11 份 md 的六段痕迹与历史 fence 不回收（Q3：清理轮处理），读面零解析。

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/mod.rs` | 契约面装配与出口 | `load_contracts` **只读 schema**（经 `validate_schema` 装配 `Contract`；schema 不在盘 ⇒ 跳过；坏 schema ⇒ `ApiError` 停摆）；`enabled(root, elements)` = 任一 `path` 非空 ∧ `api = declared` 元素 schema 在盘；`gating_findings` = `declared` ∧ schema 在盘；`ref_projection` 三路径**各自独立**（`contractPath` = doc 路径，在盘才出串）+ `status` 改判 schema 在盘；`bootstrap(root, element) -> Result<BootstrapDrafts, ApiError>`（两段文本）；`render_all` 只产 `INDEX.toml` / `.i.rs` / `.i.ts` |
| `crates/core/archi/src/api/contract.rs` | `Contract` 投影 + 复用三闸（**md 解析器退役**） | `parse` / `finish_section` / `parse_object*` / `string_end` / `decode_key` / `file_title` / `collect_reexport_prose` / `collect_surface_table` / `is_cjk` / `info_tokens` / `duplicate_key_error` 全删（约 600 行）；保留 `Contract` / `ContractOp` / `ContractType` + 新增 `Contract::from_schema`（解包 `x-function`、丢弃 `description`、`line = 0`）；保留 `duplicate_keys`（供写闸文本层）、`verify`（七族名字 / 落点 / `required ⊆ properties`，入参改判 schema 值）、`is_constructor`；`reexport_prose` / `sections` 字段删除；`FENCE_OP` / `FENCE_TYPES` 删除 |
| `crates/core/archi/src/api/schema.rs` | 接口校验单点 + 索引装配 | `build_schema` 退役；`verify_defs` 零改造保留；新增 `verify_identity` / `verify_keyspace` / `verify_shape` / `verify_mirror` / `verify_ref_prefix` / `verify_top_level_keys`；唯一公开入口 `validate_schema(fqn, text) -> Result<Contract, ApiError>`（九道闸一趟）；`build_index` 的 `present` 改判 schema 在盘、头注释改写「手写 / 生成分界 + review 口径反转」；`DEGRADED_KINDS` 与 `degraded_marks` 逐字不变 |
| `crates/core/archi/src/api/paths.rs` | 落点派生 | `carrier_path` / `doc_path` **退役**（资产① 落点恒为派生 `<FQN>.md`；`doc` 退出派生面）；`contract_path` / `schema_path` / `index_path` / `entry_state` / `ref_relative` / `generated_path` 逐字不变 |
| `crates/core/archi/src/api/diff.rs` | 三类差异比对 + 孤儿扫描 | `api_dir_artifacts` 收面**只留** `<archi_graph_dir>/INDEX.toml`（`*.schema.json` 退出枚举）；`orphaned` / `mismatched` / `missing` 三列语义不变 |
| `crates/core/archi/src/api/check.rs` | 十判据单点求值 | `none` 支 2 步（`apiUnexpected` → `docMissing`）、`declared` 支 4 步（`docMissing` → `apiEntryMissing` → `apiMissing` → 一致性；③ 判派生 md，schema 不在盘 ⇒ ⑤⑥ 零对象、不产判据）；`doc_facts` 收成 `doc_present: bool`；删 `apiReexportDrift` / `apiDocMissing` / `docNotFound` 三支与 `FENCE_*` 依赖 |
| `crates/core/archi/src/drift/report.rs` | 判据词汇 | `ArchiApiFindingKind` **十值**（`ALL: [Self; 10]`）；`severity` 的 `error` 恰 `apiMissing` / `apiSignatureMismatch`；`category` 七值（`doc` 只剩 `docMissing`） |
| `crates/core/archi/src/api/emit/**` / `extract/**` | 渲染面 / 取数面 | **逐字不变**（输入 `Contract` 形状不变 ⇒ `.i.rs` / `.i.ts` 逐字节不变的机械前提） |
| `crates/tools/archi-api-gen/src/main.rs` | 唯一 CLI 出口 | `generate` 写三类生成物、`--check` 零写入、`bootstrap` 打印 `{ schema, doc }` 单行 JSON；子命令面与退出码语义不变 |
| `.dev-team/architecture/models/**` | 资产面（**零正文改动**） | 11 份 `.md`（文档）+ 11 份 `.schema.json`（接口）原样；schema 由生成物转为手写真相但内容零改；`$comment` 旧文案留清理轮 |
| `AGENT.md` + `CLAUDE.md` | 仓库根纪律入口 | 「模块 API 契约」节恰 5 条要点重写（三资产分工 / 三类生成物 / 两脚本段序 / 留白 = 空 schema / 纳管口径）；两份节内正文逐字相同；零判据长表、零 fence 示例、零 `openspec/specs/` 路径 |

### Function: `crates/core/archi/src/api/schema.rs`

| 函数 | 变化 | 语义 |
|---|---|---|
| `validate_schema(fqn: &str, text: &str) -> Result<Contract, ApiError>` | 新增（唯一公开入口） | 九道闸一趟走完并产出 `Contract`；`write` / `edit` 与 `load_contracts` 共用（只差错误映射） |
| `verify_identity(fqn, value)` | 新增 | `$id` / `title` / 文件主干三者与 FQN 逐字一致 |
| `verify_shape(value)` | 新增 | `properties.<k>` 二选一形态（`x-function` / `$ref` 且目标在 `$defs`） |
| `verify_mirror(value)` | 新增 | `$defs` 每键在 `properties` 有指向自己的 `$ref` |
| `verify_ref_prefix(fqn, value)` | 新增 | 跨元素 `$ref` 前缀逐字等于 `ref_relative(from, to)` |
| `verify_top_level_keys(value)` | 新增 | 顶层键封闭六项；`$id` / `title` / `$defs` / `properties` 必在场 |
| `build_schema` | **删除** | 真相方向倒置后无产物 |
| `verify_defs` / `degraded_marks` / `build_index` | 保留（`build_index` 改 `present` 与头注释） | 逐字沿用 |

### API: 生成物驱动面（三条命令串零变化，产物集少一类）

| 项 | 变化 | 描述 |
|---|---|---|
| `packages/desktop/package.json` | 零改动 | `api:export` / `api:check` / `check` 三条命令串逐字不变；`version` 相对 `HEAD` +1 patch |
| `crates/core/archi/src/api/{render,artifact_check}` | 零改动 | `workspace_render` / `artifact_check` 的签名与装配不变（应生成集内部由 `render_all` 收窄） |
