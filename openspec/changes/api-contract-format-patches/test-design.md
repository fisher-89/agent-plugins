# 测试设计: api-contract-format-patches

> **变更**: api-contract-format-patches
> **基于**: `proposal.md`（AC-1…AC-6、测试文件表）、`design.md`（变更清单 / 公共函数 API 表 / 类型定义 / D1–D15）
> **范围**: design「新增文件」为空表，「修改文件」六份源码 + `packages/desktop/package.json` 一行版本；删除文件为零 ⇒ 被测模块恰六份源码。

**测试框架与布局**（按既有测试文件实测）：

- 框架：Rust 内建 `#[test]`（工作区以 `cargo nextest` 运行，见 `reports/baseline/packages_desktop_src-tauri_rust/nextest.toml`）；无外部断言库、无 dev-dependency。
- 断言风格：`assert!` / `assert_eq!` / `panic!`，消息为中文并带上下文（渲染文本整段带入 `{text}`）。
- 布局：测试文件为同层兄弟 `*_test.rs`，由 `api/mod.rs`（`contract_test` / `mod_test` / `schema_test`）、`api/emit/mod.rs`（`ts_test` / `rust_test`）、`api/extract/mod.rs`（`mod_test`）的 `#[cfg(test)] mod <name>_test;` 声明；扩展名 `.rs`。
- 零 mock 传统：被测模块全为纯函数出 `String` / `ApiType` / `Vec`，唯一进程边界是 `mod_test` / `extract/mod_test` 的文件系统（std 自建 `TempDir` RAII，非替身）。

## 验收范围

> 路径前缀 `packages/desktop/src-tauri/crates/core/archi/src/` 省略；单元格内 `→` 左侧为被测源文件、右侧为其测试文件。

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 (a) | 可空轴六组（两元素数组 / 三元素数组 / `anyOf`-null / `oneOf`-null / 非 `required` 叠加 / 数组元素位）三面同真同假：`schema_type` = `Optional` ⟺ TS 含 `\| null` ⟺ Rust 含 `Option<` | `api/extract/mod.rs` → `api/extract/mod_test.rs`（对拍主体）；`api/emit/ts.rs` → `api/emit/ts_test.rs`；`api/emit/rust.rs` → `api/emit/rust_test.rs` |
| AC-1 (b) | 可选轴两组：`schema_type` 同值（`OwnedString`，不扩参）；TS `a: string;` / `b?: string;`；Rust `pub a: String,` / `pub b: Option<String>,` | `api/extract/mod.rs` → `api/extract/mod_test.rs`；`api/emit/ts.rs` → `api/emit/ts_test.rs`；`api/emit/rust.rs` → `api/emit/rust_test.rs` |
| AC-1 (c) | 纯 `null` 一组：`Optional(Unknown("null"))` / TS `null` / Rust `serde_json::Value`，且不参与 (a) 的等价式 | `api/extract/mod.rs` → `api/extract/mod_test.rs`；`api/emit/ts.rs` → `api/emit/ts_test.rs`；`api/emit/rust.rs` → `api/emit/rust_test.rs` |
| AC-1 泄漏面 | TS 九组 `unknown` 零命中；Rust 除 (c) 外 `serde_json::Value` 零命中 | `api/emit/ts.rs` → `api/emit/ts_test.rs`；`api/emit/rust.rs` → `api/emit/rust_test.rs` |
| AC-2 | TS：非 `required` → `{field}?: T`；显式 null → `\| null` 至多一次；纯 `null` → `null`；数组元素位 → `(T \| null)[]`；金样 `unknown` 零命中 | `api/emit/ts.rs` → `api/emit/ts_test.rs` |
| AC-3 | Rust：非 `required` → `Option<T>`；显式 null → `Option<T>`；纯 `null` → `serde_json::Value`（非 `()`）；`Option<Option<` 零命中 | `api/emit/rust.rs` → `api/emit/rust_test.rs` |
| AC-4 | `data-testid` 首层属性键 ⇒ 装载期报错含 `$defs.<name>.properties.data-testid` 与出路句；`required` 悬垂 ⇒ 点名 `required[<i>]`；自洽契约通过；生成物面零引号键 | `api/contract.rs` → `api/contract_test.rs`；`api/emit/ts.rs` → `api/emit/ts_test.rs`（行为层零引号键） |
| AC-5 | 词表外值 / 非字符串值 ⇒ 契约错误并列四值；`note` 不计入；`INDEX.toml` 的 `degraded` = 全深度标记出现次数；演练 fixture 三处 ⇒ `3` | `api/schema.rs` → `api/schema_test.rs`；`api/mod.rs` → `api/mod_test.rs` |
| AC-6 | `App.Backend.Infra.ArchiRuntime` 的四类生成物与 `INDEX.toml` 逐字不变 | `api/mod.rs` → `api/mod_test.rs`（fixture 级零回归）+ 现网契约生成物（门禁级，见不可测试项） |

> 被测模块清单来源：design「变更清单 · 修改文件」表（新增文件表为空、删除文件为零）。`packages/desktop/package.json` 只有一行版本配置、无运行时行为，不纳入单元测试面（见 `## 不可测试项`）。

## 单元测试

### `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod_test.rs`

#### 待测功能

- nullable_breakdown(): 可空形状识别的唯一单点——`Some` ⟺ schema 含显式 null；`non_null: None` = 纯 null 档（无第二个分支可取）；三分支沿用「取首个非空分支」语义
- schema_type(): 改经 `nullable_breakdown` 取非空分支；入参不扩（不收 `required`），对外结果与改动前逐值相同

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `nullable_breakdown` | 正向 | `{"type":["string","null"]}` → `Some`：`explicit_null == true`，`non_null` 为「外层克隆 + `type` 换成 `string`」的窄化视图（`Cow::Owned`） | 单例 |
| `nullable_breakdown` | 正向 | `anyOf` 含 `{"type":"null"}` 分支 → `Some`，`non_null` 借自首个非空分支（`Cow::Borrowed`，零克隆）；`oneOf` 同判 | 表驱动 |
| `nullable_breakdown` | 正向 | `{"type":"null"}` → `Some { non_null: None, explicit_null: true }`（纯 null 档） | 单例 |
| `nullable_breakdown` | 异常 | 非 nullable 形态（`{"type":"string"}` / `{"$ref":"#/$defs/T"}` / `{"outcome":{…}}` / `{}`）→ `None`（交回常规分派，零误判） | 表驱动 |
| `nullable_breakdown` | 异常 | `anyOf` / `oneOf` 非数组、空数组、数组内无 null 分支 → `None` | 表驱动 |
| `nullable_breakdown` | 边界 | 三元素 `["string","integer","null"]` → 只取首个非空分支（`integer` 静默丢弃，R-3 留白）；`["null","string"]` 仍取 `string` | 表驱动 |
| `nullable_breakdown` | 边界 | `{"type":[]}` → 外层 `None`（空 kind 表不含 `"null"`，交回常规分派）；`{"type":["null"]}` / `{"type":["null","null"]}` → 纯 null 档（`non_null == None`）；`anyOf: [{"type":"null"}]` 同档 | 表驱动 |
| `nullable_breakdown` | 边界 | 非对象 schema（`json!(null)` / `json!(42)` / `json!("string")` / `json!([])`）→ `None`，零 panic | 表驱动 |
| `schema_type` | 正向 | nullable 三分支 × `Position` 三值：`Param` → `Optional(Str)`、`Return` / `Field` → `Optional(OwnedString)`（位置语义不因 nullable 改变） | 表驱动 |
| `schema_type` | 异常 | 判序不变：`{"$ref":…,"type":["string","null"]}` → `Unknown(ref)`；`{"outcome":…,"type":…}` → `Outcome`（nullable 分支在其后） | 表驱动 |
| `schema_type` | 边界 | `{"type":"null"}` → `Optional(Unknown("null"))`；`{"type":["array","null"],"items":{"type":"string"}}` → `Optional(List(OwnedString))`；`items` 内 nullable → 元素位 `Optional`（嵌套保位置） | 表驱动 |
| `schema_type` | 边界 | 可选轴不进判据面：同一片段 `{"type":"string"}` 无论必填 / 可选语境均取 `OwnedString`；`schema_type` 调用面零 `required` 入参（签名不扩） | 单例 |
| 三轴对拍（跨模块：`extract::schema_type` ↔ `emit::ts::render` ↔ `emit::rust::render`） | 正向 | (a) 可空轴六组：`["string","null"]` / `["string","integer","null"]` / `anyOf`-null / `oneOf`-null / 非 `required` 与显式 null 叠加 / 数组元素位 nullable——逐组断言 `schema_type == Optional` ⟺ TS 文本含 `\| null` ⟺ Rust 文本含 `Option<`（数组组在元素位对拍） | 逐组对拍（6 组） |
| 三轴对拍（同上） | 正向 | (b) 可选轴两组：同一 `$defs` 条目内 `a`（在 `required`，`{"type":"string"}`）与 `b`（不在 `required`，同一片段）——`schema_type` 两者同值 `OwnedString`；TS 为 `a: string;` / `b?: string;`（`a` 零 `?`、`b` 零 `\| null`）；Rust 为 `pub a: String,` / `pub b: Option<String>,`；断言不以「Rust 含 `Option<`」当可空判据 | 单例 |
| 三轴对拍（同上） | 边界 | (c) 纯 null 一组：`{"type":"null"}` 置于 `required` 字段位 → `Optional(Unknown("null"))` / TS `null` / Rust `serde_json::Value`（显式期望值，零 `()`）；该组不参与 (a) 的等价式 | 单例 |
| 三轴对拍（同上） | 异常 | 泄漏面：九组语料逐组取 TS 文本（`unknown` 零命中）与 Rust 文本（除 (c) 外 `serde_json::Value` 零命中） | 逐组对拍（9 组） |

> 组合用例挂靠：design「测试文件」表把三轴对拍语料归到 `extract/mod_test.rs`；链路发起方是判据单点 `schema_type`，两侧渲染为真实组合（真实 `contract::parse` + `schema::build_schema` + `emit::{ts,rust}::render`），describe 标题写链路方向，不新建独立集成测试章节。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数被测面） | 零 mock：`serde_json::json!` 真实构造 schema 片段，`ApiType` 真实比较 | `nullable_breakdown` / `schema_type` 全部用例 |
| `contract::parse` / `schema::build_schema` / `emit::{ts,rust}::render` | 真实调用（零替身；渲染器纯出 `String`） | 三轴对拍 9 组 |
| 文件系统 | 不适用（本文件零 IO，无 `TempDir`） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts_test.rs`

#### 待测功能

- interface_block(): 非 `required` 字段只出 `{field}?: {base}`（删去旧口径的 `format!("{base} | null")`）；`x-function` 字段仍在分支处提前 `continue`，非 `required` 也不加 `?`（D6）
- value_type(): 在 `$ref` / `common_type` 之后、`type` 分派之前插 nullable 分支——纯 null 档出 `null`、有非空分支经私有幂等 `nullable_of` 出 `T | null`（`| null` 的唯一追加点）；`array` 分支改调私有 `array_of` 出 `(T | null)[]`
- function_type(): 可选参数由 `ty.push_str(" | null")` 改为 `ty = nullable_of(ty)`（与 `value_type` 同一追加点，叠加后 `| null` 至多一次）；无 `returns` 仍出 `void`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `interface_block`（字段可选口径） | 正向 | `required=["a"]` → `a: string;` / `b?: number;`；`required=[]` 或缺省 → 全 `{field}?: T;`（改写既有 `render的非required字段为可空`） | 表驱动 |
| `interface_block` | 异常 | 非 `required` 字段文本零 `\| null`（旧口径 `T \| null` 不再产出；`\| null` 只由 `value_type` 出） | 单例 |
| `interface_block` | 边界 | `x-function` 字段（非 `required`）→ `f: (…) => R;` 零 `?`（可选轴不套方法面，D6）；`$ref` 非 `required` 字段 → `inner?: T;`（改写 `render不猜泛型语法` 的 `inner: T \| null;`） | 表驱动 |
| `value_type`（显式 null，有非空分支） | 正向 | `{"type":["string","null"]}` → `string \| null`；`required` 字段 `a: string \| null;`、非 `required` 字段 `c?: string \| null;`，每字段 `\| null` 出现次数恰 1 | 表驱动 |
| `value_type`（显式 null，有非空分支） | 异常 | `anyOf` / `oneOf` 含 null 分支同判出 `\| null`；三元素 `["string","integer","null"]` 只出首个非空分支（零 `number` 泄漏、零 `unknown`） | 逐组 |
| `value_type`（显式 null，有非空分支） | 边界 | 纯 `null` 档 `{"type":"null"}`：`required` 字段位 → `null`（MUST NOT `unknown`），非 `required` 位 → `{field}?: null`（可选符号与纯 null 档各落一处）；`$ref` / 映射规则优先于 nullable（判序与 `schema_type` 同序） | 表驱动 |
| `value_type`（数组元素位） | 正向 | `{"type":"array","items":{"type":["string","null"]}}` → `(string \| null)[]`（元素位加括号） | 单例 |
| `value_type`（数组元素位） | 异常 | 优先级错误形态 `string \| null[]` 零命中；元素非 nullable → `string[]`（零多余括号） | 表驱动 |
| `value_type`（数组元素位） | 边界 | 元素位为映射规则 union（`Outcome` → `{ ok: string } \| { err: string }`）→ `({ ok: string } \| { err: string })[]`（D7）；嵌套数组元素 nullable → `((string \| null)[])[]` | 表驱动 |
| `function_type`（可选参数面） | 正向 | `optional: true` 参数 → `a: string \| null`（`nullable_of` 幂等，唯一追加点） | 单例 |
| `function_type`（可选参数面） | 异常 | `optional: true` 且 schema 为 `{"type":["string","null"]}` → `a: string \| null` 恰一次（零 `\| null \| null`） | 单例 |
| `function_type`（可选参数面） | 边界 | 非 `optional` 参数零 `\| null`；无 `returns` → `void`（既有形态零回归） | 表驱动 |
| 渲染金样（演练草案 fixture，D.1 改写为词表形态） | 正向 | nullable 字段全部落 `\| null` / `null`，interface 文本 `unknown` 零命中 | 单例 |
| 渲染金样 | 异常 | 九组 nullable 语料逐组取渲染文本，`unknown` 零命中（泄漏面即失败） | 逐组 |
| 渲染金样 | 边界 | interface 字段行不以 `'` 开头（零引号键形态，AC-4 否决项机械面）；同一输入两次渲染逐字相等且为格式化器不动点（零尾随空白、无 tab、单引号字面量） | 表驱动 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数被测面） | 零 mock：渲染器纯出 `String`，零 IO / 零子进程 | 全部用例 |
| `contract::parse` / `schema::build_schema` | 真实调用（契约源字符串直传，零落盘） | 全部渲染用例 |
| `ApiMeta` | 真实值（`meta()` 取编译期常量与真实摘要，不构造替身类型） | 全部渲染用例 |
| 文件系统 / 时钟 / 子进程 | 不适用（本文件零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust_test.rs`

#### 待测功能

- struct_block(): 非 `required` 字段的 `format!("Option<{base}>")` 换成私有幂等 `optional_of(base)`；`x-function` 字段仍在分支处提前 `continue`（非 `required` 也不加 `Option<>`，D6）
- value_type(): 同位置插 nullable 分支——纯 null 档出 `FALLBACK_VALUE_TYPE`、有非空分支经 `optional_of` 出 `Option<T>`（叠加后至多一层，`Option<Option<` 零产出）
- variants_block(): `FALLBACK_VALUE_TYPE` 作「不可作负载」判据逐字不变——条目顶层 `oneOf` 含 `{"type":"null"}` 分支仍落 `Variant{i}`（D15 ②）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `struct_block`（字段可选口径） | 正向 | `required=["a"]` → `pub a: String,` / `pub b: Option<i64>,`；`required=[]` → 全 `Option<T>`（沿用既有 `render的非required字段为option`） | 表驱动 |
| `struct_block` | 异常 | 可空与可选叠加（`c` 非 `required` 且 `["string","null"]`）→ `pub c: Option<String>,`；全渲染文本 `Option<Option<` 零命中 | 表驱动 |
| `struct_block` | 边界 | `x-function` 字段（非 `required`）→ `pub f: fn(…) -> …,` 零 `Option<>`（可选轴不套方法面，D6） | 单例 |
| `value_type`（显式 null，有非空分支） | 正向 | `required` 字段 `{"type":["string","null"]}` → `Option<String>`；`Param` 位保留位置语义（`Option<&str>`） | 表驱动 |
| `value_type`（显式 null，有非空分支） | 异常 | 非空分支本身已带兜底（`{"type":["object","null"]}` 且无 `additionalProperties`）→ `Option<serde_json::Value>`（单层包装，零 `Option<Option<`） | 单例 |
| `value_type`（显式 null，有非空分支） | 边界 | 纯 `null` 档 `{"type":"null"}`：`required` 字段位 → `serde_json::Value`（既有兜底，显式期望值），非 `required` 位 → `Option<serde_json::Value>`（只多一层可选包装）；零 `()` | 单例 |
| `value_type`（数组元素位） | 正向 | `items: {"type":["string","null"]}` → `Vec<Option<String>>`；`Param` 位 → `&[Option<String>]` | 表驱动 |
| `value_type`（数组元素位） | 异常 | 全渲染文本 `Option<Option<` 零命中（含数组 / 字段 / 参数三面叠加） | 单例 |
| `value_type`（数组元素位） | 边界 | 元素非 nullable → `Vec<String>`（零多余 `Option`）；缺 `items` → 既有 `Vec<String>` 形态不变 | 表驱动 |
| `variants_block`（既有兜底判据） | 正向 | 条目顶层 `oneOf` 含 `{"type":"null"}` 分支 → 该分支仍落 `Variant{i}`（`FALLBACK_VALUE_TYPE` 判据逐字不变，D15 ②） | 单例 |
| `variants_block`（既有兜底判据） | 异常 | 对象型分支（无 `additionalProperties`）仍落 `Variant{i}`；带具体负载的分支仍出「归一后的变体名 + 负载」形态（既有 `render的oneof兜底分支仍落占位变体` 零回归） | 表驱动 |
| `variants_block`（既有兜底判据） | 边界 | 本变更新增的 nullable 前置分支不改「条目顶层 `oneOf`-含-null」的整体形态（仅纯 null 分支被断言；多分支完整表达为 D15 留白） | 单例 |
| 渲染金样（演练草案 fixture） | 正向 | nullable 字段全部落 `Option<…>`，字段名全为合法 Rust 标识符 | 单例 |
| 渲染金样 | 异常 | 该 fixture 渲染文本 `serde_json::Value` 零命中（除纯 null 档用例外的泄漏面） | 单例 |
| 渲染金样 | 边界 | 同一输入两次渲染逐字相等、格式化器不动点；`Option<Option<` 全文件渲染语料零命中 | 表驱动 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数被测面） | 零 mock：渲染器纯出 `String`，零 IO / 零子进程 | 全部用例 |
| `contract::parse` / `schema::build_schema` | 真实调用（契约源字符串直传，零落盘） | 全部渲染用例 |
| `ApiMeta` | 真实值（`meta()` 编译期常量），零替身 | 全部渲染用例 |
| 文件系统 / 时钟 / 子进程 | 不适用（本文件零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs`

#### 待测功能

- verify(): 既有三条双向 gate + 七族闸一 + 落点闸之上，同趟续判 `required` 元素集 ⊆ 同条目 `properties` 键集（悬垂即 `ApiError::Contract` 并点名 `$defs.<name>.required[<i>]`）；⑤ 族（属性键）文案带 JSON pointer 与族别化出路句；族别数仍七、公开入口仍一个

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `verify` | 正向 | `required ⊆ properties`（全字段必填 / 部分必填 / `required=[]` / 无 `required`）→ `Ok` | 表驱动 |
| `verify` | 正向 | 现网契约形态（`properties: {}` 且零 `required`）→ `Ok`，零误伤（AC-6 的判据面侧） | 单例 |
| `verify` | 异常 | `properties: {"name": …}` 而 `required: ["nmae"]` → `Err(ApiError::Contract)`，文案含 `$defs.Widget.required[0]` 与 `nmae` | 单例 |
| `verify` | 异常 | `required` 元素非字符串（`[42]` / `[true]` / `[null]`）→ `Err`，文案点名 `required[<i>]` 并经 `kind_of` 报形态（数字 / 布尔 / null） | 表驱动 |
| `verify` | 异常 | 条目缺 `properties` 而 `required` 非空 → `Err`，文案含 `required[0]`；首层属性键 `data-testid` → `Err`，文案含 `$defs.Widget.properties.data-testid` 与「`data-testid` 一类键不进契约」出路句 | 表驱动 |
| `verify` | 异常 | 同一 `$defs` 条目同时含非法属性键与悬垂 `required` → 报 ⑤ 族（同趟先判名字、后判闭合性，错误面稳定不漂移） | 单例 |
| `verify` | 边界 | `required` 整体非数组（`"required": "name"`）→ 维持既有静默（`Ok`）——design 待决问题 1 的显式留白，本期不加判据 | 单例 |
| `verify` | 边界 | `required` 指向 `x-function` 方法名（键在同条目 `properties` 全集内）→ `Ok`（闭合性按 `properties` 键集判，与 ① 族方法名判据互不吞并） | 单例 |
| `verify` | 边界 | 非 ⑤ 族（op id / `$defs` 键名 / 透传导出名 / 参数名 / `$ref` 末段名）报错文案零 `JSON pointer` 段（pointer 只由 ⑤ 族传入，其余族保留通用句） | 表驱动 |
| `verify` | 边界 | 既有七族判据零回归：各族非标识符仍报族别串与「ASCII 标识符」句；公开面仍恰 3 个 `pub fn`（不新增族编号、不拆第二个公开入口） | 表驱动 |
| `verify`（跨模块：`verify` → `emit::{ts,rust}::render`） | 正向 | `required: ["name"]` 的自洽契约：`verify` 通过后 TS 渲染 `name: string;`、Rust 渲染 `pub name: String,`（必填两侧同真） | 单例 |

> 组合用例挂靠：链路发起方是装载期 `verify`（先判「契约是否自洽」才轮到渲染），故「通过后两侧渲染为必填」落在本节；不新建独立集成测试章节。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数被测面） | 零 mock：`parse` / `verify` 接 `source: &str`，契约夹具以字符串直传（零落盘） | 全部用例 |
| schema 入参 | 真实 `build_schema` 产出；负例形态以 `serde_json::json!` 手写注入（`$defs` 缺定义 / 悬垂 `required` / 非法键） | 异常与边界用例 |
| 渲染（跨模块用例） | 真实 `emit::{ts,rust}::render`（零替身） | 「自洽契约两侧必填」用例 |
| 文件系统 | 不适用（本文件零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/schema_test.rs`

#### 待测功能

- DEGRADED_KINDS: 封闭四值词表（`foreignElement` / `noElement` / `unrepresentable` / `crossLanguage`），新增第五值 MUST 改 spec；MUST NOT 进 `common.json`
- degraded_marks(): 全深度单点扫描（`$defs` 条目自身、`properties` 值、`x-function.params[].schema` 及更深），pointer 指到标记本身；锚点是 `x-common.degraded`（非任意名为 `degraded` 的键）
- verify_defs(): 末尾续判 `x-common.degraded` 取值 MUST ∈ 词表（非字符串亦拒），错误点名 pointer 并列四值；既有五条 `$defs` 规则与 `x-class` 判据逐字不动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DEGRADED_KINDS` | 正向 | 逐值断言顺序与内容：`["foreignElement", "noElement", "unrepresentable", "crossLanguage"]` | 逐值 |
| `DEGRADED_KINDS` | 边界 | 长度恰 4、零重复、每值为非空 ASCII 词（驼峰，无空格 / 连字符）；常量本身无异常面（取值合法性由 `verify_defs` 用例承担） | 表驱动 |
| `degraded_marks` | 正向 | `$defs` 条目自身标记 → pointer `$defs.Widget.x-common.degraded`，值取原文 `&Value` | 单例 |
| `degraded_marks` | 正向 | 属性位标记 → pointer `$defs.Widget.properties.session.x-common.degraded`（旧口径看不见的落点） | 单例 |
| `degraded_marks` | 正向 | 深层标记：`properties.x.items`、`properties.x.x-function.params[0].schema`、`additionalProperties` → pointer 含 `.items` 段与数组段 `[0]` | 表驱动 |
| `degraded_marks` | 异常 | 根节点自身标记 → pointer `$.x-common.degraded`；标量 / 数组根 → 空 `Vec`（零 panic） | 表驱动 |
| `degraded_marks` | 异常 | `x-common` 非对象（字符串 / 数组 / null）或对象但无 `degraded` 键 → 不计入 | 表驱动 |
| `degraded_marks` | 边界 | 数据字段名恰为 `degraded`（`properties.degraded = {"type":"string"}`）→ 不计入（锚点判据；松锚点会把数据字段计成债） | 单例 |
| `degraded_marks` | 边界 | 标记值为 `true` / 数字 / `null` → 仍被扫出（扫描不看取值类型；合法性归 `verify_defs`）；`x-common.note` 永不计入 | 表驱动 |
| `degraded_marks` | 边界 | 同一 schema 多处标记（条目自身 + 属性位 + 深层）→ 逐处各一条，pointer 集合与出现点逐一对应；同一数组内按索引序（`[0]` 先于 `[1]`）；零标记 → 空 `Vec` | 表驱动 |
| `verify_defs` | 正向 | 四值逐个通过（同一契约的条目自身 + 属性位各一） | 逐值 |
| `verify_defs` | 正向 | 零标记 / 仅 `x-common.note = "待剪枝：…"` / 仅 `x-common.unit` → `Ok`，`note` 零校验 | 表驱动 |
| `verify_defs` | 异常 | `x-common.degraded = "typo"` → `Err(ApiError::Contract)`，文案含该标记 pointer 并并列四值（`foreignElement / noElement / unrepresentable / crossLanguage`） | 单例 |
| `verify_defs` | 异常 | `x-common.degraded = true` → `Err`，文案含 pointer、`kind_of` 形态（布尔）与四值；`null` / 数字 / 数组 / 对象同拒 | 表驱动 |
| `verify_defs` | 异常 | 深层非法值（`items` / `params[].schema` 内）→ `Err` 并点名该 pointer（校验面与计数面同源） | 表驱动 |
| `verify_defs` | 边界 | 既有规则零回归：嵌套 `$defs` / `$ref` 非顶层 / 同名两次 / `x-class` 带属性 / 透传条目带结构关键字各自仍报既有错误（新增取值校验不遮蔽） | 表驱动 |
| `verify_defs` | 边界 | 同契约既有五条违例 + 非法 `degraded` 并存 → 报既有规则（取值校验排在既有规则之后，错误面稳定）；缺 `$defs` / 空对象 → 既有错误不变 | 表驱动 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数被测面） | 零 mock：`verify_defs` 接 `&Value`，非法形态以 `serde_json::json!` 直接构造 | `verify_defs` / `degraded_marks` 全部用例 |
| `contract::parse` / `build_schema` | 真实调用（正向用例的真实 schema 来源） | 四值通过 / 条目自身标记用例 |
| `toml_edit` | 真实反解析（仅既有 `build_index` 用例，本变更不改口径） | 零回归用例 |
| 文件系统 | 不适用（本文件零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`

#### 待测功能

- index_entries(): `INDEX.toml` 的 `degraded` 由「`$defs` 值的 `x-common.degraded` 条目级 filter」改为 `schema::degraded_marks(value).len()`（全深度标记出现次数），零第二套遍历；键集 / 类型 / 顺序不变
- render_all(): 入口签名与产物集合逐字未变；`INDEX.toml` 的 `degraded` 键集 / 类型 / 顺序不变，取值随 `index_entries` 的口径改动
- load_contracts(): 装载链先经 `contract::verify` + `schema::verify_defs`；词表外值或非字符串的 `x-common.degraded` ⇒ `Err(ApiError::Contract)`，零半真报告、零生成物

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `INDEX.toml` 的 `degraded`（新口径：标记出现次数） | 正向 | 演练草案形态 fixture（`send` 参数位、`session` 属性位、`sessions.items` 属性位三处标 `x-common.degraded`，条目自身零标记）→ `degraded = 3`（旧条目级口径为 0） | 单例 |
| `INDEX.toml` 的 `degraded` | 正向 | 改写既有 `render_all的index_degraded派生`：fixture 由布尔 `"degraded": true` 改为词表字符串 `"noElement"`；两条条目自身标记 → `degraded = 2` | 单例 |
| `INDEX.toml` 的 `degraded` | 异常 | 词表外值（`"typo"`）与非字符串值（`true`）→ `load_contracts` 直接 `Err(ApiError::Contract)`（零半真报告、零生成物） | 表驱动 |
| `INDEX.toml` 的 `degraded` | 边界 | 零标记 → `degraded = 0` 且键仍输出（不省略）；仅 `x-common.note = "待剪枝：…"` 的 bootstrap 草稿 → `0` 且契约装载成功 | 表驱动 |
| `INDEX.toml` 的 `degraded` | 边界 | 同一元素多处标记（条目自身 + 属性位 + `params[].schema` 深层）→ 计数等于出现次数（累加，不是条目数）；同一快照两次 `render_all` 逐字相等 | 表驱动 |
| `INDEX.toml` 的 `degraded` | 边界 | `absent`（契约文件缺失）元素 → `degraded = 0`，键仍输出（口径改动不引入缺键形态） | 单例 |
| 校验与计数同源（跨模块：`schema::degraded_marks` ↔ `verify_defs` ↔ `index_entries`） | 正向 | 同一 schema 三处合法标记：装载（`verify_defs`）通过且 INDEX 计数 = 3（一处改口径两处随动，零第二套遍历） | 单例 |
| `INDEX.toml` 零回归（AC-6） | 正向 | 现网 `App.Backend.Infra.ArchiRuntime` fixture（`properties: {}`、零 `required`、零 `degraded`）→ `types` / `ops` / `degraded = 0` 与改动前逐字相同；该元素的 `.i.rs` / `.schema.json` 渲染文本零 nullable 相关变化 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（模型文件 / 契约 `.md` / 生成物根） | 真实 `TempDir` RAII（std 自建，无 dev-dependency；是临时目录而非替身类型） | 全部 `load_contracts` / `render_all` 用例 |
| `foundation::layout` 路径派生 | 真实派生（不 stub、不传假路径） | 路径装配与 INDEX 落点用例 |
| 解析 / 装配 / 判据 / 渲染 / 取数 | 真实组合（编排语义即被测对象，替身会引入第二事实源） | 全部用例 |
| 时钟 / 网络 / 子进程 | 不适用（全链零外部依赖） | 全部用例 |

## 不可测试项

| 对象 | 原因 | 承接方 |
|---|---|---|
| `packages/desktop/package.json` 的 `version`（`0.6.42` → `0.6.43`） | 纯配置行，无运行时行为可断言；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` crate 版本行 MUST NOT 随动 | 版本纪律与 release workflow（`.github/workflows/desktop-release.yml`）机械承接 |
| `.dev-team/architecture/api/**` 与 `packages/desktop/src/types/generated/**` 的零 diff（AC-6） | 生成物入仓门禁按 `api:check` 读盘比对判定，属机械静态步骤 | 桌面端 static-check（`server:check` → `bindings:export` → `api:check` → `client:check`）；行为层等价断言落在 `mod_test` 的现网契约 fixture |
| `.i.ts` 生成物面零引号键（AC-4 的 `grep -c "^  '"` 机械面） | 对已入仓生成物的文本扫描，非单元测试对象 | static-check 的 grep 判定；行为层等价断言在 `ts_test`「interface 字段行不以 `'` 开头」 |
| `api/check.rs`、`api/common.json`、`crates/tools/archi-api-gen/**`、`packages/desktop/src/**` | design 明令零改动（零新判据 / 零 finding kind / 词表不进映射规则资产 / 零 IPC 面变化），无被测代码 | 无需测试；相关门禁为 `server:check` / `client:check` |
| 「新增第五个降级值 MUST 改 spec」与「必须写 `degraded` 的机械强制」 | 前者是文档级纪律（运行时只体现为四值常量的拒收行为）；后者是 design D15 ③ 的显式留白（需跨元素可达性知识，属 `check` 面） | 前者由 `DEGRADED_KINDS` 的四值用例间接钉住；后者本期无实现、不设用例（避免把留白伪装成已实现） |
| 多分支 union 完整表达（`["string","integer","null"]`）、`required` 非数组判据、条目顶层 `oneOf`-含-null 的两侧收敛、`x-function` 字段可选轴 | design D15 / 待决问题 1–4 显式不做；本期只保留既有单分支语义与既有形态 | 留待后续变更裁决；本设计仅以既有形态的零回归用例钉住现状（`schema_type` 三元素组、`required` 非数组静默、`Variant{i}` 兜底、函数指针字段零 `?` / 零 `Option<>`） |
