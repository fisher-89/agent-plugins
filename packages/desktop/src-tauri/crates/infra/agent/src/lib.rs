//! `agent-runtime`：agent 边界的引擎门面与双引擎实现（infra 侧）。
//!
//! 门面 [`EngineFacade`] 是引擎构造唯一 match 点：壳层经 [`EngineKind`]
//! 参数选择引擎，编排层零引擎分支（引擎类型不出门面，`runner_for` 返回
//! `Box<dyn AgentRunner>`）。两租户实现 core [`agent::AgentRunner`] 契约，
//! trait / 事件 / 运行参数 / 句柄面零变化：
//! - [`cli`]：本机 claude CLI 租户（本 workspace 唯一 spawn 触点）
//!   —— [`discover`] 入口发现、[`build_args`] flag 组装、[`normalize_line`]
//!   stdout JSONL 归一化、[`ClaudeCliRunner`] 进程泵
//! - `sdk`：进程内直连 openai 兼容端点租户（rig-core provider client +
//!   手搓多轮 loop），开箱即跑、不依赖本机 CLI
//!
//! 续会话转录装载缝 [`ResumeTranscript`] 为引擎中立参数：壳层组装、sdk
//! 引擎消费、cli 引擎持有不消费（CLI 续会话仍走 `--resume` flag）。
//!
//! 本 crate 禁 Tauri 系依赖；进程 spawn 是 cli 引擎自身的实现细节，不属
//! shell 边界职责。
//!
//! 能力 spec：`specs/desktop-agent-execution/spec.md`（路径相对域根）。

mod cli;
mod sdk;

use agent::AgentRunner;

pub use cli::runner::ClaudeCliRunner;
pub use sdk::config::EngineConfig;

// 平移模块的 crate 根路径别名：cli/ 四实现文件与其测试自 `src/` 平移落位
// （移动非删除、内容零改动），文件内的 `crate::<module>` 路径经此别名继续
// 可解析（实现与测试两侧同源；`discover` 等模块别名与同名函数值分属类型 /
// 值两命名空间，不冲突）
pub use cli::{discover, flags, jsonl, runner};

/// 引擎二值：`agent_start` 的 `engine` 参数值域（serde/specta camelCase，
/// 线格式 `"cli" | "sdk"`；`Option` 承载缺省，不传即 CLI 租户、行为与演进
/// 前一致）。住 infra 门面，core 契约零污染。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EngineKind {
    /// 本机 claude CLI 租户（缺省）
    Cli,
    /// 进程内 sdk 租户（rig-core 直连 openai 兼容端点）
    Sdk,
}

/// 续会话转录装载缝（引擎中立数据）：session_id → 事件转录。`None` = 会话
/// 不存在或非 sdk 产出；`Err` = 库读取失败。sdk 引擎 `start()` 时调用并
/// 重建对话史；cli 引擎持有不消费。
pub type ResumeTranscript =
    std::sync::Arc<dyn Fn(&str) -> Result<Option<Vec<agent::AgentEvent>>, String> + Send + Sync>;

/// 引擎门面：`runner_for` 为引擎构造唯一 match 点（编排层零引擎分支）。
/// `resume` 装载缝随门面持有，按引擎分发（CLI 臂不消费）。
pub struct EngineFacade {
    resume: Option<ResumeTranscript>,
}

impl EngineFacade {
    /// 无续会话注入的门面（CLI 路径零成本）。
    pub fn new() -> Self {
        Self { resume: None }
    }

    /// 携 store 转录装载缝的门面：sdk 引擎续会话经此解析，CLI 引擎持有
    /// 不消费。
    pub fn with_resume_transcript(loader: ResumeTranscript) -> Self {
        Self {
            resume: Some(loader),
        }
    }

    /// 引擎构造唯一 match 点：kind 选臂、engine_cfg 供 sdk 臂组装 rig
    /// client（CLI 臂忽略）。引擎具体类型不出门面。
    pub fn runner_for(&self, kind: EngineKind, engine_cfg: EngineConfig) -> Box<dyn AgentRunner> {
        match kind {
            EngineKind::Cli => Box::new(ClaudeCliRunner::new()),
            EngineKind::Sdk => {
                Box::new(sdk::runner::SdkRunner::new(engine_cfg, self.resume.clone()))
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
