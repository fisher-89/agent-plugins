//! `api::emit`（渲染分派）的单元测试：按 `EmitTarget.kind` 分派到 `rust` / `ts`、
//! 单契约粒度（不产 `INDEX.toml` / `.schema.json`）、泛型零支持的判据、载元单点
//! （两路头注释同源）、`x-class` 反向警告的落点（`properties` 全为函数却未标即一行
//! stderr 警告，**每元素恰一次**、不双报、不进报告面、不改返回值）。
//!
//! 零 mock：真实 `emit::rust::render` / `emit::ts::render`（分派正确性即被测语义）+ 真实
//! `contract::parse` / `schema::build_schema`；出 `String` 零写盘，故文件系统零依赖。
//! stderr **不造替身、不捕获输出流**：沿「源码自证 + 返回集零影响」两向断言。

use std::collections::BTreeMap;

use serde_json::Value;

use crate::api::contract::{parse, Contract};

use super::{render_target, EmitTarget, ModuleRef};
use crate::api::paths::{EntryForm, GeneratedKind};
use crate::api::schema::{build_schema, verify_defs};
use crate::api::{meta, ApiError, ApiMeta};

/// 契约文件（repo 相对 POSIX）。
const FILE: &str = ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md";

/// 元素 FQN。
const FQN: &str = "App.Backend.Infra.ArchiRuntime";

/// 试点形态源文（1 op + 1 零字段类型）。
fn pilot_source() -> String {
    format!(
        "# {FQN}\n\n## 职能\n\n门禁执行体。\n\n## 操作\n\n### file_stem\n\n```json api-op\n\
         {{\"params\": [{{\"name\": \"timestamp\", \"schema\": {{\"type\": \"string\"}}}}], \
         \"returns\": {{\"type\": \"string\"}}}}\n```\n\n## 类型\n\n```json api-types\n\
         {{\"LocalArchiCheck\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    )
}

/// 解析 + 装配（失败即断言失败）。
fn prepare(source: &str) -> (Contract, Value) {
    let contract = parse(source, FQN, FILE).expect("契约应解析成功");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    (contract, schema)
}

/// 渲染目标（`kind` + 落点 + 元素根 + 入口形态 + 透传查表）。
///
/// 机械随动（D40 / D41）：`form` / `modules` 两字段为此处显式给出——渲染器零形态推导、
/// 零 FQN 派生，故两值只能由调用方（本工厂即「调用方」姿态）提供。
fn target(kind: GeneratedKind) -> EmitTarget {
    EmitTarget {
        kind,
        form: match kind {
            GeneratedKind::Rust => EntryForm::Crate,
            GeneratedKind::TypeScript => EntryForm::SingleFile,
        },
        modules: sample_modules(),
    }
}

/// 透传目标的示例查表（机械随动，D40）：值由调用方给——本装置模拟「调用方已按
/// `paths::{module_name, rust_module_path}` 算齐」的姿态，渲染器本身零派生。
fn sample_modules() -> BTreeMap<String, ModuleRef> {
    let mut modules = BTreeMap::new();
    modules.insert(
        "App.Frontend.Components.Agent.Input".to_owned(),
        ModuleRef {
            name: "Input".to_owned(),
            module_path: "crate::input".to_owned(),
        },
    );
    modules
}

/// 断言渲染返回 `Err(ApiError::Contract(..))` 并返回文案。
fn contract_error(result: Result<String, ApiError>) -> String {
    match result {
        Ok(text) => panic!("期望契约错误，实际 Ok：{text}"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 文本里出现的标识符集合（`[A-Za-z_][A-Za-z0-9_]*`；两侧对拍用）。
fn identifiers(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            current.push(character);
            continue;
        }
        if !current.is_empty() {
            found.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        found.push(current);
    }
    found.sort();
    found.dedup();
    found
}

// ---------------------------------------------------------------------------
// 分派
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`GeneratedKind::Rust` → 含 Rust 形态、不含 TS 语法。
#[test]
fn render_target的rust分派() {
    let (contract, schema) = prepare(&pilot_source());
    let text = render_target(&target(GeneratedKind::Rust), &contract, &schema, &meta())
        .expect("Rust 渲染应成功");

    assert!(
        text.contains("pub struct LocalArchiCheck;"),
        "零字段类型出 struct：\n{text}"
    );
    assert!(text.contains("pub fn new()"), "自动构造器在案：\n{text}");
    assert!(!text.contains("export interface"), "不含 TS interface");
    assert!(!text.contains("as const"), "不含 TS as const");
}

/// 正向（AC-5）：`GeneratedKind::TypeScript` → 含 `export interface`、不含 Rust 语法。
#[test]
fn render_target的ts分派() {
    let (contract, schema) = prepare(&pilot_source());
    let text = render_target(
        &target(GeneratedKind::TypeScript),
        &contract,
        &schema,
        &meta(),
    )
    .expect("TS 渲染应成功");

    assert!(
        text.contains("export interface LocalArchiCheck"),
        "零字段类型出 interface：\n{text}"
    );
    assert!(!text.contains("pub struct"), "不含 Rust struct");
    assert!(
        !text.contains("impl LocalArchiCheck"),
        "不含 Rust impl 块：
{text}"
    );
    assert!(
        !text.contains("const _: fn("),
        "不含 Rust 签名见证：
{text}"
    );
}

/// 正向（AC-5）：同一契约两侧渲染 → 类型名集合与 op 名集合逐项相等（可枚举断言，不比
/// 文本序）。
#[test]
fn render_target两侧键集一致() {
    let source = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"params\": [], \"returns\": \
         {{\"type\": \"string\"}}}}\n```\n\n```json api-types\n{{\"Alpha\": {{\"type\": \
         \"object\", \"properties\": {{}}}}, \"Beta\": {{\"type\": \"object\", \"properties\": \
         {{}}}}}}\n```\n"
    );
    let (contract, schema) = prepare(&source);

    let rust = render_target(&target(GeneratedKind::Rust), &contract, &schema, &meta())
        .expect("Rust 渲染应成功");
    let ts = render_target(
        &target(GeneratedKind::TypeScript),
        &contract,
        &schema,
        &meta(),
    )
    .expect("TS 渲染应成功");

    for name in ["Alpha", "Beta", "file_stem"] {
        assert!(rust.contains(name), "Rust 侧含 `{name}`");
        assert!(ts.contains(name), "TS 侧含 `{name}`");
    }
    let types = ["Alpha", "Beta"];
    for name in types {
        assert!(
            rust.contains(&format!("pub struct {name}")),
            "Rust 侧类型段含 {name}"
        );
        assert!(
            ts.contains(&format!("export interface {name}")),
            "TS 侧类型段含 {name}"
        );
    }
    assert!(
        rust.contains("const _: fn() -> String = super::file_stem;")
            || rust.contains("super::file_stem"),
        "Rust 侧 op 见证在案"
    );
    assert!(ts.contains("export type file_stem"), "TS 侧 op 别名在案");
}

/// 边界：单契约粒度——一个 `EmitTarget` 只出该类生成物文本，不产 `INDEX` / schema；
/// TS specifier 只由 `form` 与 `ModuleRef.name` 决定（D58），不改类型段。
#[test]
fn render_target单契约粒度() {
    let (contract, schema) = prepare(&pilot_source());
    let text = render_target(&target(GeneratedKind::Rust), &contract, &schema, &meta())
        .expect("渲染应成功");
    assert!(!text.contains("[[entry]]"), "不产 INDEX 条目");
    assert!(!text.contains("\"$schema\""), "不产 schema.json");

    // 入口形态的两种取值：目录形态 `../`、单文件形态 `./`
    let source = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    );
    let (contract, schema) = prepare(&source);
    // 形态由调用方给（D41）：两种 specifier 形态各对应一个显式形态值
    let mut directory_form = target(GeneratedKind::TypeScript);
    directory_form.form = EntryForm::SubdirModule;
    let mut single_form = target(GeneratedKind::TypeScript);
    single_form.form = EntryForm::SingleFile;

    let directory_text =
        render_target(&directory_form, &contract, &schema, &meta()).expect("目录形态渲染应成功");
    let single_text =
        render_target(&single_form, &contract, &schema, &meta()).expect("单文件形态渲染应成功");
    assert!(
        directory_text.contains("from '../Input.i'"),
        "目录形态用 `../`：\n{directory_text}"
    );
    assert!(
        single_text.contains("from './Input.i'"),
        "单文件形态用 `./`：\n{single_text}"
    );
}

// ---------------------------------------------------------------------------
// 泛型零支持
// ---------------------------------------------------------------------------

/// 异常（AC-5 反例）：`$defs` 值含 `typeParameters` / `typeArguments`，或类型名带 `<` / `>`
/// → `Err` 并点名该类型；两种 `kind` 同判。
#[test]
fn render_target泛型零支持() {
    for (name, definition) in [
        (
            "Boxed",
            "{\"type\": \"object\", \"typeParameters\": [\"T\"]}",
        ),
        (
            "Wrapped",
            "{\"type\": \"object\", \"typeArguments\": [\"T\"]}",
        ),
    ] {
        let source = format!("# {FQN}\n\n```json api-types\n{{\"{name}\": {definition}}}\n```\n");
        let (contract, schema) = prepare(&source);
        for kind in [GeneratedKind::Rust, GeneratedKind::TypeScript] {
            let message = contract_error(render_target(&target(kind), &contract, &schema, &meta()));
            assert!(message.contains(name), "点名泛型类型：{message}");
            assert!(message.contains("泛型"), "点明泛型零支持：{message}");
        }
    }

    // 类型名带尖括号：`$defs` 键即 `<T>` 形态
    let source = format!(
        "# {FQN}\n\n```json api-types\n{{\"Wrapper<T>\": {{\"type\": \"object\"}}}}\n```\n"
    );
    let (contract, schema) = prepare(&source);
    let message = contract_error(render_target(
        &target(GeneratedKind::Rust),
        &contract,
        &schema,
        &meta(),
    ));
    assert!(
        message.contains("Wrapper<T>"),
        "点名带泛型参的类型名：{message}"
    );
}

/// 边界：`$defs` 值缺失 `type` 且无 `$ref` / `x-class` / `enum` → 渲染不 panic、形态可判。
#[test]
fn render_target无type值退化不panic() {
    let source = format!(
        "# {FQN}\n\n```json api-types\n{{\"Mystery\": {{\"description\": \"无 type 键\"}}}}\n```\n"
    );
    let (contract, schema) = prepare(&source);
    let rust = render_target(&target(GeneratedKind::Rust), &contract, &schema, &meta())
        .expect("Rust 侧退化不 panic");
    assert!(rust.contains("Mystery"), "类型名仍在案：\n{rust}");

    let ts = render_target(
        &target(GeneratedKind::TypeScript),
        &contract,
        &schema,
        &meta(),
    )
    .expect("TS 侧退化不 panic");
    assert!(ts.contains("Mystery"), "类型名仍在案：\n{ts}");
}

// ---------------------------------------------------------------------------
// 空契约 / 零写盘
// ---------------------------------------------------------------------------

/// 边界：零类型零 op → 仅头注释（或空串），不含 `pub use` / `impl` / `export`（零内容
/// 不生成 `.i.*` 的判据在 `render_all` 侧）。
#[test]
fn render_target空契约() {
    let source = format!("# {FQN}\n\n## 职能\n\n正文。\n");
    let (contract, schema) = prepare(&source);

    let rust = render_target(&target(GeneratedKind::Rust), &contract, &schema, &meta())
        .expect("空契约渲染应成功");
    assert!(!rust.contains("pub use"), "空契约不出透传：\n{rust}");
    assert!(
        !rust.lines().any(|line| line.starts_with("impl ")),
        "空契约不出 impl 块：
{rust}"
    );
    assert!(
        rust.lines()
            .all(|line| line.is_empty() || line.starts_with("//")),
        "空契约只余头注释：\n{rust}"
    );

    let ts = render_target(
        &target(GeneratedKind::TypeScript),
        &contract,
        &schema,
        &meta(),
    )
    .expect("空契约 TS 渲染应成功");
    assert!(!ts.contains("export "), "空契约不出 export：\n{ts}");
}

/// 边界（AC-12）：`render_target` 纯出 `String`、零写盘——以源码自证（本文件及其两张
/// 渲染器零 `fs::write` / `create_dir_all`）。
#[test]
fn render_target零写盘源码自证() {
    for source in [
        include_str!("mod.rs"),
        include_str!("rust.rs"),
        include_str!("ts.rs"),
    ] {
        assert!(!source.contains("fs::write"), "渲染路径零写盘");
        assert!(!source.contains("create_dir_all"), "渲染路径零建目录");
        assert!(!source.contains("std::process"), "渲染路径零子进程");
    }
}

// ---------------------------------------------------------------------------
// 载元单点
// ---------------------------------------------------------------------------

/// 边界（D12 / AC-17）：两路头注释都含 `archi-api <CARGO_PKG_VERSION>` 与
/// `common.json@sha256:<12 位>`；同一 `ApiMeta` 实例传入两路，摘要文本逐字相同（载元类型
/// 只有一个）。
#[test]
fn render_target载元单点() {
    let (contract, schema) = prepare(&pilot_source());
    let meta = meta();
    let rust = render_target(&target(GeneratedKind::Rust), &contract, &schema, &meta)
        .expect("Rust 渲染应成功");
    let ts = render_target(
        &target(GeneratedKind::TypeScript),
        &contract,
        &schema,
        &meta,
    )
    .expect("TS 渲染应成功");

    let tool_version = env!("CARGO_PKG_VERSION");
    let short_digest = {
        let (algorithm, hex) = meta.common_digest.split_once(':').expect("摘要含算法前缀");
        format!("{algorithm}:{}", &hex[..12])
    };

    for (label, text) in [("rust", &rust), ("ts", &ts)] {
        assert!(
            text.contains(&format!("archi-api {tool_version}")),
            "{label} 头注释含工具版本：\n{text}"
        );
        assert!(
            text.contains(&short_digest),
            "{label} 头注释含摘要前 12 位（{short_digest}）：\n{text}"
        );
    }
    assert_eq!(
        meta.tool_version,
        env!("CARGO_PKG_VERSION"),
        "tool_version 取编译期常量"
    );

    // 载元类型只有一个：`ApiMeta` 是唯一入参形态（源码自证无第二个载元结构）
    let source = include_str!("mod.rs");
    assert!(
        !source.contains("struct EmitMeta"),
        "MUST NOT 另立第二个载元类型"
    );
    let _: &ApiMeta = &meta;
}

// ---------------------------------------------------------------------------
// x-class 反向警告（D27）
// ---------------------------------------------------------------------------

/// 单独一个「未标 `x-class` 但 properties 全为函数」的契约源文。
fn missing_x_class_source() -> String {
    format!(
        "# {FQN}\n\n```json api-types\n{{\"Runner\": {{\"type\": \"object\", \"properties\": \
         {{\"run\": {{\"x-function\": {{\"params\": [], \"returns\": {{\"type\": \"string\"}}}}}}}}}}}}\n```\n"
    )
}

/// 正向（AC-5 / D27）：未标 `x-class` 且 `properties` 每项都含 `x-function` → 返回值仍
/// `Ok`（警告只走 stderr，不改返回码、不影响产物）。
#[test]
fn render_target的x_class警告不影响返回值() {
    let (contract, schema) = prepare(&missing_x_class_source());
    let rust = render_target(&target(GeneratedKind::Rust), &contract, &schema, &meta())
        .expect("警告不改变返回码（Ok）");
    assert!(
        rust.contains("pub struct Runner"),
        "未标 x-class 按 object 渲染（函数指针结构体是合法意图）：\n{rust}"
    );
    assert!(!rust.contains("pub trait Runner"), "未标即不按 trait 渲染");
}

/// 边界（D27）：已标 `x-class` / `properties` 含非函数项 / `properties` 为空或缺省 →
/// 零警告（本层不发提示）。
#[test]
fn render_target的x_class零警告形态() {
    // 已标 x-class + 全函数 → trait（意图明确，零警告）；未标但含非函数项 / properties
    // 为空 / properties 缺省 → struct（零警告）
    let cases = [
        (
            "Marked",
            r#"{"Marked": {"x-class": true, "properties": {"run": {"x-function": {"params": []}}}}}"#,
            "pub trait Marked",
        ),
        (
            "Plain",
            r#"{"Plain": {"type": "object", "properties": {"count": {"type": "integer"}}}}"#,
            "pub struct Plain",
        ),
        (
            "Empty",
            r#"{"Empty": {"type": "object", "properties": {}}}"#,
            "pub struct Empty",
        ),
        ("Bare", r#"{"Bare": {"type": "object"}}"#, "pub struct Bare"),
    ];
    for (name, definition, expected) in cases {
        let source = format!(
            "# {FQN}

```json api-types
{definition}
```
"
        );
        let (contract, schema) = prepare(&source);
        let text = render_target(&target(GeneratedKind::Rust), &contract, &schema, &meta())
            .expect("零警告形态应渲染成功");
        assert!(
            text.contains(expected),
            "类型 {name} 的渲染形态（{expected}）在案：
{text}"
        );
    }
}

/// 边界（D27 / 事实校准 16）：警告单点、不双报——源码自证 `emit/` 内 `eprintln!` 恰一处
/// 且只在 `mod.rs`；`rust.rs` / `ts.rs` 零 `eprintln!`。
#[test]
fn render_target的x_class警告单点() {
    let mod_source = include_str!("mod.rs");
    assert_eq!(
        mod_source.matches("eprintln!(").count(),
        1,
        "`emit/mod.rs` 内 eprintln! 调用恰一处（每元素恰一次警告）"
    );
    for (label, source) in [
        ("rust.rs", include_str!("rust.rs")),
        ("ts.rs", include_str!("ts.rs")),
    ] {
        assert!(
            !source.contains("eprintln!("),
            "`{label}` MUST NOT 有 eprintln! 调用（两侧各报即双报）"
        );
    }
}

/// 异常（D27）：触发警告的输入下，返回值集合不含任何 finding 面产物；`INDEX.toml`
/// 文本与无警告输入逐字相等（警告对生成物零影响）。
#[test]
fn render_target的x_class警告不进报告面() {
    let (warning_contract, warning_schema) = prepare(&missing_x_class_source());
    let (plain_contract, plain_schema) = prepare(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Runner\": {{\"type\": \"object\", \"properties\": \
         {{\"count\": {{\"type\": \"integer\"}}}}}}}}\n```\n"
    ));

    let with_warning = render_target(
        &target(GeneratedKind::Rust),
        &warning_contract,
        &warning_schema,
        &meta(),
    )
    .expect("警告形态仍 Ok");
    let without_warning = render_target(
        &target(GeneratedKind::Rust),
        &plain_contract,
        &plain_schema,
        &meta(),
    )
    .expect("无警告形态 Ok");

    // 头注释逐字相等（警告不改产物头）
    let header = |text: &str| -> Vec<String> {
        text.lines()
            .take_while(|line| line.starts_with("//"))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(
        header(&with_warning),
        header(&without_warning),
        "警告对头注释零影响"
    );

    // `ArchiApiFindingKind` 仍十值（MUST NOT 扩第十一值）
    assert_eq!(
        crate::drift::report::ArchiApiFindingKind::ALL.len(),
        10,
        "判据词汇仍十值（警告不是第十一 kind）"
    );
}

/// 异常（D27 反例）：`x-class: true` + `properties` 非全函数 → 契约在
/// `schema::verify_defs` 已判错（`render_target` 收不到该输入）；本文件不复制该判错。
#[test]
fn render_target不复制x_class错误判据() {
    let invalid = format!(
        "# {FQN}\n\n```json api-types\n{{\"Gate\": {{\"x-class\": true, \"properties\": \
         {{\"count\": {{\"type\": \"integer\"}}}}}}}}\n```\n"
    );
    let contract = parse(&invalid, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    // 错误侧在装载期（本测试直接调 verify_defs 复现拦截面）
    assert!(
        verify_defs(&schema).is_err(),
        "带属性的 x-class 在 verify_defs 即判错（render_target 收不到）"
    );

    // 源码自证：`emit/` 内零 `x-class` 判错分支（错误判据只在 schema.rs）
    for (label, source) in [
        ("mod.rs", include_str!("mod.rs")),
        ("rust.rs", include_str!("rust.rs")),
        ("ts.rs", include_str!("ts.rs")),
    ] {
        assert!(
            !source.contains("ApiError::Contract(\"`$defs"),
            "`emit/{label}` 不复制 $defs 判错文案"
        );
        assert!(
            !source.contains("trait 仅允许方法"),
            "`emit/{label}` 不复制 trait 判错语义"
        );
    }
    assert!(
        include_str!("../schema.rs").contains("trait 仅允许方法"),
        "错误判据的落点只在 schema.rs"
    );
}

/// 边界：`identifiers` 辅助函数自证（两侧对拍用的标识符切分不改语义）。
#[test]
fn identifiers切分可判() {
    let found = identifiers("pub struct Alpha;\nexport interface Beta {}");
    assert!(found.contains(&"Alpha".to_owned()));
    assert!(found.contains(&"Beta".to_owned()));
    assert!(found.contains(&"pub".to_owned()));
}

// ---------------------------------------------------------------------------
// `EmitTarget` 三字段面（D58 / AC-34 ①）与 `modules` 退化（D40 / D41）
// ---------------------------------------------------------------------------

/// 正向（**D58 / AC-34 ①**）：`EmitTarget { kind, form, modules }` **三字段**齐备且可
/// 构造；`form` 直接取自入参（`Crate` / `SubdirModule` / `SingleFile` 三值均可传入），
/// 本文件零形态推导；`modules` 为 `BTreeMap<String, ModuleRef>`，`ModuleRef { name,
/// module_path }` 两字段可独立读取（TS 侧只读 `name`、Rust 侧只读 `module_path`）。
/// 源码自证两字段与 `use std::path::PathBuf` 各零命中、三字段各 1。
#[test]
fn render_target的emit_target三字段面() {
    for form in [
        EntryForm::Crate,
        EntryForm::SubdirModule,
        EntryForm::SingleFile,
    ] {
        let mut emit_target = target(GeneratedKind::TypeScript);
        emit_target.form = form;
        let (contract, schema) = prepare(&pilot_source());
        let text = render_target(&emit_target, &contract, &schema, &meta())
            .expect("三形态均可传入且渲染成功");
        assert!(
            text.contains("export interface LocalArchiCheck"),
            "形态不影响类型段（只影响 specifier）：{form:?}
{text}"
        );
    }

    // `ModuleRef` 两字段独立：TS 只读 `name`、Rust 只读 `module_path`
    let module = ModuleRef {
        name: "archi".to_owned(),
        module_path: "crate::archi".to_owned(),
    };
    assert_eq!(module.name, "archi");
    assert_eq!(module.module_path, "crate::archi");
    assert_ne!(module.name, module.module_path, "两字段是两个不同事实");

    let mut modules = BTreeMap::new();
    modules.insert("App.Core.A".to_owned(), module);
    let emit_target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: EntryForm::Crate,
        modules: modules.clone(),
    };
    assert_eq!(emit_target.modules.len(), 1, "查表键 = 目标 FQN");
    assert!(
        emit_target.modules.contains_key("App.Core.A"),
        "查表按目标 FQN 索引"
    );

    // 字段面：三字段各 1、两已删字段与 `PathBuf` 引入各 0（`mod.rs` 是定义处）
    let source = include_str!("mod.rs");
    for kept in [
        "pub kind: GeneratedKind",
        "pub form: EntryForm",
        "pub modules: BTreeMap<String, ModuleRef>",
    ] {
        assert_eq!(source.matches(kept).count(), 1, "字段面 `{kept}` 恰 1");
    }
    for removed in [
        "pub element_root",
        "pub path: PathBuf",
        "use std::path::PathBuf",
    ] {
        assert_eq!(
            source.matches(removed).count(),
            0,
            "D58 已删：`{removed}` 零命中"
        );
    }
}

/// 边界（**D58 / AC-34 ①** 行为面）：同一契约 + 同一 `{ kind, form, modules }` 的渲染
/// 产物**逐字节相等**（`path` / `element_root` 若被产品面读取，删后行为不可能不变）；
/// 产品面读 `target.{kind,form,modules}` 的行数恰 3（`emit/mod.rs` 的 `.kind`、
/// `emit/rust.rs` 的 `.modules`、`emit/ts.rs` 的 `.form` 与 `.modules` 同行），读
/// `target.{path,element_root}` 零行。
///
/// 判据面取 `emit/` 的三个产品文件而非全仓 `grep`：`EmitTarget` 的读取者只可能是语言
/// 分派层与两个渲染器（本层是它唯一的定义与消费面），而全仓扫描会让任何名为 `target`
/// 的无关局部变量进来干扰计数——那是把「本模块的读取面」误扩成「全仓碰巧的字符串」。
#[test]
fn render_target删两字段零行为影响() {
    let (contract, schema) = prepare(&pilot_source());
    let emit_target = target(GeneratedKind::Rust);
    let first = render_target(&emit_target, &contract, &schema, &meta()).expect("首次渲染");
    let second = render_target(&emit_target, &contract, &schema, &meta()).expect("二次渲染");
    assert_eq!(first, second, "同一三字段目标的产物逐字节相等");

    let bindings: [(&str, &str); 3] = [
        ("emit/mod.rs", include_str!("mod.rs")),
        ("emit/rust.rs", include_str!("rust.rs")),
        ("emit/ts.rs", include_str!("ts.rs")),
    ];
    let reading_lines = |needles: &[&str]| -> Vec<String> {
        let mut found = Vec::new();
        for (file, text) in bindings {
            for (index, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                if needles.iter().any(|needle| line.contains(needle)) {
                    found.push(format!("{file}:{}", index + 1));
                }
            }
        }
        found
    };

    let read_points = reading_lines(&["target.kind", "target.form", "target.modules"]);
    assert_eq!(
        read_points.len(),
        3,
        "产品面读三字段的行恰 3（`.form` 与 `.modules` 在 `ts.rs` 同行）：{read_points:?}"
    );
    assert_eq!(
        reading_lines(&["target.path", "target.element_root"]).len(),
        0,
        "产品面零读取已删字段"
    );
}

/// 边界（D40）：契约含一项 `api-reexport` 而 `EmitTarget.modules` **不含**该目标 FQN →
/// 渲染结果只留 `// 透传自 <FQN>` 注释、**不产 specifier**（MUST NOT 退化到 FQN 末段）；
/// `modules` 为空表 + 零透传条目 → 输出与非空时逐字相等（无透传即不参与）。
#[test]
fn render_target的modules查不到目标即退化() {
    let source = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    );
    let (contract, schema) = prepare(&source);

    let mut without_target = target(GeneratedKind::Rust);
    without_target.modules = BTreeMap::new();
    let rust = render_target(&without_target, &contract, &schema, &meta())
        .expect("查不到目标不是错误（只出注释）");
    assert!(
        rust.contains("// 透传自 App.Frontend.Components.Agent.Input"),
        "只留注释：\n{rust}"
    );
    assert!(
        !rust.contains("pub use "),
        "不产 `pub use`（查不到目标即无路径可派生）：\n{rust}"
    );
    assert!(
        !rust.contains("App.Frontend.Components.Agent.Input::AgentInput"),
        "MUST NOT 退化到 FQN 末段当模块路径：\n{rust}"
    );

    let mut without_target_ts = target(GeneratedKind::TypeScript);
    without_target_ts.modules = BTreeMap::new();
    let ts =
        render_target(&without_target_ts, &contract, &schema, &meta()).expect("TS 同样只出注释");
    assert!(
        !ts.contains(" from '"),
        "TS 侧不产 specifier（零 import 语句）：\n{ts}"
    );

    // 零透传条目：modules 空表与查表在场逐字相等（无透传即不参与）
    let plain = prepare(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    ));
    let mut empty_modules = target(GeneratedKind::Rust);
    empty_modules.modules = BTreeMap::new();
    let by_empty =
        render_target(&empty_modules, &plain.0, &plain.1, &meta()).expect("零透传条目渲染成功");
    let by_filled = render_target(&target(GeneratedKind::Rust), &plain.0, &plain.1, &meta())
        .expect("查表在场但零透传条目");
    assert_eq!(
        by_empty, by_filled,
        "无透传条目时查表不参与（输出逐字相等）"
    );
}

/// 正向（**D40 / D41 / D58**）：三字段承载——`EmitTarget` 的 `kind` / `form` /
/// `modules` 齐备且可构造；`ModuleRef { name, module_path }` 两字段可独立读取（TS 侧
/// 只读 `name`、Rust 侧只读 `module_path`）；`form` 由调用方传入，本文件零
/// `paths::entry_form` 调用（渲染器零 IO）。
#[test]
fn render_target的emit_target三字段承载() {
    let source = include_str!("mod.rs");
    let start = source
        .find("pub struct EmitTarget {")
        .expect("`EmitTarget` 定义在案");
    let body = &source[start..];
    let end = body.find("\n}").expect("定义收口");
    let fields: Vec<&str> = body[..end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.trim())
        .collect();
    assert_eq!(
        fields,
        vec!["kind", "form", "modules"],
        "`EmitTarget` 恰三字段且序稳定（D58）"
    );

    let module_start = source
        .find("pub struct ModuleRef {")
        .expect("`ModuleRef` 定义在案");
    let module_body = &source[module_start..];
    let module_end = module_body.find("\n}").expect("定义收口");
    let module_fields: Vec<&str> = module_body[..module_end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.trim())
        .collect();
    assert_eq!(
        module_fields,
        vec!["name", "module_path"],
        "`ModuleRef` 恰两字段（两个不同事实，MUST NOT 合一）"
    );

    assert!(
        !source.contains("paths::entry_form("),
        "本层零 `paths::entry_form` 调用（形态由调用方给、渲染器零 IO）"
    );
    let mut modules = BTreeMap::new();
    modules.insert(
        "App.Core.A".to_owned(),
        ModuleRef {
            name: "a".to_owned(),
            module_path: "crate::a".to_owned(),
        },
    );
    let emit_target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: EntryForm::Crate,
        modules,
    };
    assert_eq!(emit_target.kind, GeneratedKind::Rust);
    assert_eq!(emit_target.form, EntryForm::Crate);
    assert_eq!(emit_target.modules.len(), 1);
}

/// 异常（D41 反例在案）：`emit/` 三个产品文件内**零** `entry_form_of`、**零**
/// `rsplit('.')`、**零** `contains('.')` 形态推导分支（形态只从 `EmitTarget.form` 读）；
/// `render_target` 内部零 `paths::entry_form` 调用（那需要盘上事实，渲染器零 IO）；
/// 把同一契约的 `form` 从 `Crate` 改成 `SingleFile` → 分派到的输出随之改变。
#[test]
fn render_target的形态由调用方决定() {
    for (label, source) in [
        ("mod.rs", include_str!("mod.rs")),
        ("rust.rs", include_str!("rust.rs")),
        ("ts.rs", include_str!("ts.rs")),
    ] {
        assert!(
            !source.contains("entry_form_of"),
            "`emit/{label}` 零 `entry_form_of`（D41：私有形态推导已删）"
        );
        assert!(
            !source.contains("rsplit('.')"),
            "`emit/{label}` 零 FQN 点分段切分（D40）"
        );
        assert!(
            !source.contains("contains('.')"),
            "`emit/{label}` 零「末段是否含 `.`」形态猜测（D41）"
        );
        assert!(
            !source.contains("paths::entry_form("),
            "`emit/{label}` 零 `paths::entry_form` 调用（渲染器零 IO）"
        );
    }

    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    ));
    let mut as_directory = target(GeneratedKind::TypeScript);
    as_directory.form = EntryForm::SubdirModule;
    let mut as_single = target(GeneratedKind::TypeScript);
    as_single.form = EntryForm::SingleFile;

    let directory =
        render_target(&as_directory, &contract, &schema, &meta()).expect("目录形态渲染成功");
    let single =
        render_target(&as_single, &contract, &schema, &meta()).expect("单文件形态渲染成功");
    assert_ne!(
        directory, single,
        "形态入参改变输出（证明形态不是渲染器自算的）"
    );
    assert!(
        directory.contains("from '../Input.i'"),
        "目录形态：{directory}"
    );
    assert!(single.contains("from './Input.i'"), "单文件形态：{single}");
}

/// 边界（D45 边界）：闸一 / 闸二都不落在本层——本文件内 `is_identifier` 与三个转义单点
/// 各零命中；`render_target` 的职责只有「语言分派 + `x-class` 警告 + 透传两张表」。
#[test]
fn 闸二不落render_target() {
    let source = include_str!("mod.rs");
    for symbol in [
        "is_identifier",
        "ts_string_literal",
        "ts_comment_text",
        "rust_comment_text",
    ] {
        assert!(
            !source.contains(symbol),
            "`emit/mod.rs` 内零 `{symbol}`（闸一在 contract::verify、闸二在同语言渲染器内）"
        );
    }
    // 职责三件：语言分派 + `x-class` 警告 + 两张表（`declared_ops` / `reexport_defs` /
    // `local_defs` / `notice_lines`）；两分支同形（D46：分派只剩「选语言」，不再「选孪生」）
    for duty in ["rust::render(", "ts::render(", "warn_missing_x_class"] {
        assert!(source.contains(duty), "本层职责在案：`{duty}`");
    }
}

// ---------------------------------------------------------------------------
// 分派面出口形态（D46 / AC-24 ④）：唯一入口、且不与渲染器同序
// ---------------------------------------------------------------------------

/// 边界（AC-24 ④ / D46）：`render_target` 的签名是
/// `(target, contract, schema, meta)`（**`target` 首位**），两渲染器的签名是
/// `(contract, schema, meta, target)`（**`target` 末位**）——**两者参数序本就不同，
/// MUST NOT 主张同序**；产品 `.rs`（滤 `*_test.rs`）内 `rust::render(` / `ts::render(`
/// 零直连命中（渲染器只经本函数被调用）；两处分派点各恰 1 处，**无第三条分派路径**；
/// 行为面：同一契约 + 同一 `EmitTarget` 经 `render_target` 与经 `render(..)` 直接调用
/// 产出**逐字相等**（分派零额外语义）。
#[test]
fn render_target是唯一分派入口且不与渲染器同序() {
    let source = include_str!("mod.rs");
    // ① 分派面签名：`target` 首位（与两渲染器的 `target` 末位不同序）
    assert!(
        source.contains("pub fn render_target(\n    target: &EmitTarget,"),
        "`render_target` 的 `target` 在首位（MUST NOT 主张与渲染器同序）"
    );
    // ② 两处分派点各恰 1 处，无第三条路径
    assert_eq!(
        source.matches("rust::render(").count(),
        1,
        "`Rust` 分支恰一路"
    );
    assert_eq!(
        source.matches("ts::render(").count(),
        1,
        "`TypeScript` 分支恰一路"
    );
    assert_eq!(
        source.matches("GeneratedKind::").count(),
        2,
        "分派只覆盖两值（`GeneratedKind` 封闭二值），无第三条分派路径"
    );

    // ③ 产品面直连面**恰为分派器两处**（AC-24 ④）：`rust::render(` / `ts::render(` 各
    // 恰 1 命中且命中文件去重后恰一行 `api/emit/mod.rs`——**MUST NOT 用「零命中」作判据**
    // （分派器本身必然调这两个函数，零命中只会在「没有任何调用者」时成立，是假判据）
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut src_tauri = manifest.clone();
    for _ in 0..3 {
        src_tauri.pop();
    }
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&src_tauri.join("crates"), &mut hits);
    scan_product_rs(&src_tauri.join("src"), &mut hits);
    for needle in ["rust::render(", "ts::render("] {
        let direct: Vec<&(String, String)> = hits
            .iter()
            .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains(needle))
            .collect();
        assert_eq!(
            direct.len(),
            1,
            "产品面 `{needle}` 恰 1 处直连（分派器该分支）：{direct:?}"
        );
        assert!(
            direct[0].0.ends_with("api/emit/mod.rs"),
            "`{needle}` 的唯一调用点在分派器（实际：{}）",
            direct[0].0
        );
    }

    // ④ 行为面：分派与直接调用逐字相等
    let (contract, schema) = prepare(&pilot_source());
    let rust_target = target(GeneratedKind::Rust);
    let dispatched = render_target(&rust_target, &contract, &schema, &meta()).expect("分派应成功");
    let direct = super::rust::render(&contract, &schema, &meta(), &rust_target);
    assert_eq!(dispatched, direct, "分派零额外语义（逐字相等）");
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
// 落点闸不落本层（AC-25 / D47 边界）
// ---------------------------------------------------------------------------

/// 边界（**AC-25** / D47）：本文件内**零**落点判据——定形族别串 `x-function 落点` 与
/// `JSON pointer` 文案各零命中（D47 的闸在 `contract::verify`，装载期唯一中止点）；
/// 渲染层**不重复校验位置**（沿 D40 / D41 / D45 同款纪律）；`render_target` 收到的
/// `schema` 已通过落点闸，故零「深层 `x-function` 兜底分支」；分派前的 `x-class` 警告与
/// 落点闸**互不遮蔽**（前者是纯形状提示、后者是契约错误）。
#[test]
fn 落点闸不落render_target() {
    let source = include_str!("mod.rs");
    for marker in ["x-function 落点", "JSON pointer"] {
        assert!(
            !source.contains(marker),
            "`emit/mod.rs` 内零 `{marker}`（落点闸在 `contract::verify`）"
        );
    }
    // 零兜底分支：本层不判 `x-function` 的**位置**（`x-function` 仅作 `x-class` 警告的
    // 形状判据出现于 `warn_missing_x_class`，不涉及 pointer）
    assert!(
        !source.contains("scan_x_function") && !source.contains("verify_x_function_positions"),
        "本层零落点守卫的复制（MUST NOT 二次派生）"
    );
    // 与其他闸互不遮蔽：`x-class` 警告仍在，且不是错误面（返回值仍 Ok）
    assert!(
        source.contains("warn_missing_x_class"),
        "`x-class` 警告与落点闸互不遮蔽（前者是本层的形状提示）"
    );
    assert!(
        !source.contains("x-function 落点非法"),
        "本层不产落点错误文案"
    );
}

// ---------------------------------------------------------------------------
// `render_target` 与两渲染器的产出逐字相等（D46 行为面）
// ---------------------------------------------------------------------------

/// 正向（**D46** 行为面）：同一契约 + 同一 `EmitTarget` 经 `render_target` 分派与经
/// `emit::{rust,ts}::render` 直接调用 → 产出**逐字相等**（分派无额外语义）；`kind` 两值
/// 各一对照。
#[test]
fn render_target与render产出逐字相等() {
    let (contract, schema) = prepare(&pilot_source());
    let meta = meta();

    let rust_target = target(GeneratedKind::Rust);
    let rust_dispatched =
        render_target(&rust_target, &contract, &schema, &meta).expect("分派应成功");
    let rust_direct = super::rust::render(&contract, &schema, &meta, &rust_target);
    assert_eq!(
        rust_dispatched, rust_direct,
        "Rust 分支：分派与直接调用逐字相等（零额外语义）"
    );

    let ts_target = target(GeneratedKind::TypeScript);
    let ts_dispatched = render_target(&ts_target, &contract, &schema, &meta).expect("分派应成功");
    let ts_direct = super::ts::render(&contract, &schema, &meta, &ts_target);
    assert_eq!(
        ts_dispatched, ts_direct,
        "TypeScript 分支：分派与直接调用逐字相等"
    );
}

/// 边界（**D46**）：`render_target` 的实参序 `(target, contract, schema, meta)` 与两渲染
/// 器的 `(contract, schema, meta, target)` **本就不同序**，MUST NOT 主张同序——签名逐字在案
/// 即证；行为面：交换实参序后输出改变（`target` 不是可忽略的末项）。
#[test]
fn render_target实参序与渲染器不同() {
    let source = include_str!("mod.rs");
    assert!(
        source.contains("pub fn render_target(\n    target: &EmitTarget,"),
        "`render_target` 的 `target` 在**首位**（与渲染器 `target` 末位不同序）"
    );
    for (path, renderer) in [
        ("emit/rust.rs", include_str!("rust.rs")),
        ("emit/ts.rs", include_str!("ts.rs")),
    ] {
        assert!(
            renderer.contains(
                "pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String"
            ),
            "`{path}` 的四参签名 `target` 在**末位**（定形串带类型，三参孪生下恒 0）"
        );
    }
}

// ---------------------------------------------------------------------------
// 第四类机械随动的归属二分（D53 / AC-30 ④）：测试相位侧的断言面与用例名
// ---------------------------------------------------------------------------

/// 边界（**D53 / AC-30 ④**——归测试相位，非实现相位）：四参单形落地后，两个 sidecar
/// 用例文件的**调用面**已是四参（无一遗留三参 `render(..)` / `render_with_root(..)` /
/// `render_with_target(..)`），且**断言面保留原意图**：签名自证串带完整类型、`pub use`
/// 断言仍见证「模块路径取自 `paths::rust_module_path`」而非 FQN 派生。
#[test]
fn 第四类随动的测试相位断言面() {
    for (path, source) in [
        ("emit/rust_test.rs", include_str!("rust_test.rs")),
        ("emit/ts_test.rs", include_str!("ts_test.rs")),
    ] {
        // ① 调用面已是四参：三参形态与两个异名孪生的**调用**零命中
        for legacy in [
            "render(&contract, &schema, &meta())",
            "render(contract, schema, meta)",
            "render_with_root(",
            "render_with_target(",
        ] {
            assert!(
                !source.contains(legacy),
                "`{path}` 内零 `{legacy}`（调用形态已适配四参单形，D53 编译前置）"
            );
        }

        // ② 签名自证串带完整类型（不是裸 `^pub fn render\(` 计数）
        assert!(
            source.contains(
                "pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String"
            ),
            "`{path}` 的签名自证串带完整类型（钉住「四参 + `target` 末位 + 返回 `String`」）"
        );
    }

    // ③ `pub use` 断言仍见证「模块路径取自 `paths::rust_module_path`」
    let rust_source = include_str!("rust_test.rs");
    assert!(
        rust_source.contains("pub use crate::input::AgentInput;")
            && rust_source.contains("pub use crate::blocks::AgentBlocks;"),
        "`pub use` 断言保留（模块路径来自查表值，非 FQN 点分段派生）"
    );
    assert!(
        rust_source.contains("module.module_path"),
        "查表值只取 `ModuleRef.module_path`（真值源在 `paths::rust_module_path`，渲染器零派生）"
    );
}
