pub mod agents;
pub mod config;
pub mod db;
pub mod exec;
pub mod explores;
pub mod queries;
pub mod stats;
pub mod watch;
pub mod workspaces;

/// 全量命令清单（单一登记面，纯路径注入零依赖）
#[macro_export]
macro_rules! all_commands {
    ($mac:ident) => {
        $mac![
            $crate::commands::queries::list_changes,
            $crate::commands::queries::get_change_detail,
            $crate::commands::queries::read_artifact,
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
            $crate::commands::exec::agent_runs,
            $crate::commands::exec::agent_run_events,
            $crate::commands::exec::agent_run_chain,
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
