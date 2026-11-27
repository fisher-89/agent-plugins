//! `.i.rs` 渲染：类型定义（`struct` / `trait` / `enum` / newtype）+ 自动构造器 +
//! 透传 `pub use` + 头注释 + 签名见证。
//!
//! **属性是 `schema → 生成物` 的纯函数**：契约只写「形状 + 语义」，`derive` 项集
//! （[`facets`]）、本地字段名（[`field_ident`]）、serde 形态（容器级 `rename_all` /
//! 逐项 `rename`）、整数宽度（[`integer_type`]）与文档注释（[`doc_line`]）全部由 `$defs`
//! 推出，MUST NOT 引入声明位。形态判定只做一次（[`shape_of`]），`type_block` 与
//! `facets` 共用同一结果——MUST NOT 出现第二套形态判断。
//!
//! **解析表是 facet 递归的只读读面**：跨元素 `$ref` 的 `Eq` 判定要能看见目标元素的
//! `$defs`（[`DefsView`] 携带「FQN → 接口原文」，来源是 [`super::render_target`] 的第五
//! 参）——定位单点是与 `emit/mod.rs` 共用的 `def_entry`；不可解析 ⇒ 保守不推，不报错、
//! 不 panic（`$ref` 是否存在是判据面的事实，渲染期不是第二份判据）。
//!
//! **trait body 的两类新成员**（`x-class` 条目）：构造入口（`fn new(<params>) -> Self;`，名字
//! 由协议固定、返回面由语义固定）与静态方法（`fn <名><method_signature>;`）——排在既有
//! `properties` 方法之后，读写面来自装载期的形态判据（本层零判错）。trait 不出 `T::new`
//! 见证：绑定由 `impl` 处的编译器强制，`impl <trait>` 不可编译。
//!
//! **签名见证必须写 `super::Type::method` 形态**：`#[path] mod api_gen` 内的裸名看不见
//! crate 根的 inherent 方法（实测报 `E0425`）；`super::LocalArchiCheck::file_stem` 有效
//! ——`super::` 在 `#[path]` mod 内指向 crate 根，而 crate 根的 `LocalArchiCheck` 正是
//! `pub use api_gen::*` 出的同一类型（正例编译通过、参数数不符报 `E0308`）。方法的
//! 归属类型由 `api-op` fence 的可选 `owner` 键给出；缺省即自由函数（`super::<op>`）。
//!
//! **位置分派是内部表**（入参位 `string` → `&str`、返回位 → `String`、字段位 →
//! `String`、入参位 `list<T>` → `&[T]`）：MUST NOT 变成对外门禁，偏离由契约
//! `x-common.note` 登记理由。
//!
//! **可选与可空是两个独立事实**：可选（字段不在 `required` 内）与可空（`type` 含
//! `null`）各只有一个判定点，叠加时经 [`optional_of`] 去重到至多一层 `Option<T>`。
//!
//! 边界：纯函数出 `String`，零写盘。

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{
    declared_ops, def_entry, local_defs, notice_lines, reexport_defs, EmitTarget, ModuleRef,
};
use crate::api::contract::{is_constructor, Contract, ContractOp, X_CONSTRUCTOR, X_STATIC};
use crate::api::extract::{nullable_breakdown, Position};
use crate::api::paths::is_rust_keyword;
use crate::api::{
    is_closed_enum, protocol_key, reexport_target, ref_name, union_choices, ApiMeta, CommonRule,
};

/// 注释拼接的唯一转义点（D45 闸二）：`\n` / `\r` / `U+2028` / `U+2029` → 空格（防断行），
/// 其余原样。`//` 与 `///` 两处凡是拼接来自契约的文本都经它——契约字符串是任意文本，
/// 未转义即能把后半段顶成代码行。
pub fn rust_comment_text(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '\n' | '\r' | '\u{2028}' | '\u{2029}' => ' ',
            other => other,
        })
        .collect()
}

/// `.i.rs` 全文（幂等：同输入两次渲染逐字相等；类型段按 `$defs` 声明序）。
///
/// **唯一出口**（D46）：入口形态与透传查表由 [`EmitTarget`] 给定——本模块只有这一个
/// 模块级导出，MUST NOT 另立「空 scope 的退化形态」孪生（退化关系由调用方传空
/// `modules` 的 `EmitTarget` 表达，不由函数数量表达）。
///
/// **五参**：多一份只读解析表（「目标元素 FQN → 接口原文」），形状面不扩——它只供
/// `$ref` 的 facet 递归与可解析性判定，零读盘。
///
/// **零 FQN 派生**（D40）：`pub use` 的模块路径取 [`ModuleRef::module_path`]（目标元素
/// 根的 `crate::<末段>` 形态）——目标 FQN 的点分段不是 Rust 路径。查不到目标元素即只出
/// 注释、不产 `pub use`（模型里没有该元素 ⇒ 无路径可派生）。
pub fn render(
    contract: &Contract,
    schema: &Value,
    meta: &ApiMeta,
    target: &EmitTarget,
    schemas: &BTreeMap<String, Value>,
) -> String {
    render_scope(contract, schema, meta, &target.modules, schemas)
}

/// facet 递归的只读输入（避免多参穿透）：本元素 FQN + 本文件 `$defs` + 解析表 + 映射规则。
///
/// `common` 也在读面内：`Eq` 的保守判定要知道「这一位是否走跨语言映射规则」
/// （映射后的 `Result<…>` 不假设实现 `Eq`），而映射规则的唯一真相在 `common.json`。
struct DefsView<'a> {
    /// 本元素 FQN（跨元素 `$ref` 的前缀解析要它）
    fqn: &'a str,
    /// 本文件 `$defs`（同文件 `#/$defs/<名>` 的解析面）
    local: Option<&'a Map<String, Value>>,
    /// 目标元素 FQN → 接口原文（跨元素 `$ref` 的解析面）
    schemas: &'a BTreeMap<String, Value>,
    /// 跨语言映射规则表（`Eq` 保守判定的读面）
    common: &'a BTreeMap<String, CommonRule>,
}

/// 渲染主体（本文件 [`render`] 的唯一实现）：透传查表与解析表由调用方给。
fn render_scope(
    contract: &Contract,
    schema: &Value,
    meta: &ApiMeta,
    modules: &BTreeMap<String, ModuleRef>,
    schemas: &BTreeMap<String, Value>,
) -> String {
    let view = DefsView {
        fqn: &contract.fqn,
        local: schema.get("$defs").and_then(Value::as_object),
        schemas,
        common: &meta.common,
    };
    let mut body = String::new();

    // 类型段（`$defs` 声明序；零排序、零按结构合并）
    let locals = local_defs(schema);
    let mut wire_face = false;
    for (name, definition) in &locals {
        let (block, wire) = type_block(name, definition, &view);
        if block.is_empty() {
            continue;
        }
        wire_face |= wire;
        body.push_str(&block);
        body.push('\n');
    }

    let constructible: Vec<String> = locals
        .iter()
        .filter(|(_, definition)| is_zero_field_object(definition))
        .map(|(name, _)| name.clone())
        .collect();

    // 自动构造器（构造器是类型的推论：契约 MUST NOT 声明，生成器 MUST 产出）
    for name in &constructible {
        body.push_str(&format!(
            "impl {name} {{\n    /// 构造器（自动产出；契约 MUST NOT 声明构造器）\n    \
             pub fn new() -> Self {{\n        Self\n    }}\n}}\n\n"
        ));
    }

    // 透传（纯 `$ref` alias → `pub use <目标模块>::<名>`）：模块路径只来自调用方查表
    for (name, reference) in &reexport_defs(schema) {
        let Some(target) = reexport_target(&contract.fqn, reference) else {
            continue;
        };
        let Some(module) = modules.get(&target) else {
            // 目标元素不在模型里 → 模块路径无从派生（MUST NOT 用 FQN 点分段冒充）
            body.push_str(&format!("// 透传自 {}\n\n", rust_comment_text(&target)));
            continue;
        };
        // 模块路径的每一段都 MUST 是合法 Rust 模块段（`pub use` 的模块段是**语法位置**）。
        // 该形态闸在装配期（`api::module_refs` 的 Rust 分支）已执行：非法目标根本不入查表，
        // 故本渲染器结构上收不到非法值 —— 查表未命中即按「目标不在模型里」同口径降级为
        // 注释。**MUST NOT** 在渲染期重复判定形态（二次派生，D40 / D41 / D45 同款）。
        body.push_str(&format!(
            "// 透传自 {}\npub use {}::{name};\n\n",
            rust_comment_text(&target),
            module.module_path
        ));
    }

    // 签名见证（每 op 一行 + 每个可构造类型的 `new` 一行；`super::` 前缀是硬约束）
    let witnesses: Vec<String> = declared_ops(contract)
        .into_iter()
        .filter(|op| !is_constructor(&op.id))
        .map(|op| op_witness(op, &meta.common))
        .chain(
            constructible
                .iter()
                .map(|name| format!("const _: fn() -> super::{name} = super::{name}::new;")),
        )
        .collect();
    if !witnesses.is_empty() {
        body.push_str("// 签名见证：契约与实现的编译期绑定（参数数或类型不符即 E0308）\n");
        for witness in witnesses {
            body.push_str(&witness);
            body.push('\n');
        }
    }

    // 头注释 + 条件 import（D-5）：生成物与入口同 crate 编译，而生成模块不继承手写文件的
    // `use`——derive 名 MUST 与手写逐字一致（`serde::Serialize` 会破坏「逐字相同」），故
    // 有 wire 面时补 `serde` 的非限定导入；`BTreeMap<` 出现在产物里时补 `std` 的导入。
    // 两者皆无 ⇒ 零字节（在案三份生成物因此零 diff）。
    let needs_map = needs_map_import(&body);
    let mut out = String::new();
    for line in notice_lines(meta) {
        out.push_str("// ");
        out.push_str(&rust_comment_text(&line));
        out.push('\n');
    }
    out.push('\n');
    if wire_face {
        out.push_str("use serde::{Deserialize, Serialize};\n");
    }
    if needs_map {
        out.push_str("use std::collections::BTreeMap;\n");
    }
    if wire_face || needs_map {
        out.push('\n');
    }
    out.push_str(&body);
    out
}

/// 「产物里出现了 `BTreeMap<`」判据（对**自己产物的单次文本检查**，不是 schema 判据）：
/// 注释行剔除（`// 透传自 …` 一类注释里的字样不产生类型需求）。
fn needs_map_import(body: &str) -> bool {
    body.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with("//") && line.contains("BTreeMap<")
    })
}

/// 单个 `$defs` 条目的形态（六种协议映射的**唯一判定**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// `x-class` 条目（trait，仅方法）
    Trait,
    /// 零字段 `object`（名义类型）
    Unit,
    /// 有字段 `object`
    Struct,
    /// 标量（newtype）
    Newtype,
    /// `oneOf` / `anyOf` 全是标量 `const` 分支（封闭词表）
    ClosedEnum,
    /// `oneOf` / `anyOf` 的其余形态（带负载 / 混合 / 空 union）
    PayloadUnion,
}

/// 形态判定（判序与既有 `type_block` 一致）：`x-class` → union → 标量 → 零字段 `object`
/// → 有字段 struct。纯 `$ref` alias 由 `type_block` 先行返回空块，不入本函数。
fn shape_of(definition: &Value) -> Shape {
    let Some(object) = definition.as_object() else {
        return Shape::Unit;
    };
    if object.get("x-class").and_then(Value::as_bool) == Some(true) {
        return Shape::Trait;
    }
    if let Some(choices) = union_choices(definition) {
        return if is_closed_enum(choices) {
            Shape::ClosedEnum
        } else {
            Shape::PayloadUnion
        };
    }
    if is_scalar(object) {
        return Shape::Newtype;
    }
    if is_unit_object(object) {
        return Shape::Unit;
    }
    Shape::Struct
}

/// 属性推导结果（std 面 + wire 面 + serde 形态的唯一承载形态）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Facets {
    /// `derive` 项集（固定相对序，缺席项略去）
    derives: Vec<&'static str>,
    /// 容器级 serde 形态（`rename_all` 的四种规则名；不命中即 `None`）
    rename_all: Option<&'static str>,
    /// 逐项 `#[serde(rename = …)]` 开关（容器级候选全不命中时）
    per_item_rename: bool,
    /// `#[default]` 所在分支下标（仅封闭枚举；缺省 / 不命中即 `None`）
    default_variant: Option<usize>,
}

impl Facets {
    /// wire 面在场判据（`Serialize` / `Deserialize` / `specta::Type` 三项同进同出）。
    ///
    /// 它同时是 serde 属性的闸：wire 面不在场时出 `#[serde(…)]` 就是未知属性（编译失败）。
    fn wire(&self) -> bool {
        self.derives.contains(&"Serialize")
    }
}

/// `derive` 项的固定相对序（「生成物 vs 手写逐字比对」的前提）。
const DERIVE_ORDER: [&str; 10] = [
    "Debug",
    "Clone",
    "Copy",
    "Default",
    "PartialEq",
    "Eq",
    "Hash",
    "Serialize",
    "Deserialize",
    "specta::Type",
];

/// 字段面 serde 候选序（D-3）：`camelCase` 首试——手写 serde struct 绝大多数用它，
/// 而 `snake_case` / `lowercase` 在字段面是 identity，列末只作兜底。
static FIELD_CASES: [(Case, &str); 4] = [
    (Case::Camel, "camelCase"),
    (Case::Snake, "snake_case"),
    (Case::Pascal, "PascalCase"),
    (Case::Lower, "lowercase"),
];

/// 变体面 serde 候选序（D-3）：`lowercase` 首试——单字小写词在 `camelCase` 下同样命中，
/// 只有先试 `lowercase` 才能与手写枚举逐字对齐。
static VARIANT_CASES: [(Case, &str); 4] = [
    (Case::Lower, "lowercase"),
    (Case::Camel, "camelCase"),
    (Case::Snake, "snake_case"),
    (Case::Pascal, "PascalCase"),
];

/// serde `rename_all` 的四值词汇。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    /// `camelCase`
    Camel,
    /// `lowercase`
    Lower,
    /// `PascalCase`
    Pascal,
    /// `snake_case`
    Snake,
}

/// 归属面：同一 [`Case`] 在字段面与变体面的语义不同（serde 的两套 `apply` 实现）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Area {
    /// 字段面
    Field,
    /// 变体面
    Variant,
}

/// serde `rename_all` 四种规则的**纯模拟**（唯一实现；见数据模型的命名策略表）。
///
/// 字段面：`lowercase` / `snake_case` 是 identity、`camelCase` = `PascalCase` 后小写首字符、
/// `PascalCase` 跳过 `_` 并大写下一个字符；变体面：`PascalCase` 是 identity、
/// `camelCase` 只小写首字符、`snake_case` 在每个大写前插 `_`。两面的差异是 serde 的
/// 既有语义，MUST NOT 「统一」成一种（那会让 round-trip 断在 `snake_case` 一侧）。
fn apply_case(name: &str, case: Case, area: Area) -> String {
    match area {
        Area::Field => match case {
            Case::Lower | Case::Snake => name.to_owned(),
            Case::Pascal => pascal_of(name),
            Case::Camel => lowercase_first(&pascal_of(name)),
        },
        Area::Variant => match case {
            Case::Pascal => name.to_owned(),
            Case::Lower => name.to_ascii_lowercase(),
            Case::Camel => lowercase_first(name),
            Case::Snake => snake_of_variant(name),
        },
    }
}

/// serde 字段面 `PascalCase`：跳过 `_`，其余字符按「小写首字符位置大写」逐个搬运。
fn pascal_of(name: &str) -> String {
    let mut pascal = String::new();
    let mut capitalize = true;
    for character in name.chars() {
        if character == '_' {
            capitalize = true;
        } else if capitalize {
            pascal.push(character.to_ascii_uppercase());
            capitalize = false;
        } else {
            pascal.push(character);
        }
    }
    pascal
}

/// serde 变体面 `snake_case`：每个大写字符前插 `_` 并整体小写。
fn snake_of_variant(name: &str) -> String {
    let mut snake = String::new();
    for (index, character) in name.chars().enumerate() {
        if index > 0 && character.is_ascii_uppercase() {
            snake.push('_');
        }
        snake.push(character.to_ascii_lowercase());
    }
    snake
}

/// 首字符小写、其余原样（标识符恒 ASCII，故零多字节切分风险）。
fn lowercase_first(name: &str) -> String {
    let mut characters = name.chars();
    match characters.next() {
        Some(first) => format!("{}{}", first.to_ascii_lowercase(), characters.as_str()),
        None => String::new(),
    }
}

/// 契约键 → 本地字段名的**唯一派生点**：按词边界（小写/数字 → 大写、连续大写 → 小写）
/// 切词后 `snake_case`；命中 [`is_rust_keyword`]（仓库唯一那张 51 词表）⇒ 原始标识符形态。
///
/// 词表只有一份（经 [`is_rust_keyword`] 读取）：`emit` 侧
/// 自抄一份会让模块段与字段位的判定分叉。四个**不可 raw** 的词（`self` / `Self` /
/// `super` / `crate`）由装载期写闸拒收，故本函数结构上收不到它们。
fn field_ident(wire: &str) -> String {
    let characters: Vec<char> = wire.chars().collect();
    let mut name = String::new();
    for (index, character) in characters.iter().copied().enumerate() {
        if !character.is_ascii_alphanumeric() && character != '_' {
            if !name.is_empty() && !name.ends_with('_') {
                name.push('_');
            }
            continue;
        }
        if character.is_ascii_uppercase() {
            let previous = index.checked_sub(1).map(|at| characters[at]);
            let boundary = match previous {
                Some(previous) if previous.is_ascii_lowercase() || previous.is_ascii_digit() => {
                    true
                }
                Some(previous) if previous.is_ascii_uppercase() => characters
                    .get(index + 1)
                    .is_some_and(|next| next.is_ascii_lowercase()),
                _ => false,
            };
            if boundary && !name.is_empty() && !name.ends_with('_') {
                name.push('_');
            }
            name.push(character.to_ascii_lowercase());
            continue;
        }
        name.push(character.to_ascii_lowercase());
    }
    let name = name.trim_end_matches('_');
    if name.is_empty() {
        // 装载期已把契约名约束成 ASCII 标识符；本兜底只为直连渲染保住合法语法
        return "field".to_owned();
    }
    if is_rust_keyword(name) {
        return format!("r#{name}");
    }
    name.to_owned()
}

/// 本地字段名剥 `r#` 前缀后的串：serde 先剥 `r#` 再套 `rename_all`，故 round-trip 的
/// 比对必须用剥前缀后的名字（`r#type` 在 `camelCase` 下仍是 `type`）。
fn serde_name(wire: &str) -> String {
    let local = field_ident(wire);
    match local.strip_prefix(RAW_PREFIX) {
        Some(stripped) => stripped.to_owned(),
        None => local,
    }
}

/// 原始标识符前缀（`r#` 字面量的唯一持有者：产出与剥前缀两处共用同一常量）。
const RAW_PREFIX: &str = "r#";

/// 属性推导的**唯一入口**（纯函数：`shape + definition + 解析表 → 属性`）。
fn facets(shape: Shape, definition: &Value, view: &DefsView<'_>) -> Facets {
    let mut items: Vec<&'static str> = match shape {
        Shape::Trait => Vec::new(),
        Shape::Unit => vec!["Debug", "Clone", "Copy", "PartialEq", "Eq"],
        Shape::Struct => {
            let mut items = vec!["Debug", "Clone", "PartialEq"];
            if struct_eq_able(definition, view) {
                items.push("Eq");
            }
            items
        }
        Shape::Newtype => {
            let mut items = vec!["Debug", "Clone", "PartialEq"];
            if eq_able(definition, view, &mut Vec::new()) {
                items.push("Eq");
            }
            items
        }
        Shape::ClosedEnum => vec!["Debug", "Clone", "Copy", "PartialEq", "Eq", "Hash"],
        Shape::PayloadUnion => vec!["Debug", "Clone", "PartialEq"],
    };
    // wire 面判据：契约是否已定义线形态（零数据位 / 带负载 union / 含 `x-function` 字段
    // 的 struct 都没有线形态可承诺）——三项同进同出。
    let wire = match shape {
        Shape::Trait | Shape::Unit | Shape::PayloadUnion => false,
        Shape::Struct => definition.as_object().is_some_and(|object| {
            property_schemas(object).all(|(_, schema)| schema.get("x-function").is_none())
        }),
        Shape::Newtype | Shape::ClosedEnum => true,
    };
    let default_variant = match shape {
        Shape::ClosedEnum => default_variant(definition),
        _ => None,
    };
    if default_variant.is_some() {
        items.push("Default");
    }
    if wire {
        items.extend(["Serialize", "Deserialize", "specta::Type"]);
    }
    let derives: Vec<&'static str> = DERIVE_ORDER
        .iter()
        .copied()
        .filter(|item| items.contains(item))
        .collect();
    let (rename_all, per_item_rename) = if wire {
        serde_shape(shape, definition)
    } else {
        (None, false)
    };
    Facets {
        derives,
        rename_all,
        per_item_rename,
        default_variant,
    }
}

/// serde 形态自动选（`rename_all` 候选命中即容器级，全不命中才逐项 `rename`）。
fn serde_shape(shape: Shape, definition: &Value) -> (Option<&'static str>, bool) {
    let Some(object) = definition.as_object() else {
        return (None, true);
    };
    let struct_pairs = || {
        property_schemas(object)
            .map(|(wire, _)| (wire.clone(), serde_name(wire)))
            .collect::<Vec<_>>()
    };
    let variant_pairs = || {
        union_choices(definition)
            .into_iter()
            .flatten()
            .filter_map(|choice| choice.get("const").and_then(const_text))
            .map(|wire| {
                let local = variant_name(&wire);
                (wire, local)
            })
            .collect::<Vec<_>>()
    };
    let (candidates, area, pairs) = match shape {
        Shape::Struct => (FIELD_CASES.as_slice(), Area::Field, struct_pairs()),
        Shape::ClosedEnum => (VARIANT_CASES.as_slice(), Area::Variant, variant_pairs()),
        // trait / 零字段 / 标量 newtype / 带负载 union：无可 `rename_all` 的成员位
        _ => return (None, false),
    };
    for (case, rule) in candidates {
        if pairs
            .iter()
            .all(|(wire, local)| apply_case(local, *case, area) == *wire)
        {
            return (Some(rule), false);
        }
    }
    (None, true)
}

/// struct 的 `Eq` 条件：全部承载位可 `Eq`（`x-function` 字段不可 ⇒ 保守不推）。
fn struct_eq_able(definition: &Value, view: &DefsView<'_>) -> bool {
    definition.as_object().is_some_and(|object| {
        property_schemas(object).all(|(_, schema)| eq_able(schema, view, &mut Vec::new()))
    })
}

/// `Eq` 的环容忍 DFS（最小不动点：环上的假设是 `false`）。
///
/// 判序与 [`value_type`] 同形（`$ref` → 映射规则 → union → 可空 → `type`）：`string` /
/// `integer` / `boolean` ⇒ `true`；`number`（`f64`）⇒ `false`；`array` 看 `items`；对象字典
/// 看值、**具名结构体看字段**（`$ref` 跟随目标后落在这一臂：有字段的 `object`
/// 就是一个结构体，它的 `Eq` = 全部字段 `Eq`）；其余 `object`（零字段、内联对象与
/// `Value` 兜底）⇒ `false`；封闭枚举 ⇒ `true`；带负载 union ⇒ `false`；映射规则（`common.json`
/// 的 `Outcome` 一类）⇒ `false`（不假设映射后的 `Result` 实现 `Eq`）。不可解析的 `$ref` 与
/// 环一律 `false`——推不出来只是不推，MUST NOT 报错、MUST NOT 死循环。
fn eq_able(schema: &Value, view: &DefsView<'_>, visiting: &mut Vec<String>) -> bool {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if visiting.iter().any(|seen| seen == reference) {
            return false;
        }
        let Some(target) = def_entry(view.local, view.fqn, reference, view.schemas) else {
            return false;
        };
        visiting.push(reference.to_owned());
        let able = eq_able(target, view, visiting);
        visiting.pop();
        return able;
    }
    if schema.get("x-function").is_some() {
        return false;
    }
    if common_type(schema, Position::Field, view.common).is_some() {
        return false;
    }
    if let Some(choices) = union_choices(schema) {
        return is_closed_enum(choices);
    }
    if let Some(breakdown) = nullable_breakdown(schema) {
        return match breakdown.non_null {
            None => false,
            Some(inner) => eq_able(inner.as_ref(), view, visiting),
        };
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string" | "integer" | "boolean") => true,
        Some("array") => schema
            .get("items")
            .is_none_or(|items| eq_able(items, view, visiting)),
        Some("object") => match schema
            .get("additionalProperties")
            .filter(|value| value.is_object())
        {
            Some(values) => eq_able(values, view, visiting),
            // 具名结构体（`$defs` 条目被 `$ref` 跟随后落在这里）：`Eq` = 全部字段 `Eq`
            None => schema
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| {
                    !properties.is_empty()
                        && properties
                            .values()
                            .all(|field| eq_able(field, view, visiting))
                }),
        },
        Some(_) | None => false,
    }
}

/// `Default` 的条件（D-2）：`$defs.<T>.default` 在场且**命中某 `const` 分支** ⇒ 出
/// `Default` 并在该分支加 `#[default]`；未声明 / 值不命中 ⇒ 不出、不报错。
fn default_variant(definition: &Value) -> Option<usize> {
    let default = definition.get("default")?;
    union_choices(definition)?.iter().position(|choice| {
        choice
            .get("const")
            .is_some_and(|constant| constant == default)
    })
}

/// `///` 行的唯一产出点：经 [`rust_comment_text`] 折行后 `trim_end`（rustfmt 会剔除行尾
/// 空白，不 trim 就不是格式化器固定点）；空串 / 纯空白 / 缺省 / 非字符串 ⇒ `None`（零行）。
fn doc_line(description: Option<&Value>) -> Option<String> {
    let text = rust_comment_text(description?.as_str()?);
    let text = text.trim_end();
    if text.trim().is_empty() {
        return None;
    }
    Some(format!("/// {text}"))
}

/// `derive` 行（格式化器固定点）：组装串 ≤96 字符出单行，否则三行；项集为空 ⇒ 零字节。
///
/// 96 是从「rustfmt 默认 `max_width = 100`」实测锚出的：9 项（88 字符）保持单行、10 项
/// （97 字符，`dto.rs::GateTier` 即在案锚）折成 `#[derive(` / `    <全部项>,` / `)]`。
fn derive_line(items: &[&'static str]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let joined = items.join(", ");
    let assembled = format!("#[derive({joined})]");
    if assembled.chars().count() <= 96 {
        return format!("{assembled}\n");
    }
    format!("#[derive(\n    {joined},\n)]\n")
}

/// 容器级 serde 属性行（`rename_all` 在场且 wire 面在场时才有）。
fn container_serde(facets: &Facets) -> String {
    match facets.rename_all {
        Some(rule) if facets.wire() => format!("#[serde(rename_all = \"{rule}\")]\n"),
        _ => String::new(),
    }
}

/// 契约字符串字面量的唯一转义点（`serde(rename = …)` 的值可能来自任意 `const` 文本：
/// 引号 / 反斜杠 / 换行）。`{:?}` 产出的 Rust 字符串字面量恒合法，MUST NOT 手拼引号。
fn string_literal(text: &str) -> String {
    format!("{text:?}")
}

/// 单个 `$defs` 条目的渲染（类型级 `///` → derive 行 → 容器 serde 属性 → 形态 body）。
///
/// 返回（文本, wire 面在场）：后者决定头注释后是否补 `use serde::{…}`。
fn type_block(name: &str, definition: &Value, view: &DefsView<'_>) -> (String, bool) {
    let Some(object) = definition.as_object() else {
        // 非对象条目在装载期即被拒（`$defs.<名>` 的值 MUST 是对象）；本分支只为直连渲染
        // 的稳健性保留，MUST NOT 在此长出形态
        return (format!("pub struct {name};\n"), false);
    };
    if object.contains_key("$ref") {
        // 纯 `$ref` alias 由透传段接管（本文不重复定义）
        return (String::new(), false);
    }
    let shape = shape_of(definition);
    let facets = facets(shape, definition, view);
    let wire = facets.wire();
    let mut block = String::new();
    if let Some(line) = doc_line(object.get("description")) {
        block.push_str(&line);
        block.push('\n');
    }
    block.push_str(&derive_line(&facets.derives));
    block.push_str(&container_serde(&facets));
    let body = match shape {
        Shape::Trait => trait_block(name, object, view.common),
        Shape::Unit => format!("pub struct {name};\n"),
        Shape::Newtype => format!(
            "pub struct {name}(pub {});\n",
            scalar_type(object).unwrap_or("String")
        ),
        Shape::Struct => struct_block(name, object, view, &facets),
        Shape::ClosedEnum | Shape::PayloadUnion => variants_block(name, definition, view, &facets),
    };
    block.push_str(&body);
    (block, wire)
}

/// `object` → `pub struct`（非 `required` 字段 → `Option<T>`，经 [`optional_of`] 幂等
/// 包装：可选与可空叠加时至多一层）。
///
/// 字段序固定为「字段 `///` → 逐项 `#[serde(rename)]`（仅容器级候选全不命中且 wire 面
/// 在场时）→ 字段行」；本地名经 [`field_ident`] 派生（关键字出 `r#`），MUST NOT 在字段面
/// 另写一套命名。
fn struct_block(
    name: &str,
    object: &Map<String, Value>,
    view: &DefsView<'_>,
    facets: &Facets,
) -> String {
    let required = required_of(object);
    let mut body = String::new();
    for (wire, schema) in property_schemas(object) {
        if let Some(line) = doc_line(schema.get("description")) {
            body.push_str("    ");
            body.push_str(&line);
            body.push('\n');
        }
        if facets.wire() && facets.per_item_rename {
            body.push_str(&format!(
                "    #[serde(rename = {})]\n",
                string_literal(wire)
            ));
        }
        let local = field_ident(wire);
        if schema.get("x-function").is_some() {
            // 函数指针字段：与模块操作不互相吞并（`x-function` 的唯一可辨性来源）
            let function = schema.get("x-function").unwrap_or(&Value::Null);
            body.push_str(&format!(
                "    pub {local}: {},\n",
                function_pointer(function, view.common)
            ));
            continue;
        }
        let base = value_type(schema, Position::Field, view.common);
        let ty = if required.iter().any(|item| item == wire) {
            base
        } else {
            optional_of(base)
        };
        body.push_str(&format!("    pub {local}: {ty},\n"));
    }
    if body.is_empty() {
        return format!("pub struct {name};\n");
    }
    format!("pub struct {name} {{\n{body}}}\n")
}

/// `x-class` → `pub trait`（仅允许方法）。带属性即契约错误——错误侧由
/// [`super::super::schema::verify_defs`] 在装载期拦住（本函数因此只会看到方法）；
/// 反方向的「`properties` 全为函数却未标 `x-class`」是警告，由
/// [`super::render_target`] 在语言分派处提示。两侧都不在本渲染器内。
///
/// body 三段按序：`properties` 方法 → 构造器（`fn new(<params>) -> Self;`）→ `x-static`
/// 成员（`fn <名><method_signature>;`），各按 `serde_json::Map` 迭代序。构造器与静态方法的
/// 读写面来自装载期的形态判据（`returns` / `async` 已在那里禁掉），本函数不重复判错。
fn trait_block(
    name: &str,
    object: &Map<String, Value>,
    common: &BTreeMap<String, CommonRule>,
) -> String {
    let mut body = String::new();
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (method, schema) in properties {
            let Some(function) = schema.get("x-function") else {
                continue;
            };
            body.push_str(&format!(
                "    fn {method}{};\n",
                method_signature(function, common)
            ));
        }
    }
    if let Some(constructor) = object
        .get(X_CONSTRUCTOR)
        .and_then(|constructor| constructor.get("x-function"))
    {
        // 返回面由语义固定为本类型（装载期已禁 `returns`），故不读它
        body.push_str(&format!(
            "    fn new({}) -> Self;\n",
            param_list(constructor, common)
        ));
    }
    if let Some(statics) = object.get(X_STATIC).and_then(Value::as_object) {
        for (member, value) in statics {
            let Some(function) = value.get("x-function") else {
                continue;
            };
            body.push_str(&format!(
                "    fn {member}{};\n",
                method_signature(function, common)
            ));
        }
    }
    format!("pub trait {name} {{\n{body}}}\n")
}

/// 值映射的兜底文本（D59 ④ 的判定阈值）：`value_type` 判不出具体形时的返回值。
///
/// 单点常量而非两处字面量——`variants_block` 的「不可作负载」判定与 `value_type` 的
/// 兜底分支必须逐字同源，否则改兜底文本会让判定**静默失效**（编译期无提示）。
const FALLBACK_VALUE_TYPE: &str = "serde_json::Value";

/// `const` 分支的契约文本（字符串原样；数字 / 布尔 `to_string()`）——封闭枚举变体的
/// 唯一取值来源。非标量 `const`（`null` / 数组 / 对象）⇒ `None`，落既有的负载兜底形态。
fn const_text(constant: &Value) -> Option<String> {
    match constant {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// `oneOf` / `anyOf` → `pub enum`。
///
/// 两个来路共用本函数：`const` 分支 → unit variant（变体名经 [`variant_name`] 归一，
/// 上方可有变体 `///` 与 `#[default]`）；其余分支仍按既有形态落带负载变体——
/// 负载与判定**都来自 [`value_type`]**（D59）：返回兜底文本 [`FALLBACK_VALUE_TYPE`] 即
/// 不可作负载（落 `Variant{index}`），否则出带负载变体。MUST NOT 在此重写第二份标量表。
fn variants_block(name: &str, definition: &Value, view: &DefsView<'_>, facets: &Facets) -> String {
    let mut body = String::new();
    for (index, choice) in union_choices(definition).into_iter().flatten().enumerate() {
        if let Some(line) = doc_line(choice.get("description")) {
            body.push_str("    ");
            body.push_str(&line);
            body.push('\n');
        }
        if let Some(wire) = choice.get("const").and_then(const_text) {
            if facets.wire() && facets.per_item_rename {
                body.push_str(&format!(
                    "    #[serde(rename = {})]\n",
                    string_literal(&wire)
                ));
            }
            if facets.default_variant == Some(index) {
                body.push_str("    #[default]\n");
            }
            body.push_str(&format!("    {},\n", variant_name(&wire)));
            continue;
        }
        let payload = value_type(choice, Position::Field, view.common);
        if payload == FALLBACK_VALUE_TYPE {
            body.push_str(&format!("    Variant{index},\n"));
        } else {
            let variant = format!("{}{payload}", variant_name(&payload));
            body.push_str(&format!("    {variant}({payload}),\n"));
        }
    }
    format!("pub enum {name} {{\n{body}}}\n")
}

/// 操作签名 → 编译期见证行。
fn op_witness(op: &ContractOp, common: &BTreeMap<String, CommonRule>) -> String {
    let params = op
        .signature
        .get("params")
        .and_then(Value::as_array)
        .map(|params| {
            params
                .iter()
                .map(|param| {
                    value_type(
                        param.get("schema").unwrap_or(param),
                        Position::Param,
                        common,
                    )
                })
                .collect::<Vec<String>>()
                .join(", ")
        })
        .unwrap_or_default();
    let returns = match op.signature.get("returns") {
        Some(schema) => value_type(schema, Position::Return, common),
        None => "()".to_owned(),
    };
    let reference = match op.signature.get("owner").and_then(Value::as_str) {
        Some(owner) => format!("super::{owner}::{}", op.id),
        None => format!("super::{}", op.id),
    };
    format!("const _: fn({params}) -> {returns} = {reference};")
}

/// 入参序列（`name: T` 以 `, ` 连接；`params` 缺省 / 非数组即空串）——**入参拼装的唯一
/// 单点**：`method_signature` / `function_pointer` / trait body 的构造器三处共用，MUST NOT
/// 存在第二份签名拼装（`value_type` 的 `Position::Param` 分派面因此仍只此一条出口）。
fn param_list(function: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    function
        .get("params")
        .and_then(Value::as_array)
        .map(|params| {
            params
                .iter()
                .map(|param| {
                    let name = param.get("name").and_then(Value::as_str).unwrap_or("arg");
                    let schema = param.get("schema").unwrap_or(param);
                    format!("{name}: {}", value_type(schema, Position::Param, common))
                })
                .collect::<Vec<String>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// 方法签名尾段（`(<入参序列>) -> <返回>`；无返回即 `(<入参序列>)`）。
fn method_signature(function: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    let params = param_list(function, common);
    match function.get("returns") {
        Some(schema) => {
            let returns = value_type(schema, Position::Return, common);
            if returns == "()" {
                format!("({params})")
            } else {
                format!("({params}) -> {returns}")
            }
        }
        None => format!("({params})"),
    }
}

/// 函数指针字段类型（`fn(<入参序列>) -> <返回>`，无返回即 `fn(<入参序列>)`）。
fn function_pointer(function: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    let signature = method_signature(function, common);
    format!("fn{signature}")
}

/// 值 → Rust 类型（位置 × 语言 → 文本形的唯一实现；位置词汇取自 [`Position`]，
/// 本文件只 `use` 不自建——D43）。
///
/// **`value_type` 零 `x-function` 分支是不变量**（D47 ②）：`x-function` 的合法落点被
/// [`crate::api::contract::verify`] 的落点闸封闭在四处（模块 op 顶层签名、`$defs` 条目第一层
/// `properties` 的值、`x-class` 条目的构造入口与静态方法），故本函数结构上收不到「函数作为值」
/// 的形态——深层 `x-function` 在装载期即契约错误，TS 侧 `value_type` 同批收敛为同一形态
/// （MUST NOT 在两侧任一侧重开该分支，否则同一 schema 两侧产物形态不同）。
/// 映射规则查表（D54 ③ 的唯一查表面）：承载键经 [`protocol_key`] 求出，命中即取本
/// 语言槽位并把 `{}` 换成本 schema 的内容位类型；未命中 / 无内容位 ⇒ 交回普通
/// `type` 分派。**渲染器零 IO、五参：多一份只读解析表，形状面不扩**（规则表已进
/// [`ApiMeta`]，解析表只供 `$ref` 的 facet 递归）。
fn common_type(
    schema: &Value,
    position: Position,
    common: &BTreeMap<String, CommonRule>,
) -> Option<String> {
    for (name, rule) in common {
        let key = protocol_key(name);
        if let Some(inner) = schema.get(&key).filter(|_| !rule.rust.is_empty()) {
            return Some(
                rule.rust
                    .replace("{}", &value_type(inner, position, common)),
            );
        }
    }
    None
}

/// 值 → 文本形（规则表是唯一映射面；`common_type` 递归时回到这里）。
fn value_type(schema: &Value, position: Position, common: &BTreeMap<String, CommonRule>) -> String {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return ref_name(reference);
    }
    if let Some(mapped) = common_type(schema, position, common) {
        // 跨语言映射规则的唯一展开点（D54）：Rust 侧信封文本形来自 `common.json`
        // 的 `rust` 槽位，MUST NOT 在此硬编码（改表即变产物）。
        return mapped;
    }
    if let Some(breakdown) = nullable_breakdown(schema) {
        // 与 `schema_type` 同序（`$ref` → 映射规则 → nullable → `type` 分派）：形状识别
        // 单点在 `nullable_breakdown`，本文件 MUST NOT 二次判 null。纯 null 档保持既有
        // 兜底 `serde_json::Value`（MUST NOT 出 `()`），`Option<>` 只经 `optional_of` 包一次。
        return match breakdown.non_null {
            None => FALLBACK_VALUE_TYPE.to_owned(),
            Some(inner) => {
                let inner = value_type(inner.as_ref(), position, common);
                if breakdown.explicit_null {
                    optional_of(inner)
                } else {
                    inner
                }
            }
        };
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => match position {
            Position::Param => "&str".to_owned(),
            Position::Return | Position::Field => "String".to_owned(),
        },
        // 宽度与 `scalar_type` 同源（`integer_type` 单点）；非对象入参不可达，兜底 `i64`
        Some("integer") => schema.as_object().map_or_else(
            || "i64".to_owned(),
            |object| integer_type(object).to_owned(),
        ),
        Some("number") => "f64".to_owned(),
        Some("boolean") => "bool".to_owned(),
        Some("array") => {
            let items = schema
                .get("items")
                .map(|items| value_type(items, Position::Field, common))
                .unwrap_or_else(|| "String".to_owned());
            match position {
                Position::Param => format!("&[{items}]"),
                Position::Return | Position::Field => format!("Vec<{items}>"),
            }
        }
        Some("object") => match schema
            .get("additionalProperties")
            .filter(|value| value.is_object())
        {
            Some(values) => format!(
                "BTreeMap<String, {}>",
                value_type(values, Position::Field, common)
            ),
            None => FALLBACK_VALUE_TYPE.to_owned(),
        },
        Some(_) | None => FALLBACK_VALUE_TYPE.to_owned(),
    }
}

/// 整数宽度（**严格完整值域**的唯一判定点；[`value_type`] 与 [`scalar_type`] 共用）。
///
/// `exclusiveMinimum` / `exclusiveMaximum` 先 ±1 归一成闭区间，再只认三个完整值域；
/// 缺 bounds / 单边 / 部分范围 / 畸形区间一律 `i64`。`maximum: 100` MUST NOT 推 `u8`
/// ——业务约束 ≠ 本地精度（改一个 `maximum` 就静默换类型是耦合，不是能力）。
fn integer_type(object: &Map<String, Value>) -> &'static str {
    if let (Some(minimum), Some(maximum)) =
        (integer_lower_bound(object), integer_upper_bound(object))
    {
        if minimum == 0 && maximum == i128::from(u32::MAX) {
            return "u32";
        }
        if minimum == 0 && maximum == i128::from(u64::MAX) {
            return "u64";
        }
        if minimum == i128::from(i32::MIN) && maximum == i128::from(i32::MAX) {
            return "i32";
        }
    }
    "i64"
}

/// 闭区间下界（`minimum` 优先；`exclusiveMinimum` +1 归一）。
fn integer_lower_bound(object: &Map<String, Value>) -> Option<i128> {
    if let Some(value) = integer_value(object.get("minimum")) {
        return Some(value);
    }
    integer_value(object.get("exclusiveMinimum"))?.checked_add(1)
}

/// 闭区间上界（`maximum` 优先；`exclusiveMaximum` −1 归一）。
fn integer_upper_bound(object: &Map<String, Value>) -> Option<i128> {
    if let Some(value) = integer_value(object.get("maximum")) {
        return Some(value);
    }
    integer_value(object.get("exclusiveMaximum"))?.checked_sub(1)
}

/// `serde_json::Number` → 精确整数：只走 `as_u64` / `as_i64`（不经 `f64`——`u64::MAX` 的
/// 20 位字面量必须精确命中）。
fn integer_value(value: Option<&Value>) -> Option<i128> {
    let value = value?;
    if let Some(unsigned) = value.as_u64() {
        return Some(i128::from(unsigned));
    }
    value.as_i64().map(i128::from)
}

/// 可选包装的**唯一单点**（幂等）——[`struct_block`] 与 [`value_type`] 的可空分支都调它，
/// 故 `Option<Option<` 零产出（嵌套 `Option` 是退化产物，MUST NOT 出现）。
fn optional_of(ty: String) -> String {
    if ty.starts_with("Option<") && ty.ends_with('>') {
        return ty;
    }
    let mut wrapped = String::from("Option<");
    wrapped.push_str(&ty);
    wrapped.push('>');
    wrapped
}

/// 枚举值 / 类型名 → Rust 变体名（驼峰；非标识符字符当分隔）。
///
/// **不变量**：返回值 MUST 是合法 Rust 标识符。变体值来自契约的任意文本（可能含
/// `'` / 空格 / 首字符数字），本函数是把它**无损归一**为标识符的唯一落点（非标识符
/// 字符即分隔符、首字符为 ASCII 数字加 `V` 前缀、空串退化 `Unknown`）。这是归一而非
/// 转义，故变体值不进 [`rust_comment_text`]。
fn variant_name(token: &str) -> String {
    let mut name = String::new();
    let mut upper = true;
    for character in token.chars() {
        if character.is_ascii_alphanumeric() {
            if upper {
                name.extend(character.to_uppercase());
                upper = false;
            } else {
                name.push(character);
            }
        } else {
            upper = true;
        }
    }
    if name.is_empty() {
        return "Unknown".to_owned();
    }
    if name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        return format!("V{name}");
    }
    name
}

/// 标量判据（`type` 为标量且无 `enum`）。
///
/// `!contains_key("enum")` 守卫在 `enum` 数组退役后不再有生产路径（装载期已拒收），但保留
/// 零成本，且防住「测试直连渲染」时把 `enum` 条目误判成标量。
fn is_scalar(object: &Map<String, Value>) -> bool {
    matches!(
        object.get("type").and_then(Value::as_str),
        Some("string" | "integer" | "number" | "boolean")
    ) && !object.contains_key("enum")
}

/// 零字段 `object` 判据（`Shape::Unit` 的唯一判据面：`type: "object"` 且 `properties`
/// 缺省或空）。
fn is_unit_object(object: &Map<String, Value>) -> bool {
    object.get("type").and_then(Value::as_str) == Some("object")
        && object
            .get("properties")
            .and_then(Value::as_object)
            .map(Map::is_empty)
            .unwrap_or(true)
}

/// 标量 → Rust 基础类型（`integer` 走 [`integer_type`] 同一判定）。
fn scalar_type(object: &Map<String, Value>) -> Option<&'static str> {
    match object.get("type").and_then(Value::as_str) {
        Some("string") => Some("String"),
        Some("integer") => Some(integer_type(object)),
        Some("number") => Some("f64"),
        Some("boolean") => Some("bool"),
        _ => None,
    }
}

/// 零字段 object 判据（决定是否由生成器自动产出零参 `new` 与构造器见证行）。
///
/// **`x-class` 条目排除**：trait 不是类型本体（`impl <trait>` 报 `E0404`、
/// `const _: fn() -> super::<trait>` 同样不可编译），自动构造器与见证行只对纯数据产出。
/// 唯一消费点是 `render_scope` 的 `constructible` 收集。
fn is_zero_field_object(definition: &Value) -> bool {
    definition.as_object().is_some_and(|object| {
        object.get("type").and_then(Value::as_str) == Some("object")
            && object.get("x-class").and_then(Value::as_bool) != Some(true)
            && object
                .get("properties")
                .and_then(Value::as_object)
                .map(Map::is_empty)
                .unwrap_or(true)
    })
}

/// `properties` 的（wire 名, 字段 schema）迭代（缺省 / 非对象 ⇒ 空迭代）。
fn property_schemas(object: &Map<String, Value>) -> impl Iterator<Item = (&String, &Value)> {
    object
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
}

/// `required` 数组（缺省空）。
fn required_of(object: &Map<String, Value>) -> Vec<&str> {
    object
        .get("required")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}
