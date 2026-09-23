//! `agent-cli`：agent 边界的本机 CLI 租户实现（infra 侧，唯一 spawn 触点）。
//!
//! 实现 [`agent::AgentRunner`] 契约，具体职责：
//! - [`discover`]：CLI 可执行入口发现（Windows `.cmd` shim 优先序）
//! - [`flags`]：参数 → 命令行 flag 序列组装（纯函数）
//! - [`jsonl`]：stdout JSONL 逐行归一化为 [`agent::AgentEventKind`]（未知
//!   type / 非 JSON 行 Raw 透传：永不丢事件、永不炸解析）
//! - [`ClaudeCliRunner`]：spawn + stdout 逐行泵 + 有界事件流 + EOF 无 result
//!   补发合成收敛事件
//!
//! 本 crate 禁 Tauri 系依赖；进程 spawn 是 agent 边界自身的实现细节，
//! 不属于 shell 边界职责。
//!
//! 能力 spec：`specs/desktop-agent-execution/spec.md`（路径相对域根）。

mod discover;
mod flags;
mod jsonl;
mod runner;

pub use discover::{discover, discover_in};
pub use flags::build_args;
pub use jsonl::normalize_line;
pub use runner::ClaudeCliRunner;

#[cfg(test)]
mod discover_test;
#[cfg(test)]
mod flags_test;
#[cfg(test)]
mod jsonl_test;
#[cfg(test)]
mod runner_test;
