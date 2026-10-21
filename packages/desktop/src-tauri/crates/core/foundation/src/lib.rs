//! 共享地基 crate：磁盘布局解析 + workspace 身份段派生 + 依赖与工具产物树
//! 名单 + 取消原语四能力。
//!
//! 刻意保持极小——不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具
//!（身份段派生自 store 下沉、产物树名单自 checks 下沉、取消原语自编排 /
//! exec 两消费者下沉，均属「第二消费者出现才下沉」的正触发）。
//!
//! 取消原语（[`cancel`]）为 std-only（`Arc` + `AtomicBool`）——本 crate 的
//! 依赖面仍仅有 `sha2`，无任何 workspace 内 crate、无 tokio；两消费者为编排
//! 侧 `RunGuard`（写）与执行体 `exec::run_shell`（读）。

pub mod cancel;
pub mod identity;
pub mod layout;
pub mod prune;

#[cfg(test)]
mod cancel_test;
#[cfg(test)]
mod identity_test;
#[cfg(test)]
mod prune_test;
