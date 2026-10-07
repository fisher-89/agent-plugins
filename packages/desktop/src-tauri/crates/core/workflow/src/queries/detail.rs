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
    /// 本 change 的 worktree 绝对路径（自建档记录直读透出，None → null；
    /// legacy 记录不渲染——路径为刻意出线的执行锚，review / merge 可达）
    pub worktree: Option<String>,
}

/// 聚合单个 change 的详情；未知 change 名返回 `None`。纯读：db 缺记录的
/// change 返回空流水线 + 产物清单（文档形态），零 workflow.json 读取。
/// record 先读后定位（design D12）：worktree 自记录直传 `locate_change` 回退
/// （merge 前主仓两树未命中仍可达）；**建档记录恒可达详情**——定位全 miss
///（worktree 被手动删除、未 merge）→ dir 缺席、产物清单空、状态面在
///（`source` 自 record.status 映射）；record 与定位双缺 → `None`（文档形态
/// 未知名，既有语义）。
pub fn change_detail(
    layout: &Layout,
    store: &dyn ChangeStateStore,
    name: &str,
) -> Option<ChangeDetail> {
    let record = store.get_change(name).ok().flatten();
    let location = locate_change(
        layout,
        record
            .as_ref()
            .and_then(|record| record.worktree.as_deref()),
        name,
    );
    // record 与定位双缺 → None（不虚构文档形态）
    if record.is_none() && location.is_none() {
        return None;
    }
    let entries = match &record {
        // 建档 change：读相位评估史组装流水线
        Some(_) => store.list_phase_records(name).unwrap_or_default(),
        // 文档形态：零状态面（空流水线 + 产物清单）
        None => Vec::new(),
    };

    // created：优先 db created_at，archive 回退目录名日期前缀
    let created = record
        .as_ref()
        .map(|record| utc_date(record.created_at))
        .or_else(|| match location.as_ref().map(|location| location.source) {
            Some(ChangeSource::Archive) => super::list::archive_prefix_date(name),
            _ => None,
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
    // 产物发现经定位目录；定位 miss（建档记录恒可达路径）→ 产物清单空
    let artifacts = location
        .as_ref()
        .map(|location| discover_artifacts(&location.dir, &entries))
        .unwrap_or_default();
    // source：定位命中随定位；定位 miss 自 record.status 映射（状态面在的
    // 呈现形态——worktree 被删 / 未 merge 的建档记录）
    let source = location.map(|location| location.source).unwrap_or(
        match record.as_ref().map(|record| record.status) {
            Some(ChangeStatus::Archived) => ChangeSource::Archive,
            _ => ChangeSource::Active,
        },
    );

    let status = record.as_ref().map(|record| record.status);
    let worktree = record.as_ref().and_then(|record| record.worktree.clone());
    Some(ChangeDetail {
        name: name.to_string(),
        source,
        status,
        created,
        pipeline,
        active_phase,
        artifacts,
        worktree,
    })
}
