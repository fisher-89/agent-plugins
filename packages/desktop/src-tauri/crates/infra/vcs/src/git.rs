use std::path::Path;
use std::process::{Command, Stdio};

use workflow::write::RepoProbe;

/// 一次 git 子命令执行（stdout 捕获；stdin 关闭）：非零退出以 stderr 上抛。
/// 拉起失败（PATH 未发现 git）映射统一引导文案（spawn error 与「命令存在但
/// 退出非零」可辨——前者是环境缺失，后者携带 git 语境 stderr）。
pub(crate) fn git(main_root: &Path, args: &[&str]) -> Result<String, String> {
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

// ---------------------------------------------------------------------------
// 归档子命令族（design desktop-archive-change D8/D9）：归档链的提交 / 合入 /
// pathspec 提交 / 探测面——全部 `git -C <root>` 同步 spawn、argv 直传零 shell
// 包装（提交信息引号形态不适用——R7 留痕）
// ---------------------------------------------------------------------------

/// 脏探测：`status --porcelain [-- pathspec…]` 非空即真（空 paths = 全域；
/// gitignore 面不计）。探测失败（git 不可用等）映射 false——调用方以「干净」
/// 幂等跳过，失败面由后续显式 git 段（合入 / 提交）承载。
pub(crate) fn dirty(root: &Path, paths: &[&str]) -> bool {
    let mut args: Vec<&str> = vec!["status", "--porcelain"];
    if !paths.is_empty() {
        args.push("--");
        args.extend_from_slice(paths);
    }
    git(root, &args)
        .map(|output| !output.trim().is_empty())
        .unwrap_or(false)
}

/// worktree 全域提交：`add -A` + `commit -m`——add 范围 = worktree 全域
///（worktree 为 change 私有执行锚，全域即本 change 编辑集与 spec 同步产物；
/// `add -A` 尊重 .gitignore，bootstrap 的 node_modules 不进提交）。
pub(crate) fn commit_all(worktree: &Path, message: &str) -> Result<(), String> {
    git(worktree, &["add", "-A"])?;
    git(worktree, &["commit", "-m", message]).map(|_| ())
}

/// 祖先判定：`merge-base --is-ancestor <branch> HEAD`——退出 0 = 分支已合入
/// true、1 = 未合入 false、其余（分支缺失等）Err（与 miss 可辨）。
pub(crate) fn branch_merged(main_root: &Path, branch: &str) -> Result<bool, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(main_root)
        .args(["merge-base", "--is-ancestor"])
        .arg(branch)
        .arg("HEAD")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("git 不可用（PATH 未发现 git）: {error}"))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git merge-base --is-ancestor {branch} HEAD 失败（退出码 {}）: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

/// 主仓合入：`git merge --no-edit <branch>`（可 ff 则 ff、主仓前进则 merge
/// commit，信息用 git 默认）。非零退出先尽力 `merge --abort`（幂等——非
/// merge 态调用无害）收口半截冲突态，再 Err 带 git 语境与手动处置引导。
pub(crate) fn merge_branch(main_root: &Path, branch: &str) -> Result<(), String> {
    match git(main_root, &["merge", "--no-edit", branch]) {
        Ok(_) => Ok(()),
        Err(error) => {
            // 尽力收口（冲突态恢复干净；失败如「无 merge 可 abort」静默忽略）
            let _ = git(main_root, &["merge", "--abort"]);
            Err(format!(
                "{error}；归档合入失败（分支 {branch} 与主仓当前分支冲突或主仓状态不允许），\
                 已尽力执行 git merge --abort 收口：请手动处置冲突（自行 merge 解冲突或调整主仓\
                 状态）后重试归档"
            ))
        }
    }
}

/// 主仓当前分支名：`branch --show-current`（空输出 = detached HEAD → 显式
/// Err 引导——合入目标 = HEAD 所在分支，detached 无从合入）。
pub(crate) fn current_branch(main_root: &Path) -> Result<String, String> {
    let branch = git(main_root, &["branch", "--show-current"])?
        .trim()
        .to_owned();
    if branch.is_empty() {
        return Err(format!(
            "主仓 {} 处于 detached HEAD（branch --show-current 空输出）：归档合入目标不明确，\
             请先切换到目标分支后重试",
            main_root.display()
        ));
    }
    Ok(branch)
}

/// pathspec 圈定提交：`add -A -- <paths…>` + `commit -m <msg> -- <paths 各自
/// "/**" 形态>`——commit pathspec 用 glob 形态覆盖已删除路径（裸目录 pathspec
/// 对已删除目录报 "did not match"），且不受 index 干净度约束；无关 staged /
/// untracked 条目在 pathspec 圈外原样保留（不吞并纪律的机械保证）。
pub(crate) fn commit_paths(main_root: &Path, paths: &[&str], message: &str) -> Result<(), String> {
    let mut add_args: Vec<&str> = vec!["add", "-A", "--"];
    add_args.extend_from_slice(paths);
    git(main_root, &add_args)?;
    let globs: Vec<String> = paths.iter().map(|path| format!("{path}/**")).collect();
    let mut commit_args: Vec<&str> = vec!["commit", "-m", message, "--"];
    commit_args.extend(globs.iter().map(String::as_str));
    git(main_root, &commit_args).map(|_| ())
}
