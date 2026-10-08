#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;
use std::sync::Arc;

use tauri::{generate_handler, Manager};

use store::WorkspaceStores;

use agent::StopRegistry;
use dev_team::commands::all_commands;
use dev_team::commands::watch::WatchRegistry;
use orchestration::control::ChangeFlowControl;

/// 全局数据目录名：`home_dir()` 根下（双库落位基准——全局库直居其下、
/// workspace 库落 `workspaces/` 子树；库文件名与子树语义由 store 常量单点
/// 承载，父目录由 store 打开流程内部补齐，见 desktop-data-dimensions）。
/// 引擎异常日志落其 `logs/` 子树（sdk 泵悬挂 / panic 的带外观测面）。
const DATA_DIR: &str = ".dev-team";

/// 引擎异常日志子树目录名（数据根下）。
const LOGS_DIR_NAME: &str = "logs";

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
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_devtools::init())
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 数据根解析在此（home_dir 依赖 Tauri 上下文），store 内无环境解析。
            // 任一步失败即 setup Err → run Err → expect 报错启动失败：
            // fail fast，无静默空清单降级。
            let data_root: PathBuf = app.path().home_dir()?.join(DATA_DIR);
            let stores = WorkspaceStores::open(&data_root)?;
            // 引擎异常日志：进程一次初始化（append-only 尽力而为）+ panic 钩子
            //（泵任务 panic 默认只进无控制台的 stderr，日志面补观测）
            agent_runtime::init_engine_log(&data_root.join(LOGS_DIR_NAME));
            agent_runtime::install_engine_panic_hook();
            // 数据根注入（worktrees 落位派生的注入面，沿 store 注入式打开
            // 纪律——`create_change` 经 vcs 单点派生 worktree 落位）
            app.manage(data_root.clone());
            app.manage(stores);
            // watch 订阅注册表：消费页面生命周期由命令面退订承载，此处只挂空表
            app.manage(WatchRegistry::default());
            // agent 停止注册表（内核治理面，键 = core session id）：
            // 内核 begin_turn 登记 / drive 终态除名 / agent_stop 查询，此处
            // 只挂空表（Arc 承载跨内核实例共享，与 WatchRegistry 同型托管）
            app.manage(Arc::new(StopRegistry::default()));
            // change-flow run 控制注册表（进程内，键 = (workspace root, change)
            // 复合）：change_flow_* 命令面读写 / walker 持 RunGuard 写，此处只
            // 挂空表（Arc 承载跨 sink 桥共享，与 StopRegistry 同型托管）
            app.manage(Arc::new(ChangeFlowControl::new()));
            // 归档链控制注册表（进程内，键 = (workspace root, change) 复合）：
            // archive_flow_* 命令面读写 / 归档链持 ArchiveGuard 写，此处只挂空表
            //（与 ChangeFlowControl 同型托管——run 面与归档面两注册表互不混入）
            app.manage(Arc::new(orchestration::archive_flow::ArchiveControl::new()));
            Ok(())
        })
        .invoke_handler(all_commands!(generate_handler))
        .run(tauri::generate_context!("tauri.conf.json"))
        .expect("desktop 应用启动失败");
}
