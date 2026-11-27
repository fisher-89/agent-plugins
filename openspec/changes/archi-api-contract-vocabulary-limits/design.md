# 设计: archi-api-contract-vocabulary-limits

> **变更**: `archi-api-contract-vocabulary-limits`（契约词汇的「类型出生能力」：属性下沉 + 封闭枚举唯一形态 + 入口接线）
> **输入**: 本目录 `proposal.md`（定稿）、`specs/desktop-archi-api/spec.md`（ADDED ×1 + MODIFIED ×2）、`explore.md` §18–§32
> **性质**: 生成侧能力一次原子交付——契约只写「形状 + 语义」，`derive` / 本地命名 / 整数宽度 / serde 形态 / 文档注释下沉为 `schema → 生成物` 的**纯函数**；封闭枚举唯一形态收敛为 `oneOf: [{const, description}]`（`enum` 数组退役）；`core/archi` 的见证链接上入口。
> **模型面**: **零 delta**——不调用 `archi_element` / `archi_relation`，不新增 / 删除元素、关系、op、类型；`.dev-team/architecture/graph/**` 与 12 份 `.schema.json` 逐字节不变。落笔前只读实测 `archi_drift`：`status = clean`、`passed = true`、55 元素 / 129 关系、`apiFindings` 37 条全 `docMissing`——本变更不改变这四项读数。`Backend.Core.Archi` 的 `fileHash = sha256:cdd35da006023eb2672241cd48a30e6675152ee07b13552b887ca03ad676f1de`（`02-backend-core.toml`，本相位零模型写入）。

本设计遵守三资产分工：**TOML 答拓扑、schema 答接触面、md 答理由**。本变更的拓扑与接口都是零 delta，只有「生成器从 schema 推得出什么」与「schema 的合法写法」在变，因此落笔顺序退化为：接触面规则（C1–C5）→ 关键字表读取点（C9）→ 入口接线（C6）→ 理由文档两处成因句核对（C7）→ 版本（C8）。`enum` 数组退役与 `oneOf+const` 的迁移已在提案与 delta spec 定稿；本设计只定实现口径。

---

## 架构组件 (Architecture Components)

### C1 · 跨元素解析表与渲染入参（`core/archi/src/api/emit/mod.rs`）

- **职责**：把「元素 FQN → 接口原文」的只读表从 `render_all` 透传到两个渲染器，作为 facet 递归解析跨元素 `$ref` 的**唯一读面**；`def_entry` 是「`$ref` 字符串 → 目标 `$defs` 条目」的唯一定位点。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs`。
- **依赖**：`serde_json::Value`、`BTreeMap`、既有的 `reexport_target`（`api/mod.rs`，内部单点）与 `ref_name`（`emit/mod.rs`，内部单点）。
- **技术**：
  - `render_target(target, contract, schema, meta, schemas)` 增第五参 `schemas: &BTreeMap<String, Value>`——直接借用 `ApiSnapshot.schemas`，**不新建 `$defs` 投影拷贝、不新增公共类型**（数据早已在快照里，这是「多穿一个入参」，不是新读盘）。
  - 新增私有 `def_entry(local: Option<&Map<String, Value>>, from: &str, reference: &str, schemas: &BTreeMap<String, Value>) -> Option<&Value>`：本文件引用（`#/$defs/<名>`）走 `local`（本文件 `$defs`）；外部引用先经 `reexport_target(from, reference)` 求目标 FQN，再在 `schemas` 里取目标 `$defs` 的 `ref_name(reference)` 条目。目标不在表 / `$defs` 缺席 / 条目缺席一律 `None`（facet 侧按「推不出来」保守处理，MUST NOT 报错）。
  - `local_defs` / `reexport_defs` / `notice_lines` / `warn_missing_x_class` / `reject_generics` **逐字不变**。
- **零读盘**：本层与两渲染器都不碰文件系统；解析表的来源只有 `ApiSnapshot`。

### C2 · Rust 渲染器的属性下沉（`core/archi/src/api/emit/rust.rs`）

- **职责**：把「形状 + 语义」翻译成 `///` 文档注释、`derive` 行、容器 serde 属性、本地字段名与整数宽度；封闭枚举由 `oneOf` 的 `const` 分支出 unit variant。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`。
- **依赖**：`serde_json::{Map, Value}`、既有 `rust_comment_text`（注释唯一转义点）、`variant_name`（变体名唯一归一）、`optional_of`（可选包装唯一单点）、`paths::RUST_KEYWORDS`（关键字唯一词表，经新增 `paths::is_rust_keyword` 读取；该读取点见 C9）。
- **技术（内部单点，全部私有）**：

  | 单点 | 形态 | 职责 |
  |---|---|---|
  | `Shape` | `enum { Trait, Unit, Struct, Newtype, ClosedEnum, PayloadUnion }` | 形态判定**唯一一次**（`type_block` 与 `facets` 共用；MUST NOT 第二套形态判断） |
  | `shape_of` | `fn(definition: &Value) -> Shape` | 判序与既有 `type_block` 一致：`$ref` 由 `type_block` 先行返回空块（不入 `Shape`）→ `x-class` → `oneOf`/`anyOf` → 标量 → 零字段 `object` → 有字段 struct |
  | `DefsView<'a>` | `{ fqn, local: Option<&Map<String, Value>>, schemas, common: &BTreeMap<String, CommonRule> }` | facet 递归的唯一读面（避免多参穿透；`common` 是 `Eq` 保守判定的映射规则读面） |
  | `facets` | `fn(shape, definition, view) -> Facets` | std 面 + wire 面 + `Default` 条件 + serde 形态的**唯一推导点** |
  | `Facets` | `{ derives: Vec<&'static str>, rename_all: Option<&'static str>, per_item_rename: bool, default_variant: Option<usize> }` | derive 项集（固定相对序）、容器 serde 形态、逐项 rename 开关、`#[default]` 下标 |
  | `eq_able` | `fn(schema, view, visiting) -> bool` | `Eq` 的环容忍 DFS（最小不动点：环上假设为 false） |
  | `field_ident` | `fn(wire: &str) -> String` | 契约键 → 本地标识符的**唯一派生点**（camelCase → snake_case；关键字加 `r#`）；字段面 MUST NOT 另写一套 |
  | `apply_case` | `fn(name: &str, case: Case, area: Area) -> String` | serde `rename_all` 四种规则的纯模拟（字段面 / 变体面语义不同，`Area` 承载归属，见数据模型） |
  | `integer_type` | `fn(object: &Map<String, Value>) -> &'static str` | 完整值域判定唯一单点（`value_type` 与 `scalar_type` 共用） |
  | `doc_line` | `fn(description: Option<&Value>) -> Option<String>` | `///` 行的唯一产出点：`rust_comment_text` + 行尾 `trim_end`；空串 / 纯空白 / 缺省 ⇒ `None` |
  | `derive_line` | `fn(items: &[&'static str]) -> String` | derive 行固定点（≤96 字符单行，否则三行；空集 ⇒ 零字节；见数据模型） |
  | `string_literal` | `fn(text: &str) -> String` | 契约字符串字面量（`serde(rename = …)`）的唯一转义点（`{:?}` 形态） |

  - **派生产出**：`type_block` 统一在块首出「类型级 `///`（若有）→ derive 行 → 容器 serde 属性」，再分派到 `struct_block` / `variants_block` / `trait_block` / newtype / unit；`struct_block` 逐字段出「字段 `///` → 逐项 `#[serde(rename)]`（仅无候选命中时）→ `pub <local>: <ty>`」；`variants_block` 逐分支出「变体 `///` → `#[serde(rename)]`（仅无候选命中时）→ `#[default]`（仅封闭枚举的 default 分支）→ 变体行」。
  - **删除**：`enum_block` 与 `type_block` 的 `enum` 分支（`enum` 数组已在装载期拒收，保留函数即成第二份形态）。
  - **`value_type` / `scalar_type`**：只把 `integer` 臂改为调用 `integer_type`；其余分派顺序（`$ref` → 映射规则 → nullable → `type`）逐字不变。
  - **生成物编译前置**：本地类型有 wire 面时，在头注释后输出 `use serde::{Deserialize, Serialize};`（derive 名 MUST 与手写逐字一致，不能写 `serde::Serialize`）；最终产物文本（类型段 / 构造器 / 见证，剔除 `//` 与 `///` 行）含 `BTreeMap<` 时输出 `use std::collections::BTreeMap;`（这是对自己产物的单次文本检查，不是第二套 schema 判据）。两者都是「新能力第一次真正可编译」的前置，且当前三份生成物零 wire / 零 map ⇒ 零字节变化。
  - **逐字不变**：`op_witness` / `param_list` / `method_signature` / `function_pointer` / `optional_of` / `is_zero_field_object` / `rust_comment_text` / `variant_name`。**MUST NOT** 顺手改 `owner` 的见证形态（提案 R-6）。

### C3 · TS 渲染器的最小变化（`core/archi/src/api/emit/ts.rs`）

- **职责**：封闭枚举 / 带负载 union 的 `const` 分支 → 标量字面量并拼 union；其余面零新增。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`。
- **依赖**：既有 `ts_string_literal`（字符串字面量唯一转义点）、`union_of` / `value_type`。
- **技术**：
  - `render` / `render_scope` 增 `_schemas` 入参（签名与 Rust 侧对齐；本渲染器结构上不用它）。
  - `type_block` 的 union 臂：分支含 `const` 时产出字面量（字符串经 `ts_string_literal`；数字 / 布尔取 `to_string()` 原样），其余分支仍经 `value_type`，最后 `union_of` 拼接。
  - **删除** `enum_block` 与 `type_block` 的 `enum` 分支。
  - 字段名不派生（契约键即 wire 名）；注释面与值声明面**零新增**（逐值 JSDoc / 值对象后置）。

### C4 · 装载期三判据（`core/archi/src/api/schema.rs::verify_defs`）

- **职责**：在既有九道闸的 ⑨ 内新增三条 `$defs` 形态判据，把封闭枚举的两条旧路（`enum` 数组 / 条目顶层裸 `const`）消灭在写闸与装载期；把目标语言无法 raw 转义的键名拒收。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`。
- **依赖**：无新依赖；只读 `Value`。
- **技术**：三条判据落在**每条目循环的末尾**（`x-class` 分支与 `collect_nested_refs` 之后），确保既有五条 `$defs` 规则与骨架规则先命中，且两键落点（`verify_class_key_positions`）与降级词表仍在其后；实现抽为私有 `verify_closed_vocabulary(name: &str, definition: &Map<String, Value>) -> Result<(), ApiError>`（四不可 raw 词常量 `RAW_KEYWORD_DENIED`），由 `verify_defs` 在每条目末尾调用；三条之间判序固定为 ① 条目顶层 `enum` → ② 条目顶层裸 `const` → ③ 条目第一层 `properties` 键 ∈ `{self, Self, super, crate}`。**闸数仍是九道**，不新增第十道。`DEGRADED_KINDS` / `degraded_marks` / `build_index` / `verify_class_members` / `verify_class_key_positions` 逐字不变。

### C5 · 索引形状分类与装配（`core/archi/src/api/mod.rs`）

- **职责**：`render_all` 把 `schemas` 装配进 `render_target`；`def_shape` 的 `enum` 类目改由「`oneOf` / `anyOf` **非空且全 `const` 分支**」触发。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`。
- **依赖**：无新依赖；`snapshot.schemas` 已是既有字段。
- **技术**：`render_all` 内 `emit::render_target(&target, contract, schema_value, &meta, &snapshot.schemas)`；`def_shape` 判序改为 `$ref` → `x-class` → union（全 `const` ⇒ `enum`，否则 ⇒ `variant`）→ 标量 → `object`，`enum` 数组支删除。`categories` 键集合不变；`categories_of` / `index_entries` / `bootstrap` / `load_contracts` / `api_ref` 逐字不变。

### C6 · 入口接线（`core/archi/src/lib.rs`）

- **职责**：把 `core/archi/src/lib.i.rs` 的签名见证行真正交给编译器；`core/foundation` **不接**。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/lib.rs`。
- **技术**：在 crate 根 doc 之后、`pub mod api;` 之前加与 `infra/archi` 同款的两行：

  ```rust
  #[path = "lib.i.rs"]
  mod api_gen;
  #[allow(unused_imports)] // 本元素 $defs 为空，glob 再导出当前为空集；④ 迁移后自然非空
  pub use api_gen::*;
  ```

  `#[allow(unused_imports)]` 是零生成类型时的必需项：空模块的 glob 再导出会触发 `unused_imports`（与 AC-12「零新告警」冲突），且 `cargo clippy --fix` 会把它当未使用导入**直接删掉**（接线静默失效）。生成物零改动、`api:check` 零 diff。`core/foundation` 的 `lib.rs` 与 `lib.i.rs` 零改动（接收器缺口见理由文档）。
- **依赖**：无。

### C7 · 契约资产两处成因句（理由文档）

- **职责**：把 `Backend.Core.Archi` / `Backend.Core.Foundation` 两份文档里「为什么现在接 / 为什么不接」讲准；文档是产品面，直接渲染给用户读。
- **文件**：`.dev-team/architecture/models/Backend/Core/Archi.md`、`.dev-team/architecture/models/Backend/Core/Foundation.md`。
- **依赖**：无（文档内容与 C6 的接线决定绑定：F 若被裁，`Archi.md` 的接线句与 `Foundation.md` 的接收器句同批回退，见 R-10）。
- **技术**：两处成因句已在工作树就位（本相位产物的一部分），实现落地时按「F 是否裁剪」核对：`Archi.md` 的契约面段写「已接线（入口 `#[path] mod api_gen` + 再导出；生成物零类型，只编译见证行）」，类型段把 `$defs` 为空的成因改为「属性层此前无归属（derive / 命名 / 宽度 / 文档注释）+ 迁移是独立一轮、逐类型原子」；`Foundation.md` 把不接的成因改为「`owner` 方法的接收器在契约里不可表达 ⇒ 见证形态对 `is_set` 结构性失效」，并**逐字保留**「不会为了『让生成物能编译』多开导出面」的立场。两份文档 MUST NOT 列导出名（名字清单的唯一持有者是 `.schema.json`）。

### C8 · 版本与改动集边界

- **职责**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 现值 +1 patch（预期 `0.7.23`）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。
- **依赖**：无。
- **技术**：三条命令串（`api:export` / `api:check` / `check`）逐字不变；零新依赖、零新配置键。

### C9 · 关键字表读取点（`core/archi/src/api/paths.rs`）

- **职责**：把 `emit` 侧「本地字段名是否命中 Rust 关键字」的判定接到仓库唯一那张 51 词表上（`RUST_KEYWORDS`，`paths.rs:174`），使 `C2` 的 `field_ident` 与 `is_module_segment` 共用同一事实源；MUST NOT 在 `emit` 自抄第二份关键字表（D-4）。编号排在 C1–C8 之后：它不参与渲染流水线的分派顺序，是 `C2` 的一处依赖读取点。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs`。
- **依赖**：无（复用既有 `RUST_KEYWORDS` 常量；零新依赖、零新增公共类型）。
- **技术**：新增 `pub(crate) fn is_rust_keyword(token: &str) -> bool`（实现即 `RUST_KEYWORDS.contains(&token)`），这是 `paths.rs` 的**唯一增量**——`is_module_segment` 的常量引用改经该入口（语义与返回值逐字不变），其余函数零改动（形态判据仍由 `is_module_segment` 独担：它判「字符集 ∧ 非关键字」，本函数只回答「是不是关键字」这一半）。两档关键字的分工：可 raw 的词（`type` / `in` / `abstract` …）由 `C2` 的 `field_ident` 出 `r#<name>`；四不可 raw 词（`self` / `Self` / `super` / `crate`）已在 `C4` 的 ⑨ 由写闸拒收，故本函数无需分档。基线 Module Contract 的 `paths.rs` 行逐字不变：既有函数签名与返回值一个不改，本函数是纯增量。

---

## 变更清单 (Change Inventory)

<!-- 新增文件：本变更新增 0 个文件——所有改动都落在既有实现 / 测试 / 文档文件内；`$defs` 仍由既有 `.schema.json` 承载。 -->

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs` | `render_target` 增第五参 `schemas: &BTreeMap<String, Value>` 并透传两渲染器；新增私有 `def_entry(local, from, reference, schemas)`（本文件 `$defs` / 跨元素 `$ref` 的唯一定位点）；模块头 doc 补「解析表零拷贝、零读盘」 | C1；`local_defs` / `reexport_defs` / `notice_lines` / `warn_missing_x_class` / `reject_generics` 逐字不变 |
| `.../api/emit/rust.rs` | `render` / `render_scope` 增 `schemas` 并建 `DefsView`；新增 `Shape` / `Facets` / `shape_of` / `facets` / `eq_able` / `field_ident` / `apply_case` / `integer_type` / `doc_line` / `derive_line` / `string_literal`；`type_block` / `struct_block` / `variants_block` 出 `///`、derive、容器 serde、`#[default]`、本地名、宽度；`enum_block` 删除；`value_type` / `scalar_type` 的 integer 臂共用 `integer_type`；头注释后按需输出 `use serde::{Deserialize, Serialize};` / `use std::collections::BTreeMap;`；`render` 的 doc「四参签名不扩」改为「五参：多一份只读解析表，形状面不扩」 | C2；`op_witness` / `param_list` / `method_signature` / `function_pointer` / `optional_of` / `is_zero_field_object` / `rust_comment_text` / `variant_name` 逐字不变 |
| `.../api/emit/ts.rs` | `render` / `render_scope` 增 `_schemas`；union 臂的 `const` 分支 → 标量字面量（`ts_string_literal` / 数字 / 布尔原样）；`enum_block` 删除；`render` 的 doc「四参签名不扩」同批改写 | C3；零注释 / 零值声明 / 字段名不派生 |
| `.../api/paths.rs` | 新增 `pub(crate) fn is_rust_keyword(token: &str) -> bool`（`RUST_KEYWORDS.contains(&token)`，复用 `paths.rs:174` 的 51 词唯一词表）；`is_module_segment` 的常量引用改经该入口（语义 / 返回值不变），其余函数逐字不变 | C9；`emit` 侧字段名判关键字的唯一词表入口（MUST NOT 第二份关键字表，D-4） |
| `.../api/schema.rs` | `verify_defs` 每条目末尾增三判（条目顶层 `enum` / 裸 `const` / 第一层 `properties` 键 ∈ 四不可 raw 词），实现抽为私有 `verify_closed_vocabulary`（`RAW_KEYWORD_DENIED` 常量）；`verify_defs` 与 `validate_schema` 的九闸 doc 的 ⑨ 同批补「封闭词表形态三判」（保留测试断言的 `骨架四判据` / `两键形态` / `落点` 三个 needle） | C4；闸数不变、判序固定；`DEGRADED_KINDS` / `degraded_marks` / `build_index` / `verify_class_*` 逐字不变 |
| `.../api/mod.rs` | `render_all` 把 `&snapshot.schemas` 传给 `emit::render_target`；`def_shape` 的 `enum` 触发点改为「非空 union 且全 `const` 分支」，删除 `enum` 数组支，doc 补 `enum` / `variant` 分界 | C5；`categories_of` / `index_entries` / `bootstrap` / `load_contracts` / `api_ref` 逐字不变 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | crate 根顶部加 `#[path = "lib.i.rs"] mod api_gen;` + `#[allow(unused_imports)] pub use api_gen::*;`（属性带成因注释） | C6；`core/foundation` 不接 |
| `packages/desktop/package.json` | `version` 相对落地时 `HEAD` 现值 +1 patch（预期 `0.7.23`） | C8；三条脚本命令串逐字不变 |
| `.dev-team/architecture/models/Backend/Core/Archi.md` | 契约面段：编译期绑定已生效（入口接线、生成物零类型只编译见证行）；类型段：`$defs` 为空的成因改为「属性层此前无归属 + 迁移独立一轮、逐类型原子」 | C7；工作树已就位；MUST NOT 列导出名 |
| `.dev-team/architecture/models/Backend/Core/Foundation.md` | 契约面段末句：不接的成因是 `owner` 方法的接收器不可表达 ⇒ 见证形态对 `is_set` 结构性失效；「不为让生成物能编译而多开导出面」的立场逐字保留 | C7；工作树已就位 |

> **与提案「变更范围」两表的一致性（回溯补正后）**：提案（R-14 ①）已补齐本变更所需的三处登记——实现文件表含 `.../api/paths.rs`（C9 的 `pub(crate) fn is_rust_keyword`）、测试文件表含 `.../api/paths_test.rs`、`.../api/extract/**` 的「不要修改」由目录 glob 收窄到**产品面**并显式登记 `.../api/extract/mod_test.rs` 两处 `emit::{ts,rust}::render(...)` 的签名补参。本设计据此撤掉旧版「提案表欠精确」的提示：本表与提案实现文件表逐行对齐（8 实现 + 2 文档），测试影响登记见下「测试文件」表（7 个文件）。

### 零改动面（核对结论）

| 对象 | 理由 |
|---|---|
| 12 份 `.dev-team/architecture/models/**/<叶段>.schema.json`、6 份 `graph/model/*.toml` | 拓扑与接口零 delta：不新增 / 删除元素、关系、op、类型；现有 schema 零 `oneOf` / `enum` / `const` / `$defs` 层 `description` / 宽度关键字（op 的 `x-function.description` 不参与渲染）⇒ 生成物零变化 |
| `.../api/extract/**` 的产品面（`schema_type` / `signature_of` / `normalize_signature`） | 判据侧短形词汇归独立变更 `archi-api-signature-parity`；本变更 MUST NOT 顺手改短形。测试面唯一的例外是 `api/extract/mod_test.rs` 的**两处** `emit::{ts,rust}::render(...)` 调用（`render_both` 夹具）——它们按签名传导补空解析表，提取逻辑与断言意图不变 |
| `.../api/check.rs`、`.../drift/report.rs` | 判据词汇十值、`category` 七值、档位、纳管口径逐字不变 |
| `packages/desktop/src-tauri/crates/core/foundation/src/lib.rs` 与 `lib.i.rs` | `is_set` 的接收器缺口未裁决 ⇒ MUST NOT 接线；也 MUST NOT 为让见证行编译删 op 或多开导出面 |
| `crates/*/Cargo.toml`、`src-tauri/Cargo.toml` | 零依赖变化（`core/archi` 已有 `serde` + `specta`） |
| `.../api/common.json` | 语义封闭为「跨语言映射规则」；改它即全量重生成 + 零 diff 门噪声 |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` | 33 类型的迁移是独立一轮（逐类型原子；一个类型要么在手写、要么在生成，否则 `E0255`） |
| `.../crates/infra/agent/src/archi-knowledge.md` | `x-` 面仍五项、零新规则、体量锚不动 |
| `.dev-team/architecture/graph/INDEX.toml`、`.i.*`、`packages/desktop/src/types/generated/**`（bindings 落点） | 生成物与 bindings 零手改、零字节变化（`api:export` / `bindings:export` 幂等） |
| `openspec/specs/**` 基线 | delta-only；本变更只带 `specs/desktop-archi-api/spec.md` 一份 delta，归档时再由流程并入 |

### 测试文件（影响登记；新增用例由测试相位承接）

| 文件 | 机械传导（实现相位必须做：签名补参 / 旧形态断言改判 / 语料迁移；不新增 golden） |
|---|---|
| `.../api/emit/rust_test.rs` | `render_types` / `render_source` 辅助、**全部直接 `render(...)` 调用**与 `render_target(...)` 调用补解析表入参；2 处四参签名自证串改五参（第五参使签名超 100 字符 ⇒ rustfmt 折成多行；自证串按折行后的**多行形态**更新，连同用例名 / 注释的「四参」措辞）；6 处 `enum` 数组语料改写为 `oneOf + const`；旧的「`enum` 不落 serde 派生」断言按新 facet 表（封闭枚举出 wire 面）改判，MUST NOT 保留旧形态断言；其余断言意图不变 |
| `.../api/emit/ts_test.rs` | 同上辅助、直接 `render(...)` 调用（含跨渲染器调 Rust `render` 的一处）与 `render_target(...)` 调用补参；1 处四参签名自证串改五参（按 rustfmt 折行后的多行形态更新）；5 处 `enum` 数组语料改写为 `oneOf + const`；旧的 `as const` 数组 / 零值 enum 断言按新形态（字面量联合）改判，MUST NOT 保留旧形态断言 |
| `.../api/emit/mod_test.rs` | 所有 `render_target(...)` 调用末位补空解析表（零断言变化）；直接 `super::{rust,ts}::render(...)` 调用补参；2 处四参渲染器签名自证串改五参（按 rustfmt 折行后的多行形态更新）；`Mystery` / 超长 `description` 两处既有断言逐字保留 |
| `.../api/schema_test.rs` | 9 处 `enum` 字面量逐处判定：`$ref` + 结构关键字的负例保留 `enum` 作被测键（`$ref` 纯度先命中）；`x-class` 新键落点负例（`:2306` / `:2334`）与相关夹具改用 `oneOf + const`；`:1214` 的类目名清单与 `:1128` / `:1151` 的类目键名不变 |
| `.../api/mod_test.rs` | `render_all的def_shape六形状` 夹具的 `Choice: {"enum": …}` 改 `oneOf + const`（`enum` 计数仍 1，`variant` 计数不变）；`render_all按四参单形调render_target` 的源码自证按**五参**形态改判（实参序 `&target, contract, schema_value, &meta, &snapshot.schemas`；测试名 / 注释同批）；其余 `enum` 引用零改动 |
| `.../api/extract/mod_test.rs` | **测试面唯一例外**（产品面零改动）：`render_both` 夹具的两处 `emit::{ts,rust}::render(...)` 直接调用补空解析表（签名传导）；不补即编译失败，断言意图不变 |
| `.../api/paths_test.rs` | `render_ts(...)` 直接调用补空解析表（签名传导）；`rust_keywords单点持有` 增「`is_rust_keyword` 是唯一词表入口」的源码自证（`RUST_KEYWORDS` 的引用点从 `is_module_segment` 体内移到该入口）；`零第二形态与零清洗` 按新事实分判（`paths.rs` 仍零 `r#`；`emit/rust.rs` 的 `r#` 只许出在字段名派生单点：`RAW_PREFIX` 字面量 1 处、`r#{name}` 产出 1 处） |

> **「四参」措辞的同批改写**：`rust.rs` / `ts.rs` 的产品 doc、测试名（如 `render出口单形四参签名恰一`）、测试注释与 5 处签名自证串都写死了「四参」；签名增第五参后这些措辞与自证串 MUST 同批改为五参（多行形态），否则源码自证用例必红。这不是行为变化，是签名传导的一部分。

> **`test-design.md` 的旧句归测试相位**：`test-design.md` 现写「既有 `paths_test.rs` 的断言本变更零改动」，与本表 `paths_test.rs` 行的「两处改判」不符（提案 R-14 ③ 已登记）。本相位不改 `test-design.md`；测试设计相位须按本表与提案「测试文件」表同批改准该句，避免归档后设计与测试文稿分叉。

> 新增的 golden（三个代表类型）、宽度六形态、关键字 round-trip、文档注释、facet 五形态 / 环、封闭枚举字面量联合、三判据负例等**断言由测试设计相位编写**；本相位只登记机械传导与实现口径。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|---|---|---|---|---|
| `emit::render_target` | `.../api/emit/mod.rs` | 修改（增一个入参） | `pub fn render_target(target: &EmitTarget, contract: &Contract, schema: &Value, meta: &ApiMeta, schemas: &BTreeMap<String, Value>) -> Result<String, ApiError>` | 第五参是「FQN → 接口原文」的只读表（`ApiSnapshot.schemas`；spec 所称「FQN → `$defs` 解析表」即它的 `$defs` 投影，参数名取 `schemas` 因为值域是接口原文），供跨元素 `$ref` 的 facet 递归；唯一装配点是 `render_all` |
| `emit::rust::render` | `.../api/emit/rust.rs` | 修改（签名同上） | `pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget, schemas: &BTreeMap<String, Value>) -> String` | 解析表只供 `$ref` 的 facet 递归 / 可解析性判定；零读盘 |
| `emit::ts::render` | `.../api/emit/ts.rs` | 修改（签名同上，入参名 `_schemas`） | `pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget, _schemas: &BTreeMap<String, Value>) -> String` | TS 侧结构上不用解析表；签名与 Rust 侧对齐，避免分派器分支两套形态 |
| `api::schema::verify_defs` | `.../api/schema.rs` | 修改（行为；签名不变） | `pub fn verify_defs(schema: &Value) -> Result<(), ApiError>` | 每条目末尾新增三判（`enum` 数组 / 裸 `const` / 不可 raw 键名，实现为私有 `verify_closed_vocabulary`）；闸数仍九道 |
| `api::render_all` | `.../api/mod.rs` | 修改（内部装配；签名不变） | `pub fn render_all(root: &Path, snapshot: &ApiSnapshot) -> Result<Vec<GeneratedFile>, ApiError>` | 只多一行 `&snapshot.schemas` 透传；三类生成物的集合与落点零变化 |

> **内部单点（不计入公共 API）**：`emit::def_entry`、`emit::rust::{Shape, shape_of, Facets, DefsView, facets, eq_able, field_ident, apply_case, integer_type, doc_line, derive_line, string_literal}`、`api::mod::def_shape`、`paths::is_rust_keyword`（`pub(crate)`，C9；crate 内唯一消费方是 `emit::rust::field_ident`，故不计入公共 API）。
> `render_target` 的三个产品驱动面（IPC `archi_api_generate` / CLI `archi-api-gen` / agent 工具 `archi_generate`）都经 `render_all`，**签名零变化**——提案 R-5 的「五条驱动面同批改」在实现上收敛为「`render_all` 一处装配 + 测试夹具机械补参」（见 D-10）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|---|---|---|---|
| `Shape` | `.../api/emit/rust.rs` | 新增（私有） | 六形态（`Trait` / `Unit` / `Struct` / `Newtype` / `ClosedEnum` / `PayloadUnion`）的唯一判定结果，供 `type_block` 与 `facets` 共用 |
| `Facets` | `.../api/emit/rust.rs` | 新增（私有） | `{ derives, rename_all, per_item_rename, default_variant }`——属性推导的唯一承载形态 |
| `DefsView<'a>` | `.../api/emit/rust.rs` | 新增（私有） | `{ fqn, local, schemas, common }`——facet 递归的只读闭包输入（本元素 FQN / 本文件 `$defs` / 解析表 / 映射规则） |
| `Case` | `.../api/emit/rust.rs` | 新增（私有） | `camelCase` / `lowercase` / `PascalCase` / `snake_case` 四值——`apply_case` 的输入词汇 |
| `Area` | `.../api/emit/rust.rs` | 新增（私有） | `Field` / `Variant` 两面——同一 `Case` 在两面的 serde 规则不同，`apply_case` 必须按面分派 |

> 无新增公共类型；`EmitTarget` / `ModuleRef` / `Contract` / `GeneratedFile` / `ApiSnapshot` 零改动；零新 DTO、零 specta 出线。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | 落地时 `HEAD` 现值（预期 `0.7.22`） | patch +1（预期 `0.7.23`，`AGENT.md` 版本纪律）；**零配置键变化**——架构域 `config.json` 的 `ignore` / `exclude`、工作区 `write_protection` 面逐字不变 |

---

## 数据模型 (Data Model)

### 契约侧：`$defs` 的合法写法（`.schema.json`，手写真相）

本变更不改任何已有 schema，只把「未来可以怎么写」钉死。零新 `x-` 键；使用的全部是标准 JSON Schema 关键字。

| 语义 | 写法 | 生成侧落点 |
|---|---|---|
| 类型语义 | `$defs.<T>.description`（非空字符串） | 类型级 `///` |
| 字段语义 | `$defs.<T>.properties.<f>.description`（可与 `$ref` / `x-function` 并列） | 字段级 `///` |
| 变体语义 | `oneOf[i].description` / `anyOf[i].description` | 变体级 `///` |
| 封闭枚举 | `oneOf: [{ "const": <标量>, "description": "…" }, …]` | Rust unit variant + `///`；TS 字面量联合 |
| 默认变体 | `$defs.<T>.default` = 某分支的 `const` 值 | `Default` 派生 + 该分支 `#[default]` |
| 整数宽度 | `minimum` / `maximum`（`exclusive*` 先归一） | `u32` / `u64` / `i32` / `i64` |
| 本地命名 | `properties` 键 = wire 名（`in` / `abstract` 等关键字照写） | `snake_case` 派生 + `r#` |

**退役**：`$defs` 条目顶层 `enum` 数组（装载期 `ApiError`）；条目顶层裸 `const`（`const` 只在 union 分支内合法）。**不新增**：任何 `x-` 键、任何宽度 / 命名 / derive 声明位。`x-key` 一类「第二套命名」仍不在词汇内：顶层 `properties.<k>` 的形态闸只认 `x-function` / `$ref`，`$defs` 字段位的改名声明位也 MUST NOT 被 `emit` 读取——本变更不新增判据，只把边界写清。

### 生成侧：`.i.rs` 属性行的固定点（R-1 的落地口径）

`cargo fmt --all --check` 会把 `.i.rs` 当普通源码管——生成器 MUST 已是格式化器固定点。以下形态由实测锚定（默认 `max_width = 100`，仓库无 `rustfmt.toml`）：

| 产出点 | 规则 | 锚点 |
|---|---|---|
| derive 行 | 项集固定相对序 `Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type`（缺席项略去）；组装串 `#[derive(<项以 ", " 连接>)]` 的字符数 **≤ 96 出单行**，否则三行：`#[derive(` / `    <全部项>,` / `)]`；项集为空（trait）⇒ 零字节 | 9 项 = 88 字符保持单行；10 项 = 97 字符折三行（与 `dto.rs::GateTier` 逐字相同）；本词汇集可达长度只有 34 / 38 / 44 / 72 / 76 / 88 / 97 |
| 容器 serde | `#[serde(rename_all = "<候选>")]` 一行；只在 wire 面在场且某候选命中时出 | `lowercase` / `camelCase` |
| 字段行 | `    pub <local>: <ty>,`；`local = field_ident(wire)`；无候选命中时上方加一行 `    #[serde(rename = "<wire>")]` | `pub r#in: i64` |
| 变体行 | `    <Variant>,`；属性序固定为 `///` → `#[serde(rename = "<wire>")]`（无候选命中时）→ `#[default]`（default 分支）→ 变体行 | `#[default]` 在 `Off,` 上方；两者并存时 `rename` 在前 |
| 文档行 | `/// <rust_comment_text(text).trim_end()>`；空串 / 纯空白 / 缺省 ⇒ 零行 | 行尾空白必须剔除（rustfmt 会去掉，否则不是固定点） |
| 类型段 | `///`（若有）→ derive → 容器 serde → `pub struct/enum/trait` | 与 `dto.rs` 手写顺序一致 |
| 条件 import | 有 wire 面 ⇒ 头注释后加 `use serde::{Deserialize, Serialize};`；最终产物文本（剔除 `//` / `///` 行）含 `BTreeMap<` ⇒ 加 `use std::collections::BTreeMap;`；两者皆无 ⇒ 零字节 | 两行紧邻、按 rustfmt `reorder_imports` 的字典序（`serde` 在前、`std` 在后）；当前三份生成物零 wire / 零 map ⇒ 零 diff |

**属性序**：`serde_json::Map` 在本工作区经依赖统一启用了 `preserve_order`（`cargo tree` 实测），故 `$defs` 与 `properties` 的迭代序 = 文件书写序；生成器零排序。Golden 夹具按 `dto.rs` 的声明序书写即可与手写逐字对齐。

### facet 推导（std 面 + wire 面）

| 形态 | std 面 `derive` | wire 面（`Serialize` / `Deserialize` / `specta::Type`） |
|---|---|---|
| 零字段 `object`（名义类型） | `Debug, Clone, Copy, PartialEq, Eq`（与今天逐字相同，不扩 `Hash` / `Default`） | 不出 |
| `oneOf` / `anyOf` 全 `const`（封闭枚举） | `Debug, Clone, Copy, PartialEq, Eq, Hash` + 条件 `Default` | 出 |
| `object` 有字段（struct） | `Debug, Clone, PartialEq` + 条件 `Eq` | 出（若全部字段无 `x-function`；见 D-12） |
| 标量 newtype | `Debug, Clone, PartialEq` + 条件 `Eq`（`String` 不 `Copy`） | 出 |
| `oneOf` / `anyOf` 带负载 | `Debug, Clone, PartialEq` | 不出 |

- **`Eq` 条件**：环容忍 DFS。`string` / `integer` / `boolean` ⇒ true；`number` ⇒ false；`array` ⇒ 看 `items`；`object` 字典 ⇒ 看 `additionalProperties` 值；其余 `object`（含内联对象、`Value` 兜底）⇒ false；`$ref` ⇒ 跟随目标（本文件或跨元素；不可解析 ⇒ false）；封闭枚举 ⇒ true；带负载 union ⇒ false；可空 ⇒ 看非空分支；`x-function` 字段 ⇒ false（保守）；映射规则（`common.json` 的 `Outcome` 一类）⇒ false（保守，不假设映射后的 `Result` 实现）。环 ⇒ false（不死循环、不 panic）。
- **`Default` 条件**：`$defs.<T>.default` 在场且与某 `const` 分支值相等 ⇒ 出 `Default` 并在**该分支**加 `#[default]`；未声明 / 值不命中 ⇒ 不出、不报错。这是对 delta spec「条件 `Default`」的读法（见 D-2）：`ElementKind` 的 schema 不写 `default` ⇒ 无 `Default`，与 `dto.rs` 现状逐字相同；`GateTier` 写 `"default": "off"` ⇒ 与手写逐字相同。
- **wire 面判据**：契约是否已定义线形态。零字段（无线形态可承诺）、带负载 union（`oneOf` 不定义判别位）、含 `x-function` 字段的 struct（字段位没有线形态）⇒ 不出。**传递性不在本变更递归判定**：`$ref` 字段指向的另一条目是否本身有 wire 面，按 R-2 的已知窗口留给首个真实编译证据（④）；`Eq` / `Copy` / `Hash` 的缺席是「推不出来就不推」，不是错误。

### 命名策略（wire → local → serde）

| 事实 | 规则 |
|---|---|
| wire 名 | `properties` 键原样（`camelCase` / 关键字照写） |
| 本地字段名 | `field_ident(wire)`：按词边界（小写→大写、连续大写→小写）切词后 `snake_case`；命中 `paths::is_rust_keyword`（C9，唯一词表 `RUST_KEYWORDS`）⇒ `r#<name>`（四不可 raw 词已由写闸拒收） |
| 本地变体名 | 既有 `variant_name(const 文本)` 归一（恒为合法标识符） |
| serde 形态（字段） | 候选序 `camelCase` → `snake_case` → `PascalCase` → `lowercase`（serde 的字段规则里 `lowercase` 与 `snake_case` 都是 identity，故列末）；某候选把全部本地名映射回全部 wire 名 ⇒ 采用容器级 `rename_all`；全不命中 ⇒ 逐项 `#[serde(rename = "<wire>")]` |
| serde 形态（变体） | 候选序 `lowercase` → `camelCase` → `snake_case` → `PascalCase`；命中即容器级 `rename_all`；全不命中 ⇒ 逐变体 `rename` |
| TS 字段名 | 不派生：契约键即 wire 名，原样出现 |

候选序按形态分列不是任意选择：本仓手写枚举（`dto.rs` 十枚 + `drift/report.rs` 七枚）全用 `lowercase` 或 `camelCase`；手写 serde struct 绝大多数用 `camelCase`（含全单字字段的 `ArchiAssetSet` / `ArchiInterfaceAsset`；少数编辑域小结构无容器属性，属 ④ 迁移时逐类型核对的差异）。只有「变体先试 `lowercase`、字段先试 `camelCase`」才能让 `ElementKind` / `ArchiAssetSet` / `ArchiInterfaceAsset` 的 golden 与手写逐字对齐（见 D-3）。round-trip 语料：本地名经所选 serde 形态映射回的名字集合 == 契约键集合。

### 整数宽度（严格完整值域）

| 值域（闭区间；`exclusiveMinimum` / `exclusiveMaximum` 先 ±1 归一） | 本地类型 |
|---|---|
| `0 ..= 4294967295` | `u32` |
| `0 ..= 18446744073709551615` | `u64` |
| `-2147483648 ..= 2147483647` | `i32` |
| 缺 bounds / 单边 / 部分范围 / 其它 | `i64` |

- `maximum: 100` MUST NOT 推 `u8`（业务约束 ≠ 本地精度）。
- `type: "number"` 恒 `f64`；`["integer","null"]` + 完整 u32 值域 ⇒ `Option<u32>`（经 `optional_of` 单点包一次）。
- 判定只读 `serde_json::Number` 的 `as_u64` / `as_i64`（`u64::MAX` 的 20 位字面量必须精确命中，不经 f64）；`usize` 在契约里零写法。

### 生成物（`.i.ts` / `INDEX.toml`）

| 产物 | 本变更的形态 |
|---|---|
| `.i.ts` union | `oneOf` / `anyOf` 的 `const` 分支 → `'a' \| 1 \| true` 形态的标量字面量；其余分支照旧 `value_type`；成员经 `union_of` 连接。**零** `enum` 关键字、零值对象、零逐值 JSDoc |
| `INDEX.toml` `categories` | 键集合不变；`enum` 由「非空 union 且全 `const`」触发，其余 union 记 `variant`；在案 `INDEX.toml` 逐字节不变 |
| 生成物头注释 | 逐字不变（`common.json` 摘要与工具版本来源不变） |

### 持久化

零新增：无新文件格式、无新 schema 文件、无 DB 表 / 字段、无新生成物类别。唯一的版本字段变化是 `package.json` 的 `version`。12 份 `.schema.json` 与 6 份模型 TOML 逐字节不变。

---

## 路由 / API 设计 (Route / API Design)

### 生成物驱动面（零命令 / 零工具变化）

```
IPC archi_api_generate(root) ─┐
CLI archi-api-gen generate ───┼→ archi::workspace_render → render_all → emit::render_target(…, &snapshot.schemas)
agent 工具 archi_generate ────┘
```

| 驱动面 | 变化 | 说明 |
|---|---|---|
| IPC `archi_api_generate` | 零改动 | 签名与出参逐字不变；解析表在 `render_all` 内部装配 |
| CLI `archi-api-gen generate` / `--check` / `bootstrap` | 零改动 | 三条子命令与退出码语义不变 |
| agent 工具 `archi_generate` | 零改动 | 仍调 `workspace_render`；入参 / 响应形状不变 |

### 接口定义写闸（`write` / `edit` 的行为变化）

| 输入（`<archi_models_dir>/**/<叶段>.schema.json`） | 判定 | 出路（文案必须可执行） |
|---|---|---|
| `$defs.<T>` 条目顶层带 `enum` 数组 | `ApiError`，点名 `$defs.<T>.enum` | 改写成 `"oneOf": [{"const": "<值>", "description": "<语义>"}, …]` |
| `$defs.<T>` 条目顶层带裸 `const` | `ApiError`，点名 `$defs.<T>.const` | `const` 只放 `oneOf` / `anyOf` 分支内；单值类型用标量 `type` |
| `$defs.<T>.properties.<f>` 的键 ∈ `{self, Self, super, crate}` | `ApiError`，点名 `$defs.<T>.properties.<f>` | 改名（如 `own`）；四个词无法 raw 转义 |
| `oneOf` / `anyOf` 分支内的标量 `const` | 放行 | Rust unit variant / TS 字面量联合 |

- 写闸与装载**同一实现**（`validate_schema`），失败即 `Err` 且零落盘；`edit` 校验替换后的正文。
- 非 schema 路径零校验；`api = none` 的留白形态不变。

---

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 变化 | 用途 |
|---|---|---|
| `serde`（含 `derive`） | **零新增**（`core/archi` 已在用） | 生成物的 `Serialize` / `Deserialize` 派生；消费方前置规则：声明有数据位类型的元素 crate MUST 具备 `serde` + `specta` |
| `specta`（含 `derive`） | **零新增** | 生成物的 `specta::Type` 派生（derive 路径已限定，无需 import） |
| `serde_json` | **零新增** | `$defs` 读取、`Value` 兜底、宽度判定；工作区已启用 `preserve_order`（属性序 = 书写序的既有前提） |
| `std::collections::BTreeMap` | 标准库 | 字典字段文本；生成物按需 `use`，零新依赖 |

结论：**零新增依赖边、零 crate 清单改动**；`infra/archi` / `core/foundation` 的 `Cargo.toml` 零改动。

### 构建 / 测试依赖

零新增。测试复用既有 `TempDir` 夹具与 `serde_json`；不引入新 dev-dependency。

---

## 决策留痕

| # | 决策 | 备选与否决理由 |
|---|---|---|
| D-1 | 解析表**直接借用** `&ApiSnapshot.schemas`，不新建 `$defs` 投影或解析器类型 | 新建 `DefsIndex` 包装只多一层间接，且 `$defs` 仍在 `Value` 里；「多穿一个入参、零新读盘」是提案原意。`def_entry` 单点承担定位，两种引用形态一处判 |
| D-2 | `Default` 的**条件** = `$defs.<T>.default` 在场且命中某 `const` 分支 | delta spec 写「条件 `Default`」，D1 又把 `default` 列进标准关键字；AC-2 要求 `ElementKind`（手写无 `Default`）逐字相同、AC-3 要求 `GateTier`（手写有 `#[default] Off`）逐字相同——只有「`default` 关键字」能同时满足两者。补 `#[default]` 是「有证据的承诺」，符合「没有证据的公共承诺更贵」的立场。**回溯补正后无残留**：delta spec 已按此写准（facet 表「`default` 缺省或值不命中任何分支 ⇒ 不出 `Default`、不报错——缺省**不等于**首个变体」+ Scenario「`default` 关键字决定缺省变体」三基用例），旧的「枚举首变体带 `#[default]`」措辞已消失 |
| D-3 | serde 候选序**按形态分列**：字段 `camelCase` 首、变体 `lowercase` 首 | 单一候选序无法同时复现手写的绝大多数 `camelCase` struct 与 `lowercase`/`camelCase` enum：`ElementKind` 在 `camelCase` 下也会命中（单字小写不变），先试 `lowercase` 才能与手写逐字对齐；`SearchScope` / `GateTier` / `ApiStatus` 同族。少数编辑域小结构手写无容器属性（全单字 wire 名），生成侧会出 `camelCase` 属性——属 ④ 迁移的逐类型差异，不在本变更的 golden 面 |
| D-4 | 关键字表复用 `paths::RUST_KEYWORDS`（新增 `pub(crate) fn is_rust_keyword`） | 仓库已有一条「51 个关键字」的唯一词表与「MUST NOT 引入第二份关键字表」的纪律；`emit` 侧自抄一份会让 `mod` 段与字段位的判定分叉 |
| D-5 | 生成物按需输出 `use serde::{Deserialize, Serialize};` / `use std::collections::BTreeMap;` | AC-2 的 derive 行必须与手写一样用**非限定**名（`serde::Serialize` 会破坏逐字相同），而生成模块不继承手写文件的 `use`；不补 import，第一个有数据位的类型必然编译失败。当前三份生成物零 wire / 零 map ⇒ 零字节变化，AC-1 不受影响 |
| D-6 | derive 行固定点：组装串 ≤96 字符单行，否则三行；doc 行 `trim_end` | rustfmt 默认 `max_width = 100`；实测 88 字符保持、97 字符折三行，本词汇集可达长度只有 34/38/44/72/76/88/97。`GateTier`（10 项）就是折行锚。doc 行尾空白会被 rustfmt 剔除，不 trim 即二选一（R-1） |
| D-7 | 契约字符串字面量的唯一转义点是 `string_literal`（`{:?}`） | `serde(rename = "<wire>")` 的值可能来自任意 `const` 文本（引号 / 反斜杠 / 换行）；`rust_comment_text` 只防断行、不是字面量转义。`{:?}` 产出的 Rust 字符串字面量恒合法，且不引入第二份转义表 |
| D-8 | `enum_block` 删除；`is_scalar` 的 `!contains_key("enum")` 守卫保留 | 函数保留即成第二份形态；守卫不再有生产路径（装载期已拒），但保留零成本且防止测试直连时把 `enum` 误判成标量 |
| D-9 | `core/archi` 的 glob 再导出加 `#[allow(unused_imports)]` | 本元素 `$defs` 为空，`pub use api_gen::*;` 导入空集会触发 `unused_imports`（与 AC-12 冲突），且 `cargo clippy --fix` 会把它当未使用导入删掉——接线静默失效。属性带成因注释；④ 迁移后自然非空，属性可删但无害（Q-7） |
| D-10 | `render_target` 新参只波及 `render_all` 与测试夹具；IPC / CLI / agent 工具零签名变化 | 三个产品驱动面都经 `render_all`（`workspace_render` 是唯一入口），解析表在 `render_all` 内部从 `snapshot.schemas` 取。提案 R-5 的「五条驱动面」在实现上收敛为「一处装配 + 夹具机械补参」，避免让驱动面各建一份表 |
| D-11 | golden 的「逐字相同」比对面登记**四类例外**：`in_degree` / `usize`（提案已列）、`abstract_note`（本设计补登）、类型级**多行** `///`（折行口径） | `abstract_note` 与 `in_degree` 同族（人工后缀，不可机械派生），生成侧出 `pub r#abstract`；类型级多行 `description` 经 D45 安全闸折成一行，而手写注释是人工折行——两者不可能逐字，单行 description 的三处仍逐字比对。上述例外都须在测试设计相位按此口径写 golden（Q-2） |
| D-12 | 含 `x-function` 字段的 struct：wire 面**不出**，`Eq` 同批不推 | 函数指针字段没有 JSON 线形态，出 wire 面必编译失败；与「零字段 / 带负载 union 不出」同一条「契约是否已定义线形态」原则。**delta spec（回溯补正后）已覆盖该形态**：wire 面表单列「`object` 有字段但某字段是 `x-function` ⇒ 不出」并规定 std 面同时不推 `Eq`，`$ref` 的传递性按 R-2 的首例 fail-closed 处置（Q-3 只剩这一半） |
| D-13 | 两处**已登记但本变更不修**的判据形态 nit：① 四词闸按**契约键（wire 名）**判、派生后的本地名不做二判；② `emit::rust` 的 `is_unit_object` 与 `is_zero_field_object` 判据近重复（差异只有 `x-class` 排除项） | 两者都 fail-closed（宁可多拒 / 多判，不放行）：① 的闸与派生是等价映射（`r#` 与字段面 `camelCase` / `snake_case` 都不改单词），故 wire 名合法 ⇔ 本地名合法；② 的 `x-class` 排除项让零字段 object 与 trait 的构造面正确分叉。本变更 MUST NOT 为「顺手清理」扩闸数（仍九道）或改 `is_unit_object` 形态；收敛留后续变更，收敛时须同批复核 `emit/rust_test.rs` 的 `r#` 与构造器语料（提案 R-15） |

---

## 提案风险的落点

| 风险 | 落点 |
|---|---|
| R-1 fmt 逐字 | D-6：derive 折行规则、doc 行 `trim_end`、`#[default]` / `#[serde]` 位置先定死；AC-12 的固定点由测试相位对折行形态（`GateTier`）断言 |
| R-2 文本等价 ≠ 编译 | D-5 补 import 前置；wire 面只做「在场性」，首个真实编译证据归 ④；不可派生形态按 spec 处置（装载期 fail-closed），不放宽 |
| R-3 不可 raw 词 | C4 第三判：`$defs` 第一层 `properties` 键 ∈ `{self, Self, super, crate}` ⇒ `ApiError` 并给出路「改名」 |
| R-4 宽度与 JSON 数字精度 | 数据模型「整数宽度」：判定只走 `serde_json::Number` 的 `as_u64` / `as_i64`（不经 f64），`u64::MAX` 的 20 位字面量是唯一实例；若评审否决该字面量，回退序是「`u64` 不表达」并同步 ④ 的前置（提案原文） |
| R-5 签名波及 | D-1 / D-10：解析表只在 `render_all` 装配；三个产品驱动面零签名变化 |
| R-6 `owner` 见证 | C2 的「逐字不变」清单显式列出 `op_witness`；偏离 ② 的「不接线 + 登记」在 C6 / C7 落地 |
| R-7 `enum` 收紧 | C4 第一判 + 路由表第一行：出路文案给完整 `oneOf + const` 样例（MUST NOT 只写「非法」）；在案 12 份 schema 零 `enum` ⇒ AC-1 零 diff |
| R-8 负载 `oneOf` 已知窗口 | facet 表「带负载 union」行（零 wire 面、零 `Eq` / `Copy` / `Hash`）+ D-12；AC-8 的反向断言由测试相位落，spec 明写「线形态未定义」 |
| R-9 `def_shape` 触发点漏断言 | C5 + 变更清单测试文件表两行（`schema_test:1214` 类目名清单、`mod_test` 的 `categories` 夹具）；AC-11 用「键集合不变 + 触发点可红绿」双锚 |
| R-10 文档债边界 | C7 与 F 绑定：若 F 被裁，`Archi.md` 的接线句与 `Foundation.md` 的接收器句必须同批回退 |
| R-11 `enum` / `variant` 语义漂移 | C5 的 `def_shape` doc 写清分界（封闭枚举 vs 负载 union）；spec「索引形状分类」同句；面板零改动 |
| R-12 `r#` 叠加 `rename_all` | 数据模型「命名策略」：serde 先剥 `r#` 再套 `rename_all`；候选全不命中回退逐项 `rename`（生成器自动选）；AC-5 round-trip 覆盖该组合 |
| R-13 宽度边界之外 | 数据模型宽度表「业务约束 ≠ 本地精度」：`maximum: 100` 落 `i64`；④ 迁移需要窄类型时用 newtype，本变更不提供宽度声明位 |
| R-14 口径与登记（回溯补正） | 提案侧已补：实现文件表含 `api/paths.rs`、测试文件表含 `paths_test.rs` 与 `extract/mod_test.rs` 的两处补参、「不要修改」由 `extract/**` glob 收窄到产品面；delta spec 已按 D-2 改准 `Default` 语义。设计侧同批撤掉「提案欠精确」提示、测试文件表补 `paths_test.rs`、`extract` 行改「两处」、D-12 / Q-3 按 spec 现状改写（落地侧若再动签名 / 三判 / facet 规则，提案两表与 spec 三处措辞 MUST 同批改准） |
| R-15 两处判据 nit | D-13：四词闸只判 wire 名、`is_unit_object` 与 `is_zero_field_object` 的近重复均登记不修；本变更 MUST NOT 扩闸数或改形态 |

---

## 验收标准回应

| AC | 落点（设计面） |
|---|---|
| AC-1 存量零 diff | C1–C6 / C9 对当前契约零语义变化：三份生成物零 wire / 零 map / 零 `$defs` 层 `description`（op 的 `x-function.description` 不参与渲染）/ 零 bounds / 零 union；D-5 的条件 import 因而零字节；12 份 schema 与 6 份 TOML 零改动（变更清单） |
| AC-2 golden 等价 | 数据模型「生成侧固定点」+ D-3 候选序 + D-2 `Default` 条件 + D-11 四类例外（含本设计补登的 `abstract_note` 与类型级多行注释口径） |
| AC-3 封闭枚举 | C2 的 `variants_block`（unit variant + `///` + `#[default]`）；C3 的 TS 字面量联合；D-3 的 `lowercase` 首试 |
| AC-4 `enum` 数组退役 | C4 三判；C2 / C3 的 `enum_block` 删除；路由表的写闸三行 |
| AC-5 命名策略 | 数据模型「命名策略」：`field_ident` + 两套候选序 + 逐项 rename + round-trip；D-3 / D-4 / C9（关键字词表的唯一读取点） |
| AC-6 宽度 | 数据模型「整数宽度」：`integer_type` 单点、`exclusive*` 归一、`u64::MAX` 精确、`Option<u32>` 经 `optional_of` |
| AC-7 文档注释 | C2 的 `doc_line`：三处 `///`、空串零行、`rust_comment_text` + `trim_end`、op 的 `x-function.description` 仍丢弃（`op_witness` 逐字不变） |
| AC-8 wire 面射程 | 数据模型 facet 表：零字段 / 带负载 union / 有 `x-function` 字段的 struct 不出；三种有数据位形态出；D-5 的条件 import |
| AC-9 facet 正确性 | C2 的 `facets` / `eq_able`：项集与序固定、`number` 无 `Eq`、环容忍、纯 schema 函数；D-2 的 `Default` 条件。**第五参被真实读取**（非空解析表两支：目标可 `Eq` ⇒ 在场含 `Eq`、置空即红；目标带改判字段 ⇒ 解析成功而 `Eq` 不推）由测试相位在 `emit/mod_test.rs` 断言，设计面由 D-1 / D-10 保证 `render_all` 真传 `&snapshot.schemas`、两渲染器只在 facet 递归读它 |
| AC-10 接线 | C6：`#[path] mod api_gen;` + 再导出；`core/foundation` 零改动；D-9 的 `allow` |
| AC-11 索引类目 | C5：`def_shape` 触发点改判、键集合不变、`INDEX.toml` 零 diff |
| AC-12 生成物格式稳定 | D-6 固定点 + D-7 字面量转义 + D-5 import；`cargo fmt --check` / `clippy` 的机械执行归桌面端 |
| AC-13 改动集与版本 | C8 + C9 + 变更清单（实现文件 8 含 `package.json` 与 `api/paths.rs`，与提案实现文件表逐行对齐；文档 2；测试文件 7（含 `api/extract/mod_test.rs` 的两处签名传导与 `api/paths_test.rs`）的影响登记另表） |
| AC-14 spec delta 形态 | 本设计不改 `specs/**`；delta 已由提案定稿（ADDED ×1 + MODIFIED ×2），`Default` 措辞已按 D-2 改准（facet 表 + Scenario「`default` 关键字决定缺省变体」），Module Contract 已含本变更的「既有逐字不变断言改写」与 `paths.rs` 增量登记 |

---

## 待决问题 (Open Questions)

| # | 问题 | 现状 / 建议 |
|---|---|---|
| Q-1 | **已关闭**（回溯补正）：delta spec 的 `Default` 措辞 | 原分歧（「枚举首变体带 `#[default]`」vs AC-2 的 `ElementKind` golden）已消失——spec 现写「`default` 缺省或值不命中任何分支 ⇒ 不出 `Default`、不报错——缺省**不等于**首个变体」，并有 Scenario「`default` 关键字决定缺省变体」三基用例；D-2 与 spec 逐字一致 |
| Q-2 | AC-2 的「逐字相同」比对面需补例外：`abstract_note`（与 `in_degree` 同类）与类型级多行 `///`（D45 折行 vs 手写人工折行） | 建议按 D-11 写入测试设计：字段 / 变体级 `///` 与单行类型级 `///` 逐字比对；`abstract_note` / `in_degree` / `usize` 三行按偏离 ③ / ④ 排除；类型级多行注释只断言「折成一行且逐字等于折行后的契约文本」 |
| Q-3 | `$ref` 字段指向「本身无 wire 面」的目标时，wire 面是否传递性不出 | 前半（含 `x-function` 字段的 struct）已由 delta spec 的 wire 面表定死（D-12 关闭）；剩下 `$ref` 的传递性本变更**不递归判定**（wire 面只看本条目的形态），按 R-2 的首例 fail-closed 处置：若 ④ 的首个真实编译证据撞到不可派生形态，处置是装载期 fail-closed（扩 facet 判据），不是放宽 spec。当前 12 份 schema 零该形态，测试面可红绿 |
| Q-4 | 非字符串 `const`（数字 / 布尔）的 Rust 线形态 | 封闭枚举的 unit variant 经 serde 只能序列化成字符串，`const: 1` 的「数值线形态」无落点。本变更按 spec 出 wire 面（AC-8 只在三种字符串形态断言），并把该形态登记为已知窗口；建议后续把 `const` 收紧为字符串，或为数值枚举引入独立裁决 |
| Q-5 | `const` 归一为不可 raw 的变体名（`"self"` → `Self`）与变体名碰撞（`"a b"` / `"a-b"` → `AB`） | C4 的三判只覆盖 `properties` 键，spec 未要求 `const` 值的名字闸；本变更不扩闸（闸数不变）。建议首个真实需求时扩 ⑨（或在 `variant_name` 为不可 raw 词加固定前缀 / 对碰撞出 `ApiError`） |
| Q-6 | `default` 在场但不命中任何 `const` 分支 | 本设计取「不推 `Default`、不报错」（与 facet 的保守原则一致）；若要求 fail-closed，需在 ⑨ 增设第四判（会破坏「闸数不变」的既有断言），本变更不做 |
| Q-7 | AC-10 的「入口行与 `infra/archi` 同款」是否接受多一个 `#[allow(unused_imports)]` | D-9 的理由是空 glob 的 `unused_imports` 与 `clippy --fix` 删行；两行编排的语义与 `infra/archi` 相同。若评审拒绝该属性，替代是「只加 `#[path] mod api_gen;`、暂不 `pub use`」，但那会让 ④ 迁移时再补一行、且与 spec 的入口编排规定不一致 |
| Q-8 | 「`const` 只在 `oneOf` / `anyOf` 分支内合法且 MUST 是标量」的射程 | 本变更的 ⑨ 只判**条目顶层**裸 `const`（提案三判之一）；字段位 / 更深层的 `const` 与「非标量 `const`」目前无判据，Rust `value_type` / TS `value_type` 会落 `serde_json::Value` / `unknown` 兜底。建议首个真实需求时扩 ⑨ 的深扫（会改变「闸数不变」的现有断言，或作为 ⑨ 内的新子判），本变更不做 |
