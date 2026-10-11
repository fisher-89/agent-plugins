mod archive;
mod backtrack;
mod create;
mod decision_log;
mod phase_log;
mod phase_next;
mod phase_start;
mod phase_table;
mod promote;
mod run;
mod worktree;

pub use archive::{archive, ArchiveOutcome};
pub use backtrack::{backtrack, BacktrackInput, BacktrackOutcome};
pub(crate) use create::utc_date;
pub use create::{create, CreateOutcome};
pub use decision_log::{decision_log, DecisionLogOutcome};
pub use phase_log::{phase_log, PhaseLogInput, PhaseLogOutcome};
pub use phase_next::{
    phase_next, LastResult, PhaseNextError, PhaseNextOutcome, ResolvedPhaseSpec, SessionAnchors,
};
pub use phase_start::{phase_start, PhaseStartOutcome};
pub use phase_table::{
    allowed_backtrack_phases, phase_table, PhaseAgentSpec, PhaseDefinition, MAX_RETRY_TIMES,
};
pub use promote::{promote_explore, PromoteOutcome};
pub use run::{run_finish, run_start};
pub use worktree::{InstallRun, RepoProbe, WorktreePort};

/// 计划图侧类型再导出（phase 定义声明面）：`StepDefinition` / `StepNode` /
/// `StepGroup` / `AgentRole` / `OnFail` / `ArtifactSpec` 本体驻 `crate::state`
///（词汇单点），本出口面供编排 / 查询 / 命令层消费。
pub use crate::state::{AgentRole, ArtifactSpec, OnFail, StepDefinition, StepGroup, StepNode};

/// 执行图整包载荷再导出（run 收口写面）：`RunNodeEntry` / `RunEdgeEntry` 与
/// 其状态快照对应项同驻 `crate::state`。
pub use crate::state::{
    RunEdgeEntry, RunEdgeStateRecord, RunFinishCommand, RunNodeEntry, RunNodeStateRecord,
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
#[cfg(test)]
mod promote_test;
#[cfg(test)]
mod run_test;
