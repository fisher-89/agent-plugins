//! 编排 core crate：桌面 change 流程的薄图 walker 运行时。
//!
//! 模块边界：
//! - `state`：run 状态与步状态类型（IPC 直用，serde camelCase + specta Type）
//! - `verdict` / `decision`：evaluator checklist 与决策四动作解析器（漂移
//!   显式失败）
//! - `transcript`：密封转录提取器（最终 assistant 文本）
//! - `prompt`：executor / evaluator / 决策 prompt 组装（git diff 变更文件
//!   上下文段双入口）
//! - `port`：WorkerAgent / ToolStep / 快照 / 事件出口 + static-check / git
//!   diff 缝契约（双缝测试面）
//! - `steps`：ToolStepPort 的进程内实现（相位机四步直调 workflow::write 写面）
//! - `control`：run 控制注册表（进程内 per-change，命令层读写 + guard 写）
//! - `snapshot`：快照 port 的 fs 实现（ChangeDetail 只读装配）
//! - `walker`：相位循环主入口
//!
//! 纯度原则：零进程 spawn（static-check / git diff / 会话内核泵全部经 port
//! 落 infra）、零 workflow.json 直写（写触点唯一经 workflow::write 写面进程
//! 内直调）、图不持有转移规则。

pub mod control;
pub mod decision;
pub mod port;
pub mod prompt;
pub mod snapshot;
pub mod state;
pub mod steps;
pub mod transcript;
pub mod verdict;
pub mod walker;

pub use control::{ChangeFlowControl, RunGuard};
pub use decision::{CandidateReport, DecisionAction, DecisionInput};
pub use port::{
    BoxDiffFuture, BoxToolFuture, BoxTurnFuture, DiffContextPort, RunEventSink, StaticCheckOutcome,
    StaticCheckRunner, ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest, WorkerAgentPort,
    WorkerRole, WorkerTurnOutcome, WorkerTurnRequest, WorkflowSnapshotPort,
};
pub use snapshot::FsSnapshot;
pub use state::{
    AskPayload, ChangeRunSnapshot, ChangeRunStatus, ChangeRunSummary, ChangeStepKind,
    ChangeStepState, ChangeStepStatus, RunUpdate,
};
pub use steps::LocalToolSteps;
pub use verdict::EvaluatorChecklist;
pub use walker::{
    new_run_id, walk_run, RunRequest, STATIC_CHECK_FEEDBACK_LIMIT, STATIC_CHECK_PHASES,
};

#[cfg(test)]
mod control_test;
#[cfg(test)]
mod decision_test;
#[cfg(test)]
mod port_test;
#[cfg(test)]
mod prompt_test;
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
