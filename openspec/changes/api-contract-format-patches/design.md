# 设计: api-contract-format-patches

> **变更**: api-contract-format-patches
> **日期**: 2026-11-14
> **基于**: `proposal.md`（三个格式缺口 P1/P2/P3 + 变更范围 + AC-1…AC-6 + R-1…R-8）、`explore.md`（D.3.1 / D.3.2 / D.3.3 三补丁与 D.4 / D.5 边界）与 `specs/desktop-archi-api/spec.md`（2 ADDED + 2 MODIFIED requirement 与 Module Contract 表）
> **性质**: 契约**格式面**三处收口到单点——可空形状识别（`nullable_breakdown`）、`required ⊆ properties` 自洽闸、`x-common.degraded` 封闭词表与全深度债表扫描。零新增判据种类、零新增 IPC、零契约资产改动、零新增文件；六处源码文件 + 版本行。

---

## 起点实况核实（工作树实测；前缀省略为 `packages/desktop/src-tauri/crates/core/archi/src/api/`）

| 面 | 实测 |
|---|---|
| `extract/mod.rs`（385 行） | `schema_type` `:277`；`nullable_inner` `:317`（三分支：`anyOf`/`oneOf` 含 `{"type":"null"}` → 取**首个非空分支**；`"type": "null"` → `Unknown("null")`；`type` 数组去 null 后取首个 kind 并**克隆窄化**外层对象）；`is_null_schema` `:354`；`use` 区无 `std::borrow` |
| `emit/ts.rs`（395 行） | `interface_block` `:199`——非 `required` 字段一律 `format!("{base} | null")`（`:220`）；`x-function` 值在 `:209` 提前 `continue`（不参与可选轴）；`function_type` `:258`——`optional: true` 的参数 `ty.push_str(" | null")`（`:275`，`| null` 的第二个产出点）；`value_type` `:324`——`$ref` → `common_type` → `type` 字符串分派，数组 `format!("{items}[]")`（`:345`，无括号规则） |
| `emit/rust.rs`（468 行） | `struct_block` `:165`——非 `required` 字段 `format!("Option<{base}>")`（`:187`，**零幂等**：nullable 的 base 已是 `Option<T>` 即会出 `Option<Option<T>>`）；`x-function` 值在 `:174` 提前 `continue`；`FALLBACK_VALUE_TYPE` `:237`（`variants_block` `:245` 以它作「不可作负载」判据）；`value_type` `:355`——同 ts 的三段结构 |
| `contract.rs`（925 行） | `verify` `:273`——三条双向 gate → 闸一七族（`D45` 的 ①…⑦ 枚举注释**在 `verify` 体内**）→ 落点闸 `:457`；⑤ 族 `:397` 遍历 `schema["$defs"]` 的**值**（不取条目名 ⇒ 无 pointer）；`property_names` `:628`（滤掉 `x-function` 项）；`identifier_error` `:567`（族别 + 名字 + fence + 行，零 pointer）；`is_identifier` `:904`；公开面恰三：`parse` / `verify` / `is_constructor` |
| `schema.rs`（349 行） | `IndexEntry.degraded` `:52` doc = 「`x-common.degraded` 条目数」；`verify_defs` `:150`（五条 `$defs` 规则 + 透传纯 `$ref` + `x-class` 仅方法 + 嵌套 `$ref` 走查）；无任何词表常量 |
| `mod.rs`（965 行） | `index_entries` `:489`，`degraded` 在 `:512`–`:525`——`$defs` 值上 `.get("x-common").get("degraded").is_some()` 的**条目级**计数（属性位看不见） |
| 现网契约资产 | `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md`：`file_stem` 一个 op、`LocalArchiCheck` 为 `properties: {}` 且零 `required`、零 `x-common.degraded`；`INDEX.toml` 该元素 `degraded = 0` |
| 既有机械锚点（**不可被动摇**） | `contract_test` 断言 `contract.rs` 公开面恰 3 个 `pub fn`；断言七族族别串与 ①…⑦ 编号在 `verify` 体内、`methods_of(` / `function_param_names(` 的文件级计数；`emit/ts_test.rs:1349` 断言 `ts.rs` 的 `get("x-function")` 恰 2 处；`emit/ts_test.rs:1330` 断言 `value_type` 体内 `"x-function"` 零命中 |
| 编码了旧口径的既有断言（改写归测试相位） | `emit/ts_test.rs:144`（非 `required` → `T \| null`）、`:748`（`inner: T \| null;`）；`api/mod_test.rs:1387`（doc「条目数」）与 `:1406`（fixture `"degraded": true` 布尔）；`contract_test` 的 ⑤ 族 `gate1_error`（新文案仍含族别串 / 名字 / `ASCII 标识符` / 文件名，故该断言本身不红） |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **`non_null` 用 `Cow<'a, Value>` 而非 `&'a Value`**（spec Module Contract 的类型示意）。`type` 数组形态（`["string","null"]`）的非空分支不是 schema 里的某个子值——它是「外层对象克隆 + `type` 换成首个非空 kind」的**窄化视图**（`nullable_inner` 今天就是这么做的），没有可借用的 `&Value`。落为 `Option<Cow<'a, Value>>`：`anyOf`/`oneOf` 分支走 `Borrowed`（零克隆），`type` 数组形态走 `Owned`（与今天同一次克隆）。可观察契约与 spec 逐条一致：`None`（外层 `Option`）= 非可空；`Some { non_null: None }` = 纯 null 档；`Some { non_null: Some(..) }` = 有非空分支。
2. **`degraded_marks` 返回 `Vec<(String, &Value)>` 而非 `Vec<(String, &str)>`**（spec Module Contract 的示意）。校验面必须拒收**非字符串**值（`true`）并点名其形态，`&str` 拿不到形态；计数面只用 `.len()`，两者共用同一返回类型即可。
3. **`identifier_error` 增第 6 参 `pointer: Option<&str>`**（proposal 2.2 的行文「`identifier_error` 增族别化出路句」）。九个既有调用点补 `None`，⑤ 族传 `Some(pointer)`；`Some` 同时选「属性键」那句可执行出路（本函数是唯一「`$defs` 深处位置」的族，两件事同源）。族别串「properties 数据字段键」**逐字不变**——既有断言锚它。
4. **TS 的 `| null` 有第二个产出点**：`function_type` 对 `optional: true` 的入参加 `| null`（可选参数不能靠 `?`，见 D5）。若只去掉 `interface_block` 的拼接，`optional + nullable` 的参数会出 `string | null | null`。故 `| null` 的追加统一到私有幂等函数 `nullable_of`（`value_type` 前置分支与 `function_type` 各调一次）——「至多一次」在字段面与参数面同时成立。
5. **`x-function` 字段不参与可选轴**：两侧渲染器今天都在 `x-function` 分支提前 `continue`（`ts:209` / `rust:174`）。`x-class` 条目**结构上不带 `required`**（`verify_defs` 拒收），若把 `?` 套到方法面上，`export interface Foo { run?: () => void }` 会把「形状承诺」降级成「可选方法」。故维持「`x-function` 字段两侧都不落可选符号」——这是零回归，也是两侧对称的现状。
6. **`required` 的非字符串元素按悬垂同口径拒收**（非字符串不可能是 `properties` 的键，`kind_of` 报到文案里）；`required` 整体非数组时维持既有静默（不新增第二条规则，见待决问题 1）。
7. **自洽闸不占族编号**：闸一是「名字族」（①…⑦），`required ⊆ properties` 判的是**集合关系**，不是名字形态。故注释里不写 ⑧、不删既有 ①…⑦（`contract_test` 以 marker 判定它们在 `verify` 体内）。
8. **`verify_defs` 的错误面仍无文件前缀**：该函数的既有五条规则一律出「`$defs.<name>` …」形态的 pointer 文案，签名只有 `schema`。降级词表校验收同一形态（pointer 已足够定位到那一行），MUST NOT 为它改 `verify_defs` 签名。
9. **`common.json` 与词表是两套**：`common.json` 的 `degraded`（规则级标记，`CommonRule.degraded`）与契约资产的 `x-common.degraded`（类型级降级原因）各管一事、互不校验；`common.json` 摘要进全部生成物头注释，故词表 MUST NOT 进该资产（D11）。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 可空形状识别的单点 | `extract/mod.rs` 新增 `NullableBreakdown<'a> { non_null: Option<Cow<'a, Value>>, explicit_null: bool }` + `pub(crate) fn nullable_breakdown(schema: &Value) -> Option<NullableBreakdown<'_>>`（三分支识别整体搬入，沿用「取首个非空分支」）；`nullable_inner` 降为薄壳（`Some(inner)` → `schema_type(inner.as_ref(), position)`；`None` → `ApiType::Unknown("null")`） | 判据面与两渲染面读同一份形状判断，才能消灭「判据绿而产物错」；`schema_type` 的对外结果与签名逐值不变（AC-1(b)(c) / R-3 的留白因此成立） |
| D2 | TS 前置分支与 `\| null` 的唯一追加点 | `value_type` 在 `$ref` / `common_type` 之后、`type` 分派之前插 nullable 分支：`non_null == None` → `"null"`；否则 `value_type(inner)` 经 `nullable_of`（幂等：文本已以 `\| null` 结尾即原样返回）。`function_type` 的可选参数改用同一 `nullable_of` | 与 `schema_type` 的判序（`$ref` → 映射规则 → nullable → type）同序；`nullable_of` 幂等使「叠加去重」在字段面与参数面同时机械成立（AC-2） |
| D3 | Rust 前置分支与 `Option<T>` 至多一层 | `value_type` 同位置插 nullable 分支：`non_null == None` → `FALLBACK_VALUE_TYPE`；否则 `value_type(inner)` 经 `optional_of`（幂等：已 `Option<…>` 原样返回）。`struct_block` 的可选包装改调 `optional_of` | 幂等单点是「`Option<Option<` 零产出」的唯一机械保证（AC-3）；`FALLBACK_VALUE_TYPE` 仍是纯 null 档的兜底（`variants_block` 的「不可作负载」判据因此逐字不动） |
| D4 | 纯 `null` 档单独定档 | `{"type": "null"}`：`schema_type` 不变（`Optional(Unknown("null"))`）；TS → `null`（不再是 `unknown`）；Rust → 既有 `serde_json::Value`（MUST NOT 出 `()`）。该档 MUST NOT 参与「`Optional` ⟺ `\| null` ⟺ `Option<`」对拍 | 该形态在现网语料零命中，两侧新口径都由本设计钉住（R-4），避免实现者自行发明 `()` 一类形态 |
| D5 | 可选轴只落渲染面 | `interface_block` 对非 `required` 字段只出 `{field}?:`（`| null` 零拼接、`?` 只此一处）；`struct_block` 出 `Option<T>`；`schema_type` **不扩参、不收 `required`** | 同一 schema 片段在必填位与可选位判出同值逻辑类型（AC-1(b)）；可选性的唯一判定点是字段级 `required_of`，进判据面会让签名比对引入语境差异 |
| D6 | `x-function` 字段不进可选轴 | 两侧维持「分流分支提前 `continue`」：`{field}: (…) => R` / `pub field: fn(…)->R`，非 `required` 也不加 `?` / `Option<>` | 事实校准 5：`x-class` 无 `required`，加 `?` 会把 trait 的方法面降级为可选方法；两侧「都不加」是对称的既有形态（零回归） |
| D7 | 数组元素位的括号 | TS 新增私有 `array_of(items)`：元素文本含顶层 ` \| ` 即 `({items})[]`，否则 `{items}[]`；`value_type` 的 `array` 分支改调它 | `(T \| null)[]` 是唯一正确形态（AC-2）；顺带修正 `Outcome` 映射（`{ ok: T } \| { err: string }`）落数组位时的既有优先级错 |
| D8 | 装载期 pointer 与出路句 | `identifier_error` 加 `pointer: Option<&str>`：`Some` 时文案追加 `；JSON pointer \`<pointer>\`` 并选属性键的出路句（「属性键必须是标识符；`data-testid` 一类键不进契约（前端契约须避开 DOM 测试属性）」）；⑤ 族调用点传 `$defs.<name>.properties.<key>`；`verify` 的 `$defs` 遍历改 `iter()` 以拿到条目名 | AC-4 要求文案含 `$defs.<name>.properties.data-testid` 与出路句；族别串与「ASCII 标识符」句逐字保留 ⇒ 既有 `gate1_error` 断言不红 |
| D9 | `required ⊆ properties` 同趟续判 | `verify` 的同一 `$defs` 遍历里，紧跟 ⑤ 族一行续判：`required` 的每个元素 MUST 是同条目 `properties` 的键；悬垂（含非字符串元素）→ 新私有 `required_dangling_error`，文案点名 `$defs.<name>.required[<i>]` 与该元素并给出路；私有 `required_elements(definition)` 供取原值 | 同一趟、同一公开入口（`verify`）、零新增族别编号（事实校准 7）；`required: ["nmae"]` 的静默「字段变可选」当场变红（AC-4 / R-5） |
| D10 | 两条明确否决 | 无引号键形态（`'data-testid': string`）、无 `x-key` 重命名映射——**代码零改动**，纪律落 spec | 二者都让同一 schema 两侧产物形态不同（D47 纪律）或引入第二套命名体系；机械面即「`.i.ts` 内 `grep -c '^  \''` 恒 0」 |
| D11 | 词表常量落 `schema.rs` | `pub(crate) const DEGRADED_KINDS: [&str; 4] = ["foreignElement", "noElement", "unrepresentable", "crossLanguage"]` | 与校验判据同处（spec 明令）；MUST NOT 进 `common.json`——该资产语义封闭为**映射规则**且其摘要进全部生成物头注释（事实校准 9） |
| D12 | 全深度单点扫描 | `pub(crate) fn degraded_marks(schema: &Value) -> Vec<(String, &Value)>`：一个私有递归，锚点是「对象自身含 `x-common` 且其值是对象且含 `degraded`」；pointer 指到**标记本身**（`$defs.<name>.properties.<field>.x-common.degraded`，数组段 `[i]`，根自身命中即 `$`） | 关键词锚定而非「任意名为 `degraded` 的键」——数据字段名合法可以是 `degraded`，松锚点会把它计成债；pointer 指到标记让错误面直接落到那一行 |
| D13 | 计数口径改「标记出现次数」 | `index_entries` 的 `degraded` = `schema::degraded_marks(schema).len()`（校验与计数各调同一函数一次，零第二套遍历）；`IndexEntry.degraded` doc 改述 | 真实降级落点在属性位（R-1）；现网唯一契约零标记 ⇒ 现网计数仍 `0`、`INDEX.toml` 零 diff（AC-6） |
| D14 | 与 IPC 面的 `degraded` 分离 | 契约资产词表与 `bindings.zod.ts` 的 `degraded("原因")` 两套机制，取值词表 MUST NOT 合并、MUST NOT 互相校验 | 前者是契约资产债表，后者是生成物 schema 面的哨兵（spec 明文条款） |
| D15 | 明确不做 | ① 多分支 union 的完整表达（`["string","integer","null"]` 仍只表达首个非空分支）；② `oneOf` 含 null 的 **`$defs` 条目顶层**形态（TS `union` / Rust 带负载 `Variant{index}` 维持现状）；③ 「必须写 `degraded`」的机械强制（需跨元素可达性，属 `check` 面，显式留白）；④ `required` 非数组形态的判据；⑤ `x-common.note` 的任何校验（自由注记）；⑥ `api/check.rs` / `common.json` / `tools/archi-api-gen/**` / 前端与 `src/types/generated/**` / `AGENT.md` / `CLAUDE.md` 的改动 | 逐条对应 proposal「不要修改」与 spec 的留白条款；① 改它会动判据面的签名比对（R-3），② 的形态与 AC 的「字段 / 参数 / 数组元素位」三面无关 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 可空形状单点（**改**） | 三分支识别（`type: "null"` / `type` 数组含 `"null"` / `anyOf`-`oneOf` 含 null 分支）→ `NullableBreakdown`；`schema_type` / 两 `value_type` 共用 | `crates/core/archi/src/api/extract/mod.rs` | `serde_json::Value`、`std::borrow::Cow`（新增） | 纯函数 |
| TS 渲染面（**改**） | nullable 前置分支、`\| null` 唯一追加点（`nullable_of`）、`?` 只由 `interface_block` 出、数组元素位括号（`array_of`） | `crates/core/archi/src/api/emit/ts.rs` | D1 的 `nullable_breakdown`、`Position` | 纯函数出 `String` |
| Rust 渲染面（**改**） | nullable 前置分支、`Option<T>` 幂等包装（`optional_of`）、纯 null 档保兜底 | `crates/core/archi/src/api/emit/rust.rs` | D1 的 `nullable_breakdown`、`FALLBACK_VALUE_TYPE` | 纯函数出 `String` |
| 装载期名字 / 自洽闸（**改**） | 七族名字闸（既有）+ ⑤ 族的 pointer 与出路句 + `required ⊆ properties` 同趟续判 | `crates/core/archi/src/api/contract.rs` | 既有 `is_identifier`、`super::kind_of` | 纯函数 |
| `$defs` 规则 + 债表扫描（**改**） | `DEGRADED_KINDS` 词表、`degraded_marks` 全深度扫描、`verify_defs` 取值校验 | `crates/core/archi/src/api/schema.rs` | 既有 `verify_defs` 骨架 | 纯函数 |
| 契约面装配（**改**） | `index_entries` 的 `degraded` 走 `schema::degraded_marks` 单点 | `crates/core/archi/src/api/mod.rs` | 上述 `schema` 出口 | 既有 |

依赖方向零新增边：`emit` → `extract` 的边已在案（`Position` 就从那里取）；`contract` / `schema` / `mod` 三条既有边不变；`core/archi` 仍零写盘、零 infra 依赖。

### 数据流（改后）

```
<FQN>.md ──parse──> Contract ──build_schema──> schema(Value)
                                     │
                     ┌───────────────┼────────────────────────┐
                     ▼               ▼                        ▼
        contract::verify      schema::verify_defs        render_all
        （七族 + ⑤pointer      （五条 $defs 规则 +        ├─ emit::ts / rust
          + required⊆properties） x-class + degraded     │   └─ value_type ─┐
                     │           全深度词表校验）          │                 ├─ nullable_breakdown
                     │                                   └─ INDEX.toml      │  （extract 单点）
                     │                                      └─ degraded = degraded_marks(..).len()
                     └─ 任一 Err ⇒ load_contracts 中止（零半真报告）
```

---

## 变更清单 (Change Inventory)

### 新增文件

<!-- 本变更零新增文件：proposal「变更范围 - 实现文件」六项全为修改，另加 package.json 一行版本。 -->

无。三处单点（`nullable_breakdown` / `required_elements` + `required_dangling_error` / `degraded_marks` + `DEGRADED_KINDS`）都落在既有文件的既有职责内，不新开模块——`emit::{ts,rust}` 与 `schema.rs` 已各自是「该层唯一落点」，为单函数新开模块会造出第二处需要 `pub mod` 出线的面。

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/api/extract/mod.rs` | ① `use std::borrow::Cow;`；② 新增 `pub(crate) struct NullableBreakdown<'a>`（两字段 + `#[derive(Debug, Clone)]`）；③ 新增 `pub(crate) fn nullable_breakdown(schema: &Value) -> Option<NullableBreakdown<'_>>`（三分支识别整体搬入，`anyOf`/`oneOf` 分支走 `Cow::Borrowed`，`type` 数组形态走 `Cow::Owned` 窄化克隆）；④ `nullable_inner` 改为经它取数（签名与语义不变）；⑤ 模块 doc 的「判序固定」段补一句「nullable 的形状识别单点在 `nullable_breakdown`，判据面与两侧渲染面共用」 | D1 / 事实校准 1 / AC-1 |
| `crates/core/archi/src/api/emit/ts.rs` | ① `use` 区把 `crate::api::extract::{Position}` 扩为 `{nullable_breakdown, Position}`；② `value_type` 在 `common_type` 之后、`type` 分派之前插 nullable 分支（纯 null → `"null"`；有非空分支 → `nullable_of(inner)`）；③ 新增私有 `fn nullable_of(ty: String) -> String`；④ 新增私有 `fn array_of(items: String) -> String`，`array` 分支改调它；⑤ `interface_block` 非 `required` 字段出 `{field}?: {base}`（删 `format!("{base} \| null")`），`x-function` 分支保持提前 `continue`；⑥ `function_type` 的可选参数改 `ty = nullable_of(ty)`；⑦ 模块 doc 第 4 行的「非 `required` → `T \| null`」改为「非 `required` → `{field}?: T`、显式 null → `T \| null`（至多一次）」 | D2 / D4 / D5 / D7 / 事实校准 4；`x-function` 读取点仍恰 2 处（既有断言） |
| `crates/core/archi/src/api/emit/rust.rs` | ① `use` 区扩为 `{nullable_breakdown, Position}`；② `value_type` 同位置插 nullable 分支（纯 null → `FALLBACK_VALUE_TYPE`；有非空分支 → `optional_of(inner)`）；③ 新增私有 `fn optional_of(ty: String) -> String`（幂等）；④ `struct_block` 的非 `required` 分支把 `format!("Option<{base}>")` 换成 `optional_of(base)`；⑤ 模块 doc 第 3–4 行的字段口径补一句「可选与可空叠加至多一层 `Option<T>`」 | D3 / D4 / D5 / 事实校准 4 |
| `crates/core/archi/src/api/contract.rs` | ① `verify` 的 `$defs` 遍历由 `defs.values()` 改为 `defs.iter()`（拿到条目名）；② ⑤ 族调用点传 `Some(&format!("$defs.{name}.properties.{field}"))`；③ `identifier_error` 增第 6 参 `pointer: Option<&str>`，文案追加 pointer 段与属性键出路句（其余九处调用补 `None`）；④ 同趟续判 `required ⊆ properties`（新私有 `required_elements` + `required_dangling_error`）；⑤ `verify` 体内闸一段注释补「自洽闸不新增族编号」一句，①…⑦ 各编号逐字保留；⑥ 模块 doc 头「三条双向 gate + 闸一 + 落点闸」段补「+ `required` 与 `properties` 的自洽闸」 | D8 / D9 / 事实校准 3、6、7；公开面仍恰 3 个 `pub fn` |
| `crates/core/archi/src/api/schema.rs` | ① 新增 `pub(crate) const DEGRADED_KINDS: [&str; 4]`（带「新增第五值 MUST 改 spec」doc）；② 新增 `pub(crate) fn degraded_marks(schema: &Value) -> Vec<(String, &Value)>` + 私有递归 `collect_degraded`（锚定 `x-common.degraded`，pointer 指到标记本身）；③ `verify_defs` 末尾逐个校验取值（非字符串 / 词表外 → `ApiError::Contract`，文案点名 pointer、列出四值并报 `kind_of`）；④ `IndexEntry.degraded` 的 doc 由「`$defs` 条目数」改为「`x-common.degraded` 标记出现次数（全深度，与 `degraded_marks` 同源）」；⑤ `verify_defs` doc 补第 6 条规则说明 | D11 / D12 / D13 / AC-5 |
| `crates/core/archi/src/api/mod.rs` | `index_entries` 的 `degraded` 由「`$defs` 值的 `x-common.degraded` 条目级 filter」改为 `schema::degraded_marks(value).len()`；`use schema::{...}` 行按需补 `degraded_marks`（或就地全路径调用，与既有风格一致） | D13；零第二套遍历 |
| `packages/desktop/package.json` | `"version": "0.6.42"` → `"0.6.43"`（单行） | 版本纪律；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

> **不在本清单的对象**（proposal「不要修改」+ 本设计 D15）：`.dev-team/architecture/api/**`（零契约改动，重生成应零 diff）、`api/check.rs`、`api/common.json`、`crates/tools/archi-api-gen/**`、`packages/desktop/src/**` 与 `src/types/generated/**`、`AGENT.md` / `CLAUDE.md`、`openspec/specs/desktop-archi-api/spec.md` 其余八个 requirement。**测试文件面**（`extract/mod_test.rs` / `emit/{ts,rust}_test.rs` / `contract_test.rs` / `schema_test.rs` / `mod_test.rs`）的用例本体归测试相位（proposal「测试文件」表），本设计只登记三处旧口径的波动（「起点实况核实」末两行与事实校准 4、5）。
>
> **跨语言映射规则资产零 diff**：`common.json` 的 `degraded` 是规则级标记（`CommonRule.degraded`），本变更的词表是**契约资产**的类型级降级原因，两套不互校（D14 / 事实校准 9）——故 `common.json` 与其摘要（进全部生成物头注释）逐字不变。

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `nullable_breakdown` | `crates/core/archi/src/api/extract/mod.rs` | 新增 | `pub(crate) fn nullable_breakdown(schema: &Value) -> Option<NullableBreakdown<'_>>` | 可空形状识别的唯一单点；`Some` ⟺ 含显式 null，`Some { non_null: None }` = 纯 null 档；沿用「取首个非空分支」语义 |
| `schema_type` | 同上 | 修改（内部实现，对外逐值不变） | `pub fn schema_type(schema: &Value, position: Position) -> ApiType` | 改经 `nullable_breakdown` 取非空分支；**签名不扩参**（可选轴不进判据面，AC-1(b)） |
| `verify` | `crates/core/archi/src/api/contract.rs` | 修改（同趟续判） | `pub fn verify(contract: &Contract, schema: &Value) -> Result<(), ApiError>` | 新增 `required ⊆ properties` 自洽规则；⑤ 族文案带 pointer 与出路句；族别数仍七、公开入口仍一个 |
| `verify_defs` | `crates/core/archi/src/api/schema.rs` | 修改（末尾续判） | `pub fn verify_defs(schema: &Value) -> Result<(), ApiError>` | 新增 `x-common.degraded` 取值校验（调 `degraded_marks` 一次）；既有五条 `$defs` 规则与 `x-class` 判据逐字不动 |
| `degraded_marks` | 同上 | 新增 | `pub(crate) fn degraded_marks(schema: &Value) -> Vec<(String, &Value)>` | 全深度扫描单点（`$defs` 条目自身、`properties` 值、`x-function.params[].schema` 及更深）；校验与计数各调一次，零第二套遍历 |
| `DEGRADED_KINDS` | 同上 | 新增（常量） | `pub(crate) const DEGRADED_KINDS: [&str; 4]` | 封闭降级词表：`foreignElement` / `noElement` / `unrepresentable` / `crossLanguage`；MUST NOT 进 `common.json` |

> 私有函数（`ts::nullable_of` / `ts::array_of` / `rust::optional_of` / `contract::required_elements` / `contract::required_dangling_error` / `schema::collect_degraded`）按本表口径不列；`index_entries` 亦为私有，其行为变化由 D13 与 `IndexEntry` 的 doc 承载。

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `NullableBreakdown<'a>` | `crates/core/archi/src/api/extract/mod.rs` | 新增 | `{ pub non_null: Option<Cow<'a, Value>>, pub explicit_null: bool }`，`#[derive(Debug, Clone)]`；语义：`non_null` = 可渲染的非空分支（`None` = 纯 null 档），`explicit_null` = 显式 null 的存在（渲染面 `\| null` 的唯一依据；现行三分支识别下 `Option::Some` 蕴含其为 `true`，独立成字段是为了让「有无 null」与「非空分支是什么」各只被读一次） |
| `IndexEntry` | `crates/core/archi/src/api/schema.rs` | 修改（仅 doc 语义，字段集 / 类型 / 顺序不变） | `degraded: usize` 的语义由「`$defs` 条目自身的标记条目数」改为「`x-common.degraded` 标记的**出现次数**（全深度，含属性位与 `x-function` 参数位）」；`build_index` 的写出形态零改动 |

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.43`（现值 `0.6.42`，+1 patch） | `tauri.conf.json` 经 `"version": "../package.json"` 自动跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

> 配置项零新增：`.dev-team/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像）、`KNOWN_KEYS`、`tauri.conf.json`、`capabilities/default.json` 全部零 diff；`package.json` 的 `api:export` / `api:check` / `check` 三段命令串逐字不变。

<!-- 路由 / API 设计 (Route / API Design) 整段省略：本变更零新增 IPC 命令、零新增 HTTP 端点、零新增 CLI 子命令（tools/archi-api-gen 的三子命令形态与退出码逐字不变），改动全在 core 判定面与渲染面的进程内函数。 -->

---

## 数据模型 (Data Model)

本变更零新增持久化对象，只动两处**语义精度**与一处进程内类型。

### 契约资产（手写面，零改动）

| 键 | 形态 | 本变更带来的约束 |
|---|---|---|
| `$defs.<name>.required[]` | 字符串数组 | 每个元素 MUST 是同条目 `$defs.<name>.properties` 的键（悬垂即 `ApiError::Contract`，pointer `$defs.<name>.required[<i>]`） |
| `$defs.<name>.properties` 的键 | 字符串（对象键） | MUST 是 ASCII 标识符（既有 ⑤ 族），错误文案新带 pointer `$defs.<name>.properties.<key>` 与出路句 |
| 任意对象位 `x-common.degraded` | 字符串 | MUST ∈ `DEGRADED_KINDS`（四值）；非字符串亦拒收；`x-common.note` 不受任何约束、不计入债表 |

### 生成物 `INDEX.toml`（键集 / 类型 / 顺序不变）

| 列 | 类型 | 语义（改后） | 现网取值 |
|---|---|---|---|
| `degraded` | 整数 | 该元素 `schema.json` 内 `x-common.degraded` 标记的出现次数（全深度；与 `schema::degraded_marks` 同源） | `App.Backend.Infra.ArchiRuntime` = `0`（逐字不变） |

### 进程内类型

| 类型 | 用途 | 生命周期 |
|---|---|---|
| `NullableBreakdown<'a>` | 一次形状判断的结果（借用或窄化克隆） | 单次 `schema_type` / `value_type` 调用内 |
| `(String, &'a Value)`（`degraded_marks` 元素） | 标记的 pointer 与原文值 | 单次 `verify_defs` / `index_entries` 调用内 |

**持久化面零扩散**：本变更不产生新文件、不改文件落点、零删除面；`render_all` 的产物集合与路径由既有 `paths::*` 单点产出，逐字不变。

---

## 验收标准回应

| AC | 落点 | 归属阶段（tasks） |
|---|---|---|
| AC-1 判定单点一致（三轴分列） | D1（形状识别单点，`schema_type` 逐值不变 = (b)(c) 两轴的判据面侧）+ D2/D3（两渲染面前置分支 = (a) 两渲染面侧）+ D5（可选轴不进判据面 = (b)）+ D4（纯 null 档独立、不参与等价式 = (c)）；泄漏面由 D2/D4 的「TS 纯 null 出 `null` 而非 `unknown`」与 D3 保兜底共同决定；D15 ① 登记多分支 union 的留白 | 一 / 二 |
| AC-2 TS 产物正确 | D2（`nullable_of` 幂等 ⇒ `\| null` 至多一次；`?` 只由 `interface_block` 出）+ D4（纯 null → `null`）+ D7（`(T \| null)[]`） | 二 |
| AC-3 Rust 产物正确 | D3（`optional_of` 幂等 ⇒ `Option<Option<` 零产出；纯 null 档保 `serde_json::Value`，MUST NOT 出 `()`） | 二 |
| AC-4 属性键闸可执行 | D8（pointer + 出路句）+ D9（`required` 悬垂 + pointer `required[<i>]`）+ D10（两条否决；生成物面零引号键） | 三 |
| AC-5 词表封闭 | D11（`DEGRADED_KINDS`）+ D12（`degraded_marks` 单点，非字符串亦拒）+ D13（计数 = 标记数；`note` 零计入） | 四 |
| AC-6 零回归 | D13 + D1/D3（`ArchiRuntime` 零 nullable、零 `required` 悬垂、零 `degraded` ⇒ `.i.rs` / `.schema.json` / `INDEX.toml` 逐字不变）；零 diff 判定由桌面端 `pnpm api:check` 承接 | 五（手工核对 + 移交门禁） |

> **不列为 AC 的流程项**（沿 proposal 末尾声明）：门禁四段（`server:check` → `bindings:export` → `api:check` → `client:check`）与版本纪律各有现成落点（`desktop-archi-api` 既有 requirement），本 delta 不改该 requirement，故不在本设计里另立验收项；实现完成判据止于六文件改动落地 + 阶段五的工作树核对，门禁与生成物零 diff 的实际执行由桌面端 static-check 承接。

---

## 依赖 (Dependencies)

运行时依赖：

| 依赖 | 变化 | 用途 |
|---|---|---|
| `std::borrow::Cow` | 新增（**标准库**，非 crate） | `NullableBreakdown::non_null` 的借用 / 窄化克隆二态 |
| `serde_json` | 零新增（`archi` 已在案） | schema 片段读取与 pointer 拼装 |
| `toml_edit` | 零改动 | `INDEX.toml` 装配（本轮零形态变化） |

构建 / 测试依赖：零新增。`Cargo.lock` 零净增包（无新外部 crate）；`[workspace.dependencies]` / `members` 零改动；前端零触碰，故零 npm 依赖变化（`bindings:export` 应零 diff）。

---

## 待决问题 (Open Questions)

1. **`required` 整体非数组（如 `required: "name"`）的处置**：本设计维持既有静默（`required_elements` 对非数组返回空 ⇒ 零判据）。它与「闭合性」同源（都会让字段静默变可选），但 proposal 只钉了元素集 ⊂ 键集这一条，扩进来会改判据射程。建议在后续变更与 `properties` 键集规则一并裁决，MUST NOT 在实现期顺手加。
2. **多分支 union 的完整表达（R-3 延续）**：`["string","integer","null"]` / `anyOf: [string, integer, null]` 仍只表达首个非空分支——两面同源故「一致」，但契约语义被静默削窄。改动它会动判据面的签名比对结果（`ApiType` 只有单分支 `Optional`），须单列变更。
3. **`$defs` 条目顶层 `oneOf`-含-null 的两侧形态**（D15 ②）：TS 经 `union_of` 得 `string \| null`，Rust 经 `variants_block` 得 `enum { String(String), Variant1 }`——两侧形态不同且是既有行为。本变更的等价式只覆盖「字段 / 参数 / 数组元素位」的 value 位置；是否让条目顶层的 `oneOf` 也走 nullable 前置分支（从而两侧收敛），待后续裁决。
4. **`x-function` 字段的可选轴**（事实校准 5 / D6）：两侧现状都不落可选符号。若后续要求「非 `required` 的函数指针字段也表达可选」（Rust `Option<fn(…)>` / TS `{field}?: (…) => R`），必须两侧同批改并同时处理 `x-class` 方法面（trait 不带 `required`，方法 MUST NOT 变可选）。
