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
    let _b = ProcessArchiveVcs::default();
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

    assert_eq!(
        vcs.branch_merged(&root, "change/feature")
            .expect("判定应 Ok"),
        false,
        "分支未合入 → false"
    );
    git(&root, &["merge", "--no-edit", "change/feature"]);
    assert_eq!(
        vcs.branch_merged(&root, "change/feature")
            .expect("判定应 Ok"),
        true,
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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();

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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();

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
    assert!(
        !root.join(".git/MERGE_HEAD").exists(),
        "主仓零 merge 残态"
    );
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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();
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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();

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
    vcs.ff_merge(&root, "change/empty").expect("Already up to date 亦 Ok");
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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();

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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();
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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();

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
    vcs.ff_merge(&root, "change/resolve").expect("收尾快进应成功");
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
    let (_wt_dir, wt) = worktree_slot("arch-continue-refused");
    git::add_worktree(&root, &wt, "change/refuse").expect("建域应成功");

    fs_write(&wt, "a.txt", "分支版本\n");
    git(&wt, &["add", "-A"]);
    commit_all_fixture(&wt, "branch edit");
    fs_write(&root, "a.txt", "主仓版本\n");
    git(&root, &["add", "-A"]);
    commit_all_fixture(&root, "main edit");
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();
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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();

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
    let onto = git(&root, &["branch", "--show-current"])
        .trim()
        .to_owned();

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
