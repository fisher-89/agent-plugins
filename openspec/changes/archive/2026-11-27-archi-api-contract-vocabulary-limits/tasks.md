# 任务: archi-api-contract-vocabulary-limits

> 顺序即依赖：前一组解锁后一组。落地口径见 `design.md`（决策 `D-1`…`D-13`、C1–C9、数据模型的固定点表）与 `specs/desktop-archi-api/spec.md`（ADDED ×1 + MODIFIED ×2）。
> 纪律：接口面（`.schema.json`）与拓扑（`graph/model/*.toml`）本变更**零 delta**，任何任务都 MUST NOT 顺手改它们；生成物（`.i.*` / `INDEX.toml`）与 bindings **MUST NOT 手改**；`cargo fmt` / `cargo clippy` / `pnpm api:export` / `pnpm api:check` 等机械步骤由桌面端执行，agent 内不重复执行。
> 测试纪律：新增 golden / 宽度 / 关键字 / 文档注释 / facet / 三判据**断言由测试设计相位编写**；本清单只做「既有语料按新形态机械迁移」与「既有断言按新形态机械改判」的传导。
> 改判纪律（本轮回溯补正）：既有断言的改判 MUST 与「使该断言为真 / 使旧断言失效」的源改动**同批**落在同一阶段；源改动在后续阶段的，前序条目只留指向并显式标注「随阶段 N 同批」，MUST NOT 提前改判——提前落笔会让该阶段以红收尾。判定口径：某阶段做完后，该阶段的测试面必须已全绿，任何依赖后续阶段产物的断言都算错位。

## 阶段 1 · 解析表与渲染入参（解锁全部渲染改动）

- [x] `crates/core/archi/src/api/emit/mod.rs`：`render_target` 增第五参 `schemas: &BTreeMap<String, Value>`，分派两渲染器时同参透传；模块头 doc 补一句「解析表直接借用 `ApiSnapshot.schemas`（零拷贝、零读盘），是跨元素 `$ref` 的 facet 递归唯一读面」。
- [x] `crates/core/archi/src/api/emit/mod.rs`：新增私有 `fn def_entry<'a>(local: Option<&'a Map<String, Value>>, from: &str, reference: &str, schemas: &'a BTreeMap<String, Value>) -> Option<&'a Value>`——本文件 `#/$defs/<名>` 走 `local`；外部串先经 `reexport_target(from, reference)` 求目标 FQN、再取目标 `$defs` 的 `ref_name(reference)`；目标缺席 / 形态不解析一律 `None`（不报错）。MUST NOT 新建 `$defs` 投影拷贝、MUST NOT 新增公共类型。
- [x] `crates/core/archi/src/api/mod.rs`：`render_all` 调 `emit::render_target(&target, contract, schema_value, &meta, &snapshot.schemas)`；函数签名与三类生成物集合零变化。
- [x] `crates/core/archi/src/api/emit/rust.rs`：`render` / `render_scope` 增 `schemas` 入参（`render` 的 doc「四参签名不扩」改为「五参：多一份只读解析表，形状面不扩」）；`render_scope` 建一个私有 `DefsView`（`contract.fqn` + 本文件 `$defs` + `schemas` + `meta.common` 映射规则）并往下传，后续 facet / `def_entry` 只读它。
- [x] `crates/core/archi/src/api/emit/ts.rs`：`render` / `render_scope` 增 `_schemas` 入参（TS 侧结构上不用；签名与 Rust 侧对齐，MUST NOT 在分派器写两套形态；`render` 的 doc「四参签名不扩」同批改写）。
- [x] 机械传导（零断言变化）：`.../api/emit/mod_test.rs` / `rust_test.rs` / `ts_test.rs` 内所有 `render_target(...)` 调用末位补空解析表（建议同文件私有 `fn no_schemas() -> BTreeMap<String, Value>`，或 `&BTreeMap::new()`），直接 `super::{rust,ts}::render(...)` / `emit::rust::render(...)` 调用同批补参（含 `ts_test` 跨渲染器调 Rust 一处）；`render_source` / `render_types` 辅助补齐同参；`.../api/extract/mod_test.rs` 的**两处** `emit::{ts,rust}::render(...)` 调用（`render_both` 夹具）同批补参——这是提案登记的测试面唯一例外，不补即编译失败；5 处四参签名自证串（`mod_test` ×2 / `rust_test` ×2 / `ts_test` ×1）改五参——第五参使 `render` 签名超 100 字符、rustfmt 必折成多行，自证串按折行后的多行形态更新（MUST NOT 期待单行串）；`rust_test::render出口单形四参签名恰一` 等用例名、测试注释与「四参」措辞同批改写。
- [x] `.../api/paths_test.rs`（提案 R-14 ① 补登的测试文件）：**本阶段只落一项**——`render_ts(...)` 直接调用补空解析表（签名传导，不补即编译失败）；`rust_keywords单点持有` 的合取第二项（`is_module_segment` 体内的常量引用改经 `is_rust_keyword`）**随 `is_rust_keyword` 落地改判**——源改动在阶段 5，本条只登记指向、MUST NOT 在本阶段改判（`is_rust_keyword` 尚不在盘，提前改判必红）；「`is_rust_keyword` 是 `RUST_KEYWORDS` 唯一读取点」与「`emit/rust.rs` 的 `r#` 仅出 `field_ident` 单点」两处自证**移入阶段 5**（与 `paths.rs` / `emit/rust.rs` 的源改动同批，见该阶段条目）。

## 阶段 2 · 封闭枚举形态切换（闸 / 类目 / 两渲染器 / 语料机械迁移）

- [x] `crates/core/archi/src/api/schema.rs`：`verify_defs` 在每条目循环末尾（`x-class` 分支与 `collect_nested_refs` 之后）新增三判，判序固定：① 条目顶层带 `enum` ⇒ `ApiError`，文案点名 `$defs.<T>.enum` 并给 `"oneOf": [{"const": "<值>", "description": "<语义>"}, …]` 的可执行出路；② 条目顶层带裸 `const` ⇒ `ApiError`，点名 `$defs.<T>.const`、写明「`const` 只允许在 `oneOf` / `anyOf` 分支内且 MUST 是标量」；③ 条目第一层 `properties` 键 ∈ `{self, Self, super, crate}` ⇒ `ApiError`，点名 `$defs.<T>.properties.<f>` 并给出路「改名」。闸数仍九道，MUST NOT 新增第十道；既有五条 `$defs` 规则与 `x-class` 骨架规则 MUST NOT 被遮蔽。
- [x] `crates/core/archi/src/api/schema.rs`：`verify_defs` doc 与 `validate_schema` 九闸 doc 的 ⑨ 同批补「封闭词表形态三判」；保留 `schema_test::schema闸九别名与doc随动` 断言的三个 needle（`骨架四判据` / `两键形态` / `落点`）。`DEGRADED_KINDS` / `degraded_marks` / `build_index` / `verify_class_members` / `verify_class_key_positions` 逐字不动。
- [x] `crates/core/archi/src/api/mod.rs`：`def_shape` 的 `enum` 触发点改为「`oneOf` / `anyOf` 非空且**全** `const` 分支」；其余 union 仍 `variant`；删除 `enum` 数组支；doc 补 `enum` / `variant` 的分界（负载 union 与封闭枚举都是 union）。
- [x] `crates/core/archi/src/api/emit/rust.rs`：`variants_block` 增 `const` 分支——unit variant（名字经 `variant_name(const 文本)`）；带负载分支形态逐字不变；`enum_block` 与 `type_block` 的 `enum` 分支删除。分支 `description` 的 `///` 留到阶段 3（A1 的 `doc_line` 落地后统一出）。
- [x] `crates/core/archi/src/api/emit/ts.rs`：`type_block` 的 union 臂把 `const` 分支映射成标量字面量（字符串经 `ts_string_literal`，数字 / 布尔 `to_string()` 原样），其余分支照旧 `value_type`、`union_of` 拼接；`enum_block` 与 `type_block` 的 `enum` 分支删除。零注释 / 零值声明 / 字段名不派生。
- [x] 语料机械迁移 · **本阶段为真的改判**（新增 golden / 新断言由测试相位写；旧 `enum` 专用断言必须按新形态改判、MUST NOT 保留旧形态断言）：`.../api/emit/rust_test.rs` 6 处、`.../api/emit/ts_test.rs` 5 处 `enum` 数组改写为 `oneOf + const`；其中 `ts_test.rs` 的 `as const` 数组 / 同名 `(typeof …)[number]` 断言**在本阶段同批改判**为字面量联合（TS 侧零 facet：形态切换本身就在本阶段落地、旧断言随即失效——后移到阶段 4 会让阶段 2 以红收尾）；`.../api/schema_test.rs` 9 处按提案分别改判（`$ref` + 结构关键字负例保留 `enum` 作被测键，`x-class` 新键落点负例 `:2306` / `:2334` 改 `oneOf + const`；`:1214` 类目名清单不变）；`.../api/mod_test.rs` 的 `render_all的def_shape六形状` 夹具 `Choice` 改 `oneOf + const`（`enum` / `variant` 计数不变），`render_all按四参单形调render_target` 的源码自证按五参形态改判（实参序 `&target, contract, schema_value, &meta, &snapshot.schemas`，测试名 / 注释同批）。
- [x] 语料机械迁移 · **不在本阶段改判的一项**（显式标注「随阶段 4 同批」）：`rust_test.rs` 的「`enum` 不落 serde 派生」一族（`render的enum为pub_enum` 尾段「零 `serde` 字样」+ 整测 `render的enum不落serde派生且不降级为错误`）**随阶段 4 同批**——封闭枚举的 wire 面由阶段 4 的 facet 落地，本阶段只把 `enum` 数组改写成 `oneOf + const`（`variants_block` 出形状、不出派生），旧「零 serde」断言在本阶段（及阶段 3）仍成立；提前改判必红。改判内容见阶段 4 条目。

## 阶段 3 · A1 · 文档注释（Rust 三处）

- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `doc_line(description: Option<&Value>) -> Option<String>`——`/// ` + `rust_comment_text(text)` + 行尾 `trim_end()`；空串 / 纯空白 / 缺省 ⇒ `None`（零行）。MUST NOT 绕过 `rust_comment_text`、MUST NOT 让契约文本产生裸代码行。
- [x] `crates/core/archi/src/api/emit/rust.rs`：`type_block` 统一出类型级 `///`（含 trait / 零字段 / 标量 newtype / 枚举）；`struct_block` 每个字段（含函数指针字段）上方按 `description` 出 `///`；`variants_block` 的 `const` 变体按分支 `description` 出 `///`。`op_witness` / `param_list` / `method_signature` / `function_pointer` MUST NOT 读 `x-function.description`（与今天逐字节相同）。
- [x] `.../api/emit/rust_test.rs`（文档注释落地的既有断言随动，**随本阶段 `doc_line` 同批**）：`render规模与特殊字符` 的超长 `description` 断言由「不进类型段」改判为「落单行 `/// <文本>`」（零断行、零裸代码行）；`rust_comment_text的消费面` 的「零裸 `// {` 插值」改判为「唯一允许 `/// {` 形态」（逐处钉前一个字符）；`enum_members` 辅助过滤 `///` 行（变体注释不混入成员集），`#[` 行一并排除（阶段 4 / 5 的变体属性行预留）。**理由**：`doc_line` 一落旧断言即失效（`/// {` 命中「零裸 `// {`」、变体注释混进成员集），本阶段不改判则该阶段以红收尾。

## 阶段 4 · B · facet 推导（std 面 + wire 面）

- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `Shape`（`Trait` / `Unit` / `Struct` / `Newtype` / `ClosedEnum` / `PayloadUnion`）与 `shape_of`——判序与既有 `type_block` 一致（`$ref` 由 `type_block` 先行返回空块、不入 `Shape` → `x-class` → `oneOf`/`anyOf` → 标量 → 零字段 `object` → 有字段 struct）；`type_block` 与 `facets` 共用这一次判定，MUST NOT 出现第二套形态判断。
- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `Facets { derives, rename_all, per_item_rename, default_variant }` 与 `facets(shape, definition, view)`：按设计的数据模型 facet 表产出 derive 集合（固定相对序 `Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type`）；wire 面只对「有数据位且线形态已定义」的形态出；`Default` 条件 = `$defs.<T>.default` 命中某 `const` 分支（D-2；Q-1 已随 spec 改准关闭），命中分支记入 `default_variant`。
- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `eq_able(schema, view, visiting) -> bool`——环容忍 DFS（环 / 不可解析 `$ref` ⇒ false）；`number`、带负载 union、非字典 `object`、`x-function` 字段 ⇒ false；`array` 看 `items`、字典看值、可空看非空分支；封闭枚举 / 标量 ⇒ true。MUST NOT panic、MUST NOT 死循环。
- [x] `crates/core/archi/src/api/emit/rust.rs`：`type_block` 在块首统一出「类型级 `///`（若有）→ `derive_line(&facets.derives)` → 容器 `#[serde(rename_all = …)]`（若有）」；`struct_block` / `variants_block` / newtype / unit 各自消费 `Facets`；零字段 `object` 的 derive 保持 `Debug, Clone, Copy, PartialEq, Eq` 逐字不变。
- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `derive_line(items)`——组装串 `#[derive(...)]` 字符数 ≤96 出单行，否则 `#[derive(` / `    <全部项>,` / `)]` 三行（与 `dto.rs::GateTier` 的 10 项折行逐字相同）；MUST NOT 让 `cargo fmt --check` 再改。
- [x] `crates/core/archi/src/api/emit/rust.rs`：生成物编译前置（D-5）——至少一个本地类型有 wire 面时，在头注释后输出 `use serde::{Deserialize, Serialize};`；渲染出的类型文本含 `BTreeMap<` 时输出 `use std::collections::BTreeMap;`；两者皆无 ⇒ 零字节（三份在案生成物因此零 diff）。
- [x] `.../api/emit/rust_test.rs`（wire 面改判，**随本阶段 facet 同批**）：`render的enum为pub_enum` 尾段的「零 `serde` 字样」翻成「三项 wire 派生在场」（`Serialize` / `Deserialize` / `specta::Type`）；`render的enum不落serde派生且不降级为错误` 整测改判——用例名 / doc 改「封闭枚举落 wire 面且不降级为错误」，行为断言由「零 serde」翻成「三项在场」，源码自证由「`rust.rs` 零 `serde::`」改为「`use serde::{Deserialize, Serialize};` 字面量恰 1 处 + 零限定 derive（`serde::Serialize,` / `serde::Deserialize,` 零命中）」；`enum_members` 的 `#[` 行过滤已在阶段 3 随 `///` 一并落，本阶段零重复。**理由**：本阶段之前旧「零 serde」断言仍成立（封闭枚举尚无 wire 面），提前改判必红。

## 阶段 5 · C · 命名策略（本地名 / `r#` / serde 形态自动选）

- [x] `crates/core/archi/src/api/paths.rs`：新增 `pub(crate) fn is_rust_keyword(token: &str) -> bool`，复用既有 `RUST_KEYWORDS`（MUST NOT 另抄一份关键字表）；`is_module_segment` 的常量引用改经该入口（语义与返回值零变化；设计 C9：意图仅此一处实现增量）。测试面自证见本阶段的 `paths_test.rs` 条目（**与本源改动同批**，MUST NOT 在前序阶段落笔）。
- [x] `.../api/paths_test.rs`（自阶段 1 条目移入，与本源改动同批）：`rust_keywords单点持有` 的合取第二项改判为「`is_module_segment` 体内调用 `is_rust_keyword(…)`」（常量引用多一层单点包装，合取仍成立）+ 新增「`is_rust_keyword` 体内引用 `RUST_KEYWORDS`（词表唯一读取点）」自证；`零第二形态与零清洗` 按新事实分判——`paths.rs` 仍零 `r#`（模块段不得出），`emit/rust.rs` 的 `r#` 只许出在 `field_ident` 单点（`RAW_PREFIX` 字面量 1 处、`r#{name}` 产出 1 处）。
- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `field_ident(wire: &str) -> String`——按词边界（小写→大写、连续大写→小写）切词后 `snake_case`；命中 `is_rust_keyword` ⇒ `r#<name>`；MUST 是字段本地名的唯一派生点（变体名归既有 `variant_name`，参数名走既有 `param_list` 且已受名字闸约束，MUST NOT 另写一套派生）。
- [x] `.../api/emit/rust_test.rs`（本地名派生的既有断言随动，**随 `field_ident` 同批**）：`render的演练草案金样` 的 `pub pendingToolCallId:` 改判为 `pub pending_tool_call_id:`（camelCase 键 → `snake_case` 本地名是本阶段的产出形态）；其余金样字段（单字 / 全小写键）形态不变。
- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `apply_case(name, case, area)` 与候选选择——字段序 `camelCase` → `snake_case` → `PascalCase` → `lowercase`；变体序 `lowercase` → `camelCase` → `snake_case` → `PascalCase`；某候选把全部本地名映射回全部 wire 名 ⇒ `rename_all`；全不命中 ⇒ `per_item_rename = true`。MUST 精确模拟 serde 的字段 / 变体规则（字段的 `lowercase` / `snake_case` 是 identity；变体 `camelCase` 只小写首字符、`snake_case` 在大写前插 `_`）；两面差异经私有 `Area::{Field, Variant}` 分派，MUST NOT「统一」成一种。
- [x] `crates/core/archi/src/api/emit/rust.rs`：`struct_block` 用 `field_ident` 出本地字段名；`per_item_rename` 时每字段上方加 `#[serde(rename = "<wire>")]`（字面量经 `string_literal`）；`variants_block` 同款处理变体。关键字键（`in` / `abstract`）产出 `pub r#in` / `pub r#abstract` 且**零** `rename`；round-trip 由所选 `rename_all` / 逐项 `rename` 保证。
- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `string_literal(text) -> String`（`format!("{text:?}")`）——一切来自契约的 Rust 字符串字面量（`serde(rename = …)`）的唯一转义点；MUST NOT 手拼引号 / 反斜杠。

## 阶段 6 · D · 整数宽度（严格完整值域）

- [x] `crates/core/archi/src/api/emit/rust.rs`：新增私有 `integer_type(object) -> &'static str`——`exclusiveMinimum` / `exclusiveMaximum` 先 ±1 归一成闭区间；只认 `0..=4294967295` → `u32`、`0..=18446744073709551615` → `u64`、`-2147483648..=2147483647` → `i32`；其余（缺 bounds / 单边 / 部分范围）→ `i64`；比较只走 `serde_json::Number` 的 `as_u64` / `as_i64`（不经 f64）。
- [x] `crates/core/archi/src/api/emit/rust.rs`：`value_type` 与 `scalar_type` 的 `integer` 臂改调 `integer_type`（共用同一判定）；`number` 仍 `f64`；`["integer","null"]` + 完整 u32 值域经 `optional_of` 出 `Option<u32>`；其余分派顺序（`$ref` → 映射规则 → nullable → `type`）逐字不动。
- [x] `.../api/emit/rust_test.rs`（词面随动，零断言改判，**随 `integer_type` 同批**）：`choice_payload零残留且单通路` 的 doc 里「`rust.rs` 全文 `"i64"` 恰 2」的成因改指「`value_type` 的非对象兜底 + `integer_type` 的值域兜底」（计数断言仍为 2、语义不变；不改即与源文件实现分叉）。

## 阶段 7 · 接线 / 文档 / 版本

- [x] `crates/core/archi/src/lib.rs`：crate 根 doc 之后、`pub mod api;` 之前加 `#[path = "lib.i.rs"] mod api_gen;` 与 `#[allow(unused_imports)] pub use api_gen::*;`（属性带成因注释：本元素 `$defs` 为空、glob 当前为空集；同时挡 `clippy --fix` 把该行当未使用导入删掉）。
- [x] `crates/core/foundation/src/lib.rs` 与 `core/foundation/src/lib.i.rs`：**零改动**（MUST NOT 接线、MUST NOT 为让见证行编译删 `is_set` 或补导出面）。
- [x] `.dev-team/architecture/models/Backend/Core/Archi.md`：核对工作树已就位的两处成因句——契约面段写「已接线（入口 `#[path] mod api_gen` + 再导出；生成物零类型只编译见证行）」，类型段把 `$defs` 为空的成因写成「属性层此前无归属（derive / 命名 / 宽度 / 文档注释）+ 迁移独立一轮、逐类型原子」；MUST NOT 列导出名。若 F 被裁，接线句同批回退（R-10）。
- [x] `.dev-team/architecture/models/Backend/Core/Foundation.md`：核对工作树已就位的一处成因句——不接的原因改为「`owner` 方法的接收器在契约里不可表达 ⇒ 见证形态对 `is_set` 结构性失效」；「本模块不会为了『让生成物能编译』多开导出面」的立场逐字保留；MUST NOT 列导出名。
- [x] `packages/desktop/package.json`：`version` 相对落地时 `HEAD` 现值 +1 patch（预期 `0.7.23`）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动；三条脚本命令串逐字不变。

## 阶段 8 · 只读自查（零写入）

- [x] 生成物固定点自查（实现侧只读）：新产出形态与设计「生成侧固定点」表逐条对齐——derive 行 88 字符单行 / 97 字符三行、doc 行尾无空白、`#[default]` 在变体名上方、容器 serde 在 derive 之后；机械门禁（`cargo fmt --check` / `cargo clippy` / `api:export` / `api:check`）由桌面端执行。
- [x] `archi_drift` 零 error：覆盖 / 归属 / 接口面判据与基线一致（`status = clean`、55 元素 / 129 关系、`apiFindings` 37 条全 `docMissing`）；`archi_module Backend.Core.Archi` 的 `fileHash` 与写前一致（本相位零模型写入）。
- [x] 改动集自查：`git status --porcelain` ⊆ 变更清单（实现文件 8 含 `package.json` 与 `api/paths.rs` + 文档 2 + 测试文件 7（`emit` 三份 + `api/mod_test.rs` + `api/schema_test.rs` + `api/extract/mod_test.rs`（两处补参）+ `api/paths_test.rs`）+ 本 change 目录）；12 份 `.schema.json`、6 份 `graph/model/*.toml`、`common.json`、`dto.rs`、`api/extract/**` 的产品面（`mod_test.rs` 仅按签名补参）、`api/check.rs`、`crates/*/Cargo.toml`、生成物与 bindings 零 diff。
- [x] 回溯口径自查（R-14）：proposal「变更范围」两表已含 `api/paths.rs`、`api/paths_test.rs`、`extract/mod_test.rs` 两处补参与「不要修改」的产品面收窄；delta spec 的 `Default` 措辞已按 D-2 改准；本清单与 `design.md` 的测试文件表按 7 个文件登记，`extract` 只补入参、不改提取逻辑与断言意图。
- [x] 阶段顺序自查（本轮回溯补正的口径）：逐阶段核对「该阶段做完即全绿」——阶段 1 的 `paths_test.rs` 只补参（`is_rust_keyword` 的合取第二项与两处自证在阶段 5）；阶段 2 只做 `as const` 改判、不做「`enum` 不落 serde 派生」（随阶段 4）；阶段 3 的超长 `description` / `// {` / `enum_members` 随 `doc_line` 同批；阶段 5 的 `pendingToolCallId` 与 `paths_test.rs` 自证随本源改动同批；阶段 6 的 `i64` 词面随 `integer_type` 同批。任何「前序阶段依赖后续阶段产物」的条目即错位，MUST NOT 以「已经做完」为由保留。
