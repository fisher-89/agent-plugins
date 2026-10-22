use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use agent::{
    AgentRunner, AgentSession, AgentStartError, ModelLevel, RunHandle, SessionOpen, SessionToolset,
    TurnQuestion,
};
use rig::driver::DynModel;
use rig::message::Message;
use rig::operation::Completion;
use rig::providers::anthropic::AnthropicConfig;
use rig::providers::openai::OpenAIConfig;
use store::Store;
use tokio::sync::mpsc;

use crate::sdk::config::{EngineConfig, Protocol};
use crate::sdk::context::{ContextDefense, DefenseNotice};
use crate::sdk::explore_note::ExploreNoteContext;
use crate::sdk::model_spec::{
    parse_model_spec, ModelSpec, BETA_CONTEXT_1M, MILLION_CONTEXT_WINDOW,
};
use crate::sdk::{context, r#loop, resume};
use crate::ResumeTranscript;

/// 事件通道有界容量（背压阻塞 send，事件不丢、内存有界）。
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// 提问通道有界容量（单问送达；单会话单活动轮，容量 4 为余量）。
const QUESTION_CHANNEL_CAPACITY: usize = 4;

/// anthropic 输出上限兜底（token 数）：rig 内置模型名单未命中且 provider 未填
/// `max_tokens` 时的取值（对齐 Claude Code 表外行为 `32_000`）——名单外模型
/// 因此不再触发编码期 `max_tokens must be set for Anthropic` 失败；
/// 老代 / 第三方网关的小上限模型由用户显式填 `max_tokens` 覆盖。
const OUTPUT_TOKENS_FALLBACK: u64 = 32_000;

/// provider `max_tokens` 保存期下限（token 数）：命令层保存校验与引擎侧
/// 取值链共用同一门槛（MUST NOT 造第二套字面量）。
pub const MIN_OUTPUT_TOKENS: u64 = 4_096;

/// sdk 会话 id 进程内原子计数（配合毫秒时戳保证同毫秒不重号）。
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// sdk 引擎 runner：持有连接配置、窗长载荷、会话全史转录装载缝与 explore
/// 专用工具 store 句柄；无跨会话共享运行态（全部运行态在 `open_session` 产出
/// 的泵任务内）。
pub struct SdkRunner {
    config: EngineConfig,
    resume: Option<ResumeTranscript>,
    /// provider 上下文窗长（组合根旁路载荷；None = 128K 缺省启发式，该档模型
    /// 名带 `[1m]` 后缀时被覆盖）
    context_window: Option<u64>,
    /// provider 输出上限（组合根旁路载荷；None = 按协议缺省）
    max_tokens: Option<u64>,
    /// workspace 库句柄（组合根注入缝）：`toolset == Explore` 会话 per-turn
    /// 铸造 `create_explore_note` 执行上下文的原料（None = 未注入）
    explore_store: Option<Arc<Store>>,
}

impl SdkRunner {
    /// runner 构造（配置、窗长、装载缝与 store 句柄由门面注入，引擎细节不出
    /// 门面）。
    pub fn new(
        config: EngineConfig,
        resume: Option<ResumeTranscript>,
        context_window: Option<u64>,
        explore_store: Option<Arc<Store>>,
    ) -> Self {
        Self {
            config,
            resume,
            context_window,
            max_tokens: None,
            explore_store,
        }
    }

    /// 输出上限旁路注入（builder 缝，与 `context_window` 并列的解码后载荷）：
    /// `new` 签名不动（既有调用点零改），`None` = 按协议缺省。
    pub fn with_max_tokens(self, max_tokens: Option<u64>) -> Self {
        Self { max_tokens, ..self }
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
    /// rig 对话史。空转录（会话存在但从未开轮——explore 建档预建空会话的
    /// 首轮）→ 空史照常开跑；loader `Err`（库读取失败）/ `None`（会话不存在）
    /// / 有事件却重建不出有效史（损坏转录），全部以
    /// [`AgentStartError::ConfigMissing`] 显式失败（不产生记录，`Err` 抵达前端）。
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
        if events.is_empty() {
            return Ok(Vec::new()); // 未开轮会话：首轮即空史开跑
        }
        resume::rebuild(&events).map_err(AgentStartError::ConfigMissing)
    }
}

impl AgentRunner for SdkRunner {
    fn open_session(&self, open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        let (model_field, model_name) = match open.ctx.model_level {
            ModelLevel::High => ("model_high", self.config.model_high.clone()),
            ModelLevel::Low => ("model_low", self.config.model_low.clone()),
        };
        // 连接配置字段面齐备校验（消息区分成因；单一中性变体承载全部启动
        // 失败；协议恒有值不参与校验）
        let missing: Vec<&str> = [
            ("api_key", self.config.api_key.is_empty()),
            ("base_url", self.config.base_url.is_empty()),
            (model_field, model_name.is_empty()),
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
        // 模型名后缀解析（按档现算）：`[1m]` 剥离出裸名 + 窗长候选；非法后缀
        // 在启动期显式失败（不静默降级为裸文发送）。保存期校验复用同一函数
        // （命令层 `save_agent_provider`），无第二套规则。
        let spec = parse_model_spec(&model_name)
            .map_err(|error| AgentStartError::ConfigMissing(format!("{model_field} 档{error}")))?;
        // Continue：经装载缝取会话全史转录重建（New 空史）；先行句柄 sdk 引擎
        // 不消费（引擎侧标识每轮铸造，重建源即会话全史转录）
        let history = self.resolve_resume(&continue_session_id(&open))?;
        let model = build_model(&self.config, &spec, self.max_tokens);
        let (question_tx, question_rx) = mpsc::channel::<TurnQuestion>(QUESTION_CHANNEL_CAPACITY);
        let (observation_tx, observation_rx) =
            mpsc::channel::<agent::AgentEventKind>(EVENT_CHANNEL_CAPACITY);
        let handle = RunHandle::default();
        // 上下文防线装配（按档现算窗长：模型名后缀 > provider `context_length`
        // > 128K 缺省）+ 第一调用点：resume 重建史超 L2 水位即先行剪裁；
        // notice 交泵先于 RunStarted 转发（同任务同通道 FIFO 保序，重建史防线
        // 留痕）
        let defense = ContextDefense::resolve(spec.length.or(self.context_window));
        let (history, resume_notices) = context::prune(history, &defense);
        tokio::spawn(session_pump(
            model,
            spec,
            open,
            PumpPayload {
                history,
                defense,
                resume_notices,
                protocol: self.config.protocol,
                raw_model: model_name,
                explore_store: self.explore_store.clone(),
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

/// rig 模型组装（协议唯一分叉处）：两臂同产 `DynModel<Completion>`，故
/// `loop` / `normalize` / `resume` / core 全链协议中立零改动。
///
/// - openai 臂：与变更前逐字等价（不追加任何请求参数——`max_tokens` 保持
///   省略，各网关对超限行为不一）；
/// - anthropic 臂：条件追加 `anthropic-beta`（`[1m]` 后缀在场）+ 定型 wire
///   默认输出上限（取值链见 [`resolve_output_tokens`]）。
pub(crate) fn build_model(
    config: &EngineConfig,
    spec: &ModelSpec,
    max_tokens: Option<u64>,
) -> DynModel<Completion> {
    match config.protocol {
        Protocol::Openai => openai_provider(config)
            .client()
            .chat(spec.name.clone())
            .erase(),
        Protocol::Anthropic => {
            let mut model = anthropic_provider(config, spec)
                .client()
                .completion(spec.name.clone());
            // 定型 wire 默认上限：wire `encode` 在请求 `max_tokens` 缺席时取该
            // 值，故 `loop` / `compact` 的 `max_tokens: None` 保持零改动
            model.wire.default_max_tokens = Some(resolve_output_tokens(
                max_tokens,
                model.wire.default_max_tokens,
            ));
            model.erase()
        }
    }
}

/// openai 端点配置（协议臂内可测接缝）：`base_url` 原样承接（openai 契约要求
/// 含版本段，引擎不改写）。`build_model` 单点消费，测试相位直断描述面。
pub(crate) fn openai_provider(config: &EngineConfig) -> OpenAIConfig {
    OpenAIConfig::new(config.api_key.clone()).with_base_url(config.base_url.clone())
}

/// anthropic 端点配置（协议臂内可测接缝）：`base_url` 归一化 + 该档模型名带
/// `[1m]` 后缀时的 1M beta 头。`build_model` 单点消费，测试相位直断 betas /
/// base_url 归一化面。
///
/// 1M 上下文 beta：仅 anthropic 协议且该档模型名带 `[1m]` 后缀时发（无模型
/// 元数据表 → 不做白名单校验，网关拒绝以 api_error 显式暴露）。
pub(crate) fn anthropic_provider(config: &EngineConfig, spec: &ModelSpec) -> AnthropicConfig {
    let provider =
        AnthropicConfig::new(config.api_key.clone()).with_base_url(config.base_url.as_str());
    if spec.length == Some(MILLION_CONTEXT_WINDOW) {
        provider.with_beta(BETA_CONTEXT_1M)
    } else {
        provider
    }
}

/// anthropic 输出上限取值链：provider 显式值优先 → rig 名单内官方值（保留、
/// 不降级）→ [`OUTPUT_TOKENS_FALLBACK`] 兜底（名单外模型）。
pub(crate) fn resolve_output_tokens(provider_value: Option<u64>, rig_default: Option<u64>) -> u64 {
    provider_value
        .or(rig_default)
        .unwrap_or(OUTPUT_TOKENS_FALLBACK)
}

/// 泵任务的防线与史载荷（参数面收敛分组：重建史 + 防线配置 + 重建剪裁的
/// notice 留痕 + explore 专用工具句柄）。
struct PumpPayload {
    /// resume 重建史（已经第一调用点 L2 先行剪裁）
    history: Vec<Message>,
    /// 上下文窗防线配置
    defense: ContextDefense,
    /// 重建剪裁产出的 notice（泵内先于 RunStarted 转发）
    resume_notices: Vec<DefenseNotice>,
    /// 模型协议（泵启动日志诊断面 `protocol=…`）
    protocol: Protocol,
    /// 该档模型原文（日志 `raw=…`；上报与请求用裸名——见 `ModelSpec.name`）
    raw_model: String,
    /// workspace 库句柄（组合根注入缝）——`toolset == Explore` 会话 per-turn
    /// 铸造 `create_explore_note` 执行上下文的原料（None = 未注入）
    explore_store: Option<Arc<Store>>,
}

/// 会话泵任务（ask 段）
async fn session_pump(
    model: DynModel<Completion>,
    spec: ModelSpec,
    open: SessionOpen,
    payload: PumpPayload,
    mut questions: mpsc::Receiver<TurnQuestion>,
    observations: mpsc::Sender<agent::AgentEventKind>,
    handle: RunHandle,
) {
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
    // `create_explore_note` 专用工具执行上下文 per-turn 铸造：toolset == Explore
    // 且 store 句柄在场才铸（Continue id 缺席则 None——生产不可达，explore 会话
    // 恒 Continue）；上下文随 LoopTurn 携带、loop 侧路由，不经 `tools::execute`
    let explore_note = if open.injections.toolset == Some(SessionToolset::Explore) {
        payload
            .explore_store
            .zip(continue_session_id(&open))
            .map(|(store, id)| ExploreNoteContext::new(store, open.ctx.workspace_root.clone(), id))
    } else {
        None
    };
    // 泵启动日志（诊断面：同一 base_url 配错协议可一眼定位）。`model` 报裸名
    // （= 实际发送）、`window` 报**生效**窗长（含 128K 缺省）、`raw` 留配置原文
    let session_id = SdkRunner::next_session_id();
    let window = payload.defense.window();
    crate::sdk::log::append_engine_log(&format!(
        "泵启动 session={session_id} protocol={} model={} window={window} raw={}",
        payload.protocol.as_str(),
        spec.name,
        payload.raw_model
    ));
    let turn = r#loop::LoopTurn {
        question: question.prompt,
        cwd: open.ctx.workspace_root.clone(),
        permission_mode: open.ctx.permission_mode,
        // 裸名上报（`RunStarted.model` 反映实际发送；配置原文留
        // `SessionConfigSnapshot.model`）
        model_name: spec.name.clone(),
        session_id,
        defense: payload.defense,
        liveness: Default::default(),
        retry: Default::default(),
        injections: open.injections.clone(),
        explore_note,
    };
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
