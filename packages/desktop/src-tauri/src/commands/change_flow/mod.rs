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
use orchestration::port::{
    RunEventSink, RunHistoryPort, ToolStepPort, WorkerAgentPort, WorkflowSnapshotPort,
};
use orchestration::run_history::StoreRunHistory;
use orchestration::snapshot::StoreSnapshot;
use orchestration::state::{ChangeRunStatus, ChangeRunSummary, RunNotice, RunUpdate};
use orchestration::steps::LocalToolSteps;
use orchestration::walker::{new_run_id, walk_run, RunRequest};
use store::WorkspaceStores;
use workflow::state::ChangeStateStore;
use workflow::write::{phase_table, SessionAnchors};

/// 当前 UTC unix 毫秒（run 发起时刻铸造点；时钟早于 epoch 取 0，不 panic——
/// `begin_run` / `RunRequest.started_at` 同值，corpus 确定性由测试注入承载）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// 参数显式格式检查：空/空白串不进入库解析 / 注册表链路（与 exec 轨道同
/// 口径，本模块内聚一份避免跨命令组暴露私有件）。
fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// run 事件出口的注册表桥：WorkerAgent 会话事件（早于 turn 收口）据此先行
/// 同步 run 会话槽（停止寻址）与广播总线（Channel 由订阅转发任务回流）。
/// 复合键（workspace root, change id）随行（D10）。
struct ChangeFlowSink {
    root: String,
    change_id: String,
    control: Arc<ChangeFlowControl>,
}

impl RunEventSink for ChangeFlowSink {
    fn emit(&self, update: RunUpdate) {
        if let RunUpdate::SessionEvent { session_id, .. } = &update {
            self.control
                .set_session(&self.root, &self.change_id, Some(session_id.clone()));
        }
        self.control.publish(&self.root, &self.change_id, update);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn change_flow_start(
    app: AppHandle,
    on_event: Channel<RunNotice>,
    root: String,
    id: String,
    auto_next_phase: bool,
) -> Result<ChangeRunSummary, String> {
    change_flow_start_with(app, on_event, root, id, auto_next_phase).await
}

/// [`change_flow_start`] 的泛型测试缝（生产注入 Wry 句柄、测试注入
/// MockRuntime 句柄，沿 `agent_start_with` 先例）。
pub(crate) async fn change_flow_start_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<RunNotice>,
    root: String,
    id: String,
    auto_next_phase: bool,
) -> Result<ChangeRunSummary, String> {
    if is_blank(&root) {
        return Err("非法 root: 不得为空白（无 cwd 无从发起）".to_owned());
    }
    if is_blank(&id) {
        return Err("非法 id: 不得为空白（无 change 无从发起）".to_owned());
    }
    // 前置校验 1：目标 change 已建档（db `ChangeRecord` 在案，按 id 读记录；
    // 未建档不可发起，显式拒绝）
    let stores = app.state::<WorkspaceStores>();
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let record = store
        .find_change_record(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("change \"{id}\" 未建档（无 ChangeRecord），无从编排"))?;
    // 前置校验 2：workflow_type=requirement（写面相位表 None → 显式拒绝；
    // V1 范围显式拒绝 bug-fix / test-only，优于相位机半途报错）
    if phase_table(&record.workflow_type).is_none() {
        return Err(format!(
            "仅支持 requirement 工作流（change \"{}\" 的 workflow_type 为 \"{}\"）",
            record.name, record.workflow_type
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
    // change 二次发起 Err，异 workspace 同名不误拒）。先经反向互斥面：归档链
    // 进行中（ArchiveControl 在案）显式拒绝 run 发起（design
    // desktop-archive-change D3——互斥登记载体 = ArchiveControl 自持注册表，
    // 不进入运行态零落账）
    let control = Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner());
    if app
        .state::<Arc<orchestration::archive_flow::ArchiveControl>>()
        .is_active(&root, &id)
    {
        return Err(format!(
            "change \"{}\" 的归档链进行中，不可发起 run（请等待归档收口或先停止归档链）",
            record.name
        ));
    }
    let run_id = new_run_id();
    // 发起时刻铸造（run 运行史 started_at / 注册表条目同值——corpus 确定性
    // 由命令携带时间戳保证，design D5）
    let started_at = now_millis();
    let guard = control.begin_run(&root, &id, run_id.clone(), started_at)?;

    // 组合根装配（run 作用域一次）：组合 turn + 三 port + 快照源 + 事件桥
    // + run 级会话锚点（每 run 一个实例，W7）+ store 缝（写面落库 / 快照
    // db 读源共用 `for_root` 实例；compose 注入 workspace root 实例——cwd
    // 半边经 turn 通道携带 exec root）+ run 落库缝（每 run 两写：
    // walk_run 起点第一写 running 行 / 终态出口第二写整包，D5）
    let registry = Arc::clone(app.state::<Arc<StopRegistry>>().inner());
    let composed: ComposedTurn = compose_turn(
        stores.inner(),
        Arc::clone(&registry),
        Arc::clone(&store),
        None,
    )?;
    let sink: Arc<dyn RunEventSink> = Arc::new(ChangeFlowSink {
        root: root.clone(),
        change_id: id.clone(),
        control: Arc::clone(&control),
    });
    let worker: Arc<dyn WorkerAgentPort> = Arc::new(KernelWorkerPort::new(composed, sink));
    let anchors = Arc::new(SessionAnchors::new());
    let store_port: Arc<dyn ChangeStateStore> = store; // Arc<Store> → port 缝对象
    let history: Arc<dyn RunHistoryPort> = Arc::new(StoreRunHistory::new(Arc::clone(&store_port)));
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
        change_id: id.clone(),
        run_id: run_id.clone(),
        auto_next_phase,
        started_at,
    };

    // 提前 resolve：run_id 立即可知，变更通知经 Channel 流出（订阅先行于
    // walker 启动；kind-only Notice——通知仅失效信号，查询结果权威）
    let updates = control
        .subscribe(&root, &id)
        .ok_or_else(|| "run 订阅失败（注册表条目缺失）".to_owned())?;
    spawn_channel_forward(on_event, updates);
    tauri::async_runtime::spawn(async move {
        let _ = walk_run(worker, tools, snapshot, history, control, guard, request).await;
    });
    Ok(ChangeRunSummary {
        run_id,
        status: ChangeRunStatus::Running,
    })
}

#[tauri::command]
#[specta::specta]
pub fn change_flow_stop(app: AppHandle, root: String, id: String) -> Result<(), String> {
    change_flow_stop_with(app, root, id)
}

/// [`change_flow_stop`] 的泛型测试缝。
pub(crate) fn change_flow_stop_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    id: String,
) -> Result<(), String> {
    if is_blank(&root) || is_blank(&id) {
        // miss 幂等：与 agent_stop 同口径，blank root / id 无副作用直接成功
        return Ok(());
    }
    let control = app.state::<Arc<ChangeFlowControl>>();
    if control.request_stop(&root, &id) {
        // 当前 WorkerAgent 会话经既有 StopRegistry 请求终止（miss 幂等）
        if let Some(session_id) = control.current_session(&root, &id) {
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
    id: String,
    answer: String,
) -> Result<(), String> {
    change_flow_answer_with(app, root, id, answer)
}

/// [`change_flow_answer`] 的泛型测试缝。
pub(crate) fn change_flow_answer_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    id: String,
    answer: String,
) -> Result<(), String> {
    if is_blank(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if is_blank(&id) {
        return Err("非法 id: 不得为空白".to_owned());
    }
    app.state::<Arc<ChangeFlowControl>>()
        .answer(&root, &id, answer)
}

#[tauri::command]
#[specta::specta]
pub fn change_flow_confirm(
    app: AppHandle,
    root: String,
    id: String,
    proceed: bool,
) -> Result<(), String> {
    change_flow_confirm_with(app, root, id, proceed)
}

/// [`change_flow_confirm`] 的泛型测试缝。
pub(crate) fn change_flow_confirm_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    id: String,
    proceed: bool,
) -> Result<(), String> {
    if is_blank(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if is_blank(&id) {
        return Err("非法 id: 不得为空白".to_owned());
    }
    app.state::<Arc<ChangeFlowControl>>()
        .confirm(&root, &id, proceed)
}

#[tauri::command]
#[specta::specta]
pub fn change_flow_watch(
    app: AppHandle,
    on_event: Channel<RunNotice>,
    root: String,
    id: String,
) -> Result<(), String> {
    change_flow_watch_with(app, on_event, root, id)
}

/// [`change_flow_watch`] 的泛型测试缝：运行中视图重挂后的 broadcast 补订
///（重挂恢复 = 统一查询 `get_change_detail` activeRun 面先行，本命令补订
/// 实时通知——D4，快照命令已退役）。
pub(crate) fn change_flow_watch_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<RunNotice>,
    root: String,
    id: String,
) -> Result<(), String> {
    if is_blank(&root) || is_blank(&id) {
        return Ok(());
    }
    let control = app.state::<Arc<ChangeFlowControl>>();
    match control.subscribe(&root, &id) {
        Some(updates) => {
            spawn_channel_forward(on_event, updates);
            Ok(())
        }
        // 无运行 run：非错误（重挂时 run 可能已收口，图读史常驻派生）
        None => Ok(()),
    }
}

/// broadcast → Channel 转发任务（滞后丢通知即断流：前端重查统一视图兜底；
/// kind-only Notice 零载荷——通知仅失效信号）。
fn spawn_channel_forward(
    on_event: Channel<RunNotice>,
    mut updates: tokio::sync::broadcast::Receiver<RunNotice>,
) {
    tauri::async_runtime::spawn(async move {
        while let Ok(notice) = updates.recv().await {
            if on_event.send(notice).is_err() {
                break;
            }
        }
    });
}
