//! run 状态与步节点 / 边类型：`RunUpdate` Channel 信封（tag `ipc`）、运行状态机
//! 词汇、步节点状态行、执行图边（端点 = 节点身份键）与快照 / 摘要 DTO。serde
//! camelCase + specta `Type`——IPC 直用，前端 bindings 再生成直出线格式（与
//! `AgentRunMessage` 同式先例）。

use serde::{Deserialize, Serialize};
use specta::Type;

/// run 执行图边 kind 四值（线格式小写词：forward / feedback / retry /
/// backtrack）：词汇本体单点驻 `workflow::state::RunEdgeKind`（与落库命令
/// `RunEdgeEntry.edge_kind`、出线 `RunEdgeStateRecord.edge_kind` 同一份封闭集
/// ——零映射零漂移）。
pub use workflow::state::RunEdgeKind as EdgeKind;

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
    /// 终局判别：终态 run 自注册表除名、订阅释放（run 运行史已落库
    /// unify-run-state-persistence——图派生源 = 库读史 ∪ 注册表在飞 run，
    /// 重启后库史可达，本判别仅承载订阅释放与除名时点）。
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Stopped | Self::Failed)
    }
}

/// 步词汇（三类节点可辨）：WorkerAgent 三角色 + 相位机 / 工具步三步 + Gate
/// 三门。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
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

impl ChangeStepKind {
    /// 线格式 snake_case 词（节点身份串口径——与落库侧 `RunStepKind::as_str()`
    /// 同词面，边表端点跨两侧逐字可比）。
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeStepKind::Executor => "executor",
            ChangeStepKind::Evaluator => "evaluator",
            ChangeStepKind::Decision => "decision",
            ChangeStepKind::PhaseStart => "phase_start",
            ChangeStepKind::StaticCheck => "static_check",
            ChangeStepKind::TestExecution => "test_execution",
            ChangeStepKind::PhaseLog => "phase_log",
            ChangeStepKind::VerdictGate => "verdict_gate",
            ChangeStepKind::RetryGate => "retry_gate",
            ChangeStepKind::WhitelistGate => "whitelist_gate",
        }
    }
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

/// 步状态行：一次 `RunUpdate::Step` 的载荷，运行步节点推导的唯一输入面。稳定
/// 身份键 = `(phase, attempt, step, instance_no)`——同 `(phase, attempt, step)` 的
/// 多次执行（feedback 重跑）以 `instance_no` 递增区分，前端直读直渲染。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeStepState {
    /// 相位 id
    pub phase: String,
    /// attempt 号
    pub attempt: u32,
    /// 步词汇
    pub step: ChangeStepKind,
    /// 同 `(phase, attempt, step)` 内的执行实例号（每次执行递增）
    pub instance_no: u32,
    /// 步状态
    pub status: ChangeStepStatus,
    /// WorkerAgent 步所属会话 id（工具步 / Gate 步为 None）
    pub session_id: Option<String>,
    /// 人读详情（失败记因 / 通过摘要）
    pub detail: Option<String>,
    /// 全量输出引用（`ExecOutput::Recorded { exec_id }`；Transient 恒 None）
    pub output_ref: Option<String>,
}

/// 执行图节点身份键：`(phase, attempt, step, instance_no)`。
pub type NodeKey = (String, u32, ChangeStepKind, u32);

/// 执行图边：端点恒为节点身份键，`kind` 四值封闭集（[`EdgeKind`]）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeStepEdge {
    /// 源节点身份键
    pub from: NodeKey,
    /// 目标节点身份键
    pub to: NodeKey,
    /// 边 kind（四值）
    pub kind: EdgeKind,
}

impl ChangeStepEdge {
    /// 源节点身份串（边表端点口径，与 store `RunNodeRecord::node_id()` 同式）。
    pub fn from_node_key(&self) -> String {
        node_id(&self.from)
    }

    /// 目标节点身份串。
    pub fn to_node_key(&self) -> String {
        node_id(&self.to)
    }
}

/// 节点身份串组装单点：`"{phase}:{attempt}:{step}:{instance_no}"`（step 为
/// snake_case 线词，与落库侧 `RunStepKind::as_str()` 同词面）。
pub fn node_id(key: &NodeKey) -> String {
    format!("{}:{}:{}:{}", key.0, key.1, key.2.as_str(), key.3)
}

/// ask 载荷：决策会话 ask 动作的中断问题与候选选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AskPayload {
    pub question: String,
    pub options: Vec<String>,
}

/// 重挂快照：进程内 run 控制注册表在 view 重建时的状态恢复面（run 终态后
/// 为 None——快照只覆盖运行期，收口后常驻渲染由库读史承载）。合并查询活面
/// 消费（`ActiveRunView` 投影源，unify-run-state-persistence D11）。
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
    /// 发起时刻（UTC unix 毫秒；与 RunRequest.started_at 同值入表）
    pub started_at: i64,
    /// 节点累积器快照（全词汇 emit 序，稳定身份键；重挂恢复节点表不再恒空）
    pub nodes: Vec<ChangeStepState>,
    /// 边累积器快照（四 kind emit 序；重挂恢复边表不再恒空）
    pub edges: Vec<ChangeStepEdge>,
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
/// 步状态与执行图边上图、会话事件透传转录面板、ask / 确认停等驱动控制面
/// 板、终态收口。
/// 进程内 seam 类型（worker sink / ArchiveSink 载荷消费零改动——归档链零触
/// 点红线）；IPC 面经 [`RunNotice`] kind-only 降位（D3），载荷不出进程。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "ipc", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RunUpdate {
    /// 步状态变更（运行步节点上图输入）
    Step { step: ChangeStepState },
    /// 执行图边（边表累积输入；四 kind 显式落边）
    Edge { edge: ChangeStepEdge },
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

impl RunUpdate {
    /// kind 判别词（通知投影单点：`RunNotice` 六变体与 `RunUpdate` 六变体
    /// 一一同型对应）。
    pub fn kind(&self) -> &'static str {
        match self {
            RunUpdate::Step { .. } => "step",
            RunUpdate::Edge { .. } => "edge",
            RunUpdate::SessionEvent { .. } => "sessionEvent",
            RunUpdate::Ask { .. } => "ask",
            RunUpdate::ConfirmWait { .. } => "confirmWait",
            RunUpdate::Finished { .. } => "finished",
        }
    }
}

/// 变更通知（唯一 IPC 信封；unify-run-state-persistence D3 通知降位）：六
/// kind-only 变体与 [`RunUpdate`] 一一同型、零载荷——通知仅失效信号，查询
/// 结果权威，客户端收通知自行重查统一视图（`get_change_detail`）与转录库。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "ipc", rename_all = "camelCase")]
pub enum RunNotice {
    /// 步状态变更（→ 统一视图重查）
    Step,
    /// 执行图边变更（→ 统一视图重查）
    Edge,
    /// WorkerAgent 会话事件透传（→ 转录库重查）
    SessionEvent,
    /// ask 中断（→ 统一视图重查）
    Ask,
    /// phase 间停等确认（→ 统一视图重查）
    ConfirmWait,
    /// 终态收口（→ 统一视图重查定局）
    Finished,
}

impl From<&RunUpdate> for RunNotice {
    /// kind 投影（载荷剥离单点：`ChangeFlowControl::publish` 广播侧消费）。
    fn from(update: &RunUpdate) -> Self {
        match update {
            RunUpdate::Step { .. } => RunNotice::Step,
            RunUpdate::Edge { .. } => RunNotice::Edge,
            RunUpdate::SessionEvent { .. } => RunNotice::SessionEvent,
            RunUpdate::Ask { .. } => RunNotice::Ask,
            RunUpdate::ConfirmWait { .. } => RunNotice::ConfirmWait,
            RunUpdate::Finished { .. } => RunNotice::Finished,
        }
    }
}
