# 任务: api-contract-format-patches

> 覆盖 proposal「变更范围 - 实现文件」**7 行全量**（`extract/mod.rs` / `emit/ts.rs` / `emit/rust.rs` / `contract.rs` / `schema.rs` / `mod.rs` / `package.json`），零新增文件。测试文件面（`extract/mod_test.rs` / `emit/{ts,rust}_test.rs` / `contract_test.rs` / `schema_test.rs` / `mod_test.rs`）的用例本体归测试相位——本相位**不写用例、不跑测试、不跑门禁**；编译与门禁（`server:check` → `bindings:export` → `api:check` → `client:check`）由桌面端 static-check 承接。
>
> 依赖序要点：**阶段一**（`nullable_breakdown` 单点）解锁阶段二（两渲染面前置分支都从它取数）；阶段三（装载期闸）与阶段四（词表与债表）各不依赖前三者，但**阶段四**的 `degraded_marks` 必须先于 `index_entries` 改调，否则计数无单点可调；**阶段五**（版本行 + 工作树核对）收官。
>
> 三条不可动摇的机械锚点（既有断言锚它们，改动即红）：① `contract.rs` 公开面恰 3 个 `pub fn`（`parse` / `verify` / `is_constructor`）——本阶段新增的名字一律私有；② `contract.rs` 的七族族别串与 `verify` 体内 ①…⑦ 编号注释逐字保留；③ `emit/ts.rs` 的 `get("x-function")` 恰 2 处、其 `value_type` 体内 `"x-function"` 零命中。
>
> 自查 cwd：`packages/desktop/src-tauri`（Rust 侧）；`packages/desktop`（`package.json`）。

## 阶段一：可空形状识别的单点（`extract/mod.rs`）

- [x] 修改 `crates/core/archi/src/api/extract/mod.rs`：`use` 区补 `use std::borrow::Cow;`（置于 `use std::path::Path;` 之前，保持既有 `std` → `serde_json` → `crate` 分组序）
- [x] 同文件在 `schema_type` 之前新增 `pub(crate) struct NullableBreakdown<'a>`——两字段按序 `pub non_null: Option<Cow<'a, Value>>` / `pub explicit_null: bool`，`#[derive(Debug, Clone)]`；类型 doc 写明「`non_null` = 可渲染的非空分支（`None` = 纯 `null` 档，无任何非空分支可取）；`explicit_null` = 显式 null 的存在，是渲染面 `| null` 的**唯一依据**（渲染器 MUST NOT 回到 `schema.get("type")` 二次判 null）；`anyOf`-`oneOf` 分支走 `Cow::Borrowed`（零克隆），`type` 数组形态走 `Cow::Owned`（窄化克隆，与既有 `nullable_inner` 同一次开销）」
- [x] 同文件新增 `pub(crate) fn nullable_breakdown(schema: &Value) -> Option<NullableBreakdown<'_>>`：把 `nullable_inner` 的三分支识别整体搬入——① `anyOf`／`oneOf` 是数组且含 `is_null_schema` 分支 → `non_null = branches.iter().find(|b| !is_null_schema(b)).map(Cow::Borrowed)`；② `type` 是字符串 `"null"` → `non_null = None`；③ `type` 是数组且含 `"null"` → 去 null 后取首个 kind：无 kind 则 `non_null = None`，有则克隆外层对象、把 `type` 换成该 kind 字符串、包成 `Cow::Owned(Value::Object(..))`；三分支都不命中 → `None`（交回常规分派）。函数 doc 写明「`Option::Some` ⟺ schema 含显式 null；本函数**不收 `required`**——可选轴不属它，MUST NOT 因可选性扩参」
- [x] 同文件把私有 `fn nullable_inner(schema: &Value, position: Position) -> Option<ApiType>` 改为薄壳：`let breakdown = nullable_breakdown(schema)?;` → `Some(match breakdown.non_null { Some(inner) => schema_type(inner.as_ref(), position), None => ApiType::Unknown("null".to_owned()) })`；签名、语义与「`schema_type` 对外结果逐值不变」三句 doc 逐字保留，另补一句「形状识别已收口到 `nullable_breakdown`（判据面与两侧渲染面共用）」
- [x] 同文件 `schema_type` 的 doc「判序固定」段补一句：`nullable` 一支的**形状识别**在 `nullable_breakdown`，本函数只消费其非空分支（渲染面同源取数，MUST NOT 保留「判据认得、渲染不认得」的不对称）
- [x] 自查 阶段一：`grep -n "fn nullable_breakdown" crates/core/archi/src/api/extract/mod.rs` 恰 1 处；`grep -n "struct NullableBreakdown" crates/core/archi/src/api/extract/mod.rs` 恰 1 处；`grep -n "fn nullable_inner" crates/core/archi/src/api/extract/mod.rs` 恰 1 处（薄壳，体内 `nullable_breakdown(schema)?` 恰 1 处）；`grep -c "anyOf" crates/core/archi/src/api/extract/mod.rs` 仍为 2（识别逻辑只搬位置、不复制第二份）；`grep -n "required" crates/core/archi/src/api/extract/mod.rs` **零命中**（可选轴不得进判据面）

## 阶段二：两侧渲染面（`emit/ts.rs` + `emit/rust.rs`）

- [x] 修改 `crates/core/archi/src/api/emit/ts.rs`：`use crate::api::extract::Position;` 改为 `use crate::api::extract::{nullable_breakdown, Position};`
- [x] 同文件新增私有 `fn nullable_of(ty: String) -> String`：`ty` 已以 `| null` 结尾（`ty.trim_end().ends_with("| null")`）即原样返回，否则 `format!("{ty} | null")`；doc 写明「TS 侧 `| null` 的**唯一追加点**（幂等）——字段面的可空经 `value_type`、参数面的可选经 `function_type`，两处都调它，故 `| null` 至多一次；`interface_block` MUST NOT 自行拼接」
- [x] 同文件新增私有 `fn array_of(items: String) -> String`：`items.contains(" | ")` 时 `format!("({items})[]")`，否则 `format!("{items}[]")`；doc 写明「数组元素位的唯一拼装点：元素类型含顶层 `|` 时必须加括号（`(T | null)[]`），MUST NOT 出 `T | null[]` 这一优先级错误形态（`Outcome` 一类映射规则的内容位同理）」
- [x] 同文件 `fn value_type(...)`：在 `$ref` 分支与 `common_type` 分支之后、`match schema.get("type")` 之前插入 nullable 前置分支——命中 `nullable_breakdown(schema)` 即 `match breakdown.non_null { None => "null".to_owned(), Some(inner) => { let inner = value_type(inner.as_ref(), position, common); if breakdown.explicit_null { nullable_of(inner) } else { inner } } }` 并 `return`；注释写明「与 `schema_type` 同序（`$ref` → 映射规则 → nullable → `type` 分派），纯 null 档落 `null`（MUST NOT 是 `unknown`）」
- [x] 同文件 `value_type` 的 `array` 分支：把 `format!("{items}[]")` 改为 `array_of(items)`（`items` 的计算逻辑逐字不动）
- [x] 同文件 `fn interface_block(...)`：删掉非 `required` 分支的 `format!("{base} | null")`，改为「`required` 含该字段 → `format!("  {field}: {base};\n")`；不含 → `format!("  {field}?: {base};\n")`」；`x-function` 值分支保持提前 `continue`（函数指针字段不参与可选轴）；函数 doc 的「非 `required` 字段 → `T | null`」改为「非 `required` 字段 → `{field}?:`（零 `| null` 拼接，符号各自只落一处）」
- [x] 同文件 `fn function_type(...)`：`optional: true` 的参数由 `ty.push_str(" | null")` 改为 `ty = nullable_of(ty)`（幂等 ⇒ `optional` 与 nullable 叠加时不再出 `| null | null`）
- [x] 同文件模块 doc 第 4 行的「非 `required` → `T | null`」改为「非 `required` → `{field}?: T` / 显式 null → `T | null`（至多一次，唯一追加点 `nullable_of`）」
- [x] 修改 `crates/core/archi/src/api/emit/rust.rs`：`use crate::api::extract::Position;` 改为 `use crate::api::extract::{nullable_breakdown, Position};`
- [x] 同文件新增私有 `fn optional_of(ty: String) -> String`：`ty.starts_with("Option<") && ty.ends_with('>')` 时原样返回，否则 `format!("Option<{ty}>")`；doc 写明「可选包装的**唯一单点**（幂等）——`struct_block` 与 `value_type` 的可空分支都调它，故 `Option<Option<` 零产出（退化产物 MUST NOT 出现）」
- [x] 同文件 `fn value_type(...)`：在 `$ref` 与 `common_type` 之后、`match schema.get("type")` 之前插入 nullable 前置分支——`match breakdown.non_null { None => FALLBACK_VALUE_TYPE.to_owned(), Some(inner) => { let inner = value_type(inner.as_ref(), position, common); if breakdown.explicit_null { optional_of(inner) } else { inner } } }` 并 `return`；注释写明「纯 null 档保持既有兜底 `serde_json::Value`（即 `FALLBACK_VALUE_TYPE`），MUST NOT 出 `()`、MUST NOT 折进可空轴的等价式」
- [x] 同文件 `fn struct_block(...)`：非 `required` 分支的 `format!("Option<{base}>")` 改为 `optional_of(base)`（`x-function` 分支保持 `continue`，函数指针字段不参与可选轴）
- [x] 同文件模块 doc 第 3–4 行的字段口径补一句「可选（`required` 缺省）与可空（`type` 含 `null`）是两个独立事实，叠加时经 `optional_of` 去重到至多一层 `Option<T>`」
- [x] 自查 阶段二：`grep -n '" | null"' crates/core/archi/src/api/emit/ts.rs` 只命中 `nullable_of` 体内（`interface_block` 与 `function_type` 零命中）；`grep -n 'Option<{' crates/core/archi/src/api/emit/rust.rs` **零命中**；`grep -n "Option<Option<" crates/core/archi/src/api/emit/rust.rs` 只出现在 doc/注释（代码零产出）；`grep -n 'nullable_breakdown' crates/core/archi/src/api/emit/{ts,rust}.rs` 各恰 2 处（import + 调用）；`grep -c 'get("x-function")' crates/core/archi/src/api/emit/ts.rs` 仍为 2（既有断言锚它）；`grep -n 'unknown' crates/core/archi/src/api/emit/ts.rs` 的代码命中点与改动前同（纯 null 档不落 `unknown`）

## 阶段三：属性键与 `required` 的自洽闸（`contract.rs`）

- [x] 修改 `crates/core/archi/src/api/contract.rs`：`fn identifier_error(...)` 增第 6 参 `pointer: Option<&str>`，体内按 `pointer` 分两支——`Some(pointer)` 时文案追加 `；JSON pointer \`{pointer}\`` 并把出路句换成「属性键必须是标识符；`data-testid` 一类键不进契约（前端契约需避开 DOM 测试属性）；人类可读的措辞请写在散文区」，`None` 时保留既有整句（含「MUST NOT 含空格 / 连字符 / 点 / 首字符数字」与「需要人类可读的措辞请写在散文区」）；doc 写明「`pointer` 只由 ⑤ 族传入——它是唯一「错误位置在 `$defs` 深处」的族，故 `pointer` 在案即选属性键的出路句；其余六族 MUST NOT 传 `Some`（通用句与出路句混说会让族别文案失真）」
- [x] 同文件九处既有 `identifier_error` 调用点（`op id` / `参数名` ×2 / `owner 键` / `$ref 末段名` ×2 / `$defs 键名` / `方法名` / `透传导出名`）补第 6 实参 `None`；族别串与实参序逐字不动
- [x] 同文件 `pub fn verify(...)`：`$defs` 遍历由 `defs.values().collect::<Vec<&Value>>()` 改为 `defs.iter()`（`for (name, definition) in schema.get("$defs").and_then(Value::as_object).into_iter().flatten()`），保持既有 `.unwrap_or_default()` 的「无 `$defs` 即零趟」语义（空迭代）
- [x] 同文件 ⑤ 族调用点改为 `identifier_error(contract, "properties 数据字段键", &field, FENCE_TYPES, 0, Some(&format!("$defs.{name}.properties.{field}")))`（族别串逐字不变；pointer 指到键本身）
- [x] 同文件新增私有 `fn required_elements(definition: &Value) -> Vec<&Value>`：`definition.get("required").and_then(Value::as_array)` 的元素（缺省 / 非数组 ⇒ 空 `Vec`）；doc 写明「保留 `Value` 形态——非字符串元素也要能点名形态（`kind_of`）；`required` 整体非数组维持既有静默，不在本期加判据」
- [x] 同文件新增私有 `fn required_dangling_error(contract: &Contract, definition: &str, index: usize, element: &Value) -> ApiError`：文案含 `contract.file`、`FENCE_TYPES`、该元素（字符串原样、非字符串经 `kind_of` 报形态）、pointer `$defs.{definition}.required[{index}]`，出路句为「`required` 与 `properties` 必须闭合：改成 `properties` 里已有的键，或补上该字段的定义（悬垂会让两侧产物把该字段静默渲染成可选）」
- [x] 同文件在 ⑤ 族的 `for field in property_names(definition)` 之后、`methods_of` 之前，同趟续判闭合性：`let properties = definition.get("properties").and_then(Value::as_object);` → `for (index, element) in required_elements(definition).into_iter().enumerate()` 逐条判 `element.as_str().is_some_and(|key| properties.is_some_and(|map| map.contains_key(key)))`，不成立即 `return Err(required_dangling_error(contract, name, index, element))`；注释写明「本闸判**集合关系**、不是名字形态，故不占族编号（①…⑦ 不动、无 ⑧）；同一趟、同一公开入口，MUST NOT 拆第二个 `pub fn`」
- [x] 同文件 `verify` 体内闸一注释段补一句「同趟续判 `required` ⊆ `properties`（自洽闸，零新增族别数）」；①…⑦ 的七个族编号注释逐字保留
- [x] 同文件模块 doc 头（「**`verify` 是装载期的校验收口**：三条双向 gate + 闸一 + 落点闸」段）补「+ `required` 与 `properties` 的自洽闸（集合关系，不占族别编号）」
- [x] 自查 阶段三：`grep -c "^pub fn " crates/core/archi/src/api/contract.rs` 仍为 3（公开面不扩张：新增两处全是私有）；`grep -n "fn required_dangling_error\|fn required_elements" crates/core/archi/src/api/contract.rs` 各恰 1 处；`grep -n "properties 数据字段键" crates/core/archi/src/api/contract.rs` 仍只 1 处且字符串逐字不变；`grep -n "⑧" crates/core/archi/src/api/contract.rs` **零命中**（不新增族编号）；`grep -n "required\[" crates/core/archi/src/api/contract.rs` 的**代码**命中恰 1 处（`required_dangling_error` 的 pointer 拼装；doc 里 `required[<i>]` 的引用另计）；`grep -c "identifier_error(" crates/core/archi/src/api/contract.rs` 与改动前一致（定义 1 + 调用 10，仅实参增多）

## 阶段四：降级词表与真实债表（`schema.rs` + `mod.rs`）

- [x] 修改 `crates/core/archi/src/api/schema.rs`：新增 `pub(crate) const DEGRADED_KINDS: [&str; 4] = ["foreignElement", "noElement", "unrepresentable", "crossLanguage"];`，doc 写明「**封闭降级词表**（首期四值，新增第五值 MUST 改 `desktop-archi-api` 的「降级词表与真实债表」）；语义：`foreignElement` 跨域 / 非直接子元素引用、`noElement` 目标无模型元素、`unrepresentable` 目标语言类型系统表达不了、`crossLanguage` 规则表降级；MUST NOT 进 `common.json`（该资产语义封闭为映射规则，且其摘要进全部生成物头注释）」
- [x] 同文件新增 `pub(crate) fn degraded_marks(schema: &Value) -> Vec<(String, &Value)>` + 私有递归 `fn collect_degraded<'a>(value: &'a Value, pointer: &str, marks: &mut Vec<(String, &'a Value)>)`：对象节点先判「自身 `x-common` 是对象且含 `degraded`」→ push `("{pointer}.x-common.degraded", mark)`（根节点 pointer 用 `$`），再逐键递归（键段 `.key`、数组段 `[i]`）；doc 写明「**全深度单点扫描**（`$defs` 条目自身、条目第一层 `properties` 的值、`properties` 里的 `x-function.params[].schema` 及更深）；锚点是 `x-common.degraded` 而非任意名为 `degraded` 的键（数据字段名合法可为 `degraded`，松锚点会把它计成债）；`verify_defs`（校验）与 `index_entries`（计数）各调一次，MUST NOT 存在第二套遍历」
- [x] 同文件 `pub fn verify_defs(...)` 末尾（`Ok(())` 之前、逐 `$defs` 循环之后）加一段：`for (pointer, mark) in degraded_marks(schema)` 逐个判 `mark.as_str()` 落在 `DEGRADED_KINDS` 内，否则 `return Err(ApiError::Contract(format!(...)))`——文案点名 pointer、报值的形态（`kind_of(mark)`，非字符串时连原文一并给）、并列四值（`DEGRADED_KINDS.join(" / ")`）并说明理由「非字符串与词表外值都让债表不可读（`true` 不携带任何原因信息）」
- [x] 同文件 `verify_defs` 的 doc 补第 6 条规则：`x-common.degraded` 的取值 MUST ∈ 封闭四值词表（非字符串亦拒收），错误点名 pointer 并列四值；`x-common.note` 不受校验、不进债表
- [x] 同文件 `IndexEntry.degraded` 的 doc 由「`$defs` 条目数」改为「`x-common.degraded` 标记的**出现次数**（全深度，含属性位与 `x-function` 参数位；与 `schema::degraded_marks` 同源，不再是 `$defs` 条目数）」；字段名 / 类型 / 顺序 / `build_index` 的写出形态零改动
- [x] 修改 `crates/core/archi/src/api/mod.rs`：`fn index_entries(...)` 的 `degraded` 计算由「`schema_value.get("$defs").values()` 上的 `x-common.degraded` filter 计数」改为 `schema_value.map(|value| schema::degraded_marks(value).len()).unwrap_or(0)`；`use schema::{...}` 行按需补 `degraded_marks`（或全路径调用，与既有风格一致）；零新增遍历、零新增常量
- [x] 自查 阶段四：`grep -n "DEGRADED_KINDS" crates/core/archi/src/api/schema.rs` 代码命中 2 处（定义 + 校验引用；doc 引用另计）；`grep -rn "degraded_marks" crates/core/archi/src/api/{schema,mod}.rs` 至少命中三处**代码**行（schema 定义 1 + `verify_defs` 校验调用 1 + `index_entries` 计数调用 1；doc 引用另计，不计入这三处）；`grep -n 'x-common' crates/core/archi/src/api/mod.rs` 的 `index_entries` 段**零命中**（计数已无本地 `x-common` 读取）；`grep -n "DEGRADED_KINDS\|degraded" crates/core/archi/src/api/common.json` **零命中**（词表不进映射规则资产）；`grep -c "^pub fn " crates/core/archi/src/api/schema.rs` 与改动前一致（`degraded_marks` 是 `pub(crate)`）

## 阶段五：版本行与工作树收尾

- [x] 修改 `packages/desktop/package.json`：`"version": "0.6.42"` → `"0.6.43"`（单行；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动）
- [x] 手工核对零回归面（只读，不跑生成）：`git status --porcelain` 只含本清单七份文件；`git diff --stat .dev-team/architecture/api/ packages/desktop/src/types/generated/` **零输出**；`git diff --stat packages/desktop/src-tauri/crates/core/archi/src/api/{check.rs,common.json}` **零输出**
- [x] 移交说明（不勾为任务）：`pnpm api:check` / `server:check` / `bindings:export` / `client:check` 四段门禁与生成物零 diff 的实际判定由桌面端 static-check 承接；五份测试文件的用例本体（含三处旧口径：`emit/ts_test.rs:144` 的 `T | null`、`api/mod_test.rs:1406` 的布尔 `degraded` fixture、`api/mod_test.rs:1387` 的「条目数」doc）与新增用例归 test-design / test-gen / test-execution
