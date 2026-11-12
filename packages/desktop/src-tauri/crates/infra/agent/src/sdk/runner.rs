use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use agent::{
    AgentRunner, AgentSession, AgentStartError, ModelLevel, RunHandle, SessionOpen, TurnQuestion,
};
use rig::driver::DynModel;
use rig::message::Message;
use rig::operation::Completion;
use rig::providers::anthropic::AnthropicConfig;
use rig::providers::openai::OpenAIConfig;
use store::Store;
use tokio::sync::mpsc;

use crate::sdk::archi_tool::ArchiContext;
use crate::sdk::config::{EngineConfig, Protocol};
use crate::sdk::context::{ContextDefense, DefenseNotice};
use crate::sdk::explore_note::ExploreNoteContext;
use crate::sdk::model_spec::{
    parse_model_spec, ModelSpec, BETA_CONTEXT_1M, MILLION_CONTEXT_WINDOW,
};
use crate::sdk::mount::{ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL, EXPLORE_NOTE_TOOL};
use crate::sdk::protect::ProtectRules;
use crate::sdk::{context, r#loop, resume};
use crate::ResumeTranscript;

/// 事件通道有界容量（背压阻塞 send，事件不丢、内存有界）。
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// 提问通道有界容量（单问送达；单会话单活动轮，容量 4 为余量）。
const QUESTION_CHANNEL_CAPACITY: usize = 4;

/// 输出上限分段表（标称窗长下界, 默认输出上限），升序；[`derive_output_tokens`]
/// 取「下界 <= 窗长」的最后一档。编译期常量而非配置面——与 `MAX_ROUNDS` /
/// `FEEDBACK_LOOP_LIMIT` 同类；用户级覆盖走 provider `max_tokens` 列，不是第二
/// 条配置面。
///
/// 128K 档逐字保留既往 `32_000` 兜底：`DEFAULT_CONTEXT_WINDOW = 128 * 1024`，
/// 未配置任何字段的用户取值因此不变。封顶 64_000 而非 128_000：rig 名单内模型
/// 先命中其官方值（权威元数据），本表只服务名单外模型，对未知模型 64K 是安全
/// 猜测。下限取 `MIN_OUTPUT_TOKENS`：推导值必须落在表单与命令层的合法区间内。
const OUTPUT_TOKENS_SEGMENTS: &[(u64, u64)] = &[
    (0, MIN_OUTPUT_TOKENS),
    (32 * 1024, 16_384),
    (128 * 1024, 32_000),
    (256 * 1024, 48_000),
    (512 * 1024, 64_000),
];

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
    /// provider 输出上限（组合根旁路载荷；None = 按标称窗长分档推导）
    max_tokens: Option<u64>,
    /// 云端 `web_search` 服务端工具开关（组合根已按协议门控的最终值；
    /// `new` 缺省 false——与「显式关」同义，故全链收 `bool` 而非 `Option`）
    web_search: bool,
    /// workspace 库句柄（组合根注入缝）：名字清单含 `create_explore_note` 的
    /// 会话 per-turn 铸造该工具执行上下文的原料（None = 未注入）
    explore_store: Option<Arc<Store>>,
    /// 主仓 root（组合根注入缝）：写入保护规则装配的根（工作区配置权威所在
    /// 地）；`None` = 未注入 → 只装载内置规则（非「保护关闭」）
    workspace_root: Option<PathBuf>,
    /// ask 应答注册表（组合根注入缝；`new` 缺省空表 = 阻塞恒 miss，fail-closed）
    ask_registry: Arc<agent::AskRegistry>,
    /// ask 编排状态面（组合根注入缝；仅 `Flow` 档消费）
    ask_port: Option<Arc<dyn orchestration::port::AskPort>>,
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
            web_search: false,
            explore_store,
            workspace_root: None,
            ask_registry: Arc::new(agent::AskRegistry::default()),
            ask_port: None,
        }
    }

    /// ask 应答注册表旁路注入（builder 缝，与 `with_explore_note_store` 同型）：
    /// 组合根注入壳层托管的那张表（与内核侧同源）。
    pub fn with_ask_registry(self, registry: Arc<agent::AskRegistry>) -> Self {
        Self {
            ask_registry: registry,
            ..self
        }
    }

    /// ask 编排状态面旁路注入（builder 缝）：仅 `Flow` 档消费；`None` = 无编排
    /// 停等面（`Flow` 档执行体将以装配漂移 is_error 回灌）。
    pub fn with_ask_port(self, port: Option<Arc<dyn orchestration::port::AskPort>>) -> Self {
        Self {
            ask_port: port,
            ..self
        }
    }

    /// 输出上限旁路注入（builder 缝，与 `context_window` 并列的解码后载荷）：
    /// `new` 签名不动（既有调用点零改），`None` = 按标称窗长分档推导。
    pub fn with_max_tokens(self, max_tokens: Option<u64>) -> Self {
        Self { max_tokens, ..self }
    }

    /// 主仓 root 旁路注入（builder 缝，与 `with_max_tokens` 同型）：写入保护
    /// 规则装配的根。MUST 为主仓 root（工作区配置权威），MUST NOT 为会话执行
    /// 根（change 流程下是 worktree）。
    pub fn with_workspace_root(self, root: PathBuf) -> Self {
        Self {
            workspace_root: Some(root),
            ..self
        }
    }

    /// 云端 `web_search` 开关旁路注入（builder 缝，与 `with_max_tokens` 同
    /// 型）：值已由组合根按协议门控，runner 零协议判定；`new` 签名不动
    /// （既有调用点零改）。
    pub fn with_web_search(self, web_search: bool) -> Self {
        Self { web_search, ..self }
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
        // 标称窗长单点：输出上限推导与防线装配同源（两者都读这一处，避免
        // 「防线按一档、输出按另一档」的双源漂移）
        let window = spec.length.or(self.context_window);
        let (model, output_tokens) = build_model(&self.config, &spec, self.max_tokens, window);
        let (question_tx, question_rx) = mpsc::channel::<TurnQuestion>(QUESTION_CHANNEL_CAPACITY);
        let (observation_tx, observation_rx) =
            mpsc::channel::<agent::AgentEventKind>(EVENT_CHANNEL_CAPACITY);
        let handle = RunHandle::default();
        // 上下文防线装配（按档现算窗长：模型名后缀 > provider `context_length`
        // > 128K 缺省）+ 第一调用点：resume 重建史超 L2 水位即先行剪裁；
        // notice 交泵先于 RunStarted 转发（同任务同通道 FIFO 保序，重建史防线
        // 留痕）
        let defense = ContextDefense::resolve(window);
        let (history, resume_notices) = context::prune(history, &defense);
        tokio::spawn(session_pump(
            model,
            spec,
            open,
            PumpPayload {
                history,
                defense,
                output_tokens,
                resume_notices,
                protocol: self.config.protocol,
                web_search: self.web_search,
                raw_model: model_name,
                explore_store: self.explore_store.clone(),
                workspace_root: self.workspace_root.clone(),
                ask_registry: Arc::clone(&self.ask_registry),
                ask_port: self.ask_port.clone(),
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

/// rig 模型组装（`context_window` = 该档**标称**窗长，供输出上限分段推导；
/// None = 128K 缺省档）。
///
/// 返回值为 `(模型, 生效输出上限)`：上限与写进 wire 的值同源同值，调用方直接
/// 用于泵启动日志——日志列 MUST NOT 另走一遍取值链（否则日志可能与实际请求
/// 漂移）。openai 协议不写该字段，第二元恒 `None`。
pub(crate) fn build_model(
    config: &EngineConfig,
    spec: &ModelSpec,
    max_tokens: Option<u64>,
    context_window: Option<u64>,
) -> (DynModel<Completion>, Option<u64>) {
    match config.protocol {
        Protocol::Openai => (
            openai_provider(config)
                .client()
                .chat(spec.name.clone())
                .erase(),
            None,
        ),
        Protocol::Anthropic => {
            let mut model = with_prompt_cache(
                anthropic_provider(config, spec)
                    .client()
                    .completion(spec.name.clone()),
            );
            // 定型 wire 默认上限：wire `encode` 在请求 `max_tokens` 缺席时取该
            // 值，故 `loop` / `compact` 的 `max_tokens: None` 保持零改动
            let output_tokens =
                resolve_output_tokens(max_tokens, model.wire.default_max_tokens, context_window);
            model.wire.default_max_tokens = Some(output_tokens);
            (model.erase(), Some(output_tokens))
        }
    }
}

/// anthropic 模型的前缀缓存装配
pub(crate) fn with_prompt_cache(
    mut model: rig::Model<rig::providers::anthropic::Messages>,
) -> rig::Model<rig::providers::anthropic::Messages> {
    model.wire = model.wire.with_automatic_caching();
    model
}

/// openai 端点配置
pub(crate) fn openai_provider(config: &EngineConfig) -> OpenAIConfig {
    OpenAIConfig::new(config.api_key.clone()).with_base_url(config.base_url.clone())
}

/// anthropic 端点配置
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
/// 不降级）→ [`derive_output_tokens`] 按标称窗长分档推导（取代固定兜底）。
pub(crate) fn resolve_output_tokens(
    provider_value: Option<u64>,
    rig_default: Option<u64>,
    context_window: Option<u64>,
) -> u64 {
    provider_value
        .or(rig_default)
        .unwrap_or_else(|| derive_output_tokens(context_window))
}

/// 标称窗长 → 默认输出上限（纯函数，无 IO）：查 [`OUTPUT_TOKENS_SEGMENTS`]
/// 取「下界 <= 窗长」的最后一档；`None` / `Some(0)`（= 未配置）与
/// [`DEFAULT_CONTEXT_WINDOW`](crate::sdk::context::DEFAULT_CONTEXT_WINDOW) 同档
/// ——与防线装配的缺省口径同源，未配置用户的取值逐字不变。
pub(crate) fn derive_output_tokens(context_window: Option<u64>) -> u64 {
    let window = match context_window {
        Some(window) if window > 0 => window,
        _ => context::DEFAULT_CONTEXT_WINDOW,
    };
    OUTPUT_TOKENS_SEGMENTS
        .iter()
        .rev()
        .find(|(lower, _)| *lower <= window)
        .map(|(_, tokens)| *tokens)
        .unwrap_or(MIN_OUTPUT_TOKENS)
}

/// 泵任务的防线与史载荷（参数面收敛分组：重建史 + 防线配置 + 重建剪裁的
/// notice 留痕 + explore 专用工具句柄）。
struct PumpPayload {
    /// resume 重建史（已经第一调用点 L2 先行剪裁）
    history: Vec<Message>,
    /// 上下文窗防线配置
    defense: ContextDefense,
    /// 该次装配生效的 wire 输出上限（日志 `max_tokens=…`；anthropic 恒有值，
    /// openai 恒 `None`——协议不写该字段）
    output_tokens: Option<u64>,
    /// 重建剪裁产出的 notice（泵内先于 RunStarted 转发）
    resume_notices: Vec<DefenseNotice>,
    /// 模型协议（泵启动日志诊断面 `protocol=…`）
    protocol: Protocol,
    /// 云端 `web_search` 开关（泵启动日志诊断面 `web_search=…`；已门控值）
    web_search: bool,
    /// 该档模型原文（日志 `raw=…`；上报与请求用裸名——见 `ModelSpec.name`）
    raw_model: String,
    /// workspace 库句柄（组合根注入缝）——名字清单含 `create_explore_note` 的
    /// 会话 per-turn 铸造该工具执行上下文的原料（None = 未注入）
    explore_store: Option<Arc<Store>>,
    /// 主仓 root（组合根注入缝）——写入保护规则装配的根（`None` = 未注入 →
    /// 只装载内置规则）
    workspace_root: Option<PathBuf>,
    /// ask 应答注册表（组合根注入缝）：per-turn 铸造 `AskContext` 的句柄
    ask_registry: Arc<agent::AskRegistry>,
    /// ask 编排状态面（组合根注入缝）：仅 `Flow` 档随上下文携带
    ask_port: Option<Arc<dyn orchestration::port::AskPort>>,
}

/// 架构只读工具执行上下文 per-turn 铸造（单点；`session_pump` 唯一调用处）：
/// 本轮名字清单含任一 archi 工具名即铸（轮 root = 会话 cwd——change 会话即
/// worktree），否则 `None`。铸造值随 `LoopTurn` 携带、loop 侧路由，不经
/// `tools::execute`。
pub(crate) fn mint_archi_context(names: &[String], workspace_root: &Path) -> Option<ArchiContext> {
    let present = names
        .iter()
        .any(|name| name == ARCHI_MODULE_TOOL || name == ARCHI_SEARCH_TOOL);
    present.then(|| ArchiContext::new(workspace_root.to_path_buf()))
}

/// 本轮名字清单是否包含某工具名（per-turn 铸造判据单点）。
fn contains_tool(injections: &agent::SessionInjections, tool: &str) -> bool {
    injections
        .tools
        .as_deref()
        .is_some_and(|names| names.iter().any(|name| name == tool))
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
    // `create_explore_note` 专用工具执行上下文 per-turn 铸造：名字清单含该工具
    // 且 store 句柄在场才铸（Continue id 缺席则 None——生产不可达，explore 会话
    // 恒 Continue）；上下文随 LoopTurn 携带、loop 侧路由，不经 `tools::execute`
    let explore_note = if contains_tool(&open.injections, EXPLORE_NOTE_TOOL) {
        payload
            .explore_store
            .zip(continue_session_id(&open))
            .map(|(store, id)| ExploreNoteContext::new(store, open.ctx.workspace_root.clone(), id))
    } else {
        None
    };
    // 架构只读工具执行上下文 per-turn 铸造：名字清单含任一 archi 工具名即铸；
    // 上下文随 LoopTurn 携带、loop 侧路由
    let archi = mint_archi_context(
        open.injections.tools.as_deref().unwrap_or_default(),
        &open.ctx.workspace_root,
    );
    // ask 执行上下文 per-turn 铸造：判据 = `ask` 位存在（与 explore / archi 的
    // 「名字包含」判据并列的第二类铸造判据）；`core_session_id` 取
    // `SessionOpen.session_id`（core 铸 id），MUST NOT 取引擎侧 `sdk-…` 标识
    let ask = open.injections.ask.map(|mode| r#loop::AskContext {
        registry: Arc::clone(&payload.ask_registry),
        core_session_id: open.session_id.clone(),
        port: if mode == agent::AskMode::Flow {
            payload.ask_port.clone()
        } else {
            None
        },
    });
    // 泵启动日志（诊断面：同一 base_url 配错协议可一眼定位）。`model` 报裸名
    // （= 实际发送）、`window` 报**生效**窗长（含 128K 缺省）、`max_tokens` 报
    // **生效**输出上限（openai 协议不写该字段，记 `-`）、`raw` 留配置原文
    let session_id = SdkRunner::next_session_id();
    let window = payload.defense.window();
    let output = payload
        .output_tokens
        .map_or_else(|| "-".to_owned(), |tokens| tokens.to_string());
    crate::sdk::log::append_engine_log(&format!(
        "泵启动 session={session_id} protocol={} model={} window={window} max_tokens={output} web_search={} raw={}",
        payload.protocol.as_str(),
        spec.name,
        payload.web_search,
        payload.raw_model
    ));
    // 写入保护规则装配（**每 run 一次、protect 模块唯一 IO 点**）：规则随
    // `LoopTurn` 下发，loop 内零 IO 判定（run 内改配置不改本轮行为）
    let protect = ProtectRules::load(payload.workspace_root.as_deref());
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
        archi,
        ask,
        protect,
        web_search: payload.web_search,
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
