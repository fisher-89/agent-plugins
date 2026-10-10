//! 模块边界：
//! - `model`：领域类型（`Verdict` / `ChecklistItem` 迁出单点）
//! - `state`：change 流程状态缝（port trait + 中性状态类型）
//! - `queries`：列表 / 详情聚合（纯读）
//! - `artifacts`：ArtifactEnvelope 信封 + matcher/parser 静态注册表 + 三插件
//! - `write`：change 状态写面（相位表单源 + 相位机操作，sync 零 tokio；落库
//!   经 `state::ChangeStateStore` port 缝，双向墙——零 workflow.json 触点）

pub mod artifacts;
pub mod model;
pub mod queries;
pub mod state;
pub mod write;

#[cfg(test)]
mod state_test;
