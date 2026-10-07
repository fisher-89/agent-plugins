mod agent;

#[cfg(test)]
mod mod_test;

use std::path::Path;
use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

// `mod agent`（本地薄包装模块）与外部 `agent` 契约 crate 同名：外部 crate
// 以 `::agent::` 显式消歧
use ::agent::{
    KernelOutput, SessionCtx, SessionProvenance, SessionRef, SessionSummary, StopRegistry,
    TurnSummary,
};
use agent_runtime::compose_turn;
use store::WorkspaceStores;

use agent::AgentRunMessage;

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
    permission_mode: ::agent::AgentPermissionMode,
    session_id: Option<String>,
    source: Option<String>,
    source_ref: Option<String>,
    agent: Option<i64>,
) -> Result<TurnSummary, String> {
    agent_start_with(
        app,
        on_event,
        root,
        prompt,
        permission_mode,
        session_id,
        source,
        source_ref,
        agent,
    )
    .await
}

/// 命令薄入口的泛型缝（对 runtime 泛型；生产经 [`agent_start`] 注入 Wry 句柄，
/// 测试经 `tauri::test::mock_app` 注入 MockRuntime 句柄——沿 start_agent_run_with
/// 泛型缝先例）：参数转换 → 组合根 + 内核调用 → 错误映射三件事与命令体同源。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn agent_start_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<AgentRunMessage>,
    root: String,
    prompt: String,
    permission_mode: ::agent::AgentPermissionMode,
    session_id: Option<String>,
    source: Option<String>,
    source_ref: Option<String>,
    agent: Option<i64>,
) -> Result<TurnSummary, String> {
    if is_blank_root(&root) {
        return Err("非法 root: 不得为空白（无 cwd 无从发起）".to_owned());
    }
    // 参数转换段：会话引用（None 即 New、Some 即 Continue）+ 来源归属
    //（缺省 debug）+ 轮级上下文（cwd 恒为当前 workspace root，前端固定传 root）
    let session = match session_id {
        None => SessionRef::New,
        Some(id) => SessionRef::Continue { id },
    };
    let provenance = SessionProvenance {
        source: source.unwrap_or_else(|| "debug".to_owned()),
        source_ref,
    };
    let ctx = SessionCtx {
        workspace_root: Path::new(&root).to_path_buf(),
        permission_mode,
    };
    // 调用段：组合根 + 内核（解析/快照/编排全部下沉；命令体零解析残留）。
    // store 半边注入 workspace root 库实例（compose 拆参，design D8；会话仍
    // 主 root——exec / explore 轨道与 change 域工作面分离，D7 拍板）
    let stores = app.state::<WorkspaceStores>();
    let registry = app.state::<Arc<StopRegistry>>();
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let composed = compose_turn(stores.inner(), Arc::clone(registry.inner()), store, agent)?;
    let running = composed.begin(session, prompt, ctx, provenance)?;
    // 后台转发任务：内核泵输出 → Channel（事件与终态同构流出）；提前 resolve
    // 返回 running 态轮行（id 立即可知），终态 MUST NOT 依赖 invoke 返回
    let summary = agent::running_summary(&running);
    let session_id = running.session_id.clone();
    let started_at = running.started_at;
    tauri::async_runtime::spawn(async move {
        let _ = running
            .drive(move |output| match output {
                KernelOutput::Observation(event) => {
                    let _ = on_event.send(AgentRunMessage::Event { event });
                }
                KernelOutput::TurnFinished(outcome) => {
                    let _ = on_event.send(AgentRunMessage::Record {
                        record: agent::outcome_summary(&outcome, &session_id, started_at),
                    });
                }
            })
            .await;
    });
    Ok(summary)
}

#[tauri::command]
#[specta::specta]
pub fn agent_stop(
    registry: State<'_, Arc<StopRegistry>>,
    root: String,
    session_id: String,
) -> Result<(), String> {
    // miss 幂等：句柄不在注册表即已终态或不存在（含 blank root），无副作用
    // 直接成功、不改既有终态；root 寻址保留
    if is_blank_root(&root) {
        return Ok(());
    }
    registry.request_stop(&session_id);
    Ok(())
}

/// 会话清单 + 聚合统计（来源过滤，`updated_at` 降序）：root 寻址所属
/// workspace 库直查 DTO，无领域解释；blank root → 空结果。
#[tauri::command]
#[specta::specta]
pub fn agent_sessions(
    stores: State<'_, WorkspaceStores>,
    root: String,
    source: Option<String>,
    source_ref: Option<String>,
) -> Result<Vec<SessionSummary>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    agent_runtime::session_query(store).list_sessions(source.as_deref(), source_ref.as_deref())
}

/// 会话全史转录重放（密封事件 seq 序，不要求运行进程存活）；blank root →
/// 空结果。
#[tauri::command]
#[specta::specta]
pub fn agent_session_transcript(
    stores: State<'_, WorkspaceStores>,
    root: String,
    session_id: String,
) -> Result<Vec<::agent::AgentEvent>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    agent_runtime::session_query(store).transcript(&session_id)
}

/// 按 id 单查会话（行 + 聚合统计 + 轮行）：root 寻址所属 workspace 库直查
/// DTO，跨库隔离与既有查询命令一致；blank root → 空结果（`None`）；查无此
/// id 显式 `Err`（单查语义与清单空态区分）。
#[tauri::command]
#[specta::specta]
pub fn session_detail(
    stores: State<'_, WorkspaceStores>,
    root: String,
    session_id: String,
) -> Result<Option<SessionSummary>, String> {
    if is_blank_root(&root) {
        return Ok(None);
    }
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    agent_runtime::session_query(store)
        .find_session_detail(&session_id)
        .map(Some)
}
