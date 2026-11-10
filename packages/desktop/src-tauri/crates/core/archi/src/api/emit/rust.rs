//! `.i.rs` 渲染：类型定义（`struct` / `trait` / `enum` / newtype）+ 自动构造器 +
//! 透传 `pub use` + 头注释 + 签名见证。
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
//! 边界：纯函数出 `String`，零写盘。

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{declared_ops, local_defs, notice_lines, reexport_defs, EmitTarget, ModuleRef};
use crate::api::contract::{is_constructor, Contract, ContractOp};
use crate::api::extract::Position;
use crate::api::{protocol_key, reexport_target, ref_name, ApiMeta, CommonRule};

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
/// **零 FQN 派生**（D40）：`pub use` 的模块路径取 [`ModuleRef::module_path`]（目标元素
/// 根的 `crate::<末段>` 形态）——目标 FQN 的点分段不是 Rust 路径。查不到目标元素即只出
/// 注释、不产 `pub use`（模型里没有该元素 ⇒ 无路径可派生）。
pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String {
    render_scope(contract, schema, meta, &target.modules)
}

/// 渲染主体（本文件 [`render`] 的唯一实现）：透传查表由调用方给。
fn render_scope(
    contract: &Contract,
    schema: &Value,
    meta: &ApiMeta,
    modules: &BTreeMap<String, ModuleRef>,
) -> String {
    let mut out = String::new();
    for line in notice_lines(meta) {
        out.push_str("// ");
        out.push_str(&rust_comment_text(&line));
        out.push('\n');
    }
    out.push('\n');

    let common = &meta.common;
    let locals = local_defs(schema);
    let reexports = reexport_defs(schema);
    let constructible: Vec<String> = locals
        .iter()
        .filter(|(_, definition)| is_zero_field_object(definition))
        .map(|(name, _)| name.clone())
        .collect();

    // 类型段（`$defs` 声明序；零排序、零按结构合并）
    for (name, definition) in &locals {
        let block = type_block(name, definition, common);
        if !block.is_empty() {
            out.push_str(&block);
            out.push('\n');
        }
    }

    // 自动构造器（构造器是类型的推论：契约 MUST NOT 声明，生成器 MUST 产出）
    for name in &constructible {
        out.push_str(&format!(
            "impl {name} {{\n    /// 构造器（自动产出；契约 MUST NOT 声明构造器）\n    \
             pub fn new() -> Self {{\n        Self\n    }}\n}}\n\n"
        ));
    }

    // 透传（纯 `$ref` alias → `pub use <目标模块>::<名>`）：模块路径只来自调用方查表
    for (name, reference) in &reexports {
        let Some(target) = reexport_target(reference) else {
            continue;
        };
        let Some(module) = modules.get(&target) else {
            // 目标元素不在模型里 → 模块路径无从派生（MUST NOT 用 FQN 点分段冒充）
            out.push_str(&format!("// 透传自 {}\n\n", rust_comment_text(&target)));
            continue;
        };
        // 模块路径的每一段都 MUST 是合法 Rust 模块段（`pub use` 的模块段是**语法位置**）。
        // 该形态闸在装配期（`api::module_refs` 的 Rust 分支）已执行：非法目标根本不入查表，
        // 故本渲染器结构上收不到非法值 —— 查表未命中即按「目标不在模型里」同口径降级为
        // 注释。**MUST NOT** 在渲染期重复判定形态（二次派生，D40 / D41 / D45 同款）。
        out.push_str(&format!(
            "// 透传自 {}\npub use {}::{name};\n\n",
            rust_comment_text(&target),
            module.module_path
        ));
    }

    // 签名见证（每 op 一行 + 每个可构造类型的 `new` 一行；`super::` 前缀是硬约束）
    let witnesses: Vec<String> = declared_ops(contract)
        .into_iter()
        .filter(|op| !is_constructor(&op.id))
        .map(|op| op_witness(op, common))
        .chain(
            constructible
                .iter()
                .map(|name| format!("const _: fn() -> super::{name} = super::{name}::new;")),
        )
        .collect();
    if !witnesses.is_empty() {
        out.push_str("// 签名见证：契约与实现的编译期绑定（参数数或类型不符即 E0308）\n");
        for witness in witnesses {
            out.push_str(&witness);
            out.push('\n');
        }
    }
    out
}

/// 单个 `$defs` 条目的 Rust 形态（七种协议映射）。
fn type_block(name: &str, definition: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    let Some(object) = definition.as_object() else {
        return format!("pub struct {name};\n");
    };
    if object.contains_key("$ref") {
        // 纯 `$ref` alias 由透传段接管（本文不重复定义）
        return String::new();
    }
    if object.get("x-class").and_then(Value::as_bool) == Some(true) {
        return trait_block(name, object, common);
    }
    if let Some(choices) = object
        .get("oneOf")
        .or_else(|| object.get("anyOf"))
        .and_then(Value::as_array)
    {
        return variants_block(name, choices, common);
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        return enum_block(name, values);
    }
    if is_scalar(object) {
        return format!(
            "pub struct {name}(pub {});\n",
            scalar_type(object).unwrap_or("String")
        );
    }
    struct_block(name, object, common)
}

/// `object` → `pub struct`（非 `required` 字段 → `Option<T>`）。
fn struct_block(
    name: &str,
    object: &Map<String, Value>,
    common: &BTreeMap<String, CommonRule>,
) -> String {
    let required = required_of(object);
    let mut body = String::new();
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (field, schema) in properties {
            if schema.get("x-function").is_some() {
                // 函数指针字段：与模块操作不互相吞并（`x-function` 的唯一可辨性来源）
                let function = schema.get("x-function").unwrap_or(&Value::Null);
                body.push_str(&format!(
                    "    pub {field}: {},\n",
                    function_pointer(function, common)
                ));
                continue;
            }
            let base = value_type(schema, Position::Field, common);
            let ty = if required.iter().any(|item| item == field) {
                base
            } else {
                format!("Option<{base}>")
            };
            body.push_str(&format!("    pub {field}: {ty},\n"));
        }
    }
    if body.is_empty() {
        // 零字段 struct：std 基础派生即可（零 serde 需求——元素 crate 未必有 serde 依赖）
        return format!("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct {name};\n");
    }
    format!("pub struct {name} {{\n{body}}}\n")
}

/// `x-class` → `pub trait`（仅允许方法）。带属性即契约错误——错误侧由
/// [`super::super::schema::verify_defs`] 在装载期拦住（本函数因此只会看到方法）；
/// 反方向的「`properties` 全为函数却未标 `x-class`」是警告，由
/// [`super::render_target`] 在语言分派处提示。两侧都不在本渲染器内。
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
    format!("pub trait {name} {{\n{body}}}\n")
}

/// `enum` → `pub enum`（变体名取值的驼峰形态；值 ↔ 变体映射由手写 `impl` 承担）。
fn enum_block(name: &str, values: &[Value]) -> String {
    let mut body = String::new();
    for value in values {
        let token = value.as_str().unwrap_or("unknown");
        body.push_str(&format!("    {},\n", variant_name(token)));
    }
    format!("pub enum {name} {{\n{body}}}\n")
}

/// 值映射的兜底文本（D59 ④ 的判定阈值）：`value_type` 判不出具体形时的返回值。
///
/// 单点常量而非两处字面量——`variants_block` 的「不可作负载」判定与 `value_type` 的
/// 兜底分支必须逐字同源，否则改兜底文本会让判定**静默失效**（编译期无提示）。
const FALLBACK_VALUE_TYPE: &str = "serde_json::Value";

/// `oneOf` / `anyOf` → 带负载 `pub enum`。
///
/// 分支负载与判定**都来自 [`value_type`]**（D59）：返回兜底文本
/// [`FALLBACK_VALUE_TYPE`] 即不可作负载（落 `Variant{index}`），否则出带负载变体。
/// MUST NOT 在此重写第二份标量表——同一 schema 经本函数与经 `value_type` 的负载文本
/// 逐字相等。
fn variants_block(name: &str, choices: &[Value], common: &BTreeMap<String, CommonRule>) -> String {
    let mut body = String::new();
    for (index, choice) in choices.iter().enumerate() {
        let payload = value_type(choice, Position::Field, common);
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

/// 方法签名尾段（`(<入参序列>) -> <返回>`；无返回即 `(<入参序列>)`）。
fn method_signature(function: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    let params = function
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
        .unwrap_or_default();
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
/// [`crate::api::contract::verify`] 的落点闸封闭在二处（`api-op` 顶层签名与 `$defs`
/// 条目第一层 `properties` 的值），故本函数结构上收不到「函数作为值」的形态——深层
/// `x-function` 在装载期即契约错误，TS 侧 `value_type` 同批收敛为同一形态（MUST NOT
/// 在两侧任一侧重开该分支，否则同一 schema 两侧产物形态不同）。
/// 映射规则查表（D54 ③ 的唯一查表面）：承载键经 [`protocol_key`] 求出，命中即取本
/// 语言槽位并把 `{}` 换成本 schema 的内容位类型；未命中 / 无内容位 ⇒ 交回普通
/// `type` 分派。**渲染器零 IO、四参签名不扩**（规则表已进 [`ApiMeta`]）。
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
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => match position {
            Position::Param => "&str".to_owned(),
            Position::Return | Position::Field => "String".to_owned(),
        },
        Some("integer") => "i64".to_owned(),
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

/// 枚举值 / 类型名 → Rust 变体名（驼峰；非标识符字符当分隔）。
///
/// **不变量**：返回值 MUST 是合法 Rust 标识符。`enum` 值来自契约的任意文本（可能含
/// `'` / 空格 / 首字符数字），本函数是把它**无损归一**为标识符的唯一落点（非标识符
/// 字符即分隔符、首字符为 ASCII 数字加 `V` 前缀、空串退化 `Unknown`）。这是归一而非
/// 转义，故 `enum` 值不进 [`rust_comment_text`]。
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
fn is_scalar(object: &Map<String, Value>) -> bool {
    matches!(
        object.get("type").and_then(Value::as_str),
        Some("string" | "integer" | "number" | "boolean")
    ) && !object.contains_key("enum")
}

/// 标量 → Rust 基础类型。
fn scalar_type(object: &Map<String, Value>) -> Option<&'static str> {
    match object.get("type").and_then(Value::as_str) {
        Some("string") => Some("String"),
        Some("integer") => Some("i64"),
        Some("number") => Some("f64"),
        Some("boolean") => Some("bool"),
        _ => None,
    }
}

/// 零字段 object 判据（决定是否产出自动构造器与构造器见证）。
fn is_zero_field_object(definition: &Value) -> bool {
    definition.as_object().is_some_and(|object| {
        object.get("type").and_then(Value::as_str) == Some("object")
            && object
                .get("properties")
                .and_then(Value::as_object)
                .map(Map::is_empty)
                .unwrap_or(true)
    })
}

/// `required` 数组（缺省空）。
fn required_of(object: &Map<String, Value>) -> Vec<&str> {
    object
        .get("required")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}
