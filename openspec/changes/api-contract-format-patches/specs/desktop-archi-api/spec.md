# desktop-archi-api Specification（delta: api-contract-format-patches）

## Purpose

本 delta 只改契约**格式面**的三处，并把「同一份 schema 事实在判据面与渲染面读法不同」的不对称收口到单点：

1. **可选与可空**（P1）：`nullable` 三分支的识别由判据面（`schema_type`）与两侧渲染面（`emit::{ts,rust}::value_type`）共用同一个判定函数；可选轴（`required`）与可空轴（`type` 内含 `null`）各只有一个判定点、各自只落自己的符号（TS `?` / `| null`，Rust `Option<T>` 至多一层）；纯 `null` 档（无非空分支）按显式兜底（TS `null` / Rust 既有兜底），不折进可空轴的三面等价式。
2. **属性键与 `required`**（P2）：装载期在同趟遍历内续判 `required` 元素集 ⊆ 同条目 `properties` 键集；属性键族与 `required` 族的错误文案带 JSON pointer 并给出可执行出路；给 TS 侧加引号形态与 `x-key` 重命名映射两条路**明确否决**。
3. **降级**（P3）：`x-common.degraded` 取值封闭为四值词表并在装载期校验，`INDEX.toml` 的 `degraded` 由「`$defs` 条目数」改为「**标记出现次数**（全深度）」，成为真实债表。

其余八个 requirement（资产形态、派生与不可手改、引用与命名、生成物落点与入口查找、入仓与零 diff 门、仓库根文档纪律节、verify 闭环与 bootstrap 剪枝、模块嵌套与选择性透传）**逐字不动**；`desktop-archi-api-check` 零改动（零新判据种类、零新 finding kind、`REPORT_VERSION` 不动）。

## ADDED Requirements

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

- **WHEN** 对现网唯一契约（`App.Backend.Infra.ArchiRuntime`，`properties` 为空对象且零 `required`）与任意 `required ⊆ properties` 的契约执行装载
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
- **零影响**：现网唯一契约（`App.Backend.Infra.ArchiRuntime`）零 `degraded` 标记 ⇒ 现网计数仍为 `0`、`INDEX.toml` 逐字不变。

#### Scenario: 词表外值与非字符串值在装载期被拒

- **WHEN** 分别构造 `x-common.degraded = "typo"` 与 `x-common.degraded = true`
- **THEN** 两次均 `ApiError::Contract`，文案点名该标记的 JSON pointer 并列出四值；改为 `"noElement"` 后装载通过

#### Scenario: 债表按标记出现次数计

- **WHEN** 对演练草案形态的契约（`AgentSessionActions.send` 的参数、`AgentSessionViewProps.session`、`SessionPanelState.sessions.items` 三处标 `x-common.degraded`，其外层 `$defs` 条目自身无标记）执行 `render_all`
- **THEN** `INDEX.toml` 该元素的 `degraded = 3`；`note` 形态（`待剪枝：…`）出现任意次都零计入；同一份 `schema.json` 里三处标记也同时通过了词表校验（校验与计数同源）

#### Scenario: bootstrap 草稿零计入

- **WHEN** 对一个含 `x-common.note = "待剪枝：…"` 草稿条目的契约执行 `render_all`
- **THEN** `degraded` 计数为 `0`（草稿的 `note` 不升级为债）；该契约仍能装载（`note` 是自由注记，不加校验）

## MODIFIED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/extract/mod.rs` | nullable 形状识别的单点 | `NullableBreakdown { non_null: Option<&Value>, explicit_null: bool }` + `nullable_breakdown(schema: &Value) -> Option<NullableBreakdown>`（三分支识别唯一实现，沿用既有「取首个非空分支」语义）；`schema_type` 改经它取非空分支，**对外结果逐值不变**（`non_null: None` 即纯 null 档 → `Optional(Unknown("null"))`，既有行为不变）；`explicit_null` 是渲染面 `\| null` 的唯一依据（MUST NOT 在 `emit` 侧第二次判 null）；函数**不收 `required`**——可选轴不属它，`schema_type` 签名不因本变更扩参 |
| `crates/core/archi/src/api/emit/ts.rs` | TS 渲染 | `value_type` 在 `$ref` / 映射规则之后、`type` 分派之前处理 nullable（与 `schema_type` 同序）：可空 → `T \| null`（唯一产出点）、`array` 元素位可空 → `(T \| null)[]`、纯 null 形态 → `null`；`interface_block` 对非 `required` 字段只出 `{field}?:`，零 `\| null` 拼接；零标识符判据 |
| `crates/core/archi/src/api/emit/rust.rs` | Rust 渲染 | `value_type` 同样前置 nullable 分支（可空 → `Option<T>`）；`struct_block` 的可选包装经单点幂等函数 `optional_of`（已 `Option<…>` 即不二次包），`Option<Option<` 零产出 |
| `crates/core/archi/src/api/contract.rs` | `<FQN>.md` 解析 + 装载期闸 | `verify` 在同趟遍历内续判 `required ⊆ properties`（族别数不变、不拆第二个公开入口、零 pointer-less 文案）；属性键族与 `required` 族的错误带 pointer（`$defs.<name>.properties.<key>` / `$defs.<name>.required[<i>]`）与族别化出路句（`data-testid` 一类键不进契约） |
| `crates/core/archi/src/api/schema.rs` | `$defs` 规则 + 装配 + 债表扫描 | `DEGRADED_KINDS`（四值封闭常量，与校验同处，MUST NOT 进 `common.json`）；`degraded_marks(schema) -> Vec<(String, &str)>`（全深度单点扫描）；`verify_defs` 同趟校验取值（非字符串亦拒，文案点名 pointer 并列四值）；`IndexEntry.degraded` 语义 = **标记出现次数**（不再是 `$defs` 条目数） |
| `crates/core/archi/src/api/mod.rs` | 契约面装配与出口 | `index_entries` 的 `degraded` 改经 `schema::degraded_marks`（零第二套遍历；口径改一处两处随动）；出口八行与 `ApiSnapshot` 三面零变化 |
| `crates/core/archi/src/api/extract/mod_test.rs` | 单点对拍语料 | 9 组样例**三轴分列**（可空轴六组：`Optional` ⟺ `\| null` ⟺ `Option<`；可选轴两组：`schema_type` 同值、TS `a`/`b?` 分形、Rust `String`/`Option<String>` 分形；纯 null 一组：`Optional(Unknown("null"))` / `null` / `serde_json::Value`）；`schema_type` 结果与改动前逐值相同 |
| `crates/core/archi/src/api/emit/ts_test.rs` / `rust_test.rs` | 渲染语料 | `a: string` / `b?: string` / `c?: string \| null` 三态；纯 null 档 TS `null` / Rust `serde_json::Value`（不变）；`(T \| null)[]` 括号；`Option<Option<` 零命中；演练草案 fixture（D.1 改写为词表形态）金样零 `unknown` 泄漏 |
| `crates/core/archi/src/api/contract_test.rs` | 装载期闸语料 | `data-testid` 首层键 ⇒ 报错含 pointer 与出路句；`required` 悬垂 ⇒ 报错含 `required[0]`；自洽契约通过且渲染为必填 |
| `crates/core/archi/src/api/schema_test.rs` | 词表语料 | 四值逐个通过；词表外值与非字符串值拒收（文案列四值）；`note` 不计入 |
| `crates/core/archi/src/api/mod_test.rs` | 索引语料 | `degraded` = 全深度标记出现次数（嵌套属性位计入）；演练 fixture 三处 ⇒ `3`；既有 `render_all的index_degraded派生` 的布尔 fixture 改为词表字符串 |
| `packages/desktop/package.json` | 版本纪律 | `version` 相对落地时 `HEAD` 现值 +1 patch（`tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动） |
