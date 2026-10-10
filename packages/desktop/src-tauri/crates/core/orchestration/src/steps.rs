use std::sync::Arc;

use workflow::state::{ChangeStateStore, StepCommand, StepKind};
use workflow::write::SessionAnchors;

use crate::port::{
    BoxToolFuture, StaticCheckRunner, TestExecutionRunner, ToolCommand, ToolStepOutput,
    ToolStepPort, ToolStepRequest,
};

/// 进程内工具步骤
pub struct LocalToolSteps {
    anchors: Arc<SessionAnchors>,
    static_check: Arc<dyn StaticCheckRunner>,
    test_execution: Arc<dyn TestExecutionRunner>,
    store: Arc<dyn ChangeStateStore>,
    run_id: String,
}

impl LocalToolSteps {
    /// 组合根装配（run 作用域一次：锚点随 run 铸新、runner 随进程复用）。
    pub fn new(
        anchors: Arc<SessionAnchors>,
        static_check: Arc<dyn StaticCheckRunner>,
        test_execution: Arc<dyn TestExecutionRunner>,
        store: Arc<dyn ChangeStateStore>,
        run_id: String,
    ) -> Self {
        Self {
            anchors,
            static_check,
            test_execution,
            store,
            run_id,
        }
    }
}

impl ToolStepPort for LocalToolSteps {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture {
        let anchors = Arc::clone(&self.anchors);
        let static_check = Arc::clone(&self.static_check);
        let test_execution = Arc::clone(&self.test_execution);
        let store = Arc::clone(&self.store);
        let run_id = self.run_id.clone();
        Box::pin(async move {
            execute(anchors, static_check, test_execution, store, run_id, step).await
        })
    }
}

/// 执行一次工具步骤
async fn execute(
    anchors: Arc<SessionAnchors>,
    static_check: Arc<dyn StaticCheckRunner>,
    test_execution: Arc<dyn TestExecutionRunner>,
    store: Arc<dyn ChangeStateStore>,
    run_id: String,
    step: ToolStepRequest,
) -> Result<ToolStepOutput, String> {
    match step.command {
        ToolCommand::PhaseNext { change_id, run_id } => {
            let outcome =
                workflow::write::phase_next(store.as_ref(), &change_id, &run_id, &anchors);
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::PhaseNext,
                outcome.as_ref().map(|value| {
                    (
                        format!(
                            "phase_next → {}（round {}）",
                            value.next_phase.as_deref().unwrap_or("done"),
                            value.round
                        ),
                        None,
                    )
                }),
            );
            Ok(ToolStepOutput::PhaseNext(Box::new(outcome?)))
        }
        ToolCommand::PhaseStart { change_id, phase } => {
            let outcome = workflow::write::phase_start(store.as_ref(), &change_id, &phase);
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::PhaseStart,
                outcome.as_ref().map(|value| {
                    (
                        format!("phase_start {} attempt={}", value.phase, value.attempt),
                        None,
                    )
                }),
            );
            Ok(ToolStepOutput::PhaseStart(outcome?))
        }
        ToolCommand::PhaseLog {
            change_id, input, ..
        } => {
            let outcome = workflow::write::phase_log(store.as_ref(), &change_id, &input);
            let reference = input
                .evaluator_session_id
                .clone()
                .or_else(|| input.executor_session_id.clone());
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::PhaseLog,
                outcome.as_ref().map(|value| {
                    (
                        format!("phase_log {} attempt={}", value.phase, value.attempt),
                        reference,
                    )
                }),
            );
            Ok(ToolStepOutput::PhaseLog(outcome?))
        }
        ToolCommand::Backtrack {
            change_id, input, ..
        } => {
            let outcome = workflow::write::backtrack(store.as_ref(), &change_id, &input);
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::Backtrack,
                outcome.as_ref().map(|value| {
                    (
                        format!(
                            "backtrack {} → {}（{}）",
                            value.phase, value.target, input.reason
                        ),
                        None,
                    )
                }),
            );
            Ok(ToolStepOutput::Backtrack(outcome?))
        }
        ToolCommand::DecisionLog {
            change_id,
            phase,
            session_id,
        } => {
            let outcome =
                workflow::write::decision_log(store.as_ref(), &change_id, &phase, &session_id);
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::DecisionLog,
                outcome
                    .as_ref()
                    .map(|_| (format!("decision_log {phase}"), Some(session_id))),
            );
            Ok(ToolStepOutput::DecisionLog(outcome?))
        }
        ToolCommand::StaticCheck { change_id } => {
            // 审计行 change_id 归键（命令载荷携 id——空串占位退役，逐条
            // change id 可枚举），run_id 仍串链本 run 步骤序列
            let output = static_check.run(&step.root).await;
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::StaticCheck,
                match &output {
                    Ok(ToolStepOutput::StaticCheck(outcome)) => {
                        Ok((format!("static_check passed={}", outcome.passed), None))
                    }
                    Ok(_) => Err("static_check 输出漂移：产出类型不匹配".to_owned()),
                    Err(error) => Err(error.clone()),
                },
            );
            output
        }
        ToolCommand::TestExecution { change_id } => {
            // 磁盘面 port 收 name（报告目录派生为 name 化）：消费点经
            // id → 记录 → name 解析（未建档显式 Err）
            let name = store
                .get_change(&change_id)
                .map_err(|error| error.to_string())?
                .map(|record| record.name)
                .ok_or_else(|| {
                    format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从执行测试门禁")
                })?;
            let output = test_execution.run(&step.root, &name).await;
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::TestExecution,
                match &output {
                    Ok(ToolStepOutput::TestExecution(outcome)) => Ok((
                        format!(
                            "test_execution conclusion={} total={} passed={} failed={} skipped={}",
                            outcome.conclusion.as_str(),
                            outcome.total,
                            outcome.passed,
                            outcome.failed,
                            outcome.skipped
                        ),
                        Some(outcome.report_dir.clone()),
                    )),
                    Ok(_) => Err("test_execution 输出漂移：产出类型不匹配".to_owned()),
                    Err(error) => Err(error.clone()),
                },
            );
            output
        }
    }
}

/// 审计落行出口：成功行携摘要 + 引用（status `ok`），失败行携错误串（status
/// `error`）；best-effort——审计失败不阻断步本身（审计 only，不做恢复依据）。
fn audit_outcome<E: std::fmt::Display>(
    store: &dyn ChangeStateStore,
    run_id: &str,
    change_id: &str,
    step_kind: StepKind,
    outcome: Result<(String, Option<String>), E>,
) {
    let (status, summary, reference) = match outcome {
        Ok((summary, reference)) => ("ok", summary, reference),
        Err(error) => ("error", error.to_string(), None),
    };
    let _ = store.append_step(&StepCommand {
        run_id: run_id.to_owned(),
        change_id: change_id.to_owned(),
        step_kind,
        status: status.to_owned(),
        summary: clip_summary(summary),
        reference,
        timestamp: now_millis(),
    });
}

/// 摘要有界截断（design D10）：≤500 字符（`chars().count()` 口径）原样透传；
/// 超出截断前 500 字符并追加 `…（截断，共 N 字符）` 留痕（N 为原文全长）。
fn clip_summary(summary: String) -> String {
    const MAX_CHARS: usize = 500;
    let total = summary.chars().count();
    if total <= MAX_CHARS {
        return summary;
    }
    format!(
        "{}…（截断，共 {total} 字符）",
        summary.chars().take(MAX_CHARS).collect::<String>()
    )
}

/// 当前 UTC unix 毫秒（时钟早于 epoch 取 0，不 panic）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
