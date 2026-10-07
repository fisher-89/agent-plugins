//! `git` 的单元测试（test-design「git.rs -> git_test.rs」节）：真实 git 子进程
//! + tempfile 仓真件锚定（产品硬依赖同口径；git 经 `git -C <root>` 显式寻
//! 址）——probe 三态（干净 / 脏 / 空仓、非 git 仓、git 不可发现）、
//! branch_exists 两态、add_worktree 建域与基线恒 HEAD、补偿面（remove /
//! delete_branch）与补偿序组合。「git 不可发现」行以 PATH 隔离窗口注入（进程
//! 全局变量边界——本 crate 自持 PATH 锁串行化，真实 spawn 用例全量持锁防与
//! 窗口重叠；测毕恢复）。

use std::path::{Path, PathBuf};
use std::process::Command;

use super::git;
use crate::test_path_lock as lock_path;

// ---------------------------------------------------------------------------
// 装置：真实 git tempdir 仓（初始提交在案）+ 直驱 git 断言助手
// ---------------------------------------------------------------------------

/// 在 `root` 内执行一次真实 git 子命令（stdout 捕获；非零退出 panic——装置
/// 面失败即用例失败，不静默）。
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("装置 git 拉起失败: {error}"));
    assert!(
        output.status.success(),
        "装置 git {} 失败: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 建一个真实 git 仓（单提交 `a.txt` 在案；commit 身份经 -c 注入，不依赖全
/// 局 gitconfig）。返回仓根（tempfile RAII 随 `repo_dir` 存活）。
fn init_repo(tag: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-git-test-{tag}-"))
        .tempdir()
        .expect("创建仓临时目录失败");
    let root = dir.path().to_path_buf();
    git(&root, &["init", "-q"]);
    fs_write(&root, "a.txt", "initial\n");
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

/// 空仓（`git init` 零提交）。
fn init_empty_repo(tag: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-git-empty-{tag}-"))
        .tempdir()
        .expect("创建空仓临时目录失败");
    let root = dir.path().to_path_buf();
    git(&root, &["init", "-q"]);
    (dir, root)
}

/// 非 git 仓普通目录。
fn plain_dir(tag: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-git-plain-{tag}-"))
        .tempdir()
        .expect("创建普通目录失败");
    let root = dir.path().to_path_buf();
    fs_write(&root, "not-a-repo.txt", "普通目录\n");
    (dir, root)
}

fn fs_write(root: &Path, name: &str, content: &str) {
    std::fs::write(root.join(name), content).expect("装置写文件失败");
}

fn head_sha(root: &Path) -> String {
    git(root, &["rev-parse", "HEAD"]).trim().to_owned()
}

/// worktree 落位目录（与主仓分置 tempdir，防仓内相对路径假绿）。
fn worktree_slot(tag: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-git-worktrees-{tag}-"))
        .tempdir()
        .expect("创建 worktree 临时目录失败");
    let base = dir.path().join("wt");
    (dir, base)
}

// ---------------------------------------------------------------------------
// probe 三态 + 非 git 仓 + git 不可发现
// ---------------------------------------------------------------------------

/// 干净仓：初始提交在案、零未提交改动 → `RepoProbe { head: HEAD sha,
/// dirty: false }`。
#[test]
fn probe干净仓_head为当前head且dirty为false() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("probe-clean");

    let probe = git::probe(&root).expect("干净仓 probe 应 Ok");

    assert_eq!(probe.head, head_sha(&root), "head = 主仓 HEAD sha");
    assert!(!probe.dirty, "零未提交改动 → dirty: false");
}

/// 脏仓：未跟踪文件 + staged 文件各一 → `dirty: true` 且 head 不变（基线仍
/// HEAD——脏仓不影响基线语义的前提面）。
#[test]
fn probe脏仓_dirty为true且head不变() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("probe-dirty");
    let head_before = head_sha(&root);

    fs_write(&root, "untracked.txt", "未跟踪\n"); // 工作区未跟踪
    fs_write(&root, "staged.txt", "已暂存\n");
    git(&root, &["add", "staged.txt"]); // staged

    let probe = git::probe(&root).expect("脏仓 probe 应 Ok");

    assert!(
        probe.dirty,
        "porcelain 非空（未跟踪 + staged）→ dirty: true"
    );
    assert_eq!(probe.head, head_before, "head 不变（基线仍 HEAD）");
}

/// 空仓（零提交，无 HEAD）→ Err 含「先提交」引导（design：空仓 HEAD 失败
/// 映射）。
#[test]
fn probe空仓_err先提交引导() {
    let _guard = lock_path();
    let (_dir, root) = init_empty_repo("probe-empty");

    let error = git::probe(&root).expect_err("空仓应 Err");

    assert!(
        error.contains("先提交"),
        "Err 含「先提交」引导文案，实际: {error}"
    );
}

/// 非 git 仓（普通目录）→ 显式 Err 引导（非静默；不回退）。
#[test]
fn probe非git仓_err显式引导不静默() {
    let _guard = lock_path();
    let (_dir, root) = plain_dir("probe-plain");

    let error = git::probe(&root).expect_err("非 git 仓应 Err");

    assert!(
        error.contains("非 git 仓") && !error.contains("先提交"),
        "Err 显式记因非 git 仓（与空仓 / git 缺失可辨），实际: {error}"
    );
}

/// git 不可发现（PATH 隔离窗口）→ Err 含「git 不可用」引导文案（环境缺失
/// 与命令退出非零可辨）；测毕恢复 PATH。
#[test]
fn probe_git不可发现_err引导文案() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("probe-missing-git");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let error = git::probe(&root);
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let error = error.expect_err("git 不可发现应 Err");
    assert!(
        error.contains("git 不可用"),
        "Err 含「git 不可用」引导文案，实际: {error}"
    );
}

// ---------------------------------------------------------------------------
// branch_exists 两态
// ---------------------------------------------------------------------------

/// 既有分支 → true；不存在 → false（非 Err——miss 是合法产出）。
#[test]
fn branch_exists两态_在案true缺席false非err() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("branch-exists");

    // 既有分支：初始提交铸出的 master / main 任一在案（跨 git 默认名差异，
    // show-ref 直接枚举断言）
    let refs = git(&root, &["show-ref", "--heads"]);
    let existing: String = refs
        .lines()
        .next()
        .and_then(|line| line.rsplit('/').next())
        .expect("初始分支应在案")
        .to_owned();

    assert_eq!(
        git::branch_exists(&root, &existing).expect("在案分支应 Ok"),
        true,
        "既有分支 → true"
    );
    assert_eq!(
        git::branch_exists(&root, "change/absent").expect("缺席分支应 Ok"),
        false,
        "不存在 → false（非 Err）"
    );
}

// ---------------------------------------------------------------------------
// add_worktree 建域与基线恒 HEAD
// ---------------------------------------------------------------------------

/// add_worktree 建域：worktree 目录在场、branch `change/x` 在案且指向主仓
/// HEAD（rev-parse 对拍）。
#[test]
fn add_worktree建域_目录在场且分支指向主仓head() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("add-domain");
    let (_wt_dir, wt) = worktree_slot("add-domain");

    git::add_worktree(&root, &wt, "change/fix-bug").expect("add_worktree 应成功");

    assert!(wt.is_dir(), "worktree 目录在场");
    assert!(wt.join("a.txt").is_file(), "HEAD 检出内容在场（基线文件）");
    assert_eq!(
        git(&root, &["rev-parse", "change/fix-bug"]).trim(),
        head_sha(&root),
        "branch change/fix-bug 在案且指向主仓 HEAD"
    );
}

/// 基线恒 HEAD：主仓置未提交改动后 add → worktree 内**不含**该未提交文件、
/// 内容 = HEAD 检出（AC-4 真件半边；base_commit 语义前提）。
#[test]
fn add_worktree基线恒head_未提交改动不进worktree() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("add-baseline");
    fs_write(&root, "uncommitted.txt", "主仓未提交改动\n"); // 未跟踪改动

    let (_wt_dir, wt) = worktree_slot("add-baseline");
    git::add_worktree(&root, &wt, "change/dirty-base").expect("add 应成功");

    assert!(
        !wt.join("uncommitted.txt").exists(),
        "worktree 不含主仓未提交文件（基线恒 HEAD）"
    );
    assert!(wt.join("a.txt").is_file(), "worktree 内容 = HEAD 检出");
}

/// add 目标已存在（目录预置在场）→ Err（不覆盖既有目录）。
#[test]
fn add_worktree目标目录已存在_err不覆盖() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("add-taken");
    let (wt_dir, wt) = worktree_slot("add-taken");
    std::fs::create_dir_all(&wt).expect("预置目标目录失败");
    fs_write(&wt, "preexisting.txt", "既有目录\n");

    let error = git::add_worktree(&root, &wt, "change/taken").expect_err("目标在场应 Err");

    assert!(
        !error.contains("git 不可用"),
        "Err 为 git 语境失败而非环境缺失: {error}"
    );
    assert!(wt.join("preexisting.txt").is_file(), "既有目录零覆盖");
    let _ = wt_dir; // RAII 存活至用例尾
}

// ---------------------------------------------------------------------------
// 补偿面：remove_worktree / delete_branch / 补偿序组合
// ---------------------------------------------------------------------------

/// remove_worktree：add 后 `remove_worktree`（--force）→ worktree 目录移除且
/// 主仓树无损；再 add 同路径可成功（补偿面幂等可重试）。
#[test]
fn remove_worktree_目录移除主仓无损且同路径可再add() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-wt");
    let (_wt_dir, wt) = worktree_slot("remove-wt");

    git::add_worktree(&root, &wt, "change/fix-bug").expect("add 应成功");
    git::remove_worktree(&root, &wt).expect("remove_worktree 应成功");

    assert!(!wt.exists(), "worktree 目录移除（含 git 登记）");
    assert!(root.join("a.txt").is_file(), "主仓树无损");
    // 再 add 前先删分支（`worktree remove` 只回收目录与登记、分支留存——补偿
    // 链同序 remove → delete → 重建），同路径同分支可再建成功（补偿面幂等
    // 可重试）
    git::delete_branch(&root, "change/fix-bug").expect("分支清除应成功");
    git::add_worktree(&root, &wt, "change/fix-bug")
        .expect("同路径再 add 可成功（补偿面幂等可重试）");
}

/// delete_branch：add 铸出的分支经 `delete_branch` → show-ref miss；分支删除
/// 不触碰主仓工作区。
#[test]
fn delete_branch_分支清除且主仓工作区零触碰() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("delete-branch");
    let (_wt_dir, wt) = worktree_slot("delete-branch");

    git::add_worktree(&root, &wt, "change/to-delete").expect("add 应成功");
    fs_write(&root, "workspace-edit.txt", "主仓工作区改动\n"); // 工作区观察锚

    // worktree 在案时分支被占用（checked out）——先移除 worktree 再删分支
    //（create 补偿链同序）
    git::remove_worktree(&root, &wt).expect("remove 应成功");
    git::delete_branch(&root, "change/to-delete").expect("delete_branch 应成功");

    assert_eq!(
        git::branch_exists(&root, "change/to-delete").expect("show-ref 应 Ok"),
        false,
        "分支清除（show-ref miss）"
    );
    assert!(
        root.join("workspace-edit.txt").is_file(),
        "分支删除不触碰主仓工作区"
    );
    assert!(root.join("a.txt").is_file(), "主仓树无损");
}

/// 补偿序组合：add → remove → delete 全链后主仓回到初态（无 worktree 登记、
/// 无分支残留——create 补偿链的 vcs 半边证据）。
#[test]
fn 补偿序组合_add后remove加delete全链主仓回初态() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("compensate-chain");
    let (_wt_dir, wt) = worktree_slot("compensate-chain");

    git::add_worktree(&root, &wt, "change/compensate").expect("add 应成功");
    git::remove_worktree(&root, &wt).expect("remove 应成功");
    git::delete_branch(&root, "change/compensate").expect("delete 应成功");

    // 主仓回到初态：worktree 登记清空（list 仅主仓一行）、分支零残留
    let list = git(&root, &["worktree", "list"]);
    assert!(
        !list.contains(wt.to_string_lossy().as_ref()),
        "worktree 登记清空，实际 list: {list}"
    );
    assert_eq!(
        git::branch_exists(&root, "change/compensate").expect("show-ref 应 Ok"),
        false,
        "分支零残留"
    );
    assert!(root.join("a.txt").is_file(), "主仓树无损（初态在案）");
    assert!(!wt.exists(), "worktree 磁盘目录已回收");
}
