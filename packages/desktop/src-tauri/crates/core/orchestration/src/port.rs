use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use agent::{AgentEvent, AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance};
use foundation::cancel::CancelFlag;
use foundation::layout::ReportSlot;
use workflow::state::{RunFinishCommand, RunStartCommand};
use workflow::write::{BacktrackInput, PhaseLogInput, PhaseNextOutcome, PhaseStartOutcome};

use crate::state::{ArchivePreflight, RunUpdate};

/// WorkerAgent 轮 future 别名（std-only）。
pub type BoxTurnFuture = Pin<Box<dyn Future<Output = Result<WorkerTurnOutcome, String>> + Send>>;

/// ToolStep future 别名（std-only；失败统一 `Err` 串——显式失败停给用户）。
pub type BoxToolFuture = Pin<Box<dyn Future<Output = Result<ToolStepOutput, String>> + Send>>;

/// git diff 变更文件上下文 future 别名（std-only）。
pub type BoxDiffFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;

/// WorkerAgent 角色（source_ref 定式的第三段；D3）。归档相位的两 agent 步各
/// 占一词（`spec_sync` / `resolve_conflicts`）——同 phase 同 attempt 内 source_ref
/// 唯一，不撞号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRole {
    Executor,
    Evaluator,
    Decision,
    /// 归档 delta specs 同步会话（SpecSync 步）
    SpecSync,
    /// 归档合入冲突解算会话（ResolveConflicts 步）
    ResolveConflicts,
}

impl WorkerRole {
    /// sourceRef 角色段（线格式小写词）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Executor => "executor",
            Self::Evaluator => "evaluator",
            Self::Decision => "decision",
            Self::SpecSync => "spec_sync",
            Self::ResolveConflicts => "resolve_conflicts",
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
    /// 来源归属（source 恒 "change"，source_ref = change id / 相位 / 角色 / attempt 四段）
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

/// 工具步命令封闭集（相位机四步 + 四检查门禁 + 归档内联两步；写通道唯一——
/// 相位机载荷直载写面输入类型，进程内直调 `workflow::write`）。各变体定位载荷
/// 为 change **id**（一切寻址以 id 为准；磁盘面 name 由消费点经记录解析）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCommand {
    /// 相位路由（read-only；run_id 为会话窗口标识）
    PhaseNext { change_id: String, run_id: String },
    /// 开启阶段（attempt 计时）
    PhaseStart { change_id: String, phase: String },
    /// 评估落账（桌面代写）
    PhaseLog {
        change_id: String,
        phase: String,
        input: PhaseLogInput,
    },
    /// 回溯落账（白名单由 walker 缓存随行，写面二次校验兜底）
    Backtrack {
        change_id: String,
        phase: String,
        input: BacktrackInput,
    },
    /// 决策会话槽位挂账（该相位最新 eval 条目定点改写，幂等覆写）
    DecisionLog {
        change_id: String,
        phase: String,
        session_id: String,
    },
    /// static-check 门禁（`change_id` 归键：审计行按 change 可枚举；`slot` 报告
    /// 槽位——test 缺省 / baseline 基线检查，报告落点经 layout 槽位单点派生）
    StaticCheck { change_id: String, slot: ReportSlot },
    /// test-execution 门禁（确定性测试执行链；id 定位写面与记录，报告目录
    /// 派生经记录 name 解析；`slot` 报告槽位——test 缺省 / baseline 基线检查）
    TestExecution { change_id: String, slot: ReportSlot },
    /// mutation-test 门禁（确定性突变执行链；清单 / 补丁经 vcs 缝取数注入，
    /// 报告目录派生经记录 name 解析）
    MutationTest { change_id: String },
    /// archi-check 门禁（架构漂移判定；报告槽位恒 `ReportSlot::Archi` 由 runner
    /// 单点取用——不设 slot 字段，避免出现恒一值的形参；变更集经 vcs 缝取数注入）
    ArchiCheck { change_id: String },
    /// 归档前置只读载荷（store 读 + vcs 探测；请求 root 恒主 workspace root
    /// ——layout 解析与主仓分支探测锚）
    ArchivePreflight { change_id: String },
    /// 归档双写收口（写面 `archive` 单点复用：目录改名 + db 翻转；请求 root
    /// 恒主 workspace root）
    ArchiveSeal { change_id: String },
}

/// 工具步请求：workspace 根 + 步命令 + 取消信号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolStepRequest {
    /// workspace 根（写面 layout 解析与 spawn cwd）
    pub root: String,
    pub command: ToolCommand,
    /// 取消信号（run 级进程面标志；walker `run_tool` 单点从 `guard.cancel_flag()`
    /// 取值填充，各步函数不各自取值。缺省 `CancelFlag::never()` = 永不取消——
    /// 非取消路径行为零变化）。`Eq` 语义为**身份比较**（同一底层原子才相等），
    /// 故本载荷 derive 保持不变。
    pub cancel: CancelFlag,
}

/// 工具步产出（封闭集，载荷 = 写面原生类型；PhaseNext 体量最大（白名单 +
/// prompt 双 Vec），Box 收敛变体尺寸差）。`MutationTestOutcome` 载荷携 f64
///（score / threshold）——本枚举只派生 `PartialEq`（零 `Eq`）。
#[derive(Debug, Clone, PartialEq)]
pub enum ToolStepOutput {
    PhaseNext(Box<PhaseNextOutcome>),
    PhaseStart(PhaseStartOutcome),
    PhaseLog(workflow::write::PhaseLogOutcome),
    Backtrack(workflow::write::BacktrackOutcome),
    DecisionLog(workflow::write::DecisionLogOutcome),
    StaticCheck(StaticCheckOutcome),
    TestExecution(TestExecutionOutcome),
    MutationTest(MutationTestOutcome),
    ArchiCheck(ArchiCheckOutcome),
    ArchivePreflight(ArchivePreflight),
    ArchiveSeal(workflow::write::ArchiveOutcome),
}

/// 执行面归因（检查结论**之外**的原因；`None` 侧 = 结论本身有效）。
///
/// `Stopped` = 用户停止（cancel 置位 → 进程树击杀 → `Terminal::stopped`）；
/// `Environment(String)` = 环境错误摘要（瞬态锁耗尽 / 超时——相位重试必然再失败，
/// MUST NOT 注回 executor、MUST NOT 升格相位 fail）。
///
/// **零 `Serialize`——不出线**：不进 `RunUpdate` / `RunNodeRecord` 载荷、不改
/// `src/types/generated/bindings.ts`（三 outcome 均只在进程内 walker 消费）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunFault {
    /// 用户停止（节点 `stopped`、run `stopped`）
    Stopped,
    /// 环境错误摘要（节点 `failed`、run `failed`；不注回 executor）
    Environment(String),
}

/// static-check 门禁产出（passed=false 走定向反馈边）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticCheckOutcome {
    pub passed: bool,
    pub diagnostics: String,
    /// 执行面归因（`None` = 结论本身有效；在位时 `passed` 不参与门禁分派——
    /// walker 先查 `fault`）
    pub fault: Option<RunFault>,
    /// 全量输出引用（`ExecOutput::Recorded { exec_id }`；默认 recorded → 恒回填
    /// `Some(exec_id)`，落盘失败降级 `None`——不阻断检查主流程；反馈边仍只消费
    /// `diagnostics`，全量经本引用回查）
    pub output_ref: Option<String>,
    /// 报告目录（按槽位经 layout 单点派生，pass / fail / 无配置直过三分支均携
    /// ——baseline Terminal 文案供给面，与 `TestExecutionOutcome.report_dir`
    /// 对称）
    pub report_dir: String,
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
    /// 输出记录引用（`ExecOutput::Recorded { exec_id }`，经
    /// `exec::read_exec_output` 回查；与 `report_dir` 同锚同口径，落盘失败空串 =
    /// 未记录——`report_dir` 报告仍为全量权威）
    pub exec_id: String,
    /// 执行面归因（`None` = 结论本身有效；在位时 `conclusion` 不参与门禁分派——
    /// walker 先查 `fault`）
    pub fault: Option<RunFault>,
}

/// mutation-test 门禁结论三值（`TestExecutionConclusion` 同型；线格式小写词）：
/// `Pass` = 逐 suite 全达标（含跳过面——null 观察期不判 fail）、`Fail` = 全部段
/// 跑完但有 suite 未达标、`Error` = 段中断（段执行 error / harness 崩 / 超时）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationTestConclusion {
    /// 逐 suite 全部达标 / 跳过面收口
    Pass,
    /// 全部段跑完但有 suite 未达标
    Fail,
    /// 段中断（错误摘要随 `feedback_text` 即时反馈，不等汇总）
    Error,
}

impl MutationTestConclusion {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Error => "error",
        }
    }
}

/// 突变跳过因两值（`Option` 缺省 = 已执行；线格式 snake_case 词）：
/// `EmptyDiff` = 突变面为空（diff 空跳过）、`NoExecutor` = 全部 suite 无突变
/// 执行器（框架设计上无执行器，与「工具没装」语义不同）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationSkipReason {
    /// 突变面为空（编辑集无可突变文件落入任何 suite 桶）
    EmptyDiff,
    /// 全部 suite 无突变执行器（node-test 等）
    NoExecutor,
}

impl MutationSkipReason {
    /// 线格式 snake_case 词。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EmptyDiff => "empty_diff",
            Self::NoExecutor => "no_executor",
        }
    }
}

/// 突变归并计数载荷（`MutationMeasured` 计数面的 port 流量像——载荷类型自持，
/// core/orchestration 零 checks 依赖）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MutationCounts {
    pub killed: u64,
    pub survived: u64,
    pub timeout: u64,
    pub compile_error: u64,
    pub runtime_error: u64,
    pub ignored: u64,
    pub total: u64,
}

/// mutation-test 门禁产出载荷（**零 Serialize 不进 IPC**：只在进程内 walker
/// 反馈边②与机械 checklist 消费，MUST NOT 经 bindings / `RunUpdate` 出线——
/// 全量明细留报告文件）。
#[derive(Debug, Clone, PartialEq)]
pub struct MutationTestOutcome {
    /// 聚合结论
    pub conclusion: MutationTestConclusion,
    /// 跳过因（`None` = 已执行）
    pub skip: Option<MutationSkipReason>,
    /// 归并块口径得分（skip 时 0.0 占位）
    pub score: f64,
    /// 归并块口径阈值（首块阈值；skip 时 0.0 占位）
    pub threshold: f64,
    /// 归并计数
    pub counts: MutationCounts,
    /// 段级 + 归并报告路径清单（POSIX）
    pub report_paths: Vec<String>,
    /// 反馈文本（反馈边②报告指引式：结论摘要 + 报告路径清单；pass 恒空串；
    /// MUST NOT 内联存活突变体清单——明细由 executor 自行读报告）
    pub feedback_text: String,
    /// 执行面归因（`None` = 结论本身有效；在位时 `conclusion` 不参与门禁分派——
    /// walker 先查 `fault`。mutation 面只产 `Stopped`：段超时仍走 `Interrupted` +
    /// `conclusion = Error`，有意不进 fault）
    pub fault: Option<RunFault>,
}

/// ToolStep 契约（进程内缝）：相位机四步由编排侧 [`crate::steps::LocalToolSteps`]
/// 直调写面，static-check 委托注入的 [`StaticCheckRunner`]。
/// archi-check 门禁产出（`passed` 由 runner 侧从报告派生：`errors` 与
/// `model.structuralErrors` 皆空即通过——`warnings` / `observations` /
/// `suppressed` 一律不影响通过判定）。**零 Serialize 不进 IPC**：只在进程内
/// walker 反馈边与步 detail 消费，全量明细留报告文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiCheckOutcome {
    /// 门禁结论（`errors.is_empty() && model.structural_errors.is_empty()`）
    pub passed: bool,
    /// 诊断文本（runner 侧装配并**有界**：通过态为人读摘要、失败态为违例明细 +
    /// 固定处置提示；整体不超过 `MAX_REPORT_CHARS` 量级）
    pub diagnostics: String,
    /// 执行面归因（生产路径恒 `None`——本 runner 零进程；字段保留以与既有三
    /// outcome 同形，取消时落 `Some(RunFault::Stopped)`）
    pub fault: Option<RunFault>,
    /// 报告目录（`reports/archi`，经 layout 槽位单点派生）
    pub report_dir: String,
    /// 报告文件路径（`None` = 落盘失败——审计面降级不阻断结论）
    pub report_ref: Option<String>,
}

pub trait ToolStepPort: Send + Sync {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture;
}

/// static-check spawn 缝（infra 实现；W4——static-check 非 agent 会话，独立
/// port 不并入 [`WorkerAgentPort`]）。产出契约与 [`ToolStepPort`] 同封闭集
///（`ToolStepOutput::StaticCheck`）。**磁盘面 port 收 name**（报告目录
/// `change_reports(root, name, slot)` 为 name 化磁盘面、按槽位派生，infra 侧零
/// store 依赖）；id → 记录 → name 的解析在 `steps.rs` 消费点（与
/// [`TestExecutionRunner`] 同式）。`slot` 报告槽位：test 缺省 / baseline 基线
/// 检查——报告锚点与 exec-outputs 记录同锚派生，执行语义零分叉。
pub trait StaticCheckRunner: Send + Sync {
    /// 取消信号经 infra 执行体观测（`exec::run_shell` 轮询）——core 零 spawn；
    /// 置位 → 进程树击杀 → 产出以 `fault = Some(RunFault::Stopped)` 正常返回
    ///（取消 MUST NOT 以 `Err` 表达——`Err` 只留给进程拉不起 / 程序不可达等基础
    /// 设施失败）。
    fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture;
}

/// test-execution spawn 缝（checks 边界 infra 实现；W4 红线与
/// [`StaticCheckRunner`] 同型——检查域家族第二成员，spawn 不进 core）。产出
/// 契约同 `ToolStepOutput` 封闭集（`ToolStepOutput::TestExecution`）。**磁盘面
/// port 收 name**（报告目录 `change_reports(root, name, slot)` 为 name 化磁盘面、
/// 按槽位派生，infra 侧零 store 依赖）；id → 记录 → name 的解析在 `steps.rs`
/// 消费点（D7）。`slot` 报告槽位：test 缺省（既有 `reports/test` 语义零改动）/
/// baseline 基线检查（`reports/baseline` 分树隔离）——复用门按本槽位报告树判定。
pub trait TestExecutionRunner: Send + Sync {
    /// 取消信号经 infra 执行体观测——core 零 spawn；置位 → 进程树击杀 →
    /// 产出以 `fault = Some(RunFault::Stopped)` 正常返回（MUST NOT 返回 `Err`）；
    /// 取消 MUST NOT 落 plan 级 `execution_error`，也不再跑余下 plan。
    fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture;
}

/// mutation-test spawn 缝（checks 边界 infra 实现；检查域家族第三成员，W4 红线
/// 与 [`TestExecutionRunner`] 同型——spawn 不进 core）。产出契约同 `ToolStepOutput`
/// 封闭集（`ToolStepOutput::MutationTest`）。**四参显式化**（E3）：`root` = exec
/// root（worktree）、`name` = change 磁盘名（报告目录 `change_test_reports(root,
/// name)` 派生）、`files` = 突变面清单入参、`patch` = `--in-diff` 补丁入参——
/// 两者同经 vcs 缝（[`ArchiveVcsPort::change_edit_set`] / [`change_edit_patch`]）
/// 取数后由 `steps.rs` 注入，checks-runtime 依赖面零 vcs（清单 / 补丁唯一入口在
/// vcs 边界）。
pub trait MutationRunner: Send + Sync {
    /// 末位为取消信号（经 infra 执行体观测——core 零 spawn）；置位 → 进程树击杀
    /// → 产出以 `fault = Some(RunFault::Stopped)` 正常返回（MUST NOT 以 `Err` /
    /// `conclusion = Error` 表达取消）。段超时仍走 `Interrupted` +
    /// `conclusion = Error` + `fault = None`（长跑是产品语义，超时是配置问题）。
    fn run(
        &self,
        root: &str,
        name: &str,
        files: &[String],
        patch: &str,
        cancel: CancelFlag,
    ) -> BoxToolFuture;
}

/// archi-check 门禁缝（infra 实现，`crates/infra/archi`；检查域家族第四成员，
/// 与 [`MutationRunner`] 同式——**本 runner 零进程**，判定全部复用 `archi` 的
/// 漂移判定纯函数）。产出契约同 `ToolStepOutput` 封闭集
///（`ToolStepOutput::ArchiCheck`）。**四参显式化**：`root` = exec root
///（worktree / 主仓根——模型与源码同根）、`name` = change 磁盘名（报告目录
/// `change_reports(root, name, ReportSlot::Archi)` 派生）、`changed` = 变更集
/// （经 [`ArchiveVcsPort::change_edit_set`] 由 `steps.rs` 注入，infra 侧零 git）、
/// 末位取消信号。报告槽位不设形参（恒 `ReportSlot::Archi`，runner 内部取用）。
pub trait ArchiCheckRunner: Send + Sync {
    /// 取消信号经扫描循环观测（遍历 / 解析内轮询）——置位 → 产出以
    /// `fault = Some(RunFault::Stopped)` 正常返回（MUST NOT 以 `Err` 表达取消）。
    /// 报告写盘失败 MUST NOT 返回 `Err`（降级 `report_ref = None` 并保留结论）。
    fn run(
        &self,
        root: &str,
        name: &str,
        changed: Vec<String>,
        cancel: CancelFlag,
    ) -> BoxToolFuture;
}

/// git diff 变更文件上下文缝（infra 实现；W5——git diff 是进程 spawn，落
/// infra）。产出为拼接好的上下文文本（porcelain 清单 + diff HEAD 补丁）。
pub trait DiffContextPort: Send + Sync {
    fn diff_context(&self, root: &str) -> BoxDiffFuture;
}

/// git 步执行缝（walker 第三执行缝）：归档相位的 git 步（commit / rebase /
/// merge / finalize）与前置探测（current_branch / branch_merged）经本缝直调；
/// port 定义落消费者（core/orchestration），实现驻 `crates/infra/vcs`
/// （进程 spawn 不进 core）。
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
    /// 变更文件清单唯一入口（**突变范围与文件清单唯一入口**，清单获取全仓单点）：
    /// `git diff <base> --name-only -z` ∪ `git ls-files --others --exclude-standard -z`
    /// （untracked 并入），排序去重 POSIX 归一；`base = "HEAD"` 时即本 change 编辑集
    ///（worktree 语义）。git 失败显式 `Err`——MUST NOT 静默降级空清单（空清单会
    /// 误走 diff 空跳过）。
    fn change_edit_set(&self, root: &Path, base: &str) -> Result<Vec<String>, String>;
    /// `--in-diff` 补丁唯一入口：untracked 面先行 `git add -N`（intent-to-add，
    /// E2）再 `git diff <base>` 输出 unified diff 文本；与 [`Self::change_edit_set`]
    /// 同居 vcs 单点，checks 侧零 git spawn。
    fn change_edit_patch(&self, root: &Path, base: &str) -> Result<String, String>;
    /// worktree 清理：`git worktree remove --force <path>`（`-C main_root` 锚）
    /// ——归档相位 Cleanup 步的尽力步执行缝，`--force` 带走 bootstrap 的
    /// `node_modules` / `target` 磁盘大头；branch MUST NOT 触碰（`worktree
    /// remove` 本就不动分支）；`Err` 面带路径与手动清理指引（`git worktree
    /// remove --force <path>`）。worktree 在场探测驻调用方 fs 判定，port 零探
    /// 测方法。
    fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>;
    /// 基线恢复（initialize 相位 Clean 步执行缝）：`reset --hard` + `clean -fd`
    /// 逐项 `-e` 排除（不带 `-x`——尊重 .gitignore，依赖树常态已免疫）。返回
    /// 恢复摘要（步 detail 消费）；`Err` 带 git 语境。worktree 在场判定驻调用
    /// 方（walker），port 零探测方法；主仓 MUST NOT 传入（legacy 整相位短路
    /// 先行）。
    fn restore_baseline(&self, worktree: &Path, exclusions: &[&str]) -> Result<String, String>;
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
/// 写是两个关注点——写触点唯一经 `workflow::write` 写面）。第二参为 change
/// **id**（经 queries id 寻址）。
pub trait WorkflowSnapshotPort: Send + Sync {
    fn detail(&self, root: &str, id: &str) -> Result<workflow::queries::ChangeDetail, String>;
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
