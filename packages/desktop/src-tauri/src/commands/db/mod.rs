//! db 查看命令轨道（native-db-store-upgrade 开通）：只读 db 检查命令，经
//! `State<'_, Store>` 调用 store 的记录信封 API（`list_models` / `scan`），
//! 无状态薄包装——参数转换 + 信封 API 调用 + DTO 返回。
//!
//! 轨道纪律：只读，MUST NOT 出现任何写命令（写操作待真实 debug 需求出现后
//! 由后续变更裁定）。错误约定沿 workspaces 轨道模板：命令返回
//! `Result<T, String>`，`Err` 由 Tauri 转为前端 reject，MUST NOT 静默吞掉
//! 失败。

use tauri::State;

use store::{ModelInfo, RecordEnvelope, Store};

#[cfg(test)]
mod mod_test;

/// 模型清单与记录计数（只读；计数 0 也列出）。
#[tauri::command]
pub fn db_models(store: State<'_, Store>) -> Result<Vec<ModelInfo>, String> {
    store.list_models().map_err(|e| e.to_string())
}

/// 按模型主键自然序分页扫描记录信封（只读；未知模型名 reject）。
#[tauri::command]
pub fn db_records(
    store: State<'_, Store>,
    model: String,
    offset: u32,
    limit: u32,
) -> Result<Vec<RecordEnvelope>, String> {
    store.scan(&model, offset, limit).map_err(|e| e.to_string())
}
