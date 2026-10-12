use serde::{Deserialize, Serialize};
use specta::Type;

use super::list::ChangeSource;
use super::{iso_from_millis, locate_change};
use crate::artifacts::{discover_artifacts, ArtifactDescriptor};
use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    ChangeStateStore, ChangeStatus, PhaseStateRecord, RunEdgeKind, RunEdgeStateRecord,
    RunNodeStateRecord, RunStateRecord, RunStatus, RunStepKind, RunStepStatus,
};
use crate::write::{phase_table, utc_date};
use foundation::layout::Layout;

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

/// run 执行图节点行线面（十八值全词汇封闭集直出，词汇 = `state::RunStepKind`
/// 单点；稳定身份键 `(phase, attempt, step, instanceNo)`，库读序 = node_seq 升序
/// = emit 序）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRunNodeRecord {
    pub phase: String,
    pub attempt: u32,
    pub step: RunStepKind,
    /// 同 `(phase, attempt, step)` 内的执行实例号（feedback 重跑递增）
    pub instance_no: u32,
    pub status: RunStepStatus,
    pub session_id: Option<String>,
    pub detail: Option<String>,
    /// 全量输出引用（`ExecOutput::Recorded { exec_id }`；无记录恒 null）
    pub output_ref: Option<String>,
}

/// run 执行图边行线面（四 kind 封闭集直出；端点 = 节点身份串）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRunEdgeRecord {
    pub from_node_key: String,
    pub to_node_key: String,
    pub edge_kind: RunEdgeKind,
}

/// 单次 run 执行图条目（全史不截；在飞 run 的 start 行已在库——status=
/// running、nodes / edges 恒空，run 清单面运行中可见）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRunEntry {
    pub run_id: String,
    pub status: RunStatus,
    pub reason: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// 执行图节点（全词汇零过滤，node_seq 升序 = emit 序）
    pub nodes: Vec<ChangeRunNodeRecord>,
    /// 执行图边（四 kind，edge_seq 升序 = emit 序）
    pub edges: Vec<ChangeRunEdgeRecord>,
}

impl From<&RunStateRecord> for ChangeRunEntry {
    fn from(record: &RunStateRecord) -> Self {
        ChangeRunEntry {
            run_id: record.run_id.clone(),
            status: record.status,
            reason: record.reason.clone(),
            started_at: Some(iso_from_millis(record.started_at)),
            finished_at: record.finished_at.map(iso_from_millis),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

impl From<&RunNodeStateRecord> for ChangeRunNodeRecord {
    fn from(record: &RunNodeStateRecord) -> Self {
        ChangeRunNodeRecord {
            phase: record.phase.clone(),
            attempt: record.attempt,
            step: record.step,
            instance_no: record.instance_no,
            status: record.status,
            session_id: record.session_id.clone(),
            detail: record.detail.clone(),
            output_ref: record.output_ref.clone(),
        }
    }
}

impl From<&RunEdgeStateRecord> for ChangeRunEdgeRecord {
    fn from(record: &RunEdgeStateRecord) -> Self {
        ChangeRunEdgeRecord {
            from_node_key: record.from_node_key.clone(),
            to_node_key: record.to_node_key.clone(),
            edge_kind: record.edge_kind,
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
    /// run 执行图全量读面（unify-run-state-persistence + run-workflow-by-step：
    /// 全史不截，runs 按 `started_at` 升序、nodes 按 emit 序、edges 按 emit 序；
    /// 未建档 / 未跑形态恒空数组）
    pub runs: Vec<ChangeRunEntry>,
    pub artifacts: Vec<ArtifactDescriptor>,
    /// 本 change 的 worktree 绝对路径（自建档记录直读透出，None → null；
    /// legacy 记录不渲染——路径为刻意出线的执行锚，review / merge 可达）
    pub worktree: Option<String>,
}

/// 聚合单个 change 的详情（按 id 寻址）；db 无该 id 记录 → 恒 `None`
///（未知 id 未找到——MUST NOT 回退磁盘目录解析、MUST NOT 返回文档形态空面、
/// 零 workflow.json 读取）；`record.workflow_type` 不在相位表内（未知 / 空白
/// 类型）→ 同样恒 `None`（读面严格：不静默按 requirement 出线）。纯读：
/// record 先读后定位（design D12）——`name` / `worktree` 自记录直供
/// `locate_change` 回退（merge 前主仓两树未命中仍可达）；**建档记录恒可达
/// 详情**——定位全 miss（worktree 被手动删除、未 merge）→ dir 缺席、产物清单
/// 空、状态面在（`source` 自 record.status 映射）。
pub fn change_detail(
    layout: &Layout,
    store: &dyn ChangeStateStore,
    id: &str,
) -> Option<ChangeDetail> {
    // 未建档（未知 id）→ 恒 None（零磁盘回退零文档形态）
    let record = store.get_change(id).ok().flatten()?;
    // 相位表随 workflow_type 分派（未知类型 → 恒 None，与未知 id 同语义「详情
    // 不可达」——读面 MUST NOT 静默按 requirement 出线）
    let table = phase_table(&record.workflow_type)?;
    // 磁盘面 name / worktree 恒自记录供给（id → 记录 → name 分辨率单点）
    let location = locate_change(layout, record.worktree.as_deref(), record.name.as_str());
    let entries = store.list_phase_records(id).unwrap_or_default();

    // run 执行图全量读面：runs 按 started_at 升序（并列按 run_id 稳定序）、
    // nodes / edges 按库读序（node_seq / edge_seq 升序 = emit 序）；读失败降级空
    // runs（与清单读面 unwrap_or_default 同哲学）。
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
        run_entry.nodes = store
            .list_run_nodes(&run_entry.run_id)
            .unwrap_or_default()
            .iter()
            .map(ChangeRunNodeRecord::from)
            .collect();
        run_entry.edges = store
            .list_run_edges(&run_entry.run_id)
            .unwrap_or_default()
            .iter()
            .map(ChangeRunEdgeRecord::from)
            .collect();
    }

    // created：db created_at 唯一来源（磁盘目录名日期前缀回退已退役——
    // 记录恒在，name 恒裸名不带前缀）
    let created = Some(utc_date(record.created_at));

    // 流水线相位序 = `phase_table(workflow_type)` 相位序（requirement 9 列 /
    // test-only 6 列，archive 列位置随表；无 attempt 记录的站为空序列）
    let mut pipeline: Vec<PhaseEntry> = table
        .iter()
        .map(|definition| PhaseEntry { phase: definition.id.to_string(), attempts: Vec::new() })
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
    // 产物发现经定位目录 + 相位定义清单（清单为唯一候选来源；归属随清单出线）；
    // 定位 miss（建档记录恒可达路径）→ 产物清单空；archive / code-analyze 相位
    // 零产物清单。
    let manifest = table;
    let artifacts = location
        .as_ref()
        .map(|location| discover_artifacts(&location.dir, &entries, manifest))
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
