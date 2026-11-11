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

/// verdict 推导全局单点：checklist 全 pass → pass，否则 fail（空 checklist →
/// pass，与插件 `resolveVerdict` 的 every 语义一致）。落账路径（`phase_log`）
/// 与编排侧 `finish_step` 校验器共用本实现——MUST NOT 出现第二份。
pub fn derive_verdict(checklist: &[ChecklistItem]) -> Verdict {
    if checklist.iter().all(|item| item.pass) {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}
