use std::path::{Path, PathBuf};

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

/// 落位 / 执行根派生产出：**落位**与「workspace root 在仓库内的位置」两值
/// 一次给出（落位恒与 workspace 位置无关，`rel` 只参与执行根派生与准入判据）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreePlacement {
    /// worktree 落位父锚：`<git 根父目录>/.worktrees/<git 根目录名>`
    pub root: PathBuf,
    /// workspace root 相对 git 根的偏移（`/` 分隔、无尾分隔符；git 根
    /// workspace 为 `""`）
    pub rel: String,
}

/// git 工作面 port
pub trait WorktreePort: Send + Sync {
    /// 一次 git 派生「落位父锚 + workspace 相对偏移」：`root` 恒
    /// `<git 根父目录>/.worktrees/<git 根目录名>`（与 workspace 在仓库内的位置
    /// 无关），`rel` 为执行根的挂尾段（`""` = workspace root 即 git 根）。非
    /// git 仓 / 裸仓 / git 不可用均显式 `Err` 引导。
    fn placement(&self, main_root: &Path) -> Result<WorktreePlacement, String>;

    /// `HEAD:<rel>` 在案判据（create 前置第十道）：回答「HEAD 里有没有这个路
    /// 径」，不回答「目录里有什么」。`rel = ""` 恒 `Ok(true)` 且零 spawn；
    /// git 询问失败 `Err`（fail-closed——不把「问不出来」伪装成「不在案」）。
    fn tracked_at_head(&self, main_root: &Path, rel: &str) -> Result<bool, String>;

    /// git 可用 + git 仓 + HEAD + 脏仓单点探测（create 前置⑤；git 不可发现 /
    /// 非 git 仓 / 空仓无 HEAD 均显式 `Err` 引导——MUST NOT 静默回退主 root
    /// 创建）。
    fn probe(&self, main_root: &Path) -> Result<RepoProbe, String>;

    /// branch 在案校验（create 前置⑥：`change/<name>` 冲突；miss = `Ok(false)`
    /// 非 Err）。
    fn branch_exists(&self, main_root: &Path, branch: &str) -> Result<bool, String>;

    /// worktree 建域：HEAD 基线检出 + 铸 `branch` 分支（主仓未提交改动不进入
    /// worktree）。`worktree` 恒为**落位目录**（host），MUST NOT 传执行根——
    /// 执行根是 host 内的一段子目录，建域对象恒为整棵检出。
    fn add_worktree(&self, main_root: &Path, worktree: &Path, branch: &str) -> Result<(), String>;

    /// worktree 补偿移除（--force；create 树写出失败补偿链第一环）。
    fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>;

    /// 分支补偿删除（-D；不触碰主仓工作区）。
    fn delete_branch(&self, main_root: &Path, branch: &str) -> Result<(), String>;

    /// 依赖引导安装执行体。
    fn run_install(&self, cwd: &Path, command: &str) -> Result<InstallRun, String>;
}
