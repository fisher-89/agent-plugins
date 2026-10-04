//! change 详情查询：固定 9 站流水线聚合 + 运行状态 + 产物清单。
//!
//! 出线 DTO 约定（API 层转换，golden 契约）：磁盘模型（`model`，snake_case
//! alias + 宽松时间戳）不出线；本层 DTO 为自然结构体纯 derive（零字段属性、
//! 零手动序列化——`alias`/`skip_serializing_if`/自定义编解码任一都会被
//! specta phases 模式判为相位差，分裂出 `*_Serialize/_Deserialize` 联合
//! 别名）。线面：缺省字段 `null`、时间戳 ISO 串（与磁盘数据源形态一致，
//! 由 `tests/golden` 逐字节钉死）；时间戳在 `From` 转换时定格为字符串。

use serde::{Deserialize, Serialize};
use specta::Type;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::list::ChangeSource;
use super::locate_change;
use crate::artifacts::{discover_artifacts, ArtifactDescriptor};
use crate::model::{
    ActivePhase as DiskActivePhase, ChecklistItem, FileLogEntry as DiskFileLogEntry, FileLogOp,
    InterruptedEntry as DiskInterruptedEntry, Inventory, PhaseLog, Verdict,
};
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

/// 时间戳出线转换：ISO 串。与 `model` 层 `lenient_timestamp::serialize`
/// 语义逐字一致（Rfc3339 + `unwrap_or_default` 空串降级）。
fn to_iso(timestamp: &OffsetDateTime) -> String {
    timestamp.format(&Rfc3339).unwrap_or_default()
}

/// 运行中 phase 状态（线面）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivePhase {
    pub phase: String,
    pub attempt: u32,
    pub start_at: Option<String>,
}

impl From<&DiskActivePhase> for ActivePhase {
    fn from(entry: &DiskActivePhase) -> Self {
        ActivePhase {
            phase: entry.phase.clone(),
            attempt: entry.attempt,
            start_at: entry.start_at.as_ref().map(to_iso),
        }
    }
}

/// 中断留档（线面）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedEntry {
    pub phase: String,
    pub attempt: u32,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
}

impl From<&DiskInterruptedEntry> for InterruptedEntry {
    fn from(entry: &DiskInterruptedEntry) -> Self {
        InterruptedEntry {
            phase: entry.phase.clone(),
            attempt: entry.attempt,
            start_at: entry.start_at.as_ref().map(to_iso),
            end_at: entry.end_at.as_ref().map(to_iso),
        }
    }
}

/// 一条日志式文件清单（线面）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileLogEntry {
    pub op: FileLogOp,
    pub scope: String,
    pub attempt: Option<u32>,
    pub path: String,
    pub at: Option<String>,
}

impl From<&DiskFileLogEntry> for FileLogEntry {
    fn from(entry: &DiskFileLogEntry) -> Self {
        FileLogEntry {
            op: entry.op,
            scope: entry.scope.clone(),
            attempt: entry.attempt,
            path: entry.path.clone(),
            at: entry.at.as_ref().map(to_iso),
        }
    }
}

/// 单次尝试记录：backtrack 目标与原因随条目可查；会话槽位（executor /
/// evaluator / decision）自 eval 条目直读透出，无槽位字段三值均 `null`。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRecord {
    pub attempt: Option<u32>,
    pub verdict: Verdict,
    pub report: String,
    pub checklist: Vec<ChecklistItem>,
    pub skipped: bool,
    pub stale: bool,
    pub start_at: Option<String>,
    pub timestamp: Option<String>,
    pub backtrack_to: Option<String>,
    pub backtrack_reason: Option<String>,
    pub executor_session_id: Option<String>,
    pub evaluator_session_id: Option<String>,
    pub decision_session_id: Option<String>,
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
            start_at: entry.start_at.as_ref().map(to_iso),
            timestamp: entry.timestamp.as_ref().map(to_iso),
            backtrack_to: entry.backtrack_to.clone(),
            backtrack_reason: entry.backtrack_reason.clone(),
            executor_session_id: entry.executor_session_id.clone(),
            evaluator_session_id: entry.evaluator_session_id.clone(),
            decision_session_id: entry.decision_session_id.clone(),
        }
    }
}

/// 一站流水线：该 phase 的全部 attempt 序列。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PhaseEntry {
    pub phase: String,
    pub attempts: Vec<AttemptRecord>,
}

/// change 详情聚合。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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
            if let Some(station) = pipeline
                .iter_mut()
                .find(|station| station.phase == entry.phase)
            {
                station.attempts.push(AttemptRecord::from(entry));
            }
        }
        for station in &mut pipeline {
            // 稳定排序：attempt 相同（或缺省视为 0）者保持 eval 原始顺序
            station
                .attempts
                .sort_by_key(|record| record.attempt.unwrap_or(0));
        }
    }

    let active_phase = workflow
        .and_then(|workflow| workflow.active_phase.as_ref())
        .map(ActivePhase::from);
    let interrupted = workflow
        .map(|workflow| {
            workflow
                .interrupted
                .iter()
                .map(InterruptedEntry::from)
                .collect()
        })
        .unwrap_or_default();
    let file_log = workflow
        .and_then(|workflow| workflow.file_log.as_ref())
        .map(|entries| entries.iter().map(FileLogEntry::from).collect());
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
