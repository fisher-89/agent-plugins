//! exec 执行轨道：agent 执行、终止与查询命令。
//!
//! `agent_start`（执行）：body 三件事——参数转换（IPC 入参组装
//! `AgentRunParams` 与 `RunProvenance`）、调用（编排收在 `agent.rs` 的
//! `start_agent_run()`）、错误映射（启动阶段失败 → `Err(String)`）；契约
//! 演进为提前 resolve——run 记录落库进入 running 后即返回 running 记录
//! （id 立即可用），执行转后台任务继续，终态记录经 `onEvent` Channel 以
//! `AgentRunMessage::Record` 信封流出（MUST NOT 再依赖 invoke 返回携带
//! 终态）。可选续会话与来源参数（`resume_session_id` / `source` /
//! `source_ref` / `parent_run_id`）全部缺省安全：不传即既有 one-shot 调试
//! 行为（来源缺省 `debug`、链指针 `None`）。落库经 `for_root` 路由至当前
//! workspace 库（run id 为库域内自增）；blank root → `Err`（无 cwd 无从发起）。
//! `agent_stop`（终止）：携 root 按 `(root, run id)` 复合键寻址运行中 run 的
//! 停止句柄置位信号（句柄注册表 `RunStopRegistry`），租户泵击杀进程树、编排
//! 收敛 `stopped` 并经 Channel 流出终态 Record；对非 running（已终态除名）
//! 或不存在键幂等 `Ok`（blank root 天然 miss 同口径）。
//! `agent_runs` / `agent_run_events` / `agent_run_chain`（查询）：携 root
//! 无状态薄包装——`for_root` 解析所属 workspace 库 + 参数转换 + DTO 返回，
//! 运行清单随之收窄为当前 workspace 的历史；blank root 查询空结果（不触发
//! 库解析）；事件与链还原经 store 类型化 API 直接出库（链拼接收口 store 单
//! 点，命令不做领域解释）。
//! 错误约定沿 workspaces 轨道模板：命令返回 `Result<T, String>`，`Err` 由
//! Tauri 转为前端 reject，MUST NOT 静默吞掉失败。

mod agent;

#[cfg(test)]
mod mod_test;

use std::path::Path;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

// `mod agent`（本地编排模块）与外部 `agent` 契约 crate 同名：外部 crate
// 以 `::agent::` 显式消歧
use ::agent::{AgentEvent, AgentPermissionMode, AgentRunParams};
use agent_runtime::EngineKind;
use store::{AgentRunRecord, WorkspaceStores};

pub use agent::RunStopRegistry;

use agent::{AgentRunMessage, RunProvenance, DEFAULT_ENGINE};

/// root 显式格式检查：空/空白串不进入库解析链路（同 explores 轨道口径）。
fn is_blank_root(root: &str) -> bool {
    root.trim().is_empty()
}

/// 发起一次 agent 运行：run 记录落库进入 running 后**提前 resolve** 返回
/// running 记录（含 id，可直接用于 `agent_stop` 寻址）；执行转后台任务，
/// 事件实时流与终态记录均经 `onEvent` Channel 流出。启动阶段失败（CLI
/// 缺失 / spawn 失败 / 配置缺失 / workspace 库解析失败）返回 `Err`。cwd
/// 隐含为当前 workspace root（前端 invoke 固定传 `root`，无 UI 输入）。
/// 可选参数：`resume_session_id` 续会话（进 runner 契约——CLI 组装
/// `--resume` flag、sdk 引擎经转录装载缝重建对话史）；`source` /
/// `source_ref` / `parent_run_id` 来源三元组（旁路编排落库，`source` 缺省
/// `debug`）；`engine` 参数选择引擎（invoke body 携 engine 字段，壳层仅
/// 映射、缺省硬编码默认 agent（`DEFAULT_ENGINE`，当前 SDK/rig）：引擎选择
/// 仅调试页暴露，正式场景 MUST NOT 传 engine；引擎接线全在门面
/// `EngineFacade::runner_for`，编排层零引擎分支）。
// 命令入参即扁平 IPC 参数面（三件事纪律的参数转换段），打包成结构体反而
// 背离轨道模板，故豁免 clippy 参数数上限；`app` 为 Tauri 注入项（非 IPC
// 参数），后台任务经前者取托管句柄。stores 不再作注入形参：specta rc.25
// 的 SpectaFn 实现上限 10 个 Rust 形参（engine 尾参入列前已顶格），而
// `State<'_, WorkspaceStores>` 与 IPC 面无关（不出线、不进生成绑定），改经
// `app.state()` 托管态取用等价，IPC 观测面零变化
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
    engine: Option<EngineKind>,
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
    // 引擎缺省收敛默认 agent（Option 保证不传 engine 的既有调用零改动）
    let stores = app.state::<WorkspaceStores>();
    agent::start_agent_run(
        app.clone(),
        stores.inner(),
        on_event,
        params,
        provenance,
        engine.unwrap_or(DEFAULT_ENGINE),
    )
}

/// 终止一次运行中 agent 运行：携 root 按 `(root, run id)` 复合键寻址停止
/// 句柄置位信号（租户泵击杀进程树、编排收敛 `stopped`、Channel 流出终态
/// Record）；对已终态（除名）或不存在键幂等 `Ok`，不报错、不改写既有终态
/// （run id 为 workspace 库域内自增，裸 id 跨库歧义由 root 消解，不跨库误停）。
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
