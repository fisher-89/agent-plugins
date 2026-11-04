//! 用户删除面（desktop-change-delete）：按 change **id** 读记录 → 可选清空
//! worktree（目录 + 分支）→ 删主仓产物目录 → 最后级联删库记录。顺序 = 磁盘
//! 先、库最后（沿 `archive` 的「先改名后翻库」哲学）：任一磁盘步失败即停，
//! 库记录仍在、清单照常可见，用户可处置后重试；反之则出现「记录没了、文件还
//! 在」的不可见残留。各步幂等（目录 / 分支缺席跳过），重试链直接收敛。
//!
//! sync 零 Tauri；git 触点全部经 [`WorktreePort`] 缝（进程 spawn 驻
//! infra/vcs），磁盘路径经 [`Layout`](foundation::layout::Layout) 取得。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

use foundation::layout::Layout;

use crate::queries::{is_single_component_name, locate_change};
use crate::state::{ChangeStateStore, ChangeStatus};
use crate::write::create::CHANGE_BRANCH_PREFIX;
use crate::write::worktree::WorktreePort;

/// 删除产出（IPC DTO）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    pub name: String,
    /// 本次是否实际移除了 worktree 目录（未勾选清空 / legacy 记录 / 目录缺席
    /// 均 `false`）
    pub removed_worktree: bool,
    /// 非阻断注记（worktree 保留告知等）；空清单 = 无注记
    pub warnings: Vec<String>,
}

/// 用户删除：id 读记录（未建档显式拒绝）→ 可选 worktree 清理 → 产物目录清理
/// → 级联删库。`clean_worktree` 仅在记录带 worktree 时有语义（legacy / 归档
/// change 忽略该旗标）。
pub fn delete(
    main_root: &Path,
    layout: &Layout,
    store: &dyn ChangeStateStore,
    vcs: &dyn WorktreePort,
    id: &str,
    clean_worktree: bool,
) -> Result<DeleteOutcome, String> {
    let record = store
        .get_change(id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("change \"{id}\" 未建档（无 ChangeRecord），不可删除"))?;
    // 磁盘面名字自记录直供；目录名语义校验施于解析出的 name
    let name = record.name.as_str();
    if !is_single_component_name(name) {
        return Err(format!("非法 change 名: {name:?}（须为单分量名）"));
    }

    let mut warnings = Vec::new();
    let mut removed_worktree = false;

    // 执行段一：worktree 清理（勾选 + 记录带 worktree 才进入；分支清理不随
    // 目录缺席跳过——残留分支会撞 create 前置⑥「branch 已存在」）
    if let Some(worktree) = record.worktree.as_deref() {
        if clean_worktree {
            let worktree_path = Path::new(worktree);
            if worktree_path.is_dir() {
                vcs.remove_worktree(main_root, worktree_path)
                    .map_err(|error| format!("worktree 移除失败（{worktree}）: {error}"))?;
                removed_worktree = true;
            }
            let branch = format!("{CHANGE_BRANCH_PREFIX}{name}");
            if vcs
                .branch_exists(main_root, &branch)
                .map_err(|error| format!("branch 探测失败（{branch}）: {error}"))?
            {
                vcs.delete_branch(main_root, &branch)
                    .map_err(|error| format!("branch 删除失败（{branch}）: {error}"))?;
            }
        } else if Path::new(worktree).is_dir() {
            warnings.push(format!(
                "worktree 目录与分支 {CHANGE_BRANCH_PREFIX}{name} 保留（未勾选清空）: {worktree}"
            ));
        }
    }

    // 执行段二：主仓产物目录清理（候选序 = 本记录归档前缀目录 → active 树 →
    // archive 树 → 首个日期前缀后缀命中）
    if let Some(dir) = main_change_dir(layout, &record.name, record.status, record.archived_at) {
        fs::remove_dir_all(&dir)
            .map_err(|error| format!("产物目录删除失败（{}）: {error}", dir.display()))?;
    }

    // 执行段三：库记录级联（最后一步——磁盘全成后才落库）
    store
        .delete_change_cascade(id)
        .map_err(|error| error.to_string())?;

    Ok(DeleteOutcome {
        name: record.name,
        removed_worktree,
        warnings,
    })
}

/// 主仓产物目录定位：归档记录先按自身 `archived_at` 推导 `YYYY-MM-DD-<name>`
/// （同名归档共存合法，D11——前缀优先命中自己那一份），其余走
/// [`locate_change`] 的 active → archive 精确 → 前缀扫描序。
fn main_change_dir(
    layout: &Layout,
    name: &str,
    status: ChangeStatus,
    archived_at: Option<i64>,
) -> Option<PathBuf> {
    if status == ChangeStatus::Archived {
        if let Some(archived_at) = archived_at {
            let prefixed = layout
                .archive_root
                .join(format!("{}-{name}", crate::write::utc_date(archived_at)));
            if prefixed.is_dir() {
                return Some(prefixed);
            }
        }
    }
    locate_change(layout, None, name).map(|location| location.dir)
}
