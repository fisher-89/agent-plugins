use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent::{KernelOutput, SessionCtx, SessionRef, TurnOutcome};
use orchestration::port::{
    BoxTurnFuture, RunEventSink, WorkerAgentPort, WorkerTurnOutcome, WorkerTurnRequest,
};
use orchestration::state::RunUpdate;
use orchestration::transcript::final_assistant_text;

use crate::compose::ComposedTurn;

/// WorkerAgent 会话执行器：绑定组合 turn + run 事件出口。
pub struct KernelWorkerPort {
    composed: Arc<ComposedTurn>,
    sink: Arc<dyn RunEventSink>,
}

impl KernelWorkerPort {
    /// 组合根装配（命令层以 run 作用域一次 compose_turn 后注入）。
    pub fn new(composed: ComposedTurn, sink: Arc<dyn RunEventSink>) -> Self {
        Self {
            composed: Arc::new(composed),
            sink,
        }
    }
}

impl WorkerAgentPort for KernelWorkerPort {
    fn run(&self, turn: WorkerTurnRequest) -> BoxTurnFuture {
        let composed = Arc::clone(&self.composed);
        let sink = Arc::clone(&self.sink);
        Box::pin(async move { execute(composed, sink, turn).await })
    }
}

/// 一轮会话执行全流程
async fn execute(
    composed: Arc<ComposedTurn>,
    sink: Arc<dyn RunEventSink>,
    turn: WorkerTurnRequest,
) -> Result<WorkerTurnOutcome, String> {
    // CLI 预检（CLI 引擎臂）
    if composed.is_cli_engine() {
        crate::discover::discover().map_err(|error| error.to_string())?;
    }
    let session = match turn.continue_session {
        None => SessionRef::New,
        Some(id) => SessionRef::Continue { id },
    };
    let ctx = SessionCtx {
        workspace_root: PathBuf::from(&turn.root),
        permission_mode: turn.permission,
    };
    let running = composed.begin(session, turn.prompt, ctx, turn.provenance)?;
    let session_id = running.session_id.clone();
    let transcript: Arc<Mutex<Vec<agent::AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let collected = Arc::clone(&transcript);
    let event_sink = Arc::clone(&sink);
    let emit_session = session_id.clone();
    let outcome: TurnOutcome = running
        .drive(move |output| match output {
            KernelOutput::Observation(event) => {
                // 密封事件累积（提取器输入全集）；增量只透传不累积
                if event.kind.is_sealed() {
                    collected
                        .lock()
                        .expect("转录累积锁不可中毒")
                        .push(event.clone());
                }
                event_sink.emit(RunUpdate::SessionEvent {
                    session_id: emit_session.clone(),
                    event,
                });
            }
            KernelOutput::TurnFinished(_) => {}
        })
        .await;
    let transcript = transcript.lock().expect("转录累积锁不可中毒").clone();
    Ok(WorkerTurnOutcome {
        session_id,
        status: outcome.status,
        final_message: final_assistant_text(&transcript),
        transcript,
    })
}

#[cfg(test)]
#[path = "worker_test.rs"]
mod worker_test;
