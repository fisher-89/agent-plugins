//! change 域命令组（读 + 记录面）：四条命令——三读（change 列表 / 详情
//! 聚合 / 产物信封读取）+ 一记录面（新建建档）；change 域二分职责——本组
//! （读 + 记录面，沿 explores 组同组先例）/ `change_flow`（run 编排控制）。
//!
//! 读命令均为无状态薄包装——参数 → resolve → core 函数 → DTO；无 State、
//! 不持有或缓存 workspace 状态、无直接文件系统访问。记录面三件事纪律：
//! 参数转换 → 调写面 → 错误映射；name / goal 校验权威在写面，命令层不过
//! 关。全组 sync 纯函数命令（无 State / AppHandle / Channel，无需
//! `_with` 测试缝）。
//!
//! 组内 blank root 双口径并存且 MUST NOT 互换：读命令空/空白 root 早退
//! 空结果语义（空列表 / `None`，见 [`is_blank_root`]）；`create_change`
//! 显式 `Err`（写无空结果语义，不进入写面链路）。
//!
//! IPC 字符串入参的显式格式/包含性检查口径（读命令）：
//! - `root`：空/空白视作空 workspace（返回空结果而非报错），见 [`is_blank_root`]；
//! - `change`：单分量目录名（拒绝路径穿越），由 core `locate_change` 强制；
//! - `source`：change 内相对 POSIX 路径或 eval 序号串（拒绝 `..`/绝对路径/
//!   反斜杠/盘符），由 core `read_artifact` 强制，另含 canonical 包含性兜底；
//! - `kind`：非空，且仅与静态注册表精确比对，由 core `read_artifact` 强制。
//!
//! 能力 spec：`specs/desktop-app-shell/spec.md`、
//! `specs/desktop-change-create/spec.md`（路径相对域根）。

use std::path::Path;

use foundation::layout::resolve;
use workflow::artifacts::{read_artifact as core_read_artifact, ArtifactEnvelope};
use workflow::model::Workflow;
use workflow::parse::{detect_inventory, load_workflow};
use workflow::queries::{self, locate_change, ChangeDetail, ChangeList};
use workflow::write::{self, CreateOutcome};

#[cfg(test)]
mod mod_test;

/// root 显式格式检查：空/空白串不进入查询链路，直接给出空结果语义。
fn is_blank_root(root: &str) -> bool {
    root.trim().is_empty()
}

/// change 列表（active + archive 按月分组）。
#[tauri::command]
#[specta::specta]
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
#[specta::specta]
pub fn get_change_detail(root: String, change: String) -> Option<ChangeDetail> {
    if is_blank_root(&root) {
        return None;
    }
    let layout = resolve(Path::new(&root));
    queries::change_detail(&layout, &change)
}

/// 按信封读取单个产物；kind 未注册、source 非法或解析失败返回 `None`。
#[tauri::command]
#[specta::specta]
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

/// 新建 change：目录建树、workflow.json 初始文档与 explore.md（落最初
/// goal）写出均在写面 `create`；blank root 显式 `Err`。
#[tauri::command]
#[specta::specta]
pub fn create_change(root: String, name: String, goal: String) -> Result<CreateOutcome, String> {
    if root.trim().is_empty() {
        return Err("非法 root: 不得为空白".to_owned());
    }
    let layout = resolve(Path::new(&root));
    write::create(&layout, &name, &goal)
}
