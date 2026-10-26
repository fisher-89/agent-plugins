#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::Arc;

use tauri::{generate_handler, Manager};

use store::WorkspaceStores;

use agent::{SessionStreamRegistry, StopRegistry};
use dev_team::commands::all_commands;
use dev_team::commands::watch::WatchRegistry;
use dev_team::env;
use dev_team::selfcheck;
use orchestration::control::ChangeFlowControl;

fn main() {
    // 自检参数：dev 构建识别后走零窗口自检并退出；prod 构建识别后显式拒绝
    //（MUST NOT 用 `#[cfg]` 把拒绝分支整块删掉——否则 prod 传参会静默起 GUI，
    // agent 挂到超时）。
    let want_self_check = std::env::args()
        .skip(1)
        .any(|arg| arg == selfcheck::SELF_CHECK_ARG);
    if want_self_check && !cfg!(debug_assertions) {
        eprintln!("{}", selfcheck::PROD_REJECT_MESSAGE);
        std::process::exit(2);
    }

    let mut builder =
        tauri::Builder::default().plugin(tauri_plugin_updater::Builder::new().build());
    // 单实例仅在 prod 构建注册：dev 构建与已安装的生产版并存（Windows 下该插件
    // 以 identifier 派生具名 mutex，两模式互相唤起 / 退出会让验收被运行中的
    // 生产版阻塞）。identifier 零改动。
    #[cfg(all(desktop, not(debug_assertions)))]
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
    // devtools 与 dev 构建同侧注册（控制台窗口 / dev server 语义不变）；但
    // `--self-check` 路径不注册：该插件在 init 时向 stdout 打印启动横幅与空行，
    // 会污染自检的 `key=value` 机器可读流（契约要求十行、无横幅、无空行）。
    // 这是「自检路径」判据而非模式判据（模式分歧仍全在 `#[cfg]`）。
    #[cfg(debug_assertions)]
    if !want_self_check {
        builder = builder.plugin(tauri_plugin_devtools::init())
    }
    let app = builder
        .plugin(tauri_plugin_dialog::init())
        // 外链打开（agent 搜索来源 / 引用列表的唯一调用点收敛在前端
        // `ExternalLink`；这里与 dialog 同位注册，MUST NOT 自造第二种形态）
        .plugin(tauri_plugin_opener::init())
        .setup(setup_hook)
        .invoke_handler(all_commands!(generate_handler))
        // build() 而非 run()：数据根解析失败要在 main 内显式处置（自然语言提示 +
        // 双写 + 退出码 2），经 setup 返回 Err 会变成 `Failed to setup app` 的
        // panic 文本并丢掉提示；`--self-check` 也必须早于事件循环（放 setup 会闪窗）。
        // 零窗口即返回 ⇒ 内部 setup 尚未执行（它在 `run` 内的 Ready 事件里跑），
        // 故 `app.path()` 与 `manage` 在此均可用。
        .build(tauri::generate_context!("tauri.conf.json"))
        .expect("desktop 应用构建失败");

    let home = app.path().home_dir().expect("home 目录解析失败");
    // 数据根唯一计算点（判别源 `cfg!(debug_assertions)` 单点传入）。
    let data_root = match env::resolve_root(&home, cfg!(debug_assertions)) {
        Ok(root) => root,
        Err(err) => {
            let message = env::render_root_error(&err, &home);
            eprintln!("{message}");
            env::append_boot_error(&home, &message);
            std::process::exit(2);
        }
    };
    // prod 侧 env 只探测不阻断：陈旧变量不该让安装版打不开，但「设了 env 却误跑
    // build:prod」必须留痕（否则数据无声写进真实生产库）。dev 构建零留痕。
    #[cfg(not(debug_assertions))]
    if std::env::var_os(env::DATA_ROOT_ENV).is_some() {
        env::append_ignored_env(&home);
    }
    // 单点注入：setup_hook 与自检共用同一份值（两处看到同一个根由类型保证）。
    app.manage(data_root.clone());

    #[cfg(debug_assertions)]
    if want_self_check {
        std::process::exit(selfcheck::run(&app, &data_root));
    }

    // `App::run` 无返回值（启动期失败在内部 panic）：setup 失败的 fail fast 面
    // 由 `expect`（build）与 setup 返回的 Err 共同承载，这里只进事件循环。
    app.run(|_, _| {});
}

/// setup 钩子：只做真正需要 runtime 的事（打开双库 / 引擎日志 / panic 钩子 /
/// 五注册表挂载）。数据根由 `main` 单点解析后经管理态注入，此处 MUST NOT 二次
/// 解析、MUST NOT 自行拼接 prod 目录名或 dev 子目录名；模式判定与自检 MUST NOT
/// 落在这里。
fn setup_hook(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_root = app.state::<env::DataRoot>().path.clone();
    // 任一步失败即 setup Err → 启动失败（fail fast，无静默空清单降级）。
    let stores = WorkspaceStores::open(&data_root)?;
    // 引擎异常日志：进程一次初始化（append-only 尽力而为）+ panic 钩子
    //（泵任务 panic 默认只进无控制台的 stderr，日志面补观测）
    agent_runtime::init_engine_log(&data_root.join(env::LOGS_DIR_NAME));
    agent_runtime::install_engine_panic_hook();
    app.manage(stores);
    // watch 订阅注册表：消费页面生命周期由命令面退订承载，此处只挂空表
    app.manage(WatchRegistry::default());
    // agent 停止注册表（内核治理面，键 = core session id）：
    // 内核 begin_turn 登记 / drive 终态除名 / agent_stop 查询，此处
    // 只挂空表（Arc 承载跨内核实例共享，与 WatchRegistry 同型托管）
    app.manage(Arc::new(StopRegistry::default()));
    // agent 会话实时流注册表（键 = core session id，与 StopRegistry 同
    // 型托管）：内核 begin_turn 登记 / converge 投终态除名 /
    // agent_session_watch 订阅，进程内存态重启即清空（delta 永不落库）
    app.manage(Arc::new(SessionStreamRegistry::default()));
    // change-flow run 控制注册表（进程内，键 = (workspace root, change)
    // 复合）：change_flow_* 命令面读写 / walker 持 RunGuard 写，此处只
    // 挂空表（Arc 承载跨 sink 桥共享，与 StopRegistry 同型托管）。归档
    // 已并入 run 的 archive 相位（stop-before-archive 手动门），无独立
    // 归档链注册表。
    app.manage(Arc::new(ChangeFlowControl::new()));
    Ok(())
}
