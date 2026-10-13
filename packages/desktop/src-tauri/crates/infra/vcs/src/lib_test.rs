//! vcs crate 根（lib.rs）的单元测试（test-design「lib.rs -> lib_test.rs」节）：
//! worktree 落位派生单点——`data_root/worktrees/{身份段}/<change>` 消费
//! foundation 身份段（同源单点直调对拍）；同根可复现 / 异根不撞；change 名
//! 零清洗零截断（清洗权威在 create 前置，本函数零校验的负向锚）；目录名常
//! 量与无状态装配入口。纯路径推导内存直驱（tempfile 仅取路径字符串，零 git
//! 零 spawn）。

use std::path::{Path, PathBuf};

use super::{worktree_dir, ProcessWorktree, WORKTREES_DIR_NAME};

/// 落位形态：`data_root/worktrees/{workspace_identity_segment(ws_root)}/{change}`
/// ——第三级与 foundation 单点同源（直调对拍）；`WORKTREES_DIR_NAME == "worktrees"`。
#[test]
fn 落位形态_身份段父目录与foundation单点同源对拍() {
    assert_eq!(
        WORKTREES_DIR_NAME, "worktrees",
        "worktrees 子树目录名单点常量"
    );

    let data_root = PathBuf::from("C:\\app-data");
    for (ws_root, change) in [
        ("C:\\ws\\demo-alpha", "fix-bug"),
        ("/home/u/repo", "add-feature"),
        ("C:\\ws\\...", "edge-name"),
    ] {
        let placement = worktree_dir(&data_root, ws_root, change);
        let expected = data_root
            .join(WORKTREES_DIR_NAME)
            .join(foundation::identity::workspace_identity_segment(ws_root))
            .join(change);
        assert_eq!(
            placement, expected,
            "落位 = data_root/worktrees/{{身份段}}/<change>（{ws_root}）"
        );
    }
}

/// 可复现与隔离：同 (data_root, ws_root, change) 两次派生等值；异 ws_root 同
/// change → 不同身份段父目录（互不冲突互不可见）。
#[test]
fn 可复现与隔离_同根等值异根父目录互异() {
    let data_root = PathBuf::from("C:\\app-data");
    let first = worktree_dir(&data_root, "C:\\ws\\alpha", "fix-bug");
    let second = worktree_dir(&data_root, "C:\\ws\\alpha", "fix-bug");
    assert_eq!(
        first, second,
        "同 (data_root, ws_root, change) 派生确定可复现"
    );

    let other = worktree_dir(&data_root, "C:\\ws\\beta", "fix-bug");
    assert_ne!(
        first.parent(),
        other.parent(),
        "异 ws_root 同 change → 身份段父目录不同（互不冲突）"
    );
    assert_eq!(
        first.file_name(),
        other.file_name(),
        "change 名尾段不受 workspace 身份影响"
    );
}

/// change 名原样挂尾：不做任何清洗 / 截断（清洗权威在 create 前置——本函数
/// 零校验的负向锚；非法名经此原样透出，由 create 七道前置拒绝）。
#[test]
fn change名原样挂尾_零清洗零截断() {
    let data_root = Path::new("C:\\app-data");
    for raw in [
        "Fix-Bug",
        "fix_bug",
        "a/b",
        "fix--bug",
        "超长".repeat(64).as_str(),
    ] {
        let placement = worktree_dir(data_root, "C:\\ws\\demo", raw);
        assert!(
            placement.ends_with(Path::new(raw)),
            "change 名零清洗零截断原样挂尾（含多分量名原样透出）: {raw:?} -> {}",
            placement.display()
        );
    }
}

/// 无状态装配入口：`new()` 与 `Default::default()` 等值可重复构造（组合根
/// 按需铸的编译锚——零字段无状态，多次构造互不干扰）。
#[test]
fn processworktree构造_new与default等值() {
    let _a = ProcessWorktree::new();
    let _b = ProcessWorktree::default();
    let _c = ProcessWorktree::new();
    // 无状态空类型：可重复构造即断言面（字段零个，等值由类型层承载）
}

// ---------------------------------------------------------------------------
// ProcessArchiveVcs 清单 / 补丁两方法转发（desktop-fix-mutation-test / AC-3）：
// trait object 动态分发与 git.rs 自由函数逐字等值（同仓同 base 双跑对拍）；
// Err 原文透传不包装不吞；空清单 / 空补丁（干净仓）Ok 面透传
// ---------------------------------------------------------------------------

use orchestration::port::ArchiveVcsPort;

use super::ProcessArchiveVcs;

/// 装置：仓内直驱 git 助手（git_test 同式最小装置——本节转发对拍面）。
fn git(root: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("装置 git 拉起失败");
    assert!(
        output.status.success(),
        "装置 git {} 失败: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn init_repo(tag: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-lib-test-{tag}-"))
        .tempdir()
        .expect("创建仓临时目录失败");
    let root = dir.path().to_path_buf();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "fixture"]);
    git(&root, &["config", "user.email", "fixture@example.com"]);
    std::fs::write(root.join("a.txt"), "initial\n").expect("写初始文件");
    git(&root, &["add", "a.txt"]);
    git(
        &root,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-q",
            "-m",
            "init",
        ],
    );
    (dir, root)
}

/// 正向：经 trait object 动态分发，`change_edit_set` / `change_edit_patch` 结果
/// 与 `git.rs` 自由函数逐字等值（同仓同 base 双跑对拍——转发层薄实现真实执行）。
#[test]
fn process_archive_vcs清单与补丁转发与自由函数逐字等值() {
    let port = ProcessArchiveVcs::new();
    let (_dir, root) = init_repo("lib-forward");

    // 仓内制造编辑面：tracked 修改 + untracked 新建
    std::fs::write(root.join("a.txt"), "edited\n").expect("写修改");
    std::fs::write(root.join("fresh.rs"), "pub fn f() {}\n").expect("写新文件");

    // trait object 动态分发
    let dyn_port: &dyn ArchiveVcsPort = &port;
    let via_port_set = dyn_port
        .change_edit_set(&root, "HEAD")
        .expect("trait 面清单应成功");
    let via_port_patch = dyn_port
        .change_edit_patch(&root, "HEAD")
        .expect("trait 面补丁应成功");

    // 自由函数直调对拍（同仓同 base 双跑——补丁已在上一步 add -N，二次调用幂等）
    let via_free_set = super::git::change_edit_set(&root, "HEAD").expect("自由函数清单面");
    let via_free_patch = super::git::change_edit_patch(&root, "HEAD").expect("自由函数补丁面");

    assert_eq!(
        via_port_set, via_free_set,
        "trait 转发与自由函数清单逐字等值"
    );
    assert_eq!(
        via_port_patch, via_free_patch,
        "trait 转发与自由函数补丁逐字等值"
    );
    assert!(
        via_port_set.contains(&"a.txt".to_owned())
            && via_port_set.contains(&"fresh.rs".to_owned()),
        "编辑集两类文件在案: {via_port_set:?}"
    );
    assert!(
        via_port_patch.contains("diff --git"),
        "补丁形态保真（trait 转发零改写）"
    );
}

/// 异常：`git.rs` 侧 Err 原文透传（不包装不吞）——非 git 目录与非法 base 两面
/// 与自由函数同口径。
#[test]
fn process_archive_vcs清单与补丁err原文透传() {
    let port = ProcessArchiveVcs::new();

    // 非 git 目录：Err 透传（与自由函数同串——薄转发零包装）
    let plain = tempfile::Builder::new()
        .prefix("vcs-lib-test-plain-")
        .tempdir()
        .expect("创建普通目录失败");
    let via_port = port
        .change_edit_set(plain.path(), "HEAD")
        .expect_err("trait 面非 git 目录应 Err");
    let via_free = super::git::change_edit_set(plain.path(), "HEAD")
        .expect_err("自由函数非 git 目录应 Err");
    assert_eq!(via_port, via_free, "Err 原文透传不包装不吞");
    assert!(port.change_edit_patch(plain.path(), "HEAD").is_err());

    // 非法 base：同口径
    let (_dir, root) = init_repo("lib-err-base");
    let empty_error = port
        .change_edit_set(&root, "")
        .expect_err("空 base 应 Err");
    assert!(empty_error.contains("base 为空"), "同口径前置校验");
    assert!(port.change_edit_patch(&root, "").is_err());
}

/// 边界：干净仓空清单 / 空补丁（Ok 面）透传；`base = "HEAD"` 与非法 base 两态
/// 与 git.rs 同口径。
#[test]
fn process_archive_vcs干净仓空面透传() {
    let port = ProcessArchiveVcs::new();
    let (_dir, root) = init_repo("lib-clean");

    assert_eq!(
        port.change_edit_set(&root, "HEAD").expect("干净仓清单 Ok 面"),
        Vec::<String>::new(),
        "干净仓空清单透传（与 Err 严格区分）"
    );
    assert_eq!(
        port
            .change_edit_patch(&root, "HEAD")
            .expect("干净仓补丁 Ok 面"),
        "",
        "干净仓空补丁透传"
    );

    // 构造形态：new() 与 Default::default() 等价可用（无状态缝）
    let _default = ProcessArchiveVcs::default();
    assert!(
        _default.change_edit_set(&root, "HEAD").is_ok(),
        "Default 构造面同式可用"
    );
}

/// 组合链路（AC-3 全链唯一入口）：真实 `ProcessArchiveVcs` 取数（真实 git
/// tempdir：tracked 修改 + untracked 新建 + intent-to-add 补丁）→ fake
/// `MutationRunner` 收四参 `(root, name, files, patch)`——files 恰为 worktree
/// 编辑集、patch 恰为 unified diff（清单 / 补丁唯一入口，checks 侧零 git
/// spawn 的组合面）。
#[tokio::test]
async fn 组合链路_真实vcs取数注入fake_runner四参() {
    use orchestration::port::{BoxToolFuture, MutationRunner, ToolStepOutput};
    use orchestration::port::MutationCounts;
    use orchestration::port::MutationSkipReason;
    use orchestration::port::MutationTestConclusion;
    use orchestration::port::MutationTestOutcome;

    /// 记录四参的 fake MutationRunner（缝注入面）。
    struct RecordingMutationRunner {
        calls: std::sync::Mutex<Vec<(String, String, Vec<String>, String)>>,
    }
    impl MutationRunner for RecordingMutationRunner {
        fn run(
            &self,
            root: &str,
            name: &str,
            files: &[String],
            patch: &str,
        ) -> BoxToolFuture {
            self.calls
                .lock()
                .expect("锁不可中毒")
                .push((
                    root.to_owned(),
                    name.to_owned(),
                    files.to_vec(),
                    patch.to_owned(),
                ));
            Box::pin(async move {
                Ok(ToolStepOutput::MutationTest(MutationTestOutcome {
                    conclusion: MutationTestConclusion::Pass,
                    skip: Some(MutationSkipReason::EmptyDiff),
                    score: 0.0,
                    threshold: 0.0,
                    counts: MutationCounts::default(),
                    report_paths: Vec::new(),
                    feedback_text: String::new(),
                }))
            })
        }
    }

    // 真实 git tempdir：tracked 修改 + untracked 新建
    let vcs = ProcessArchiveVcs::new();
    let (_dir, root) = init_repo("combo-link");
    std::fs::write(root.join("a.txt"), "combo edit\n").expect("写 tracked 修改");
    std::fs::create_dir_all(root.join("src")).expect("建 src");
    std::fs::write(root.join("src").join("fresh.rs"), "pub fn f() {}\n").expect("写 untracked");

    // vcs 缝取数（steps.rs 消费点同式——base 恒 HEAD worktree 语义）
    let files = vcs
        .change_edit_set(&root, "HEAD")
        .expect("清单取数应成功");
    let patch = vcs
        .change_edit_patch(&root, "HEAD")
        .expect("补丁取数应成功");

    // 注入 runner 四参（组合面：清单 / 补丁唯一入口 → runner 零 git）
    let runner = RecordingMutationRunner {
        calls: std::sync::Mutex::new(Vec::new()),
    };
    let root_str = root.to_string_lossy().into_owned();
    runner
        .run(&root_str, "demo-change", &files, &patch)
        .await
        .expect("runner 应可用");

    let captured = runner.calls.lock().expect("锁").clone();
    assert_eq!(captured.len(), 1);
    let (_, name, runner_files, runner_patch) = &captured[0];
    assert_eq!(name, "demo-change", "name 逐字透传");
    assert_eq!(
        runner_files, &files,
        "files 恰为 worktree 编辑集（清单唯一入口——AC-3）"
    );
    assert!(
        runner_files.contains(&"a.txt".to_owned())
            && runner_files.contains(&"src/fresh.rs".to_owned()),
        "tracked 修改 + untracked 新建俱在案: {runner_files:?}"
    );
    assert_eq!(
        runner_patch, &patch,
        "patch 恰为 unified diff（--in-diff 补丁唯一入口）"
    );
    assert!(
        runner_patch.contains("diff --git a/a.txt b/a.txt")
            && runner_patch.contains("diff --git a/src/fresh.rs b/src/fresh.rs"),
        "多文件段头在案（段切片面原料）: {runner_patch}"
    );
}
