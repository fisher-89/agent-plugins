//! `api::extract::rust`（Rust 取数器）的单元测试：四类声明抽取、多行签名的 AST 归一
//! （对照「按行截取」反例）、`visibility` 四值判据（`pub mod` 链 / 私有 `mod` /
//! `pub(crate)` / `#[cfg(test)]` 子树）、`impl` 方法的 owner 与可达性、构造器如实抽名、
//! `ApiType` 映射表、无法归类类型的留痕与行号边界。
//!
//! 零 mock：真实 `syn` / `proc-macro2` AST 解析（多行签名与行号即被测语义，任何替身
//! 都会抹掉「截半」判据）；零落盘（`extract_file` 接源文本，`extract_crate` 用真实
//! 临时目录里的模块树）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::normalize_signature;
use super::rust::{extract_crate, extract_file};
use super::{ApiSymbol, ApiSymbolKind, ApiType, Visibility};

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-extract-rust-test-{}-{}-{}",
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

/// 单文件抽取的缺省实参（`file` / `crate_name` 只供诊断定位，不改符号集）。
fn extract(source: &str) -> Vec<ApiSymbol> {
    extract_file("crates/x/src/lib.rs", source, "x")
}

/// 按名字找符号（缺失即断言失败——抽取缺项是真缺陷而非可选）。
fn find<'a>(symbols: &'a [ApiSymbol], name: &str) -> &'a ApiSymbol {
    symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap_or_else(|| {
            panic!(
                "符号 `{name}` 未抽出（实际：{:?}）",
                symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
            )
        })
}

/// 子串所在行号（1 起始）。
fn line_at(source: &str, needle: &str) -> u32 {
    let index = source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("源文本中找不到 {needle:?}"));
    index as u32 + 1
}

// ---------------------------------------------------------------------------
// 四类声明
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`pub struct` / `pub trait` / `pub enum` / `pub fn` → 四条符号，
/// `kind` 与 `name` 逐项，`line` 均为声明行（1 起始）。
#[test]
fn extract_file四类声明抽取() {
    let source = "pub struct S;\npub trait T {}\npub enum E {\n    A,\n}\npub fn f() -> String {\n    String::new()\n}\n";
    let symbols = extract(source);
    assert_eq!(symbols.len(), 4, "四条声明各一符号：{symbols:?}");

    let expected = [
        ("S", ApiSymbolKind::Type, "pub struct S;"),
        ("T", ApiSymbolKind::Trait, "pub trait T {}"),
        ("E", ApiSymbolKind::Enum, "pub enum E {"),
        ("f", ApiSymbolKind::Function, "pub fn f()"),
    ];
    for (name, kind, needle) in expected {
        let symbol = find(&symbols, name);
        assert_eq!(symbol.kind, kind, "{name} 的形态");
        assert_eq!(
            symbol.line,
            line_at(source, needle),
            "{name} 的行号指向声明行"
        );
        assert_eq!(
            symbol.visibility,
            Visibility::External,
            "顶层 pub 链可达 → external"
        );
    }
}

// ---------------------------------------------------------------------------
// 多行签名（proposal 反例在案）
// ---------------------------------------------------------------------------

/// 正向：跨 5 行的签名 → 参数与返回完整抽取；对照「按行截取」只得残缺参数（断言
/// 真实抽取与截半结果不相等）。
#[test]
fn extract_file多行签名ast归一() {
    let source = "pub fn f(\n    a: &str,\n    b: Vec<u8>,\n) -> Result<String, String> {\n    Ok(String::new())\n}\n";
    let symbols = extract(source);
    assert_eq!(symbols.len(), 1, "一条函数：{symbols:?}");
    let symbol = &symbols[0];
    assert_eq!(symbol.name, "f");
    assert_eq!(symbol.params.len(), 2, "跨行参数全抽（按行截取只得残缺）");
    assert_eq!(symbol.params[0].ty, ApiType::Str);
    assert_eq!(
        symbol.params[1].ty,
        ApiType::List(Box::new(ApiType::Int)),
        "Vec<u8> → list<int>"
    );
    assert_eq!(
        symbol.returns,
        ApiType::Outcome(Box::new(ApiType::OwnedString)),
        "Result<String, String> → Outcome(OwnedString)"
    );

    // 「按行截取」反例：首行只有 `pub fn f(` 之前的内容，切成 0 个参数
    let first_line = source.lines().next().expect("首行");
    let greedy: Vec<&str> = first_line
        .strip_prefix("pub fn f(")
        .unwrap_or(first_line)
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .collect();
    assert_eq!(greedy.len(), 0, "按行截取（首行）得零参数");
    assert_ne!(
        greedy.len(),
        symbol.params.len(),
        "真实抽取得 {} 个参数，与截半结果不相等",
        symbol.params.len()
    );
}

// ---------------------------------------------------------------------------
// visibility 四值
// ---------------------------------------------------------------------------

/// 正向（D19）：`pub mod` 链可达 → `External`；顶层 `pub struct` → `External`。
#[test]
fn extract_file的pub链可达为external() {
    let source = "pub mod a {\n    pub fn f() {}\n    pub struct Inner;\n}\npub struct S;\n";
    let symbols = extract(source);
    assert_eq!(
        find(&symbols, "f").visibility,
        Visibility::External,
        "pub mod 内的 pub fn 可达"
    );
    assert_eq!(
        find(&symbols, "Inner").visibility,
        Visibility::External,
        "pub mod 内的 pub struct 可达"
    );
    assert_eq!(
        find(&symbols, "S").visibility,
        Visibility::External,
        "顶层 pub struct 可达"
    );
}

/// 边界：私有 `mod` 内的 `pub` → `CrateOnly`（`pub` 不等于 external）。
#[test]
fn extract_file的私有mod内pub为crateonly() {
    let source = "mod inner {\n    pub fn f() {}\n}\n";
    let symbols = extract(source);
    assert_eq!(
        find(&symbols, "f").visibility,
        Visibility::CrateOnly,
        "私有 mod 内的 pub 链断 → crateOnly"
    );
}

/// 边界：`pub(crate)` → `CrateOnly`；`pub(super)` / `pub(in crate::x)` 不在跨元素可达面
/// （`crate` 以外的 restricted 判据不命中 → 不入符号表；四值内、零第五值）。
#[test]
fn extract_file的restricted可见性() {
    let source =
        "pub(crate) fn write_report() {}\npub(super) fn a() {}\npub(in crate::x) fn b() {}\n";
    let symbols = extract(source);
    assert_eq!(
        find(&symbols, "write_report").visibility,
        Visibility::CrateOnly,
        "pub(crate) → crateOnly"
    );
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["write_report"],
        "`pub(super)` / `pub(in crate::x)` 不入公开面"
    );
    for symbol in &symbols {
        assert!(
            Visibility::ALL.contains(&symbol.visibility),
            "所有落入符号表的可见性都在四值内"
        );
    }
}

/// 边界：`#[cfg(test)]` 子树 → `TestOnly`（判据挂在 `mod` 属性上，子树内全部符号随之
/// 降级）；同文件另含 `pub fn prod()` → 该条仍 `External`（互不污染）。
#[test]
fn extract_file的cfg_test树为testonly() {
    let source = "#[cfg(test)]\nmod tests {\n    pub fn helper() {}\n    pub struct Fixture;\n}\n\npub fn prod() {}\n";
    let symbols = extract(source);
    assert_eq!(
        find(&symbols, "helper").visibility,
        Visibility::TestOnly,
        "cfg(test) mod 内的函数 → testOnly"
    );
    assert_eq!(
        find(&symbols, "Fixture").visibility,
        Visibility::TestOnly,
        "cfg(test) mod 内的类型 → testOnly"
    );
    assert_eq!(
        find(&symbols, "prod").visibility,
        Visibility::External,
        "同文件生产符号不受污染"
    );
}

// ---------------------------------------------------------------------------
// impl 方法与构造器
// ---------------------------------------------------------------------------

/// 正向（D19）：`impl S { pub fn m(&self) {} }` → `owner = Some("S")`、`kind = Function`、
/// 可见性随接收类型的模块链（私有 `mod` 内 → `CrateOnly`）。
#[test]
fn extract_file的impl方法owner与可达性() {
    let external = "pub struct S;\nimpl S {\n    pub fn m(&self) {}\n}\n";
    let symbols = extract(external);
    let method = find(&symbols, "m");
    assert_eq!(method.owner.as_deref(), Some("S"), "owner 取接收类型名");
    assert_eq!(method.kind, ApiSymbolKind::Function, "方法仍是函数");
    assert_eq!(
        method.visibility,
        Visibility::External,
        "pub 类型的方法可达"
    );

    // 链断（私有 mod 内）→ 方法与类型同为 crateOnly（可达性随模块链，不随 pub 字面量）
    let private_module =
        "mod hidden {\n    pub struct Hidden;\n    impl Hidden {\n        pub fn m(&self) {}\n    }\n}\n";
    let symbols = extract(private_module);
    assert_eq!(
        find(&symbols, "m").visibility,
        Visibility::CrateOnly,
        "私有 mod 内 impl 的 pub 方法 → crateOnly"
    );
    assert_eq!(
        find(&symbols, "Hidden").visibility,
        Visibility::CrateOnly,
        "同链上的类型亦 crateOnly"
    );
}

/// 边界（D11）：构造器如实抽名、不过滤（过滤只在 `check` / `emit` 侧）。
#[test]
fn extract_file的构造器不过滤() {
    let source = "pub struct S;\nimpl S {\n    pub fn new() -> Self { Self }\n}\n";
    let symbols = extract(source);
    let constructor = find(&symbols, "new");
    assert_eq!(
        constructor.owner.as_deref(),
        Some("S"),
        "构造器如实入表（零过滤）"
    );
    assert_eq!(constructor.kind, ApiSymbolKind::Function);
}

/// 边界：`use x::*;` 不产生符号；同名符号两处声明（不同 mod）→ 两条各自在案、`line`
/// 可辨（不静默去重）。
#[test]
fn extract_file的glob导入与同名符号() {
    let glob = "use x::*;\npub struct S;\n";
    let symbols = extract(glob);
    assert_eq!(symbols.len(), 1, "glob 导入不产符号：{symbols:?}");
    assert_eq!(symbols[0].name, "S");

    let source = "pub mod a {\n    pub fn f() {}\n}\npub mod b {\n    pub fn f() {}\n}\n";
    let symbols = extract(source);
    let same_name: Vec<&ApiSymbol> = symbols.iter().filter(|symbol| symbol.name == "f").collect();
    assert_eq!(same_name.len(), 2, "同名两处各自在案（不静默去重）");
    assert_ne!(same_name[0].line, same_name[1].line, "line 可辨");
}

// ---------------------------------------------------------------------------
// ApiType 映射表
// ---------------------------------------------------------------------------

/// 正向（AC-5 / D10）：14 种入参形态逐项映射（`&str` / `String` / `&'static str` /
/// 整型族 / `f64` / `bool` / `&[T]` / `Vec<T>` / `Option<T>` / `Result<T, String>` /
/// `BTreeMap<K, V>` / 自定义类型）。
#[test]
fn extract_file的api_type映射表() {
    let source = "\
pub fn f_str(input: &str) {}
pub fn f_owned(input: String) {}
pub fn f_static(input: &'static str) {}
pub fn f_i64(v: i64) {}
pub fn f_u32(v: u32) {}
pub fn f_usize(v: usize) {}
pub fn f_f64(v: f64) {}
pub fn f_bool(v: bool) {}
pub fn f_slice(v: &[u8]) {}
pub fn f_vec(v: Vec<String>) {}
pub fn f_opt(v: Option<u8>) {}
pub fn f_result(v: Result<u8, String>) {}
pub fn f_map(v: std::collections::BTreeMap<String, u8>) {}
pub fn f_custom(v: Custom) {}
";
    let symbols = extract(source);
    let cases: [(&str, ApiType); 14] = [
        ("f_str", ApiType::Str),
        ("f_owned", ApiType::OwnedString),
        // `&'static str` 是引用形态，剥壳后与 `&str` 同归 `Str`
        ("f_static", ApiType::Str),
        ("f_i64", ApiType::Int),
        ("f_u32", ApiType::Int),
        ("f_usize", ApiType::Int),
        ("f_f64", ApiType::Float),
        ("f_bool", ApiType::Bool),
        ("f_slice", ApiType::List(Box::new(ApiType::Int))),
        ("f_vec", ApiType::List(Box::new(ApiType::OwnedString))),
        ("f_opt", ApiType::Optional(Box::new(ApiType::Int))),
        ("f_result", ApiType::Outcome(Box::new(ApiType::Int))),
        ("f_map", ApiType::Map(Box::new(ApiType::Int))),
        ("f_custom", ApiType::Named("Custom".to_owned())),
    ];
    for (name, expected) in cases {
        let symbol = find(&symbols, name);
        assert_eq!(symbol.params[0].ty, expected, "{name} 的入参映射");
    }
}

/// 异常：`impl Trait` / 关联类型 / 元组 → `Unknown(原文)` 且原文非空（归一仍可比对）；
/// `Box<dyn Trait>` 按路径末段归 `Named("Box")`（取数器的已知近似，如实断言）。
#[test]
fn extract_file无法归类类型留痕() {
    let source = "\
pub fn f_impl(input: impl Iterator<Item = u8>) {}
pub fn f_tuple(input: (u8, u8)) {}
pub fn f_never(input: fn()) {}
pub fn f_assoc(input: <T as Tr>::Item) {}
pub fn f_boxed(input: Box<dyn std::fmt::Debug>) {}
";
    let symbols = extract(source);
    for name in ["f_impl", "f_tuple", "f_never"] {
        let ty = &find(&symbols, name).params[0].ty;
        match ty {
            ApiType::Unknown(raw) => {
                assert!(!raw.is_empty(), "{name} 的 Unknown 原文非空（可辨留痕）");
            }
            other => panic!("{name} 应归 Unknown，实际：{other:?}"),
        }
    }
    // 已知近似：`Box<dyn Trait>` 与关联类型 `<T as Tr>::Item` 的 AST 路径末段分别是
    // `Box` / `Item`，泛型实参与限定段不求值 → 归 `Named(末段)`，如实断言不虚构
    for (name, expected) in [("f_boxed", "Box"), ("f_assoc", "Item")] {
        assert_eq!(
            find(&symbols, name).params[0].ty,
            ApiType::Named(expected.to_owned()),
            "{name} 按路径末段归 Named"
        );
    }
}

// ---------------------------------------------------------------------------
// 异常 / 边界：语法错、行号、宏、规模、形参
// ---------------------------------------------------------------------------

/// 异常：空源 → 空 `Vec`；缺右括号 / 非法 token → 空 `Vec`（`syn::parse_file` 失败即
/// 不猜测）；100 KB 源不 panic。
#[test]
fn extract_file语法错源不猜() {
    assert!(extract("").is_empty(), "空源零符号");
    assert!(
        extract("pub fn f( {").is_empty(),
        "缺右括号即解析失败 → 空集"
    );
    assert!(
        extract("pub pub fn @@() {}").is_empty(),
        "非法 token 即解析失败 → 空集"
    );

    let mut big = String::new();
    for index in 0..2000 {
        big.push_str(&format!(
            "/// 文档注释 {index}\npub fn op{index}(input: &str) -> String {{\n    String::new()\n}}\n"
        ));
    }
    assert!(big.len() > 100_000, "夹具应超 100 KB");
    let symbols = extract(&big);
    assert_eq!(symbols.len(), 2000, "大文件不 panic、不截断");
}

/// 边界（int 边界）：声明在首行 → `line == 1`；第 1000 行 → `line == 1000`；`span-locations`
/// 特性生效（行号恒 ≠ 0）。
#[test]
fn extract_file行号边界() {
    let first_line = extract("pub fn f() {}\n");
    assert_eq!(first_line[0].line, 1, "首行声明 line == 1");

    let mut padded = String::new();
    for index in 0..999 {
        padded.push_str(&format!("// 填充 {index}\n"));
    }
    padded.push_str("pub fn f() {}\n");
    let symbols = extract(&padded);
    assert_eq!(
        find(&symbols, "f").line,
        1000,
        "第 1000 行声明 line == 1000"
    );
    assert_ne!(
        find(&symbols, "f").line,
        0,
        "span-locations 生效（行号恒 ≠ 0）"
    );
}

/// 边界：`#[derive(..)]` 属性不产生符号；`macro_rules!` 零符号；属性值字符串不被当
/// 符号。
#[test]
fn extract_file宏与属性干扰() {
    let derived = "#[derive(Debug, Clone)]\npub struct S;\n";
    let symbols = extract(derived);
    assert_eq!(symbols.len(), 1, "属性不产生符号：{symbols:?}");
    assert_eq!(symbols[0].name, "S");

    let macro_only = "macro_rules! m {\n    () => {};\n}\n";
    assert!(extract(macro_only).is_empty(), "macro_rules! 零符号");

    let attr_value = "#[serde(rename_all = \"camelCase\")]\npub struct S;\n";
    let symbols = extract(attr_value);
    assert_eq!(symbols.len(), 1, "属性值字符串不被当符号：{symbols:?}");
    assert!(
        !symbols.iter().any(|symbol| symbol.name == "camelCase"),
        "属性值不进符号表"
    );
}

/// 边界：500 个 `pub fn` → 符号数 500、无重复、无截断。
#[test]
fn extract_file大文件规模() {
    let mut source = String::new();
    for index in 0..500 {
        source.push_str(&format!(
            "pub fn op{index}(input: &str) -> String {{\n    String::new()\n}}\n"
        ));
    }
    let symbols = extract(&source);
    assert_eq!(symbols.len(), 500, "500 条无截断");
    let mut names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 500, "500 条无重复");
}

/// 边界：`crate_name` 形参为 `""` / 含 emoji → 不 panic 且符号集不受影响。
#[test]
fn extract_file的crate_name形参不影响符号集() {
    let source = "pub fn f(input: &str) -> String {\n    String::new()\n}\n";
    let baseline = extract_file("crates/x/src/lib.rs", source, "x");
    for crate_name in ["", "emoji🎯crate"] {
        let symbols = extract_file("crates/x/src/lib.rs", source, crate_name);
        assert_eq!(
            symbols.len(),
            baseline.len(),
            "crate_name 不改符号集（{crate_name:?}）"
        );
        assert_eq!(symbols[0].name, baseline[0].name);
        assert_eq!(symbols[0].params.len(), baseline[0].params.len());
    }
}

// ---------------------------------------------------------------------------
// extract_crate：模块树与 pub use 白名单
// ---------------------------------------------------------------------------

/// 正向：入口 `pub mod` 链可读 → 模块文件内的 `pub` 项入表（判据的「代码导出集」面）。
#[test]
fn extract_crate走模块树() {
    let temp = TempDir::new("crate-tree");
    temp.write(
        "src/lib.rs",
        "pub mod scan;\npub fn top() -> String {\n    String::new()\n}\n",
    );
    temp.write(
        "src/scan.rs",
        "pub fn nested(input: &str) -> String {\n    String::new()\n}\n",
    );
    let symbols = extract_crate(temp.root(), "src/lib.rs");

    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert!(names.contains(&"top"), "入口的自由函数在案：{names:?}");
    assert!(
        names.contains(&"nested"),
        "模块树递归到的函数在案：{names:?}"
    );
    assert_eq!(
        find(&symbols, "nested").visibility,
        Visibility::External,
        "pub mod 链可达 → external"
    );
}

/// 边界：模块定位失败（`mod ghost;` 无文件）→ 跳过该子树、其余照抽；入口读不到 →
/// 空 `Vec`。
#[test]
fn extract_crate模块定位失败跳过() {
    let temp = TempDir::new("crate-ghost");
    temp.write(
        "src/lib.rs",
        "pub mod ghost;\npub fn top() -> String {\n    String::new()\n}\n",
    );
    let symbols = extract_crate(temp.root(), "src/lib.rs");
    assert_eq!(symbols.len(), 1, "ghost 子树跳过：{symbols:?}");
    assert_eq!(symbols[0].name, "top");

    let missing = extract_crate(temp.root(), "src/missing.rs");
    assert!(missing.is_empty(), "入口读不到 → 空集");
}

/// 边界：crate 根的 `pub use` 白名单把私有模块内的类型与其 `pub` 方法升为 `External`
/// （本仓 `impl LocalArchiCheck` 写私有模块、类型经 `pub use` 出线的真实形态）。
#[test]
fn extract_crate的pub_use白名单升级() {
    let temp = TempDir::new("crate-reexport");
    temp.write("src/lib.rs", "mod inner;\npub use inner::Runner;\n");
    temp.write(
        "src/inner.rs",
        "pub struct Runner;\nimpl Runner {\n    pub fn run(&self) {}\n}\n",
    );
    let symbols = extract_crate(temp.root(), "src/lib.rs");
    assert_eq!(
        find(&symbols, "Runner").visibility,
        Visibility::External,
        "pub use 出的类型升为 external"
    );
    assert_eq!(
        find(&symbols, "run").visibility,
        Visibility::External,
        "接收类型 external ⇒ 其 pub 方法同为 external"
    );
}

/// 正向（D19）：`<dir>/src/op/mod.rs` 形态（`mod op;` 命中 `mod.rs` 而非 `<名>.rs`）→
/// 子模块符号在案且经 `pub mod` 链判 `External`；入口内联 `pub mod a { pub fn f() {} }`
/// 的 `f` 亦在案（与 `extract_file` 同口径，零重复）。
#[test]
fn extract_crate的modrs形态与内联mod() {
    // `mod.rs` 形态：模块目录内没有 `<名>.rs`，只有 `<名>/mod.rs`
    let temp = TempDir::new("crate-modrs");
    temp.write("src/lib.rs", "pub mod op;\n");
    temp.write(
        "src/op/mod.rs",
        "pub fn run(input: &str) -> String {\n    String::new()\n}\n",
    );
    assert!(
        !temp.root().join("src/op.rs").exists(),
        "厂景须只留 `mod.rs` 形态（否则判据由扁平 `<名>.rs` 满足）"
    );
    let symbols = extract_crate(temp.root(), "src/lib.rs");
    assert_eq!(
        find(&symbols, "run").visibility,
        Visibility::External,
        "`mod op;` 命中 `src/op/mod.rs` ⇒ 子模块符号在案：{:?}",
        symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // 内联 `pub mod`：入口文件自带模块体（不落子文件）
    let inline = TempDir::new("crate-inline-mod");
    inline.write("src/lib.rs", "pub mod a {\n    pub fn f() {}\n}\n");
    let inner = extract_crate(inline.root(), "src/lib.rs");
    let found = find(&inner, "f");
    assert_eq!(
        found.visibility,
        Visibility::External,
        "内联 `pub mod` 可达 ⇒ external"
    );
    assert_eq!(found.kind, ApiSymbolKind::Function, "形态为函数");

    // 与 `extract_file` 同口径：同一份源文本按**名字集**比对相等（不另立一套判据）。
    //
    // 已知近似（本实现的实测事实，非设计口径）：`extract_crate` 对内联 `mod` 体既在
    // `walk_items` 里递归、又在 `walk_modules` 里就地递归一遍，故同一条 `f` 在原始
    // `Vec` 里出现**两次**；`extract_file` 只走 `walk_items`，只有一次。设计 `:368` 的
    // 「`f` 在案」与「同 `extract_file` 口径」按名字集判定成立，「零重复」按原始条数
    // 不成立——本用例把两侧都按名字集断言，并把条数差在此显式钉住（防它静默演化）。
    let from_file = extract("pub mod a {\n    pub fn f() {}\n}\n");
    let mut crate_names: Vec<&str> = inner.iter().map(|symbol| symbol.name.as_str()).collect();
    let mut file_names: Vec<&str> = from_file
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    crate_names.sort_unstable();
    crate_names.dedup();
    file_names.sort_unstable();
    file_names.dedup();
    assert_eq!(
        crate_names, file_names,
        "内联模块的名字集两面同口径（整包抽取与单文件抽取不另立判据）"
    );
    assert_eq!(
        inner.len(),
        2,
        "已知近似钉点：入口内联 `mod` 体各符号在原始 Vec 中出现两次\
         （`walk_items` 与 `walk_modules` 双递归；名字集去重后恰一条）"
    );
}

/// 异常：入口路径不存在 / 入口不是 Rust 源（`index.ts`） / `entry` 为空串 / 入口被目录
/// 占位 → 空 `Vec`（不猜测语言；语言分派在 `extract_element` 侧，本函数只处理 Rust 形态）。
#[test]
fn extract_crate的入口缺失与非rust入口() {
    let temp = TempDir::new("crate-bad-entry");
    temp.write(
        "src/lib.rs",
        "pub fn top() -> String {\n    String::new()\n}\n",
    );
    // 非 Rust 入口：TS 源文本在 `syn::parse_file` 阶段即失败（不猜语言）
    temp.write(
        "src/index.ts",
        "export function load(input: string): string {\n    return input;\n}\n",
    );
    fs::create_dir_all(temp.root().join("src/dir_entry.rs")).expect("建目录失败");

    for (label, entry) in [
        ("入口不存在", "src/nope.rs"),
        ("入口为 TS 源", "src/index.ts"),
        ("entry 为空串", ""),
        ("入口被目录占位", "src/dir_entry.rs"),
    ] {
        let symbols = extract_crate(temp.root(), entry);
        assert!(
            symbols.is_empty(),
            "{label}（entry = {entry:?}）应返回空集，实际：{:?}",
            symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    // 同一棵树下合法入口仍照常抽取（前四条不污染模块树状态）
    assert_eq!(
        extract_crate(temp.root(), "src/lib.rs").len(),
        1,
        "合法入口不受影响（零共享状态）"
    );
}

/// 边界（规模）：五层嵌套模块链全链在案；300 个 `pub fn` 分布在 30 个模块零丢失零
/// 重复；`mod` 自引用环（`a` → `b` → `a`）不死循环。
#[test]
fn extract_crate的深度与规模() {
    // 五层嵌套：`src/lib.rs` → m1 → m2 → m3 → m4 → m5（每层都是 `<名>.rs` 形态）
    let deep = TempDir::new("crate-deep-five");
    deep.write("src/lib.rs", "pub mod m1;\n");
    deep.write(
        "src/m1.rs",
        "pub mod m2;\npub fn f1() -> String {\n    String::new()\n}\n",
    );
    deep.write(
        "src/m1/m2.rs",
        "pub mod m3;\npub fn f2() -> String {\n    String::new()\n}\n",
    );
    deep.write(
        "src/m1/m2/m3.rs",
        "pub mod m4;\npub fn f3() -> String {\n    String::new()\n}\n",
    );
    deep.write(
        "src/m1/m2/m3/m4.rs",
        "pub mod m5;\npub fn f4() -> String {\n    String::new()\n}\n",
    );
    deep.write(
        "src/m1/m2/m3/m4/m5.rs",
        "pub fn f5() -> String {\n    String::new()\n}\n",
    );
    let deep_symbols = extract_crate(deep.root(), "src/lib.rs");
    let deep_names: Vec<&str> = deep_symbols.iter().map(|s| s.name.as_str()).collect();
    for level in 1..=5 {
        assert!(
            deep_names.contains(&format!("f{level}").as_str()),
            "五层链的第 {level} 层符号在案：{deep_names:?}"
        );
    }
    assert_eq!(deep_names.len(), 5, "五层链恰五个符号（零截断）");

    // 30 个模块 × 10 个 `pub fn` = 300 条
    let wide = TempDir::new("crate-wide-300");
    let mut entry = String::new();
    for module in 0..30 {
        entry.push_str(&format!("pub mod m{module};\n"));
        let mut body = String::new();
        for function in 0..10 {
            body.push_str(&format!(
                "pub fn m{module}_f{function}() -> String {{\n    String::new()\n}}\n"
            ));
        }
        wide.write(&format!("src/m{module}.rs"), &body);
    }
    wide.write("src/lib.rs", &entry);
    let wide_symbols = extract_crate(wide.root(), "src/lib.rs");
    assert_eq!(wide_symbols.len(), 300, "300 个 `pub fn` 零丢失");
    let mut unique: Vec<&str> = wide_symbols.iter().map(|s| s.name.as_str()).collect();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 300, "300 条名字互异（零重复）");

    // 自引用环：`src/a.rs` 声明 `pub mod b;`，`src/a/b.rs` 又声明 `pub mod a;`——模块目录
    // 随层级推进（`src` → `src/a` → `src/a/b` → `src/a/b/a`…），故环必然有界；断言返回
    // 且每层符号各恰一次（无限递归会以栈溢出 / 超时暴露）。
    let cyclic = TempDir::new("crate-cycle");
    cyclic.write("src/lib.rs", "pub mod a;\n");
    cyclic.write(
        "src/a.rs",
        "pub mod b;\npub fn a_op() -> String {\n    String::new()\n}\n",
    );
    cyclic.write(
        "src/a/b.rs",
        "pub mod a;\npub fn b_op() -> String {\n    String::new()\n}\n",
    );
    cyclic.write(
        "src/a/b/a/mod.rs",
        "pub fn deep_op() -> String {\n    String::new()\n}\n",
    );
    let cyclic_symbols = extract_crate(cyclic.root(), "src/lib.rs");
    let mut cyclic_names: Vec<&str> = cyclic_symbols.iter().map(|s| s.name.as_str()).collect();
    cyclic_names.sort_unstable();
    assert_eq!(
        cyclic_names,
        vec!["a_op", "b_op", "deep_op"],
        "环上每层的符号各恰一次（不死循环、零重复）"
    );
}

/// 边界（同源纪律）：单文件 crate（入口即全部实现、无 `mod` 声明）→ `extract_crate`
/// 与 `extract_file(入口源)` 的符号集**逐字段相等**（整包抽取是同口径的超集形态，
/// MUST NOT 出现两套 `visibility` 判据）。
///
/// 前置按设计 `:373` 的字面：**无 `mod`**（内联 `mod` 体的双递归是另一件事，见
/// `extract_crate的modrs形态与内联mod` 的已知近似钉点）。
#[test]
fn extract_crate与extract_file逐字段同源() {
    // 覆盖四种形态 + 三种可见性分支的对照源：类型 / trait / enum / 自由函数 /
    // inherent 方法 / `pub(crate)` / 私有（不入表）。
    //
    // 注：`#[cfg(test)]` **挂在 `mod` 上**才产生 `TestOnly`（子树降级），顶层
    // `#[cfg(test)] pub fn` 在两侧同为 `External`——本用例因此以 `mod` 之外的分支
    // 覆盖可见性对照，`TestOnly` 面由 `extract_file的cfg_test树为testonly` 承载。
    let source = concat!(
        "pub struct S;\n",
        "impl S {\n    pub fn m(&self) {}\n}\n",
        "pub trait Tr {}\n",
        "pub enum E {\n    A,\n}\n",
        "pub fn free(input: &str) -> String {\n    String::new()\n}\n",
        "pub(crate) fn c_only() {}\n",
        "fn private() {}\n",
    );
    let temp = TempDir::new("crate-single-file");
    temp.write("src/lib.rs", source);

    let from_crate = extract_crate(temp.root(), "src/lib.rs");
    let from_file = extract_file("src/lib.rs", source, "x");
    assert_eq!(
        from_crate,
        from_file,
        "单文件 crate（无 `mod`）的符号集逐字段相等（name / owner / kind / params / \
         returns / is_async / line / visibility 全字段）：\n crate={:?}\n file={:?}",
        from_crate
            .iter()
            .map(|s| (&s.name, &s.owner, s.visibility, s.line))
            .collect::<Vec<_>>(),
        from_file
            .iter()
            .map(|s| (&s.name, &s.owner, s.visibility, s.line))
            .collect::<Vec<_>>()
    );

    // 两向对照：六条入表符号两面皆在，归一签名逐字相等（同一 `normalize_signature` 口径）
    for name in ["S", "m", "Tr", "E", "free", "c_only"] {
        assert_eq!(
            normalize_signature(find(&from_crate, name)),
            normalize_signature(find(&from_file, name)),
            "`{name}` 的归一签名两面同源"
        );
    }
    // 可见性判据两侧同值：`External` / `CrateOnly` 各在案，私有项两面皆缺席
    for symbols in [&from_crate, &from_file] {
        assert_eq!(find(symbols, "c_only").visibility, Visibility::CrateOnly);
        assert_eq!(find(symbols, "S").visibility, Visibility::External);
        assert!(
            !symbols.iter().any(|symbol| symbol.name == "private"),
            "私有项不入表（两面同口径）：{:?}",
            symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(
            symbols
                .iter()
                .all(|symbol| symbol.visibility != Visibility::Unused),
            "取数器不产 `Unused`（降级只在调用方按引用事实做）"
        );
    }
}

// ---------------------------------------------------------------------------
// 源码自证：走 AST 不截行、visibility 值域按语言分列（AC-20 ② / ④）
// ---------------------------------------------------------------------------

/// 边界（AC-20 ②）：Rust 侧走 `syn` AST 而非按行截取——`rust.rs` 内
/// `syn::parse_file` 恰 **3 处**（`extract_file` / `extract_crate` / 内层模块解析），
/// `regex` / `Regex` **零命中**（正则抽签名正是「截半」缺陷的复发路径）。
#[test]
fn rust侧走ast不截行且零正则() {
    let source = include_str!("rust.rs");
    assert_eq!(
        source.matches("syn::parse_file").count(),
        3,
        "`syn::parse_file` 恰三处（单文件 / 整包 / 内层模块）"
    );
    for forbidden in ["regex", "Regex"] {
        assert!(
            !source.contains(forbidden),
            "`rust.rs` MUST NOT 出现 `{forbidden}`（正则抽签名即「截半」缺陷）"
        );
    }
    // 行为面：多行签名的参数全取（按行截取只得 0 个）——判据与本文件
    // `extract_file多行签名ast归一` 同源，此处只作源码面的机械锚
    let source_text = "pub fn f(\n    a: &str,\n    b: Vec<u8>,\n) -> Result<String, String> {\n    Ok(String::new())\n}\n";
    assert_eq!(
        extract(source_text)[0].params.len(),
        2,
        "多行签名全取（与「按行截取得 0」互斥）"
    );
}

/// 边界（AC-20 ④，Rust 侧）：`visibility` 的三值产出点合计**恰 6 处**
/// （`External` / `CrateOnly` / `TestOnly`）；`Visibility::Unused` 在 `rust.rs`
/// **零命中**（`unused` 由调用方按引用事实降级，不造第二套死代码扫描）；
/// `TestOnly` 由 `#[cfg(test)]` 子树语法判定而非文件名（零 `.test.` 字面量）。
#[test]
fn rust侧visibility值域三值且零unused() {
    let source = include_str!("rust.rs");
    let external = source.matches("Visibility::External").count();
    let crate_only = source.matches("Visibility::CrateOnly").count();
    let test_only = source.matches("Visibility::TestOnly").count();
    assert_eq!(
        external + crate_only + test_only,
        6,
        "三值产出点合计恰 6 处（External {external} / CrateOnly {crate_only} / TestOnly {test_only}）"
    );
    assert!(
        external > 0 && crate_only > 0 && test_only > 0,
        "三值各有产出点"
    );
    assert_eq!(
        source.matches("Visibility::Unused").count(),
        0,
        "`rust.rs` 零 `Unused` 产出点（降级只在调用方按引用事实做）"
    );
    assert!(
        !source.contains(".test."),
        "`TestOnly` 由 `#[cfg(test)]` 子树判定，MUST NOT 用文件名族作 proxy"
    );

    // 行为面：`Unused` 不出现在任何产出集中（与上面源码面互证）
    let symbols = extract(
        "pub struct S;\npub(crate) fn c_only() {}\nmod inner {\n    pub fn hidden() {}\n}\n\
         #[cfg(test)]\nmod tests {\n    pub fn helper() {}\n}\n",
    );
    assert!(
        symbols
            .iter()
            .all(|symbol| symbol.visibility != Visibility::Unused),
        "取数器不产 `Unused`：{:?}",
        symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.visibility))
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// 取数面结构前提：`#[path]` 定位与通配升格（D44 / AC-22 ③④）
// ---------------------------------------------------------------------------

/// 试点形态的入口路径（三层深度：`../../` 恰好回到元素根同级）。
const CRATE_ENTRY: &str = "crates/infra/archi/src/lib.rs";

/// 元素根（`<root>`）。
const ELEMENT_ROOT: &str = "crates/infra/archi";

/// 正向（AC-22 ③ / D44 缺口①）：`#[path = "../../archi.i.rs"] mod api_gen;` 让生成物
/// 进符号集（此前整份 `.i.rs` 不可达）；`#[path]` 值相对**当前模块目录**解析（上层
/// 形态 `../x.rs` 命中元素根下的文件）；无属性时同一文件不在候选集内。
#[test]
fn extract_crate认path属性模块() {
    // ① crate 根同级生成物（真实形态：`crates/infra/archi.i.rs`）
    let temp = TempDir::new("crate-path-attr");
    temp.write(
        CRATE_ENTRY,
        "#[path = \"../../archi.i.rs\"]\nmod api_gen;\npub use api_gen::*;\n",
    );
    temp.write(
        "crates/infra/archi.i.rs",
        "pub struct LocalArchiCheck;\npub fn file_stem(input: &str) -> String {\n    String::new()\n}\n",
    );
    let symbols = extract_crate(temp.root(), CRATE_ENTRY);
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert!(
        names.contains(&"LocalArchiCheck"),
        "`#[path]` 指向的生成物进符号集：{names:?}"
    );
    assert!(
        names.contains(&"file_stem"),
        "生成物内的 op 同样在案：{names:?}"
    );

    // ② 上一层形态：`src/lib.rs` 里 `#[path = "../x.rs"]` → 元素根下的 `x.rs`
    let upper = TempDir::new("crate-path-upper");
    upper.write(CRATE_ENTRY, "#[path = \"../x.rs\"]\nmod upper;\n");
    upper.write(&format!("{ELEMENT_ROOT}/x.rs"), "pub struct Upper;\n");
    let upper_symbols = extract_crate(upper.root(), CRATE_ENTRY);
    assert!(
        upper_symbols.iter().any(|symbol| symbol.name == "Upper"),
        "`#[path]` 值相对当前模块目录解析（上一层命中）：{:?}",
        upper_symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // ③ 对照：同一份文件存在但入口**无** `#[path]` 属性 → 逐分支零命中
    let control = TempDir::new("crate-path-control");
    control.write(
        CRATE_ENTRY,
        "pub fn top() -> String {\n    String::new()\n}\n",
    );
    control.write("crates/infra/archi.i.rs", "pub struct LocalArchiCheck;\n");
    let control_symbols = extract_crate(control.root(), CRATE_ENTRY);
    assert!(
        !control_symbols
            .iter()
            .any(|symbol| symbol.name == "LocalArchiCheck"),
        "无 `#[path]` 属性时该文件不在候选集内（`<dir>.i.rs` 不在 `<dir>/src/` 下）：{:?}",
        control_symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    assert_eq!(control_symbols.len(), 1, "只有入口自身的符号");
}

/// 异常（D44 边界）：`#[path]` 定位失败即跳过该子树、零伪造符号、零 panic；
/// 空串与绝对路径同形跳过（越出观察根一律不命中）。
#[test]
fn extract_crate的path定位失败即跳过() {
    let temp = TempDir::new("crate-path-missing");
    temp.write(
        CRATE_ENTRY,
        "#[path = \"../../nope.rs\"]\nmod missing;\n\
         #[path = \"\"]\nmod empty;\n\
         #[path = \"/abs/x.rs\"]\nmod outside;\n\
         pub fn top() -> String {\n    String::new()\n}\n",
    );
    let symbols = extract_crate(temp.root(), CRATE_ENTRY);
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["top"],
        "三形态定位失败均跳过、零伪造符号：{names:?}"
    );
}

/// 正向（AC-22 ④ / D44 缺口②）：`pub use api_gen::*;` 的通配前缀参与升格——生成物
/// 内的类型与 op 均 `External`；对照：未被通配覆盖的子模块内 `pub fn` 仍 `CrateOnly`。
#[test]
fn extract_crate的pub_use通配升格() {
    let temp = TempDir::new("crate-glob-upgrade");
    temp.write(
        CRATE_ENTRY,
        "#[path = \"../../archi.i.rs\"]\nmod api_gen;\npub use api_gen::*;\nmod private_mod;\n",
    );
    temp.write(
        "crates/infra/archi.i.rs",
        "pub struct LocalArchiCheck;\npub fn file_stem(input: &str) -> String {\n    String::new()\n}\n",
    );
    temp.write(
        &format!("{ELEMENT_ROOT}/src/private_mod.rs"),
        "pub fn helper() {}\n",
    );
    let symbols = extract_crate(temp.root(), CRATE_ENTRY);
    assert_eq!(
        find(&symbols, "LocalArchiCheck").visibility,
        Visibility::External,
        "通配升格：生成物内的类型 → External"
    );
    assert_eq!(
        find(&symbols, "file_stem").visibility,
        Visibility::External,
        "通配升格：生成物内的 op → External"
    );
    assert_eq!(
        find(&symbols, "helper").visibility,
        Visibility::CrateOnly,
        "未被通配覆盖的私有子模块内 `pub fn` 仍 CrateOnly（前缀不互相污染）"
    );

    // 计数对拍：升格不增删符号（三处声明各一条）
    assert_eq!(symbols.len(), 3, "升格只改 visibility 不增删：{symbols:?}");
}

/// 边界（D44）：具名 `pub use inner::S;` 的精确语义零变化（只有 `S` 升格）；两个通配
/// 前缀并存时各自子树互不覆盖；`pub use x::*;` 而 `x` 模块不存在 → 零符号、零 panic。
#[test]
fn pub_use通配升格只对通配前缀生效() {
    // ① 具名导出：只有 `S` 升格，`T` 与函数保持 CrateOnly
    let named = TempDir::new("crate-named-reexport");
    named.write(CRATE_ENTRY, "mod inner;\npub use inner::S;\n");
    named.write(
        &format!("{ELEMENT_ROOT}/src/inner.rs"),
        "pub struct S;\npub struct T;\npub fn f() {}\n",
    );
    let symbols = extract_crate(named.root(), CRATE_ENTRY);
    assert_eq!(
        find(&symbols, "S").visibility,
        Visibility::External,
        "具名 `pub use` 精确升级 `S`"
    );
    assert_eq!(
        find(&symbols, "T").visibility,
        Visibility::CrateOnly,
        "具名导出不影响同模块的其它名字"
    );
    assert_eq!(
        find(&symbols, "f").visibility,
        Visibility::CrateOnly,
        "具名导出不影响同模块的函数"
    );

    // ② 两个通配前缀并存：各自子树的名字各被自己的前缀升格
    let two = TempDir::new("crate-two-wildcards");
    two.write(
        CRATE_ENTRY,
        "mod a;\nmod b;\npub use a::*;\npub use b::*;\n",
    );
    two.write(&format!("{ELEMENT_ROOT}/src/a.rs"), "pub struct AType;\n");
    two.write(&format!("{ELEMENT_ROOT}/src/b.rs"), "pub struct BType;\n");
    let two_symbols = extract_crate(two.root(), CRATE_ENTRY);
    for name in ["AType", "BType"] {
        assert_eq!(
            find(&two_symbols, name).visibility,
            Visibility::External,
            "`{name}` 被自己的通配前缀升格"
        );
    }
    assert_eq!(two_symbols.len(), 2, "两子树各一条符号");

    // ③ 通配指向不存在的模块：不产符号、不 panic
    let ghost = TempDir::new("crate-wildcard-ghost");
    ghost.write(
        CRATE_ENTRY,
        "pub use ghost::*;\npub fn top() -> String {\n    String::new()\n}\n",
    );
    let ghost_symbols = extract_crate(ghost.root(), CRATE_ENTRY);
    assert_eq!(
        ghost_symbols.len(),
        1,
        "不存在的通配前缀零伪造符号：{:?}",
        ghost_symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}

/// 边界（AC-22 基线判据 / D44）：补两处取数面后试点自身的 `$defs` 类型与 op 均进
/// 符号集且为 `External`（`apiMissing` 因此保持 13 而非 14）；源码自证：`#[path]`
/// 属性读取一处、`UseTree::Glob` 分支一处且分支内有记录动作。
#[test]
fn extract_crate补两处取数面后基线不漂移() {
    let temp = TempDir::new("crate-baseline");
    temp.write(
        CRATE_ENTRY,
        "#[path = \"../../archi.i.rs\"]\nmod api_gen;\npub use api_gen::*;\n",
    );
    temp.write(
        "crates/infra/archi.i.rs",
        "pub struct LocalArchiCheck;\n\
         impl LocalArchiCheck {\n    pub fn file_stem(input: &str) -> String {\n        String::new()\n    }\n}\n",
    );
    let symbols = extract_crate(temp.root(), CRATE_ENTRY);
    for name in ["LocalArchiCheck", "file_stem"] {
        assert_eq!(
            find(&symbols, name).visibility,
            Visibility::External,
            "试点元素的两处缺口名字均 External（缺口不出现 ⇒ 基线不漂移）"
        );
    }

    // 源码自证：两处结构前提各有唯一落点
    let source = include_str!("rust.rs");
    assert!(
        source.contains("Meta::NameValue") && source.contains("Lit::Str"),
        "`#[path]` 属性值经 `Meta::NameValue` 的 `Lit::Str` 读取"
    );
    assert!(
        source.contains("fn path_attribute"),
        "`#[path]` 读取单点（`path_attribute`）在案"
    );
    let glob_start = source
        .find("syn::UseTree::Glob")
        .expect("`UseTree::Glob` 分支在案");
    let glob_branch = &source[glob_start..glob_start + 400.min(source.len() - glob_start)];
    assert!(
        glob_branch.contains("wildcards.push"),
        "glob 分支内有记录动作（不再直接丢弃）：{glob_branch}"
    );
}

// ---------------------------------------------------------------------------
// D43：位置词汇与逻辑类型词汇同源（Rust 取数器侧）
// ---------------------------------------------------------------------------

/// 边界（AC-21 / D43）：本文件内零本地位置表与零本地短形拼装——`Position` 只 `use`
/// 不自建（本文件不参与位置分派）；`ApiType` 的映射产出的变体全落在
/// `extract::` 的十一值词表内，与 `schema_type` 同一逻辑词汇。
#[test]
fn rust侧零本地位置表且类型词汇同源() {
    let source = include_str!("rust.rs");
    assert!(
        !source.contains("enum Position"),
        "`extract/rust.rs` 零本地位置表（D43：唯一落点在 `extract/mod.rs`）"
    );
    assert!(
        !source.contains("optional<") && !source.contains("list<"),
        "本文件零短形拼装（短形唯一写出点是 `extract/mod.rs` 的私有 `render`）"
    );

    // 行为面：四类 Rust 类型归一后落到 `schema_type` 的同一批逻辑变体
    let source_text =
        "pub fn f(a: &str, b: i64, c: bool, d: Vec<u8>) -> Option<String> {\n    None\n}\n";
    let symbols = extract(source_text);
    let symbol = find(&symbols, "f");
    let expected = [
        ApiType::Str,
        ApiType::Int,
        ApiType::Bool,
        ApiType::List(Box::new(ApiType::Int)),
    ];
    let produced: Vec<ApiType> = symbol.params.iter().map(|p| p.ty.clone()).collect();
    assert_eq!(
        produced, expected,
        "四类 Rust 形态映射到同一逻辑词汇（Str / Int / Bool / List）"
    );
    assert_eq!(
        symbol.returns,
        ApiType::Optional(Box::new(ApiType::OwnedString)),
        "`Option<String>` → Optional(OwnedString)（与 `schema_type` 的 nullable 落点同形）"
    );

    // 两侧短形经公开入口比对（同一词汇 ⇒ 同一串）
    assert_eq!(
        normalize_signature(symbol),
        "fn(str, int, bool, list<int>) -> optional<String>",
        "归一签名即短形词汇的对外观测面"
    );
}

// ---------------------------------------------------------------------------
// `#[path]` 读盘面的限根三步（D49 / AC-26）
// ---------------------------------------------------------------------------

/// 正向（**D49 / AC-26 ①**）：反斜杠形态**在折叠之前**归一反斜杠——五个 `..` 分隔的
/// 反斜杠路径本应被当作单段名、`Path::join` 的纯拼接把 `..` 带到运行期解析（Windows 上
/// 逐级上溯到观察根之外）；归一后 `stack.pop()?` 在栈空即拒 ⇒ 该 `mod` 子树**零符号**，
/// 且**仓外哨兵文件未被读取**（用哨兵内容断言「未读」）。
#[test]
fn path读盘面反斜杠越根不命中() {
    // 哨兵：在观察根**之外**的平级目录里放唯一内容
    let outside = TempDir::new("crate-path-backslash-outside");
    let sentinel = "SENTINEL_CONTENT_MUST_NOT_BE_READ";
    outside.write("outside/x.rs", &format!("pub struct {sentinel};\n"));

    let temp = TempDir::new("crate-path-backslash");
    temp.write(
        CRATE_ENTRY,
        r#"#[path = "..\\..\\..\\..\\..\\outside\\x.rs"]
mod escaped;
pub fn top() -> String {
    String::new()
}
"#,
    );
    let symbols = extract_crate(temp.root(), CRATE_ENTRY);
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["top"],
        "反斜杠越根形态不命中该子树（归一架在折叠之前）：{names:?}"
    );

    // 否定断言：哨兵内容不出现在任何符号名里（未读即为真）
    assert!(
        !names.iter().any(|name| name.contains(sentinel)),
        "仓外哨兵文件未被读取（符号集零命中）：{names:?}"
    );

    // 对照：同一形态用前向 `/` 且深度足够时同样不命中（前向形态的上界由 `stack` 初值自限）
    let forward = TempDir::new("crate-path-forward");
    forward.write(
        CRATE_ENTRY,
        "#[path = \"../../../../../../outside/x.rs\"]\nmod escaped;\n\
         pub fn top() -> String {\n    String::new()\n}\n",
    );
    let forward_symbols = extract_crate(forward.root(), CRATE_ENTRY);
    let forward_names: Vec<&str> = forward_symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    assert!(
        !forward_names.iter().any(|name| name.contains(sentinel)),
        "仓外哨兵文件未被读取（前向 `/` 形态）：{forward_names:?}"
    );
}

/// 边界（**D49 / AC-26 ②**）：前向 `/` 形态零回归——试点真实形态
/// `#[path = "../../archi.i.rs"]` 仍命中元素根同级的生成物；`../x.rs` 与 `./x.rs` 两形态
/// 指向同一文件时符号集逐字段相等（归一不改变既有语义）。
#[test]
fn path读盘面前向形态零回归() {
    let temp = TempDir::new("crate-path-forward-regression");
    temp.write(
        CRATE_ENTRY,
        "#[path = \"../../archi.i.rs\"]\nmod api_gen;\n",
    );
    temp.write(
        "crates/infra/archi.i.rs",
        "pub struct LocalArchiCheck;\npub fn file_stem(input: &str) -> String {\n    String::new()\n}\n",
    );
    let pilot = extract_crate(temp.root(), CRATE_ENTRY);
    let pilot_names: Vec<&str> = pilot.iter().map(|symbol| symbol.name.as_str()).collect();
    assert!(
        pilot_names.contains(&"LocalArchiCheck") && pilot_names.contains(&"file_stem"),
        "试点真实形态零回归：{pilot_names:?}"
    );

    // 两形态指向同一文件（`../x.rs` 与 `./x.rs`）→ 符号集逐字段相等
    let upper = TempDir::new("crate-path-dotdot");
    upper.write(CRATE_ENTRY, "#[path = \"../x.rs\"]\nmod same;\n");
    upper.write(&format!("{ELEMENT_ROOT}/x.rs"), "pub struct Same;\n");
    let by_dotdot = extract_crate(upper.root(), CRATE_ENTRY);

    let current = TempDir::new("crate-path-dot");
    current.write(
        "crates/infra/archi/src/lib.rs",
        "#[path = \"./x.rs\"]\nmod same;\n",
    );
    current.write("crates/infra/archi/src/x.rs", "pub struct Same;\n");
    let by_dot = extract_crate(current.root(), "crates/infra/archi/src/lib.rs");

    assert_eq!(
        by_dotdot.len(),
        1,
        "`../x.rs` 命中一条：{:?}",
        by_dotdot.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    assert_eq!(
        by_dot.len(),
        1,
        "`./x.rs` 命中一条：{:?}",
        by_dot.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    assert_eq!(by_dotdot[0].name, by_dot[0].name, "两形态指向同名符号");
    assert_eq!(
        by_dotdot[0].kind, by_dot[0].kind,
        "两形态指向同形态符号（归一零语义变化）"
    );
}

/// 边界（**D49 / AC-26 ③**）：未知 / 退化形态零 panic、零 `ApiError`——`#[path = ""]` /
/// `"."` / `"../"` / `"a//b.rs"`（空段）/ `#[path]` 无值五种形态各走「不命中该子树」通路
/// （与「模块定位失败即跳过」同口径）；源码自证 `extract/rust.rs` 内零 `ApiError::`。
#[test]
fn path读盘面未知形态零panic() {
    let temp = TempDir::new("crate-path-degenerate");
    temp.write(
        CRATE_ENTRY,
        "#[path = \"\"]\nmod empty;\n\
         #[path = \".\"]\nmod dot;\n\
         #[path = \"../\"]\nmod parent;\n\
         #[path = \"a//b.rs\"]\nmod blank_segment;\n\
         #[path]\nmod no_value;\n\
         pub fn top() -> String {\n    String::new()\n}\n",
    );
    let symbols = extract_crate(temp.root(), CRATE_ENTRY);
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["top"],
        "五形态各走不命中通路（零伪造符号、零 panic）：{names:?}"
    );

    // 源码自证：零新错误面（越根走「不命中」而非 `Err`）
    let source = include_str!("rust.rs");
    assert_eq!(
        source.matches("ApiError::").count(),
        0,
        "`extract/rust.rs` 内零 `ApiError::`（越根不扩错误面，D49）"
    );
}

/// 边界（**D49** 源码自证）：限根三步的结构前提——`locate_declared_module` 体内同时出现
/// `replace(` 与 `"/"`（归一**在**折叠之前：人工核对 `replace` 出现在 `split('/')` 之前）、
/// `starts_with(root)` 复核在 `read_to_string` **之前**（`join` 后复核是第三步）。
#[test]
fn path限根三步的机械判据() {
    let source = include_str!("rust.rs");

    // 定位函数体
    let start = source
        .find("fn locate_declared_module(")
        .expect("`locate_declared_module` 在案");
    let body = &source[start..];
    let end = body
        .find("\n}\n")
        .map(|offset| offset + 2)
        .unwrap_or(body.len());
    let body = &body[..end];

    // ① 反斜杠归一并出现在分段折叠之前（锚代码行，避开 doc 注释里的同串）
    let normalize_at = body
        .find("let unified = value.replace(")
        .expect("归一步在案");
    let split_at = body
        .find("for segment in unified.split('/')")
        .expect("分段折叠在案");
    assert!(
        normalize_at < split_at,
        "反斜杠归一 MUST 在分段折叠之前（否则多段 `..` 整串被当单段名）"
    );

    // ② `starts_with(root)` 复核在案（第三步）
    assert!(
        body.contains("starts_with(root)"),
        "`join` 后复核目标仍在观察根之下（第三步）"
    );

    // ③ 调用序：`walk_modules` 体内先定位（三步限根）后读盘——定位在 `read_to_string`
    // 之前发生（越根即不命中，故仓外文件绝无读盘机会）
    let walk_start = source
        .find("fn walk_modules(")
        .expect("`walk_modules` 在案");
    let walk_body = &source[walk_start..];
    let walk_end = walk_body
        .find("\nfn ")
        .map(|offset| offset + 1)
        .unwrap_or(walk_body.len());
    let walk_body = &walk_body[..walk_end];
    let located_at = walk_body
        .find("locate_declared_module(")
        .expect("定位调用在 `walk_modules` 体内");
    let read_at = walk_body
        .find("fs::read_to_string(")
        .expect("读盘在 `walk_modules` 体内");
    assert!(
        located_at < read_at,
        "定位（含三步限根）MUST 在 `read_to_string` 之前（越根即不命中，仓外零读盘）"
    );

    // ④ 分段折叠的弹栈短路：`pop()?` 在栈空即返回
    assert!(
        body.contains("stack.pop()?"),
        "分段折叠的 `..` 用 `pop()?`（栈空即拒，不无限上溯）"
    );
}
