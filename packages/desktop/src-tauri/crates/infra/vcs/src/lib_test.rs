use std::path::{Path, PathBuf};

use super::{placement, worktree_root_from_toplevel, ProcessWorktree, WORKTREES_DIR_NAME};

/// 落位形态：`<git 根父目录>/.worktrees/<git 根目录名>`——与数据根 / 身份段
/// 解耦（不再叠加 hash 段）；`WORKTREES_DIR_NAME == ".worktrees"`。
#[test]
fn 落位形态_git根同级点worktrees子目录() {
    assert_eq!(
        WORKTREES_DIR_NAME, ".worktrees",
        "git 根同级子树目录名单点常量"
    );

    for toplevel in [
        Path::new("C:\\Projects\\wpsweb"),
        Path::new("/home/u/repo"),
        Path::new("relative-dir\\repo"),
    ] {
        let expected = toplevel
            .parent()
            .expect("夹具 toplevel 恒有父目录")
            .join(WORKTREES_DIR_NAME)
            .join(toplevel.file_name().expect("夹具 toplevel 恒有目录名"));
        assert_eq!(
            worktree_root_from_toplevel(toplevel).expect("落位派生应成功"),
            expected,
            "落位 = <git 根父目录>/.worktrees/<git 根目录名>（{}）",
            toplevel.display()
        );
    }
}

/// 可复现与隔离：同 git 根两次派生等值；异 git 根（同父目录 / 异父目录）落位
/// 互异。
#[test]
fn 可复现与隔离_同根等值异根互异() {
    let first =
        worktree_root_from_toplevel(Path::new("C:\\Projects\\alpha")).expect("首次派生应成功");
    let second =
        worktree_root_from_toplevel(Path::new("C:\\Projects\\alpha")).expect("再次派生应成功");
    assert_eq!(first, second, "同 git 根派生确定可复现");

    let sibling =
        worktree_root_from_toplevel(Path::new("C:\\Projects\\beta")).expect("异根派生应成功");
    assert_ne!(first, sibling, "同父目录异仓库目录名 → 落位互异");

    let elsewhere =
        worktree_root_from_toplevel(Path::new("D:\\Projects\\alpha")).expect("异盘派生应成功");
    assert_ne!(first, elsewhere, "异父目录同名仓库 → 落位互异");
}

/// change 名原样挂尾：不做任何清洗 / 截断（清洗权威在 create 前置——本函数
/// 零校验的负向锚；非法名经此原样透出，由 create 前置拒绝）。
#[test]
fn change名原样挂尾_零清洗零截断() {
    let anchor =
        worktree_root_from_toplevel(Path::new("C:\\Projects\\demo")).expect("落位派生应成功");
    for raw in [
        "Fix-Bug",
        "fix_bug",
        "a/b",
        "fix--bug",
        "超长".repeat(64).as_str(),
    ] {
        let placement = anchor.join(raw);
        assert!(
            placement.ends_with(Path::new(raw)),
            "change 名零清洗零截断原样挂尾（含多分量名原样透出）: {raw:?} -> {}",
            placement.display()
        );
    }
}

/// 派生异常：git 根无父目录（单分量相对路径）→ 显式 Err 引导。
#[test]
fn worktree_root_from_toplevel_无父目录err() {
    let error = worktree_root_from_toplevel(Path::new("demo")).expect_err("无父目录应 Err");
    assert!(
        error.contains("无父目录") && error.contains(".worktrees"),
        "Err 携派生语境，实际: {error}"
    );
}

/// 真实 git（正向）：workspace root = git 根 → 落位在 git 根同级的
/// `.worktrees/<仓库目录名>` 且偏移为空串；workspace root 为仓库子目录时仍锚
/// 定 git 根，偏移为（`/` 分隔、无尾分隔符的）相对路径。
#[test]
fn placement_真实git_子目录workspace仍锚定git根() {
    let (base, repo) = init_repo("placement");
    let expected = base.path().join(WORKTREES_DIR_NAME).join("repo");

    let root_workspace = placement(&repo).expect("git 根 workspace 派生应成功");
    assert_eq!(
        root_workspace.root, expected,
        "落位 = <git 根父目录>/.worktrees/<仓库目录名>"
    );
    assert_eq!(root_workspace.rel, "", "git 根 workspace 偏移为空串");

    let nested = repo.join("client").join("app");
    std::fs::create_dir_all(&nested).expect("建仓库子目录失败");
    let nested_workspace = placement(&nested).expect("子目录 workspace 派生应成功");
    assert_eq!(
        nested_workspace.root, expected,
        "workspace 为仓库子目录时仍以 git 根为锚"
    );
    assert_eq!(
        nested_workspace.rel, "client/app",
        "偏移 = workspace root 相对 git 根（`/` 分隔、无尾分隔符）"
    );
    // 分隔符一致性（回归）：git 恒以 `/` 输出 toplevel；落位串须转本机分隔符，
    // 否则 Windows 上出现 `C:/x\.worktrees\…` 混式（用户可见路径）
    let foreign = if std::path::MAIN_SEPARATOR == '/' {
        '\\'
    } else {
        '/'
    };
    let root_text = nested_workspace.root.to_string_lossy();
    assert!(
        !root_text.contains(foreign),
        "落位串仅含本机分隔符，实际: {root_text}"
    );
}

/// 真实 git（异常）：非 git 目录 → 显式 Err（引导文案与 create probe 同口径）。
#[test]
fn placement_非git目录err() {
    let plain = tempfile::Builder::new()
        .prefix("vcs-lib-placement-plain-")
        .tempdir()
        .expect("创建普通目录失败");
    let error = placement(plain.path()).expect_err("非 git 目录应 Err");
    assert!(
        error.contains("非 git 仓"),
        "Err 携非 git 仓引导，实际: {error}"
    );
}

/// 无状态装配入口：`new()` 与 `Default::default()` 等值可重复构造（组合根
/// 按需铸的编译锚——零字段无状态，多次构造互不干扰）。
#[test]
fn processworktree构造_new与default等值() {
    let _a = ProcessWorktree::new();
    let _b = ProcessWorktree;
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
    // 仓落父 tempdir 的 `repo` 子目录：worktree 新落位在 git 根同级
    // （`.worktrees/repo`），随父 tempdir 一并回收。
    let root = dir.path().join("repo");
    std::fs::create_dir_all(&root).expect("创建仓目录失败");
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
        via_port_set.contains(&"a.txt".to_owned()) && via_port_set.contains(&"fresh.rs".to_owned()),
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
    let via_free =
        super::git::change_edit_set(plain.path(), "HEAD").expect_err("自由函数非 git 目录应 Err");
    assert_eq!(via_port, via_free, "Err 原文透传不包装不吞");
    assert!(port.change_edit_patch(plain.path(), "HEAD").is_err());

    // 非法 base：同口径
    let (_dir, root) = init_repo("lib-err-base");
    let empty_error = port.change_edit_set(&root, "").expect_err("空 base 应 Err");
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
        port.change_edit_set(&root, "HEAD")
            .expect("干净仓清单 Ok 面"),
        Vec::<String>::new(),
        "干净仓空清单透传（与 Err 严格区分）"
    );
    assert_eq!(
        port.change_edit_patch(&root, "HEAD")
            .expect("干净仓补丁 Ok 面"),
        "",
        "干净仓空补丁透传"
    );

    // 构造形态：new() 与 Default::default() 等价可用（无状态缝）
    let _default = ProcessArchiveVcs;
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
    use orchestration::port::MutationCounts;
    use orchestration::port::MutationSkipReason;
    use orchestration::port::MutationTestConclusion;
    use orchestration::port::MutationTestOutcome;
    use orchestration::port::OutputRefSink;
    use orchestration::port::{BoxToolFuture, MutationRunner, ToolStepOutput};

    /// 记录四参的 fake MutationRunner（缝注入面）。
    #[allow(clippy::type_complexity)]
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
            _cancel: foundation::cancel::CancelFlag,
            _on_output_ref: Option<OutputRefSink>,
        ) -> BoxToolFuture {
            self.calls.lock().expect("锁不可中毒").push((
                root.to_owned(),
                name.to_owned(),
                files.to_vec(),
                patch.to_owned(),
            ));
            Box::pin(async move {
                Ok(ToolStepOutput::MutationTest(MutationTestOutcome {
                    fault: None,
                    conclusion: MutationTestConclusion::Pass,
                    skip: Some(MutationSkipReason::EmptyDiff),
                    score: 0.0,
                    threshold: 0.0,
                    counts: MutationCounts::default(),
                    report_paths: Vec::new(),
                    feedback_text: String::new(),
                    exec_id: String::new(),
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
    let files = vcs.change_edit_set(&root, "HEAD").expect("清单取数应成功");
    let patch = vcs
        .change_edit_patch(&root, "HEAD")
        .expect("补丁取数应成功");

    // 注入 runner 四参（组合面：清单 / 补丁唯一入口 → runner 零 git）
    let runner = RecordingMutationRunner {
        calls: std::sync::Mutex::new(Vec::new()),
    };
    let root_str = root.to_string_lossy().into_owned();
    runner
        .run(
            &root_str,
            "demo-change",
            &files,
            &patch,
            foundation::cancel::CancelFlag::never(),
            None,
        )
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

// ---------------------------------------------------------------------------
// ProcessArchiveVcs.remove_worktree（worktree-remove-orphan-defect / AC-1 /
// AC-3 / AC-6）：归档 Cleanup 步执行缝真件面——委托既有 `git::remove_worktree`
// 两段式单点（与 ProcessWorktree 同实现同文案、零新 spawn）；本缝自组手动指引
// wrapper 已退役，`Ok` 语义 = 「磁盘已无该目录」（含缺席幂等），`Err` 面成因由
// 单点提供
// ---------------------------------------------------------------------------

/// worktree 落位目录（与主仓分置 tempdir——git_test `worktree_slot` 同式，
/// 防仓内相对路径假绿）。
fn archive_worktree_slot(tag: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-lib-worktrees-{tag}-"))
        .tempdir()
        .expect("创建 worktree 临时目录失败");
    let base = dir.path().join("wt");
    (dir, base)
}

/// 落位目录名可指定版（两缝 / 多 worktree 用例：admin 目录名取自落位目录
/// basename，重名会撞车）。
fn archive_worktree_slot_named(tag: &str, name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-lib-worktrees-{tag}-"))
        .tempdir()
        .expect("创建 worktree 临时目录失败");
    let base = dir.path().join(name);
    (dir, base)
}

/// 登记面快照：`<main>/.git/worktrees/` 下的 admin 目录名（升序；目录缺席即
/// 空集）——admin 名不写死，由 read_dir 现场取。
fn admin_names(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join(".git").join("worktrees")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// 「另一进程持有」的本进程等价面（Windows）：只读 + 无 delete 共享的句柄——
/// 存活期间 git 段与 OS 段删除均失败（沿 `lockprobe_test` 形态，本文件自建
/// 不跨模块引用私有装置）。
#[cfg(windows)]
fn hold_without_delete_share(path: &Path) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    const SHARE_READ_ONLY: u32 = 0x1;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(SHARE_READ_ONLY)
        .open(path)
        .expect("以无 delete 共享模式打开 worktree 内文件失败")
}

/// 正向：真实 git tempdir——`add_worktree` 建位 → `remove_worktree` Ok →
/// worktree 目录移除、`git worktree list` 不再列出、主仓无损（AC-5 清理成功面）。
#[test]
fn process_archive_vcs_remove_worktree目录移除且list不再列出() {
    let port = ProcessArchiveVcs::new();
    let (_dir, root) = init_repo("remove-ok");
    let (_wt_dir, wt) = archive_worktree_slot("remove-ok");

    super::git::add_worktree(&root, &wt, "change/fix-bug").expect("add_worktree 应成功");
    assert!(wt.is_dir(), "前置：worktree 目录在场");
    // git 输出恒 POSIX 分隔符（Windows 路径对拍前归一化）
    let wt_posix = wt.to_string_lossy().replace('\\', "/");
    let list_before = git(&root, &["worktree", "list"]);
    assert!(
        list_before.contains(wt_posix.as_str()),
        "前置：worktree list 列出该位: {list_before}"
    );

    port.remove_worktree(&root, &wt)
        .expect("remove_worktree 应成功");

    assert!(!wt.exists(), "worktree 磁盘目录已移除");
    let list_after = git(&root, &["worktree", "list"]);
    assert!(
        !list_after.contains(wt.to_string_lossy().as_ref()),
        "git worktree list 不再列出该位: {list_after}"
    );
    assert!(root.join("a.txt").is_file(), "主仓无损");
}

/// 正向（转发对拍）：经 `&dyn ArchiveVcsPort` 调用与 `git::remove_worktree`
/// 自由函数**效果逐字等值**（两 worktree 同式建位、各自经两入口移除——移除
/// 效果与登记回收一致，单点复用防漂移；沿既有清单 / 补丁转发测试形态）。
#[test]
fn process_archive_vcs_remove_worktree转发与自由函数效果等值() {
    let port = ProcessArchiveVcs::new();
    let dyn_port: &dyn ArchiveVcsPort = &port;
    let (_dir, root) = init_repo("remove-forward");
    let (_wt_dir_a, wt_a) = archive_worktree_slot("remove-forward-a");
    let (_wt_dir_b, wt_b) = archive_worktree_slot("remove-forward-b");

    super::git::add_worktree(&root, &wt_a, "change/via-port").expect("add A 应成功");
    super::git::add_worktree(&root, &wt_b, "change/via-free").expect("add B 应成功");

    // 各自经两入口移除
    dyn_port
        .remove_worktree(&root, &wt_a)
        .expect("trait 面移除应成功");
    super::git::remove_worktree(&root, &wt_b).expect("自由函数移除应成功");

    // 效果逐字等值：两目录同归回收、登记同清（git 输出恒 POSIX 分隔符，
    // Windows 路径对拍前归一化）
    assert!(!wt_a.exists(), "trait 面移除效果：目录回收");
    assert!(!wt_b.exists(), "自由函数移除效果：目录回收");
    let wt_a_posix = wt_a.to_string_lossy().replace('\\', "/");
    let wt_b_posix = wt_b.to_string_lossy().replace('\\', "/");
    let list = git(&root, &["worktree", "list"]);
    assert!(
        !list.contains(wt_a_posix.as_str()) && !list.contains(wt_b_posix.as_str()),
        "两入口登记回收一致（零漂移）: {list}"
    );
}

/// 边界（AC-3 + 兜底，既有 Err 面改判 `Ok`）：① 缺席路径 → `Ok`（幂等、零删除
/// 动作）② 普通目录在场（非 worktree）→ `Ok` 且目录消失。`Err` 面迁移至真占用
/// 用例（两段式皆失败）。
#[test]
fn process_archive_vcs_remove_worktree缺席与普通目录改判ok() {
    let port = ProcessArchiveVcs::new();
    let (_dir, root) = init_repo("remove-ok-idempotent");
    let absent =
        std::env::temp_dir().join(format!("vcs-lib-absent-worktree-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&absent);

    port.remove_worktree(&root, &absent)
        .expect("形态一：缺席路径应 Ok（幂等收口）");
    assert!(!absent.exists(), "缺席路径零产生");

    let plain = tempfile::Builder::new()
        .prefix("vcs-lib-plain-remove-")
        .tempdir()
        .expect("创建普通目录失败");
    std::fs::write(plain.path().join("stray.txt"), "非 worktree\n").expect("写普通目录文件");
    port.remove_worktree(&root, plain.path())
        .expect("形态二：普通目录应经 OS 层兜底回收成功");
    assert!(!plain.path().exists(), "普通目录已从磁盘消失");
    assert!(root.join("a.txt").is_file(), "主仓无损");
}

/// 正向（AC-6 wrapper 退役 / 单点透传）：真占用（无 delete 共享句柄）→ `Err`
/// 携两段式五段成因；**MUST NOT 含**旧 wrapper 前缀 `worktree 移除失败（` 与
/// 祈使短语 `请手动执行`（自组文案整段退役的判据——两缝同实现同文案）。
#[cfg(windows)]
#[test]
fn process_archive_vcs_remove_worktree_wrapper退役_五段成因由单点提供() {
    let port = ProcessArchiveVcs::new();
    let (_dir, root) = init_repo("remove-wrapper-retired");
    let (_wt_dir, wt) = archive_worktree_slot("remove-wrapper-retired");
    super::git::add_worktree(&root, &wt, "change/locked").expect("add_worktree 应成功");
    std::fs::write(wt.join("locked.txt"), "被持有的文件\n").expect("写占用文件失败");
    let holder = hold_without_delete_share(&wt.join("locked.txt"));

    let error = port
        .remove_worktree(&root, &wt)
        .expect_err("两段式皆失败应 Err");
    drop(holder);

    assert!(
        error.contains(&wt.to_string_lossy().into_owned()),
        "Err 携 worktree 绝对路径: {error}"
    );
    assert!(
        error.contains("git worktree remove --force") && error.contains("失败（退出码"),
        "Err 携原 git 错误原文（含命令串与退出码）: {error}"
    );
    assert!(
        error.contains("OS 删除错误"),
        "Err 携 OS 段删除错误: {error}"
    );
    for phrase in ["终端 cwd", "编辑器", "常驻子进程", "杀毒实时扫描", "云同步"]
    {
        assert!(
            error.contains(phrase),
            "Err 成因清单含「{phrase}」: {error}"
        );
    }
    assert!(
        error.contains("① git worktree list") && error.contains("② 直接检查 worktree 落位根"),
        "Err 携双源对账: {error}"
    );
    assert!(
        !error.contains("worktree 移除失败（"),
        "旧 wrapper 前缀已整段退役: {error}"
    );
    assert!(!error.contains("请手动执行"), "祈使短语缺席: {error}");
    assert!(!error.contains('\n'), "文案单行（零换行）: {error}");
}

/// 正向（组合行：两缝同文案，D8 反向判据）：两个同形孤儿目录（各删自身
/// admin 登记）分别经 `ProcessWorktree` 与 `ProcessArchiveVcs` → 两 `Ok`、两
/// 目录均消失、`git worktree list` 均无该位；真占用下两缝产出的 `Err` 文案
/// 除路径外等值（同一单点，零分歧）。
#[test]
fn 两缝同实现同文案_孤儿均回收且err除路径外等值() {
    use workflow::write::WorktreePort as _;

    let archive = ProcessArchiveVcs::new();
    let worktree_port = super::ProcessWorktree::new();
    let (_dir, root) = init_repo("two-seams");
    let (_wt_dir_a, wt_a) = archive_worktree_slot_named("two-seams", "wt-archive");
    let (_wt_dir_b, wt_b) = archive_worktree_slot_named("two-seams", "wt-worktree");

    let admins_before_a = admin_names(&root);
    super::git::add_worktree(&root, &wt_a, "change/via-archive").expect("add A 应成功");
    let admins_after_a = admin_names(&root);
    let admin_a: Vec<&String> = admins_after_a
        .iter()
        .filter(|name| !admins_before_a.contains(name))
        .collect();
    assert_eq!(admin_a.len(), 1, "前置：A 的新增 admin 恰一条");

    let admins_before_b = admin_names(&root);
    super::git::add_worktree(&root, &wt_b, "change/via-worktree").expect("add B 应成功");
    let admins_after_b = admin_names(&root);
    let admin_b: Vec<&String> = admins_after_b
        .iter()
        .filter(|name| !admins_before_b.contains(name))
        .collect();
    assert_eq!(admin_b.len(), 1, "前置：B 的新增 admin 恰一条");

    // 两位各自造成孤儿形态（只删自身 admin——登记面已注销而目录仍在场）
    for admin in [admin_a[0], admin_b[0]] {
        std::fs::remove_dir_all(root.join(".git").join("worktrees").join(admin))
            .expect("装置手删 admin 失败");
    }
    let list_before = git(&root, &["worktree", "list"]);
    for wt in [&wt_a, &wt_b] {
        assert!(
            !list_before.contains(wt.to_string_lossy().replace('\\', "/").as_str()),
            "前置：两位均已注销（孤儿形态）: {list_before}"
        );
    }

    worktree_port
        .remove_worktree(&root, &wt_b)
        .expect("create / delete 缝孤儿回收应 Ok");
    archive
        .remove_worktree(&root, &wt_a)
        .expect("归档缝孤儿回收应 Ok");

    assert!(!wt_a.exists() && !wt_b.exists(), "两目录均消失");
    let list_after = git(&root, &["worktree", "list"]);
    for wt in [&wt_a, &wt_b] {
        assert!(
            !list_after.contains(wt.to_string_lossy().replace('\\', "/").as_str()),
            "list 均无该位: {list_after}"
        );
    }

    // Err 面：同一单点在两缝产出的文案除路径外等值
    #[cfg(windows)]
    {
        // 同一落位根下两个 worktree（只有 basename 不同——文案对拍只差路径）
        let err_slot = tempfile::Builder::new()
            .prefix("vcs-lib-two-seams-err-")
            .tempdir()
            .expect("创建 worktree 临时目录失败");
        let wt_c = err_slot.path().join("wt-err-a");
        let wt_d = err_slot.path().join("wt-err-b");
        super::git::add_worktree(&root, &wt_c, "change/err-a").expect("add C 应成功");
        super::git::add_worktree(&root, &wt_d, "change/err-b").expect("add D 应成功");
        for wt in [&wt_c, &wt_d] {
            std::fs::write(wt.join("locked.txt"), "被持有的文件\n").expect("写占用文件失败");
        }
        let holder_c = hold_without_delete_share(&wt_c.join("locked.txt"));
        let holder_d = hold_without_delete_share(&wt_d.join("locked.txt"));
        let error_c = worktree_port
            .remove_worktree(&root, &wt_c)
            .expect_err("真占用应 Err");
        let error_d = archive
            .remove_worktree(&root, &wt_d)
            .expect_err("真占用应 Err");
        drop(holder_c);
        drop(holder_d);

        // git 输出恒 POSIX 分隔符、OS / 落位根用原样分隔符：两种形态一并归一
        let normalize = |error: &str, wt: &Path| {
            let display = wt.to_string_lossy().into_owned();
            let posix = display.replace('\\', "/");
            error.replace(&display, "<wt>").replace(&posix, "<wt>")
        };
        let normalized_c = normalize(&error_c, &wt_c);
        let normalized_d = normalize(&error_d, &wt_d);
        assert_eq!(
            normalized_c, normalized_d,
            "两缝对同一 `Err` 装置产出的文案除路径外等值（同一单点）"
        );
    }
}

/// 边界（AC-1 的缝面）：`add_worktree` 后手删 admin 造成已注销孤儿 → 经
/// `&dyn ArchiveVcsPort` 调用 → `Ok`、目录消失、`list` 无该位（Cleanup 步的
/// 真实受益面）。
#[test]
fn process_archive_vcs_remove_worktree孤儿经trait对象回收() {
    let port = ProcessArchiveVcs::new();
    let dyn_port: &dyn ArchiveVcsPort = &port;
    let (_dir, root) = init_repo("remove-orphan");
    let (_wt_dir, wt) = archive_worktree_slot("remove-orphan");

    super::git::add_worktree(&root, &wt, "change/orphan").expect("add_worktree 应成功");
    let admins: Vec<PathBuf> = std::fs::read_dir(root.join(".git").join("worktrees"))
        .expect("读登记面失败")
        .flatten()
        .map(|entry| entry.path())
        .collect();
    assert_eq!(admins.len(), 1, "前置：admin 登记恰一条: {admins:?}");
    std::fs::remove_dir_all(&admins[0]).expect("装置手删 admin 失败");
    assert!(wt.is_dir(), "前置：目录仍在场（孤儿形态）");

    dyn_port
        .remove_worktree(&root, &wt)
        .expect("孤儿应经兜底回收成功");

    assert!(!wt.exists(), "孤儿目录已消失");
    let list = git(&root, &["worktree", "list"]);
    assert!(
        !list.contains(wt.to_string_lossy().replace('\\', "/").as_str()),
        "git worktree list 无该位: {list}"
    );
}

/// 边界（AC-7 branch 零触碰）：remove 后 `git branch` 列表仍含 `change/<name>`
///（`worktree remove --force` 不动分支——归档链 branch 保留语义的 vcs 半边）。
#[test]
fn process_archive_vcs_remove_worktree后分支留存() {
    let port = ProcessArchiveVcs::new();
    let (_dir, root) = init_repo("remove-branch");
    let (_wt_dir, wt) = archive_worktree_slot("remove-branch");

    super::git::add_worktree(&root, &wt, "change/fix-bug").expect("add 应成功");
    port.remove_worktree(&root, &wt).expect("remove 应成功");

    let branches = git(&root, &["branch", "--list", "change/fix-bug"]);
    assert!(
        branches.contains("change/fix-bug"),
        "remove 后 branch change/<name> 留存: {branches:?}"
    );
}

/// 边界（D1 --force 语义锚）：脏 worktree（未提交 `node_modules` 形态目录在
/// 盘）经 `--force` 仍可移除——bootstrap 脏面不阻断清理（不带 force 恒失败的
/// 语义锚：`--force` 带走未提交产物）。
#[test]
fn process_archive_vcs_remove_worktree脏worktree经force仍可移除() {
    let port = ProcessArchiveVcs::new();
    let (_dir, root) = init_repo("remove-dirty");
    let (_wt_dir, wt) = archive_worktree_slot("remove-dirty");

    super::git::add_worktree(&root, &wt, "change/dirty-wt").expect("add 应成功");
    // bootstrap 形态脏面：未提交的 node_modules 目录 + 未跟踪文件
    std::fs::create_dir_all(wt.join("node_modules").join("left-pad"))
        .expect("预置 node_modules 形态目录失败");
    std::fs::write(
        wt.join("node_modules").join("left-pad").join("index.js"),
        "module.exports = 1;\n",
    )
    .expect("预置脏文件失败");
    std::fs::write(wt.join("scratch.md"), "未提交草稿\n").expect("预置未跟踪文件失败");

    port.remove_worktree(&root, &wt)
        .expect("脏 worktree 经 --force 仍应移除成功");

    assert!(!wt.exists(), "脏面（node_modules 形态）随 --force 一并回收");
    assert!(root.join("a.txt").is_file(), "主仓无损");
}

/// 构造面（正向）：`new()` / `default()` 等值可重复构造（无状态缝——组合根
/// 按需铸，随新方法扩展构造面持衡）。
#[test]
fn process_archive_vcs构造_new与default等值() {
    let _a = ProcessArchiveVcs::new();
    let _b = ProcessArchiveVcs;
    let _c = ProcessArchiveVcs::new();
    // 无状态空类型：可重复构造即断言面（字段零个，等值由类型层承载）
}

// ---------------------------------------------------------------------------
// ProcessArchiveVcs.restore_baseline（check-before-flow-start / AC-5 / AC-6）：
// 委托 `git::restore_baseline` 单点；`Err` 经 map_err 附 worktree 路径与「主仓
// 未受影响」注记——真实 git tempdir + worktree 分置（零 mock）
// ---------------------------------------------------------------------------

use crate::test_path_lock as lock_path;

/// 正向（转发对拍）：经 `&dyn ArchiveVcsPort` 调用与 `git::restore_baseline`
/// 自由函数同仓同排除面双跑效果逐字等值（单点复用防漂移——同仓同排除面双跑
/// 后 status / 摘要形态一致）。
#[test]
fn process_archive_vcs_restore_baseline转发对拍与自由函数等值() {
    let _guard = lock_path();
    let port = ProcessArchiveVcs::new();

    // 双仓同构改动：tracked 编辑 + untracked + 排除目录，双跑后仓态等值
    let (_dir_a, root_a) = init_repo("restore-fwd-a");
    let (_dir_b, root_b) = init_repo("restore-fwd-b");
    for root in [&root_a, &root_b] {
        std::fs::write(root.join("a.txt"), "编辑\n").expect("写改动");
        std::fs::write(root.join("fresh.txt"), "未跟踪\n").expect("写新文件");
        let modules = root.join("node_modules").join("lib");
        std::fs::create_dir_all(&modules).expect("建排除目录");
        std::fs::write(modules.join("index.js"), "x").expect("写依赖");
    }

    let summary_a = port
        .restore_baseline(&root_a, &["node_modules", ".*", "openspec"])
        .expect("port 侧恢复应 Ok");
    let summary_b = super::git::restore_baseline(&root_b, &["node_modules", ".*", "openspec"])
        .expect("自由函数侧恢复应 Ok");

    // 摘要同形（排除面计数一致）、仓态等值（双跑后 status 同空）
    assert_eq!(summary_a, summary_b, "恢复摘要串逐字等值（单点委托）");
    let status_a = git(&root_a, &["status", "--porcelain"]);
    let status_b = git(&root_b, &["status", "--porcelain"]);
    assert_eq!(status_a, status_b, "双跑后仓态等值");
    // 排除面（node_modules）为未跟踪保留面——status 仅含该行（fresh.txt 已清、
    // a.txt 已回 HEAD）
    for line in status_a.lines() {
        assert!(
            line.contains("node_modules"),
            "status 面仅排除目录保留行: {line:?}"
        );
    }
    assert!(
        root_a
            .join("node_modules")
            .join("lib")
            .join("index.js")
            .is_file(),
        "排除面保留（port 侧）"
    );
    assert!(
        !root_a.join("fresh.txt").exists(),
        "未跟踪新文件被清除（port 侧 clean 半边）"
    );
    assert_eq!(
        std::fs::read_to_string(root_a.join("a.txt")).expect("读 a.txt"),
        "initial
",
        "tracked 回 HEAD（port 侧 reset 半边）"
    );
}

/// 异常（map_err 语境面）：非 git 目录 → `Err` 含 worktree 路径字面量与「主仓
/// 未受影响」注记（与 git 侧原文可辨）。
#[test]
fn process_archive_vcs_restore_baseline_err携路径与主仓注记() {
    let _guard = lock_path();
    let port = ProcessArchiveVcs::new();
    let plain = tempfile::Builder::new()
        .prefix("vcs-lib-restore-plain-")
        .tempdir()
        .expect("建普通目录失败");

    let error = port
        .restore_baseline(plain.path(), &[".*"])
        .expect_err("非 git 目录应 Err");
    let rendered = plain.path().to_string_lossy().into_owned();
    assert!(
        error.contains(&rendered),
        "Err 携 worktree 路径字面量: {error}"
    );
    assert!(
        error.contains("主仓未受影响"),
        "Err 携主仓未受影响注记: {error}"
    );
}

/// 边界（主仓双保险真件锚 / AC-6 灾难面护栏）：真实主仓 + worktree 分置
/// tempdir，主仓预置未提交改动 → restore 调用后主仓未提交改动原样在案
///（restore 仅作用于 worktree）。
#[test]
fn process_archive_vcs_restore_baseline_主仓双保险真件锚() {
    let _guard = lock_path();
    let port = ProcessArchiveVcs::new();
    let (_main_dir, main_root) = init_repo("restore-main-guard");
    let (_wt_dir, wt_base) = archive_worktree_slot("restore-main-guard");
    let wt = wt_base.clone();
    git(
        &main_root,
        &["worktree", "add", &wt.to_string_lossy(), "-b", "change/wt"],
    );

    // 主仓预置未提交改动（tracked 编辑 + untracked 新文件）
    std::fs::write(main_root.join("a.txt"), "主仓未提交编辑\n").expect("写主仓改动");
    std::fs::write(main_root.join("main-local.txt"), "主仓本地文件\n").expect("写主仓新文件");

    // worktree 内也放改动（restore 作用面）
    std::fs::write(wt.join("a.txt"), "worktree 编辑\n").expect("写 wt 改动");

    port.restore_baseline(&wt, &[".*", "openspec"])
        .expect("恢复应 Ok");

    // 主仓未提交改动原样在案（灾难面护栏——restore 仅作用 worktree）
    assert_eq!(
        std::fs::read_to_string(main_root.join("a.txt")).expect("读主仓 a.txt"),
        "主仓未提交编辑\n",
        "主仓 tracked 改动原样（零重置）"
    );
    assert!(
        main_root.join("main-local.txt").is_file(),
        "主仓 untracked 原样（零 clean）"
    );
    let main_status = git(&main_root, &["status", "--porcelain"]);
    assert!(
        main_status.contains("a.txt") && main_status.contains("main-local.txt"),
        "主仓改动在 status 面在案: {main_status:?}"
    );
    // worktree 侧已恢复（对照面）
    assert_eq!(
        std::fs::read_to_string(wt.join("a.txt")).expect("读 wt a.txt"),
        "initial\n",
        "worktree 内容回 HEAD（对照面）"
    );
}

/// 正向（构造持衡）：`new()` / `Default::default()` 等值构造既有断言随新方法
/// 持衡（无状态装配回归锚）。
#[test]
fn process_archive_vcs_restore_baseline构造持衡() {
    let _new = ProcessArchiveVcs::new();
    let _default = ProcessArchiveVcs;
}

// ---------------------------------------------------------------------------
// vcs_runtime::placement / ProcessWorktree::tracked_at_head（落位与准入判据的
// 单点委托面）：`&dyn WorktreePort` 动态分发与自由函数双入口逐字对拍
// ---------------------------------------------------------------------------

use workflow::write::WorktreePort;

/// trait 委托（正向）：`ProcessWorktree::placement` 经 `&dyn WorktreePort` 动态
/// 分发与自由函数 `placement` 同仓双跑逐字等值（`root` 与 `rel` 两值对拍）。
#[test]
fn process_worktree_placement委托_与自由函数逐字等值() {
    let (_dir, repo) = init_repo("worktree-port-placement");
    let port = ProcessWorktree::new();
    let dynamic: &dyn WorktreePort = &port;

    let via_fn = placement(&repo).expect("自由函数派生应成功");
    let via_port = dynamic.placement(&repo).expect("trait 面派生应成功");
    assert_eq!(via_fn, via_port, "root / rel 两值逐字等值");
    assert_eq!(via_fn.rel, "", "git 根 workspace 偏移为空串");

    let nested = repo.join("client").join("app");
    std::fs::create_dir_all(&nested).expect("建子目录 workspace 失败");
    let nested_fn = placement(&nested).expect("自由函数子目录派生应成功");
    let nested_port = dynamic
        .placement(&nested)
        .expect("trait 面子目录派生应成功");
    assert_eq!(nested_fn, nested_port, "子目录形态两入口逐字等值");
    assert_eq!(nested_port.rel, "client/app", "trait 面携相对偏移");
}

/// trait 委托（正向，表驱动两形态）：`ProcessWorktree::tracked_at_head` 与
/// `git::tracked_at_head` 自由函数结果逐字等值（在 HEAD / 未跟踪两态）。
#[test]
fn process_worktree_tracked_at_head委托对拍_两态等值() {
    let (_dir, repo) = init_repo("worktree-port-tracked");
    let port = ProcessWorktree::new();
    let dynamic: &dyn WorktreePort = &port;
    std::fs::create_dir_all(repo.join("tracked")).expect("建已提交子目录失败");
    std::fs::write(repo.join("tracked/keep.txt"), "在案\n").expect("写子目录文件失败");
    git(&repo, &["add", "tracked"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-q",
            "-m",
            "tracked",
        ],
    );
    std::fs::create_dir_all(repo.join("fresh")).expect("建未跟踪子目录失败");

    for (rel, expected) in [("tracked", true), ("fresh", false)] {
        let via_fn = super::git::tracked_at_head(&repo, rel).expect("自由函数判据应 Ok");
        let via_port = dynamic
            .tracked_at_head(&repo, rel)
            .expect("trait 面判据应 Ok");
        assert_eq!(via_fn, expected, "自由函数两态: {rel} → {expected}");
        assert_eq!(via_port, via_fn, "trait 面与自由函数等值（{rel}）");
    }
}

/// trait 委托（边界）：`rel = ""` → `Ok(true)`（零 spawn 语义随实现侧兜底，
/// `PATH` 注入下仍成立）。
#[test]
fn process_worktree_tracked_at_head_rel空串_ok_true() {
    let _guard = lock_path();
    let (_dir, repo) = init_repo("worktree-port-empty-rel");
    let port = ProcessWorktree::new();
    let dynamic: &dyn WorktreePort = &port;

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let result = dynamic.tracked_at_head(&repo, "");
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    assert!(
        result.expect("rel 空串应 Ok(true)"),
        "git 不可发现仍 Ok(true)（实现侧零 spawn 兜底）"
    );
}

/// trait 委托（异常）：非 git 目录 → trait 面 `Err` 原文透传（不包装不吞，与
/// 自由函数同串）。
#[test]
fn process_worktree_tracked_at_head_非git目录_err原文透传() {
    let plain = tempfile::Builder::new()
        .prefix("vcs-lib-port-plain-")
        .tempdir()
        .expect("建普通目录失败");
    let port = ProcessWorktree::new();
    let dynamic: &dyn WorktreePort = &port;

    let via_fn = super::git::tracked_at_head(plain.path(), "sub").expect_err("自由函数应 Err");
    let via_port = dynamic
        .tracked_at_head(plain.path(), "sub")
        .expect_err("trait 面应 Err");

    assert_eq!(via_port, via_fn, "Err 原文透传（同串，不包装）");
}

/// `worktree_root` 公共入口退役（异常）：本 crate 出口只余 `placement`，旧名
/// 不可达（编译面以 `use super::placement` 成立；源面零残留以字面量自证）。
#[test]
fn worktree_root公共入口退役_出口只余placement() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("lib.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取 {} 失败: {error}", path.display()));

    assert!(
        source.contains("pub fn placement(workspace_root: &Path)"),
        "落位派生单点以 `placement` 出口"
    );
    assert!(
        !source.contains("pub fn worktree_root("),
        "旧 `worktree_root` 公共入口已退役（零残留定义）"
    );
    // 编译面：本文件顶部 `use super::placement` 成立即新出口可达
    let _ = placement(Path::new(".")).is_ok();
}

/// 误传锚的整树锚（异常，AC-4 行为钉）：子目录 workspace 夹具下把 exec root
/// （host 下逐段 `join` 的 `rel` 子目录）传给 `ProcessArchiveVcs::remove_worktree`
/// → host 整树仍在场且 `git worktree list` 仍列出（误传「回收零生效 + 登记残留」
/// 形态可见）；对照面传 host → 整树回收且登记注销。执行根子目录自身是否被 OS
/// 兜底删除不作硬断言（兜底删除受路径形态影响，生产回收恒传 host）。
#[test]
fn process_archive_vcs_remove_worktree误传执行根_host整树与登记留存() {
    let _guard = lock_path();
    let port = ProcessArchiveVcs::new();
    let (_dir, main_root) = init_repo("remove-wrong-anchor");
    let (_wt_dir, host) = archive_worktree_slot_named("remove-wrong-anchor", "wrong-anchor-wt");
    git(
        &main_root,
        &[
            "worktree",
            "add",
            &host.to_string_lossy(),
            "-b",
            "change/subdir",
        ],
    );
    let mut exec_root = host.clone();
    for segment in ["packages", "desktop"] {
        exec_root = exec_root.join(segment);
    }
    std::fs::create_dir_all(&exec_root).expect("预置执行根目录失败");
    std::fs::write(exec_root.join("keep.txt"), "执行根内容\n").expect("写执行根文件失败");

    let _ = port.remove_worktree(&main_root, &exec_root);

    // 误传锚的主判据：host 整树与登记留存（执行根子目录自身是否被 OS 兜底删除
    // 受路径形态影响，不作硬断言——生产回收恒传 host）
    assert!(host.is_dir(), "host 整树仍在场（回收零生效）");
    assert!(
        host.join("a.txt").is_file(),
        "host 检出内容原样（整树零回收）"
    );
    let listing = git(&main_root, &["worktree", "list"]);
    assert!(
        listing.contains("wrong-anchor-wt"),
        "git worktree list 仍列出 host（登记残留可见）: {listing}"
    );

    // 对照面：传 host 才回收整树且登记注销
    port.remove_worktree(&main_root, &host)
        .expect("传 host 应回收成功");
    assert!(!host.exists(), "传 host → 整树回收");
    let listing_after = git(&main_root, &["worktree", "list"]);
    assert!(
        !listing_after.contains("wrong-anchor-wt"),
        "传 host → 登记注销: {listing_after}"
    );
}
