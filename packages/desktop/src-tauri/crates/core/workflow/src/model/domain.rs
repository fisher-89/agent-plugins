//! change 流程域类型单点：`Verdict` / `ChecklistItem` 自退役的磁盘模型
//! （`model/workflow.rs`，随双向墙删除）迁入，形状与导出面不变——`state.rs`
//! 中性状态与 queries 线面 DTO 的既有消费词汇。

use serde::{Deserialize, Serialize};
use specta::Type;

/// 评估 verdict。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Pass,
    Fail,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
        }
    }
}

/// checklist 检查项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ChecklistItem {
    pub item: String,
    pub pass: bool,
    pub evidence: String,
}
