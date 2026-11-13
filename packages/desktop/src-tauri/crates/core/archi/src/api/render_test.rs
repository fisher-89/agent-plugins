//! `api::render`（应生成物渲染单点）的单元测试：四类生成物齐备（schema / `.i.rs` /
//! `.i.ts` / `INDEX.toml`）、输出序确定与幂等、结构诊断 fail-closed（`ApiError::Element`
//! 且零部分产出）、坏模型零写盘、空模型 / 零契约的冷启动应生成集、`root` 形态边界、
//! 超大语料规模与零 spawn 自证。
//!
//! 零 mock：自建 `TempDir` RAII + `AtomicU64`（本 crate 无 dev-dependency）、真实
//! `elements_of` → `load_contracts` → `render_all` 组合——渲染序与 fail-closed 即被测
//! 语义，替身会引入第二事实源。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::api::mod_test::declare_ancestors;
use crate::api::paths;

use super::render::workspace_render;
use super::ApiError;

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-api-render-test-{}-{}-{}",
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
        self.write_absolute(&self.0.join(relative), content);
    }

    fn write_absolute(&self, path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(path, content).expect("写文件失败");
    }

    /// 落模型（`.dev-team/architecture/models/01-test.toml`，祖先链自动补齐）。
    ///
    /// `elements_of` 经 `dto::graph_view` 取数：缺祖先即 `orphanHierarchy` 结构错，
    /// 而本文件多数夹具只声明叶元素，故统一在落盘前补齐（与 `paths_test` 同一装置）。
    fn write_model(&self, content: &str) {
        self.write(
            ".dev-team/architecture/models/01-test.toml",
            &declare_ancestors(content),
        );
    }

    fn write_contract(&self, fqn: &str, content: &str) {
        let path = paths::contract_path(self.root(), fqn).expect("测试 FQN 恒为合法形态");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建契约目录失败");
        }
        fs::write(&path, content).expect("写契约失败");
    }

    /// 全量字节 + mtime 快照（零写盘对拍）。
    fn snapshot(&self) -> Vec<(String, Vec<u8>, Option<SystemTime>)> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>, Option<SystemTime>)>) {
            let Ok(read) = fs::read_dir(dir) else {
                return;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, out);
                    continue;
                }
                let relative = path
                    .strip_prefix(base)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/");
                let mtime = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
                out.push((relative, fs::read(&path).expect("读文件失败"), mtime));
            }
        }
        let mut entries = Vec::new();
        walk(&self.0, &self.0, &mut entries);
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// 语料夹具
// ---------------------------------------------------------------------------

/// 只含 `api-types` 一 fence 的最小合法契约（H1 = `fqn`）。
fn typed_contract(fqn: &str) -> String {
    format!(
        "# {fqn}\n\n## 职能\n\n渲染单点语料。\n\n## 类型\n\n```json api-types\n\
         {{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    )
}

/// 双元素语料：crate 形态（`src/lib.rs` 入口）+ 前端目录形态（`index.ts` 入口）。
///
/// 返回 `(core, frontend)` 两个 FQN；模型与契约（及两处入口源文）已落盘。
fn write_two_language_corpus(temp: &TempDir) -> (String, String) {
    let core = "App.Core.Archi";
    let frontend = "App.Frontend.Welcome";
    temp.write_model(&format!(
        "[element.\"{core}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"./crates/core/archi/\"]\n\n\
         [element.\"{frontend}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"views/welcome-view.tsx\"]\n"
    ));
    temp.write_contract(core, &typed_contract(core));
    temp.write_contract(frontend, &typed_contract(frontend));
    temp.write(
        "crates/core/archi/src/lib.rs",
        "pub fn stem() -> String { String::new() }\n",
    );
    temp.write(
        "views/welcome-view.tsx/index.ts",
        "export function view(): string { return ''; }\n",
    );
    (core.to_owned(), frontend.to_owned())
}

// ---------------------------------------------------------------------------
// 正向：四类生成物 / 输出序 / 幂等
// ---------------------------------------------------------------------------

/// 正向（AC-4 / D1）：目录型元素根 + 前端元素根 + 在案契约 → 四个生成物落点齐备，
/// 每份落 `root` 之下、内容非空。
#[test]
fn workspace_render四类生成物齐备() {
    let temp = TempDir::new("render-four-kinds");
    let (core, frontend) = write_two_language_corpus(&temp);

    let files = workspace_render(temp.root()).expect("渲染应成功");
    let paths_of: Vec<&Path> = files.iter().map(|file| file.path.as_path()).collect();

    let schema_core = paths::schema_path(temp.root(), &core).expect("合法 FQN");
    let schema_frontend = paths::schema_path(temp.root(), &frontend).expect("合法 FQN");
    let rust_generated = paths::generated_path(
        temp.root(),
        "crates/core/archi/",
        paths::GeneratedKind::Rust,
    )
    .expect("Rust 生成物路径");
    let ts_generated = paths::generated_path(
        temp.root(),
        "views/welcome-view.tsx",
        paths::GeneratedKind::TypeScript,
    )
    .expect("TS 生成物路径");
    let index = paths::index_path(temp.root());

    for expected in [
        &schema_core,
        &schema_frontend,
        &rust_generated,
        &ts_generated,
        &index,
    ] {
        assert!(
            paths_of.contains(&expected.as_path()),
            "生成物落点齐备：{expected:?}（实际：{paths_of:?}）"
        );
    }
    assert_eq!(files.len(), 5, "两份 schema + 两份 .i.* + 一份索引");

    for file in &files {
        assert!(
            file.path.starts_with(temp.root()),
            "每份生成物落 root 之下：{:?}",
            file.path
        );
        assert!(!file.content.is_empty(), "内容非空：{:?}", file.path);
    }
}

/// 正向（AC-4 / D4 前提）：输出序 = 契约 FQN 序 → 各元素 `.i.*` → `INDEX.toml` 末位；
/// 两次调用逐字段相等（幂等，无 HashMap 序泄漏）。
#[test]
fn workspace_render输出序确定且幂等() {
    let temp = TempDir::new("render-order");
    let (core, frontend) = write_two_language_corpus(&temp);

    let first = workspace_render(temp.root()).expect("渲染应成功");
    let second = workspace_render(temp.root()).expect("渲染应成功");
    assert_eq!(first, second, "同语料两次渲染逐字段相等（幂等）");

    let ordered: Vec<PathBuf> = first.iter().map(|file| file.path.clone()).collect();
    assert_eq!(
        ordered,
        vec![
            paths::schema_path(temp.root(), &core).expect("合法 FQN"),
            paths::schema_path(temp.root(), &frontend).expect("合法 FQN"),
            paths::generated_path(
                temp.root(),
                "crates/core/archi/",
                paths::GeneratedKind::Rust
            )
            .expect("Rust 生成物路径"),
            paths::generated_path(
                temp.root(),
                "views/welcome-view.tsx",
                paths::GeneratedKind::TypeScript
            )
            .expect("TS 生成物路径"),
            paths::index_path(temp.root()),
        ],
        "序为契约 FQN 序（App.Core.Archi < App.Frontend.Welcome）→ 元素序 .i.* → 索引末位"
    );
}

// ---------------------------------------------------------------------------
// 异常：结构诊断 fail-closed / 零写盘
// ---------------------------------------------------------------------------

/// 三类结构错模型（环 / 孤儿层级 / 未知引用）。
fn broken_models() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "cycle",
            "[element.\"App\"]\nkind = \"package\"\n\n\
             [element.\"App.A\"]\nkind = \"module\"\n\n\
             [element.\"App.B\"]\nkind = \"module\"\n\n\
             [[relation]]\nfrom = \"App.A\"\nto = \"App.B\"\n\n\
             [[relation]]\nfrom = \"App.B\"\nto = \"App.A\"\n",
        ),
        (
            "orphanHierarchy",
            "[element.\"App.Auth.Token\"]\nkind = \"component\"\n",
        ),
        (
            "unknownReference",
            "[element.\"App\"]\nkind = \"package\"\n\n\
             [element.\"App.Auth\"]\nkind = \"domain\"\n\n\
             [[relation]]\nfrom = \"App.Auth\"\nto = \"App.Shared\"\n",
        ),
    ]
}

/// 异常（AC-4 / D1 fail-closed）：模型含①环②孤儿层级③未知引用任一 → `Err(ApiError::Element)`，
/// 文案含「模型结构错」与诊断种类；MUST NOT 返回部分生成物集。
#[test]
fn workspace_render结构诊断非空即err() {
    for (kind, model) in broken_models() {
        let temp = TempDir::new(&format!("render-broken-{kind}"));
        // 绕过 `declare_ancestors` 直接落盘坏模型（补齐会让孤儿层级变成合法树）
        temp.write(".dev-team/architecture/models/01-test.toml", model);
        temp.write_contract("App.Core.Archi", &typed_contract("App.Core.Archi"));

        let error = workspace_render(temp.root()).expect_err("结构错 ⇒ 整体 Err");
        assert!(
            matches!(error, ApiError::Element(_)),
            "{kind}：错误面为 Element，实际：{error:?}"
        );
        assert!(
            error.message().contains("模型结构错"),
            "{kind}：文案点名结构诊断：{}",
            error.message()
        );
        assert!(
            error.message().contains(kind),
            "{kind}：文案含诊断种类（{}）：{}",
            kind,
            error.message()
        );
        assert!(
            error.message().contains("模型结构错（共 "),
            "{kind}：文案含诊断条数前缀：{}",
            error.message()
        );
    }
}

/// 异常（AC-4 / AC-2 的行为侧证）：结构错模型 → `Err` 前后对 `root` 递归快照（字节 + mtime）
/// 相等；零临时文件残留（`.tmp` 形态零出现）。
#[test]
fn workspace_render坏模型零写盘() {
    let temp = TempDir::new("render-broken-no-write");
    temp.write(
        ".dev-team/architecture/models/01-test.toml",
        broken_models()[0].1,
    );
    temp.write_contract("App.Core.Archi", &typed_contract("App.Core.Archi"));

    let before = temp.snapshot();
    let error = workspace_render(temp.root()).expect_err("结构错 ⇒ Err");
    assert!(matches!(error, ApiError::Element(_)), "错误面为 Element");
    let after = temp.snapshot();

    assert_eq!(before, after, "纯读盘：字节与 mtime 逐项相等");
    assert!(
        !after
            .iter()
            .any(|(relative, _, _)| relative.contains(".tmp")),
        "零临时文件残留：{after:?}"
    );
}

// ---------------------------------------------------------------------------
// 边界：空模型 / 零契约 / root 形态 / 超大语料
// ---------------------------------------------------------------------------

/// 边界（AC-4 / D1）：①模型目录缺席②`models/` 存在但零 `*.toml` → 均 `Ok`；返回集**恰一份**
/// `INDEX.toml`（零 `.i.rs` / 零 `.i.ts` / 零 `.schema.json`）。
#[test]
fn workspace_render空模型冷启动应生成集() {
    let absent = TempDir::new("render-empty-absent");
    let no_toml = TempDir::new("render-empty-no-toml");
    fs::create_dir_all(no_toml.root().join(".dev-team/architecture/models")).expect("建空模型目录");

    for (tag, temp) in [("模型目录缺席", &absent), ("零 toml", &no_toml)] {
        let files = workspace_render(temp.root()).expect("空模型不报错");
        assert_eq!(files.len(), 1, "{tag}：恰一份生成物");
        assert_eq!(
            files[0].path,
            paths::index_path(temp.root()),
            "{tag}：唯一生成物是 INDEX.toml"
        );
        assert!(!files[0].content.is_empty(), "{tag}：索引内容非空");
    }
}

/// 边界（AC-4 / D1）：模型非空但契约面缺席 → `Ok`；仅 `INDEX.toml`，不 `Err`
/// （判据缺口归 `check`，归集行为归 `compare`）。
#[test]
fn workspace_render零契约() {
    let temp = TempDir::new("render-no-contract");
    temp.write_model(
        "[element.\"App.Core.Archi\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/core/archi/\"]\n",
    );

    let files = workspace_render(temp.root()).expect("零契约不报错");
    assert_eq!(files.len(), 1, "只有索引一份（零 schema / 零 .i.*）");
    let names: Vec<String> = files
        .iter()
        .map(|file| file.path.to_string_lossy().replace('\\', "/"))
        .collect();
    assert!(
        names.iter().all(|name| name.ends_with("INDEX.toml")),
        "仅 INDEX.toml：{names:?}"
    );
}

/// 边界（边界 / `&Path` 映射）：`root` 为空 `Path` / 不存在路径 / 指向文件 → 均 `Ok`
/// 且返回集恰一份 `INDEX.toml`（元素表空 ⇒ 零 schema / 零 `.i.*`）；超长（>1000 chars）/
/// 含空格 / 非 ASCII / emoji 的 `root` → 同形，不 panic。
#[test]
fn workspace_render_root形态边界() {
    let file_root = TempDir::new("render-root-is-file");
    file_root.write("plain.txt", "not a dir\n");
    let file_path = file_root.root().join("plain.txt");

    let long_root = std::env::temp_dir().join("x".repeat(1200));
    let spaced = std::env::temp_dir().join("archi render with space");
    let non_ascii = std::env::temp_dir().join("架构-渲染-根");
    let emoji = std::env::temp_dir().join("archi-render-🎯-root");

    let cases: Vec<(&str, PathBuf)> = vec![
        ("空 Path", PathBuf::from("")),
        (
            "不存在路径",
            std::env::temp_dir().join("archi-render-no-such-root"),
        ),
        ("指向文件", file_path),
        ("超长路径", long_root),
        ("含空格", spaced),
        ("非 ASCII", non_ascii),
        ("含 emoji", emoji),
    ];

    for (tag, root) in cases {
        let outcome = workspace_render(&root);
        match outcome {
            Ok(files) => {
                assert_eq!(files.len(), 1, "{tag}：无非空模型 ⇒ 恰一份生成物");
                assert!(
                    files[0].path.to_string_lossy().ends_with("INDEX.toml"),
                    "{tag}：唯一生成物是索引：{:?}",
                    files[0].path
                );
                assert!(!files[0].content.is_empty(), "{tag}：索引内容非空");
            }
            Err(error) => panic!("{tag}：元素表缺席不应 Err，实际：{error:?}"),
        }
    }
}

/// 边界（边界 / list 映射）：合成 60 元素各带契约且无实现入口（⇒ 零 `.i.*`）→ 返回集
/// 规模 = 60 份 schema + 1 份索引；两次调用逐字段相等；不抛错。
#[test]
fn workspace_render超大语料() {
    let temp = TempDir::new("render-large-corpus");
    let mut model = String::new();
    for index in 0..60 {
        let fqn = format!("App.Core.M{index:02}");
        model.push_str(&format!(
            "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
             path = [\"crates/core/m{index:02}/\"]\n\n"
        ));
    }
    temp.write_model(&model);
    for index in 0..60 {
        let fqn = format!("App.Core.M{index:02}");
        temp.write_contract(&fqn, &typed_contract(&fqn));
    }

    let first = workspace_render(temp.root()).expect("大语料不抛错");
    let second = workspace_render(temp.root()).expect("大语料不抛错");
    assert_eq!(first, second, "两次渲染逐字段相等");
    assert_eq!(
        first.len(),
        61,
        "60 份 schema + 1 份索引（无入口 ⇒ 零 .i.*）"
    );
    assert_eq!(
        first.last().map(|file| file.path.clone()),
        Some(paths::index_path(temp.root())),
        "索引恒在末位"
    );
}

// ---------------------------------------------------------------------------
// 纪律：纯读盘零 spawn（源码自证）
// ---------------------------------------------------------------------------

/// 异常（AC-4 / 纪律判据）：`include_str!("render.rs")` 自证零写盘 / 零 spawn / 零自建遍历；
/// `pub fn` 计数恰 1；三个下潜调用各恰一处（零第二份 collect）。
#[test]
fn workspace_render纯读盘零spawn() {
    let source = include_str!("render.rs");
    for forbidden in [
        "fs::write",
        "create_dir",
        "create_dir_all",
        "File::create",
        "std::process",
        "tokio::process",
        "WalkBuilder",
        "read_dir",
    ] {
        assert!(
            !source.contains(forbidden),
            "`api/render.rs` 零 `{forbidden}`（纯读盘、零 spawn、零自建遍历）"
        );
    }
    assert_eq!(
        source.matches("pub fn ").count(),
        1,
        "公共面恰一个函数（workspace_render）"
    );
    assert_eq!(
        source.matches("elements_of(root)").count(),
        1,
        "元素表取数恰一处（零第二份投影）"
    );
    assert_eq!(
        source.matches("load_contracts(root, &elements)").count(),
        1,
        "契约装载恰一处"
    );
    assert_eq!(
        source.matches("render_all(root, &snapshot)").count(),
        1,
        "渲染恰一处（零第二份 collect）"
    );
}
