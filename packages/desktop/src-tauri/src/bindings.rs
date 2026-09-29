//! specta builder 组装与 TS bindings 幂等导出：全部 IPC 命令的唯一注册面
//! （`main` 的 `invoke_handler` 与导出工具共用 [`builder`]），前端 IPC 类型
//! 与调用面的唯一生成源。
//!
//! 错误通道定夺（PoC 回填 design「PoC 前置门」）：`ErrorHandlingMode::Throw`
//! ——`Result<T, String>` 命令的生成绑定为 `Promise<T>` reject 语义，与前端
//! 既有 `.catch → error 态` 接线逐字兼容（替换前后错误面同形）。
//! events 面不引入：应用流式全景为全 Channel、零 emit/listen。

use std::path::PathBuf;

use tauri_specta::{collect_commands, Builder, ErrorHandlingMode};

/// 全部命令的 specta builder：23 条命令全量注册（迁自 `main.rs`
/// `generate_handler!`，命令清单与总数以 `main.rs` 迁移前为准）。
pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            crate::commands::queries::list_changes,
            crate::commands::queries::get_change_detail,
            crate::commands::queries::read_artifact,
            crate::commands::workspaces::list_workspaces,
            crate::commands::workspaces::add_workspace,
            crate::commands::workspaces::remove_workspace,
            crate::commands::exec::agent_start,
            crate::commands::exec::agent_stop,
            crate::commands::exec::agent_runs,
            crate::commands::exec::agent_run_events,
            crate::commands::exec::agent_run_chain,
            crate::commands::explores::read_explore,
            crate::commands::explores::scan_explores,
            crate::commands::explores::explore_doc_path,
            crate::commands::explores::list_explore_records,
            crate::commands::explores::create_explore_record,
            crate::commands::explores::rename_explore_record,
            crate::commands::explores::delete_explore_record,
            crate::commands::watch::watch_subscribe,
            crate::commands::watch::watch_unsubscribe,
            crate::commands::db::db_models,
            crate::commands::db::db_records,
            crate::commands::stats::code_stats,
        ])
        .error_handling(ErrorHandlingMode::Throw)
        // serde_json::Value 自引用递归（Value → Vec<Value> → Value），不可结构
        // 化展开；语义规则改写为 TS `unknown`（与既有 dto.ts 的 payload/usage/
        // input/key/value 口径一致，前端 renderer 自行收窄）
        .semantic_types(
            specta_typescript::semantic::Configuration::default().define::<serde_json::Value>(
                |_| specta_typescript::define("unknown").into(),
                None,
                None,
            ),
        )
        // i64/u64 时间戳与 id（毫秒时间戳、run id 等，值域远低于 2^53）按线
        // 格式（serde_json number）出线为 TS `number`，与既有 dto.ts 口径一致
        .dangerously_cast_bigints_to_number()
}

/// 幂等导出 TS bindings 到 `packages/desktop/src/types/generated/bindings.ts`：
/// 路径以 `CARGO_MANIFEST_DIR` 定位（父目录 `create_dir_all`），输出确定性
/// （无时间戳 / 无机器路径嵌入），同输入连续两次导出零 diff。
pub fn export_bindings() -> Result<(), String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "CARGO_MANIFEST_DIR 无父目录".to_owned())?
        .join("src/types/generated");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建生成目录失败: {e}"))?;
    builder()
        .export(
            specta_typescript::Typescript::default(),
            dir.join("bindings.ts"),
        )
        .map_err(|e| format!("导出 TS bindings 失败: {e}"))
}
