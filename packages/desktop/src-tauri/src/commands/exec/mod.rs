//! exec 执行轨道：agent 首批三命令（结束空轨道状态）。
//!
//! `agent_start`（执行）：async 命令阻塞至 run 结束，body 三件事——参数转换
//! （IPC 入参 → `AgentRunParams`）、调用（编排收在 `agent.rs` 的
//! `run_agent()`）、错误映射（启动阶段失败 → `Err(String)`）；事件实时流经
//! `onEvent` Channel 逐事件推送（命令作用域执行流通道）。
//! `agent_runs` / `agent_run_events`（查询）：无状态薄包装——`State<'_, Store>`
//! 取 store + 参数转换 + DTO 返回，事件行以 `serde_json::Value` 出库后反序列化。
//! 错误约定沿 workspaces 轨道模板：命令返回 `Result<T, String>`，`Err` 由
//! Tauri 转为前端 reject，MUST NOT 静默吞掉失败。

mod agent;

#[cfg(test)]
mod mod_test;

use std::path::Path;

use tauri::ipc::Channel;
use tauri::State;

// `mod agent`（本地编排模块）与外部 `agent` 契约 crate 同名：外部 crate
// 以 `::agent::` 显式消歧
use ::agent::{AgentEnvMode, AgentEvent, AgentPermissionMode, AgentRunParams};
use store::{AgentRunRecord, Store};

/// 发起一次 agent 运行：阻塞至 run 结束，返回最终记录（含 failed 终态）；
/// 启动阶段失败（CLI 缺失 / spawn 失败 / store 失败）返回 `Err`。
/// cwd 隐含为当前 workspace root（前端 invoke 固定传 `root`，无 UI 输入）。
#[tauri::command]
pub async fn agent_start(
    store: State<'_, Store>,
    on_event: Channel<AgentEvent>,
    root: String,
    prompt: String,
    env: AgentEnvMode,
    permission_mode: AgentPermissionMode,
) -> Result<AgentRunRecord, String> {
    let params = AgentRunParams {
        prompt,
        cwd: Path::new(&root).to_path_buf(),
        env,
        permission_mode,
    };
    agent::run_agent(&store, on_event, params).await
}

/// 历史运行清单（started_at 降序）。
#[tauri::command]
pub fn agent_runs(store: State<'_, Store>) -> Result<Vec<AgentRunRecord>, String> {
    store.list_agent_runs().map_err(|e| e.to_string())
}

/// 单 run 事件重放（seq 升序）：事件行 `Value → AgentEvent` 反序列化后返回。
#[tauri::command]
pub fn agent_run_events(store: State<'_, Store>, run_id: i64) -> Result<Vec<AgentEvent>, String> {
    store
        .list_agent_run_events(run_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|value| serde_json::from_value(value).map_err(|e| format!("事件反序列化失败: {e}")))
        .collect()
}
