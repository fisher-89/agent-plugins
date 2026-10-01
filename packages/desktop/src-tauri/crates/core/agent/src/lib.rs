mod event;
mod kernel;
mod port;
mod runner;
mod session;
mod state;

pub use event::{AgentBlock, AgentDelta, AgentEvent, AgentEventKind};
pub use kernel::{KernelOutput, RunningTurn, SessionKernel, StopRegistry, TurnRequest};
pub use port::{SessionQuery, SessionSink, TurnOutcome};
pub use runner::{
    AgentPermissionMode, AgentRunStatus, AgentRunner, AgentSession, AgentStartError, RunHandle,
    SessionCtx, SessionInjections, SessionOpen, SessionRef, TurnQuestion,
};
pub use session::{
    new_session_id, NewSessionRow, SessionProvenance, SessionRow, SessionStats, SessionSummary,
    TurnSummary,
};
pub use state::{AgentRunState, RunStateMachine};

#[cfg(test)]
mod event_test;
#[cfg(test)]
mod kernel_test;
#[cfg(test)]
mod lib_test;
#[cfg(test)]
mod port_test;
#[cfg(test)]
mod runner_test;
#[cfg(test)]
mod session_test;
#[cfg(test)]
mod state_test;
