//! `api::paths` 的单元测试：`<root>` 归一两步法（不去扩展名）、四类路径派生
//! （契约 / schema / 生成物 / 索引）、入口解析链三态与四条判据、`#[path]` 值退化为
//! 生成物文件名、跨目录 `$ref` 前缀、分层目录段与语言判定。
//!
//! 零 mock：`entry_state` / `generated_path` 的存在性探测用真实临时目录落空文件
//! （零内容即足以驱动分支），其余函数零 IO；`foundation::layout` 的真实派生值是
//! 前缀断言的基准（路径单点即被测语义）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::{archi_graph_dir, archi_models_dir};

use crate::api::emit::ts::render as render_ts;
use crate::api::emit::{EmitTarget, ModuleRef};
use crate::api::mod_test::{declare_ancestors, mirrored_schema_text, reexport_def};
use crate::api::{load_contracts, meta, render_all};
use crate::dto::{ApiStatus, ArchiElement, ElementKind, GateTier, NodeMark};

use super::paths::{
    checked_root, contract_path, dir_segments, directory, entry_candidates, entry_state,
    generated_kind, generated_path, index_path, is_fqn, is_module_segment, language_of,
    module_name, normalize_root, ref_relative, rust_module_path, schema_fqn, schema_path,
    ts_import_specifier, EntryForm, EntryProblem, EntryState, GeneratedKind, Language,
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

    /// 落一个空的 repo 相对文件（自动建父目录；`entry_state` 只看 `is_file`）。
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

    /// 落模型（`.dev-team/architecture/graph/model/01-test.toml`）。
    ///
    /// 叶元素的祖先链自动补齐：`render_all` 的元素表经 `dto::graph_view` 取数，
    /// 缺祖先即 `orphanHierarchy` 结构错（fail-closed）。
    fn write_model(&self, content: &str) {
        self.write(
            ".dev-team/architecture/graph/model/01-test.toml",
            &declare_ancestors(content),
        );
    }

    /// 落两资产：文档（`<archi_models_dir>/<分层目录…>/<FQN>.md`，只为「在盘」这一事实）+
    /// 手写接口定义原文（`.schema.json`）。渲染的输入面是接口定义，两资产成对落盘。
    fn write_contract(&self, fqn: &str, schema: &str) {
        let path = contract_path(self.root(), fqn).expect("测试 FQN 恒为合法形态");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建契约目录失败");
        }
        fs::write(&path, super::mod_test::doc_of(fqn)).expect("写契约失败");
        let schema_path = super::paths::schema_path(self.root(), fqn).expect("合法 FQN");
        fs::write(schema_path, schema).expect("写接口定义失败");
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

/// 递归收集目录下的 repo 相对文件清单（盘面零变化断言用，0 起始无隐藏过滤）。
fn tree_listing(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
                continue;
            }
            out.push(
                path.strip_prefix(base)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
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
// contract_path / schema_path：同目录、分层目录段
// ---------------------------------------------------------------------------

/// 正向（AC-4）：`App.Backend.Infra.ArchiRuntime` → `<archi_models_dir>/App/Backend/Infra/...`
/// （FQN 段链去掉叶段）；两函数父目录逐字相等、均以 `archi_models_dir(root)` 为前缀
/// （零行内段名）。
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
        "目录末段 = 父 FQN 末段"
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
        Some(
            archi_models_dir(root)
                .join("App")
                .join("Backend")
                .join("Infra")
                .as_path()
        ),
        "目录段 = FQN 段链去叶段（App.Backend.Infra → App/Backend/Infra）"
    );
}

/// 边界：单段 FQN 落契约目录根（无分层段）；含 emoji 的合法 FQN → 路径可判且零目录
/// 穿越（不含 `..` 段）；形态非法的 FQN → `None`。
#[test]
fn contract_path顶层落契约目录根且零穿越() {
    let root = Path::new("/ws");
    assert_eq!(
        contract_path(root, "App")
            .and_then(|path| path.parent().map(|parent| parent.to_path_buf())),
        Some(directory(root)),
        "单段 FQN 落契约目录根"
    );
    assert_eq!(
        contract_path(root, "App.Backend")
            .and_then(|path| path.parent().map(|parent| parent.to_path_buf())),
        Some(directory(root).join("App")),
        "根段参与目录分层（只去掉叶段）"
    );

    for fqn in ["App.🎯.Target", "App.Core.Alpha"] {
        let path = contract_path(root, fqn).expect("合法 FQN 应可派生");
        assert!(
            dot_segments(&path).is_empty(),
            "契约路径 MUST NOT 含目录穿越段：{fqn:?} → {path:?}"
        );
        assert!(
            path.starts_with(archi_models_dir(root)),
            "契约路径恒在 archi_models_dir 之下：{fqn:?}"
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
// generated_path：入参是入口（落点 = 入口同级）
// ---------------------------------------------------------------------------

/// 正向（AC-3）：crate / 子目录模块 / 文件型根三种入口形态的落点均为
/// `<入口目录>/<入口主干>.i.<后缀>`——入口同级，不再回元素根同级。
#[test]
fn generated_path三种入口形态() {
    let root = Path::new("/ws");
    let cases = [
        (
            "crates/infra/archi/src/lib.rs",
            GeneratedKind::Rust,
            "/ws/crates/infra/archi/src/lib.i.rs",
        ),
        (
            "crates/core/archi/src/scan/mod.rs",
            GeneratedKind::Rust,
            "/ws/crates/core/archi/src/scan/mod.i.rs",
        ),
        (
            "views/welcome-view.tsx",
            GeneratedKind::TypeScript,
            "/ws/views/welcome-view.i.ts",
        ),
    ];
    for (entry, kind, expected) in cases {
        let path = generated_path(root, entry, kind).expect("路径应可派生");
        assert_eq!(path, PathBuf::from(expected), "生成物落点：{entry}");
        let entry_dir = Path::new(entry).parent().expect("入口有目录段");
        assert_eq!(
            path.parent(),
            Some(root.join(entry_dir).as_path()),
            "入口同级：与入口同目录"
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
/// `generated_path` 给 `None`、`entry_state` 给 `Absent`（候选表为空）——
/// `root.join(绝对路径)` 会丢弃 base、`..` 段会上溯、`~` 会被下游 shell 形态展开，
/// 放行即让生成物落到仓外。
#[test]
fn generated_path与entry_state拒收越界形态() {
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
        assert_eq!(
            entry_state(root, std::slice::from_ref(&candidate), &candidate),
            EntryState::Absent,
            "入口解析拒收：{raw:?}"
        );
    }
}

/// 边界（主干取法）：入口主干 = 文件名最后一次 `.` 之前的部分；无扩展名 / 仓根入口
/// 不丢文件名；扩展名只随 `kind`（`kind` 由调用方经 `generated_kind` 分派，本函数零
/// 第二次语言判定）。
#[test]
fn generated_path主干取法与扩展名随kind() {
    let root = Path::new("/ws");
    assert_eq!(
        generated_path(root, "views/a.b.tsx", GeneratedKind::TypeScript),
        Some(root.join("views/a.b.i.ts")),
        "主干取最后一次 `.` 之前（`a.b` 整体保留）"
    );
    assert_eq!(
        generated_path(root, "lib", GeneratedKind::Rust),
        Some(root.join("lib.i.rs")),
        "无扩展名入口：整个文件名即主干"
    );
    assert_eq!(
        generated_path(root, "lib.rs", GeneratedKind::Rust),
        Some(root.join("lib.i.rs")),
        "仓根入口不丢文件名（目录段为空）"
    );
    for (kind, expected) in [
        (GeneratedKind::Rust, "src/view.i.rs"),
        (GeneratedKind::TypeScript, "src/view.i.ts"),
    ] {
        assert_eq!(
            generated_path(root, "src/view.ts", kind),
            Some(root.join(expected)),
            "扩展名只随 kind：{kind:?}"
        );
    }
}

/// 异常（D41 / 删除面）：`entry_form` 与 `mod_path_attr` 两个函数体已删——`paths.rs`
/// 内零定义 / 零词形（含注释）、产品 `.rs` 零调用形、本测试文件不再 import 该名。
#[test]
fn 已删函数零残留() {
    let entry_form = ["entry", "_form"].concat();
    let mod_path_attr = ["mod_path", "_attr"].concat();

    let source = include_str!("paths.rs");
    for removed in [&entry_form, &mod_path_attr] {
        assert_eq!(
            source.matches(&format!("fn {removed}(")).count(),
            0,
            "`{removed}` 定义已删"
        );
        assert_eq!(
            source.matches(removed.as_str()).count(),
            0,
            "`{removed}` 在 `paths.rs` 内零命中（含注释）"
        );
    }

    // 产品 `.rs` 的调用形零命中（扫描排除 `*_test.rs`；注释里的词形不构成调用）
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut src_tauri = manifest.clone();
    for _ in 0..3 {
        src_tauri.pop();
    }
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&src_tauri.join("crates"), &mut hits);
    scan_product_rs(&src_tauri.join("src"), &mut hits);
    let calls: Vec<&(String, String)> = hits
        .iter()
        .filter(|(_, line)| {
            let code = line.trim_start();
            !code.starts_with("//")
                && (code.contains(&format!("{entry_form}("))
                    || code.contains(&format!("{mod_path_attr}(")))
        })
        .collect();
    assert!(calls.is_empty(), "产品 `.rs` 零旧名调用：{calls:?}");

    // 本测试文件的 import 区不再引入这两个名字
    let own = include_str!("paths_test.rs");
    let header = own
        .find("// ---")
        .map(|index| &own[..index])
        .expect("测试文件以注释分隔区成形");
    for removed in [&entry_form, &mod_path_attr] {
        assert!(
            !header.contains(removed.as_str()),
            "`paths_test.rs` 的 import 区不再含 `{removed}`"
        );
    }
}

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
// entry_state：入口解析链三态（显式 / 文件型根 / 约定默认 / Absent）
// ---------------------------------------------------------------------------

/// 正向（AC-1 ①②③）：显式 `entry` 命中；文件型根 = 根文件自身；目录型根按
/// `src/lib.rs` → `mod.rs` → `index.ts` → `index.tsx` 的顺序取首个落盘者。
#[test]
fn entry_state三态各命中一条() {
    let temp = TempDir::new("entry-states");

    // ① 显式 entry（目录型壳：约定默认拼成 .../src/src/lib.rs，指不到）
    let mut shell = element("App.Backend.Shell", &["src-tauri/src/"]);
    shell.entry = Some("src-tauri/src/lib.rs".to_owned());
    temp.touch("src-tauri/src/lib.rs");
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&shell), &shell),
        EntryState::Resolved {
            form: EntryForm::Crate,
            entry: "src-tauri/src/lib.rs".to_owned(),
        },
        "显式 entry 命中（Crate：入口以 src/lib.rs 收尾）"
    );

    // ② 文件型根 = 根文件自身（MUST NOT 在已带扩展名的根上再拼 `.tsx`）
    temp.touch("src/views/welcome-view.tsx");
    let single = element(
        "App.Frontend.Views.Welcome",
        &["src/views/welcome-view.tsx"],
    );
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&single), &single),
        EntryState::Resolved {
            form: EntryForm::SingleFile,
            entry: "src/views/welcome-view.tsx".to_owned(),
        },
        "文件型根：根文件自身即入口"
    );

    // ③ 目录型根：四条约定候选按序，取首个落盘者（不做语言判定）
    let crate_element = element("App.Backend.Core.Archi", &["crates/core/archi/"]);
    temp.touch("crates/core/archi/src/lib.rs");
    temp.touch("crates/core/archi/mod.rs");
    assert_eq!(
        entry_state(
            temp.root(),
            std::slice::from_ref(&crate_element),
            &crate_element
        ),
        EntryState::Resolved {
            form: EntryForm::Crate,
            entry: "crates/core/archi/src/lib.rs".to_owned(),
        },
        "第一候选优先（src/lib.rs 胜过 mod.rs）"
    );
    fs::remove_file(temp.root().join("crates/core/archi/src/lib.rs")).expect("移除 lib.rs");
    assert_eq!(
        entry_state(
            temp.root(),
            std::slice::from_ref(&crate_element),
            &crate_element
        ),
        EntryState::Resolved {
            form: EntryForm::SubdirModule,
            entry: "crates/core/archi/mod.rs".to_owned(),
        },
        "第二候选：<root>/mod.rs"
    );

    let view = element("App.Frontend.Views", &["src/views/"]);
    temp.touch("src/views/index.ts");
    temp.touch("src/views/index.tsx");
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&view), &view),
        EntryState::Resolved {
            form: EntryForm::SubdirModule,
            entry: "src/views/index.ts".to_owned(),
        },
        "前端目录根：index.ts 先于 index.tsx"
    );

    // ④ 其余：`path` 空 / 约定候选全不落盘 → Absent
    let container = element("App", &[]);
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&container), &container),
        EntryState::Absent,
        "容器元素（path 空）无入口"
    );
    let missing = element("App.Frontend.Views.Missing", &["src/views/missing/"]);
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&missing), &missing),
        EntryState::Absent,
        "目录不存在 → 无入口"
    );
}

/// 边界（AC-1 候选派生零 IO）：`entry_candidates` 只做字符串运算——目录型根的四条
/// 约定候选逐个在案，且**不做语言判定**（同一目录根同时列出 rs 与 ts(x) 候选）。
#[test]
fn entry_candidates目录型根四候选() {
    let candidates = entry_candidates(&element("App.Backend.Core.Archi", &["crates/core/archi"]));
    assert_eq!(
        candidates,
        vec![
            "crates/core/archi/src/lib.rs".to_owned(),
            "crates/core/archi/mod.rs".to_owned(),
            "crates/core/archi/index.ts".to_owned(),
            "crates/core/archi/index.tsx".to_owned(),
        ],
        "语言无关、按优先序列出（目录型根无扩展名，按语言分派会让前端候选失效）"
    );
    assert_eq!(
        entry_candidates(&element(
            "App.Frontend.Views.Welcome",
            &["views/welcome-view.tsx"]
        )),
        vec!["views/welcome-view.tsx".to_owned()],
        "文件型根 = 根文件自身（单候选）"
    );
    assert_eq!(
        entry_candidates(&element("App", &[])),
        Vec::<String>::new(),
        "path 空 → 零候选"
    );
    // 已删的三条不在列：`<dir>.rs` / `<dir>.ts` / `<dir>.tsx`（只能指到 path 之外）
    let dir_candidates = entry_candidates(&element("App.X", &["src/x"]));
    for absent in ["src/x.rs", "src/x.ts", "src/x.tsx", "src/x/lib.rs"] {
        assert!(
            !dir_candidates.iter().any(|candidate| candidate == absent),
            "{absent} MUST NOT 在候选列内"
        );
    }
}

/// 异常（AC-2 四条判据各一反例）：形态非法 / 落位越界 / 不在盘 / 跨元素同一入口，
/// 外加「单文件根声明同目录的另一个文件」归 `OutsidePath`（落位判据兜住，不设第五条）；
/// 存在判据只看声明值——同目录 `src/lib.rs` 在盘也不回落到约定候选。
#[test]
fn entry_state四条判据各一反例() {
    let temp = TempDir::new("entry-problems");
    temp.touch("src/a/x.rs");
    temp.touch("src/b/y.rs");

    let mut malformed = element("App.A", &["src/a/"]);
    malformed.entry = Some("/abs/x.rs".to_owned());
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&malformed), &malformed),
        EntryState::Invalid(EntryProblem::Malformed {
            entry: "/abs/x.rs".to_owned(),
        }),
        "绝对路径 → Malformed"
    );

    let mut outside = element("App.A", &["src/a/"]);
    outside.entry = Some("src/b/y.rs".to_owned());
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&outside), &outside),
        EntryState::Invalid(EntryProblem::OutsidePath {
            entry: "src/b/y.rs".to_owned(),
        }),
        "落在本元素 path 之外 → OutsidePath"
    );

    let mut not_on_disk = element("App.A", &["src/a/"]);
    // 存在判据 MUST NOT 回落约定候选：同目录 `src/a/src/lib.rs` 在盘，显式声明仍只看声明值
    temp.touch("src/a/src/lib.rs");
    not_on_disk.entry = Some("src/a/missing.rs".to_owned());
    assert_eq!(
        entry_state(
            temp.root(),
            std::slice::from_ref(&not_on_disk),
            &not_on_disk
        ),
        EntryState::Invalid(EntryProblem::NotOnDisk {
            entry: "src/a/missing.rs".to_owned(),
        }),
        "声明值不在盘 → NotOnDisk（同目录 src/lib.rs 在盘也不回落约定候选）"
    );

    // 入口 MUST 落在自己 path 内才走到唯一判据：让两条 path 各自覆盖同一文件
    let mut first = element("App.A", &["src/a/"]);
    first.entry = Some("src/a/x.rs".to_owned());
    let mut second = element("App.B", &["src/a/x.rs"]);
    second.entry = Some("src/a/x.rs".to_owned());
    let both = vec![first.clone(), second.clone()];
    for element in [&first, &second] {
        let other = if element.fqn == "App.A" {
            "App.B"
        } else {
            "App.A"
        };
        match entry_state(temp.root(), &both, element) {
            EntryState::Invalid(EntryProblem::Duplicated { other_fqn, .. }) => {
                assert_eq!(other_fqn, other, "点名冲突方 FQN");
            }
            other_state => panic!("两元素同一入口应判 Duplicated，实际：{other_state:?}"),
        }
    }

    // 单文件根声明「同目录的另一个文件」→ 落位判据覆盖（不设第五条判据）
    temp.touch("src/solo/a.ts");
    temp.touch("src/solo/b.ts");
    let mut file_root = element("App.Solo", &["src/solo/a.ts"]);
    file_root.entry = Some("src/solo/b.ts".to_owned());
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&file_root), &file_root),
        EntryState::Invalid(EntryProblem::OutsidePath {
            entry: "src/solo/b.ts".to_owned(),
        }),
        "文件型根声明别的文件 → OutsidePath"
    );
}

/// 异常（test-design:74 逐值）：基准判据逐值拒收——绝对路径 / `..` 段 / 盘符 / `~` 起始 /
/// 空串，各返 `Malformed { entry }`，载荷是**归一后原文**（`\` 先归一为 `/`，其余零清洗）。
#[test]
fn entry_state形态非法逐值拒收() {
    let temp = TempDir::new("entry-malformed-values");
    // （声明值，归一后原文）：`C:\x.rs` 的反斜杠由 `normalize_root` 归一为 `/`，
    // 其余四值归一后逐字不变——载荷即「归一后原文」，故逐值对拍。
    let cases = [
        ("/abs/x.rs", "/abs/x.rs"),
        ("../x.rs", "../x.rs"),
        ("C:\\x.rs", "C:/x.rs"),
        ("~/x.rs", "~/x.rs"),
        ("", ""),
    ];
    for (declared, normalized) in cases {
        let mut element = element("App.A", &["src/a/"]);
        element.entry = Some(declared.to_owned());
        assert_eq!(
            entry_state(temp.root(), std::slice::from_ref(&element), &element),
            EntryState::Invalid(EntryProblem::Malformed {
                entry: normalized.to_owned(),
            }),
            "声明 {declared:?} 形态非法 ⇒ Malformed（载荷为归一后原文 {normalized:?}）"
        );
    }
}

/// 异常（test-design:75 逐值）：落位判据的跨元素两形态——① A 的 `entry` 等于**元素 B 的
/// `path` 条目**；② A 的 `entry` 落在 B 的**目录型** `path` 之内。两者在盘、且对 B 合法，
/// 但对 A 都在其 `path` 之外 ⇒ 各返 `OutsidePath { entry }`（落位判据按本元素 `path` 判，
/// MUST NOT 因「别的元素认领了它」而放行）。
#[test]
fn entry_state落位判据跨元素越界逐值() {
    let temp = TempDir::new("entry-outside-other-element");
    temp.touch("src/b/entry.rs");
    temp.touch("src/c/mod.rs");

    let file_root = element("App.B", &["src/b/entry.rs"]);
    let dir_root = element("App.C", &["src/c/"]);
    let others = [file_root, dir_root];

    // ① `entry` 等于另一元素的 `path` 条目（文件型根）
    let mut same_as_other_path = element("App.A", &["src/a/"]);
    same_as_other_path.entry = Some("src/b/entry.rs".to_owned());
    assert_eq!(
        entry_state(temp.root(), &others, &same_as_other_path),
        EntryState::Invalid(EntryProblem::OutsidePath {
            entry: "src/b/entry.rs".to_owned(),
        }),
        "A 的 entry 等于 B 的 path 条目 ⇒ OutsidePath（B 的根不是 A 的根）"
    );

    // ② `entry` 落在另一元素的目录型 `path` 之内
    let mut inside_other_dir = element("App.A", &["src/a/"]);
    inside_other_dir.entry = Some("src/c/mod.rs".to_owned());
    assert_eq!(
        entry_state(temp.root(), &others, &inside_other_dir),
        EntryState::Invalid(EntryProblem::OutsidePath {
            entry: "src/c/mod.rs".to_owned(),
        }),
        "A 的 entry 落在 B 的目录型 path 之内 ⇒ OutsidePath（前缀匹配只对本元素生效）"
    );
}

/// 异常：`<dir>/lib.rs`（非 `src/lib.rs`）MUST NOT 在列——如实报无入口。
#[test]
fn entry_candidates排除目录内lib_rs() {
    let temp = TempDir::new("lib-rs-not-listed");
    temp.touch("crates/core/archi/lib.rs");
    let element = element("App.Backend.Core.Archi", &["crates/core/archi/"]);
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&element), &element),
        EntryState::Absent,
        "crate 的 lib.rs 恒在 src/ 下；<dir>/lib.rs 不是入口"
    );
}

/// 异常（AC-4 基线的成因）：目录内只有非入口名文件 → 无入口。
#[test]
fn entry_state约定候选全不落盘() {
    let temp = TempDir::new("entry-none");
    temp.touch("src/views/x.ts");
    let element = element("App.Frontend.Views", &["src/views/"]);
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&element), &element),
        EntryState::Absent,
        "目录内只有 views/x.ts，四条约定候选皆不落盘"
    );
}

/// 边界（AC-1 ④ / 三态封闭）：`Absent` 的三形态逐值可判——① 无声明且目录型根四条约定
/// 候选全不落盘；② `path` 空（容器，零候选）；③ `path` 首条形态非法（`checked_root` 收
/// 空 ⇒ 零候选）。**形态判据只看声明的 `entry`**，无声明即无 `Malformed`，故第三种成因
/// 仍归 `Absent`（与 `Invalid` 可分辨，不是同一个含糊终态）。
#[test]
fn entry_state的三种缺席形态() {
    let temp = TempDir::new("entry-absent-three");
    temp.touch("src/views/x.ts");

    // ① 目录型根：候选表恒四条，盘上一个都不在 → Absent
    let no_default = element("App.Frontend.Views", &["src/views/"]);
    assert_eq!(
        entry_candidates(&no_default).len(),
        4,
        "目录型根恒四条约定候选"
    );
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&no_default), &no_default),
        EntryState::Absent,
        "四候选全不落盘 → Absent"
    );

    // ② / ③ 零候选的两种成因：容器（`path` 空）与根形态非法
    for (tag, raw) in [("path 空", None), ("根形态非法", Some("/abs/x"))] {
        let target = match raw {
            Some(raw) => element("App.Core.Bad", &[raw]),
            None => element("App", &[]),
        };
        assert_eq!(
            entry_candidates(&target),
            Vec::<String>::new(),
            "{tag} → 零候选"
        );
        assert_eq!(
            entry_state(temp.root(), std::slice::from_ref(&target), &target),
            EntryState::Absent,
            "{tag} ⇒ Absent（无声明即无 Malformed，不进 Invalid）"
        );
    }
}

// ---------------------------------------------------------------------------
// entry_candidates：显式优先 / 非法声明原样返回 / 非源扩展名归目录型 / 零 IO
// ---------------------------------------------------------------------------

/// 正向（AC-1 ①）：显式 `entry` 归一后是**唯一**候选——哪怕元素根是目录型，也不追加
/// 任何约定形态候选。
#[test]
fn entry_candidates显式声明单候选优先() {
    let mut declared = element("App.Frontend.Views.Welcome", &["views/"]);
    declared.entry = Some("./views/welcome-view.tsx".to_owned());
    assert_eq!(
        entry_candidates(&declared),
        vec!["views/welcome-view.tsx".to_owned()],
        "显式声明归一后单候选（`./` 前缀剥除），不追加约定候选"
    );
    for default_candidate in [
        "views/src/lib.rs",
        "views/mod.rs",
        "views/index.ts",
        "views/index.tsx",
    ] {
        assert!(
            !entry_candidates(&declared)
                .iter()
                .any(|candidate| candidate == default_candidate),
            "显式声明后 MUST NOT 追加约定候选 {default_candidate}"
        );
    }
}

/// 边界：非源扩展名的根（`.py` / `.txt`）不按文件型根命中——白名单判据只有一份
/// （`scan::is_source_file`），故一律归目录型根（四条约定候选）。
#[test]
fn entry_candidates非源扩展名根归目录型() {
    for root in ["foo.py", "x.txt"] {
        let expected: Vec<String> = ["src/lib.rs", "mod.rs", "index.ts", "index.tsx"]
            .iter()
            .map(|tail| format!("{root}/{tail}"))
            .collect();
        assert_eq!(
            entry_candidates(&element("App.X", &[root])),
            expected,
            "非源扩展名根不按文件型根命中：{root}"
        );
    }
}

/// 边界：`path` 空 → 空表；根形态非法（绝对 / `..` / 盘符 / `~` / 空串 / 空段）→
/// 空表——候选派生与 `checked_root` 同口径，非法形态不产出可穿越候选。
#[test]
fn entry_candidates空表与非法根形态() {
    assert_eq!(
        entry_candidates(&element("App", &[])),
        Vec::<String>::new(),
        "容器元素零候选"
    );
    for raw in ["/abs/x", "../x", r"C:\x", "~/x", "", "a//b"] {
        assert_eq!(
            entry_candidates(&element("App.X", &[raw])),
            Vec::<String>::new(),
            "非法根形态零候选：{raw:?}"
        );
    }
}

/// 异常：非法显式声明**原样**进候选表（不滤空、不 panic）——形态判定归 `entry_state`，
/// 这里滤掉就分不清「声明非法」与「没有声明」。
#[test]
fn entry_candidates非法声明原样返回() {
    let long = format!("views/{}.tsx", "a".repeat(1100));
    let special = "views/🎯\n模块.tsx";
    for raw in ["/abs/x.rs", "", long.as_str(), special] {
        let mut declared = element("App.X", &["views/"]);
        declared.entry = Some(raw.to_owned());
        assert_eq!(
            entry_candidates(&declared),
            vec![raw.to_owned()],
            "非法声明逐字返回（不做校验）：{raw:?}"
        );
    }
}

/// 边界（AC-1「候选派生零 IO」行为面）：候选文件全不在盘的临时目录下仍返回完整文本
/// 候选，且派生前后盘面零变化（不建目录、不落文件、不探测）。
#[test]
fn entry_candidates零io候选派生不触盘() {
    let temp = TempDir::new("candidates-zero-io");
    let element = element("App.Backend.Core.Archi", &["crates/core/archi"]);
    let before = tree_listing(temp.root());

    let candidates = entry_candidates(&element);
    let again = entry_candidates(&element);

    assert_eq!(candidates, again, "两次派生逐字相等（纯文本）");
    assert_eq!(candidates.len(), 4, "候选表恒四条，与盘面无关");
    for candidate in &candidates {
        assert!(
            !temp.root().join(candidate).exists(),
            "候选文件全不在盘：{candidate}"
        );
    }
    assert_eq!(
        tree_listing(temp.root()),
        before,
        "派生前后盘面零变化（零 IO）"
    );
}

/// 边界（AC-1「候选派生零 IO」源码自证）：`paths.rs` 的全部 `is_file(` 代码命中都落在
/// `entry_state` 体内；`entry_candidates` 的函数体内零 `is_file(` / 零 `fs::`。
#[test]
fn 候选派生零io源码自证() {
    let source = include_str!("paths.rs");
    let code: Vec<&str> = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    let position_of = |needle: &str| {
        code.iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("源码缺 {needle:?}"))
    };

    let candidates_at = position_of("pub fn entry_candidates(");
    let state_at = position_of("pub fn entry_state(");
    assert!(candidates_at < state_at, "解析链两个出口的定义序");

    let probes: Vec<usize> = code
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("is_file("))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(probes.len(), 2, "全部 `is_file(` 代码命中：{probes:?}");
    assert!(
        probes.iter().all(|index| *index > state_at),
        "读盘只发生在 `entry_state` 体内（候选派生零 IO）：{probes:?}"
    );

    let body = &code[candidates_at..state_at];
    assert!(
        !body.iter().any(|line| line.contains("is_file(")),
        "`entry_candidates` 函数体内零 `is_file(`"
    );
    assert!(
        !body.iter().any(|line| line.contains("fs::")),
        "`entry_candidates` 函数体内零 `fs::`"
    );
}

/// 正向（AC-1 ② / 形态位派生优先级）：显式 `entry` 落盘且等于元素根 → `SingleFile`；
/// 文件型根 `crates/x/src/lib.rs` 同样先命中「入口 == 根」判据（`src/lib.rs` 收尾判据
/// 不会把它误判成 `Crate`）。
#[test]
fn entry_state显式等于根即单文件形态() {
    let temp = TempDir::new("entry-single-file");
    temp.touch("views/welcome-view.tsx");
    let mut single = element("App.Frontend.Views.Welcome", &["views/welcome-view.tsx"]);
    single.entry = Some("views/welcome-view.tsx".to_owned());
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&single), &single),
        EntryState::Resolved {
            form: EntryForm::SingleFile,
            entry: "views/welcome-view.tsx".to_owned(),
        },
        "显式 entry 命中且等于根 → SingleFile"
    );

    temp.touch("crates/x/src/lib.rs");
    let mut crate_file_root = element("App.Core.X", &["crates/x/src/lib.rs"]);
    crate_file_root.entry = Some("crates/x/src/lib.rs".to_owned());
    assert_eq!(
        entry_state(
            temp.root(),
            std::slice::from_ref(&crate_file_root),
            &crate_file_root
        ),
        EntryState::Resolved {
            form: EntryForm::SingleFile,
            entry: "crates/x/src/lib.rs".to_owned(),
        },
        "等值判据先于 `src/lib.rs` 收尾判据（形态位不反推入口）"
    );
}

/// 正向：多文件根只取 `path.first()`（`<root>` 仍是首条）；显式声明优先于同目录的
/// 约定候选。
#[test]
fn entry_state多文件根与显式优先() {
    let temp = TempDir::new("entry-multi-root");
    temp.touch("app.tsx");
    let multi = element(
        "App.Frontend.App",
        &["app.tsx", "app/main.tsx", "app/index.ts", "app/index.tsx"],
    );
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&multi), &multi),
        EntryState::Resolved {
            form: EntryForm::SingleFile,
            entry: "app.tsx".to_owned(),
        },
        "多文件根取 `path.first()`，且不拼 `index.ts(x)`"
    );

    temp.touch("crates/x/src/lib.rs");
    temp.touch("crates/x/mod.rs");
    let mut declared = element("App.Core.X", &["crates/x/"]);
    declared.entry = Some("crates/x/mod.rs".to_owned());
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&declared), &declared),
        EntryState::Resolved {
            form: EntryForm::SubdirModule,
            entry: "crates/x/mod.rs".to_owned(),
        },
        "显式优先：同目录 `src/lib.rs` 在盘也不改选约定候选"
    );
}

/// 正向（顺序封闭四形态各自独立可取）：只落 `src/lib.rs` / `mod.rs` / `index.ts` /
/// `index.tsx` 之一时，各自独立命中，形态位按入口与根的关系分列。
#[test]
fn entry_state四种约定形态独立命中() {
    let temp = TempDir::new("entry-four-defaults");
    let cases: [(&str, &str, EntryForm); 4] = [
        ("src/a/", "src/a/src/lib.rs", EntryForm::Crate),
        ("src/b/", "src/b/mod.rs", EntryForm::SubdirModule),
        ("src/c/", "src/c/index.ts", EntryForm::SubdirModule),
        ("src/d/", "src/d/index.tsx", EntryForm::SubdirModule),
    ];
    for (root, entry, form) in cases {
        temp.touch(entry);
        let element = element("App.Core.Probe", &[root]);
        assert_eq!(
            entry_state(temp.root(), std::slice::from_ref(&element), &element),
            EntryState::Resolved {
                form,
                entry: entry.to_owned(),
            },
            "只落 {entry} 时独立命中（入口是全串，不是主干 / 形态名）"
        );
    }
}

/// 异常（唯一判据的三条边）：单元素声明同一入口**不**假阳；同一目录型根下
/// `src/lib.rs` 与 `mod.rs` 同时在盘时显式声明后者被判 `Duplicated`（保守近似，只多报
/// 不瞒报）；另一元素的候选未落盘也参与候选交集（判据是候选文本交集，零递归读盘）。
#[test]
fn entry_state唯一判据不假阳与保守近似() {
    let temp = TempDir::new("entry-dup-edges");
    temp.touch("crates/solo/src/lib.rs");
    let mut solo = element("App.Core.Solo", &["crates/solo/"]);
    solo.entry = Some("crates/solo/src/lib.rs".to_owned());
    assert_eq!(
        entry_state(temp.root(), std::slice::from_ref(&solo), &solo),
        EntryState::Resolved {
            form: EntryForm::Crate,
            entry: "crates/solo/src/lib.rs".to_owned(),
        },
        "同一入口只被一个元素声明 → Resolved（唯一判据不假阳）"
    );

    temp.touch("crates/pair/src/lib.rs");
    temp.touch("crates/pair/mod.rs");
    let dir_root = element("App.Core.Pair", &["crates/pair/"]);
    let mut declared = element("App.Core.PairMod", &["crates/pair/"]);
    declared.entry = Some("crates/pair/mod.rs".to_owned());
    let pair = [dir_root.clone(), declared.clone()];
    assert_eq!(
        entry_state(temp.root(), &pair, &declared),
        EntryState::Invalid(EntryProblem::Duplicated {
            entry: "crates/pair/mod.rs".to_owned(),
            other_fqn: "App.Core.Pair".to_owned(),
        }),
        "同一目录型根下两种约定形态同时在盘 ⇒ 声明后者被判冲突（保守近似）"
    );
    assert_eq!(
        entry_state(temp.root(), &pair, &dir_root),
        EntryState::Resolved {
            form: EntryForm::Crate,
            entry: "crates/pair/src/lib.rs".to_owned(),
        },
        "目录型根自身仍按序取 `src/lib.rs`（多报只发生在声明方）"
    );

    // 候选交集判据不递归读对方元素的盘面：对方元素的入口候选是纯文本
    let ghost = element("App.Core.Ghost", &["crates/ghost/"]);
    let mut ghost_declared = element("App.Core.GhostEntry", &["crates/ghost/"]);
    ghost_declared.entry = Some("crates/ghost/src/lib.rs".to_owned());
    temp.touch("crates/ghost/src/lib.rs");
    let ghosts = [ghost.clone(), ghost_declared.clone()];
    for element in [&ghost, &ghost_declared] {
        match entry_state(temp.root(), &ghosts, element) {
            EntryState::Invalid(EntryProblem::Duplicated { entry, other_fqn }) => {
                assert_eq!(entry, "crates/ghost/src/lib.rs");
                let expected = if element.fqn == ghost.fqn {
                    ghost_declared.fqn.as_str()
                } else {
                    ghost.fqn.as_str()
                };
                assert_eq!(other_fqn, expected, "双方各点名冲突方");
            }
            other => panic!("同一入口的两个候选来源应判 Duplicated，实际：{other:?}"),
        }
    }
}

/// 边界：`EntryState` / `EntryProblem` 的派生相等语义——三态互不相等、四变体互不相等、
/// 载荷逐字段参与比较（`entry` 是归一后原文、`other_fqn` 是冲突方）。
#[test]
fn entry_state与problem逐值可对拍() {
    let entry = "crates/infra/archi/src/lib.rs";
    let resolved = EntryState::Resolved {
        form: EntryForm::Crate,
        entry: entry.to_owned(),
    };
    let problems = [
        EntryProblem::Malformed {
            entry: entry.to_owned(),
        },
        EntryProblem::OutsidePath {
            entry: entry.to_owned(),
        },
        EntryProblem::NotOnDisk {
            entry: entry.to_owned(),
        },
        EntryProblem::Duplicated {
            entry: entry.to_owned(),
            other_fqn: "App.Backend.Infra.Other".to_owned(),
        },
    ];

    assert_ne!(resolved, EntryState::Absent, "三态互不相等");
    for problem in &problems {
        let invalid = EntryState::Invalid(problem.clone());
        assert_ne!(resolved, invalid, "Resolved ≠ Invalid（{problem:?}）");
        assert_ne!(
            EntryState::Absent,
            invalid,
            "Absent ≠ Invalid（{problem:?}）"
        );
        assert_eq!(invalid, EntryState::Invalid(problem.clone()), "同值可对拍");
    }
    for (index, left) in problems.iter().enumerate() {
        for right in &problems[index + 1..] {
            assert_ne!(left, right, "四变体互不相等（同载荷也须可分辨）");
        }
    }
    assert_ne!(
        problems[3],
        EntryProblem::Duplicated {
            entry: entry.to_owned(),
            other_fqn: "App.Backend.Infra.Third".to_owned(),
        },
        "`other_fqn` 参与相等语义"
    );
    assert_ne!(
        problems[0],
        EntryProblem::Malformed {
            entry: "crates/infra/archi".to_owned(),
        },
        "`entry` 参与相等语义"
    );
}

// ---------------------------------------------------------------------------
// 落点（入口同级）与 `#[path]` 值退化为文件名
// ---------------------------------------------------------------------------

/// 正向（D6）：crate / 子目录模块 / 文件型根三形态的落点均为 `<入口主干>.i.<后缀>`，
/// 与入口同目录；`<入口主干>` 不是元素根末段。
#[test]
fn generated_path落入口同级三形态() {
    let root = Path::new("/ws");
    for (entry, kind, expected) in [
        (
            "crates/infra/archi/src/lib.rs",
            GeneratedKind::Rust,
            "crates/infra/archi/src/lib.i.rs",
        ),
        (
            "crates/core/archi/src/scan/mod.rs",
            GeneratedKind::Rust,
            "crates/core/archi/src/scan/mod.i.rs",
        ),
        (
            "views/welcome-view.tsx",
            GeneratedKind::TypeScript,
            "views/welcome-view.i.ts",
        ),
        (
            "views/architecture/index.ts",
            GeneratedKind::TypeScript,
            "views/architecture/index.i.ts",
        ),
    ] {
        let derived = generated_path(root, entry, kind).expect("形态合法");
        assert_eq!(
            derived,
            root.join(expected),
            "落点 = <入口主干>.i.<后缀>（入口同级）"
        );
        assert!(
            dot_segments(&derived).is_empty(),
            "落点零 `..` 段（入口同级后不再手算层数）"
        );
    }

    // 入口无目录段：落仓根（主干不丢文件名）
    assert_eq!(
        generated_path(root, "lib.rs", GeneratedKind::Rust),
        Some(root.join("lib.i.rs")),
        "无目录段时目录为空"
    );
}

/// 边界（AC-3）：`#[path]` 的值是**字面量**，等值真值源退化为 `generated_path` 的
/// 文件名——对已编排入口 `crates/infra/archi/src/lib.rs`，入口行字面量逐字等于
/// `generated_path(entry).file_name()`，且恒无 `../`；`mod_path_attr` 已删，产品 `.rs`
/// 内零命中。
#[test]
fn 入口行字面量等于生成物文件名() {
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

    let expected = generated_path(
        Path::new("/ws"),
        "crates/infra/archi/src/lib.rs",
        GeneratedKind::Rust,
    )
    .and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    })
    .expect("生成物路径可派生");
    assert_eq!(attribute, expected, "字面量 == generated_path 的文件名");
    assert!(
        !attribute.contains('/') && !attribute.contains(".."),
        "值恒为文件名（零 `../`、零目录段）：{attribute}"
    );

    // 旧名零残留：`mod_path_attr(` 在产品 `.rs` 内零命中（函数已删）
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&src_tauri.join("crates"), &mut hits);
    scan_product_rs(&src_tauri.join("src"), &mut hits);
    let calls: Vec<&(String, String)> = hits
        .iter()
        .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains("mod_path_attr("))
        .collect();
    assert!(
        calls.is_empty(),
        "`mod_path_attr` 已删（入口同级后属性值恒为文件名）：{calls:?}"
    );
}

/// 边界（AC-3 机械面）：入口行上方注释与真实落点一致——零「与 `mod_path_attr` 输出
/// 等值」与「`mod_path_attr` 零产品调用者」两句旧表述，且点名「入口同级」
/// 「（值）恒为文件名」两条新纪律（注释改口是落点迁移的记账面，不能停在旧句上）。
#[test]
fn 入口行注释与入口同级落点一致() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/core/archi → crates/core → crates → src-tauri
    let mut src_tauri = manifest.clone();
    for _ in 0..3 {
        src_tauri.pop();
    }
    let lib_rs = src_tauri.join("crates/infra/archi/src/lib.rs");
    let source = fs::read_to_string(&lib_rs).expect("入口编排文件在案");

    let lines: Vec<&str> = source.lines().collect();
    let attribute_at = lines
        .iter()
        .position(|line| line.trim_start().starts_with("#[path = \""))
        .expect("入口行含 `#[path = \"…\"]` 字面量");
    let mut comment: Vec<&str> = lines[..attribute_at]
        .iter()
        .rev()
        .take_while(|line| line.trim_start().starts_with("//"))
        .map(|line| line.trim_start().trim_start_matches("//").trim())
        .collect();
    comment.reverse();
    let text = comment.join("\n");
    assert!(!text.is_empty(), "入口行上方有紧邻注释块");

    for stale in ["输出等值", "零产品调用者", "mod_path_attr"] {
        assert!(
            !text.contains(stale),
            "入口行注释零旧句 `{stale}`（等值真值源已退化为 `generated_path`）：
{text}"
        );
    }
    for current in ["入口同级", "恒为文件名"] {
        assert!(
            text.contains(current),
            "入口行注释点到 `{current}`：
{text}"
        );
    }
    assert!(
        !source.contains("mod_path_attr"),
        "整文件零 `mod_path_attr` 词形（函数已删、注释同批改述）"
    );
}

/// 正向：TS import specifier 三形态逐值——目录形态 `../<name>.i`（`Crate` / `SubdirModule`
/// 同值）、单文件形态 `./<name>.i`（零 `../`）；doc 明确标注本期未接线，当前输出 MUST NOT
/// 被当作既有真值（R-7）。
#[test]
fn ts_import_specifier三形态() {
    assert_eq!(ts_import_specifier(EntryForm::Crate, "archi"), "../archi.i");
    assert_eq!(
        ts_import_specifier(EntryForm::SubdirModule, "archi"),
        "../archi.i"
    );
    assert_eq!(ts_import_specifier(EntryForm::SingleFile, "app"), "./app.i");

    // 形态位取自 `entry_state` 的 `Resolved`（文件型根 → `SingleFile` ⇒ 同目录 specifier）
    assert_eq!(
        ts_import_specifier(EntryForm::SingleFile, "welcome-view.tsx"),
        "./welcome-view.tsx.i",
        "文件型根的 specifier 是同目录形态（旧 `../` 断言已按新形态位改写）"
    );

    let source = include_str!("paths.rs");
    assert!(
        source.contains("待接线留白"),
        "doc 标注「本期未接线」（当前输出 MUST NOT 当既有真值）"
    );
    assert!(
        source.contains("入口同级规则"),
        "doc 点名入口同级规则归前端契约变更"
    );
}

// ---------------------------------------------------------------------------
// ref_relative / dir_segments / module_name
// ---------------------------------------------------------------------------

/// 正向（AC-11）：同目录 → `./`；同层跨子树 → `../<段>/`；向上回根层 → 逐层 `../`；
/// 向下进子层 → `./<段>/`。前缀恒以 `/` 收尾，层数由段链求出（零手算）。
#[test]
fn ref_relative同目录同层与跨层() {
    assert_eq!(
        ref_relative("App.Backend.Core.Archi", "App.Backend.Core.Foundation"),
        "./",
        "同目录用 `./`"
    );
    assert_eq!(
        ref_relative("App.Backend.Infra.ArchiRuntime", "App.Backend.Core.Archi"),
        "../Core/",
        "同层跨子树用 `../<段>/`"
    );
    assert_eq!(
        ref_relative("App.Backend.Core.Archi", "App"),
        "../../../",
        "目标为单段 FQN（零目录段）→ 从三层逐层上溯到契约目录根"
    );
    assert_eq!(
        ref_relative(
            "App.Frontend.Components.Agent",
            "App.Frontend.Components.Agent.AgentInput"
        ),
        "./Agent/",
        "目标在子层 → 向下进一段"
    );
    assert_eq!(
        ref_relative(
            "App.Frontend.Components.Agent.AgentInput",
            "App.Backend.Core.Archi"
        ),
        "../../../Backend/Core/",
        "跨层：先上溯到公共祖先再逐段下行"
    );
}

/// 边界（R-8）：上下游末段撞名不影响前缀（元素身份是 FQN，撞名只落在不同层次）；
/// `from == to` → `./`。
#[test]
fn ref_relative撞名段与自引用() {
    let left = ref_relative("App.Backend.Core.Agent", "App.Frontend.Components.Agent");
    let right = ref_relative("App.Frontend.Components.Agent", "App.Backend.Core.Agent");
    assert_eq!(left, "../../Frontend/Components/", "目标段链完整给出");
    assert_eq!(right, "../../Backend/Core/", "反向目标段链完整给出");
    assert_ne!(left, right, "撞名只影响布局，前缀仍按目标段链算");
    assert_eq!(
        ref_relative("App.Backend.Core.Archi", "App.Backend.Core.Archi"),
        "./",
        "自引用视为同目录"
    );
}

/// 边界：FQN 段链去掉叶段（根段保留）；单段 → 零段（契约落目录根）；空串 → `None`；
/// 尾点形态可判（空段由 `is_fqn` 挡在派生之外）。
#[test]
fn dir_segments形态() {
    assert_eq!(
        dir_segments("App.Backend.Infra.ArchiRuntime"),
        Some(vec![
            "App".to_owned(),
            "Backend".to_owned(),
            "Infra".to_owned()
        ])
    );
    assert_eq!(
        dir_segments("App.Agent"),
        Some(vec!["App".to_owned()]),
        "两段 FQN 的目录段 = 根段"
    );
    assert_eq!(dir_segments("App"), Some(Vec::new()), "单段 FQN 零段");
    assert_eq!(dir_segments(""), None, "空串无段链");
    // 尾点形态：`App.` 的叶段为空串（可判、不 panic）；`App..X` 的中间段为空串
    assert_eq!(dir_segments("App."), Some(vec!["App".to_owned()]));
    assert_eq!(
        dir_segments("App..X"),
        Some(vec!["App".to_owned(), String::new()])
    );
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
        dir_segments("App.Backend.Infra.ArchiRuntime")
            .and_then(|segments| segments.last().cloned()),
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

/// 边界：索引路径恒为 `<archi_graph_dir>/INDEX.toml`（全局唯一一份，落独立于契约
/// 目录的索引家，不在分层目录内、也不在契约目录根）。
#[test]
fn index_path位于索引目录根() {
    let root = Path::new("/ws");
    assert_eq!(index_path(root), archi_graph_dir(root).join("INDEX.toml"));
    assert_eq!(
        index_path(root).parent(),
        Some(archi_graph_dir(root).as_path()),
        "索引不进分层子目录"
    );
    assert_ne!(
        index_path(root).parent(),
        Some(directory(root).as_path()),
        "索引与契约目录分家（兄弟目录）"
    );
}

// ---------------------------------------------------------------------------
// directory / index_path：契约目录的单点派生
// ---------------------------------------------------------------------------

/// 正向（AC-4 / AC-5 前置）：`directory(root)` 与 `foundation::layout::archi_models_dir(root)`
/// **逐字相等**（同一值，不是相等前缀；契约目录不另设函数）；`root` 含空格 / 非 ASCII /
/// 尾分隔符三形态均成立。
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
            archi_models_dir(root),
            "directory 是 archi_models_dir 的唯一转发点（同一值）：{root:?}"
        );
        assert_eq!(
            directory(root).file_name().map(|name| name.to_owned()),
            archi_models_dir(Path::new("/tmp"))
                .file_name()
                .map(|name| name.to_owned()),
            "末段沿用 layout 的段名（零行内拼段）：{root:?}"
        );
    }
}

/// 边界（AC-5）：契约目录一切派生都经 `directory` 单点、索引经独立派生点——来源文件内
/// 零 `"api"` / `"models"` / `"graph"` 段字面量（段名只在 `foundation::layout` 定义），
/// 且 `contract_path` / `schema_path` 的父目录可由 `directory(root)` 推出、`index_path`
/// 的父目录**不能**（索引与契约目录分家）。
#[test]
fn directory零行内段名字面量() {
    let source = include_str!("paths.rs");
    for banned in ["\"api\"", "\"models\"", "\"graph\""] {
        assert!(
            !source.contains(banned),
            "paths.rs 内零 `{banned}` 字面量（MUST NOT 行内拼段）"
        );
    }
    assert!(
        source.contains("archi_models_dir"),
        "契约目录派生经 foundation::layout::archi_models_dir"
    );
    assert!(
        source.contains("archi_graph_dir"),
        "索引派生经 foundation::layout::archi_graph_dir"
    );
    assert!(source.contains("fn directory"), "单点转发函数在案");

    let root = Path::new("/ws");
    assert_eq!(
        index_path(root).parent(),
        Some(archi_graph_dir(root).as_path()),
        "索引的父目录是 graph 目录（非 directory(root)）"
    );
    assert_eq!(
        contract_path(root, "App.Core.Alpha")
            .expect("合法 FQN")
            .parent(),
        Some(directory(root).join("App").join("Core").as_path()),
        "契约在 directory(root)/<分层目录…>/ 下一层"
    );
    assert_eq!(
        schema_path(root, "App.Core.Alpha")
            .expect("合法 FQN")
            .parent(),
        Some(directory(root).join("App").join("Core").as_path()),
        "schema 与契约同层"
    );
}

/// 正向（AC-5）：`index_path(root)` 末段恰为 `INDEX.toml`、父目录恰为 `graph/`——与
/// **契约目录分家**（兄弟目录，互不嵌套），与 `contract_path` / `schema_path`
/// （分层目录内）**不同层**。
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
        Some(archi_graph_dir(root).as_path()),
        "父目录恰为 graph 目录"
    );
    assert!(
        !index.starts_with(directory(root)),
        "索引不在契约目录之下（两目录分家）"
    );

    let contract = contract_path(root, "App.Backend.Infra.ArchiRuntime").expect("合法 FQN");
    assert_ne!(
        index.parent(),
        contract.parent(),
        "索引在 graph 根、契约在分层目录（不同层）"
    );
    assert!(
        contract
            .parent()
            .is_some_and(|parent| parent.starts_with(directory(root))),
        "契约目录在 directory(root) 之下"
    );
    let contracts = directory(root);
    assert_eq!(
        index.parent().and_then(|parent| parent.parent()),
        contracts.parent(),
        "索引家与契约目录是兄弟（同一架构域根下的平行子目录）"
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
        archi_graph_dir(missing).join("INDEX.toml"),
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

    // 形态校验只经三处：`contract_path` / `schema_path` 各调 `is_fqn` 一次（共 2）；
    // `checked_root` 定义 1 + 三个消费面（`generated_path` / `entry_candidates` / `form_of`）
    assert_eq!(
        source.matches("!is_fqn(").count(),
        2,
        "`contract_path` / `schema_path` 的形态校验各经 `is_fqn` 一次"
    );
    assert_eq!(
        source.matches("checked_root(").count(),
        4,
        "`checked_root` 定义 1 + 三个消费面（`generated_path` / `entry_candidates` / `form_of`）"
    );
    // `is_repo_relative` 只被 `checked_root` / `doc_path` 与入口判据消费：定义 1 +
    // `checked_root` 1 + `doc_path` 1 + 基准 / 落位两条判据各 1
    assert_eq!(
        source.matches("is_repo_relative(").count(),
        4,
        "`is_repo_relative` 无第二份实现（定义 1 + 三处判据消费：`checked_root` / 入口基准 / 落位）"
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
        mirrored_schema_text(
            fqn,
            &format!(
                "{}, {}, {}, {}",
                reexport_def(fqn, "AgentView", dotted_target),
                reexport_def(fqn, "Empty", empty_target),
                reexport_def(fqn, "Base", valid_target),
                r#""Thing": {"type": "object", "properties": {}}"#
            ),
            "",
        )
    };

    // A：Rust 消费方（crate 形态入口 `<dir>/src/lib.rs`）
    temp.write_contract(rust_consumer, &reexports(rust_consumer));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    // D：TS 消费方（元素根本身带 `.tsx` ⇒ 文件型根，入口 = 根文件自身）
    temp.write_contract(ts_consumer, &reexports(ts_consumer));
    temp.write("views/architecture.tsx", "export const architecture = 1;\n");

    // B：被透传的 TS 元素（同一个带扩展名的元素根，也有自己的入口 → 自带 `.i.ts`）
    temp.write_contract(
        dotted_target,
        &mirrored_schema_text(
            dotted_target,
            r#""Item": {"type": "object", "properties": {}}"#,
            "",
        ),
    );
    temp.write("views/welcome-view.tsx", "export interface Item {}\n");

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
    let rust_path = generated_path(
        temp.root(),
        "crates/infra/archi/src/lib.rs",
        GeneratedKind::Rust,
    )
    .expect("Rust 路径");
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
    // D 是文件型根 ⇒ 形态位 `SingleFile`（同目录 specifier）
    let expected = ts_import_specifier(EntryForm::SingleFile, "welcome-view.tsx");
    assert_eq!(expected, "./welcome-view.tsx.i");
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

    // 语料落盘形态已改写：文件型根的入口是**根文件本身**，故被透传的 B 自己也产
    // `.i.ts`（旧 `views/welcome-view.tsx/index.ts` 目录形态在新链下是 `NotOnDisk`）
    let dotted_generated = generated_path(temp.root(), dotted_root, GeneratedKind::TypeScript)
        .expect("文件型根的入口形态合法");
    assert_eq!(
        dotted_generated,
        temp.root().join("views/welcome-view.i.ts"),
        "B 的落点与入口同级（根文件的兄弟文件）"
    );
    assert!(
        files.iter().any(|file| file.path == dotted_generated),
        "被透传的单文件根元素自身产 `.i.ts`：{:?}",
        files.iter().map(|file| &file.path).collect::<Vec<_>>()
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
    let file = ".dev-team/architecture/models/Backend/Infra/App.Backend.Infra.ArchiRuntime.md";
    let source = mirrored_schema_text(fqn, &reexport_def(fqn, "Archived", fqn), "");
    let (contract, schema): (crate::api::contract::Contract, serde_json::Value) =
        crate::api::mod_test::prepare(&source, fqn, file);

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

// ---------------------------------------------------------------------------
// 段名纪律的删除面与索引文件名常量（`module-entry-docs-semantics` / AC-1 / AC-5）
// ---------------------------------------------------------------------------

/// 异常（AC-5）：`paths.rs` 内 `api` 段族三符号零命中；`archi_models_dir` /
/// `archi_graph_dir` 同批 import 且各在 `directory` / `index_path` 体内消费一次（段名
/// 只经派生函数，零行内拼接）。
#[test]
fn paths零api段族且两族派生单点消费() {
    let source = include_str!("paths.rs");
    for removed in ["archi_api_dir", "archi_api_rel_dir", "API_DIR_NAME"] {
        assert_eq!(
            source.matches(removed).count(),
            0,
            "`paths.rs` 内 `{removed}` 零命中（段名只经 foundation::layout 派生）"
        );
    }

    let import_line = source
        .lines()
        .find(|line| line.trim_start().starts_with("use foundation::layout"))
        .unwrap_or_else(|| panic!("缺 `foundation::layout` 的 import 行"));
    for needle in ["archi_models_dir", "archi_graph_dir"] {
        assert!(
            import_line.contains(needle),
            "`{needle}` 在 import 行（段名单点导入）：{import_line}"
        );
    }

    let directory_body = function_body(source, "pub fn directory(root: &Path) -> PathBuf {");
    assert_eq!(
        directory_body.matches("archi_models_dir(root)").count(),
        1,
        "`directory` 体恰一次 `archi_models_dir(root)`：{directory_body}"
    );
    assert!(
        !directory_body.contains("archi_graph_dir"),
        "`directory` 体零 `archi_graph_dir`（索引与契约分家）：{directory_body}"
    );

    let index_body = function_body(source, "pub fn index_path(root: &Path) -> PathBuf {");
    assert_eq!(
        index_body.matches("archi_graph_dir(root)").count(),
        1,
        "`index_path` 体恰一次 `archi_graph_dir(root)`：{index_body}"
    );
    assert!(
        !index_body.contains("directory(root)"),
        "`index_path` MUST NOT 由 `directory(root).join(…)` 派生：{index_body}"
    );
}

/// 边界：空 root / 盘符风格 root / 尾分隔符 / 超长 root（1200 字符）——`directory` 恒与
/// `archi_models_dir` 同值、末段恒 `models`，不 panic。
#[test]
fn directory边界root形态仍与layout同值() {
    let long = format!("C:\\{}", "a".repeat(1200));
    for (label, root) in [
        ("空 root", Path::new("")),
        ("盘符 root", Path::new(r"C:\demo\alpha")),
        ("尾分隔符 root", Path::new("/ws/")),
        ("超长 root", Path::new(long.as_str())),
    ] {
        assert_eq!(
            directory(root),
            archi_models_dir(root),
            "{label}：`directory` 与 `archi_models_dir` 是同一值"
        );
        assert_eq!(
            directory(root).file_name().and_then(|name| name.to_str()),
            Some("models"),
            "{label}：末段恒 models"
        );
    }
}

/// 正向（AC-1）：`INDEX_FILE_NAME` 值恰 `"INDEX.toml"`、`paths.rs` 内该字面量恰 1 处
/// （常量定义，`pub(crate)` 可见）；`diff.rs` 内零字面量、以
/// `super::paths::INDEX_FILE_NAME` 恰消费一次（跨模块引用在编译期即证明可见性可达）。
#[test]
fn index_file_name单点持有与跨模块消费() {
    let source = include_str!("paths.rs");
    assert_eq!(
        source.matches("\"INDEX.toml\"").count(),
        1,
        "`\"INDEX.toml\"` 字面量恰一处（常量定义）"
    );
    assert!(
        source.contains("pub(crate) const INDEX_FILE_NAME: &str = \"INDEX.toml\";"),
        "常量定义与 `pub(crate)` 可见性在案"
    );
    assert_eq!(
        source.matches("const INDEX_FILE_NAME").count(),
        1,
        "索引文件名常量恰一处定义"
    );
    assert!(
        !source.contains("INDEX_SUFFIX"),
        "零同值第二常量（`INDEX_SUFFIX` 一类）"
    );
    assert_eq!(
        index_path(Path::new("/ws"))
            .file_name()
            .and_then(|name| name.to_str()),
        Some("INDEX.toml"),
        "常量值即索引路径末段"
    );

    let diff = include_str!("diff.rs");
    assert_eq!(
        diff.matches("\"INDEX.toml\"").count(),
        0,
        "`diff.rs` 内零索引文件名字面量（唯一持有者在 paths）"
    );
    assert_eq!(
        diff.matches("super::paths::INDEX_FILE_NAME").count(),
        1,
        "`diff.rs` 恰一处引用本常量（收面后缀共用同一持有者）"
    );
}

/// 边界（AC-1）：`schema_path` 末段恒 `<FQN>.schema.json`、单段 FQN 落契约目录根、root
/// 三形态父链与 `contract_path` 同源。
#[test]
fn schema_path边界形态与contract同源() {
    for root in [Path::new("/ws"), Path::new(""), Path::new("/ws/")] {
        let path = schema_path(root, "App")
            .unwrap_or_else(|| panic!("合法 FQN 应可派生：root = {root:?}"));
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("App.schema.json"),
            "末段恒 `<FQN>.schema.json`：root = {root:?}"
        );
        assert_eq!(
            path.parent(),
            Some(directory(root).as_path()),
            "单段 FQN 落契约目录根：root = {root:?}"
        );

        let contract = contract_path(root, "App.Backend.Infra.ArchiRuntime").expect("合法 FQN");
        let schema = schema_path(root, "App.Backend.Infra.ArchiRuntime").expect("合法 FQN");
        assert_eq!(
            schema.parent(),
            contract.parent(),
            "两函数父目录逐字相等：root = {root:?}"
        );
    }
}

/// 边界：超长合法 FQN（1200 字符、零分隔符）与含换行的合法 FQN 均 `Some` 且不 panic
/// （判据只看形态，不清洗内容）。
#[test]
fn contract_path超长与特殊字符fqn放行() {
    let root = Path::new("/ws");
    let long = format!("App.{}", "a".repeat(1200));
    let path = contract_path(root, &long).unwrap_or_else(|| panic!("超长合法 FQN 应可派生"));
    assert_eq!(
        path.parent(),
        Some(directory(root).join("App").as_path()),
        "父目录 = `models/App`（`App.<末段>` 去掉叶段后只剩根段）"
    );
    assert!(schema_path(root, &long).is_some(), "schema 路径同步放行");

    for fqn in ["App.🎯.Target", "App.Core.Line\nBreak"] {
        let contract =
            contract_path(root, fqn).unwrap_or_else(|| panic!("形态合法即放行：{fqn:?}"));
        let schema = schema_path(root, fqn).unwrap_or_else(|| panic!("形态合法即放行：{fqn:?}"));
        assert_eq!(
            contract.parent(),
            schema.parent(),
            "两函数父目录同源：{fqn:?}"
        );
        assert!(
            dot_segments(&contract).is_empty() && dot_segments(&schema).is_empty(),
            "零目录穿越段：{fqn:?}"
        );
    }
}

/// 取 `signature` 起的函数体原文（到首个行首 `}` 为止）。
fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("缺少函数签名 `{signature}`"));
    let rest = &source[start..];
    let end = rest.find("\n}\n").map_or(rest.len(), |index| index + 1);
    &rest[..end]
}

// ---------------------------------------------------------------------------
// schema_fqn：`<FQN>.schema.json` 去后缀的唯一反查
// ---------------------------------------------------------------------------

/// 正向：`App.Auth.schema.json` ⇒ `Some("App.Auth")`；四段 FQN 同式；与
/// `schema_path` 的落盘名往返一致（同一后缀常量）。
#[test]
fn 文件名反查唯一() {
    assert_eq!(schema_fqn("App.Auth.schema.json"), Some("App.Auth"));
    assert_eq!(
        schema_fqn("App.Backend.Infra.ArchiRuntime.schema.json"),
        Some("App.Backend.Infra.ArchiRuntime")
    );
    for fqn in ["App.Auth", "App.Backend.Infra.ArchiRuntime"] {
        let path = schema_path(Path::new(""), fqn).expect("合法 FQN");
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("文件名");
        assert_eq!(schema_fqn(name), Some(fqn), "落盘名往返一致：{name}");
    }
    assert_eq!(
        schema_fqn("App.schema.json"),
        Some("App"),
        "单段 FQN ⇒ Some"
    );
}

/// 异常：`App.Auth.md` / `schema.json`（零主干）/ `X.schema.JSON`（大写）/ 空串 ⇒ 全 `None`。
#[test]
fn 非schema名零命中() {
    for name in ["App.Auth.md", "schema.json", "X.schema.JSON", ""] {
        assert_eq!(schema_fqn(name), None, "非接口文件名零命中：{name:?}");
    }
}

/// 边界：超长名 / 含 emoji / 多重后缀 `.schema.json.json` ⇒ 按既有后缀规则裁定、`None` 或
/// 去一层后缀（不 panic）；单段 FQN ⇒ `Some`。
#[test]
fn 边界不panic() {
    let long = format!("{}.schema.json", "a".repeat(1200));
    assert!(
        schema_fqn(&long).is_some_and(|stem| stem.len() == 1200),
        "超长名仍按后缀切分"
    );
    assert_eq!(
        schema_fqn("App.🎯.schema.json"),
        Some("App.🎯"),
        "emoji 主干原样保留"
    );
    assert_eq!(
        schema_fqn("App.schema.json.json"),
        None,
        "后缀不在串尾 ⇒ None（后缀常量唯一，零模糊匹配）"
    );
}

// ---------------------------------------------------------------------------
// 形态判据的可见性：`is_fqn` / `checked_root`（写闸与 link 共用同一判据）
// ---------------------------------------------------------------------------

/// 边界：`is_fqn` 已提为 `pub`（`link` 的 FQN 形态判据复用）；合法四段 / 单段 / 空串 /
/// 带中划线四形态逐值。
#[test]
fn link复用同一判据() {
    for valid in ["App", "App.Auth", "App.Backend.Infra.ArchiRuntime"] {
        assert!(is_fqn(valid), "合法 FQN：{valid}");
    }
    for invalid in ["", "App..Auth", "App.", "App/Auth", "C:App"] {
        assert!(!is_fqn(invalid), "非法 FQN：{invalid:?}");
    }
}

/// 边界：`checked_root` 已提为 `pub(crate)`（写面 `path_on_disk` 复用「归一 + 越根」判据）
/// ——`""` / `"."` / `"./"` / `"a/./b"` / `"../x"` / 绝对路径六形态与 `entry_state` 同判据；
/// 两次调用幂等。
#[test]
fn 写面复用同一归一() {
    assert_eq!(checked_root(""), None, "空串越根");
    assert_eq!(checked_root("."), None, "单个点段越根");
    assert_eq!(checked_root("./"), None, "点斜杠归一后为空");
    assert_eq!(checked_root("a/./b"), None, "含点段");
    assert_eq!(checked_root("../x"), None, "越根");
    assert_eq!(checked_root("/etc/passwd"), None, "绝对路径");
    assert_eq!(
        checked_root("./src/alpha/"),
        Some("src/alpha".to_owned()),
        "合法形态归一去前导点斜杠与尾斜杠"
    );
    assert_eq!(
        checked_root("./src/alpha/"),
        checked_root("./src/alpha/"),
        "两次调用幂等"
    );
}

/// 异常（资产① 落点单份）：`paths.rs` 内 `carrier_path` / `doc_path` 零命中——文档落点
/// 恒为派生 `<FQN>.md`，不存在第二份载体派生。
#[test]
fn 载体派生单份() {
    let source = include_str!("paths.rs");
    for retired in [
        "pub fn carrier_path",
        "pub fn doc_path",
        "carrier_path(",
        "doc_path(",
    ] {
        assert_eq!(
            source.matches(retired).count(),
            0,
            "该符号已随资产① 落点收面退役：{retired}"
        );
    }
}
