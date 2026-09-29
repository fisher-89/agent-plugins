//! `commands::stats` 的单元测试：`code_stats` 无状态薄包装 + `code_stats_inner`
//! 领域组装纯函数（AC-3 / AC-6）。
//!
//! `#[tauri::command]` 保留原函数可直调，测试不启动 Tauri runtime（queries
//! mod_test 先例）。文件系统不 mock：以真实 tempdir fixture（`std::env::temp_dir`
//! + RAII 清理）承载多语言 / 嵌套 / 空目录 / 无效 root；tokei 作为被测函数内部
//! 真实依赖消费，其计数 / 识别 / ignore 语义不在断言范围——仅断言自研组装层
//! （同源自洽求和、排序键、深度截断、前缀聚合、Err 通道）。

use std::fs;
use std::path::{Path, PathBuf};

use super::{code_stats, code_stats_inner, DirNode, TreeEntry};

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "desktop-app-stats-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn ensure_dir(&self) {
        fs::create_dir_all(&self.0).expect("创建临时根目录失败");
    }

    /// 相对 root 写 fixture 文件（自动建父目录）。
    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("相对路径应有父目录")).expect("建父目录失败");
        fs::write(path, content).expect("写 fixture 文件失败");
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 多语言 fixture：`.rs` / `.ts` / `.md` 三扩展名四文件，行规模各异
/// （绝对行数不进入断言——计数语义归 tokei，断言只用组装不变量）。
fn seed_multi_language(ws: &TempWs) {
    ws.write(
        "src/main.rs",
        "// 入口注释\nfn main() {\n    let x = 1;\n\n    println!(\"{x}\");\n}\n",
    );
    ws.write(
        "src/util.ts",
        "// 工具注释\nexport function double(n: number): number {\n\n  return n * 2;\n}\n",
    );
    ws.write("README.md", "# 标题\n\n正文一行。\n");
    ws.write("docs/guide.md", "# 指南\n\n- 条目一\n- 条目二\n");
}

/// 嵌套 fixture：根层直属文件 + 两层目录链（树面截断 / 聚合用例共用）。
fn seed_nested(ws: &TempWs) {
    ws.write("top.rs", "fn top() {}\n");
    ws.write("src/a.rs", "fn a() {}\n");
    ws.write("src/deep/b.rs", "fn b() {}\n");
}

/// 树条目的末段名（目录 / 文件通用，排序与排除断言用）。
fn entry_name(entry: &TreeEntry) -> &str {
    match entry {
        TreeEntry::Dir { node } => &node.name,
        TreeEntry::File { node } => &node.name,
    }
}

/// 顶层**目录节点** files 之和（父含子的子树全量口径：顶层节点已含全部后代，
/// 不再递归；文件叶不计——目录聚合不含根层直属文件）。
fn sum_files(entries: &[TreeEntry]) -> u64 {
    entries
        .iter()
        .filter_map(|entry| match entry {
            TreeEntry::Dir { node } => Some(node.files),
            TreeEntry::File { .. } => None,
        })
        .sum()
}

// ---------------------------------------------------------------------------
// code_stats：薄包装结构证明（不启动 runtime 的可直调性）
// ---------------------------------------------------------------------------

#[test]
fn code_stats命令与inner纯函数结果逐字段一致_命令层零加工() {
    let ws = TempWs::new("wrapper-parity");
    seed_multi_language(&ws);
    let root = ws.root().to_string_lossy().into_owned();

    let via_command = code_stats(root.clone(), 5);
    let via_inner = code_stats_inner(Path::new(&root), 5);

    // 经 serde 序列化对比：命令层除参数转换（String → &Path）与 Err 透传外零加工
    let a = serde_json::to_value(&via_command).expect("命令结果序列化失败");
    let b = serde_json::to_value(&via_inner).expect("inner 结果序列化失败");
    assert_eq!(a, b, "命令层与纯函数对同一 tempdir 根结果逐字段一致");
    assert!(
        via_inner.expect("有效 root 应 Ok").totals.files >= 1,
        "前置：fixture 被解析出文件"
    );
}

#[test]
fn code_stats以string加u32原签名直调_无state薄包装结构证明() {
    let ws = TempWs::new("wrapper-direct");
    seed_multi_language(&ws);
    let root = ws.root().to_string_lossy().into_owned();

    // `#[tauri::command]` 保留 (String, u32) 原函数签名可直调、无 State 入参
    let report = code_stats(root, 5).expect("有效 root 应返回 Ok");
    assert!(report.totals.files >= 1);
    assert!(!report.languages.is_empty());

    // 无效 root 经命令层 Err 透传（错误映射三件事之末环）
    assert!(
        code_stats("肯定不存在的路径-直调".to_string(), 5).is_err(),
        "无效 root 的 Err 应经命令层透传"
    );
}

// ---------------------------------------------------------------------------
// code_stats_inner：root 有效性检查（Err 通道唯一来源）
// ---------------------------------------------------------------------------

#[test]
fn 不存在root路径返回err不panic() {
    let missing = std::env::temp_dir().join(format!(
        "desktop-app-stats-test-{}-不存在",
        std::process::id()
    ));

    let result = code_stats_inner(&missing, 5);
    let err = result.expect_err("缺失 root 应返回 Err");
    assert!(
        err.contains("root 无效"),
        "错误串应出自前置 metadata 检查，实际 {err:?}"
    );
}

#[test]
fn root指向普通文件返回err() {
    let ws = TempWs::new("file-root");
    ws.write("plain.txt", "普通文本文件\n");

    let result = code_stats_inner(&ws.0.join("plain.txt"), 5);
    let err = result.expect_err("非目录 root 应返回 Err");
    assert!(err.contains("不是目录"), "错误串应指明非目录，实际 {err:?}");
}

#[test]
#[cfg(windows)]
fn 不可读root返回err_windows下以悬空junction使metadata失败() {
    // 实现期 fixture 定夺：提权环境下目录 ACL 收紧拦不住 fs::metadata（backup
    // semantics），改以悬空 junction（指向不存在目标）使 metadata 必然失败——
    // 被测属性不变：metadata 失败 → Err 通道，不 panic、不静默空报告。
    let ws = TempWs::new("unreadable-root");
    ws.ensure_dir();
    let link = ws.0.join("dangling");
    let mklink = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(ws.0.join("no-such-target"))
        .output()
        .expect("mklink 应可执行（cmd 内置）");
    assert!(mklink.status.success(), "创建悬空 junction 应成功");
    assert!(
        std::fs::metadata(&link).is_err(),
        "前置：悬空 junction 的 metadata 应失败"
    );

    let result = code_stats_inner(&link, 5);
    let err = result.expect_err("不可达 root 应返回 Err");
    assert!(
        err.contains("root 无效"),
        "错误串应出自前置 metadata 检查，实际 {err:?}"
    );
}

#[test]
fn 空白root返回err() {
    // blank root 与无效 root 同走 Err（与查询轨道的空结果语义反向，spec 裁定）
    let result = code_stats_inner(Path::new(""), 5);
    assert!(result.is_err(), "空串 root 应返回 Err 而非空报告");
}

#[test]
fn 超长不存在路径root返回err不panic() {
    let long = format!(
        "{}\\{}",
        std::env::temp_dir().display(),
        "very-long-segment-".repeat(80)
    );
    assert!(long.chars().count() > 1000, "前置：路径超长");
    assert!(!Path::new(&long).exists(), "前置：路径不存在");

    let result = code_stats_inner(Path::new(&long), 5);
    assert!(result.is_err(), "超长不存在路径应返回 Err 而非 panic");
}

// ---------------------------------------------------------------------------
// code_stats_inner：汇总面与语言面组装（多语言 tempdir fixture）
// ---------------------------------------------------------------------------

#[test]
fn 多语言fixture汇总面与语言面同源自洽且按code降序() {
    let ws = TempWs::new("multi-language");
    seed_multi_language(&ws);

    let report = code_stats_inner(ws.root(), 5).expect("有效 root 应返回 Ok");
    let rows = &report.languages;
    assert!(rows.len() >= 2, "多语言 fixture 应识别出至少两种语言");

    // 同源自洽不变量：totals 四项 == 各语言行合计
    assert_eq!(
        report.totals.files,
        rows.iter().map(|r| r.files).sum::<u64>()
    );
    assert_eq!(report.totals.code, rows.iter().map(|r| r.code).sum::<u64>());
    assert_eq!(
        report.totals.comments,
        rows.iter().map(|r| r.comments).sum::<u64>()
    );
    assert_eq!(
        report.totals.blanks,
        rows.iter().map(|r| r.blanks).sum::<u64>()
    );

    // 语言行按代码行降序（允许 tie 相邻）
    for pair in rows.windows(2) {
        assert!(
            pair[0].code >= pair[1].code,
            "语言行应按 code 降序，实际 {}={} 在前 {}={} 在后",
            pair[0].name,
            pair[0].code,
            pair[1].name,
            pair[1].code
        );
    }
}

#[test]
fn 深度为聚合截断参数_depth1与depth10的汇总语言面完全相等() {
    let ws = TempWs::new("depth-irrelevant");
    seed_multi_language(&ws);

    let shallow = code_stats_inner(ws.root(), 1).expect("depth=1 应返回 Ok");
    let deep = code_stats_inner(ws.root(), 10).expect("depth=10 应返回 Ok");

    // 遍历恒全量：深度只截断树面，汇总 / 语言面数字不随 depth 变化（PoC 留档）
    let a = serde_json::to_value(&shallow).expect("depth=1 序列化失败");
    let b = serde_json::to_value(&deep).expect("depth=10 序列化失败");
    assert_eq!(a["totals"], b["totals"], "汇总面与深度无关");
    assert_eq!(a["languages"], b["languages"], "语言面与深度无关");
}

#[test]
fn 两语言code相等的tie按语言名字典序() {
    let ws = TempWs::new("tie-order");
    // 各置一个无注释无空行的单语句文件，两语言 code 相等触发 tie 键
    ws.write("alpha.rs", "fn main() {}\n");
    ws.write("beta.py", "x = 1\n");

    let report = code_stats_inner(ws.root(), 5).expect("tie fixture 应返回 Ok");
    let rows = &report.languages;
    assert_eq!(rows.len(), 2, "前置：恰两语言");
    assert_eq!(rows[0].code, rows[1].code, "前置：两语言 code 相等（tie）");
    assert!(
        rows[0].name < rows[1].name,
        "tie 应按语言名字典序，实际 [{}, {}]",
        rows[0].name,
        rows[1].name
    );
}

#[test]
fn 仅注释与空行的语言_code为0_share为0() {
    let ws = TempWs::new("comments-only");
    ws.write(
        "only_comments.rs",
        "// 第一行注释\n// 第二行注释\n\n// 第三行注释\n",
    );

    let report = code_stats_inner(ws.root(), 5).expect("仅注释 fixture 应返回 Ok");

    // Σcode=0：该语言行在场但 code=0，share 口径为 0.0（非 NaN / 非缺失）
    assert_eq!(report.totals.code, 0, "仅注释与空行时 Σcode 为 0");
    assert!(!report.languages.is_empty(), "语言行应在场（非空报告面）");
    for row in &report.languages {
        assert_eq!(row.code, 0);
        assert_eq!(
            row.share, 0.0,
            "Σcode=0 时 share 应为 0.0，实际 {}",
            row.share
        );
    }
    assert_eq!(
        report.totals.files, 1,
        "仅注释文件仍计入文件数（每报告 +1）"
    );
}

#[test]
fn 目录存在但无被识别文件返回空report而非err() {
    // 空目录
    let empty = TempWs::new("empty-dir");
    empty.ensure_dir();
    let report = code_stats_inner(empty.root(), 5).expect("空目录应返回 Ok 而非 Err");
    assert_eq!(report.totals.files, 0);
    assert_eq!(report.totals.code, 0);
    assert_eq!(report.totals.comments, 0);
    assert_eq!(report.totals.blanks, 0);
    assert!(
        report.languages.is_empty(),
        "语言面为空数组（前端空态输入）"
    );
    assert!(report.tree.is_empty(), "树面为空数组");

    // 仅未知扩展名文件
    let unknown = TempWs::new("unknown-ext");
    unknown.write("data.xyz", "任意文本内容\n");
    let report = code_stats_inner(unknown.root(), 5).expect("仅未知扩展名应返回 Ok");
    assert_eq!(report.totals.files, 0);
    assert!(report.languages.is_empty());
    assert!(report.tree.is_empty());
}

#[test]
fn share值域0到100且与code降序单调一致_计数非负() {
    let ws = TempWs::new("share-invariants");
    seed_multi_language(&ws);

    let report = code_stats_inner(ws.root(), 5).expect("有效 root 应返回 Ok");
    for (index, row) in report.languages.iter().enumerate() {
        assert!(
            (0.0..=100.0).contains(&row.share),
            "share 应在 [0, 100]，实际 {}（{}）",
            row.share,
            row.name
        );
        if index > 0 {
            assert!(
                report.languages[index - 1].share >= row.share,
                "share 与 code 降序单调一致"
            );
        }
    }

    let value = serde_json::to_value(&report).expect("报告序列化失败");
    for key in ["files", "code", "comments", "blanks"] {
        assert!(
            value["totals"][key].as_u64().is_some(),
            "totals.{key} 应为非负整数"
        );
    }
    for row in value["languages"].as_array().expect("语言面为数组") {
        for key in ["files", "code", "comments", "blanks"] {
            assert!(row[key].as_u64().is_some(), "语言行 {key} 应为非负整数");
        }
    }
}

// ---------------------------------------------------------------------------
// code_stats_inner：openspec 目录排除（ignored_directories 通道接线）
// ---------------------------------------------------------------------------

#[test]
fn 根层openspec目录整棵排除_三面均无其文件() {
    let ws = TempWs::new("excl-root-openspec");
    ws.write("src/main.rs", "// 注释\nfn main() {}\n");
    ws.write("openspec/proposal.md", "# 提案\n\n正文。\n");
    ws.write("openspec/changes/desktop-x/spec.md", "# 规格\n\n场景一。\n");
    ws.write("openspec/changes/desktop-x/workflow.json", "{}\n");

    let report = code_stats_inner(ws.root(), 10).expect("有效 root 应返回 Ok");

    // 汇总面：仅 src/main.rs 一个被识别文件（openspec 下 md/json 均不计入）
    assert_eq!(report.totals.files, 1, "openspec 下文件不计入汇总");
    // 语言面：openspec 是唯一 markdown 来源时应无 Markdown 行（遍历层剪枝）
    assert!(
        report.languages.iter().all(|row| row.name != "Markdown"),
        "目录被剪枝的语言不产生行"
    );
    // 树面：无 openspec 节点
    assert!(
        report
            .tree
            .iter()
            .all(|node| entry_name(node) != "openspec"),
        "树面不得出现 openspec 节点"
    );
}

#[test]
fn 任意层级名为openspec的目录同样排除() {
    let ws = TempWs::new("excl-nested-openspec");
    ws.write("keep.rs", "fn keep() {}\n");
    ws.write("pkgs/openspec/lib.rs", "fn hidden() {}\n");

    let report = code_stats_inner(ws.root(), 10).expect("有效 root 应返回 Ok");

    assert_eq!(report.totals.files, 1, "深层 openspec 目录同样整棵排除");
    fn assert_no_openspec(entries: &[TreeEntry]) {
        for entry in entries {
            match entry {
                TreeEntry::Dir { node } => {
                    assert_ne!(node.name, "openspec", "任意层级的 openspec 均不出树");
                    assert_no_openspec(&node.children);
                }
                TreeEntry::File { node } => {
                    assert_ne!(node.name, "openspec", "openspec 内文件不出叶");
                }
            }
        }
    }
    assert_no_openspec(&report.tree);
}

// ---------------------------------------------------------------------------
// code_stats_inner：目录树组装（≤depth 前缀聚合、POSIX path）
// ---------------------------------------------------------------------------

#[test]
fn 嵌套fixture树面posix路径_末段名与children字典序() {
    let ws = TempWs::new("tree-posix");
    ws.write("src/Alpha/y.rs", "fn y() {}\n");
    ws.write("src/beta/x.rs", "fn x() {}\n");
    ws.write("src/中文目录/z.rs", "fn z() {}\n");

    let report = code_stats_inner(ws.root(), 10).expect("有效 root 应返回 Ok");
    let tree = &report.tree;
    assert_eq!(tree.len(), 1, "仅 src 一个顶层目录");
    let TreeEntry::Dir { node: src } = &tree[0] else {
        panic!("顶层条目应为目录节点");
    };
    assert_eq!(src.name, "src");
    assert_eq!(src.path, "src", "路径为相对 root 的 POSIX 路径");
    assert_eq!(
        src.children.iter().map(entry_name).collect::<Vec<_>>(),
        vec!["Alpha", "beta", "中文目录"],
        "children 按名字典序（字节序）"
    );
    for child in &src.children {
        let TreeEntry::Dir { node } = child else {
            panic!("fixture 下 src 直接子条目应全为目录");
        };
        assert_eq!(node.path, format!("src/{}", node.name), "子路径以 / 分隔");
        assert!(!node.path.contains('\\'), "路径不得含反斜杠");
        // 每个子目录 children 恰为其直属文件叶
        let [TreeEntry::File { node: file }] = &node.children[..] else {
            panic!("子目录 children 应恰为一个文件叶");
        };
        assert_eq!(file.path, format!("{}/{}", node.path, file.name));
        assert!(!file.path.contains('\\'), "文件叶路径同为 POSIX");
    }
}

#[test]
fn depth1树仅顶层一层且节点聚合为子树全量() {
    let ws = TempWs::new("tree-depth1");
    seed_nested(&ws);

    let shallow = code_stats_inner(ws.root(), 1).expect("depth=1 应返回 Ok");
    let deep = code_stats_inner(ws.root(), 10).expect("depth=10 应返回 Ok");

    // 顶层 = src 目录节点 + top.rs 根层文件叶（目录先于文件）
    assert_eq!(shallow.tree.len(), 2, "顶层 = 1 目录节点 + 1 根层文件叶");
    let TreeEntry::Dir { node: src } = &shallow.tree[0] else {
        panic!("首条目应为目录（目录先于文件）");
    };
    assert_eq!(src.path, "src");
    // 深层目录被截断不出节点：children 无目录条目，仅剩直属文件叶 a.rs
    assert_eq!(
        src.children.len(),
        1,
        "depth=1 下 src children 仅直属文件叶"
    );
    assert!(
        matches!(&src.children[0], TreeEntry::File { node } if node.path == "src/a.rs"),
        "第二级目录不出节点，deep/b.rs 深度外不出叶"
    );
    assert_eq!(
        src.files, 2,
        "深层文件行计入祖先顶层节点（a.rs + deep/b.rs）"
    );

    // 子树全量口径：父含子——depth=10 下 src 节点统计与 depth=1 完全一致
    let deep_src = deep
        .tree
        .iter()
        .find_map(|entry| match entry {
            TreeEntry::Dir { node } if node.path == "src" => Some(node),
            _ => None,
        })
        .expect("depth=10 树应含 src 节点");
    assert_eq!(deep_src.files, src.files);
    assert_eq!(deep_src.code, src.code);
    assert_eq!(deep_src.comments, src.comments);
    assert_eq!(deep_src.blanks, src.blanks);
}

#[test]
fn depth0树无目录节点_根层直属文件仍为顶层文件叶() {
    let ws = TempWs::new("tree-depth0");
    seed_nested(&ws);

    let truncated = code_stats_inner(ws.root(), 0).expect("depth=0 应返回 Ok");
    let reference = code_stats_inner(ws.root(), 10).expect("depth=10 应返回 Ok");

    // 截断到无目录层：目录节点全无，根层直属文件（目录级数 0 ≤ 0）仍出叶
    assert!(
        truncated
            .tree
            .iter()
            .all(|entry| matches!(entry, TreeEntry::File { .. })),
        "depth=0 树面无目录节点"
    );
    assert_eq!(truncated.tree.len(), 1, "仅 top.rs 一个根层文件叶");
    let a = serde_json::to_value(&truncated).expect("depth=0 序列化失败");
    let b = serde_json::to_value(&reference).expect("depth=10 序列化失败");
    assert_eq!(a["totals"], b["totals"], "汇总面不变");
    assert_eq!(a["languages"], b["languages"], "语言面不变");
}

#[test]
fn 根层直属文件为顶层文件叶_不计入任何目录节点聚合() {
    let ws = TempWs::new("tree-root-file");
    seed_nested(&ws);

    let report = code_stats_inner(ws.root(), 10).expect("有效 root 应返回 Ok");
    let tree = &report.tree;

    // 无虚拟根目录节点：top.rs 不产生目录节点、不计入任何目录节点聚合，
    // 但作为顶层文件叶呈现（目录先于文件）
    assert_eq!(tree.len(), 2, "src 目录节点 + top.rs 根层文件叶");
    let TreeEntry::Dir { node: src } = &tree[0] else {
        panic!("首条目应为目录（目录先于文件）");
    };
    assert_eq!(src.path, "src");
    assert_eq!(
        src.files, 2,
        "src 节点恰含其全部后代文件（a.rs + deep/b.rs）"
    );
    let deep = src
        .children
        .iter()
        .find_map(|entry| match entry {
            TreeEntry::Dir { node } if node.path == "src/deep" => Some(node),
            _ => None,
        })
        .expect("src/deep 节点在场");
    assert_eq!(deep.files, 1, "deep 节点恰含 b.rs");
    assert_eq!(sum_files(tree), 2, "目录节点聚合和不含根层直属文件");

    // 对照：总数含根层文件（3 = top.rs + a.rs + b.rs，每识别文件 +1）
    assert_eq!(report.totals.files, 3, "汇总面含根层直属文件，目录聚合不含");
}

#[test]
fn 五十个顶层子目录混合命名children字典序稳定全量呈现() {
    let ws = TempWs::new("many-dirs");
    ws.ensure_dir();
    let mut names: Vec<String> = Vec::new();
    for index in 0..20 {
        names.push(format!("dir-{index:02}"));
    }
    // 大小写混排改为不相交命名：Windows 路径大小写不敏感，仅大小写不同的两个
    // 名字会同指一个目录（dir-00 与 Dir-00 碰撞），fixture 需互异目录
    for index in 0..15 {
        names.push(format!("Alpha-{index:02}"));
    }
    for index in 0..15 {
        names.push(format!("目录-{index:02}"));
    }
    for name in &names {
        ws.write(&format!("{name}/a.rs"), "fn a() {}\n");
    }

    let report = code_stats_inner(ws.root(), 5).expect("50 目录 fixture 应返回 Ok");
    let tree = &report.tree;
    assert_eq!(tree.len(), 50, "50 个顶层子目录全量呈现");
    assert!(
        tree.windows(2)
            .all(|pair| entry_name(&pair[0]) <= entry_name(&pair[1])),
        "children（顶层）按 name 字典序稳定排列"
    );
    for entry in tree {
        let TreeEntry::Dir { node } = entry else {
            panic!("顶层应全为目录节点（fixture 无根层直属文件）");
        };
        assert_eq!(node.files, 1, "每目录节点恰含其一个后代文件");
        assert!(
            matches!(&node.children[..], [TreeEntry::File { node: file }] if file.name == "a.rs"),
            "children 恰为直属文件叶 a.rs"
        );
    }
    assert_eq!(
        sum_files(tree),
        report.totals.files,
        "无根层直属文件时顶层聚合和恰为总数"
    );
    assert_eq!(report.totals.files, 50);
}

#[test]
fn 含空格与中文名的目录_path出线保持原名() {
    let ws = TempWs::new("dir-names");
    ws.write("with space/a.rs", "fn a() {}\n");
    ws.write("带 空格 目录/子 目录/b.rs", "fn b() {}\n");

    let report = code_stats_inner(ws.root(), 10).expect("目录名 fixture 应返回 Ok");
    let tree = &report.tree;
    assert_eq!(tree.len(), 2, "两个顶层目录全量呈现");

    let top_dir = |path: &str| {
        tree.iter().find_map(|entry| match entry {
            TreeEntry::Dir { node } if node.path == path => Some(node),
            _ => None,
        })
    };
    let ascii = top_dir("with space").expect("空格目录在场");
    assert_eq!(ascii.name, "with space", "归一化不改名");

    let chinese = top_dir("带 空格 目录").expect("中文目录在场");
    assert_eq!(chinese.name, "带 空格 目录");
    let nested = chinese
        .children
        .iter()
        .find_map(|entry| match entry {
            TreeEntry::Dir { node } if node.path == "带 空格 目录/子 目录" => Some(node),
            _ => None,
        })
        .expect("嵌套中文目录 path 保持原名并以 / 分隔");
    assert_eq!(nested.name, "子 目录");

    // 归一化仅统一分隔符：全树（目录节点 + 文件叶）无反斜杠出线
    fn assert_posix(entries: &[TreeEntry]) {
        for entry in entries {
            match entry {
                TreeEntry::Dir { node } => {
                    assert!(
                        !node.path.contains('\\'),
                        "path 不得含反斜杠，实际 {:?}",
                        node.path
                    );
                    assert_posix(&node.children);
                }
                TreeEntry::File { node } => assert!(
                    !node.path.contains('\\'),
                    "path 不得含反斜杠，实际 {:?}",
                    node.path
                ),
            }
        }
    }
    assert_posix(tree);
}

// ---------------------------------------------------------------------------
// code_stats_inner：文件叶归属与排序（展开到文件层）
// ---------------------------------------------------------------------------

#[test]
fn 文件叶挂于父目录children_同父条目目录先于文件() {
    let ws = TempWs::new("leaf-order");
    // 同父混合：src 下既有子目录又有直属文件，根层另有直属文件与根层目录
    ws.write("root.rs", "fn r() {}\n");
    ws.write("src/adir/x.rs", "fn x() {}\n");
    ws.write("src/zeta.rs", "fn z() {}\n");
    ws.write("src/alpha.rs", "fn a() {}\n");

    let report = code_stats_inner(ws.root(), 10).expect("有效 root 应返回 Ok");

    // 顶层：目录组 [src] 字典序在前，文件组 [root.rs] 在后
    let top: Vec<(&str, bool)> = report
        .tree
        .iter()
        .map(|entry| (entry_name(entry), matches!(entry, TreeEntry::Dir { .. })))
        .collect();
    assert_eq!(
        top,
        vec![("src", true), ("root.rs", false)],
        "同父条目目录先于文件，各自按 name 字典序"
    );

    // src children：目录组 [adir] 在前，文件组 [alpha.rs, zeta.rs] 字典序在后
    let src = report
        .tree
        .iter()
        .find_map(|entry| match entry {
            TreeEntry::Dir { node } if node.path == "src" => Some(node),
            _ => None,
        })
        .expect("src 目录节点在场");
    let children: Vec<(&str, bool)> = src
        .children
        .iter()
        .map(|entry| (entry_name(entry), matches!(entry, TreeEntry::Dir { .. })))
        .collect();
    assert_eq!(
        children,
        vec![("adir", true), ("alpha.rs", false), ("zeta.rs", false)],
        "目录先于文件，文件叶按 name 字典序"
    );
}

#[test]
fn 深度外文件不出叶_仅并入最深可达祖先聚合() {
    let ws = TempWs::new("leaf-truncate");
    ws.write("src/a.rs", "fn a() {}\n");
    ws.write("src/deep/b.rs", "fn b() {}\n");

    let report = code_stats_inner(ws.root(), 1).expect("depth=1 应返回 Ok");

    let src = report
        .tree
        .iter()
        .find_map(|entry| match entry {
            TreeEntry::Dir { node } if node.path == "src" => Some(node),
            _ => None,
        })
        .expect("src（level 1 ≤ depth）节点在场");
    assert_eq!(src.files, 2, "深度外 b.rs 仍并入 src 子树聚合");
    let leaves: Vec<&str> = src
        .children
        .iter()
        .filter_map(|entry| match entry {
            TreeEntry::File { node } => Some(node.path.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        leaves,
        vec!["src/a.rs"],
        "仅父目录在深度内的文件出叶（src/deep 截断，b.rs 无叶）"
    );
}

#[test]
fn 文件叶行统计与目录聚合自洽_四项均叶加子目录() {
    let ws = TempWs::new("leaf-consistency");
    seed_nested(&ws);

    let report = code_stats_inner(ws.root(), 10).expect("有效 root 应返回 Ok");

    // 组装不变量（非 tokei 计数断言）：目录节点四项统计 = 直接文件叶合计 +
    // 子目录聚合之和——树面展开到文件层后逐节点自洽
    fn assert_consistent(node: &DirNode) {
        let leaves: Vec<_> = node
            .children
            .iter()
            .filter_map(|entry| match entry {
                TreeEntry::File { node } => Some((1u64, node.code, node.comments, node.blanks)),
                _ => None,
            })
            .collect();
        let dirs: Vec<_> = node
            .children
            .iter()
            .filter_map(|entry| match entry {
                TreeEntry::Dir { node } => {
                    Some((node.files, node.code, node.comments, node.blanks))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            node.files,
            leaves.iter().map(|it| it.0).sum::<u64>() + dirs.iter().map(|it| it.0).sum::<u64>(),
            "{} 的 files = 直接叶 + 子目录聚合",
            node.path
        );
        assert_eq!(
            node.code,
            leaves.iter().map(|it| it.1).sum::<u64>() + dirs.iter().map(|it| it.1).sum::<u64>(),
            "{} 的 code = 直接叶 + 子目录聚合",
            node.path
        );
        assert_eq!(
            node.comments,
            leaves.iter().map(|it| it.2).sum::<u64>() + dirs.iter().map(|it| it.2).sum::<u64>(),
            "{} 的 comments 自洽",
            node.path
        );
        assert_eq!(
            node.blanks,
            leaves.iter().map(|it| it.3).sum::<u64>() + dirs.iter().map(|it| it.3).sum::<u64>(),
            "{} 的 blanks 自洽",
            node.path
        );
        for child in &node.children {
            if let TreeEntry::Dir { node } = child {
                assert_consistent(node);
            }
        }
    }
    for entry in &report.tree {
        if let TreeEntry::Dir { node } = entry {
            assert_consistent(node);
        }
    }
}
