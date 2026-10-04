//! change 工作流写面：相位机四操作（phase-next 路由 / phase-start 开相位 /
//! phase-log 评估落账 / backtrack 回溯）+ 决策会话槽位挂账（decision-log
//! amend）+ requirement 相位表单源。sync 零 tokio；workflow.json 写触点全
//! 部收敛本面（进程内直调，AC-6 写通道唯一），经 crate 内私有持久层 raw
//! Value 保形定点改写（W2），schema 形状零新字段（W3：校验 / 写出实现权
//! 威自 zod 移交 serde；唯一例外 = eval 条目会话槽位字段
//!（desktop-change-session-visibility 裁定），显式在位才写）。

mod backtrack;
mod create;
mod decision_log;
mod persist;
mod phase_log;
mod phase_next;
mod phase_start;
mod phase_table;

pub use backtrack::{backtrack, BacktrackInput, BacktrackOutcome};
pub use create::{create, CreateOutcome};
pub use decision_log::{decision_log, DecisionLogOutcome};
pub use phase_log::{phase_log, PhaseLogInput, PhaseLogOutcome};
pub use phase_next::{phase_next, LastResult, PhaseNextError, PhaseNextOutcome, SessionAnchors};
pub use phase_start::{phase_start, PhaseStartOutcome};
pub use phase_table::{
    allowed_backtrack_phases, interpolate, phase_table, PhaseAgentSpec, PhaseDefinition,
    MAX_RETRY_TIMES,
};

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
