use std::sync::atomic::{AtomicU64, Ordering};

use agent::{AgentRunner, AgentSession, AgentStartError, RunHandle, SessionOpen, TurnQuestion};
use rig::client::CompletionClient;
use rig::completion::CompletionModel;
use rig::message::Message;
use rig::providers::openai;
use tokio::sync::mpsc;

use crate::sdk::config::EngineConfig;
use crate::sdk::context::{ContextDefense, DefenseNotice};
use crate::sdk::{context, r#loop, resume};
use crate::ResumeTranscript;

/// 事件通道有界容量（与 CLI 泵同策略：背压阻塞 send，事件不丢、内存有界）。
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// 提问通道有界容量（单问送达；单会话单活动轮，容量 4 为余量）。
const QUESTION_CHANNEL_CAPACITY: usize = 4;

/// sdk 会话 id 进程内原子计数（配合毫秒时戳保证同毫秒不重号）。
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// sdk 引擎 runner：持有连接配置、窗长载荷与会话全史转录装载缝；无跨会话
/// 共享运行态（全部运行态在 `open_session` 产出的泵任务内）。
pub struct SdkRunner {
    config: EngineConfig,
    resume: Option<ResumeTranscript>,
    /// provider 上下文窗长（组合根旁路载荷；None = 128K 缺省启发式）
    context_window: Option<u64>,
}

impl SdkRunner {
    /// runner 构造（配置、窗长与装载缝由门面注入，引擎细节不出门面）。
    pub fn new(
        config: EngineConfig,
        resume: Option<ResumeTranscript>,
        context_window: Option<u64>,
    ) -> Self {
        Self {
            config,
            resume,
            context_window,
        }
    }

    /// `sdk-<进程内计数>-<毫秒时戳>` 引擎侧会话 id（每轮铸造；前缀仅保留为
    /// 铸造格式，归属校验凭配置快照收组合根）。
    fn next_session_id() -> String {
        let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        format!("{}{counter}-{millis}", resume::SESSION_PREFIX)
    }

    /// 续会话解析：全新运行空史；显式续会话经装载缝取**会话全史转录**并重建
    /// rig 对话史——loader `Err`（库读取失败）/ `None`（会话不存在）/ 重建
    /// 空史，全部以 [`AgentStartError::ConfigMissing`] 显式失败（不产生记录，
    /// `Err` 抵达前端）。
    fn resolve_resume(&self, session_id: &Option<String>) -> Result<Vec<Message>, AgentStartError> {
        let Some(session_id) = session_id else {
            return Ok(Vec::new());
        };
        let Some(loader) = &self.resume else {
            return Err(AgentStartError::ConfigMissing(format!(
                "转录装载器未注入，无法续会话: {session_id}"
            )));
        };
        let events = loader(session_id)
            .map_err(|error| AgentStartError::ConfigMissing(format!("转录读取失败: {error}")))?
            .ok_or_else(|| AgentStartError::ConfigMissing(format!("会话不存在: {session_id}")))?;
        resume::rebuild(&events).map_err(AgentStartError::ConfigMissing)
    }
}

impl AgentRunner for SdkRunner {
    fn open_session(&self, open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        // 配置三件套校验（消息区分成因；单一中性变体承载全部启动失败）
        let missing: Vec<&str> = [
            ("api_key", self.config.api_key.is_empty()),
            ("base_url", self.config.base_url.is_empty()),
            ("model", self.config.model.is_empty()),
        ]
        .into_iter()
        .filter(|(_, is_missing)| *is_missing)
        .map(|(field, _)| field)
        .collect();
        if !missing.is_empty() {
            return Err(AgentStartError::ConfigMissing(format!(
                "配置项未填: {}（引擎配置硬编码位未手填）",
                missing.join(" / ")
            )));
        }
        // Continue：经装载缝取会话全史转录重建（New 空史）；先行句柄 sdk 引擎
        // 不消费（引擎侧标识每轮铸造，重建源即会话全史转录）
        let history = self.resolve_resume(&continue_session_id(&open))?;
        let model = build_model(&self.config)?;
        let model_name = self.config.model.clone();
        let (question_tx, question_rx) = mpsc::channel::<TurnQuestion>(QUESTION_CHANNEL_CAPACITY);
        let (observation_tx, observation_rx) =
            mpsc::channel::<agent::AgentEventKind>(EVENT_CHANNEL_CAPACITY);
        let handle = RunHandle::default();
        // 上下文防线装配（组合根旁路载荷 → 缺省解析）+ 第一调用点：resume
        // 重建史超 L2 水位即先行剪裁；notice 交泵先于 RunStarted 转发（同
        // 任务同通道 FIFO 保序，重建史防线留痕）
        let defense = ContextDefense::resolve(self.context_window);
        let (history, resume_notices) = context::prune(history, &defense);
        tokio::spawn(session_pump(
            model,
            model_name,
            open,
            PumpPayload {
                history,
                defense,
                resume_notices,
            },
            question_rx,
            observation_tx,
            handle.clone(),
        ));
        Ok(AgentSession {
            observations: observation_rx,
            questions: question_tx,
            handle,
        })
    }
}

/// Continue 引用的会话 id（重建史寻址；New 为 None）。
fn continue_session_id(open: &SessionOpen) -> Option<String> {
    match &open.session {
        agent::SessionRef::Continue { id } => Some(id.clone()),
        agent::SessionRef::New => None,
    }
}

/// rig client 组装：openai 兼容端点（自定义 base_url）+ chat completions
/// API + 模型标识。端点不可达性在请求期才暴露（构造失败仅 URL/凭据形态
/// 非法），落 `ConfigMissing`（启动失败单一中性变体）。
fn build_model(config: &EngineConfig) -> Result<impl CompletionModel, AgentStartError> {
    let client = openai::Client::builder()
        .api_key(config.api_key.clone())
        .base_url(&config.base_url)
        .build()
        .map_err(|error| AgentStartError::ConfigMissing(format!("端点配置非法: {error}")))?
        .completions_api();
    Ok(client.completion_model(config.model.clone()))
}

/// 泵任务的防线与史载荷（参数面收敛分组：重建史 + 防线配置 + 重建剪裁的
/// notice 留痕）。
struct PumpPayload {
    /// resume 重建史（已经第一调用点 L2 先行剪裁）
    history: Vec<Message>,
    /// 上下文窗防线配置
    defense: ContextDefense,
    /// 重建剪裁产出的 notice（泵内先于 RunStarted 转发）
    resume_notices: Vec<DefenseNotice>,
}

/// 会话泵任务（ask 段）
async fn session_pump<M>(
    model: M,
    model_name: String,
    open: SessionOpen,
    payload: PumpPayload,
    mut questions: mpsc::Receiver<TurnQuestion>,
    observations: mpsc::Sender<agent::AgentEventKind>,
    handle: RunHandle,
) where
    M: CompletionModel,
{
    let Some(question) = questions.recv().await else {
        return; // 组合根半边先关：无问即无泵生命周期
    };
    // resume 重建防线 notice 先行转发（泵的 RunStarted 之先，重建史留痕）
    for DefenseNotice { subtype, payload } in &payload.resume_notices {
        let _ = observations
            .send(agent::AgentEventKind::SystemNotice {
                subtype: subtype.clone(),
                payload: payload.clone(),
            })
            .await;
    }
    let turn = r#loop::LoopTurn {
        question: question.prompt,
        cwd: open.ctx.workspace_root.clone(),
        permission_mode: open.ctx.permission_mode,
        model_name: model_name.clone(),
        session_id: SdkRunner::next_session_id(),
        defense: payload.defense,
        liveness: Default::default(),
    };
    crate::sdk::log::append_engine_log(&format!(
        "泵启动 session={} model={model_name}",
        turn.session_id
    ));
    let pump_handle = handle.clone();
    let pump_session = turn.session_id.clone();
    tokio::select! {
        biased;
        _ = r#loop::run(&model, &turn, payload.history, observations.clone(), pump_handle.clone()) => {
            // 一轮一命：单轮收敛即泵生命周期终点；累积史（跨轮回灌）无
            // 消费方，弃用
            crate::sdk::log::append_engine_log(&format!(
                "泵结束 session={pump_session}"
            ));
        }
        _ = pump_handle.wait_requested() => {
            // 停止后的会话不再接受续轮（编排侧已显式收敛）
            crate::sdk::log::append_engine_log(&format!(
                "泵停止截停（select 停止臂获胜，loop future 轮中 drop）session={pump_session}"
            ));
        }
    }
}
