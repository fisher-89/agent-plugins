//! 查询模块根：列表与详情聚合（纯读，无任何写入路径，无指令概念）。

pub mod detail;
pub mod list;

pub use detail::{change_detail, AttemptRecord, ChangeDetail, PhaseEntry};
pub use list::{list_changes, ArchiveGroup, ChangeList, ChangeSource, ChangeSummary};

use std::path::PathBuf;

use foundation::layout::Layout;

/// change 目录在 workspace 中的定位结果。
#[derive(Debug, Clone)]
pub struct ChangeLocation {
    pub dir: PathBuf,
    pub source: ChangeSource,
}

/// 按名称在 active 与 archive 两棵树中定位 change 目录；
/// 名称必须是单个普通目录名（拒绝路径穿越），未知名称返回 `None`。
pub fn locate_change(layout: &Layout, name: &str) -> Option<ChangeLocation> {
    if !is_change_name(name) {
        return None;
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
    None
}

fn is_change_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
}

#[cfg(test)]
mod list_test;

#[cfg(test)]
mod detail_test;
