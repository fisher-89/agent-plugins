//! exec 执行轨道：agent 执行与查询命令。
//!
//! `agent_start`（执行）：async 命令阻塞至 run 结束，body 三件事——参数转换
//! （IPC 入参 → `AgentRunParams` + `RunProvenance`）、调用（编排收在
//! `agent.rs` 的 `run_agent()`）、错误映射（启动阶段失败 → `Err(String)`）；
//! 事件实时流经 `onEvent` Channel 逐事件推送（命令作用域执行流通道）。可选
//! 续会话与来源参数（`resume_session_id` / `source` / `source_ref` /
//! `parent_run_id`）全部缺省安全：不传即既有 one-shot 调试行为（来源缺省
//! `debug`、链指针 `None`），调试页 invoke 行为与现状一致。
//! `agent_runs` / `agent_run_events` / `agent_run_chain`（查询）：无状态薄
//! 包装——`State<'_, Store>` 取 store + 参数转换 + DTO 返回，事件与链还原经
//! store 类型化 API 直接出库（链拼接收口 store 单点，命令不做领域解释）。
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

use agent::RunProvenance;

/// 发起一次 agent 运行：阻塞至 run 结束，返回最终记录（含 failed 终态）；
/// 启动阶段失败（CLI 缺失 / spawn 失败 / store 失败）返回 `Err`。
/// cwd 隐含为当前 workspace root（前端 invoke 固定传 `root`，无 UI 输入）。
/// 四个可选参数：`resume_session_id` 续会话（进 runner 契约，组装
/// `--resume` flag）；`source` / `source_ref` / `parent_run_id` 来源三元组
/// （旁路编排落库，`source` 缺省 `debug`）。
// 命令入参即扁平 IPC 参数面（三件事纪律的参数转换段），打包成结构体反而
// 背离轨道模板，故豁免 clippy 参数数上限
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn agent_start(
    store: State<'_, Store>,
    on_event: Channel<AgentEvent>,
    root: String,
    prompt: String,
    env: AgentEnvMode,
    permission_mode: AgentPermissionMode,
    resume_session_id: Option<String>,
    source: Option<String>,
    source_ref: Option<String>,
    parent_run_id: Option<i64>,
) -> Result<AgentRunRecord, String> {
    let params = AgentRunParams {
        prompt,
        cwd: Path::new(&root).to_path_buf(),
        env,
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
    agent::run_agent(&store, on_event, params, provenance).await
}

/// 历史运行清单（started_at 降序）。
#[tauri::command]
pub fn agent_runs(store: State<'_, Store>) -> Result<Vec<AgentRunRecord>, String> {
    store.list_agent_runs().map_err(|e| e.to_string())
}

/// 单 run 事件重放（seq 升序）：store 事件 API 类型化，直接返回。
#[tauri::command]
pub fn agent_run_events(store: State<'_, Store>, run_id: i64) -> Result<Vec<AgentEvent>, String> {
    store
        .list_agent_run_events(run_id)
        .map_err(|e| e.to_string())
}

/// 来源单链还原（发起顺序）：沿 `parent_run_id` 显式指针回溯整链，链拼接收
/// 口 store 单点；无链返回空数组。通用面查询（非 explore 专属）。
#[tauri::command]
pub fn agent_run_chain(
    store: State<'_, Store>,
    source: String,
    source_ref: String,
) -> Result<Vec<AgentRunRecord>, String> {
    store
        .restore_run_chain(&source, &source_ref)
        .map_err(|e| e.to_string())
}
