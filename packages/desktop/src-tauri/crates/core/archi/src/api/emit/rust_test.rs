//! `api::emit::rust`（`.i.rs` 渲染）的单元测试：七种协议映射（`object` → `struct`、
//! `oneOf` / `anyOf` 全 `const` → 封闭 `enum`、其余 union → 带负载 enum、`x-class` → `trait`、标量 →
//! newtype、`x-function` → `pub fn`、`Outcome` → `Result<T, String>`）、非 `required` →
//! `Option<T>`、自动构造器、签名见证形态（`super::` 前缀）、透传 `pub use`、头注释四句、
//! 泛型零支持与幂等；闸二（D45 / AC-23 ②⑤）的注释转义单点 `rust_comment_text`（防断行）
//! 与变体名归一 `variant_name`（恒为合法标识符，经 `render` 的 enum 成员断言）。
//!
//! 本变更的核心闸（AC-2 golden）：`ArchiAssetSet` / `ArchiInterfaceAsset` / `ElementKind` /
//! `GateTier` 四类型的**类型定义段**与 `dto.rs` 手写段落逐字比对（比对面取
//! `include_str!("../../dto.rs")`，不另抄期望文本）；另有整数宽度「只认三条完整值域」、
//! serde 命名策略（容器 `rename_all` / 关键字 `r#` / 逐项 `rename`）、文档注释三处位置、
//! wire 面射程与 facet 五形态 / 环容忍。
//!
//! 零 mock：手写接口定义 JSON 经 `Contract::from_schema` 投影产出输入；本渲染器纯出
//! `String`（文件系统 / 时钟 / 子进程零依赖）。

use std::collections::BTreeMap;

use serde_json::Value;

use crate::api::contract::Contract;

use crate::api::mod_test::{empty_schema, mirrored_schema_text, op_schema, prepare, reexport_def};

use super::rust::{render, rust_comment_text};
use super::{render_target, EmitTarget, ModuleRef};
use crate::api::paths::GeneratedKind;
use crate::api::{meta, ApiError};

/// 契约文件（repo 相对 POSIX）。
const FILE: &str = ".dev-team/architecture/models/Backend/Infra/App.Backend.Infra.ArchiRuntime.md";

/// 元素 FQN。
const FQN: &str = "App.Backend.Infra.ArchiRuntime";

/// 解析 + 投影（共用 `emit/mod_test` 的夹具投影，失败即断言失败）。
fn prepare_local(source: &str) -> (Contract, Value) {
    prepare(source, FQN, FILE)
}

/// 渲染（失败即断言失败）。
///
/// 「空 scope」由**入参**表达（D46）：空查表 + 目录 / crate 形态的 `EmitTarget`——渲染器
/// 只有五参 `render` 这一个出口，MUST NOT 靠第二个函数表达退化形态。
fn empty_target() -> EmitTarget {
    EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules: std::collections::BTreeMap::new(),
    }
}

/// 空解析表：本文件全部用例都在单元素内渲染（零跨元素 `$ref` 的 facet 递归）。
fn no_schemas() -> BTreeMap<String, Value> {
    BTreeMap::new()
}

/// 渲染（失败即断言失败）。
fn render_source(source: &str) -> String {
    let (contract, schema) = prepare_local(source);
    render(&contract, &schema, &meta(), &empty_target(), &no_schemas())
}

/// 只含 `api-types` fence 的源文渲染。
fn render_types(types: &str) -> String {
    render_source(&mirrored_schema_text(FQN, types, ""))
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

/// 正向（AC-3 / AC-5）：`oneOf` 全标量 `const` → 封闭 `pub enum`（unit variant，变体名经
/// `variant_name` 归一；`enum` 数组写法已退役）。
///
/// 表驱动两形态（同一映射，入参字符面不同）：
/// ① 标识符干净的取值（`a` / `b`）→ 驼峰形态 `A` / `B`；
/// ② 含非标识符字符的取值（`it's` / `a b`）→ 归一后仍落 `ItS` / `AB`（归一而非转义）。
///
/// 封闭枚举是**有数据位**的形态（词表值就是线形态）⇒ 三项 wire 派生同进同出。
#[test]
fn render的封闭枚举为pub_enum() {
    // ① 标识符干净的取值
    let text = render_types(r#"{"Mode": {"oneOf": [{"const": "a"}, {"const": "b"}]}}"#);
    assert!(
        text.contains("pub enum Mode {"),
        "enum → pub enum：\n{text}"
    );
    assert!(text.contains("    A,"), "变体驼峰化：\n{text}");
    assert!(text.contains("    B,"), "两变体均在案：\n{text}");

    // ② 含非标识符字符的取值：仍经 `variant_name` 归一为合法变体名
    let dirty = render_types(r#"{"Mode": {"oneOf": [{"const": "it's"}, {"const": "a b"}]}}"#);
    assert!(
        dirty.contains("pub enum Mode {"),
        "非标识符取值仍落 pub enum：\n{dirty}"
    );
    assert!(dirty.contains("    ItS,"), "`it's` → `ItS`：\n{dirty}");
    assert!(dirty.contains("    AB,"), "`a b` → `AB`：\n{dirty}");

    // 封闭枚举出 wire 面（三项同进同出）：词表值就是线形态，序列化有落点
    for form in [&text, &dirty] {
        for required in ["Serialize", "Deserialize", "specta::Type"] {
            assert!(
                form.contains(required),
                "封闭枚举 MUST 带 {required}（有数据位的形态出 wire 面）：\n{form}"
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
/// 体内零标量表（该函数体 `"i64"` 零命中）；`rust.rs` 全文 `"i64"` 恰 **2**（`value_type`
/// 的非对象兜底与 `integer_type` 的值域兜底两处；宽度判定已收敛到 `integer_type` 单点，
/// `scalar_type` 不再自带字面量）；`contract.rs` 的注释词面已改指 `value_type`。
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
    "type": "object",
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

    let bare = render_types(r#"{"Marker": {"type": "object", "x-class": true}}"#);
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
    let array = render_source(&op_schema(
        FQN,
        "load",
        r#"{"params": [{"name": "items", "schema": {"type": "array", "items": {"type": "string"}}}], "returns": {"type": "array", "items": {"type": "integer"}}}"#,
    ));
    assert!(
        array.contains("const _: fn(&[String]) -> Vec<i64> = super::load;"),
        "入参 &[T] / 返回 Vec<T>：\n{array}"
    );

    let map = render_source(&op_schema(
        FQN,
        "load",
        r#"{"params": [{"name": "m", "schema": {"type": "object", "additionalProperties": {"type": "integer"}}}], "returns": {"type": "object"}}"#,
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
    let text = render_source(&mirrored_schema_text(
        FQN,
        r#""Cache": {"type": "object", "properties": {}}"#,
        r#""load": { "x-function": {"params": [], "returns": {"outcome": {"type": "string"}}} }"#,
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
    let text = render_source(&mirrored_schema_text(
        FQN,
        r#""LocalArchiCheck": {"type": "object", "properties": {}}"#,
        r#""file_stem": { "x-function": {"owner": "LocalArchiCheck", "params": [{"name": "timestamp", "schema": {"type": "string"}}], "returns": {"type": "string"}} }"#,
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
    let free = render_source(&op_schema(
        FQN,
        "compose_text",
        r#"{"params": [], "returns": {"type": "string"}}"#,
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
    let (contract, schema) = prepare_local(&mirrored_schema_text(
        FQN,
        format!(
            "{}, {}",
            reexport_def(FQN, "AgentInput", "App.Frontend.Components.Agent.Input"),
            reexport_def(
                FQN,
                "AgentBlocks",
                "App.Frontend.Components.Agent.AgentBlocks"
            )
        ),
        "",
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
    let text =
        render_target(&target, &contract, &schema, &meta(), &no_schemas()).expect("渲染应成功");
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

/// 正向（AC-2 / D13）：文本前若干行为注释块，含勿手改 / 覆盖 / 生成物检查 / impl 分离
/// 四句 + 工具版本 + `common.json@sha256:<12 位>`；文案零本仓脚本名。
#[test]
fn render的头注释四句() {
    let text = render_source(&empty_schema(FQN));
    let header: Vec<&str> = text
        .lines()
        .take_while(|line| line.starts_with("//"))
        .collect();
    assert!(header.len() >= 4, "头注释至少四行：{header:?}");

    let joined = header.join("\n");
    for phrase in ["勿手改", "下次生成时被覆盖", "生成物检查", "impl 块"] {
        assert!(joined.contains(phrase), "头注释含 `{phrase}`：\n{joined}");
    }
    for command in ["api:check", "api:export", "pnpm"] {
        assert!(
            !joined.contains(command),
            "头注释零本仓脚本名 `{command}`：\n{joined}"
        );
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
    let source = mirrored_schema_text(
        FQN,
        r#""Zeta": {"type": "object", "properties": {}}, "Alpha": {"type": "object", "properties": {}}"#,
        "",
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
    let text = render_source(&mirrored_schema_text(
        FQN,
        r#""Item": {"type": "object", "properties": {"a": {"type": "string"}}}"#,
        r#""load": { "x-function": {"params": [], "returns": {"type": "string"}} }"#,
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
    let text = render_source(&empty_schema(FQN));
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
    let text = render_source(&empty_schema(FQN));
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

    let mut properties = String::new();
    for index in 0..200 {
        if index > 0 {
            properties.push_str(", ");
        }
        properties.push_str(&format!(
            "\"op{index}\": {{ \"x-function\": {} }}",
            r#"{"params": [], "returns": {"type": "string"}}"#
        ));
    }
    let source = mirrored_schema_text(FQN, &types, &properties);

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
    // 超长 `description` 只落类型级 `///` 注释行（单行、零断行、零裸代码行）
    let doc = format!("/// {long}");
    let has_doc = text.lines().any(|line| line == doc);
    assert!(
        has_doc,
        "超长 description 落单行 `///`（不折行、不破坏语法）：{}",
        text.lines().take(8).collect::<Vec<_>>().join("\n")
    );
}

/// 异常（AC-3 / AC-8）：封闭枚举落 wire 面（有数据位的形态），且不降级为错误。
///
/// ① **不降级为错误**：union 形 `$defs` 经 `render_target` 分派后仍 `Ok`（`Err` 只会
///    来自上游 `reject_generics` / `verify_defs`），且文本落 `pub enum` 与两变体；
/// ② **不静默丢线形态**：三项 wire 派生在场（封闭枚举的词表值就是线形态）——
///    元素 crate 的 `serde` / `specta` 依赖是**消费方前置**（缺依赖的表现是 `cargo build` 失败），
///    渲染期不探测依赖面。
#[test]
fn render的封闭枚举落wire面且不降级为错误() {
    let source = mirrored_schema_text(
        FQN,
        r#""Mode": {"oneOf": [{"const": "a"}, {"const": "b"}]}"#,
        "",
    );
    let (contract, schema) = prepare_local(&source);

    // ① 经语言分派处（`render_target`）走完整路径：`Ok` 而非 `Err(ApiError::Element)`
    let target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules: std::collections::BTreeMap::new(),
    };
    let text = render_target(&target, &contract, &schema, &meta(), &no_schemas())
        .expect("enum 形态不降级为 Element 错误（本期不按元素依赖面判错）");
    assert!(
        text.contains("pub enum Mode {"),
        "enum 仍按形态渲染：\n{text}"
    );
    assert!(text.contains("    A,"), "变体 A 在案：\n{text}");
    assert!(text.contains("    B,"), "变体 B 在案：\n{text}");

    // ② wire 面三项同进同出（封闭枚举是有数据位的形态）
    for required in ["Serialize", "Deserialize", "specta::Type"] {
        assert!(
            text.contains(required),
            "封闭枚举 MUST 带 {required}（wire 面在场）：\n{text}"
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

    // 源码自证：生成物的 import 是**字面量**（derive 名与手写逐字相同，绕过 `serde::` 限定路径）；
    // 本文件零任何 serde 派生 / 序列化 API 调用
    let rust_source = include_str!("rust.rs");
    assert_eq!(
        rust_source
            .matches("\"use serde::{Deserialize, Serialize};")
            .count(),
        1,
        "wire 面的非限定导入字面量恰 1 处：\n{rust_source}"
    );
    for qualified in ["serde::Serialize,", "serde::Deserialize,"] {
        assert!(
            !rust_source.contains(qualified),
            "derive 项 MUST NOT 写成限定路径 `{qualified}`（那会破坏与手写的逐字相同）"
        );
    }
    assert!(
        rust_source.contains("use serde::{Deserialize, Serialize};"),
        "有 wire 面时补非限定导入（否则生成物编译不过，且与手写逐字相同会破）"
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
    // 第五参（只读解析表）使签名超 100 字符 ⇒ rustfmt 必折成多行
    assert!(
        source.contains(
            "pub fn render(\n    contract: &Contract,\n    schema: &Value,\n    meta: &ApiMeta,\n    target: &EmitTarget,\n    schemas: &BTreeMap<String, Value>,\n) -> String {"
        ),
        "渲染器签名为五参单形（解析表末位）、纯出 String（D46）"
    );
    assert!(
        !source.contains(
            "pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String"
        ),
        "四参单行形态已退役（第五参入参）"
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
    let reference = "./Agent.schema.json#/$defs/Input";
    let fqn = crate::api::reexport_target("App.Frontend.Components.ToolCards", reference)
        .expect("外部文件引用取目标 FQN");
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
    let (contract, schema) = prepare_local(&mirrored_schema_text(
        FQN,
        reexport_def(FQN, "AgentInput", "App.Frontend.Components.Agent.Input"),
        "",
    ));
    let target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules: std::collections::BTreeMap::new(),
    };
    let text = render_target(&target, &contract, &schema, &meta(), &no_schemas())
        .expect("查不到目标不是错误");
    assert!(
        text.contains("// 透传自 App.Frontend.Components.Agent.Input"),
        "只留注释：\n{text}"
    );
    assert!(!text.contains("pub use "), "不产 `pub use`：\n{text}");
    assert!(
        !text.contains("Agent.Input::AgentInput"),
        "MUST NOT 退化到 FQN 末段：\n{text}"
    );

    let (plain_contract, plain_schema) = prepare_local(&mirrored_schema_text(
        FQN,
        r#""Item": {"type": "object", "properties": {}}"#,
        "",
    ));
    let by_empty = render_target(
        &target,
        &plain_contract,
        &plain_schema,
        &meta(),
        &no_schemas(),
    )
    .expect("零透传条目渲染成功");
    assert_eq!(
        by_empty,
        render(
            &plain_contract,
            &plain_schema,
            &meta(),
            &target,
            &no_schemas()
        ),
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
        .filter(|member| {
            !member.is_empty() && !member.starts_with("///") && !member.starts_with("#[")
        })
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
    // 零裸 `// {`：唯一允许的形态是 `/// {`（[`doc_line`] 内部已过 `rust_comment_text`）。
    // 裸 `// {{…}}` 插值会让契约文本未转义就进注释，故逐处钉前一个字符。
    for (index, _) in source.match_indices("// {") {
        assert_eq!(
            source[..index].chars().next_back(),
            Some('/'),
            "注释插值只允许 `/// {{` 形态（位置 {index}）"
        );
    }

    // 行为面：透传目标 FQN 含换行 → `// 透传自 …` 仍是一行注释
    let (contract, schema) = prepare_local(&mirrored_schema_text(
        FQN,
        reexport_def(FQN, "AgentInput", r#"App.Frontend\nComponents.Agent"#),
        "",
    ));
    let target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: crate::api::paths::EntryForm::Crate,
        modules: std::collections::BTreeMap::new(),
    };
    let text = render_target(&target, &contract, &schema, &meta(), &no_schemas())
        .expect("换行不影响渲染成功");
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
fn render的枚举变体名恒为合法标识符() {
    let text = render_types(
        r#"{"Mode": {"oneOf": [{"const": "it's"}, {"const": "a b"}, {"const": "1abc"}, {"const": ""}, {"const": "ready"}]}}"#,
    );
    assert!(
        text.contains("pub enum Mode {"),
        "封闭枚举 → pub enum：\n{text}"
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
// 出口单形（D46 / AC-24）：五参带签名恰一、零异名孪生、render_scope 私有
// ---------------------------------------------------------------------------

/// 正向（AC-24 ① / D46）：`emit/rust.rs` 内**带完整五参签名**的 `pub fn render` **恰 1**
/// 命中（解析表 **末位**、返回 `String`；第五参使签名超 100 字符 ⇒ rustfmt 折成多行，
/// 此处按多行形态钉；MUST NOT 只数 `^pub fn render\(`）；行为面：同一契约 + 同一
/// `EmitTarget` 经 `render(..)` 与经 `emit::render_target(..)` 分派产出逐字相等。
#[test]
fn render出口单形五参签名恰一() {
    let source = include_str!("rust.rs");
    const SIGNATURE: &str = "pub fn render(\n    contract: &Contract,\n    schema: &Value,\n    meta: &ApiMeta,\n    target: &EmitTarget,\n    schemas: &BTreeMap<String, Value>,\n) -> String {";
    assert_eq!(
        source.matches(SIGNATURE).count(),
        1,
        "五参单形（解析表末位、返回 `String`）恰 1 处（D46）"
    );

    let (contract, schema) = prepare_local(&mirrored_schema_text(
        FQN,
        r#""Item": {"type": "object", "properties": {"a": {"type": "string"}}}"#,
        "",
    ));
    let target = empty_target();
    let direct = render(&contract, &schema, &meta(), &target, &no_schemas());
    let dispatched =
        render_target(&target, &contract, &schema, &meta(), &no_schemas()).expect("分派应成功");
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
    let (plain_contract, plain_schema) = prepare_local(&mirrored_schema_text(
        FQN,
        r#""Item": {"type": "object", "properties": {}}"#,
        "",
    ));
    let by_empty = render(
        &plain_contract,
        &plain_schema,
        &meta(),
        &empty_target(),
        &no_schemas(),
    );
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
        render(
            &plain_contract,
            &plain_schema,
            &meta(),
            &filled,
            &no_schemas()
        ),
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
/// 本文件（测试）以完整五参形态直接驱动 `render` 属测试相位的合法姿态。
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
/// 的合法命中点恒 **5**（`struct_block` 的 `is_some` + `unwrap_or`、`trait_block` 的
/// `properties` 方法 / 构造器 / `x-static` 成员三处），任何「恰 N / 相等」的文件级断言都无
/// 区分能力）：`value_type` 体内 `"x-function"` 字面量**恰 0**；`struct_block` / `trait_block`
/// 两处 `x-function` 读取点**仍在**（`properties` 值的方法分流，后者另有构造器与静态方法的
/// 两处成员读取）。行为面：`{"type":"array","items":{"x-function":…}}` 形态的渲染需求在
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

    let source = op_schema(
        FQN,
        "load",
        r#"{"params": [], "returns": {"outcome": {"type": "string"}}}"#,
    );
    let (contract, schema) = prepare_local(&source);

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
    let text = render(&contract, &schema, &meta, &empty_target(), &no_schemas());
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

    let source = op_schema(
        FQN,
        "load",
        r#"{"params": [], "returns": {"outcome": {"type": "string"}}}"#,
    );
    let (contract, schema) = prepare_local(&source);
    let mut meta = meta();
    meta.common = BTreeMap::new();
    let text = render(&contract, &schema, &meta, &empty_target(), &no_schemas());
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

// ---------------------------------------------------------------------------
// 可选与可空两个独立事实（AC-1 / AC-3 / D1）
// ---------------------------------------------------------------------------

/// struct 内某字段的整行（含缩进；缺席即断言失败）。
fn field_line<'a>(text: &'a str, field: &str) -> &'a str {
    text.lines()
        .find(|line| line.trim_start().starts_with(&format!("pub {field}:")))
        .unwrap_or_else(|| panic!("struct 内应有字段 `{field}`：\n{text}"))
}

/// 单字段 `api-types` 源文（`$defs.Holder.properties.f` 取给定片段）。
fn holder_types(fragment: &str, required: bool) -> String {
    let required = if required {
        ", \"required\": [\"f\"]"
    } else {
        ""
    };
    format!(r#"{{"Holder": {{"type": "object", "properties": {{"f": {fragment}}}{required}}}}}"#)
}

/// `api-op` 单操作源文（`params` / `returns` 原文直传）。
fn op_source(params: &str, returns: &str) -> String {
    op_schema(
        FQN,
        "load",
        format!(r#"{{"params": {params}, "returns": {returns}}}"#),
    )
}

/// 异常（AC-3 关键反例）：可空与可选叠加（字段非 `required` 且 `type` 含 `null`）→
/// `pub c: Option<String>,`（只多一层）；全渲染文本 `Option<Option<` 零命中。
#[test]
fn render的可空与可选叠加只包一层() {
    let text = render_types(
        r#"{
  "Item": {
    "type": "object",
    "properties": {
      "a": {"type": "string"},
      "c": {"type": ["string", "null"]},
      "d": {"type": ["array", "null"], "items": {"type": ["integer", "null"]}}
    },
    "required": ["a"]
  }
}"#,
    );
    assert_eq!(
        field_line(&text, "a").trim(),
        "pub a: String,",
        "必填 + 非 nullable → 裸类型：\n{text}"
    );
    assert_eq!(
        field_line(&text, "c").trim(),
        "pub c: Option<String>,",
        "可选 + 显式 null → 单层 `Option<…>`：\n{text}"
    );
    assert_eq!(
        field_line(&text, "d").trim(),
        "pub d: Option<Vec<Option<i64>>>,",
        "数组位可空 + 元素位可空 → 逐层各一层：\n{text}"
    );
    assert!(
        !text.contains("Option<Option<"),
        "全渲染文本 MUST NOT 出 `Option<Option<`：\n{text}"
    );
}

/// 边界（D6）：`x-function` 字段（即便非 `required`）→ `pub f: fn(…) -> …,` 零
/// `Option<>`（可选轴不套方法面）。
#[test]
fn render的函数指针字段零optional() {
    let text = render_types(
        r#"{"Wrapper": {"type": "object", "properties": {
  "f": {"x-function": {"params": [{"name": "input", "schema": {"type": "string"}}], "returns": {"type": "boolean"}}}
}}}"#,
    );
    assert_eq!(
        field_line(&text, "f").trim(),
        "pub f: fn(input: &str) -> bool,",
        "函数指针字段零 `Option<>`、零 `?`：\n{text}"
    );
    assert!(
        !field_line(&text, "f").contains("Option<"),
        "MUST NOT 出 `Option<fn(…)>` 形态：\n{text}"
    );
}

/// 正向（AC-3，表驱动）：`required` 字段位的显式 null → `Option<T>`；入参位保留位置
/// 语义（`Option<&str>`）。
#[test]
fn render的显式null落optional() {
    let field = render_types(&holder_types(r#"{"type": ["string", "null"]}"#, true));
    assert_eq!(
        field_line(&field, "f").trim(),
        "pub f: Option<String>,",
        "必填 + 显式 null → `Option<String>`（单层）：\n{field}"
    );

    let param = render_source(&op_source(
        r#"[{"name": "a", "schema": {"type": ["string", "null"]}}]"#,
        r#"{"type": "string"}"#,
    ));
    assert!(
        param.contains("const _: fn(Option<&str>) -> String = super::load;"),
        "入参位保留位置语义（`Option<&str>`）：\n{param}"
    );
}

/// 异常（AC-3）：非空分支本身已带兜底（`{"type":["object","null"]}` 且无
/// `additionalProperties`）→ `Option<serde_json::Value>`（单层包装，零
/// `Option<Option<`）。
#[test]
fn render的nullable非空分支自带兜底只包一层() {
    let text = render_types(&holder_types(r#"{"type": ["object", "null"]}"#, true));
    assert_eq!(
        field_line(&text, "f").trim(),
        "pub f: Option<serde_json::Value>,",
        "兜底分支再包一层可空 → 单层 `Option<serde_json::Value>`：\n{text}"
    );
    assert!(
        !text.contains("Option<Option<"),
        "MUST NOT 出双层 `Option<`：\n{text}"
    );
}

/// 边界（AC-3，表驱动）：纯 `null` 档——`required` 字段位 → `serde_json::Value`（既有
/// 兜底，显式期望值）、非 `required` 位 → `Option<serde_json::Value>`（只多一层可选
/// 包装）；零 `()`。
#[test]
fn render的纯null档() {
    let required = render_types(&holder_types(r#"{"type": "null"}"#, true));
    assert_eq!(
        field_line(&required, "f").trim(),
        "pub f: serde_json::Value,",
        "纯 null 档必填位 → 既有兜底 `serde_json::Value`：\n{required}"
    );

    let optional = render_types(&holder_types(r#"{"type": "null"}"#, false));
    assert_eq!(
        field_line(&optional, "f").trim(),
        "pub f: Option<serde_json::Value>,",
        "纯 null 档可选位 → 只多一层可选包装：\n{optional}"
    );

    for text in [&required, &optional] {
        assert!(
            !text.contains("()"),
            "纯 null 档 MUST NOT 落 `()`：\n{text}"
        );
    }
}

/// 正向（AC-3）：数组元素位可空 → `Vec<Option<String>>`；`Param` 位 →
/// `&[Option<String>]`。
#[test]
fn render的数组元素位可空() {
    let field = render_types(&holder_types(
        r#"{"type": "array", "items": {"type": ["string", "null"]}}"#,
        true,
    ));
    assert_eq!(
        field_line(&field, "f").trim(),
        "pub f: Vec<Option<String>>,",
        "字段位 array + 元素可空 → `Vec<Option<String>>`：\n{field}"
    );

    let param = render_source(&op_source(
        r#"[{"name": "items", "schema": {"type": "array", "items": {"type": ["string", "null"]}}}]"#,
        r#"{"type": "string"}"#,
    ));
    assert!(
        param.contains("const _: fn(&[Option<String>]) -> String = super::load;"),
        "入参位 array → `&[Option<String>]`（位置语义 + 元素可空）：\n{param}"
    );
}

/// 异常：全渲染文本 `Option<Option<` 零命中（字段 / 参数 / 数组三面叠加的同一语料）。
#[test]
fn render全文本零双层option() {
    let types = render_types(
        r#"{
  "Holder": {
    "type": "object",
    "properties": {
      "scalar": {"type": ["string", "null"]},
      "list": {"type": "array", "items": {"type": ["integer", "null"]}},
      "nested": {"type": "array", "items": {"type": "array", "items": {"type": ["string", "null"]}}}
    }
  }
}"#,
    );
    let op = render_source(&op_source(
        r#"[{"name": "a", "schema": {"type": ["string", "null"]}}, {"name": "b", "schema": {"type": "array", "items": {"type": ["string", "null"]}}}]"#,
        r#"{"type": ["boolean", "null"]}"#,
    ));
    for text in [&types, &op] {
        assert!(
            !text.contains("Option<Option<"),
            "全渲染文本 MUST NOT 出 `Option<Option<`：\n{text}"
        );
    }
}

/// 边界（表驱动）：元素非 nullable → `Vec<String>`（零多余 `Option`）；缺 `items` →
/// 既有 `Vec<String>` 形态不变。
#[test]
fn render的数组元素位非nullable() {
    let plain = render_types(&holder_types(
        r#"{"type": "array", "items": {"type": "string"}}"#,
        true,
    ));
    assert_eq!(
        field_line(&plain, "f").trim(),
        "pub f: Vec<String>,",
        "元素非 nullable → 零多余 `Option`：\n{plain}"
    );

    let no_items = render_types(&holder_types(r#"{"type": "array"}"#, true));
    assert_eq!(
        field_line(&no_items, "f").trim(),
        "pub f: Vec<String>,",
        "缺 `items` → 既有 `Vec<String>` 形态：\n{no_items}"
    );
}

/// 正向（D15 ②）：条目顶层 `oneOf` 含 `{"type": "null"}` 分支 → 该分支仍落
/// `Variant{i}`（`FALLBACK_VALUE_TYPE` 判据逐字不变——纯 null 档的兜底文本未换成别的
/// 形态）。
#[test]
fn render的oneof纯null分支仍落占位变体() {
    let text = render_types(
        r##"{"Choice": {"oneOf": [{"type": "null"}, {"$ref": "#/$defs/A"}]},
  "A": {"type": "object", "properties": {}}}"##,
    );
    assert!(
        text.contains("    Variant0,"),
        "纯 null 分支落 `Variant0`（兜底判据不变）：\n{text}"
    );
    assert!(
        !text.contains("serde_json::Value)"),
        "MUST NOT 出 `(serde_json::Value)` 负载：\n{text}"
    );
    assert!(
        text.contains("    AA(A),"),
        "同条目其余分支形态不变：\n{text}"
    );
}

/// 异常（表驱动，既有形态零回归）：对象型分支（无 `additionalProperties`）仍落
/// `Variant{i}`；带具体负载的分支仍出「归一后的变体名 + 负载」形态。
#[test]
fn render的oneof兜底与负载分支零回归() {
    let text = render_types(
        r##"{"Choice": {"oneOf": [{"type": "object"}, {"type": "array", "items": {"type": "string"}}, {"outcome": {"type": "string"}}]}}"##,
    );
    assert!(
        text.contains("    Variant0,"),
        "对象型分支（无 additionalProperties）仍落 `Variant0`：\n{text}"
    );
    assert!(
        text.contains("    VecStringVec<String>(Vec<String>),"),
        "数组分支带负载（归一后的变体名 + 负载）：\n{text}"
    );
    assert!(
        text.contains("    ResultStringStringResult<String, String>(Result<String, String>),"),
        "映射规则分支带负载（查表展开）：\n{text}"
    );
    assert!(
        !text.contains("serde_json::Value"),
        "两分支均带负载，兜底文本零出现：\n{text}"
    );
}

/// 边界：本变更新增的 nullable 前置分支不改「条目顶层 `oneOf`-含-null」的整体形态
/// ——只有纯 null 分支被断言为 `Variant{i}`，其余分支形态逐字不变（多分支完整表达为
/// D15 留白）。
#[test]
fn render的oneof含null整体形态不变() {
    let text = render_types(
        r##"{"Choice": {"oneOf": [{"type": "string"}, {"type": "null"}]},
  "Holder": {"type": "object", "properties": {"payload": {"oneOf": [{"type": "string"}, {"type": "null"}]}}}}"##,
    );
    assert!(
        text.contains("    StringString(String),"),
        "非 null 分支的既有形态不变：\n{text}"
    );
    assert!(
        text.contains("    Variant1,"),
        "纯 null 分支落 `Variant1`：\n{text}"
    );
    assert!(
        !text.contains("Option<String>)"),
        "条目顶层 oneOf 不因 nullable 前置分支变成 `Option<…>` 负载：\n{text}"
    );
}

// ---------------------------------------------------------------------------
// 演练草案金样（AC-3 金样 / 泄漏面）
// ---------------------------------------------------------------------------

/// 演练草案金样（explore D.1 改写为降级词表形态）：三处 `x-common.degraded` 落在
/// `send` 参数位 / `session` 属性位 / `sessions.items`，nullable 与可选两轴同时在场。
fn golden_types() -> &'static str {
    r##"{
  "AskAnswerActions": {
    "type": "object",
    "properties": {
      "onAnswer": {
        "x-function": {
          "params": [{ "name": "text", "schema": { "type": "string" } }],
          "returns": { "type": "boolean" }
        }
      },
      "pendingToolCallId": { "$ref": "#/$defs/SessionSummary" }
    },
    "required": ["onAnswer"]
  },
  "AgentSessionActions": {
    "type": "object",
    "properties": {
      "running": { "type": "boolean" },
      "loading": { "type": "boolean" },
      "steering": { "type": "boolean" },
      "send": {
        "x-function": {
          "params": [
            {
              "name": "input",
              "schema": { "type": ["string", "null"], "x-common": { "degraded": "crossLanguage" } }
            }
          ],
          "returns": { "type": "boolean" }
        }
      },
      "stop": { "x-function": { "params": [], "returns": { "type": "boolean" } } },
      "steer": {
        "x-function": {
          "params": [{ "name": "text", "schema": { "type": "string" } }],
          "returns": { "type": "boolean" }
        }
      }
    },
    "required": ["running", "loading", "steering", "send", "stop", "steer"]
  },
  "AgentSessionViewProps": {
    "type": "object",
    "properties": {
      "root": { "type": "string" },
      "sessionId": { "type": "string" },
      "refreshKey": { "type": "integer" },
      "session": {
        "anyOf": [{ "$ref": "#/$defs/SessionSummary" }, { "type": "null" }],
        "x-common": { "degraded": "noElement" }
      },
      "onDismiss": { "x-function": { "params": [], "returns": { "type": "boolean" } } }
    }
  },
  "SessionPanelState": {
    "type": "object",
    "properties": {
      "sessions": {
        "type": "array",
        "items": {
          "anyOf": [{ "$ref": "#/$defs/SessionSummary" }, { "type": "null" }],
          "x-common": { "degraded": "noElement" }
        }
      },
      "total": { "type": "integer" },
      "listOpen": { "type": "boolean" },
      "canStart": { "type": "boolean" },
      "note": { "type": "string" }
    },
    "required": ["sessions", "total", "listOpen", "canStart"]
  },
  "SessionSummary": {
    "type": "object",
    "properties": { "id": { "type": "string" }, "total": { "type": "integer" } },
    "required": ["id"]
  }
}"##
}

/// 正向（AC-3 金样）：nullable 字段全部落 `Option<…>`，字段名全为合法 Rust 标识符。
#[test]
fn render的演练草案金样() {
    let text = render_types(golden_types());
    for expected in [
        "pub session: Option<SessionSummary>,",
        "pub sessions: Vec<Option<SessionSummary>>,",
        "pub pending_tool_call_id: Option<SessionSummary>,",
        "pub send: fn(input: Option<&str>) -> bool,",
    ] {
        assert!(
            text.contains(expected),
            "金样字段形态 `{expected}` 在案：\n{text}"
        );
    }

    let fields: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("    pub "))
        .filter_map(|line| line.trim_start().strip_prefix("pub "))
        .filter_map(|rest| rest.split(':').next())
        .collect();
    assert!(!fields.is_empty(), "字段名清单非空：\n{text}");
    for field in fields {
        assert!(
            is_rust_identifier(field),
            "字段名 `{field}` MUST 是合法 Rust 标识符：\n{text}"
        );
    }
}

/// 异常（AC-3 泄漏面）：金样渲染文本 `serde_json::Value` 零命中（除纯 null 档用例外的
/// 泄漏面）。
#[test]
fn render的演练草案金样零兜底泄漏() {
    let text = render_types(golden_types());
    assert!(
        !text.contains("serde_json::Value"),
        "金样 MUST NOT 泄漏兜底文本 `serde_json::Value`：\n{text}"
    );
    assert!(
        !text.contains("Option<Option<"),
        "金样 MUST NOT 出双层 `Option<`：\n{text}"
    );
}

/// 边界（表驱动）：同一输入两次渲染逐字相等、为格式化器不动点（零尾随空白、零 tab）；
/// 全语料（九组 nullable 片段）`Option<Option<` 零命中。
#[test]
fn render的幂等与不动点全语料() {
    let first = render_types(golden_types());
    let second = render_types(golden_types());
    assert_eq!(first, second, "同一输入两次渲染逐字相等");
    for line in first.lines() {
        assert_eq!(line, line.trim_end(), "零尾随空白：{line:?}");
        assert!(!line.contains('\t'), "零 tab 缩进：{line:?}");
    }
    assert!(first.ends_with('\n'), "末尾以换行收束");

    let corpus = [
        ("类型数组含 null", r#"{"type": ["string", "null"]}"#, true),
        (
            "三元素类型数组",
            r#"{"type": ["string", "integer", "null"]}"#,
            true,
        ),
        (
            "anyOf 含 null",
            r#"{"anyOf": [{"type": "string"}, {"type": "null"}]}"#,
            true,
        ),
        (
            "oneOf 含 null",
            r#"{"oneOf": [{"type": "integer"}, {"type": "null"}]}"#,
            true,
        ),
        (
            "非 required 叠加显式 null",
            r#"{"type": ["string", "null"]}"#,
            false,
        ),
        (
            "数组元素位可空",
            r#"{"type": "array", "items": {"type": ["string", "null"]}}"#,
            true,
        ),
        ("必填字段（可选轴）", r#"{"type": "string"}"#, true),
        ("可选字段（可选轴）", r#"{"type": "string"}"#, false),
        ("纯 null 档", r#"{"type": "null"}"#, true),
    ];
    assert_eq!(corpus.len(), 9, "语料九组（6 可空 + 2 可选 + 1 纯 null）");
    for (label, fragment, required) in corpus {
        let text = render_types(&holder_types(fragment, required));
        assert!(
            !text.contains("Option<Option<"),
            "{label}：MUST NOT 出双层 `Option<`：\n{text}"
        );
        assert_eq!(
            text,
            render_types(&holder_types(fragment, required)),
            "{label}：同一输入两次渲染逐字相等"
        );
    }
}

// ---------------------------------------------------------------------------
// x-class 的构造入口与静态方法（AC-2 / AC-7 / AC-8）
// ---------------------------------------------------------------------------

/// 正向（AC-2 构造器正例）：`x-constructor` ⇒ trait body 内 `fn new(<params>) -> Self;`，
/// 全文**零** `impl Store`、**零** `const _: fn() -> super::Store`。
#[test]
fn render构造器产trait成员() {
    let text = render_types(
        r#"{
  "Store": {
    "type": "object",
    "x-class": true,
    "properties": {"count": {"x-function": {}}},
    "x-constructor": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}]}}
  }
}"#,
    );
    assert!(
        text.contains("pub trait Store {\n    fn count();\n    fn new(path: &str) -> Self;\n}\n"),
        "body 行序 = properties 方法 → 构造器：\n{text}"
    );
    assert!(
        !text.contains("impl Store"),
        "零 impl（类是 trait）：\n{text}"
    );
    assert!(
        !text.contains("const _: fn() -> super::Store"),
        "零构造器见证（trait 不可 impl / 不可 new）：\n{text}"
    );
}

/// 正向（AC-2 静态方法正例）：`x-static` 两成员 + ctor + `properties.count` ⇒ body 行序
/// properties 方法 → `fn new(…)` → static 按声明序（`open` → `default_path`）；整块逐字。
#[test]
fn render静态方法按声明序追加() {
    let text = render_types(
        r##"{
  "Store": {
    "type": "object",
    "x-class": true,
    "properties": {"count": {"x-function": {}}},
    "x-constructor": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}]}},
    "x-static": {
      "open": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}], "returns": {"$ref": "#/$defs/Store"}}},
      "default_path": {"x-function": {"returns": {"type": "string"}}}
    }
  }
}"##,
    );
    assert!(
        text.contains(
            "pub trait Store {\n    fn count();\n    fn new(path: &str) -> Self;\n    \
             fn open(path: &str) -> Store;\n    fn default_path() -> String;\n}\n"
        ),
        "三段按序 + 声明序 static：\n{text}"
    );
    assert!(
        !text.contains("impl Store") && !text.contains("const _: fn() -> super::Store"),
        "类条目不产自动构造器 / 见证：\n{text}"
    );
}

/// 边界（构造器 params 四臂）：缺省 ⇒ `fn new() -> Self;`；`[]` ⇒ 同；单参 ⇒
/// `fn new(path: &str) -> Self;`；100 参 ⇒ 一行内逗号连接、零尾逗号、零换行。
#[test]
fn render构造器params四臂() {
    let bare = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-constructor": {"x-function": {}}}}"#,
    );
    assert!(
        bare.contains("    fn new() -> Self;\n"),
        "params 缺省：\n{bare}"
    );

    let empty = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-constructor": {"x-function": {"params": []}}}}"#,
    );
    assert!(
        empty.contains("    fn new() -> Self;\n"),
        "params 为空：\n{empty}"
    );

    let single = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-constructor": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}]}}}}"#,
    );
    assert!(
        single.contains("    fn new(path: &str) -> Self;\n"),
        "单参：\n{single}"
    );

    let params: Vec<serde_json::Value> = (0..100)
        .map(|index| serde_json::json!({"name": format!("p{index}"), "schema": {"type": "string"}}))
        .collect();
    let definition = serde_json::json!({
        "Store": {"type": "object", "x-class": true, "x-constructor": {"x-function": {"params": params}}}
    });
    let wide = render_types(&definition.to_string());
    let line = wide
        .lines()
        .find(|line| line.contains("fn new("))
        .unwrap_or_else(|| panic!("应产构造器行：\n{wide}"));
    assert_eq!(line.matches(", ").count(), 99, "100 参逗号连接：{line}");
    assert!(line.ends_with("-> Self;"), "零尾逗号且闭合：{line}");
}

/// 边界（静态方法形态）：`returns` 缺省 ⇒ `fn m();`（无 `->`）；`async: true` 沿用既有
/// `method_signature`（Rust 侧不改产物形态）；成员名 1200 字符合法 ASCII ⇒ 行内含该名。
#[test]
fn render静态方法形态边界() {
    let no_return = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-static": {"m": {"x-function": {}}}}}"#,
    );
    assert!(
        no_return.contains("    fn m();\n"),
        "returns 缺省 ⇒ 无 `->`：\n{no_return}"
    );

    let asynchronous = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-static": {"m": {"x-function": {"async": true, "returns": {"type": "string"}}}}}}"#,
    );
    assert!(
        asynchronous.contains("    fn m() -> String;\n"),
        "async 不改 Rust 产物形态：\n{asynchronous}"
    );

    let long = "a".repeat(1200);
    let mut statics = serde_json::Map::new();
    statics.insert(long.clone(), serde_json::json!({"x-function": {}}));
    let definition = serde_json::json!({
        "Store": {
            "type": "object",
            "x-class": true,
            "x-static": serde_json::Value::Object(statics),
        }
    });
    let text = render_types(&definition.to_string());
    assert!(
        text.contains(&format!("    fn {long}();")),
        "超长成员名在行内"
    );
}

/// 异常（`trait_block` 渲染期零判据）：`x-class` 条目 `properties` 含非函数值（绕过写闸直喂）
/// ⇒ 该行被跳过、零 panic、`render` 仍只出 `String`（错误面只在 `schema.rs`）。
#[test]
fn render类条目非函数属性跳过该行() {
    let mixed = render_types(
        r#"{"Gate": {"type": "object", "x-class": true, "properties": {"count": {"type": "integer"}, "run": {"x-function": {}}}}}"#,
    );
    assert!(
        mixed.contains("pub trait Gate {\n    fn run();\n}\n"),
        "非函数属性零行、方法行按声明序在案：\n{mixed}"
    );
    assert!(
        !mixed.contains("fn count"),
        "非函数值不产 trait 方法行（本层零判据）：\n{mixed}"
    );

    let only_field = render_types(
        r#"{"Gate": {"type": "object", "x-class": true, "properties": {"count": {"type": "integer"}}}}"#,
    );
    assert!(
        only_field.contains("pub trait Gate {\n}\n"),
        "全非函数属性 ⇒ 空 trait body、零 panic：\n{only_field}"
    );
    assert!(
        !only_field.contains("impl Gate {"),
        "`x-class` 条目零自动 impl（非函数属性不进 body）：\n{only_field}"
    );
}

/// 边界（零成员三类）：`properties: {}` + 无 ctor / static ⇒ 恰 `pub trait Gate {\n}\n`；
/// 有 ctor 无 props ⇒ body 只有 `new` 行；有 static 无 props ⇒ body 只有 static 行。
#[test]
fn render零成员三类() {
    let empty = render_types(r#"{"Gate": {"type": "object", "x-class": true, "properties": {}}}"#);
    assert!(
        empty.contains("pub trait Gate {\n}\n"),
        "空 trait：\n{empty}"
    );

    let only_constructor = render_types(
        r#"{"Gate": {"type": "object", "x-class": true, "properties": {}, "x-constructor": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}]}}}}"#,
    );
    assert!(
        only_constructor.contains("pub trait Gate {\n    fn new(path: &str) -> Self;\n}\n"),
        "body 只有 new 行：\n{only_constructor}"
    );

    let only_statics = render_types(
        r#"{"Gate": {"type": "object", "x-class": true, "properties": {}, "x-static": {"open": {"x-function": {}}}}}"#,
    );
    assert!(
        only_statics.contains("pub trait Gate {\n    fn open();\n}\n"),
        "body 只有 static 行：\n{only_statics}"
    );
}

/// 正向（AC-7 空 trait 不产 impl）：`{"type":"object","x-class":true,"properties":{}}` ⇒
/// `render` 只出 `pub trait Gate {\n}\n`，零 `impl Gate {`、零
/// `const _: fn() -> super::Gate`；源码锚：`is_zero_field_object` 体内含 `x-class` 排除、
/// 调用点（`constructible` 收集）恰 1 处。
#[test]
fn render的零字段类不产构造器() {
    let text = render_types(r#"{"Gate": {"type": "object", "x-class": true, "properties": {}}}"#);
    assert!(text.contains("pub trait Gate {\n}\n"), "空 trait：\n{text}");
    assert!(!text.contains("impl Gate {"), "MUST NOT 出 impl：\n{text}");
    assert!(
        !text.contains("const _: fn() -> super::Gate"),
        "MUST NOT 出构造器见证：\n{text}"
    );

    let source = include_str!("rust.rs");
    assert_eq!(
        source.matches("is_zero_field_object").count(),
        2,
        "判据函数单点定义 + `constructible` 收集唯一调用点"
    );
    let body = function_body(source, "fn is_zero_field_object(");
    assert!(
        body.contains("x-class"),
        "判据体内含 `x-class` 排除：{body}"
    );
    assert_eq!(
        source.matches("let constructible").count(),
        1,
        "`constructible` 收集恰一处（零第二份判据）"
    );
}

/// 边界（AC-8 良性对照）：`{"type":"object"}` / `{"type":"object","properties":{}}` /
/// `{"type":"object","additionalProperties":{"type":"string"}}`（三者均无 `x-class`）⇒ 仍产
/// `impl T { … pub fn new() -> Self … }` 与 `const _: fn() -> super::T = super::T::new;`。
#[test]
fn render良性对照仍产自动构造器() {
    for definition in [
        r#"{"Item": {"type": "object"}}"#,
        r#"{"Item": {"type": "object", "properties": {}}}"#,
        r#"{"Item": {"type": "object", "additionalProperties": {"type": "string"}}}"#,
    ] {
        let text = render_types(definition);
        assert!(
            text.contains("impl Item {") && text.contains("pub fn new() -> Self {"),
            "无 x-class 的零字段 object 仍产自动构造器：\n{text}"
        );
        assert!(
            text.contains("const _: fn() -> super::Item = super::Item::new;"),
            "构造器见证仍在：\n{text}"
        );
    }
}

/// 异常（`is_zero_field_object` 非命中形态）：字符串 / 数组 / `null` / 纯 `$ref` alias /
/// 有字段 `object` ⇒ 不产自动构造器（零 panic、零误产 `impl`）。
#[test]
fn render非命中形态零impl() {
    for definition in [
        r#"{"Item": {"type": "string"}}"#,
        r#"{"Item": [1, 2]}"#,
        r#"{"Item": null}"#,
        r##"{"Other": {"type": "object"}, "Item": {"$ref": "#/$defs/Other"}}"##,
        r#"{"Item": {"type": "object", "properties": {"a": {"type": "string"}}}}"#,
    ] {
        let text = render_types(definition);
        assert!(
            !text.contains("impl Item {"),
            "非零字段 object 不产自动构造器（{definition}）：\n{text}"
        );
        assert!(
            !text.contains("const _: fn() -> super::Item"),
            "非零字段 object 不产见证（{definition}）：\n{text}"
        );
    }
}

/// 异常（渲染期零判据）：`params` 值非数组 ⇒ 视为缺省出 `()`；`x-function` 非法 ⇒ 按
/// 缺省兜底、零 panic（非法形态在装载期已死，渲染器不给第二份判据）。
#[test]
fn render构造器参数兜底() {
    for declaration in [
        r#"{"x-function": {"params": "oops"}}"#,
        r#"{"x-function": {"params": {"a": 1}}}"#,
        r#"{"x-function": {"params": null}}"#,
        r#"{"x-function": 42}"#,
    ] {
        let text = render_types(&format!(
            r#"{{"Store": {{"type": "object", "x-class": true, "x-constructor": {declaration}}}}}"#
        ));
        assert!(
            text.contains("    fn new() -> Self;\n"),
            "非法 params / x-function ⇒ 零参兜底（{declaration}）：\n{text}"
        );
    }

    // 缺 `x-function` 包裹的构造器在装载期即死（本层跳过，零第二份判据）
    let unwrapped = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-constructor": {"params": []}}}"#,
    );
    assert!(
        !unwrapped.contains("fn new"),
        "缺 `x-function` 包裹 ⇒ 本层不产成员：\n{unwrapped}"
    );
}

/// 异常（非法形态不可达）：`x-class` 条目带 `type: "string"`（绕过写闸直喂）⇒ 仍按 trait
/// 渲染、不报错；`x-constructor` 带 `returns` ⇒ 渲染器不读取（装载期已禁）；`verify_defs`
/// 复现拦截面。
#[test]
fn render非法形态不可达() {
    let wrong_type = render_types(
        r#"{"Ghost": {"type": "string", "x-class": true, "properties": {"run": {"x-function": {}}}}}"#,
    );
    assert!(
        wrong_type.contains("pub trait Ghost {\n    fn run();\n}\n"),
        "渲染器按 trait 走、零报错路径：\n{wrong_type}"
    );

    let with_returns = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-constructor": {"x-function": {"returns": {"type": "string"}}}}}"#,
    );
    assert!(
        with_returns.contains("    fn new() -> Self;\n"),
        "构造器返回面固定为本类型（不读 returns）：\n{with_returns}"
    );

    let value: serde_json::Value = serde_json::from_str(
        r#"{"$defs": {"Store": {"type": "object", "x-class": true, "x-constructor": {"x-function": {"returns": {"type": "string"}}}}}}"#,
    )
    .expect("夹具必须是合法 JSON");
    let message = match crate::api::schema::verify_defs(&value) {
        Ok(()) => panic!("写闸应拦下构造器的 returns"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    };
    assert!(message.contains("returns"), "拦截面点名禁项：{message}");
}

/// 回归（AC-1 / 幂等）：同一 schema 两次渲染逐字相等；非类条目（零字段 object / 有字段
/// object / 封闭枚举（`oneOf` 全 `const`）/ 带负载 `oneOf` / 标量）与既有产物形态一致。
#[test]
fn render幂等且非类条目形态不变() {
    let source = mirrored_schema_text(
        FQN,
        r##""Unit": {"type": "object", "properties": {}},
            "Item": {"type": "object", "properties": {"a": {"type": "string"}}},
            "Choice": {"oneOf": [{"const": "a"}, {"const": "b"}]},
            "Variant": {"oneOf": [{"type": "string"}, {"type": "integer"}]},
            "Scalar": {"type": "integer"}"##,
        "",
    );
    let first = render_source(&source);
    let second = render_source(&source);
    assert_eq!(first, second, "同一 schema 两次渲染逐字相等");

    for needle in [
        "pub struct Unit;",
        "pub struct Item {",
        "pub enum Choice {",
        "pub enum Variant {",
        "pub struct Scalar(pub i64);",
    ] {
        assert!(
            first.contains(needle),
            "非类条目形态不变（缺 `{needle}`）：\n{first}"
        );
    }
    assert!(
        !first.contains("pub trait"),
        "非类条目零 trait 产物：\n{first}"
    );
}

/// 正向（入参拼装兜底）：`params[].name` 缺省回退 `arg`、参数 `schema` 缺省回退 param 自身
/// （`param_list` 是构造器与 `method_signature` 共用的唯一拼装点）。
#[test]
fn render入参拼装兜底() {
    let constructor = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-constructor": {"x-function": {"params": [{"type": "integer"}]}}}}"#,
    );
    assert!(
        constructor.contains("    fn new(arg: i64) -> Self;\n"),
        "缺 name ⇒ `arg`、schema 缺省回退 param 自身：\n{constructor}"
    );

    let statics = render_types(
        r#"{"Store": {"type": "object", "x-class": true, "x-static": {"open": {"x-function": {"params": [{"name": "raw", "schema": {"type": "object", "additionalProperties": {"type": "integer"}}}]}}}}}"#,
    );
    assert!(
        statics.contains("    fn open(raw: BTreeMap<String, i64>);\n"),
        "静态方法复用同一拼装点：\n{statics}"
    );
}

// ---------------------------------------------------------------------------
// 契约词汇的「类型出生能力」：golden / 宽度 / 命名 / 文档 / wire 面 / facet
// （AC-2 / AC-3 / AC-5 / AC-6 / AC-7 / AC-8 / AC-9）
//
// 本节全部用例零 mock：契约原文手写 → `prepare` 真实投影 → `render` 纯出 `String`；
// golden 比对面取 `include_str!("../../dto.rs")` 的手写类型本体（生成物与手写逐字相同是
// 本变更的核心闸，故期望文本直接取源码，不另抄一份）。
// ---------------------------------------------------------------------------

/// `dto.rs` 全文：golden 逐字比对的**唯一事实源**。
const DTO_SOURCE: &str = include_str!("../../dto.rs");

/// 文本内某类型的**定义段**（连续 `#[…]` 属性行起、类型体闭合行止）。
///
/// 属性面可能折行（`#[derive(` / `    …十项…,` / `)]` 三行形态）——向上收集按括号配对走，
/// 不能只看行首 `#[`（`)]` 收尾行不以 `#[` 开头，只看行首会把折行 derive 整块漏掉）。
///
/// 类型级 `///` 不进比对面（D-11 例外：手写类型文档是契约表达不了的多行段落）；字段 /
/// 变体级 `///` 在类型体内，照比。
fn type_segment(source: &str, type_name: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let heads = [
        format!("pub struct {type_name}"),
        format!("pub enum {type_name}"),
    ];
    let anchor = lines
        .iter()
        .position(|line| {
            let trimmed = line.trim_end();
            heads.iter().any(|head| {
                trimmed.starts_with(head.as_str())
                    && trimmed[head.len()..]
                        .chars()
                        .next()
                        .is_none_or(|next| matches!(next, ' ' | '{' | '(' | ';'))
            })
        })
        .unwrap_or_else(|| panic!("文本内应有类型 `{type_name}` 的定义段"));
    let mut start = anchor;
    // 向上收集属性块：`unclosed` 是「还需上方多少个开括号」——折行 derive 的 `)]` 收尾行
    // 让它是正数，只有括号配平且本行不是属性行首时才停。
    let mut unclosed = 0i32;
    while start > 0 {
        let previous = lines[start - 1].trim();
        let opens = previous
            .chars()
            .filter(|character| matches!(character, '(' | '['))
            .count() as i32;
        let closes = previous
            .chars()
            .filter(|character| matches!(character, ')' | ']'))
            .count() as i32;
        let remaining = unclosed + closes - opens;
        if unclosed == 0 && !previous.starts_with("#[") && remaining <= 0 {
            break;
        }
        unclosed = remaining;
        start -= 1;
    }
    let mut end = anchor;
    if lines[anchor].contains('{') {
        let mut depth = 0i32;
        for (index, line) in lines.iter().enumerate().skip(anchor) {
            for character in line.chars() {
                match character {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if depth <= 0 {
                end = index;
                break;
            }
        }
    }
    lines[start..=end].join("\n")
}

/// 跨元素字段引用（`<相对前缀><目标叶段>.schema.json#/$defs/<名>`；前缀经 `paths` 单点求，
/// 夹具 MUST NOT 手算 `../` 层数）。
fn external_ref(target: &str, name: &str) -> String {
    format!(
        "{}{}.schema.json#/$defs/{name}",
        crate::api::paths::ref_relative(FQN, target),
        crate::api::paths::leaf_name(target)
    )
}

/// `ArchiAssetSet` 的契约原文：三字段 `$ref` 到本文件 `$defs` 条目 + 逐字段语义，`required`
/// 全列、键全 `camelCase`；三个目标类型只需「可 `Eq`」即够（比对面只有 `ArchiAssetSet` 段）。
fn archi_asset_set_types() -> &'static str {
    r##"{
  "ArchiAssetSet": {
    "type": "object",
    "properties": {
      "topology": {"$ref": "#/$defs/ArchiTopologyAsset", "description": "拓扑（模型声明位置与出入度数；零额外读盘）"},
      "interface": {"$ref": "#/$defs/ArchiInterfaceAsset", "description": "接口（`<叶段>.schema.json`；只给键名摘要，不给 content）"},
      "doc": {"$ref": "#/$defs/ArchiDocAsset", "description": "文档（`<叶段>.md`；只给 `##` 段名与体量，不给正文）"}
    },
    "required": ["topology", "interface", "doc"]
  },
  "ArchiTopologyAsset": {"type": "object", "properties": {"file": {"type": "string"}}, "required": ["file"]},
  "ArchiInterfaceAsset": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]},
  "ArchiDocAsset": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}
}"##
}

/// 正向（AC-2 core gate）：`ArchiAssetSet` 的类型定义段与 `dto.rs::ArchiAssetSet` **逐字
/// 相同**——derive 单行（≤96 字符）+ 容器级 `#[serde(rename_all = "camelCase")]` + 三行
/// 字段 `///` + 三行字段。
#[test]
fn render的golden资产集与dto逐字相同() {
    let text = render_types(archi_asset_set_types());
    let handwritten = type_segment(DTO_SOURCE, "ArchiAssetSet");
    assert_eq!(
        type_segment(&text, "ArchiAssetSet"),
        handwritten,
        "类型定义段 MUST 与 `dto.rs::ArchiAssetSet` 逐字相同：\n{text}"
    );
    assert!(
        handwritten.contains(
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]"
        ),
        "derive 单行在案（≤96 字符不折行）：\n{handwritten}"
    );
    assert!(
        handwritten.contains("#[serde(rename_all = \"camelCase\")]"),
        "容器级 `rename_all` 上场（零逐项 `rename`）：\n{handwritten}"
    );
}

/// 异常（AC-2 / AC-9 保守口径）：三字段 `$ref` 指向解析表缺席的跨元素目标（空表）⇒ 字段行
/// 照出、`Eq` 保守缺席、零 `Err` / 零 panic、零半截属性行（推不出来只是不推）。
#[test]
fn render的golden资产集跨元素目标缺席时保守不推eq() {
    let types = format!(
        r##"{{"ArchiAssetSet": {{"type": "object", "properties": {{
  "topology": {{"$ref": "{topology}", "description": "拓扑（模型声明位置与出入度数；零额外读盘）"}},
  "interface": {{"$ref": "{interface}", "description": "接口（`<叶段>.schema.json`；只给键名摘要，不给 content）"}},
  "doc": {{"$ref": "{doc}", "description": "文档（`<叶段>.md`；只给 `##` 段名与体量，不给正文）"}}
}}, "required": ["topology", "interface", "doc"]}}}}"##,
        topology = external_ref("App.Backend.Infra.Agent", "ArchiTopologyAsset"),
        interface = external_ref("App.Backend.Infra.Agent", "ArchiInterfaceAsset"),
        doc = external_ref("App.Backend.Infra.Agent", "ArchiDocAsset"),
    );
    let text = render_types(&types);
    let segment = type_segment(&text, "ArchiAssetSet");
    for field in [
        "    pub topology: ArchiTopologyAsset,",
        "    pub interface: ArchiInterfaceAsset,",
        "    pub doc: ArchiDocAsset,",
    ] {
        assert!(
            segment.contains(field),
            "字段行照出（`$ref` 不可解析不影响类型文本）：缺失 `{field}`\n{text}"
        );
    }
    assert_eq!(
        segment
            .lines()
            .filter(|line| line.starts_with("#[derive"))
            .count(),
        1,
        "derive 恰一行（零半截属性行）：\n{segment}"
    );
    assert!(
        !segment.contains(", Eq"),
        "目标缺席 ⇒ `Eq` 保守缺席（推不出来只是不推）：\n{segment}"
    );
}

/// 边界（AC-2 / AC-5）：`required: []`（空数组）⇒ 三字段全 `Option<…>`，容器
/// `rename_all` 与 derive 其余项形态不变；`properties: {}`（空对象）⇒ 走零字段形态，
/// 不产空 `{}` 块。
#[test]
fn render的golden资产集可选轴与零字段形态() {
    let optional_types = archi_asset_set_types().replace(
        "\"required\": [\"topology\", \"interface\", \"doc\"]",
        "\"required\": []",
    );
    let optional = render_types(&optional_types);
    let segment = type_segment(&optional, "ArchiAssetSet");
    for field in [
        "    pub topology: Option<ArchiTopologyAsset>,",
        "    pub interface: Option<ArchiInterfaceAsset>,",
        "    pub doc: Option<ArchiDocAsset>,",
    ] {
        assert!(
            segment.contains(field),
            "`required: []` ⇒ 全 `Option<…>`：缺失 `{field}`\n{optional}"
        );
    }
    assert!(
        segment.contains("#[serde(rename_all = \"camelCase\")]")
            && segment.contains(
                "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]"
            ),
        "可选轴不动属性面（容器 serde 形态与 derive 项集不变）：\n{segment}"
    );
    assert!(
        !segment.contains("Option<Option<"),
        "可选与可空不叠成双层：\n{segment}"
    );

    let zero = render_types(r#"{"ArchiAssetSet": {"type": "object", "properties": {}}}"#);
    assert!(
        zero.contains("pub struct ArchiAssetSet;"),
        "`properties: {{}}` ⇒ 零字段形态，不产空花括号块：\n{zero}"
    );
    assert!(
        !zero.contains("pub struct ArchiAssetSet {"),
        "零字段 MUST NOT 出空花括号块：\n{zero}"
    );
}

/// `ArchiInterfaceAsset` 的契约原文：`path` 可空、`present` 布尔、`bytes` 完整 `u64` 值域、
/// 三数组、键为 Rust 关键字的 `abstract`，`required` 全列。
fn archi_interface_asset_types() -> &'static str {
    r##"{
  "ArchiInterfaceAsset": {
    "type": "object",
    "properties": {
      "path": {"type": ["string", "null"], "description": "接口定义文件的 repo 相对路径（派生不出路径时为 `null`）"},
      "present": {"type": "boolean", "description": "文件是否在盘（`false` 时三数组为空、`bytes = 0`）"},
      "bytes": {"type": "integer", "minimum": 0, "maximum": 18446744073709551615, "description": "文件字节数（不在盘 ⇒ `0`）"},
      "ops": {"type": "array", "items": {"type": "string"}, "description": "操作键（`properties` 里带 `x-function` 的键）"},
      "types": {"type": "array", "items": {"type": "string"}, "description": "本模块自有类型键（`$defs` 里非纯 `$ref` alias 者）"},
      "reexports": {"type": "array", "items": {"type": "string"}, "description": "透传键（`$defs` 里的纯 `$ref` alias 者）"},
      "abstract": {"type": "string", "description": "一句话判读（`abstract` 是 Rust 保留字，故字段名带后缀、线格式由 `rename` 出线）"}
    },
    "required": ["path", "present", "bytes", "ops", "types", "reexports", "abstract"]
  }
}"##
}

/// 正向（AC-2 / AC-5 / AC-6）：五类字段行与 `dto.rs::ArchiInterfaceAsset` 逐字相同
/// （`abstract_note` 字段名属 D-11 例外：只比 `pub r#abstract: String` 与 wire 名
/// `abstract`，不比手写后辍名）。
#[test]
fn render的golden接口资产与dto逐字相同() {
    let text = render_types(archi_interface_asset_types());
    let handwritten = type_segment(DTO_SOURCE, "ArchiInterfaceAsset");
    for line in [
        "    pub path: Option<String>,",
        "    pub present: bool,",
        "    pub bytes: u64,",
        "    pub ops: Vec<String>,",
        "    pub types: Vec<String>,",
        "    pub reexports: Vec<String>,",
    ] {
        assert!(
            handwritten.contains(line),
            "手写基准内含 `{line}`：\n{handwritten}"
        );
        assert!(
            text.contains(line),
            "生成物内含 `{line}`（与 `dto.rs` 逐字相同）：\n{text}"
        );
    }
    assert!(
        text.contains("    pub r#abstract: String,"),
        "关键字键 `abstract` ⇒ `pub r#abstract`（`r#` 剥前缀即 wire 名）：\n{text}"
    );
    assert!(
        handwritten.contains("rename = \"abstract\""),
        "手写侧以 `rename` 出同一线格式（wire 名同为 `abstract`）：\n{handwritten}"
    );
    assert!(
        !text.contains("rename = \"abstract\""),
        "生成物侧由容器级 `rename_all` 覆盖，零逐项 `rename`：\n{text}"
    );
    assert_eq!(
        type_segment(&text, "ArchiInterfaceAsset")
            .lines()
            .filter(|line| line.starts_with("#[derive"))
            .count(),
        1,
        "derive 单行（7 项 76 字符）：\n{text}"
    );
}

/// 异常（AC-7）：某字段 `description: ""`（空串）⇒ 该字段零 `///` 行且其余字段行与带说明时
/// 逐字相同（单字段归因，零连带改写）；`description` 为 `null` / `true`（非字符串）⇒ 同为零
/// 行、不 panic。
#[test]
fn render的空说明只摘该字段的注释行() {
    let described = render_types(
        r#"{"Item": {"type": "object", "properties": {"a": {"type": "string", "description": "说明"}, "b": {"type": "boolean"}}, "required": ["a", "b"]}}"#,
    );
    for blank in [r#""""#, "null", "true"] {
        let undecorated = render_types(&format!(
            r#"{{"Item": {{"type": "object", "properties": {{"a": {{"type": "string", "description": {blank}}}, "b": {{"type": "boolean"}}}}, "required": ["a", "b"]}}}}"#
        ));
        let without_key = render_types(
            r#"{"Item": {"type": "object", "properties": {"a": {"type": "string"}, "b": {"type": "boolean"}}, "required": ["a", "b"]}}"#,
        );
        assert_eq!(
            undecorated, without_key,
            "`description: {blank}` ⇒ 与不带该键逐字相同（零连带改写）：\n{undecorated}"
        );
        assert!(
            !type_segment(&undecorated, "Item").contains("///"),
            "非字符串 / 空串说明零 `///` 行：\n{undecorated}"
        );
    }
    assert!(
        type_segment(&described, "Item").contains("    /// 说明\n    pub a: String,"),
        "有说明时字段 `///` 在字段行之上：\n{described}"
    );
}

/// 边界（AC-6 / AC-2）：`required` 缺省（全可选）与全列两形态对照——`path` 本身可空 + 可选
/// 叠加仍只 `Option<String>`（`Option<Option<…>>` 零产出）；`bytes` 上界恰为 20 位十进制
/// `u64::MAX` ⇒ 精确命中 `u64`（不经 f64 近似）；`["integer","null"]` + 完整 u32 值域 ⇒
/// `Option<u32>`。
#[test]
fn render的golden接口资产宽度与可选叠加() {
    let all_optional = archi_interface_asset_types().replace(
        "\"required\": [\"path\", \"present\", \"bytes\", \"ops\", \"types\", \"reexports\", \"abstract\"]",
        "\"required\": []",
    );
    let text = render_types(&all_optional);
    assert!(
        text.contains("    pub path: Option<String>,"),
        "可空 + 可选叠加仍只一层 `Option<…>`：\n{text}"
    );
    assert!(
        !text.contains("Option<Option<"),
        "零双层 `Option<…>`：\n{text}"
    );
    assert!(
        text.contains("    pub bytes: Option<u64>,"),
        "`u64::MAX`（20 位十进制）精确命中 `u64`：\n{text}"
    );

    let nullable_u32 = render_types(&holder_types(
        r#"{"type": ["integer", "null"], "minimum": 0, "maximum": 4294967295}"#,
        true,
    ));
    assert_eq!(
        field_line(&nullable_u32, "f").trim(),
        "pub f: Option<u32>,",
        "可空 + 完整 u32 值域 ⇒ `Option<u32>`：\n{nullable_u32}"
    );
}

/// `ElementKind` 的契约原文：`oneOf` 四分支 `const` + 逐分支语义、无 `default`。
fn element_kind_types() -> &'static str {
    r##"{"ElementKind": {"oneOf": [
  {"const": "package", "description": "包级（最小编译单元 / 独立依赖管理）"},
  {"const": "domain", "description": "域级（按业务实体边界聚合的代码）"},
  {"const": "module", "description": "模块级（目录型模块 / 视图 / 单文件组件）"},
  {"const": "composite", "description": "组合（无实际层级，仅将过多的同级元素分类；可嵌套在任意类型之上）"}
]}}"##
}

/// 正向（AC-3 / AC-2）：四 `const` 分支 ⇒ 与 `dto.rs::ElementKind` 段落逐字相同——9 项
/// derive 单行（88 字符）+ `#[serde(rename_all = "lowercase")]` + 四变体各带 `///`；零
/// `Default` / 零 `#[default]`（`default` 缺席即不推，D-2）。
#[test]
fn render的golden元素种类与dto逐字相同() {
    let text = render_types(element_kind_types());
    let handwritten = type_segment(DTO_SOURCE, "ElementKind");
    assert_eq!(
        type_segment(&text, "ElementKind"),
        handwritten,
        "类型定义段 MUST 与 `dto.rs::ElementKind` 逐字相同：\n{text}"
    );
    assert!(
        handwritten.contains(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]"
        ),
        "9 项 derive 单行在案：\n{handwritten}"
    );
    assert!(
        handwritten.contains("#[serde(rename_all = \"lowercase\")]"),
        "变体面候选序 `lowercase` 首试（与手写逐字对齐）：\n{handwritten}"
    );
    assert!(
        !handwritten.contains("Default") && !handwritten.contains("#[default]"),
        "`default` 缺席 ⇒ 零 `Default` / 零 `#[default]`：\n{handwritten}"
    );
}

/// 异常（AC-3 / Q-6 保守口径）：`"default": "unknown"`（值不命中任何 `const` 分支）⇒ 零
/// `Default`、零 `#[default]`、零 `Err`。
#[test]
fn render的default不命中任何分支时零default() {
    let text = render_types(
        r#"{"Mode": {"oneOf": [{"const": "a"}, {"const": "b"}], "default": "unknown"}}"#,
    );
    assert!(
        text.contains("pub enum Mode {"),
        "形态不变（仍为封闭枚举）：\n{text}"
    );
    assert!(
        !text.contains("Default") && !text.contains("#[default]"),
        "值不命中 ⇒ MUST NOT 推 `Default`：\n{text}"
    );
    assert!(
        text.contains("    A,") && text.contains("    B,"),
        "两分支仍在案：\n{text}"
    );
}

/// 边界（AC-3）：单 `const` 分支（`oneOf` 一项）⇒ 仍为封闭枚举形态（unit variant、零悬垂
/// 括号）；`const` 文本非法标识符面归一为合法变体名。
#[test]
fn render的封闭枚举单分支与非法标识符归一() {
    let single = render_types(r#"{"Mode": {"oneOf": [{"const": "only"}]}}"#);
    assert!(
        single.contains("pub enum Mode {\n    Only,\n}\n"),
        "单分支仍是封闭枚举（unit variant、零悬垂括号）：\n{single}"
    );
    assert_eq!(
        enum_members(&single, "Mode"),
        vec!["Only"],
        "唯一变体在案：\n{single}"
    );

    let dirty = render_types(
        r#"{"Mode": {"oneOf": [{"const": "it's"}, {"const": "a b"}, {"const": "1st"}, {"const": ""}]}}"#,
    );
    assert_eq!(
        enum_members(&dirty, "Mode"),
        vec!["ItS", "AB", "V1st", "Unknown"],
        "四类非标识符文本归一：\n{dirty}"
    );
    for member in enum_members(&dirty, "Mode") {
        assert!(
            is_rust_identifier(&member),
            "变体名恒为合法标识符：{member:?}"
        );
    }
}

/// `GateTier` 的契约原文：三 `const`（`off` / `warn` / `strict`）+ `"default": "off"`。
fn gate_tier_types() -> &'static str {
    r##"{"GateTier": {"oneOf": [
  {"const": "off", "description": "不参与门禁（缺省）"},
  {"const": "warn", "description": "违例仅告警"},
  {"const": "strict", "description": "违例即门禁失败"}
], "default": "off"}}"##
}

/// 正向（AC-3 / AC-12 折行锚）：三 `const` + `default` ⇒ 与 `dto.rs::GateTier` 逐字相同——
/// 10 项 derive **折三行**（组装串 97 字符）+ `#[serde(rename_all = "lowercase")]` +
/// `#[default]` 在 `Off,` 上方 + 逐变体 `///`。
#[test]
fn render的golden门禁档位折行与default() {
    let text = render_types(gate_tier_types());
    let handwritten = type_segment(DTO_SOURCE, "GateTier");
    assert_eq!(
        type_segment(&text, "GateTier"),
        handwritten,
        "类型定义段 MUST 与 `dto.rs::GateTier` 逐字相同：\n{text}"
    );
    assert!(
        handwritten.contains(
            "#[derive(\n    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,\n)]"
        ),
        "10 项 derive 折三行（rustfmt 固定点）：\n{handwritten}"
    );
    assert!(
        handwritten.contains("    /// 不参与门禁（缺省）\n    #[default]\n    Off,"),
        "`#[default]` 落在 `Off,` 上方、`///` 之后：\n{handwritten}"
    );
}

/// 异常（AC-8 / AC-9）：混合分支（两个 `const` + 一个 `$ref` 负载）⇒ `const` 支落 unit
/// variant、负载支沿用既有带负载形态；wire 面不出、`Eq` / `Copy` / `Hash` 全缺、零 panic。
#[test]
fn render的混合分支零wire面且facet保守() {
    let text = render_types(
        r##"{"Mixed": {"oneOf": [{"const": "a"}, {"const": "b"}, {"$ref": "#/$defs/Load"}], "default": "a"},
  "Load": {"type": "object", "properties": {}}}"##,
    );
    let segment = type_segment(&text, "Mixed");
    assert!(
        segment.contains("    A,") && segment.contains("    B,"),
        "`const` 支落 unit variant：\n{text}"
    );
    assert!(
        segment.contains("    LoadLoad(Load),"),
        "负载支沿用既有带负载形态：\n{text}"
    );
    for absent in ["Serialize", ", Eq", "Copy", "Hash", "#[default]"] {
        assert!(
            !segment.contains(absent),
            "带负载 union MUST NOT 出 `{absent}`：\n{segment}"
        );
    }
    assert!(
        !text.contains("use serde::"),
        "零 wire 面 ⇒ 零 `serde` 导入：\n{text}"
    );
}

/// 边界（AC-12 / AC-11）：10+ `const` 分支 + `default`（10 项 derive）⇒ derive 行折三行且
/// 两次渲染逐字相同（固定点稳定）；`oneOf: []`（空分支集）⇒ 不 panic、零半截属性行、两次
/// 渲染逐字相同。
#[test]
fn render的超大词表与空union() {
    let consts: Vec<String> = (0..12)
        .map(|index| format!("{{\"const\": \"v{index}\"}}"))
        .collect();
    let types = format!(
        r#"{{"Big": {{"oneOf": [{}], "default": "v0"}}}}"#,
        consts.join(", ")
    );
    let big = render_types(&types);
    assert!(
        big.contains(
            "#[derive(\n    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,\n)]"
        ),
        "10 项 derive 折三行（超大词表不动固定点）：\n{big}"
    );
    assert!(
        big.contains("    #[default]\n    V0,"),
        "`#[default]` 落在首个分支：\n{big}"
    );
    assert_eq!(big, render_types(&types), "两次渲染逐字相同");
    assert_eq!(
        enum_members(&big, "Big").len(),
        12,
        "12 个 unit variant 在案：\n{big}"
    );

    let empty = render_types(r#"{"Void": {"oneOf": []}}"#);
    assert!(
        empty.contains("pub enum Void {\n}\n"),
        "空 union 不 panic、零半截属性行：\n{empty}"
    );
    assert_eq!(
        empty,
        render_types(r#"{"Void": {"oneOf": []}}"#),
        "空 union 两次渲染逐字相同"
    );
}

// ---------------------------------------------------------------------------
// 整数宽度：三条完整值域是唯一可推的目标（AC-6）
// ---------------------------------------------------------------------------

/// 正向（AC-6）：三条完整值域各出对应类型；字段位与标量 newtype 位两臂同源。
#[test]
fn render的整数宽度按完整值域() {
    let cases = [
        (
            r#"{"type": "integer", "minimum": 0, "maximum": 4294967295}"#,
            "u32",
        ),
        (
            r#"{"type": "integer", "minimum": 0, "maximum": 18446744073709551615}"#,
            "u64",
        ),
        (
            r#"{"type": "integer", "minimum": -2147483648, "maximum": 2147483647}"#,
            "i32",
        ),
    ];
    for (fragment, expected) in cases {
        let field = render_types(&holder_types(fragment, true));
        assert_eq!(
            field_line(&field, "f").trim(),
            format!("pub f: {expected},"),
            "字段位命中 `{expected}`：\n{field}"
        );
        let newtype = render_types(&format!(r#"{{"T": {fragment}}}"#));
        assert!(
            newtype.contains(&format!("pub struct T(pub {expected});")),
            "标量 newtype 位与字段位同源（`{expected}`）：\n{newtype}"
        );
    }
    assert!(
        DTO_SOURCE.contains("pub bytes: u64,"),
        "手写基准同样落 `u64`（golden 前提）：\n{}",
        type_segment(DTO_SOURCE, "ArchiInterfaceAsset")
    );
}

/// 异常（AC-6）：部分范围与单边一律 `i64`——业务约束 ≠ 本地精度；`exclusive*` 先 ±1 归一；
/// `number` 的值域不参与宽度推导（仍 `f64`）。
#[test]
fn render的部分值域与number一律不推窄类型() {
    let cases = [
        (r#"{"type": "integer", "maximum": 100}"#, "i64"),
        (r#"{"type": "integer", "minimum": 0}"#, "i64"),
        (r#"{"type": "integer", "minimum": -1}"#, "i64"),
        (
            r#"{"type": "integer", "minimum": 0, "maximum": 4294967296}"#,
            "i64",
        ),
        (
            r#"{"type": "integer", "exclusiveMinimum": -1, "exclusiveMaximum": 4294967296}"#,
            "u32",
        ),
        (
            r#"{"type": "number", "minimum": 0, "maximum": 4294967295}"#,
            "f64",
        ),
    ];
    for (fragment, expected) in cases {
        let text = render_types(&holder_types(fragment, true));
        assert_eq!(
            field_line(&text, "f").trim(),
            format!("pub f: {expected},"),
            "`{fragment}` ⇒ `{expected}`：\n{text}"
        );
    }
}

/// 边界（AC-6）：无 bounds ⇒ `i64` 且与今天产物逐字相同；`minimum` / `maximum` 非数字 ⇒
/// `i64`、不 panic；`minimum > maximum` 的畸形区间 ⇒ `i64`（不外溢、不 panic）。
#[test]
fn render的整数宽度边界保守() {
    let cases = [
        (r#"{"type": "integer"}"#, "i64"),
        (
            r#"{"type": "integer", "minimum": "0", "maximum": null}"#,
            "i64",
        ),
        (
            r#"{"type": "integer", "minimum": true, "maximum": false}"#,
            "i64",
        ),
        (r#"{"type": "integer", "minimum": 10, "maximum": 1}"#, "i64"),
    ];
    for (fragment, expected) in cases {
        let text = render_types(&holder_types(fragment, true));
        assert_eq!(
            field_line(&text, "f").trim(),
            format!("pub f: {expected},"),
            "`{fragment}` ⇒ `{expected}`（保守兜底）：\n{text}"
        );
    }

    let bare = render_types(r#"{"Count": {"type": "integer"}}"#);
    assert!(
        bare.contains("pub struct Count(pub i64);"),
        "无 bounds 的裸整数与今天产物逐字相同：\n{bare}"
    );
}

// ---------------------------------------------------------------------------
// 命名策略：容器级 rename_all 优先、关键字走 r#、全不命中才逐项 rename（AC-5）
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`camelCase` 键 ⇒ `snake_case` + 容器级 `rename_all`；关键字键 ⇒
/// `pub r#in` 且零逐项 `rename`（`r#` 剥前缀即 wire 名）。
#[test]
fn render的关键字键走raw标识符() {
    let text = render_types(
        r#"{"Item": {"type": "object", "properties": {"in": {"type": "integer"}, "typeOnly": {"type": "boolean"}}, "required": ["in", "typeOnly"]}}"#,
    );
    assert!(
        text.contains("#[serde(rename_all = \"camelCase\")]"),
        "容器级 `rename_all` 上场：\n{text}"
    );
    assert_eq!(
        field_line(&text, "r#in").trim(),
        "pub r#in: i64,",
        "关键字键 ⇒ `pub r#in`：\n{text}"
    );
    assert_eq!(
        field_line(&text, "type_only").trim(),
        "pub type_only: bool,",
        "camelCase 键 ⇒ `snake_case` 本地名：\n{text}"
    );
    assert!(
        !text.contains("rename = \"in\""),
        "`r#` 剥前缀即 wire 名 ⇒ 零逐项 `rename`：\n{text}"
    );

    let abstract_key = render_types(
        r#"{"Item": {"type": "object", "properties": {"abstract": {"type": "string"}}, "required": ["abstract"]}}"#,
    );
    assert!(
        abstract_key.contains("    pub r#abstract: String,"),
        "`abstract` 键同式出 `pub r#abstract`：\n{abstract_key}"
    );
}

/// 异常（AC-5）：候选 `rename_all` 全不命中（wire 名 `a-b` / `x y` 一类不可机械回归）⇒ 每
/// 字段上方逐项 `#[serde(rename = …)]`，本地名仍 `snake_case`；wire 名含引号 / 反斜杠 / 换行
/// ⇒ 经 `string_literal`（`{:?}` 形态）转义，产物恒为合法 Rust 字面量。
#[test]
fn render的不可机械回归键走逐项rename() {
    let text = render_types(
        r#"{"Item": {"type": "object", "properties": {"a-b": {"type": "integer"}, "x y": {"type": "string"}}, "required": ["a-b", "x y"]}}"#,
    );
    assert!(
        !text.contains("rename_all"),
        "容器级候选全不命中 ⇒ 零 `rename_all`：\n{text}"
    );
    assert!(
        text.contains("    #[serde(rename = \"a-b\")]\n    pub a_b: i64,"),
        "逐项 `rename` 在字段行上方、本地名仍 `snake_case`：\n{text}"
    );
    assert!(
        text.contains("    #[serde(rename = \"x y\")]\n    pub x_y: String,"),
        "空格键同式：\n{text}"
    );

    // 转义面：引号 / 反斜杠 / 换行经 `string_literal`（`{:?}` 形态）
    let escaped = render_types(
        "{\"Item\": {\"type\": \"object\", \"properties\": {\"a\\\"b\": {\"type\": \"integer\"}, \"c\\\\d\": {\"type\": \"string\"}}}}",
    );
    assert!(
        escaped.contains("    #[serde(rename = \"a\\\"b\")]"),
        "引号经 `string_literal` 转义（`{{:?}}` 形态）：\n{escaped}"
    );
    assert!(
        escaped.contains("    #[serde(rename = \"c\\\\d\")]"),
        "反斜杠经 `string_literal` 转义：\n{escaped}"
    );
    assert!(
        !escaped.contains("rename = \"a\"b\""),
        "MUST NOT 手拼未转义引号（产物必须是合法 Rust 字面量）：\n{escaped}"
    );
}

/// 边界（AC-5 / R-12）：四候选逐一命中 + 全不命中各断言所选形态（字段面 `lowercase` 是
/// identity ⇒ 该档只在变体面可达，故按面分组）；变体面候选序 `lowercase` 首试；四不可 raw
/// 的词由写闸拒收（零产出）。
#[test]
fn render的serde形态选择逐档命中() {
    // 字段面：camelCase 首试、snake_case identity 次之、PascalCase 兜底
    let field_cases = [
        (
            "camelCase",
            r#"{"Item": {"type": "object", "properties": {"typeOnly": {"type": "boolean"}}}}"#,
            "#[serde(rename_all = \"camelCase\")]",
        ),
        (
            "snake_case",
            r#"{"Item": {"type": "object", "properties": {"type_only": {"type": "boolean"}}}}"#,
            "#[serde(rename_all = \"snake_case\")]",
        ),
        (
            "PascalCase",
            r#"{"Item": {"type": "object", "properties": {"TypeOnly": {"type": "boolean"}}}}"#,
            "#[serde(rename_all = \"PascalCase\")]",
        ),
    ];
    for (label, types, expected) in field_cases {
        let text = render_types(types);
        assert!(
            text.contains(expected),
            "字段面 {label}：所选形态 `{expected}` 上场：\n{text}"
        );
    }

    // 变体面：lowercase 首试，camelCase / snake_case / PascalCase 依次兜底
    let variant_cases = [
        (
            "lowercase",
            r#"{"Mode": {"oneOf": [{"const": "a"}]}}"#,
            "#[serde(rename_all = \"lowercase\")]",
        ),
        (
            "camelCase",
            r#"{"Mode": {"oneOf": [{"const": "warnValue"}]}}"#,
            "#[serde(rename_all = \"camelCase\")]",
        ),
        (
            "snake_case",
            r#"{"Mode": {"oneOf": [{"const": "warn_value"}]}}"#,
            "#[serde(rename_all = \"snake_case\")]",
        ),
        (
            "PascalCase",
            r#"{"Mode": {"oneOf": [{"const": "Warn"}]}}"#,
            "#[serde(rename_all = \"PascalCase\")]",
        ),
    ];
    for (label, types, expected) in variant_cases {
        let text = render_types(types);
        assert!(
            text.contains(expected),
            "变体面 {label}：所选形态 `{expected}` 上场：\n{text}"
        );
    }

    // 全不命中两形态：字段面与变体面都走逐项 `rename`
    let struct_none =
        render_types(r#"{"Item": {"type": "object", "properties": {"a-b": {"type": "boolean"}}}}"#);
    assert!(
        !struct_none.contains("rename_all") && struct_none.contains("#[serde(rename = \"a-b\")]"),
        "字段面全不命中 ⇒ 逐项 `rename`：\n{struct_none}"
    );
    let variant_none = render_types(r#"{"Mode": {"oneOf": [{"const": "a-b"}]}}"#);
    assert!(
        !variant_none.contains("rename_all")
            && variant_none.contains("    #[serde(rename = \"a-b\")]\n    AB,"),
        "变体面全不命中 ⇒ 逐项 `rename`：\n{variant_none}"
    );

    // 变体面候选序：单字小写词只有 `lowercase` 首试才与手写 `ElementKind` 对齐
    let kind = render_types(element_kind_types());
    assert!(
        kind.contains("#[serde(rename_all = \"lowercase\")]")
            && !kind.contains("#[serde(rename_all = \"camelCase\")]"),
        "变体面 `lowercase` 首试：\n{kind}"
    );

    // 四不可 raw 的词由写闸拒收 ⇒ 渲染器结构上收不到，零 `r#self` / `r#Self` / `r#super` /
    // `r#crate` 产出
    for word in ["self", "Self", "super", "crate"] {
        let value: Value = serde_json::from_str(&format!(
            r#"{{"$defs": {{"T": {{"type": "object", "properties": {{"{word}": {{"type": "string"}}}}}}}}}}"#
        ))
        .expect("夹具必须是合法 JSON");
        let message = match crate::api::schema::verify_defs(&value) {
            Ok(()) => panic!("`{word}` 应被写闸拒收"),
            Err(ApiError::Contract(message)) => message,
            Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
        };
        assert!(
            message.contains(&format!("properties.{word}")),
            "文案点名该字段位（`{word}`）：{message}"
        );
    }
    // 渲染器内零硬编码的四词例外（拒收在写闸，不在渲染期）
    let source = include_str!("rust.rs");
    for word in ["r#self", "r#Self", "r#super", "r#crate"] {
        assert!(
            !source.contains(&format!("\"{word}\"")),
            "渲染器零 `{word}` 硬编码（四词由写闸拒收，渲染期不设第二份例外）"
        );
    }
}

// ---------------------------------------------------------------------------
// 文档注释：类型 / 字段 / 变体三处各一条正例（AC-7）
// ---------------------------------------------------------------------------

/// 正向（AC-7）：类型级 / 字段级 / 变体级三处各出一行 `///`，文本与契约逐字相等，位置固定
/// （类型 `///` 在 derive 行之上、字段 `///` 在字段行之上、变体 `///` 在变体行之上）。
#[test]
fn render的三处文档注释位置固定() {
    let text = render_types(
        r#"{"Shown": {"type": "object", "description": "类型级说明", "properties": {"f": {"type": "string", "description": "字段级说明"}}},
  "Mode": {"oneOf": [{"const": "a", "description": "变体级说明"}]}}"#,
    );
    assert!(
        text.contains("/// 类型级说明\n#[derive("),
        "类型级 `///` 在 derive 行之上：\n{text}"
    );
    assert!(
        text.contains("    /// 字段级说明\n    pub f: Option<String>,"),
        "字段级 `///` 在字段行之上：\n{text}"
    );
    assert!(
        text.contains("    /// 变体级说明\n    A,"),
        "变体级 `///` 在变体行之上：\n{text}"
    );
    for line in text
        .lines()
        .filter(|line| line.trim_start().starts_with("///"))
    {
        assert_eq!(line, line.trim_end(), "注释行零尾随空白：{line:?}");
    }
}

/// 异常（AC-7）：含 `\n` / `\r` / `U+2028` / `U+2029` / `*/` 的 `description` ⇒ 折成空格、
/// 零断行、零非法语法（注释不顶出裸代码行）；`\0`（NUL）与 emoji 原样保留、不 panic。
#[test]
fn render的说明折行不破语法() {
    let text = render_types(
        "{\"Item\": {\"type\": \"object\", \"description\": \"a\\nb\\rc\\u2028d\\u2029e */ f\", \"properties\": {}}}",
    );
    assert!(
        text.contains("/// a b c d e */ f\n"),
        "四类断行字符折成空格、`*/` 原样（注释内无需转义）：\n{text}"
    );
    assert_eq!(
        text.lines()
            // 类型级 `///` 顶格、构造器的 `/// 构造器` 缩进四格：按顶格判即零污染
            .filter(|line| line.starts_with("///"))
            .count(),
        1,
        "类型级注释恰一行（零断行；自动构造器的 `/// 构造器` 不在判据内）：\n{text}"
    );

    let raw = render_types(
        "{\"Item\": {\"type\": \"object\", \"description\": \"\\u0000 与 emoji 🎯 原样\", \"properties\": {}}}",
    );
    assert!(
        raw.contains("/// \u{0} 与 emoji 🎯 原样"),
        "NUL 与 emoji 原样保留（不 panic、不改写）：\n{raw}"
    );
}

/// 边界（AC-7 / AC-12）：`description: ""` / 纯空白串 / 键缺省三形态 ⇒ 零注释行；>1000 字符
/// 的 `description` ⇒ 单行产出且行尾零空白（`trim_end` 固定点）；op 的
/// `x-function.description` 仍丢弃。
#[test]
fn render的说明边界与op说明丢弃() {
    let without = render_types(r#"{"Item": {"type": "object", "properties": {}}}"#);
    for blank in [r#""""#, r#""   ""#] {
        let text = render_types(&format!(
            r#"{{"Item": {{"type": "object", "description": {blank}, "properties": {{}}}}}}"#
        ));
        assert_eq!(text, without, "`description: {blank}` ⇒ 零注释行：\n{text}");
    }
    assert!(
        !type_segment(&without, "Item").contains("///"),
        "键缺省 ⇒ 类型段零注释行（构造器的自证 `///` 不在类型段内）：\n{without}"
    );

    let long = "长".repeat(1001);
    let text = render_types(&format!(
        r#"{{"Item": {{"type": "object", "description": "{long}", "properties": {{}}}}}}"#
    ));
    let doc_lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("///"))
        .collect();
    assert_eq!(doc_lines.len(), 1, "超长说明单行产出：\n{text}");
    assert_eq!(doc_lines[0], doc_lines[0].trim_end(), "行尾零空白");
    assert!(
        doc_lines[0].chars().count() > 1000,
        "整段文本在案（不折行、不截断）：{}",
        doc_lines[0].chars().count()
    );

    // op 的 `x-function.description` 仍丢弃：两次渲染逐字节相同
    let with_description = render_source(&op_schema(
        FQN,
        "load",
        r#"{"description": "操作说明不进体", "returns": {"type": "string"}}"#,
    ));
    let without_description = render_source(&op_schema(
        FQN,
        "load",
        r#"{"returns": {"type": "string"}}"#,
    ));
    assert_eq!(
        with_description, without_description,
        "op 的 `x-function.description` 丢弃（签名见证逐字不变）"
    );
}

// ---------------------------------------------------------------------------
// wire 面射程：谁出三项派生、谁零 wire 面（AC-8）
// ---------------------------------------------------------------------------

/// 正向（AC-8）：有字段 struct / 封闭枚举 / 标量 newtype 三形态各出
/// `Serialize, Deserialize, specta::Type` 三项且落在固定相对序位；头注释后出
/// `use serde::{Deserialize, Serialize};`（非限定名，`serde` 行在前）。
#[test]
fn render的三形态出wire面() {
    let types = r#"{
  "Item": {"type": "object", "properties": {"f": {"type": "string"}}},
  "Mode": {"oneOf": [{"const": "a"}]},
  "Count": {"type": "integer"}
}"#;
    let text = render_types(types);
    for name in ["Item", "Mode", "Count"] {
        let segment = type_segment(&text, name);
        for item in ["Serialize", "Deserialize", "specta::Type"] {
            assert!(
                segment.contains(item),
                "`{name}` MUST 带 `{item}`（三项同进同出）：\n{segment}"
            );
        }
        let derive = segment
            .lines()
            .find(|line| line.starts_with("#[derive("))
            .expect("derive 行在案");
        assert!(
            derive.find("Serialize") < derive.find("Deserialize")
                && derive.find("Deserialize") < derive.find("specta::Type"),
            "`{name}` 三项相对序固定：{derive}"
        );
    }
    assert!(
        text.contains("use serde::{Deserialize, Serialize};\n"),
        "wire 面在场 ⇒ 非限定导入于头注释之后：\n{text}"
    );
}

/// 异常（AC-8 / Q-3）：含 `x-function` 字段的 struct ⇒ 零 wire 面（无 JSON 线形态），字段仍
/// 出 `fn(…)` 指针形；带负载 union ⇒ 零 wire 面 + 零 `Eq` / `Copy` / `Hash`、零 panic。
#[test]
fn render的函数字段与带负载union零wire面() {
    let function_struct = render_types(
        r#"{"Hooks": {"type": "object", "properties": {"run": {"x-function": {"params": [{"name": "text", "schema": {"type": "string"}}], "returns": {"type": "boolean"}}}}, "required": ["run"]}}"#,
    );
    let segment = type_segment(&function_struct, "Hooks");
    assert!(
        segment.contains("    pub run: fn(text: &str) -> bool,"),
        "函数字段仍出 `fn(…)` 指针形：\n{segment}"
    );
    for absent in ["Serialize", "Deserialize", "specta::Type", "serde"] {
        assert!(
            !segment.contains(absent),
            "含 `x-function` 字段 ⇒ 零 wire 面（`{absent}` 缺席）：\n{segment}"
        );
    }
    assert!(
        !function_struct.contains("use serde::"),
        "零 wire 面 ⇒ 零 `serde` 导入：\n{function_struct}"
    );

    let payload_union = render_types(
        r##"{"Choice": {"oneOf": [{"$ref": "#/$defs/A"}]}, "A": {"type": "object", "properties": {"f": {"type": "string"}}}}"##,
    );
    let segment = type_segment(&payload_union, "Choice");
    for absent in ["Serialize", ", Eq", "Copy", "Hash"] {
        assert!(
            !segment.contains(absent),
            "带负载 union MUST NOT 出 `{absent}`：\n{segment}"
        );
    }
}

/// 边界（AC-1 / AC-8）：零字段 `object` ⇒ derive 逐字
/// `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`（零 `Hash` / 零 `Default`）；产物含
/// `BTreeMap<` ⇒ 出 `use std::collections::BTreeMap;`（`serde` 行在前、`std` 行在后）；两
/// 导入皆无 ⇒ 零字节。
#[test]
fn render的条件import与零字段派生() {
    let zero_field = render_types(r#"{"LocalArchiCheck": {"type": "object", "properties": {}}}"#);
    let segment = type_segment(&zero_field, "LocalArchiCheck");
    assert_eq!(
        segment, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct LocalArchiCheck;",
        "零字段形态逐字（零 `Hash` / 零 `Default` / 零 wire 面）：\n{zero_field}"
    );
    assert!(
        !zero_field.contains("use "),
        "两导入皆无 ⇒ 零字节（AC-1 零 diff 前提）：\n{zero_field}"
    );

    let both = render_types(
        r#"{"Holder": {"type": "object", "properties": {"map": {"type": "object", "additionalProperties": {"type": "integer"}}}, "required": ["map"]}}"#,
    );
    let serde_line = both.find("use serde::{Deserialize, Serialize};");
    let map_line = both.find("use std::collections::BTreeMap;");
    assert!(
        serde_line.is_some() && map_line.is_some(),
        "两个条件导入同样在场：\n{both}"
    );
    assert!(
        serde_line.unwrap() < map_line.unwrap(),
        "`serde` 行在前、`std` 行在后：\n{both}"
    );
    assert!(
        both.contains("pub map: BTreeMap<String, i64>,"),
        "`BTreeMap<` 是 `std` 导入的判据来源：\n{both}"
    );

    let only_map = render_types(
        r#"{"Holder": {"type": "object", "properties": {"map": {"type": "object", "additionalProperties": {"type": "integer"}}, "run": {"x-function": {}}}, "required": ["map", "run"]}}"#,
    );
    assert!(
        only_map.contains("use std::collections::BTreeMap;") && !only_map.contains("use serde::"),
        "零 wire 面但含 `BTreeMap<` ⇒ 只出 `std` 导入：\n{only_map}"
    );
}

// ---------------------------------------------------------------------------
// facet：五形态逐项相等与环容忍（AC-9）
// ---------------------------------------------------------------------------

/// 正向（AC-9）：表驱动五形态逐项对拍——derive 集合与相对序（`Debug, Clone, Copy, Default,
/// PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type` 子序）逐项相等。
#[test]
fn render的五形态facet逐项相等() {
    let cases = [
        (
            "零字段 object",
            r#"{"T": {"type": "object", "properties": {}}}"#,
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        ),
        (
            "封闭枚举",
            r#"{"T": {"oneOf": [{"const": "a"}]}}"#,
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]",
        ),
        (
            "有字段 struct",
            r#"{"T": {"type": "object", "properties": {"f": {"type": "string"}}}}"#,
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]",
        ),
        (
            "标量 newtype",
            r#"{"T": {"type": "string"}}"#,
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]",
        ),
        (
            "带负载 union",
            r##"{"T": {"oneOf": [{"$ref": "#/$defs/A"}]}, "A": {"type": "object", "properties": {}}}"##,
            "#[derive(Debug, Clone, PartialEq)]",
        ),
    ];
    for (label, types, expected) in cases {
        let text = render_types(types);
        let segment = type_segment(&text, "T");
        let derive: Vec<&str> = segment
            .lines()
            .filter(|line| line.starts_with("#[derive"))
            .collect();
        assert_eq!(
            derive
                .first()
                .map(|line| line.trim_end())
                .filter(|_| derive.len() == 1),
            Some(expected),
            "{label}：derive 集合与序固定（期望 `{expected}`）：\n{segment}"
        );
    }
}

/// 异常（AC-9）：含 `number` 字段（`f64`）⇒ 无 `Eq`；含 `x-function` 字段 ⇒ 无 `Eq` 且零
/// wire 面；字段走映射规则（`common.json` 的 `Outcome`）⇒ 无 `Eq`；`$ref` 不可解析 ⇒ 无
/// `Eq`（保守、零 `Err`）。
#[test]
fn render的facet保守不推eq() {
    let cases = [
        (
            "number 字段（f64）",
            r#"{"T": {"type": "object", "properties": {"f": {"type": "number"}}}}"#,
            "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]",
        ),
        (
            "x-function 字段",
            r#"{"T": {"type": "object", "properties": {"f": {"x-function": {}}}}}"#,
            "#[derive(Debug, Clone, PartialEq)]",
        ),
        (
            "映射规则字段（Outcome）",
            r#"{"T": {"type": "object", "properties": {"f": {"outcome": {"type": "string"}}}}}"#,
            "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]",
        ),
        (
            "$ref 不可解析",
            r##"{"T": {"type": "object", "properties": {"f": {"$ref": "#/$defs/Ghost"}}}}"##,
            "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]",
        ),
    ];
    for (label, types, expected) in cases {
        let text = render_types(types);
        let segment = type_segment(&text, "T");
        let derive = segment
            .lines()
            .find(|line| line.starts_with("#[derive("))
            .unwrap_or_else(|| panic!("{label}：derive 行在案\n{segment}"));
        assert_eq!(
            derive.trim_end(),
            expected,
            "{label}：`Eq` 保守缺席（不假设可 `Eq`）：\n{segment}"
        );
    }
}

/// 边界（AC-9）：自引用 `$ref` 环（T 的字段 `$ref` 到 T）⇒ 无 `Copy`、有限步返回、零栈溢出
/// 零 panic；互引环（A→B→A）同判；同一 schema 两次 `render` 逐字相等（纯函数、零读盘）；
/// `$ref` 指向解析表缺席的目标与指向环的两形态不混淆。
#[test]
fn render的facet环容忍与纯函数性() {
    let self_cycle = r##"{"T": {"type": "object", "properties": {"next": {"$ref": "#/$defs/T"}}, "required": ["next"]}}"##;
    let text = render_types(self_cycle);
    let segment = type_segment(&text, "T");
    assert!(
        !segment.contains(", Eq") && !segment.contains("Copy"),
        "自引用环 ⇒ 无 `Eq` / 无 `Copy`（不 panic、不死循环）：\n{segment}"
    );
    assert_eq!(
        text,
        render_types(self_cycle),
        "同一 schema 两次渲染逐字相等（facet 是 schema 的纯函数）"
    );

    let mutual = r##"{
  "A": {"type": "object", "properties": {"b": {"$ref": "#/$defs/B"}}, "required": ["b"]},
  "B": {"type": "object", "properties": {"a": {"$ref": "#/$defs/A"}}, "required": ["a"]}
}"##;
    let mutual_text = render_types(mutual);
    for name in ["A", "B"] {
        let segment = type_segment(&mutual_text, name);
        assert!(
            !segment.contains(", Eq"),
            "互引环 `{name}` ⇒ 无 `Eq`（环上的假设是 `false`）：\n{segment}"
        );
    }
    assert_eq!(
        mutual_text,
        render_types(mutual),
        "互引环两次渲染逐字相等（有限步返回）"
    );

    // 两形态不混淆：环（有目标）与缺席（无目标）都保守不推 `Eq`，但都不是错误
    let absent = render_types(
        r##"{"T": {"type": "object", "properties": {"next": {"$ref": "#/$defs/Ghost"}}, "required": ["next"]}}"##,
    );
    assert!(
        !type_segment(&absent, "T").contains(", Eq"),
        "索引表缺席的目标同样保守不推：\n{absent}"
    );
    assert_ne!(
        type_segment(&absent, "T"),
        type_segment(&text, "T"),
        "两条路径的产物可区分（字段类型文本不同，成因不混同）"
    );
}

// ---------------------------------------------------------------------------
// 旧形态退役的源码自证（AC-3 / AC-4）
// ---------------------------------------------------------------------------

/// 异常（AC-4）：`enum` 数组形态的渲染支已整体删除——`enum_block` **零命中**、
/// `type_block` 内零 `"enum"` 触发支（两渲染器同判）；签名见证与错误族持衡。
#[test]
fn render的enum数组渲染支已退役() {
    let source = include_str!("rust.rs");
    assert_eq!(
        source.matches("enum_block").count(),
        0,
        "`enum_block` 零命中（函数已删）：\n"
    );
    let type_body = function_body(source, "fn type_block(");
    assert!(
        !type_body.contains("\"enum\""),
        "`type_block` 内零 `\"enum\"` 数组触发支：\n{type_body}"
    );
    assert!(
        source.matches("ApiError::Element").count() == 0,
        "渲染器零 `ApiError::Element` 构造（错误族持衡）"
    );

    let emit_mod = include_str!("mod.rs");
    assert_eq!(
        emit_mod.matches("ApiError::Element").count(),
        0,
        "分派器同样零 `ApiError::Element` 构造"
    );
    assert_eq!(
        emit_mod.matches("enum_block").count(),
        0,
        "`emit/mod.rs` 零 `enum_block` 引用"
    );

    // 见证形态逐字不变（R-6 禁令：MUST NOT 顺手改接收器位）
    let witness = function_body(source, "fn op_witness(");
    assert!(
        witness.contains("super::{owner}::{}") && witness.contains("\"super::{}\""),
        "`op_witness` 的 owner / 自由函数两臂逐字不变：\n{witness}"
    );
}
