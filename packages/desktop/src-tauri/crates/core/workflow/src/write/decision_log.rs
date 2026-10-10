//! 决策会话槽位挂账写操作（amend 语义）：经
//! [`ChangeStateStore::amend_decision_session`] 对该相位最新 PhaseRecord 定点
//! 改写 `decision_session_id` 列（幂等覆写，无条目 `StoreFault::NotFound`），
//! 不做表位校验（不新增条目、不触相位路由语义）。槽位值由调用方透传，本写
//! 面不解释不改写；decision 会话产生于 fail 条目落账之后，故走本显式挂账操
//! 作而非 `phase_log` 随行（design D9）。

use crate::state::ChangeStateStore;

/// 挂账产出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionLogOutcome {
    pub phase: String,
}

/// 决策会话槽位挂账（按 change **id** 寻址）：该相位最新评估条目定点改写
/// `decision_session_id`（幂等覆写，无条目显式 `Err`）。
pub fn decision_log(
    store: &dyn ChangeStateStore,
    change_id: &str,
    phase: &str,
    session_id: &str,
) -> Result<DecisionLogOutcome, String> {
    store
        .amend_decision_session(change_id, phase, session_id)
        .map_err(|error| error.to_string())?;
    Ok(DecisionLogOutcome {
        phase: phase.to_owned(),
    })
}
