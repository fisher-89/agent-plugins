//! `cli` 引擎：本机 claude CLI 租户（本 workspace 唯一 spawn 触点）。
//!
//! 四模块自 `src/` 平移落位（移动非删除，内容零改动）：
//! - [`discover`]：CLI 可执行入口发现（Windows `.cmd` shim 优先序）
//! - [`flags`]：参数 → 命令行 flag 序列组装（纯函数）
//! - [`jsonl`]：stdout JSONL 逐行归一化为 [`agent::AgentEventKind`]（未知
//!   type / 非 JSON 行 Raw 透传：永不丢事件、永不炸解析）
//! - [`runner`]：spawn + stdout 逐行泵 + 有界事件流 + EOF 无 result 补发
//!   合成收敛事件
//!
//! 对外形状经门面 `lib.rs` 以 `cli` 前缀 re-export（`discover` / `build_args`
//! / `normalize_line` / [`crate::ClaudeCliRunner`]），引擎细节不出门面。

pub mod discover;
pub mod flags;
pub mod jsonl;
pub mod runner;

#[cfg(test)]
mod discover_test;
#[cfg(test)]
mod flags_test;
#[cfg(test)]
mod jsonl_test;
#[cfg(test)]
mod runner_test;
