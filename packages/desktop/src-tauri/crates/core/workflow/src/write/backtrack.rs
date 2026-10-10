use std::collections::HashSet;

use crate::state::{BacktrackCommand, ChangeStateStore};
use crate::write::phase_table::{dependents, phase_table, PhaseDefinition, MAX_REASON_CHARS};

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

/// 回溯（按 change **id** 寻址）：校验（白名单 / 双端表位 / reason 长度）→
/// 最新条目在位校验 → stale 闭包计算（目标最新 pass + `dependents` BFS 全条目，
/// 自 persist 迁入本文件）→ 经 [`ChangeStateStore`] port 缝单事务落库（回跳
/// 标记 + stale 翻转传播原子完成）。
pub fn backtrack(
    store: &dyn ChangeStateStore,
    change_id: &str,
    input: &BacktrackInput,
) -> Result<BacktrackOutcome, String> {
    let record = store
        .get_change(change_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从回溯"))?;
    let table = phase_table(&record.workflow_type).ok_or_else(|| {
        format!(
            "workflow_type \"{}\" 不受支持（V1 仅 requirement 工作流）",
            record.workflow_type
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
                record.workflow_type, input.phase
            )
        })?;
    let target_idx = table
        .iter()
        .position(|def| def.id == input.to)
        .ok_or_else(|| {
            format!(
                "工作流 \"{}\" 不包含 phase \"{}\"",
                record.workflow_type, input.to
            )
        })?;
    if target_idx > phase_idx {
        return Err(format!(
            "无效的回溯目标 phase: \"{}\"。不支持回溯到未来 phase。",
            input.to
        ));
    }
    let reason_chars = input.reason.chars().count();
    if reason_chars > MAX_REASON_CHARS {
        return Err(format!(
            "决策 reason 超长（{reason_chars} > {MAX_REASON_CHARS}）"
        ));
    }

    // 最新条目在位校验（发起相位无评估条目不可回溯——语义与既往一致）
    let entries = store
        .list_phase_records(change_id)
        .map_err(|error| error.to_string())?;
    if !entries.iter().any(|entry| entry.phase == input.phase) {
        return Err(format!(
            "Phase \"{}\" 没有评估条目，无法设置回溯。请先执行该 phase 并记录评估结果。",
            input.phase
        ));
    }

    store
        .apply_backtrack(&BacktrackCommand {
            change_id: change_id.to_owned(),
            phase: input.phase.clone(),
            to: input.to.clone(),
            reason: input.reason.clone(),
            stale_dependents: stale_closure(table, &input.to),
        })
        .map_err(|error| error.to_string())?;
    Ok(BacktrackOutcome {
        phase: input.phase.clone(),
        target: input.to.clone(),
    })
}

/// stale 传播闭包：目标相位的全部下游相位（`dependents` BFS；依赖图为 DAG，
/// visited 集防御式保留，与插件 `propagateStale` 一致——目标自身不在闭包内，
/// 其「最新 pass 置 stale」由 store 落账半边单独承接）。
fn stale_closure(table: &[PhaseDefinition], target: &str) -> Vec<String> {
    let mut visited = HashSet::from([target.to_owned()]);
    let mut closure = Vec::new();
    let mut queue = dependents(table, target);
    while let Some(phase) = queue.pop() {
        if !visited.insert(phase.clone()) {
            continue;
        }
        closure.push(phase.clone());
        queue.extend(dependents(table, &phase));
    }
    closure
}
