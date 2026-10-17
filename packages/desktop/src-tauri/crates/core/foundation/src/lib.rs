//! 共享地基 crate：磁盘布局解析 + workspace 身份段派生 + 依赖与工具产物树
//! 名单三能力。
//!
//! 刻意保持极小——不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具
//!（身份段派生自 store 下沉、产物树名单自 checks 下沉，均属「第二消费者
//! 出现才下沉」的正触发）。

pub mod identity;
pub mod layout;
pub mod prune;

#[cfg(test)]
mod identity_test;
#[cfg(test)]
mod prune_test;
