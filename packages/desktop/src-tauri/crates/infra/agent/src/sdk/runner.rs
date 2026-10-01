use std::sync::atomic::{AtomicU64, Ordering};

use agent::{AgentRunner, AgentSession, AgentStartError, RunHandle, SessionOpen, TurnQuestion};
use rig_core::client::CompletionClient;
use rig_core::completion::CompletionModel;
use rig_core::message::Message;
use rig_core::providers::openai;
use tokio::sync::mpsc;

use crate::sdk::config::EngineConfig;
use crate::sdk::{r#loop, resume};
use crate::ResumeTranscript;

/// 事件通道有界容量（与 CLI 泵同策略：背压阻塞 send，事件不丢、内存有界）。
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// 提问通道有界容量（逐轮送达；单会话单活动轮，容量 4 为余量）。
const QUESTION_CHANNEL_CAPACITY: usize = 4;

/// sdk 会话 id 进程内原子计数（配合毫秒时戳保证同毫秒不重号）。
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// sdk 引擎 runner：持有连接配置与会话全史转录装载缝；无跨会话共享运行态
/// （全部运行态在 `open_session` 产出的泵任务内）。
pub struct SdkRunner {
    config: EngineConfig,
    resume: Option<ResumeTranscript>,
}

impl SdkRunner {
    /// runner 构造（配置与装载缝由门面注入，引擎细节不出门面）。
    pub fn new(config: EngineConfig, resume: Option<ResumeTranscript>) -> Self {
        Self { config, resume }
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
        tokio::spawn(session_pump(
            model,
            model_name,
            open,
            history,
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

/// 会话泵任务（ask 段）：逐轮等待提问 → 铸造本轮引擎侧会话标识 → select
/// 包裹 loop future 与停止信号（停止 drop future，不合成收敛、本轮增量史
/// 随之丢弃）→ 正常收敛回灌累积史、回到等待下一轮；问题通道关闭（内核轮
/// 驱动半边已收）即退出。
async fn session_pump<M>(
    model: M,
    model_name: String,
    open: SessionOpen,
    history: Vec<Message>,
    mut questions: mpsc::Receiver<TurnQuestion>,
    observations: mpsc::Sender<agent::AgentEventKind>,
    handle: RunHandle,
) where
    M: CompletionModel,
{
    // 史槽（Option 承载跨轮全史）：select 的停止臂 drop loop future 时本轮
    // 增量史一并丢弃——停止后的会话不再续跑（编排侧已显式收敛）
    let mut history_slot = Some(history);
    while let Some(question) = questions.recv().await {
        let Some(history) = history_slot.take() else {
            return;
        };
        let turn = r#loop::LoopTurn {
            question: question.prompt,
            cwd: open.ctx.workspace_root.clone(),
            permission_mode: open.ctx.permission_mode,
            model_name: model_name.clone(),
            session_id: SdkRunner::next_session_id(),
        };
        let pump_handle = handle.clone();
        // biased：loop 臂先 poll。停止先置位时两臂首轮 poll 同为 Ready——
        // wait_requested 已置位立即返回，loop future 也在首轮 poll 内发出
        // RunStarted + user 两枚先导事件后经轮间快速路径返回（有界通道发送
        // 不让渡）；随机臂序会让停止臂在 loop future 首次 poll 前获胜，先导
        // 事件丢失、发送端随泵 return 全部 dropped、观测通道提前关闭。固定
        // 臂序后先导事件恒达，停止臂仅在轮中途截停（drop future，不合成收敛）。
        tokio::select! {
            biased;
            updated = r#loop::run(&model, &turn, history, observations.clone(), pump_handle.clone()) => {
                history_slot = Some(updated);
            }
            _ = pump_handle.wait_requested() => {
                return; // 停止后的会话不再接受续轮（编排侧已显式收敛）
            }
        }
    }
}
