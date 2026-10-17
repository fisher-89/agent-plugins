use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use orchestration::port::{IndexEntry, RebaseOutcome, StatusEntry, WorktreeSnapshot};
use workflow::write::RepoProbe;

/// 一次 git 子命令执行（stdout 捕获；stdin 关闭）
pub(crate) fn git(main_root: &Path, args: &[&str]) -> Result<String, String> {
    git_impl(main_root, args, None)
}

/// stdin 载荷版 git 子命令执行：海量 pathspec 清单经
/// `--pathspec-from-file=- --pathspec-file-nul` 入线（git ≥ 2.25），规避
/// argv 展开——Windows CreateProcess 命令行 32767 上限在海量 untracked
/// 清单（test-execution 遗留 llvm-cov 产物 500+ 路径 ≈ 70KB）下 spawn 直接
/// os error 206（「文件名或扩展名太长」）。
pub(crate) fn git_stdin(
    main_root: &Path,
    args: &[&str],
    stdin_payload: &[u8],
) -> Result<String, String> {
    git_impl(main_root, args, Some(stdin_payload))
}

fn git_impl(
    main_root: &Path,
    args: &[&str],
    stdin_payload: Option<&[u8]>,
) -> Result<String, String> {
    let mut command = exec::SystemCommand::bare("git").into_std();
    command
        .arg("-C")
        .arg(main_root)
        .args(args)
        .env("GIT_EDITOR", "true")
        .stdin(if stdin_payload.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        format!(
            "git 不可用（PATH 未发现 git）: {error}；change worktree 建域依赖 git，\
             请安装 git 并确认 PATH 可达后重试"
        )
    })?;
    if let Some(payload) = stdin_payload {
        // 尽力写 + drop 关管道（EOF 信号）。写错吞——git 提前退出（非法
        // pathspec 等）时管道断，退出码 / stderr 面已足；add 读 stdin 期间
        // stdout / stderr 输出近零，无管道互锁
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(payload);
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("git {} 收敛失败: {error}", args.join(" ")))?;
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
/// 回收；目录登记与磁盘目录一并清）。create 补偿面之外由归档相位 Cleanup 步
/// 经 `ProcessArchiveVcs` 复用本单点（`--force` 带走 bootstrap 产物；零逻辑
/// 改动，`Err` 面语境由消费侧组装）。
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

// ---------------------------------------------------------------------------
// 基线恢复（initialize 相位 Clean 步 spawn 单点；经 ArchiveVcsPort 缝消费——
// 排除面组装驻 walker 调用方，infra 无状态）
// ---------------------------------------------------------------------------

/// worktree 基线恢复：`reset --hard` → `clean -fd` + 逐项 `-e <pattern>` 排除
///（**不带 `-x`**——尊重 .gitignore，依赖树常态已免疫；`-e` 双保险覆盖无 ignore
/// 规则仓；`openspec/` 工件树未跟踪面显式保护）。先 reset 后 clean = tracked
/// 恢复 + untracked 清除的规范序。成功返回恢复摘要串；失败 `Err` 沿既有
/// `git()` 语境（命令串 + 退出码 + stderr）。
pub(crate) fn restore_baseline(worktree: &Path, exclusions: &[&str]) -> Result<String, String> {
    git(worktree, &["reset", "--hard"])?;
    let mut clean_args: Vec<&str> = vec!["clean", "-fd"];
    for pattern in exclusions {
        clean_args.push("-e");
        clean_args.push(pattern);
    }
    git(worktree, &clean_args)?;
    Ok(format!(
        "已恢复基线（reset --hard + clean -fd，排除面 {0} 项）",
        exclusions.len()
    ))
}

// ---------------------------------------------------------------------------
// 变更文件清单 / 补丁（突变范围唯一入口——`change_edit_set` 是全仓取 diff /
// 列清单的单点封装，`change_edit_patch` 与其同居；checks-runtime 依赖面零 vcs，
// 清单与补丁经 `ArchiveVcsPort` 缝注入 MutationRunner）
// ---------------------------------------------------------------------------

/// 变更文件清单唯一入口：`git diff <base> --name-only -z` ∪ `git ls-files
/// --others --exclude-standard -z`（untracked 并入——新建源文件不 add 也入清
/// 单），排序去重、POSIX 归一后返回。`base = "HEAD"` 时即本 change 编辑集
/// （worktree 语义）。git 失败显式 `Err`——MUST NOT 静默降级空清单（空清单会
/// 误走 diff 空跳过）。取数链路内幂等确保 mutation 调试产物忽略规则（见
/// [`ensure_mutation_artifact_exclude`]——发生在任何段临时 config 写盘前）。
pub(crate) fn change_edit_set(root: &Path, base: &str) -> Result<Vec<String>, String> {
    validate_base(base)?;
    let tracked = git(root, &["diff", base, "--name-only", "-z"])?;
    let untracked = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    ensure_mutation_artifact_exclude(root)?;
    let mut files = std::collections::BTreeSet::new();
    for raw in [tracked, untracked] {
        for entry in raw.split('\0').filter(|entry| !entry.is_empty()) {
            files.insert(entry.replace('\\', "/"));
        }
    }
    Ok(files.into_iter().collect())
}

/// mutation 调试产物忽略规则（`.git/info/exclude` 追加面；basename 语义任意
/// 深度命中——`_stryker-tmp/` 目录段 / `mutation-stryker-seg-*.conf.json`
/// 文件名）。一次 ensure 三覆盖：`commit_all` 的 `add -A`、`dirty` 的
/// `status --porcelain`、本清单的 `ls-files --others --exclude-standard`
/// 全自动剔除——失败段调试产物留盘复现但 MUST NOT 进归档提交。
const MUTATION_EXCLUDE_PATTERNS: [&str; 2] =
    ["_stryker-tmp/", "mutation-stryker-seg-*.conf.json"];

/// 幂等确保 mutation 调试产物忽略规则写入 workspace `.git/info/exclude`：
/// 已全在 → 零写盘；缺任一 → 补齐（`.git/info` 目录缺失则创建；行终止符跟随
/// 既有内容——CRLF 仓不写裸 LF）。配套撤出 staged 面残留（修复前 `git add -N`
/// 的 intent-to-add——tracked 面 ignore 盖不住，`add -A` 会真正收录；文件留
/// 盘只撤索引）。写盘 / git 失败显式 `Err`（保证优先：排除写不进就别让
/// mutation 继续跑）。经 `git rev-parse --git-path` 解析——linked worktree 返回
/// 主仓 common-dir 绝对路径（主仓 + 全部 worktree 共享一份）。
pub(crate) fn ensure_mutation_artifact_exclude(root: &Path) -> Result<(), String> {
    let exclude_path = resolve_info_exclude(root)?;
    let existing = std::fs::read_to_string(&exclude_path).unwrap_or_default();
    if !MUTATION_EXCLUDE_PATTERNS
        .iter()
        .all(|pattern| exclude_line_present(&existing, pattern))
    {
        if let Some(parent) = exclude_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "mutation 调试产物忽略目录创建失败 {}: {error}",
                    parent.display()
                )
            })?;
        }
        let newline = if existing.contains("\r\n") { "\r\n" } else { "\n" };
        let mut updated = existing.clone();
        for pattern in MUTATION_EXCLUDE_PATTERNS {
            if !exclude_line_present(&existing, pattern) {
                if !updated.is_empty() && !updated.ends_with(newline) {
                    updated.push_str(newline);
                }
                updated.push_str(pattern);
                updated.push_str(newline);
            }
        }
        std::fs::write(&exclude_path, updated).map_err(|error| {
            format!(
                "mutation 调试产物忽略规则写盘失败 {}: {error}",
                exclude_path.display()
            )
        })?;
    }
    reset_staged_mutation_artifacts(root)
}

/// `.git/info/exclude` 路径解析（`git rev-parse --git-path`——相对输出对
/// root 绝对化；linked worktree 返回主仓绝对路径直接可用）。
fn resolve_info_exclude(root: &Path) -> Result<PathBuf, String> {
    let raw = git(root, &["rev-parse", "--git-path", "info/exclude"])?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(
            "mutation 调试产物忽略规则定位失败：git-path info/exclude 输出为空".to_owned(),
        );
    }
    let path = Path::new(trimmed);
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    })
}

/// 行级存在判定（剥 `\r` 后整行相等——避免子串误命中注释行）。
fn exclude_line_present(content: &str, pattern: &str) -> bool {
    content
        .split('\n')
        .any(|line| line.trim_end_matches('\r') == pattern)
}

/// staged 条目是否 mutation 调试产物（路径含 `_stryker-tmp` 段，或 basename
/// 匹配临时 config 命名前缀 + `.conf.json` 后缀）。
fn is_mutation_artifact_entry(entry: &str) -> bool {
    let segments: Vec<&str> = entry.split('/').collect();
    if segments
        .iter()
        .take(segments.len().saturating_sub(1))
        .any(|segment| *segment == "_stryker-tmp")
    {
        return true;
    }
    segments
        .last()
        .copied()
        .unwrap_or(entry)
        .starts_with("mutation-stryker-seg-")
        && entry.ends_with(".conf.json")
}

/// 撤出索引面的 mutation 调试产物（`git reset -q --` 只撤索引、文件留盘——
/// 用户不会主动 stage 这两个自有命名，安全）。探测面用 `ls-files --cached`
///（索引清单）：intent-to-add 条目不出现在 `diff --cached`（实测），但确在
/// 索引里、`add -A` / `commit` 会真正收录——升级残留场景靠本面撤出。
fn reset_staged_mutation_artifacts(root: &Path) -> Result<(), String> {
    let staged = git(root, &["ls-files", "--cached", "-z"])?;
    let matches: Vec<&str> = staged
        .split('\0')
        .filter(|entry| !entry.is_empty() && is_mutation_artifact_entry(entry))
        .collect();
    if matches.is_empty() {
        return Ok(());
    }
    let mut args: Vec<&str> = vec!["reset", "-q", "--"];
    args.extend(matches.iter().copied());
    git(root, &args).map(|_| ())
}

/// `--in-diff` 补丁唯一入口：untracked 面先行 `git add -N`（intent-to-add——不改
/// 文件内容、不改 worktree 语义：worktree 为 change 私有执行锚，归档 `commit_all`
/// 本就 `add -A` 收编；重复执行幂等，不回退标记）再 `git diff <base>` 输出
/// unified diff 文本。与 [`change_edit_set`] 同居 vcs 单点，checks 侧零 git spawn。
pub(crate) fn change_edit_patch(root: &Path, base: &str) -> Result<String, String> {
    validate_base(base)?;
    // 仅对 untracked 清单面做 intent-to-add（与 change_edit_set 的 untracked 口径
    // 同源同集——gitignore 面不入）
    let untracked = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    if untracked.split('\0').any(|entry| !entry.is_empty()) {
        // 清单经 stdin NUL 入线（ls-files -z 产物原样回喂）：argv 展开面在
        // 海量 untracked（llvm-cov 产物等 500+ 路径）下超 Windows
        // CreateProcess 命令行 32767 上限——spawn 直接 os error 206
        git_stdin(
            root,
            &["add", "-N", "--pathspec-from-file=-", "--pathspec-file-nul"],
            untracked.as_bytes(),
        )?;
    }
    git(root, &["diff", base])
}

/// base 入参前置校验（空白 base 显式 Err——`git diff ""` 的歧义语境不可辨，
/// 显式拒绝优于 git 半途报错；含换行 / 超长等非法 rev 交 git 失败显式 Err）。
fn validate_base(base: &str) -> Result<(), String> {
    if base.trim().is_empty() {
        return Err("变更文件清单取数失败：base 为空（期望 rev 形态如 \"HEAD\"）".to_owned());
    }
    Ok(())
}
