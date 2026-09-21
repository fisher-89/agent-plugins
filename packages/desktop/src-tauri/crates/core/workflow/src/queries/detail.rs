//! change 详情查询：固定 9 站流水线聚合 + 运行状态 + 产物清单。

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::list::ChangeSource;
use super::locate_change;
use crate::artifacts::{discover_artifacts, ArtifactDescriptor};
use crate::model::{ActivePhase, ChecklistItem, FileLogEntry, InterruptedEntry, Inventory, PhaseLog, Verdict};
use crate::parse::{detect_inventory, parse_workflow_file, WorkflowFileParse, WORKFLOW_FILE_NAME};
use foundation::layout::Layout;

/// 固定 9 站流水线（顺序固定，不依赖 eval 排列）。
pub const PIPELINE_PHASES: [&str; 9] = [
    "proposal",
    "dev-design",
    "test-design",
    "implement",
    "test-gen",
    "test-execution",
    "code-review",
    "acceptance",
    "code-analyze",
];

/// 单次尝试记录：backtrack 目标与原因随条目可查。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRecord {
    pub attempt: Option<u32>,
    pub verdict: Verdict,
    pub report: String,
    pub checklist: Vec<ChecklistItem>,
    pub skipped: bool,
    pub stale: bool,
    pub start_at: Option<OffsetDateTime>,
    pub timestamp: Option<OffsetDateTime>,
    pub backtrack_to: Option<String>,
    pub backtrack_reason: Option<String>,
}

impl From<&PhaseLog> for AttemptRecord {
    fn from(entry: &PhaseLog) -> Self {
        AttemptRecord {
            attempt: entry.attempt,
            verdict: entry.verdict,
            report: entry.report.clone(),
            checklist: entry.checklist.clone(),
            skipped: entry.skipped,
            stale: entry.stale,
            start_at: entry.start_at,
            timestamp: entry.timestamp,
            backtrack_to: entry.backtrack_to.clone(),
            backtrack_reason: entry.backtrack_reason.clone(),
        }
    }
}

/// 一站流水线：该 phase 的全部 attempt 序列。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseEntry {
    pub phase: String,
    pub attempts: Vec<AttemptRecord>,
}

/// change 详情聚合。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDetail {
    pub name: String,
    pub source: ChangeSource,
    pub inventory: Inventory,
    pub created: Option<String>,
    pub unparsable: bool,
    pub pipeline: Vec<PhaseEntry>,
    pub active_phase: Option<ActivePhase>,
    pub interrupted: Vec<InterruptedEntry>,
    /// v1 及更早代际无此字段 → `None`，对应区块降级留空
    pub file_log: Option<Vec<FileLogEntry>>,
    pub artifacts: Vec<ArtifactDescriptor>,
}

/// 聚合单个 change 的详情；未知 change 名返回 `None`。纯读。
pub fn change_detail(layout: &Layout, name: &str) -> Option<ChangeDetail> {
    let location = locate_change(layout, name)?;
    let inventory = detect_inventory(&location.dir);

    let workflow_path = location.dir.join(WORKFLOW_FILE_NAME);
    let parse_result = if workflow_path.is_file() {
        Some(parse_workflow_file(&workflow_path))
    } else {
        None
    };
    let (workflow, unparsable) = match &parse_result {
        Some(WorkflowFileParse::Parsed(workflow)) => (Some(workflow), false),
        Some(WorkflowFileParse::Unparsable { .. }) => (None, true),
        None => (None, false),
    };

    // created：优先 workflow.json，archive 回退目录名日期前缀
    let created = workflow
        .and_then(|workflow| workflow.created.clone())
        .or_else(|| match location.source {
            ChangeSource::Archive => super::list::archive_prefix_date(name),
            ChangeSource::Active => None,
        });

    // 固定 9 站全量输出（无 attempt 记录的站为空序列）；
    // v0 早期代际无 workflow.json → 空流水线 + 纯文档清单形态
    let mut pipeline: Vec<PhaseEntry> = if inventory == Inventory::V0 {
        Vec::new()
    } else {
        PIPELINE_PHASES
            .iter()
            .map(|phase| PhaseEntry {
                phase: (*phase).to_string(),
                attempts: Vec::new(),
            })
            .collect()
    };
    if let Some(workflow) = workflow {
        for entry in &workflow.eval {
            if let Some(station) = pipeline.iter_mut().find(|station| station.phase == entry.phase) {
                station.attempts.push(AttemptRecord::from(entry));
            }
        }
        for station in &mut pipeline {
            // 稳定排序：attempt 相同（或缺省视为 0）者保持 eval 原始顺序
            station.attempts.sort_by_key(|record| record.attempt.unwrap_or(0));
        }
    }

    let active_phase = workflow.and_then(|workflow| workflow.active_phase.clone());
    let interrupted = workflow
        .map(|workflow| workflow.interrupted.clone())
        .unwrap_or_default();
    let file_log = workflow.and_then(|workflow| workflow.file_log.clone());
    let artifacts = discover_artifacts(&location.dir, inventory, workflow);

    Some(ChangeDetail {
        name: name.to_string(),
        source: location.source,
        inventory,
        created,
        unparsable,
        pipeline,
        active_phase,
        interrupted,
        file_log,
        artifacts,
    })
}
