mod archi_knowledge;
mod compose;
mod sdk;
mod store_port;
mod worker;

use std::path::PathBuf;
use std::sync::Arc;

use agent::AgentRunner;

pub use compose::{compose_turn, ComposedTurn};
pub use sdk::config::EngineConfig;
pub use sdk::log::{append_engine_log, init_engine_log, install_engine_panic_hook};
pub use sdk::model_spec::{parse_model_spec, ModelSpec, ModelSpecError};
pub use sdk::runner::MIN_OUTPUT_TOKENS;
pub use store_port::session_query;
pub use worker::KernelWorkerPort;

/// 引擎二值：`agent_start` 的 `engine` 参数值域（serde/specta camelCase，
/// 线格式 `"cli" | "sdk"`；`Option` 承载缺省）。`Cli` 为退役态·协议预留——
/// 多租户机制（本机 CLI / 进程内 SDK / 远程 API）保留，CLI 租户实现已随
/// retire-cli-engine 退役、无生产构造路径。住 infra 门面，core 契约零污染。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EngineKind {
    /// 本机 claude CLI 租户（退役态·协议预留）
    Cli,
    /// 进程内 sdk 租户（唯一活档）
    Sdk,
}

/// 续会话转录装载缝（引擎中立数据）：session_id → 事件转录。`None` = 会话
/// 不存在或非 sdk 产出；`Err` = 库读取失败。sdk 引擎 `start()` 时调用并
/// 重建对话史。
pub type ResumeTranscript =
    std::sync::Arc<dyn Fn(&str) -> Result<Option<Vec<agent::AgentEvent>>, String> + Send + Sync>;

/// 引擎门面：`runner_for` 为引擎构造唯一 match 点（编排层零引擎分支）。
/// `resume` 装载缝、sdk 引擎可空调参载荷（窗长 / 输出上限）与 explore 专用
/// 工具 store 句柄随门面持有，按引擎分发。
pub struct EngineFacade {
    resume: Option<ResumeTranscript>,
    /// sdk 引擎上下文窗长（组合根旁路载荷，不经 `EngineConfig` 连接配置字段
    /// 面；`None` = 128K 缺省启发式，该档模型名带 `[1m]` 后缀时被覆盖）
    context_window: Option<u64>,
    /// sdk 引擎输出上限（组合根旁路载荷，不经 `EngineConfig`；`None` = 按
    /// 协议缺省，仅 anthropic 协议消费）
    max_tokens: Option<u64>,
    /// sdk 引擎云端 `web_search` 服务端工具开关（组合根旁路载荷；语义为
    /// 「**已按协议门控**的最终开关」——门面 MUST NOT 二次判定协议）
    web_search: bool,
    /// workspace 库句柄（组合根注入缝）：per-turn 铸造 `create_explore_note`
    /// 专用工具执行上下文的原料（`None` = 未注入——专用工具上下文缺席）
    explore_note_store: Option<Arc<store::Store>>,
    /// sdk 引擎主仓 root（组合根注入缝）：写入保护规则装配的根（`None` =
    /// 未注入 → 只装载内置规则，非「保护关闭」）
    workspace_root: Option<PathBuf>,
}

impl EngineFacade {
    /// 无续会话注入的门面。
    pub fn new() -> Self {
        Self {
            resume: None,
            context_window: None,
            max_tokens: None,
            web_search: false,
            explore_note_store: None,
            workspace_root: None,
        }
    }

    /// 携 store 转录装载缝的门面：sdk 引擎续会话经此解析。
    pub fn with_resume_transcript(loader: ResumeTranscript) -> Self {
        Self {
            resume: Some(loader),
            context_window: None,
            max_tokens: None,
            web_search: false,
            explore_note_store: None,
            workspace_root: None,
        }
    }

    /// 携 sdk 引擎上下文窗长的门面（builder 缝，与 `with_resume_transcript`
    /// 同型）：provider 记录 `context_length` 列经组合根旁路抵达 sdk 引擎
    /// （`EngineConfig` 连接配置字段面与 `runner_for` 签名不动）。语义为
    /// 「缺省来源」——该档模型名带 `[1m]` 后缀时被后缀覆盖。
    pub fn with_context_window(self, context_window: Option<u64>) -> Self {
        Self {
            resume: self.resume,
            context_window,
            max_tokens: self.max_tokens,
            web_search: self.web_search,
            explore_note_store: self.explore_note_store,
            workspace_root: self.workspace_root,
        }
    }

    /// 携 sdk 引擎输出上限的门面（builder 缝，与 `with_context_window` 同型
    /// 并列）：provider 记录 `max_tokens` 列经组合根旁路抵达 sdk 引擎
    /// （MUST NOT 进 `EngineConfig`）；`None` = 按协议缺省。
    pub fn with_max_tokens(self, max_tokens: Option<u64>) -> Self {
        Self {
            resume: self.resume,
            context_window: self.context_window,
            max_tokens,
            web_search: self.web_search,
            explore_note_store: self.explore_note_store,
            workspace_root: self.workspace_root,
        }
    }

    /// 携 sdk 引擎云端 `web_search` 开关的门面（builder 缝，与
    /// `with_context_window` / `with_max_tokens` 同型并列）：provider 记录
    /// `web_search` 经组合根**按协议门控**后下行；门面与 runner 零协议判定，
    /// 收 `bool`（`false` = 未启用，与「未装配」同义）。
    pub fn with_web_search(self, web_search: bool) -> Self {
        Self {
            resume: self.resume,
            context_window: self.context_window,
            max_tokens: self.max_tokens,
            web_search,
            explore_note_store: self.explore_note_store,
            workspace_root: self.workspace_root,
        }
    }

    /// 携 workspace 库句柄的门面（builder 缝，与 `with_resume_transcript` /
    /// `with_context_window` 同型）：组合根 store 句柄注入缝——per-turn 铸造
    /// `create_explore_note` 专用工具执行上下文的铸造原料（pump 侧仅
    /// `toolset == Explore` 且句柄在场时铸造）。
    pub fn with_explore_note_store(self, store: Arc<store::Store>) -> Self {
        Self {
            resume: self.resume,
            context_window: self.context_window,
            max_tokens: self.max_tokens,
            web_search: self.web_search,
            explore_note_store: Some(store),
            workspace_root: self.workspace_root,
        }
    }

    /// 携主仓 root 的门面（builder 缝，与 `with_explore_note_store` 并列）：
    /// 写入保护规则的装配根。MUST 为主仓 root（工作区配置权威所在），MUST NOT
    /// 为会话执行根（change 流程下是 worktree）；`None` = 只装载内置规则。
    pub fn with_workspace_root(self, root: PathBuf) -> Self {
        Self {
            workspace_root: Some(root),
            ..self
        }
    }

    /// 引擎构造唯一 match 点：kind 选臂、engine_cfg 供 sdk 臂组装 rig
    /// client。引擎具体类型不出门面。`EngineKind::Cli` 臂退役为显式 `Err`
    /// （[`store::CLI_TENANT_RETIRED`] 逐字文案）——生产路径不可达（组合根
    /// `resolve_agent_engine` 先行拒 Cli），该臂为结构保留 + 防御兜底，
    /// **唯一 match 点结构与 `EngineKind` 双值不塌**。
    pub fn runner_for(
        &self,
        kind: EngineKind,
        engine_cfg: EngineConfig,
    ) -> Result<Box<dyn AgentRunner>, String> {
        match kind {
            EngineKind::Cli => Err(store::CLI_TENANT_RETIRED.to_owned()),
            EngineKind::Sdk => {
                let mut runner = sdk::runner::SdkRunner::new(
                    engine_cfg,
                    self.resume.clone(),
                    self.context_window,
                    self.explore_note_store.clone(),
                )
                .with_max_tokens(self.max_tokens)
                .with_web_search(self.web_search);
                if let Some(root) = &self.workspace_root {
                    runner = runner.with_workspace_root(root.clone());
                }
                Ok(Box::new(runner))
            }
        }
    }
}

impl Default for EngineFacade {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod lib_test;

#[cfg(test)]
mod archi_knowledge_test;

#[cfg(test)]
pub(crate) static TEST_PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
