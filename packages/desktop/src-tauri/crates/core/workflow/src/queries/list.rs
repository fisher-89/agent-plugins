//! change 列表查询：db 记录 ∪ 磁盘目录去重并集 + 按月分组。
//!
//! 发现语义（proposal 拍板）：目录存在即 change——workspace 库 ChangeRecord
//! 全量 + changes / archive 两棵目录树内的目录，同名条目以 db 为准合并一条。
//! 读时以磁盘事实归组（D7）：条目来源树以磁盘目录存在性为准，查询路径不回
//! 写 db（纯读纪律）。db 缺记录条目（存量 CLI change）以文档形态入列（无状
//! 态面）。归档按月分组：db 取 `archived_at`、磁盘回退目录名日期前缀、无前
//! 缀入「未知时间」组置尾。

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::queries::detail::ActivePhase;
use crate::state::{ChangeStateRecord, ChangeStateStore, ChangeStatus};
use crate::write::utc_date;
use foundation::layout::Layout;

/// change 来源：进行中 / 已归档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChangeSource {
    Active,
    Archive,
}

/// 列表条目摘要。`name` 为磁盘目录名（归档条目含日期前缀；db 条目按磁盘事
/// 实取位，目录缺失回退建档名）。db 建档条目携状态面（`status` /
/// `active_phase`），文档形态条目两值为 `null`。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSummary {
    pub name: String,
    pub source: ChangeSource,
    pub status: Option<ChangeStatus>,
    pub active_phase: Option<ActivePhase>,
    pub created: Option<String>,
}

/// archive 月份分组；`month` 为 `None` 即"未知时间"组，固定排组序列尾。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveGroup {
    pub month: Option<String>,
    pub changes: Vec<ChangeSummary>,
}

/// change 列表：active 全量 + archive 按月分组。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeList {
    pub active: Vec<ChangeSummary>,
    pub archive_groups: Vec<ArchiveGroup>,
}

/// 扫描 change 列表：db 记录 ∪ 磁盘目录去重并集（同名以 db 为准）。
/// 目录缺失（如无归档树）返回空结果而非报错；db 半边故障降级为仅磁盘半边
///（签名无错误面，与 fs 扫描缺席同哲学）。
pub fn list_changes(layout: &Layout, store: &dyn ChangeStateStore) -> ChangeList {
    let records = store.list_change_records().unwrap_or_default();
    let db_names: HashSet<String> = records.iter().map(|record| record.name.clone()).collect();

    let mut active: Vec<ChangeSummary> = Vec::new();
    let mut archive: Vec<(ChangeSummary, Option<String>)> = Vec::new();
    for record in &records {
        let entry = db_entry(layout, record);
        match entry.source {
            ChangeSource::Active => active.push(entry),
            ChangeSource::Archive => {
                let month = archive_month(record, &entry.name);
                archive.push((entry, month));
            }
        }
    }

    // archive 目录名（layout 取得）：active 侧扫描跳过之，避免把 archive 本
    // 身误当作名为该目录名的 active change
    let archive_dir_name = layout
        .archive_root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    for name in scan_dir_names(&layout.changes_root, archive_dir_name.as_deref()) {
        if db_names.contains(&name) {
            continue; // 同名共存以 db 为准
        }
        active.push(ChangeSummary {
            name,
            source: ChangeSource::Active,
            status: None,
            active_phase: None,
            created: None,
        });
    }
    for (dir_name, base, prefix) in scan_archive_dirs(&layout.archive_root) {
        if db_names.contains(&base) {
            continue; // 同名共存以 db 为准
        }
        archive.push((
            ChangeSummary {
                name: dir_name,
                source: ChangeSource::Archive,
                status: None,
                active_phase: None,
                created: prefix.clone(),
            },
            prefix,
        ));
    }

    active.sort_by(|a, b| a.name.cmp(&b.name));
    ChangeList {
        active,
        archive_groups: group_archive(archive),
    }
}

/// db 建档条目：来源树与目录名按磁盘事实取位（D7 读时归组，不回写 db）。
fn db_entry(layout: &Layout, record: &ChangeStateRecord) -> ChangeSummary {
    let active_dir = layout.changes_root.join(&record.name);
    let dir_name = if active_dir.is_dir() {
        record.name.clone()
    } else {
        locate_prefixed_archive_name(layout, &record.name).unwrap_or_else(|| record.name.clone())
    };
    let source = if active_dir.is_dir() {
        ChangeSource::Active
    } else {
        ChangeSource::Archive
    };
    ChangeSummary {
        name: dir_name,
        source,
        status: Some(record.status),
        active_phase: record.active_phase.as_ref().map(|active| ActivePhase {
            phase: active.phase.clone(),
            attempt: active.attempt,
            start_at: Some(super::iso_from_millis(active.start_at)),
        }),
        created: Some(utc_date(record.created_at)),
    }
}

/// archive 树日期前缀后缀匹配（db 名 → 磁盘目录名；目录缺省 None）。
fn locate_prefixed_archive_name(layout: &Layout, name: &str) -> Option<String> {
    let entries = fs::read_dir(&layout.archive_root).ok()?;
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().into_owned();
        let suffix_hit = archive_prefix_date(&dir_name)
            .and_then(|date| dir_name.strip_prefix(&date))
            .and_then(|rest| rest.strip_prefix('-'));
        if suffix_hit == Some(name) {
            return Some(dir_name);
        }
    }
    None
}

/// db 归档条目的分组月份：`archived_at` 优先，磁盘目录名日期前缀回退，
/// 皆缺为 `None`（未知时间组）。
fn archive_month(record: &ChangeStateRecord, entry_name: &str) -> Option<String> {
    record
        .archived_at
        .map(utc_date)
        .or_else(|| archive_prefix_date(entry_name))
        .map(|date| date[..7].to_owned())
}

/// active 树目录名枚举（跳过点前缀项与指定目录名；目录缺失返回空）。
fn scan_dir_names(root: &Path, skip_dir_name: Option<&str>) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || Some(name.as_str()) == skip_dir_name {
            continue;
        }
        names.push(name);
    }
    names
}

/// archive 树目录枚举：(目录名, 基名（剥日期前缀）, 日期前缀)。
fn scan_archive_dirs(root: &Path) -> Vec<(String, String, Option<String>)> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let prefix = archive_prefix_date(&name);
        let base = prefix
            .as_ref()
            .and_then(|date| name.strip_prefix(date))
            .and_then(|rest| rest.strip_prefix('-'))
            .map(str::to_owned)
            .unwrap_or_else(|| name.clone());
        dirs.push((name, base, prefix));
    }
    dirs
}

/// 目录名日期前缀 `YYYY-MM-DD-` → "YYYY-MM-DD"；无前缀或形状不符返回 `None`。
pub(crate) fn archive_prefix_date(name: &str) -> Option<String> {
    prefix_date(name)
}

fn prefix_date(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    if bytes.len() < 11 {
        return None;
    }
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    let dashes = [4, 7, 10].iter().all(|&i| bytes[i] == b'-');
    if digits(0..4) && digits(5..7) && digits(8..10) && dashes {
        Some(name[..10].to_string())
    } else {
        None
    }
}

/// 月份分组：有月份者折叠（新月份在前），无月份入"未知时间"组置尾。
fn group_archive(changes: Vec<(ChangeSummary, Option<String>)>) -> Vec<ArchiveGroup> {
    let mut known: BTreeMap<String, Vec<ChangeSummary>> = BTreeMap::new();
    let mut unknown: Vec<ChangeSummary> = Vec::new();
    for (summary, month) in changes {
        match month {
            Some(month) => known.entry(month).or_default().push(summary),
            None => unknown.push(summary),
        }
    }
    let mut groups: Vec<ArchiveGroup> = known
        .into_iter()
        .rev()
        .map(|(month, mut changes)| {
            changes.sort_by(|a, b| b.name.cmp(&a.name));
            ArchiveGroup {
                month: Some(month),
                changes,
            }
        })
        .collect();
    if !unknown.is_empty() {
        unknown.sort_by(|a, b| b.name.cmp(&a.name));
        groups.push(ArchiveGroup {
            month: None,
            changes: unknown,
        });
    }
    groups
}
