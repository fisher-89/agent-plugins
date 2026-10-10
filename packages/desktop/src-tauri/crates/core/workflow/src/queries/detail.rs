//! change 详情查询（按 change **id** 寻址）：固定 9 站流水线聚合 + 状态面 +
//! run 运行史 + 产物清单。db 无该 id 记录 → 恒 `None`（文档形态分支整体退役
//! ——零磁盘目录解析回退、零空流水线空面、零 workflow.json 读取）。
//!
//! 出线 DTO 约定（API 层转换，golden 契约）：db 中性状态类型（`state`）不出
//! 线（`ChangeStatus` / `RunStatus` / `RunStepKind` / `RunStepStatus` 词汇枚
//! 举先例直用）；本层 DTO 为自然结构体纯 derive（零字段属性、零手动序列化——
//! `alias`/`skip_serializing_if`/自定义编解码任一都会被 specta phases 模式
//! 判为相位差，分裂出 `*_Serialize/_Deserialize` 联合别名）。线面：缺省字段
//! `null`、时间戳 ISO 串（由 `tests/golden` 逐字节钉死）；时间戳在 `From`
//! 转换时定格为字符串（i64 毫秒 → RFC3339 收 queries 层单点）。状态面单源
//! workspace 库（经 port 缝）；磁盘扫描保留为产物发现。runs / steps 字段演
//! 进走 golden 显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留
//! 痕，unify-run-state-persistence 增键先例）。

use serde::{Deserialize, Serialize};
use specta::Type;

use super::list::ChangeSource;
use super::{iso_from_millis, locate_change};
use crate::artifacts::{discover_artifacts, ArtifactDescriptor};
use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    ChangeStateStore, ChangeStatus, PhaseStateRecord, RunStateRecord, RunStatus, RunStepKind,
    RunStepStateRecord, RunStepStatus,
};
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

/// run 步节点史行线面（五词汇封闭集直出，词汇 = `state::RunStepKind` 单点；
/// seq = emit 序稳定升序）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRunStepRecord {
    pub seq: u64,
    pub phase: String,
    pub attempt: u32,
    pub step: RunStepKind,
    pub status: RunStepStatus,
    pub session_id: Option<String>,
    pub detail: Option<String>,
}

/// 单次 run 运行史条目（全史不截；在飞 run 的 start 行已在库——status=
/// running、steps 恒空，run 清单面运行中可见）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRunEntry {
    pub run_id: String,
    pub status: RunStatus,
    pub reason: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub steps: Vec<ChangeRunStepRecord>,
}

impl From<&RunStateRecord> for ChangeRunEntry {
    fn from(record: &RunStateRecord) -> Self {
        ChangeRunEntry {
            run_id: record.run_id.clone(),
            status: record.status,
            reason: record.reason.clone(),
            started_at: Some(iso_from_millis(record.started_at)),
            finished_at: record.finished_at.map(iso_from_millis),
            steps: Vec::new(),
        }
    }
}

impl From<&RunStepStateRecord> for ChangeRunStepRecord {
    fn from(record: &RunStepStateRecord) -> Self {
        ChangeRunStepRecord {
            seq: record.seq,
            phase: record.phase.clone(),
            attempt: record.attempt,
            step: record.step,
            status: record.status,
            session_id: record.session_id.clone(),
            detail: record.detail.clone(),
        }
    }
}

/// change 详情聚合。`id` 为身份锚（寻址入参同值）；`name` 自记录直读（恒裸名
/// ——归档 change 的日期前缀仅存在于磁盘目录名，MUST NOT 进入出线值）；db 无
/// 该 id 记录 → 详情整体不可达（`None`，未知 id 语义）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDetail {
    /// change 身份锚（uuid 形态）
    pub id: String,
    /// change 名（自记录直读，恒裸名）
    pub name: String,
    /// 人类可读标题，恒非空（自记录直读，见 desktop-change-state-store）
    pub title: String,
    pub source: ChangeSource,
    pub status: Option<ChangeStatus>,
    pub created: Option<String>,
    pub pipeline: Vec<PhaseEntry>,
    pub active_phase: Option<ActivePhase>,
    /// run 运行史全量读面（unify-run-state-persistence：全史不截，runs 按
    /// `started_at` 升序、steps 按 seq 升序；文档形态恒空数组）
    pub runs: Vec<ChangeRunEntry>,
    pub artifacts: Vec<ArtifactDescriptor>,
    /// 本 change 的 worktree 绝对路径（自建档记录直读透出，None → null；
    /// legacy 记录不渲染——路径为刻意出线的执行锚，review / merge 可达）
    pub worktree: Option<String>,
}

/// 聚合单个 change 的详情（按 id 寻址）；db 无该 id 记录 → 恒 `None`
///（未知 id 未找到——MUST NOT 回退磁盘目录解析、MUST NOT 返回文档形态空面、
/// 零 workflow.json 读取）。纯读：record 先读后定位（design D12）——`name` /
/// `worktree` 自记录直供 `locate_change` 回退（merge 前主仓两树未命中仍可
/// 达）；**建档记录恒可达详情**——定位全 miss（worktree 被手动删除、未
/// merge）→ dir 缺席、产物清单空、状态面在（`source` 自 record.status 映射）。
pub fn change_detail(
    layout: &Layout,
    store: &dyn ChangeStateStore,
    id: &str,
) -> Option<ChangeDetail> {
    // 未建档（未知 id）→ 恒 None（零磁盘回退零文档形态）
    let record = store.get_change(id).ok().flatten()?;
    // 磁盘面 name / worktree 恒自记录供给（id → 记录 → name 分辨率单点）
    let location = locate_change(layout, record.worktree.as_deref(), record.name.as_str());
    let entries = store.list_phase_records(id).unwrap_or_default();

    // run 运行史全量读面：runs 按 started_at 升序（并列按 run_id 稳定序）、
    // steps 按 seq 升序（= emit 序）；读失败降级空 runs（与清单读面
    // unwrap_or_default 同哲学）。
    let mut runs: Vec<ChangeRunEntry> = store
        .list_runs(id)
        .unwrap_or_default()
        .iter()
        .map(ChangeRunEntry::from)
        .collect();
    runs.sort_by(|a, b| {
        // started_at 升序已在 store 面保证，此处并列序兜底（run_id 稳定序）
        a.started_at
            .cmp(&b.started_at)
            .then_with(|| a.run_id.cmp(&b.run_id))
    });
    for run_entry in &mut runs {
        let mut steps: Vec<ChangeRunStepRecord> = store
            .list_run_steps(&run_entry.run_id)
            .unwrap_or_default()
            .iter()
            .map(ChangeRunStepRecord::from)
            .collect();
        // seq 升序（= emit 序；库面空洞合法，稳定序即重放序）
        steps.sort_by_key(|step| step.seq);
        run_entry.steps = steps;
    }

    // created：db created_at 唯一来源（磁盘目录名日期前缀回退已退役——
    // 记录恒在，name 恒裸名不带前缀）
    let created = Some(utc_date(record.created_at));

    // 固定 9 站全量输出（无 attempt 记录的站为空序列）
    let mut pipeline: Vec<PhaseEntry> = PIPELINE_PHASES
        .iter()
        .map(|phase| PhaseEntry {
            phase: (*phase).to_string(),
            attempts: Vec::new(),
        })
        .collect();
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

    let active_phase = record.active_phase.as_ref().map(|active| ActivePhase {
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
    let source = location
        .map(|location| location.source)
        .unwrap_or(match record.status {
            ChangeStatus::Archived => ChangeSource::Archive,
            ChangeStatus::Active => ChangeSource::Active,
        });

    Some(ChangeDetail {
        id: record.id.clone(),
        name: record.name.clone(),
        title: record.title.clone(),
        source,
        status: Some(record.status),
        created,
        pipeline,
        active_phase,
        runs,
        artifacts,
        worktree: record.worktree.clone(),
    })
}
