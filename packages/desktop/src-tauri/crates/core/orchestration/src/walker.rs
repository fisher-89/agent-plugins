use std::path::Path;
use std::sync::Arc;

use agent::{sum_usage_tokens, AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance};
use foundation::layout::{domain_dir_name, ReportSlot};
use workflow::model::{ChecklistItem, Verdict};
use workflow::queries::ChangeDetail;
use workflow::write::{
    ArchiveOutcome, BacktrackInput, BacktrackOption, BacktrackOutcome, DecisionLogOutcome, OnFail,
    PhaseLogInput, PhaseLogOutcome, PhaseNextError, PhaseNextOutcome, PhaseStartOutcome,
    StepDefinition, StepNode, INITIALIZE_PHASE_ID,
};

use crate::control::{ChangeFlowControl, RunGuard};
use crate::decision::{
    ensure_backtrack_allowed, parse_decision, CandidateReport, DecisionAction, DecisionInput,
};
use crate::port::{
    ArchiveVcsPort, MutationSkipReason, MutationTestConclusion, MutationTestOutcome, RebaseOutcome,
    RunHistoryPort, StaticCheckOutcome, TestExecutionConclusion, TestExecutionOutcome, ToolCommand,
    ToolStepOutput, ToolStepPort, WorkerAgentPort, WorkerRole, WorkerTurnOutcome,
    WorkerTurnRequest, WorkflowSnapshotPort, WorktreeSnapshot,
};
use crate::prompt::{
    decision_fix_prompt, decision_prompt, evaluator_prompt, merge_conflict_prompt,
    spec_sync_prompt, verdict_fix_prompt,
};
use crate::run_history;
use crate::state::{
    ArchivePreflight, ChangeRunStatus, ChangeStepEdge, ChangeStepKind, ChangeStepState,
    ChangeStepStatus, EdgeKind, NodeKey, RunUpdate,
};
use crate::steps::change_branch;
use crate::transcript::{failure_reason, final_assistant_text};
use crate::verdict::{parse_verdict, EvaluatorChecklist, MAX_REPORT_CHARS};

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

/// 决策软拒环漂移上限：`drift > LIMIT`（第 3 次驳回）才 `Terminal::failed`——
/// 前两次驳回均可同会话续注纠正；ask 轮次不计（用户驱动的停等，非模型漂移）。
/// 与 `MAX_ROUNDS` / `MAX_RETRY_TIMES` 同类编译期常量（非运行期可调配置）。
const DECISION_DRIFT_LIMIT: usize = 2;

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
/// **计划图驱动**：每轮问写面 `phase_next`（路由唯一权威，白名单随响应缓存），
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
    let mut allowed: Vec<BacktrackOption> = Vec::new();
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
            // 决策分叉
            match resolve_deadlock(
                worker, tools, snapshot, guard, request, &next, error, &allowed, &mut trace,
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
            // 计划步序列缺位即计划图漂移（相位定义恒含 PhaseStart 起步）：显式失
            // 败停给用户，MUST NOT 空转（无步可走却回 phase-next 会成为活锁）
            return Terminal::failed(format!(
                "phase-next 输出漂移：相位 \"{phase}\" 缺计划步序列（PhaseDefinition.steps）"
            ))
            .into_pair();
        }
        allowed = next.allowed_backtrack_phases.clone();

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
        let mut cursor = PhaseCursor::new(&next, entry_kind, sync_specs);
        let mut upgraded = false;
        // legacy 整相位短路（initialize 表头一次性基线检查相位）：`worktree=None`
        // 的存量记录 exec root = 主仓（root == main_root ⟺ worktree 缺席——发起
        // 前置校验保证 worktree 在场时 exec root ≠ 主仓）——restore / 检查 spawn
        // 在主仓执行会 reset 用户未提交改动（灾难），故步序列零执行（零 vcs /
        // 零检查 spawn、主仓零重置零 clean），仅 PhaseStart 开相 + PhaseLog 以
        // skipped 落账（跳过因可见，一次性持久同 pass 语义）
        if phase == INITIALIZE_PHASE_ID && request.root == request.main_root {
            cursor.attempt = match step_phase_start(&ctx, &mut trace, &cursor).await {
                Ok(attempt) => attempt,
                Err(terminal) => return terminal.into_pair(),
            };
            if let Err(terminal) =
                step_initialize_skipped_phase_log(&ctx, &mut trace, &cursor).await
            {
                return terminal.into_pair();
            }
        } else {
            // ②–⑥ 相位主体：按计划步序列迭代（exec / agent / git / gate 由
            //       `StepNode` 声明，walker 按 `group()` 语义分派执行体）
            for step in &next.steps {
                match run_step(&ctx, &mut cursor, &mut trace, step).await {
                    Ok(StepFlow::Proceed) => {}
                    Ok(StepFlow::Upgraded) => {
                        upgraded = true;
                        break;
                    }
                    Err(terminal) => return terminal.into_pair(),
                }
            }
        }
        trace.failed_phase = cursor.failed.then(|| phase.clone());
        if upgraded {
            // 门禁步失败升格（fail 已落账、零会话）：回 phase-next 进决策分叉
            continue;
        }

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
///（决策 backtrack 落账后置位，相位入口消费一次即清）+ 上一轮相位 fail 记因
///（同相位重跑 → retry 边判定）。
#[derive(Default)]
struct EdgeTrace {
    tail: Option<NodeKey>,
    incoming: Option<EdgeKind>,
    failed_phase: Option<String>,
}

/// 节点身份键组装（`(phase, attempt, step, instance_no)`）。
fn node_key(phase: &str, attempt: u32, step: ChangeStepKind, instance_no: u32) -> NodeKey {
    (phase.to_owned(), attempt, step, instance_no)
}

/// 步节点起点：分配实例号（`(phase, attempt, step)` 单点递增，feedback 重跑为递
/// 增新实例）→ 落 incoming 边（链尾 → 本节点；run 首节点无源不落边）→ 链尾前
/// 移；返回实例号供步状态行 emit 复用。
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

/// 步状态行落值（emit 载荷收窄：状态 + 三可选面 + 步级时间与 token 载荷镜像面）。
/// token 两值为数据随行面（`run_worker` 经 [`StepSettle::with_tokens`] 落值、其余
/// 步不设即 None）；`started_at` / `ended_at` 两时间字段由 `emit_node` 单点铸造
/// 覆写——构造器恒中性值（0 / None），MUST NOT 由调用方携带时间。
struct StepSettle {
    status: ChangeStepStatus,
    session_id: Option<String>,
    detail: Option<String>,
    output_ref: Option<String>,
    started_at: i64,
    ended_at: Option<i64>,
    token_input: Option<u64>,
    token_output: Option<u64>,
}

impl StepSettle {
    /// 步进行中（无会话 / 详情 / 输出引用；时间与 token 恒中性——时间由
    /// `emit_node` 铸造覆写）。
    fn running() -> Self {
        Self {
            status: ChangeStepStatus::Running,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 0,
            ended_at: None,
            token_input: None,
            token_output: None,
        }
    }

    /// 步通过（携人读摘要）。
    fn passed(detail: Option<String>) -> Self {
        Self {
            status: ChangeStepStatus::Passed,
            detail,
            ..Self::running()
        }
    }

    /// 步失败（携失败记因）。
    fn failed(detail: Option<String>) -> Self {
        Self {
            status: ChangeStepStatus::Failed,
            detail,
            ..Self::running()
        }
    }

    /// 步因停止 / 终止收敛。
    fn stopped(detail: Option<String>) -> Self {
        Self {
            status: ChangeStepStatus::Stopped,
            detail,
            ..Self::running()
        }
    }

    /// 随行会话 id（WorkerAgent 步）。
    fn with_session(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    /// 随行全量输出引用（`ExecOutput::Recorded { exec_id }`）。
    fn with_output_ref(mut self, output_ref: Option<String>) -> Self {
        self.output_ref = output_ref;
        self
    }

    /// 随行本步 token 增量（in / out 两值；仅 WorkerAgent 步终态落值，缺席
    /// None）——数据随行面，时间字段不在本 builder 语义内。
    fn with_tokens(mut self, token_input: Option<u64>, token_output: Option<u64>) -> Self {
        self.token_input = token_input;
        self.token_output = token_output;
        self
    }
}

/// 步状态行 emit（节点累积 + Notice 广播；同稳定身份键的后续 emit 就地合并）。
/// 步级时间铸造收本单点：全部 44 个调用点零触点覆盖——running emit 铸
/// `(now, None)`、终态 emit 铸 `(now, Some(now))`（`phaseStart` 等单 emit 瞬步
/// 自然 `started_at == ended_at` 同刻）；该 started 同刻值是否被采信由
/// `accumulate_node` 合并侧裁定（started 保首见）。复用既有 `now_millis()` 零新
/// 时钟缝——收口时刻铸造只在生产路径消费，corpus 确定性由测试直构
/// `ChangeStepState` 注入定值承载（沿本文件 `now_millis` 既有注释口径）。
fn emit_node(
    guard: &RunGuard,
    phase: &str,
    attempt: u32,
    step: ChangeStepKind,
    instance_no: u32,
    mut settle: StepSettle,
) {
    let now = now_millis();
    settle.started_at = now;
    settle.ended_at = (settle.status != ChangeStepStatus::Running).then_some(now);
    guard.emit(RunUpdate::Step {
        step: ChangeStepState {
            phase: phase.to_owned(),
            attempt,
            step,
            instance_no,
            status: settle.status,
            session_id: settle.session_id,
            detail: settle.detail,
            output_ref: settle.output_ref,
            started_at: settle.started_at,
            ended_at: settle.ended_at,
            token_input: settle.token_input,
            token_output: settle.token_output,
        },
    });
}

// ---------------------------------------------------------------------------
// 相位内计划步执行体（initialize 表头一次性基线检查相位步序：PhaseStart →
// Clean → StaticCheck → TestExecution → PhaseLog——Clean 为 vcs 直调基线恢复
// 步，两检查步 `on_fail=Terminal` 无反馈边例外、失败即停给用户；其余相位沿各
// 表 steps 声明执行，PhaseLog 机械 checklist 零改动复用）
// ---------------------------------------------------------------------------

/// 计划步执行携出物。
enum StepFlow {
    /// 步完成，继续计划序下一步。
    Proceed,
    /// 门禁升格（fail phase-log 已代写）：截断本相位余下步回 phase-next。
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

/// 相位内步序携出状态（跨 step 传递的产物；相位入口边 kind 单点消费）。
struct PhaseCursor {
    /// 相位入口 incoming 边 kind（drive 计算：backtrack > retry > forward）
    entry_kind: EdgeKind,
    /// 开相 attempt（PhaseStart 步产出；未开相 0）
    attempt: u32,
    /// 当前 executor 会话槽（反馈边续注同会话；phase-log 落账携出）
    executor_session: Option<String>,
    /// executor 模型档（相位表 spec 供给；反馈边续注不换档）
    executor_level: ModelLevel,
    /// evaluator 模型档（相位表 spec 供给；verdict 反馈边续注不换档）
    evaluator_level: ModelLevel,
    /// evaluator 步收口产物（VerdictGate / PhaseLog 步消费）
    eval_outcome: Option<WorkerTurnOutcome>,
    /// verdict 解析产物（PhaseLog 步消费）
    verdict: Option<EvaluatorChecklist>,
    /// test-execution 门禁通过产物（PhaseLog 步机械代写消费）
    test_outcome: Option<TestExecutionOutcome>,
    /// mutation-test 门禁通过产物（含跳过面——PhaseLog 步机械 checklist 第 3 条
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
    /// 归档双写产物（Seal 步产出 → Finalize / PhaseLog 步消费）
    seal_outcome: Option<ArchiveOutcome>,
    /// 本相位是否以 fail 收场（相位重试边判定面；升格路径恒真）
    failed: bool,
}

impl PhaseCursor {
    /// 空携出状态（attempt 未开、无会话、无收口产物）。
    fn new(next: &PhaseNextOutcome, entry_kind: EdgeKind, sync_specs: bool) -> Self {
        Self {
            entry_kind,
            attempt: 0,
            executor_session: None,
            executor_level: next
                .executor
                .as_ref()
                .map(|spec| spec.model_level)
                .unwrap_or_default(),
            evaluator_level: next
                .evaluator
                .as_ref()
                .map(|spec| spec.model_level)
                .unwrap_or_default(),
            eval_outcome: None,
            verdict: None,
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

/// 计划步分派（**按 step 节点语义执行，无步序 / 门禁特判**）：决策分叉三节点
///（decision / retry-gate / whitelist-gate）为 phase-next 错误触发的共享子序列，
/// 不由相位 `steps` 声明——声明漂移即无操作跳过。
async fn run_step(
    ctx: &StepContext<'_>,
    cursor: &mut PhaseCursor,
    trace: &mut EdgeTrace,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    match step.node {
        StepNode::PhaseStart => {
            cursor.attempt = step_phase_start(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
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
        StepNode::Evaluator => {
            step_evaluator(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::VerdictGate => step_verdict_gate(ctx, trace, cursor, step).await,
        StepNode::PhaseLog => {
            step_phase_log(ctx, trace, cursor).await?;
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
        StepNode::Decision | StepNode::RetryGate | StepNode::WhitelistGate => Ok(StepFlow::Proceed),
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
    )
    .await
}

/// ② phase-start 步：开相（attempt 计时）→ 单节点 emit（`Passed`；开相失败落
/// `Failed` 节点后显式 `Err`）。incoming 边 kind = 相位入口值（backtrack / retry
/// / forward）。
async fn step_phase_start(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &PhaseCursor,
) -> Result<u32, Terminal> {
    let phase = ctx.phase;
    let outcome: Result<PhaseStartOutcome, Terminal> = run_tool(
        ctx.tools,
        ctx.guard,
        &ctx.request.root,
        ToolCommand::PhaseStart {
            change_id: ctx.request.change_id.clone(),
            phase: phase.to_owned(),
        },
        "phase-start",
    )
    .await;
    match outcome {
        Ok(outcome) => {
            let instance_no = begin_node(
                ctx.guard,
                trace,
                phase,
                outcome.attempt,
                ChangeStepKind::PhaseStart,
                cursor.entry_kind,
            );
            emit_node(
                ctx.guard,
                phase,
                outcome.attempt,
                ChangeStepKind::PhaseStart,
                instance_no,
                StepSettle::passed(Some(format!("attempt {} 已开跑", outcome.attempt))),
            );
            Ok(outcome.attempt)
        }
        Err(terminal) => {
            let instance_no = begin_node(
                ctx.guard,
                trace,
                phase,
                0,
                ChangeStepKind::PhaseStart,
                cursor.entry_kind,
            );
            emit_node(
                ctx.guard,
                phase,
                0,
                ChangeStepKind::PhaseStart,
                instance_no,
                StepSettle::failed(Some("开相失败".to_owned())),
            );
            Err(terminal)
        }
    }
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
    if ctx.request.root == ctx.request.main_root {
        settle_archive_node(
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
            settle_archive_node(ctx, cursor, kind, instance_no, StepSettle::passed(Some(summary)));
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

/// initialize legacy 短路落账（skipped=true + 空 checklist + 跳过因）：PhaseLog
/// 节点 emit + 写面落账——skipped 条目与 pass 一次性持久同语义（写面
/// `has_phase_passed` 谓词两者皆认），后续 run 持久跳过；三会话槽位恒 `None`
///（零 agent 会话）。Clean / StaticCheck / TestExecution 三步零执行。
async fn step_initialize_skipped_phase_log(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &PhaseCursor,
) -> Result<(), Terminal> {
    let instance_no = begin_node(
        ctx.guard,
        trace,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::PhaseLog,
        EdgeKind::Forward,
    );
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::running(),
    );
    let logged = run_phase_log(
        ctx.tools,
        ctx.guard,
        ctx.request,
        PhaseLogInput {
            phase: ctx.phase.to_owned(),
            report: TXT_SKIP_LEGACY_INITIALIZE.to_owned(),
            checklist: Vec::new(),
            skipped: true,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
        },
    )
    .await?;
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::passed(Some(format!(
            "attempt {} 已落账（skipped：{TXT_SKIP_LEGACY_INITIALIZE}）",
            logged.attempt
        ))),
    );
    Ok(())
}

/// ③ executor 步：本相位 executor 会话（prompt 由写面 phase-next 单点组装——
/// 上下文头 + 失败反馈段 + 回溯原因后缀全在写面）。
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
    )
    .await?;
    cursor.executor_session = Some(outcome.session_id);
    cursor.executor_level = executor.model_level;
    Ok(())
}

/// ④ static-check 步：门禁 spawn + 定向反馈边（步声明 `OnFail::FeedbackLoop`，
/// 独立计数 ≤ limit）——失败诊断 Continue 注入同一 executor 会话修复重跑（新
/// `instance_no` 节点 + `feedback` 边）；超限升格相位 fail（桌面代写 fail
/// phase-log，不跑 evaluator），回 phase-next 进重试 / 决策分叉。
async fn step_static_check(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    // initialize 相位声明 `on_fail=Terminal`（基线绿门：失败即停——无反馈边、
    // 零 agent 会话可注入，重试基线无意义）→ 单发执行体（verdict-gate Terminal
    // 前置分支同式先例）；缺省相位走反馈边路径不受影响
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
        )
        .await?;
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
            step_fail_phase_log(
                ctx.tools,
                ctx.guard,
                trace,
                ctx.request,
                ctx.phase,
                cursor.attempt,
                cursor.executor_session.as_deref(),
                &outcome.diagnostics,
            )
            .await?;
            cursor.failed = true;
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
        )
        .await?;
        cursor.executor_session = Some(fix.session_id);
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
    )
    .await?;
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
    step_test_execution_upgraded_phase_log(
        ctx.tools,
        ctx.guard,
        trace,
        ctx.request,
        ctx.phase,
        cursor.attempt,
        &outcome,
    )
    .await?;
    cursor.failed = true;
    Ok(StepFlow::Upgraded)
}

/// initialize 相位 static-check 单发执行体（`on_fail=Terminal` 前置分支分派）：
/// 单节点 running → spawn（baseline 槽位）→ pass / fail 收口。pass：`passed` +
/// `output_ref` 回填后 Proceed；fail：failed 节点（detail = 诊断摘要、
/// `output_ref` 回填）+ `Terminal::failed`（static 红文案携 baseline 报告目录
/// ——`outcome.report_dir` 供给 `{dir}` 占位）。
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
    )
    .await?;
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
///（PhaseLog 机械 checklist 分支天然分派）后 Proceed；Error / Fail：failed 节点
/// + `Terminal::failed` 分文案（环境 / 测试红可辨，携 baseline 报告目录）。
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
    )
    .await?;
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
            ))),
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
        StepSettle::failed(Some(diagnose_brief(&outcome.feedback_text))),
    );
    // 首败升格（桌面代写 fail 落账，不跑 evaluator、零修复会话；三会话槽位恒
    // None），随后回 phase-next 进决策分叉
    step_mutation_test_upgraded_phase_log(
        ctx.tools,
        ctx.guard,
        trace,
        ctx.request,
        ctx.phase,
        cursor.attempt,
        &outcome,
    )
    .await?;
    cursor.failed = true;
    Ok(StepFlow::Upgraded)
}

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
    let outcome = run_worker(
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
    )
    .await?;
    cursor.eval_outcome = Some(outcome);
    cursor.evaluator_level = evaluator.model_level;
    Ok(())
}

async fn step_verdict_gate(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    // code-analyze 相位声明 VerdictGate on_fail=Terminal（分析相位：verdict
    // 解析失败即终态，无 agent 会话可续注修复）→ 单次解析，失败即停。
    if step.on_fail == Some(OnFail::Terminal) {
        return step_verdict_gate_terminal(ctx, trace, cursor).await;
    }
    let limit = feedback_limit(step, "verdict-gate")?;
    let mut feedback: u32 = 0;
    loop {
        let instance_no = begin_node(
            ctx.guard,
            trace,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::VerdictGate,
            EdgeKind::Forward,
        );
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::VerdictGate,
            instance_no,
            StepSettle::running(),
        );
        // 会话 id 与最终消息文本一次取走（同会话续注 / 重解析所需），避免后续
        // cursor 赋值与借用冲突；None 产物 = 计划图漂移（无会话可续注、无文本
        // 可修）——保持显式失败停给用户，MUST NOT 走反馈边
        let (session_id, text) = match cursor.eval_outcome.as_ref() {
            Some(outcome) => (
                outcome.session_id.clone(),
                outcome
                    .final_message
                    .clone()
                    .or_else(|| final_assistant_text(&outcome.transcript))
                    .unwrap_or_default(),
            ),
            None => {
                emit_node(
                    ctx.guard,
                    ctx.phase,
                    cursor.attempt,
                    ChangeStepKind::VerdictGate,
                    instance_no,
                    StepSettle::failed(Some("缺 evaluator 收口产物".to_owned())),
                );
                return Err(Terminal::failed(format!(
                    "phase \"{}\" 计划步漂移：VerdictGate 步缺 evaluator 收口产物",
                    ctx.phase
                )));
            }
        };
        match parse_verdict(&text) {
            Ok(checklist) => {
                emit_node(
                    ctx.guard,
                    ctx.phase,
                    cursor.attempt,
                    ChangeStepKind::VerdictGate,
                    instance_no,
                    StepSettle::passed(Some(format!("verdict={}", checklist.verdict.as_str()))),
                );
                cursor.verdict = Some(checklist);
                return Ok(StepFlow::Proceed);
            }
            Err(error) => {
                emit_node(
                    ctx.guard,
                    ctx.phase,
                    cursor.attempt,
                    ChangeStepKind::VerdictGate,
                    instance_no,
                    StepSettle::failed(Some(diagnose_brief(&error))),
                );
                if feedback >= limit {
                    // 超限升格相位 fail（桌面代写，不跑 executor；携 executor +
                    // evaluator 双槽位——两会话均在反馈循环中在场）
                    step_verdict_gate_upgraded_phase_log(
                        ctx.tools,
                        ctx.guard,
                        trace,
                        ctx.request,
                        ctx.phase,
                        cursor.attempt,
                        cursor.executor_session.as_deref(),
                        Some(session_id.as_str()),
                        &error,
                    )
                    .await?;
                    cursor.failed = true;
                    return Ok(StepFlow::Upgraded);
                }
                feedback += 1;
                // 反馈边：解析失败诊断注入同一 evaluator 会话（Continue）修复
                // ——修复节点为 instance_no 递增的独立新节点，边 kind = feedback
                let fix = run_worker(
                    ctx.worker,
                    ctx.guard,
                    ctx.request,
                    &ctx.request.root,
                    ctx.phase,
                    cursor.attempt,
                    WorkerRole::Evaluator,
                    verdict_fix_prompt(&error, feedback, limit),
                    cursor.evaluator_level,
                    Some(session_id),
                    trace,
                    EdgeKind::Feedback,
                )
                .await?;
                cursor.eval_outcome = Some(fix);
            }
        }
    }
}

/// ⑧ phase-log 步：桌面代写落账（写通道唯一——经写面进程内直调）。三分流按
/// 「产物在否」分离：test-execution 相位走机械 checklist（绿跑零 agent 会话）→
/// 无评估产物的确定性相位（archive）走空 checklist（`derive_verdict([]) = Pass`）
/// → 其余相位走 verdict 解析产物（executor + evaluator 双槽位随行，decision
/// 槽位归 decision-log 单点挂账恒缺省）。
async fn step_phase_log(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<(), Terminal> {
    let instance_no = begin_node(
        ctx.guard,
        trace,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::PhaseLog,
        EdgeKind::Forward,
    );
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::running(),
    );
    if let Some(outcome) = cursor.test_outcome.take() {
        let mutation = cursor.mutation_outcome.take();
        let logged = run_phase_log(
            ctx.tools,
            ctx.guard,
            ctx.request,
            test_execution_phase_log_input(ctx.phase, &outcome, mutation.as_ref()),
        )
        .await?;
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::PhaseLog,
            instance_no,
            StepSettle::passed(Some(format!("attempt {} 已落账", logged.attempt))),
        );
        cursor.failed = false;
        return Ok(());
    }
    let Some(checklist) = cursor.verdict.take() else {
        // 无评估产物（archive 确定性相位：无 TestExecution / Evaluator /
        // VerdictGate step）→ 空 checklist 落账：`derive_verdict([]) = Pass` 即
        // 「相位已 pass」标记（非评估证据面）。三分流按「产物在否」天然分离
        //（test_outcome 分支在前、verdict 分支在后），零 `phase == archive` 特判。
        let logged = run_phase_log(
            ctx.tools,
            ctx.guard,
            ctx.request,
            PhaseLogInput {
                phase: ctx.phase.to_owned(),
                report: String::new(),
                checklist: Vec::new(),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .await?;
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::PhaseLog,
            instance_no,
            StepSettle::passed(Some(format!(
                "attempt {} 已落账（空 checklist）",
                logged.attempt
            ))),
        );
        cursor.failed = false;
        return Ok(());
    };
    let evaluator_session = cursor
        .eval_outcome
        .as_ref()
        .map(|outcome| outcome.session_id.clone());
    let failed = checklist.verdict == Verdict::Fail;
    let logged = run_phase_log(
        ctx.tools,
        ctx.guard,
        ctx.request,
        PhaseLogInput {
            phase: ctx.phase.to_owned(),
            report: checklist.report,
            checklist: checklist.checklist,
            skipped: false,
            executor_session_id: cursor.executor_session.clone(),
            evaluator_session_id: evaluator_session,
            decision_session_id: None,
        },
    )
    .await?;
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::passed(Some(format!("attempt {} 已落账", logged.attempt))),
    );
    cursor.failed = failed;
    Ok(())
}

/// 门禁步反馈边上限读取（**步声明单点**）：缺声明 / 误声明即计划图漂移——
/// MUST NOT 由 walker 侧常量兜底（反馈边计数上限随门禁 step 声明退役，三门禁
/// 步各自的独立预算不再驻 walker）。
///
/// 本函数只服务 static-check / verdict-gate 两门禁步；TestExecution /
/// MutationTest 走 `on_fail=Escalate` 的单发执行体，不经反馈计数（Escalate 声明
/// 落在此处即漂移，显式失败）。
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

/// code-analyze 相位 VerdictGate 单次解析路径（`on_fail=Terminal`）：解析
/// evaluator 收口产物中的 verdict——成功即 Proceed（交 PhaseLog 落账）；失败即
/// Terminal（分析相位无 agent 会话可续注修复，显式停给用户）。
async fn step_verdict_gate_terminal(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
) -> Result<StepFlow, Terminal> {
    let instance_no = begin_node(
        ctx.guard,
        trace,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::VerdictGate,
        EdgeKind::Forward,
    );
    emit_node(
        ctx.guard,
        ctx.phase,
        cursor.attempt,
        ChangeStepKind::VerdictGate,
        instance_no,
        StepSettle::running(),
    );
    let Some(outcome) = &cursor.eval_outcome else {
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::VerdictGate,
            instance_no,
            StepSettle::failed(Some("缺 evaluator 收口产物".to_owned())),
        );
        return Err(Terminal::failed(format!(
            "phase \"{}\" 计划步漂移：VerdictGate 步缺 evaluator 收口产物",
            ctx.phase
        )));
    };
    let text = outcome
        .final_message
        .clone()
        .or_else(|| final_assistant_text(&outcome.transcript))
        .unwrap_or_default();
    match parse_verdict(&text) {
        Ok(checklist) => {
            emit_node(
                ctx.guard,
                ctx.phase,
                cursor.attempt,
                ChangeStepKind::VerdictGate,
                instance_no,
                StepSettle::passed(Some(format!("verdict={}", checklist.verdict.as_str()))),
            );
            cursor.verdict = Some(checklist);
            Ok(StepFlow::Proceed)
        }
        Err(error) => {
            emit_node(
                ctx.guard,
                ctx.phase,
                cursor.attempt,
                ChangeStepKind::VerdictGate,
                instance_no,
                StepSettle::failed(Some(diagnose_brief(&error))),
            );
            Err(Terminal::failed(format!("verdict 解析失败: {error}")))
        }
    }
}

/// phase-log 落账单点（写通道唯一：落账经写面进程内直调）；步状态行 emit 归调
/// 用方。
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
    )
    .await
}

/// 升格落账：桌面代写 fail checklist（静态检查反馈边超限），随后回 phase-next
/// 进重试 / 决策分叉。fail 条目仅携 executor 槽位（反馈边续注同一 executor 会
/// 话，id 稳定；evaluator 未跑）。
#[allow(clippy::too_many_arguments)]
async fn step_fail_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    trace: &mut EdgeTrace,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    executor_session: Option<&str>,
    diagnostics: &str,
) -> Result<(), Terminal> {
    let instance_no = begin_node(
        guard,
        trace,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        EdgeKind::Forward,
    );
    emit_node(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::running(),
    );
    let logged = run_phase_log(
        tools,
        guard,
        request,
        PhaseLogInput {
            phase: phase.to_owned(),
            report: "静态检查反馈边超限（连续修复未通过），相位升格 fail".to_owned(),
            checklist: vec![ChecklistItem {
                item: "静态检查".to_owned(),
                pass: false,
                evidence: diagnostics.to_owned(),
            }],
            skipped: false,
            executor_session_id: executor_session.map(ToOwned::to_owned),
            evaluator_session_id: None,
            decision_session_id: None,
        },
    )
    .await?;
    emit_node(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::passed(Some(format!("attempt {} 已落账", logged.attempt))),
    );
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

/// 首败升格落账（零会话）：report 携完整明细（≤2000，供决策 prompt 的失败
/// 报告段），evidence 携诊断摘要 + 报告目录；三会话槽位恒 `None`（本相位零
/// agent 会话，决策会话槽位由 `decision-log` 另行挂账）。随后回 phase-next 进
/// 决策分叉。
async fn step_test_execution_upgraded_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    trace: &mut EdgeTrace,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    outcome: &TestExecutionOutcome,
) -> Result<(), Terminal> {
    let instance_no = begin_node(
        guard,
        trace,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        EdgeKind::Forward,
    );
    emit_node(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::running(),
    );
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
    let logged = run_phase_log(
        tools,
        guard,
        request,
        PhaseLogInput {
            phase: phase.to_owned(),
            report: truncate_report(&detail),
            checklist: vec![ChecklistItem {
                item: "测试执行".to_owned(),
                pass: false,
                evidence: format!(
                    "{}；报告目录 {}",
                    diagnose_brief(&outcome.findings_brief),
                    outcome.report_dir
                ),
            }],
            skipped: false,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
        },
    )
    .await?;
    emit_node(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::passed(Some(format!("attempt {} 已落账", logged.attempt))),
    );
    Ok(())
}

/// 首败升格落账（零会话，两时点同式）：report 携完整反馈文本（≤2000，供决策
/// prompt 的失败报告段），evidence 携诊断摘要 + 报告路径清单；三会话槽位恒
/// `None`。随后回 phase-next 进决策分叉。
async fn step_mutation_test_upgraded_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    trace: &mut EdgeTrace,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    outcome: &MutationTestOutcome,
) -> Result<(), Terminal> {
    let instance_no = begin_node(
        guard,
        trace,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        EdgeKind::Forward,
    );
    emit_node(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::running(),
    );
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
    let logged = run_phase_log(
        tools,
        guard,
        request,
        PhaseLogInput {
            phase: phase.to_owned(),
            report: truncate_report(&detail),
            checklist: vec![ChecklistItem {
                item: "突变测试".to_owned(),
                pass: false,
                evidence: format!(
                    "{}；{}",
                    diagnose_brief(&outcome.feedback_text),
                    mutation_refs
                ),
            }],
            skipped: false,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
        },
    )
    .await?;
    emit_node(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::passed(Some(format!("attempt {} 已落账", logged.attempt))),
    );
    Ok(())
}

/// 超限升格落账：桌面代写 fail checklist（评估输出反馈边超限），随后回
/// phase-next 进重试 / 决策分叉。fail 条目携 executor + evaluator 双槽位（反馈
/// 边续注同一 evaluator 会话、executor 会话自 executor 步在场；decision 未跑无
/// 会话可记）——条目 item / evidence 会流入下一 attempt 双 prompt 与决策输入。
#[allow(clippy::too_many_arguments)]
async fn step_verdict_gate_upgraded_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    trace: &mut EdgeTrace,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    executor_session: Option<&str>,
    evaluator_session: Option<&str>,
    error: &str,
) -> Result<(), Terminal> {
    let instance_no = begin_node(
        guard,
        trace,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        EdgeKind::Forward,
    );
    emit_node(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::running(),
    );
    let logged = run_phase_log(
        tools,
        guard,
        request,
        PhaseLogInput {
            phase: phase.to_owned(),
            report: "评估输出反馈边超限（连续修复未通过），相位升格 fail".to_owned(),
            checklist: vec![ChecklistItem {
                item: "评估输出格式".to_owned(),
                pass: false,
                evidence: error.to_owned(),
            }],
            skipped: false,
            executor_session_id: executor_session.map(ToOwned::to_owned),
            evaluator_session_id: evaluator_session.map(ToOwned::to_owned),
            decision_session_id: None,
        },
    )
    .await?;
    emit_node(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        instance_no,
        StepSettle::passed(Some(format!("attempt {} 已落账", logged.attempt))),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 归档相位 9 步执行体（walker 独立 vcs 缝 + 归档两 agent 步；Cleanup 为尽力
// 步例外——失败仅 detail 警告、步恒 passed、链必收口，永不 Terminal；其余步
// 失败一律 Terminal——相位表 `on_fail = None`，不唤决策、不进相位重试）
// ---------------------------------------------------------------------------

/// 归档步起点：分配实例号 + 落 incoming 边 + emit 运行中节点（git / 内联步与
/// 条件短路分支共用；agent 步节点由 [`run_worker`] 单点 emit）。
fn begin_archive_node(
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

/// 归档步收口（passed 携通过 / 跳过摘要，failed 携失败记因；同实例身份键就地
/// 合并）。
fn settle_archive_node(
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
    let payload: ArchivePreflight = match run_tool(
        ctx.tools,
        ctx.guard,
        &ctx.request.main_root,
        ToolCommand::ArchivePreflight {
            change_id: ctx.request.change_id.clone(),
        },
        "archive-preflight",
    )
    .await
    {
        Ok(payload) => payload,
        Err(terminal) => {
            settle_archive_node(
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
                    settle_archive_node(
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
                settle_archive_node(
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
    let brief = preflight_brief(&payload);
    settle_archive_node(
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
    let (name, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => {
            settle_archive_node(
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
                    settle_archive_node(
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
    settle_archive_node(ctx, cursor, kind, instance_no, settle);
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
    let (name, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => {
            settle_archive_node(
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
    settle_archive_node(ctx, cursor, kind, instance_no, settle);
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
    let (name, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => {
            settle_archive_node(
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
    settle_archive_node(ctx, cursor, kind, instance_no, settle);
    Ok(())
}

/// 归档步失败收口单点：emit 失败节点（携记因）+ 返回 Terminal。
fn fail_archive_node(
    ctx: &StepContext<'_>,
    cursor: &PhaseCursor,
    kind: ChangeStepKind,
    instance_no: u32,
    reason: String,
) -> Terminal {
    settle_archive_node(
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
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
        let instance_no = begin_archive_node(ctx, trace, cursor, kind);
        settle_archive_node(
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
        let instance_no = begin_archive_node(ctx, trace, cursor, kind);
        settle_archive_node(
            ctx,
            cursor,
            kind,
            instance_no,
            StepSettle::passed(Some(TXT_SKIP_NO_DELTA.to_owned())),
        );
        return Ok(());
    }
    if !cursor.sync_specs {
        let instance_no = begin_archive_node(ctx, trace, cursor, kind);
        settle_archive_node(
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
    let (_, worktree) = match archive_context(cursor) {
        Ok(context) => context,
        Err(terminal) => {
            settle_archive_node(
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
                // 步恒 passed、Seal / Finalize / PhaseLog 照常续走
                Err(error) => StepSettle::passed(Some(format!(
                    "worktree 清理失败（尽力步，不影响归档收口）: {error}"
                ))),
            }
        }
    };
    settle_archive_node(ctx, cursor, kind, instance_no, settle);
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
    let outcome: ArchiveOutcome = match run_tool(
        ctx.tools,
        ctx.guard,
        &ctx.request.main_root,
        ToolCommand::ArchiveSeal {
            change_id: ctx.request.change_id.clone(),
        },
        "archive-seal",
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(terminal) => {
            settle_archive_node(
                ctx,
                cursor,
                kind,
                instance_no,
                StepSettle::failed(Some("归档双写收口失败".to_owned())),
            );
            return Err(terminal);
        }
    };
    settle_archive_node(
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
    let instance_no = begin_archive_node(ctx, trace, cursor, kind);
    let name = match archive_context(cursor) {
        Ok((name, _)) => name,
        Err(terminal) => {
            settle_archive_node(
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
        settle_archive_node(
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
    settle_archive_node(ctx, cursor, kind, instance_no, settle);
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
/// evaluator 相位 fail 均直达决策，自走同相位重跑退役）：决策会话产出三动作，
/// backtrack 经写面落账后回外层 phase-next 重路由（白名单门与解析软拒环在
/// `decision_session` 内闭环）。
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
    let input = build_decision_input(&detail, fail_phase, allowed);
    let action = match decision_session(worker, tools, guard, request, &input, trace).await {
        Ok(action) => action,
        Err(terminal) => return Deadlock::Terminal(terminal),
    };

    match action {
        DecisionAction::Backtrack { to, reason } => {
            let retry_instance = begin_node(
                guard,
                trace,
                fail_phase,
                input.attempt,
                ChangeStepKind::RetryGate,
                EdgeKind::Forward,
            );
            emit_node(
                guard,
                fail_phase,
                input.attempt,
                ChangeStepKind::RetryGate,
                retry_instance,
                StepSettle::running(),
            );
            emit_node(
                guard,
                fail_phase,
                input.attempt,
                ChangeStepKind::RetryGate,
                retry_instance,
                StepSettle::passed(Some(format!("决策 backtrack → {to}"))),
            );
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
            )
            .await;
            match backtrack {
                Ok(_) => {
                    emit_node(
                        guard,
                        fail_phase,
                        input.attempt,
                        ChangeStepKind::RetryGate,
                        retry_instance,
                        StepSettle::passed(Some("回溯已落账，回 phase-next 重路由".to_owned())),
                    );
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
            // ask 在决策会话内被应答循环消化（wait_answer → Continue 重出封闭
            // 集）；携出封闭集外的 ask 到这里即漂移
            Deadlock::Terminal(Terminal::failed(
                "决策漂移：ask 动作未被应答循环消化".to_owned(),
            ))
        }
    }
}

/// 决策会话：白名单门与输出解析的软拒环内闭环（漂移 ≤ `DECISION_DRIFT_LIMIT`
/// 可同会话续注纠正、超限 `Terminal::failed`「决策漂移」）、ask 停等不计漂移、
/// 每轮 parsed 动作 emit 一个 `WhitelistGate` 实例（通过 passed / 驳回 failed 携
/// 拒绝文案）、每轮 `decision-log` 幂等挂账（置于 parse 之前——parse 失败路径
/// 同样留痕；失败 / 停止收口的 decision 会话不挂账，转录仍在库内可经调试页
/// change 筛选回查）。取消信号置位 → 受控 stopped。
async fn decision_session(
    worker: &Arc<dyn WorkerAgentPort>,
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    input: &DecisionInput,
    trace: &mut EdgeTrace,
) -> Result<DecisionAction, Terminal> {
    let mut prompt = decision_prompt(input);
    let mut continue_session: Option<String> = None;
    let mut drift: usize = 0;
    loop {
        let outcome = run_worker(
            worker,
            guard,
            request,
            &request.root,
            &input.phase,
            input.attempt,
            WorkerRole::Decision,
            prompt.clone(),
            // 决策 agent 恒 High 档（重试上限分叉的决策质量面）
            ModelLevel::High,
            continue_session.clone(),
            trace,
            EdgeKind::Forward,
        )
        .await?;
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
        )
        .await?;
        let text = outcome
            .final_message
            .clone()
            .or_else(|| final_assistant_text(&outcome.transcript))
            .unwrap_or_default();
        let action = match parse_decision(&text) {
            Ok(DecisionAction::Ask { question, options }) => {
                guard.emit(RunUpdate::Ask {
                    question: question.clone(),
                    options: options.clone(),
                });
                match guard.wait_answer().await {
                    None => return Err(Terminal::stopped("ask 停等被终止")),
                    Some(answer) => {
                        // ask 停在 WaitingAsk 属用户驱动停等，不计漂移
                        prompt = format!(
                            "用户应答：{answer}\n\n请基于应答重新输出决策 JSON（三选一封闭集）。"
                        );
                        continue_session = Some(session_id);
                        continue;
                    }
                }
            }
            Ok(action) => action,
            Err(error) => {
                // 解析失败（含已退役的 retry / 未知名）：软拒一次、同会话续注纠正
                drift += 1;
                if drift > DECISION_DRIFT_LIMIT {
                    return Err(Terminal::failed(format!(
                        "决策漂移：连续 {drift} 次决策输出无法解析（{error}）"
                    )));
                }
                prompt = decision_fix_prompt(input, &error);
                continue_session = Some(session_id);
                continue;
            }
        };
        // 每轮 parsed 动作 emit 一个门节点实例（驳回在图面可见）
        let gate_instance = begin_node(
            guard,
            trace,
            &input.phase,
            input.attempt,
            ChangeStepKind::WhitelistGate,
            EdgeKind::Forward,
        );
        emit_node(
            guard,
            &input.phase,
            input.attempt,
            ChangeStepKind::WhitelistGate,
            gate_instance,
            StepSettle::running(),
        );
        match ensure_backtrack_allowed(&action, &input.phase, &input.allowed) {
            Ok(()) => {
                emit_node(
                    guard,
                    &input.phase,
                    input.attempt,
                    ChangeStepKind::WhitelistGate,
                    gate_instance,
                    StepSettle::passed(None),
                );
                return Ok(action);
            }
            Err(message) => {
                emit_node(
                    guard,
                    &input.phase,
                    input.attempt,
                    ChangeStepKind::WhitelistGate,
                    gate_instance,
                    StepSettle::failed(Some(diagnose_brief(&message))),
                );
                drift += 1;
                if drift > DECISION_DRIFT_LIMIT {
                    return Err(Terminal::failed(format!(
                        "决策漂移：连续 {drift} 次决策输出被驳回（{message}）"
                    )));
                }
                prompt = decision_fix_prompt(input, &message);
                continue_session = Some(session_id);
            }
        }
    }
}

/// 决策有界输入组装（fail checklist + 白名单 + 候选 eval report + 失败条目
/// report 原文）：`fail_report` 取失败相位最新 attempt 的 `report`（升格落账写入
/// 的完整明细，空则空串——prompt 侧渲染「（无报告文本）」）。
fn build_decision_input(
    detail: &ChangeDetail,
    fail_phase: &str,
    allowed: &[BacktrackOption],
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
    }
}

// ---------------------------------------------------------------------------
// 共用执行面
// ---------------------------------------------------------------------------

/// WorkerAgent 会话执行（三类角色统一通道）：bypassPermissions 恒档、
/// provenance（change id / 相位 / 角色 / attempt 四段；身份段恒 change id）、取消与
/// 会话失败收敛终态。
/// 步状态随行 emit（running → passed / failed / stopped，同实例身份键就地合并）；
/// 会话 id 槽由命令层 sink 桥随首个会话事件先行同步（停止寻址不依赖 turn 收口）。
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
) -> Result<WorkerTurnOutcome, Terminal> {
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
    // 本步 token 增量：终态 emit 前对 transcript 的 TurnDone.usage 单次求和落值
    //——终态单次落值即「会话结束后累计结果」（await 收口后 transcript 已含
    // TurnDone），无二次刷新通道；无论 outcome.status 为 Completed / Failed /
    // Stopped / Running 漂移收敛（transcript 在手即求和），usage 缺席 → None。
    // Err 路径（无 outcome / 无 transcript）failed emit 不落值恒 None。
    let token_input = sum_usage_tokens(&outcome.transcript, "input_tokens");
    let token_output = sum_usage_tokens(&outcome.transcript, "output_tokens");
    let (settle, terminal) = match outcome.status {
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
    };
    emit_node(
        guard,
        phase,
        attempt,
        kind,
        instance_no,
        settle
            .with_session(Some(outcome.session_id.clone()))
            .with_tokens(token_input, token_output),
    );
    match terminal {
        None => Ok(outcome),
        Some(terminal) => Err(terminal),
    }
}

/// 工具步执行：产出按封闭集窄化（漂移显式失败）；步失败 `Err` 串映射终态。
///
/// `guard` 是取消信号的**单点取值来源**：`ToolStepRequest.cancel` 只在此处经
/// `guard.cancel_flag()` 填充（各步函数不各自取值），使停止信号在步边界
/// [`check_cancel`] 之外贯通到进程面——运行中的检查域 spawn 被
/// `exec::run_shell` 的 200 ms 轮询观测并杀树收敛。
async fn run_tool<V>(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    root: &str,
    command: ToolCommand,
    label: &str,
) -> Result<V, Terminal>
where
    V: TryFrom<ToolStepOutput>,
{
    let output = match tools
        .run(crate::port::ToolStepRequest {
            root: root.to_owned(),
            command,
            cancel: guard.cancel_flag(),
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

/// 当前 UTC unix 毫秒（时钟早于 epoch 取 0，不 panic）——run 收口时刻铸造
/// 点（finish 整包同刻，corpus 确定性由测试注入 started_at / 假 history 承
/// 载，本时钟只在生产路径消费）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
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
