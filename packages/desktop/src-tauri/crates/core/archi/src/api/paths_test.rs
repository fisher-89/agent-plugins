//! `api::paths` 的单元测试：`<root>` 归一两步法（不去扩展名）、四类路径派生
//! （契约 / schema / 生成物 / 索引）、入口三分支与四分支的**顺序封闭**、`#[path]`
//! 与 TS import specifier 的单点产出、跨域 `$ref` 前缀、`<Domain>` 与语言判定。
//!
//! 零 mock：`entry_form` / `generated_path` 的存在性探测用真实临时目录落空文件
//! （零内容即足以驱动分支），其余函数零 IO；`foundation::layout` 的真实派生值是
//! 前缀断言的基准（路径单点即被测语义）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_api_dir;

use crate::api::contract::parse;
use crate::api::emit::ts::render as render_ts;
use crate::api::emit::{EmitTarget, ModuleRef};
use crate::api::mod_test::declare_ancestors;
use crate::api::schema::build_schema;
use crate::api::{load_contracts, meta, render_all};
use crate::dto::{ApiStatus, ArchiElement, ElementKind, GateTier, NodeMark};

use super::paths::{
    contract_path, directory, domain_of, entry_form, generated_kind, generated_path, index_path,
    is_module_segment, language_of, mod_path_attr, module_name, normalize_root, ref_relative,
    rust_module_path, schema_path, ts_import_specifier, EntryForm, GeneratedKind, Language,
};

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-api-paths-test-{}-{}-{}",
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

    /// 落一个空的 repo 相对文件（自动建父目录；`entry_form` 只看 `is_file`）。
    fn touch(&self, relative: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, "").expect("落文件失败");
    }

    /// 落一个 repo 相对文件（带内容；模型 / 契约 / 入口源文）。
    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }

    /// 落模型（`.dev-team/architecture/models/01-test.toml`）。
    ///
    /// 叶元素的祖先链自动补齐：`render_all` 的元素表经 `dto::graph_view` 取数，
    /// 缺祖先即 `orphanHierarchy` 结构错（fail-closed）。
    fn write_model(&self, content: &str) {
        self.write(
            ".dev-team/architecture/models/01-test.toml",
            &declare_ancestors(content),
        );
    }

    /// 落契约（`<archi_api_dir>/<Domain>/<FQN>.md`）。
    fn write_contract(&self, fqn: &str, content: &str) {
        let path = contract_path(self.root(), fqn).expect("测试 FQN 恒为合法形态");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建契约目录失败");
        }
        fs::write(&path, content).expect("写契约失败");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 元素构造器（`path` 原文直传，不经归一——归一是被测行为）。
fn element(fqn: &str, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Warn,
        api: ApiStatus::Declared,
        path: paths.iter().map(|path| (*path).to_owned()).collect(),
        summary: None,
        metadata: BTreeMap::new(),
        model_file: ".dev-team/architecture/models/01-test.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
    }
}

/// 路径的 `..` / `.` 段清单（目录穿越自证）。
fn dot_segments(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::ParentDir => Some("..".to_owned()),
            Component::CurDir => Some(".".to_owned()),
            _ => None,
        })
        .collect()
}

/// 词法规范化（`a/b/../../c` → `c`；零触盘——`#[path]` 解析指向的自证装置）。
fn lexical_normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
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
// normalize_root：两步归一（不去扩展名）
// ---------------------------------------------------------------------------

/// 正向（D2）：`./` 前缀、`\` 分隔、尾 `/` 三形态归一到同一串，且等于无前缀无尾
/// 斜杠形态（四写法逐字相等）。
#[test]
fn normalize_root四写法逐字相等() {
    let expected = "packages/desktop/src-tauri/crates/infra/archi";
    let cases = [
        "./packages/desktop/src-tauri/crates/infra/archi/",
        "packages\\desktop\\src-tauri\\crates\\infra\\archi",
        "packages/desktop/src-tauri/crates/infra/archi",
        "packages/desktop/src-tauri/crates/infra/archi/",
    ];
    for raw in cases {
        assert_eq!(
            normalize_root(raw),
            expected,
            "四写法归一后逐字相等：{raw:?}"
        );
    }
}

/// 边界（D2 反例在案）：MUST NOT 去文件扩展名——前端单文件元素的 `<root>` 保留
/// `.tsx`，否则生成物文件名会错位。
#[test]
fn normalize_root不去文件扩展名() {
    assert_eq!(
        normalize_root("./packages/desktop/src/views/architecture.tsx"),
        "packages/desktop/src/views/architecture.tsx",
        "扩展名是模块身份的一部分"
    );
}

/// 边界：空串 / 仅 `./` / `.` / `/` / 中段双斜杠 / 超长输入 → 不 panic 且结果可判。
#[test]
fn normalize_root边界值零panic() {
    for raw in ["", "./", ".", "/", "a//b"] {
        let once = normalize_root(raw);
        // 中段双斜杠不折叠，但必须幂等（调用方按单次归一结果比对）
        assert_eq!(
            normalize_root(&once),
            once,
            "归一结果自身必须是不动点：{raw:?}"
        );
    }
    assert_eq!(normalize_root(""), "", "空串归空串");
    assert_eq!(normalize_root("./"), "", "仅 `./` 归空串");
    assert_eq!(normalize_root("/"), "", "仅 `/` 归空串");
    assert_eq!(normalize_root("a//b"), "a//b", "中段双斜杠不折叠");

    let long = "a".repeat(1200);
    assert_eq!(normalize_root(&long), long, "超长输入原样返回");
}

/// 边界：任一形态归一两次 == 归一一次（幂等是 diff 门稳定的前提）。
#[test]
fn normalize_root幂等() {
    for raw in ["./a/b/", "a\\b\\", "packages/desktop/src/views/x.tsx"] {
        let once = normalize_root(raw);
        assert_eq!(
            normalize_root(&once),
            once,
            "normalize(normalize(x)) == normalize(x)：{raw:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// contract_path / schema_path：同目录、`<Domain>` 目录段
// ---------------------------------------------------------------------------

/// 正向（AC-4）：`App.Backend.Infra.ArchiRuntime` → `<archi_api_dir>/Infra/...`；
/// 两函数父目录逐字相等、均以 `archi_api_dir(root)` 为前缀（零行内 `api` 段）。
#[test]
fn contract_path与schema_path同目录同前缀() {
    let root = Path::new("/ws");
    let fqn = "App.Backend.Infra.ArchiRuntime";

    let contract = contract_path(root, fqn).expect("合法 FQN 应可派生");
    let schema = schema_path(root, fqn).expect("合法 FQN 应可派生");

    assert_eq!(
        contract.parent(),
        schema.parent(),
        "契约与 schema 必须同目录"
    );
    assert_eq!(
        contract
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|name| name.to_string_lossy().into_owned()),
        Some("Infra".to_owned()),
        "目录段 = 父 FQN 末段"
    );
    assert_eq!(
        contract
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        Some(format!("{fqn}.md")),
        "契约文件名 = <FQN>.md"
    );
    assert_eq!(
        schema
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        Some(format!("{fqn}.schema.json")),
        "schema 文件名 = <FQN>.schema.json"
    );
    assert_eq!(
        contract.parent(),
        Some(archi_api_dir(root).join("Infra").as_path()),
        "前缀恒为 archi_api_dir(root)"
    );
}

/// 边界：单段 FQN 退化 `Root` 目录；含 emoji 的合法 FQN → 路径可判且零目录穿越
/// （不含 `..` 段）；形态非法的 FQN → `None`。
#[test]
fn contract_path顶层退化root且零穿越() {
    let root = Path::new("/ws");
    assert_eq!(
        contract_path(root, "App")
            .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
            .and_then(|parent| parent
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())),
        Some("Root".to_owned()),
        "单段 FQN 落 Root 目录"
    );

    for fqn in ["App.🎯.Target", "App.Core.Alpha"] {
        let path = contract_path(root, fqn).expect("合法 FQN 应可派生");
        assert!(
            dot_segments(&path).is_empty(),
            "契约路径 MUST NOT 含目录穿越段：{fqn:?} → {path:?}"
        );
        assert!(
            path.starts_with(archi_api_dir(root)),
            "契约路径恒在 archi_api_dir 之下：{fqn:?}"
        );
    }
}

/// 边界（路径形态校验，安全边界）：绝对路径 / 盘符 / `..` 段 / 空段 / 空串的 FQN
/// 一律 `None`——放行即让 `root.join(fqn)` 替换 base 或上溯，把 `is_file()` 探测
/// 变成任意文件读、把工具 crate 的写盘变成任意文件写。
#[test]
fn contract_path与schema_path拒收越界形态() {
    let root = Path::new("/ws");
    for fqn in [
        "",
        ".",
        "..",
        "App..Target",
        "App.",
        ".App",
        "../App.Core.Alpha",
        "App/../../etc/passwd",
        "/abs/path/Fqn",
        r"C:\windows\x",
        r"App.Core.Alpha\x",
    ] {
        assert_eq!(contract_path(root, fqn), None, "契约路径拒收：{fqn:?}");
        assert_eq!(schema_path(root, fqn), None, "schema 路径拒收：{fqn:?}");
    }
}

// ---------------------------------------------------------------------------
// generated_path：三种入口形态
// ---------------------------------------------------------------------------

/// 正向（AC-4）：crate 形态落 `<root>.i.rs`（**根同级**，不在目录内）、`mod.rs`
/// 形态与单文件形态同款规则（`<root>` 归一后追加 `.i.<ext>`）。
#[test]
fn generated_path三种入口形态() {
    let root = Path::new("/ws");
    let cases = [
        (
            element("App.Backend.Core.Archi", &["./crates/core/archi/"]),
            GeneratedKind::Rust,
            "/ws/crates/core/archi.i.rs",
        ),
        (
            element("App.Backend.Core.Archi", &["crates/core/archi/mod.rs"]),
            GeneratedKind::Rust,
            "/ws/crates/core/archi/mod.rs.i.rs",
        ),
        (
            element("App.Frontend.Views.Welcome", &["views/welcome-view.tsx"]),
            GeneratedKind::TypeScript,
            "/ws/views/welcome-view.tsx.i.ts",
        ),
    ];
    for (element, kind, expected) in cases {
        let path = generated_path(root, element.path.first().expect("非空 root"), kind)
            .expect("路径应可派生");
        assert_eq!(path, PathBuf::from(expected), "生成物落点：{element:?}");
        assert_eq!(
            path.parent(),
            root.join(&element.path[0]).parent(),
            "根同级：不在元素目录之内"
        );
    }
}

/// 边界：`path` 空（容器元素）→ `None`；`path` 首条为空串 → `None`（`<root>` 归一
/// 后为空即无生成物）；断言均不 panic 且不以 `..` 穿越。
#[test]
fn generated_path路径空为零() {
    let root = Path::new("/ws");
    assert_eq!(
        generated_path(root, "", GeneratedKind::Rust),
        None,
        "容器元素天然无生成物"
    );
    assert_eq!(
        generated_path(root, "", GeneratedKind::Rust),
        None,
        "首条空串归约为空 root → 无生成物"
    );
}

/// 边界（路径形态校验，安全边界）：绝对路径 / 盘符 / 含 `..` 段 / 空段 / `~` 起始 →
/// `generated_path` 与 `entry_form` 双 `None`——`root.join(绝对路径)` 会丢弃 base、
/// `..` 段会上溯、`~` 会被下游 shell 形态展开，放行即让生成物落到仓外。
#[test]
fn generated_path与entry_form拒收越界形态() {
    let root = Path::new("/ws");
    for raw in [
        "/abs/elsewhere",
        "../outside/repo",
        "crates/../../outside",
        r"C:\windows",
        "crates//double",
        "~/home/repo",
    ] {
        let candidate = element("App.Core.X", &[raw]);
        assert_eq!(
            generated_path(root, raw, GeneratedKind::Rust),
            None,
            "生成物路径拒收：{raw:?}"
        );
        assert_eq!(entry_form(root, &candidate), None, "入口查找拒收：{raw:?}");
    }
}

/// 边界（语言 → 生成物种类的分派单点）：`TypeScript` → `TypeScript`（四扩展名同族）、
/// `Rust` 与 `Unknown` → `Rust`（无扩展名的元素根唯一对应 Rust 形态；见函数 doc）。
#[test]
fn generated_kind语言分派单点() {
    assert_eq!(generated_kind(Language::Rust), GeneratedKind::Rust);
    for file in ["a.ts", "a.tsx", "a.js", "a.jsx"] {
        assert_eq!(
            generated_kind(language_of(file)),
            GeneratedKind::TypeScript,
            "{file} 同归 TypeScript"
        );
    }
    assert_eq!(
        generated_kind(Language::Unknown),
        GeneratedKind::Rust,
        "无扩展名 / 非前端扩展名的元素根按 Rust 形态渲染"
    );
    assert_eq!(generated_kind(language_of("a.py")), GeneratedKind::Rust);
    assert_eq!(generated_kind(language_of("noext")), GeneratedKind::Rust);
}

// ---------------------------------------------------------------------------
// entry_form：Rust 三分支 / 前端四分支的顺序封闭
// ---------------------------------------------------------------------------

/// 正向（D4）：同目录同时存在 `<dir>/src/lib.rs` 与 `<dir>/mod.rs` → 命中第一分支
/// （`Crate`）；移除后依次命中 `SubdirModule` / `SingleFile`。
#[test]
fn entry_form的rust三分支顺序() {
    let temp = TempDir::new("rust-branches");
    let element = element("App.Backend.Core.Archi", &["crates/core/archi/"]);

    temp.touch("crates/core/archi/src/lib.rs");
    temp.touch("crates/core/archi/mod.rs");
    temp.touch("crates/core/archi.rs");
    assert_eq!(
        entry_form(temp.root(), &element),
        Some((EntryForm::Crate, "crates/core/archi/src/lib.rs".to_owned())),
        "第一分支优先（三分支顺序封闭）"
    );

    fs::remove_file(temp.root().join("crates/core/archi/src/lib.rs")).expect("移除 lib.rs");
    assert_eq!(
        entry_form(temp.root(), &element),
        Some((
            EntryForm::SubdirModule,
            "crates/core/archi/mod.rs".to_owned()
        )),
        "第二分支：<dir>/mod.rs"
    );

    fs::remove_file(temp.root().join("crates/core/archi/mod.rs")).expect("移除 mod.rs");
    assert_eq!(
        entry_form(temp.root(), &element),
        Some((EntryForm::SingleFile, "crates/core/archi.rs".to_owned())),
        "第三分支：<dir>.rs"
    );
}

/// 正向（D4）：前端四分支顺序 `index.ts` → `index.tsx` → `<dir>.ts` → `<dir>.tsx`，
/// 逐个移除后依次命中后三支（4 步递进）。
#[test]
fn entry_form的前端四分支顺序() {
    let temp = TempDir::new("ts-branches");
    let element = element("App.Frontend.Views.Welcome", &["src/views/welcome/"]);

    temp.touch("src/views/welcome/index.ts");
    temp.touch("src/views/welcome/index.tsx");
    temp.touch("src/views/welcome.ts");
    temp.touch("src/views/welcome.tsx");

    let steps = [
        ("src/views/welcome/index.ts", EntryForm::SubdirModule),
        ("src/views/welcome/index.tsx", EntryForm::SubdirModule),
        ("src/views/welcome.ts", EntryForm::SingleFile),
        ("src/views/welcome.tsx", EntryForm::SingleFile),
    ];
    for (expected_entry, expected_form) in steps {
        assert_eq!(
            entry_form(temp.root(), &element),
            Some((expected_form, expected_entry.to_owned())),
            "前端入口顺序封闭（当前应命中 {expected_entry}）"
        );
        fs::remove_file(temp.root().join(expected_entry)).expect("移除入口文件");
    }
    assert_eq!(
        entry_form(temp.root(), &element),
        None,
        "四分支全移除后无入口"
    );
}

/// 异常：`<dir>/lib.rs`（非 `src/lib.rs`）MUST NOT 在列——如实报无入口。
#[test]
fn entry_form排除目录内lib_rs() {
    let temp = TempDir::new("lib-rs-not-listed");
    temp.touch("crates/core/archi/lib.rs");
    assert_eq!(
        entry_form(
            temp.root(),
            &element("App.Backend.Core.Archi", &["crates/core/archi/"])
        ),
        None,
        "crate 的 lib.rs 恒在 src/ 下；<dir>/lib.rs 不是入口"
    );
}

/// 异常（AC-7 基线 28 的成因）：`path` 指向不存在目录 / 目录内只有非入口名文件 /
/// `path` 空 → 全 `None`。
#[test]
fn entry_form全不命中() {
    let temp = TempDir::new("entry-none");
    temp.touch("src/views/x.ts");

    assert_eq!(
        entry_form(
            temp.root(),
            &element("App.Frontend.Views.Missing", &["src/views/missing/"])
        ),
        None,
        "目录不存在 → 无入口"
    );
    assert_eq!(
        entry_form(temp.root(), &element("App.Frontend.Views", &["src/views/"])),
        None,
        "目录内只有 views/x.ts，不是 views 的入口"
    );
    assert_eq!(
        entry_form(temp.root(), &element("App", &[])),
        None,
        "容器元素无入口"
    );
}

// ---------------------------------------------------------------------------
// mod_path_attr / ts_import_specifier：值单点产出
// ---------------------------------------------------------------------------

/// 正向（AC-4 / D5）：三形态值逐字相等，且解析后指向**根同级**生成物；手写
/// 「入口同级」（`../archi.i.rs`）会错位到 `<dir>/archi.i.rs`（两值不相等）。
#[test]
fn mod_path_attr三形态与根同级自证() {
    assert_eq!(mod_path_attr(EntryForm::Crate, "archi"), "../../archi.i.rs");
    assert_eq!(
        mod_path_attr(EntryForm::SubdirModule, "archi"),
        "../archi.i.rs"
    );
    assert_eq!(mod_path_attr(EntryForm::SingleFile, "archi"), "archi.i.rs");

    let entry = "crates/infra/archi/src/lib.rs";
    let entry_dir = entry
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .expect("入口目录");
    let correct = lexical_normalize(&format!(
        "{entry_dir}/{}",
        mod_path_attr(EntryForm::Crate, "archi")
    ));
    assert_eq!(correct, "crates/infra/archi.i.rs", "解析后落在根同级");

    let wrong = lexical_normalize(&format!(
        "{entry_dir}/{}",
        mod_path_attr(EntryForm::SubdirModule, "archi")
    ));
    assert_eq!(
        wrong, "crates/infra/archi/archi.i.rs",
        "手写「入口同级」错位到目录内"
    );
    assert_ne!(correct, wrong, "两值 MUST NOT 相等（手算层数是禁用面）");
}

/// 边界（D38）：`#[path]` 的值是**字面量**，等值真值源是 `mod_path_attr`——对已编排入口
/// `crates/infra/archi/src/lib.rs`，其 `#[path = "…"]` 的字面量逐字等于
/// `mod_path_attr(EntryForm::Crate, "archi")` 的输出；且该函数在产品 `.rs` 内**零调用
/// 者**（`lib.rs` 的注释只写「字面量与函数输出等值」，不写「由该函数产出」）。
#[test]
fn mod_path_attr被入口行等值消费() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/core/archi → crates/core → crates → src-tauri
    let mut src_tauri = manifest.clone();
    for _ in 0..3 {
        src_tauri.pop();
    }
    let lib_rs = src_tauri.join("crates/infra/archi/src/lib.rs");
    let source = fs::read_to_string(&lib_rs).expect("入口编排文件在案");

    let attribute = source
        .lines()
        .find_map(|line| line.trim().strip_prefix("#[path = \""))
        .and_then(|tail| tail.strip_suffix("\"]"))
        .expect("入口行含 `#[path = \"…\"]` 字面量");
    assert_eq!(
        attribute,
        mod_path_attr(EntryForm::Crate, "archi"),
        "入口行的字面量与单点函数输出逐字相等（D38 等值真值源）"
    );

    // 零产品调用者：`mod_path_attr(` 在产品 `.rs` 内只出现在定义行与注释引用行
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&src_tauri.join("crates"), &mut hits);
    scan_product_rs(&src_tauri.join("src"), &mut hits);
    let product_calls: Vec<&(String, String)> = hits
        .iter()
        .filter(|(path, line)| {
            !path.ends_with("api/paths.rs")
                && !line.trim_start().starts_with("//")
                && line.contains("mod_path_attr(")
        })
        .collect();

    assert!(
        product_calls.is_empty(),
        "`mod_path_attr` 零产品调用者（D38 显式形态，不是缺陷）：{product_calls:?}"
    );
    assert!(
        !source.contains("由该函数产出"),
        "入口注释 MUST NOT 写「由该函数产出」（只可写「字面量 + 与函数输出等值」）"
    );
}

/// 正向：TS import specifier 两形态——目录形态 `../<name>.i`、单文件形态
/// `./<name>.i`（零 `../`）。
#[test]
fn ts_import_specifier两形态() {
    assert_eq!(ts_import_specifier(EntryForm::Crate, "archi"), "../archi.i");
    assert_eq!(
        ts_import_specifier(EntryForm::SubdirModule, "archi"),
        "../archi.i"
    );
    assert_eq!(ts_import_specifier(EntryForm::SingleFile, "app"), "./app.i");
}

// ---------------------------------------------------------------------------
// ref_relative / domain_of / module_name
// ---------------------------------------------------------------------------

/// 正向（AC-11）：同 `<Domain>` → `./`；跨目录 → `../<Domain>/`；目标为顶层元素 →
/// `../Root/`。
#[test]
fn ref_relative同域跨域与顶层() {
    assert_eq!(
        ref_relative("App.Backend.Core.Archi", "App.Backend.Core.Foundation"),
        "./",
        "同域用 `./`"
    );
    assert_eq!(
        ref_relative("App.Backend.Infra.ArchiRuntime", "App.Backend.Core.Archi"),
        "../Core/",
        "跨域用 `../<Domain>/`"
    );
    assert_eq!(
        ref_relative("App.Backend.Core.Archi", "App"),
        "../Root/",
        "目标无父段 → Root"
    );
}

/// 边界（R-8）：上下游末段撞名不影响前缀（元素身份是 FQN，撞名只影响布局）；
/// `from == to` → `./`。
#[test]
fn ref_relative撞名域与自引用() {
    let left = ref_relative("App.Backend.Core.Agent", "App.Frontend.Components.Agent");
    let right = ref_relative("App.Frontend.Components.Agent", "App.Backend.Core.Agent");
    assert_eq!(left, "../Components/", "目标域末段");
    assert_eq!(right, "../Core/", "反向目标域末段");
    assert_ne!(left, right, "撞名只影响布局，前缀仍按目标域算");
    assert_eq!(
        ref_relative("App.Backend.Core.Archi", "App.Backend.Core.Archi"),
        "./",
        "自引用视为同域"
    );
}

/// 边界：四段 → 父段末段；两段 → 首段；单段 / 空串 → `None`；尾点形态可判。
#[test]
fn domain_of形态() {
    assert_eq!(
        domain_of("App.Backend.Infra.ArchiRuntime"),
        Some("Infra".to_owned())
    );
    assert_eq!(domain_of("App.Agent"), Some("App".to_owned()));
    assert_eq!(domain_of("App"), None, "单段无父段");
    assert_eq!(domain_of(""), None, "空串无父段");
    // 尾点形态：`App.` 分三段后父段取到 `App`（可判、不 panic）；`App..X` 的父段为空串
    assert_eq!(domain_of("App."), Some("App".to_owned()));
    assert_eq!(domain_of("App..X"), Some(String::new()));
}

/// 正向（D40 正例 / 反例）：`module_name("<root>")` 是透传 `<name>` 的唯一产出点——
/// 元素 `App.Backend.Infra.ArchiRuntime`（FQN 末段 `ArchiRuntime`）的元素根末段是
/// `archi`，两值**不相等**；同前缀形态（尾 `/`、前导 `./`）归一后结果不变；`emit/`
/// 内零 `rsplit('.')`（FQN 派生第二套命名的在案反例）。
#[test]
fn module_name是透传name的唯一产出点() {
    let element_root = "packages/desktop/src-tauri/crates/infra/archi";
    let name = module_name(element_root);
    assert_eq!(name, "archi", "路径末段");
    assert_eq!(
        domain_of("App.Backend.Infra.ArchiRuntime"),
        Some("Infra".to_owned())
    );
    let fqn_tail = "App.Backend.Infra.ArchiRuntime"
        .rsplit('.')
        .next()
        .expect("FQN 末段");
    assert_eq!(fqn_tail, "ArchiRuntime");
    assert_ne!(
        name, fqn_tail,
        "路径末段 vs FQN 末段（D40 的在案分叉，两值 MUST NOT 相等）"
    );

    for equivalent in [
        "./packages/desktop/src-tauri/crates/infra/archi/",
        "packages/desktop/src-tauri/crates/infra/archi/",
    ] {
        assert_eq!(
            module_name(equivalent),
            name,
            "前缀与尾斜杠归一后不变：{equivalent:?}"
        );
    }

    let ts_source = include_str!("emit/ts.rs");
    let rust_source = include_str!("emit/rust.rs");
    for (label, source) in [("ts.rs", ts_source), ("rust.rs", rust_source)] {
        assert!(
            !source.contains("rsplit('.')"),
            "`emit/{label}` 内零 `rsplit('.')`（FQN 派生第二套命名的在案反例）"
        );
    }
}

/// 正向：`<root>` 末段（不去扩展名）是三形态的唯一来源。
#[test]
fn module_name取末段不去扩展名() {
    assert_eq!(module_name("crates/infra/archi"), "archi");
    assert_eq!(
        module_name("views/welcome-view.tsx"),
        "welcome-view.tsx",
        "末段保留扩展名（D2）"
    );
    assert_eq!(
        module_name("./packages/desktop/src-tauri/crates/infra/archi/"),
        "archi",
        "前缀与尾斜杠先归一"
    );
}

/// 边界：空串 / `/` / 超长 / emoji → 不 panic，末段原样。
#[test]
fn module_name边界() {
    assert_eq!(module_name(""), "", "空串末段为空串");
    assert_eq!(module_name("/"), "", "仅分隔符末段为空串");
    let long = "a".repeat(1200);
    assert_eq!(module_name(&long), long, "超长输入不 panic");
    assert_eq!(module_name("src/🎯/模块"), "模块", "非 ASCII 末段原样");
}

// ---------------------------------------------------------------------------
// rust_module_path：Rust 透传模块路径的单点（D40）
// ---------------------------------------------------------------------------

/// 正向（D40）：`<root>` 末段 → `crate::<末段>`；两个不同 crate 的同名元素给同一模块
/// 路径（跨 crate 父子已由 `apiNestedCrossCrate` 判不可表达，本函数零 crate 名推导）。
#[test]
fn rust_module_path末段转crate路径() {
    assert_eq!(
        rust_module_path("packages/desktop/src-tauri/crates/infra/archi"),
        "crate::archi"
    );
    assert_eq!(
        rust_module_path("packages/desktop/src-tauri/crates/core/archi"),
        "crate::archi",
        "两个不同 crate 的同名元素同路径（零 crate 名推导）"
    );
    assert_eq!(
        rust_module_path("src/views/welcome-view.tsx"),
        "crate::welcome-view.tsx",
        "形态可判即可（输出面只要求末段）"
    );
}

/// 边界（D40）：空串 / `/` / 超长 / 前导 `./` 与尾 `/` → 不 panic，且**与 `module_name`
/// 同源**（结果恒为 `format!("crate::{}", module_name(root))`）；输出零 FQN 点分段。
#[test]
fn rust_module_path边界与module_name同源() {
    for raw in [
        "",
        "/",
        "./packages/desktop/src-tauri/crates/infra/archi/",
        "src/views/welcome-view.tsx",
        "src/🎯/模块",
    ] {
        assert_eq!(
            rust_module_path(raw),
            format!("crate::{}", module_name(raw)),
            "与 `module_name` 逐字同源：{raw:?}"
        );
    }
    assert_eq!(
        rust_module_path(""),
        "crate::",
        "空串退化形态可判、不 panic"
    );
    assert_eq!(rust_module_path("/"), "crate::", "仅分隔符同形");

    let long = "a".repeat(1200);
    assert_eq!(
        rust_module_path(&long),
        format!("crate::{long}"),
        "超长输入不 panic"
    );

    for fqn in ["App.Backend.Infra.ArchiRuntime", "App.X.Y"] {
        assert!(
            !rust_module_path("crates/infra/archi").contains(fqn),
            "MUST NOT 把 FQN 点分段当 Rust 路径：{fqn}"
        );
    }
    assert!(
        !rust_module_path("crates/infra/archi").contains('.'),
        "输出零点分段（`App.X.Y` 形态即错）"
    );
}

/// 异常（D40 反例在案）：`emit/rust.rs` 的 `pub use` 目标取 `ModuleRef.module_path` 而非
/// 目标 FQN 的点分段——该文件内零 `rsplit('.')`、零 `replace('.', "::")`，且
/// `module.module_path` 是唯一模块段来源。
#[test]
fn rust_module_path是透传路径的唯一产出点() {
    let source = include_str!("emit/rust.rs");
    assert!(
        !source.contains("rsplit('.')"),
        "`emit/rust.rs` 零 FQN 点分段切分（D40 反例在案）"
    );
    assert!(
        !source.contains("replace('.', \"::\")"),
        "零「点 → ::」替换（第二套命名即错）"
    );
    assert_eq!(
        source.matches("module.module_path").count(),
        1,
        "`pub use` 的模块段只取自 `ModuleRef.module_path`（单点）"
    );
}

// ---------------------------------------------------------------------------
// language_of：六扩展名与未知面
// ---------------------------------------------------------------------------

/// 正向（D19 分派）：`.rs` → Rust；`.ts` / `.tsx` / `.js` / `.jsx` → TypeScript。
#[test]
fn language_of六扩展名() {
    assert_eq!(language_of("x.rs"), Language::Rust);
    for file in ["x.ts", "x.tsx", "x.js", "x.jsx"] {
        assert_eq!(language_of(file), Language::TypeScript, "同族归并：{file}");
    }
}

/// 异常：`x.py` / 无扩展名 / `x.rs.bak` / 空串 / 大写 `x.RS` → 全 `Unknown`（不猜测；
/// 取数器对 `Unknown` 返回空 `Vec`）。
#[test]
fn language_of未知面不猜测() {
    for file in ["x.py", "x", "x.rs.bak", "", "x.RS"] {
        assert_eq!(
            language_of(file),
            Language::Unknown,
            "词面外一律 Unknown：{file:?}"
        );
    }
}

/// 边界：索引路径恒为 `<archi_api_dir>/INDEX.toml`（全局唯一一份，不在 `<Domain>`
/// 目录内）。
#[test]
fn index_path位于契约目录根() {
    let root = Path::new("/ws");
    assert_eq!(index_path(root), archi_api_dir(root).join("INDEX.toml"));
    assert_eq!(
        index_path(root).parent(),
        Some(archi_api_dir(root).as_path()),
        "索引不进 <Domain> 子目录"
    );
}

// ---------------------------------------------------------------------------
// directory / index_path：契约目录的单点派生
// ---------------------------------------------------------------------------

/// 正向（AC-4 / AC-17 前置）：`directory(root)` 与 `foundation::layout::archi_api_dir(root)`
/// **逐字相等**（同一值，不是相等前缀）；`root` 含空格 / 非 ASCII / 尾分隔符三形态均成立。
#[test]
fn directory单点转发与layout同值() {
    let roots = [
        Path::new("/tmp/with space/ws"),
        Path::new("/tmp/中文根/ws"),
        Path::new("/tmp/trailing/"),
    ];
    for root in roots {
        assert_eq!(
            directory(root),
            archi_api_dir(root),
            "directory 是 archi_api_dir 的唯一转发点（同一值）：{root:?}"
        );
        assert_eq!(
            directory(root).file_name().map(|name| name.to_owned()),
            archi_api_dir(Path::new("/tmp"))
                .file_name()
                .map(|name| name.to_owned()),
            "末段沿用 layout 的段名（零行内拼段）：{root:?}"
        );
    }
}

/// 边界（AC-17）：契约目录一切派生都经 `directory` 单点——来源文件内零 `"api"` 字面量
/// （`API_DIR_NAME` 只在 `foundation::layout` 定义），且 `contract_path` / `schema_path` /
/// `index_path` 三者的父目录均可由 `directory(root)` 推出。
#[test]
fn directory零行内api段() {
    let source = include_str!("paths.rs");
    assert!(
        !source.contains("\"api\""),
        "paths.rs 内零 `\"api\"` 字面量（MUST NOT 行内拼 api 段）"
    );
    assert!(
        source.contains("archi_api_dir"),
        "目录派生经 foundation::layout::archi_api_dir"
    );
    assert!(source.contains("fn directory"), "单点转发函数在案");

    let root = Path::new("/ws");
    assert_eq!(
        index_path(root).parent(),
        Some(directory(root).as_path()),
        "索引的父目录逐字等于 directory(root)"
    );
    assert_eq!(
        contract_path(root, "App.Core.Alpha")
            .expect("合法 FQN")
            .parent(),
        Some(directory(root).join("Core").as_path()),
        "契约在 directory(root)/<Domain>/ 下一层"
    );
    assert_eq!(
        schema_path(root, "App.Core.Alpha")
            .expect("合法 FQN")
            .parent(),
        Some(directory(root).join("Core").as_path()),
        "schema 与契约同层"
    );
}

/// 正向（D21 / 持久化二）：`index_path(root)` 末段恰为 `INDEX.toml`、父目录恰为
/// `directory(root)`；与 `contract_path` / `schema_path`（`<Domain>/` 下一层）**不同层**。
#[test]
fn index_path形态与层级对比() {
    let root = Path::new("/ws");
    let index = index_path(root);
    assert_eq!(
        index.file_name().and_then(|name| name.to_str()),
        Some("INDEX.toml"),
        "末段恰为 INDEX.toml"
    );
    assert_eq!(
        index.parent(),
        Some(directory(root).as_path()),
        "父目录恰为 directory(root)"
    );

    let contract = contract_path(root, "App.Backend.Infra.ArchiRuntime").expect("合法 FQN");
    assert_ne!(
        index.parent(),
        contract.parent(),
        "索引在 api 根、契约在域目录（不同层）"
    );
    assert!(
        contract
            .parent()
            .is_some_and(|parent| parent.starts_with(directory(root))),
        "契约目录在 directory(root) 之下"
    );
    assert_eq!(
        index.parent().map(|parent| parent.components().count()),
        Some(directory(root).components().count()),
        "索引层级 = 契约目录本身（比域目录浅一层）"
    );
}

/// 边界：索引全局唯一一份——同一 root 下与元素数无关恒同一路径；`root` 指向不存在
/// 路径 → 纯推导返回、不 panic、零 IO（不创建任何目录）。
#[test]
fn index_path全局唯一且零io() {
    let root = Path::new("/ws");
    let first = index_path(root);
    for _ in 0..5 {
        assert_eq!(index_path(root), first, "索引路径与元素数无关（恒同一值）");
    }

    let missing = Path::new("/definitely/not/exists/for/index");
    let derived = index_path(missing);
    assert_eq!(
        derived,
        directory(missing).join("INDEX.toml"),
        "不存在 root 仍纯推导返回"
    );
    assert!(!missing.exists(), "纯推导零 IO（不因派生而创建目录）");
    assert!(
        dot_segments(&derived).is_empty(),
        "派生结果无目录穿越段：{derived:?}"
    );
}

// ---------------------------------------------------------------------------
// 形态判据单点（D33 源码自证）与语言分派的唯一 match 点（D40）
// ---------------------------------------------------------------------------

/// 边界（D33 源码自证）：`is_fqn` / `is_repo_relative` / `checked_root` 三判据各**恰 1
/// 处**定义，四类派生函数的形态校验**只经这三处**（零行内重复判据）——放行即让
/// `is_file()` 探测变成任意文件读、让工具 crate 的写盘变成任意文件写。
#[test]
fn 形态判据三单点各一处定义() {
    let source = include_str!("paths.rs");
    for (label, needle) in [
        ("is_fqn", "fn is_fqn("),
        ("is_repo_relative", "fn is_repo_relative("),
        ("checked_root", "fn checked_root("),
    ] {
        assert_eq!(
            source.matches(needle).count(),
            1,
            "`{label}` 恰一处定义（形态判据单点）"
        );
    }

    // 四类派生函数的形态校验只经三处：`contract_path` / `schema_path` 各调 `is_fqn`
    // 一次（共 2），`generated_path` 调 `checked_root`、`entry_form` 调 `checked_root`
    assert_eq!(
        source.matches("!is_fqn(").count(),
        2,
        "`contract_path` / `schema_path` 的形态校验各经 `is_fqn` 一次"
    );
    assert_eq!(
        source.matches("checked_root(").count(),
        3,
        "`checked_root` 定义 1 + 两个消费面（`generated_path` / `entry_form`）"
    );
    // `is_repo_relative` 只被 `checked_root` 消费（定义 1 + 调用 1）
    assert_eq!(
        source.matches("is_repo_relative(").count(),
        2,
        "`is_repo_relative` 只经 `checked_root` 一处消费（零第二份判据）"
    );
}

/// 边界（D40 同源纪律）：`paths.rs` 内对 `Language` 的 `match` **恰 1 处**（本函数体内，
/// `generated_kind`），`api/` 其余产品文件对 `Language` 的 `match` **零命中**——调用方
/// MUST NOT 各自 match 语言再拼 `GeneratedKind`。
#[test]
fn generated_kind是唯一语言match点() {
    let source = include_str!("paths.rs");
    assert_eq!(
        source.matches("match language").count(),
        1,
        "语言 → 生成物种类的 `match` 恰一处（`generated_kind` 体内）"
    );

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let api_dir = manifest.join("src/api");
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&api_dir, &mut hits);
    let language_matches: Vec<&(String, String)> = hits
        .iter()
        .filter(|(path, line)| !path.ends_with("api/paths.rs") && line.contains("match language"))
        .collect();
    assert!(
        language_matches.is_empty(),
        "`api/` 其余产品文件零语言 `match`（调用方零第二份分派）：{language_matches:?}"
    );
}

// ---------------------------------------------------------------------------
// 路径派生的模块段进目标语言源码（D48）：Rust 侧形态闸 / TS 侧字面量转义
// ---------------------------------------------------------------------------

/// 边界（D48；`pub(crate)` 私有备案）：`is_module_segment` 的形态判据是
/// `[A-Za-z_][A-Za-z0-9_]*`（**空串不合法**）——合法 5 形态通过、7 形态被拒。
///
/// **零归一化断言**：`a-b` 恒 `false`（MUST NOT 把 `-` 换成 `_` 后再判——那是第二套命名，
/// D40 明禁）。**与 TS 侧的分工断言**：`is_module_segment` 在 `emit/ts.rs` 内零命中
/// （TS 的字面量面由 `emit::ts::ts_string_literal` 承接）。
#[test]
fn is_module_segment形态判据() {
    for token in ["archi", "Input", "_x", "a1", "A"] {
        assert!(
            is_module_segment(token),
            "合法模块段应通过：{token:?}（`[A-Za-z_][A-Za-z0-9_]*`）"
        );
    }

    // 非法形态逐条：空串 / 带 `.` 与 `-` / 空格（含非 ASCII）/ 引号 / 换行 / 首字符数字 /
    // 纯 emoji（非 ASCII 首字符）
    for token in [
        "",
        "welcome-view.tsx",
        "带 空格",
        "it's",
        "a\nb",
        "1abc",
        "🎯",
    ] {
        assert!(
            !is_module_segment(token),
            "非法模块段应被拒：{token:?}（该值进 `pub use` 的语法位置即不可编译）"
        );
    }

    // 零归一化：`-` 不被换成 `_` 后再判（第二套命名 = D40 明禁的静默错配来源）
    assert!(
        !is_module_segment("a-b"),
        "`is_module_segment(\"a-b\")` 恒 false（MUST NOT 清洗字符集后再判）"
    );
    assert_ne!(
        is_module_segment("a-b"),
        is_module_segment("a_b"),
        "`-` 与 `_` 两态 MUST NOT 被归一成同一结果（零清洗）"
    );

    // 分工断言：Rust 侧的形态闸只在装配点被消费，渲染器零命中（TS 侧用字面量转义）
    let ts_source = include_str!("emit/ts.rs");
    assert!(
        !ts_source.contains("is_module_segment"),
        "`emit/ts.rs` 内零 `is_module_segment`（TS 的 specifier 落在字符串字面量里，\
         由 `ts_string_literal` 承接；拿 Rust 标识符判据筛 TS 会误杀合法透传）"
    );
    let rust_source = include_str!("emit/rust.rs");
    assert!(
        !rust_source.contains("is_module_segment"),
        "`emit/rust.rs` 内零 `is_module_segment`（形态闸在装配期 `api::module_refs`，\
         渲染期零二次校验）"
    );
}

/// 异常（D48 行为面）：透传目标的元素根末段非法 ⇒ 该 FQN **不在** `EmitTarget.modules`
/// ——`render_all` 的 Rust 产物只含 `// 透传自 <FQN>` 注释、**不含** `pub use`
/// （`crate::welcome-view.tsx` 是不可编译文本）；同一目标在 **TS 产物**里**仍入表**
/// （specifier 落在字面量里、带扩展名合法）⇒ 两侧对同一目标的取舍**刻意不同**；
/// 空末段（`"./"` 归一后）两语言都不入表。
///
/// `kind` 取**消费方契约**的语言（`render_all` 按消费方元素根扩展名经
/// `paths::generated_kind` 分派）：A（Rust 消费方）与 D（TS 消费方）透传同一批目标，
/// 于是同一目标在两份产物里的取舍可直接对照。
#[test]
fn module_refs的rust分支筛选() {
    let temp = TempDir::new("module-refs-branch");
    let rust_consumer = "App.Backend.Infra.ArchiRuntime";
    let ts_consumer = "App.Frontend.Views.Architecture";
    let dotted_target = "App.Frontend.Views.Welcome";
    let empty_target = "App.Core.EmptyTail";
    let valid_target = "App.Backend.Core.Foundation";
    let dotted_root = "views/welcome-view.tsx";
    let empty_root = "./";
    let valid_root = "crates/core/foundation/";

    temp.write_model(&format!(
        "[element.\"{rust_consumer}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/infra/archi/\"]\n\n\
         [element.\"{ts_consumer}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"views/architecture.tsx\"]\n\n\
         [element.\"{dotted_target}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"{dotted_root}\"]\n\n\
         [element.\"{empty_target}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"{empty_root}\"]\n\n\
         [element.\"{valid_target}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"{valid_root}\"]\n"
    ));

    // 同一份透传声明由两个消费方共用：B 末段 `welcome-view.tsx`（非法模块段）、
    // C 的 `./` 归一后为空末段（语言无关的退化）
    let reexports = |fqn: &str| {
        format!(
            "# {fqn}\n\n## 职能\n\n正文。\n\n\
             ```json api-reexport\n{{\"AgentView\": \"{dotted_target}\", \
             \"Empty\": \"{empty_target}\", \"Base\": \"{valid_target}\"}}\n```\n\n\
             ## 透传\n\n- AgentView → {dotted_target}\n- Empty → {empty_target}\n\
             - Base → {valid_target}\n\n\
             ```json api-types\n{{\"Thing\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
        )
    };

    // A：Rust 消费方（crate 形态入口 `<dir>/src/lib.rs`）
    temp.write_contract(rust_consumer, &reexports(rust_consumer));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    // D：TS 消费方（元素根本身带 `.tsx`，目录形态入口 `<root>/index.ts`）
    temp.write_contract(ts_consumer, &reexports(ts_consumer));
    temp.write(
        "views/architecture.tsx/index.ts",
        "export const architecture = 1;\n",
    );

    // B：被透传的 TS 元素（同一个带扩展名的元素根，也有自己的入口 → 自带 `.i.ts`）
    temp.write_contract(
        dotted_target,
        "# App.Frontend.Views.Welcome\n\n## 职能\n\n正文。\n\n\
         ```json api-types\n{\"Item\": {\"type\": \"object\", \"properties\": {}}}\n```\n",
    );
    temp.write(
        "views/welcome-view.tsx/index.ts",
        "export interface Item {}\n",
    );

    let elements = [
        element(rust_consumer, &["crates/infra/archi/"]),
        element(ts_consumer, &["views/architecture.tsx"]),
        element(dotted_target, &[dotted_root]),
        element(empty_target, &[empty_root]),
        element(valid_target, &[valid_root]),
    ];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    // Rust 侧（A 的 `.i.rs`）：两目标都不入表 ⇒ 只出注释、零 `pub use`
    let rust_path =
        generated_path(temp.root(), "crates/infra/archi/", GeneratedKind::Rust).expect("Rust 路径");
    let rust_file = files
        .iter()
        .find(|file| file.path == rust_path)
        .expect("A 的 .i.rs 在案");
    for target in [dotted_target, empty_target] {
        assert!(
            rust_file.content.contains(&format!("// 透传自 {target}")),
            "非合法 / 空末段的目标只留透传注释（FQN 原文）：\n{}",
            rust_file.content
        );
    }
    // 合法末段（`foundation`）一侧仍**入表**并产出 `pub use`（正例对照：闸只拒非法形态，
    // 不误杀合法透传）
    assert!(
        rust_file
            .content
            .contains("pub use crate::foundation::Base;"),
        "合法模块段仍走 `pub use <module_path>::<name>`：\n{}",
        rust_file.content
    );
    for forbidden in ["pub use crate::welcome-view.tsx", "pub use crate::::"] {
        assert!(
            !rust_file.content.contains(forbidden),
            "MUST NOT 产出不可编译的 `{forbidden}`：\n{}",
            rust_file.content
        );
    }

    // TS 侧（D 的 `.i.ts`）：同一批目标里 B **仍入表**——带扩展名的 specifier 是前端单文件
    // 元素合法且必需的引用形式（两侧刻意不对称）；C 的空末段在 TS 侧同样不入表
    let ts_path = generated_path(
        temp.root(),
        "views/architecture.tsx",
        GeneratedKind::TypeScript,
    )
    .expect("TS 路径");
    let ts_file = files
        .iter()
        .find(|file| file.path == ts_path)
        .expect("D 的 .i.ts 在案");
    let expected = ts_import_specifier(EntryForm::SubdirModule, "welcome-view.tsx");
    assert_eq!(expected, "../welcome-view.tsx.i");
    assert!(
        ts_file.content.contains(&format!("from '{expected}'")),
        "TS 侧零筛选（带扩展名的 specifier 合法且必需）：\n{}",
        ts_file.content
    );
    assert!(
        ts_file
            .content
            .contains(&format!("// 透传自 {empty_target}")),
        "空末段的目标在 TS 侧同样不入表（语言无关的退化）：\n{}",
        ts_file.content
    );
    assert!(
        !ts_file.content.contains("Empty }"),
        "空末段目标零 specifier：\n{}",
        ts_file.content
    );

    // 形态闸单点在装配点：`is_module_segment(` 在产品侧只被 `api::module_refs` 消费
    let api_source = include_str!("mod.rs");
    assert_eq!(
        api_source.matches("is_module_segment(").count(),
        1,
        "`api/mod.rs` 内 `is_module_segment(` 恰 1 处消费（`module_refs` 的 Rust 分支）"
    );
    assert!(
        api_source.contains("kind == paths::GeneratedKind::Rust"),
        "判据按语言分列（`kind == Rust` 才筛，TS 侧零筛选）"
    );
}

/// 正向（**D48 ②** / 闸二补漏面）：`render` 的 `from` 经 `ts_string_literal` 产出——
/// `ModuleRef.name` 含 `'` / 反斜杠 / 换行时产物仍是**合法 TS**（不是裸插值）。
///
/// 源码自证两条：`ts_string_literal(&paths::ts_import_specifier(` 恰 1 处；裸拼
/// `from '{<插值}` 形态零命中。
#[test]
fn render的from经ts_string_literal() {
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let file = ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md";
    let source = format!(
        "# {fqn}\n\n```json api-reexport\n{{\"Archived\": \"{fqn}\"}}\n```\n\n\
         ## 透传\n\n- Archived → {fqn}\n"
    );
    let contract = parse(&source, fqn, file).expect("契约应解析成功");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");

    for name in ["it's", "a\\b", "a\nb"] {
        let mut modules = BTreeMap::new();
        modules.insert(
            fqn.to_owned(),
            ModuleRef {
                name: name.to_owned(),
                module_path: rust_module_path("crates/infra/archi"),
            },
        );
        let target = EmitTarget {
            kind: GeneratedKind::TypeScript,
            form: EntryForm::SubdirModule,
            modules,
        };
        let text = render_ts(&contract, &schema, &meta(), &target);
        let expected = ts_import_specifier(EntryForm::SubdirModule, name);
        let literal = crate::api::emit::ts::ts_string_literal(&expected);
        assert!(
            text.contains(&format!("from {literal};")),
            "specifier 经 `ts_string_literal` 产出（合法 TS，非裸插值；name = {name:?}）：\n{text}"
        );
        // 断言单引号串形态：`from '` 开、`';` 收（裸插值同样形，故再钉转义后的差异）
        assert!(
            literal.starts_with('\'') && literal.ends_with('\''),
            "字面量首末均为单引号：{literal:?}"
        );
        if name.contains('\'') {
            assert!(
                literal.contains("\\'"),
                "`'` 被转义（裸插值会断串）：{literal:?}"
            );
        }
        if name.contains('\\') {
            assert!(literal.contains("\\\\"), "反斜杠被转义：{literal:?}");
        }
        if name.contains('\n') {
            assert!(
                !literal.contains('\n'),
                "裸换行 MUST NOT 进字面量：{literal:?}"
            );
        }
    }

    let ts_source = include_str!("emit/ts.rs");
    assert_eq!(
        ts_source
            .matches("ts_string_literal(&paths::ts_import_specifier(")
            .count(),
        1,
        "`ts_string_literal(&paths::ts_import_specifier(` 恰 1 处（唯一 specifier 转义点）"
    );
    assert!(
        !ts_source.contains("from '{"),
        "裸拼 `from '{{<插值>` 的形态零命中（D48 ② 的补漏面）"
    );
    assert!(
        !ts_source.contains("from '{specifier}'"),
        "裸拼 `from '{{specifier}}'` 的形态零命中"
    );
}

// ---------------------------------------------------------------------------
// 模块段的关键字面（D50）：关键字即拒绝、常量表单点持有、零第二形态
// ---------------------------------------------------------------------------

/// 从来源文件内机械抽取 `const RUST_KEYWORDS` 的 51 个字面量（两段之间不插行注释，
/// 故逐值对拍不会因折行失真）。
fn rust_keywords(source: &str) -> Vec<String> {
    let start = source
        .find("const RUST_KEYWORDS: [&str; 51] = [")
        .expect("`const RUST_KEYWORDS: [&str; 51]` 在案");
    let body = &source[start..];
    let end = body.find("];").expect("常量表以 `];` 收尾");
    body[..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// 正向（**D50** 行为判据）：Rust 关键字一律拒绝——`pub use crate::<关键字>::X;` 是
/// **语法位置**，放行即产出不可编译文本（本仓真实来源：
/// `crates/infra/agent/src/sdk/mod.rs` 的 `pub(crate) mod r#loop;`）；合法非关键字一律
/// 通过。判定只经 `is_module_segment` 的公共语义观测（`pub(crate)` 对本模块可见，
/// 零 test-only `pub` 放宽）。
#[test]
fn is_module_segment关键字即拒绝() {
    for keyword in [
        "loop", "crate", "self", "type", "super", "impl", "use", "fn", "mod", "pub", "struct",
        "enum", "match", "move", "ref", "where", "while", "return", "static", "trait", "unsafe",
        "as", "async", "await", "break", "const", "continue", "dyn", "else", "extern", "false",
        "for", "if", "in", "let", "mut", "Self", "true", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ] {
        assert!(
            !is_module_segment(keyword),
            "关键字 `{keyword}` MUST 被拒（进 `pub use` 的模块段位置即不可编译）"
        );
    }

    for token in [
        "archi",
        "agent",
        "foundation",
        "welcome",
        "_x1",
        "x",
        "A1",
        "u8",
    ] {
        assert!(
            is_module_segment(token),
            "合法非关键字 `{token}` MUST 通过（闸只拒关键字，不误伤形态合法名）"
        );
    }

    // 反例：`r#<关键字>` 是第二形态、MUST NOT 产出——一旦引入，`paths.rs` 内必现 `r#`
    let source = include_str!("paths.rs");
    assert_eq!(
        source.matches("\"r#\"").count(),
        0,
        "`paths.rs` 内零 `\"r#\"` 形态（零改名、零第二形态）"
    );
}

/// 边界（**D48 / D50**）：字符集与空串边界——`[A-Za-z_][A-Za-z0-9_]*` 且空串不合法；
/// **零归一化**（`-` 不被换成 `_`、不剥扩展名、不大小写折叠）；超长形态不 panic。
#[test]
fn is_module_segment字符集边界() {
    for token in [
        "",
        "1abc",
        "a-b",
        "a b",
        "a.b",
        "a/b",
        "welcome-view.tsx",
        "中文",
        "🎯",
        "a\nb",
        "a/b/c",
        "$x",
    ] {
        assert!(
            !is_module_segment(token),
            "非法模块段 MUST 被拒：{token:?}（形态 `[A-Za-z_][A-Za-z0-9_]*`）"
        );
    }

    for token in ["_", "_9", "A", "a", "abc123", "A_b_C"] {
        assert!(
            is_module_segment(token),
            "形态合法且非关键字 MUST 通过：{token:?}"
        );
    }

    // 超长（>1000 chars）合法形态：不 panic、裁定可判
    let long = "a".repeat(1200);
    assert!(
        is_module_segment(&long),
        "超长纯 ASCII 标识符仍是合法形态（无长度上限）"
    );

    // 零归一化：`-` 不被清洗成 `_` 后再判（清洗即第二套命名，D40 明禁）
    assert_ne!(
        is_module_segment("a-b"),
        is_module_segment("a_b"),
        "零归一化：`a-b` 与 `a_b` 的裁定 MUST NOT 相同（不自动改写）"
    );

    // 末段取自路径时零清洗：`module_name` 体内零字符改写调用
    let source = include_str!("paths.rs");
    let start = source
        .find("pub fn module_name(element_root: &str) -> String {")
        .expect("`module_name` 在案");
    let body = &source[start..];
    let end = body.find("\n}\n").expect("`module_name` 函数体收尾");
    let body = &body[..end];
    for cleaner in [
        "replace(",
        "trim_matches(",
        "trim_end_matches(",
        "to_lowercase(",
    ] {
        assert!(
            !body.contains(cleaner),
            "`module_name` 体内零 `{cleaner}`（纯派生、零清洗）"
        );
    }
}

/// 正向（**D50**）：`RUST_KEYWORDS` 恰 **51** 值（38 严格关键字 + 13 保留字），逐值
/// `is_module_segment(词) == false`；去重后长度不变（漏值 / 重复不可绕过）。值的真值源
/// 是产品文件内的常量表本体（`include_str!` 机械抽取，MUST NOT 手抄第二份）。
#[test]
fn rust_keywords封闭五十一值() {
    let source = include_str!("paths.rs");
    let keywords = rust_keywords(source);
    assert_eq!(
        keywords.len(),
        51,
        "常量表恰 51 值（38 严格关键字 + 13 保留字）：{keywords:?}"
    );

    let mut unique = keywords.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 51, "常量表零重复（去重后长度不变）");

    for keyword in &keywords {
        assert!(
            !is_module_segment(keyword),
            "常量表内的 `{keyword}` MUST 被 `is_module_segment` 拒绝"
        );
    }

    // 38 + 13 两段的分界：严格关键字段与保留字段各在案（首段末位 `while`、次段首位
    // `abstract`，两段之间不插行注释，故此处逐值对拍不受折行影响）
    assert_eq!(
        keywords.iter().position(|word| word.as_str() == "abstract"),
        Some(38),
        "严格关键字段恰 38 项（`abstract` 是保留字段首位）"
    );
    assert_eq!(
        keywords.last().map(String::as_str),
        Some("yield"),
        "保留字段末位 `yield`"
    );
}

/// 边界（**D50** 源码自证）：`const RUST_KEYWORDS` 在 `paths.rs` 内**恰 1 处**且类型为
/// `[&str; 51]`；零 `matches!(token` 长臂（关键字判据 MUST 由常量表持有、MUST NOT 散在
/// 匹配臂里）；`is_module_segment` 体内**同时**含常量引用与字符集判定（两项缺一即不成
/// 合取）；关键字集在其余产品文件内零第二份。
#[test]
fn rust_keywords单点持有() {
    let source = include_str!("paths.rs");
    assert_eq!(
        source.matches("const RUST_KEYWORDS").count(),
        1,
        "`const RUST_KEYWORDS` 恰 1 处定义"
    );
    assert!(
        source.contains("const RUST_KEYWORDS: [&str; 51]"),
        "常量类型为 `[&str; 51]`（长度即契约，改表必改类型）"
    );
    assert!(
        !source.contains("matches!(token"),
        "零 `matches!(token` 长臂（常量表之外的匹配臂形态即第二份判据）"
    );

    // 判据体的合取二项：常量引用 + 字符集判定
    let start = source
        .find("pub(crate) fn is_module_segment(token: &str) -> bool {")
        .expect("`is_module_segment` 在案");
    let body = &source[start..];
    let end = body.find("\n}\n").expect("判据体收尾");
    let body = &body[..end];
    assert!(
        body.contains("RUST_KEYWORDS"),
        "判据体内引用常量表（合取第二项）：\n{body}"
    );
    assert!(
        body.contains("is_ascii_alphabetic") && body.contains("is_ascii_alphanumeric"),
        "判据体内含字符集判定（合取第一项）：\n{body}"
    );

    // 关键字集零第二份：其余产品文件不各持一份关键字清单
    for (path, other) in [
        ("emit/rust.rs", include_str!("emit/rust.rs")),
        ("emit/ts.rs", include_str!("emit/ts.rs")),
        ("contract.rs", include_str!("contract.rs")),
        ("check.rs", include_str!("check.rs")),
    ] {
        assert!(
            !other.contains("RUST_KEYWORDS"),
            "`{path}` 内零 `RUST_KEYWORDS`（关键字表单点）"
        );
    }
}

/// 异常（**D50** 源码自证 / 逐产品文件枚举——MUST NOT 目录级 `grep`：`*_test.rs` 的
/// 字面量会把判据变噪声）：`paths.rs` 与 `emit/rust.rs` 内 `"r#"` 与 `r#{` 各 **0**；
/// `paths.rs` 内零 `replace('-', '_')`（零清洗 = 零第二套命名的直接证据）。
#[test]
fn 零第二形态与零清洗() {
    for (path, source) in [
        ("paths.rs", include_str!("paths.rs")),
        ("emit/rust.rs", include_str!("emit/rust.rs")),
    ] {
        assert_eq!(
            source.matches("\"r#\"").count(),
            0,
            "`{path}` 内零 `\"r#\"`（改名成原始标识符是第二形态，MUST NOT）"
        );
        assert_eq!(
            source.matches("r#{").count(),
            0,
            "`{path}` 内零 `r#{{`（`format!` 侧同样不产原始标识符）"
        );
    }

    let source = include_str!("paths.rs");
    for cleaner in [
        "replace('-', '_')",
        "replace('-',\"_\")",
        "replace('-', \"_\")",
    ] {
        assert!(
            !source.contains(cleaner),
            "`paths.rs` 内零 `{cleaner}`（末段零清洗：清洗即静默错配）"
        );
    }
}
