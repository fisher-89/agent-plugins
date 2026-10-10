use std::path::Path;
use std::process::Stdio;

use orchestration::port::{IndexEntry, RebaseOutcome, StatusEntry, WorktreeSnapshot};
use workflow::write::RepoProbe;

/// 一次 git 子命令执行（stdout 捕获；stdin 关闭）
pub(crate) fn git(main_root: &Path, args: &[&str]) -> Result<String, String> {
    let output = exec::SystemCommand::bare("git")
        .into_std()
        .arg("-C")
        .arg(main_root)
        .args(args)
        .env("GIT_EDITOR", "true")
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
    let output = exec::SystemCommand::bare("git")
        .into_std()
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
// 归档子命令族（design desktop-archive-change D8/D9 + archive-rebase-merge）：
// 归档链的提交 / 合入（worktree 内 rebase 重放 + 主仓 ff-only 快进——线性历
// 史零 merge commit，冲突态保留在 worktree）/ 快照 / 代续走 / abort / pathspec
// 提交 / 探测面——全部 `git -C <root>` 同步 spawn、argv 直传零 shell 包装（提
// 交信息引号形态不适用——R7 留痕）；清单 / 快照读取全线 `-z`（NUL 分隔零引号
// 形态——路径引号与非 ASCII 形态免疫，R6）
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
    let output = exec::SystemCommand::bare("git")
        .into_std()
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

/// worktree 内分支重放（冲突态保留语义）
pub(crate) fn rebase_branch(worktree: &Path, onto: &str) -> Result<RebaseOutcome, String> {
    match git(worktree, &["rebase", "--empty=drop", onto]) {
        Ok(_) => Ok(RebaseOutcome::Rebased),
        Err(error) => match unmerged_files(worktree) {
            Ok(files) if !files.is_empty() => Ok(RebaseOutcome::Conflicted(files)),
            _ => {
                // 尽力收口（冲突态恢复干净；失败如「无 rebase 可 abort」静默忽略）
                let _ = git(worktree, &["rebase", "--abort"]);
                Err(format!(
                    "{error}；归档合入失败（分支重放至 {onto} 冲突或 worktree 状态不允许），\
                     已尽力执行 git rebase --abort 收口：请手动处置冲突（在 worktree 内自行\
                     rebase 解冲突或调整 worktree 状态）后重试归档"
                ))
            }
        },
    }
}

/// 主仓快进（重放的后半段）
pub(crate) fn ff_merge(main_root: &Path, branch: &str) -> Result<(), String> {
    git(main_root, &["merge", "--ff-only", branch])
        .map(|_| ())
        .map_err(|error| {
            format!(
                "{error}；主仓快进失败（分支重放已完成，快进前主仓前进或主仓状态不允许）：\
                 请重试归档（重试将重放至主仓新 tip）或手动 git merge --ff-only {branch} 收口"
            )
        })
}

/// unmerged 清单（冲突判据读取面）：`diff --name-only --diff-filter=U -z`——
/// NUL 分隔零引号形态（Windows 路径引号 / 非 ASCII 形态免疫，R6）。
fn unmerged_files(main_root: &Path) -> Result<Vec<String>, String> {
    let raw = git(main_root, &["diff", "--name-only", "--diff-filter=U", "-z"])?;
    Ok(raw
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect())
}

pub(crate) fn worktree_snapshot(root: &Path) -> Result<WorktreeSnapshot, String> {
    let head = git(root, &["rev-parse", "HEAD"])?.trim().to_owned();
    let rebase_head = rebase_head_sha(root)?;
    let status = parse_status_z(&git(root, &["status", "--porcelain", "-z"])?);
    let index = parse_ls_files_z(&git(root, &["ls-files", "-s", "-z"])?);
    Ok(WorktreeSnapshot {
        head,
        rebase_head,
        status,
        index,
    })
}

fn rebase_head_sha(root: &Path) -> Result<Option<String>, String> {
    let state_dir = git(root, &["rev-parse", "--git-path", "rebase-merge"])?;
    if !Path::new(state_dir.trim()).exists() {
        return Ok(None);
    }
    let output = exec::SystemCommand::bare("git")
        .into_std()
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "-q", "--verify", "REBASE_HEAD"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("git 不可用（PATH 未发现 git）: {error}"))?;
    match output.status.code() {
        Some(0) => Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        )),
        Some(1) => Ok(Some("<rebase-merge>".to_owned())),
        _ => Err(format!(
            "git rev-parse -q --verify REBASE_HEAD 失败（退出码 {}）: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

/// `status --porcelain -z` 解析：条目 = XY + 空格 + 路径，NUL 分隔；rename /
/// copy（X ∈ {R, C}）条目后随第二个 NUL 段（原路径）——跳过并记新路径（
/// `-z` 下路径裸出，零引号形态）。
fn parse_status_z(raw: &str) -> Vec<StatusEntry> {
    let segments: Vec<&str> = raw
        .split('\0')
        .filter(|segment| !segment.is_empty())
        .collect();
    let mut entries = Vec::new();
    let mut iter = segments.into_iter().peekable();
    while let Some(segment) = iter.next() {
        let bytes = segment.as_bytes();
        if bytes.len() < 3 {
            continue; // 畸形段（XY + 空格 + 至少一路径字符）——跳过
        }
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        if x == 'R' || x == 'C' {
            iter.next(); // rename 双段：第二段为原路径，跳过（记新路径）
        }
        entries.push(StatusEntry {
            x,
            y,
            path: segment[3..].to_owned(),
        });
    }
    entries
}

/// `ls-files -s -z` 解析：条目 = `<mode> <hash> <stage>\t<path>`，NUL 分隔。
fn parse_ls_files_z(raw: &str) -> Vec<IndexEntry> {
    raw.split('\0')
        .filter(|segment| !segment.is_empty())
        .filter_map(|segment| {
            let (meta, path) = segment.split_once('\t')?;
            let mut fields = meta.split_whitespace();
            Some(IndexEntry {
                mode: fields.next()?.to_owned(),
                hash: fields.next()?.to_owned(),
                stage: fields.next()?.parse::<u8>().ok()?,
                path: path.to_owned(),
            })
        })
        .collect()
}

/// rebase 续走收口：`git rebase --continue`——沿用重放提交既定信息（GIT_EDITOR
/// 已在 `git()` 钉死——零交互面）；多提交分支逐个重放，下一个提交冲突 → 非
/// 零退出且 unmerged 非空 → [`RebaseOutcome::Conflicted`]（调用方循环解算）；
/// 清单空 = 非冲突失败 → `Err` 带 git 语境（链侧附人工收口引导——解算成果
/// 已验，不自动 abort）。
pub(crate) fn rebase_continue(worktree: &Path) -> Result<RebaseOutcome, String> {
    match git(worktree, &["rebase", "--continue"]) {
        Ok(_) => Ok(RebaseOutcome::Rebased),
        Err(error) => match unmerged_files(worktree) {
            Ok(files) if !files.is_empty() => Ok(RebaseOutcome::Conflicted(files)),
            _ => Err(error),
        },
    }
}

/// lean 收口面：`git rebase --abort`——worktree 恢复重放前干净态；Err 上抛由
/// 链侧附注呈现（不静默吞二次失败）。
pub(crate) fn rebase_abort(worktree: &Path) -> Result<(), String> {
    git(worktree, &["rebase", "--abort"]).map(|_| ())
}

/// 主仓当前分支名：`branch --show-current`（空输出 = detached HEAD → 显式
/// Err 引导——重放与快进目标 = HEAD 所在分支，detached 无从合入）。
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
