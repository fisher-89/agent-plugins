mod agent;

#[cfg(test)]
mod mod_test;

use std::path::Path;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

// `mod agent`（本地编排模块）与外部 `agent` 契约 crate 同名：外部 crate
// 以 `::agent::` 显式消歧
use ::agent::{AgentEvent, AgentPermissionMode, AgentRunParams};
use store::{AgentRunRecord, WorkspaceStores};

pub use agent::RunStopRegistry;

use agent::{AgentRunMessage, RunProvenance};

/// root 显式格式检查：空/空白串不进入库解析链路（同 explores 轨道口径）。
fn is_blank_root(root: &str) -> bool {
    root.trim().is_empty()
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub async fn agent_start(
    app: AppHandle,
    on_event: Channel<AgentRunMessage>,
    root: String,
    prompt: String,
    permission_mode: AgentPermissionMode,
    resume_session_id: Option<String>,
    source: Option<String>,
    source_ref: Option<String>,
    parent_run_id: Option<i64>,
    agent: Option<i64>,
) -> Result<AgentRunRecord, String> {
    if is_blank_root(&root) {
        return Err("非法 root: 不得为空白（无 cwd 无从发起）".to_owned());
    }
    let params = AgentRunParams {
        prompt,
        cwd: Path::new(&root).to_path_buf(),
        permission_mode,
        resume_session_id,
    };
    // 来源缺省 debug（调试链路语义不变）；显式传入的定位与链参数始终保留
    let mut provenance = RunProvenance::debug();
    if let Some(source) = source {
        provenance.source = source;
    }
    provenance.source_ref = source_ref;
    provenance.parent_run_id = parent_run_id;
    // 参数转换段：agent 缺省 / 显式解析收单点（产物 kind + 连接配置；解析
    // Err reject 前端，不落库不推流）
    let stores = app.state::<WorkspaceStores>();
    let resolved = agent::resolve_agent_engine(stores.inner(), agent)?;
    agent::start_agent_run(
        app.clone(),
        stores.inner(),
        on_event,
        params,
        provenance,
        resolved,
    )
}

#[tauri::command]
#[specta::specta]
pub fn agent_stop(
    registry: State<'_, RunStopRegistry>,
    root: String,
    run_id: i64,
) -> Result<(), String> {
    // miss 幂等：句柄不在注册表即已终态或不存在（含 blank root），无副作用直接成功
    let _ = registry.request_stop(&root, run_id);
    Ok(())
}

/// 当前 workspace 的历史运行清单（started_at 降序）；blank root → 空结果。
#[tauri::command]
#[specta::specta]
pub fn agent_runs(
    stores: State<'_, WorkspaceStores>,
    root: String,
) -> Result<Vec<AgentRunRecord>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    stores
        .for_root(&root)
        .map_err(|e| e.to_string())?
        .list_agent_runs()
        .map_err(|e| e.to_string())
}

/// 单 run 事件重放（seq 升序）：store 事件 API 类型化，直接返回；
/// blank root → 空结果。
#[tauri::command]
#[specta::specta]
pub fn agent_run_events(
    stores: State<'_, WorkspaceStores>,
    root: String,
    run_id: i64,
) -> Result<Vec<AgentEvent>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    stores
        .for_root(&root)
        .map_err(|e| e.to_string())?
        .list_agent_run_events(run_id)
        .map_err(|e| e.to_string())
}

/// 来源单链还原（发起顺序）：沿 `parent_run_id` 显式指针回溯整链，链拼接收
/// 口 store 单点；无链返回空数组。通用面查询（非 explore 专属）；
/// `source_ref` 为 workspace 库域内的 explore 记录 id（root 寻址与库域内 id
/// 配套消解跨库歧义）；blank root → 空结果。
#[tauri::command]
#[specta::specta]
pub fn agent_run_chain(
    stores: State<'_, WorkspaceStores>,
    root: String,
    source: String,
    source_ref: String,
) -> Result<Vec<AgentRunRecord>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    stores
        .for_root(&root)
        .map_err(|e| e.to_string())?
        .restore_run_chain(&source, &source_ref)
        .map_err(|e| e.to_string())
}
