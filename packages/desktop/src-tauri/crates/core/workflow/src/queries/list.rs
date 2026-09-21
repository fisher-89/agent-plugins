//! change 列表查询：全量扫描 + 代际标注 + archive 按月分组。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::Inventory;
use crate::parse::{detect_inventory, parse_workflow_file, WorkflowFileParse, WORKFLOW_FILE_NAME};
use foundation::layout::Layout;

/// change 来源：进行中 / 已归档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeSource {
    Active,
    Archive,
}

/// 列表条目摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSummary {
    pub name: String,
    pub source: ChangeSource,
    pub inventory: Inventory,
    pub created: Option<String>,
    pub unparsable: bool,
}

/// archive 月份分组；`month` 为 `None` 即"未知时间"组，固定排组序列尾。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveGroup {
    pub month: Option<String>,
    pub changes: Vec<ChangeSummary>,
}

/// change 列表：active 全量 + archive 按月分组。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeList {
    pub active: Vec<ChangeSummary>,
    pub archive_groups: Vec<ArchiveGroup>,
}

/// 扫描 changes_root 与 archive_root 全量。
/// 目录缺失返回空结果而非报错。
///
/// archive 目录物理上位于 changes_root 之内：扫描 active 侧时按 layout
/// 取得的 archive 目录名跳过之（不在此硬编码磁盘目录名），避免把
/// archive 本身误当作名为 "archive" 的 active change。
pub fn list_changes(layout: &Layout) -> ChangeList {
    let archive_dir_name = layout
        .archive_root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let active = scan_dir(&layout.changes_root, ChangeSource::Active, archive_dir_name.as_deref());
    let archive = scan_dir(&layout.archive_root, ChangeSource::Archive, None);
    ChangeList {
        active,
        archive_groups: group_archive(archive),
    }
}

fn scan_dir(root: &Path, source: ChangeSource, skip_dir_name: Option<&str>) -> Vec<ChangeSummary> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut summaries = Vec::new();
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || Some(name.as_str()) == skip_dir_name {
            continue;
        }
        let dir = root.join(&name);
        let inventory = detect_inventory(&dir);
        let (created, unparsable) = summarize_change(&dir, &name, source);
        summaries.push(ChangeSummary {
            name,
            source,
            inventory,
            created,
            unparsable,
        });
    }
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    summaries
}

/// created 优先取 workflow.json `created`；缺失或整体损坏时 archive 回退目录名日期前缀。
fn summarize_change(dir: &Path, name: &str, source: ChangeSource) -> (Option<String>, bool) {
    let workflow_path = dir.join(WORKFLOW_FILE_NAME);
    if !workflow_path.is_file() {
        return (created_fallback(None, name, source), false);
    }
    match parse_workflow_file(&workflow_path) {
        WorkflowFileParse::Parsed(workflow) => (created_fallback(workflow.created, name, source), false),
        WorkflowFileParse::Unparsable { .. } => (created_fallback(None, name, source), true),
    }
}

fn created_fallback(created: Option<String>, name: &str, source: ChangeSource) -> Option<String> {
    created.or_else(|| match source {
        ChangeSource::Archive => prefix_date(name),
        ChangeSource::Active => None,
    })
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

/// 月份分组：有日期前缀者折叠到月（新月份在前），无前缀入"未知时间"组置尾。
fn group_archive(changes: Vec<ChangeSummary>) -> Vec<ArchiveGroup> {
    let mut known: BTreeMap<String, Vec<ChangeSummary>> = BTreeMap::new();
    let mut unknown: Vec<ChangeSummary> = Vec::new();
    for summary in changes {
        match prefix_date(&summary.name).map(|date| date[..7].to_string()) {
            Some(month) => known.entry(month).or_default().push(summary),
            None => unknown.push(summary),
        }
    }
    let mut groups: Vec<ArchiveGroup> = known
        .into_iter()
        .rev()
        .map(|(month, changes)| ArchiveGroup {
            month: Some(month),
            changes,
        })
        .collect();
    if !unknown.is_empty() {
        groups.push(ArchiveGroup {
            month: None,
            changes: unknown,
        });
    }
    groups
}
