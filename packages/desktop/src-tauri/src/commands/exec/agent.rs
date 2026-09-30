use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime};

use agent::{
    AgentEnvMode, AgentEvent, AgentEventKind, AgentRun, AgentRunParams, AgentRunState,
    AgentRunStatus, AgentRunner, RunHandle, RunStateMachine,
};
use agent_runtime::{EngineConfig, EngineFacade, EngineKind, ResumeTranscript};
use store::{AgentEngineKind, AgentRunRecord, Store, WorkspaceStores};

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

/// session_id → 事件转录（store 既有 API 组合，无 store 改动）
fn find_events_by_session(
    store: &Store,
    session_id: &str,
) -> Result<Option<Vec<AgentEvent>>, String> {
    let hit = store
        .list_agent_runs()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|run| run.session_id.as_deref() == Some(session_id));
    match hit {
        Some(run) => store
            .list_agent_run_events(run.id)
            .map(Some)
            .map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

/// 运行发起解析产物（解析单点输出、编排薄入口 [`start_agent_run`] 尾参）：
/// 引擎 kind + 组装好的连接配置。引擎具体类型不出本结构（门面 `runner_for`
/// 签名与编排层零改动承诺的参数传递面）。
pub(crate) struct ResolvedEngine {
    /// 引擎二值（门面 `EngineKind`，自 store 本地 `AgentEngineKind` 映射）
    kind: EngineKind,
    /// 连接配置（sdk 臂由引用 provider 组装；cli 臂 `EngineConfig::empty()`
    /// 占位，CLI 引擎不消费）
    config: EngineConfig,
}

/// 运行发起解析单点
pub(crate) fn resolve_agent_engine(
    stores: &WorkspaceStores,
    agent: Option<i64>,
) -> Result<ResolvedEngine, String> {
    let global = stores.global();
    let instance = match agent {
        None => global
            .default_agent_instance()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                "未设置默认 agent：请前往 Agent 管理页（/agents）配置后再发起".to_owned()
            })?,
        Some(id) => global
            .find_agent_instance(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("agent 不存在: id={id}"))?,
    };
    match instance.engine {
        AgentEngineKind::Sdk => {
            let provider_id = instance.provider_id.ok_or_else(|| {
                format!(
                    "agent {:?} 未配置 provider：请前往 Agent 管理页（/agents）修正",
                    instance.name
                )
            })?;
            let provider = global
                .find_agent_provider(provider_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    format!(
                        "agent {:?} 引用的 provider 不存在: id={provider_id}",
                        instance.name
                    )
                })?;
            Ok(ResolvedEngine {
                kind: EngineKind::Sdk,
                config: EngineConfig {
                    api_key: provider.api_key,
                    base_url: provider.base_url,
                    model: provider.models.high,
                },
            })
        }
        AgentEngineKind::Cli => Ok(ResolvedEngine {
            kind: EngineKind::Cli,
            config: EngineConfig::empty(),
        }),
    }
}

pub(crate) fn start_agent_run<T: Runtime>(
    app: AppHandle<T>,
    stores: &WorkspaceStores,
    on_event: Channel<AgentRunMessage>,
    params: AgentRunParams,
    provenance: RunProvenance,
    resolved: ResolvedEngine,
) -> Result<AgentRunRecord, String> {
    // cwd 恒为当前 workspace root（root 已由命令面 blank 检查，for_root 可解析）
    let store = stores
        .for_root(&params.cwd.to_string_lossy())
        .map_err(|e| e.to_string())?;
    let resume: ResumeTranscript = {
        let store = Arc::clone(&store);
        Arc::new(move |session_id: &str| find_events_by_session(&store, session_id))
    };
    let runner =
        EngineFacade::with_resume_transcript(resume).runner_for(resolved.kind, resolved.config);
    start_agent_run_with(app, stores, runner.as_ref(), on_event, params, provenance)
}

pub(crate) fn start_agent_run_with<T>(
    app: AppHandle<T>,
    stores: &WorkspaceStores,
    runner: &dyn AgentRunner,
    on_event: Channel<AgentRunMessage>,
    params: AgentRunParams,
    provenance: RunProvenance,
) -> Result<AgentRunRecord, String>
where
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
