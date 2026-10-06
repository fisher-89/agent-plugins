//! run 状态与步状态类型：`RunUpdate` Channel 信封（tag `ipc`）、运行状态机
//! 词汇、步状态行与快照 / 摘要 DTO。serde camelCase + specta `Type`——IPC
//! 直用，前端 bindings 再生成直出线格式（与 `AgentRunMessage` 同式先例）。

use serde::{Deserialize, Serialize};
use specta::Type;

/// run 状态六档（camelCase 线格式）：三态运行期（running / 两类停等）与
/// 三态终局（completed / stopped / failed）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChangeRunStatus {
    /// 运行中
    Running,
    /// phase 间停等用户确认
    WaitingConfirm,
    /// ask 中断，等待自由文本应答
    WaitingAsk,
    /// 全相位 pass 走完（不触发归档，停等用户）
    Completed,
    /// 受控终止（用户停止 / confirm=false / 决策 stop 动作）
    Stopped,
    /// 失败终止（写面 / 会话失败 / 解析失败显式停给用户）
    Failed,
}

impl ChangeRunStatus {
    /// 终局判别：终态 run 自注册表除名、订阅释放（图回落派生规则）。
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Stopped | Self::Failed)
    }
}

/// 步词汇（三类节点可辨）：WorkerAgent 三角色 + 相位机 / 工具步三步 + Gate
/// 三门。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChangeStepKind {
    /// executor 会话
    Executor,
    /// evaluator 会话
    Evaluator,
    /// 决策会话
    Decision,
    /// phase-start 相位机步
    PhaseStart,
    /// static-check 工具步
    StaticCheck,
    /// test-execution 门禁工具步（确定性测试执行链，绿跑零 agent）
    TestExecution,
    /// phase-log 相位机步
    PhaseLog,
    /// verdict 解析门
    VerdictGate,
    /// retry 预算门（phase-next 重试 / 上限分叉）
    RetryGate,
    /// backtrack 白名单门（决策越权预校验）
    WhitelistGate,
}

/// 步状态四档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChangeStepStatus {
    /// 步进行中（节点 pulse 运行态）
    Running,
    /// 步通过
    Passed,
    /// 步失败（图上红节点）
    Failed,
    /// 步因停止 / 终止收敛（非失败语义）
    Stopped,
}

/// 步状态行：一次 `RunUpdate::Step` 的载荷，运行步节点推导的唯一输入面。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeStepState {
    /// 相位 id
    pub phase: String,
    /// attempt 号
    pub attempt: u32,
    /// 步词汇
    pub step: ChangeStepKind,
    /// 步状态
    pub status: ChangeStepStatus,
    /// WorkerAgent 步所属会话 id（工具步 / Gate 步为 None）
    pub session_id: Option<String>,
    /// 人读详情（失败记因 / 通过摘要）
    pub detail: Option<String>,
}

/// ask 载荷：决策会话 ask 动作的中断问题与候选选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AskPayload {
    pub question: String,
    pub options: Vec<String>,
}

/// 重挂快照：进程内 run 控制注册表在 view 重建时的状态恢复面（run 终态后
/// 为 None——快照只覆盖运行期，终态由图派生规则承载）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRunSnapshot {
    pub run_id: String,
    pub status: ChangeRunStatus,
    /// 当前相位（停等定位用）
    pub phase: Option<String>,
    /// 当前 attempt
    pub attempt: Option<u32>,
    /// waitingAsk 时的中断载荷
    pub ask: Option<AskPayload>,
}

/// 发起提前 resolve 返回值：run_id 立即可知，运行态经 Channel 流出（与
/// agent_start 提前 resolve 契约同型）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRunSummary {
    pub run_id: String,
    pub status: ChangeRunStatus,
}

/// run 状态流信封（tag `ipc` 判别，TS 镜像经 bindings 再生成直出）：
/// 步状态上图、会话事件透传转录面板、ask / 确认停等驱动控制面板、终态收口。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "ipc", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RunUpdate {
    /// 步状态变更（运行步节点上图输入）
    Step { step: ChangeStepState },
    /// WorkerAgent 会话事件透传（转录面板实时流）
    SessionEvent {
        session_id: String,
        event: agent::AgentEvent,
    },
    /// ask 中断（决策会话无法裁决 → UI 中断提问）
    Ask {
        question: String,
        options: Vec<String>,
    },
    /// phase 间停等确认
    ConfirmWait { phase: String },
    /// 终态收口（walker 收敛唯一出口）
    Finished {
        status: ChangeRunStatus,
        reason: Option<String>,
    },
}
