//! `agent`：agent 边界中立契约 crate（五类边界之一，core 侧）。
//!
//! 定义 agent 域的三租户共享契约：
//! - [`AgentEvent`] 事件信封（五变体 + 块模型 + seq/时间戳 + Raw 透传）
//! - [`AgentRunner`] trait 与 [`AgentRunParams`]（逻辑事件流 + 句柄，进程模型不可见）
//! - [`RunStateMachine`] run 状态机（running → completed/failed）
//!
//! 本 crate 刻意保持中立：零 Tauri、零进程 spawn、不认识任何具体 CLI
//! （flag 名与 JSONL 行解析全部关在 infra 侧实现 crate）。三租户（本机 CLI /
//! 进程内 SDK / 远程 API）都应落在 [`AgentRunner`] 预留内；MVP 仅实现 CLI
//! 租户，其余 MUST NOT 预建。
//!
//! 能力 spec：`specs/desktop-agent-execution/spec.md`（路径相对域根）。

mod event;
mod runner;
mod state;

pub use event::{AgentBlock, AgentEvent, AgentEventKind};
pub use runner::{
    AgentEnvMode, AgentPermissionMode, AgentRun, AgentRunParams, AgentRunner, AgentStartError,
    RunHandle,
};
pub use state::{AgentRunState, RunStateMachine};

#[cfg(test)]
mod event_test;
#[cfg(test)]
mod runner_test;
#[cfg(test)]
mod state_test;
