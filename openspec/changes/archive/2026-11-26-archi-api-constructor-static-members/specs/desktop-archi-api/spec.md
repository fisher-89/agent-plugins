# desktop-archi-api Specification（delta: archi-api-constructor-static-members）

## Purpose

本 delta 给 `<叶段>.schema.json` 的 `x-` 关键字面加**两个行为面构造**，使「这个类型怎么造」与「这个类型有哪些类型级方法可调」成为协议里可写的事实：

- **两个新键（仅 `x-class` 条目可用）**：`x-constructor`（规范构造入口，名字固定 `new`，只声明 `params`，隐式返回本类型）与 `x-static`（「名字 → 签名」的类型级静态方法表，返回本类型即工厂）。`object` 是纯数据、无行为、**非测试主体**，两个新键在其上出现即 `ApiError`。
- **两处静默的消灭（同批）**：`x-class` MUST 带 `type: "object"`（骨架必在场；否则标准校验器视作「任意值」，且 `type_block` 先判 `x-class` 会把 `type: "string"` 静默吞掉）；`x-class` MUST NOT 带 `additionalProperties`（语义互斥：字典 vs 仅允许方法的 trait；放行即静默丢，且会反噬出 `impl <trait>`）。
- **协议只固定语义**：构造入口的名字、有序入参表与隐式返回、静态方法的「类型级可调用成员」身份。目标语言语法（`pub fn` / `declare const` / `constructor`）只属于生成器；换语言或换写法不改协议。
- **零迁移面**：在案 11 份 schema 零 `x-class`、零新键 ⇒ 生成物逐字节不变、`INDEX.toml` 的 `categories` 口径不变。既有 `x-class` 语义（仅允许方法、带属性即错、`$defs` 位置）逐条不动，新增的只是骨架规则与两个新键。
- **不含**：`$defs` 成员按 `(owner, name)` 比对与「至少一条构造通路」的机器判定（二期，跨元素）；`object` 条目层 `additionalProperties` / `type: "array"` 的静默（另一变更的面）。

## ADDED Requirements

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

## MODIFIED Requirements

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

## Module Contract

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
