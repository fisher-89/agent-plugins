//! 运行契约：`AgentRunner` trait、运行参数、逻辑事件流与运行句柄。
//!
//! trait 面仅暴露逻辑事件与运行参数：`start` 返回「逻辑事件流 + 句柄」，
//! 进程模型（spawn、stdout/stdin、退出码）MUST NOT 出现在 trait 面上。
//! 三租户（本机 CLI / 进程内 SDK / 远程 API）都应落在本 trait 预留内。

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::event::AgentEvent;

/// 环境档位双档：`default`（完整环境）/ `bare`（纯净档；不读 OAuth 凭据，
/// 须 `ANTHROPIC_API_KEY` 等外部认证前提——提示责任在参数面，不在本 crate）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentEnvMode {
    /// 完整环境（页面默认档）
    Default,
    /// 纯净档（显式开关）
    Bare,
}

impl AgentEnvMode {
    /// 受控字符串（落库口径，与 serde 线格式一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Bare => "bare",
        }
    }
}

/// permission-mode 三档；无头模式下档位决定工具审批行为（档位语义由能力
/// spec 留痕，本 crate 不解释）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentPermissionMode {
    /// CLI 默认档（需审批工具在无头下直接被拒）
    Default,
    /// 自动接受文件编辑
    AcceptEdits,
    /// 跳过全部审批（调试页默认档）
    BypassPermissions,
}

impl AgentPermissionMode {
    /// 受控字符串（落库口径，与 serde 线格式一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::BypassPermissions => "bypassPermissions",
        }
    }
}

/// 一次运行的入参：trait 面只认逻辑参数；cwd 由壳层注入（隐含当前
/// workspace root，无用户输入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunParams {
    /// 提示词（必填，空串交由上层参数面禁用）
    pub prompt: String,
    /// 工作目录
    pub cwd: PathBuf,
    /// 环境档位
    pub env: AgentEnvMode,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
}

/// 运行句柄：MVP 无 kill / cancel，刻意为空结构，作为未来终止能力的
/// 挂点（trait 形状「事件流 + 句柄」的句柄半边）。
#[derive(Debug, Clone, Copy)]
pub struct RunHandle;

/// 一次运行的产出：逻辑事件流（有界 mpsc）+ 运行句柄。
#[derive(Debug)]
pub struct AgentRun {
    /// 逻辑事件流：消费端关闭后生产端自行停止
    pub events: tokio::sync::mpsc::Receiver<AgentEvent>,
    /// 运行句柄（MVP 预留）
    pub handle: RunHandle,
}

/// 启动阶段失败（区别于运行内失败——后者由 `RunResult.is_error` 表达）：
/// 此形态的失败不产生 run 记录，直接以 `Err` 抵达前端。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStartError {
    /// CLI 不可发现
    CliMissing(String),
    /// 启动进程失败
    SpawnFailed(String),
}

impl fmt::Display for AgentStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CliMissing(msg) => write!(f, "CLI 未找到: {msg}"),
            Self::SpawnFailed(msg) => write!(f, "启动失败: {msg}"),
        }
    }
}

/// agent 运行器中立契约：唯一方法 `start`，进程模型不可见。
/// 实现方自泵事件入 [`AgentRun::events`]（seq 每 run 从 0 单调递增）。
pub trait AgentRunner: Send + Sync {
    /// 发起一次运行：启动阶段失败返回 [`AgentStartError`]，不产生任何事件。
    fn start(&self, params: AgentRunParams) -> Result<AgentRun, AgentStartError>;
}
