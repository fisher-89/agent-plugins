//! 基于磁盘事实的三代代际探测。

use std::fs;
use std::path::Path;

use serde_json::Value;

use super::WORKFLOW_FILE_NAME;
use crate::model::Inventory;

/// 判定 change 目录的数据代际：
///
/// - 有 `workflow.json` 且含 `file_log` 键 → V2
/// - 有 `workflow.json` 无 `file_log` 键 → V1
/// - 无 `workflow.json` → V0
///
/// 判定基于文件与字段存在性，不依赖目录命名约定；
/// workflow.json 存在但不可读或整体损坏时，按现役结构假设为 V2
/// （配合查询层的 unparsable 标记降级展示）。
pub fn detect_inventory(change_dir: &Path) -> Inventory {
    let workflow_path = change_dir.join(WORKFLOW_FILE_NAME);
    if !workflow_path.is_file() {
        return Inventory::V0;
    }
    let Ok(text) = fs::read_to_string(&workflow_path) else {
        return Inventory::V2;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Inventory::V2;
    };
    if value.get("file_log").is_some() {
        Inventory::V2
    } else {
        Inventory::V1
    }
}
