//! 回溯写操作：白名单二次校验（越权 `Err` 不写——walker 侧 `ensure_backtrack_allowed`
//! 预校验的兜底道）、reason 长度门、最新条目标记 backtrack_to / backtrack_reason、
//! stale 标记与相位表依赖向后传播。backtrack 状态的唯一写入点（`phase_log`
//! 不触 backtrack，与插件同分责）。

use serde_json::Value;

use crate::write::persist::{
    entry_phase, eval_entries_mut, latest_entry_index, load_doc, mark_phase_stale, save,
};
use crate::write::phase_table::{phase_table, MAX_TEXT_CHARS};
use foundation::layout::Layout;

/// 回溯输入（allowed 随行走带——phase_next 缓存白名单，写面二次校验兜底）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacktrackInput {
    pub phase: String,
    pub to: String,
    pub reason: String,
    pub allowed: Vec<String>,
}

/// 回溯产出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacktrackOutcome {
    pub phase: String,
    pub target: String,
}

/// 回溯：校验（白名单 / 表位 / reason 长度）→ 最新条目标记 → stale 标记与
/// 传播 → 保形写回。workflow.json 原文之外的字段零触碰。
pub fn backtrack(
    layout: &Layout,
    change: &str,
    input: &BacktrackInput,
) -> Result<BacktrackOutcome, String> {
    let mut doc = load_doc(layout, change)?;
    let table = phase_table(&doc.typed.workflow_type).ok_or_else(|| {
        format!(
            "workflow_type \"{}\" 不受支持（V1 仅 requirement 工作流）",
            doc.typed.workflow_type
        )
    })?;

    // 白名单二次校验（越权 Err 不写）
    if !input.allowed.iter().any(|id| id == &input.to) {
        return Err(format!(
            "越权 backtrack：目标 \"{}\" 不在白名单内（allowed: {}）",
            input.to,
            input.allowed.join(", ")
        ));
    }
    // 表位校验（与插件 validatePhaseTarget 同语义：双端在表内、目标不超前）
    let phase_idx = table
        .iter()
        .position(|def| def.id == input.phase)
        .ok_or_else(|| {
            format!(
                "工作流 \"{}\" 不包含 phase \"{}\"",
                doc.typed.workflow_type, input.phase
            )
        })?;
    let target_idx = table
        .iter()
        .position(|def| def.id == input.to)
        .ok_or_else(|| {
            format!(
                "工作流 \"{}\" 不包含 phase \"{}\"",
                doc.typed.workflow_type, input.to
            )
        })?;
    if target_idx > phase_idx {
        return Err(format!(
            "无效的回溯目标 phase: \"{}\"。不支持回溯到未来 phase。",
            input.to
        ));
    }
    let reason_chars = input.reason.chars().count();
    if reason_chars > MAX_TEXT_CHARS {
        return Err(format!(
            "决策 reason 超长（{reason_chars} > {MAX_TEXT_CHARS}）"
        ));
    }

    // 最新条目标记 backtrack_to / backtrack_reason（raw 定点改写）
    {
        let entries = eval_entries_mut(&mut doc.raw);
        let latest = latest_entry_index(entries, |entry| {
            entry_phase(entry) == Some(input.phase.as_str())
        })
        .ok_or_else(|| {
            format!(
                "Phase \"{}\" 没有评估条目，无法设置回溯。请先执行该 phase 并记录评估结果。",
                input.phase
            )
        })?;
        if let Some(object) = entries[latest].as_object_mut() {
            object.insert("backtrack_to".to_owned(), Value::String(input.to.clone()));
            object.insert(
                "backtrack_reason".to_owned(),
                Value::String(input.reason.clone()),
            );
        }
    }

    // 目标相位最新 pass 条目 stale + 依赖向后传播（无 pass 条目 no-op）
    mark_phase_stale(&mut doc.raw, table, &input.to);
    save(&doc)?;
    Ok(BacktrackOutcome {
        phase: input.phase.clone(),
        target: input.to.clone(),
    })
}
