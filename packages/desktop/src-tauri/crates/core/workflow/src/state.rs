use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::model::{ChecklistItem, Verdict};

/// change 状态二值（线格式小写词；queries DTO 直接复用本类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum ChangeStatus {
    /// 进行中（建档后默认态）
    Active,
    /// 已归档（写面 archive 翻转）
    Archived,
}

impl ChangeStatus {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeStatus::Active => "active",
            ChangeStatus::Archived => "archived",
        }
    }
}

/// 相位机步骤种类封闭集（八臂命令包络；线格式 snake_case 词）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    PhaseNext,
    PhaseStart,
    PhaseLog,
    Backtrack,
    DecisionLog,
    StaticCheck,
    TestExecution,
    MutationTest,
}

impl StepKind {
    /// 线格式 snake_case 词。
    pub fn as_str(self) -> &'static str {
        match self {
            StepKind::PhaseNext => "phase_next",
            StepKind::PhaseStart => "phase_start",
            StepKind::PhaseLog => "phase_log",
            StepKind::Backtrack => "backtrack",
            StepKind::DecisionLog => "decision_log",
            StepKind::StaticCheck => "static_check",
            StepKind::TestExecution => "test_execution",
            StepKind::MutationTest => "mutation_test",
        }
    }
}

/// 运行中 phase 状态（中性快照）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivePhaseState {
    pub phase: String,
    pub attempt: u32,
    /// 开相时刻（UTC unix 毫秒；写事务内铸出，恒在位）
    pub start_at: i64,
}

/// change 建档中性快照（`ChangeRecord` 的 port 流量像）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeStateRecord {
    /// 建档铸出的稳定唯一身份锚（UUID 形态；主键，一切寻址键——终身恒定、
    /// MUST NOT 复用）
    pub id: String,
    /// change 名（恒裸名；可变属性，无唯一约束，MUST NOT 作身份键）
    pub name: String,
    /// 人类可读标题，恒非空（创建与升级默认 = `name`，promote 路径继承
    /// `explore.title`）
    pub title: String,
    pub workflow_type: String,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    pub status: ChangeStatus,
    /// 归档时间（UTC unix 毫秒；active 恒 None）
    pub archived_at: Option<i64>,
    /// 运行中 phase（开相在位、落账清位）
    pub active_phase: Option<ActivePhaseState>,
    /// 该 change 分配的 worktree 绝对路径（执行锚）；`None` = legacy 主 root
    /// change。**执行锚引用**，MUST NOT 反向参与库身份派生（`for_root` 恒以
    /// workspace root 为锚——双 root 不变量）。
    pub worktree: Option<String>,
    /// 创建基线 fork 点（主仓 HEAD，建域时铸出）；调试 / UI 价值。
    pub base_commit: Option<String>,
}

/// 相位评估条目中性快照（`PhaseRecord` 行 + checklist 子行内联重组的 port
/// 流量像；checklist 已按打包键序 = evaluator 输出序）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PhaseStateRecord {
    /// 条目 id（写事务内 max+1 分配）
    pub id: i64,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    pub phase: String,
    pub attempt: u32,
    pub verdict: Verdict,
    /// 评估报告（内联 ≤2000 字，写面校验）
    pub report: String,
    /// checklist（子表重组，打包键序）
    pub checklist: Vec<ChecklistItem>,
    pub skipped: bool,
    pub stale: bool,
    pub backtrack_to: Option<String>,
    pub backtrack_reason: Option<String>,
    /// 会话槽位三列（缺省落账恒 None）
    pub executor_session_id: Option<String>,
    pub evaluator_session_id: Option<String>,
    pub decision_session_id: Option<String>,
    /// 开相时刻（UTC unix 毫秒；缺省 None）
    pub start_at: Option<i64>,
    /// 落账时刻（UTC unix 毫秒）
    pub timestamp: i64,
}

/// 步骤审计行中性快照（`StepRecord` 的 port 流量像；审计 only，不做 run 恢复
/// 依据）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StepStateRecord {
    /// 行 id（写事务内 max+1 分配）
    pub id: i64,
    /// 所属 run（`run-<millis>` 铸造标识，同 run 步骤串链键）
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    pub step_kind: StepKind,
    /// 步终态（线格式词：ok | error）
    pub status: String,
    /// 落行时刻（UTC unix 毫秒）
    pub timestamp: i64,
    /// 有界输出摘要（≤500 字截断留痕，design D10）
    pub summary: String,
    /// 全量输出引用（checks 报告目录 / 会话 id）
    pub reference: Option<String>,
}

/// 开相产出（attempt 写事务内推导）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PhaseStartState {
    pub attempt: u32,
    /// 开相时刻（UTC unix 毫秒，即调用方传入的 `now`）
    pub start_at: i64,
}

/// 落账写命令（`phase_log` 持久化载荷）：verdict / report 长度 / 表位 /
/// active_phase 匹配校验全在写面前置，本命令只携通过校验的落库载荷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLogCommand {
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    pub phase: String,
    pub verdict: Verdict,
    pub report: String,
    pub skipped: bool,
    pub checklist: Vec<ChecklistItem>,
    /// 会话槽位三列（缺省 None，显式在位纪律沿磁盘模型先例）
    pub executor_session_id: Option<String>,
    pub evaluator_session_id: Option<String>,
    pub decision_session_id: Option<String>,
    /// 开相时刻（来自 active_phase，随行落条目）
    pub start_at: Option<i64>,
    /// 落账时刻（UTC unix 毫秒，写面铸出）
    pub timestamp: i64,
}

/// 回跳写命令（`backtrack` 持久化载荷）：白名单 / 双端表位 / reason 长度
/// 校验全在写面前置；stale 闭包由 core 计算随行（store 只按相位名全条目置
/// stale，不自持依赖表）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacktrackCommand {
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    /// 回跳发起相位（最新条目落 backtrack_to / backtrack_reason 标记）
    pub phase: String,
    /// 回跳目标相位（其最新 pass 条目置 stale）
    pub to: String,
    pub reason: String,
    /// stale 传播闭包（`phase_table::dependents` BFS 全量，不含目标自身）
    pub stale_dependents: Vec<String>,
}

/// 步骤审计写命令（`StepCommand`）：编排八臂命令包络逐条落行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepCommand {
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    pub step_kind: StepKind,
    /// 步终态（线格式词：ok | error）
    pub status: String,
    /// 有界输出摘要（调用方截断至 ≤500 字）
    pub summary: String,
    /// 全量输出引用（checks 报告目录 / 会话 id）
    pub reference: Option<String>,
    /// 落行时刻（UTC unix 毫秒）
    pub timestamp: i64,
}

// --- 计划图（源码单源：由 `write::phase_table` 的 const 表声明，不落库、不经
// port 缝、无运行时文件 IO / 目录遍历；prompt 仍全静态零占位）--------------

/// 计划步分组四分类（`StepNode::group()` 派生；walker 按本值分派 exec / agent /
/// git / gate 步执行，MUST NOT 以 step 类型特判实现）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum StepGroup {
    /// 脚本步（相位机步 phase-start / phase-log + 检查域 spawn 步 + 归档内联步
    /// preflight / seal）
    Exec,
    /// agent 会话步（executor / evaluator / decision / spec-sync /
    /// resolve-conflicts）
    Agent,
    /// vcs 步（commit / rebase / merge / cleanup / finalize——独立 vcs 缝进程
    /// spawn）
    Git,
    /// 纯 Rust 分支门（verdict / retry / whitelist 三门）
    Gate,
}

/// agent 步角色（`StepNode::role()` 派生；线格式 snake_case 词与 `WorkerRole`
/// 同词面——归档步两会话的 source_ref 定式第三段直用本词）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// 实现 / 修复会话
    Executor,
    /// 评估会话
    Evaluator,
    /// 决策会话（重试上限分叉）
    Decision,
    /// spec 同步会话（归档相位 SpecSync 步）
    SpecSync,
    /// 合入冲突解算会话（归档相位 ResolveConflicts 步）
    ResolveConflicts,
}

/// 计划步节点全词汇（20 值封闭集，线格式 snake_case 词）：agent 阶段三角色、
/// 脚本阶段三检查步（static-check / test-execution / mutation-test）、流程面两步、
/// 三门 gate 与归档 9 步。词汇本体单点——计划图声明（[`StepDefinition::node`]）
/// 与执行图落库（[`RunStepKind`]）同词面 1:1 全映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum StepNode {
    /// executor 会话
    Executor,
    /// evaluator 会话
    Evaluator,
    /// 决策会话
    Decision,
    /// phase-start 相位机步
    PhaseStart,
    /// static-check 门禁工具步（定向反馈边）
    StaticCheck,
    /// test-execution 门禁工具步（确定性测试执行链）
    TestExecution,
    /// mutation-test 门禁工具步（确定性突变执行链，独立反馈边②）
    MutationTest,
    /// phase-log 相位机步
    PhaseLog,
    /// verdict 解析门
    VerdictGate,
    /// retry 预算门
    RetryGate,
    /// backtrack 白名单门
    WhitelistGate,
    /// 归档前置只读校验步（store 读 + vcs 探测；内联步）
    Preflight,
    /// 归档 worktree 提交步（vcs.commit_all；干净则跳过）
    Commit,
    /// 归档分支重放步（vcs.rebase_branch；冲突清单入跨步产物）
    Rebase,
    /// 归档合入冲突解算步（条件执行：无冲突 passed；有冲突 agent 解算）
    ResolveConflicts,
    /// 归档主仓快进步（vcs.ff_merge）
    Merge,
    /// 归档 delta specs 同步步（agent 会话）
    SpecSync,
    /// 归档 worktree 清理步（vcs.remove_worktree 尽力步）
    Cleanup,
    /// 归档双写收口步（写面 `archive`；内联步）
    Seal,
    /// 归档落盘提交步（vcs.commit_paths；已落盘则跳过）
    Finalize,
}

impl StepNode {
    /// 线格式 snake_case 词（与 [`RunStepKind::as_str`] 逐字同源）。
    pub fn as_str(self) -> &'static str {
        match self {
            StepNode::Executor => "executor",
            StepNode::Evaluator => "evaluator",
            StepNode::Decision => "decision",
            StepNode::PhaseStart => "phase_start",
            StepNode::StaticCheck => "static_check",
            StepNode::TestExecution => "test_execution",
            StepNode::MutationTest => "mutation_test",
            StepNode::PhaseLog => "phase_log",
            StepNode::VerdictGate => "verdict_gate",
            StepNode::RetryGate => "retry_gate",
            StepNode::WhitelistGate => "whitelist_gate",
            StepNode::Preflight => "preflight",
            StepNode::Commit => "commit",
            StepNode::Rebase => "rebase",
            StepNode::ResolveConflicts => "resolve_conflicts",
            StepNode::Merge => "merge",
            StepNode::SpecSync => "spec_sync",
            StepNode::Cleanup => "cleanup",
            StepNode::Seal => "seal",
            StepNode::Finalize => "finalize",
        }
    }

    /// 四类节点分组（派生访问器；MUST NOT 落独立冗余字段）。归档步归组：git 步
    ///（commit / rebase / merge / cleanup / finalize）→ Git、内联步（preflight /
    /// seal）→ Exec、agent 步（spec_sync / resolve_conflicts）→ Agent。
    pub fn group(self) -> StepGroup {
        match self {
            StepNode::Executor
            | StepNode::Evaluator
            | StepNode::Decision
            | StepNode::SpecSync
            | StepNode::ResolveConflicts => StepGroup::Agent,
            StepNode::PhaseStart
            | StepNode::StaticCheck
            | StepNode::TestExecution
            | StepNode::MutationTest
            | StepNode::PhaseLog
            | StepNode::Preflight
            | StepNode::Seal => StepGroup::Exec,
            StepNode::Commit
            | StepNode::Rebase
            | StepNode::Merge
            | StepNode::Cleanup
            | StepNode::Finalize => {
                StepGroup::Git
            }
            StepNode::VerdictGate | StepNode::RetryGate | StepNode::WhitelistGate => {
                StepGroup::Gate
            }
        }
    }

    /// agent 步角色（非 agent 步 `None`）。
    pub fn role(self) -> Option<AgentRole> {
        match self {
            StepNode::Executor => Some(AgentRole::Executor),
            StepNode::Evaluator => Some(AgentRole::Evaluator),
            StepNode::Decision => Some(AgentRole::Decision),
            StepNode::SpecSync => Some(AgentRole::SpecSync),
            StepNode::ResolveConflicts => Some(AgentRole::ResolveConflicts),
            StepNode::PhaseStart
            | StepNode::StaticCheck
            | StepNode::TestExecution
            | StepNode::MutationTest
            | StepNode::PhaseLog
            | StepNode::Preflight
            | StepNode::Commit
            | StepNode::Rebase
            | StepNode::Merge
            | StepNode::Cleanup
            | StepNode::Seal
            | StepNode::Finalize
            | StepNode::VerdictGate
            | StepNode::RetryGate
            | StepNode::WhitelistGate => None,
        }
    }
}

/// 步失败语义（`StepDefinition.on_fail`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum OnFail {
    /// 定向反馈边
    FeedbackLoop {
        /// 反馈边独立计数上限
        limit: u32,
    },
    /// 终态失败停给用户。现仅 code-analyze 相位 VerdictGate 声明（spec 定稿：
    /// 分析相位 verdict 失败即终态）；其余 verdict 解析失败自 feedback 边改造后
    /// 不再走终态。语义与缺省 `None` 等价。
    Terminal,
}

/// 计划步定义（相位内有序序列项；全静态 const 表内联）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepDefinition {
    /// 步节点词汇（kind / role 经 [`StepNode::group`] / [`StepNode::role`] 派生）
    pub node: StepNode,
    /// agent 步静态 prompt 引用（`include_str!` 编译期静态文本；非 agent 步 `None`）
    pub prompt_ref: Option<&'static str>,
    /// 步失败语义（缺省 `None` = 按步失败收场）
    pub on_fail: Option<OnFail>,
}

/// 产物清单项（相对路径 → phase 归属的权威来源；产物发现按本清单枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactSpec {
    /// 契约 ID（`markdown-doc` / `tasks-progress` 等）
    pub kind: &'static str,
    /// 相对路径模式（精确路径，或 `<前缀>/**` 前缀 glob）
    pub path: &'static str,
    /// 清单声明的人读标题（插件无名单标题时的兜底）
    pub title: &'static str,
}

// --- run 执行图（unify-run-state-persistence：决策翻案「不建 flow_runs 表」
// 立项；run-workflow-by-step 决策③重设计为 RunRecord / RunNodeRecord /
// RunEdgeRecord 三表——节点承载全词汇 + 稳定身份键，边单独落表；词汇本体单点
// 驻本节，写命令与查询投影同类型消费）--------------------------------------

/// run 状态五值（线格式小写词；queries DTO 直接复用本类型）。`interrupted`
/// 仅启动标定产生（重启后 run 客观已死），运行期写路径不产生该值；停等两态
/// （waitingAsk / waitingConfirm）不落库——应答通道活在进程内。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    /// 运行中（发起建档起步）
    Running,
    /// 全相位 pass 走完
    Completed,
    /// 受控终止（用户停止 / confirm=false / 决策 stop 动作）
    Stopped,
    /// 失败终止
    Failed,
    /// 中断（仅启动标定产生）
    Interrupted,
}

impl RunStatus {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Stopped => "stopped",
            RunStatus::Failed => "failed",
            RunStatus::Interrupted => "interrupted",
        }
    }

    /// 终态判别（收口三值 + interrupted 标定值；running 非终态）。
    pub fn is_terminal(self) -> bool {
        !matches!(self, RunStatus::Running)
    }
}

/// run 步词汇封闭集（**二十值全词汇**，线格式 snake_case 词）：agent 阶段
///（executor / evaluator / decision）、脚本阶段（static_check / test_execution /
/// mutation_test）、流程面步骤（phase_start / phase_log）、三门 gate（verdict_gate、
/// retry_gate、whitelist_gate）与归档 9 步（preflight / commit / rebase /
/// resolve_conflicts / merge / spec_sync / cleanup / seal / finalize）。词汇本体单点——落库命令
///（`RunNodeEntry.step`）与查询投影（`detail.rs`）同本类型消费，落库零过滤
///（旧 5 值词表的过滤谓词已退役）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunStepKind {
    /// executor 会话
    Executor,
    /// evaluator 会话
    Evaluator,
    /// 决策会话
    Decision,
    /// static-check 工具步
    StaticCheck,
    /// test-execution 门禁工具步
    TestExecution,
    /// mutation-test 门禁工具步
    MutationTest,
    /// phase-start 相位机步
    PhaseStart,
    /// phase-log 相位机步
    PhaseLog,
    /// verdict 解析门
    VerdictGate,
    /// retry 预算门
    RetryGate,
    /// backtrack 白名单门
    WhitelistGate,
    /// 归档前置只读校验步（内联步）
    Preflight,
    /// 归档 worktree 提交步（vcs）
    Commit,
    /// 归档分支重放步（vcs）
    Rebase,
    /// 归档合入冲突解算步（agent 会话；条件执行）
    ResolveConflicts,
    /// 归档主仓快进步（vcs）
    Merge,
    /// 归档 delta specs 同步步（agent 会话）
    SpecSync,
    /// 归档 worktree 清理步（vcs.remove_worktree 尽力步）
    Cleanup,
    /// 归档双写收口步（内联步）
    Seal,
    /// 归档落盘提交步（vcs）
    Finalize,
}

impl RunStepKind {
    /// 线格式 snake_case 词。
    pub fn as_str(self) -> &'static str {
        match self {
            RunStepKind::Executor => "executor",
            RunStepKind::Evaluator => "evaluator",
            RunStepKind::Decision => "decision",
            RunStepKind::StaticCheck => "static_check",
            RunStepKind::TestExecution => "test_execution",
            RunStepKind::MutationTest => "mutation_test",
            RunStepKind::PhaseStart => "phase_start",
            RunStepKind::PhaseLog => "phase_log",
            RunStepKind::VerdictGate => "verdict_gate",
            RunStepKind::RetryGate => "retry_gate",
            RunStepKind::WhitelistGate => "whitelist_gate",
            RunStepKind::Preflight => "preflight",
            RunStepKind::Commit => "commit",
            RunStepKind::Rebase => "rebase",
            RunStepKind::ResolveConflicts => "resolve_conflicts",
            RunStepKind::Merge => "merge",
            RunStepKind::SpecSync => "spec_sync",
            RunStepKind::Cleanup => "cleanup",
            RunStepKind::Seal => "seal",
            RunStepKind::Finalize => "finalize",
        }
    }
}

/// run 执行图边词汇封闭集（四值，线格式小写词）：`forward` 计划序推进 /
/// `feedback` gate 失败回同 phase agent 步（phase 内回边）/ `retry` phase fail
/// 后同相位重跑 / `backtrack` 跨相位回跳。词汇本体单点——编排侧 `ChangeStepEdge`
/// 与落库命令（`RunEdgeEntry.edge_kind`）同本类型消费（零映射）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunEdgeKind {
    /// 计划序推进（相位内按 steps 序推进、相位间推进）
    Forward,
    /// gate 失败回到同 phase agent 步（phase 内回边）
    Feedback,
    /// phase fail 后同相位重跑
    Retry,
    /// 跨相位回跳
    Backtrack,
}

impl RunEdgeKind {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            RunEdgeKind::Forward => "forward",
            RunEdgeKind::Feedback => "feedback",
            RunEdgeKind::Retry => "retry",
            RunEdgeKind::Backtrack => "backtrack",
        }
    }
}

/// run 步状态四值（线格式 camelCase 词）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RunStepStatus {
    /// 步进行中（收口在途步可留 running）
    Running,
    /// 步通过
    Passed,
    /// 步失败
    Failed,
    /// 步因停止 / 终止收敛
    Stopped,
}

/// run 运行史主行中性快照（`RunRecord` 的 port 流量像）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunStateRecord {
    /// run id（walker `run-<millis>` 铸造标识，主键）
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    pub status: RunStatus,
    /// 终态记因 / 标定记因（running 恒 None）
    pub reason: Option<String>,
    /// 发起时刻（UTC unix 毫秒）
    pub started_at: i64,
    /// 收口 / 标定时刻（UTC unix 毫秒；running 恒 None）
    pub finished_at: Option<i64>,
}

/// run 执行图节点史行中性快照（`RunNodeRecord` 的 port 流量像；图史面，与
/// `StepRecord` 审计职责分立）。
///
/// 稳定身份键 = `(run_id, phase, attempt, step, instance_no)`（`instance_no` 每
/// 次执行递增，feedback 重跑为独立新节点）；身份串
/// `"{phase}:{attempt}:{step}:{instance_no}"` 为边表端点口径（store 侧
/// `RunNodeRecord::node_id()` 同式组装）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunNodeStateRecord {
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    pub phase: String,
    pub attempt: u32,
    pub step: RunStepKind,
    /// 同 `(phase, attempt, step)` 内的执行实例号（每次执行递增）
    pub instance_no: u32,
    pub status: RunStepStatus,
    /// WorkerAgent 步所属会话 id（工具步为 None）
    pub session_id: Option<String>,
    /// 人读记因 / 摘要（有界，写面截断同 diagnose_brief 口径）
    pub detail: Option<String>,
    /// 全量输出引用（`ExecOutput::Recorded { exec_id }`；无记录模式为 None）
    pub output_ref: Option<String>,
    /// 步开始时刻（UTC unix 毫秒；旧库行缺省 0 = 步级时间缺席）
    pub started_at: i64,
    /// 步收口时刻（UTC unix 毫秒；running 收口在途步 None）
    pub ended_at: Option<i64>,
    /// 本步 turn token 增量（`TurnDone.usage` 求和；仅 WorkerAgent 步终态有值）
    pub token_input: Option<u64>,
    /// 本步 turn token 增量（输出向；口径同 `token_input`）
    pub token_output: Option<u64>,
    /// 落包时刻（UTC unix 毫秒，= finish 的 finished_at）
    pub timestamp: i64,
}

/// run 执行图边史行中性快照（`RunEdgeRecord` 的 port 流量像）：`edge_kind` 四值
/// 封闭集，`from_node_key` / `to_node_key` 指向节点身份串。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunEdgeStateRecord {
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    /// 源节点身份串
    pub from_node_key: String,
    /// 目标节点身份串
    pub to_node_key: String,
    pub edge_kind: RunEdgeKind,
    /// 落包时刻（UTC unix 毫秒，= finish 的 finished_at）
    pub timestamp: i64,
}

/// run 发起写命令（walker 起点第一写载荷）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStartCommand {
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    /// 发起时刻（UTC unix 毫秒，命令层发起时铸造，corpus 确定性）
    pub started_at: i64,
}

/// run 节点整包条目（finish 载荷内联；词汇 = [`RunStepKind`] 二十值全词汇，零
/// 过滤）。`node_seq` 由整包下标盖戳（store 侧组装打包主键）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunNodeEntry {
    pub phase: String,
    pub attempt: u32,
    pub step: RunStepKind,
    /// 同 `(phase, attempt, step)` 内的执行实例号
    pub instance_no: u32,
    pub status: RunStepStatus,
    pub session_id: Option<String>,
    pub detail: Option<String>,
    /// 全量输出引用（`exec_id`）
    pub output_ref: Option<String>,
    /// 步开始时刻（UTC unix 毫秒）
    pub started_at: i64,
    /// 步收口时刻（UTC unix 毫秒；running 收口在途步 None）
    pub ended_at: Option<i64>,
    /// 本步 turn token 增量（`TurnDone.usage` 求和；仅 WorkerAgent 步终态有值）
    pub token_input: Option<u64>,
    /// 本步 turn token 增量（输出向；口径同 `token_input`）
    pub token_output: Option<u64>,
}

/// run 边整包条目（finish 载荷内联）：端点恒为节点身份串。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEdgeEntry {
    pub from_node_key: String,
    pub to_node_key: String,
    pub edge_kind: RunEdgeKind,
}

/// run 收口写命令（终态更新 + 节点整包 + 边整包 + active_phase 清位单事务载
/// 荷）。`status` 为终态三值（completed / stopped / failed），`interrupted` 写面
/// 拒绝——运行期写路径不产生，仅启动标定写入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFinishCommand {
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    pub status: RunStatus,
    pub reason: Option<String>,
    /// 收口时刻（UTC unix 毫秒，命令携带；corpus 确定性）
    pub finished_at: i64,
    /// 节点整包（全词汇零过滤，序 = emit 序）
    pub nodes: Vec<RunNodeEntry>,
    /// 边整包（四 kind，序 = emit 序）
    pub edges: Vec<RunEdgeEntry>,
}

/// port 错误面（`StoreFault`）：读 / 写半边统一错误收敛；命令层以
/// `.to_string()` 呈现，`Display` 恒带语境前缀。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreFault {
    /// db 打开 / 事务 / 读写失败
    Db(String),
    /// 主键冲突
    Conflict(String),
    /// 目标记录不存在（miss 非幂等写面）
    NotFound(String),
}

impl fmt::Display for StoreFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreFault::Db(msg) => write!(f, "db: {msg}"),
            StoreFault::Conflict(msg) => write!(f, "conflict: {msg}"),
            StoreFault::NotFound(msg) => write!(f, "not_found: {msg}"),
        }
    }
}

impl std::error::Error for StoreFault {}

/// 落库 port 缝（读半边 + 写半边）：core/workflow 对 change 状态载体的唯一
/// 依赖面。实现驻 infra/store（`impl ChangeStateStore for Store`，组合根装
/// 配）；测试以进程内假件实现本 trait（写面 / 编排测试既有 fake port 先例）。
pub trait ChangeStateStore: Send + Sync {
    // --- 读半边 -----------------------------------------------------------

    /// 建档单查（主键 id 直查；`None` = 未建档——不再有文档形态语义）。
    fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault>;

    /// 建档全量（列表单源全量；主键 id 自然序）。
    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault>;

    /// 相位评估史（按 change id 归属过滤，落行序 = id 升序；checklist 已按
    /// 打包键序内联）。
    fn list_phase_records(&self, change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreFault>;

    /// 步骤审计枚举（按 change id 归属过滤，时间序 = id 升序；run 可选圈定）。
    fn list_steps(
        &self,
        change_id: &str,
        run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault>;

    // --- 写半边 -----------------------------------------------------------

    /// 建档（同 id 记录已存在 → [`StoreFault::Conflict`]——同 id 防御拒绝，
    /// 零同名检查：同名 active 拒绝归写面 create 前置扫描；写面 create 的建
    /// 档半边，status=active 起步）。
    fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault>;

    /// 补偿删除（create 双写 fs 半边失败的回滚面，design D5：只删本次自插
    /// 行；miss 幂等 `Ok(false)`）。
    fn delete_change_record(&self, id: &str) -> Result<bool, StoreFault>;

    /// 写 active_phase（按 change id 定位记录；attempt 写事务内推导 = 该相位
    /// 既有条目数 + 1）；miss → [`StoreFault::NotFound`]。
    fn start_phase(
        &self,
        change_id: &str,
        phase: &str,
        now: i64,
    ) -> Result<PhaseStartState, StoreFault>;

    /// 落账单事务原子（PhaseRecord 行 + checklist 子行 + active_phase 清位 +
    /// `(change_id, phase, attempt)` 查重），返回事务内推导的 attempt。
    fn log_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreFault>;

    /// 回跳单事务（最新条目回跳标记 + 目标最新 pass 置 stale + 闭包全条目置
    /// stale）；发起相位无条目 → [`StoreFault::NotFound`]。
    fn apply_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreFault>;

    /// decision 槽位幂等挂账（按 change id 定位该相位最新条目定点改写；无条目
    /// → [`StoreFault::NotFound`]）。
    fn amend_decision_session(
        &self,
        change_id: &str,
        phase: &str,
        session_id: &str,
    ) -> Result<(), StoreFault>;

    /// status 翻转（归档 db 半边；主键 id 与 name 均不变；miss →
    /// [`StoreFault::NotFound`]）。
    fn set_archived(&self, id: &str, archived_at: i64) -> Result<(), StoreFault>;

    /// 步骤审计行追加（行 id 写事务内 max+1）。
    fn append_step(&self, command: &StepCommand) -> Result<(), StoreFault>;

    // --- run 执行图（unify-run-state-persistence 决策翻案 + run-workflow-by-step
    // 决策③：run 记录落 RunRecord / RunNodeRecord / RunEdgeRecord 三表；每 run
    // 两写零每步写放大）---------------------------------------------------

    /// run 运行史清单（按 change id 归属过滤，`started_at` 升序；run_id 并列
    /// 稳定序由实现保证）。
    fn list_runs(&self, change_id: &str) -> Result<Vec<RunStateRecord>, StoreFault>;

    /// run 执行图节点行（`run_id` 圈定，node_seq 升序 = emit 序；二十值全词汇
    /// 零过滤——读面与写面同一份封闭集）。
    fn list_run_nodes(&self, run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreFault>;

    /// run 执行图边行（`run_id` 圈定，edge_seq 升序 = emit 序；四 kind 零过滤）。
    fn list_run_edges(&self, run_id: &str) -> Result<Vec<RunEdgeStateRecord>, StoreFault>;

    /// run 发起落行（status=running；同 run_id 冲突 → [`StoreFault::Conflict`]，
    /// change 未建档 → [`StoreFault::NotFound`]）。
    fn run_start(&self, command: &RunStartCommand) -> Result<(), StoreFault>;

    /// run 收口单事务（run 行在案且 running → 终态 + reason + finished_at +
    /// 节点整包 + 边整包 + 该 change `active_phase` 按 change id 清位；miss →
    /// [`StoreFault::NotFound`]，非 running 或 status 含 `interrupted` →
    /// [`StoreFault::Conflict`]）。
    fn run_finish(&self, command: &RunFinishCommand) -> Result<(), StoreFault>;
}
