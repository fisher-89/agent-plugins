#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::queries::list_changes,
            commands::queries::get_change_detail,
            commands::queries::read_artifact,
        ])
        .run(tauri::generate_context!("tauri.conf.json"))
        .expect("desktop 应用启动失败");
}
