//! 架构漂移门禁执行面（crate 裸名 `archi-runtime`）：只做「取输入 → 调
//! [`archi::drift::check`] 判定 → 落 `reports/archi` 报告 → 派生 outcome」。
//!
//! **零第二套判定**：归属匹配、定级分支与 `status` 推导全部在 `crates/core/archi`
//! （判定单源）；本层零 `owner_of` / 零 `ArchiDriftKind` 分支。
//!
//! 边界：零 Tauri / 零 spawn（无 `std::process` / `tokio::process` / shell）/ 零
//! vcs（变更集经 port 注入）/ 零 store / 零 config / 零 workflow。

mod report;
mod runner;

pub use runner::LocalArchiCheck;

#[cfg(test)]
mod report_test;
#[cfg(test)]
mod runner_test;
