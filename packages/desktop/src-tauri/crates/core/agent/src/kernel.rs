use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::event::{now_millis, AgentEvent, AgentEventKind};
use crate::port::{SessionSink, TurnOutcome};
use crate::runner::{
    AgentRunStatus, AgentRunner, AgentSession, AgentStartError, RunHandle, SessionCtx,
    SessionInjections, SessionOpen, SessionRef, TurnQuestion,
};
use crate::session::{new_session_id, NewSessionRow, SessionProvenance};
use crate::state::{AgentRunState, RunStateMachine};

/// 停止注册表（内存态，进程生命周期，应用重启即清空）：运行中会话的停止
/// 句柄登记，键 = core session id（core 铸 id 进程内全局唯一，免复合键）。
/// 会话为逻辑寻址单位——CLI 每轮进程、SDK 每轮泵任务，actor 统一为逻辑层。
#[derive(Default)]
pub struct StopRegistry {
    handles: Mutex<HashMap<String, RunHandle>>,
}

impl StopRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记运行中会话的停止句柄（begin_turn 落库后立即注册）。
    pub fn register(&self, session_id: &str, handle: RunHandle) {
        self.handles
            .lock()
            .expect("停止注册表锁不可中毒")
            .insert(session_id.to_owned(), handle);
    }

    /// 置位终止信号；miss 幂等返回 false（不报错、不改既有终态）。
    pub fn request_stop(&self, session_id: &str) -> bool {
        let handles = self.handles.lock().expect("停止注册表锁不可中毒");
        match handles.get(session_id) {
            Some(handle) => {
                handle.request_stop();
                true
            }
            None => false,
        }
    }

    /// 收敛除名（除名后 stop 对该键幂等忽略）。
    pub fn remove(&self, session_id: &str) {
        self.handles
            .lock()
            .expect("停止注册表锁不可中毒")
            .remove(session_id);
    }
}

/// 内核轮请求：命令 / 组合根侧的一轮发起入参（快照由组合根组装、内核不
/// 解释）。`prior_handle` 为引擎侧先行句柄（Continue 时组合根自会话记录
/// 提取，经 [`SessionOpen`] 回供引擎）。
#[derive(Debug, Clone, PartialEq)]
pub struct TurnRequest {
    /// 会话引用（New 建会话行；Continue 沿用既有行）
    pub session: SessionRef,
    /// 轮提问
    pub question: String,
    /// 会话级注入
    pub injections: SessionInjections,
    /// 轮级上下文
    pub ctx: SessionCtx,
    /// 来源归属（缺省 debug）
    pub provenance: SessionProvenance,
    /// 装配配置快照（JSON 形态，内核不解释）
    pub config_snapshot: serde_json::Value,
    /// 引擎侧先行句柄（New 恒 None）
    pub prior_handle: Option<String>,
}

/// 内核泵输出：盖戳后的全量观察（传输面）与轮终态（与实时同构）。
#[derive(Debug, Clone)]
pub enum KernelOutput {
    /// 盖戳后观察事件（增量与密封全量流出；落库归 sink 半边）
    Observation(AgentEvent),
    /// 轮终态（与实时事件同构流出）
    TurnFinished(TurnOutcome),
}

/// 轮统计摘取（TurnDone 字段面唯一口径的内核侧收口载体）。
#[derive(Debug, Clone, Default)]
struct TurnStats {
    num_turns: Option<u64>,
    duration_ms: Option<u64>,
    cost_usd: Option<f64>,
    usage: serde_json::Value,
    remote_session_id: Option<String>,
}

/// 提前 resolve 产物：会话建立与轮注册的同步段完成后的运行中轮句柄。三件
/// 公开字段即命令面装配 running 态轮行所需的全部信息；驱动经 [`Self::drive`]。
pub struct RunningTurn {
    /// 所属会话 id
    pub session_id: String,
    /// 轮 id（sink 分配）
    pub turn_id: i64,
    /// 开始时间（UTC unix 毫秒）
    pub started_at: i64,
    /// 引擎会话三件套（观察回流 + 轮驱动 + 句柄）
    agent_session: AgentSession,
    /// 持久面（write-through）
    sink: Arc<dyn SessionSink>,
    /// 停止注册表（终态除名）
    registry: Arc<StopRegistry>,
    /// 轮提问
    question: String,
}

/// 会话内核：治理面 + 运行面驱动（组合根装配；内核自身零引擎 / 零持久化
/// 技术知识）。
pub struct SessionKernel {
    sink: Arc<dyn SessionSink>,
    registry: Arc<StopRegistry>,
}

impl SessionKernel {
    /// 内核构造：持久面 port + 停止注册（组合根装配）。
    pub fn new(sink: Arc<dyn SessionSink>, registry: Arc<StopRegistry>) -> Self {
        Self { sink, registry }
    }

    /// 会话建立与轮注册的同步段（提前 resolve 契约）：open（失败不落库）→
    /// New 建会话行 → 开轮行 → 停止登记 → 返回 [`RunningTurn`]。
    /// 持久面失败同样属于启动失败（不产生半成品记录）。
    pub fn begin_turn(
        &self,
        runner: Arc<dyn AgentRunner>,
        request: TurnRequest,
    ) -> Result<RunningTurn, AgentStartError> {
        let session_id = match &request.session {
            SessionRef::New => new_session_id(),
            SessionRef::Continue { id } => id.clone(),
        };
        let is_new = matches!(request.session, SessionRef::New);
        // open：启动阶段失败不产生任何记录
        let agent_session = runner.open_session(SessionOpen {
            injections: request.injections,
            ctx: request.ctx,
            session: request.session,
            prior_handle: request.prior_handle,
        })?;
        let started_at = now_millis();
        // New：建会话行（Continue 行已在库，不重写）
        if is_new {
            self.sink
                .create_session(&NewSessionRow {
                    id: session_id.clone(),
                    config_snapshot: request.config_snapshot,
                    provenance: request.provenance,
                })
                .map_err(|error| {
                    AgentStartError::SpawnFailed(format!("会话记录落库失败: {error}"))
                })?;
        }
        // 开轮行：写事务内分配轮 id，running 初值
        let turn_id = self
            .sink
            .begin_turn(&session_id, started_at)
            .map_err(|error| AgentStartError::SpawnFailed(format!("轮记录落库失败: {error}")))?;
        // 停止登记（键 = core session id）
        self.registry
            .register(&session_id, agent_session.handle.clone());
        Ok(RunningTurn {
            session_id,
            turn_id,
            started_at,
            agent_session,
            sink: Arc::clone(&self.sink),
            registry: Arc::clone(&self.registry),
            question: request.question,
        })
    }
}

impl RunningTurn {
    pub async fn drive<F>(self, mut on_output: F) -> TurnOutcome
    where
        F: FnMut(KernelOutput),
    {
        let RunningTurn {
            session_id,
            turn_id,
            agent_session: mut session,
            sink,
            registry,
            question,
            ..
        } = self;
        // ask：轮提问送达（接收端已关即引擎侧异常终止，failed 收敛记因）
        if session
            .questions
            .send(TurnQuestion { prompt: question })
            .await
            .is_err()
        {
            return Self::converge(
                &session_id,
                turn_id,
                AgentRunStatus::Failed,
                Some("轮提问送达失败（引擎侧已终止）".to_owned()),
                TurnStats::default(),
                &*sink,
                &registry,
                &mut on_output,
            )
            .await;
        }

        let mut machine = RunStateMachine::new();
        let mut seq: u64 = 0;
        let mut stats = TurnStats::default();
        while let Some(kind) = session.observations.recv().await {
            let event = AgentEvent::stamp(seq, kind);
            seq += 1;
            if event.kind.is_delta() {
                // 增量只上传输面（store 永不见；盖戳治理单点的传输半边）
                on_output(KernelOutput::Observation(event));
                continue;
            }
            // 双 id 映射落库半边：引擎侧标识上报即绑定 + updated_at 刷新
            //（尽力语义，元数据刷新失败不中断轮）
            if let Some(remote) = report_session_id(&event.kind) {
                let _ = sink.bind_remote_session(&session_id, &remote, now_millis());
            }
            machine.apply(&event);
            if let AgentEventKind::TurnDone {
                num_turns,
                duration_ms,
                cost_usd,
                usage,
                session_id: remote,
                ..
            } = &event.kind
            {
                stats = TurnStats {
                    num_turns: *num_turns,
                    duration_ms: *duration_ms,
                    cost_usd: *cost_usd,
                    usage: usage.clone(),
                    remote_session_id: remote.clone(),
                };
            }
            // 密封 write-through：落库失败不可静默 → failed 收敛记因、尽力
            // 流出终态并终止 tee
            if let Err(store_error) = sink.append_sealed(&session_id, &event) {
                return Self::converge(
                    &session_id,
                    turn_id,
                    AgentRunStatus::Failed,
                    Some(format!("事件落库失败: {store_error}")),
                    stats,
                    &*sink,
                    &registry,
                    &mut on_output,
                )
                .await;
            }
            on_output(KernelOutput::Observation(event));
        }

        // 观察流结束：状态机已收敛以状态机为准（首个收敛生效）；否则停止
        // 信号已置位 → 显式收敛 stopped（error 不记因——用户主动终止非失败）；
        // 兜底 failed 记因
        let (status, error) = match machine.current() {
            AgentRunState::Completed => (AgentRunStatus::Completed, None),
            AgentRunState::Failed => (AgentRunStatus::Failed, None),
            AgentRunState::Running if session.handle.stop_requested() => {
                machine.stop();
                (AgentRunStatus::Stopped, None)
            }
            AgentRunState::Running | AgentRunState::Stopped => (
                AgentRunStatus::Failed,
                Some("轮结束但未产出收敛事件".to_owned()),
            ),
        };
        Self::converge(
            &session_id,
            turn_id,
            status,
            error,
            stats,
            &*sink,
            &registry,
            &mut on_output,
        )
        .await
    }

    /// 轮收口：组装终态 → 轮行落库（失败记因仍尽力流出）→ 注册表除名 →
    /// `TurnFinished` 流出。收口路径唯一（ask 失败 / 落库失败 / 正常 EOF
    /// 三臂共用），终态幂等。
    #[allow(clippy::too_many_arguments)]
    async fn converge<F>(
        session_id: &str,
        turn_id: i64,
        status: AgentRunStatus,
        error: Option<String>,
        stats: TurnStats,
        sink: &dyn SessionSink,
        registry: &StopRegistry,
        on_output: &mut F,
    ) -> TurnOutcome
    where
        F: FnMut(KernelOutput),
    {
        let mut outcome = TurnOutcome {
            turn_id,
            status,
            finished_at: now_millis(),
            num_turns: stats.num_turns,
            duration_ms: stats.duration_ms,
            cost_usd: stats.cost_usd,
            usage: stats.usage,
            error,
            remote_session_id: stats.remote_session_id,
        };
        if let Err(store_error) = sink.finish_turn(turn_id, &outcome) {
            // 终态落库失败同样不可静默：error 记因后仍尽力流出终态
            outcome.error = Some(format!("终态落库失败: {store_error}"));
        }
        registry.remove(session_id);
        on_output(KernelOutput::TurnFinished(outcome.clone()));
        outcome
    }
}

/// 事件携带的引擎侧会话标识（RunStarted / TurnDone 的上报位）。
fn report_session_id(kind: &AgentEventKind) -> Option<String> {
    match kind {
        AgentEventKind::RunStarted { session_id, .. }
        | AgentEventKind::TurnDone { session_id, .. } => session_id.clone(),
        _ => None,
    }
}
