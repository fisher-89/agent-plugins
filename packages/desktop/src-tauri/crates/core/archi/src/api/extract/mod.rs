//! 取数器分派与签名归一：按元素语言分派到 [`rust`] / [`ts`]，产出候选
//! [`ApiSymbol`]（名字 / 归属 / 形态 / 归一签名 / `visibility` / 位置）。
//!
//! **取数器非权威**：它只供 bootstrap 起草与判据的「代码导出集」一侧；契约权威在
//! `<archi_dir>/api/<Domain>/<FQN>.md`。两套事实源各自单点，判据在
//! [`crate::api::check`] 汇合。
//!
//! **构造器识别不在取数器**：取数器如实抽名、MUST NOT 过滤构造器——过滤只在两处调用
//! [`crate::api::contract::is_constructor`]（判据的声明集 / 导出集比对与 `emit` 的
//! 自动构造器产出）。
//!
//! **两个取数入口的能力面不同**：[`extract_element`] 是**整元素导出集**（从实现入口沿
//! `mod` 树走，判据与 bootstrap 的唯一取数面）；[`extract`] 是**单文件入口面**（只抽一份源
//! 文本），**本期产品路径零调用者**——判据与 bootstrap 一律经 `extract_element`，`extract`
//! 保留为「按文件而非按元素起草」这一能力面的预留接口（`extract/mod_test.rs` 的调用测的是
//! **接口正确性**，不是接线状态）；退出条件 = 首个「按文件起草」的 bootstrap 需求落地时
//! 接线。MUST NOT 为让判据变绿而新增产品调用者（「造消费者满足判据」比零调用者更坏），
//! MUST NOT 因本条把 `rust::extract_file` 的 `pub` 降级（删 `extract` 会让它产品侧零调用
//! 者，两件事必须一起不做）。
//!
//! **类型词汇三层各有唯一落点**（D43）：逻辑类型词汇 = [`ApiType`]；位置词汇 =
//! [`Position`]（`Param` / `Return` / `Field`，产品侧只此一处）；短形写出点 = 本文件的私有
//! `render`（仅 [`normalize_signature`] 与 [`signature_of`] 消费）。[`schema_type`] 是 JSON
//! Schema → 逻辑类型的单点，语言无关（「位置 × 语言 → 文本形」的映射在 `emit`，判据只调用
//! 本文件的公开函数、MUST NOT 自建第二份）。
//!
//! 边界：纯读源文本（调用方给 `source`）+ AST，零读盘、零写盘、零 spawn。

pub mod rust;
pub mod ts;

use std::path::Path;

use serde_json::Value;

use crate::dto::ArchiElement;

use super::paths::{self, Language};

/// 代码符号形态（封闭四值；构造器不单独成值——用 `is_constructor(name)` 判）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiSymbolKind {
    /// 类型（`struct` / `interface` / `class`）
    Type,
    /// trait / 形状承诺
    Trait,
    /// 枚举
    Enum,
    /// 函数（自由函数或方法）
    Function,
}

/// 归一后的逻辑类型（`Unknown` 使归一仍可比对且显式留痕）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiType {
    /// `&str`
    Str,
    /// `String`（自有字符串）
    OwnedString,
    /// 整型
    Int,
    /// 浮点
    Float,
    /// 布尔
    Bool,
    /// `&[T]` / `Vec<T>` / `T[]`
    List(Box<ApiType>),
    /// `BTreeMap<K, V>` / `Record<K, V>`
    Map(Box<ApiType>),
    /// `Option<T>` / `T | null`
    Optional(Box<ApiType>),
    /// `Result<T, String>` / `{ ok } | { err }`
    Outcome(Box<ApiType>),
    /// 具名类型
    Named(String),
    /// 无法归类（原文保留）
    Unknown(String),
}

/// 类型出现的位置（**位置词汇的唯一落点**，D43）。
///
/// 同一段 JSON Schema 在不同位置对应不同逻辑类型：`{"type": "string"}` 在入参位是
/// [`ApiType::Str`]（代码 `&str`）、在返回位与字段位是 [`ApiType::OwnedString`]
/// （代码 `String`）。故「schema → 逻辑类型」必须带位置，而位置词汇只在此处定义——
/// 两侧渲染器与判据一律 `use` 本枚举，MUST NOT 各建一份（`emit` 只负责
/// 「位置 × 语言 → 文本形」的映射，判据只调用本文件的 [`schema_type`] /
/// [`signature_of`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// 入参位
    Param,
    /// 返回位
    Return,
    /// 字段位与容器元素位（`items` / `additionalProperties`）
    Field,
}

/// 单参数（名字不参与签名比对，仅供 bootstrap 起草与人读）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiParam {
    /// 参数名
    pub name: String,
    /// 逻辑类型
    pub ty: ApiType,
    /// 可选（缺省值 / `Option` 语义）
    pub optional: bool,
}

/// 可见性（封闭四值，bootstrap 剪枝的机械判据）。
///
/// **词表封闭，本期可判值域按语言分列**：Rust 侧四值全可判（`pub mod` 链 / `pub(crate)` /
/// `#[cfg(test)]` 树 / 由调用方按引用事实降级 `Unused`）；TS 侧**生产面只产
/// [`Visibility::External`]**（有 `export`），其余三值留白——`Unused` 的降级路径存在但
/// 未接线（见下节），`CrateOnly` / `TestOnly` 的判据都是「谁引用了这个符号」这类跨文件
/// import 反向事实，而 TS 取数器只吃单文件 AST（`ts::extract_file(file, source)`），结构上
/// 拿不到；将来补齐所需的输入是 `scan` 的 `ImportFact`（specifier + `typeOnly`）与 knip
/// 报告的 files 条目。
///
/// **文件名族不得作 proxy**：`.test.` / `.spec.` / `__tests__` / `fixtures` 与引用关系无必然
/// 联系（`foo.test.ts` 可以只被生产代码 import，`foo.ts` 也可只被测试 import），用文件名冒充
/// `testOnly` 是伪判据，不是近似。
///
/// **`unused` 本期生产面不产出（未接线）**：knip 报告未接线（见待决 5 / 12），
/// [`crate::api::extract::ts::apply_knip_report`] 是**零产品调用者的预留接口**——它的签名与
/// 「`None` 即不降级」语义都已实现，`ts_test` 的五条用例测的是**接口正确性**，不是接线状态。
/// 接线是独立变更（需要确认 knip 报文形态与版本兼容性），MUST NOT 用「已实现」掩盖该留白。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// 跨元素可达（真承诺，保留）
    External,
    /// 仅同 crate 或同元素目录内可达（内部契约，降格保留）
    ///
    /// TS 侧本期不产出（留白，见枚举 doc）。
    CrateOnly,
    /// 全仓无引用者（伪公共，首选删除）
    Unused,
    /// 仅测试树引用（不进 API）
    ///
    /// TS 侧本期不产出（留白，见枚举 doc）。
    TestOnly,
}

impl Visibility {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::External => "external",
            Visibility::CrateOnly => "crateOnly",
            Visibility::Unused => "unused",
            Visibility::TestOnly => "testOnly",
        }
    }

    /// 全部合法值（清单单点）。
    pub const ALL: [Visibility; 4] = [
        Visibility::External,
        Visibility::CrateOnly,
        Visibility::Unused,
        Visibility::TestOnly,
    ];
}

/// 一个候选代码符号（bootstrap 与判据共用的中间形态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiSymbol {
    /// 符号名
    pub name: String,
    /// 归属类型名（方法 / 关联函数；自由函数为 `None`）
    pub owner: Option<String>,
    /// 形态
    pub kind: ApiSymbolKind,
    /// 参数清单（`Function` 才有）
    pub params: Vec<ApiParam>,
    /// 返回类型（`Function` 才有意义）
    pub returns: ApiType,
    /// 是否异步（`Function` 才有意义）
    pub is_async: bool,
    /// 定义所在行（1 起始）
    pub line: u32,
    /// 可见性
    pub visibility: Visibility,
}

/// 按元素语言分派取数（未知语言 → 空 `Vec`——不猜测）。
///
/// 单文件形态（bootstrap 的入口文件面）：只抽这一份源文本。元素 `path` 为空时
/// `<root>` 取空串（[`paths::module_name`] 对空串返回空串，`crate_name` 只作诊断
/// 留痕，不影响抽取面）——**MUST NOT 索引 `path[0]`**：容器元素无 `path`，索引会
/// panic 而「无 path 的元素问它自己的符号集」是合法调用。
///
/// **显式留白（D52）：本期产品路径零调用者**——判据与 bootstrap 一律经 [`extract_element`]
/// （整元素导出集），本函数是**单文件入口面**的预留接口；`extract/mod_test.rs` 的调用测的是
/// **接口正确性**，不是接线状态。退出条件 = 首个「按文件而非按元素起草」的 bootstrap 需求
/// 落地时接线（届时同批补判据与 spec 措辞）。MUST NOT 新增产品调用者，MUST NOT 删除接口。
pub fn extract(element: &ArchiElement, file: &str, source: &str) -> Vec<ApiSymbol> {
    let element_root = element.path.first().map(String::as_str).unwrap_or("");
    match paths::language_of(file) {
        Language::Rust => rust::extract_file(file, source, &paths::module_name(element_root)),
        Language::TypeScript => ts::extract_file(file, source),
        Language::Unknown => Vec::new(),
    }
}

/// 整元素导出集（判据的「代码导出集」唯一取数面）：从实现入口出发，按语言决定是否走
/// 模块树——一个模块的公开面散在入口与它声明的各模块文件里，只看入口会把大部分操作
/// 判成缺失。
pub fn extract_element(root: &Path, entry: &str) -> Vec<ApiSymbol> {
    match paths::language_of(entry) {
        Language::Rust => rust::extract_crate(root, entry),
        Language::TypeScript => std::fs::read_to_string(root.join(entry))
            .map(|source| ts::extract_file(entry, &source))
            .unwrap_or_default(),
        Language::Unknown => Vec::new(),
    }
}

/// 归一签名（`fn(<类型序列>) -> <返回类型>`）：参数名 / 空白 / 多行排列不参与，故
/// 契约 `input: string` 与代码 `input: &str` 归一后一致。判据比对与 bootstrap 投影
/// 共用本函数（MUST NOT 各写一份）。
pub fn normalize_signature(symbol: &ApiSymbol) -> String {
    let params = symbol
        .params
        .iter()
        .map(|param| render(&param.ty))
        .collect::<Vec<String>>()
        .join(", ");
    format!("fn({params}) -> {}", render(&symbol.returns))
}

/// 契约侧归一签名（与 [`normalize_signature`] 同一短形词汇；判据的唯一契约侧写出点）。
///
/// 入参是 `api-op` fence 原文：既接受裸形态（`params` / `returns` / `async` 三键），
/// 也接受 `{"x-function": …}` 包裹形态（装配后的 `properties` 值即该形态）。逐参数
/// `schema_type(param["schema"] 或 param, Position::Param)`，**`optional == true` 的参数
/// 包 [`ApiType::Optional`]**（漏该分支即「契约写可选参数、代码写 `Option<T>`」恒不等，
/// 属静默假阳）；返回位走 `Position::Return`。缺 `params` / `returns` 时按空序列 /
/// 空返回处理（与代码侧 `()` 的 [`ApiType::Unknown`] 留痕同形）。
pub fn signature_of(signature: &Value) -> String {
    let function = signature.get("x-function").unwrap_or(signature);
    let params = function
        .get("params")
        .and_then(Value::as_array)
        .map(|params| {
            params
                .iter()
                .map(|param| {
                    let ty = schema_type(param.get("schema").unwrap_or(param), Position::Param);
                    let ty = if param.get("optional").and_then(Value::as_bool) == Some(true) {
                        ApiType::Optional(Box::new(ty))
                    } else {
                        ty
                    };
                    render(&ty)
                })
                .collect::<Vec<String>>()
                .join(", ")
        })
        .unwrap_or_default();
    let returns = match function.get("returns") {
        Some(schema) => render(&schema_type(schema, Position::Return)),
        // 空返回是「无结果」而非「未知类型」，故不落 `Unknown` 的 `?` 前缀（与代码侧
        // 零返回的 `Unknown("()")` 同形命名，两侧都写 `()`）
        None => "()".to_owned(),
    };
    format!("fn({params}) -> {returns}")
}

/// JSON Schema 片段 → 逻辑类型（**schema → 类型的唯一单点**，D43）。
///
/// 语言无关：产出 [`ApiType`]，**零语言文本形**（`&str` / `Vec<T>` / `Record<…>` 一类是
/// `emit` 的射程）。判据与两侧渲染器一律经本函数取逻辑类型，MUST NOT 各写一份映射。
///
/// 判序固定：`$ref` → [`ApiType::Unknown`]（`ref_name` 剥 `#/$defs/` 取末段，与代码侧
/// 同名体系同形）；`outcome` 关键字 → [`ApiType::Outcome`]；nullable 三分支 → [`ApiType::Optional`]；
/// 其余按 `type` 分派（`string` 按位置分流 `Str` / `OwnedString`，`array` 与对象型
/// `additionalProperties` 的内容位固定 [`Position::Field`]）。
pub fn schema_type(schema: &Value, position: Position) -> ApiType {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return ApiType::Unknown(super::ref_name(reference));
    }
    if let Some(outcome) = schema.get("outcome") {
        return ApiType::Outcome(Box::new(schema_type(outcome, position)));
    }
    if let Some(inner) = nullable_inner(schema, position) {
        return ApiType::Optional(Box::new(inner));
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => match position {
            Position::Param => ApiType::Str,
            Position::Return | Position::Field => ApiType::OwnedString,
        },
        Some("integer") => ApiType::Int,
        Some("number") => ApiType::Float,
        Some("boolean") => ApiType::Bool,
        Some("array") => ApiType::List(Box::new(match schema.get("items") {
            Some(items) => schema_type(items, Position::Field),
            None => ApiType::Unknown(String::new()),
        })),
        Some("object") => match schema
            .get("additionalProperties")
            .filter(|value| value.is_object())
        {
            Some(values) => ApiType::Map(Box::new(schema_type(values, Position::Field))),
            None => ApiType::Unknown("object".to_owned()),
        },
        Some(other) => ApiType::Unknown(other.to_owned()),
        None => ApiType::Unknown(String::new()),
    }
}

/// nullable 三分支（`"type": "null"` / 含 `"null"` 的 `type` 数组 / `anyOf`-`oneOf` 内含
/// `{"type": "null"}` 分支）→ 非空分支的逻辑类型（由 [`schema_type`] 包成
/// [`ApiType::Optional`]）。非 nullable 形态 → `None`（交回常规分派）。
///
/// 位置语义不因 nullable 改变：`{"type": ["string", "null"]}` 在入参位仍按
/// [`Position::Param`] 解成 `Str`，故「契约写可选参数」与「代码写 `Option<T>`」两侧同形。
fn nullable_inner(schema: &Value, position: Position) -> Option<ApiType> {
    if let Some(branches) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        if branches.iter().any(is_null_schema) {
            return Some(
                match branches.iter().find(|branch| !is_null_schema(branch)) {
                    Some(branch) => schema_type(branch, position),
                    None => ApiType::Unknown("null".to_owned()),
                },
            );
        }
    }
    match schema.get("type") {
        Some(Value::String(kind)) if kind == "null" => Some(ApiType::Unknown("null".to_owned())),
        Some(Value::Array(kinds)) => {
            let mut names: Vec<&str> = kinds.iter().filter_map(Value::as_str).collect();
            if !names.contains(&"null") {
                return None;
            }
            names.retain(|name| *name != "null");
            let Some(inner_kind) = names.first() else {
                return Some(ApiType::Unknown("null".to_owned()));
            };
            let mut narrowed = schema.as_object()?.clone();
            narrowed.insert("type".to_owned(), Value::String((*inner_kind).to_owned()));
            Some(schema_type(&Value::Object(narrowed), position))
        }
        // 三形态穷尽（字符串 `null` / 类型数组 / 其余），零兜底臂：语言与形态的分派都不
        // 猜测——兜底臂会让「新形态」静默落进某个既有分支
        Some(_) | None => None,
    }
}

/// `{"type": "null"}` 判据（nullable 分支的识别面）。
fn is_null_schema(schema: &Value) -> bool {
    schema.get("type").and_then(Value::as_str) == Some("null")
}

/// 逻辑类型的规范短形（**唯一写出点**，D43）。
///
/// 唯一消费方是同文件的 [`normalize_signature`] 与 [`signature_of`]（公开函数），故
/// **保持私有**——升格为 `pub` 即成为「仅测试可见的导出」。`String` → `str`、
/// `OwnedString` → `String`、…、`Unknown(raw)` → `?raw`；`emit` / `check` MUST NOT 各拼
/// 一份短形。
fn render(ty: &ApiType) -> String {
    match ty {
        ApiType::Str => "str".to_owned(),
        ApiType::OwnedString => "String".to_owned(),
        ApiType::Int => "int".to_owned(),
        ApiType::Float => "float".to_owned(),
        ApiType::Bool => "bool".to_owned(),
        ApiType::List(inner) => format!("list<{}>", render(inner)),
        ApiType::Map(inner) => format!("dict<{}>", render(inner)),
        ApiType::Optional(inner) => format!("optional<{}>", render(inner)),
        ApiType::Outcome(inner) => format!("result<{}>", render(inner)),
        ApiType::Named(name) => name.clone(),
        ApiType::Unknown(raw) => format!("?{raw}"),
    }
}

#[cfg(test)]
mod mod_test;
#[cfg(test)]
mod rust_test;
#[cfg(test)]
mod ts_test;
