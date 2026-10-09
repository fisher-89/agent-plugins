//! 归档编排流命令组（design D1）：五命令薄包装——`archive_flow_preflight`
//!（确认对话读面）、`archive_flow_start`（提前 resolve + Channel 同构流）、
//! `archive_flow_stop`、`archive_flow_state`（重挂快照）、`archive_flow_watch`
//!（broadcast 补订）。命令组三分：changes（读 + 记录面）、change_flow（run
//! 编排控制）、archive_flow（归档编排流——带 agent 会话的第三面，与 run 面
//! 并置但组件隔离；链内段序 = 校验 → 提交 → 合入（含冲突 agent）→ 同步 →
//! 双写 → 落盘，archive-merge-first D1）。
//!
//! 互斥（design D3）：正向——run 注册表在案（`ChangeFlowControl::snapshot`）
//! 显式拒绝归档发起；反向见 `change_flow_start` 前置序列（归档链进行中拒
//! run 发起）。既有裸双写 `archive_change` 保留零改动（互斥不圈裸命令——
//! V1 边界，用户自担）。三件事纪律：`Result<T, String>`。
//!
//! [`ArchiveSink`]（design D4）：worker.rs 复用通道的 `RunUpdate::SessionEvent`
//! 即时转译 `ArchiveUpdate::SessionEvent` 发布归档 broadcast + 同步会话槽
//!（停止寻址）——run 注册表与 run Channel 零写入（run 面隔离红线按其
//! scenario 语义执行）。

use std::path::Path;
use std::sync::Arc;

#[cfg(test)]
mod mod_test;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use ::agent::StopRegistry;
use agent_runtime::{compose_turn, ComposedTurn, KernelWorkerPort};
use orchestration::archive_flow::{
    preflight, run_archive_flow, ArchiveControl, ArchiveGuard, ArchivePreflight, ArchiveRequest,
    ArchiveSnapshot, ArchiveUpdate,
};
use orchestration::control::ChangeFlowControl;
use orchestration::port::{ArchiveVcsPort, RunEventSink, WorkerAgentPort};
use orchestration::state::RunUpdate;
use store::WorkspaceStores;
use vcs_runtime::ProcessArchiveVcs;
use workflow::state::ChangeStateStore;

/// 参数显式格式检查：空/空白串不进入库解析 / 注册表链路（与 change_flow 组
/// 同口径，本模块内聚一份）。
fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// 归档链事件出口的注册表桥：WorkerAgent 会话事件（早于 turn 收口）据此先行
/// 同步归档会话槽（停止寻址）与广播总线（Channel 由订阅转发任务回流）；
/// 其余 run 信封变体不外泄（归档面零 RunUpdate 语义）。复合键
///（workspace root, change）随行。
struct ArchiveSink {
    root: String,
    change: String,
    control: Arc<ArchiveControl>,
}

impl RunEventSink for ArchiveSink {
    fn emit(&self, update: RunUpdate) {
        if let RunUpdate::SessionEvent { session_id, event } = update {
            self.control
                .set_session(&self.root, &self.change, Some(session_id.clone()));
            self.control.publish(
                &self.root,
                &self.change,
                ArchiveUpdate::SessionEvent { session_id, event },
            );
        }
    }
}

/// 归档前置读面（确认对话数据面）：blank root / change 早退 `None`（读语义）；
/// `None` = 不可归档（未建档 / 已归档 / 未知名——前端据此不呈现入口路径的
/// 兜底）。run_active 自 run 注册表快照；merge_target 自 worktree 记录在场
/// 才探测的真实 git。
#[tauri::command]
#[specta::specta]
pub fn archive_flow_preflight(
    app: AppHandle,
    root: String,
    change: String,
) -> Option<ArchivePreflight> {
    archive_flow_preflight_with(app, root, change)
}

/// [`archive_flow_preflight`] 的泛型测试缝（生产注入 Wry 句柄、测试注入
/// MockRuntime 句柄）。
pub(crate) fn archive_flow_preflight_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    change: String,
) -> Option<ArchivePreflight> {
    if is_blank(&root) || is_blank(&change) {
        return None;
    }
    let stores = app.state::<WorkspaceStores>();
    let Ok(store) = stores.for_root(&root) else {
        return None;
    };
    let worktree = store
        .find_change_record(&change)
        .ok()
        .flatten()
        .and_then(|record| record.worktree);
    let run_active = app
        .state::<Arc<ChangeFlowControl>>()
        .snapshot(&root, &change)
        .is_some();
    let vcs = ProcessArchiveVcs::new();
    preflight(
        Path::new(&root),
        store.as_ref(),
        worktree.as_deref(),
        &change,
        &vcs,
        run_active,
    )
}

/// 发起归档链（提前 resolve：接受即 `Ok(true)`，阶段 / 会话事件 / 终态经
/// Channel 流出——agent 同步分钟级，一次性 await 无进度面必致重复点击，
/// design D2）。blank root / change 显式 `Err`；未建档 / 已归档 / run 运行
/// 中 / 链进行中（重入）各显式拒绝——零装配零 spawn。
#[tauri::command]
#[specta::specta]
pub async fn archive_flow_start(
    app: AppHandle,
    on_event: Channel<ArchiveUpdate>,
    root: String,
    change: String,
    sync_specs: bool,
) -> Result<bool, String> {
    archive_flow_start_with(app, on_event, root, change, sync_specs).await
}

/// [`archive_flow_start`] 的泛型测试缝（沿 `change_flow_start_with` 先例）。
pub(crate) async fn archive_flow_start_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<ArchiveUpdate>,
    root: String,
    change: String,
    sync_specs: bool,
) -> Result<bool, String> {
    if is_blank(&root) {
        return Err("非法 root: 不得为空白（无 cwd 无从发起）".to_owned());
    }
    if is_blank(&change) {
        return Err("非法 change: 不得为空白（无 change 无从发起）".to_owned());
    }
    // 前置校验 1：目标 change 已建档且 status=active（文档形态 / 已归档不可
    // 归档——写面既有拒绝面的命令面前置镜像）
    let stores = app.state::<WorkspaceStores>();
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let record = store
        .find_change_record(&change)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("change \"{change}\" 未建档（无 ChangeRecord），无从归档"))?;
    if record.status != workflow::state::ChangeStatus::Active {
        return Err(format!(
            "change \"{change}\" 已归档（status=archived），不可重复归档"
        ));
    }
    // 前置校验 2：无运行中 run（正向互斥——归档链与 run 双向不可并行）
    let run_control = Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner());
    if run_control.snapshot(&root, &change).is_some() {
        return Err(format!(
            "change \"{change}\" 存在运行中的 run，请先停止 run（或等待收口）再发起归档"
        ));
    }
    // 前置校验 3：归档链重入防护（ArchiveControl 在案即拒——重复点击幂等）
    let control = Arc::clone(app.state::<Arc<ArchiveControl>>().inner());
    let guard: ArchiveGuard = control.begin(&root, &change)?;

    // 组合根装配（链作用域一次）：组合 turn + worker port（ArchiveSink 桥）+
    // vcs 执行器 + store port
    let registry = Arc::clone(app.state::<Arc<StopRegistry>>().inner());
    let composed: ComposedTurn = compose_turn(
        stores.inner(),
        Arc::clone(&registry),
        Arc::clone(&store),
        None,
    )?;
    let sink: Arc<dyn RunEventSink> = Arc::new(ArchiveSink {
        root: root.clone(),
        change: change.clone(),
        control: Arc::clone(&control),
    });
    let worker: Arc<dyn WorkerAgentPort> = Arc::new(KernelWorkerPort::new(composed, sink));
    let vcs: Arc<dyn ArchiveVcsPort> = Arc::new(ProcessArchiveVcs::new());
    let store_port: Arc<dyn ChangeStateStore> = store;
    let request = ArchiveRequest {
        root: root.clone(),
        change: change.clone(),
        sync_specs,
    };

    // 提前 resolve：接受即返回，运行态经 Channel 流出（订阅先行于链启动）
    let updates = control
        .subscribe(&root, &change)
        .ok_or_else(|| "归档链订阅失败（注册表条目缺失）".to_owned())?;
    spawn_channel_forward(on_event, updates);
    tauri::async_runtime::spawn(async move {
        run_archive_flow(worker, vcs, store_port, control, guard, request).await;
    });
    Ok(true)
}

/// 停止归档链：取消旗（阶段间检查点收敛）+ 当前 agent 会话经既有 StopRegistry
/// 请求终止（miss 幂等）；blank root / change 零副作用直接成功。
#[tauri::command]
#[specta::specta]
pub fn archive_flow_stop(app: AppHandle, root: String, change: String) -> Result<(), String> {
    archive_flow_stop_with(app, root, change)
}

/// [`archive_flow_stop`] 的泛型测试缝。
pub(crate) fn archive_flow_stop_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    change: String,
) -> Result<(), String> {
    if is_blank(&root) || is_blank(&change) {
        return Ok(());
    }
    let control = app.state::<Arc<ArchiveControl>>();
    if control.request_stop(&root, &change) {
        // 当前归档 agent 会话经既有 StopRegistry 请求终止（miss 幂等）
        if let Some(session_id) = control.current_session(&root, &change) {
            let registry = app.state::<Arc<StopRegistry>>();
            registry.request_stop(&session_id);
        }
    }
    Ok(())
}

/// 归档链重挂快照（进程内；链终态后除名 → `None`——终态由 db / 磁盘事实
/// 承载，详情页 refresh 回归已归档形态）。
#[tauri::command]
#[specta::specta]
pub fn archive_flow_state(app: AppHandle, root: String, change: String) -> Option<ArchiveSnapshot> {
    archive_flow_state_with(app, root, change)
}

/// [`archive_flow_state`] 的泛型测试缝。
pub(crate) fn archive_flow_state_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    change: String,
) -> Option<ArchiveSnapshot> {
    if is_blank(&root) || is_blank(&change) {
        return None;
    }
    app.state::<Arc<ArchiveControl>>().snapshot(&root, &change)
}

/// 归档链 broadcast 补订（运行中视图重挂）；无在案链 `Ok` 非错误（重挂时
/// 链可能已收口）。
#[tauri::command]
#[specta::specta]
pub fn archive_flow_watch(
    app: AppHandle,
    on_event: Channel<ArchiveUpdate>,
    root: String,
    change: String,
) -> Result<(), String> {
    archive_flow_watch_with(app, on_event, root, change)
}

/// [`archive_flow_watch`] 的泛型测试缝。
pub(crate) fn archive_flow_watch_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<ArchiveUpdate>,
    root: String,
    change: String,
) -> Result<(), String> {
    if is_blank(&root) || is_blank(&change) {
        return Ok(());
    }
    let control = app.state::<Arc<ArchiveControl>>();
    match control.subscribe(&root, &change) {
        Some(updates) => {
            spawn_channel_forward(on_event, updates);
            Ok(())
        }
        None => Ok(()),
    }
}

/// broadcast → Channel 转发任务（滞后丢事件即断流：前端重挂快照兜底）。
fn spawn_channel_forward(
    on_event: Channel<ArchiveUpdate>,
    mut updates: tokio::sync::broadcast::Receiver<ArchiveUpdate>,
) {
    tauri::async_runtime::spawn(async move {
        while let Ok(update) = updates.recv().await {
            if on_event.send(update).is_err() {
                break;
            }
        }
    });
}
