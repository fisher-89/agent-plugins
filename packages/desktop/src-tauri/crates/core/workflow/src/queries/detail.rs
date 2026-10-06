//! change 详情查询：固定 9 站流水线聚合 + 状态面 + 产物清单。
//!
//! 出线 DTO 约定（API 层转换，golden 契约）：db 中性状态类型（`state`）不出
//! 线；本层 DTO 为自然结构体纯 derive（零字段属性、零手动序列化——
//! `alias`/`skip_serializing_if`/自定义编解码任一都会被 specta phases 模式
//! 判为相位差，分裂出 `*_Serialize/_Deserialize` 联合别名）。线面：缺省字段
//! `null`、时间戳 ISO 串（由 `tests/golden` 逐字节钉死）；时间戳在 `From`
//! 转换时定格为字符串（i64 毫秒 → RFC3339 收 queries 层单点）。状态面单源
//! workspace 库（经 port 缝）；磁盘扫描保留为产物发现。

use serde::{Deserialize, Serialize};
use specta::Type;

use super::list::ChangeSource;
use super::{iso_from_millis, locate_change};
use crate::artifacts::{discover_artifacts, ArtifactDescriptor};
use crate::model::{ChecklistItem, Verdict};
use crate::state::{ChangeStateStore, ChangeStatus, PhaseStateRecord};
use crate::write::utc_date;
use foundation::layout::Layout;

/// 固定 9 站流水线（顺序固定，不依赖落行排列）。
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

/// 运行中 phase 状态（线面）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivePhase {
    pub phase: String,
    pub attempt: u32,
    pub start_at: Option<String>,
}

/// 单次尝试记录：backtrack 目标与原因随条目可查；会话槽位（executor /
/// evaluator / decision）自 PhaseRecord 三槽位列直读透出，无槽位字段三值均
/// `null`。
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

impl From<&PhaseStateRecord> for AttemptRecord {
    fn from(entry: &PhaseStateRecord) -> Self {
        AttemptRecord {
            attempt: Some(entry.attempt),
            verdict: entry.verdict,
            report: entry.report.clone(),
            checklist: entry.checklist.clone(),
            skipped: entry.skipped,
            stale: entry.stale,
            start_at: entry.start_at.map(iso_from_millis),
            timestamp: Some(iso_from_millis(entry.timestamp)),
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

/// change 详情聚合。`status` 为建档判别面：`Some` = db 已建档（完整状态面），
/// `None` = 文档形态（db 缺记录的存量 CLI change，空流水线 + 产物清单）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDetail {
    pub name: String,
    pub source: ChangeSource,
    pub status: Option<ChangeStatus>,
    pub created: Option<String>,
    pub pipeline: Vec<PhaseEntry>,
    pub active_phase: Option<ActivePhase>,
    pub artifacts: Vec<ArtifactDescriptor>,
}

/// 聚合单个 change 的详情；未知 change 名返回 `None`。纯读：db 缺记录的
/// change 返回空流水线 + 产物清单（文档形态），零 workflow.json 读取。
pub fn change_detail(
    layout: &Layout,
    store: &dyn ChangeStateStore,
    name: &str,
) -> Option<ChangeDetail> {
    let location = locate_change(layout, name)?;
    let record = store.get_change(name).ok().flatten();
    let entries = match record {
        // 建档 change：读相位评估史组装流水线
        Some(_) => store.list_phase_records(name).unwrap_or_default(),
        // 文档形态：零状态面（空流水线 + 产物清单）
        None => Vec::new(),
    };

    // created：优先 db created_at，archive 回退目录名日期前缀
    let created = record
        .as_ref()
        .map(|record| utc_date(record.created_at))
        .or_else(|| match location.source {
            ChangeSource::Archive => super::list::archive_prefix_date(name),
            ChangeSource::Active => None,
        });

    // 固定 9 站全量输出（无 attempt 记录的站为空序列）；文档形态（db 缺记
    // 录）空流水线
    let mut pipeline: Vec<PhaseEntry> = if record.is_some() {
        PIPELINE_PHASES
            .iter()
            .map(|phase| PhaseEntry {
                phase: (*phase).to_string(),
                attempts: Vec::new(),
            })
            .collect()
    } else {
        Vec::new()
    };
    for entry in &entries {
        if let Some(station) = pipeline
            .iter_mut()
            .find(|station| station.phase == entry.phase)
        {
            station.attempts.push(AttemptRecord::from(entry));
        }
    }
    for station in &mut pipeline {
        // 稳定排序：落行序（id 升序）即既有 eval 顺序，attempt 相同者保持原序
        station
            .attempts
            .sort_by_key(|record| record.attempt.unwrap_or(0));
    }

    let active_phase = record
        .as_ref()
        .and_then(|record| record.active_phase.as_ref())
        .map(|active| ActivePhase {
            phase: active.phase.clone(),
            attempt: active.attempt,
            start_at: Some(iso_from_millis(active.start_at)),
        });
    let artifacts = discover_artifacts(&location.dir, &entries);

    Some(ChangeDetail {
        name: name.to_string(),
        source: location.source,
        status: record.map(|record| record.status),
        created,
        pipeline,
        active_phase,
        artifacts,
    })
}
