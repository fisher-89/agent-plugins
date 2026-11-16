//! 架构漂移门禁执行面（crate 裸名 `archi-runtime`）：只做「取输入 → 调
//! [`archi::drift::check`] 判定 → 落 `reports/archi` 报告 → 派生 outcome」，并与报告
//! 落盘并列承载**生成物写盘单点**（[`api_artifacts::write_artifacts`]，CLI 与两条
//! 契约面 IPC 同源）。两处写盘都只是 dumb IO，零判定。
//!
//! **零第二套判定**：归属匹配、定级分支与 `status` 推导全部在 `crates/core/archi`
//! （判定单源）；本层零 `owner_of` / 零 `ArchiDriftKind` 分支。
//!
//! 边界：零 Tauri / 零 spawn（无 `std::process` / `tokio::process` / shell）/ 零
//! vcs（变更集经 port 注入）/ 零 store / 零 config / 零 workflow。

// 生成类型与自动构造器由契约派发，落**实现入口同级**（本文件同目录的 `lib.i.rs`）。
// `#[path]` 的值是**字面量**（Rust 属性值不能由函数求值）；入口同级后该值**恒为文件名
// （零 `../`）**——等值性由 `paths_test` 断言「本行字面量 == `paths::generated_path(...)`
// 的文件名」，MUST NOT 手写带目录段的形态。
#[path = "lib.i.rs"]
mod api_gen;
pub use api_gen::*;

mod report;
mod runner;

pub mod api_artifacts;

#[cfg(test)]
mod api_artifacts_test;

#[cfg(test)]
mod report_test;
#[cfg(test)]
mod runner_test;
