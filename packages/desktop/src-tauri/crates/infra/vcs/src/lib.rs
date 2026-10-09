use std::path::{Path, PathBuf};

use orchestration::port::{RebaseOutcome, WorktreeSnapshot};
use orchestration::ArchiveVcsPort;
use workflow::write::WorktreePort;

mod bootstrap;
mod git;

#[cfg(test)]
mod bootstrap_test;
#[cfg(test)]
mod git_test;
#[cfg(test)]
mod lib_test;

/// PATH 环境变量修改串行化（进程全局变量边界；沿 checks / agent crate 的
/// TEST_PATH_LOCK 先例——「git 不可发现」注入窗口与真实 git 真件用例互斥）。
#[cfg(test)]
pub(crate) static TEST_PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// PATH 锁获取（中毒容错——持锁用例 panic 不级联炸掉同二进制内其余用例；
/// bindings mod_test 先例同式）。
#[cfg(test)]
pub(crate) fn test_path_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_PATH_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// worktrees 子树目录名（数据根直下，每 workspace 恒一段子树）。
pub const WORKTREES_DIR_NAME: &str = "worktrees";

/// worktree 落位派生单点：`data_root/worktrees/{身份段}/<change>`——身份段
/// 消费 foundation 单点（与 workspace db 文件名同源：同根恒同名、异根必不同
/// 名）。canonical 锚口径同身份段（命令入参 canonical root 契约，纯字符串
/// 零路径 IO）；change 名不做任何清洗 / 截断（清洗权威在 create 前置）。
pub fn worktree_dir(data_root: &Path, workspace_root: &str, change: &str) -> PathBuf {
    data_root
        .join(WORKTREES_DIR_NAME)
        .join(foundation::identity::workspace_identity_segment(
            workspace_root,
        ))
        .join(change)
}

/// git 工作面进程执行器（无状态，组合根按需构造）。
pub struct ProcessWorktree;

impl ProcessWorktree {
    /// 构造（组合根装配）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessWorktree {
    fn default() -> Self {
        Self::new()
    }
}

impl WorktreePort for ProcessWorktree {
    fn probe(&self, main_root: &Path) -> Result<workflow::write::RepoProbe, String> {
        git::probe(main_root)
    }

    fn branch_exists(&self, main_root: &Path, branch: &str) -> Result<bool, String> {
        git::branch_exists(main_root, branch)
    }

    fn add_worktree(&self, main_root: &Path, worktree: &Path, branch: &str) -> Result<(), String> {
        git::add_worktree(main_root, worktree, branch)
    }

    fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String> {
        git::remove_worktree(main_root, worktree)
    }

    fn delete_branch(&self, main_root: &Path, branch: &str) -> Result<(), String> {
        git::delete_branch(main_root, branch)
    }

    fn run_install(
        &self,
        worktree: &Path,
        command: &str,
    ) -> Result<workflow::write::InstallRun, String> {
        bootstrap::run_install(worktree, command)
    }
}

/// git 归档工作面进程执行器（无状态，组合根按需构造——归档链命令组装配）。
/// 方法族随 archive-rebase-merge 演进：`rebase_branch` worktree 内重放（冲突
/// 态保留，返回 `RebaseOutcome`，不自动 abort）、`ff_merge` 主仓纯快进、
/// `worktree_snapshot` 后验快照、`rebase_continue` 链代续走、`rebase_abort`
/// lean 收口——全部委托 git 子命令族。
pub struct ProcessArchiveVcs;

impl ProcessArchiveVcs {
    /// 构造（组合根装配）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessArchiveVcs {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveVcsPort for ProcessArchiveVcs {
    fn dirty(&self, root: &Path, paths: &[&str]) -> bool {
        git::dirty(root, paths)
    }

    fn commit_all(&self, worktree: &Path, message: &str) -> Result<(), String> {
        git::commit_all(worktree, message)
    }

    fn branch_merged(&self, main_root: &Path, branch: &str) -> Result<bool, String> {
        git::branch_merged(main_root, branch)
    }

    fn rebase_branch(&self, worktree: &Path, onto: &str) -> Result<RebaseOutcome, String> {
        git::rebase_branch(worktree, onto)
    }

    fn ff_merge(&self, main_root: &Path, branch: &str) -> Result<(), String> {
        git::ff_merge(main_root, branch)
    }

    fn worktree_snapshot(&self, root: &Path) -> Result<WorktreeSnapshot, String> {
        git::worktree_snapshot(root)
    }

    fn rebase_continue(&self, worktree: &Path) -> Result<RebaseOutcome, String> {
        git::rebase_continue(worktree)
    }

    fn rebase_abort(&self, worktree: &Path) -> Result<(), String> {
        git::rebase_abort(worktree)
    }

    fn current_branch(&self, main_root: &Path) -> Result<String, String> {
        git::current_branch(main_root)
    }

    fn commit_paths(&self, main_root: &Path, paths: &[&str], message: &str) -> Result<(), String> {
        git::commit_paths(main_root, paths, message)
    }
}
