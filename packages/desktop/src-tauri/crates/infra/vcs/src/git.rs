//! git 子命令族同步执行（`git -C <main_root>` 显式寻址，零 tokio）：worktree
//! 建域（`add -b`，HEAD 基线铸分支）/ 补偿面（`remove --force` / `branch -D`）
//! / 探测三连（`rev-parse` × 2 + `status --porcelain`）/ 分支在案校验
//! （`show-ref --verify`）。拉起失败（PATH 无 git）映射「git 不可用」引导
//! 文案；空仓 HEAD 失败映射「先提交」引导；非 git 仓显式 Err（调用方
//! MUST NOT 静默回退主 root 创建）。

use std::path::Path;
use std::process::{Command, Stdio};

use workflow::write::RepoProbe;

/// 一次 git 子命令执行（stdout 捕获；stdin 关闭）：非零退出以 stderr 上抛。
/// 拉起失败（PATH 未发现 git）映射统一引导文案（spawn error 与「命令存在但
/// 退出非零」可辨——前者是环境缺失，后者携带 git 语境 stderr）。
fn git(main_root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(main_root)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            format!(
                "git 不可用（PATH 未发现 git）: {error}；change worktree 建域依赖 git，\
                 请安装 git 并确认 PATH 可达后重试"
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git {} 失败（退出码 {}）: {}",
            args.join(" "),
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 探测三连（git 可用 + git 仓 + HEAD + 脏仓单点）：`rev-parse
/// --is-inside-work-tree` → `rev-parse HEAD` → `status --porcelain`。非 git
/// 仓 / 空仓（无 HEAD）各映射显式引导文案（不静默）。
pub(crate) fn probe(main_root: &Path) -> Result<RepoProbe, String> {
    let inside = git(main_root, &["rev-parse", "--is-inside-work-tree"]).map_err(|error| {
        // 「fatal: not a git repository」语境 → 非 git 仓引导（与 git 缺失可辨）
        if error.contains("not a git repository") {
            format!(
                "主仓 {} 为非 git 仓：change worktree 建域需要 git 仓库（不回退主 root 创建），\
                 请先 git init 并提交",
                main_root.display()
            )
        } else {
            error
        }
    })?;
    if inside.trim() != "true" {
        return Err(format!(
            "主仓 {} 为非 git 仓（rev-parse --is-inside-work-tree 非 true）：\
             change worktree 建域需要 git 仓库（不回退主 root 创建）",
            main_root.display()
        ));
    }
    let head = git(main_root, &["rev-parse", "HEAD"]).map_err(|error| {
        // 空仓：HEAD 引用不存在（unknown revision 语境）→ 先提交引导
        if error.contains("unknown revision") || error.contains("ambiguous argument") {
            format!(
                "主仓 {} 为空 git 仓（无任何提交，HEAD 不存在）：请先提交再开新 change",
                main_root.display()
            )
        } else {
            error
        }
    })?;
    let head = head.trim().to_owned();
    let status = git(main_root, &["status", "--porcelain"])?;
    Ok(RepoProbe {
        head,
        dirty: !status.trim().is_empty(),
    })
}

/// 分支在案校验：`show-ref --verify --quiet refs/heads/<branch>`（退出 0 =
/// 在案 true；退出 1 = miss false；其余 Err）。
pub(crate) fn branch_exists(main_root: &Path, branch: &str) -> Result<bool, String> {
    let reference = format!("refs/heads/{branch}");
    let output = Command::new("git")
        .arg("-C")
        .arg(main_root)
        .args(["show-ref", "--verify", "--quiet"])
        .arg(&reference)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("git 不可用（PATH 未发现 git）: {error}"))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git show-ref --verify {reference} 失败（退出码 {}）: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

/// worktree 建域：`git worktree add -b <branch> <path>`——基线恒主仓 HEAD
/// 检出（主仓未提交改动不进入 worktree），branch 于建域时铸出。
pub(crate) fn add_worktree(main_root: &Path, worktree: &Path, branch: &str) -> Result<(), String> {
    git(
        main_root,
        &["worktree", "add", "-b", branch, &worktree.to_string_lossy()],
    )
    .map(|_| ())
}

/// worktree 补偿移除：`git worktree remove --force <path>`（补偿面，尽力
/// 回收；目录登记与磁盘目录一并清）。
pub(crate) fn remove_worktree(main_root: &Path, worktree: &Path) -> Result<(), String> {
    git(
        main_root,
        &["worktree", "remove", "--force", &worktree.to_string_lossy()],
    )
    .map(|_| ())
}

/// 分支补偿删除：`git branch -D <branch>`（补偿面；不触碰主仓工作区）。
pub(crate) fn delete_branch(main_root: &Path, branch: &str) -> Result<(), String> {
    git(main_root, &["branch", "-D", branch]).map(|_| ())
}
