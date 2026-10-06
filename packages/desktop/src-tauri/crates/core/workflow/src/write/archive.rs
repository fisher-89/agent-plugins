//! 归档双写操作（design D6）：**先目录改名**（active → archive 树
//! `YYYY-MM-DD-<name>`，目标已存在先查拒绝）**后 db 翻转**（status=archived +
//! archived_at）。改名成功而翻转失败 → Err 呈现半完成态，重试经「archive 树
//! 定位命中 + db 仍 active」续半边分支仅补 db 翻转（不重复改名）。「无建档
//! 目录拒绝」先于一切变更。`ChangeRecord` 主键 name 不随目录改名变。
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

/// 归档双写：db 建档校验 → active 树定位 → 目标冲突预检 → 目录改名 →
/// status 翻转；续半边分支（archive 树命中 + db 仍 active）仅补翻转。
pub fn archive(
    layout: &Layout,
    store: &dyn ChangeStateStore,
    change: &str,
) -> Result<ArchiveOutcome, String> {
    if !is_single_component_name(change) {
        return Err(format!("非法 change 名: {change:?}（须为单分量名）"));
    }
    let record = store
        .get_change(change)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("change \"{change}\" 未建档（无 ChangeRecord），不可归档"))?;
    let today = super::create::utc_date(now_millis());

    // active 树定位命中 → 常规双写路径
    let active_dir = layout.changes_root.join(change);
    if active_dir.is_dir() {
        let target_name = format!("{today}-{change}");
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
        if let Err(error) = store.set_archived(change, now_millis()) {
            return Err(format!(
                "归档目录改名已完成（{} → {}），但 db status 翻转失败: {error}；\
                 请重试归档以补齐 db 半边",
                active_dir.display(),
                target_dir.display()
            ));
        }
        return Ok(ArchiveOutcome {
            name: change.to_owned(),
            archived_date: today,
        });
    }

    // active 树缺失 → archive 树定位（精确名 + 日期前缀后缀匹配；无前缀的
    // 同名目录不识别为续半边对象，走未命中 Err 人工处置）
    match locate_archived_dir(layout, change) {
        Some((_archived_dir, prefix_date)) => {
            // 续半边分支：仅补 db 翻转（不重复改名）；已翻转则幂等成功
            if record.status != ChangeStatus::Archived {
                store
                    .set_archived(change, now_millis())
                    .map_err(|error| error.to_string())?;
            }
            Ok(ArchiveOutcome {
                name: change.to_owned(),
                archived_date: prefix_date.unwrap_or(today),
            })
        }
        None => Err(format!(
            "change \"{change}\" 目录未找到（active 与 archive 两棵树均未命中），不可归档"
        )),
    }
}

/// archive 树定位：精确名优先，其次 `YYYY-MM-DD-<name>` 前缀后缀匹配；返回
/// （目录路径，前缀日期）。
fn locate_archived_dir(layout: &Layout, change: &str) -> Option<(std::path::PathBuf, Option<String>)> {
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
