use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::env::DataRoot;

#[cfg(test)]
mod mod_test;

/// 应用环境线面：`{ mode, dataRoot, source }`（camelCase，与 specta 生成物
/// 字段同形）。字段值只搬运，无第二处加工。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppEnvironment {
    /// 构建模式线值：`dev` | `prod`（`crate::env::mode()`）
    pub mode: String,
    /// 数据根绝对路径（自检摘要 `data_root=` 同源）
    pub data_root: String,
    /// 数据根来源线值：`prod` | `dev-default` | `env`（`RootSource::wire()`）
    pub source: String,
}

/// 应用环境：管理态 `DataRoot` → 线面平移（无 IO、无 `Result` 错误面）。
#[tauri::command]
#[specta::specta]
pub fn app_environment(data_root: State<'_, DataRoot>) -> AppEnvironment {
    app_environment_inner(&data_root)
}

/// 领域平移纯函数（无 Tauri State）：三个字段各取单点，命令层零加工。
pub fn app_environment_inner(data_root: &DataRoot) -> AppEnvironment {
    AppEnvironment {
        mode: crate::env::mode().to_owned(),
        data_root: data_root.path.to_string_lossy().into_owned(),
        source: data_root.source.wire().to_owned(),
    }
}
