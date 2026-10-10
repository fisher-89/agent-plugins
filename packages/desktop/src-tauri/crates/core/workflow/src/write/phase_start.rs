use crate::state::{ChangeStateStore, PhaseStartState};
use crate::write::phase_table::phase_table;

/// 开启阶段产出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseStartOutcome {
    pub phase: String,
    pub attempt: u32,
    /// 开相时刻（UTC unix 毫秒，写事务内铸出）
    pub start_at: i64,
}

/// 开启阶段（按 change **id** 寻址）：`active_phase` 定点写入。phase 必须属于
/// 该 change workflow_type 的相位表（非法 phase 显式 `Err`，状态库不动）；
/// attempt 在写事务内推导，持久化经 [`ChangeStateStore`] port 缝落库。
pub fn phase_start(
    store: &dyn ChangeStateStore,
    change_id: &str,
    phase: &str,
) -> Result<PhaseStartOutcome, String> {
    let record = store
        .get_change(change_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从开相"))?;
    let table = phase_table(&record.workflow_type).ok_or_else(|| {
        format!(
            "workflow_type \"{}\" 不受支持（V1 仅 requirement 工作流）",
            record.workflow_type
        )
    })?;
    if !table.iter().any(|def| def.id == phase) {
        return Err(format!(
            "phase \"{phase}\" 不属于 workflow_type \"{}\" 的 phase 表 (change: {})，\
             active_phase 未写入。",
            record.workflow_type, record.name
        ));
    }

    let PhaseStartState { attempt, start_at } = store
        .start_phase(change_id, phase, now_millis())
        .map_err(|error| error.to_string())?;
    Ok(PhaseStartOutcome {
        phase: phase.to_owned(),
        attempt,
        start_at,
    })
}

/// 当前 UTC unix 毫秒（时钟早于 epoch 取 0，不 panic）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
