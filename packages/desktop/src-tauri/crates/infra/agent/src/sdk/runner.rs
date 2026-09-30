//! `SdkRunner`：sdk 引擎的 [`agent::AgentRunner`] 契约实现。
//!
//! `start` 同步段：配置三件套校验（任一缺失 → `ConfigMissing`，不产生 run
//! 记录）→ 续会话解析（经 [`crate::ResumeTranscript`] 注入缝：loader
//! `Err` / `None` / 非 `sdk-` 前缀 → `ConfigMissing`）→ rig client 组装
//! （openai chat completions + 自定义 base_url + model）→ 生成 `sdk-` 前缀
//! 会话 id → 有界 mpsc + 泵任务 spawn。
//!
//! 泵任务与 CLI `pump_lines` 同构（select 语义同一停止抹平）：`tokio::select!`
//! 包裹 loop future 与 `RunHandle.wait_requested()`——停止即 drop loop
//! future（SDK 无进程树可杀，取消即中止流读取），不合成 RunResult，编排侧
//! 显式收敛 stopped；消费端关闭由 loop 的事件发送失败自行退出。

use std::sync::atomic::{AtomicU64, Ordering};

use agent::{AgentRun, AgentRunParams, AgentRunner, AgentStartError, RunHandle};
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

/// sdk 会话 id 进程内原子计数（配合毫秒时戳保证同毫秒不重号）。
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// sdk 引擎 runner：持有连接配置与续会话转录装载缝；无跨 run 共享运行态
/// （全部运行态在 `start` 产出的 [`AgentRun`] 内）。
pub struct SdkRunner {
    config: EngineConfig,
    resume: Option<ResumeTranscript>,
}

impl SdkRunner {
    /// runner 构造（配置与装载缝由门面注入，引擎细节不出门面）。
    pub fn new(config: EngineConfig, resume: Option<ResumeTranscript>) -> Self {
        Self { config, resume }
    }

    /// `sdk-<进程内计数>-<毫秒时戳>` 会话 id：前缀即引擎归属标记（resume
    /// 归属校验凭据，见 [`resume::owns_session`]）。
    fn next_session_id() -> String {
        let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        format!("{}{counter}-{millis}", resume::SESSION_PREFIX)
    }

    /// 续会话解析：全新运行空史；显式续会话经装载缝取转录并重建 rig 对话
    /// 史——loader `Err`（库读取失败）/ `None`（会话不存在或非 sdk 产出）/
    /// 非 `sdk-` 前缀 / 重建空史，全部以 [`AgentStartError::ConfigMissing`]
    /// 显式失败（不产生 run 记录，`Err` 抵达前端）。
    fn resolve_resume(
        &self,
        resume_session_id: &Option<String>,
    ) -> Result<Vec<Message>, AgentStartError> {
        let Some(session_id) = resume_session_id else {
            return Ok(Vec::new());
        };
        if !resume::owns_session(session_id) {
            return Err(AgentStartError::ConfigMissing(format!(
                "会话不存在或非 SDK 产出: {session_id}"
            )));
        }
        let Some(loader) = &self.resume else {
            return Err(AgentStartError::ConfigMissing(format!(
                "转录装载器未注入，无法续会话: {session_id}"
            )));
        };
        let events = loader(session_id)
            .map_err(|error| AgentStartError::ConfigMissing(format!("转录读取失败: {error}")))?
            .ok_or_else(|| {
                AgentStartError::ConfigMissing(format!("会话不存在或非 SDK 产出: {session_id}"))
            })?;
        resume::rebuild(&events).map_err(AgentStartError::ConfigMissing)
    }
}

impl AgentRunner for SdkRunner {
    fn start(&self, params: AgentRunParams) -> Result<AgentRun, AgentStartError> {
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
        let history = self.resolve_resume(&params.resume_session_id)?;
        let model = build_model(&self.config)?;
        let session_id = SdkRunner::next_session_id();
        let (sender, receiver) = mpsc::channel::<agent::AgentEvent>(EVENT_CHANNEL_CAPACITY);
        let handle = RunHandle::default();
        let model_name = self.config.model.clone();
        let pump_handle = handle.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = r#loop::run(model, params, model_name, history, session_id, sender, pump_handle.clone()) => {}
                _ = pump_handle.wait_requested() => {}
            }
        });
        Ok(AgentRun {
            events: receiver,
            handle,
        })
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
