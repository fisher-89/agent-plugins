use std::path::Path;
use std::sync::Arc;

#[cfg(test)]
mod mod_test;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use ::agent::StopRegistry;
use agent_runtime::{
    compose_turn, ComposedTurn, GitDiffSource, KernelWorkerPort, ProcessStaticCheck,
};
use foundation::layout;
use orchestration::control::ChangeFlowControl;
use orchestration::port::{
    DiffContextPort, RunEventSink, ToolStepPort, WorkerAgentPort, WorkflowSnapshotPort,
};
use orchestration::snapshot::FsSnapshot;
use orchestration::state::{ChangeRunSnapshot, ChangeRunStatus, ChangeRunSummary, RunUpdate};
use orchestration::steps::LocalToolSteps;
use orchestration::walker::{new_run_id, walk_run, RunRequest};
use store::WorkspaceStores;
use workflow::write::{phase_table, SessionAnchors};

/// 参数显式格式检查：空/空白串不进入库解析 / 注册表链路（与 exec 轨道同
/// 口径，本模块内聚一份避免跨命令组暴露私有件）。
fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// run 事件出口的注册表桥：WorkerAgent 会话事件（早于 turn 收口）据此先行
/// 同步 run 会话槽（停止寻址）与广播总线（Channel 由订阅转发任务回流）。
struct ChangeFlowSink {
    change: String,
    control: Arc<ChangeFlowControl>,
}

impl RunEventSink for ChangeFlowSink {
    fn emit(&self, update: RunUpdate) {
        if let RunUpdate::SessionEvent { session_id, .. } = &update {
            self.control
                .set_session(&self.change, Some(session_id.clone()));
        }
        self.control.publish(&self.change, update);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn change_flow_start(
    app: AppHandle,
    on_event: Channel<RunUpdate>,
    root: String,
    change: String,
) -> Result<ChangeRunSummary, String> {
    change_flow_start_with(app, on_event, root, change).await
}

/// [`change_flow_start`] 的泛型测试缝（生产注入 Wry 句柄、测试注入
/// MockRuntime 句柄，沿 `agent_start_with` 先例）。
pub(crate) async fn change_flow_start_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<RunUpdate>,
    root: String,
    change: String,
) -> Result<ChangeRunSummary, String> {
    if is_blank(&root) {
        return Err("非法 root: 不得为空白（无 cwd 无从发起）".to_owned());
    }
    if is_blank(&change) {
        return Err("非法 change: 不得为空白（无 change 无从发起）".to_owned());
    }
    // 前置校验 1：change 存在且 workflow.json 可解析
    let detail = FsSnapshot::new(root.clone()).detail(&root, &change)?;
    if detail.unparsable {
        return Err(format!(
            "change \"{change}\" 的 workflow.json 无法解析，无从编排"
        ));
    }
    // 前置校验 2：workflow_type=requirement（写面相位表 None → 显式拒绝；
    // V1 范围显式拒绝 bug-fix / test-only，优于相位机半途报错）
    let workflow_layout = layout::resolve(Path::new(&root));
    let workflow = workflow::parse::load_workflow(&workflow_layout.changes_root.join(&change))
        .ok_or_else(|| format!("change \"{change}\" 的 workflow.json 无法解析，无从编排"))?;
    if phase_table(&workflow.workflow_type).is_none() {
        return Err(format!(
            "仅支持 requirement 工作流（change \"{change}\" 的 workflow_type 为 \"{}\"）",
            workflow.workflow_type
        ));
    }
    // 前置校验 3：无并行 run（begin_run 冲突检测）
    let control = Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner());
    let run_id = new_run_id();
    let guard = control.begin_run(&change, run_id.clone())?;

    // 组合根装配（run 作用域一次）：组合 turn + 三 port + 快照源 + 事件桥
    // + run 级会话锚点（每 run 一个实例，W7）
    let stores = app.state::<WorkspaceStores>();
    let registry = Arc::clone(app.state::<Arc<StopRegistry>>().inner());
    let composed: ComposedTurn = compose_turn(stores.inner(), Arc::clone(&registry), &root, None)?;
    let sink: Arc<dyn RunEventSink> = Arc::new(ChangeFlowSink {
        change: change.clone(),
        control: Arc::clone(&control),
    });
    let worker: Arc<dyn WorkerAgentPort> = Arc::new(KernelWorkerPort::new(composed, sink));
    let anchors = Arc::new(SessionAnchors::new());
    let tools: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::clone(&anchors),
        Arc::new(ProcessStaticCheck::new()),
    ));
    let diff: Arc<dyn DiffContextPort> = Arc::new(GitDiffSource::new());
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.clone()));
    let request = RunRequest {
        root,
        change: change.clone(),
        run_id: run_id.clone(),
    };

    // 提前 resolve：run_id 立即可知，运行态经 Channel 流出（订阅先行于 walker 启动）
    let updates = control
        .subscribe(&change)
        .ok_or_else(|| "run 订阅失败（注册表条目缺失）".to_owned())?;
    spawn_channel_forward(on_event, updates);
    tauri::async_runtime::spawn(async move {
        let _ = walk_run(worker, tools, diff, snapshot, control, guard, request).await;
    });
    Ok(ChangeRunSummary {
        run_id,
        status: ChangeRunStatus::Running,
    })
}

#[tauri::command]
#[specta::specta]
pub fn change_flow_stop(app: AppHandle, root: String, change: String) -> Result<(), String> {
    change_flow_stop_with(app, root, change)
}

/// [`change_flow_stop`] 的泛型测试缝。
pub(crate) fn change_flow_stop_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    change: String,
) -> Result<(), String> {
    if is_blank(&root) || is_blank(&change) {
        // miss 幂等：与 agent_stop 同口径，blank root / change 无副作用直接成功
        return Ok(());
    }
    let control = app.state::<Arc<ChangeFlowControl>>();
    if control.request_stop(&change) {
        // 当前 WorkerAgent 会话经既有 StopRegistry 请求终止（miss 幂等）
        if let Some(session_id) = control.current_session(&change) {
            let registry = app.state::<Arc<StopRegistry>>();
            registry.request_stop(&session_id);
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_flow_answer(
    app: AppHandle,
    root: String,
    change: String,
    answer: String,
) -> Result<(), String> {
    change_flow_answer_with(app, root, change, answer)
}

/// [`change_flow_answer`] 的泛型测试缝。
pub(crate) fn change_flow_answer_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    change: String,
    answer: String,
) -> Result<(), String> {
    if is_blank(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if is_blank(&change) {
        return Err("非法 change: 不得为空白".to_owned());
    }
    app.state::<Arc<ChangeFlowControl>>()
        .answer(&change, answer)
}

#[tauri::command]
#[specta::specta]
pub fn change_flow_confirm(
    app: AppHandle,
    root: String,
    change: String,
    proceed: bool,
) -> Result<(), String> {
    change_flow_confirm_with(app, root, change, proceed)
}

/// [`change_flow_confirm`] 的泛型测试缝。
pub(crate) fn change_flow_confirm_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    change: String,
    proceed: bool,
) -> Result<(), String> {
    if is_blank(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if is_blank(&change) {
        return Err("非法 change: 不得为空白".to_owned());
    }
    app.state::<Arc<ChangeFlowControl>>()
        .confirm(&change, proceed)
}

#[tauri::command]
#[specta::specta]
pub fn change_flow_state(
    app: AppHandle,
    root: String,
    change: String,
) -> Result<Option<ChangeRunSnapshot>, String> {
    change_flow_state_with(app, root, change)
}

/// [`change_flow_state`] 的泛型测试缝。
pub(crate) fn change_flow_state_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    change: String,
) -> Result<Option<ChangeRunSnapshot>, String> {
    if is_blank(&root) || is_blank(&change) {
        return Ok(None);
    }
    Ok(app.state::<Arc<ChangeFlowControl>>().snapshot(&change))
}

#[tauri::command]
#[specta::specta]
pub fn change_flow_watch(
    app: AppHandle,
    on_event: Channel<RunUpdate>,
    root: String,
    change: String,
) -> Result<(), String> {
    change_flow_watch_with(app, on_event, root, change)
}

/// [`change_flow_watch`] 的泛型测试缝：运行中视图重挂后的 broadcast 补订。
pub(crate) fn change_flow_watch_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<RunUpdate>,
    root: String,
    change: String,
) -> Result<(), String> {
    if is_blank(&root) || is_blank(&change) {
        return Ok(());
    }
    let control = app.state::<Arc<ChangeFlowControl>>();
    match control.subscribe(&change) {
        Some(updates) => {
            spawn_channel_forward(on_event, updates);
            Ok(())
        }
        // 无运行 run：非错误（重挂时 run 可能已收口，图回落派生规则）
        None => Ok(()),
    }
}

/// broadcast → Channel 转发任务（滞后丢事件即断流：前端重挂快照兜底）。
fn spawn_channel_forward(
    on_event: Channel<RunUpdate>,
    mut updates: tokio::sync::broadcast::Receiver<RunUpdate>,
) {
    tauri::async_runtime::spawn(async move {
        while let Ok(update) = updates.recv().await {
            if on_event.send(update).is_err() {
                break;
            }
        }
    });
}
