use std::sync::Arc;

use agent::{AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance};
use workflow::model::ChecklistItem;
use workflow::queries::ChangeDetail;
use workflow::write::{
    BacktrackInput, BacktrackOutcome, DecisionLogOutcome, PhaseLogInput, PhaseLogOutcome,
    PhaseNextError, PhaseNextOutcome, PhaseStartOutcome,
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
use crate::prompt::{decision_prompt, evaluator_prompt, executor_prompt};
use crate::run_history;
use crate::state::{ChangeRunStatus, ChangeStepKind, ChangeStepState, ChangeStepStatus, RunUpdate};
use crate::transcript::final_assistant_text;
use crate::verdict::parse_verdict;

/// static-check 定向反馈边独立上限（沿原 SubagentStop hook `loop_limit`
/// 语义；与相位 retry 预算分账，超限才升格相位 fail）。
pub const STATIC_CHECK_FEEDBACK_LIMIT: u32 = 5;

/// static-check 步门控布局常量（implement / test-gen 相位 executor 收口后
/// 必经；布局词汇，非路由权威——门控与否以本常量定性，相位推进仍问
/// phase-next）。
pub const STATIC_CHECK_PHASES: [&str; 2] = ["implement", "test-gen"];

/// test-execution 反馈边独立上限（与 [`STATIC_CHECK_FEEDBACK_LIMIT`] 分立、
/// 各自计满各自升格；不计相位 retry 预算，沿「反馈边不计相位 retry 预算」
/// 先例，取值对齐 static-check）。
pub const TEST_EXECUTION_FEEDBACK_LIMIT: u32 = 5;

/// test-execution 相位门禁布局常量（对齐插件工作流的 test-execution 独立
/// 相位语义；布局词汇非路由权威，相位推进仍问 phase-next）。
pub const TEST_EXECUTION_PHASES: [&str; 1] = ["test-execution"];

/// 会话 provenance 来源（sourceRef = `<id>/<phase>/<role>/<attempt>`——身份段
/// 恒 change id）。
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

/// 相位循环主入口（每 run 两写时序，unify-run-state-persistence D5/D6）：
/// 起点第一写（running 行，失败 fail-fast 收口）→ drive（既有循环零改动，
/// emit_step 经 control 累积 + Notice 广播）→ 终态出口单点第二写（累积器快
/// 照 → run_history 组装 → best-effort 落包）→ `guard.finish`（既有唯一终
/// 态出口，零改动）。
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
    // 终态出口单点第二写：终态 + 步整包 + active_phase 清位单事务落库。
    // best-effort（D6）：Err 静默不阻断收口——guard.finish 照常，run 终态与
    // 订阅释放不受损；start 行残留 running 经启动标定 interrupted 自愈留痕。
    let steps = guard.steps();
    let command =
        run_history::finish_command(&request, status, reason.clone(), now_millis(), &steps);
    if let Err(error) = history.run_finished(&command) {
        // D6：finish 落包失败收敛 = best-effort 单次尝试，无重试无错误面
        let _ = error;
    }
    drop(control);
    guard.finish(status, reason);
    status
}

/// 循环体：终态一律以 `(status, reason)` 返回，不触 finish。
async fn drive(
    worker: &Arc<dyn WorkerAgentPort>,
    tools: &Arc<dyn ToolStepPort>,
    snapshot: &Arc<dyn WorkflowSnapshotPort>,
    guard: &RunGuard,
    request: &RunRequest,
) -> (ChangeRunStatus, Option<String>) {
    let mut allowed: Vec<String> = Vec::new();
    loop {
        if let Err(terminal) = check_cancel(guard) {
            return terminal.into_pair();
        }
        // ① phase-next（路由唯一权威；白名单随响应缓存）
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
            // ⑦ 决策分叉（max_retries_exceeded → 决策 agent；路由上限 /
            //    backtrack 目标非法等失败路径已在步执行体显式 Err 终态）
            match resolve_deadlock(
                worker, tools, snapshot, guard, request, &next, error, &allowed,
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
        let Some(evaluator) = &next.evaluator else {
            return Terminal::failed(format!("phase \"{phase}\" 无 evaluator 定义，无法评估落账"))
                .into_pair();
        };
        allowed = next.allowed_backtrack_phases.clone();

        // ② phase-start（开启阶段、attempt 计时）
        let attempt = match step_phase_start(tools, guard, request, &phase).await {
            Ok(attempt) => attempt,
            Err(terminal) => return terminal.into_pair(),
        };

        // ③–⑥ 相位主体分叉：test-execution 相位走确定性门禁步（绿跑零 agent
        // 会话，executor / evaluator 会话与 verdict 解析门全部跳过）；其余相位
        // 走 executor / evaluator 会话主链
        if TEST_EXECUTION_PHASES.contains(&phase.as_str()) {
            // 修复边续注 executor 会话：等级随相位表 executor spec（缺 executor
            // 定义的漂移形态退 High 档）
            let executor_level = next
                .executor
                .as_ref()
                .map(|spec| spec.model_level)
                .unwrap_or_default();
            match test_execution_loop(
                worker,
                tools,
                guard,
                request,
                &phase,
                attempt,
                executor_level,
            )
            .await
            {
                Ok(TestExecutionFlow::Proceed) => {}
                Ok(TestExecutionFlow::Upgraded) => continue, // fail 已落账，回 phase-next 分叉
                Err(terminal) => return terminal.into_pair(),
            }
        } else {
            // ③ executor 会话
            let mut executor_session: Option<String> = None;
            if let Some(executor) = &next.executor {
                let prompt = executor_prompt(&executor.agent_type, &executor.prompt);
                let outcome = match run_worker(
                    worker,
                    guard,
                    request,
                    &phase,
                    attempt,
                    WorkerRole::Executor,
                    prompt,
                    executor.model_level,
                    None,
                )
                .await
                {
                    Ok(outcome) => outcome,
                    Err(terminal) => return terminal.into_pair(),
                };

                // ④ static-check 定向反馈边（implement / test-gen 门控）
                if STATIC_CHECK_PHASES.contains(&phase.as_str()) {
                    let loop_in = FeedbackLoop {
                        worker,
                        tools,
                        guard,
                        request,
                        phase: &phase,
                        attempt,
                        executor_session: outcome.session_id.clone(),
                        model_level: executor.model_level,
                    };
                    match static_check_loop(loop_in).await {
                        Ok(StaticCheckFlow::Proceed) => {}
                        Ok(StaticCheckFlow::Upgraded) => continue, // fail 已落账，回 phase-next 分叉
                        Err(terminal) => return terminal.into_pair(),
                    }
                }
                executor_session = Some(outcome.session_id);
            }

            // ⑤ evaluator 会话
            let eval_prompt = evaluator_prompt(&evaluator.prompt);
            let outcome = match run_worker(
                worker,
                guard,
                request,
                &phase,
                attempt,
                WorkerRole::Evaluator,
                eval_prompt,
                evaluator.model_level,
                None,
            )
            .await
            {
                Ok(outcome) => outcome,
                Err(terminal) => return terminal.into_pair(),
            };

            // ⑥ verdict 解析门 → 桌面代写 phase-log（executor + evaluator 双槽位
            // 随行落账，decision 槽位归 decision-log 单点挂账恒缺省）
            if let Err(terminal) = step_verdict_phase_log(
                tools,
                guard,
                request,
                &phase,
                attempt,
                executor_session,
                &outcome,
            )
            .await
            {
                return terminal.into_pair();
            }
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
// 步执行体
// ---------------------------------------------------------------------------

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

/// ② phase-start：开启阶段并取回 attempt 计时。
async fn step_phase_start(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    phase: &str,
) -> Result<u32, Terminal> {
    emit_step(
        guard,
        phase,
        0,
        ChangeStepKind::PhaseStart,
        ChangeStepStatus::Running,
        None,
        None,
    );
    let outcome: PhaseStartOutcome = run_tool(
        tools,
        &request.root,
        ToolCommand::PhaseStart {
            change_id: request.change_id.clone(),
            phase: phase.to_owned(),
        },
        "phase-start",
    )
    .await?;
    emit_step(
        guard,
        phase,
        outcome.attempt,
        ChangeStepKind::PhaseStart,
        ChangeStepStatus::Passed,
        None,
        Some(format!("attempt {} 已开跑", outcome.attempt)),
    );
    Ok(outcome.attempt)
}

/// ④ static-check 定向反馈边的输入面（参数打包，门控阈值 8 参收窄）。
struct FeedbackLoop<'a> {
    worker: &'a Arc<dyn WorkerAgentPort>,
    tools: &'a Arc<dyn ToolStepPort>,
    guard: &'a RunGuard,
    request: &'a RunRequest,
    phase: &'a str,
    attempt: u32,
    executor_session: String,
    /// 续注 executor 会话的模型档位（反馈边不换档）
    model_level: ModelLevel,
}

/// ④ static-check 定向反馈边：失败诊断 Continue 注入同一 executor 会话修复
/// （≤5 次独立计数）；超限按 D8 升格——桌面代写 fail phase-log（不跑
/// evaluator），回 phase-next 进重试 / 决策分叉。
async fn static_check_loop(loop_in: FeedbackLoop<'_>) -> Result<StaticCheckFlow, Terminal> {
    let FeedbackLoop {
        worker,
        tools,
        guard,
        request,
        phase,
        attempt,
        executor_session,
        model_level,
    } = loop_in;
    let mut feedback: u32 = 0;
    loop {
        emit_step(
            guard,
            phase,
            attempt,
            ChangeStepKind::StaticCheck,
            ChangeStepStatus::Running,
            None,
            None,
        );
        let outcome: StaticCheckOutcome = run_tool(
            tools,
            &request.root,
            ToolCommand::StaticCheck {
                change_id: request.change_id.clone(),
            },
            "static-check",
        )
        .await?;
        if outcome.passed {
            emit_step(
                guard,
                phase,
                attempt,
                ChangeStepKind::StaticCheck,
                ChangeStepStatus::Passed,
                None,
                Some("静态检查通过".to_owned()),
            );
            return Ok(StaticCheckFlow::Proceed);
        }
        emit_step(
            guard,
            phase,
            attempt,
            ChangeStepKind::StaticCheck,
            ChangeStepStatus::Failed,
            None,
            Some(diagnose_brief(&outcome.diagnostics)),
        );
        if feedback >= STATIC_CHECK_FEEDBACK_LIMIT {
            // 升格相位 fail（桌面代写，不跑 evaluator；消耗相位 retry 预算；
            // 仅携 executor 槽位——evaluator 未跑无会话可记）
            step_fail_phase_log(
                tools,
                guard,
                request,
                phase,
                attempt,
                &executor_session,
                &outcome.diagnostics,
            )
            .await?;
            return Ok(StaticCheckFlow::Upgraded);
        }
        feedback += 1;
        let fix_prompt = format!(
            "静态检查未通过（第 {feedback}/{STATIC_CHECK_FEEDBACK_LIMIT} 次反馈修复），请修复以下错误后重新提交：\n\n{}",
            outcome.diagnostics
        );
        run_worker(
            worker,
            guard,
            request,
            phase,
            attempt,
            WorkerRole::Executor,
            fix_prompt,
            model_level,
            Some(executor_session.clone()),
        )
        .await?;
    }
}

/// ⑥ verdict 解析门 → 桌面代写 phase-log（写通道唯一：落账经写面进程内直调）。
/// 会话槽位随行：executor 槽位携出自主循环、evaluator 槽位取本次收口会话
///（`WorkerTurnOutcome.session_id`），decision 槽位恒 `None`。
async fn step_verdict_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    executor_session: Option<String>,
    outcome: &WorkerTurnOutcome,
) -> Result<(), Terminal> {
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::VerdictGate,
        ChangeStepStatus::Running,
        None,
        None,
    );
    let text = outcome
        .final_message
        .clone()
        .or_else(|| final_assistant_text(&outcome.transcript))
        .unwrap_or_default();
    let checklist = match parse_verdict(&text) {
        Ok(checklist) => checklist,
        Err(error) => {
            emit_step(
                guard,
                phase,
                attempt,
                ChangeStepKind::VerdictGate,
                ChangeStepStatus::Failed,
                None,
                Some(diagnose_brief(&error)),
            );
            return Err(Terminal::failed(format!("verdict 解析失败: {error}")));
        }
    };
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::VerdictGate,
        ChangeStepStatus::Passed,
        None,
        Some(format!("verdict={}", checklist.verdict.as_str())),
    );
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        ChangeStepStatus::Running,
        None,
        None,
    );
    let logged: PhaseLogOutcome = run_tool(
        tools,
        &request.root,
        ToolCommand::PhaseLog {
            change_id: request.change_id.clone(),
            phase: phase.to_owned(),
            input: PhaseLogInput {
                phase: phase.to_owned(),
                report: checklist.report,
                checklist: checklist.checklist,
                skipped: false,
                executor_session_id: executor_session,
                evaluator_session_id: Some(outcome.session_id.clone()),
                decision_session_id: None,
            },
        },
        "phase-log",
    )
    .await?;
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        ChangeStepStatus::Passed,
        None,
        Some(format!("attempt {} 已落账", logged.attempt)),
    );
    Ok(())
}

/// D8 升格落账：桌面代写 fail checklist（静态检查反馈边超限），随后回
/// phase-next 进重试 / 决策分叉。fail 条目仅携 executor 槽位（反馈边续注
/// 同一 executor 会话，id 稳定；evaluator 未跑）。
async fn step_fail_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    executor_session: &str,
    diagnostics: &str,
) -> Result<(), Terminal> {
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        ChangeStepStatus::Running,
        None,
        Some("静态检查反馈边超限，升格相位 fail".to_owned()),
    );
    let _: PhaseLogOutcome = run_tool(
        tools,
        &request.root,
        ToolCommand::PhaseLog {
            change_id: request.change_id.clone(),
            phase: phase.to_owned(),
            input: PhaseLogInput {
                phase: phase.to_owned(),
                report: "静态检查反馈边超限（连续修复未通过），相位升格 fail".to_owned(),
                checklist: vec![ChecklistItem {
                    item: "静态检查".to_owned(),
                    pass: false,
                    evidence: diagnostics.to_owned(),
                }],
                skipped: false,
                executor_session_id: Some(executor_session.to_owned()),
                evaluator_session_id: None,
                decision_session_id: None,
            },
        },
        "phase-log",
    )
    .await?;
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        ChangeStepStatus::Passed,
        None,
        None,
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
    let action = match decision_session(worker, tools, guard, request, &input).await {
        Ok(action) => action,
        Err(terminal) => return Deadlock::Terminal(terminal),
    };

    // 白名单预校验（walker 侧第一道；写面 backtrack 二次校验兜底）
    emit_step(
        guard,
        fail_phase,
        input.attempt,
        ChangeStepKind::WhitelistGate,
        ChangeStepStatus::Running,
        None,
        None,
    );
    if let Err(message) = ensure_backtrack_allowed(&action, allowed) {
        emit_step(
            guard,
            fail_phase,
            input.attempt,
            ChangeStepKind::WhitelistGate,
            ChangeStepStatus::Failed,
            None,
            Some(diagnose_brief(&message)),
        );
        return Deadlock::Terminal(Terminal::failed(format!("决策越权被拒: {message}")));
    }
    emit_step(
        guard,
        fail_phase,
        input.attempt,
        ChangeStepKind::WhitelistGate,
        ChangeStepStatus::Passed,
        None,
        None,
    );

    match action {
        DecisionAction::Backtrack { to, reason } => {
            emit_step(
                guard,
                fail_phase,
                input.attempt,
                ChangeStepKind::RetryGate,
                ChangeStepStatus::Running,
                None,
                Some(format!("决策 backtrack → {to}")),
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
                    emit_step(
                        guard,
                        fail_phase,
                        input.attempt,
                        ChangeStepKind::RetryGate,
                        ChangeStepStatus::Passed,
                        None,
                        Some("回溯已落账，回 phase-next 重路由".to_owned()),
                    );
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
/// provenance `<id>/<phase>/<role>/<attempt>`（身份段恒 change id）、取消与
/// 会话失败收敛终态。
/// 步状态随行 emit（running → passed / failed / stopped）；会话 id 槽由命令
/// 层 sink 桥随首个会话事件先行同步（停止寻址不依赖 turn 收口）。
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
) -> Result<WorkerTurnOutcome, Terminal> {
    let kind = match role {
        WorkerRole::Executor => ChangeStepKind::Executor,
        WorkerRole::Evaluator => ChangeStepKind::Evaluator,
        WorkerRole::Decision => ChangeStepKind::Decision,
    };
    emit_step(
        guard,
        phase,
        attempt,
        kind,
        ChangeStepStatus::Running,
        None,
        None,
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
            emit_step(
                guard,
                phase,
                attempt,
                kind,
                ChangeStepStatus::Failed,
                None,
                Some(diagnose_brief(&error)),
            );
            return Err(Terminal::failed(format!("会话执行失败: {error}")));
        }
    };
    let (status, detail) = match outcome.status {
        AgentRunStatus::Completed => (ChangeStepStatus::Passed, None),
        AgentRunStatus::Failed => (ChangeStepStatus::Failed, Some("会话失败收敛".to_owned())),
        AgentRunStatus::Stopped => (ChangeStepStatus::Stopped, Some("会话被终止".to_owned())),
        AgentRunStatus::Running => (ChangeStepStatus::Failed, Some("会话收口缺终态".to_owned())),
    };
    emit_step(
        guard,
        phase,
        attempt,
        kind,
        status,
        Some(outcome.session_id.clone()),
        detail,
    );
    match outcome.status {
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

/// 步状态行 emit。
#[allow(clippy::too_many_arguments)]
fn emit_step(
    guard: &RunGuard,
    phase: &str,
    attempt: u32,
    step: ChangeStepKind,
    status: ChangeStepStatus,
    session_id: Option<String>,
    detail: Option<String>,
) {
    guard.emit(RunUpdate::Step {
        step: ChangeStepState {
            phase: phase.to_owned(),
            attempt,
            step,
            status,
            session_id,
            detail,
        },
    });
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

/// static-check 反馈边闭环后的携出物。
enum StaticCheckFlow {
    /// 通过，继续 evaluator 会话。
    Proceed,
    /// 超限升格（fail 已落账），回外层 phase-next。
    Upgraded,
}

/// test-execution 门禁闭环后的携出物。
enum TestExecutionFlow {
    /// pass 已机械 checklist 落账，回外层停等确认。
    Proceed,
    /// 超限升格（fail 已落账），回外层 phase-next。
    Upgraded,
}

/// test-execution 相位确定性门禁循环（`model_level` 随相位表 executor spec——
/// 修复边续注 executor 会话不换档）
async fn test_execution_loop(
    worker: &Arc<dyn WorkerAgentPort>,
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    model_level: ModelLevel,
) -> Result<TestExecutionFlow, Terminal> {
    let mut feedback: u32 = 0;
    let mut fix_session: Option<String> = None;
    loop {
        emit_step(
            guard,
            phase,
            attempt,
            ChangeStepKind::TestExecution,
            ChangeStepStatus::Running,
            None,
            None,
        );
        let outcome: TestExecutionOutcome = run_tool(
            tools,
            &request.root,
            ToolCommand::TestExecution {
                change_id: request.change_id.clone(),
            },
            "test-execution",
        )
        .await?;
        if outcome.conclusion == TestExecutionConclusion::Pass {
            emit_step(
                guard,
                phase,
                attempt,
                ChangeStepKind::TestExecution,
                ChangeStepStatus::Passed,
                None,
                Some(format!(
                    "conclusion=pass total={} passed={} failed={} skipped={}",
                    outcome.total, outcome.passed, outcome.failed, outcome.skipped
                )),
            );
            step_test_execution_phase_log(tools, guard, request, phase, attempt, &outcome).await?;
            return Ok(TestExecutionFlow::Proceed);
        }
        emit_step(
            guard,
            phase,
            attempt,
            ChangeStepKind::TestExecution,
            ChangeStepStatus::Failed,
            None,
            Some(diagnose_brief(&outcome.findings_brief)),
        );
        if feedback >= TEST_EXECUTION_FEEDBACK_LIMIT {
            step_test_execution_upgraded_phase_log(
                tools,
                guard,
                request,
                phase,
                attempt,
                &outcome,
                fix_session.as_deref(),
            )
            .await?;
            return Ok(TestExecutionFlow::Upgraded);
        }
        feedback += 1;
        let fix_prompt = format!(
            "测试执行未通过（第 {feedback}/{TEST_EXECUTION_FEEDBACK_LIMIT} 次反馈修复，conclusion={}），\
             请根据以下结果明细修复后重新提交：\n\n{}\n\n全量报告见目录：{}",
            outcome.conclusion.as_str(),
            outcome.findings_detail,
            outcome.report_dir
        );
        let turn = run_worker(
            worker,
            guard,
            request,
            phase,
            attempt,
            WorkerRole::Executor,
            fix_prompt,
            model_level,
            fix_session.clone(),
        )
        .await?;
        fix_session = Some(turn.session_id);
    }
}

/// pass 机械 checklist 代写 phase_log：三条目全 pass（verdict 由 checklist
/// 全 pass 推导）——① suite 结论一致 ② 聚合 conclusion 与计数一致
/// ③ mutation null 自动通过（V1 恒真）；report 携 conclusion + 计数 + 报告
/// 路径摘要；三会话槽位恒 `None`（绿跑零 agent）。
async fn step_test_execution_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    outcome: &TestExecutionOutcome,
) -> Result<(), Terminal> {
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        ChangeStepStatus::Running,
        None,
        None,
    );
    let _: PhaseLogOutcome = run_tool(
        tools,
        &request.root,
        ToolCommand::PhaseLog {
            change_id: request.change_id.clone(),
            phase: phase.to_owned(),
            input: PhaseLogInput {
                phase: phase.to_owned(),
                report: format!(
                    "测试执行结论 pass：total={} passed={} failed={} skipped={}；报告目录 {}",
                    outcome.total,
                    outcome.passed,
                    outcome.failed,
                    outcome.skipped,
                    outcome.report_dir
                ),
                checklist: vec![
                    ChecklistItem {
                        item: "suite 结论一致".to_owned(),
                        pass: true,
                        evidence:
                            "各 suite 子报告非 error 且与聚合 conclusion 一致（确定性门禁步校验）"
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
            },
        },
        "phase-log",
    )
    .await?;
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        ChangeStepStatus::Passed,
        None,
        None,
    );
    Ok(())
}

/// 超限升格落账：桌面代写 fail checklist（测试执行反馈边超限），随后回
/// phase-next 进重试 / 决策分叉。fail 条目仅携 executor 槽位（携反馈会话
/// id；evaluator 未跑无会话可记）。
async fn step_test_execution_upgraded_phase_log(
    tools: &Arc<dyn ToolStepPort>,
    guard: &RunGuard,
    request: &RunRequest,
    phase: &str,
    attempt: u32,
    outcome: &TestExecutionOutcome,
    fix_session: Option<&str>,
) -> Result<(), Terminal> {
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        ChangeStepStatus::Running,
        None,
        Some("测试执行反馈边超限，升格相位 fail".to_owned()),
    );
    let _: PhaseLogOutcome = run_tool(
        tools,
        &request.root,
        ToolCommand::PhaseLog {
            change_id: request.change_id.clone(),
            phase: phase.to_owned(),
            input: PhaseLogInput {
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
        },
        "phase-log",
    )
    .await?;
    emit_step(
        guard,
        phase,
        attempt,
        ChangeStepKind::PhaseLog,
        ChangeStepStatus::Passed,
        None,
        None,
    );
    Ok(())
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
