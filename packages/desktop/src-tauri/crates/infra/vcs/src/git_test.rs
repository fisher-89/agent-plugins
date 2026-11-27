use std::path::{Path, PathBuf};
use std::process::Command;

use super::git;
use crate::test_path_lock as lock_path;

// ---------------------------------------------------------------------------
// 装置：真实 git tempdir 仓（初始提交在案）+ 直驱 git 断言助手
// ---------------------------------------------------------------------------

/// 在 `root` 内执行一次真实 git 子命令
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

/// 建一个真实 git 仓（单提交 `a.txt` 在案；commit 身份经仓本地 config 注入
/// ——rebase 重放会铸新提交，仅靠 -c 逐条注入不够；不依赖全局 gitconfig）。
/// 返回仓根（tempfile RAII 随 `repo_dir` 存活）。
fn init_repo(tag: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-git-test-{tag}-"))
        .tempdir()
        .expect("创建仓临时目录失败");
    let root = dir.path().to_path_buf();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "fixture"]);
    git(&root, &["config", "user.email", "fixture@example.com"]);
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

/// 同 [`worktree_slot`]，落位目录名可指定（多 worktree 用例：admin 目录名
/// 取自落位目录 basename，重名会让登记面对拍撞车）。
fn worktree_slot_named(tag: &str, name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-git-worktrees-{tag}-"))
        .tempdir()
        .expect("创建 worktree 临时目录失败");
    let base = dir.path().join(name);
    (dir, base)
}

/// 登记面快照：`<main>/.git/worktrees/` 下的 admin 目录名（升序；目录缺席即
/// 空集）。admin 名不写死——由 read_dir 现场取，随 git 命名策略演进不假红。
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
/// 存活期间 `git worktree remove` 与 OS 层 `remove_dir_all` 均失败，`drop` 即
/// 释放（沿 `lockprobe_test` 的 `hold_with_share` 形态，本文件自建不跨模块引用）。
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

    assert!(
        git::branch_exists(&root, &existing).expect("在案分支应 Ok"),
        "既有分支 → true"
    );
    assert!(
        !git::branch_exists(&root, "change/absent").expect("缺席分支应 Ok"),
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

    assert!(
        !git::branch_exists(&root, "change/to-delete").expect("show-ref 应 Ok"),
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
    assert!(
        !git::branch_exists(&root, "change/compensate").expect("show-ref 应 Ok"),
        "分支零残留"
    );
    assert!(root.join("a.txt").is_file(), "主仓树无损（初态在案）");
    assert!(!wt.exists(), "worktree 磁盘目录已回收");
}

/// remove_worktree 分支留存（边界，worktree-cleanup-and-active-cap / AC-7）：
/// remove 后 branch `change/<name>` 仍在案——`worktree remove --force` 只回收
/// 目录与登记、分支留存（既有注释语义断言面：清理步不触碰分支）。
#[test]
fn remove_worktree后分支留存_清理步零触碰() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-branch-kept");
    let (_wt_dir, wt) = worktree_slot("remove-branch-kept");

    git::add_worktree(&root, &wt, "change/fix-bug").expect("add 应成功");
    assert!(
        git::branch_exists(&root, "change/fix-bug").expect("show-ref 应 Ok"),
        "前置：分支铸出在案"
    );
    git::remove_worktree(&root, &wt).expect("remove 应成功");

    assert!(!wt.exists(), "worktree 目录回收");
    assert!(
        git::branch_exists(&root, "change/fix-bug").expect("show-ref 应 Ok"),
        "branch change/<name> 留存（worktree remove 不动分支）"
    );
}

/// 边界（AC-1 兜底正面，原 Err 面改判 `Ok`）：目录在场但非 worktree（git 段必
/// 失败 `not a working tree`）→ OS 层兜底删除成功 → `Ok`，目录内文件随之消失，
/// 主仓树无损。
#[test]
fn remove_worktree普通目录兜底删除_ok且目录消失() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-not-wt");
    let (_plain_dir, plain) = plain_dir("remove-not-wt");
    fs_write(&plain, "stray.txt", "非 worktree 普通目录\n");

    git::remove_worktree(&root, &plain).expect("非 worktree 目录应经 OS 层兜底回收成功");

    assert!(!plain.exists(), "目录树经 OS 层删除");
    assert!(root.join("a.txt").is_file(), "主仓树无损");
}

/// 边界（AC-3 幂等三形态，DD2）：① 从未存在的路径 ② 同路径连续两次 ③ 普通目录
/// 被手删后再调 → 均 `Ok`，零 panic、零残留（`NotFound` 单判据收口，不设
/// `exists()` 前置）。
#[test]
fn remove_worktree缺席幂等三形态_均ok() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-absent");
    let absent =
        std::env::temp_dir().join(format!("vcs-git-absent-idempotent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&absent);

    git::remove_worktree(&root, &absent).expect("形态一：从未存在的路径应 Ok");
    git::remove_worktree(&root, &absent).expect("形态二：同路径连续两次均应 Ok");

    let (_plain_dir, plain) = plain_dir("remove-absent-manual");
    fs_write(&plain, "gone.txt", "手动删除前的在场证明\n");
    std::fs::remove_dir_all(&plain).expect("装置手删普通目录失败");
    git::remove_worktree(&root, &plain).expect("形态三：手动删除后再调应 Ok");

    assert!(!absent.exists() && !plain.exists(), "三形态均零残留");
    assert!(root.join("a.txt").is_file(), "主仓树无损");
}

/// 正向（AC-1 孤儿自愈）：`git worktree add` 后手删 admin 登记（git 已注销、
/// 目录仍在场）→ 一次调用即 `Ok`、目录从磁盘消失、`git worktree list` 仍只主仓、
/// 登记面零残留 admin。
#[test]
fn remove_worktree已注销孤儿自愈_一次调用回收且零残留admin() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-orphan");
    let (_wt_dir, wt) = worktree_slot("remove-orphan");

    git::add_worktree(&root, &wt, "change/orphan").expect("add 应成功");
    let admins = admin_names(&root);
    assert_eq!(admins.len(), 1, "前置：admin 登记恰一条: {admins:?}");
    std::fs::remove_dir_all(root.join(".git").join("worktrees").join(&admins[0]))
        .expect("装置手删 admin 失败");

    let wt_posix = wt.to_string_lossy().replace('\\', "/");
    let list_before = git(&root, &["worktree", "list"]);
    assert!(
        !list_before.contains(wt_posix.as_str()),
        "前置：git 已注销该位（list 不显示）: {list_before}"
    );
    assert!(admin_names(&root).is_empty(), "前置：登记面零 admin");
    assert!(wt.is_dir(), "前置：目录仍在场（孤儿形态）");
    assert!(
        git(&root, &["worktree", "prune"]).trim().is_empty(),
        "前置：prune 无事可做（登记面已空）"
    );

    git::remove_worktree(&root, &wt).expect("已注销孤儿应经兜底回收成功（零重试）");

    assert!(!wt.exists(), "孤儿目录从磁盘消失");
    let list_after = git(&root, &["worktree", "list"]);
    assert!(
        !list_after.contains(wt_posix.as_str()),
        "list 仍只主仓: {list_after}"
    );
    assert!(admin_names(&root).is_empty(), "登记面零残留 admin");
    assert!(root.join("a.txt").is_file(), "主仓树无损");
}

/// 正向（AC-3 的 `prune` 承担面）：worktree 目录被手删而 git 登记仍在场 →
/// `remove_worktree` 仍 `Ok`，且登记面无残留 admin（git 段自身清登记 / 兜底段
/// 后由 `prune` 清——两路收口同一可观测面）。
#[test]
fn remove_worktree目录缺席但admin登记在场_ok且零残留admin() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-dir-gone");
    let (_wt_dir, wt) = worktree_slot("remove-dir-gone");

    git::add_worktree(&root, &wt, "change/dir-gone").expect("add 应成功");
    assert_eq!(admin_names(&root).len(), 1, "前置：admin 登记在案");
    std::fs::remove_dir_all(&wt).expect("装置手删 worktree 目录失败");
    assert!(!wt.exists(), "前置：目录已不在场");
    let wt_posix = wt.to_string_lossy().replace('\\', "/");
    assert!(
        git(&root, &["worktree", "list"]).contains(wt_posix.as_str()),
        "前置：登记仍在场（目录缺席形态）"
    );

    git::remove_worktree(&root, &wt).expect("目录缺席但登记在案应 Ok（幂等收口）");

    assert!(admin_names(&root).is_empty(), "登记面零残留 admin");
    assert!(
        !git(&root, &["worktree", "list"]).contains(wt_posix.as_str()),
        "list 不再列出该位"
    );
}

/// 正向（AC-5 `prune` 不误伤）：另一活 worktree 不受波及——只把 A 造成孤儿
/// 形态（删 A 的 admin）→ 对 A 走兜底分支 → B 仍在 `git worktree list`、B 目录
/// 在场、B 的 admin 登记仍在。
#[test]
fn remove_worktree兜底后另一活worktree零波及() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-two");
    let (_wt_dir_a, wt_a) = worktree_slot_named("remove-two", "wt-a");
    let (_wt_dir_b, wt_b) = worktree_slot_named("remove-two", "wt-b");

    git::add_worktree(&root, &wt_a, "change/first").expect("add A 应成功");
    let after_a = admin_names(&root);
    assert_eq!(after_a.len(), 1, "前置：A 的 admin 登记恰一条: {after_a:?}");
    git::add_worktree(&root, &wt_b, "change/second").expect("add B 应成功");
    let after_b = admin_names(&root);
    let admin_b: Vec<&String> = after_b
        .iter()
        .filter(|name| !after_a.contains(name))
        .collect();
    assert_eq!(admin_b.len(), 1, "前置：B 的新增 admin 恰一条: {after_b:?}");

    // A 造成孤儿形态（只删 A 的 admin）
    std::fs::remove_dir_all(root.join(".git").join("worktrees").join(&after_a[0]))
        .expect("装置手删 A 的 admin 失败");

    git::remove_worktree(&root, &wt_a).expect("孤儿 A 应经兜底回收成功");

    assert!(!wt_a.exists(), "A 目录已回收");
    assert!(wt_b.is_dir(), "B 目录仍在场（`remove_dir_all` 零波及）");
    let list = git(&root, &["worktree", "list"]);
    let wt_b_posix = wt_b.to_string_lossy().replace('\\', "/");
    assert!(
        list.contains(wt_b_posix.as_str()),
        "B 仍在 git worktree list: {list}"
    );
    assert_eq!(
        admin_names(&root),
        vec![(*admin_b[0]).clone()],
        "B 的 admin 登记仍在场（A 的已被 prune / git 段清走）"
    );
}

/// 异常（AC-4 护栏五形态）：主仓自身 / 主仓祖先 / 主仓子目录 / 文件系统根 /
/// 空路径 → 均 `Err` 且磁盘零变化（`git worktree list` 与登记面快照逐一相等，
/// 主仓文件全在）。
#[test]
fn remove_worktree护栏五形态_皆err且磁盘零变化() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("guard");
    let sub = root.join("openspec");
    std::fs::create_dir_all(&sub).expect("建主仓子目录失败");
    let fs_root = root
        .ancestors()
        .last()
        .expect("文件系统根应在案")
        .to_path_buf();
    let ancestor = root.parent().expect("仓根应有父目录").to_path_buf();
    let forms: [(&str, PathBuf); 5] = [
        ("主仓自身", root.clone()),
        ("主仓祖先", ancestor),
        ("主仓子目录", sub.clone()),
        ("文件系统根", fs_root),
        ("空路径", PathBuf::new()),
    ];
    let admins_before = admin_names(&root);
    let list_before = git(&root, &["worktree", "list"]);

    for (label, target) in forms {
        let error = git::remove_worktree(&root, &target).expect_err("护栏命中应 Err");
        assert!(
            error.contains("拒绝回收 worktree"),
            "{label}：Err 记因护栏拒绝，实际: {error}"
        );
        assert!(
            error.contains("自身、其祖先、其子目录或文件系统根"),
            "{label}：Err 携拒绝因（四类）: {error}"
        );
    }

    assert_eq!(admin_names(&root), admins_before, "登记面快照逐一相等");
    assert_eq!(
        git(&root, &["worktree", "list"]),
        list_before,
        "git 登记面零变化（护栏先于任何副作用）"
    );
    assert!(root.join("a.txt").is_file(), "主仓树无损");
    assert!(sub.is_dir(), "主仓子目录未被删");
    assert!(root.join(".git").is_dir(), "主仓 .git 完整");
}

/// 边界（护栏先于副作用）：护栏命中时 git 段未跑过——活 worktree 的登记与目录
/// 均零触碰（不是「目录尚未删」，而是根本没进入删除段）。
#[test]
fn remove_worktree护栏先于副作用_活worktree登记与目录零触碰() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("guard-first");
    let (_wt_dir, wt) = worktree_slot("guard-first");
    git::add_worktree(&root, &wt, "change/live").expect("add 应成功");
    let admins_before = admin_names(&root);
    assert_eq!(admins_before.len(), 1, "前置：活 worktree 登记在案");

    git::remove_worktree(&root, &root).expect_err("主仓自身应被护栏拒绝");

    assert_eq!(admin_names(&root), admins_before, "活 worktree 登记零触碰");
    assert!(wt.is_dir(), "活 worktree 目录零触碰");
    assert!(
        git(&root, &["worktree", "list"])
            .contains(wt.to_string_lossy().replace('\\', "/").as_str()),
        "活 worktree 仍在 list"
    );
}

/// 正向（D4 反向判据：护栏不削弱自愈面）：合法 worktree（落位根下、与主仓
/// 同级）与落位根子目录均不被拒——黑名单之外零形态判据。
#[test]
fn remove_worktree护栏不外溢_合法worktree与落位根子目录均不拒() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("guard-scope");
    let (slot_dir, wt) = worktree_slot("guard-scope");
    git::add_worktree(&root, &wt, "change/legal").expect("add 应成功");
    git::remove_worktree(&root, &wt).expect("合法 worktree 不应被护栏拒绝");
    assert!(!wt.exists(), "合法 worktree 已回收");

    // 落位根子目录：非法 worktree（git 段必失败）但不在黑名单 → 走两段式并 Ok
    let stray = slot_dir.path().join("stray");
    std::fs::create_dir_all(&stray).expect("装置建落位根子目录失败");
    git::remove_worktree(&root, &stray).expect("落位根子目录不应被护栏拒绝");
    assert!(!stray.exists(), "落位根子目录经兜底删除");
}

/// 边界（DD1 归一）：同一 `main_root` 的四类文本形态（原样 / 尾部带分隔符 /
/// `\\?\` 扩展前缀 / 大小写变异；POSIX 侧为符号链接）经护栏归一后拒绝结论不变。
#[test]
fn remove_worktree护栏归一形态_同一主仓形态差异不改结论() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("guard-normalize");
    let mut forms: Vec<(&str, PathBuf)> =
        vec![("原样", root.clone()), ("尾部带分隔符", root.join(""))];
    #[cfg(windows)]
    {
        forms.push((
            "扩展前缀形",
            PathBuf::from(format!(r"\\?\{}", root.display())),
        ));
        forms.push((
            "大小写变异",
            PathBuf::from(root.display().to_string().to_uppercase()),
        ));
    }
    for (label, target) in forms {
        let error = git::remove_worktree(&root, &target).expect_err("归一后应同判护栏拒绝");
        assert!(
            error.contains("拒绝回收 worktree"),
            "{label} 形态应被护栏拒绝，实际: {error}"
        );
    }
    assert!(root.join("a.txt").is_file(), "主仓树无损");

    // POSIX：符号链接指向主仓（canonicalize 消解）——同一结论
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let link = std::env::temp_dir().join(format!("vcs-git-guard-link-{}", std::process::id()));
        let _ = std::fs::remove_file(&link);
        symlink(&root, &link).expect("装置建符号链接失败");
        let error = git::remove_worktree(&root, &link).expect_err("符号链接形态应同判护栏拒绝");
        assert!(
            error.contains("拒绝回收 worktree"),
            "符号链接形态应被护栏拒绝，实际: {error}"
        );
        let _ = std::fs::remove_file(&link);
    }
}

/// 异常（AC-6 两段式皆失败）：worktree 内文件以无 delete 共享的句柄持有（另
/// 一进程持有的本进程等价面）→ `Err` 携五段成因（路径 / 原 git 错误 / OS 删除
/// 错误 / 占用成因清单 / 双源对账），文案单行且**祈使短语缺席**（DD3：锚定祈使
/// 短语而非裸命令串——原文必回显 `git worktree remove --force <path>`）。
#[cfg(windows)]
#[test]
fn remove_worktree两段皆失败_err携五段成因且零祈使短语() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-both-fail");
    let (_wt_dir, wt) = worktree_slot("remove-both-fail");
    git::add_worktree(&root, &wt, "change/locked").expect("add 应成功");
    fs_write(&wt, "locked.txt", "被持有的文件\n");
    let holder = hold_without_delete_share(&wt.join("locked.txt"));

    let error = git::remove_worktree(&root, &wt).expect_err("两段式皆失败应 Err");
    drop(holder);

    assert!(
        error.contains(&wt.to_string_lossy().into_owned()),
        "第一段：携 worktree 绝对路径: {error}"
    );
    assert!(
        error.contains("git worktree remove --force") && error.contains("失败（退出码"),
        "第二段：携原 git 错误原文（含命令串与退出码）: {error}"
    );
    assert!(
        error.contains("OS 删除错误"),
        "第三段：携 OS 层删除错误: {error}"
    );
    for phrase in ["终端 cwd", "编辑器", "常驻子进程", "杀毒实时扫描", "云同步"]
    {
        assert!(
            error.contains(phrase),
            "第四段：成因清单含「{phrase}」: {error}"
        );
    }
    assert!(
        error.contains("① git worktree list") && error.contains("② 直接检查 worktree 落位根"),
        "第五段：携双源对账（git 登记面 + 落位根直检）: {error}"
    );
    assert!(
        !error.contains("请手动执行"),
        "祈使短语缺席（单源手动指引不得出现）: {error}"
    );
    assert!(
        !error.contains("对账并手动清理"),
        "祈使短语缺席（单源对账收尾不得出现）: {error}"
    );
    assert!(
        !error.contains('\n'),
        "失败面文案单行（零换行——walker 的 step detail 渲染面）: {error}"
    );
}

/// 正向（AC-6 后半）：释放占用后**立即**同路径再调 → `Ok`、目录消失（零
/// sleep、零退避、零重试——两次调用间隔无等待窗口）。
#[cfg(windows)]
#[test]
fn remove_worktree释放占用后立即收敛_ok且目录消失() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-released");
    let (_wt_dir, wt) = worktree_slot("remove-released");
    git::add_worktree(&root, &wt, "change/released").expect("add 应成功");
    fs_write(&wt, "locked.txt", "被持有的文件\n");
    let holder = hold_without_delete_share(&wt.join("locked.txt"));

    git::remove_worktree(&root, &wt).expect_err("持有期应 Err");
    drop(holder);
    git::remove_worktree(&root, &wt).expect("释放后立即重试应收敛为 Ok");

    assert!(!wt.exists(), "目录已回收");
    assert!(admin_names(&root).is_empty(), "登记面零残留 admin");
}

/// 异常（DD9 git 段失败无例外处置）：PATH 置空（git 不可用）→ ① 普通目录仍经
/// OS 层兜底 `Ok`（不因 git 缺失拒绝自愈）② 普通目录 + 真占用 → `Err` 同时携
/// 「git 不可用」与 OS 删除错误（两段失败因各可辨）。
#[test]
fn remove_worktree_git不可用_兜底不设分支且两段失败双因可辨() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-no-git");
    let (_plain_dir, plain) = plain_dir("remove-no-git");
    fs_write(&plain, "stray.txt", "git 不可用下的普通目录\n");
    let (_slot_dir, occupied) = worktree_slot("remove-no-git-locked");
    std::fs::create_dir_all(&occupied).expect("装置建占用目录失败");
    fs_write(&occupied, "locked.txt", "被持有的文件\n");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let fallback = git::remove_worktree(&root, &plain);
    #[cfg(windows)]
    let occupied_result = {
        let holder = hold_without_delete_share(&occupied.join("locked.txt"));
        let result = git::remove_worktree(&root, &occupied);
        drop(holder);
        result
    };
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    fallback.expect("git 不可用不应拒绝兜底回收（DD9：git 段失败的 Err 同归第二段）");
    assert!(!plain.exists(), "普通目录经 OS 层删除");

    #[cfg(windows)]
    {
        let error = occupied_result.expect_err("git 不可用 + 真占用应 Err");
        assert!(
            error.contains("git 不可用（PATH 未发现 git）"),
            "Err 携 git 环境缺失因: {error}"
        );
        assert!(error.contains("OS 删除错误"), "Err 携 OS 层删除因: {error}");
    }
}

/// 边界（D9 / `\\?\` 前缀）：非 git 的深层目录树（绝对路径 > 260 字符，含一个
/// 文件）→ git 段必失败 → 兜底经扩展前缀删除成功 → `Ok`、深目录树消失。
#[cfg(windows)]
#[test]
fn remove_worktree_windows超长路径_兜底经扩展前缀删除成功() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-long-path");
    let (_plain_dir, plain) = plain_dir("remove-long-path");
    let mut deep = plain.clone();
    while deep.to_string_lossy().chars().count() <= 300 {
        deep = deep.join("deep-segment-000000");
    }
    std::fs::create_dir_all(&deep).expect("装置建超长路径目录失败");
    std::fs::write(deep.join("payload.txt"), "deep\n").expect("装置写超长路径文件失败");
    assert!(
        deep.to_string_lossy().chars().count() > 260,
        "前置：绝对路径 > 260 字符（实际 {}）",
        deep.to_string_lossy().chars().count()
    );

    git::remove_worktree(&root, &deep).expect("超长路径应经扩展前缀兜底删除成功");

    assert!(!deep.exists(), "深目录树已删除");
}

/// 边界（目标位点是普通文件）：`remove_dir_all` 非 `NotFound` 形态 → 结果二值
/// 可考（`Err` 携双源 或 `Ok`）；硬判据 = 不 panic、文件内容未被改写、主仓零
/// 变化、再次调用结果稳定。
#[test]
fn remove_worktree目标位点是普通文件_不panic且内容完整结果稳定() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("remove-file-target");
    let (_plain_dir, plain) = plain_dir("remove-file-target");
    let target = plain.join("not-a-directory.txt");
    std::fs::write(&target, "普通文件\n").expect("装置写普通文件失败");

    let first = git::remove_worktree(&root, &target);
    assert_eq!(
        std::fs::read_to_string(&target).expect("文件仍在场可读"),
        "普通文件\n",
        "文件内容未被改写"
    );
    assert!(root.join("a.txt").is_file(), "主仓零变化");

    let second = git::remove_worktree(&root, &target);
    match (&first, &second) {
        (Ok(()), Ok(())) => {}
        (Err(first_error), Err(second_error)) => {
            assert!(
                first_error.contains("① git worktree list")
                    && first_error.contains("② 直接检查 worktree 落位根"),
                "Err 携双源对账: {first_error}"
            );
            assert!(
                second_error.contains("① git worktree list"),
                "再次调用同形态: {second_error}"
            );
        }
        _ => panic!("两次调用结果应稳定（一次 Err 一次 Ok 属不稳定）: {first:?} / {second:?}"),
    }
}

/// 边界（DD5 `prune` 失败静默）：`main_root` 非 git 目录 + 目标路径缺席 →
/// `prune` 段必失败 → 仍 `Ok`（不升级为 `Err`、不把 git 错误外泄为用户面文案）。
#[test]
fn remove_worktree_prune失败静默_非git主仓缺席目标仍ok() {
    let _guard = lock_path();
    let (_dir, plain_root) = plain_dir("remove-prune-fail");
    let absent = std::env::temp_dir().join(format!("vcs-git-absent-prune-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&absent);

    git::remove_worktree(&plain_root, &absent)
        .expect("prune 失败应静默吞（目标目录不在场即主目标达成）");

    assert!(!absent.exists(), "缺席目标零产生");
    assert!(
        plain_root.join("not-a-repo.txt").is_file(),
        "非 git 主仓零变化"
    );
}

// ---------------------------------------------------------------------------
// 归档子命令族 + ProcessArchiveVcs（test-design「git.rs -> git_test.rs」扩展节：
// 真实 git tempdir 夹具——D9 实验定锚的测试形态同源。挂 AC-3 提交 / 合入 /
// 祖先判定、AC-4 无关 staged / untracked 前后保持机械断言、AC-8 冲突 abort
// 收口。worktree 侧分支铸造经既有 add_worktree 装置；PATH 隔离窗口沿 crate
// TEST_PATH_LOCK 互斥纪律）
// ---------------------------------------------------------------------------

use crate::ProcessArchiveVcs;
use orchestration::port::RebaseOutcome;
use orchestration::ArchiveVcsPort;

/// 被测执行器（无状态；构造锚 + 各归档族用例共用入口）。
fn archive_vcs() -> ProcessArchiveVcs {
    ProcessArchiveVcs::new()
}

/// 提交身份注入的 commit 装置（不依赖全局 gitconfig）。
fn commit_all_fixture(dir: &Path, message: &str) {
    git(
        dir,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
}

/// 无状态装配入口：`new()` 与 `Default::default()` 等值可重复构造（组合根按
/// 需铸的编译锚——`ProcessWorktree` 先例同型）。
#[test]
fn process_archive_vcs构造锚_new与default等值可重复构造() {
    let _a = archive_vcs();
    let _b = ProcessArchiveVcs;
    let _c = ProcessArchiveVcs::new();
}

/// dirty 全域两态：未跟踪 + 已修改文件在场 → true；干净仓 → false（空 paths
/// = 全域口径）。
#[test]
fn dirty全域两态_脏仓true干净仓false() {
    let _guard = lock_path();
    let vcs = archive_vcs();

    let (_dir, root) = init_repo("arch-dirty-yes");
    fs_write(&root, "a.txt", "已修改\n"); // 修改既有
    fs_write(&root, "untracked.txt", "未跟踪\n");
    assert!(vcs.dirty(&root, &[]), "未跟踪 + 已修改在场 → true");

    let (_clean_dir, clean) = init_repo("arch-dirty-no");
    assert!(!vcs.dirty(&clean, &[]), "干净仓 → false");
}

/// dirty gitignore 面不计：`.gitignore` 圈定目录（node_modules/）在场 → 全域
/// 与 pathspec 两口径均 false（D9⑤——bootstrap 产物不脏不进提交的前提锚）。
#[test]
fn dirty_gitignore面不计_全域与pathspec均false() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-dirty-ignored");
    fs_write(&root, ".gitignore", "node_modules/\n");
    git(&root, &["add", ".gitignore"]);
    commit_all_fixture(&root, "ignore");
    let modules = root.join("node_modules");
    std::fs::create_dir_all(&modules).expect("建 gitignore 目录失败");
    fs_write(&modules, "pkg.js", "bootstrap 产物\n");

    assert!(!vcs.dirty(&root, &[]), "全域口径：gitignore 面不计 → false");
    assert!(
        !vcs.dirty(&root, &["node_modules"]),
        "pathspec 口径同 false"
    );
}

/// dirty pathspec 圈定：无关脏文件 + paths 指向干净区 → false；paths 指向脏区
/// → true（D11 finalize 脏探测的圈定语义——只看归档两路径）。
#[test]
fn dirty_pathspec圈定_圈外脏不误报圈内脏即真() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-dirty-pathspec");
    std::fs::create_dir_all(root.join("openspec/changes/archive")).expect("建圈定区失败");
    fs_write(&root, "unrelated.txt", "圈外脏文件\n"); // 未跟踪脏（圈外）
    fs_write(&root, "openspec/changes/archive/x.txt", "圈内\n"); // 未跟踪脏（圈内）

    assert!(
        !vcs.dirty(&root, &["openspec/changes/absent"]),
        "paths 指向干净区 → false（圈外脏不误报）"
    );
    assert!(
        vcs.dirty(&root, &["openspec/changes/archive/x.txt"]),
        "paths 指向脏区 → true"
    );
}

/// commit_all 提交编辑集：worktree 内修改 + untracked 新文件 → 恰一个新提交
///（rev-list 计数 +1）且两者入树（ls-tree 命中）；gitignored 目录不入提交
///（D9⑤）。
#[test]
fn commit_all_提交编辑集_恰一提交且gitignore不入() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-commit-all");
    fs_write(&root, "a.txt", "已修改\n"); // 修改既有
    fs_write(&root, "new-file.txt", "spec 同步产物\n"); // untracked 新文件
    fs_write(&root, ".gitignore", "node_modules/\n");
    git(&root, &["add", ".gitignore"]);
    commit_all_fixture(&root, "ignore");
    let before = git(&root, &["rev-list", "--count", "HEAD"])
        .trim()
        .to_owned();
    let modules = root.join("node_modules");
    std::fs::create_dir_all(&modules).expect("建目录失败");
    fs_write(&modules, "dep.js", "依赖产物\n");

    vcs.commit_all(&root, "archive: demo-change")
        .expect("commit_all 应成功");

    let after = git(&root, &["rev-list", "--count", "HEAD"])
        .trim()
        .to_owned();
    assert_eq!(
        after,
        (before.parse::<u32>().expect("计数可解析") + 1).to_string(),
        "恰一个新提交"
    );
    let tree = git(&root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(tree.contains("a.txt"), "修改入树");
    assert!(tree.contains("new-file.txt"), "untracked 产物入树");
    assert!(!tree.contains("node_modules"), "gitignored 目录不入提交");
    assert!(!vcs.dirty(&root, &[]), "提交后全域干净（工作区收净）");
}

/// commit_all 信息 argv 直传：CJK / 含空格信息（`archive: <name>` 形态）→
/// `git log -1 --format=%s` 与入参逐字一致（零 shell 包装——R7 引号形态不适
/// 用的正面锚）。
#[test]
fn commit_all_信息argv直传_cjk与空格逐字保真() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-commit-msg");
    fs_write(&root, "b.txt", "新文件\n");

    let message = "archive: 中文 change 名 with spaces";
    vcs.commit_all(&root, message).expect("commit_all 应成功");

    let subject = git(&root, &["log", "-1", "--format=%s"]);
    assert_eq!(
        subject.trim(),
        message,
        "提交信息逐字一致（argv 直传零 shell 包装）"
    );
}

/// branch_merged 三态：分支未合入 → false；merge 后 → true；缺分支 → Err
///（>1 退出码面，非 false——与 miss 可辨）。
#[test]
fn branch_merged三态_未合入false合入true缺分支err() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-merged");
    let (_wt_dir, wt) = worktree_slot("arch-merged");
    git::add_worktree(&root, &wt, "change/feature").expect("建域应成功");
    fs_write(&wt, "feat.txt", "分支提交\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "feat");

    assert!(
        !vcs.branch_merged(&root, "change/feature")
            .expect("判定应 Ok"),
        "分支未合入 → false"
    );
    git(&root, &["merge", "--no-edit", "change/feature"]);
    assert!(
        vcs.branch_merged(&root, "change/feature")
            .expect("判定应 Ok"),
        "merge 后 → true"
    );
    let error = vcs
        .branch_merged(&root, "change/absent")
        .expect_err("缺分支应 Err");
    assert!(
        error.contains("merge-base"),
        "缺分支 Err（非 false——与 miss 可辨）: {error}"
    );
}

#[test]
fn rebase重放主仓ff_无关staged与untracked原样保留且零merge_commit() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-rebase-ff");
    let (_wt_dir, wt) = worktree_slot("arch-rebase-ff");
    git::add_worktree(&root, &wt, "change/ff").expect("建域应成功");
    fs_write(&wt, "feat.txt", "分支新增\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "feat");
    let branch_tip = git(&wt, &["rev-parse", "HEAD"]).trim().to_owned();

    // 无关 staged + 无关 untracked（主仓侧预置）
    fs_write(&root, "other.txt", "无关 staged\n");
    git(&root, &["add", "other.txt"]);
    fs_write(&root, "untracked.txt", "无关 untracked\n");
    let status_before = git(&root, &["status", "--porcelain"]);
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();

    let outcome = vcs
        .rebase_branch(&wt, &onto)
        .expect("主仓未前进形态重放应成功");
    assert_eq!(outcome, RebaseOutcome::Rebased, "重放完成 → Rebased");
    vcs.ff_merge(&root, "change/ff").expect("主仓快进应成功");

    assert_eq!(head_sha(&root), branch_tip, "快进后主仓 HEAD = 分支 tip");
    let parents = git(&root, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(
        parents.split_whitespace().count() - 1,
        1,
        "HEAD 单亲（零 merge commit——线性合入锚）"
    );
    let status_after = git(&root, &["status", "--porcelain"]);
    assert_eq!(
        status_after, status_before,
        "无关 staged / untracked 前后逐字一致"
    );
    assert!(
        status_after.contains("A  other.txt"),
        "staged 条目原样 staged"
    );
    assert!(status_after.contains("?? untracked.txt"), "untracked 原样");
}

#[test]
fn rebase主仓前进_重放后ff线性合入零merge_commit() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-rebase-diverged");
    let (_wt_dir, wt) = worktree_slot("arch-rebase-diverged");
    git::add_worktree(&root, &wt, "change/diverged").expect("建域应成功");

    // 分支前进（worktree 侧提交）
    fs_write(&wt, "feat.txt", "分支提交\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "feat");
    let branch_before = git(&wt, &["rev-parse", "HEAD"]).trim().to_owned();
    // 主仓前进（旧 non-ff 前提；index 保持干净）
    fs_write(&root, "main-line.txt", "主仓前进\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main");
    let main_tip = head_sha(&root);
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();

    let outcome = vcs.rebase_branch(&wt, &onto).expect("异文件重放应成功");
    assert_eq!(outcome, RebaseOutcome::Rebased, "重放完成");
    let branch_after = git(&wt, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_ne!(
        branch_after, branch_before,
        "分支提交被重放改写（新 sha——change 私有分支面）"
    );
    vcs.ff_merge(&root, "change/diverged").expect("快进应成功");

    // 线性形态：HEAD 单亲、HEAD = 分支重放 tip、其单亲 = 主仓前进 tip
    assert_eq!(head_sha(&root), branch_after, "主仓 HEAD = 分支重放 tip");
    let parents = git(&root, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    let fields: Vec<&str> = parents.split_whitespace().collect();
    assert_eq!(fields.len(), 2, "单亲线性（零 merge commit）");
    assert_eq!(fields[1], main_tip, "分支重放提交立于主仓前进 tip 之上");
    assert!(!root.join(".git/MERGE_HEAD").exists(), "主仓零 merge 残态");
}

#[test]
fn rebase非冲突失败_显式err引导且worktree零破坏() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-rebase-refused");
    let (_wt_dir, wt) = worktree_slot("arch-rebase-refused");
    git::add_worktree(&root, &wt, "change/dirty").expect("建域应成功");
    fs_write(&root, "main-line.txt", "主仓前进\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main");
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();
    fs_write(&wt, "a.txt", "未提交改动\n"); // 已跟踪文件未 staged 修改

    let error = vcs
        .rebase_branch(&wt, &onto)
        .expect_err("worktree 脏应显式 Err");

    assert!(
        error.contains("归档合入失败") && error.contains("git rebase --abort"),
        "Err 带 rebase 语境与 abort 收口引导: {error}"
    );
    assert_eq!(
        std::fs::read_to_string(wt.join("a.txt")).expect("读文件失败"),
        "未提交改动\n",
        "worktree 未提交改动零破坏"
    );
}

#[test]
fn rebase空提交丢弃_内容已在主仓幂等吸收() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-rebase-empty");
    let (_wt_dir, wt) = worktree_slot("arch-rebase-empty");
    git::add_worktree(&root, &wt, "change/empty").expect("建域应成功");

    // 分支提交与主仓提交内容一致（重放变空）
    fs_write(&wt, "feat.txt", "同一内容\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "feat");
    fs_write(&root, "feat.txt", "同一内容\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "same feat");
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();

    let outcome = vcs.rebase_branch(&wt, &onto).expect("空重放应成功（丢弃）");
    assert_eq!(outcome, RebaseOutcome::Rebased, "空提交丢弃 → Rebased");
    assert_eq!(
        git(&wt, &["rev-parse", "HEAD"]).trim(),
        head_sha(&root),
        "空提交丢弃 → 分支 tip = 主仓 tip"
    );
    assert!(
        vcs.branch_merged(&root, "change/empty").expect("判定应 Ok"),
        "已合入判定幂等吸收"
    );
    vcs.ff_merge(&root, "change/empty")
        .expect("Already up to date 亦 Ok");
}

#[test]
fn rebase冲突_冲突态保留在worktree且清单返回() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-rebase-conflict");
    // CJK 文件种入基线提交（双侧同为修改 → UU 形态；`-z` 清单照常裸出）
    fs_write(&root, "中文说明.md", "基线\n");
    git(&root, &["add", "中文说明.md"]);
    commit_all_fixture(&root, "seed cjk");
    let (_wt_dir, wt) = worktree_slot("arch-rebase-conflict");
    git::add_worktree(&root, &wt, "change/conflict").expect("建域应成功");

    // 双方改同一批文件：a.txt（ASCII）+ 中文说明.md（CJK 文件名——R6 形态）
    fs_write(&wt, "a.txt", "分支版本\n");
    fs_write(&wt, "中文说明.md", "分支版本\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "branch edit");
    fs_write(&root, "a.txt", "主仓版本\n");
    fs_write(&root, "中文说明.md", "主仓版本\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main edit");
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();

    let outcome = vcs
        .rebase_branch(&wt, &onto)
        .expect("冲突应为 Ok(Conflicted)——冲突态保留不 abort");

    let files = match outcome {
        RebaseOutcome::Conflicted(files) => files,
        other => panic!("冲突应 Conflicted，实际: {other:?}"),
    };
    assert_eq!(
        files,
        vec!["a.txt".to_owned(), "中文说明.md".to_owned()],
        "清单精确（`-z` 归一——CJK 裸路径零引号零转义，R6 读取面锚）"
    );
    assert!(
        files.iter().all(|file| !file.contains('"')),
        "全清单零引号形态（R6）"
    );
    // 展示面用 quotepath 关闭口径（产品读取面为 `status --porcelain -z`——
    // 裸形态已由上方 Conflicted 清单断言锚定；此处只锚 UU 双路径在场）
    let status = git(
        &wt,
        &["-c", "core.quotepath=false", "status", "--porcelain"],
    );
    assert!(
        status.contains("UU a.txt") && status.contains("UU 中文说明.md"),
        "冲突态保留在 worktree（UU 双路径在场）: {status}"
    );
    assert!(
        std::fs::read_to_string(wt.join("a.txt"))
            .expect("读文件失败")
            .contains("<<<<<<<"),
        "冲突标记在场（未自动裁决）"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).expect("读文件失败"),
        "主仓版本\n",
        "主仓工作区零冲突态（冲突囚于 worktree——主仓红线结构性收窄）"
    );
}

#[test]
fn worktree_snapshot冲突态_rebase半程四字段齐备() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-snap-conflict");
    let (_wt_dir, wt) = worktree_slot("arch-snap-conflict");
    git::add_worktree(&root, &wt, "change/snap").expect("建域应成功");

    fs_write(&wt, "a.txt", "分支版本\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "branch edit");
    let branch_tip = git(&wt, &["rev-parse", "HEAD"]).trim().to_owned();
    fs_write(&root, "a.txt", "主仓版本\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main edit");
    let main_tip = head_sha(&root);
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();
    let outcome = vcs.rebase_branch(&wt, &onto).expect("冲突 Ok");
    assert!(
        matches!(outcome, RebaseOutcome::Conflicted(_)),
        "冲突态在场前提"
    );

    let snapshot = vcs.worktree_snapshot(&wt).expect("冲突态快照应 Ok");
    assert_eq!(
        snapshot.head, main_tip,
        "head = onto tip（rebase 半程 HEAD——分支引用不前移）"
    );
    assert_eq!(
        snapshot.rebase_head.as_deref(),
        Some(branch_tip.as_str()),
        "rebase_head = Some(重放中提交 = 分支 tip sha)"
    );
    let entry = snapshot
        .status
        .iter()
        .find(|entry| entry.path == "a.txt")
        .expect("冲突路径 porcelain 条目在场");
    assert_eq!((entry.x, entry.y), ('U', 'U'), "UU 条目（XY 双 U）");
    let stages: Vec<u8> = snapshot
        .index
        .iter()
        .filter(|entry| entry.path == "a.txt")
        .map(|entry| entry.stage)
        .collect();
    assert_eq!(stages, vec![1, 2, 3], "索引三方条目 stage 1/2/3");
    // 逐字段对拍 `ls-files -s` 原始输出（mode / hash / stage）
    let raw = git(&wt, &["ls-files", "-s", "a.txt"]);
    assert!(!raw.trim().is_empty(), "原始索引输出在场");
    for line in raw.lines() {
        let mut fields = line.split_whitespace();
        let mode = fields.next().expect("mode 字段");
        let hash = fields.next().expect("hash 字段");
        let stage = fields.next().expect("stage 字段");
        assert!(
            snapshot.index.iter().any(|entry| {
                entry.path == "a.txt"
                    && entry.mode == mode
                    && entry.hash == hash
                    && entry.stage == stage.parse::<u8>().expect("stage 数")
            }),
            "索引条目与 ls-files -s 原始输出逐字段对拍: {line}"
        );
    }
}

/// worktree_snapshot 解析形态族（`-z` 归一）：干净仓（rebase_head=None / status
/// 空 / index 全 stage-0）；混合态——untracked（XY "?","?"）、modified 未
/// staged（XY " M"）、staged rename（R 码 + NUL 双段——新路径记录、原路径段
/// 跳过）、CJK 路径裸形态（零引号——R6 对偶锚）逐条目断言解析像。
#[test]
fn worktree_snapshot解析形态族_干净与混合态() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-snap-shapes");

    // 干净仓基线
    let clean = vcs.worktree_snapshot(&root).expect("干净仓快照应 Ok");
    assert_eq!(clean.rebase_head, None, "非 rebase 态 → rebase_head=None");
    assert!(clean.status.is_empty(), "干净仓 status 空");
    assert!(
        !clean.index.is_empty() && clean.index.iter().all(|entry| entry.stage == 0),
        "index 全 stage-0"
    );

    // 混合态：modified 未 staged + untracked + staged rename + staged CJK 新文件
    std::fs::create_dir_all(root.join("中文目录")).expect("建 CJK 目录失败");
    fs_write(&root, "a.txt", "已修改未 staged\n");
    fs_write(&root, "untracked.txt", "未跟踪\n");
    fs_write(&root, "old-name.txt", "旧路径\n");
    git(&root, &["add", "old-name.txt"]);
    commit_all_fixture(&root, "seed old");
    git(&root, &["mv", "old-name.txt", "renamed.txt"]);
    fs_write(&root, "中文目录/文档.md", "中文路径\n");
    git(&root, &["add", "中文目录/文档.md"]);

    let snapshot = vcs.worktree_snapshot(&root).expect("混合态快照应 Ok");
    let find = |path: &str| {
        snapshot
            .status
            .iter()
            .find(|entry| entry.path == path)
            .unwrap_or_else(|| panic!("条目缺席: {path}"))
    };
    let untracked = find("untracked.txt");
    assert_eq!(
        (untracked.x, untracked.y),
        ('?', '?'),
        "untracked XY = \"?\",\"?\""
    );
    let modified = find("a.txt");
    assert_eq!(
        (modified.x, modified.y),
        (' ', 'M'),
        "modified 未 staged XY = \" M\""
    );
    let renamed = find("renamed.txt");
    assert_eq!(renamed.x, 'R', "staged rename R 码（NUL 双段记新路径）");
    assert!(
        snapshot
            .status
            .iter()
            .all(|entry| entry.path != "old-name.txt"),
        "rename 原路径段跳过（零重复记录）"
    );
    let cjk = find("中文目录/文档.md");
    assert_eq!(cjk.x, 'A', "staged CJK 新文件");
    assert_eq!(
        cjk.path, "中文目录/文档.md",
        "CJK 路径裸形态零引号零转义（R6 对偶锚）"
    );
    assert!(
        snapshot
            .status
            .iter()
            .all(|entry| !entry.path.contains('"')),
        "全清单零引号形态（`-z` 归一）"
    );
}

#[test]
fn rebase_continue续走收口_信息沿用且无关改动零吞并() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-continue");
    // b.txt 种入基线（worktree 侧可用的已跟踪无关面）
    fs_write(&root, "b.txt", "已跟踪基线\n");
    git(&root, &["add", "b.txt"]);
    commit_all_fixture(&root, "seed b");
    let (_wt_dir, wt) = worktree_slot("arch-continue");
    git::add_worktree(&root, &wt, "change/resolve").expect("建域应成功");

    fs_write(&wt, "a.txt", "分支版本\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "branch edit");
    fs_write(&root, "a.txt", "主仓版本\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main edit");
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();

    let outcome = vcs.rebase_branch(&wt, &onto).expect("冲突 Ok");
    assert!(
        matches!(outcome, RebaseOutcome::Conflicted(_)),
        "冲突态在场前提"
    );

    // 无关 untracked（收口不得吞并的面）。无关 tracked 未 staged 编辑不在场——
    // git 要求 continue 时 tracked 面干净（拒绝面另测），生产面由后验 ④「清
    // 单外新改动」先行拦截，不可达。
    fs_write(&wt, "untracked.txt", "无关 untracked\n");
    let unrelated_before = git(&wt, &["status", "--porcelain", "--", "untracked.txt"]);

    // 冲突解算：编辑移除标记 + git add（解冲突 agent 裁决的机械对译）
    fs_write(&wt, "a.txt", "裁决结果\n");
    git(&wt, &["add", "a.txt"]);

    let outcome = vcs.rebase_continue(&wt).expect("续走应成功");
    assert_eq!(outcome, RebaseOutcome::Rebased, "单提交分支一停即完成");

    let snapshot = vcs.worktree_snapshot(&wt).expect("快照应 Ok");
    assert!(
        snapshot.rebase_head.is_none(),
        "REBASE_HEAD 消失（rebase 态收口）"
    );
    let status = git(&wt, &["status", "--porcelain"]);
    assert!(!status.contains("UU"), "冲突态离场: {status}");
    assert_eq!(
        git(&wt, &["log", "-1", "--format=%s"]).trim(),
        "branch edit",
        "信息沿用原重放提交主题（不分化为 merge 信息）"
    );
    assert_eq!(
        git(&wt, &["status", "--porcelain", "--", "untracked.txt"]),
        unrelated_before,
        "无关 untracked 前后保持（零吞并）"
    );
    assert_eq!(
        std::fs::read_to_string(wt.join("b.txt")).expect("读文件失败"),
        "已跟踪基线\n",
        "无关 tracked 文件内容零触碰"
    );
    vcs.ff_merge(&root, "change/resolve")
        .expect("收尾快进应成功");
    assert_eq!(
        head_sha(&root),
        git(&wt, &["rev-parse", "HEAD"]).trim(),
        "主仓 HEAD = 分支重放 tip"
    );
}

#[test]
fn rebase_continue拒绝面_无关未staged编辑err() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-continue-refused");
    // b.txt 种入基线（tracked 面——untracked 文件不阻塞 continue）
    fs_write(&root, "b.txt", "已跟踪基线\n");
    git(&root, &["add", "b.txt"]);
    commit_all_fixture(&root, "seed b");
    let (_wt_dir, wt) = worktree_slot("arch-continue-refused");
    git::add_worktree(&root, &wt, "change/refuse").expect("建域应成功");

    fs_write(&wt, "a.txt", "分支版本\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "branch edit");
    fs_write(&root, "a.txt", "主仓版本\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main edit");
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();
    let outcome = vcs.rebase_branch(&wt, &onto).expect("冲突 Ok");
    assert!(matches!(outcome, RebaseOutcome::Conflicted(_)));

    fs_write(&wt, "a.txt", "裁决结果\n");
    git(&wt, &["add", "a.txt"]);
    fs_write(&wt, "b.txt", "无关未 staged 编辑\n"); // tracked 面不干净

    let error = vcs.rebase_continue(&wt).expect_err("tracked 面脏应 Err");
    assert!(
        !error.is_empty() && !error.contains("git 不可用"),
        "Err 为 git 语境（退出码面）: {error}"
    );
}

/// rebase_continue 多停循环（多提交分支形态）：worktree 两笔提交各改同一文
/// 件、主仓同改 → 首停 Conflicted → 解算后续走又停在第二笔 → 再解算续走 →
/// Rebased（编排链循环解算的 port 半边锚）；收尾快进后主仓内容 = 末次裁决。
#[test]
fn rebase_continue多停_第二笔冲突新清单直至rebased() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-continue-multi");
    let (_wt_dir, wt) = worktree_slot("arch-continue-multi");
    git::add_worktree(&root, &wt, "change/multi").expect("建域应成功");

    // 分支两笔提交（先后改同一文件）
    fs_write(&wt, "a.txt", "分支一\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "branch one");
    fs_write(&wt, "a.txt", "分支二\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "branch two");
    fs_write(&root, "a.txt", "主仓版本\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main edit");
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();

    let outcome = vcs.rebase_branch(&wt, &onto).expect("首停应为冲突");
    assert_eq!(
        outcome,
        RebaseOutcome::Conflicted(vec!["a.txt".to_owned()]),
        "首停清单（第一笔）"
    );
    fs_write(&wt, "a.txt", "裁决一\n");
    git(&wt, &["add", "a.txt"]);
    let outcome = vcs.rebase_continue(&wt).expect("续走应 Ok（第二停或完成）");
    assert_eq!(
        outcome,
        RebaseOutcome::Conflicted(vec!["a.txt".to_owned()]),
        "续走又停在第二笔（新清单回炉）"
    );
    fs_write(&wt, "a.txt", "裁决二\n");
    git(&wt, &["add", "a.txt"]);
    let outcome = vcs.rebase_continue(&wt).expect("末次续走应成功");
    assert_eq!(outcome, RebaseOutcome::Rebased, "两笔解完 → Rebased");

    vcs.ff_merge(&root, "change/multi").expect("快进应成功");
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).expect("读文件失败"),
        "裁决二\n",
        "主仓内容 = 末次裁决"
    );
}

#[test]
fn rebase_abort_冲突态收口回重放前_非rebase态err() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-abort");
    let (_wt_dir, wt) = worktree_slot("arch-abort");
    git::add_worktree(&root, &wt, "change/abort").expect("建域应成功");

    fs_write(&wt, "a.txt", "分支版本\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "branch edit");
    let tip_before = git(&wt, &["rev-parse", "HEAD"]).trim().to_owned();
    fs_write(&root, "a.txt", "主仓版本\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main edit");
    let onto = git(&root, &["branch", "--show-current"]).trim().to_owned();

    let outcome = vcs.rebase_branch(&wt, &onto).expect("冲突 Ok");
    assert!(
        matches!(outcome, RebaseOutcome::Conflicted(_)),
        "冲突态在场前提"
    );

    vcs.rebase_abort(&wt).expect("冲突态 abort 应成功");

    let status = git(&wt, &["status", "--porcelain"]);
    assert!(!status.contains("UU"), "无 UU 残留: {status}");
    assert_eq!(
        std::fs::read_to_string(wt.join("a.txt")).expect("读文件失败"),
        "分支版本\n",
        "worktree 内容回重放前（零破坏）"
    );
    assert_eq!(
        git(&wt, &["rev-parse", "HEAD"]).trim(),
        tip_before,
        "分支引用回原 tip"
    );

    // 非 rebase 态再 abort → Err（git 语境，非环境缺失——附注面来源）
    let error = vcs.rebase_abort(&wt).expect_err("非 rebase 态应 Err");
    assert!(
        error.contains("rebase --abort") && !error.contains("git 不可用"),
        "Err 为 git 语境失败: {error}"
    );
}

/// current_branch 两态：分支居位 → 返回分支名；`checkout --detach` → Err 含
/// detached HEAD 引导（D8 空输出映射）。
#[test]
fn current_branch两态_居位返回名detached显式err() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-current-branch");

    let branch = git(&root, &["branch", "--show-current"]);
    assert_eq!(
        vcs.current_branch(&root).expect("分支居位应 Ok"),
        branch.trim(),
        "返回当前分支名"
    );

    git(&root, &["checkout", "--detach", "-q"]);
    let error = vcs.current_branch(&root).expect_err("detached 应 Err");
    assert!(
        error.contains("detached HEAD"),
        "Err 含 detached HEAD 引导: {error}"
    );
}

/// commit_paths 改名 pathspec：预置无关 staged 条目；目录改名（旧删新增）后
/// `commit_paths([old, new], msg)` → Ok 且新提交同时含旧路径删除与新路径新增
///（`/**` 形态覆盖已删除目录——D9②「裸目录 pathspec 报 did not match」的定锚
/// 反面）；无关 staged 前后 porcelain 逐字一致（AC-4 机械断言）；`commit --
/// pathspec` 不受 index 干净度约束（既有 staged 不阻断提交）。
#[test]
fn commit_paths_改名pathspec_删除与新增同提交且无关staged原样() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-commit-paths");

    // 旧路径目录在案（已提交）
    std::fs::create_dir_all(root.join("openspec/changes/old-name")).expect("建旧目录失败");
    fs_write(&root, "openspec/changes/old-name/proposal.md", "旧产物\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "seed");

    // 无关 staged 条目（改名提交不得吞并）
    fs_write(&root, "other.txt", "无关 staged\n");
    git(&root, &["add", "other.txt"]);
    let status_before = git(&root, &["status", "--porcelain"]);

    // 目录改名：旧删新增（新目录 untracked）
    std::fs::remove_dir_all(root.join("openspec/changes/old-name")).expect("删旧目录失败");
    std::fs::create_dir_all(root.join("openspec/changes/archive/2026-10-08-old-name"))
        .expect("建新目录失败");
    fs_write(
        &root,
        "openspec/changes/archive/2026-10-08-old-name/proposal.md",
        "旧产物\n",
    );

    vcs.commit_paths(
        &root,
        &[
            "openspec/changes/old-name",
            "openspec/changes/archive/2026-10-08-old-name",
        ],
        "archive: move old-name to archive",
    )
    .expect("pathspec 提交应成功");

    let name_status = git(&root, &["show", "--name-status", "--format=", "HEAD"]);
    // 旧路径离场 + 新路径在场（内容同文件时 git 以 R100 rename 对呈现——同一
    // 事实的两种记法；树面以 ls-tree 复核）
    assert!(
        name_status.contains("openspec/changes/old-name/proposal.md"),
        "旧路径离场入提交（/** 形态覆盖删除路径——D9②）: {name_status}"
    );
    assert!(
        name_status.contains("openspec/changes/archive/2026-10-08-old-name/proposal.md"),
        "新路径在场入提交: {name_status}"
    );
    let tree = git(&root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(!tree.contains("changes/old-name/"), "旧路径已离树: {tree}");
    assert!(
        tree.contains("changes/archive/2026-10-08-old-name/proposal.md"),
        "新路径已在树"
    );
    let status_after = git(&root, &["status", "--porcelain"]);
    assert_eq!(
        status_after, status_before,
        "无关 staged 前后 porcelain 逐字一致（AC-4）"
    );
    assert!(
        status_after.contains("A  other.txt"),
        "既有 staged 原样（不吞并）"
    );
}

/// commit_paths 三 pathspec 含 specs 子树（archive-merge-first D9 真件半边）：
/// 归档改名 + delta capability 主 specs 子树更新 + 无关 capability specs 用户
/// 未提交编辑 + 无关 staged 条目 → 新提交同含改名两路径与 specs 更新（`/**`
/// glob 既有纪律）；无关 capability specs 编辑与无关 staged 前后 porcelain
/// 逐字一致（全树通配禁区的机械反面——AC-4）。
#[test]
fn commit_paths_三pathspec含specs子树_无关specs不吞并() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-commit-paths-specs");

    // 种子：旧 change 目录 + 主 specs 两 capability（cap-a = delta、cap-b = 无关）
    std::fs::create_dir_all(root.join("openspec/changes/old-name")).expect("建旧目录失败");
    fs_write(&root, "openspec/changes/old-name/proposal.md", "旧产物\n");
    std::fs::create_dir_all(root.join("openspec/specs/cap-a")).expect("建 cap-a 失败");
    fs_write(&root, "openspec/specs/cap-a/spec.md", "基线\n");
    std::fs::create_dir_all(root.join("openspec/specs/cap-b")).expect("建 cap-b 失败");
    fs_write(&root, "openspec/specs/cap-b/spec.md", "无关基线\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "seed");

    // 归档改名（旧删新增）+ delta specs 更新 + 无关 capability specs 用户编辑
    // + 无关 staged 条目（全部就位后取 porcelain 基线）
    std::fs::remove_dir_all(root.join("openspec/changes/old-name")).expect("删旧目录失败");
    std::fs::create_dir_all(root.join("openspec/changes/archive/2026-10-08-old-name"))
        .expect("建归档目录失败");
    fs_write(
        &root,
        "openspec/changes/archive/2026-10-08-old-name/proposal.md",
        "旧产物\n",
    );
    fs_write(&root, "openspec/specs/cap-a/spec.md", "同步产物\n");
    fs_write(&root, "openspec/specs/cap-b/spec.md", "用户未提交编辑\n");
    fs_write(&root, "other.txt", "无关 staged\n");
    git(&root, &["add", "other.txt"]);
    // 圈外条目基线（无关 capability specs 编辑 + 无关 staged——pathspec 圈外）
    let unrelated_before = git(
        &root,
        &[
            "status",
            "--porcelain",
            "--",
            "openspec/specs/cap-b",
            "other.txt",
        ],
    );

    vcs.commit_paths(
        &root,
        &[
            "openspec/changes/old-name",
            "openspec/changes/archive/2026-10-08-old-name",
            "openspec/specs/cap-a",
        ],
        "archive: move old-name to archive",
    )
    .expect("pathspec 提交应成功");

    let tree = git(&root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(
        !tree.contains("changes/old-name/"),
        "旧路径离树（/** 形态覆盖删除路径）: {tree}"
    );
    assert!(
        tree.contains("changes/archive/2026-10-08-old-name/proposal.md"),
        "归档新路径入树"
    );
    assert!(
        tree.contains("specs/cap-a/spec.md"),
        "delta specs 子树入提交"
    );
    assert_eq!(
        git(&root, &["show", "HEAD:openspec/specs/cap-a/spec.md"]),
        "同步产物\n",
        "specs 更新内容入提交"
    );
    assert_eq!(
        git(&root, &["show", "HEAD:openspec/specs/cap-b/spec.md"]),
        "无关基线\n",
        "无关 capability specs 不入提交（全树通配禁区反面）"
    );
    let status_after = git(&root, &["status", "--porcelain"]);
    assert_eq!(
        git(
            &root,
            &[
                "status",
                "--porcelain",
                "--",
                "openspec/specs/cap-b",
                "other.txt"
            ]
        ),
        unrelated_before,
        "无关 specs 编辑与无关 staged 圈外条目前后逐字一致（AC-4——pathspec 纪律）"
    );
    assert!(
        status_after.contains(" M openspec/specs/cap-b/spec.md"),
        "无关 specs 用户编辑原样保留: {status_after}"
    );
    assert!(
        status_after.contains("A  other.txt"),
        "无关 staged 原样保留"
    );
}

/// PATH 隔离 Err 面：git 不可发现（PATH 隔离窗口）→ 六方法（`rebase_branch` /
/// `ff_merge` / `commit_paths` / `branch_merged` + `worktree_snapshot` /
/// `rebase_continue` / `rebase_abort`）各 Err 含「git 不可用」引导（`git()`
/// 执行器既有面随族扩展——archive-rebase-merge 新方法同口径）；窗口经共享
/// 锁串行化、测毕恢复。
#[test]
fn path隔离_err面_git不可用引导() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("arch-path-isolated");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let rebase_error = vcs.rebase_branch(&root, "main").map(|_| ());
    let ff_error = vcs.ff_merge(&root, "change/x");
    let commit_error = vcs.commit_paths(&root, &["a"], "msg");
    let merged_error = vcs.branch_merged(&root, "change/x").map(|_| ());
    let snapshot_error = vcs.worktree_snapshot(&root).map(|_| ());
    let continue_error = vcs.rebase_continue(&root).map(|_| ());
    let abort_error = vcs.rebase_abort(&root);
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    for (label, error) in [
        ("rebase_branch", rebase_error),
        ("ff_merge", ff_error),
        ("commit_paths", commit_error),
        ("branch_merged", merged_error),
        ("worktree_snapshot", snapshot_error),
        ("rebase_continue", continue_error),
        ("rebase_abort", abort_error),
    ] {
        let error = match error {
            Ok(_) => panic!("{label} 在 git 缺失下应 Err"),
            Err(error) => error,
        };
        assert!(
            error.contains("git 不可用"),
            "{label} Err 含「git 不可用」引导，实际: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// 变更文件清单 / 补丁唯一入口（desktop-fix-mutation-test / AC-3）：
// change_edit_set（tracked ∪ untracked 排序去重 POSIX 归一）+ change_edit_patch
// （untracked intent-to-add 后 unified diff）——真实 git tempdir 装置直驱
// ---------------------------------------------------------------------------

/// 正向：`change_edit_set` tracked 修改 + untracked 新建并入 → 清单含两类文件、
/// 字典序排序、去重、POSIX 归一（AC-3 / D3）。
#[test]
fn change_edit_set_tracked与untracked并入且字典序去重() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-set-mixed");
    // tracked 修改
    fs_write(&root, "a.txt", "modified\n");
    // untracked 新建（嵌套深层 + 特殊文件名）
    std::fs::create_dir_all(root.join("src").join("nested")).expect("建子目录");
    fs_write(&root, "src/nested/mod.rs", "pub fn f() {}\n");
    fs_write(&root, "src/lib.rs", "mod nested;\n");
    fs_write(&root, "文件 名.txt", "特殊名\n");

    let files = vcs.change_edit_set(&root, "HEAD").expect("清单取数应成功");
    assert_eq!(
        files,
        vec![
            "a.txt".to_owned(),
            "src/lib.rs".to_owned(),
            "src/nested/mod.rs".to_owned(),
            "文件 名.txt".to_owned(),
        ],
        "tracked ∪ untracked 并入、字典序排序、逐项在案"
    );
    // 去重：无重复项（BTreeSet 并集语义）
    let mut unique = files.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), files.len(), "清单无重复项");
    // POSIX 归一：反斜杠不出线（git -z 输出恒正斜杠——断言形态面）
    for file in &files {
        assert!(!file.contains('\\'), "POSIX 归一（反斜杠不出线）: {file}");
    }
}

/// 正向：仅 tracked 改动（无 untracked）→ 清单恰为 diff 面；仅 untracked →
/// 清单为 ls-files 面（两源口径分立）。
#[test]
fn change_edit_set_仅tracked与仅untracked两源口径分立() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-set-sources");

    // 仅 tracked：改 a.txt
    fs_write(&root, "a.txt", "changed\n");
    let tracked_only = vcs
        .change_edit_set(&root, "HEAD")
        .expect("tracked 面应成功");
    assert_eq!(tracked_only, vec!["a.txt".to_owned()], "清单恰为 diff 面");

    // 复位 tracked 改动
    git(&root, &["checkout", "--", "a.txt"]);

    // 仅 untracked：新建 b.txt
    fs_write(&root, "b.txt", "new\n");
    let untracked_only = vcs
        .change_edit_set(&root, "HEAD")
        .expect("untracked 面应成功");
    assert_eq!(
        untracked_only,
        vec!["b.txt".to_owned()],
        "清单为 ls-files 面"
    );
}

/// 正向：`base = "HEAD"` 语义——清单恰为本 change 编辑集（worktree 语义：工作
/// 区对 HEAD 的全部未提交面）；其它合法 base（sha / 分支名）透传生效。
#[test]
fn change_edit_set_base语义_head与sha透传() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-set-base");
    let head = head_sha(&root);

    // HEAD 语义：未提交面（tracked + untracked）
    fs_write(&root, "a.txt", "worktree edit\n");
    fs_write(&root, "untracked.txt", "new\n");
    let head_set = vcs.change_edit_set(&root, "HEAD").expect("HEAD 面应成功");
    assert_eq!(
        head_set,
        vec!["a.txt".to_owned(), "untracked.txt".to_owned()],
        "HEAD 语义 = 本 change 编辑集（worktree 语义）"
    );

    // 提交后对 HEAD 零改动
    git(&root, &["add", "-A"]);
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
            "second",
        ],
    );
    let clean = vcs.change_edit_set(&root, "HEAD").expect("干净面应成功");
    assert_eq!(clean, Vec::<String>::new(), "提交后对 HEAD 零编辑集");

    // 其它合法 base（sha 透传）：上一提交 → 本次提交的变更面
    let sha_set = vcs.change_edit_set(&root, &head).expect("sha 面应成功");
    assert_eq!(
        sha_set,
        vec!["a.txt".to_owned(), "untracked.txt".to_owned()],
        "sha base 透传生效（两文件为该提交的变更面）"
    );
}

/// 异常：非 git 目录 / git 命令失败 → 显式 `Err`（MUST NOT 静默降级空清单
/// ——空清单会误走 diff 空跳过）。
#[test]
fn change_edit_set_非git仓显式err不静默降级() {
    let vcs = archive_vcs();
    let (_dir, plain) = plain_dir("edit-set-plain");
    let error = vcs
        .change_edit_set(&plain, "HEAD")
        .expect_err("非 git 目录应 Err");
    assert!(
        error.contains("失败") || error.contains("git"),
        "Err 携 git 语境，实际: {error}"
    );
}

/// 异常：`base` 非法——`""` 空串 → 前置校验显式 Err（含引导文案）；非存在 rev
/// / 含换行 / 超长（>1000 字符）→ git 失败显式 Err 不 panic。
#[test]
fn change_edit_set_base非法族显式err不panic() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-set-bad-base");

    // 空串：前置校验（validate_base）显式拒绝
    let empty_error = vcs.change_edit_set(&root, "").expect_err("空 base 应 Err");
    assert!(
        empty_error.contains("base 为空"),
        "空 base 前置校验记因: {empty_error}"
    );

    // 非存在 rev / 含换行 / 超长 → git 失败显式 Err
    let long_rev = "f".repeat(1200);
    for bad_base in ["不存在的rev-xyz", "HEAD\nbranch", long_rev.as_str()] {
        let error = vcs
            .change_edit_set(&root, bad_base)
            .expect_err(&format!("非法 base {bad_base:?} 应 Err"));
        assert!(!error.is_empty(), "Err 记因非空");
    }
}

/// 边界：干净仓零改动 → `Ok(空 Vec)`（与 `Err` 严格区分）；单文件清单 / 超大
/// 清单（100+ 文件）顺序稳定。
#[test]
fn change_edit_set_干净仓空vec与超大清单顺序稳定() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-set-boundary");

    // 干净仓：Ok(空)——与 Err 严格区分（空清单合法面）
    let clean = vcs.change_edit_set(&root, "HEAD").expect("干净仓 Ok 面");
    assert_eq!(clean, Vec::<String>::new(), "干净仓零改动 = Ok(空 Vec)");

    // 单文件清单
    fs_write(&root, "only.txt", "one\n");
    assert_eq!(
        vcs.change_edit_set(&root, "HEAD").expect("单文件面"),
        vec!["only.txt".to_owned()],
        "单文件清单"
    );

    // 超大清单（100+ untracked 文件）顺序稳定（字典序）
    std::fs::create_dir_all(root.join("bulk")).expect("建 bulk 目录");
    for index in (0..120).rev() {
        fs_write(&root, &format!("bulk/f{index:03}.rs"), "// generated\n");
    }
    let bulk = vcs.change_edit_set(&root, "HEAD").expect("超大清单面");
    assert_eq!(bulk.len(), 121, "超大清单全量在案（1 + 120）");
    let mut sorted = bulk.clone();
    sorted.sort();
    assert_eq!(bulk, sorted, "顺序稳定（字典序）");
    assert!(
        bulk.windows(2).all(|pair| pair[0] < pair[1]),
        "严格递增无重复"
    );
}

/// 边界：双源重复（intent-to-add 后 diff 面与 ls-files 面并集）→ 去重单条；
/// 特殊文件名（空格 / 中文 / emoji）与深层路径归一。
#[test]
fn change_edit_set_双源去重与特殊文件名归一() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-set-dedupe");

    // untracked 文件先经 intent-to-add（add -N）：diff 面与 ls-files 面双源并集
    std::fs::create_dir_all(root.join("deep").join("a").join("b")).expect("建深层目录");
    fs_write(&root, "deep/a/b/c.rs", "deep new\n");
    git(&root, &["add", "-N", "deep/a/b/c.rs"]);
    fs_write(&root, "emoji 🚀.txt", "emoji\n");
    fs_write(&root, "中文 文件.txt", "中文\n");

    let files = vcs.change_edit_set(&root, "HEAD").expect("并集面应成功");
    assert_eq!(
        files.iter().filter(|file| *file == "deep/a/b/c.rs").count(),
        1,
        "双源重复去重单条（intent-to-add 后 diff / ls-files 双面并集）: {files:?}"
    );
    assert!(
        files.contains(&"emoji 🚀.txt".to_owned())
            && files.contains(&"中文 文件.txt".to_owned())
            && files.contains(&"deep/a/b/c.rs".to_owned()),
        "特殊文件名（空格 / 中文 / emoji）与深层路径在案: {files:?}"
    );
}

/// 边界：`root` 不存在 / `root` 指向文件 → 显式 `Err`。
#[test]
fn change_edit_set_root不存在或指向文件显式err() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-set-root-err");

    // root 不存在
    let missing = root.join("no-such-dir");
    assert!(
        vcs.change_edit_set(&missing, "HEAD").is_err(),
        "root 不存在应 Err"
    );

    // root 指向文件
    let file = root.join("a.txt");
    assert!(
        vcs.change_edit_set(&file, "HEAD").is_err(),
        "root 指向文件应 Err"
    );
}

/// 首次调用即剔除残留：盘上已有旧版本 / 早前失败遗留的 `_stryker-tmp`
/// 可插桩源文件（`.ts`）与临时 config，且 exclude 尚未在位——`change_edit_set`
/// 的 ensure 先于两次 git 列举，二者不得作为 untracked 进入清单；ensure 同步
/// 补齐两条 pattern。
#[test]
fn change_edit_set_首次调用即剔除残留调试产物() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-set-first-call-residue");
    // 模拟升级 / 早前失败留存：sandbox 内可插桩源文件 + 段临时 config
    let mutation = root
        .join("openspec")
        .join("changes")
        .join("demo")
        .join("reports")
        .join("test")
        .join("app_vite-plus")
        .join("mutation");
    std::fs::create_dir_all(mutation.join("_stryker-tmp").join("sandbox-x").join("src"))
        .expect("建 sandbox");
    fs_write(
        &root,
        "openspec/changes/demo/reports/test/app_vite-plus/mutation/_stryker-tmp/sandbox-x/src/legacy.ts",
        "export const legacy = 1;\n",
    );
    fs_write(
        &root,
        "openspec/changes/demo/reports/test/app_vite-plus/mutation/mutation-stryker-seg-1.conf.json",
        "{}\n",
    );
    // 对照组：普通编辑照常入清单
    fs_write(&root, "a.txt", "edited\n");

    let files = vcs.change_edit_set(&root, "HEAD").expect("首次调用应成功");
    assert_eq!(files, vec!["a.txt".to_owned()], "残留调试产物不入清单");
    let exclude = read_exclude(&root);
    assert!(
        exclude.contains("_stryker-tmp/"),
        "sandbox 规则首次调用即补齐: {exclude}"
    );
    assert!(
        exclude.contains("mutation-stryker-seg-*.conf.json"),
        "临时 config 规则首次调用即补齐: {exclude}"
    );
}

/// 正向：untracked 经 `git add -N` 后补丁含新文件 unified diff hunk（行级
/// 精度，E2 / 风险 3 定稿——intent-to-add 不回退标记）。
#[test]
fn change_edit_patch_untracked经intent_to_add后补丁含新文件hunk() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-patch-untracked");
    fs_write(&root, "fresh.rs", "line one\nline two\n");

    let patch = vcs
        .change_edit_patch(&root, "HEAD")
        .expect("补丁取数应成功");
    assert!(
        patch.contains("diff --git a/fresh.rs b/fresh.rs"),
        "补丁含新文件段头: {patch}"
    );
    assert!(
        patch.contains("--- /dev/null") && patch.contains("+++ b/fresh.rs"),
        "新文件创建面 hunks（/dev/null → b/）: {patch}"
    );
    assert!(
        patch.contains("+line one") && patch.contains("+line two"),
        "行级 hunk 内容在案: {patch}"
    );
    // intent-to-add 标记在位（index 记 intent 条目——ls-files 面可查；
    // add -N 不回退——worktree 私有锚）
    let staged = git(&root, &["ls-files", "--stage", "fresh.rs"]);
    assert!(
        staged.contains("fresh.rs"),
        "intent-to-add 标记保留（git add -N 不回退）: {staged}"
    );
}

/// 正向：tracked 修改 → 补丁含对应 hunks；多文件补丁含多 `diff --git` 文件段
/// 头（供段切片——mutation segments 消费面）。
#[test]
fn change_edit_patch_多文件补丁含多段头() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-patch-multi");
    fs_write(&root, "a.txt", "alpha v1\n");
    std::fs::create_dir_all(root.join("src")).expect("建 src");
    fs_write(&root, "src/lib.rs", "fn one() {}\n");
    git(&root, &["add", "-A"]);
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
            "base",
        ],
    );

    // 两 tracked 文件各改一行 + 一 untracked 新建
    fs_write(&root, "a.txt", "alpha v2\n");
    fs_write(&root, "src/lib.rs", "fn one() {}\nfn two() {}\n");
    fs_write(&root, "extra.txt", "brand new\n");

    let patch = vcs
        .change_edit_patch(&root, "HEAD")
        .expect("补丁取数应成功");
    assert_eq!(
        patch.matches("diff --git").count(),
        3,
        "三文件段头（多文件补丁供段切片）: {patch}"
    );
    assert!(patch.contains("a/a.txt") && patch.contains("b/a.txt"));
    assert!(patch.contains("+++ b/src/lib.rs"));
    assert!(
        patch.contains("+fn two() {}"),
        "tracked 修改 hunk 在案: {patch}"
    );
    assert!(patch.contains("+brand new"), "untracked hunk 在案");
}

/// 异常：非 git 目录 / 非法 `base` → 显式 `Err`（与 `change_edit_set` 同口径）。
#[test]
fn change_edit_patch_非git仓与非法base显式err() {
    let vcs = archive_vcs();
    let (_dir, plain) = plain_dir("edit-patch-plain");
    assert!(
        vcs.change_edit_patch(&plain, "HEAD").is_err(),
        "非 git 目录应 Err"
    );

    let (_dir2, root) = init_repo("edit-patch-bad-base");
    let empty_error = vcs
        .change_edit_patch(&root, "")
        .expect_err("空 base 应 Err");
    assert!(
        empty_error.contains("base 为空"),
        "同口径前置校验: {empty_error}"
    );
    assert!(
        vcs.change_edit_patch(&root, "不存在的rev-xyz").is_err(),
        "非存在 rev 应 Err"
    );
}

/// 边界：干净仓 → 空串（零补丁）；补丁文本含特殊字符（换行 / CRLF / emoji
/// 内容）往返不破坏 diff 形态。
#[test]
fn change_edit_patch_干净仓空串与特殊字符保真() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-patch-boundary");

    // 干净仓：空串（零补丁——与 Err 严格区分）
    let clean = vcs.change_edit_patch(&root, "HEAD").expect("干净仓 Ok 面");
    assert_eq!(clean, "", "干净仓零 diff = 空串");

    // 特殊字符内容：CRLF 混排 + emoji + 中文（补丁往返不破坏 diff 形态）
    fs_write(&root, "special.txt", "crlf line\r\nemoji 🚀 内容\n中文行\n");
    let patch = vcs
        .change_edit_patch(&root, "HEAD")
        .expect("特殊字符补丁面应成功");
    assert!(
        patch.starts_with("diff --git a/special.txt b/special.txt"),
        "diff 形态起点保真: {patch}"
    );
    assert!(
        patch.contains("+crlf line\r") || patch.contains("+crlf line"),
        "CRLF 内容保真（git 原样承载）: {patch}"
    );
    assert!(patch.contains("🚀"), "emoji 内容保真");
    assert!(patch.contains("中文行"), "中文内容保真");
}

/// 边界：`git add -N` 幂等——重复调用结果一致（intent-to-add 重复执行不改语
/// 义，E2 单次执行口径）。
#[test]
fn change_edit_patch_幂等_重复调用结果一致() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-patch-idempotent");
    fs_write(&root, "fresh.rs", "stable content\n");
    fs_write(&root, "a.txt", "tracked edit\n");

    let first = vcs.change_edit_patch(&root, "HEAD").expect("首次补丁面");
    let second = vcs.change_edit_patch(&root, "HEAD").expect("二次补丁面");
    assert_eq!(
        first, second,
        "git add -N 幂等：重复调用补丁逐字一致（intent-to-add 重复执行不改语义）"
    );
    assert!(!first.is_empty(), "补丁非空（双源在场）");
}

/// 回归（os error 206）：海量 untracked 清单（argv 展开面超 Windows
/// CreateProcess 命令行 32767 上限的量级——test-execution 遗留 llvm-cov
/// 产物 500+ 路径现场 ≈ 70KB）经 `--pathspec-from-file=-` stdin 入线不炸：
/// 补丁取数成功且含新文件段头（argv 展开旧径此装置下 spawn 直接失败）。
#[test]
fn change_edit_patch_海量untracked清单经stdin入线不超命令行上限() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("edit-patch-flood");
    // 600 × ~131 字符路径 ≈ 78KB，稳超 32767；单层平铺（装置面勿嵌套目录拖慢）
    let mut total = 0usize;
    for i in 0..600 {
        let name = format!("f-{i:03}-{}", "x".repeat(124));
        fs_write(&root, &name, "fn mutated() {}\n");
        total += name.len() + 1;
    }
    assert!(
        total > 32_767,
        "装置面须超 Windows 命令行上限（实际 {total}）"
    );

    let patch = vcs
        .change_edit_patch(&root, "HEAD")
        .expect("海量 untracked 补丁取数应成功（stdin 入线）");
    assert!(
        patch.contains("diff --git a/f-000-") && patch.contains("diff --git a/f-599-"),
        "补丁含首尾新文件段头（intent-to-add 全量生效）"
    );
}

// ---------------------------------------------------------------------------
// ensure_mutation_artifact_exclude（.git/info/exclude 幂等追加 + staged 残留
// 撤出）——失败段调试产物留盘复现但 MUST NOT 进归档提交
// ---------------------------------------------------------------------------

/// 读取 root 的 info/exclude 全文（装置断言原料；经 git-path 解析——与生产
/// `resolve_info_exclude` 同口径）。
fn read_exclude(root: &Path) -> String {
    let raw = git(root, &["rev-parse", "--git-path", "info/exclude"]);
    let raw = raw.trim();
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    std::fs::read_to_string(path).unwrap_or_default()
}

/// 幂等：预置注释行 + 两条规则后重复 ensure → 各 pattern 恰一行、注释行原
/// 样、二次调用零追加。
#[test]
fn ensure幂等_重复调用不重复追加() {
    let (_dir, root) = init_repo("ensure-idempotent");
    git::ensure_mutation_artifact_exclude(&root).expect("首次 ensure 应成功");
    git::ensure_mutation_artifact_exclude(&root).expect("二次 ensure 应成功");
    let content = read_exclude(&root);
    assert_eq!(
        content.matches("_stryker-tmp/").count(),
        1,
        "sandbox 规则恰一行: {content}"
    );
    assert_eq!(
        content.matches("mutation-stryker-seg-*.conf.json").count(),
        1,
        "临时 config 规则恰一行: {content}"
    );
}

/// CRLF 兼容 + `.git/info` 目录补建：CRLF 仓追加用 CRLF、目录缺失重建。
#[test]
fn ensure兼容crlf与补目录() {
    let (_dir, root) = init_repo("ensure-crlf");
    // 预置 CRLF 注释行
    std::fs::create_dir_all(root.join(".git").join("info")).expect("建 info 目录");
    std::fs::write(
        root.join(".git").join("info").join("exclude"),
        "# custom\r\n",
    )
    .expect("预置 CRLF exclude");
    git::ensure_mutation_artifact_exclude(&root).expect("CRLF 仓 ensure 应成功");
    let content = read_exclude(&root);
    assert!(
        content.contains("# custom\r\n"),
        "既有 CRLF 行原样: {content:?}"
    );
    assert!(
        content.contains("_stryker-tmp/\r\n"),
        "追加行跟随 CRLF: {content:?}"
    );

    // 目录缺失：删 .git/info 后 ensure 重建
    std::fs::remove_dir_all(root.join(".git").join("info")).expect("删 info 目录");
    git::ensure_mutation_artifact_exclude(&root).expect("目录缺失 ensure 应成功");
    assert!(
        read_exclude(&root).contains("mutation-stryker-seg-*.conf.json"),
        "目录重建 + 规则在案"
    );
}

/// ensure 后调试产物三面全剔：change_edit_set 不含、status --porcelain 不列
/// （dirty 不被顶真）、add -A 后 diff --cached 不含。
#[test]
fn ensure后_调试产物三面全剔() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("ensure-three-faces");
    git::ensure_mutation_artifact_exclude(&root).expect("ensure 应成功");

    // 深位调试产物在盘（报告树真实形态）
    let mutation = root
        .join("openspec")
        .join("changes")
        .join("demo")
        .join("reports")
        .join("test")
        .join("app_vite-plus")
        .join("mutation");
    std::fs::create_dir_all(mutation.join("_stryker-tmp").join("sandbox-x")).expect("建 sandbox");
    fs_write(
        &root,
        "openspec/changes/demo/reports/test/app_vite-plus/mutation/mutation-stryker-seg-0.conf.json",
        "{}\n",
    );
    // 对照组：普通编辑照常入清单
    fs_write(&root, "a.txt", "edited\n");

    // 面 1：清单不含
    let files = vcs.change_edit_set(&root, "HEAD").expect("清单应成功");
    assert_eq!(files, vec!["a.txt".to_owned()], "调试产物不入清单");
    // 面 2：status 不列（dirty 不被顶真）
    let status = git(&root, &["status", "--porcelain"]);
    assert!(!status.contains("stryker"), "status 不列调试产物: {status}");
    assert!(status.contains("a.txt"), "普通编辑照常在列");
    // 面 3：add -A 后 staged 不含
    git(&root, &["add", "-A"]);
    let staged = git(&root, &["diff", "--cached", "--name-only"]);
    assert!(
        !staged.contains("stryker"),
        "add -A 不收录调试产物: {staged}"
    );
    assert!(staged.contains("a.txt"), "普通编辑照常收录");
}

/// 升级路径：修复前 `git add -N` 留下的 intent-to-add 残留被 ensure 撤出索引
///（文件留盘）。
#[test]
fn ensure撤出intent_to_add残留() {
    let (_dir, root) = init_repo("ensure-reset-residue");
    // 模拟修复前残留：临时 config 已被 add -N（intent-to-add 不出
    // `diff --cached`，但确在索引、add -A / commit 会真正收录）
    std::fs::create_dir_all(root.join("reports")).expect("建 reports 目录");
    fs_write(&root, "reports/mutation-stryker-seg-0.conf.json", "{}\n");
    git(
        &root,
        &[
            "add",
            "-N",
            "--",
            "reports/mutation-stryker-seg-0.conf.json",
        ],
    );
    let cached = git(&root, &["ls-files", "--cached"]);
    assert!(
        cached.contains("mutation-stryker-seg-0.conf.json"),
        "前置：残留确在索引（{cached}）"
    );

    git::ensure_mutation_artifact_exclude(&root).expect("ensure 应成功");
    // 撤出索引、文件留盘
    let cached = git(&root, &["ls-files", "--cached"]);
    assert!(
        !cached.contains("mutation-stryker-seg-0.conf.json"),
        "残留撤出索引: {cached}"
    );
    assert!(
        root.join("reports/mutation-stryker-seg-0.conf.json")
            .is_file(),
        "文件留盘（只撤索引不删文件）"
    );
    // 归档面验收：add -A 后 staged 不含（不进归档提交）
    git(&root, &["add", "-A"]);
    let staged = git(&root, &["diff", "--cached", "--name-only"]);
    assert!(
        !staged.contains("mutation-stryker-seg-0.conf.json"),
        "add -A 不收录残留: {staged}"
    );
}

/// 非 git 仓：ensure 显式 Err（前序 change_edit_set 的 git 调用已先行 Err，
/// 此处直测 ensure 自身错误面）。
#[test]
fn ensure非git仓_显式err() {
    let dir = tempfile::Builder::new()
        .prefix("vcs-git-test-ensure-nongit-")
        .tempdir()
        .expect("创建临时目录失败");
    assert!(
        git::ensure_mutation_artifact_exclude(dir.path()).is_err(),
        "非 git 仓显式 Err"
    );
}

/// linked worktree：ensure(worktree) 写主仓共享 exclude（common-dir 形态回
/// 归锚）——worktree 内产物同被剔除。
#[test]
fn ensure_worktree写主仓共享exclude() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("ensure-worktree-main");
    let (_wt_dir, wt) = worktree_slot("ensure-worktree-wt");
    git(
        &root,
        &["worktree", "add", &wt.to_string_lossy(), "-b", "change/wt"],
    );

    // worktree 内深位产物
    let mutation = wt
        .join("openspec")
        .join("changes")
        .join("demo")
        .join("reports")
        .join("test")
        .join("mutation");
    std::fs::create_dir_all(mutation.join("_stryker-tmp")).expect("建 sandbox");
    std::fs::write(mutation.join("mutation-stryker-seg-0.conf.json"), "{}\n")
        .expect("写临时 config");

    git::ensure_mutation_artifact_exclude(&wt).expect("worktree ensure 应成功");
    // 主仓 .git/info/exclude 在案（common dir 共享）
    let main_exclude =
        std::fs::read_to_string(root.join(".git").join("info").join("exclude")).unwrap_or_default();
    assert!(
        main_exclude.contains("_stryker-tmp/"),
        "主仓 exclude 在案: {main_exclude}"
    );
    // worktree 内 untracked 剔除
    let untracked = git(&wt, &["ls-files", "--others", "--exclude-standard"]);
    assert!(
        !untracked.contains("stryker"),
        "worktree 产物被共享 exclude 剔除: {untracked}"
    );
}

// ---------------------------------------------------------------------------
// restore_baseline（initialize 相位 Clean 步 spawn 单点，check-before-flow-start
// / AC-5 / AC-6）：reset --hard + clean -fd 逐项 -e 排除（不带 -x）——零 mock
// 真件直驱（Mock策略：无）
// ---------------------------------------------------------------------------

/// git 语境错误串（`git()` Err 组装形态：命令串 + 退出码 + stderr 三段在案）。
fn assert_git_context(error: &str) {
    assert!(error.contains("git"), "错误串携命令语境: {error}");
    assert!(
        error.contains("exit") || error.contains("退出"),
        "错误串携退出码: {error}"
    );
}

/// tracked 重置（正向 / AC-5 reset 半边）：已跟踪文件改动 + staged 改动 → 恢复
/// 后 `status --porcelain` 干净、内容回 HEAD 版本。
#[test]
fn restore_baseline_tracked重置_改动与staged回head() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("restore-tracked");

    // 工作区改动 + staged 改动各一
    fs_write(&root, "a.txt", "工作区改动\n");
    fs_write(&root, "staged.txt", "已暂存改动\n");
    git(&root, &["add", "staged.txt"]);
    let dirty = git(&root, &["status", "--porcelain"]);
    assert!(!dirty.trim().is_empty(), "前置：改动在案");

    let summary = git::restore_baseline(&root, &[".*", "openspec"]).expect("恢复应 Ok");
    assert!(!summary.is_empty(), "恢复摘要串非空（step detail 消费面）");

    let status = git(&root, &["status", "--porcelain"]);
    assert!(
        status.trim().is_empty(),
        "恢复后工作区干净（tracked reset + staged 弃置）: {status:?}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).expect("读回 a.txt"),
        "initial\n",
        "内容回 HEAD 版本（reset --hard 语义）"
    );
}

/// untracked 清除 + 排除面六目录保留（正向，枚举遍历 / AC-5 clean 半边）：未
/// 跟踪新文件被清除；node_modules / dist / build / target / coverage /
/// .nyc_output 逐目录夹具原样保留。
#[test]
fn restore_baseline_untracked清除且排除面六目录保留() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("restore-untracked");

    // 未跟踪普通文件 + 排除面六目录各一（目录内带文件——目录非空不因空目录
    // 语义假绿）
    fs_write(&root, "fresh.txt", "未跟踪新文件\n");
    for name in [
        "node_modules",
        "dist",
        "build",
        "target",
        "coverage",
        ".nyc_output",
    ] {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).expect("建排除目录");
        std::fs::write(dir.join("artifact.bin"), b"payload").expect("写产物");
    }

    git::restore_baseline(
        &root,
        &[
            "node_modules",
            "dist",
            "build",
            "target",
            "coverage",
            ".nyc_output",
            ".*",
            "openspec",
        ],
    )
    .expect("恢复应 Ok");

    let status = git(&root, &["status", "--porcelain"]);
    assert!(
        !status.contains("fresh.txt"),
        "未跟踪新文件被清除（clean -fd）: {status:?}"
    );
    for name in [
        "node_modules",
        "dist",
        "build",
        "target",
        "coverage",
        ".nyc_output",
    ] {
        assert!(
            root.join(name).join("artifact.bin").is_file(),
            "排除目录 {name} 原样保留（-e 逐项排除）"
        );
    }
}

/// dot 与工件树（正向 / AC-5）：未跟踪 dot 目录（`.idea` 等）经 `-e ".*"`
/// 保留；未跟踪 `openspec/` 工件树经显式 `-e` 保留（bootstrap 依赖与过程文档
/// 不丢）。
#[test]
fn restore_baseline_dot目录与openspec工件树保留() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("restore-dot");

    let idea = root.join(".idea");
    std::fs::create_dir_all(&idea).expect("建 dot 目录");
    std::fs::write(idea.join("workspace.xml"), "<ide/>").expect("写 dot 文件");
    let specs = root.join("openspec").join("changes").join("demo");
    std::fs::create_dir_all(&specs).expect("建工件树");
    std::fs::write(specs.join("proposal.md"), "# 提案\n").expect("写工件");

    git::restore_baseline(&root, &[".*", "openspec"]).expect("恢复应 Ok");

    assert!(
        idea.join("workspace.xml").is_file(),
        "dot 目录经 -e \".*\" 保留（dot 剪枝语义）"
    );
    assert!(
        specs.join("proposal.md").is_file(),
        "openspec 工件树经显式 -e 保留（bootstrap / 过程文档不丢）"
    );
    let status = git(&root, &["status", "--porcelain"]);
    // clean -e 只是不删除（保护语义），未跟踪排除目录仍出现在 status 面——
    // 保护断言 = 目录内容逐字在案 + status 仅含排除面两行（零额外清除面）
    assert!(
        status.contains(".idea/") && status.contains("openspec/"),
        "排除目录在 status 面在案（未删除即保护）: {status:?}"
    );
    for line in status.lines() {
        assert!(
            line.contains(".idea") || line.contains("openspec"),
            "status 面零额外条目（排除面外零清除零残留）: {line:?}"
        );
    }
}

/// gitignore 免疫层（边界 / `-x` 缺席语义锚）：带 .gitignore 规则仓的被忽略
/// 目录保留（clean 无 -x 不碰 ignored）；无 ignore 规则仓的同名目录经 `-e`
/// 双保险保留（两种仓形态各一）。
#[test]
fn restore_baseline_不带x_gitignore免疫与排除面双保险() {
    let _guard = lock_path();

    // 形态一：.gitignore 覆盖 node_modules（依赖树常态）——clean 无 -x 不碰
    {
        let (_dir, root) = init_repo("restore-ignore");
        fs_write(&root, ".gitignore", "node_modules/\n");
        git(&root, &["add", ".gitignore"]);
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
                "ignore",
            ],
        );
        let modules = root.join("node_modules").join("lib");
        std::fs::create_dir_all(&modules).expect("建依赖树");
        std::fs::write(modules.join("index.js"), "module.exports={};").expect("写依赖");

        git::restore_baseline(&root, &[".*", "openspec"]).expect("恢复应 Ok");
        assert!(
            modules.join("index.js").is_file(),
            "被 .gitignore 覆盖的依赖树保留（不带 -x——尊重 ignore 规则）"
        );
    }

    // 形态二：无 ignore 规则仓的同名目录——显式 -e 双保险保留
    {
        let (_dir, root) = init_repo("restore-no-ignore");
        let modules = root.join("node_modules").join("lib");
        std::fs::create_dir_all(&modules).expect("建依赖树");
        std::fs::write(modules.join("index.js"), "module.exports={};").expect("写依赖");

        git::restore_baseline(&root, &["node_modules", ".*", "openspec"]).expect("恢复应 Ok");
        assert!(
            modules.join("index.js").is_file(),
            "无 ignore 规则仓的同名目录经 -e 双保险保留"
        );
    }
}

/// 幂等安全（边界，重试链 / AC-5 幂等半边）：干净仓（零改动零 untracked）二
/// 次调用 Ok 且仓态不变；exclusions 空切片 → 纯 reset + clean -fd 照常 Ok。
#[test]
fn restore_baseline_幂等二次调用与空排除面() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("restore-idempotent");

    let first = git::restore_baseline(&root, &[".*", "openspec"]).expect("首次恢复应 Ok");
    assert!(!first.is_empty());
    let second = git::restore_baseline(&root, &[".*", "openspec"]).expect("二次恢复应 Ok");
    assert!(!second.is_empty(), "干净仓二次调用 Ok（幂等安全）");
    let status = git(&root, &["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "仓态不变: {status:?}");
    assert_eq!(head_sha(&root), head_sha(&root), "HEAD 不移位");

    // 空排除面：纯 reset + clean -fd 照常 Ok
    let (_dir2, root2) = init_repo("restore-empty-exclusions");
    fs_write(&root2, "junk.txt", "未跟踪\n");
    git::restore_baseline(&root2, &[]).expect("空排除面恢复应 Ok");
    let status2 = git(&root2, &["status", "--porcelain"]);
    assert!(
        status2.trim().is_empty(),
        "空排除面 = 纯 reset + clean -fd: {status2:?}"
    );
}

/// 非 git 目录（异常）：`Err` 沿既有 `git()` 语境（命令串 + 退出码 + stderr
/// ——不吞不静默）。
#[test]
fn restore_baseline_非git目录err沿git语境() {
    let _guard = lock_path();
    let (_dir, root) = plain_dir("restore-plain");

    let error = git::restore_baseline(&root, &[".*"]).expect_err("非 git 目录应 Err");
    assert_git_context(&error);
}

/// 嵌套排除名（边界 / design 待决注记真件护栏面）：排除目录内再建同名子目录
///（`node_modules/node_modules`）——`-e` 形态匹配下两层同保（git clean -e 按
/// 路径段匹配，嵌套同名不漏清不误清）。
#[test]
fn restore_baseline_嵌套排除名两层同保() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("restore-nested");

    let nested = root.join("node_modules").join("node_modules");
    std::fs::create_dir_all(&nested).expect("建嵌套目录");
    std::fs::write(nested.join("deep.bin"), b"deep").expect("写深位产物");

    git::restore_baseline(&root, &["node_modules", ".*", "openspec"]).expect("恢复应 Ok");
    assert!(
        nested.join("deep.bin").is_file(),
        "嵌套同名排除目录两层同保（-e 路径段匹配）"
    );
}

// ---------------------------------------------------------------------------
// Windows 保留设备名（git 无法索引；Git Bash 的 `2>nul` 会落成真实文件）：
// 过滤 / 忽略规则 / commit 守卫——真实 `\\?\` 文件装置
// ---------------------------------------------------------------------------

/// `\\?\` 扩展长度路径上的真实保留名文件；Drop 经扩展路径删除（`TempDir`
/// 的普通 `remove_dir_all` 可能删不掉保留名文件）。
#[cfg(windows)]
struct ReservedFile {
    path: PathBuf,
}

#[cfg(windows)]
impl Drop for ReservedFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 创建真实保留名文件：`File::create("nul")` 打开的是设备，`\\?\` 前缀才
/// 能落成普通文件（`nul` / `NUL.txt` / `dir/aux.log` / 尾随空格形态均适用）。
#[cfg(windows)]
fn fs_write_reserved(root: &Path, relative_posix: &str, content: &str) -> ReservedFile {
    let target = root.join(relative_posix.replace('/', "\\"));
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).expect("建保留名父目录失败");
    }
    let raw = PathBuf::from(format!("\\\\?\\{}", target.display()));
    std::fs::write(&raw, content).expect("创建 Windows 保留设备名文件失败");
    ReservedFile { path: raw }
}

#[cfg(windows)]
fn reserved_file_is_file(root: &Path, relative_posix: &str) -> bool {
    let target = root.join(relative_posix.replace('/', "\\"));
    let raw = PathBuf::from(format!("\\\\?\\{}", target.display()));
    std::fs::symlink_metadata(raw)
        .map(|meta| meta.is_file())
        .unwrap_or(false)
}

/// `git check-ignore -q` 直驱（退出 1 = 未命中，不能用断言成功的 `git()` 助手）。
#[cfg(windows)]
fn is_gitignored(root: &Path, path: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "--no-index", "-q", "--", path])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// 表征锚：Git for Windows 对保留名的拒绝面为真——`ls-files` 列出、原始
/// `add -A` / `add -N` 失败、精确 `:(exclude,literal)` 可绕过。若未来 git
/// 改变该行为，此用例先红，提示重新评估 commit 守卫层。
#[cfg(windows)]
#[test]
fn windows保留名_表征_add整批失败且精确排除可绕过() {
    let (_dir, root) = init_repo("reserved-characterization");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");
    fs_write(&root, "valid.txt", "valid\n");

    let untracked = git(&root, &["ls-files", "--others", "--exclude-standard"]);
    assert!(
        untracked.lines().any(|line| line == "nul"),
        "ls-files 应列出 nul: {untracked}"
    );

    let add_all = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["add", "-A"])
        .output()
        .expect("装置 git 拉起失败");
    assert!(!add_all.status.success(), "原始 add -A 应整批失败");
    let add_all_stderr = String::from_utf8_lossy(&add_all.stderr);
    assert!(
        add_all_stderr.contains("nul")
            && (add_all_stderr.contains("unable to index file")
                || add_all_stderr.contains("invalid path")),
        "失败面应点名 nul（add -A 走索引短读 / add -N 走 invalid path）: {add_all_stderr}"
    );

    let add_n = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["add", "-N", "nul"])
        .output()
        .expect("装置 git 拉起失败");
    assert!(!add_n.status.success(), "add -N nul 应失败");

    git(&root, &["add", "-A", "--", ".", ":(exclude,literal)nul"]);
    let staged = git(&root, &["diff", "--cached", "--name-only", "-z"]);
    assert!(
        staged.contains("valid.txt"),
        "合法文件照常入索引: {staged:?}"
    );
    assert!(!staged.contains("nul"), "保留名不入索引: {staged:?}");
}

/// pattern 覆盖：共享清单 25 名逐个 `check-ignore` 命中，近邻不误伤。
#[cfg(windows)]
#[test]
fn windows保留名pattern覆盖共享清单且近邻不误伤() {
    let (_dir, root) = init_repo("reserved-patterns");
    git::ensure_windows_reserved_exclude(&root).expect("ensure 应成功");

    for name in foundation::windows_name::RESERVED_DEVICE_NAMES {
        let lower = name.to_ascii_lowercase();
        assert!(is_gitignored(&root, &lower), "pattern 应覆盖 {lower}");
        assert!(
            is_gitignored(&root, &format!("{lower}.txt")),
            "pattern 应覆盖 {lower}.txt"
        );
    }
    for name in ["com0", "com10", "lpt10", "nullable.txt", "console.txt"] {
        assert!(!is_gitignored(&root, name), "{name} 不应被忽略");
    }
}

/// 大写形态经 `status`（core.ignorecase=true）剔除——`check-ignore` 的
/// `--no-index` 面不保证读到 `core.ignorecase`，另以真实文件 + status 钉住。
#[cfg(windows)]
#[test]
fn windows保留名大写形态经status剔除() {
    let (_dir, root) = init_repo("reserved-case");
    git::ensure_windows_reserved_exclude(&root).expect("ensure 应成功");
    let _upper = fs_write_reserved(&root, "NUL.TXT", "junk\n");

    let status = git(&root, &["status", "--porcelain"]);
    assert!(
        !status.to_ascii_lowercase().contains("nul"),
        "status 不列大写形态: {status}"
    );
}

/// `change_edit_set`：保留名剔除、合法 untracked 保留（含大小写/嵌套/扩展名族）。
#[cfg(windows)]
#[test]
fn change_edit_set_windows保留名剔除_合法文件保留() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-edit-set");
    fs_write(&root, "a.txt", "edited\n");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");
    let _upper = fs_write_reserved(&root, "NUL.txt", "junk\n");
    let _nested = fs_write_reserved(&root, "dir/aux.log", "junk\n");
    let _com = fs_write_reserved(&root, "com1", "junk\n");
    fs_write(&root, "nullable.txt", "valid\n");
    fs_write(&root, "console.txt", "valid\n");
    fs_write(&root, "com10", "valid\n");

    let files = vcs.change_edit_set(&root, "HEAD").expect("清单应成功");
    assert_eq!(
        files,
        vec![
            "a.txt".to_owned(),
            "com10".to_owned(),
            "console.txt".to_owned(),
            "nullable.txt".to_owned(),
        ],
        "仅合法文件入清单: {files:?}"
    );
}

/// `change_edit_set`：仅保留名 → `Ok(空)`（不是 Err——git 枚举本身成功）。
#[cfg(windows)]
#[test]
fn change_edit_set_仅保留名_空清单ok() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-only-set");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");

    let files = vcs.change_edit_set(&root, "HEAD").expect("清单应成功");
    assert!(files.is_empty(), "仅保留名 → 空清单: {files:?}");
}

/// `change_edit_patch`：保留名不阻断，合法 untracked / tracked 修改照常入补丁；
/// 保留名无 intent-to-add 残留。
#[cfg(windows)]
#[test]
fn change_edit_patch_windows保留名不阻断_合法文件入补丁() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-patch");
    fs_write(&root, "a.txt", "edited\n");
    fs_write(&root, "fresh.rs", "pub fn f() {}\n");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");
    let _nested = fs_write_reserved(&root, "dir/aux.log", "junk\n");

    let patch = vcs.change_edit_patch(&root, "HEAD").expect("补丁应成功");
    assert!(
        patch.contains("diff --git a/fresh.rs b/fresh.rs"),
        "合法 untracked 入补丁: {patch}"
    );
    assert!(
        patch.contains("diff --git a/a.txt b/a.txt"),
        "tracked 修改入补丁: {patch}"
    );
    // `--- /dev/null` 是合法新文件段头（含子串 nul），按段头路径断言而非裸子串
    assert!(
        !patch.contains("diff --git a/nul") && !patch.contains("diff --git a/dir/aux.log"),
        "保留名不入补丁: {patch}"
    );

    let cached = git(&root, &["ls-files", "--cached", "-z"]);
    assert!(
        !cached.contains("nul"),
        "保留名无 intent-to-add: {cached:?}"
    );
}

/// `change_edit_patch`：仅保留名 → 空补丁、零 `add -N` 副作用。
#[cfg(windows)]
#[test]
fn change_edit_patch_仅保留名_空补丁且零intent_to_add() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-only-patch");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");

    let patch = vcs.change_edit_patch(&root, "HEAD").expect("补丁应成功");
    assert!(patch.is_empty(), "仅保留名 → 空补丁: {patch:?}");
    let cached = git(&root, &["ls-files", "--cached", "-z"]);
    assert!(!cached.contains("nul"), "零 intent-to-add: {cached:?}");
}

/// ensure 后三面全剔：`change_edit_set`、`status --porcelain`、`add -A` 索引面。
#[cfg(windows)]
#[test]
fn ensure后_windows保留名三面全剔() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-three-faces");
    git::ensure_windows_reserved_exclude(&root).expect("ensure 应成功");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");
    fs_write(&root, "a.txt", "edited\n");

    let files = vcs.change_edit_set(&root, "HEAD").expect("清单应成功");
    assert_eq!(files, vec!["a.txt".to_owned()], "清单不含保留名");

    let status = git(&root, &["status", "--porcelain"]);
    assert!(!status.contains("nul"), "status 不列保留名: {status}");

    git(&root, &["add", "-A"]);
    let staged = git(&root, &["diff", "--cached", "--name-only", "-z"]);
    assert!(!staged.contains("nul"), "add -A 不收录: {staged:?}");
    assert!(staged.contains("a.txt"), "普通编辑照常收录: {staged:?}");
}

/// `commit_all`：保留名不进提交，合法改动照常提交，提交后 `dirty` false
///（守住既有“worktree 全域提交后干净”不变量）。
#[cfg(windows)]
#[test]
fn commit_all_windows保留名不进提交且提交后干净() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-commit-all");
    fs_write(&root, "a.txt", "edited\n");
    fs_write(&root, "new-valid.txt", "valid\n");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");

    vcs.commit_all(&root, "archive: reserved")
        .expect("commit_all 应成功");
    let tree = git(&root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(tree.lines().any(|line| line == "a.txt"), "修改入树: {tree}");
    assert!(
        tree.lines().any(|line| line == "new-valid.txt"),
        "合法新文件入树: {tree}"
    );
    assert!(
        !tree.lines().any(|line| line == "nul"),
        "保留名不入树: {tree}"
    );
    assert!(!vcs.dirty(&root, &[]), "提交后干净（保留名被忽略）");
}

/// 仅保留名 + ignore 生效 → `dirty` false（生产 archive Commit 步据此跳过）。
#[cfg(windows)]
#[test]
fn windows保留名_仅nul时dirty为false() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-dirty-only");
    git::ensure_windows_reserved_exclude(&root).expect("ensure 应成功");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");

    assert!(!vcs.dirty(&root, &[]), "ignore 生效后仅保留名不顶真 dirty");
}

/// pattern 漏网分支（尾随空格形态）：`commit_all` 精确排除后空暂存按幂等
/// 成功收口，零新提交。
#[cfg(windows)]
#[test]
fn commit_all_pattern漏网_空暂存收口零提交() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-commit-miss");
    let _nul = fs_write_reserved(&root, "nul ", "junk\n");
    let before = git(&root, &["rev-list", "--count", "HEAD"]);

    vcs.commit_all(&root, "archive: reserved")
        .expect("pattern 漏网应幂等收口");
    let after = git(&root, &["rev-list", "--count", "HEAD"]);
    assert_eq!(after, before, "仅保留名 → 零新提交");
}

/// `commit_paths`：圈内有效改动 + 普通保留名（ignore 生效）→ Ok。
#[cfg(windows)]
#[test]
fn commit_paths_有效改动照常提交_普通保留名ignore生效() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-commit-paths");
    std::fs::create_dir_all(root.join("openspec").join("changes").join("demo"))
        .expect("建 change 目录");
    fs_write(&root, "openspec/changes/demo/proposal.md", "v1\n");
    git(&root, &["add", "openspec/changes/demo/proposal.md"]);
    git(&root, &["commit", "-q", "-m", "seed"]);
    fs_write(&root, "openspec/changes/demo/proposal.md", "v2\n");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");

    vcs.commit_paths(&root, &["openspec/changes/demo"], "archive: demo")
        .expect("commit_paths 应成功");
    let tree = git(&root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(
        tree.lines()
            .any(|line| line == "openspec/changes/demo/proposal.md"),
        "圈内改动入树: {tree}"
    );
    assert!(
        !tree.lines().any(|line| line == "nul"),
        "保留名不入树: {tree}"
    );
}

/// `commit_paths`：pattern 漏网且圈内仅剩保留名 → 显式 Err 点名路径。
#[cfg(windows)]
#[test]
fn commit_paths_圈内仅保留名_显式err携路径() {
    let _guard = lock_path();
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("reserved-commit-paths-miss");
    std::fs::create_dir_all(root.join("openspec").join("changes").join("demo"))
        .expect("建 change 目录");
    let _nul = fs_write_reserved(&root, "openspec/changes/demo/nul ", "junk\n");

    let error = vcs
        .commit_paths(&root, &["openspec/changes/demo"], "archive: demo")
        .expect_err("圈内仅保留名应 Err");
    assert!(error.contains("保留设备名"), "错误应点名保留名: {error}");
    assert!(error.contains("nul"), "错误应含路径: {error}");
}

/// `restore_baseline`：保留名在盘 → `clean -fd` 不炸，文件留盘但 git 不可见。
#[cfg(windows)]
#[test]
fn restore_baseline_windows保留名_clean成功且status干净() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("reserved-restore");
    let _nul = fs_write_reserved(&root, "nul", "junk\n");

    git::restore_baseline(&root, &[".*", "openspec"]).expect("恢复应 Ok");
    assert!(reserved_file_is_file(&root, "nul"), "忽略面文件留盘");
    assert!(
        git(&root, &["status", "--porcelain"]).trim().is_empty(),
        "status 干净（保留名被忽略）"
    );
}

/// POSIX 反向：`nul` 是合法文件名，不得按 Windows 保留名过滤。
#[cfg(not(windows))]
#[test]
fn posix_nul是合法文件名_不按windows保留名过滤() {
    let vcs = archive_vcs();
    let (_dir, root) = init_repo("posix-nul");
    fs_write(&root, "nul", "posix\n");

    let files = vcs.change_edit_set(&root, "HEAD").expect("清单应成功");
    assert!(
        files.iter().any(|file| file == "nul"),
        "POSIX 不过滤: {files:?}"
    );
}

// ---------------------------------------------------------------------------
// placement（落位 + 偏移一次派生：`rev-parse --show-toplevel --show-prefix`）
// ---------------------------------------------------------------------------

/// 「子目录 workspace」真实仓：`rel` 目录内提交一个文件（HEAD 在案），返回
/// （仓 RAII，仓根，workspace root）。
fn init_repo_with_subdir(tag: &str, rel: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let (dir, root) = init_repo(tag);
    let workspace = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
    std::fs::create_dir_all(&workspace).expect("建子目录 workspace 失败");
    fs_write(&workspace, "keep.txt", "子目录 workspace\n");
    git(&root, &["add", "--", rel]);
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
            "subdir",
        ],
    );
    (dir, root, workspace)
}

/// placement（正向）：workspace root = git 根 → 返回 git 根且 `rel = ""`
/// （`--show-prefix` 空行归一）。
#[test]
fn placement_git根workspace_返git根与空rel() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("placement-root");

    let (toplevel, rel) = git::placement(&root).expect("git 根 workspace 派生应成功");

    assert_eq!(
        std::fs::canonicalize(&toplevel).expect("canonicalize toplevel"),
        std::fs::canonicalize(&root).expect("canonicalize repo root"),
        "git 根 workspace → toplevel = 仓根"
    );
    assert_eq!(rel, "", "`--show-prefix` 空行归一为 `\"\"`");
}

/// placement（正向）：workspace root = `<repo>/packages/desktop` → `rel =
/// "packages/desktop"`（`/` 分隔、无前导 / 尾随 `/`）。
#[test]
fn placement_子目录workspace_rel为相对偏移() {
    let _guard = lock_path();
    let (_dir, root, workspace) = init_repo_with_subdir("placement-sub", "packages/desktop");

    let (toplevel, rel) = git::placement(&workspace).expect("子目录 workspace 派生应成功");

    assert_eq!(
        std::fs::canonicalize(&toplevel).expect("canonicalize toplevel"),
        std::fs::canonicalize(&root).expect("canonicalize repo root"),
        "落位锚恒 git 根（与 workspace 位置无关）"
    );
    assert_eq!(rel, "packages/desktop", "rel = 相对偏移（`/` 分隔）");
    assert!(!rel.starts_with('/'), "无前导 `/`");
    assert!(!rel.ends_with('/'), "无尾随 `/`（trim_end_matches 归一）");
}

/// placement（边界）：深层子目录（`client/app/web`）→ `rel` 逐段保真、无尾
/// `/`。
#[test]
fn placement_深层子目录_rel逐段保真无尾斜杠() {
    let _guard = lock_path();
    let (_dir, _root, workspace) = init_repo_with_subdir("placement-deep", "client/app/web");

    let (_toplevel, rel) = git::placement(&workspace).expect("深层子目录派生应成功");

    assert_eq!(rel, "client/app/web", "逐段保真");
    assert!(!rel.ends_with('/'), "无尾 `/`");
    assert_eq!(rel.split('/').count(), 3, "三段可切分");
}

/// placement（边界）：一次 spawn 取两值——`rev-parse --show-toplevel
/// --show-prefix` 同调用（`--show-prefix` 不再独立 spawn）。判据：两行输出互
/// 洽（toplevel + rel 组件级等于 workspace root）。
#[test]
fn placement_一次spawn两行互洽_toplevel加rel即workspace() {
    let _guard = lock_path();
    let (_dir, _root, workspace) = init_repo_with_subdir("placement-one-spawn", "packages/desktop");

    let (toplevel, rel) = git::placement(&workspace).expect("派生应成功");

    assert_eq!(
        std::fs::canonicalize(toplevel.join(&rel)).expect("canonicalize 拼回路径"),
        std::fs::canonicalize(&workspace).expect("canonicalize workspace"),
        "两值同源互洽（同一次 rev-parse 的两行）"
    );
}

/// placement（边界）：CRLF / 空白归一——rel 含内部空格时原样保真、尾部无残
/// 留空白字符；toplevel 无 `\r` / `\n` 残留。
#[test]
fn placement_路径含空格_残留空白零() {
    let _guard = lock_path();
    let (_dir, _root, workspace) = init_repo_with_subdir("placement-space", "with space/app");

    let (toplevel, rel) = git::placement(&workspace).expect("含空格路径派生应成功");

    assert_eq!(rel, "with space/app", "内部空格原样保真");
    assert!(
        !rel.ends_with(' ') && !rel.ends_with('\r') && !rel.ends_with('\n'),
        "rel 尾部无残留空白: {rel:?}"
    );
    let toplevel_raw = toplevel.to_string_lossy().into_owned();
    assert!(
        !toplevel_raw.contains('\r') && !toplevel_raw.contains('\n'),
        "toplevel 无 CR / LF 残留: {toplevel_raw:?}"
    );
    assert_eq!(toplevel_raw.trim_end(), toplevel_raw, "toplevel 尾部无空白");
}

/// placement（异常）：非 git 目录 → `Err` 含「非 git 仓」+ `git init` 引导
/// （文案自 `toplevel` 原样迁移）。
#[test]
fn placement_非git目录_err含非git仓与init引导() {
    let _guard = lock_path();
    let (_dir, plain) = plain_dir("placement-plain");

    let error = git::placement(&plain).expect_err("非 git 目录应 Err");

    assert!(error.contains("非 git 仓"), "Err 含「非 git 仓」: {error}");
    assert!(error.contains("git init"), "Err 含 git init 引导: {error}");
}

/// placement（异常）：裸仓（`git init --bare`）→ `Err` 走「非 git 仓」中文引导
/// （上游 stderr 的 `must be run in a work tree` 仅作内部匹配键，不进产品文案），
/// 可辨于空输出档。
#[test]
fn placement_裸仓_err走非git仓中文引导() {
    let _guard = lock_path();
    let dir = tempfile::Builder::new()
        .prefix("vcs-git-bare-placement-")
        .tempdir()
        .expect("创建裸仓临时目录失败");
    let bare = dir.path().join("bare.git");
    git(
        dir.path(),
        &["init", "-q", "--bare", bare.to_string_lossy().as_ref()],
    );

    let error = git::placement(&bare).expect_err("裸仓应 Err");

    assert!(error.contains("非 git 仓"), "Err 含「非 git 仓」: {error}");
    assert!(error.contains("git init"), "Err 含 git init 引导: {error}");
    assert!(
        !error.contains("must be run in a work tree"),
        "上游 stderr 英文串不得进入产品文案: {error}"
    );
    assert!(
        !error.contains("工作树根解析为空"),
        "裸仓档不得与空输出档混同: {error}"
    );
}

/// placement（异常，边界）：`--show-prefix` 输出为空行且 `--show-toplevel`
/// 亦空 → `Err` 含「工作树根解析为空」——真 git 无法产出该形态（裸仓已由
/// work-tree 引导先接住），以源字面量自证守卫在场。
#[test]
fn placement_空输出守卫在场_源字面量自证() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("git.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取 {} 失败: {error}", path.display()));

    assert!(
        source.contains("的 git 工作树根解析为空（rev-parse --show-toplevel 无输出）"),
        "空输出守卫文案在场（自 toplevel 原样迁移）"
    );
    assert!(
        source.contains("rev-parse --show-toplevel --show-prefix"),
        "一次 spawn 取两值（`--show-prefix` 不独立 spawn）"
    );
}

// ---------------------------------------------------------------------------
// tracked_at_head（`HEAD:<rel>` 在案判据：退出 0 / 1 / 其余三分）
// ---------------------------------------------------------------------------

/// tracked_at_head（正向）：子目录在 HEAD（内含至少一个 tracked 文件）→
/// `Ok(true)`。
#[test]
fn tracked_at_head_在head_ok_true() {
    let _guard = lock_path();
    let (_dir, root, _workspace) = init_repo_with_subdir("tracked-ok", "packages/desktop");

    assert!(
        git::tracked_at_head(&root, "packages/desktop").expect("判据应 Ok"),
        "HEAD:<rel> 在案 → true"
    );
    assert!(
        git::tracked_at_head(&root, "packages/desktop/keep.txt").expect("判据应 Ok"),
        "文件级路径同样在案 → true"
    );
}

/// tracked_at_head（异常）：子目录整体未跟踪（本地新建目录）→ `Ok(false)`
/// （退出码 1）。
#[test]
fn tracked_at_head_未跟踪目录_ok_false() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("tracked-untracked");
    std::fs::create_dir_all(root.join("fresh/child")).expect("建本地新目录失败");
    fs_write(&root, "fresh/child/x.txt", "未跟踪内容\n");

    assert!(
        !git::tracked_at_head(&root, "fresh").expect("判据应 Ok"),
        "未跟踪目录 → false（非 Err）"
    );
}

/// tracked_at_head（异常）：HEAD 已删除该子目录、工作区仍有未跟踪残留 →
/// `Ok(false)`。
#[test]
fn tracked_at_head_head已删除工作区留残留_ok_false() {
    let _guard = lock_path();
    let (_dir, root, workspace) = init_repo_with_subdir("tracked-deleted", "packages/desktop");
    git(
        &root,
        &["rm", "-r", "--cached", "-q", "--", "packages/desktop"],
    );
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
            "drop subdir",
        ],
    );
    assert!(workspace.join("keep.txt").is_file(), "工作区残留仍在场");

    assert!(
        !git::tracked_at_head(&root, "packages/desktop").expect("判据应 Ok"),
        "HEAD 已删除 → false（工作区残留不影响判据）"
    );
}

/// tracked_at_head（异常）：子目录被 `.gitignore` 命中 → `Ok(false)`。
#[test]
fn tracked_at_head_gitignore命中_ok_false() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("tracked-ignored");
    fs_write(&root, ".gitignore", "ignored-sub/\n");
    git(&root, &["add", ".gitignore"]);
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
            "ignore",
        ],
    );
    std::fs::create_dir_all(root.join("ignored-sub")).expect("建被忽略目录失败");
    fs_write(&root, "ignored-sub/a.txt", "被忽略\n");

    assert!(
        !git::tracked_at_head(&root, "ignored-sub").expect("判据应 Ok"),
        "gitignore 命中 → false"
    );
}

/// tracked_at_head（边界）：子目录被跟踪但仅含被 ignore 的文件 →
/// `Ok(true)`（判据只答「HEAD 有没有这个路径」，不答「目录里有什么」）。
#[test]
fn tracked_at_head_仅含被ignore文件_ok_true() {
    let _guard = lock_path();
    let (_dir, root, workspace) = init_repo_with_subdir("tracked-ignored-only", "packages/desktop");
    fs_write(&workspace, ".gitignore", "*.log\n");
    git(&root, &["add", "--", "packages/desktop/.gitignore"]);
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
            "ignore only",
        ],
    );
    fs_write(&workspace, "debug.log", "被 ignore 的内容\n");

    assert!(
        git::tracked_at_head(&root, "packages/desktop").expect("判据应 Ok"),
        "路径在 HEAD 即在案（内容是否被 ignore 不参与判据）"
    );
}

/// tracked_at_head（边界）：`rel = ""` → `Ok(true)` 且零 spawn（清空 PATH 下
/// 仍 `Ok(true)`，可证零 spawn）。
#[test]
fn tracked_at_head_rel空串_ok_true且零spawn() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("tracked-empty-rel");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let result = git::tracked_at_head(&root, "");
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    assert!(
        result.expect("rel 空串应 Ok(true)"),
        "git 不可发现仍 Ok(true)（零 spawn 直接返回）"
    );
}

/// tracked_at_head（异常）：退出码既非 0 也非 1（如 128，路径非法 / 仓库损
/// 坏）→ `Err`（fail-closed，与 `Ok(false)` 严格可分）。
#[test]
fn tracked_at_head_退出码128_err与不在案可辨() {
    let _guard = lock_path();
    let dir = tempfile::Builder::new()
        .prefix("vcs-git-broken-")
        .tempdir()
        .expect("创建临时目录失败");
    let broken = dir.path().to_path_buf();
    fs_write(&broken, ".git", "garbage\n");

    let error = git::tracked_at_head(&broken, "packages/desktop").expect_err("退出码 128 应 Err");

    assert!(error.contains("退出码 128"), "Err 携退出码，实际: {error}");
    assert!(
        !error.contains("不在 git HEAD 中"),
        "Err 不伪装成「未跟踪」（与 Ok(false) 判据可辨），实际: {error}"
    );
}

/// tracked_at_head（异常）：git 不可发现（PATH 注入）→ `Err` 含「git 不可
/// 用」引导，不伪装成「未跟踪」。
#[test]
fn tracked_at_head_git不可发现_err引导() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("tracked-missing-git");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let result = git::tracked_at_head(&root, "packages/desktop");
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let error = result.expect_err("git 不可发现应 Err");
    assert!(
        error.contains("git 不可用"),
        "Err 含「git 不可用」引导，实际: {error}"
    );
}

/// tracked_at_head（边界）：`rel` 超长（> 1000 字符多层路径）→ 参数原样入
/// 线，结果稳定不 panic（`Ok` / `Err` 之一且文案可读）。
#[test]
fn tracked_at_head_rel超长_结果稳定不panic() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("tracked-long-rel");
    let segments: Vec<String> = (0..40)
        .map(|index| format!("layer{index:02}").repeat(5))
        .collect();
    let rel = format!("{}/tail", segments.join("/"));
    assert!(
        rel.len() > 1000,
        "夹具 rel 应超 1000 字符，实际 {}",
        rel.len()
    );

    let first = git::tracked_at_head(&root, &rel);
    let second = git::tracked_at_head(&root, &rel);

    assert_eq!(
        first.as_ref().map_err(|error| error.clone()),
        second.as_ref().map_err(|error| error.clone()),
        "同参数结果稳定（零 panic）"
    );
    match first {
        Ok(tracked) => assert!(!tracked, "超长未跟踪路径 → false"),
        Err(error) => assert!(
            error.contains("rev-parse") || error.contains("git "),
            "Err 文案可读: {error}"
        ),
    }
}

// ---------------------------------------------------------------------------
// git 退出码通道（`git_raw` 新增私有通道不改 `git()` 既有语义）
// ---------------------------------------------------------------------------

/// `git()`（正向）：非零退出仍 `Err` 且文案「git <args> 失败（退出码 N）:
/// stderr」逐字保留。
#[test]
fn git非零退出_err文案逐字保留() {
    let _guard = lock_path();
    let (_dir, plain) = plain_dir("git-nonzero");

    let error = git::git(&plain, &["status", "--porcelain"]).expect_err("非 git 目录应 Err");

    assert!(
        error.contains("git status --porcelain 失败（退出码 "),
        "Err 前缀携命令串与退出码，实际: {error}"
    );
    assert!(
        error.contains("not a git repository"),
        "Err 携 stderr 原文，实际: {error}"
    );
}

/// `git()`（边界）：退出码 0 且 stdout 为空 → `Ok("")`（空面语义不变——空输
/// 出由各调用点自判，如 `placement` 空 toplevel 仍 `Err`）。
#[test]
fn git退出0stdout空_空面语义不变() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("git-empty-stdout");

    let stdout = git::git(&root, &["status", "--porcelain"]).expect("干净仓应 Ok");

    assert_eq!(stdout, "", "退出 0 + 空 stdout → Ok(空串)");
}

// ---------------------------------------------------------------------------
// git 步锚点（host 全树 vs 执行根子目录：AC-4 / R-2 的真件对照面）
// ---------------------------------------------------------------------------

/// `restore_baseline`（正向）：子目录 workspace 夹具传 host → `reset --hard`
/// 覆盖**整个** worktree（域外 tracked 改动被恢复），`clean -fd` 清域外
/// untracked。
#[test]
fn restore_baseline_传host_域外tracked与untracked双恢复() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("restore-host");
    let (_wt_dir, host) = worktree_slot("restore-host");
    git::add_worktree(&root, &host, "change/subdir").expect("建域应成功");

    // 域外（rel 之外）改动 + 域外 untracked 残留
    fs_write(&host, "a.txt", "域外 tracked 改动\n");
    fs_write(&host, "outside-junk.txt", "域外 untracked 残留\n");
    std::fs::create_dir_all(host.join("packages/desktop")).expect("建执行根目录失败");
    fs_write(&host, "packages/desktop/inside.txt", "域内 untracked\n");

    git::restore_baseline(&host, &[".*", "openspec"]).expect("恢复应 Ok");

    assert_eq!(
        std::fs::read_to_string(host.join("a.txt")).expect("读回 a.txt"),
        "initial\n",
        "域外 tracked 改动被 reset --hard 恢复"
    );
    assert!(
        !host.join("outside-junk.txt").exists(),
        "域外 untracked 残留被 clean -fd 清除（不缩域）"
    );
    assert!(
        !host.join("packages/desktop/inside.txt").exists(),
        "域内 untracked 同样被清（host 全树）"
    );
}

/// `restore_baseline`（边界，误传锚形态）：同一夹具传 exec root（host/rel 子
/// 目录）→ `reset --hard` 仍全域生效，但 `clean -fd` **缩域**（域外 untracked
/// 留存）——与传 host 的对照面形成可辨差异。
#[test]
fn restore_baseline_传执行根_reset全域而clean缩域() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("restore-exec-root");
    let (_wt_dir, host) = worktree_slot("restore-exec-root");
    git::add_worktree(&root, &host, "change/subdir").expect("建域应成功");
    let exec_root = host.join("packages/desktop");
    std::fs::create_dir_all(&exec_root).expect("建执行根目录失败");

    fs_write(&host, "a.txt", "域外 tracked 改动\n");
    fs_write(&host, "outside-junk.txt", "域外 untracked 残留\n");
    fs_write(&exec_root, "inside-junk.txt", "域内 untracked 残留\n");

    git::restore_baseline(&exec_root, &[".*", "openspec"]).expect("恢复应 Ok");

    assert_eq!(
        std::fs::read_to_string(host.join("a.txt")).expect("读回 a.txt"),
        "initial\n",
        "reset --hard 与 cwd 无关（域外 tracked 改动仍被恢复）"
    );
    assert!(
        host.join("outside-junk.txt").is_file(),
        "clean -fd 缩域：域外 untracked 残留留存（锚点不可替换的反向对照）"
    );
    assert!(
        !exec_root.join("inside-junk.txt").exists(),
        "缩域内的 untracked 仍被清除"
    );
}

/// `worktree_snapshot`（正向）：子目录 workspace 夹具传 host → index 条目含
/// 域外条目（`ls-files -s` 不缩域）。
#[test]
fn worktree_snapshot_传host_含域外index条目() {
    let _guard = lock_path();
    let (_dir, root) = init_repo("snapshot-host");
    let (_wt_dir, host) = worktree_slot("snapshot-host");
    git::add_worktree(&root, &host, "change/subdir").expect("建域应成功");
    std::fs::create_dir_all(host.join("packages/desktop")).expect("建执行根目录失败");

    let snapshot = git::worktree_snapshot(&host).expect("快照应成功");

    let paths: Vec<String> = snapshot
        .index
        .iter()
        .map(|entry| entry.path.clone())
        .collect();
    assert!(
        paths.iter().any(|path| path == "a.txt"),
        "index 含域外条目 a.txt: {paths:?}"
    );
}

/// `worktree_snapshot`（边界）：传 exec root → index 仅含 `rel` 子树条目
/// （缩域形态可见，证明锚点不可替换）。
#[test]
fn worktree_snapshot_传执行根_仅rel子树条目() {
    let _guard = lock_path();
    let (_dir, root, _workspace) = init_repo_with_subdir("snapshot-exec", "packages/desktop");
    let (_wt_dir, host) = worktree_slot("snapshot-exec");
    git::add_worktree(&root, &host, "change/subdir").expect("建域应成功");
    let exec_root = host.join("packages/desktop");
    assert!(exec_root.is_dir(), "执行根随检出在场");

    let snapshot = git::worktree_snapshot(&exec_root).expect("快照应成功");

    let paths: Vec<String> = snapshot
        .index
        .iter()
        .map(|entry| entry.path.clone())
        .collect();
    assert!(
        paths.iter().all(|path| path != "a.txt"),
        "index 不含域外条目（缩域可见）: {paths:?}"
    );
    assert!(
        paths.iter().any(|path| path == "keep.txt"),
        "index 仅含 rel 子树条目: {paths:?}"
    );
}
