#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use std::path::PathBuf;

use tauri::Manager;

use store::Store;

/// db 文件落位：`home_dir()/.dev-team` 根，不建子目录（数据维度语义由表名承载）；
/// 父目录由 `Store::open` 内部补齐。
const DB_FILE_NAME: &str = ".dev-team/desktop-store.redb";

fn main() {
    let mut builder =
        tauri::Builder::default().plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(true) = window.is_minimized() {
                    let _ = window.unminimize();
                }
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // db 路径解析在此（app_data_dir 依赖 Tauri 上下文），store 内无环境解析。
            // 任一步失败即 setup Err → run Err → expect 报错启动失败：
            // fail fast，无静默空清单降级。
            let db_path: PathBuf = app.path().home_dir()?.join(DB_FILE_NAME);
            let store = Store::open(&db_path)?;
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::queries::list_changes,
            commands::queries::get_change_detail,
            commands::queries::read_artifact,
            commands::workspaces::list_workspaces,
            commands::workspaces::add_workspace,
            commands::workspaces::remove_workspace,
            commands::workspaces::touch_workspace,
        ])
        .run(tauri::generate_context!("tauri.conf.json"))
        .expect("desktop 应用启动失败");
}
