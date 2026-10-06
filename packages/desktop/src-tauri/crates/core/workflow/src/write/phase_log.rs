use serde_json::Value;

use crate::model::{ChecklistItem, Verdict};
use crate::write::persist::{eval_entries_mut, format_timestamp, load_doc, now_iso, save};
use crate::write::phase_table::{phase_table, MAX_REPORT_CHARS};
use foundation::layout::Layout;

/// 落账输入（checklist 用 `workflow::model::ChecklistItem` 域类型）。会话槽
/// 位由调用方（walker）从 `WorkerTurnOutcome.session_id` 取值传入，写面不
/// 解释不改写；decision 槽位不走本输入（归
/// [`decision_log`](super::decision_log) 单点挂账），恒 `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLogInput {
    pub phase: String,
    pub report: String,
    pub checklist: Vec<ChecklistItem>,
    pub skipped: bool,
    pub executor_session_id: Option<String>,
    pub evaluator_session_id: Option<String>,
    pub decision_session_id: Option<String>,
}

/// 落账产出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLogOutcome {
    pub phase: String,
    pub attempt: u32,
}

/// verdict 推导：checklist 全 pass → pass，否则 fail（空 checklist → pass，
/// 与插件 `resolveVerdict` 的 every 语义一致）。
fn derive_verdict(checklist: &[ChecklistItem]) -> Verdict {
    if checklist.iter().all(|item| item.pass) {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

/// 评估落账：校验（verdict-skipped 约束 / report 长度 / 表位 / 开相前置）→
/// 纯追加 → 清 `active_phase`。落账不做 gate-check（gate 逻辑全归
/// [`phase_next`](super::phase_next)），不触 backtrack 状态（归
/// [`backtrack`](super::backtrack) 单点）。
pub fn phase_log(
    layout: &Layout,
    change: &str,
    input: &PhaseLogInput,
) -> Result<PhaseLogOutcome, String> {
    let verdict = derive_verdict(&input.checklist);
    if input.skipped && verdict != Verdict::Pass {
        return Err(format!(
            "skipped=true 时 verdict 必须为 \"pass\"，但推导为: \"{}\"",
            verdict.as_str()
        ));
    }
    let report_chars = input.report.chars().count();
    if report_chars > MAX_REPORT_CHARS {
        return Err(format!(
            "报告长度超过 {MAX_REPORT_CHARS} 字符限制（当前 {report_chars} 字符）。请精简报告内容。"
        ));
    }

    let mut doc = load_doc(layout, change)?;
    let table = phase_table(&doc.typed.workflow_type).ok_or_else(|| {
        format!(
            "workflow_type \"{}\" 不受支持（V1 仅 requirement 工作流）",
            doc.typed.workflow_type
        )
    })?;
    // 表位前置：未知 / 非表内相位显式 `Err`（与插件 validatePhaseTarget 同
    // 词汇——写面不为表外相位落账）
    if !table.iter().any(|def| def.id == input.phase) {
        return Err(format!(
            "工作流 \"{}\" 不包含 phase \"{}\"",
            doc.typed.workflow_type, input.phase
        ));
    }
    // 开启阶段
    let start_at = match doc.typed.active_phase.as_ref() {
        None => {
            return Err(format!(
                "phase \"{}\" 未开启（无 active_phase），请先 phase_start 开启阶段",
                input.phase
            ))
        }
        Some(active) if active.phase == input.phase => active.start_at,
        Some(active) => {
            return Err(format!(
                "phase \"{}\" 未开启（active_phase 停留在 \"{}\"），不可跨相落账",
                input.phase, active.phase
            ))
        }
    };

    // 纯追加（W9）：attempt 只数该相位既有条目 + 1，无任何短路跳过
    let attempt = doc
        .typed
        .eval
        .iter()
        .filter(|entry| entry.phase == input.phase)
        .count() as u32
        + 1;

    let mut entry = serde_json::json!({
        "phase": input.phase,
        "attempt": attempt,
        "verdict": verdict.as_str(),
        "report": input.report,
        "checklist": input.checklist,
        "timestamp": now_iso(),
    });
    if let Some(object) = entry.as_object_mut() {
        // 扩展字段仅在显式在位时写入（与插件 buildEntry 同形态）；会话槽位
        // 缺省不产生键（条目形状与既有形态一致）
        if input.skipped {
            object.insert("skipped".to_owned(), Value::Bool(true));
        }
        if let Some(start_at) = start_at {
            object.insert(
                "start_at".to_owned(),
                Value::String(format_timestamp(start_at)),
            );
        }
        if let Some(session_id) = &input.executor_session_id {
            object.insert(
                "executor_session_id".to_owned(),
                Value::String(session_id.clone()),
            );
        }
        if let Some(session_id) = &input.evaluator_session_id {
            object.insert(
                "evaluator_session_id".to_owned(),
                Value::String(session_id.clone()),
            );
        }
        if let Some(session_id) = &input.decision_session_id {
            object.insert(
                "decision_session_id".to_owned(),
                Value::String(session_id.clone()),
            );
        }
    }
    eval_entries_mut(&mut doc.raw).push(entry);

    // 落账后清 `active_phase`（开相才可落账 → 落账即收相位）
    doc.raw["active_phase"] = Value::Null;
    save(&doc)?;
    Ok(PhaseLogOutcome {
        phase: input.phase.clone(),
        attempt,
    })
}
