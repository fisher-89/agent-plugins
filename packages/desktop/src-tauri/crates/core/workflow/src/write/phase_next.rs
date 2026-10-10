//! 相位路由状态机（只读）：初始 / 推进 / fail 重试（≤[`MAX_RETRY_TIMES`]）/
//! 重试上限判定 / backtrack 目标路由 / mid-phase interruption（会话锚点比对
//! ——进程内复活，中断相位由重入的窗口比对标定）。[`phase_next`] 不改状态库
//! （只读路由，stale 标记归 [`backtrack`](super::backtrack) 单点）；executor /
//! evaluator prompt 已插值随行下发，白名单随行（walker 缓存带
//! 走，不自相位表推导——守住路由红线）。状态读取单源自 workspace 库（经
//! [`ChangeStateStore`](crate::state::ChangeStateStore) port 缝，D8：锚点基线
//! 平移为 PhaseRecord 行数）。

use std::collections::HashMap;
use std::sync::Mutex;

use crate::model::Verdict;
use crate::state::{ChangeStateStore, PhaseStateRecord};
use crate::write::phase_table::{
    allowed_backtrack_phases, interpolate, phase_table, PhaseAgentSpec, PhaseDefinition,
    MAX_RETRY_TIMES, MAX_ROUNDS,
};

/// 进程内会话锚点：(change_id, run_id) → 首见时 PhaseRecord 行数基线。每 run
/// 一个实例（组合根创建后注入工具步缝；run_id 键隔离，跨 run / 跨实例不共享，
/// 无进程级全局可变状态）。
#[derive(Default)]
pub struct SessionAnchors {
    anchors: Mutex<HashMap<(String, String), usize>>,
}

impl SessionAnchors {
    /// 空锚点表（每 run 组合根创建一个）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 首见登记基线、复见返回既有锚点（与插件 `getOrCreateAnchor` 同语义；
    /// 复合键身份段 = change id）。
    fn get_or_create(&self, change_id: &str, run_id: &str, entries_len: usize) -> usize {
        let mut anchors = self.anchors.lock().expect("会话锚点锁不可中毒");
        *anchors
            .entry((change_id.to_owned(), run_id.to_owned()))
            .or_insert(entries_len)
    }
}

/// 路由错误封闭集：重试上限是决策 agent 的唤起触发点（其余失败路径一律
/// `Err` 串走显式失败）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseNextError {
    /// 相位连续 fail 达重试上限（决策分叉）
    MaxRetriesExceeded { phase: String, round: u32 },
}

/// 最近一次评估条目快照（决策输入面）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastResult {
    pub phase: String,
    pub verdict: Verdict,
    pub report: String,
    /// 落账时刻（UTC unix 毫秒；条目恒在位，位保留消费方可空语义）
    pub timestamp: Option<i64>,
}

/// 路由产出（executor / evaluator prompt 已插值；白名单随行下发）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseNextOutcome {
    /// 全相位 pass 走完（run 收口，不触发归档）
    pub done: bool,
    pub next_phase: Option<String>,
    /// 会话窗口轮次（窗口条目数 + 1）
    pub round: u32,
    pub executor: Option<PhaseAgentSpec>,
    pub evaluator: Option<PhaseAgentSpec>,
    pub allowed_backtrack_phases: Vec<String>,
    pub last_result: Option<LastResult>,
    pub error: Option<PhaseNextError>,
}

/// 只读路由状态机（按 change **id** 寻址）：不改状态库。路由权威唯一——
/// walker 每步过渡都问本函数，白名单经其缓存下发；prompt 插值的 change 段
/// 由记录 `name` 供给（id → 记录 → name 分辨率单点）。
pub fn phase_next(
    store: &dyn ChangeStateStore,
    change_id: &str,
    run_id: &str,
    anchors: &SessionAnchors,
) -> Result<PhaseNextOutcome, String> {
    if run_id.trim().is_empty() {
        return Err("missing_run_id: 缺少必需参数 run_id".to_owned());
    }
    let record = store
        .get_change(change_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从路由"))?;
    let table = phase_table(&record.workflow_type).ok_or_else(|| {
        format!(
            "workflow_type \"{}\" 不受支持（V1 仅 requirement 工作流）",
            record.workflow_type
        )
    })?;
    let entries = store
        .list_phase_records(change_id)
        .map_err(|error| error.to_string())?;
    let anchor = anchors
        .get_or_create(change_id, run_id, entries.len())
        .min(entries.len());
    let round = (entries.len() - anchor) as u32 + 1;
    let last_result = latest_result(&entries);

    if round > MAX_ROUNDS {
        return Err(format!(
            "round_limit_exceeded: 超过 {MAX_ROUNDS} 轮限制，可能存在循环回溯。请检查 \
             回溯记录，或手动清理后重试。"
        ));
    }

    // backtrack 检测：最新条目的 backtrack_to 在位 → 路由到最早有效目标
    //（旧 phase_log 机制遗留的 backtrack 条目兼容面，与插件 handleBacktrack 同语义）
    if let Some(entry) = latest_entry(&entries) {
        if let Some(target) = entry
            .backtrack_to
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
        {
            let Some(idx) = table.iter().position(|def| def.id == target) else {
                return Err(format!(
                    "invalid_backtrack_target: 回溯目标 \"{target}\" 不包含有效的 phase 标识符"
                ));
            };
            return Ok(build_phase_response(
                table,
                &table[idx],
                round,
                &record.name,
                entry.backtrack_reason.as_deref(),
                last_result,
            ));
        }
    }

    // 默认路由：全 pass → done；首个未过相位 → 重试上限判定 → 正常下发
    let all_passed = table.iter().all(|def| has_phase_passed(&entries, def.id));
    if all_passed {
        return Ok(PhaseNextOutcome {
            done: true,
            next_phase: None,
            round,
            executor: None,
            evaluator: None,
            allowed_backtrack_phases: Vec::new(),
            last_result,
            error: None,
        });
    }
    let next = table
        .iter()
        .find(|def| !has_phase_passed(&entries, def.id))
        .expect("存在未过相位（all_passed 已排除空判）");
    let window_fails = entries[anchor..]
        .iter()
        .filter(|entry| entry.phase == next.id && entry.verdict == Verdict::Fail)
        .count() as u32;
    if window_fails >= MAX_RETRY_TIMES {
        return Ok(PhaseNextOutcome {
            done: false,
            next_phase: None,
            round,
            executor: None,
            evaluator: None,
            allowed_backtrack_phases: Vec::new(),
            last_result,
            error: Some(PhaseNextError::MaxRetriesExceeded {
                phase: next.id.to_owned(),
                round,
            }),
        });
    }
    Ok(build_phase_response(
        table,
        next,
        round,
        &record.name,
        None,
        last_result,
    ))
}

/// 相位是否已过（非 stale 的 pass / skipped 条目在位；与插件 `hasPhasePassed`
/// 同语义）。
fn has_phase_passed(entries: &[PhaseStateRecord], phase_id: &str) -> bool {
    entries.iter().any(|entry| {
        entry.phase == phase_id && (entry.verdict == Verdict::Pass || entry.skipped) && !entry.stale
    })
}

/// 最新条目（timestamp 降序取首；同时间戳保序取先——按落行序扫描，严格大于
/// 才替换）。
fn latest_entry(entries: &[PhaseStateRecord]) -> Option<&PhaseStateRecord> {
    let mut best: Option<&PhaseStateRecord> = None;
    for entry in entries {
        let replace = match best {
            None => true,
            Some(current) => entry.timestamp > current.timestamp,
        };
        if replace {
            best = Some(entry);
        }
    }
    best
}

/// 最新条目快照（决策输入面）。
fn latest_result(entries: &[PhaseStateRecord]) -> Option<LastResult> {
    latest_entry(entries).map(|entry| LastResult {
        phase: entry.phase.clone(),
        verdict: entry.verdict,
        report: entry.report.clone(),
        timestamp: Some(entry.timestamp),
    })
}

/// 正常路由响应：prompt 插值（`<change>` 段取记录 name / `<phase>`）+ 回溯
/// 原因后缀 + 白名单随行（与插件 `buildPhaseResponse` 同语义）。
fn build_phase_response(
    table: &'static [PhaseDefinition],
    def: &PhaseDefinition,
    round: u32,
    name: &str,
    backtrack_reason: Option<&str>,
    last_result: Option<LastResult>,
) -> PhaseNextOutcome {
    let reason_suffix = backtrack_reason
        .map(|reason| format!("\n\n⚠️ 回溯原因: {reason}"))
        .unwrap_or_default();
    let resolve = |spec: &PhaseAgentSpec| PhaseAgentSpec {
        agent_type: spec.agent_type.clone(),
        prompt: interpolate(&spec.prompt, name, Some(def.id)) + &reason_suffix,
        model_level: spec.model_level,
    };
    PhaseNextOutcome {
        done: false,
        next_phase: Some(def.id.to_owned()),
        round,
        executor: def.executor.as_ref().map(resolve),
        evaluator: def.evaluator.as_ref().map(resolve),
        allowed_backtrack_phases: allowed_backtrack_phases(table, def.id),
        last_result,
        error: None,
    }
}
