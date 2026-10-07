use crate::model::{ChecklistItem, Verdict};
use crate::state::{ChangeStateStore, PhaseLogCommand};
use crate::write::now_millis;
use crate::write::phase_table::{phase_table, MAX_REPORT_CHARS};

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
/// 经 [`ChangeStateStore`] port 缝单事务原子落库（PhaseRecord 行 + checklist
/// 子行 + active_phase 清位）。落账不做 gate-check（gate 逻辑全归
/// [`phase_next`](super::phase_next)），不触 backtrack 状态（归
/// [`backtrack`](super::backtrack) 单点）。
pub fn phase_log(
    store: &dyn ChangeStateStore,
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

    let record = store
        .get_change(change)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("change \"{change}\" 未建档（无 ChangeRecord），无从落账"))?;
    let table = phase_table(&record.workflow_type).ok_or_else(|| {
        format!(
            "workflow_type \"{}\" 不受支持（V1 仅 requirement 工作流）",
            record.workflow_type
        )
    })?;
    // 表位前置：未知 / 非表内相位显式 `Err`（与插件 validatePhaseTarget 同
    // 词汇——写面不为表外相位落账）
    if !table.iter().any(|def| def.id == input.phase) {
        return Err(format!(
            "工作流 \"{}\" 不包含 phase \"{}\"",
            record.workflow_type, input.phase
        ));
    }
    // 开启阶段前置：active_phase 匹配校验（开相才可落账）
    let start_at = match record.active_phase.as_ref() {
        None => {
            return Err(format!(
                "phase \"{}\" 未开启（无 active_phase），请先 phase_start 开启阶段",
                input.phase
            ))
        }
        Some(active) if active.phase == input.phase => Some(active.start_at),
        Some(active) => {
            return Err(format!(
                "phase \"{}\" 未开启（active_phase 停留在 \"{}\"），不可跨相落账",
                input.phase, active.phase
            ))
        }
    };

    let attempt = store
        .log_phase(&PhaseLogCommand {
            change: change.to_owned(),
            phase: input.phase.clone(),
            verdict,
            report: input.report.clone(),
            skipped: input.skipped,
            checklist: input.checklist.clone(),
            executor_session_id: input.executor_session_id.clone(),
            evaluator_session_id: input.evaluator_session_id.clone(),
            decision_session_id: input.decision_session_id.clone(),
            start_at,
            timestamp: now_millis(),
        })
        .map_err(|error| error.to_string())?;
    Ok(PhaseLogOutcome {
        phase: input.phase.clone(),
        attempt,
    })
}
