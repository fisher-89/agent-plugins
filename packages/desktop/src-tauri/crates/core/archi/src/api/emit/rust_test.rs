//! `api::emit::rust`（`.i.rs` 渲染）的单元测试：七种协议映射（`object` → `struct`、
//! `enum` → `enum`、`oneOf` / `anyOf` → 带负载 enum、`x-class` → `trait`、标量 →
//! newtype、`x-function` → `pub fn`、`Outcome` → `Result<T, String>`）、非 `required` →
//! `Option<T>`、自动构造器、签名见证形态（`super::` 前缀）、透传 `pub use`、头注释四句、
//! 泛型零支持与幂等；闸二（D45 / AC-23 ②⑤）的注释转义单点 `rust_comment_text`（防断行）
//! 与变体名归一 `variant_name`（恒为合法标识符，经 `render` 的 enum 成员断言）。
//!
//! 零 mock：真实 `contract::parse` + `schema::build_schema` 产出输入；本渲染器纯出
//! `String`（文件系统 / 时钟 / 子进程零依赖）。

use serde_json::Value;

use crate::api::contract::{parse, Contract};

use super::rust::{render, rust_comment_text};
use super::{render_target, EmitTarget, ModuleRef};
use crate::api::paths::GeneratedKind;
use crate::api::schema::build_schema;
use crate::api::{meta, ApiError};

/// 契约文件（repo 相对 POSIX）。
const FILE: &str = ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md";

/// 元素 FQN。
const FQN: &str = "App.Backend.Infra.ArchiRuntime";

/// 解析 + 装配（失败即断言失败）。
fn prepare(source: &str) -> (Contract, Value) {
    let contract = parse(source, FQN, FILE).expect("契约应解析成功");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    (contract, schema)
}

/// 渲染（失败即断言失败）。
///
/// 「空 scope」由**入参**表达（D46）：空查表 + 目录 / crate 形态的 `EmitTarget`——渲染器
/// 只有四参 `render` 这一个出口，MUST NOT 靠第二个函数表达退化形态。
fn empty_target() -> EmitTarget {
    EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules: std::collections::BTreeMap::new(),
    }
}

/// 渲染（失败即断言失败）。
fn render_source(source: &str) -> String {
    let (contract, schema) = prepare(source);
    render(&contract, &schema, &meta(), &empty_target())
}

/// 只含 `api-types` fence 的源文渲染。
fn render_types(types: &str) -> String {
    render_source(&format!("# {FQN}\n\n```json api-types\n{types}\n```\n"))
}

/// 断言渲染返回 `Err(ApiError::Contract(..))` 并返回文案。
fn contract_error(result: Result<String, ApiError>) -> String {
    match result {
        Ok(text) => panic!("期望契约错误，实际 Ok：{text}"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// object / 非 required 字段
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`{"type":"object","properties":{…}}` → `pub struct`，字段名与类型逐项
/// 映射；零字段 → 零字段 struct。
#[test]
fn render的object为struct() {
    let text = render_types(
        r#"{
  "Item": {"type": "object", "properties": {"a": {"type": "string"}, "b": {"type": "integer"}}}
}"#,
    );
    assert!(
        text.contains("pub struct Item {"),
        "object → struct：\n{text}"
    );
    assert!(
        text.contains("pub a: Option<String>"),
        "string 字段 → String：\n{text}"
    );
    assert!(
        text.contains("pub b: Option<i64>"),
        "integer 字段 → i64：\n{text}"
    );

    let zero_field = render_types(r#"{"Unit": {"type": "object", "properties": {}}}"#);
    assert!(
        zero_field.contains("pub struct Unit;"),
        "零字段 object → 单元 struct：\n{zero_field}"
    );
    assert!(
        zero_field.contains("#[derive(Debug, Clone, Copy, PartialEq, Eq)]"),
        "零字段 struct 带 std 派生：\n{zero_field}"
    );
}

/// 正向（AC-5）：`required` 只列一字段 → 未列字段为 `Option<T>`、已列为裸 `T`；
/// `required = []` → 全 `Option<T>`。
#[test]
fn render的非required字段为option() {
    let partial = render_types(
        r#"{
  "Item": {
    "type": "object",
    "properties": {"a": {"type": "string"}, "b": {"type": "integer"}},
    "required": ["a"]
  }
}"#,
    );
    assert!(
        partial.contains("pub a: String,"),
        "required 字段为裸类型：\n{partial}"
    );
    assert!(
        partial.contains("pub b: Option<i64>,"),
        "未列字段为 Option：\n{partial}"
    );

    let none_required = render_types(
        r#"{
  "Item": {
    "type": "object",
    "properties": {"a": {"type": "string"}},
    "required": []
  }
}"#,
    );
    assert!(
        none_required.contains("pub a: Option<String>,"),
        "required = [] → 全 Option：\n{none_required}"
    );
}

// ---------------------------------------------------------------------------
// enum / oneOf / anyOf / x-class
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`{"enum":["a","b"]}` → `pub enum` 两变体（变体名经 `variant_name` 归一）。
///
/// 表驱动两形态（同一映射，入参字符面不同）：
/// ① 标识符干净的取值（`a` / `b`）→ 驼峰形态 `A` / `B`；
/// ② 含非标识符字符的取值（`it's` / `a b`）→ 归一后仍落 `ItS` / `AB`（归一而非转义）。
///
/// `Deserialize` 派生本期零下发（元素 crate 是否有 `serde` 不在本层判定：出参面是纯
/// `String`、结构上无错误面，见待决问题 1「本期不落实现、不写死」）⇒ **不派生、亦不降级
/// 为错误**：同一文本零 `serde` 字样，且 `render` 的 `String` 返回值本身无错误通道。
#[test]
fn render的enum为pub_enum() {
    // ① 标识符干净的取值
    let text = render_types(r#"{"Mode": {"enum": ["a", "b"]}}"#);
    assert!(
        text.contains("pub enum Mode {"),
        "enum → pub enum：\n{text}"
    );
    assert!(text.contains("    A,"), "变体驼峰化：\n{text}");
    assert!(text.contains("    B,"), "两变体均在案：\n{text}");

    // ② 含非标识符字符的取值：仍经 `variant_name` 归一为合法变体名
    let dirty = render_types(r#"{"Mode": {"enum": ["it's", "a b"]}}"#);
    assert!(
        dirty.contains("pub enum Mode {"),
        "非标识符取值仍落 pub enum：\n{dirty}"
    );
    assert!(dirty.contains("    ItS,"), "`it's` → `ItS`：\n{dirty}");
    assert!(dirty.contains("    AB,"), "`a b` → `AB`：\n{dirty}");

    // 不派生、亦不降级为错误（两形态同一判据：零 serde 字样）
    for form in [&text, &dirty] {
        for forbidden in ["Deserialize", "Serialize", "#[derive(serde", "serde::"] {
            assert!(
                !form.contains(forbidden),
                "enum 文本 MUST NOT 携带 {forbidden}（派生面本期留白）：\n{form}"
            );
        }
    }
}

/// 正向（AC-5）：`oneOf` / `anyOf` → 带负载 `pub enum`。
#[test]
fn render的oneof为带负载enum() {
    for key in ["oneOf", "anyOf"] {
        let text = render_types(&format!(
            r##"{{
  "Choice": {{"{key}": [{{"$ref": "#/$defs/A"}}, {{"$ref": "#/$defs/B"}}]}},
  "A": {{"type": "object", "properties": {{}}}},
  "B": {{"type": "object", "properties": {{}}}}
}}"##
        ));
        assert!(
            text.contains("pub enum Choice {"),
            "{key} → 带负载 enum：\n{text}"
        );
        // 变体名 = 驼峰化负载名 + 负载（本实现取 `variant_name(payload) + payload`）
        assert!(text.contains("    AA(A),"), "分支负载类型 A：\n{text}");
        assert!(text.contains("    BB(B),"), "分支负载类型 B：\n{text}");
    }
}

/// 正向（**D59 / AC-34 ②**）：`oneOf` 的三类**新增带负载**分支（协议键 / `array` / 带
/// `additionalProperties` 的 `object`）各产带负载变体——此前落 `Variant{index}`；判据是
/// 「分支负载文本 == 同一 schema 在字段位的文本」（两路径同出 `value_type`，MUST NOT 二份
/// 标量表）。
#[test]
fn render的oneof三类分支均带负载() {
    let cases: [(&str, &str); 3] = [
        ("协议键", r#"{"outcome": {"type": "string"}}"#),
        ("array", r#"{"type": "array", "items": {"type": "string"}}"#),
        (
            "object+additionalProperties",
            r#"{"type": "object", "additionalProperties": {"type": "integer"}}"#,
        ),
    ];
    for (label, choice) in cases {
        // 路径一：分支位（`variants_block` 经 `value_type(choice, Position::Field, …)`）
        let branch = render_types(&format!(r#"{{"Choice": {{"oneOf": [{choice}]}}}}"#));
        assert!(
            branch.contains("pub enum Choice {"),
            "{label}：oneOf 出 enum：
{branch}"
        );
        assert!(
            !branch.contains("    Variant0,"),
            "{label}：MUST NOT 落 `Variant{{index}}`（兜底判据是 `serde_json::Value`）：
{branch}"
        );

        // 路径二：字段位（同一 schema 的常规值映射）
        let field = render_types(&format!(
            r#"{{"Holder": {{"type": "object", "properties": {{"payload": {choice}}}, "required": ["payload"]}}}}"#
        ));
        let payload = field
            .split_once("pub payload: ")
            .expect("字段类型在案")
            .1
            .lines()
            .next()
            .expect("字段行在案")
            .trim_end_matches(',')
            .to_owned();
        assert_ne!(
            payload, "serde_json::Value",
            "{label}：字段位不是兜底文本（本例前提）：
{field}"
        );
        // 负载文本逐字相等：分支出的变体负载即字段位那一段
        assert!(
            branch.contains(&format!("({payload}),")),
            "{label}：分支负载 `({payload})` 与字段位文本逐字相等（两路径同出 `value_type`）：
{branch}"
        );
    }
}

/// 边界（**D59 边界**）：「`object` 无 `additionalProperties`」与「无 `type` 且非
/// `$ref`」两分支仍落 `Variant{index}`（`value_type` 的兜底文本 `serde_json::Value` 即
/// 判定阈值）；`$ref` 分支与四种标量分支逐字不变（D59 的射程精确到此）。
#[test]
fn render的oneof兜底分支仍落占位变体() {
    for (label, choice) in [
        ("object 无 additionalProperties", r#"{"type": "object"}"#),
        ("无 type 且非 $ref", r#"{"description": "无形状"}"#),
    ] {
        let text = render_types(&format!(r#"{{"Choice": {{"oneOf": [{choice}]}}}}"#));
        assert!(
            text.contains("    Variant0,"),
            "{label}：兜底即落 `Variant{{index}}`：
{text}"
        );
        assert!(
            !text.contains("serde_json::Value"),
            "{label}：MUST NOT 出 `(serde_json::Value)` 形态的负载：
{text}"
        );
    }

    // `$ref` 与四种标量分支逐字不变
    let unchanged = render_types(
        r##"{"Choice": {"oneOf": [{"$ref": "#/$defs/A"}, {"type": "string"}, {"type": "integer"}, {"type": "number"}, {"type": "boolean"}]},
  "A": {"type": "object", "properties": {}}}"##,
    );
    for expected in [
        "    AA(A),",
        "    StringString(String),",
        "    I64i64(i64),",
        "    F64f64(f64),",
        "    Boolbool(bool),",
    ] {
        assert!(
            unchanged.contains(expected),
            "既有分支逐字不变：期望 `{expected}`
{unchanged}"
        );
    }
}

/// 异常（**D59 / AC-34 ②** 源码自证）：`fn choice_payload` 零残留；`variants_block`
/// 体内零标量表（该函数体 `"i64"` 零命中）；`rust.rs` 全文 `"i64"` 恰 **2**（剩
/// `value_type` 与 `scalar_type` 两处合法映射——MUST NOT 取 1，那会让 `scalar_type`
/// 被误判为残留）；`contract.rs` 的注释词面已改指 `value_type`。
#[test]
fn choice_payload零残留且单通路() {
    let source = include_str!("rust.rs");
    assert_eq!(
        source.matches("fn choice_payload").count(),
        0,
        "`fn choice_payload` 已整条删除（D59）"
    );
    let variants = function_body(source, "fn variants_block(");
    assert_eq!(
        variants.matches("\"i64\"").count(),
        0,
        "`variants_block` 体内零标量表（MUST NOT 第二份映射）：
{variants}"
    );
    assert!(
        variants.contains("value_type("),
        "分支负载一律经 `value_type`：
{variants}"
    );
    assert_eq!(
        source.matches("\"i64\"").count(),
        2,
        "全文 `\"i64\"` 恰 2（`value_type` + `scalar_type`；删前 3）"
    );
    let contract = include_str!("../contract.rs");
    assert_eq!(
        contract.matches("choice_payload").count(),
        0,
        "`contract.rs` 注释词面已改指 `value_type`（D59 ⑤ c）"
    );
}

/// 正向（AC-5 / D27）：`x-class: true` + `properties` 全为 `x-function` → `pub trait`
/// 且方法签名逐项；零 `properties` 的 `x-class` → 空 trait（合法）。
#[test]
fn render的x_class为trait() {
    let text = render_types(
        r#"{
  "Runner": {
    "x-class": true,
    "properties": {"run": {"x-function": {"params": [{"name": "input", "schema": {"type": "string"}}], "returns": {"type": "boolean"}}}}
  }
}"#,
    );
    assert!(
        text.contains("pub trait Runner {"),
        "x-class → trait：\n{text}"
    );
    assert!(
        text.contains("    fn run(input: &str) -> bool;"),
        "方法签名逐项（入参 &str、返回 bool）：\n{text}"
    );
    assert!(
        !text.contains("pub struct Runner {"),
        "MUST NOT 产 struct + 方法的混装：\n{text}"
    );

    let bare = render_types(r#"{"Marker": {"x-class": true}}"#);
    assert!(
        bare.contains("pub trait Marker {"),
        "零 properties 的 x-class → 空 trait：\n{bare}"
    );
}

// ---------------------------------------------------------------------------
// 标量 / x-function / Outcome
// ---------------------------------------------------------------------------

/// 正向（AC-5 / D9）：标量 → newtype（`integer` → `i64`、`string` → `String`）。
#[test]
fn render的标量为newtype() {
    let integer = render_types(r#"{"TimestampMs": {"type": "integer"}}"#);
    assert!(
        integer.contains("pub struct TimestampMs(pub i64);"),
        "integer → newtype i64：\n{integer}"
    );

    let string = render_types(r#"{"Name": {"type": "string"}}"#);
    assert!(
        string.contains("pub struct Name(pub String);"),
        "string → newtype String：\n{string}"
    );
}

/// 正向（AC-5 / D11）：`$defs` 内的 `x-function` → 位置分派表生效（入参 `string` →
/// `&str`、返回 `string` → `String`）；`async` 键不 panic。
#[test]
fn render的x_function位置分派() {
    let text = render_types(
        r#"{
  "Hook": {
    "type": "object",
    "properties": {"call": {"x-function": {"params": [{"name": "input", "schema": {"type": "string"}}], "returns": {"type": "string"}}}}
  }
}"#,
    );
    assert!(
        text.contains("pub call: fn(input: &str) -> String,"),
        "函数指针字段按位置分派：\n{text}"
    );

    let asynchronous = render_types(
        r#"{
  "Hook": {
    "type": "object",
    "properties": {"call": {"x-function": {"params": [], "returns": {"type": "string"}, "async": true}}}
  }
}"#,
    );
    assert!(
        asynchronous.contains("pub call: fn() -> String,"),
        "Rust 侧不区分 async 帧（形态可判）：\n{asynchronous}"
    );
}

/// 正向（AC-5 / D10 位置分派）：容器类型在两侧逐位对等——`array` 的入参位是 `&[T]`
/// （免 clone 借用）、返回位与字段位是 `Vec<T>`；`object` + `additionalProperties`
/// 是 `BTreeMap<String, V>`（缺 `additionalProperties` → `serde_json::Value`，
/// 不可归类的 `type` 同样退 `serde_json::Value`，零猜测新类型）。
#[test]
fn render的容器类型位置分派() {
    let array = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [{{\"name\": \"items\", \"schema\": \
         {{\"type\": \"array\", \"items\": {{\"type\": \"string\"}}}}}}], \"returns\": {{\"type\": \
         \"array\", \"items\": {{\"type\": \"integer\"}}}}}}\n```\n"
    ));
    assert!(
        array.contains("const _: fn(&[String]) -> Vec<i64> = super::load;"),
        "入参 &[T] / 返回 Vec<T>：\n{array}"
    );

    let map = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [{{\"name\": \"m\", \"schema\": \
         {{\"type\": \"object\", \"additionalProperties\": {{\"type\": \"integer\"}}}}}}], \
         \"returns\": {{\"type\": \"object\"}}}}\n```\n"
    ));
    assert!(
        map.contains("const _: fn(BTreeMap<String, i64>) -> serde_json::Value = super::load;"),
        "map 入参位 → BTreeMap<String, V>、无 additionalProperties → serde_json::Value：\n{map}"
    );

    // 字段位：array → Vec<T>（与返回位同形，非 `&[T]`）
    let field = render_types(
        r#"{"Holder": {"type": "object", "properties": {"items": {"type": "array", "items": {"type": "boolean"}}}, "required": ["items"]}}"#,
    );
    assert!(
        field.contains("pub items: Vec<bool>,"),
        "字段位 array → Vec<T>（MUST NOT 出 `&[T]`）：\n{field}"
    );
}

/// 正向（AC-5）：返回类型 `Outcome(T)` → `Result<T, String>`（std 类型，不生成新类型）。
#[test]
fn render的outcome为result() {
    let text = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"outcome\": \
         {{\"type\": \"string\"}}}}}}\n```\n\n```json api-types\n{{\"Cache\": {{\"type\": \
         \"object\", \"properties\": {{}}}}}}\n```\n"
    ));
    assert!(
        text.contains("const _: fn() -> Result<String, String> = super::load;"),
        "Outcome → Result<T, String> 见证：\n{text}"
    );
    assert!(
        !text.contains("pub struct Outcome"),
        "MUST NOT 生成 Outcome 新类型：\n{text}"
    );
}

// ---------------------------------------------------------------------------
// 自动构造器 / 签名见证 / 透传
// ---------------------------------------------------------------------------

/// 正向（AC-5 / D11）：零字段 `object` → 自动构造器 `impl <Type> { pub fn new() -> Self }`；
/// 带字段 object 的构造器形态可判（不 panic），且契约 MUST NOT 声明构造器。
#[test]
fn render的自动构造器() {
    let zero_field = render_types(r#"{"Unit": {"type": "object", "properties": {}}}"#);
    assert!(
        zero_field.contains("impl Unit {"),
        "零字段类型产构造器：\n{zero_field}"
    );
    assert!(
        zero_field.contains("pub fn new() -> Self {"),
        "构造器形态：\n{zero_field}"
    );

    let with_field =
        render_types(r#"{"Item": {"type": "object", "properties": {"a": {"type": "string"}}}}"#);
    assert!(
        with_field.contains("pub struct Item {"),
        "带字段 object 正常渲染：\n{with_field}"
    );
    assert!(
        !with_field.contains("impl Item {"),
        "带字段 object 不走零字段构造器（契约不声明构造器）：\n{with_field}"
    );
}

/// 正向（事实校准 5 / AC-13）：签名见证用 `super::` 前缀（裸名会报 E0425），且每 op 一行、
/// 每个可构造类型一行。
#[test]
fn render的签名见证形态() {
    let text = render_source(&format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"owner\": \"LocalArchiCheck\", \
         \"params\": [{{\"name\": \"timestamp\", \"schema\": {{\"type\": \"string\"}}}}], \
         \"returns\": {{\"type\": \"string\"}}}}\n```\n\n```json api-types\n\
         {{\"LocalArchiCheck\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    ));
    assert!(
        text.contains("const _: fn(&str) -> String = super::LocalArchiCheck::file_stem;"),
        "方法见证带 super:: 与 owner：\n{text}"
    );
    assert!(
        text.contains("const _: fn() -> super::LocalArchiCheck = super::LocalArchiCheck::new;"),
        "构造器见证带 super::：\n{text}"
    );
    assert!(
        !text.contains("const _: fn(&str) -> String = LocalArchiCheck::file_stem;"),
        "MUST NOT 用裸名见证（E0425）：\n{text}"
    );

    // 自由函数（无 owner）→ `super::<op>`
    let free = render_source(&format!(
        "# {FQN}\n\n### compose_text\n\n```json api-op\n{{\"params\": [], \"returns\": \
         {{\"type\": \"string\"}}}}\n```\n"
    ));
    assert!(
        free.contains("const _: fn() -> String = super::compose_text;"),
        "自由函数见证用 `super::<op>`：\n{free}"
    );
}

/// 正向（AC-11 / D40）：纯 `$ref` alias 两项 → 两行 `pub use`，模块路径取自调用方给的
/// 查表（`EmitTarget.modules[..].module_path`，`crate::*` 形态），零本地 struct 重定义。
#[test]
fn render的透传pub_use() {
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\", \
         \"AgentBlocks\": \"App.Frontend.Components.Agent.AgentBlocks\"}}\n```\n\n## 透传\n\n\
         - AgentInput → App.Frontend.Components.Agent.Input\n\
         - AgentBlocks → App.Frontend.Components.Agent.AgentBlocks\n"
    ));
    let mut modules = std::collections::BTreeMap::new();
    for (fqn, path) in [
        ("App.Frontend.Components.Agent.Input", "crate::input"),
        ("App.Frontend.Components.Agent.AgentBlocks", "crate::blocks"),
    ] {
        modules.insert(
            fqn.to_owned(),
            ModuleRef {
                name: path.rsplit("::").next().unwrap_or(path).to_owned(),
                module_path: path.to_owned(),
            },
        );
    }
    let target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules,
    };
    let text = render_target(&target, &contract, &schema, &meta()).expect("渲染应成功");
    assert!(
        text.contains("pub use crate::input::AgentInput;"),
        "透传模块路径取自查表（`crate::*` 形态）：\n{text}"
    );
    assert!(
        text.contains("pub use crate::blocks::AgentBlocks;"),
        "第二项透传在案：\n{text}"
    );
    assert!(
        !text.contains("App.Frontend.Components.Agent.Input::AgentInput"),
        "MUST NOT 用目标 FQN 当 Rust 模块路径（点分段不是合法路径）：\n{text}"
    );
    assert!(
        !text.contains("pub struct AgentInput"),
        "MUST NOT 本地重定义透传类型：\n{text}"
    );
}

// ---------------------------------------------------------------------------
// 头注释
// ---------------------------------------------------------------------------

/// 正向（AC-2 / D13）：文本前若干行为注释块，含勿手改 / 覆盖 / `api:check` / impl 分离
/// 四句 + 工具版本 + `common.json@sha256:<12 位>`。
#[test]
fn render的头注释四句() {
    let text = render_source(&format!("# {FQN}\n\n## 职能\n\n正文。\n"));
    let header: Vec<&str> = text
        .lines()
        .take_while(|line| line.starts_with("//"))
        .collect();
    assert!(header.len() >= 4, "头注释至少四行：{header:?}");

    let joined = header.join("\n");
    for phrase in ["勿手改", "下次 generate 时被覆盖", "api:check", "impl 块"] {
        assert!(joined.contains(phrase), "头注释含 `{phrase}`：\n{joined}");
    }
    assert!(
        joined.contains(&format!("archi-api {}", env!("CARGO_PKG_VERSION"))),
        "头注释含工具版本：\n{joined}"
    );
    assert!(
        joined.contains("common.json@sha256:"),
        "头注释含映射规则摘要：\n{joined}"
    );
    let digest = meta().common_digest;
    let (algorithm, hex) = digest.split_once(':').expect("摘要含算法前缀");
    assert!(
        joined.contains(&format!("{algorithm}:{}", &hex[..12])),
        "摘要为前 12 位短形：\n{joined}"
    );
}

// ---------------------------------------------------------------------------
// 异常：泛型 / 幂等 / 规模 / 空契约
// ---------------------------------------------------------------------------

/// 异常（AC-5 反例）：本渲染器只处理已装配的 `$defs`，泛型的拦截在
/// `emit::render_target` 的 `reject_generics`（本函数的入参不含泛型——以「带泛型键的
/// `$defs` 仍逐字落类型名」表达「本层不做猜测/改写」）。
#[test]
fn render不猜泛型形态() {
    let text = render_types(
        r##"{"Wrapper": {"type": "object", "properties": {"inner": {"$ref": "#/$defs/T"}}}}"##,
    );
    assert!(
        text.contains("pub struct Wrapper {"),
        "泛型实参的 $ref 按名字落位（本层零猜测）：\n{text}"
    );
    assert!(
        text.contains("pub inner: Option<T>,"),
        "`$ref` 名字原样进字段类型：\n{text}"
    );
    assert!(
        !text.contains("<t>") && !text.contains("Array<"),
        "MUST NOT 自创泛型语法：\n{text}"
    );
}

/// 边界：同输入两次渲染逐字相等；类型段按 `$defs` 声明序（不排序、不合并）。
#[test]
fn render幂等与键序() {
    let source = format!(
        "# {FQN}\n\n```json api-types\n{{\"Zeta\": {{\"type\": \"object\", \"properties\": {{}}}}, \
         \"Alpha\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    );
    let first = render_source(&source);
    let second = render_source(&source);
    assert_eq!(first, second, "两次渲染逐字相等");

    let zeta = first.find("pub struct Zeta").expect("Zeta 在案");
    let alpha = first.find("pub struct Alpha").expect("Alpha 在案");
    assert!(zeta < alpha, "类型段按声明序（Zeta 先于 Alpha）：\n{first}");
}

/// 边界（AC-12 / D29）：输出为格式化器语义的不动点形态——零尾随空白、文件末尾恰一个
/// 换行、无 tab 缩进；渲染路径零 `std::process`。
#[test]
fn render形态为格式化器不动点() {
    let text = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"type\": \
         \"string\"}}}}\n```\n\n```json api-types\n{{\"Item\": {{\"type\": \"object\", \
         \"properties\": {{\"a\": {{\"type\": \"string\"}}}}}}}}\n```\n"
    ));
    for line in text.lines() {
        assert_eq!(line, line.trim_end(), "零尾随空白：{line:?}");
        assert!(!line.contains('\t'), "零 tab 缩进：{line:?}");
    }
    assert!(text.ends_with('\n'), "末尾有换行");
    assert!(!text.ends_with("\n\n"), "末尾恰一个换行");
    assert!(
        !include_str!("rust.rs").contains("std::process"),
        "渲染路径零子进程（格式化器不 spawn）"
    );
}

/// 边界（AC-17）：`.i.rs` 是语言特定产物，但不承载跨语言词汇——文本零 `Cargo.toml`
/// 字样、零 `crate` 定义段。
#[test]
fn render语言词边界() {
    let text = render_source(&format!("# {FQN}\n\n## 职能\n\n正文。\n"));
    assert!(
        !text.contains("Cargo.toml"),
        "MUST NOT 出现 Cargo.toml 字样：\n{text}"
    );
    assert!(
        !text.contains("\n crate "),
        "MUST NOT 出现 crate 定义段：\n{text}"
    );
}

/// 边界：零类型零 op → 仅头注释（不含 `impl` / `pub use` / 见证行）。
#[test]
fn render空契约只余头注释() {
    let text = render_source(&format!("# {FQN}\n\n## 职能\n\n正文。\n"));
    assert!(
        !text.lines().any(|line| line.starts_with("impl ")),
        "零内容不出 impl：\n{text}"
    );
    assert!(!text.contains("pub use"), "零内容不出 pub use：\n{text}");
    assert!(
        !text.contains("const _: fn("),
        "零内容不出签名见证：\n{text}"
    );
    assert!(
        text.lines()
            .all(|line| line.is_empty() || line.starts_with("//")),
        "只余头注释：\n{text}"
    );
}

/// 边界：1000 类型 + 200 op → 不 panic、条数逐项相等；类型名含 emoji / 超长
/// `description`（>1000 chars）不破坏语法形态（不含裸 `\0`）。
#[test]
fn render规模与特殊字符() {
    let long = "描".repeat(1200);
    let mut types = String::from("{");
    for index in 0..1000 {
        if index > 0 {
            types.push(',');
        }
        types.push_str(&format!(
            "\"T{index}\": {{\"type\": \"object\", \"properties\": {{}}}}"
        ));
    }
    types.push_str(&format!(
        ",\"长🎯\": {{\"type\": \"object\", \"properties\": {{}}, \"description\": \"{long}\"}}}}"
    ));

    let mut source = format!("# {FQN}\n\n## 操作\n\n");
    for index in 0..200 {
        source.push_str(&format!(
            "### op{index}\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"type\": \"string\"}}}}\n```\n\n"
        ));
    }
    source.push_str(&format!("```json api-types\n{types}\n```\n"));

    let text = render_source(&source);
    assert_eq!(
        text.matches("pub struct T").count(),
        1000,
        "1000 类型逐条渲染"
    );
    assert_eq!(
        text.matches("const _: fn() -> String").count(),
        200,
        "200 op 各一行见证"
    );
    assert!(
        text.contains("pub struct 长🎯"),
        "非 ASCII 类型名原样：\n{text}"
    );
    assert!(!text.contains('\0'), "MUST NOT 含裸 NUL");
    assert!(
        !text.contains(&long),
        "超长 description 不进类型段（不破坏语法）：\n{}",
        &text[..200.min(text.len())]
    );
}

/// 异常（AC-5 / 待决问题 1 的裁定面）：`enum` 的 `Deserialize` 派生按元素 crate 现有
/// 依赖决定，而 `render` 的签名是纯出 `String`、**不收元素依赖面**——设计 `:491` 描述的
/// 「元素 crate 已有 `serde` ⇒ 返回 `ApiError::Element`」这一分支在实现里不存在（本渲染器
/// 零 `ApiError` 构造，见 `render签名不含错误面`）。本用例据实现事实钉住两侧：
///
/// ① **不降级为错误**：`enum` 形 `$defs` 经 `render_target` 分派后仍 `Ok`（`Err` 只会
///    来自上游 `reject_generics` / `verify_defs`），且文本落 `pub enum` 与两变体；
/// ② **不静默下发 serde**：同一文本零 `Deserialize` / `Serialize` 字样（元素 crate 可能
///    无 serde 依赖，硬写派生会直接编译失败），派生面留给首个真实 `enum` 元素契约落地时
///    按该元素 `Cargo.toml` 定（待决问题 1）。
#[test]
fn render的enum不落serde派生且不降级为错误() {
    let source =
        format!("# {FQN}\n\n```json api-types\n{{\"Mode\": {{\"enum\": [\"a\", \"b\"]}}}}\n```\n");
    let (contract, schema) = prepare(&source);

    // ① 经语言分派处（`render_target`）走完整路径：`Ok` 而非 `Err(ApiError::Element)`
    let target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules: std::collections::BTreeMap::new(),
    };
    let text = render_target(&target, &contract, &schema, &meta())
        .expect("enum 形态不降级为 Element 错误（本期不按元素依赖面判错）");
    assert!(
        text.contains("pub enum Mode {"),
        "enum 仍按形态渲染：\n{text}"
    );
    assert!(text.contains("    A,"), "变体 A 在案：\n{text}");
    assert!(text.contains("    B,"), "变体 B 在案：\n{text}");

    // ② 零 serde 派生（派生面按待决问题 1 留白；硬写会与零 serde 依赖的元素 crate 冲突）
    for forbidden in ["Deserialize", "Serialize", "#[derive(serde", "serde::"] {
        assert!(
            !text.contains(forbidden),
            "enum 文本 MUST NOT 携带 {forbidden}（派生面本期留白）：\n{text}"
        );
    }
    // 本渲染器与分派处都不构造 `Element` 错误：源码自证（`Err` 只在 `reject_generics` /
    // `verify_defs` 一侧产生，两处均为 `Contract` 变体）
    assert!(
        !include_str!("rust.rs").contains("ApiError::Element"),
        "`emit/rust.rs` 零 `ApiError::Element` 构造（本层不产出元素面错误）"
    );
    assert!(
        !include_str!("../emit/mod.rs").contains("ApiError::Element"),
        "语言分派处零 `ApiError::Element` 构造（同层不产出元素面错误）"
    );

    // 源码自证：`emit/rust.rs` 内零 `serde::`（不引用任何 serde 派生 / 序列化 API）；
    // `serde_json` 的 `use` 只取 `Map` / `Value` 两个类型（零 `to_string` 一类的产出面）
    let rust_source = include_str!("rust.rs");
    assert!(
        !rust_source.contains("serde::"),
        "`emit/rust.rs` 零 `serde::` 引用（派生面本期留白；`serde_json::` 不算 `serde::`）"
    );
    assert!(
        rust_source.contains("use serde_json::{Map, Value};"),
        "`serde_json` 的 `use` 只取 `Map` / `Value` 两个类型：\n{rust_source}"
    );
}

/// 异常面自证：`render` 不返回 `Err`（泛型拦截在 `render_target`）；本文件零 `Err(` 构造。
#[test]
fn render签名不含错误面() {
    let source = include_str!("rust.rs");
    assert!(
        source.contains(
            "pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String"
        ),
        "渲染器签名为四参单形、纯出 String（D46）"
    );
    assert!(
        !source.contains("ApiError::Contract"),
        "本渲染器不构造契约错误（拦截在上游）"
    );
    let _ = contract_error; // 保持辅助函数被引用（供等价断言复用）
}

/// 异常（D40 反例在案）：来源文件内零 `rsplit('.')`、零 `replace('.', "::")`；`pub use`
/// 的模块段只取自查表值 `ModuleRef.module_path`（`crate::*` 形态），与
/// `reexport_target(..)` 的 FQN 形态**不相等**——文本取后者以外的那一个。
#[test]
fn render的透传模块路径零fqn派生() {
    let source = include_str!("rust.rs");
    assert!(
        !source.contains("rsplit('.')"),
        "`emit/rust.rs` 零 FQN 点分段切分"
    );
    assert!(
        !source.contains("replace('.', \"::\")"),
        "零「点 → ::」替换（第二套命名即禁）"
    );
    assert_eq!(
        source.matches("module.module_path").count(),
        1,
        "模块段只取自 `ModuleRef.module_path`（单点）"
    );

    // 两形态不相等：`reexport_target` 给 FQN 形态，`ModuleRef.module_path` 给 `crate::*`
    let reference = "./App.Frontend.Components.Agent.schema.json#/$defs/Input";
    let fqn = crate::api::reexport_target(reference).expect("外部文件引用取目标 FQN");
    assert_eq!(fqn, "App.Frontend.Components.Agent");
    let module_path = format!(
        "crate::{}",
        crate::api::paths::module_name("views/agent/Input")
    );
    assert_eq!(module_path, "crate::Input");
    assert_ne!(
        fqn, module_path,
        "目标 FQN 的点分段不是 Rust 路径（两形态 MUST NOT 相等）"
    );
}

/// 边界（D40 边界 / D46）：透传条目有而 `modules` 无该键 → 只出 `// 透传自 <FQN>`
/// 注释、**不产 `pub use`**（不退化到 FQN 末段）；`modules` 为空 + 零透传条目 → 输出
/// 与非空时逐字相等（「空 scope」由入参表达，不由函数数量表达）。
#[test]
fn render的modules查不到目标只出注释() {
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    ));
    let target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules: std::collections::BTreeMap::new(),
    };
    let text = render_target(&target, &contract, &schema, &meta()).expect("查不到目标不是错误");
    assert!(
        text.contains("// 透传自 App.Frontend.Components.Agent.Input"),
        "只留注释：\n{text}"
    );
    assert!(!text.contains("pub use "), "不产 `pub use`：\n{text}");
    assert!(
        !text.contains("Agent.Input::AgentInput"),
        "MUST NOT 退化到 FQN 末段：\n{text}"
    );

    let (plain_contract, plain_schema) = prepare(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    ));
    let by_empty = render_target(&target, &plain_contract, &plain_schema, &meta())
        .expect("零透传条目渲染成功");
    assert_eq!(
        by_empty,
        render(&plain_contract, &plain_schema, &meta(), &target),
        "无透传条目时查表不参与（两路逐字相等）"
    );
}

// ---------------------------------------------------------------------------
// 闸二：注释转义单点与变体名归一（D45 / AC-23 ②⑤）
// ---------------------------------------------------------------------------

/// 取源码内顶层函数体（自 `signature` 起的花括号配对区段；未命中即断言失败）。
fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("源码内应有 `{signature}`"));
    let open = start + source[start..].find('{').expect("函数体起手花括号");
    let mut depth = 0usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open..open + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("`{signature}` 的函数体未闭合");
}

/// `pub enum (<name>) { … }` 的成员名清单（声明序）。
fn enum_members(text: &str, name: &str) -> Vec<String> {
    let opening = format!("pub enum {name} {{");
    let start = text
        .find(&opening)
        .unwrap_or_else(|| panic!("文本应含 `{opening}`"));
    let body = &text[start + opening.len()..];
    let end = body.find('}').expect("enum 体应有闭合花括号");
    body[..end]
        .lines()
        .map(|line| line.trim().trim_end_matches(',').to_owned())
        .filter(|member| !member.is_empty())
        .collect()
}

/// ASCII 标识符判据（`[A-Za-z_][A-Za-z0-9_]*`，与 `contract::is_identifier` 同形——
/// 测试侧独立一份，产品面仍只有闸一那一处）。
fn is_rust_identifier(token: &str) -> bool {
    let mut characters = token.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// 正向（AC-23 ② / D45 闸二）：`rust_comment_text` 的断行防护——`\n` / `\r` / `U+2028` /
/// `U+2029` 一概落为空格（防断行）；引号与斜杠在注释内无需转义（原样）；空串不 panic。
#[test]
fn rust_comment_text断行防护() {
    assert_eq!(
        rust_comment_text("a\nb"),
        "a b",
        "换行 → 空格（后半段不顶成代码行）"
    );
    assert_eq!(rust_comment_text("a\rb"), "a b", "回车 → 空格");
    assert_eq!(
        rust_comment_text("a\u{2028}b"),
        "a b",
        "U+2028 行分隔符 → 空格"
    );
    assert_eq!(
        rust_comment_text("a\u{2029}b"),
        "a b",
        "U+2029 段分隔符 → 空格"
    );
    assert_eq!(
        rust_comment_text("a\"b"),
        "a\"b",
        "注释内 `\"` 无需转义（原样）"
    );
    assert_eq!(
        rust_comment_text("a//b"),
        "a//b",
        "注释内 `//` 无需转义（原样）"
    );
    assert_eq!(rust_comment_text(""), "", "空串不 panic");

    // 七形态之外的四字叠加：逐位替换，长度不变
    let mixed = rust_comment_text("x\ny\rz\u{2028}w\u{2029}v");
    assert_eq!(mixed, "x y z w v", "四类断行字符逐位落为空格");
}

/// 边界（D45）：`//` 与 `///` 两处注释拼接都经 `rust_comment_text`（零裸 `// {` 插值）；
/// FQN 含 `\n` 时透传注释不断行——后半段留在同一注释行，不变成语句。
#[test]
fn rust_comment_text的消费面() {
    let source = include_str!("rust.rs");
    let calls = source.matches("rust_comment_text(").count();
    assert!(
        calls >= 2,
        "注释拼接均以 `rust_comment_text(` 包裹（定义 + 消费点）；实际 {calls} 处"
    );
    assert!(
        !source.contains("// {"),
        "零裸 `// {{…}}` 插值进注释（契约文本一律先过 rust_comment_text）"
    );

    // 行为面：透传目标 FQN 含换行 → `// 透传自 …` 仍是一行注释
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend\\nComponents.Agent\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent\n"
    ));
    let target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules: std::collections::BTreeMap::new(),
    };
    let text = render_target(&target, &contract, &schema, &meta()).expect("换行不影响渲染成功");
    let comments: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("// 透传自 "))
        .collect();
    assert_eq!(comments.len(), 1, "透传注释恰一行：\n{text}");
    assert_eq!(
        comments[0], "// 透传自 App.Frontend Components.Agent",
        "换行落为空格（后半段不变成代码）"
    );
    assert!(
        !text
            .lines()
            .any(|line| line.starts_with("Components.Agent")),
        "后半段 MUST NOT 独占一行成为语句：\n{text}"
    );
}

/// 边界（AC-23 ⑤ / D45）：`variant_name` 的输出恒为合法 Rust 标识符——`it's` → `ItS`
/// （非标识符字符即分隔符后转驼峰）、`a b` → `AB`、首字符数字 → 加 `V` 前缀（`V1abc`）、
/// 空串 → `Unknown`、干净取值只驼峰化；且 `variant_name` 在产品面恰 1 处定义（归一单点，
/// TS 侧零同名函数）。
#[test]
fn render的enum变体名恒为合法标识符() {
    let text = render_types(r#"{"Mode": {"enum": ["it's", "a b", "1abc", "", "ready"]}}"#);
    assert!(
        text.contains("pub enum Mode {"),
        "enum → pub enum：\n{text}"
    );

    let members = enum_members(&text, "Mode");
    assert_eq!(
        members,
        vec!["ItS", "AB", "V1abc", "Unknown", "Ready"],
        "五形态各自的归一结果（声明序；干净取值只驼峰化）：\n{text}"
    );
    for member in &members {
        assert!(
            is_rust_identifier(member),
            "变体名 MUST 匹配 `[A-Za-z_][A-Za-z0-9_]*`：{member:?}"
        );
        assert!(!member.contains('\''), "变体名零 `'`：{member:?}");
        assert!(!member.contains(' '), "变体名零空格：{member:?}");
        assert!(
            !member
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit()),
            "变体名首字符 MUST NOT 是数字：{member:?}"
        );
    }

    // 源码自证：归一单点恰 1 处，且只在本文件（TS 侧无变体名归一）
    let source = include_str!("rust.rs");
    assert_eq!(
        source.matches("fn variant_name").count(),
        1,
        "`variant_name` 恰 1 处定义"
    );
    for (path, other) in [
        ("emit/mod.rs", include_str!("mod.rs")),
        ("emit/ts.rs", include_str!("ts.rs")),
    ] {
        assert!(
            !other.contains("fn variant_name"),
            "`{path}` 零 `variant_name` 定义（归一单点在 `emit/rust.rs`）"
        );
    }
}

/// 异常（D45）：注释与标识符两族闸分工——`is_identifier` 零命中（闸一在
/// `contract::verify`、渲染期不重复校验）；`variant_name` 与 `rust_comment_text` 互不调用
/// （归一是无损改名、转义是断行防护，语义不混）；注释拼接输出的四个替换字符全不在案。
#[test]
fn 注释与标识符两族闸的分工() {
    let source = include_str!("rust.rs");
    assert!(
        !source.contains("is_identifier"),
        "本文件零 `is_identifier`（闸一在 `contract::verify`，渲染期不重复校验）"
    );

    let comment_body = function_body(source, "pub fn rust_comment_text");
    let variant_body = function_body(source, "fn variant_name");
    assert!(
        !comment_body.contains("variant_name"),
        "`rust_comment_text` 不做名字归一（语义不混）：\n{comment_body}"
    );
    assert!(
        !variant_body.contains("rust_comment_text"),
        "`variant_name` 不做注释转义（归一而非转义）：\n{variant_body}"
    );

    // 反向断言：断行字符不出现在转义输出内
    let escaped = rust_comment_text("a\nb\rc\u{2028}d\u{2029}e");
    for banned in ['\n', '\r', '\u{2028}', '\u{2029}'] {
        assert!(
            !escaped.contains(banned),
            "输出内零断行字符 {banned:?}：{escaped:?}"
        );
    }
    assert_eq!(escaped, "a b c d e", "四类断行字符逐位落为空格");
}

// ---------------------------------------------------------------------------
// 出口单形（D46 / AC-24）：四参带签名恰一、零异名孪生、render_scope 私有
// ---------------------------------------------------------------------------

/// 正向（AC-24 ① / D46）：`emit/rust.rs` 内**带完整四参签名**的 `pub fn render` **恰 1**
/// 命中（`target` **末位**、返回 `String` 四事实同钉；MUST NOT 只数 `^pub fn render\(`）；
/// 行为面：同一契约 + 同一 `EmitTarget` 经 `render(..)` 与经 `emit::render_target(..)`
/// 分派产出逐字相等。
#[test]
fn render出口单形四参签名恰一() {
    let source = include_str!("rust.rs");
    const SIGNATURE: &str =
        "pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String";
    assert_eq!(
        source.matches(SIGNATURE).count(),
        1,
        "四参单形（`target` 末位、返回 `String`）恰 1 处（D46）"
    );

    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Item\": {{\"type\": \"object\", \
         \"properties\": {{\"a\": {{\"type\": \"string\"}}}}}}}}\n```\n"
    ));
    let target = empty_target();
    let direct = render(&contract, &schema, &meta(), &target);
    let dispatched = render_target(&target, &contract, &schema, &meta()).expect("分派应成功");
    assert_eq!(direct, dispatched, "分派零额外语义（两路逐字相等）");
}

/// 异常（AC-24 ② / D46）：逐产品文件枚举（`rust.rs` / `ts.rs` / `mod.rs`）——
/// `fn render_with_target` / `fn render_with_root` **各 0 命中**；「空 scope」由
/// `EmitTarget { modules: BTreeMap::new(), .. }` 表达，MUST NOT 靠第二个函数表达。
#[test]
fn render零异名孪生() {
    for (path, source) in [
        ("emit/rust.rs", include_str!("rust.rs")),
        ("emit/ts.rs", include_str!("ts.rs")),
        ("emit/mod.rs", include_str!("mod.rs")),
    ] {
        for twin in ["fn render_with_target", "fn render_with_root"] {
            assert_eq!(
                source.matches(twin).count(),
                0,
                "`{path}` 内零 `{twin}`（两个异名孪生一并删除，D46）"
            );
        }
    }

    // 行为面：空 `modules` + `Crate` 形态的 `EmitTarget` 与「有查表但零透传条目」
    // 产出逐字相等（退化关系由入参表达，不由函数数量表达）
    let (plain_contract, plain_schema) = prepare(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    ));
    let by_empty = render(&plain_contract, &plain_schema, &meta(), &empty_target());
    let mut filled = empty_target();
    filled.modules.insert(
        "App.Core.A".to_owned(),
        ModuleRef {
            name: "a".to_owned(),
            module_path: "crate::a".to_owned(),
        },
    );
    assert_eq!(
        by_empty,
        render(&plain_contract, &plain_schema, &meta(), &filled),
        "零透传条目时查表不参与（空 scope 由入参表达）"
    );
}

/// 边界（AC-24 ③ / D46）：`^fn render_scope` 在 `emit/rust.rs` **恰 1** 命中、
/// `pub fn render_scope` **零命中**；唯一消费方是同文件的 `render`。
#[test]
fn render_scope保持私有() {
    let source = include_str!("rust.rs");
    assert_eq!(
        source.matches("\nfn render_scope(").count(),
        1,
        "`fn render_scope` 恰 1 处定义"
    );
    assert_eq!(
        source.matches("pub fn render_scope").count(),
        0,
        "MUST NOT 为可测性升格 `render_scope` 的可见性"
    );
    let render_body = function_body(source, "pub fn render(");
    assert_eq!(
        render_body.matches("render_scope(").count(),
        1,
        "`render` 体内恰一处 `render_scope(` 调用（唯一消费方）"
    );
    assert_eq!(
        source.matches("render_scope(").count(),
        2,
        "全文件 `render_scope(` 共 2 处：定义行 + `render` 体内唯一调用"
    );
}

/// 边界（AC-24 ④ / D46）：产品 `.rs`（滤 `*_test.rs`）内 `rust::render(` 的直连面**恰为
/// 分派器一处**——`MUST NOT 用「零命中」作判据`（分派器本身必然调该函数，零命中只会在
/// 「没有任何调用者」时成立，是假判据）；判据是「恰 1 命中 + 命中文件即 `api/emit/mod.rs`」。
/// 本文件（测试）以四参形态直接驱动 `render` 属测试相位的合法姿态。
#[test]
fn 产品侧零直连rust渲染器() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut src_tauri = manifest.clone();
    for _ in 0..3 {
        src_tauri.pop();
    }
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&src_tauri.join("crates"), &mut hits);
    scan_product_rs(&src_tauri.join("src"), &mut hits);
    let direct: Vec<&(String, String)> = hits
        .iter()
        .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains("rust::render("))
        .collect();
    assert_eq!(
        direct.len(),
        1,
        "产品面 `rust::render(` 恰 1 处直连（分派器该分支）：{direct:?}"
    );
    assert!(
        direct[0].0.ends_with("api/emit/mod.rs"),
        "唯一调用点在分派器（实际：{}）",
        direct[0].0
    );
}

/// 递归收集产品 `.rs`（排除 `*_test.rs` 与构建产物目录）的（路径，行）二元组。
fn scan_product_rs(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .map(|name| name == "target" || name == "node_modules")
                .unwrap_or(false);
            if !skip {
                scan_product_rs(&path, out);
            }
            continue;
        }
        let Some(name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if !name.ends_with(".rs") || name.ends_with("_test.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let display = path.to_string_lossy().replace('\\', "/");
        for line in text.lines() {
            out.push((display.clone(), line.to_owned()));
        }
    }
}

// ---------------------------------------------------------------------------
// value_type 零 x-function 分支（AC-25 ② / D47）
// ---------------------------------------------------------------------------

/// 边界（**AC-25 ②** / D47）：**逐函数体**判（**MUST NOT 用文件级 `grep -c`**——`rust.rs`
/// 的合法命中点恒 **3**（`struct_block` 的 `is_some` + `unwrap_or`、`trait_block` 一处），
/// 任何「恰 N / 相等」的文件级断言都无区分能力）：`value_type` 体内 `"x-function"` 字面量
/// **恰 0**；`struct_block` / `trait_block` 两处 `x-function` 读取点**仍在**（`properties`
/// 值的方法分流）。行为面：`{"type":"array","items":{"x-function":…}}` 形态的渲染需求在
/// 装载期已被 D47 中止（本层结构上收不到，故**不给**兜底形态——那正是「同一 schema
/// 两侧产物形态不同」的静默源）。
#[test]
fn value_type零x_function分支() {
    let source = include_str!("rust.rs");
    let value_type_body = function_body(source, "fn value_type(");
    assert_eq!(
        value_type_body.matches("\"x-function\"").count(),
        0,
        "`value_type` 体内零 `\"x-function\"` 分支（D47 后不可达，写成不变量）"
    );
    // 两处合法读取点仍在：`properties` 值的方法 / 函数指针字段分流
    let struct_body = function_body(source, "fn struct_block(");
    assert!(
        struct_body.contains("\"x-function\""),
        "`struct_block` 的 `x-function` 读取点在案（函数指针字段分流）"
    );
    let trait_body = function_body(source, "fn trait_block(");
    assert!(
        trait_body.contains("\"x-function\""),
        "`trait_block` 的 `x-function` 读取点在案（x-class 方法分流）"
    );

    // 行为面：字段位 `items` 内的 `x-function` 不再产出函数指针形态（本层无该分支 ⇒
    // 落到 `array` 的 items 常规映射 `Vec<serde_json::Value>`，而非 `Vec<fn(...)>`）
    let text = render_types(
        r#"{"Holder": {"type": "object", "properties": {"boxed": {"type": "array", "items": {"x-function": {"params": [], "returns": {"type": "string"}}}}}}}"#,
    );
    assert!(
        !text.contains("fn("),
        "深层 `x-function` 不给兜底形态（装载期即中止）：\n{text}"
    );
    assert!(
        text.contains("Vec<serde_json::Value>"),
        "`items` 走常规值映射：\n{text}"
    );
}

// ---------------------------------------------------------------------------
// 映射规则查表（D54 ③ / AC-32）：产物文本由资产驱动，非渲染器硬编码
// ---------------------------------------------------------------------------

/// 查表命中：`common_type` 从 `ApiMeta.common` 取本语言槽位并替换 `{}`（D54 ③）。
///
/// 判据力：改用**自造规则表**（与真实资产文本不同）后产物随之变化 ⇒ 证明产物来自表、
/// 不是渲染器里的硬编码串。
#[test]
fn common_type按表展开且随表变化() {
    use std::collections::BTreeMap;

    use crate::api::CommonRule;

    let source = format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"outcome\": \
         {{\"type\": \"string\"}}}}}}\n```\n"
    );
    let (contract, schema) = prepare(&source);

    let mut custom: BTreeMap<String, CommonRule> = BTreeMap::new();
    custom.insert(
        "Outcome".to_owned(),
        CommonRule {
            rust: "Mapped<{}>".to_owned(),
            typescript: "Mapped<{}>".to_owned(),
            degraded: None,
            note: None,
        },
    );
    let mut meta = meta();
    meta.common = custom;
    let text = render(&contract, &schema, &meta, &empty_target());
    assert!(
        text.contains("Mapped<String>"),
        "产物经表展开（改表即变产物）：\n{text}"
    );
    assert!(
        !text.contains("Result<String, String>"),
        "资产被替换后 MUST NOT 残留原文本形：\n{text}"
    );
}

/// 查表未命中：空表 ⇒ 交回普通 `type` 分派（不 panic、不产出空洞包装）。
#[test]
fn common_type空表回落普通分派() {
    use std::collections::BTreeMap;

    let source = format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"outcome\": \
         {{\"type\": \"string\"}}}}}}\n```\n"
    );
    let (contract, schema) = prepare(&source);
    let mut meta = meta();
    meta.common = BTreeMap::new();
    let text = render(&contract, &schema, &meta, &empty_target());
    assert!(
        text.contains("const _: fn() -> serde_json::Value = super::load;"),
        "空表 ⇒ 回落普通 object 分派（不 panic）：\n{text}"
    );
}

/// 渲染器零硬编码：产品面不得再出现资产里的文本形（D54 ③ 的单点要求）。
#[test]
fn 渲染器产品面零硬编码映射串() {
    let source = include_str!("rust.rs");
    // 只判产品面：doc 已按纪律改述，故整文件零命中即达标。
    assert_eq!(
        source.matches("Result<{}, String>").count(),
        0,
        "映射文本形只许在 `common.json`，渲染器零字面量"
    );
    assert!(source.contains("fn common_type"), "查表单点在案（D54 ③）");
}
