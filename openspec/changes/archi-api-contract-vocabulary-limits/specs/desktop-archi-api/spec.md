# desktop-archi-api（delta）

> **基线**：`openspec/specs/desktop-archi-api/spec.md`。只写本变更的增量：**ADDED ×1**（类型出生与生成属性策略）+ **MODIFIED ×2**（关键字集与类型映射 / 装载校验与写闸）。归档时 ADDED 原样并入基线，MODIFIED 原位替换同名 requirement，其余 requirement 逐字不动。

## ADDED Requirements

### Requirement: 类型出生与生成属性策略（facets / 命名 / 宽度 / 文档注释 / 封闭枚举）

`$defs` SHALL 是跨边界类型的**唯一出生地**：契约写「形状 + 语义」，属性（derive / 本地命名 / 整数宽度 / serde 形态）SHALL 全部是 `schema → 生成物` 的**纯函数**，MUST NOT 引入任何声明位。

- **零新词汇**：`x-` 面仍封闭五项（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`），MUST NOT 为属性新增第六个 `x-` 键；`description` / `oneOf` / `const` / `default` / `minimum` / `maximum` 均为标准关键字。属性不再「契约说了什么」，而是「形状推得出什么」。
- **修复落规则不落文件**：生成物（`.i.*`）不可手改，故 facet / 宽度 / 命名的修复点永远在**规则与语料**（`schema → 属性` 的纯函数），MUST NOT 在生成物里改一处了事。规则 MUST 可单测、MUST 只吃 schema（零读盘）。

**facet 推导（std 面）** SHALL 按下表产出（`derive` 项在场顺序固定为 `Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type`，缺席项略去、在场项保持该相对序——这是「生成物 vs 手写逐字比对」的前提）：

| 形态 | `derive` 集合 |
|---|---|
| 零字段 `object`（名义类型） | `Debug, Clone, Copy, PartialEq, Eq`（**与既有产物逐字相同**，MUST NOT 扩 `Hash` / `Default`） |
| `oneOf` 全 `const`（封闭枚举） | 同上 + `Hash` + 条件 `Default`（条件 = `$defs.<T>.default` 在场且等于某分支的 `const` 值；该分支 SHALL 带 `#[default]`。`default` 缺省或值不命中任何分支 ⇒ 不出 `Default`、不报错——缺省**不等于**首个变体） |
| `object` 有字段（struct） | `Debug, Clone, PartialEq` + 条件 `Eq` |
| 标量 newtype | `Debug, Clone, PartialEq` + 条件 `Eq` |
| `oneOf` / `anyOf` 带负载 | `Debug, Clone, PartialEq`（MUST NOT 推 `Eq` / `Copy` / `Hash`） |

- **`Eq` 的条件**：全部承载位可 `Eq` —— `string` / `integer` / `boolean` / 可 `Eq` 的 `array`（看元素）/ 可 `Eq` 的 `object` 字典（看值）/ 可解析到本文件或其它元素 `$defs` 的 `$ref`（看目标）；`number`（`f64`）使 `Eq` 不成立；`oneOf` 带非标量负载使 `Eq` 不成立；含 `x-function` 字段（函数指针无 `Eq`）使 `Eq` 不成立。递归 MUST 是最小不动点且 MUST 容忍环（自引用 ⇒ `Eq` / `Copy` / `Hash` 不成立，且 MUST NOT 死循环）。
- **`Default` 只对封闭枚举，且只在 `$defs.<T>.default` 给出证据时**：`default` 在场且等于某 `const` 分支的值 ⇒ 出 `Default` 并在**该分支**加 `#[default]`；`default` 缺省 / 值不命中 ⇒ 不出、不报错（MUST NOT 退化为「首个变体」）。struct / newtype MUST NOT 推 `Default`——没有证据的公共承诺比缺一个 derive 更贵。

**facet 推导（wire 面）** SHALL 按「契约是否已定义线形态」判定，`Serialize` / `Deserialize` / `specta::Type` 三项恒同进同出：

| 形态 | wire 面 | 理由 |
|---|---|---|
| `object` 有字段（字段位无 `x-function`）、封闭枚举、标量 newtype | **出** | 契约已定义线形态（对象字段 / 词表值 / 内层标量） |
| 零字段 `object`（名义类型） | **不出** | 零数据位没有线形态可承诺（`serde` 会把它写成 `null`）；在案唯一实例是名义标记 `LocalArchiCheck`，出它就得给一个不消费序列化的 crate 加两条依赖 |
| `oneOf` / `anyOf` 带负载 | **不出** | JSON Schema `oneOf` **不定义判别位**，而 Rust 缺省外部标记会产出一个契约没写的线形态（静默错）。该形态的线形态裁决归第一个真实需求，本要求只登记「未定义」 |
| `object` 有字段但某字段是 `x-function` | **不出** | 函数指针没有 JSON 线形态（`serde` 派生必 `E0277`）。字段本身仍按既有形态出 `pub <name>: fn(…) -> …`；该 struct 的 std 面同时不推 `Eq`（保守） |

- **消费方约束（编译前置）**：生成物与元素入口同 crate 编译，故声明**有数据位**类型的元素 crate MUST 具备 `serde`（`derive` 特性）与 `specta`（`derive` 特性）依赖。缺依赖的表现是 `cargo build` 失败（fail-closed），MUST NOT 由渲染期探测 crate 依赖来回避。本仓 `core/archi` 已具备两者（它的线面类型即该面），其余 crate 首次声明有数据位类型时同批补依赖。

**命名策略（本地标识符 = 契约键的纯函数）**：

- 契约 `properties` 的键即 **wire 名**；本地标识符 SHALL 由其机械派生（`camelCase` → `snake_case`；已是合法小写标识符则原样）。MUST NOT 出现「声明位」形态的改名（`x-key` 一类仍被「属性键与 `required` 的自洽闸」否决）：被否决的是**第二套命名的持有者**，机械派生不持有第二份真值。
- **serde 形态自动选**：先试容器级 `#[serde(rename_all = "<候选>")]`（候选集 `camelCase` / `lowercase` / `PascalCase` / `snake_case`），任一候选能把全部本地名映射回全部契约名即采用；全不命中才逐项 `#[serde(rename = "<wire>")]`。选择是输入的函数，MUST NOT 需要作者声明。
- **关键字位用目标语言的原生转义**：本地名命中目标语言关键字表 ⇒ Rust 侧出 `r#in` / `r#abstract`（serde 剥 `r#` 后恰为 wire 名 ⇒ 零 `rename`）。该表 MUST 是仓库唯一那张 `RUST_KEYWORDS`（`api/paths.rs`，经 `pub(crate) fn is_rust_keyword` 读取）——`emit` MUST NOT 自抄第二份关键字清单。四个不可 raw 的词（`self` / `Self` / `super` / `crate`）无法表达 ⇒ 写闸 `ApiError` 并给出路「改名」（见「接口定义文件的装载校验与写闸」）。
- **TS 侧字段名不派生**：契约键即 wire 名，TS 结构里原样出现。

**整数宽度（严格完整值域）**：`type: "integer"` 的本地类型 SHALL 由值域关键字推导，且**只认完整值域**：

| 值域（闭区间，`exclusive*` 先归一） | 本地类型 |
|---|---|
| `0 ..= 4294967295` | `u32` |
| `0 ..= 18446744073709551615` | `u64` |
| `-2147483648 ..= 2147483647` | `i32` |
| 缺 bounds / 部分范围 / 单边 / 其它 | `i64`（与既有产物逐字相同） |

- **不推窄**：`maximum: 100` MUST NOT 推 `u8`——业务约束与本地精度是两件事，让业务值域决定类型会产出「改一个 `maximum` 就静默换类型」的耦合。
- `type: "number"` 恒 `f64`（值域推不出精度）。
- **`usize` 不可表达**：平台相关类型在线面上与 `u64` 无法区分 ⇒ 契约 MUST NOT 提供 `usize` 的写法；需要它的手写类型在迁移时落 `u64`。
- **值域关键字从此有两个读者**：标准校验语义 + 本地宽度推导。本要求 MUST NOT 被读成「判据面新增宽度判据」：`$defs` 字段零判据，op 签名的宽度差异由签名见证行（`E0308`）兜。

**文档注释**：`description` SHALL 进 `.i.rs` 的 `///`，三处——类型（`$defs.<T>.description`）、字段（`$defs.<T>.properties.<f>.description`）、变体（`oneOf[i].description`）。

- 空串与缺省 ⇒ **零注释行**（bootstrap 草稿的 `"description": ""` 因此不产空注释）。
- 一切来自契约的文本 MUST 经该语言单点注释转义（换行 / `U+2028` / `U+2029` 折成空格），否则一条 `description` 就能把后半段顶成代码行。
- op 的 `x-function.description` **仍被丢弃**（既有规定不变：签名见证只读 `params` / `returns` / `owner`）。
- **TS 侧本要求零新增**：逐值 / 类型级 JSDoc 与封闭枚举值对象一并归后续变更（本变更对 TS 只落「封闭枚举 → 字面量联合」）。

**封闭枚举的唯一形态 = `oneOf: [{const, description}, …]`**：

- `$defs` 条目顶层的 `enum` 数组**退役**（装载期拒收，出路指向本形态）；`const` 只允许出现在 `oneOf` / `anyOf` 的分支内，且该 `const` MUST 是标量。
- Rust → `pub enum`：每个 `const` 分支出一个 unit variant（变体名由既有 `variant_name` 归一）+ `///`（分支 `description`）；混合分支（部分带负载）沿用既有「带负载变体」形态。
- TS → 字面量联合（`export type X = 'off' | 'warn';`，成员经 `ts_string_literal` 转义 / 数字与布尔原样），MUST NOT 出 `enum` 关键字，MUST NOT 出值对象。
- `oneOf` 带负载的分支其 wire 面不出（见上表）。

**索引形状分类**：`def_shape` 的 `enum` 类目 SHALL 改由「`oneOf` / `anyOf` **全 `const` 分支**」触发，其余 union 记 `variant`；`enum` 数组不再产生任何类目（装载期已拒）。`categories` 的键集合 MUST NOT 变（`INDEX.toml` 与面板零改动）。

**明确不做（本变更）**：wire 面的元素级声明位、负载 union 的线形态、`usize`、元组与别名词汇、`$ref` 可解析 ⇒ `Named`（判据侧，归独立变更）、`async` 短形与见证分叉（同上）、TS 值对象、`$defs` 字段形状判据。

#### Scenario: 三个代表类型与手写逐字等价

- **WHEN** 把一个「有字段 struct（具名 `$ref` 字段 + camelCase 键 + 字段说明）」、「一个含 `Option<string>` / `boolean` / 完整 `u64` 值域 integer / `array<string>` 的 struct」、「一个 unit-only 封闭枚举（`oneOf` + `const` + 逐值 `description`、`lowercase`）」分别写成 schema 并渲染 `.i.rs`
- **THEN** 三者的类型定义段（`///` 注释 + `derive` 行 + 容器 `#[serde(rename_all = …)]` + 变体 / 字段行）与手写对应段落**逐字相同**；`derive` 项顺序与手写一致；`Default` 与 `#[default]` 只在 `$defs.<T>.default` 在场且命中某 `const` 分支时出现（命中分支带 `#[default]`，`default` 缺省 ⇒ 零 `Default`）

#### Scenario: `default` 关键字决定缺省变体

- **WHEN** 分别渲染三个同形三值封闭枚举（`off` / `warn` / `strict`）：一份带 `"default": "off"`、一份不带 `default`、一份带 `"default": "unknown"`（不命中任何 `const` 分支）
- **THEN** 第一份出 `Default` 且 `#[default]` 落在 `Off`（与手写逐字相同）；后两份零 `Default`、零 `#[default]`，`derive` 其余项逐字不变；第三份**不报错**（值不命中只是不推，不是坏输入）

#### Scenario: 零数据位与带负载形态的 wire 面缺席

- **WHEN** 渲染零字段 `object`（名义类型）、带负载 `oneOf`、以及一个含 `x-function` 字段的 `object` 三个条目
- **THEN** 第一个出 `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` 与既有产物**逐字节相同**、零 `Serialize` / `Deserialize` / `specta::Type`；第二个出 `Debug, Clone, PartialEq`（零 `Eq` / `Copy` / `Hash`）且同样零 wire 面；第三个同样零 wire 面、零 `Eq`，其字段仍出 `fn(…)` 指针形；三者都不 panic、不产半截属性行

#### Scenario: 关键字键与容器级 rename_all 组合成立

- **WHEN** 写 `{"$defs": {"T": {"type": "object", "properties": {"in": {"type": "integer"}, "typeOnly": {"type": "boolean"}}, "required": ["in", "typeOnly"]}}}`
- **THEN** 生成为 `#[serde(rename_all = "camelCase")] pub struct T { pub r#in: i64, pub type_only: bool }`；把两个本地名经该 serde 形态映射回去恰得 `in` / `typeOnly`（round-trip 成立）；源码零 `#[serde(rename = "in")]`（`r#` 已足够）

#### Scenario: 值域只认完整区间

- **WHEN** 分别渲染 `[0, 4294967295]`、`[0, 18446744073709551615]`、`[-2147483648, 2147483647]`、无 bounds、`maximum: 100`、仅 `minimum: 0` 六个 integer
- **THEN** 前三个出 `u32` / `u64` / `i32`；后三个一律 `i64`（`maximum: 100` MUST NOT 出 `u8`）；`type: "number"` 恒 `f64`；`["integer","null"]` + 完整 u32 值域出 `Option<u32>`

#### Scenario: 注释三处且空串零行

- **WHEN** 写带类型级 / 字段级 / 变体级 `description` 的条目，另写一个 `description: ""` 的条目与一个带 `x-function.description` 的 op
- **THEN** 前者三处各出一行 `///`（文本与契约逐字相等）；空串条目零注释行；op 的 `.i.rs` 产物与不带 `description` 时**逐字节相同**；含换行 / `U+2028` 的 `description` 不产生裸代码行

#### Scenario: 环与不可推 facet 是保守而非猜测

- **WHEN** 渲染一个字段为自身 `$ref` 的 `object`，与一个含 `number` 字段的 struct
- **THEN** 前者 `derive` 无 `Copy`（且渲染在有限步内返回、不栈溢出）；后者 `derive` 无 `Eq`；两者都**不报错**（推不出来只是不推）；facet 推导两次调用同一 schema 结果逐字相同

#### Scenario: 封闭枚举的 `enum` 数组路径被装载期消灭

- **WHEN** 写 `{"$defs": {"Plain": {"enum": ["a", "b"]}}}`，另写一个条目顶层裸 `{"const": "a"}`
- **THEN** 两者均 `ApiError`：前者点名 `$defs.Plain.enum` 并给出 `oneOf: [{"const": "a", "description": "…"}]` 的出路，后者点名 `const` 只允许在 `oneOf` / `anyOf` 分支内；`oneOf` 分支内的 `const` 正常通过；源码 `enum_block` 零命中（两渲染器都不再有该函数）

#### Scenario: 索引类目按触发点分流

- **WHEN** 对「`oneOf` 全 `const`」与「`oneOf` 带一个负载分支」两个 `$defs` 条目取形状分类并渲染 `INDEX.toml`
- **THEN** 前者记 `enum`、后者记 `variant`；`categories` 的键集合与改动前相同；在案 `INDEX.toml` 逐字节不变

#### Scenario: 生成物是格式化器固定点

- **WHEN** 对 golden 用例（含 `derive` 行超宽折行形态）执行 `api:export` 后跑 `cargo fmt --all --check`
- **THEN** 零 diff（生成器产出即已格式化的稳定态）；MUST NOT 出现「生成物文本与手写逐字相同但 fmt 判红」的二选一

## MODIFIED Requirements

### Requirement: 关键字集、类型映射与错误面

接口定义 SHALL 只用标准 JSON Schema 关键字加**五个**扩展构造（`x-` 面封闭，MUST NOT 自造新的 `x-` 前缀键）：

| 关键字 | 语义 | 生成 |
|---|---|---|
| `object` / `const` / `default` / `oneOf` / `array` / `minimum` / `maximum` / `$ref` | 标准关键字（`enum` **已退役**，见本条末） | 见映射表 |
| `x-class` | 声明为 trait，**仅允许方法**（出现属性即 `ApiError`）；MUST 带 `type: "object"`、MUST NOT 带 `additionalProperties` | `pub trait` / `export interface` |
| `x-function` | 函数签名：`params` / `returns` / `async` / `owner` / `description` | `pub fn` / `export type Fn = (…) => R` |
| `x-common` | 自由注记（`unit` / `note` / `since` / `owner` / `tag`）+ `degraded`（取值受封闭四值词表校验） | 不变换 |
| `x-constructor` | **仅 `x-class` 条目**：规范构造入口，单个对象 `{ "x-function": { "params": [ … ] } }`，名字固定 `new`，`returns` 禁止 | trait body `fn new(…) -> Self` / 值声明的 `new (…)` |
| `x-static` | **仅 `x-class` 条目**：「名字 → `{ "x-function": { … } }`」类型级静态方法表，名字非 `new` 且并入方法名族，返回本类型即工厂 | trait body `fn <名>(…) -> R` / 值声明的 `<名> (…)` |

- **`x-function` 的落点 SHALL 封闭四处**：`properties.<k>.x-function`（模块操作）、`$defs.<T>.properties.<k>.x-function`（函数指针字段）、`$defs.<T>.x-constructor.x-function`（构造入口）与 `$defs.<T>.x-static.<名>.x-function`（静态方法，后两处只在 `x-class` 条目内可达）；其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 及其更深层、`x-function` 自身子树）即 `ApiError` 并点名 JSON pointer。位置类错误的正确处置是**搬位置**，不是在渲染期兜底。
- **`properties.<k>` 的形态 SHALL 封闭二选一**：`{ "x-function": { … } }` 或 `{ "$ref": "#/$defs/<名>" }` 且 `<名> ∈ $defs`。其余形态（内联 `object` / 数组 / `oneOf` / 裸 `type`）即 `ApiError`——内联 object 会经 `schema_type` 的 type 分派静默退化为 `Record<string, unknown>`（编译通过、类型全丢），这是本闸存在的唯一理由。「想复用类型就放进 `$defs`，想声明操作就用 `x-function`」。
- **`x-function` 内层是自由形状**：闸只判外层形态，MUST NOT 对 `x-function` 内层（`async` / `description` / 将来可能有的 `deprecated`）做未知键校验（`x-constructor` 的 `returns` / `async` 例外已由「类型的构造入口与静态方法」定形，属**语义**约束而非未知键闸）。
- **op 的散文说明 SHALL 落在 `x-function.description`**：写**调用方要知道的语义**（它做什么 / 边界在哪 / 为什么），MUST NOT 写实现细节、MUST NOT 复述签名（签名就在同一处，复述是第二事实源）。`Contract::from_schema` 装配 `ContractOp.signature` 时 SHALL **解包** `x-function` 并**丢弃** `description`（`emit` 只读 `params` / `returns` / `owner`，渲染无关字段不进 emit 输入）。
- **`$defs.<T>.description` 承载类型语义**，并 SHALL 进 `.i.rs` 的类型级注释（字段位与 `oneOf` 分支位同款；语义规则见「类型出生与生成属性策略」）；`$comment` 只承载**写者纪律**（人读、不生成），MUST NOT 被任何判据读取。
- **注解载体三选封闭**：`description`（进 `.i.*` 文档注释的语义：类型 / 字段 / 变体三处；TS 侧的逐值注释本变更留白）/ `$comment`（只在源文件）/ `x-*`（只在本表**五项**内）。JSONC 行注释 MUST NOT 采纳（解析器多一份真相）。
- **封闭词表 MUST 用 `oneOf: [{const, description}]` 表达**：`$defs` 条目顶层的 `enum` 数组已退役（装载期 `ApiError`，见「接口定义文件的装载校验与写闸」⑨）；`const` 只在 `oneOf` / `anyOf` 分支内合法且 MUST 是标量。退役理由：同一语义两种写法就是第二份真相，且两条路今天各缺一半（`enum` 数组没有逐值语义落点、`oneOf` + `const` 在 Rust 侧落 `Variant0` 兜底）。
- 映射表（`object` → `struct` / `interface`；`oneOf` / `anyOf` 的**全 `const` 分支** → Rust `enum`（unit variant + 逐变体 `///`）/ TS 字面量联合、**带负载分支** → 带负载 `enum` / union；`x-class` → `pub trait` / `export interface`；`x-constructor` / `x-static` → trait body 的 `fn new(…) -> Self` / `fn <名>(…) -> R` 与同名的值声明（语义定义归「类型的构造入口与静态方法」，写法属生成器决策）；标量 → newtype / 别名；非 `required` 字段 → `Option<T>` / `{field}?: T`；显式 null 三分支 → `Option<T>` / `T | null`；`x-function` → `pub fn` / `(…) => R`；错误面 `Outcome` 信封 → `Result<T, String>` / union 或 `Promise` reject）与两条位置分派纪律 SHALL 逐条不变（`enum` 行的退役见上一条）。
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

#### Scenario: 封闭词表的两种写法只有一种合法

- **WHEN** 分别写 `{"$defs": {"Mode": {"enum": ["a", "b"]}}}` 与 `{"$defs": {"Mode": {"oneOf": [{"const": "a", "description": "甲"}, {"const": "b"}]}, "properties": {"Mode": {"$ref": "#/$defs/Mode"}}}}`
- **THEN** 前者 `ApiError`（文案点名 `$defs.Mode.enum` 与 `oneOf` + `const` 的出路）；后者写闸通过，`.i.rs` 出两个 unit variant（首个带 `/// 甲`）、`.i.ts` 出 `export type Mode = 'a' | 'b';`

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
   + $defs 条目顶层的封闭词表形态（enum 数组拒收 / 裸 const 拒收 / 不可 raw 的键名拒收）
```

- **装载消费**：`load_contracts` 读盘后调本函数，失败 ⇒ `ApiError` **停摆**（MUST NOT 产半真报告、MUST NOT 跳过该元素继续）。
- **写闸消费**：`write` / `edit` 落 `<archi_models_dir>/**/<叶段>.schema.json` 时先调本函数，失败 ⇒ `Err` 且**零落盘**；成功 ⇒ 落盘并在响应追加「接口已校验」一行。路径判定以 sandbox 后的绝对路径落在 `<root>/<archi_models_rel_dir>/` 之下为准（目录前缀经 `foundation::layout` 派生，MUST NOT 行内字面量）；非该路径原样落盘、零校验。
- **文件级自洽，零跨元素**：本函数 MUST 只吃「本文件 + 本 FQN」。跨元素的一切（`$ref` 目标是否在模型内 / 是否直接子元素 / 目标是否 `api = none` / 目标文件在不在盘 / **`x-class` 是否真有构造通路**）仍归 `apiRefMissing` 既有判据面或后续 change——写闸做它就变成第二份判据。唯一例外是 ⑦ 的**前缀**：它只需两个 FQN 做纯计算，故留在写闸。
- **`x-class` 分支的判序固定**：`type: "object"` → 无 `additionalProperties` → 无 `required` → `properties` 全为方法 → 预留名（`new` 与 `x-static` 重名）——「你不是 object」比「你不能有 required」更根本、「你是字典」比「你不能有 required」更根本；判序即错误面，MUST NOT 调换，也 MUST NOT 让新判据遮蔽既有五条 `$defs` 规则（嵌套 `$defs` / `$ref` 形态 / 重复键等仍在外层先命中）。
- **封闭词表形态三判（本变更新增，同落 ⑨）**：① `$defs` **条目顶层**带 `enum` ⇒ `ApiError`，文案点名 `$defs.<T>.enum` 并给可执行出路（`"oneOf": [{"const": "<值>", "description": "<语义>"}, …]`）；② 条目顶层带裸 `const` ⇒ `ApiError`（`const` 只允许在 `oneOf` / `anyOf` 分支内，且 MUST 是标量）；③ 条目第一层 `properties` 的键 ∈ `{self, Self, super, crate}` ⇒ `ApiError`（目标语言无法为这些词提供 raw identifier，机械派生不成立），出路「改名」。三判都排在既有五条 `$defs` 规则与 `x-class` 分支**之后**、降级词表之前——外层规则与骨架规则优先，MUST NOT 让新判据遮蔽它们。
- **闸数不变**：上一版九道闸的**条数**逐字不变（新规则落 ⑨ 的 `$defs` 校验内），MUST NOT 为两个新键新增第十道闸；本变更同样 MUST NOT 为封闭词表形态新增第十道闸。
- **错误面**：`Err` 的第一行自足（含文件 + 期望值 + 一条出路），MUST NOT 只写「非法」。`edit` 校验的是**替换后的正文**（不是替换前的旧文）。
- **零第二份实现**：MUST NOT 在 `write` / `edit` 侧另写一份 JSON 校验、MUST NOT 在装载侧再写一份；两处 MUST 调同一函数。渲染期 MUST NOT 重复判 `enum` / `const` 的合法性（渲染器结构上收不到——写闸已拒）。

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

#### Scenario: 封闭词表与不可 raw 键名各自可判

- **WHEN** 分别写「条目顶层 `enum`」「条目顶层裸 `const`」「`$defs.T.properties.self`」三种 schema 落盘，再各单独修成 `oneOf` + `const` / 分支内 `const` / 改名为 `own`
- **THEN** 三次首次均 `Err` 且零落盘，文案分别点名 `$defs.T.enum`（含 `oneOf` + `const` 的出路）/ `$defs.T.const` / `$defs.T.properties.self`（出路「改名」）；修正后各落盘成功并在响应追加「接口已校验」；同一份 schema 里 `oneOf` 分支内的 `const` 不被误伤

## Module Contract

### 模块增量：类型出生与属性下沉（archi-api-contract-vocabulary-limits）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `crates/core/archi/src/api/emit/mod.rs` | 语言分派 + 渲染入参装配 | `render_target(target, contract, schema, meta, defs)` 增一份「FQN → `$defs`」解析表入参（跨元素 `$ref` 的 facet 递归唯一读面；数据来自 `ApiSnapshot.schemas`，**零新增读盘**）；`local_defs` / `reexport_defs` / `notice_lines` / `warn_missing_x_class` / `reject_generics` 逐字不变 |
| `crates/core/archi/src/api/emit/rust.rs` | `.i.rs` 渲染 | 新增：`///` 三处注释（经 `rust_comment_text` 单点转义）、facet 推导（std 面 + wire 面，最小不动点 + 环容忍）、derive 行与容器 serde 属性、`#[default]`、字段本地名派生与 `r#`、值域宽度。改：`variants_block` 增 `const` 分支（unit variant + 变体 `///`）。删：`enum_block`。零改动：`op_witness` / `param_list` / `method_signature` / `function_pointer` / `optional_of` / `is_zero_field_object`（`owner` 见证形态 MUST NOT 在本变更改） |
| `crates/core/archi/src/api/emit/ts.rs` | `.i.ts` 渲染 | 改：`oneOf` / `anyOf` 的 `const` 分支产出标量字面量并与其余分支拼 union（`ts_string_literal` 转义）。删：`enum_block`。零新增：注释面与值声明面（逐值 / 类型级 JSDoc 归后续变更）；字段名不派生 |
| `crates/core/archi/src/api/schema.rs` | 装载校验单点 | `verify_defs` 增三条封闭词表形态判据（条目顶层 `enum` / 裸 `const` / 不可 raw 的键名），落 ⑨、判序在既有五条 `$defs` 规则与 `x-class` 分支之后、降级词表之前；九道闸条数不变；`DEGRADED_KINDS` / `degraded_marks` / `build_index` / `verify_class_members` / `verify_class_key_positions` 逐字不变 |
| `crates/core/archi/src/api/mod.rs` | 契约面装配与索引 | `render_all` 装配解析表并透传；`def_shape` 的 `enum` 类目改由「`oneOf` / `anyOf` 全 `const` 分支」触发（其余 union 仍 `variant`）；`categories_of` / `index_entries` / `bootstrap` / `enabled` / `load_contracts` / `api_ref` 逐字不变 |
| `crates/core/archi/src/lib.rs` | 入口编排 | 增 `#[path = "lib.i.rs"] mod api_gen; pub use api_gen::*;`（spec「生成物落点与入口查找」既有规定、本变更补齐实现；与 `crates/infra/archi/src/lib.rs` 同款） |

- **基线中被本变更改写的两条「逐字不变 / 零改动」断言**（不点名会让 spec 自称不变而实现已变）：① Module Contract 主表的「`api/emit/mod.rs` + `emit/{rust,ts}.rs` / 渲染面 / 逐字不变」——本变更改三处渲染面（`render_target` 增参、两渲染器的类型段与封闭枚举形态）；② 「模块增量：构造入口与静态方法」的「`api/mod.rs::def_shape` / `categories_of` / 零改动」——`def_shape` 的 `enum` 支改判触发点（`categories_of` 仍零改动）。其余各行（`check.rs` / `contract.rs` / `render.rs` / `diff.rs` / `DEGRADED_KINDS` / 判据词汇 / DTO / 命令串）逐字不变；`paths.rs` 的唯一增量是内部单点 `pub(crate) fn is_rust_keyword`（`RUST_KEYWORDS` 的唯一读取点，`emit::rust` 的字段名派生与 `is_module_segment` 共用，`is_module_segment` 的行为与返回值零变化），不计入公共 API。

### Function: 渲染面（`crates/core/archi/src/api/emit/**`）

| 函数 | 变化 | 语义 |
|---|---|---|
| `emit::render_target` | 改（签名 + 一个入参） | 增「FQN → `$defs`」解析表；五条驱动面（IPC / CLI / agent 工具 / `render_all` / 测试）同源透传，MUST NOT 各建一份 |
| `emit::rust::render` / `emit::ts::render` | 改（签名同上） | 解析表只供 `$ref` 的 facet 递归与可解析性判定；两渲染器仍零读盘、零 `../` 手算 |
| `emit::rust::struct_block` | 改 | 出 `///`（类型 / 字段）+ derive 行 + 容器 serde 属性；本地字段名派生与 `r#`；可选包装仍经 `optional_of` 单点 |
| `emit::rust::variants_block` | 改 | `const` 分支 → unit variant + `///`；带负载分支形态逐字不变 |
| `emit::rust::enum_block` / `emit::ts::enum_block` | **删除** | `enum` 数组已在装载期拒收，函数保留即成第二份形态 |
| `emit::rust::value_type` / `scalar_type` | 改 | 共用同一值域宽度判定；其余分派（`$ref` → 映射规则 → nullable → `type`）与序逐字不变 |
| `emit::rust::field_ident`（新） | 新增（私有） | 契约键 → 本地标识符的唯一派生点（含关键字 `r#`）；判关键字经 `paths::is_rust_keyword`（`RUST_KEYWORDS` 唯一词表，MUST NOT 自抄第二份）；MUST NOT 在字段 / 变体 / 方法三处各写一套 |
| `emit::rust::facets`（新） | 新增（私有） | 形态 → derive 集合 + serde 形态的唯一推导点（纯函数：`schema + 解析表 → 属性`；环容忍的最小不动点） |

### API: 生成物驱动面（零命令变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `packages/desktop/package.json` | 零改动 | `api:export` / `api:check` / `check` 三条命令串逐字不变；`version` 相对 `HEAD` +1 patch |
| IPC `archi_api_generate` / CLI `archi-api-gen` / agent 工具 `archi_generate` | 零改动 | 三者仍调 `render_all` → `emit::render_target`（新入参在 `render_all` 内部装配，驱动面签名零变化） |
| `INDEX.toml` / `.i.rs` / `.i.ts` | 零字节变化 | 在案三元素零有数据位类型、零 `description`、零值域关键字、零 `enum` / `oneOf` ⇒ `api:check` 零差异 |

### Component: 契约资产（理由文档）

| 项 | 变化 | 描述 |
|---|---|---|
| `.dev-team/architecture/models/Backend/Core/Archi.md` | 改（两处成因句） | 契约面段：见证链已接（入口 `#[path] mod api_gen`）；类型段：`$defs` 为空的成因改为「属性层此前无归属 + 迁移是独立一轮」；MUST NOT 列导出名 |
| `.dev-team/architecture/models/Backend/Core/Foundation.md` | 改（一处成因句） | 不接生成物的原因改为「`owner` 方法的接收器在契约里不可表达 ⇒ 见证形态对 `is_set` 结构性失效」；「不为让生成物能编译而多开导出面」的立场逐字保留 |
| `.dev-team/architecture/models/**/<叶段>.schema.json`、`graph/model/*.toml` | 零改动 | 接口面与拓扑逐字节不变（本变更不增删元素 / 关系 / op / 类型） |
