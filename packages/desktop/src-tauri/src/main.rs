#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;
use std::sync::Arc;

use tauri::{generate_handler, window::Color, Manager, Theme, WebviewUrl, WebviewWindowBuilder};

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

/// 启动参数：开启原生 webview devtools（F12 / 右键检查）。
/// 门控在主窗 `.devtools()` 建窗参数上——release 无此参数时 devtools 完全关闭
/// （同未编 devtools feature 的产物），传入时开启并自动弹出；debug 构建恒开。
const DEV_FLAG: &str = "--dev";

fn main() {
    let dev = std::env::args().any(|arg| arg == DEV_FLAG);
    let mut builder =
        tauri::Builder::default().plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(true) = window.is_minimized() {
                    let _ = window.unminimize();
                }
                let _ = window.show();
                let _ = window.set_focus();
                // 二实例的 --dev 转发为再开 devtools（如面板被关后再唤起）。
                // 仅当首实例建窗时已启用 devtools 才生效——门控在建窗期，
                // 首实例未带 --dev 时此处为无操作，无法事后补开。
                if args.iter().any(|arg| arg == DEV_FLAG) {
                    window.open_devtools();
                }
            }
        }));
    }
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_devtools::init())
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // 主窗由代码创建（tauri.conf.json windows 置空）：devtools 门控需要
            // 建窗期 .devtools()，配置窗口无此出口。尺寸/最小尺寸/dark 主题/
            // 背景色（消启动白闪）逐项平移自原窗口配置。
            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Dev Team")
                    .inner_size(1200.0, 800.0)
                    .min_inner_size(900.0, 600.0)
                    .resizable(true)
                    .theme(Some(Theme::Dark))
                    .background_color(Color(0x16, 0x1a, 0x22, 0xff))
                    .devtools(dev || cfg!(debug_assertions))
                    .build()?;
            if dev {
                window.open_devtools();
            }
            // 数据根解析在此（home_dir 依赖 Tauri 上下文），store 内无环境解析。
            // 任一步失败即 setup Err → run Err → expect 报错启动失败：
            // fail fast，无静默空清单降级。
            let data_root: PathBuf = app.path().home_dir()?.join(DATA_DIR);
            let stores = WorkspaceStores::open(&data_root)?;
            // 引擎异常日志：进程一次初始化（append-only 尽力而为）+ panic 钩子
            //（泵任务 panic 默认只进无控制台的 stderr，日志面补观测）
            agent_runtime::init_engine_log(&data_root.join(LOGS_DIR_NAME));
            agent_runtime::install_engine_panic_hook();
            app.manage(stores);
            // watch 订阅注册表：消费页面生命周期由命令面退订承载，此处只挂空表
            app.manage(WatchRegistry::default());
            // agent 停止注册表（内核治理面，键 = core session id）：
            // 内核 begin_turn 登记 / drive 终态除名 / agent_stop 查询，此处
            // 只挂空表（Arc 承载跨内核实例共享，与 WatchRegistry 同型托管）
            app.manage(Arc::new(StopRegistry::default()));
            // change-flow run 控制注册表（进程内，键 = change 名）：
            // change_flow_* 命令面读写 / walker 持 RunGuard 写，此处只挂空表
            //（Arc 承载跨 sink 桥共享，与 StopRegistry 同型托管）
            app.manage(Arc::new(ChangeFlowControl::new()));
            Ok(())
        })
        .invoke_handler(all_commands!(generate_handler))
        .run(tauri::generate_context!("tauri.conf.json"))
        .expect("desktop 应用启动失败");
}
