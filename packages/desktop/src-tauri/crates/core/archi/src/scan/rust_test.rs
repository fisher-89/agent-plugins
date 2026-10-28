//! `scan::rust` 的单元测试：workspace 清单 + member glob → crate 清单 → 模块树
//! 四种形态（`mod foo;` / `foo/mod.rs` / `#[path]` / `cfg` 两臂），`use` 前缀四
//! 形态的最长前缀反查，外部 crate 静默丢弃，以及建树长尾 / `use` 解析失败的逐条
//! 留痕。
//!
//! 零 mock：真实 `syn 2`（`features = ["full"]`）+ 真实 `toml` 清单解析 + 真实
//! 临时目录（`#[path]` 与孤儿文件判定必须真盘）；**零子进程**（不调
//! `cargo metadata`）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::rust::RustResolver;

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-rust-test-{}-{}-{}",
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

    /// 落一个 repo 相对文件（自动建父目录）。
    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }

    /// 观察根的 POSIX 形态（member glob 条目按枚举到的目录路径匹配，见下）。
    fn posix_root(&self) -> String {
        self.0.to_string_lossy().replace('\\', "/")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 单 crate（观察根自身即 crate 根，无 `[workspace]` 成员）→ 返回入口文件 repo
/// 相对路径。
fn single_crate(tag: &str, lib_rs: &str) -> (TempDir, String) {
    let temp = TempDir::new(tag);
    temp.write(
        "Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    temp.write("src/lib.rs", lib_rs);
    (temp, "src/lib.rs".to_owned())
}

/// 双 crate workspace：成员条目的匹配面是**枚举到的目录路径**（与观察根同锚），
/// 逐个 member 的入口登记同在该面上——故夹具按该面给出具体 glob，用例亦以同一
/// 形态的入口路径查询（`member_glob` 展开语义在案）。
fn double_crate(tag: &str, alpha_lib: &str, beta_lib: &str) -> (TempDir, String, String) {
    let temp = TempDir::new(tag);
    let root = temp.posix_root();
    temp.write(
        "Cargo.toml",
        &format!("[workspace]\nmembers = [\"{root}/crates/*\"]\n"),
    );
    temp.write(
        "crates/alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
    );
    temp.write(
        "crates/beta/Cargo.toml",
        "[package]\nname = \"beta\"\nversion = \"0.1.0\"\n",
    );
    temp.write("crates/alpha/src/lib.rs", alpha_lib);
    temp.write("crates/beta/src/lib.rs", beta_lib);
    let alpha_entry = format!("{root}/crates/alpha/src/lib.rs");
    let beta_entry = format!("{root}/crates/beta/src/lib.rs");
    (temp, alpha_entry, beta_entry)
}

/// 留痕文案（`unresolved` 的 message 清单）。
fn messages(resolver: &RustResolver) -> Vec<String> {
    resolver
        .unresolved()
        .iter()
        .map(|entry| entry.message.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// 正向：清单解析与模块树四形态
// ---------------------------------------------------------------------------

/// 正向：workspace `members` glob 展开生效——两 member 的 crate 均可被 `resolve`
/// 命中（零子进程）。
#[test]
fn new清单解析与member_glob展开() {
    let (temp, alpha_entry, beta_entry) = double_crate("members", "mod foo;\n", "mod bar;\n");
    let root = temp.posix_root();
    temp.write("crates/alpha/src/foo.rs", "pub struct Bar;\n");
    temp.write("crates/beta/src/bar.rs", "pub struct Baz;\n");

    let resolver = RustResolver::new(temp.root()).expect("合法 workspace 清单应可解析");
    assert_eq!(
        resolver.resolve(&alpha_entry, "crate::foo::Bar"),
        Some(format!("{root}/crates/alpha/src/foo.rs")),
        "第一个 member 可命中（glob 展开生效）"
    );
    assert_eq!(
        resolver.resolve(&beta_entry, "crate::bar::Baz"),
        Some(format!("{root}/crates/beta/src/bar.rs")),
        "第二个 member 可命中"
    );
}

/// 正向（AC-3 形态 1）：`mod foo;` + `src/foo.rs` → `crate::foo::Bar` 落
/// `src/foo.rs`。
#[test]
fn 模块树形态一mod_文件() {
    let (temp, entry) = single_crate("mod-file", "mod foo;\n");
    temp.write("src/foo.rs", "pub struct Bar;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    assert_eq!(
        resolver.resolve(&entry, "crate::foo::Bar"),
        Some("src/foo.rs".to_owned()),
        "mod foo; 反查到 foo.rs"
    );
}

/// 正向（AC-3 形态 2）：`foo/mod.rs` + `foo/bar.rs` → 两条 use 各落各自文件。
#[test]
fn 模块树形态二目录形态() {
    let (temp, entry) = single_crate("mod-dir", "mod foo;\n");
    temp.write("src/foo/mod.rs", "mod bar;\n");
    temp.write("src/foo/bar.rs", "pub struct Baz;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    assert_eq!(
        resolver.resolve(&entry, "crate::foo::Thing"),
        Some("src/foo/mod.rs".to_owned()),
        "目录形态反查到 foo/mod.rs"
    );
    assert_eq!(
        resolver.resolve(&entry, "crate::foo::bar::Baz"),
        Some("src/foo/bar.rs".to_owned()),
        "目录形态下的子模块反查到 foo/bar.rs"
    );
}

/// 正向（AC-3 形态 3）：`#[path = "custom/x.rs"] mod x;` 覆盖默认推导（同目录另有
/// `x.rs` 时取 `#[path]`，未被纳入的 `x.rs` 留痕为孤儿）。
#[test]
fn 模块树形态三path属性优先() {
    let (temp, entry) = single_crate("path-attr", "#[path = \"custom/x.rs\"]\nmod x;\n");
    temp.write("src/custom/x.rs", "pub struct Y;\n");
    temp.write("src/x.rs", "pub struct Default;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    assert_eq!(
        resolver.resolve(&entry, "crate::x::Y"),
        Some("src/custom/x.rs".to_owned()),
        "#[path] 覆盖默认推导"
    );
    assert!(
        messages(&resolver)
            .iter()
            .any(|message| message.contains("孤儿")),
        "未被 #[path] 纳入的 x.rs 应留痕为孤儿：{:?}",
        messages(&resolver)
    );
}

/// 边界（AC-3 形态 4）：`#[cfg(unix)] mod a;` + `#[cfg(windows)] mod a;` 两臂同
/// 路径 → 建树不报重复、无留痕、落同一文件。
#[test]
fn 模块树形态四cfg两臂同路径() {
    let (temp, entry) = single_crate(
        "cfg-arms",
        "#[cfg(unix)]\nmod a;\n#[cfg(windows)]\nmod a;\n",
    );
    temp.write("src/a.rs", "pub struct Shared;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    assert!(
        messages(&resolver).is_empty(),
        "两臂同路径不应报重复：{:?}",
        messages(&resolver)
    );
    assert_eq!(
        resolver.resolve(&entry, "crate::a::Shared"),
        Some("src/a.rs".to_owned()),
        "两臂同落同一文件"
    );
}

/// 边界（D23）：crate 表按**源码标识符形态**登记（清单名 `archi-runtime` → 键
/// `archi_runtime`）——`use archi_runtime::…` 命中该 crate，连字符形态不是标识符
/// （按外部 crate 静默丢弃）。
#[test]
fn 清单名连字符登记为标识符() {
    let temp = TempDir::new("hyphen-crate");
    let root = temp.posix_root();
    temp.write(
        "Cargo.toml",
        &format!("[workspace]\nmembers = [\"{root}/crates/*\"]\n"),
    );
    temp.write(
        "crates/alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
    );
    temp.write("crates/alpha/src/lib.rs", "pub struct A;\n");
    temp.write(
        "crates/runtime/Cargo.toml",
        "[package]\nname = \"archi-runtime\"\nversion = \"0.1.0\"\n",
    );
    temp.write("crates/runtime/src/lib.rs", "pub struct Runner;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let entry = format!("{root}/crates/alpha/src/lib.rs");
    let runtime_entry = format!("{root}/crates/runtime/src/lib.rs");
    assert_eq!(
        resolver.resolve(&entry, "archi_runtime::Runner"),
        Some(runtime_entry.clone()),
        "连字符清单名按标识符形态命中"
    );
    assert_eq!(
        resolver.resolve(&entry, "archi_runtime::unknown::X"),
        Some(runtime_entry),
        "未命中模块降级到该 crate 根（不是 None）"
    );
    assert_eq!(
        resolver.resolve(&entry, "archi-runtime::Runner"),
        None,
        "连字符形态不是源码标识符 ⇒ 按外部 crate 静默丢弃"
    );
}

/// 边界（D23）：`#[path = "…"]` 取值先归一（`\` → `/`、折叠 `.`）并**限根**——
/// 上溯越过观察根的形态不采信（留痕「定位不到文件」，模块树不被带出仓外）。
#[test]
fn path属性归一与限根() {
    let (temp, entry) = single_crate(
        "path-root-limit",
        "#[path = \"custom\\\\x.rs\"]\nmod x;\n#[path = \"./nested/y.rs\"]\nmod y;\n#[path = \"../../out/z.rs\"]\nmod z;\n",
    );
    temp.write("src/custom/x.rs", "pub struct X;\n");
    temp.write("src/nested/y.rs", "pub struct Y;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    assert_eq!(
        resolver.resolve(&entry, "crate::x::X"),
        Some("src/custom/x.rs".to_owned()),
        "`\\` 分隔与 `./` 前缀归一后落盘"
    );
    assert_eq!(
        resolver.resolve(&entry, "crate::y::Y"),
        Some("src/nested/y.rs".to_owned()),
        "`./` 前缀归一后落盘"
    );
    let escape = messages(&resolver);
    assert!(
        escape
            .iter()
            .any(|message| message.contains("mod z 定位不到文件")),
        "上溯出仓的 #[path] 不采信并留痕：{escape:?}"
    );
    assert_eq!(
        resolver.resolve(&entry, "crate::z::Z"),
        Some(entry.clone()),
        "被拒模块不入树 ⇒ 降级到 crate 根（MUST NOT 带出仓外）"
    );
}

/// 边界：内联 `mod inner { … }` 只登记路径不产文件（映射回本文件）。
#[test]
fn 内联模块不产文件() {
    let (temp, entry) = single_crate(
        "inline-mod",
        "mod y;\nmod inner {\n    pub fn helper() {}\n}\n",
    );
    temp.write("src/y.rs", "pub struct Z;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    assert_eq!(
        resolver.resolve(&entry, "crate::inner::helper"),
        Some("src/lib.rs".to_owned()),
        "内联模块映射回所在文件（不产新文件）"
    );
    assert_eq!(
        resolver.resolve(&entry, "crate::y::Z"),
        Some("src/y.rs".to_owned()),
        "同文件的 use 仍按模块路径定位"
    );
}

// ---------------------------------------------------------------------------
// 正向：use 前缀四形态与最长前缀
// ---------------------------------------------------------------------------

/// 正向：`crate::` / `self::` / `super::` / workspace crate 名四形态各落各自文件
/// （第四落被引 crate 的入口 `lib.rs`）。
#[test]
fn use前缀四形态() {
    let (temp, entry, beta_entry) = double_crate(
        "use-prefixes",
        "mod a;\nmod x;\nmod z;\n",
        "pub struct P;\n",
    );
    let root = temp.posix_root();
    temp.write("crates/alpha/src/a.rs", "pub struct B;\n");
    temp.write("crates/alpha/src/x.rs", "pub struct Y;\n");
    temp.write("crates/alpha/src/z.rs", "pub struct W;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");

    let (facts, unresolved) = resolver.extract(
        &entry,
        "use crate::a::B;\nuse self::x::Y;\nuse super::z::W;\nuse beta::P;\n",
    );
    assert!(unresolved.is_empty(), "四条 use 均可解析");
    let specifiers: Vec<&str> = facts.iter().map(|fact| fact.specifier.as_str()).collect();
    assert_eq!(
        specifiers,
        vec!["crate::a::B", "self::x::Y", "super::z::W", "beta::P"],
        "四条 use 的规范化路径逐字"
    );

    assert_eq!(
        resolver.resolve(&entry, "crate::a::B"),
        Some(format!("{root}/crates/alpha/src/a.rs")),
        "crate:: 从 crate 根拼"
    );
    assert_eq!(
        resolver.resolve(&entry, "self::x::Y"),
        Some(format!("{root}/crates/alpha/src/x.rs")),
        "self:: 按当前模块路径"
    );
    assert_eq!(
        resolver.resolve(&entry, "super::z::W"),
        Some(format!("{root}/crates/alpha/src/z.rs")),
        "super:: 按模块路径栈上溯"
    );
    assert_eq!(
        resolver.resolve(&entry, "beta::P"),
        Some(beta_entry.clone()),
        "workspace crate 名从该 crate 入口起拼"
    );
}

/// 边界：`crate::a` 与 `crate::a::b` 均建树 → 取最长前缀（落 `b` 的文件）。
#[test]
fn 最长前缀匹配() {
    let (temp, entry) = single_crate("longest", "mod a;\n");
    temp.write("src/a.rs", "mod b;\npub struct A;\n");
    temp.write("src/a/b.rs", "pub struct C;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    assert_eq!(
        resolver.resolve(&entry, "crate::a::b::C"),
        Some("src/a/b.rs".to_owned()),
        "最长前缀落 b.rs（不落 a.rs）"
    );
    assert_eq!(
        resolver.resolve(&entry, "crate::a::A"),
        Some("src/a.rs".to_owned()),
        "较浅的 use 仍落 a.rs"
    );
}

/// 异常（AC-3）：`std` / 第三方 crate → `resolve` 恒 `None`，且 `extract` 不把它
/// 们记入 `unresolved`（零副作用）。
#[test]
fn 外部crate静默丢弃() {
    let (temp, entry) = single_crate("external", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    for specifier in ["std::collections::HashMap", "serde::Deserialize"] {
        assert_eq!(
            resolver.resolve(&entry, specifier),
            None,
            "{specifier} 是外部 crate，不进归属"
        );
    }
    let (facts, unresolved) = resolver.extract(
        &entry,
        "use std::collections::HashMap;\nuse serde::Deserialize;\n",
    );
    assert_eq!(
        facts.len(),
        2,
        "外部 crate 的 use 仍算声明（只解析落盘失败）"
    );
    assert!(unresolved.is_empty(), "外部 crate 不记 unresolved");
}

// ---------------------------------------------------------------------------
// 异常：建树长尾留痕
// ---------------------------------------------------------------------------

/// 异常（AC-3）：宏生成 `mod` / `include!` / 孤儿 `.rs` / `mod` 定位不到文件 四类
/// 各一条留痕（`file` / `message` 非空）。
#[test]
fn 建树长尾留痕四形态() {
    let (temp, _entry) = single_crate(
        "long-tail",
        "macro_rules! make_mod {\n    () => { mod generated; };\n}\ninclude!(\"gen.rs\");\nmod ghost;\n",
    );
    temp.write("src/orphan.rs", "pub struct Orphan;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let messages = messages(&resolver);
    for needle in ["macro_rules!", "include!", "孤儿", "定位不到文件"] {
        assert!(
            messages.iter().any(|message| message.contains(needle)),
            "缺少 {needle} 的留痕：{messages:?}"
        );
    }
    assert!(
        resolver
            .unresolved()
            .iter()
            .all(|entry| !entry.file.is_empty() && !entry.message.is_empty()),
        "留痕条目 file / message 均非空"
    );
}

/// 异常：截断的 `.rs` / 未闭合泛型 → 一条 `unresolved` 且不 panic（零 fact）。
#[test]
fn extract语法错留痕() {
    let (temp, entry) = single_crate("syntax-error", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    for source in ["use crate::a::B;\nfn broken(\n", "pub struct X<T {\n"] {
        let (facts, unresolved) = resolver.extract(&entry, source);
        assert!(facts.is_empty(), "语法错不产 fact");
        assert_eq!(unresolved.len(), 1, "语法错一条留痕：{source:?}");
        assert!(
            unresolved[0].message.contains("解析失败"),
            "留痕文案指明解析失败"
        );
    }
}

// ---------------------------------------------------------------------------
// 正向 / 边界：extract 面
// ---------------------------------------------------------------------------

/// 正向：单文件 3 条 `use`（首行 / 中间 / 末行）→ 3 条 fact，行号各自正确、
/// `type_only` 恒 `false`（Rust `use` 无类型专用形态）。
#[test]
fn extract行号与多use() {
    let (temp, entry) = single_crate("use-lines", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let source =
        "use crate::a::A;\n\n// 注释\nuse crate::b::B;\n\nfn helper() {}\n\nuse crate::c::C;\n";
    let (facts, unresolved) = resolver.extract(&entry, source);
    assert!(unresolved.is_empty(), "合法 use 不产留痕");
    assert_eq!(
        facts
            .iter()
            .map(|fact| (fact.specifier.as_str(), fact.line, fact.type_only))
            .collect::<Vec<(&str, u32, bool)>>(),
        vec![
            ("crate::a::A", 1, false),
            ("crate::b::B", 4, false),
            ("crate::c::C", 8, false),
        ],
        "三条 use 的行号与 typeOnly 逐条正确"
    );
}

/// 边界：空文件 / 仅注释 / 仅 `fn` 定义 → 零 fact 零 `unresolved`。
#[test]
fn extract零use面() {
    let (temp, entry) = single_crate("no-use", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    for source in [
        "",
        "// 只有注释\n/* 块注释 */\n",
        "pub fn helper() -> u8 { 1 }\n",
    ] {
        let (facts, unresolved) = resolver.extract(&entry, source);
        assert!(facts.is_empty(), "空面不产 fact：{source:?}");
        assert!(unresolved.is_empty(), "空面不冒充留痕：{source:?}");
    }
}

// ---------------------------------------------------------------------------
// 异常 / 边界：清单面
// ---------------------------------------------------------------------------

/// 异常：清单面三形态（非法 TOML / 空文件 / 缺 `[workspace]` 且顶层非 package）
/// 一律 `Err`——文案指明清单、不 panic（D23：配置面坏了 MUST NOT 静默变成
/// 「零 crate = 零命中」）。
#[test]
fn 清单不可解析形态() {
    for (tag, content) in [
        ("broken-manifest", "[workspace\nmembers = []\n"),
        ("empty-manifest", ""),
        ("no-workspace", "[package]\nversion = \"0.1.0\"\n"),
    ] {
        let temp = TempDir::new(tag);
        temp.write("Cargo.toml", content);
        temp.write("src/main.rs", "fn main() {}\n");
        let error = RustResolver::new(temp.root())
            .err()
            .unwrap_or_else(|| panic!("{tag} 不可判定清单必须 Err"));
        assert!(
            error.contains("清单") || error.contains("Cargo.toml"),
            "{tag} 错误文案指明清单：{error}"
        );
    }
}

/// 异常 / 边界：member 缺 `[package] name`、crate 无入口文件 → `Err`（文案指明
/// 缺项）；观察根无 `Cargo.toml` 的纯 TS 工程 → `Ok` 且零 crate（无清单不冒充配置
/// 错误，D23）。
#[test]
fn 清单缺名与缺入口() {
    let nameless = TempDir::new("nameless-member");
    nameless.write(
        "Cargo.toml",
        &format!(
            "[workspace]\nmembers = [\"{}/crates/a\"]\n",
            nameless.posix_root()
        ),
    );
    nameless.write("crates/a/Cargo.toml", "[package]\nversion = \"0.1.0\"\n");
    nameless.write("crates/a/src/lib.rs", "pub struct A;\n");
    let error = RustResolver::new(nameless.root())
        .err()
        .expect("member 缺 [package] name 必须 Err");
    assert!(
        error.contains("[package] name") || error.contains("Cargo.toml"),
        "错误文案指明缺项：{error}"
    );

    let no_entry = TempDir::new("no-entry");
    no_entry.write("Cargo.toml", "[package]\nname = \"solo\"\n");
    let error = RustResolver::new(no_entry.root())
        .err()
        .expect("crate 无入口文件必须 Err");
    assert!(
        error.contains("入口") || error.contains("Cargo.toml"),
        "错误文案指明入口：{error}"
    );

    let ts_only = TempDir::new("ts-only");
    ts_only.write("src/a.ts", "export const a = 1;\n");
    let resolver = RustResolver::new(ts_only.root()).expect("无清单是纯 TS 工程的合法形态");
    assert_eq!(
        resolver.resolve("src/a.ts", "crate::a::B"),
        None,
        "零 crate ⇒ 任何 use 均不落盘"
    );
    assert!(resolver.unresolved().is_empty(), "零 crate 不产留痕");
}

/// 边界：member glob 无匹配 → `Ok`（crate 清单空），所有 `resolve` 恒 `None`。
#[test]
fn member_glob无匹配() {
    let temp = TempDir::new("no-member");
    temp.write(
        "Cargo.toml",
        &format!(
            "[workspace]\nmembers = [\"{}/crates/*\"]\n",
            temp.posix_root()
        ),
    );
    let resolver = RustResolver::new(temp.root()).expect("零匹配合法");
    assert_eq!(
        resolver.resolve("src/lib.rs", "crate::a::B"),
        None,
        "无 crate 可归属"
    );
}

/// 异常：未知模块路径按「最长前缀降级」落到 crate 根（`""` 条目恒在案）；空
/// specifier → `None`；>1000 chars 取最长命中前缀且不 panic。
#[test]
fn resolve未命中与降级() {
    let (temp, entry) = single_crate(
        "resolve-miss",
        "mod a;
",
    );
    temp.write(
        "src/a.rs",
        "pub struct A;
",
    );
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    assert_eq!(
        resolver.resolve(&entry, "crate::unknown::X"),
        Some(entry.clone()),
        "未知模块降级到 crate 根（最长前缀退到空路径为止）"
    );
    assert_eq!(resolver.resolve(&entry, ""), None, "空 specifier 无首段");
    let long = format!("crate::{}", "a::".repeat(600));
    assert_eq!(
        resolver.resolve(&entry, &long),
        Some("src/a.rs".to_owned()),
        "超长 specifier 不 panic（取最长命中前缀 a）"
    );
}

/// 边界：1000 条 `use` 行 + 300 个模块 → `extract` 全量返回、`resolve` 命中稳定
/// （建树一次复用，不每查询重建）。
#[test]
fn 规模边界批量与复用() {
    let mut lib_source = String::new();
    for index in 0..300 {
        lib_source.push_str(&format!("mod m{index};\n"));
    }
    lib_source.push_str("mod heavy;\n");
    let (temp, entry) = single_crate("scale", &lib_source);
    for index in 0..300 {
        temp.write(
            &format!("src/m{index}.rs"),
            &format!("pub struct M{index};\n"),
        );
    }
    let mut heavy = String::new();
    for index in 0..1000 {
        heavy.push_str(&format!("use crate::m{index}::M{index};\n"));
    }
    temp.write("src/heavy.rs", &heavy);

    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let (facts, unresolved) = resolver.extract("src/heavy.rs", &heavy);
    assert_eq!(facts.len(), 1000, "1000 条 use 全量返回");
    assert!(unresolved.is_empty(), "合法 use 不产留痕");
    assert_eq!(facts[0].line, 1, "首行 use 行号");
    assert_eq!(facts[999].line, 1000, "末行 use 行号");

    for _ in 0..3 {
        assert_eq!(
            resolver.resolve("src/heavy.rs", "crate::m299::M299"),
            Some("src/m299.rs".to_owned()),
            "批量查询命中稳定（复用已建模块树）"
        );
    }
    assert_eq!(
        resolver.resolve(&entry, "crate::m0::M0"),
        Some("src/m0.rs".to_owned()),
        "入口文件的查询同样稳定"
    );
}
