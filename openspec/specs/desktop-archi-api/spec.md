# desktop-archi-api Specification

## Purpose

定义模块资产的三份分工与生成链：`<archi_models_dir>/<分层目录…>/<叶段>.md` 写理由（无结构文档）、同目录 `<叶段>.schema.json` 写接口（**唯一凭据、手写真相**，装载与写闸共用 `validate_schema` 九道闸），`INDEX.toml` / `.i.rs` / `.i.ts` 为三类生成物；加上引用与命名规则、关键字集与类型映射、生成物落点与入口查找、零 diff 门挂载、仓库根契约纪律节、语言无关纪律、`verify` 闭环与 `bootstrap` 双草稿、模块嵌套与选择性透传、TS 导出闭包取数面。

## Requirements

### Requirement: 契约资产形态与唯一手写真相

模块资产 SHALL 按**三份分工**落位，每份只回答一个问题，MUST NOT 相互兼答：

| # | 资产 | 载体 | 结构 | 回答的问题 |
|---|---|---|---|---|
| ① | 文档 | `<archi_models_dir>/<分层目录…>/<叶段>.md` | **无结构**（段落自定） | 为什么这样切、有什么坑、什么不归它管 |
| ② | 接口 | `<archi_models_dir>/<分层目录…>/<叶段>.schema.json` | JSON Schema（强结构） | 这条边跨过去时接触面长什么样 |
| ③ | 拓扑 | `<archi_graph_model_dir>/NN-<域>.toml` | TOML（强结构，见 `repo-archi-model`） | 谁在谁的里面、谁依赖谁 |

`<archi_dir>` 经 `foundation::layout::archi_dir`、资产目录取既有 `foundation::layout::archi_models_dir`（MUST NOT 行内字面量、MUST NOT 新增第二个目录派生函数）。`<分层目录…>` SHALL 是元素 FQN 的**祖先链镜像**（取 FQN 段链去掉叶段后的逐段路径：`Backend.Infra.ArchiRuntime` → `Backend/Infra/`），**文件名 SHALL 只取叶段**（`ArchiRuntime.md` / `ArchiRuntime.schema.json`——祖先链已由目录承载，再写进文件名会让同一段前缀在路径里累计两次并顶到 Windows `MAX_PATH`），元素身份仍是 FQN（目录 + 叶段拼回），目录只是布局。

- **接口的唯一凭据是 `<叶段>.schema.json`，零 md 回落**：读面（装载 / 判据 / 生成物投影）SHALL 只从 schema 取接口事实，MUST NOT 解析 md 的 fenced json；schema 不在盘 ⇒ 该元素「接口未写」（`present = false`，无判据）；schema 在盘但装载校验失败 ⇒ `ApiError` **停摆**（与模型结构错同族，MUST NOT 产出半真报告）。`schemaInvalid` / `schemaMissing` 两条判据不在本期（后续 change），本期以「停摆 / 跳过」表达。
- **文档无结构**：`<叶段>.md` MUST NOT 被任何解析器赋予段语义——零 `##` 段名白名单、零段数约束、零 fence 义务、零 H1 校验。`bootstrap` 的 doc 草稿给出 H1，但它不被任何闸校验。文档是**产品面**：架构页右栏「文档」段直接把路径交给 `FileViewerBlock` 渲染给用户读（人 + AI 共读），故它 MUST 可读、MUST 只留语义（职能 / 边界 / 不变量 / 承载理由 / 坑）。
- **名字清单的唯一持有者是 schema**：导出名 / 签名 / 类型形状的唯一持有者为 `properties` 与 `$defs` 的键。文档 MUST NOT 枚举导出名（含表格、列表、裸正文三种形态）——该纪律**只有 prompt 与知识文本承载，零机械闸**（闸随 md 解析器一起退役）。历史 `api-op` / `api-types` / `api-reexport` fence SHALL 被当作**普通文档内容**：读面零解析、`archi_api check` 零新增计数、MUST NOT 设 `legacyFenceFiles` 一类读数；其清理归专门一轮，MUST NOT 为「fence 是否残留」新增判据。
- **`api` 字段只表达资产②的存在意图，二值**：`declared`（承诺有接口）/ `none`（显式无接口面）。MUST NOT 新增第三值（如 `deferred`）。
  - `api = none` 而 `<叶段>.schema.json` 在盘 ⇒ `apiUnexpected`（留白被违反）。
  - `api = declared` 而模型写了 `doc` ⇒ **不检查**（`doc` 字段零读者，见下）。
  - 留白（有导出却无对外约定）的**正规形态**是 `api = declared` + 空 schema（`$defs {}` + `properties {}`）——它是「看过、确实没有要声明的接口」的机器可读版本，与「还没写（schema 缺）」严格可分。
- **`doc` 字段保留但零判据读者**：判据面 / 纳管面 / 路径派生面 / 检索语料面都不读它（指不到盘不报、写了不报、`api = declared` 元素写了也不报）；保留是为了外部工作区写了 `doc` 不突然变成 `unknownField` 结构错。`api = none` 元素的入口文档仍可由它声明，`archi_module` 会照原样投影、画布「入口文档」段照旧渲染——那是呈现事实，不是判据。
- **接口面启用判据（换面）**：`api::enabled` SHALL 判「存在任一 `path` 非空 ∧ `api = declared` 元素的 `<叶段>.schema.json` 在盘」。MUST NOT 退化为「`<archi_models_dir>` 递归任意 `*.md` / `*.schema.json`」——非任何元素载体的散落文件 MUST NOT 构成启用证据；`api = none` 元素在盘的 schema 是违约事实（`apiUnexpected`），MUST NOT 构成启用证据。该判据与判据面同源，MUST NOT 在命令层另判一套。
- **不接入文档关联面**：资产 MUST NOT 成为任何「文档关联来源」（`explicit` / `adjacent` / `text` 三源已退役，MUST NOT 复活为第四种），MUST NOT 新增 `kind = api` 一类形态标记。正文抵达路径只有三条：`api.contractPath`（doc 路径）/ `api.schemaPath` / 通用 `read_file` 按路径自取；检索面只收路径串、零正文。
- **与模型分目录**：`<archi_models_dir>` SHALL 只容纳文档与接口两份资产；模型声明在 `<archi_graph_model_dir>`。两面的分离判据是**目录**（模型收面只扫 `graph/model/` 一级 + `.toml`），MUST NOT 引入目录级豁免清单。
- **不依赖语言概念**：资产形态、目录布局、索引中 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` 一类语言专有概念——语言特定部分只落取数器（`api/extract/**`）与渲染器（`api/emit/**`）。
- **`INDEX.toml` 的 form status 值域不动**：`declared` / `absent` 两词保留，`present`（`declared` 的判定）改判「schema 在盘」；命名债（`declared` 读起来像模型意图）并入清理轮，MUST NOT 在本变更改值域（改值域会让 `INDEX.toml` 全条目 diff，破坏「生成物逐字节不变」的验收锚）。
- **统一目录（不跟代码走）**：契约 MUST NOT 落元素主目录。审计型问题（「这批改动动了哪些模块的契约」）只有统一目录能答（`git diff --stat <archi>/models/<分层目录…>/` 即列出），且无替代物；伴随型问题（「我改的这行契约同步了吗」）由门禁机械回答，不需要文件相邻。代价 SHALL 显式留痕：在 crate 里读代码时看不到契约，只能经画布 / `archi_module` / 门禁失败提示 / 全局搜索四条路之一。
- **派生前置（消费方约束）**：读 `schema.json` 的工具 SHALL 以 **draft 2020-12 校验器**配合 `strict: "log"` 或 `false`（`x-function` / `x-common` / `x-class` 是未知关键字，strict 模式必然报错），并 SHALL 先按 `$id` 逐个注册被引用 schema（跨文件 `$ref` 是结构性的，`schema.json` 不自包含）。

#### Scenario: 三份资产各答一个问题且同目录落位

- **WHEN** 审查 `Backend.Infra.ArchiRuntime` 的三份资产
- **THEN** `Backend/Infra/Backend.Infra.ArchiRuntime.md` 是散文（任意段名 / 段数均可）、`Backend/Infra/Backend.Infra.ArchiRuntime.schema.json` 是接口真相（`properties` / `$defs` 键即导出名）、`graph/model/*.toml` 只含元素与关系；MUST NOT 出现「md 内 fenced json 被当作接口真相」的读取路径

#### Scenario: 文档零结构与历史 fence 惰性

- **WHEN** 某文档有 10 个自定 `##` 段 + 两张表格 + 两个历史 `api-op` fence，另一文档只有 H1 与一段散文
- **THEN** 两者装载路径完全相同（零解析、零错误、零判据、零计数）；`archi_api check` 的响应不含任何「fence 残留」计数键

#### Scenario: 启用判据与散落文件

- **WHEN** 观察根下 `models/notes.md`（非任何元素载体）与 `models/Backend/notes.schema.json`（非任何元素的落点）存在，而全部 `declared` 元素都无 schema
- **THEN** `enabled = false`、`apiFindings = []`；随后给任一 `api = declared` 元素补一份合法 schema ⇒ `enabled = true`

#### Scenario: 留白是空 schema 而非缺文件

- **WHEN** 某 `api = declared` 元素的 schema 为 `{"$defs": {}, "properties": {}}`，另一同类元素无 schema
- **THEN** 前者 `present = true` 且零判据（这是「已看过、无对外约定」：空清单是**已声明**的空）；后者 `present = false` ⇒ 「接口未写」**零判据**（`schemaMissing` 归后续 change，本期不以任何别名预铺）——两者 MUST NOT 被压成同一读数（空 schema 与缺文件是两个不同事实，`docMissing` 只判派生 md）

#### Scenario: none 元素带 schema 即违约

- **WHEN** 某 `api = none` 元素目录下存在 `<叶段>.schema.json`
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
- **跨元素 `$ref` SHALL 用相对文件路径**：同目录 `./<叶段>.schema.json#/$defs/<name>`；跨目录的前缀由两侧分层目录逐段求出（先上溯到公共祖先、再逐段下行），由**单点函数 `paths::ref_relative(from_fqn, to_fqn)`** 产出，MUST NOT 手算、MUST NOT 用自定义 URI scheme（实测 ajv 对 `module://X#/$defs/Y` 报 `can't resolve reference`，只认标准 URI）。
- **跨元素 `$ref` 的前缀 SHALL 被逐字校验**：文件部分的 FQN 由**本元素分层目录 + 相对前缀**解出（文件名只剩叶段，单看串已不知道祖先链），再逐字比对规范形 `ref_relative(本FQN, 目标FQN) + <目标叶段>`；解不出（越界 / 带点的段）与不逐字相等都判 `ApiError`。少一个 `../` 会绕过 `apiRefMissing` 并在 `.i.ts` 里 import 一个不存在的路径、到编译期才炸。校验点 SHALL 是写闸与装载校验的**同一个** `validate_schema`（见 ADDED「接口定义文件的装载校验与写闸」），文案 SHALL 给出期望串。
- **透传 alias 形态**：`$defs` 的透传条目 SHALL 是纯 `$ref` alias（值指向**直接子元素**的 schema 文件 + `#/$defs/<name>`），MUST NOT 携带结构关键字（`type` / `properties` / `enum` / `const` / `items` / `oneOf`）——携带即沦为重复定义。透传目标 FQN 由本元素分层目录 + 相对前缀反查（落点即身份）。
- **`$id` / `title` / 落点三者一致**：`$id` MUST 等于 `<FQN>.schema.json`、`title` MUST 等于 FQN、落点（分层目录段 + 叶段主干）反查 MUST 等于 FQN；任一漂移即 `ApiError`（三处持有同一元素身份，漂移会让反查与报告文案指向不同元素）。

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

接口定义 SHALL 只用标准 JSON Schema 关键字加**五个**扩展构造（`x-` 面封闭，MUST NOT 自造新的 `x-` 前缀键）：

| 关键字 | 语义 | 生成 |
|---|---|---|
| `object` / `enum` / `const` / `default` / `oneOf` / `$ref` | 标准关键字 | 见映射表 |
| `x-class` | 声明为 trait，**仅允许方法**（出现属性即 `ApiError`）；MUST 带 `type: "object"`、MUST NOT 带 `additionalProperties` | `pub trait` / `export interface` |
| `x-function` | 函数签名：`params` / `returns` / `async` / `owner` / `description` | `pub fn` / `export type Fn = (…) => R` |
| `x-common` | 自由注记（`unit` / `note` / `since` / `owner` / `tag`）+ `degraded`（取值受封闭四值词表校验） | 不变换 |
| `x-constructor` | **仅 `x-class` 条目**：规范构造入口，单个对象 `{ "x-function": { "params": [ … ] } }`，名字固定 `new`，`returns` 禁止 | trait body `fn new(…) -> Self` / 值声明的 `new (…)` |
| `x-static` | **仅 `x-class` 条目**：「名字 → `{ "x-function": { … } }`」类型级静态方法表，名字非 `new` 且并入方法名族，返回本类型即工厂 | trait body `fn <名>(…) -> R` / 值声明的 `<名> (…)` |

- **`x-function` 的落点 SHALL 封闭四处**：`properties.<k>.x-function`（模块操作）、`$defs.<T>.properties.<k>.x-function`（函数指针字段）、`$defs.<T>.x-constructor.x-function`（构造入口）与 `$defs.<T>.x-static.<名>.x-function`（静态方法，后两处只在 `x-class` 条目内可达）；其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 及其更深层、`x-function` 自身子树）即 `ApiError` 并点名 JSON pointer。位置类错误的正确处置是**搬位置**，不是在渲染期兜底。
- **`properties.<k>` 的形态 SHALL 封闭二选一**：`{ "x-function": { … } }` 或 `{ "$ref": "#/$defs/<名>" }` 且 `<名> ∈ $defs`。其余形态（内联 `object` / 数组 / `oneOf` / 裸 `type`）即 `ApiError`——内联 object 会经 `schema_type` 的 type 分派静默退化为 `Record<string, unknown>`（编译通过、类型全丢），这是本闸存在的唯一理由。「想复用类型就放进 `$defs`，想声明操作就用 `x-function`」。
- **`x-function` 内层是自由形状**：闸只判外层形态，MUST NOT 对 `x-function` 内层（`async` / `description` / 将来可能有的 `deprecated`）做未知键校验（`x-constructor` 的 `returns` / `async` 例外已由「类型的构造入口与静态方法」定形，属**语义**约束而非未知键闸）。
- **op 的散文说明 SHALL 落在 `x-function.description`**：写**调用方要知道的语义**（它做什么 / 边界在哪 / 为什么），MUST NOT 写实现细节、MUST NOT 复述签名（签名就在同一处，复述是第二事实源）。`Contract::from_schema` 装配 `ContractOp.signature` 时 SHALL **解包** `x-function` 并**丢弃** `description`（`emit` 只读 `params` / `returns` / `owner`，渲染无关字段不进 emit 输入）。
- **`$defs.<T>.description` 承载类型语义**；`$comment` 只承载**写者纪律**（人读、不生成），MUST NOT 被任何判据读取。
- **注解载体三选封闭**：`description`（进 `.i.*` 文档注释的语义）/ `$comment`（只在源文件）/ `x-*`（只在本表**五项**内）。JSONC 行注释 MUST NOT 采纳（解析器多一份真相）。
- 映射表（`object` → `struct` / `interface`；`enum` → Rust `enum` + TS `as const` + `type`；`oneOf`/`anyOf` → 带负载 `enum` / union；`x-class` → `pub trait` / `export interface`；`x-constructor` / `x-static` → trait body 的 `fn new(…) -> Self` / `fn <名>(…) -> R` 与同名的值声明（语义定义归「类型的构造入口与静态方法」，写法属生成器决策）；标量 → newtype / 别名；非 `required` 字段 → `Option<T>` / `{field}?: T`；显式 null 三分支 → `Option<T>` / `T | null`；`x-function` → `pub fn` / `(…) => R`；错误面 `Outcome` 信封 → `Result<T, String>` / union 或 `Promise` reject）与两条位置分派纪律 SHALL 逐条不变。
- **可选与可空两轴、纯 null 档**、**数组元素位可空加括号**、**标量必 newtype 不 trait**、**构造器名 MUST NOT 作模块 op**（模块级 `properties` 键出现 `new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open` 即 `apiConstructorDeclared`；**类型级构造改走 `$defs.<T>.x-constructor`**，`$defs` 条目的 `properties.new` 同样被拒但与模块面判据分立）、**枚举列举由函数承担**、**泛型不支持**、**TS 侧不做品牌类型**、**`common.json` 只承载映射规则** —— 除「构造器」一条按上述改写外，其余各条逐字沿用。
- **两道人字闸（改写落点，判据内容不变）**：① **装载期**（`validate_schema` 同一实现）**七族名字**——`properties` 键 / `$defs` 键 / 透传键 / `x-function.params[].name` / `$defs` 条目的第一层 `properties` 键 / `x-function.owner` / `$ref` 末段名——MUST 是 ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*`，不成立即 `ApiError` 并点名族别与 pointer（`x-constructor` 的名字固定 `new`、不设槽位；`x-static` 的键并入「方法名」族，族别数仍七）；同趟遍历另判 `x-function` 落点、`required ⊆ properties` 与 `x-class` 的骨架四判据（见「类型的构造入口与静态方法」）。② **渲染期**一切来自契约的字符串字面量与注释 MUST 经该语言单点转义；`emit` 内零标识符判据、两侧 `value_type` 零 `x-function` 分支是它的机械判据。
- **四条明确不做**逐字沿用（`x-interface` / `x-nominal` / `x-singleSource` 独立构造、自造协议记法、L/S 双层锁、Zod 作通用载体）。

#### Scenario: 内联 object 在写闸即被拒

- **WHEN** `write` 落一份把 `properties.send` 写成 `{"type": "object", "properties": {...}}` 的 schema
- **THEN** `Err` 零落盘，文案给出二选一形态与「想复用类型就放进 `$defs`」的出路；该内联形态 MUST NOT 抵达 `emit`（否则 TS 侧静默退化为 `Record<string, unknown>`）

#### Scenario: 函数指针字段与模块操作可辨

- **WHEN** schema 同时含 `properties.parse` 的 `x-function` 与 `$defs.Widget.properties.parse` 的 `x-function`
- **THEN** 两处落点均合法（封闭四处中的前两处）；前者生成为模块级 `pub fn`，后者生成为 `pub parse: fn(&str) -> …` 字段，互不吞并

#### Scenario: `x-function.description` 进契约但不进 emit

- **WHEN** 某 op 的 `x-function` 带 `description` 与 `returns`，装载后取 `ContractOp.signature`
- **THEN** `signature` 恰为解包后的 `{params, returns, owner?}`（`description` 已丢弃）；`.i.rs` 的签名见证与不带 `description` 时逐字节相同

#### Scenario: `x-class` 带属性即报错

- **WHEN** 某 `$defs` 条目标 `x-class: true` + `type: "object"`，但同时声明 `properties: { "count": { "type": "integer" } }`
- **THEN** `ApiError`（trait 仅允许方法）；去掉 `x-class` 改为普通 `object` 后通过

#### Scenario: 自造 `x-` 键被拒

- **WHEN** schema 顶层出现 `x-note` / `x-owner` 一类新前缀键，或 `properties.<k>` 出现 `x-deprecated`
- **THEN** 前者触发「零未知顶层键」`ApiError`（文案列封闭六项与 `x-` 面五项）；后者触发形态白名单 `ApiError`（`properties.<k>` 只允许 `x-function` 或 `$ref`）

#### Scenario: 构造器声明即契约自身矛盾

- **WHEN** schema 的**模块级** `properties` 出现键 `new`（带 `x-function`）
- **THEN** 报 `apiConstructorDeclared` 并点名该 op；删掉该键后模块级不再出现 `new()`，构造入口改由 `$defs.<T>.x-constructor` 声明（零字段 object 条目仍自动产 `new()`）

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

**三类**生成物 SHALL 全部入仓并进零 diff 门，形态沿既有 `bindings.ts` 先例（MUST NOT 另造第二套守卫机制）：

| 生成物 | 落点 | 入库 | 门禁 |
|---|---|---|---|
| `INDEX.toml` | `<archi_graph_dir>/`（全局唯一一份） | 是 | `api:check` 零 diff |
| `<入口主干>.i.rs` | **元素实现入口同级**（`crates/infra/archi/src/lib.i.rs`） | 是 | `api:check` 零 diff + `cargo check` |
| `<入口主干>.i.ts` | **元素实现入口同级**（`views/welcome-view.i.ts`） | 是 | `api:check` 零 diff + `tsc --noEmit` |

- **`<叶段>.schema.json` 不再是生成物**：它是手写真相（第二份资产），MUST NOT 进 `api:check` 的应生成集、MUST NOT 进 `orphaned` 列、MUST NOT 由任何生成器写出。它同样入库（git 基线是人工 review 与审计的入口）。
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

- **WHEN** 手改一份已提交的 `<叶段>.schema.json`（合法改动）后执行 `api:check`
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

- 该节 SHALL 恰含**五条要点**：① **三资产分工与落位**——`<archi_models_dir>/<分层目录…>/<叶段>.md` 写理由（文档，无结构）、同目录 `<叶段>.schema.json` 写接口（唯一凭据），附一句「文件名只取 FQN 叶段（祖先链由分层目录承载）」与一句「文档 MUST NOT 枚举导出名（名字清单的唯一持有者是 schema）」；② **三类生成物勿手改**（`INDEX.toml` 在 `<archi_graph_dir>`、**实现入口同级**的 `<入口主干>.i.rs` / `<入口主干>.i.ts`）与「已提交手改即零 diff 门红、未提交即被静默覆盖」；③ `api:export` / `api:check` 两脚本与 `check` 链的段序；④ `api = none` 是显式留白而非「还没写」，留白的正规形态是空 schema（`$defs {}` + `properties {}`）；⑤ **纳管口径**——`api = declared` ∧ schema 在盘的元素，其判据影响 `status`（`error` 档还进 `passed`），文档债（`docMissing`）恒 `warn`、永不纳管；索引与计数经 `<archi_graph_dir>/INDEX.toml` 查看。
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

### Requirement: 语言无关纪律与语言特定落点

契约资产层 SHALL 与语言无关，语言特定能力 SHALL 只落取数器与渲染器：

```
语言无关    models · api 契约 · docs · graph · 漂移与契约判据 · FQN · gate
────────────────────────────────────────────────────────────────
语言特定    api/extract/{rust,ts}.rs（取数器，仅 bootstrap）· api/emit/{rust,ts}.rs（生成物投影）
```

- `crate` 是 Rust 工具链概念，架构层 MUST NOT 依赖：`dto::ElementKind` 无语言字段的原则已被遵守，契约格式 MUST NOT 重复 models 注释里的历史错误（`02-backend-core.toml:2` 写「Cargo `[dependencies]`」、`06-frontend.toml:3` 写「import 证据」）。
- `<叶段>.md` / `<叶段>.schema.json` / `INDEX.toml` 内 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` / `TypeScript` 一类语言专有词（`x-common.note` 里说明具体语言的实现细节除外，且须显式标注为注记）。
- **生成物按模块落盘，MUST NOT 按 crate 聚合**：按 crate 聚合会让架构层依赖语言概念，且 diff 粒度串味。
- **跨语言语义标注**：语言能力缺失 SHALL 经 `x-common.degraded` 显式留痕，取值 MUST 来自**封闭四值词表**（`foreignElement` / `noElement` / `unrepresentable` / `crossLanguage`，见「降级词表与真实债表」）；旧口径的两类（错误通道走 `Promise` reject、TS 结构化类型无法强制实现）分别是该词表里的 `unrepresentable` 与 `crossLanguage` 两值，MUST NOT 作为并列的第二套分类并存。降级是机制属性，MUST NOT 靠散文掩盖。

#### Scenario: 资产面零语言词

- **WHEN** 审查三份手写 / 生成资产（`.md` / `.schema.json` / `INDEX.toml`）的全部文本
- **THEN** 零 `crate` / `Cargo.toml` / `Rust` / `TypeScript` 词（注记内标注为注记的语言说明除外）；语言词只出现在 `api/extract/**` 与 `api/emit/**` 的源码注释里

#### Scenario: 降级词表封闭且可枚举

- **WHEN** 取 `INDEX.toml` 的 `degraded` 计数与各契约内全部 `x-common.degraded` 出现点
- **THEN** 取值只来自四值词表（逐值可枚举）；词表外值与非字符串值在装载期即契约错误（文案并列四值）；新增第五值 MUST 改本 requirement 与「降级词表与真实债表」，MUST NOT 自由发挥

### Requirement: 生成器、verify 闭环与 bootstrap 剪枝

`verify` SHALL 定义为机械判定而非独立模式：**`generate`（`--check` 零写入，比对在工具内完成）+ 语言编译器**，另加**写闸**（接口文件上盘即合法）。MUST NOT 引入归一函数、双层锁或语义 diff 比对器。

- **判定在 core，写盘在工具 crate**：接口文件的装载校验、生成物渲染、判据求值 SHALL 全部落 `crates/core/archi/src/api/**`（纯读、零写盘、零 spawn）；唯一写盘面 SHALL 为 `crates/tools/archi-api-gen`（CLI）与 `crates/infra/archi` 的 `write_artifacts` 单点。
- **CLI 三子命令封闭**：`generate`（写**三类**生成物：`INDEX.toml` / `<入口主干>.i.rs` / `<入口主干>.i.ts`）、`--check`（生成到内存后与磁盘比对，零写入，差异即非零退出并列文件名）、`bootstrap`（输出**双草稿**到 stdout，不写盘）。`generate` SHALL 幂等。MUST NOT 让 `generate` 写 `.schema.json`（它是手写真相）。
- **`bootstrap` 的出口形状**：`api::bootstrap(root, element)` SHALL 返回两段文本（schema 骨架 + doc 骨架）；CLI 与 agent 工具两条出口 SHALL 出**同一份**内容（CLI 打印单行 JSON `{ "schema": "…", "doc": "…" }`，工具回同名对象）。
- **`verify` 覆盖不到的三件事 SHALL 显式列出**：bootstrap 方向（代码 → 接口草稿，一次性迁移工具，非常态门禁）、跨模块语义不变量（散文 + 专项测试）、生成器自身正确性（生成器自己的单测）。
- **bootstrap SHALL 走 AST，MUST NOT 正则抽签名**（Rust 多行签名占 30–62%，`grep` 单行抽取会截半）；TS 侧只判 `export function`（实测 `export const` 里零箭头函数）。
- **`visibility` 封闭四值**（`external` / `crateOnly` / `unused` / `testOnly`）与其本期值域留白（TS 生产面只产 `external`，`unused` 降级接口零调用者）逐字沿用；MUST NOT 为 TS 造第二套死代码扫描。
- **试点先行与近似误差留痕**逐字沿用。

- **试点先行**：bootstrap 的首次落地 SHALL 只做**一个**元素（`Backend.Infra.ArchiRuntime`，14 个 crate 元素里公开面最小的 2 项），由人工或 agent 删除伪公共面形成初版接口定义。
- **bootstrap 的近似误差 SHALL 如实留痕**：glob 导入（`use a::*`）、同名符号、宏生成引用都会让「有引用者」判定偏宽——这些是 bootstrap 的已知近似，MUST NOT 被伪装成精确判定；契约侧以「调用方要知道的语义」与 `x-function.description` 补足，不得以取数近似为由放宽判据。

#### Scenario: verify 三件套闭环

- **WHEN** 改动代码签名但不重生成生成物
- **THEN** `api:check` 报差异并以非零退出（点名文件）；重生成后零 diff；若改动与接口定义不符则编译器报错

#### Scenario: `--check` 零写盘

- **WHEN** 以 `--check` 在有过期生成物的仓库上运行
- **THEN** 磁盘字节零变化（含生成物、`INDEX.toml`），退出码非零、输出列出差异文件

#### Scenario: generate 不产接口文件

- **WHEN** 删除一份 `<叶段>.schema.json` 后执行 `api:export`
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
| 父 `<叶段>.schema.json` 的 `$defs` | 本地定义 **+ 透传项（纯 `$ref` alias，目标 = 直接子元素 schema + `#/$defs/<name>`）** | 机器检查 + 生成物（悬空引用编译期即报） |
| 父 `<叶段>.md` 散文 | 透传意图（人读，MUST NOT 枚举名字清单） | 人读 |

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


### Requirement: bootstrap 双草稿（schema 骨架 + doc 骨架）

`api::bootstrap` SHALL 一次产出**两段**草稿文本（`{ schema, doc }`），两段都是「可直接落盘」的骨架，且都**不含任何段模板**：

- **schema 骨架** SHALL 含正确的 `$schema` / `$id`（= `<FQN>.schema.json`）/ `title`（= FQN），`properties` 由代码符号生成的占位项（每个 op 一项 `{ "x-function": { "description": "", "params": [...], "returns": … } }`；每个类型一项 `{ "$ref": "#/$defs/<名>" }`），`$defs` 由代码符号生成；`description` 占位为空串 `""` 而 MUST NOT 写「（待补）」一类提示语（空串与缺省语义等价，提示语会被 `description` 的读者当正文渲染）。写闸 MUST NOT 校验 `description` 是否非空（留空是合法中间态）。
- **schema 骨架 SHALL 可直接落盘并通过写闸**：它 MUST 满足 identity / 键空间 / 形态白名单 / 镜像 / `$ref` 前缀 / 零未知顶层键 / 复用三闸（含 `required ⊆ properties`）。MUST NOT 产 `api-reexport` 形态的占位（透传键值是设计判断，MUST NOT 以空条目冒充已声明）。
- **doc 骨架 SHALL 只有 H1 与一句指引**：正文恰为 `# <FQN>` + 一句「段落按内容自定，先例见同目录同类元素」；MUST NOT 给任何 `##` 段名、MUST NOT 给 fence、MUST NOT 给作者注块。
- **零写盘、零新出口**：两段只经既有两条出口流出（CLI stdout 单行 JSON / agent 工具 `{ schema, doc }`），MUST NOT 创建 / 覆盖 / 删除任何文件。
- **零参数与错误面不变**：`bootstrap` 的入参（`element`）、元素不存在 / 无实现入口 / 符号抽取失败的 `Err` 面逐字不变。

#### Scenario: schema 骨架可直接落盘并过写闸

- **WHEN** 对某有符号元素调 `bootstrap`，把返回的 `schema` 原样经 `write` 落到 `<models>/…/<叶段>.schema.json`
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
③ identity：$id == "<FQN>.schema.json" / title == <FQN> / 落点（分层目录段 + 叶段主干）反查 == <FQN>
④ 键空间：$defs ∪ properties 无重名（op id 与类型名不撞）
⑤ 形态白名单：properties.<k> 只允许 {x-function: …} 或 {$ref: "#/$defs/<名>"}，且后者 <名> ∈ $defs
⑥ 镜像：$defs 每个键在 properties 有指向自己的 $ref
⑦ $ref 前缀：跨元素串逐字等于 ref_relative(本FQN, 目标FQN)
⑧ 零未知顶层键：$schema / $id / title / $comment / $defs / properties 之外一律拒；
   $id / title / $defs / properties 四键 MUST 在场
⑨ 复用三闸：七族名字 ASCII（含 x-static 名）/ x-function 落点封闭四处 / required ⊆ properties
   + $defs 的 x-class 骨架判据（type: "object" / 无 additionalProperties / 无 required /
     properties 全为方法 / 预留名不撞），判序即错误面
```

- **装载消费**：`load_contracts` 读盘后调本函数，失败 ⇒ `ApiError` **停摆**（MUST NOT 产半真报告、MUST NOT 跳过该元素继续）。
- **写闸消费**：`write` / `edit` 落 `<archi_models_dir>/**/<叶段>.schema.json` 时先调本函数，失败 ⇒ `Err` 且**零落盘**；成功 ⇒ 落盘并在响应追加「接口已校验」一行。路径判定以 sandbox 后的绝对路径落在 `<root>/<archi_models_rel_dir>/` 之下为准（目录前缀经 `foundation::layout` 派生，MUST NOT 行内字面量）；非该路径原样落盘、零校验。
- **文件级自洽，零跨元素**：本函数 MUST 只吃「本文件 + 本 FQN」。跨元素的一切（`$ref` 目标是否在模型内 / 是否直接子元素 / 目标是否 `api = none` / 目标文件在不在盘 / **`x-class` 是否真有构造通路**）仍归 `apiRefMissing` 既有判据面或后续 change——写闸做它就变成第二份判据。唯一例外是 ⑦ 的**前缀**：它只需两个 FQN 做纯计算，故留在写闸。
- **`x-class` 分支的判序固定**：`type: "object"` → 无 `additionalProperties` → 无 `required` → `properties` 全为方法 → 预留名（`new` 与 `x-static` 重名）——「你不是 object」比「你不能有 required」更根本、「你是字典」比「你不能有 required」更根本；判序即错误面，MUST NOT 调换，也 MUST NOT 让新判据遮蔽既有五条 `$defs` 规则（嵌套 `$defs` / `$ref` 形态 / 重复键等仍在外层先命中）。
- **闸数不变**：上一版九道闸的**条数**逐字不变（新规则落 ⑨ 的 `$defs` 校验内），MUST NOT 为两个新键新增第十道闸。
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
- **THEN** 原样落盘、零校验、零「接口已校验」行（写闸射程只有 `<archi_models_dir>/**/<叶段>.schema.json`）

#### Scenario: edit 校验替换后正文

- **WHEN** 以 `edit` 把一份合法 schema 的 `title` 改成另一名字
- **THEN** `Err`（identity 闸），盘上字节零变化；改成正确 `title` 后落盘成功

#### Scenario: 新判据不遮蔽既有 `$defs` 规则

- **WHEN** 一份 schema 同时含「嵌套 `$defs`」违例与「`x-class` 缺 `type`」违例，分别单独去掉其中一条后再校验
- **THEN** 两条各自独立可判（单独保留任一条仍 `Err`）、文案互不相等；并存时按固定判序报先命中者（嵌套 `$defs` 在外层，先于 `x-class` 分支）

### Requirement: 类型的构造入口与静态方法（`x-constructor` / `x-static`）

`x-` 关键字面 SHALL 由三项扩为**五项**（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`），两个新键 SHALL 遵循同一落点纪律：**只允许出现在 `$defs.<T>` 条目内部，且仅 `x-class: true` 的条目可用**。

- **落点封闭**：`object` / `enum` / `oneOf` / `anyOf` / 标量 / 纯 `$ref` alias 上出现任一新键即 `ApiError` 并点名条目与该键；顶层键封闭六项、`properties.<k>` 的形态白名单逐字不变。理由：`object` 是 JSON Schema 原有的类型，代表**纯数据、无行为、非测试主体**，其实例由 `properties` + `required` 的字面量构造——再声明构造入口只会制造第二份真相，且会打散「**类与数据的分界 = 有没有方法**」这个唯一判据。没有 `x-class` 的 object 等同于 JSON 对象，不承载任何行为面。
- **`x-function` 落点由二处扩为四处**：`properties.<k>.x-function`（模块操作）/ `$defs.<T>.properties.<k>.x-function`（方法 / 函数指针字段）/ `$defs.<T>.x-constructor.x-function`（构造入口）/ `$defs.<T>.x-static.<名>.x-function`（静态方法）；后两处只在 `x-class: true` 的条目内可达。`x-function` 自身的子树仍不合法（函数值内部不得再有函数作为值），其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 及其更深层）仍 `ApiError` 并点名 JSON pointer。
- **`x-constructor` 的形态**：**单个对象**（不是「名字 → 签名」表）`{ "x-function": { "params": [ … ] } }`。`params` 可省（= 零参）；`returns` MUST NOT 出现（构造器的返回面由语义固定为本类型）；内层 `async` MUST NOT 为 `true`（构造器是个值而非 future，两语言都没有异步构造——放行只能静默丢该标志）。名字**固定 `new`**：构造是类型的唯一入口，名字不携带信息，故不设名字槽位、不入名字族；需要别名构造（`open` / `from_path`）的类型写进 `x-static` 并返回本类型。内层其余键沿用「`x-function` 内层自由形状」，MUST NOT 增设未知键闸。
- **`x-static` 的形态**：「名字 → `{ "x-function": { … } }`」对象。名字 MUST 是 ASCII 标识符、MUST NOT 为 `new`（保留给构造器），并入既有「**方法名**」标识符族（族别数仍七）；值的内层自由形状（`params` / `returns` / `async` / `description`）。返回本类型 `$ref` 的条目即**工厂**。

`x-class` 的骨架规则与判序 SHALL 固定如下（判序即错误面，MUST NOT 调换）：

```text
if x-class == true:
    type != "object"               → ApiError（缺骨架；出路：补 "type": "object"）
    contains additionalProperties  → ApiError（语义互斥；见下）
    contains "required"            → ApiError（原判据不变）
    properties 有非 x-function      → ApiError（原判据不变）
    预留名冲突                      → ApiError（见下两条）
```

- **`x-class` MUST 带 `type: "object"`**：骨架（标准 JSON Schema 能看见的形状）与行为面（只有本仓 `x-` 面能看见的 `x-class` + 方法）分两处声明，缺骨架即 `ApiError`。不写 `type` 的条目在标准校验器里是「任意值」；更实际的是 `type_block` **先判 `x-class`**（`emit/rust.rs` / `emit/ts.rs`），故 `{"type": "string", "x-class": true}` 会被静默吞成 trait / interface、`type: "string"` 无声消失。强制 `object` 后该组合在写闸即死，类 / 数据的分界也随之变成机械判据（两者共享同一骨架，差别只在 `x-class` 这一位）。
- **`x-class` MUST NOT 带 `additionalProperties`**（对象形态与布尔形态同拒）：`additionalProperties` 断言「键任意、值同型、无行为」，与「仅允许方法的 trait」语义互斥；条目层对它是零消费者（`trait_block` / `interface_block` 只读 `properties`），放行即静默丢；且 `is_zero_field_object`（只判 `type == "object"` ∧ `properties` 空）会对它反噬出 `impl <trait> { pub fn new() … }`（E0404）。错误文案的出路 MUST 可执行且 MUST NOT 把人引到第二个静默：① 去掉 `x-class` 变纯数据（并说明**条目层 map 形态本期不渲染** ⇒ 键值对容器写在值位：`x-function.params[].schema` / `returns` / 字段位）；② 若本意是「方法集合」，去掉 `additionalProperties`——trait 没有实例数据面。**只治一侧**：无 `x-class` 的 `object` 条目带 `additionalProperties` 仍被放行，本期不改它的静默。
- **两条预留名判据**（新增，同属「写闸消灭静默」，均为文件级自洽事实）：① `x-class` 条目的 `properties` 键 MUST NOT 为 `new`（保留给 `x-constructor`）——与构造器同名即在同一 trait body 产两处 `fn new`（`E0428`）；② `x-static` 的名字 MUST NOT 与该条目 `properties` 的键重名——两表在 Rust 合成同一命名空间，重名即 `E0428`。两条的出路句分别指向 `x-constructor` 与「改名或合并」。
- **生成面（生成器决策，不是协议判据）**：Rust 侧 `x-class` 条目把构造器与静态写进 trait body（`fn new(params) -> Self;` / `fn <名>(params) -> R;`，复用既有 `method_signature`）；trait 不可实例化 ⇒ 不产 `T::new` 见证行，绑定发生在 **impl 处**（实现该 trait 的具体类型必须补齐 `fn new` 且签名逐项相符，编译器强制）。TS 侧保留 `export interface T { … }`（形状承诺）并在 `x-constructor` / `x-static` 至少一项在场时追加同名值声明 `export declare const T: { new (params): T; <名>(…): R };`（类型 / 值同名共存，`isolatedModules` 下成立；两者皆无时零值声明，产物与今天逐字节相同）。协议文本 MUST NOT 出现任何目标语言语法字面。object 条目的产物零变化（零字段 object 仍自动产零参 `new()` 与见证行）。
- **`x-class` 条目 MUST NOT 被自动构造器命中**：自动零参 `new()` 与其见证行的判据 SHALL 显式排除 `x-class`（trait 不是类型本体，`impl <trait>` 与 `fn() -> super::<trait>` 都不可编译）。该排除与本 requirement 的 `type: "object"` 断言**同批**落地——否则空 trait 必然被命中。
- **至少一条构造通路（类的存在条件）**：每个 `x-class` MUST 至少有一条——① `x-constructor`；② `x-static` 中返回本类型 `$ref` 的方法（工厂；多个取声明序首个）；③ 任一 `x-function.returns` 指向它（父 / 兄弟元素的工厂 op 也算——**跨元素**事实）。**判定面本期缺席**：`validate_schema` 的纪律是「文件级自洽、零跨元素」，通路判定与成员比对一并归后续 change；本期写闸放行、零判据、零警告，规则文本 MUST NOT 因此下沉到写闸。
- **`INDEX.toml` 口径不变**：`$defs` 形状判定的判序 MUST NOT 变（`x-class` 先于 `scalar` / `object`）⇒ `categories` 仍记 `class`，`types` / `ops` / `degraded` 三项计数口径逐字不变。
- **零第二份判据**：上述全部规则落装载期 `validate_schema` 的同一实现（`verify_defs` 与 `contract::verify` 同趟），MUST NOT 在渲染期重复判定，MUST NOT 在写工具侧另写一份。

#### Scenario: 构造入口与静态方法的生成形态

- **WHEN** 某 `api = declared` 元素的 `$defs.Store` 为 `{"type":"object","x-class":true,"properties":{"count":{"x-function":{}}},"x-constructor":{"x-function":{"params":[{"name":"path","schema":{"type":"string"}}]}},"x-static":{"open":{"x-function":{"params":[{"name":"path","schema":{"type":"string"}}],"returns":{"$ref":"#/$defs/Store"}},"default_path":{"x-function":{"returns":{"type":"string"}}}}}}`
- **THEN** `.i.rs` 的 trait body 内（按声明序）出 `fn new(path: &str) -> Self;` 与 `fn open(path: &str) -> Store;` / `fn default_path() -> String;`；`.i.ts` 出 `export interface Store { … }` 加 `export declare const Store: { new (path: string): Store; open (path: string): Store; default_path (): string; };`；`INDEX.toml` 的该元素 `categories.class` 仍为 1、`types` 不因新键变化

#### Scenario: 新键落点在非类条目即错

- **WHEN** 分别把 `x-constructor` 写进一个纯 `object` 条目、把 `x-static` 写进一个 `enum` 条目、把任一新键写进顶层或 `properties.<k>` 内
- **THEN** 每次均 `ApiError`，文案点名 `$defs.<T>.<键>`（顶层 / `properties` 位则点名该位置）并给出路「只有 `x-class` 条目可声明构造入口与静态方法」；盘上字节零变化（写闸分支）；零字段 object 仍自动产 `new()`（新键不在其射程内）

#### Scenario: 构造器形态负例

- **WHEN** 分别构造 `x-constructor` 带 `returns`、`x-constructor.x-function` 带 `"async": true`、`x-static` 名写作 `new`、`x-static` 名写作 `open-it`
- **THEN** 四次均 `ApiError`：前两条点名 `returns` / `async` 与「构造器的返回面由语义固定 / 没有异步构造」的出路；第三条点名保留名并指向 `x-constructor`；第四条按「方法名」族报非 ASCII 标识符

#### Scenario: `x-class` 缺骨架即错

- **WHEN** 分别写 `{"$defs": {"Gate": {"x-class": true}}}`（缺 `type`）与 `{"$defs": {"Gate": {"type": "string", "x-class": true}}}`
- **THEN** 两者均 `ApiError`，点名 `$defs.Gate.type` 与「补 `\"type\": \"object\"`」的出路；`{"x-class": "true"}`（字符串值）不命中 `as_bool` ⇒ 走「未标 class」的既有警告面，不是本条错误

#### Scenario: `x-class` 带 `additionalProperties` 即错

- **WHEN** 分别写 `{"type":"object","x-class":true,"additionalProperties":{"type":"string"}}` 与 `{"type":"object","x-class":true,"additionalProperties":false}`
- **THEN** 两者均 `ApiError`，点名 `additionalProperties` 并给两条出路（去掉 `x-class` 变纯数据且说明条目层 map 形态本期不渲染、键值对容器写在值位 / 若本意是方法集合则去掉该键）；该条目 MUST NOT 抵达渲染期

#### Scenario: 空 trait 不产 `impl`

- **WHEN** 写 `{"$defs": {"Gate": {"type": "object", "x-class": true, "properties": {}}}}` 并重生成
- **THEN** `.i.rs` 只出 `pub trait Gate {\n}\n`；**零** `impl Gate {`、**零** `const _: fn() -> super::Gate`；自动构造器与见证行只对「`type == "object"` ∧ `properties` 空 ∧ 非 `x-class`」的条目产出

#### Scenario: 纯数据面零变化（良性对照）

- **WHEN** 写 `{"$defs": {"Ticket": {"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}}}` 与 `{"$defs": {"Marker": {"type": "object"}}}` 并重生成
- **THEN** `Ticket` 的产物逐字节与今天相同；`Marker`（零字段 object、无 `x-class`）仍出 `impl Marker { pub fn new() -> Self { Self } }` 与 `const _: fn() -> super::Marker = super::Marker::new;`；两者均零值声明（TS）

#### Scenario: 预留名冲突即错

- **WHEN** 分别写一个 `x-class` 条目带 `"properties": {"new": {"x-function": {}}}`，以及一个同时带 `properties.open` 与 `x-static.open` 的条目
- **THEN** 两者均 `ApiError`：前者点名 `new` 保留给 `x-constructor`，后者点名重名的方法并给出「改名或合并」的出路；错误都不落到渲染期（不产重复的 `fn new` / `fn open`）

#### Scenario: 构造通路判定本期缺席

- **WHEN** 写一个 `x-class` 条目既无 `x-constructor`、也无返回自身 `$ref` 的 `x-static`，且模块内无任何 op 的 `returns` 指向它，随后跑 `archi_drift`
- **THEN** 写闸放行、生成成功、`apiFindings` 零条目、stderr 零警告（规则文本在 spec 与后续 change 的笔记里，MUST NOT 由本期实现产出判据或提示）

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
| `crates/core/archi/src/api/contract.rs` | `<叶段>.md` 解析 + 装载期闸 | H1 锚点校验、`### <op-id>` 分段、三类 fence 提取、双向 gate 三条校验、构造器声明检测；错误带文件与行；`verify` 在同趟遍历内续判 `required ⊆ properties`（族别数不变、不拆第二个公开入口、零 pointer-less 文案），属性键族与 `required` 族的错误带 JSON pointer（`$defs.<name>.properties.<key>` / `$defs.<name>.required[<i>]`）与族别化出路句（`data-testid` 一类键不进契约） |
| `crates/core/archi/src/api/contract_test.rs` | 装载期闸语料 | `data-testid` 首层键 ⇒ 报错含 pointer 与出路句；`required` 悬垂 ⇒ 报错含 `required[0]`；自洽契约通过且渲染为必填 |
| `crates/core/archi/src/api/schema.rs` | `$defs` 规则 + 装配 + 债表扫描（说明段目录串换段） | `build_index` 说明段的「契约面：目录 …」串 SHALL 取 `foundation::layout::archi_models_rel_dir()`（原 `archi_api_rel_dir()` 已删；MUST NOT 取索引自身所在目录）；`IndexEntry.schema_rel` 仍是 repo 相对路径（现含 `models/<分层目录…>/`）；`$defs` 身份 / 提升原名 / `$ref` 顶层 / 禁按结构去重 / 透传条目纯 `$ref`；`schema.json` 六字段装配（`$schema` / `$id` / `title` / `$defs` / `properties` / `$comment`）；`INDEX.toml` 装配；`DEGRADED_KINDS`（四值封闭常量，与校验同处，MUST NOT 进 `common.json`）；`degraded_marks(schema) -> Vec<(String, &str)>`（全深度单点扫描）；`verify_defs` 同趟校验取值（非字符串亦拒，文案点名 pointer 并列四值）；`IndexEntry.degraded` 语义 = **标记出现次数**（不再是 `$defs` 条目数） |
| `crates/core/archi/src/api/schema_test.rs` | 词表语料 | 四值逐个通过；词表外值与非字符串值拒收（文案列四值）；`note` 不计入 |
| `crates/core/archi/src/api/extract/mod.rs` | 取数器分派 + **TS 导出闭包** + nullable 形状识别单点（本变更：仅模块 doc 方位串） | 模块 doc 的契约权威位置改指 `<archi_models_dir>/<分层目录…>/<叶段>.md`（doc-only，代码零改动；该串既非引号字面量、也不会被按符号 / 字面量扫描抓到）； 按语言分派到 `rust` / `ts`；产出候选 `{ symbol, kind, location, signature, visibility }` 与引用方（`used_by`，本期留白）栏；仅 bootstrap 使用，非权威；`NullableBreakdown { non_null: Option<&Value>, explicit_null: bool }` + `nullable_breakdown(schema: &Value) -> Option<NullableBreakdown>`（三分支识别唯一实现，沿用既有「取首个非空分支」语义），`schema_type` 改经它取非空分支、**对外结果逐值不变**（`non_null: None` 即纯 null 档 → `Optional(Unknown("null"))`），`explicit_null` 是渲染面 `\| null` 的唯一依据（MUST NOT 在 `emit` 侧第二次判 null）；函数**不收 `required`**——可选轴不属它，`schema_type` 签名不因本变更扩参；`ApiSymbol` 新增 `file: String`（定义文件，repo 相对 POSIX；进程内类型，不进 `dto` / 不进 specta）；`extract_element(root, elements, element, entry)`：Rust 分支 = `rust::extract_crate` 逐字不变，TS 分支 = 入口派生闭包（递归 `surface` + visited(文件, 名字集) + `scan::ts::TsResolver::resolve` + `scan::owner::OwnerIndex::owner_of` 的 owner 子树 + `(name, kind)` 去重 + `A as B` 改名），入口读不到 / 语言未知 ⇒ 空集；`extract(element, file, source)` 仍是**显式留白（D52）零产品调用者**，本变更 MUST NOT 接线；边界 doc 更正为「`extract` / `ts` / 归一纯函数零读盘；`extract_element`（与 `rust::extract_crate`）按入口读盘、零写盘、零 spawn」 |
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
| `.dev-team/architecture/models/<分层目录…>/**`（契约资产新落点） | 契约与生成物资产 | `<分层目录…>/<叶段>.md`（唯一手写）+ `<分层目录…>/<叶段>.schema.json`（生成，与 md 同目录）；由 `git mv` 自 `<archi_dir>/api/<分层目录…>/**` 搬入，内容逐字不变；两者入库、进零 diff 门 |
| `.dev-team/architecture/graph/INDEX.toml`（契约索引新落点） | 契约索引 | 由 `git mv` 自 `<archi_dir>/api/INDEX.toml` 搬入后由 `api:export` 重生成；差异恰为说明段目录串（`<archi_models_rel_dir>`）与已声明元素的 `schema` 列（repo 相对路径含 `models/<分层目录…>/`）两行，其余条目逐字不变 |
| `.dev-team/architecture/api/**` | **删除** | `api` 段退役，迁移后该目录不存在；孤儿枚举严格两处派生（`<archi_models_dir>/**` + `<archi_graph_dir>/INDEX.toml`），故 `api/` 残留对 `api:check` 不可见——缺席由 `git mv`（同提交）+ 目录树读数守，MUST NOT 写成「`api:check` 报应删未删」 |
| `AGENT.md` + `CLAUDE.md`（仓库根文档，见「仓库根文档的契约纪律节」） | agent 侧操作纪律入口（本变更：第 ① / ② / ⑤ 条目录串换段） | 第 ① 条手写真相 `<archi_models_dir>/<分层目录…>/<叶段>.md`、第 ② 条 `INDEX.toml` 方位 `<archi_graph_dir>`（原写「契约目录根」，`grep 'architecture/api'` 抓不到它）、第 ⑤ 条索引串 `<archi_graph_dir>/INDEX.toml`；`## 模块 API 契约` 节恰 5 条要点（手写真相位置与三类 fence / 四类生成物勿手改 / 两脚本与 `check` 链段序 / `api = none` 留白 / 入口文档承诺已成立元素的判据进 `status`、其中 `error` 档进 `passed`）；第②条 = 「实现入口同级」+ `<入口主干>.i.rs` / `<入口主干>.i.ts`（与「生成物落点与入口查找」同口径），条目数恒 5、其余四条要点逐字不动；MUST NOT 复制契约正文、判据长表或 fence 示例；零 `entry` 字段说明（字段知识归 `desktop-archi-agent` 的知识文本）；节内零 `openspec/specs/**` 路径；两文件该节正文逐字相同（同源纪律） |

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
| `AGENT.md` / `CLAUDE.md` | 改 | `## 模块 API 契约` 节第 ⑤ 条为纳管口径（入口文档承诺已成立元素的判据进 `status`、其中 `error` 档进 `passed`）；本变更再同步第 ① / ② / ⑤ 条的目录串（手写真相 `<archi_models_dir>/<分层目录…>/<叶段>.md`、索引方位 `<archi_graph_dir>`、索引串 `<archi_graph_dir>/INDEX.toml`），第 ③ / ④ 条零改动，条目数仍 5；两份副本该节正文逐字相同；第 ① 条另补一句「散文不复述名字清单（唯一持有者是 fence）」——要点数仍 5、其余四条逐字不动 |

### 模块增量：入口文档语义定位换面（module-doc-field-docs-retirement）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|------|
| `crates/core/archi/src/api/paths.rs` | 落点派生 | `directory(root)` == `archi_models_dir(root)`、`contract_path` == `<models>/<分层目录…>/<叶段>.md`、`schema_path` == 同目录 `<叶段>.schema.json`、`index_path` 逐字不变；**载体派生单点**：declared → `contract_path`、none → `doc` 归一后 join root（形态校验复用 `normalize_root` + `is_repo_relative`，后者提 `pub(crate)`）；MUST NOT 出现第二份路径归一实现（`docs::normalize_rel` 已随 `docs.rs` 删除） |
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

#### Function: 契约装载期抽取面（`crates/core/archi/src/api/contract.rs`，零新增公开出口）

| 函数 | 变化 | 语义 |
|---|---|---|
| `collect_surface_table(sections: &[ContractSection]) -> Vec<String>` | 新增（私有） | 扫 `## <中文二标题>` 段正文的表格行：去前导空白后以 `|` 起手 → 按 `|` 切首列 → 去空白后要求整体被一对反引号包裹且内容是 `is_identifier` ⇒ 计入；去重保序。fence 内容不在 `section.body` 内（解析期状态机保证），列表项与裸正文零抽取 |
| `verify(contract: &Contract, schema: &Value) -> Result<(), ApiError>` | 改（签名零变化） | 追加第四条闸：前置 `ops` / `types` / `reexports` 任一非空；断言抽取集 ∩（op id ∪ `api-types` 键 ∪ `api-reexport` 键）= ∅；非空即 `ApiError::Contract`，点名文件 + 交集名字 + 出路句（名字归 fence、散文只留语义）。既有各项（三闸 + 闸一 + 落点闸 + 自洽闸）逐字不变 |

- **零变化面（同批断言，不是省略）**：`api::load_contracts` / `api::enabled` / `api::check` / `api::bootstrap` / `api::render_all` / `api::api_ref` 签名与语义逐字不变（第四条闸经既有 `contract::verify` 调用点自动生效）；`Contract` / `ApiSnapshot` / `ArchiApiFinding` / `ArchiDriftReport` 零字段新增、零新 finding kind、`REPORT_VERSION = 3` 不变；IPC 命令面 / DTO / `src/types/generated/**` 零改动（`bindings:export` 幂等、`client:check` 零 diff）；`crates/tools/archi-api-gen` 与 `packages/desktop/package.json` 三条命令串零改动（闸只作用于装载期，不参与渲染 ⇒ `.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` 逐字节零 diff）；`views/architecture/components/dashboard/contract-panel.tsx` / `model-info.tsx` 零改动（无 `kind` / `category` 增删）。

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

### 模块增量：接口手写真相与写闸（archi-agent-context-review）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/mod.rs` | 契约面装配与出口 | `load_contracts` **只读 schema**（经 `validate_schema` 装配 `Contract`；schema 不在盘 ⇒ 跳过；坏 schema ⇒ `ApiError` 停摆）；`enabled(root, elements)` = 任一 `path` 非空 ∧ `api = declared` 元素 schema 在盘；`gating_findings` = `declared` ∧ schema 在盘；`ref_projection` 三路径**各自独立**（`contractPath` = doc 路径，在盘才出串）+ `status` 改判 schema 在盘；`bootstrap(root, element) -> Result<BootstrapDrafts, ApiError>`（两段文本）；`render_all` 只产 `INDEX.toml` / `.i.rs` / `.i.ts` |
| `crates/core/archi/src/api/contract.rs` | `Contract` 投影 + 复用三闸（**md 解析器退役**） | `parse` / `finish_section` / `parse_object*` / `string_end` / `decode_key` / `file_title` / `collect_reexport_prose` / `collect_surface_table` / `is_cjk` / `info_tokens` / `duplicate_key_error` 全删（约 600 行）；保留 `Contract` / `ContractOp` / `ContractType` + 新增 `Contract::from_schema`（解包 `x-function`、丢弃 `description`、`line = 0`）；保留 `duplicate_keys`（供写闸文本层）、`verify`（七族名字 / 落点 / `required ⊆ properties`，入参改判 schema 值）、`is_constructor`；`reexport_prose` / `sections` 字段删除；`FENCE_OP` / `FENCE_TYPES` 删除 |
| `crates/core/archi/src/api/schema.rs` | 接口校验单点 + 索引装配 | `build_schema` 退役；`verify_defs` 零改造保留；新增 `verify_identity` / `verify_keyspace` / `verify_shape` / `verify_mirror` / `verify_ref_prefix` / `verify_top_level_keys`；唯一公开入口 `validate_schema(fqn, text) -> Result<Contract, ApiError>`（九道闸一趟）；`build_index` 的 `present` 改判 schema 在盘、头注释改写「手写 / 生成分界 + review 口径反转」；`DEGRADED_KINDS` 与 `degraded_marks` 逐字不变 |
| `crates/core/archi/src/api/paths.rs` | 落点派生 | `carrier_path` / `doc_path` **退役**（资产① 落点恒为派生 `<叶段>.md`；`doc` 退出派生面）；`contract_path` / `schema_path` / `index_path` / `entry_state` / `ref_relative` / `generated_path` 逐字不变 |
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
| `verify_identity(fqn, value)` | 新增 | `$id` / `title` / 落点（分层目录段 + 叶段主干）三者与 FQN 逐字一致 |
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

### 模块增量：构造入口与静态方法（archi-api-constructor-static-members）

主基线既有行逐字沿用；本变更受影响模块如下：

### Function: `crates/core/archi/src/api/schema.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `verify_defs` | 改 | `x-class` 分支按固定判序增四类判据：`type != "object"` ⇒ `ApiError`（缺骨架，出路补 `"type": "object"`）/ 含 `additionalProperties`（含布尔形态）⇒ `ApiError`（两条出路）/ `x-constructor` 的 `returns` / `async` 形态 ⇒ `ApiError` / 新键落点（非 `x-class` 条目上出现即错）；既有 `required` 与「`properties` 全为方法」两条逐字保留；其余五条 `$defs` 规则与 `degraded` 词表校验零改动 |
| `verify_top_level_keys` | 改（文案） | 「`x-` 面只有 `x-function` / `x-class` / `x-common` 三项」→ **五项**（逐名）；顶层封闭六项与四键必在场逐字不变 |
| `validate_schema` | 零签名变化 | 九道闸的**条数**不变（新规则落 ⑨）；闸序（①–⑧）逐字不变 |
| `DEGRADED_KINDS` / `degraded_marks` / `build_index` | 零改动 | 降级词表、全深度扫描、索引装配与 `INDEX.toml` 口径逐字不变 |

### Function: `crates/core/archi/src/api/contract.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `verify_x_function_positions` | 改 | 落点由**二处扩为四处**：`$defs.<T>.x-constructor.x-function` 与 `$defs.<T>.x-static.<名>.x-function` 合法（仅当该条目 `x-class: true`，否则仍 `ApiError`）；`x-function` 自身子树与 `items` / `additionalProperties` 等位置仍非法；错误文案的「二处」→「四处」 |
| `verify` | 改 | 名字趟把 `x-static` 的键并入「方法名」族（族别数仍七）；增两条预留名判据（`x-class` 条目的 `properties` 键非 `new`、`x-static` 名与该条目 `properties` 键不重名），均为文件级自洽事实、零跨元素 |
| `is_constructor` / `CONSTRUCTOR_NAMES` / `CONSTRUCTOR_PREFIXES` | 零改动 | 封闭六值 + 两族前缀逐字不变（类型级构造改走 `x-constructor`，不改这张表） |

### Function: `crates/core/archi/src/api/emit/rust.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `trait_block` | 改 | `x-class` 条目的 trait body 依次渲染既有的 `properties` 方法、`x-constructor`（`fn new(params) -> Self;`）与 `x-static`（`fn <名>(params) -> R;`），签名尾段复用 `method_signature`；零第二份签名拼装 |
| `is_zero_field_object` | 改 | 判据增 `x-class != true`（自动零参 `new()` 与见证行只对纯数据产出）；唯一消费点是 `render_scope` 的 `constructible` 收集（`:73`），函数是否改名属实现自由 |
| `render_scope` 的见证段 | 零改动 | 不为 trait 产 `const _: fn(…) -> super::<T>` 见证行（trait 不是类型本体）；模块 op 见证逐字不变 |

### Function: `crates/core/archi/src/api/emit/ts.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `interface_block` / 类型段 | 改 | `x-class` 条目在 `export interface T { … }` 之后追加同名值声明 `export declare const T: { … };`（`new (params): T` + 每个 `x-static` 一条 `<名> (params): R`，声明序）；`x-constructor` / `x-static` **皆无时零值声明**；object 条目零值声明（形状与今天逐字节相同） |

### Function: `crates/core/archi/src/api/check.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `constructor_declared` | 改（文案 / 口径） | 只判**模块级** `properties` 的构造器名；文案指向 `$defs.<T>.x-constructor` 作为类型级构造的正规形态；判据词汇、求值序、`category_of` 零改动 |
| `is_constructor` 的双向用法 | 零改动 | `declared_ops` 与生成器的构造器过滤逐字不变 |

### API / CLI: 零改动面

| 项 | 变化 | 描述 |
|---|---|---|
| `api/mod.rs::def_shape` / `categories_of` | 零改动 | 判序不变（`x-class` 先于 `scalar` / `object`）⇒ `INDEX.toml` 的 `categories` 仍记 `class`，`types` / `ops` / `degraded` 三计数不变 |
| `api/emit/mod.rs`（`render_target` / `warn_missing_x_class` / `local_defs` / `reexport_defs`） | 零改动 | 条目分类与「全为函数却未标 `x-class`」的警告面逐字不变 |
| `api/extract/**` / `api/paths.rs` / `api/diff.rs` / `api/render.rs` | 零改动 | 取数面、路径面、差异面与本变更零交集 |
| DTO / IPC / 前端生成物 / `crates/tools/archi-api-gen` / `package.json` 三条命令串 | 零改动 | 零字段新增、零命令变化、零新判据值；`api:export` 重生成后 `INDEX.toml` / `.i.rs` / `.i.ts` 在案 11 份逐字节不变 |
| `.dev-team/architecture/**` | 零改动 | 零数据变化（55 元素 / 129 关系 / 六份表头计数逐字不变） |
