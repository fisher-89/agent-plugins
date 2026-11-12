//! `tree::file_tree` 的单元测试：真实 tempdir（模型 / 源文件 / 架构域配置 /
//! `.gitignore` 全落盘）+ 真实 `ignore::WalkBuilder` + 真实 `OwnerIndex`，零 mock。
//!
//! 判据只经公共面观测：`entries` 的 `path` / `name` / `kind` / `mark` / `owners` /
//! `contains` / `exists` 七字段。私有 helper（`candidate_files` / `node_set` /
//! `mark_of` / `exists_of` / `ancestors` / `name_of`）一律不直调——直调会绕过
//! 「判据顺序 ① 模块根表 → ② `owner_of` → ③ 后代含根」的断言面。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::{archi_config_path, archi_models_dir};

use crate::dto::{ArchiFileEntry, ArchiFileTree, FileEntryKind, FileTreeMark};

use crate::tree::file_tree;

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-tree-test-{}-{}-{}",
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

    /// 落模型（单文件）：每项为 `(FQN, kind, path 列表)`。
    fn write_model(&self, elements: &[(&str, &str, &[&str])]) {
        let dir = archi_models_dir(&self.0);
        fs::create_dir_all(&dir).expect("创建模型目录失败");
        fs::write(dir.join("01-test.toml"), model_toml(elements)).expect("写模型文件失败");
    }

    fn write_config(&self, json: &str) {
        let path = archi_config_path(&self.0);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建架构域目录失败");
        }
        fs::write(&path, json).expect("写配置失败");
    }

    /// 递归字节快照（相对路径 → 字节；用于「零写盘」对拍）。
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut frontier: Vec<PathBuf> = vec![self.0.clone()];
        while let Some(dir) = frontier.pop() {
            let Ok(read) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    frontier.push(path);
                    continue;
                }
                let relative = path
                    .strip_prefix(&self.0)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/");
                entries.insert(relative, fs::read(&path).expect("读文件失败"));
            }
        }
        entries
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 单元素 TOML 表头（`path` 为空时不写该键，等价于 `path = []`）。
fn element_toml(fqn: &str, kind: &str, paths: &[&str]) -> String {
    let mut source = format!("[element.\"{fqn}\"]\nkind = \"{kind}\"\n");
    if !paths.is_empty() {
        let quoted: Vec<String> = paths.iter().map(|path| format!("\"{path}\"")).collect();
        source.push_str(&format!("path = [{}]\n", quoted.join(", ")));
    }
    source.push('\n');
    source
}

fn model_toml(elements: &[(&str, &str, &[&str])]) -> String {
    elements
        .iter()
        .map(|(fqn, kind, paths)| element_toml(fqn, kind, paths))
        .collect()
}

// ---------------------------------------------------------------------------
// 观测辅助（只读公共面）
// ---------------------------------------------------------------------------

fn paths_of(tree: &ArchiFileTree) -> Vec<&str> {
    tree.entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect()
}

fn entry_of<'a>(tree: &'a ArchiFileTree, path: &str) -> &'a ArchiFileEntry {
    match tree.entries.iter().find(|entry| entry.path == path) {
        Some(entry) => entry,
        None => panic!("树中缺条目 {path:?}；实际条目：{:?}", paths_of(tree)),
    }
}

/// 断言条目及其整棵子树都不在树中（目录缺席即其下源文件也缺席）。
fn assert_absent(tree: &ArchiFileTree, needle: &str) {
    let prefix = format!("{needle}/");
    let hits: Vec<&str> = paths_of(tree)
        .into_iter()
        .filter(|path| *path == needle || path.starts_with(&prefix))
        .collect();
    assert!(
        hits.is_empty(),
        "条目 {needle:?} 及其子树不应进树，实际命中：{hits:?}"
    );
}

fn owners_of(entry: &ArchiFileEntry) -> Vec<&str> {
    entry.owners.iter().map(String::as_str).collect()
}

/// 逐条断言：条目在场 + 形态 + 标记 + 归属。
fn assert_entry(
    tree: &ArchiFileTree,
    path: &str,
    kind: FileEntryKind,
    mark: FileTreeMark,
    owners: &[&str],
) -> ArchiFileEntry {
    let entry = entry_of(tree, path);
    assert_eq!(entry.kind, kind, "{path:?} 形态");
    assert_eq!(entry.mark, mark, "{path:?} 标记");
    assert_eq!(owners_of(entry), owners, "{path:?} 归属");
    entry.clone()
}

// ===========================================================================
// 归属判定（AC-5）：目录型 / 单文件型 / 祖先 / 嵌套 / 多路径 / 容器
// ===========================================================================

/// 正向（AC-5 / spec Scenario 1）：元素根目录标 `elementRoot`，其内部目录与文件
/// 标 `member` 且归属同一元素。
#[test]
fn 目录型元素根与其内部文件的归属分层() {
    let temp = TempDir::new("dir-root");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Core", "domain", &[]),
        ("App.Core.Archi", "module", &["./crates/core/archi/"]),
    ]);
    temp.write("crates/core/archi/src/lib.rs", "pub fn a() {}\n");
    temp.write("crates/core/archi/src/dto.rs", "pub struct A;\n");

    let tree = file_tree(temp.root()).expect("健康模型应 Ok");

    let root_entry = assert_entry(
        &tree,
        "crates/core/archi",
        FileEntryKind::Dir,
        FileTreeMark::ElementRoot,
        &["App.Core.Archi"],
    );
    assert!(root_entry.exists, "在盘的元素根目录存在性恒真");
    assert_eq!(root_entry.contains, 0, "元素根自身不含后代模块");

    assert_entry(
        &tree,
        "crates/core/archi/src",
        FileEntryKind::Dir,
        FileTreeMark::Member,
        &["App.Core.Archi"],
    );
    assert_entry(
        &tree,
        "crates/core/archi/src/lib.rs",
        FileEntryKind::File,
        FileTreeMark::Member,
        &["App.Core.Archi"],
    );
    assert_entry(
        &tree,
        "crates/core/archi/src/dto.rs",
        FileEntryKind::File,
        FileTreeMark::Member,
        &["App.Core.Archi"],
    );
    assert_eq!(
        entry_of(&tree, "crates/core/archi/src/lib.rs").contains,
        0,
        "文件条目的后代计数恒 0"
    );
}

/// 正向（AC-5 / spec Scenario 2）：单文件形元素根——该文件条目即 `elementRoot`。
#[test]
fn 单文件型元素根归该文件条目() {
    let temp = TempDir::new("file-root");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Frontend", "domain", &[]),
        ("App.Frontend.Views", "module", &[]),
        (
            "App.Frontend.Views.Welcome",
            "component",
            &["./packages/desktop/src/views/welcome-view.tsx"],
        ),
    ]);
    temp.write(
        "packages/desktop/src/views/welcome-view.tsx",
        "export const Welcome = 1;\n",
    );

    let tree = file_tree(temp.root()).expect("应 Ok");
    let entry = assert_entry(
        &tree,
        "packages/desktop/src/views/welcome-view.tsx",
        FileEntryKind::File,
        FileTreeMark::ElementRoot,
        &["App.Frontend.Views.Welcome"],
    );
    assert!(entry.exists, "在盘文件存在性恒真");
    assert_eq!(entry.name, "welcome-view.tsx", "末段名");
}

/// 正向（AC-5 / D6③）：无归属但后代含元素根的祖先目录标 `containsElements`，
/// `contains` 按后代模块数计。
#[test]
fn 祖先目录标记为含元素并计数后代模块() {
    let temp = TempDir::new("ancestor");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.A", "module", &["./src/a/"]),
        ("App.B", "module", &["./src/b/"]),
        ("App.C", "module", &["./src/c/"]),
    ]);
    temp.write("src/a/x.ts", "export const a = 1;\n");
    temp.write("src/b/y.ts", "export const b = 1;\n");
    temp.write("src/c/z.ts", "export const c = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    let ancestor = assert_entry(
        &tree,
        "src",
        FileEntryKind::Dir,
        FileTreeMark::ContainsElements,
        &[],
    );
    assert_eq!(ancestor.contains, 3, "三个子元素根各计一次");
    let leaf_root = assert_entry(
        &tree,
        "src/a",
        FileEntryKind::Dir,
        FileTreeMark::ElementRoot,
        &["App.A"],
    );
    assert_eq!(leaf_root.contains, 0, "元素根自身不计入后代数");
}

/// 正向（AC-5 / 正交计数）：父元素目录内既有归属又含子元素根——`mark = member`
/// 与 `contains = 1` 并存，两者互不吞并。
#[test]
fn 后代计数与归属标记正交并存() {
    let temp = TempDir::new("orthogonal");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Core", "domain", &[]),
        ("App.Core.Archi", "module", &["./crates/core/archi/"]),
        (
            "App.Core.Archi.Scan",
            "component",
            &["./crates/core/archi/src/scan/"],
        ),
    ]);
    temp.write("crates/core/archi/src/scan/owner.rs", "pub fn a() {}\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    let entry = assert_entry(
        &tree,
        "crates/core/archi/src",
        FileEntryKind::Dir,
        FileTreeMark::Member,
        &["App.Core.Archi"],
    );
    assert_eq!(entry.contains, 1, "严格前缀内的子元素根计一次");
}

/// 正向（AC-5 / spec Scenario 多 path 去重）：一元素四条文件形路径各出
/// `elementRoot`，祖先目录的后代计数按 **FQN** 去重（1 而非 4）。
#[test]
fn 多路径元素的条目与后代计数按元素去重() {
    let temp = TempDir::new("multi-path");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Frontend", "domain", &[]),
        (
            "App.Frontend.Shell",
            "module",
            &[
                "./src/shell/index.ts",
                "./src/shell/app.ts",
                "./src/shell/routes.ts",
                "./src/shell/store.ts",
            ],
        ),
    ]);
    for name in ["index.ts", "app.ts", "routes.ts", "store.ts"] {
        temp.write(&format!("src/shell/{name}"), "export const x = 1;\n");
    }

    let tree = file_tree(temp.root()).expect("应 Ok");
    for name in ["index.ts", "app.ts", "routes.ts", "store.ts"] {
        assert_entry(
            &tree,
            &format!("src/shell/{name}"),
            FileEntryKind::File,
            FileTreeMark::ElementRoot,
            &["App.Frontend.Shell"],
        );
    }
    let ancestor = entry_of(&tree, "src/shell");
    assert_eq!(
        ancestor.contains, 1,
        "四条路径同属一元素 ⇒ 后代计数按 FQN 去重"
    );
    assert_eq!(ancestor.mark, FileTreeMark::ContainsElements, "自身无归属");
}

/// 正向（AC-5 / spec Scenario 嵌套父子各出根）：父子目录各 `elementRoot`，
/// 内层文件归最深前缀，父目录的后代计数含子元素。
#[test]
fn 嵌套父子各出元素根且内层文件归最深() {
    let temp = TempDir::new("nested");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Core", "domain", &[]),
        ("App.Core.Archi", "module", &["./crates/core/archi/"]),
        (
            "App.Core.Archi.Scan",
            "component",
            &["./crates/core/archi/src/scan/"],
        ),
    ]);
    temp.write("crates/core/archi/src/scan/owner.rs", "pub fn a() {}\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    assert_entry(
        &tree,
        "crates/core/archi",
        FileEntryKind::Dir,
        FileTreeMark::ElementRoot,
        &["App.Core.Archi"],
    );
    assert_entry(
        &tree,
        "crates/core/archi/src/scan",
        FileEntryKind::Dir,
        FileTreeMark::ElementRoot,
        &["App.Core.Archi.Scan"],
    );
    assert_entry(
        &tree,
        "crates/core/archi/src/scan/owner.rs",
        FileEntryKind::File,
        FileTreeMark::Member,
        &["App.Core.Archi.Scan"],
    );
    assert_eq!(
        entry_of(&tree, "crates/core/archi").contains,
        1,
        "父元素的后代计数含子元素根"
    );
}

/// 异常（AC-5 / spec Scenario 同路径多元素声明）：两个元素归一后声明同一路径 →
/// 该目录条目 `ambiguous`、`owners` 字典序全量、绝不判 `elementRoot`。
#[test]
fn 同一归一路径被多元素声明时目录条目歧义() {
    let temp = TempDir::new("ambiguous-root");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Shared", "domain", &[]),
        ("App.Shared.A", "module", &["./src/shared/"]),
        ("App.Shared.B", "module", &["src/shared"]),
    ]);
    temp.write("src/shared/f.ts", "export const f = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    let entry = assert_entry(
        &tree,
        "src/shared",
        FileEntryKind::Dir,
        FileTreeMark::Ambiguous,
        &["App.Shared.A", "App.Shared.B"],
    );
    assert!(
        entry.owners.len() >= 2,
        "歧义条目给出全量候选（不任选其一）"
    );
}

/// 异常（AC-5 / R-10）：歧义目录**内部**文件不经模块根表，走 `owner_of` 的等长
/// 多命中分支——同样 `ambiguous` 且候选全量字典序。
#[test]
fn 歧义目录内部文件同样走最长前缀歧义分支() {
    let temp = TempDir::new("ambiguous-inner");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Shared", "domain", &[]),
        ("App.Shared.A", "module", &["./src/shared/"]),
        ("App.Shared.B", "module", &["src/shared"]),
    ]);
    temp.write("src/shared/f.ts", "export const f = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    let inner = assert_entry(
        &tree,
        "src/shared/f.ts",
        FileEntryKind::File,
        FileTreeMark::Ambiguous,
        &["App.Shared.A", "App.Shared.B"],
    );
    // 与目录条目同一分支：两条来源（模块根表 / 最长前缀）给出同一词
    assert_eq!(
        entry_of(&tree, "src/shared").mark,
        inner.mark,
        "两条 ambiguous 来源共用同一标记词"
    );
}

/// 异常（AC-5 / spec Scenario 容器元素零节点）：`path` 为空的容器元素在任何条目
/// 的 `owners` 中零出现（只在元素树上可见）。
#[test]
fn 容器元素不产任何节点() {
    let temp = TempDir::new("container");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.DomainA", "domain", &[]),
        ("App.DomainA.Module", "module", &[]),
        ("App.DomainA.Module.Component", "component", &[]),
    ]);
    temp.write("src/a.ts", "export const a = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    let containers = [
        "App",
        "App.DomainA",
        "App.DomainA.Module",
        "App.DomainA.Module.Component",
    ];
    for entry in &tree.entries {
        for container in containers {
            assert!(
                !entry.owners.iter().any(|fqn| fqn == container),
                "容器元素 {container} 不得出现在条目 {:?} 的 owners 中",
                entry.path
            );
            assert_ne!(entry.path, container, "容器元素不产同名字节点");
        }
    }
    assert_entry(
        &tree,
        "src/a.ts",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
}

/// 正向（AC-5 / D4）：元素根判据是 `path` 非空而非 `kind`——`domain` 声明了单文件
/// 路径即 `elementRoot`。
#[test]
fn 元素根判据是路径非空而非元素种类() {
    let temp = TempDir::new("kind-irrelevant");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Components", "domain", &[]),
        (
            "App.Components.Agent",
            "domain",
            &["./src/components/agent/index.tsx"],
        ),
    ]);
    temp.write(
        "src/components/agent/index.tsx",
        "export const Agent = 1;\n",
    );

    let tree = file_tree(temp.root()).expect("应 Ok");
    assert_entry(
        &tree,
        "src/components/agent/index.tsx",
        FileEntryKind::File,
        FileTreeMark::ElementRoot,
        &["App.Components.Agent"],
    );
}

/// 正向（AC-7 / spec Scenario 生成物按并集判据归元素）：元素根**同级**生成物
/// （`crates/infra/archi.i.rs`）由整组剥离候选命中，不落 `none`。
#[test]
fn 生成物按并集判据归元素() {
    let temp = TempDir::new("generated-union");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Backend", "domain", &[]),
        ("App.Backend.Infra", "module", &[]),
        (
            "App.Backend.Infra.ArchiRuntime",
            "component",
            &["./crates/infra/archi/"],
        ),
    ]);
    temp.write("crates/infra/archi.i.rs", "pub struct Session;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    assert_entry(
        &tree,
        "crates/infra/archi.i.rs",
        FileEntryKind::File,
        FileTreeMark::Member,
        &["App.Backend.Infra.ArchiRuntime"],
    );
}

// ===========================================================================
// 模型面边界：结构诊断 / 空模型 / 前向防御
// ===========================================================================

/// 异常（AC-2 / spec Scenario 结构诊断非空即 Err）：成环 / 孤儿层级 / 未定义引用
/// 任一命中即 `Err`，文案含「模型结构错」与诊断条数，且不返回部分树。
#[test]
fn 结构诊断非空即错误且不给部分树() {
    let cases: [(&str, &str, &str); 3] = [
        (
            "cycle",
            concat!(
                "[element.\"A\"]\nkind = \"module\"\n\n",
                "[element.\"B\"]\nkind = \"module\"\n\n",
                "[[relation]]\nfrom = \"A\"\nto = \"B\"\n\n",
                "[[relation]]\nfrom = \"B\"\nto = \"A\"\n",
            ),
            "成环",
        ),
        (
            "orphan",
            "[element.\"App.Auth\"]\nkind = \"module\"\n",
            "孤儿层级（祖先未声明）",
        ),
        (
            "unknown-reference",
            concat!(
                "[element.\"A\"]\nkind = \"module\"\n\n",
                "[[relation]]\nfrom = \"A\"\nto = \"Ghost\"\n",
            ),
            "关系引用未声明元素",
        ),
    ];

    for (tag, source, why) in cases {
        let temp = TempDir::new(&format!("diag-{tag}"));
        let dir = archi_models_dir(temp.root());
        fs::create_dir_all(&dir).expect("建模型目录");
        fs::write(dir.join("01-test.toml"), source).expect("写模型");
        temp.write("src/a.ts", "export const a = 1;\n");

        let error = file_tree(temp.root()).expect_err(&format!("{why} 应 Err"));
        assert!(error.contains("模型结构错"), "{why} 文案形：{error}");
        assert!(error.contains("共 1 条"), "{why} 文案含诊断条数：{error}");
    }
}

/// 边界（AC-2 / spec Scenario 空模型返回全 none）：模型目录缺席与「目录在但零
/// `.toml`」两形态均 `Ok`，条目照常由磁盘枚举得出且全 `none`。
#[test]
fn 空模型返回全无主条目而非错误() {
    for tag in ["models-absent", "models-empty"] {
        let temp = TempDir::new(tag);
        if tag == "models-empty" {
            fs::create_dir_all(archi_models_dir(temp.root())).expect("建空模型目录");
        }
        temp.write("src/a.ts", "export const a = 1;\n");
        temp.write("src/deep/b.rs", "pub fn b() {}\n");

        let tree = file_tree(temp.root()).expect("空模型是合法起点，不得 Err");
        assert!(!tree.entries.is_empty(), "条目仍由磁盘枚举得出");
        for entry in &tree.entries {
            assert_eq!(entry.mark, FileTreeMark::None, "{tag} 全条目无主");
            assert!(entry.owners.is_empty(), "{tag} 无主条目 owners 为空");
            assert_eq!(entry.contains, 0, "{tag} 无主条目后代计数为 0");
        }
        assert_entry(
            &tree,
            "src/a.ts",
            FileEntryKind::File,
            FileTreeMark::None,
            &[],
        );
    }
}

/// 边界（AC-10 / spec Scenario 声明但目录不存在仍出节点）：前向防御分支给出
/// `exists = false` 的目录条目，其祖先照常出现并计入后代数。
#[test]
fn 声明但目录不存在的元素根仍出节点且标记缺失() {
    let temp = TempDir::new("ghost-root");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Ghost", "module", &["./src/ghost/"]),
    ]);
    temp.write("src/a.ts", "export const a = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok（声明不因盘缺席而报错）");
    let ghost = assert_entry(
        &tree,
        "src/ghost",
        FileEntryKind::Dir,
        FileTreeMark::ElementRoot,
        &["App.Ghost"],
    );
    assert!(!ghost.exists, "声明了但目录不在盘 ⇒ exists = false");
    let parent = entry_of(&tree, "src");
    assert_eq!(
        parent.mark,
        FileTreeMark::ContainsElements,
        "父目录含该元素根"
    );
    assert_eq!(parent.contains, 1, "缺席的元素根同样计入后代数");
    assert!(parent.exists, "在盘目录存在性恒真");
}

/// 正向（AC-10 / spec Scenario 在盘目录 exists 为真）：`exists = false` 只属
/// 「`Dir` 且为模块根且不在盘」，其余条目恒真。
#[test]
fn 在盘条目存在性恒真() {
    let temp = TempDir::new("exists-true");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Core", "domain", &[]),
        ("App.Core.Archi", "module", &["./crates/core/archi/"]),
    ]);
    temp.write("crates/core/archi/src/lib.rs", "pub fn a() {}\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    for entry in &tree.entries {
        assert!(entry.exists, "条目 {:?} 应在盘（exists 恒真）", entry.path);
    }
}

// ===========================================================================
// 条目形状：path / name 派生、扁平、字节序、去重、幂等
// ===========================================================================

/// 边界（AC-3）：`path` 无前导 `./`、无尾 `/`、不产空 `path` 根条目；`name` 为
/// 末段（单段时等于 `path`）。
#[test]
fn 路径形态与末段名派生() {
    let temp = TempDir::new("path-shape");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Core", "domain", &[]),
        ("App.Core.Archi", "module", &["./crates/core/archi/"]),
    ]);
    temp.write("crates/core/archi/src/lib.rs", "pub fn a() {}\n");
    temp.write("top.ts", "export const t = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    for entry in &tree.entries {
        assert!(!entry.path.is_empty(), "不产空 path 根条目");
        assert!(!entry.path.starts_with("./"), "{:?} 无前导 ./", entry.path);
        assert!(!entry.path.ends_with('/'), "{:?} 无尾 /", entry.path);
        let expected = entry
            .path
            .rsplit_once('/')
            .map_or(entry.path.as_str(), |(_, last)| last);
        assert_eq!(entry.name, expected, "{:?} 的 name 为末段", entry.path);
    }
    assert_entry(
        &tree,
        "crates/core/archi",
        FileEntryKind::Dir,
        FileTreeMark::ElementRoot,
        &["App.Core.Archi"],
    );
    assert_eq!(
        entry_of(&tree, "crates/core/archi").name,
        "archi",
        "嵌套条目取末段"
    );
    assert_eq!(
        entry_of(&tree, "crates").name,
        "crates",
        "顶层条目 name 即 path"
    );
    assert_eq!(
        entry_of(&tree, "crates").path,
        "crates",
        "顶层条目 path 即单段名"
    );
}

/// 正向（AC-3 / spec Scenario 扁平条目按 path 字节序且幂等）：两次调用逐字节
/// 相同，`path` 序列单调不减，同一文件与同一目录各恰一条。
#[test]
fn 扁平条目按路径字节序且两次调用逐字节相同() {
    let temp = TempDir::new("ordering");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.A", "module", &["./src/b/"]),
        ("App.B", "module", &["./src/a/"]),
    ]);
    temp.write("src/a/x.ts", "export const a = 1;\n");
    temp.write("src/b/y.ts", "export const b = 1;\n");
    temp.write("src/c/z.ts", "export const c = 1;\n");

    let first = file_tree(temp.root()).expect("应 Ok");
    let second = file_tree(temp.root()).expect("应 Ok");
    assert_eq!(
        serde_json::to_value(&first).expect("序列化"),
        serde_json::to_value(&second).expect("序列化"),
        "同输入同输出（确定性）"
    );

    let paths = paths_of(&first);
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "entries 按 path 字节序");

    let mut deduped = paths.clone();
    deduped.dedup();
    assert_eq!(paths, deduped, "同一路径恰一条（挂表天然去重）");

    // 扁平：顶层结构只有 entries 一个键，条目自身无嵌套字段
    let value = serde_json::to_value(&first).expect("序列化");
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["entries"], "ArchiFileTree 单字段扁平");
    let first_entry = value["entries"][0].as_object().expect("条目对象");
    assert!(
        !first_entry.contains_key("children"),
        "DTO 无嵌套 children 字段：{:?}",
        first_entry.keys().collect::<Vec<_>>()
    );
}

// ===========================================================================
// 枚举面 = 漂移判定面（AC-8 / AC-9）
// ===========================================================================

/// 异常（AC-8 / spec Scenario 测试面不出现）：七条默认排除约定逐条不在树中，
/// 同目录的非测试源文件照常出现。
#[test]
fn 测试面文件与目录一律不在树中() {
    let temp = TempDir::new("test-face");
    temp.write_model(&[("App", "package", &[])]);
    let cases: [(&str, &str); 7] = [
        ("src/a.test.tsx", "export const a = 1;\n"),
        ("crates/core/archi/src/tree_test.rs", "pub fn a() {}\n"),
        ("src/b.spec.ts", "export const b = 1;\n"),
        ("src/__tests__/c.ts", "export const c = 1;\n"),
        ("crates/core/archi/tests/d.rs", "pub fn d() {}\n"),
        ("src/fixtures/e.ts", "export const e = 1;\n"),
        ("src/__mocks__/f.ts", "export const f = 1;\n"),
    ];
    for (path, content) in cases {
        temp.write(path, content);
    }
    temp.write("src/file-tree.tsx", "export const FileTree = 1;\n");
    temp.write("crates/core/archi/src/tree.rs", "pub fn a() {}\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    for (path, _) in cases {
        assert_absent(&tree, path);
    }
    assert_entry(
        &tree,
        "src/file-tree.tsx",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
    assert_entry(
        &tree,
        "crates/core/archi/src/tree.rs",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
}

/// 异常（AC-8 / AC-9 / spec Scenario 配置 exclude 命中项不出现）：架构域配置的
/// 排除条目连同其各级目录一并消失，同配置下普通源文件照常出现。
#[test]
fn 架构域配置排除项不出现() {
    let temp = TempDir::new("config-exclude");
    temp.write_model(&[("App", "package", &[])]);
    temp.write_config("{ \"exclude\": [\"openspec/**\"] }");
    temp.write(
        "openspec/changes/x/prototypes/index.tsx",
        "export const prototype = 1;\n",
    );
    temp.write("src/a.ts", "export const a = 1;\n");

    let tree = file_tree(temp.root()).expect("合法配置应 Ok");
    assert_absent(&tree, "openspec");
    assert_absent(&tree, "openspec/changes/x/prototypes/index.tsx");
    assert_entry(
        &tree,
        "src/a.ts",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
}

/// 异常（AC-8 / spec Scenario 构建 / 依赖产物树不出现）：六类剪枝形态全部缺席，
/// 同层正常源文件照常出现。
#[test]
fn 构建与依赖产物树及隐藏项不出现() {
    let temp = TempDir::new("pruned");
    temp.write_model(&[("App", "package", &[])]);
    let cases: [&str; 6] = [
        "target/debug/x.rs",
        "node_modules/a/index.js",
        "dist/y.ts",
        "build/z.ts",
        ".git/hooks/y.ts",
        ".hidden/h.ts",
    ];
    for path in cases {
        temp.write(path, "export const x = 1;\n");
    }
    temp.write("src/a.ts", "export const a = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    for path in cases {
        assert_absent(&tree, path);
    }
    assert_entry(
        &tree,
        "src/a.ts",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
}

/// 异常（AC-8）：`.gitignore` 命中项由遍历器跳过（不出现），未命中项照常出现。
#[test]
fn 版本控制忽略项不出现() {
    let temp = TempDir::new("gitignore");
    temp.write_model(&[("App", "package", &[])]);
    // `git_ignore` 语义要求观察根是仓库边界（`.git` 在场），否则 `.gitignore`
    // 整条不生效——故显式造空 `.git` 目录。
    fs::create_dir_all(temp.root().join(".git")).expect("建 .git 目录");
    temp.write(".gitignore", "ignored/**\n");
    temp.write("ignored/a.ts", "export const a = 1;\n");
    temp.write("src/b.ts", "export const b = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    assert_absent(&tree, "ignored");
    assert_entry(
        &tree,
        "src/b.ts",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
}

/// 异常（AC-8）：五扩展名之外的形态不进树（含无扩展名文件）。
#[test]
fn 非源文件扩展名不出现() {
    let temp = TempDir::new("non-source");
    temp.write_model(&[("App", "package", &[])]);
    for path in ["src/a.py", "src/b.md", "src/c.json", "src/d.toml", "src/e"] {
        temp.write(path, "内容\n");
    }
    temp.write("src/a.rs", "pub fn a() {}\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    for path in ["src/a.py", "src/b.md", "src/c.json", "src/d.toml", "src/e"] {
        assert_absent(&tree, path);
    }
    assert_entry(
        &tree,
        "src/a.rs",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
}

/// 异常（AC-8 / spec Scenario 非法配置使树进错误态）：无通配符 / 空白条目 /
/// `Glob::new` 失败三形态一律 `Err`，文案沿用既有显式口径。
#[test]
fn 非法架构域配置使取数失败() {
    let cases: [(&str, &str, &str); 3] = [
        (
            "no-wildcard",
            "{ \"exclude\": [\"src/legacy\"] }",
            "不是文件级 glob",
        ),
        ("blank", "{ \"exclude\": [\"  \"] }", "空白条目"),
        ("bad-glob", "{ \"exclude\": [\"[\"] }", "排除规则非法"),
    ];
    for (tag, config, needle) in cases {
        let temp = TempDir::new(&format!("bad-config-{tag}"));
        temp.write_model(&[("App", "package", &[])]);
        temp.write_config(config);
        temp.write("src/a.ts", "export const a = 1;\n");

        let error = file_tree(temp.root()).expect_err(&format!("{tag} 配置应 Err"));
        assert!(error.contains(needle), "{tag} 文案应收 {needle:?}：{error}");
        if tag == "no-wildcard" {
            assert!(error.contains("/**"), "文案提示 `/**` 形态：{error}");
        }
    }
}

/// 异常 / 正向（AC-9 / R-3）：配置带 `$schema` 键即 `Err`（严格键面）；不带该键
/// 的等价形态 `Ok` 且排除生效。
#[test]
fn 配置含模式键即失败而合法配置生效() {
    let rejected = TempDir::new("schema-key");
    rejected.write_model(&[("App", "package", &[])]);
    rejected.write_config("{ \"$schema\": \"https://example.com/schema.json\", \"exclude\": [] }");
    rejected.write("src/a.ts", "export const a = 1;\n");
    let error = file_tree(rejected.root()).expect_err("带 $schema 的配置应 Err");
    assert!(
        error.contains("unknown field"),
        "文案沿用 serde 严格键面：{error}"
    );

    let accepted = TempDir::new("no-schema-key");
    accepted.write_model(&[("App", "package", &[])]);
    accepted.write_config("{ \"exclude\": [\"openspec/**\"] }");
    accepted.write("openspec/x.ts", "export const x = 1;\n");
    accepted.write("src/a.ts", "export const a = 1;\n");
    let tree = file_tree(accepted.root()).expect("无 $schema 的等价形态应 Ok");
    assert_absent(&tree, "openspec");
    assert_entry(
        &tree,
        "src/a.ts",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
}

/// 边界（AC-8 / D9）：`ignore` 键只服务元素对豁免，不参与文件树——条目集合与
/// 标记逐字节不变，且配置本身合法（不 `Err`）。
#[test]
fn 豁免键不参与文件树() {
    let plain = TempDir::new("ignore-key-plain");
    plain.write_model(&[
        ("App", "package", &[]),
        ("App.A", "module", &["./src/a/"]),
        ("App.B", "module", &["./src/b/"]),
    ]);
    plain.write("src/a/x.ts", "export const a = 1;\n");
    plain.write("src/b/y.ts", "export const b = 1;\n");

    let with_ignore = TempDir::new("ignore-key-set");
    with_ignore.write_model(&[
        ("App", "package", &[]),
        ("App.A", "module", &["./src/a/"]),
        ("App.B", "module", &["./src/b/"]),
    ]);
    with_ignore.write("src/a/x.ts", "export const a = 1;\n");
    with_ignore.write("src/b/y.ts", "export const b = 1;\n");
    with_ignore.write_config(
        "{ \"ignore\": [{ \"from\": \"App.A\", \"to\": \"App.B\", \"reason\": \"测试豁免\" }] }",
    );

    let left = file_tree(plain.root()).expect("无配置应 Ok");
    let right = file_tree(with_ignore.root()).expect("豁免配置合法，应 Ok");
    assert_eq!(
        serde_json::to_value(&left).expect("序列化"),
        serde_json::to_value(&right).expect("序列化"),
        "ignore 键不参与树（条目与标记零变化）"
    );
}

/// 边界（AC-12 数据侧）：模型非空但仓库内零符合扩展名的源文件 → `Ok`，条目只剩
/// 模块根合成的目录（不在盘即 `exists = false`）。
#[test]
fn 零进树源文件时只余合成目录() {
    let temp = TempDir::new("no-source");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Ghost", "module", &["./src/ghost/"]),
    ]);
    temp.write("src/a.py", "print(1)\n");

    let tree = file_tree(temp.root()).expect("零源文件不 Err");
    assert_eq!(paths_of(&tree), vec!["src", "src/ghost"], "只剩合成目录");
    assert_entry(
        &tree,
        "src",
        FileEntryKind::Dir,
        FileTreeMark::ContainsElements,
        &[],
    );
    let ghost = assert_entry(
        &tree,
        "src/ghost",
        FileEntryKind::Dir,
        FileTreeMark::ElementRoot,
        &["App.Ghost"],
    );
    assert!(!ghost.exists, "不在盘的声明根 ⇒ exists = false");
}

// ===========================================================================
// 纪律面：只读零写盘 / 零 spawn / 判定单源（源码自证）
// ===========================================================================

/// 边界（AC-1 / AC-8 / spec Scenario 只读零写盘）：调用前后磁盘字节快照相等，
/// 且源码零写盘 / 零子进程字面量。
#[test]
fn 取数只读零写盘且零子进程() {
    let temp = TempDir::new("read-only");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Core", "domain", &[]),
        ("App.Core.Archi", "module", &["./crates/core/archi/"]),
    ]);
    temp.write_config("{ \"exclude\": [\"openspec/**\"] }");
    temp.write("crates/core/archi/src/lib.rs", "pub fn a() {}\n");

    let before = temp.snapshot();
    let tree = file_tree(temp.root()).expect("应 Ok");
    let after = temp.snapshot();
    assert!(!tree.entries.is_empty(), "语料非空");
    assert_eq!(before, after, "取数全程零写盘（含临时文件零残留）");

    let source = include_str!("tree.rs");
    for forbidden in ["fs::write", "fs::remove", "Command::new", "std::process"] {
        assert!(
            !source.contains(forbidden),
            "`tree.rs` 命中禁用字面量 {forbidden:?}（只读 / 零 spawn 纪律）"
        );
    }
}

/// 异常（AC-6）：判定单源——`tree.rs` 零前缀匹配 / 零归一 / 零剥离 / 零自建遍历，
/// 且公共入口恰一条。
#[test]
fn 归属与枚举判据单源() {
    let source = include_str!("tree.rs");
    for forbidden in [
        "starts_with",
        "replace(",
        ".i.",
        "WalkBuilder",
        "read_dir",
        "fn is_source_file",
    ] {
        assert!(
            !source.contains(forbidden),
            "`tree.rs` 命中禁用字面量 {forbidden:?}（第二份判据即缺陷）"
        );
    }
    assert_eq!(
        source.matches("pub fn ").count(),
        1,
        "唯一公共入口（其余全私有）"
    );
    assert!(
        source.contains("pub fn file_tree("),
        "公共入口即 `file_tree`"
    );
    assert!(
        source.contains("crate::scan::owner::Owner") && source.contains("module_roots"),
        "归属一律经 `OwnerIndex`（含模块根表）"
    );
}

// ===========================================================================
// 词表可达性与优先级（AC-4）
// ===========================================================================

/// 正向（AC-4 / spec Scenario 五值各渲其标记）：一份语料同时给出五种标记，
/// 每值各取其条目并核对 `owners` 维度。
#[test]
fn 五种标记在树上逐值可达() {
    let temp = TempDir::new("five-marks");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Mods", "domain", &[]),
        ("App.Mods.A", "module", &["./src/mods/a/"]),
        ("App.Shared", "domain", &[]),
        ("App.Shared.A", "module", &["./src/shared/"]),
        ("App.Shared.B", "module", &["src/shared"]),
    ]);
    temp.write("src/mods/a/x.ts", "export const x = 1;\n");
    temp.write("src/shared/y.ts", "export const y = 1;\n");
    temp.write("other/loose.ts", "export const loose = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    let cases: [(&str, FileTreeMark, usize); 5] = [
        // 元素根目录
        ("src/mods/a", FileTreeMark::ElementRoot, 1),
        // 同路径多元素声明
        ("src/shared", FileTreeMark::Ambiguous, 2),
        // 元素根内部文件
        ("src/mods/a/x.ts", FileTreeMark::Member, 1),
        // 含元素根的祖先目录
        ("src", FileTreeMark::ContainsElements, 0),
        // 无主文件
        ("other/loose.ts", FileTreeMark::None, 0),
    ];
    for (path, mark, owner_count) in cases {
        let entry = entry_of(&tree, path);
        assert_eq!(entry.mark, mark, "{path:?} 的标记");
        assert_eq!(entry.owners.len(), owner_count, "{path:?} 的 owners 维度");
    }
    // 五个标记词逐一出现在本次语料（不是只覆盖部分分支）
    let marks: Vec<FileTreeMark> = tree.entries.iter().map(|entry| entry.mark).collect();
    for mark in FileTreeMark::ALL {
        assert!(
            marks.contains(&mark),
            "标记 {mark:?} 在本语料中不可达（分支未覆盖）"
        );
    }
}

/// 边界（AC-4 / spec Scenario 优先级单值）：节点既是元素根又在父元素路径内部 →
/// 取 `elementRoot`（父元素不额外给出该节点的 `member` 表达）。
#[test]
fn 归属优先级取单值() {
    let temp = TempDir::new("priority");
    temp.write_model(&[
        ("App", "package", &[]),
        ("App.Upper", "module", &["./src/"]),
        ("App.Upper.Child", "component", &["./src/child/"]),
    ]);
    temp.write("src/child/x.ts", "export const x = 1;\n");

    let tree = file_tree(temp.root()).expect("应 Ok");
    let child = assert_entry(
        &tree,
        "src/child",
        FileEntryKind::Dir,
        FileTreeMark::ElementRoot,
        &["App.Upper.Child"],
    );
    assert_eq!(child.owners.len(), 1, "优先级单值：只给元素根归属");
    // 子元素根内部文件归子元素（最深前缀），不出现父元素的并列表达
    let inner = assert_entry(
        &tree,
        "src/child/x.ts",
        FileEntryKind::File,
        FileTreeMark::Member,
        &["App.Upper.Child"],
    );
    assert_eq!(inner.owners.len(), 1, "归属单值");
    assert_eq!(
        entry_of(&tree, "src").mark,
        FileTreeMark::ElementRoot,
        "父元素根自身仍为 elementRoot"
    );
    assert_eq!(
        entry_of(&tree, "src").contains,
        1,
        "子元素根计入父元素后代数"
    );
}

// ===========================================================================
// 参数边界（`root: &Path`）
// ===========================================================================

/// 边界：空 `Path` / 不存在路径 / 指向文件 / 超长 / 含空格与非 ASCII 的 `root`
/// 一律落在 `Result` 内不 panic；含空格与非 ASCII 的合法根照常取数。
#[test]
fn 根路径非法与字符串边界形态不panic() {
    let temp = TempDir::new("root-bounds");
    temp.write("not-a-dir.txt", "我是文件\n");

    // 空 `Path`：模型目录派生指向相对路径，结果随进程 cwd 变化——只钉「落在 Result
    // 内、不 panic」；`Ok` 时条目仍须满足路径形态纪律（不因相对根破形）。
    match file_tree(Path::new("")) {
        Ok(tree) => {
            for entry in &tree.entries {
                assert!(!entry.path.is_empty(), "空 Path 下仍不产空 path 条目");
                assert!(!entry.path.starts_with("./"), "{:?} 无前导 ./", entry.path);
            }
        }
        Err(error) => assert!(!error.is_empty(), "错误文案非空"),
    }

    let file_root = temp.root().join("not-a-dir.txt");
    let missing = temp.root().join("no-such-dir");
    let long = temp.root().join("a".repeat(1200));
    for root in [file_root.as_path(), missing.as_path(), long.as_path()] {
        // 不存在 / 指向文件 / 超长：模型面与枚举面皆空 ⇒ 空树（不 panic）
        let tree = file_tree(root).expect("非法 root 不应 panic");
        assert!(tree.entries.is_empty(), "{root:?} 无进树源文件 ⇒ 空树");
    }

    // 合法但形态特殊的根：空格 / 非 ASCII / emoji 均可正常取数
    let exotic = temp.root().join("空 格🚀");
    let models = archi_models_dir(&exotic);
    fs::create_dir_all(&models).expect("建特殊名根");
    fs::write(
        models.join("01-test.toml"),
        "[element.\"App\"]\nkind = \"package\"\n",
    )
    .expect("写模型");
    let source = exotic.join("src");
    fs::create_dir_all(&source).expect("建源目录");
    fs::write(source.join("a.ts"), "export const a = 1;\n").expect("写源文件");

    let tree = file_tree(&exotic).expect("特殊名根应 Ok");
    assert_entry(
        &tree,
        "src/a.ts",
        FileEntryKind::File,
        FileTreeMark::None,
        &[],
    );
}

// ===========================================================================
// 规模与确定性（AC-15 的可判半边）
// ===========================================================================

/// 边界：300 文件 + 20 元素根的合成语料——条目覆盖全部文件、`path` 单调不减、
/// 两次调用逐字节相同、不抛错。
#[test]
fn 超大语料的规模与确定性() {
    let temp = TempDir::new("scale");
    let mut source = String::from("[element.\"App\"]\nkind = \"package\"\n\n");
    for index in 0..20 {
        source.push_str(&format!(
            "[element.\"App.M{index:02}\"]\nkind = \"module\"\npath = [\"./src/m{index:02}/\"]\n\n"
        ));
    }
    let dir = archi_models_dir(temp.root());
    fs::create_dir_all(&dir).expect("建模型目录");
    fs::write(dir.join("01-test.toml"), source).expect("写模型");

    let mut expected: Vec<(String, String)> = Vec::new();
    for index in 0..20 {
        for file in 0..15 {
            let path = format!("src/m{index:02}/f{file:02}.ts");
            temp.write(&path, "export const x = 1;\n");
            expected.push((path, format!("App.M{index:02}")));
        }
    }

    let tree = file_tree(temp.root()).expect("规模语料应 Ok");
    assert!(
        tree.entries.len() > expected.len(),
        "条目数应包含 300 文件与合成目录（实际 {}）",
        tree.entries.len()
    );
    for (path, fqn) in &expected {
        let entry = assert_entry(
            &tree,
            path,
            FileEntryKind::File,
            FileTreeMark::Member,
            &[fqn.as_str()],
        );
        assert!(entry.exists, "{path:?} 在盘");
    }
    let paths = paths_of(&tree);
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "规模下仍按 path 字节序");

    let again = file_tree(temp.root()).expect("应 Ok");
    assert_eq!(
        serde_json::to_value(&tree).expect("序列化"),
        serde_json::to_value(&again).expect("序列化"),
        "规模下两次调用逐字节相同"
    );
}
