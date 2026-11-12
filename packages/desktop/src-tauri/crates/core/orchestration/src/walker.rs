use std::path::Path;
use std::sync::Arc;

use agent::{sum_usage_tokens, AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance};
use foundation::layout::{domain_dir_name, ReportSlot};
use workflow::model::{ChecklistItem, Verdict};
use workflow::queries::ChangeDetail;
use workflow::write::{
    ArchiveOutcome, BacktrackInput, BacktrackOption, BacktrackOutcome, BacktrackWindowEntry,
    DecisionLogOutcome, OnFail, PhaseLogInput, PhaseLogOutcome, PhaseNextError, PhaseNextOutcome,
    PhaseStartOutcome, StepDefinition, StepNode, INITIALIZE_PHASE_ID,
};

use crate::control::{emit_node, now_millis, ChangeFlowControl, RunGuard, StepSettle};
use crate::decision::{ensure_backtrack_allowed, CandidateReport, DecisionAction, DecisionInput};
use crate::finish::FinishPayload;
use crate::port::{
    ArchiCheckOutcome, ArchiveVcsPort, MutationSkipReason, MutationTestConclusion,
    MutationTestOutcome, OutputRefSink, RebaseOutcome, RunFault, RunHistoryPort,
    StaticCheckOutcome, TestExecutionConclusion, TestExecutionOutcome, ToolCommand, ToolStepOutput,
    ToolStepPort, WorkerAgentPort, WorkerRole, WorkerTurnOutcome, WorkerTurnRequest,
    WorkflowSnapshotPort, WorktreeSnapshot,
};
use crate::prompt::{decision_prompt, evaluator_prompt, merge_conflict_prompt, spec_sync_prompt};
use crate::run_history;
use crate::state::{
    ArchivePreflight, ChangeRunStatus, ChangeStepEdge, ChangeStepKind, EdgeKind, NodeKey, RunUpdate,
};
use crate::steps::{change_branch, check_rebase_halfway, clip_reason};
use crate::transcript::failure_reason;
use crate::verdict::{EvaluatorChecklist, MAX_REPORT_CHARS};

/// 会话 provenance 来源（sourceRef = change id / 相位 / 角色 / attempt 四段——
/// 身份段恒 change id）。
const SOURCE_CHANGE: &str = "change";

/// run 收口文案（全相位含 archive 相位 pass 走完——归档为相位序末段，其完成
/// 即 run 收口）。
const ALL_PHASES_PASSED: &str = "All phases have passed, including archiving. Run completed.";

/// 归档 git / 内联步跳过因词汇单点（step 内判断的短路依据以 `detail` 落节点：
/// 跳过不是独立状态，`status = passed` + 跳过因）。
const TXT_SKIP_CLEAN: &str = "干净";
const TXT_SKIP_MERGED: &str = "已合入";
const TXT_SKIP_NO_DELTA: &str = "无 delta specs";
const TXT_SKIP_BY_USER: &str = "用户选择";
const TXT_SKIP_LEGACY: &str = "legacy 无 worktree";
const TXT_SKIP_ABSENT: &str = "worktree 已不在场，跳过清理";
const TXT_SKIP_FINALIZED: &str = "已落盘";
const TXT_NO_CONFLICT: &str = "无冲突";

/// initialize 相位词汇（legacy 跳过因 + baseline 三态失败文案——三态不共用文
/// 案，均 Terminal 停给用户；`{dir}` 为 baseline 报告目录占位）。
const TXT_SKIP_LEGACY_INITIALIZE: &str = "legacy 主仓执行无 worktree 基线，跳过基线检查";
const TXT_BASELINE_STATIC_FAILED: &str =
    "基线静态检查未通过，请在基线上修复后重新发起（基线的债不属于本次变更）；报告目录 {dir}";
const TXT_BASELINE_TEST_ERROR: &str =
    "基线测试执行出错（环境 / 配置问题），请检查配置与环境后重新发起；报告目录 {dir}";
const TXT_BASELINE_TEST_FAILED: &str =
    "基线测试未通过，请在基线上修复测试后重新发起；报告目录 {dir}";

/// 环境错误终局文案（执行面归因 `RunFault::Environment` 专用——与 `TXT_BASELINE_*`
/// **零共用引导句**：本组引导「排除安全软件路径」，基线组引导「在基线上修复后
/// 重新发起」，两态 MUST NOT 复用同一段文案）。
///
/// `{fault}` 承载占用者点名 / 被锁文件 / 「已自动重试 N 次」/ 「排除 workspace 父
/// 目录（逐 worktree 无效）」指引（MUST NOT 在模板里再写一份——那些事实由
/// checks 域 `environment` 单点装配）；`{dir}` 为报告目录。
const TXT_ENV_STATIC: &str = "静态检查无法完成（环境错误）：\n{fault}\n报告目录 {dir}";
const TXT_ENV_TEST: &str = "测试执行无法完成（环境错误）：\n{fault}\n报告目录 {dir}";

/// lean 档无法裁决收敛的原因词族（解冲突后验不通过的收敛原因，词汇单点驻本
/// 文件）。
const LEAN_AGENT_FAILED: &str = "agent 会话失败";
const LEAN_AGENT_STOPPED: &str = "agent 会话被停止";
const LEAN_RESIDUAL: &str = "残留冲突未解";
const LEAN_OUTSIDE_CHANGES: &str = "清单外新改动";
const LEAN_PROBE_FAILED: &str = "后验探测失败";
const LEAN_REBASE_CLOSED: &str = "agent 违约自行收口 rebase";
/// lean 串 abort 附注（收敛序中 abort 自身失败的显式面——不掩盖原记因）。
const TXT_LEAN_ABORT_FAILED: &str = "abort 未成功，请手动核验 worktree git 状态";
/// 被停止注记（冲突 agent 被 stop 的 lean 串尾注——冲突摘要与手动裁决引导照常
/// 呈现）。
const TXT_LEAN_STOPPED_NOTE: &str = "（归档已停止）";
/// `rebase_continue` 失败的收口引导（冲突解已验通过、rebase 态完整——人工续走
/// 或回退二选一，重试前半截态须人工清，MUST NOT 自动 abort 已验的解算成果）。
const TXT_REBASE_CONTINUE_GUIDE: &str = "worktree 处于 rebase 态：可手动 git rebase --continue\
     续走收口（完成后主仓手动 git merge --ff-only 快进），或 git rebase --abort 回退后重试归档\
     （重试前半截态须人工清）";
/// Rebase 段冲突交据（冲突清单非空时交 ResolveConflicts 步条件解算）。
const TXT_REBASE_CONFLICTED: &str = "冲突待解算";

/// ask 携出会话外（提问已迁出为阻塞工具 `AskUserQuestion`）= 编排漂移（防线单
/// 点文案：`resolve_deadlock` 与决策会话动作裁决面的 `Ask` 臂共用——两处都只在
/// 历史载荷回放 / 装配漂移时读到 `Ask`）。
const TXT_ASK_DRIFT: &str = "决策漂移：ask 应经 AskUserQuestion 工具提交";

/// run 发起入参：workspace 根（exec root —— 会话 cwd / 检查域执行目录）+ 主
/// workspace root（归档相位的主仓锚：layout 解析 / 归档双写 / 落盘提交）+ change
/// id + 会话窗口标识（`new_run_id` 铸造，每 run 一个）+ 发起时刻（命令层铸造，
/// 与 `begin_run` 同值入 RunEntry——运行史 started_at 与统一视图 startedAt 的
/// 同源锚）+ 「按指定 phase 停止」停止点（stop-before-phase）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub root: String,
    /// 主 workspace root（双 root 拆分：`root` 为 worktree exec root 时本字段
    /// 恒主仓；归档步的 git / layout 锚）
    pub main_root: String,
    /// change 身份锚（uuid 形态；一切寻址入参）
    pub change_id: String,
    pub run_id: String,
    pub auto_next_phase: bool,
    /// 停止点相位名（`Some(phase)` = 进入该相位前必停等确认一次，无论 auto 与
    /// 否；命令层固定 `Some("archive")`——归档为独立手动执行门）
    pub stop_before: Option<String>,
    /// 发起时刻（UTC unix 毫秒；run 运行史 started_at 落库与快照投影同源）
    pub started_at: i64,
}

/// 铸造 `run-<millis>` 会话窗口标识（每 run 发起一个；时钟早于 epoch 取 0，
/// 不 panic）。
pub fn new_run_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    format!("run-{millis}")
}

/// 内部终态：helper 层失败路径的携带物（guard 未动，finish 由主入口单点）。
struct Terminal {
    status: ChangeRunStatus,
    reason: Option<String>,
}

impl Terminal {
    fn failed(reason: String) -> Self {
        Self {
            status: ChangeRunStatus::Failed,
            reason: Some(reason),
        }
    }

    fn stopped(reason: &str) -> Self {
        Self {
            status: ChangeRunStatus::Stopped,
            reason: Some(reason.to_owned()),
        }
    }

    /// drive 返回面拆解。
    fn into_pair(self) -> (ChangeRunStatus, Option<String>) {
        (self.status, self.reason)
    }
}

/// 相位循环主入口（每 run 两写时序，unify-run-state-persistence D5/D6）：起点第
/// 一写（running 行，失败 fail-fast 收口）→ drive（计划步循环，emit 节点 + 边经
/// control 累积 + Notice 广播）→ 终态出口单点第二写（节点整包 + 边整包 →
/// run_history 组装 → best-effort 落包）→ `guard.finish`（既有唯一终态出口）。
// 三执行缝（agent / 工具步 / vcs）+ 快照 + 运行史 + 注册表 + 控制柄 + 入参：
// 参数面为组合根装配契约（design「公共函数 / API」定稿签名），拆结构体只增间接。
#[allow(clippy::too_many_arguments)]
pub async fn walk_run(
    worker: Arc<dyn WorkerAgentPort>,
    tools: Arc<dyn ToolStepPort>,
    vcs: Arc<dyn ArchiveVcsPort>,
    snapshot: Arc<dyn WorkflowSnapshotPort>,
    history: Arc<dyn RunHistoryPort>,
    control: Arc<ChangeFlowControl>,
    guard: RunGuard,
    request: RunRequest,
) -> ChangeRunStatus {
    // 起点第一写：run 运行史建 running 行（支撑启动标定）。失败 = store 已
    // 坏，后续相位写必败——fail-fast 收口（D5），零相位执行
    if let Err(error) = history.run_started(&workflow::state::RunStartCommand {
        run_id: request.run_id.clone(),
        change_id: request.change_id.clone(),
        started_at: request.started_at,
    }) {
        let reason = format!("run 运行史落库失败（run 起始行）: {error}");
        drop(control);
        guard.finish(ChangeRunStatus::Failed, Some(reason));
        return ChangeRunStatus::Failed;
    }
    let (status, reason) = drive(&worker, &tools, &vcs, &snapshot, &guard, &request).await;
    // 终态出口单点第二写：终态 + 节点整包 + 边整包 + active_phase 清位单事务落
    // 库。best-effort（D6）：Err 静默不阻断收口——guard.finish 照常，run 终态与
    // 订阅释放不受损；start 行残留 running 经启动标定 interrupted 自愈留痕。
    let nodes = guard.nodes();
    let edges = guard.edges();
    let command = run_history::finish_command(
        &request,
        status,
        reason.clone(),
        now_millis(),
        &nodes,
        &edges,
    );
    if let Err(error) = history.run_finished(&command) {
        // D6：finish 落包失败收敛 = best-effort 单次尝试，无重试无错误面
        let _ = error;
    }
    drop(control);
    guard.finish(status, reason);
    status
}

/// 循环体：终态一律以 `(status, reason)` 返回，不触 finish。
///
/// **计划图驱动**：每轮问写面 `phase_next`（路由唯一权威，白名单随响应下发），
/// next 相位按响应下发的 `steps` 计划步序列迭代执行——walker 零自持步序（门禁
/// 由 step 声明、反馈 / 终态由步 `on_fail` 语义承载）；相位间 confirm 停等仍为
/// run 级 `auto_next_phase` 开关（非 step 属性），并在 `stop_before` 相位前追加
/// 一道无条件停止门（stop-before-phase——归档为固定停止点，手动执行语义）。
async fn drive(
    worker: &Arc<dyn WorkerAgentPort>,
    tools: &Arc<dyn ToolStepPort>,
    vcs: &Arc<dyn ArchiveVcsPort>,
    snapshot: &Arc<dyn WorkflowSnapshotPort>,
    guard: &RunGuard,
    request: &RunRequest,
) -> (ChangeRunStatus, Option<String>) {
    let mut trace = EdgeTrace::default();
    loop {
        if let Err(terminal) = check_cancel(guard) {
            return terminal.into_pair();
        }
        // ① phase-next（路由唯一权威；白名单与 next 相位计划步序列 / 产物清单
        //    随响应下发）
        let next = match step_phase_next(tools, guard, request).await {
            Ok(outcome) => outcome,
            Err(terminal) => return terminal.into_pair(),
        };
        if next.done {
            return (
                ChangeRunStatus::Completed,
                Some(ALL_PHASES_PASSED.to_owned()),
            );
        }
        if let Some(error) = &next.error {
            // 决策分叉（白名单随 error 响应下发——walker 不持跨迭代缓存）
            match resolve_deadlock(
                worker,
                tools,
                snapshot,
                guard,
                request,
                &next,
                error,
                &next.allowed_backtrack_phases,
                &mut trace,
            )
            .await
            {
                Deadlock::Continue => continue,
                Deadlock::Terminal(terminal) => return terminal.into_pair(),
            }
        }
        let Some(phase) = next.next_phase.clone() else {
            return Terminal::failed(
                "phase-next 输出漂移：非 done 非错误响应缺 next_phase".to_owned(),
            )
            .into_pair();
        };
        if next.steps.is_empty() {
            // 计划步序列缺位即计划图漂移（相位定义恒含至少一个真实步）：显式失
            // 败停给用户，MUST NOT 空转（无步可走却回 phase-next 会成为活锁）
            return Terminal::failed(format!(
                "phase-next 输出漂移：相位 \"{phase}\" 缺计划步序列（PhaseDefinition.steps）"
            ))
            .into_pair();
        }

        // 停止门（stop-before-phase）：无论 auto 与否，进入 stop_before 相位前
        // 必发一道确认停等——归档前置只读载荷随行（停等面数据面），确认回流携
        // 「是否执行 delta specs 同步」。手动模式下相位收口后的逐相位确认块对
        // stop_before 相位跳过（归档前恰好一道确认、归档后零确认）。
        let mut sync_specs = true;
        let stop_gate = request.stop_before.as_deref() == Some(phase.as_str());
        if stop_gate {
            let preflight = run_tool::<ArchivePreflight>(
                tools,
                guard,
                &request.main_root,
                ToolCommand::ArchivePreflight {
                    change_id: request.change_id.clone(),
                },
                "archive-preflight",
                None,
            )
            .await
            .ok();
            guard.emit(RunUpdate::ConfirmWait {
                phase: phase.clone(),
                archive: preflight,
            });
            let (proceed, confirmed_sync_specs) = guard.wait_confirm().await;
            if !proceed {
                return Terminal::stopped(&format!(
                    "phase \"{phase}\" 停等未获继续确认（归档为独立手动执行门）"
                ))
                .into_pair();
            }
            sync_specs = confirmed_sync_specs;
        }

        // 相位入口 incoming 边：决策 backtrack 待决值 > 同相位上一轮 fail 重跑
        //（retry）> 计划序推进（forward）。`retry` 边为存量解码契约（`Retry`
        // 动作已退役、新写面零产出）——本分支零改动保留，新写面不可达（失败
        // 相位只经决策 backtrack 重入，backtrack 优先级更高）；清理需另立变更
        let entry_kind = match trace.incoming.take() {
            Some(EdgeKind::Backtrack) => EdgeKind::Backtrack,
            _ if trace.failed_phase.as_deref() == Some(phase.as_str()) => EdgeKind::Retry,
            _ => EdgeKind::Forward,
        };
        let ctx = StepContext {
            worker,
            tools,
            vcs,
            guard,
            request,
            next: &next,
            phase: &phase,
        };
        let mut cursor = PhaseCursor::new(&next, sync_specs);
        // 开相：相位机固定动作（零 emit / 零审计 / 零计划步）
        cursor.attempt = match enter_phase(tools, guard, request, &phase).await {
            Ok(attempt) => attempt,
            Err(terminal) => {
                trace.entry_override = None;
                return terminal.into_pair();
            }
        };
        // legacy 整相位短路（initialize 表头一次性基线检查相位）：`worktree=None`
        // 的存量记录 exec root = 主仓（root == main_root ⟺ worktree 缺席——发起
        // 前置校验保证 worktree 在场时 exec root ≠ 主仓）——restore / 检查 spawn
        // 在主仓执行会 reset 用户未提交改动（灾难），故步序列零执行（零 vcs /
        // 零检查 spawn、主仓零重置零 clean），以 `settle_phase(skipped)` 收口（跳过
        // 因可见，一次性持久同 pass 语义）
        if phase == INITIALIZE_PHASE_ID && request.root == request.main_root {
            trace.entry_override = None;
            if let Err(terminal) = settle_phase(
                tools,
                guard,
                request,
                &mut cursor,
                &phase,
                PhaseSettle::Skipped(TXT_SKIP_LEGACY_INITIALIZE),
            )
            .await
            {
                return terminal.into_pair();
            }
        } else {
            // ②–⑥ 相位主体：按计划步序列迭代（exec / agent / git 由 `StepNode`
            //      声明，walker 按 `group()` 语义分派执行体）。相位入口边由首个
            //      真实步节点承载（辅助步退场后相位入口不再是节点）
            trace.entry_override = Some(entry_kind);
            let mut upgraded = false;
            for step in &next.steps {
                match run_step(&ctx, &mut cursor, &mut trace, step).await {
                    Ok(StepFlow::Proceed) => {}
                    Ok(StepFlow::Upgraded) => {
                        upgraded = true;
                        break;
                    }
                    Err(terminal) => {
                        trace.entry_override = None;
                        return terminal.into_pair();
                    }
                }
            }
            trace.entry_override = None;
            trace.failed_phase = cursor.failed.then(|| phase.clone());
            if upgraded {
                // 门禁步失败升格（fail 已由 `settle_phase(Fail)` 落账、零会话）：
                // 回 phase-next 进决策分叉
                continue;
            }
            // 相位收口：三分流取自 cursor 产物 → 落账 → active_phase 清位（零 emit）
            if let Err(terminal) = settle_phase(
                tools,
                guard,
                request,
                &mut cursor,
                &phase,
                PhaseSettle::Pass,
            )
            .await
            {
                return terminal.into_pair();
            }
        }
        // verdict fail 的结论由收口动作给出，故 failed_phase 的第二次置位必须在
        // 收口之后（否则同相位重跑边判定读到旧值）
        trace.failed_phase = cursor.failed.then(|| phase.clone());

        // phase 间停等确认 or 自动确认（stop_before 相位前已在停止门确认过，
        // 收口后不再补一道——归档后零确认）
        if !request.auto_next_phase && !stop_gate {
            guard.emit(RunUpdate::ConfirmWait {
                phase: phase.clone(),
                archive: None,
            });
            let (proceed, _) = guard.wait_confirm().await;
            if !proceed {
                return Terminal::stopped(&format!("phase \"{phase}\" 已通过，停等未获继续确认"))
                    .into_pair();
            }
        }

        // ⑦ 回 phase-next：pass 推进 / fail 条目未决 → 决策分叉（重做本相位经
        //    决策 backtrack 到自身表达）
    }
}

// ---------------------------------------------------------------------------
// 执行图节点与边（walker 侧 emit 单点）
// ---------------------------------------------------------------------------

/// 执行图边追踪：链尾节点身份键（incoming 边的源）+ 相位入口待消费边 kind
///（决策 backtrack 落账后置位，相位入口消费一次即清）+ 相位入口边覆盖值
///（walker 在进入步序列后置位，相位首个真实步节点落边时一次性消费）+ 上一轮
/// 相位 fail 记因（同相位重跑 → retry 边判定）。
#[derive(Default)]
struct EdgeTrace {
    tail: Option<NodeKey>,
    incoming: Option<EdgeKind>,
    /// 相位入口边覆盖值：辅助步退场后相位入口不再是节点，故该边由相位首个真实
    /// 步节点承载（`begin_node` 一次性取用，`drive` 兜底清零）
    entry_override: Option<EdgeKind>,
    failed_phase: Option<String>,
}

/// 节点身份键组装（`(phase, attempt, step, instance_no)`）。
fn node_key(phase: &str, attempt: u32, step: ChangeStepKind, instance_no: u32) -> NodeKey {
    (phase.to_owned(), attempt, step, instance_no)
}

/// 步节点起点：分配实例号（`(phase, attempt, step)` 单点递增，feedback 重跑为递
/// 增新实例）→ 落 incoming 边（链尾 → 本节点；run 首节点无源不落边）→ 链尾前
/// 移；返回实例号供步状态行 emit 复用。
///
/// 落边时一次性消费 [`EdgeTrace::entry_override`]（相位入口边由相位首个真实步节点
/// 承载：有覆盖值用覆盖值，否则用调用方传入的 `incoming`）。
fn begin_node(
    guard: &RunGuard,
    trace: &mut EdgeTrace,
    phase: &str,
    attempt: u32,
    step: ChangeStepKind,
    incoming: EdgeKind,
) -> u32 {
    let instance_no = guard.begin_instance(phase, attempt, step);
    let key = node_key(phase, attempt, step, instance_no);
    let incoming = trace.entry_override.take().unwrap_or(incoming);
    if let Some(from) = trace.tail.clone() {
        if from != key {
            emit_edge(guard, from, key.clone(), incoming);
        }
    }
    trace.tail = Some(key);
    instance_no
}

/// 边 emit 单点（四 kind 显式落边）。
fn emit_edge(guard: &RunGuard, from: NodeKey, to: NodeKey, kind: EdgeKind) {
    guard.emit(RunUpdate::Edge {
        edge: ChangeStepEdge { from, to, kind },
    });
}

// ---------------------------------------------------------------------------
// 相位内计划步执行体（`next.steps` 只含真实业务步——开相归 `enter_phase`、落账归
// `settle_phase`，两者是相位机固定动作而非计划步；initialize 表头一次性基线检查
// 相位步序为 Clean → StaticCheck → TestExecution——Clean 为 vcs 直调基线恢复步，
// 两检查步 `on_fail=Terminal` 无反馈边例外、失败即停给用户；其余相位沿各表
// steps 声明执行，机械 checklist 由相位收口复写）
// ---------------------------------------------------------------------------

/// 计划步执行携出物。
enum StepFlow {
    /// 步完成，继续计划序下一步。
    Proceed,
    /// 门禁升格（fail 已由 `settle_phase(PhaseSettle::Fail)` 代写）：截断本相位余
    /// 下步回 phase-next。
    Upgraded,
}

/// 相位步执行上下文（借用打包；路由产出与目标相位随行）。
struct StepContext<'a> {
    worker: &'a Arc<dyn WorkerAgentPort>,
    tools: &'a Arc<dyn ToolStepPort>,
    /// vcs 执行缝（walker 第三缝）：归档 git 步与前探测直调
    vcs: &'a Arc<dyn ArchiveVcsPort>,
    guard: &'a RunGuard,
    request: &'a RunRequest,
    /// next 相位路由产出（已组装 prompt / 白名单 / 计划步序列）
    next: &'a PhaseNextOutcome,
    /// next 相位 id
    phase: &'a str,
}

/// evaluator 步收口产物：会话结果与收口载荷同源——消「有 `outcome` 无 checklist」
/// 的非法可达态（两份事实合一槽）。
struct EvaluatorTurn {
    outcome: WorkerTurnOutcome,
    checklist: EvaluatorChecklist,
}

/// 相位内步序携出状态（跨 step 传递的产物）。
struct PhaseCursor {
    /// 开相 attempt（`enter_phase` 产出；未开相 0）
    attempt: u32,
    /// 当前 executor 会话槽（反馈边续注同会话；相位收口落账携出）
    executor_session: Option<String>,
    /// executor 模型档（相位表 spec 供给；门禁反馈边续注不换档）
    executor_level: ModelLevel,
    /// evaluator 步收口产物（会话结果 + checklist 同源；相位收口消费）
    evaluator_turn: Option<EvaluatorTurn>,
    /// test-execution 门禁通过产物（相位收口机械代写消费）
    test_outcome: Option<TestExecutionOutcome>,
    /// mutation-test 门禁通过产物（含跳过面——相位收口机械 checklist 第 3 条
    /// evidence 三态消费；与 `test_outcome` 双槽并立互不覆盖）
    mutation_outcome: Option<MutationTestOutcome>,
    /// 归档前置载荷（Preflight 步产出 → Commit / Rebase / ResolveConflicts /
    /// Merge / SpecSync / Finalize 消费）
    preflight: Option<ArchivePreflight>,
    /// 归档重放冲突清单（Rebase 步产出 → ResolveConflicts 步消费；`None` = 未
    /// 重放，`Some(空)` = 重放无冲突）
    rebase_conflicts: Option<Vec<String>>,
    /// 归档前停等回流的「是否执行 delta specs 同步」（SpecSync 步消费）
    sync_specs: bool,
    /// 归档双写产物（Seal 步产出 → Finalize 消费）
    seal_outcome: Option<ArchiveOutcome>,
    /// 本相位是否以 fail 收场（相位重试边判定面；由 `settle_phase` 单点回写）
    failed: bool,
}

impl PhaseCursor {
    /// 空携出状态（attempt 未开、无会话、无收口产物）。
    fn new(next: &PhaseNextOutcome, sync_specs: bool) -> Self {
        Self {
            attempt: 0,
            executor_session: None,
            executor_level: next
                .executor
                .as_ref()
                .map(|spec| spec.model_level)
                .unwrap_or_default(),
            evaluator_turn: None,
            test_outcome: None,
            mutation_outcome: None,
            preflight: None,
            rebase_conflicts: None,
            sync_specs,
            seal_outcome: None,
            failed: false,
        }
    }
}

/// 计划步分派（**按 step 节点语义执行，无步序 / 门禁特判**）：`steps` 只含真实
/// 业务步——辅助步（phase-start / phase-log / verdict-gate）与三门 gate 已退出
/// 计划图（前三者由相位机固定动作或 evaluator 步内化承载，后两者节点面归零），
/// 故六变体共用一条不可达臂：计划图漂移即显式失败，MUST NOT 静默 `Proceed`（原
/// 空臂形态会把漂移吞成一次空操作）。
async fn run_step(
    ctx: &StepContext<'_>,
    cursor: &mut PhaseCursor,
    trace: &mut EdgeTrace,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    match step.node {
        // initialize 基线恢复步（vcs 直调，git 步族区段先例同归档步）
        StepNode::Clean => {
            step_clean(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Executor => {
            step_executor(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::StaticCheck => step_static_check(ctx, trace, cursor, step).await,
        StepNode::TestExecution => step_test_execution(ctx, trace, cursor, step).await,
        StepNode::MutationTest => step_mutation_test(ctx, trace, cursor, step).await,
        StepNode::ArchiCheck => step_archi_check(ctx, trace, cursor, step).await,
        StepNode::Evaluator => {
            step_evaluator(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        // ── 归档相位 9 步（Cleanup 尽力步例外：失败仅 detail 警告恒 passed；
        // 其余步失败一律 Terminal：相位表 `on_fail = None`）──
        StepNode::Preflight => {
            step_preflight(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Commit => {
            step_commit(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Rebase => {
            step_rebase(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::ResolveConflicts => {
            step_resolve_conflicts(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Merge => {
            step_merge(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::SpecSync => {
            step_spec_sync(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Cleanup => {
            step_cleanup(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Seal => {
            step_seal(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Finalize => {
            step_finalize(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Decision
        | StepNode::RetryGate
        | StepNode::WhitelistGate
        | StepNode::PhaseStart
        | StepNode::PhaseLog
        | StepNode::VerdictGate => Err(Terminal::failed(format!(
            "计划图漂移：相位 \"{}\" 的 steps 含不可达步词 \"{}\"（该步词不出现在任何相位的 steps 内）",
            ctx.phase,
            step.node.as_str()
        ))),
    }
}

async fn step_phase_next(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
) -> Result<PhaseNextOutcome, Terminal> {
    run_tool(
        tools,
        guard,
        &request.root,
        ToolCommand::PhaseNext {
            change_id: request.change_id.clone(),
            run_id: request.run_id.clone(),
        },
        "phase-next",
        None,
    )
    .await
}

/// 相位入口（相位机固定动作，非计划步、非图节点）：经写面既有 `phase_start` 单
/// 事务写 `active_phase`（phase / attempt / start_at），返回 attempt。
///
/// **零 emit、零审计、零 `StepRecord`**——开相不再是可观测步（`active_phase` 在案，
/// 列表 / 详情直接读该字段）。`attempt` 推导口径：store 内该 `(change_id, phase)`
/// 的 PhaseRecord **全量条数（含 stale）+ 1**，由写面单点推导——walker MUST NOT 自
/// 推导、MUST NOT 取 `detail.pipeline` 条数（后者过滤 stale，回溯会导致 attempt
/// 回退、节点身份键撞号）。该口径与 `phase_next` 的 `round`（窗口口径）是两个
/// 不同口径，MUST NOT 合并。
///
/// 写失败 = store 坏 / 编程错误 → `Terminal::failed`（重启由启动标定处置残留
/// `active_phase`），MUST NOT 虚构 attempt、MUST NOT 进入步循环。
async fn enter_phase(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    phase: &str,
) -> Result<u32, Terminal> {
    let outcome: PhaseStartOutcome = run_tool(
        tools,
        guard,
        &request.root,
        ToolCommand::PhaseStart {
            change_id: request.change_id.clone(),
            phase: phase.to_owned(),
        },
        "phase-start",
        None,
    )
    .await?;
    Ok(outcome.attempt)
}

/// initialize Clean 步（vcs 直调先例同归档 git 步，不经 ToolCommand 枚举）：
/// 主仓双保险（`root == main_root` → `passed` + legacy 跳过因，正常不可达——
/// 相位级短路先行）→ 排除面组装（依赖与工具产物树名单 + dot 目录 + `openspec/`
/// 工件树增补——增补驻调用方，不入名单本体）→ `restore_baseline` 恢复 worktree
/// 基线态（tracked reset、untracked 清除、排除面原样保留）。失败即 Terminal
///（`on_fail=None`：步失败即步终态，不唤决策、不进相位重试）。
async fn step_clean(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::Clean;
    let instance_no = open_step(ctx, trace, cursor, kind);
    if ctx.request.root == ctx.request.main_root {
        close_step(
            ctx,
            cursor,
            kind,
            instance_no,
            StepSettle::passed(Some(TXT_SKIP_LEGACY_INITIALIZE.to_owned())),
        );
        return Ok(());
    }
    let mut exclusions: Vec<String> = foundation::prune::PRUNED_DIR_NAMES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    exclusions.push(".*".to_owned());
    exclusions.push(domain_dir_name().to_owned());
    let exclusion_refs: Vec<&str> = exclusions.iter().map(String::as_str).collect();
    match ctx
        .vcs
        .restore_baseline(Path::new(&ctx.request.root), &exclusion_refs)
    {
        Ok(summary) => {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::passed(Some(summary)),
            );
            Ok(())
        }
        Err(error) => Err(fail_archive_node(
            ctx,
            cursor,
            kind,
            instance_no,
            format!("基线恢复失败: {error}"),
        )),
    }
}

/// 相位收口形态（单入口三形态）：三形态共用同一落账单点（`run_phase_log` +
/// `active_phase` 清位，单事务），零 emit、零审计。
enum PhaseSettle<'a> {
    /// 正常收口：三分流取自 [`PhaseCursor`]（机械双槽 / evaluator checklist / 空
    /// checklist）
    Pass,
    /// 门禁升格与归档步失败：代写 fail checklist
    Fail(PhaseFail<'a>),
    /// legacy initialize 整相位短路：`skipped=true` + 跳过因
    Skipped(&'a str),
}

/// fail 落账载荷（`PhaseSettle::Fail` 的条目面）：`item` 为 checklist 条目标题，
/// `report` ≤ 写面 `MAX_REPORT_CHARS`（装配侧截断），`evidence` 为诊断摘要。
struct PhaseFail<'a> {
    item: &'a str,
    report: String,
    evidence: String,
    executor_session: Option<String>,
}

/// 相位收口（相位机固定动作，非计划步、非图节点）：单入口三形态 → 落账（单事务
/// 含 `active_phase` 清位）→ 回写 `cursor.failed`。**零 emit、零审计**。
///
/// 落账失败保留原 `status`（`Stopped` 不降级为 `Failed`）而按形态重写 `reason`：
/// 节点侧仍是全绿（它们确实都通过了），用户靠 reason 前缀定位「收口未落库」——
/// MUST NOT 与「相位内步骤系统异常」的文案混同（后者是红节点路径）。
async fn settle_phase(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    cursor: &mut PhaseCursor,
    phase: &str,
    settle: PhaseSettle<'_>,
) -> Result<u32, Terminal> {
    // 形态判定与载荷装配（Pass 的三分流按「产物在否」分离，逐字沿用既有落账：
    // test-execution 相位走机械 checklist 双槽 → 评估相位走 evaluator checklist
    // → 二者皆无（archive）走空 checklist，`derive_verdict([]) = Pass`，零
    // `phase == archive` 特判）
    let (label, failed, input) = match settle {
        PhaseSettle::Pass => {
            let (input, failed) = take_phase_log_input(cursor, phase);
            ("全部步通过但落账失败", failed, input)
        }
        PhaseSettle::Fail(fail) => (
            "门禁失败落账失败",
            true,
            PhaseLogInput {
                phase: phase.to_owned(),
                report: fail.report,
                checklist: vec![ChecklistItem {
                    item: fail.item.to_owned(),
                    pass: false,
                    evidence: fail.evidence,
                }],
                skipped: false,
                executor_session_id: fail.executor_session,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        ),
        PhaseSettle::Skipped(reason) => (
            "跳过落账失败",
            false,
            PhaseLogInput {
                phase: phase.to_owned(),
                report: reason.to_owned(),
                checklist: Vec::new(),
                skipped: true,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        ),
    };
    let outcome = run_phase_log(tools, guard, request, input)
        .await
        .map_err(|terminal| Terminal {
            status: terminal.status,
            reason: Some(format!(
                "相位 {phase} {label}: {}",
                terminal.reason.unwrap_or_default()
            )),
        })?;
    cursor.failed = failed;
    Ok(outcome.attempt)
}

/// `PhaseSettle::Pass` 的三分流载荷装配（返回载荷 + 本相位是否 fail）。
///
/// ① `test_outcome` 在场 → 机械 checklist（含 mutation 双槽）；② 否则
/// `evaluator_turn` 在场 → 该 turn 的 checklist；③ 两者皆无（archive）→ 空
/// checklist（`derive_verdict([]) = Pass` 即「相位已 pass」标记，非评估证据面）。
fn take_phase_log_input(cursor: &mut PhaseCursor, phase: &str) -> (PhaseLogInput, bool) {
    if let Some(outcome) = cursor.test_outcome.take() {
        let mutation = cursor.mutation_outcome.take();
        let input = test_execution_phase_log_input(phase, &outcome, mutation.as_ref());
        return (input, false);
    }
    let Some(turn) = cursor.evaluator_turn.take() else {
        return (
            PhaseLogInput {
                phase: phase.to_owned(),
                report: String::new(),
                checklist: Vec::new(),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
            false,
        );
    };
    let failed = turn.checklist.verdict == Verdict::Fail;
    let input = PhaseLogInput {
        phase: phase.to_owned(),
        report: turn.checklist.report,
        checklist: turn.checklist.checklist,
        skipped: false,
        executor_session_id: cursor.executor_session.clone(),
        evaluator_session_id: Some(turn.outcome.session_id),
        decision_session_id: None,
    };
    (input, failed)
}

/// ③ executor 步：本相位 executor 会话（prompt 由写面 phase-next 单点组装——
/// 上下文头 + 回溯重做窗口段 + 失败反馈段全在写面）。
async fn step_executor(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let Some(executor) = &ctx.next.executor else {
        return Err(Terminal::failed(format!(
            "phase \"{}\" 计划步漂移：Executor 步缺 executor 定义",
            ctx.phase
        )));
    };
    let outcome = run_worker(
        ctx.worker,
        ctx.guard,
        ctx.request,
        &ctx.request.root,
        ctx.phase,
        cursor.attempt,
        WorkerRole::Executor,
        executor.prompt.clone(),
        executor.model_level,
        None,
        trace,
        EdgeKind::Forward,
        SettleMode::Immediate,
    )
    .await?;
    cursor.executor_session = Some(outcome.outcome.session_id);
    cursor.executor_level = executor.model_level;
    Ok(())
}

/// ④ static-check 步：门禁 spawn + 定向反馈边（步声明 `OnFail::FeedbackLoop`，
/// 独立计数 ≤ limit）——失败诊断 Continue 注入同一 executor 会话修复重跑（新
/// `instance_no` 节点 + `feedback` 边）；超限升格相位 fail（经相位收口代写 fail
/// checklist，不跑 evaluator），回 phase-next 进重试 / 决策分叉。
///
/// **fault 先于结论**：产出带 `fault` 时早退——`Environment` → 节点 failed +
/// `Terminal::failed`（零 worker 调用、零 fail phase_log、不进决策重试）；`Stopped`
/// → 节点 stopped + run stopped。
async fn step_static_check(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    // initialize 相位声明 `on_fail=Terminal`（基线绿门：失败即停——无反馈边、
    // 零 agent 会话可注入，重试基线无意义）→ 单发执行体；缺省相位走反馈边路径
    // 不受影响
    if step.on_fail == Some(OnFail::Terminal) {
        return step_static_check_terminal(ctx, trace, cursor).await;
    }
    let limit = feedback_limit(step, "static-check")?;
    let mut feedback: u32 = 0;
    loop {
        let instance_no = begin_node(
            ctx.guard,
            trace,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::StaticCheck,
            EdgeKind::Forward,
        );
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::StaticCheck,
            instance_no,
            StepSettle::running(),
        );
        let outcome: StaticCheckOutcome = run_tool(
            ctx.tools,
            ctx.guard,
            &ctx.request.root,
            ToolCommand::StaticCheck {
                change_id: ctx.request.change_id.clone(),
                slot: ReportSlot::Test,
            },
            "static-check",
            Some(StepIdentity {
                phase: ctx.phase.to_owned(),
                attempt: cursor.attempt,
                step: ChangeStepKind::StaticCheck,
                instance_no,
            }),
        )
        .await?;
        // fault 先于结论：环境错误 / 取消不烧反馈边预算（零 executor 会话）、
        // 不升格相位 fail
        if let Some(fault) = outcome.fault.as_ref() {
            return Err(settle_fault(
                ctx.guard,
                ctx.phase,
                cursor.attempt,
                ChangeStepKind::StaticCheck,
                instance_no,
                fault,
                Some(TXT_ENV_STATIC),
                &outcome.report_dir,
                outcome.output_ref.clone(),
            ));
        }
        if outcome.passed {
            emit_node(
                ctx.guard,
                ctx.phase,
                cursor.attempt,
                ChangeStepKind::StaticCheck,
                instance_no,
                StepSettle::passed(Some("静态检查通过".to_owned()))
                    .with_output_ref(outcome.output_ref.clone()),
            );
            return Ok(StepFlow::Proceed);
        }
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::StaticCheck,
            instance_no,
            StepSettle::failed(Some(diagnose_brief(&outcome.diagnostics)))
                .with_output_ref(outcome.output_ref.clone()),
        );
        if feedback >= limit {
            // 升格相位 fail（桌面代写，不跑 evaluator；消耗相位 retry 预算；仅
            // 携 executor 槽位——evaluator 未跑无会话可记）
            step_gate_fail_phase_log(
                ctx.tools,
                ctx.guard,
                ctx.request,
                cursor,
                ctx.phase,
                GateFailLog {
                    item: "静态检查",
                    reason: "静态检查反馈边超限（连续修复未通过），相位升格 fail",
                    diagnostics: &outcome.diagnostics,
                },
            )
            .await?;
            return Ok(StepFlow::Upgraded);
        }
        feedback += 1;
        let fix_prompt = format!(
            "静态检查未通过（第 {feedback}/{limit} 次反馈修复），请修复以下错误后重新提交：\n\n{}",
            outcome.diagnostics
        );
        // 反馈边：失败诊断注入同一 executor 会话（Continue）修复——修复节点为
        // instance_no 递增的独立新节点，边 kind = feedback（phase 内回边）
        let fix = run_worker(
            ctx.worker,
            ctx.guard,
            ctx.request,
            &ctx.request.root,
            ctx.phase,
            cursor.attempt,
            WorkerRole::Executor,
            fix_prompt,
            cursor.executor_level,
            cursor.executor_session.clone(),
            trace,
            EdgeKind::Feedback,
            SettleMode::Immediate,
        )
        .await?;
        cursor.executor_session = Some(fix.outcome.session_id);
    }
}

/// test-execution 相位 TestExecution 步分派（步声明单点）：`on_fail` 三分派——
/// `Terminal`（initialize 基线绿门）→ 单发 terminal 执行体；`Escalate`（本相位
/// 确定性门禁）→ 单发升格执行体（首败落账交决策、零会话）；其余声明 → 计划图
/// 漂移 Err（`FeedbackLoop` 误落到本步立即显式失败，不静默跑反馈边）。
async fn step_test_execution(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    match step.on_fail {
        Some(OnFail::Terminal) => step_test_execution_terminal(ctx, trace, cursor).await,
        Some(OnFail::Escalate) => step_test_execution_escalate(ctx, trace, cursor).await,
        Some(OnFail::FeedbackLoop { .. }) | None => Err(Terminal::failed(format!(
            "计划图漂移：phase \"{}\" 的 test-execution 步缺 OnFail::Terminal / Escalate 声明",
            ctx.phase
        ))),
    }
}

/// test-execution 相位 TestExecution 单发升格执行体（`on_fail=Escalate`）：
/// 单节点 running → spawn（test 槽位）→ pass 收口（挂 `test_outcome` 后
/// Proceed）/ fail 或 error 升格（failed 节点 + 代写 fail 落账 + `Upgraded`
/// 截断余下步回 `phase_next` 进决策分叉，零 WorkerAgent 会话）。runner `Err`
/// 经 `?` 直达 `Terminal`（环境问题不该由决策 agent 裁决，节点留 running 与
/// terminal 分支同式）。
///
/// **fault 先于结论**：`Environment` → 节点 failed + `Terminal::failed`（文案
/// `TXT_ENV_TEST`，零决策分叉、零 fail phase_log）；`Stopped` → 节点 stopped +
/// run stopped。
async fn step_test_execution_escalate(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<StepFlow, Terminal> {
    let instance_no = begin_node(
        ctx.guard,
        trace,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::TestExecution,
        EdgeKind::Forward,
    );
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::TestExecution,
        instance_no,
        StepSettle::running(),
    );
    let outcome: TestExecutionOutcome = run_tool(
        ctx.tools,
        ctx.guard,
        &ctx.request.root,
        ToolCommand::TestExecution {
            change_id: ctx.request.change_id.clone(),
            slot: ReportSlot::Test,
        },
        "test-execution",
        Some(StepIdentity {
            phase: ctx.phase.to_owned(),
            attempt: cursor.attempt,
            step: ChangeStepKind::TestExecution,
            instance_no,
        }),
    )
    .await?;
    // fault 先于结论：环境错误 → 节点 failed + Terminal::failed（零决策分叉、零
    // fail phase_log 代写）；取消 → 节点 stopped + run stopped
    if let Some(fault) = outcome.fault.as_ref() {
        return Err(settle_fault(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::TestExecution,
            instance_no,
            fault,
            Some(TXT_ENV_TEST),
            &outcome.report_dir,
            Some(outcome.exec_id.clone()),
        ));
    }
    if outcome.conclusion == TestExecutionConclusion::Pass {
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::TestExecution,
            instance_no,
            StepSettle::passed(Some(format!(
                "conclusion=pass total={} passed={} failed={} skipped={}",
                outcome.total, outcome.passed, outcome.failed, outcome.skipped
            )))
            .with_output_ref(Some(outcome.exec_id.clone())),
        );
        cursor.test_outcome = Some(outcome);
        return Ok(StepFlow::Proceed);
    }
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::TestExecution,
        instance_no,
        StepSettle::failed(Some(diagnose_brief(&outcome.findings_brief)))
            .with_output_ref(Some(outcome.exec_id.clone())),
    );
    let fail = test_execution_fail(&outcome);
    settle_phase(
        ctx.tools,
        ctx.guard,
        ctx.request,
        cursor,
        ctx.phase,
        PhaseSettle::Fail(fail),
    )
    .await?;
    Ok(StepFlow::Upgraded)
}

/// initialize 相位 static-check 单发执行体（`on_fail=Terminal` 前置分支分派）：
/// 单节点 running → spawn（baseline 槽位）→ pass / fail 收口。pass：`passed` +
/// `output_ref` 回填后 Proceed；fail：failed 节点（detail = 诊断摘要、
/// `output_ref` 回填）+ `Terminal::failed`（static 红文案携 baseline 报告目录
/// ——`outcome.report_dir` 供给 `{dir}` 占位）。
///
/// **fault 先于结论**：环境错误走 `TXT_ENV_STATIC`（引导排除安全软件路径，
/// MUST NOT 落基线红文案），取消走 stopped。
async fn step_static_check_terminal(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<StepFlow, Terminal> {
    let instance_no = begin_node(
        ctx.guard,
        trace,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::StaticCheck,
        EdgeKind::Forward,
    );
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::StaticCheck,
        instance_no,
        StepSettle::running(),
    );
    let outcome: StaticCheckOutcome = run_tool(
        ctx.tools,
        ctx.guard,
        &ctx.request.root,
        ToolCommand::StaticCheck {
            change_id: ctx.request.change_id.clone(),
            slot: ReportSlot::Baseline,
        },
        "static-check",
        Some(StepIdentity {
            phase: ctx.phase.to_owned(),
            attempt: cursor.attempt,
            step: ChangeStepKind::StaticCheck,
            instance_no,
        }),
    )
    .await?;
    // fault 先于结论：基线环境错误 / 取消走环境文案与 stopped，不落基线红文案
    //（不误导用户去修基线）
    if let Some(fault) = outcome.fault.as_ref() {
        return Err(settle_fault(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::StaticCheck,
            instance_no,
            fault,
            Some(TXT_ENV_STATIC),
            &outcome.report_dir,
            outcome.output_ref.clone(),
        ));
    }
    if outcome.passed {
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::StaticCheck,
            instance_no,
            StepSettle::passed(Some("静态检查通过".to_owned()))
                .with_output_ref(outcome.output_ref.clone()),
        );
        return Ok(StepFlow::Proceed);
    }
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::StaticCheck,
        instance_no,
        StepSettle::failed(Some(diagnose_brief(&outcome.diagnostics)))
            .with_output_ref(outcome.output_ref.clone()),
    );
    Err(Terminal::failed(
        TXT_BASELINE_STATIC_FAILED.replace("{dir}", &outcome.report_dir),
    ))
}

/// initialize 相位 test-execution 单发执行体（`on_fail=Terminal` 前置分支分派）：
/// 单节点 running → spawn（baseline 槽位）→ pass / Error / Fail 三态收口。
/// pass：`passed` + 计数 detail + `output_ref` 回填，`cursor.test_outcome` 挂载
///（相位收口机械 checklist 分支天然分派）后 Proceed；Error / Fail：failed 节点
/// + `Terminal::failed` 分文案（环境 / 测试红可辨，携 baseline 报告目录）。
///
/// **fault 先于结论**：环境错误走 `TXT_ENV_TEST`（不落基线红 / 基线错误文案），
/// 取消走 stopped。
async fn step_test_execution_terminal(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<StepFlow, Terminal> {
    let instance_no = begin_node(
        ctx.guard,
        trace,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::TestExecution,
        EdgeKind::Forward,
    );
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::TestExecution,
        instance_no,
        StepSettle::running(),
    );
    let outcome: TestExecutionOutcome = run_tool(
        ctx.tools,
        ctx.guard,
        &ctx.request.root,
        ToolCommand::TestExecution {
            change_id: ctx.request.change_id.clone(),
            slot: ReportSlot::Baseline,
        },
        "test-execution",
        Some(StepIdentity {
            phase: ctx.phase.to_owned(),
            attempt: cursor.attempt,
            step: ChangeStepKind::TestExecution,
            instance_no,
        }),
    )
    .await?;
    // fault 先于结论：基线环境错误 → TXT_ENV_TEST（不落基线红 / 基线错误文案），
    // 取消 → stopped
    if let Some(fault) = outcome.fault.as_ref() {
        return Err(settle_fault(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::TestExecution,
            instance_no,
            fault,
            Some(TXT_ENV_TEST),
            &outcome.report_dir,
            Some(outcome.exec_id.clone()),
        ));
    }
    if outcome.conclusion == TestExecutionConclusion::Pass {
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::TestExecution,
            instance_no,
            StepSettle::passed(Some(format!(
                "conclusion=pass total={} passed={} failed={} skipped={}",
                outcome.total, outcome.passed, outcome.failed, outcome.skipped
            )))
            .with_output_ref(Some(outcome.exec_id.clone())),
        );
        cursor.test_outcome = Some(outcome);
        return Ok(StepFlow::Proceed);
    }
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::TestExecution,
        instance_no,
        StepSettle::failed(Some(diagnose_brief(&outcome.findings_brief)))
            .with_output_ref(Some(outcome.exec_id.clone())),
    );
    let template = match outcome.conclusion {
        TestExecutionConclusion::Error => TXT_BASELINE_TEST_ERROR,
        _ => TXT_BASELINE_TEST_FAILED,
    };
    Err(Terminal::failed(
        template.replace("{dir}", &outcome.report_dir),
    ))
}

/// ④′ mutation-test 步分派（步声明单点）：`Escalate`（本相位确定性门禁）→ 单发
/// 升格执行体（fail / error 两时点同式升格交决策、零会话）；其余声明 → 计划图
/// 漂移 Err（该步无语义上合法的 `Terminal` / `FeedbackLoop` 声明）。
async fn step_mutation_test(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    match step.on_fail {
        Some(OnFail::Escalate) => step_mutation_test_escalate(ctx, trace, cursor).await,
        Some(OnFail::FeedbackLoop { .. }) | Some(OnFail::Terminal) | None => Err(Terminal::failed(
            "计划图漂移：mutation-test 步缺 OnFail::Escalate 声明".to_owned(),
        )),
    }
}

/// mutation-test 单发升格执行体（`on_fail=Escalate`）：确定性突变门禁步——fail /
/// error 两时点同式升格交决策（零会话），达标 pass 直接收口。双时点：全部段跑完
/// 仅未达标、段中断错误随载荷即时收场（两时点在 walker 侧同走 fail 面）。runner
/// 显式 `Err` 停给用户（环境问题不进决策）。
///
/// **fault 先于结论**：`Stopped`（用户停止）→ 节点 stopped + run stopped
///（MUST NOT 收敛为 failed + `conclusion = Error` 的升格面）；mutation 不产
/// `Environment` 面（段超时仍走 `Interrupted` → `conclusion = Error`）。
async fn step_mutation_test_escalate(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<StepFlow, Terminal> {
    let instance_no = begin_node(
        ctx.guard,
        trace,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::MutationTest,
        EdgeKind::Forward,
    );
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::MutationTest,
        instance_no,
        StepSettle::running(),
    );
    let outcome: MutationTestOutcome = match run_tool::<MutationTestOutcome>(
        ctx.tools,
        ctx.guard,
        &ctx.request.root,
        ToolCommand::MutationTest {
            change_id: ctx.request.change_id.clone(),
        },
        "mutation-test",
        Some(StepIdentity {
            phase: ctx.phase.to_owned(),
            attempt: cursor.attempt,
            step: ChangeStepKind::MutationTest,
            instance_no,
        }),
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(terminal) => {
            // runner 显式 Err（工具不可达 / 配置不支持 / 写盘失败）停给用户：
            // 节点呈失败态、run 不静默越过、不产出部分结论
            emit_node(
                ctx.guard,
                ctx.phase,
                cursor.attempt,
                ChangeStepKind::MutationTest,
                instance_no,
                StepSettle::failed(Some("突变执行显式失败（停给用户）".to_owned())),
            );
            return Err(terminal);
        }
    };
    // fault 先于结论：取消 → 节点 stopped + run stopped（MUST NOT 走 fail 升格
    // 面）；mutation 不产 Environment 面，`template = None` 仅防御性退化
    if let Some(fault) = outcome.fault.as_ref() {
        return Err(settle_fault(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::MutationTest,
            instance_no,
            fault,
            None,
            "",
            output_ref(&outcome.exec_id),
        ));
    }
    if outcome.conclusion == MutationTestConclusion::Pass {
        // 达标 / 跳过面（null 观察期不判 fail）：零反馈收口，结论落双槽
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::MutationTest,
            instance_no,
            StepSettle::passed(Some(format!(
                "conclusion={} score={} threshold={}",
                outcome.conclusion.as_str(),
                outcome.score,
                outcome.threshold
            )))
            .with_output_ref(output_ref(&outcome.exec_id)),
        );
        cursor.mutation_outcome = Some(outcome);
        return Ok(StepFlow::Proceed);
    }
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::MutationTest,
        instance_no,
        StepSettle::failed(Some(diagnose_brief(&outcome.feedback_text)))
            .with_output_ref(output_ref(&outcome.exec_id)),
    );
    // 首败升格（桌面代写 fail 落账，不跑 evaluator、零修复会话；三会话槽位恒
    // None），随后回 phase-next 进决策分叉
    let fail = mutation_test_fail(&outcome);
    settle_phase(
        ctx.tools,
        ctx.guard,
        ctx.request,
        cursor,
        ctx.phase,
        PhaseSettle::Fail(fail),
    )
    .await?;
    Ok(StepFlow::Upgraded)
}

/// archi-check 门禁步执行体（`on_fail=FeedbackLoop{3}`）：与 `step_static_check`
/// 逐段同型——fault 先行 / `passed` 收口 / `failed` 进定向反馈边（诊断注入**同一
/// executor 会话**，修复节点为 `instance_no` 递增的独立新节点）/ 超限代写 fail
/// 落账并升格相位 fail。
///
/// **零进程零自动修复**：本 runner 不 spawn、不写源码 / 模型；改代码是 executor
/// 的活，改模型要回 dev-design。报告路径随步 detail 呈现，**MUST NOT** 走节点
/// `output_ref`（该字段是 exec-outputs 回查通道，架构报告是 JSON 文件）。
async fn step_archi_check(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    let limit = feedback_limit(step, "archi-check")?;
    let mut feedback: u32 = 0;
    loop {
        let instance_no = begin_node(
            ctx.guard,
            trace,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::ArchiCheck,
            EdgeKind::Forward,
        );
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::ArchiCheck,
            instance_no,
            StepSettle::running(),
        );
        let outcome: ArchiCheckOutcome = run_tool(
            ctx.tools,
            ctx.guard,
            &ctx.request.root,
            ToolCommand::ArchiCheck {
                change_id: ctx.request.change_id.clone(),
            },
            "archi-check",
            None,
        )
        .await?;
        // fault 先于结论：取消 / 环境错误不烧反馈边预算（零 executor 会话）、
        // 不升格相位 fail。本 runner 零进程 ⇒ 生产路径 `fault` 恒 None，本臂只
        // 作同形保留（`template = None`：消息即 fault 原文，零新增环境文案常量）
        if let Some(fault) = outcome.fault.as_ref() {
            return Err(settle_fault(
                ctx.guard,
                ctx.phase,
                cursor.attempt,
                ChangeStepKind::ArchiCheck,
                instance_no,
                fault,
                None,
                &outcome.report_dir,
                None,
            ));
        }
        if outcome.passed {
            emit_node(
                ctx.guard,
                ctx.phase,
                cursor.attempt,
                ChangeStepKind::ArchiCheck,
                instance_no,
                StepSettle::passed(Some(diagnose_brief(&outcome.diagnostics))),
            );
            return Ok(StepFlow::Proceed);
        }
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::ArchiCheck,
            instance_no,
            StepSettle::failed(Some(diagnose_brief(&outcome.diagnostics))),
        );
        if feedback >= limit {
            // 升格相位 fail（桌面代写，不跑 evaluator；仅携 executor 槽位）
            step_gate_fail_phase_log(
                ctx.tools,
                ctx.guard,
                ctx.request,
                cursor,
                ctx.phase,
                GateFailLog {
                    item: "架构检查",
                    reason: "架构漂移门禁反馈边超限（连续修复未通过），相位升格 fail",
                    diagnostics: &outcome.diagnostics,
                },
            )
            .await?;
            return Ok(StepFlow::Upgraded);
        }
        feedback += 1;
        let fix_prompt = format!(
            "架构漂移门禁未通过（第 {feedback}/{limit} 次反馈修复），请修复以下违例后重新提交：

{}",
            outcome.diagnostics
        );
        // 反馈边：违例诊断注入同一 executor 会话（Continue）修复——修复节点为
        // instance_no 递增的独立新节点，边 kind = feedback（phase 内回边）
        let fix = run_worker(
            ctx.worker,
            ctx.guard,
            ctx.request,
            &ctx.request.root,
            ctx.phase,
            cursor.attempt,
            WorkerRole::Executor,
            fix_prompt,
            cursor.executor_level,
            cursor.executor_session.clone(),
            trace,
            EdgeKind::Feedback,
            SettleMode::Immediate,
        )
        .await?;
        cursor.executor_session = Some(fix.outcome.session_id);
    }
}

/// evaluator 步（含 verdict 裁决内化）：会话以 `SettleMode::Deferred` 跑，先过会
/// 话状态面（系统异常仍落红节点 / 停止节点 + `Terminal`），再过业务面——读
/// `finalized` 的 `FinishPayload::Evaluator(checklist)` 臂即「本步 passed、携
/// verdict」并写 `cursor.evaluator_turn`（会话结果与收口载荷同源）；缺席即与本实
/// 例 failed + `Terminal::failed`（零反馈边重试、零独立门步），成因文案按「输出被
/// 截断」与「未调用 finish_step」两态分列。
///
/// **收口判定先于终态拍**（`Deferred` 的语义）：`Completed` 但无载荷 = 业务失败，
/// MUST NOT 复用 `worker_settle` 的 passed（那会先 emit passed 再被 failed 覆盖）。
async fn step_evaluator(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let Some(evaluator) = &ctx.next.evaluator else {
        return Err(Terminal::failed(format!(
            "phase \"{}\" 计划步漂移：Evaluator 步缺 evaluator 定义",
            ctx.phase
        )));
    };
    let eval_prompt = evaluator_prompt(&evaluator.prompt);
    let turn = run_worker(
        ctx.worker,
        ctx.guard,
        ctx.request,
        &ctx.request.root,
        ctx.phase,
        cursor.attempt,
        WorkerRole::Evaluator,
        eval_prompt,
        evaluator.model_level,
        None,
        trace,
        EdgeKind::Forward,
        SettleMode::Deferred,
    )
    .await?;
    let WorkerTurn {
        mut outcome,
        instance_no,
    } = turn;
    let (settle, terminal) = worker_settle(&outcome);
    if let Some(terminal) = terminal {
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::Evaluator,
            instance_no,
            with_turn(settle, &outcome),
        );
        return Err(terminal);
    }
    let Some(FinishPayload::Evaluator(checklist)) = outcome.finalized.take() else {
        let reason = if truncated(&outcome.transcript) {
            "评估输出被截断（output_truncated），未产生 finish_step 收口产物"
        } else {
            "未调用 finish_step 收口（相位 evaluator 会话 Completed 但无收口载荷）"
        };
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::Evaluator,
            instance_no,
            with_turn(StepSettle::failed(Some(diagnose_brief(reason))), &outcome),
        );
        return Err(Terminal::failed(reason.to_owned()));
    };
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::Evaluator,
        instance_no,
        with_turn(
            StepSettle::passed(Some(format!("verdict={}", checklist.verdict.as_str()))),
            &outcome,
        ),
    );
    cursor.evaluator_turn = Some(EvaluatorTurn { outcome, checklist });
    Ok(())
}

/// 截断成因判定：转录含 `output_truncated` notice 即真（只影响失败文案，不影响
/// 判据；subtype 是开放词典，与 `infra` 侧常量同字面量，零跨 crate 收口）。
fn truncated(transcript: &[agent::AgentEvent]) -> bool {
    transcript.iter().any(|event| {
        matches!(
            &event.kind,
            agent::AgentEventKind::SystemNotice { subtype, .. } if subtype == "output_truncated"
        )
    })
}

/// 门禁步反馈边上限读取（**步声明单点**）：缺声明 / 误声明即计划图漂移——
/// MUST NOT 由 walker 侧常量兜底（反馈边计数上限随门禁 step 声明退役，三门禁
/// 步各自的独立预算不再驻 walker）。
///
/// 本函数只服务 static-check / archi-check 两门禁步；TestExecution /
/// MutationTest 走 `on_fail=Escalate` 的单发执行体，不经反馈计数（Escalate 声明
/// 落在此处即漂移，显式失败）；verdict-gate 的反馈面已随 `finish_step` 收口退役
/// （`finalized` 缺席即终态，零反馈边）。
fn feedback_limit(step: &StepDefinition, label: &str) -> Result<u32, Terminal> {
    match step.on_fail {
        Some(OnFail::FeedbackLoop { limit }) => Ok(limit),
        Some(OnFail::Escalate) => Err(Terminal::failed(format!(
            "计划图漂移：{label} 步不得声明 OnFail::Escalate"
        ))),
        Some(OnFail::Terminal) | None => Err(Terminal::failed(format!(
            "计划图漂移：{label} 步缺 OnFail::FeedbackLoop 声明"
        ))),
    }
}

/// 相位落账单点（写通道唯一：落账经写面进程内直调）；零步状态行 emit——落账是
/// 相位机固定动作，不是可观测步。
async fn run_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    input: PhaseLogInput,
) -> Result<PhaseLogOutcome, Terminal> {
    run_tool(
        tools,
        guard,
        &request.root,
        ToolCommand::PhaseLog {
            change_id: request.change_id.clone(),
            phase: input.phase.clone(),
            input,
        },
        "phase-log",
        None,
    )
    .await
}

/// 门禁反馈边升格落账载荷（`step_gate_fail_phase_log` 的条目面入参）：`item`
/// 为 checklist 条目标题、`reason` 为 report 首句，其余文案与本函数行为无关——
/// 静态检查与架构检查两条反馈边逐字同形，只差这两处文案。
struct GateFailLog<'a> {
    /// checklist 条目标题（如「静态检查」/「架构检查」）
    item: &'a str,
    /// 相位 fail 记因（report 首句）
    reason: &'a str,
    /// 门禁诊断明细（evaluator 未跑时作为该条目 evidence）
    diagnostics: &'a str,
}

/// 门禁反馈边升格落账：装配 [`PhaseFail`] 后交相位收口动作代写 fail checklist
/// （`PhaseSettle::Fail`），随后回 phase-next 进重试 / 决策分叉。fail 条目仅携
/// executor 槽位（反馈边续注同一 executor 会话，id 稳定；evaluator 未跑）。
/// static-check 与 archi-check 两条反馈边共用本单点（文案经 [`GateFailLog`] 参数
/// 化，MUST NOT 复制第二份）。
async fn step_gate_fail_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    cursor: &mut PhaseCursor,
    phase: &str,
    fail: GateFailLog<'_>,
) -> Result<(), Terminal> {
    // 先绑局部变量再传入：同一表达式内既 `&mut cursor` 又读 `cursor.executor_session`
    // 会撞借用检查（签名 MUST NOT 改成 `&PhaseCursor`——那会逼出整包 clone）
    let fail = PhaseFail {
        item: fail.item,
        report: fail.reason.to_owned(),
        evidence: fail.diagnostics.to_owned(),
        executor_session: cursor.executor_session.clone(),
    };
    settle_phase(
        tools,
        guard,
        request,
        cursor,
        phase,
        PhaseSettle::Fail(fail),
    )
    .await?;
    Ok(())
}

/// pass 机械 checklist 代写载荷：三条目全 pass（verdict 由 checklist 全 pass 推
/// 导）——① suite 结论一致 ② 聚合 conclusion 与计数一致 ③ mutation null 自动通
/// 过（判定语义恒 pass——null 观察期不收紧，D8；evidence 三态：跑过且达标 /
/// diff 空跳过 / null（未跑），NoExecutor 并入 null 态附注）；report 携
/// conclusion + 计数 + 报告路径摘要；三会话槽位恒 `None`（绿跑零 agent）。
fn test_execution_phase_log_input(
    phase: &str,
    outcome: &TestExecutionOutcome,
    mutation: Option<&MutationTestOutcome>,
) -> PhaseLogInput {
    PhaseLogInput {
        phase: phase.to_owned(),
        report: format!(
            "测试执行结论 pass：total={} passed={} failed={} skipped={}；报告目录 {}",
            outcome.total, outcome.passed, outcome.failed, outcome.skipped, outcome.report_dir
        ),
        checklist: vec![
            ChecklistItem {
                item: "suite 结论一致".to_owned(),
                pass: true,
                evidence: "各 suite 子报告非 error 且与聚合 conclusion 一致（确定性门禁步校验）"
                    .to_owned(),
            },
            ChecklistItem {
                item: "聚合计数一致".to_owned(),
                pass: true,
                evidence: format!(
                    "total={} passed={} failed={} skipped={}",
                    outcome.total, outcome.passed, outcome.failed, outcome.skipped
                ),
            },
            ChecklistItem {
                item: "mutation null 自动通过".to_owned(),
                pass: true,
                evidence: mutation_evidence(mutation),
            },
        ],
        skipped: false,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
    }
}

/// 机械 checklist 第 3 条 evidence 三态（E8）：`skip=None` → 「跑过且达标（score
/// X ≥ threshold Y）」；`skip=EmptyDiff` → 「diff 空跳过」；`skip=NoExecutor` →
/// 「null（未跑）」并附注「无突变执行器」；`mutation=None` → 「null（未跑）」。
fn mutation_evidence(mutation: Option<&MutationTestOutcome>) -> String {
    match mutation {
        Some(outcome) => match outcome.skip {
            None => format!(
                "跑过且达标（score {} ≥ threshold {}）",
                outcome.score, outcome.threshold
            ),
            Some(MutationSkipReason::EmptyDiff) => "diff 空跳过".to_owned(),
            Some(MutationSkipReason::NoExecutor) => {
                "null（未跑）：全部 suite 无突变执行器".to_owned()
            }
        },
        None => "null（未跑）".to_owned(),
    }
}

/// 升格 report 截断（写面 `phase_log` 硬校验 `report` ≤
/// [`MAX_REPORT_CHARS`]）：不超长原样返回；超长取前 `MAX_REPORT_CHARS - 1` 字符
/// + `…`（总长恰 ≤2000——禁止截 2000 再加省略号，否则 2001 被写面拒绝）。
fn truncate_report(text: &str) -> String {
    if text.chars().count() <= MAX_REPORT_CHARS {
        return text.to_owned();
    }
    let mut truncated: String = text.chars().take(MAX_REPORT_CHARS - 1).collect();
    truncated.push('…');
    truncated
}

/// 突变报告面原文（evidence 的报告目录面）：`report_paths` 为空 → 「（无报告
/// 产物）」，否则逐条路径 `、` 连接、零改写——全路径比「推导共同父目录」更精确
/// 且可直接 `read`。
fn mutation_report_refs(outcome: &MutationTestOutcome) -> String {
    if outcome.report_paths.is_empty() {
        return "（无报告产物）".to_owned();
    }
    outcome.report_paths.join("、")
}

/// 首败升格落账载荷（零会话）：report 携完整明细（≤2000，供决策 prompt 的失败
/// 报告段），evidence 携诊断摘要 + 报告目录；executor 槽位恒 `None`（相位零
/// agent 会话）。
fn test_execution_fail(outcome: &TestExecutionOutcome) -> PhaseFail<'static> {
    // report 携完整明细（空兜底结论行）+ 报告目录；evidence 携诊断摘要 + 报告目录
    let detail = if outcome.findings_detail.trim().is_empty() {
        format!(
            "conclusion={}；报告目录 {}",
            outcome.conclusion.as_str(),
            outcome.report_dir
        )
    } else {
        outcome.findings_detail.clone()
    };
    PhaseFail {
        item: "测试执行",
        report: truncate_report(&detail),
        evidence: format!(
            "{}；报告目录 {}",
            diagnose_brief(&outcome.findings_brief),
            outcome.report_dir
        ),
        executor_session: None,
    }
}

/// 首败升格落账载荷（零会话，两时点同式）：report 携完整反馈文本（≤2000，供决策
/// prompt 的失败报告段），evidence 携诊断摘要 + 报告路径清单；executor 槽位恒
/// `None`。
fn mutation_test_fail(outcome: &MutationTestOutcome) -> PhaseFail<'static> {
    // report 携完整反馈文本（空兜底结论行）+ 报告路径清单；evidence 携诊断摘要 +
    // 报告路径清单（零改写原文，供决策 agent 直读）
    let mutation_refs = mutation_report_refs(outcome);
    let detail = if outcome.feedback_text.trim().is_empty() {
        format!(
            "conclusion={}；报告路径 {}",
            outcome.conclusion.as_str(),
            mutation_refs
        )
    } else {
        outcome.feedback_text.clone()
    };
    PhaseFail {
        item: "突变测试",
        report: truncate_report(&detail),
        evidence: format!(
            "{}；{}",
            diagnose_brief(&outcome.feedback_text),
            mutation_refs
        ),
        executor_session: None,
    }
}

// ---------------------------------------------------------------------------
// 归档相位 9 步执行体（walker 独立 vcs 缝 + 归档两 agent 步；Cleanup 为尽力
// 步例外——失败仅 detail 警告、步恒 passed、链必收口，永不 Terminal；其余步
// 失败一律 Terminal——相位表 `on_fail = None`，不唤决策、不进相位重试）
// ---------------------------------------------------------------------------

/// 步节点起点（与 [`close_step`] 成对）：分配实例号 + 落 incoming 边 + emit 运行
/// 中节点（git / 内联步与条件短路分支共用；agent 步节点由 [`run_worker`] 单点
/// emit）。
fn open_step(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &PhaseCursor,
    kind: ChangeStepKind,
) -> u32 {
    let instance_no = begin_node(
        ctx.guard,
        trace,
        ctx.phase,
        cursor.attempt,
        kind,
        EdgeKind::Forward,
    );
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        kind,
        instance_no,
        StepSettle::running(),
    );
    instance_no
}

/// 步节点收口（passed 携通过 / 跳过摘要，failed 携失败记因；同实例身份键就地
/// 合并）。
fn close_step(
    ctx: &StepContext<'_>,
    cursor: &PhaseCursor,
    kind: ChangeStepKind,
    instance_no: u32,
    settle: StepSettle,
) {
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        kind,
        instance_no,
        settle,
    );
}

/// 归档步前置载荷读取（`(name, worktree)`）：缺位即计划图漂移（Preflight 步未
/// 跑或未识入 `PhaseCursor.preflight`）——显式失败停给用户。
fn archive_context(cursor: &PhaseCursor) -> Result<(String, Option<String>), Terminal> {
    let Some(preflight) = cursor.preflight.as_ref() else {
        return Err(Terminal::failed(
            "archive 相位计划步漂移：缺 Preflight 步前置载荷（PhaseCursor.preflight）".to_owned(),
        ));
    };
    Ok((preflight.name.clone(), preflight.worktree.clone()))
}

/// 归档 Preflight 步（只读校验 + vcs 探测）：载荷经 `ToolStepPort` 单点重算
///（与归档前停等面同一函数），入 `PhaseCursor.preflight` 供后续步消费；worktree
/// 记录在场且目录缺失时以分支合入判定分流（已合入 = 内容已在主仓，照常；未合入
/// 显式引导恢复目录 / 手动合入）。detail 携完成度警告 / delta specs / 合入目标。
async fn step_preflight(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::Preflight;
    let instance_no = open_step(ctx, trace, cursor, kind);
    let payload: ArchivePreflight = match run_tool(
        ctx.tools,
        ctx.guard,
        &ctx.request.main_root,
        ToolCommand::ArchivePreflight {
            change_id: ctx.request.change_id.clone(),
        },
        "archive-preflight",
        None,
    )
    .await
    {
        Ok(payload) => payload,
        Err(terminal) => {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some("归档前置核对失败".to_owned())),
            );
            return Err(terminal);
        }
    };
    if let Some(worktree) = payload.worktree.as_deref() {
        if !Path::new(worktree).is_dir() {
            let branch = payload
                .branch
                .clone()
                .unwrap_or_else(|| change_branch(&payload.name));
            let merged = match ctx
                .vcs
                .branch_merged(Path::new(&ctx.request.main_root), &branch)
            {
                Ok(merged) => merged,
                Err(error) => {
                    let reason = format!("分支合入判定失败: {error}");
                    close_step(
                        ctx,
                        cursor,
                        kind,
                        instance_no,
                        StepSettle::failed(Some(diagnose_brief(&reason))),
                    );
                    return Err(Terminal::failed(reason));
                }
            };
            if !merged {
                let reason = format!(
                    "worktree 目录不存在（可能已被手动删除）: {worktree}；\
                     请恢复目录，或手动将分支 {branch} 合入主仓后重试归档"
                );
                close_step(
                    ctx,
                    cursor,
                    kind,
                    instance_no,
                    StepSettle::failed(Some(diagnose_brief(&reason))),
                );
                return Err(Terminal::failed(reason));
            }
        }
    }
    // 半程挡回（写动作之前）：上一次归档在 ResolveConflicts 以模型冲突失败会保留
    // rebase 半程，用户直接重跑时 Commit 步的 `add -A` + `commit` 会在 rebase 中间
    // 态产生提交——此处置为 Terminal 挡回，后续步不启动（`cursor.preflight` 未赋值）。
    // 目录不在场即不探测（design D13）：worktree 被手动删除且分支已合入时上面的分支
    // 已放行，此处若仍探测，「不是 git 仓库」的探测错误会被 fail-closed 误当成半程，
    // 把合法放行态挡回去。
    if let Some(worktree) = payload
        .worktree
        .as_deref()
        .filter(|path| Path::new(path).is_dir())
    {
        if let Err(reason) = check_rebase_halfway(ctx.vcs.as_ref(), worktree) {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some(diagnose_brief(&reason))),
            );
            return Err(Terminal::failed(reason));
        }
    }
    let brief = preflight_brief(&payload);
    close_step(
        ctx,
        cursor,
        kind,
        instance_no,
        StepSettle::passed(Some(brief)),
    );
    cursor.preflight = Some(payload);
    Ok(())
}

/// Preflight detail 摘要（完成度警告 / 产物缺失 / delta specs 清单 / 合入目标；
/// 全无即「前置校验通过」）。
fn preflight_brief(payload: &ArchivePreflight) -> String {
    let mut notes: Vec<String> = Vec::new();
    if !payload.completed {
        notes.push(if payload.incomplete_phases.is_empty() {
            "完成度不可核算（工作流类型无相位表）".to_owned()
        } else {
            format!("工作流未全部通过：{}", payload.incomplete_phases.join("、"))
        });
    }
    if !payload.missing_artifacts.is_empty() {
        notes.push(format!(
            "缺少产物文档：{}",
            payload.missing_artifacts.join("、")
        ));
    }
    if !payload.delta_specs.is_empty() {
        notes.push(format!("delta specs：{}", payload.delta_specs.join("、")));
    }
    if payload.worktree.is_some() {
        notes.push(format!(
            "合入目标：{}",
            payload
                .merge_target
                .as_deref()
                .unwrap_or("（当前分支探测失败）")
        ));
    }
    if notes.is_empty() {
        return "前置校验通过".to_owned();
    }
    notes.join("；")
}

/// 归档 Commit 步（vcs.commit_all）：**跳过依据仅 worktree 干净**（分支可达性
/// MUST NOT 构成跳过分支——零提交分支 tip = 创建基线，主仓前进即令基线可达
/// HEAD，可达性跳过正是死法根源）；legacy 记录与 worktree 目录缺失（内容已在
/// 主仓）以 `passed` + 跳过因短路。
async fn step_commit(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::Commit;
    let instance_no = open_step(ctx, trace, cursor, kind);
    let (name, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some("缺归档前置载荷".to_owned())),
            );
            return Err(terminal);
        }
    };
    let settle = match worktree {
        None => StepSettle::passed(Some(TXT_SKIP_LEGACY.to_owned())),
        Some(path) if !Path::new(&path).is_dir() => {
            StepSettle::passed(Some(TXT_SKIP_MERGED.to_owned()))
        }
        Some(path) if !ctx.vcs.dirty(Path::new(&path), &[]) => {
            StepSettle::passed(Some(TXT_SKIP_CLEAN.to_owned()))
        }
        Some(path) => {
            let message = format!("archive: {name}");
            match ctx.vcs.commit_all(Path::new(&path), &message) {
                Ok(()) => StepSettle::passed(Some(format!("worktree 已提交（{message}）"))),
                Err(error) => {
                    let reason = format!("worktree 提交失败: {error}");
                    close_step(
                        ctx,
                        cursor,
                        kind,
                        instance_no,
                        StepSettle::failed(Some(diagnose_brief(&reason))),
                    );
                    return Err(Terminal::failed(reason));
                }
            }
        }
    };
    close_step(ctx, cursor, kind, instance_no, settle);
    Ok(())
}

/// 归档 Rebase 步（vcs.rebase_branch）：分支已合入即幂等跳过；重放至主仓当前
/// 分支，产出冲突清单入 `PhaseCursor.rebase_conflicts`（解算归下一步，冲突态保留
/// 于 worktree——MUST NOT 自动 abort）；非冲突失败由 port 面尽力 abort 后携引导
/// 上抛（步失败即 Terminal）。
async fn step_rebase(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::Rebase;
    let instance_no = open_step(ctx, trace, cursor, kind);
    let (name, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some("缺归档前置载荷".to_owned())),
            );
            return Err(terminal);
        }
    };
    let main_root = Path::new(&ctx.request.main_root);
    let settle = match worktree {
        None => {
            cursor.rebase_conflicts = Some(Vec::new());
            StepSettle::passed(Some(TXT_SKIP_LEGACY.to_owned()))
        }
        Some(path) => {
            let branch = change_branch(&name);
            match ctx.vcs.branch_merged(main_root, &branch) {
                Ok(true) => {
                    cursor.rebase_conflicts = Some(Vec::new());
                    StepSettle::passed(Some(TXT_SKIP_MERGED.to_owned()))
                }
                Ok(false) => {
                    let onto = match ctx.vcs.current_branch(main_root) {
                        Ok(onto) => onto,
                        Err(error) => {
                            return Err(fail_archive_node(
                                ctx,
                                cursor,
                                kind,
                                instance_no,
                                format!("合入目标分支探测失败: {error}"),
                            ))
                        }
                    };
                    match ctx.vcs.rebase_branch(Path::new(&path), &onto) {
                        Ok(RebaseOutcome::Rebased) => {
                            cursor.rebase_conflicts = Some(Vec::new());
                            StepSettle::passed(Some(format!("已重放至 {onto}")))
                        }
                        Ok(RebaseOutcome::Conflicted(conflicts)) => {
                            let count = conflicts.len();
                            cursor.rebase_conflicts = Some(conflicts);
                            StepSettle::passed(Some(format!(
                                "{TXT_REBASE_CONFLICTED} {count} 文件"
                            )))
                        }
                        Err(error) => {
                            return Err(fail_archive_node(ctx, cursor, kind, instance_no, error))
                        }
                    }
                }
                Err(error) => {
                    return Err(fail_archive_node(
                        ctx,
                        cursor,
                        kind,
                        instance_no,
                        format!("分支合入判定失败: {error}"),
                    ))
                }
            }
        }
    };
    close_step(ctx, cursor, kind, instance_no, settle);
    Ok(())
}

/// 归档 Merge 步（vcs.ff_merge）：主仓 `merge --ff-only` 纯快进收口（线性历史
/// 零 merge commit）；已合入即幂等跳过；legacy 记录零动作。
async fn step_merge(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::Merge;
    let instance_no = open_step(ctx, trace, cursor, kind);
    let (name, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some("缺归档前置载荷".to_owned())),
            );
            return Err(terminal);
        }
    };
    let main_root = Path::new(&ctx.request.main_root);
    let settle = match worktree {
        None => StepSettle::passed(Some(TXT_SKIP_LEGACY.to_owned())),
        Some(_) => {
            let branch = change_branch(&name);
            match ctx.vcs.branch_merged(main_root, &branch) {
                Ok(true) => StepSettle::passed(Some(TXT_SKIP_MERGED.to_owned())),
                Ok(false) => match ctx.vcs.ff_merge(main_root, &branch) {
                    Ok(()) => StepSettle::passed(Some(format!("主仓已快进 {branch}"))),
                    Err(error) => {
                        return Err(fail_archive_node(ctx, cursor, kind, instance_no, error))
                    }
                },
                Err(error) => {
                    return Err(fail_archive_node(
                        ctx,
                        cursor,
                        kind,
                        instance_no,
                        format!("分支合入判定失败: {error}"),
                    ))
                }
            }
        }
    };
    close_step(ctx, cursor, kind, instance_no, settle);
    Ok(())
}

/// 模型冲突分流的失败文案（worktree 绝对路径 + 冲突文件数与前两个路径 + 「解完
/// 模型冲突后重跑归档（未自动 abort，rebase 半程保留于 worktree）」）；总长
/// ≤ 200 字符（节点 detail 与 `run-error` 同文的截断宽度）。
fn archi_conflict_message(worktree: &str, conflicts: &[String]) -> String {
    let sample: Vec<&str> = conflicts.iter().take(2).map(String::as_str).collect();
    let message = format!(
        "架构模型冲突需人工裁决（不交 agent 解算）：worktree {worktree}；冲突 {} 个（{}）；\
         解完模型冲突后重跑归档（未自动 abort，rebase 半程保留于 worktree）",
        conflicts.len(),
        sample.join("、")
    );
    clip_reason(message)
}

/// 归档步失败收口单点：emit 失败节点（携记因）+ 返回 Terminal。
fn fail_archive_node(
    ctx: &StepContext<'_>,
    cursor: &PhaseCursor,
    kind: ChangeStepKind,
    instance_no: u32,
    reason: String,
) -> Terminal {
    close_step(
        ctx,
        cursor,
        kind,
        instance_no,
        StepSettle::failed(Some(diagnose_brief(&reason))),
    );
    Terminal::failed(reason)
}

/// 归档步失败收口（无既有节点实例时自铸一个）：多轮会话步（ResolveConflicts）
/// 的失败收敛面——agent 会话节点之外另落一条携收敛记因的失败节点，保证「步失败
/// 可见」不因会话内部分收口而失落。
fn fail_archive_step(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &PhaseCursor,
    kind: ChangeStepKind,
    reason: String,
) -> Terminal {
    let instance_no = open_step(ctx, trace, cursor, kind);
    fail_archive_node(ctx, cursor, kind, instance_no, reason)
}

// ---------------------------------------------------------------------------
// 合入冲突解算（Rebase 步产出冲突清单 → 条件执行步）：冲突态保留 → A 快照 →
// 解冲突 agent 会话 → 后验（标记扫描 + B 快照 + 状态面对比）→ walker 代续走；
// 无法裁决一律 lean 收敛（尽力 abort + 步失败 + 冲突摘要与手动裁决引导）。
// ---------------------------------------------------------------------------

/// 归档 ResolveConflicts 步（条件执行）：Rebase 步无冲突即 `passed` + detail 短
/// 路（零会话）；有冲突则解冲突 agent 会话逐批解算，agent 仅编辑冲突清单内文件
/// 并以 pathspec 收口——rebase 收口动词由 walker 代行（agent 无收口权）。
async fn step_resolve_conflicts(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::ResolveConflicts;
    let conflicts = cursor.rebase_conflicts.clone().unwrap_or_default();
    if conflicts.is_empty() {
        // 无冲突（含 legacy 跳过 / 已合入幂等跳过形态）：零 agent 会话
        let instance_no = open_step(ctx, trace, cursor, kind);
        close_step(
            ctx,
            cursor,
            kind,
            instance_no,
            StepSettle::passed(Some(TXT_NO_CONFLICT.to_owned())),
        );
        return Ok(());
    }
    let (name, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => return Err(terminal),
    };
    let Some(worktree) = worktree else {
        // 冲突清单在场却无 worktree 记录 = 计划 / 执行图漂移
        return Err(Terminal::failed(
            "archive 相位漂移：冲突清单在场但 change 无 worktree 记录".to_owned(),
        ));
    };
    let branch = change_branch(&name);
    let mut pending = conflicts;
    // 模型冲突不进 agent 解算（任何快照 / 会话 / git 写动作之前）：架构域路径命中
    // 即整体交人——零 agent 会话、零 `rebase --abort`（半程保留给人工处置，重入风
    // 险由 Preflight 的半程挡回兜住）。混合冲突走模型臂（不先解代码再解模型）
    if pending
        .iter()
        .any(|path| foundation::layout::is_archi_rel_path(path))
    {
        return Err(fail_archive_step(
            ctx,
            trace,
            cursor,
            kind,
            archi_conflict_message(&worktree, &pending),
        ));
    }
    loop {
        let worktree_path = Path::new(&worktree);
        // A 快照（后验对比基面）：探测失败即 lean（解算无从验）
        let baseline = match ctx.vcs.worktree_snapshot(worktree_path) {
            Ok(snapshot) => snapshot,
            Err(_) => {
                return Err(fail_archive_step(
                    ctx,
                    trace,
                    cursor,
                    kind,
                    lean_converge(
                        ctx,
                        worktree_path,
                        &branch,
                        &pending,
                        LEAN_PROBE_FAILED,
                        false,
                    ),
                ))
            }
        };
        let prompt = merge_conflict_prompt(&name, &pending);
        let turn = run_worker(
            ctx.worker,
            ctx.guard,
            ctx.request,
            &worktree,
            ctx.phase,
            cursor.attempt,
            WorkerRole::ResolveConflicts,
            prompt,
            ModelLevel::High,
            None,
            trace,
            EdgeKind::Forward,
            SettleMode::Immediate,
        )
        .await;
        if let Err(terminal) = turn {
            // 会话失败 / 被停止：一律 lean 收敛（abort 恢复 + 引导），停止形态
            // 附「（归档已停止）」注记
            let (reason, stopped_note) = match terminal.status {
                ChangeRunStatus::Stopped => (LEAN_AGENT_STOPPED, true),
                _ => (LEAN_AGENT_FAILED, false),
            };
            return Err(fail_archive_step(
                ctx,
                trace,
                cursor,
                kind,
                lean_converge(ctx, worktree_path, &branch, &pending, reason, stopped_note),
            ));
        }
        // 后验序：残留标记 fs 扫描（locale 免疫）→ B 快照 → 状态面对比
        if !residual_markers(worktree_path, &pending).is_empty() {
            return Err(fail_archive_step(
                ctx,
                trace,
                cursor,
                kind,
                lean_converge(ctx, worktree_path, &branch, &pending, LEAN_RESIDUAL, false),
            ));
        }
        let after = match ctx.vcs.worktree_snapshot(worktree_path) {
            Ok(snapshot) => snapshot,
            Err(_) => {
                return Err(fail_archive_step(
                    ctx,
                    trace,
                    cursor,
                    kind,
                    lean_converge(
                        ctx,
                        worktree_path,
                        &branch,
                        &pending,
                        LEAN_PROBE_FAILED,
                        false,
                    ),
                ))
            }
        };
        if let Err(reason) = verify_resolution(&baseline, &after, &pending) {
            return Err(fail_archive_step(
                ctx,
                trace,
                cursor,
                kind,
                lean_converge(ctx, worktree_path, &branch, &pending, &reason, false),
            ));
        }
        // 链代续走（`rebase --continue` 沿用既定提交信息）：失败不走 lean abort
        //（冲突解已验通过、rebase 态完整，自动回退会丢弃解算成果）
        match ctx.vcs.rebase_continue(worktree_path) {
            Ok(RebaseOutcome::Rebased) => return Ok(()),
            Ok(RebaseOutcome::Conflicted(next)) => pending = next,
            Err(error) => {
                return Err(fail_archive_step(
                    ctx,
                    trace,
                    cursor,
                    kind,
                    format!(
                        "{error}；冲突解算已验通过但 rebase 续走失败：{TXT_REBASE_CONTINUE_GUIDE}"
                    ),
                ))
            }
        }
    }
}

/// lean 档无法裁决收敛（单点）：`rebase_abort` 尽力执行（worktree 恢复重放前
/// 干净态；自身 Err 附注呈现，不掩盖原记因）+ 返回收敛串（lean 串 = 原因词 + 冲突
/// 逐行清单 + abort 告知 + 手动裁决引导 + 重试幂等说明；调用方以
/// [`fail_archive_step`] 落失败节点并终止）。
fn lean_converge(
    ctx: &StepContext<'_>,
    worktree: &Path,
    branch: &str,
    conflicts: &[String],
    reason: &str,
    stopped_note: bool,
) -> String {
    let abort_note = match ctx.vcs.rebase_abort(worktree) {
        Ok(()) => String::new(),
        Err(_) => format!("（{TXT_LEAN_ABORT_FAILED}）"),
    };
    let mut message = format!(
        "合入冲突无法自动裁决（{reason}）。冲突文件 {} 个：{}\n已执行 git rebase --abort \
         恢复 worktree 干净态{abort_note}。请手动将分支 {branch} 合入主仓（在 worktree 内自行\
         rebase 解冲突，或主仓手动 merge）后重试归档——重试将识别已合入并续走收口。",
        conflicts.len(),
        conflicts.join("\n"),
    );
    if stopped_note {
        message.push_str(TXT_LEAN_STOPPED_NOTE);
    }
    message
}

/// 后验状态面对比（纯函数）：① REBASE_HEAD 在场性（rebase 半程判别——agent 违约
/// 收口 / 中止的捕获面）②③ 冲突路径无 stage>0 残留、worktree 干净且索引单条
/// stage-0（全量 staged 才可收口）④ 非冲突路径逐字一致 + B 无 A 缺席的新路径 +
/// 消失路径 ⊆ 冲突清单。
pub(crate) fn verify_resolution(
    baseline: &WorktreeSnapshot,
    after: &WorktreeSnapshot,
    conflicts: &[String],
) -> Result<(), String> {
    // ① REBASE_HEAD 在场性（rebase 半程判别——agent 违约收口 / 中止的捕获面）
    if after.rebase_head.is_none() {
        let baseline_head = &baseline.head;
        return Err(format!(
            "{LEAN_REBASE_CLOSED}——REBASE_HEAD 缺席（rebase 已被收口或中止），\
             请手动核验 worktree git 状态（基线 HEAD {baseline_head}）后重试"
        ));
    }
    let conflict_set: std::collections::HashSet<&str> =
        conflicts.iter().map(String::as_str).collect();
    // ②③ 冲突路径：无 stage>0 残留条目；worktree 干净（y=' '）且索引单条
    // stage-0（或已删除缺席）——全量 staged 才可收口
    for path in conflicts {
        let unmerged = after
            .index
            .iter()
            .any(|entry| entry.path == *path && entry.stage > 0);
        let worktree_dirty = after
            .status
            .iter()
            .any(|entry| entry.path == *path && entry.y != ' ');
        let stage_zero = after
            .index
            .iter()
            .filter(|entry| entry.path == *path && entry.stage == 0)
            .count();
        if unmerged || worktree_dirty || stage_zero > 1 {
            return Err(LEAN_RESIDUAL.to_owned());
        }
    }
    // ④ 非冲突路径逐字一致（porcelain XY 与索引 mode/hash/stage 双面对拍）
    let mut status_a: Vec<(&str, char, char)> = baseline
        .status
        .iter()
        .filter(|entry| !conflict_set.contains(entry.path.as_str()))
        .map(|entry| (entry.path.as_str(), entry.x, entry.y))
        .collect();
    status_a.sort_unstable();
    let mut status_b: Vec<(&str, char, char)> = after
        .status
        .iter()
        .filter(|entry| !conflict_set.contains(entry.path.as_str()))
        .map(|entry| (entry.path.as_str(), entry.x, entry.y))
        .collect();
    status_b.sort_unstable();
    let mut index_a: Vec<(&str, &str, &str, u8)> = baseline
        .index
        .iter()
        .filter(|entry| !conflict_set.contains(entry.path.as_str()))
        .map(|entry| {
            (
                entry.path.as_str(),
                entry.mode.as_str(),
                entry.hash.as_str(),
                entry.stage,
            )
        })
        .collect();
    index_a.sort_unstable();
    let mut index_b: Vec<(&str, &str, &str, u8)> = after
        .index
        .iter()
        .filter(|entry| !conflict_set.contains(entry.path.as_str()))
        .map(|entry| {
            (
                entry.path.as_str(),
                entry.mode.as_str(),
                entry.hash.as_str(),
                entry.stage,
            )
        })
        .collect();
    index_b.sort_unstable();
    if status_a != status_b || index_a != index_b {
        return Err(LEAN_OUTSIDE_CHANGES.to_owned());
    }
    // ④ 续：B 无 A 缺席的新路径（agent 新建 / 新 add 即违约）；消失路径 ⊆
    // 冲突清单（冲突路径解算后收敛至与 HEAD 一致而离开 status 是合法消失）
    let paths_a: std::collections::HashSet<&str> = baseline
        .status
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    if let Some(new_path) = after
        .status
        .iter()
        .map(|entry| entry.path.as_str())
        .find(|path| !paths_a.contains(path))
    {
        return Err(format!("{LEAN_OUTSIDE_CHANGES}: {new_path}"));
    }
    let paths_b: std::collections::HashSet<&str> = after
        .status
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    if let Some(gone_path) = baseline
        .status
        .iter()
        .map(|entry| entry.path.as_str())
        .find(|path| !paths_b.contains(path) && !conflict_set.contains(path))
    {
        return Err(format!("{LEAN_OUTSIDE_CHANGES}: {gone_path}"));
    }
    Ok(())
}

/// 冲突标记 fs 扫描（locale 免疫）：逐清单文件逐行扫描行首 `<<<<<<< ` /
/// `>>>>>>> ` 形态，返回 `文件:行` 命中清单；读取失败 / 文件缺席按无标记处理
///（fs 缺席形态由后验状态面对比承载）。
pub(crate) fn residual_markers(root: &Path, conflicts: &[String]) -> Vec<String> {
    let mut hits = Vec::new();
    for path in conflicts {
        let Ok(content) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        for (offset, line) in content.lines().enumerate() {
            if line.starts_with("<<<<<<< ") || line.starts_with(">>>>>>> ") {
                hits.push(format!("{path}:{}", offset + 1));
            }
        }
    }
    hits
}

/// 归档 SpecSync 步：delta specs 在场且用户未跳过时发起 spec 同步 agent 会话
///（cwd 恒主 workspace root——worktree change 的 delta specs 已随合入进入主仓
/// active 树，同步基线恒主仓当前 specs）；缺席 / 跳过即 `passed` + 跳过因（零
/// 会话）；会话失败 / 被停止即步失败 Terminal（零归档变更，重试自本步续走）。
async fn step_spec_sync(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::SpecSync;
    let (name, _) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => return Err(terminal),
    };
    let delta_specs = cursor
        .preflight
        .as_ref()
        .map(|preflight| preflight.delta_specs.len())
        .unwrap_or(0);
    if delta_specs == 0 {
        let instance_no = open_step(ctx, trace, cursor, kind);
        close_step(
            ctx,
            cursor,
            kind,
            instance_no,
            StepSettle::passed(Some(TXT_SKIP_NO_DELTA.to_owned())),
        );
        return Ok(());
    }
    if !cursor.sync_specs {
        let instance_no = open_step(ctx, trace, cursor, kind);
        close_step(
            ctx,
            cursor,
            kind,
            instance_no,
            StepSettle::passed(Some(TXT_SKIP_BY_USER.to_owned())),
        );
        return Ok(());
    }
    run_worker(
        ctx.worker,
        ctx.guard,
        ctx.request,
        &ctx.request.main_root,
        ctx.phase,
        cursor.attempt,
        WorkerRole::SpecSync,
        spec_sync_prompt(&name),
        ModelLevel::High,
        None,
        trace,
        EdgeKind::Forward,
        SettleMode::Immediate,
    )
    .await?;
    Ok(())
}

/// 归档 Cleanup 步（vcs.remove_worktree 尽力步）：SpecSync 与 Seal 之间清场
///——worktree 记录在案即移除（`--force` 带走 bootstrap 的 `node_modules` /
/// `target`），legacy 与目录缺席（手动清 / 重试链）幂等跳过；移除失败吞为
/// detail 警告——步恒 `passed`、链必收口（归档残留是警告不是回滚，永不
/// `Terminal`）；branch `change/<name>` 全程零触碰。缺 preflight 载荷沿既有
/// 计划漂移面 Terminal（非清理失败面）。
async fn step_cleanup(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::Cleanup;
    let instance_no = open_step(ctx, trace, cursor, kind);
    let (_, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some("缺归档前置载荷".to_owned())),
            );
            return Err(terminal);
        }
    };
    let settle = match worktree {
        None => StepSettle::passed(Some(TXT_SKIP_LEGACY.to_owned())),
        Some(path) if !Path::new(&path).is_dir() => {
            StepSettle::passed(Some(TXT_SKIP_ABSENT.to_owned()))
        }
        Some(path) => {
            match ctx
                .vcs
                .remove_worktree(Path::new(&ctx.request.main_root), Path::new(&path))
            {
                Ok(()) => StepSettle::passed(Some(format!("已移除 worktree（{path}）"))),
                // 尽力步：失败仅 detail 警告（err 已含路径与手动清理指引），
                // 步恒 passed、Seal / Finalize / 相位收口照常续走
                Err(error) => StepSettle::passed(Some(format!(
                    "worktree 清理失败（尽力步，不影响归档收口）: {error}"
                ))),
            }
        }
    };
    close_step(ctx, cursor, kind, instance_no, settle);
    Ok(())
}

/// 归档 Seal 步：写面 `archive` 双写单点复用（目录改名 + db 翻转；半完成重试走
/// 写面既有续半边）；产出交 `PhaseCursor.seal_outcome` 供 Finalize 派生归档目录
/// 路径。
async fn step_seal(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::Seal;
    let instance_no = open_step(ctx, trace, cursor, kind);
    let outcome: ArchiveOutcome = match run_tool(
        ctx.tools,
        ctx.guard,
        &ctx.request.main_root,
        ToolCommand::ArchiveSeal {
            change_id: ctx.request.change_id.clone(),
        },
        "archive-seal",
        None,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(terminal) => {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some("归档双写收口失败".to_owned())),
            );
            return Err(terminal);
        }
    };
    close_step(
        ctx,
        cursor,
        kind,
        instance_no,
        StepSettle::passed(Some(format!("已归档（{}）", outcome.archived_date))),
    );
    cursor.seal_outcome = Some(outcome);
    Ok(())
}

/// 归档 Finalize 步：归档落盘 pathspec 圈定提交（改名两路径 + delta capability
/// 主 specs 子树；全树通配为禁区——无关 specs 的用户未提交编辑 MUST NOT 被吞并；
/// 子树缺席经 fs 门剔除——git add / commit 对 unmatched pathspec fatal）；脏探测
/// 为零即跳过（幂等面）。
async fn step_finalize(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let kind = ChangeStepKind::Finalize;
    let instance_no = open_step(ctx, trace, cursor, kind);
    let name = match archive_context(cursor) {
        Ok((name, _)) => name,
        Err(terminal) => {
            close_step(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some("缺归档前置载荷".to_owned())),
            );
            return Err(terminal);
        }
    };
    let Some(sealed) = cursor.seal_outcome.clone() else {
        let reason = "archive 相位计划步漂移：Finalize 步缺 Seal 步双写产物".to_owned();
        close_step(
            ctx,
            cursor,
            kind,
            instance_no,
            StepSettle::failed(Some(reason.clone())),
        );
        return Err(Terminal::failed(reason));
    };
    let main_root = Path::new(&ctx.request.main_root);
    let domain = domain_dir_name();
    let active_pathspec = format!("{domain}/changes/{name}");
    let archived_dir = format!("{}-{}", sealed.archived_date, name);
    let archived_pathspec = format!("{domain}/changes/archive/{archived_dir}");
    let mut pathspecs = vec![active_pathspec, archived_pathspec.clone()];
    for capability in cursor
        .preflight
        .as_ref()
        .map(|preflight| preflight.delta_specs.clone())
        .unwrap_or_default()
    {
        let spec_path = format!("{domain}/specs/{capability}");
        if main_root.join(&spec_path).is_dir() {
            pathspecs.push(spec_path);
        }
    }
    let pathspec_refs: Vec<&str> = pathspecs.iter().map(String::as_str).collect();
    let settle = if ctx.vcs.dirty(main_root, &pathspec_refs) {
        let message = format!("archive: move {name} to archive");
        match ctx.vcs.commit_paths(main_root, &pathspec_refs, &message) {
            Ok(()) => StepSettle::passed(Some(archived_pathspec)),
            Err(error) => {
                return Err(fail_archive_node(ctx, cursor, kind, instance_no, error));
            }
        }
    } else {
        StepSettle::passed(Some(TXT_SKIP_FINALIZED.to_owned()))
    };
    close_step(ctx, cursor, kind, instance_no, settle);
    Ok(())
}

/// `ToolStepOutput → ArchivePreflight` 窄化（归档前停等门 / Preflight 步消费）。
impl TryFrom<ToolStepOutput> for ArchivePreflight {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::ArchivePreflight(payload) => Ok(payload),
            _ => Err(()),
        }
    }
}

/// `ToolStepOutput → ArchiveOutcome` 窄化（Seal 步消费）。
impl TryFrom<ToolStepOutput> for ArchiveOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::ArchiveSeal(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

// ---------------------------------------------------------------------------
// 决策分叉
// ---------------------------------------------------------------------------

enum Deadlock {
    /// backtrack 已执行（或无需执行），回外层 phase-next。
    Continue,
    Terminal(Terminal),
}

/// phase-next 决策分叉（两触发点：verdict fail 未决 / 重试上限标注——任意带
/// evaluator 相位 fail 均直达决策，自走同相位重跑退役）：决策会话经
/// `finish_step` 单轮收口产出三动作，backtrack 经写面落账后回外层 phase-next
/// 重路由（白名单门在 `decision_session` 内一次即判，越权即终态）。
#[allow(clippy::too_many_arguments)]
async fn resolve_deadlock(
    worker: &Arc<dyn WorkerAgentPort>,
    tools: &Arc<dyn ToolStepPort>,
    snapshot: &Arc<dyn WorkflowSnapshotPort>,
    guard: &RunGuard,
    request: &RunRequest,
    next: &PhaseNextOutcome,
    error: &PhaseNextError,
    allowed: &[BacktrackOption],
    trace: &mut EdgeTrace,
) -> Deadlock {
    let fail_phase = match error {
        PhaseNextError::MaxRetriesExceeded { phase, .. }
        | PhaseNextError::VerdictFail { phase, .. } => phase,
    };
    if next.last_result.is_none() {
        return Deadlock::Terminal(Terminal::failed(
            "phase-next 报决策分叉触发但缺 last_result 快照，无法组装决策输入".to_owned(),
        ));
    }
    let detail = match snapshot.detail(&request.root, &request.change_id) {
        Ok(detail) => detail,
        Err(message) => {
            return Deadlock::Terminal(Terminal::failed(format!("快照读取失败: {message}")))
        }
    };
    let input = build_decision_input(&detail, fail_phase, allowed, &next.backtrack_window);
    let action = match decision_session(worker, tools, guard, request, &input, trace).await {
        Ok(action) => action,
        Err(terminal) => return Deadlock::Terminal(terminal),
    };

    match action {
        DecisionAction::Backtrack { to, reason } => {
            let backtrack: Result<BacktrackOutcome, Terminal> = run_tool(
                tools,
                guard,
                &request.root,
                ToolCommand::Backtrack {
                    change_id: request.change_id.clone(),
                    phase: fail_phase.clone(),
                    input: BacktrackInput {
                        phase: fail_phase.clone(),
                        to,
                        reason,
                        allowed: allowed.iter().map(|option| option.phase.clone()).collect(),
                    },
                },
                "backtrack",
                None,
            )
            .await;
            match backtrack {
                Ok(_) => {
                    // 目标相位首节点 incoming 边 kind = backtrack（跨相位回跳，区
                    // 别于计划序推进）
                    trace.incoming = Some(EdgeKind::Backtrack);
                    Deadlock::Continue
                }
                Err(terminal) => Deadlock::Terminal(terminal),
            }
        }
        DecisionAction::Stop { reason } => Deadlock::Terminal(Terminal {
            status: ChangeRunStatus::Stopped,
            reason: Some(format!("决策 stop: {reason}")),
        }),
        DecisionAction::Ask { .. } => {
            // 生产不可达（decision 形态 enum 已不含 ask）：提问经阻塞工具
            // `AskUserQuestion` 在 turn 内闭合，携出会话外的 ask 到这里即编排漂移
            Deadlock::Terminal(Terminal::failed(TXT_ASK_DRIFT.to_owned()))
        }
    }
}

/// 决策会话：单轮收口为常态（`finish_step` 一调即本 step 结束）。跑一次会话
/// （`SettleMode::Deferred`）→ `decision-log` 幂等挂账（置于动作裁决**之前**
/// ——stop / 驳回路径同样留痕）→ 会话状态面 → 读 `finalized`：
///
/// - 决策动作 → `ensure_backtrack_allowed` 判定后**在同一 decision 节点实例**收
///   口：通过即 passed + 动作摘要（`backtrack → <to>` / `stop：<reason>`），越权
///   即 failed + 拒因（携 `allowed`）并由调用方**一次即** `Terminal::failed`；
/// - `Ask` 载荷 → failed 节点 + `Terminal::failed`（提问已迁出为独立阻塞工具
///   `AskUserQuestion`，该载荷出现即决策漂移；不停等、不开新轮）；
/// - 缺席 → 同实例 failed + `Terminal::failed`（成因文案区分截断 / 未收口）。
///
/// 动作裁决不经独立 gate 节点（`WhitelistGate` / `RetryGate` 新写面零产出），
/// 留痕由节点 `detail` 承载。协议层纠错全部在工具内闭合（is_error 当场回灌），
/// 本函数无软拒环、无跨轮漂移计数；提问经阻塞工具在同一 turn 内闭合，故无第
/// 二轮（循环结构已退役）。
async fn decision_session(
    worker: &Arc<dyn WorkerAgentPort>,
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    input: &DecisionInput,
    trace: &mut EdgeTrace,
) -> Result<DecisionAction, Terminal> {
    let turn = run_worker(
        worker,
        guard,
        request,
        &request.root,
        &input.phase,
        input.attempt,
        WorkerRole::Decision,
        decision_prompt(input),
        // 决策 agent 恒 High 档（重试上限分叉的决策质量面）
        ModelLevel::High,
        // 单轮收口：零续注（提问经阻塞工具在 turn 内闭合，不开新轮）
        None,
        trace,
        EdgeKind::Forward,
        SettleMode::Deferred,
    )
    .await?;
    let WorkerTurn {
        mut outcome,
        instance_no,
    } = turn;
    let session_id = outcome.session_id.clone();
    // 决策会话槽位挂账（写面单点：最新 eval 条目定点改写，幂等覆写）
    let _: DecisionLogOutcome = run_tool(
        tools,
        guard,
        &request.root,
        ToolCommand::DecisionLog {
            change_id: request.change_id.clone(),
            phase: input.phase.clone(),
            session_id: session_id.clone(),
        },
        "decision-log",
        None,
    )
    .await?;
    // 会话状态面：系统异常（Failed / Stopped / Running 漂移）仍落红节点 /
    // 停止节点 + Terminal，MUST NOT 被业务分支吞成 passed
    let (settle, terminal) = worker_settle(&outcome);
    if let Some(terminal) = terminal {
        emit_node(
            guard,
            &input.phase,
            input.attempt,
            ChangeStepKind::Decision,
            instance_no,
            with_turn(settle, &outcome),
        );
        return Err(terminal);
    }
    let action = match outcome.finalized.take() {
        // `Ask` 载荷不在提取面拦（漂移判定单点在下方动作裁决面）
        Some(FinishPayload::Decision(action)) => action,
        Some(FinishPayload::Evaluator(_)) => {
            let reason = "决策会话返回 evaluator 收口载荷（相位表角色漂移）".to_owned();
            emit_node(
                guard,
                &input.phase,
                input.attempt,
                ChangeStepKind::Decision,
                instance_no,
                with_turn(StepSettle::failed(Some(reason.clone())), &outcome),
            );
            return Err(Terminal::failed(reason));
        }
        None => {
            let reason = if truncated(&outcome.transcript) {
                "决策输出被截断（output_truncated），未产生 finish_step 收口产物"
            } else {
                "未调用 finish_step 收口（决策会话 Completed 但无收口载荷）"
            };
            emit_node(
                guard,
                &input.phase,
                input.attempt,
                ChangeStepKind::Decision,
                instance_no,
                with_turn(StepSettle::failed(Some(diagnose_brief(reason))), &outcome),
            );
            return Err(Terminal::failed(reason.to_owned()));
        }
    };
    // 动作裁决写同一 decision 节点实例的 detail（不另立 gate 节点）
    match ensure_backtrack_allowed(&action, &input.phase, &input.allowed) {
        Ok(()) => {
            let detail = match &action {
                DecisionAction::Backtrack { to, .. } => format!("backtrack → {to}"),
                DecisionAction::Stop { reason } => format!("stop：{reason}"),
                // Ask 载荷（历史回放 / 装配漂移）在动作裁决面收口：指路阻塞工具，
                // 不留 running 节点（文案与 `resolve_deadlock` 的同名漂移臂同源）
                DecisionAction::Ask { .. } => {
                    emit_node(
                        guard,
                        &input.phase,
                        input.attempt,
                        ChangeStepKind::Decision,
                        instance_no,
                        with_turn(StepSettle::failed(Some(TXT_ASK_DRIFT.to_owned())), &outcome),
                    );
                    return Err(Terminal::failed(TXT_ASK_DRIFT.to_owned()));
                }
            };
            emit_node(
                guard,
                &input.phase,
                input.attempt,
                ChangeStepKind::Decision,
                instance_no,
                with_turn(StepSettle::passed(Some(detail)), &outcome),
            );
            Ok(action)
        }
        Err(message) => {
            emit_node(
                guard,
                &input.phase,
                input.attempt,
                ChangeStepKind::Decision,
                instance_no,
                with_turn(StepSettle::failed(Some(diagnose_brief(&message))), &outcome),
            );
            Err(Terminal::failed(format!("决策越权：{message}")))
        }
    }
}

/// 决策有界输入组装（fail checklist + 白名单 + 候选 eval report + 失败条目
/// report 原文 + 回溯重做窗口）：`fail_report` 取失败相位最新 attempt 的
/// `report`（升格落账写入的完整明细，空则空串——prompt 侧渲染「（无报告文
/// 本）」）；`backtrack_window` 原样取自写面 error 响应（零 store 读取、零相位
/// 表推导——判据单源在写面）。
fn build_decision_input(
    detail: &ChangeDetail,
    fail_phase: &str,
    allowed: &[BacktrackOption],
    backtrack_window: &[BacktrackWindowEntry],
) -> DecisionInput {
    let station = detail
        .pipeline
        .iter()
        .find(|station| station.phase == fail_phase);
    let latest = station.and_then(|station| station.attempts.last());
    let attempt = latest.and_then(|record| record.attempt).unwrap_or(0);
    let fail_report = latest
        .map(|record| record.report.clone())
        .unwrap_or_default();
    let fail_checklist = latest
        .map(|record| {
            let failing: Vec<ChecklistItem> = record
                .checklist
                .iter()
                .filter(|item| !item.pass)
                .cloned()
                .collect();
            if failing.is_empty() {
                // 全 pass 清单配 fail verdict 的漂移形态：全量随行不虚减
                record.checklist.clone()
            } else {
                failing
            }
        })
        .unwrap_or_default();
    let mut candidates: Vec<CandidateReport> = Vec::new();
    for option in allowed {
        let phase = &option.phase;
        if phase == fail_phase {
            continue;
        }
        if let Some(station) = detail
            .pipeline
            .iter()
            .find(|station| &station.phase == phase)
        {
            if let Some(record) = station.attempts.last() {
                candidates.push(CandidateReport {
                    phase: phase.clone(),
                    verdict: Some(record.verdict),
                    report: Some(record.report.clone()),
                });
            }
        }
    }
    DecisionInput {
        phase: fail_phase.to_owned(),
        attempt,
        fail_checklist,
        fail_report,
        allowed: allowed.to_vec(),
        candidates,
        backtrack_window: backtrack_window.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// 共用执行面
// ---------------------------------------------------------------------------

/// 会话收口时机（`run_worker` 形参）：`Immediate` 由 `run_worker` 自收口（缺省
/// 语义，其余会话步）；`Deferred` 只发 running 拍、终态由调用方在收口载荷判定
/// 之后 emit（evaluator / decision 两会话步——它们的终态取决于 `finalized` 是否
/// 在场，先 emit passed 再发现无载荷会自相矛盾）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettleMode {
    Immediate,
    Deferred,
}

/// 会话轮产出（`Deferred` 调用方据 `instance_no` 在同实例上补终态拍）。
struct WorkerTurn {
    outcome: WorkerTurnOutcome,
    instance_no: u32,
}

/// 会话状态面分类单点（`Completed` → passed 无终态；`Failed` / `Stopped` /
/// `Running` 漂移 → 对应终态拍 + `Terminal`）。`Immediate` 与 `Deferred` 两路
/// 共用同一份判据与文案，MUST NOT 出现第二份。
fn worker_settle(outcome: &WorkerTurnOutcome) -> (StepSettle, Option<Terminal>) {
    match outcome.status {
        AgentRunStatus::Completed => (StepSettle::passed(None), None),
        AgentRunStatus::Failed => {
            // 失败成因随行（api_error 记因 / TurnDone subtype 兜底）：步 detail
            // 与 run 终态同文携真因；缺席回落通用文案
            let reason = match failure_reason(&outcome.transcript) {
                Some(cause) => format!("会话失败收敛: {}", diagnose_brief(&cause)),
                None => "会话失败收敛".to_owned(),
            };
            (
                StepSettle::failed(Some(reason.clone())),
                Some(Terminal::failed(reason)),
            )
        }
        AgentRunStatus::Stopped => (
            StepSettle::stopped(Some("会话被终止".to_owned())),
            Some(Terminal::stopped("会话被终止")),
        ),
        AgentRunStatus::Running => {
            let reason = "会话收口缺终态".to_owned();
            (
                StepSettle::failed(Some(reason.clone())),
                Some(Terminal::failed(reason)),
            )
        }
    }
}

/// 本步 token 增量单点：对 `transcript` 的 TurnDone usage 求 in / out 两键之和
///（`await` 收口后 transcript 已含 TurnDone——终态单次落值即会话结束后累计结果，
/// 无二次刷新通道）；usage 缺席 → None。
fn worker_tokens(outcome: &WorkerTurnOutcome) -> (Option<u64>, Option<u64>) {
    (
        sum_usage_tokens(&outcome.transcript, "input_tokens"),
        sum_usage_tokens(&outcome.transcript, "output_tokens"),
    )
}

/// 会话终态拍组装：状态面 settle 随行会话 id 与 token 两值（[`worker_tokens`] 的
/// 单点消费面）。`Immediate` 的 `run_worker` 与 `Deferred` 的各调用方共用此一处，
/// MUST NOT 在调用侧再手写 `.with_session(..).with_tokens(..)`。
fn with_turn(settle: StepSettle, outcome: &WorkerTurnOutcome) -> StepSettle {
    let (token_input, token_output) = worker_tokens(outcome);
    settle
        .with_session(Some(outcome.session_id.clone()))
        .with_tokens(token_input, token_output)
}

/// WorkerAgent 会话执行（三类角色统一通道）：bypassPermissions 恒档、
/// provenance（change id / 相位 / 角色 / attempt 四段；身份段恒 change id）、取消与
/// 会话失败收敛终态。
/// 步状态随行 emit（running → passed / failed / stopped，同实例身份键就地合并）；
/// 会话 id 槽由命令层 sink 桥随首个会话事件先行同步（停止寻址不依赖 turn 收口）。
///
/// `Immediate` 与既有语义逐字等价；`Deferred` 成功路径零终态 emit，返回
/// [`WorkerTurn`] 交调用方收口（`worker.run` 的 `Err` 臂两模式都发 failed 节点并
/// `Err`）。
#[allow(clippy::too_many_arguments)]
async fn run_worker(
    worker: &Arc<dyn WorkerAgentPort>,
    guard: &RunGuard,
    request: &RunRequest,
    root: &str,
    phase: &str,
    attempt: u32,
    role: WorkerRole,
    prompt: String,
    model_level: ModelLevel,
    continue_session: Option<String>,
    trace: &mut EdgeTrace,
    incoming: EdgeKind,
    mode: SettleMode,
) -> Result<WorkerTurn, Terminal> {
    let kind = match role {
        WorkerRole::Executor => ChangeStepKind::Executor,
        WorkerRole::Evaluator => ChangeStepKind::Evaluator,
        WorkerRole::Decision => ChangeStepKind::Decision,
        WorkerRole::SpecSync => ChangeStepKind::SpecSync,
        WorkerRole::ResolveConflicts => ChangeStepKind::ResolveConflicts,
    };
    let instance_no = begin_node(guard, trace, phase, attempt, kind, incoming);
    emit_node(
        guard,
        phase,
        attempt,
        kind,
        instance_no,
        StepSettle::running(),
    );
    let turn = WorkerTurnRequest {
        root: root.to_owned(),
        prompt,
        provenance: SessionProvenance {
            source: SOURCE_CHANGE.to_owned(),
            source_ref: Some(format!(
                "{}/{}/{}/{}",
                request.change_id,
                phase,
                role.as_str(),
                attempt
            )),
        },
        permission: AgentPermissionMode::BypassPermissions,
        model_level,
        continue_session,
        agent: None,
        role,
    };
    let outcome = match worker.run(turn).await {
        Ok(outcome) => outcome,
        Err(error) => {
            emit_node(
                guard,
                phase,
                attempt,
                kind,
                instance_no,
                StepSettle::failed(Some(diagnose_brief(&error))),
            );
            return Err(Terminal::failed(format!("会话执行失败: {error}")));
        }
    };
    if mode == SettleMode::Deferred {
        return Ok(WorkerTurn {
            outcome,
            instance_no,
        });
    }
    let (settle, terminal) = worker_settle(&outcome);
    emit_node(
        guard,
        phase,
        attempt,
        kind,
        instance_no,
        with_turn(settle, &outcome),
    );
    match terminal {
        None => Ok(WorkerTurn {
            outcome,
            instance_no,
        }),
        Some(terminal) => Err(terminal),
    }
}

/// 脚本步的输出引用上抛身份（`run_tool` 组装回调的四段——与 `node_key` 同源；
/// 缺席即老行为：running 拍不带输出引用）。
struct StepIdentity {
    phase: String,
    attempt: u32,
    step: ChangeStepKind,
    instance_no: u32,
}

/// 终态拍的输出引用：空串（记录面降级 / 跳过面）→ `None`，否则原样回填。
fn output_ref(exec_id: &str) -> Option<String> {
    (!exec_id.is_empty()).then(|| exec_id.to_owned())
}

/// 工具步执行：产出按封闭集窄化（漂移显式失败）；步失败 `Err` 串映射终态。
///
/// `guard` 是取消信号的**单点取值来源**：`ToolStepRequest.cancel` 只在此处经
/// `guard.cancel_flag()` 填充（各步函数不各自取值），使停止信号在步边界
/// [`check_cancel`] 之外贯通到进程面——运行中的检查域 spawn 被
/// `exec::run_shell` 的 200 ms 轮询观测并杀树收敛。
///
/// `identity` 是输出引用上抛身份的**单点组装面**：`Some`（四位脚本步）时以
/// [`RunGuard::emitter`] 组装 `'static` 的 [`OutputRefSink`]，回调内补发一拍带
/// `output_ref` 的 running 步状态；`None`（其余工具步）时 `on_output_ref` 缺席，
/// 老行为逐字节不变。
async fn run_tool<V>(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    root: &str,
    command: ToolCommand,
    label: &str,
    identity: Option<StepIdentity>,
) -> Result<V, Terminal>
where
    V: TryFrom<ToolStepOutput>,
{
    let on_output_ref = identity.map(|identity| {
        let emitter = guard.emitter();
        OutputRefSink::new(move |exec_id| {
            emitter.attach_output_ref(
                &identity.phase,
                identity.attempt,
                identity.step,
                identity.instance_no,
                exec_id,
            );
        })
    });
    let output = match tools
        .run(crate::port::ToolStepRequest {
            root: root.to_owned(),
            command,
            cancel: guard.cancel_flag(),
            on_output_ref,
        })
        .await
    {
        Ok(output) => output,
        Err(error) => return Err(Terminal::failed(format!("{label} 失败: {error}"))),
    };
    V::try_from(output)
        .map_err(|_| Terminal::failed(format!("{label} 输出漂移：步命令与产出类型不匹配")))
}

/// 取消观测（步边界）：取消信号置位 → 受控 stopped。
///
/// 本函数只在循环顶部一拍（一步边界观测）；**运行中的检查步不等步结束**——
/// `run_tool` 把同一 run 取消标志经 `ToolStepRequest.cancel` 透传到执行体，
/// `exec::run_shell` 以 200 ms 轮询观测并杀进程树收敛。
fn check_cancel(guard: &RunGuard) -> Result<(), Terminal> {
    if guard.cancelled() {
        Err(Terminal::stopped("用户请求停止"))
    } else {
        Ok(())
    }
}

/// 诊断文本人读摘要（步 detail 面截断，全量仍在会话转录 / 诊断文本内）。
fn diagnose_brief(text: &str) -> String {
    const BRIEF_LIMIT: usize = 200;
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= BRIEF_LIMIT {
        collapsed
    } else {
        format!(
            "{}…",
            collapsed.chars().take(BRIEF_LIMIT).collect::<String>()
        )
    }
}

/// 执行面归因分派单点（`fault` 先于结论）：五处门禁步执行体在 `run_tool` 之后、
/// 既有 `passed` / `conclusion` 分支**之前**早退到本函数。
///
/// - `Stopped` → 节点 `stopped` + `Terminal::stopped`（run 收敛 `stopped`，MUST NOT
///   呈现为失败）；
/// - `Environment(text)` → 节点 `failed` + `Terminal::failed`（`template` 非空时把
///   `{fault}` / `{dir}` 展开），**且 MUST NOT 升格相位 fail / MUST NOT 进决策重试 /
///   MUST NOT 注回 executor**——环境坏的相位重试必然再失败，反馈预算零消耗。
#[allow(clippy::too_many_arguments)]
fn settle_fault(
    guard: &RunGuard,
    phase: &str,
    attempt: u32,
    step: ChangeStepKind,
    instance_no: u32,
    fault: &RunFault,
    template: Option<&str>,
    report_dir: &str,
    output_ref: Option<String>,
) -> Terminal {
    match fault {
        RunFault::Stopped => {
            emit_node(
                guard,
                phase,
                attempt,
                step,
                instance_no,
                StepSettle::stopped(Some("用户请求停止".to_owned())).with_output_ref(output_ref),
            );
            Terminal::stopped("用户请求停止")
        }
        RunFault::Environment(text) => {
            let message = template
                .map(|template| {
                    template
                        .replace("{fault}", text)
                        .replace("{dir}", report_dir)
                })
                .unwrap_or_else(|| text.clone());
            emit_node(
                guard,
                phase,
                attempt,
                step,
                instance_no,
                StepSettle::failed(Some(diagnose_brief(&message))).with_output_ref(output_ref),
            );
            Terminal::failed(message)
        }
    }
}

// ToolStepOutput → 各封闭产出的窄化转换（run_tool 泛型的 TryFrom 半边）。
impl TryFrom<ToolStepOutput> for PhaseNextOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::PhaseNext(outcome) => Ok(*outcome),
            _ => Err(()),
        }
    }
}

impl TryFrom<ToolStepOutput> for PhaseStartOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::PhaseStart(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

impl TryFrom<ToolStepOutput> for PhaseLogOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::PhaseLog(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

impl TryFrom<ToolStepOutput> for workflow::write::BacktrackOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::Backtrack(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

impl TryFrom<ToolStepOutput> for DecisionLogOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::DecisionLog(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

impl TryFrom<ToolStepOutput> for StaticCheckOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::StaticCheck(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

impl TryFrom<ToolStepOutput> for TestExecutionOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::TestExecution(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

impl TryFrom<ToolStepOutput> for MutationTestOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::MutationTest(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

impl TryFrom<ToolStepOutput> for ArchiCheckOutcome {
    type Error = ();
    fn try_from(value: ToolStepOutput) -> Result<Self, Self::Error> {
        match value {
            ToolStepOutput::ArchiCheck(outcome) => Ok(outcome),
            _ => Err(()),
        }
    }
}

/// 同 crate 测试探针（`#[cfg(test)]`）：`enter_phase` 的重入幂等与 attempt 口径
/// 经 `walk_run` 整链不可观测（整链两次同相开相之间恒隔一次落账），故以薄包装
/// 返回 `Result<u32, String>` 供 `walker_test` 直调——不泄漏私有 `Terminal`，生产
/// 编译零参与（无 export 面变化）。
#[cfg(test)]
pub(crate) async fn enter_phase_for_test(
    tools: &std::sync::Arc<dyn crate::port::ToolStepPort>,
    guard: &crate::control::RunGuard,
    request: &RunRequest,
    phase: &str,
) -> Result<u32, String> {
    enter_phase(tools, guard, request, phase)
        .await
        .map_err(|terminal| terminal.reason.unwrap_or_default())
}

/// 相位收口形态探针入参（`PhaseSettle` 三形态外部不可构造，测试经本枚举
/// 表达意图）。
#[cfg(test)]
pub(crate) enum SettleProbe {
    Pass {
        evaluator_turn: Option<Box<(WorkerTurnOutcome, EvaluatorChecklist)>>,
        test_outcome: Option<Box<TestExecutionOutcome>>,
        mutation_outcome: Option<Box<MutationTestOutcome>>,
        executor_session: Option<String>,
    },
    Fail {
        item: &'static str,
        report: String,
        evidence: String,
        executor_session: Option<String>,
    },
    Skipped {
        reason: &'static str,
    },
}

/// 相位收口探针产出：返回值 + `cursor.failed` 回写面 + `evaluator_turn` 消费面。
#[cfg(test)]
pub(crate) struct SettleProbeView {
    pub result: Result<u32, String>,
    pub failed: bool,
    pub evaluator_turn_taken: bool,
    pub terminal_status: Option<ChangeRunStatus>,
}

#[cfg(test)]
fn cursor_for_probe(
    evaluator_turn: Option<EvaluatorTurn>,
    test_outcome: Option<TestExecutionOutcome>,
    mutation_outcome: Option<MutationTestOutcome>,
    executor_session: Option<String>,
) -> PhaseCursor {
    PhaseCursor {
        attempt: 0,
        executor_session,
        executor_level: ModelLevel::Low,
        evaluator_turn,
        test_outcome,
        mutation_outcome,
        preflight: None,
        rebase_conflicts: None,
        sync_specs: true,
        seal_outcome: None,
        failed: false,
    }
}

/// 相位收口直调探针：三形态逐分支可达（`walk_run` 整链内 `Pass` 的两条机械 /
/// evaluator 分支依赖上下文产物、`Fail` / `Skipped` 的落账失败前缀不可注入，故
/// 需绕步循环直取）。
#[cfg(test)]
pub(crate) async fn settle_phase_probe(
    tools: &std::sync::Arc<dyn crate::port::ToolStepPort>,
    guard: &crate::control::RunGuard,
    request: &RunRequest,
    phase: &str,
    probe: SettleProbe,
) -> SettleProbeView {
    let (mut cursor, settle) = match probe {
        SettleProbe::Pass {
            evaluator_turn,
            test_outcome,
            mutation_outcome,
            executor_session,
        } => (
            cursor_for_probe(
                evaluator_turn.map(|turn| {
                    let (outcome, checklist) = *turn;
                    EvaluatorTurn { outcome, checklist }
                }),
                test_outcome.map(|outcome| *outcome),
                mutation_outcome.map(|outcome| *outcome),
                executor_session,
            ),
            PhaseSettle::Pass,
        ),
        SettleProbe::Fail {
            item,
            report,
            evidence,
            executor_session,
        } => (
            cursor_for_probe(None, None, None, executor_session.clone()),
            PhaseSettle::Fail(PhaseFail {
                item,
                report,
                evidence,
                executor_session,
            }),
        ),
        SettleProbe::Skipped { reason } => (
            cursor_for_probe(None, None, None, None),
            PhaseSettle::Skipped(reason),
        ),
    };
    let had_turn = cursor.evaluator_turn.is_some();
    let outcome = settle_phase(tools, guard, request, &mut cursor, phase, settle).await;
    let (result, terminal_status) = match outcome {
        Ok(attempt) => (Ok(attempt), None),
        Err(terminal) => (
            Err(terminal.reason.unwrap_or_default()),
            Some(terminal.status),
        ),
    };
    SettleProbeView {
        result,
        failed: cursor.failed,
        evaluator_turn_taken: had_turn && cursor.evaluator_turn.is_none(),
        terminal_status,
    }
}

/// 会话状态面分类探针产出（`StepSettle` 字段私有，故经 `emit_node` 落一拍后从
/// 快照面读取终态行）。
#[cfg(test)]
pub(crate) struct WorkerSettleProbeView {
    pub step_status: Option<crate::state::ChangeStepStatus>,
    pub detail: Option<String>,
    pub session_id: Option<String>,
    pub token_input: Option<u64>,
    pub token_output: Option<u64>,
    pub terminal_status: Option<ChangeRunStatus>,
    pub terminal_reason: Option<String>,
}

/// 会话状态面分类探针（四臂逐文案 + 会话 id / token 随行）。
#[cfg(test)]
pub(crate) fn worker_settle_probe(outcome: &WorkerTurnOutcome) -> WorkerSettleProbeView {
    let control = std::sync::Arc::new(crate::control::ChangeFlowControl::new(std::sync::Arc::new(
        agent::AskRegistry::default(),
    )));
    let guard = control
        .begin_run(PROBE_ROOT, PROBE_CHANGE, "probe-run".to_owned(), 0)
        .expect("探针登记 run 应成功");
    let (settle, terminal) = worker_settle(outcome);
    emit_node(
        &guard,
        "probe",
        1,
        ChangeStepKind::Evaluator,
        0,
        with_turn(settle, outcome),
    );
    let node = guard.nodes().into_iter().next();
    WorkerSettleProbeView {
        step_status: node.as_ref().map(|node| node.status),
        detail: node.as_ref().and_then(|node| node.detail.clone()),
        session_id: node.as_ref().and_then(|node| node.session_id.clone()),
        token_input: node.as_ref().and_then(|node| node.token_input),
        token_output: node.as_ref().and_then(|node| node.token_output),
        terminal_status: terminal.as_ref().map(|terminal| terminal.status),
        terminal_reason: terminal
            .as_ref()
            .and_then(|terminal| terminal.reason.clone()),
    }
}

#[cfg(test)]
const PROBE_ROOT: &str = "/probe-root";
#[cfg(test)]
const PROBE_CHANGE: &str = "probe-change";

/// token 求和单点探针（in / out 两键）。
#[cfg(test)]
pub(crate) fn worker_tokens_probe(outcome: &WorkerTurnOutcome) -> (Option<u64>, Option<u64>) {
    worker_tokens(outcome)
}

/// 会话执行探针：返回（会话产出，实例号）——`Deferred` 调用方据实例号在同实例
/// 补终态拍的断言面。
#[cfg(test)]
pub(crate) async fn run_worker_probe(
    worker: &std::sync::Arc<dyn crate::port::WorkerAgentPort>,
    guard: &crate::control::RunGuard,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    role: crate::port::WorkerRole,
    mode: SettleMode,
) -> Result<(WorkerTurnOutcome, u32), String> {
    let mut trace = EdgeTrace::default();
    let turn = run_worker(
        worker,
        guard,
        request,
        PROBE_ROOT,
        phase,
        attempt,
        role,
        "探针 prompt".to_owned(),
        ModelLevel::Low,
        None,
        &mut trace,
        EdgeKind::Forward,
        mode,
    )
    .await
    .map_err(|terminal| terminal.reason.unwrap_or_default())?;
    Ok((turn.outcome, turn.instance_no))
}

/// 升格失败载荷装配探针视图（`test_execution_fail` / `mutation_test_fail` 的
/// 字段面；两函数产物不进图节点，需直取）。
#[cfg(test)]
pub(crate) struct PhaseFailProbeView {
    pub item: String,
    pub report: String,
    pub evidence: String,
    pub executor_session: Option<String>,
}

#[cfg(test)]
pub(crate) fn test_execution_fail_probe(outcome: &TestExecutionOutcome) -> PhaseFailProbeView {
    let fail = test_execution_fail(outcome);
    PhaseFailProbeView {
        item: fail.item.to_owned(),
        report: fail.report,
        evidence: fail.evidence,
        executor_session: fail.executor_session,
    }
}

#[cfg(test)]
pub(crate) fn mutation_test_fail_probe(outcome: &MutationTestOutcome) -> PhaseFailProbeView {
    let fail = mutation_test_fail(outcome);
    PhaseFailProbeView {
        item: fail.item.to_owned(),
        report: fail.report,
        evidence: fail.evidence,
        executor_session: fail.executor_session,
    }
}

/// `PhaseCursor::new` 中性值探针（字段私有，经视图读出）。
#[cfg(test)]
pub(crate) struct CursorNewProbeView {
    pub attempt: u32,
    pub executor_session: Option<String>,
    pub evaluator_turn: Option<()>,
    pub test_outcome: Option<()>,
    pub mutation_outcome: Option<()>,
    pub sync_specs: bool,
    pub failed: bool,
    pub executor_level_default: bool,
}

#[cfg(test)]
pub(crate) fn phase_cursor_new_probe(
    next: &PhaseNextOutcome,
    sync_specs: bool,
) -> CursorNewProbeView {
    let cursor = PhaseCursor::new(next, sync_specs);
    CursorNewProbeView {
        attempt: cursor.attempt,
        executor_session: cursor.executor_session,
        evaluator_turn: cursor.evaluator_turn.as_ref().map(|_| ()),
        test_outcome: cursor.test_outcome.as_ref().map(|_| ()),
        mutation_outcome: cursor.mutation_outcome.as_ref().map(|_| ()),
        sync_specs: cursor.sync_specs,
        failed: cursor.failed,
        executor_level_default: cursor.executor_level == ModelLevel::default(),
    }
}

/// 步节点起收成对探针（`open_step` / `close_step` 语义）：返回两次 open 的实例号
/// （同名步单点递增，自 0 起）。
#[cfg(test)]
pub(crate) fn open_close_step_probe(
    worker: &std::sync::Arc<dyn crate::port::WorkerAgentPort>,
    tools: &std::sync::Arc<dyn crate::port::ToolStepPort>,
    vcs: &std::sync::Arc<dyn crate::port::ArchiveVcsPort>,
    guard: &crate::control::RunGuard,
    request: &RunRequest,
    next: &PhaseNextOutcome,
    phase: &str,
) -> (u32, u32) {
    let ctx = StepContext {
        worker,
        tools,
        vcs,
        guard,
        request,
        next,
        phase,
    };
    let mut trace = EdgeTrace::default();
    let cursor = PhaseCursor {
        attempt: 1,
        ..cursor_for_probe(None, None, None, None)
    };
    let first = open_step(&ctx, &mut trace, &cursor, ChangeStepKind::Commit);
    close_step(
        &ctx,
        &cursor,
        ChangeStepKind::Commit,
        first,
        StepSettle::passed(Some("提交完成".to_owned())),
    );
    let second = open_step(&ctx, &mut trace, &cursor, ChangeStepKind::Commit);
    close_step(
        &ctx,
        &cursor,
        ChangeStepKind::Commit,
        second,
        StepSettle::passed(None),
    );
    (first, second)
}

/// 未成对 `close_step` 探针：`instance_no` 未由 `open_step` 开出（调用方不变量
/// 由成对书写保证），仍按身份键就地合并且不 panic——返回（节点数，边数）。
#[cfg(test)]
pub(crate) fn close_step_unopened_probe(
    worker: &std::sync::Arc<dyn crate::port::WorkerAgentPort>,
    tools: &std::sync::Arc<dyn crate::port::ToolStepPort>,
    vcs: &std::sync::Arc<dyn crate::port::ArchiveVcsPort>,
    guard: &crate::control::RunGuard,
    request: &RunRequest,
    next: &PhaseNextOutcome,
    phase: &str,
) -> (usize, usize) {
    let ctx = StepContext {
        worker,
        tools,
        vcs,
        guard,
        request,
        next,
        phase,
    };
    let cursor = PhaseCursor {
        attempt: 1,
        ..cursor_for_probe(None, None, None, None)
    };
    close_step(
        &ctx,
        &cursor,
        ChangeStepKind::Commit,
        7,
        StepSettle::passed(Some("未成对收口".to_owned())),
    );
    close_step(
        &ctx,
        &cursor,
        ChangeStepKind::Commit,
        7,
        StepSettle::failed(Some("同键再收口".to_owned())),
    );
    (guard.nodes().len(), guard.edges().len())
}
