use std::path::Path;
use std::sync::Arc;

use foundation::layout;
use workflow::write::SessionAnchors;

use crate::port::{
    BoxToolFuture, StaticCheckRunner, TestExecutionRunner, ToolCommand, ToolStepOutput,
    ToolStepPort, ToolStepRequest,
};

/// 进程内工具步：绑定 run 级会话锚点（W7——锚点只在 phase_next 消费，每 run
/// 一个实例）与检查域两个 spawn 缝（static-check / test-execution）。
pub struct LocalToolSteps {
    anchors: Arc<SessionAnchors>,
    static_check: Arc<dyn StaticCheckRunner>,
    test_execution: Arc<dyn TestExecutionRunner>,
}

impl LocalToolSteps {
    /// 组合根装配（run 作用域一次：锚点随 run 铸新、runner 随进程复用）。
    pub fn new(
        anchors: Arc<SessionAnchors>,
        static_check: Arc<dyn StaticCheckRunner>,
        test_execution: Arc<dyn TestExecutionRunner>,
    ) -> Self {
        Self {
            anchors,
            static_check,
            test_execution,
        }
    }
}

impl ToolStepPort for LocalToolSteps {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture {
        let anchors = Arc::clone(&self.anchors);
        let static_check = Arc::clone(&self.static_check);
        let test_execution = Arc::clone(&self.test_execution);
        Box::pin(async move { execute(anchors, static_check, test_execution, step).await })
    }
}

/// 一次工具步执行：命令封闭集按臂分发——相位机四步 layout 解析后直调写面，
/// StaticCheck / TestExecution 委托注入 runner。
async fn execute(
    anchors: Arc<SessionAnchors>,
    static_check: Arc<dyn StaticCheckRunner>,
    test_execution: Arc<dyn TestExecutionRunner>,
    step: ToolStepRequest,
) -> Result<ToolStepOutput, String> {
    let layout = layout::resolve(Path::new(&step.root));
    match step.command {
        ToolCommand::PhaseNext { change, run_id } => {
            let outcome = workflow::write::phase_next(&layout, &change, &run_id, &anchors)?;
            Ok(ToolStepOutput::PhaseNext(Box::new(outcome)))
        }
        ToolCommand::PhaseStart { change, phase } => {
            let outcome = workflow::write::phase_start(&layout, &change, &phase)?;
            Ok(ToolStepOutput::PhaseStart(outcome))
        }
        ToolCommand::PhaseLog { change, input, .. } => {
            let outcome = workflow::write::phase_log(&layout, &change, &input)?;
            Ok(ToolStepOutput::PhaseLog(outcome))
        }
        ToolCommand::Backtrack { change, input, .. } => {
            let outcome = workflow::write::backtrack(&layout, &change, &input)?;
            Ok(ToolStepOutput::Backtrack(outcome))
        }
        ToolCommand::DecisionLog {
            change,
            phase,
            session_id,
        } => {
            let outcome = workflow::write::decision_log(&layout, &change, &phase, &session_id)?;
            Ok(ToolStepOutput::DecisionLog(outcome))
        }
        ToolCommand::StaticCheck => static_check.run(&step.root).await,
        ToolCommand::TestExecution { change } => test_execution.run(&step.root, &change).await,
    }
}
