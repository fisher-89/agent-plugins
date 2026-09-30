//! agent 运行编排（app 层微形态，与 `commands::workspaces` 的 `*_inner`
//! 同列）：[`start_agent_run`] 同步段（组装 runner → start → begin 落
//! running 行 → 注册停止句柄 → spawn 后台任务 → 提前 resolve running 记
//! 录），加 [`drive_agent_run`] 后台任务体（事件流 tee 双 sink → EOF 收敛
//! → 终态落库 → Channel 流出 Record → 注册表除名）。
//!
//! 提前 resolve 契约（能力 spec `specs/desktop-agent-execution/spec.md`，
//! 路径相对域根）：run id 在 begin 落库时即可用，命令在落库后立即返回
//! running 记录，执行转后台任务继续；终态记录经 Channel 以
//! [`AgentRunMessage::Record`] 信封流出，MUST NOT 再依赖 invoke 返回携带终态。
//!
//! 泛型缝 [`start_agent_run_with`] 让编排路径可被假 runner 测试（`AppHandle`
//! 对 runtime 泛型：生产经命令注入 Wry 句柄，测试注入 `tauri::test` 的
//! MockRuntime 句柄）；[`drive_agent_run`] 以 `&Store` + `&RunStopRegistry`
//! 入参保持可注入。本文件独立于 mod.rs，将来抽 app crate 时单文件平移复用、
//! 不重写。
//!
//! tee 循环节奏（背压策略）：`recv → 状态机 apply → store 逐事件单事务追加
//! → Channel 发送`。Channel 发送失败（页面已关闭）不中断落库；store 写入
//! 失败立即收敛 run 为 failed（error 记因）、尽力流出 Record 并终止 tee——
//! 落库是兜底路径，失败不可静默。
//!
//! EOF 收敛优先级：状态机已收敛（RunResult 驱动）以状态机为准；否则停止
//! 信号已置位 → 显式收敛 stopped（停止路径泵不合成 error_process_exit，
//! 状态机不被驱动成 failed）；兜底 failed 记因（进程异常终止无 result）。

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime};

use agent::{
    AgentEnvMode, AgentEvent, AgentEventKind, AgentRun, AgentRunParams, AgentRunState,
    AgentRunStatus, AgentRunner, RunHandle, RunStateMachine,
};
use agent_cli::ClaudeCliRunner;
use store::{AgentRunRecord, Store, WorkspaceStores};

/// `agent_start` Channel 的消息信封（app 层 IPC 类型，非 core 契约）：实时
/// 事件与终态记录双变体，tag `ipc` 判别（TS 镜像放 transport，camelCase
/// 对齐）。信封不含 record 语义——终态记录塞进事件 usage 是反模式。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "ipc", rename_all = "camelCase")]
pub enum AgentRunMessage {
    /// 实时事件（落库与流出同源同构）
    Event { event: AgentEvent },
    /// 终态 run 记录（提前 resolve 契约的终态流出半边）
    Record { record: AgentRunRecord },
}

/// 运行中 run 的停止句柄注册表（托管状态，与 `WatchRegistry` 同型）：
/// `(root, run id)` 复合键 → 逻辑终止信号句柄。run id 为 workspace 库域内
/// 自增，裸 id 跨库有歧义（双 workspace 同 id 并行），寻址键随 root 消解。
/// `agent_start` 登记、`drive_agent_run` 终态除名、`agent_stop` 查询；内存态
/// 与进程同生命周期（应用重启即清空）。
#[derive(Default)]
pub struct RunStopRegistry {
    handles: Mutex<HashMap<(String, i64), RunHandle>>,
}

impl RunStopRegistry {
    /// 登记运行中 run 的停止句柄（begin 落库分配 id 后立即注册）。
    pub(crate) fn register(&self, root: &str, run_id: i64, handle: RunHandle) {
        self.handles
            .lock()
            .expect("停止注册表锁不可中毒")
            .insert((root.to_owned(), run_id), handle);
    }

    /// 按 `(root, run id)` 寻址置位停止信号；命中返回 true，非 running /
    /// 不存在返回 false（`agent_stop` 幂等忽略，不报错——blank root 天然
    /// miss，无副作用直接成功）。
    pub(crate) fn request_stop(&self, root: &str, run_id: i64) -> bool {
        let handles = self.handles.lock().expect("停止注册表锁不可中毒");
        match handles.get(&(root.to_owned(), run_id)) {
            Some(handle) => {
                handle.request_stop();
                true
            }
            None => false,
        }
    }

    /// 终态除名（收敛落库后调用；除名后 `agent_stop` 对该键幂等忽略）。
    pub(crate) fn remove(&self, root: &str, run_id: i64) {
        self.handles
            .lock()
            .expect("停止注册表锁不可中毒")
            .remove(&(root.to_owned(), run_id));
    }
}

/// run 记录来源与链字段（app 层微形态，store 记录面元数据）：`source` 来源
/// 受控字符串、`source_ref` 来源内定位、`parent_run_id` 链上游 run。编排
/// 填充点单点——`running` 记录初值即携带，终态替换不改写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunProvenance {
    /// 来源受控字符串（debug | explore | …）
    pub source: String,
    /// 来源内定位（explore 指向探索记录主键的十进制串）
    pub source_ref: Option<String>,
    /// 链上游 run id（链首为 None）
    pub parent_run_id: Option<i64>,
}

impl RunProvenance {
    /// 调试链路缺省来源（不携带定位与链指针，与演进前写入语义一致）。
    pub(crate) fn debug() -> Self {
        Self {
            source: "debug".to_owned(),
            source_ref: None,
            parent_run_id: None,
        }
    }
}

/// `RunResult` 汇总字段摘取（终态记录填充用；is_error 由状态机承载，不在此重复）。
struct RunSummary {
    num_turns: Option<u64>,
    cost_usd: Option<f64>,
    duration_ms: Option<u64>,
    session_id: Option<String>,
}

/// UTC unix 毫秒：std 唯一时间源（时钟早于 epoch 时取 0，不 panic）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// running 记录初值（id 由 begin 分配）：初值携带 provenance 来源与链字段。
/// 三字段直写枚举（core/agent 契约值域，无字符串降级）；env 恒为完整档
/// （bare 纯净档不进本编排的 IPC 面，无 UI 输入口）。
fn running_record(params: &AgentRunParams, provenance: RunProvenance) -> AgentRunRecord {
    AgentRunRecord {
        id: 0,
        prompt: params.prompt.clone(),
        cwd: params.cwd.to_string_lossy().into_owned(),
        env: AgentEnvMode::Default,
        permission_mode: params.permission_mode,
        status: AgentRunStatus::Running,
        started_at: now_millis(),
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
        source: provenance.source,
        source_ref: provenance.source_ref,
        parent_run_id: provenance.parent_run_id,
    }
}

/// 薄入口：组装真实 CLI runner 后委托 [`start_agent_run_with`]。
pub(crate) fn start_agent_run<T: Runtime>(
    app: AppHandle<T>,
    stores: &WorkspaceStores,
    on_event: Channel<AgentRunMessage>,
    params: AgentRunParams,
    provenance: RunProvenance,
) -> Result<AgentRunRecord, String> {
    let runner = ClaudeCliRunner::new();
    start_agent_run_with(app, stores, &runner, on_event, params, provenance)
}

/// 泛型编排同步段：runner 启动（启动阶段失败 → `Err`，不留 run 行）→ 按
/// root 预解析所属 workspace 库（`for_root`，同步段完成后 `Arc<Store>` 供
/// 后台任务持有收尾——库实例跨 await 稳定借用，不经 AppHandle 二次取 State）
/// → begin 落 `running` 行（初值携带 provenance 来源与链字段）→ 注册停止句
/// 柄（`(root, run id)` 复合键）→ spawn [`drive_agent_run`] 后台任务 → 立即
/// 返回 running 记录（提前 resolve；终态经 Channel 流出）。runtime 泛型仅为
/// 测试注入 MockRuntime 句柄，生产命令面解析为 Wry。
pub(crate) fn start_agent_run_with<R, T>(
    app: AppHandle<T>,
    stores: &WorkspaceStores,
    runner: &R,
    on_event: Channel<AgentRunMessage>,
    params: AgentRunParams,
    provenance: RunProvenance,
) -> Result<AgentRunRecord, String>
where
    R: AgentRunner + 'static,
    T: Runtime,
{
    let running = running_record(&params, provenance);
    // cwd 恒为当前 workspace root（前端固定传 root，见能力 spec），即注册表
    // 复合键的 root 分量
    let root = running.cwd.clone();
    let run = runner.start(params).map_err(|e| e.to_string())?;
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let record = store.begin_agent_run(&running).map_err(|e| e.to_string())?;
    app.state::<RunStopRegistry>()
        .register(&root, record.id, run.handle.clone());
    let app = app.clone();
    let task_record = record.clone();
    // 后台任务走 tauri 异步运行时（命令层不直依赖 tokio；spawn 语义等价）
    tauri::async_runtime::spawn(async move {
        let registry = app.state::<RunStopRegistry>();
        drive_agent_run(&store, on_event, run, task_record, registry.inner()).await;
    });
    Ok(record)
}

/// 后台任务体：事件流 tee 双 sink（store 逐事件单事务追加 + Channel 实时
/// 流出）→ EOF 按状态机/停止信号收敛终态并落库 → 注册表除名 → Channel 流出
/// 终态 Record 信封 → 返回最终记录。in-band 失败（is_error result）返回
/// failed 记录；store 写失败收敛 failed、尽力流出 Record 后终止。
pub(crate) async fn drive_agent_run(
    store: &Store,
    on_event: Channel<AgentRunMessage>,
    mut run: AgentRun,
    mut record: AgentRunRecord,
    registry: &RunStopRegistry,
) -> AgentRunRecord {
    let mut machine = RunStateMachine::new();
    let mut summary: Option<RunSummary> = None;

    while let Some(event) = run.events.recv().await {
        machine.apply(&event);
        if let AgentEventKind::RunResult {
            num_turns,
            cost_usd,
            duration_ms,
            session_id,
            ..
        } = &event.kind
        {
            summary = Some(RunSummary {
                num_turns: *num_turns,
                cost_usd: *cost_usd,
                duration_ms: *duration_ms,
                session_id: session_id.clone(),
            });
        }
        // store sink（兜底路径）：类型化事件直写；失败立即收敛 failed、
        // 尽力流出 Record 并终止 tee（落库失败不可静默）
        if let Err(store_error) =
            store.append_agent_run_events(record.id, std::slice::from_ref(&event))
        {
            let failed =
                abort_with_store_failure(store, record, format!("事件落库失败: {store_error}"));
            registry.remove(&failed.cwd, failed.id);
            let _ = on_event.send(AgentRunMessage::Record {
                record: failed.clone(),
            });
            return failed;
        }
        // Channel sink（实时流）：发送失败（页面已关闭）不中断落库
        let _ = on_event.send(AgentRunMessage::Event { event });
    }

    // EOF：状态机已收敛以状态机为准（首个收敛生效）；否则停止信号已置位 →
    // 显式收敛 stopped（error 不记因——用户主动终止非失败）；兜底 failed 记因
    record.status = match machine.current() {
        AgentRunState::Completed => AgentRunStatus::Completed,
        AgentRunState::Failed => AgentRunStatus::Failed,
        AgentRunState::Running if run.handle.stop_requested() => {
            machine.stop();
            AgentRunStatus::Stopped
        }
        AgentRunState::Running | AgentRunState::Stopped => {
            record.error = Some("进程结束但未产出 result 事件".to_owned());
            AgentRunStatus::Failed
        }
    };
    if let Some(result) = summary {
        record.num_turns = result.num_turns;
        record.cost_usd = result.cost_usd;
        record.duration_ms = result.duration_ms;
        record.session_id = result.session_id;
    }
    record.finished_at = Some(now_millis());
    if let Err(store_error) = store.finish_agent_run(record.id, &record) {
        // 终态落库失败同样不可静默：error 记因后仍尽力流出 Record
        record.error = Some(format!("终态落库失败: {store_error}"));
    }
    // 除名键 root 取 record.cwd（归属键同源：注册时 cwd 即当前 workspace root）
    registry.remove(&record.cwd, record.id);
    let _ = on_event.send(AgentRunMessage::Record {
        record: record.clone(),
    });
    record
}

/// store 写失败的失败收敛：run 收敛为 failed、error 记因，并尽力落终态行
/// （终态落库也失败时，错误串并入 error 字段保留，不再向上传播）。
fn abort_with_store_failure(
    store: &Store,
    mut record: AgentRunRecord,
    cause: String,
) -> AgentRunRecord {
    record.status = AgentRunStatus::Failed;
    record.error = Some(cause);
    record.finished_at = Some(now_millis());
    if let Err(store_error) = store.finish_agent_run(record.id, &record) {
        record.error = Some(format!(
            "{}（终态落库亦失败: {store_error}）",
            record.error.clone().unwrap_or_default()
        ));
    }
    record
}

#[cfg(test)]
#[path = "agent_test.rs"]
pub(crate) mod agent_test;
