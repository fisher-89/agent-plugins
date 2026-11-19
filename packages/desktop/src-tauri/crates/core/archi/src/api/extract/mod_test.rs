//! `api::extract`（取数器分派与签名归一）的单元测试：语言分派（Rust / TS / 未知
//! 语言 → 空 `Vec`）、`normalize_signature` 的归一语义（参数名 / 空白 / 多行 / 异步性
//! 不参与，参数顺序参与）、十一值 `ApiType` 的规范短形与封闭词汇 `Visibility` /
//! `ApiSymbolKind`，以及 `extract_element`（判据唯一取数面）的整元素抽取。
//!
//! 零 mock：真实 `extract::rust` / `extract::ts` 取数器（分派语义即被测对象，替身
//! 会让「语言判错」不可见）；`extract` 零落盘（接 `source: &str`），`extract_element`
//! 用自建 `TempDir` RAII（本 crate 无 dev-dependency）落真实模块树。

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::dto::{ApiStatus, ArchiElement, ElementKind, GateTier, NodeMark};

use serde_json::{json, Value};

use crate::api::contract::parse;
use crate::api::emit::{self, EmitTarget};
use crate::api::meta;
use crate::api::paths::{EntryForm, GeneratedKind};
use crate::api::schema::build_schema;

use super::{
    extract, extract_element, normalize_signature, nullable_breakdown, schema_type, signature_of,
    ApiParam, ApiSymbol, ApiSymbolKind, ApiType, Position, Visibility,
};

/// 元素构造器（`path` 决定语言分派）。
fn element(fqn: &str, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Warn,
        api: ApiStatus::Declared,
        entry: None,
        path: paths.iter().map(|path| (*path).to_owned()).collect(),
        summary: None,
        metadata: BTreeMap::new(),
        model_file: ".dev-team/architecture/graph/model/01-test.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
        doc: None,
    }
}

/// 符号夹具（`Function` 形态的缺省值）。
fn symbol(params: Vec<ApiParam>, returns: ApiType) -> ApiSymbol {
    ApiSymbol {
        name: "op".to_owned(),
        owner: None,
        kind: ApiSymbolKind::Function,
        params,
        returns,
        is_async: false,
        line: 1,
        file: "src/lib.rs".to_owned(),
        visibility: Visibility::External,
    }
}

/// 参数夹具。
fn param(name: &str, ty: ApiType) -> ApiParam {
    ApiParam {
        name: name.to_owned(),
        ty,
        optional: false,
    }
}

// ---------------------------------------------------------------------------
// extract：语言分派
// ---------------------------------------------------------------------------

/// 正向：Rust 元素（`path` 首条 `.rs`）→ 真实 `syn` 抽取，`pub fn` 一条、参数与返回
/// 归一到逻辑类型。
#[test]
fn extract的rust分派() {
    let symbols = extract(
        &element("App.Backend.Core.Archi", &["crates/core/archi/src/lib.rs"]),
        "crates/core/archi/src/lib.rs",
        "pub fn parse(input: &str) -> String {\n    String::new()\n}\n",
    );
    assert_eq!(symbols.len(), 1, "一条 `pub fn`");
    assert_eq!(symbols[0].name, "parse");
    assert_eq!(symbols[0].kind, ApiSymbolKind::Function);
    assert_eq!(symbols[0].params.len(), 1, "一个参数");
    assert_eq!(symbols[0].params[0].ty, ApiType::Str, "入参 &str → Str");
    assert_eq!(
        symbols[0].returns,
        ApiType::OwnedString,
        "返回 String → OwnedString"
    );
    assert!(!symbols[0].is_async, "非异步");
}

/// 正向（AC-5）：TS 元素（`path` 首条 `.tsx`）→ 真实 `oxc` 抽取，`export function`
/// 一条。
#[test]
fn extract的ts分派() {
    let symbols = extract(
        &element("App.Frontend.Views.Welcome", &["src/views/x.tsx"]),
        "src/views/x.tsx",
        "export function load(): string {\n  return '';\n}\n",
    );
    assert_eq!(symbols.len(), 1, "一条 export function");
    assert_eq!(symbols[0].name, "load");
    assert_eq!(symbols[0].kind, ApiSymbolKind::Function);
    assert!(!symbols[0].is_async, "同步函数");
    assert_eq!(symbols[0].returns, ApiType::Str, "返回 string");
}

/// 异常：`README.md` / 无扩展名 / `.py` / `path` 空 → 全空 `Vec`（不猜测、零 panic）。
#[test]
fn extract未知语言返回空集() {
    let cases = [
        (element("App.Docs", &["README.md"]), "README.md"),
        (element("App.NoExt", &["scripts/run"]), "scripts/run"),
        (element("App.Py", &["scripts/x.py"]), "scripts/x.py"),
        (element("App.Container", &[]), ""),
    ];
    for (element, file) in cases {
        let symbols = extract(&element, file, "pub fn ignored() {}");
        assert!(
            symbols.is_empty(),
            "未知语言不猜符号（{file:?}）：{symbols:?}"
        );
    }
}

/// 异常（D19-a 同族：容器元素的 `path` 为空）：`extract` 在 `path = []` 上 MUST NOT
/// panic——`<root>` 取空串（`module_name("")` → `""`），`crate_name` 只作诊断留痕，
/// 抽取面（`pub` 项）与 `path` 非空时逐字相同。
#[test]
fn extract容器元素零path不panic() {
    let source = "pub fn file_stem(timestamp: &str) -> String { String::new() }\n";
    let container = element("App", &[]);
    let symbols = extract(&container, "crates/core/archi/src/lib.rs", source);
    assert!(
        symbols.iter().any(|symbol| symbol.name == "file_stem"),
        "空 path 仍按文件语言抽取：{symbols:?}"
    );

    // 与 path 非空时逐字段相等（crate_name 不参与抽取面）
    let with_path = element("App.Backend.Core.Archi", &["crates/core/archi/"]);
    let expected = extract(&with_path, "crates/core/archi/src/lib.rs", source);
    assert_eq!(symbols, expected, "空 path 与有 path 的抽取面逐字段相等");
}

/// 边界：空源 / 语法错源 / 100 KB 单文件 → 不 panic，返回形态可判。
#[test]
fn extract空源与语法错源() {
    let rust_element = element("App.Backend.Core.Archi", &["crates/core/archi/src/lib.rs"]);
    assert!(
        extract(&rust_element, "crates/core/archi/src/lib.rs", "").is_empty(),
        "空源零符号"
    );

    let broken = extract(&rust_element, "crates/core/archi/src/lib.rs", "pub fn f( {");
    assert!(
        broken.is_empty(),
        "语法错源不猜测（syn 解析失败 → 空集）：{broken:?}"
    );

    let ts_element = element("App.Frontend.Views.Welcome", &["src/views/x.ts"]);
    let broken_ts = extract(&ts_element, "src/views/x.ts", "export function f( {");
    assert!(
        broken_ts.is_empty(),
        "TS 语法错源不猜测（解析 panic 即零符号）：{broken_ts:?}"
    );

    let mut big = String::new();
    for index in 0..2000 {
        big.push_str(&format!(
            "pub fn op{index}(input: &str) -> String {{\n    String::new()\n}}\n"
        ));
    }
    assert!(big.len() > 100_000, "夹具应超 100 KB");
    let symbols = extract(&rust_element, "crates/core/archi/src/lib.rs", &big);
    assert_eq!(symbols.len(), 2000, "大文件不 panic、符号不截断");
}

// ---------------------------------------------------------------------------
// normalize_signature：位置分派后的判等
// ---------------------------------------------------------------------------

/// 正向（AC-5 / D10）：契约侧 `input: string` 与代码侧 `input: &str` 归一后逐字相等；
/// 返回位 `String` / `&str` 的归一等；列表与可选字段的短形可辨。
#[test]
fn normalize_signature四种位置分派后判等() {
    let from_str = symbol(vec![param("input", ApiType::Str)], ApiType::OwnedString);
    let from_owned = symbol(
        vec![param("input", ApiType::OwnedString)],
        ApiType::OwnedString,
    );

    assert_eq!(
        normalize_signature(&from_str),
        "fn(str) -> String",
        "入参 &str → str、返回 String → String"
    );
    assert_ne!(
        normalize_signature(&from_str),
        normalize_signature(&from_owned),
        "入参 str 与 String 是两种形态（位置分派只在入参 / 返回位生效）"
    );

    let list_str = symbol(
        vec![param("items", ApiType::List(Box::new(ApiType::Str)))],
        ApiType::List(Box::new(ApiType::OwnedString)),
    );
    assert_eq!(
        normalize_signature(&list_str),
        "fn(list<str>) -> list<String>",
        "列表形状可比对"
    );

    let optional = symbol(vec![], ApiType::Optional(Box::new(ApiType::OwnedString)));
    assert_eq!(
        normalize_signature(&optional),
        "fn() -> optional<String>",
        "可选字段短形"
    );
}

/// 边界：参数名不参与（同名同型不同名 → 相等）；参数顺序参与（顺序不同 → 不相等）。
#[test]
fn normalize_signature参数名与顺序() {
    let left = symbol(vec![param("a", ApiType::Str)], ApiType::Bool);
    let right = symbol(vec![param("timestamp", ApiType::Str)], ApiType::Bool);
    assert_eq!(
        normalize_signature(&left),
        normalize_signature(&right),
        "参数名不参与归一"
    );

    let ordered = symbol(
        vec![param("a", ApiType::Str), param("b", ApiType::Int)],
        ApiType::Bool,
    );
    let swapped = symbol(
        vec![param("b", ApiType::Int), param("a", ApiType::Str)],
        ApiType::Bool,
    );
    assert_ne!(
        normalize_signature(&ordered),
        normalize_signature(&swapped),
        "参数顺序是签名语义"
    );
}

/// 边界：多行排列与空白差异不影响归一（AST 抽取已归一，等价形态产出同一串）。
#[test]
fn normalize_signature空白与多行() {
    let element = element("App.Backend.Core.Archi", &["crates/core/archi/src/lib.rs"]);
    let single_line = extract(
        &element,
        "crates/core/archi/src/lib.rs",
        "pub fn f(a: &str, b: i64) -> String { String::new() }\n",
    );
    let multi_line = extract(
        &element,
        "crates/core/archi/src/lib.rs",
        "pub fn f(\n    a: &str,\n    b: i64,\n) -> String {\n    String::new()\n}\n",
    );
    assert_eq!(single_line.len(), 1);
    assert_eq!(multi_line.len(), 1);
    assert_eq!(
        normalize_signature(&single_line[0]),
        normalize_signature(&multi_line[0]),
        "多行与单行归一后逐字相等"
    );
    assert_eq!(
        normalize_signature(&single_line[0]),
        "fn(str, int) -> String",
        "类型间分隔固定（零多余空白）"
    );
}

/// 边界：零参 → `fn() -> <返回>`；`Unknown("")` → 归一带 `?` 前缀；零参零返回不 panic。
#[test]
fn normalize_signature零参与未知返回() {
    let no_params = symbol(vec![], ApiType::Bool);
    assert_eq!(normalize_signature(&no_params), "fn() -> bool");

    let unknown = symbol(vec![], ApiType::Unknown(String::new()));
    assert_eq!(
        normalize_signature(&unknown),
        "fn() -> ?",
        "Unknown 显式留痕（仍可比对）"
    );

    let raw_unknown = symbol(vec![], ApiType::Unknown("impl Trait".to_owned()));
    assert_eq!(
        normalize_signature(&raw_unknown),
        "fn() -> ?impl Trait",
        "Unknown 保留原文"
    );

    let empty = symbol(vec![], ApiType::Unknown("()".to_owned()));
    assert_eq!(normalize_signature(&empty), "fn() -> ?()", "零返回值可判");
}

/// 正向（Enum N+1）：十一值 `ApiType` 各归一出一个规范短形，且十一结果两两不等。
#[test]
fn normalize_signature全api_type十一值() {
    let values = [
        ApiType::Str,
        ApiType::OwnedString,
        ApiType::Int,
        ApiType::Float,
        ApiType::Bool,
        ApiType::List(Box::new(ApiType::List(Box::new(ApiType::Str)))),
        ApiType::Map(Box::new(ApiType::OwnedString)),
        ApiType::Optional(Box::new(ApiType::Named("X".to_owned()))),
        ApiType::Named("Y".to_owned()),
        ApiType::Outcome(Box::new(ApiType::Bool)),
        ApiType::Unknown("$T".to_owned()),
    ];
    assert_eq!(values.len(), 11, "ApiType 恰十一值");

    let rendered: Vec<String> = values
        .iter()
        .map(|ty| normalize_signature(&symbol(vec![], ty.clone())))
        .collect();

    assert_eq!(
        rendered,
        vec![
            "fn() -> str".to_owned(),
            "fn() -> String".to_owned(),
            "fn() -> int".to_owned(),
            "fn() -> float".to_owned(),
            "fn() -> bool".to_owned(),
            "fn() -> list<list<str>>".to_owned(),
            "fn() -> dict<String>".to_owned(),
            "fn() -> optional<X>".to_owned(),
            "fn() -> Y".to_owned(),
            "fn() -> result<bool>".to_owned(),
            "fn() -> ?$T".to_owned(),
        ],
        "十一值各出规范短形"
    );

    let mut unique = rendered.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 11, "十一结果两两不等（值域封闭可辨）");
}

/// 边界（AC-5）：`is_async` 不参与归一（异步性由 `x-function` 的 `async` 键承载）。
#[test]
fn normalize_signature的is_async不参与() {
    let sync = symbol(vec![param("input", ApiType::Str)], ApiType::Bool);
    let mut asynchronous = sync.clone();
    asynchronous.is_async = true;
    assert_eq!(
        normalize_signature(&sync),
        normalize_signature(&asynchronous),
        "异步性不进签名串"
    );
}

/// 边界：四层嵌套与 1000 参数 → 归一串可判且不 panic。
#[test]
fn normalize_signature深度嵌套与规模() {
    let nested = symbol(
        vec![],
        ApiType::Optional(Box::new(ApiType::List(Box::new(ApiType::Optional(
            Box::new(ApiType::Map(Box::new(ApiType::Named("T".to_owned())))),
        ))))),
    );
    assert_eq!(
        normalize_signature(&nested),
        "fn() -> optional<list<optional<dict<T>>>>",
        "四层嵌套逐层可判"
    );

    let many = symbol(
        (0..1000)
            .map(|index| param(&format!("p{index}"), ApiType::Int))
            .collect(),
        ApiType::Bool,
    );
    let rendered = normalize_signature(&many);
    assert!(
        rendered.starts_with("fn(int, int,"),
        "1000 参数不 panic：{}",
        &rendered[..32.min(rendered.len())]
    );
    assert_eq!(
        rendered.matches("int").count(),
        1000,
        "逐参各出一个类型短形"
    );
}

// ---------------------------------------------------------------------------
// 封闭词汇：Visibility / ApiSymbolKind
// ---------------------------------------------------------------------------

/// 正向（AC-5 / AC-9）：`Visibility::ALL` 四值、`as_str()` 逐值逐字、去重后长度不变。
#[test]
fn visibility四值与all() {
    assert_eq!(Visibility::ALL.len(), 4, "四值封闭");
    let words: Vec<&str> = Visibility::ALL.iter().map(|value| value.as_str()).collect();
    assert_eq!(
        words,
        vec!["external", "crateOnly", "unused", "testOnly"],
        "四值线词逐字"
    );

    let mut unique = words.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 4, "ALL 无重复值");
}

/// 正向（N+1）：`ApiSymbolKind` 四值封闭，构造器不单独成值（`is_constructor(name)` 判
/// 而非新 kind）。
#[test]
fn api_symbol_kind四值封闭() {
    let kinds = [
        ApiSymbolKind::Type,
        ApiSymbolKind::Trait,
        ApiSymbolKind::Enum,
        ApiSymbolKind::Function,
    ];
    assert_eq!(kinds.len(), 4, "四值封闭（无第五值）");
    for (index, left) in kinds.iter().enumerate() {
        for right in &kinds[index + 1..] {
            assert_ne!(left, right, "四值可辨");
        }
    }
    // 构造器不单独成值：枚举上没有 `Constructor` 变体（编译期由穷尽 match 保证）
    let named = match kinds[0] {
        ApiSymbolKind::Type => "type",
        ApiSymbolKind::Trait => "trait",
        ApiSymbolKind::Enum => "enum",
        ApiSymbolKind::Function => "function",
    };
    assert_eq!(named, "type", "穷尽 match 证明无第五变体");
}

// ---------------------------------------------------------------------------
// extract_element：判据唯一取数面（整元素导出集）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-api-extract-mod-test-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 正向（AC-9 / D19）：Rust 元素的 op 实现在私有子模块 `src/op.rs` 内、由入口
/// `pub use` 出线 → 返回集**含**该 op（只看入口会漏判）；`owner` / `visibility` 与
/// `rust::extract_crate` 逐字段相等（判据唯一取数面，零第二套抽取）。
#[test]
fn extract_element整元素导出集含子模块op() {
    let temp = TempDir::new("element-rust");
    temp.write(
        "crates/x/src/lib.rs",
        "mod op;\npub use op::run;\n\npub struct Handle;\n\npub fn top() -> String {\n    String::new()\n}\n",
    );
    temp.write(
        "crates/x/src/op.rs",
        "pub fn run(input: &str) -> String {\n    String::new()\n}\n\nfn helper() {}\n",
    );

    let elements = [element("App.Core.X", &["crates/x/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "crates/x/src/lib.rs");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert!(names.contains(&"run"), "子模块的 op 在案：{names:?}");
    assert!(names.contains(&"top"), "入口的自由函数在案：{names:?}");
    assert!(names.contains(&"Handle"), "入口的类型在案：{names:?}");
    assert!(!names.contains(&"helper"), "私有函数不进导出集：{names:?}");

    let run = symbols
        .iter()
        .find(|symbol| symbol.name == "run")
        .expect("run 在案");
    assert_eq!(
        run.visibility,
        Visibility::External,
        "经 crate 根 `pub use` 出线即 External（D19 白名单升级）"
    );
    assert_eq!(run.owner, None, "自由函数无 owner");

    let from_crate = super::rust::extract_crate(temp.root(), "crates/x/src/lib.rs");
    assert_eq!(
        symbols, from_crate,
        "extract_element 的 Rust 分支即 extract_crate（逐字段相等，零第二套）"
    );
}

/// 正向（D19）：TS 元素——源文无透传，故返回集等于对入口源跑 `ts::extract_file`
/// （声明面）；`.tsx` 入口同形不 panic。
#[test]
fn extract_element的ts走单文件() {
    let temp = TempDir::new("element-ts");
    let source = "export function load(a: string): string {\n  return a;\n}\n\nexport interface Shape { x: number }\n";
    let elements = [element("App.Frontend.Views", &["views/", "tsx/"])];
    temp.write("views/index.ts", source);

    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    let direct = super::ts::extract_file("views/index.ts", source);
    assert_eq!(symbols, direct, "TS 元素 = 声明面（零模块树、零透传）");
    assert_eq!(symbols.len(), 2, "一个操作 + 一个类型：{symbols:?}");

    temp.write(
        "tsx/index.tsx",
        "export function view(): string {\n  return '';\n}\n\nexport const Card = () => null;\n",
    );
    let tsx = extract_element(temp.root(), &elements, &elements[0], "tsx/index.tsx");
    assert_eq!(tsx.len(), 1, "「.tsx」入口同形抽取：{tsx:?}");
    assert_eq!(tsx[0].name, "view");
}

/// 异常：入口路径不存在 / 入口是目录 / 入口不可读（以目录占位表达）/ `entry = ""`
/// → 全空 `Vec`（不 panic、不伪造符号；缺口由 `apiEntryMissing` 报）。
#[test]
fn extract_element入口读不到返回空集() {
    let temp = TempDir::new("element-entry-missing");
    // 目录占位入口（`read_to_string` 对目录必然失败）
    fs::create_dir_all(temp.root().join("dir/src/lib.rs")).expect("建目录占位");

    let cases = ["nowhere/src/lib.rs", "dir/src/lib.rs", "", "nowhere/x.ts"];
    let elements = [element("App.Core.X", &["crates/x/"])];
    for entry in cases {
        assert!(
            entry.is_empty() || !temp.root().join(entry).is_file(),
            "夹具应为「读不到」形态：{entry:?}"
        );
        let symbols = extract_element(temp.root(), &elements, &elements[0], entry);
        assert!(
            symbols.is_empty(),
            "入口读不到即空集（不猜测）：{entry:?} → {symbols:?}"
        );
    }
}

/// 边界：入口语言未知（`README.md` / 无扩展名 / `.py`）→ 空 `Vec`（与 `extract` 的
/// `Unknown` 分支同口径，不猜测）。
#[test]
fn extract_element语言未知返回空集() {
    let temp = TempDir::new("element-unknown-lang");
    let elements = [element("App.Core.Scripts", &["scripts/"])];
    for line in ["README.md", "scripts/run", "scripts/x.py"] {
        temp.write(line, "pub fn ignored() {}\n");
        let symbols = extract_element(temp.root(), &elements, &elements[0], line);
        assert!(
            symbols.is_empty(),
            "未知语言不猜符号：{line:?} → {symbols:?}"
        );
    }
}

/// 边界（AC-12）：同一元素下 `extract_element` 的符号集与 `api::bootstrap` 草稿列出
/// 的候选符号集逐项相等（两者共用同一取数面，MUST NOT 各走一套；构造器不进草稿）。
#[test]
fn extract_element与bootstrap同源() {
    let temp = TempDir::new("element-bootstrap-same");
    let fqn = "App.Core.Alpha";
    temp.write(
        ".dev-team/architecture/graph/model/01-test.toml",
        &crate::api::mod_test::declare_ancestors(&format!(
            "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
        )),
    );
    temp.write(
        "src/alpha/src/lib.rs",
        "pub struct Thing;\npub fn alpha() -> String {\n    String::new()\n}\n\
         pub fn new() -> Self {\n    Self\n}\n",
    );

    let elements = [element(fqn, &["src/alpha/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "src/alpha/src/lib.rs");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Thing", "alpha", "new"],
        "取数器如实抽名（构造器不过滤，D11）：{names:?}"
    );

    let text = crate::api::bootstrap(temp.root(), fqn).expect("起草应成功");
    let expected_ops: Vec<&str> = symbols
        .iter()
        .filter(|symbol| symbol.kind == ApiSymbolKind::Function)
        .filter(|symbol| !crate::api::contract::is_constructor(&symbol.name))
        .map(|symbol| symbol.name.as_str())
        .collect();
    assert_eq!(expected_ops, vec!["alpha"], "草稿的 op 集排除构造器");

    for op in &expected_ops {
        assert!(
            text.contains(&format!("### {op}")),
            "草稿逐个列出取数面的 op `{op}`：\n{text}"
        );
    }
    assert!(
        !text.contains("### new"),
        "构造器不进草稿（由生成器产出）：\n{text}"
    );
    let expected_types: Vec<&str> = symbols
        .iter()
        .filter(|symbol| symbol.kind != ApiSymbolKind::Function)
        .map(|symbol| symbol.name.as_str())
        .collect();
    for name in &expected_types {
        assert!(
            text.contains(&format!("\"{name}\"")),
            "草稿的 api-types 含取数面的类型 `{name}`：\n{text}"
        );
    }
}

// ---------------------------------------------------------------------------
// 源码自证：取数面单点（AC-20 ① / ⑥）
// ---------------------------------------------------------------------------

/// 边界（AC-20 ⑥）：未知语言返回空集是**唯一**的回落形态——`extract/mod.rs` 内
/// `Language::Unknown => Vec::new()` 恰 **2 处**（`extract` 与 `extract_element` 各一），
/// 两处之外零「猜成 Rust / TypeScript」的回落分支（未知语言绝不猜测）。
#[test]
fn 未知语言落空集恰两处且无猜测回落() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("Language::Unknown => Vec::new()").count(),
        2,
        "未知语言落空集恰两处（extract / extract_element 各一）"
    );
    // 只按语言分派：两处 `match paths::language_of(..)` 恰对应两个函数
    assert_eq!(
        source.matches("match paths::language_of(").count(),
        2,
        "语言分派只在 extract / extract_element 两处"
    );
    // 第三处 `paths::language_of(` 是 TS 闭包的目标白名单过滤（`export_target`），
    // 不是分派：MUST NOT 因此多出第三个按语言取数的分支
    assert_eq!(
        source.matches("paths::language_of(").count(),
        3,
        "`language_of` 恰三处：两处分派 + 闭包的一处 TS 面过滤"
    );
    // 回落臂（`_ =>`）零命中：三值穷尽，MUST NOT 有「猜成 Rust / TypeScript」的兜底
    assert!(
        !source.contains("_ =>"),
        "语言分派三值穷尽、零兜底臂（未知语言不猜测）"
    );
    // 两个未知分支的产物都是空集（不是某个具体语言的取数器）
    for (offset, _) in source.match_indices("Language::Unknown => Vec::new()") {
        let tail = &source[offset..];
        let line = tail.lines().next().unwrap_or_default();
        assert_eq!(
            line.trim(),
            "Language::Unknown => Vec::new(),",
            "未知分支的产物恰为空集：{line}"
        );
    }
}

/// 边界（AC-20 ①）：`normalize_signature` 的**定义单点与消费单点**——产品 `.rs` 内
/// 恰 1 处 `pub fn` 定义（`extract/mod.rs`），`check.rs` 内恰 1 处消费，且判据对
/// `rust::extract_crate` / `ts::extract_file` 的**直连零命中**（只经 `extract_element`
/// 取数，MUST NOT 各走一套）。
#[test]
fn normalize_signature定义与消费各单点() {
    // 定义面：本文件恰一处 `pub fn normalize_signature`
    let own = include_str!("mod.rs");
    assert_eq!(
        own.matches("pub fn normalize_signature").count(),
        1,
        "`normalize_signature` 定义恰一处（本文件）"
    );

    // 消费面：判据侧恰 1 处消费，且不直连两台语言取数器
    let check = include_str!("../check.rs");
    assert_eq!(
        check.matches("normalize_signature").count(),
        1,
        "`check.rs` 内 `normalize_signature` 恰 1 处消费"
    );
    for direct in [
        "rust::extract_file",
        "rust::extract_crate",
        "ts::extract_file",
    ] {
        assert!(
            !check.contains(direct),
            "判据 MUST NOT 直连语言取数器 `{direct}`（只经 extract_element）"
        );
    }

    // 全 `api/` 目录内 `fn normalize_signature` 只此一处定义（测试文件除外）
    let mut definitions = 0usize;
    for (path, source) in [
        ("mod.rs", own),
        ("check.rs", check),
        ("contract.rs", include_str!("../contract.rs")),
        ("paths.rs", include_str!("../paths.rs")),
        ("schema.rs", include_str!("../schema.rs")),
        ("emit/mod.rs", include_str!("../emit/mod.rs")),
        ("emit/rust.rs", include_str!("../emit/rust.rs")),
        ("emit/ts.rs", include_str!("../emit/ts.rs")),
        ("extract/rust.rs", include_str!("rust.rs")),
        ("extract/ts.rs", include_str!("ts.rs")),
    ] {
        let hit = source.matches("fn normalize_signature").count();
        if hit > 0 {
            assert_eq!(path, "mod.rs", "定义单点只允许在本文件，实际见于 {path}");
            definitions += hit;
        }
    }
    assert_eq!(definitions, 1, "`api/` 内 `fn normalize_signature` 恰一处");
}

// ---------------------------------------------------------------------------
// signature_of / schema_type / Position：类型词汇三层单一（D43 / AC-21）
// ---------------------------------------------------------------------------

/// 正向（AC-21 / D43）：`api-op` fence 原文 → `fn(str) -> String`；与代码侧
/// `fn parse(input: &str) -> String` 的 `normalize_signature` 结果**逐字相等**；
/// `x-function` 包裹形态与裸形态输出逐字相等。
#[test]
fn signature_of契约侧归一() {
    let bare = json!({
        "params": [{"name": "input", "schema": {"type": "string"}}],
        "returns": {"type": "string"}
    });
    assert_eq!(
        signature_of(&bare),
        "fn(str) -> String",
        "入参位 string → str、返回位 → String"
    );

    let code_side = symbol(vec![param("input", ApiType::Str)], ApiType::OwnedString);
    assert_eq!(
        signature_of(&bare),
        normalize_signature(&code_side),
        "契约侧与代码侧同一短形词汇（判定单源的行为证据）"
    );

    let wrapped = json!({
        "x-function": {
            "params": [{"name": "input", "schema": {"type": "string"}}],
            "returns": {"type": "string"}
        }
    });
    assert_eq!(
        signature_of(&wrapped),
        signature_of(&bare),
        "`x-function` 包裹形态与裸形态逐字相等"
    );

    let many = json!({
        "params": [
            {"name": "a", "schema": {"type": "integer"}},
            {"name": "b", "schema": {"type": "boolean"}},
            {"name": "c", "schema": {"type": "number"}}
        ],
        "returns": {"type": "array", "items": {"type": "string"}}
    });
    assert_eq!(
        signature_of(&many),
        "fn(int, bool, float) -> list<String>",
        "多参数逐项映射（items 位固定 Field）"
    );
}

/// 异常（AC-21 关键反例 / D43）：契约 `{"type": "string", "optional": true}` + 代码
/// `fn parse(input: Option<&str>) -> String` → 两侧归一结果**逐字相等**（上一稿漏该
/// 分支即恒假阳）；对照：契约不写 `optional` 时归一**不相等**（该报 ④）。
#[test]
fn signature_of的可选参数包optional() {
    let optional_contract = json!({
        "params": [{
            "name": "input",
            "schema": {"type": "string"},
            "optional": true
        }],
        "returns": {"type": "string"}
    });
    let code_side = symbol(
        vec![param("input", ApiType::Optional(Box::new(ApiType::Str)))],
        ApiType::OwnedString,
    );
    assert_eq!(
        signature_of(&optional_contract),
        "fn(optional<str>) -> String",
        "`optional: true` → `Optional(Str)` 短形"
    );
    assert_eq!(
        signature_of(&optional_contract),
        normalize_signature(&code_side),
        "可选入参两侧同形（零 apiSignatureMismatch 的前提）"
    );

    // 对照：契约不写 optional 而代码是 Option<&str> → 两侧不等
    let required_contract = json!({
        "params": [{"name": "input", "schema": {"type": "string"}}],
        "returns": {"type": "string"}
    });
    assert_ne!(
        signature_of(&required_contract),
        normalize_signature(&code_side),
        "契约不可选而代码可选 → 两侧不等（该报 ④ 判据）"
    );

    // nullable 形态（`anyOf` 含 null 分支）同形不假阳
    let nullable_contract = json!({
        "params": [{
            "name": "input",
            "schema": {"anyOf": [{"type": "string"}, {"type": "null"}]}
        }],
        "returns": {"type": "string"}
    });
    assert_eq!(
        signature_of(&nullable_contract),
        "fn(optional<str>) -> String",
        "nullable 三分支同样落 Optional（与 optional 键同形）"
    );
}

/// 边界（D43）：缺 `params` / 缺 `returns` → 空序列 / `()`；`params = []` 同空序列；
/// `params` 非数组 → 不 panic、结果可判。
#[test]
fn signature_of缺参缺返回() {
    assert_eq!(signature_of(&json!({})), "fn() -> ()", "空契约兜底");

    assert_eq!(
        signature_of(&json!({"returns": {"type": "boolean"}})),
        "fn() -> bool",
        "只写 returns"
    );
    assert_eq!(
        signature_of(&json!({"params": [{"name": "a", "schema": {"type": "integer"}}]})),
        "fn(int) -> ()",
        "只写 params → 空返回 `()`"
    );
    assert_eq!(
        signature_of(&json!({"params": [], "returns": {"type": "string"}})),
        "fn() -> String",
        "空数组同空序列"
    );

    for bad in [json!({"params": "不是数组"}), json!({"params": null})] {
        let rendered = signature_of(&bad);
        assert_eq!(
            rendered, "fn() -> ()",
            "非数组 params 退化为空序列：{rendered}"
        );
    }
}

/// 边界（AC-21 单点源码自证）：`signature_of` 与 `normalize_signature` 都只经私有
/// `render` 写出短形；`check.rs` 内零语言短形字面量（MUST NOT 各拼一份）。
#[test]
fn signature_of与normalize_signature同短形() {
    let source = include_str!("mod.rs");
    assert!(
        source.contains("pub fn normalize_signature")
            && source.contains("pub fn signature_of")
            && source.contains("fn render(ty: &ApiType)"),
        "两个公开入口与一个私有短形写出点同文件"
    );
    assert!(
        source.matches("render(&").count() >= 4,
        "两函数体内均调用 `render(`（唯一短形写出点）"
    );

    let check = include_str!("../check.rs");
    for literal in ["\"str\"", "\"String\"", "\"list<", "\"optional<"] {
        assert!(
            !check.contains(literal),
            "`check.rs` 内零短形字面量 {literal}（MUST NOT 各拼一份）"
        );
    }

    // 行为面：同一逻辑类型序列两侧输出逐字相等
    let code = symbol(
        vec![
            param("a", ApiType::Str),
            param("b", ApiType::List(Box::new(ApiType::Int))),
        ],
        ApiType::Optional(Box::new(ApiType::OwnedString)),
    );
    let contract = json!({
        "params": [
            {"name": "a", "schema": {"type": "string"}},
            {"name": "b", "schema": {"type": "array", "items": {"type": "integer"}}}
        ],
        "returns": {"type": ["string", "null"]}
    });
    assert_eq!(
        signature_of(&contract),
        normalize_signature(&code),
        "同一逻辑类型序列两侧逐字相等"
    );
}

/// 正向（AC-21 / D43）：`schema_type` 全类型分派——`string` 三位置、`integer` /
/// `number` / `boolean`、`array`（items 固定 `Field`）、`object` +
/// `additionalProperties`、`outcome`。
#[test]
fn schema_type全类型分派() {
    assert_eq!(
        schema_type(&json!({"type": "string"}), Position::Param),
        ApiType::Str,
        "入参位 string → Str"
    );
    assert_eq!(
        schema_type(&json!({"type": "string"}), Position::Return),
        ApiType::OwnedString,
        "返回位 string → OwnedString"
    );
    assert_eq!(
        schema_type(&json!({"type": "string"}), Position::Field),
        ApiType::OwnedString,
        "字段位 string → OwnedString"
    );
    assert_eq!(
        schema_type(&json!({"type": "integer"}), Position::Field),
        ApiType::Int
    );
    assert_eq!(
        schema_type(&json!({"type": "number"}), Position::Field),
        ApiType::Float
    );
    assert_eq!(
        schema_type(&json!({"type": "boolean"}), Position::Field),
        ApiType::Bool
    );
    assert_eq!(
        schema_type(
            &json!({"type": "array", "items": {"type": "string"}}),
            Position::Param
        ),
        ApiType::List(Box::new(ApiType::OwnedString)),
        "items 位固定 Field（故 string → OwnedString）"
    );
    assert_eq!(
        schema_type(
            &json!({"type": "object", "additionalProperties": {"type": "integer"}}),
            Position::Param
        ),
        ApiType::Map(Box::new(ApiType::Int)),
        "对象型 additionalProperties → Map"
    );
    assert_eq!(
        schema_type(&json!({"outcome": {"type": "string"}}), Position::Param),
        ApiType::Outcome(Box::new(ApiType::Str)),
        "outcome 信封 → Outcome（内容位沿用位置：入参位 string → Str）"
    );
    assert_eq!(
        schema_type(&json!({"outcome": {"type": "string"}}), Position::Return),
        ApiType::Outcome(Box::new(ApiType::OwnedString)),
        "outcome 信封的内容位随位置（返回位 → OwnedString）"
    );
}

/// 边界（AC-21 关键 / D43）：nullable 三分支全落 `Optional`；嵌套 nullable 保位置。
#[test]
fn schema_type的nullable三分支() {
    assert_eq!(
        schema_type(&json!({"type": "null"}), Position::Param),
        ApiType::Optional(Box::new(ApiType::Unknown("null".to_owned()))),
        "① `type: null` → Optional(Unknown(\"null\"))"
    );
    assert_eq!(
        schema_type(&json!({"type": ["string", "null"]}), Position::Param),
        ApiType::Optional(Box::new(ApiType::Str)),
        "② 类型数组含 null → Optional(Str)（位置语义不变）"
    );
    assert_eq!(
        schema_type(
            &json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            Position::Param
        ),
        ApiType::Optional(Box::new(ApiType::Str)),
        "③ anyOf 含 null 分支 → Optional(Str)"
    );
    assert_eq!(
        schema_type(
            &json!({"oneOf": [{"type": "integer"}, {"type": "null"}]}),
            Position::Return
        ),
        ApiType::Optional(Box::new(ApiType::Int)),
        "④ oneOf 含 null 分支 → Optional(Int)"
    );
    assert_eq!(
        schema_type(
            &json!({"type": ["array", "null"], "items": {"type": "string"}}),
            Position::Param
        ),
        ApiType::Optional(Box::new(ApiType::List(Box::new(ApiType::OwnedString)))),
        "嵌套 nullable 保位置（items 固定 Field）"
    );
}

/// 边界（AC-21 / D43）：`$ref` → `Unknown(ref_name(..))`（**不归 `Named`**）；未知
/// 关键字 → `Unknown`、零 panic；同文件引用同形。
#[test]
fn schema_type的ref与unknown留痕() {
    let external = json!({"$ref": "../Mod/App.Mod.schema.json#/$defs/SummaryReport"});
    assert_eq!(
        schema_type(&external, Position::Param),
        ApiType::Unknown("SummaryReport".to_owned()),
        "`$ref` → Unknown（剥 `#/$defs/` 取末段）"
    );
    assert!(
        !matches!(schema_type(&external, Position::Param), ApiType::Named(_)),
        "MUST NOT 把 `$ref` 归成 Named（第二套命名规则会让留痕失效）"
    );

    assert_eq!(
        schema_type(&json!({"$ref": "#/$defs/X"}), Position::Field),
        ApiType::Unknown("X".to_owned()),
        "同文件引用同形"
    );
    assert_eq!(
        schema_type(&json!({"format": "date-time"}), Position::Param),
        ApiType::Unknown(String::new()),
        "未知关键字（无 type）→ Unknown(空串)"
    );
    assert_eq!(
        schema_type(&json!({"type": "snowflake"}), Position::Param),
        ApiType::Unknown("snowflake".to_owned()),
        "未知 type 原文留痕"
    );
    assert_eq!(
        schema_type(&json!({"type": "object"}), Position::Param),
        ApiType::Unknown("object".to_owned()),
        "无对象型 additionalProperties 的 object → Unknown(\"object\")"
    );

    // 经公开函数观测：`$ref` 的短形是 `?<末段>`（`Unknown` 的显式留痕），不是裸名
    assert_eq!(
        signature_of(&json!({"returns": external})),
        "fn() -> ?SummaryReport",
        "`$ref` 短形带 `?` 前缀（Unknown 留痕，非 Named 裸名）"
    );
}

/// 边界（D43 注记面）：`Unknown` 与 `Named` 两个变体的短形各有恰 1 个 `match` 臂且在
/// 同一 `render` 内；两侧短形共用同一写出点（零第二套归一规则）。
#[test]
fn ref短形与named共用写出点() {
    let source = include_str!("mod.rs");
    let render_start = source
        .find("fn render(ty: &ApiType)")
        .expect("私有 `render` 在案");
    let render_body = &source[render_start..];
    assert_eq!(
        render_body.matches("ApiType::Named(").count(),
        1,
        "`Named` 的短形恰 1 个 match 臂（在 `render` 内）"
    );
    assert_eq!(
        render_body.matches("ApiType::Unknown(").count(),
        1,
        "`Unknown` 的短形恰 1 个 match 臂（在 `render` 内）"
    );
    assert!(
        render_body.contains("format!(\"?{raw}\")"),
        "`Unknown` 短形带 `?` 前缀（显式留痕）"
    );
}

/// 边界（AC-21 / D43）：`schema_type` 的输出是 `ApiType` 逻辑类型，**零语言文本形**
/// （体内零 `&str` / `Vec<` / `Record<` 字面量）；同一 schema 在 `Param` / `Return`
/// 两位产出**不同**类型（位置敏感）。
#[test]
fn schema_type与emit的射程边界() {
    let source = include_str!("mod.rs");
    for literal in ["\"&str\"", "\"Vec<\"", "\"Record<\""] {
        assert!(
            !source.contains(literal),
            "`extract/mod.rs` 内零语言文本形字面量 {literal}（逻辑类型层不产文本形）"
        );
    }

    let string_schema = json!({"type": "string"});
    let as_param = schema_type(&string_schema, Position::Param);
    let as_return = schema_type(&string_schema, Position::Return);
    assert_eq!(as_param, ApiType::Str);
    assert_eq!(as_return, ApiType::OwnedString);
    assert_ne!(
        as_param, as_return,
        "同一 schema 两位产出不同类型（位置敏感）"
    );
}

/// 正向（AC-21 / D43）：`Position` 三值可辨；产品侧 `pub enum Position` **恰 1 处**
/// （本文件），`emit/{rust,ts}.rs` 与 `check.rs` **各零处**（只 `use` 不自建）。
#[test]
fn position三值封闭() {
    let values = [Position::Param, Position::Return, Position::Field];
    assert_eq!(values.len(), 3, "位置词汇三值封闭");
    assert_ne!(Position::Param, Position::Return);
    assert_ne!(Position::Return, Position::Field);
    assert_ne!(Position::Param, Position::Field);

    assert_eq!(
        include_str!("mod.rs").matches("pub enum Position").count(),
        1,
        "位置词汇的唯一落点（本文件恰 1 处）"
    );
    for (path, other) in [
        ("emit/rust.rs", include_str!("../emit/rust.rs")),
        ("emit/ts.rs", include_str!("../emit/ts.rs")),
        ("check.rs", include_str!("../check.rs")),
    ] {
        assert!(
            !other.contains("enum Position"),
            "`{path}` 内零本地位置表（只 `use`）"
        );
    }
    assert!(
        include_str!("../emit/rust.rs")
            .contains("use crate::api::extract::{nullable_breakdown, Position}")
            && include_str!("../emit/ts.rs")
                .contains("use crate::api::extract::{nullable_breakdown, Position}"),
        "两渲染器都从本文件 `use Position`（位置词汇与可空形状识别同一来源）"
    );
}

/// 边界（AC-21 ② / D43）：私有 `render` 恰 1 处定义、`pub fn render(ty: &ApiType)` 零
/// 命中（MUST NOT 为可测性升格）；短形词汇逐值可经两个公开函数间接观测。
#[test]
fn render保持私有() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("fn render(ty: &ApiType)").count(),
        1,
        "私有 `render` 恰 1 处定义"
    );
    assert_eq!(
        source.matches("pub fn render(ty: &ApiType)").count(),
        0,
        "MUST NOT 升格为 `pub`（唯一消费方同文件）"
    );

    // 返回位短形逐值经公开函数 `signature_of` 观测（不直调私有 `render`）
    let returns_cases = [
        (json!({"type": "string"}), "fn() -> String"),
        (json!({"type": "integer"}), "fn() -> int"),
        (json!({"type": "number"}), "fn() -> float"),
        (json!({"type": "boolean"}), "fn() -> bool"),
        (
            json!({"type": "array", "items": {"type": "string"}}),
            "fn() -> list<String>",
        ),
        (
            json!({"type": "object", "additionalProperties": {"type": "integer"}}),
            "fn() -> dict<int>",
        ),
        (json!({"$ref": "#/$defs/Raw"}), "fn() -> ?Raw"),
    ];
    for (schema, expected) in returns_cases {
        assert_eq!(
            signature_of(&json!({"returns": schema})),
            expected,
            "短形词汇经公开函数可观测"
        );
    }

    // 入参位短形（`str`）同源可观测
    assert_eq!(
        signature_of(&json!({"params": [{"name": "x", "schema": {"type": "string"}}]})),
        "fn(str) -> ()",
        "入参位短形经公开函数可观测"
    );
    // `Optional` / `Outcome` 短形同源
    assert_eq!(
        signature_of(&json!({"returns": {"type": ["integer", "null"]}})),
        "fn() -> optional<int>",
        "Optional 短形"
    );
    assert_eq!(
        signature_of(&json!({"returns": {"outcome": {"type": "boolean"}}})),
        "fn() -> result<bool>",
        "Outcome 短形"
    );
}

/// 异常（AC-21 ⑤ / D43）：`emit/{rust,ts}.rs` 与 `check.rs` 对 `ApiType` 零引用（不
/// 自拼短形）；短形词汇的 `match ty` 只在私有 `render` 内。
#[test]
fn 短形词汇只在render一处() {
    for (path, source) in [
        ("emit/rust.rs", include_str!("../emit/rust.rs")),
        ("emit/ts.rs", include_str!("../emit/ts.rs")),
        ("check.rs", include_str!("../check.rs")),
    ] {
        assert!(
            !source.contains("ApiType"),
            "`{path}` 内零 `ApiType` 引用（不自拼短形，走位置 × 语言映射）"
        );
    }
    assert_eq!(
        include_str!("mod.rs").matches("match ty").count(),
        1,
        "`match ty` 只在私有 `render` 内"
    );
}

// ---------------------------------------------------------------------------
// schema_type 边界片（D43）：非对象值、超长类型名、未知 type、多类型无 null
// ---------------------------------------------------------------------------

/// 边界（**D43** 边界）：`Value::Null` / 数组 / 裸字符串 / 数字四形态 → 全 `Unknown(..)`
/// 且零 panic（显式留痕，归一仍可比对）；未知 `type` 原文留痕；多类型无 null 可判退化不
/// panic；超长类型名（>1000 chars）原样进 `Unknown`。
#[test]
fn schema_type边界与异常片() {
    // ① 非对象值四形态：显式留痕（`Unknown("")`），不 panic、不误判成已知类型
    for fragment in [json!(null), json!([]), json!("裸字符串"), json!(42)] {
        let resolved = schema_type(&fragment, Position::Param);
        assert_eq!(
            resolved,
            ApiType::Unknown(String::new()),
            "非对象片段 {fragment} → Unknown(空串)（显式留痕，不猜）"
        );
    }

    // ② 未知 `type` 值：原文进 `Unknown`（留痕可比对）
    assert_eq!(
        schema_type(&json!({"type": "timestamp"}), Position::Param),
        ApiType::Unknown("timestamp".to_owned()),
        "未知 type 原文留痕"
    );

    // ③ 多类型无 null：可判退化、不 panic、不包 Optional
    let multi = schema_type(&json!({"type": ["string", "integer"]}), Position::Param);
    assert!(
        !matches!(multi, ApiType::Optional(_)),
        "无 null 分支的多类型 MUST NOT 包 Optional：{multi:?}"
    );

    // ④ 超长类型名（>1000 chars）：原样进 Unknown，不 panic
    let long_type = "t".repeat(1200);
    assert_eq!(
        schema_type(&json!({"type": long_type}), Position::Param),
        ApiType::Unknown(long_type.clone()),
        "超长未知 type 原样留痕（不截断）"
    );

    // ⑤ 经公开函数观测：留痕的 `?` 前缀仍在（短形可比对）
    assert_eq!(
        signature_of(&json!({"returns": json!("裸字符串")})),
        "fn() -> ?",
        "非对象返回位 → `?` 前缀 + 空留痕"
    );
}

/// 正向（**D43** 私有备案 / 经公开函数观测）：短形十一值各在场且两两可辨——`Named`
/// 裸名与 `Unknown` 的 `?` 前缀**形态不同**（已知名 vs 显式留痕）。
#[test]
fn render短形十一值全可辨() {
    let cases = [
        (ApiType::Str, "str"),
        (ApiType::OwnedString, "String"),
        (ApiType::Int, "int"),
        (ApiType::Float, "float"),
        (ApiType::Bool, "bool"),
        (ApiType::List(Box::new(ApiType::Str)), "list<str>"),
        (ApiType::Map(Box::new(ApiType::Int)), "dict<int>"),
        (ApiType::Optional(Box::new(ApiType::Str)), "optional<str>"),
        (ApiType::Outcome(Box::new(ApiType::Bool)), "result<bool>"),
        (ApiType::Named("Y".to_owned()), "Y"),
        (ApiType::Unknown("$T".to_owned()), "?$T"),
    ];
    assert_eq!(cases.len(), 11, "十一值封闭（`ApiType` 全值域）");

    // 经 `normalize_signature` 逐值观测（不直调私有 `render`）
    let mut rendered: Vec<String> = Vec::new();
    for (ty, expected) in &cases {
        let text = normalize_signature(&symbol(vec![], ty.clone()));
        assert_eq!(
            text,
            format!("fn() -> {expected}"),
            "短形 {expected} 经公开函数可观测"
        );
        rendered.push(text);
    }
    let mut unique = rendered.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 11, "十一个短形两两不等（值域封闭可辨）");

    // `Named` 裸名与 `Unknown` 的 `?` 前缀形态不同（留痕 vs 已知名）
    assert_eq!(
        normalize_signature(&symbol(vec![], ApiType::Named("T".to_owned()))),
        "fn() -> T"
    );
    assert_eq!(
        normalize_signature(&symbol(vec![], ApiType::Unknown("T".to_owned()))),
        "fn() -> ?T"
    );
}

// ---------------------------------------------------------------------------
// 调用面单点（D43 / AC-21 ③⑤）
// ---------------------------------------------------------------------------

/// 边界（**D43 / AC-21 ③⑤** 源码自证）：`pub fn schema_type` 与 `pub fn signature_of`
/// 各恰 **1** 处（本文件）；`check.rs` 内零本地归一函数、对 `"anyOf"` / `"null"` 的直
/// 接匹配零命中（nullable 只经 `schema_type`）、对语言取数器的直连零命中。
#[test]
fn schema_type与signature_of调用面单点() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("pub fn schema_type").count(),
        1,
        "`pub fn schema_type` 恰 1 处（类型单点）"
    );
    assert_eq!(
        source.matches("pub fn signature_of").count(),
        1,
        "`pub fn signature_of` 恰 1 处（契约侧归一单点）"
    );
    assert_eq!(
        source.matches("pub fn normalize_signature").count(),
        1,
        "`pub fn normalize_signature` 恰 1 处（代码侧归一单点）"
    );

    let check = include_str!("../check.rs");
    for local in [
        "fn normalize_signature",
        "fn normalize_schema",
        "fn contract_signature",
    ] {
        assert!(
            !check.contains(local),
            "`check.rs` 内零本地归一 `{local}`（判据只调用单点）"
        );
    }
    for keyword in ["\"anyOf\"", "\"oneOf\"", "\"null\""] {
        assert!(
            !check.contains(keyword),
            "`check.rs` 内零 `{keyword}` 直接匹配（nullable 只经 `schema_type`）"
        );
    }
    for direct in [
        "rust::extract_crate",
        "ts::extract_file",
        "extract_crate(",
        "extract_file(",
    ] {
        assert!(
            !check.contains(direct),
            "`check.rs` 内零 `{direct}` 直连（导出集只经 `extract_element`）"
        );
    }
}

// ---------------------------------------------------------------------------
// extract 的显式留白（D52 / AC-29）
// ---------------------------------------------------------------------------

/// 边界（**D52 / AC-29**）：`extract` 是**单文件入口面的预留接口**，产品侧零调用者——
/// ① 产品 `.rs`（滤 `*_test.rs`）内 `extract::extract(` 零命中；② 留白句 ≥ 2 处（模块
/// doc + 函数 doc 各一处）；③ `pub fn extract(` 恰 1（接口未删，**MUST 用带 `(` 的定形
/// 串**——裸 `pub fn extract` 会把 `extract_element` 一并命中）；行为面：`extract` 与
/// `extract_element` 在同一单文件 Rust 入口上符号集逐字段相等（留白不是死代码）。
#[test]
fn extract显式留白() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("pub fn extract(").count(),
        1,
        "`pub fn extract(` 恰 1 处（接口未删；定形串带 `(` 以避开 `extract_element`）"
    );
    assert_eq!(
        source.matches("pub fn extract_element(").count(),
        1,
        "`pub fn extract_element(` 恰 1 处（判据唯一取数面）"
    );

    // ② 留白句 ≥ 2：模块 doc 与函数 doc 各一处「零调用者 / 预留接口」语义
    let blank_markers = source.matches("显式留白").count() + source.matches("零调用者").count();
    assert!(
        blank_markers >= 2,
        "留白句 ≥ 2 处（模块 doc + 函数 doc），实际 {blank_markers} 处"
    );

    // ① 产品侧零调用者（滤测试文件后零命中）
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&manifest.join("src"), &mut hits);
    let mut src_tauri = manifest.clone();
    for _ in 0..3 {
        src_tauri.pop();
    }
    scan_product_rs(&src_tauri.join("crates"), &mut hits);
    scan_product_rs(&src_tauri.join("src"), &mut hits);
    let callers: Vec<&(String, String)> = hits
        .iter()
        .filter(|(_, line)| {
            !line.trim_start().starts_with("//") && line.contains("extract::extract(")
        })
        .collect();
    assert!(
        callers.is_empty(),
        "`extract::extract(` 产品侧零调用者（留白是既定形态，缺它即有人加了第二取数面）：{callers:?}"
    );

    // 行为面：同一单文件 Rust 入口上 `extract` 与 `extract_element` 符号集逐字段相等
    let temp = TempDir::new("extract-blank-same-face");
    let elements = [element("App.Core.Alpha", &["src/alpha/src/lib.rs"])];
    let body = "pub struct T;\npub fn a(input: &str) -> String { String::new() }\n";
    temp.write("src/alpha/src/lib.rs", body);
    let entry = "src/alpha/src/lib.rs";
    let by_extract = extract(&elements[0], entry, body);
    let by_element = extract_element(temp.root(), &elements, &elements[0], entry);
    assert_eq!(
        by_extract.len(),
        by_element.len(),
        "两条入口的符号条数相等（留白不改语义）"
    );
    for (left, right) in by_extract.iter().zip(by_element.iter()) {
        assert_eq!(left, right, "符号逐字段相等（共用同一取数器）");
    }
}

/// 递归收集产品 `.rs`（排除 `*_test.rs` 与构建产物目录）的（路径，行）二元组。
fn scan_product_rs(dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
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
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let display = path.to_string_lossy().replace('\\', "/");
        for line in text.lines() {
            out.push((display.clone(), line.to_owned()));
        }
    }
}

// ---------------------------------------------------------------------------
// nullable_breakdown：可空形状识别的唯一单点（AC-1 / D1）
// ---------------------------------------------------------------------------

/// 契约文件（repo 相对 POSIX）与元素 FQN（`parse` 的 H1 锚点）。
const CONTRACT_FILE: &str = ".dev-team/architecture/models/App.Demo.Widget.md";
const CONTRACT_FQN: &str = "App.Demo.Widget";

/// 动态分支键的 schema 片段（`anyOf` / `oneOf` 二键同判）。
fn branch_schema(key: &str, branches: Value) -> Value {
    let mut object = serde_json::Map::new();
    object.insert(key.to_owned(), branches);
    Value::Object(object)
}

/// 单字段 `api-types` 源文（`$defs.Widget.properties.f` 取给定片段）。
fn widget_source(fragment: &Value, required: bool) -> String {
    let required = if required {
        ", \"required\": [\"f\"]"
    } else {
        ""
    };
    format!(
        "# {CONTRACT_FQN}\n\n```json api-types\n{{\"Widget\": {{\"type\": \"object\", \
         \"properties\": {{\"f\": {fragment}}}{required}}}}}\n```\n"
    )
}

/// 双字段源文（同一片段落在必填位 `a` 与可选位 `b` 上）。
fn two_field_source(fragment: &Value) -> String {
    format!(
        "# {CONTRACT_FQN}\n\n```json api-types\n{{\"Widget\": {{\"type\": \"object\", \
         \"properties\": {{\"a\": {fragment}, \"b\": {fragment}}}, \"required\": [\"a\"]}}}}\n```\n"
    )
}

/// 两侧渲染文本（真实 `contract::parse` + `schema::build_schema` + `emit::{ts,rust}::render`）。
fn render_both(source: &str) -> (String, String) {
    let contract = parse(source, CONTRACT_FQN, CONTRACT_FILE).expect("契约应解析成功");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let target = |kind: GeneratedKind| EmitTarget {
        kind,
        form: EntryForm::SubdirModule,
        modules: BTreeMap::new(),
    };
    let ts = emit::ts::render(
        &contract,
        &schema,
        &meta(),
        &target(GeneratedKind::TypeScript),
    );
    let rust = emit::rust::render(&contract, &schema, &meta(), &target(GeneratedKind::Rust));
    (ts, rust)
}

/// TS interface 内某字段的整行（缺席即断言失败）。
fn ts_field_line<'a>(text: &'a str, field: &str) -> &'a str {
    text.lines()
        .find(|line| {
            let head = line.trim_start();
            head.starts_with(&format!("{field}:")) || head.starts_with(&format!("{field}?:"))
        })
        .unwrap_or_else(|| panic!("TS interface 内应有字段 `{field}`：\n{text}"))
}

/// Rust struct 内某字段的整行（缺席即断言失败）。
fn rust_field_line<'a>(text: &'a str, field: &str) -> &'a str {
    text.lines()
        .find(|line| line.trim_start().starts_with(&format!("pub {field}:")))
        .unwrap_or_else(|| panic!("Rust struct 内应有字段 `{field}`：\n{text}"))
}

/// 三轴对拍语料：`(标签, 字段片段, 字段是否在 required 内)`，九组。
///
/// 前六组是**可空轴**（显式 null 的三种形状 + 叠加可选 + 数组元素位），次两组是**可选轴**
/// （同一片段在必填 / 可选两位），末一组是**纯 null 档**（不参与可空轴的等价式）。
fn corpus() -> Vec<(&'static str, Value, bool)> {
    vec![
        ("类型数组含 null", json!({"type": ["string", "null"]}), true),
        (
            "三元素类型数组",
            json!({"type": ["string", "integer", "null"]}),
            true,
        ),
        (
            "anyOf 含 null 分支",
            branch_schema("anyOf", json!([{"type": "string"}, {"type": "null"}])),
            true,
        ),
        (
            "oneOf 含 null 分支",
            branch_schema("oneOf", json!([{"type": "integer"}, {"type": "null"}])),
            true,
        ),
        (
            "非 required 叠加显式 null",
            json!({"type": ["string", "null"]}),
            false,
        ),
        (
            "数组元素位可空",
            json!({"type": "array", "items": {"type": ["string", "null"]}}),
            true,
        ),
        ("必填字段（可选轴对照）", json!({"type": "string"}), true),
        ("可选字段（可选轴对照）", json!({"type": "string"}), false),
        ("纯 null 档", json!({"type": "null"}), true),
    ]
}

/// 正向：`{"type": ["string","null"]}` → `Some`——`explicit_null` 在案、`non_null` 是
/// 「外层克隆 + `type` 换成首个非空 kind」的窄化视图（`Cow::Owned`）。
#[test]
fn nullable_breakdown类型数组窄化视图() {
    let schema = json!({"type": ["string", "null"]});
    let breakdown = nullable_breakdown(&schema).expect("类型数组含 null → Some");
    assert!(breakdown.explicit_null, "显式 null 在案");
    let inner = breakdown.non_null.as_ref().expect("有非空分支可取");
    assert!(
        matches!(inner, Cow::Owned(_)),
        "类型数组形态经窄化克隆（`Cow::Owned`）：{inner:?}"
    );
    assert_eq!(
        inner.as_ref(),
        &json!({"type": "string"}),
        "非空分支 = 外层克隆 + `type` 换成首个非空 kind"
    );
}

/// 正向（表驱动）：`anyOf` / `oneOf` 含 `{"type": "null"}` 分支 → `Some`，`non_null`
/// 借自首个非空分支（`Cow::Borrowed`，零克隆）。
#[test]
fn nullable_breakdown分支数组零克隆() {
    for key in ["anyOf", "oneOf"] {
        let schema = branch_schema(key, json!([{"type": "integer"}, {"type": "null"}]));
        let breakdown = nullable_breakdown(&schema).expect("分支数组含 null 分支 → Some");
        assert!(breakdown.explicit_null, "{key}：显式 null 在案");
        let inner = breakdown.non_null.as_ref().expect("有非空分支可取");
        assert!(
            matches!(inner, Cow::Borrowed(_)),
            "{key}：分支数组走借用（`Cow::Borrowed`，零克隆）：{inner:?}"
        );
        assert_eq!(
            inner.as_ref(),
            &json!({"type": "integer"}),
            "{key}：非空分支逐字取自分支数组的首个非空项"
        );
    }
}

/// 正向：`{"type": "null"}` → 纯 null 档（`non_null == None`，无第二个分支可取）。
#[test]
fn nullable_breakdown纯null档() {
    let schema = json!({"type": "null"});
    let breakdown = nullable_breakdown(&schema).expect("`type: null` → Some");
    assert!(breakdown.explicit_null, "显式 null 在案");
    assert!(
        breakdown.non_null.is_none(),
        "纯 null 档无非空分支可取（MUST NOT 造占位分支）"
    );
}

/// 异常（表驱动）：非 nullable 形态 → `None`（交回常规分派，零误判）——`string` /
/// `$ref` / `outcome` / 空对象四形态。
#[test]
fn nullable_breakdown非nullable形态交回常规分派() {
    let cases = [
        json!({"type": "string"}),
        json!({"$ref": "#/$defs/T"}),
        json!({"outcome": {"type": "string"}}),
        json!({}),
    ];
    for fragment in cases {
        assert!(
            nullable_breakdown(&fragment).is_none(),
            "非 nullable 形态 → None（交回常规分派）：{fragment}"
        );
    }
}

/// 异常（表驱动）：`anyOf` / `oneOf` 非数组、空数组、数组内无 null 分支 → `None`
/// （分支数组形态未成立即不猜）。
#[test]
fn nullable_breakdown分支数组非数组空数组无null() {
    let cases = [
        branch_schema("anyOf", json!("string")),
        branch_schema("anyOf", json!([])),
        branch_schema("anyOf", json!([{"type": "string"}, {"type": "integer"}])),
        branch_schema("oneOf", json!({})),
        branch_schema("oneOf", json!([])),
        branch_schema("oneOf", json!([{"type": "boolean"}])),
    ];
    for fragment in cases {
        assert!(
            nullable_breakdown(&fragment).is_none(),
            "分支数组形态未成立 → None：{fragment}"
        );
    }
}

/// 边界（表驱动）：三元素 `["string","integer","null"]` 只取首个非空分支（`integer`
/// 静默丢弃，多分支完整表达不在本期）；`["null","string"]` 仍取 `string`。
#[test]
fn nullable_breakdown多分支取首个非空分支() {
    let three_schema = json!({"type": ["string", "integer", "null"]});
    let three = nullable_breakdown(&three_schema).expect("三元素 → Some");
    assert_eq!(
        three.non_null.as_ref().map(|inner| inner.as_ref().clone()),
        Some(json!({"type": "string"})),
        "只取首个非空分支（`integer` 静默丢弃，R-3 留白）"
    );

    let null_first_schema = json!({"type": ["null", "string"]});
    let null_first = nullable_breakdown(&null_first_schema).expect("null 在前 → Some");
    assert_eq!(
        null_first
            .non_null
            .as_ref()
            .map(|inner| inner.as_ref().clone()),
        Some(json!({"type": "string"})),
        "null 在前仍取第一个非空分支 `string`"
    );
}

/// 边界（表驱动）：空类型表 → `None`（空 kind 表不含 `null`）；纯 null 类型表与只含
/// null 分支的分支数组 → 纯 null 档（`non_null == None`）。
#[test]
fn nullable_breakdown空类型表与纯null数组形态() {
    assert!(
        nullable_breakdown(&json!({"type": []})).is_none(),
        "空 kind 表不含 `null` → None（交回常规分派）"
    );

    for fragment in [json!({"type": ["null"]}), json!({"type": ["null", "null"]})] {
        let breakdown = nullable_breakdown(&fragment).expect("纯 null 类型表 → Some");
        assert!(breakdown.explicit_null, "{fragment}：显式 null 在案");
        assert!(
            breakdown.non_null.is_none(),
            "{fragment}：无非空分支可取（纯 null 档）"
        );
    }

    let only_null = branch_schema("anyOf", json!([{"type": "null"}]));
    let breakdown = nullable_breakdown(&only_null).expect("分支数组只含 null → Some");
    assert!(
        breakdown.explicit_null,
        "分支数组纯 null 档：显式 null 在案"
    );
    assert!(
        breakdown.non_null.is_none(),
        "分支数组纯 null 档：无非空分支可取"
    );
}

/// 边界（表驱动）：非对象 schema（`null` / 数字 / 字符串 / 数组）→ `None`，零 panic。
#[test]
fn nullable_breakdown非对象零panic() {
    for fragment in [json!(null), json!(42), json!("string"), json!([])] {
        assert!(
            nullable_breakdown(&fragment).is_none(),
            "非对象 schema → None（零 panic）：{fragment}"
        );
    }
}

/// 正向（表驱动）：nullable 三分支 × `Position` 三值——带非空分支的两形态在 `Param`
/// 位出 `Optional(Str)`、`Return` / `Field` 位出 `Optional(OwnedString)`（位置语义不因
/// nullable 改变）；纯 null 档三位置同值 `Optional(Unknown("null"))`。
#[test]
fn schema_type的nullable三分支保位置语义() {
    let string_branches = [
        json!({"type": ["string", "null"]}),
        branch_schema("anyOf", json!([{"type": "string"}, {"type": "null"}])),
        branch_schema("oneOf", json!([{"type": "null"}, {"type": "string"}])),
    ];
    for fragment in string_branches {
        for (position, inner) in [
            (Position::Param, ApiType::Str),
            (Position::Return, ApiType::OwnedString),
            (Position::Field, ApiType::OwnedString),
        ] {
            assert_eq!(
                schema_type(&fragment, position),
                ApiType::Optional(Box::new(inner.clone())),
                "{fragment} 在 {position:?} 位 → Optional({inner:?})"
            );
        }
    }

    for position in [Position::Param, Position::Return, Position::Field] {
        assert_eq!(
            schema_type(&json!({"type": "null"}), position),
            ApiType::Optional(Box::new(ApiType::Unknown("null".to_owned()))),
            "纯 null 档在 {position:?} 位同值 Optional(Unknown(\"null\"))"
        );
    }
}

/// 异常（表驱动）：判序不变——`$ref` 先于 nullable（→ `Unknown(ref)`）、`outcome` 先于
/// nullable（→ `Outcome`）。
#[test]
fn schema_type判序ref与outcome先于nullable() {
    assert_eq!(
        schema_type(
            &json!({"$ref": "#/$defs/T", "type": ["string", "null"]}),
            Position::Param
        ),
        ApiType::Unknown("T".to_owned()),
        "`$ref` 判在 nullable 之前（末段名留痕，不包 Optional）"
    );
    assert_eq!(
        schema_type(
            &json!({"outcome": {"type": "string"}, "type": ["string", "null"]}),
            Position::Return
        ),
        ApiType::Outcome(Box::new(ApiType::OwnedString)),
        "`outcome` 判在 nullable 之前（内容位沿用位置语义）"
    );
}

/// 边界（表驱动）：`{"type":"null"}` → `Optional(Unknown("null"))`；数组位可空 →
/// `Optional(List(OwnedString))`；`items` 内 nullable → 元素位 `Optional`（嵌套保位置）。
#[test]
fn schema_type的纯null与嵌套元素位() {
    assert_eq!(
        schema_type(&json!({"type": "null"}), Position::Field),
        ApiType::Optional(Box::new(ApiType::Unknown("null".to_owned()))),
        "纯 null 档显式留痕 `Unknown(\"null\")`"
    );
    assert_eq!(
        schema_type(
            &json!({"type": ["array", "null"], "items": {"type": "string"}}),
            Position::Param
        ),
        ApiType::Optional(Box::new(ApiType::List(Box::new(ApiType::OwnedString)))),
        "数组位可空 → Optional(List(..))（items 固定 Field）"
    );
    assert_eq!(
        schema_type(
            &json!({"type": "array", "items": {"type": ["integer", "null"]}}),
            Position::Param
        ),
        ApiType::List(Box::new(ApiType::Optional(Box::new(ApiType::Int)))),
        "items 内 nullable → 元素位 Optional（嵌套保位置）"
    );
}

/// 边界：可选轴不进 `schema_type` 的判据面——同一片段 `{"type":"string"}` 在必填 /
/// 可选语境取同值（`OwnedString`），可选性只在字段级由 `required` 决定（TS `?` /
/// Rust `Option<>`）；签名两参、体内零 `required`。
#[test]
fn schema_type不收字段可选语境() {
    let fragment = json!({"type": "string"});
    assert_eq!(
        schema_type(&fragment, Position::Field),
        ApiType::OwnedString,
        "片段本身的逻辑类型（可选语境不改它）"
    );

    let (required_ts, required_rust) = render_both(&widget_source(&fragment, true));
    let (optional_ts, optional_rust) = render_both(&widget_source(&fragment, false));
    assert_eq!(
        ts_field_line(&required_ts, "f").trim(),
        "f: string;",
        "必填位零 `?` 零 `| null`：\n{required_ts}"
    );
    assert_eq!(
        ts_field_line(&optional_ts, "f").trim(),
        "f?: string;",
        "可选位只加 `?`（零 `| null`）：\n{optional_ts}"
    );
    assert_eq!(
        rust_field_line(&required_rust, "f").trim(),
        "pub f: String,",
        "必填位裸类型：\n{required_rust}"
    );
    assert_eq!(
        rust_field_line(&optional_rust, "f").trim(),
        "pub f: Option<String>,",
        "可选位包 `Option<…>`：\n{optional_rust}"
    );

    let source = include_str!("mod.rs");
    assert_eq!(
        source
            .matches("pub fn schema_type(schema: &Value, position: Position) -> ApiType")
            .count(),
        1,
        "`schema_type` 签名两参（不因可选轴扩参）"
    );
    assert!(
        !source.contains("required"),
        "`schema_type` 的入参面零 `required`（可选轴只在字段级判）"
    );
}

/// 正向（AC-1 (a)，逐组对拍 6 组）：可空轴三面同真同假——`schema_type` 为 `Optional`
/// ⟺ TS 文本含 `| null` ⟺ Rust 文本含 `Option<`（数组组在元素位对拍）。
#[test]
fn 三轴对拍可空轴六组() {
    for (label, fragment, required) in corpus().into_iter().take(6) {
        let source = widget_source(&fragment, required);
        let (ts, rust) = render_both(&source);
        // 数组形态的判据面在**元素位**（`items`）：字段类型本身是 `List`，元素才是 Optional
        let judged = match fragment.get("type").and_then(Value::as_str) {
            Some("array") => schema_type(&fragment["items"], Position::Field),
            _ => schema_type(&fragment, Position::Field),
        };
        let nullable = matches!(judged, ApiType::Optional(_));
        let ts_nullable = ts_field_line(&ts, "f").contains("| null");
        let rust_nullable = rust_field_line(&rust, "f").contains("Option<");
        assert!(
            nullable,
            "{label}：`schema_type` 应为 Optional，实际 {judged:?}"
        );
        assert_eq!(
            nullable, ts_nullable,
            "{label}：`schema_type` 的 Optional 与 TS `| null` 必须同真同假：\n{ts}"
        );
        assert_eq!(
            nullable, rust_nullable,
            "{label}：`schema_type` 的 Optional 与 Rust `Option<` 必须同真同假：\n{rust}"
        );
    }
}

/// 正向（AC-1 (b)，单例两组）：可选轴——同一 `$defs` 条目内 `a`（在 `required`）与
/// `b`（不在）用同一片段：`schema_type` 两者同值 `OwnedString`；TS `a: string;` /
/// `b?: string;`（`a` 零 `?`、`b` 零 `| null`）；Rust `pub a: String,` /
/// `pub b: Option<String>,`；MUST NOT 拿「Rust 含 `Option<`」当可空判据。
#[test]
fn 三轴对拍可选轴两组() {
    let fragment = json!({"type": "string"});
    let (ts, rust) = render_both(&two_field_source(&fragment));

    assert_eq!(
        schema_type(&fragment, Position::Field),
        ApiType::OwnedString,
        "同一片段在必填 / 可选两位同值"
    );
    assert!(
        !matches!(
            schema_type(&fragment, Position::Field),
            ApiType::Optional(_)
        ),
        "可选字段的 `schema_type` MUST NOT 是 Optional（可选 ≠ 可空）"
    );

    assert_eq!(
        ts_field_line(&ts, "a").trim(),
        "a: string;",
        "必填位零 `?`：\n{ts}"
    );
    assert_eq!(
        ts_field_line(&ts, "b").trim(),
        "b?: string;",
        "可选位只出 `?`（零 `| null`）：\n{ts}"
    );

    assert_eq!(
        rust_field_line(&rust, "a").trim(),
        "pub a: String,",
        "必填位裸类型：\n{rust}"
    );
    assert_eq!(
        rust_field_line(&rust, "b").trim(),
        "pub b: Option<String>,",
        "可选位包 `Option<…>`：\n{rust}"
    );
    assert!(
        !ts.contains("| null"),
        "可选轴零 `| null`（该符号只属可空轴）：\n{ts}"
    );
}

/// 边界（AC-1 (c)，单例）：纯 `null` 一组——`{"type":"null"}` 置于 `required` 字段位 →
/// `Optional(Unknown("null"))` / TS `null` / Rust `serde_json::Value`（显式期望值，零
/// `()`）；该组不参与可空轴的等价式（Rust 侧不出 `Option<`）。
#[test]
fn 三轴对拍纯null一组() {
    let fragment = json!({"type": "null"});
    let (ts, rust) = render_both(&widget_source(&fragment, true));

    assert_eq!(
        schema_type(&fragment, Position::Field),
        ApiType::Optional(Box::new(ApiType::Unknown("null".to_owned()))),
        "纯 null 档判据面：Optional(Unknown(\"null\"))"
    );
    assert_eq!(
        ts_field_line(&ts, "f").trim(),
        "f: null;",
        "TS 纯 null 档落 `null`（MUST NOT `unknown`）：\n{ts}"
    );
    assert_eq!(
        rust_field_line(&rust, "f").trim(),
        "pub f: serde_json::Value,",
        "Rust 纯 null 档落既有兜底 `serde_json::Value`（MUST NOT `()`）：\n{rust}"
    );
    assert!(
        !rust.contains("Option<"),
        "纯 null 档不出 `Option<`——故本组 MUST NOT 参与可空轴等价式：\n{rust}"
    );
}

/// 异常（AC-1 泄漏面，逐组对拍 9 组）：九组语料逐组取渲染文本——TS 文本 `unknown` 零
/// 命中；Rust 文本除纯 null 档外 `serde_json::Value` 零命中（判据认得、渲染不认得的
/// 不对称即失败）。
#[test]
fn 三轴对拍泄漏面九组() {
    let corpus = corpus();
    assert_eq!(corpus.len(), 9, "语料九组（6 可空 + 2 可选 + 1 纯 null）");
    for (label, fragment, required) in corpus {
        let source = widget_source(&fragment, required);
        let (ts, rust) = render_both(&source);
        assert!(
            !ts.contains("unknown"),
            "{label}：TS 文本 `unknown` 零命中（泄漏面即失败）：\n{ts}"
        );
        if fragment == json!({"type": "null"}) {
            assert!(
                rust.contains("serde_json::Value"),
                "{label}：纯 null 档的 Rust 兜底是显式期望值：\n{rust}"
            );
            continue;
        }
        assert!(
            !rust.contains("serde_json::Value"),
            "{label}：Rust 文本 `serde_json::Value` 零命中（泄漏面即失败）：\n{rust}"
        );
    }
}

// ---------------------------------------------------------------------------
// extract_element：TS 入口导出闭包（re-export 沿 `export … from` / `export * from` 走）
// ---------------------------------------------------------------------------

/// 正向（AC-1 / AC-2）：具名名单**精确取**——入口 `export { A } from './x'` 只取 `A`，
/// 目标文件里的 `B` / `C` 零命中（具名名单不虚增）；`file` / `line` 指目标文件内的定义
/// 处，不是入口串。
#[test]
fn extract_element闭包具名名单不虚增() {
    let temp = TempDir::new("closure-named");
    temp.write("views/index.ts", "export { A } from './x';\n");
    temp.write(
        "views/x.ts",
        "export function A(): string {\n  return '';\n}\n\n\
         export function B(): string {\n  return '';\n}\n\n\
         export function C(): string {\n  return '';\n}\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(symbols.len(), 1, "具名名单只取一个名字：{symbols:?}");
    assert_eq!(symbols[0].name, "A");
    assert_eq!(symbols[0].kind, ApiSymbolKind::Function);
    assert_eq!(
        symbols[0].file, "views/x.ts",
        "`file` 指目标文件（MUST NOT 是入口串）"
    );
    assert_eq!(symbols[0].line, 1, "`line` 是目标文件内的定义行");
    for absent in ["B", "C"] {
        assert!(
            !symbols.iter().any(|symbol| symbol.name == absent),
            "未在名单里的 `{absent}` 零命中：{symbols:?}"
        );
    }
}

/// 正向（AC-2）：同一簇「三条逐名 re-export」与「一条 `export * from`」两个入口 ⇒
/// 产出逐字段相等（名字集与顺序均同，各 3 条）。
#[test]
fn extract_element闭包精确名单与通配等价() {
    let temp = TempDir::new("closure-star-equal");
    temp.write(
        "views/x.ts",
        "export function A(): string { return ''; }\n\
         export function B(): string { return ''; }\n\
         export function C(): string { return ''; }\n",
    );
    temp.write(
        "views/named.ts",
        "export { A } from './x';\nexport { B } from './x';\nexport { C } from './x';\n",
    );
    temp.write("views/star.ts", "export * from './x';\n");

    let elements = [element("App.Frontend.Views", &["views/"])];
    let by_named = extract_element(temp.root(), &elements, &elements[0], "views/named.ts");
    let by_star = extract_element(temp.root(), &elements, &elements[0], "views/star.ts");

    let names: Vec<&str> = by_star.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(names, vec!["A", "B", "C"], "各 3 条且顺序 = 声明序");
    assert_eq!(
        by_named, by_star,
        "两条路径逐字段相等（精确名单与通配等价）"
    );
}

/// 正向（AC-1 / AC-2）：多跳闭包——入口自带声明 + `export * from './mid'`，`mid`
/// 自带声明并 `export * from './leaf'` ⇒ 三条全在集，顺序 = 入口语句序 + `mid`
/// 语句序 + `leaf` 声明序。
#[test]
fn extract_element闭包多跳顺序() {
    let temp = TempDir::new("closure-multi-hop");
    temp.write(
        "views/index.ts",
        "export function rootOp(): string { return ''; }\nexport * from './mid';\n",
    );
    temp.write(
        "views/mid.ts",
        "export function midOp(): string { return ''; }\nexport * from './leaf';\n",
    );
    temp.write(
        "views/leaf.ts",
        "export function leafOp(): string { return ''; }\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["rootOp", "midOp", "leafOp"],
        "顺序 = 入口语句序 + 目标文件声明序"
    );
    assert_eq!(
        symbols[2].file, "views/leaf.ts",
        "多跳后 `file` 仍指真正的定义文件：{symbols:?}"
    );
}

/// 正向（AC-1）：`export { A as B } from './x'` ⇒ 入集名 `B`，而 `file` / `line` /
/// `kind` / `is_async` 仍指 `./x` 内 `A` 的定义（改名只改 `name`）。
#[test]
fn extract_element闭包别名改名只改名字() {
    let temp = TempDir::new("closure-alias");
    temp.write("views/index.ts", "export { A as B } from './x';\n");
    temp.write(
        "views/x.ts",
        "// 头部注释\nexport async function A(): Promise<string> { return ''; }\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(symbols.len(), 1, "别名一条：{symbols:?}");
    assert_eq!(symbols[0].name, "B", "对外名 = `exported`");
    assert_eq!(symbols[0].file, "views/x.ts", "定义文件不变（只改名字）");
    assert_eq!(symbols[0].line, 2, "定义行不变（只改名字）");
    assert_eq!(symbols[0].kind, ApiSymbolKind::Function);
    assert!(symbols[0].is_async, "`is_async` 不被改名影响");
}

/// 正向（AC-1 / R-3 本仓形态）：入口同一目标两条语句（`export { V }` + `export type
/// { Actions, Props }`）⇒ 3 名全部在集——`visited` 键含名字集，两条语句互不吞名。
#[test]
fn extract_element闭包同目标两语句不吞名() {
    let temp = TempDir::new("closure-same-target");
    temp.write(
        "views/index.ts",
        "export { V } from './v';\nexport type { Actions, Props } from './v';\n",
    );
    temp.write(
        "views/v.ts",
        "export const V = memo(() => null);\n\
         export type Actions = { a: string };\n\
         export interface Props { b: string }\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(names, vec!["V", "Actions", "Props"], "两条语句互不吞名");
}

/// 正向（AC-4 值域）：闭包内全部符号 `visibility == External`（含跨文件并入的符号），
/// 零 `CrateOnly` / `TestOnly` / `Unused` 产出。
#[test]
fn extract_element闭包值域恒external() {
    let temp = TempDir::new("closure-visibility");
    temp.write(
        "views/index.ts",
        "export function own(): string { return ''; }\nexport { far } from './far';\nexport * from './more';\n",
    );
    temp.write(
        "views/far.ts",
        "export function far(): string { return ''; }\n",
    );
    temp.write(
        "views/more.ts",
        "export interface More { x: number }\nexport enum Mode { A }\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(
        symbols.len(),
        4,
        "声明 + 具名 + 通配两面全并入：{symbols:?}"
    );
    for symbol in &symbols {
        assert_eq!(
            symbol.visibility,
            Visibility::External,
            "闭包内全 External（含跨文件）：{} 为 {:?}",
            symbol.name,
            symbol.visibility
        );
    }
}

/// 正向（AC-2）：入口自带声明 + 具名 re-export + 通配 re-export ⇒ 顺序恒
/// `声明 → 具名 → 通配`，各段内按语句序。
#[test]
fn extract_element闭包混合序() {
    let temp = TempDir::new("closure-mixed-order");
    temp.write(
        "views/index.ts",
        "export function own(): string { return ''; }\n\
         export { namedFirst } from './named';\n\
         export { namedSecond } from './named';\n\
         export * from './starred';\n",
    );
    temp.write(
        "views/named.ts",
        "export function namedFirst(): string { return ''; }\nexport function namedSecond(): string { return ''; }\n",
    );
    temp.write(
        "views/starred.ts",
        "export function starFirst(): string { return ''; }\nexport interface StarType { x: number }\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["own", "namedFirst", "namedSecond", "starFirst", "StarType"],
        "声明段 → 具名段 → 通配段，段内按语句序"
    );
}

/// 边界（AC-3）：环 `a → b → a`（各含一条声明、互 `export *`）⇒ 零 panic、零无限递归，
/// 两符号各恰一次。
#[test]
fn extract_element闭环收敛() {
    let temp = TempDir::new("closure-cycle");
    temp.write(
        "views/a.ts",
        "export function fa(): string { return ''; }\nexport * from './b';\n",
    );
    temp.write(
        "views/b.ts",
        "export function fb(): string { return ''; }\nexport * from './a';\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/a.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(names, vec!["fa", "fb"], "环上两符号各恰一次：{symbols:?}");
}

/// 边界（AC-3）：菱形——入口 `export * from './l'` 与 `export * from './r'`，两者都
/// `export * from './leaf'` ⇒ `leaf` 的符号按 `(name, kind)` 恰一条。
#[test]
fn extract_element闭包菱形去重() {
    let temp = TempDir::new("closure-diamond");
    temp.write(
        "views/index.ts",
        "export * from './l';\nexport * from './r';\n",
    );
    temp.write("views/l.ts", "export * from './leaf';\n");
    temp.write("views/r.ts", "export * from './leaf';\n");
    temp.write(
        "views/leaf.ts",
        "export function shared(): string { return ''; }\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(symbols.len(), 1, "菱形只留一条：{symbols:?}");
    assert_eq!(symbols[0].name, "shared");
}

/// 边界（AC-3）：去重键含 `kind`——两条路径带来**同名不同 kind**（`export type A` 与
/// `export function A`）⇒ 两条都在（去重不按名字或结构合并）。
#[test]
fn extract_element闭包去重键含kind() {
    let temp = TempDir::new("closure-dedupe-kind");
    temp.write(
        "views/index.ts",
        "export * from './l';\nexport * from './r';\n",
    );
    temp.write("views/l.ts", "export type Dup = { a: string };\n");
    temp.write(
        "views/r.ts",
        "export function Dup(): string { return ''; }\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(symbols.len(), 2, "同名不同 kind 两条都在：{symbols:?}");
    let kinds: Vec<ApiSymbolKind> = symbols.iter().map(|symbol| symbol.kind).collect();
    assert_eq!(
        kinds,
        vec![ApiSymbolKind::Type, ApiSymbolKind::Function],
        "去重键 = (name, kind)"
    );
}

/// 边界（AC-3）：同一输入连续两次调用 ⇒ 逐字段相等（零哈希序依赖）；顺序同时与「精确
/// 名单与通配等价」用例互证。
#[test]
fn extract_element闭包稳定序() {
    let temp = TempDir::new("closure-stable");
    temp.write(
        "views/index.ts",
        "export { a } from './x';\nexport * from './y';\nexport { c } from './x';\n",
    );
    temp.write(
        "views/x.ts",
        "export function a(): string { return ''; }\nexport function c(): string { return ''; }\n",
    );
    temp.write("views/y.ts", "export interface Y { value: string }\n");

    let elements = [element("App.Frontend.Views", &["views/"])];
    let first = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    let second = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(first, second, "两次调用逐字段相等（稳定序）");
    let names: Vec<&str> = first.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(names, vec!["a", "c", "Y"], "具名段在前、通配段在后");
}

/// 边界（AC-2 空面，逐值迭代 3 值）：入口空源 / 仅注释 / `export * from './x'` 而
/// `./x` 零导出 ⇒ 空集。
#[test]
fn extract_element闭包空面() {
    let temp = TempDir::new("closure-empty-faces");
    temp.write("views/blank.ts", "");
    temp.write("views/comments.ts", "// 只有注释\nexport {};\n");
    temp.write("views/empty-target.ts", "export * from './nothing';\n");
    temp.write("views/nothing.ts", "// 零导出\n");

    let elements = [element("App.Frontend.Views", &["views/"])];
    for entry in [
        "views/blank.ts",
        "views/comments.ts",
        "views/empty-target.ts",
    ] {
        let symbols = extract_element(temp.root(), &elements, &elements[0], entry);
        assert!(symbols.is_empty(), "空面即空集（{entry}）：{symbols:?}");
    }
}

/// 边界（AC-2 规模，规模迭代 2 夹具）：入口 500 条具名 re-export ⇒ 500 符号零截断零
/// 重复；目标文件 1000 条通配语句 ⇒ 不 panic（环防键命中的重复语句被跳过）。
#[test]
fn extract_element闭包规模() {
    let named = TempDir::new("closure-scale-named");
    let mut target = String::new();
    let mut entry = String::new();
    for index in 0..500 {
        target.push_str(&format!("export function op{index}(): void {{}}\n"));
        entry.push_str(&format!("export {{ op{index} }} from './x';\n"));
    }
    named.write("views/x.ts", &target);
    named.write("views/index.ts", &entry);
    let elements = [element("App.Frontend.Views", &["views/"])];
    let symbols = extract_element(named.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(symbols.len(), 500, "500 条具名 re-export 零截断");
    let mut names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 500, "500 条零重复");

    let starred = TempDir::new("closure-scale-star");
    let mut many = String::new();
    let mut stars = String::new();
    for index in 0..1000 {
        many.push_str(&format!("export function fn{index}(): void {{}}\n"));
    }
    for _ in 0..1000 {
        stars.push_str("export * from './x';\n");
    }
    starred.write("views/x.ts", &many);
    starred.write("views/index.ts", &stars);
    let wide = extract_element(starred.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(
        wide.len(),
        1000,
        "1000 条通配语句不 panic、不重复（环防键跳过重复目标）"
    );
}

/// 边界（AC-4）：`elements = []`（无任何归属）⇒ 所有 re-export 目标判 `Owner::None`
/// 被拒，只剩入口自身的声明。
#[test]
fn extract_element闭包elements空表() {
    let temp = TempDir::new("closure-no-elements");
    temp.write(
        "views/index.ts",
        "export function own(): string { return ''; }\nexport { far } from './far';\n",
    );
    temp.write(
        "views/far.ts",
        "export function far(): string { return ''; }\n",
    );

    let owner = element("App.Frontend.Views", &["views/"]);
    let empty: [ArchiElement; 0] = [];
    let symbols = extract_element(temp.root(), &empty, &owner, "views/index.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(names, vec!["own"], "零归属 ⇒ re-export 全拒、入口声明保留");
}

/// 边界（AC-4，逐值迭代 2 值）：元素表**仅本元素**且 `path` 是文件型入口 ⇒ 兄弟文件
/// 无元素认领、re-export 全拒（入口声明保留）；500 元素表 ⇒ 零 panic、归属仍单点。
#[test]
fn extract_element闭包元素表极端() {
    let single = TempDir::new("closure-single-element");
    single.write(
        "views/index.ts",
        "export function own(): string { return ''; }\nexport { sibling } from './sibling';\n",
    );
    single.write(
        "views/sibling.ts",
        "export function sibling(): string { return ''; }\n",
    );
    // 文件型 `path`：本元素的归属面只有入口自身，兄弟文件无主
    let lone = [element("App.Frontend.Views.Welcome", &["views/index.ts"])];
    let symbols = extract_element(single.root(), &lone, &lone[0], "views/index.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(names, vec!["own"], "兄弟文件无主 ⇒ re-export 被拒");

    // 500 元素表：归属仍单点（`OwnerIndex` 只建一次），结果不变
    let crowded = TempDir::new("closure-many-elements");
    crowded.write("views/index.ts", "export { far } from './far';\n");
    crowded.write(
        "views/far.ts",
        "export function far(): string { return ''; }\n",
    );
    let mut elements: Vec<ArchiElement> = Vec::new();
    for index in 0..499 {
        elements.push(element(
            &format!("App.Frontend.Bulk{index}"),
            &[&format!("views/bulk{index}/")],
        ));
    }
    elements.push(element("App.Frontend.Views", &["views/"]));
    let subject = elements.last().expect("本元素在表尾");
    let symbols = extract_element(crowded.root(), &elements, subject, "views/index.ts");
    assert_eq!(symbols.len(), 1, "500 元素表下归属仍正确：{symbols:?}");
    assert_eq!(symbols[0].name, "far");
}

/// 异常（AC-4，表驱动 4 行）：四条 re-export——① 目标归属本元素子树 ② 归属另一元素
/// ③ 无元素认领 ④ 两元素等长前缀 → `Owner::Ambiguous` ⇒ 只 ① 入集，其余零符号零 panic。
#[test]
fn extract_element闭包同元素约束四值() {
    let temp = TempDir::new("closure-owner-four");
    temp.write(
        "views/agent/index.ts",
        "export { ownOp } from './input/a';\n\
         export { otherOp } from '../other/o';\n\
         export { orphanOp } from '../../shared/s';\n\
         export { ambiguousOp } from '../amb/x';\n",
    );
    temp.write(
        "views/agent/input/a.ts",
        "export function ownOp(): string { return ''; }\n",
    );
    temp.write(
        "views/other/o.ts",
        "export function otherOp(): string { return ''; }\n",
    );
    temp.write(
        "shared/s.ts",
        "export function orphanOp(): string { return ''; }\n",
    );
    temp.write(
        "views/amb/x.ts",
        "export function ambiguousOp(): string { return ''; }\n",
    );

    let elements = [
        element("App.Frontend.Components.Agent", &["views/agent/"]),
        element(
            "App.Frontend.Components.Agent.Input",
            &["views/agent/input/"],
        ),
        element("App.Frontend.Components.Other", &["views/other/"]),
        element("App.Frontend.Components.Amb1", &["views/amb/"]),
        element("App.Frontend.Components.Amb2", &["views/amb/"]),
    ];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/agent/index.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["ownOp"],
        "只有子树命中的一条入集（另一元素 / 无主 / 歧义三条全拒）"
    );
    assert_eq!(
        symbols[0].file, "views/agent/input/a.ts",
        "入集符号仍如实带目标文件"
    );
}

/// 异常（AC-4）：目标归属 `…Components.AgentFoo`（裸前缀相同、按 `.` 分段比较不等）⇒
/// 不入集；对照 `…Components.Agent.Input` 入集。
#[test]
fn extract_element闭包分段前缀不误认() {
    let temp = TempDir::new("closure-segment-prefix");
    temp.write(
        "views/agent/index.ts",
        "export { fooOp } from '../agentfoo/a';\nexport { childOp } from './input/b';\n",
    );
    temp.write(
        "views/agentfoo/a.ts",
        "export function fooOp(): string { return ''; }\n",
    );
    temp.write(
        "views/agent/input/b.ts",
        "export function childOp(): string { return ''; }\n",
    );

    let elements = [
        element("App.Frontend.Components.Agent", &["views/agent/"]),
        element(
            "App.Frontend.Components.Agent.Input",
            &["views/agent/input/"],
        ),
        element("App.Frontend.Components.AgentFoo", &["views/agentfoo/"]),
    ];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/agent/index.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["childOp"],
        "裸前缀相同的兄弟元素不被误认（分段比较）：{symbols:?}"
    );
}

/// 异常（AC-4，逐值迭代 4 值）：仓外包 / 不在盘 / 非 TS 目标（`.json` / `.css`）⇒ 静默
/// 跳过：零符号、零 panic（入口自身声明照常产出）。
#[test]
fn extract_element闭包解析失败静默跳过() {
    let temp = TempDir::new("closure-unresolved");
    temp.write(
        "views/agent/index.ts",
        "export function own(): string { return ''; }\n\
         export { extOp } from 'react-resizable-panels';\n\
         export { missingOp } from './not-on-disk';\n\
         export { jsonOp } from './data.json';\n\
         export { cssOp } from './style.css';\n",
    );
    temp.write("views/agent/data.json", "{\"a\": 1}\n");
    temp.write("views/agent/style.css", ".a { color: red }\n");

    let elements = [element("App.Frontend.Components.Agent", &["views/agent/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "views/agent/index.ts");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["own"],
        "仓外包 / 不在盘 / 非 TS 目标静默跳过：{symbols:?}"
    );
}

/// 边界（entry 边界，表驱动 4 行）：`entry` 超长（>1000 chars）/ 含 emoji 与 `\n` /
/// 零扩展名 / 含 `\0` ⇒ 空集或正常返回，零 panic。
#[test]
fn extract_element的entry边界() {
    let temp = TempDir::new("closure-entry-boundary");
    temp.write(
        "views/index.ts",
        "export function own(): string { return ''; }\n",
    );

    let elements = [element("App.Frontend.Views", &["views/"])];
    let long = format!("views/{}.ts", "a".repeat(1200));
    assert!(long.chars().count() > 1000, "夹具应超 1000 chars");
    let cases = [
        long,
        "views/🎯\nemoji.ts".to_owned(),
        "views/no-extension".to_owned(),
        format!("views/x{}.ts", '\0'),
    ];
    for entry in &cases {
        let symbols = extract_element(temp.root(), &elements, &elements[0], entry);
        assert!(
            symbols.is_empty(),
            "异常 entry 不 panic、不猜符号（{entry:?}）：{symbols:?}"
        );
    }

    // 对照：合法 entry 正常返回（异常 entry 零共享状态）
    let normal = extract_element(temp.root(), &elements, &elements[0], "views/index.ts");
    assert_eq!(normal.len(), 1, "合法入口不受影响：{normal:?}");
}

/// 边界（element 边界，表驱动 3 行）：`path` 空（容器）/ `fqn` 空串 / `fqn` 超长 ⇒
/// 零 panic；`path` 空 ⇒ 归属判据恒 `None`（re-export 全拒，入口声明保留）。
#[test]
fn extract_element的element边界() {
    let temp = TempDir::new("closure-element-boundary");
    temp.write(
        "views/index.ts",
        "export function own(): string { return ''; }\nexport { far } from './far';\n",
    );
    temp.write(
        "views/far.ts",
        "export function far(): string { return ''; }\n",
    );

    let long_fqn = format!("App.{}", "a".repeat(1500));
    let container = element("App.Frontend.Container", &[]);
    let cases = [
        element("App.Frontend.Container", &[]),
        element("", &["views/"]),
        element(&long_fqn, &["views/"]),
    ];
    for subject in &cases {
        let symbols = extract_element(
            temp.root(),
            std::slice::from_ref(&container),
            subject,
            "views/index.ts",
        );
        assert!(
            !symbols.iter().any(|symbol| symbol.name == "far"),
            "异常元素面下 re-export 不被放行（{}）：{symbols:?}",
            subject.fqn
        );
        assert!(
            symbols.iter().any(|symbol| symbol.name == "own"),
            "入口自身声明照常产出（{}）",
            subject.fqn
        );
    }
}

/// 正向（AC-6）：Rust 元素分支与 `rust::extract_crate` 逐字段相等（含 `file`），
/// `elements` / `element` 不参与分派。
#[test]
fn extract_element的rust分支与extract_crate同源() {
    let temp = TempDir::new("closure-rust-branch");
    temp.write(
        "crates/x/src/lib.rs",
        "mod op;\npub use op::run;\n\npub struct Handle;\n",
    );
    temp.write(
        "crates/x/src/op.rs",
        "pub fn run(input: &str) -> String {\n    String::new()\n}\n",
    );

    let elements = [element("App.Core.X", &["crates/x/"])];
    let symbols = extract_element(temp.root(), &elements, &elements[0], "crates/x/src/lib.rs");
    let from_crate = super::rust::extract_crate(temp.root(), "crates/x/src/lib.rs");
    assert_eq!(
        symbols, from_crate,
        "Rust 分支即 extract_crate（逐字段相等，含 file）"
    );
    let run = symbols
        .iter()
        .find(|symbol| symbol.name == "run")
        .expect("子模块的 run 在案");
    assert_eq!(
        run.file, "crates/x/src/op.rs",
        "`file` = 子模块文件（MUST NOT 是入口串）"
    );
}

/// 边界（AC-8 / 单点源码自证）：闭包只减不增与「零第二份解析 / 归属」的机械面——
/// `pub fn extract_element(` 恰 1 处、`format!("{}.",…)` 恰 1 处（分段前缀而非裸
/// `starts_with`）、`TsResolver::new` / `OwnerIndex::build` / 环防键各恰 1 处、
/// `api/` 内零第二份解析器与归属判据、零 `unresolved` 写入。
#[test]
fn 闭包边界单点源码自证() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("pub fn extract_element(").count(),
        1,
        "整元素导出集入口恰 1 处"
    );
    assert_eq!(
        source.matches("format!(\"{}.\",").count(),
        1,
        "FQN 按 `.` 分段比较恰 1 处（MUST NOT 裸 `starts_with`）"
    );
    assert_eq!(
        source.matches("TsResolver::new").count(),
        1,
        "specifier 解析单点只构造一次（每元素一次）"
    );
    assert_eq!(
        source.matches("OwnerIndex::build").count(),
        1,
        "归属索引只建一次"
    );
    assert_eq!(
        source
            .matches("BTreeSet<(String, Option<Vec<String>>)>")
            .count(),
        1,
        "环防键形态恰 1 处"
    );
    for needle in [
        "oxc_resolver",
        "TsconfigOptions",
        "fn owner_of",
        "prefix_matches",
    ] {
        assert!(
            !source.contains(needle),
            "`extract/mod.rs` MUST NOT 命中 `{needle}`（零第二份解析 / 归属）"
        );
    }
    assert!(
        !source.contains("ArchiUnresolvedFile") && !source.contains("unresolved.push"),
        "取数器零 `unresolved` 写入（那是依赖边抽取的账）"
    );

    // 同目录族的其余产品文件同样零第二份解析 / 归属
    for (path, text) in [
        ("check.rs", include_str!("../check.rs")),
        ("contract.rs", include_str!("../contract.rs")),
        ("paths.rs", include_str!("../paths.rs")),
        ("schema.rs", include_str!("../schema.rs")),
        ("diff.rs", include_str!("../diff.rs")),
        ("render.rs", include_str!("../render.rs")),
        ("extract/ts.rs", include_str!("ts.rs")),
        ("extract/rust.rs", include_str!("rust.rs")),
    ] {
        for needle in [
            "oxc_resolver",
            "Resolver::new",
            "TsconfigOptions",
            "fn owner_of",
            "prefix_matches",
        ] {
            assert!(
                !text.contains(needle),
                "`api/{path}` MUST NOT 命中 `{needle}`（零第二份解析 / 归属）"
            );
        }
    }
}

/// 边界（design「私有签名变化」段）：`Closure` 及私有辅助全部**私有**（零 `pub` 命中），
/// 只经 `extract_element` 行为面观测（MUST NOT 为可测性升格）。
#[test]
fn 闭包辅助保持私有() {
    let source = include_str!("mod.rs");
    for private in [
        "pub struct Closure",
        "pub fn collect",
        "pub fn reexport_ok",
        "pub fn export_target",
        "pub fn visited_key",
        "pub fn dedupe",
        "pub fn keeps",
    ] {
        assert!(
            !source.contains(private),
            "闭包辅助 MUST NOT 升格为公开面：`{private}`"
        );
    }
    for defined in [
        "struct Closure<'a>",
        "fn collect(&mut self",
        "fn reexport_ok(",
        "fn export_target(",
        "fn visited_key(",
        "fn dedupe(",
        "fn keeps(",
    ] {
        assert_eq!(
            source.matches(defined).count(),
            1,
            "私有辅助各恰 1 处定义：`{defined}`"
        );
    }
}

/// 边界（AC-5 真值源 / design「类型定义」表）：`ApiSymbol.file` 非 `Option`、紧邻
/// `line`（中间无第二个字段），产品侧 4 处构造点各带 `file:`。
#[test]
fn api_symbol_file字段形状() {
    let source = include_str!("mod.rs");
    assert!(
        source.contains("pub file: String,"),
        "`file` 是非 `Option` 的 `String`"
    );
    assert!(
        !source.contains("pub file: Option"),
        "`file` MUST NOT 是 `Option`（每个符号都有定义文件）"
    );
    let line_at = source.find("pub line: u32").expect("`line` 字段在案");
    let file_at = source.find("pub file: String").expect("`file` 字段在案");
    assert!(file_at > line_at, "`file` 紧随 `line`");
    let between = &source[line_at + "pub line: u32,".len()..file_at];
    assert!(
        !between.contains("    pub "),
        "`file` 与 `line` 相邻（中间无第二个字段）：{between}"
    );

    // 产品侧构造点：`ts::{push_function,push_type}` 与 `rust::{push_type,function_symbol}`
    for (path, text) in [
        ("extract/ts.rs", include_str!("ts.rs")),
        ("extract/rust.rs", include_str!("rust.rs")),
    ] {
        assert_eq!(
            text.matches("file: file.to_owned(),").count(),
            2,
            "`{path}` 恰 2 处构造点各落 `file`"
        );
    }
}

/// 边界（design「修改文件」表模块 doc）：`extract/mod.rs` 的边界句在案——`extract_element`
/// 与 `rust::extract_crate` 按入口与闭包内目标文件读盘、零写盘、零 spawn；TS 分支已走
/// 入口导出闭包。
#[test]
fn 模块doc边界句在案() {
    let source = include_str!("mod.rs");
    for phrase in ["按入口与闭包内目标文件", "零写盘、零 spawn", "入口导出闭包"]
    {
        assert!(
            source.contains(phrase),
            "模块 doc 边界句在案：`{phrase}` 缺失"
        );
    }
}
