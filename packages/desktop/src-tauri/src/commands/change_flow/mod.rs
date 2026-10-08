use std::path::Path;
use std::sync::Arc;

#[cfg(test)]
mod mod_test;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use ::agent::StopRegistry;
use agent_runtime::{compose_turn, ComposedTurn, KernelWorkerPort};
use checks_runtime::{ProcessStaticCheck, ProcessTestExecution};
use orchestration::control::ChangeFlowControl;
use orchestration::port::{RunEventSink, ToolStepPort, WorkerAgentPort, WorkflowSnapshotPort};
use orchestration::snapshot::StoreSnapshot;
use orchestration::state::{ChangeRunSnapshot, ChangeRunStatus, ChangeRunSummary, RunUpdate};
use orchestration::steps::LocalToolSteps;
use orchestration::walker::{new_run_id, walk_run, RunRequest};
use store::WorkspaceStores;
use workflow::state::ChangeStateStore;
use workflow::write::{phase_table, SessionAnchors};

/// 参数显式格式检查：空/空白串不进入库解析 / 注册表链路（与 exec 轨道同
/// 口径，本模块内聚一份避免跨命令组暴露私有件）。
fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// run 事件出口的注册表桥：WorkerAgent 会话事件（早于 turn 收口）据此先行
/// 同步 run 会话槽（停止寻址）与广播总线（Channel 由订阅转发任务回流）。
/// 复合键（workspace root, change）随行（D10）。
struct ChangeFlowSink {
    root: String,
    change: String,
    control: Arc<ChangeFlowControl>,
}

impl RunEventSink for ChangeFlowSink {
    fn emit(&self, update: RunUpdate) {
        if let RunUpdate::SessionEvent { session_id, .. } = &update {
            self.control
                .set_session(&self.root, &self.change, Some(session_id.clone()));
        }
        self.control.publish(&self.root, &self.change, update);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn change_flow_start(
    app: AppHandle,
    on_event: Channel<RunUpdate>,
    root: String,
    change: String,
    auto_next_phase: bool,
) -> Result<ChangeRunSummary, String> {
    change_flow_start_with(app, on_event, root, change, auto_next_phase).await
}

/// [`change_flow_start`] 的泛型测试缝（生产注入 Wry 句柄、测试注入
/// MockRuntime 句柄，沿 `agent_start_with` 先例）。
pub(crate) async fn change_flow_start_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<RunUpdate>,
    root: String,
    change: String,
    auto_next_phase: bool,
) -> Result<ChangeRunSummary, String> {
    if is_blank(&root) {
        return Err("非法 root: 不得为空白（无 cwd 无从发起）".to_owned());
    }
    if is_blank(&change) {
        return Err("非法 change: 不得为空白（无 change 无从发起）".to_owned());
    }
    // 前置校验 1：目标 change 已建档（db `ChangeRecord` 在案；存量 CLI change
    // 无建档不可发起——文档形态 change 不可运行，显式拒绝）
    let stores = app.state::<WorkspaceStores>();
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let record = store
        .find_change_record(&change)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("change \"{change}\" 未建档（无 ChangeRecord），无从编排"))?;
    // 前置校验 2：workflow_type=requirement（写面相位表 None → 显式拒绝；
    // V1 范围显式拒绝 bug-fix / test-only，优于相位机半途报错）
    if phase_table(&record.workflow_type).is_none() {
        return Err(format!(
            "仅支持 requirement 工作流（change \"{change}\" 的 workflow_type 为 \"{}\"）",
            record.workflow_type
        ));
    }
    // 前置校验 3：exec root 解析（design D9 双 root 拆分）——record.worktree
    // 非 None → 该绝对路径为 exec root（会话 cwd / 工件根 / 检查器执行目录），
    // is_dir 存在性校验（worktree 被手动删除后发起点一行显式 Err 优于深埋在
    // 引擎 cwd 的失败）；None → 主 workspace root（legacy 零变化）。store 半
    // 边恒 workspace root（双 root 不变量——worktree 路径不进 `for_root`）
    let exec_root = match record.worktree.as_deref() {
        Some(worktree) if Path::new(worktree).is_dir() => worktree.to_owned(),
        Some(worktree) => {
            return Err(format!(
                "worktree 目录不存在（可能已被手动删除）: {worktree}；\
                 请恢复目录或手动清理后重建 change"
            ));
        }
        None => root.clone(),
    };
    // 前置校验 4：无并行 run（begin_run 复合键冲突检测——同 workspace 同
    // change 二次发起 Err，异 workspace 同名不误拒）
    let control = Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner());
    let run_id = new_run_id();
    let guard = control.begin_run(&root, &change, run_id.clone())?;

    // 组合根装配（run 作用域一次）：组合 turn + 三 port + 快照源 + 事件桥
    // + run 级会话锚点（每 run 一个实例，W7）+ store 缝（写面落库 / 快照
    // db 读源共用 `for_root` 实例；compose 注入 workspace root 实例——cwd
    // 半边经 turn 通道携带 exec root）
    let registry = Arc::clone(app.state::<Arc<StopRegistry>>().inner());
    let composed: ComposedTurn = compose_turn(
        stores.inner(),
        Arc::clone(&registry),
        Arc::clone(&store),
        None,
    )?;
    let sink: Arc<dyn RunEventSink> = Arc::new(ChangeFlowSink {
        root: root.clone(),
        change: change.clone(),
        control: Arc::clone(&control),
    });
    let worker: Arc<dyn WorkerAgentPort> = Arc::new(KernelWorkerPort::new(composed, sink));
    let anchors = Arc::new(SessionAnchors::new());
    let store_port: Arc<dyn ChangeStateStore> = store; // Arc<Store> → port 缝对象
    let tools: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::clone(&anchors),
        Arc::new(ProcessStaticCheck::new()),
        Arc::new(ProcessTestExecution::new()),
        Arc::clone(&store_port),
        run_id.clone(),
    ));
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StoreSnapshot::new(
        exec_root.clone(),
        Arc::clone(&store_port),
    ));
    let request = RunRequest {
        root: exec_root,
        change: change.clone(),
        run_id: run_id.clone(),
        auto_next_phase,
    };

    // 提前 resolve：run_id 立即可知，运行态经 Channel 流出（订阅先行于 walker 启动）
    let updates = control
        .subscribe(&root, &change)
        .ok_or_else(|| "run 订阅失败（注册表条目缺失）".to_owned())?;
    spawn_channel_forward(on_event, updates);
    tauri::async_runtime::spawn(async move {
        let _ = walk_run(worker, tools, snapshot, control, guard, request).await;
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
    if control.request_stop(&root, &change) {
        // 当前 WorkerAgent 会话经既有 StopRegistry 请求终止（miss 幂等）
        if let Some(session_id) = control.current_session(&root, &change) {
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
        .answer(&root, &change, answer)
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
        .confirm(&root, &change, proceed)
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
    Ok(app
        .state::<Arc<ChangeFlowControl>>()
        .snapshot(&root, &change))
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
    match control.subscribe(&root, &change) {
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
