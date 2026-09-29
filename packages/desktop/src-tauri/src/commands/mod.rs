//! Tauri command 六轨：queries（change 域查询 + workspace 注册）、exec（执行；
//! 承载 agent 命令）、explores（explore 读面 + 记录面）、watch（单文件失效
//! 信号订阅）、db（db 查看轨道，只读）与 stats（工作区代码统计，无状态解析）。
//!
//! Command body 纪律（决策出处：desktop-app-shell 能力 spec，
//! `specs/desktop-app-shell/spec.md`，路径相对域根）：每条命令的 body 仅允许三件事——
//! 参数转换（IPC 入参 → 领域/store 入参）、调用（core 函数或 store 操作）、
//! 错误映射（领域/Store 错误 → `Err(String)`）。两条禁令：领域解释 SHALL 下推 core；
//! 跨边界协调 SHALL 触发 app crate 决策（五条翻转信号见 spec），
//! MUST NOT 以「先塞进命令里」的方式消化编排增长。
//! `commands/workspaces` 的 `*_inner(&Store)` 纯函数模式与 `commands/exec` 的
//! `start_agent_run()` / `drive_agent_run()` 编排函数同为 app 层微形态，将来
//! 抽 app crate 时平移复用（函数边界升 crate 边界）、不重写、不内联回命令体。

pub mod db;
pub mod exec;
pub mod explores;
pub mod queries;
pub mod stats;
pub mod watch;
pub mod workspaces;

/// 全量命令清单（单一登记面，纯路径注入零依赖）：23 条命令路径（迁自旧版
/// `main.rs` `generate_handler!`）注入调用方传入的宏——运行时 `main` 传原生
/// `tauri::generate_handler`，导出 `bindings` 传 `tauri_specta::collect_commands`，
/// 新增命令只改此处，两侧自动同步。`commands` 自身不依赖 tauri_specta。
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
        ]
    };
}
pub use crate::all_commands;
