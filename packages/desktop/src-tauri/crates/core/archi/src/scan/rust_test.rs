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

// ===========================================================================
// `extract` 产名（`relation-api-dependency-check` / D5 / AC-2）
//
// 名字口径（唯一实现，`flatten_use_tree`）：`Name` → 该标识符；`Rename` → **被引入
// 名**（不是别名）；`Glob` → 无名；末段 `self` → 无名（`self` 就是模块本身）。
// 产名与「能否落盘」解耦：外部 crate 的 `use` 照产名，落盘失败在扫描层丢边。
// ===========================================================================

/// 正向：单条 `use` 的各形态 → `(specifier, names)` 两列逐值（`Name` / `Rename` /
/// `Glob` / `Group` 四形态一次过）。
#[test]
fn extract产名单条形态四列() {
    let (temp, entry) = single_crate("names-single", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    for (source, specifier, names) in [
        ("use crate::b::B;\n", "crate::b::B", vec!["B"]),
        ("use crate::b::B as W;\n", "crate::b::B", vec!["B"]),
        ("use self::x::B;\n", "self::x::B", vec!["B"]),
        ("use crate::b::*;\n", "crate::b", vec![]),
    ] {
        let (facts, unresolved) = resolver.extract(&entry, source);
        assert!(unresolved.is_empty(), "{source:?} 不产留痕");
        assert_eq!(facts.len(), 1, "{source:?} 恰一条 fact");
        assert_eq!(facts[0].specifier, specifier, "{source:?} 路径列");
        assert_eq!(
            facts[0].names,
            names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<String>>(),
            "{source:?} 名字列（Rename 取被引入名、Glob 无名）"
        );
    }
}

/// 正向：`use crate::b::{B, C};` → 两条 fact 各带自己的名字（一条声明两 facts 的既有
/// 形态不变）。
#[test]
fn extract产名group两facts() {
    let (temp, entry) = single_crate("names-group", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let (facts, unresolved) = resolver.extract(&entry, "use crate::b::{B, C};\n");
    assert!(unresolved.is_empty(), "合法 use 不产留痕");
    assert_eq!(
        facts
            .iter()
            .map(|fact| (fact.specifier.as_str(), fact.names.clone()))
            .collect::<Vec<(&str, Vec<String>)>>(),
        vec![
            ("crate::b::B", vec!["B".to_owned()]),
            ("crate::b::C", vec!["C".to_owned()]),
        ],
        "两 facts 各带自己的名字"
    );
}

/// 正向：`use crate::b::{B, C as D};` → 名字分别 `B` 与 **`C`**（被引入名，非别名
/// `D`）。
#[test]
fn extract产名rename取被引入名() {
    let (temp, entry) = single_crate("names-rename", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let (facts, _) = resolver.extract(&entry, "use crate::b::{B, C as D};\n");
    assert_eq!(
        facts
            .iter()
            .map(|fact| fact.names.clone())
            .collect::<Vec<Vec<String>>>(),
        vec![vec!["B".to_owned()], vec!["C".to_owned()]],
        "别名 `D` 不入值面"
    );
    assert!(
        facts
            .iter()
            .all(|fact| !fact.names.iter().any(|name| name == "D")),
        "别名零命中"
    );
}

/// 边界：末段恰 `self`（`use crate::b::{self};`）→ 无名；嵌套 group
/// `use a::{b::{C, D}, E};` → 三条 fact 名字分别 C / D / E。
#[test]
fn extract产名self与嵌套group() {
    let (temp, entry) = single_crate("names-self-nested", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");

    let (facts, _) = resolver.extract(&entry, "use crate::b::{self};\n");
    assert_eq!(facts.len(), 1, "self 形态一条 fact");
    assert!(facts[0].names.is_empty(), "末段 self ⇒ 无名");

    let (facts, _) = resolver.extract(&entry, "use a::{b::{C, D}, E};\n");
    assert_eq!(
        facts
            .iter()
            .map(|fact| (fact.specifier.as_str(), fact.names.clone()))
            .collect::<Vec<(&str, Vec<String>)>>(),
        vec![
            ("a::b::C", vec!["C".to_owned()]),
            ("a::b::D", vec!["D".to_owned()]),
            ("a::E", vec!["E".to_owned()]),
        ],
        "嵌套 group 逐条展平且各带名字"
    );
}

/// 边界：同一文件多条 `use` 声明 → 每条独立 fact（行号各自对应，跨声明零合并）；
/// 两条重复声明 → 两条 fact 各 `["B"]`（去重只在判定侧）。
#[test]
fn extract产名跨声明不合并() {
    let (temp, entry) = single_crate("names-lines", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let (facts, _) = resolver.extract(
        &entry,
        "use crate::b::B;\n\nuse crate::b::B;\nuse crate::c::C;\n",
    );
    assert_eq!(
        facts
            .iter()
            .map(|fact| (fact.line, fact.names.clone()))
            .collect::<Vec<(u32, Vec<String>)>>(),
        vec![
            (1, vec!["B".to_owned()]),
            (3, vec!["B".to_owned()]),
            (4, vec!["C".to_owned()]),
        ],
        "重复声明各成一条 fact（零合并、零去重）"
    );
}

/// 边界：极长名字（> 1000 chars）与多字节标识符 → 原样、零 panic。
#[test]
fn extract产名极长与多字节() {
    let (temp, entry) = single_crate("names-long", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let long = "N".repeat(1200);
    let (facts, unresolved) = resolver.extract(&entry, &format!("use crate::b::{long};\n"));
    assert!(unresolved.is_empty(), "极长标识符合法");
    assert_eq!(facts[0].names, vec![long], "极长名原样");

    let (facts, unresolved) = resolver.extract(&entry, "use crate::b::名字;\n");
    assert!(unresolved.is_empty(), "多字节标识符合法");
    assert_eq!(facts[0].names, vec!["名字".to_owned()], "多字节名原样");
}

/// 异常：非 `use` 项（`fn` / `impl` / `mod` / `struct`）→ 零 fact（`Item::Use` 之外
/// 一律跳过）。
#[test]
fn extract产名非use项零fact() {
    let (temp, entry) = single_crate("names-non-use", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    for source in [
        "pub fn helper() {}\n",
        "impl A { pub fn f(&self) {} }\n",
        "mod nested;\n",
        "pub struct B;\n",
    ] {
        let (facts, unresolved) = resolver.extract(&entry, source);
        assert!(facts.is_empty(), "{source:?} 不产 fact");
        assert!(unresolved.is_empty(), "{source:?} 不冒充留痕");
    }
}

/// 异常：外部 crate（`use std::collections::HashMap;`）→ fact 照出、名字照产（产名与
/// 可否落盘解耦）。
#[test]
fn extract产名外部crate照产() {
    let temp = TempDir::new("names-external");
    temp.write(
        "Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    temp.write("src/lib.rs", "pub struct A;\n");
    let entry = "src/lib.rs".to_owned();
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let (facts, unresolved) = resolver.extract(
        &entry,
        "use std::collections::HashMap;\nuse std::collections::*;\n",
    );
    assert!(unresolved.is_empty(), "外部 crate 不是解析失败");
    assert_eq!(
        facts
            .iter()
            .map(|fact| (fact.specifier.as_str(), fact.names.clone()))
            .collect::<Vec<(&str, Vec<String>)>>(),
        vec![
            ("std::collections::HashMap", vec!["HashMap".to_owned()]),
            ("std::collections", vec![]),
        ],
        "落盘会丢弃，但名字事实照产出"
    );
    assert_eq!(
        resolver.resolve(&entry, "std::collections::HashMap"),
        None,
        "外部 crate 落盘照旧静默丢弃（两面互不污染）"
    );
}

// ===========================================================================
// `flatten_use_tree` 与「产名 / 落盘」分工（`relation-api-dependency-check` / D5）
//
// 名字面与落盘面同次遍历产出、互不污染：名字决定判定侧的事实名，specifier 决定
// 落盘文件；空 prefix 拼不出 specifier，尾段不参与前缀匹配。
// ===========================================================================

/// 边界：空 prefix + `Glob`（`use *;`，模块面无从拼起的形态）→ 不产路径、零 panic；
/// `extract` 面照常不冒充留痕（glob 没有可枚举的符号）。
#[test]
fn flatten_use_tree空prefix与glob不产路径() {
    let (temp, entry) = single_crate("names-glob-root", "pub struct A;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let (facts, unresolved) = resolver.extract(&entry, "use *;\n");
    assert!(
        facts.is_empty(),
        "空 prefix + Glob 拼不出 specifier ⇒ 零 fact"
    );
    assert!(
        unresolved.is_empty(),
        "该形态不是解析失败（空 prefix 由 `flatten_use_tree` 自行跳过）：{:?}",
        unresolved
            .iter()
            .map(|entry| entry.message.clone())
            .collect::<Vec<String>>()
    );
    assert_eq!(resolver.resolve(&entry, "*"), None, "无 specifier 可落盘");
}

/// 组合：同一条 `use crate::b::{B, C}` → 两条 fact（names 各 `B` / `C`），但两个
/// specifier 的 `resolve` 落到**同一文件**（尾段类型名不参与前缀匹配，`crate::b`
/// 就是模块面）。
#[test]
fn extract与resolve分工两fact落同一文件() {
    let (temp, entry) = single_crate("names-split", "mod b;\n");
    temp.write("src/b.rs", "pub struct B;\npub struct C;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    let (facts, unresolved) = resolver.extract(&entry, "use crate::b::{B, C};\n");
    assert!(unresolved.is_empty(), "合法 use 不产留痕");
    assert_eq!(
        facts
            .iter()
            .map(|fact| (fact.specifier.as_str(), fact.names.clone()))
            .collect::<Vec<(&str, Vec<String>)>>(),
        vec![
            ("crate::b::B", vec!["B".to_owned()]),
            ("crate::b::C", vec!["C".to_owned()]),
        ],
        "一条声明两条 fact，名字各异"
    );
    assert_eq!(
        facts
            .iter()
            .map(|fact| resolver.resolve(&entry, &fact.specifier))
            .collect::<Vec<Option<String>>>(),
        vec![Some("src/b.rs".to_owned()), Some("src/b.rs".to_owned())],
        "尾段被最长前缀吃掉 ⇒ 两 specifier 落同一模块文件"
    );
}

/// 边界（表驱动 2 行）：同模块两形态（`use crate::b;` 模块自身 + `use crate::b::*;`）
/// → 落盘同一文件；names 分别 `["b"]`（模块名照产）与 `[]`（glob 无名）。
#[test]
fn extract与resolve同模块多形态() {
    let (temp, entry) = single_crate("names-same-module", "mod b;\n");
    temp.write("src/b.rs", "pub struct B;\n");
    let resolver = RustResolver::new(temp.root()).expect("清单可解析");
    for (source, specifier, names) in [
        ("use crate::b;\n", "crate::b", vec!["b"]),
        ("use crate::b::*;\n", "crate::b", vec![]),
    ] {
        let (facts, unresolved) = resolver.extract(&entry, source);
        assert!(unresolved.is_empty(), "{source:?} 合法 use 不产留痕");
        assert_eq!(facts.len(), 1, "{source:?} 恰一条 fact");
        assert_eq!(facts[0].specifier, specifier, "{source:?} 路径列");
        assert_eq!(
            facts[0].names,
            names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<String>>(),
            "{source:?} 名字列（模块自身照产名、glob 无名）"
        );
        assert_eq!(
            resolver.resolve(&entry, &facts[0].specifier),
            Some("src/b.rs".to_owned()),
            "{source:?} 落盘同文件"
        );
    }
}
