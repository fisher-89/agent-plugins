//! workspace 库冷开的阻塞池出口：`for_root` 首次命中可能触发 redb 全库修复
//! 扫描（成本随库大小，GB 级库秒级），命令体 MUST NOT 在 IPC 线程内联执行。
//! 命令面统一经本函数落阻塞池——store 解析也在闭包内，避免把修复扫描放到
//! tokio worker 上（命令执行线程纪律的唯一持有者是 `desktop-app-shell` spec）。

use tauri::{AppHandle, Manager};

/// 在阻塞池执行一次 store 任务；`context` 进 `JoinError` 文案（前向防御）。
pub(crate) async fn run_store_task<R: tauri::Runtime, T, F>(
    context: &'static str,
    app: AppHandle<R>,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&store::WorkspaceStores) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let stores = app.state::<store::WorkspaceStores>();
        task(stores.inner())
    })
    .await
    .map_err(|error| format!("{context}: {error}"))?
}
