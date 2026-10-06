mod archive;
mod backtrack;
mod create;
mod decision_log;
mod phase_log;
mod phase_next;
mod phase_start;
mod phase_table;

pub use archive::{archive, ArchiveOutcome};
pub use backtrack::{backtrack, BacktrackInput, BacktrackOutcome};
pub use create::{create, CreateOutcome};
pub(crate) use create::utc_date;
pub use decision_log::{decision_log, DecisionLogOutcome};
pub use phase_log::{phase_log, PhaseLogInput, PhaseLogOutcome};
pub use phase_next::{phase_next, LastResult, PhaseNextError, PhaseNextOutcome, SessionAnchors};
pub use phase_start::{phase_start, PhaseStartOutcome};
pub use phase_table::{
    allowed_backtrack_phases, interpolate, phase_table, PhaseAgentSpec, PhaseDefinition,
    MAX_RETRY_TIMES,
};

/// 当前 UTC unix 毫秒（写面时钟单点；时钟早于 epoch 取 0，不 panic）。
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod archive_test;
#[cfg(test)]
mod backtrack_test;
#[cfg(test)]
mod create_test;
#[cfg(test)]
mod decision_log_test;
#[cfg(test)]
mod phase_log_test;
#[cfg(test)]
mod phase_next_test;
#[cfg(test)]
mod phase_start_test;
#[cfg(test)]
mod phase_table_test;
