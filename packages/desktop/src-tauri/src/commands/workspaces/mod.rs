//! workspace 注册命令轨道（shell 记忆，非 change 域查询）：三命令薄包装——
//! `State<'_, WorkspaceStores>` 取注册表 + 参数转换（`String` → `Path`）+
//! DTO 返回。注册表操作走**全局库**（`global()`，user 维度注册表，不误路由
//! workspace 库）；`add_workspace` 注册成功后预开对应 workspace 库（坏文件
//! 注册时即 `Err` 暴露，fail fast 口径同 setup）。自身不直接操作 redb 或 db
//! 文件，store 层 `StoreError` 不进入命令签名。
//!
//! 本轨道确立后续可失败命令的错误约定模板：命令返回 `Result<T, String>`，
//! `Err` 由 Tauri 转为前端 reject，前端 hook 以 error 态接住呈现；
//! MUST NOT 静默吞掉 db 打开或读写失败。

use std::path::Path;

use tauri::State;

use store::{StoreError, WorkspaceRecord, WorkspaceStores};

/// 清单（表主键 canonical root 自然序，顺序与使用时间无关）。
#[tauri::command]
#[specta::specta]
pub fn list_workspaces(stores: State<'_, WorkspaceStores>) -> Result<Vec<WorkspaceRecord>, String> {
    list_workspaces_inner(&stores).map_err(|e| e.to_string())
}

fn list_workspaces_inner(stores: &WorkspaceStores) -> Result<Vec<WorkspaceRecord>, StoreError> {
    stores.global().list_workspaces()
}

/// 文件夹选择器选定后入库：canonicalize + upsert，返回 canonical 记录；注册
/// 成功后预开对应 workspace 库（坏文件注册时 `Err`，注册记录保留——重加同
/// root 时 upsert 幂等并再次校验）。
#[tauri::command]
#[specta::specta]
pub fn add_workspace(
    stores: State<'_, WorkspaceStores>,
    root: String,
) -> Result<WorkspaceRecord, String> {
    add_workspace_inner(&stores, Path::new(&root)).map_err(|e| e.to_string())
}

fn add_workspace_inner(
    stores: &WorkspaceStores,
    root: &Path,
) -> Result<WorkspaceRecord, StoreError> {
    let record = stores.global().add_workspace(root)?;
    stores.for_root(&record.root)?;
    Ok(record)
}

/// 移除清单项；返回是否命中（miss 幂等，不算错误）。仅删注册记录——workspace
/// db 文件与缓存实例保留（移除注册 ≠ 销毁历史，重加同 root 历史完整恢复）。
#[tauri::command]
#[specta::specta]
pub fn remove_workspace(stores: State<'_, WorkspaceStores>, root: String) -> Result<bool, String> {
    remove_workspace_inner(&stores, Path::new(&root)).map_err(|e| e.to_string())
}

fn remove_workspace_inner(stores: &WorkspaceStores, root: &Path) -> Result<bool, StoreError> {
    stores.global().remove_workspace(root)
}

#[cfg(test)]
mod mod_test;
