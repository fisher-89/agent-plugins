use std::sync::Arc;

use agent::{AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance};
use workflow::model::{ChecklistItem, Verdict};
use workflow::queries::ChangeDetail;
use workflow::write::{
    BacktrackInput, BacktrackOutcome, DecisionLogOutcome, OnFail, PhaseLogInput, PhaseLogOutcome,
    PhaseNextError, PhaseNextOutcome, PhaseStartOutcome, StepDefinition, StepNode,
};

use crate::control::{ChangeFlowControl, RunGuard};
use crate::decision::{
    ensure_backtrack_allowed, parse_decision, CandidateReport, DecisionAction, DecisionInput,
};
use crate::port::{
    RunHistoryPort, StaticCheckOutcome, TestExecutionConclusion, TestExecutionOutcome, ToolCommand,
    ToolStepOutput, ToolStepPort, WorkerAgentPort, WorkerRole, WorkerTurnOutcome,
    WorkerTurnRequest, WorkflowSnapshotPort,
};
use crate::prompt::{decision_prompt, evaluator_prompt};
use crate::run_history;
use crate::state::{
    ChangeRunStatus, ChangeStepEdge, ChangeStepKind, ChangeStepState, ChangeStepStatus, EdgeKind,
    NodeKey, RunUpdate,
};
use crate::transcript::final_assistant_text;
use crate::verdict::{parse_verdict, EvaluatorChecklist};

/// 会话 provenance 来源（sourceRef = change id / 相位 / 角色 / attempt 四段——
/// 身份段恒 change id）。
const SOURCE_CHANGE: &str = "change";

/// run 收口文案（全相位 pass；不触发归档，停等用户——与写面 done 路由的
/// 插件同源语义）。
const ALL_PHASES_PASSED: &str = "All phases have passed evaluation. Ready for archiving.";

/// run 发起入参：workspace 根 + change id + 会话窗口标识（`new_run_id` 铸造，
/// 每 run 一个）+ 发起时刻（命令层铸造，与 `begin_run` 同值入 RunEntry——
/// 运行史 started_at 与统一视图 startedAt 的同源锚）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub root: String,
    /// change 身份锚（uuid 形态；一切寻址入参）
    pub change_id: String,
    pub run_id: String,
    pub auto_next_phase: bool,
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
pub async fn walk_run(
    worker: Arc<dyn WorkerAgentPort>,
    tools: Arc<dyn ToolStepPort>,
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
    let (status, reason) = drive(&worker, &tools, &snapshot, &guard, &request).await;
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
/// run 级 `auto_next_phase` 开关（非 step 属性）。
async fn drive(
    worker: &Arc<dyn WorkerAgentPort>,
    tools: &Arc<dyn ToolStepPort>,
    snapshot: &Arc<dyn WorkflowSnapshotPort>,
    guard: &RunGuard,
    request: &RunRequest,
) -> (ChangeRunStatus, Option<String>) {
    let mut allowed: Vec<String> = Vec::new();
    let mut trace = EdgeTrace::default();
    loop {
        if let Err(terminal) = check_cancel(guard) {
            return terminal.into_pair();
        }
        // ① phase-next（路由唯一权威；白名单与 next 相位计划步序列 / 产物清单
        //    随响应下发）
        let next = match step_phase_next(tools, request).await {
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
            // 决策分叉（max_retries_exceeded → 决策 agent；路由上限 /
            // backtrack 目标非法等失败路径已在步执行体显式 Err 终态）
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
        if next.evaluator.is_none() {
            return Terminal::failed(format!("phase \"{phase}\" 无 evaluator 定义，无法评估落账"))
                .into_pair();
        }
        if next.steps.is_empty() {
            // 计划步序列缺位即计划图漂移（相位定义恒含 PhaseStart 起步）：显式失
            // 败停给用户，MUST NOT 空转（无步可走却回 phase-next 会成为活锁）
            return Terminal::failed(format!(
                "phase-next 输出漂移：相位 \"{phase}\" 缺计划步序列（PhaseDefinition.steps）"
            ))
            .into_pair();
        }
        allowed = next.allowed_backtrack_phases.clone();

        // 相位入口 incoming 边：决策 backtrack 待决值 > 同相位上一轮 fail 重跑
        //（retry）> 计划序推进（forward）
        let entry_kind = match trace.incoming.take() {
            Some(EdgeKind::Backtrack) => EdgeKind::Backtrack,
            _ if trace.failed_phase.as_deref() == Some(phase.as_str()) => EdgeKind::Retry,
            _ => EdgeKind::Forward,
        };
        let ctx = StepContext {
            worker,
            tools,
            guard,
            request,
            next: &next,
            phase: &phase,
        };
        let mut cursor = PhaseCursor::new(&next, entry_kind);
        let mut upgraded = false;
        // ②–⑥ 相位主体：按计划步序列迭代（exec / agent / gate 由 `StepNode` 声
        //       明，walker 按 `group()` 语义分派执行体）
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
        trace.failed_phase = cursor.failed.then(|| phase.clone());
        if upgraded {
            // 门禁反馈边超限升格（fail 已落账）：回 phase-next 进重试 / 决策分叉
            continue;
        }

        // phase 间停等确认 or 自动确认
        if !request.auto_next_phase {
            guard.emit(RunUpdate::ConfirmWait {
                phase: phase.clone(),
            });
            if !guard.wait_confirm().await {
                return Terminal::stopped(&format!("phase \"{phase}\" 已通过，停等未获继续确认"))
                    .into_pair();
            }
        }

        // ⑦ 回 phase-next：pass 推进 / fail 重试（预算内 walker 自走）/
        //    上限 → 决策分叉
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

/// 步状态行落值（emit 载荷收窄：状态 + 三可选面）。
struct StepSettle {
    status: ChangeStepStatus,
    session_id: Option<String>,
    detail: Option<String>,
    output_ref: Option<String>,
}

impl StepSettle {
    /// 步进行中（无会话 / 详情 / 输出引用）。
    fn running() -> Self {
        Self {
            status: ChangeStepStatus::Running,
            session_id: None,
            detail: None,
            output_ref: None,
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
}

/// 步状态行 emit（节点累积 + Notice 广播；同稳定身份键的后续 emit 就地合并）。
fn emit_node(
    guard: &RunGuard,
    phase: &str,
    attempt: u32,
    step: ChangeStepKind,
    instance_no: u32,
    settle: StepSettle,
) {
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
        },
    });
}

// ---------------------------------------------------------------------------
// 相位内计划步执行体
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
    /// evaluator 步收口产物（VerdictGate / PhaseLog 步消费）
    eval_outcome: Option<WorkerTurnOutcome>,
    /// verdict 解析产物（PhaseLog 步消费）
    verdict: Option<EvaluatorChecklist>,
    /// test-execution 门禁通过产物（PhaseLog 步机械代写消费）
    test_outcome: Option<TestExecutionOutcome>,
    /// 本相位是否以 fail 收场（相位重试边判定面；升格路径恒真）
    failed: bool,
}

impl PhaseCursor {
    /// 空携出状态（attempt 未开、无会话、无收口产物）。
    fn new(next: &PhaseNextOutcome, entry_kind: EdgeKind) -> Self {
        Self {
            entry_kind,
            attempt: 0,
            executor_session: None,
            executor_level: next
                .executor
                .as_ref()
                .map(|spec| spec.model_level)
                .unwrap_or_default(),
            eval_outcome: None,
            verdict: None,
            test_outcome: None,
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
        StepNode::Executor => {
            step_executor(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::StaticCheck => step_static_check(ctx, trace, cursor, step).await,
        StepNode::TestExecution => step_test_execution(ctx, trace, cursor, step).await,
        StepNode::Evaluator => {
            step_evaluator(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::VerdictGate => {
            step_verdict_gate(ctx, trace, cursor, step).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::PhaseLog => {
            step_phase_log(ctx, trace, cursor).await?;
            Ok(StepFlow::Proceed)
        }
        StepNode::Decision | StepNode::RetryGate | StepNode::WhitelistGate => Ok(StepFlow::Proceed),
    }
}

/// ① phase-next：run_id 会话窗口标识随行（进程内锚点随 run 存续——mid-phase
/// interruption 由重入窗口比对标定）。
async fn step_phase_next(
    tools: &Arc<dyn ToolStepPort>,
    request: &RunRequest,
) -> Result<PhaseNextOutcome, Terminal> {
    run_tool(
        tools,
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
            &ctx.request.root,
            ToolCommand::StaticCheck {
                change_id: ctx.request.change_id.clone(),
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

/// ⑤ test-execution 步：确定性测试执行链门禁（步声明 `OnFail::FeedbackLoop`）—
/// —pass 收口（产物交 PhaseLog 步机械 checklist 代写，绿跑零 agent 会话）；fail /
/// error 走反馈边修复重跑，超限升格相位 fail。
async fn step_test_execution(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
    step: &StepDefinition,
) -> Result<StepFlow, Terminal> {
    let limit = feedback_limit(step, "test-execution")?;
    let mut feedback: u32 = 0;
    let mut fix_session: Option<String> = None;
    loop {
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
            &ctx.request.root,
            ToolCommand::TestExecution {
                change_id: ctx.request.change_id.clone(),
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
        if feedback >= limit {
            step_test_execution_upgraded_phase_log(
                ctx.tools,
                ctx.guard,
                trace,
                ctx.request,
                ctx.phase,
                cursor.attempt,
                &outcome,
                fix_session.as_deref(),
            )
            .await?;
            cursor.failed = true;
            return Ok(StepFlow::Upgraded);
        }
        feedback += 1;
        let fix_prompt = format!(
            "测试执行未通过（第 {feedback}/{limit} 次反馈修复，conclusion={}），\
             请根据以下结果明细修复后重新提交：\n\n{}\n\n全量报告见目录：{}",
            outcome.conclusion.as_str(),
            outcome.findings_detail,
            outcome.report_dir
        );
        let turn = run_worker(
            ctx.worker,
            ctx.guard,
            ctx.request,
            ctx.phase,
            cursor.attempt,
            WorkerRole::Executor,
            fix_prompt,
            cursor.executor_level,
            fix_session.clone(),
            trace,
            EdgeKind::Feedback,
        )
        .await?;
        fix_session = Some(turn.session_id.clone());
        cursor.executor_session = Some(turn.session_id);
    }
}

/// ⑥ evaluator 步：本相位 evaluator 会话（prompt 组装同 executor 单点）。
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
    Ok(())
}

/// ⑦ verdict 解析门步：终态语义由步声明（`OnFail::Terminal`）承载——解析失败
/// 显式失败停给用户，MUST NOT 臆测 verdict、MUST NOT 静默跳过；解析产物交
/// PhaseLog 步落账。
async fn step_verdict_gate(
    ctx: &StepContext<'_>,
    trace: &mut EdgeTrace,
    cursor: &mut PhaseCursor,
    step: &StepDefinition,
) -> Result<(), Terminal> {
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
            Ok(())
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
            match step.on_fail {
                // 终态停给用户（相位表 VerdictGate 声明 Terminal）；缺省 `None`
                // 亦显式失败收场——两条路径同为显式停给用户，不吞不猜
                Some(OnFail::Terminal) | None => {
                    Err(Terminal::failed(format!("verdict 解析失败: {error}")))
                }
                // 反馈边语义仅门禁（static-check / test-execution）步使用；误声明
                // 不臆造反馈对象，按显式失败收场
                Some(OnFail::FeedbackLoop { .. }) => Err(Terminal::failed(format!(
                    "verdict 解析失败: {error}（VerdictGate 步不支持反馈边语义）"
                ))),
            }
        }
    }
}

/// ⑧ phase-log 步：桌面代写落账（写通道唯一——经写面进程内直调）。两形态按携
/// 出产物分流：test-execution 相位走机械 checklist（绿跑零 agent 会话）；其余
/// 相位走 verdict 解析产物（executor + evaluator 双槽位随行，decision 槽位归
/// decision-log 单点挂账恒缺省）。
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
        let logged = run_phase_log(
            ctx.tools,
            ctx.request,
            test_execution_phase_log_input(ctx.phase, &outcome),
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
        emit_node(
            ctx.guard,
            ctx.phase,
            cursor.attempt,
            ChangeStepKind::PhaseLog,
            instance_no,
            StepSettle::failed(Some("缺评估产物".to_owned())),
        );
        return Err(Terminal::failed(format!(
            "phase \"{}\" 计划步漂移：PhaseLog 步缺 verdict / 门禁产物",
            ctx.phase
        )));
    };
    let evaluator_session = cursor
        .eval_outcome
        .as_ref()
        .map(|outcome| outcome.session_id.clone());
    let failed = checklist.verdict == Verdict::Fail;
    let logged = run_phase_log(
        ctx.tools,
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
/// MUST NOT 由 walker 侧常量兜底（反馈边计数上限随门禁 step 声明退役，两门禁相
/// 位各自的独立预算不再驻 walker）。
fn feedback_limit(step: &StepDefinition, label: &str) -> Result<u32, Terminal> {
    match step.on_fail {
        Some(OnFail::FeedbackLoop { limit }) => Ok(limit),
        Some(OnFail::Terminal) | None => Err(Terminal::failed(format!(
            "计划图漂移：{label} 步缺 OnFail::FeedbackLoop 声明"
        ))),
    }
}

/// phase-log 落账单点（写通道唯一：落账经写面进程内直调）；步状态行 emit 归调
/// 用方。
async fn run_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    request: &RunRequest,
    input: PhaseLogInput,
) -> Result<PhaseLogOutcome, Terminal> {
    run_tool(
        tools,
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
/// 过（V1 恒真）；report 携 conclusion + 计数 + 报告路径摘要；三会话槽位恒
/// `None`（绿跑零 agent）。
fn test_execution_phase_log_input(phase: &str, outcome: &TestExecutionOutcome) -> PhaseLogInput {
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
                evidence: "V1 不移植 mutation，报告位恒 null（spec V1 边界）".to_owned(),
            },
        ],
        skipped: false,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
    }
}

/// 超限升格落账：桌面代写 fail checklist（测试执行反馈边超限），随后回
/// phase-next 进重试 / 决策分叉。fail 条目仅携 executor 槽位（携反馈会话 id；
/// evaluator 未跑无会话可记）。
#[allow(clippy::too_many_arguments)]
async fn step_test_execution_upgraded_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    trace: &mut EdgeTrace,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    outcome: &TestExecutionOutcome,
    fix_session: Option<&str>,
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
        request,
        PhaseLogInput {
            phase: phase.to_owned(),
            report: format!(
                "测试执行反馈边超限（连续修复未通过，conclusion={}），相位升格 fail；报告目录 {}",
                outcome.conclusion.as_str(),
                outcome.report_dir
            ),
            checklist: vec![ChecklistItem {
                item: "测试执行".to_owned(),
                pass: false,
                evidence: diagnose_brief(&outcome.findings_brief),
            }],
            skipped: false,
            executor_session_id: fix_session.map(ToOwned::to_owned),
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

// ---------------------------------------------------------------------------
// 决策分叉（D7：仅 phase-next 返回 max_retries_exceeded 时唤起决策 agent）
// ---------------------------------------------------------------------------

enum Deadlock {
    /// backtrack / retry 已执行（或无需执行），回外层 phase-next。
    Continue,
    Terminal(Terminal),
}

/// phase-next 重试上限分叉：唤起决策 agent——有界输入（fail checklist +
/// 写面下发白名单 + 候选相位最近 eval report），四动作封闭集分叉。
#[allow(clippy::too_many_arguments)]
async fn resolve_deadlock(
    worker: &Arc<dyn WorkerAgentPort>,
    tools: &Arc<dyn ToolStepPort>,
    snapshot: &Arc<dyn WorkflowSnapshotPort>,
    guard: &RunGuard,
    request: &RunRequest,
    next: &PhaseNextOutcome,
    error: &PhaseNextError,
    allowed: &[String],
    trace: &mut EdgeTrace,
) -> Deadlock {
    let PhaseNextError::MaxRetriesExceeded {
        phase: fail_phase, ..
    } = error;
    if next.last_result.is_none() {
        return Deadlock::Terminal(Terminal::failed(
            "phase-next 报 max_retries_exceeded 但缺 last_result 快照，无法组装决策输入".to_owned(),
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

    // 白名单预校验（walker 侧第一道；写面 backtrack 二次校验兜底）
    let gate_instance = begin_node(
        guard,
        trace,
        fail_phase,
        input.attempt,
        ChangeStepKind::WhitelistGate,
        EdgeKind::Forward,
    );
    emit_node(
        guard,
        fail_phase,
        input.attempt,
        ChangeStepKind::WhitelistGate,
        gate_instance,
        StepSettle::running(),
    );
    if let Err(message) = ensure_backtrack_allowed(&action, allowed) {
        emit_node(
            guard,
            fail_phase,
            input.attempt,
            ChangeStepKind::WhitelistGate,
            gate_instance,
            StepSettle::failed(Some(diagnose_brief(&message))),
        );
        return Deadlock::Terminal(Terminal::failed(format!("决策越权被拒: {message}")));
    }
    emit_node(
        guard,
        fail_phase,
        input.attempt,
        ChangeStepKind::WhitelistGate,
        gate_instance,
        StepSettle::passed(None),
    );

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
                &request.root,
                ToolCommand::Backtrack {
                    change_id: request.change_id.clone(),
                    phase: fail_phase.clone(),
                    input: BacktrackInput {
                        phase: fail_phase.clone(),
                        to,
                        reason,
                        allowed: allowed.to_vec(),
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
                    // 别于计划序与 retry）
                    trace.incoming = Some(EdgeKind::Backtrack);
                    Deadlock::Continue
                }
                Err(terminal) => Deadlock::Terminal(terminal),
            }
        }
        DecisionAction::Retry => Deadlock::Continue,
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

/// 决策会话：ask 动作 → UI 中断（`wait_answer`）→ 应答文本 Continue 同会话
/// 重出封闭集；取消信号置位 → 受控 stopped。每轮收口即 `decision-log` 挂账
///（置于 parse 之前——parse 失败路径同样留痕；ask 续轮同会话同值幂等重挂；
/// 失败 / 停止收口的 decision 会话不挂账，转录仍在库内可经调试页 change 筛
/// 选回查）。
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
    loop {
        let outcome = run_worker(
            worker,
            guard,
            request,
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
        // 决策会话槽位挂账（写面单点：最新 eval 条目定点改写，幂等覆写）
        let _: DecisionLogOutcome = run_tool(
            tools,
            &request.root,
            ToolCommand::DecisionLog {
                change_id: request.change_id.clone(),
                phase: input.phase.clone(),
                session_id: outcome.session_id.clone(),
            },
            "decision-log",
        )
        .await?;
        let text = outcome
            .final_message
            .clone()
            .or_else(|| final_assistant_text(&outcome.transcript))
            .unwrap_or_default();
        match parse_decision(&text) {
            Ok(DecisionAction::Ask { question, options }) => {
                guard.emit(RunUpdate::Ask {
                    question: question.clone(),
                    options: options.clone(),
                });
                match guard.wait_answer().await {
                    None => return Err(Terminal::stopped("ask 停等被终止")),
                    Some(answer) => {
                        prompt = format!(
                            "用户应答：{answer}\n\n请基于应答重新输出决策 JSON（四选一封闭集）。"
                        );
                        continue_session = Some(outcome.session_id);
                    }
                }
            }
            Ok(action) => return Ok(action),
            Err(error) => return Err(Terminal::failed(format!("决策解析失败: {error}"))),
        }
    }
}

/// 决策有界输入组装
fn build_decision_input(
    detail: &ChangeDetail,
    fail_phase: &str,
    allowed: &[String],
) -> DecisionInput {
    let station = detail
        .pipeline
        .iter()
        .find(|station| station.phase == fail_phase);
    let latest = station.and_then(|station| station.attempts.last());
    let attempt = latest.and_then(|record| record.attempt).unwrap_or(0);
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
    for phase in allowed {
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
        root: request.root.clone(),
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
    let (settle, status) = match outcome.status {
        AgentRunStatus::Completed => (StepSettle::passed(None), outcome.status),
        AgentRunStatus::Failed => (
            StepSettle::failed(Some("会话失败收敛".to_owned())),
            outcome.status,
        ),
        AgentRunStatus::Stopped => (
            StepSettle::stopped(Some("会话被终止".to_owned())),
            outcome.status,
        ),
        AgentRunStatus::Running => (
            StepSettle::failed(Some("会话收口缺终态".to_owned())),
            outcome.status,
        ),
    };
    emit_node(
        guard,
        phase,
        attempt,
        kind,
        instance_no,
        settle.with_session(Some(outcome.session_id.clone())),
    );
    match status {
        AgentRunStatus::Completed => Ok(outcome),
        AgentRunStatus::Stopped => Err(Terminal::stopped("会话被终止")),
        AgentRunStatus::Failed | AgentRunStatus::Running => {
            Err(Terminal::failed("会话失败收敛".to_owned()))
        }
    }
}

/// 工具步执行：产出按封闭集窄化（漂移显式失败）；步失败 `Err` 串映射终态。
async fn run_tool<V>(
    tools: &Arc<dyn ToolStepPort>,
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
        })
        .await
    {
        Ok(output) => output,
        Err(error) => return Err(Terminal::failed(format!("{label} 失败: {error}"))),
    };
    V::try_from(output)
        .map_err(|_| Terminal::failed(format!("{label} 输出漂移：步命令与产出类型不匹配")))
}

/// 取消观测：取消信号置位 → 受控 stopped。
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
