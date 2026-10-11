use std::collections::HashMap;
use std::sync::Mutex;

use agent::ModelLevel;

use crate::model::{ChecklistItem, Verdict};
use crate::state::{ChangeStateStore, PhaseStateRecord};
use crate::write::phase_table::{
    allowed_backtrack_phases, phase_table, PhaseAgentSpec, PhaseDefinition, MAX_RETRY_TIMES,
    MAX_ROUNDS,
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

/// 路由产出的已组装角色会话规格：静态角色知识主体 + 上下文头
/// `change:` + 记录 `name`（append）+ 失败反馈段 / 回溯原因行（append）；跨
/// crate 消费方（orchestration）只消费本类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPhaseSpec {
    pub prompt: String,
    pub model_level: ModelLevel,
}

/// 路由产出（executor / evaluator prompt 已组装；白名单随行下发）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseNextOutcome {
    /// 全相位 pass 走完（run 收口，不触发归档）
    pub done: bool,
    pub next_phase: Option<String>,
    /// 会话窗口轮次（窗口条目数 + 1）
    pub round: u32,
    pub executor: Option<ResolvedPhaseSpec>,
    pub evaluator: Option<ResolvedPhaseSpec>,
    pub allowed_backtrack_phases: Vec<String>,
    pub last_result: Option<LastResult>,
    pub error: Option<PhaseNextError>,
}

/// 只读路由状态机（按 change **id** 寻址）：不改状态库。路由权威唯一——
/// walker 每步过渡都问本函数，白名单经其缓存下发；上下文头的 change 段由记录
/// `name` 供给（id → 记录 → name 分辨率单点）。
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
                None,
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
    let retry_feedback =
        latest_phase_entry(&entries, next.id).filter(|entry| entry.verdict == Verdict::Fail);
    Ok(build_phase_response(
        table,
        next,
        round,
        &record.name,
        None,
        last_result,
        retry_feedback,
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

/// 指定相位的最新条目（timestamp 降序取首，与 [`latest_entry`] 同判序；相位
/// 过滤前置——失败反馈只认所路由相位自身的落账，不借全局最新条目）。
fn latest_phase_entry<'a>(
    entries: &'a [PhaseStateRecord],
    phase: &str,
) -> Option<&'a PhaseStateRecord> {
    let mut best: Option<&PhaseStateRecord> = None;
    for entry in entries.iter().filter(|entry| entry.phase == phase) {
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

/// 失败反馈段：fail 重试时随 prompt 下发的检查结果——失败项优先（item +
/// evidence）；全 pass 清单配 fail verdict 的漂移形态全量随行不虚减（与决策
/// 输入 `build_decision_input` 同判）；清单空退回 report 全文。
fn fail_feedback_suffix(record: &PhaseStateRecord) -> String {
    let failing: Vec<&ChecklistItem> = record
        .checklist
        .iter()
        .filter(|item| !item.pass)
        .collect();
    let source: Vec<&ChecklistItem> = if failing.is_empty() {
        record.checklist.iter().collect()
    } else {
        failing
    };
    let items: Vec<String> = source
        .iter()
        .map(|item| format!("- {}：{}", item.item, item.evidence))
        .collect();
    let body = if items.is_empty() {
        record.report.clone()
    } else {
        items.join("\n")
    };
    format!(
        "\n\n⚠️ 上次评估未通过（attempt {}），请针对以下失败项修复：\n\n{body}",
        record.attempt
    )
}

/// 正常路由响应：上下文头（`change:` + 记录 `name`）+ 静态角色知识主体 +
/// 失败反馈段（fail 重试时在场）+ 回溯原因后缀（backtrack 路由时在场，与
/// 反馈段互斥）+ 白名单随行（append 组装，零模板替换）。
fn build_phase_response(
    table: &'static [PhaseDefinition],
    def: &PhaseDefinition,
    round: u32,
    name: &str,
    backtrack_reason: Option<&str>,
    last_result: Option<LastResult>,
    retry_feedback: Option<&PhaseStateRecord>,
) -> PhaseNextOutcome {
    let context_header = format!("change: {name}\n\n");
    let reason_suffix = backtrack_reason
        .map(|reason| format!("\n\n⚠️ 回溯原因: {reason}"))
        .unwrap_or_default();
    let feedback_suffix = retry_feedback
        .map(fail_feedback_suffix)
        .unwrap_or_default();
    let resolve = |spec: &PhaseAgentSpec| ResolvedPhaseSpec {
        prompt: format!(
            "{context_header}{}{reason_suffix}{feedback_suffix}",
            spec.prompt
        ),
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
