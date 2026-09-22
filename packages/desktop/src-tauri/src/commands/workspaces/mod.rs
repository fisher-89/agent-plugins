//! workspace 注册命令轨道（shell 记忆，非 change 域查询）：四命令薄包装——
//! `State<'_, Store>` 取 store + 参数转换（`String` → `Path`）+ DTO 返回。
//! 自身不直接操作 redb 或 db 文件，store 层 `StoreError` 不进入命令签名。
//!
//! 本轨道确立后续可失败命令的错误约定模板：命令返回 `Result<T, String>`，
//! `Err` 由 Tauri 转为前端 reject，前端 hook 以 error 态接住呈现；
//! MUST NOT 静默吞掉 db 打开或读写失败。

use std::path::Path;

use tauri::State;

use store::{Store, StoreError, WorkspaceRecord};

/// 清单（`last_opened_at` 降序，第一名即最近打开）。
#[tauri::command]
pub fn list_workspaces(store: State<'_, Store>) -> Result<Vec<WorkspaceRecord>, String> {
    list_workspaces_inner(&store).map_err(|e| e.to_string())
}

fn list_workspaces_inner(store: &Store) -> Result<Vec<WorkspaceRecord>, StoreError> {
    store.list_workspaces()
}

/// 文件夹选择器选定后入库：canonicalize + upsert + touch，返回 canonical 记录。
#[tauri::command]
pub fn add_workspace(store: State<'_, Store>, root: String) -> Result<WorkspaceRecord, String> {
    add_workspace_inner(&store, Path::new(&root)).map_err(|e| e.to_string())
}

fn add_workspace_inner(store: &Store, root: &Path) -> Result<WorkspaceRecord, StoreError> {
    store.add_workspace(root)
}

/// 移除清单项；返回是否命中（miss 幂等，不算错误）。
#[tauri::command]
pub fn remove_workspace(store: State<'_, Store>, root: String) -> Result<bool, String> {
    remove_workspace_inner(&store, Path::new(&root)).map_err(|e| e.to_string())
}

fn remove_workspace_inner(store: &Store, root: &Path) -> Result<bool, StoreError> {
    store.remove_workspace(root)
}

/// 选中即 touch（自动恢复 / 下拉切换 / 添加后打开）；返回是否命中。
#[tauri::command]
pub fn touch_workspace(store: State<'_, Store>, root: String) -> Result<bool, String> {
    touch_workspace_inner(&store, Path::new(&root)).map_err(|e| e.to_string())
}

fn touch_workspace_inner(store: &Store, root: &Path) -> Result<bool, StoreError> {
    store.touch_workspace(root)
}

#[cfg(test)]
mod mod_test;
