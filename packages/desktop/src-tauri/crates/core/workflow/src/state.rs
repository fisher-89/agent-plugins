//! change 流程状态缝：port trait + 中性状态类型。
//!
//! core/workflow 自持中性状态词汇（记录快照 + 写命令 + 错误面），写面与查询
//! 面经 [`ChangeStateStore`] 落库——crate 零 infra 依赖（AC-2 红线），适配器
//! `impl ChangeStateStore for Store` 驻 infra/store（D1）。时间戳统一 i64 UTC
//! unix 毫秒（store 惯例口径）；ISO 串出线归 queries 层单点。中性类型与持久
//! 化记录分离：native_model 版本治理全留 infra，本层零 derive 负担之外的形
//! 状承诺（serde / specta 仅为线面复用与测试断言）。

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

/// 相位机步骤种类封闭集（七臂命令包络；线格式 snake_case 词）。
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
    /// change 名（身份主键，不随归档改名变）
    pub name: String,
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
    pub change: String,
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
    pub change: String,
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
    pub change: String,
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
    pub change: String,
    /// 回跳发起相位（最新条目落 backtrack_to / backtrack_reason 标记）
    pub phase: String,
    /// 回跳目标相位（其最新 pass 条目置 stale）
    pub to: String,
    pub reason: String,
    /// stale 传播闭包（`phase_table::dependents` BFS 全量，不含目标自身）
    pub stale_dependents: Vec<String>,
}

/// 步骤审计写命令（`StepCommand`）：编排七臂命令包络逐条落行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepCommand {
    pub run_id: String,
    pub change: String,
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

// --- run 运行史（unify-run-state-persistence：决策翻案「不建 flow_runs 表」
// 立项，run 运行史落库 RunRecord / RunStepRecord 两表；词汇本体单点驻本节
// ——写命令与查询投影同类型消费）-------------------------------------------

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

/// run 步词汇封闭集（五值，线格式 snake_case 词）：agent 阶段（executor /
/// evaluator / decision）+ 脚本阶段（static_check / test_execution）。词汇
/// 本体单点——落库命令（`RunStepEntry.step`）与查询投影（`detail.rs`）同本
/// 类型消费，store 结构上收不到忽略集（流程面步骤与三门不可表达）。
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
    pub change: String,
    pub status: RunStatus,
    /// 终态记因 / 标定记因（running 恒 None）
    pub reason: Option<String>,
    /// 发起时刻（UTC unix 毫秒）
    pub started_at: i64,
    /// 收口 / 标定时刻（UTC unix 毫秒；running 恒 None）
    pub finished_at: Option<i64>,
}

/// run 步节点史行中性快照（`RunStepRecord` 的 port 流量像；图史面，与
/// `StepRecord` 审计职责分立）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunStepStateRecord {
    /// emit 序号（同 run 内 emit 序，库面空洞合法）
    pub seq: u64,
    pub run_id: String,
    pub phase: String,
    pub attempt: u32,
    pub step: RunStepKind,
    pub status: RunStepStatus,
    /// WorkerAgent 步所属会话 id（工具步为 None）
    pub session_id: Option<String>,
    /// 人读记因 / 摘要（有界，写面截断同 diagnose_brief 口径）
    pub detail: Option<String>,
    /// 落包时刻（UTC unix 毫秒，= finish 的 finished_at）
    pub timestamp: i64,
}

/// run 发起写命令（walker 起点第一写载荷）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStartCommand {
    pub run_id: String,
    pub change: String,
    /// 发起时刻（UTC unix 毫秒，命令层发起时铸造，corpus 确定性）
    pub started_at: i64,
}

/// run 步整包条目（finish 载荷内联；词汇 = [`RunStepKind`] 封闭集，流程面
/// 步骤与三门结构上不可表达）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStepEntry {
    /// emit 序号（全词汇 emit 序，被过滤步不占号——词汇在类型面已封闭）
    pub seq: u64,
    pub phase: String,
    pub attempt: u32,
    pub step: RunStepKind,
    pub status: RunStepStatus,
    pub session_id: Option<String>,
    pub detail: Option<String>,
}

/// run 收口写命令（终态更新 + 步整包 + active_phase 清位单事务载荷）。
/// `status` 为终态三值（completed / stopped / failed），`interrupted` 写面
/// 拒绝——运行期写路径不产生，仅启动标定写入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFinishCommand {
    pub run_id: String,
    pub change: String,
    pub status: RunStatus,
    pub reason: Option<String>,
    /// 收口时刻（UTC unix 毫秒，命令携带；corpus 确定性）
    pub finished_at: i64,
    pub steps: Vec<RunStepEntry>,
}

/// port 错误面（`StoreFault`）：读 / 写半边统一错误收敛；命令层以
/// `.to_string()` 呈现，`Display` 恒带语境前缀。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreFault {
    /// db 打开 / 事务 / 读写失败
    Db(String),
    /// 唯一性冲突（同名建档 / 重复落账）
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

    /// 建档单查（None = 文档形态：db 缺记录的存量 CLI change）。
    fn get_change(&self, name: &str) -> Result<Option<ChangeStateRecord>, StoreFault>;

    /// 建档全量（列表并集 db 半边；主键 name 自然序）。
    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault>;

    /// 相位评估史（按落行序 = id 升序；checklist 已按打包键序内联）。
    fn list_phase_records(&self, change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault>;

    /// 步骤审计枚举（时间序 = id 升序；run 可选圈定）。
    fn list_steps(
        &self,
        change: &str,
        run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault>;

    // --- 写半边 -----------------------------------------------------------

    /// 建档（同名记录已存在 → [`StoreFault::Conflict`]；写面 create 的建档
    /// 半边，status=active 起步）。
    fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault>;

    /// 补偿删除（create 双写 fs 半边失败的回滚面，design D5：只删本次自插
    /// 行；miss 幂等 `Ok(false)`）。
    fn delete_change_record(&self, name: &str) -> Result<bool, StoreFault>;

    /// 写 active_phase（attempt 写事务内推导 = 该相位既有条目数 + 1）；miss
    /// → [`StoreFault::NotFound`]。
    fn start_phase(
        &self,
        change: &str,
        phase: &str,
        now: i64,
    ) -> Result<PhaseStartState, StoreFault>;

    /// 落账单事务原子（PhaseRecord 行 + checklist 子行 + active_phase 清位 +
    /// `(change, phase, attempt)` 查重），返回事务内推导的 attempt。
    fn log_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreFault>;

    /// 回跳单事务（最新条目回跳标记 + 目标最新 pass 置 stale + 闭包全条目置
    /// stale）；发起相位无条目 → [`StoreFault::NotFound`]。
    fn apply_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreFault>;

    /// decision 槽位幂等挂账（该相位最新条目定点改写；无条目 →
    /// [`StoreFault::NotFound`]）。
    fn amend_decision_session(
        &self,
        change: &str,
        phase: &str,
        session_id: &str,
    ) -> Result<(), StoreFault>;

    /// status 翻转（归档 db 半边；主键 name 不变；miss →
    /// [`StoreFault::NotFound`]）。
    fn set_archived(&self, name: &str, archived_at: i64) -> Result<(), StoreFault>;

    /// 步骤审计行追加（行 id 写事务内 max+1）。
    fn append_step(&self, command: &StepCommand) -> Result<(), StoreFault>;

    // --- run 运行史（unify-run-state-persistence 决策翻案：run 运行史落库
    // RunRecord / RunStepRecord 两表；每 run 两写零每步写放大）--------------

    /// run 运行史清单（`started_at` 升序；run_id 并列稳定序由实现保证）。
    fn list_runs(&self, change: &str) -> Result<Vec<RunStateRecord>, StoreFault>;

    /// run 步节点史行（`seq` 升序 = emit 序；流程面步骤与三门不在
    /// [`RunStepKind`] 封闭集内，词汇过滤单点在落库侧——读面零过滤）。
    fn list_run_steps(&self, run_id: &str) -> Result<Vec<RunStepStateRecord>, StoreFault>;

    /// run 发起落行（status=running；同 run_id 冲突 → [`StoreFault::Conflict`]，
    /// change 未建档 → [`StoreFault::NotFound`]）。
    fn run_start(&self, command: &RunStartCommand) -> Result<(), StoreFault>;

    /// run 收口单事务（run 行在案且 running → 终态 + reason + finished_at +
    /// 步整包 + 该 change `active_phase` 清位；miss → [`StoreFault::NotFound`]，
    /// 非 running 或 status 含 `interrupted` → [`StoreFault::Conflict`]）。
    fn run_finish(&self, command: &RunFinishCommand) -> Result<(), StoreFault>;
}
