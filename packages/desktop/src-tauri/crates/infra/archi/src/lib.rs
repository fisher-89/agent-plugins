//! 架构漂移门禁执行面（crate 裸名 `archi-runtime`）：只做「取输入 → 调
//! [`archi::drift::check`] 判定 → 落 `reports/archi` 报告 → 派生 outcome」。
//!
//! **零第二套判定**：归属匹配、定级分支与 `status` 推导全部在 `crates/core/archi`
//! （判定单源）；本层零 `owner_of` / 零 `ArchiDriftKind` 分支。
//!
//! 边界：零 Tauri / 零 spawn（无 `std::process` / `tokio::process` / shell）/ 零
//! vcs（变更集经 port 注入）/ 零 store / 零 config / 零 workflow。

// 生成类型与自动构造器：`<root>.i.rs` 由契约派发，落元素根同级。
// `#[path]` 的值是**字面量**（Rust 属性值不能由函数求值）；
// 本行的字面量与 `archi::api::paths::mod_path_attr(EntryForm::Crate, "archi")` 输出等值——
// 等值性由 `paths_test` 的三形态断言与 tasks 的逐入口自查行双守（逐字相等），MUST NOT
// 手写与函数输出不等值的形态。`mod_path_attr` 因此零产品侧调用者，这不是缺陷：它是这份
// 真值的唯一来源。
#[path = "../../archi.i.rs"]
mod api_gen;
pub use api_gen::*;

mod report;
mod runner;

#[cfg(test)]
mod report_test;
#[cfg(test)]
mod runner_test;
