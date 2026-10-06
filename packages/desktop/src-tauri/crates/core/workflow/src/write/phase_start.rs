use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::write::persist::{load_doc, now_iso, save};
use crate::write::phase_table::phase_table;
use foundation::layout::Layout;

/// 开启阶段产出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseStartOutcome {
    pub phase: String,
    pub attempt: u32,
    pub start_at: OffsetDateTime,
}

/// 开启阶段：`active_phase` 定点写入。phase 必须属于该 change workflow_type 的
/// 相位表（非法 phase 显式 `Err`，workflow.json 原文不动）。
pub fn phase_start(
    layout: &Layout,
    change: &str,
    phase: &str,
) -> Result<PhaseStartOutcome, String> {
    let mut doc = load_doc(layout, change)?;
    let table = phase_table(&doc.typed.workflow_type).ok_or_else(|| {
        format!(
            "workflow_type \"{}\" 不受支持（V1 仅 requirement 工作流）",
            doc.typed.workflow_type
        )
    })?;
    if !table.iter().any(|def| def.id == phase) {
        return Err(format!(
            "phase \"{phase}\" 不属于 workflow_type \"{}\" 的 phase 表 (change: {change})，\
             active_phase 未写入。",
            doc.typed.workflow_type
        ));
    }

    let attempt = doc
        .typed
        .eval
        .iter()
        .filter(|entry| entry.phase == phase)
        .count() as u32
        + 1;
    let start_at = OffsetDateTime::now_utc();
    doc.raw["active_phase"] = serde_json::json!({
        "phase": phase,
        "attempt": attempt,
        "start_at": start_at.format(&Rfc3339).unwrap_or_else(|_| now_iso()),
    });
    save(&doc)?;
    Ok(PhaseStartOutcome {
        phase: phase.to_owned(),
        attempt,
        start_at,
    })
}
