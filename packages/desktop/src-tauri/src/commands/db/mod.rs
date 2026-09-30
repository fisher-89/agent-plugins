//! db 查看命令轨道（native-db-store-upgrade 开通）：只读 db 检查命令，经
//! `State<'_, WorkspaceStores>` 按 scope 解析对应库实例（`User` 全局库 /
//! `Workspace` 当前 workspace 库）后调用 store 的记录信封 API（`list_models`
//! / `scan`），无状态薄包装——参数转换 + 信封 API 调用 + DTO 返回。两库清单
//! 与扫描互不混列（维度由实例锁定）；信封 API 与零模型特定代码不变。
//!
//! 轨道纪律：只读，MUST NOT 出现任何写命令（写操作待真实 debug 需求出现后
//! 由后续变更裁定）。错误约定沿 workspaces 轨道模板：命令返回
//! `Result<T, String>`，`Err` 由 Tauri 转为前端 reject，MUST NOT 静默吞掉
//! 失败（workspace 库打开失败同样 reject，前端 inline 持久呈现）。

use tauri::State;

use store::{DbDimension, ModelInfo, RecordEnvelope, WorkspaceStores};

#[cfg(test)]
mod mod_test;

/// root 显式格式检查：空/空白串不进入库解析链路（同 explores / exec 轨道口径）。
fn is_blank_root(root: &str) -> bool {
    root.trim().is_empty()
}

/// 模型清单与记录计数（只读；计数 0 也列出；scope 寻址两库——全局库仅 user
/// 维度模型、workspace 库仅 workspace 维度模型，互不混列；Global 忽略 root，
/// Workspace blank root → 空结果不触发库解析）。
#[tauri::command]
#[specta::specta]
pub fn db_models(
    stores: State<'_, WorkspaceStores>,
    scope: DbDimension,
    root: String,
) -> Result<Vec<ModelInfo>, String> {
    match scope {
        DbDimension::User => stores.global().list_models().map_err(|e| e.to_string()),
        DbDimension::Workspace if is_blank_root(&root) => Ok(Vec::new()),
        DbDimension::Workspace => {
            let store = stores.for_root(&root).map_err(|e| e.to_string())?;
            store.list_models().map_err(|e| e.to_string())
        }
    }
}

/// 按模型主键自然序分页扫描记录信封（只读；scope 寻址两库，分页语义不变；
/// 未知模型名 reject——含跨维度模型名，维度由实例锁定；Global 忽略 root，
/// Workspace blank root → 空结果）。
#[tauri::command]
#[specta::specta]
pub fn db_records(
    stores: State<'_, WorkspaceStores>,
    scope: DbDimension,
    root: String,
    model: String,
    offset: u32,
    limit: u32,
) -> Result<Vec<RecordEnvelope>, String> {
    match scope {
        DbDimension::User => stores.global().scan(&model, offset, limit),
        DbDimension::Workspace if is_blank_root(&root) => Ok(Vec::new()),
        DbDimension::Workspace => {
            let store = stores.for_root(&root).map_err(|e| e.to_string())?;
            store.scan(&model, offset, limit)
        }
    }
    .map_err(|e| e.to_string())
}
