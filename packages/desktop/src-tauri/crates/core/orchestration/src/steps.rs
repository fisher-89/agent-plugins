//! 进程内工具步适配：[`ToolStepPort`] 的本地实现——相位机四步
//!（phase-next / phase-start / phase-log / backtrack）直调 `workflow::write`
//! 写面（AC-6 写通道唯一，零 CLI 子进程），static-check 步委托注入的
//! [`StaticCheckRunner`]（spawn 留 infra，W4 红线：本 crate 零进程 spawn）。

use std::path::Path;
use std::sync::Arc;

use foundation::layout;
use workflow::write::SessionAnchors;

use crate::port::{
    BoxToolFuture, StaticCheckRunner, ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest,
};

/// 进程内工具步：绑定 run 级会话锚点（W7——锚点只在 phase_next 消费，每 run
/// 一个实例）与 static-check spawn 缝。
pub struct LocalToolSteps {
    anchors: Arc<SessionAnchors>,
    static_check: Arc<dyn StaticCheckRunner>,
}

impl LocalToolSteps {
    /// 组合根装配（run 作用域一次：锚点随 run 铸新、runner 随进程复用）。
    pub fn new(anchors: Arc<SessionAnchors>, static_check: Arc<dyn StaticCheckRunner>) -> Self {
        Self {
            anchors,
            static_check,
        }
    }
}

impl ToolStepPort for LocalToolSteps {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture {
        let anchors = Arc::clone(&self.anchors);
        let static_check = Arc::clone(&self.static_check);
        Box::pin(async move { execute(anchors, static_check, step).await })
    }
}

/// 一次工具步执行：命令封闭集按臂分发——相位机四步 layout 解析后直调写面，
/// StaticCheck 委托注入 runner。
async fn execute(
    anchors: Arc<SessionAnchors>,
    static_check: Arc<dyn StaticCheckRunner>,
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
    }
}
