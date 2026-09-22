//! 领域类型模块根。

pub mod inventory;
pub mod workflow;

pub use inventory::Inventory;
pub use workflow::{
    ActivePhase, ChecklistItem, FileLogEntry, FileLogOp, InterruptedEntry, PhaseLog, Verdict,
    Workflow,
};
