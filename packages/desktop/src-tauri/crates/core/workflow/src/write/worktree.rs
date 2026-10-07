//! 写面 → vcs 执行的进程内缝（port 属消费者，流量词汇驻 core/workflow；
//! checks 先例同构——port trait 留 core、进程执行落 infra）。六方法细粒度
//! 使 create 编排（顺序 / 补偿 / 警告收口）留在 core 单点、vcs 只做进程执行
//!（design D1）。sync 签名零 tokio（写面 sync 纪律）；测试以进程内脚本化
//! 假件实现本 trait。

use std::path::Path;

/// git 探测产出：基线（主仓 HEAD）+ 脏仓位（`git status --porcelain` 非空）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoProbe {
    /// 主仓 HEAD commit sha（worktree 建域基线 / `base_commit` 记录源）
    pub head: String,
    /// 主仓是否含未提交改动（脏仓警告触发位；基线恒 HEAD 不受影响）
    pub dirty: bool,
}

/// 依赖引导安装产出：退出态 + 输出尾部摘要（≤300 字，vcs 铸出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRun {
    /// 安装命令退出态（true = 退出 0）
    pub success: bool,
    /// stdout + stderr 尾部摘要（失败语境所在）
    pub summary: String,
}

/// git 工作面 port 缝：建域（`add_worktree`）/ 探测（`probe` / `branch_exists`）
/// / 补偿（`remove_worktree` / `delete_branch`）/ 依赖引导（`run_install`）。
/// 实现驻 infra/vcs（`ProcessWorktree`，组合根装配注入）；`Err` 面为带引导
/// 文案的 String（调用方直接呈现）。
pub trait WorktreePort: Send + Sync {
    /// git 可用 + git 仓 + HEAD + 脏仓单点探测（create 前置⑤；git 不可发现 /
    /// 非 git 仓 / 空仓无 HEAD 均显式 `Err` 引导——MUST NOT 静默回退主 root
    /// 创建）。
    fn probe(&self, main_root: &Path) -> Result<RepoProbe, String>;

    /// branch 在案校验（create 前置⑥：`change/<name>` 冲突；miss = `Ok(false)`
    /// 非 Err）。
    fn branch_exists(&self, main_root: &Path, branch: &str) -> Result<bool, String>;

    /// worktree 建域：HEAD 基线检出 + 铸 `branch` 分支（主仓未提交改动不进入
    /// worktree）。
    fn add_worktree(&self, main_root: &Path, worktree: &Path, branch: &str) -> Result<(), String>;

    /// worktree 补偿移除（--force；create 树写出失败补偿链第一环）。
    fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>;

    /// 分支补偿删除（-D；不触碰主仓工作区）。
    fn delete_branch(&self, main_root: &Path, branch: &str) -> Result<(), String>;

    /// 依赖引导安装执行体：cwd = worktree、环境全继承、超时到点 kill（超时 /
    /// 拉起失败统一 `Err` 面；非零退出为 `Ok(InstallRun { success: false })`）。
    fn run_install(&self, worktree: &Path, command: &str) -> Result<InstallRun, String>;
}
