//! change 列表查询：**db 单源全量**（唯一基准数据源，零磁盘扫描触点）。
//!
//! 条目集合 = workspace 库 `ChangeRecord` 全量：active / archive 两树目录发现
//! 与「db ∪ 磁盘去重并集」语义整体退役，无 db 记录的存量 CLI change 零发现
//! （不入列、不报错、不建档、零磁盘读取）；目录缺席（无归档树等）不影响结果，
//! 无降级分支。月分组时间以 `archived_at` 为唯一权威（磁盘目录名日期前缀不
//! 参与分组），`archived_at` 缺失归「未知时间」组置尾。`name` 恒裸名出线
//! （归档 change 的日期前缀仅存在于磁盘目录名），身份寻址恒以 `id`。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::queries::detail::ActivePhase;
use crate::state::{ChangeStateRecord, ChangeStateStore, ChangeStatus};
use crate::write::utc_date;

/// change 来源：进行中 / 已归档（自 db status 派生）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChangeSource {
    Active,
    Archive,
}

/// 列表条目摘要。`id` 为身份锚（行键 / 前端路由 / 一切后续寻址），`name` 恒
/// 裸名；条目集合 db 单源，状态面恒在场（`Option` 形态保留——非档案缺位
/// 语义）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSummary {
    /// change 身份锚（uuid 形态）
    pub id: String,
    /// change 名（恒裸名——归档日期前缀仅存在于磁盘目录名，MUST NOT 出线）
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

/// change 列表（db 单源）：`list_change_records` 全量逐条投影，状态归组以
/// `record.status` 权威（带 worktree 记录的 active change 在 merge 前主仓两树
/// 未命中，照常归进行中组）。db 半边故障降级为空列表（签名无错误面，读命令
/// 面空结果语义）。
pub fn list_changes(store: &dyn ChangeStateStore) -> ChangeList {
    let records = store.list_change_records().unwrap_or_default();
    let mut active: Vec<ChangeSummary> = Vec::new();
    let mut archive: Vec<(ChangeSummary, Option<String>)> = Vec::new();
    for record in &records {
        let entry = db_entry(record);
        match entry.source {
            ChangeSource::Active => active.push(entry),
            ChangeSource::Archive => {
                let month = archive_month(record);
                archive.push((entry, month));
            }
        }
    }
    active.sort_by(|a, b| a.name.cmp(&b.name));
    ChangeList {
        active,
        archive_groups: group_archive(archive),
    }
}

/// db 建档条目：归组以 `record.status` 权威（db status 是桌面写面唯一事实）；
/// name 恒裸名、created 取 `created_at`（磁盘目录名不参与出线）。
fn db_entry(record: &ChangeStateRecord) -> ChangeSummary {
    let source = match record.status {
        ChangeStatus::Active => ChangeSource::Active,
        ChangeStatus::Archived => ChangeSource::Archive,
    };
    ChangeSummary {
        id: record.id.clone(),
        name: record.name.clone(),
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

/// 日期串截月（`YYYY-MM-DD` → `YYYY-MM`）：归档分组的粒度单点。
fn month_of(date: &str) -> String {
    date[..7].to_owned()
}

/// db 归档条目的分组月份：`archived_at` 唯一权威（磁盘目录名日期前缀不参与
/// 分组）；缺失归 `None`（未知时间组，不丢弃）。
fn archive_month(record: &ChangeStateRecord) -> Option<String> {
    record.archived_at.map(utc_date).map(|date| month_of(&date))
}

/// 目录名日期前缀 `YYYY-MM-DD-` → "YYYY-MM-DD"；无前缀或形状不符返回 `None`
/// （archive 树后缀扫描判定点共用本单点：`locate_change` 与写面 `archive`）。
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
