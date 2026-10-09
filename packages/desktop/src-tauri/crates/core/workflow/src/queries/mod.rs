//! 查询模块根：列表与详情聚合（纯读，无任何写入路径，无指令概念）。

pub mod detail;
pub mod explore;
pub mod list;

pub use detail::{change_detail, AttemptRecord, ChangeDetail, PhaseEntry};
pub use explore::{read_explore, scan_explores, ExploreDoc, ExploreScanEntry};
pub use list::{list_changes, ArchiveGroup, ChangeList, ChangeSource, ChangeSummary};

use std::path::{Path, PathBuf};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use foundation::layout::{resolve, Layout};

/// change 目录在 workspace 中的定位结果。
#[derive(Debug, Clone)]
pub struct ChangeLocation {
    pub dir: PathBuf,
    pub source: ChangeSource,
}

/// 按名称定位 change 目录
pub fn locate_change(
    layout: &Layout,
    worktree: Option<&str>,
    name: &str,
) -> Option<ChangeLocation> {
    if !is_single_component_name(name) {
        return None;
    }
    if let Some(worktree_root) = worktree {
        let worktree_dir = resolve(Path::new(worktree_root)).changes_root.join(name);
        if worktree_dir.is_dir() {
            return Some(ChangeLocation {
                dir: worktree_dir,
                source: ChangeSource::Active,
            });
        }
    }
    let active = layout.changes_root.join(name);
    if active.is_dir() {
        return Some(ChangeLocation {
            dir: active,
            source: ChangeSource::Active,
        });
    }
    let archived = layout.archive_root.join(name);
    if archived.is_dir() {
        return Some(ChangeLocation {
            dir: archived,
            source: ChangeSource::Archive,
        });
    }
    if let Some(prefixed) = locate_prefixed_archive_dir(layout, name) {
        return Some(ChangeLocation {
            dir: prefixed,
            source: ChangeSource::Archive,
        });
    }
    None
}

/// archive 树日期前缀后缀匹配：目录名 = `YYYY-MM-DD-<name>` 且前缀为合法
/// 日期形态。目录缺省（无归档树）返回 `None`。
fn locate_prefixed_archive_dir(layout: &Layout, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(&layout.archive_root).ok()?;
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().into_owned();
        let suffix_hit = list::archive_prefix_date(&dir_name)
            .and_then(|date| dir_name.strip_prefix(&date))
            .and_then(|rest| rest.strip_prefix('-'));
        if suffix_hit == Some(name) {
            return Some(layout.archive_root.join(dir_name));
        }
    }
    None
}

/// 单分量名口径（非空、非 `.` / `..`、不含 `/` `\` `:`）：change 目录名与
/// explore 笔记 stem 共用同一校验单点，防两处漂移。
pub(crate) fn is_single_component_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
}

/// 时间戳出线转换单点：UTC unix 毫秒 → ISO 8601 串（Rfc3339；格式失败降级
/// 空串，与既往 `lenient_timestamp::serialize` 同式）。
pub(crate) fn iso_from_millis(millis: i64) -> String {
    let secs = if millis < 0 { 0 } else { millis / 1000 };
    let millis_part = if millis < 0 { 0 } else { millis % 1000 };
    let timestamp = OffsetDateTime::from_unix_timestamp(secs).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    timestamp
        .replace_millisecond(millis_part as u16)
        .unwrap_or(timestamp)
        .format(&Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod list_test;

#[cfg(test)]
mod mod_test;

#[cfg(test)]
mod detail_test;

#[cfg(test)]
mod explore_test;
