//! 三个查询命令：均为无状态薄包装——参数 → resolve → core 函数 → DTO。
//! 无 State、不持有或缓存 workspace 状态、无直接文件系统访问。
//!
//! IPC 字符串入参的显式格式/包含性检查口径：
//! - `root`：空/空白视作空 workspace（返回空结果而非报错），见 [`is_blank_root`]；
//! - `change`：单分量目录名（拒绝路径穿越），由 core `locate_change` 强制；
//! - `source`：change 内相对 POSIX 路径或 eval 序号串（拒绝 `..`/绝对路径/
//!   反斜杠/盘符），由 core `read_artifact` 强制，另含 canonical 包含性兜底；
//! - `kind`：非空，且仅与静态注册表精确比对，由 core `read_artifact` 强制。

use std::path::Path;

use foundation::layout::resolve;
use workflow::artifacts::{read_artifact as core_read_artifact, ArtifactEnvelope};
use workflow::model::Workflow;
use workflow::parse::{detect_inventory, load_workflow};
use workflow::queries::{self, locate_change, ChangeDetail, ChangeList};

/// root 显式格式检查：空/空白串不进入查询链路，直接给出空结果语义。
fn is_blank_root(root: &str) -> bool {
    root.trim().is_empty()
}

/// change 列表（active + archive 按月分组）。
#[tauri::command]
pub fn list_changes(root: String) -> ChangeList {
    if is_blank_root(&root) {
        return ChangeList {
            active: Vec::new(),
            archive_groups: Vec::new(),
        };
    }
    let layout = resolve(Path::new(&root));
    queries::list_changes(&layout)
}

/// 单 change 详情聚合；未知 change 名返回 `None`。
#[tauri::command]
pub fn get_change_detail(root: String, change: String) -> Option<ChangeDetail> {
    if is_blank_root(&root) {
        return None;
    }
    let layout = resolve(Path::new(&root));
    queries::change_detail(&layout, &change)
}

/// 按信封读取单个产物；kind 未注册、source 非法或解析失败返回 `None`。
#[tauri::command]
pub fn read_artifact(
    root: String,
    change: String,
    kind: String,
    source: String,
) -> Option<ArtifactEnvelope> {
    if is_blank_root(&root) {
        return None;
    }
    let layout = resolve(Path::new(&root));
    let location = locate_change(&layout, &change)?;
    let inventory = detect_inventory(&location.dir);
    let workflow: Option<Workflow> = load_workflow(&location.dir);
    core_read_artifact(&location.dir, inventory, workflow.as_ref(), &kind, &source)
}

#[cfg(test)]
mod mod_test;
