//! `.i.ts` 渲染：`export interface` / `as const` + `type` / 别名 / union /
//! 透传 `export type { … } from` / 头注释。
//!
//! **值声明只在构造器或静态方法在场时产出**：`x-class` 条目保持 `export interface T { … }`
//! （形状承诺），并在 `x-constructor` / `x-static` 至少一项在场时追加同名值声明
//! `export declare const T: { new (…): T; <名> (…): R };`（类型级可调用成员只有值位置表达得
//! 了）。两者皆无时零值声明——产物与今天逐字节相同。
//!
//! **ambient-free**：产物是普通模块（MUST NOT 用 `.d.ts`），实现侧 `import type` 引用
//! 并标注返回类型——形状不符即 `tsc` 报 TS2322（TS 侧形状校验的唯一机制）。
//!
//! **两类降级显式留痕**（`x-common.degraded` 语义）：`x-class`（TS 结构化类型无法强制
//! 实现）与错误通道走 `Promise` reject（不在类型系统内）。标量别名是**按设计退化、
//! 不记 degraded**；MUST NOT 做品牌类型（`number & { __brand }` 会让 cast 传染）。
//!
//! **`x-class` 的两侧判据不在本文件**：带属性即契约错误由
//! [`crate::api::schema::verify_defs`] 判、`properties` 全为函数却未标的提示由
//! [`super::render_target`]（语言分派处）发——本文件零 `eprintln!`（同一条契约两侧
//! 各报一次即噪声）。
//!
//! 边界：纯函数出 `String`，零写盘、零 `../` 手算（specifier 由
//! [`crate::api::paths`] 的 `ts_import_specifier` 单点产出）。

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{declared_ops, local_defs, notice_lines, reexport_defs, EmitTarget, ModuleRef};
use crate::api::contract::{is_constructor, Contract, X_CONSTRUCTOR, X_STATIC};
use crate::api::extract::{nullable_breakdown, Position};
use crate::api::paths::{self, EntryForm};
use crate::api::{protocol_key, reexport_target, ref_name, ApiMeta, CommonRule};

/// TS 字符串字面量的唯一转义点（D45 闸二）：产出**单引号**串。
///
/// 一切来自契约的字符串（`enum` 成员、`const` 值）都经它——契约文本是任意的，值含
/// `'` / 反斜杠 / 换行即能注入出非法 TS。**MUST NOT 借 JSON 序列化器生成字面量**：它会
/// 产出双引号形态，破坏生成物「单引号」的格式化器不动点前提（`vp fmt` 的稳定态）。
pub fn ts_string_literal(text: &str) -> String {
    let mut out = String::from("'");
    for character in text.chars() {
        match character {
            '\\' => {
                out.push('\\');
                out.push('\\');
            }
            '\'' => {
                out.push('\\');
                out.push('\'');
            }
            '\n' => {
                out.push('\\');
                out.push('n');
            }
            '\r' => {
                out.push('\\');
                out.push('r');
            }
            '\t' => {
                out.push('\\');
                out.push('t');
            }
            '\u{2028}' => {
                out.push('\\');
                out.push_str("u2028");
            }
            '\u{2029}' => {
                out.push('\\');
                out.push_str("u2029");
            }
            other => out.push(other),
        }
    }
    out.push('\'');
    out
}

/// 注释拼接的唯一转义点（D45 闸二）：`\n` / `\r` / `U+2028` / `U+2029` → 空格（防断行），
/// 其余原样。`// 透传自 <FQN>` 一类拼接都经它。
pub fn ts_comment_text(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '\n' | '\r' | '\u{2028}' | '\u{2029}' => ' ',
            other => other,
        })
        .collect()
}

/// `.i.ts` 全文（幂等：同输入两次渲染逐字相等；类型段按 `$defs` 声明序）。
///
/// **唯一出口**（D46）：入口形态与透传查表由 [`EmitTarget`] 给定——本模块只有这一个
/// 模块级渲染导出，MUST NOT 另立「空 scope 的退化形态」孪生（退化关系由调用方传空
/// `modules` + `EntryForm::SubdirModule` 的 `EmitTarget` 表达）。
///
/// **零形态推导**（D41）：specifier 形态取 [`EmitTarget::form`]（调用方经
/// `paths::entry_state` 的 `Resolved` 求出），本函数 MUST NOT 自行判断「末段是否含 `.`」
/// ——那是入口解析链的判据，两套判据必然漂移。
/// **零 FQN 派生**（D40）：透传条目的 `<name>` 取 [`ModuleRef::name`]（目标元素根路径
/// 末段），查不到目标即只出注释、不产 specifier——MUST NOT 退化到 FQN 末段。
/// specifier 值仍由 [`paths::ts_import_specifier`] 产出（零手算 `../` 层数）——**其
/// 入口同级的真值待前端契约变更接线**，本期输出 MUST NOT 当既有真值（本仓零 `.i.ts`）。
pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String {
    render_scope(contract, schema, meta, target.form, &target.modules)
}

/// 渲染主体（本文件 [`render`] 的唯一实现）：形态与透传查表由调用方给。
fn render_scope(
    contract: &Contract,
    schema: &Value,
    meta: &ApiMeta,
    form: EntryForm,
    modules: &BTreeMap<String, ModuleRef>,
) -> String {
    let mut out = String::new();
    for line in notice_lines(meta) {
        out.push_str("// ");
        out.push_str(&ts_comment_text(&line));
        out.push('\n');
    }
    out.push('\n');

    let common = &meta.common;
    let locals = local_defs(schema);
    let reexports = reexport_defs(schema);

    // 类型段（`$defs` 声明序；零排序、零按结构合并）
    for (name, definition) in &locals {
        let block = type_block(name, definition, common);
        if !block.is_empty() {
            out.push_str(&block);
            out.push('\n');
        }
    }

    // 操作段：模块级函数 → `export type <Op> = (…) => R`
    for op in declared_ops(contract) {
        if is_constructor(&op.id) {
            continue;
        }
        let function = function_of(&op.signature);
        out.push_str(&format!(
            "export type {} = {};\n\n",
            op.id,
            function_type(function, common)
        ));
    }

    // 透传段（`isolatedModules` 下标 `export type`）：`<name>` 只来自调用方给的查表
    for (name, reference) in &reexports {
        let Some(target) = reexport_target(&contract.fqn, reference) else {
            continue;
        };
        let Some(module) = modules.get(&target) else {
            // 目标元素不在模型里 → 模块名无从派生（MUST NOT 退化到 FQN 末段）
            out.push_str(&format!("// 透传自 {}\n\n", ts_comment_text(&target)));
            continue;
        };
        // specifier 的 `<name>` 只来自调用方查表，但**末段是任意文本**（模型里的元素根可带
        // 扩展名 / 引号 / 反斜杠 / 换行）⇒ 拼进单引号串必先经本文件唯一的字面量转义点
        // （D45 闸二；`from '<裸拼>'` 形态即该闸在 TS 侧的落点，D48 ② 把它改为经
        // `ts_string_literal` 产出，与 `enum` 成员、`const` 值同一条纪律）。
        let specifier = ts_string_literal(&paths::ts_import_specifier(form, &module.name));
        out.push_str(&format!(
            "// 透传自 {}\nexport type {{ {name} }} from {specifier};\n\n",
            ts_comment_text(&target)
        ));
    }
    out
}

/// 单个 `$defs` 条目的 TS 形态（七种协议映射）。
fn type_block(name: &str, definition: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    let Some(object) = definition.as_object() else {
        return format!("export type {name} = unknown;\n");
    };
    if object.contains_key("$ref") {
        // 纯 `$ref` alias 由透传段接管（本文不重复定义）
        return String::new();
    }
    if object.get("x-class").and_then(Value::as_bool) == Some(true) {
        return interface_block(name, object, true, common);
    }
    if let Some(choices) = object
        .get("oneOf")
        .or_else(|| object.get("anyOf"))
        .and_then(Value::as_array)
    {
        let members: Vec<String> = choices
            .iter()
            .map(|choice| value_type(choice, Position::Field, common))
            .collect();
        return format!("export type {name} = {};\n", union_of(&members));
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        return enum_block(name, values);
    }
    if is_scalar(object) {
        // 标量 → 别名（按设计退化、不记 degraded；MUST NOT 品牌类型）
        return format!("export type {name} = {};\n", scalar_type(object));
    }
    interface_block(name, object, false, common)
}

/// `object` / `x-class` → `export interface`（非 `required` 字段 → `{field}?:`，
/// 零 `| null` 拼接——两个事实各只落自己的符号）。
///
/// `x-class` 条目在此之后按条件追加**同名值声明**（[`value_declaration`]）：interface 是形状
/// 承诺，值声明才是类型级可调用成员的载体（类型 / 值同名共存，`isolatedModules` 下成立）。
/// 构造器与静态方法皆无时零值声明——产物与今天逐字节相同。
fn interface_block(
    name: &str,
    object: &Map<String, Value>,
    class: bool,
    common: &BTreeMap<String, CommonRule>,
) -> String {
    let required = required_of(object);
    let mut body = String::new();
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (field, schema) in properties {
            if let Some(function) = schema.get("x-function") {
                // 函数指针字段不参与可选轴（`x-class` 方法面 MUST NOT 变可选）
                body.push_str(&format!(
                    "  {field}: {};\n",
                    function_type(function, common)
                ));
                continue;
            }
            let base = value_type(schema, Position::Field, common);
            let line = if required.iter().any(|item| item == field) {
                format!("  {field}: {base};\n")
            } else {
                format!("  {field}?: {base};\n")
            };
            body.push_str(&line);
        }
    }
    let mut out = String::new();
    if class {
        out.push_str("// x-common.degraded：结构化类型无法强制实现（本 interface 是形状承诺，不是实现约束）\n");
    }
    if body.is_empty() {
        out.push_str(&format!("export interface {name} {{}}\n"));
    } else {
        out.push_str(&format!("export interface {name} {{\n{body}}}\n"));
    }
    if class {
        out.push_str(&value_declaration(name, object, common));
    }
    out
}

/// `x-class` 条目的同名值声明（`export declare const T: { … };`）：`new` 在前、`x-static`
/// 成员按 `serde_json::Map` 迭代序，缩进 2 空格（同 interface body），紧跟 interface 的 `}`
/// 之后零空行。
///
/// **产出条件**：构造器在场 ∨ `x-static` 非空（`{}` 视同不在场）；两者皆无即零字节。
/// 成员读取只走 [`function_of`]（`x-function` 解包的唯一单点）；成员**不套整签名包装**
/// （`common_type` 的整签名形态留给 [`function_type`]）——值声明的成员是「可调用成员」而不是
/// 「函数类型」，返回位仍经 `value_type(returns, Return)` 拿到按位置的映射。
fn value_declaration(
    name: &str,
    object: &Map<String, Value>,
    common: &BTreeMap<String, CommonRule>,
) -> String {
    let constructor = object.get(X_CONSTRUCTOR).map(function_of);
    let statics = object.get(X_STATIC).and_then(Value::as_object);
    let has_statics = statics.is_some_and(|statics| !statics.is_empty());
    if constructor.is_none() && !has_statics {
        return String::new();
    }
    let mut body = String::new();
    if let Some(function) = constructor {
        body.push_str(&format!(
            "  new ({}): {name};\n",
            parameter_list(function, common)
        ));
    }
    for (member, value) in statics.into_iter().flatten() {
        let function = function_of(value);
        body.push_str(&format!(
            "  {member} ({}): {};\n",
            parameter_list(function, common),
            return_text(function, common)
        ));
    }
    format!("export declare const {name}: {{\n{body}}};\n")
}

/// `enum` → `as const` 数组 + 同名类型（`isolatedModules` 下同名值 / 类型共存）。
fn enum_block(name: &str, values: &[Value]) -> String {
    let members: Vec<String> = values
        .iter()
        .map(|value| match value {
            Value::String(text) => ts_string_literal(text),
            other => other.to_string(),
        })
        .collect();
    format!(
        "export const {name} = [{}] as const;\nexport type {name} = (typeof {name})[number];\n",
        members.join(", ")
    )
}

/// `x-function` 对象（`api-op` fence 原文或 `$defs` 内嵌项同形）。
fn function_of(signature: &Value) -> &Value {
    signature.get("x-function").unwrap_or(signature)
}

/// `x-function` → `(<入参序列>) => <返回>`（`async` → `Promise<R>`）。
///
/// 两段文本各由 [`parameter_list`] / [`return_text`] 单点拼装；**整签名包装**
/// （`common_type` 的 `x-function.<承载键>` 形态）**只在本函数**——值声明成员不套它。
fn function_type(function: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    // 协议级包装（如异步信封）经**同一查表出口**（D54 ③）：命中即整条签名交回资产
    // 的 `typescript` 槽位，MUST NOT 在本函数另硬编码语言侧包装形。
    if let Some(mapped) = common_type(function, Position::Return, common) {
        return mapped;
    }
    format!(
        "({}) => {}",
        parameter_list(function, common),
        return_text(function, common)
    )
}

/// 入参序列（`name: T` 以 `, ` 连接；`optional` → `T | undefined`）——**入参拼装的唯一单点**：
/// `function_type` 与值声明成员两处共用（与字段面的 `{field}?:` 分立：可选参靠类型、可选字段
/// 靠标记）。
fn parameter_list(function: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    function
        .get("params")
        .and_then(Value::as_array)
        .map(|params| {
            params
                .iter()
                .map(|param| {
                    let name = param.get("name").and_then(Value::as_str).unwrap_or("arg");
                    let schema = param.get("schema").unwrap_or(param);
                    let mut ty = value_type(schema, Position::Param, common);
                    if param.get("optional").and_then(Value::as_bool) == Some(true) {
                        ty = nullable_of(ty);
                    }
                    format!("{name}: {ty}")
                })
                .collect::<Vec<String>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// 返回位文本（`returns` 缺省即 `void`；`async` → `Promise<R>`）——**返回位拼装的唯一单点**。
fn return_text(function: &Value, common: &BTreeMap<String, CommonRule>) -> String {
    let returns = match function.get("returns") {
        Some(schema) => value_type(schema, Position::Return, common),
        None => "void".to_owned(),
    };
    // 异步函数：错误通道走 Promise reject（不在类型系统内 → 记 degraded）
    if function.get("async").and_then(Value::as_bool) == Some(true) {
        return format!("Promise<{returns}>");
    }
    returns
}

/// 映射规则查表（D54 ③ 的唯一查表面）：承载键经 [`protocol_key`] 求出，命中即取
/// 本语言槽位并把 `{}` 换成本 schema 的内容位类型；未命中 / 无内容位 ⇒ 交回普通
/// `type` 分派。**渲染器零 IO、四参签名不扩**（规则表已进 [`ApiMeta`]）。
fn common_type(
    schema: &Value,
    position: Position,
    common: &BTreeMap<String, CommonRule>,
) -> Option<String> {
    for (name, rule) in common {
        let key = protocol_key(name);
        if let Some(inner) = schema.get(&key).filter(|_| !rule.typescript.is_empty()) {
            return Some(
                rule.typescript
                    .replace("{}", &value_type(inner, position, common)),
            );
        }
    }
    None
}

/// 值 → TS 类型（TS 侧字符串不区分入参 / 返回位：`position` 只作语义留痕；位置词汇本身
/// 取自 [`Position`]，本文件只 `use` 不自建——D43）。
///
/// **`value_type` 零 `x-function` 分支**（D47 ② 的不变量）：`x-function` 的合法落点被
/// [`crate::api::contract::verify`] 的落点闸封闭在二处（`api-op` 顶层签名与 `$defs`
/// 条目第一层 `properties` 的值），故本函数结构上收不到「函数作为值」的形态——深层
/// `x-function` 在装载期即契约错误。留一个分支即「同一 schema 两侧产物形态不同」的静默
/// 源（Rust 侧 `value_type` 零该分支，出 `serde_json::Value`；TS 侧出 `(…) => R`），
/// 且它会让「闸一 ④ 族已判 `params[].name`」这句断言在深层路径上失效。MUST NOT 在此
/// 再判一次位置（那是二次派生，D40 / D41 / D45 同款）。
fn value_type(schema: &Value, position: Position, common: &BTreeMap<String, CommonRule>) -> String {
    // `position` 在 TS 侧不影响产物形状（结构化类型的 `string` 两位置同形），保留参数
    // 以与 Rust 侧的调用形态对齐
    let _ = position;
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return ref_name(reference);
    }
    if let Some(mapped) = common_type(schema, position, common) {
        // 跨语言映射规则的唯一展开点（D54）：TS 侧信封文本形来自 `common.json`
        // 的 `typescript` 槽位，MUST NOT 在此硬编码（改表即变产物）。
        return mapped;
    }
    if let Some(breakdown) = nullable_breakdown(schema) {
        // 与 `schema_type` 同序（`$ref` → 映射规则 → nullable → `type` 分派）：形状识别
        // 单点在 `nullable_breakdown`，本文件 MUST NOT 二次判 null。纯 null 档落 `null`
        // （MUST NOT 是 `unknown`）；`| null` 只经 `nullable_of` 追一次。
        return match breakdown.non_null {
            None => "null".to_owned(),
            Some(inner) => {
                let inner = value_type(inner.as_ref(), position, common);
                if breakdown.explicit_null {
                    nullable_of(inner)
                } else {
                    inner
                }
            }
        };
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => "string".to_owned(),
        Some("integer") | Some("number") => "number".to_owned(),
        Some("boolean") => "boolean".to_owned(),
        Some("array") => {
            let items = schema
                .get("items")
                .map(|items| value_type(items, Position::Field, common))
                .unwrap_or_else(|| "unknown".to_owned());
            array_of(items)
        }
        Some("object") => match schema
            .get("additionalProperties")
            .filter(|value| value.is_object())
        {
            Some(values) => format!(
                "Record<string, {}>",
                value_type(values, Position::Field, common)
            ),
            None => "Record<string, unknown>".to_owned(),
        },
        Some(_) | None => "unknown".to_owned(),
    }
}

/// TS 侧 `| null` 的**唯一追加点**（幂等）：字段面的可空经 [`value_type`]、参数面的
/// 可选经 [`function_type`]，两处都调它，故 `| null` 至多一次；`interface_block`
/// MUST NOT 自行拼接。
fn nullable_of(ty: String) -> String {
    if ty.trim_end().ends_with("| null") {
        return ty;
    }
    format!("{ty} | null")
}

/// 数组元素位的唯一拼装点：元素类型含顶层 `|` 时必须加括号（`(T | null)[]`），
/// MUST NOT 出 `T | null[]` 这一优先级错误形态（映射规则的内容位同理）。
fn array_of(items: String) -> String {
    if items.contains(" | ") {
        return format!("({items})[]");
    }
    format!("{items}[]")
}

/// union 成员连接（单元素无悬垂 `|`）。
fn union_of(members: &[String]) -> String {
    match members {
        [] => "never".to_owned(),
        [only] => only.clone(),
        many => many.join(" | "),
    }
}

/// 标量判据（`type` 为标量且无 `enum`）。
fn is_scalar(object: &Map<String, Value>) -> bool {
    matches!(
        object.get("type").and_then(Value::as_str),
        Some("string" | "integer" | "number" | "boolean")
    ) && !object.contains_key("enum")
}

/// 标量 → TS 基础类型（别名，无品牌类型）。
fn scalar_type(object: &Map<String, Value>) -> &'static str {
    match object.get("type").and_then(Value::as_str) {
        Some("string") => "string",
        Some("integer") | Some("number") => "number",
        Some("boolean") => "boolean",
        _ => "unknown",
    }
}

/// `required` 数组（缺省空）。
fn required_of(object: &Map<String, Value>) -> Vec<&str> {
    object
        .get("required")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}
