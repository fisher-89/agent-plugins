use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use agent::{AgentEvent, AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance};
use workflow::state::{RunFinishCommand, RunStartCommand};
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
    /// 模型等级
    pub model_level: ModelLevel,
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

/// test-execution 门禁产出载荷
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
    /// 修复边明细文本（fail / error 非空、pass 恒空串）
    pub findings_detail: String,
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

/// 归档链
pub trait ArchiveVcsPort: Send + Sync {
    /// 脏探测：`status --porcelain [-- pathspec…]` 非空即真（空 paths = 全域；
    /// gitignore 面不计——探测失败同 false，不阻断幂等跳过）。
    fn dirty(&self, root: &Path, paths: &[&str]) -> bool;
    /// worktree 全域提交（`add -A` + `commit -m`；worktree 即 change 私有执行
    /// 锚，全域 = 本 change 编辑集 + spec 同步产物）。
    fn commit_all(&self, worktree: &Path, message: &str) -> Result<(), String>;
    /// 祖先判定：`merge-base --is-ancestor <branch> HEAD`（退出 0/1 映射 bool，
    /// >1 Err——合入段幂等跳过的判定依据；提交段跳过依据只认干净探测）。
    fn branch_merged(&self, main_root: &Path, branch: &str) -> Result<bool, String>;
    /// worktree 内分支重放
    fn rebase_branch(&self, worktree: &Path, onto: &str) -> Result<RebaseOutcome, String>;
    /// 主仓快进：`merge --ff-only <branch>`（重放完成后的后半段——零 merge
    /// commit、不改写主仓历史；重放与快进间主仓前进的竞态 → Err 带重试引导）。
    fn ff_merge(&self, main_root: &Path, branch: &str) -> Result<(), String>;
    /// worktree 工作区快照
    fn worktree_snapshot(&self, root: &Path) -> Result<WorktreeSnapshot, String>;
    /// rebase 续走收口：`rebase --continue`（沿用重放提交既定信息；解冲突
    /// 后验通过后由归档链代收口——agent 无收口权）。多提交分支逐个重放：下一
    /// 个提交再冲突 → [`RebaseOutcome::Conflicted`]（调用方循环解算）；非冲突
    /// 失败 → `Err` 带 git 语境。
    fn rebase_continue(&self, worktree: &Path) -> Result<RebaseOutcome, String>;
    /// lean 收口面：`rebase --abort`（无法裁决时归档链尽力执行——worktree 恢
    /// 复重放前干净态；自身 Err 上抛由调用方附注呈现，不静默吞）。
    fn rebase_abort(&self, worktree: &Path) -> Result<(), String>;
    /// 主仓当前分支名（重放目标 = HEAD 所在分支；空输出 = detached HEAD Err）。
    fn current_branch(&self, main_root: &Path) -> Result<String, String>;
    /// pathspec 圈定提交：`add -A -- <paths…>` + `commit -m <msg> -- <paths 各自
    /// "/**" 形态>`（glob 覆盖已删除路径——裸目录 pathspec 对已删除目录报
    /// "did not match"；无关 staged / untracked 原样保留——pathspec 纪律）。
    fn commit_paths(&self, main_root: &Path, paths: &[&str], message: &str) -> Result<(), String>;
}

/// rebase 结果：冲突语义 = 冲突态保留、不 abort——收口归调用方。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseOutcome {
    /// 重放完成（分支已立于 onto 之上——等待主仓 ff-only 快进）。
    Rebased,
    /// 冲突（unmerged 清单非空；`-z` 归一路径；冲突态保留在 worktree）。
    Conflicted(Vec<String>),
}

/// `status --porcelain -z` 解析像（XY = index / worktree 状态码；untracked 为
/// "?","?"；rename 条目记新路径）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// index 侧状态码
    pub x: char,
    /// worktree 侧状态码（' ' = 工作区对 index 干净）
    pub y: char,
    /// 路径（rename / copy 条目记新路径）
    pub path: String,
}

/// `ls-files -s` 解析像（stage 0 = 正常条目；1/2/3 = unmerged 三方）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// 文件模式（如 `100644`）
    pub mode: String,
    /// blob sha
    pub hash: String,
    /// stage 号（unmerged 三方为 1/2/3）
    pub stage: u8,
    /// 路径
    pub path: String,
}

/// worktree 工作区快照（合入冲突后验的 A/B 对比基面，锚 = worktree）：A = 冲突
/// 即时、B = 解冲突收口前各取一次，由归档链纯函数逐字对比。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSnapshot {
    /// HEAD（A 时点 = 重放停止时的 HEAD——rebase 半程不前移分支引用）。
    pub head: String,
    /// rebase 目标分支或commit
    pub rebase_head: Option<String>,
    /// porcelain 状态条目（`-z` 归一）。
    pub status: Vec<StatusEntry>,
    /// 索引条目（`ls-files -s -z` 归一）。
    pub index: Vec<IndexEntry>,
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

/// run 运行史落库缝（unify-run-state-persistence）：walker 侧两写（起点第
/// 一写 running 行 / 终态出口第二写终态 + 步整包）的唯一通道。sync 签名零
/// tokio（walker 调用点均在 async 语境但写面同步——store 同步调用惯例）；
/// `Err` 串语义与既有 port 同型（`Display` 记因）。实现驻
/// `crate::run_history::StoreRunHistory`（组合根装配，core 零 infra 依赖）。
pub trait RunHistoryPort: Send + Sync {
    /// run 发起落行（running 行；失败 fail-fast 语义由调用方承载）。
    fn run_started(&self, command: &RunStartCommand) -> Result<(), String>;
    /// run 收口落包（终态 + 步整包 + active_phase 清位单事务）。
    fn run_finished(&self, command: &RunFinishCommand) -> Result<(), String>;
}
