//! 归档双写操作（design D6）：按 change **id** 读记录（未建档显式拒绝）→
//! **先目录改名**（active → archive 树 `YYYY-MM-DD-<name>`，目标已存在先查
//! 拒绝）**后 db 翻转**（status=archived + archived_at）。改名成功而翻转失败
//! → Err 呈现半完成态，重试经「archive 树定位命中 + db 仍 active」续半边分支
//! 仅补 db 翻转（不重复改名）。磁盘面目录名恒由 `record.name` 供给（id →
//! 记录 → name 分辨率单点）——`ChangeRecord.id` 与 `name` 均不随目录改名变。
//! sync 零 Tauri；磁盘路径全部经 [`Layout`](foundation::layout::Layout) 取得。

use std::fs;

use serde::{Deserialize, Serialize};
use specta::Type;

use foundation::layout::Layout;

use crate::queries::is_single_component_name;
use crate::state::{ChangeStateStore, ChangeStatus};
use crate::write::now_millis;

/// 归档产出（IPC DTO）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveOutcome {
    pub name: String,
    /// 归档日期 UTC `YYYY-MM-DD`（本日；续半边命中带前缀目录时取前缀日期）
    pub archived_date: String,
}

/// 归档双写：id 读记录（未建档显式拒绝）→ active 树定位 → 目标冲突预检 →
/// 目录改名 → status 翻转；续半边分支（archive 树命中 + db 仍 active）仅补
/// 翻转。磁盘面目录名 / 后缀扫描恒由记录的 `name` 供给（id → 记录 → name
/// 分辨率单点，D7）；`id` 与 `name` 均不随改名变。
pub fn archive(
    layout: &Layout,
    store: &dyn ChangeStateStore,
    id: &str,
) -> Result<ArchiveOutcome, String> {
    let record = store
        .get_change(id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("change \"{id}\" 未建档（无 ChangeRecord），不可归档"))?;
    let today = super::create::utc_date(now_millis());
    // 磁盘面名字自记录直供；目录名语义校验施于解析出的 name（id 为不透明串
    // 不做目录名语义校验）
    let name = record.name.as_str();
    if !is_single_component_name(name) {
        return Err(format!("非法 change 名: {name:?}（须为单分量名）"));
    }

    // active 树定位命中 → 常规双写路径
    let active_dir = layout.changes_root.join(name);
    if active_dir.is_dir() {
        let target_name = format!("{today}-{name}");
        let target_dir = layout.archive_root.join(&target_name);
        if target_dir.exists() {
            return Err(format!(
                "归档目标已存在: {}（请手工处置后重试）",
                target_dir.display()
            ));
        }
        fs::rename(&active_dir, &target_dir)
            .map_err(|error| format!("归档目录改名失败: {error}"))?;
        // 改名成功而翻转失败 → Err 呈现半完成态（重试走续半边分支仅补翻转）
        if let Err(error) = store.set_archived(id, now_millis()) {
            return Err(format!(
                "change \"{name}\" 归档目录改名已完成（{} → {}），但 db status 翻转失败: {error}；\
                 请重试归档以补齐 db 半边",
                active_dir.display(),
                target_dir.display()
            ));
        }
        return Ok(ArchiveOutcome {
            name: name.to_owned(),
            archived_date: today,
        });
    }

    // active 树缺失 → archive 树定位（精确名优先——无前缀同名目录命中
    // 续半边，archived_date 回落当日；其次 `YYYY-MM-DD-<name>` 前缀后缀匹配）
    match locate_archived_dir(layout, name) {
        Some((_archived_dir, prefix_date)) => {
            // 续半边分支：仅补 db 翻转（不重复改名）；已翻转则幂等成功
            if record.status != ChangeStatus::Archived {
                store
                    .set_archived(id, now_millis())
                    .map_err(|error| error.to_string())?;
            }
            Ok(ArchiveOutcome {
                name: name.to_owned(),
                archived_date: prefix_date.unwrap_or(today),
            })
        }
        // 未命中 Err 前的 worktree merge-first 前置（design D13）：带 worktree
        // 记录的 change 在 merge 前主仓两树必然未命中——显式引导 merge 优于
        // 泛化「目录未找到」；归档不触碰 worktree / branch（清理为手动边界）
        None if record.worktree.is_some() => Err(format!(
            "change \"{name}\" 的目录未在主仓出现（active / archive 两树未命中）：\
             请先将 worktree 分支 change/{name} merge 回主仓，再发起归档"
        )),
        None => Err(format!(
            "change \"{name}\" 目录未找到（active 与 archive 两棵树均未命中），不可归档"
        )),
    }
}

/// archive 树定位：精确名优先，其次 `YYYY-MM-DD-<name>` 前缀后缀匹配；返回
/// （目录路径，前缀日期）。
fn locate_archived_dir(
    layout: &Layout,
    change: &str,
) -> Option<(std::path::PathBuf, Option<String>)> {
    let exact = layout.archive_root.join(change);
    if exact.is_dir() {
        return Some((exact, None));
    }
    let entries = fs::read_dir(&layout.archive_root).ok()?;
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let date = crate::queries::list::archive_prefix_date(&name);
        let suffix = date
            .as_ref()
            .and_then(|date| name.strip_prefix(date))
            .and_then(|rest| rest.strip_prefix('-'));
        if suffix == Some(change) {
            return Some((layout.archive_root.join(&name), date));
        }
    }
    None
}
