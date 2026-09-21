//! 共享地基 crate：今天仅含磁盘布局解析。
//!
//! 刻意保持极小——不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具。

pub mod layout;

#[cfg(test)]
mod layout_test;
