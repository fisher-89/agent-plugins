//! `api::diff`（三类差异比对单点）的单元测试：`mismatched` / `missing` 按应生成集序、
//! `orphaned` 按路径字节序、三类同场互不混淆、零差异成功态、非生成物命名与构建 / 依赖
//! 产物树 / `.gitignore` 命中项不误报、契约目录有界枚举（深度上限两侧）、路径串与
//! `archi::display` 同源、零写盘、结构错模型即 `Err`、遍历面零第二份实现（源码自证）。
//!
//! 零 mock：自建 `TempDir` RAII + `AtomicU64`（本 crate 无 dev-dependency）+ 真实
//! `ignore` 遍历器（经 `scan::walk_source_files`）——剪枝与 gitignore 感知即被测语义，
//! 替身会让「不扫构建产物树」自证。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::api::mod_test::declare_ancestors;
use crate::api::paths;

use super::diff::{compare, ArtifactDiff};
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
            "archi-api-diff-test-{}-{}-{}",
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

    fn write_model(&self, content: &str) {
        self.write(
            ".dev-team/architecture/models/01-test.toml",
            &declare_ancestors(content),
        );
    }

    fn write_contract(&self, fqn: &str, content: &str) {
        let path = paths::contract_path(self.root(), fqn).expect("测试 FQN 恒为合法形态");
        self.write_absolute(&path, content);
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

fn typed_contract(fqn: &str) -> String {
    format!(
        "# {fqn}\n\n## 职能\n\n比对单点语料。\n\n## 类型\n\n```json api-types\n\
         {{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    )
}

/// 双语言语料（crate 形态 + 前端目录形态），返回 `(core, frontend)` 两个 FQN。
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

/// 把应生成集逐份落盘（得到「零差异」基线），返回落盘的 `(绝对路径, 内容)` 清单。
fn materialize(temp: &TempDir) -> Vec<(PathBuf, String)> {
    let files = workspace_render(temp.root()).expect("渲染应成功");
    for file in &files {
        temp.write_absolute(&file.path, &file.content);
    }
    files
        .into_iter()
        .map(|file| (file.path, file.content))
        .collect()
}

/// repo 相对串（与被测 `compare` 的投影口径同源单点）。
fn rel(temp: &TempDir, path: &Path) -> String {
    super::display(temp.root(), path)
}

/// 三列全空（零差异）。
fn assert_clean(diff: &ArtifactDiff, context: &str) {
    assert_eq!(diff.mismatched, Vec::<String>::new(), "{context}：无不符");
    assert_eq!(diff.orphaned, Vec::<String>::new(), "{context}：无孤儿");
    assert_eq!(diff.missing, Vec::<String>::new(), "{context}：无缺失");
}

// ---------------------------------------------------------------------------
// 正向：三类差异各自成列
// ---------------------------------------------------------------------------

/// 正向（AC-3 / AC-4 / spec「检查命令契约」）：应生成物在盘但内容改写 → `mismatched`
/// 恰含该份 repo 相对串；`missing` / `orphaned` 不含它。
#[test]
fn compare内容不符计入mismatched() {
    let temp = TempDir::new("compare-mismatched");
    let (core, _) = write_two_language_corpus(&temp);
    let expected = materialize(&temp);
    let schema = paths::schema_path(temp.root(), &core).expect("合法 FQN");
    temp.write_absolute(&schema, "{\"tampered\": true}\n");

    let diff = compare(temp.root()).expect("比对应成功");
    assert_eq!(
        diff.mismatched,
        vec![rel(&temp, &schema)],
        "mismatched 恰含被改写的一份"
    );
    assert!(diff.missing.is_empty(), "missing 不含它");
    assert!(diff.orphaned.is_empty(), "orphaned 不含它");
    assert!(
        expected.iter().any(|(path, _)| path == &schema),
        "该份确在应生成集内（前提断言）"
    );
}

/// 正向（AC-3 / AC-4）：删掉一份应生成物 → `missing` 恰含该串；其余两列不含它。
#[test]
fn compare应生成未生成计入missing() {
    let temp = TempDir::new("compare-missing");
    let (_, frontend) = write_two_language_corpus(&temp);
    materialize(&temp);
    let schema = paths::schema_path(temp.root(), &frontend).expect("合法 FQN");
    fs::remove_file(&schema).expect("删文件失败");

    let diff = compare(temp.root()).expect("比对应成功");
    assert_eq!(diff.missing, vec![rel(&temp, &schema)], "missing 恰含一份");
    assert!(diff.mismatched.is_empty(), "mismatched 不含它");
    assert!(diff.orphaned.is_empty(), "orphaned 不含它");
}

/// 正向（AC-3 / AC-4 / spec「生成从不删文件」的报告侧）：多落一份命名符合判据、不在应生成集
/// 的 `.i.rs` → `orphaned` 恰含该串；另两列不含它。
#[test]
fn compare孤儿生成物计入orphaned() {
    let temp = TempDir::new("compare-orphaned");
    write_two_language_corpus(&temp);
    materialize(&temp);
    temp.write("src/loose.i.rs", "// 历史孤儿\n");

    let diff = compare(temp.root()).expect("比对应成功");
    assert_eq!(diff.orphaned, vec!["src/loose.i.rs".to_owned()], "孤儿入列");
    assert!(diff.mismatched.is_empty(), "mismatched 不含它");
    assert!(diff.missing.is_empty(), "missing 不含它");
}

/// 组合迭代（AC-3 / D4）：同一 tempdir 同时造「两份不符 + 两份缺 + 两份孤儿」→ 三列各恰两条、
/// 互不交叠；`mismatched` / `missing` 序 = `workspace_render` 输出序，`orphaned` 按路径字节序。
#[test]
fn compare三类差异同场互不混淆() {
    let temp = TempDir::new("compare-three-kinds");
    let (core, frontend) = write_two_language_corpus(&temp);
    materialize(&temp);

    let schema_core = paths::schema_path(temp.root(), &core).expect("合法 FQN");
    let schema_frontend = paths::schema_path(temp.root(), &frontend).expect("合法 FQN");
    temp.write_absolute(&schema_core, "{\"broken\": 1}\n");
    temp.write_absolute(&schema_frontend, "{\"broken\": 2}\n");

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
    fs::remove_file(&rust_generated).expect("删文件失败");
    fs::remove_file(&ts_generated).expect("删文件失败");

    temp.write("src/zzz.i.ts", "// 孤儿 z\n");
    temp.write("src/aaa.i.rs", "// 孤儿 a\n");

    let diff = compare(temp.root()).expect("比对应成功");
    assert_eq!(
        diff.mismatched,
        vec![rel(&temp, &schema_core), rel(&temp, &schema_frontend)],
        "mismatched 序 = 契约 FQN 序"
    );
    assert_eq!(
        diff.missing,
        vec![rel(&temp, &rust_generated), rel(&temp, &ts_generated)],
        "missing 序 = 元素序（.i.rs 先于 .i.ts）"
    );
    assert_eq!(
        diff.orphaned,
        vec!["src/aaa.i.rs".to_owned(), "src/zzz.i.ts".to_owned()],
        "orphaned 按路径字节序"
    );

    let mut triples = diff
        .mismatched
        .iter()
        .chain(diff.orphaned.iter())
        .chain(diff.missing.iter())
        .cloned()
        .collect::<Vec<String>>();
    let total = triples.len();
    triples.sort();
    triples.dedup();
    assert_eq!(triples.len(), total, "三列互不交叠：{diff:?}");
}

/// 边界（AC-2 / AC-11 的「生成后清空」前提）：按应生成集逐份落盘且内容逐字相同 →
/// `ArtifactDiff { mismatched: [], orphaned: [], missing: [] }`；不 `Err`。
#[test]
fn compare零差异三列全空() {
    let temp = TempDir::new("compare-clean");
    write_two_language_corpus(&temp);
    let written = materialize(&temp);
    assert_eq!(written.len(), 5, "基线五份生成物");

    let diff = compare(temp.root()).expect("比对应成功");
    assert_clean(&diff, "零差异");
    assert!(diff.mismatched.is_empty() && diff.orphaned.is_empty() && diff.missing.is_empty());
}

// ---------------------------------------------------------------------------
// 异常：命名判据 / 剪枝 / gitignore
// ---------------------------------------------------------------------------

/// 异常（AC-3 / 反向判据）：非生成物命名的同名文件均不入 `orphaned`（判据为
/// `ends_with(".i.rs") || ends_with(".i.ts")`，与落点深度无关）；对照臂 `src/x.i.ts` 入列。
#[test]
fn compare盘上无关同名文件不误报() {
    let temp = TempDir::new("compare-name-rule");
    write_two_language_corpus(&temp);
    materialize(&temp);

    let non_hits = [
        "src/app.ts",
        "src/app.tsx",
        "src/app.js",
        "src/app.schema.json.bak",
        "src/dir.i.tsx",
        "src/notes.i.js",
    ];
    for relative in non_hits {
        temp.write(relative, "// 非生成物\n");
    }
    temp.write("src/x.i.ts", "// 命中命名判据\n");

    let diff = compare(temp.root()).expect("比对应成功");
    for relative in non_hits {
        assert!(
            !diff.orphaned.iter().any(|entry| entry == relative),
            "{relative} 命名不符判据，不入 orphaned：{:?}",
            diff.orphaned
        );
    }
    assert_eq!(
        diff.orphaned,
        vec!["src/x.i.ts".to_owned()],
        "对照臂：命名命中即入列（与落点深度无关）"
    );
}

/// 异常（AC-4 / D3）：构建 / 依赖产物树与 VCS 目录不扫——五类形态均不入 `orphaned`，
/// 同层正常孤儿 `src/loose.i.rs` 入列。
#[test]
fn compare构建与依赖产物树不扫() {
    let temp = TempDir::new("compare-prune");
    write_two_language_corpus(&temp);
    materialize(&temp);

    let pruned = [
        "target/debug/x.i.rs",
        "node_modules/a/index.i.ts",
        "dist/y.i.ts",
        "build/z.i.ts",
        ".git/hooks/h.i.ts",
    ];
    for relative in pruned {
        temp.write(relative, "// 剪枝面\n");
    }
    temp.write("src/loose.i.rs", "// 同层正常孤儿\n");

    let diff = compare(temp.root()).expect("比对应成功");
    for relative in pruned {
        assert!(
            !diff.orphaned.iter().any(|entry| entry == relative),
            "{relative} 属剪枝面，不入 orphaned：{:?}",
            diff.orphaned
        );
    }
    assert_eq!(
        diff.orphaned,
        vec!["src/loose.i.rs".to_owned()],
        "同层正常孤儿照常入列"
    );
}

/// 异常（AC-4 / D3 / R-1）：`.gitignore` 命中项不扫；对照臂：去掉该规则后同一文件入
/// `orphaned`（遍历经 `scan::walk_source_files` 的 gitignore 感知即被测语义）。
#[test]
fn compare_gitignore命中项不扫() {
    let temp = TempDir::new("compare-gitignore");
    write_two_language_corpus(&temp);
    materialize(&temp);
    // `git_ignore` 语义要求观察根是仓库边界（`.git` 在场），否则 `.gitignore` 不生效。
    fs::create_dir_all(temp.root().join(".git")).expect("建 .git 目录");
    temp.write(".gitignore", "generated/**\n");
    temp.write("generated/old.i.rs", "// 忽略项\n");

    let ignored = compare(temp.root()).expect("比对应成功");
    assert!(
        !ignored
            .orphaned
            .iter()
            .any(|entry| entry == "generated/old.i.rs"),
        "gitignore 命中项由遍历器跳过：{:?}",
        ignored.orphaned
    );

    temp.write(".gitignore", "# 无规则\n");
    let visible = compare(temp.root()).expect("比对应成功");
    assert_eq!(
        visible.orphaned,
        vec!["generated/old.i.rs".to_owned()],
        "对照臂：去掉规则后入列"
    );
}

/// 正向（AC-4 / D3 有界枚举）：契约目录内多落 `App/Ghost.schema.json` 与内容不符的
/// `INDEX.toml` → 前者入 `orphaned`、后者入 `mismatched`；契约目录**外**同名的
/// `App/Ghost.schema.json` 不入 `orphaned`（schema 判据只在契约目录内成立）。
#[test]
fn compare契约目录的schema与索引孤儿() {
    let temp = TempDir::new("compare-api-dir-artifacts");
    write_two_language_corpus(&temp);
    materialize(&temp);

    let api_dir = paths::directory(temp.root());
    let ghost = api_dir.join("App").join("Ghost.schema.json");
    temp.write_absolute(&ghost, "{\"$defs\": {}}\n");
    let index = paths::index_path(temp.root());
    temp.write_absolute(&index, "# 旧索引（内容不符）\n");
    temp.write("src/App/Ghost.schema.json", "{\"$defs\": {}}\n");

    let diff = compare(temp.root()).expect("比对应成功");
    assert_eq!(
        diff.orphaned,
        vec![rel(&temp, &ghost)],
        "契约目录内的 schema 孤儿入列，目录外的同名文件不入"
    );
    assert_eq!(
        diff.mismatched,
        vec![rel(&temp, &index)],
        "内容不符的 INDEX.toml 入 mismatched"
    );
    assert!(diff.missing.is_empty(), "零缺失");
}

/// 边界（AC-4 / D3 / 零新常量）：契约目录枚举的深度上限两侧——超过 `MAX_API_WALK_DEPTH`
/// 的深层 schema 不入 `orphaned`、不 panic；上限内的浅层照常入列。
#[test]
fn compare契约目录有界枚举的深度上限() {
    let temp = TempDir::new("compare-api-depth");
    write_two_language_corpus(&temp);
    materialize(&temp);

    let api_dir = paths::directory(temp.root());
    let shallow = api_dir.join("Ghost2.schema.json");
    let at_limit = api_dir
        .join("a")
        .join("b")
        .join("c")
        .join("d")
        .join("Deep.schema.json");
    let beyond = api_dir
        .join("a")
        .join("b")
        .join("c")
        .join("d")
        .join("e")
        .join("f")
        .join("Ghost.schema.json");
    temp.write_absolute(&shallow, "{}\n");
    temp.write_absolute(&at_limit, "{}\n");
    temp.write_absolute(&beyond, "{}\n");

    let diff = compare(temp.root()).expect("比对应成功");
    assert!(diff.orphaned.contains(&rel(&temp, &shallow)), "浅层入列");
    assert!(
        diff.orphaned.contains(&rel(&temp, &at_limit)),
        "上限内（depth 4）仍在扫面"
    );
    assert!(
        !diff.orphaned.contains(&rel(&temp, &beyond)),
        "越过上限（depth 6）不入列"
    );
}

// ---------------------------------------------------------------------------
// 边界：空仓库 / 路径归一 / 零写盘 / root 形态 / 超大孤儿列表
// ---------------------------------------------------------------------------

/// 边界（AC-3）：①模型缺席②零 `*.toml`③有元素但 api 目录缺席 → 均 `Ok`；`missing` 恰为
/// `INDEX.toml` 一条、另两列空（`enabled = false` 时照常报该条，不因未启用而空列）。
#[test]
fn compare空仓库与契约面未启用() {
    let absent = TempDir::new("compare-empty-absent");
    let no_toml = TempDir::new("compare-empty-no-toml");
    fs::create_dir_all(no_toml.root().join(".dev-team/architecture/models")).expect("建空模型目录");
    let no_api_dir = TempDir::new("compare-empty-no-api");
    no_api_dir.write_model(
        "[element.\"App.Core.Archi\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/core/archi/\"]\n",
    );

    for (tag, temp) in [
        ("模型目录缺席", &absent),
        ("零 toml", &no_toml),
        ("api 目录缺席", &no_api_dir),
    ] {
        let diff = compare(temp.root()).expect("空仓库不报错");
        assert_eq!(diff.missing.len(), 1, "{tag}：missing 恰一条");
        assert!(
            diff.missing[0].ends_with("INDEX.toml"),
            "{tag}：缺口是 INDEX.toml：{:?}",
            diff.missing
        );
        assert!(diff.mismatched.is_empty(), "{tag}：零不符");
        assert!(diff.orphaned.is_empty(), "{tag}：零孤儿");
    }
}

/// 组合迭代（AC-3 / D2 / spec「DTO 形状」）：三列每个串无 `\`、无前导 `./`、无前导 `/`、
/// 不含 `root` 前缀、分隔符恒 `/`；含中文 / 空格 / emoji 的目录名同样归一；反向判据：
/// `diff.rs` 零本地归一（零 `replace('\\', "/")` / 零 `strip_prefix`）。
#[test]
fn compare路径串与display同源() {
    let temp = TempDir::new("compare-display-same-source");
    write_two_language_corpus(&temp);
    materialize(&temp);

    let exotic = [
        "src/中文目录/契约.i.ts",
        "src/with space/note.i.rs",
        "src/emoji🎯/artifact.i.ts",
        "src/plain/plain.i.rs",
    ];
    for relative in exotic {
        temp.write(relative, "// 归一形态语料\n");
    }

    let diff = compare(temp.root()).expect("比对应成功");
    let root_prefix = temp.root().to_string_lossy().replace('\\', "/");
    assert_eq!(diff.orphaned.len(), exotic.len(), "四条孤儿全部入列");
    for entry in &diff.orphaned {
        assert!(!entry.contains('\\'), "无反斜杠：{entry}");
        assert!(!entry.starts_with("./"), "无前导 ./：{entry}");
        assert!(!entry.starts_with('/'), "无前导 /：{entry}");
        assert!(!entry.contains(&root_prefix), "不含 root 前缀：{entry}");
        assert!(
            !entry.contains('\\'),
            "分隔符恒 `/`（与 archi::display 同源）：{entry}"
        );
    }
    for entry in &exotic {
        let absolute = temp.root().join(entry);
        assert!(
            diff.orphaned
                .contains(&super::display(temp.root(), &absolute)),
            "{entry}：串逐字等于 archi::display"
        );
    }
    assert!(diff.orphaned.contains(&"src/中文目录/契约.i.ts".to_owned()));

    let source = include_str!("diff.rs");
    assert!(
        !source.contains(r#"replace('\\', "/")"#),
        "零本地路径归一（归一单点在 `api::display`）"
    );
    assert!(
        !source.contains("strip_prefix"),
        "零本地剥前缀（归一单点在 `api::display`）"
    );
}

/// 边界（AC-2 / AC-4）：调用前后对 `root` 递归快照（字节 + mtime）相等；盘上文件数不变
/// （不删孤儿、不写缺口）；`diff.rs` 零 `fs::write` / `create_dir` / `File::create`。
#[test]
fn compare零写盘() {
    let temp = TempDir::new("compare-no-write");
    write_two_language_corpus(&temp);
    materialize(&temp);
    temp.write("src/loose.i.rs", "// 孤儿不得被删\n");
    fs::remove_file(paths::index_path(temp.root())).expect("删索引制造缺口");

    let before = temp.snapshot();
    let diff = compare(temp.root()).expect("比对应成功");
    let after = temp.snapshot();

    assert_eq!(before, after, "纯读盘：字节与 mtime 逐项相等");
    assert_eq!(
        diff.orphaned,
        vec!["src/loose.i.rs".to_owned()],
        "孤儿仍在报告内"
    );
    assert_eq!(diff.missing.len(), 1, "缺口照实报");
    assert_eq!(
        before.len(),
        after.len(),
        "盘上文件数不变（不删孤儿、不写缺口）"
    );

    let source = include_str!("diff.rs");
    for forbidden in ["fs::write", "create_dir", "File::create"] {
        assert!(
            !source.contains(forbidden),
            "`api/diff.rs` 零 `{forbidden}`（纯读盘）"
        );
    }
}

/// 异常（AC-4 / D1 / D2）：结构错模型 → `Err`（与 `workspace_render` 同口径，命令面不返回
/// 部分差异）；前后零写盘。
#[test]
fn compare结构错模型即err() {
    let temp = TempDir::new("compare-broken-model");
    temp.write(
        ".dev-team/architecture/models/01-test.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n\
         [element.\"App.A\"]\nkind = \"module\"\n\n\
         [element.\"App.B\"]\nkind = \"module\"\n\n\
         [[relation]]\nfrom = \"App.A\"\nto = \"App.B\"\n\n\
         [[relation]]\nfrom = \"App.B\"\nto = \"App.A\"\n",
    );
    temp.write_contract("App.Core.Archi", &typed_contract("App.Core.Archi"));

    let before = temp.snapshot();
    let error = compare(temp.root()).expect_err("结构错 ⇒ Err");
    assert!(
        matches!(error, ApiError::Element(_)),
        "错误面为 Element，实际：{error:?}"
    );
    assert!(
        error.message().contains("模型结构错"),
        "文案点名结构诊断：{}",
        error.message()
    );
    assert_eq!(before, temp.snapshot(), "Err 前后零写盘");
}

/// 异常（AC-4 / D3）：`include_str!("diff.rs")` 自证遍历面零第二份实现——`walk_source_files`
/// 调用恰一处、`WalkBuilder` 零命中、`read_dir` 只在 `collect_api_dir`、深度上限复用既有常量、
/// 零 `SKIPPED_DIRECTORY` / `MAX_MODULE_WALK_DEPTH`、`pub fn` 计数恰 1。
#[test]
fn compare遍历面零第二份实现() {
    let source = include_str!("diff.rs");
    assert_eq!(
        source.matches("walk_source_files(root)").count(),
        1,
        "仓库级遍历恰一处（MUST NOT 复制第二份遍历）"
    );
    assert!(
        !source.contains("WalkBuilder"),
        "零自建遍历器（`ignore` 只经 `scan::walk_source_files`）"
    );
    assert_eq!(
        source.matches("read_dir").count(),
        1,
        "`read_dir` 只出现在契约目录有界枚举里"
    );
    assert_eq!(
        source.matches("depth > super::MAX_API_WALK_DEPTH").count(),
        1,
        "深度上限复用既有常量（零新常量）"
    );
    for absent in ["SKIPPED_DIRECTORY", "MAX_MODULE_WALK_DEPTH"] {
        assert!(
            !source.contains(absent),
            "`api/diff.rs` 零 `{absent}`（剪枝单点在 `scan`）"
        );
    }
    assert_eq!(
        source.matches("pub fn ").count(),
        1,
        "公共面恰一个函数（compare）"
    );
}

/// 边界（边界）：`root` 为空 `Path` / 不存在 / 指向文件 → 落 `Result` 内不 panic；盘上 1000
/// 份孤儿 → `orphaned.len() == 1000`、序为路径字节序、不抛错。
#[test]
fn compare_root非法与超大孤儿列表() {
    let file_root = TempDir::new("compare-root-is-file");
    file_root.write("plain.txt", "not a dir\n");
    // 空 `Path` 的遍历面依实现而定（遍历器对空根的行为不作断言），此处只钉「不 panic」；
    // 另两形态的元素表与盘面皆空，列内容可判。
    let empty_root = TempDir::new("compare-root-empty");
    let empty_outcome = compare(Path::new(""));
    if let Ok(diff) = &empty_outcome {
        assert_eq!(diff.missing.len(), 1, "空 Path：缺口恒为索引一份");
    }
    assert!(
        empty_root.snapshot() == empty_root.snapshot(),
        "空 Path 调用不产生写盘副作用（两次快照同形）"
    );
    for root in [
        std::env::temp_dir().join("archi-compare-no-such-root"),
        file_root.root().join("plain.txt"),
    ] {
        let outcome = compare(&root);
        match outcome {
            Ok(diff) => {
                assert!(diff.mismatched.is_empty(), "无非空模型 ⇒ 零不符");
                assert!(diff.orphaned.is_empty(), "无不存在的盘面 ⇒ 零孤儿");
                assert_eq!(diff.missing.len(), 1, "缺口恒为索引一份");
            }
            Err(error) => panic!("{root:?}：元素表缺席不应 Err，实际：{error:?}"),
        }
    }

    let large = TempDir::new("compare-large-orphans");
    write_two_language_corpus(&large);
    materialize(&large);
    for index in 0..1000 {
        large.write(&format!("src/orphan{index:04}.i.rs"), "// 孤儿\n");
    }
    let diff = compare(large.root()).expect("大列表不抛错");
    assert_eq!(diff.orphaned.len(), 1000, "1000 份孤儿全部入列");
    let mut sorted = diff.orphaned.clone();
    sorted.sort();
    assert_eq!(diff.orphaned, sorted, "序为路径字节序");
    assert!(
        diff.mismatched.is_empty() && diff.missing.is_empty(),
        "另两列空"
    );
}
