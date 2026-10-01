//! 评估落账写操作：verdict 推导（checklist 全 pass）、skipped 约束、report
//! 长度门、表位与开相前置（相位在表中且 `active_phase` 匹配，未开相显式
//! `Err`——写面严格语义，对照插件的宽容形态收紧）、纯追加落账（W9 缺陷修复
//! 点——attempt 推导只数该相位既有条目加一，不因相位已有 pass 条目短路跳过，
//! backtrack 回跳后同相位重评条目逐条 append）、start_at 自 `active_phase`
//! 继承、落账后清 `active_phase`。

use serde_json::Value;

use crate::model::{ChecklistItem, Verdict};
use crate::write::persist::{eval_entries_mut, format_timestamp, load_doc, now_iso, save};
use crate::write::phase_table::{phase_table, MAX_TEXT_CHARS};
use foundation::layout::Layout;

/// 落账输入（checklist 用 `workflow::model::ChecklistItem` 域类型）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLogInput {
    pub phase: String,
    pub report: String,
    pub checklist: Vec<ChecklistItem>,
    pub skipped: bool,
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
    if report_chars > MAX_TEXT_CHARS {
        return Err(format!(
            "报告长度超过 {MAX_TEXT_CHARS} 字符限制（当前 {report_chars} 字符）。请精简报告内容。"
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
    // 开相前置：相位须处于开启态（`active_phase` 匹配）——开相位
    // （[`phase_start`](super::phase_start)）才可落账，杜绝无主落账与
    // start_at 无源（对照插件「非匹配 active_phase 照落不拒」的宽容形态，
    // 进程内写面收紧为显式 `Err`）
    let start_at = match doc.typed.active_phase.as_ref() {
        None => {
            return Err(format!(
                "phase \"{}\" 未开启（无 active_phase），请先 phase_start 开相位",
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
        // 扩展字段仅在显式在位时写入（与插件 buildEntry 同形态）
        if input.skipped {
            object.insert("skipped".to_owned(), Value::Bool(true));
        }
        if let Some(start_at) = start_at {
            object.insert(
                "start_at".to_owned(),
                Value::String(format_timestamp(start_at)),
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
