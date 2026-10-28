pub mod control;
pub mod decision;
pub mod port;
pub mod prompt;
pub mod run_history;
pub mod snapshot;
pub mod state;
pub mod steps;
pub mod transcript;
pub mod verdict;
pub mod walker;

pub use control::{ChangeFlowControl, RunGuard};
pub use decision::{CandidateReport, DecisionAction, DecisionInput};
pub use port::{
    ArchiCheckOutcome, ArchiCheckRunner, ArchiveVcsPort, BoxDiffFuture, BoxToolFuture,
    BoxTurnFuture, DiffContextPort, RunEventSink, RunHistoryPort, StaticCheckOutcome,
    StaticCheckRunner, ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest, WorkerAgentPort,
    WorkerRole, WorkerTurnOutcome, WorkerTurnRequest, WorkflowSnapshotPort,
};
pub use run_history::{finish_command, StoreRunHistory};
pub use snapshot::StoreSnapshot;
pub use state::{
    ArchivePreflight, AskPayload, ChangeRunSnapshot, ChangeRunStatus, ChangeRunSummary,
    ChangeStepEdge, ChangeStepKind, ChangeStepState, ChangeStepStatus, EdgeKind, NodeKey,
    RunNotice, RunUpdate,
};
pub use steps::LocalToolSteps;
pub use verdict::EvaluatorChecklist;
pub use walker::{new_run_id, walk_run, RunRequest};

#[cfg(test)]
mod control_test;
#[cfg(test)]
mod decision_test;
#[cfg(test)]
mod port_test;
#[cfg(test)]
mod prompt_test;
#[cfg(test)]
mod run_history_test;
#[cfg(test)]
mod snapshot_test;
#[cfg(test)]
mod state_test;
#[cfg(test)]
mod steps_test;
#[cfg(test)]
mod transcript_test;
#[cfg(test)]
mod verdict_test;
#[cfg(test)]
mod walker_test;
