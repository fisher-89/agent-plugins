//! change 目录的数据代际标注。

use serde::{Deserialize, Serialize};

/// 三代结构代际：v2（现在）/ v1（中期）/ v0（早期）。
///
/// 判定规则见 `crate::parse::detect_inventory`，这里只承载结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Inventory {
    /// 有 workflow.json 且含 file_log
    V2,
    /// 有 workflow.json 无 file_log
    V1,
    /// 无 workflow.json，仅 markdown 产物
    V0,
}
