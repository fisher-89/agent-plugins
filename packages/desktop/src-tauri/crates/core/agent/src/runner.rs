use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::Notify;

use crate::event::AgentEventKind;

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
    /// 运行中（轮 begin 落库行初值）
    Running,
    /// 正常收敛（TurnDone 事件 is_error=false）
    Completed,
    /// 失败收敛（TurnDone 事件 is_error=true / 无 TurnDone 异常终止 / 落库失败）
    Failed,
    /// 用户主动终止收敛（stop 显式请求，语义区别于引擎失败）
    Stopped,
}

/// 会话级注入：preamble（系统前导）与 tools（工具面）为切片③预留接口位，
/// MVP 引擎忽略（整合方式——preamble / flag / 忽略——引擎自选，协议形状
/// 不因整合方式改变）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionInjections {
    /// 会话级系统前导（None 即不注入）
    pub preamble: Option<String>,
    /// 会话级工具面收窄（None 即引擎默认工具面）
    pub tools: Option<Vec<String>>,
}

/// 模型等级（轮级选型档位）：引擎侧按档取 provider 双档模型（High = 高能力
/// 档，Low = 轻量档）；档位语义归编排 / 相位表解释，本 crate 只承载词汇。
/// 缺省 High——additive 演进旧 JSON 反序列化与既有单模型行为对齐。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelLevel {
    #[default]
    High,
    Low,
}

/// 轮级上下文：workspace root + permission-mode + 模型等级。serde 不加
/// `deny_unknown_fields`（未知字段忽略）——additive 演进不破线格式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCtx {
    /// 工作目录（隐含当前 workspace root，无用户输入）
    pub workspace_root: PathBuf,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
    /// 模型等级（缺省 High，旧形态 JSON 反序列化承接）
    #[serde(default)]
    pub model_level: ModelLevel,
}

/// 会话引用（协议寻址单位）：`New` 建立新会话；`Continue { id }` 以既有会话
/// 续发新一轮。`--continue` 类隐式续会话不进参数面。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRef {
    /// 新建会话
    New,
    /// 以既有会话续发
    Continue { id: String },
}

/// 会话建立参数（协议唯一入口的入参）：注入面 + 上下文 + 会话引用 + 引擎侧
/// 先行句柄（Continue 时由编排侧自会话记录提取回供；New 恒 None）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOpen {
    /// 会话级注入（injections/轮分离不变量的会话级半边）
    pub injections: SessionInjections,
    /// 轮级上下文
    pub ctx: SessionCtx,
    /// 会话引用
    pub session: SessionRef,
    /// 引擎侧先行句柄（引擎自译为传输层形态；None 即全新会话）
    pub prior_handle: Option<String>,
}

/// 轮级驱动词汇：一次提问。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnQuestion {
    /// 提示词（必填，空串交由上层参数面禁用）
    pub prompt: String,
}

#[derive(Debug)]
pub struct AgentSession {
    /// 观察回流：引擎产未盖戳事件种类，seq / 时间戳由内核统一盖戳
    pub observations: tokio::sync::mpsc::Receiver<AgentEventKind>,
    /// 轮驱动：单问送达（一轮一命——引擎泵服务恰此一问后返回，观测通道随
    /// 之关闭）
    pub questions: tokio::sync::mpsc::Sender<TurnQuestion>,
    /// 运行句柄（逻辑终止信号，kill 机制归租户实现）
    pub handle: RunHandle,
}

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

pub trait AgentRunner: Send + Sync {
    fn open_session(&self, open: SessionOpen) -> Result<AgentSession, AgentStartError>;
}
