//! 运行契约：`AgentRunner` trait、运行参数、逻辑事件流与运行句柄。
//!
//! trait 面仅暴露逻辑事件与运行参数：`start` 返回「逻辑事件流 + 句柄」，
//! 进程模型（spawn、stdout/stdin、退出码）MUST NOT 出现在 trait 面上。
//! 三租户（本机 CLI / 进程内 SDK / 远程 API）都应落在本 trait 预留内。

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::Notify;

use crate::event::AgentEvent;

/// 环境档位双档：`default`（完整环境）/ `bare`（纯净档；不读 OAuth 凭据，
/// 须 `ANTHROPIC_API_KEY` 等外部认证前提——提示责任在参数面，不在本 crate）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentEnvMode {
    /// 完整环境（页面默认档）
    Default,
    /// 纯净档（显式开关）
    Bare,
}

/// permission-mode 三档；无头模式下档位决定工具审批行为（档位语义由能力
/// spec 留痕，本 crate 不解释）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentPermissionMode {
    /// CLI 默认档（需审批工具在无头下直接被拒）
    Default,
    /// 自动接受文件编辑
    AcceptEdits,
    /// 跳过全部审批（调试页默认档）
    BypassPermissions,
}

/// run 状态四档（落库 / 出线契约）：与 [`crate::state::AgentRunState`] 状态机
/// 内存态两型并存——本枚举是 serde camelCase 线格式与 store 落库形态的值域
/// 契约，状态机类型不出契约面（编排侧显式 `match` 映射）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentRunStatus {
    /// 运行中（run begin 落库行初值）
    Running,
    /// 正常收敛（result 事件 is_error=false）
    Completed,
    /// 失败收敛（result 事件 is_error=true / 无 result 异常终止 / 落库失败）
    Failed,
    /// 用户主动终止收敛（agent_stop 显式请求，语义区别于 CLI 失败）
    Stopped,
}

/// 一次运行的入参：trait 面只认逻辑参数；cwd 由壳层注入（隐含当前
/// workspace root，无用户输入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunParams {
    /// 提示词（必填，空串交由上层参数面禁用）
    pub prompt: String,
    /// 工作目录
    pub cwd: PathBuf,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
    /// 续会话入参（唯一进契约的续会话参数）：非空时以该 session 续发新一轮，
    /// 由实现方翻译为传输层形态（CLI 租户为 `--resume <id>` flag）；`None`
    /// 即全新 one-shot 运行。`--continue` 隐式续会话不进参数面。
    pub resume_session_id: Option<String>,
}

/// 运行句柄：逻辑终止信号（trait 形状「事件流 + 句柄」的句柄半边）。信号
/// 面零进程类型——句柄只承载「终止已请求」状态与等待原语，进程树击杀等
/// kill 机制归租户实现（租户经 `wait_requested` 或 `stop_requested` 观测
/// 信号后自行终止其进程模型）。`Clone` 共享同一信号：编排侧持有一份、
/// 租户泵持有一份，任一置位双方可见。
#[derive(Debug, Clone, Default)]
pub struct RunHandle {
    /// 终止请求标志（置位后不可清除；首个收敛生效语义的信号半边）
    stop_requested: Arc<AtomicBool>,
    /// 等待方唤醒原语：`request_stop` 置位后唤醒所有在途等待
    notify: Arc<Notify>,
}

impl RunHandle {
    /// 置位终止请求并唤醒等待方（幂等：重复置位无副作用、不重复 panic）。
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// 同步观测终止请求（编排侧 EOF 收敛判定：状态机未收敛但信号已置位
    /// → 显式收敛 stopped）。
    pub fn stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::SeqCst)
    }

    /// 异步等待终止请求（租户泵 select 半边）：已置位立即返回；未置位挂起
    /// 至 `request_stop` 唤醒。置位与注册等待之间的竞态经「先注册、再复查」
    /// 关闭（enable 先于复查，复查先于挂起）。
    pub async fn wait_requested(&self) {
        if self.stop_requested() {
            return;
        }
        let mut notified = std::pin::pin!(self.notify.notified());
        notified.as_mut().enable();
        if self.stop_requested() {
            return;
        }
        notified.await;
    }
}

/// 一次运行的产出：逻辑事件流（有界 mpsc）+ 运行句柄。
#[derive(Debug)]
pub struct AgentRun {
    /// 逻辑事件流：消费端关闭后生产端自行停止
    pub events: tokio::sync::mpsc::Receiver<AgentEvent>,
    /// 运行句柄（逻辑终止信号，kill 机制归租户）
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
    /// 启动所需配置缺失或非法（认证凭据未配 / 模型标识缺失 / 引用的会话
    /// 不存在或不属产出方等；消息区分成因）。中性命名：契约面不解释配置
    /// 从何而来，也不出现任何引擎字样。
    ConfigMissing(String),
}

impl fmt::Display for AgentStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CliMissing(msg) => write!(f, "CLI 未找到: {msg}"),
            Self::SpawnFailed(msg) => write!(f, "启动失败: {msg}"),
            Self::ConfigMissing(msg) => write!(f, "配置缺失: {msg}"),
        }
    }
}

/// agent 运行器中立契约：唯一方法 `start`，进程模型不可见。
/// 实现方自泵事件入 [`AgentRun::events`]（seq 每 run 从 0 单调递增）。
pub trait AgentRunner: Send + Sync {
    /// 发起一次运行：启动阶段失败返回 [`AgentStartError`]，不产生任何事件。
    fn start(&self, params: AgentRunParams) -> Result<AgentRun, AgentStartError>;
}
