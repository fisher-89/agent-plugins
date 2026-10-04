pub mod agents;
pub mod change_flow;
pub mod changes;
pub mod config;
pub mod db;
pub mod exec;
pub mod explores;
pub mod stats;
pub mod watch;
pub mod workspaces;

/// PATH 进程全局窗口的命令测试共享串行化锁（测试装置）：命令层以 PATH 隔离
/// 驱动引擎不可达收敛（change_flow / exec 的合成收敛链路），窗口期间全进程
/// PATH 被替换——各命令测试文件的 PATH 替换窗口经本锁互斥、测毕恢复（std
/// 锁；持锁跨 await 的臂仅出现在 current_thread 测试运行时，不依赖 Send）。
#[cfg(test)]
pub(crate) static TEST_PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 全量命令清单（单一登记面，纯路径注入零依赖）
#[macro_export]
macro_rules! all_commands {
    ($mac:ident) => {
        $mac![
            $crate::commands::changes::list_changes,
            $crate::commands::changes::get_change_detail,
            $crate::commands::changes::read_artifact,
            $crate::commands::workspaces::list_workspaces,
            $crate::commands::workspaces::add_workspace,
            $crate::commands::workspaces::remove_workspace,
            $crate::commands::agents::list_agent_providers,
            $crate::commands::agents::save_agent_provider,
            $crate::commands::agents::delete_agent_provider,
            $crate::commands::agents::list_agent_instances,
            $crate::commands::agents::save_agent_instance,
            $crate::commands::agents::delete_agent_instance,
            $crate::commands::agents::set_default_agent_instance,
            $crate::commands::exec::agent_start,
            $crate::commands::exec::agent_stop,
            $crate::commands::exec::agent_sessions,
            $crate::commands::exec::agent_session_transcript,
            $crate::commands::exec::session_detail,
            $crate::commands::change_flow::change_flow_start,
            $crate::commands::change_flow::change_flow_stop,
            $crate::commands::change_flow::change_flow_answer,
            $crate::commands::change_flow::change_flow_confirm,
            $crate::commands::change_flow::change_flow_state,
            $crate::commands::change_flow::change_flow_watch,
            $crate::commands::changes::create_change,
            $crate::commands::explores::read_explore,
            $crate::commands::explores::scan_explores,
            $crate::commands::explores::explore_doc_path,
            $crate::commands::explores::list_explore_records,
            $crate::commands::explores::create_explore_record,
            $crate::commands::explores::rename_explore_record,
            $crate::commands::explores::delete_explore_record,
            $crate::commands::watch::watch_subscribe,
            $crate::commands::watch::watch_unsubscribe,
            $crate::commands::db::db_models,
            $crate::commands::db::db_records,
            $crate::commands::stats::code_stats,
            $crate::commands::config::workspace_config,
        ]
    };
}
pub use crate::all_commands;
