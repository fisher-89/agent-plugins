use std::future::Future;
use std::pin::Pin;

use agent::{AgentEvent, AgentPermissionMode, AgentRunStatus, SessionProvenance};
use workflow::write::{BacktrackInput, PhaseLogInput, PhaseNextOutcome, PhaseStartOutcome};

use crate::state::RunUpdate;

/// WorkerAgent 轮 future 别名（std-only）。
pub type BoxTurnFuture = Pin<Box<dyn Future<Output = Result<WorkerTurnOutcome, String>> + Send>>;

/// ToolStep future 别名（std-only；失败统一 `Err` 串——显式失败停给用户）。
pub type BoxToolFuture = Pin<Box<dyn Future<Output = Result<ToolStepOutput, String>> + Send>>;

/// git diff 变更文件上下文 future 别名（std-only）。
pub type BoxDiffFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;

/// WorkerAgent 角色（source_ref 定式的第三段；D3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRole {
    Executor,
    Evaluator,
    Decision,
}

impl WorkerRole {
    /// sourceRef 角色段（线格式小写词）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Executor => "executor",
            Self::Evaluator => "evaluator",
            Self::Decision => "decision",
        }
    }
}

/// WorkerAgent 会话执行请求：一轮新会话（walker 恒 New——每步每会话独立，
/// static-check 反馈边经 `continue_session` 同会话续注）。
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerTurnRequest {
    /// workspace 根（会话 cwd）
    pub root: String,
    /// 组装好的轮 prompt
    pub prompt: String,
    /// 来源归属（source 恒 "change"，source_ref = `<change>/<phase>/<role>/<attempt>`）
    pub provenance: SessionProvenance,
    /// permission 档（run 恒 bypassPermissions）
    pub permission: AgentPermissionMode,
    /// 同会话续注（static-check 定向反馈边注入同一 executor 会话）
    pub continue_session: Option<String>,
    /// agent 实例 id（None 走默认解析）
    pub agent: Option<i64>,
    /// 角色（provenance sourceRef 组装面）
    pub role: WorkerRole,
}

/// WorkerAgent 会话产出：密封事件全集（提取器输入）+ 终态 + 最终消息载体
/// 所属会话 id。
#[derive(Debug, Clone)]
pub struct WorkerTurnOutcome {
    pub session_id: String,
    pub status: AgentRunStatus,
    pub final_message: Option<String>,
    pub transcript: Vec<AgentEvent>,
}

/// WorkerAgent 会话执行契约（假引擎缝）：infra 内核适配实现，测试以预录
/// 事件序列的假引擎直驱 walker。
pub trait WorkerAgentPort: Send + Sync {
    fn run(&self, turn: WorkerTurnRequest) -> BoxTurnFuture;
}

/// 工具步命令封闭集（相位机四步 + static-check 门禁；写通道唯一——相位机
/// 载荷直载写面输入类型，进程内直调 `workflow::write`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCommand {
    /// 相位路由（read-only；run_id 为会话窗口标识）
    PhaseNext { change: String, run_id: String },
    /// 开启阶段（attempt 计时）
    PhaseStart { change: String, phase: String },
    /// 评估落账（桌面代写）
    PhaseLog {
        change: String,
        phase: String,
        input: PhaseLogInput,
    },
    /// 回溯落账（白名单由 walker 缓存随行，写面二次校验兜底）
    Backtrack {
        change: String,
        phase: String,
        input: BacktrackInput,
    },
    /// 决策会话槽位挂账（该相位最新 eval 条目定点改写，幂等覆写）
    DecisionLog {
        change: String,
        phase: String,
        session_id: String,
    },
    /// static-check 门禁
    StaticCheck,
    /// test-execution 门禁（确定性测试执行链；change 定位报告目录与写面）
    TestExecution { change: String },
}

/// 工具步请求：workspace 根 + 步命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolStepRequest {
    /// workspace 根（写面 layout 解析与 spawn cwd）
    pub root: String,
    pub command: ToolCommand,
}

/// 工具步产出（封闭集，载荷 = 写面原生类型；PhaseNext 体量最大（白名单 +
/// prompt 双 Vec），Box 收敛变体尺寸差）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStepOutput {
    PhaseNext(Box<PhaseNextOutcome>),
    PhaseStart(PhaseStartOutcome),
    PhaseLog(workflow::write::PhaseLogOutcome),
    Backtrack(workflow::write::BacktrackOutcome),
    DecisionLog(workflow::write::DecisionLogOutcome),
    StaticCheck(StaticCheckOutcome),
    TestExecution(TestExecutionOutcome),
}

/// static-check 门禁产出（passed=false 走定向反馈边）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticCheckOutcome {
    pub passed: bool,
    pub diagnostics: String,
}

/// test-execution 门禁结论三值（checks 域聚合 `Conclusion` 的 port 映射像；
/// 线格式小写词，`WorkerRole` 先例同型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestExecutionConclusion {
    /// 全部 suite 通过且覆盖达阈
    Pass,
    /// 存在测试失败 / 覆盖或突变未达阈
    Fail,
    /// 存在执行错误（命令非零退出且结果不可解析 / 超时等）
    Error,
}

impl TestExecutionConclusion {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Error => "error",
        }
    }
}

/// test-execution 门禁产出最小载荷：conclusion + 四计数 + 诊断摘要 + 报告
/// 目录。全量 findings 留报告文件（修复会话经 `report_dir` 自读），本载荷
/// 只随 `RunUpdate::Step` detail 摘要与反馈边 prompt 流出——IPC 面零全量
/// 透传。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestExecutionOutcome {
    /// 聚合结论
    pub conclusion: TestExecutionConclusion,
    /// 用例总数
    pub total: u64,
    /// 通过数
    pub passed: u64,
    /// 失败数
    pub failed: u64,
    /// 跳过数
    pub skipped: u64,
    /// 诊断摘要（单条截断 + 至多 10 条，walker `diagnose_brief` 口径）
    pub findings_brief: String,
    /// 报告目录（change 报告树，全量 findings 所在）
    pub report_dir: String,
}

/// ToolStep 契约（进程内缝）：相位机四步由编排侧 [`crate::steps::LocalToolSteps`]
/// 直调写面，static-check 委托注入的 [`StaticCheckRunner`]。
pub trait ToolStepPort: Send + Sync {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture;
}

/// static-check spawn 缝（infra 实现；W4——static-check 非 agent 会话，独立
/// port 不并入 [`WorkerAgentPort`]）。产出契约与 [`ToolStepPort`] 同封闭集
///（`ToolStepOutput::StaticCheck`）。
pub trait StaticCheckRunner: Send + Sync {
    fn run(&self, root: &str) -> BoxToolFuture;
}

/// test-execution spawn 缝（checks 边界 infra 实现；W4 红线与
/// [`StaticCheckRunner`] 同型——检查域家族第二成员，spawn 不进 core）。产出
/// 契约同 `ToolStepOutput` 封闭集（`ToolStepOutput::TestExecution`）。
pub trait TestExecutionRunner: Send + Sync {
    fn run(&self, root: &str, change: &str) -> BoxToolFuture;
}

/// git diff 变更文件上下文缝（infra 实现；W5——git diff 是进程 spawn，落
/// infra）。产出为拼接好的上下文文本（porcelain 清单 + diff HEAD 补丁）。
pub trait DiffContextPort: Send + Sync {
    fn diff_context(&self, root: &str) -> BoxDiffFuture;
}

/// 只读快照契约：`ChangeDetail` 只读装配（决策输入与前置校验的输入面；读、
/// 写是两个关注点——写触点唯一经 `workflow::write` 写面）。
pub trait WorkflowSnapshotPort: Send + Sync {
    fn detail(&self, root: &str, change: &str) -> Result<workflow::queries::ChangeDetail, String>;
}

/// run 状态流出口：命令层桥接到 Channel 与 broadcast（walker 只认本缝）。
pub trait RunEventSink: Send + Sync {
    fn emit(&self, update: RunUpdate);
}
